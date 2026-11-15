//! What one provider answers when it is refreshed.
//!
//! A reading is deliberately permissive: a tool may report quotas, a plan, a credit
//! balance, per-model local totals, or nothing but an error. The UI decides what to
//! draw; the domain only insists that every value says where it came from.

use serde::{Deserialize, Serialize};

/// Where the numbers in a reading were obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSource {
    /// Parsed from files the tool already wrote on this machine.
    LocalSessions,
    /// Read from the provider's own usage endpoint with the CLI's token.
    Oauth,
    /// Read from a provider or gateway quota endpoint.
    ProviderQuota,
    /// Restored from the cloud copy of the history.
    CloudHistory,
}

/// The unit a quota metric is expressed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaUnit {
    Tokens,
    Credits,
    Requests,
    Messages,
    Dollars,
    Percent,
    Minutes,
    Unknown,
}

/// One measurable slice of a plan: tokens left, credits spent, minutes used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaMetric {
    pub id: String,
    pub label: String,
    pub unit: QuotaUnit,
    pub used: Option<f64>,
    pub limit: Option<f64>,
    pub remaining: Option<f64>,
    pub reset_at: Option<String>,
}

impl QuotaMetric {
    /// Fills in whichever of used/limit/remaining the provider left out, so the UI
    /// never has to guess. Returns `None` when nothing numeric was reported at all.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        unit: QuotaUnit,
        used: Option<f64>,
        limit: Option<f64>,
        remaining: Option<f64>,
        reset_at: Option<String>,
    ) -> Option<Self> {
        let (mut used, mut limit, mut remaining) = (used, limit, remaining);
        if used.is_none() && limit.is_none() && remaining.is_none() {
            return None;
        }
        if used.is_none() {
            if let (Some(limit), Some(remaining)) = (limit, remaining) {
                used = Some((limit - remaining).max(0.0));
            }
        }
        if limit.is_none() {
            if let (Some(used), Some(remaining)) = (used, remaining) {
                limit = Some(used + remaining);
            }
        }
        if remaining.is_none() {
            if let (Some(limit), Some(used)) = (limit, used) {
                remaining = Some((limit - used).max(0.0));
            }
        }
        Some(Self {
            id: id.into(),
            label: label.into(),
            unit,
            used,
            limit,
            remaining,
            reset_at,
        })
    }

}

/// A plan window: the 5-hour session, the week, an API budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingWindow {
    pub kind: String,
    pub label: String,
    pub used_percent: f64,
    pub remaining_percent: f64,
    pub reset_at: Option<String>,
}

impl ReadingWindow {
    pub fn new(kind: impl Into<String>, label: impl Into<String>, used_percent: f64, reset_at: Option<String>) -> Self {
        let used = used_percent.clamp(0.0, 100.0);
        Self {
            kind: kind.into(),
            label: label.into(),
            used_percent: used,
            remaining_percent: 100.0 - used,
            reset_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderIdentity {
    pub plan: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Credits {
    pub remaining: f64,
    pub unit: String,
}

/// A failure an adapter can explain to the user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct ProviderError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl ProviderError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }

    pub fn invalid(provider: &str) -> Self {
        Self::new(
            "invalid_response",
            format!("{provider} returned an invalid usage response."),
            false,
        )
    }

    pub fn unreachable(provider: &str) -> Self {
        Self::new(
            "network_error",
            format!("{provider} usage could not be reached. Try again."),
            true,
        )
    }

    pub fn unauthorized(provider: &str) -> Self {
        Self::new(
            "auth_required",
            format!("{provider} rejected this credential. Update it in Settings → Sources."),
            false,
        )
    }
}

/// One provider after a refresh.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderReading {
    pub source: ProviderSource,
    #[serde(default)]
    pub windows: Vec<ReadingWindow>,
    #[serde(default)]
    pub metrics: Vec<QuotaMetric>,
    #[serde(default)]
    pub identity: Option<ProviderIdentity>,
    #[serde(default)]
    pub credits: Option<Credits>,
    /// The account a login belongs to, or `local` for device-local logs.
    #[serde(default)]
    pub account_key: Option<String>,
    /// The day this reading describes, when it describes one.
    #[serde(default)]
    pub day: Option<crate::LocalDay>,
    #[serde(default)]
    pub error: Option<ProviderError>,
    /// RFC 3339, always present: the UI shows staleness, not just values.
    pub updated_at: String,
}

impl ProviderReading {
    /// A reading that failed but still carries what was read locally: an expired
    /// token must not hide yesterday's totals.
    pub fn degraded(source: ProviderSource, error: ProviderError, updated_at: String) -> Self {
        Self {
            source,
            windows: Vec::new(),
            metrics: Vec::new(),
            identity: None,
            credits: None,
            account_key: None,
            day: None,
            error: Some(error),
            updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_metric_fills_in_the_missing_side() {
        let limit_only = QuotaMetric::new("tokens", "Tokens", QuotaUnit::Tokens, Some(40.0), Some(100.0), None, None).unwrap();
        assert_eq!(limit_only.remaining, Some(60.0));

        let remaining_only = QuotaMetric::new("credits", "Credits", QuotaUnit::Credits, None, Some(10.0), Some(2.0), None).unwrap();
        assert_eq!(remaining_only.used, Some(8.0));

        assert!(QuotaMetric::new("empty", "Empty", QuotaUnit::Unknown, None, None, None, None).is_none());
    }

    #[test]
    fn a_window_clamps_to_a_hundred() {
        let window = ReadingWindow::new("weekly", "Weekly", 140.0, None);
        assert_eq!(window.used_percent, 100.0);
        assert_eq!(window.remaining_percent, 0.0);
    }
}
