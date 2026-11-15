//! The schema, applied in order.
//!
//! Two rules shape it:
//!
//! 1. **Events are the truth.** `usage_event` is append-only and keyed by a content
//!    fingerprint, so rescanning a file cannot double-count and a parser fix can be
//!    applied by rebuilding the projection.
//! 2. **The projection is disposable.** `usage_day` and `usage_model` can be dropped
//!    and rebuilt from the events at any time; hours are bucketed at read time.

/// Bumped when a migration below changes the meaning of stored rows.
pub const SCHEMA_VERSION: i32 = 1;

pub const MIGRATIONS: &[&str] = &[
    // 1: the first ledger.
    r#"
    CREATE TABLE IF NOT EXISTS usage_event (
        fingerprint  TEXT PRIMARY KEY NOT NULL,
        provider     TEXT NOT NULL,
        account      TEXT NOT NULL,
        model        TEXT NOT NULL,
        at_ms        INTEGER NOT NULL,
        day          TEXT NOT NULL,
        input_tokens INTEGER NOT NULL DEFAULT 0,
        cached_tokens INTEGER NOT NULL DEFAULT 0,
        cache_write_tokens INTEGER NOT NULL DEFAULT 0,
        output_tokens INTEGER NOT NULL DEFAULT 0,
        session      TEXT,
        project      TEXT,
        estimated    INTEGER NOT NULL DEFAULT 0,
        observed_ms  INTEGER NOT NULL
    );

    -- A day is read far more often than it is written; the projection indexes the
    -- three columns every screen filters on.
    CREATE INDEX IF NOT EXISTS usage_event_day ON usage_event (day, provider, account);

    CREATE TABLE IF NOT EXISTS usage_day (
        provider     TEXT NOT NULL,
        account      TEXT NOT NULL,
        day          TEXT NOT NULL,
        sealed       INTEGER NOT NULL DEFAULT 0,
        input_tokens INTEGER NOT NULL DEFAULT 0,
        cached_tokens INTEGER NOT NULL DEFAULT 0,
        cache_write_tokens INTEGER NOT NULL DEFAULT 0,
        output_tokens INTEGER NOT NULL DEFAULT 0,
        requests     INTEGER NOT NULL DEFAULT 0,
        unpriced_tokens INTEGER NOT NULL DEFAULT 0,
        cost_micros  INTEGER,
        updated_ms   INTEGER NOT NULL,
        PRIMARY KEY (provider, account, day)
    );

    -- Hours are not a table: a local hour depends on the reporting zone, which
    -- SQLite cannot resolve, so they are bucketed in Rust from `usage_event`.

    CREATE TABLE IF NOT EXISTS usage_model (
        provider     TEXT NOT NULL,
        account      TEXT NOT NULL,
        day          TEXT NOT NULL,
        model        TEXT NOT NULL,
        input_tokens INTEGER NOT NULL DEFAULT 0,
        cached_tokens INTEGER NOT NULL DEFAULT 0,
        cache_write_tokens INTEGER NOT NULL DEFAULT 0,
        output_tokens INTEGER NOT NULL DEFAULT 0,
        requests     INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (provider, account, day, model)
    );

    -- The last capacity reading per provider, so a cold start can draw the gauges
    -- before the first refresh finishes.
    CREATE TABLE IF NOT EXISTS capacity_snapshot (
        provider   TEXT NOT NULL,
        account    TEXT NOT NULL,
        captured_ms INTEGER NOT NULL,
        payload    TEXT NOT NULL,
        PRIMARY KEY (provider, account)
    );

    -- A source scanned but not yet fully imported survives a crash here.
    CREATE TABLE IF NOT EXISTS import_marker (
        provider TEXT PRIMARY KEY NOT NULL,
        since    TEXT NOT NULL,
        started_ms INTEGER NOT NULL
    );
    "#,
];

/// The indexes built after the first bulk import, where building them up front
/// costs more than it saves.
pub const DEFERRED_INDEXES: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS usage_day_day ON usage_day (day)",
    "CREATE INDEX IF NOT EXISTS usage_model_model ON usage_model (model)",
];
