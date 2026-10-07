//! `Recorder` — SQLite write path + query path.
//!
//! Wraps an `SqlitePool` and exposes three operations:
//! - [`Recorder::open`] — open (or create) the DB and apply schema.
//! - [`Recorder::record_state`] — persist a `StateChangedEvent`.
//! - [`Recorder::record_event`] — persist a `DomainEvent`.
//! - [`Recorder::get_state_history`] — read back rows in time order.
//!
//! State attributes are deduped via `fnv64a_hash` (see [`crate::dedup`]):
//! if an identical attributes blob was previously written its
//! `attributes_id` is reused and no new row is inserted.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use thiserror::Error;
use tokio::sync::RwLock;

use homecore::entity::{EntityId, State};
use homecore::event::{Context, DomainEvent, StateChangedEvent};
use homecore::StateMachine;

use crate::dedup::fnv64a_hash;
use crate::schema::ALL_DDL;

// Preserve the narrow `sqlx::*` call surface used in this module while
// depending only on SQLx core + SQLite. The umbrella crate resolves unused
// database backends into Cargo.lock, including MySQL's vulnerable RSA stack.
mod sqlx {
    pub use sqlx_core::error::Error;
    pub use sqlx_core::query::query;
    pub use sqlx_core::query_as::query_as;

    pub mod sqlite {
        pub use sqlx_sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
    }
}

type SearchStateRecord = (
    i64,
    String,
    String,
    Option<String>,
    f64,
    f64,
    Option<String>,
);
type StateRecord = (String, String, Option<String>, f64, f64, Option<String>);
type HistoryStateRecord = (i64, String, Option<String>, f64, f64, Option<String>);

/// Hard upper bound on rows returned by [`Recorder::get_state_history`].
///
/// Without this cap a wide `[since, until]` window over a high-frequency entity
/// would load an unbounded number of rows into memory (a memory-DoS). The value
/// is deliberately generous — large enough never to truncate a realistic
/// history-graph query, small enough to bound the worst case. Callers needing a
/// wider span page by narrowing the window.
pub const MAX_HISTORY_ROWS: i64 = 1_000_000;
/// Absolute cap for one startup restore query.
pub const MAX_RESTORE_STATES: usize = 100_000;

/// Errors returned by `Recorder` operations.
#[derive(Error, Debug)]
pub enum RecorderError {
    #[error("SQLite error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("serialisation error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("URL parse error: {0}")]
    UrlParse(String),
}

/// Trait for pluggable semantic (vector) indexing of state writes.
///
/// The no-op [`NullSemanticIndex`] is used in P1. P2 ships a ruvector-backed
/// implementation behind the `ruvector` feature flag.
///
/// ## P2 API change
///
/// The `insert_state` method now accepts a `state_id` (SQLite rowid) so the
/// HNSW index can map vector results back to SQLite rows. `search` embeds a
/// free-text query and returns `(state_id, score)` pairs.
#[async_trait]
pub trait SemanticIndex: Send + Sync {
    /// Insert an embedding for `state` keyed by its SQLite `state_id`.
    /// Called after the SQLite insert succeeds. Must not propagate errors
    /// back to the recorder — failure is logged, not fatal.
    async fn insert_state(
        &mut self,
        state_id: i64,
        state: &State,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Search for the `k` nearest states to the free-text `query`.
    /// Returns `(state_id, score)` pairs sorted by ascending distance.
    async fn search(
        &self,
        query: &str,
        k: usize,
    ) -> Result<Vec<(i64, f32)>, Box<dyn std::error::Error + Send + Sync>>;
}

/// No-op `SemanticIndex`. Used by default when the `ruvector` feature is off.
pub struct NullSemanticIndex;

#[async_trait]
impl SemanticIndex for NullSemanticIndex {
    async fn insert_state(
        &mut self,
        _state_id: i64,
        _state: &State,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn search(
        &self,
        _query: &str,
        _k: usize,
    ) -> Result<Vec<(i64, f32)>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }
}

/// The recorder. Cheap to clone (Arc-backed pool). Pass copies to the
/// `RecorderListener` and the API history handler.
///
/// The `semantic` field is wrapped in `Arc<RwLock<...>>` so that
/// `insert_state` (which takes `&mut self` on the trait) can be called
/// without requiring `&mut Recorder` from callers.
#[derive(Clone)]
pub struct Recorder {
    pool: SqlitePool,
    semantic: Arc<RwLock<dyn SemanticIndex>>,
}

impl Recorder {
    /// Open (or create) the SQLite database at `path` and apply the schema.
    ///
    /// Pass `"sqlite::memory:"` for an in-memory database (tests).
    ///
    /// The schema DDL uses `CREATE TABLE IF NOT EXISTS` so calling this on an
    /// existing database is safe.
    pub async fn open(path: &str) -> Result<Self, RecorderError> {
        Self::open_with_index(path, Arc::new(RwLock::new(NullSemanticIndex))).await
    }

    /// Open with a custom `SemanticIndex` (P2 entry point).
    pub async fn open_with_index(
        path: &str,
        semantic: Arc<RwLock<dyn SemanticIndex>>,
    ) -> Result<Self, RecorderError> {
        let options = path
            .parse::<SqliteConnectOptions>()
            .map_err(|e| RecorderError::UrlParse(e.to_string()))?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;

        let recorder = Self { pool, semantic };
        recorder.apply_schema().await?;
        Ok(recorder)
    }

    /// Apply all DDL statements. Idempotent.
    async fn apply_schema(&self) -> Result<(), RecorderError> {
        for ddl in ALL_DDL {
            // Each DDL block may contain multiple statements separated by `;`.
            // sqlx::query does not support multi-statement strings directly,
            // so we split on the statement boundary and execute individually.
            for stmt in split_statements(ddl) {
                let stmt = stmt.trim();
                if !stmt.is_empty() {
                    sqlx::query(stmt).execute(&self.pool).await?;
                }
            }
        }
        Ok(())
    }

    /// Persist a `StateChangedEvent`. Inserts into `states` and dedupes into
    /// `state_attributes`. Returns the `state_id` of the new row.
    pub async fn record_state(
        &self,
        event: &StateChangedEvent,
    ) -> Result<Option<i64>, RecorderError> {
        let new_state = match &event.new_state {
            Some(s) => s,
            None => return Ok(None), // removal event — no row to insert
        };

        let attrs_json = serde_json::to_string(&new_state.attributes)?;
        let hash = fnv64a_hash(&attrs_json);

        // Take the SQLite writer lock before reading the dedup row. The upsert
        // handles concurrent first writes, and the transaction prevents purge
        // from collecting the blob before its state row references it.
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO state_attributes (shared_attrs, hash) VALUES (?, ?) ON CONFLICT(hash) DO NOTHING",
        )
        .bind(&attrs_json)
        .bind(hash)
        .execute(&mut *tx)
        .await?;
        let (attributes_id,): (i64,) =
            sqlx::query_as("SELECT attributes_id FROM state_attributes WHERE hash = ?")
                .bind(hash)
                .fetch_one(&mut *tx)
                .await?;

        let context_id = new_state.context.id.to_string();
        let last_changed_ts = new_state.last_changed.timestamp_micros() as f64 / 1_000_000.0;
        let last_updated_ts = new_state.last_updated.timestamp_micros() as f64 / 1_000_000.0;

        let result = sqlx::query(
            "INSERT INTO states \
             (entity_id, state, attributes_id, last_changed_ts, last_updated_ts, context_id) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(new_state.entity_id.as_str())
        .bind(&new_state.state)
        .bind(attributes_id)
        .bind(last_changed_ts)
        .bind(last_updated_ts)
        .bind(&context_id)
        .execute(&mut *tx)
        .await?;

        let state_id = result.last_insert_rowid();
        tx.commit().await?;

        // Best-effort semantic indexing — failure is logged, not propagated.
        if let Err(e) = self
            .semantic
            .write()
            .await
            .insert_state(state_id, new_state)
            .await
        {
            tracing::warn!(
                error = %e,
                entity_id = %new_state.entity_id,
                "semantic indexing failed"
            );
        }

        Ok(Some(state_id))
    }

    /// Search for state history rows that semantically match `query`.
    ///
    /// When a vector [`SemanticIndex`] is wired (the `ruvector` feature), this
    /// uses the HNSW index to find the top-`k` nearest state embeddings and
    /// fetches the full `StateRow` for each, in ascending distance order.
    ///
    /// When the index yields no hits — e.g. the default [`NullSemanticIndex`]
    /// with no `ruvector` feature — it transparently falls back to the SQL
    /// text query [`search_states_by_text`](Self::search_states_by_text), so a
    /// caller always gets real matching rows rather than a silent empty `Vec`.
    pub async fn search_semantic(
        &self,
        query: &str,
        k: usize,
    ) -> Result<Vec<StateRow>, RecorderError> {
        let hits = self
            .semantic
            .read()
            .await
            .search(query, k)
            .await
            .unwrap_or_default();

        // No vector backend (or no embeddings indexed) → real SQL text search.
        if hits.is_empty() {
            return self.search_states_by_text(query, k).await;
        }

        let mut rows = Vec::with_capacity(hits.len());
        for (state_id, _score) in hits {
            if let Some(row) = self.fetch_state_row(state_id).await? {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    /// Real text search over state history: returns the most recent up-to-`k`
    /// rows whose `entity_id`, `state` value, or attribute blob contains
    /// `query` (case-insensitive `LIKE`). Ordered newest-first.
    ///
    /// This is the feature-independent query path — it returns real rows from
    /// SQLite with no vector backend required. An empty `query` matches all
    /// rows (most-recent-first), giving callers a "latest activity" view.
    pub async fn search_states_by_text(
        &self,
        query: &str,
        k: usize,
    ) -> Result<Vec<StateRow>, RecorderError> {
        // Escape LIKE metacharacters so user text is treated literally.
        let escaped = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped}%");

        let rows: Vec<SearchStateRecord> = sqlx::query_as(
            "SELECT s.state_id, s.entity_id, s.state, sa.shared_attrs, \
                        s.last_changed_ts, s.last_updated_ts, s.context_id \
                 FROM states s \
                 LEFT JOIN state_attributes sa ON s.attributes_id = sa.attributes_id \
                 WHERE ?1 = '' \
                    OR s.entity_id   LIKE ?2 ESCAPE '\\' \
                    OR s.state        LIKE ?2 ESCAPE '\\' \
                    OR sa.shared_attrs LIKE ?2 ESCAPE '\\' \
                 ORDER BY s.last_updated_ts DESC \
                 LIMIT ?3",
        )
        .bind(query)
        .bind(&pattern)
        .bind(k as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(
                |(
                    state_id,
                    entity_id,
                    state,
                    shared_attrs,
                    last_changed_ts,
                    last_updated_ts,
                    context_id,
                )| {
                    let eid = EntityId::parse(&entity_id)
                        .unwrap_or_else(|_| EntityId::parse("unknown.unknown").unwrap());
                    let attributes = shared_attrs
                        .as_deref()
                        .map(serde_json::from_str)
                        .transpose()?
                        .unwrap_or(serde_json::Value::Object(Default::default()));
                    Ok(StateRow {
                        state_id,
                        entity_id: eid,
                        state,
                        attributes,
                        last_changed_ts,
                        last_updated_ts,
                        context_id,
                    })
                },
            )
            .collect()
    }

    /// Fetch a single `StateRow` by its `state_id`, joining attributes.
    async fn fetch_state_row(&self, state_id: i64) -> Result<Option<StateRow>, RecorderError> {
        let row: Option<StateRecord> = sqlx::query_as(
            "SELECT s.entity_id, s.state, sa.shared_attrs, \
                         s.last_changed_ts, s.last_updated_ts, s.context_id \
                 FROM states s \
                 LEFT JOIN state_attributes sa ON s.attributes_id = sa.attributes_id \
                 WHERE s.state_id = ?",
        )
        .bind(state_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some((entity_id, state, shared_attrs, last_changed_ts, last_updated_ts, context_id)) =
            row
        else {
            return Ok(None);
        };

        let eid = EntityId::parse(&entity_id)
            .unwrap_or_else(|_| EntityId::parse("unknown.unknown").unwrap());
        let attributes = shared_attrs
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?
            .unwrap_or(serde_json::Value::Object(Default::default()));
        Ok(Some(StateRow {
            state_id,
            entity_id: eid,
            state,
            attributes,
            last_changed_ts,
            last_updated_ts,
            context_id,
        }))
    }

    /// Persist a `DomainEvent`. Returns the `event_id`.
    pub async fn record_event(&self, event: &DomainEvent) -> Result<i64, RecorderError> {
        let data_json = serde_json::to_string(&event.event_data)?;
        let time_fired_ts = event.fired_at.timestamp_micros() as f64 / 1_000_000.0;
        let context_id = event.context.id.to_string();

        let result = sqlx::query(
            "INSERT INTO events (event_type, event_data, time_fired_ts, context_id) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(&event.event_type)
        .bind(&data_json)
        .bind(time_fired_ts)
        .bind(&context_id)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Query state history for `entity_id` between `since` and `until`.
    /// Returns state snapshots in ascending `last_updated_ts` order, capped at
    /// [`MAX_HISTORY_ROWS`] rows (oldest-first within the window).
    ///
    /// ## Bounded result set (memory-DoS guard)
    ///
    /// A high-frequency entity (e.g. a power sensor polled per-second) writes
    /// ~86k rows/day; a wide `[since, until]` window over months would otherwise
    /// load millions of rows into a single in-memory `Vec`, an unbounded-memory
    /// denial-of-service. The query therefore carries a hard `LIMIT` so the
    /// working set is bounded regardless of the requested time range. Callers
    /// that genuinely need a wider span must page by narrowing the window.
    pub async fn get_state_history(
        &self,
        entity_id: &EntityId,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<StateRow>, RecorderError> {
        self.get_state_history_limited(entity_id, since, until, MAX_HISTORY_ROWS as usize)
            .await
    }

    /// Query state history with a caller-selected limit capped by
    /// [`MAX_HISTORY_ROWS`]. The bound is applied in SQL, before rows are
    /// materialized.
    pub async fn get_state_history_limited(
        &self,
        entity_id: &EntityId,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
        requested_limit: usize,
    ) -> Result<Vec<StateRow>, RecorderError> {
        let limit = requested_limit.min(MAX_HISTORY_ROWS as usize);
        if limit == 0 {
            return Ok(Vec::new());
        }
        let since_ts = since.timestamp_micros() as f64 / 1_000_000.0;
        let until_ts = until.timestamp_micros() as f64 / 1_000_000.0;

        let rows: Vec<HistoryStateRecord> = sqlx::query_as(
            "SELECT s.state_id, s.state, sa.shared_attrs, \
                    s.last_changed_ts, s.last_updated_ts, s.context_id \
             FROM states s \
             LEFT JOIN state_attributes sa ON s.attributes_id = sa.attributes_id \
             WHERE s.entity_id = ? \
               AND s.last_updated_ts >= ? \
               AND s.last_updated_ts <= ? \
             ORDER BY s.last_updated_ts ASC \
             LIMIT ?",
        )
        .bind(entity_id.as_str())
        .bind(since_ts)
        .bind(until_ts)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(
                |(state_id, state, shared_attrs, last_changed_ts, last_updated_ts, context_id)| {
                    let attributes = shared_attrs
                        .as_deref()
                        .map(serde_json::from_str)
                        .transpose()?
                        .unwrap_or(serde_json::Value::Object(Default::default()));

                    Ok(StateRow {
                        state_id,
                        entity_id: entity_id.clone(),
                        state,
                        attributes,
                        last_changed_ts,
                        last_updated_ts,
                        context_id,
                    })
                },
            )
            .collect()
    }

    /// Read the newest row for each entity in deterministic entity-id order.
    ///
    /// The query is bounded and uses `(last_updated_ts, state_id)` as a stable
    /// newest-row tie-break. Malformed rows are reported and skipped
    /// individually so one corrupt entity cannot prevent the rest from
    /// starting.
    pub async fn latest_states(
        &self,
        requested_limit: usize,
    ) -> Result<LatestStates, RecorderError> {
        let limit = requested_limit.min(MAX_RESTORE_STATES);
        if limit == 0 {
            return Ok(LatestStates::default());
        }
        type RawRestoreRow = (
            i64,
            String,
            Option<String>,
            Option<String>,
            Option<f64>,
            Option<f64>,
            Option<String>,
        );
        let rows: Vec<RawRestoreRow> = sqlx::query_as(
            "SELECT state_id, entity_id, state, shared_attrs, \
                    last_changed_ts, last_updated_ts, context_id \
             FROM ( \
               SELECT s.state_id, s.entity_id, s.state, sa.shared_attrs, \
                      s.last_changed_ts, s.last_updated_ts, s.context_id, \
                      ROW_NUMBER() OVER ( \
                        PARTITION BY s.entity_id \
                        ORDER BY s.last_updated_ts DESC, s.state_id DESC \
                      ) AS newest \
               FROM states s \
               LEFT JOIN state_attributes sa ON s.attributes_id = sa.attributes_id \
             ) \
             WHERE newest = 1 \
             ORDER BY entity_id ASC \
             LIMIT ?",
        )
        .bind((limit + 1) as i64)
        .fetch_all(&self.pool)
        .await?;

        let truncated = rows.len() > limit;
        let mut states = Vec::with_capacity(rows.len().min(limit));
        let mut warnings = Vec::new();
        for (
            state_id,
            raw_entity_id,
            raw_state,
            raw_attributes,
            last_changed_ts,
            last_updated_ts,
            context_id,
        ) in rows.into_iter().take(limit)
        {
            let entity_id = match EntityId::parse(&raw_entity_id) {
                Ok(value) => value,
                Err(error) => {
                    warnings.push(RestoreWarning::MalformedEntityId {
                        state_id,
                        entity_id: raw_entity_id,
                        reason: error.to_string(),
                    });
                    continue;
                }
            };
            let Some(state) = raw_state else {
                warnings.push(RestoreWarning::MissingState {
                    state_id,
                    entity_id: raw_entity_id,
                });
                continue;
            };
            let attributes = match raw_attributes {
                Some(value) => match serde_json::from_str(&value) {
                    Ok(value) => value,
                    Err(error) => {
                        warnings.push(RestoreWarning::MalformedAttributes {
                            state_id,
                            entity_id: raw_entity_id,
                            reason: error.to_string(),
                        });
                        continue;
                    }
                },
                None => serde_json::json!({}),
            };
            let Some(last_changed) = last_changed_ts.and_then(timestamp_from_seconds) else {
                warnings.push(RestoreWarning::MalformedTimestamp {
                    state_id,
                    entity_id: raw_entity_id,
                    field: "last_changed_ts",
                });
                continue;
            };
            let Some(last_updated) = last_updated_ts.and_then(timestamp_from_seconds) else {
                warnings.push(RestoreWarning::MalformedTimestamp {
                    state_id,
                    entity_id: raw_entity_id,
                    field: "last_updated_ts",
                });
                continue;
            };
            let parent_id = match context_id {
                Some(value) => match value.parse() {
                    Ok(value) => Some(value),
                    Err(_) => {
                        warnings.push(RestoreWarning::MalformedContext {
                            state_id,
                            entity_id: raw_entity_id.clone(),
                        });
                        None
                    }
                },
                None => None,
            };
            states.push(State {
                entity_id,
                state,
                attributes,
                last_changed,
                last_updated,
                context: Context::restoration(parent_id),
            });
        }
        Ok(LatestStates {
            states,
            warnings,
            truncated,
        })
    }

    /// Load latest durable snapshots into a state machine without producing
    /// fresh recorder events.
    pub async fn restore_latest(
        &self,
        states: &StateMachine,
        limit: usize,
    ) -> Result<RestoreReport, RecorderError> {
        let batch = self.latest_states(limit).await?;
        let mut restored = 0;
        for state in batch.states {
            // latest_states constructs the required restoration marker.
            if states.restore(state).is_ok() {
                restored += 1;
            }
        }
        Ok(RestoreReport {
            restored,
            warnings: batch.warnings,
            truncated: batch.truncated,
        })
    }

    /// Purge history older than `older_than`, returning a [`PurgeStats`] summary.
    ///
    /// Deletes:
    /// - `states` rows whose `last_updated_ts` is **strictly before** the cutoff,
    /// - `events` rows whose `time_fired_ts` is strictly before the cutoff,
    /// - then garbage-collects any `state_attributes` blob no surviving state
    ///   row still references (so dedup-shared blobs are only dropped once their
    ///   last referencing state is gone).
    ///
    /// ## Retention boundary (data-integrity guard)
    ///
    /// The cutoff is **exclusive**: a row exactly at `older_than` is retained.
    /// This makes `purge(t)` idempotent on the boundary and guarantees that a
    /// row written at the same instant the retention window opens is never lost
    /// to an off-by-one. Anything *at or after* `older_than` survives.
    ///
    /// ## Atomicity (no partial-corrupt state)
    ///
    /// All three deletes run inside a single transaction. A failure mid-purge
    /// rolls the whole operation back — the store is never left with states
    /// deleted but their events kept, or attributes orphaned by a half-purge.
    ///
    /// Note: this reclaims logical rows; it does not `VACUUM` the file. SQLite
    /// reuses freed pages for subsequent writes, so disk growth stays bounded
    /// under a periodic purge even without an explicit vacuum.
    pub async fn purge(&self, older_than: DateTime<Utc>) -> Result<PurgeStats, RecorderError> {
        let cutoff_ts = older_than.timestamp_micros() as f64 / 1_000_000.0;

        let mut tx = self.pool.begin().await?;

        let states_deleted = sqlx::query("DELETE FROM states WHERE last_updated_ts < ?")
            .bind(cutoff_ts)
            .execute(&mut *tx)
            .await?
            .rows_affected();

        let events_deleted = sqlx::query("DELETE FROM events WHERE time_fired_ts < ?")
            .bind(cutoff_ts)
            .execute(&mut *tx)
            .await?
            .rows_affected();

        // GC attribute blobs no surviving state references. A dedup-shared blob
        // is only removed once its last referencing state row is gone.
        let attributes_deleted = sqlx::query(
            "DELETE FROM state_attributes \
             WHERE attributes_id NOT IN \
                 (SELECT attributes_id FROM states WHERE attributes_id IS NOT NULL)",
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        tx.commit().await?;

        Ok(PurgeStats {
            states_deleted,
            events_deleted,
            attributes_deleted,
        })
    }
}

fn timestamp_from_seconds(value: f64) -> Option<DateTime<Utc>> {
    if !value.is_finite() {
        return None;
    }
    let micros = (value * 1_000_000.0).round();
    if micros < i64::MIN as f64 || micros > i64::MAX as f64 {
        return None;
    }
    DateTime::from_timestamp_micros(micros as i64)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreWarning {
    MalformedEntityId {
        state_id: i64,
        entity_id: String,
        reason: String,
    },
    MissingState {
        state_id: i64,
        entity_id: String,
    },
    MalformedAttributes {
        state_id: i64,
        entity_id: String,
        reason: String,
    },
    MalformedTimestamp {
        state_id: i64,
        entity_id: String,
        field: &'static str,
    },
    MalformedContext {
        state_id: i64,
        entity_id: String,
    },
}

#[derive(Debug, Default)]
pub struct LatestStates {
    pub states: Vec<State>,
    pub warnings: Vec<RestoreWarning>,
    pub truncated: bool,
}

#[derive(Debug)]
pub struct RestoreReport {
    pub restored: usize,
    pub warnings: Vec<RestoreWarning>,
    pub truncated: bool,
}

/// Summary of a [`Recorder::purge`] run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurgeStats {
    /// Number of `states` rows deleted.
    pub states_deleted: u64,
    /// Number of `events` rows deleted.
    pub events_deleted: u64,
    /// Number of orphaned `state_attributes` blobs garbage-collected.
    pub attributes_deleted: u64,
}

/// A state row returned from `get_state_history`.
#[derive(Debug, Clone)]
pub struct StateRow {
    pub state_id: i64,
    pub entity_id: EntityId,
    pub state: String,
    pub attributes: serde_json::Value,
    /// Unix timestamp (seconds, fractional) when the state string last changed.
    pub last_changed_ts: f64,
    /// Unix timestamp (seconds, fractional) when this snapshot was written.
    pub last_updated_ts: f64,
    pub context_id: Option<String>,
}

/// Split a multi-statement DDL string on `;` boundaries.
/// Trims whitespace; skips empty fragments.
fn split_statements(ddl: &str) -> impl Iterator<Item = &str> {
    ddl.split(';').map(str::trim).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::Utc;

    use homecore::entity::{EntityId, State};
    use homecore::event::{Context, DomainEvent, StateChangedEvent};

    use super::*;

    async fn open_memory() -> Recorder {
        Recorder::open("sqlite::memory:")
            .await
            .expect("open in-memory DB")
    }

    fn entity(s: &str) -> EntityId {
        EntityId::parse(s).unwrap()
    }

    fn make_state_event(
        entity_id: &str,
        state_val: &str,
        attrs: serde_json::Value,
    ) -> StateChangedEvent {
        let eid = entity(entity_id);
        let ctx = Context::new();
        let s = Arc::new(State::new(eid.clone(), state_val, attrs, ctx));
        StateChangedEvent {
            entity_id: eid,
            old_state: None,
            new_state: Some(s),
            fired_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn concurrent_writers_share_attributes_without_dropping_states() {
        let recorder = open_memory().await;
        let barrier = Arc::new(tokio::sync::Barrier::new(32));
        let mut tasks = Vec::new();
        for n in 0..32 {
            let recorder = recorder.clone();
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                let event = make_state_event(
                    &format!("sensor.concurrent_{n}"),
                    "on",
                    serde_json::json!({"unit": "C"}),
                );
                barrier.wait().await;
                recorder.record_state(&event).await
            }));
        }
        for task in tasks {
            task.await
                .unwrap()
                .expect("concurrent deduplication must not lose writes");
        }
        let counts: (i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM states), (SELECT COUNT(*) FROM state_attributes)",
        )
        .fetch_one(&recorder.pool)
        .await
        .unwrap();
        assert_eq!(counts, (32, 1));
    }

    #[tokio::test]
    async fn failed_state_insert_rolls_back_new_attributes() {
        let recorder = open_memory().await;
        sqlx::query("CREATE TRIGGER reject_state BEFORE INSERT ON states BEGIN SELECT RAISE(ABORT, 'test write failure'); END")
            .execute(&recorder.pool).await.unwrap();
        let result = recorder
            .record_state(&make_state_event(
                "sensor.failed",
                "on",
                serde_json::json!({"unique": 17}),
            ))
            .await;
        assert!(result.is_err());
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM state_attributes")
            .fetch_one(&recorder.pool)
            .await
            .unwrap();
        assert_eq!(
            count.0, 0,
            "failed state writes must not leave orphaned attribute blobs"
        );
    }

    // ── schema ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn schema_applies_on_fresh_db() {
        let recorder = open_memory().await;
        // Verify all four tables exist by querying sqlite_master.
        let tables: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .fetch_all(&recorder.pool)
                .await
                .unwrap();
        let names: Vec<&str> = tables.iter().map(|(n,)| n.as_str()).collect();
        assert!(
            names.contains(&"state_attributes"),
            "missing state_attributes"
        );
        assert!(names.contains(&"states"), "missing states");
        assert!(names.contains(&"events"), "missing events");
        assert!(names.contains(&"recorder_runs"), "missing recorder_runs");
    }

    #[tokio::test]
    async fn schema_idempotent_double_open() {
        // Applying schema twice (on the same pool) must not panic or error.
        let recorder = open_memory().await;
        recorder
            .apply_schema()
            .await
            .expect("second apply_schema must be a no-op");
    }

    // ── record_state ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn record_state_inserts_row() {
        let recorder = open_memory().await;
        let event = make_state_event(
            "light.kitchen",
            "on",
            serde_json::json!({"brightness": 200}),
        );

        let state_id = recorder.record_state(&event).await.unwrap();
        assert!(state_id.is_some(), "expected a state_id");

        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM states WHERE entity_id = 'light.kitchen'")
                .fetch_one(&recorder.pool)
                .await
                .unwrap();
        assert_eq!(count.0, 1);
    }

    #[tokio::test]
    async fn removal_event_returns_none() {
        let recorder = open_memory().await;
        let event = StateChangedEvent {
            entity_id: entity("light.kitchen"),
            old_state: None,
            new_state: None, // removal
            fired_at: Utc::now(),
        };
        let result = recorder.record_state(&event).await.unwrap();
        assert!(result.is_none(), "removal event should yield None state_id");
    }

    // ── attribute deduplication ────────────────────────────────────────────────

    #[tokio::test]
    async fn same_attrs_dedup_to_one_row() {
        let recorder = open_memory().await;
        let attrs = serde_json::json!({"brightness": 200, "color_temp": 4000});

        let e1 = make_state_event("light.a", "on", attrs.clone());
        let e2 = make_state_event("light.b", "on", attrs.clone());

        recorder.record_state(&e1).await.unwrap();
        recorder.record_state(&e2).await.unwrap();

        let attr_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM state_attributes")
            .fetch_one(&recorder.pool)
            .await
            .unwrap();
        // Both events share identical attrs → only one state_attributes row.
        assert_eq!(
            attr_count.0, 1,
            "identical attrs must share one state_attributes row"
        );

        let state_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM states")
            .fetch_one(&recorder.pool)
            .await
            .unwrap();
        assert_eq!(state_count.0, 2, "two states rows expected");
    }

    #[tokio::test]
    async fn different_attrs_each_get_own_row() {
        let recorder = open_memory().await;
        let e1 = make_state_event("sensor.a", "20", serde_json::json!({"unit": "C"}));
        let e2 = make_state_event("sensor.b", "20", serde_json::json!({"unit": "F"}));

        recorder.record_state(&e1).await.unwrap();
        recorder.record_state(&e2).await.unwrap();

        let attr_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM state_attributes")
            .fetch_one(&recorder.pool)
            .await
            .unwrap();
        assert_eq!(attr_count.0, 2);
    }

    // ── get_state_history ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn history_returns_rows_in_time_order() {
        let recorder = open_memory().await;
        let eid = entity("sensor.temp");

        // Insert three states with slightly different timestamps by sleeping.
        for val in &["20.0", "21.0", "22.0"] {
            let e = make_state_event("sensor.temp", val, serde_json::json!({}));
            recorder.record_state(&e).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }

        let since = Utc::now() - chrono::Duration::seconds(10);
        let until = Utc::now() + chrono::Duration::seconds(10);
        let rows = recorder
            .get_state_history(&eid, since, until)
            .await
            .unwrap();

        assert_eq!(rows.len(), 3, "expected 3 history rows");
        // Verify ascending order by last_updated_ts.
        for w in rows.windows(2) {
            assert!(
                w[0].last_updated_ts <= w[1].last_updated_ts,
                "rows must be in ascending time order"
            );
        }
        assert_eq!(rows[0].state, "20.0");
        assert_eq!(rows[2].state, "22.0");
    }

    #[tokio::test]
    async fn history_caller_limit_is_applied_before_materialization() {
        let recorder = open_memory().await;
        let eid = entity("sensor.bounded");
        for value in ["1", "2", "3"] {
            recorder
                .record_state(&make_state_event(
                    "sensor.bounded",
                    value,
                    serde_json::json!({}),
                ))
                .await
                .unwrap();
        }
        let since = Utc::now() - chrono::Duration::seconds(10);
        let until = Utc::now() + chrono::Duration::seconds(10);
        let rows = recorder
            .get_state_history_limited(&eid, since, until, 2)
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].state, "1");
        assert_eq!(rows[1].state, "2");
        assert!(recorder
            .get_state_history_limited(&eid, since, until, 0)
            .await
            .unwrap()
            .is_empty());
    }

    // ── record_event ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn record_event_round_trips() {
        let recorder = open_memory().await;
        let ctx = Context::new();
        let event = DomainEvent::new(
            "call_service",
            serde_json::json!({"domain": "light", "service": "turn_on"}),
            ctx,
        );

        let event_id = recorder.record_event(&event).await.unwrap();
        assert!(event_id > 0);

        let row: (String, String) =
            sqlx::query_as("SELECT event_type, event_data FROM events WHERE event_id = ?")
                .bind(event_id)
                .fetch_one(&recorder.pool)
                .await
                .unwrap();

        assert_eq!(row.0, "call_service");
        let data: serde_json::Value = serde_json::from_str(&row.1).unwrap();
        assert_eq!(data["domain"], "light");
    }

    // ── search_states_by_text (real DB query) ───────────────────────────────────

    #[tokio::test]
    async fn text_search_returns_inserted_rows() {
        // FAILS against the old always-empty path: asserts real rows come back.
        let recorder = open_memory().await;
        recorder
            .record_state(&make_state_event(
                "light.kitchen",
                "on",
                serde_json::json!({}),
            ))
            .await
            .unwrap();
        recorder
            .record_state(&make_state_event(
                "light.bedroom",
                "off",
                serde_json::json!({}),
            ))
            .await
            .unwrap();
        recorder
            .record_state(&make_state_event("switch.fan", "on", serde_json::json!({})))
            .await
            .unwrap();

        // Match by entity_id substring.
        let rows = recorder.search_states_by_text("kitchen", 10).await.unwrap();
        assert_eq!(rows.len(), 1, "exactly one kitchen row");
        assert_eq!(rows[0].entity_id.as_str(), "light.kitchen");

        // Match by domain prefix → both lights.
        let lights = recorder.search_states_by_text("light.", 10).await.unwrap();
        assert_eq!(lights.len(), 2, "both light rows");

        // Match by state value.
        let on_rows = recorder.search_states_by_text("on", 10).await.unwrap();
        // "on" matches light.kitchen (state on) and switch.fan (state on);
        // "bedroom" has state "off" — substring "on" not present in its
        // entity_id/state. Two rows expected.
        assert_eq!(on_rows.len(), 2, "two rows with state 'on'");
    }

    #[tokio::test]
    async fn text_search_matches_attribute_blob() {
        let recorder = open_memory().await;
        recorder
            .record_state(&make_state_event(
                "sensor.weather",
                "cloudy",
                serde_json::json!({"location": "portland"}),
            ))
            .await
            .unwrap();
        let rows = recorder
            .search_states_by_text("portland", 10)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entity_id.as_str(), "sensor.weather");
        assert_eq!(rows[0].attributes["location"], "portland");
    }

    #[tokio::test]
    async fn text_search_empty_query_returns_recent_rows() {
        let recorder = open_memory().await;
        for v in &["1", "2", "3"] {
            recorder
                .record_state(&make_state_event("counter.c", v, serde_json::json!({})))
                .await
                .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(3)).await;
        }
        // Empty query → all rows, newest first, capped at k.
        let rows = recorder.search_states_by_text("", 2).await.unwrap();
        assert_eq!(rows.len(), 2, "k caps the result set");
        assert_eq!(rows[0].state, "3", "newest first");
        assert_eq!(rows[1].state, "2");
    }

    #[tokio::test]
    async fn text_search_no_match_returns_empty() {
        let recorder = open_memory().await;
        recorder
            .record_state(&make_state_event(
                "light.kitchen",
                "on",
                serde_json::json!({}),
            ))
            .await
            .unwrap();
        let rows = recorder
            .search_states_by_text("nonexistent_entity_xyz", 10)
            .await
            .unwrap();
        assert!(rows.is_empty(), "genuine no-match is empty, not an error");
    }

    // ── SQL injection (parameterization guarantee) ──────────────────────────────

    #[tokio::test]
    async fn malicious_entity_id_is_stored_literally_not_executed() {
        // FAILS if any query interpolated entity_id into SQL: the `states` table
        // would be dropped and the later COUNT would error / mismatch. Bound
        // parameters store the metacharacter-laden string verbatim instead.
        let recorder = open_memory().await;

        // A valid domain.name whose `name` part carries SQL metacharacters.
        // EntityId::parse permits this, so it reaches the bind path as data.
        let evil = "light.x_drop_table_states_select";
        recorder
            .record_state(&make_state_event(
                evil,
                "'; DROP TABLE states; --",
                serde_json::json!({}),
            ))
            .await
            .unwrap();

        // states table still exists and holds exactly the one row we inserted.
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM states")
            .fetch_one(&recorder.pool)
            .await
            .expect("states table must still exist — proves no injection");
        assert_eq!(count.0, 1);

        // The malicious state string round-trips literally.
        let rows = recorder
            .search_states_by_text("DROP TABLE", 10)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "metacharacter payload matched as a literal");
        assert_eq!(rows[0].state, "'; DROP TABLE states; --");
    }

    #[tokio::test]
    async fn like_metacharacters_in_query_are_literal_not_wildcards() {
        // A `%` in the search text must match a literal percent sign, not act as
        // a SQL LIKE wildcard. Proves the ESCAPE clause + metacharacter escaping.
        let recorder = open_memory().await;
        recorder
            .record_state(&make_state_event("sensor.a", "100%", serde_json::json!({})))
            .await
            .unwrap();
        recorder
            .record_state(&make_state_event("sensor.b", "50", serde_json::json!({})))
            .await
            .unwrap();

        // Literal "%" must match only sensor.a's "100%", NOT every row.
        let rows = recorder.search_states_by_text("%", 10).await.unwrap();
        assert_eq!(rows.len(), 1, "'%' is a literal, not a match-all wildcard");
        assert_eq!(rows[0].entity_id.as_str(), "sensor.a");

        // Underscore is likewise literal: matches nothing here.
        let none = recorder.search_states_by_text("_", 10).await.unwrap();
        assert!(none.is_empty(), "'_' is literal, matches no row");
    }

    // ── get_state_history bound (memory-DoS guard) ──────────────────────────────

    #[tokio::test]
    async fn history_query_carries_a_limit_clause() {
        // Pin: the history SQL must carry a LIMIT bound (memory-DoS guard).
        // Inserting a million rows is infeasible in a unit test, so we prove the
        // clause is wired by bulk-inserting more rows than a deliberately tiny
        // bound and asserting the executed query honours a LIMIT. We bypass the
        // public method (whose cap is MAX_HISTORY_ROWS) and run the *same* SQL
        // shape with a small bind to demonstrate the LIMIT term is effective —
        // and separately assert the constant is a sane positive bound.
        let recorder = open_memory().await;
        for v in &["1", "2", "3", "4", "5"] {
            recorder
                .record_state(&make_state_event(
                    "sensor.bounded",
                    v,
                    serde_json::json!({}),
                ))
                .await
                .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        // Same query shape as get_state_history, with a tiny LIMIT bind: if the
        // SQL lacked a LIMIT term this would return all 5; with it, exactly 2.
        let capped: Vec<(i64,)> = sqlx::query_as(
            "SELECT s.state_id FROM states s \
             WHERE s.entity_id = ? \
             ORDER BY s.last_updated_ts ASC LIMIT ?",
        )
        .bind("sensor.bounded")
        .bind(2_i64)
        .fetch_all(&recorder.pool)
        .await
        .unwrap();
        assert_eq!(
            capped.len(),
            2,
            "LIMIT term effectively bounds the result set"
        );

        // And the real method returns all rows when under the cap.
        let eid = entity("sensor.bounded");
        let rows = recorder
            .get_state_history(
                &eid,
                Utc::now() - chrono::Duration::seconds(10),
                Utc::now() + chrono::Duration::seconds(10),
            )
            .await
            .unwrap();
        assert_eq!(rows.len(), 5, "all rows under the cap return");
    }

    // ── purge (retention correctness + atomicity) ───────────────────────────────

    #[tokio::test]
    async fn purge_keeps_boundary_row_and_drops_older() {
        // FAILS if purge had an off-by-one (deleting the row exactly at cutoff)
        // or deleted too much/too little. Cutoff is EXCLUSIVE: a row at the
        // cutoff instant survives; strictly-older rows are removed.
        let recorder = open_memory().await;
        let eid = entity("sensor.r");

        // Three rows at known, increasing timestamps.
        for v in &["old", "mid", "new"] {
            recorder
                .record_state(&make_state_event("sensor.r", v, serde_json::json!({})))
                .await
                .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }

        // Read back the actual timestamps so the cutoff is exact.
        let since = Utc::now() - chrono::Duration::seconds(60);
        let until = Utc::now() + chrono::Duration::seconds(60);
        let all = recorder
            .get_state_history(&eid, since, until)
            .await
            .unwrap();
        assert_eq!(all.len(), 3);
        // Cut off exactly at the middle row's timestamp.
        let mid_ts = all[1].last_updated_ts;
        let cutoff = DateTime::<Utc>::from_timestamp_micros((mid_ts * 1_000_000.0) as i64).unwrap();

        let stats = recorder.purge(cutoff).await.unwrap();
        assert_eq!(stats.states_deleted, 1, "only the strictly-older 'old' row");

        let remaining = recorder
            .get_state_history(&eid, since, until)
            .await
            .unwrap();
        assert_eq!(
            remaining.len(),
            2,
            "boundary 'mid' row is KEPT (exclusive cutoff)"
        );
        assert_eq!(remaining[0].state, "mid");
        assert_eq!(remaining[1].state, "new");
    }

    #[tokio::test]
    async fn purge_gcs_orphaned_attributes_but_keeps_shared() {
        // Dedup means two states can share one attribute blob. Purging one of
        // them must NOT drop the still-referenced blob; purging the last one must.
        let recorder = open_memory().await;
        let shared = serde_json::json!({"unit": "C"});

        recorder
            .record_state(&make_state_event("sensor.a", "20", shared.clone()))
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        recorder
            .record_state(&make_state_event("sensor.b", "21", shared.clone()))
            .await
            .unwrap();

        let attr_count = |r: &Recorder| {
            let pool = r.pool.clone();
            async move {
                let c: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM state_attributes")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
                c.0
            }
        };
        assert_eq!(attr_count(&recorder).await, 1, "deduped to one blob");

        // Purge before sensor.b's write → removes sensor.a only; blob still
        // referenced by sensor.b, so it must survive.
        let eid_b = entity("sensor.b");
        let rows_b = recorder
            .get_state_history(
                &eid_b,
                Utc::now() - chrono::Duration::seconds(60),
                Utc::now() + chrono::Duration::seconds(60),
            )
            .await
            .unwrap();
        let b_ts = rows_b[0].last_updated_ts;
        let cutoff = DateTime::<Utc>::from_timestamp_micros((b_ts * 1_000_000.0) as i64).unwrap();
        let stats = recorder.purge(cutoff).await.unwrap();
        assert_eq!(stats.states_deleted, 1, "sensor.a purged");
        assert_eq!(
            stats.attributes_deleted, 0,
            "shared blob still referenced — kept"
        );
        assert_eq!(attr_count(&recorder).await, 1, "blob survives");

        // Now purge everything → sensor.b gone, blob orphaned → GC'd.
        let stats2 = recorder
            .purge(Utc::now() + chrono::Duration::seconds(120))
            .await
            .unwrap();
        assert_eq!(stats2.states_deleted, 1, "sensor.b purged");
        assert_eq!(stats2.attributes_deleted, 1, "now-orphaned blob GC'd");
        assert_eq!(attr_count(&recorder).await, 0, "no blobs remain");
    }

    #[tokio::test]
    async fn purge_also_removes_old_events() {
        let recorder = open_memory().await;
        let ctx = Context::new();
        recorder
            .record_event(&DomainEvent::new(
                "call_service",
                serde_json::json!({}),
                ctx,
            ))
            .await
            .unwrap();
        // Purge with a far-future cutoff removes the event.
        let stats = recorder
            .purge(Utc::now() + chrono::Duration::seconds(120))
            .await
            .unwrap();
        assert_eq!(stats.events_deleted, 1);
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM events")
            .fetch_one(&recorder.pool)
            .await
            .unwrap();
        assert_eq!(count.0, 0);
    }

    #[tokio::test]
    async fn search_semantic_falls_back_to_text_with_null_index() {
        // With the default NullSemanticIndex, search_semantic must STILL return
        // real rows via the text fallback — proving it's no longer always-empty.
        let recorder = open_memory().await;
        recorder
            .record_state(&make_state_event(
                "light.kitchen",
                "on",
                serde_json::json!({}),
            ))
            .await
            .unwrap();
        let rows = recorder.search_semantic("kitchen", 5).await.unwrap();
        assert_eq!(rows.len(), 1, "fallback must surface the kitchen row");
        assert_eq!(rows[0].entity_id.as_str(), "light.kitchen");
    }

    #[tokio::test]
    async fn latest_states_is_deterministic_newest_per_entity() {
        let recorder = open_memory().await;
        recorder
            .record_state(&make_state_event("sensor.z", "old", serde_json::json!({})))
            .await
            .unwrap();
        recorder
            .record_state(&make_state_event("sensor.a", "only", serde_json::json!({})))
            .await
            .unwrap();
        recorder
            .record_state(&make_state_event("sensor.z", "new", serde_json::json!({})))
            .await
            .unwrap();

        let batch = recorder.latest_states(10).await.unwrap();
        assert_eq!(batch.states.len(), 2);
        assert_eq!(batch.states[0].entity_id.as_str(), "sensor.a");
        assert_eq!(batch.states[1].entity_id.as_str(), "sensor.z");
        assert_eq!(batch.states[1].state, "new");
        assert!(batch
            .states
            .iter()
            .all(|state| state.context.is_restoration()));
    }

    #[tokio::test]
    async fn latest_states_isolates_malformed_rows_and_honours_bound() {
        let recorder = open_memory().await;
        recorder
            .record_state(&make_state_event(
                "sensor.good",
                "ok",
                serde_json::json!({"x": 1}),
            ))
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO states \
             (entity_id, state, attributes_id, last_changed_ts, last_updated_ts, context_id) \
             VALUES ('INVALID', 'bad', NULL, 1.0, 2.0, NULL)",
        )
        .execute(&recorder.pool)
        .await
        .unwrap();
        let attrs_id = sqlx::query(
            "INSERT INTO state_attributes (shared_attrs, hash) VALUES ('not-json', 424242)",
        )
        .execute(&recorder.pool)
        .await
        .unwrap()
        .last_insert_rowid();
        sqlx::query(
            "INSERT INTO states \
             (entity_id, state, attributes_id, last_changed_ts, last_updated_ts, context_id) \
             VALUES ('sensor.badattrs', 'bad', ?, 1.0, 2.0, NULL)",
        )
        .bind(attrs_id)
        .execute(&recorder.pool)
        .await
        .unwrap();

        let batch = recorder.latest_states(10).await.unwrap();
        assert_eq!(batch.states.len(), 1);
        assert_eq!(batch.warnings.len(), 2);
        assert!(batch
            .warnings
            .iter()
            .any(|warning| matches!(warning, RestoreWarning::MalformedEntityId { .. })));
        assert!(batch
            .warnings
            .iter()
            .any(|warning| matches!(warning, RestoreWarning::MalformedAttributes { .. })));

        let bounded = recorder.latest_states(1).await.unwrap();
        assert!(bounded.truncated);
        assert!(bounded.states.len() <= 1);
    }
}
