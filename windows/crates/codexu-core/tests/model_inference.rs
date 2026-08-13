//! Integration tests for model inference performance rebuilt from Codex
//! transcripts and aggregated by `make_inference_performance`.

use std::collections::HashMap;

use chrono::{Duration, TimeZone, Utc};
use codexu_core::models::InferenceCallSample;
use codexu_core::readers::codex_transcript::CodexTranscriptSummary;
use codexu_core::readers::{make_inference_performance, CodexTranscriptReader};

fn sample(
    model: &str,
    effort: &str,
    duration_ms: i64,
    output_tokens: i64,
    reasoning_output_tokens: i64,
) -> InferenceCallSample {
    InferenceCallSample {
        turn_id: Some("turn".to_string()),
        model: Some(model.to_string()),
        effort: Some(effort.to_string()),
        duration_ms,
        output_tokens,
        reasoning_output_tokens,
        date: Utc.timestamp_opt(1_800_000_000, 0).unwrap(),
    }
}

fn summary_with(samples: Vec<InferenceCallSample>) -> CodexTranscriptSummary {
    CodexTranscriptSummary {
        file_path: "rollout-inference.jsonl".to_string(),
        session_id: "session-inference".to_string(),
        project_path: "/tmp".to_string(),
        model: None,
        last_active_at: None,
        deltas: Vec::new(),
        tool_calls: HashMap::new(),
        skill_loads: Vec::new(),
        task_intervals: Vec::new(),
        inference_calls: samples,
    }
}

#[test]
fn aggregates_samples_by_model_and_effort_with_percentiles() {
    let now = Utc.timestamp_opt(1_800_000_000, 0).unwrap();
    let summaries = vec![summary_with(vec![
        sample("gpt-test", "high", 2000, 20, 5),
        sample("gpt-test", "high", 4000, 40, 10),
        sample("gpt-test", "high", 8000, 80, 20),
        sample("gpt-test", "low", 6000, 30, 0),
    ])];
    let performance = make_inference_performance(&summaries, now).expect("should aggregate");

    assert_eq!(performance.refreshed_at, now);
    assert_eq!(performance.models.len(), 2);

    let high = performance
        .models
        .iter()
        .find(|stats| stats.effort.as_deref() == Some("high"))
        .expect("high effort group");
    assert_eq!(high.model.as_deref(), Some("gpt-test"));
    assert_eq!(high.call_count, 3);
    assert_eq!(high.total_duration_ms, 14000);
    assert!((high.average_duration_ms - 14000.0 / 3.0).abs() < 1e-9);
    assert_eq!(high.p50_duration_ms, 4000.0);
    assert_eq!(high.p90_duration_ms, 7200.0);
    assert_eq!(high.total_output_tokens, 140);
    assert!((high.average_tokens_per_second - 10.0).abs() < 1e-9);
    assert!((high.reasoning_output_ratio - 0.25).abs() < 1e-9);

    let low = performance
        .models
        .iter()
        .find(|stats| stats.effort.as_deref() == Some("low"))
        .expect("low effort group");
    assert_eq!(low.call_count, 1);
    assert_eq!(low.total_duration_ms, 6000);
    assert_eq!(low.total_output_tokens, 30);
    assert_eq!(low.reasoning_output_ratio, 0.0);

    // Sorted by total duration descending: high (14000ms) before low (6000ms).
    assert_eq!(performance.models[0].effort.as_deref(), Some("high"));
    assert_eq!(performance.models[1].effort.as_deref(), Some("low"));
}

#[test]
fn returns_none_without_valid_calls() {
    let now = Utc.timestamp_opt(1_800_000_000, 0).unwrap();
    assert_eq!(make_inference_performance(&[], now), None);
    // Zero-duration samples are not valid calls.
    let summaries = vec![summary_with(vec![sample("m", "e", 0, 10, 0)])];
    assert_eq!(make_inference_performance(&summaries, now), None);
}

#[tokio::test]
async fn parses_transcript_into_inference_performance() {
    let temp = tempfile::tempdir().unwrap();
    let archived = temp.path().join("archived_sessions");
    tokio::fs::create_dir_all(&archived).await.unwrap();

    let session = archived.join("rollout-inference.jsonl");
    let started = Utc.with_ymd_and_hms(2026, 3, 26, 12, 0, 0).unwrap();
    let completed = started + Duration::seconds(5);
    let lines = vec![
        format!(
            r#"{{"timestamp":"{}","type":"session_meta","payload":{{"id":"session-1","cwd":"h:\\project\\demo","model_provider":"openai"}}}}"#,
            started.to_rfc3339()
        ),
        format!(
            r#"{{"timestamp":"{}","type":"turn_context","payload":{{"turn_id":"turn-1","model":"gpt-5.4","effort":"high"}}}}"#,
            started.to_rfc3339()
        ),
        format!(
            r#"{{"timestamp":"{}","type":"event_msg","payload":{{"type":"task_started","turn_id":"turn-1","started_at":"{}"}}}}"#,
            started.to_rfc3339(),
            started.to_rfc3339()
        ),
        format!(
            r#"{{"timestamp":"{}","type":"event_msg","payload":{{"type":"token_count","turn_id":"turn-1","info":{{"last_token_usage":{{"input_tokens":100,"cached_input_tokens":0,"output_tokens":100,"reasoning_output_tokens":40,"total_tokens":200}}}}}}}}"#,
            started.to_rfc3339()
        ),
        format!(
            r#"{{"timestamp":"{}","type":"event_msg","payload":{{"type":"task_complete","turn_id":"turn-1","completed_at":"{}","duration_ms":5000}}}}"#,
            completed.to_rfc3339(),
            completed.to_rfc3339()
        ),
    ];
    tokio::fs::write(&session, lines.join("\n")).await.unwrap();

    let cache = temp.path().join("cache");
    let reader = CodexTranscriptReader::new(&cache);
    let usage = reader
        .load_local_usage(temp.path(), Utc::now())
        .await
        .unwrap()
        .expect("should produce LocalUsage");

    let performance = usage
        .inference_performance
        .as_ref()
        .expect("should build inference performance");
    assert_eq!(performance.models.len(), 1);
    let stats = &performance.models[0];
    assert_eq!(stats.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(stats.effort.as_deref(), Some("high"));
    assert_eq!(stats.call_count, 1);
    assert_eq!(stats.total_duration_ms, 5000);
    assert_eq!(stats.average_duration_ms, 5000.0);
    assert_eq!(stats.p50_duration_ms, 5000.0);
    assert_eq!(stats.p90_duration_ms, 5000.0);
    assert_eq!(stats.total_output_tokens, 100);
    assert!((stats.average_tokens_per_second - 20.0).abs() < 1e-9);
    assert!((stats.reasoning_output_ratio - 0.4).abs() < 1e-9);
}

#[tokio::test]
async fn missing_task_complete_yields_no_inference_performance() {
    let temp = tempfile::tempdir().unwrap();
    let archived = temp.path().join("archived_sessions");
    tokio::fs::create_dir_all(&archived).await.unwrap();

    let session = archived.join("rollout-no-complete.jsonl");
    let started = Utc.with_ymd_and_hms(2026, 3, 26, 12, 0, 0).unwrap();
    let lines = vec![
        format!(
            r#"{{"timestamp":"{}","type":"turn_context","payload":{{"turn_id":"turn-1","model":"gpt-5.4","effort":"high"}}}}"#,
            started.to_rfc3339()
        ),
        format!(
            r#"{{"timestamp":"{}","type":"event_msg","payload":{{"type":"token_count","turn_id":"turn-1","info":{{"last_token_usage":{{"input_tokens":100,"cached_input_tokens":0,"output_tokens":50,"reasoning_output_tokens":10,"total_tokens":160}}}}}}}}"#,
            started.to_rfc3339()
        ),
    ];
    tokio::fs::write(&session, lines.join("\n")).await.unwrap();

    let cache = temp.path().join("cache");
    let reader = CodexTranscriptReader::new(&cache);
    let usage = reader
        .load_local_usage(temp.path(), Utc::now())
        .await
        .unwrap()
        .expect("should produce LocalUsage");

    assert!(usage.inference_performance.is_none());
}

#[tokio::test]
async fn interval_only_turns_become_zero_token_calls() {
    let temp = tempfile::tempdir().unwrap();
    let archived = temp.path().join("archived_sessions");
    tokio::fs::create_dir_all(&archived).await.unwrap();

    let session = archived.join("rollout-interval-only.jsonl");
    let started = Utc.with_ymd_and_hms(2026, 3, 26, 12, 0, 0).unwrap();
    let token_turn_complete = started + Duration::seconds(5);
    let interval_only_complete = started + Duration::seconds(30);
    let lines = vec![
        // Turn 1 has full data: token event + explicit completion duration.
        format!(
            r#"{{"timestamp":"{}","type":"turn_context","payload":{{"turn_id":"turn-1","model":"gpt-5.4","effort":"high"}}}}"#,
            started.to_rfc3339()
        ),
        format!(
            r#"{{"timestamp":"{}","type":"event_msg","payload":{{"type":"token_count","turn_id":"turn-1","info":{{"last_token_usage":{{"input_tokens":100,"cached_input_tokens":0,"output_tokens":100,"reasoning_output_tokens":40,"total_tokens":200}}}}}}}}"#,
            started.to_rfc3339()
        ),
        format!(
            r#"{{"timestamp":"{}","type":"event_msg","payload":{{"type":"task_complete","turn_id":"turn-1","completed_at":"{}","duration_ms":5000}}}}"#,
            token_turn_complete.to_rfc3339(),
            token_turn_complete.to_rfc3339()
        ),
        // Turn 2 has only a task interval, no token event at all.
        format!(
            r#"{{"timestamp":"{}","type":"turn_context","payload":{{"turn_id":"turn-2","model":"gpt-5.4","effort":"medium"}}}}"#,
            started.to_rfc3339()
        ),
        format!(
            r#"{{"timestamp":"{}","type":"event_msg","payload":{{"type":"task_started","turn_id":"turn-2","started_at":"{}"}}}}"#,
            started.to_rfc3339(),
            started.to_rfc3339()
        ),
        format!(
            r#"{{"timestamp":"{}","type":"event_msg","payload":{{"type":"task_complete","turn_id":"turn-2","completed_at":"{}"}}}}"#,
            interval_only_complete.to_rfc3339(),
            interval_only_complete.to_rfc3339()
        ),
    ];
    tokio::fs::write(&session, lines.join("\n")).await.unwrap();

    let cache = temp.path().join("cache");
    let reader = CodexTranscriptReader::new(&cache);
    let usage = reader
        .load_local_usage(temp.path(), Utc::now())
        .await
        .unwrap()
        .expect("should produce LocalUsage");

    let performance = usage
        .inference_performance
        .as_ref()
        .expect("interval-only turns should still count as calls");
    assert_eq!(performance.models.len(), 2);

    let medium = performance
        .models
        .iter()
        .find(|stats| stats.effort.as_deref() == Some("medium"))
        .expect("medium effort group from interval-only turn");
    assert_eq!(medium.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(medium.call_count, 1);
    assert_eq!(medium.total_duration_ms, 30000);
    assert_eq!(medium.total_output_tokens, 0);
    assert_eq!(medium.reasoning_output_ratio, 0.0);
    assert_eq!(medium.average_tokens_per_second, 0.0);

    let high = performance
        .models
        .iter()
        .find(|stats| stats.effort.as_deref() == Some("high"))
        .expect("high effort group from token turn");
    assert_eq!(high.call_count, 1);
    assert_eq!(high.total_duration_ms, 5000);
    assert_eq!(high.total_output_tokens, 100);
}
