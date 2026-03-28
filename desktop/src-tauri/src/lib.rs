mod computer_use;
mod computer_use_bridge;
mod permissions;

use tauri::{Emitter, Manager};
use tauri::menu::{AboutMetadataBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri_plugin_deep_link::DeepLinkExt;

#[tauri::command]
fn navigate(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let ww = app
        .get_webview_window("main")
        .ok_or("main webview not found")?;
    let parsed: url::Url = url.parse().map_err(|e: url::ParseError| e.to_string())?;
    ww.navigate(parsed).map_err(|e| e.to_string())
}

fn navigate_home(app: &tauri::AppHandle) -> Result<(), String> {
    let ww = app
        .get_webview_window("main")
        .ok_or("main webview not found")?;
    // Use the dev URL in dev mode, tauri:// in production
    let url = if cfg!(debug_assertions) {
        "http://localhost:1420/"
    } else {
        "tauri://localhost/"
    };
    let parsed: url::Url = url.parse().map_err(|e: url::ParseError| e.to_string())?;
    ww.navigate(parsed).map_err(|e| e.to_string())
}

fn settings_url() -> String {
    if cfg!(debug_assertions) {
        "http://localhost:1420/settings/".to_string()
    } else {
        "tauri://localhost/settings/".to_string()
    }
}

fn open_settings_window(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::webview::WebviewWindowBuilder;

    // If settings window already exists, just focus it
    if let Some(win) = app.get_webview_window("settings") {
        win.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let url = settings_url();
    WebviewWindowBuilder::new(app, "settings", tauri::WebviewUrl::External(
        url.parse().map_err(|e: url::ParseError| e.to_string())?,
    ))
    .title("Bolly Settings")
    .inner_size(420.0, 480.0)
    .resizable(false)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_http::init())
        .invoke_handler(tauri::generate_handler![
            navigate,
            computer_use::computer_screenshot,
            computer_use::computer_click,
            computer_use::computer_double_click,
            computer_use::computer_mouse_move,
            computer_use::computer_scroll,
            computer_use::computer_type,
            computer_use::computer_key,
            computer_use_bridge::connect_computer_use,
            computer_use_bridge::disconnect_computer_use,
            permissions::check_permissions,
            permissions::open_permission_settings,
        ])
        .setup(|app| {
            let about = AboutMetadataBuilder::new()
                .name(Some("Bolly"))
                .version(Some(env!("CARGO_PKG_VERSION")))
                .website(Some("https://bollyai.dev"))
                .website_label(Some("bollyai.dev"))
                .comments(Some("Your AI companion"))
                .build();

            let settings = MenuItemBuilder::with_id("settings", "Settings...")
                .accelerator("CmdOrCtrl+,")
                .build(app)?;

            let app_menu = SubmenuBuilder::new(app, "Bolly")
                .about(Some(about))
                .separator()
                .items(&[&settings])
                .separator()
                .hide()
                .hide_others()
                .show_all()
                .separator()
                .quit()
                .build()?;

            let edit_menu = SubmenuBuilder::new(app, "Edit")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;

            let back = MenuItemBuilder::with_id("back", "Back to Dashboard")
                .accelerator("CmdOrCtrl+Shift+D")
                .build(app)?;

            let view_menu = SubmenuBuilder::new(app, "View")
                .items(&[&back])
                .build()?;

            let menu = MenuBuilder::new(app)
                .items(&[&app_menu, &edit_menu, &view_menu])
                .build()?;
            app.set_menu(menu)?;

            let handle = app.handle().clone();
            app.on_menu_event(move |_app, event| {
                if event.id() == "back" {
                    let _ = navigate_home(&handle);
                } else if event.id() == "settings" {
                    let _ = open_settings_window(&handle);
                }
            });

            // Handle deep links received while app is running
            let handle2 = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                if let Some(url) = event.urls().first() {
                    handle2.emit("deep-link", url.to_string()).ok();
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
