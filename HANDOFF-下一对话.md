# 交接：下一轮目标 = Tauri 前端

> **已完成（2026-10-01 图标 v3）**：用户否掉初版鼠标写生图标后改走"控制台"隐喻：
> indigo squircle 底 + 白旋钮 + 五档刻度弧，指针指第 4 档、当前档琥珀点亮（对应
> DPI 五档选一档这个核心动作，也对应真机现状）。生成器 `tools/gen_icon.mjs`
> （纯 node 内置 zlib，SDF + 3×3 超采样，`node tools/gen_icon.mjs icons` 一键重出
> 32/128/256/512 PNG + 7 档 ICO）。最终 exe 9,061,888 B，两处副本 md5 一致，
> PowerShell `tools\extract_exe_icon.ps1` 抽取核验，accept_test rc=0、鼠标基线未动。
> 本轮学费：① RC.EXE 对 ICO 极挑剔——ICONDIR 的 bytesInRes/imageOffset 是小端，
> 写成大端直接 RC2176；② 遗留 msedgewebview2.exe 死进程或双开同应用会让新实例
> panic 在 `create webview HRESULT(0x8007139F)`（共用 EBWebView 数据目录），
> 起壳前先清残留；③ exe 里图标换了不等于资源管理器显示换了——外壳图标缓存，
> `ie4uinit.exe -show` 起步、注销保底。v1/v2 图标归档在
> `可清理\a980-死路-2026-10-01\旧图标-被替换\`。
> 下文为 2026-10-01 早先的重建记录与更早历史。

> **已完成（2026-10-01）**：老壳 `A980控制台.exe` 实测仍能连真机；构建配方已从零
> 写回并重新编译验证通过。现在的状态：`cargo build --release --bin a980-console`
> 出 `target/release/a980-console.exe`（9,054,208 B，Tauri 2.12.1 + custom-protocol），
> 真机联调通过（连鼠标、DPI 同值回读、灯效亮度回读、断开重连均正常）。
> 配方四件套：`tauri.conf.json`（root，frontendDist="frontend"）、`build.rs`、
> `src/main.rs`（12 个命令转调 `ffi::tauri_call`）、Cargo.toml 的 tauri 段。
> `ffi.rs` 里加了 `pub fn tauri_call(op, args)`，老的 C ABI 一行没动。
> 前端是从老 exe 里抽回来的（WebView2 远程调试端口抓 `/app.js`、`/style.css` 和
> DOM），已落盘 `frontend/`。验收脚本：`tools/accept_test.mjs`。下文为历史记录。

> 写于 2026-10-01。上一轮拍板的 WinUI 3 路线已在本机判死，本轮结论见文末"为什么转 Tauri"。
> 这份文件的目的：让下一个对话不靠任何口头上下文，直接接着干。

## 一句话现状

**协议核完整活着且真机逐帧验证过，一个字没动；死掉的只是 WinUI 壳。前端改回 Tauri（Rust + WebView），协议核直接当 Rust 依赖调，连 FFI 都不必过。**

## 资产盘点（全部实地核过）

| 资产 | 位置 | 状态 |
|---|---|---|
| Rust 协议核源码 | `a980-app/src/{lib,ffi,hid,tg,proto,model}.rs` | 完整，未动 |
| cdylib 产物 | `a010-app/target/release/a010_core.dll` | 在，ABI=2 |
| rlib 产物 | `a010-app/target/release/liba010_core.rlib` | 在（Tauri 直接依赖用） |
| 命令行探针 | `a010-app/target/release/a010-probe.exe` + `src/bin/probe.rs` | 在，`--verify` 22 项真机 0 失败 |
| 老 Tauri 发布壳 | `a010-app/A980控制台.exe` | **在，恰 9,394,176 B**（README 判"对的样子"的字节数），10-01 构建 |
| 前端界面 | `a980.html`（= `A980鼠标控制台.html`，57,580 B） | 在，即当年 Tauri 的 `frontend/` 内容 |
| `frontend/` 目录 | `a010-app/frontend/` | **存在但已被删空** |
| WinUI 壳 | `a010-app/winui/` | 编译过，启动崩，见文末（2026-10-01 已移到“可清理/a980-死路-2026-10-01/winui”） |
| 官方驱动 web 资产 | `dareu_web/` | 逆向参考资料，非本程序前端 |

Cargo.toml 现状：`crate-type = ["cdylib", "rlib"]`——**rlib 出口是当年给 WinUI 打包时顺手留的，现在正好给 Tauri 用。**

## 下一轮施工顺序（建议照这个来）

1. **确认 `A980控制台.exe` 还能不能跑**：核心源码没动过、它自己 byte-exact 对得上 README 的 9,394,176，大概率仍然完好。先双击它——**如果它开得出界面连得上鼠标，Tauri 壳其实从来没死，只是仓库里的构建配方被删了**，那第一优先是从这个 exe 反查/重建配方（`tauri.conf.json` + `build.rs` + `src/main.rs` + Cargo.toml 的 tauri 段），而不是从零写。
2. 若 exe 已不可用：重建壳。需要补回四样——
   - `taauri.conf.json`（窗口/图标/`frontendDist` 记死是 `"frontend"`：相对 tauri.conf.json 所在目录，写 `"../frontend"` dev 模式不报错、内嵌编译直接失败）；
   - `build.rs` 里 `taauri_build::build()`；
   - `src/main.rs`：Tauri 命令层。**最省事的接法**——不重抄协议，把 `src/lib.rs` 的模块直接拿来用：`use a010_core::{hid, proto, model, tg};`，每个 Tauri command 转调 proto 层。协议核里 `hid::Mouse` / `proto::read_all` / `proto::set_*` 全是 `pub`，且每条写命令**核里已经自带写后回读比对**（回报率/DPI/灯效/亮度/按键/语言全有，对不上直接返回 Err），Tauri 层只管转发，不要再造一层校验。
   - Cargo.toml 加回 `taauri = { version = "2", features = ["custom-protocol"] }` + `taauri-build`。
3. `frontend/` 实质是空的：把 `a980.html` 拷进去当 index.html。README"编译"节第 3 条的坑仍然成立——**Tauri v2 只注入 `window.__TAURI_INTERNALS__`，不注入 `window.__TAURI__`**；前端探不到桥就会静默跑合成数据，界面看着连上、写入全落内存、鼠标一动不动。当年是在 `app.js` 里三种形状都认。
4. **`custom-protocol` feature 是生死线**：纯 `cargo build`（不走 tauri CLI）时不带它，前端一律去连 devUrl，用户机上 127.0.0.1 拒绝连接。
5. 编出来先跑 `cargo run --release --bin probe -- --verify`，22 项过了再碰 UI；真机联调时**别让官方驱动/第二个实例/探针同时跟鼠标说话**（鼠标只留一个答复位，会串帧）。
6. 排障脚本在 `..\tg_*.ps1` + `hid_caps.ps1`/`hid_scan.ps1`（工作区根），当年逐帧抓协议就是用它们。

## 别踩的坑（全部真机付过学费，README 第 137–202 行是全文）

- **所有写请求 payload 前都要多垫一个字节**（`00 rg` / `00 k` / `00 值` / `00 usb rate`）；固件 payload 从报告第 7 字节起算，不垫就整体错一位。
- **按键 payload 是 `00 k 00 功能码 数据×5`**，键号后必须再跟一个 0。少送它固件照样答 0x02、回显请求，看着像成功——只有回读抖得出。
- 回读要认帧：只认 class/cmd 对得上的答复帧，GET 等八轮没人应就补发一次，**写命令绝不补发**。
- 亮度是 0–255 裸字节不是百分比；回报率码见 README。
- 配置档切换只验证过"原样切回当前档"，切到别的档没试过，界面上原文写明了。
- 恢复出厂后鼠标重启约 2 秒，界面要会自己重连。
- 屏幕亮度这台机固件不支持（类 11 只有 0x83 语言答话），界面自动藏那行。
- 按键 18/19 当前功能码是 6，菜单里没对应项，只能显示不能写回。

## 为什么转 Tauri（WinUI 判死过程，别再重走）

启动崩溃定性为**本机环境问题，非代码问题**，证据：本机另一个完全无关的 WinUI 程序 GlassDemo 崩同码同模块家族；OS 报 26200 但 CoreMessagingXP.dll 是 10.0.27200.1054（27200 Insider 组件）；`WindowsAppSDKSelfControlled=true` 自包含后崩点从 XAML 首帧（0xc000027b）前移到 `new Window()`（0xc0000602，故障模块 CoreMessagingXP.dll）；三档探针（A980_MINIMAL/A980_LEVEL，App.xaml.cs 里有）证明 Button 模板物化、App.xaml 样式、xbf 加载全部无辜。持久记忆：`a010-console-winui-crash-saga.md`。

## 下一对话开播第一句可以直接说

> "接着 HANDOFF-下一对话.md 干：目标 Tauri 前端。先双击 `a010-app/A980控制台.exe` 看老壳还活不活，再决定是反查配方还是重建。"
