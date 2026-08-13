//! Integration tests for Claude Code skill usage aggregation.

use chrono::{TimeZone, Utc};
use codexu_core::readers::ClaudeCodeTranscriptReader;

fn write_session(path: &std::path::Path, lines: Vec<&str>) {
    std::fs::write(path, lines.join("\n")).unwrap();
}

fn session_line(timestamp: &str, message_id: &str, skill: Option<&str>, total: i64) -> String {
    let skill_field = match skill {
        Some(name) => format!(r#","attributionSkill":"{name}""#),
        None => String::new(),
    };
    format!(
        r#"{{"timestamp":"{ts}","type":"user","message":{{"id":"{id}","model":"claude-sonnet-4-5"{skill},"usage":{{"input_tokens":100,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":20,"reasoning_output_tokens":0,"total_tokens":{tokens}}}}}}}"#,
        ts = timestamp,
        id = message_id,
        skill = skill_field,
        tokens = total,
    )
}

#[tokio::test]
async fn aggregates_claude_skill_loads_across_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let projects = temp.path().join("claude_projects").join("-C-Demo");
    std::fs::create_dir_all(&projects).unwrap();

    // session-a: blueprint x2, review x1
    write_session(
        &projects.join("session-a.jsonl"),
        vec![
            &session_line("2026-07-26T09:00:00.000Z", "a-1", Some("blueprint"), 100),
            &session_line("2026-07-26T10:00:00.000Z", "a-2", Some("blueprint"), 120),
            &session_line("2026-07-27T09:00:00.000Z", "a-3", Some("review"), 90),
        ],
    );
    // session-b: blueprint x1, review x2
    write_session(
        &projects.join("session-b.jsonl"),
        vec![
            &session_line("2026-07-27T11:00:00.000Z", "b-1", Some("blueprint"), 100),
            &session_line("2026-07-28T09:00:00.000Z", "b-2", Some("review"), 80),
            &session_line("2026-07-28T10:00:00.000Z", "b-3", Some("review"), 70),
        ],
    );
    // session-c: review x2
    write_session(
        &projects.join("session-c.jsonl"),
        vec![
            &session_line("2026-07-28T11:00:00.000Z", "c-1", Some("review"), 60),
            &session_line("2026-07-28T12:00:00.000Z", "c-2", Some("review"), 50),
        ],
    );

    let now = Utc.with_ymd_and_hms(2026, 7, 28, 12, 0, 0).unwrap();
    let reader = ClaudeCodeTranscriptReader::new(temp.path().join("cache"));
    let usage = reader
        .load_local_usage(temp.path().join("claude_projects"), now)
        .await
        .unwrap()
        .expect("should produce LocalUsage");

    assert_eq!(usage.skill_usages.len(), 2);

    // review: 5 loads across 3 sessions, sorts first by load_count.
    let review = &usage.skill_usages[0];
    assert_eq!(review.name, "review");
    assert_eq!(review.source_label, "Claude Code skill");
    assert_eq!(review.load_count, 5);
    assert_eq!(review.thread_count, 3);
    assert_eq!(
        review.last_loaded_at,
        Some(Utc.with_ymd_and_hms(2026, 7, 28, 12, 0, 0).unwrap())
    );
    assert!(review.id.starts_with("claude-code-skill:"));

    // blueprint: 3 loads across 2 sessions.
    let blueprint = &usage.skill_usages[1];
    assert_eq!(blueprint.name, "blueprint");
    assert_eq!(blueprint.source_label, "Claude Code skill");
    assert_eq!(blueprint.load_count, 3);
    assert_eq!(blueprint.thread_count, 2);
    assert_eq!(
        blueprint.last_loaded_at,
        Some(Utc.with_ymd_and_hms(2026, 7, 27, 11, 0, 0).unwrap())
    );
}

#[tokio::test]
async fn returns_empty_skill_usages_when_no_skills_loaded() {
    let temp = tempfile::tempdir().unwrap();
    let projects = temp.path().join("claude_projects").join("-C-Demo");
    std::fs::create_dir_all(&projects).unwrap();

    write_session(
        &projects.join("session-plain.jsonl"),
        vec![
            &session_line("2026-07-28T11:00:00.000Z", "p-1", None, 100),
            &session_line("2026-07-28T12:00:00.000Z", "p-2", None, 120),
        ],
    );

    let now = Utc.with_ymd_and_hms(2026, 7, 28, 12, 0, 0).unwrap();
    let reader = ClaudeCodeTranscriptReader::new(temp.path().join("cache"));
    let usage = reader
        .load_local_usage(temp.path().join("claude_projects"), now)
        .await
        .unwrap()
        .expect("should produce LocalUsage");

    assert!(usage.skill_usages.is_empty());
}
