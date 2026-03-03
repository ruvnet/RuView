//! Metrics collection for analysis layer

use prometheus::{
    Histogram, HistogramOpts, IntCounter, IntCounterVec, IntGauge, Opts, Registry,
};
use std::sync::OnceLock;

static REGISTRY: OnceLock<Registry> = OnceLock::new();

/// Get or create metrics registry
pub fn registry() -> &'static Registry {
    REGISTRY.get_or_init(|| {
        let registry = Registry::new();
        register_metrics(&registry);
        registry
    })
}

/// Register all metrics
fn register_metrics(registry: &Registry) {
    registry.register(Box::new(ANALYSIS_DURATION.clone())).unwrap();
    registry.register(Box::new(BEHAVIORAL_DURATION.clone())).unwrap();
    registry.register(Box::new(POLICY_DURATION.clone())).unwrap();
    registry.register(Box::new(ANOMALY_DETECTED.clone())).unwrap();
    registry.register(Box::new(POLICY_VIOLATIONS.clone())).unwrap();
    registry.register(Box::new(BASELINE_ATTRACTORS.clone())).unwrap();
    registry.register(Box::new(ACTIVE_POLICIES.clone())).unwrap();
}

lazy_static::lazy_static! {
    /// Total analysis duration histogram
    pub static ref ANALYSIS_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new(
            "aimds_analysis_duration_seconds",
            "Duration of full analysis in seconds"
        )
        .buckets(vec![0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0])
    ).unwrap();

    /// Behavioral analysis duration histogram
    pub static ref BEHAVIORAL_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new(
            "aimds_behavioral_duration_seconds",
            "Duration of behavioral analysis in seconds"
        )
        .buckets(vec![0.01, 0.025, 0.05, 0.1, 0.2, 0.5, 1.0])
    ).unwrap();

    /// Policy verification duration histogram
    pub static ref POLICY_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new(
            "aimds_policy_duration_seconds",
            "Duration of policy verification in seconds"
        )
        .buckets(vec![0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0])
    ).unwrap();

    /// Anomaly detection counter
    pub static ref ANOMALY_DETECTED: IntCounterVec = IntCounterVec::new(
        Opts::new("aimds_anomaly_detected_total", "Total anomalies detected"),
        &["severity"]
    ).unwrap();

    /// Policy violation counter
    pub static ref POLICY_VIOLATIONS: IntCounterVec = IntCounterVec::new(
        Opts::new("aimds_policy_violations_total", "Total policy violations"),
        &["policy_id"]
    ).unwrap();

    /// Number of baseline attractors
    pub static ref BASELINE_ATTRACTORS: IntGauge = IntGauge::new(
        "aimds_baseline_attractors",
        "Number of baseline attractors"
    ).unwrap();

    /// Number of active policies
    pub static ref ACTIVE_POLICIES: IntGauge = IntGauge::new(
        "aimds_active_policies",
        "Number of active policies"
    ).unwrap();
}
