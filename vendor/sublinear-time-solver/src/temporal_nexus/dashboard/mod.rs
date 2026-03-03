// Consciousness metrics dashboard module exports
//
// This module provides real-time monitoring and visualization of consciousness
// emergence metrics with nanosecond temporal precision.

pub mod dashboard;
pub mod metrics_collector;
pub mod visualizer;
pub mod exporter;

pub use dashboard::{
    ConsciousnessMetricsDashboard,
    DashboardConfig,
    ConsciousnessMetrics,
    MetricThresholds,
    AnomalyAlert,
};

pub use metrics_collector::{
    MetricsCollector,
    CollectorConfig,
    MetricSource,
    TemporalMetrics,
};

pub use visualizer::{
    ConsciousnessVisualizer,
    VisualizationMode,
    TerminalRenderer,
    MetricChart,
};

pub use exporter::{
    MetricsExporter,
    ExportFormat,
    ExportConfig,
    MetricsSummary,
};

// Re-export common types for convenience
pub type Timestamp = std::time::Instant;
pub type ConsciousnessLevel = f64;
pub type TemporalAdvantage = u64; // microseconds
pub type PrecisionNanos = u64;

// Constants for consciousness metrics
pub const MAX_CONSCIOUSNESS_LEVEL: f64 = 1.0;
pub const MIN_CONSCIOUSNESS_LEVEL: f64 = 0.0;
pub const CRITICAL_COHERENCE_THRESHOLD: f64 = 0.85;
pub const WARNING_COHERENCE_THRESHOLD: f64 = 0.75;
pub const NANOSECOND_PRECISION_TARGET: u64 = 100; // Target precision in nanoseconds