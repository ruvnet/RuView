//! Inference Metrics for Optimization Decisions
//!
//! This module provides comprehensive metrics collection for LLM inference,
//! enabling data-driven optimization decisions.
//!
//! ## Tracked Metrics
//!
//! - **TTFT (Time to First Token)**: Latency until first token generation
//! - **TPS (Tokens Per Second)**: Generation throughput
//! - **KV Cache Hit Rate**: Cache efficiency metric
//! - **Memory Usage**: Current memory consumption
//! - **Request Statistics**: Active requests, queue depth

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Moving average calculator with configurable window
#[derive(Debug)]
pub struct MovingAverage {
    /// Circular buffer of values
    values: RwLock<VecDeque<f32>>,
    /// Window size
    window_size: usize,
    /// Running sum for O(1) average calculation
    running_sum: RwLock<f32>,
}

impl MovingAverage {
    /// Create a new moving average calculator
    pub fn new(window_size: usize) -> Self {
        Self {
            values: RwLock::new(VecDeque::with_capacity(window_size)),
            window_size,
            running_sum: RwLock::new(0.0),
        }
    }

    /// Add a value to the moving average
    pub fn add(&self, value: f32) {
        let mut values = self.values.write();
        let mut sum = self.running_sum.write();

        // Remove oldest if at capacity
        if values.len() >= self.window_size {
            if let Some(old) = values.pop_front() {
                *sum -= old;
            }
        }

        values.push_back(value);
        *sum += value;
    }

    /// Get the current average
    pub fn average(&self) -> f32 {
        let values = self.values.read();
        let sum = self.running_sum.read();

        if values.is_empty() {
            0.0
        } else {
            *sum / values.len() as f32
        }
    }

    /// Get the minimum value in the window
    pub fn min(&self) -> f32 {
        let values = self.values.read();
        values.iter().cloned().fold(f32::INFINITY, f32::min)
    }

    /// Get the maximum value in the window
    pub fn max(&self) -> f32 {
        let values = self.values.read();
        values.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
    }

    /// Get the standard deviation
    pub fn std_dev(&self) -> f32 {
        let values = self.values.read();
        if values.len() < 2 {
            return 0.0;
        }

        let mean = self.average();
        let variance: f32 =
            values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / (values.len() - 1) as f32;

        variance.sqrt()
    }

    /// Get the current window size (number of samples)
    pub fn count(&self) -> usize {
        self.values.read().len()
    }

    /// Clear all values
    pub fn clear(&self) {
        let mut values = self.values.write();
        let mut sum = self.running_sum.write();
        values.clear();
        *sum = 0.0;
    }

    /// Get percentile value (0-100)
    pub fn percentile(&self, p: f32) -> f32 {
        let values = self.values.read();
        if values.is_empty() {
            return 0.0;
        }

        let mut sorted: Vec<f32> = values.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let idx = ((p / 100.0) * (sorted.len() - 1) as f32).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}

impl Default for MovingAverage {
    fn default() -> Self {
        Self::new(100)
    }
}

impl Clone for MovingAverage {
    fn clone(&self) -> Self {
        let values = self.values.read();
        let sum = self.running_sum.read();

        Self {
            values: RwLock::new(values.clone()),
            window_size: self.window_size,
            running_sum: RwLock::new(*sum),
        }
    }
}

/// Latency histogram for distribution analysis
#[derive(Debug)]
pub struct LatencyHistogram {
    /// Bucket boundaries in milliseconds
    buckets: Vec<f32>,
    /// Counts per bucket
    counts: Vec<AtomicU64>,
    /// Total count
    total: AtomicU64,
    /// Sum for mean calculation
    sum: RwLock<f64>,
}

impl LatencyHistogram {
    /// Create a new histogram with default buckets
    pub fn new() -> Self {
        Self::with_buckets(vec![
            1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0,
        ])
    }

    /// Create a histogram with custom bucket boundaries
    pub fn with_buckets(buckets: Vec<f32>) -> Self {
        let counts = buckets.iter().map(|_| AtomicU64::new(0)).collect();
        Self {
            buckets,
            counts,
            total: AtomicU64::new(0),
            sum: RwLock::new(0.0),
        }
    }

    /// Record a latency value in milliseconds
    pub fn record(&self, latency_ms: f32) {
        // Find the appropriate bucket
        let bucket_idx = self
            .buckets
            .iter()
            .position(|&b| latency_ms <= b)
            .unwrap_or(self.buckets.len() - 1);

        self.counts[bucket_idx].fetch_add(1, Ordering::Relaxed);
        self.total.fetch_add(1, Ordering::Relaxed);

        let mut sum = self.sum.write();
        *sum += latency_ms as f64;
    }

    /// Get the mean latency
    pub fn mean(&self) -> f32 {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let sum = self.sum.read();
        (*sum / total as f64) as f32
    }

    /// Get approximate percentile (linear interpolation between buckets)
    pub fn percentile(&self, p: f32) -> f32 {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }

        let target = (p / 100.0 * total as f32) as u64;
        let mut cumulative = 0u64;

        for (i, count) in self.counts.iter().enumerate() {
            let bucket_count = count.load(Ordering::Relaxed);
            cumulative += bucket_count;

            if cumulative >= target {
                // Found the bucket containing the percentile
                if i == 0 {
                    return self.buckets[0];
                }
                // Linear interpolation
                let prev_cumulative = cumulative - bucket_count;
                let fraction = (target - prev_cumulative) as f32 / bucket_count.max(1) as f32;
                let prev_bucket = if i > 0 { self.buckets[i - 1] } else { 0.0 };
                return prev_bucket + fraction * (self.buckets[i] - prev_bucket);
            }
        }

        *self.buckets.last().unwrap_or(&0.0)
    }

    /// Get bucket counts for visualization
    pub fn bucket_counts(&self) -> Vec<(f32, u64)> {
        self.buckets
            .iter()
            .zip(self.counts.iter())
            .map(|(b, c)| (*b, c.load(Ordering::Relaxed)))
            .collect()
    }

    /// Reset all counts
    pub fn reset(&self) {
        for count in &self.counts {
            count.store(0, Ordering::Relaxed);
        }
        self.total.store(0, Ordering::Relaxed);
        *self.sum.write() = 0.0;
    }

    /// Get total count
    pub fn count(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for LatencyHistogram {
    fn clone(&self) -> Self {
        let counts: Vec<AtomicU64> = self
            .counts
            .iter()
            .map(|c| AtomicU64::new(c.load(Ordering::Relaxed)))
            .collect();
        let sum = *self.sum.read();

        Self {
            buckets: self.buckets.clone(),
            counts,
            total: AtomicU64::new(self.total.load(Ordering::Relaxed)),
            sum: RwLock::new(sum),
        }
    }
}

/// Comprehensive inference metrics
#[derive(Debug)]
pub struct InferenceMetrics {
    /// Time to first token (milliseconds)
    pub ttft_ms: MovingAverage,
    /// Tokens per second throughput
    pub tps: MovingAverage,
    /// KV cache hit rate (0.0 - 1.0)
    kv_cache_hits: AtomicU64,
    kv_cache_misses: AtomicU64,
    /// Memory usage in bytes
    memory_usage_bytes: AtomicUsize,
    /// Peak memory usage
    peak_memory_bytes: AtomicUsize,
    /// Active request count
    active_requests: AtomicUsize,
    /// Total requests processed
    total_requests: AtomicU64,
    /// Total tokens generated
    total_tokens: AtomicU64,
    /// Request latency histogram
    pub latency_histogram: LatencyHistogram,
    /// Queue depth for pending requests
    queue_depth: AtomicUsize,
    /// Start time for uptime calculation
    start_time: Instant,
    /// Last update time
    last_update: RwLock<Instant>,
    /// Inter-token latency
    pub inter_token_latency_ms: MovingAverage,
    /// Batch size history
    pub batch_sizes: MovingAverage,
}

impl InferenceMetrics {
    /// Create new inference metrics
    pub fn new() -> Self {
        Self {
            ttft_ms: MovingAverage::new(100),
            tps: MovingAverage::new(100),
            kv_cache_hits: AtomicU64::new(0),
            kv_cache_misses: AtomicU64::new(0),
            memory_usage_bytes: AtomicUsize::new(0),
            peak_memory_bytes: AtomicUsize::new(0),
            active_requests: AtomicUsize::new(0),
            total_requests: AtomicU64::new(0),
            total_tokens: AtomicU64::new(0),
            latency_histogram: LatencyHistogram::new(),
            queue_depth: AtomicUsize::new(0),
            start_time: Instant::now(),
            last_update: RwLock::new(Instant::now()),
            inter_token_latency_ms: MovingAverage::new(100),
            batch_sizes: MovingAverage::new(50),
        }
    }

    /// Record time to first token
    pub fn record_ttft(&self, ttft_ms: f32) {
        self.ttft_ms.add(ttft_ms);
        self.latency_histogram.record(ttft_ms);
        *self.last_update.write() = Instant::now();
    }

    /// Record tokens per second for a generation
    pub fn record_tps(&self, tokens: usize, duration: Duration) {
        if duration.as_secs_f32() > 0.0 {
            let tps = tokens as f32 / duration.as_secs_f32();
            self.tps.add(tps);
        }
        self.total_tokens
            .fetch_add(tokens as u64, Ordering::Relaxed);
        *self.last_update.write() = Instant::now();
    }

    /// Record inter-token latency
    pub fn record_inter_token_latency(&self, latency_ms: f32) {
        self.inter_token_latency_ms.add(latency_ms);
    }

    /// Record batch size
    pub fn record_batch_size(&self, size: usize) {
        self.batch_sizes.add(size as f32);
    }

    /// Record KV cache hit
    pub fn record_kv_cache_hit(&self) {
        self.kv_cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record KV cache miss
    pub fn record_kv_cache_miss(&self) {
        self.kv_cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current KV cache hit rate
    pub fn kv_cache_hit_rate(&self) -> f32 {
        let hits = self.kv_cache_hits.load(Ordering::Relaxed);
        let misses = self.kv_cache_misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            1.0 // No accesses yet, assume perfect
        } else {
            hits as f32 / total as f32
        }
    }

    /// Update memory usage
    pub fn update_memory_usage(&self, bytes: usize) {
        self.memory_usage_bytes.store(bytes, Ordering::Relaxed);

        // Update peak if necessary
        let current_peak = self.peak_memory_bytes.load(Ordering::Relaxed);
        if bytes > current_peak {
            self.peak_memory_bytes.store(bytes, Ordering::Relaxed);
        }
    }

    /// Get current memory usage
    pub fn memory_usage_bytes(&self) -> usize {
        self.memory_usage_bytes.load(Ordering::Relaxed)
    }

    /// Get peak memory usage
    pub fn peak_memory_bytes(&self) -> usize {
        self.peak_memory_bytes.load(Ordering::Relaxed)
    }

    /// Increment active requests
    pub fn request_started(&self) {
        self.active_requests.fetch_add(1, Ordering::Relaxed);
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active requests
    pub fn request_completed(&self) {
        self.active_requests.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get active request count
    pub fn active_requests(&self) -> usize {
        self.active_requests.load(Ordering::Relaxed)
    }

    /// Get total requests
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    /// Get total tokens generated
    pub fn total_tokens(&self) -> u64 {
        self.total_tokens.load(Ordering::Relaxed)
    }

    /// Update queue depth
    pub fn set_queue_depth(&self, depth: usize) {
        self.queue_depth.store(depth, Ordering::Relaxed);
    }

    /// Get queue depth
    pub fn queue_depth(&self) -> usize {
        self.queue_depth.load(Ordering::Relaxed)
    }

    /// Get uptime duration
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get time since last update
    pub fn time_since_update(&self) -> Duration {
        self.last_update.read().elapsed()
    }

    /// Take a snapshot of current metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            ttft_avg_ms: self.ttft_ms.average(),
            ttft_p50_ms: self.ttft_ms.percentile(50.0),
            ttft_p95_ms: self.ttft_ms.percentile(95.0),
            ttft_p99_ms: self.ttft_ms.percentile(99.0),
            tps_avg: self.tps.average(),
            tps_min: self.tps.min(),
            tps_max: self.tps.max(),
            kv_cache_hit_rate: self.kv_cache_hit_rate(),
            memory_usage_bytes: self.memory_usage_bytes(),
            peak_memory_bytes: self.peak_memory_bytes(),
            active_requests: self.active_requests(),
            total_requests: self.total_requests(),
            total_tokens: self.total_tokens(),
            queue_depth: self.queue_depth(),
            uptime_secs: self.uptime().as_secs_f32(),
            inter_token_latency_avg_ms: self.inter_token_latency_ms.average(),
            avg_batch_size: self.batch_sizes.average(),
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.ttft_ms.clear();
        self.tps.clear();
        self.kv_cache_hits.store(0, Ordering::Relaxed);
        self.kv_cache_misses.store(0, Ordering::Relaxed);
        self.peak_memory_bytes.store(
            self.memory_usage_bytes.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
        self.total_requests.store(0, Ordering::Relaxed);
        self.total_tokens.store(0, Ordering::Relaxed);
        self.latency_histogram.reset();
        self.inter_token_latency_ms.clear();
        self.batch_sizes.clear();
    }
}

impl Default for InferenceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of metrics at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Average time to first token (ms)
    pub ttft_avg_ms: f32,
    /// P50 TTFT
    pub ttft_p50_ms: f32,
    /// P95 TTFT
    pub ttft_p95_ms: f32,
    /// P99 TTFT
    pub ttft_p99_ms: f32,
    /// Average tokens per second
    pub tps_avg: f32,
    /// Minimum TPS observed
    pub tps_min: f32,
    /// Maximum TPS observed
    pub tps_max: f32,
    /// KV cache hit rate (0.0 - 1.0)
    pub kv_cache_hit_rate: f32,
    /// Current memory usage (bytes)
    pub memory_usage_bytes: usize,
    /// Peak memory usage (bytes)
    pub peak_memory_bytes: usize,
    /// Active requests
    pub active_requests: usize,
    /// Total requests processed
    pub total_requests: u64,
    /// Total tokens generated
    pub total_tokens: u64,
    /// Queue depth
    pub queue_depth: usize,
    /// Uptime in seconds
    pub uptime_secs: f32,
    /// Average inter-token latency
    pub inter_token_latency_avg_ms: f32,
    /// Average batch size
    pub avg_batch_size: f32,
}

impl MetricsSnapshot {
    /// Check if metrics indicate healthy performance
    pub fn is_healthy(&self, max_ttft_ms: f32, min_tps: f32) -> bool {
        self.ttft_avg_ms <= max_ttft_ms && self.tps_avg >= min_tps
    }

    /// Calculate throughput efficiency
    pub fn throughput_efficiency(&self, target_tps: f32) -> f32 {
        if target_tps <= 0.0 {
            return 1.0;
        }
        (self.tps_avg / target_tps).min(1.0)
    }

    /// Calculate latency score (0-1, higher is better)
    pub fn latency_score(&self, target_ttft_ms: f32) -> f32 {
        if self.ttft_avg_ms <= 0.0 {
            return 1.0;
        }
        (target_ttft_ms / self.ttft_avg_ms).min(1.0)
    }
}

/// Metrics collector with periodic aggregation
pub struct MetricsCollector {
    /// Current metrics
    metrics: InferenceMetrics,
    /// Historical snapshots
    history: RwLock<VecDeque<(Instant, MetricsSnapshot)>>,
    /// Maximum history size
    max_history: usize,
    /// Snapshot interval
    snapshot_interval: Duration,
    /// Last snapshot time
    last_snapshot: RwLock<Instant>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(max_history: usize, snapshot_interval: Duration) -> Self {
        Self {
            metrics: InferenceMetrics::new(),
            history: RwLock::new(VecDeque::with_capacity(max_history)),
            max_history,
            snapshot_interval,
            last_snapshot: RwLock::new(Instant::now()),
        }
    }

    /// Get reference to current metrics
    pub fn metrics(&self) -> &InferenceMetrics {
        &self.metrics
    }

    /// Record TTFT and auto-snapshot if needed
    pub fn record_ttft(&self, ttft_ms: f32) {
        self.metrics.record_ttft(ttft_ms);
        self.maybe_snapshot();
    }

    /// Record TPS and auto-snapshot if needed
    pub fn record_tps(&self, tokens: usize, duration: Duration) {
        self.metrics.record_tps(tokens, duration);
        self.maybe_snapshot();
    }

    /// Check if snapshot is needed and take it
    fn maybe_snapshot(&self) {
        let last = *self.last_snapshot.read();
        if last.elapsed() >= self.snapshot_interval {
            self.take_snapshot();
        }
    }

    /// Force a snapshot
    pub fn take_snapshot(&self) {
        let snapshot = self.metrics.snapshot();
        let now = Instant::now();

        let mut history = self.history.write();
        if history.len() >= self.max_history {
            history.pop_front();
        }
        history.push_back((now, snapshot));

        *self.last_snapshot.write() = now;
    }

    /// Get recent snapshots
    pub fn get_history(&self, count: usize) -> Vec<MetricsSnapshot> {
        let history = self.history.read();
        history
            .iter()
            .rev()
            .take(count)
            .map(|(_, s)| s.clone())
            .collect()
    }

    /// Get trend analysis (positive = improving, negative = degrading)
    pub fn ttft_trend(&self) -> f32 {
        let history = self.history.read();
        if history.len() < 2 {
            return 0.0;
        }

        let recent: Vec<f32> = history
            .iter()
            .rev()
            .take(10)
            .map(|(_, s)| s.ttft_avg_ms)
            .collect();

        if recent.len() < 2 {
            return 0.0;
        }

        // Simple linear regression slope
        let n = recent.len() as f32;
        let sum_x: f32 = (0..recent.len()).map(|i| i as f32).sum();
        let sum_y: f32 = recent.iter().sum();
        let sum_xy: f32 = recent.iter().enumerate().map(|(i, y)| i as f32 * y).sum();
        let sum_xx: f32 = (0..recent.len()).map(|i| (i * i) as f32).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);

        // Negative slope means TTFT is decreasing (improving)
        -slope
    }

    /// Get TPS trend
    pub fn tps_trend(&self) -> f32 {
        let history = self.history.read();
        if history.len() < 2 {
            return 0.0;
        }

        let recent: Vec<f32> = history
            .iter()
            .rev()
            .take(10)
            .map(|(_, s)| s.tps_avg)
            .collect();

        if recent.len() < 2 {
            return 0.0;
        }

        let n = recent.len() as f32;
        let sum_x: f32 = (0..recent.len()).map(|i| i as f32).sum();
        let sum_y: f32 = recent.iter().sum();
        let sum_xy: f32 = recent.iter().enumerate().map(|(i, y)| i as f32 * y).sum();
        let sum_xx: f32 = (0..recent.len()).map(|i| (i * i) as f32).sum();

        (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x)
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new(1000, Duration::from_secs(60))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moving_average() {
        let ma = MovingAverage::new(3);

        ma.add(1.0);
        ma.add(2.0);
        ma.add(3.0);

        assert!((ma.average() - 2.0).abs() < 0.01);

        // Adding 4th value should evict 1.0
        ma.add(4.0);
        assert!((ma.average() - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_moving_average_percentile() {
        let ma = MovingAverage::new(10);

        for i in 1..=10 {
            ma.add(i as f32);
        }

        let p50 = ma.percentile(50.0);
        assert!(p50 >= 5.0 && p50 <= 6.0);

        let p90 = ma.percentile(90.0);
        assert!(p90 >= 9.0);
    }

    #[test]
    fn test_latency_histogram() {
        let hist = LatencyHistogram::new();

        hist.record(5.0);
        hist.record(15.0);
        hist.record(50.0);

        assert_eq!(hist.count(), 3);
        assert!((hist.mean() - 23.33).abs() < 1.0);
    }

    #[test]
    fn test_inference_metrics() {
        let metrics = InferenceMetrics::new();

        metrics.record_ttft(10.0);
        metrics.record_ttft(20.0);

        assert!((metrics.ttft_ms.average() - 15.0).abs() < 0.01);

        metrics.record_kv_cache_hit();
        metrics.record_kv_cache_hit();
        metrics.record_kv_cache_miss();

        assert!((metrics.kv_cache_hit_rate() - 0.667).abs() < 0.01);
    }

    #[test]
    fn test_metrics_snapshot() {
        let metrics = InferenceMetrics::new();

        metrics.record_ttft(10.0);
        metrics.record_tps(100, Duration::from_secs(1));
        metrics.update_memory_usage(1024 * 1024);
        metrics.request_started();

        let snapshot = metrics.snapshot();

        assert!((snapshot.ttft_avg_ms - 10.0).abs() < 0.01);
        assert!((snapshot.tps_avg - 100.0).abs() < 0.01);
        assert_eq!(snapshot.memory_usage_bytes, 1024 * 1024);
        assert_eq!(snapshot.active_requests, 1);
    }

    #[test]
    fn test_metrics_collector() {
        let collector = MetricsCollector::new(100, Duration::from_millis(10));

        for i in 1..=5 {
            collector.record_ttft(i as f32 * 10.0);
        }

        collector.take_snapshot();

        let history = collector.get_history(1);
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_snapshot_health_check() {
        let snapshot = MetricsSnapshot {
            ttft_avg_ms: 50.0,
            ttft_p50_ms: 45.0,
            ttft_p95_ms: 80.0,
            ttft_p99_ms: 100.0,
            tps_avg: 150.0,
            tps_min: 100.0,
            tps_max: 200.0,
            kv_cache_hit_rate: 0.95,
            memory_usage_bytes: 1024 * 1024,
            peak_memory_bytes: 2 * 1024 * 1024,
            active_requests: 5,
            total_requests: 1000,
            total_tokens: 100000,
            queue_depth: 2,
            uptime_secs: 3600.0,
            inter_token_latency_avg_ms: 5.0,
            avg_batch_size: 8.0,
        };

        assert!(snapshot.is_healthy(100.0, 100.0));
        assert!(!snapshot.is_healthy(30.0, 100.0)); // TTFT too high
        assert!(!snapshot.is_healthy(100.0, 200.0)); // TPS too low
    }
}
