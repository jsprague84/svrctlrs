#[cfg(desktop)]
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

/// Whether a system tray exists. Without one, closing the window must quit: hiding the app
/// where nothing can bring it back is worse than losing the sessions.
#[cfg(desktop)]
struct TrayAvailable(bool);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Register plugins
            app.handle().plugin(tauri_plugin_os::init())?;
            app.handle().plugin(tauri_plugin_http::init())?;
            #[cfg(mobile)]
            app.handle().plugin(tauri_plugin_biometric::init())?;

            // System tray (desktop only)
            #[cfg(desktop)]
            {
                let show = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show, &quit])?;

                let mut tray = TrayIconBuilder::new()
                    .menu(&menu)
                    .tooltip("SvrCtlRS")
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let tauri::tray::TrayIconEvent::Click { .. } = event {
                            if let Some(window) = tray.app_handle().get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    });
                // A tray without an image is invisible on most desktops, which left the menu
                // unreachable; reuse the bundled application icon.
                if let Some(icon) = app.default_window_icon() {
                    tray = tray.icon(icon.clone());
                }
                let available = match tray.build(app) {
                    Ok(_) => true,
                    Err(e) => {
                        log::warn!("system tray unavailable, close will quit: {e}");
                        false
                    }
                };
                app.manage(TrayAvailable(available));
            }

            Ok(())
        })
        .on_window_event(|_window, _event| {
            // Hide to tray on close instead of quitting (desktop only), but only when a
            // tray exists to bring the window back.
            #[cfg(desktop)]
            if let tauri::WindowEvent::CloseRequested { api, .. } = _event {
                let tray = _window
                    .try_state::<TrayAvailable>()
                    .map(|t| t.0)
                    .unwrap_or(false);
                if tray {
                    let _ = _window.hide();
                    api.prevent_close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
