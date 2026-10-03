//! 对 WinUI 前端的唯一出口：`a980_call(JSON 请求) -> JSON 应答`。
//!
//! 前端（C#）只认这一个函数，协议层里的东西一律不伸手进来——HID 收发、接口探测、
//! 答复位复用、写后回读全都关在这个库里面，界面崩了也拖不垮鼠标状态。
//!
//! 应答永远是同一个形状：
//!
//! ```json
//! {"ok":true,"error":null,"snapshot":{ … }}
//! ```
//!
//! 失败也带一份当前快照回去，这样前端弹完错误还能照实刷新界面，不用再问一遍。

use crate::hid;
use crate::model::{Button, LedEffect, Snapshot};
use crate::proto as p;
use serde::{Deserialize, Serialize};
use crate::tg;
use std::sync::{Mutex, MutexGuard};

/// 一次调用能带回的最大字节数。快照含 15 个按键 + 4 个灯区 + 日志，几十 KB 足够。
const OUT_MAX: usize = 1 << 20;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Req {
    Refresh,
    Connect,
    Disconnect,
    ForgetInterface,
    FactoryReset,
    SetDpi {
        values: Vec<[u16; 2]>,
        stage: u8,
    },
    SetRate {
        usb: u8,
        rate: u8,
    },
    SetProfile {
        id: u8,
    },
    SetLedEffect {
        region: u8,
        effect: u8,
        flag: u8,
        speed: u8,
        colors: Vec<[u8; 3]>,
    },
    SetLedBright {
        region: u8,
        bright: u8,
    },
    SetButton {
        btn: u8,
        code: u8,
        data: Vec<u8>,
    },
    SetLcdLang {
        lang: u8,
    },
    SetLcdBright {
        bright: u8,
    },
}

#[derive(Serialize)]
struct Resp<'a> {
    ok: bool,
    error: Option<String>,
    snapshot: &'a Snapshot,
}

struct Core {
    mouse: Option<hid::Mouse>,
    /// 上次成功的那条接口，下次连接先试它
    known: Option<String>,
    snap: Snapshot,
    log: Vec<String>,
    /// 连不上时的原因，连上就清掉
    problem: Option<String>,
    /// 用户手动断开后，refresh 不要再自动重连
    manual_disconnect: bool,
}

/// 进程内唯一状态。前端可能是 STA 的 UI 线程调、也可能在任务线程里调，
/// 统一在这把锁上排队——HID 端点本来就只能一问一答。
static CORE: Mutex<Option<Core>> = Mutex::new(None);

fn core() -> MutexGuard<'static, Option<Core>> {
    let mut g = CORE.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        *g = Some(Core {
            mouse: None,
            known: hid::load_last_path(),
            snap: Snapshot::default(),
            log: Vec::new(),
            problem: None,
            manual_disconnect: false,
        });
    }
    g
}

fn tail(v: &[String], n: usize) -> Vec<String> {
    let start = v.len().saturating_sub(n);
    v[start..].to_vec()
}

/// 给前端的那份快照：把"现在连没连上、卡在哪"这类只有内核知道的事补齐。
fn snapshot(c: &mut Core) -> Snapshot {
    let mut s = c.snap.clone();
    match &c.mouse {
        Some(m) => {
            s.connected = true;
            s.interface = m.tag.clone();
            s.problem = None;
        }
        None => {
            s.connected = false;
            s.problem = c.problem.clone();
        }
    }
    s.busy = false;
    s.log = tail(&c.log, 150);
    s
}

/// 连上之后把状态读全，顺手记住这条接口
fn attach(c: &mut Core, m: hid::Mouse) {
    let (tag, path) = (m.tag.clone(), m.path.clone());
    c.known = Some(path.clone());
    hid::save_last_path(&path);
    c.mouse = Some(m);
    c.problem = None;
    let Core { mouse, log, snap, .. } = c;
    if let Some(m) = mouse.as_ref() {
        *snap = p::read_all(&m.tg(), &tag, log);
    }
}

/// 试一次连接。日志不封顶会一直长，超过就从头砍。
fn try_now(c: &mut Core) -> Result<hid::Mouse, String> {
    let Core { known, log, .. } = c;
    let r = hid::try_connect(log, known.clone());
    if log.len() > 400 {
        let drop = log.len() - 400;
        log.drain(0..drop);
    }
    r
}

/// 在已连接的鼠标上执行一次命令；设备被拔掉时自动退回未连接态。
/// 和写成功与否无关：只要错误文案像"设备没了"，这把就得当掉线处理。
fn with_mouse<T>(c: &mut Core, f: impl FnOnce(&tg::Tg) -> Result<T, String>) -> Result<T, String> {
    let Core { mouse, log, .. } = c;
    let m = mouse.as_ref().ok_or_else(|| "鼠标没连上".to_string())?;
    match f(&m.tg()) {
        Ok(v) => Ok(v),
        Err(e) => {
            log.push(format!("操作失败：{}", e));
            if tg::is_gone(&e) {
                *mouse = None;
            }
            Err(e)
        }
    }
}

/// 没连上就直接报错。早先这里会返回成功，界面弹"已写入"而鼠标什么都没改——
/// 宁可让前端把"连不上"弹出来。
fn must_connect(c: &Core) -> Result<(), String> {
    if c.mouse.is_none() {
        return Err("鼠标没连上".to_string());
    }
    Ok(())
}

/// 连着呢就重读一遍，没连上就顺手连一下（用户没手动断开的话）。
fn op_refresh(c: &mut Core) -> Result<(), String> {
    if c.mouse.is_none() {
        if c.manual_disconnect {
            c.snap.connected = false;
            return Ok(());
        }
        match try_now(c) {
            Ok(m) => {
                attach(c, m);
                c.manual_disconnect = false;
                return Ok(());
            }
            Err(e) => {
                c.problem = Some(e);
                c.snap.connected = false;
                return Ok(());
            }
        }
    }
    let Core { mouse, log, snap: out, .. } = c;
    if let Some(m) = mouse.as_ref() {
        let tag = m.tag.clone();
        *out = p::read_all(&m.tg(), &tag, log);
    }
    Ok(())
}

fn op_set_dpi(c: &mut Core, values: Vec<[u16; 2]>, stage: u8) -> Result<(), String> {
    must_connect(c)?;
    let sensor_id = c.snap.sensor.id;
    let xs: Vec<u16> = values.iter().map(|v| v[0]).collect();
    let ys: Vec<u16> = values.iter().map(|v| v[1]).collect();
    with_mouse(c, |tg| {
        // DPI 按档存，写当前活动档
        let pf = p::active_profile(tg)?;
        p::set_dpi(tg, pf, sensor_id, stage, &xs, &ys)
    })?;
    c.log.push("DPI 已写入".into());
    let Core { mouse, log, snap: out, .. } = c;
    if let Some(m) = mouse.as_ref() {
        let tag = m.tag.clone();
        *out = p::read_all(&m.tg(), &tag, log);
        /* 写完重读一遍，设备那边对不上就把话说清楚：写 5 档进去读回来不是
           那 5 档，就是固件没收下，不能让界面显示一个自己编的值。 */
        let want: Vec<[u16; 2]> = (0..5)
            .map(|t| {
                let f = |v: &Vec<u16>| if t < v.len() { v[t] } else { v.last().copied().unwrap_or(800) }.max(50);
                [f(&xs), f(&ys)]
            })
            .collect();
        let got: Vec<[u16; 2]> = out.dpi.x.iter().zip(out.dpi.y.iter()).map(|(x, y)| [*x, *y]).collect();
        if out.dpi.stage as u8 != stage || got != want {
            return Err(format!(
                "DPI 没写进去：写了第 {} 档 {:?}，读回第 {} 档 {:?}",
                stage, want, out.dpi.stage, got
            ));
        }
    }
    Ok(())
}

fn op_set_rate(c: &mut Core, usb: u8, rate: u8) -> Result<(), String> {
    must_connect(c)?;
    /* 固件答状态 2 只说明帧被收下了，不说明值真的变了。写完必须回读，
       对不上就照实报错——界面弹"改了"而设备没动，比弹错更糟。 */
    let back = with_mouse(c, |tg| {
        p::set_polling(tg, usb, rate)?;
        let back = p::polling(tg)?;
        if back.usb != usb || back.rate != rate {
            return Err(format!(
                "回报率没写进去：写了 usb {} / 码 {}，读回 usb {} / 码 {}",
                usb, rate, back.usb, back.rate
            ));
        }
        Ok(back)
    })?;
    c.log.push(format!("回报率已改成码 {}（回读一致）", rate));
    c.snap.rate = back;
    Ok(())
}

fn op_set_profile(c: &mut Core, id: u8) -> Result<(), String> {
    must_connect(c)?;
    let back = with_mouse(c, |tg| {
        p::set_active_profile(tg, id)?;
        let back = p::active_profile(tg)?;
        if back != id {
            return Err(format!("配置没切过去：写 {} 读回 {}", id, back));
        }
        Ok(back)
    })?;
    c.log.push(format!("已切到配置 {}（回读一致）", back));
    c.snap.profile_active = back;
    Ok(())
}

fn op_set_led_effect(
    c: &mut Core,
    region: u8,
    effect: u8,
    flag: u8,
    speed: u8,
    colors: Vec<[u8; 3]>,
) -> Result<(), String> {
    must_connect(c)?;
    let ef = LedEffect { effect, flag, speed, colors: colors.clone() };
    let back = with_mouse(c, |tg| {
        // 灯效按档存，写当前活动档——写死 0 档的话界面显示一份、鼠标亮的是另一份
        let pf = p::active_profile(tg)?;
        p::set_led_effect(tg, pf, region, &ef)?;
        let back = p::led_effect(tg, pf, region)?;
        if back != ef {
            return Err(format!("灯区 {} 的灯效没写进去：写 {:?}，读回 {:?}", region, ef, back));
        }
        Ok(back)
    })?;
    c.log.push(format!("灯区 {} 灯效改成 {}（回读一致）", region, effect));
    for l in c.snap.leds.iter_mut() {
        if l.id == region {
            l.effect = back.clone();
        }
    }
    Ok(())
}

fn op_set_led_bright(c: &mut Core, region: u8, bright: u8) -> Result<(), String> {
    must_connect(c)?;
    let back = with_mouse(c, |tg| {
        // 亮度同灯效，按当前活动档写
        let pf = p::active_profile(tg)?;
        p::set_led_bright(tg, pf, region, bright)?;
        let back = p::led_bright(tg, pf, region)?;
        if back != bright {
            return Err(format!("灯区 {} 的亮度没写进去：写 {}，读回 {}", region, bright, back));
        }
        Ok(back)
    })?;
    c.log.push(format!("灯区 {} 亮度改成 {}（回读一致）", region, back));
    for l in c.snap.leds.iter_mut() {
        if l.id == region {
            l.bright = back;
        }
    }
    Ok(())
}

fn op_set_button(c: &mut Core, btn: u8, code: u8, data: Vec<u8>) -> Result<(), String> {
    must_connect(c)?;
    let back = with_mouse(c, |tg| {
        p::set_button_assign(tg, btn, code, &data)?;
        /* 固件会把按键功能码和 5 个参数字节按自己的长度读回来，逐字节对上。
           回读最多三次、连着两次一样才算数：鼠标那头只留一个答复位，偶尔会把
           上一轮的答复当这次的弹回来（同键号的旧帧），只读一次可能读到写之前
           的旧值，界面就误报"没写进去"。回读本身也会偶发不答，所以读失败就再读，
           别拿一次误读当写失败报给用户。 */
        let mut first: Option<Button> = None;
        for i in 0..3 {
            let b = match p::button_assign(tg, btn) {
                Ok(b) => b,
                Err(e) => {
                    if first.is_none() && i == 2 {
                        return Err(format!("按键 {} 写完读不回来：{}", btn, e));
                    }
                    continue;
                }
            };
            match &first {
                Some(prev) if *prev == b => break,
                _ => first = Some(b),
            }
        }
        let back = first.ok_or_else(|| format!("按键 {} 写完读不回来", btn))?;
        let same =
            back.code == code && back.data.len() == data.len() && back.data.iter().zip(data.iter()).all(|(a, b)| a == b);
        if !same {
            return Err(format!(
                "按键 {} 没改成功：写功能 {} {:?}，读回功能 {} {:?}",
                btn, code, data, back.code, back.data
            ));
        }
        Ok(back)
    })?;
    c.log.push(format!("按键 {} 已改成功能 {}（回读一致）", btn, code));
    for b in c.snap.buttons.iter_mut() {
        if b.id == btn {
            b.code = back.code;
            b.data = back.data.clone();
        }
    }
    Ok(())
}

fn op_set_lcd_lang(c: &mut Core, lang: u8) -> Result<(), String> {
    must_connect(c)?;
    let back = with_mouse(c, |tg| {
        p::set_lcd_lang(tg, lang)?;
        let back = p::lcd_lang(tg)?;
        if back != lang {
            return Err(format!("屏幕语言没写进去：写 {}，读回 {}", lang, back));
        }
        Ok(back)
    })?;
    c.log.push(format!("屏幕语言已改成 {}（回读一致）", back));
    c.snap.lcd_lang = Some(back);
    Ok(())
}

fn op_set_lcd_bright(c: &mut Core, bright: u8) -> Result<(), String> {
    must_connect(c)?;
    let back = with_mouse(c, |tg| {
        p::set_lcd_bright(tg, bright)?;
        let back = p::lcd_bright(tg)?;
        if back != bright {
            return Err(format!("屏幕亮度没写进去：写 {}，读回 {}", bright, back));
        }
        Ok(back)
    })?;
    c.log.push(format!("屏幕亮度已改成 {}（回读一致）", back));
    c.snap.lcd_bright = Some(back);
    Ok(())
}

fn op_factory_reset(c: &mut Core) -> Result<(), String> {
    must_connect(c)?;
    with_mouse(c, |tg| p::factory_reset(tg))?;
    c.log.push("已下发恢复出厂，鼠标要重启一下（约 2 秒）".into());
    std::thread::sleep(std::time::Duration::from_millis(1500));
    c.mouse = None;
    Ok(())
}

fn handle(c: &mut Core, req: Req) -> Result<(), String> {
    match req {
        Req::Refresh => op_refresh(c),
        Req::Connect => match try_now(c) {
            Ok(m) => {
                attach(c, m);
                Ok(())
            }
            Err(e) => {
                c.problem = Some(e.clone());
                Err(e)
            }
        },
        Req::Disconnect => {
            c.mouse = None;
            c.problem = None;
            c.snap.connected = false;
            c.manual_disconnect = true;
            Ok(())
        }
        Req::ForgetInterface => {
            hid::forget_last_path();
            c.mouse = None;
            c.known = None;
            c.problem = None;
            c.log.push("已忘掉上次的接口，重新探测".into());
            Ok(())
        }
        Req::FactoryReset => op_factory_reset(c),
        Req::SetDpi { values, stage } => op_set_dpi(c, values, stage),
        Req::SetRate { usb, rate } => op_set_rate(c, usb, rate),
        Req::SetProfile { id } => op_set_profile(c, id),
        Req::SetLedEffect { region, effect, flag, speed, colors } => {
            op_set_led_effect(c, region, effect, flag, speed, colors)
        }
        Req::SetLedBright { region, bright } => op_set_led_bright(c, region, bright),
        Req::SetButton { btn, code, data } => op_set_button(c, btn, code, data),
        Req::SetLcdLang { lang } => op_set_lcd_lang(c, lang),
        Req::SetLcdBright { bright } => op_set_lcd_bright(c, bright),
    }
}

fn err_json(msg: &str) -> String {
    serde_json::to_string(&Resp { ok: false, error: Some(msg.to_string()), snapshot: &Snapshot::default() })
        .unwrap_or_else(|_| String::from("{\"ok\":false,\"error\":\"序列化失败\"}"))
}

fn process(json: &str) -> String {
    let req: Req = match serde_json::from_str(json) {
        Ok(r) => r,
        Err(e) => return err_json(&format!("请求不认识：{}", e)),
    };
    let mut g = core();
    let c = match g.as_mut() {
        Some(c) => c,
        None => return err_json("内核没起来"),
    };
    let r = handle(c, req);
    let snap = snapshot(c);
    match r {
        Ok(()) => match serde_json::to_string(&Resp { ok: true, error: None, snapshot: &snap }) {
            Ok(s) => s,
            Err(_) => err_json("应答序列化失败"),
        },
        Err(e) => match serde_json::to_string(&Resp { ok: false, error: Some(e), snapshot: &snap }) {
            Ok(s) => s,
            Err(_) => err_json("应答序列化失败"),
        },
    }
}

/// Tauri 命令层入口。前端 app.js 用 `invoke('set_dpi', {…})` 一字调用，这儿把
/// op 名和参数拼回内部请求再分发。写失败的原因 reject 回界面；connect 失败不
/// reject——界面要连快照一起读（原因在 problem 字段里），不然弹完错就瞎了。
pub fn tauri_call(op: &str, args: serde_json::Value) -> Result<Snapshot, String> {
    let mut obj = match args {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    obj.insert("op".into(), serde_json::Value::String(op.to_string()));
    let req: Req = serde_json::from_value(serde_json::Value::Object(obj))
        .map_err(|e| format!("请求不认识：{}", e))?;
    let mut g = core();
    let c = match g.as_mut() {
        Some(c) => c,
        None => return Err("内核没起来".to_string()),
    };
    let r = handle(c, req);
    let snap = snapshot(c);
    match r {
        Ok(()) => Ok(snap),
        Err(e) => {
            if op == "connect" {
                Ok(snap)
            } else {
                Err(e)
            }
        }
    }
}

/// 协议核的 ABI 版本。前端启动时可以核一次，对不上就明说，不要静默跑错字段。
#[no_mangle]
pub extern "C" fn a980_abi_version() -> u32 {
    2
}

/// 唯一的出入口。请求/应答都是 UTF-8 JSON。
///
/// 返回 0 成功（`*out_len` 是有效字节数）；负数是调用本身不对：
/// -1 空指针，-2 缓冲区太小（`*out_len` 置 0，前端应当扩容再来）。
/// 业务失败不走这里——它照样写一份 `{"ok":false,...}` 出去、返回 0。
#[no_mangle]
pub unsafe extern "C" fn a980_call(
    req: *const u8,
    req_len: u32,
    out: *mut u8,
    out_cap: u32,
    out_len: *mut u32,
) -> i32 {
    if out_len.is_null() {
        return -1;
    }
    unsafe { *out_len = 0 };
    if req.is_null() || out.is_null() {
        return -1;
    }
    let req = unsafe { std::slice::from_raw_parts(req, req_len as usize) };
    let js = match std::str::from_utf8(req) {
        Ok(s) => s,
        Err(e) => {
            let b = err_json(&format!("请求不是 UTF-8：{}", e)).into_bytes();
            if b.len() > out_cap as usize {
                return -2;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(b.as_ptr(), out, b.len());
                *out_len = b.len() as u32;
            }
            return 0;
        }
    };
    /* 协议层再稳也不该把一次 panic 泄过 FFI 边界——那会直接掀掉 WinUI 进程。
       在这儿兜住，照样回一份"内核崩了"，界面至少弹得出话说。 */
    let body = std::panic::catch_unwind(|| process(js))
        .unwrap_or_else(|_| err_json("内核出错：协议层panic 已拦下"));
    let b = body.into_bytes();
    if b.len() > out_cap.min(OUT_MAX as u32) as usize {
        return -2;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(b.as_ptr(), out, b.len());
        *out_len = b.len() as u32;
    }
    0
}
