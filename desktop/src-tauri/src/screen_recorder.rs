use std::path::PathBuf;
use std::sync::Mutex;

use screencapturekit::prelude::*;
use screencapturekit::cm::CMTime;
use screencapturekit::recording_output::{
    SCRecordingOutput, SCRecordingOutputCodec, SCRecordingOutputConfiguration,
    SCRecordingOutputFileType,
};

const RECORDING_PATH: &str = "/tmp/bolly_screen.mp4";

struct RecorderState {
    stream: SCStream,
    recording: SCRecordingOutput,
}

static RECORDER: Mutex<Option<RecorderState>> = Mutex::new(None);

/// Start screen recording using native ScreenCaptureKit.
pub fn start() -> Result<(), String> {
    let mut guard = RECORDER.lock().map_err(|e| e.to_string())?;

    if guard.is_some() {
        return Ok(()); // already recording
    }

    // Get the main display
    let content = SCShareableContent::get()
        .map_err(|e| format!("failed to get shareable content: {e:?}"))?;
    let display = content.displays().into_iter().next()
        .ok_or("no display found")?;

    let width = display.width() as u32;
    let height = display.height() as u32;

    // Capture entire display
    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&[])
        .build();

    // 1 fps, low res for small files
    let stream_config = SCStreamConfiguration::new()
        .with_width(width.min(1280))
        .with_height(height.min(800))
        .with_pixel_format(PixelFormat::BGRA)
        .with_minimum_frame_interval(&CMTime {
            value: 1,
            timescale: 1,
            flags: 0,
            epoch: 0,
        })
        .with_shows_cursor(true);

    // Record to MP4
    let output_path = PathBuf::from(RECORDING_PATH);
    // Remove old file if exists
    let _ = std::fs::remove_file(&output_path);

    let recording_config = SCRecordingOutputConfiguration::new()
        .with_output_url(&output_path)
        .with_video_codec(SCRecordingOutputCodec::H264)
        .with_output_file_type(SCRecordingOutputFileType::MP4);

    let recording = SCRecordingOutput::new(&recording_config)
        .ok_or("failed to create recording output (requires macOS 15+)")?;

    let stream = SCStream::new(&filter, &stream_config);
    stream.add_recording_output(&recording)
        .map_err(|e| format!("failed to add recording output: {e:?}"))?;
    stream.start_capture()
        .map_err(|e| format!("failed to start capture: {e:?}"))?;

    eprintln!("[recorder] started ({width}x{height} → {}x{}, 1fps, H264)",
        width.min(1280), height.min(800));

    *guard = Some(RecorderState { stream, recording });
    Ok(())
}

/// Stop recording. Returns the path to the recorded file.
pub fn stop() -> Result<String, String> {
    let mut guard = RECORDER.lock().map_err(|e| e.to_string())?;

    let state = guard.take()
        .ok_or("not recording")?;

    state.stream.stop_capture()
        .map_err(|e| format!("failed to stop capture: {e:?}"))?;
    state.stream.remove_recording_output(&state.recording)
        .map_err(|e| format!("failed to remove recording output: {e:?}"))?;

    let duration = state.recording.recorded_duration();
    let file_size = state.recording.recorded_file_size();
    eprintln!("[recorder] stopped ({}/{} secs, {} bytes)",
        duration.value, duration.timescale, file_size);

    // Give time for file to finalize
    std::thread::sleep(std::time::Duration::from_millis(500));

    if std::path::Path::new(RECORDING_PATH).exists() {
        Ok(RECORDING_PATH.to_string())
    } else {
        Err("recording file not found after stop".into())
    }
}

/// Check if currently recording.
pub fn is_recording() -> bool {
    RECORDER.lock().map(|g| g.is_some()).unwrap_or(false)
}
