//! 协议核。Tauri 前端经 src/ffi.rs 的命令层进来，命令行探针（bin/probe.rs）
//! 直接调库——两边共用同一份代码，探针验证过的读法就是界面用的读法。

pub mod ffi;
pub mod hid;
pub mod model;
pub mod proto;
pub mod tg;
