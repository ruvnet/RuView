//! Learning Drift Detection

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DriftMetric {
    pub name: String,
    pub current_value: f64,
    pub baseline_value: f64,
    pub drift_magnitude: f64,
    pub trend: DriftTrend,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriftTrend {
    Improving,
    Stable,
    Declining,
}

pub struct LearningDriftDetector {
    baselines: HashMap<String, f64>,
    current_values: HashMap<String, Vec<f64>>,
    drift_threshold: f64,
    window_size: usize,
}

impl LearningDriftDetector {
    pub fn new(drift_threshold: f64, window_size: usize) -> Self {
        Self {
            baselines: HashMap::new(),
            current_values: HashMap::new(),
            drift_threshold,
            window_size,
        }
    }

    pub fn set_baseline(&mut self, metric: &str, value: f64) {
        self.baselines.insert(metric.to_string(), value);
    }

    pub fn record(&mut self, metric: &str, value: f64) {
        let values = self
            .current_values
            .entry(metric.to_string())
            .or_insert_with(Vec::new);

        values.push(value);

        // Keep only window_size values
        if values.len() > self.window_size {
            values.remove(0);
        }
    }

    pub fn check_drift(&self, metric: &str) -> Option<DriftMetric> {
        let baseline = self.baselines.get(metric)?;
        let values = self.current_values.get(metric)?;

        if values.is_empty() {
            return None;
        }

        let current = values.iter().sum::<f64>() / values.len() as f64;
        let drift_magnitude = (current - baseline).abs() / baseline.abs().max(1e-10);

        let trend = if current > *baseline * 1.05 {
            DriftTrend::Improving
        } else if current < *baseline * 0.95 {
            DriftTrend::Declining
        } else {
            DriftTrend::Stable
        };

        Some(DriftMetric {
            name: metric.to_string(),
            current_value: current,
            baseline_value: *baseline,
            drift_magnitude,
            trend,
        })
    }

    pub fn check_all_drifts(&self) -> Vec<DriftMetric> {
        self.baselines
            .keys()
            .filter_map(|metric| self.check_drift(metric))
            .filter(|d| d.drift_magnitude > self.drift_threshold)
            .collect()
    }

    pub fn drift_threshold(&self) -> f64 {
        self.drift_threshold
    }

    pub fn window_size(&self) -> usize {
        self.window_size
    }

    pub fn metrics(&self) -> Vec<String> {
        self.baselines.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_setting() {
        let mut detector = LearningDriftDetector::new(0.1, 10);
        detector.set_baseline("accuracy", 0.95);

        assert_eq!(detector.baselines.get("accuracy"), Some(&0.95));
    }

    #[test]
    fn test_stable_metric() {
        let mut detector = LearningDriftDetector::new(0.1, 10);
        detector.set_baseline("accuracy", 0.95);

        for _ in 0..10 {
            detector.record("accuracy", 0.95);
        }

        let drift = detector.check_drift("accuracy").unwrap();
        assert_eq!(drift.trend, DriftTrend::Stable);
        assert!(drift.drift_magnitude < 0.01);
    }

    #[test]
    fn test_improving_trend() {
        let mut detector = LearningDriftDetector::new(0.1, 10);
        detector.set_baseline("accuracy", 0.80);

        for i in 0..10 {
            detector.record("accuracy", 0.85 + (i as f64) * 0.01);
        }

        let drift = detector.check_drift("accuracy").unwrap();
        assert_eq!(drift.trend, DriftTrend::Improving);
    }

    #[test]
    fn test_declining_trend() {
        let mut detector = LearningDriftDetector::new(0.1, 10);
        detector.set_baseline("accuracy", 0.95);

        for _ in 0..10 {
            detector.record("accuracy", 0.85);
        }

        let drift = detector.check_drift("accuracy").unwrap();
        assert_eq!(drift.trend, DriftTrend::Declining);
    }

    #[test]
    fn test_drift_threshold() {
        let mut detector = LearningDriftDetector::new(0.1, 10);
        detector.set_baseline("metric1", 1.0);
        detector.set_baseline("metric2", 1.0);

        // metric1: no drift
        for _ in 0..10 {
            detector.record("metric1", 1.05);
        }

        // metric2: significant drift
        for _ in 0..10 {
            detector.record("metric2", 1.5);
        }

        let drifts = detector.check_all_drifts();
        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].name, "metric2");
    }
}
