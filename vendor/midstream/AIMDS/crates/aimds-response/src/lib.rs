//! AIMDS Response Layer
//!
//! Adaptive response and mitigation system with meta-learning capabilities.
//! Uses strange-loop recursive self-improvement for autonomous threat response.
//!
//! # Features
//!
//! - **Meta-Learning**: 25-level recursive optimization using strange-loop
//! - **Adaptive Mitigation**: Self-improving threat response strategies
//! - **Rollback Support**: Safe mitigation with automatic rollback
//! - **Audit Logging**: Comprehensive tracking of all mitigation actions
//!
//! # Example
//!
//! ```rust,no_run
//! use aimds_response::{ResponseSystem, MitigationStrategy};
//! use aimds_core::ThreatIncident;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let response_system = ResponseSystem::new().await?;
//!
//!     // Apply adaptive mitigation
//!     let result = response_system.mitigate(&threat).await?;
//!
//!     // Learn from outcome
//!     response_system.learn_from_result(&result).await?;
//!
//!     Ok(())
//! }
//! ```

pub mod meta_learning;
pub mod adaptive;
pub mod mitigations;
pub mod audit;
pub mod rollback;
pub mod error;

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::meta_learning::ThreatIncident;

pub use meta_learning::MetaLearningEngine;
pub use adaptive::{AdaptiveMitigator, MitigationStrategy};
pub use mitigations::{MitigationAction, MitigationOutcome, ThreatContext};
pub use audit::AuditLogger;
pub use rollback::RollbackManager;
pub use error::{ResponseError, Result};

/// Main response system coordinating meta-learning and adaptive mitigation
#[derive(Clone)]
pub struct ResponseSystem {
    meta_learner: Arc<RwLock<MetaLearningEngine>>,
    mitigator: Arc<RwLock<AdaptiveMitigator>>,
    audit_logger: Arc<AuditLogger>,
    rollback_manager: Arc<RollbackManager>,
}

impl ResponseSystem {
    /// Create new response system with default configuration
    pub async fn new() -> Result<Self> {
        Ok(Self {
            meta_learner: Arc::new(RwLock::new(MetaLearningEngine::new())),
            mitigator: Arc::new(RwLock::new(AdaptiveMitigator::new())),
            audit_logger: Arc::new(AuditLogger::new()),
            rollback_manager: Arc::new(RollbackManager::new()),
        })
    }

    /// Apply mitigation to detected threat
    pub async fn mitigate(&self, threat: &ThreatIncident) -> Result<MitigationOutcome> {
        let context = ThreatContext::from_incident(threat);

        // Record mitigation attempt
        self.audit_logger.log_mitigation_start(&context).await;

        // Apply mitigation with rollback support
        let mitigator = self.mitigator.read().await;
        let result = mitigator.apply_mitigation(threat).await;

        match &result {
            Ok(outcome) => {
                self.audit_logger.log_mitigation_success(&context, outcome).await;

                // Update effectiveness tracking
                drop(mitigator);
                let mut mitigator = self.mitigator.write().await;
                mitigator.update_effectiveness(&outcome.strategy_id, true);
            }
            Err(e) => {
                self.audit_logger.log_mitigation_failure(&context, e).await;

                // Attempt rollback
                self.rollback_manager.rollback_last().await?;
            }
        }

        result
    }

    /// Learn from mitigation outcome to improve future responses
    pub async fn learn_from_result(&self, outcome: &MitigationOutcome) -> Result<()> {
        let mut meta_learner = self.meta_learner.write().await;
        meta_learner.learn_from_outcome(outcome).await;
        Ok(())
    }

    /// Optimize strategies based on feedback signals
    pub async fn optimize(&self, feedback: &[FeedbackSignal]) -> Result<()> {
        let mut meta_learner = self.meta_learner.write().await;
        meta_learner.optimize_strategy(feedback);
        Ok(())
    }

    /// Get current system metrics
    pub async fn metrics(&self) -> ResponseMetrics {
        let meta_learner = self.meta_learner.read().await;
        let mitigator = self.mitigator.read().await;

        ResponseMetrics {
            learned_patterns: meta_learner.learned_patterns_count(),
            active_strategies: mitigator.active_strategies_count(),
            total_mitigations: self.audit_logger.total_mitigations(),
            successful_mitigations: self.audit_logger.successful_mitigations(),
            optimization_level: meta_learner.current_optimization_level(),
        }
    }
}

/// Feedback signal for meta-learning optimization
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FeedbackSignal {
    pub strategy_id: String,
    pub success: bool,
    pub effectiveness_score: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub context: Option<String>,
}

/// Response system performance metrics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResponseMetrics {
    pub learned_patterns: usize,
    pub active_strategies: usize,
    pub total_mitigations: u64,
    pub successful_mitigations: u64,
    pub optimization_level: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_response_system_creation() {
        let system = ResponseSystem::new().await;
        assert!(system.is_ok());
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        let system = ResponseSystem::new().await.unwrap();
        let metrics = system.metrics().await;

        assert_eq!(metrics.learned_patterns, 0);
        assert_eq!(metrics.total_mitigations, 0);
    }
}
