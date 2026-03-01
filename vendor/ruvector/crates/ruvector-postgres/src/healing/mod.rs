//! Self-Healing Engine for RuVector Postgres v2
//!
//! This module provides automated problem detection and remediation capabilities:
//! - **Problem Detection**: Monitors system health and detects issues
//! - **Remediation Strategies**: Defines actions to fix detected problems
//! - **Remediation Engine**: Orchestrates strategy execution with rollback
//! - **Learning System**: Tracks outcomes and improves strategy selection
//! - **Background Worker**: Continuous health monitoring
//!
//! # Architecture
//!
//! ```text
//! +------------------------------------------------------------------+
//! |                    Integrity Monitor                              |
//! |  - Detects state transitions (normal -> stress -> critical)      |
//! +------------------------------------------------------------------+
//!                               |
//!                               v
//! +------------------------------------------------------------------+
//! |                    Problem Detector                               |
//! |  - Classifies problem types from witness edges                   |
//! +------------------------------------------------------------------+
//!                               |
//!                               v
//! +------------------------------------------------------------------+
//! |                    Remediation Engine                             |
//! |  - Selects strategy, executes with timeout/rollback              |
//! +------------------------------------------------------------------+
//!                               |
//!                               v
//! +------------------------------------------------------------------+
//! |                    Learning System                                |
//! |  - Records outcomes, updates strategy weights                    |
//! +------------------------------------------------------------------+
//! ```

pub mod detector;
pub mod engine;
pub mod functions;
pub mod learning;
pub mod strategies;
pub mod worker;

pub use detector::{Problem, ProblemDetector, ProblemType, SystemMetrics};
pub use engine::{HealingConfig, HealingOutcome, RemediationContext, RemediationEngine};
pub use learning::{OutcomeRecord, OutcomeTracker, StrategyWeight};
pub use strategies::{
    IntegrityRecovery, PromoteReplica, QueryCircuitBreaker, ReindexPartition, RemediationOutcome,
    RemediationResult, RemediationStrategy, StrategyRegistry, TierEviction,
};
pub use worker::{HealingWorker, HealingWorkerConfig, HealingWorkerState};

use parking_lot::RwLock;
use std::sync::Arc;

/// Global healing engine instance
static HEALING_ENGINE: std::sync::OnceLock<Arc<RwLock<HealingEngine>>> = std::sync::OnceLock::new();

/// Get or initialize the global healing engine
pub fn get_healing_engine() -> Arc<RwLock<HealingEngine>> {
    HEALING_ENGINE
        .get_or_init(|| Arc::new(RwLock::new(HealingEngine::new())))
        .clone()
}

/// Main healing engine combining all components
pub struct HealingEngine {
    /// Problem detector
    pub detector: ProblemDetector,
    /// Remediation engine
    pub remediation: RemediationEngine,
    /// Outcome tracker for learning
    pub tracker: OutcomeTracker,
    /// Background worker state
    pub worker_state: Arc<HealingWorkerState>,
    /// Configuration
    pub config: HealingConfig,
    /// Whether healing is enabled
    pub enabled: bool,
}

impl HealingEngine {
    /// Create a new healing engine with default configuration
    pub fn new() -> Self {
        let config = HealingConfig::default();
        let tracker = OutcomeTracker::new();
        let registry = StrategyRegistry::new_with_defaults();

        Self {
            detector: ProblemDetector::new(),
            remediation: RemediationEngine::new(registry, config.clone(), tracker.clone()),
            tracker,
            worker_state: Arc::new(HealingWorkerState::new(HealingWorkerConfig::default())),
            config,
            enabled: true,
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: HealingConfig) -> Self {
        let tracker = OutcomeTracker::new();
        let registry = StrategyRegistry::new_with_defaults();

        Self {
            detector: ProblemDetector::new(),
            remediation: RemediationEngine::new(registry, config.clone(), tracker.clone()),
            tracker,
            worker_state: Arc::new(HealingWorkerState::new(HealingWorkerConfig::default())),
            config,
            enabled: true,
        }
    }

    /// Check system health and return current status
    pub fn health_status(&self) -> HealthStatus {
        let metrics = self.detector.collect_metrics();
        let problems = self.detector.detect_problems(&metrics);
        let active_remediations = self.remediation.active_remediations();

        HealthStatus {
            healthy: problems.is_empty() && active_remediations.is_empty(),
            problem_count: problems.len(),
            active_remediation_count: active_remediations.len(),
            problems,
            metrics,
            enabled: self.enabled,
            last_check: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Enable or disable healing
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Update configuration
    pub fn update_config(&mut self, config: HealingConfig) {
        self.config = config.clone();
        self.remediation.update_config(config);
    }

    /// Trigger manual healing for a specific problem type
    pub fn trigger_healing(&self, problem_type: ProblemType) -> Option<HealingOutcome> {
        if !self.enabled {
            return None;
        }

        let problem = Problem {
            problem_type,
            severity: detector::Severity::Medium,
            detected_at: std::time::SystemTime::now(),
            details: serde_json::json!({"source": "manual_trigger"}),
            affected_partitions: vec![],
        };

        Some(self.remediation.heal(&problem))
    }
}

impl Default for HealingEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Health status summary
#[derive(Debug, Clone)]
pub struct HealthStatus {
    /// Whether the system is healthy
    pub healthy: bool,
    /// Number of detected problems
    pub problem_count: usize,
    /// Number of active remediations
    pub active_remediation_count: usize,
    /// List of detected problems
    pub problems: Vec<Problem>,
    /// Current system metrics
    pub metrics: SystemMetrics,
    /// Whether healing is enabled
    pub enabled: bool,
    /// Timestamp of last health check
    pub last_check: u64,
}

impl HealthStatus {
    /// Convert to JSON for SQL function output
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "healthy": self.healthy,
            "problem_count": self.problem_count,
            "active_remediation_count": self.active_remediation_count,
            "problems": self.problems.iter().map(|p| p.to_json()).collect::<Vec<_>>(),
            "metrics": self.metrics.to_json(),
            "enabled": self.enabled,
            "last_check": self.last_check,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_healing_engine_creation() {
        let engine = HealingEngine::new();
        assert!(engine.enabled);

        let status = engine.health_status();
        assert!(status.healthy);
    }

    #[test]
    fn test_healing_enable_disable() {
        let mut engine = HealingEngine::new();

        engine.set_enabled(false);
        assert!(!engine.enabled);

        engine.set_enabled(true);
        assert!(engine.enabled);
    }

    #[test]
    fn test_global_instance() {
        let engine1 = get_healing_engine();
        let engine2 = get_healing_engine();
        assert!(Arc::ptr_eq(&engine1, &engine2));
    }
}
