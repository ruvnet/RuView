//! Quality monitoring for delta-aware HNSW
//!
//! Monitors recall quality and detects when repair is needed.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;

use crate::SearchResult;

/// Configuration for quality monitoring
#[derive(Debug, Clone)]
pub struct QualityConfig {
    /// Size of the sample window
    pub window_size: usize,
    /// Number of random samples for estimation
    pub sample_count: usize,
    /// Recall threshold below which repair is triggered
    pub recall_threshold: f32,
    /// How often to run quality checks (in search count)
    pub check_interval: usize,
}

impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            window_size: 1000,
            sample_count: 100,
            recall_threshold: 0.9,
            check_interval: 100,
        }
    }
}

/// Quality metrics
#[derive(Debug, Clone, Default)]
pub struct QualityMetrics {
    /// Estimated recall
    pub recall: f32,
    /// Average number of distance computations
    pub avg_distance_comps: f32,
    /// Average search latency (ns)
    pub avg_latency_ns: f64,
    /// Total searches performed
    pub total_searches: u64,
    /// Searches since last repair
    pub searches_since_repair: u64,
}

/// Recall estimate with confidence
#[derive(Debug, Clone)]
pub struct RecallEstimate {
    /// Point estimate
    pub recall: f32,
    /// Lower bound (95% CI)
    pub lower_bound: f32,
    /// Upper bound (95% CI)
    pub upper_bound: f32,
    /// Number of samples
    pub samples: usize,
}

/// Search sample for quality estimation
struct SearchSample {
    /// Query vector (for potential re-search)
    query_hash: u64,
    /// Returned result IDs
    result_ids: Vec<String>,
    /// Distances
    distances: Vec<f32>,
    /// Timestamp
    timestamp_ns: u64,
}

/// Quality monitor for the index
pub struct QualityMonitor {
    config: QualityConfig,
    samples: RwLock<VecDeque<SearchSample>>,
    metrics: RwLock<QualityMetrics>,
    search_count: AtomicU64,
    dimensions: usize,
}

impl QualityMonitor {
    /// Create a new quality monitor
    pub fn new(dimensions: usize) -> Self {
        Self {
            config: QualityConfig::default(),
            samples: RwLock::new(VecDeque::with_capacity(1000)),
            metrics: RwLock::new(QualityMetrics::default()),
            search_count: AtomicU64::new(0),
            dimensions,
        }
    }

    /// Create with custom configuration
    pub fn with_config(dimensions: usize, config: QualityConfig) -> Self {
        Self {
            config,
            samples: RwLock::new(VecDeque::with_capacity(1000)),
            metrics: RwLock::new(QualityMetrics::default()),
            search_count: AtomicU64::new(0),
            dimensions,
        }
    }

    /// Record a search for quality monitoring
    pub fn record_search(&self, query: &[f32], results: &[SearchResult]) {
        let count = self.search_count.fetch_add(1, Ordering::Relaxed);

        // Only sample periodically
        if count % (self.config.check_interval as u64) != 0 {
            return;
        }

        let sample = SearchSample {
            query_hash: hash_vector(query),
            result_ids: results.iter().map(|r| r.id.clone()).collect(),
            distances: results.iter().map(|r| r.distance).collect(),
            timestamp_ns: current_time_ns(),
        };

        let mut samples = self.samples.write();
        samples.push_back(sample);

        // Maintain window size
        while samples.len() > self.config.window_size {
            samples.pop_front();
        }

        // Update metrics
        let mut metrics = self.metrics.write();
        metrics.total_searches = count + 1;
        metrics.searches_since_repair += 1;

        // Update average distance
        if !results.is_empty() {
            let avg_dist = results.iter().map(|r| r.distance).sum::<f32>() / results.len() as f32;
            let n = metrics.total_searches as f32;
            metrics.avg_distance_comps =
                metrics.avg_distance_comps * ((n - 1.0) / n) + avg_dist / n;
        }
    }

    /// Get current metrics
    pub fn metrics(&self) -> QualityMetrics {
        self.metrics.read().clone()
    }

    /// Estimate current recall
    pub fn estimate_recall(&self) -> RecallEstimate {
        let samples = self.samples.read();

        if samples.is_empty() {
            return RecallEstimate {
                recall: 1.0,
                lower_bound: 0.0,
                upper_bound: 1.0,
                samples: 0,
            };
        }

        // Estimate based on distance consistency
        let mut consistent = 0;
        let mut total = 0;

        for i in 1..samples.len() {
            let prev = &samples[i - 1];
            let curr = &samples[i];

            // If queries are similar, results should overlap
            if similar_queries(prev.query_hash, curr.query_hash) {
                total += 1;
                let overlap = count_overlap(&prev.result_ids, &curr.result_ids);
                if overlap > 0 {
                    consistent += 1;
                }
            }
        }

        let recall = if total > 0 {
            consistent as f32 / total as f32
        } else {
            1.0
        };

        // Wilson confidence interval
        let n = total.max(1) as f32;
        let z = 1.96; // 95% CI
        let center = (recall + z * z / (2.0 * n)) / (1.0 + z * z / n);
        let width =
            z * (recall * (1.0 - recall) / n + z * z / (4.0 * n * n)).sqrt() / (1.0 + z * z / n);

        RecallEstimate {
            recall,
            lower_bound: (center - width).max(0.0),
            upper_bound: (center + width).min(1.0),
            samples: samples.len(),
        }
    }

    /// Check if repair is needed based on quality
    pub fn needs_repair(&self) -> bool {
        let estimate = self.estimate_recall();
        estimate.recall < self.config.recall_threshold
    }

    /// Reset counters after repair
    pub fn on_repair(&self) {
        let mut metrics = self.metrics.write();
        metrics.searches_since_repair = 0;
    }

    /// Clear all samples
    pub fn clear(&self) {
        self.samples.write().clear();
        *self.metrics.write() = QualityMetrics::default();
        self.search_count.store(0, Ordering::Relaxed);
    }
}

/// Hash a vector for comparison
fn hash_vector(v: &[f32]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();

    // Hash first few elements for quick comparison
    let sample_size = v.len().min(16);
    for &x in &v[..sample_size] {
        x.to_bits().hash(&mut hasher);
    }

    hasher.finish()
}

/// Check if two queries are similar (by hash)
fn similar_queries(h1: u64, h2: u64) -> bool {
    // XOR and count differing bits
    let diff = (h1 ^ h2).count_ones();
    diff < 32
}

/// Count overlapping IDs
fn count_overlap(a: &[String], b: &[String]) -> usize {
    a.iter().filter(|id| b.contains(id)).count()
}

/// Get current time in nanoseconds
fn current_time_ns() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Distance distribution statistics
#[derive(Debug, Clone, Default)]
pub struct DistanceStats {
    /// Mean distance
    pub mean: f32,
    /// Standard deviation
    pub std_dev: f32,
    /// Minimum distance
    pub min: f32,
    /// Maximum distance
    pub max: f32,
    /// Median distance
    pub median: f32,
}

impl DistanceStats {
    /// Calculate from a list of distances
    pub fn from_distances(distances: &[f32]) -> Self {
        if distances.is_empty() {
            return Self::default();
        }

        let n = distances.len() as f32;
        let mean = distances.iter().sum::<f32>() / n;
        let variance = distances.iter().map(|d| (d - mean).powi(2)).sum::<f32>() / n;

        let mut sorted = distances.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        Self {
            mean,
            std_dev: variance.sqrt(),
            min: *sorted.first().unwrap_or(&0.0),
            max: *sorted.last().unwrap_or(&0.0),
            median: sorted[sorted.len() / 2],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_monitor_creation() {
        let monitor = QualityMonitor::new(128);
        let metrics = monitor.metrics();

        assert_eq!(metrics.total_searches, 0);
    }

    #[test]
    fn test_recall_estimation() {
        let monitor = QualityMonitor::new(128);
        let estimate = monitor.estimate_recall();

        // Empty monitor should return 1.0 recall
        assert!((estimate.recall - 1.0).abs() < 1e-6);
        assert_eq!(estimate.samples, 0);
    }

    #[test]
    fn test_hash_vector() {
        let v1 = vec![1.0f32, 2.0, 3.0, 4.0];
        let v2 = vec![1.0f32, 2.0, 3.0, 4.0];
        let v3 = vec![5.0f32, 6.0, 7.0, 8.0];

        assert_eq!(hash_vector(&v1), hash_vector(&v2));
        assert_ne!(hash_vector(&v1), hash_vector(&v3));
    }

    #[test]
    fn test_distance_stats() {
        let distances = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        let stats = DistanceStats::from_distances(&distances);

        assert!((stats.mean - 3.0).abs() < 1e-6);
        assert!((stats.min - 1.0).abs() < 1e-6);
        assert!((stats.max - 5.0).abs() < 1e-6);
        assert!((stats.median - 3.0).abs() < 1e-6);
    }
}
