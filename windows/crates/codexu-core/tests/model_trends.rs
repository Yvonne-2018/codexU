//! Integration tests for per-model token trends (model_trends).

use chrono::{DateTime, Utc};
use codexu_core::{
    models::{TokenBreakdown, UsageTrend},
    readers::{make_local_usage, SessionSummary, UsageDelta},
};
use std::collections::HashMap;

const NOW: &str = "2026-07-28T12:00:00Z";

fn at(iso: &str) -> DateTime<Utc> {
    iso.parse::<DateTime<Utc>>().unwrap()
}

fn summary(session_id: &str, model: Option<&str>, deltas: Vec<UsageDelta>) -> SessionSummary {
    SessionSummary {
        file_path: format!("rollout-{}.jsonl", session_id),
        session_id: session_id.to_string(),
        project_path: "C:\\Projects\\Demo".to_string(),
        model: model.map(|m| m.to_string()),
        last_active_at: deltas.iter().map(|d| d.date).max(),
        created_at: None,
        deltas,
        tool_calls: HashMap::new(),
        title: None,
        archived: false,
        git_branch: None,
        git_origin_url: None,
        thread_source: None,
        parent_thread_id: None,
        task_intervals: Vec::new(),
    }
}

fn delta(message_id: &str, date: DateTime<Utc>, model: Option<&str>, total: i64) -> UsageDelta {
    UsageDelta {
        message_id: Some(message_id.to_string()),
        date,
        tokens: TokenBreakdown {
            input_tokens: total,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            total_tokens: total,
        },
        model: model.map(|m| m.to_string()),
        project_path: "C:\\Projects\\Demo".to_string(),
        session_id: "session".to_string(),
    }
}

fn trend_of<'a>(
    usage: &'a codexu_core::models::LocalUsage,
    model: &str,
) -> &'a codexu_core::models::ModelUsageTrend {
    let trends = usage
        .usage_trend
        .as_ref()
        .and_then(|t| t.model_trends.as_ref())
        .unwrap_or_else(|| panic!("missing model_trends"));
    trends
        .iter()
        .find(|t| t.model.as_deref() == Some(model))
        .unwrap_or_else(|| panic!("missing trend for {model}"))
}

#[test]
fn groups_models_into_180_day_trends_with_correct_summaries() {
    let now = at(NOW);

    let gpt = summary(
        "session-gpt",
        Some("gpt-5.4"),
        vec![
            delta("g-1", at("2026-07-28T10:00:00Z"), Some("gpt-5.4"), 100),
            delta("g-2", at("2026-07-25T10:00:00Z"), Some("gpt-5.4"), 50),
            delta("g-3", at("2026-07-18T10:00:00Z"), Some("gpt-5.4"), 30),
        ],
    );
    let claude = summary(
        "session-claude",
        Some("claude-sonnet-4-5"),
        vec![delta(
            "c-1",
            at("2026-07-28T09:00:00Z"),
            Some("claude-sonnet-4-5"),
            40,
        )],
    );
    let unrecorded = summary(
        "session-unknown",
        None,
        vec![delta("u-1", at("2026-07-28T08:00:00Z"), None, 10)],
    );

    let usage = make_local_usage(vec![gpt, claude, unrecorded], now).expect("local usage");
    let trends = usage
        .usage_trend
        .as_ref()
        .and_then(|t: &UsageTrend| t.model_trends.as_ref())
        .expect("model_trends should be present");

    // Three models, ranked by total tokens descending.
    assert_eq!(trends.len(), 3);
    assert_eq!(
        trends
            .iter()
            .map(|t| t.model.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["gpt-5.4", "claude-sonnet-4-5", "unknown"]
    );

    let gpt_trend = trend_of(&usage, "gpt-5.4");
    assert_eq!(gpt_trend.day_buckets.len(), 180);
    assert_eq!(gpt_trend.day_buckets.first().unwrap().id, "2026-01-30");
    assert_eq!(gpt_trend.day_buckets.last().unwrap().id, "2026-07-28");
    assert_eq!(gpt_trend.active_day_count, 3);
    assert_eq!(
        gpt_trend.summary.seven_day.tokens.total_tokens, 150,
        "last seven days should include 07-25 and 07-28"
    );
    assert_eq!(gpt_trend.summary.daily_average_tokens, 21);
    assert_eq!(gpt_trend.summary.change_percent, Some(400.0));
    assert!(!gpt_trend.summary.is_new_activity);
    assert_eq!(
        gpt_trend.summary.peak_day.as_ref().unwrap().id,
        "2026-07-28"
    );

    let claude_trend = trend_of(&usage, "claude-sonnet-4-5");
    assert_eq!(claude_trend.active_day_count, 1);
    assert_eq!(claude_trend.summary.seven_day.tokens.total_tokens, 40);
    assert_eq!(claude_trend.summary.change_percent, None);
    assert!(claude_trend.summary.is_new_activity);

    let unknown_trend = trend_of(&usage, "unknown");
    assert_eq!(unknown_trend.summary.seven_day.tokens.total_tokens, 10);
    assert!(unknown_trend.summary.is_new_activity);
}

#[test]
fn caps_model_trends_at_top_eight_by_total_tokens() {
    let now = at(NOW);

    // Ten models with total tokens 1000 down to 100.
    let sessions: Vec<SessionSummary> = (0..10)
        .map(|index| {
            let model = format!("m{index}");
            summary(
                &format!("session-{model}"),
                Some(&model),
                vec![delta(
                    &format!("{model}-1"),
                    now,
                    Some(&model),
                    1000 - 100 * index,
                )],
            )
        })
        .collect();

    let usage = make_local_usage(sessions, now).expect("local usage");
    let trends = usage
        .usage_trend
        .as_ref()
        .and_then(|t: &UsageTrend| t.model_trends.as_ref())
        .expect("model_trends should be present");

    assert_eq!(trends.len(), 8);
    assert_eq!(
        trends
            .iter()
            .map(|t| t.model.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["m0", "m1", "m2", "m3", "m4", "m5", "m6", "m7"]
    );
    assert!(!trends.iter().any(|t| t.model.as_deref() == Some("m8")));
    assert!(!trends.iter().any(|t| t.model.as_deref() == Some("m9")));
    assert!(trends.iter().all(|t| t.active_day_count == 1));
}

#[test]
fn returns_none_without_any_token_deltas() {
    let now = at(NOW);
    assert!(make_local_usage(Vec::new(), now).is_none());
}
