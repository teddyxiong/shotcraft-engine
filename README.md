# ShotCraft Engine

一个可被其他 Rust 软件直接调用的全屏滚动截屏组件，同时提供命令行程序 `scroll_capture`。
保留选屏、连续采集、向下拼接、JPEG/PNG 编码、Base64 输出和中途停止。
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
use shotcraft_engine::scrolling::{WindowBounds, capture_to_base64};
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
        capture_to_base64(bounds, Some("jpeg".into()), Some(80), &worker_stop)
    });

    // 将 stop 保留在应用状态中，供其他事件处理函数发送停止信号。
    // 此命令行示例只等待结果；UI 软件不要在事件线程上同步 join。
    let base64 = task.join()
        .map_err(|_| std::io::Error::other("capture worker panicked"))??;
    println!("JPEG Base64 length: {}", base64.len());
    Ok(())
}
```

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
`png` 或其他格式参数（包括 `None`）使用 PNG。返回值均为纯 Base64 字符串。
不缩放图片，完成拼接后才进行一次编码。

## 平台与当前边界

- 最多采集 50 帧或高 40,000 像素；连续 3 帧完全不变也会结束。
  达到上限返回已采集部分，不保证任意长度页面都到达底部。
- 连续 3 帧无法可靠拼接时返回错误。停止与当前采集/滚动报错同时发生时优先返回已有部分。
- 自动滚动会移动鼠标并改变页面；无截图窗口不代表桌面画面静止。
- macOS：依赖调用进程已有的屏幕录制及自动滚动所需权限。
- Windows：独立 CLI 初始化 DPI awareness；库不改变宿主的进程级 DPI 设置。
  当前 Enigo 后端按主屏归一化鼠标绝对坐标，副屏自动滚动已知不可靠，先在主屏验收。
- Ubuntu：当前自动滚动使用 X11 后端，原生 Wayland 尚未适配。
  在 X11 桌面运行可用 `echo "$XDG_SESSION_TYPE"` 确认会话。
- Ubuntu 24.04 用于构建。工作流额外在 22.04 容器运行 `--help` 验证启动兼容；
  容器启动成功不能代替真实桌面截图测试。运行库：

```sh
sudo apt-get install -y libpipewire-0.3-0 libxcb1 libgbm1 libegl1 libx11-6 libwayland-client0
```

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
