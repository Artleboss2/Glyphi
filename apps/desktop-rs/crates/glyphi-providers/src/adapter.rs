//! The one shape every provider implements.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use glyphi_domain::{ProviderReading, TokenCounts};

/// A credential an adapter may need, already resolved from wherever the CLI keeps
/// it. The secret is never logged and never leaves this struct.
#[derive(Clone)]
pub struct Credential {
    pub secret: String,
    pub plan: Option<String>,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Printing the secret is the one mistake that turns a debug line into an
        // incident, so the type refuses to do it.
        formatter
            .debug_struct("Credential")
            .field("secret", &"<redacted>")
            .field("plan", &self.plan)
            .finish()
    }
}

/// Where the adapters look for the credentials and the logs they need.
#[derive(Debug, Clone)]
pub struct CredentialStore {
    pub home: PathBuf,
    pub credentials: BTreeMap<String, Credential>,
}

impl CredentialStore {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self {
            home: home.into(),
            credentials: BTreeMap::new(),
        }
    }

    pub fn with(mut self, provider: impl Into<String>, credential: Credential) -> Self {
        self.credentials.insert(provider.into(), credential);
        self
    }
}

/// Everything an adapter is allowed to know about "now".
pub struct ScanContext<'a> {
    pub now: DateTime<Utc>,
    pub zone: Tz,
    pub history_days: u32,
    /// Extra JSONL roots chosen in Settings, including mounted remote paths.
    pub extra_log_paths: &'a [PathBuf],
    /// Path fragments excluded before a record reaches any total.
    pub ignored_projects: &'a [String],
    pub credentials: &'a CredentialStore,
}

impl<'a> ScanContext<'a> {
    pub fn ignored(&self, path: &str) -> bool {
        self.ignored_projects
            .iter()
            .any(|fragment| !fragment.is_empty() && path.contains(fragment.as_str()))
    }
}

/// One measured request, as the source reported it.
///
/// The day is *not* stored here: an event carries the instant it happened, and the
/// engine buckets it in the reporting zone. Storing a precomputed day would bake in
/// a timezone the user may later change.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageEvent {
    pub provider: String,
    pub account: String,
    pub model: String,
    pub at: DateTime<Utc>,
    pub tokens: TokenCounts,
    pub session: Option<String>,
    pub project: Option<String>,
    /// True when the counts were derived from text rather than reported.
    pub estimated: bool,
}

impl UsageEvent {
    pub fn new(
        provider: impl Into<String>,
        account: impl Into<String>,
        model: impl Into<String>,
        at: DateTime<Utc>,
        tokens: TokenCounts,
    ) -> Self {
        Self {
            provider: provider.into(),
            account: account.into(),
            model: model.into(),
            at,
            tokens,
            session: None,
            project: None,
            estimated: false,
        }
    }

    /// A stable per-source key, so a rescan of the same file cannot count twice.
    pub fn fingerprint(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.provider.as_bytes());
        hasher.update(b"\x1f");
        hasher.update(self.account.as_bytes());
        hasher.update(b"\x1f");
        hasher.update(self.model.as_bytes());
        hasher.update(b"\x1f");
        hasher.update(self.at.timestamp_millis().to_le_bytes());
        hasher.update(b"\x1f");
        hasher.update(self.tokens.total().to_le_bytes());
        hasher.update(b"\x1f");
        hasher.update(self.session.as_deref().unwrap_or("").as_bytes());
        let digest = hasher.finalize();
        digest[..16].iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

/// What a refresh produced.
#[derive(Debug, Clone)]
pub struct ProviderSnapshot {
    pub id: String,
    pub name: String,
    pub reading: ProviderReading,
    pub events: Vec<UsageEvent>,
}

impl ProviderSnapshot {
    pub fn new(id: impl Into<String>, name: impl Into<String>, reading: ProviderReading) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            reading,
            events: Vec::new(),
        }
    }

    pub fn with_events(mut self, events: Vec<UsageEvent>) -> Self {
        self.events = events;
        self
    }
}

/// One provider.
///
/// Blocking on purpose: a refresh runs on its own thread and a handful of requests
/// per refresh does not justify an async runtime in the core.
pub trait Adapter: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;

    /// Cheap check for "is this tool installed". Never returns an error: an absent
    /// tool is a normal machine, not a failure to report.
    fn available(&self) -> bool;

    fn refresh(&self, context: &ScanContext<'_>) -> ProviderSnapshot;
}

/// The home directory the adapters read from, resolved once.
pub fn home_directory() -> std::path::PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
}

/// Reads a JSON value as a number, accepting the string forms providers send.
pub fn number(value: Option<&serde_json::Value>) -> Option<f64> {
    match value? {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

pub fn positive(value: Option<&serde_json::Value>) -> Option<f64> {
    number(value).filter(|value| value.is_finite() && *value >= 0.0)
}

pub fn text(value: Option<&serde_json::Value>) -> Option<String> {
    let raw = value?.as_str()?.trim();
    if raw.is_empty() {
        None
    } else {
        Some(raw.to_string())
    }
}

/// Parses an RFC 3339 stamp, or a Unix timestamp in seconds or milliseconds.
pub fn instant(value: Option<&serde_json::Value>) -> Option<DateTime<Utc>> {
    match value? {
        serde_json::Value::String(text) => DateTime::parse_from_rfc3339(text.trim())
            .ok()
            .map(|parsed| parsed.with_timezone(&Utc)),
        serde_json::Value::Number(number) => {
            let raw = number.as_i64()?;
            let seconds = if raw > 100_000_000_000 { raw / 1_000 } else { raw };
            DateTime::<Utc>::from_timestamp(seconds, 0)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_arrive_as_numbers_or_strings() {
        assert_eq!(number(Some(&json!(12.5))), Some(12.5));
        assert_eq!(number(Some(&json!("12.5"))), Some(12.5));
        assert_eq!(positive(Some(&json!(-1))), None);
        assert_eq!(number(Some(&json!(null))), None);
    }

    #[test]
    fn instants_accept_both_epoch_shapes() {
        let seconds = instant(Some(&json!(1_765_000_000))).unwrap();
        let millis = instant(Some(&json!(1_765_000_000_000i64))).unwrap();
        assert_eq!(seconds, millis);
        assert!(instant(Some(&json!("not a date"))).is_none());
    }

    #[test]
    fn a_fingerprint_ignores_nothing_it_should_keep() {
        let at = DateTime::<Utc>::from_timestamp(1_765_000_000, 0).unwrap();
        let base = UsageEvent::new("claude", "local", "sonnet", at, TokenCounts::default());
        let mut other = base.clone();
        other.account = "someone-else".into();
        assert_ne!(base.fingerprint(), other.fingerprint());
    }
}
