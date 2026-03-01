//! Observability and Metrics
//!
//! This module provides tracing, metrics, and observability features for
//! production deployments. Integrates with standard observability stacks.
//!
//! ## Features
//!
//! - **Tracing**: Structured spans for request tracing
//! - **Metrics**: Counters, gauges, histograms for monitoring
//! - **Health Checks**: Liveness and readiness probes
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ruqu::metrics::{MetricsCollector, MetricsConfig};
//!
//! let config = MetricsConfig::default();
//! let mut metrics = MetricsCollector::new(config);
//!
//! // Record gate decision
//! metrics.record_decision(GateDecision::Permit, latency_ns);
//!
//! // Export metrics
//! let snapshot = metrics.snapshot();
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::tile::GateDecision;

/// Configuration for metrics collection
#[derive(Clone, Debug)]
pub struct MetricsConfig {
    /// Enable detailed histograms (more memory)
    pub enable_histograms: bool,
    /// Histogram bucket boundaries (nanoseconds)
    pub histogram_buckets: Vec<u64>,
    /// Enable per-tile metrics
    pub per_tile_metrics: bool,
    /// Metrics export interval
    pub export_interval: Duration,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enable_histograms: true,
            histogram_buckets: vec![
                100, 250, 500, 1_000, 2_500, 5_000, 10_000, 25_000, 50_000, 100_000,
            ],
            per_tile_metrics: false,
            export_interval: Duration::from_secs(10),
        }
    }
}

/// Counter metric
#[derive(Debug, Default)]
pub struct Counter {
    value: AtomicU64,
}

impl Counter {
    /// Create a new counter
    pub fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
        }
    }

    /// Increment counter by 1
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Add value to counter
    pub fn add(&self, val: u64) {
        self.value.fetch_add(val, Ordering::Relaxed);
    }

    /// Get current value
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Reset counter
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

/// Gauge metric (can go up or down)
#[derive(Debug, Default)]
pub struct Gauge {
    value: AtomicU64,
}

impl Gauge {
    /// Create a new gauge
    pub fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
        }
    }

    /// Set gauge value
    pub fn set(&self, val: u64) {
        self.value.store(val, Ordering::Relaxed);
    }

    /// Set gauge from f64 (stored as fixed-point)
    pub fn set_f64(&self, val: f64) {
        self.value
            .store((val * 1_000_000.0) as u64, Ordering::Relaxed);
    }

    /// Get current value
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Get as f64
    pub fn get_f64(&self) -> f64 {
        self.value.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }
}

/// Histogram for latency distribution
#[derive(Debug)]
pub struct Histogram {
    buckets: Vec<u64>,
    counts: Vec<AtomicU64>,
    sum: AtomicU64,
    count: AtomicU64,
}

impl Histogram {
    /// Create a new histogram with bucket boundaries
    pub fn new(buckets: Vec<u64>) -> Self {
        let counts = (0..=buckets.len()).map(|_| AtomicU64::new(0)).collect();

        Self {
            buckets,
            counts,
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    /// Record a value
    pub fn observe(&self, value: u64) {
        self.sum.fetch_add(value, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);

        // Find bucket
        let idx = self
            .buckets
            .iter()
            .position(|&b| value <= b)
            .unwrap_or(self.buckets.len());

        self.counts[idx].fetch_add(1, Ordering::Relaxed);
    }

    /// Get bucket counts
    pub fn bucket_counts(&self) -> Vec<u64> {
        self.counts
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .collect()
    }

    /// Get total count
    pub fn get_count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Get sum
    pub fn get_sum(&self) -> u64 {
        self.sum.load(Ordering::Relaxed)
    }

    /// Get mean
    pub fn mean(&self) -> f64 {
        let count = self.get_count();
        if count == 0 {
            return 0.0;
        }
        self.get_sum() as f64 / count as f64
    }

    /// Estimate percentile (approximate)
    pub fn percentile(&self, p: f64) -> u64 {
        let total = self.get_count();
        if total == 0 {
            return 0;
        }

        let target = (total as f64 * p) as u64;
        let mut cumulative = 0u64;

        for (i, count) in self.counts.iter().enumerate() {
            cumulative += count.load(Ordering::Relaxed);
            if cumulative >= target {
                return if i < self.buckets.len() {
                    self.buckets[i]
                } else {
                    self.buckets.last().copied().unwrap_or(0) * 2
                };
            }
        }

        self.buckets.last().copied().unwrap_or(0) * 2
    }
}

/// Main metrics collector
pub struct MetricsCollector {
    config: MetricsConfig,

    // Decision counters
    permits: Counter,
    defers: Counter,
    denies: Counter,

    // Latency histograms
    tick_latency: Histogram,
    merge_latency: Histogram,
    total_latency: Histogram,

    // Throughput gauges
    throughput: Gauge,
    active_tiles: Gauge,

    // Error metrics
    errors: Counter,

    // Min-cut metrics
    min_cut_value: Gauge,
    min_cut_queries: Counter,

    // Coherence metrics
    coherence_level: Gauge,
    shift_pressure: Gauge,

    // Timing
    start_time: Instant,
    last_export: Instant,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(config: MetricsConfig) -> Self {
        let buckets = config.histogram_buckets.clone();

        Self {
            config,
            permits: Counter::new(),
            defers: Counter::new(),
            denies: Counter::new(),
            tick_latency: Histogram::new(buckets.clone()),
            merge_latency: Histogram::new(buckets.clone()),
            total_latency: Histogram::new(buckets),
            throughput: Gauge::new(),
            active_tiles: Gauge::new(),
            errors: Counter::new(),
            min_cut_value: Gauge::new(),
            min_cut_queries: Counter::new(),
            coherence_level: Gauge::new(),
            shift_pressure: Gauge::new(),
            start_time: Instant::now(),
            last_export: Instant::now(),
        }
    }

    /// Record a gate decision
    pub fn record_decision(&self, decision: GateDecision, latency_ns: u64) {
        match decision {
            GateDecision::Permit => self.permits.inc(),
            GateDecision::Defer => self.defers.inc(),
            GateDecision::Deny => self.denies.inc(),
        }

        self.total_latency.observe(latency_ns);
    }

    /// Record tick latency
    pub fn record_tick_latency(&self, latency_ns: u64) {
        self.tick_latency.observe(latency_ns);
    }

    /// Record merge latency
    pub fn record_merge_latency(&self, latency_ns: u64) {
        self.merge_latency.observe(latency_ns);
    }

    /// Record min-cut query
    pub fn record_min_cut(&self, value: f64, latency_ns: u64) {
        self.min_cut_value.set_f64(value);
        self.min_cut_queries.inc();
        // Could add a separate histogram for min-cut latency
    }

    /// Record coherence metrics
    pub fn record_coherence(&self, min_cut: f64, shift: f64) {
        self.coherence_level.set_f64(min_cut);
        self.shift_pressure.set_f64(shift);
    }

    /// Record an error
    pub fn record_error(&self) {
        self.errors.inc();
    }

    /// Update throughput gauge
    pub fn update_throughput(&self, syndromes_per_sec: f64) {
        self.throughput.set_f64(syndromes_per_sec);
    }

    /// Set active tile count
    pub fn set_active_tiles(&self, count: u64) {
        self.active_tiles.set(count);
    }

    /// Get metrics snapshot
    pub fn snapshot(&self) -> MetricsSnapshot {
        let elapsed = self.start_time.elapsed();

        MetricsSnapshot {
            uptime_secs: elapsed.as_secs(),

            // Decisions
            permits: self.permits.get(),
            defers: self.defers.get(),
            denies: self.denies.get(),

            // Latency
            tick_latency_mean_ns: self.tick_latency.mean() as u64,
            tick_latency_p50_ns: self.tick_latency.percentile(0.5),
            tick_latency_p99_ns: self.tick_latency.percentile(0.99),

            merge_latency_mean_ns: self.merge_latency.mean() as u64,
            merge_latency_p99_ns: self.merge_latency.percentile(0.99),

            total_latency_mean_ns: self.total_latency.mean() as u64,
            total_latency_p99_ns: self.total_latency.percentile(0.99),

            // Throughput
            throughput: self.throughput.get_f64(),
            total_decisions: self.permits.get() + self.defers.get() + self.denies.get(),

            // Health
            errors: self.errors.get(),
            active_tiles: self.active_tiles.get(),

            // Coherence
            min_cut_value: self.min_cut_value.get_f64(),
            shift_pressure: self.shift_pressure.get_f64(),
        }
    }

    /// Export as Prometheus format
    pub fn prometheus_export(&self) -> String {
        let snap = self.snapshot();

        let mut out = String::new();

        // Help and type declarations
        out.push_str("# HELP ruqu_decisions_total Total gate decisions by type\n");
        out.push_str("# TYPE ruqu_decisions_total counter\n");
        out.push_str(&format!(
            "ruqu_decisions_total{{type=\"permit\"}} {}\n",
            snap.permits
        ));
        out.push_str(&format!(
            "ruqu_decisions_total{{type=\"defer\"}} {}\n",
            snap.defers
        ));
        out.push_str(&format!(
            "ruqu_decisions_total{{type=\"deny\"}} {}\n",
            snap.denies
        ));

        out.push_str("\n# HELP ruqu_latency_nanoseconds Latency in nanoseconds\n");
        out.push_str("# TYPE ruqu_latency_nanoseconds summary\n");
        out.push_str(&format!(
            "ruqu_latency_nanoseconds{{quantile=\"0.5\"}} {}\n",
            snap.tick_latency_p50_ns
        ));
        out.push_str(&format!(
            "ruqu_latency_nanoseconds{{quantile=\"0.99\"}} {}\n",
            snap.tick_latency_p99_ns
        ));

        out.push_str("\n# HELP ruqu_throughput_syndromes_per_second Current throughput\n");
        out.push_str("# TYPE ruqu_throughput_syndromes_per_second gauge\n");
        out.push_str(&format!(
            "ruqu_throughput_syndromes_per_second {}\n",
            snap.throughput
        ));

        out.push_str("\n# HELP ruqu_coherence_min_cut Current min-cut value\n");
        out.push_str("# TYPE ruqu_coherence_min_cut gauge\n");
        out.push_str(&format!("ruqu_coherence_min_cut {}\n", snap.min_cut_value));

        out.push_str("\n# HELP ruqu_errors_total Total errors\n");
        out.push_str("# TYPE ruqu_errors_total counter\n");
        out.push_str(&format!("ruqu_errors_total {}\n", snap.errors));

        out
    }

    /// Check if healthy (for liveness probes)
    pub fn is_healthy(&self) -> bool {
        // Healthy if we've processed something and error rate is low
        let total = self.permits.get() + self.defers.get() + self.denies.get();
        let errors = self.errors.get();

        if total == 0 {
            return true; // Not started yet
        }

        // Error rate < 1%
        (errors as f64 / total as f64) < 0.01
    }

    /// Check if ready (for readiness probes)
    pub fn is_ready(&self) -> bool {
        // Ready if we're processing within latency targets
        let p99 = self.total_latency.percentile(0.99);
        p99 < 4_000_000 // 4ms target
    }
}

/// Metrics snapshot for export
#[derive(Clone, Debug, Default)]
pub struct MetricsSnapshot {
    /// Uptime in seconds
    pub uptime_secs: u64,

    /// Total permit decisions
    pub permits: u64,
    /// Total defer decisions
    pub defers: u64,
    /// Total deny decisions
    pub denies: u64,

    /// Mean tick latency (ns)
    pub tick_latency_mean_ns: u64,
    /// P50 tick latency (ns)
    pub tick_latency_p50_ns: u64,
    /// P99 tick latency (ns)
    pub tick_latency_p99_ns: u64,

    /// Mean merge latency (ns)
    pub merge_latency_mean_ns: u64,
    /// P99 merge latency (ns)
    pub merge_latency_p99_ns: u64,

    /// Mean total latency (ns)
    pub total_latency_mean_ns: u64,
    /// P99 total latency (ns)
    pub total_latency_p99_ns: u64,

    /// Current throughput
    pub throughput: f64,
    /// Total decisions made
    pub total_decisions: u64,

    /// Total errors
    pub errors: u64,
    /// Active tiles
    pub active_tiles: u64,

    /// Current min-cut value
    pub min_cut_value: f64,
    /// Current shift pressure
    pub shift_pressure: f64,
}

impl MetricsSnapshot {
    /// Calculate permit rate
    pub fn permit_rate(&self) -> f64 {
        if self.total_decisions == 0 {
            return 0.0;
        }
        self.permits as f64 / self.total_decisions as f64
    }

    /// Calculate deny rate
    pub fn deny_rate(&self) -> f64 {
        if self.total_decisions == 0 {
            return 0.0;
        }
        self.denies as f64 / self.total_decisions as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter() {
        let counter = Counter::new();
        assert_eq!(counter.get(), 0);

        counter.inc();
        assert_eq!(counter.get(), 1);

        counter.add(10);
        assert_eq!(counter.get(), 11);
    }

    #[test]
    fn test_gauge() {
        let gauge = Gauge::new();
        gauge.set(100);
        assert_eq!(gauge.get(), 100);

        gauge.set_f64(3.14159);
        assert!((gauge.get_f64() - 3.14159).abs() < 0.001);
    }

    #[test]
    fn test_histogram() {
        let hist = Histogram::new(vec![100, 500, 1000]);

        hist.observe(50);
        hist.observe(200);
        hist.observe(800);
        hist.observe(2000);

        assert_eq!(hist.get_count(), 4);

        let counts = hist.bucket_counts();
        assert_eq!(counts[0], 1); // <= 100
        assert_eq!(counts[1], 1); // <= 500
        assert_eq!(counts[2], 1); // <= 1000
        assert_eq!(counts[3], 1); // > 1000
    }

    #[test]
    fn test_metrics_collector() {
        let config = MetricsConfig::default();
        let metrics = MetricsCollector::new(config);

        metrics.record_decision(GateDecision::Permit, 500);
        metrics.record_decision(GateDecision::Permit, 600);
        metrics.record_decision(GateDecision::Deny, 1000);

        let snap = metrics.snapshot();
        assert_eq!(snap.permits, 2);
        assert_eq!(snap.denies, 1);
        assert_eq!(snap.total_decisions, 3);
    }

    #[test]
    fn test_prometheus_export() {
        let config = MetricsConfig::default();
        let metrics = MetricsCollector::new(config);

        metrics.record_decision(GateDecision::Permit, 500);

        let prom = metrics.prometheus_export();
        assert!(prom.contains("ruqu_decisions_total"));
        assert!(prom.contains("permit"));
    }

    #[test]
    fn test_health_checks() {
        let config = MetricsConfig::default();
        let metrics = MetricsCollector::new(config);

        assert!(metrics.is_healthy());
        assert!(metrics.is_ready());

        // Record some decisions
        for _ in 0..100 {
            metrics.record_decision(GateDecision::Permit, 500);
        }

        assert!(metrics.is_healthy());
    }
}
