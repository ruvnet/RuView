//! Gate decision types, thresholds, and three-filter decision logic
//!
//! This module implements the three-filter decision process:
//! 1. Structural filter - based on min-cut analysis
//! 2. Shift filter - drift detection from expected patterns
//! 3. Evidence filter - confidence score threshold
//!
//! ## Performance Optimizations
//!
//! - VecDeque for O(1) history rotation (instead of Vec::remove(0))
//! - Inline score calculation functions
//! - Pre-computed threshold reciprocals for division optimization
//! - Early-exit evaluation order (most likely failures first)

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::supergraph::ReducedGraph;

/// Gate decision: Permit, Defer, or Deny
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GateDecision {
    /// Action is permitted - stable enough to proceed
    Permit,
    /// Action is deferred - uncertain, escalate to human/stronger model
    Defer,
    /// Action is denied - unstable or policy-violating
    Deny,
}

impl std::fmt::Display for GateDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateDecision::Permit => write!(f, "permit"),
            GateDecision::Defer => write!(f, "defer"),
            GateDecision::Deny => write!(f, "deny"),
        }
    }
}

/// Evidence filter decision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceDecision {
    /// Sufficient evidence of coherence
    Accept,
    /// Insufficient evidence either way
    Continue,
    /// Strong evidence of incoherence
    Reject,
}

/// Filter type in the decision process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionFilter {
    /// Min-cut based structural analysis
    Structural,
    /// Drift detection from patterns
    Shift,
    /// Confidence/evidence threshold
    Evidence,
}

impl std::fmt::Display for DecisionFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecisionFilter::Structural => write!(f, "Structural"),
            DecisionFilter::Shift => write!(f, "Shift"),
            DecisionFilter::Evidence => write!(f, "Evidence"),
        }
    }
}

/// Outcome of the three-filter decision process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionOutcome {
    /// The gate decision
    pub decision: GateDecision,
    /// Overall confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Which filter rejected (if any)
    pub rejected_by: Option<DecisionFilter>,
    /// Reason for rejection (if rejected)
    pub rejection_reason: Option<String>,
    /// Structural filter score
    pub structural_score: f64,
    /// Shift filter score
    pub shift_score: f64,
    /// Evidence filter score
    pub evidence_score: f64,
    /// Min-cut value from structural analysis
    pub mincut_value: f64,
}

impl DecisionOutcome {
    /// Create a permit outcome
    #[inline]
    pub fn permit(
        confidence: f64,
        structural: f64,
        shift: f64,
        evidence: f64,
        mincut: f64,
    ) -> Self {
        Self {
            decision: GateDecision::Permit,
            confidence,
            rejected_by: None,
            rejection_reason: None,
            structural_score: structural,
            shift_score: shift,
            evidence_score: evidence,
            mincut_value: mincut,
        }
    }

    /// Create a deferred outcome
    #[inline]
    pub fn defer(
        filter: DecisionFilter,
        reason: String,
        structural: f64,
        shift: f64,
        evidence: f64,
        mincut: f64,
    ) -> Self {
        // OPTIMIZATION: Multiply by reciprocal instead of divide
        let confidence = (structural + shift + evidence) * (1.0 / 3.0);
        Self {
            decision: GateDecision::Defer,
            confidence,
            rejected_by: Some(filter),
            rejection_reason: Some(reason),
            structural_score: structural,
            shift_score: shift,
            evidence_score: evidence,
            mincut_value: mincut,
        }
    }

    /// Create a denied outcome
    #[inline]
    pub fn deny(
        filter: DecisionFilter,
        reason: String,
        structural: f64,
        shift: f64,
        evidence: f64,
        mincut: f64,
    ) -> Self {
        // OPTIMIZATION: Multiply by reciprocal instead of divide
        let confidence = (structural + shift + evidence) * (1.0 / 3.0);
        Self {
            decision: GateDecision::Deny,
            confidence,
            rejected_by: Some(filter),
            rejection_reason: Some(reason),
            structural_score: structural,
            shift_score: shift,
            evidence_score: evidence,
            mincut_value: mincut,
        }
    }
}

/// Threshold configuration for the gate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateThresholds {
    /// E-process level indicating incoherence (default: 0.01)
    pub tau_deny: f64,
    /// E-process level indicating coherence (default: 100.0)
    pub tau_permit: f64,
    /// Minimum cut value for structural stability
    pub min_cut: f64,
    /// Maximum shift pressure before deferral
    pub max_shift: f64,
    /// Permit token TTL in nanoseconds
    pub permit_ttl_ns: u64,
    /// Conformal set size requiring deferral
    pub theta_uncertainty: f64,
    /// Conformal set size for confident permit
    pub theta_confidence: f64,
}

impl Default for GateThresholds {
    fn default() -> Self {
        Self {
            tau_deny: 0.01,
            tau_permit: 100.0,
            min_cut: 5.0,
            max_shift: 0.5,
            permit_ttl_ns: 60_000_000_000, // 60 seconds
            theta_uncertainty: 20.0,
            theta_confidence: 5.0,
        }
    }
}

/// Three-filter decision evaluator
///
/// Implements the core decision logic for the coherence gate:
/// 1. Structural filter - checks min-cut stability
/// 2. Shift filter - detects drift from baseline
/// 3. Evidence filter - validates confidence threshold
///
/// OPTIMIZATION: Uses VecDeque for O(1) history rotation instead of Vec::remove(0)
pub struct ThreeFilterDecision {
    /// Gate thresholds
    thresholds: GateThresholds,
    /// Pre-computed reciprocals for fast division
    /// OPTIMIZATION: Avoid division in hot path
    inv_min_cut: f64,
    inv_max_shift: f64,
    inv_tau_range: f64,
    /// Historical baseline for shift detection
    baseline_mincut: Option<f64>,
    /// Window of recent mincut values for drift detection
    /// OPTIMIZATION: VecDeque for O(1) push_back and pop_front
    mincut_history: VecDeque<f64>,
    /// Maximum history size
    history_size: usize,
}

impl ThreeFilterDecision {
    /// Create a new three-filter decision evaluator
    pub fn new(thresholds: GateThresholds) -> Self {
        // OPTIMIZATION: Pre-compute reciprocals for fast division
        let inv_min_cut = 1.0 / thresholds.min_cut;
        let inv_max_shift = 1.0 / thresholds.max_shift;
        let inv_tau_range = 1.0 / (thresholds.tau_permit - thresholds.tau_deny);

        Self {
            thresholds,
            inv_min_cut,
            inv_max_shift,
            inv_tau_range,
            baseline_mincut: None,
            // OPTIMIZATION: Use VecDeque for O(1) rotation
            mincut_history: VecDeque::with_capacity(100),
            history_size: 100,
        }
    }

    /// Set baseline min-cut for shift detection
    #[inline]
    pub fn set_baseline(&mut self, baseline: f64) {
        self.baseline_mincut = Some(baseline);
    }

    /// Update history with a new min-cut observation
    ///
    /// OPTIMIZATION: Uses VecDeque for O(1) push/pop instead of Vec::remove(0) which is O(n)
    #[inline]
    pub fn observe_mincut(&mut self, mincut: f64) {
        // OPTIMIZATION: VecDeque::push_back + pop_front is O(1)
        if self.mincut_history.len() >= self.history_size {
            self.mincut_history.pop_front();
        }
        self.mincut_history.push_back(mincut);

        // Update baseline if not set
        if self.baseline_mincut.is_none() && !self.mincut_history.is_empty() {
            self.baseline_mincut = Some(self.compute_baseline());
        }
    }

    /// Compute baseline from history
    ///
    /// OPTIMIZATION: Uses iterator sum for cache-friendly access
    #[inline]
    fn compute_baseline(&self) -> f64 {
        let len = self.mincut_history.len();
        if len == 0 {
            return 0.0;
        }
        let sum: f64 = self.mincut_history.iter().sum();
        sum / len as f64
    }

    /// Evaluate a request against the three filters
    ///
    /// OPTIMIZATION: Uses pre-computed reciprocals for division,
    /// inline score calculations, early-exit on failures
    #[inline]
    pub fn evaluate(&self, graph: &ReducedGraph) -> DecisionOutcome {
        let mincut_value = graph.global_cut();
        let shift_pressure = graph.aggregate_shift_pressure();
        let e_value = graph.aggregate_evidence();

        // 1. Structural Filter - Min-cut analysis
        // OPTIMIZATION: Use pre-computed reciprocal
        let structural_score = self.compute_structural_score(mincut_value);

        if mincut_value < self.thresholds.min_cut {
            return DecisionOutcome::deny(
                DecisionFilter::Structural,
                format!(
                    "Min-cut {:.3} below threshold {:.3}",
                    mincut_value, self.thresholds.min_cut
                ),
                structural_score,
                0.0,
                0.0,
                mincut_value,
            );
        }

        // 2. Shift Filter - Drift detection
        // OPTIMIZATION: Use pre-computed reciprocal
        let shift_score = self.compute_shift_score(shift_pressure);

        if shift_pressure >= self.thresholds.max_shift {
            return DecisionOutcome::defer(
                DecisionFilter::Shift,
                format!(
                    "Shift pressure {:.3} exceeds threshold {:.3}",
                    shift_pressure, self.thresholds.max_shift
                ),
                structural_score,
                shift_score,
                0.0,
                mincut_value,
            );
        }

        // 3. Evidence Filter - E-value threshold
        // OPTIMIZATION: Use pre-computed reciprocal
        let evidence_score = self.compute_evidence_score(e_value);

        if e_value < self.thresholds.tau_deny {
            return DecisionOutcome::deny(
                DecisionFilter::Evidence,
                format!(
                    "E-value {:.3} below denial threshold {:.3}",
                    e_value, self.thresholds.tau_deny
                ),
                structural_score,
                shift_score,
                evidence_score,
                mincut_value,
            );
        }

        if e_value < self.thresholds.tau_permit {
            return DecisionOutcome::defer(
                DecisionFilter::Evidence,
                format!(
                    "E-value {:.3} below permit threshold {:.3}",
                    e_value, self.thresholds.tau_permit
                ),
                structural_score,
                shift_score,
                evidence_score,
                mincut_value,
            );
        }

        // All filters passed
        // OPTIMIZATION: Multiply by reciprocal
        let confidence = (structural_score + shift_score + evidence_score) * (1.0 / 3.0);

        DecisionOutcome::permit(
            confidence,
            structural_score,
            shift_score,
            evidence_score,
            mincut_value,
        )
    }

    /// Compute structural score from min-cut value
    ///
    /// OPTIMIZATION: Uses pre-computed reciprocal, marked inline(always)
    #[inline(always)]
    fn compute_structural_score(&self, mincut_value: f64) -> f64 {
        if mincut_value >= self.thresholds.min_cut {
            1.0
        } else {
            // OPTIMIZATION: Multiply by reciprocal instead of divide
            mincut_value * self.inv_min_cut
        }
    }

    /// Compute shift score from shift pressure
    ///
    /// OPTIMIZATION: Uses pre-computed reciprocal, marked inline(always)
    #[inline(always)]
    fn compute_shift_score(&self, shift_pressure: f64) -> f64 {
        // OPTIMIZATION: Multiply by reciprocal, use f64::min for branchless
        1.0 - (shift_pressure * self.inv_max_shift).min(1.0)
    }

    /// Compute evidence score from e-value
    ///
    /// OPTIMIZATION: Uses pre-computed reciprocal, marked inline(always)
    #[inline(always)]
    fn compute_evidence_score(&self, e_value: f64) -> f64 {
        if e_value >= self.thresholds.tau_permit {
            1.0
        } else if e_value <= self.thresholds.tau_deny {
            0.0
        } else {
            // OPTIMIZATION: Multiply by reciprocal
            (e_value - self.thresholds.tau_deny) * self.inv_tau_range
        }
    }

    /// Get current thresholds
    #[inline]
    pub fn thresholds(&self) -> &GateThresholds {
        &self.thresholds
    }

    /// Get history size
    #[inline(always)]
    pub fn history_len(&self) -> usize {
        self.mincut_history.len()
    }

    /// Get current baseline
    #[inline(always)]
    pub fn baseline(&self) -> Option<f64> {
        self.baseline_mincut
    }

    /// Update thresholds and recompute reciprocals
    ///
    /// OPTIMIZATION: Recomputes cached reciprocals when thresholds change
    pub fn update_thresholds(&mut self, thresholds: GateThresholds) {
        self.inv_min_cut = 1.0 / thresholds.min_cut;
        self.inv_max_shift = 1.0 / thresholds.max_shift;
        self.inv_tau_range = 1.0 / (thresholds.tau_permit - thresholds.tau_deny);
        self.thresholds = thresholds;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_decision_display() {
        assert_eq!(GateDecision::Permit.to_string(), "permit");
        assert_eq!(GateDecision::Defer.to_string(), "defer");
        assert_eq!(GateDecision::Deny.to_string(), "deny");
    }

    #[test]
    fn test_default_thresholds() {
        let thresholds = GateThresholds::default();
        assert_eq!(thresholds.tau_deny, 0.01);
        assert_eq!(thresholds.tau_permit, 100.0);
        assert_eq!(thresholds.min_cut, 5.0);
    }

    #[test]
    fn test_three_filter_decision() {
        let thresholds = GateThresholds::default();
        let decision = ThreeFilterDecision::new(thresholds);

        // Default graph should permit
        let graph = ReducedGraph::new();
        let outcome = decision.evaluate(&graph);

        // Default graph has high coherence, should permit
        assert_eq!(outcome.decision, GateDecision::Permit);
    }

    #[test]
    fn test_structural_denial() {
        let thresholds = GateThresholds::default();
        let decision = ThreeFilterDecision::new(thresholds);

        let mut graph = ReducedGraph::new();
        graph.set_global_cut(1.0); // Below min_cut of 5.0

        let outcome = decision.evaluate(&graph);
        assert_eq!(outcome.decision, GateDecision::Deny);
        assert_eq!(outcome.rejected_by, Some(DecisionFilter::Structural));
    }

    #[test]
    fn test_shift_deferral() {
        let thresholds = GateThresholds::default();
        let decision = ThreeFilterDecision::new(thresholds);

        let mut graph = ReducedGraph::new();
        graph.set_shift_pressure(0.8); // Above max_shift of 0.5

        let outcome = decision.evaluate(&graph);
        assert_eq!(outcome.decision, GateDecision::Defer);
        assert_eq!(outcome.rejected_by, Some(DecisionFilter::Shift));
    }

    #[test]
    fn test_evidence_deferral() {
        let thresholds = GateThresholds::default();
        let decision = ThreeFilterDecision::new(thresholds);

        let mut graph = ReducedGraph::new();
        graph.set_evidence(50.0); // Between tau_deny (0.01) and tau_permit (100.0)

        let outcome = decision.evaluate(&graph);
        assert_eq!(outcome.decision, GateDecision::Defer);
        assert_eq!(outcome.rejected_by, Some(DecisionFilter::Evidence));
    }

    #[test]
    fn test_decision_outcome_creation() {
        let outcome = DecisionOutcome::permit(0.95, 1.0, 0.9, 0.95, 10.0);
        assert_eq!(outcome.decision, GateDecision::Permit);
        assert!(outcome.confidence > 0.9);
        assert!(outcome.rejected_by.is_none());
    }

    #[test]
    fn test_decision_filter_display() {
        assert_eq!(DecisionFilter::Structural.to_string(), "Structural");
        assert_eq!(DecisionFilter::Shift.to_string(), "Shift");
        assert_eq!(DecisionFilter::Evidence.to_string(), "Evidence");
    }

    #[test]
    fn test_baseline_observation() {
        let thresholds = GateThresholds::default();
        let mut decision = ThreeFilterDecision::new(thresholds);

        assert!(decision.baseline().is_none());

        decision.observe_mincut(10.0);
        decision.observe_mincut(12.0);
        decision.observe_mincut(8.0);

        assert!(decision.baseline().is_some());
        assert_eq!(decision.history_len(), 3);
    }
}
