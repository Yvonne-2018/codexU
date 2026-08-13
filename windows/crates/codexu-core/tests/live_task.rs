use chrono::{TimeZone, Utc};
use codexu_core::models::{TaskBoard, TaskColumn, TaskItem};
use codexu_core::readers::{
    merge_live_tasks, parse_thread_list_response, LiveTaskRecord, LiveTaskSnapshot,
};
use serde_json::json;

fn thread_item(
    id: &str,
    title: &str,
    kind: &str,
    display_state: &str,
    state_basis: &str,
    thread_id: Option<&str>,
    updated_at: chrono::DateTime<Utc>,
) -> TaskItem {
    TaskItem {
        id: format!("{id}-task"),
        code: "LOCAL-THREAD".to_string(),
        title: title.to_string(),
        detail: "Detail".to_string(),
        chip: display_state.to_string(),
        updated_at: Some(updated_at),
        tokens: None,
        kind: kind.to_string(),
        thread_id: thread_id.map(str::to_owned),
        runtime_state: "recorded".to_string(),
        source_kind: "codexThread".to_string(),
        display_state: display_state.to_string(),
        state_basis: state_basis.to_string(),
        raw_status: None,
        next_run_at: None,
    }
}

fn scheduled_item(id: &str, title: &str) -> TaskItem {
    let mut item = thread_item(
        id,
        title,
        "scheduled",
        "scheduled",
        "scheduleConfig",
        None,
        Utc::now(),
    );
    item.updated_at = None;
    item.source_kind = "codexAutomation".to_string();
    item
}

fn column(id: &str, items: Vec<TaskItem>) -> TaskColumn {
    TaskColumn {
        id: id.to_string(),
        title: id.to_string(),
        count: items.len() as i64,
        items,
    }
}

fn live_record(
    thread_id: &str,
    state: &str,
    name: Option<&str>,
    updated_at: Option<chrono::DateTime<Utc>>,
) -> LiveTaskRecord {
    LiveTaskRecord {
        thread_id: thread_id.to_string(),
        state: state.to_string(),
        updated_at,
        name: name.map(str::to_owned),
    }
}

fn board_column<'a>(board: &'a TaskBoard, id: &str) -> &'a TaskColumn {
    board
        .columns
        .iter()
        .find(|column| column.id == id)
        .unwrap_or_else(|| panic!("missing {id} column"))
}

fn thread_ids(column: &TaskColumn) -> Vec<Option<&str>> {
    column
        .items
        .iter()
        .map(|item| item.thread_id.as_deref())
        .collect()
}

#[test]
fn merges_live_tasks_into_the_static_board() {
    let now = Utc.with_ymd_and_hms(2026, 8, 13, 12, 0, 0).unwrap();
    let t = |hour: u32, minute: u32| Utc.with_ymd_and_hms(2026, 8, 13, hour, minute, 0).unwrap();
    let yesterday = Utc.with_ymd_and_hms(2026, 8, 12, 11, 0, 0).unwrap();

    let board = TaskBoard {
        refreshed_at: now,
        columns: vec![
            column(
                "active",
                vec![thread_item(
                    "t-active",
                    "Active work",
                    "active",
                    "recentlyActive",
                    "activityWindow",
                    Some("t-active"),
                    t(10, 0),
                )],
            ),
            column(
                "pending",
                vec![thread_item(
                    "t-pending",
                    "Pending work",
                    "pending",
                    "continueLater",
                    "activityWindow",
                    Some("t-pending"),
                    t(9, 0),
                )],
            ),
            column("scheduled", vec![scheduled_item("auto", "Daily check")]),
            column(
                "done",
                vec![thread_item(
                    "t-done",
                    "Done work",
                    "done",
                    "archived",
                    "archive",
                    Some("t-done"),
                    t(8, 0),
                )],
            ),
        ],
    };

    let live = LiveTaskSnapshot {
        refreshed_at: now,
        records: vec![
            // Existing cards are updated regardless of age.
            live_record("t-active", "idle", None, Some(t(11, 30))),
            live_record("t-pending", "running", None, Some(t(11, 0))),
            live_record("t-done", "failed", None, Some(t(10, 30))),
            // New today card enters the board.
            live_record(
                "new-today",
                "running",
                Some("New live task"),
                Some(t(11, 45)),
            ),
            // New nil-updated_at card is treated as today.
            live_record("new-now", "waitingInput", None, None),
            // New card from a previous day is not added.
            live_record("new-yesterday", "idle", Some("Old thread"), Some(yesterday)),
        ],
    };

    let merged = merge_live_tasks(&board, &live, now);

    assert_eq!(merged.refreshed_at, now);

    let active = board_column(&merged, "active");
    assert_eq!(
        thread_ids(active),
        vec![Some("new-today"), Some("t-pending"), Some("new-now")]
    );

    let pending = board_column(&merged, "pending");
    assert_eq!(thread_ids(pending), vec![Some("t-active"), Some("t-done")]);

    let done = board_column(&merged, "done");
    assert!(done.items.is_empty());

    let scheduled = board_column(&merged, "scheduled");
    assert_eq!(scheduled.items.len(), 1);
    assert_eq!(scheduled.items[0].title, "Daily check");
    assert_eq!(scheduled.items[0].kind, "scheduled");

    // Existing card preserved title/detail but took the live state.
    let t_active = &pending.items[0];
    assert_eq!(t_active.title, "Active work");
    assert_eq!(t_active.runtime_state, "idle");
    assert_eq!(t_active.display_state, "continueLater");
    assert_eq!(t_active.updated_at, Some(t(11, 30)));

    let t_pending = active
        .items
        .iter()
        .find(|item| item.thread_id.as_deref() == Some("t-pending"))
        .unwrap();
    assert_eq!(t_pending.title, "Pending work");
    assert_eq!(t_pending.runtime_state, "running");
    assert_eq!(t_pending.display_state, "recentlyActive");
    assert_eq!(t_pending.state_basis, "activityWindow");

    let t_done = &pending.items[1];
    assert_eq!(t_done.title, "Done work");
    assert_eq!(t_done.runtime_state, "failed");
    assert_eq!(t_done.display_state, "continueLater");

    // New cards use the live name (or a fallback) and a live code prefix.
    let new_today = active
        .items
        .iter()
        .find(|item| item.thread_id.as_deref() == Some("new-today"))
        .unwrap();
    assert_eq!(new_today.title, "New live task");
    assert_eq!(new_today.runtime_state, "running");
    assert_eq!(new_today.display_state, "recentlyActive");
    assert!(new_today.id.starts_with("live-new-today"));
    assert!(new_today.code.starts_with("COD-"));

    let new_now = active
        .items
        .iter()
        .find(|item| item.thread_id.as_deref() == Some("new-now"))
        .unwrap();
    assert_eq!(new_now.title, "Codex task");
    assert_eq!(new_now.runtime_state, "waitingInput");
    assert!(new_now.updated_at.is_none());

    // The previous-day thread was never added.
    let all_thread_ids: Vec<Option<&str>> = merged.columns.iter().flat_map(thread_ids).collect();
    assert!(!all_thread_ids.contains(&Some("new-yesterday")));
}

fn t(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .unwrap()
}

#[test]
fn parses_thread_list_response_with_normalized_states() {
    let response = json!({
        "threads": [
            {
                "id": "a",
                "name": "Running task",
                "status": {"type": "active", "activeFlags": []},
                "updatedAt": 1_720_000_000
            },
            {"id": "b", "status": {"type": "active", "activeFlags": ["waitingOnUserInput"]}},
            {"id": "c", "status": {"type": "idle"}},
            {"id": "d", "status": {"type": "systemError"}, "updatedAt": 1_720_000_001.0},
            {"id": "e", "status": {"type": "unknown"}},
            {"id": "f"},
            {"id": "g", "threadSource": "subagent", "status": {"type": "active"}},
            {"id": "h", "source": "subagent", "status": {"type": "idle"}},
            {"id": "i", "source": {"subagent": true}, "status": {"type": "idle"}}
        ]
    });

    let records = parse_thread_list_response(&response);

    assert_eq!(records.len(), 6);
    let by_id = |id: &str| {
        records
            .iter()
            .find(|record| record.thread_id == id)
            .unwrap_or_else(|| panic!("missing thread {id}"))
    };

    assert_eq!(by_id("a").state, "running");
    assert_eq!(by_id("a").name.as_deref(), Some("Running task"));
    assert_eq!(by_id("a").updated_at, Some(t(2024, 7, 3, 9, 46, 40)));
    assert_eq!(by_id("b").state, "waitingInput");
    assert_eq!(by_id("c").state, "idle");
    assert_eq!(by_id("d").state, "failed");
    assert_eq!(by_id("d").updated_at, Some(t(2024, 7, 3, 9, 46, 41)));
    assert_eq!(by_id("e").state, "recorded");
    assert_eq!(by_id("f").state, "recorded");
    assert!(by_id("f").name.is_none());
}

#[test]
fn accepts_thread_list_result_as_bare_array_or_data_key() {
    let records = parse_thread_list_response(&json!([
        {"id": "x", "status": {"type": "idle"}},
        {"id": "y", "status": {"type": "active"}}
    ]));
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].thread_id, "x");

    let records = parse_thread_list_response(&json!({
        "data": [{"id": "x", "status": {"type": "active"}}]
    }));
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].thread_id, "x");
}
