//! Aggregates per-turn inference call samples into model performance stats.
//!
//! The samples themselves are produced while parsing Codex transcripts (see
//! `codex_transcript.rs`); this module only performs the aggregation so that
//! readers stay free of dashboard-level logic.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::codex_transcript::CodexTranscriptSummary;
use crate::models::{InferenceCallSample, InferenceModelStats, InferencePerformance};

/// Rebuilds model inference performance from the call samples captured in the
/// given summaries, grouped by (model, effort) and sorted by total duration.
///
/// Returns `None` when there are no valid calls (duration > 0).
pub fn make_inference_performance(
    summaries: &[CodexTranscriptSummary],
    now: DateTime<Utc>,
) -> Option<InferencePerformance> {
    let samples: Vec<&InferenceCallSample> = summaries
        .iter()
        .flat_map(|summary| summary.inference_calls.iter())
        .filter(|sample| sample.duration_ms > 0)
        .collect();
    if samples.is_empty() {
        return None;
    }

    let mut grouped: HashMap<(Option<String>, Option<String>), Vec<&InferenceCallSample>> =
        HashMap::new();
    for sample in &samples {
        grouped
            .entry((sample.model.clone(), sample.effort.clone()))
            .or_default()
            .push(sample);
    }

    let mut models: Vec<InferenceModelStats> = Vec::with_capacity(grouped.len());
    for ((model, effort), group) in grouped {
        let mut durations: Vec<f64> = group
            .iter()
            .map(|sample| sample.duration_ms as f64)
            .collect();
        durations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let call_count = group.len() as i64;
        let total_duration_ms: i64 = group.iter().map(|sample| sample.duration_ms).sum();
        let total_output_tokens: i64 = group.iter().map(|sample| sample.output_tokens).sum();
        let total_reasoning_tokens: i64 = group
            .iter()
            .map(|sample| sample.reasoning_output_tokens)
            .sum();

        let duration_seconds = total_duration_ms as f64 / 1000.0;
        let average_tokens_per_second = if duration_seconds > 0.0 {
            total_output_tokens as f64 / duration_seconds
        } else {
            0.0
        };
        let reasoning_output_ratio = if total_output_tokens > 0 {
            total_reasoning_tokens as f64 / total_output_tokens as f64
        } else {
            0.0
        };

        models.push(InferenceModelStats {
            model,
            effort,
            call_count,
            total_duration_ms,
            average_duration_ms: total_duration_ms as f64 / call_count as f64,
            p50_duration_ms: percentile(&durations, 0.5),
            p90_duration_ms: percentile(&durations, 0.9),
            total_output_tokens,
            average_tokens_per_second,
            reasoning_output_ratio,
        });
    }

    models.sort_by(|left, right| right.total_duration_ms.cmp(&left.total_duration_ms));

    Some(InferencePerformance {
        refreshed_at: now,
        models,
    })
}

/// Interpolated percentile over sorted values, mirroring the macOS reference.
fn percentile(sorted_values: &[f64], fraction: f64) -> f64 {
    match sorted_values.len() {
        0 => 0.0,
        1 => sorted_values[0],
        _ => {
            let clamped = fraction.clamp(0.0, 1.0);
            let position = (sorted_values.len() - 1) as f64 * clamped;
            let lower = position.floor() as usize;
            let upper = position.ceil() as usize;
            if lower == upper {
                return sorted_values[lower];
            }
            let progress = position - lower as f64;
            sorted_values[lower] + (sorted_values[upper] - sorted_values[lower]) * progress
        }
    }
}
