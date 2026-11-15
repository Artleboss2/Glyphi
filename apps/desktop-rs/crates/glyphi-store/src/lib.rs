//! The local ledger.
//!
//! Append the events, rebuild the projection, read the projection. Nothing else
//! writes to `usage_day`, which is what makes a rescan safe: the worst it can do is
//! replace a day with a day derived from the same events.

pub mod schema;

use std::path::Path;

use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;
use glyphi_domain::{LocalDay, TokenCounts, UsageTotals};
use glyphi_providers::UsageEvent;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub use schema::SCHEMA_VERSION;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("the local database could not be opened: {0}")]
    Open(#[from] rusqlite::Error),
    #[error("the local database is at version {found}, this build understands {understood}")]
    Version { found: i32, understood: i32 },
}

/// One provider's totals for one day, as the dashboard reads them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayRow {
    pub provider: String,
    pub account: String,
    pub day: LocalDay,
    pub sealed: bool,
    pub totals: UsageTotals,
}

/// One model's share of a day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRow {
    pub model: String,
    pub totals: TokenCounts,
    pub requests: u64,
}

/// One hour of a day, 0 to 23 in the reporting zone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HourRow {
    pub hour: u32,
    pub totals: TokenCounts,
    pub requests: u64,
}

pub struct Store {
    connection: Connection,
}

impl Store {
    /// Opens (and creates) the database at `path`, applying migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let connection = Connection::open(path)?;
        // WAL keeps a scan from blocking the window's reads; NORMAL is the right
        // durability for a local cache of facts that files still hold.
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let mut store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let connection = Connection::open_in_memory()?;
        let mut store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<(), StoreError> {
        let found: i32 = self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if found > SCHEMA_VERSION {
            return Err(StoreError::Version {
                found,
                understood: SCHEMA_VERSION,
            });
        }
        for (index, migration) in schema::MIGRATIONS.iter().enumerate().skip(found as usize) {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(migration)?;
            transaction.pragma_update(None, "user_version", (index + 1) as i32)?;
            transaction.commit()?;
        }
        Ok(())
    }

    /// Stores events, skipping the ones already known. Returns how many were new.
    pub fn insert_events(
        &mut self,
        events: &[UsageEvent],
        day_of: impl Fn(DateTime<Utc>) -> LocalDay,
        now_ms: i64,
    ) -> Result<usize, StoreError> {
        let transaction = self.connection.transaction()?;
        let mut inserted = 0usize;
        {
            let mut statement = transaction.prepare(
                "INSERT OR IGNORE INTO usage_event (
                    fingerprint, provider, account, model, at_ms, day,
                    input_tokens, cached_tokens, cache_write_tokens, output_tokens,
                    session, project, estimated, observed_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            )?;
            for event in events {
                let changed = statement.execute(params![
                    event.fingerprint(),
                    event.provider,
                    event.account,
                    event.model,
                    event.at.timestamp_millis(),
                    day_of(event.at).as_key(),
                    event.tokens.input_tokens,
                    event.tokens.cached_input_tokens,
                    event.tokens.cache_creation_input_tokens,
                    event.tokens.output_tokens,
                    event.session,
                    event.project,
                    i64::from(event.estimated),
                    now_ms,
                ])?;
                inserted += changed;
            }
        }
        transaction.commit()?;
        Ok(inserted)
    }

    /// Recomputes the projection for the given days from the events.
    ///
    /// Rebuilding rather than incrementing is what makes a parser fix visible: the
    /// day is replaced with what the events now say, not adjusted by a delta.
    pub fn rebuild_days(&mut self, days: &[LocalDay]) -> Result<(), StoreError> {
        let transaction = self.connection.transaction()?;
        for day in days {
            let key = day.as_key();
            transaction.execute("DELETE FROM usage_day WHERE day = ?1", params![key])?;
            transaction.execute("DELETE FROM usage_model WHERE day = ?1", params![key])?;

            transaction.execute(
                "INSERT INTO usage_day (
                    provider, account, day, sealed,
                    input_tokens, cached_tokens, cache_write_tokens, output_tokens,
                    requests, unpriced_tokens, cost_micros, updated_ms
                 )
                 SELECT provider, account, day, 0,
                    SUM(input_tokens), SUM(cached_tokens), SUM(cache_write_tokens), SUM(output_tokens),
                    COUNT(*), 0, NULL, ?2
                 FROM usage_event WHERE day = ?1
                 GROUP BY provider, account",
                params![key, Utc::now().timestamp_millis()],
            )?;

            // Hours are not projected here: SQLite has no time-zone table, so the
            // local hour of an event is only knowable in Rust. `hours()` reads the
            // events and buckets them at query time, which also survives a change
            // of reporting zone.

            transaction.execute(
                "INSERT INTO usage_model (
                    provider, account, day, model,
                    input_tokens, cached_tokens, cache_write_tokens, output_tokens, requests
                 )
                 SELECT provider, account, day, model,
                    SUM(input_tokens), SUM(cached_tokens), SUM(cache_write_tokens), SUM(output_tokens), COUNT(*)
                 FROM usage_event WHERE day = ?1
                 GROUP BY provider, account, model",
                params![key],
            )?;
        }
        for index in schema::DEFERRED_INDEXES {
            transaction.execute_batch(index)?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// The hourly buckets for one day, computed in the reporting zone.
    ///
    /// Done here rather than in SQL because the local hour of an event depends on
    /// the zone it was reported in, and daylight saving makes that a table lookup,
    /// not arithmetic.
    pub fn hours(
        &self,
        day: LocalDay,
        zone: Tz,
        provider: Option<&str>,
    ) -> Result<Vec<HourRow>, StoreError> {
        let start = day.start_utc(zone).unwrap_or_else(Utc::now);
        let end = day.end_utc(zone).unwrap_or(start);
        let mut statement = self.connection.prepare(
            "SELECT at_ms, input_tokens, cached_tokens, cache_write_tokens, output_tokens
             FROM usage_event WHERE at_ms >= ?1 AND at_ms < ?2
               AND (?3 IS NULL OR provider = ?3)",
        )?;
        let rows = statement.query_map(
            params![start.timestamp_millis(), end.timestamp_millis(), provider],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )?;
        let mut buckets: [Option<(TokenCounts, u64)>; 24] = [None; 24];
        for row in rows {
            let (at_ms, input, cached, cache_write, output) = row?;
            let instant = Utc
                .timestamp_millis_opt(at_ms)
                .single()
                .unwrap_or_else(Utc::now);
            let hour = instant.with_timezone(&zone).hour();
            let entry = buckets[hour as usize].get_or_insert((TokenCounts::ZERO, 0));
            entry.0 = entry.0.saturating_add(TokenCounts {
                input_tokens: input as u64,
                cached_input_tokens: cached as u64,
                cache_creation_input_tokens: cache_write as u64,
                output_tokens: output as u64,
            });
            entry.1 += 1;
        }
        Ok(buckets
            .iter()
            .enumerate()
            .map(|(hour, bucket)| HourRow {
                hour: hour as u32,
                totals: bucket.map(|entry| entry.0).unwrap_or(TokenCounts::ZERO),
                requests: bucket.map(|entry| entry.1).unwrap_or(0),
            })
            .collect())
    }

    /// Every day between two dates, one row per provider and account.
    pub fn days(&self, first: LocalDay, last: LocalDay) -> Result<Vec<DayRow>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT provider, account, day, sealed,
                    input_tokens, cached_tokens, cache_write_tokens, output_tokens,
                    requests, unpriced_tokens, cost_micros
             FROM usage_day WHERE day >= ?1 AND day <= ?2
             ORDER BY day, provider",
        )?;
        let rows = statement.query_map(params![first.as_key(), last.as_key()], |row| {
            let day: String = row.get(2)?;
            Ok(DayRow {
                provider: row.get(0)?,
                account: row.get(1)?,
                day: LocalDay::parse(&day).unwrap_or(first),
                sealed: row.get::<_, i64>(3)? != 0,
                totals: UsageTotals {
                    tokens: TokenCounts {
                        input_tokens: row.get::<_, i64>(4)? as u64,
                        cached_input_tokens: row.get::<_, i64>(5)? as u64,
                        cache_creation_input_tokens: row.get::<_, i64>(6)? as u64,
                        output_tokens: row.get::<_, i64>(7)? as u64,
                    },
                    requests: row.get::<_, i64>(8)? as u64,
                    unpriced_tokens: row.get::<_, i64>(9)? as u64,
                    estimated_cost_micros_usd: row.get(10)?,
                },
            })
        })?;
        let mut days = Vec::new();
        for row in rows {
            days.push(row?);
        }
        Ok(days)
    }

    /// One day's models, for one provider or for all of them.
    pub fn models(
        &self,
        day: LocalDay,
        provider: Option<&str>,
    ) -> Result<Vec<ModelRow>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT model, SUM(input_tokens), SUM(cached_tokens), SUM(cache_write_tokens),
                    SUM(output_tokens), SUM(requests)
             FROM usage_model WHERE day = ?1 AND (?2 IS NULL OR provider = ?2)
             GROUP BY model ORDER BY SUM(output_tokens) DESC",
        )?;
        let rows = statement.query_map(params![day.as_key(), provider], |row| {
            Ok(ModelRow {
                model: row.get(0)?,
                totals: TokenCounts {
                    input_tokens: row.get::<_, i64>(1)? as u64,
                    cached_input_tokens: row.get::<_, i64>(2)? as u64,
                    cache_creation_input_tokens: row.get::<_, i64>(3)? as u64,
                    output_tokens: row.get::<_, i64>(4)? as u64,
                },
                requests: row.get::<_, i64>(5)? as u64,
            })
        })?;
        let mut models = Vec::new();
        for row in rows {
            models.push(row?);
        }
        Ok(models)
    }

    /// Every model's totals per day across a range, for one provider.
    ///
    /// The dashboard splits its model breakdown by day, which `models()` cannot
    /// answer: it collapses the range into one row per model.
    pub fn models_range(
        &self,
        provider: &str,
        first: LocalDay,
        last: LocalDay,
    ) -> Result<Vec<(LocalDay, ModelRow)>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT day, model, SUM(input_tokens), SUM(cached_tokens), SUM(cache_write_tokens),
                    SUM(output_tokens), SUM(requests)
             FROM usage_model WHERE provider = ?1 AND day >= ?2 AND day <= ?3
             GROUP BY day, model ORDER BY day, SUM(output_tokens) DESC",
        )?;
        let rows = statement.query_map(params![provider, first.as_key(), last.as_key()], |row| {
            let day: String = row.get(0)?;
            Ok((
                LocalDay::parse(&day).unwrap_or(first),
                ModelRow {
                    model: row.get(1)?,
                    totals: TokenCounts {
                        input_tokens: row.get::<_, i64>(2)? as u64,
                        cached_input_tokens: row.get::<_, i64>(3)? as u64,
                        cache_creation_input_tokens: row.get::<_, i64>(4)? as u64,
                        output_tokens: row.get::<_, i64>(5)? as u64,
                    },
                    requests: row.get::<_, i64>(6)? as u64,
                },
            ))
        })?;
        let mut models = Vec::new();
        for row in rows {
            models.push(row?);
        }
        Ok(models)
    }

    /// The whole stored history, oldest first: what a first paint needs.
    pub fn totals(&self, provider: Option<&str>) -> Result<UsageTotals, StoreError> {
        let totals = self.connection.query_row(
            "SELECT COALESCE(SUM(input_tokens),0), COALESCE(SUM(cached_tokens),0),
                    COALESCE(SUM(cache_write_tokens),0), COALESCE(SUM(output_tokens),0),
                    COALESCE(SUM(requests),0)
             FROM usage_day WHERE (?1 IS NULL OR provider = ?1)",
            params![provider],
            |row| {
                Ok(UsageTotals::of(
                    TokenCounts {
                        input_tokens: row.get::<_, i64>(0)? as u64,
                        cached_input_tokens: row.get::<_, i64>(1)? as u64,
                        cache_creation_input_tokens: row.get::<_, i64>(2)? as u64,
                        output_tokens: row.get::<_, i64>(3)? as u64,
                    },
                    row.get::<_, i64>(4)? as u64,
                ))
            },
        )?;
        Ok(totals)
    }

    /// Saves the newest capacity reading so the gauges survive a restart.
    pub fn save_capacity(&mut self, provider: &str, account: &str, payload: &str) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT INTO capacity_snapshot (provider, account, captured_ms, payload)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(provider, account) DO UPDATE SET captured_ms = ?3, payload = ?4",
            params![provider, account, Utc::now().timestamp_millis(), payload],
        )?;
        Ok(())
    }

    pub fn capacity(&self, provider: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT payload FROM capacity_snapshot WHERE provider = ?1",
                params![provider],
                |row| row.get::<_, String>(0),
            )
            .optional()?)
    }
}

use chrono::Timelike as _;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;

    fn event(at: DateTime<Utc>, input: u64, output: u64) -> UsageEvent {
        UsageEvent::new(
            "claude",
            "local",
            "sonnet",
            at,
            TokenCounts {
                input_tokens: input,
                output_tokens: output,
                ..TokenCounts::default()
            },
        )
    }

    fn utc() -> Tz {
        chrono_tz::UTC
    }

    #[test]
    fn an_event_is_stored_once_however_often_it_is_scanned() {
        let mut store = Store::open_in_memory().unwrap();
        let at = Utc.with_ymd_and_hms(2026, 11, 15, 9, 0, 0).unwrap();
        let events = vec![event(at, 10, 5)];
        assert_eq!(store.insert_events(&events, |at| LocalDay::of(at, utc()), 0).unwrap(), 1);
        assert_eq!(store.insert_events(&events, |at| LocalDay::of(at, utc()), 0).unwrap(), 0);
    }

    #[test]
    fn the_projection_is_rebuilt_from_the_events() {
        let mut store = Store::open_in_memory().unwrap();
        let at = Utc.with_ymd_and_hms(2026, 11, 15, 9, 0, 0).unwrap();
        store
            .insert_events(&[event(at, 10, 5), event(at, 20, 7)], |at| LocalDay::of(at, utc()), 0)
            .unwrap();
        let day = LocalDay::of(at, utc());
        store.rebuild_days(&[day]).unwrap();

        let days = store.days(day, day).unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].totals.tokens.total(), 42);
        assert_eq!(days[0].totals.requests, 2);

        let hours = store.hours(day, utc(), None).unwrap();
        assert_eq!(hours[9].requests, 2);
        assert_eq!(hours[10].requests, 0);

        let models = store.models(day, None).unwrap();
        assert_eq!(models[0].model, "sonnet");
        assert_eq!(models[0].totals.total(), 42);
    }

    #[test]
    fn a_rebuild_never_keeps_the_old_numbers() {
        let mut store = Store::open_in_memory().unwrap();
        let at = Utc.with_ymd_and_hms(2026, 11, 15, 9, 0, 0).unwrap();
        let day = LocalDay::of(at, utc());
        store.insert_events(&[event(at, 10, 5)], |at| LocalDay::of(at, utc()), 0).unwrap();
        store.rebuild_days(&[day]).unwrap();
        store.rebuild_days(&[day]).unwrap();
        assert_eq!(store.days(day, day).unwrap()[0].totals.tokens.total(), 15);
    }

    #[test]
    fn capacity_survives_a_reopen() {
        let mut store = Store::open_in_memory().unwrap();
        store.save_capacity("claude", "local", "{\"plan\":\"max\"}").unwrap();
        assert_eq!(store.capacity("claude").unwrap().as_deref(), Some("{\"plan\":\"max\"}"));
        assert_eq!(store.capacity("codex").unwrap(), None);
    }
}
