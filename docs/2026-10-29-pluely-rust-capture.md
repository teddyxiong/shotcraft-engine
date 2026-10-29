# pluely14 Rust 滚动截图调用

## 本轮范围与处理约定

在已验证的拼接器上提供无窗口的 Rust 调用，先对齐图片处理，再封装采集。
不改 pluely14 的前端、AI 提交、远程控制协议或普通单张截图行为。

已读取 pluely14 的 `src-tauri/src/capture.rs`、`src/hooks/useCompletion.ts` 和
`src/pages/screenshot/components/ScreenshotConfigs.tsx`：

- PNG 默认；`jpg` / `jpeg` 使用 JPEG，其余格式回退 PNG。
- JPEG 质量默认 80，限制为 1–100；透明像素按原函数的浮点计算和四舍五入合成白底。
- 保留原图尺寸，不缩放、分片，不添加 data URI 前缀，返回标准 Base64。
- Auto / Manual 是截图后的提交模式，继续由客户端决定提交 AI 或加入附件。
- 全屏选屏沿用窗口最大重叠面积、无重叠时最近屏幕中心的规则；输入明确为
  Tauri 的物理窗口坐标及窗口自身的 `scale_factor`，macOS 内部先还原为统一的
  逻辑坐标再与显示器比较，避免不同缩放比例的屏幕物理矩形互相重叠而选错屏。

## 最小实现方案

复用现有 `shotcraft-engine`，增加 `scrolling` 模块，继续使用唯一的
`shotcraft_doc::stitch::Stitcher`。不搬入 egui，不创建新 crate、进程协议或另一套拼接器。
相比独立抽取新 crate，这样改动更少，但调用方也会引入 engine 现有依赖（含 OCR）。
相比分进程 CLI，Rust 调用直接返回结果和错误，不需要临时文件或标准输出协议。

两个入口：已有长图直接编码；按窗口位置执行全屏滚动截图后编码。
采集函数阻塞，应在 Tauri `spawn_blocking` 中执行。最多采集 50 帧、高度 40,000；
每次滚动 3 个单位、等待 350 ms。连续 3 帧完全不变时结束；连续 3 次无法拼接时返回错误。
达到帧数或高度上限会返回已采集部分，不能等同于任意长页面都已到达底部。

该库不创建控制面板、预览、边框或保存文件，不操作剪贴板。
调用方继续负责已有客户端窗口的可见性和内容保护；自动滚动仍会改变页面和鼠标位置，
无窗口不代表桌面内容完全静止。使用调用进程的屏幕录制和输入控制权限。

## 执行与验收

1. 新增 engine 编码入口，复用已存在的 image / base64 编码器；用一个 PNG 往返测试锁定原尺寸及原像素。
2. 新增选屏及滚动调用入口，复用现有 capture 和 Stitcher；仅调整 engine 的现有依赖声明。
3. 用上一阶段真实长图，从外部 Rust 程序调用编码入口，核对 PNG / JPEG 解码尺寸。
4. 编译真实 Tauri Rust 适配代码，运行相关测试、clippy、格式及 diff 检查。
5. 核查最终 diff；本轮未重新实测的界面、权限及多屏行为明确列出，不将编译当作实机验收。

## Rust 调用

在 pluely14 的 `src-tauri/Cargo.toml` 添加本地依赖（相对当前两个项目的位置）：

```toml
shotcraft-engine = { path = "../../shotcraft-lab/upstream/crates/engine" }
```

使用本项目最低 Rust 1.95 或更高版本；调用方可以继续使用 edition 2021。
本轮没有写入 pluely14 仓库。以下适配代码可放入其 `src-tauri/src/capture.rs`，
在已有 Builder 上增加 `.manage(capture::ScrollCaptureState::default())`，并将
`capture::capture_scrolling_to_base64` 和 `capture::stop_scrolling_capture`
添加到原有 `generate_handler!` 列表。

```rust
use shotcraft_engine::scrolling::{self, WindowBounds};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex, PoisonError};

#[derive(Default)]
pub struct ScrollCaptureState {
    active: Mutex<Option<Arc<AtomicBool>>>,
}

#[tauri::command]
pub fn stop_scrolling_capture(state: tauri::State<'_, ScrollCaptureState>) -> bool {
    let active = state.active.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(stop) = active.as_ref() {
        stop.store(true, Ordering::Relaxed);
        true
    } else {
        false
    }
}

/// Tauri command adapter, placed in the client's capture module.
#[tauri::command]
pub async fn capture_scrolling_to_base64(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, ScrollCaptureState>,
    format: Option<String>,
    quality: Option<u8>,
) -> Result<String, String> {
    let position = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let scale_factor = window.scale_factor().map_err(|e| e.to_string())?;
    let bounds = WindowBounds {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        scale_factor,
    };
    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut active = state.active.lock().unwrap_or_else(PoisonError::into_inner);
        if active.is_some() {
            return Err("A scrolling capture is already running".into());
        }
        *active = Some(stop.clone());
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        scrolling::capture_to_base64(bounds, format, quality, &stop).map_err(|e| e.to_string())
    })
    .await;
    *state.active.lock().unwrap_or_else(PoisonError::into_inner) = None;
    result.map_err(|e| format!("Scrolling capture task failed: {e}"))?
}
```

如果已有拼接完成的 `image::RgbaImage`，直接处理它，无需再次采集屏幕：

```rust
let base64 = scrolling::encode_image_to_base64(&long_image, format, quality)?;
```

返回值和普通截图一样是 `String`；原有 Auto / Manual 提交链路可继续使用。

## 中途停止

Rust 采集函数的第四个参数为 `&AtomicBool`，由调用方为每次采集新建，并通过
`Arc` 同时保留在控制侧和采集线程。控制侧调用 `stop.store(true, Ordering::Relaxed)`
即发出停止指令，不需要结束线程或进程。

上方 Tauri 示例提供具体指令 `stop_scrolling_capture`：有正在执行的采集时设置停止标记并
返回 `true`，没有任务时返回 `false`。原来的开始调用会在收尾后返回已有图片的 Base64。
状态只保存当前一个任务的标记；运行期间拒绝再次开始，避免停止信号落到错误任务。

- 已有帧时，停止后正常拼接收尾、编码，返回当前已采集部分。
- 首帧采集之前已经停止，返回 `scrolling capture stopped before first frame`，没有图片。
- 检查点位于下一帧采集前、当前帧拼接后，以及发送下一次滚动前。
- 无法撤销已经发给系统的滚动事件，也不强行打断正在执行的截屏或拼接。
  当前 350 ms 等待会自然结束再检查，因此响应时间包括剩余等待或正在执行的操作；
  JPEG/Base64 编码结束后，开始调用才返回结果。
- 停止与当前采集/滚动报错同时发生时，优先返回此前已经提交的图片；未收到停止信号时，
  原有采集和滚动错误照常返回。
- 不新增控制窗口、快捷键、进程间指令服务或 Pluely14 前端。命令示例已经给出，
  但仍需要在 Pluely14 中注册后才能从客户端发出。
- 支持由 Pluely14 的全局快捷键触发同一停止指令，不依赖截图窗口获取焦点。
  当前库没有绑定具体按键。

## 本轮验证结果

- `cargo test -p shotcraft-engine --locked --offline`：19 个测试通过，doc-tests 通过。
  图片接口阶段新增 PNG 原尺寸/原像素往返与 macOS 混合缩放坐标两个测试。
  本次停止功能仅新增两个测试：收到停止后不再采集并返回已有帧；停止与下一帧报错同时发生仍返回已有帧。
- engine 的 all-target clippy、构建、workspace 格式检查及 diff 空白检查通过。
- 外部 edition 2021 crate 依赖本地 engine 和与客户端相同版本的 Tauri 2.10.3，
  编译上面的开始/停止适配函数、状态及 `generate_handler!` 注册成功；不启动窗口。
  工程位于 `/tmp/shotcraft-pluely-rust-check`，没有改客户端 Cargo.lock 或源码。
- 外部 Rust 程序调用公开编码函数，处理上一阶段实际长图
  `/tmp/shotcraft-scroll.KHrdOp/stitched.png`：PNG 与 JPEG 解码后均为 `3072×13626`；
  PNG 全部像素与原图一致。PNG 8,591,501 字节，JPEG 质量 80 为 3,618,046 字节。
  编码结果在 `/tmp/shotcraft-pluely-rust-check/encoded.png` 和 `encoded.jpeg`。
- 审查中的 macOS 混合缩放、Linux DPI 及定点距离溢出问题已修正并复核。
  本轮没有进行 Windows/Linux 或真实多显示器滚动验收。
- 单独编译了调用正式库函数的无窗口命令行程序；零宽窗口参数在采集前返回错误。
  公开入口预先收到停止标记时，无需访问显示器或输入控制器即可返回首帧前停止错误，
  已通过外部 Rust 程序验证。
  人工实测脚本为 `bash /tmp/shotcraft-pluely-rust-check/run-capture.sh`，针对当前已知
  `3072×1920 @2x` 主屏，等待 10 秒切换页面，输出 PNG 及纯 Base64。
  此脚本本轮尚未由用户运行；不能把上一阶段临时采集程序的实测当成本轮正式入口实测。
- 没有引入新依赖版本；仅 engine 增加现有 workspace base64，并将既有 enigo 依赖用于 macOS。
  锁文件只增加 engine 的 base64 引用。三端构建提交前已在临时目录安装 cargo-about 0.9.2，
  按项目要求重新生成第三方许可清单，清单内容没有变化。
