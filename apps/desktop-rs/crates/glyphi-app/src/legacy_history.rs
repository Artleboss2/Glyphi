//! The history the Electron build left behind.
//!
//! The port replaced the app, not the user's usage: the old build kept one
//! `usage_record` row per provider per day in `history.sqlite`, with the day split
//! into hourly buckets. Those buckets become events, so the ledger derives days,
//! models and hours from them exactly like a scan would — no second projection to
//! keep in step.
//!
//! The import is idempotent: an event's session carries the provider, the day and
//! the hour start, and `insert_events` ignores a fingerprint it already has. It runs
//! on every start, which costs one read of a few dozen rows and needs no marker.
//!
//! Today's rows are skipped: the live adapters own today, and importing a day twice
//! would be worse than starting the day empty.

use std::path::Path;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use glyphi_domain::{LocalDay, TokenCounts};
use glyphi_providers::adapter::UsageEvent;
use glyphi_store::Store;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

/// Reads `history.sqlite` from `directory`, if it is there. Returns how many events
/// were new.
pub fn import(directory: &Path, store: &mut Store, zone: Tz) -> Result<usize, String> {
    let database = directory.join("history.sqlite");
    if !database.is_file() {
        return Ok(0);
    }
    // Read-only: the old build may still be installed on another machine, and a
    // ledger this app cannot write must never be locked against it.
    let connection = Connection::open_with_flags(&database, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| error.to_string())?;
    let today = LocalDay::of(Utc::now(), zone).as_key();

    let mut events: Vec<UsageEvent> = Vec::new();
    let mut days: Vec<LocalDay> = Vec::new();
    {
        let mut statement = connection
            .prepare("SELECT provider, day, payload FROM usage_record WHERE day IS NOT NULL")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| error.to_string())?;

        for row in rows {
            let (provider, day, payload) = row.map_err(|error| error.to_string())?;
            // ISO days sort lexicographically, so the comparison is the filter.
            if day.as_str() >= today.as_str() {
                continue;
            }
            let Ok(payload) = serde_json::from_str::<Value>(&payload) else {
                continue;
            };
            if payload.get("kind").and_then(Value::as_str) != Some("usage_day") {
                continue;
            }
            let Some(buckets) = payload.get("hourly").and_then(Value::as_array) else {
                continue;
            };
            let mut added = false;
            for bucket in buckets {
                let Some(at) = bucket
                    .get("utcStart")
                    .and_then(Value::as_str)
                    .and_then(parse_instant)
                else {
                    continue;
                };
                let tokens = tokens_of(bucket.get("totals"));
                if tokens.total() == 0 {
                    continue;
                }
                // The old build stored model keys as hashes (`m-61c26317cc`), so a
                // model name here would be invented. No name is honest.
                let mut event = UsageEvent::new(provider.clone(), "local", "", at, tokens);
                event.session = Some(format!(
                    "electron:{provider}:{day}:{}",
                    at.timestamp_millis()
                ));
                events.push(event);
                added = true;
            }
            if added {
                if let Some(parsed) = LocalDay::parse(&day) {
                    days.push(parsed);
                }
            }
        }
    }
    drop(connection);

    if events.is_empty() {
        return Ok(0);
    }
    let now_ms = Utc::now().timestamp_millis();
    let inserted = store
        .insert_events(&events, |at| LocalDay::of(at, zone), now_ms)
        .map_err(|error| error.to_string())?;
    // Today's refresh would rebuild the window anyway, but the first paint reads
    // the projection before that: writing it here is what puts history on screen.
    store
        .rebuild_days(&days)
        .map_err(|error| error.to_string())?;
    Ok(inserted)
}

fn parse_instant(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.with_timezone(&Utc))
}

fn tokens_of(totals: Option<&Value>) -> TokenCounts {
    let count = |key: &str| {
        totals
            .and_then(|totals| totals.get(key))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    TokenCounts {
        input_tokens: count("inputTokens"),
        cached_input_tokens: count("cachedInputTokens"),
        cache_creation_input_tokens: count("cacheCreationInputTokens"),
        output_tokens: count("outputTokens"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_is_imported_once() {
        let directory = std::env::temp_dir().join(format!("glyphi-legacy-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let history = directory.join("history.sqlite");
        let _ = std::fs::remove_file(&history);
        {
            let connection = Connection::open(&history).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE usage_record (provider TEXT, day TEXT, payload TEXT);
                     INSERT INTO usage_record VALUES (
                       'freebuff', '2026-11-10',
                       '{\"kind\":\"usage_day\",\"hourly\":[{\"utcStart\":\"2026-11-10T21:00:00.000Z\",\"totals\":{\"inputTokens\":100,\"cachedInputTokens\":40,\"cacheCreationInputTokens\":2,\"outputTokens\":7}}]}'
                     );",
                )
                .unwrap();
        }

        let mut store =
            Store::open(&directory.join("glyphi.db")).expect("the test ledger opens");
        let zone = chrono_tz::UTC;
        let first = import(&directory, &mut store, zone).unwrap();
        assert_eq!(first, 1);
        // A second start reads the same file and adds nothing.
        assert_eq!(import(&directory, &mut store, zone).unwrap(), 0);

        let day = LocalDay::parse("2026-11-10").unwrap();
        let rows = store.days(day, day).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].totals.tokens.input_tokens, 100);
        assert_eq!(rows[0].totals.tokens.output_tokens, 7);

        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_missing_database_is_not_an_error() {
        let directory = std::env::temp_dir().join(format!("glyphi-legacy-none-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut store = Store::open(&directory.join("glyphi.db")).unwrap();
        assert_eq!(import(&directory, &mut store, chrono_tz::UTC).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&directory);
    }
}
