//! Data ingestion pipeline for streaming data into RuVector

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::{DataRecord, DataSource, FrameworkError, Result};

/// Configuration for data ingestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionConfig {
    /// Batch size for fetching
    pub batch_size: usize,

    /// Maximum concurrent fetches
    pub max_concurrent: usize,

    /// Retry count on failure
    pub retry_count: u32,

    /// Delay between retries (ms)
    pub retry_delay_ms: u64,

    /// Enable deduplication
    pub deduplicate: bool,

    /// Rate limit (requests per second, 0 = unlimited)
    pub rate_limit: u32,
}

impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            max_concurrent: 4,
            retry_count: 3,
            retry_delay_ms: 1000,
            deduplicate: true,
            rate_limit: 10,
        }
    }
}

/// Configuration for a specific data source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    /// Source identifier
    pub source_id: String,

    /// API base URL
    pub base_url: String,

    /// API key (if required)
    pub api_key: Option<String>,

    /// Additional headers
    pub headers: HashMap<String, String>,

    /// Custom parameters
    pub params: HashMap<String, String>,
}

/// Statistics for ingestion process
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IngestionStats {
    /// Total records fetched
    pub records_fetched: u64,

    /// Batches processed
    pub batches_processed: u64,

    /// Retries performed
    pub retries: u64,

    /// Errors encountered
    pub errors: u64,

    /// Duplicates skipped
    pub duplicates_skipped: u64,

    /// Bytes downloaded
    pub bytes_downloaded: u64,

    /// Average batch fetch time (ms)
    pub avg_batch_time_ms: f64,
}

/// Data ingestion pipeline
pub struct DataIngester {
    config: IngestionConfig,
    stats: Arc<std::sync::RwLock<IngestionStats>>,
    seen_ids: Arc<std::sync::RwLock<std::collections::HashSet<String>>>,
}

impl DataIngester {
    /// Create a new data ingester
    pub fn new(config: IngestionConfig) -> Self {
        Self {
            config,
            stats: Arc::new(std::sync::RwLock::new(IngestionStats::default())),
            seen_ids: Arc::new(std::sync::RwLock::new(std::collections::HashSet::new())),
        }
    }

    /// Ingest all data from a source
    pub async fn ingest_all<S: DataSource>(&self, source: &S) -> Result<Vec<DataRecord>> {
        let mut all_records = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let (batch, next_cursor) = self
                .fetch_with_retry(source, cursor.clone(), self.config.batch_size)
                .await?;

            if batch.is_empty() {
                break;
            }

            // Deduplicate if enabled
            let records = if self.config.deduplicate {
                self.deduplicate_batch(batch)
            } else {
                batch
            };

            all_records.extend(records);

            {
                let mut stats = self.stats.write().unwrap();
                stats.batches_processed += 1;
            }

            cursor = next_cursor;
            if cursor.is_none() {
                break;
            }

            // Rate limiting
            if self.config.rate_limit > 0 {
                let delay = 1000 / self.config.rate_limit as u64;
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            }
        }

        Ok(all_records)
    }

    /// Stream records with backpressure
    pub async fn stream_records<S: DataSource + 'static>(
        &self,
        source: Arc<S>,
        buffer_size: usize,
    ) -> Result<mpsc::Receiver<DataRecord>> {
        let (tx, rx) = mpsc::channel(buffer_size);
        let config = self.config.clone();
        let stats = self.stats.clone();
        let seen_ids = self.seen_ids.clone();

        tokio::spawn(async move {
            let mut cursor: Option<String> = None;

            loop {
                match source
                    .fetch_batch(cursor.clone(), config.batch_size)
                    .await
                {
                    Ok((batch, next_cursor)) => {
                        if batch.is_empty() {
                            break;
                        }

                        for record in batch {
                            // Deduplicate
                            if config.deduplicate {
                                let mut ids = seen_ids.write().unwrap();
                                if ids.contains(&record.id) {
                                    continue;
                                }
                                ids.insert(record.id.clone());
                            }

                            if tx.send(record).await.is_err() {
                                return; // Receiver dropped
                            }

                            let mut s = stats.write().unwrap();
                            s.records_fetched += 1;
                        }

                        cursor = next_cursor;
                        if cursor.is_none() {
                            break;
                        }
                    }
                    Err(_) => {
                        let mut s = stats.write().unwrap();
                        s.errors += 1;
                        break;
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Fetch a batch with retry logic
    async fn fetch_with_retry<S: DataSource>(
        &self,
        source: &S,
        cursor: Option<String>,
        batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        let mut last_error = None;

        for attempt in 0..=self.config.retry_count {
            if attempt > 0 {
                let delay = self.config.retry_delay_ms * (1 << (attempt - 1));
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;

                let mut stats = self.stats.write().unwrap();
                stats.retries += 1;
            }

            match source.fetch_batch(cursor.clone(), batch_size).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        let mut stats = self.stats.write().unwrap();
        stats.errors += 1;

        Err(last_error.unwrap_or_else(|| FrameworkError::Ingestion("Unknown error".to_string())))
    }

    /// Deduplicate a batch of records
    fn deduplicate_batch(&self, batch: Vec<DataRecord>) -> Vec<DataRecord> {
        let mut unique = Vec::with_capacity(batch.len());
        let mut seen = self.seen_ids.write().unwrap();

        for record in batch {
            if !seen.contains(&record.id) {
                seen.insert(record.id.clone());
                unique.push(record);
            } else {
                let mut stats = self.stats.write().unwrap();
                stats.duplicates_skipped += 1;
            }
        }

        unique
    }

    /// Get current ingestion statistics
    pub fn stats(&self) -> IngestionStats {
        self.stats.read().unwrap().clone()
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        *self.stats.write().unwrap() = IngestionStats::default();
    }
}

/// Trait for transforming records during ingestion
#[async_trait]
pub trait RecordTransformer: Send + Sync {
    /// Transform a record
    async fn transform(&self, record: DataRecord) -> Result<DataRecord>;

    /// Filter records (return false to skip)
    fn filter(&self, record: &DataRecord) -> bool {
        true
    }
}

/// Identity transformer (no-op)
pub struct IdentityTransformer;

#[async_trait]
impl RecordTransformer for IdentityTransformer {
    async fn transform(&self, record: DataRecord) -> Result<DataRecord> {
        Ok(record)
    }
}

/// Batched ingestion with transformations
pub struct BatchIngester<T: RecordTransformer> {
    ingester: DataIngester,
    transformer: T,
}

impl<T: RecordTransformer> BatchIngester<T> {
    /// Create a new batch ingester with transformer
    pub fn new(config: IngestionConfig, transformer: T) -> Self {
        Self {
            ingester: DataIngester::new(config),
            transformer,
        }
    }

    /// Ingest and transform all records
    pub async fn ingest_all<S: DataSource>(&self, source: &S) -> Result<Vec<DataRecord>> {
        let raw_records = self.ingester.ingest_all(source).await?;

        let mut transformed = Vec::with_capacity(raw_records.len());
        for record in raw_records {
            if self.transformer.filter(&record) {
                let t = self.transformer.transform(record).await?;
                transformed.push(t);
            }
        }

        Ok(transformed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = IngestionConfig::default();
        assert_eq!(config.batch_size, 1000);
        assert!(config.deduplicate);
    }

    #[test]
    fn test_ingester_creation() {
        let config = IngestionConfig::default();
        let ingester = DataIngester::new(config);
        let stats = ingester.stats();
        assert_eq!(stats.records_fetched, 0);
    }
}
