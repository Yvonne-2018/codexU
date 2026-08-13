//! Model inference performance models.
//!
//! These structures describe per-(model, effort) inference performance rebuilt
//! from Codex transcript events (`task_complete` durations, `turn_context`
//! model/effort, and `token_count` output tokens). They live in `models`
//! because `LocalUsage` embeds the aggregate; readers may depend on models,
//! never the other way around.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single model inference call reconstructed from a Codex transcript turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceCallSample {
    pub turn_id: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub duration_ms: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub date: DateTime<Utc>,
}

/// Aggregated performance statistics for one (model, effort) combination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceModelStats {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub call_count: i64,
    pub total_duration_ms: i64,
    pub average_duration_ms: f64,
    pub p50_duration_ms: f64,
    pub p90_duration_ms: f64,
    pub total_output_tokens: i64,
    pub average_tokens_per_second: f64,
    pub reasoning_output_ratio: f64,
}

/// Model inference performance summary across all parsed transcripts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferencePerformance {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub refreshed_at: DateTime<Utc>,
    pub models: Vec<InferenceModelStats>,
}
