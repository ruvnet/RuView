//! Anomaly Detection using Z-score analysis

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct AnomalyConfig {
    pub z_threshold: f64,   // Z-score threshold (default: 3.0)
    pub window_size: usize, // Rolling window size (default: 100)
    pub min_samples: usize, // Minimum samples before detection (default: 10)
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            z_threshold: 3.0,
            window_size: 100,
            min_samples: 10,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AnomalyType {
    LatencySpike,
    PatternDrift,
    MemoryPressure,
    CacheEviction,
    LearningStall,
}

#[derive(Debug, Clone)]
pub struct Anomaly {
    pub anomaly_type: AnomalyType,
    pub z_score: f64,
    pub value: f64,
    pub expected: f64,
    pub timestamp: std::time::Instant,
    pub component: String,
}

pub struct AnomalyDetector {
    config: AnomalyConfig,
    observations: VecDeque<f64>,
    sum: f64,
    sum_sq: f64,
}

impl AnomalyDetector {
    pub fn new(config: AnomalyConfig) -> Self {
        Self {
            config,
            observations: VecDeque::with_capacity(100),
            sum: 0.0,
            sum_sq: 0.0,
        }
    }

    pub fn observe(&mut self, value: f64) {
        // Add to window
        if self.observations.len() >= self.config.window_size {
            if let Some(old) = self.observations.pop_front() {
                self.sum -= old;
                self.sum_sq -= old * old;
            }
        }

        self.observations.push_back(value);
        self.sum += value;
        self.sum_sq += value * value;
    }

    pub fn is_anomaly(&self, value: f64) -> Option<f64> {
        if self.observations.len() < self.config.min_samples {
            return None;
        }

        let n = self.observations.len() as f64;
        let mean = self.sum / n;
        let variance = (self.sum_sq / n) - (mean * mean);
        let std_dev = variance.sqrt();

        if std_dev < 1e-10 {
            return None;
        }

        let z_score = (value - mean) / std_dev;

        if z_score.abs() > self.config.z_threshold {
            Some(z_score)
        } else {
            None
        }
    }

    pub fn detect(&self) -> Vec<Anomaly> {
        // Check recent observations for anomalies
        let mut anomalies = Vec::new();

        if let Some(&last) = self.observations.back() {
            if let Some(z_score) = self.is_anomaly(last) {
                let n = self.observations.len() as f64;
                let mean = self.sum / n;

                anomalies.push(Anomaly {
                    anomaly_type: AnomalyType::LatencySpike,
                    z_score,
                    value: last,
                    expected: mean,
                    timestamp: std::time::Instant::now(),
                    component: "unknown".to_string(),
                });
            }
        }

        anomalies
    }

    pub fn mean(&self) -> f64 {
        if self.observations.is_empty() {
            0.0
        } else {
            self.sum / self.observations.len() as f64
        }
    }

    pub fn std_dev(&self) -> f64 {
        if self.observations.len() < 2 {
            return 0.0;
        }
        let n = self.observations.len() as f64;
        let mean = self.sum / n;
        let variance = (self.sum_sq / n) - (mean * mean);
        variance.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anomaly_detection() {
        let mut detector = AnomalyDetector::new(AnomalyConfig::default());

        // Add normal observations
        for i in 0..20 {
            detector.observe(10.0 + (i as f64) * 0.1);
        }

        // Add anomaly
        detector.observe(50.0);

        let anomalies = detector.detect();
        assert!(!anomalies.is_empty());
    }

    #[test]
    fn test_rolling_window() {
        let config = AnomalyConfig {
            z_threshold: 3.0,
            window_size: 10,
            min_samples: 5,
        };
        let mut detector = AnomalyDetector::new(config);

        for i in 0..20 {
            detector.observe(i as f64);
        }

        assert_eq!(detector.observations.len(), 10);
    }
}
