//! Hybrid classical-quantum control theory engine for QEC.
//!
//! Models the QEC feedback loop as a discrete-time control system:
//! `Physical qubits -> Syndrome extraction -> Classical decode -> Correction -> Repeat`
//!
//! If classical decoding latency exceeds the syndrome extraction period, errors
//! accumulate faster than they are corrected (the "backlog problem").

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

#[allow(unused_imports)]
use crate::error::{QuantumError, Result};

// -- 1. Control Loop Model --------------------------------------------------

/// Full QEC control loop: plant (quantum) + controller (classical) + state.
#[derive(Debug, Clone)]
pub struct QecControlLoop {
    pub plant: QuantumPlant,
    pub controller: ClassicalController,
    pub state: ControlState,
}

/// Physical parameters of the quantum error-correction code.
#[derive(Debug, Clone)]
pub struct QuantumPlant {
    pub code_distance: u32,
    pub physical_error_rate: f64,
    pub num_data_qubits: u32,
    pub coherence_time_ns: u64,
}

/// Classical decoder performance characteristics.
#[derive(Debug, Clone)]
pub struct ClassicalController {
    pub decode_latency_ns: u64,
    pub decode_throughput: f64,
    pub accuracy: f64,
}

/// Evolving state of the control loop during execution.
#[derive(Debug, Clone)]
pub struct ControlState {
    pub logical_error_rate: f64,
    pub error_backlog: f64,
    pub rounds_decoded: u64,
    pub total_latency_ns: u64,
}

impl ControlState {
    pub fn new() -> Self {
        Self {
            logical_error_rate: 0.0,
            error_backlog: 0.0,
            rounds_decoded: 0,
            total_latency_ns: 0,
        }
    }
}

impl Default for ControlState {
    fn default() -> Self {
        Self::new()
    }
}

// -- 2. Stability Analysis ---------------------------------------------------

/// Result of analyzing the control loop's stability.
#[derive(Debug, Clone)]
pub struct StabilityCondition {
    pub is_stable: bool,
    pub margin: f64,
    pub critical_latency_ns: u64,
    pub critical_error_rate: f64,
    pub convergence_rate: f64,
}

/// Syndrome extraction period (ns) for distance-d surface code.
/// 6 gate layers per cycle, ~20 ns per gate layer.
fn syndrome_period_ns(distance: u32) -> u64 {
    6 * (distance as u64) * 20
}

/// Analyze stability: the loop is stable when `decode_latency < syndrome_period`.
pub fn analyze_stability(config: &QecControlLoop) -> StabilityCondition {
    let d = config.plant.code_distance;
    let p = config.plant.physical_error_rate;
    let t_decode = config.controller.decode_latency_ns;
    let acc = config.controller.accuracy;
    let t_syndrome = syndrome_period_ns(d);

    let margin = if t_decode == 0 {
        f64::INFINITY
    } else {
        (t_syndrome as f64 / t_decode as f64) - 1.0
    };
    let is_stable = t_decode < t_syndrome;
    let critical_latency_ns = t_syndrome;
    let critical_error_rate = 0.01 * acc;
    let error_injection = p * (d as f64);
    let convergence_rate = if t_syndrome > 0 {
        1.0 - (t_decode as f64 / t_syndrome as f64) - error_injection
    } else {
        -1.0
    };

    StabilityCondition {
        is_stable,
        margin,
        critical_latency_ns,
        critical_error_rate,
        convergence_rate,
    }
}

/// Maximum code distance stable for a given controller and physical error rate.
/// Iterates odd distances 3, 5, 7, ... until latency exceeds syndrome period.
pub fn max_stable_distance(controller: &ClassicalController, error_rate: f64) -> u32 {
    let mut best = 3u32;
    for d in (3..=201).step_by(2) {
        if controller.decode_latency_ns >= syndrome_period_ns(d) {
            break;
        }
        if error_rate >= 0.01 * controller.accuracy {
            break;
        }
        best = d;
    }
    best
}

/// Minimum decoder throughput (syndromes/sec) to keep up with the plant.
pub fn min_throughput(plant: &QuantumPlant) -> f64 {
    let t_ns = syndrome_period_ns(plant.code_distance);
    if t_ns == 0 {
        return f64::INFINITY;
    }
    1e9 / t_ns as f64
}

// -- 3. Resource Optimization ------------------------------------------------

/// Available hardware resources.
#[derive(Debug, Clone)]
pub struct ResourceBudget {
    pub total_physical_qubits: u32,
    pub classical_cores: u32,
    pub classical_clock_ghz: f64,
    pub total_time_budget_us: u64,
}

/// A candidate allocation on the Pareto frontier.
#[derive(Debug, Clone)]
pub struct OptimalAllocation {
    pub code_distance: u32,
    pub logical_qubits: u32,
    pub decode_threads: u32,
    pub expected_logical_error_rate: f64,
    pub pareto_score: f64,
}

/// Enumerate Pareto-optimal resource allocations sorted by descending score.
pub fn optimize_allocation(
    budget: &ResourceBudget,
    error_rate: f64,
    min_logical: u32,
) -> Vec<OptimalAllocation> {
    let mut candidates = Vec::new();
    for d in (3u32..=99).step_by(2) {
        let qpl = 2 * d * d - 2 * d + 1;
        if qpl == 0 {
            continue;
        }
        let max_logical = budget.total_physical_qubits / qpl;
        if max_logical < min_logical {
            continue;
        }

        let decode_ns = if budget.classical_cores > 0 && budget.classical_clock_ghz > 0.0 {
            ((d as f64).powi(3) / (budget.classical_cores as f64 * budget.classical_clock_ghz))
                as u64
        } else {
            u64::MAX
        };
        let decode_threads = budget.classical_cores.min(max_logical);

        let p_th = 0.01_f64;
        let ratio = error_rate / p_th;
        let exp = (d as f64 + 1.0) / 2.0;
        let p_logical = if ratio < 1.0 {
            0.1 * ratio.powf(exp)
        } else {
            1.0_f64.min(ratio.powf(exp))
        };

        let t_syn = syndrome_period_ns(d);
        let round_time = t_syn.max(decode_ns);
        let budget_ns = budget.total_time_budget_us * 1000;
        if round_time == 0 || budget_ns / round_time == 0 {
            continue;
        }

        let score = if p_logical > 0.0 && max_logical > 0 {
            (max_logical as f64).log2() - p_logical.log10()
        } else if max_logical > 0 {
            (max_logical as f64).log2() + 15.0
        } else {
            0.0
        };

        candidates.push(OptimalAllocation {
            code_distance: d,
            logical_qubits: max_logical,
            decode_threads,
            expected_logical_error_rate: p_logical,
            pareto_score: score,
        });
    }
    candidates.sort_by(|a, b| {
        b.pareto_score
            .partial_cmp(&a.pareto_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates
}

// -- 4. Latency Budget Planner -----------------------------------------------

/// Breakdown of time budgets for a single QEC round.
#[derive(Debug, Clone)]
pub struct LatencyBudget {
    pub syndrome_extraction_ns: u64,
    pub decode_ns: u64,
    pub correction_ns: u64,
    pub total_round_ns: u64,
    pub slack_ns: i64,
}

/// Plan the latency budget for one QEC round at the given distance and decode time.
pub fn plan_latency_budget(distance: u32, decode_ns_per_syndrome: u64) -> LatencyBudget {
    let extraction_ns = syndrome_period_ns(distance);
    let correction_ns: u64 = 20;
    let total_round_ns = extraction_ns + decode_ns_per_syndrome + correction_ns;
    let slack_ns = extraction_ns as i64 - (decode_ns_per_syndrome as i64 + correction_ns as i64);
    LatencyBudget {
        syndrome_extraction_ns: extraction_ns,
        decode_ns: decode_ns_per_syndrome,
        correction_ns,
        total_round_ns,
        slack_ns,
    }
}

// -- 5. Backlog Simulator ----------------------------------------------------

/// Full trace of a simulated control loop execution.
#[derive(Debug, Clone)]
pub struct SimulationTrace {
    pub rounds: Vec<RoundSnapshot>,
    pub converged: bool,
    pub final_logical_error_rate: f64,
    pub max_backlog: f64,
}

/// Snapshot of a single simulation round.
#[derive(Debug, Clone)]
pub struct RoundSnapshot {
    pub round: u64,
    pub errors_this_round: u32,
    pub errors_corrected: u32,
    pub backlog: f64,
    pub decode_latency_ns: u64,
}

/// Monte Carlo simulation of the QEC control loop with seeded RNG.
pub fn simulate_control_loop(
    config: &QecControlLoop,
    num_rounds: u64,
    seed: u64,
) -> SimulationTrace {
    let mut rng = StdRng::seed_from_u64(seed);
    let d = config.plant.code_distance;
    let p = config.plant.physical_error_rate;
    let n_q = config.plant.num_data_qubits;
    let t_decode = config.controller.decode_latency_ns;
    let acc = config.controller.accuracy;
    let t_syn = syndrome_period_ns(d);

    let mut rounds = Vec::with_capacity(num_rounds as usize);
    let (mut backlog, mut max_backlog) = (0.0_f64, 0.0_f64);
    let mut logical_errors = 0u64;

    for r in 0..num_rounds {
        let mut errs: u32 = 0;
        for _ in 0..n_q {
            if rng.gen::<f64>() < p {
                errs += 1;
            }
        }

        let jitter = 0.8 + 0.4 * rng.gen::<f64>();
        let actual_lat = (t_decode as f64 * jitter) as u64;
        let in_time = actual_lat < t_syn;

        let corrected = if in_time {
            let mut c = 0u32;
            for _ in 0..errs {
                if rng.gen::<f64>() < acc {
                    c += 1;
                }
            }
            c
        } else {
            0
        };

        let uncorrected = errs.saturating_sub(corrected);
        backlog += uncorrected as f64;
        if in_time && backlog > 0.0 {
            backlog -= (backlog * acc).min(backlog);
        }
        if backlog > max_backlog {
            max_backlog = backlog;
        }
        if uncorrected > (d.saturating_sub(1)) / 2 {
            logical_errors += 1;
        }

        rounds.push(RoundSnapshot {
            round: r,
            errors_this_round: errs,
            errors_corrected: corrected,
            backlog,
            decode_latency_ns: actual_lat,
        });
    }

    let final_logical_error_rate = if num_rounds > 0 {
        logical_errors as f64 / num_rounds as f64
    } else {
        0.0
    };
    SimulationTrace {
        rounds,
        converged: backlog < 1.0,
        final_logical_error_rate,
        max_backlog,
    }
}

// -- 6. Scaling Laws ---------------------------------------------------------

/// A power-law scaling relation: `y = prefactor * x^exponent`.
#[derive(Debug, Clone)]
pub struct ScalingLaw {
    pub name: String,
    pub exponent: f64,
    pub prefactor: f64,
}

/// Classical overhead scaling for a named decoder.
/// Known: `"union_find"` O(n), `"mwpm"` O(n^3), `"neural"` O(n). Default: O(n^2).
pub fn classical_overhead_scaling(decoder_name: &str) -> ScalingLaw {
    match decoder_name {
        "union_find" => ScalingLaw {
            name: "Union-Find decoder".into(),
            exponent: 1.0,
            prefactor: 1.0,
        },
        "mwpm" => ScalingLaw {
            name: "Minimum Weight Perfect Matching".into(),
            exponent: 3.0,
            prefactor: 0.5,
        },
        "neural" => ScalingLaw {
            name: "Neural network decoder".into(),
            exponent: 1.0,
            prefactor: 10.0,
        },
        _ => ScalingLaw {
            name: format!("Generic decoder ({})", decoder_name),
            exponent: 2.0,
            prefactor: 1.0,
        },
    }
}

/// Logical error rate scaling: p_L ~ prefactor * (p/p_th)^exponent per distance step.
/// Below threshold the exponent is the suppression factor lambda = -ln(p/p_th).
pub fn logical_error_scaling(physical_rate: f64, threshold: f64) -> ScalingLaw {
    if threshold <= 0.0 || physical_rate <= 0.0 {
        return ScalingLaw {
            name: "Logical error scaling (degenerate)".into(),
            exponent: 0.0,
            prefactor: 1.0,
        };
    }
    if physical_rate >= threshold {
        return ScalingLaw {
            name: "Logical error scaling (above threshold)".into(),
            exponent: 0.0,
            prefactor: 1.0,
        };
    }
    let lambda = -(physical_rate / threshold).ln();
    ScalingLaw {
        name: "Logical error scaling (below threshold)".into(),
        exponent: lambda,
        prefactor: 0.1,
    }
}

// == Tests ===================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_plant(d: u32, p: f64) -> QuantumPlant {
        QuantumPlant {
            code_distance: d,
            physical_error_rate: p,
            num_data_qubits: d * d,
            coherence_time_ns: 100_000,
        }
    }
    fn make_controller(lat: u64, tp: f64, acc: f64) -> ClassicalController {
        ClassicalController {
            decode_latency_ns: lat,
            decode_throughput: tp,
            accuracy: acc,
        }
    }
    fn make_loop(d: u32, p: f64, lat: u64) -> QecControlLoop {
        QecControlLoop {
            plant: make_plant(d, p),
            controller: make_controller(lat, 1e6, 0.99),
            state: ControlState::new(),
        }
    }

    #[test]
    fn test_control_state_new() {
        let s = ControlState::new();
        assert_eq!(s.logical_error_rate, 0.0);
        assert_eq!(s.error_backlog, 0.0);
        assert_eq!(s.rounds_decoded, 0);
        assert_eq!(s.total_latency_ns, 0);
    }
    #[test]
    fn test_control_state_default() {
        assert_eq!(ControlState::default().rounds_decoded, 0);
    }

    #[test]
    fn test_syndrome_period_scales() {
        assert!(syndrome_period_ns(3) < syndrome_period_ns(5));
        assert!(syndrome_period_ns(5) < syndrome_period_ns(7));
    }
    #[test]
    fn test_syndrome_period_d3() {
        assert_eq!(syndrome_period_ns(3), 360);
    }

    #[test]
    fn test_stable_loop() {
        let c = analyze_stability(&make_loop(5, 0.001, 100));
        assert!(c.is_stable);
        assert!(c.margin > 0.0);
        assert!(c.convergence_rate > 0.0);
    }
    #[test]
    fn test_unstable_loop() {
        let c = analyze_stability(&make_loop(3, 0.001, 1000));
        assert!(!c.is_stable);
        assert!(c.margin < 0.0);
    }
    #[test]
    fn test_stability_critical_latency() {
        assert_eq!(
            analyze_stability(&make_loop(5, 0.001, 100)).critical_latency_ns,
            syndrome_period_ns(5)
        );
    }
    #[test]
    fn test_stability_zero_decode() {
        let c = analyze_stability(&make_loop(3, 0.001, 0));
        assert!(c.is_stable);
        assert!(c.margin.is_infinite());
    }

    #[test]
    fn test_max_stable_fast() {
        assert!(max_stable_distance(&make_controller(100, 1e7, 0.99), 0.001) >= 3);
    }
    #[test]
    fn test_max_stable_slow() {
        assert!(max_stable_distance(&make_controller(10_000, 1e5, 0.99), 0.001) >= 3);
    }
    #[test]
    fn test_max_stable_above_thresh() {
        assert_eq!(
            max_stable_distance(&make_controller(100, 1e7, 0.99), 0.5),
            3
        );
    }

    #[test]
    fn test_min_throughput_d3() {
        let tp = min_throughput(&make_plant(3, 0.001));
        assert!(tp > 2e6 && tp < 3e6);
    }
    #[test]
    fn test_min_throughput_ordering() {
        assert!(min_throughput(&make_plant(3, 0.001)) > min_throughput(&make_plant(5, 0.001)));
    }

    #[test]
    fn test_optimize_basic() {
        let b = ResourceBudget {
            total_physical_qubits: 10_000,
            classical_cores: 8,
            classical_clock_ghz: 3.0,
            total_time_budget_us: 1_000,
        };
        let a = optimize_allocation(&b, 0.001, 1);
        assert!(!a.is_empty());
        for w in a.windows(2) {
            assert!(w[0].pareto_score >= w[1].pareto_score);
        }
    }
    #[test]
    fn test_optimize_min_logical() {
        let b = ResourceBudget {
            total_physical_qubits: 100,
            classical_cores: 4,
            classical_clock_ghz: 2.0,
            total_time_budget_us: 1_000,
        };
        for a in &optimize_allocation(&b, 0.001, 5) {
            assert!(a.logical_qubits >= 5);
        }
    }
    #[test]
    fn test_optimize_insufficient() {
        let b = ResourceBudget {
            total_physical_qubits: 5,
            classical_cores: 1,
            classical_clock_ghz: 1.0,
            total_time_budget_us: 100,
        };
        assert!(optimize_allocation(&b, 0.001, 1).is_empty());
    }
    #[test]
    fn test_optimize_zero_cores() {
        let b = ResourceBudget {
            total_physical_qubits: 10_000,
            classical_cores: 0,
            classical_clock_ghz: 0.0,
            total_time_budget_us: 1_000,
        };
        assert!(optimize_allocation(&b, 0.001, 1).is_empty());
    }

    #[test]
    fn test_latency_budget_d3() {
        let lb = plan_latency_budget(3, 100);
        assert_eq!(lb.syndrome_extraction_ns, 360);
        assert_eq!(lb.decode_ns, 100);
        assert_eq!(lb.correction_ns, 20);
        assert_eq!(lb.total_round_ns, 480);
        assert_eq!(lb.slack_ns, 240);
    }
    #[test]
    fn test_latency_budget_negative_slack() {
        assert!(plan_latency_budget(3, 1000).slack_ns < 0);
    }
    #[test]
    fn test_latency_budget_scales() {
        assert!(
            plan_latency_budget(7, 100).syndrome_extraction_ns
                > plan_latency_budget(3, 100).syndrome_extraction_ns
        );
    }

    #[test]
    fn test_sim_stable() {
        let t = simulate_control_loop(&make_loop(5, 0.001, 100), 100, 42);
        assert_eq!(t.rounds.len(), 100);
        assert!(t.converged);
        assert!(t.max_backlog < 50.0);
    }
    #[test]
    fn test_sim_unstable() {
        let t = simulate_control_loop(&make_loop(3, 0.3, 1000), 200, 42);
        assert_eq!(t.rounds.len(), 200);
        assert!(t.max_backlog > 0.0);
    }
    #[test]
    fn test_sim_zero_rounds() {
        let t = simulate_control_loop(&make_loop(3, 0.001, 100), 0, 42);
        assert!(t.rounds.is_empty());
        assert_eq!(t.final_logical_error_rate, 0.0);
        assert!(t.converged);
    }
    #[test]
    fn test_sim_deterministic() {
        let t1 = simulate_control_loop(&make_loop(5, 0.01, 200), 50, 123);
        let t2 = simulate_control_loop(&make_loop(5, 0.01, 200), 50, 123);
        for (a, b) in t1.rounds.iter().zip(t2.rounds.iter()) {
            assert_eq!(a.errors_this_round, b.errors_this_round);
            assert_eq!(a.errors_corrected, b.errors_corrected);
        }
    }
    #[test]
    fn test_sim_zero_error_rate() {
        let t = simulate_control_loop(&make_loop(5, 0.0, 100), 50, 99);
        assert!(t.converged);
        assert_eq!(t.final_logical_error_rate, 0.0);
        for s in &t.rounds {
            assert_eq!(s.errors_this_round, 0);
        }
    }
    #[test]
    fn test_sim_snapshot_fields() {
        let t = simulate_control_loop(&make_loop(3, 0.01, 100), 10, 7);
        for (i, s) in t.rounds.iter().enumerate() {
            assert_eq!(s.round, i as u64);
            assert!(s.errors_corrected <= s.errors_this_round);
            assert!(s.decode_latency_ns > 0);
        }
    }

    #[test]
    fn test_scaling_uf() {
        let l = classical_overhead_scaling("union_find");
        assert_eq!(l.exponent, 1.0);
        assert!(l.name.contains("Union-Find"));
    }
    #[test]
    fn test_scaling_mwpm() {
        assert_eq!(classical_overhead_scaling("mwpm").exponent, 3.0);
    }
    #[test]
    fn test_scaling_neural() {
        let l = classical_overhead_scaling("neural");
        assert_eq!(l.exponent, 1.0);
        assert!(l.prefactor > 1.0);
    }
    #[test]
    fn test_scaling_unknown() {
        let l = classical_overhead_scaling("custom");
        assert_eq!(l.exponent, 2.0);
        assert!(l.name.contains("custom"));
    }

    #[test]
    fn test_logical_below() {
        let l = logical_error_scaling(0.001, 0.01);
        assert!(l.exponent > 0.0);
        assert_eq!(l.prefactor, 0.1);
    }
    #[test]
    fn test_logical_above() {
        let l = logical_error_scaling(0.05, 0.01);
        assert_eq!(l.exponent, 0.0);
        assert_eq!(l.prefactor, 1.0);
    }
    #[test]
    fn test_logical_at() {
        assert_eq!(logical_error_scaling(0.01, 0.01).exponent, 0.0);
    }
    #[test]
    fn test_logical_zero_rate() {
        assert_eq!(logical_error_scaling(0.0, 0.01).exponent, 0.0);
    }
    #[test]
    fn test_logical_zero_thresh() {
        assert_eq!(logical_error_scaling(0.001, 0.0).exponent, 0.0);
    }
}
