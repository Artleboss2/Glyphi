//! Gemini CLI.
//!
//! The CLI writes one session per file under `~/.gemini/tmp/<project>/chats/`, as
//! JSON lines. Every assistant line carries the CLI's own accounting:
//!
//!   {"type":"gemini","timestamp":"2026-11-20T09:12:03Z","model":"gemini-2.5-pro",
//!    "tokens":{"input":812,"output":96,"cached":200,"thoughts":0,"tool":0}}
//!
//! `cached` is counted inside `input` by the CLI, so it is clamped to it and the
//! stored input is the fresh part — the same reconciliation the dashboard expects
//! everywhere. `thoughts` are reasoning tokens already included in `output`, so
//! they are not added twice.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use glyphi_domain::{
    LocalDay, ProviderReading, ProviderSource, QuotaMetric, QuotaUnit, TokenCounts,
};
use serde_json::Value;

use crate::adapter::{home_directory, instant, number, text, Adapter, ProviderSnapshot, ScanContext, UsageEvent};

pub struct GeminiAdapter;

impl Adapter for GeminiAdapter {
    fn id(&self) -> &str {
        "gemini"
    }

    fn name(&self) -> &str {
        "Gemini CLI"
    }

    fn available(&self) -> bool {
        let root = home_directory().join(".gemini").join("tmp");
        root.is_dir()
    }

    fn refresh(&self, context: &ScanContext<'_>) -> ProviderSnapshot {
        let events = self.scan(context);
        let day = LocalDay::of(context.now, context.zone);
        let today = events
            .iter()
            .filter(|event| LocalDay::of(event.at, context.zone) == day)
            .fold(TokenCounts::ZERO, |sum, event| sum.saturating_add(event.tokens));

        let metrics = vec![
            QuotaMetric::new("gemini_tokens_today", "Tokens used today", QuotaUnit::Tokens, Some(today.total() as f64), None, None, None),
            QuotaMetric::new("gemini_input_today", "Input tokens today", QuotaUnit::Tokens, Some(today.input_tokens as f64), None, None, None),
            QuotaMetric::new("gemini_output_today", "Output tokens today", QuotaUnit::Tokens, Some(today.output_tokens as f64), None, None, None),
        ]
        .into_iter()
        .flatten()
        .collect();

        ProviderReading {
            source: ProviderSource::LocalSessions,
            windows: Vec::new(),
            metrics,
            identity: None,
            credits: None,
            account_key: Some("local".to_string()),
            day: Some(day),
            error: None,
            updated_at: context.now.to_rfc3339(),
        }
        .into_snapshot(self.id(), self.name(), events)
    }
}

impl GeminiAdapter {
    fn scan(&self, context: &ScanContext<'_>) -> Vec<UsageEvent> {
        let mut events = Vec::new();
        let root = context.credentials.home.join(".gemini").join("tmp");
        collect_sessions(&root, 0, context, &mut events);
        events
    }
}

/// `tmp/<project>/chats/*.jsonl`; the project folder name is a path hash, so the
/// walk is two levels deep and skips anything that is not a chat file.
fn collect_sessions(directory: &Path, depth: usize, context: &ScanContext<'_>, events: &mut Vec<UsageEvent>) {
    let Ok(entries) = std::fs::read_dir(directory) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < 3 {
                collect_sessions(&path, depth + 1, context, events);
            }
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        if context.ignored(&path.to_string_lossy()) {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&path) else { continue };
        for line in contents.lines() {
            let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
            if let Some(event) = event_of(&value, &path, context.now) {
                events.push(event);
            }
        }
    }
}

/// One assistant line of a session file.
pub fn event_of(value: &Value, path: &PathBuf, now: DateTime<Utc>) -> Option<UsageEvent> {
    let tokens = value.get("tokens")?;
    let reported = number(tokens.get("input")).unwrap_or(0.0).round() as u64;
    let cached = (number(tokens.get("cached")).unwrap_or(0.0).round() as u64).min(reported);
    let counts = TokenCounts {
        input_tokens: reported - cached,
        cached_input_tokens: cached,
        cache_creation_input_tokens: 0,
        output_tokens: number(tokens.get("output")).unwrap_or(0.0).round() as u64,
    };
    if counts.is_empty() {
        return None;
    }
    let at = instant(value.get("timestamp"))
        .or_else(|| instant(value.get("ts")))
        .unwrap_or(now);
    let model = text(value.get("model")).unwrap_or_else(|| "Gemini CLI".to_string());
    let mut event = UsageEvent::new("gemini", "local", model, at, counts);
    event.session = path
        .file_stem()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string());
    Some(event)
}

/// Small helper so the reading and its events are built in one expression.
trait IntoSnapshot {
    fn into_snapshot(self, id: &str, name: &str, events: Vec<UsageEvent>) -> ProviderSnapshot;
}

impl IntoSnapshot for ProviderReading {
    fn into_snapshot(self, id: &str, name: &str, events: Vec<UsageEvent>) -> ProviderSnapshot {
        ProviderSnapshot::new(id, name, self).with_events(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn cached_tokens_are_taken_out_of_the_input() {
        let line = json!({
            "type": "gemini",
            "timestamp": "2026-11-15T09:00:00Z",
            "model": "gemini-2.5-pro",
            "tokens": { "input": 812, "output": 96, "cached": 200, "thoughts": 0, "tool": 0 }
        });
        let event = event_of(&line, &PathBuf::from("session-1.jsonl"), now()).unwrap();
        assert_eq!(event.tokens.input_tokens, 612);
        assert_eq!(event.tokens.cached_input_tokens, 200);
        assert_eq!(event.tokens.output_tokens, 96);
        assert_eq!(event.model, "gemini-2.5-pro");
        assert_eq!(event.session.as_deref(), Some("session-1"));
    }

    #[test]
    fn a_line_without_tokens_is_not_an_event() {
        assert!(event_of(&json!({ "type": "user", "content": "hi" }), &PathBuf::new(), now()).is_none());
    }
}
