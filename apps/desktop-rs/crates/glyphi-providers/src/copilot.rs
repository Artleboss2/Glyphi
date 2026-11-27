//! GitHub Copilot Chat, from the transcripts VS Code keeps.
//!
//! VS Code stores chat sessions as JSON lines next to the extension that made them:
//! `globalStorage/emptyWindowChatSessions/*.jsonl` for a chat with no folder open,
//! and `workspaceStorage/<hash>/chatSessions/*.jsonl` for the rest. The first line
//! holds the session (`requests[]`), the following ones are edits to it.
//!
//! The token counts are whatever the extension put in the request: the shape moved
//! between versions, so this adapter looks for a `usage` object anywhere inside a
//! request and reads the four names it knows (`promptTokens`/`inputTokens`,
//! `completionTokens`/`outputTokens`, plus cached input). A request that carries no
//! usage is skipped rather than estimated — an invented number is worse than none.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use glyphi_domain::{
    LocalDay, ProviderReading, ProviderSource, QuotaMetric, QuotaUnit, TokenCounts,
};
use serde_json::Value;

use crate::adapter::{home_directory, instant, number, text, Adapter, ProviderSnapshot, ScanContext, UsageEvent};

pub struct CopilotAdapter;

impl Adapter for CopilotAdapter {
    fn id(&self) -> &str {
        "copilot"
    }

    fn name(&self) -> &str {
        "GitHub Copilot"
    }

    fn available(&self) -> bool {
        self.roots(&home_directory()).iter().any(|root| root.is_dir())
    }

    fn refresh(&self, context: &ScanContext<'_>) -> ProviderSnapshot {
        let events = self.scan(context);
        let day = LocalDay::of(context.now, context.zone);
        let today = events
            .iter()
            .filter(|event| LocalDay::of(event.at, context.zone) == day)
            .fold(TokenCounts::ZERO, |sum, event| sum.saturating_add(event.tokens));

        let metrics = vec![
            QuotaMetric::new("copilot_tokens_today", "Tokens used today", QuotaUnit::Tokens, Some(today.total() as f64), None, None, None),
            QuotaMetric::new("copilot_input_today", "Input tokens today", QuotaUnit::Tokens, Some(today.input_tokens as f64), None, None, None),
            QuotaMetric::new("copilot_output_today", "Output tokens today", QuotaUnit::Tokens, Some(today.output_tokens as f64), None, None, None),
        ]
        .into_iter()
        .flatten()
        .collect();

        let reading = ProviderReading {
            source: ProviderSource::LocalSessions,
            windows: Vec::new(),
            metrics,
            identity: None,
            credits: None,
            account_key: Some("local".to_string()),
            day: Some(day),
            error: None,
            updated_at: context.now.to_rfc3339(),
        };
        ProviderSnapshot::new(self.id(), self.name(), reading).with_events(events)
    }
}

impl CopilotAdapter {
    fn scan(&self, context: &ScanContext<'_>) -> Vec<UsageEvent> {
        let mut events = Vec::new();
        for root in self.roots(&context.credentials.home) {
            collect(&root, 0, context, &mut events);
        }
        events
    }

    /// Every editor profile and every place a chat transcript can live.
    fn roots(&self, home: &PathBuf) -> Vec<PathBuf> {
        let mut bases: Vec<PathBuf> = Vec::new();
        for key in ["APPDATA", "LOCALAPPDATA"] {
            if let Ok(value) = std::env::var(key) {
                bases.push(PathBuf::from(value));
            }
        }
        bases.push(home.join("AppData").join("Roaming"));

        // `%APPDATA%` is usually `<home>/AppData/Roaming`, so the same folder arrives
        // twice. A set keeps a transcript from being counted twice.
        let mut roots = std::collections::BTreeSet::new();
        for base in bases {
            for editor in ["Code", "Code - Insiders", "VSCodium"] {
                let user = base.join(editor).join("User");
                roots.insert(user.join("globalStorage").join("emptyWindowChatSessions"));
                roots.insert(user.join("workspaceStorage"));
            }
        }
        roots.into_iter().collect()
    }
}

fn collect(directory: &Path, depth: usize, context: &ScanContext<'_>, events: &mut Vec<UsageEvent>) {
    let Ok(entries) = std::fs::read_dir(directory) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < 3 {
                collect(&path, depth + 1, context, events);
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
        let session = path.file_stem().and_then(|name| name.to_str()).map(|name| name.to_string());
        for line in contents.lines() {
            let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
            events.extend(events_of(&value, session.as_deref(), context.now));
        }
    }
}

/// The requests in one transcript line that carry token usage.
pub fn events_of(value: &Value, session: Option<&str>, now: DateTime<Utc>) -> Vec<UsageEvent> {
    let mut events = Vec::new();
    let Some(requests) = value
        .get("v")
        .and_then(|state| state.get("requests"))
        .and_then(Value::as_array)
        .or_else(|| value.get("requests").and_then(Value::as_array))
    else {
        return events;
    };
    for request in requests {
        let Some(tokens) = usage_in(request) else { continue };
        if tokens.is_empty() {
            continue;
        }
        let at = instant(request.get("timestamp"))
            .or_else(|| instant(request.get("creationDate")))
            .unwrap_or(now);
        let model = text(request.get("modelId"))
            .or_else(|| text(request.get("model")))
            .unwrap_or_else(|| "GitHub Copilot".to_string());
        let mut event = UsageEvent::new("copilot", "local", model, at, tokens);
        event.session = text(request.get("requestId")).or_else(|| session.map(|name| name.to_string()));
        events.push(event);
    }
    events
}

/// Finds a `usage` object anywhere inside a request and reads the names seen in the
/// wild. Depth is bounded: a transcript nests a few levels, never a hundred.
fn usage_in(value: &Value) -> Option<TokenCounts> {
    match value {
        Value::Object(map) => {
            for (key, nested) in map {
                if key.eq_ignore_ascii_case("usage") || key.eq_ignore_ascii_case("tokenusage") {
                    if let Some(tokens) = tokens_of(nested) {
                        return Some(tokens);
                    }
                }
            }
            map.values().find_map(usage_in)
        }
        Value::Array(items) => items.iter().find_map(usage_in),
        _ => None,
    }
}

fn tokens_of(value: &Value) -> Option<TokenCounts> {
    let object = value.as_object()?;
    let read = |keys: &[&str]| -> Option<u64> {
        keys.iter().find_map(|key| {
            object
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(key))
                .and_then(|(_, value)| number(Some(value)))
                .map(|value| value.max(0.0).round() as u64)
        })
    };
    let reported = read(&["promptTokens", "inputTokens", "input_tokens", "prompt_tokens"])?;
    let output = read(&["completionTokens", "outputTokens", "output_tokens", "completion_tokens"]).unwrap_or(0);
    let cached = read(&["cachedTokens", "cachedInputTokens", "cacheReadInputTokens"]).unwrap_or(0).min(reported);
    Some(TokenCounts {
        input_tokens: reported - cached,
        cached_input_tokens: cached,
        cache_creation_input_tokens: 0,
        output_tokens: output,
    })
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
    fn a_request_with_usage_becomes_an_event() {
        let line = json!({
            "kind": 0,
            "v": {
                "requests": [
                    {
                        "requestId": "r-1",
                        "timestamp": 1774000000000i64,
                        "modelId": "gpt-4o",
                        "result": { "metadata": { "usage": { "promptTokens": 1200, "completionTokens": 300, "cachedTokens": 400 } } }
                    }
                ]
            }
        });
        let events = events_of(&line, Some("session-1"), now());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].tokens.input_tokens, 800);
        assert_eq!(events[0].tokens.cached_input_tokens, 400);
        assert_eq!(events[0].tokens.output_tokens, 300);
        assert_eq!(events[0].model, "gpt-4o");
        assert_eq!(events[0].session.as_deref(), Some("r-1"));
    }

    #[test]
    fn a_request_without_usage_is_skipped() {
        let line = json!({ "v": { "requests": [{ "requestId": "r-1", "message": { "text": "hi" } }] } });
        assert!(events_of(&line, None, now()).is_empty());
    }
}
