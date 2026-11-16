//! Settings → Providers: the directory of tools Glyphi knows about.
//!
//! The list itself is a JSON file built from the same catalog the renderer used,
//! so the app can search 305 products without the engine holding a table of names.
//! The engine only knows the tools it has a reader for; the rest of the directory
//! exists so a card can say "no reader yet" instead of the tool never appearing.
//!
//! Scanning is passive: it looks in the folders an installer writes and reports
//! what it found. Nothing is enabled here — `providerEnabled` is the user's switch,
//! and this module only reads it.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::bridge::BridgeReply;

#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub kind: String,
    #[serde(default)]
    pub adapter: Option<String>,
}

const CATALOG: &str = include_str!("../assets/catalog.json");

pub fn entries() -> Vec<Entry> {
    serde_json::from_str(CATALOG).unwrap_or_default()
}

fn compact(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

/// The folder names a tool might have left behind: its full name in every spelling
/// we have seen, then the first word, which is what a CLI usually names its folder.
/// Only the product's own name may fall back to its first word. An alias is a way of
/// writing the same product, not a second product: "claude dev" is Cline, and its
/// first word alone would make Cline claim Claude Code's folder.
fn spellings(entry: &Entry) -> Vec<String> {
    let mut names = BTreeSet::new();
    for (index, lead) in std::iter::once(&entry.name).chain(entry.aliases.iter()).enumerate() {
        let primary = index == 0;
        let squashed = compact(lead);
        if squashed.is_empty() {
            continue;
        }
        names.insert(squashed.clone());
        names.insert(format!(".{squashed}"));
        let words: Vec<String> = lead
            .split(|character: char| !character.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(|word| word.to_ascii_lowercase())
            .collect();
        if words.len() > 1 {
            names.insert(words.join(""));
            names.insert(words.join("-"));
            names.insert(words.join("_"));
        }
        if let (true, Some(first)) = (primary, words.first()) {
            names.insert(first.clone());
            names.insert(format!(".{first}"));
        }
    }
    names.into_iter().collect()
}

/// Every place a desktop app or a CLI keeps per-user data on this platform.
fn bases() -> Vec<PathBuf> {
    let mut bases = Vec::new();
    for key in ["APPDATA", "LOCALAPPDATA", "XDG_CONFIG_HOME", "XDG_DATA_HOME"] {
        if let Ok(value) = std::env::var(key) {
            if !value.is_empty() {
                bases.push(PathBuf::from(value));
            }
        }
    }
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        let home = PathBuf::from(home);
        bases.push(home.join(".config"));
        bases.push(home.join(".local").join("share"));
        bases.push(home.join("AppData").join("Roaming"));
        bases.push(home.join("AppData").join("Local"));
        bases.push(home.clone());
    }
    bases
}

/// The first folder that exists for a tool, which is the evidence "On this PC" is
/// built from. A directory is enough: an empty one still means the tool was installed.
pub fn installed_folder(entry: &Entry) -> Option<PathBuf> {
    let roots = bases();
    for spelling in spellings(entry) {
        for base in &roots {
            let candidate = base.join(&spelling);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }
    None
}

fn tracked(enabled: &Map<String, Value>, entry: &Entry) -> bool {
    entry
        .adapter
        .as_deref()
        .and_then(|adapter| enabled.get(adapter))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// The rows Settings → Providers renders. `installedFolder` is what puts a tool at
/// the top of the list, so the scan has to have run for the order to be useful.
pub fn directory(enabled: &Map<String, Value>) -> Value {
    let rows: Vec<Value> = entries()
        .into_iter()
        .map(|entry| {
            json!({
                "name": entry.name,
                "kind": entry.kind,
                "adapter": entry.adapter,
                "tracked": tracked(&enabled, &entry),
                "installedFolder": installed_folder(&entry).map(|path| path.display().to_string()),
            })
        })
        .collect();
    Value::Array(rows)
}

/// The names the scan proved are on this computer, for the "Scanned" message.
pub fn scan() -> Value {
    let found: Vec<String> = entries()
        .into_iter()
        .filter(|entry| installed_folder(entry).is_some())
        .map(|entry| entry.name)
        .collect();
    Value::Array(found.into_iter().map(Value::String).collect())
}

/// Counts what a tool keeps on disk, for the tools with no reader: it answers
/// "there is usage in here, nobody can read it yet" from real files.
pub fn probe(name: &str) -> Value {
    let wanted = compact(name);
    let Some(entry) = entries()
        .into_iter()
        .find(|entry| compact(&entry.name) == wanted)
    else {
        return json!({ "files": [], "folders": 0 });
    };
    let Some(folder) = installed_folder(&entry) else {
        return json!({ "files": [], "folders": 0 });
    };
    let mut files = Vec::new();
    let mut folders = 0usize;
    walk(&folder, 2, &mut files, &mut folders);
    json!({ "files": files, "folders": folders })
}

fn walk(directory: &PathBuf, depth: usize, files: &mut Vec<String>, folders: &mut usize) {
    if files.len() >= 2000 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else { return };
    for item in entries.flatten() {
        let path = item.path();
        if path.is_dir() {
            *folders += 1;
            if depth > 0 {
                walk(&path, depth - 1, files, folders);
            }
        } else if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
            if name.ends_with(".jsonl") || name.ends_with(".json") || name.ends_with(".db") {
                files.push(path.display().to_string());
            }
        }
    }
}

/// Answers the four directory calls the renderer makes. Returns `None` for every
/// other method, which then follows the normal path into the engine.
pub fn intercept(
    call: &crate::bridge::BridgeCall,
    user_data: Option<&std::path::Path>,
) -> Option<BridgeReply> {
    let enabled = || {
        crate::preferences::apply(user_data, &Value::Null)
            .get("providerEnabled")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    };
    let result = match call.method.as_str() {
        "glyphi.getProviderDirectory" => directory(&enabled()),
        "glyphi.scanInstalledProviders" => scan(),
        "glyphi.discoverProvider" => probe(
            call.params.get("name").and_then(Value::as_str).unwrap_or_default(),
        ),
        "glyphi.setProviderEnabled" => {
            let adapter = call
                .params
                .get("providerId")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let on = call
                .params
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Value::Bool(if adapter.is_empty() { false } else { set_enabled(user_data, adapter, on) })
        }
        _ => return None,
    };
    Some(BridgeReply::ok(call.id.clone(), result))
}

/// Merges one switch into `providerEnabled` and writes the preferences back.
pub fn set_enabled(user_data: Option<&std::path::Path>, adapter: &str, enabled: bool) -> bool {
    let current = crate::preferences::apply(user_data, &Value::Null);
    let mut map = current
        .get("providerEnabled")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    map.insert(adapter.to_string(), Value::Bool(enabled));
    let patch = json!({ "providerEnabled": map });
    crate::preferences::apply(user_data, &json!({ "patch": patch }));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_is_not_empty() {
        let entries = entries();
        assert!(entries.len() > 250, "the catalog lost entries: {}", entries.len());
        assert!(entries.iter().any(|entry| entry.adapter.as_deref() == Some("claude")));
    }

    #[test]
    fn a_folder_name_is_recognised_in_every_spelling() {
        let entry = Entry {
            name: "Claude Code".to_string(),
            aliases: vec!["claude".to_string()],
            kind: "cli-agent".to_string(),
            adapter: Some("claude".to_string()),
        };
        let names = spellings(&entry);
        assert!(names.contains(&".claude".to_string()));
        assert!(names.contains(&"claudecode".to_string()));
        assert!(names.contains(&"claude-code".to_string()));
    }

    #[test]
    fn a_tracked_switch_is_read_per_adapter() {
        let mut enabled = Map::new();
        enabled.insert("claude".to_string(), Value::Bool(true));
        let rows = directory(&enabled);
        let rows = rows.as_array().unwrap();
        let claude = rows
            .iter()
            .find(|row| row["adapter"] == json!("claude"))
            .unwrap();
        assert_eq!(claude["tracked"], json!(true));
        // Every row carries the four keys the settings page reads, even when null.
        assert!(claude.get("installedFolder").is_some());
    }
}
