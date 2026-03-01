//! # Execution Module: Coherence-Gated Action Execution
//!
//! This module implements the coherence gate and compute ladder from ADR-014,
//! providing threshold-based gating for external side effects with mandatory
//! witness creation.
//!
//! ## Architecture Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                         ACTION EXECUTOR                                  │
//! │  Orchestrates the entire execution flow with mandatory witnesses        │
//! └─────────────────────────────────────────────────────────────────────────┘
//!                                    │
//!                                    ▼
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                         COHERENCE GATE                                   │
//! │  Threshold-based gating with persistence detection                      │
//! │  Policy bundle reference • Energy history • Witness creation            │
//! └─────────────────────────────────────────────────────────────────────────┘
//!                                    │
//!                                    ▼
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                         COMPUTE LADDER                                   │
//! │  Lane 0 (Reflex) → Lane 1 (Retrieval) → Lane 2 (Heavy) → Lane 3 (Human)│
//! │  <1ms              ~10ms               ~100ms             async          │
//! └─────────────────────────────────────────────────────────────────────────┘
//!                                    │
//!                                    ▼
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                         ACTION TRAIT                                     │
//! │  Scope • Impact • Metadata • Execute • Content Hash                     │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Key Design Principles
//!
//! 1. **Most updates stay in reflex lane** - Low energy (<threshold) = fast path
//! 2. **Persistence detection** - Sustained incoherence triggers escalation
//! 3. **Mandatory witness creation** - Every decision is auditable
//! 4. **Policy bundle reference** - All decisions reference signed governance
//!
//! ## Example Usage
//!
//! ```ignore
//! use prime_radiant::execution::{
//!     Action, ActionExecutor, CoherenceGate, EnergySnapshot,
//!     LaneThresholds, PolicyBundleRef,
//! };
//!
//! // Create gate with thresholds
//! let gate = CoherenceGate::new(
//!     LaneThresholds::default(),
//!     Duration::from_secs(5),
//!     PolicyBundleRef::placeholder(),
//! );
//!
//! // Create executor
//! let executor = ActionExecutor::with_defaults(gate);
//!
//! // Execute action
//! let energy = EnergySnapshot::new(0.1, 0.05, action.scope().clone());
//! let result = executor.execute(&action, &energy);
//!
//! // Result always includes witness
//! assert!(result.witness.verify_integrity());
//! ```
//!
//! ## Module Structure
//!
//! - [`action`] - Action trait and related types for external side effects
//! - [`gate`] - Coherence gate with threshold-based gating logic
//! - [`ladder`] - Compute lane enum and escalation logic
//! - [`executor`] - Action executor with mandatory witness creation

pub mod action;
pub mod executor;
pub mod gate;
pub mod ladder;

// Re-export primary types for convenient access
pub use action::{
    Action, ActionError, ActionId, ActionImpact, ActionMetadata, ActionResult, BoxedAction,
    ExecutionContext, ScopeId,
};

pub use gate::{
    CoherenceGate, EnergyHistory, EnergySnapshot, GateDecision, PolicyBundleRef, WitnessId,
    WitnessRecord,
};

pub use ladder::{ComputeLane, EscalationReason, LaneThresholds, LaneTransition, ThresholdError};

pub use executor::{
    ActionExecutor, ActionResultBuilder, ExecutionResult, ExecutionStats, ExecutorConfig,
    ExecutorStats, HumanReviewItem,
};

/// Prelude module for convenient imports.
pub mod prelude {
    pub use super::{
        Action, ActionError, ActionExecutor, ActionId, ActionImpact, ActionMetadata, ActionResult,
        CoherenceGate, ComputeLane, EnergySnapshot, EscalationReason, ExecutionContext,
        ExecutionResult, ExecutorConfig, GateDecision, LaneThresholds, PolicyBundleRef, ScopeId,
        WitnessId, WitnessRecord,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // Integration test action
    struct IntegrationTestAction {
        scope: ScopeId,
        metadata: ActionMetadata,
    }

    impl IntegrationTestAction {
        fn new(scope: &str) -> Self {
            Self {
                scope: ScopeId::new(scope),
                metadata: ActionMetadata::new("IntegrationTest", "Test action", "test"),
            }
        }
    }

    impl Action for IntegrationTestAction {
        type Output = String;
        type Error = ActionError;

        fn scope(&self) -> &ScopeId {
            &self.scope
        }

        fn impact(&self) -> ActionImpact {
            ActionImpact::low()
        }

        fn metadata(&self) -> &ActionMetadata {
            &self.metadata
        }

        fn execute(&self, ctx: &ExecutionContext) -> Result<String, ActionError> {
            Ok(format!(
                "Executed in {:?} lane, energy: {:.3}",
                ctx.assigned_lane, ctx.current_energy
            ))
        }

        fn content_hash(&self) -> [u8; 32] {
            let hash = blake3::hash(format!("test:{}", self.scope.as_str()).as_bytes());
            let mut result = [0u8; 32];
            result.copy_from_slice(hash.as_bytes());
            result
        }

        fn make_rollback_not_supported_error() -> ActionError {
            ActionError::RollbackNotSupported
        }
    }

    #[test]
    fn test_integration_low_energy() {
        let gate = CoherenceGate::new(
            LaneThresholds::default(),
            Duration::from_secs(5),
            PolicyBundleRef::placeholder(),
        );
        let executor = ActionExecutor::with_defaults(gate);

        let action = IntegrationTestAction::new("users.123");
        let energy = EnergySnapshot::new(0.1, 0.05, action.scope.clone());

        let result = executor.execute(&action, &energy);

        assert!(result.result.is_ok());
        assert_eq!(result.decision.lane, ComputeLane::Reflex);
        assert!(result.witness.verify_integrity());
        assert!(result.result.unwrap().contains("Reflex"));
    }

    #[test]
    fn test_integration_escalation() {
        let gate = CoherenceGate::new(
            LaneThresholds::new(0.1, 0.3, 0.6),
            Duration::from_secs(5),
            PolicyBundleRef::placeholder(),
        );
        let executor = ActionExecutor::with_defaults(gate);

        let action = IntegrationTestAction::new("trades.456");
        let energy = EnergySnapshot::new(0.4, 0.25, action.scope.clone());

        let result = executor.execute(&action, &energy);

        assert!(result.result.is_ok());
        assert!(result.decision.lane >= ComputeLane::Retrieval);
        assert!(result.decision.is_escalated());
    }

    #[test]
    fn test_integration_denial() {
        let gate = CoherenceGate::new(
            LaneThresholds::new(0.1, 0.3, 0.6),
            Duration::from_secs(5),
            PolicyBundleRef::placeholder(),
        );
        let executor = ActionExecutor::with_defaults(gate);

        let action = IntegrationTestAction::new("critical.789");
        let energy = EnergySnapshot::new(0.9, 0.85, action.scope.clone());

        let result = executor.execute(&action, &energy);

        assert!(result.result.is_err());
        assert!(!result.decision.allow);
        assert_eq!(result.decision.lane, ComputeLane::Human);
    }

    #[test]
    fn test_integration_witness_chain() {
        let gate = CoherenceGate::new(
            LaneThresholds::default(),
            Duration::from_secs(5),
            PolicyBundleRef::placeholder(),
        );
        let executor = ActionExecutor::with_defaults(gate);

        // Execute multiple actions
        let mut witnesses = Vec::new();
        for i in 0..3 {
            let action = IntegrationTestAction::new(&format!("scope.{}", i));
            let energy = EnergySnapshot::new(0.1, 0.05, action.scope.clone());
            let result = executor.execute(&action, &energy);
            witnesses.push(result.witness);
        }

        // Verify chain
        assert!(witnesses[0].previous_witness.is_none());
        assert_eq!(witnesses[1].previous_witness, Some(witnesses[0].id.clone()));
        assert_eq!(witnesses[2].previous_witness, Some(witnesses[1].id.clone()));

        // All witnesses should have valid integrity
        for witness in &witnesses {
            assert!(witness.verify_integrity());
        }
    }

    #[test]
    fn test_lane_budget_ordering() {
        // Verify that lane latency budgets increase with lane number
        let lanes = [
            ComputeLane::Reflex,
            ComputeLane::Retrieval,
            ComputeLane::Heavy,
            ComputeLane::Human,
        ];

        for window in lanes.windows(2) {
            assert!(window[0].latency_budget_us() < window[1].latency_budget_us());
        }
    }

    #[test]
    fn test_scope_hierarchy() {
        let global = ScopeId::global();
        let parent = ScopeId::new("users");
        let child = ScopeId::path(&["users", "123", "profile"]);

        assert!(global.is_parent_of(&parent));
        assert!(global.is_parent_of(&child));
        assert!(parent.is_parent_of(&child));
        assert!(!child.is_parent_of(&parent));
    }

    #[test]
    fn test_impact_risk_scores() {
        let impacts = [
            ActionImpact::minimal(),
            ActionImpact::low(),
            ActionImpact::medium(),
            ActionImpact::high(),
            ActionImpact::critical(),
        ];

        // Risk scores should generally increase
        for window in impacts.windows(2) {
            assert!(
                window[0].risk_score() <= window[1].risk_score(),
                "Risk scores should increase: {:?} vs {:?}",
                window[0].risk_score(),
                window[1].risk_score()
            );
        }
    }
}
