use tauri::{AppHandle, Emitter, Manager};

fn overlay_url() -> String {
    if cfg!(debug_assertions) {
        "http://localhost:1420/overlay/".to_string()
    } else {
        "tauri://localhost/overlay/".to_string()
    }
}

/// Show the overlay — dispatched to the main thread (required by AppKit).
pub fn show(app: &AppHandle) {
    let handle = app.clone();
    let inner = app.clone();
    let _ = handle.run_on_main_thread(move || {
        show_inner(inner);
    });
}

fn show_inner(app: AppHandle) {
    if app.get_webview_window("overlay").is_some() {
        return;
    }

    let url = overlay_url();
    let parsed: url::Url = match url.parse() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("[overlay] bad URL: {e}");
            return;
        }
    };

    let win = match tauri::webview::WebviewWindowBuilder::new(
        &app,
        "overlay",
        tauri::WebviewUrl::External(parsed),
    )
    .title("")
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .resizable(false)
    .inner_size(800.0, 200.0)
    .build()
    {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[overlay] build error: {e}");
            return;
        }
    };

    let _ = win.set_ignore_cursor_events(true);

    #[cfg(target_os = "macos")]
    apply_macos_overlay(&win);

    eprintln!("[overlay] shown");
}

#[cfg(target_os = "macos")]
fn apply_macos_overlay(win: &tauri::WebviewWindow) {
    use objc2_app_kit::NSWindow;

    match win.ns_window() {
        Ok(ns_win) => unsafe {
            let ns_window: &NSWindow = &*(ns_win as *const NSWindow);
            ns_window.setLevel(1001);
            ns_window.setOpaque(false);
            ns_window.setBackgroundColor(Some(&objc2_app_kit::NSColor::clearColor()));
            ns_window.setIgnoresMouseEvents(true);

            if let Some(screen) = ns_window.screen() {
                ns_window.setFrame_display(screen.frame(), true);
            }
        },
        Err(e) => eprintln!("[overlay] ns_window error: {e}"),
    }
}

/// Hide the overlay — dispatched to the main thread.
pub fn hide(app: &AppHandle) {
    let caller = app.clone();
    let inner = app.clone();
    let _ = caller.run_on_main_thread(move || {
        if let Some(win) = inner.get_webview_window("overlay") {
            let _ = win.close();
            eprintln!("[overlay] hidden");
        }
    });
}

/// Notify the overlay of a computer-use action.
pub fn emit_action(app: &AppHandle, action: &str) {
    app.emit("computer-use-action", action.to_string()).ok();
}

/// Signal the overlay that computer use is idle.
pub fn emit_idle(app: &AppHandle) {
    app.emit("computer-use-idle", ()).ok();
}
