//! Metrics collection and aggregation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// Collects metrics during evaluation
#[derive(Debug)]
pub struct MetricCollector {
    /// Start time
    start: Instant,
    /// Snapshots taken
    snapshots: Vec<MetricSnapshot>,
    /// Current values
    current: HashMap<String, f64>,
    /// Counters
    counters: HashMap<String, u64>,
}

impl MetricCollector {
    /// Create new collector
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            snapshots: Vec::new(),
            current: HashMap::new(),
            counters: HashMap::new(),
        }
    }

    /// Record a value
    pub fn record(&mut self, name: &str, value: f64) {
        self.current.insert(name.to_string(), value);
    }

    /// Increment a counter
    pub fn increment(&mut self, name: &str) {
        *self.counters.entry(name.to_string()).or_insert(0) += 1;
    }

    /// Add to a counter
    pub fn add(&mut self, name: &str, delta: u64) {
        *self.counters.entry(name.to_string()).or_insert(0) += delta;
    }

    /// Take a snapshot of current state
    pub fn snapshot(&mut self, label: &str) {
        let elapsed = self.start.elapsed();
        self.snapshots.push(MetricSnapshot {
            label: label.to_string(),
            timestamp_ms: elapsed.as_millis() as u64,
            values: self.current.clone(),
            counters: self.counters.clone(),
        });
    }

    /// Get all snapshots
    pub fn get_snapshots(&self) -> &[MetricSnapshot] {
        &self.snapshots
    }

    /// Aggregate metrics across snapshots
    pub fn aggregate(&self) -> AggregatedMetrics {
        let mut aggregated = AggregatedMetrics::new();

        for snapshot in &self.snapshots {
            for (name, value) in &snapshot.values {
                aggregated.add_sample(name, *value);
            }
        }

        // Add final counter values
        for (name, count) in &self.counters {
            aggregated.counters.insert(name.clone(), *count);
        }

        aggregated
    }
}

impl Default for MetricCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// A snapshot of metrics at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSnapshot {
    /// Label for this snapshot
    pub label: String,
    /// Timestamp in milliseconds from start
    pub timestamp_ms: u64,
    /// Current values
    pub values: HashMap<String, f64>,
    /// Current counters
    pub counters: HashMap<String, u64>,
}

/// Aggregated metrics with statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    /// Per-metric statistics
    pub stats: HashMap<String, MetricStats>,
    /// Final counter values
    pub counters: HashMap<String, u64>,
}

impl AggregatedMetrics {
    /// Create new aggregated metrics
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
            counters: HashMap::new(),
        }
    }

    /// Add a sample for a metric
    pub fn add_sample(&mut self, name: &str, value: f64) {
        self.stats.entry(name.to_string()).or_default().add(value);
    }

    /// Get statistics for a metric
    pub fn get_stats(&self, name: &str) -> Option<&MetricStats> {
        self.stats.get(name)
    }

    /// Get counter value
    pub fn get_counter(&self, name: &str) -> u64 {
        self.counters.get(name).copied().unwrap_or(0)
    }
}

impl Default for AggregatedMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for a single metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricStats {
    /// Number of samples
    pub count: usize,
    /// Sum of all values
    pub sum: f64,
    /// Sum of squares (for variance)
    pub sum_sq: f64,
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
}

impl MetricStats {
    /// Create new stats
    pub fn new() -> Self {
        Self {
            count: 0,
            sum: 0.0,
            sum_sq: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
        }
    }

    /// Add a value
    pub fn add(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        self.sum_sq += value * value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
    }

    /// Get mean
    pub fn mean(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.sum / self.count as f64
    }

    /// Get variance
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        let mean = self.mean();
        (self.sum_sq / self.count as f64) - (mean * mean)
    }

    /// Get standard deviation
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }
}

impl Default for MetricStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_collector() {
        let mut collector = MetricCollector::new();

        collector.record("latency", 100.0);
        collector.increment("requests");
        collector.snapshot("after_first");

        collector.record("latency", 150.0);
        collector.increment("requests");
        collector.snapshot("after_second");

        let snapshots = collector.get_snapshots();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].values.get("latency"), Some(&100.0));
        assert_eq!(snapshots[1].counters.get("requests"), Some(&2));
    }

    #[test]
    fn test_aggregation() {
        let mut collector = MetricCollector::new();

        for i in 1..=5 {
            collector.record("value", i as f64);
            collector.snapshot(&format!("step_{}", i));
        }

        let aggregated = collector.aggregate();
        let stats = aggregated.get_stats("value").unwrap();

        assert_eq!(stats.count, 5);
        assert_eq!(stats.mean(), 3.0);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
    }

    #[test]
    fn test_metric_stats() {
        let mut stats = MetricStats::new();
        stats.add(1.0);
        stats.add(2.0);
        stats.add(3.0);

        assert_eq!(stats.count, 3);
        assert!((stats.mean() - 2.0).abs() < 0.001);
        assert!((stats.variance() - 0.6666).abs() < 0.01);
    }
}
