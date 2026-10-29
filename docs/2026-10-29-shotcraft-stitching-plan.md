# 滚动截图拼接 Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task in the current session.

**Goal:** 参考 LiteSnap 修复当前滚动截图拼接，使用真实帧验证完整宽度和接缝。
**Architecture:** 先在原路径增加可选诊断，再依据真实输入修复匹配核心；沿用现有输出及接口。
**Tech Stack:** Rust、现有 image crate，不新增依赖。
**Spec:** [已确认设计](2026-10-29-shotcraft-stitching-design.md)。

**当前结论：本机当前 Chrome/GitHub 页面上的拼接阶段已验收通过。** 最终证据为 `/tmp/shotcraft-scroll.KHrdOp/{scroll.log,stitched.png}`；正式后台接口、自动选屏、Base64 和 pluely14 接入不在本阶段交付范围。以下实验章节保留诊断过程，最终验收以文末记录为准。

## Global Constraints

- 主要修改 `crates/doc/src/stitch.rs`、`crates/ui-egui/src/scrolling.rs` 及本轮文档。失败的隐藏补丁已撤回，`platform.rs` 恢复原状；采集隔离使用临时目录中的无窗口诊断程序，不增加正式后台接口。
- 不变更 `Push` 和现有调用接口，不搬入第三方产品代码；保留用户已有日志。
- 用户后续已授权代理编译及从命令行启动实测；操作者配合准备页面。不重置权限。iTerm 桌面控制因签名校验拒绝，当前由操作者运行已准备脚本。
- 不提交或推送。测试仅服务本次行为，最多增加一个主路径及一个关键失败路径测试。

## Review Focus

- 图片为空或尺寸变化：记录明确原因，继续拒绝无效帧。
- 首次匹配失败：日志包含低于门槛的最佳候选，不能仅记录成功候选。
- 诊断图片：默认不保存，启用时只保存两帧输入；Done 时另存最终 `stitched.png` 用于验收。不覆盖已有文件。
- 固定侧栏、空白背景和局部变化：真实帧证明位移，避免误拼。
- 匹配失败后的基准：保留最后成功拼接帧，不能丢掉未提交的滚动距离。

## Task 1: 获取失败证据

- [x] 在 `stitch.rs` 增加每次 push 的诊断信息，不改匹配决策；最佳候选包括未达 0.85 门槛的结果。
- [x] 在 `scrolling.rs` 使用 `SHOTCRAFT_SCROLL_DEBUG_DIR` 可选诊断目录；记录帧序号、显示器和尺寸，只保存首个成功和首个失败帧，错误写 stderr。
- [x] 运行 `cargo test -p shotcraft-doc stitch --locked --offline` 和相关格式、编译检查。
- [x] 提供操作者编译和实测命令，核对真实输入、最佳候选及拒绝原因。

## Task 2: 修复并验收

- [x] 根据 Task 1 的真实证据确定唯一当前根因；若输入错误先修输入。
- [x] 若证据指向匹配限制，在现有测试模块写一个主路径回归测试，确认旧行为失败。
- [x] 在 `stitch.rs` 局部实现所需的变化列、特征位移及像素差验证，保留完整宽度输出和现有固定头尾、高度限制。
- [x] 运行相关测试和检查；回放相同真实帧并检查位移、输出高度和接缝。
- [x] 操作者完成 Chrome/GitHub 全屏滚动验收；最终长图及日志已经检查，正文没有明显缺行、重复或接缝遮挡。

## 诊断阶段的检查

- `cargo test -p shotcraft-doc stitch --locked --offline`：5 passed。
- `cargo clippy -p shotcraft-ui-egui --all-targets --locked --offline -- -D warnings`：通过，包含 doc、engine 及 UI 的编译检查。
- `cargo fmt --all -- --check`、`git diff --check`：通过。
- 临时固定侧栏实验仍为 `NoMatch`，诊断报告 `score below threshold`、0 个匹配行；对照实验仍正确识别 37 像素位移，63/63 行匹配。这符合“本步不改匹配策略”的约束。

## 真实失败输入已确认（2026-10-29）

- 代理执行 `cargo build --release --locked --offline` 成功，用时 2m21s；操作者在 iTerm 运行 `/tmp/shotcraft-stitch-investigation/run-capture.sh` 并反馈未拼接。
- 输入位于 `/tmp/shotcraft-scroll.WpBpnb`，两张图片均为正确的 Chrome/GitHub 全屏，尺寸 `3072×1920`。日志目标显示器逻辑尺寸 `1536×960`、scale=2，截图尺寸一致。本次不是捕获了终端或尺寸不一致。
- 结果为首帧 `Appended(1920)` 一次、随后 `NoMatch` 165 次。
- 首次失败评分：best_dy=128，matched=14，compared=1778，score=0.007874016，远低于旧门槛 0.85。
- 独立像素分析得到真实位移 240；正文区域 x=320..2100、y=430..1470 的对应像素有约 74.93% 完全一致，RGB 平均绝对差仅约 0.823/255。但同范围行扩展到完整宽度后，1040 行中没有一整行完全相同。
- 因此当前根因是全宽精确行哈希将固定边缘、悬停/像素变化等混入匹配，拒绝了有充分正文重叠的帧。不是缺少重叠，也不能靠降低滚动速度解释本次首次失败。
- Dock 在这两帧中发生变化，仅同位置稳定行检测会把底栏估计为 39 行。平移支持分析显示正文末端为 y=1740、真实底栏 180 行；修复需在位移确定后检查底部哪些行确实参与平移，避免把 Dock 作为新增正文。
- 修改前的本地 Rust 回放重现 `NoMatch`（exit 1）；新增唯一主路径测试 `stitches_with_fixed_sidebar_and_small_body_color_changes` 在旧实现返回 `NoMatch`，预期 `Appended(37)`（exit 101）。
- 原始截图仅留在临时目录，未提交。首次成功帧 SHA-256：`1d847c289784dc07659a6d78467f99c7b2503567ddfee5652432c35dd16e6cb0`；首次失败帧：`027483ef7054f45e428ccbbc42827d46b447858b55353962eba51307c9f0c63e`。

## 修复后的检查

- 核心由整行精确哈希替换为变化列、纹理 patch 位移提议、广域文字/边缘差异验证。移除旧行哈希缓存，沿用原有拼接缓冲区及公开调用接口。
- 匹配失败仍不更新已提交的基准帧。输出追加完整宽度；固定底栏在首次成功位移后确定，并在后续追加中保持。
- 新 `MatchDiagnostic.score` 是 `1 - 归一化像素差`，门槛 0.90，不再是旧版的精确行匹配比例，不能将两者数值直接比较。
- 新增 1 个固定侧栏/头尾/颜色扰动主路径测试，以及独立复核实际发现的 1 个关键失败路径：纯色固定底栏在小位移时自匹配，被错误推断为 0。两者均先失败再修复通过；底栏推断不再小于已有稳定底栏证据。
- `cargo test -p shotcraft-doc --locked --offline`：36 passed，包含 7 个拼接测试。
- `cargo clippy -p shotcraft-ui-egui --all-targets --locked --offline -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check`：通过。
- 真实两帧的 Rust 离线回放成功：`Appended(240)`、footer=180、height=2160、score=0.9987398；`3072×2160` 输出与独立确定的源行逐像素一致，图片已实际查看。产物 `/tmp/shotcraft-scroll.WpBpnb/stitched-first-pair.png`。
- 临时真实画面悬停检查：只将 3230 个链接文字像素改蓝、无位移，返回 `NoMatch`，高度保持 1920，没有误追加。
- 修复后的 `cargo build --release --locked --offline` 成功，用时 2m35s；新的 `target/release/shotcraft` 已生成，可进行下一轮连续采集。
- 尚未证明修改后在连续实时采集中每次位移都可靠；等待下一轮操作者实测，不标记整个目标完成。

## 连续采集首次复测：拼接增长，但尚未验收通过

- 输入及长图目录 `/tmp/shotcraft-scroll.AMNTvm`，操作者已选择 Done，保存了 `stitched.png`（`3072×9120`）。
- 首帧之后成功追加 22 次：240 行 17 次、480 行 4 次、1200 行 1 次；另有 38 次 `NoMatch`。从首帧无法增长的原始故障中已取得实测进展，但不能据此宣布完整可靠。
- 最终长图中反复出现本程序控制面板，接缝处有横线。原始首次截图未出现面板，面板在启动后进入画面；这已证明原版全屏时自身 UI 会污染截图。
- 保留旧基准使部分 `NoMatch` 后可一次补回 480 或 1200 行；末尾仍有连续匹配失败，未证明页面尾部完整。
- 下一步不降低匹配门槛：仅在 macOS 的诊断自动开始运行中，将本进程窗口临时隐藏一次，诊断运行不创建边框。操作者用 Cmd+Tab 切回截图程序，再按 Enter/Done 完成。普通非诊断运行保持原有可见面板。
- `platform.rs` 使用现有安全 objc2 绑定调用本进程的 `NSApplication::hide`，不操作其他应用。不增加新的开始/停止参数、IPC 或客户端接口。
- 已检查本机为 macOS 26.7.1；[Apple 当前文档](https://developer.apple.com/documentation/appkit/nswindow/sharingtype-swift.enum)将 `NSWindow.SharingType.none` 标为系统不再使用的遗留常量，因此未保留该窗口共享标记方案，不依赖它让操作者盲测。
- 这项最小改动的格式与相关 UI clippy 编译检查已通过；未新增测试或依赖。此前的 1 个主路径和 1 个关键失败测试是本轮新增测试的全部。
- 临时隐藏版本 `cargo build --release --locked --offline` 已成功，用时 2m11s；`target/release/shotcraft` 已更新。平台临时隐藏的真实效果及消除干扰后的连续匹配仍待操作者复测。

## 隐藏复测失败及已确认的启动时序

- 操作者再次实测，目录 `/tmp/shotcraft-scroll.I7sGEM`，输出 `3072×8880`；面板仍反复进入长图，98 次 `NoMatch`，未通过。
- 日志确实包含隐藏请求，但它发生在第 3 次采集后，说明采集已经在窗口准备完成前开始。
- 核对实际锁定依赖 `eframe 0.36.2`：`native/epi_integration.rs::post_rendering` 在首帧结束时无条件调用 `window.set_visible(true)`。原实现于首帧 `Session::ui` 内隐藏，随后被框架重新显示，解释了“请求已执行但后续仍拍到面板”。
- 修正仍使用同一隐藏机制：诊断自动开始时先令 `Shared.running=false`；待 `ctx.cumulative_frame_nr()>0` 后请求隐藏一次，读取 `NSApplication::isHidden()`，仅为 true 时启动采集。隐藏失败时保持暂停并显示明确错误，不重试或叠加窗口策略。
- 新日志记录 `hidden` 与 GUI 帧号，可核对其是否先于 `frame=1`。普通非诊断运行及拼接门槛均未改动。格式及相关 UI clippy 编译检查通过，实际隐藏和长图验收仍待复测。

## 隐藏状态返回 false：撤回窗口补丁，改用临时命令行验证

- 操作者报告 `Could not hide the scrolling capture window; capture was not started`、0 px；目录 `/tmp/shotcraft-scroll.SUlMmT` 的日志只有显示器元数据和 `hidden=false gui_frame=1`。本次采集未启动，无法提供拼接验收证据。
- 停止叠加窗口隐藏措施：删除 `hide_on_start`、AppKit 隐藏帮助函数及隐藏失败门槛，恢复原有 `running=start`；`platform.rs` 与本次任务开始前一致。拼接算法及既有诊断保存逻辑未改动。格式和相关 UI clippy 编译检查通过。
- 在已有临时目录创建 `/tmp/shotcraft-stitch-investigation/capture-probe.rs`，直接调用本项目编译好的 `shotcraft_engine::capture`、`doc::stitch::Stitcher` 及相同的 Enigo 滚轮调用。不复制匹配算法，不初始化 egui/eframe，不创建窗口，不新增仓库 example、依赖或测试设施。
- 临时程序固定最多采集 50 帧，帧间滚轮量 3、等待 350ms，满高 40,000 时提前结束；最终直接保存 `stitched.png`。这是拼接验证工具，不是对正式产品开始/停止接口的实现。
- 首次失败时保存实际对应的最近成功基准 `baseline.png` 及 `first-failure.png` 两张输入。前次实验的失败发生在多次追加后，初始帧已不是匹配基准，因此这次改为保存直接对应的帧对，便于离线复现。
- 临时程序通过 rustc 直接链接现有 release 库并编译成功；无参数启动按预期在截图前返回“缺少诊断目录”错误。尚未运行屏幕采集，实际权限及长图仍需操作者在 iTerm 复测。
- `run-capture.sh` 现指向该临时程序，每次新建目录，延迟 10 秒，约 20 秒自动完成。运行前先关闭之前的截图窗口并将 Chrome 页面回到顶部；采集中不需要切回截图程序。
- 撤回隐藏门槛后的正式程序 `cargo build --release --locked --offline` 已成功，用时 2m23s。临时诊断程序也已单独编译成功；下一轮使用临时程序以保证采集期间没有本程序面板。

## 无窗口实测：位移匹配通过，继续修复接缝残留

- 操作者运行临时程序后，目录 `/tmp/shotcraft-scroll.Xb1yIF` 保存了 `3072×13626` 长图，已实际检查到页面 License 末尾及 Dock。截图中没有本程序面板。
- 50 帧全部接受：首帧 1920 行、48 次追加 240 行、最后追加 186 行，`NoMatch=0`，高度算术一致。匹配门槛未因隐藏实验而放宽。
- 逐段检查及 49 个接缝位置分析发现旧窗口底部阴影、圆角及浏览器悬停 URL 提示残留；提示条遮住正文，因此仍不将整张长图视为验收通过。
- 参考 LiteSnap 的重叠更新思路，成功匹配后刷新重叠正文，再追加新增行。失败及 Unchanged 不修改已提交基准。这样后帧能补回此前被视口边框、状态条挡住的正文。
- 独立复核发现直接使用当帧 `header` 刷新不安全：固定页头变色会令检测值归零，造成页头被复制到正文。已改为使用所选精确位移特征票中的最早已验证正文坐标 `prev_y-dy`，限制刷新起点，不新增边缘推断或阈值。
- 本轮仍只有两个新增测试：主路径同时核对全宽、灰色底边消除、正文像素坐标及允许的 +1 颜色差；关键失败路径核对小位移下动态页头和纯色底栏。均经历失败后修复通过，7 个拼接测试及 36 个 doc 测试全通过，相关 UI clippy、格式和 diff 检查通过。
- 原动态页头 dy37 临时复现程序按最新源码重编译后，输出逐像素等于正确源图。原真实两帧回放仍恢复 dy240、footer180、`3072×2160`；逐行核对输出只能来自两帧对应的正确空间坐标，并检查旧底边已由后帧可见正文替换。新产物 `/tmp/shotcraft-scroll.WpBpnb/stitched-first-pair-refreshed.png`。
- 正式程序已重新编译成功（2m38s），临时诊断程序已重新链接更新后的 release 库。新诊断日志首行包含 `build=feature-verified-overlap-refresh`，可区分刷新之前的静态程序；该版本已在下一节完成整页验收。

## 最终验收：当前拼接阶段通过

- 操作者完成新版命令行采集并提供目录 `/tmp/shotcraft-scroll.KHrdOp`。日志含 `build=feature-verified-overlap-refresh`，确认使用重叠刷新后的版本。
- 显示器截图为 `3072×1920`，全宽保留。50 帧全部成功：首帧 1920 行、48 次 240 行、最后 186 行；`NoMatch=0`。输出高度 `1920 + 48×240 + 186 = 13626`，最终图片 `3072×13626`。
- 实际分段查看覆盖整张长图，从仓库文件列表、README、两张大图、功能介绍、下载及快速开始到 License 末尾。正文未见明显缺行、重复或断裂，原先遮住 Screenshot-Based Coding Help 的悬停提示已消失。
- 对前版 49 个接缝旧边框位置作像素比较，49 处均已由后帧替换；实际查看确认重复阴影、圆角和悬停状态条未继续残留在正文中。本程序面板未进入截图。
- 全屏图仍保留浏览器和系统 UI，右侧滚动条可见分段位置痕迹；本阶段不做额外 UI 图像修饰，不将正文验收扩大成所有固定 UI 的通用分割能力。
- 最终图 SHA-256：`9b4e71f48d3d2dc10e5515912b565260af869801b2b2baacfbee996b96dd1f40`。日志 SHA-256：`d5439298b5ec5924cee19bcbdbb4a096e41bf026e3ef27a7057cb4ea3e1372ed`。截图仅在本地临时目录，不提交。
- 当前源码已经通过 36 个 doc 测试（含 7 个拼接测试）、相关 UI clippy、格式检查及 release 编译。新增测试始终只有 1 个主路径和 1 个关键失败路径，没有新增依赖或测试设施。未提交、推送或改动正式客户端。
- 此结论限定本机 macOS 26.7.1 的当前 Chrome/GitHub 页面。无窗口实测使用临时 Rust 诊断程序直接调用项目库；正式 CLI 的后台开始/停止、目标显示器自动选择及 Base64 交付尚未实现，不能由本次验收推导为完成。

## 操作者采集真实失败帧

以下命令由操作者运行。先编译；进入第二段命令后有 5 秒切换至 Chrome/GitHub 的稳定长页面。显示器及尺寸沿用交接中的本机参数；诊断日志会记录实际截图尺寸。

```sh
cd /Users/teddy/Documents/ddd/shotcraft-lab/upstream
cargo build --release --locked
```

```sh
shotcraft_scroll_debug_dir=$(mktemp -d /tmp/shotcraft-scroll.XXXXXX)
printf '诊断目录：%s\n' "$shotcraft_scroll_debug_dir"
(
  sleep 5
  SHOTCRAFT_SCROLL_DEBUG_DIR="$shotcraft_scroll_debug_dir" \
    ./target/release/shotcraft scrolling-capture \
      --display 1 --x 0 --y 0 --width 3072 --height 1920 \
      --start --autoscroll
) 2> "$shotcraft_scroll_debug_dir/scroll.log"
```

上面的正式程序命令仍会启动面板，适合一般诊断；当前为排除面板干扰，应使用下面的临时 `run-capture.sh`。普通面板选择 Enter/Done 时可保存最终 `stitched.png`。
Ctrl+C 结束会保留已保存的输入帧和日志，但不经过最终拼接图片的保存路径。
将打印的本地目录路径交给开发者，便可读取 `initial.png`、`first-failure.png` 和 `scroll.log`，不需要上传图片。
若日志出现 `save failed` 或未生成失败帧，保留该目录及日志，不把没有失败帧当成拼接已通过。

代理已准备的 `/tmp/shotcraft-stitch-investigation/run-capture.sh` 每次执行都会新建诊断目录，给出 10 秒切换页面，并设置临时 `SHOTCRAFT_HOME`。当前脚本运行无窗口临时诊断程序，固定 50 帧后自动保存退出。每次运行的目录路径追加记录在同目录的 `runs.txt`，不覆盖上一轮证据。
