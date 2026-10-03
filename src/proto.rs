//! 在帧收发之上的一层命令封装，逐条对应 a980.html 里验证过的读法。

use crate::model::*;
use crate::tg::*;

fn safe<T>(log: &mut Vec<String>, what: &str, f: impl FnOnce() -> Result<T, String>) -> Option<T> {
    match f() {
        Ok(v) => Some(v),
        Err(e) => {
            log.push(format!("{} 读不到：{}", what, e));
            None
        }
    }
}

pub fn fw(tg: &Tg) -> Result<String, String> {
    let r = tg.xchg(CLS_DEV, 0 | GET, 0, 0, &[])?;
    Ok(format!("{}.{}.{}", r.u8(0), r.u8(1), r.u8(2)))
}

pub fn hw(tg: &Tg) -> Result<String, String> {
    let r = tg.xchg(CLS_DEV, 1 | GET, 0, 0, &[])?;
    Ok(format!("{}.x", r.u8(0)))
}

pub fn sn(tg: &Tg) -> Result<String, String> {
    let r = tg.xchg(CLS_DEV, 2 | GET, 0, 0, &[])?;
    let d = r.slice(0, 16);
    if d.iter().all(|b| *b == 0xFF) || d.iter().all(|b| *b == 0x00) {
        return Ok(String::new());
    }
    Ok(d.iter().map(|b| format!("{:02X}", b)).collect())
}

pub fn sensor(tg: &Tg) -> Result<Sensor, String> {
    let r = tg.xchg(CLS_SNR, 0 | GET, 8, 0, &[])?;
    Ok(Sensor {
        id: r.u8(0),
        model: r.u8(1),
        max: r.u16(2),
        // 官方驱动只读一个字节的步进，第四个字节开始是别的字段
        step: r.u8(4) as u16,
    })
}

pub fn polling(tg: &Tg) -> Result<Polling, String> {
    let r = tg.xchg(CLS_DEV, 4 | GET, 2, 0, &[])?;
    Ok(Polling {
        usb: r.u8(0),
        rate: r.u8(1),
    })
}

pub fn set_polling(tg: &Tg, usb: u8, rate: u8) -> Result<(), String> {
    // 前面那个 0 不能省：省了固件会把 usb 当成它，rate 位置对不上，直接答状态 0x03
    tg.xchg(CLS_DEV, 4 | SET, 2, 0, &[0, usb, rate])?;
    Ok(())
}

pub fn dpi_cfg(tg: &Tg, profile: u8, sensor_id: u8) -> Result<Dpi, String> {
    // 和写一样带两个选择字节；少带一个也能读（报告后面补的是 0），但对着写就看出来
    // 固件确实是按两个字节解析的
    let r = tg.xchg(CLS_SNR, 3 | GET, 0, profile, &[sensor_id, 0])?;
    let n = r.u8(1) as usize;
    let stage = r.u8(2) as usize;
    let mut x = Vec::with_capacity(n);
    let mut y = Vec::with_capacity(n);
    for t in 0..n {
        let o = 3 + 4 * t;
        x.push(r.u16(o));
        y.push(r.u16(o + 2));
    }
    Ok(Dpi { count: n, stage, x, y })
}

/// 写 DPI。固件要的是 5 档齐着带，不管当前是几档，缺的用最后一档补齐（官方驱动同款）。
///
/// payload 里 sensor 后面还要跟一个 0：不带的话固件把它后面的字节整体前移一位，
/// 档位数读回来会变成"原来的档位号"、DPI 值也会错位（真机实测）。
pub fn set_dpi(tg: &Tg, profile: u8, sensor_id: u8, stage: u8, x: &[u16], y: &[u16]) -> Result<(), String> {
    let fill = |v: &[u16], t: usize| -> u16 {
        if t < v.len() {
            v[t]
        } else {
            v.last().copied().unwrap_or(800).max(50)
        }
    };
    let mut pl = vec![sensor_id, 0, x.len() as u8, stage];
    for t in 0..5 {
        let px = fill(x, t).max(50);
        let py = fill(y, t).max(50);
        pl.push((px >> 8) as u8);
        pl.push(px as u8);
        pl.push((py >> 8) as u8);
        pl.push(py as u8);
    }
    tg.xchg(CLS_SNR, 3 | SET, 29, profile, &pl)?;
    Ok(())
}

pub fn button_ids(tg: &Tg) -> Result<Vec<u8>, String> {
    let d = tg.multi_get(CLS_BTN, 0 | GET, &[], 1, 0)?;
    Ok(d.into_iter().filter(|v| *v != 0).collect())
}

pub fn button_assign(tg: &Tg, k: u8) -> Result<Button, String> {
    // 回读校验键号：答复里 payload[0] 就是键号，对不上说明拿到的是别的键（或上一轮
    // 残留）的帧，等真正的答复
    let r = tg.xchg_echo(CLS_BTN, 3 | GET, 8, 0, &[0, k], &[k])?;
    Ok(Button {
        id: k,
        code: r.u8(2),
        data: r.slice(3, 8),
    })
}

/// 写按键映射。payload 从 [6] 起是 `00 k 00 功能码 数据×5`——**key 后面必须再跟一个
/// 0**。这是官方驱动的布局：它 PAYLOAD_BASE(=线缆第 7 字节) 开始送的是
/// [键号, 0, 功能码, 5 个数据]，size=8 数的正是这 8 个字节，[6] 那个 0 是额外的前导。
///
/// 少送这个 0，功能码整体前移一格：固件会把功能码当成第二选择字节、把数据的第一个
/// 字节当成功能码。拼出来无效就静默丢掉——但仍然答状态 0x02、还回显请求，看着像成功。
/// "禁用"（功能码和数据全是 0）碰巧能对上，所以这个错位只在改回真实功能码时才暴露：
/// 界面说"没写进去"，鼠标也真的没变。写键位务必回读比对，就是这个原因。
pub fn set_button_assign(tg: &Tg, k: u8, code: u8, data: &[u8]) -> Result<(), String> {
    let mut pl = vec![0, k, 0, code];
    pl.extend_from_slice(data);
    // 回显要连功能码带 5 个数据全对上：固件把这一串原样还回来，对得上才说明它照我们
    // 发的内容解析了
    let mut echo = vec![k, 0, code];
    echo.extend_from_slice(data);
    tg.xchg_echo(CLS_BTN, 3 | SET, 8, 0, &pl, &echo)?;
    Ok(())
}

pub fn led_ids(tg: &Tg) -> Result<Vec<u8>, String> {
    let d = tg.multi_get(CLS_LED, 0 | GET, &[], 1, 0)?;
    let mut out: Vec<u8> = Vec::new();
    for v in d {
        if v == 0 || v == 1 {
            if !out.iter().any(|x| *x == 0 || *x == 1) {
                out.push(v);
            }
        } else {
            out.push(v);
        }
    }
    Ok(out)
}

/// 灯区寻址要带一个前缀字节：`00 rg`，和按键寻址 `00 k` 一个路子。
/// 只送 `rg` 设备会答状态 0x03（参数不对），官方驱动里就是这么发的。
fn led_addr(rg: u8) -> [u8; 2] {
    [0, rg]
}

/// 返回（灯区类型, fps, 支持的灯效编号）
pub fn led_attr(tg: &Tg, rg: u8) -> Result<(u8, u8, Vec<u8>), String> {
    let r = tg.xchg(CLS_LED, 1 | GET, 1, 0, &led_addr(rg))?;
    let n = r.u8(5) as usize;
    let mut ef = Vec::new();
    for t in 0..n {
        let e = r.u8(6 + t);
        if e < 13 || e > 18 {
            ef.push(e);
        }
    }
    Ok((r.u8(1), r.u8(2), ef))
}

pub fn led_effect(tg: &Tg, profile: u8, rg: u8) -> Result<LedEffect, String> {
    let r = tg.xchg(CLS_LED, 2 | GET, 1, profile, &led_addr(rg))?;
    let n = r.u8(4) as usize;
    let mut colors = Vec::new();
    for t in 0..n {
        colors.push([r.u8(5 + 3 * t), r.u8(6 + 3 * t), r.u8(7 + 3 * t)]);
    }
    Ok(LedEffect {
        effect: r.u8(1),
        flag: r.u8(2),
        speed: r.u8(3),
        colors,
    })
}

pub fn set_led_effect(tg: &Tg, profile: u8, rg: u8, ef: &LedEffect) -> Result<(), String> {
    let mut pl = vec![0, rg, ef.effect, ef.flag, ef.speed, ef.colors.len() as u8];
    for c in &ef.colors {
        pl.extend_from_slice(c);
    }
    tg.xchg(CLS_LED, 2 | SET, (5 + 3 * ef.colors.len()) as u8, profile, &pl)?;
    Ok(())
}

pub fn led_bright(tg: &Tg, profile: u8, rg: u8) -> Result<u8, String> {
    // 亮度是 0..255 的裸字节，不是百分比——实测写 0x64 读回 0x64
    let r = tg.xchg(CLS_LED, 3 | GET, 2, profile, &led_addr(rg))?;
    Ok(r.u8(1))
}

pub fn set_led_bright(tg: &Tg, profile: u8, rg: u8, b: u8) -> Result<(), String> {
    tg.xchg(CLS_LED, 3 | SET, 2, profile, &[0, rg, b])?;
    Ok(())
}

pub fn profile_ids(tg: &Tg) -> Result<Vec<u8>, String> {
    Ok(tg.multi_get(CLS_PFL, 0 | GET, &[], 1, 0)?)
}

pub fn active_profile(tg: &Tg) -> Result<u8, String> {
    let r = tg.xchg(CLS_PFL, 3 | GET, 0, 0, &[])?;
    Ok(r.profile())
}

pub fn set_active_profile(tg: &Tg, id: u8) -> Result<(), String> {
    tg.xchg(CLS_PFL, 3 | SET, 0, id, &[])?;
    Ok(())
}

pub fn macro_ids(tg: &Tg) -> Result<Vec<u8>, String> {
    let d = tg.multi_get(CLS_MCO, 1 | GET, &[], 1, 0)?;
    Ok(d.into_iter().filter(|v| *v != 0).collect())
}

pub fn macro_name(tg: &Tg, id: u8) -> Result<String, String> {
    let d = tg.multi_get(CLS_MCO, 4 | GET, &[id], 1, 0)?;
    Ok(String::from_utf8_lossy(&d).trim_end_matches('\0').to_string())
}

pub fn macro_size(tg: &Tg, id: u8) -> Result<usize, String> {
    let d = tg.multi_get(CLS_MCO, 5 | GET, &[id], 2, 0)?;
    Ok(d.len())
}

pub fn lcd_lang(tg: &Tg) -> Result<u8, String> {
    let r = tg.xchg(CLS_LCD, 3 | GET, 0, 0, &[])?;
    Ok(r.u8(0))
}

pub fn set_lcd_lang(tg: &Tg, v: u8) -> Result<(), String> {
    tg.xchg(CLS_LCD, 3 | SET, 2, 0, &[0, v])?;
    Ok(())
}

/// 这台机的固件不支持调亮度（返回 0x05），读不到就该省掉 UI 上的那一行
pub fn lcd_bright(tg: &Tg) -> Result<u8, String> {
    let r = tg.xchg(CLS_LCD, 1 | GET, 0, 0, &[])?;
    Ok(r.u8(0))
}

pub fn set_lcd_bright(tg: &Tg, v: u8) -> Result<(), String> {
    tg.xchg(CLS_LCD, 1 | SET, 1, 0, &[v])?;
    Ok(())
}

pub fn battery(tg: &Tg) -> Result<u8, String> {
    let r = tg.xchg(CLS_PWR, 0 | GET, 3, 0, &[])?;
    if r.payload().iter().all(|b| *b == 0) {
        // 这台是有线鼠，PWR 类答一个空 payload。硬读会得到 0%，
        // 不如报个错，让界面显示"无电池"
        return Err("设备没有回报电池信息（有线鼠标）".into());
    }
    Ok(r.u8(1))
}

pub fn factory_reset(tg: &Tg) -> Result<(), String> {
    tg.xchg(CLS_DEV, 8 | SET, 1, 0, &[1])?;
    Ok(())
}

/// 一次全量读取。任何一项失败都不影响别的项，日志里记下来就行。
///
/// 读不到的项一律留空/None，不拿默认值顶上：界面上出现一个看着能调、其实没读到
/// 的假值，比少一行更糟（灯区尤其如此）。
pub fn read_all(tg: &Tg, iface: &str, log: &mut Vec<String>) -> Snapshot {
    let mut s = Snapshot {
        connected: true,
        busy: false,
        interface: iface.to_string(),
        ..Default::default()
    };
    s.fw = safe(log, "固件", || fw(tg)).unwrap_or_default();
    s.hw = safe(log, "硬件", || hw(tg)).unwrap_or_default();
    s.sn = safe(log, "序列号", || sn(tg)).unwrap_or_default();
    /* 当前配置档先读出来：DPI 和灯效都是按档存的（实测 0 档和 1 档的灯效
       flag/speed 不一样），鼠标实际跑的是活动档。读/写都必须跟着活动档走，
       死盯 0 档的话界面显示的是一份、鼠标亮的是另一份——官方驱动也是用
       ProfileId（活动档）寻址灯效的。 */
    let pf = safe(log, "当前配置", || active_profile(tg)).unwrap_or(0);
    let sensor = safe(log, "传感器", || sensor(tg));
    s.dpi = match &sensor {
        Some(sn) => safe(log, "DPI", || dpi_cfg(tg, pf, sn.id)).unwrap_or_default(),
        None => {
            log.push("DPI 读不到：没有传感器型号，不知道该问哪一颗".into());
            Dpi::default()
        }
    };
    s.sensor = sensor.unwrap_or_default();
    s.rate = safe(log, "回报率", || polling(tg)).unwrap_or_default();

    if let Ok(ids) = button_ids(tg) {
        let mut buttons = Vec::new();
        for k in ids {
            match button_assign(tg, k) {
                Ok(b) => buttons.push(b),
                Err(e) => log.push(format!("按键 {} 读不到：{}", k, e)),
            }
        }
        s.buttons = buttons;
    } else {
        log.push("按键列表读不到".into());
    }

    let mut leds = Vec::new();
    match led_ids(tg) {
        Ok(ids) => {
            for rg in ids {
                let attr = match led_attr(tg, rg) {
                    Ok(a) => a,
                    Err(e) => {
                        log.push(format!("灯区 {} 属性读不到：{}", rg, e));
                        continue;
                    }
                };
                let effect = match led_effect(tg, pf, rg) {
                    Ok(e) => e,
                    Err(e) => {
                        log.push(format!("灯区 {} 灯效读不到：{}", rg, e));
                        continue;
                    }
                };
                let bright = match led_bright(tg, pf, rg) {
                    Ok(b) => b,
                    Err(e) => {
                        log.push(format!("灯区 {} 亮度读不到：{}", rg, e));
                        continue;
                    }
                };
                leds.push(Led {
                    id: rg,
                    kind: attr.0,
                    fps: attr.1,
                    effects: attr.2,
                    bright,
                    effect,
                });
            }
        }
        Err(e) => log.push(format!("灯区列表读不到：{}", e)),
    }
    s.leds = leds;

    let mut macros = Vec::new();
    if let Ok(ids) = macro_ids(tg) {
        for id in ids {
            let name = match macro_name(tg, id) {
                Ok(n) if !n.is_empty() => n,
                _ => format!("宏 {}", id),
            };
            let size = macro_size(tg, id).unwrap_or_else(|e| {
                log.push(format!("宏 {} 长度读不到：{}", id, e));
                0
            });
            macros.push(Macro { id, name, size });
        }
    } else {
        log.push("宏列表读不到".into());
    }
    s.macros = macros;

    s.profiles = profile_ids(tg).unwrap_or_default();
    s.profile_active = pf;
    s.lcd_lang = lcd_lang(tg).ok();
    s.lcd_bright = lcd_bright(tg).ok();
    s.battery = battery(tg).ok();
    s
}
