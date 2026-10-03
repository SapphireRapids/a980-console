//! 送给前端的快照。字段名全部扁平、驼峰友好的 JSON，前端拿起来直接用。

use serde::Serialize;

#[derive(Serialize, Clone, Default, Debug, PartialEq)]
pub struct Sensor {
    pub id: u8,
    pub model: u8,
    pub max: u16,
    pub step: u16,
}

#[derive(Serialize, Clone, Default, Debug, PartialEq)]
pub struct Dpi {
    /// 可用档位数（真机 5）
    pub count: usize,
    /// 当前档，从 1 开始
    pub stage: usize,
    pub x: Vec<u16>,
    pub y: Vec<u16>,
}

#[derive(Serialize, Clone, Default, Debug, PartialEq)]
pub struct Polling {
    pub usb: u8,
    pub rate: u8,
}

#[derive(Serialize, Clone, Default, Debug, PartialEq)]
pub struct Button {
    pub id: u8,
    /// 功能码
    pub code: u8,
    /// 功能参数
    pub data: Vec<u8>,
}

#[derive(Serialize, Clone, Default, Debug, PartialEq)]
pub struct LedEffect {
    pub effect: u8,
    pub flag: u8,
    pub speed: u8,
    pub colors: Vec<[u8; 3]>,
}

#[derive(Serialize, Clone, Default, Debug, PartialEq)]
pub struct Led {
    pub id: u8,
    pub kind: u8,
    pub fps: u8,
    /// 这个灯区支持的灯效编号
    pub effects: Vec<u8>,
    pub bright: u8,
    pub effect: LedEffect,
}

#[derive(Serialize, Clone, Default, Debug, PartialEq)]
pub struct Macro {
    pub id: u8,
    pub name: String,
    pub size: usize,
}

#[derive(Serialize, Clone, Default, Debug, PartialEq)]
pub struct Snapshot {
    pub connected: bool,
    pub busy: bool,
    /// 正在用的接口路径（诊断用）
    pub interface: String,
    pub fw: String,
    pub hw: String,
    pub sn: String,
    pub sensor: Sensor,
    pub dpi: Dpi,
    pub rate: Polling,
    pub buttons: Vec<Button>,
    pub leds: Vec<Led>,
    pub macros: Vec<Macro>,
    pub profiles: Vec<u8>,
    pub profile_active: u8,
    pub lcd_lang: Option<u8>,
    pub lcd_bright: Option<u8>,
    pub battery: Option<u8>,
    /// 最近一次失败的说明，null 表示一切正常
    pub problem: Option<String>,
    /// 探测/收发日志，最新在最后
    pub log: Vec<String>,
    /// 用户手动断开后，前端不要再自动重连，等用户点“连接”
    pub manual_disconnect: bool,
}
