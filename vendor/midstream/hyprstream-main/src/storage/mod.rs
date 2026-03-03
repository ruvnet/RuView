//! Storage backends for metric data persistence and caching.
//!
//! This module provides multiple storage backend implementations:
//! - `duckdb`: High-performance embedded database for caching and local storage
//! - `adbc`: Arrow Database Connectivity for external database integration
//! - `cached`: Two-tier storage with configurable caching layer
//!
//! Each backend implements the `StorageBackend` trait, providing a consistent
//! interface for metric storage and retrieval operations.

pub mod adbc;
pub mod duckdb;
pub mod cache;
pub mod table_manager;

use crate::config::Credentials;
use crate::metrics::MetricRecord;
use crate::aggregation::{AggregateFunction, GroupBy, AggregateResult, TimeWindow};
use crate::storage::table_manager::{TableManager, AggregationView};
use async_trait::async_trait;
use std::collections::HashMap;
use tonic::Status;
use arrow_schema::Schema;
use arrow_array::RecordBatch;

/// Batch-level aggregation state for efficient updates
#[derive(Debug, Clone)]
pub struct BatchAggregation {
    /// The metric ID this aggregation belongs to
    pub metric_id: String,
    /// Start of the time window
    pub window_start: i64,
    /// End of the time window
    pub window_end: i64,
    /// Running sum within the window
    pub running_sum: f64,
    /// Running count within the window
    pub running_count: i64,
    /// Minimum value in the window
    pub min_value: f64,
    /// Maximum value in the window
    pub max_value: f64,
}

/// Storage backend trait for metric data persistence.
///
/// This trait defines the interface that all storage backends must implement.
/// It provides methods for:
/// - Initialization and configuration
/// - Metric data insertion
/// - Metric data querying
/// - SQL query preparation and execution
/// - Aggregation of metrics
/// - Table and view management
#[async_trait]
pub trait StorageBackend: Send + Sync + 'static {
    /// Initialize the storage backend.
    async fn init(&self) -> Result<(), Status>;

    /// Insert metrics into storage.
    async fn insert_metrics(&self, metrics: Vec<MetricRecord>) -> Result<(), Status>;

    /// Query metrics from storage.
    async fn query_metrics(&self, from_timestamp: i64) -> Result<Vec<MetricRecord>, Status>;

    /// Prepare a SQL query and return a handle.
    /// The handle is backend-specific and opaque to the caller.
    async fn prepare_sql(&self, query: &str) -> Result<Vec<u8>, Status>;

    /// Execute a prepared SQL query using its handle.
    /// The handle must have been obtained from prepare_sql.
    async fn query_sql(&self, statement_handle: &[u8]) -> Result<Vec<MetricRecord>, Status>;

    /// Aggregate metrics using the specified function and grouping.
    async fn aggregate_metrics(
        &self,
        function: AggregateFunction,
        group_by: &GroupBy,
        from_timestamp: i64,
        to_timestamp: Option<i64>,
    ) -> Result<Vec<AggregateResult>, Status>;

    /// Create a new instance with the given options.
    /// The connection string and options are backend-specific.
    fn new_with_options(
        connection_string: &str,
        options: &HashMap<String, String>,
        credentials: Option<&Credentials>,
    ) -> Result<Self, Status>
    where
        Self: Sized;

    /// Create a new table with the given schema
    async fn create_table(&self, table_name: &str, schema: &Schema) -> Result<(), Status>;

    /// Insert data into a table
    async fn insert_into_table(&self, table_name: &str, batch: RecordBatch) -> Result<(), Status>;

    /// Query data from a table
    async fn query_table(&self, table_name: &str, projection: Option<Vec<String>>) -> Result<RecordBatch, Status>;

    /// Create an aggregation view
    async fn create_aggregation_view(&self, view: &AggregationView) -> Result<(), Status>;

    /// Query data from an aggregation view
    async fn query_aggregation_view(&self, view_name: &str) -> Result<RecordBatch, Status>;

    /// Drop a table
    async fn drop_table(&self, table_name: &str) -> Result<(), Status>;

    /// Drop an aggregation view
    async fn drop_aggregation_view(&self, view_name: &str) -> Result<(), Status>;

    /// Get the table manager instance
    fn table_manager(&self) -> &TableManager;

    /// Update batch-level aggregations.
    /// This is called during batch writes to maintain running aggregations.
    async fn update_batch_aggregations(
        &self,
        batch: &[MetricRecord],
        window: TimeWindow,
    ) -> Result<Vec<BatchAggregation>, Status> {
        // Default implementation that processes the batch and updates aggregations
        let mut aggregations = HashMap::new();

        for metric in batch {
            let (window_start, window_end) = window.window_bounds(metric.timestamp);
            let key = (metric.metric_id.clone(), window_start, window_end);

            let agg = aggregations.entry(key).or_insert_with(|| BatchAggregation {
                metric_id: metric.metric_id.clone(),
                window_start,
                window_end,
                running_sum: 0.0,
                running_count: 0,
                min_value: f64::INFINITY,
                max_value: f64::NEG_INFINITY,
            });

            // Update running aggregations
            agg.running_sum += metric.value_running_window_sum;
            agg.running_count += 1;
            agg.min_value = agg.min_value.min(metric.value_running_window_sum);
            agg.max_value = agg.max_value.max(metric.value_running_window_sum);
        }

        Ok(aggregations.into_values().collect())
    }

    /// Insert batch-level aggregations.
    /// This is called after update_batch_aggregations to persist the aggregations.
    async fn insert_batch_aggregations(
        &self,
        aggregations: Vec<BatchAggregation>,
    ) -> Result<(), Status> {
        // Default implementation that stores aggregations in a separate table
        let mut batch = Vec::new();
        for agg in aggregations {
            batch.push(MetricRecord {
                metric_id: agg.metric_id,
                timestamp: agg.window_start,
                value_running_window_sum: agg.running_sum,
                value_running_window_avg: agg.running_sum / agg.running_count as f64,
                value_running_window_count: agg.running_count,
            });
        }
        self.insert_metrics(batch).await
    }
}
