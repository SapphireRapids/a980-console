#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Tauri 壳。协议核在 a980_core 里，这一层只是 12 个命令的转发：
//! 界面（frontend/app.js）invoke 什么 op，这儿原样递给 ffi::tauri_call，
//! 成功回快照、失败回原因，不多造一层校验——写后回读核里已经做了。
//!
//! 参数名必须和前端 app.js 里 invoke 的字段一字对齐：Tauri 按参数名收字段，
//! 名字对不上就静默拿默认值/报缺字段。

use a980_core::{ffi, model::Snapshot};
use serde_json::json;

macro_rules! bridge {
    ($($name:ident => $call:expr),* $(,)?) => {
        $(
            #[tauri::command]
            fn $name() -> Result<Snapshot, String> {
                ffi::tauri_call($call, json!({}))
            }
        )*
    };
}

bridge!(
    connect => "connect",
    disconnect => "disconnect",
    factory_reset => "factory_reset",
    forget_interface => "forget_interface",
    refresh => "refresh",
);

#[tauri::command]
fn set_dpi(values: Vec<[u16; 2]>, stage: u8) -> Result<Snapshot, String> {
    ffi::tauri_call("set_dpi", json!({ "values": values, "stage": stage }))
}

#[tauri::command]
fn set_rate(usb: u8, rate: u8) -> Result<Snapshot, String> {
    ffi::tauri_call("set_rate", json!({ "usb": usb, "rate": rate }))
}

#[tauri::command]
fn set_profile(id: u8) -> Result<Snapshot, String> {
    ffi::tauri_call("set_profile", json!({ "id": id }))
}

#[tauri::command]
fn set_led_effect(
    region: u8,
    effect: u8,
    flag: u8,
    speed: u8,
    colors: Vec<[u8; 3]>,
) -> Result<Snapshot, String> {
    ffi::tauri_call(
        "set_led_effect",
        json!({ "region": region, "effect": effect, "flag": flag, "speed": speed, "colors": colors }),
    )
}

#[tauri::command]
fn set_led_bright(region: u8, bright: u8) -> Result<Snapshot, String> {
    ffi::tauri_call("set_led_bright", json!({ "region": region, "bright": bright }))
}

#[tauri::command]
fn set_button(btn: u8, code: u8, data: Vec<u8>) -> Result<Snapshot, String> {
    ffi::tauri_call("set_button", json!({ "btn": btn, "code": code, "data": data }))
}

#[tauri::command]
fn set_lcd_lang(lang: u8) -> Result<Snapshot, String> {
    ffi::tauri_call("set_lcd_lang", json!({ "lang": lang }))
}

#[tauri::command]
fn set_lcd_bright(bright: u8) -> Result<Snapshot, String> {
    ffi::tauri_call("set_lcd_bright", json!({ "bright": bright }))
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            connect,
            disconnect,
            factory_reset,
            forget_interface,
            refresh,
            set_dpi,
            set_rate,
            set_profile,
            set_led_effect,
            set_led_bright,
            set_button,
            set_lcd_lang,
            set_lcd_bright,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
