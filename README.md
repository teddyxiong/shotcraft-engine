# ShotCraft Engine

一个可被其他 Rust 软件直接调用的全屏滚动截屏组件，同时提供命令行程序 `scroll_capture`。
保留选屏、连续采集、向下拼接、逐页截图、JPEG/PNG 编码、Base64 输出和中途停止。
截图核心不创建窗口、不写文件、不操作剪贴板；命令行程序负责将结果保存到指定目录。

## 编译

需要 Rust 1.95 或更新版本。

- macOS：安装 Xcode Command Line Tools。
- Windows x64：使用 Rust MSVC 工具链，并安装 Visual Studio C++ Build Tools。
- Ubuntu：使用 24.04 构建，安装以下开发包：

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libclang-dev libxcb1-dev libgbm-dev libegl-dev libpipewire-0.3-dev libspa-0.2-dev libwayland-dev
```

在仓库根目录编译：

```sh
cargo build --release --locked
```

生成的命令行程序：

- macOS / Ubuntu：`target/release/scroll_capture`
- Windows：`target/release/scroll_capture.exe`

只检查 Rust 库可使用 `cargo check --lib --locked`。日常检查：

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

## 命令行启动

macOS / Ubuntu：

```sh
./target/release/scroll_capture ./capture-01
```

Windows PowerShell：

```powershell
.\target\release\scroll_capture.exe .\capture-01
```

也可直接通过 Cargo 启动：

```sh
cargo run --release --locked --bin scroll_capture -- ./capture-01
```

输出目录必须尚不存在。启动后有 10 秒准备时间，请切到浏览器长页面顶部。
默认选取桌面原点附近的屏幕，自动向下滚动，结束后输出：

- `capture.jpg`：JPEG，质量 80，保持拼接结果的像素尺寸。
- `capture.base64`：对应 JPEG 的标准 Base64，不含 data URI 前缀。

中途在启动终端输入 `stop` 并回车，会停止后续采集和滚动，保存已经拼接的部分。
`Ctrl+C` 会终止进程，无法保证保存结果。首次采集前就收到 `stop` 时返回错误，不生成图片。
切回终端可能改变目标画面，停止测试时也应检查图片末尾是否混入终端内容。

指定用于选屏的模拟客户端窗口外框：

```text
scroll_capture <新输出目录> <x> <y> <width> <height> <scale_factor>
```

坐标和尺寸使用物理像素；macOS 的 `scale_factor` 使用调用方窗口自身的缩放比例。
例如 Retina 主屏：

```sh
./target/release/scroll_capture ./capture-retina-01 200 200 800 600 2
```

`scroll_capture --help` 仅显示用法，不开始采集。

## 按 PageDown 逐张保存

增加前置参数 `--pages`，切换为分页模式：

```sh
./target/release/scroll_capture --pages ./pages-01
```

Windows PowerShell：

```powershell
.\target\release\scroll_capture.exe --pages .\pages-01
```

也可指定目标屏幕对应的模拟窗口外框，例如：

```sh
./target/release/scroll_capture --pages ./pages-retina-01 200 200 800 600 2
```

程序读取所选屏幕的实际截图尺寸，先截取第一屏，随后每次发送一次 PageDown，等待 350 ms 后再采集。
每张图片保留该次完整屏幕的像素尺寸，不缩放、不拼接。翻页距离由应用的 PageDown 行为决定，
不以显示器像素高度直接换算滚轮次数；相邻图片允许有重叠。

10 秒准备时间内，请让目标网页正文获得键盘焦点，并确保它位于所选屏幕。
分页模式只发送 PageDown，不移动鼠标、不点击或抢占焦点；地址栏、编辑框或嵌套滚动区域
持有焦点时，PageDown 可能不会翻动目标页面。应用滚动动画及异步加载需在真实桌面测试。
切回终端手动停止也可能将终端截为下一张；客户端可通过停止标记结束采集，保持目标页面的焦点。

采集正常结束或收到 `stop` 后，CLI 按采集顺序分别保存：

```text
page-0001.jpg
page-0001.base64
page-0002.jpg
page-0002.base64
...
```

JPEG 质量仍为 80，Base64 对应各自的 JPEG；未带 `--pages` 时仍输出 `capture.jpg` 和 `capture.base64`。
库在过程中保留已编码的图片列表，CLI 在函数返回后写文件，因此强行结束进程不能保证保存已有页面。

分页模式最多尝试采集 50 帧，跳过像素完全相同的连续帧，连续 3 帧完全不变时结束；
每张独立图片不受拼接长图 40,000 像素高度限制。视频、动画或系统状态变化可能使画面始终不同，
此时可主动 `stop` 或等待达到次数上限；当前不做相似度去重。

## 被其他 Rust 软件调用

在调用方的 `Cargo.toml` 添加依赖。本地开发示例：

```toml
[dependencies]
shotcraft-engine = { path = "../shotcraft-engine" }
```

也可从此仓库引用；私有仓库需要构建环境已有正常的 Git 访问权限：

```toml
[dependencies]
shotcraft-engine = { git = "https://github.com/teddyxiong/shotcraft-engine.git", branch = "main" }
```

主要接口位于 `shotcraft_engine::scrolling`：

```rust
use shotcraft_engine::scrolling::{CaptureMode, WindowBounds, capture_to_base64};
use std::sync::{Arc, atomic::AtomicBool};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 实际应用中，从自己的客户端窗口读取物理外框和窗口缩放比例。
    let bounds = WindowBounds {
        x: 200,
        y: 200,
        width: 800,
        height: 600,
        scale_factor: 2.0,
    };
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let task = std::thread::spawn(move || {
        capture_to_base64(bounds, Some("jpeg".into()), Some(80), &worker_stop, CaptureMode::Stitched)
    });

    // 将 stop 保留在应用状态中，供其他事件处理函数发送停止信号。
    // 此命令行示例只等待结果；UI 软件不要在事件线程上同步 join。
    let images = task.join()
        .map_err(|_| std::io::Error::other("capture worker panicked"))??;
    println!("Captured images: {}", images.len());
    Ok(())
}
```

采集入口新增最后一个 `CaptureMode` 参数，返回值统一为 `Result<Vec<String>>`：

- `CaptureMode::Stitched`：返回一张拼接长图的 Base64，列表长度为 1。
- `CaptureMode::Pages`：返回按采集顺序排列的独立页面 Base64 列表，不进行拼接。

调用分页模式时，将上例的 `CaptureMode::Stitched` 改为 `CaptureMode::Pages`。
调用方按列表逐张使用图片；已有图片的 `encode_image_to_base64` 接口仍返回单个 `String`。

控制侧收到停止指令或快捷键后，在其事件处理函数中执行：

```rust
stop.store(true, std::sync::atomic::Ordering::Relaxed);
```

停止信号应为每次采集新建，不在同一任务运行期间复位。已有帧时正常返回部分图像；
尚无首帧时返回停止错误。无法撤销已经发出的系统事件，也不会强行中断正在执行的截图、
拼接或剩余 350 ms 等待；收尾编码完成后才返回 Base64。

采集函数是同步阻塞函数。Tauri 等异步客户端应放在 `spawn_blocking` 中调用，
同时在客户端自己的状态中保留停止标记；本库没有 Tauri 依赖。
调用方继续负责自己的窗口可见性、全局快捷键和系统权限。

如果已有 `image::RgbaImage`，可直接编码，不重新截屏：

```rust
let base64 = shotcraft_engine::scrolling::encode_image_to_base64(
    &image,
    Some("jpeg".into()),
    Some(80),
)?;
```

`jpg` / `jpeg` 使用 JPEG，质量默认 80、限制为 1–100，透明像素合成白底。
`png` 或其他格式参数（包括 `None`）使用 PNG。每张图片均编码为不带前缀的 Base64 字符串。
不缩放图片；拼接模式完成拼接后编码一次，分页模式对每张完整截图分别编码。

## 只截第一屏：待实现方案

当前公开接口和 CLI 尚未提供单屏模式。现有流程已经先截取第一帧，再初始化鼠标控制器并发送滚轮事件，
因此可以用一个小开关扩展，不需要另写一套截图实现。

当前模式参数已包含 `Stitched`、`Pages`。后续单屏能力可在同一参数中增加 `Single`，
无需再增加独立的 `scroll_enabled` 开关：完成选屏和首帧采集、校验后，直接编码并返回长度为 1 的列表。
在创建连续采集后端和输入控制器之前返回，跳过输入事件、帧间等待及拼接循环。

CLI 拟增加 `--single`，默认仍执行滚动截图。**该参数尚未实现，当前不能使用。**
单屏结果沿用现有输出格式、质量、原始尺寸及文件保存规则。

单屏模式不通过预置 `stop=true` 实现：当前首帧前收到停止信号会返回错误，
靠启动后立即发送停止指令也无法可靠保证只采集一帧。新增模式仍应保留原来的停止语义。
验收时需确认只采集一次、不创建鼠标控制器、不移动鼠标或滚动，输出尺寸与首帧一致。

单屏模式仍需要屏幕采集授权，但无需执行自动滚动所需的输入控制。
它本身不会补齐下文的原生 Wayland 适配。

## 平台与当前边界

- 拼接模式最多采集 50 帧或高 40,000 像素；连续 3 帧完全不变也会结束。
  达到上限返回已采集部分，不保证任意长度页面都到达底部。
- 拼接模式连续 3 帧无法可靠拼接时返回错误；分页模式不要求相邻图片能匹配。
  停止与当前采集/翻页报错同时发生时优先返回已有结果。
- 拼接模式会移动鼠标并发送滚轮事件；分页模式发送 PageDown。两种模式均会改变页面，
  无截图窗口不代表桌面画面静止。
- macOS：依赖调用进程已有的屏幕录制及自动滚动所需权限。
- Windows：独立 CLI 初始化 DPI awareness；库不改变宿主的进程级 DPI 设置。
  拼接模式使用的 Enigo 绝对鼠标定位按主屏归一化，副屏滚轮操作已知不可靠，先在主屏验收。
  分页模式只发送键盘事件，需确保焦点和目标屏幕一致；副屏效果仍需实机验收。
- Ubuntu：当前自动滚动使用 X11 后端，原生 Wayland 尚未适配。
  在 X11 桌面运行可用 `echo "$XDG_SESSION_TYPE"` 确认会话。
- Ubuntu 24.04 用于构建。工作流额外在 22.04 容器运行 `--help` 验证启动兼容；
  容器启动成功不能代替真实桌面截图测试。运行库：

```sh
sudo apt-get install -y libpipewire-0.3-0 libxcb1 libgbm1 libegl1 libx11-6 libwayland-client0
```

## 原生 Wayland：适配条件与当前状态

本项目尚未完成原生 Wayland 的滚动截图适配。当前锁定的 xcap 有 Wayland 单次截图路径，
但本项目没有维护持续的 Portal/PipeWire 采集会话；Enigo 当前仅启用默认 X11 输入后端。
安装 Wayland 开发包、编译成功或单次截图成功，都不能证明原生 Wayland 自动滚动已可用。

适配需要分别满足屏幕采集与输入控制条件：

| 条件 | 用途 |
| --- | --- |
| 实际 Wayland 桌面会话、可用的用户会话 D-Bus | 与桌面服务通信并在真实环境验证 |
| xdg-desktop-portal 及匹配桌面的后端 | 由 GNOME、KDE 等桌面实际提供所需接口 |
| ScreenCast Portal 与 PipeWire | 在授权会话中持续读取所选屏幕的帧 |
| RemoteDesktop Portal 的输入控制能力 | 拼接模式需指针/滚轮输入，分页模式需键盘 PageDown；通过受支持的接口或 libei/EIS 路径发送 |
| 用户授予屏幕及所用输入设备的访问权限 | 指针和键盘权限需按模式确认；只获屏幕授权不能代替输入授权 |

最小适配方向是建立并复用一次授权会话，持续取帧并发送滚动事件，再接入现有拼接、编码和停止逻辑。
需验证所选输入后端与桌面提供的接口相匹配；只启用一个 Enigo feature 不等于整条流程已适配。
屏幕流的位置、尺寸与缩放还需对应到目标显示器，不能直接假定合成器坐标就是物理像素，
也不能保证原来的窗口外框重叠选屏方法在原生 Wayland 上直接成立。

组件可以继续不创建截图面板或预览，但标准 Portal 流程通常会显示系统授权、选屏界面。
支持持久化的桌面可通过 `persist_mode`、`restore_token` 减少重复授权；令牌需要按规范更新，
权限撤销、屏幕变化或会话恢复失败时仍可能重新提示。因此不能统一承诺首次也无弹窗，或永久无系统提示。

若需切到浏览器后用全局快捷键停止，还要确认桌面支持 GlobalShortcuts Portal 并由宿主绑定；
现有停止标记可以复用。建议先以 Ubuntu 24.04 + GNOME Wayland 验证持续取帧、自动滚动、拼接和中途停止，
再分别验证 KDE 等环境，不能只按 Ubuntu 版本判定支持。

参考：[ScreenCast](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)、
[RemoteDesktop](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html)、
[GlobalShortcuts](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html)。

## GitHub Actions 手动编译

仓库只保留 **Engine native builds**，触发入口仅为 `workflow_dispatch`。
提交、推送、PR、标签和定时任务均不会启动编译。

打开 [Actions](https://github.com/teddyxiong/shotcraft-engine/actions/workflows/engine-build.yml)，
选择 **Run workflow**，选择分支后再次点击 **Run workflow**。
四个原生目标分别输出 `scroll-capture-macos-arm64`、`scroll-capture-macos-x64`、
`scroll-capture-windows-x64`、`scroll-capture-ubuntu-x64` Artifact，保留 14 天。

解压对应 Artifact 后，在解压目录使用 `./scroll_capture ./capture-01`，
Windows 使用 `.\scroll_capture.exe .\capture-01`；macOS/Ubuntu 必要时先 `chmod +x ./scroll_capture`。
每次测试换一个新目录，分别检查完整采集与中途 `stop` 的图片是否连续、无明显缺行或重复。
CI 中的编译、单元测试和 `--help` 不代表真实桌面验收已通过。

## 源码与许可

源码集中在 `src/capture.rs`、`src/stitch.rs`、`src/scrolling.rs` 和 `src/bin/scroll_capture.rs`。
原应用的 UI、OCR、编辑器、托盘、字体资源和历史文档已移出构建，留在被 Git 忽略的本地 `backups/`。

采用 MIT OR Apache-2.0，保留上游 ShotCraft 版权与许可声明。
来源说明见 `ATTRIBUTION.md`，依赖许可见生成的 `THIRD-PARTY-LICENSES.md`。
