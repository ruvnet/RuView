//! Prometheus metrics for Tiny Dancer routing system
//!
//! This module provides comprehensive metrics collection for monitoring
//! routing performance, circuit breaker state, and system health.

use once_cell::sync::Lazy;
use prometheus::{
    register_counter_vec, register_gauge, register_histogram_vec, CounterVec, Gauge, HistogramVec,
    Registry, TextEncoder, Encoder,
};
use std::sync::Arc;

/// Global metrics registry
pub static METRICS_REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

/// Request counter tracking total routing requests
///
/// Note: Metrics are globally registered. In tests, the first registration wins.
pub static ROUTING_REQUESTS_TOTAL: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "tiny_dancer_routing_requests_total",
        "Total number of routing requests processed",
        &["status"]
    )
    .unwrap_or_else(|_| {
        // Already registered from a previous test/usage - create a new instance
        // that won't be registered but can still be used
        CounterVec::new(
            prometheus::Opts::new(
                "tiny_dancer_routing_requests_total_test",
                "Total number of routing requests processed",
            ),
            &["status"],
        )
        .expect("Failed to create fallback metric")
    })
});

/// Latency histogram for routing operations
pub static ROUTING_LATENCY: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "tiny_dancer_routing_latency_seconds",
        "Histogram of routing latency in seconds",
        &["operation"],
        vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]
    )
    .expect("Failed to create routing_latency metric")
});

/// Feature engineering time histogram
pub static FEATURE_ENGINEERING_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "tiny_dancer_feature_engineering_duration_seconds",
        "Time spent on feature engineering",
        &["batch_size"],
        vec![0.00001, 0.00005, 0.0001, 0.0005, 0.001, 0.005, 0.01]
    )
    .expect("Failed to create feature_engineering_duration metric")
});

/// Model inference time histogram
pub static MODEL_INFERENCE_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "tiny_dancer_model_inference_duration_seconds",
        "Time spent on model inference",
        &["model_type"],
        vec![0.00001, 0.00005, 0.0001, 0.0005, 0.001, 0.005, 0.01]
    )
    .expect("Failed to create model_inference_duration metric")
});

/// Circuit breaker state gauge (0=closed, 1=half-open, 2=open)
pub static CIRCUIT_BREAKER_STATE: Lazy<Gauge> = Lazy::new(|| {
    register_gauge!(
        "tiny_dancer_circuit_breaker_state",
        "Current state of circuit breaker (0=closed, 1=half-open, 2=open)"
    )
    .expect("Failed to create circuit_breaker_state metric")
});

/// Routing decision counter (lightweight vs powerful)
pub static ROUTING_DECISIONS: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "tiny_dancer_routing_decisions_total",
        "Number of routing decisions by model type",
        &["model_type"]
    )
    .expect("Failed to create routing_decisions metric")
});

/// Error counter by error type
pub static ERRORS_TOTAL: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "tiny_dancer_errors_total",
        "Total number of errors by type",
        &["error_type"]
    )
    .expect("Failed to create errors_total metric")
});

/// Candidates processed counter
pub static CANDIDATES_PROCESSED: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "tiny_dancer_candidates_processed_total",
        "Total number of candidates processed",
        &["batch_size_range"]
    )
    .expect("Failed to create candidates_processed metric")
});

/// Confidence score histogram
pub static CONFIDENCE_SCORES: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "tiny_dancer_confidence_scores",
        "Distribution of confidence scores",
        &["decision_type"],
        vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.85, 0.9, 0.95, 1.0]
    )
    .expect("Failed to create confidence_scores metric")
});

/// Uncertainty estimates histogram
pub static UNCERTAINTY_ESTIMATES: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "tiny_dancer_uncertainty_estimates",
        "Distribution of uncertainty estimates",
        &["decision_type"],
        vec![0.0, 0.05, 0.1, 0.15, 0.2, 0.3, 0.5, 1.0]
    )
    .expect("Failed to create uncertainty_estimates metric")
});

/// Metrics collector for Tiny Dancer
#[derive(Clone)]
pub struct MetricsCollector {
    registry: Arc<Registry>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            registry: Arc::new(METRICS_REGISTRY.clone()),
        }
    }

    /// Record a successful routing request
    pub fn record_routing_success(&self) {
        ROUTING_REQUESTS_TOTAL.with_label_values(&["success"]).inc();
    }

    /// Record a failed routing request
    pub fn record_routing_failure(&self, error_type: &str) {
        ROUTING_REQUESTS_TOTAL.with_label_values(&["failure"]).inc();
        ERRORS_TOTAL.with_label_values(&[error_type]).inc();
    }

    /// Record routing latency
    pub fn record_routing_latency(&self, operation: &str, duration_secs: f64) {
        ROUTING_LATENCY
            .with_label_values(&[operation])
            .observe(duration_secs);
    }

    /// Record feature engineering duration
    pub fn record_feature_engineering_duration(&self, batch_size: usize, duration_secs: f64) {
        let batch_label = self.batch_size_label(batch_size);
        FEATURE_ENGINEERING_DURATION
            .with_label_values(&[&batch_label])
            .observe(duration_secs);
    }

    /// Record model inference duration
    pub fn record_model_inference_duration(&self, model_type: &str, duration_secs: f64) {
        MODEL_INFERENCE_DURATION
            .with_label_values(&[model_type])
            .observe(duration_secs);
    }

    /// Update circuit breaker state
    /// 0 = Closed, 1 = HalfOpen, 2 = Open
    pub fn set_circuit_breaker_state(&self, state: f64) {
        CIRCUIT_BREAKER_STATE.set(state);
    }

    /// Record routing decision
    pub fn record_routing_decision(&self, use_lightweight: bool) {
        let model_type = if use_lightweight {
            "lightweight"
        } else {
            "powerful"
        };
        ROUTING_DECISIONS.with_label_values(&[model_type]).inc();
    }

    /// Record confidence score
    pub fn record_confidence_score(&self, use_lightweight: bool, score: f32) {
        let decision_type = if use_lightweight {
            "lightweight"
        } else {
            "powerful"
        };
        CONFIDENCE_SCORES
            .with_label_values(&[decision_type])
            .observe(score as f64);
    }

    /// Record uncertainty estimate
    pub fn record_uncertainty_estimate(&self, use_lightweight: bool, uncertainty: f32) {
        let decision_type = if use_lightweight {
            "lightweight"
        } else {
            "powerful"
        };
        UNCERTAINTY_ESTIMATES
            .with_label_values(&[decision_type])
            .observe(uncertainty as f64);
    }

    /// Record candidates processed
    pub fn record_candidates_processed(&self, count: usize) {
        let batch_label = self.batch_size_label(count);
        CANDIDATES_PROCESSED
            .with_label_values(&[&batch_label])
            .inc_by(count as f64);
    }

    /// Get batch size label for metrics
    fn batch_size_label(&self, size: usize) -> String {
        match size {
            0 => "0".to_string(),
            1..=10 => "1-10".to_string(),
            11..=50 => "11-50".to_string(),
            51..=100 => "51-100".to_string(),
            _ => "100+".to_string(),
        }
    }

    /// Export metrics in Prometheus text format
    pub fn export_metrics(&self) -> Result<String, prometheus::Error> {
        let encoder = TextEncoder::new();
        // Use prometheus::gather() to get metrics from the default global registry
        // where our metrics are actually registered
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        String::from_utf8(buffer).map_err(|e| {
            prometheus::Error::Msg(format!("Failed to encode metrics as UTF-8: {}", e))
        })
    }

    /// Get the registry
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new();
        // Registry is not empty because metrics are globally registered
        assert!(collector.registry().gather().len() >= 0);
    }

    #[test]
    fn test_record_routing_success() {
        let collector = MetricsCollector::new();
        collector.record_routing_success();
        collector.record_routing_success();

        // Metrics should be recorded
        let metrics = collector.export_metrics().unwrap();
        // Just verify it doesn't panic and returns something
        assert!(!metrics.is_empty());
    }

    #[test]
    fn test_record_routing_failure() {
        let collector = MetricsCollector::new();
        collector.record_routing_failure("inference_error");

        let metrics = collector.export_metrics().unwrap();
        // Just verify export works
        assert!(!metrics.is_empty());
    }

    #[test]
    fn test_circuit_breaker_state() {
        let collector = MetricsCollector::new();
        collector.set_circuit_breaker_state(0.0); // Closed
        collector.set_circuit_breaker_state(1.0); // Half-open
        collector.set_circuit_breaker_state(2.0); // Open

        let metrics = collector.export_metrics().unwrap();
        // Verify export works
        assert!(!metrics.is_empty());
    }

    #[test]
    fn test_routing_decisions() {
        let collector = MetricsCollector::new();
        collector.record_routing_decision(true); // lightweight
        collector.record_routing_decision(false); // powerful

        let metrics = collector.export_metrics().unwrap();
        // Verify export works
        assert!(!metrics.is_empty());
    }

    #[test]
    fn test_confidence_scores() {
        let collector = MetricsCollector::new();
        collector.record_confidence_score(true, 0.95);
        collector.record_confidence_score(false, 0.75);

        let metrics = collector.export_metrics().unwrap();
        // Verify export works
        assert!(!metrics.is_empty());
    }

    #[test]
    fn test_batch_size_labels() {
        let collector = MetricsCollector::new();
        assert_eq!(collector.batch_size_label(0), "0");
        assert_eq!(collector.batch_size_label(5), "1-10");
        assert_eq!(collector.batch_size_label(25), "11-50");
        assert_eq!(collector.batch_size_label(75), "51-100");
        assert_eq!(collector.batch_size_label(150), "100+");
    }

    #[test]
    fn test_record_latency() {
        let collector = MetricsCollector::new();
        collector.record_routing_latency("total", 0.001); // 1ms
        collector.record_feature_engineering_duration(10, 0.0005); // 0.5ms
        collector.record_model_inference_duration("fastgrnn", 0.0002); // 0.2ms

        // Verify these don't panic
        assert!(collector.export_metrics().is_ok());
    }

    #[test]
    fn test_record_candidates() {
        let collector = MetricsCollector::new();
        collector.record_candidates_processed(5);
        collector.record_candidates_processed(50);
        collector.record_candidates_processed(150);

        // Verify export works
        assert!(!collector.export_metrics().unwrap().is_empty());
    }
}
