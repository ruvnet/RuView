//! Cost Curve Compression Tracker and Acceleration Scoreboard
//!
//! Measures whether cost curves compress faster in each new domain.
//! If they do, you are increasing general problem-solving capability.
//!
//! ## Acceptance Test
//!
//! Domain 2 must converge faster than Domain 1.
//! Measure cycles to reach:
//! - 95% accuracy
//! - Target cost per solve
//! - Target robustness
//! - Zero policy violations

use crate::domain::DomainId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single data point on the cost curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostCurvePoint {
    /// Cycle number (training iteration).
    pub cycle: u64,
    /// Current accuracy [0.0, 1.0].
    pub accuracy: f32,
    /// Cost per solve at this point.
    pub cost_per_solve: f32,
    /// Robustness score [0.0, 1.0].
    pub robustness: f32,
    /// Number of policy violations in this cycle.
    pub policy_violations: u32,
    /// Wall-clock timestamp (seconds since epoch).
    pub timestamp: f64,
}

/// Convergence thresholds for the acceptance test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergenceThresholds {
    /// Target accuracy (default: 0.95).
    pub target_accuracy: f32,
    /// Target cost per solve.
    pub target_cost: f32,
    /// Target robustness (default: 0.90).
    pub target_robustness: f32,
    /// Maximum allowed policy violations (default: 0).
    pub max_violations: u32,
}

impl Default for ConvergenceThresholds {
    fn default() -> Self {
        Self {
            target_accuracy: 0.95,
            target_cost: 0.01,
            target_robustness: 0.90,
            max_violations: 0,
        }
    }
}

/// Cost curve for a single domain, tracking convergence over cycles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostCurve {
    /// Domain this curve belongs to.
    pub domain_id: DomainId,
    /// Whether this was trained with transfer priors.
    pub used_transfer: bool,
    /// Source domain for transfer (if any).
    pub transfer_source: Option<DomainId>,
    /// Ordered data points.
    pub points: Vec<CostCurvePoint>,
    /// Convergence thresholds.
    pub thresholds: ConvergenceThresholds,
}

impl CostCurve {
    /// Create a new cost curve for a domain.
    pub fn new(domain_id: DomainId, thresholds: ConvergenceThresholds) -> Self {
        Self {
            domain_id,
            used_transfer: false,
            transfer_source: None,
            points: Vec::new(),
            thresholds,
        }
    }

    /// Create a cost curve with transfer metadata.
    pub fn with_transfer(
        domain_id: DomainId,
        source: DomainId,
        thresholds: ConvergenceThresholds,
    ) -> Self {
        Self {
            domain_id,
            used_transfer: true,
            transfer_source: Some(source),
            points: Vec::new(),
            thresholds,
        }
    }

    /// Record a new data point.
    pub fn record(&mut self, point: CostCurvePoint) {
        self.points.push(point);
    }

    /// Check if all convergence criteria are met at the latest point.
    pub fn has_converged(&self) -> bool {
        self.points.last().map_or(false, |p| {
            p.accuracy >= self.thresholds.target_accuracy
                && p.cost_per_solve <= self.thresholds.target_cost
                && p.robustness >= self.thresholds.target_robustness
                && p.policy_violations <= self.thresholds.max_violations
        })
    }

    /// Cycles to reach target accuracy (None if not yet reached).
    pub fn cycles_to_accuracy(&self) -> Option<u64> {
        self.points
            .iter()
            .find(|p| p.accuracy >= self.thresholds.target_accuracy)
            .map(|p| p.cycle)
    }

    /// Cycles to reach target cost (None if not yet reached).
    pub fn cycles_to_cost(&self) -> Option<u64> {
        self.points
            .iter()
            .find(|p| p.cost_per_solve <= self.thresholds.target_cost)
            .map(|p| p.cycle)
    }

    /// Cycles to reach target robustness.
    pub fn cycles_to_robustness(&self) -> Option<u64> {
        self.points
            .iter()
            .find(|p| p.robustness >= self.thresholds.target_robustness)
            .map(|p| p.cycle)
    }

    /// Cycles to full convergence (all criteria met).
    pub fn cycles_to_convergence(&self) -> Option<u64> {
        self.points
            .iter()
            .find(|p| {
                p.accuracy >= self.thresholds.target_accuracy
                    && p.cost_per_solve <= self.thresholds.target_cost
                    && p.robustness >= self.thresholds.target_robustness
                    && p.policy_violations <= self.thresholds.max_violations
            })
            .map(|p| p.cycle)
    }

    /// Area under the accuracy curve (higher = faster learning).
    pub fn auc_accuracy(&self) -> f32 {
        if self.points.len() < 2 {
            return 0.0;
        }
        self.points
            .windows(2)
            .map(|w| {
                let dx = (w[1].cycle - w[0].cycle) as f32;
                let avg_y = (w[0].accuracy + w[1].accuracy) / 2.0;
                dx * avg_y
            })
            .sum()
    }

    /// Compression ratio: how fast the cost curve drops.
    /// Computed as initial_cost / final_cost (higher = more compression).
    pub fn compression_ratio(&self) -> f32 {
        if self.points.len() < 2 {
            return 1.0;
        }
        let initial = self.points.first().unwrap().cost_per_solve;
        let final_cost = self.points.last().unwrap().cost_per_solve;
        if final_cost > 1e-10 {
            initial / final_cost
        } else {
            initial / 1e-10
        }
    }
}

/// Acceleration scoreboard comparing domain learning curves.
/// Shows acceleration, not just improvement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccelerationScoreboard {
    /// Per-domain cost curves.
    pub curves: HashMap<DomainId, CostCurve>,
    /// Pairwise acceleration factors.
    pub accelerations: Vec<AccelerationEntry>,
}

/// An entry showing how transfer from source to target affected convergence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccelerationEntry {
    /// Source domain.
    pub source: DomainId,
    /// Target domain.
    pub target: DomainId,
    /// Cycles to convergence without transfer (baseline).
    pub baseline_cycles: Option<u64>,
    /// Cycles to convergence with transfer.
    pub transfer_cycles: Option<u64>,
    /// Acceleration factor: baseline / transfer (>1 = transfer helped).
    pub acceleration: f32,
    /// AUC comparison (higher = better learning curve).
    pub auc_baseline: f32,
    pub auc_transfer: f32,
    /// Compression ratio comparison.
    pub compression_baseline: f32,
    pub compression_transfer: f32,
    /// Whether generalization test passed.
    pub generalization_passed: bool,
}

impl AccelerationScoreboard {
    pub fn new() -> Self {
        Self {
            curves: HashMap::new(),
            accelerations: Vec::new(),
        }
    }

    /// Add a cost curve for a domain.
    pub fn add_curve(&mut self, curve: CostCurve) {
        self.curves.insert(curve.domain_id.clone(), curve);
    }

    /// Compute acceleration between a baseline (no transfer) and transfer curve.
    pub fn compute_acceleration(
        &mut self,
        baseline_domain: &DomainId,
        transfer_domain: &DomainId,
    ) -> Option<AccelerationEntry> {
        let baseline = self.curves.get(baseline_domain)?;
        let transfer = self.curves.get(transfer_domain)?;

        let baseline_cycles = baseline.cycles_to_convergence();
        let transfer_cycles = transfer.cycles_to_convergence();

        let acceleration = match (baseline_cycles, transfer_cycles) {
            (Some(b), Some(t)) if t > 0 => b as f32 / t as f32,
            _ => 1.0, // No measurable acceleration
        };

        let entry = AccelerationEntry {
            source: transfer
                .transfer_source
                .clone()
                .unwrap_or_else(|| DomainId("none".into())),
            target: transfer_domain.clone(),
            baseline_cycles,
            transfer_cycles,
            acceleration,
            auc_baseline: baseline.auc_accuracy(),
            auc_transfer: transfer.auc_accuracy(),
            compression_baseline: baseline.compression_ratio(),
            compression_transfer: transfer.compression_ratio(),
            generalization_passed: acceleration > 1.0,
        };

        self.accelerations.push(entry.clone());
        Some(entry)
    }

    /// Check whether each successive domain converges faster (the IQ growth test).
    pub fn progressive_acceleration(&self) -> bool {
        if self.accelerations.len() < 2 {
            return true; // Not enough data to judge
        }

        self.accelerations
            .windows(2)
            .all(|w| w[1].acceleration >= w[0].acceleration)
    }

    /// Summary report of all domains.
    pub fn summary(&self) -> ScoreboardSummary {
        let domain_summaries: Vec<DomainSummary> = self
            .curves
            .iter()
            .map(|(id, curve)| DomainSummary {
                domain_id: id.clone(),
                total_cycles: curve.points.last().map(|p| p.cycle).unwrap_or(0),
                final_accuracy: curve.points.last().map(|p| p.accuracy).unwrap_or(0.0),
                final_cost: curve
                    .points
                    .last()
                    .map(|p| p.cost_per_solve)
                    .unwrap_or(f32::MAX),
                converged: curve.has_converged(),
                cycles_to_convergence: curve.cycles_to_convergence(),
                compression_ratio: curve.compression_ratio(),
                used_transfer: curve.used_transfer,
            })
            .collect();

        let overall_acceleration = if self.accelerations.is_empty() {
            1.0
        } else {
            self.accelerations
                .iter()
                .map(|a| a.acceleration)
                .sum::<f32>()
                / self.accelerations.len() as f32
        };

        ScoreboardSummary {
            domains: domain_summaries,
            accelerations: self.accelerations.clone(),
            overall_acceleration,
            progressive_improvement: self.progressive_acceleration(),
        }
    }
}

impl Default for AccelerationScoreboard {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of a single domain's learning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainSummary {
    pub domain_id: DomainId,
    pub total_cycles: u64,
    pub final_accuracy: f32,
    pub final_cost: f32,
    pub converged: bool,
    pub cycles_to_convergence: Option<u64>,
    pub compression_ratio: f32,
    pub used_transfer: bool,
}

/// Full scoreboard summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreboardSummary {
    pub domains: Vec<DomainSummary>,
    pub accelerations: Vec<AccelerationEntry>,
    pub overall_acceleration: f32,
    /// True if each new domain converges faster than the previous.
    pub progressive_improvement: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_curve(domain: &str, transfer: bool, accuracy_steps: &[(u64, f32, f32)]) -> CostCurve {
        let mut curve = if transfer {
            CostCurve::with_transfer(
                DomainId(domain.into()),
                DomainId("source".into()),
                ConvergenceThresholds::default(),
            )
        } else {
            CostCurve::new(DomainId(domain.into()), ConvergenceThresholds::default())
        };

        for &(cycle, accuracy, cost) in accuracy_steps {
            curve.record(CostCurvePoint {
                cycle,
                accuracy,
                cost_per_solve: cost,
                robustness: accuracy * 0.95,
                policy_violations: 0,
                timestamp: cycle as f64,
            });
        }
        curve
    }

    #[test]
    fn test_cost_curve_convergence() {
        let curve = make_curve(
            "test",
            false,
            &[
                (0, 0.3, 0.1),
                (10, 0.6, 0.05),
                (20, 0.8, 0.02),
                (30, 0.95, 0.008),
            ],
        );

        assert!(curve.has_converged());
        assert_eq!(curve.cycles_to_accuracy(), Some(30));
        assert_eq!(curve.cycles_to_cost(), Some(30));
    }

    #[test]
    fn test_cost_curve_not_converged() {
        let curve = make_curve("test", false, &[(0, 0.3, 0.1), (10, 0.6, 0.05)]);

        assert!(!curve.has_converged());
        assert_eq!(curve.cycles_to_accuracy(), None);
    }

    #[test]
    fn test_compression_ratio() {
        let curve = make_curve(
            "test",
            false,
            &[(0, 0.3, 1.0), (10, 0.6, 0.5), (20, 0.9, 0.1)],
        );

        let ratio = curve.compression_ratio();
        assert!((ratio - 10.0).abs() < 1e-4); // 1.0 / 0.1 = 10x
    }

    #[test]
    fn test_acceleration_scoreboard() {
        let mut board = AccelerationScoreboard::new();

        // Domain 1: baseline (slow convergence)
        let baseline = make_curve(
            "d1_baseline",
            false,
            &[
                (0, 0.2, 0.1),
                (20, 0.5, 0.05),
                (50, 0.8, 0.02),
                (100, 0.95, 0.008),
            ],
        );

        // Domain 2: with transfer (fast convergence)
        let transfer = make_curve(
            "d2_transfer",
            true,
            &[
                (0, 0.4, 0.08),
                (10, 0.7, 0.03),
                (20, 0.9, 0.01),
                (40, 0.96, 0.007),
            ],
        );

        board.add_curve(baseline);
        board.add_curve(transfer);

        let entry = board
            .compute_acceleration(
                &DomainId("d1_baseline".into()),
                &DomainId("d2_transfer".into()),
            )
            .unwrap();

        assert!(entry.acceleration > 1.0, "Transfer should accelerate");
        assert_eq!(entry.baseline_cycles, Some(100));
        assert_eq!(entry.transfer_cycles, Some(40));
        assert!((entry.acceleration - 2.5).abs() < 1e-4);
        assert!(entry.generalization_passed);
    }

    #[test]
    fn test_scoreboard_summary() {
        let mut board = AccelerationScoreboard::new();
        let curve = make_curve("d1", false, &[(0, 0.5, 0.1), (50, 0.96, 0.005)]);
        board.add_curve(curve);

        let summary = board.summary();
        assert_eq!(summary.domains.len(), 1);
        assert!(summary.domains[0].converged);
    }

    #[test]
    fn test_auc_accuracy() {
        let curve = make_curve(
            "test",
            false,
            &[(0, 0.0, 1.0), (10, 0.5, 0.5), (20, 1.0, 0.1)],
        );

        let auc = curve.auc_accuracy();
        // Trapezoid: (10*(0+0.5)/2) + (10*(0.5+1.0)/2) = 2.5 + 7.5 = 10.0
        assert!((auc - 10.0).abs() < 1e-4);
    }
}
