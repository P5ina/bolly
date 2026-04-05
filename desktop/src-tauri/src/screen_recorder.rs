use std::sync::{Mutex, atomic::{AtomicBool, Ordering}};

static STREAMING: AtomicBool = AtomicBool::new(false);
static LAST_FRAME: Mutex<Option<Vec<u8>>> = Mutex::new(None);

/// Start capturing frames in a background thread.
pub fn start() -> Result<(), String> {
    if STREAMING.load(Ordering::Relaxed) {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    return Err("screen recording not yet supported on Windows".into());

    #[cfg(not(target_os = "windows"))]
    {
        if !scap::is_supported() {
            return Err("platform not supported for screen capture".into());
        }
        if !scap::has_permission() {
            if !scap::request_permission() {
                return Err("screen recording permission denied".into());
            }
        }

        STREAMING.store(true, Ordering::Relaxed);

        std::thread::spawn(|| {
            if let Err(e) = run_capture_loop() {
                eprintln!("[recorder] capture error: {e}");
            }
            STREAMING.store(false, Ordering::Relaxed);
        });

        eprintln!("[recorder] started (scap, 1fps)");
        Ok(())
    }
}

/// Stop capturing.
pub fn stop() -> Result<(), String> {
    STREAMING.store(false, Ordering::Relaxed);
    eprintln!("[recorder] stopped");
    Ok(())
}

/// Get the last captured frame as JPEG bytes.
pub fn get_last_frame() -> Option<Vec<u8>> {
    LAST_FRAME.lock().ok()?.clone()
}

/// Check if currently streaming.
pub fn is_recording() -> bool {
    STREAMING.load(Ordering::Relaxed)
}

#[cfg(not(target_os = "windows"))]
fn run_capture_loop() -> Result<(), String> {
    use std::time::Duration;
    use scap::{
        capturer::{Capturer, Options, Resolution},
        frame::Frame,
    };

    let options = Options {
        fps: 1,
        target: None,
        show_cursor: true,
        show_highlight: false,
        excluded_targets: None,
        output_type: scap::frame::FrameType::BGRAFrame,
        output_resolution: Resolution::_720p,
        crop_area: None,
        ..Default::default()
    };

    let mut capturer = Capturer::build(options)
        .map_err(|e| format!("failed to build capturer: {e}"))?;

    capturer.start_capture();

    while STREAMING.load(Ordering::Relaxed) {
        match capturer.get_next_frame() {
            Ok(Frame::BGRA(frame)) => {
                match bgra_to_jpeg(&frame.data, frame.width as u32, frame.height as u32) {
                    Ok(jpeg) => {
                        if let Ok(mut guard) = LAST_FRAME.lock() {
                            *guard = Some(jpeg);
                        }
                    }
                    Err(e) => eprintln!("[recorder] jpeg error: {e}"),
                }
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("[recorder] frame error: {e}");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }

    capturer.stop_capture();
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn bgra_to_jpeg(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    use image::{RgbaImage, DynamicImage};
    use std::io::Cursor;

    let mut rgba = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(4) {
        rgba.push(chunk[2]);
        rgba.push(chunk[1]);
        rgba.push(chunk[0]);
        rgba.push(chunk[3]);
    }

    let img = RgbaImage::from_raw(width, height, rgba)
        .ok_or("failed to create image")?;
    let rgb = DynamicImage::ImageRgba8(img).to_rgb8();

    let mut buf = Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 60)
        .encode(rgb.as_raw(), rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
        .map_err(|e| e.to_string())?;

    Ok(buf.into_inner())
}
