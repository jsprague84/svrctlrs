mod auth;

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
            app.handle().plugin(tauri_plugin_opener::init())?;
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
                // The tray library is loaded at run time, and libappindicator-sys panics
                // (instead of returning an error) when it is not installed: run without a tray
                // then, rather than crash at start.
                let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    tray.build(app)
                }));
                let available = match built {
                    Ok(Ok(_)) => true,
                    Ok(Err(e)) => {
                        log::warn!("system tray unavailable, close will quit: {e}");
                        false
                    }
                    Err(_) => {
                        log::warn!("no tray library (libayatana-appindicator), close will quit");
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
        .invoke_handler(tauri::generate_handler![
            auth::sign_in_with_browser,
            auth::stored_token,
            auth::store_token,
            auth::forget_token
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
