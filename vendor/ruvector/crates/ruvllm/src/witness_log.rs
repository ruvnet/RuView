//! Witness Log Index
//!
//! Audit logging with semantic indexing for postmortem analysis.
//! Every request generates a witness entry that is indexed in Ruvector
//! for semantic search over execution history.
//!
//! ## Use Cases
//!
//! - Debug failed requests by finding similar queries
//! - Analyze routing decision patterns
//! - Track quality metrics over time
//! - Identify latency bottlenecks
//!
//! ## Async Write Architecture
//!
//! The witness log uses a non-blocking async write system with:
//!
//! - **Write batching**: Batches up to 100 entries or 1 second before flushing
//! - **Background flush task**: Periodic flush every second via tokio
//! - **Backpressure handling**: Queue size limit with graceful degradation
//! - **Durability**: Optional fsync for critical writes
//!
//! ## Example
//!
//! ```rust,ignore
//! let log = WitnessLog::new("./witness", 768)?;
//!
//! // Start the background flush task
//! log.start_background_flush().await;
//!
//! // Record entries (non-blocking)
//! let entry = WitnessEntry::new(session_id, query_embedding, routing_decision);
//! log.record_async(entry).await?;
//!
//! // Force flush on shutdown
//! log.flush_async().await?;
//! ```

use crate::error::{Result, RuvLLMError};
use crate::types::{ErrorInfo, ModelSize, QualityMetrics};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use ruvector_core::types::DbOptions;
use ruvector_core::{AgenticDB, SearchQuery, VectorEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "async-runtime")]
use tokio::sync::{oneshot, Notify};
#[cfg(feature = "async-runtime")]
use tokio::time::{interval, Duration};

/// Latency breakdown for profiling
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyBreakdown {
    /// Embedding generation time (ms)
    pub embedding_ms: f32,
    /// HNSW retrieval time (ms)
    pub retrieval_ms: f32,
    /// Router decision time (ms)
    pub routing_ms: f32,
    /// Graph attention time (ms)
    pub attention_ms: f32,
    /// LLM generation time (ms)
    pub generation_ms: f32,
    /// Total end-to-end time (ms)
    pub total_ms: f32,
}

impl LatencyBreakdown {
    /// Create a new latency breakdown
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute total from components
    pub fn compute_total(&mut self) {
        self.total_ms = self.embedding_ms
            + self.retrieval_ms
            + self.routing_ms
            + self.attention_ms
            + self.generation_ms;
    }

    /// Check if any component exceeds threshold
    pub fn exceeds_threshold(&self, threshold_ms: f32) -> bool {
        self.total_ms > threshold_ms
    }

    /// Get the slowest component
    pub fn slowest_component(&self) -> (&'static str, f32) {
        let components = [
            ("embedding", self.embedding_ms),
            ("retrieval", self.retrieval_ms),
            ("routing", self.routing_ms),
            ("attention", self.attention_ms),
            ("generation", self.generation_ms),
        ];

        components
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(("unknown", 0.0))
    }
}

/// Routing decision record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    /// Selected model
    pub model: ModelSize,
    /// Context size bucket
    pub context_size: usize,
    /// Temperature used
    pub temperature: f32,
    /// Top-p used
    pub top_p: f32,
    /// Router confidence (0.0 - 1.0)
    pub confidence: f32,
    /// Model probability distribution [tiny, small, medium, large]
    pub model_probs: [f32; 4],
}

impl Default for RoutingDecision {
    fn default() -> Self {
        Self {
            model: ModelSize::Small,
            context_size: 0,
            temperature: 0.7,
            top_p: 0.9,
            confidence: 0.5,
            model_probs: [0.25, 0.25, 0.25, 0.25],
        }
    }
}

/// Execution witness log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessEntry {
    /// Unique request identifier
    pub request_id: Uuid,
    /// Associated session ID
    pub session_id: String,
    /// Query embedding for semantic search (768-D)
    pub query_embedding: Vec<f32>,
    /// Routing decision made
    pub routing_decision: RoutingDecision,
    /// Model used for generation
    pub model_used: ModelSize,
    /// Quality score (0.0 - 1.0) from evaluation
    pub quality_score: f32,
    /// End-to-end latency breakdown
    pub latency: LatencyBreakdown,
    /// Context documents retrieved
    pub context_doc_ids: Vec<Uuid>,
    /// Response embedding for clustering
    pub response_embedding: Vec<f32>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Error details if failed
    pub error: Option<ErrorInfo>,
    /// Quality metrics breakdown
    pub quality_metrics: Option<QualityMetrics>,
    /// Custom tags for filtering
    pub tags: Vec<String>,
}

impl WitnessEntry {
    /// Create a new witness entry
    pub fn new(
        session_id: String,
        query_embedding: Vec<f32>,
        routing_decision: RoutingDecision,
    ) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            session_id,
            query_embedding,
            routing_decision: routing_decision.clone(),
            model_used: routing_decision.model,
            quality_score: 0.0,
            latency: LatencyBreakdown::default(),
            context_doc_ids: Vec::new(),
            response_embedding: Vec::new(),
            timestamp: Utc::now(),
            error: None,
            quality_metrics: None,
            tags: Vec::new(),
        }
    }

    /// Set quality score
    pub fn with_quality(mut self, score: f32) -> Self {
        self.quality_score = score;
        self
    }

    /// Set latency breakdown
    pub fn with_latency(mut self, latency: LatencyBreakdown) -> Self {
        self.latency = latency;
        self
    }

    /// Set error
    pub fn with_error(mut self, error: ErrorInfo) -> Self {
        self.error = Some(error);
        self
    }

    /// Check if this was a successful request
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }

    /// Check if quality score meets threshold
    pub fn meets_quality_threshold(&self, threshold: f32) -> bool {
        self.quality_score >= threshold
    }
}

/// Configuration for async write behavior
#[derive(Debug, Clone)]
pub struct AsyncWriteConfig {
    /// Maximum batch size before forcing flush (default: 100)
    pub max_batch_size: usize,
    /// Maximum wait time before flush in milliseconds (default: 1000)
    pub max_wait_ms: u64,
    /// Maximum queue depth for backpressure (default: 10000)
    pub max_queue_depth: usize,
    /// Enable fsync on critical writes (default: false for performance)
    pub fsync_critical: bool,
    /// Background flush interval in milliseconds (default: 1000)
    pub flush_interval_ms: u64,
}

impl Default for AsyncWriteConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 100,
            max_wait_ms: 1000,
            max_queue_depth: 10000,
            fsync_critical: false,
            flush_interval_ms: 1000,
        }
    }
}

/// Write-back queue for batching writes with backpressure support
struct WritebackQueue {
    /// Pending entries
    entries: Vec<WitnessEntry>,
    /// Configuration
    config: AsyncWriteConfig,
    /// Last flush timestamp
    last_flush: DateTime<Utc>,
    /// Total entries dropped due to backpressure
    dropped_count: usize,
}

impl WritebackQueue {
    fn new(config: AsyncWriteConfig) -> Self {
        Self {
            entries: Vec::with_capacity(config.max_batch_size),
            config,
            last_flush: Utc::now(),
            dropped_count: 0,
        }
    }

    fn should_flush(&self) -> bool {
        if self.entries.len() >= self.config.max_batch_size {
            return true;
        }

        let elapsed = (Utc::now() - self.last_flush).num_milliseconds() as u64;
        elapsed >= self.config.max_wait_ms && !self.entries.is_empty()
    }

    /// Push an entry with backpressure handling
    /// Returns true if entry was accepted, false if dropped due to backpressure
    fn push(&mut self, entry: WitnessEntry) -> bool {
        if self.entries.len() >= self.config.max_queue_depth {
            self.dropped_count += 1;
            return false;
        }
        self.entries.push(entry);
        true
    }

    fn drain(&mut self) -> Vec<WitnessEntry> {
        self.last_flush = Utc::now();
        std::mem::take(&mut self.entries)
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn dropped_count(&self) -> usize {
        self.dropped_count
    }
}

/// Witness log backed by Ruvector
pub struct WitnessLog {
    /// Ruvector database
    db: AgenticDB,
    /// Embedding dimension
    embedding_dim: usize,
    /// Write-back queue for batching
    writeback_queue: Arc<Mutex<WritebackQueue>>,
    /// Total entries recorded
    total_entries: AtomicUsize,
    /// Success count
    success_count: AtomicUsize,
    /// Error count
    error_count: AtomicUsize,
    /// Async write configuration
    async_config: AsyncWriteConfig,
    /// Storage path for fsync operations
    storage_path: String,
    /// Flag to indicate if background task is running
    background_running: Arc<AtomicBool>,
    /// Notify signal for flush requests
    #[cfg(feature = "async-runtime")]
    flush_notify: Arc<Notify>,
    /// Shutdown signal sender
    #[cfg(feature = "async-runtime")]
    shutdown_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl WitnessLog {
    /// Create a new witness log with default async write configuration
    pub fn new(storage_path: &str, embedding_dim: usize) -> Result<Self> {
        Self::with_config(storage_path, embedding_dim, AsyncWriteConfig::default())
    }

    /// Create a new witness log with custom async write configuration
    pub fn with_config(
        storage_path: &str,
        embedding_dim: usize,
        async_config: AsyncWriteConfig,
    ) -> Result<Self> {
        let mut options = DbOptions::default();
        options.storage_path = storage_path.to_string();
        options.dimensions = embedding_dim;

        let db = AgenticDB::new(options).map_err(|e| RuvLLMError::Storage(e.to_string()))?;

        Ok(Self {
            db,
            embedding_dim,
            writeback_queue: Arc::new(Mutex::new(WritebackQueue::new(async_config.clone()))),
            total_entries: AtomicUsize::new(0),
            success_count: AtomicUsize::new(0),
            error_count: AtomicUsize::new(0),
            async_config,
            storage_path: storage_path.to_string(),
            background_running: Arc::new(AtomicBool::new(false)),
            #[cfg(feature = "async-runtime")]
            flush_notify: Arc::new(Notify::new()),
            #[cfg(feature = "async-runtime")]
            shutdown_tx: Arc::new(Mutex::new(None)),
        })
    }

    /// Record a witness entry (non-blocking, batched writes)
    ///
    /// This method adds the entry to a write-back queue for batched writes.
    /// Returns Ok(()) if the entry was accepted, or an error if dropped due to backpressure.
    pub fn record(&self, entry: WitnessEntry) -> Result<()> {
        // Update counters
        self.total_entries.fetch_add(1, Ordering::SeqCst);
        if entry.is_success() {
            self.success_count.fetch_add(1, Ordering::SeqCst);
        } else {
            self.error_count.fetch_add(1, Ordering::SeqCst);
        }

        // Add to writeback queue with backpressure handling
        let mut queue = self.writeback_queue.lock();
        if !queue.push(entry) {
            return Err(RuvLLMError::OutOfMemory(
                "Witness log queue full, entry dropped due to backpressure".to_string(),
            ));
        }

        // Flush if needed (synchronous fallback when background task not running)
        if !self.background_running.load(Ordering::SeqCst) && queue.should_flush() {
            let entries = queue.drain();
            drop(queue); // Release lock before writing
            self.flush_entries(entries)?;
        }

        // If background task is running, notify it
        #[cfg(feature = "async-runtime")]
        if self.background_running.load(Ordering::SeqCst) {
            self.flush_notify.notify_one();
        }

        Ok(())
    }

    /// Record a witness entry with critical durability (fsync)
    ///
    /// Use this for entries that must be persisted immediately (e.g., errors, critical events).
    /// This bypasses batching and writes directly with fsync.
    pub fn record_critical(&self, entry: WitnessEntry) -> Result<()> {
        // Update counters
        self.total_entries.fetch_add(1, Ordering::SeqCst);
        if entry.is_success() {
            self.success_count.fetch_add(1, Ordering::SeqCst);
        } else {
            self.error_count.fetch_add(1, Ordering::SeqCst);
        }

        // Write immediately
        self.flush_entries(vec![entry])?;

        // Sync to disk if configured
        if self.async_config.fsync_critical {
            self.fsync()?;
        }

        Ok(())
    }

    /// Force fsync to ensure durability
    fn fsync(&self) -> Result<()> {
        // Open the database file and sync
        // Note: redb (used by AgenticDB) handles its own durability via WAL
        // This is a best-effort sync for the witness log directory
        #[cfg(feature = "async-runtime")]
        {
            use std::fs::OpenOptions;
            if let Ok(file) = OpenOptions::new().read(true).open(&self.storage_path) {
                let _ = file.sync_all();
            }
        }
        Ok(())
    }

    /// Flush pending entries to storage
    fn flush_entries(&self, entries: Vec<WitnessEntry>) -> Result<()> {
        for entry in entries {
            let mut metadata = HashMap::new();
            metadata.insert(
                "request_id".to_string(),
                serde_json::json!(entry.request_id.to_string()),
            );
            metadata.insert(
                "session_id".to_string(),
                serde_json::json!(entry.session_id),
            );
            metadata.insert(
                "model_used".to_string(),
                serde_json::to_value(&entry.model_used).unwrap_or_default(),
            );
            metadata.insert(
                "quality_score".to_string(),
                serde_json::json!(entry.quality_score),
            );
            metadata.insert(
                "routing_decision".to_string(),
                serde_json::to_value(&entry.routing_decision).unwrap_or_default(),
            );
            metadata.insert(
                "latency".to_string(),
                serde_json::to_value(&entry.latency).unwrap_or_default(),
            );
            metadata.insert(
                "timestamp".to_string(),
                serde_json::json!(entry.timestamp.to_rfc3339()),
            );
            metadata.insert(
                "is_success".to_string(),
                serde_json::json!(entry.is_success()),
            );
            metadata.insert("tags".to_string(), serde_json::json!(entry.tags));

            if let Some(error) = &entry.error {
                metadata.insert(
                    "error".to_string(),
                    serde_json::to_value(error).unwrap_or_default(),
                );
            }

            if let Some(qm) = &entry.quality_metrics {
                metadata.insert(
                    "quality_metrics".to_string(),
                    serde_json::to_value(qm).unwrap_or_default(),
                );
            }

            let vector_entry = VectorEntry {
                id: Some(entry.request_id.to_string()),
                vector: entry.query_embedding,
                metadata: Some(metadata),
            };

            self.db
                .insert(vector_entry)
                .map_err(|e| RuvLLMError::Storage(e.to_string()))?;
        }

        Ok(())
    }

    /// Force flush all pending entries
    pub fn flush(&self) -> Result<()> {
        let mut queue = self.writeback_queue.lock();
        if !queue.entries.is_empty() {
            let entries = queue.drain();
            drop(queue);
            self.flush_entries(entries)?;
        }
        Ok(())
    }

    /// Search witness logs by semantic similarity
    pub fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<WitnessEntry>> {
        let query = SearchQuery {
            vector: query_embedding.to_vec(),
            k: limit,
            filter: None,
            ef_search: None,
        };

        let results = self
            .db
            .search(query)
            .map_err(|e| RuvLLMError::Storage(e.to_string()))?;

        let mut entries = Vec::with_capacity(results.len());
        for result in results {
            if let Some(metadata) = &result.metadata {
                if let Some(entry) = self.entry_from_metadata(&result.id, query_embedding, metadata)
                {
                    entries.push(entry);
                }
            }
        }

        Ok(entries)
    }

    /// Get statistics
    pub fn stats(&self) -> WitnessLogStats {
        let total = self.total_entries.load(Ordering::SeqCst);
        let success = self.success_count.load(Ordering::SeqCst);
        let errors = self.error_count.load(Ordering::SeqCst);
        let queue = self.writeback_queue.lock();

        WitnessLogStats {
            total_entries: total,
            success_count: success,
            error_count: errors,
            success_rate: if total > 0 {
                success as f32 / total as f32
            } else {
                0.0
            },
            pending_writes: queue.len(),
            dropped_entries: queue.dropped_count(),
            background_running: self.background_running.load(Ordering::SeqCst),
        }
    }

    /// Get the async write configuration
    pub fn async_config(&self) -> &AsyncWriteConfig {
        &self.async_config
    }

    /// Check if entries have been dropped due to backpressure
    pub fn has_dropped_entries(&self) -> bool {
        self.writeback_queue.lock().dropped_count() > 0
    }

    /// Reconstruct WitnessEntry from metadata
    fn entry_from_metadata(
        &self,
        _id: &str,
        embedding: &[f32],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Option<WitnessEntry> {
        let request_id = metadata
            .get("request_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())?;

        let session_id = metadata
            .get("session_id")
            .and_then(|v| v.as_str())?
            .to_string();

        let model_used: ModelSize = metadata
            .get("model_used")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let quality_score = metadata
            .get("quality_score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;

        let routing_decision: RoutingDecision = metadata
            .get("routing_decision")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let latency: LatencyBreakdown = metadata
            .get("latency")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let timestamp = metadata
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let error: Option<ErrorInfo> = metadata
            .get("error")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let quality_metrics: Option<QualityMetrics> = metadata
            .get("quality_metrics")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let tags: Vec<String> = metadata
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Some(WitnessEntry {
            request_id,
            session_id,
            query_embedding: embedding.to_vec(),
            routing_decision,
            model_used,
            quality_score,
            latency,
            context_doc_ids: Vec::new(),
            response_embedding: Vec::new(),
            timestamp,
            error,
            quality_metrics,
            tags,
        })
    }
}

/// Witness log statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WitnessLogStats {
    /// Total entries recorded
    pub total_entries: usize,
    /// Successful requests
    pub success_count: usize,
    /// Failed requests
    pub error_count: usize,
    /// Success rate (0.0 - 1.0)
    pub success_rate: f32,
    /// Pending writes in queue
    pub pending_writes: usize,
    /// Entries dropped due to backpressure
    pub dropped_entries: usize,
    /// Background flush task running
    pub background_running: bool,
}

// ============================================================================
// Async write support
// ============================================================================

#[cfg(feature = "async-runtime")]
impl WitnessLog {
    /// Start the background flush task
    ///
    /// This spawns a tokio task that periodically flushes the write-back queue.
    /// Call this once after creating the WitnessLog.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let log = WitnessLog::new("./witness", 768)?;
    /// log.start_background_flush();
    /// ```
    pub fn start_background_flush(self: &Arc<Self>) {
        if self.background_running.swap(true, Ordering::SeqCst) {
            // Already running
            return;
        }

        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        *self.shutdown_tx.lock() = Some(shutdown_tx);

        let log = Arc::clone(self);
        let flush_interval = Duration::from_millis(self.async_config.flush_interval_ms);

        tokio::spawn(async move {
            let mut ticker = interval(flush_interval);

            loop {
                tokio::select! {
                    // Periodic tick
                    _ = ticker.tick() => {
                        log.flush_if_needed_internal();
                    }
                    // Notified by record()
                    _ = log.flush_notify.notified() => {
                        log.flush_if_needed_internal();
                    }
                    // Shutdown signal
                    _ = &mut shutdown_rx => {
                        // Final flush before shutdown
                        if let Err(e) = log.flush() {
                            tracing::error!("Error during final witness log flush: {}", e);
                        }
                        log.background_running.store(false, Ordering::SeqCst);
                        break;
                    }
                }
            }
        });
    }

    /// Stop the background flush task
    ///
    /// This signals the background task to stop and performs a final flush.
    pub async fn stop_background_flush(&self) {
        if !self.background_running.load(Ordering::SeqCst) {
            return;
        }

        if let Some(tx) = self.shutdown_tx.lock().take() {
            let _ = tx.send(());
        }

        // Wait a bit for the task to complete
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    /// Record a witness entry asynchronously
    ///
    /// This is the preferred async method for recording entries.
    /// It handles backpressure and notifies the background flush task.
    pub async fn record_async(&self, entry: WitnessEntry) -> Result<()> {
        self.record(entry)
    }

    /// Flush all pending entries asynchronously
    ///
    /// This performs the flush in a blocking task to avoid blocking the async runtime.
    pub async fn flush_async(&self) -> Result<()> {
        let queue = Arc::clone(&self.writeback_queue);

        // Get entries to flush
        let entries = {
            let mut q = queue.lock();
            if q.is_empty() {
                return Ok(());
            }
            q.drain()
        };

        // Flush entries (this is synchronous, could be optimized with async db)
        self.flush_entries(entries)
    }

    /// Internal method to check and flush if needed
    fn flush_if_needed_internal(&self) {
        let entries = {
            let mut queue = self.writeback_queue.lock();
            if queue.should_flush() {
                queue.drain()
            } else {
                return;
            }
        };

        if let Err(e) = self.flush_entries(entries) {
            tracing::error!("Background witness log flush failed: {}", e);
        }
    }

    /// Record multiple entries in a batch
    ///
    /// This is more efficient than calling `record_async` multiple times.
    pub async fn record_batch(&self, entries: Vec<WitnessEntry>) -> Result<usize> {
        let mut accepted = 0;

        for entry in entries {
            self.total_entries.fetch_add(1, Ordering::SeqCst);
            if entry.is_success() {
                self.success_count.fetch_add(1, Ordering::SeqCst);
            } else {
                self.error_count.fetch_add(1, Ordering::SeqCst);
            }

            let mut queue = self.writeback_queue.lock();
            if queue.push(entry) {
                accepted += 1;
            }
        }

        // Notify background task
        self.flush_notify.notify_one();

        Ok(accepted)
    }

    /// Get detailed async statistics including background task state
    pub fn stats_async(&self) -> WitnessLogStats {
        let total = self.total_entries.load(Ordering::SeqCst);
        let success = self.success_count.load(Ordering::SeqCst);
        let errors = self.error_count.load(Ordering::SeqCst);
        let queue = self.writeback_queue.lock();

        WitnessLogStats {
            total_entries: total,
            success_count: success,
            error_count: errors,
            success_rate: if total > 0 {
                success as f32 / total as f32
            } else {
                0.0
            },
            pending_writes: queue.len(),
            dropped_entries: queue.dropped_count(),
            background_running: self.background_running.load(Ordering::SeqCst),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_breakdown() {
        let mut latency = LatencyBreakdown {
            embedding_ms: 10.0,
            retrieval_ms: 5.0,
            routing_ms: 2.0,
            attention_ms: 50.0,
            generation_ms: 100.0,
            total_ms: 0.0,
        };

        latency.compute_total();
        assert_eq!(latency.total_ms, 167.0);

        let (name, _) = latency.slowest_component();
        assert_eq!(name, "generation");
    }

    #[test]
    fn test_witness_entry() {
        let entry = WitnessEntry::new(
            "session-1".to_string(),
            vec![0.1; 768],
            RoutingDecision::default(),
        );

        assert!(entry.is_success());
        assert!(!entry.meets_quality_threshold(0.5));

        let entry = entry.with_quality(0.8);
        assert!(entry.meets_quality_threshold(0.5));
    }

    #[test]
    fn test_routing_decision() {
        let decision = RoutingDecision::default();
        assert_eq!(decision.model, ModelSize::Small);
        assert_eq!(decision.temperature, 0.7);
    }

    #[test]
    fn test_async_write_config_default() {
        let config = AsyncWriteConfig::default();
        assert_eq!(config.max_batch_size, 100);
        assert_eq!(config.max_wait_ms, 1000);
        assert_eq!(config.max_queue_depth, 10000);
        assert!(!config.fsync_critical);
        assert_eq!(config.flush_interval_ms, 1000);
    }

    #[test]
    fn test_writeback_queue_batching() {
        let config = AsyncWriteConfig {
            max_batch_size: 5,
            max_wait_ms: 1000,
            max_queue_depth: 100,
            fsync_critical: false,
            flush_interval_ms: 1000,
        };
        let mut queue = WritebackQueue::new(config);

        // Queue should not need flush initially
        assert!(!queue.should_flush());
        assert!(queue.is_empty());

        // Add entries
        for i in 0..4 {
            let entry = WitnessEntry::new(
                format!("session-{}", i),
                vec![0.1; 768],
                RoutingDecision::default(),
            );
            assert!(queue.push(entry));
        }

        // Queue has entries but not at batch size
        assert_eq!(queue.len(), 4);
        assert!(!queue.should_flush()); // Only 4 of 5

        // Add one more to trigger batch size
        let entry = WitnessEntry::new(
            "session-4".to_string(),
            vec![0.1; 768],
            RoutingDecision::default(),
        );
        assert!(queue.push(entry));

        // Now should flush
        assert!(queue.should_flush());

        // Drain and verify
        let entries = queue.drain();
        assert_eq!(entries.len(), 5);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_writeback_queue_backpressure() {
        let config = AsyncWriteConfig {
            max_batch_size: 5,
            max_wait_ms: 1000,
            max_queue_depth: 10, // Small queue for testing
            fsync_critical: false,
            flush_interval_ms: 1000,
        };
        let mut queue = WritebackQueue::new(config);

        // Fill up to max depth
        for i in 0..10 {
            let entry = WitnessEntry::new(
                format!("session-{}", i),
                vec![0.1; 768],
                RoutingDecision::default(),
            );
            assert!(queue.push(entry), "Entry {} should be accepted", i);
        }

        // Next entry should be dropped
        let entry = WitnessEntry::new(
            "session-overflow".to_string(),
            vec![0.1; 768],
            RoutingDecision::default(),
        );
        assert!(
            !queue.push(entry),
            "Entry should be dropped due to backpressure"
        );
        assert_eq!(queue.dropped_count(), 1);

        // Another dropped entry
        let entry2 = WitnessEntry::new(
            "session-overflow-2".to_string(),
            vec![0.1; 768],
            RoutingDecision::default(),
        );
        assert!(!queue.push(entry2));
        assert_eq!(queue.dropped_count(), 2);
    }

    #[test]
    fn test_witness_log_stats() {
        let config = AsyncWriteConfig {
            max_batch_size: 100,
            max_wait_ms: 1000,
            max_queue_depth: 5, // Small for testing backpressure
            fsync_critical: false,
            flush_interval_ms: 1000,
        };
        let temp_dir = tempfile::tempdir().unwrap();
        let storage_path = temp_dir.path().join("witness_test");

        let log = WitnessLog::with_config(storage_path.to_str().unwrap(), 64, config).unwrap();

        // Record some entries
        for i in 0..3 {
            let entry = WitnessEntry::new(
                format!("session-{}", i),
                vec![0.1; 64],
                RoutingDecision::default(),
            );
            log.record(entry).unwrap();
        }

        let stats = log.stats();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.success_count, 3);
        assert_eq!(stats.error_count, 0);
        assert!(!stats.background_running);
    }

    #[cfg(feature = "async-runtime")]
    mod async_tests {
        use super::*;
        use std::sync::Arc;

        #[tokio::test]
        async fn test_background_flush_task() {
            let config = AsyncWriteConfig {
                max_batch_size: 5,
                max_wait_ms: 100, // Short for testing
                max_queue_depth: 1000,
                fsync_critical: false,
                flush_interval_ms: 50, // Short flush interval for testing
            };
            let temp_dir = tempfile::tempdir().unwrap();
            let storage_path = temp_dir.path().join("async_witness_test");

            let log = Arc::new(
                WitnessLog::with_config(storage_path.to_str().unwrap(), 64, config).unwrap(),
            );

            // Start background flush task
            log.start_background_flush();

            // Verify it's running
            let stats = log.stats_async();
            assert!(stats.background_running);

            // Record some entries
            for i in 0..10 {
                let entry = WitnessEntry::new(
                    format!("async-session-{}", i),
                    vec![0.1; 64],
                    RoutingDecision::default(),
                );
                log.record_async(entry).await.unwrap();
            }

            // Wait for background flush
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Entries should have been flushed (pending < 10)
            let stats = log.stats_async();
            assert!(stats.pending_writes < 10);

            // Stop background task
            log.stop_background_flush().await;

            let stats = log.stats_async();
            assert!(!stats.background_running);
        }

        #[tokio::test]
        async fn test_record_batch() {
            let temp_dir = tempfile::tempdir().unwrap();
            let storage_path = temp_dir.path().join("batch_witness_test");

            let log = Arc::new(WitnessLog::new(storage_path.to_str().unwrap(), 64).unwrap());

            log.start_background_flush();

            // Create batch of entries
            let entries: Vec<_> = (0..50)
                .map(|i| {
                    WitnessEntry::new(
                        format!("batch-session-{}", i),
                        vec![0.1; 64],
                        RoutingDecision::default(),
                    )
                })
                .collect();

            // Record batch
            let accepted = log.record_batch(entries).await.unwrap();
            assert_eq!(accepted, 50);

            let stats = log.stats_async();
            assert_eq!(stats.total_entries, 50);

            log.stop_background_flush().await;
        }

        #[tokio::test]
        async fn test_flush_async() {
            let temp_dir = tempfile::tempdir().unwrap();
            let storage_path = temp_dir.path().join("flush_async_test");

            let log = WitnessLog::new(storage_path.to_str().unwrap(), 64).unwrap();

            // Record entries
            for i in 0..5 {
                let entry = WitnessEntry::new(
                    format!("flush-session-{}", i),
                    vec![0.1; 64],
                    RoutingDecision::default(),
                );
                log.record(entry).unwrap();
            }

            // Force async flush
            log.flush_async().await.unwrap();

            // All entries should be flushed
            let stats = log.stats();
            assert_eq!(stats.pending_writes, 0);
        }
    }
}
