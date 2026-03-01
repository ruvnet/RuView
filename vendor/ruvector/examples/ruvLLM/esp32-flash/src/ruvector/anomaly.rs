//! Anomaly Detection via Embedding Distance

use heapless::Vec as HVec;
use super::{MicroHNSW, HNSWConfig, MicroVector, DistanceMetric};

const ANOMALY_DIM: usize = 32;
const HISTORY_SIZE: usize = 64;

#[derive(Debug, Clone)]
pub struct AnomalyConfig {
    pub threshold_multiplier: f32,
    pub min_samples: usize,
    pub window_size: usize,
    pub adapt_rate: f32,
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self { threshold_multiplier: 2.0, min_samples: 10, window_size: 32, adapt_rate: 0.1 }
    }
}

#[derive(Debug, Clone)]
pub struct AnomalyResult {
    pub is_anomaly: bool,
    pub score: i32,
    pub threshold: i32,
    pub confidence: u8,
    pub nearest_distance: i32,
}

pub struct AnomalyDetector {
    config: AnomalyConfig,
    index: MicroHNSW<ANOMALY_DIM, HISTORY_SIZE>,
    distance_history: HVec<i32, HISTORY_SIZE>,
    mean_distance: i32,
    std_distance: i32,
    next_id: u32,
}

impl AnomalyDetector {
    pub fn new(config: AnomalyConfig) -> Self {
        let hnsw_config = HNSWConfig { m: 4, m_max0: 8, ef_construction: 16, ef_search: 8, metric: DistanceMetric::Euclidean, binary_mode: false };
        Self { config, index: MicroHNSW::new(hnsw_config), distance_history: HVec::new(), mean_distance: 0, std_distance: 100, next_id: 0 }
    }

    pub fn len(&self) -> usize { self.index.len() }

    pub fn add_sample(&mut self, embedding: &[i8]) -> Result<AnomalyResult, &'static str> {
        let result = self.check(embedding);

        let id = self.next_id;
        self.next_id += 1;

        let mut data = HVec::new();
        for &v in embedding.iter().take(ANOMALY_DIM) { data.push(v).map_err(|_| "Embedding too large")?; }
        let vec = MicroVector { data, id };
        self.index.insert(&vec)?;

        if result.nearest_distance > 0 {
            if self.distance_history.len() >= HISTORY_SIZE { self.distance_history.remove(0); }
            let _ = self.distance_history.push(result.nearest_distance);
            self.update_stats();
        }

        Ok(result)
    }

    pub fn check(&self, embedding: &[i8]) -> AnomalyResult {
        if self.index.len() < self.config.min_samples {
            return AnomalyResult { is_anomaly: false, score: 0, threshold: 0, confidence: 0, nearest_distance: 0 };
        }

        let results = self.index.search(embedding, 1);
        let nearest_distance = results.first().map(|r| r.distance).unwrap_or(i32::MAX);
        let threshold = self.compute_threshold();
        let is_anomaly = nearest_distance > threshold;
        let score = nearest_distance - self.mean_distance;
        let confidence = self.compute_confidence(nearest_distance, threshold);

        AnomalyResult { is_anomaly, score, threshold, confidence, nearest_distance }
    }

    fn compute_threshold(&self) -> i32 {
        let multiplier = (self.config.threshold_multiplier * 100.0) as i32;
        self.mean_distance + (self.std_distance * multiplier) / 100
    }

    fn compute_confidence(&self, distance: i32, threshold: i32) -> u8 {
        if threshold == 0 { return 0; }
        let diff = (distance - threshold).abs();
        let conf = if distance > threshold {
            50 + ((diff * 50) / threshold.max(1)).min(50)
        } else {
            50 - ((diff * 50) / threshold.max(1)).min(50)
        };
        conf.clamp(0, 100) as u8
    }

    fn update_stats(&mut self) {
        if self.distance_history.is_empty() { return; }

        let sum: i32 = self.distance_history.iter().sum();
        self.mean_distance = sum / self.distance_history.len() as i32;

        let variance: i32 = self.distance_history.iter()
            .map(|&d| { let diff = d - self.mean_distance; diff * diff })
            .sum::<i32>() / self.distance_history.len() as i32;

        self.std_distance = isqrt(variance as u64) as i32;
    }

    pub fn reset(&mut self) {
        self.index = MicroHNSW::new(HNSWConfig::default());
        self.distance_history.clear();
        self.mean_distance = 0;
        self.std_distance = 100;
        self.next_id = 0;
    }

    pub fn stats(&self) -> AnomalyStats {
        AnomalyStats { samples: self.index.len(), mean_distance: self.mean_distance, std_distance: self.std_distance, threshold: self.compute_threshold() }
    }
}

#[derive(Debug, Clone)]
pub struct AnomalyStats {
    pub samples: usize,
    pub mean_distance: i32,
    pub std_distance: i32,
    pub threshold: i32,
}

fn isqrt(n: u64) -> u64 {
    if n == 0 { return 0; }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x { x = y; y = (x + n / x) / 2; }
    x
}

impl Default for AnomalyDetector { fn default() -> Self { Self::new(AnomalyConfig::default()) } }
