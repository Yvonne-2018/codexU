// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing::{info, warn};

mod app_state;
mod commands;
mod tray;

use app_state::AppState;

const BACKGROUND_CAPTURE_ARGUMENT: &str = "--codexu-native-capture-background";

fn is_background_capture() -> bool {
    std::env::args().any(|argument| argument == BACKGROUND_CAPTURE_ARGUMENT)
}

/// Spawns a background task that periodically refreshes the Codex usage snapshot
/// and pushes a `usage:updated` event so an open dashboard stays current without
/// requiring manual refreshes. The interval is read from `refresh_interval_secs`
/// on every cycle so settings changes take effect without a restart.
fn spawn_usage_auto_refresh(app: tauri::AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        loop {
            let interval_secs = {
                let config = state.config.read().await;
                config.refresh_interval_secs.max(10)
            };
            tokio::time::sleep(Duration::from_secs(interval_secs)).await;

            match state.refresh_usage().await {
                Ok(snapshot) => {
                    let _ = app.emit("usage:updated", ());
                    let language = *state.runtime_language.read().await;
                    let quota = snapshot.as_ref().map(codex_quota_snapshot_from_dashboard);
                    if let Err(error) = tray::update_quota_menu(&app, language, quota.as_ref()) {
                        warn!(error = %error, "Failed to update tray quota menu");
                    }
                }
                Err(error) => {
                    warn!(error = %error, "Background usage auto-refresh failed");
                }
            }
        }
    });
}

/// Converts the official quota windows embedded in a dashboard snapshot into the
/// shape the tray quota menu consumes.
fn codex_quota_snapshot_from_dashboard(
    dashboard: &codexu_core::models::CodexDashboardSnapshot,
) -> codexu_core::readers::CodexAppServerQuotaSnapshot {
    codexu_core::readers::CodexAppServerQuotaSnapshot {
        account: Some(dashboard.codex.snapshot.account.clone()),
        limit_id: Some(dashboard.codex.snapshot.limit_id.clone()),
        limit_name: Some(dashboard.codex.snapshot.limit_name.clone()),
        quota_read_succeeded: dashboard.codex.snapshot.quota_read_succeeded,
        five_hour_quota: dashboard.codex.snapshot.five_hour_quota.clone(),
        seven_day_quota: dashboard.codex.snapshot.seven_day_quota.clone(),
        monthly_quota: dashboard.codex.snapshot.monthly_quota.clone(),
    }
}

#[cfg(windows)]
fn prepare_background_capture_window(window: &tauri::WebviewWindow) {
    use std::ffi::c_void;

    #[link(name = "user32")]
    extern "system" {
        fn GetWindowLongPtrW(hwnd: *mut c_void, index: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: *mut c_void, index: i32, value: isize) -> isize;
    }

    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
    const WS_EX_NOACTIVATE: isize = 0x0800_0000;
    const WS_EX_APPWINDOW: isize = 0x0004_0000;

    let Ok(hwnd) = window.hwnd() else {
        return;
    };

    unsafe {
        let current = GetWindowLongPtrW(hwnd.0, GWL_EXSTYLE);
        let updated = (current | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) & !WS_EX_APPWINDOW;
        let _ = SetWindowLongPtrW(hwnd.0, GWL_EXSTYLE, updated);
    }
}

#[cfg(windows)]
fn show_background_capture_window(window: &tauri::WebviewWindow) {
    use std::ffi::c_void;

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowPos(
            hwnd: *mut c_void,
            insert_after: *mut c_void,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            flags: u32,
        ) -> i32;
        fn ShowWindow(hwnd: *mut c_void, command: i32) -> i32;
    }

    const HWND_BOTTOM: isize = -2;
    const SW_SHOWNOACTIVATE: i32 = 4;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_NOOWNERZORDER: u32 = 0x0200;
    const SWP_FRAMECHANGED: u32 = 0x0020;
    const SWP_SHOWWINDOW: u32 = 0x0040;

    let Ok(hwnd) = window.hwnd() else {
        return;
    };

    unsafe {
        let insert_after = HWND_BOTTOM as *mut c_void;
        let flags = SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_FRAMECHANGED;
        let _ = SetWindowPos(hwnd.0, insert_after, 0, 0, 0, 0, flags);
        let _ = ShowWindow(hwnd.0, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(
            hwnd.0,
            insert_after,
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_SHOWWINDOW,
        );
    }
}

#[cfg(not(windows))]
fn prepare_background_capture_window(_window: &tauri::WebviewWindow) {}

#[cfg(not(windows))]
fn show_background_capture_window(_window: &tauri::WebviewWindow) {}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            crate::tray::show_main_window(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().map_err(|e| {
                eprintln!("Failed to resolve app data dir: {}", e);
                e
            })?;
            info!("App data dir: {}", app_data_dir.display());

            let state = Arc::new(AppState::new(app_data_dir));
            let initial_language = state
                .config
                .try_read()
                .map(|config| config.language.resolved(app_state::ResolvedLanguage::En))
                .unwrap_or(app_state::ResolvedLanguage::En);
            app.manage(state.clone());
            spawn_usage_auto_refresh(app.handle().clone(), state);

            let background_capture = is_background_capture();

            // Hide main window to tray on close instead of quitting.
            if let Some(window) = app.get_webview_window("main") {
                if background_capture {
                    // Keep the capture window non-activating before revealing it.
                    // Calling Tauri's asynchronous `show` here can use an
                    // activating Win32 show path before z-order correction.
                    prepare_background_capture_window(&window);
                    show_background_capture_window(&window);
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        tray::hide_to_tray(&window_clone);
                        api.prevent_close();
                    }
                });
            }

            tray::setup_tray(app.handle(), initial_language)?;

            register_global_shortcut(app.handle());
            if let Err(error) = tray::update_quota_menu(app.handle(), initial_language, None) {
                warn!(error = %error, "Failed to initialize tray quota menu");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::usage::get_local_usage,
            commands::usage::refresh_usage,
            commands::usage::clear_cache,
            commands::settings::get_settings,
            commands::settings::set_settings,
            commands::settings::open_settings_window,
            commands::settings::sync_runtime_language,
            commands::settings::set_autostart,
            commands::settings::get_autostart,
            commands::settings::run_diagnostics,
            commands::updates::check_for_updates,
            tray_show_main_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Registers the Ctrl+U global shortcut used to toggle the main window.
/// Registration failures (e.g. the shortcut is already taken) are logged and
/// ignored so the app keeps running.
fn register_global_shortcut(app: &AppHandle) {
    let shortcut = Shortcut::new(Some(Modifiers::CONTROL), Code::KeyU);
    match app
        .global_shortcut()
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                toggle_main_window(app);
            }
        }) {
        Ok(_) => info!("Registered global shortcut Ctrl+U to toggle the main window"),
        Err(error) => warn!(error = %error, "Failed to register global shortcut Ctrl+U"),
    }
}

/// Shows and focuses the main window, or hides it when it is already visible.
fn toggle_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

#[tauri::command]
fn tray_show_main_window(app: tauri::AppHandle) {
    tray::show_main_window(&app);
}
