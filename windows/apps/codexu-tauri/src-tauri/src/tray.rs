use std::sync::Mutex;

use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tracing::warn;

use crate::app_state::ResolvedLanguage;
use crate::commands::usage::refresh_usage;

struct TrayMenu {
    open: Mutex<MenuItem<tauri::Wry>>,
    settings: Mutex<MenuItem<tauri::Wry>>,
    refresh: Mutex<MenuItem<tauri::Wry>>,
    quit: Mutex<MenuItem<tauri::Wry>>,
}

struct TrayLabels {
    open: &'static str,
    settings: &'static str,
    refresh: &'static str,
    quit: &'static str,
}

pub fn setup_tray(app: &AppHandle, language: ResolvedLanguage) -> anyhow::Result<()> {
    let labels = labels_for(language);
    let open_i = MenuItem::with_id(app, "open", labels.open, true, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "settings", labels.settings, true, None::<&str>)?;
    let refresh_i = MenuItem::with_id(app, "refresh", labels.refresh, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_i = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[&open_i, &settings_i, &refresh_i, &separator, &quit_i],
    )?;

    app.manage(TrayMenu {
        open: Mutex::new(open_i.clone()),
        settings: Mutex::new(settings_i.clone()),
        refresh: Mutex::new(refresh_i.clone()),
        quit: Mutex::new(quit_i.clone()),
    });

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("codexU")
        .menu(&menu)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                show_main_window_or_log(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window_or_log(app),
            "settings" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = crate::commands::settings::open_settings_window(app).await;
                });
            }
            "refresh" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<std::sync::Arc<crate::app_state::AppState>>();
                    let _ = refresh_usage(app.clone(), state).await;
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// Rebuilds the tray menu with disabled Codex quota lines inserted above the
/// Open/Settings/Refresh items. The action items keep their fixed ids so the
/// existing menu event handler keeps working after the menu is replaced.
pub fn update_quota_menu(
    app: &AppHandle,
    language: ResolvedLanguage,
    quota: Option<&codexu_core::readers::CodexAppServerQuotaSnapshot>,
) -> anyhow::Result<()> {
    let labels = labels_for(language);
    let open_i = MenuItem::with_id(app, "open", labels.open, true, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "settings", labels.settings, true, None::<&str>)?;
    let refresh_i = MenuItem::with_id(app, "refresh", labels.refresh, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_i = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;

    let mut quota_items: Vec<MenuItem<tauri::Wry>> = Vec::new();
    let mut had_quota = false;
    if let Some(quota) = quota {
        if let Some(window) = &quota.five_hour_quota {
            quota_items.push(MenuItem::with_id(
                app,
                "quota-5h",
                quota_line(language, "5h", window.used_percent),
                false,
                None::<&str>,
            )?);
            had_quota = true;
        }
        if let Some(window) = &quota.seven_day_quota {
            quota_items.push(MenuItem::with_id(
                app,
                "quota-7d",
                quota_line(language, "7d", window.used_percent),
                false,
                None::<&str>,
            )?);
            had_quota = true;
        }
        if let Some(window) = &quota.monthly_quota {
            quota_items.push(MenuItem::with_id(
                app,
                "quota-monthly",
                quota_line(language, "monthly", window.used_percent),
                false,
                None::<&str>,
            )?);
            had_quota = true;
        }
    }
    if !had_quota {
        quota_items.push(MenuItem::with_id(
            app,
            "quota-unavailable",
            quota_unavailable_label(language),
            false,
            None::<&str>,
        )?);
    }

    let mut items: Vec<&dyn IsMenuItem<tauri::Wry>> = Vec::new();
    for item in &quota_items {
        items.push(item);
    }
    items.push(&open_i);
    items.push(&settings_i);
    items.push(&refresh_i);
    items.push(&separator);
    items.push(&quit_i);

    let menu = Menu::with_items(app, &items)?;

    if let Some(state) = app.try_state::<TrayMenu>() {
        *state.open.lock().unwrap() = open_i;
        *state.settings.lock().unwrap() = settings_i;
        *state.refresh.lock().unwrap() = refresh_i;
        *state.quit.lock().unwrap() = quit_i;
    }
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu))?;
    }

    Ok(())
}

pub fn update_labels(app: &AppHandle, language: ResolvedLanguage) {
    let labels = labels_for(language);
    if let Some(menu) = app.try_state::<TrayMenu>() {
        let _ = menu.open.lock().unwrap().set_text(labels.open);
        let _ = menu.settings.lock().unwrap().set_text(labels.settings);
        let _ = menu.refresh.lock().unwrap().set_text(labels.refresh);
        let _ = menu.quit.lock().unwrap().set_text(labels.quit);
    }
}

fn labels_for(language: ResolvedLanguage) -> TrayLabels {
    match language {
        ResolvedLanguage::ZhHans => TrayLabels {
            open: "打开仪表盘",
            settings: "设置",
            refresh: "刷新",
            quit: "退出",
        },
        ResolvedLanguage::En => TrayLabels {
            open: "Open Dashboard",
            settings: "Settings",
            refresh: "Refresh",
            quit: "Quit",
        },
    }
}

fn quota_unavailable_label(language: ResolvedLanguage) -> &'static str {
    match language {
        ResolvedLanguage::ZhHans => "额度: --",
        ResolvedLanguage::En => "Quota: --",
    }
}

fn quota_line(language: ResolvedLanguage, key: &str, used_percent: f64) -> String {
    let label = match (language, key) {
        (ResolvedLanguage::ZhHans, "monthly") => "月度",
        (ResolvedLanguage::En, "monthly") => "Monthly",
        (_, other) => other,
    };
    format!("{}: {:.0}%", label, used_percent)
}

pub(crate) fn show_main_window_or_log(app: &AppHandle) {
    if let Err(error) = show_main_window(app) {
        warn!(error = %error, "Could not show main window from tray action");
    }
}

pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .show()
            .map_err(|e| format!("Failed to show main window: {}", e))?;
        window
            .set_focus()
            .map_err(|e| format!("Failed to focus main window: {}", e))?;
    } else {
        let window = tauri::WebviewWindowBuilder::from_config(
            app,
            &app.config()
                .app
                .windows
                .first()
                .cloned()
                .unwrap_or_default(),
        )
        .map_err(|e| format!("Failed to create main window builder: {}", e))?
        .build()
        .map_err(|e| format!("Failed to build main window: {}", e))?;
        window
            .show()
            .map_err(|e| format!("Failed to show rebuilt main window: {}", e))?;
        window
            .set_focus()
            .map_err(|e| format!("Failed to focus rebuilt main window: {}", e))?;
    }
    Ok(())
}

pub fn hide_to_tray(window: &tauri::WebviewWindow) {
    let _ = window.hide();
}
