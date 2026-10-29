# ShotCraft 滚动截图调研与实验交接文档

- 文档日期：2026-10-29。
- 实验日期：2026-10-28；2026-10-29 重新读取源码、日志及工作区状态完成交接。
- 时区：America/Los_Angeles；本次记录为 PDT（UTC−07:00）。以下文件时间均采用该时区。
- 实验仓库：`/Users/teddy/Documents/ddd/shotcraft-lab/upstream`。
- 正式客户端：`/Users/teddy/Documents/ddd/pluely14`。本次调研未向正式客户端接入 ShotCraft。
- 上游：<https://github.com/mmartinez/shotcraft>。
- 本次核对的固定提交：`4f4ddeb62708b5d622f5e5106177528e9f0c7640`。
- 当前状态：**原版在本机 Chrome/GitHub 实测中未能拼接；未确认根因，暂停正式集成。**

## 1. 交接时最重要的结论

1. ShotCraft 确有 Rust 实现的屏幕采集、自动滚轮和长图拼接，源码可按 MIT 许可复用。但它仍是候选，不能写成“已验证适用的成熟组件”。
2. CLI 的 `scrolling-capture` 仍启动 egui 控制面板及边框；`--start` 只表示立即采集，不代表无界面。原版没有满足本需求的后台开始/停止及图片交付接口。
3. 用户已完成自动滚动和手动极慢滚动实验。两者均仅接受第一帧，其余帧没有成功追加。手动慢滚失败说明“自动滚动太快”不是充分解释。
4. 自动实验日志：`Appended(1920)` 1 次、`NoMatch` 128 次。手动实验日志：`Appended(920)` 1 次、`Unchanged` 2 次、`NoMatch` 66 次。
5. `NoMatch` 后拼接器保留最后一张成功接受的帧，不更新匹配基准。这里所有后续帧都未成功，所以持续与第一帧比较，页面继续滚动后更难恢复。这是已确认的代码行为，但不是首次匹配失败的已确认原因。
6. 下一步应取得“首次接受帧 + 首次失败帧”和匹配评分，不应继续盲调分辨率、步长或阈值。

## 2. 需求与范围演变

最初讨论过远程多张截图，再转向生成一张长图复用现有单图 AI 链路。当前需要评估的目标流程是：

```text
手机发送开始命令
  → 客户端后台启动
  → 截取目标显示器全屏、自动滚动、持续拼接
  → 不显示控制面板、边框或预览窗口
手机发送停止命令
  → 停止滚动，补采最后一帧并完成拼接
  → 图片编码为 Base64，释放不再需要的像素缓冲区
  → 携带现有 Screenshot autoPrompt，进入现有 AI 请求
```

用户不希望手动框选区域。实验中的 `3072×920` 是为了让原版面板可操作而采用的诊断尺寸，不是对最终“全屏”需求的修改。

后续用户明确提出不希望新增 macOS 辅助功能授权，并探讨“手机滑动指挥电脑滚动”。这构成尚待选择的产品约束：

- 使用 ShotCraft/Enigo 的模拟滚轮路径，无论指令来自自动计时还是手机，macOS 均需要相应辅助功能权限。
- 在不新增此权限、也不采用其他已授权控制渠道的范围内，可行的最小替代是手机开始/停止、用户用电脑实体鼠标或触控板滚动。
- “任意其他软件可由手机远程滚动”和“不申请控制输入权限”不能在现有 Enigo 路径下同时承诺。

曾提出的约 3 秒画面不变、60 秒总时长、到底自动发送等均为讨论中的建议初值，**没有实现，也没有经过用户最终确认或实测**。最终是否正常到底自动发送、异常结果保留多久，仍需在可行性通过后确定。

## 3. 源码核对与可复用边界

以下相对路径均从本文所在 `docs` 目录链接至实验仓库。

| 位置 | 已核实内容 | 对本需求的影响 |
|---|---|---|
| [CLI 入口](../apps/shotcraft/src/main.rs) | `scrolling-capture` 接收 x/y/width/height/display/start/autoscroll；四个区域参数齐全时直接进入 `scrolling::run`，否则进入选择界面 | 原版不能省略宽高后自动视为全屏；没有后台 stop 子命令或长图 stdout 输出 |
| [窗口与滚动会话](../crates/ui-egui/src/scrolling.rs) | `run` 在约第 75 行；创建控制面板，UI 创建四周边框；`beside` 在约第 34 行，无空间时允许面板与区域重叠 | 原版不是 headless，尤其全屏时存在自有窗口进入采集范围的风险 |
| [采集循环](../crates/ui-egui/src/scrolling.rs) | `capture_loop` 约第 106 行；每轮重新采集显示器并裁剪，然后调用 `Stitcher::push` | 不是设计上重复使用启动时的截图；但实际采集内容仍需原始帧确认 |
| 同上 | 自动模式将鼠标移到区域中心，调用 `scroll(3, Vertical)`，等待约 350 ms；手动模式等待约 110 ms | 不是页面结构识别，滚动量不是固定 3 像素；周期还包含截图/处理时间 |
| 同上 | 连续 3 次 `Unchanged` 或达到高度上限时暂停；`NoMatch` 不显示错误 | 原版可能滚动但不增长，也不会按“连续匹配失败”结束 |
| 同上 | 预览取 `tail(1400)`；捕获高度来自 `Stitcher::height` | 不能仅凭预览形状判断是否拼接；本次日志高度未增长，已排除单纯预览误判 |
| 同上 | `finish` 约第 171 行：设置 stop、join 工作线程、调用拼接器 finish | 没有显式保证停止滚动后再补采最后一帧；后台版本需单独处理 |
| [拼接器](../crates/doc/src/stitch.rs) | 独立文件，生产代码约 160 行，只直接引用 `image::RgbaImage` | 最小复用单位，可与现有截图函数分开；无需引入整个 UI |
| [采集后端](../crates/engine/src/capture.rs) | 显示器选择与连续采集；macOS 区分逻辑坐标、实际图片像素 | 不能直接把 Retina 所有宽高除以二；需记录实际图片尺寸与坐标换算 |
| [MCP](../apps/shotcraft/src/mcp.rs) | 现有工具可单次截图，但无滚动截图会话开始/停止工具 | 不能直接以现有 MCP 满足后台控制 |

### 3.1 拼接器具体限制

- 每帧按整行 RGBA 字节计算哈希；有效行的哈希相同才算该行匹配。
- 忽略部分纯色行，搜索垂直正向位移；至少比较 6 个有效行，匹配比例门槛 `MIN_SCORE = 0.85`。
- 只实现向下追加，没有通用向上扩展或横向长图逻辑。
- 尝试识别固定页头、页脚，但不是任意固定侧栏或动态页面的通用分割算法。
- `NoMatch` 可能来自空图、尺寸不一致或未找到合格位移。当前日志没有细分这些分支。
- 只有成功接受帧才更新 `last`；`NoMatch` 不会更新基准。
- 原版会话设置最大高度 40,000 像素，但高度限制不是内存字节上限。宽 2,000 像素、40,000 行的单个 RGBA 缓冲区约 320 MB，尚未包括其他帧和编码副本。
- 已阅读普通拼接、固定头尾、无关帧拒绝、高度限制等源码测试。**本次调研没有执行这些单元测试，不能记为已通过。**

### 3.2 技术栈、许可及平台

- [workspace Cargo.toml](../Cargo.toml)：版本 `0.1.0-beta.1`、Rust edition 2024、最低 Rust 1.95、`publish = false`。本机调研时 `rustc --version` 为 1.98.1。
- 当前实验锁文件使用 `xcap 0.9.8`。本机已读取该版本的 macOS 源码：`capture_image` 调用 `CGWindowListCreateImage`，并非将所有后台兼容性问题自动解决。
- 正式客户端当时为 Tauri 2、Rust edition 2021，已有 `image 0.25.6` 与 `xcap 0.0.12`。不能直接将新版 xcap 的行为当成客户端旧版行为。
- [LICENSE-MIT](../LICENSE-MIT) 与 [Cargo.toml](../Cargo.toml) 确认 `MIT OR Apache-2.0`，可选择 MIT，复用时保留版权及许可声明。
- 完整 `shotcraft-doc` crate 还包含模型、绘制、字体等；完整 engine 还包含 OCR、历史、上传等。本需求无需因此引入这些模块。
- [README 平台说明](../README.md#platform-notes) 明确 Linux 仅测试过 X11（含特定配置下 XWayland）；不能把“支持 Linux”扩大为已验证 Ubuntu 原生 Wayland。
- 本次用户实测仅 macOS。Windows、Ubuntu、跨平台编译和打包均未实测。

## 4. 日期明确的实验记录

时间不足以精确确认的早期实验只标日期，不编造开始/结束时间。日志 mtime 是文件最后修改时间，不等于实验准确起止时间。

### T1：2026-10-28，iTerm 作为初始测试内容

用户从终端启动原版，在 iTerm 制造大量输出进行测试。用户报告能滚动，但没有观察到拼接；之后指出目标已切换到 Chrome，预览仍像 iTerm。

当时没有对应原始帧或详细拼接日志。讨论过的可能性包括终端初始已在底部、向上翻历史不受支持、首帧 iTerm 与后续 Chrome 无重叠、权限或显示器问题。**这些均不是已证实根因。**

### T2：2026-10-28，全屏分辨率及面板可操作性

用户执行 `system_profiler SPDisplaysDataType`，报告：

```text
Resolution: 3072 x 1920 Retina
```

指定 `--x 0 --y 0 --width 3072 --height 1920` 时，用户报告控制面板按钮位于屏幕下方，无法正常操作。将高度改为 920 后可以操作、可以滚动，但高度始终是 920，不增长。

用户随后明确说明：在 Chrome 的长 GitHub 页面从顶部向下滚，已不再使用 iTerm 内容。

结论边界：面板布局/定位存在实测可用性问题，但没有记录实际 viewport 坐标，不能断言一定是 Retina 缩放错误。920 高度只缓解面板操作，不证明拼接成功。

### T3：2026-10-28，延迟 5 秒启动

提出并由用户执行 `sleep 5` 后启动，用于提前切换到 Chrome，避免立即采集到 iTerm。用户确认 Chrome 自动滚动，但仍不拼接。

结论：该操作没有解决问题。“首帧是 iTerm”仍是某些早期运行的可能机制，不能作为整个故障的定论。延迟后的首次原始输入帧没有保存。

### T4：2026-10-28 11:14—11:16，增加日志并重新编译

用户在 `crates/ui-egui/src/scrolling.rs` 的 `stitcher.push(frame)` 后加入：

```rust
let pushed = stitcher.push(frame);
eprintln!("[scroll] result={pushed:?}, height={}", stitcher.height());
```

2026-10-29 交接复核时，该文件修改时间为 `2026-10-28 11:14:17 -0700`，`target/release/shotcraft` 为 `2026-10-28 11:16:37 -0700`。工作区只有上述一条日志及空行的源码修改，没有算法变更。

曾发现旧可执行文件早于日志源码修改。代理尝试执行 `cargo build --release --locked --offline`，在等待构建锁时用户说明自己正在编译；代理随即中断自己的等待进程，退出码 130。**不能把代理这次尝试记录为成功编译。**用户随后生成了包含诊断行的运行日志，证明其运行版本包含诊断输出。

### T5：2026-10-28，自动滚动全屏实验，有日志

用户报告执行命令如下。`./shotcraft` 是用户实际运行位置下的文件名；完整路径及是否为复制件未被记录，不能假定与 `target/release/shotcraft` 是同一路径。

```sh
(
  sleep 5
  ./shotcraft scrolling-capture \
    --display 1 \
    --x 0 --y 0 \
    --width 3072 --height 1920 \
    --start --autoscroll
) 2> /tmp/shotcraft-scroll.log
```

用户报告：Chrome 自动滚动，捕获高度始终 1920。日志最后修改时间：`2026-10-28 11:19:31 -0700`（精确时间见附录）。

日志统计：129 条诊断行；`Appended(1920)` 1 次，`NoMatch` 128 次，没有 `Unchanged`。

```text
[scroll] result=Appended(1920), height=1920
[scroll] result=NoMatch, height=1920
... 后续均为 NoMatch，总计 128 次 ...
```

已证明第一帧被接受，后续没有任何成功追加。没有采集错误日志，但仅凭当前记录不能证明后续图片内容、尺寸或重叠正确。

### T6：2026-10-28，手动极慢滚动实验，有日志

为检验“自动滚动过快”，提供不带 `--start --autoscroll` 的命令。用户确认执行，并手动非常缓慢向下滚动 Chrome/GitHub 页面：

```sh
./shotcraft scrolling-capture \
  --display 1 \
  --x 0 --y 0 \
  --width 3072 --height 920 \
  2> /tmp/shotcraft-scroll-manual.log
```

按实验步骤，切换到 Chrome 后点击 Start，保持 Auto-scroll 关闭。用户报告仍未拼接，并提供一张页面/边框截图。该图不是本次采集器保存的“首帧与失败帧”配对，不能用于计算实际重叠或评分。

日志最后修改时间：`2026-10-28 11:22:20 -0700`。69 条诊断行：`Appended(920)` 1 次、`Unchanged` 2 次、`NoMatch` 66 次。

```text
[scroll] result=Appended(920), height=920
[scroll] result=Unchanged, height=920
[scroll] result=Unchanged, height=920
[scroll] result=NoMatch, height=920
... 后续均为 NoMatch，总计 66 次 ...
```

结论：首次接受后没有增长；慢速手动操作也失败。“单纯降低自动滚动速度即可解决”未得到支持。与 T5 相比还改变了区域高度和启动方式，因此不是严格的单变量速度对照，不能据此排除速度在所有场景中的影响。

## 5. 已证实、未证实及此前判断修正

| 项目 | 证据状态 | 当前判定 |
|---|---|---|
| 首帧采集及入拼接器 | 两份日志均有 Appended | 已证实 |
| 持续成功拼接 | 两份日志均无后续 Appended | 本机两次留有日志的实验均失败 |
| 只是预览未显示完整长图 | 捕获高度始终不增长 | 不能解释本次故障 |
| 模拟滚轮可驱动 Chrome | 用户观察与报告 | 本机可以；不证明跨平台通用，也不证明无需权限 |
| 后续一定抓错了 iTerm | 没有保存原始输入帧；预览只更新成功帧 | 尚不能确认 |
| 启动首帧错误是唯一根因 | 延迟启动后仍失败 | 不能作为结论 |
| 滚动太快是唯一根因 | 手动极慢仍失败 | 不能作为结论 |
| 自有面板进入全屏图片 | 源码允许重叠；没有配对原始帧 | 具体运行尚待验证 |
| Retina 坐标转换错误 | 有面板定位症状，无实际坐标记录 | 待验证，不应直接除二“修复” |
| 权限造成错误/受限画面 | 系统机制及旧 API 路径相关，但没有实际权限状态证据 | 待验证，不能宣称权限就是根因 |
| 严格行哈希受固定侧栏、动态元素、子像素渲染影响 | 从算法可推导出风险，无原始帧评分证据 | 候选解释，不是已复现根因 |
| NoMatch 后一直与第一张成功帧比较 | 源码 + 后续无 Appended 的日志 | 失败恢复受限的确定机制，不能替代首次失败诊断 |

早期将 ShotCraft 描述为“适合作为基础组件”，指技术结构与许可证适合评估，并不构成运行可靠性验证。交接时应采用更严格结论：**可复用候选，实测未通过，暂不进入产品接入。**

## 6. 下一位开发者的最小工作顺序

### 第一步：只补失败诊断，不改匹配策略

仅在实验副本进行，保留用户已有日志修改。建议优先观察这两个文件：

1. `crates/ui-egui/src/scrolling.rs`：记录帧序号、原始截图尺寸、裁剪后尺寸、目标显示器 ID/几何及结果。只保存第一张接受帧和首次 NoMatch 输入帧；若首次 NoMatch 是过渡帧，再按证据决定是否需要增加一张，不连续保存整个屏幕过程。
2. `crates/doc/src/stitch.rs`：记录失败原因（空图/尺寸/无位移）、固定头尾行数、最佳候选 dy、有效比较行数、匹配行数及分数。低于阈值的最佳候选也要记录；只记录通过阈值的候选无法解释失败。

这些诊断**尚未开发或运行**。只保存必要图片；图片包含真实屏幕信息，不上传到公共 issue 或提交到 Git。先由用户本地核对是否为预期 Chrome 页面、是否包含面板、是否有足够重叠。

不要在缺乏帧证据时直接降低 0.85 门槛、缩减到正文 ROI、替换算法、升级 xcap 或引入 OpenCV。用户最终要求全屏；若仅靠缩窄匹配区才能成功，需要明确区分“最终图全屏”和“内部匹配范围”，重新审核必要性。

### 第二步：按诊断结果只处理一个原因

- 若尺寸不一致，先定位截图坐标/缩放转换。
- 若帧内容不正确，先确认显示器、权限和采集后端。
- 若面板进入画面，先隔离 UI 干扰，再评估算法；不把一次无 UI 成功推广到所有页面。
- 若有充分重叠但分数很低，根据实际像素差异判断滚动条、固定栏、动态渲染或行哈希限制。
- 若确无重叠，再调整滚轮步长/等待与采样时机。首次匹配失败的恢复是另一项明确行为，不能靠无限滚动解决。

只跑与当次修改相关的验证。已有合成图单测即使通过，也不能代替 Chrome 实际帧的实验；不补完整跨平台测试体系。

### 第三步：拼接通过后再验证无界面后台会话

最小范围为复用拼接文件及必要采集/滚轮逻辑，不启动 egui，不引入 OCR、历史、上传或第二套 AI 接口。外部 stop 可先用实验用信号验证，原版 CLI 并不存在预先设计的 headless/stop-file 参数。

需验证：开始前目标内容已稳定；停止不再发送滚轮；最后画面稳定后补采尾帧；只完成一次；输出尺寸及内存受控；失败有明确反馈。是否自动到底发送由产品确认，不能偷偷改变远程停止的发送语义。

### 第四步：最后才接入客户端与远程协议

现有客户端 `live_control.rs` 主要处理单次 Screenshot，`useCompletion.ts` 可将图片与 autoPrompt 发给 AI。需要新建独立滚动截图控制，保留原有截图函数行为。

最终长图应单独编码，再交给现有图片提交链路；不能调用会重新截取当前屏幕的 `capture_to_base64` 来代替长图编码。保留远程身份/账号校验；stop 不能被采集中的 busy 拦截。以上均未开发。

## 7. 构建、复现与权限提示

在确认无人同时编译后，可使用：

```sh
cd /Users/teddy/Documents/ddd/shotcraft-lab/upstream
cargo build --release --locked
```

依赖已缓存时可加 `--offline`；不保证新机器离线成功。明确使用 `./target/release/shotcraft`，避免运行未更新的同名复制件。用户先前明确要求自己负责重新编译，不要启动第二个 cargo 争抢锁。

查看帮助不会采集屏幕：

```sh
./target/release/shotcraft scrolling-capture --help
```

标准全屏与日志命令模板（主动运行会截取屏幕并移动鼠标；仅由操作者准备好目标页面后执行）：

```sh
(
  sleep 5
  ./target/release/shotcraft scrolling-capture \
    --display 1 \
    --x 0 --y 0 \
    --width 3072 --height 1920 \
    --start --autoscroll
) 2> /tmp/shotcraft-scroll-next.log
```

- `--display 1` 是主显示器，不是绑定启动终端的窗口；截图区域参数是目标显示器截图的像素坐标。
- `system_profiler SPDisplaysDataType` 已由用户执行；不要把 `UI Looks like` 直接作为截图像素大小。
- 曾提供“screencapture 后用 sips 读取尺寸”的替代方案，但没有证据确认用户执行过该方案。
- macOS 截图与辅助功能是两种权限。主客户端已有截图授权不等于独立实验程序已授权；从 iTerm 启动时按系统归属确认，不能擅自断言只需给某一个固定二进制授权。
- Apple 说明未授权屏幕录制时可见内容可能受限；当前两份日志不能证明权限是根因。
- 不主动重置 TCC、不使用 sudo、不清理用户数据或截图，不提交/推送/发布。

## 8. 尚未完成的验收清单

- [ ] 采集器首帧与失败帧已保存并确认内容、尺寸及重叠正确。
- [ ] 首次 NoMatch 的具体原因及最佳匹配评分已记录。
- [ ] 本机 Chrome/GitHub 向下滚动可生成完整、无明显缺行/重复的长图。
- [ ] 全屏输出成立，不依赖用户手动框选。
- [ ] 控制面板/边框不干扰最终后台方案。
- [ ] 无界面后台开始与外部停止运行验证通过。
- [ ] 最后尾帧、失败结果、超时、大小/内存边界的产品行为明确。
- [ ] 不新增辅助功能授权与远程滚轮控制之间的取舍已决定。
- [ ] Base64 及现有 AI prompt 链路完成验证。
- [ ] Windows / Ubuntu X11 / Ubuntu Wayland 分别验证；当前均未执行。

## 9. 外部证据与本地复核方法

- [ShotCraft 固定提交源码](https://github.com/mmartinez/shotcraft/tree/4f4ddeb62708b5d622f5e5106177528e9f0c7640)。后续上游变更不自动适用于本次结论。
- [Apple 屏幕录制授权](https://support.apple.com/guide/mac-help/control-access-screen-system-audio-recording-mchld6aa7d23/mac)。
- [Apple 辅助功能授权](https://support.apple.com/guide/mac-help/allow-accessibility-apps-to-access-your-mac-mh43185/mac)。
- [Apple WWDC19：Advances in macOS Security](https://developer.apple.com/videos/play/wwdc2019/701/)，作为屏幕访问限制的背景资料，不作为本机权限状态实测。

检查源码改动与提交：

```sh
git rev-parse HEAD
git diff -- crates/ui-egui/src/scrolling.rs
git status --short
```

原始日志位于临时目录，可能被系统清理。本文已保存结果、计数、摘录和摘要校验值；尚未复制日志或原始屏幕图片到仓库。如下附录为 2026-10-29 生成文档时再次读取原日志得到的证据。

## 附录 A：交接时重新计算的日志证据

### `/tmp/shotcraft-scroll.log`

- 最后修改时间：`2026-10-28T11:19:31.043653-07:00`。
- 字节数：4780；总行数：129。
- SHA-256：`b3af1bb561de1d5a0edeed1e358b4a976fcfd9b39b41dbd6f8530e089f1b2bb9`。
- 结果统计：`Appended(1920)` 1 次；`NoMatch` 128 次。
- 按原始顺序对连续相同行压缩展示，重复次数不省略：

```text
[scroll] result=Appended(1920), height=1920    [consecutive count: 1]
[scroll] result=NoMatch, height=1920    [consecutive count: 128]
```

### `/tmp/shotcraft-scroll-manual.log`

- 最后修改时间：`2026-10-28T11:22:20.316626-07:00`。
- 字节数：2494；总行数：69。
- SHA-256：`0b1d537cb428e395377f133ebfe86a3a8852e23895ad234edf6b0e577a5fa751`。
- 结果统计：`Appended(920)` 1 次；`Unchanged` 2 次；`NoMatch` 66 次。
- 按原始顺序对连续相同行压缩展示，重复次数不省略：

```text
[scroll] result=Appended(920), height=920    [consecutive count: 1]
[scroll] result=Unchanged, height=920    [consecutive count: 2]
[scroll] result=NoMatch, height=920    [consecutive count: 66]
```

## 附录 B：本次文档操作边界

2026-10-29 仅新增本交接文档，重新读取源码、已有 diff 和两份日志；未运行截图、未新增诊断源码、未重新编译、未执行测试、未提交或推送。原先由用户加入的一条 stderr 日志保持原样。
