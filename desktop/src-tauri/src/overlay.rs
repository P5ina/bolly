use tauri::{AppHandle, Emitter, Manager};
use tauri::webview::WebviewWindowBuilder;

fn overlay_url() -> String {
    if cfg!(debug_assertions) {
        "http://localhost:1420/overlay/".to_string()
    } else {
        "tauri://localhost/overlay/".to_string()
    }
}

/// Show the computer-use overlay (fullscreen, transparent, click-through, always on top).
pub fn show(app: &AppHandle) {
    if app.get_webview_window("overlay").is_some() {
        return;
    }

    let url = overlay_url();
    let parsed: url::Url = match url.parse() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("[overlay] failed to parse URL: {e}");
            return;
        }
    };

    match WebviewWindowBuilder::new(app, "overlay", tauri::WebviewUrl::External(parsed))
        .title("")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .resizable(false)
        .build()
    {
        Ok(win) => {
            // Click-through: all mouse events pass to windows below
            let _ = win.set_ignore_cursor_events(true);

            // macOS: set window to cover entire screen and make truly transparent
            #[cfg(target_os = "macos")]
            {
                use objc2_app_kit::NSWindow;

                if let Ok(ns_win) = win.ns_window() {
                    unsafe {
                        let ns_window: &NSWindow = &*(ns_win as *const NSWindow);

                        // Set window level above everything (screen saver level = 1000, +1)
                        ns_window.setLevel(1001);

                        // Make background truly transparent
                        ns_window.setOpaque(false);
                        ns_window.setBackgroundColor(Some(
                            &objc2_app_kit::NSColor::clearColor()
                        ));

                        // Cover the entire screen
                        if let Some(screen) = ns_window.screen() {
                            let frame = screen.frame();
                            ns_window.setFrame_display(frame, true);
                        }

                        // Ignore mouse events at the window level
                        ns_window.setIgnoresMouseEvents(true);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("[overlay] failed to create window: {e}");
        }
    }
}

/// Hide the overlay.
pub fn hide(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("overlay") {
        let _ = win.close();
    }
}

/// Notify the overlay of a computer-use action.
pub fn emit_action(app: &AppHandle, action: &str) {
    app.emit("computer-use-action", action.to_string()).ok();
}

/// Signal the overlay that computer use is idle.
pub fn emit_idle(app: &AppHandle) {
    app.emit("computer-use-idle", ()).ok();
}
