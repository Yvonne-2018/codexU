use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::app_state::{
    AppConfig, AppState, InterfaceLanguage, ResolvedLanguage, ThemeMode, TrayDensity,
};

#[derive(Debug, serde::Serialize)]
pub struct SettingsDto {
    #[serde(flatten)]
    pub config: AppConfig,
    pub app_data_dir: PathBuf,
}

#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    let app_state = app.state::<std::sync::Arc<AppState>>();
    let runtime_language = *app_state.runtime_language.read().await;

    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_title(settings_window_title(runtime_language));
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    let window =
        tauri::WebviewWindowBuilder::new(&app, "settings", tauri::WebviewUrl::App("/".into()))
            .title(settings_window_title(runtime_language))
            .inner_size(540.0, 680.0)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .center()
            .build()
            .map_err(|e| format!("Failed to create settings window: {}", e))?;

    let theme = {
        let config = app_state.config.read().await;
        config.theme
    };
    apply_theme(&app, theme);
    let _ = window.show();
    let _ = window.set_focus();
    Ok(())
}

#[tauri::command]
pub async fn get_settings(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<SettingsDto, String> {
    let config = state.config.read().await.clone();
    Ok(SettingsDto {
        config,
        app_data_dir: state.app_data_dir.clone(),
    })
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateSettingsRequest {
    pub codex_root: Option<PathBuf>,
    pub cache_dir: Option<PathBuf>,
    pub theme: Option<ThemeMode>,
    pub palette_id: Option<String>,
    pub refresh_interval_secs: Option<u64>,
    pub tray_density: Option<TrayDensity>,
    pub language: Option<InterfaceLanguage>,
    pub query_codex_official_quota: Option<bool>,
    pub query_claude_official_quota: Option<bool>,
}

#[tauri::command]
pub async fn set_settings(
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
    req: UpdateSettingsRequest,
) -> Result<AppConfig, String> {
    let config = state
        .update_config(|config| {
            if let Some(path) = req.codex_root {
                config.codex_root = path;
            }
            if let Some(path) = req.cache_dir {
                config.cache_dir = path;
            }
            if let Some(theme) = req.theme {
                config.theme = theme;
            }
            if let Some(palette_id) = req.palette_id {
                let palette_id = palette_id.trim();
                if !palette_id.is_empty() {
                    config.palette_id = palette_id.to_string();
                }
            }
            if let Some(interval) = req.refresh_interval_secs {
                config.refresh_interval_secs = interval.clamp(10, 3600);
            }
            if let Some(density) = req.tray_density {
                config.tray_density = density;
            }
            if let Some(language) = req.language {
                config.language = language;
            }
            if let Some(value) = req.query_codex_official_quota {
                config.query_codex_official_quota = value;
            }
            if let Some(value) = req.query_claude_official_quota {
                config.query_claude_official_quota = value;
            }
        })
        .await
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    apply_theme(&app, config.theme);
    if config.language != InterfaceLanguage::Auto {
        let language = config.language.resolved(ResolvedLanguage::En);
        state.inner().set_runtime_language(language).await;
        apply_language(&app, language);
    }
    let _ = app.emit("settings:changed", config.clone());
    Ok(config)
}

#[tauri::command]
pub async fn sync_runtime_language(
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
    language: ResolvedLanguage,
) -> Result<(), String> {
    state.inner().set_runtime_language(language).await;
    apply_language(&app, language);
    Ok(())
}

pub fn apply_language(app: &AppHandle, language: ResolvedLanguage) {
    crate::tray::update_labels(app, language);
    update_window_titles(app, language);
}

pub fn update_window_titles(app: &AppHandle, language: ResolvedLanguage) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_title(settings_window_title(language));
    }
}

fn settings_window_title(language: ResolvedLanguage) -> &'static str {
    match language {
        ResolvedLanguage::ZhHans => "设置 — codexU",
        ResolvedLanguage::En => "Settings — codexU",
    }
}

#[tauri::command]
pub async fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    if enabled {
        autostart
            .enable()
            .map_err(|e| format!("Failed to enable autostart: {}", e))
    } else {
        autostart
            .disable()
            .map_err(|e| format!("Failed to disable autostart: {}", e))
    }
}

#[tauri::command]
pub async fn get_autostart(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .is_enabled()
        .map_err(|e| format!("Failed to read autostart state: {}", e))
}

#[derive(Debug, serde::Serialize)]
pub struct DiagnosticsReport {
    pub codex_root: String,
    pub codex_root_exists: bool,
    pub state_db_exists: bool,
    pub claude_projects_exists: bool,
    pub claude_tasks_exists: bool,
    pub codex_executable: Option<String>,
    pub codex_quota_read_succeeded: bool,
    pub messages: Vec<String>,
}

#[tauri::command]
pub async fn run_diagnostics(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<DiagnosticsReport, String> {
    let (codex_root, query_codex_official_quota) = {
        let config = state.config.read().await;
        (config.codex_root.clone(), config.query_codex_official_quota)
    };

    let mut messages = Vec::new();

    let codex_root_exists = codex_root.is_dir();
    if !codex_root_exists {
        messages.push(format!(
            "Codex root does not exist: {}",
            codex_root.display()
        ));
    }

    let state_db_path = codex_root.join("state_5.sqlite");
    let state_db_exists = state_db_path.is_file();
    if !state_db_exists {
        messages.push(format!(
            "Codex state DB not found: {}",
            state_db_path.display()
        ));
    }

    let home = dirs::home_dir();
    let claude_projects_exists = home
        .as_ref()
        .map(|home| home.join(".claude").join("projects").is_dir())
        .unwrap_or(false);
    if !claude_projects_exists {
        messages.push("Claude Code projects directory not found (~/.claude/projects)".to_string());
    }

    let claude_tasks_exists = home
        .as_ref()
        .map(|home| home.join(".claude").join("tasks").is_dir())
        .unwrap_or(false);
    if !claude_tasks_exists {
        messages.push("Claude Code tasks directory not found (~/.claude/tasks)".to_string());
    }

    let codex_executable = find_codex_executable().map(|path| path.display().to_string());
    if codex_executable.is_none() {
        messages.push("Could not locate the installed Codex CLI executable".to_string());
    }

    let codex_quota_read_succeeded = if query_codex_official_quota {
        codexu_core::readers::read_installed_codex_quota()
            .await
            .map(|quota| quota.quota_read_succeeded)
            .unwrap_or(false)
    } else {
        messages.push("Official Codex quota query is disabled in settings; skipped".to_string());
        false
    };
    if !codex_quota_read_succeeded && query_codex_official_quota {
        messages.push("Could not read official Codex quota from the local app-server".to_string());
    }

    Ok(DiagnosticsReport {
        codex_root: codex_root.display().to_string(),
        codex_root_exists,
        state_db_exists,
        claude_projects_exists,
        claude_tasks_exists,
        codex_executable,
        codex_quota_read_succeeded,
        messages,
    })
}

/// Mirrors the Codex CLI candidate paths used by codexu-core so the diagnostic
/// report can expose the resolved executable path without coupling to its
/// private resolver.
fn find_codex_executable() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(user_profile) = std::env::var_os("USERPROFILE") {
        let user_dir = PathBuf::from(user_profile);
        candidates.push(
            user_dir
                .join(".codex")
                .join(".sandbox-bin")
                .join("codex.exe"),
        );
        candidates.push(user_dir.join(".local").join("bin").join("codex.exe"));
    }

    if let Some(app_data) = std::env::var_os("APPDATA") {
        let triple = if cfg!(target_arch = "aarch64") {
            "aarch64-pc-windows-msvc"
        } else {
            "x86_64-pc-windows-msvc"
        };
        candidates.push(
            PathBuf::from(app_data)
                .join("npm")
                .join("node_modules")
                .join("@openai")
                .join("codex")
                .join("node_modules")
                .join("@openai")
                .join("codex-win32-x64")
                .join("vendor")
                .join(triple)
                .join("bin")
                .join("codex.exe"),
        );
    }

    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local_app_data)
                .join("Programs")
                .join("codex")
                .join("codex.exe"),
        );
    }

    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            candidates.push(dir.join("codex.exe"));
        }
    }

    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn apply_theme(app: &AppHandle, theme: ThemeMode) {
    let windows = app.webview_windows();
    let dark = match theme {
        ThemeMode::System => {
            // Frontend will detect system preference on load.
            return;
        }
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
    };
    for (_, window) in windows {
        let _ = window.eval(&format!(
            "document.documentElement.classList.remove('dark'); if ({}) document.documentElement.classList.add('dark');",
            dark
        ));
        let _ = window.eval(&format!(
            "window.__CODEXU_THEME__ = '{}'",
            if dark { "dark" } else { "light" }
        ));
    }
}
