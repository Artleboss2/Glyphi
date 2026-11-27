//! Roo Code, and Cline, which share one task layout.
//!
//! Both extensions keep a folder per task under the editor's extension storage,
//! and each folder carries a `history_item.json` with the task's own totals:
//!
//!   {"ts":1773587780669,"tokensIn":27021,"tokensOut":269,
//!    "cacheWrites":0,"cacheReads":0,"workspace":"c:\\work\\project","mode":"architect"}
//!
//! That is the provider's accounting, not an estimate, so one task becomes one
//! event with its four token categories kept apart. The workspace names the project
//! and the task id is the session, which is what the dashboard groups by.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use glyphi_domain::{
    LocalDay, ProviderReading, ProviderSource, QuotaMetric, QuotaUnit, TokenCounts,
};
use serde_json::Value;

use crate::adapter::{home_directory, instant, number, text, Adapter, ProviderSnapshot, ScanContext, UsageEvent};

/// The extension identifiers that write this layout. A Cline install is the same
/// format under a different publisher, so it is read rather than ignored.
const EXTENSIONS: &[&str] = &["rooveterinaryinc.roo-cline", "saoudrizwan.claude-dev"];
/// The editor profiles that can hold them: stable, Insiders, VSCodium, Cursor.
const EDITORS: &[&str] = &["Code", "Code - Insiders", "VSCodium", "Cursor"];

pub struct RooAdapter;

impl Adapter for RooAdapter {
    fn id(&self) -> &str {
        "roo"
    }

    fn name(&self) -> &str {
        "Roo Code"
    }

    fn available(&self) -> bool {
        self.roots(&home_directory()).iter().any(|root| root.is_dir())
    }

    fn refresh(&self, context: &ScanContext<'_>) -> ProviderSnapshot {
        let events = self.scan(context);
        let day = LocalDay::of(context.now, context.zone);
        let today: TokenCounts = events
            .iter()
            .filter(|event| LocalDay::of(event.at, context.zone) == day)
            .fold(TokenCounts::ZERO, |sum, event| sum.saturating_add(event.tokens));

        let metrics = vec![
            QuotaMetric::new("roo_tokens_today", "Tokens used today", QuotaUnit::Tokens, Some(today.total() as f64), None, None, None),
            QuotaMetric::new("roo_input_today", "Input tokens today", QuotaUnit::Tokens, Some(today.input_tokens as f64), None, None, None),
            QuotaMetric::new("roo_output_today", "Output tokens today", QuotaUnit::Tokens, Some(today.output_tokens as f64), None, None, None),
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

impl RooAdapter {
    fn scan(&self, context: &ScanContext<'_>) -> Vec<UsageEvent> {
        let mut events = Vec::new();
        for root in self.roots(&context.credentials.home) {
            for extension in EXTENSIONS {
                let tasks = root.join(extension).join("tasks");
                let Ok(entries) = std::fs::read_dir(&tasks) else { continue };
                for entry in entries.flatten() {
                    let file = entry.path().join("history_item.json");
                    if context.ignored(&file.to_string_lossy()) {
                        continue;
                    }
                    let Ok(contents) = std::fs::read_to_string(&file) else { continue };
                    let Ok(value) = serde_json::from_str::<Value>(&contents) else { continue };
                    if let Some(event) = event_of(&value, context.now) {
                        events.push(event);
                    }
                }
            }
        }
        events
    }

    /// Every editor profile the extension could have been installed into.
    fn roots(&self, home: &PathBuf) -> Vec<PathBuf> {
        let mut bases: Vec<PathBuf> = Vec::new();
        for key in ["APPDATA", "LOCALAPPDATA"] {
            if let Ok(value) = std::env::var(key) {
                bases.push(PathBuf::from(value));
            }
        }
        bases.push(home.join("AppData").join("Roaming"));

        // `%APPDATA%` is usually `<home>/AppData/Roaming`, so the same storage
        // folder can arrive twice; a set keeps a task from being counted twice.
        let mut roots = std::collections::BTreeSet::new();
        for base in bases {
            for editor in EDITORS {
                roots.insert(base.join(editor).join("User").join("globalStorage"));
            }
        }
        roots.into_iter().collect()
    }
}

/// One task out of its `history_item.json`.
pub fn event_of(value: &Value, now: DateTime<Utc>) -> Option<UsageEvent> {
    let tokens = TokenCounts {
        input_tokens: whole(value.get("tokensIn")),
        cached_input_tokens: whole(value.get("cacheReads")),
        cache_creation_input_tokens: whole(value.get("cacheWrites")),
        output_tokens: whole(value.get("tokensOut")),
    };
    if tokens.is_empty() {
        return None;
    }
    let at = instant(value.get("ts")).unwrap_or(now);
    // The history records no model: `mode` is how the agent works ("architect",
    // "code") and `apiConfigName` is the name of a settings profile, so neither is
    // stored as one. A neutral label keeps the charts honest until the model is known.
    let model = "Roo Code".to_string();
    let mut event = UsageEvent::new("roo", "local", model, at, tokens);
    event.session = text(value.get("id"));
    event.project = text(value.get("workspace"));
    Some(event)
}

fn whole(value: Option<&Value>) -> u64 {
    number(value).map(|value| value.round() as u64).unwrap_or(0)
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
    fn a_task_becomes_one_event_with_its_categories() {
        let value = json!({
            "id": "019cf211",
            "ts": 1773587780669i64,
            "tokensIn": 27021,
            "tokensOut": 269,
            "cacheWrites": 12,
            "cacheReads": 340,
            "workspace": "c:\\work\\project",
            "mode": "architect"
        });
        let event = event_of(&value, now()).unwrap();
        assert_eq!(event.tokens.input_tokens, 27_021);
        assert_eq!(event.tokens.cached_input_tokens, 340);
        assert_eq!(event.tokens.cache_creation_input_tokens, 12);
        assert_eq!(event.tokens.output_tokens, 269);
        assert_eq!(event.session.as_deref(), Some("019cf211"));
        // The mode is not a model and must not be stored as one.
        assert_eq!(event.model, "Roo Code");
    }

    #[test]
    fn a_task_without_tokens_is_not_an_event() {
        let value = json!({ "id": "x", "ts": 1, "tokensIn": 0, "tokensOut": 0 });
        assert!(event_of(&value, now()).is_none());
    }
}
