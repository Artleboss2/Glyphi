//! What the user configured, in the one place both halves agree on.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The floor and ceiling for how far back a scan looks. A year is generous; ten is
/// a typo that would make every refresh walk a decade of logs.
pub const MIN_HISTORY_DAYS: u32 = 1;
pub const MAX_HISTORY_DAYS: u32 = 365;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineSettings {
    pub history_days: u32,
    pub reporting_zone: String,
    pub extra_log_paths: Vec<PathBuf>,
    pub ignored_projects: Vec<String>,
    /// The home directory the adapters read from. Passed in rather than guessed, so
    /// a test and a packaged app never disagree about where `~` is.
    pub home: PathBuf,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            history_days: 30,
            reporting_zone: "UTC".to_string(),
            extra_log_paths: Vec::new(),
            ignored_projects: Vec::new(),
            home: std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(".")),
        }
    }
}

impl EngineSettings {
    /// Applies a `config.update` payload. Unknown keys are ignored rather than
    /// rejected: the window may be newer than the engine, and failing a refresh
    /// over an extra field would be worse than not using it.
    pub fn apply(&mut self, patch: &Value) {
        if let Some(days) = patch.get("historyDays").and_then(Value::as_u64) {
            self.history_days = (days as u32).clamp(MIN_HISTORY_DAYS, MAX_HISTORY_DAYS);
        }
        if let Some(zone) = patch.get("reportingZone").and_then(Value::as_str) {
            if zone.parse::<chrono_tz::Tz>().is_ok() {
                self.reporting_zone = zone.to_string();
            }
        }
        if let Some(paths) = patch.get("extraLogPaths").and_then(Value::as_array) {
            self.extra_log_paths = paths
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .map(PathBuf::from)
                .collect();
        }
        if let Some(projects) = patch.get("ignoredProjects").and_then(Value::as_array) {
            self.ignored_projects = projects
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect();
        }
        if let Some(home) = patch.get("home").and_then(Value::as_str) {
            if !home.trim().is_empty() {
                self.home = PathBuf::from(home);
            }
        }
    }

    pub fn zone(&self) -> chrono_tz::Tz {
        self.reporting_zone
            .parse()
            .unwrap_or(chrono_tz::UTC)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_history_window_is_clamped_not_trusted() {
        let mut settings = EngineSettings::default();
        settings.apply(&json!({ "historyDays": 99_999 }));
        assert_eq!(settings.history_days, MAX_HISTORY_DAYS);
        settings.apply(&json!({ "historyDays": 0 }));
        assert_eq!(settings.history_days, MIN_HISTORY_DAYS);
    }

    #[test]
    fn an_unknown_zone_is_ignored() {
        let mut settings = EngineSettings::default();
        settings.apply(&json!({ "reportingZone": "Mars/Olympus" }));
        assert_eq!(settings.reporting_zone, "UTC");
        settings.apply(&json!({ "reportingZone": "Europe/Berlin" }));
        assert_eq!(settings.zone().to_string(), "Europe/Berlin");
    }

    #[test]
    fn blank_paths_never_become_scan_roots() {
        let mut settings = EngineSettings::default();
        settings.apply(&json!({ "extraLogPaths": ["  ", "/tmp/logs"], "ignoredProjects": [""] }));
        assert_eq!(settings.extra_log_paths.len(), 1);
        assert!(settings.ignored_projects.is_empty());
    }
}
