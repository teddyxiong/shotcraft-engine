# 滚动截图三端测试

本轮仅验证独立截图库及其测试程序。不接入 Pluely14，不增加权限申请流程，
不发布安装包或 Release。测试程序直接调用 `shotcraft_engine::scrolling::capture_to_base64`，
没有截图面板、预览或编辑器。测试程序自己的终端窗口仍由操作者切换到后台。

## 构建方案与产物

手动运行 Actions 的 [Engine native builds](https://github.com/teddyxiong/shotcraft-engine/actions/workflows/engine-build.yml)
工作流。四个原生 job 分别构建：

| 下载的 Artifact | 系统 | 程序 |
| --- | --- | --- |
| scroll-capture-macos-arm64 | Apple Silicon macOS | scroll_capture |
| scroll-capture-macos-x64 | Intel macOS | scroll_capture |
| scroll-capture-windows-x64 | Windows x64 | scroll_capture.exe |
| scroll-capture-ubuntu-x64 | Ubuntu x64，先验证 X11 桌面 | scroll_capture |

每项运行现有 doc/engine 测试和 clippy，编译 release 测试程序并执行 `--help`。
Artifact 包含程序、许可、提交 SHA、编译器版本及本文。没有签名、公证或自动安装。
编译或 `--help` 成功不能替代真实桌面截图、滚动、拼接和停止验收。

沿用上游的 Ubuntu 24.04 构建环境，因为其发布流程已注明 22.04 的 PipeWire
开发头文件太旧。额外在 Ubuntu 22.04 容器安装运行库并执行 `--help`，验证启动兼容；
这一步没有桌面，不能证明 Ubuntu 22.04 上的真实截图和滚动已通过。
原生 Wayland 不在本轮已支持范围：当前 Enigo 默认使用 X11 后端。

## macOS 启动

解压对应芯片的 Artifact，在解压目录打开终端，执行：

```sh
chmod +x ./scroll_capture
./scroll_capture ./capture-mac-01
```

## Windows 启动

解压 Windows Artifact，在解压目录打开 PowerShell，执行：

```powershell
.\scroll_capture.exe .\capture-win-01
```

## Ubuntu 启动

先使用 X11 桌面会话，可通过 `echo "$XDG_SESSION_TYPE"` 确认输出为 `x11`。
解压后在终端执行：

```sh
sudo apt-get update
sudo apt-get install -y libpipewire-0.3-0 libxcb1 libgbm1 libegl1 libx11-6
chmod +x ./scroll_capture
./scroll_capture ./capture-ubuntu-01
```

以上 apt 命令仅安装运行库。若需本机编译，构建包列表在工作流中；不要把在
22.04 上运行 Artifact 与在 22.04 上从源码构建混为一谈。

## 统一操作与验收

1. 先打开同一个有文字和图片的长网页，回到页面顶部，并关闭旧的截图窗口。
2. 启动程序后有 10 秒准备时间，切到浏览器；不要最小化或关闭正在运行程序的终端进程。
3. 默认选取桌面原点附近的屏幕，自动向下滚动。最多 50 帧或 40,000 像素高；
   画面连续 3 次完全不变也会结束。失败会输出错误及非零退出码。
4. 完成后，输出目录中生成 `capture.jpg` 和纯 Base64 的 `capture.base64`。
   JPEG 质量为 80；终端显示最终宽高和文件大小。目录必须是新目录，避免覆盖原有结果。
5. 查看图片：正文顺序应连续，没有明显重复/缺行，没有 ShotCraft 面板或预览混入。
6. 再运行一次，用另一个新目录。采集到中途时，在启动程序的终端输入 `stop` 并回车。
   应停止后续滚动、正常输出已有部分；不能使用 Ctrl+C，后者结束进程，无法保证保存结果。
   切换终端可能改变当前画面，因此停止测试后也应检查最后一段是否混入终端内容。

收到 `stop` 时，正在执行的截图/拼接和最多剩余 350 ms 等待会先结束，然后编码返回。
若首帧开始前就收到 `stop`，程序返回“首帧前停止”错误，此时没有图片。
当前不绑定全局快捷键；Pluely14 的快捷键接入留到后续。

## 多屏与缩放测试

可指定用于选屏的模拟客户端窗口外框：

```text
scroll_capture <新输出目录> <x> <y> <width> <height> <scale_factor>
```

矩形为物理像素坐标；macOS 必须提供模拟窗口所在屏幕的真实缩放比例。
例如 Retina 主屏上的窗口可用 `./scroll_capture ./capture-retina-01 200 200 800 600 2`。
在副屏提供位于该副屏内的矩形，确认滚动和采集发生在同一块目标屏幕；同时验证一次
主副屏缩放不同的情况。本工具不运行 Pluely14，因此这里测试的是模拟窗口坐标选屏。

## 权限与失败记录

本轮不设计或增加权限申请。测试依赖机器当前已授予的系统权限；权限不足时保留错误日志，
不要把它记成拼接算法失败或三端功能通过。macOS 的屏幕录制和自动滚动权限属于调用进程，
原生桌面环境的限制不能靠 Base64 或停止信号消除。

记录：系统版本、芯片/架构、X11/Wayland、屏幕数量与缩放、Artifact 的 commit.txt、
启动命令、终端错误、最终宽高，以及正常结束/中途停止两张图的结果。

## 本地构建

需要 Rust 1.95 或更高版本及 README 中列出的各平台编译依赖。

```sh
cargo build -p shotcraft-engine --release --example scroll_capture --locked
```

macOS/Ubuntu 程序位于 `target/release/examples/scroll_capture`；Windows 位于
`target/release/examples/scroll_capture.exe`。优先测试 Actions 下载产物以核对相同提交。
