#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use shotcraft_engine::scrolling::{WindowBounds, capture_to_base64};

const USAGE: &str = "Usage: scroll_capture <new-output-directory> [x y width height scale_factor]\n\
Bounds are physical outer-window coordinates; scale_factor is required for macOS mixed-DPI display selection.\n\
Without geometry, selects the display containing the desktop origin using bounds (0, 0, 100, 100).\n\
During capture, enter exactly `stop` on stdin to request a partial result.";

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let Some(first) = args.next() else {
        return Err(io::Error::other(USAGE).into());
    };
    if first == "--help" || first == "-h" {
        println!("{USAGE}");
        return Ok(());
    }
    let output_dir = PathBuf::from(first);
    let rest: Vec<_> = args.collect();
    let bounds = match rest.as_slice() {
        [] => WindowBounds { x: 0, y: 0, width: 100, height: 100, scale_factor: 1.0 },
        [x, y, width, height, scale] => {
            WindowBounds { x: x.parse()?, y: y.parse()?, width: width.parse()?, height: height.parse()?, scale_factor: scale.parse()? }
        }
        _ => return Err(io::Error::other(USAGE).into()),
    };
    if bounds.width == 0 || bounds.height == 0 {
        return Err(io::Error::other("width and height must be positive").into());
    }
    if !bounds.scale_factor.is_finite() || bounds.scale_factor <= 0.0 {
        return Err(io::Error::other("scale_factor must be finite and positive").into());
    }

    #[cfg(target_os = "windows")]
    enigo::set_dpi_awareness()
        .map_err(|()| io::Error::other("could not set per-monitor DPI awareness for the Windows capture test; launch this standalone test directly"))?;

    fs::create_dir(&output_dir)?;
    let stop = Arc::new(AtomicBool::new(false));
    let listener_stop = Arc::clone(&stop);
    thread::Builder::new().name("scroll-stop-listener".into()).spawn(move || {
        for line in io::stdin().lock().lines() {
            match line {
                Ok(line) if line == "stop" => {
                    listener_stop.store(true, Ordering::Relaxed);
                    break;
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    })?;

    eprintln!("Starting in 10 seconds. Switch to the long page in your browser; type `stop` in this terminal to request a partial capture.");
    thread::sleep(Duration::from_secs(10));
    let encoded = capture_to_base64(bounds, Some("jpeg".into()), Some(80), &stop)?;
    let jpeg = STANDARD.decode(encoded.as_bytes())?;
    let image = image::load_from_memory(&jpeg)?;
    let jpeg_path = output_dir.join("capture.jpg");
    let base64_path = output_dir.join("capture.base64");
    let mut jpeg_file = File::create_new(&jpeg_path)?;
    jpeg_file.write_all(&jpeg)?;
    let mut base64_file = File::create_new(&base64_path)?;
    base64_file.write_all(encoded.as_bytes())?;
    println!("JPEG: {}", jpeg_path.display());
    println!("Base64: {}", base64_path.display());
    println!("Dimensions: {}x{}; JPEG bytes: {}", image.width(), image.height(), jpeg.len());
    Ok(())
}
