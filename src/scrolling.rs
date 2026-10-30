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
    /// Stitch the scrolling content into one image, rejecting unmatched frames.
    Stitched,
    /// Return each changed full-screen frame separately; PageDown advances the application.
    Pages,
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
/// image or individual pages as plain Base64 strings. Stitched mode moves the pointer to the
/// display center and uses wheel scrolling; Pages mode sends PageDown to the focused application,
/// which the caller must focus beforehand. The caller must hide its own windows when needed and
/// grant the required screen-recording and input permissions. Set `stop` with `Ordering::Relaxed`
/// to request stopping; an in-flight capture and the remaining 350 ms wait cannot be interrupted.
///
/// `format` selects JPEG with `jpg` or `jpeg`; every other value uses PNG. JPEG quality defaults
/// to 80 and is clamped to 1–100. Only the selected display is captured.
pub fn capture_to_base64(bounds: WindowBounds, format: Option<String>, quality: Option<u8>, stop: &AtomicBool, mode: CaptureMode) -> Result<Vec<String>> {
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
    if first.width() == 0 || first.height() == 0 {
        return Err(Error::Other("selected display returned an empty capture".into()));
    }
    let monitor = if stop.load(Ordering::Relaxed) { None } else { Some(capture::Monitor::by_id(monitor_id).ok_or(Error::MonitorGone(monitor_id))?) };
    let mut enigo = None;
    let images = collect_frames(
        first,
        stop,
        mode,
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
    )?;
    Ok(images)
}

fn collect_frames(
    first: RgbaImage,
    stop: &AtomicBool,
    mode: CaptureMode,
    mut next_frame: impl FnMut() -> Result<RgbaImage>,
    mut scroll: impl FnMut() -> Result<bool>,
    mut encode: impl FnMut(&RgbaImage) -> Result<String>,
) -> Result<Vec<String>> {
    let mut stitcher = (mode == CaptureMode::Stitched).then(|| Stitcher::new(40_000));
    let mut pages = Vec::new();
    let mut last_page: Option<RgbaImage> = None;
    let mut unchanged = 0usize;
    let mut no_match = 0usize;
    let mut first = Some(first);
    for frame_number in 0..50 {
        if frame_number > 0 && stop.load(Ordering::Relaxed) {
            break;
        }
        let frame = if frame_number == 0 {
            first.take().ok_or_else(|| Error::Other("initial capture was already consumed".into()))?
        } else {
            match next_frame() {
                Ok(frame) => frame,
                Err(_) if stop.load(Ordering::Relaxed) => break,
                Err(error) => return Err(error),
            }
        };
        match mode {
            CaptureMode::Stitched => {
                let result = stitcher.as_mut().ok_or_else(|| Error::Other("stitcher was not initialized".into()))?.push(frame);
                match result {
                    Push::Appended(_) => {
                        unchanged = 0;
                        no_match = 0;
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
                    pages.push(encode(&frame)?);
                    last_page = Some(frame);
                }
            }
        }
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if mode == CaptureMode::Stitched && no_match >= 3 {
            return Err(Error::Other("scrolling capture could not match three consecutive frames".into()));
        }
        let full = stitcher.as_ref().is_some_and(Stitcher::is_full);
        if unchanged >= 3 || full || frame_number == 49 {
            break;
        }
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match scroll() {
            Ok(true) => {}
            Ok(false) => break,
            Err(_) if stop.load(Ordering::Relaxed) => break,
            Err(error) => return Err(error),
        }
    }
    match mode {
        CaptureMode::Stitched => {
            let image = stitcher
                .ok_or_else(|| Error::Other("stitcher was not initialized".into()))?
                .finish()
                .ok_or_else(|| Error::Other("scrolling capture produced no image".into()))?;
            Ok(vec![encode(&image)?])
        }
        CaptureMode::Pages if pages.is_empty() => Err(Error::Other("scrolling capture produced no pages".into())),
        CaptureMode::Pages => Ok(pages),
    }
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
        let partial = collect_frames(
            first.clone(),
            &stop,
            CaptureMode::Stitched,
            || {
                captures += 1;
                Err(Error::Other("unexpected next frame".into()))
            },
            || {
                scrolls += 1;
                stop.store(true, Ordering::Relaxed);
                Ok(true)
            },
            |image| encode_image_to_base64(image, None, None),
        )?;
        assert_eq!(partial, vec![encode_image_to_base64(&first, None, None)?]);
        assert_eq!(scrolls, 1);
        assert_eq!(captures, 0);
        Ok(())
    }

    #[test]
    fn stop_during_failed_next_capture_returns_partial_frame() -> Result<()> {
        let first = RgbaImage::from_pixel(8, 8, image::Rgba([30, 60, 90, 255]));
        let stop = AtomicBool::new(false);
        let mut captures = 0;
        let partial = collect_frames(
            first.clone(),
            &stop,
            CaptureMode::Stitched,
            || {
                captures += 1;
                stop.store(true, Ordering::Relaxed);
                Err(Error::Other("capture interrupted".into()))
            },
            || Ok(true),
            |image| encode_image_to_base64(image, None, None),
        )?;
        assert_eq!(partial, vec![encode_image_to_base64(&first, None, None)?]);
        assert_eq!(captures, 1);
        Ok(())
    }

    #[test]
    fn pages_mode_preserves_changed_frames_and_skips_exact_duplicates() -> Result<()> {
        let first = RgbaImage::from_pixel(8, 8, image::Rgba([30, 60, 90, 255]));
        let second = RgbaImage::from_pixel(8, 8, image::Rgba([180, 70, 20, 255]));
        let stop = AtomicBool::new(false);
        let mut captures = 0;
        let pages = collect_frames(
            first.clone(),
            &stop,
            CaptureMode::Pages,
            || {
                captures += 1;
                Ok(second.clone())
            },
            || Ok(true),
            |image| encode_image_to_base64(image, None, None),
        )?;
        assert_eq!(captures, 4);
        assert_eq!(pages.len(), 2);
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
