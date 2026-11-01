# ShotCraft Engine

当前版本：**0.1.4**。

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [Français](README.fr.md)

## 1. 功能概要

### 来源与具体改动

**ShotCraft Engine 是基于 [mmartinez/shotcraft](https://github.com/mmartinez/shotcraft) 源代码修改、删减而来的衍生项目。** 保留了上游的采集和拼接代码，以及 ShotCraft 贡献者的版权声明。本仓库独立维护，不是上游官方发布版本。

相对于原应用，本项目做了以下改动：

- 将截图组件提取为单个 Rust 库和 CLI；从当前构建中移除 egui 界面、编辑器、OCR、托盘、历史记录、应用设置、上传、MCP 和应用资源。
- 改写滚动帧匹配，使用纹理块与更大范围的像素验证；保留固定头尾、刷新重叠内容，并以最后成功接纳的帧作为拼接基准。改动参考了 [LiteSnap](https://github.com/HuibingLin/LiteSnap) 的算法思想，不包含其产品代码、二进制或资源，详见 [ATTRIBUTION.md](ATTRIBUTION.md)。
- 根据宿主窗口的物理外框和缩放比例选择屏幕，处理混合 DPI 坐标；Windows CLI 初始化 DPI awareness。保留的 Linux X11 pixmap 采集路径来自上游。现存平台限制见下文。
- 提供 JPEG/PNG 转 Base64 接口、Rust 侧 JPEG 质量设置和有序图片列表，便于宿主集成，不创建截图面板、不改剪贴板。
- 新增 Rust 原子停止标记和 CLI 标准输入 `stop` 指令，停止时保留已有截图内容。
- 在滚动拼接外增加单屏截图 `--single` 和 PageDown 逐张截图 `--pages`。
- 将原应用打包改为手动触发的组件编译，覆盖 macOS ARM64/Intel、Windows x64 和 Ubuntu x64，并提供四语言集成文档。

### 截图模式

可被其他 Rust 软件调用的全屏截图组件，同时提供独立命令行程序 `scroll_capture`。

| 模式 | 发送的输入 | 返回结果 |
| --- | --- | --- |
| 默认 / `CaptureMode::Stitched` | 将鼠标移动到目标屏幕中心，向下滚动 | 一张拼接长图 |
| `--single` / `CaptureMode::Single` | 无 | 当前完整屏幕的一张截图 |
| `--pages` / `CaptureMode::Pages` | PageDown | 按采集顺序返回多张完整屏幕截图 |

根据调用方窗口外框选屏，支持 JPEG/PNG 和 Base64，不缩放图片。库不创建窗口、不写文件、不修改剪贴板；CLI 负责保存文件。支持中途停止并保留已有内容。单屏模式已通过真实桌面验证，但不代表所有平台均已验收。

## 2. 详细使用方法

### 调用格式与参数

方括号表示可选项，不要原样输入：

```text
scroll_capture [--single | --pages] <new-output-directory> [x y width height scale_factor]
scroll_capture --help
scroll_capture -h
```

| 参数 | 可用值 | 默认值 / 要求 |
| --- | --- | --- |
| `--single` | 放在输出目录前的开关；与 `--pages` 互斥 | 不提供模式开关时使用滚动拼接 |
| `--pages` | 放在输出目录前的开关；与 `--single` 互斥 | 不提供模式开关时使用滚动拼接 |
| `new-output-directory` | 尚不存在的目录路径，父目录必须存在；路径含空格时加引号 | 必填，不覆盖已有目录 |
| `x`、`y` | `i32` 整数，−2,147,483,648 至 2,147,483,647；窗口外框左上角物理坐标 | `0`、`0` |
| `width`、`height` | `u32` 整数，1 至 4,294,967,295；窗口外框物理宽高 | `100`、`100` |
| `scale_factor` | 有限且大于 0 的 `f64`；调用方窗口自身的缩放比例，用于 macOS 坐标换算 | `1.0` |
| `--help`、`-h` | 单独使用，或紧接 `--single` / `--pages` | 显示帮助，不截图、不创建目录 |

五个几何参数必须按顺序全部提供或全部省略。它们只用于选屏，不用于裁切：优先选择与窗口重叠面积最大的屏幕，没有重叠时选择中心最近的屏幕。省略时使用 `(0, 0, 100, 100, 1.0)`，选择桌面原点附近的屏幕，通常是主屏。CLI 不会自动读取终端或 Pluely14 的窗口位置。目录名以 `-` 开头时，使用 `./--name` 这样的路径。

CLI 固定使用 **JPEG 质量 80** 和 **10 秒准备时间**。没有 `--format`、`--quality`、`--delay` 参数；需要选择编码时使用 Rust 接口。

### 启动示例

编译后，在 macOS / Ubuntu 上运行：

```sh
./target/release/scroll_capture ./capture-01
./target/release/scroll_capture --single ./single-01
./target/release/scroll_capture --pages ./pages-01
```

Windows PowerShell：

```powershell
.\target\release\scroll_capture.exe .\capture-01
.\target\release\scroll_capture.exe --single .\single-01
.\target\release\scroll_capture.exe --pages .\pages-01
```

使用 Cargo 时，程序参数放在 `--` 后。第二条示例指定 Retina 屏幕上的物理窗口外框：

```sh
cargo run --release --locked --bin scroll_capture -- --single ./single-01
./target/release/scroll_capture --pages ./pages-retina-01 200 200 800 600 2
```

每条命令都应使用不同的新目录。准备期间切到目标画面；滚动截图从你希望的位置开始。单屏只截当前画面，不会自动回到页面顶部。

分页模式先截第一张，再每次发送一次 PageDown，等待 350 ms 后继续采集。请自行让目标屏幕上的网页正文获得焦点；此模式不移动鼠标、不点击、不抢占焦点。翻页距离取决于当前应用，不是精确的显示器高度，相邻图片允许重叠。地址栏、编辑器或嵌套滚动区域可能接收到按键。

### 输出与停止

| 模式 | 输出文件 |
| --- | --- |
| 拼接 / 单屏 | `capture.jpg`、`capture.base64` |
| 分页 | `page-0001.jpg`、`page-0001.base64`、`page-0002.jpg`、`page-0002.base64`…… |

每个 `.base64` 文件是对应 JPEG 的标准 Base64，不含 data URI 前缀。完整屏幕截图保留实际采集尺寸，拼接图片保留拼接结果尺寸，不缩放。

向标准输入发送单独一行小写 `stop` 并回车，前后不要有空格，将停止后续滚动并返回已有内容。首帧前停止会报错，不生成图片；单屏已截到的画面会保留。切回终端可能污染截图，宿主可通过管道发送 `stop\n`，避免改变焦点。

文件只在采集函数返回后写入，不逐帧落盘。`Ctrl+C` 或强制结束可能丢失全部未保存结果。普通采集错误通常不保存此前帧；写入失败可能留下部分文件。成功退出码为 0，错误为非零。标准输出打印路径、尺寸和 JPEG 字节数，不直接输出 Base64；不能仅凭目录存在判断成功。

### 读取 CLI 实时进度

从 `0.1.3` 开始，所有截图命令默认向**标准错误 stderr**逐行输出进度，无需新增参数。只有以精确前缀 `PROGRESS ` 开头的行是机器可读进度，去掉前缀后按 JSON 解析。stderr 也可能包含准备提示和错误信息；stdout 仍输出最终文件路径及尺寸。

```text
PROGRESS {"mode":"pages","captured_images":6,"stitched_height":null,"screen_height":1080,"approximate_screens":null,"simulated_percent":48,"status":"capturing"}
PROGRESS {"mode":"stitched","captured_images":null,"stitched_height":2592,"screen_height":1080,"approximate_screens":2.4,"simulated_percent":48,"status":"capturing"}
```

单屏/分页读取 `captured_images`，拼接读取 `approximate_screens`。后者为 `stitched_height / screen_height`，一“屏”指首帧完整截图的高度，包含画面中的应用工具栏和系统界面，不等于文档的一页。`null` 表示该模式不适用，或尚未取得首帧高度。首个事件计数为 0，`screen_height` 为 0。

每条事件都是当前值的快照，直接替换显示值，不要每收到一行就加一。编码和状态变化可能重复相同数字，不变或被拒绝的帧不增加计数。这些数值仅表示本次采集量，不是整页总长度或百分比。建议显示“已采集 6 张”或“已采集约 2.4 屏”。

下面的 Python 3 子进程读取示例仅使用标准库。使用尚不存在的新目录；将 `--pages` 改为 `--single` 可读单屏，删除模式参数则使用拼接。stdout 直接继承，避免最终输出阻塞在无人读取的管道中：

```python
import json
import subprocess
import sys

command = ["./target/release/scroll_capture", "--pages", "./pages-01"]
if sys.platform == "win32":
    command[0] = r".\target\release\scroll_capture.exe"

with subprocess.Popen(command, stderr=subprocess.PIPE, text=True) as child:
    for line in child.stderr:
        if not line.startswith("PROGRESS "):
            print(line, end="", file=sys.stderr)
            continue
        progress = json.loads(line[len("PROGRESS "):])
        if progress["captured_images"] is not None:
            print("Captured images:", progress["captured_images"])
        if progress["approximate_screens"] is not None:
            print(f"Captured approximately {progress['approximate_screens']:.1f} screens")
        print("Simulated processing:", str(progress["simulated_percent"]) + "%")
        print("Capture status:", progress["status"])
    if child.wait() != 0:
        raise SystemExit(child.returncode)
```

其他宿主同样应持续读取 stderr 并按前缀解析 JSON；若同时管道化 stdout 和 stderr，两个都要读取。数值代表已采集/编码的内容，不代表文件已经保存。即使收到成功的采集终态，也要等进程退出码为 0 后，才能确认文件输出成功。

## 3. 被其他软件调用

### Rust 依赖与接口

使用本地源码，按实际位置修改路径：

```toml
[dependencies]
shotcraft-engine = { path = "../shotcraft-engine" }
```

或使用 Git 依赖，私有仓库需要构建环境已配置 Git 访问权限：

```toml
[dependencies]
shotcraft-engine = { git = "https://github.com/teddyxiong/shotcraft-engine.git", branch = "main" }
```

库由 Cargo 编译进宿主，不是现成的 C ABI DLL 或 HTTP 服务。公共入口位于 `shotcraft_engine::scrolling`：

```rust,ignore
pub fn capture_to_base64(
    bounds: WindowBounds,
    format: Option<String>,
    quality: Option<u8>,
    stop: &std::sync::atomic::AtomicBool,
    mode: CaptureMode,
) -> shotcraft_engine::Result<Vec<String>>;

pub fn capture_to_base64_with_progress(
    bounds: WindowBounds,
    format: Option<String>,
    quality: Option<u8>,
    stop: &std::sync::atomic::AtomicBool,
    mode: CaptureMode,
    on_progress: impl FnMut(CaptureProgress),
) -> shotcraft_engine::Result<Vec<String>>;

pub fn encode_image_to_base64(
    image: &image::RgbaImage,
    format: Option<String>,
    quality: Option<u8>,
) -> shotcraft_engine::Result<String>;
```

| 接口参数 | 可用值与行为 |
| --- | --- |
| `bounds: WindowBounds` | 必填字段：`x: i32`、`y: i32`、`width: u32`、`height: u32`、`scale_factor: f64`。单位、合法范围和选屏规则同 CLI；库不提供默认窗口外框 |
| `format: Option<String>` | 仅精确小写 `Some("jpg".into())` 或 `Some("jpeg".into())` 选择 JPEG；`None`、`"png"` 或其他字符串均选择 PNG，区分大小写 |
| `quality: Option<u8>` | 可传入 0–255；`None` 默认 80，JPEG 限制在 1–100。PNG 忽略此值。JPEG 透明区域合成白底 |
| `stop: &AtomicBool` | 必填，初始为 `false`，其他线程或事件处理函数写入 `true` 请求停止 |
| `mode: CaptureMode` | 必填，无默认值：`Single`、`Stitched` 或 `Pages` |
| `image: &image::RgbaImage` | 仅编码接口使用：已有 RGBA 图片，不截图、不发送输入 |

示例：在工作线程截取当前一屏。将示例外框替换为宿主窗口的物理位置、外框尺寸及自身缩放比例，例如 Tauri 的 `outer_position`、`outer_size`、`scale_factor`：

```rust
use shotcraft_engine::scrolling::{CaptureMode, WindowBounds, capture_to_base64};
use std::sync::{Arc, atomic::AtomicBool};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bounds = WindowBounds {
        x: 200, y: 200, width: 800, height: 600, scale_factor: 2.0,
    };
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let task = std::thread::spawn(move || {
        capture_to_base64(
            bounds, Some("jpeg".into()), Some(80), &worker_stop,
            CaptureMode::Single,
        )
    });
    let images = task.join()
        .map_err(|_| std::io::Error::other("capture worker panicked"))??;
    println!("Captured images: {}", images.len());
    Ok(())
}
```

改用 `CaptureMode::Stitched` 返回长图，或 `CaptureMode::Pages` 返回分页列表。`Single` 和 `Stitched` 成功时列表长度为 1；`Pages` 每张有效图片对应一个字符串。成功图片结果至少一张，在函数结束后一次性返回；可选进度回调仅报告数字与状态。

接口同步阻塞，没有 CLI 的 10 秒倒计时。放在工作线程或宿主的 `spawn_blocking` 中调用，不要在 UI 事件线程同步等待 `join`。窗口可见性、焦点、权限及快捷键注册由宿主负责；库没有 Tauri 依赖，也没有内置全局停止快捷键。

将 `Arc<AtomicBool>` 保留在应用状态中，停止命令或快捷键处理函数执行：

```rust,ignore
stop.store(true, std::sync::atomic::Ordering::Relaxed);
```

每次任务新建停止标记，运行中不要复位。停止保留已确认的图片；正在执行的采集、拼接、编码及剩余 350 ms 等待无法强制中断，已经发出的输入不能撤销。单屏跳过该等待、全部输入、连续采集后端及拼接器。

已有图片可以直接编码：

```rust,ignore
let base64 = shotcraft_engine::scrolling::encode_image_to_base64(
    &image, Some("jpeg".into()), Some(80),
)?;
```

Base64 解码后即所选格式的图片字节。接收端支持纯 Base64 时直接传字符串；需要 data URI 时添加 `data:image/jpeg;base64,` 或 `data:image/png;base64,`。分页按顺序分别传入，不要拼接多个 Base64 字符串来组成图片。库不会自动上传至 AI 服务或 Pluely14。

### 在 Rust 中读取实时进度

原 `capture_to_base64` 调用仍可使用，不接收回调。需要实时进度时，调用 `capture_to_base64_with_progress`，在原五个参数后传入回调。在前述示例的工作线程内部使用：

```rust,ignore
use shotcraft_engine::scrolling::{
    CaptureMode, CaptureProgress, capture_to_base64_with_progress,
};

let images = capture_to_base64_with_progress(
    bounds, Some("jpeg".into()), Some(80), &worker_stop, CaptureMode::Pages,
    |progress: CaptureProgress| {
        if let Some(count) = progress.captured_images {
            println!("Captured images: {count}");
        }
        if let Some(screens) = progress.approximate_screens() {
            println!("Captured approximately {screens:.1} screens");
        }
        println!("Simulated processing: {}%", progress.simulated_percent);
        println!("Capture status: {:?}", progress.status);
    },
)?;
```

选择 `CaptureMode::Single` 时，成功后 `captured_images` 为 `Some(1)`；选择 `Pages` 读取已接纳图片张数；选择 `Stitched` 读取 `approximate_screens()`。图片仍在结束后统一以 `Vec<String>` 返回，进度事件不包含图片或 Base64。

| Rust 值 | CLI JSON 值 | 含义 |
| --- | --- | --- |
| `progress.mode` | `mode`：`single`、`pages`、`stitched` | 当前模式 |
| `progress.captured_images` | `captured_images`：整数或 `null` | 单屏/分页已接纳图片张数；拼接为 `None` / `null` |
| `progress.stitched_height` | `stitched_height`：整数或 `null` | 拼接结果当前像素高度；其他模式为 `None` / `null` |
| `progress.screen_height` | `screen_height`：整数 | 首帧截图的像素高度，初始 0，取得后固定 |
| `progress.approximate_screens()` | `approximate_screens`：数值或 `null` | 拼接高度除以非零首帧高度，不可计算时为空 |
| `progress.simulated_percent` | `simulated_percent`：1–100 整数 | 模拟处理进度；结果编码成功后才到 100，不代表文件保存成功 |
| `progress.status` | `status`：下表字符串 | 当前阶段或采集结束原因 |

### 模拟处理百分比

Rust 读取 `progress.simulated_percent`，CLI JSON 读取 `simulated_percent`。这是独立的 **1–100 整数**，仅表示模拟处理进度，不是页面滚动位置、已截页面比例或剩余时间。它不读取滚动条，也不估算页面总高度。回调入口、命令和参数不变，无需新增开关。

初始事件为 1%。每接纳一张图片或实际新增拼接高度，按 `p = min(90, p + max(1, (90 - p) / 8))` 向 90% 推进，除法取整数。例如连续内容事件依次为 12%、21%、29%、36%、42%、48%，随后增速放缓。单屏/拼接进入最终编码时，至少达到 95%；分页每张都要编码，因此逐张编码事件不会直接跳到 95%。

只有全部结果图片编码成功后，成功终态才报告 100%。`Failed` 保持此前小于 100% 的数值。用户停止 `Stopped` 或达到 `FrameLimit`、`HeightLimit` 后，只要部分结果成功产出，也报告 100%：表示本次任务处理完成，不表示整页全部截完，应结合 `status` 区分。单次任务数值单调不降，但仅随事件更新，可能重复或跳变；没有定时器，也不保证逐个显示所有整数。

建议显示“处理中：48%”，与“已采集 6 张／约 2.4 屏”分别展示。CLI 的 100% 仍不代表文件保存成功，要等退出码为 0。宿主可以在已收到的数值之间做动画，但收到成功完成通知前，自己的动画不要达到 100%。

| `CaptureStatus` | CLI `status` | 含义 |
| --- | --- | --- |
| `Capturing` | `capturing` | 初始快照，或新增已确认内容 |
| `Encoding` | `encoding` | 正在编码接纳的图片，分页时可能每张都会出现 |
| `SingleFrame` | `single_frame` | 单屏编码成功 |
| `Stopped` | `stopped` | 用户停止，已有内容编码成功 |
| `Unchanged` | `unchanged` | 连续三帧不变，不保证已到页面底部 |
| `FrameLimit` | `frame_limit` | 达到 50 次采集尝试上限 |
| `HeightLimit` | `height_limit` | 达到拼接高度上限 |
| `Failed` | `failed` | 采集/编码失败，读取函数返回的 `Err` |

后六种为采集终态。成功终态在编码后发出，不代表 CLI 写文件成功。若报告过部分内容后采集失败，返回值仍是错误，不会返回这些图片；之前的数字仅作信息展示，不能当作已保存结果。

回调在截图线程同步执行，不会自动切到 UI 线程。回调应快速、不阻塞、不 panic，只把小型快照转发到宿主的 UI/事件循环；没有固定频率的心跳。同一任务的回调由宿主自己关联，不新增全局进度存储或任务注册表。

### 作为 CLI 子进程调用

随宿主分发对应系统和架构的 CLI 可执行文件及必要许可证。使用上述参数和新目录启动，保留标准输入管道；停止时写入并刷新 `stop\n`。仅关闭标准输入不会停止。等待退出码为 0 后读取 `.jpg` / `.base64`。CLI 是独立可执行文件，直接 Rust 集成则使用前述源码依赖。

## 4. 平台与边界

### 当前行为与限制

- 拼接：最多尝试采集 50 帧或长图高 40,000 像素。连续 3 帧不变时结束，连续 3 帧无法匹配时报错。达到上限返回已有部分，不保证滚到底。每次发送 3 个垂直滚轮单位，再等待 350 ms。
- 拼接模式允许在外围界面变化时，通过严格的正文零位移验证判定画面不变，不再要求整屏逐像素完全一致；判定不变仍不保证已经到达页面底部。
- 分页：最多尝试采集 50 帧；跳过连续像素完全相同的 RGBA 帧，连续 3 帧不变时结束。不做相似度去重，不要求相邻图片匹配，不受长图 40,000 像素高度限制。动画、视频或系统状态变化可能妨碍到底检测，可主动停止或等待次数上限。
- 单屏：只采集一次，不发送输入、不拼接，仍需屏幕采集授权；并不因此代表原生 Wayland 已支持。
- 后续截图或输入报错与停止同时发生时，优先保留已有内容。不创建组件窗口不代表不会截到其他可见内容；滚动模式会改变桌面画面。
- macOS：调用进程需已有屏幕录制权限；滚动模式还需自动输入所需权限。
- Windows：独立 CLI 初始化 DPI awareness，库不改变宿主进程 DPI 设置。拼接鼠标定位使用 Enigo 主屏归一化，副屏滚轮已知不可靠，先在主屏验收。分页向焦点应用发按键，副屏仍需实机验证。
- Ubuntu：当前滚动输入使用 X11，可用 `echo "$XDG_SESSION_TYPE"` 检查。原生 Wayland 滚动尚未适配。使用 Ubuntu 24.04 构建；22.04 容器里的 `--help` 仅验证启动，不验证桌面截图。

Ubuntu 运行库：

```sh
sudo apt-get install -y libpipewire-0.3-0 libxcb1 libgbm1 libegl1 libx11-6 libwayland-client0
```

### 原生 Wayland：适配条件与当前状态

锁定的 xcap 有 Wayland 单次截图路径，但组件没有持续 Portal/PipeWire 采集会话；Enigo 当前使用默认 X11 输入后端。安装开发包、编译成功或截到一张图片，都不能证明原生 Wayland 滚动可用。

| 必要能力 | 用途 |
| --- | --- |
| 真实 Wayland 桌面及用户会话 D-Bus | 与桌面服务通信、进行实际验证 |
| xdg-desktop-portal 及匹配桌面的后端 | GNOME、KDE 等需实现相应接口 |
| ScreenCast Portal 与 PipeWire | 在授权会话中持续获取所选屏幕帧 |
| RemoteDesktop Portal 输入 | 拼接需指针/滚轮，分页需 PageDown 键盘输入，通过支持的接口或 libei/EIS |
| 屏幕与相应输入设备授权 | 屏幕权限不能替代指针或键盘权限 |

适配方向是复用一个授权采集/输入会话，将流的位置、尺寸、缩放对应到目标屏幕，再复用拼接、编码和停止逻辑。仅开启一个 Enigo feature 不够；不能假定合成器坐标就是物理像素，现有窗口重叠选屏方法也需要验证。

组件可以不创建自己的面板，但标准 Portal 通常有系统授权和选屏界面。支持持久化的桌面可用 `persist_mode`、`restore_token` 减少重复提示；令牌需更新，权限撤销、屏幕变化或恢复失败可能再次提示。不能统一承诺首次静默或永久无系统弹窗。

全局快捷键停止还需宿主验证并绑定 GlobalShortcuts Portal。先在 Ubuntu 24.04 + GNOME Wayland 验证持续取帧、输入、拼接和中途停止，再验证 KDE 等桌面，不能只按 Ubuntu 版本判断兼容性。单屏沿用现有截图后端，可用性和授权交互仍依赖桌面环境。

Portal 参考：[ScreenCast](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html) · [RemoteDesktop](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html) · [GlobalShortcuts](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html)

## 5. 本地编译与 GitHub 编译

### 本地准备与编译

安装 **Rust 1.95 或更新版本**及 Cargo，获取源码并检查工具链：

```sh
git clone https://github.com/teddyxiong/shotcraft-engine.git
cd shotcraft-engine
rustc --version
cargo --version
```

使用 rustup 安装时，可通过 `rustup update stable` 更新稳定版。私有仓库需要预先配置访问权限。

| 平台 | 编译要求 |
| --- | --- |
| macOS | Xcode Command Line Tools（`xcode-select --install`）及本机 Rust 工具链 |
| Windows x64 | Visual Studio C++ Build Tools、Windows SDK、`x86_64-pc-windows-msvc` Rust 工具链；使用已配置环境的开发者终端 |
| Ubuntu 24.04 x64 | Rust 及以下开发包 |

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libclang-dev libxcb1-dev libgbm-dev libegl-dev libpipewire-0.3-dev libspa-0.2-dev libwayland-dev
```

在仓库根目录编译 CLI 或检查库：

```sh
cargo build --release --locked --bin scroll_capture
cargo check --lib --locked
```

本机 CLI 输出为 `target/release/scroll_capture`（macOS/Ubuntu）或 `target/release/scroll_capture.exe`（Windows）。其他 Rust 项目依赖此库时，Cargo 自动编译它。显式使用 `--target <triple>` 时，输出位于 `target/<triple>/release/`。

项目检查：

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

### GitHub Actions 手动编译

仅保留 **Engine native builds**，配置见 [engine-build.yml](.github/workflows/engine-build.yml)。只允许 `workflow_dispatch` 触发；推送、PR、标签和定时任务均不会启动编译。

1. 打开 [Actions](https://github.com/teddyxiong/shotcraft-engine/actions/workflows/engine-build.yml)。
2. 点击 **Run workflow**，选择包含目标改动的分支，再点击 **Run workflow**。
3. 等待四个任务完成，从运行页面下载匹配系统的 Artifact，保留期为 14 天。

| 平台 | 目标 triple | Artifact |
| --- | --- | --- |
| macOS ARM64 | `aarch64-apple-darwin` | `scroll-capture-macos-arm64` |
| macOS Intel | `x86_64-apple-darwin` | `scroll-capture-macos-x64` |
| Windows x64 | `x86_64-pc-windows-msvc` | `scroll-capture-windows-x64` |
| Ubuntu x64 | `x86_64-unknown-linux-gnu` | `scroll-capture-ubuntu-x64` |

任务执行格式检查、Clippy、测试、release 构建及 `--help`；Ubuntu 另有 22.04 启动检查。Artifact 包含可执行文件、英文 README、许可证、来源说明、提交和工具链信息。其他语言文档在仓库中查看。

解压后在该目录运行 `./scroll_capture --single ./single-01`；Windows 使用 `.\scroll_capture.exe --single .\single-01`。macOS/Ubuntu 必要时先运行 `chmod +x ./scroll_capture`。按第 2 节参数测试滚动，每次换新目录；在真实桌面检查完整采集和停止效果，CI 编译及帮助检查不能证明桌面行为。

## 6. 源码与许可

### 许可证与使用权限

本次修订随附的、维护者有权许可的原创贡献与修改，采用自定义 [ShotCraft Engine Source-Available License 1.0](LICENSE)。当前修订的整个组件不再统一按 MIT/Apache 提供。英文 `LICENSE` 为正式文本，本节为说明性摘要。

| 使用场景 | 授权要求 |
| --- | --- |
| 个人、企业或组织发布的软件，不向用户收取访问、使用或功能费用 | 按 `LICENSE` 免费使用 |
| 广告、志愿赞助或自愿捐赠支持的免费软件 | 免费，但付费不能成为访问、使用或解锁功能的条件 |
| 企业或组织自己的内部使用，包括业务运营 | 按 `LICENSE` 免费使用 |
| 向用户收取购买、订阅、许可、付费档位、功能解锁或托管软件访问费用的软件 | 必须另行取得书面付费商业授权，即使截图功能本身免费 |

该规则适用于库集成和调用集成的 CLI，也适用于修改后的版本。此自定义许可不要求公开宿主应用源码或私有修改。商业授权通过[本仓库 Issue](https://github.com/teddyxiong/shotcraft-engine/issues) 联系维护者，价格和范围另行书面约定；仅咨询或捐赠不等于已获授权。

上游 ShotCraft 部分保留 **MIT OR Apache-2.0** 及版权，依赖保留各自许可。自定义条款只覆盖维护者有权许可的内容，不能撤销独立取得的上游或既有授权。特别是 **`v0.1.1`（`114184a9a7a37928c818b1aa8b804bcf02c869a7`）及更早版本保留原许可**。已经获得有效旧版宽松授权的人，可以继续依据该授权使用相应旧代码，新条款不会强迫已有被许可人付费。

这属于带收费软件限制的**源码可见（source-available）许可**，不是 OSI 认可的开源许可。`v0.1.1` 标签没有移动；仅凭包版本 `0.1.1` 无法判断 main 后续提交的许可，应固定并核对完整提交号。本次不创建新发布标签。

发布修改或衍生版本时，欢迎通过 Issue 告知维护者。通知为自愿请求，不是额外许可条件，也不能代替所需的付费授权。

| 文件 | 职责 |
| --- | --- |
| `src/lib.rs` | 库导出和结构化错误 |
| `src/capture.rs` | 屏幕采集，包括 Linux X11 pixmap 路径 |
| `src/stitch.rs` | 基于像素的滚动帧拼接 |
| `src/scrolling.rs` | 选屏、模式、停止与 Base64 编码 |
| `src/bin/scroll_capture.rs` | CLI 参数和文件输出 |

上游署名与许可文本保留在 [LICENSE-MIT](LICENSE-MIT)、[LICENSE-APACHE](LICENSE-APACHE)、[NOTICE](NOTICE) 和 [ATTRIBUTION.md](ATTRIBUTION.md)，依赖许可见生成的 [THIRD-PARTY-LICENSES.md](THIRD-PARTY-LICENSES.md)。分发组件或集成产品时携带 [LICENSE](LICENSE) 及所有适用的上游和依赖声明。本文件不改变原始 ShotCraft 代码的许可。

原应用的 UI、OCR、编辑器、托盘及资源已移出当前组件。被忽略的本地 `backups/` 仅供恢复，不包含在 Git 分发或构建中。
