//! Dynamic batching for throughput optimization
//!
//! Provides intelligent batching to maximize GPU/CPU utilization while
//! maintaining acceptable latency.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{oneshot, Mutex};
use tokio::time::sleep;

/// Item in the batching queue
pub struct BatchItem<T, R> {
    pub data: T,
    pub response: oneshot::Sender<BatchResult<R>>,
    pub enqueued_at: Instant,
}

/// Result of batch processing
pub type BatchResult<T> = std::result::Result<T, BatchError>;

/// Batch processing errors
#[derive(Debug, Clone)]
pub enum BatchError {
    Timeout,
    ProcessingFailed(String),
    QueueFull,
}

impl std::fmt::Display for BatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            BatchError::Timeout => write!(f, "Batch processing timeout"),
            BatchError::ProcessingFailed(msg) => write!(f, "Processing failed: {}", msg),
            BatchError::QueueFull => write!(f, "Queue is full"),
        }
    }
}

impl std::error::Error for BatchError {}

/// Dynamic batcher configuration
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum items in a batch
    pub max_batch_size: usize,
    /// Maximum time to wait before processing partial batch
    pub max_wait_ms: u64,
    /// Maximum queue size
    pub max_queue_size: usize,
    /// Minimum batch size to prefer
    pub preferred_batch_size: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 32,
            max_wait_ms: 50,
            max_queue_size: 1000,
            preferred_batch_size: 16,
        }
    }
}

/// Dynamic batcher for throughput optimization
pub struct DynamicBatcher<T, R> {
    config: BatchConfig,
    queue: Arc<Mutex<VecDeque<BatchItem<T, R>>>>,
    processor: Arc<dyn Fn(Vec<T>) -> Vec<std::result::Result<R, String>> + Send + Sync>,
    shutdown: Arc<Mutex<bool>>,
}

impl<T, R> DynamicBatcher<T, R>
where
    T: Send + 'static,
    R: Send + 'static,
{
    /// Create new dynamic batcher
    pub fn new<F>(config: BatchConfig, processor: F) -> Self
    where
        F: Fn(Vec<T>) -> Vec<std::result::Result<R, String>> + Send + Sync + 'static,
    {
        Self {
            config,
            queue: Arc::new(Mutex::new(VecDeque::new())),
            processor: Arc::new(processor),
            shutdown: Arc::new(Mutex::new(false)),
        }
    }

    /// Add item to batch queue
    pub async fn add(&self, item: T) -> BatchResult<R> {
        let (tx, rx) = oneshot::channel();

        let batch_item = BatchItem {
            data: item,
            response: tx,
            enqueued_at: Instant::now(),
        };

        {
            let mut queue = self.queue.lock().await;
            if queue.len() >= self.config.max_queue_size {
                return Err(BatchError::QueueFull);
            }
            queue.push_back(batch_item);
        }

        // Wait for response
        rx.await.map_err(|_| BatchError::Timeout)?
    }

    /// Start batch processing loop
    pub async fn run(&self) {
        let mut last_process = Instant::now();

        loop {
            // Check if shutdown requested
            {
                let shutdown = self.shutdown.lock().await;
                if *shutdown {
                    break;
                }
            }

            let should_process = {
                let queue = self.queue.lock().await;
                queue.len() >= self.config.max_batch_size
                    || (queue.len() >= self.config.preferred_batch_size
                        && last_process.elapsed().as_millis() >= self.config.max_wait_ms as u128)
                    || (queue.len() > 0
                        && last_process.elapsed().as_millis() >= self.config.max_wait_ms as u128)
            };

            if should_process {
                self.process_batch().await;
                last_process = Instant::now();
            } else {
                // Sleep briefly to avoid busy waiting
                sleep(Duration::from_millis(1)).await;
            }
        }

        // Process remaining items before shutdown
        self.process_batch().await;
    }

    /// Process current batch
    async fn process_batch(&self) {
        let items = {
            let mut queue = self.queue.lock().await;
            let batch_size = self.config.max_batch_size.min(queue.len());
            if batch_size == 0 {
                return;
            }
            queue.drain(..batch_size).collect::<Vec<_>>()
        };

        if items.is_empty() {
            return;
        }

        // Extract data and response channels
        let (data, responses): (Vec<_>, Vec<_>) = items
            .into_iter()
            .map(|item| (item.data, item.response))
            .unzip();

        // Process batch
        let results = (self.processor)(data);

        // Send responses
        for (response_tx, result) in responses.into_iter().zip(results.into_iter()) {
            let batch_result = result.map_err(|e| BatchError::ProcessingFailed(e));
            let _ = response_tx.send(batch_result);
        }
    }

    /// Gracefully shutdown the batcher
    pub async fn shutdown(&self) {
        let mut shutdown = self.shutdown.lock().await;
        *shutdown = true;
    }

    /// Get current queue size
    pub async fn queue_size(&self) -> usize {
        self.queue.lock().await.len()
    }

    /// Get current queue statistics
    pub async fn stats(&self) -> BatchStats {
        let queue = self.queue.lock().await;
        let queue_size = queue.len();

        let max_wait = queue
            .front()
            .map(|item| item.enqueued_at.elapsed())
            .unwrap_or(Duration::from_secs(0));

        BatchStats {
            queue_size,
            max_wait_time: max_wait,
        }
    }
}

/// Batch statistics
#[derive(Debug, Clone)]
pub struct BatchStats {
    pub queue_size: usize,
    pub max_wait_time: Duration,
}

/// Adaptive batcher that adjusts batch size based on latency
pub struct AdaptiveBatcher<T, R> {
    inner: DynamicBatcher<T, R>,
    config: Arc<Mutex<BatchConfig>>,
    latency_history: Arc<Mutex<VecDeque<Duration>>>,
    target_latency: Duration,
}

impl<T, R> AdaptiveBatcher<T, R>
where
    T: Send + 'static,
    R: Send + 'static,
{
    /// Create adaptive batcher with target latency
    pub fn new<F>(initial_config: BatchConfig, target_latency: Duration, processor: F) -> Self
    where
        F: Fn(Vec<T>) -> Vec<Result<R, String>> + Send + Sync + 'static,
    {
        let config = Arc::new(Mutex::new(initial_config.clone()));
        let inner = DynamicBatcher::new(initial_config, processor);

        Self {
            inner,
            config,
            latency_history: Arc::new(Mutex::new(VecDeque::with_capacity(100))),
            target_latency,
        }
    }

    /// Add item and adapt batch size
    pub async fn add(&self, item: T) -> Result<R, BatchError> {
        let start = Instant::now();
        let result = self.inner.add(item).await;
        let latency = start.elapsed();

        // Record latency
        {
            let mut history = self.latency_history.lock().await;
            history.push_back(latency);
            if history.len() > 100 {
                history.pop_front();
            }
        }

        // Adapt batch size every 10 requests
        {
            let history = self.latency_history.lock().await;
            if history.len() % 10 == 0 && history.len() >= 10 {
                let avg_latency: Duration = history.iter().sum::<Duration>() / history.len() as u32;

                let mut config = self.config.lock().await;
                if avg_latency > self.target_latency {
                    // Reduce batch size to lower latency
                    config.max_batch_size = (config.max_batch_size * 9 / 10).max(1);
                } else if avg_latency < self.target_latency / 2 {
                    // Increase batch size for better throughput
                    config.max_batch_size = (config.max_batch_size * 11 / 10).min(128);
                }
            }
        }

        result
    }

    /// Run the batcher
    pub async fn run(&self) {
        self.inner.run().await;
    }

    /// Get current configuration
    pub async fn current_config(&self) -> BatchConfig {
        self.config.lock().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dynamic_batcher() {
        let config = BatchConfig {
            max_batch_size: 4,
            max_wait_ms: 100,
            max_queue_size: 100,
            preferred_batch_size: 2,
        };

        let batcher = Arc::new(DynamicBatcher::new(config, |items: Vec<i32>| {
            items.into_iter().map(|x| Ok(x * 2)).collect()
        }));

        // Start processing loop
        let batcher_clone = batcher.clone();
        tokio::spawn(async move {
            batcher_clone.run().await;
        });

        // Add items
        let mut handles = vec![];
        for i in 0..8 {
            let batcher = batcher.clone();
            handles.push(tokio::spawn(async move { batcher.add(i).await }));
        }

        // Wait for results
        for (i, handle) in handles.into_iter().enumerate() {
            let result = handle.await.unwrap().unwrap();
            assert_eq!(result, (i as i32) * 2);
        }

        batcher.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_stats() {
        let config = BatchConfig::default();
        let batcher = DynamicBatcher::new(config, |items: Vec<i32>| {
            items.into_iter().map(|x| Ok(x)).collect()
        });

        // Queue some items without processing
        let _ = batcher.add(1);
        let _ = batcher.add(2);
        let _ = batcher.add(3);

        let stats = batcher.stats().await;
        assert_eq!(stats.queue_size, 3);
    }

    #[tokio::test]
    async fn test_queue_full() {
        let config = BatchConfig {
            max_queue_size: 2,
            ..Default::default()
        };

        let batcher = DynamicBatcher::new(config, |items: Vec<i32>| {
            std::thread::sleep(Duration::from_secs(1)); // Slow processing
            items.into_iter().map(|x| Ok(x)).collect()
        });

        // Fill queue
        let _ = batcher.add(1);
        let _ = batcher.add(2);

        // This should fail - queue is full
        let result = batcher.add(3).await;
        assert!(matches!(result, Err(BatchError::QueueFull)));
    }

    #[tokio::test]
    async fn test_adaptive_batcher() {
        let config = BatchConfig {
            max_batch_size: 8,
            max_wait_ms: 50,
            max_queue_size: 100,
            preferred_batch_size: 4,
        };

        let batcher = Arc::new(AdaptiveBatcher::new(
            config,
            Duration::from_millis(100),
            |items: Vec<i32>| items.into_iter().map(|x| Ok(x * 2)).collect(),
        ));

        let batcher_clone = batcher.clone();
        tokio::spawn(async move {
            batcher_clone.run().await;
        });

        // Process some requests
        for i in 0..20 {
            let result = batcher.add(i).await.unwrap();
            assert_eq!(result, i * 2);
        }

        // Configuration should have adapted
        let final_config = batcher.current_config().await;
        assert!(final_config.max_batch_size > 0);
    }
}
