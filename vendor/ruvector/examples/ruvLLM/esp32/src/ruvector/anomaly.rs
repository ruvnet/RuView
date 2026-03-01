//! Anomaly Detection - Intelligent Pattern Recognition for ESP32
//!
//! Uses vector embeddings to detect unusual patterns in sensor data,
//! behavior, or any time-series data. Perfect for:
//! - Industrial equipment monitoring
//! - Security systems
//! - Health monitoring
//! - Environmental sensing
//!
//! # How It Works
//!
//! ```text
//! Training Phase:
//! ┌─────────────────────────────────────────────────────────┐
//! │  Normal readings ──▶ Embed ──▶ Store in cluster         │
//! │  [temp=25, vibration=1.2, sound=40dB]                   │
//! │           ▼                                              │
//! │     [0.2, 0.1, 0.8, ...]  ──▶  Centroid A               │
//! └─────────────────────────────────────────────────────────┘
//!
//! Detection Phase:
//! ┌─────────────────────────────────────────────────────────┐
//! │  New reading ──▶ Embed ──▶ Distance to clusters         │
//! │  [temp=85, vibration=15.0, sound=95dB]  ◀── ANOMALY!    │
//! │           ▼                                              │
//! │     [0.9, 0.8, 0.1, ...]  ──▶  Distance: 0.95           │
//! │                                (threshold: 0.5)          │
//! └─────────────────────────────────────────────────────────┘
//! ```

use heapless::Vec as HVec;
use super::{MicroHNSW, HNSWConfig, MicroVector, DistanceMetric, euclidean_distance_i8};

/// Maximum normal patterns to learn
pub const MAX_PATTERNS: usize = 128;
/// Pattern embedding dimension
pub const PATTERN_DIM: usize = 32;
/// Maximum clusters
pub const MAX_CLUSTERS: usize = 8;

/// Anomaly detection configuration
#[derive(Debug, Clone)]
pub struct AnomalyConfig {
    /// Distance threshold for anomaly (0-1000 scale)
    pub threshold: i32,
    /// Minimum samples to establish baseline
    pub min_samples: usize,
    /// Enable adaptive threshold
    pub adaptive: bool,
    /// Smoothing factor for running average (0-100)
    pub smoothing: u8,
    /// Number of clusters for pattern grouping
    pub num_clusters: usize,
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            threshold: 500,      // Distance threshold
            min_samples: 10,     // Need 10 samples for baseline
            adaptive: true,      // Adapt threshold over time
            smoothing: 80,       // 80% weight to historical average
            num_clusters: 4,     // Group into 4 clusters
        }
    }
}

/// Anomaly detection result
#[derive(Debug, Clone)]
pub struct AnomalyResult {
    /// Is this an anomaly?
    pub is_anomaly: bool,
    /// Distance to nearest normal pattern
    pub distance: i32,
    /// Anomaly score (0-100, higher = more anomalous)
    pub score: u8,
    /// Nearest cluster ID
    pub nearest_cluster: Option<u8>,
    /// Confidence level (0-100)
    pub confidence: u8,
    /// Suggested label for anomaly type
    pub anomaly_type: AnomalyType,
}

/// Types of anomalies
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnomalyType {
    /// Normal operation
    Normal,
    /// Point anomaly (single unusual reading)
    Point,
    /// Contextual anomaly (unusual for this context)
    Contextual,
    /// Collective anomaly (pattern of unusual readings)
    Collective,
    /// Drift (gradual change from baseline)
    Drift,
    /// Spike (sudden large change)
    Spike,
    /// Unknown pattern
    Unknown,
}

/// Cluster centroid
#[derive(Debug, Clone)]
struct Cluster {
    /// Centroid embedding
    centroid: HVec<i32, PATTERN_DIM>,
    /// Number of samples in cluster
    count: u32,
    /// Sum for online averaging
    sum: HVec<i64, PATTERN_DIM>,
    /// Variance estimate
    variance: i32,
}

impl Default for Cluster {
    fn default() -> Self {
        Self {
            centroid: HVec::new(),
            count: 0,
            sum: HVec::new(),
            variance: 0,
        }
    }
}

/// Anomaly Detector
pub struct AnomalyDetector {
    /// Configuration
    config: AnomalyConfig,
    /// HNSW index for pattern matching
    index: MicroHNSW<PATTERN_DIM, MAX_PATTERNS>,
    /// Pattern storage
    patterns: HVec<HVec<i8, PATTERN_DIM>, MAX_PATTERNS>,
    /// Cluster centroids
    clusters: HVec<Cluster, MAX_CLUSTERS>,
    /// Running average distance
    avg_distance: i32,
    /// Running variance
    variance: i32,
    /// Sample count
    sample_count: u32,
    /// Consecutive anomaly count
    anomaly_streak: u16,
    /// Last few readings for collective detection
    recent_window: HVec<i32, 16>,
}

impl AnomalyDetector {
    /// Create new anomaly detector
    pub fn new(config: AnomalyConfig) -> Self {
        let hnsw_config = HNSWConfig {
            m: 4,
            m_max0: 8,
            ef_construction: 16,
            ef_search: 8,
            metric: DistanceMetric::Euclidean,
            binary_mode: false,
        };

        let mut clusters = HVec::new();
        for _ in 0..config.num_clusters {
            let _ = clusters.push(Cluster::default());
        }

        Self {
            config,
            index: MicroHNSW::new(hnsw_config),
            patterns: HVec::new(),
            clusters,
            avg_distance: 0,
            variance: 0,
            sample_count: 0,
            anomaly_streak: 0,
            recent_window: HVec::new(),
        }
    }

    /// Number of learned patterns
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// Has enough samples for reliable detection
    pub fn is_trained(&self) -> bool {
        self.sample_count >= self.config.min_samples as u32
    }

    /// Memory usage in bytes
    pub fn memory_bytes(&self) -> usize {
        self.index.memory_bytes() +
        self.patterns.len() * PATTERN_DIM +
        self.clusters.len() * core::mem::size_of::<Cluster>()
    }

    /// Learn a normal pattern
    pub fn learn(&mut self, embedding: &[i8]) -> Result<(), &'static str> {
        if self.patterns.len() >= MAX_PATTERNS {
            // Remove oldest pattern
            self.patterns.swap_remove(0);
        }

        // Store pattern
        let mut pattern = HVec::new();
        for &v in embedding.iter().take(PATTERN_DIM) {
            pattern.push(v).map_err(|_| "Pattern overflow")?;
        }

        // Add to index
        let vec = MicroVector {
            data: pattern.clone(),
            id: self.patterns.len() as u32,
        };
        self.index.insert(&vec)?;

        // Update clusters
        self.update_clusters(&pattern);

        self.patterns.push(pattern).map_err(|_| "Pattern storage full")?;
        self.sample_count += 1;

        Ok(())
    }

    /// Detect if embedding is anomalous
    pub fn detect(&mut self, embedding: &[i8]) -> AnomalyResult {
        // Not enough training data
        if !self.is_trained() {
            // Learn this as normal
            let _ = self.learn(embedding);
            return AnomalyResult {
                is_anomaly: false,
                distance: 0,
                score: 0,
                nearest_cluster: None,
                confidence: 0,
                anomaly_type: AnomalyType::Normal,
            };
        }

        // Find nearest pattern
        let results = self.index.search(embedding, 3);

        let distance = if results.is_empty() {
            i32::MAX
        } else {
            results[0].distance
        };

        // Find nearest cluster
        let (nearest_cluster, cluster_distance) = self.find_nearest_cluster(embedding);

        // Update running statistics
        self.update_statistics(distance);

        // Calculate adaptive threshold
        let threshold = if self.config.adaptive {
            self.avg_distance + 2 * self.variance.max(100)
        } else {
            self.config.threshold
        };

        // Determine anomaly type
        let is_anomaly = distance > threshold;
        let anomaly_type = self.classify_anomaly(distance, is_anomaly);

        // Update streak
        if is_anomaly {
            self.anomaly_streak = self.anomaly_streak.saturating_add(1);
        } else {
            self.anomaly_streak = 0;
            // Optionally learn this as normal
            if distance < threshold / 2 {
                let _ = self.learn(embedding);
            }
        }

        // Calculate score (0-100)
        let score = if threshold > 0 {
            ((distance * 100) / threshold).min(100) as u8
        } else {
            0
        };

        // Confidence based on sample count (0-100 scale)
        let confidence = self.sample_count.min(100) as u8;

        AnomalyResult {
            is_anomaly,
            distance,
            score,
            nearest_cluster: Some(nearest_cluster),
            confidence,
            anomaly_type,
        }
    }

    /// Update running statistics
    fn update_statistics(&mut self, distance: i32) {
        // Online mean and variance (Welford's algorithm)
        self.sample_count += 1;
        let n = self.sample_count as i64;

        let delta = distance - self.avg_distance;
        self.avg_distance += (delta / n as i32);

        let delta2 = distance - self.avg_distance;
        self.variance = ((self.variance as i64 * (n - 1) + (delta as i64 * delta2 as i64)) / n) as i32;

        // Update recent window
        if self.recent_window.len() >= 16 {
            self.recent_window.remove(0);
        }
        let _ = self.recent_window.push(distance);
    }

    /// Update cluster centroids
    fn update_clusters(&mut self, pattern: &[i8]) {
        // Find nearest cluster
        let (cluster_idx, _) = self.find_nearest_cluster(pattern);

        if let Some(cluster) = self.clusters.get_mut(cluster_idx as usize) {
            // Initialize if empty
            if cluster.count == 0 {
                for &v in pattern.iter().take(PATTERN_DIM) {
                    let _ = cluster.centroid.push(v as i32);
                    let _ = cluster.sum.push(v as i64);
                }
            } else {
                // Online centroid update
                for (i, &v) in pattern.iter().take(PATTERN_DIM).enumerate() {
                    if i < cluster.sum.len() {
                        cluster.sum[i] += v as i64;
                    }
                    if i < cluster.centroid.len() {
                        cluster.centroid[i] = (cluster.sum[i] / (cluster.count as i64 + 1)) as i32;
                    }
                }
            }
            cluster.count += 1;
        }
    }

    /// Find nearest cluster centroid
    fn find_nearest_cluster(&self, pattern: &[i8]) -> (u8, i32) {
        let mut best_idx = 0u8;
        let mut best_dist = i32::MAX;

        for (i, cluster) in self.clusters.iter().enumerate() {
            if cluster.count == 0 {
                continue;
            }

            // Calculate distance to centroid
            let mut dist = 0i32;
            for (j, &v) in pattern.iter().take(PATTERN_DIM).enumerate() {
                if j < cluster.centroid.len() {
                    let diff = v as i32 - cluster.centroid[j];
                    dist += diff * diff;
                }
            }

            if dist < best_dist {
                best_dist = dist;
                best_idx = i as u8;
            }
        }

        (best_idx, best_dist)
    }

    /// Classify the type of anomaly
    fn classify_anomaly(&self, distance: i32, is_anomaly: bool) -> AnomalyType {
        if !is_anomaly {
            return AnomalyType::Normal;
        }

        // Check for spike (sudden large deviation)
        if distance > self.avg_distance * 3 {
            return AnomalyType::Spike;
        }

        // Check for collective (multiple anomalies in window)
        let anomalies_in_window = self.recent_window.iter()
            .filter(|&&d| d > self.config.threshold)
            .count();

        if anomalies_in_window >= 3 {
            return AnomalyType::Collective;
        }

        // Check for drift (gradual increase)
        if self.recent_window.len() >= 8 {
            let first_half_avg: i32 = self.recent_window[..4].iter().sum::<i32>() / 4;
            let second_half_avg: i32 = self.recent_window[4..8].iter().sum::<i32>() / 4;
            if second_half_avg > first_half_avg + self.variance {
                return AnomalyType::Drift;
            }
        }

        // Check for streak
        if self.anomaly_streak > 2 {
            return AnomalyType::Collective;
        }

        AnomalyType::Point
    }

    /// Get current threshold
    pub fn current_threshold(&self) -> i32 {
        if self.config.adaptive {
            self.avg_distance + 2 * self.variance.max(100)
        } else {
            self.config.threshold
        }
    }

    /// Reset to untrained state
    pub fn reset(&mut self) {
        self.patterns.clear();
        self.sample_count = 0;
        self.avg_distance = 0;
        self.variance = 0;
        self.anomaly_streak = 0;
        self.recent_window.clear();

        for cluster in self.clusters.iter_mut() {
            cluster.count = 0;
            cluster.centroid.clear();
            cluster.sum.clear();
        }
    }
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::new(AnomalyConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anomaly_detector() {
        let mut detector = AnomalyDetector::default();

        // Train with normal patterns
        for i in 0..20 {
            let pattern: HVec<i8, PATTERN_DIM> = (0..PATTERN_DIM).map(|j| ((i + j) % 20) as i8).collect();
            detector.learn(&pattern).unwrap();
        }

        assert!(detector.is_trained());
        assert!(detector.pattern_count() >= 10);
    }

    #[test]
    fn test_detect_anomaly() {
        let mut detector = AnomalyDetector::default();

        // Train with similar patterns
        for _ in 0..20 {
            let pattern = [10i8; PATTERN_DIM];
            detector.learn(&pattern).unwrap();
        }

        // Normal pattern
        let normal = [11i8; PATTERN_DIM];
        let result = detector.detect(&normal);
        assert!(!result.is_anomaly || result.score < 50);

        // Anomalous pattern
        let anomaly = [100i8; PATTERN_DIM];
        let result = detector.detect(&anomaly);
        assert!(result.is_anomaly || result.score > 50);
    }
}
