use serde::Serialize;

#[derive(Serialize)]
pub struct PermissionStatus {
    pub screen_recording: bool,
    pub accessibility: bool,
}

/// Check if macOS permissions are granted.
#[tauri::command]
pub fn check_permissions() -> PermissionStatus {
    PermissionStatus {
        screen_recording: check_screen_recording(),
        accessibility: check_accessibility(),
    }
}

/// Open macOS System Settings to the relevant privacy pane.
#[tauri::command]
pub fn open_permission_settings(permission: String) -> Result<(), String> {
    let url = match permission.as_str() {
        "screen_recording" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
        }
        "accessibility" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        }
        _ => return Err(format!("unknown permission: {permission}")),
    };

    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Test screen recording by attempting a tiny capture.
fn check_screen_recording() -> bool {
    screenshots::Screen::all()
        .ok()
        .and_then(|screens| screens.into_iter().next())
        .and_then(|screen| screen.capture().ok())
        .is_some()
}

/// Check accessibility permission via macOS API.
fn check_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        // AXIsProcessTrusted() returns true if accessibility is granted
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        unsafe { AXIsProcessTrusted() }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true // Non-macOS doesn't need this
    }
}
