#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod capture;
pub mod scrolling;
pub mod stitch;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Image(#[from] image::ImageError),
    #[error("screen capture: {0}")]
    Capture(#[from] xcap::XCapError),
    #[error("screen capture failed (on macOS, grant Screen Recording permission): {0}")]
    ScreenRecording(xcap::XCapError),
    #[error("no monitors found")]
    NoMonitors,
    #[error("the display used for that capture (id {0}) is no longer connected")]
    MonitorGone(u32),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
