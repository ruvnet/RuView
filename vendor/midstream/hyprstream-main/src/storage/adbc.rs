//! ADBC (Arrow Database Connectivity) storage backend implementation.
//!
//! This module provides a storage backend using ADBC, enabling:
//! - Connection to any ADBC-compliant database
//! - High-performance data transport using Arrow's columnar format
//! - Connection pooling and prepared statements
//! - Support for various database systems (PostgreSQL, MySQL, etc.)
//!
//! # Configuration
//!
//! The ADBC backend can be configured using the following options:
//!
//! ```toml
//! [engine]
//! engine = "adbc"
//! # Base connection without credentials
//! connection = "postgresql://localhost:5432/metrics"
//! options = {
//!     driver_path = "/usr/local/lib/libadbc_driver_postgresql.so",  # Required: Path to ADBC driver
//!     pool_max = "10",                                            # Optional: Maximum pool connections
//!     pool_min = "1",                                             # Optional: Minimum pool connections
//!     connect_timeout = "30"                                      # Optional: Connection timeout in seconds
//! }
//! ```
//!
//! For security, credentials should be provided via environment variables:
//! ```bash
//! export HYPRSTREAM_DB_USERNAME=postgres
//! export HYPRSTREAM_DB_PASSWORD=secret
//! ```
//!
//! Or via command line:
//!
//! ```bash
//! hyprstream \
//!   --engine adbc \
//!   --engine-connection "postgresql://localhost:5432/metrics" \
//!   --engine-options driver_path=/usr/local/lib/libadbc_driver_postgresql.so \
//!   --engine-options pool_max=10
//! ```
//!
//! The implementation is optimized for efficient data transfer and
//! query execution using Arrow's native formats.

use adbc_core::{
    driver_manager::{ManagedConnection, ManagedDriver},
    options::{AdbcVersion, OptionDatabase, OptionValue},
    Connection, Database, Driver, Statement, Optionable,
};
use arrow_array::{
    Array, Int8Array, Int16Array, Int32Array, Int64Array,
    Float32Array, Float64Array, BooleanArray, StringArray,
    BinaryArray, TimestampNanosecondArray,
};
use arrow_schema::{Schema, DataType, Field};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;
use tonic::Status;
use crate::aggregation::{AggregateFunction, GroupBy, AggregateResult, build_aggregate_query};
use crate::storage::table_manager::{TableManager, AggregationView};
use crate::config::Credentials;
use crate::metrics::MetricRecord;
use crate::storage::StorageBackend;
use crate::storage::cache::{CacheManager, CacheEviction};
use arrow_array::ArrayRef;
use arrow_array::RecordBatch;
use crate::aggregation::TimeWindow;
use crate::storage::BatchAggregation;
use std::time::Duration;
use hex;

pub struct AdbcBackend {
    conn: Arc<Mutex<ManagedConnection>>,
    statement_counter: AtomicU64,
    prepared_statements: Arc<Mutex<Vec<(u64, String)>>>,
    cache_manager: CacheManager,
    table_manager: TableManager,
}

#[async_trait]
impl CacheEviction for AdbcBackend {
    async fn execute_eviction(&self, query: &str) -> Result<(), Status> {
        let conn = self.conn.clone();
        let query = query.to_string(); // Clone for background task
        tokio::spawn(async move {
            let mut conn_guard = conn.lock().await;
            if let Err(e) = conn_guard.new_statement()
                .and_then(|mut stmt| {
                    stmt.set_sql_query(&query)?;
                    stmt.execute_update()
                }) {
                eprintln!("Background eviction error: {}", e);
            }
        });
        Ok(())
    }
}

impl AdbcBackend {
    pub fn new(driver_path: &str, connection: Option<&str>, credentials: Option<&Credentials>) -> Result<Self, Status> {
        let mut driver = ManagedDriver::load_dynamic_from_filename(
            driver_path,
            None,
            AdbcVersion::V100,
        ).map_err(|e| Status::internal(format!("Failed to load ADBC driver: {}", e)))?;

        let mut database = driver.new_database()
            .map_err(|e| Status::internal(format!("Failed to create database: {}", e)))?;

        // Set connection string if provided
        if let Some(conn_str) = connection {
            database.set_option(OptionDatabase::Uri, OptionValue::String(conn_str.to_string()))
                .map_err(|e| Status::internal(format!("Failed to set connection string: {}", e)))?;
        }

        // Set credentials if provided
        if let Some(creds) = credentials {
            database.set_option(OptionDatabase::Username, OptionValue::String(creds.username.clone()))
                .map_err(|e| Status::internal(format!("Failed to set username: {}", e)))?;

            database.set_option(OptionDatabase::Password, OptionValue::String(creds.password.clone()))
                .map_err(|e| Status::internal(format!("Failed to set password: {}", e)))?;
        }

        let connection = database.new_connection()
            .map_err(|e| Status::internal(format!("Failed to create connection: {}", e)))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(connection)),
            statement_counter: AtomicU64::new(0),
            prepared_statements: Arc::new(Mutex::new(Vec::new())),
            cache_manager: CacheManager::new(None), // Initialize without TTL
            table_manager: TableManager::new(),
        })
    }

    async fn get_connection(&self) -> Result<tokio::sync::MutexGuard<'_, ManagedConnection>, Status> {
        Ok(self.conn.lock().await)
    }

    async fn execute_statement(&self, conn: &mut ManagedConnection, query: &str) -> Result<(), Status> {
        let mut stmt = conn.new_statement()
            .map_err(|e| Status::internal(format!("Failed to create statement: {}", e)))?;

        stmt.set_sql_query(query)
            .map_err(|e| Status::internal(format!("Failed to set query: {}", e)))?;

        stmt.execute_update()
            .map_err(|e| Status::internal(format!("Failed to execute statement: {}", e)))?;

        Ok(())
    }

    async fn execute_query(&self, conn: &mut ManagedConnection, query: &str, params: Option<RecordBatch>) -> Result<Vec<MetricRecord>, Status> {
        let mut stmt = conn.new_statement()
            .map_err(|e| Status::internal(format!("Failed to create statement: {}", e)))?;

        stmt.set_sql_query(query)
            .map_err(|e| Status::internal(format!("Failed to set query: {}", e)))?;

        if let Some(batch) = params {
            // Create a new statement for binding parameters
            let mut bind_stmt = conn.new_statement()
                .map_err(|e| Status::internal(format!("Failed to create bind statement: {}", e)))?;

            // Set the parameters using SQL directly
            let mut param_values = Vec::new();
            for i in 0..batch.num_rows() {
                for j in 0..batch.num_columns() {
                    let col = batch.column(j);
                    match col.data_type() {
                        DataType::Int64 => {
                            let array = col.as_any().downcast_ref::<Int64Array>().unwrap();
                            param_values.push(array.value(i).to_string());
                        }
                        DataType::Float64 => {
                            let array = col.as_any().downcast_ref::<Float64Array>().unwrap();
                            param_values.push(array.value(i).to_string());
                        }
                        DataType::Utf8 => {
                            let array = col.as_any().downcast_ref::<StringArray>().unwrap();
                            param_values.push(format!("'{}'", array.value(i)));
                        }
                        _ => return Err(Status::internal("Unsupported parameter type")),
                    }
                }
            }

            let params_sql = format!("VALUES ({})", param_values.join(", "));
            bind_stmt.set_sql_query(&params_sql)
                .map_err(|e| Status::internal(format!("Failed to set parameters: {}", e)))?;

            let mut bind_result = bind_stmt.execute()
                .map_err(|e| Status::internal(format!("Failed to execute parameter binding: {}", e)))?;

            while let Some(batch_result) = bind_result.next() {
                let _ = batch_result.map_err(|e| Status::internal(format!("Failed to bind parameters: {}", e)))?;
            }
        }

        let mut reader = stmt.execute()
            .map_err(|e| Status::internal(format!("Failed to execute query: {}", e)))?;

        let mut metrics = Vec::new();
        while let Some(batch_result) = reader.next() {
            let batch = batch_result.map_err(|e| Status::internal(format!("Failed to get next batch: {}", e)))?;
            
            let metric_ids = batch.column_by_name("metric_id")
                .and_then(|col| col.as_any().downcast_ref::<StringArray>())
                .ok_or_else(|| Status::internal("Invalid metric_id column"))?;

            let timestamps = batch.column_by_name("timestamp")
                .and_then(|col| col.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| Status::internal("Invalid timestamp column"))?;

            let sums = batch.column_by_name("value_running_window_sum")
                .and_then(|col| col.as_any().downcast_ref::<Float64Array>())
                .ok_or_else(|| Status::internal("Invalid value_running_window_sum column"))?;

            let avgs = batch.column_by_name("value_running_window_avg")
                .and_then(|col| col.as_any().downcast_ref::<Float64Array>())
                .ok_or_else(|| Status::internal("Invalid value_running_window_avg column"))?;

            let counts = batch.column_by_name("value_running_window_count")
                .and_then(|col| col.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| Status::internal("Invalid value_running_window_count column"))?;

            for i in 0..batch.num_rows() {
                metrics.push(MetricRecord {
                    metric_id: metric_ids.value(i).to_string(),
                    timestamp: timestamps.value(i),
                    value_running_window_sum: sums.value(i),
                    value_running_window_avg: avgs.value(i),
                    value_running_window_count: counts.value(i),
                });
            }
        }

        Ok(metrics)
    }

    fn prepare_timestamp_param(timestamp: i64) -> Result<RecordBatch, Status> {
        let schema = Arc::new(Schema::new(vec![
            Field::new("timestamp", DataType::Int64, false),
        ]));

        let timestamps: ArrayRef = Arc::new(Int64Array::from(vec![timestamp]));
        
        RecordBatch::try_new(schema, vec![timestamps])
            .map_err(|e| Status::internal(format!("Failed to create parameter batch: {}", e)))
    }

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

    /// Inserts a batch of metrics with optimized aggregation updates.
    async fn insert_batch_optimized(&self, metrics: &[MetricRecord], _window: TimeWindow) -> Result<(), Status> {
        // Begin transaction
        self.begin_transaction().await?;
        let mut conn = self.conn.lock().await;
        
        // Insert metrics
        let batch = Self::prepare_params(metrics)?;
        let sql = self.build_insert_sql("metrics", &batch);
        let mut stmt = conn.new_statement()
            .map_err(|e| Status::internal(format!("Failed to create statement: {}", e)))?;

        stmt.set_sql_query(&sql)
            .map_err(|e| Status::internal(format!("Failed to set query: {}", e)))?;

        // Bind parameters
        let mut bind_stmt = conn.new_statement()
            .map_err(|e| Status::internal(format!("Failed to create bind statement: {}", e)))?;

        let mut param_values = Vec::new();
        for i in 0..batch.num_rows() {
            for j in 0..batch.num_columns() {
                let col = batch.column(j);
                match col.data_type() {
                    DataType::Int64 => {
                        let array = col.as_any().downcast_ref::<Int64Array>().unwrap();
                        param_values.push(array.value(i).to_string());
                    }
                    DataType::Float64 => {
                        let array = col.as_any().downcast_ref::<Float64Array>().unwrap();
                        param_values.push(array.value(i).to_string());
                    }
                    DataType::Utf8 => {
                        let array = col.as_any().downcast_ref::<StringArray>().unwrap();
                        param_values.push(format!("'{}'", array.value(i)));
                    }
                    _ => return Err(Status::internal("Unsupported parameter type")),
                }
            }
        }

        let params_sql = format!("VALUES ({})", param_values.join(", "));
        bind_stmt.set_sql_query(&params_sql)
            .map_err(|e| Status::internal(format!("Failed to set parameters: {}", e)))?;

        let mut bind_result = bind_stmt.execute()
            .map_err(|e| Status::internal(format!("Failed to execute parameter binding: {}", e)))?;

        while let Some(batch_result) = bind_result.next() {
            let _ = batch_result.map_err(|e| Status::internal(format!("Failed to bind parameters: {}", e)))?;
        }

        stmt.execute_update()
            .map_err(|e| Status::internal(format!("Failed to insert metrics: {}", e)))?;

        // Commit transaction
        self.commit_transaction().await?;

        Ok(())
    }

    /// Prepares parameters for aggregation insertion
    fn prepare_aggregation_params(agg: &BatchAggregation) -> Result<RecordBatch, Status> {
        let schema = Arc::new(Schema::new(vec![
            Field::new("metric_id", DataType::Utf8, false),
            Field::new("window_start", DataType::Int64, false),
            Field::new("window_end", DataType::Int64, false),
            Field::new("running_sum", DataType::Float64, false),
            Field::new("running_count", DataType::Int64, false),
            Field::new("min_value", DataType::Float64, false),
            Field::new("max_value", DataType::Float64, false),
        ]));

        let arrays: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(vec![agg.metric_id.as_str()])),
            Arc::new(Int64Array::from(vec![agg.window_start])),
            Arc::new(Int64Array::from(vec![agg.window_end])),
            Arc::new(Float64Array::from(vec![agg.running_sum])),
            Arc::new(Int64Array::from(vec![agg.running_count])),
            Arc::new(Float64Array::from(vec![agg.min_value])),
            Arc::new(Float64Array::from(vec![agg.max_value])),
        ];

        RecordBatch::try_new(schema, arrays)
            .map_err(|e| Status::internal(format!("Failed to create aggregation batch: {}", e)))
    }

    async fn begin_transaction(&self) -> Result<(), Status> {
        let mut conn = self.conn.lock().await;
        let mut stmt = conn.new_statement()
            .map_err(|e| Status::internal(format!("Failed to create statement: {}", e)))?;

        stmt.set_sql_query("BEGIN")
            .map_err(|e| Status::internal(format!("Failed to set SQL query: {}", e)))?;

        stmt.execute_update()
            .map_err(|e| Status::internal(format!("Failed to execute query: {}", e)))?;

        Ok(())
    }

    async fn commit_transaction(&self) -> Result<(), Status> {
        let mut conn = self.conn.lock().await;
        let mut stmt = conn.new_statement()
            .map_err(|e| Status::internal(format!("Failed to create statement: {}", e)))?;

        stmt.set_sql_query("COMMIT")
            .map_err(|e| Status::internal(format!("Failed to set SQL query: {}", e)))?;

        stmt.execute_update()
            .map_err(|e| Status::internal(format!("Failed to execute query: {}", e)))?;

        Ok(())
    }

    async fn rollback_transaction(&self, conn: &mut ManagedConnection) -> Result<(), Status> {
        self.execute_statement(conn, "ROLLBACK").await
    }

    async fn create_table(&self, table_name: &str, schema: &Schema) -> Result<(), Status> {
        let mut conn = self.conn.lock().await;
        let sql = self.build_create_table_sql(table_name, schema);
        self.execute_statement(&mut conn, &sql).await
    }

    async fn create_view(&self, view: &AggregationView, sql: &str) -> Result<(), Status> {
        let mut conn = self.conn.lock().await;
        let create_view_sql = format!("CREATE VIEW {} AS {}", view.source_table, sql);
        self.execute_statement(&mut conn, &create_view_sql).await
    }

    async fn drop_table(&self, table_name: &str) -> Result<(), Status> {
        let mut conn = self.conn.lock().await;
        let sql = format!("DROP TABLE IF EXISTS {}", table_name);
        self.execute_statement(&mut conn, &sql).await
    }

    async fn drop_view(&self, view_name: &str) -> Result<(), Status> {
        let mut conn = self.conn.lock().await;
        let sql = format!("DROP VIEW IF EXISTS {}", view_name);
        self.execute_statement(&mut conn, &sql).await
    }

    fn build_create_table_sql(&self, table_name: &str, schema: &Schema) -> String {
        let mut sql = format!("CREATE TABLE IF NOT EXISTS {} (", table_name);
        let mut first = true;

        for field in schema.fields() {
            if !first {
                sql.push_str(", ");
            }
            first = false;

            sql.push_str(&format!("{} {}", field.name(), self.arrow_type_to_sql_type(field.data_type())));
        }

        sql.push_str(")");
        sql
    }

    fn build_insert_sql(&self, table_name: &str, batch: &RecordBatch) -> String {
        let mut sql = format!("INSERT INTO {} (", table_name);
        let mut first = true;

        for field in batch.schema().fields() {
            if !first {
                sql.push_str(", ");
            }
            first = false;
            sql.push_str(field.name());
        }

        sql.push_str(") VALUES (");
        first = true;

        for _i in 0..batch.num_columns() {
            if !first {
                sql.push_str(", ");
            }
            first = false;
            sql.push('?');
        }

        sql.push(')');
        sql
    }

    fn arrow_type_to_sql_type(&self, data_type: &DataType) -> &'static str {
        match data_type {
            DataType::Boolean => "BOOLEAN",
            DataType::Int8 => "TINYINT",
            DataType::Int16 => "SMALLINT",
            DataType::Int32 => "INTEGER",
            DataType::Int64 => "BIGINT",
            DataType::UInt8 => "TINYINT UNSIGNED",
            DataType::UInt16 => "SMALLINT UNSIGNED",
            DataType::UInt32 => "INTEGER UNSIGNED",
            DataType::UInt64 => "BIGINT UNSIGNED",
            DataType::Float32 => "FLOAT",
            DataType::Float64 => "DOUBLE",
            DataType::Utf8 => "VARCHAR",
            DataType::Binary => "BLOB",
            DataType::Date32 => "DATE",
            DataType::Date64 => "DATE",
            DataType::Time32(_) => "TIME",
            DataType::Time64(_) => "TIME",
            DataType::Timestamp(_, _) => "TIMESTAMP",
            _ => "VARCHAR",
        }
    }
}

#[async_trait]
impl StorageBackend for AdbcBackend {
    async fn init(&self) -> Result<(), Status> {
        let mut conn = self.conn.lock().await;
        
        // Create metrics table
        let mut stmt = conn.new_statement()
            .map_err(|e| Status::internal(format!("Failed to create statement: {}", e)))?;

        stmt.set_sql_query(r#"
            CREATE TABLE IF NOT EXISTS metrics (
                metric_id VARCHAR NOT NULL,
                timestamp BIGINT NOT NULL,
                value_running_window_sum DOUBLE PRECISION NOT NULL,
                value_running_window_avg DOUBLE PRECISION NOT NULL,
                value_running_window_count BIGINT NOT NULL,
                PRIMARY KEY (metric_id, timestamp)
            );

            CREATE INDEX IF NOT EXISTS idx_metrics_timestamp ON metrics(timestamp);

            CREATE TABLE IF NOT EXISTS metric_aggregations (
                metric_id VARCHAR NOT NULL,
                window_start BIGINT NOT NULL,
                window_end BIGINT NOT NULL,
                running_sum DOUBLE PRECISION NOT NULL,
                running_count BIGINT NOT NULL,
                min_value DOUBLE PRECISION NOT NULL,
                max_value DOUBLE PRECISION NOT NULL,
                PRIMARY KEY (metric_id, window_start, window_end)
            );

            CREATE INDEX IF NOT EXISTS idx_aggregations_window 
            ON metric_aggregations(window_start, window_end);
        "#).map_err(|e| Status::internal(format!("Failed to set query: {}", e)))?;

        stmt.execute_update()
            .map_err(|e| Status::internal(format!("Failed to create tables: {}", e)))?;

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

        let mut conn = self.conn.lock().await;
        
        let query = r#"
            SELECT
                metric_id,
                timestamp,
                value_running_window_sum,
                value_running_window_avg,
                value_running_window_count
            FROM metrics
            WHERE timestamp >= ?
            ORDER BY timestamp ASC
        "#;

        let params = Self::prepare_timestamp_param(from_timestamp)?;
        self.execute_query(&mut conn, query, Some(params)).await
    }

    async fn prepare_sql(&self, query: &str) -> Result<Vec<u8>, Status> {
        let handle = self.statement_counter.fetch_add(1, Ordering::SeqCst);
        let mut statements = self.prepared_statements.lock().await;
        statements.push((handle, query.to_string()));
        Ok(handle.to_le_bytes().to_vec())
    }

    async fn query_sql(&self, statement_handle: &[u8]) -> Result<Vec<MetricRecord>, Status> {
        let handle = u64::from_le_bytes(
            statement_handle.try_into()
                .map_err(|_| Status::invalid_argument("Invalid statement handle"))?
        );

        let statements = self.prepared_statements.lock().await;
        let sql = statements
            .iter()
            .find(|(h, _)| *h == handle)
            .map(|(_, sql)| sql.as_str())
            .ok_or_else(|| Status::invalid_argument("Statement handle not found"))?;

        let mut conn = self.conn.lock().await;
        self.execute_query(&mut conn, sql, None).await
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

        const DEFAULT_COLUMNS: [&str; 5] = [
            "metric_id",
            "timestamp",
            "value_running_window_sum",
            "value_running_window_avg",
            "value_running_window_count"
        ];

        let query = build_aggregate_query(
            "metrics",
            function,
            group_by,
            &DEFAULT_COLUMNS,
            Some(from_timestamp),
            to_timestamp,
        );
        let mut conn = self.conn.lock().await;
        let metrics = self.execute_query(&mut conn, &query, None).await?;

        let mut results = Vec::new();
        for metric in metrics {
            let result = AggregateResult {
                value: metric.value_running_window_sum,
                timestamp: metric.timestamp,
                // Add any other fields required by AggregateResult
            };
            results.push(result);
        }

        Ok(results)
    }

    fn new_with_options(
        connection_string: &str,
        options: &HashMap<String, String>,
        credentials: Option<&Credentials>,
    ) -> Result<Self, Status> {
        let driver_path = options.get("driver_path")
            .ok_or_else(|| Status::invalid_argument("driver_path is required"))?;

        let mut driver = ManagedDriver::load_dynamic_from_filename(
            driver_path,
            None,
            AdbcVersion::V100,
        ).map_err(|e| Status::internal(format!("Failed to load ADBC driver: {}", e)))?;

        let mut database = driver.new_database()
            .map_err(|e| Status::internal(format!("Failed to create database: {}", e)))?;

        // Set connection string
        database.set_option(OptionDatabase::Uri, OptionValue::String(connection_string.to_string()))
            .map_err(|e| Status::internal(format!("Failed to set connection string: {}", e)))?;

        // Set credentials if provided
        if let Some(creds) = credentials {
            database.set_option(OptionDatabase::Username, OptionValue::String(creds.username.clone()))
                .map_err(|e| Status::internal(format!("Failed to set username: {}", e)))?;

            database.set_option(OptionDatabase::Password, OptionValue::String(creds.password.clone()))
                .map_err(|e| Status::internal(format!("Failed to set password: {}", e)))?;
        }

        let connection = database.new_connection()
            .map_err(|e| Status::internal(format!("Failed to create connection: {}", e)))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(connection)),
            statement_counter: AtomicU64::new(0),
            prepared_statements: Arc::new(Mutex::new(Vec::new())),
            cache_manager: CacheManager::new(None), // Initialize without TTL
            table_manager: TableManager::new(),
        })
    }

    async fn create_table(&self, table_name: &str, schema: &Schema) -> Result<(), Status> {
        let mut conn = self.conn.lock().await;
        let sql = self.build_create_table_sql(table_name, schema);
        self.execute_statement(&mut conn, &sql).await
    }

    async fn insert_into_table(&self, table_name: &str, batch: RecordBatch) -> Result<(), Status> {
        let mut conn = self.conn.lock().await;
        let sql = self.build_insert_sql(table_name, &batch);
        self.execute_statement(&mut conn, &sql).await
    }

    async fn query_table(&self, table_name: &str, projection: Option<Vec<String>>) -> Result<RecordBatch, Status> {
        let mut conn = self.conn.lock().await;
        let mut stmt = conn.new_statement()
            .map_err(|e| Status::internal(format!("Failed to create statement: {}", e)))?;

        let columns = projection.map(|cols| cols.join(", ")).unwrap_or_else(|| "*".to_string());
        let sql = format!("SELECT {} FROM {}", columns, table_name);

        stmt.set_sql_query(&sql)
            .map_err(|e| Status::internal(format!("Failed to set query: {}", e)))?;

        let mut reader = stmt.execute()
            .map_err(|e| Status::internal(format!("Failed to execute query: {}", e)))?;

        let batch = reader.next()
            .ok_or_else(|| Status::internal("No data returned"))?
            .map_err(|e| Status::internal(format!("Failed to read record batch: {}", e)))?;

        // Convert ADBC RecordBatch to Arrow RecordBatch
        let schema = batch.schema();
        let mut arrays = Vec::with_capacity(batch.num_columns());

        for i in 0..batch.num_columns() {
            let col = batch.column(i);
            let array: ArrayRef = match col.data_type() {
                &duckdb::arrow::datatypes::DataType::Int64 => {
                    Arc::new(col.as_any().downcast_ref::<Int64Array>().unwrap().clone())
                },
                &duckdb::arrow::datatypes::DataType::Float64 => {
                    Arc::new(col.as_any().downcast_ref::<Float64Array>().unwrap().clone())
                },
                &duckdb::arrow::datatypes::DataType::Utf8 => {
                    Arc::new(col.as_any().downcast_ref::<StringArray>().unwrap().clone())
                },
                _ => return Err(Status::internal("Unsupported column type")),
            };
            arrays.push(array);
        }

        // Convert DuckDB schema to Arrow schema
        let fields: Vec<Field> = schema.fields().iter().map(|f| {
            Field::new(
                f.name(),
                match f.data_type() {
                    &duckdb::arrow::datatypes::DataType::Int64 => DataType::Int64,
                    &duckdb::arrow::datatypes::DataType::Float64 => DataType::Float64,
                    &duckdb::arrow::datatypes::DataType::Utf8 => DataType::Utf8,
                    _ => DataType::Utf8, // Default to string for unsupported types
                },
                f.is_nullable()
            )
        }).collect();

        let arrow_schema = Schema::new(fields);
        RecordBatch::try_new(Arc::new(arrow_schema), arrays)
            .map_err(|e| Status::internal(format!("Failed to create record batch: {}", e)))
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
        
        let mut conn = self.conn.lock().await;
        let mut stmt = conn.new_statement()
            .map_err(|e| Status::internal(format!("Failed to create statement: {}", e)))?;

        stmt.set_sql_query(&format!("CREATE VIEW {} AS {}", view.source_table, sql))
            .map_err(|e| Status::internal(format!("Failed to set SQL query: {}", e)))?;

        stmt.execute_update()
            .map_err(|e| Status::internal(format!("Failed to execute query: {}", e)))?;

        Ok(())
    }

    async fn query_aggregation_view(&self, view_name: &str) -> Result<RecordBatch, Status> {
        let sql = format!("SELECT * FROM {}", view_name);

        let mut conn = self.conn.lock().await;
        let mut stmt = conn.new_statement()
            .map_err(|e| Status::internal(format!("Failed to create statement: {}", e)))?;

        stmt.set_sql_query(&sql)
            .map_err(|e| Status::internal(format!("Failed to set SQL query: {}", e)))?;

        let mut reader = stmt.execute()
            .map_err(|e| Status::internal(format!("Failed to execute query: {}", e)))?;

        let batch = reader.next()
            .ok_or_else(|| Status::internal("No data returned"))?
            .map_err(|e| Status::internal(format!("Failed to read record batch: {}", e)))?;

        // Convert DuckDB RecordBatch to Arrow RecordBatch
        let schema = batch.schema();
        let mut arrays = Vec::with_capacity(batch.num_columns());

        for i in 0..batch.num_columns() {
            let col = batch.column(i);
            let array: ArrayRef = match col.data_type() {
                &duckdb::arrow::datatypes::DataType::Int64 => {
                    Arc::new(col.as_any().downcast_ref::<Int64Array>().unwrap().clone())
                },
                &duckdb::arrow::datatypes::DataType::Float64 => {
                    Arc::new(col.as_any().downcast_ref::<Float64Array>().unwrap().clone())
                },
                &duckdb::arrow::datatypes::DataType::Utf8 => {
                    Arc::new(col.as_any().downcast_ref::<StringArray>().unwrap().clone())
                },
                _ => return Err(Status::internal("Unsupported column type")),
            };
            arrays.push(array);
        }

        // Convert DuckDB schema to Arrow schema
        let fields: Vec<Field> = schema.fields().iter().map(|f| {
            Field::new(
                f.name(),
                match f.data_type() {
                    &duckdb::arrow::datatypes::DataType::Int64 => DataType::Int64,
                    &duckdb::arrow::datatypes::DataType::Float64 => DataType::Float64,
                    &duckdb::arrow::datatypes::DataType::Utf8 => DataType::Utf8,
                    _ => DataType::Utf8, // Default to string for unsupported types
                },
                f.is_nullable()
            )
        }).collect();

        let arrow_schema = Schema::new(fields);
        RecordBatch::try_new(Arc::new(arrow_schema), arrays)
            .map_err(|e| Status::internal(format!("Failed to create record batch: {}", e)))
    }

    async fn drop_table(&self, table_name: &str) -> Result<(), Status> {
        let mut conn = self.conn.lock().await;
        let mut stmt = conn.new_statement().map_err(|e| Status::internal(format!("Failed to create statement: {}", e)))?;
        stmt.set_sql_query(&format!("DROP TABLE IF EXISTS {}", table_name))
            .map_err(|e| Status::internal(format!("Failed to set SQL query: {}", e)))?;
        stmt.execute_update().map_err(|e| Status::internal(format!("Failed to drop table: {}", e)))?;
        Ok(())
    }

    async fn drop_aggregation_view(&self, view_name: &str) -> Result<(), Status> {
        let mut conn = self.conn.lock().await;
        let mut stmt = conn.new_statement().map_err(|e| Status::internal(format!("Failed to create statement: {}", e)))?;
        stmt.set_sql_query(&format!("DROP VIEW IF EXISTS {}", view_name))
            .map_err(|e| Status::internal(format!("Failed to set SQL query: {}", e)))?;
        stmt.execute_update().map_err(|e| Status::internal(format!("Failed to drop view: {}", e)))?;
        Ok(())
    }

    fn table_manager(&self) -> &TableManager {
        &self.table_manager
    }
}

