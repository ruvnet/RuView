//! Two-tier cached storage backend implementation.
//!
//! This module provides a caching layer on top of any storage backend:
//! - Fast access to frequently used data
//! - Write-through caching for consistency
//! - Configurable cache duration
//! - Support for any StorageBackend as cache or store
//!
//! # Configuration
//!
//! The cached backend can be configured using the following options:
//!
//! ```toml
//! # Primary storage configuration
//! [engine]
//! engine = "adbc"
//! connection = "postgresql://localhost:5432"
//! options = {
//!     driver_path = "/usr/local/lib/libadbc_driver_postgresql.so",
//!     username = "postgres",
//!     database = "metrics"
//! }
//!
//! # Cache configuration
//! [cache]
//! enabled = true
//! engine = "duckdb"
//! connection = ":memory:"
//! max_duration_secs = 3600
//! options = {
//!     threads = "2"
//! }
//! ```
//!
//! Or via command line:
//!
//! ```bash
//! hyprstream \
//!   --engine adbc \
//!   --engine-connection "postgresql://localhost:5432" \
//!   --engine-options driver_path=/usr/local/lib/libadbc_driver_postgresql.so \
//!   --engine-options username=postgres \
//!   --enable-cache \
//!   --cache-engine duckdb \
//!   --cache-connection ":memory:" \
//!   --cache-options threads=2 \
//!   --cache-max-duration 3600
//! ```
//!
//! The implementation follows standard caching patterns while ensuring
//! data consistency between cache and backing store.

use crate::config::Credentials;
use crate::metrics::MetricRecord;
use crate::storage::{StorageBackend, adbc::AdbcBackend, duckdb::DuckDbBackend};
use std::sync::Arc;
use std::collections::HashMap;
use tonic::Status;

/// Two-tier storage backend with caching support.
///
/// This backend provides:
/// - Fast access to recent data through caching
/// - Write-through caching for data consistency
/// - Configurable cache duration
/// - Support for any StorageBackend implementation
///
/// The implementation uses two storage backends:
/// 1. A fast cache (e.g., in-memory DuckDB)
/// 2. A persistent store (e.g., PostgreSQL via ADBC)
pub struct CachedStorageBackend {
    /// Fast storage backend for caching
    cache: Arc<dyn StorageBackend>,
    /// Persistent storage backend for data
    store: Arc<dyn StorageBackend>,
    /// Maximum cache entry lifetime in seconds
    max_duration_secs: u64,
}

impl CachedStorageBackend {
    /// Creates a new cached storage backend.
    ///
    /// This method sets up a two-tier storage system with:
    /// - A fast cache layer for frequent access
    /// - A persistent backing store
    /// - Configurable cache duration
    ///
    /// # Arguments
    ///
    /// * `cache` - Fast storage backend for caching
    /// * `store` - Persistent storage backend
    /// * `max_duration_secs` - Maximum cache entry lifetime in seconds
    pub fn new(
        cache: Arc<dyn StorageBackend>,
        store: Arc<dyn StorageBackend>,
        max_duration_secs: u64,
    ) -> Self {
        Self {
            cache,
            store,
            max_duration_secs,
        }
    }
}

#[async_trait::async_trait]
impl StorageBackend for CachedStorageBackend {
    /// Initializes both cache and backing store.
    ///
    /// This method ensures both storage layers are properly
    /// initialized and ready for use.
    async fn init(&self) -> Result<(), Status> {
        // Initialize both cache and backing store
        self.cache.init().await?;
        self.store.init().await?;
        Ok(())
    }

    /// Inserts metrics into both cache and backing store.
    ///
    /// This method implements write-through caching:
    /// 1. Writes to cache for fast access
    /// 2. Writes to backing store for persistence
    ///
    /// # Arguments
    ///
    /// * `metrics` - Vector of MetricRecord instances to insert
    async fn insert_metrics(&self, metrics: Vec<MetricRecord>) -> Result<(), Status> {
        // Insert into both cache and backing store
        self.cache.insert_metrics(metrics.clone()).await?;
        self.store.insert_metrics(metrics).await?;
        Ok(())
    }

    /// Queries metrics with caching support.
    ///
    /// This method implements a cache-first query strategy:
    /// 1. Attempts to read from cache
    /// 2. On cache miss, reads from backing store
    /// 3. Updates cache with results from backing store
    ///
    /// # Arguments
    ///
    /// * `from_timestamp` - Unix timestamp to query from
    async fn query_metrics(&self, from_timestamp: i64) -> Result<Vec<MetricRecord>, Status> {
        // Calculate cache cutoff time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let cache_cutoff = now - self.max_duration_secs as i64;

        // Only use cache for data within cache window
        if from_timestamp >= cache_cutoff {
            match self.cache.query_metrics(from_timestamp).await {
                Ok(metrics) if !metrics.is_empty() => return Ok(metrics),
                _ => {}
            }
        }

        // Cache miss or data too old, query backing store
        let metrics = self.store.query_metrics(from_timestamp).await?;

        // Update cache with results if within cache window
        if !metrics.is_empty() && from_timestamp >= cache_cutoff {
            self.cache.insert_metrics(metrics.clone()).await?;
        }

        Ok(metrics)
    }

    /// Prepares a SQL statement on the backing store.
    ///
    /// This method bypasses the cache and prepares statements
    /// directly on the backing store, as prepared statements
    /// are typically used for complex queries.
    ///
    /// # Arguments
    ///
    /// * `query` - SQL query to prepare
    async fn prepare_sql(&self, query: &str) -> Result<Vec<u8>, Status> {
        // Prepare on backing store only
        self.store.prepare_sql(query).await
    }

    /// Executes a prepared SQL statement on the backing store.
    ///
    /// This method bypasses the cache and executes statements
    /// directly on the backing store, ensuring consistent results
    /// for complex queries.
    ///
    /// # Arguments
    ///
    /// * `statement_handle` - Handle of the prepared statement
    async fn query_sql(&self, statement_handle: &[u8]) -> Result<Vec<MetricRecord>, Status> {
        // Execute on backing store only
        self.store.query_sql(statement_handle).await
    }

    fn new_with_options(
        connection_string: &str,
        options: &HashMap<String, String>,
        credentials: Option<&Credentials>,
    ) -> Result<Self, Status> {
        // Parse cache duration from options
        let max_duration_secs = options
            .get("max_duration_secs")
            .and_then(|s| s.parse().ok())
            .unwrap_or(3600);

        // Create cache backend
        let default_engine = "duckdb".to_string();
        let default_connection = ":memory:".to_string();
        let cache_engine = options.get("cache_engine").unwrap_or(&default_engine);
        let cache_connection = options.get("cache_connection").unwrap_or(&default_connection);
        let cache_options: HashMap<String, String> = options
            .iter()
            .filter(|(k, _)| k.starts_with("cache_"))
            .map(|(k, v)| (k[6..].to_string(), v.clone()))
            .collect();

        let cache: Arc<dyn StorageBackend> = match cache_engine.as_str() {
            "duckdb" => Arc::new(DuckDbBackend::new_with_options(
                cache_connection,
                &cache_options,
                None,
            )?),
            "adbc" => Arc::new(AdbcBackend::new_with_options(
                cache_connection,
                &cache_options,
                None,
            )?),
            _ => return Err(Status::invalid_argument("Invalid cache engine type")),
        };

        // Create store backend
        let store = Arc::new(AdbcBackend::new_with_options(
            connection_string,
            options,
            credentials,
        )?);

        Ok(Self::new(cache, store, max_duration_secs))
    }
}
