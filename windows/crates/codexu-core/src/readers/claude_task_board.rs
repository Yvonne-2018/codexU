//! Claude Code task-board reader.
//!
//! Mirrors the Swift `ClaudeCodeTaskReader` logic: reads task JSON files under
//! `~/.claude/tasks/**` (created by `claude task`) and classifies them into the
//! four stable board columns by explicit status. Only task metadata is read;
//! prompts, replies, and tool arguments stay local.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};

use crate::models::{TaskBoard, TaskColumn, TaskItem};

/// Reads only Claude task records from `~/.claude/tasks/**/*.json`.
pub struct ClaudeTaskBoardReader {
    tasks_root: PathBuf,
}

impl ClaudeTaskBoardReader {
    pub fn new(tasks_root: impl AsRef<Path>) -> Self {
        Self {
            tasks_root: tasks_root.as_ref().to_path_buf(),
        }
    }

    /// Returns `None` when no trustworthy Claude task source is available.
    pub async fn load(&self, now: DateTime<Utc>) -> Result<Option<TaskBoard>> {
        let tasks_root = self.tasks_root.clone();
        tokio::task::spawn_blocking(move || load_claude_task_board(&tasks_root, now)).await?
    }
}

#[derive(Debug)]
struct ClaudeTaskRecord {
    id: String,
    title: String,
    detail: String,
    updated_at: Option<DateTime<Utc>>,
    raw_status: Option<String>,
    kind: String,
    display_state: String,
    runtime_state: String,
}

fn load_claude_task_board(root: &Path, now: DateTime<Utc>) -> Result<Option<TaskBoard>> {
    let mut files = Vec::new();
    find_task_json_files(root, &mut files);
    let records: Vec<ClaudeTaskRecord> = files
        .iter()
        .filter_map(|path| read_task_file(path))
        .collect();

    if records.is_empty() {
        return Ok(None);
    }

    let mut active = Vec::new();
    let mut pending = Vec::new();
    let mut scheduled = Vec::new();
    let mut done = Vec::new();
    for record in records {
        let item = task_item(record);
        match item.kind.as_str() {
            "active" => active.push(item),
            "scheduled" => scheduled.push(item),
            "done" => done.push(item),
            _ => pending.push(item),
        }
    }

    sort_task_items(&mut active);
    sort_task_items(&mut pending);
    sort_task_items(&mut scheduled);
    sort_task_items(&mut done);

    Ok(Some(TaskBoard {
        refreshed_at: now,
        columns: vec![
            task_column("active", "Recent activity", active),
            task_column("pending", "To continue", pending),
            task_column("scheduled", "Scheduled", scheduled),
            task_column("done", "Archived today", done),
        ],
    }))
}

fn find_task_json_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
            files.push(path);
        } else if path.is_dir() {
            find_task_json_files(&path, files);
        }
    }
}

fn read_task_file(path: &Path) -> Option<ClaudeTaskRecord> {
    let text = fs::read_to_string(path).ok()?;
    let object = serde_json::from_str::<serde_json::Value>(&text)
        .ok()?
        .as_object()?
        .clone();

    let raw_status = string_value(object.get("status"));
    let (kind, display_state, runtime_state) = classify(raw_status.as_deref());
    let title = string_value(object.get("subject"))
        .or_else(|| string_value(object.get("title")))
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_default()
        });
    let updated_at = date_value(object.get("updatedAt"))
        .or_else(|| date_value(object.get("updated_at")))
        .or_else(|| {
            fs::metadata(path)
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .and_then(|duration| Utc.timestamp_opt(duration.as_secs() as i64, 0).single())
        });
    let detail = ["project", "projectPath", "project_path", "cwd"]
        .iter()
        .find_map(|key| string_value(object.get(*key)))
        .map(|value| short_workspace_name(&value))
        .unwrap_or_default();

    Some(ClaudeTaskRecord {
        id: path.to_string_lossy().to_string(),
        title,
        detail,
        updated_at,
        raw_status,
        kind: kind.to_string(),
        display_state: display_state.to_string(),
        runtime_state: runtime_state.to_string(),
    })
}

/// Maps an explicit Claude task status to column kind and display state,
/// mirroring `TaskSourceClassifier.claudeTask` on macOS.
fn classify(raw_status: Option<&str>) -> (&'static str, &'static str, &'static str) {
    match raw_status
        .map(|status| status.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("in_progress" | "active" | "running") => ("active", "recentlyActive", "running"),
        Some("pending") => ("pending", "continueLater", "recorded"),
        Some("failed" | "error") => ("pending", "continueLater", "failed"),
        Some("blocked") => ("pending", "continueLater", "blocked"),
        Some("scheduled") => ("scheduled", "scheduled", "recorded"),
        Some("completed" | "done" | "success") => ("done", "archived", "completed"),
        _ => ("pending", "continueLater", "recorded"),
    }
}

fn task_item(record: ClaudeTaskRecord) -> TaskItem {
    TaskItem {
        id: opaque_task_id(&record.kind, &record.id),
        code: "CLAUDE-TASK".to_string(),
        title: record.title,
        detail: record.detail,
        chip: record.display_state.clone(),
        updated_at: record.updated_at,
        tokens: None,
        kind: record.kind,
        thread_id: None,
        runtime_state: record.runtime_state,
        source_kind: "claudeTask".to_string(),
        display_state: record.display_state,
        state_basis: "explicit".to_string(),
        raw_status: record.raw_status,
        next_run_at: None,
    }
}

fn task_column(id: &str, title: &str, items: Vec<TaskItem>) -> TaskColumn {
    TaskColumn {
        id: id.to_string(),
        title: title.to_string(),
        count: items.len() as i64,
        items,
    }
}

fn sort_task_items(items: &mut [TaskItem]) {
    items.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn string_value(value: Option<&serde_json::Value>) -> Option<String> {
    value.and_then(|v| {
        if let Some(s) = v.as_str() {
            if !s.is_empty() {
                Some(s.to_string())
            } else {
                None
            }
        } else {
            v.as_number().map(|n| n.to_string())
        }
    })
}

fn date_value(value: Option<&serde_json::Value>) -> Option<DateTime<Utc>> {
    value.and_then(|v| {
        if let Some(n) = v.as_f64() {
            let seconds = if n > 10_000_000_000.0 { n / 1000.0 } else { n };
            Utc.timestamp_opt(seconds as i64, 0).single()
        } else if let Some(s) = v.as_str() {
            s.parse::<DateTime<Utc>>().ok()
        } else {
            None
        }
    })
}

fn short_workspace_name(path: &str) -> String {
    path.trim()
        .rsplit(['\\', '/'])
        .find(|part| !part.is_empty())
        .unwrap_or("")
        .to_string()
}

fn opaque_task_id(kind: &str, source_id: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    kind.hash(&mut hasher);
    source_id.hash(&mut hasher);
    format!("{kind}-{:016x}", hasher.finish())
}
