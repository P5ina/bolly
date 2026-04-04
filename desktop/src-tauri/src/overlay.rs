use tauri::{AppHandle, Emitter, Manager};

fn overlay_url() -> String {
    if cfg!(debug_assertions) {
        "http://localhost:1420/overlay/".to_string()
    } else {
        "tauri://localhost/overlay/".to_string()
    }
}

/// Show the overlay — pure Tauri, dispatched to main thread.
pub fn show(app: &AppHandle) {
    let handle = app.clone();
    let inner = app.clone();
    let _ = handle.run_on_main_thread(move || {
        if inner.get_webview_window("overlay").is_some() {
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

        match tauri::webview::WebviewWindowBuilder::new(
            &inner,
            "overlay",
            tauri::WebviewUrl::External(parsed),
        )
        .title("")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focused(false)
        .resizable(false)
        .maximized(true)
        .build()
        {
            Ok(win) => {
                let _ = win.set_ignore_cursor_events(true);
                eprintln!("[overlay] shown");
            }
            Err(e) => eprintln!("[overlay] build error: {e}"),
        }
    });
}

/// Hide the overlay — dispatched to main thread.
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

/// Signal idle.
pub fn emit_idle(app: &AppHandle) {
    app.emit("computer-use-idle", ()).ok();
}

/// Temporarily hide the overlay (for screenshots) without destroying the window.
pub fn set_visible(app: &AppHandle, visible: bool) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(win) = handle.get_webview_window("overlay") {
            if visible {
                let _ = win.show();
            } else {
                let _ = win.hide();
            }
        }
    });
}
