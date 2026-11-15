//! Claude.
//!
//! Two sources, used together:
//!
//! - `~/.claude/projects/**/*.jsonl`, written by Claude Code. Every assistant turn
//!   carries `message.usage`, which is the provider's own accounting — no estimate
//!   involved.
//! - `https://api.anthropic.com/api/oauth/usage`, read with the token the CLI
//!   already stored, which answers the 5-hour and weekly windows.
//!
//! The local scan is not optional: an expired token must still show yesterday's
//! tokens, only the quota gauges go missing.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use glyphi_domain::{
    LocalDay, ProviderError, ProviderIdentity, ProviderReading, ProviderSource, ReadingWindow,
    TokenCounts,
};
use serde_json::Value;

use crate::adapter::{
    home_directory, instant, number, positive, text, Adapter, Credential, ProviderSnapshot,
    ScanContext, UsageEvent,
};
use crate::jsonl::{for_each_value, recent_files, window_start};

pub struct ClaudeAdapter;

impl Adapter for ClaudeAdapter {
    fn id(&self) -> &str {
        "claude"
    }

    fn name(&self) -> &str {
        "Claude"
    }

    fn available(&self) -> bool {
        let home = home_directory();
        self.roots_in(&home).iter().any(|root| root.is_dir()) || credentials_path(&home).is_file()
    }

    fn refresh(&self, context: &ScanContext<'_>) -> ProviderSnapshot {
        let events = self.scan(context);
        let reading = match self.quota(context) {
            Ok(reading) => reading,
            Err(error) => ProviderReading {
                windows: Vec::new(),
                identity: None,
                metrics: Vec::new(),
                credits: None,
                account_key: None,
                day: None,
                updated_at: context.now.to_rfc3339(),
                source: ProviderSource::LocalSessions,
                error: Some(error),
            },
        };
        ProviderSnapshot::new(self.id(), self.name(), reading).with_events(events)
    }
}

impl ClaudeAdapter {
    fn scan(&self, context: &ScanContext<'_>) -> Vec<UsageEvent> {
        let since = window_start(context.now, context.history_days);
        let mut events = Vec::new();
        let mut roots = self.roots_in(&context.credentials.home);
        roots.extend(context.extra_log_paths.iter().cloned());

        for root in roots {
            for file in recent_files(&root, since) {
                if context.ignored(&file.to_string_lossy()) {
                    continue;
                }
                for_each_value(&file, |value| {
                    if let Some(event) = event_of(value, context.now) {
                        if event.at >= since {
                            events.push(event);
                        }
                    }
                    // Files are appended chronologically, so once a line is past the
                    // window the rest of the file is too.
                    true
                });
            }
        }
        events
    }

    /// `~/.claude/projects` and the XDG-style sibling both exist in the wild.
    fn roots_in(&self, home: &PathBuf) -> Vec<PathBuf> {
        vec![
            home.join(".claude").join("projects"),
            home.join(".config").join("claude").join("projects"),
        ]
    }

    fn quota(&self, context: &ScanContext<'_>) -> Result<ProviderReading, ProviderError> {
        let credential = claude_credential(&context.credentials.home)
            .ok_or_else(|| ProviderError::new("credentials_missing", "Connect Claude in Settings → Sources.", false))?;
        let response = ureq::get("https://api.anthropic.com/api/oauth/usage")
            .set("authorization", &format!("Bearer {}", credential.secret))
            .set("anthropic-beta", "oauth-2025-04-20")
            .set("user-agent", "claude-code/2.1.0")
            .set("accept", "application/json")
            .timeout(std::time::Duration::from_secs(15))
            .call();
        let body: Value = match response {
            Ok(response) => response
                .into_json()
                .map_err(|_| ProviderError::invalid("Claude"))?,
            Err(ureq::Error::Status(401 | 403, _)) => return Err(ProviderError::unauthorized("Claude")),
            Err(ureq::Error::Status(status, _)) => {
                return Err(ProviderError::new(
                    format!("http_{status}"),
                    format!("Claude usage returned HTTP {status}."),
                    status == 429 || status >= 500,
                ))
            }
            Err(_) => return Err(ProviderError::unreachable("Claude")),
        };
        parse_quota(&body, credential.plan.as_deref(), context.now)
    }
}

/// The token Claude Code stores at login, or the one the environment provides.
pub fn claude_credential(home: &PathBuf) -> Option<Credential> {
    if let Ok(token) = std::env::var("CLAUDE_CODE_OAUTH_TOKEN") {
        if !token.trim().is_empty() {
            return Some(Credential {
                secret: token,
                plan: None,
            });
        }
    }
    let contents = std::fs::read_to_string(credentials_path(home)).ok()?;
    let root: Value = serde_json::from_str(&contents).ok()?;
    let oauth = root.get("claudeAiOauth")?;
    Some(Credential {
        secret: text(oauth.get("accessToken"))?,
        plan: text(oauth.get("subscriptionType")),
    })
}

fn credentials_path(home: &PathBuf) -> PathBuf {
    home.join(".claude").join(".credentials.json")
}

/// One assistant turn out of a session file.
pub fn event_of(value: &Value, now: DateTime<Utc>) -> Option<UsageEvent> {
    let kind = value.get("type").and_then(Value::as_str)?;
    if kind != "assistant" {
        return None;
    }
    let message = value.get("message")?;
    let usage = message.get("usage")?;
    let tokens = TokenCounts {
        input_tokens: whole(usage.get("input_tokens")),
        cached_input_tokens: whole(usage.get("cache_read_input_tokens")),
        cache_creation_input_tokens: whole(usage.get("cache_creation_input_tokens")),
        output_tokens: whole(usage.get("output_tokens")),
    };
    if tokens.is_empty() {
        return None;
    }
    let at = instant(value.get("timestamp"))
        .or_else(|| instant(message.get("timestamp")))
        .unwrap_or(now);
    let model = text(message.get("model")).unwrap_or_else(|| "unknown".to_string());
    let mut event = UsageEvent::new("claude", "local", model, at, tokens);
    event.session = text(value.get("sessionId")).or_else(|| text(value.get("session_id")));
    event.project = text(value.get("cwd")).or_else(|| text(value.get("project")));
    Some(event)
}

fn whole(value: Option<&Value>) -> u64 {
    positive(value).map(|value| value.round() as u64).unwrap_or(0)
}

/// The two windows Anthropic reports, plus the extra-usage credit balance.
pub fn parse_quota(value: &Value, plan: Option<&str>, now: DateTime<Utc>) -> Result<ProviderReading, ProviderError> {
    let object = value
        .as_object()
        .ok_or_else(|| ProviderError::invalid("Claude"))?;

    let mut windows = Vec::new();
    for (key, kind, label) in [
        ("five_hour", "session", "5-hour"),
        ("seven_day", "weekly", "Weekly"),
    ] {
        let Some(candidate) = object.get(key).and_then(Value::as_object) else {
            continue;
        };
        let Some(used) = number(candidate.get("utilization")) else {
            return Err(ProviderError::invalid("Claude"));
        };
        if used < 0.0 {
            return Err(ProviderError::invalid("Claude"));
        }
        windows.push(ReadingWindow::new(
            kind,
            label,
            used,
            text(candidate.get("resets_at")),
        ));
    }

    let credits = object
        .get("extra_usage")
        .and_then(Value::as_object)
        .and_then(|extra| {
            if extra.get("is_enabled").and_then(Value::as_bool) != Some(true) {
                return None;
            }
            let limit = number(extra.get("monthly_limit"))?;
            let used = number(extra.get("used_credits"))?;
            Some(glyphi_domain::Credits {
                remaining: (limit - used).max(0.0),
                unit: text(extra.get("currency")).unwrap_or_else(|| "credits".to_string()),
            })
        });

    Ok(ProviderReading {
        source: ProviderSource::Oauth,
        windows,
        metrics: Vec::new(),
        identity: plan.map(|plan| ProviderIdentity {
            plan: Some(plan.to_string()),
        }),
        credits,
        account_key: Some("local".to_string()),
        day: Some(LocalDay::of(now, chrono_tz::UTC)),
        error: None,
        updated_at: now.to_rfc3339(),
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
    fn an_assistant_turn_becomes_an_event() {
        let line = json!({
            "type": "assistant",
            "timestamp": "2026-11-15T10:00:00Z",
            "sessionId": "s-1",
            "cwd": "/work/project",
            "message": {
                "model": "claude-sonnet-4-5",
                "usage": {
                    "input_tokens": 120,
                    "cache_read_input_tokens": 900,
                    "cache_creation_input_tokens": 30,
                    "output_tokens": 45
                }
            }
        });
        let event = event_of(&line, now()).unwrap();
        assert_eq!(event.tokens.input_tokens, 120);
        assert_eq!(event.tokens.cached_input_tokens, 900);
        assert_eq!(event.tokens.total(), 1_095);
        assert_eq!(event.model, "claude-sonnet-4-5");
        assert_eq!(event.session.as_deref(), Some("s-1"));
    }

    #[test]
    fn a_turn_without_usage_is_not_an_event() {
        let line = json!({ "type": "assistant", "message": { "model": "x" } });
        assert!(event_of(&line, now()).is_none());
        let empty = json!({
            "type": "assistant",
            "message": { "usage": { "input_tokens": 0, "output_tokens": 0 } }
        });
        assert!(event_of(&empty, now()).is_none());
    }

    #[test]
    fn a_user_message_is_not_counted_as_usage() {
        let line = json!({ "type": "user", "message": { "usage": { "input_tokens": 5 } } });
        assert!(event_of(&line, now()).is_none());
    }

    #[test]
    fn the_quota_reading_keeps_both_windows() {
        let payload = json!({
            "five_hour": { "utilization": 62, "resets_at": "2026-11-15T15:00:00Z" },
            "seven_day": { "utilization": 18 },
            "extra_usage": { "is_enabled": true, "monthly_limit": 100, "used_credits": 40, "currency": "usd" }
        });
        let reading = parse_quota(&payload, Some("max"), now()).unwrap();
        assert_eq!(reading.windows.len(), 2);
        assert_eq!(reading.windows[0].used_percent, 62.0);
        assert_eq!(reading.credits.unwrap().remaining, 60.0);
        assert_eq!(reading.identity.unwrap().plan.as_deref(), Some("max"));
    }

    #[test]
    fn a_quota_payload_without_numbers_is_rejected() {
        assert!(parse_quota(&json!({ "five_hour": {} }), None, now()).is_err());
        assert!(parse_quota(&json!([]), None, now()).is_err());
    }
}
