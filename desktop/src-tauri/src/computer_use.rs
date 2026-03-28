use base64::{engine::general_purpose::STANDARD, Engine};
use enigo::{
    Axis, Button, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings,
};
use image::codecs::png::PngEncoder;
use image::{DynamicImage, ImageEncoder};
use serde::Serialize;
use std::io::Cursor;
use std::thread;
use std::time::Duration;

/// Max pixels on the longest edge for screenshots sent to Claude.
const MAX_SCREENSHOT_EDGE: u32 = 1568;

// ─── Screenshot ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ScreenshotResult {
    /// Base64-encoded PNG image, resized to fit MAX_SCREENSHOT_EDGE.
    pub image: String,
    /// Width of the returned (scaled) image.
    pub width: u32,
    /// Height of the returned (scaled) image.
    pub height: u32,
    /// Multiply Claude's coordinates by this to get real screen coordinates.
    pub scale: f64,
}

#[tauri::command]
pub fn computer_screenshot() -> Result<ScreenshotResult, String> {
    let screens = screenshots::Screen::all().map_err(|e| e.to_string())?;
    let screen = screens.into_iter().next().ok_or("no screen found")?;

    let capture = screen.capture().map_err(|e| e.to_string())?;
    let real_w = capture.width();
    let real_h = capture.height();

    // Convert to image::DynamicImage
    let img = DynamicImage::ImageRgba8(
        image::RgbaImage::from_raw(real_w, real_h, capture.into_raw())
            .ok_or("failed to create image buffer")?,
    );

    // Scale so the longest edge ≤ MAX_SCREENSHOT_EDGE
    let longest = real_w.max(real_h);
    let (scaled_img, scale) = if longest > MAX_SCREENSHOT_EDGE {
        let ratio = MAX_SCREENSHOT_EDGE as f64 / longest as f64;
        let new_w = (real_w as f64 * ratio).round() as u32;
        let new_h = (real_h as f64 * ratio).round() as u32;
        let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);
        let scale = real_w as f64 / new_w as f64;
        (resized, scale)
    } else {
        (img, 1.0)
    };

    let out_w = scaled_img.width();
    let out_h = scaled_img.height();

    // Encode as PNG → base64
    let mut buf = Cursor::new(Vec::new());
    PngEncoder::new(&mut buf)
        .write_image(
            scaled_img.as_bytes(),
            out_w,
            out_h,
            scaled_img.color().into(),
        )
        .map_err(|e| e.to_string())?;

    let b64 = STANDARD.encode(buf.into_inner());

    Ok(ScreenshotResult {
        image: b64,
        width: out_w,
        height: out_h,
        scale,
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Scale coordinates from Claude's space back to real screen coordinates.
fn scale_coords(x: i32, y: i32, scale: f64) -> (i32, i32) {
    ((x as f64 * scale).round() as i32, (y as f64 * scale).round() as i32)
}

fn new_enigo() -> Result<Enigo, String> {
    Enigo::new(&Settings::default()).map_err(|e| e.to_string())
}

// ─── Mouse ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn computer_click(x: i32, y: i32, scale: f64, button: String) -> Result<(), String> {
    let (rx, ry) = scale_coords(x, y, scale);
    let mut enigo = new_enigo()?;
    enigo
        .move_mouse(rx, ry, Coordinate::Abs)
        .map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(50));

    let btn = match button.as_str() {
        "right" => Button::Right,
        "middle" => Button::Middle,
        _ => Button::Left,
    };
    enigo.button(btn, Direction::Click).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn computer_double_click(x: i32, y: i32, scale: f64) -> Result<(), String> {
    let (rx, ry) = scale_coords(x, y, scale);
    let mut enigo = new_enigo()?;
    enigo
        .move_mouse(rx, ry, Coordinate::Abs)
        .map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(50));
    enigo
        .button(Button::Left, Direction::Click)
        .map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(80));
    enigo
        .button(Button::Left, Direction::Click)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn computer_mouse_move(x: i32, y: i32, scale: f64) -> Result<(), String> {
    let (rx, ry) = scale_coords(x, y, scale);
    let mut enigo = new_enigo()?;
    enigo
        .move_mouse(rx, ry, Coordinate::Abs)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn computer_scroll(x: i32, y: i32, scale: f64, delta_x: i32, delta_y: i32) -> Result<(), String> {
    let (rx, ry) = scale_coords(x, y, scale);
    let mut enigo = new_enigo()?;
    enigo
        .move_mouse(rx, ry, Coordinate::Abs)
        .map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(50));

    if delta_y != 0 {
        enigo
            .scroll(delta_y, Axis::Vertical)
            .map_err(|e| e.to_string())?;
    }
    if delta_x != 0 {
        enigo
            .scroll(delta_x, Axis::Horizontal)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ─── Keyboard ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn computer_type(text: String) -> Result<(), String> {
    let mut enigo = new_enigo()?;
    enigo.text(&text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn computer_key(key: String) -> Result<(), String> {
    use enigo::Key;

    let mut enigo = new_enigo()?;

    // Parse key combo like "ctrl+a", "shift+cmd+z", "Return", "space"
    let parts: Vec<&str> = key.split('+').map(|s| s.trim()).collect();
    let mut modifiers: Vec<Key> = Vec::new();
    let mut main_key: Option<Key> = None;

    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        let parsed = parse_key(part);

        if is_last {
            main_key = Some(parsed);
        } else {
            modifiers.push(parsed);
        }
    }

    // Press modifiers
    for m in &modifiers {
        enigo
            .key(*m, Direction::Press)
            .map_err(|e| e.to_string())?;
    }

    // Press and release main key
    if let Some(k) = main_key {
        enigo
            .key(k, Direction::Click)
            .map_err(|e| e.to_string())?;
    }

    // Release modifiers (reverse order)
    for m in modifiers.iter().rev() {
        enigo
            .key(*m, Direction::Release)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn parse_key(name: &str) -> enigo::Key {
    use enigo::Key;

    match name.to_lowercase().as_str() {
        // Modifiers
        "ctrl" | "control" => Key::Control,
        "alt" | "option" => Key::Alt,
        "shift" => Key::Shift,
        "cmd" | "command" | "meta" | "super" => Key::Meta,

        // Navigation
        "return" | "enter" => Key::Return,
        "tab" => Key::Tab,
        "escape" | "esc" => Key::Escape,
        "space" => Key::Space,
        "backspace" | "delete" => Key::Backspace,
        "forwarddelete" | "del" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "page_up" => Key::PageUp,
        "pagedown" | "page_down" => Key::PageDown,

        // Arrows
        "up" | "arrowup" => Key::UpArrow,
        "down" | "arrowdown" => Key::DownArrow,
        "left" | "arrowleft" => Key::LeftArrow,
        "right" | "arrowright" => Key::RightArrow,

        // Function keys
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,

        // Single character
        other => {
            if other.len() == 1 {
                Key::Unicode(other.chars().next().unwrap())
            } else {
                Key::Unicode(' ') // fallback
            }
        }
    }
}
