//! The counting proxy, as a provider of its own.
//!
//! `glyphi-proxy` listens on loopback and counts every request routed through
//! `http://127.0.0.1:4787/p/<provider>`. Its totals are the only usage some tools
//! will ever produce, and this adapter is the window onto `/__glyphi/usage`.
//!
//! Sixteen metrics is the contract, so twelve providers are named and everything
//! past them is summed into one line: a busy machine cannot break the reading.

use chrono::{DateTime, Utc};
use glyphi_domain::{
    LocalDay, ProviderError, ProviderReading, ProviderSource, QuotaMetric, QuotaUnit, TokenCounts,
};
use serde_json::Value;

use crate::adapter::{number, Adapter, ProviderSnapshot, ScanContext, UsageEvent};

pub const PROXY_ORIGIN: &str = "http://127.0.0.1:4787";
const MAX_NAMED_PROVIDERS: usize = 12;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4);

pub struct ProxyAdapter {
    origin: String,
}

impl Default for ProxyAdapter {
    fn default() -> Self {
        Self {
            origin: std::env::var("GLYPHI_PROXY_ORIGIN").unwrap_or_else(|_| PROXY_ORIGIN.to_string()),
        }
    }
}

impl ProxyAdapter {
    pub fn with_origin(origin: impl Into<String>) -> Self {
        Self {
            origin: origin.into().trim_end_matches('/').to_string(),
        }
    }

    fn get(&self, path: &str) -> Result<Value, ProviderError> {
        ureq::get(&format!("{}{path}", self.origin))
            .timeout(TIMEOUT)
            .call()
            .map_err(|error| match error {
                ureq::Error::Status(status, _) => ProviderError::new(
                    format!("http_{status}"),
                    "The counting proxy answered with an error.",
                    true,
                ),
                _ => ProviderError::new(
                    "proxy_unreachable",
                    "The counting proxy is not running. It starts with Glyphi; if it was stopped, relaunch the app.",
                    true,
                ),
            })?
            .into_json()
            .map_err(|_| ProviderError::invalid("The counting proxy"))
    }
}

impl Adapter for ProxyAdapter {
    fn id(&self) -> &str {
        "proxy"
    }

    fn name(&self) -> &str {
        "Counting proxy"
    }

    fn available(&self) -> bool {
        self.get("/__glyphi/health").is_ok()
    }

    fn refresh(&self, context: &ScanContext<'_>) -> ProviderSnapshot {
        match self.get("/__glyphi/usage") {
            Ok(payload) => {
                let day = LocalDay::of(context.now, context.zone);
                // The proxy's own JSONL log can be rotated between two refreshes;
                // emitting events now is what keeps the ledger and the gauges in
                // agreement about a day the log no longer describes.
                let events = events_of(&payload, day, context.now);
                ProviderSnapshot::new(self.id(), self.name(), reading_of(&payload, context.now, context.zone))
                    .with_events(events)
            }
            Err(error) => ProviderSnapshot::new(
                self.id(),
                self.name(),
                ProviderReading::degraded(
                    ProviderSource::ProviderQuota,
                    error,
                    context.now.to_rfc3339(),
                ),
            ),
        }
    }
}

/// Today's per-provider totals as the proxy counted them, busiest first.
pub fn today_usage(payload: &Value, day: LocalDay) -> Vec<(String, TokenCounts)> {
    let mut totals: Vec<(String, TokenCounts)> = Vec::new();
    let Some(days) = payload.get("days").and_then(Value::as_array) else {
        return totals;
    };
    for entry in days {
        if entry.get("day").and_then(Value::as_str) != Some(day.as_key().as_str()) {
            continue;
        }
        let Some(providers) = entry.get("providers").and_then(Value::as_object) else {
            continue;
        };
        for (provider, usage) in providers {
            let input = number(usage.get("input")).unwrap_or(0.0).round() as u64;
            let output = number(usage.get("output")).unwrap_or(0.0).round() as u64;
            if input == 0 && output == 0 {
                continue;
            }
            totals.push((
                provider.clone(),
                TokenCounts {
                    input_tokens: input,
                    cached_input_tokens: 0,
                    cache_creation_input_tokens: 0,
                    output_tokens: output,
                },
            ));
        }
    }
    totals.sort_by(|left, right| right.1.total().cmp(&left.1.total()));
    totals
}

fn reading_of(payload: &Value, now: DateTime<Utc>, zone: chrono_tz::Tz) -> ProviderReading {
    let day = LocalDay::of(now, zone);
    let usage = today_usage(payload, day);
    if usage.is_empty() {
        return ProviderReading::degraded(
            ProviderSource::ProviderQuota,
            ProviderError::new("proxy_idle", "Nothing has gone through the counting proxy today.", false),
            now.to_rfc3339(),
        );
    }

    let input: u64 = usage.iter().map(|(_, tokens)| tokens.input_tokens).sum();
    let output: u64 = usage.iter().map(|(_, tokens)| tokens.output_tokens).sum();

    let mut metrics = vec![
        QuotaMetric::new("proxy_tokens_today", "Tokens through the proxy today", QuotaUnit::Tokens, Some((input + output) as f64), None, None, None),
        QuotaMetric::new("proxy_input_today", "Input through the proxy today", QuotaUnit::Tokens, Some(input as f64), None, None, None),
        QuotaMetric::new("proxy_output_today", "Output through the proxy today", QuotaUnit::Tokens, Some(output as f64), None, None, None),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    for (provider, tokens) in usage.iter().take(MAX_NAMED_PROVIDERS) {
        if let Some(metric) = QuotaMetric::new(
            format!("proxy_{provider}"),
            format!("{provider} through the proxy"),
            QuotaUnit::Tokens,
            Some(tokens.total() as f64),
            None,
            None,
            None,
        ) {
            metrics.push(metric);
        }
    }

    let overflow = &usage[usage.len().min(MAX_NAMED_PROVIDERS)..];
    if !overflow.is_empty() {
        if let Some(metric) = QuotaMetric::new(
            "proxy_other",
            format!(
                "{} more {} through the proxy",
                overflow.len(),
                if overflow.len() == 1 { "provider" } else { "providers" }
            ),
            QuotaUnit::Tokens,
            Some(overflow.iter().map(|(_, tokens)| tokens.total()).sum::<u64>() as f64),
            None,
            None,
            None,
        ) {
            metrics.push(metric);
        }
    }

    ProviderReading {
        source: ProviderSource::ProviderQuota,
        windows: Vec::new(),
        metrics,
        identity: None,
        credits: None,
        account_key: Some("loopback".to_string()),
        day: Some(day),
        error: None,
        updated_at: now.to_rfc3339(),
    }
}

/// One event per provider that went through the proxy today, so the ledger and the
/// quota screen agree even if the JSONL log is rotated between two refreshes.
pub fn events_of(payload: &Value, day: LocalDay, now: DateTime<Utc>) -> Vec<UsageEvent> {
    today_usage(payload, day)
        .into_iter()
        .map(|(provider, tokens)| UsageEvent::new("proxy", provider, "proxy", now, tokens))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;
    use serde_json::json;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 15, 12, 0, 0).unwrap()
    }

    fn payload(providers: Value) -> Value {
        json!({ "days": [{ "day": "2026-11-15", "providers": providers }] })
    }

    #[test]
    fn sixteen_metrics_at_most_however_many_providers() {
        let mut map = serde_json::Map::new();
        for index in 0..40 {
            map.insert(
                format!("provider-{index}"),
                json!({ "input": 100 - index, "output": 0, "requests": 1 }),
            );
        }
        let reading = reading_of(&payload(Value::Object(map)), now(), chrono_tz::UTC);
        assert_eq!(reading.metrics.len(), 16);
        assert!(reading.metrics.iter().any(|metric| metric.id == "proxy_other"));
        let other = reading.metrics.iter().find(|metric| metric.id == "proxy_other").unwrap();
        assert!(other.used.unwrap() > 0.0);
    }

    #[test]
    fn a_quiet_day_is_not_an_error() {
        let reading = reading_of(&payload(json!({})), now(), chrono_tz::UTC);
        assert_eq!(reading.error.unwrap().code, "proxy_idle");
    }

    #[test]
    fn another_day_is_ignored() {
        let value = json!({ "days": [{ "day": "2026-11-14", "providers": { "openai": { "input": 5, "output": 5 } } }] });
        assert!(today_usage(&value, LocalDay::of(now(), chrono_tz::UTC)).is_empty());
    }

    #[test]
    fn totals_add_up_across_providers() {
        let value = payload(json!({
            "openai": { "input": 900, "output": 150 },
            "anthropic": { "input": 320, "output": 80 }
        }));
        // Sorted: openai is the busiest, so its line comes before anthropic's.
        let reading = reading_of(&value, now(), chrono_tz::UTC);
        let total = reading.metrics.iter().find(|metric| metric.id == "proxy_tokens_today").unwrap();
        assert_eq!(total.used, Some(1450.0));
    }
}
