//! Live/runtime Codex task reader.
//!
//! Queries a short-lived local Codex app-server for the current thread list
//! and merges the resulting runtime state into the static task board so the
//! dashboard can reflect tasks that are running right now.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Datelike, Local, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::timeout;

use crate::models::{TaskBoard, TaskColumn, TaskItem};
use crate::readers::codex_app_server::{
    connect_loopback_transport, launch_app_server, reserve_loopback_port, stop_child,
    AppServerTransport,
};

const LIVE_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

/// A single live task record read from the Codex app-server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveTaskRecord {
    pub thread_id: String,
    /// Normalized runtime state: running / waitingInput / idle / failed / recorded.
    pub state: String,
    #[serde(with = "chrono::serde::ts_milliseconds_option")]
    pub updated_at: Option<DateTime<Utc>>,
    pub name: Option<String>,
}

/// A snapshot of the live Codex tasks at one refresh.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveTaskSnapshot {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub refreshed_at: DateTime<Utc>,
    pub records: Vec<LiveTaskRecord>,
}

/// Reads the live Codex thread list through a short-lived app-server.
pub struct CodexLiveTaskReader {
    codex_root: PathBuf,
}

impl CodexLiveTaskReader {
    pub fn new(codex_root: impl AsRef<Path>) -> Self {
        Self {
            codex_root: codex_root.as_ref().to_path_buf(),
        }
    }

    /// Loads the current live task snapshot.
    ///
    /// Returns `Ok(None)` when the app-server cannot be reached or any request
    /// fails or times out, so callers can fall back to the static task board.
    pub async fn load(&self, now: DateTime<Utc>) -> anyhow::Result<Option<LiveTaskSnapshot>> {
        if !self.codex_root.is_dir() {
            return Ok(None);
        }
        let port = match reserve_loopback_port().await {
            Ok(port) => port,
            Err(_) => return Ok(None),
        };
        let mut child = match launch_app_server(port) {
            Ok(child) => child,
            Err(_) => return Ok(None),
        };
        let endpoint = format!("ws://127.0.0.1:{port}");

        let result = timeout(LIVE_REQUEST_TIMEOUT, async {
            let mut transport = connect_loopback_transport(&endpoint).await?;
            read_live_tasks_from_transport(&mut transport, now).await
        })
        .await;

        stop_child(&mut child).await;

        match result {
            Ok(Ok(snapshot)) => Ok(Some(snapshot)),
            _ => Ok(None),
        }
    }
}

/// Performs the live thread-list request sequence after a transport has
/// connected to a local Codex app-server.
async fn read_live_tasks_from_transport<T: AppServerTransport>(
    transport: &mut T,
    now: DateTime<Utc>,
) -> anyhow::Result<LiveTaskSnapshot> {
    request_result(
        transport,
        serde_json::json!({
            "id": 1,
            "method": "initialize",
            "params": {
                "clientInfo": {
                    "name": "codexu",
                    "title": "codexU",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": true,
                    "optOutNotificationMethods": []
                }
            }
        }),
    )
    .await?;
    transport
        .notify(serde_json::json!({ "method": "initialized" }))
        .await?;

    let result = request_result(
        transport,
        serde_json::json!({ "id": 2, "method": "thread/list" }),
    )
    .await?;

    Ok(LiveTaskSnapshot {
        refreshed_at: now,
        records: parse_thread_list_response(&result),
    })
}

async fn request_result<T: AppServerTransport>(
    transport: &mut T,
    request: Value,
) -> anyhow::Result<Value> {
    let response = transport.request(request).await?;
    if response.get("error").is_some() {
        anyhow::bail!("Codex app-server rejected a live task request")
    }
    response.get("result").cloned().ok_or_else(|| {
        anyhow::anyhow!("Codex app-server returned no result for a live task request")
    })
}

/// Parses a `thread/list` result into normalized live task records.
///
/// Accepts `{"threads": [...]}`, `{"data": [...]}`, or a bare array.
/// Subagent threads are skipped, mirroring the macOS `TaskThreadVisibility`.
pub fn parse_thread_list_response(result: &Value) -> Vec<LiveTaskRecord> {
    let threads: &[Value] = match result {
        Value::Array(threads) => threads.as_slice(),
        Value::Object(object) => {
            let Some(array) = object
                .get("threads")
                .or_else(|| object.get("data"))
                .and_then(Value::as_array)
            else {
                return Vec::new();
            };
            array.as_slice()
        }
        _ => return Vec::new(),
    };

    threads
        .iter()
        .filter(|thread| !is_subagent(thread))
        .filter_map(parse_live_record)
        .collect()
}

fn parse_live_record(thread: &Value) -> Option<LiveTaskRecord> {
    let thread_id = thread.get("id")?.as_str()?.to_owned();
    let name = thread
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let state = normalize_state(thread.get("status"));
    let updated_at = thread.get("updatedAt").and_then(parse_epoch_seconds);
    Some(LiveTaskRecord {
        thread_id,
        state,
        updated_at,
        name,
    })
}

/// Normalizes a Codex thread `status` object into a compact runtime state.
fn normalize_state(status: Option<&Value>) -> String {
    let Some(status) = status.filter(|value| value.is_object()) else {
        return "recorded".to_string();
    };
    let waiting_on_input = status
        .get("activeFlags")
        .and_then(Value::as_array)
        .map(|flags| flags.iter().filter_map(Value::as_str))
        .is_some_and(|mut flags| flags.any(|flag| flag == "waitingOnUserInput"));

    match status.get("type").and_then(Value::as_str) {
        Some("active") if waiting_on_input => "waitingInput".to_string(),
        Some("active") => "running".to_string(),
        Some("idle") => "idle".to_string(),
        Some("systemError") => "failed".to_string(),
        _ => "recorded".to_string(),
    }
}

fn parse_epoch_seconds(value: &Value) -> Option<DateTime<Utc>> {
    value
        .as_i64()
        .or_else(|| value.as_f64().map(|seconds| seconds as i64))
        .and_then(|seconds| Utc.timestamp_opt(seconds, 0).single())
}

/// Mirrors macOS `TaskThreadVisibility.isSubagent`.
fn is_subagent(thread: &Value) -> bool {
    let direct = thread
        .get("threadSource")
        .or_else(|| thread.get("thread_source"))
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase);
    if direct.as_deref() == Some("subagent") {
        return true;
    }
    match thread.get("source") {
        Some(Value::String(source)) => source.to_ascii_lowercase() == "subagent",
        Some(Value::Object(source)) => source
            .keys()
            .any(|key| key.to_ascii_lowercase() == "subagent"),
        _ => false,
    }
}

/// Merges live task records into a static task board.
///
/// Existing cards with the same `thread_id` are updated in place (preserving
/// their title and detail); new cards are only added when their `updated_at`
/// falls on the same local day as `now`, mirroring the macOS board. The
/// `scheduled` column is preserved unchanged.
pub fn merge_live_tasks(
    board: &TaskBoard,
    live: &LiveTaskSnapshot,
    now: DateTime<Utc>,
) -> TaskBoard {
    let mut thread_items: HashMap<String, TaskItem> = HashMap::new();
    let mut scheduled = Vec::new();

    for column in &board.columns {
        for item in &column.items {
            if item.kind == "scheduled" || item.thread_id.is_none() {
                scheduled.push(item.clone());
            } else if let Some(thread_id) = &item.thread_id {
                thread_items.insert(thread_id.clone(), item.clone());
            }
        }
    }

    for record in &live.records {
        if let Some(item) = thread_items.get_mut(&record.thread_id) {
            *item = apply_live_record(item.clone(), record);
        } else if is_same_local_day(record.updated_at.unwrap_or(now), now) {
            thread_items.insert(record.thread_id.clone(), new_live_item(record));
        }
    }

    let mut active = Vec::new();
    let mut pending = Vec::new();
    let mut done = Vec::new();
    for item in thread_items.values() {
        match item.kind.as_str() {
            "active" => active.push(item.clone()),
            "done" => done.push(item.clone()),
            _ => pending.push(item.clone()),
        }
    }
    sort_task_items(&mut active);
    sort_task_items(&mut pending);
    sort_task_items(&mut done);

    TaskBoard {
        refreshed_at: now,
        columns: vec![
            task_column(
                "active",
                column_title(board, "active", "Recent activity"),
                active,
            ),
            task_column(
                "pending",
                column_title(board, "pending", "To continue"),
                pending,
            ),
            task_column(
                "scheduled",
                column_title(board, "scheduled", "Scheduled"),
                scheduled,
            ),
            task_column("done", column_title(board, "done", "Archived today"), done),
        ],
    }
}

/// Maps a normalized live state to its target column and display state.
fn classify_state(state: &str) -> (&'static str, &'static str) {
    match state {
        "running" | "waitingInput" => ("active", "recentlyActive"),
        "idle" => ("pending", "continueLater"),
        "failed" => ("pending", "continueLater"),
        "completed" => ("done", "archived"),
        _ => ("pending", "continueLater"),
    }
}

fn apply_live_record(mut item: TaskItem, record: &LiveTaskRecord) -> TaskItem {
    let (kind, display_state) = classify_state(&record.state);
    item.kind = kind.to_string();
    item.display_state = display_state.to_string();
    item.runtime_state = record.state.clone();
    item.state_basis = "activityWindow".to_string();
    item.updated_at = record.updated_at.or(item.updated_at);
    item.thread_id = Some(record.thread_id.clone());
    item
}

fn new_live_item(record: &LiveTaskRecord) -> TaskItem {
    let (kind, display_state) = classify_state(&record.state);
    let compact = record.thread_id.replace('-', "");
    let code = compact
        .get(compact.len().saturating_sub(4)..)
        .unwrap_or(&compact)
        .to_ascii_uppercase();
    TaskItem {
        id: format!("live-{}", record.thread_id),
        code: format!("COD-{code}"),
        title: record
            .name
            .clone()
            .unwrap_or_else(|| "Codex task".to_string()),
        detail: String::new(),
        chip: "Live".to_string(),
        updated_at: record.updated_at,
        tokens: None,
        kind: kind.to_string(),
        thread_id: Some(record.thread_id.clone()),
        runtime_state: record.state.clone(),
        source_kind: "codexThread".to_string(),
        display_state: display_state.to_string(),
        state_basis: "activityWindow".to_string(),
        raw_status: None,
        next_run_at: None,
    }
}

fn is_same_local_day(a: DateTime<Utc>, b: DateTime<Utc>) -> bool {
    let a = a.with_timezone(&Local);
    let b = b.with_timezone(&Local);
    a.year() == b.year() && a.month() == b.month() && a.day() == b.day()
}

fn sort_task_items(items: &mut [TaskItem]) {
    items.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn task_column(id: &str, title: String, items: Vec<TaskItem>) -> TaskColumn {
    TaskColumn {
        id: id.to_string(),
        title,
        count: items.len() as i64,
        items,
    }
}

fn column_title(board: &TaskBoard, id: &str, fallback: &str) -> String {
    board
        .columns
        .iter()
        .find(|column| column.id == id)
        .map(|column| column.title.clone())
        .unwrap_or_else(|| fallback.to_string())
}
