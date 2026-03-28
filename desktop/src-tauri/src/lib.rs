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
    let home: url::Url = "tauri://localhost/".parse().map_err(|e: url::ParseError| e.to_string())?;
    ww.navigate(home).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![navigate])
        .setup(|app| {
            let about = AboutMetadataBuilder::new()
                .name(Some("Bolly"))
                .version(Some(env!("CARGO_PKG_VERSION")))
                .website(Some("https://bollyai.dev"))
                .website_label(Some("bollyai.dev"))
                .comments(Some("Your AI companion"))
                .build();

            let app_menu = SubmenuBuilder::new(app, "Bolly")
                .about(Some(about))
                .separator()
                .hide()
                .hide_others()
                .show_all()
                .separator()
                .quit()
                .build()?;

            let back = MenuItemBuilder::with_id("back", "Back to Dashboard")
                .accelerator("CmdOrCtrl+Shift+D")
                .build(app)?;

            let view_menu = SubmenuBuilder::new(app, "View")
                .items(&[&back])
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

            let menu = MenuBuilder::new(app)
                .items(&[&app_menu, &edit_menu, &view_menu])
                .build()?;
            app.set_menu(menu)?;

            let handle = app.handle().clone();
            app.on_menu_event(move |_app, event| {
                if event.id() == "back" {
                    let _ = navigate_home(&handle);
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
