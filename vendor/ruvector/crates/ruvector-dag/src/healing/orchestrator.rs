//! Healing Orchestrator - Main coordination

use super::{
    AnomalyConfig, AnomalyDetector, IndexHealthChecker, IndexThresholds, LearningDriftDetector,
    RepairResult, RepairStrategy,
};
use std::sync::Arc;

pub struct HealingOrchestrator {
    anomaly_detectors: std::collections::HashMap<String, AnomalyDetector>,
    index_checker: IndexHealthChecker,
    drift_detector: LearningDriftDetector,
    repair_strategies: Vec<Arc<dyn RepairStrategy>>,
    repair_history: Vec<RepairResult>,
    max_history_size: usize,
}

impl HealingOrchestrator {
    pub fn new() -> Self {
        Self {
            anomaly_detectors: std::collections::HashMap::new(),
            index_checker: IndexHealthChecker::new(IndexThresholds::default()),
            drift_detector: LearningDriftDetector::new(0.1, 100),
            repair_strategies: Vec::new(),
            repair_history: Vec::new(),
            max_history_size: 1000,
        }
    }

    pub fn with_config(
        index_thresholds: IndexThresholds,
        drift_threshold: f64,
        drift_window: usize,
    ) -> Self {
        Self {
            anomaly_detectors: std::collections::HashMap::new(),
            index_checker: IndexHealthChecker::new(index_thresholds),
            drift_detector: LearningDriftDetector::new(drift_threshold, drift_window),
            repair_strategies: Vec::new(),
            repair_history: Vec::new(),
            max_history_size: 1000,
        }
    }

    pub fn add_detector(&mut self, name: &str, config: AnomalyConfig) {
        self.anomaly_detectors
            .insert(name.to_string(), AnomalyDetector::new(config));
    }

    pub fn add_repair_strategy(&mut self, strategy: Arc<dyn RepairStrategy>) {
        self.repair_strategies.push(strategy);
    }

    pub fn observe(&mut self, component: &str, value: f64) {
        if let Some(detector) = self.anomaly_detectors.get_mut(component) {
            detector.observe(value);
        }
    }

    pub fn set_drift_baseline(&mut self, metric: &str, value: f64) {
        self.drift_detector.set_baseline(metric, value);
    }

    pub fn record_drift_metric(&mut self, metric: &str, value: f64) {
        self.drift_detector.record(metric, value);
    }

    pub fn run_cycle(&mut self) -> HealingCycleResult {
        #[allow(unused_assignments)]
        let mut anomalies_detected = 0;
        let mut repairs_attempted = 0;
        let mut repairs_succeeded = 0;

        // Detect anomalies
        let mut all_anomalies = Vec::new();
        for (component, detector) in &self.anomaly_detectors {
            let mut anomalies = detector.detect();
            for a in &mut anomalies {
                a.component = component.clone();
            }
            all_anomalies.extend(anomalies);
        }
        anomalies_detected = all_anomalies.len();

        // Check drift
        let drifts = self.drift_detector.check_all_drifts();

        // Apply repairs
        for anomaly in &all_anomalies {
            for strategy in &self.repair_strategies {
                if strategy.can_repair(anomaly) {
                    repairs_attempted += 1;
                    let result = strategy.repair(anomaly);
                    if result.success {
                        repairs_succeeded += 1;
                    }
                    self.add_repair_result(result);
                    break;
                }
            }
        }

        HealingCycleResult {
            anomalies_detected,
            drifts_detected: drifts.len(),
            repairs_attempted,
            repairs_succeeded,
        }
    }

    fn add_repair_result(&mut self, result: RepairResult) {
        self.repair_history.push(result);

        // Keep history size bounded
        if self.repair_history.len() > self.max_history_size {
            self.repair_history.remove(0);
        }
    }

    pub fn health_score(&self) -> f64 {
        // Compute overall health score 0-1
        let recent_repairs = self
            .repair_history
            .iter()
            .rev()
            .take(10)
            .filter(|r| r.success)
            .count();

        let recent_total = self.repair_history.iter().rev().take(10).count();

        if recent_total == 0 {
            1.0 // No recent issues = healthy
        } else {
            recent_repairs as f64 / recent_total as f64
        }
    }

    pub fn repair_history(&self) -> &[RepairResult] {
        &self.repair_history
    }

    pub fn detector_stats(&self, component: &str) -> Option<DetectorStats> {
        self.anomaly_detectors
            .get(component)
            .map(|d| DetectorStats {
                component: component.to_string(),
                mean: d.mean(),
                std_dev: d.std_dev(),
            })
    }

    pub fn drift_detector(&self) -> &LearningDriftDetector {
        &self.drift_detector
    }

    pub fn index_checker(&self) -> &IndexHealthChecker {
        &self.index_checker
    }
}

#[derive(Debug)]
pub struct HealingCycleResult {
    pub anomalies_detected: usize,
    pub drifts_detected: usize,
    pub repairs_attempted: usize,
    pub repairs_succeeded: usize,
}

#[derive(Debug, Clone)]
pub struct DetectorStats {
    pub component: String,
    pub mean: f64,
    pub std_dev: f64,
}

impl Default for HealingOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::healing::{Anomaly, AnomalyType, IndexRebalanceStrategy};

    #[test]
    fn test_orchestrator_creation() {
        let orchestrator = HealingOrchestrator::new();
        assert_eq!(orchestrator.health_score(), 1.0);
    }

    #[test]
    fn test_add_detector() {
        let mut orchestrator = HealingOrchestrator::new();
        orchestrator.add_detector("test", AnomalyConfig::default());

        // Observe some values
        for i in 0..20 {
            orchestrator.observe("test", i as f64);
        }

        let stats = orchestrator.detector_stats("test").unwrap();
        assert!(stats.mean > 0.0);
    }

    #[test]
    fn test_repair_cycle() {
        let mut orchestrator = HealingOrchestrator::new();
        orchestrator.add_detector("latency", AnomalyConfig::default());
        orchestrator.add_repair_strategy(Arc::new(IndexRebalanceStrategy::new(0.95)));

        // Add normal observations
        for i in 0..20 {
            orchestrator.observe("latency", 10.0 + (i as f64) * 0.1);
        }

        // Add anomaly
        orchestrator.observe("latency", 100.0);

        let result = orchestrator.run_cycle();
        assert!(result.anomalies_detected > 0 || result.repairs_attempted > 0);
    }

    #[test]
    fn test_drift_detection_integration() {
        let mut orchestrator = HealingOrchestrator::new();
        orchestrator.set_drift_baseline("accuracy", 0.95);

        // Record declining performance
        for _ in 0..10 {
            orchestrator.record_drift_metric("accuracy", 0.85);
        }

        let result = orchestrator.run_cycle();
        assert!(result.drifts_detected > 0);
    }
}
