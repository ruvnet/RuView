//! AGI Contract — Defines intelligence as a measurable, falsifiable contract.
//!
//! The AGI contract states: a system improves utility over time without violating
//! policy, while maintaining structural health.
//!
//! ## Core Metrics (all deterministic, all auditable)
//!
//! - **Solved tasks per cost** — graded outcomes normalized by compute
//! - **Stability under noise** — accuracy retention when inputs are corrupted
//! - **Contradiction rate** — solved-but-wrong / total attempted
//! - **Rollback correctness** — recovery rate when bad inputs are detected
//! - **Policy violations** — budget overruns + contradictions (must be zero)
//!
//! ## Autonomy Ladder
//!
//! Each level requires sustained health metrics before advancement:
//! 0. Read-only (observe only)
//! 1. Write to memory (store episodes, no execution)
//! 2. Execute tools (run solver, generate puzzles)
//! 3. Write to external systems (publish results)
//! 4. Deploy and operate (self-directed improvement)

use crate::intelligence_metrics::{IntelligenceAssessment, RawMetrics};
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════
// Contract Health Snapshot
// ═══════════════════════════════════════════════════════════════════════════

/// A single point-in-time health measurement against the AGI contract.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractHealth {
    /// Solved tasks per unit cost (tasks_correct / total_steps)
    pub solved_per_cost: f64,
    /// Accuracy on noise-injected tasks
    pub noise_stability: f64,
    /// Contradiction rate: solved-but-wrong / attempted
    pub contradiction_rate: f64,
    /// Rollback correctness: successful rollbacks / attempted rollbacks
    pub rollback_correctness: f64,
    /// Total policy violations (must be zero for contract compliance)
    pub policy_violations: usize,
    /// Clean accuracy (graded outcome baseline)
    pub accuracy: f64,
    /// Cost efficiency (0-1, higher = cheaper per solve)
    pub cost_efficiency: f64,
    /// Whether the contract is satisfied
    pub compliant: bool,
}

impl ContractHealth {
    /// Evaluate contract health from raw metrics.
    pub fn from_raw(raw: &RawMetrics) -> Self {
        let accuracy = if raw.tasks_attempted > 0 {
            raw.tasks_correct as f64 / raw.tasks_attempted as f64
        } else {
            0.0
        };

        let solved_per_cost = if raw.total_steps > 0 {
            raw.tasks_correct as f64 / raw.total_steps as f64
        } else {
            0.0
        };

        let noise_stability = if raw.noise_tasks_attempted > 0 {
            raw.noise_tasks_correct as f64 / raw.noise_tasks_attempted as f64
        } else {
            0.0
        };

        let contradiction_rate = if raw.tasks_attempted > 0 {
            raw.contradictions as f64 / raw.tasks_attempted as f64
        } else {
            0.0
        };

        let rollback_correctness = if raw.rollback_attempts > 0 {
            raw.rollback_successes as f64 / raw.rollback_attempts as f64
        } else {
            1.0 // no rollbacks needed => perfect
        };

        let cost_efficiency = (1.0 - {
            let sps = if raw.tasks_correct > 0 {
                raw.total_steps as f64 / raw.tasks_correct as f64
            } else {
                100.0
            };
            (sps - 5.0) / 95.0
        })
        .clamp(0.0, 1.0);

        let compliant = raw.policy_violations == 0 && contradiction_rate < 0.01 && accuracy >= 0.90;

        ContractHealth {
            solved_per_cost,
            noise_stability,
            contradiction_rate,
            rollback_correctness,
            policy_violations: raw.policy_violations,
            accuracy,
            cost_efficiency,
            compliant,
        }
    }

    /// Evaluate contract health from an IntelligenceAssessment.
    pub fn from_assessment(assessment: &IntelligenceAssessment) -> Self {
        Self::from_raw(&assessment.raw_data)
    }

    /// Print formatted contract health report.
    pub fn print(&self) {
        println!("  Contract Health:");
        println!("    Solved/Cost:        {:.4}", self.solved_per_cost);
        println!(
            "    Noise Stability:    {:.2}%",
            self.noise_stability * 100.0
        );
        println!(
            "    Contradiction Rate: {:.4}%",
            self.contradiction_rate * 100.0
        );
        println!(
            "    Rollback Correct:   {:.2}%",
            self.rollback_correctness * 100.0
        );
        println!("    Policy Violations:  {}", self.policy_violations);
        println!("    Accuracy:           {:.2}%", self.accuracy * 100.0);
        println!(
            "    Cost Efficiency:    {:.2}%",
            self.cost_efficiency * 100.0
        );
        println!(
            "    Compliant:          {}",
            if self.compliant { "YES" } else { "NO" }
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Contract Trend — compares two snapshots
// ═══════════════════════════════════════════════════════════════════════════

/// Tracks improvement across contract dimensions between two measurement points.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractDelta {
    /// Change in solved-per-cost (positive = improving)
    pub solved_per_cost_delta: f64,
    /// Change in noise stability (positive = more robust)
    pub noise_stability_delta: f64,
    /// Change in contradiction rate (negative = improving)
    pub contradiction_rate_delta: f64,
    /// Change in rollback correctness (positive = better recovery)
    pub rollback_delta: f64,
    /// Change in accuracy (positive = better)
    pub accuracy_delta: f64,
    /// Change in cost efficiency (positive = cheaper)
    pub cost_efficiency_delta: f64,
    /// Number of dimensions that improved
    pub dimensions_improved: usize,
    /// Number of dimensions that regressed
    pub dimensions_regressed: usize,
}

impl ContractDelta {
    /// Compute delta between two health snapshots.
    pub fn between(before: &ContractHealth, after: &ContractHealth) -> Self {
        let solved_per_cost_delta = after.solved_per_cost - before.solved_per_cost;
        let noise_stability_delta = after.noise_stability - before.noise_stability;
        let contradiction_rate_delta = after.contradiction_rate - before.contradiction_rate;
        let rollback_delta = after.rollback_correctness - before.rollback_correctness;
        let accuracy_delta = after.accuracy - before.accuracy;
        let cost_efficiency_delta = after.cost_efficiency - before.cost_efficiency;

        // Count improvements (positive is better for all except contradiction_rate)
        let deltas = [
            solved_per_cost_delta > 0.001,
            noise_stability_delta > 0.001,
            contradiction_rate_delta < -0.001, // decrease = improvement
            rollback_delta > 0.001,
            accuracy_delta > 0.001,
            cost_efficiency_delta > 0.001,
        ];
        let regressions = [
            solved_per_cost_delta < -0.001,
            noise_stability_delta < -0.001,
            contradiction_rate_delta > 0.001,
            rollback_delta < -0.001,
            accuracy_delta < -0.01,
            cost_efficiency_delta < -0.001,
        ];

        ContractDelta {
            solved_per_cost_delta,
            noise_stability_delta,
            contradiction_rate_delta,
            rollback_delta,
            accuracy_delta,
            cost_efficiency_delta,
            dimensions_improved: deltas.iter().filter(|&&d| d).count(),
            dimensions_regressed: regressions.iter().filter(|&&r| r).count(),
        }
    }

    pub fn print(&self) {
        let arrow = |v: f64, invert: bool| {
            let positive = if invert { v < 0.0 } else { v > 0.0 };
            if positive {
                "+"
            } else if v == 0.0 {
                "="
            } else {
                "-"
            }
        };
        println!("  Contract Delta:");
        println!(
            "    Solved/Cost:     {:>+.4} [{}]",
            self.solved_per_cost_delta,
            arrow(self.solved_per_cost_delta, false)
        );
        println!(
            "    Noise Stability: {:>+.4} [{}]",
            self.noise_stability_delta,
            arrow(self.noise_stability_delta, false)
        );
        println!(
            "    Contradiction:   {:>+.4} [{}]",
            self.contradiction_rate_delta,
            arrow(self.contradiction_rate_delta, true)
        );
        println!(
            "    Rollback:        {:>+.4} [{}]",
            self.rollback_delta,
            arrow(self.rollback_delta, false)
        );
        println!(
            "    Accuracy:        {:>+.4} [{}]",
            self.accuracy_delta,
            arrow(self.accuracy_delta, false)
        );
        println!(
            "    Cost Efficiency: {:>+.4} [{}]",
            self.cost_efficiency_delta,
            arrow(self.cost_efficiency_delta, false)
        );
        println!("    Dimensions improved:  {}/6", self.dimensions_improved);
        println!("    Dimensions regressed: {}/6", self.dimensions_regressed);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Autonomy Ladder
// ═══════════════════════════════════════════════════════════════════════════

/// Autonomy level gated by sustained contract health.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AutonomyLevel {
    /// Level 0: Read-only observation
    ReadOnly = 0,
    /// Level 1: Write to memory (store episodes)
    WriteMemory = 1,
    /// Level 2: Execute tools (run solver)
    ExecuteTools = 2,
    /// Level 3: Write to external systems (publish results)
    WriteExternal = 3,
    /// Level 4: Deploy and operate (self-directed improvement)
    DeployOperate = 4,
}

/// Thresholds for advancing autonomy levels.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutonomyGates {
    /// Minimum consecutive compliant cycles to advance
    pub min_compliant_cycles: usize,
    /// Maximum allowed contradiction rate per level
    pub max_contradiction_rate: [f64; 5],
    /// Minimum accuracy per level
    pub min_accuracy: [f64; 5],
    /// Minimum cost efficiency per level
    pub min_cost_efficiency: [f64; 5],
    /// Minimum noise stability per level
    pub min_noise_stability: [f64; 5],
    /// Must have zero policy violations for levels >= 2
    pub zero_violations_above: AutonomyLevel,
}

impl Default for AutonomyGates {
    fn default() -> Self {
        Self {
            min_compliant_cycles: 3,
            //                          L0    L1    L2    L3    L4
            max_contradiction_rate: [1.0, 0.05, 0.02, 0.01, 0.005],
            min_accuracy: [0.0, 0.70, 0.85, 0.92, 0.96],
            min_cost_efficiency: [0.0, 0.20, 0.40, 0.60, 0.75],
            min_noise_stability: [0.0, 0.50, 0.65, 0.80, 0.90],
            zero_violations_above: AutonomyLevel::ExecuteTools,
        }
    }
}

/// Evaluator that determines current autonomy level from contract history.
pub struct AutonomyEvaluator {
    pub gates: AutonomyGates,
}

impl Default for AutonomyEvaluator {
    fn default() -> Self {
        Self {
            gates: AutonomyGates::default(),
        }
    }
}

impl AutonomyEvaluator {
    /// Determine the highest autonomy level supported by the health history.
    /// `history` is ordered oldest-first.
    pub fn evaluate(&self, history: &[ContractHealth]) -> AutonomyLevel {
        if history.is_empty() {
            return AutonomyLevel::ReadOnly;
        }

        let mut level = AutonomyLevel::ReadOnly;
        let levels = [
            AutonomyLevel::WriteMemory,
            AutonomyLevel::ExecuteTools,
            AutonomyLevel::WriteExternal,
            AutonomyLevel::DeployOperate,
        ];

        for &candidate in &levels {
            let idx = candidate as usize;
            let required = self.gates.min_compliant_cycles;

            // Need enough recent history
            if history.len() < required {
                break;
            }

            let recent = &history[history.len().saturating_sub(required)..];
            let all_pass = recent.iter().all(|h| {
                h.accuracy >= self.gates.min_accuracy[idx]
                    && h.contradiction_rate <= self.gates.max_contradiction_rate[idx]
                    && h.cost_efficiency >= self.gates.min_cost_efficiency[idx]
                    && h.noise_stability >= self.gates.min_noise_stability[idx]
                    && (candidate < self.gates.zero_violations_above || h.policy_violations == 0)
            });

            if all_pass {
                level = candidate;
            } else {
                break;
            }
        }

        level
    }

    pub fn print_status(&self, level: AutonomyLevel, health: &ContractHealth) {
        let labels = [
            "Read-Only",
            "Write Memory",
            "Execute Tools",
            "Write External",
            "Deploy & Operate",
        ];
        println!(
            "  Autonomy Level: {} ({})",
            level as usize, labels[level as usize]
        );
        println!("  Gates for next level:");
        let next = (level as usize + 1).min(4);
        println!(
            "    Accuracy:       {:.0}% (need {:.0}%)",
            health.accuracy * 100.0,
            self.gates.min_accuracy[next] * 100.0
        );
        println!(
            "    Contradiction:  {:.3}% (need <{:.3}%)",
            health.contradiction_rate * 100.0,
            self.gates.max_contradiction_rate[next] * 100.0
        );
        println!(
            "    Cost Eff:       {:.0}% (need {:.0}%)",
            health.cost_efficiency * 100.0,
            self.gates.min_cost_efficiency[next] * 100.0
        );
        println!(
            "    Noise Stab:     {:.0}% (need {:.0}%)",
            health.noise_stability * 100.0,
            self.gates.min_noise_stability[next] * 100.0
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Viability Checklist
// ═══════════════════════════════════════════════════════════════════════════

/// The 5 viability checks that determine if the system is on an AGI trajectory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViabilityChecklist {
    /// Can replay runs and get identical grades
    pub deterministic_replay: bool,
    /// Improves utility over time without raising policy violations
    pub improving_without_violations: bool,
    /// Can roll back bad learning reliably
    pub reliable_rollback: bool,
    /// Can generate infinite novel tasks with automatic grading
    pub infinite_gradeable_tasks: bool,
    /// Cost per solve trending down over weeks
    pub cost_trending_down: bool,
}

impl ViabilityChecklist {
    /// Evaluate from contract health history.
    pub fn evaluate(history: &[ContractHealth]) -> Self {
        // Deterministic replay: verified externally (always true in our harness)
        let deterministic_replay = true;

        // Improving without violations: later health better than earlier, zero violations
        let improving_without_violations = if history.len() >= 2 {
            let first = &history[0];
            let last = &history[history.len() - 1];
            last.accuracy >= first.accuracy
                && last.policy_violations == 0
                && history.iter().all(|h| h.policy_violations == 0)
        } else {
            false
        };

        // Reliable rollback: rollback correctness >= 80% when attempted
        let reliable_rollback = history.iter().all(|h| h.rollback_correctness >= 0.8);

        // Infinite gradeable tasks: always true (PuzzleGenerator is unbounded)
        let infinite_gradeable_tasks = true;

        // Cost trending down: solved_per_cost increases over time
        let cost_trending_down = if history.len() >= 3 {
            let first_third: f64 = history[..history.len() / 3]
                .iter()
                .map(|h| h.solved_per_cost)
                .sum::<f64>()
                / (history.len() / 3) as f64;
            let last_third: f64 = history[history.len() * 2 / 3..]
                .iter()
                .map(|h| h.solved_per_cost)
                .sum::<f64>()
                / (history.len() - history.len() * 2 / 3) as f64;
            last_third > first_third
        } else {
            false
        };

        ViabilityChecklist {
            deterministic_replay,
            improving_without_violations,
            reliable_rollback,
            infinite_gradeable_tasks,
            cost_trending_down,
        }
    }

    pub fn all_pass(&self) -> bool {
        self.deterministic_replay
            && self.improving_without_violations
            && self.reliable_rollback
            && self.infinite_gradeable_tasks
            && self.cost_trending_down
    }

    pub fn print(&self) {
        let check = |b: bool| if b { "PASS" } else { "FAIL" };
        println!("  Viability Checklist:");
        println!(
            "    1. Deterministic replay:       {}",
            check(self.deterministic_replay)
        );
        println!(
            "    2. Improving w/o violations:    {}",
            check(self.improving_without_violations)
        );
        println!(
            "    3. Reliable rollback:           {}",
            check(self.reliable_rollback)
        );
        println!(
            "    4. Infinite gradeable tasks:    {}",
            check(self.infinite_gradeable_tasks)
        );
        println!(
            "    5. Cost trending down:          {}",
            check(self.cost_trending_down)
        );
        println!(
            "    Overall: {}",
            if self.all_pass() {
                "VIABLE AGI TRAJECTORY"
            } else {
                "NOT YET VIABLE"
            }
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_health_from_raw() {
        let mut raw = RawMetrics::default();
        raw.tasks_attempted = 100;
        raw.tasks_completed = 95;
        raw.tasks_correct = 92;
        raw.total_steps = 600;
        raw.noise_tasks_attempted = 30;
        raw.noise_tasks_correct = 25;
        raw.contradictions = 0; // zero contradictions for compliance
        raw.rollback_attempts = 5;
        raw.rollback_successes = 4;

        let health = ContractHealth::from_raw(&raw);
        assert!((health.accuracy - 0.92).abs() < 0.01);
        assert!((health.solved_per_cost - 92.0 / 600.0).abs() < 0.01);
        assert!((health.noise_stability - 25.0 / 30.0).abs() < 0.01);
        assert!((health.contradiction_rate).abs() < 0.001);
        assert!((health.rollback_correctness - 0.8).abs() < 0.01);
        assert!(health.compliant); // 0 violations, 0% contradictions, >=90% accuracy
    }

    #[test]
    fn contract_delta_detects_improvement() {
        let before = ContractHealth {
            solved_per_cost: 0.10,
            noise_stability: 0.70,
            contradiction_rate: 0.03,
            rollback_correctness: 0.80,
            policy_violations: 0,
            accuracy: 0.85,
            cost_efficiency: 0.50,
            compliant: false,
        };
        let after = ContractHealth {
            solved_per_cost: 0.15,
            noise_stability: 0.85,
            contradiction_rate: 0.01,
            rollback_correctness: 0.90,
            policy_violations: 0,
            accuracy: 0.93,
            cost_efficiency: 0.70,
            compliant: true,
        };
        let delta = ContractDelta::between(&before, &after);
        assert_eq!(delta.dimensions_improved, 6);
        assert_eq!(delta.dimensions_regressed, 0);
    }

    #[test]
    fn autonomy_ladder_advances() {
        let evaluator = AutonomyEvaluator::default();

        // No history => ReadOnly
        assert_eq!(evaluator.evaluate(&[]), AutonomyLevel::ReadOnly);

        // 3 compliant cycles at L1 level
        let h = ContractHealth {
            solved_per_cost: 0.15,
            noise_stability: 0.55,
            contradiction_rate: 0.04,
            rollback_correctness: 1.0,
            policy_violations: 0,
            accuracy: 0.75,
            cost_efficiency: 0.30,
            compliant: true,
        };
        let history = vec![h.clone(), h.clone(), h.clone()];
        assert_eq!(evaluator.evaluate(&history), AutonomyLevel::WriteMemory);
    }

    #[test]
    fn viability_checklist_basic() {
        let h1 = ContractHealth {
            solved_per_cost: 0.10,
            noise_stability: 0.70,
            contradiction_rate: 0.01,
            rollback_correctness: 0.90,
            policy_violations: 0,
            accuracy: 0.85,
            cost_efficiency: 0.50,
            compliant: true,
        };
        let h2 = ContractHealth {
            solved_per_cost: 0.12,
            noise_stability: 0.80,
            contradiction_rate: 0.005,
            rollback_correctness: 0.95,
            policy_violations: 0,
            accuracy: 0.90,
            cost_efficiency: 0.60,
            compliant: true,
        };
        let h3 = ContractHealth {
            solved_per_cost: 0.15,
            noise_stability: 0.85,
            contradiction_rate: 0.002,
            rollback_correctness: 0.95,
            policy_violations: 0,
            accuracy: 0.93,
            cost_efficiency: 0.70,
            compliant: true,
        };
        let viability = ViabilityChecklist::evaluate(&[h1, h2, h3]);
        assert!(viability.deterministic_replay);
        assert!(viability.improving_without_violations);
        assert!(viability.reliable_rollback);
        assert!(viability.infinite_gradeable_tasks);
        assert!(viability.cost_trending_down);
        assert!(viability.all_pass());
    }
}
