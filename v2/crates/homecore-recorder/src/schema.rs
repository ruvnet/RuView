//! SQL DDL for the HA-compatible recorder schema (ADR-132).
//!
//! Schema mirrors Home Assistant recorder schema v48 (HA 2025.1):
//! - `states`           — one row per state write (entity_id, state, attrs)
//! - `state_attributes` — shared attribute blobs, deduped by fnv64a hash
//! - `events`           — domain events fired by integrations
//! - `recorder_runs`    — boot/shutdown bookends for gap detection
//!
//! All DDL strings use `CREATE TABLE IF NOT EXISTS` so `apply_schema` is
//! idempotent and safe to call on every startup.

/// Create `state_attributes` table.
///
/// `shared_attrs` is stored as TEXT (JSON blob). `hash` is the FNV-1a 64-bit
/// hash of `shared_attrs` encoded as a signed i64 — matches HA's dedup key.
pub const CREATE_STATE_ATTRIBUTES: &str = "
CREATE TABLE IF NOT EXISTS state_attributes (
    attributes_id  INTEGER PRIMARY KEY NOT NULL,
    shared_attrs   TEXT    NOT NULL,
    hash           INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS ix_state_attributes_hash
    ON state_attributes (hash);
";

/// Create `states` table.
///
/// `state_id`        — auto-increment primary key
/// `entity_id`       — validated `domain.name` string
/// `state`           — state value string (\"on\", \"off\", \"20.5\", …)
/// `attributes_id`   — FK → state_attributes (nullable for HA compat)
/// `last_changed_ts` — Unix timestamp seconds (float, UTC)
/// `last_updated_ts` — Unix timestamp seconds (float, UTC)
/// `context_id`      — UUID as TEXT; links to the causality chain
pub const CREATE_STATES: &str = "
CREATE TABLE IF NOT EXISTS states (
    state_id         INTEGER PRIMARY KEY NOT NULL,
    entity_id        TEXT    NOT NULL,
    state            TEXT,
    attributes_id    INTEGER,
    last_changed_ts  REAL,
    last_updated_ts  REAL    NOT NULL,
    context_id       TEXT
);

CREATE INDEX IF NOT EXISTS ix_states_entity_id_last_updated_ts
    ON states (entity_id, last_updated_ts);

CREATE INDEX IF NOT EXISTS ix_states_last_updated_ts
    ON states (last_updated_ts);
";

/// Explicit markers for removal rows, separate from legacy malformed NULL states.
/// The state row supplies the timestamp and row-id ordering used during restore.
pub const CREATE_STATE_REMOVALS: &str = "
CREATE TABLE IF NOT EXISTS state_removals (
    state_id INTEGER PRIMARY KEY NOT NULL
);
";

/// Create `events` table.
///
/// `event_type`      — string key (e.g. \"state_changed\", \"call_service\")
/// `event_data`      — JSON blob
/// `time_fired_ts`   — Unix timestamp seconds (float, UTC)
/// `context_id`      — UUID as TEXT
pub const CREATE_EVENTS: &str = "
CREATE TABLE IF NOT EXISTS events (
    event_id      INTEGER PRIMARY KEY NOT NULL,
    event_type    TEXT NOT NULL,
    event_data    TEXT,
    time_fired_ts REAL NOT NULL,
    context_id    TEXT
);

CREATE INDEX IF NOT EXISTS ix_events_event_type_time_fired_ts
    ON events (event_type, time_fired_ts);
";

/// Create `recorder_runs` table.
///
/// Records each start/stop pair so the history API can annotate gaps.
pub const CREATE_RECORDER_RUNS: &str = "
CREATE TABLE IF NOT EXISTS recorder_runs (
    run_id    INTEGER PRIMARY KEY NOT NULL,
    start_ts  REAL    NOT NULL,
    end_ts    REAL
);
";

/// All DDL statements in dependency order.
pub const ALL_DDL: &[&str] = &[
    CREATE_STATE_ATTRIBUTES,
    CREATE_STATES,
    CREATE_STATE_REMOVALS,
    CREATE_EVENTS,
    CREATE_RECORDER_RUNS,
];
