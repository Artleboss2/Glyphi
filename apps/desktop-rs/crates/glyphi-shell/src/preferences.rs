//! The renderer's settings, kept in the file the Electron build wrote.
//!
//! The Rust core owns usage; preferences are the window's. Reading the same
//! `desktop-preferences.json` is what keeps a chosen theme, language and background
//! across the port, and writing it back is what makes a settings page survive a
//! restart. Every key the renderer reads has a default here, so a fresh machine
//! still gets an object it can parse instead of `undefined`.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

/// The shape of `DesktopPreferences`, with the product defaults.
fn defaults() -> Value {
    json!({
        "language": "system",
        "appearance": {
            "theme": "system",
            "font": "cormorant",
            "fontScale": 1,
            "textColor": null,
            "mark": "glyph",
            "numberStyle": "exact",
            "accent": "system",
            "surface": "glass",
            "animate": true,
            "density": "comfortable",
            "windowOpacity": 1,
            "blur": 18,
            "providerColors": {}
        },
        "backgroundImage": "desert",
        "updateChannel": "github",
        "updateRepository": "",
        "customBackgroundName": null,
        "launchAtLogin": true,
        "minimizeToTray": true,
        "anonymousAnalytics": true,
        "providerEnabled": {},
        "limitOrder": [],
        "trayLimits": {},
        "usageAlerts": {},
        "tokenAlerts": { "enabled": true, "thresholdTokens": 100000 },
        "reportPlans": [],
        "startMinimized": false,
        "hotkey": "CommandOrControl+Shift+U",
        "pricingSyncHours": 24,
        "powerSaving": true,
        "widget": {
            "enabled": false,
            "style": "balanced",
            "autoHideSeconds": 0,
            "opacity": 0.94,
            "snapToEdges": true,
            "width": 300,
            "height": 150,
            "position": null
        },
        "advanced": {
            "trayMetric": "remaining",
            "monthlyBudgetUSD": 0,
            "currency": "USD",
            "taxPercent": 0,
            "blendedCostEnabled": true,
            "blendedCostPerMillionUSD": 0.54,
            "promptCaching": true,
            "modelCostOverrides": [],
            "extraLogPaths": [],
            "ignoredProjects": [],
            "staleSessionMinutes": 0,
            "burnRateEnabled": false
        }
    })
}

fn file(user_data: Option<&Path>) -> Option<PathBuf> {
    Some(user_data?.join("desktop-preferences.json"))
}

/// Answers `shell.preferences`, and persists the patch when one is given.
pub fn apply(user_data: Option<&Path>, params: &Value) -> Value {
    let mut values = defaults();
    let path = file(user_data);
    if let Some(stored) = path.as_deref().and_then(read) {
        merge(&mut values, &stored);
    }
    let patch = params.get("patch").cloned().unwrap_or(Value::Null);
    if !patch.is_null() {
        merge(&mut values, &patch);
        if let Some(path) = path.as_deref() {
            let _ = write(path, &values);
        }
    }
    values
}

fn read(path: &Path) -> Option<Value> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write(path: &Path, values: &Value) -> std::io::Result<()> {
    let encoded = serde_json::to_vec_pretty(values).unwrap_or_else(|_| b"{}".to_vec());
    std::fs::write(path, encoded)
}

/// Objects merge key by key; anything else is replaced by the newer value.
///
/// A shallow merge would throw away `appearance` because a stored file happens to
/// lack one new key, which is exactly how a settings page loses a theme.
fn merge(into: &mut Value, from: &Value) {
    match (into, from) {
        (Value::Object(target), Value::Object(source)) => {
            for (key, value) in source {
                match target.get_mut(key) {
                    Some(existing) => merge(existing, value),
                    None => {
                        target.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (target, source) => *target = source.clone(),
    }
}

#[allow(dead_code)]
fn as_object(value: &Value) -> Option<&Map<String, Value>> {
    value.as_object()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_still_answers_every_key() {
        let prefs = apply(None, &Value::Null);
        assert_eq!(prefs["language"], "system");
        assert_eq!(prefs["advanced"]["currency"], "USD");
        assert!(prefs["appearance"].is_object());
        assert!(prefs["widget"].is_object());
    }

    #[test]
    fn a_patch_keeps_the_keys_it_does_not_mention() {
        let directory = std::env::temp_dir().join(format!(
            "glyphi-preferences-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("desktop-preferences.json"),
            br#"{"language":"fr","appearance":{"theme":"dark"}}"#,
        )
        .unwrap();

        let patched = apply(
            Some(&directory),
            &json!({ "patch": { "appearance": { "theme": "light" } } }),
        );
        assert_eq!(patched["language"], "fr");
        assert_eq!(patched["appearance"]["theme"], "light");
        // A key the patch does not carry is not lost to the merge.
        assert_eq!(patched["appearance"]["font"], "cormorant");

        // And it reached the file, so the choice survives a restart.
        let stored = read(&directory.join("desktop-preferences.json")).unwrap();
        assert_eq!(stored["appearance"]["theme"], "light");

        let _ = std::fs::remove_dir_all(&directory);
    }
}
