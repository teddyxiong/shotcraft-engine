//! Synchronous, window-free scrolling capture for external callers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use enigo::{Axis, Coordinate, Enigo, Mouse, Settings};
use image::codecs::jpeg::JpegEncoder;
use image::{ImageEncoder, RgbImage, RgbaImage};

use crate::doc::stitch::{Push, Stitcher};
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

/// Capture a scrolling page on the display that best overlaps `bounds`, then return a plain
/// Base64 encoded image. This call blocks while it captures and scrolls. The caller must hide
/// its own windows when they should not appear in the capture and grant screen-recording and
/// pointer-control permissions where the operating system requires them. Set `stop` with
/// `Ordering::Relaxed` to request stopping; a current screen capture and the remaining portion
/// of the 350 ms inter-scroll wait cannot be interrupted.
///
/// `format` selects JPEG with `jpg` or `jpeg`; every other value uses PNG. JPEG quality defaults
/// to 80 and is clamped to 1–100. Only the selected display is captured.
pub fn capture_to_base64(bounds: WindowBounds, format: Option<String>, quality: Option<u8>, stop: &AtomicBool) -> Result<String> {
    if stop.load(Ordering::Relaxed) {
        return Err(Error::Other("scrolling capture stopped before first frame".into()));
    }
    if !bounds.scale_factor.is_finite() || bounds.scale_factor <= 0.0 {
        return Err(Error::Other("window scale factor must be finite and positive".into()));
    }
    if bounds.width == 0 || bounds.height == 0 {
        return Err(Error::Other("window bounds must have a positive size".into()));
    }
    let mut selected: Option<(f64, f64, u32, (i32, i32))> = None;
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
            let center = (pointer_coordinate(monitor_center.0)?, pointer_coordinate(monitor_center.1)?);
            selected = Some((area, distance, monitor.id()?, center));
        }
    }
    let (_, _, monitor_id, center) = selected.ok_or(Error::NoMonitors)?;
    if stop.load(Ordering::Relaxed) {
        return Err(Error::Other("scrolling capture stopped before first frame".into()));
    }
    let first = capture::capture_monitor(Some(monitor_id), None)?.image;
    if first.width() == 0 || first.height() == 0 {
        return Err(Error::Other("selected display returned an empty capture".into()));
    }
    let monitor = if stop.load(Ordering::Relaxed) { None } else { Some(capture::Monitor::by_id(monitor_id).ok_or(Error::MonitorGone(monitor_id))?) };
    let mut enigo = None;
    let image = collect_frames(
        first,
        stop,
        || monitor.as_ref().ok_or_else(|| Error::Other("scrolling capture stopped before next frame".into()))?.capture(),
        || {
            if enigo.is_none() {
                enigo = Some(Enigo::new(&Settings::default()).map_err(|error| {
                    Error::Other(format!("cannot control the pointer for scrolling (check Accessibility/Input Monitoring permissions): {error}"))
                })?);
            }
            enigo
                .as_mut()
                .ok_or_else(|| Error::Other("pointer controller was not initialized".into()))?
                .move_mouse(center.0, center.1, Coordinate::Abs)
                .map_err(|error| Error::Other(format!("cannot move the pointer for scrolling (check Accessibility/Input Monitoring permissions): {error}")))?;
            if stop.load(Ordering::Relaxed) {
                return Ok(false);
            }
            enigo
                .as_mut()
                .ok_or_else(|| Error::Other("pointer controller was not initialized".into()))?
                .scroll(3, Axis::Vertical)
                .map_err(|error| Error::Other(format!("scroll input failed: {error}")))?;
            thread::sleep(Duration::from_millis(350));
            Ok(true)
        },
    )?;
    encode_image_to_base64(&image, format, quality)
}

fn collect_frames(
    first: RgbaImage,
    stop: &AtomicBool,
    mut next_frame: impl FnMut() -> Result<RgbaImage>,
    mut scroll: impl FnMut() -> Result<bool>,
) -> Result<RgbaImage> {
    let mut stitcher = Stitcher::new(40_000);
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
        let result = stitcher.push(frame);
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
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if no_match >= 3 {
            return Err(Error::Other("scrolling capture could not match three consecutive frames".into()));
        }
        if unchanged >= 3 || stitcher.is_full() || frame_number == 49 {
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
    stitcher.finish().ok_or_else(|| Error::Other("scrolling capture produced no image".into()))
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
            || {
                captures += 1;
                Err(Error::Other("unexpected next frame".into()))
            },
            || {
                scrolls += 1;
                stop.store(true, Ordering::Relaxed);
                Ok(true)
            },
        )?;
        assert_eq!(partial, first);
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
            || {
                captures += 1;
                stop.store(true, Ordering::Relaxed);
                Err(Error::Other("capture interrupted".into()))
            },
            || Ok(true),
        )?;
        assert_eq!(partial, first);
        assert_eq!(captures, 1);
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
