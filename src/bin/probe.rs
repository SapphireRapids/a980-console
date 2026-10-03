//! 命令行探针：不弹窗口，把一次全量读取得快照打成 JSON 打到标准输出。
//!
//! 用途有两个：一是在没有图形界面的环境下验证协议层读法对不对（测试机、
//! 远程排障），二是把每个候选接口是怎么死的打出来，比界面里的日志更全。
//!
//! ```text
//! probe          连上读一遍，漂亮 JSON
//! probe -j       紧凑 JSON，方便管道
//! probe -f       忘掉上次的接口，重新全量探测
//! probe --verify 写路径自检：把读到的值原样写回，再读一遍比对（不改任何设置）
//! ```

use a980_core::{hid, proto, tg};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let compact = args.iter().any(|a| a == "-j" || a == "--json");
    let fresh = args.iter().any(|a| a == "-f" || a == "--fresh");
    if args.iter().any(|a| a == "--scan") {
        scan();
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--do") {
        do_cmd(&args[i + 1..]);
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--btnset") {
        btn_set(&args[i + 1..]);
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--pr") {
        pr_cmd(&args[i + 1..]);
        return;
    }
    let verify = args.iter().any(|a| a == "--verify" || a == "--verify-write");

    let mut log: Vec<String> = Vec::new();
    let last = if fresh { None } else { hid::load_last_path() };
    eprintln!(
        "上次成功的接口：{}",
        last.as_deref().unwrap_or("（没记过，全量探测）")
    );

    let m = match hid::try_connect(&mut log, last) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("连不上：{}", e);
            dump(&log);
            std::process::exit(2);
        }
    };
    eprintln!("用 {} 通讯", m.tag);
    hid::save_last_path(&m.path);

    let snap = proto::read_all(&m.tg(), &m.tag, &mut log);
    if verify {
        verify_writes(&m);
        return;
    }
    if compact {
        println!("{}", serde_json::to_string(&snap).unwrap_or_default());
    } else {
        println!("{}", serde_json::to_string_pretty(&snap).unwrap_or_default());
    }
    dump(&log);
}

fn dump(log: &[String]) {
    for l in log {
        eprintln!("[log] {}", l);
    }
}

/// 写路径自检。
///
/// 每条写命令都把刚读到的值原样写回去，再读一遍比对。写进去的是原值，等于什么都没改，
/// 但固件认不认这个请求帧（状态字节对不对）、写进去和读回来一不一致，就都验掉了。
/// 灯效、亮度、DPI 这些"读不回去就没法确认写对"的项，只有这一条路能验证。
fn verify_writes(m: &hid::Mouse) {
    let t = m.tg();
    // 后面全都要 &Tg，统一借一次，免得每个调用点写 &
    let t = &t;
    let mut fail = 0usize;
    let mut skip = 0usize;

    /// 读→原样写回→再读→比对。读不到就跳过（不是写路径的错）。
    fn rt<T, R, W>(name: &str, read: R, write: W, fail: &mut usize, skip: &mut usize) -> bool
    where
        T: PartialEq + std::fmt::Debug,
        R: Fn() -> Result<T, String>,
        W: Fn(&T) -> Result<(), String>,
    {
        let v = match read() {
            Ok(v) => v,
            Err(e) => {
                println!("SKIP {} 读不到：{}", name, e);
                *skip += 1;
                return true;
            }
        };
        if let Err(e) = write(&v) {
            println!("FAIL {} 写入被拒：{}（原值 {:?}）", name, e, v);
            *fail += 1;
            return false;
        }
        match read() {
            Ok(after) if after == v => {
                println!("OK   {} = {:?}", name, v);
                true
            }
            Ok(after) => {
                println!("FAIL {} 写 {:?} 读回 {:?}", name, v, after);
                *fail += 1;
                false
            }
            Err(e) => {
                println!("FAIL {} 写后读不回：{}", name, e);
                *fail += 1;
                false
            }
        }
    }

    println!("== 写路径自检：读到的值原样写回，不改任何设置 ==");

    // DPI 和灯效都按档存，自检跟着当前活动档走
    let pf = proto::active_profile(t).unwrap_or(0);
    println!("（当前活动配置档 {}）", pf);

    rt("屏幕语言", || proto::lcd_lang(t), |v| proto::set_lcd_lang(t, *v), &mut fail, &mut skip);

    let sid = match proto::sensor(t) {
        Ok(s) => s.id,
        Err(e) => {
            println!("SKIP 传感器读不到：{}", e);
            skip += 1;
            0
        }
    };
    rt("DPI 档位", || proto::dpi_cfg(t, pf, sid), |d| {
        proto::set_dpi(t, pf, sid, d.stage as u8, &d.x, &d.y)
    }, &mut fail, &mut skip);

    rt("回报率", || proto::polling(t), |p| proto::set_polling(t, p.usb, p.rate), &mut fail, &mut skip);
    rt("当前配置", || proto::active_profile(t), |v| proto::set_active_profile(t, *v), &mut fail, &mut skip);

    match proto::led_ids(t) {
        Ok(ids) => {
            for rg in ids {
                rt(&format!("灯区 {} 灯效", rg), || proto::led_effect(t, pf, rg), |e| {
                    proto::set_led_effect(t, pf, rg, e)
                }, &mut fail, &mut skip);
                rt(&format!("灯区 {} 亮度", rg), || proto::led_bright(t, pf, rg), |b| {
                    proto::set_led_bright(t, pf, rg, *b)
                }, &mut fail, &mut skip);
            }
        }
        Err(e) => {
            println!("SKIP 灯区列表读不到：{}", e);
            skip += 1;
        }
    }

    match proto::button_ids(t) {
        Ok(ids) => {
            for k in ids {
                rt(&format!("按键 {}", k), || proto::button_assign(t, k), |b| {
                    proto::set_button_assign(t, k, b.code, &b.data)
                }, &mut fail, &mut skip);
            }
        }
        Err(e) => {
            println!("SKIP 按键列表读不到：{}", e);
            skip += 1;
        }
    }

    println!("== 自检结束：{} 项失败，{} 项跳过 ==", fail, skip);
}

/// 扫描模式：每条接口都问一次 getFw，把整帧打出来。
/// 只读不写。用来确认哪条接口真的在答 tg 协议、答的是什么。
fn scan() {
    let api = match hidapi::HidApi::new() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("HID 初始化失败：{}", e);
            std::process::exit(2);
        }
    };
    for c in hid::candidates(&api) {
        println!("=== {}  路径 {}", c.tag(), c.path);
        let cs = match std::ffi::CString::new(c.path.clone()) {
            Ok(cs) => cs,
            Err(_) => continue,
        };
        let dev = match api.open_path(cs.as_c_str()) {
            Ok(d) => d,
            Err(e) => {
                println!("    打不开：{}", e);
                continue;
            }
        };
        let out = tg::Tg::frame(tg::CLS_DEV, tg::GET, 0, 0, &[]);
        if let Err(e) = dev.send_feature_report(&out) {
            println!("    发送失败：{}", e);
            continue;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
        let mut buf = [0u8; tg::REPORT];
        for i in 0..3 {
            buf[0] = 0;
            match dev.get_feature_report(&mut buf) {
                Ok(n) => println!("    第{}次回报 {} 字节：{}", i + 1, n, hex(&buf)),
                Err(e) => println!("    第{}次读取失败：{}", i + 1, e),
            }
            std::thread::sleep(std::time::Duration::from_millis(12));
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02X}", x)).collect::<Vec<_>>().join(" ")
}

/// 带 profile 的手工读：probe --pr <profile> <class> <cmd> <size> [payload...]
/// 用来核对"当前配置档"到底是哪一档、界面读的 0 档和鼠标实际跑的档是否同一份。
fn pr_cmd(rest: &[String]) {
    let num = |s: &str| u8::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0);
    if rest.len() < 4 {
        eprintln!("用法：probe --pr <profile> <class> <cmd> <size> [payload...]");
        std::process::exit(2);
    }
    let profile = num(&rest[0]);
    let cls = num(&rest[1]);
    let cmd = num(&rest[2]);
    let size = num(&rest[3]);
    let payload: Vec<u8> = rest[4..].iter().map(|s| num(s)).collect();

    let mut log = Vec::new();
    let m = match hid::try_connect(&mut log, hid::load_last_path()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("连不上：{}", e);
            dump(&log);
            std::process::exit(2);
        }
    };
    let t = m.tg();
    println!("profile {} class {} cmd 0x{:02X} size {} payload {}", profile, cls, cmd, size, hex(&payload));
    match t.xchg(cls, cmd, size, profile, &payload) {
        Ok(r) => println!("  {}", hex(&r.0)),
        Err(e) => println!("  失败：{}", e),
    }
}

/// 手工发一条：probe --do <class> <cmd> <size> [payload 十六字节...]
/// 排障时用来看某条命令的原始回报。
fn do_cmd(rest: &[String]) {
    let num = |s: &str| u8::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0);
    if rest.len() < 3 {
        eprintln!("用法：probe --do <class> <cmd> <size> [payload...]");
        std::process::exit(2);
    }
    let cls = num(&rest[0]);
    let cmd = num(&rest[1]);
    let size = num(&rest[2]);
    let payload: Vec<u8> = rest[3..].iter().map(|s| num(s)).collect();

    let mut log = Vec::new();
    let m = match hid::try_connect(&mut log, hid::load_last_path()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("连不上：{}", e);
            dump(&log);
            std::process::exit(2);
        }
    };
    println!("请求 class {} cmd 0x{:02X} size {} payload {}", cls, cmd, size, hex(&payload));
    // 走原始收发，状态不对也把整帧打出来——排障要看的就是失败时长什么样
    let out = tg::Tg::frame(cls, cmd, size, 0, &payload);
    if let Err(e) = m.dev.send_feature_report(&out) {
        println!("发送失败：{}", e);
        return;
    }
    std::thread::sleep(std::time::Duration::from_millis(25));
    let mut buf = [0u8; tg::REPORT];
    for i in 0..4 {
        buf[0] = 0;
        match m.dev.get_feature_report(&mut buf) {
            Ok(n) => println!("应答（第{}次，{} 字节）：{}", i + 1, n, hex(&buf)),
            Err(e) => println!("读取失败：{}", e),
        }
        std::thread::sleep(std::time::Duration::from_millis(12));
    }
}

/// 复刻界面"改按键"那一步：probe --btnset <键号> <功能码> [参数字节...]
/// 在同一个进程里 读→写→回读，三帧原始字节全打出来——排查"界面说没写进去"用。
fn btn_set(rest: &[String]) {
    let num = |s: &str| u8::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0);
    if rest.len() < 2 {
        eprintln!("用法：probe --btnset <键号> <功能码> [d0..d4]");
        std::process::exit(2);
    }
    let k = num(&rest[0]);
    let code = num(&rest[1]);
    let mut data: Vec<u8> = rest[2..].iter().map(|s| num(s)).collect();
    data.resize(5, 0);
    data.truncate(5);

    let mut log = Vec::new();
    let m = match hid::try_connect(&mut log, hid::load_last_path()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("连不上：{}", e);
            dump(&log);
            std::process::exit(2);
        }
    };
    let t = m.tg();

    let raw = |tag: &str, r: &tg::Reply| {
        println!("  {}: {}", tag, hex(&r.0));
    };

    println!("改按键 {}：功能 {} 参数 {:?}", k, code, data);
    match proto::button_assign(&t, k) {
        Ok(b) => println!("  改之前：功能 {} 参数 {:?}", b.code, b.data),
        Err(e) => println!("  改之前读不到：{}", e),
    }

    if let Err(e) = proto::set_button_assign(&t, k, code, &data) {
        println!("  写入被拒：{}", e);
        return;
    }
    println!("  写命令已答成功");

    // 回读连读三次：第一帧偶尔是上一轮的残留，看它多久才稳定
    for i in 0..3 {
        match t.xchg(tg::CLS_BTN, 3 | tg::GET, 8, 0, &[0, k]) {
            Ok(r) => {
                let c = r.u8(2);
                let d = r.slice(3, 8);
                let verdict = if c == code && d == data { "一致" } else { "不一致" };
                println!("  回读第{}次：功能 {} 参数 {:?} <== {}", i + 1, c, d, verdict);
                raw("    原帧", &r);
            }
            Err(e) => println!("  回读第{}次失败：{}", i + 1, e),
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
