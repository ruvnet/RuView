//! Self-Healing System for Neural DAG Learning

mod anomaly;
mod drift_detector;
mod index_health;
mod orchestrator;
mod strategies;

pub use anomaly::{Anomaly, AnomalyConfig, AnomalyDetector, AnomalyType};
pub use drift_detector::{DriftMetric, DriftTrend, LearningDriftDetector};
pub use index_health::{
    HealthStatus, IndexCheckResult, IndexHealth, IndexHealthChecker, IndexThresholds, IndexType,
};
pub use orchestrator::{HealingCycleResult, HealingOrchestrator};
pub use strategies::{
    CacheFlushStrategy, IndexRebalanceStrategy, PatternResetStrategy, RepairResult, RepairStrategy,
};
