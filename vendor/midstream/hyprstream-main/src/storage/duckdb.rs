//! DuckDB storage backend implementation.
//!
//! This module provides a high-performance storage backend using DuckDB,
//! an embedded analytical database. The implementation supports:
//! - In-memory and persistent storage options
//! - Efficient batch operations
//! - SQL query capabilities
//! - Time-based filtering
//!
//! # Configuration
//!
//! The DuckDB backend can be configured using the following options:
//!
//! ```toml
//! [engine]
//! engine = "duckdb"
//! connection = ":memory:"  # Use ":memory:" for in-memory or file path
//! options = {
//!     threads = "4",      # Optional: Number of threads (default: 4)
//!     read_only = "false" # Optional: Read-only mode (default: false)
//! }
//! ```
//!
//! Or via command line:
//!
//! ```bash
//! hyprstream \
//!   --engine duckdb \
//!   --engine-connection ":memory:" \
//!   --engine-options threads=4 \
//!   --engine-options read_only=false
//! ```
//!
//! DuckDB is particularly well-suited for analytics workloads and
//! provides excellent performance for both caching and primary storage.

use std::collections::HashMap;
use std::sync::Arc;
use duckdb::{Connection, Config, params, ToSql};
use tokio::sync::Mutex;
use tonic::Status;
use crate::metrics::MetricRecord;
use crate::config::Credentials;
use crate::storage::{StorageBackend, BatchAggregation};
use crate::storage::cache::{CacheManager, CacheEviction};
use crate::storage::table_manager::{TableManager, AggregationView};
use crate::aggregation::{TimeWindow, AggregateFunction, GroupBy, AggregateResult, build_aggregate_query};
use async_trait::async_trait;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::array::{
    Array, ArrayRef, RecordBatch, Int64Array, Float64Array, StringArray,
};
use arrow::array::builder::{
    ArrayBuilder, Int64Builder, Float64Builder, StringBuilder,
};
use std::time::Duration;

/// DuckDB-based storage backend for metrics.
#[derive(Clone)]
pub struct DuckDbBackend {
    conn: Arc<Mutex<Connection>>,
    cache_manager: CacheManager,
    table_manager: TableManager,
}

impl DuckDbBackend {
    /// Creates a new DuckDB backend instance.
    pub fn new(connection_string: String, _options: HashMap<String, String>, ttl: Option<u64>) -> Result<Self, Status> {
        let config = Config::default();
        let conn = Connection::open_with_flags(&connection_string, config)
            .map_err(|e| Status::internal(e.to_string()))?;

        let backend = Self {
            conn: Arc::new(Mutex::new(conn)),
            cache_manager: CacheManager::new(ttl),
            table_manager: TableManager::new(),
        };

        // Initialize tables
        let backend_clone = backend.clone();
        tokio::spawn(async move {
            if let Err(e) = backend_clone.init().await {
                eprintln!("Failed to initialize tables: {}", e);
            }
        });

        Ok(backend)
    }

    /// Creates a new DuckDB backend with an in-memory database.
    pub fn new_in_memory() -> Result<Self, Status> {
        Self::new(":memory:".to_string(), HashMap::new(), Some(0))
    }

    /// Inserts a batch of metrics with optimized aggregation updates.
    async fn insert_batch_optimized(&self, metrics: &[MetricRecord], window: TimeWindow) -> Result<(), Status> {
        let conn = self.conn.lock().await;
        
        // Begin transaction
        conn.execute("BEGIN TRANSACTION", params![])
            .map_err(|e| Status::internal(format!("Failed to begin transaction: {}", e)))?;

        // Convert metrics to RecordBatch for efficient insertion
        let batch = Self::prepare_params(metrics)?;

        // Insert metrics using prepared statement
        let mut stmt = conn.prepare(r#"
            INSERT INTO metrics (
                metric_id,
                timestamp,
                value_running_window_sum,
                value_running_window_avg,
                value_running_window_count
            ) VALUES (?, ?, ?, ?, ?)
        "#).map_err(|e| Status::internal(format!("Failed to prepare statement: {}", e)))?;

        // Bind and execute in batches
        for i in 0..batch.num_rows() {
            let metric_id = batch.column(0).as_any().downcast_ref::<StringArray>().unwrap().value(i);
            let timestamp = batch.column(1).as_any().downcast_ref::<Int64Array>().unwrap().value(i);
            let sum = batch.column(2).as_any().downcast_ref::<Float64Array>().unwrap().value(i);
            let avg = batch.column(3).as_any().downcast_ref::<Float64Array>().unwrap().value(i);
            let count = batch.column(4).as_any().downcast_ref::<Int64Array>().unwrap().value(i);

            stmt.execute(params![
                metric_id,
                timestamp,
                sum,
                avg,
                count,
            ]).map_err(|e| Status::internal(format!("Failed to insert metrics: {}", e)))?;
        }

        // Update aggregations based on window
        let window_start = match window {
            TimeWindow::Sliding { window, slide: _ } => {
                let now = metrics.iter().map(|m| m.timestamp).max().unwrap_or(0);
                now - window.as_nanos() as i64
            }
            TimeWindow::Fixed(start) => start.as_nanos() as i64,
            TimeWindow::None => metrics.iter().map(|m| m.timestamp).min().unwrap_or(0),
        };

        let window_end = match window {
            TimeWindow::Sliding { window: _, slide: _ } => {
                metrics.iter().map(|m| m.timestamp).max().unwrap_or(0)
            }
            TimeWindow::Fixed(end) => end.as_nanos() as i64,
            TimeWindow::None => metrics.iter().map(|m| m.timestamp).max().unwrap_or(0),
        };

        // Group metrics by ID and calculate aggregations
        let mut aggregations = HashMap::new();
        for metric in metrics {
            let entry = aggregations.entry(metric.metric_id.clone()).or_insert_with(|| BatchAggregation {
                metric_id: metric.metric_id.clone(),
                window_start,
                window_end,
                running_sum: 0.0,
                running_count: 0,
                min_value: f64::INFINITY,
                max_value: f64::NEG_INFINITY,
            });

            entry.running_sum += metric.value_running_window_sum;
            entry.running_count += metric.value_running_window_count as i64;
            entry.min_value = entry.min_value.min(metric.value_running_window_sum);
            entry.max_value = entry.max_value.max(metric.value_running_window_sum);
        }

        // Update aggregations table using prepared statement with proper type handling
        let mut agg_stmt = conn.prepare(r#"
            INSERT INTO metric_aggregations (
                metric_id, window_start, window_end,
                running_sum, running_count, min_value, max_value
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (metric_id, window_start, window_end) DO UPDATE
            SET running_sum = metric_aggregations.running_sum + EXCLUDED.running_sum,
                running_count = metric_aggregations.running_count + EXCLUDED.running_count,
                min_value = LEAST(metric_aggregations.min_value, EXCLUDED.min_value),
                max_value = GREATEST(metric_aggregations.max_value, EXCLUDED.max_value)
        "#).map_err(|e| Status::internal(format!("Failed to prepare aggregation statement: {}", e)))?;

        for agg in aggregations.values() {
            agg_stmt.execute(params![
                agg.metric_id,
                agg.window_start,
                agg.window_end,
                agg.running_sum,
                agg.running_count,
                agg.min_value,
                agg.max_value,
            ]).map_err(|e| Status::internal(format!("Failed to update aggregations: {}", e)))?;
        }

        // Commit transaction
        conn.execute("COMMIT", params![])
            .map_err(|e| Status::internal(format!("Failed to commit transaction: {}", e)))?;

        Ok(())
    }

    /// Prepares parameters for batch insertion
    fn prepare_params(metrics: &[MetricRecord]) -> Result<RecordBatch, Status> {
        let schema = Arc::new(Schema::new(vec![
            Field::new("metric_id", DataType::Utf8, false),
            Field::new("timestamp", DataType::Int64, false),
            Field::new("value_running_window_sum", DataType::Float64, false),
            Field::new("value_running_window_avg", DataType::Float64, false),
            Field::new("value_running_window_count", DataType::Int64, false),
        ]));

        let metric_ids = StringArray::from_iter_values(metrics.iter().map(|m| m.metric_id.as_str()));
        let timestamps = Int64Array::from_iter_values(metrics.iter().map(|m| m.timestamp));
        let sums = Float64Array::from_iter_values(metrics.iter().map(|m| m.value_running_window_sum));
        let avgs = Float64Array::from_iter_values(metrics.iter().map(|m| m.value_running_window_avg));
        let counts = Int64Array::from_iter_values(metrics.iter().map(|m| m.value_running_window_count));

        let arrays: Vec<ArrayRef> = vec![
            Arc::new(metric_ids),
            Arc::new(timestamps),
            Arc::new(sums),
            Arc::new(avgs),
            Arc::new(counts),
        ];

        RecordBatch::try_new(schema, arrays)
            .map_err(|e| Status::internal(format!("Failed to create parameter batch: {}", e)))
    }

}

#[async_trait]
impl CacheEviction for DuckDbBackend {
    async fn execute_eviction(&self, query: &str) -> Result<(), Status> {
        let conn = self.conn.clone();
        let query = query.to_string();
        tokio::spawn(async move {
            let conn_guard = conn.lock().await;
            if let Err(e) = conn_guard.execute_batch(&query) {
                eprintln!("Background eviction error: {}", e);
            }
        });
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for DuckDbBackend {
    async fn init(&self) -> Result<(), Status> {
        let conn = self.conn.lock().await;
        
        // Create metrics table with optimized schema
        conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS metrics (
                metric_id VARCHAR NOT NULL,
                timestamp BIGINT NOT NULL,
                value_running_window_sum DOUBLE NOT NULL,
                value_running_window_avg DOUBLE NOT NULL,
                value_running_window_count BIGINT NOT NULL,
                PRIMARY KEY (metric_id, timestamp)
            );

            CREATE INDEX IF NOT EXISTS idx_metrics_timestamp ON metrics(timestamp);

            CREATE TABLE IF NOT EXISTS metric_aggregations (
                metric_id VARCHAR NOT NULL,
                window_start BIGINT NOT NULL,
                window_end BIGINT NOT NULL,
                running_sum DOUBLE NOT NULL,
                running_count BIGINT NOT NULL,
                min_value DOUBLE NOT NULL,
                max_value DOUBLE NOT NULL,
                PRIMARY KEY (metric_id, window_start, window_end)
            );

            CREATE INDEX IF NOT EXISTS idx_aggregations_window 
            ON metric_aggregations(window_start, window_end);
        "#).map_err(|e| Status::internal(format!("Failed to create tables: {}", e)))?;

        Ok(())
    }

    async fn insert_metrics(&self, metrics: Vec<MetricRecord>) -> Result<(), Status> {
        if metrics.is_empty() {
            return Ok(());
        }

        // Check if eviction is needed
        if let Some(cutoff) = self.cache_manager.should_evict().await? {
            let query = self.cache_manager.eviction_query(cutoff);
            self.execute_eviction(&query).await?;
        }

        // Use sliding window for batch-level aggregations
        let window = TimeWindow::Sliding {
            window: Duration::from_secs(3600), // 1 hour window
            slide: Duration::from_secs(60),    // 1 minute slide
        };

        // Use optimized batch insertion
        self.insert_batch_optimized(&metrics, window).await
    }

    async fn query_metrics(&self, from_timestamp: i64) -> Result<Vec<MetricRecord>, Status> {
        // Check if eviction is needed
        if let Some(cutoff) = self.cache_manager.should_evict().await? {
            let query = self.cache_manager.eviction_query(cutoff);
            self.execute_eviction(&query).await?;
        }

        let query = format!(
            "SELECT metric_id, timestamp, value_running_window_sum, value_running_window_avg, value_running_window_count \
             FROM metrics WHERE timestamp >= {} ORDER BY timestamp ASC",
            from_timestamp
        );

        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(&query)
            .map_err(|e| Status::internal(e.to_string()))?;

        let mut rows = stmt.query(params![])
            .map_err(|e| Status::internal(e.to_string()))?;

        let mut metrics = Vec::new();
        while let Some(row) = rows.next().map_err(|e| Status::internal(e.to_string()))? {
            metrics.push(MetricRecord {
                metric_id: row.get(0).map_err(|e| Status::internal(e.to_string()))?,
                timestamp: row.get(1).map_err(|e| Status::internal(e.to_string()))?,
                value_running_window_sum: row.get(2).map_err(|e| Status::internal(e.to_string()))?,
                value_running_window_avg: row.get(3).map_err(|e| Status::internal(e.to_string()))?,
                value_running_window_count: row.get(4).map_err(|e| Status::internal(e.to_string()))?,
            });
        }

        Ok(metrics)
    }

    async fn prepare_sql(&self, query: &str) -> Result<Vec<u8>, Status> {
        Ok(query.as_bytes().to_vec())
    }

    async fn query_sql(&self, statement_handle: &[u8]) -> Result<Vec<MetricRecord>, Status> {
        let sql = std::str::from_utf8(statement_handle)
            .map_err(|e| Status::internal(e.to_string()))?;
        self.query_metrics(sql.parse().unwrap_or(0)).await
    }

    async fn aggregate_metrics(
        &self,
        function: AggregateFunction,
        group_by: &GroupBy,
        from_timestamp: i64,
        to_timestamp: Option<i64>,
    ) -> Result<Vec<AggregateResult>, Status> {
        // Check if eviction is needed
        if let Some(cutoff) = self.cache_manager.should_evict().await? {
            let query = self.cache_manager.eviction_query(cutoff);
            self.execute_eviction(&query).await?;
        }

        let query = build_aggregate_query(
            "metrics",
            function,
            group_by,
            &["value_running_window_sum"],
            Some(from_timestamp),
            to_timestamp,
        );

        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(&query)
            .map_err(|e| Status::internal(e.to_string()))?;

        let mut rows = stmt.query(params![])
            .map_err(|e| Status::internal(e.to_string()))?;

        let mut results = Vec::new();
        while let Some(row) = rows.next().map_err(|e| Status::internal(e.to_string()))? {
            let value: f64 = row.get(0).map_err(|e| Status::internal(e.to_string()))?;
            let timestamp: i64 = row.get(1).map_err(|e| Status::internal(e.to_string()))?;
            
            results.push(AggregateResult {
                value,
                timestamp,
            });
        }

        Ok(results)
    }

    fn new_with_options(
        connection_string: &str,
        options: &HashMap<String, String>,
        credentials: Option<&Credentials>,
    ) -> Result<Self, Status> {
        let mut all_options = options.clone();
        if let Some(creds) = credentials {
            all_options.insert("username".to_string(), creds.username.clone());
            all_options.insert("password".to_string(), creds.password.clone());
        }

        let ttl = all_options.get("ttl")
            .and_then(|s| s.parse().ok())
            .map(|ttl| if ttl == 0 { None } else { Some(ttl) })
            .unwrap_or(None);

        Self::new(connection_string.to_string(), all_options, ttl)
    }

    async fn create_table(&self, table_name: &str, schema: &Schema) -> Result<(), Status> {
        // Create table in DuckDB
        let sql = Self::schema_to_create_table_sql(table_name, schema);
        self.execute(&sql).await?;

        // Register table in manager
        self.table_manager.create_table(table_name.to_string(), schema.clone()).await?;
        Ok(())
    }

    async fn insert_into_table(&self, table_name: &str, batch: RecordBatch) -> Result<(), Status> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(&format!("INSERT INTO {} VALUES ({})",
            table_name,
            (0..batch.num_columns()).map(|_| "?").collect::<Vec<_>>().join(", ")
        )).map_err(|e| Status::internal(e.to_string()))?;

        for row_idx in 0..batch.num_rows() {
            let mut param_values: Vec<Box<dyn ToSql>> = Vec::new();
            for col_idx in 0..batch.num_columns() {
                let col = batch.column(col_idx);
                match col.data_type() {
                    DataType::Int64 => {
                        let array = col.as_any().downcast_ref::<Int64Array>().unwrap();
                        param_values.push(Box::new(array.value(row_idx)));
                    }
                    DataType::Float64 => {
                        let array = col.as_any().downcast_ref::<Float64Array>().unwrap();
                        param_values.push(Box::new(array.value(row_idx)));
                    }
                    DataType::Utf8 => {
                        let array = col.as_any().downcast_ref::<StringArray>().unwrap();
                        param_values.push(Box::new(array.value(row_idx).to_string()));
                    }
                    _ => return Err(Status::internal("Unsupported column type")),
                }
            }

            let param_refs: Vec<&dyn ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
            stmt.execute(param_refs.as_slice()).map_err(|e| Status::internal(e.to_string()))?;
        }

        Ok(())
    }

    async fn query_table(&self, table_name: &str, projection: Option<Vec<String>>) -> Result<RecordBatch, Status> {
        let schema = self.table_manager.get_table_schema(table_name).await?;
        
        let mut builders: Vec<Box<dyn ArrayBuilder>> = schema.fields().iter()
            .map(|field| Self::create_array_builder(field))
            .collect();
        
        let projection = projection.unwrap_or_else(|| {
            schema.fields().iter().map(|f| f.name().clone()).collect()
        });

        let sql = format!(
            "SELECT {} FROM {}",
            projection.join(", "),
            table_name
        );

        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(&sql)
            .map_err(|e| Status::internal(e.to_string()))?;

        let mut rows = stmt.query(params![])
            .map_err(|e| Status::internal(e.to_string()))?;

        while let Some(row) = rows.next().map_err(|e| Status::internal(e.to_string()))? {
            for (i, field) in schema.fields().iter().enumerate() {
                match field.data_type() {
                    DataType::Int64 => {
                        let builder = builders[i].as_any_mut().downcast_mut::<Int64Builder>().unwrap();
                        match row.get::<usize, i64>(i) {
                            Ok(value) => builder.append_value(value),
                            Err(_) => builder.append_null(),
                        }
                    }
                    DataType::Float64 => {
                        let builder = builders[i].as_any_mut().downcast_mut::<Float64Builder>().unwrap();
                        match row.get::<usize, f64>(i) {
                            Ok(value) => builder.append_value(value),
                            Err(_) => builder.append_null(),
                        }
                    }
                    DataType::Utf8 => {
                        let builder = builders[i].as_any_mut().downcast_mut::<StringBuilder>().unwrap();
                        match row.get::<usize, String>(i) {
                            Ok(value) => builder.append_value(value),
                            Err(_) => builder.append_null(),
                        }
                    }
                    _ => return Err(Status::internal("Unsupported column type")),
                }
            }
        }

        let arrays: Vec<ArrayRef> = builders.into_iter()
            .map(|mut builder| Arc::new(builder.finish()) as ArrayRef)
            .collect();

        Ok(RecordBatch::try_new(Arc::new(schema), arrays)
            .map_err(|e| Status::internal(format!("Failed to create record batch: {}", e)))?)
    }

    async fn create_aggregation_view(&self, view: &AggregationView) -> Result<(), Status> {
        let columns: Vec<&str> = view.aggregate_columns.iter()
            .map(|s| s.as_str())
            .collect();
            
        let sql = build_aggregate_query(
            &view.source_table,
            view.function,
            &view.group_by,
            &columns,
            None,
            None
        );
        
        let view_name = format!("agg_view_{}", view.source_table);
        let conn = self.conn.lock().await;
        conn.execute(&format!("CREATE VIEW {} AS {}", view_name, sql), params![])
            .map_err(|e| Status::internal(format!("Failed to create view: {}", e)))?;

        // Register view in manager
        self.table_manager.create_aggregation_view(
            view_name,
            view.source_table.clone(),
            view.function.clone(),
            view.group_by.clone(),
            view.window.clone(),
            view.aggregate_columns.clone(),
        ).await?;

        Ok(())
    }

    async fn query_aggregation_view(&self, view_name: &str) -> Result<RecordBatch, Status> {
        self.query_table(view_name, None).await
    }

    async fn drop_table(&self, table_name: &str) -> Result<(), Status> {
        let conn = self.conn.lock().await;
        conn.execute(&format!("DROP TABLE IF EXISTS {}", table_name), params![])
            .map_err(|e| Status::internal(format!("Failed to drop table: {}", e)))?;

        self.table_manager.drop_table(table_name).await?;
        Ok(())
    }

    async fn drop_aggregation_view(&self, view_name: &str) -> Result<(), Status> {
        let conn = self.conn.lock().await;
        conn.execute(&format!("DROP VIEW IF EXISTS {}", view_name), params![])
            .map_err(|e| Status::internal(format!("Failed to drop view: {}", e)))?;

        self.table_manager.drop_aggregation_view(view_name).await?;
        Ok(())
    }

    fn table_manager(&self) -> &TableManager {
        &self.table_manager
    }
}

impl DuckDbBackend {
    /// Executes a SQL query.
    async fn execute(&self, query: &str) -> Result<(), Status> {
        let conn = self.conn.lock().await;
        conn.execute(query, params![])
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(())
    }

    /// Converts an Arrow schema to a DuckDB CREATE TABLE statement
    fn schema_to_create_table_sql(table_name: &str, schema: &Schema) -> String {
        let mut sql = format!("CREATE TABLE IF NOT EXISTS \"{}\" (", table_name);
        let mut first = true;

        for field in schema.fields() {
            if !first {
                sql.push_str(", ");
            }
            first = false;

            sql.push_str(&format!("\"{}\" {}", field.name(), Self::arrow_type_to_duckdb_type(field.data_type())));
        }

        sql.push_str(")");
        sql
    }

    /// Converts an Arrow data type to a DuckDB type string
    fn arrow_type_to_duckdb_type(data_type: &DataType) -> &'static str {
        match data_type {
            DataType::Boolean => "BOOLEAN",
            DataType::Int8 => "TINYINT",
            DataType::Int16 => "SMALLINT",
            DataType::Int32 => "INTEGER",
            DataType::Int64 => "BIGINT",
            DataType::UInt8 => "TINYINT",
            DataType::UInt16 => "SMALLINT",
            DataType::UInt32 => "INTEGER",
            DataType::UInt64 => "BIGINT",
            DataType::Float32 => "REAL",
            DataType::Float64 => "DOUBLE",
            DataType::Utf8 => "VARCHAR",
            DataType::Binary => "BLOB",
            DataType::Date32 => "DATE",
            DataType::Date64 => "DATE",
            DataType::Time32(_) => "TIME",
            DataType::Time64(_) => "TIME",
            DataType::Timestamp(_, _) => "TIMESTAMP",
            _ => "VARCHAR", // Default to VARCHAR for unsupported types
        }
    }

    fn create_array_builder(field: &Field) -> Box<dyn ArrayBuilder> {
        match field.data_type() {
            DataType::Int64 => Box::new(Int64Builder::new()),
            DataType::Float64 => Box::new(Float64Builder::new()),
            DataType::Utf8 => Box::new(StringBuilder::new()),
            _ => panic!("Unsupported column type"),
        }
    }
}

