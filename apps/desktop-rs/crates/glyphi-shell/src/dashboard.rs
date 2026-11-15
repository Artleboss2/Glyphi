//! The renderer's `DashboardSnapshot`, built from the engine's snapshot.
//!
//! The two shapes are close but not the same, and the difference is deliberate.
//! The engine's snapshot is a ledger: days, models, hours, all providers' rows
//! together. `DashboardSnapshot` in `packages/contracts` is a presentation
//! envelope: one entry per provider, with that provider's analytics attached, and
//! `schemaVersion` / `staleAfterSeconds` / `host` around it.
//!
//! Keeping the translation here means the React app runs unchanged, and it keeps
//! the engine free of anything that only exists to be drawn.

use serde::Serialize;
use serde_json::Value;

use glyphi_domain::{
    Credits, ProviderError, ProviderIdentity, ProviderSource, QuotaMetric, ReadingWindow,
};
use glyphi_engine::{ProviderAnalytics, Snapshot};

/// `DASHBOARD_SCHEMA_VERSION` in `packages/contracts`.
const SCHEMA_VERSION: u32 = 2;

/// How long the window may show these numbers before calling them stale. The
/// Electron engine used the same three minutes.
const STALE_AFTER_SECONDS: u32 = 180;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub schema_version: u32,
    pub generated_at: String,
    pub stale_after_seconds: u32,
    pub host: Host,
    pub providers: Vec<DashboardProvider>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Host {
    pub engine: &'static str,
    pub version: &'static str,
    pub platform: &'static str,
    pub architecture: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardProvider {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub source: &'static str,
    pub windows: Vec<ReadingWindow>,
    pub quota_metrics: Vec<QuotaMetric>,
    pub identity: Option<ProviderIdentity>,
    pub credits: Option<Credits>,
    pub analytics: Option<ProviderAnalytics>,
    pub error: Option<ProviderError>,
    pub updated_at: Option<String>,
}

/// The envelope the renderer's `DashboardState` expects: the snapshot sits under
/// `snapshot`, next to the flags the refresh screens read.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardState {
    pub revision: u64,
    pub snapshot: DashboardSnapshot,
    pub refreshing: bool,
    pub progress: Option<Value>,
    pub error: Option<String>,
    pub local_import: Option<Value>,
}

/// Wraps a snapshot in the state the page updates from.
pub fn state(snapshot: &Snapshot) -> DashboardState {
    DashboardState {
        revision: 1,
        snapshot: from_snapshot(snapshot),
        refreshing: false,
        progress: None,
        error: None,
        local_import: None,
    }
}

/// Builds the envelope the renderer already knows how to read.
pub fn from_snapshot(snapshot: &Snapshot) -> DashboardSnapshot {
    DashboardSnapshot {
        schema_version: SCHEMA_VERSION,
        generated_at: snapshot.generated_at.clone(),
        stale_after_seconds: STALE_AFTER_SECONDS,
        host: Host {
            engine: "rust",
            version: env!("CARGO_PKG_VERSION"),
            platform: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
        },
        providers: snapshot
            .providers
            .iter()
            .map(|provider| DashboardProvider {
                id: provider.id.clone(),
                name: provider.name.clone(),
                // Every provider in a snapshot was available to the last scan, so
                // there is no separate enabled flag to consult yet.
                enabled: true,
                source: source_name(provider.reading.source),
                windows: provider.reading.windows.clone(),
                quota_metrics: provider.reading.metrics.clone(),
                identity: provider.reading.identity.clone(),
                credits: provider.reading.credits.clone(),
                analytics: provider.analytics.clone(),
                error: provider.reading.error.clone(),
                updated_at: Some(provider.reading.updated_at.clone()),
            })
            .collect(),
    }
}

/// The contract types `source` as a plain string; the domain enum serialises to
/// these three words, and naming them here keeps a rename a compile error.
fn source_name(source: ProviderSource) -> &'static str {
    match source {
        ProviderSource::LocalSessions => "local_sessions",
        ProviderSource::Oauth => "oauth",
        ProviderSource::ProviderQuota => "provider_quota",
        ProviderSource::CloudHistory => "cloud_history",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyphi_domain::ProviderReading;

    #[test]
    fn a_provider_becomes_the_shape_the_dashboard_reads() {
        let mut reading = ProviderReading::degraded(
            ProviderSource::ProviderQuota,
            ProviderError::new("auth_required", "Sign in again.", false),
            "2026-11-15T09:00:00Z".to_string(),
        );
        reading.windows.push(ReadingWindow::new("session", "5-hour session", 42.0, None));

        let snapshot = Snapshot {
            generated_at: "2026-11-15T09:00:00Z".to_string(),
            day: glyphi_domain::LocalDay::parse("2026-11-15").unwrap(),
            reporting_zone: "UTC".to_string(),
            providers: vec![glyphi_engine::SnapshotProvider {
                id: "claude".to_string(),
                name: "Claude Code".to_string(),
                reading,
                analytics: None,
            }],
            days: Vec::new(),
            models: Vec::new(),
            hours: Vec::new(),
            totals: Default::default(),
        };

        let dashboard = from_snapshot(&snapshot);
        let value = serde_json::to_value(&dashboard).unwrap();
        assert_eq!(value["schemaVersion"], 2);
        assert_eq!(value["host"]["engine"], "rust");
        assert_eq!(value["providers"][0]["id"], "claude");
        assert_eq!(value["providers"][0]["source"], "provider_quota");
        assert_eq!(value["providers"][0]["windows"][0]["usedPercent"], 42.0);
        assert_eq!(value["providers"][0]["error"]["code"], "auth_required");
    }
}
