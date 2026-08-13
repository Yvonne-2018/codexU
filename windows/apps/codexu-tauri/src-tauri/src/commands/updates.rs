use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;

const UPDATE_CHECK_URL: &str = "https://api.github.com/repos/Yvonne-2018/codexU/releases/latest";
const UPDATE_CHECK_MIN_INTERVAL_SECS: i64 = 300;
const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(15);

/// In-memory timestamp of the last successful update check, used to avoid
/// hammering the GitHub API on every dashboard open.
static LAST_CHECKED_AT: AtomicI64 = AtomicI64::new(0);

#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_url: Option<String>,
    pub checked_at: Option<i64>,
    pub error: Option<String>,
}

/// Checks the latest codexU release on GitHub. Network failures are reported in
/// the `error` field instead of failing the command.
#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let now = unix_now();

    let last_checked = LAST_CHECKED_AT.load(Ordering::SeqCst);
    if last_checked != 0 && now - last_checked < UPDATE_CHECK_MIN_INTERVAL_SECS {
        return Ok(UpdateInfo {
            current_version,
            latest_version: None,
            release_url: None,
            checked_at: Some(last_checked),
            error: None,
        });
    }
    LAST_CHECKED_AT.store(now, Ordering::SeqCst);

    let request_version = current_version.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let user_agent = format!("codexU/{}", request_version);
        ureq::get(UPDATE_CHECK_URL)
            .set("User-Agent", &user_agent)
            .timeout(UPDATE_CHECK_TIMEOUT)
            .call()
            .map_err(|e| format!("Failed to check for updates: {}", e))
            .and_then(|response| {
                response
                    .into_json::<serde_json::Value>()
                    .map_err(|e| format!("Failed to parse update response: {}", e))
            })
    })
    .await
    .map_err(|e| format!("Update check task failed: {}", e))?;

    match result {
        Ok(body) => {
            let latest_version = body
                .get("tag_name")
                .and_then(|value| value.as_str())
                .map(|tag| tag.trim_start_matches('v').to_string());
            let release_url = body
                .get("html_url")
                .and_then(|value| value.as_str())
                .map(str::to_owned);
            Ok(UpdateInfo {
                current_version,
                latest_version,
                release_url,
                checked_at: Some(now),
                error: None,
            })
        }
        Err(error) => Ok(UpdateInfo {
            current_version,
            latest_version: None,
            release_url: None,
            checked_at: Some(now),
            error: Some(error),
        }),
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
