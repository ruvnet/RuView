pub mod performance_monitor;

pub use performance_monitor::{
    PerformanceMonitor, PerformanceMetrics, AggregatedMetrics,
    PerformanceThreshold, PerformanceAlert, AlertType, AlertSeverity,
    get_global_monitor,
};

// Re-export the macros
pub use monitor_performance;
pub use set_performance_threshold;