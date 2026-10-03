//! 找出真正会答话的那条 HID 接口。
//!
//! 这支鼠标在系统里是一个复合设备，挂着七八条接口，全部共用同一个产品名：
//! 指针(1/e)、键盘、系统控制(1/80)、厂商自定义(ff00/1)，外加两条 usage 都是
//! 0x0c/1 的用户控制接口。tg 协议只活在 0x0c/1 那两条上，其中只有 mi_03 那条带
//! 65 字节 feature 报告、会答话，另一条 feature 长度 0，是个哑接口。
//!
//! 桌面版的优势就在这：没有浏览器选择框，我们直接枚举全部接口路径，自己挑。
//! 挑法还是"用一句 getFw 问一下"，谁答得上用谁——认死 usage 没问题，但绝不拿
//! 固件版本、接口序号之类的启发式当下结论，否则换个型号就白写。

use crate::tg::{Tg, CLS_DEV, GET, PID, VID};

pub struct Candidate {
    pub path: String,
    pub page: u16,
    pub usage: u16,
    pub product: String,
}

impl Candidate {
    /// 带 mi_0X 尾巴的短标签，日志里一眼看出是哪一条
    pub fn tag(&self) -> String {
        let iface = self.path.split("mi_").nth(1).map(|s| {
            let end = s.find('&').unwrap_or(s.len());
            format!("mi_{}", &s[..end.min(2)])
        });
        match iface {
            Some(i) => format!("{} [{:x}/{:x}] {}", self.short_name(), self.page, self.usage, i),
            None => format!("{} [{:x}/{:x}]", self.short_name(), self.page, self.usage),
        }
    }

    fn short_name(&self) -> String {
        let mut s = self.product.clone();
        if let Some(rest) = s.strip_prefix("DAREU ") {
            s = rest.to_string();
        }
        s.trim_end_matches(" Gaming Mouse").to_string()
    }

    /// 尝试顺序：会答话的排前面，指针/键盘这种可能影响正常输入的排最后
    fn rank(&self) -> u8 {
        match (self.page, self.usage) {
            (0x0C, 0x01) => 0,
            (0xFF00, _) => 1,
            (0x01, 0x80) => 2,
            (0x01, 0x02) | (0x01, 0x06) => 3,
            (0x01, 0x0E) => 9,
            _ => 5,
        }
    }
}

/// 枚举所有 A980 接口，按尝试顺序排好
pub fn candidates(api: &hidapi::HidApi) -> Vec<Candidate> {
    let mut v: Vec<Candidate> = api
        .device_list()
        .filter(|d| d.vendor_id() == VID && d.product_id() == PID)
        .map(|d| Candidate {
            path: d.path().to_string_lossy().into_owned(),
            page: d.usage_page(),
            usage: d.usage(),
            product: d.product_string().unwrap_or("A980").to_string(),
        })
        .collect();
    v.sort_by_key(|c| c.rank());
    v
}

pub struct Mouse {
    pub dev: hidapi::HidDevice,
    pub path: String,
    pub tag: String,
}

impl Mouse {
    pub fn tg(&self) -> Tg<'_> {
        Tg { dev: &self.dev }
    }
}

/// 按顺序试，谁答得上 getFw 就用谁。返回 None 时 log 里已经写清每个候选怎么死的。
///
/// 每次调用都新建一个 HidApi：设备插拔之后需要重新枚举，而一个常驻的
/// HidApi 里缓存的是建立时的设备列表。api 用完就丢，打开的句柄照样有效
/// （Windows 上 HidDevice 自己持有句柄，不引用 api）。
pub fn try_connect(log: &mut Vec<String>, last_path: Option<String>) -> Result<Mouse, String> {
    let api = hidapi::HidApi::new().map_err(|e| format!("HID 初始化失败：{}", e))?;
    let list = candidates(&api);
    if list.is_empty() {
        let msg = "没枚举到 A980 接口，鼠标可能没插好";
        log.push(msg.into());
        return Err(msg.into());
    }

    let mut ordered: Vec<&Candidate> = list.iter().collect();
    if let Some(p) = last_path {
        if let Some(idx) = ordered.iter().position(|c| c.path == p) {
            let hit = ordered.remove(idx);
            ordered.insert(0, hit);
        }
    }

    let mut why: Vec<String> = Vec::new();
    for c in ordered {
        let cs = match std::ffi::CString::new(c.path.clone()) {
            Ok(cs) => cs,
            Err(_) => continue,
        };
        let dev = match api.open_path(cs.as_c_str()) {
            Ok(d) => d,
            Err(e) => {
                why.push(format!("{} → 打不开：{}", c.tag(), e));
                continue;
            }
        };
        let m = Mouse {
            dev,
            path: c.path.clone(),
            tag: c.tag(),
        };
        match m.tg().xchg(CLS_DEV, 0 | GET, 0, 0, &[]) {
            Ok(r) => {
                let fw = format!("{}.{}.{}", r.u8(0), r.u8(1), r.u8(2));
                log.push(format!("连上 {}（固件 {}）", m.tag, fw));
                return Ok(m);
            }
            Err(e) => {
                why.push(format!("{} → 问了 getFw 没答话：{}", c.tag(), e));
                continue;
            }
        }
    }
    let summary = if why.is_empty() {
        "没有可试的接口".to_string()
    } else {
        why.join("；")
    };
    log.push(format!("探测失败：{}", summary));
    Err(format!("鼠标不答话：{}", summary))
}

/// 上次成功的接口路径，存 %APPDATA%/a980-console/last-interface.txt
pub fn load_last_path() -> Option<String> {
    let dir = std::env::var("APPDATA").ok()?;
    let p = std::path::Path::new(&dir).join("a980-console").join("last-interface.txt");
    let s = std::fs::read_to_string(p).ok()?;
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

pub fn save_last_path(p: &str) {
    if let Ok(dir) = std::env::var("APPDATA") {
        let d = std::path::Path::new(&dir).join("a980-console");
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join("last-interface.txt"), p);
    }
}

pub fn forget_last_path() {
    if let Ok(dir) = std::env::var("APPDATA") {
        let _ = std::fs::remove_file(std::path::Path::new(&dir).join("a980-console").join("last-interface.txt"));
    }
}
