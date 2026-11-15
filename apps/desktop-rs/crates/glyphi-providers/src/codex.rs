//! Codex, the ChatGPT coding CLI.
//!
//! Its session files interleave conversation turns with `token_count` events. Those
//! events carry a **cumulative** `total_token_usage` for the session, so the totals
//! must be differenced: taking the last line as the session's usage is right, adding
//! every line is wrong by a factor of the number of turns.
//!
//! The same events carry `rate_limits`, which is where the plan windows come from.

use chrono::Utc;
use glyphi_domain::{
    ProviderError, ProviderIdentity, ProviderReading, ProviderSource, QuotaMetric, QuotaUnit,
    ReadingWindow, TokenCounts,
};
use serde_json::Value;

use crate::adapter::{
    home_directory, instant, number, positive, text, Adapter, ProviderSnapshot, ScanContext,
    UsageEvent,
};
use crate::jsonl::{for_each_value, recent_files, window_start};

pub struct CodexAdapter;

impl Adapter for CodexAdapter {
    fn id(&self) -> &str {
        "codex"
    }

    fn name(&self) -> &str {
        "Codex"
    }

    fn available(&self) -> bool {
        home_directory().join(".codex").is_dir()
    }

    fn refresh(&self, context: &ScanContext<'_>) -> ProviderSnapshot {
        let (events, reading) = self.scan(context);
        let reading = reading.unwrap_or_else(|| ProviderReading {
            source: ProviderSource::LocalSessions,
            windows: Vec::new(),
            metrics: Vec::new(),
            identity: None,
            credits: None,
            account_key: Some("local".to_string()),
            day: None,
            error: Some(ProviderError::new(
                "no_quota",
                "Codex has not reported a rate limit on this machine yet.",
                false,
            )),
            updated_at: context.now.to_rfc3339(),
        });
        ProviderSnapshot::new(self.id(), self.name(), reading).with_events(events)
    }
}

impl CodexAdapter {
    /// One pass over the sessions: events and the newest quota snapshot.
    fn scan(&self, context: &ScanContext<'_>) -> (Vec<UsageEvent>, Option<ProviderReading>) {
        let since = window_start(context.now, context.history_days);
        let mut roots = vec![
            home_directory().join(".codex").join("sessions"),
            context.credentials.home.join(".codex").join("sessions"),
        ];
        roots.extend(context.extra_log_paths.iter().cloned());
        roots.sort();
        roots.dedup();

        let mut events = Vec::new();
        let mut reading: Option<ProviderReading> = None;

        for root in roots {
            for file in recent_files(&root, since) {
                if context.ignored(&file.to_string_lossy()) {
                    continue;
                }
                // Cumulative counters reset with every session file.
                let mut previous = TokenCounts::ZERO;
                let mut model = "unknown".to_string();
                for_each_value(&file, |value| {
                    if let Some(name) = turn_model(value) {
                        model = name;
                    }
                    if let Some(counts) = token_counts(value) {
                        let delta = subtract(counts, previous);
                        previous = counts;
                        if !delta.is_empty() {
                            let at = instant(value.get("timestamp")).unwrap_or(context.now);
                            if at >= since {
                                let mut event =
                                    UsageEvent::new("codex", "local", model.clone(), at, delta);
                                event.session = text(value.get("session_id"))
                                    .or_else(|| file.file_stem().map(|stem| stem.to_string_lossy().into_owned()));
                                events.push(event);
                            }
                        }
                    }
                    if let Some((windows, plan)) = rate_limits(value) {
                        // The last session read wins: files are visited newest-name
                        // first and a later line is a later snapshot.
                        reading = Some(ProviderReading {
                            source: ProviderSource::ProviderQuota,
                            windows,
                            metrics: quota_metrics(value),
                            identity: plan.map(|plan| ProviderIdentity { plan: Some(plan) }),
                            credits: None,
                            account_key: Some("local".to_string()),
                            day: None,
                            error: None,
                            updated_at: context.now.to_rfc3339(),
                        });
                    }
                    true
                });
            }
        }
        (events, reading)
    }
}

/// `token_count` events hold the session's running total.
fn token_counts(value: &Value) -> Option<TokenCounts> {
    let payload = value.get("payload")?;
    if payload.get("type").and_then(Value::as_str) != Some("token_count") {
        return None;
    }
    let usage = payload
        .get("info")
        .and_then(|info| info.get("total_token_usage"))
        .or_else(|| payload.get("total_token_usage"))?;
    let counts = TokenCounts {
        input_tokens: whole(usage.get("input_tokens")),
        cached_input_tokens: whole(usage.get("cached_input_tokens")),
        cache_creation_input_tokens: 0,
        output_tokens: whole(usage.get("output_tokens")),
    };
    Some(counts)
}

/// The count of what the last line added, never negative even if a session resets.
fn subtract(current: TokenCounts, previous: TokenCounts) -> TokenCounts {
    TokenCounts {
        input_tokens: current.input_tokens.saturating_sub(previous.input_tokens),
        cached_input_tokens: current
            .cached_input_tokens
            .saturating_sub(previous.cached_input_tokens),
        cache_creation_input_tokens: 0,
        output_tokens: current.output_tokens.saturating_sub(previous.output_tokens),
    }
}

fn whole(value: Option<&Value>) -> u64 {
    positive(value).map(|value| value.round() as u64).unwrap_or(0)
}

fn turn_model(value: &Value) -> Option<String> {
    let payload = value.get("payload")?;
    if payload.get("type").and_then(Value::as_str) != Some("turn_context") {
        return None;
    }
    text(payload.get("model")).or_else(|| text(payload.get("model_name")))
}

/// The plan windows the CLI reports alongside its token counts.
pub fn rate_limits(value: &Value) -> Option<(Vec<ReadingWindow>, Option<String>)> {
    let payload = value.get("payload")?;
    let limits = payload.get("rate_limits")?;
    let mut windows = Vec::new();
    for (key, kind, label) in [
        ("primary", "session", "Session"),
        ("secondary", "weekly", "Weekly"),
    ] {
        let Some(window) = limits.get(key).and_then(Value::as_object) else {
            continue;
        };
        let Some(used) = number(window.get("used_percent")) else {
            continue;
        };
        windows.push(ReadingWindow::new(kind, label, used, reset_at(window)));
    }
    if windows.is_empty() {
        return None;
    }
    Some((windows, text(limits.get("plan_type")).or_else(|| text(payload.get("plan_type")))))
}

fn reset_at(window: &serde_json::Map<String, Value>) -> Option<String> {
    if let Some(seconds) = number(window.get("resets_in_seconds")) {
        let at = Utc::now() + chrono::Duration::seconds(seconds as i64);
        return Some(at.to_rfc3339());
    }
    text(window.get("resets_at")).or_else(|| text(window.get("reset_at")))
}

/// Codex also reports a token budget for the plan in some builds.
pub fn quota_metrics(value: &Value) -> Vec<QuotaMetric> {
    let mut metrics = Vec::new();
    let Some(payload) = value.get("payload") else {
        return metrics;
    };
    for (key, label) in [("token_budget", "Plan tokens"), ("credits", "Credits")] {
        let Some(entry) = payload.get(key).and_then(Value::as_object) else {
            continue;
        };
        if let Some(metric) = QuotaMetric::new(
            key,
            label,
            if key == "credits" { QuotaUnit::Credits } else { QuotaUnit::Tokens },
            number(entry.get("used")),
            number(entry.get("limit")),
            number(entry.get("remaining")),
            text(entry.get("reset_at")),
        ) {
            metrics.push(metric);
        }
    }
    metrics
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cumulative_totals_are_differenced() {
        let first = json!({
            "timestamp": "2026-11-15T10:00:00Z",
            "payload": { "type": "token_count", "info": { "total_token_usage": { "input_tokens": 100, "output_tokens": 20 } } }
        });
        let second = json!({
            "timestamp": "2026-11-15T10:05:00Z",
            "payload": { "type": "token_count", "info": { "total_token_usage": { "input_tokens": 300, "output_tokens": 60 } } }
        });
        let first_counts = token_counts(&first).unwrap();
        let second_counts = token_counts(&second).unwrap();
        assert_eq!(first_counts.input_tokens, 100);
        let delta = subtract(second_counts, first_counts);
        assert_eq!(delta.input_tokens, 200);
        assert_eq!(delta.output_tokens, 40);
    }

    #[test]
    fn a_reset_never_produces_a_negative_count() {
        let delta = subtract(
            TokenCounts::ZERO,
            TokenCounts {
                input_tokens: 500,
                ..TokenCounts::default()
            },
        );
        assert_eq!(delta.total(), 0);
    }

    #[test]
    fn rate_limits_become_windows() {
        let line = json!({
            "timestamp": "2026-11-15T10:00:00Z",
            "payload": {
                "type": "token_count",
                "rate_limits": {
                    "primary": { "used_percent": 24, "resets_in_seconds": 3600 },
                    "secondary": { "used_percent": 51 }
                },
                "plan_type": "plus"
            }
        });
        let (windows, plan) = rate_limits(&line).unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[1].used_percent, 51.0);
        assert_eq!(plan.as_deref(), Some("plus"));
    }

    #[test]
    fn a_turn_event_without_limits_is_ignored() {
        assert!(rate_limits(&json!({ "payload": { "type": "event_msg" } })).is_none());
        assert!(token_counts(&json!({ "payload": { "type": "event_msg" } })).is_none());
    }

    #[test]
    fn the_model_is_read_from_the_turn_context() {
        let line = json!({ "payload": { "type": "turn_context", "model": "gpt-5-codex" } });
        assert_eq!(turn_model(&line).as_deref(), Some("gpt-5-codex"));
    }
}
