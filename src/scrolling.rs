//! Synchronous, window-free scrolling capture for external callers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use enigo::{Axis, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use image::codecs::jpeg::JpegEncoder;
use image::{ImageEncoder, RgbImage, RgbaImage};

use crate::stitch::{Push, Stitcher};
use crate::{Error, Result, capture};

#[derive(Clone, Copy)]
struct Rect {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

/// Physical outer bounds of a caller-owned window, such as Tauri `outer_position` and
/// `outer_size`. This API selects the display and scrolls its center; it does not read the
/// window contents or control the caller's UI.
#[derive(Clone, Copy, Debug)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Tauri window scale factor; used to convert physical bounds to macOS logical coordinates.
    pub scale_factor: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureMode {
    /// Capture the selected display once without moving the pointer or sending input.
    Single,
    /// Stitch the scrolling content into one image, rejecting unmatched frames.
    Stitched,
    /// Return each changed full-screen frame separately; PageDown advances the application.
    Pages,
}

/// Capture-stage state; successful terminal states follow image encoding, not file saving.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureStatus {
    Capturing,
    Encoding,
    SingleFrame,
    Stopped,
    Unchanged,
    FrameLimit,
    HeightLimit,
    Failed,
}

/// Latest committed capture counts. No total page count is known in advance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureProgress {
    pub mode: CaptureMode,
    pub captured_images: Option<u32>,
    pub stitched_height: Option<u32>,
    pub screen_height: u32,
    pub status: CaptureStatus,
    /// Simulated processing progress, not the percentage of the page captured.
    /// Reaches 100 only after successful capture and encoding.
    pub simulated_percent: u8,
}

impl CaptureProgress {
    /// Stitched output height divided by the first captured screen height.
    pub fn approximate_screens(&self) -> Option<f64> {
        self.stitched_height.filter(|_| self.screen_height != 0).map(|height| f64::from(height) / f64::from(self.screen_height))
    }

    fn new(mode: CaptureMode) -> Self {
        Self {
            mode,
            captured_images: (mode != CaptureMode::Stitched).then_some(0),
            stitched_height: (mode == CaptureMode::Stitched).then_some(0),
            screen_height: 0,
            status: CaptureStatus::Capturing,
            simulated_percent: 1,
        }
    }

    fn report(&mut self, status: CaptureStatus, on_progress: &mut impl FnMut(Self)) {
        self.status = status;
        match status {
            CaptureStatus::Capturing if self.screen_height > 0 => {
                let remaining = 90u8.saturating_sub(self.simulated_percent);
                if remaining > 0 {
                    self.simulated_percent = (self.simulated_percent + (remaining / 8).max(1)).min(90);
                }
            }
            CaptureStatus::Encoding if self.mode != CaptureMode::Pages => {
                self.simulated_percent = self.simulated_percent.max(95);
            }
            CaptureStatus::SingleFrame | CaptureStatus::Stopped | CaptureStatus::Unchanged | CaptureStatus::FrameLimit | CaptureStatus::HeightLimit => {
                self.simulated_percent = 100;
            }
            _ => {}
        }
        on_progress(*self);
    }
}

fn bounds_rect(bounds: WindowBounds) -> Result<Rect> {
    #[cfg(target_os = "macos")]
    let scale = bounds.scale_factor;
    #[cfg(not(target_os = "macos"))]
    let scale = 1.0;
    let left = f64::from(bounds.x) / scale;
    let top = f64::from(bounds.y) / scale;
    let right = left + f64::from(bounds.width) / scale;
    let bottom = top + f64::from(bounds.height) / scale;
    finite_rect(Rect { left, top, right, bottom })
}

fn monitor_rect(x: f64, y: f64, width: f64, height: f64, scale: f64) -> Result<Rect> {
    #[cfg(target_os = "linux")]
    let (x, y, width, height) = (x * scale, y * scale, width * scale, height * scale);
    #[cfg(not(target_os = "linux"))]
    let _ = scale;
    finite_rect(Rect { left: x, top: y, right: x + width, bottom: y + height })
}

fn finite_rect(rect: Rect) -> Result<Rect> {
    if [rect.left, rect.top, rect.right, rect.bottom].into_iter().all(f64::is_finite) {
        Ok(rect)
    } else {
        Err(Error::Other("display geometry exceeds coordinate range".into()))
    }
}

fn intersection_area(a: Rect, b: Rect) -> f64 {
    (a.right.min(b.right) - a.left.max(b.left)).max(0.0) * (a.bottom.min(b.bottom) - a.top.max(b.top)).max(0.0)
}

fn pointer_coordinate(value: f64) -> Result<i32> {
    let rounded = value.round();
    if !rounded.is_finite() || rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return Err(Error::Other("monitor center is outside pointer coordinates".into()));
    }
    Ok(rounded as i32)
}

/// Capture a scrolling page on the display that best overlaps `bounds`, then return one stitched
/// image or individual pages as plain Base64 strings. Single mode captures once without input.
/// Stitched mode moves the pointer to the
/// display center and uses wheel scrolling; Pages mode sends PageDown to the focused application,
/// which the caller must focus beforehand. The caller must hide its own windows when needed and
/// grant the required screen-recording and input permissions. Set `stop` with `Ordering::Relaxed`
/// to request stopping; an in-flight capture and the remaining 350 ms wait cannot be interrupted.
///
/// `format` selects JPEG with `jpg` or `jpeg`; every other value uses PNG. JPEG quality defaults
/// to 80 and is clamped to 1–100. Only the selected display is captured.
pub fn capture_to_base64(bounds: WindowBounds, format: Option<String>, quality: Option<u8>, stop: &AtomicBool, mode: CaptureMode) -> Result<Vec<String>> {
    capture_to_base64_with_progress(bounds, format, quality, stop, mode, |_| {})
}

/// Capture with synchronous progress notifications on the calling thread.
/// The callback must be fast, nonblocking and nonpanicking. Terminal events describe capture
/// and encoding only; the returned `Result` remains authoritative when capture fails.
pub fn capture_to_base64_with_progress(
    bounds: WindowBounds,
    format: Option<String>,
    quality: Option<u8>,
    stop: &AtomicBool,
    mode: CaptureMode,
    mut on_progress: impl FnMut(CaptureProgress),
) -> Result<Vec<String>> {
    let mut progress = CaptureProgress::new(mode);
    progress.report(CaptureStatus::Capturing, &mut on_progress);
    let result = capture_with_progress(bounds, format, quality, stop, mode, &mut progress, &mut on_progress);
    if result.is_err() {
        progress.report(CaptureStatus::Failed, &mut on_progress);
    }
    result
}

fn capture_with_progress(
    bounds: WindowBounds,
    format: Option<String>,
    quality: Option<u8>,
    stop: &AtomicBool,
    mode: CaptureMode,
    progress: &mut CaptureProgress,
    on_progress: &mut impl FnMut(CaptureProgress),
) -> Result<Vec<String>> {
    if stop.load(Ordering::Relaxed) {
        return Err(Error::Other("scrolling capture stopped before first frame".into()));
    }
    if !bounds.scale_factor.is_finite() || bounds.scale_factor <= 0.0 {
        return Err(Error::Other("window scale factor must be finite and positive".into()));
    }
    if bounds.width == 0 || bounds.height == 0 {
        return Err(Error::Other("window bounds must have a positive size".into()));
    }
    let mut selected: Option<(f64, f64, u32, (f64, f64))> = None;
    let monitors = xcap::Monitor::all()?;
    if monitors.is_empty() {
        return Err(Error::NoMonitors);
    }
    let window_rect = bounds_rect(bounds)?;
    let window_center = ((window_rect.left + window_rect.right) / 2.0, (window_rect.top + window_rect.bottom) / 2.0);

    for monitor in monitors {
        let scale = monitor.scale_factor()?;
        let scale = if scale.is_finite() && scale > 0.0 { scale as f64 } else { 1.0 };
        let x = f64::from(monitor.x()?);
        let y = f64::from(monitor.y()?);
        let width = f64::from(monitor.width()?);
        let height = f64::from(monitor.height()?);
        let rect = monitor_rect(x, y, width, height, scale)?;
        let area = intersection_area(window_rect, rect);
        let monitor_center = ((rect.left + rect.right) / 2.0, (rect.top + rect.bottom) / 2.0);
        let dx = window_center.0 - monitor_center.0;
        let dy = window_center.1 - monitor_center.1;
        let distance = dx.hypot(dy);
        let replace = selected
            .as_ref()
            .is_none_or(|(best_area, best_distance, _, _)| area > *best_area || (area == 0.0 && *best_area == 0.0 && distance < *best_distance));
        if replace {
            selected = Some((area, distance, monitor.id()?, monitor_center));
        }
    }
    let (_, _, monitor_id, monitor_center) = selected.ok_or(Error::NoMonitors)?;
    let center = if mode == CaptureMode::Stitched { Some((pointer_coordinate(monitor_center.0)?, pointer_coordinate(monitor_center.1)?)) } else { None };
    if stop.load(Ordering::Relaxed) {
        return Err(Error::Other("scrolling capture stopped before first frame".into()));
    }
    let first = capture::capture_monitor(monitor_id)?;
    progress.screen_height = first.height();
    if first.width() == 0 || first.height() == 0 {
        return Err(Error::Other("selected display returned an empty capture".into()));
    }
    if mode == CaptureMode::Single {
        progress.report(CaptureStatus::Encoding, on_progress);
        let encoded = encode_image_to_base64(&first, format, quality)?;
        progress.captured_images = Some(1);
        progress.report(CaptureStatus::SingleFrame, on_progress);
        return Ok(vec![encoded]);
    }
    let monitor = if stop.load(Ordering::Relaxed) { None } else { Some(capture::Monitor::by_id(monitor_id).ok_or(Error::MonitorGone(monitor_id))?) };
    let mut enigo = None;
    let images = collect_frames(
        first,
        stop,
        || monitor.as_ref().ok_or_else(|| Error::Other("scrolling capture stopped before next frame".into()))?.capture(),
        || {
            if enigo.is_none() {
                enigo =
                    Some(Enigo::new(&Settings::default()).map_err(|error| {
                        Error::Other(format!("cannot initialize scrolling input (check Accessibility/Input Monitoring permissions): {error}"))
                    })?);
            }
            if stop.load(Ordering::Relaxed) {
                return Ok(false);
            }
            let controller = enigo.as_mut().ok_or_else(|| Error::Other("pointer controller was not initialized".into()))?;
            match mode {
                CaptureMode::Single => return Ok(false),
                CaptureMode::Stitched => {
                    let center = center.ok_or_else(|| Error::Other("stitched scrolling pointer center is unavailable".into()))?;
                    controller.move_mouse(center.0, center.1, Coordinate::Abs).map_err(|error| {
                        Error::Other(format!("cannot move the pointer for scrolling (check Accessibility/Input Monitoring permissions): {error}"))
                    })?;
                    if stop.load(Ordering::Relaxed) {
                        return Ok(false);
                    }
                    controller.scroll(3, Axis::Vertical).map_err(|error| Error::Other(format!("scroll input failed: {error}")))?;
                }
                CaptureMode::Pages => {
                    controller.key(Key::PageDown, Direction::Click).map_err(|error| Error::Other(format!("PageDown input failed: {error}")))?
                }
            }
            thread::sleep(Duration::from_millis(350));
            Ok(true)
        },
        |image| encode_image_to_base64(image, format.clone(), quality),
        progress,
        on_progress,
    )?;
    Ok(images)
}

fn collect_frames(
    first: RgbaImage,
    stop: &AtomicBool,
    mut next_frame: impl FnMut() -> Result<RgbaImage>,
    mut scroll: impl FnMut() -> Result<bool>,
    mut encode: impl FnMut(&RgbaImage) -> Result<String>,
    progress: &mut CaptureProgress,
    on_progress: &mut impl FnMut(CaptureProgress),
) -> Result<Vec<String>> {
    let mode = progress.mode;
    progress.screen_height = first.height();
    let mut stitcher = (mode == CaptureMode::Stitched).then(|| Stitcher::new(40_000));
    let mut pages = Vec::new();
    let mut last_page: Option<RgbaImage> = None;
    let mut unchanged = 0usize;
    let mut no_match = 0usize;
    let mut first = Some(first);
    let mut terminal = CaptureStatus::FrameLimit;
    for frame_number in 0..50 {
        if frame_number > 0 && stop.load(Ordering::Relaxed) {
            terminal = CaptureStatus::Stopped;
            break;
        }
        let frame = if frame_number == 0 {
            first.take().ok_or_else(|| Error::Other("initial capture was already consumed".into()))?
        } else {
            match next_frame() {
                Ok(frame) => frame,
                Err(_) if stop.load(Ordering::Relaxed) => {
                    terminal = CaptureStatus::Stopped;
                    break;
                }
                Err(error) => return Err(error),
            }
        };
        match mode {
            CaptureMode::Single => return Err(Error::Other("single capture does not collect scrolling frames".into())),
            CaptureMode::Stitched => {
                let stitcher = stitcher.as_mut().ok_or_else(|| Error::Other("stitcher was not initialized".into()))?;
                match stitcher.push(frame) {
                    Push::Appended(_) => {
                        unchanged = 0;
                        no_match = 0;
                        let height = stitcher.height();
                        if progress.stitched_height != Some(height) {
                            progress.stitched_height = Some(height);
                            progress.report(CaptureStatus::Capturing, on_progress);
                        }
                    }
                    Push::Unchanged => {
                        unchanged += 1;
                        no_match = 0;
                    }
                    Push::NoMatch => {
                        no_match += 1;
                        unchanged = 0;
                    }
                }
            }
            CaptureMode::Pages => {
                if last_page.as_ref().is_some_and(|last| last == &frame) {
                    unchanged += 1;
                } else {
                    unchanged = 0;
                    progress.report(CaptureStatus::Encoding, on_progress);
                    pages.push(encode(&frame)?);
                    progress.captured_images = Some(pages.len() as u32);
                    progress.report(CaptureStatus::Capturing, on_progress);
                    last_page = Some(frame);
                }
            }
        }
        if stop.load(Ordering::Relaxed) {
            terminal = CaptureStatus::Stopped;
            break;
        }
        if mode == CaptureMode::Stitched && no_match >= 3 {
            return Err(Error::Other("scrolling capture could not match three consecutive frames".into()));
        }
        let full = stitcher.as_ref().is_some_and(Stitcher::is_full);
        if unchanged >= 3 || full || frame_number == 49 {
            terminal = if unchanged >= 3 {
                CaptureStatus::Unchanged
            } else if full {
                CaptureStatus::HeightLimit
            } else {
                CaptureStatus::FrameLimit
            };
            break;
        }
        if stop.load(Ordering::Relaxed) {
            terminal = CaptureStatus::Stopped;
            break;
        }
        match scroll() {
            Ok(true) => {}
            Ok(false) => {
                terminal = CaptureStatus::Stopped;
                break;
            }
            Err(_) if stop.load(Ordering::Relaxed) => {
                terminal = CaptureStatus::Stopped;
                break;
            }
            Err(error) => return Err(error),
        }
    }
    let images = match mode {
        CaptureMode::Stitched => {
            let image = stitcher
                .ok_or_else(|| Error::Other("stitcher was not initialized".into()))?
                .finish()
                .ok_or_else(|| Error::Other("scrolling capture produced no image".into()))?;
            progress.report(CaptureStatus::Encoding, on_progress);
            vec![encode(&image)?]
        }
        CaptureMode::Pages if pages.is_empty() => return Err(Error::Other("scrolling capture produced no pages".into())),
        CaptureMode::Single | CaptureMode::Pages => pages,
    };
    progress.report(terminal, on_progress);
    Ok(images)
}

/// Encode an existing stitched image using the capture API's format and quality conventions.
pub fn encode_image_to_base64(image: &RgbaImage, format: Option<String>, quality: Option<u8>) -> Result<String> {
    let bytes = encode_image(image, format.as_deref(), quality.unwrap_or(80).clamp(1, 100))?;
    Ok(STANDARD.encode(bytes))
}

fn encode_image(image: &RgbaImage, format: Option<&str>, quality: u8) -> Result<Vec<u8>> {
    if matches!(format, Some("jpg" | "jpeg")) {
        let rgb = RgbImage::from_fn(image.width(), image.height(), |x, y| {
            let pixel = image.get_pixel(x, y);
            let alpha = f32::from(pixel[3]) / 255.0;
            let inverse = 1.0 - alpha;
            let flatten = |channel: u8| (f32::from(channel) * alpha + 255.0 * inverse).round().clamp(0.0, 255.0) as u8;
            image::Rgb([flatten(pixel[0]), flatten(pixel[1]), flatten(pixel[2])])
        });
        let mut bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut bytes, quality).encode_image(&rgb)?;
        Ok(bytes)
    } else {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes).write_image(image.as_raw(), image.width(), image.height(), image::ExtendedColorType::Rgba8)?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn png_base64_round_trips_dimensions() -> Result<()> {
        let image = RgbaImage::from_pixel(3, 2, image::Rgba([20, 40, 60, 255]));
        let encoded = encode_image_to_base64(&image, None, None)?;
        let bytes = STANDARD.decode(encoded).map_err(|error| Error::Other(error.to_string()))?;
        let decoded = image::load_from_memory(&bytes)?.to_rgba8();
        assert_eq!(decoded, image);
        Ok(())
    }

    #[test]
    fn stop_returns_committed_partial_frame() -> Result<()> {
        let first = RgbaImage::from_pixel(8, 8, image::Rgba([30, 60, 90, 255]));
        let stop = AtomicBool::new(false);
        let mut captures = 0;
        let mut scrolls = 0;
        let mut progress = CaptureProgress::new(CaptureMode::Stitched);
        let mut events = Vec::new();
        let encoded = std::cell::Cell::new(false);
        let partial = collect_frames(
            first.clone(),
            &stop,
            || {
                captures += 1;
                Err(Error::Other("unexpected next frame".into()))
            },
            || {
                scrolls += 1;
                stop.store(true, Ordering::Relaxed);
                Ok(true)
            },
            |image| {
                let result = encode_image_to_base64(image, None, None)?;
                encoded.set(true);
                Ok(result)
            },
            &mut progress,
            &mut |event| {
                if event.simulated_percent == 100 {
                    assert!(encoded.get());
                }
                events.push(event);
            },
        )?;
        assert_eq!(partial, vec![encode_image_to_base64(&first, None, None)?]);
        assert_eq!(scrolls, 1);
        assert_eq!(captures, 0);
        assert_eq!(events.iter().map(|event| event.status).collect::<Vec<_>>(), [CaptureStatus::Capturing, CaptureStatus::Encoding, CaptureStatus::Stopped]);
        assert_eq!(events.iter().map(|event| event.simulated_percent).collect::<Vec<_>>(), [12, 95, 100]);
        assert_eq!(progress.stitched_height, Some(first.height()));
        assert_eq!(progress.captured_images, None);
        assert_eq!(progress.approximate_screens(), Some(1.0));
        Ok(())
    }

    #[test]
    fn stop_during_failed_next_capture_returns_partial_frame() -> Result<()> {
        let first = RgbaImage::from_pixel(8, 8, image::Rgba([30, 60, 90, 255]));
        let stop = AtomicBool::new(false);
        let mut captures = 0;
        let mut progress = CaptureProgress::new(CaptureMode::Stitched);
        let mut events = Vec::new();
        let partial = collect_frames(
            first.clone(),
            &stop,
            || {
                captures += 1;
                stop.store(true, Ordering::Relaxed);
                Err(Error::Other("capture interrupted".into()))
            },
            || Ok(true),
            |image| encode_image_to_base64(image, None, None),
            &mut progress,
            &mut |event| events.push(event),
        )?;
        assert_eq!(partial, vec![encode_image_to_base64(&first, None, None)?]);
        assert_eq!(captures, 1);
        assert_eq!(events.last().map(|event| event.status), Some(CaptureStatus::Stopped));
        assert_eq!(progress.stitched_height, Some(first.height()));
        Ok(())
    }

    #[test]
    fn pages_mode_preserves_changed_frames_and_skips_exact_duplicates() -> Result<()> {
        let first = RgbaImage::from_pixel(8, 8, image::Rgba([30, 60, 90, 255]));
        let second = RgbaImage::from_pixel(8, 8, image::Rgba([180, 70, 20, 255]));
        let stop = AtomicBool::new(false);
        let mut captures = 0;
        let mut progress = CaptureProgress::new(CaptureMode::Pages);
        let mut events = Vec::new();
        let pages = collect_frames(
            first.clone(),
            &stop,
            || {
                captures += 1;
                Ok(second.clone())
            },
            || Ok(true),
            |image| encode_image_to_base64(image, None, None),
            &mut progress,
            &mut |event| events.push(event),
        )?;
        assert_eq!(captures, 4);
        assert_eq!(pages.len(), 2);
        let counts = events.iter().filter(|event| event.status == CaptureStatus::Capturing).map(|event| event.captured_images).collect::<Vec<_>>();
        assert_eq!(counts, [Some(1), Some(2)]);
        assert_eq!(events.iter().map(|event| event.simulated_percent).collect::<Vec<_>>(), [1, 12, 12, 21, 100]);
        assert_eq!(events.last().map(|event| event.status), Some(CaptureStatus::Unchanged));
        assert_eq!(progress.screen_height, 8);
        assert_eq!(progress.stitched_height, None);
        assert_eq!(progress.approximate_screens(), None);
        let decoded = pages
            .into_iter()
            .map(|page| {
                let bytes = STANDARD.decode(page).map_err(|error| Error::Other(error.to_string()))?;
                Ok(image::load_from_memory(&bytes)?.to_rgba8())
            })
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(decoded, vec![first, second]);
        Ok(())
    }

    #[test]
    fn stitched_progress_reports_growth_and_skips_duplicate_frames() -> Result<()> {
        let mut noise = 0x1234_5678u32;
        let page = RgbaImage::from_fn(16, 160, |_, _| {
            noise ^= noise << 13;
            noise ^= noise >> 17;
            noise ^= noise << 5;
            let bytes = noise.to_le_bytes();
            image::Rgba([bytes[0], bytes[1], bytes[2], 255])
        });
        let first = image::imageops::crop_imm(&page, 0, 0, 16, 120).to_image();
        let next = image::imageops::crop_imm(&page, 0, 40, 16, 120).to_image();
        let mut progress = CaptureProgress::new(CaptureMode::Stitched);
        let mut events = Vec::new();
        collect_frames(
            first,
            &AtomicBool::new(false),
            || Ok(next.clone()),
            || Ok(true),
            |image| encode_image_to_base64(image, None, None),
            &mut progress,
            &mut |event| events.push(event),
        )?;
        let heights = events.iter().filter(|event| event.status == CaptureStatus::Capturing).map(|event| event.stitched_height).collect::<Vec<_>>();
        assert_eq!(heights, [Some(120), Some(160)]);
        assert_eq!(events.iter().map(|event| event.simulated_percent).collect::<Vec<_>>(), [12, 21, 95, 100]);
        assert_eq!(progress.approximate_screens(), Some(4.0 / 3.0));
        assert_eq!(events.last().map(|event| event.status), Some(CaptureStatus::Unchanged));
        Ok(())
    }

    #[test]
    fn pre_stopped_capture_reports_failure_without_accessing_the_display() {
        let mut events = Vec::new();
        let result = capture_to_base64_with_progress(
            WindowBounds { x: 0, y: 0, width: 0, height: 0, scale_factor: f64::NAN },
            None,
            None,
            &AtomicBool::new(true),
            CaptureMode::Single,
            |event| events.push(event),
        );
        assert!(matches!(result, Err(Error::Other(message)) if message == "scrolling capture stopped before first frame"));
        assert_eq!(events.iter().map(|event| event.status).collect::<Vec<_>>(), [CaptureStatus::Capturing, CaptureStatus::Failed]);
        assert_eq!(events.iter().map(|event| event.simulated_percent).collect::<Vec<_>>(), [1, 1]);
        assert_eq!(events.last().map(|event| (event.captured_images, event.screen_height)), Some((Some(0), 0)));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn physical_window_bounds_select_correct_mixed_scale_display() -> Result<()> {
        let bounds = bounds_rect(WindowBounds { x: 2400, y: 200, width: 500, height: 800, scale_factor: 2.0 })?;
        let display_a = monitor_rect(0.0, 0.0, 1536.0, 1200.0, 2.0)?;
        let display_b = monitor_rect(1536.0, 0.0, 1920.0, 1200.0, 1.0)?;
        assert_eq!(bounds.left, 1200.0);
        assert_eq!(bounds.right, 1450.0);
        assert_eq!(intersection_area(bounds, display_a), 250.0 * 400.0);
        assert_eq!(intersection_area(bounds, display_b), 0.0);
        assert!(intersection_area(bounds, display_a) > intersection_area(bounds, display_b));
        Ok(())
    }
}
