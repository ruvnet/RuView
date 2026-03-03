//! Common test utilities

use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize test environment
pub fn setup() {
    INIT.call_once(|| {
        // Initialize tracing for tests
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();
    });
}

/// Test configuration
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub max_mitigations: usize,
    pub optimization_levels: usize,
    pub timeout_ms: u64,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            max_mitigations: 100,
            optimization_levels: 25,
            timeout_ms: 5000,
        }
    }
}

/// Create test metrics collector
pub fn metrics_collector() -> MetricsCollector {
    MetricsCollector::new()
}

/// Metrics collector for testing
#[derive(Debug, Default)]
pub struct MetricsCollector {
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_pass(&mut self) {
        self.total_tests += 1;
        self.passed_tests += 1;
    }

    pub fn record_fail(&mut self) {
        self.total_tests += 1;
        self.failed_tests += 1;
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_tests == 0 {
            return 0.0;
        }
        self.passed_tests as f64 / self.total_tests as f64
    }
}
