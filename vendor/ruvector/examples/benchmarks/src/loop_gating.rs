//! Three-Loop Gating Architecture
//!
//! Separates the intelligence engine into three explicit loops with strict gating:
//!
//! ## Fast Loop (per step)
//! - Runs every step of every solver invocation
//! - No planning, no model calls
//! - Only checks invariants: allow, block, quarantine, or rollback
//! - Outputs: GateDecision, HealthDelta, WitnessRecord
//!
//! ## Medium Loop (per attempt)
//! - Runs per solve attempt (one puzzle)
//! - Multi-strategy solver, ensemble vote, cascade passes
//! - Can PROPOSE memory writes, but cannot COMMIT them
//! - Outputs: CandidateSolution, AttemptTrace, ProposedMemoryWrites
//!
//! ## Slow Loop (per cycle)
//! - Runs per training/evaluation cycle
//! - Consolidation, compiler updates, promotion review, meta parameter updates
//! - Only component that can PROMOTE patterns (Volatile → Trusted)
//! - Outputs: NewPolicyCheckpoint, NewMemoryRoot, PromotionLog
//!
//! ## Critical Gating Rule
//! Medium loop can propose memory writes.
//! Fast loop is the only component allowed to commit them.
//! Slow loop is the only component allowed to promote them.

use serde::{Deserialize, Serialize};

use crate::agi_contract::ContractHealth;
use crate::reasoning_bank::{
    Counterexample, MemoryCheckpoint, MemoryClass, ReasoningBank, RollbackWitness, Trajectory,
    Verdict,
};

// ═══════════════════════════════════════════════════════════════════════════
// Fast Loop: per-step invariant gating
// ═══════════════════════════════════════════════════════════════════════════

/// Decision made by the fast loop gate on each step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GateDecision {
    /// Allow the step to proceed
    Allow,
    /// Block: step would violate a policy
    Block { reason: String },
    /// Quarantine: result is suspicious, hold for review
    Quarantine { reason: String },
    /// Rollback: regression detected, revert to checkpoint
    Rollback {
        checkpoint_id: usize,
        reason: String,
    },
}

/// Health delta tracked per step.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HealthDelta {
    pub steps_taken: usize,
    pub contradictions_detected: usize,
    pub policy_violations: usize,
    pub cost_accumulated: f64,
}

/// Fast loop gate: checks invariants on every step.
/// This is the ONLY component allowed to commit memory writes.
#[derive(Clone, Debug)]
pub struct FastGate {
    /// Maximum steps before forced halt
    pub step_limit: usize,
    /// Maximum cost accumulation before halt
    pub cost_limit: f64,
    /// Contradiction threshold before quarantine
    pub contradiction_threshold: usize,
    /// Running health delta
    pub delta: HealthDelta,
    /// Pending writes from medium loop (committed by fast loop)
    pub pending_writes: Vec<ProposedWrite>,
    /// Gate decisions log
    pub decisions: Vec<GateDecision>,
}

impl FastGate {
    pub fn new(step_limit: usize) -> Self {
        Self {
            step_limit,
            cost_limit: f64::MAX,
            contradiction_threshold: 3,
            delta: HealthDelta::default(),
            pending_writes: Vec::new(),
            decisions: Vec::new(),
        }
    }

    /// Check a step and return a gate decision.
    pub fn check_step(&mut self, step: usize, solved: bool, correct: bool) -> GateDecision {
        self.delta.steps_taken = step;

        // Check step budget
        if step >= self.step_limit {
            let decision = GateDecision::Block {
                reason: format!("step budget exhausted ({}/{})", step, self.step_limit),
            };
            self.decisions.push(decision.clone());
            return decision;
        }

        // Check contradiction (solved but wrong)
        if solved && !correct {
            self.delta.contradictions_detected += 1;
            if self.delta.contradictions_detected >= self.contradiction_threshold {
                let decision = GateDecision::Quarantine {
                    reason: format!(
                        "{} contradictions in this attempt",
                        self.delta.contradictions_detected,
                    ),
                };
                self.decisions.push(decision.clone());
                return decision;
            }
        }

        let decision = GateDecision::Allow;
        self.decisions.push(decision.clone());
        decision
    }

    /// Commit pending writes from the medium loop into the bank.
    /// Only the fast loop has authority to do this.
    pub fn commit_writes(&mut self, bank: &mut ReasoningBank) -> usize {
        let count = self.pending_writes.len();
        for write in self.pending_writes.drain(..) {
            match write {
                ProposedWrite::RecordTrajectory(traj) => {
                    bank.record_trajectory_gated(traj);
                }
                ProposedWrite::RecordCounterexample {
                    constraint_type,
                    trajectory,
                } => {
                    bank.record_counterexample(&constraint_type, trajectory);
                }
                ProposedWrite::QuarantineTrajectory { trajectory, reason } => {
                    bank.quarantine_trajectory(trajectory, &reason);
                }
            }
        }
        count
    }

    /// Reset for next attempt.
    pub fn reset(&mut self) {
        self.delta = HealthDelta::default();
        self.decisions.clear();
    }
}

/// A proposed memory write from the medium loop.
/// Cannot be committed directly — must go through FastGate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProposedWrite {
    RecordTrajectory(Trajectory),
    RecordCounterexample {
        constraint_type: String,
        trajectory: Trajectory,
    },
    QuarantineTrajectory {
        trajectory: Trajectory,
        reason: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════════
// Medium Loop: per-attempt solving
// ═══════════════════════════════════════════════════════════════════════════

/// Trace of a single solve attempt.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttemptTrace {
    /// Puzzle ID
    pub puzzle_id: String,
    /// Strategy used
    pub strategy: String,
    /// Steps taken
    pub steps: usize,
    /// Whether the answer was correct
    pub correct: bool,
    /// Whether a retry was attempted
    pub retried: bool,
    /// Gate decisions during this attempt
    pub gate_decisions: Vec<GateDecision>,
    /// Proposed memory writes (not yet committed)
    pub proposed_writes: Vec<ProposedWrite>,
}

/// Medium loop: handles one puzzle solve attempt.
/// Can propose memory writes but cannot commit them.
pub struct MediumLoop {
    /// Fast gate for step-level invariant checking
    pub gate: FastGate,
}

impl MediumLoop {
    pub fn new(step_limit: usize) -> Self {
        Self {
            gate: FastGate::new(step_limit),
        }
    }

    /// Process a solve result and produce an attempt trace.
    /// Proposes memory writes but does NOT commit them.
    pub fn process_result(
        &mut self,
        puzzle_id: &str,
        difficulty: u8,
        strategy: &str,
        steps: usize,
        solved: bool,
        correct: bool,
        constraint_types: &[String],
    ) -> AttemptTrace {
        // Fast loop gate check
        let decision = self.gate.check_step(steps, solved, correct);

        let mut proposed_writes = Vec::new();

        // Build trajectory
        let mut traj = Trajectory::new(puzzle_id, difficulty);
        traj.constraint_types = constraint_types.to_vec();
        traj.record_attempt(
            if correct {
                "correct".to_string()
            } else {
                "incorrect".to_string()
            },
            if correct { 0.9 } else { 0.2 },
            steps,
            1,
            strategy,
        );
        traj.set_verdict(
            if correct {
                Verdict::Success
            } else {
                Verdict::Failed
            },
            None,
        );

        match decision {
            GateDecision::Allow => {
                // Propose recording the trajectory
                proposed_writes.push(ProposedWrite::RecordTrajectory(traj));
            }
            GateDecision::Block { .. } => {
                // Don't record — budget exhausted
            }
            GateDecision::Quarantine { ref reason } => {
                proposed_writes.push(ProposedWrite::QuarantineTrajectory {
                    trajectory: traj.clone(),
                    reason: reason.clone(),
                });
                for ct in constraint_types {
                    proposed_writes.push(ProposedWrite::RecordCounterexample {
                        constraint_type: ct.clone(),
                        trajectory: traj.clone(),
                    });
                }
            }
            GateDecision::Rollback { .. } => {
                // Rollback handled at fast loop level
            }
        }

        AttemptTrace {
            puzzle_id: puzzle_id.to_string(),
            strategy: strategy.to_string(),
            steps,
            correct,
            retried: false,
            gate_decisions: vec![decision],
            proposed_writes,
        }
    }

    /// Finalize: transfer proposed writes to fast gate for commitment.
    pub fn finalize(&mut self, trace: &AttemptTrace) {
        for write in &trace.proposed_writes {
            self.gate.pending_writes.push(write.clone());
        }
    }

    /// Reset for next attempt.
    pub fn reset(&mut self) {
        self.gate.reset();
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Slow Loop: per-cycle consolidation
// ═══════════════════════════════════════════════════════════════════════════

/// Log of pattern promotions during a cycle.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PromotionLog {
    /// Patterns promoted from Volatile → Trusted
    pub promoted: usize,
    /// Patterns demoted from Trusted → Quarantined
    pub demoted: usize,
    /// Patterns remaining in Volatile
    pub volatile_remaining: usize,
    /// Patterns in Trusted
    pub trusted_total: usize,
    /// Patterns in Quarantined
    pub quarantined_total: usize,
}

/// Result of a slow loop cycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CycleConsolidation {
    /// Cycle number
    pub cycle: usize,
    /// Checkpoint created at start of cycle
    pub checkpoint_id: usize,
    /// Promotion log
    pub promotion_log: PromotionLog,
    /// Contract health after consolidation
    pub contract_health: Option<ContractHealth>,
    /// Whether a rollback occurred
    pub rolled_back: bool,
    /// Rollback witness if rollback occurred
    pub rollback_witness: Option<RollbackWitness>,
}

/// Slow loop: handles per-cycle consolidation.
/// Only component allowed to promote patterns.
pub struct SlowLoop {
    /// History of consolidations
    pub history: Vec<CycleConsolidation>,
}

impl SlowLoop {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }

    /// Run consolidation: promote eligible patterns, demote failing ones.
    /// This is the ONLY place where pattern promotion happens.
    pub fn consolidate(
        &mut self,
        bank: &mut ReasoningBank,
        cycle: usize,
        checkpoint_id: usize,
        holdout_accuracy: f64,
        prev_accuracy: Option<f64>,
    ) -> CycleConsolidation {
        let mut rolled_back = false;
        let mut rollback_witness = None;

        // Check for regression — if accuracy dropped, rollback
        if let Some(prev) = prev_accuracy {
            if holdout_accuracy < prev - 0.05 {
                let ok = bank.rollback_with_witness(
                    checkpoint_id,
                    "slow loop: accuracy regression",
                    prev,
                    holdout_accuracy,
                );
                if ok {
                    rolled_back = true;
                    rollback_witness = bank.rollback_witnesses.last().cloned();
                }
            }
        }

        // Promote eligible patterns (requires counterexample)
        let promoted = bank.promote_patterns();

        let log = PromotionLog {
            promoted,
            demoted: 0, // Demotions happen in the fast loop
            volatile_remaining: bank.volatile_count(),
            trusted_total: bank.trusted_count(),
            quarantined_total: bank.quarantined_pattern_count(),
        };

        let consolidation = CycleConsolidation {
            cycle,
            checkpoint_id,
            promotion_log: log,
            contract_health: None,
            rolled_back,
            rollback_witness,
        };

        self.history.push(consolidation.clone());
        consolidation
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_gate_allows_normal_step() {
        let mut gate = FastGate::new(100);
        let decision = gate.check_step(5, false, false);
        assert_eq!(decision, GateDecision::Allow);
    }

    #[test]
    fn fast_gate_blocks_over_budget() {
        let mut gate = FastGate::new(10);
        let decision = gate.check_step(10, false, false);
        assert!(matches!(decision, GateDecision::Block { .. }));
    }

    #[test]
    fn fast_gate_quarantines_contradictions() {
        let mut gate = FastGate::new(100);
        gate.contradiction_threshold = 2;

        // First contradiction: still allowed
        let d1 = gate.check_step(1, true, false);
        assert_eq!(d1, GateDecision::Allow);

        // Second contradiction: quarantine
        let d2 = gate.check_step(2, true, false);
        assert!(matches!(d2, GateDecision::Quarantine { .. }));
    }

    #[test]
    fn fast_gate_commits_pending_writes() {
        let mut gate = FastGate::new(100);
        let mut bank = ReasoningBank::new();

        let mut traj = Trajectory::new("test_1", 5);
        traj.constraint_types.push("Before".to_string());
        traj.record_attempt("answer".into(), 0.9, 10, 1, "default");
        traj.set_verdict(Verdict::Success, None);

        gate.pending_writes
            .push(ProposedWrite::RecordTrajectory(traj));
        let committed = gate.commit_writes(&mut bank);
        assert_eq!(committed, 1);
        assert_eq!(bank.trajectories.len(), 1);
    }

    #[test]
    fn medium_loop_proposes_writes() {
        let mut medium = MediumLoop::new(100);

        let trace = medium.process_result(
            "puzzle_1",
            5,
            "adaptive",
            15,
            true,
            true,
            &["Before".to_string()],
        );

        assert!(trace.correct);
        assert_eq!(trace.proposed_writes.len(), 1);
        assert!(matches!(
            trace.proposed_writes[0],
            ProposedWrite::RecordTrajectory(_)
        ));
    }

    #[test]
    fn medium_loop_quarantines_contradictions() {
        let mut medium = MediumLoop::new(100);
        medium.gate.contradiction_threshold = 1;

        // Solved but wrong → quarantine (threshold 1)
        let trace = medium.process_result(
            "puzzle_1",
            5,
            "default",
            15,
            true,
            false,
            &["Month".to_string()],
        );

        assert!(!trace.correct);
        // Should have quarantine + counterexample writes
        assert!(trace.proposed_writes.len() >= 2);
        assert!(trace
            .proposed_writes
            .iter()
            .any(|w| matches!(w, ProposedWrite::QuarantineTrajectory { .. })));
    }

    #[test]
    fn slow_loop_promotes_patterns() {
        let mut bank = ReasoningBank::new();
        bank.evidence_threshold = 3;

        // Build enough observations
        for i in 0..5 {
            let mut traj = Trajectory::new(&format!("s_{}", i), 5);
            traj.constraint_types.push("Year".to_string());
            traj.record_attempt("2024".into(), 0.9, 10, 1, "default");
            traj.set_verdict(Verdict::Success, None);
            bank.record_trajectory(traj);
        }

        // Add counterexample (required for promotion)
        let ce_traj = Trajectory::new("fail_1", 5);
        bank.record_counterexample("Year", ce_traj);

        let cp = bank.checkpoint();

        let mut slow = SlowLoop::new();
        let result = slow.consolidate(&mut bank, 0, cp, 0.95, None);

        assert_eq!(result.promotion_log.promoted, 1);
        assert_eq!(result.promotion_log.trusted_total, 1);
        assert!(!result.rolled_back);
    }

    #[test]
    fn slow_loop_rolls_back_on_regression() {
        let mut bank = ReasoningBank::new();

        for i in 0..3 {
            let mut traj = Trajectory::new(&format!("r_{}", i), 5);
            traj.constraint_types.push("DayOfWeek".to_string());
            traj.record_attempt("answer".into(), 0.9, 10, 1, "default");
            traj.set_verdict(Verdict::Success, None);
            bank.record_trajectory(traj);
        }

        let cp = bank.checkpoint();

        // Simulate bad learning
        for i in 3..6 {
            let mut traj = Trajectory::new(&format!("r_{}", i), 5);
            traj.constraint_types.push("DayOfWeek".to_string());
            traj.record_attempt("wrong".into(), 0.1, 50, 1, "default");
            traj.set_verdict(Verdict::Failed, None);
            bank.record_trajectory(traj);
        }

        let mut slow = SlowLoop::new();
        // Previous accuracy 0.95, current 0.80 → regression > 0.05
        let result = slow.consolidate(&mut bank, 1, cp, 0.80, Some(0.95));

        assert!(result.rolled_back);
        assert!(result.rollback_witness.is_some());
        assert_eq!(bank.trajectories.len(), 3); // Rolled back to checkpoint
    }

    #[test]
    fn three_loop_integration() {
        let mut bank = ReasoningBank::new();
        bank.evidence_threshold = 2;

        // === Cycle 1 ===
        let cp = bank.checkpoint();

        // Medium loop: solve puzzles
        let mut medium = MediumLoop::new(100);

        for i in 0..5 {
            let trace = medium.process_result(
                &format!("p_{}", i),
                5,
                "adaptive",
                10,
                true,
                true,
                &["Before".to_string()],
            );
            medium.finalize(&trace);
        }

        // Fast loop: commit writes
        let committed = medium.gate.commit_writes(&mut bank);
        assert_eq!(committed, 5);
        medium.reset();

        // Add counterexample (for promotion eligibility)
        let ce = Trajectory::new("ce_1", 5);
        bank.record_counterexample("Before", ce);

        // Slow loop: consolidate
        let mut slow = SlowLoop::new();
        let consolidation = slow.consolidate(&mut bank, 0, cp, 0.90, None);

        assert!(consolidation.promotion_log.promoted > 0);
        assert_eq!(bank.trusted_count(), 1);
    }
}
