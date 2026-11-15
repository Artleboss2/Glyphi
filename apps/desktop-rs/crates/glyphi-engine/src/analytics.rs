//! The per-provider analytics block the dashboard reads.
//!
//! `LocalUsageAnalytics` in `packages/contracts` is flat and denormalised on
//! purpose: the renderer sums providers, splits by day and by model and draws the
//! hourly trend, all from one snapshot. Building that block here — once per
//! provider, from the same ledger the totals come from — keeps the contract intact
//! without asking the window to run queries.

use chrono::Utc;
use glyphi_domain::{LocalDay, ProviderError, UsageTotals};
use glyphi_store::{Store, StoreError};
use serde::{Deserialize, Serialize};

use crate::settings::EngineSettings;

/// `UsageTotals` as the contract writes it: one flat object, cost in dollars.
///
/// The ledger keeps the four token categories nested and the cost in integer
/// micro-dollars; presentation wants them side by side and rounded once.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub requests: u64,
    pub estimated_cost_usd: Option<f64>,
    pub unpriced_tokens: u64,
}

impl Totals {
    fn of(totals: UsageTotals) -> Self {
        Self {
            input_tokens: totals.tokens.input_tokens,
            cached_input_tokens: totals.tokens.cached_input_tokens,
            cache_creation_input_tokens: totals.tokens.cache_creation_input_tokens,
            output_tokens: totals.tokens.output_tokens,
            total_tokens: totals.tokens.total(),
            requests: totals.requests,
            estimated_cost_usd: totals.estimated_cost_usd(),
            unpriced_tokens: totals.unpriced_tokens,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyMetric {
    pub date: String,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HourlyMetric {
    pub date: String,
    pub hour: u32,
    /// The UTC instant this bucket starts at. Left `None`: a local hour inside a
    /// daylight-saving jump has no single UTC start, and the window only needs the
    /// hour number to draw the trend.
    pub utc_start: Option<String>,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Breakdown {
    pub id: String,
    pub label: String,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyModelMetric {
    pub date: String,
    pub id: String,
    pub label: String,
    #[serde(flatten)]
    pub totals: Totals,
}

/// One provider's local-history view, as `DashboardProvider.analytics`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAnalytics {
    /// `available`, `no_data`, `partial` or `unavailable`.
    pub status: String,
    /// Always `local_sessions`: the Rust core has no remote history yet.
    pub source: String,
    pub history_days: u32,
    pub coverage_start: String,
    pub coverage_end: String,
    pub updated_at: String,
    pub files_scanned: u64,
    pub records_processed: u64,
    pub totals: Totals,
    pub today: Totals,
    pub daily: Vec<DailyMetric>,
    pub hourly: Vec<HourlyMetric>,
    pub models: Vec<Breakdown>,
    pub daily_models: Vec<DailyModelMetric>,
    pub projects: Vec<Breakdown>,
    pub sessions: Vec<Breakdown>,
    pub service_tiers: Vec<Breakdown>,
    pub error: Option<ProviderError>,
}

/// Reads one provider's window out of the ledger and shapes it for the dashboard.
///
/// `updated_at` comes from the reading, so the window can shade numbers as stale
/// without a second clock.
pub fn build(
    store: &Store,
    provider: &str,
    settings: &EngineSettings,
    updated_at: &str,
    error: Option<ProviderError>,
) -> Result<ProviderAnalytics, StoreError> {
    let zone = settings.zone();
    let today = LocalDay::of(Utc::now(), zone);
    let first = today.nth_prior(settings.history_days);

    let mut daily = Vec::new();
    let mut window_totals = UsageTotals::default();
    let mut today_totals = UsageTotals::default();
    for row in store.days(first, today)?.into_iter().filter(|row| row.provider == provider) {
        window_totals = window_totals.saturating_add(row.totals);
        if row.day == today {
            today_totals = today_totals.saturating_add(row.totals);
        }
        daily.push(DailyMetric {
            date: row.day.as_key(),
            totals: Totals::of(row.totals),
        });
    }

    let daily_models: Vec<DailyModelMetric> = store
        .models_range(provider, first, today)?
        .into_iter()
        .map(|(day, row)| DailyModelMetric {
            date: day.as_key(),
            id: row.model.clone(),
            label: row.model,
            totals: Totals::of(UsageTotals::of(row.totals, row.requests)),
        })
        .collect();

    // The window's model total is the sum of its days, so one query answers both
    // `models` and `dailyModels` and the two can never disagree.
    let mut by_model: Vec<Breakdown> = Vec::new();
    for row in &daily_models {
        let totals = Totals {
            input_tokens: row.totals.input_tokens,
            cached_input_tokens: row.totals.cached_input_tokens,
            cache_creation_input_tokens: row.totals.cache_creation_input_tokens,
            output_tokens: row.totals.output_tokens,
            total_tokens: row.totals.total_tokens,
            requests: row.totals.requests,
            estimated_cost_usd: row.totals.estimated_cost_usd,
            unpriced_tokens: row.totals.unpriced_tokens,
        };
        match by_model.iter_mut().find(|entry| entry.id == row.id) {
            Some(entry) => {
                entry.totals.input_tokens += totals.input_tokens;
                entry.totals.cached_input_tokens += totals.cached_input_tokens;
                entry.totals.cache_creation_input_tokens += totals.cache_creation_input_tokens;
                entry.totals.output_tokens += totals.output_tokens;
                entry.totals.total_tokens += totals.total_tokens;
                entry.totals.requests += totals.requests;
                entry.totals.unpriced_tokens += totals.unpriced_tokens;
                entry.totals.estimated_cost_usd = match (
                    entry.totals.estimated_cost_usd,
                    totals.estimated_cost_usd,
                ) {
                    (None, None) => None,
                    (left, right) => Some(left.unwrap_or(0.0) + right.unwrap_or(0.0)),
                };
            }
            None => by_model.push(Breakdown {
                id: row.id.clone(),
                label: row.label.clone(),
                totals,
            }),
        }
    }
    by_model.sort_by(|left, right| right.totals.total_tokens.cmp(&left.totals.total_tokens));

    let hourly = store
        .hours(today, zone, Some(provider))?
        .into_iter()
        .map(|row| HourlyMetric {
            date: today.as_key(),
            hour: row.hour,
            utc_start: None,
            totals: Totals::of(UsageTotals::of(row.totals, row.requests)),
        })
        .collect();

    let totals = Totals::of(store.totals(Some(provider))?);
    let status = if totals.total_tokens == 0 && daily.is_empty() {
        if error.is_some() {
            "unavailable"
        } else {
            "no_data"
        }
    } else {
        "available"
    };

    Ok(ProviderAnalytics {
        status: status.to_string(),
        source: "local_sessions".to_string(),
        history_days: settings.history_days,
        coverage_start: first.as_key(),
        coverage_end: today.as_key(),
        updated_at: updated_at.to_string(),
        // The core does not count files and records separately yet; requests is the
        // honest denominator it does have.
        files_scanned: 0,
        records_processed: totals.requests,
        totals,
        today: Totals::of(today_totals),
        daily,
        hourly,
        models: by_model,
        daily_models,
        // Projects, sessions and service tiers need column-level aggregates the
        // ledger does not project yet. Empty lists are valid answers to "nothing
        // recorded"; the window hides the panels.
        projects: Vec::new(),
        sessions: Vec::new(),
        service_tiers: Vec::new(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyphi_domain::TokenCounts;
    use glyphi_providers::UsageEvent;

    #[test]
    fn a_provider_sees_only_its_own_tokens() {
        let mut store = Store::open_in_memory().unwrap();
        let now = Utc::now();
        let at = now - chrono::Duration::hours(1);
        let counts = TokenCounts {
            input_tokens: 10,
            output_tokens: 5,
            ..TokenCounts::default()
        };
        store
            .insert_events(
                &[
                    UsageEvent::new("claude", "local", "sonnet", at, counts),
                    UsageEvent::new("codex", "local", "gpt", at, counts),
                ],
                |at| LocalDay::of(at, chrono_tz::UTC),
                now.timestamp_millis(),
            )
            .unwrap();
        store
            .rebuild_days(&[LocalDay::of(at, chrono_tz::UTC), LocalDay::of(now, chrono_tz::UTC)])
            .unwrap();

        let mut settings = EngineSettings::default();
        settings.history_days = 7;
        let analytics = build(&store, "claude", &settings, &now.to_rfc3339(), None).unwrap();
        assert_eq!(analytics.totals.total_tokens, 15);
        assert_eq!(analytics.totals.requests, 1);
        assert_eq!(analytics.status, "available");
        assert_eq!(analytics.models.len(), 1);
        assert_eq!(analytics.models[0].label, "sonnet");
        assert_eq!(analytics.coverage_end, LocalDay::of(now, chrono_tz::UTC).as_key());
    }

    #[test]
    fn a_provider_without_rows_is_no_data_rather_than_empty_available() {
        let store = Store::open_in_memory().unwrap();
        let analytics = build(&store, "claude", &EngineSettings::default(), "2026-11-15T09:00:00Z", None)
            .unwrap();
        assert_eq!(analytics.status, "no_data");
        assert!(analytics.daily.is_empty());
        // A day with no rows still has its twenty-four hours, all at zero: the
        // trend is flat, not missing.
        assert_eq!(analytics.hourly.len(), 24);
        assert!(analytics.hourly.iter().all(|row| row.totals.total_tokens == 0));
    }
}
