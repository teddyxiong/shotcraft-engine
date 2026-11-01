# ShotCraft Engine

Version: **0.1.4**.

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [Français](README.fr.md)

## 1. Features

### Origin and changes

**ShotCraft Engine is a modified and reduced derivative of the source code of [mmartinez/shotcraft](https://github.com/mmartinez/shotcraft).** It retains upstream capture and stitching code and the ShotCraft contributors' copyright notices. This repository is maintained independently and is not an official upstream release.

Compared with the original application, this derivative:

- Extracts the capture component into one Rust library and a CLI; removes the egui UI, editor, OCR, tray, history, application settings, upload and MCP features, and bundled application assets from the active build.
- Reworks scrolling-frame matching with textured patches and broader pixel verification, preserves fixed headers/footers, refreshes overlapping content and keeps the last accepted frame as the stitching anchor. Algorithmic ideas from [LiteSnap](https://github.com/HuibingLin/LiteSnap) informed these changes; no LiteSnap product code, binaries or assets are bundled. See [ATTRIBUTION.md](ATTRIBUTION.md).
- Adds display selection from the host window's physical bounds and scale factor, including mixed-DPI coordinate handling; initializes DPI awareness in the Windows CLI. The retained Linux X11 pixmap capture path comes from upstream. Platform limitations remain documented below.
- Provides JPEG/PNG-to-Base64 APIs, configurable JPEG quality in Rust and ordered image lists for host integration, without a capture panel or clipboard side effects.
- Adds cooperative stopping through an atomic flag in Rust and a `stop` command on CLI stdin, preserving captured content when stopped.
- Adds single-screen capture (`--single`) and separate PageDown captures (`--pages`) alongside stitched scrolling capture.
- Replaces the original application packaging with manually triggered component builds for macOS ARM64/Intel, Windows x64 and Ubuntu x64, and documents integration in four languages.

### Capture modes

A Rust component for full-display capture, with a standalone `scroll_capture` CLI.

| Mode | Input sent | Result |
| --- | --- | --- |
| Default / `CaptureMode::Stitched` | Move the pointer to the selected display center and scroll down | One stitched image |
| `--single` / `CaptureMode::Single` | None | One capture of the current display contents |
| `--pages` / `CaptureMode::Pages` | PageDown | Separate full-display images, in capture order |

Display selection uses the caller's window bounds. The library returns JPEG or PNG as Base64 without resizing; it creates no windows, writes no files and does not change the clipboard. The CLI saves the results to disk. Cooperative stopping preserves captured content. Single mode has been verified on a real desktop; this does not establish validation on every platform.

## 2. Detailed usage

### Syntax and parameters

```text
scroll_capture [--single | --pages] <new-output-directory> [x y width height scale_factor]
scroll_capture --help
scroll_capture -h
```

| Parameter | Accepted values | Default / requirement |
| --- | --- | --- |
| `--single` | Flag before the output directory; cannot be combined with `--pages` | Omitted: stitched mode |
| `--pages` | Flag before the output directory; cannot be combined with `--single` | Omitted: stitched mode |
| `new-output-directory` | Path to a directory that does not exist; its parent must exist. Quote paths with spaces | Required; no overwrite |
| `x`, `y` | Signed `i32` integers, −2,147,483,648 to 2,147,483,647; physical outer-window coordinates | `0`, `0` |
| `width`, `height` | `u32` integers, 1 to 4,294,967,295; physical outer-window dimensions | `100`, `100` |
| `scale_factor` | Finite positive `f64`; the caller window's scale, used for macOS coordinate conversion | `1.0` |
| `--help`, `-h` | Alone, or immediately after `--single` / `--pages` | Print help without capturing or creating a directory |

Supply all five geometry values in order, or omit all five. They select a display, not a crop region: choose the display with the largest overlap, or the nearest display center if none overlaps. Without geometry, bounds `(0, 0, 100, 100, 1.0)` select near the desktop origin, usually the primary display. The CLI does not discover the terminal's or Pluely14's window automatically. A directory name starting with `-` must use a path such as `./--name`.

The CLI always uses **JPEG quality 80** and a **10-second preparation delay**. There are no `--format`, `--quality` or `--delay` options. Use the Rust API to choose encoding.

### Commands

macOS / Ubuntu, after building:

```sh
./target/release/scroll_capture ./capture-01
./target/release/scroll_capture --single ./single-01
./target/release/scroll_capture --pages ./pages-01
```

Windows PowerShell:

```powershell
.\target\release\scroll_capture.exe .\capture-01
.\target\release\scroll_capture.exe --single .\single-01
.\target\release\scroll_capture.exe --pages .\pages-01
```

With Cargo, place program arguments after `--`. The second example specifies physical window bounds on a Retina display:

```sh
cargo run --release --locked --bin scroll_capture -- --single ./single-01
./target/release/scroll_capture --pages ./pages-retina-01 200 200 800 600 2
```

Use a different new directory for each command. During preparation, show the target screen; for scrolling capture, start at the desired page position. Single mode captures the current view and never scrolls back to the top.

Pages mode captures first, then sends one PageDown and waits 350 ms before each subsequent capture. Focus the intended page body on the selected display yourself. It does not move the pointer, click or claim focus. PageDown follows the focused application's behavior, not an exact display-height displacement; overlap is expected. Address bars, editors and nested scrolling areas can receive the key instead.

### Files and stopping

| Mode | Output files |
| --- | --- |
| Stitched / Single | `capture.jpg`, `capture.base64` |
| Pages | `page-0001.jpg`, `page-0001.base64`, `page-0002.jpg`, `page-0002.base64`, … |

Each `.base64` file contains standard Base64 for its JPEG, without a data URI prefix. Full-display images retain capture dimensions; stitched images retain the resulting stitched dimensions. No resizing occurs.

Enter exactly `stop` followed by Enter on stdin to stop further scrolling and return captured content. It is lowercase with no surrounding spaces. Before the first frame, a stop returns an error without an image. Single mode preserves a frame already captured. Switching to the terminal may contaminate captures; a host can send `stop\n` through a pipe without changing focus.

Files are written only after capture returns, not after each frame. `Ctrl+C` or forced termination may lose all unsaved results. A normal capture error generally does not save earlier frames; a write error may leave partial files. Exit code 0 means successful completion; failures return a nonzero code. stdout lists paths, dimensions and JPEG byte counts, not raw Base64. Directory existence alone does not indicate success.

### Reading live CLI progress

Since `0.1.3`, every capture command automatically writes progress to **stderr**, one line per event. No new CLI option is needed. Only lines beginning with the exact prefix `PROGRESS ` are machine-readable; parse the remainder as JSON. Preparation and error messages can also appear on stderr. stdout still contains the final file paths and dimensions.

```text
PROGRESS {"mode":"pages","captured_images":6,"stitched_height":null,"screen_height":1080,"approximate_screens":null,"simulated_percent":48,"status":"capturing"}
PROGRESS {"mode":"stitched","captured_images":null,"stitched_height":2592,"screen_height":1080,"approximate_screens":2.4,"simulated_percent":48,"status":"capturing"}
```

Read `captured_images` for Single/Pages, or `approximate_screens` for Stitched. The latter equals `stitched_height / screen_height`; the unit is one full first-frame height, including visible application/system chrome, not a document page. `null` means the field does not apply or the first screen height is not yet known. The first event has zero counts and `screen_height: 0`.

Each event is a snapshot: replace the displayed value rather than incrementing once per message. Encoding/status events can repeat the same counts, and unchanged or rejected frames do not increase the count. These values describe only this capture session, not total page length or percentage. Use “Captured 6 images” or “Captured approximately 2.4 screens”.

For example, this Python 3 subprocess reader uses only the standard library. Use a fresh output directory, change `--pages` to `--single`, or omit the mode argument for Stitched. stdout is inherited, so final file output is not left blocked in an unread pipe:

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

For other hosts, consume stderr continuously and apply the same prefix/JSON parsing. If both stdout and stderr are piped, drain both. Counts mean captured/encoded content, not files already saved. Wait for process exit code 0 before treating file output as successful, even after a successful capture terminal event.

## 3. Integration with other software

### Rust dependency and API

Use a local checkout (adjust the path):

```toml
[dependencies]
shotcraft-engine = { path = "../shotcraft-engine" }
```

Or use Git; private repositories require Git access in the build environment:

```toml
[dependencies]
shotcraft-engine = { git = "https://github.com/teddyxiong/shotcraft-engine.git", branch = "main" }
```

The library is compiled into the host through Cargo; it is not a ready-made C ABI DLL or HTTP service. Public entry points are in `shotcraft_engine::scrolling`:

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

| API parameter | Values and behavior |
| --- | --- |
| `bounds: WindowBounds` | Required fields: `x: i32`, `y: i32`, `width: u32`, `height: u32`, `scale_factor: f64`. Same units, valid ranges and display-selection rules as the CLI; no library default |
| `format: Option<String>` | Exactly lowercase `Some("jpg".into())` or `Some("jpeg".into())` selects JPEG; `None`, `"png"` and every other string select PNG. Case-sensitive |
| `quality: Option<u8>` | Input 0–255; `None` means 80; clamp to 1–100 for JPEG. Ignored for PNG. JPEG composites transparency over white |
| `stop: &AtomicBool` | Required; initialize to `false`, set to `true` from another thread/event handler to stop cooperatively |
| `mode: CaptureMode` | Required: `Single`, `Stitched` or `Pages`; no default |
| `image: &image::RgbaImage` | Encoder only: an existing RGBA image; no capture or input |

Example: capture one screen on a worker thread. Replace the sample bounds with the host's physical outer position/size and its own scale factor (for example, Tauri `outer_position`, `outer_size`, `scale_factor`):

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

Choose `CaptureMode::Stitched` for one long image, or `CaptureMode::Pages` for separate pages. Both `Single` and `Stitched` return a list of length 1; `Pages` returns one string per accepted page. Successful image results are nonempty and returned together when the call completes; the optional progress callback reports numbers and states only.

This API blocks and has no 10-second CLI delay. Use a worker thread or a host's `spawn_blocking`; never synchronously join on the UI event thread. The host owns window visibility, focus, permissions and shortcut registration. The library has no Tauri dependency or built-in global stop shortcut.

Keep the `Arc<AtomicBool>` in application state. In the stop command or shortcut handler:

```rust,ignore
stop.store(true, std::sync::atomic::Ordering::Relaxed);
```

Create a new flag for each task; do not reset it during a capture. A stop preserves committed images. In-flight capture, stitching, encoding and the remaining 350 ms wait cannot be forcibly interrupted, and already-issued input cannot be undone. Single mode skips that wait, all input, the continuous capture backend and the stitcher.

For an existing image, encode directly:

```rust,ignore
let base64 = shotcraft_engine::scrolling::encode_image_to_base64(
    &image, Some("jpeg".into()), Some(80),
)?;
```

Base64 decoding yields the selected image format's bytes. Pass strings directly if the receiver expects plain Base64, or prepend `data:image/jpeg;base64,` / `data:image/png;base64,` if required. For Pages, pass images individually in list order; concatenating Base64 strings does not form one image. The library does not upload to AI services or Pluely14.

### Reading live progress in Rust

The existing `capture_to_base64` API still works without a callback. To receive progress, call `capture_to_base64_with_progress` with the same five arguments plus a callback. Inside the worker from the preceding example, use:

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

Select `CaptureMode::Single` to receive `captured_images = Some(1)` on success, `Pages` for accepted image counts, or `Stitched` for `approximate_screens()`. Images are still returned together as `Vec<String>`; progress notifications contain no images or Base64.

| Rust value | CLI JSON value | Meaning |
| --- | --- | --- |
| `progress.mode` | `mode`: `single`, `pages`, `stitched` | Current mode |
| `progress.captured_images` | `captured_images`: integer or `null` | Single/Pages: accepted image count; Stitched: `None` / `null` |
| `progress.stitched_height` | `stitched_height`: integer or `null` | Stitched: current output height in pixels; other modes: `None` / `null` |
| `progress.screen_height` | `screen_height`: integer | First screenshot height in pixels; initially 0, then fixed |
| `progress.approximate_screens()` | `approximate_screens`: number or `null` | Stitched height divided by nonzero first screenshot height; otherwise unavailable |
| `progress.simulated_percent` | `simulated_percent`: integer 1–100 | Simulated processing percentage; 100 only after successful result encoding, not file saving |
| `progress.status` | `status`: string below | Current phase or capture end reason |

### Simulated processing percentage

Read `progress.simulated_percent` in Rust or `simulated_percent` in CLI JSON. This separate integer goes from **1 to 100** and describes simulated processing progress, not the page's scroll position, captured fraction or remaining time. It does not read the scrollbar or estimate total page height. The callback, command and arguments are unchanged; no new option is required.

The initial event is 1%. Each committed image or increase in stitched height advances toward 90% using `p = min(90, p + max(1, (90 - p) / 8))`, with integer division. For example, successive content events give 12%, 21%, 29%, 36%, 42%, 48%, then grow more slowly. Single/Stitched final encoding advances to at least 95%. Pages encode each image during capture, so those encoding events do not jump to 95%.

Only after all result images have been successfully encoded does a successful terminal event report 100%. `Failed` keeps the previous value below 100%. A successful partial result after `Stopped`, `FrameLimit` or `HeightLimit` also reports 100%: the current task finished processing, not necessarily the entire page. Check `status` to distinguish these outcomes. Values are monotonic within a task but update only on events, may repeat, and can jump; no timer or continuous sequence of every integer is promised.

Display this as “Processing: 48%”. Keep “Captured 6 images” / “approximately 2.4 screens” as the separate content counters. CLI 100% still does not confirm file saving; wait for exit code 0. A host can animate between received values, but should cap its own animation below 100 until it receives successful completion.

| `CaptureStatus` | CLI `status` | Meaning |
| --- | --- | --- |
| `Capturing` | `capturing` | Initial snapshot or newly committed content |
| `Encoding` | `encoding` | Encoding an accepted image; may occur once per page |
| `SingleFrame` | `single_frame` | Single image successfully encoded |
| `Stopped` | `stopped` | User stop; committed content successfully encoded |
| `Unchanged` | `unchanged` | Three consecutive unchanged frames; not proof of reaching the page bottom |
| `FrameLimit` | `frame_limit` | 50 capture attempts reached |
| `HeightLimit` | `height_limit` | Stitched height limit reached |
| `Failed` | `failed` | Capture/encoding failed; inspect the returned `Err` |

The last six states are terminal capture states. Successful terminal events follow encoding, not CLI file saving. If capture fails after reporting some content, the `Result` is still an error and does not return those images. Keep the last count informational; it is not a saved-result guarantee.

Callbacks execute synchronously on the capture thread, not automatically on the UI thread. Keep them fast, nonblocking and nonpanicking; enqueue only the small snapshot to the host's UI/event loop. There is no fixed-rate heartbeat. A session is identified by the callback/task owned by the host; no global progress storage or task registry is added.

### CLI as a subprocess

Bundle the CLI executable for the target OS/architecture and the required license files. Launch it with the arguments above and a fresh output directory; keep stdin piped. To stop, write and flush `stop\n`; closing stdin alone does not stop capture. Wait for exit code 0 before reading `.jpg` / `.base64` files. The CLI is an executable, while direct Rust integration uses the source dependency above.

## 4. Platforms and boundaries

### Current behavior and limits

- Stitched: at most 50 capture attempts or 40,000 pixels in height. Three consecutive unchanged frames end capture; three consecutive unmatched frames return an error. Hitting a limit returns the captured portion, not a guarantee of reaching the bottom. Input uses 3 vertical wheel units per step, then a 350 ms wait.
- Stitched can recognize a verified stationary content region even when peripheral interface elements change; this avoids requiring a byte-identical full-screen image. An unchanged result still does not prove the page bottom was reached.
- Pages: at most 50 capture attempts; consecutive exactly identical RGBA frames are skipped, and three unchanged frames end capture. No similarity-based deduplication, no stitching requirement and no 40,000-pixel stitched-height limit. Animation, video or system status changes may prevent bottom detection; use `stop` or the attempt limit.
- Single: exactly one capture, without input or stitching. Screen-capture authorization is still required. This does not by itself establish native Wayland support.
- When a subsequent capture/input error coincides with a stop, committed content takes priority. A screenshot component without its own windows still captures other visible content; scrolling modes visibly change the desktop.
- macOS: the invoking process needs screen-recording permission and, for scrolling modes, the permissions required for automated input.
- Windows: the standalone CLI initializes DPI awareness; the library does not change the host's process DPI state. Stitched pointer positioning uses Enigo's primary-display normalization, so secondary-display wheel scrolling is known to be unreliable. Validate on the primary display first. Pages sends keys to the focused app; secondary-display behavior still requires desktop testing.
- Ubuntu: current scrolling input uses X11. Check `echo "$XDG_SESSION_TYPE"`; native Wayland scrolling is not adapted. Builds target Ubuntu 24.04. A 22.04 container `--help` check verifies startup only, not desktop capture.

Ubuntu runtime libraries:

```sh
sudo apt-get install -y libpipewire-0.3-0 libxcb1 libgbm1 libegl1 libx11-6 libwayland-client0
```

### Native Wayland: requirements and current status

The locked xcap version has single-capture Wayland paths, but this component has no persistent Portal/PipeWire capture session. Enigo currently uses its default X11 input backend. Installing headers, compiling successfully or obtaining one screenshot does not prove native Wayland scrolling support.

| Required capability | Purpose |
| --- | --- |
| Real Wayland desktop and user-session D-Bus | Communicate with desktop services and test actual behavior |
| xdg-desktop-portal and a matching desktop backend | GNOME/KDE/etc. must implement the needed interfaces |
| ScreenCast Portal and PipeWire | Continuous frames of the selected display in an authorized session |
| RemoteDesktop Portal input | Pointer/wheel for Stitched, PageDown keyboard input for Pages, through supported methods or libei/EIS |
| Screen and relevant input-device authorization | Screen permission alone does not authorize pointer or keyboard input |

The adaptation direction is to reuse one authorized capture/input session, map stream position, size and scale to the target display, then reuse stitching, encoding and stopping. Enabling one Enigo feature alone is insufficient. Compositor coordinates cannot simply be assumed to be physical pixels; the existing window-overlap selection also needs validation.

The component can avoid its own capture panels, but standard Portal flows usually show system authorization/display selection. Where supported, `persist_mode` and `restore_token` reduce repeated prompts; tokens must be updated and revocation, display changes or failed restoration can prompt again. No universal guarantee of silent first use or permanently prompt-free operation is possible.

For global shortcut stopping, the host must also verify and bind desktop GlobalShortcuts Portal support. Validate continuous frames, input, stitching and partial stopping on Ubuntu 24.04 + GNOME Wayland first, then other desktops such as KDE; Ubuntu version alone is not a compatibility criterion. Single mode uses the existing screenshot backend and remains dependent on the desktop's capture/authorization behavior.

Portal references: [ScreenCast](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html) · [RemoteDesktop](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html) · [GlobalShortcuts](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html)

## 5. Building locally and on GitHub

### Local prerequisites

Install **Rust 1.95 or newer** with Cargo. Obtain the source and check the active toolchain:

```sh
git clone https://github.com/teddyxiong/shotcraft-engine.git
cd shotcraft-engine
rustc --version
cargo --version
```

If installed through rustup, `rustup update stable` updates the stable toolchain. Private-repository authentication must already be configured.

| Platform | Build requirements |
| --- | --- |
| macOS | Xcode Command Line Tools (`xcode-select --install`) and a native Rust toolchain |
| Windows x64 | Visual Studio C++ Build Tools, Windows SDK and the `x86_64-pc-windows-msvc` Rust toolchain; build from a configured developer terminal |
| Ubuntu 24.04 x64 | Rust and the development packages below |

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libclang-dev libxcb1-dev libgbm-dev libegl-dev libpipewire-0.3-dev libspa-0.2-dev libwayland-dev
```

From the repository root, build the CLI or check the library:

```sh
cargo build --release --locked --bin scroll_capture
cargo check --lib --locked
```

The native CLI is `target/release/scroll_capture` (macOS/Ubuntu) or `target/release/scroll_capture.exe` (Windows). Cargo builds the library automatically when another Rust project depends on it. If explicitly using `--target <triple>`, output is under `target/<triple>/release/`.

Project checks:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

### Manual GitHub Actions builds

Only **Engine native builds**, in [engine-build.yml](.github/workflows/engine-build.yml), is active. Its trigger is `workflow_dispatch`: pushes, PRs, tags and schedules do not trigger builds.

1. Open [Actions](https://github.com/teddyxiong/shotcraft-engine/actions/workflows/engine-build.yml).
2. Click **Run workflow**, select the branch containing the desired changes, then click **Run workflow** again.
3. Wait for all four build jobs and download the matching Artifact from the run page. Artifacts are retained for 14 days.

| Platform | Target triple | Artifact |
| --- | --- | --- |
| macOS ARM64 | `aarch64-apple-darwin` | `scroll-capture-macos-arm64` |
| macOS Intel | `x86_64-apple-darwin` | `scroll-capture-macos-x64` |
| Windows x64 | `x86_64-pc-windows-msvc` | `scroll-capture-windows-x64` |
| Ubuntu x64 | `x86_64-unknown-linux-gnu` | `scroll-capture-ubuntu-x64` |

Jobs run formatting, Clippy, tests, a release build and `--help`; Ubuntu also runs the 22.04 startup check. Artifacts contain the executable, English README, licenses, attribution, commit and toolchain information. Other README languages are available in the repository.

Extract the matching Artifact. From its directory run `./scroll_capture --single ./single-01` or, on Windows, `.\scroll_capture.exe --single .\single-01`. On macOS/Ubuntu, run `chmod +x ./scroll_capture` first if needed. Use the section 2 arguments for scrolling tests and a new output directory each time. Verify full capture and stopping on a real desktop: CI build/help checks do not establish desktop behavior.

## 6. Source and license

### License and usage permissions

Maintainer-owned contributions and modifications distributed with this revision use the custom [ShotCraft Engine Source-Available License 1.0](LICENSE). This revision of the component as a whole is no longer offered under a blanket MIT/Apache license. The English `LICENSE` is authoritative; this summary explains its scope.

| Use | Authorization |
| --- | --- |
| Software with no user access, usage or feature fee, published by an individual, company or organization | Free under `LICENSE` |
| Free software funded by advertising, voluntary sponsorship or optional donations | Free, provided payment is not required for access, use or features |
| An organization's own internal use, including business operations | Free under `LICENSE` |
| Software charging users for purchase, subscription, licensing, a paid tier, feature unlock or hosted software access | Separate written paid commercial license required, even if the capture feature itself is free |

The rule applies to library integration and invoking an integrated CLI, including modified versions. No source disclosure of an embedding application or private changes is required by this custom license. Contact the maintainer through a [repository Issue](https://github.com/teddyxiong/shotcraft-engine/issues) to arrange a commercial license; price and terms are agreed separately. An inquiry or donation alone is not authorization.

Upstream ShotCraft portions retain their **MIT OR Apache-2.0** licenses and copyright. Dependencies retain their own licenses. This custom license covers only material the maintainers have the right to license; it cannot remove independently granted upstream rights or earlier permissions. In particular, **`v0.1.1` at `114184a9a7a37928c818b1aa8b804bcf02c869a7` and earlier versions keep their original terms**. Someone with a valid earlier permissive grant may continue using the covered earlier code under that grant; the new requirement does not force existing licensees to pay.

This is a **source-available** component with a paid-software restriction, not an OSI-approved open-source license. The repository tag `v0.1.1` is unchanged; package version `0.1.1` alone does not identify the license of later main-branch commits, so pin and review the exact commit. This change does not create a new release tag.

If you publish modifications or a derivative, please consider notifying the maintainer through an Issue. This is voluntary, not an additional license condition or a substitute for paid authorization.

| File | Responsibility |
| --- | --- |
| `src/lib.rs` | Library exports and structured errors |
| `src/capture.rs` | Display capture, including the Linux X11 pixmap path |
| `src/stitch.rs` | Pixel-based scrolling-frame stitching |
| `src/scrolling.rs` | Display selection, modes, stopping and Base64 encoding |
| `src/bin/scroll_capture.rs` | CLI arguments and output files |

Upstream attribution and license texts are retained in [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE), [NOTICE](NOTICE) and [ATTRIBUTION.md](ATTRIBUTION.md). Dependency licenses are in generated [THIRD-PARTY-LICENSES.md](THIRD-PARTY-LICENSES.md). Distribute [LICENSE](LICENSE) together with all applicable upstream and dependency notices when redistributing the component or embedding it in a product. The original ShotCraft code is not relicensed by this document.

The original UI, OCR, editor, tray and bundled assets are excluded from the active component. Ignored local `backups/` is recovery material, not included in Git distributions or builds.
