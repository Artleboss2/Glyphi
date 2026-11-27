//! The engine.
//!
//! A refresh is: ask every adapter, store what it proved, rebuild the days it
//! touched, then answer with a snapshot. The order matters — the day rows the UI
//! reads must never be a half-written refresh — so the projection is rebuilt inside
//! the same call that stored the events.

use chrono::Utc;
use glyphi_domain::{LocalDay, ProviderReading, UsageTotals};
use glyphi_providers::adapter::ScanContext;
use glyphi_providers::{Adapter, CredentialStore, UsageEvent};
use glyphi_store::{DayRow, HourRow, ModelRow, Store};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::analytics::{self, ProviderAnalytics};
use crate::protocol::{
    EngineError, EngineMessage, EngineMethod, EngineProgress, EngineRequest, EngineResponse,
    ProgressStatus,
};
use crate::settings::EngineSettings;

/// What the window paints from one call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub generated_at: String,
    pub day: LocalDay,
    pub reporting_zone: String,
    pub providers: Vec<SnapshotProvider>,
    pub days: Vec<DayRow>,
    pub models: Vec<ModelRow>,
    /// Today, hour by hour in the reporting zone: the trend the window draws first.
    pub hours: Vec<HourRow>,
    pub totals: UsageTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotProvider {
    pub id: String,
    pub name: String,
    #[serde(flatten)]
    pub reading: ProviderReading,
    /// The provider's share of the local history, already shaped for the window.
    #[serde(default)]
    pub analytics: Option<ProviderAnalytics>,
}

/// A reading for a provider that exists only in the ledger: the numbers are in the
/// analytics beside it, and no quota endpoint has answered for it.
fn ledger_reading() -> ProviderReading {
    ProviderReading {
        source: glyphi_domain::ProviderSource::LocalSessions,
        windows: Vec::new(),
        metrics: Vec::new(),
        identity: None,
        credits: None,
        account_key: None,
        day: None,
        error: None,
        updated_at: Utc::now().to_rfc3339(),
    }
}

/// The name a ledger id is drawn under. An unknown id keeps its own spelling with a
/// capital, so a gateway added to the ledger tomorrow is readable today.
fn display_name(id: &str) -> String {
    match id {
        "freebuff" => "Freebuff".to_string(),
        "roo" => "Roo Code".to_string(),
        "codex" => "Codex".to_string(),
        "cline" => "Cline".to_string(),
        "claude" => "Claude".to_string(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => other.to_string(),
            }
        }
    }
}

pub struct Engine {
    store: Store,
    settings: EngineSettings,
    adapters: Vec<Box<dyn Adapter>>,
    credentials: CredentialStore,
    /// The last reading of each adapter. Kept so a one-card refresh can answer with
    /// the whole dashboard without rescanning every other provider.
    readings: Vec<SnapshotProvider>,
    shutting_down: bool,
}

impl Engine {
    pub fn new(store: Store, settings: EngineSettings, adapters: Vec<Box<dyn Adapter>>) -> Self {
        let credentials = CredentialStore::new(settings.home.clone());
        Self {
            store,
            settings,
            adapters,
            credentials,
            readings: Vec::new(),
            shutting_down: false,
        }
    }

    pub fn settings(&self) -> &EngineSettings {
        &self.settings
    }

    /// Answers one protocol request.
    pub fn answer(&mut self, request: EngineRequest) -> EngineResponse {
        let id = request.id.clone();
        let result = match request.method {
            // Reading the ledger is cheap and must not rescan a filesystem: the
            // window asks for a snapshot far more often than it asks for a refresh.
            EngineMethod::SnapshotGet => self
                .snapshot()
                .map(|snapshot| serde_json::to_value(snapshot).unwrap_or(Value::Null)),
            EngineMethod::RefreshAll => self
                .refresh_all(&mut |_| {})
                .map(|snapshot| serde_json::to_value(snapshot).unwrap_or(Value::Null)),
            EngineMethod::ProviderRefresh => self.refresh_one(&request.params),
            EngineMethod::ConfigUpdate => self.update_config(&request.params),
            EngineMethod::PricingRefresh => {
                // Pricing is compiled into the adapters for now; the method exists so
                // the window does not have to special-case its absence.
                Ok(serde_json::json!({ "refreshed": false, "reason": "pricing ships with the build" }))
            }
            EngineMethod::Cloud => Ok(serde_json::json!({
                "enabled": false,
                "reason": "cloud save is not part of the Rust core yet"
            })),
            EngineMethod::Shutdown => {
                self.shutting_down = true;
                Ok(serde_json::json!({ "shutdown": true }))
            }
        };
        match result {
            Ok(value) => EngineResponse::ok(id, value),
            Err(error) => EngineResponse::failed(id, error),
        }
    }

    pub fn should_stop(&self) -> bool {
        self.shutting_down
    }

    /// Refreshes every available adapter and returns the new snapshot.
    pub fn refresh_all(
        &mut self,
        progress: &mut dyn FnMut(EngineMessage),
    ) -> Result<Snapshot, EngineError> {
        let available: Vec<usize> = self
            .adapters
            .iter()
            .enumerate()
            .filter(|(_, adapter)| adapter.available())
            .map(|(index, _)| index)
            .collect();
        let total = available.len() as u32;
        let mut readings = Vec::with_capacity(total as usize);
        let mut events: Vec<UsageEvent> = Vec::new();

        for (completed, index) in available.iter().enumerate() {
            let adapter = &self.adapters[*index];
            progress(EngineMessage::Progress(EngineProgress {
                status: ProgressStatus::Started,
                completed: completed as u32,
                total,
                provider_id: Some(adapter.id().to_string()),
                provider_name: Some(adapter.name().to_string()),
            }));

            let snapshot = self.scan(adapter.as_ref());
            readings.push(SnapshotProvider {
                id: snapshot.id.clone(),
                name: snapshot.name.clone(),
                reading: snapshot.reading.clone(),
                analytics: None,
            });
            if let Ok(encoded) = serde_json::to_string(&snapshot.reading) {
                let account = snapshot
                    .reading
                    .account_key
                    .clone()
                    .unwrap_or_else(|| "local".to_string());
                let _ = self.store.save_capacity(&snapshot.id, &account, &encoded);
            }
            events.extend(snapshot.events);
        }

        let zone = self.settings.zone();
        let now_ms = Utc::now().timestamp_millis();
        self.store
            .insert_events(&events, |at| LocalDay::of(at, zone), now_ms)
            .map_err(|error| EngineError::new("store_unavailable", error.to_string(), true))?;

        self.rebuild_window()?;

        // A provider can exist only in the ledger: the Electron history arrived
        // without an adapter, and a gateway that has stopped still has days to show.
        // Leaving it out would count its usage and never draw it.
        let zone = self.settings.zone();
        let today = LocalDay::of(Utc::now(), zone);
        let first = today.nth_prior(self.settings.history_days);
        let known = self
            .store
            .providers_between(first, today)
            .map_err(|error| EngineError::new("store_unavailable", error.to_string(), true))?;
        for provider in known {
            if readings.iter().any(|reading| reading.id == provider) {
                continue;
            }
            readings.push(SnapshotProvider {
                name: display_name(&provider),
                id: provider,
                reading: ledger_reading(),
                analytics: None,
            });
        }

        // Keep the readings so a later single-card refresh can answer with the whole
        // dashboard, then build the snapshot from the ledger just written.
        self.readings = readings;
        let snapshot = self.snapshot()?;
        progress(EngineMessage::Progress(EngineProgress {
            status: ProgressStatus::Completed,
            completed: total,
            total,
            provider_id: None,
            provider_name: None,
        }));
        Ok(snapshot)
    }

    /// Runs one adapter on its own, for the refresh button on a provider card.
    fn refresh_one(&mut self, params: &Value) -> Result<Value, EngineError> {
        let id = params
            .get("providerId")
            .and_then(Value::as_str)
            .ok_or_else(|| EngineError::new("invalid_request", "providerId is required.", false))?;
        let index = self
            .adapters
            .iter()
            .position(|adapter| adapter.id() == id)
            .ok_or_else(|| EngineError::new("unknown_provider", format!("No adapter named {id}."), false))?;

        let scanned = self.scan(self.adapters[index].as_ref());
        let zone = self.settings.zone();
        self.store
            .insert_events(&scanned.events, |at| LocalDay::of(at, zone), Utc::now().timestamp_millis())
            .map_err(|error| EngineError::new("store_unavailable", error.to_string(), true))?;

        // Replace this provider's reading and keep the others: the window asked
        // about one card, and answering with only that card would blank the rest.
        let entry = SnapshotProvider {
            id: scanned.id.clone(),
            name: scanned.name.clone(),
            reading: scanned.reading.clone(),
            analytics: None,
        };
        match self.readings.iter_mut().find(|existing| existing.id == entry.id) {
            Some(existing) => *existing = entry,
            None => self.readings.push(entry),
        }

        self.rebuild_window()?;
        serde_json::to_value(self.snapshot()?)
            .map_err(|_| EngineError::new("invalid_response", "The snapshot could not be encoded.", false))
    }

    fn update_config(&mut self, params: &Value) -> Result<Value, EngineError> {
        self.settings.apply(params);
        self.credentials = CredentialStore::new(self.settings.home.clone());
        Ok(serde_json::to_value(&self.settings).unwrap_or(Value::Null))
    }

    fn scan(&self, adapter: &dyn Adapter) -> glyphi_providers::adapter::ProviderSnapshot {
        let zone = self.settings.zone();
        let context = ScanContext {
            now: Utc::now(),
            zone,
            history_days: self.settings.history_days,
            extra_log_paths: &self.settings.extra_log_paths,
            ignored_projects: &self.settings.ignored_projects,
            credentials: &self.credentials,
        };
        adapter.refresh(&context)
    }

    /// Rebuilds the days inside the configured window, so a parser change is
    /// reflected on the next refresh instead of persisting forever.
    fn rebuild_window(&mut self) -> Result<(), EngineError> {
        let zone = self.settings.zone();
        let today = LocalDay::of(Utc::now(), zone);
        let first = today.nth_prior(self.settings.history_days);
        let days = LocalDay::range(first, today).unwrap_or_else(|| vec![today]);
        self.store
            .rebuild_days(&days)
            .map_err(|error| EngineError::new("store_unavailable", error.to_string(), true))
    }

    fn snapshot(&self) -> Result<Snapshot, EngineError> {
        let zone = self.settings.zone();
        let today = LocalDay::of(Utc::now(), zone);
        let first = today.nth_prior(self.settings.history_days);
        let days = self
            .store
            .days(first, today)
            .map_err(|error| EngineError::new("store_unavailable", error.to_string(), true))?;
        let models = self
            .store
            .models(today, None)
            .map_err(|error| EngineError::new("store_unavailable", error.to_string(), true))?;
        let hours = self
            .store
            .hours(today, zone, None)
            .map_err(|error| EngineError::new("store_unavailable", error.to_string(), true))?;
        let totals = self
            .store
            .totals(None)
            .map_err(|error| EngineError::new("store_unavailable", error.to_string(), true))?;
        let providers = self
            .readings
            .iter()
            .map(|entry| {
                let mut provider = entry.clone();
                // A provider whose ledger read failed still gets its card: the
                // gauges it reported are worth more than a blank panel.
                provider.analytics = analytics::build(
                    &self.store,
                    &entry.id,
                    &self.settings,
                    &entry.reading.updated_at,
                    entry.reading.error.clone(),
                )
                .ok();
                provider
            })
            .collect();
        Ok(Snapshot {
            generated_at: Utc::now().to_rfc3339(),
            day: today,
            reporting_zone: self.settings.reporting_zone.clone(),
            providers,
            days,
            models,
            hours,
            totals,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyphi_domain::TokenCounts;
    use glyphi_providers::adapter::{ProviderSnapshot, ScanContext};
    use glyphi_store::DayRow;

    struct FixedAdapter {
        events: Vec<UsageEvent>,
    }

    impl Adapter for FixedAdapter {
        fn id(&self) -> &str {
            "fixed"
        }
        fn name(&self) -> &str {
            "Fixed"
        }
        fn available(&self) -> bool {
            true
        }
        fn refresh(&self, context: &ScanContext<'_>) -> ProviderSnapshot {
            ProviderSnapshot::new(
                self.id(),
                self.name(),
                ProviderReading::degraded(
                    glyphi_domain::ProviderSource::LocalSessions,
                    glyphi_domain::ProviderError::new("test", "Nothing configured.", false),
                    context.now.to_rfc3339(),
                ),
            )
            .with_events(self.events.clone())
        }
    }

    fn engine(events: Vec<UsageEvent>) -> Engine {
        Engine::new(
            Store::open_in_memory().unwrap(),
            EngineSettings::default(),
            vec![Box::new(FixedAdapter { events })],
        )
    }

    fn request(method: &str, params: Value) -> EngineRequest {
        EngineRequest {
            id: "1".into(),
            method: serde_json::from_value(Value::String(method.to_string())).unwrap(),
            params,
        }
    }

    #[test]
    fn a_refresh_stores_events_and_returns_days() {
        let at = Utc::now();
        let mut engine = engine(vec![UsageEvent::new(
            "fixed",
            "local",
            "model",
            at,
            TokenCounts {
                input_tokens: 100,
                output_tokens: 20,
                ..TokenCounts::default()
            },
        )]);
        let snapshot = engine.refresh_all(&mut |_| {}).unwrap();
        let day = snapshot.day;
        let rows: Vec<&DayRow> = snapshot.days.iter().filter(|row| row.day == day).collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].totals.tokens.total(), 120);
    }

    #[test]
    fn a_second_refresh_does_not_double_count() {
        let at = Utc::now();
        let mut engine = engine(vec![UsageEvent::new(
            "fixed",
            "local",
            "model",
            at,
            TokenCounts {
                input_tokens: 100,
                ..TokenCounts::default()
            },
        )]);
        engine.refresh_all(&mut |_| {}).unwrap();
        let snapshot = engine.refresh_all(&mut |_| {}).unwrap();
        assert_eq!(snapshot.totals.tokens.total(), 100);
    }

    #[test]
    fn config_update_clamps_and_reports_back() {
        let mut engine = engine(Vec::new());
        let response = engine.answer(request("config.update", serde_json::json!({ "historyDays": 5 })));
        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value["ok"], true);
        assert_eq!(value["result"]["historyDays"], 5);
        assert_eq!(engine.settings().history_days, 5);
    }

    #[test]
    fn shutdown_is_acknowledged_before_the_loop_stops() {
        let mut engine = engine(Vec::new());
        let response = engine.answer(request("shutdown", Value::Null));
        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value["result"]["shutdown"], true);
        assert!(engine.should_stop());
    }

    #[test]
    fn an_unknown_provider_is_an_error_not_a_panic() {
        let mut engine = engine(Vec::new());
        let response = engine.answer(request("provider.refresh", serde_json::json!({ "providerId": "nope" })));
        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "unknown_provider");
    }
}
