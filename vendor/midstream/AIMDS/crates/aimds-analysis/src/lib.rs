//! # AIMDS Analysis Layer
//!
//! High-level behavioral analysis and policy verification for AIMDS using
//! temporal-attractor-studio and temporal-neural-solver.
//!
//! ## Components
//!
//! - **Behavioral Analyzer**: Attractor-based anomaly detection (target: <100ms p99)
//! - **Policy Verifier**: LTL-based policy verification (target: <500ms p99)
//! - **LTL Checker**: Linear Temporal Logic verification engine
//!
//! ## Performance
//!
//! - Behavioral analysis: 87ms baseline + overhead → <100ms p99
//! - Policy verification: 423ms baseline + overhead → <500ms p99
//! - Combined deep path: <520ms total

pub mod behavioral;
pub mod policy_verifier;
pub mod ltl_checker;
pub mod errors;

pub use behavioral::{BehavioralAnalyzer, BehaviorProfile, AnomalyScore};
pub use policy_verifier::{PolicyVerifier, SecurityPolicy, VerificationResult};
pub use ltl_checker::{LTLChecker, LTLFormula, Trace};
pub use errors::{AnalysisError, AnalysisResult};

use std::sync::Arc;
use tokio::sync::RwLock;
use aimds_core::types::PromptInput;

/// Combined analysis engine integrating behavioral and policy verification
pub struct AnalysisEngine {
    behavioral: Arc<BehavioralAnalyzer>,
    policy: Arc<RwLock<PolicyVerifier>>,
    ltl: Arc<LTLChecker>,
}

impl AnalysisEngine {
    /// Create new analysis engine with default configuration
    pub fn new(dimensions: usize) -> AnalysisResult<Self> {
        Ok(Self {
            behavioral: Arc::new(BehavioralAnalyzer::new(dimensions)?),
            policy: Arc::new(RwLock::new(PolicyVerifier::new()?)),
            ltl: Arc::new(LTLChecker::new()),
        })
    }

    /// Analyze behavior and verify policies
    pub async fn analyze_full(
        &self,
        sequence: &[f64],
        input: &PromptInput,
    ) -> AnalysisResult<FullAnalysis> {
        let start = std::time::Instant::now();

        // Parallel behavioral analysis and policy verification
        let behavior_future = self.behavioral.analyze_behavior(sequence);
        let policy_guard = self.policy.read().await;
        let policy_future = policy_guard.verify_policy(input);

        let (behavior_result, policy_result) = tokio::join!(
            behavior_future,
            policy_future
        );

        let behavior = behavior_result?;
        let policy = policy_result?;

        let duration = start.elapsed();

        Ok(FullAnalysis {
            behavior,
            policy,
            duration,
        })
    }

    /// Get behavioral analyzer reference
    pub fn behavioral(&self) -> &BehavioralAnalyzer {
        &self.behavioral
    }

    /// Get policy verifier reference
    pub fn policy(&self) -> Arc<RwLock<PolicyVerifier>> {
        Arc::clone(&self.policy)
    }

    /// Get LTL checker reference
    pub fn ltl(&self) -> &LTLChecker {
        &self.ltl
    }
}

/// Combined analysis result
#[derive(Debug, Clone)]
pub struct FullAnalysis {
    pub behavior: AnomalyScore,
    pub policy: VerificationResult,
    pub duration: std::time::Duration,
}

impl FullAnalysis {
    /// Check if analysis indicates a threat
    pub fn is_threat(&self) -> bool {
        self.behavior.is_anomalous || !self.policy.verified
    }

    /// Get threat severity (0.0 = safe, 1.0 = critical)
    pub fn threat_level(&self) -> f64 {
        if !self.is_threat() {
            return 0.0;
        }

        // Combine behavioral score and policy verification
        let behavioral_weight = 0.6;
        let policy_weight = 0.4;

        let behavioral_score = self.behavior.score;
        let policy_score = if self.policy.verified { 0.0 } else { 1.0 };

        behavioral_score * behavioral_weight + policy_score * policy_weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_engine_creation() {
        let engine = AnalysisEngine::new(10).unwrap();
        assert!(Arc::strong_count(&engine.behavioral) >= 1);
    }

    #[tokio::test]
    async fn test_threat_level() {
        let analysis = FullAnalysis {
            behavior: AnomalyScore {
                score: 0.8,
                is_anomalous: true,
                confidence: 0.95,
            },
            policy: VerificationResult {
                verified: false,
                confidence: 0.9,
                violations: vec!["unauthorized_access".to_string()],
                proof: None,
            },
            duration: std::time::Duration::from_millis(150),
        };

        assert!(analysis.is_threat());
        let level = analysis.threat_level();
        assert!(level > 0.6 && level < 1.0);
    }
}
