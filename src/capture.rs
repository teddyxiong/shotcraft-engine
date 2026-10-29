//! Monitor capture through xcap, with the project's X11 pixmap path on Linux.

use image::RgbaImage;

use crate::{Error, Result};

pub(crate) fn capture_monitor(id: u32) -> Result<RgbaImage> {
    let monitor = xcap::Monitor::all()?.into_iter().find(|monitor| monitor.id().ok() == Some(id)).ok_or(Error::MonitorGone(id))?;
    capture_image(&monitor)
}

pub(crate) struct Monitor(xcap::Monitor);

impl Monitor {
    pub(crate) fn by_id(id: u32) -> Option<Self> {
        xcap::Monitor::all().ok()?.into_iter().find(|monitor| monitor.id().ok() == Some(id)).map(Self)
    }

    pub(crate) fn capture(&self) -> Result<RgbaImage> {
        capture_image(&self.0)
    }
}

fn capture_image(monitor: &xcap::Monitor) -> Result<RgbaImage> {
    #[cfg(target_os = "linux")]
    if !x11::wayland_session()
        && let Some(image) = x11::grab_output(monitor.id()?).map_err(|error| Error::Other(format!("screen capture: {error}")))?
    {
        return Ok(image);
    }
    monitor.capture_image().map_err(Error::ScreenRecording)
}
#[cfg(target_os = "linux")]
mod x11 {
    //! Monitor capture on X11. `GetImage` on the root window has returned bands of stale
    //! content on a composited NVIDIA desktop, so the root is first copied into a pixmap with
    //! `IncludeInferiors` (the server draws each window into it) and that pixmap is read.

    use image::RgbaImage;
    use x11rb::connection::Connection;
    use x11rb::protocol::randr::ConnectionExt as _;
    use x11rb::protocol::xproto::{ConnectionExt as _, CreateGCAux, ImageFormat, ImageOrder, SubwindowMode};

    type BoxError = Box<dyn std::error::Error + Send + Sync>;

    /// The test xcap uses to pick its Wayland backend, which then also captures for us.
    pub fn wayland_session() -> bool {
        std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t == "wayland") || std::env::var("WAYLAND_DISPLAY").is_ok_and(|d| d.to_lowercase().contains("wayland"))
    }

    /// The monitor driving RandR output `output` (xcap's monitor id), or `None` when the
    /// root's pixel format is not 32 bits per pixel and xcap should read it instead.
    pub fn grab_output(output: u32) -> Result<Option<RgbaImage>, BoxError> {
        let (conn, screen) = x11rb::connect(None)?;
        let setup = conn.setup();
        let root = setup.roots.get(screen).ok_or("no X screen")?.root;
        let monitors = conn.randr_get_monitors(root, true)?.reply()?.monitors;
        let m = monitors.iter().find(|m| m.outputs.contains(&output)).ok_or("the monitor is no longer connected")?;
        let depth = conn.get_geometry(root)?.reply()?.depth;
        if setup.pixmap_formats.iter().find(|f| f.depth == depth).map(|f| f.bits_per_pixel) != Some(32) {
            return Ok(None);
        }
        // The pixmap and GC go away with the connection.
        let pixmap = conn.generate_id()?;
        conn.create_pixmap(depth, pixmap, root, m.width, m.height)?;
        let gc = conn.generate_id()?;
        conn.create_gc(gc, pixmap, &CreateGCAux::new().subwindow_mode(SubwindowMode::INCLUDE_INFERIORS))?;
        conn.copy_area(root, pixmap, gc, m.x, m.y, 0, 0, m.width, m.height)?;
        let data = conn.get_image(ImageFormat::Z_PIXMAP, pixmap, 0, 0, m.width, m.height, !0)?.reply()?.data;
        let rgba = to_rgba(data, setup.image_byte_order == ImageOrder::LSB_FIRST);
        Ok(Some(RgbaImage::from_raw(m.width.into(), m.height.into(), rgba).ok_or("the X server returned a short image")?))
    }

    /// 32-bit `xRGB` pixels in the server's byte order to opaque RGBA, in place.
    fn to_rgba(mut data: Vec<u8>, lsb_first: bool) -> Vec<u8> {
        for p in data.as_chunks_mut::<4>().0 {
            let [a, b, c, d] = *p;
            *p = if lsb_first { [c, b, a, 255] } else { [b, c, d, 255] };
        }
        data
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn pixels_convert_from_either_byte_order() {
            // 0x00112233 (red 0x11, green 0x22, blue 0x33) as the server sends it.
            assert_eq!(super::to_rgba(vec![0x33, 0x22, 0x11, 0x00], true), [0x11, 0x22, 0x33, 255]);
            assert_eq!(super::to_rgba(vec![0x00, 0x11, 0x22, 0x33], false), [0x11, 0x22, 0x33, 255]);
        }
    }
}
