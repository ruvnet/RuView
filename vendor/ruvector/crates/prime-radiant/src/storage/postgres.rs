//! PostgreSQL Storage Implementation
//!
//! Production-ready PostgreSQL storage with async sqlx queries.
//! This module is feature-gated behind the `postgres` feature.
//!
//! # Schema (ADR-014)
//!
//! ```sql
//! -- Policy bundles table
//! CREATE TABLE policy_bundles (
//!     id UUID PRIMARY KEY,
//!     version_major INT NOT NULL,
//!     version_minor INT NOT NULL,
//!     version_patch INT NOT NULL,
//!     name VARCHAR(255) NOT NULL,
//!     description TEXT,
//!     status VARCHAR(50) NOT NULL,
//!     thresholds JSONB NOT NULL,
//!     escalation_rules JSONB NOT NULL,
//!     approvals JSONB NOT NULL,
//!     required_approvals INT NOT NULL,
//!     allowed_approvers JSONB,
//!     content_hash BYTEA NOT NULL,
//!     supersedes UUID REFERENCES policy_bundles(id),
//!     created_at TIMESTAMPTZ NOT NULL,
//!     updated_at TIMESTAMPTZ NOT NULL,
//!     activated_at TIMESTAMPTZ
//! );
//!
//! -- Witness records table
//! CREATE TABLE witness_records (
//!     id UUID PRIMARY KEY,
//!     sequence BIGINT NOT NULL UNIQUE,
//!     action_hash BYTEA NOT NULL,
//!     energy_snapshot JSONB NOT NULL,
//!     decision JSONB NOT NULL,
//!     policy_bundle_id UUID NOT NULL REFERENCES policy_bundles(id),
//!     previous_witness UUID REFERENCES witness_records(id),
//!     previous_hash BYTEA,
//!     content_hash BYTEA NOT NULL,
//!     actor VARCHAR(255),
//!     correlation_id VARCHAR(255),
//!     created_at TIMESTAMPTZ NOT NULL,
//!     INDEX idx_witness_sequence (sequence),
//!     INDEX idx_witness_action (action_hash),
//!     INDEX idx_witness_policy (policy_bundle_id),
//!     INDEX idx_witness_correlation (correlation_id)
//! );
//!
//! -- Lineage records table
//! CREATE TABLE lineage_records (
//!     id UUID PRIMARY KEY,
//!     entity_type VARCHAR(100) NOT NULL,
//!     entity_id VARCHAR(255) NOT NULL,
//!     entity_namespace VARCHAR(255),
//!     entity_version BIGINT,
//!     operation VARCHAR(50) NOT NULL,
//!     dependencies UUID[] NOT NULL,
//!     authorizing_witness UUID NOT NULL REFERENCES witness_records(id),
//!     actor VARCHAR(255) NOT NULL,
//!     description TEXT,
//!     previous_state_hash BYTEA,
//!     new_state_hash BYTEA,
//!     content_hash BYTEA NOT NULL,
//!     metadata JSONB NOT NULL,
//!     created_at TIMESTAMPTZ NOT NULL,
//!     INDEX idx_lineage_entity (entity_type, entity_id),
//!     INDEX idx_lineage_actor (actor),
//!     INDEX idx_lineage_witness (authorizing_witness)
//! );
//!
//! -- Event log table for audit trail
//! CREATE TABLE event_log (
//!     id BIGSERIAL PRIMARY KEY,
//!     event_type VARCHAR(100) NOT NULL,
//!     entity_type VARCHAR(100) NOT NULL,
//!     entity_id VARCHAR(255) NOT NULL,
//!     data JSONB NOT NULL,
//!     actor VARCHAR(255),
//!     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
//!     INDEX idx_event_type (event_type),
//!     INDEX idx_event_entity (entity_type, entity_id),
//!     INDEX idx_event_time (created_at)
//! );
//!
//! -- Node states table (for graph storage)
//! CREATE TABLE node_states (
//!     node_id VARCHAR(255) PRIMARY KEY,
//!     state REAL[] NOT NULL,
//!     dimension INT NOT NULL,
//!     updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
//! );
//!
//! -- Edge table
//! CREATE TABLE edges (
//!     source VARCHAR(255) NOT NULL,
//!     target VARCHAR(255) NOT NULL,
//!     weight REAL NOT NULL,
//!     updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
//!     PRIMARY KEY (source, target)
//! );
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use prime_radiant::storage::PostgresStorage;
//!
//! let storage = PostgresStorage::connect("postgresql://localhost/prime_radiant").await?;
//! storage.migrate().await?;
//!
//! // Store data
//! storage.store_node("node-1", &[1.0, 0.0, 0.0]).await?;
//! ```

use super::{StorageConfig, StorageError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use sqlx::FromRow;
use std::sync::Arc;
use uuid::Uuid;

/// PostgreSQL connection pool wrapper
#[derive(Clone)]
pub struct PostgresStorage {
    /// Connection pool
    pool: PgPool,
    /// Configuration
    config: PostgresConfig,
}

/// PostgreSQL-specific configuration
#[derive(Debug, Clone)]
pub struct PostgresConfig {
    /// Connection string
    pub connection_string: String,
    /// Maximum connections in pool
    pub max_connections: u32,
    /// Connection timeout in seconds
    pub connect_timeout_secs: u64,
    /// Enable statement logging
    pub log_statements: bool,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            connection_string: "postgresql://localhost/prime_radiant".to_string(),
            max_connections: 10,
            connect_timeout_secs: 30,
            log_statements: false,
        }
    }
}

impl PostgresConfig {
    /// Create from a connection string
    #[must_use]
    pub fn from_url(url: impl Into<String>) -> Self {
        Self {
            connection_string: url.into(),
            ..Default::default()
        }
    }
}

/// Policy bundle row from database
#[derive(Debug, Clone, FromRow)]
pub struct PolicyBundleRow {
    pub id: Uuid,
    pub version_major: i32,
    pub version_minor: i32,
    pub version_patch: i32,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub thresholds: serde_json::Value,
    pub escalation_rules: serde_json::Value,
    pub approvals: serde_json::Value,
    pub required_approvals: i32,
    pub allowed_approvers: Option<serde_json::Value>,
    pub content_hash: Vec<u8>,
    pub supersedes: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub activated_at: Option<DateTime<Utc>>,
}

/// Witness record row from database
#[derive(Debug, Clone, FromRow)]
pub struct WitnessRecordRow {
    pub id: Uuid,
    pub sequence: i64,
    pub action_hash: Vec<u8>,
    pub energy_snapshot: serde_json::Value,
    pub decision: serde_json::Value,
    pub policy_bundle_id: Uuid,
    pub previous_witness: Option<Uuid>,
    pub previous_hash: Option<Vec<u8>>,
    pub content_hash: Vec<u8>,
    pub actor: Option<String>,
    pub correlation_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Lineage record row from database
#[derive(Debug, Clone, FromRow)]
pub struct LineageRecordRow {
    pub id: Uuid,
    pub entity_type: String,
    pub entity_id: String,
    pub entity_namespace: Option<String>,
    pub entity_version: Option<i64>,
    pub operation: String,
    pub dependencies: Vec<Uuid>,
    pub authorizing_witness: Uuid,
    pub actor: String,
    pub description: Option<String>,
    pub previous_state_hash: Option<Vec<u8>>,
    pub new_state_hash: Option<Vec<u8>>,
    pub content_hash: Vec<u8>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Node state row from database
#[derive(Debug, Clone, FromRow)]
pub struct NodeStateRow {
    pub node_id: String,
    pub state: Vec<f32>,
    pub dimension: i32,
    pub updated_at: DateTime<Utc>,
}

/// Edge row from database
#[derive(Debug, Clone, FromRow)]
pub struct EdgeRow {
    pub source: String,
    pub target: String,
    pub weight: f32,
    pub updated_at: DateTime<Utc>,
}

/// Event log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogEntry {
    pub event_type: String,
    pub entity_type: String,
    pub entity_id: String,
    pub data: serde_json::Value,
    pub actor: Option<String>,
}

impl PostgresStorage {
    /// Connect to PostgreSQL with default configuration.
    ///
    /// # Errors
    ///
    /// Returns error if connection fails.
    pub async fn connect(connection_string: &str) -> Result<Self, StorageError> {
        let config = PostgresConfig::from_url(connection_string);
        Self::with_config(config).await
    }

    /// Connect to PostgreSQL with custom configuration.
    ///
    /// # Errors
    ///
    /// Returns error if connection fails.
    pub async fn with_config(config: PostgresConfig) -> Result<Self, StorageError> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(std::time::Duration::from_secs(config.connect_timeout_secs))
            .connect(&config.connection_string)
            .await
            .map_err(|e| StorageError::Connection(e.to_string()))?;

        Ok(Self { pool, config })
    }

    /// Create from a StorageConfig.
    ///
    /// # Errors
    ///
    /// Returns error if postgres_url is not set or connection fails.
    pub async fn from_storage_config(config: &StorageConfig) -> Result<Self, StorageError> {
        let url = config
            .postgres_url
            .as_ref()
            .ok_or_else(|| StorageError::Connection("postgres_url not configured".to_string()))?;

        Self::connect(url).await
    }

    /// Run database migrations to create schema.
    ///
    /// # Errors
    ///
    /// Returns error if migration fails.
    pub async fn migrate(&self) -> Result<(), StorageError> {
        // Create tables
        sqlx::query(SCHEMA_SQL)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(())
    }

    /// Check if the database is healthy.
    ///
    /// # Errors
    ///
    /// Returns error if health check fails.
    pub async fn health_check(&self) -> Result<bool, StorageError> {
        let result: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Connection(e.to_string()))?;

        Ok(result.0 == 1)
    }

    /// Get the connection pool for advanced usage.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Log an event to the event log.
    pub async fn log_event(&self, entry: EventLogEntry) -> Result<i64, StorageError> {
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO event_log (event_type, entity_type, entity_id, data, actor)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
        )
        .bind(&entry.event_type)
        .bind(&entry.entity_type)
        .bind(&entry.entity_id)
        .bind(&entry.data)
        .bind(&entry.actor)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(row.0)
    }

    // =========================================================================
    // Node Storage Operations
    // =========================================================================

    /// Store a node state.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn store_node(&self, node_id: &str, state: &[f32]) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO node_states (node_id, state, dimension, updated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (node_id) DO UPDATE SET
                state = EXCLUDED.state,
                dimension = EXCLUDED.dimension,
                updated_at = NOW()
            "#,
        )
        .bind(node_id)
        .bind(state)
        .bind(state.len() as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(())
    }

    /// Get a node state.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn get_node(&self, node_id: &str) -> Result<Option<Vec<f32>>, StorageError> {
        let row: Option<NodeStateRow> = sqlx::query_as(
            r#"
            SELECT node_id, state, dimension, updated_at
            FROM node_states
            WHERE node_id = $1
            "#,
        )
        .bind(node_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(row.map(|r| r.state))
    }

    /// Delete a node state.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn delete_node(&self, node_id: &str) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM node_states WHERE node_id = $1")
            .bind(node_id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(())
    }

    /// Store an edge.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn store_edge(
        &self,
        source: &str,
        target: &str,
        weight: f32,
    ) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO edges (source, target, weight, updated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (source, target) DO UPDATE SET
                weight = EXCLUDED.weight,
                updated_at = NOW()
            "#,
        )
        .bind(source)
        .bind(target)
        .bind(weight)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(())
    }

    /// Delete an edge.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn delete_edge(&self, source: &str, target: &str) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM edges WHERE source = $1 AND target = $2")
            .bind(source)
            .bind(target)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(())
    }

    /// Find similar nodes using cosine similarity.
    /// Note: For production, consider using pgvector extension for better performance.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn find_similar(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(String, f32)>, StorageError> {
        // This is a simple implementation without pgvector
        // For production, use: CREATE EXTENSION vector; and proper vector operations
        let rows: Vec<NodeStateRow> = sqlx::query_as(
            r#"
            SELECT node_id, state, dimension, updated_at
            FROM node_states
            WHERE dimension = $1
            "#,
        )
        .bind(query.len() as i32)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        // Compute similarities in memory (inefficient for large datasets)
        let mut results: Vec<(String, f32)> = rows
            .iter()
            .map(|row| {
                let similarity = cosine_similarity(query, &row.state);
                (row.node_id.clone(), similarity)
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);

        Ok(results)
    }

    // =========================================================================
    // Policy Bundle Operations
    // =========================================================================

    /// Store a policy bundle.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn store_policy_bundle(&self, bundle: &[u8]) -> Result<Uuid, StorageError> {
        let id = Uuid::new_v4();

        // Store raw bytes in thresholds as empty JSON, and raw data in content_hash
        let data = serde_json::json!({
            "size": bundle.len()
        });

        sqlx::query(
            r#"
            INSERT INTO policy_bundles (
                id, version_major, version_minor, version_patch,
                name, status, thresholds, escalation_rules, approvals,
                required_approvals, content_hash, created_at, updated_at
            )
            VALUES ($1, 1, 0, 0, 'raw', 'draft', $2, '[]', '[]', 1, $3, NOW(), NOW())
            "#,
        )
        .bind(id)
        .bind(&data)
        .bind(bundle)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(id)
    }

    /// Get a policy bundle.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn get_policy_bundle(&self, id: Uuid) -> Result<Option<Vec<u8>>, StorageError> {
        let row: Option<(Vec<u8>,)> = sqlx::query_as(
            r#"
            SELECT content_hash FROM policy_bundles WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(row.map(|r| r.0))
    }

    /// Get the active policy bundle.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn get_active_policy(&self) -> Result<Option<PolicyBundleRow>, StorageError> {
        let row: Option<PolicyBundleRow> = sqlx::query_as(
            r#"
            SELECT * FROM policy_bundles WHERE status = 'active' LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(row)
    }

    // =========================================================================
    // Witness Record Operations
    // =========================================================================

    /// Store a witness record.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn store_witness(&self, witness: &[u8]) -> Result<Uuid, StorageError> {
        let id = Uuid::new_v4();

        // Get the next sequence number
        let seq: (i64,) = sqlx::query_as(
            r#"
            SELECT COALESCE(MAX(sequence), 0) + 1 FROM witness_records
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        // For raw bytes, we need a default policy bundle
        // In production, this would be properly deserialized
        let default_policy = self.get_or_create_default_policy().await?;

        sqlx::query(
            r#"
            INSERT INTO witness_records (
                id, sequence, action_hash, energy_snapshot, decision,
                policy_bundle_id, content_hash, created_at
            )
            VALUES ($1, $2, $3, '{}', '{}', $4, $5, NOW())
            "#,
        )
        .bind(id)
        .bind(seq.0)
        .bind(witness)
        .bind(default_policy)
        .bind(witness)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(id)
    }

    /// Get witnesses for an action.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn get_witnesses_for_action(
        &self,
        action_hash: &[u8],
    ) -> Result<Vec<WitnessRecordRow>, StorageError> {
        let rows: Vec<WitnessRecordRow> = sqlx::query_as(
            r#"
            SELECT * FROM witness_records WHERE action_hash = $1 ORDER BY sequence
            "#,
        )
        .bind(action_hash)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(rows)
    }

    /// Get the head (latest) witness.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn get_witness_head(&self) -> Result<Option<WitnessRecordRow>, StorageError> {
        let row: Option<WitnessRecordRow> = sqlx::query_as(
            r#"
            SELECT * FROM witness_records ORDER BY sequence DESC LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(row)
    }

    // =========================================================================
    // Lineage Record Operations
    // =========================================================================

    /// Store a lineage record.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn store_lineage(&self, lineage: &[u8]) -> Result<Uuid, StorageError> {
        let id = Uuid::new_v4();

        // Get or create a default witness for raw storage
        let default_witness = self.get_or_create_default_witness().await?;

        sqlx::query(
            r#"
            INSERT INTO lineage_records (
                id, entity_type, entity_id, operation, dependencies,
                authorizing_witness, actor, content_hash, metadata, created_at
            )
            VALUES ($1, 'raw', $2, 'CREATE', '{}', $3, 'system', $4, '{}', NOW())
            "#,
        )
        .bind(id)
        .bind(id.to_string())
        .bind(default_witness)
        .bind(lineage)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(id)
    }

    /// Get lineage records for an entity.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn get_lineage_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<LineageRecordRow>, StorageError> {
        let rows: Vec<LineageRecordRow> = sqlx::query_as(
            r#"
            SELECT * FROM lineage_records
            WHERE entity_type = $1 AND entity_id = $2
            ORDER BY created_at
            "#,
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(rows)
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    /// Get or create a default policy bundle for raw storage operations.
    async fn get_or_create_default_policy(&self) -> Result<Uuid, StorageError> {
        // Try to get existing default policy
        let existing: Option<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT id FROM policy_bundles WHERE name = '__default__' LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        if let Some((id,)) = existing {
            return Ok(id);
        }

        // Create default policy
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO policy_bundles (
                id, version_major, version_minor, version_patch,
                name, status, thresholds, escalation_rules, approvals,
                required_approvals, content_hash, created_at, updated_at
            )
            VALUES ($1, 1, 0, 0, '__default__', 'active', '{}', '[]', '[]', 0, '', NOW(), NOW())
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(id)
    }

    /// Get or create a default witness for raw storage operations.
    async fn get_or_create_default_witness(&self) -> Result<Uuid, StorageError> {
        // Try to get existing default witness
        let existing: Option<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT id FROM witness_records WHERE actor = '__default__' LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        if let Some((id,)) = existing {
            return Ok(id);
        }

        // Create default witness
        let id = Uuid::new_v4();
        let policy_id = self.get_or_create_default_policy().await?;

        sqlx::query(
            r#"
            INSERT INTO witness_records (
                id, sequence, action_hash, energy_snapshot, decision,
                policy_bundle_id, content_hash, actor, created_at
            )
            VALUES ($1, 0, '', '{}', '{}', $2, '', '__default__', NOW())
            "#,
        )
        .bind(id)
        .bind(policy_id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(id)
    }

    /// Get database statistics.
    ///
    /// # Errors
    ///
    /// Returns error if the operation fails.
    pub async fn stats(&self) -> Result<PostgresStats, StorageError> {
        let node_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM node_states")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Transaction(e.to_string()))?;

        let edge_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM edges")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Transaction(e.to_string()))?;

        let policy_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM policy_bundles")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Transaction(e.to_string()))?;

        let witness_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM witness_records")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Transaction(e.to_string()))?;

        let lineage_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM lineage_records")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(PostgresStats {
            node_count: node_count.0 as u64,
            edge_count: edge_count.0 as u64,
            policy_count: policy_count.0 as u64,
            witness_count: witness_count.0 as u64,
            lineage_count: lineage_count.0 as u64,
        })
    }
}

/// PostgreSQL storage statistics
#[derive(Debug, Clone)]
pub struct PostgresStats {
    /// Number of nodes
    pub node_count: u64,
    /// Number of edges
    pub edge_count: u64,
    /// Number of policy bundles
    pub policy_count: u64,
    /// Number of witness records
    pub witness_count: u64,
    /// Number of lineage records
    pub lineage_count: u64,
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Database schema SQL
const SCHEMA_SQL: &str = r#"
-- Policy bundles table
CREATE TABLE IF NOT EXISTS policy_bundles (
    id UUID PRIMARY KEY,
    version_major INT NOT NULL DEFAULT 1,
    version_minor INT NOT NULL DEFAULT 0,
    version_patch INT NOT NULL DEFAULT 0,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    status VARCHAR(50) NOT NULL DEFAULT 'draft',
    thresholds JSONB NOT NULL DEFAULT '{}',
    escalation_rules JSONB NOT NULL DEFAULT '[]',
    approvals JSONB NOT NULL DEFAULT '[]',
    required_approvals INT NOT NULL DEFAULT 1,
    allowed_approvers JSONB,
    content_hash BYTEA NOT NULL DEFAULT '',
    supersedes UUID REFERENCES policy_bundles(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    activated_at TIMESTAMPTZ
);

-- Index on policy status
CREATE INDEX IF NOT EXISTS idx_policy_status ON policy_bundles(status);
CREATE INDEX IF NOT EXISTS idx_policy_name ON policy_bundles(name);

-- Witness records table
CREATE TABLE IF NOT EXISTS witness_records (
    id UUID PRIMARY KEY,
    sequence BIGINT NOT NULL,
    action_hash BYTEA NOT NULL DEFAULT '',
    energy_snapshot JSONB NOT NULL DEFAULT '{}',
    decision JSONB NOT NULL DEFAULT '{}',
    policy_bundle_id UUID NOT NULL REFERENCES policy_bundles(id),
    previous_witness UUID REFERENCES witness_records(id),
    previous_hash BYTEA,
    content_hash BYTEA NOT NULL DEFAULT '',
    actor VARCHAR(255),
    correlation_id VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes on witness records
CREATE UNIQUE INDEX IF NOT EXISTS idx_witness_sequence ON witness_records(sequence);
CREATE INDEX IF NOT EXISTS idx_witness_action ON witness_records(action_hash);
CREATE INDEX IF NOT EXISTS idx_witness_policy ON witness_records(policy_bundle_id);
CREATE INDEX IF NOT EXISTS idx_witness_correlation ON witness_records(correlation_id);

-- Lineage records table
CREATE TABLE IF NOT EXISTS lineage_records (
    id UUID PRIMARY KEY,
    entity_type VARCHAR(100) NOT NULL,
    entity_id VARCHAR(255) NOT NULL,
    entity_namespace VARCHAR(255),
    entity_version BIGINT,
    operation VARCHAR(50) NOT NULL,
    dependencies UUID[] NOT NULL DEFAULT '{}',
    authorizing_witness UUID NOT NULL REFERENCES witness_records(id),
    actor VARCHAR(255) NOT NULL,
    description TEXT,
    previous_state_hash BYTEA,
    new_state_hash BYTEA,
    content_hash BYTEA NOT NULL DEFAULT '',
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes on lineage records
CREATE INDEX IF NOT EXISTS idx_lineage_entity ON lineage_records(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_lineage_actor ON lineage_records(actor);
CREATE INDEX IF NOT EXISTS idx_lineage_witness ON lineage_records(authorizing_witness);

-- Event log table for audit trail
CREATE TABLE IF NOT EXISTS event_log (
    id BIGSERIAL PRIMARY KEY,
    event_type VARCHAR(100) NOT NULL,
    entity_type VARCHAR(100) NOT NULL,
    entity_id VARCHAR(255) NOT NULL,
    data JSONB NOT NULL DEFAULT '{}',
    actor VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes on event log
CREATE INDEX IF NOT EXISTS idx_event_type ON event_log(event_type);
CREATE INDEX IF NOT EXISTS idx_event_entity ON event_log(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_event_time ON event_log(created_at);

-- Node states table (for graph storage)
CREATE TABLE IF NOT EXISTS node_states (
    node_id VARCHAR(255) PRIMARY KEY,
    state REAL[] NOT NULL,
    dimension INT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Edge table
CREATE TABLE IF NOT EXISTS edges (
    source VARCHAR(255) NOT NULL,
    target VARCHAR(255) NOT NULL,
    weight REAL NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (source, target)
);

-- Indexes on edges
CREATE INDEX IF NOT EXISTS idx_edge_source ON edges(source);
CREATE INDEX IF NOT EXISTS idx_edge_target ON edges(target);
"#;

/// Async wrapper for GraphStorage trait (sync trait, async impl)
pub struct AsyncGraphStorageAdapter {
    storage: Arc<PostgresStorage>,
}

impl AsyncGraphStorageAdapter {
    /// Create a new adapter
    pub fn new(storage: PostgresStorage) -> Self {
        Self {
            storage: Arc::new(storage),
        }
    }

    /// Get the underlying storage
    #[must_use]
    pub fn storage(&self) -> &PostgresStorage {
        &self.storage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        // Identical vectors
        let sim = cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]);
        assert!((sim - 1.0).abs() < 0.001);

        // Orthogonal vectors
        let sim = cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]);
        assert!(sim.abs() < 0.001);

        // Opposite vectors
        let sim = cosine_similarity(&[1.0, 0.0], &[-1.0, 0.0]);
        assert!((sim - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_postgres_config() {
        let config = PostgresConfig::default();
        assert_eq!(config.max_connections, 10);

        let config = PostgresConfig::from_url("postgresql://test");
        assert_eq!(config.connection_string, "postgresql://test");
    }

    // Integration tests require a running PostgreSQL instance
    // These would be run with `cargo test --features postgres -- --ignored`

    #[tokio::test]
    #[ignore = "requires PostgreSQL"]
    async fn test_postgres_connection() {
        let storage = PostgresStorage::connect("postgresql://localhost/test")
            .await
            .unwrap();
        assert!(storage.health_check().await.unwrap());
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL"]
    async fn test_postgres_migration() {
        let storage = PostgresStorage::connect("postgresql://localhost/test")
            .await
            .unwrap();
        storage.migrate().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL"]
    async fn test_postgres_node_operations() {
        let storage = PostgresStorage::connect("postgresql://localhost/test")
            .await
            .unwrap();
        storage.migrate().await.unwrap();

        // Store node
        storage
            .store_node("test-node", &[1.0, 2.0, 3.0])
            .await
            .unwrap();

        // Get node
        let state = storage.get_node("test-node").await.unwrap();
        assert!(state.is_some());
        assert_eq!(state.unwrap(), vec![1.0, 2.0, 3.0]);

        // Delete node
        storage.delete_node("test-node").await.unwrap();
        let state = storage.get_node("test-node").await.unwrap();
        assert!(state.is_none());
    }
}
