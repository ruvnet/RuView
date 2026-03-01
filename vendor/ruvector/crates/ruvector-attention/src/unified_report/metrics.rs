//! Individual Geometry Metrics

use serde::{Deserialize, Serialize};

/// Type of geometric metric
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    /// Sliced Wasserstein OT distance
    OTDistance,
    /// Topology coherence (k-NN based)
    TopologyCoherence,
    /// H0 persistence death sum
    H0Persistence,
    /// Information bottleneck KL
    IBKL,
    /// Diffusion energy
    DiffusionEnergy,
    /// Fisher-Rao distance
    FisherRao,
    /// Attention entropy
    AttentionEntropy,
}

/// A metric value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricValue {
    /// Metric type
    pub metric_type: MetricType,
    /// Raw value
    pub value: f32,
    /// Normalized value (0-1)
    pub normalized: f32,
    /// Whether this is healthy (within expected range)
    pub is_healthy: bool,
    /// Warning threshold
    pub warning_threshold: f32,
    /// Critical threshold
    pub critical_threshold: f32,
}

impl MetricValue {
    /// Create new metric value
    pub fn new(
        metric_type: MetricType,
        value: f32,
        min_expected: f32,
        max_expected: f32,
        warning_threshold: f32,
        critical_threshold: f32,
    ) -> Self {
        let range = max_expected - min_expected;
        let normalized = if range > 0.0 {
            ((value - min_expected) / range).clamp(0.0, 1.0)
        } else {
            0.5
        };

        let is_healthy = value >= min_expected && value <= max_expected;

        Self {
            metric_type,
            value,
            normalized,
            is_healthy,
            warning_threshold,
            critical_threshold,
        }
    }

    /// Check if metric is in warning state
    pub fn is_warning(&self) -> bool {
        self.value > self.warning_threshold && self.value <= self.critical_threshold
    }

    /// Check if metric is in critical state
    pub fn is_critical(&self) -> bool {
        self.value > self.critical_threshold
    }

    /// Get status string
    pub fn status(&self) -> &'static str {
        if self.is_critical() {
            "CRITICAL"
        } else if self.is_warning() {
            "WARNING"
        } else if self.is_healthy {
            "OK"
        } else {
            "UNKNOWN"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_value() {
        let metric = MetricValue::new(MetricType::TopologyCoherence, 0.7, 0.0, 1.0, 0.3, 0.1);

        assert_eq!(metric.metric_type, MetricType::TopologyCoherence);
        assert!((metric.normalized - 0.7).abs() < 1e-5);
        assert!(metric.is_healthy);
    }

    #[test]
    fn test_warning_critical() {
        let metric = MetricValue::new(
            MetricType::OTDistance,
            5.0, // High OT distance
            0.0,
            10.0,
            3.0, // Warning at 3
            7.0, // Critical at 7
        );

        assert!(metric.is_warning());
        assert!(!metric.is_critical());
        assert_eq!(metric.status(), "WARNING");
    }
}
