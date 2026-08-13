use chrono::{TimeZone, Utc};
use codexu_core::readers::ClaudeTaskBoardReader;

fn write_task(root: &std::path::Path, folder: &str, name: &str, contents: &str) {
    let dir = root.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), contents).unwrap();
}

fn column<'a>(
    board: &'a codexu_core::models::TaskBoard,
    id: &str,
) -> &'a codexu_core::models::TaskColumn {
    board
        .columns
        .iter()
        .find(|column| column.id == id)
        .unwrap_or_else(|| panic!("missing {id} column"))
}

#[tokio::test]
async fn classifies_claude_tasks_into_stable_columns() {
    let root = tempfile::tempdir().unwrap();
    let now = Utc.with_ymd_and_hms(2026, 8, 13, 12, 0, 0).unwrap();

    write_task(
        root.path(),
        "session-a",
        "1.json",
        r#"{"id":"1","subject":"In progress work","status":"in_progress","blocks":[],"blockedBy":[]}"#,
    );
    write_task(
        root.path(),
        "session-a",
        "2.json",
        r#"{"id":"2","subject":"Waiting work","status":"pending","blocks":[],"blockedBy":[]}"#,
    );
    write_task(
        root.path(),
        "session-b",
        "1.json",
        r#"{"id":"1","subject":"Failed work","status":"error","blocks":[],"blockedBy":[]}"#,
    );
    write_task(
        root.path(),
        "session-b",
        "2.json",
        r#"{"id":"2","subject":"Finished work","status":"completed","blocks":[],"blockedBy":[]}"#,
    );
    write_task(
        root.path(),
        "session-c",
        "1.json",
        r#"{"id":"1","subject":"Planned work","status":"scheduled","blocks":[],"blockedBy":[]}"#,
    );

    // Non-JSON state files must be ignored.
    write_task(root.path(), "session-a", ".lock", "lock");
    write_task(root.path(), "session-a", ".highwatermark", "0");

    let board = ClaudeTaskBoardReader::new(root.path())
        .load(now)
        .await
        .unwrap()
        .expect("tasks root should produce a board");

    let active = column(&board, "active");
    assert_eq!(active.count, 1);
    assert_eq!(active.items[0].title, "In progress work");
    assert_eq!(active.items[0].display_state, "recentlyActive");
    assert_eq!(active.items[0].runtime_state, "running");
    assert_eq!(active.items[0].source_kind, "claudeTask");
    assert_eq!(active.items[0].state_basis, "explicit");

    let pending = column(&board, "pending");
    assert_eq!(pending.count, 2);
    let pending_titles: Vec<&str> = pending
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert!(pending_titles.contains(&"Waiting work"));
    assert!(pending_titles.contains(&"Failed work"));

    let scheduled = column(&board, "scheduled");
    assert_eq!(scheduled.count, 1);
    assert_eq!(scheduled.items[0].title, "Planned work");
    assert_eq!(scheduled.items[0].display_state, "scheduled");

    let done = column(&board, "done");
    assert_eq!(done.count, 1);
    assert_eq!(done.items[0].title, "Finished work");
    assert_eq!(done.items[0].runtime_state, "completed");
}

#[tokio::test]
async fn returns_none_when_no_task_json_files_exist() {
    let root = tempfile::tempdir().unwrap();
    let now = Utc.with_ymd_and_hms(2026, 8, 13, 12, 0, 0).unwrap();

    let board = ClaudeTaskBoardReader::new(root.path())
        .load(now)
        .await
        .unwrap();
    assert!(board.is_none());
}

#[tokio::test]
async fn falls_back_to_file_stem_title_and_keeps_task_metadata_private() {
    let root = tempfile::tempdir().unwrap();
    let now = Utc.with_ymd_and_hms(2026, 8, 13, 12, 0, 0).unwrap();

    write_task(
        root.path(),
        "session-a",
        "1.json",
        r#"{"id":"1","status":"completed","cwd":"C:\\Users\\private-user\\work","blocks":[],"blockedBy":[]}"#,
    );

    let board = ClaudeTaskBoardReader::new(root.path())
        .load(now)
        .await
        .unwrap()
        .expect("tasks root should produce a board");

    let done = column(&board, "done");
    assert_eq!(done.count, 1);
    assert_eq!(done.items[0].title, "1");
    assert_eq!(done.items[0].detail, "work");
    assert_eq!(done.items[0].raw_status.as_deref(), Some("completed"));

    let serialized = serde_json::to_string(&board).unwrap();
    assert!(!serialized.contains("C:\\Users\\private-user\\work"));
    assert!(!serialized.contains("session-a"));
}
