//! tg 协议层。
//!
//! 线缆格式与字段含义逆向自 dr.dareu.com 的官方驱动，并用 PowerShell 在真机上
//! 逐条实测验证过（本机有线、FwType 0、ReportId 0）：
//!
//! ```text
//! [0] 报告 ID（0）  [1] 状态  [2] size  [3] class  [4] cmd  [5] profile  [6..] payload
//! ```
//!
//! 应答镜像这个布局，只是 payload 整体往后错一个字节（从 [7] 开始，[6] 恒为 0）。
//! 状态低 4 位等于 2 代表成功，0xAA 代表还没数据，0x05 代表
//! 这条命令固件不支持。桌面版走 Windows 的 HidD，报告长度固定 65 字节（含 ID），
//! 没有 WebHID 那种"浏览器替不替你补 ID 字节"的不确定性。

/// 一个 feature report 的总长度，含开头那个 report ID 字节
pub const REPORT: usize = 65;

/// 请求帧里 payload 的起点
const REQ_PAY: usize = 6;

/// 应答里 payload 的起点。
///
/// 比请求多一个字节，真机实测出来的：读 getFw 时 65 字节回报是
/// `00 02 03 00 80 00 | 00 01 02 01`，头六个字节之后还有一个 00，
/// 真正的 1.2.1 从 [7] 开始。所有字段都以 [7] 为基准读才和官方驱动对得上，
/// 拿 [6] 当起点会把固件读成 0.1.2、传感器读成型号 1。
const REPLY_PAY: usize = 7;

/// payload 上限（和官方驱动一致，别顶满）
const PAYLOAD_MAX: usize = 58;

pub const VID: u16 = 0x260D;
pub const PID: u16 = 0x1045;

pub const CLS_DEV: u8 = 0;
pub const CLS_BTN: u8 = 2;
pub const CLS_LED: u8 = 3;
pub const CLS_SNR: u8 = 4;
pub const CLS_PFL: u8 = 5;
pub const CLS_MCO: u8 = 6;
pub const CLS_PWR: u8 = 7;
pub const CLS_LCD: u8 = 11;

/// GET 位，和 cmd 按位或
pub const GET: u8 = 0x80;

/// 写命令（就是不加位）
pub const SET: u8 = 0x00;

const STATUS_EMPTY: u8 = 0xAA;

/// 一次收发的应答
pub struct Reply(pub [u8; REPORT]);

impl Reply {
    /// payload 视作以 [7] 为起点的字节流（见 REPLY_PAY 的说明）
    pub fn payload(&self) -> &[u8] {
        &self.0[REPLY_PAY..]
    }

    /// 应答回显的 profile 字节（在帧头，不在 payload 里）
    pub fn profile(&self) -> u8 {
        self.0[5]
    }

    pub fn u8(&self, i: usize) -> u8 {
        self.payload().get(i).copied().unwrap_or(0)
    }

    pub fn u16(&self, i: usize) -> u16 {
        let p = self.payload();
        ((p.get(i).copied().unwrap_or(0) as u16) << 8) | p.get(i + 1).copied().unwrap_or(0) as u16
    }

    pub fn slice(&self, from: usize, to: usize) -> Vec<u8> {
        let p = self.payload();
        let to = to.min(p.len());
        if from >= to {
            return Vec::new();
        }
        p[from..to].to_vec()
    }
}

/// 一条已打开的 tg 链路
pub struct Tg<'a> {
    pub dev: &'a hidapi::HidDevice,
}

impl<'a> Tg<'a> {
    /// 组一个请求帧。探针的扫描模式也要用它，所以放出来。
    pub fn frame(cls: u8, cmd: u8, size: u8, profile: u8, payload: &[u8]) -> [u8; REPORT] {
        let mut b = [0u8; REPORT];
        b[2] = size;
        b[3] = cls;
        b[4] = cmd;
        b[5] = profile;
        let n = payload.len().min(PAYLOAD_MAX);
        b[REQ_PAY..REQ_PAY + n].copy_from_slice(&payload[..n]);
        b
    }

    /// 一问一答。同一个 HID 端点只能串行，所以整个收发都对着一个设备句柄做完。
    ///
    /// 回帧要认得出是我们这条命令：鼠标一次只留一个答复位，另有进程（官方驱动、
    /// 第二个实例）同时在说的时候帧会串，只校验状态位会把别人的成功答复当成自己
    /// 的收下——界面报“改好了”，其实什么都没发生。串过来的帧直接放过重读，
    /// 不重发请求，免得 SET 被重复执行。
    pub fn xchg(&self, cls: u8, cmd: u8, size: u8, profile: u8, payload: &[u8]) -> Result<Reply, String> {
        self.xchg_echo(cls, cmd, size, profile, payload, &[])
    }

    /// 同 xchg，但额外要求应答 payload 的前 expect.len() 个字节和 expect 完全一致。
    ///
    /// 只校验 class/cmd 认不出"这帧是上一轮命令的残留"：鼠标那头答复只留一个位置，
    /// 连着问的时候会把上一条的答复当这次的弹回来（实测连着读按键，会冒出旁边几个
    /// 键的答复帧）。按键这类"寻址写在请求里、回显在答复里"的命令靠它兜底——
    /// 键号对不上就继续等真正的答复，宁可多等几轮也不把别人的帧收下。
    pub fn xchg_echo(
        &self,
        cls: u8,
        cmd: u8,
        size: u8,
        profile: u8,
        payload: &[u8],
        expect: &[u8],
    ) -> Result<Reply, String> {
        let out = Self::frame(cls, cmd, size, profile, payload);
        if let Err(e) = self.dev.send_feature_report(&out) {
            return Err(format!("发送失败：{}", e));
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
        let mut buf = [0u8; REPORT];
        for round in 0..20 {
            /* GET 到第 8、15 轮还没认出来就补发一次：鼠标一次只留一个答复位，上一轮
               的残帧还占着位置时它会把新答复整个丢掉，只靠等永远等不到。SET 绝不补发
               ——写命令重复执行不是幂等的（恢复出厂、切配置这类发两遍就出了两回事）。 */
            if (round == 8 || round == 15) && cmd & GET != 0 {
                let _ = self.dev.send_feature_report(&out);
            }
            buf[0] = 0;
            match self.dev.get_feature_report(&mut buf) {
                Ok(n) if n >= 2 => {
                    let st = buf[1];
                    /* 先认类/命令号，再看状态：帧对不上就是别的命令（或上一轮、另一个
                       进程）的答复，不能当自己的收下。原来只看状态位，别人一个失败的
                       状态 5 就会被当成"这条命令被拒"报上去——界面莫名少一行，
                       日志里还查不出原因。 */
                    if buf[3] != cls || buf[4] != cmd {
                        std::thread::sleep(std::time::Duration::from_millis(12));
                        continue;
                    }
                    if st == STATUS_EMPTY || st == 0x00 {
                        std::thread::sleep(std::time::Duration::from_millis(12));
                        continue;
                    }
                    if st & 0xF == 2 {
                        if !expect.is_empty() && buf[REPLY_PAY..].len() >= expect.len()
                            && buf[REPLY_PAY..REPLY_PAY + expect.len()] != *expect
                        {
                            std::thread::sleep(std::time::Duration::from_millis(12));
                            continue;
                        }
                        return Ok(Reply(buf));
                    }
                    return Err(format!("设备返回状态 0x{:02X}（c{} m0x{:02X}）", st, cls, cmd));
                }
                Ok(_) => {
                    std::thread::sleep(std::time::Duration::from_millis(12));
                }
                Err(e) => {
                    let msg = e.to_string();
                    if is_gone(&msg) {
                        return Err(msg);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(12));
                }
            }
        }
        Err(format!("设备无应答（c{} m0x{:02X}）", cls, cmd))
    }

    /// 分包读大块数据：先宣告要读多少，再 48 字节一片拉回来
    pub fn multi_get(&self, cls: u8, cmd: u8, prefix: &[u8], addr: usize, profile: u8) -> Result<Vec<u8>, String> {
        let plen = prefix.len();
        let first = self.xchg(cls, cmd, 1, profile, prefix)?;
        let p = first.payload();
        let mut total = 0usize;
        for k in 0..addr {
            total = (total << 8) | p.get(plen + k).copied().unwrap_or(0) as usize;
        }
        let mut out = vec![0u8; total];
        let mut got = 0usize;
        while got < total {
            let chunk = (total - got).min(48);
            let mut pl = Vec::with_capacity(plen + 2 * addr + chunk);
            pl.extend_from_slice(prefix);
            for k in 0..addr {
                pl.push((total >> (8 * (addr - 1 - k))) as u8);
            }
            for k in 0..addr {
                pl.push((got >> (8 * (addr - 1 - k))) as u8);
            }
            pl.extend(std::iter::repeat(0u8).take(chunk));
            let r = self.xchg(cls, cmd, (chunk + plen + 2 * addr) as u8, profile, &pl)?;
            let body = r.slice(plen + 2 * addr, plen + 2 * addr + chunk);
            let n = body.len().min(total - got);
            out[got..got + n].copy_from_slice(&body[..n]);
            got += chunk;
        }
        Ok(out)
    }
}

/// 设备被拔掉时 Windows 的错误文案里通常有这些词
pub fn is_gone(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("not connected") || m.contains("disconnected") || m.contains("handle is invalid")
}
