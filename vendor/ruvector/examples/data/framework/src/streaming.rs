//! Real-time Streaming Data Ingestion
//!
//! Provides async stream processing with windowed analysis, real-time pattern
//! detection, backpressure handling, and comprehensive metrics collection.
//!
//! ## Features
//! - Async stream processing for continuous data ingestion
//! - Tumbling and sliding window analysis
//! - Real-time pattern detection with callbacks
//! - Automatic backpressure handling
//! - Throughput and latency metrics
//!
//! ## Example
//! ```rust,ignore
//! use futures::stream;
//! use std::time::Duration;
//!
//! let config = StreamingConfig {
//!     window_size: Duration::from_secs(60),
//!     slide_interval: Duration::from_secs(30),
//!     max_buffer_size: 10000,
//!     ..Default::default()
//! };
//!
//! let mut engine = StreamingEngine::new(config);
//!
//! // Set pattern callback
//! engine.set_pattern_callback(|pattern| {
//!     println!("Pattern detected: {:?}", pattern);
//! });
//!
//! // Ingest stream
//! let stream = stream::iter(vectors);
//! engine.ingest_stream(stream).await?;
//!
//! // Get metrics
//! let metrics = engine.metrics();
//! println!("Processed: {} vectors, {} patterns",
//!          metrics.vectors_processed, metrics.patterns_detected);
//! ```

use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, Semaphore};

use crate::optimized::{OptimizedConfig, OptimizedDiscoveryEngine, SignificantPattern};
use crate::ruvector_native::SemanticVector;
use crate::Result;

/// Configuration for the streaming engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    /// Discovery engine configuration
    pub discovery_config: OptimizedConfig,

    /// Window size for temporal analysis
    pub window_size: StdDuration,

    /// Slide interval for sliding windows (if None, use tumbling windows)
    pub slide_interval: Option<StdDuration>,

    /// Maximum buffer size before applying backpressure
    pub max_buffer_size: usize,

    /// Timeout for processing a single vector (None = no timeout)
    pub processing_timeout: Option<StdDuration>,

    /// Batch size for parallel processing
    pub batch_size: usize,

    /// Enable automatic pattern detection
    pub auto_detect_patterns: bool,

    /// Pattern detection interval (check every N vectors)
    pub detection_interval: usize,

    /// Maximum concurrent processing tasks
    pub max_concurrency: usize,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            discovery_config: OptimizedConfig::default(),
            window_size: StdDuration::from_secs(60),
            slide_interval: Some(StdDuration::from_secs(30)),
            max_buffer_size: 10000,
            processing_timeout: Some(StdDuration::from_secs(5)),
            batch_size: 100,
            auto_detect_patterns: true,
            detection_interval: 100,
            max_concurrency: 4,
        }
    }
}

/// Streaming metrics for monitoring performance
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamingMetrics {
    /// Total vectors processed
    pub vectors_processed: u64,

    /// Total patterns detected
    pub patterns_detected: u64,

    /// Average latency in milliseconds
    pub avg_latency_ms: f64,

    /// Throughput (vectors per second)
    pub throughput_per_sec: f64,

    /// Current window count
    pub windows_processed: u64,

    /// Total bytes processed (if available)
    pub bytes_processed: u64,

    /// Backpressure events (times buffer was full)
    pub backpressure_events: u64,

    /// Processing errors
    pub errors: u64,

    /// Peak vectors in buffer
    pub peak_buffer_size: usize,

    /// Start time
    pub start_time: Option<DateTime<Utc>>,

    /// Last update time
    pub last_update: Option<DateTime<Utc>>,
}

impl StreamingMetrics {
    /// Calculate uptime in seconds
    pub fn uptime_secs(&self) -> f64 {
        if let (Some(start), Some(last)) = (self.start_time, self.last_update) {
            (last - start).num_milliseconds() as f64 / 1000.0
        } else {
            0.0
        }
    }

    /// Calculate average throughput
    pub fn calculate_throughput(&mut self) {
        let uptime = self.uptime_secs();
        if uptime > 0.0 {
            self.throughput_per_sec = self.vectors_processed as f64 / uptime;
        }
    }
}

/// Time window for analysis
#[derive(Debug, Clone)]
struct TimeWindow {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    vectors: Vec<SemanticVector>,
}

impl TimeWindow {
    fn new(start: DateTime<Utc>, duration: ChronoDuration) -> Self {
        Self {
            start,
            end: start + duration,
            vectors: Vec::new(),
        }
    }

    fn contains(&self, timestamp: DateTime<Utc>) -> bool {
        timestamp >= self.start && timestamp < self.end
    }

    fn add_vector(&mut self, vector: SemanticVector) {
        self.vectors.push(vector);
    }

    fn is_complete(&self, now: DateTime<Utc>) -> bool {
        now >= self.end
    }
}

/// Streaming engine for real-time data ingestion and pattern detection
pub struct StreamingEngine {
    /// Configuration
    config: StreamingConfig,

    /// Underlying discovery engine (wrapped in Arc<RwLock> for async access)
    engine: Arc<RwLock<OptimizedDiscoveryEngine>>,

    /// Pattern callback
    on_pattern: Arc<RwLock<Option<Box<dyn Fn(SignificantPattern) + Send + Sync>>>>,

    /// Metrics
    metrics: Arc<RwLock<StreamingMetrics>>,

    /// Current windows (for sliding window analysis)
    windows: Arc<RwLock<Vec<TimeWindow>>>,

    /// Backpressure semaphore
    semaphore: Arc<Semaphore>,

    /// Latency tracking
    latencies: Arc<RwLock<Vec<f64>>>,
}

impl StreamingEngine {
    /// Create a new streaming engine
    pub fn new(config: StreamingConfig) -> Self {
        let discovery_config = config.discovery_config.clone();
        let max_buffer = config.max_buffer_size;

        let mut metrics = StreamingMetrics::default();
        metrics.start_time = Some(Utc::now());

        Self {
            config,
            engine: Arc::new(RwLock::new(OptimizedDiscoveryEngine::new(discovery_config))),
            on_pattern: Arc::new(RwLock::new(None)),
            metrics: Arc::new(RwLock::new(metrics)),
            windows: Arc::new(RwLock::new(Vec::new())),
            semaphore: Arc::new(Semaphore::new(max_buffer)),
            latencies: Arc::new(RwLock::new(Vec::with_capacity(1000))),
        }
    }

    /// Set the pattern detection callback
    pub async fn set_pattern_callback<F>(&mut self, callback: F)
    where
        F: Fn(SignificantPattern) + Send + Sync + 'static,
    {
        let mut on_pattern = self.on_pattern.write().await;
        *on_pattern = Some(Box::new(callback));
    }

    /// Ingest a stream of vectors with windowed analysis
    pub async fn ingest_stream<S>(&mut self, stream: S) -> Result<()>
    where
        S: Stream<Item = SemanticVector> + Send,
    {
        let mut stream = Box::pin(stream);
        let mut vector_count = 0_u64;
        let mut current_batch = Vec::with_capacity(self.config.batch_size);

        // Initialize first window
        let window_duration = ChronoDuration::from_std(self.config.window_size)
            .map_err(|e| crate::FrameworkError::Config(format!("Invalid window size: {}", e)))?;

        let mut last_window_start = Utc::now();
        self.create_window(last_window_start, window_duration).await;

        while let Some(vector) = stream.next().await {
            // Backpressure handling
            let _permit = self.semaphore.acquire().await.map_err(|e| {
                crate::FrameworkError::Ingestion(format!("Backpressure semaphore error: {}", e))
            })?;

            let start = Instant::now();

            // Check if we need to create a new window (sliding)
            if let Some(slide_interval) = self.config.slide_interval {
                let slide_duration = ChronoDuration::from_std(slide_interval)
                    .map_err(|e| crate::FrameworkError::Config(format!("Invalid slide interval: {}", e)))?;

                let now = Utc::now();
                if (now - last_window_start) >= slide_duration {
                    self.create_window(now, window_duration).await;
                    last_window_start = now;
                }
            }

            // Add vector to appropriate windows
            self.add_to_windows(vector.clone()).await;
            current_batch.push(vector);
            vector_count += 1;

            // Process batch
            if current_batch.len() >= self.config.batch_size {
                self.process_batch(&current_batch).await?;
                current_batch.clear();
            }

            // Pattern detection
            if self.config.auto_detect_patterns && vector_count % self.config.detection_interval as u64 == 0 {
                self.detect_patterns().await?;
            }

            // Close completed windows
            self.close_completed_windows().await?;

            // Record latency
            let latency_ms = start.elapsed().as_micros() as f64 / 1000.0;
            self.record_latency(latency_ms).await;

            // Update metrics
            let mut metrics = self.metrics.write().await;
            metrics.vectors_processed = vector_count;
            metrics.last_update = Some(Utc::now());
        }

        // Process remaining batch
        if !current_batch.is_empty() {
            self.process_batch(&current_batch).await?;
        }

        // Final pattern detection
        if self.config.auto_detect_patterns {
            self.detect_patterns().await?;
        }

        // Close all remaining windows
        self.close_all_windows().await?;

        // Calculate final metrics
        let mut metrics = self.metrics.write().await;
        metrics.calculate_throughput();

        Ok(())
    }

    /// Process a batch of vectors in parallel
    async fn process_batch(&self, vectors: &[SemanticVector]) -> Result<()> {
        let batch_size = self.config.batch_size;
        let chunks: Vec<_> = vectors.chunks(batch_size).collect();

        // Process chunks with controlled concurrency
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrency));
        let mut tasks = Vec::new();

        for chunk in chunks {
            let chunk_vec = chunk.to_vec();
            let engine = self.engine.clone();
            let sem = semaphore.clone();

            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.ok()?;
                let mut engine_guard = engine.write().await;

                #[cfg(feature = "parallel")]
                {
                    engine_guard.add_vectors_batch(chunk_vec);
                }

                #[cfg(not(feature = "parallel"))]
                {
                    for vector in chunk_vec {
                        engine_guard.add_vector(vector);
                    }
                }

                Some(())
            });

            tasks.push(task);
        }

        // Wait for all tasks to complete
        for task in tasks {
            if let Err(e) = task.await {
                tracing::warn!("Batch processing task failed: {}", e);
                let mut metrics = self.metrics.write().await;
                metrics.errors += 1;
            }
        }

        Ok(())
    }

    /// Create a new time window
    async fn create_window(&self, start: DateTime<Utc>, duration: ChronoDuration) {
        let window = TimeWindow::new(start, duration);
        let mut windows = self.windows.write().await;
        windows.push(window);
    }

    /// Add vector to all active windows
    async fn add_to_windows(&self, vector: SemanticVector) {
        let timestamp = vector.timestamp;
        let mut windows = self.windows.write().await;

        for window in windows.iter_mut() {
            if window.contains(timestamp) {
                window.add_vector(vector.clone());
            }
        }
    }

    /// Close completed windows and analyze them
    async fn close_completed_windows(&self) -> Result<()> {
        let now = Utc::now();
        let mut windows = self.windows.write().await;

        // Find completed windows
        let (completed, active): (Vec<_>, Vec<_>) = windows
            .drain(..)
            .partition(|w| w.is_complete(now));

        *windows = active;
        drop(windows); // Release lock before processing

        // Process completed windows
        for window in completed {
            self.process_window(window).await?;

            let mut metrics = self.metrics.write().await;
            metrics.windows_processed += 1;
        }

        Ok(())
    }

    /// Close all remaining windows
    async fn close_all_windows(&self) -> Result<()> {
        let mut windows = self.windows.write().await;
        let all_windows: Vec<_> = windows.drain(..).collect();
        drop(windows);

        for window in all_windows {
            self.process_window(window).await?;
        }

        Ok(())
    }

    /// Process a completed window
    async fn process_window(&self, window: TimeWindow) -> Result<()> {
        if window.vectors.is_empty() {
            return Ok(());
        }

        tracing::debug!(
            "Processing window: {} vectors from {} to {}",
            window.vectors.len(),
            window.start,
            window.end
        );

        // Add vectors to engine
        self.process_batch(&window.vectors).await?;

        // Detect patterns for this window
        if self.config.auto_detect_patterns {
            self.detect_patterns().await?;
        }

        Ok(())
    }

    /// Detect patterns and trigger callbacks
    async fn detect_patterns(&self) -> Result<()> {
        let patterns = {
            let mut engine = self.engine.write().await;
            engine.detect_patterns_with_significance()
        };

        let pattern_count = patterns.len();

        // Trigger callback for each significant pattern
        let on_pattern = self.on_pattern.read().await;
        if let Some(callback) = on_pattern.as_ref() {
            for pattern in patterns {
                if pattern.is_significant {
                    callback(pattern);
                }
            }
        }

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.patterns_detected += pattern_count as u64;

        Ok(())
    }

    /// Record latency measurement
    async fn record_latency(&self, latency_ms: f64) {
        let mut latencies = self.latencies.write().await;
        latencies.push(latency_ms);

        // Keep only last 1000 measurements
        let len = latencies.len();
        if len > 1000 {
            latencies.drain(0..len - 1000);
        }

        // Update average latency
        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let mut metrics = self.metrics.write().await;
        metrics.avg_latency_ms = avg;
    }

    /// Get current metrics
    pub async fn metrics(&self) -> StreamingMetrics {
        let mut metrics = self.metrics.read().await.clone();
        metrics.calculate_throughput();
        metrics
    }

    /// Get engine statistics
    pub async fn engine_stats(&self) -> crate::optimized::OptimizedStats {
        let engine = self.engine.read().await;
        engine.stats()
    }

    /// Reset metrics
    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = StreamingMetrics::default();
        metrics.start_time = Some(Utc::now());

        let mut latencies = self.latencies.write().await;
        latencies.clear();
    }
}

/// Builder for StreamingEngine with fluent API
pub struct StreamingEngineBuilder {
    config: StreamingConfig,
}

impl StreamingEngineBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: StreamingConfig::default(),
        }
    }

    /// Set window size
    pub fn window_size(mut self, duration: StdDuration) -> Self {
        self.config.window_size = duration;
        self
    }

    /// Set slide interval (for sliding windows)
    pub fn slide_interval(mut self, duration: StdDuration) -> Self {
        self.config.slide_interval = Some(duration);
        self
    }

    /// Use tumbling windows (no overlap)
    pub fn tumbling_windows(mut self) -> Self {
        self.config.slide_interval = None;
        self
    }

    /// Set max buffer size
    pub fn max_buffer_size(mut self, size: usize) -> Self {
        self.config.max_buffer_size = size;
        self
    }

    /// Set batch size
    pub fn batch_size(mut self, size: usize) -> Self {
        self.config.batch_size = size;
        self
    }

    /// Set max concurrency
    pub fn max_concurrency(mut self, concurrency: usize) -> Self {
        self.config.max_concurrency = concurrency;
        self
    }

    /// Set detection interval
    pub fn detection_interval(mut self, interval: usize) -> Self {
        self.config.detection_interval = interval;
        self
    }

    /// Set discovery config
    pub fn discovery_config(mut self, config: OptimizedConfig) -> Self {
        self.config.discovery_config = config;
        self
    }

    /// Build the streaming engine
    pub fn build(self) -> StreamingEngine {
        StreamingEngine::new(self.config)
    }
}

impl Default for StreamingEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use crate::ruvector_native::Domain;
    use std::collections::HashMap;

    fn create_test_vector(id: &str, domain: Domain) -> SemanticVector {
        SemanticVector {
            id: id.to_string(),
            embedding: vec![0.1, 0.2, 0.3, 0.4],
            domain,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_streaming_engine_creation() {
        let config = StreamingConfig::default();
        let engine = StreamingEngine::new(config);
        let metrics = engine.metrics().await;

        assert_eq!(metrics.vectors_processed, 0);
        assert_eq!(metrics.patterns_detected, 0);
    }

    #[tokio::test]
    async fn test_pattern_callback() {
        let config = StreamingConfig {
            auto_detect_patterns: true,
            detection_interval: 2,
            ..Default::default()
        };

        let mut engine = StreamingEngine::new(config);

        let pattern_count = Arc::new(RwLock::new(0_u64));
        let pc = pattern_count.clone();

        engine.set_pattern_callback(move |_pattern| {
            let pc = pc.clone();
            tokio::spawn(async move {
                let mut count = pc.write().await;
                *count += 1;
            });
        }).await;

        // Create a stream of vectors
        let vectors = vec![
            create_test_vector("v1", Domain::Climate),
            create_test_vector("v2", Domain::Climate),
            create_test_vector("v3", Domain::Finance),
        ];

        let vector_stream = stream::iter(vectors);
        engine.ingest_stream(vector_stream).await.unwrap();

        let metrics = engine.metrics().await;
        assert!(metrics.vectors_processed >= 3);
    }

    #[tokio::test]
    async fn test_windowed_processing() {
        let config = StreamingConfig {
            window_size: StdDuration::from_millis(100),
            slide_interval: Some(StdDuration::from_millis(50)),
            auto_detect_patterns: false,
            ..Default::default()
        };

        let mut engine = StreamingEngine::new(config);

        let vectors = vec![
            create_test_vector("v1", Domain::Climate),
            create_test_vector("v2", Domain::Climate),
        ];

        let vector_stream = stream::iter(vectors);
        engine.ingest_stream(vector_stream).await.unwrap();

        let metrics = engine.metrics().await;
        assert_eq!(metrics.vectors_processed, 2);
    }

    #[tokio::test]
    async fn test_builder() {
        let engine = StreamingEngineBuilder::new()
            .window_size(StdDuration::from_secs(30))
            .slide_interval(StdDuration::from_secs(15))
            .max_buffer_size(5000)
            .batch_size(50)
            .build();

        let metrics = engine.metrics().await;
        assert_eq!(metrics.vectors_processed, 0);
    }

    #[tokio::test]
    async fn test_metrics_calculation() {
        let mut metrics = StreamingMetrics {
            vectors_processed: 1000,
            start_time: Some(Utc::now() - ChronoDuration::seconds(10)),
            last_update: Some(Utc::now()),
            ..Default::default()
        };

        metrics.calculate_throughput();
        assert!(metrics.throughput_per_sec > 0.0);
        assert!(metrics.uptime_secs() >= 9.0 && metrics.uptime_secs() <= 11.0);
    }
}
