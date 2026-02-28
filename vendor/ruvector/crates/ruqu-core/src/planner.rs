//! Cost-model circuit execution planner.
//!
//! Replaces the simple heuristic backend selector in [`crate::backend`] with a
//! full cost-model planner that produces a concrete [`ExecutionPlan`] -- not
//! just a backend enum. The planner predicts memory usage, runtime, selects
//! verification policies, mitigation strategies, and computes entanglement
//! budgets for tensor-network simulation.
//!
//! # Cost Model
//!
//! | Backend | Memory | Runtime |
//! |---------|--------|---------|
//! | StateVector | 2^n * 16 bytes | 2^n * gates * 4ns (SIMD, n<=25) |
//! | Stabilizer | n^2 / 4 bytes | n^2 * gates * 0.1ns |
//! | TensorNetwork | n * chi^2 * 16 bytes | n * chi^3 * gates * 2ns |
//!
//! # Example
//!
//! ```
//! use ruqu_core::circuit::QuantumCircuit;
//! use ruqu_core::planner::{plan_execution, PlannerConfig};
//! use ruqu_core::backend::BackendType;
//!
//! let mut circ = QuantumCircuit::new(5);
//! circ.h(0).cnot(0, 1).t(2);
//!
//! let config = PlannerConfig::default();
//! let plan = plan_execution(&circ, &config);
//! assert_eq!(plan.backend, BackendType::StateVector);
//! assert!(plan.predicted_memory_bytes < config.available_memory_bytes);
//! ```

use crate::backend::{analyze_circuit, BackendType, CircuitAnalysis};
use crate::circuit::QuantumCircuit;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A concrete execution plan produced by the cost-model planner.
///
/// Contains the selected backend, predicted resource usage, verification and
/// mitigation policies, and an optional entanglement budget for tensor-network
/// simulation.
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    /// Selected simulation backend.
    pub backend: BackendType,
    /// Predicted peak memory usage in bytes.
    pub predicted_memory_bytes: u64,
    /// Predicted wall-clock runtime in milliseconds.
    pub predicted_runtime_ms: f64,
    /// Confidence in the plan (0.0 to 1.0).
    pub confidence: f64,
    /// How to verify the simulation result.
    pub verification_policy: VerificationPolicy,
    /// Error mitigation strategy to apply.
    pub mitigation_strategy: MitigationStrategy,
    /// Entanglement budget for tensor-network backends.
    pub entanglement_budget: Option<EntanglementBudget>,
    /// Human-readable explanation of the planning decisions.
    pub explanation: String,
    /// Breakdown of computational costs.
    pub cost_breakdown: CostBreakdown,
}

/// Policy for verifying simulation results.
///
/// Higher-confidence plans may skip verification entirely, while lower
/// confidence triggers cross-checks against a different backend or sampling.
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationPolicy {
    /// Pure Clifford circuit: verify by running the stabilizer backend and
    /// comparing results exactly.
    ExactCliffordCheck,
    /// Run a reduced-qubit version of the circuit on state-vector for a spot
    /// check. The `u32` is the number of qubits in the downscaled version.
    DownscaledStateVector(u32),
    /// Compare a subset of observables between backends. The `u32` is the
    /// number of observables to sample.
    StatisticalSampling(u32),
    /// No verification needed (high confidence in the result).
    None,
}

/// Strategy for mitigating simulation or hardware noise.
#[derive(Debug, Clone, PartialEq)]
pub enum MitigationStrategy {
    /// No mitigation needed (noiseless simulation).
    None,
    /// Apply measurement error correction only.
    MeasurementCorrectionOnly,
    /// Zero-noise extrapolation with the given noise scale factors.
    ZneWithScales(Vec<f64>),
    /// ZNE combined with measurement error correction.
    ZnePlusMeasurementCorrection(Vec<f64>),
    /// Full mitigation pipeline: ZNE + CDR training circuits.
    Full {
        /// Noise scale factors for ZNE.
        zne_scales: Vec<f64>,
        /// Number of Clifford Data Regression training circuits.
        cdr_circuits: usize,
    },
}

/// Entanglement budget for tensor-network simulation.
///
/// Controls the maximum bond dimension and whether truncation is needed.
#[derive(Debug, Clone, PartialEq)]
pub struct EntanglementBudget {
    /// Maximum bond dimension the simulator should allow.
    pub max_bond_dimension: u32,
    /// Predicted peak bond dimension based on circuit analysis.
    pub predicted_peak_bond: u32,
    /// Whether truncation will be needed to stay within budget.
    pub truncation_needed: bool,
}

/// Breakdown of computational costs for the execution plan.
#[derive(Debug, Clone)]
pub struct CostBreakdown {
    /// Estimated floating-point operations in units of 10^9 (GFLOPs).
    pub simulation_cost: f64,
    /// Multiplier overhead from ZNE (e.g., 3.0x for 3 scale factors).
    pub mitigation_overhead: f64,
    /// Multiplier overhead from verification.
    pub verification_overhead: f64,
    /// Total number of shots needed (including mitigation overhead).
    pub total_shots_needed: u32,
}

/// Configuration for the execution planner.
#[derive(Debug, Clone)]
pub struct PlannerConfig {
    /// Available system memory in bytes (default: 8 GiB).
    pub available_memory_bytes: u64,
    /// Optional noise level from 0.0 (noiseless) to 1.0 (fully depolarized).
    /// `None` means noiseless simulation.
    pub noise_level: Option<f64>,
    /// Maximum total shots the user is willing to spend.
    pub shot_budget: u32,
    /// Target precision for observable estimation (standard error).
    pub target_precision: f64,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            available_memory_bytes: 8 * 1024 * 1024 * 1024, // 8 GiB
            noise_level: Option::None,
            shot_budget: 10_000,
            target_precision: 0.01,
        }
    }
}

// ---------------------------------------------------------------------------
// Cost model constants
// ---------------------------------------------------------------------------

/// Nanoseconds per state-vector gate application (SIMD-optimized).
const SV_NS_PER_GATE: f64 = 4.0;

/// Nanoseconds per stabilizer gate application.
const STAB_NS_PER_GATE: f64 = 0.1;

/// Nanoseconds per tensor-network contraction step.
const TN_NS_PER_GATE: f64 = 2.0;

/// Maximum qubit count for comfortable state-vector simulation.
const SV_COMFORT_QUBITS: u32 = 25;

/// Default bond dimension cap for tensor networks when no better estimate
/// is available.
const DEFAULT_MAX_BOND_DIM: u32 = 256;

/// Maximum bond dimension the simulator can practically handle.
const ABSOLUTE_MAX_BOND_DIM: u32 = 4096;

/// Nanoseconds per Clifford+T gate application (per stabilizer term).
const CT_NS_PER_GATE: f64 = 0.15;

/// Maximum T-count where Clifford+T is practical (2^40 terms is too many).
const CT_MAX_T_COUNT: usize = 40;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Plan the execution of a quantum circuit.
///
/// Analyzes the circuit structure, predicts resource costs for each candidate
/// backend, and selects the optimal backend subject to the memory and shot
/// budget constraints in `config`. Returns a complete [`ExecutionPlan`].
///
/// # Arguments
///
/// * `circuit` -- The quantum circuit to plan for.
/// * `config` -- Planner constraints (memory, noise, shots, precision).
///
/// # Example
///
/// ```
/// use ruqu_core::circuit::QuantumCircuit;
/// use ruqu_core::planner::{plan_execution, PlannerConfig};
/// use ruqu_core::backend::BackendType;
///
/// let mut circ = QuantumCircuit::new(3);
/// circ.h(0).cnot(0, 1);
/// let plan = plan_execution(&circ, &PlannerConfig::default());
/// assert_eq!(plan.backend, BackendType::Stabilizer);
/// ```
pub fn plan_execution(circuit: &QuantumCircuit, config: &PlannerConfig) -> ExecutionPlan {
    let analysis = analyze_circuit(circuit);
    let entanglement = estimate_entanglement(circuit);
    let num_qubits = analysis.num_qubits;
    let total_gates = analysis.total_gates;

    // --- Candidate evaluation ---

    // Evaluate Stabilizer backend.
    let stab_viable = analysis.clifford_fraction >= 1.0;
    let stab_memory = predict_memory_stabilizer(num_qubits);
    let stab_runtime = predict_runtime_stabilizer(num_qubits, total_gates);

    // Evaluate StateVector backend.
    let sv_memory = predict_memory_statevector(num_qubits);
    let sv_viable = sv_memory <= config.available_memory_bytes;
    let sv_runtime = predict_runtime_statevector(num_qubits, total_gates);

    // Evaluate TensorNetwork backend.
    let chi = entanglement.predicted_peak_bond.min(ABSOLUTE_MAX_BOND_DIM);
    let tn_memory = predict_memory_tensor_network(num_qubits, chi);
    let tn_viable = tn_memory <= config.available_memory_bytes;
    let tn_runtime = predict_runtime_tensor_network(num_qubits, total_gates, chi);

    // Evaluate CliffordT backend.
    let t_count = analysis.non_clifford_gates;
    let ct_viable = t_count > 0 && t_count <= CT_MAX_T_COUNT && num_qubits > 32;
    let ct_terms = if ct_viable {
        1u64.checked_shl(t_count as u32).unwrap_or(u64::MAX)
    } else {
        u64::MAX
    };
    let ct_memory = predict_memory_clifford_t(num_qubits, ct_terms);
    let ct_runtime = predict_runtime_clifford_t(num_qubits, total_gates, ct_terms);

    // --- Backend selection ---

    let (backend, predicted_memory, predicted_runtime, confidence, explanation) =
        select_optimal_backend(
            &analysis,
            &entanglement,
            config,
            stab_viable,
            stab_memory,
            stab_runtime,
            sv_viable,
            sv_memory,
            sv_runtime,
            tn_viable,
            tn_memory,
            tn_runtime,
            chi,
            ct_viable,
            ct_memory,
            ct_runtime,
            ct_terms,
        );

    // --- Verification policy ---
    let verification_policy = select_verification_policy(&analysis, backend, num_qubits);

    // --- Mitigation strategy ---
    let mitigation_strategy =
        select_mitigation_strategy(config.noise_level, config.shot_budget, &analysis);

    // --- Entanglement budget ---
    let entanglement_budget = if backend == BackendType::TensorNetwork {
        Some(entanglement)
    } else {
        Option::None
    };

    // --- Cost breakdown ---
    let cost_breakdown = compute_cost_breakdown(
        backend,
        predicted_runtime,
        &mitigation_strategy,
        &verification_policy,
        config.shot_budget,
        config.target_precision,
    );

    ExecutionPlan {
        backend,
        predicted_memory_bytes: predicted_memory,
        predicted_runtime_ms: predicted_runtime,
        confidence,
        verification_policy,
        mitigation_strategy,
        entanglement_budget,
        explanation,
        cost_breakdown,
    }
}

/// Estimate the entanglement budget for a quantum circuit.
///
/// Walks the circuit gate-by-gate, tracking cumulative two-qubit gate count
/// across each possible bipartition of the qubit register. The peak bond
/// dimension is derived from the worst-case cut.
///
/// # Arguments
///
/// * `circuit` -- The quantum circuit to analyze.
///
/// # Returns
///
/// An [`EntanglementBudget`] with the predicted peak bond dimension and
/// whether truncation would be needed.
pub fn estimate_entanglement(circuit: &QuantumCircuit) -> EntanglementBudget {
    let n = circuit.num_qubits();
    if n <= 1 {
        return EntanglementBudget {
            max_bond_dimension: 1,
            predicted_peak_bond: 1,
            truncation_needed: false,
        };
    }

    // Track cumulative entangling-gate count crossing each cut position.
    // cut_counts[k] counts gates straddling the partition [0..k) | [k..n).
    let num_cuts = (n - 1) as usize;
    let mut cut_counts = vec![0u32; num_cuts];

    for gate in circuit.gates() {
        let qubits = gate.qubits();
        if qubits.len() == 2 {
            let (lo, hi) = if qubits[0] < qubits[1] {
                (qubits[0], qubits[1])
            } else {
                (qubits[1], qubits[0])
            };
            // This gate crosses every cut between lo and hi.
            for cut_idx in (lo as usize)..(hi as usize) {
                if cut_idx < num_cuts {
                    cut_counts[cut_idx] += 1;
                }
            }
        }
    }

    let max_gates_across_cut = cut_counts.iter().copied().max().unwrap_or(0);

    // Bond dimension grows as 2^(gates across cut), but we cap it sensibly.
    // For circuits where max_gates_across_cut is large, the bond dimension
    // is effectively 2^(n/2) (the maximum for n qubits).
    let half_n = n / 2;
    let effective_exponent = max_gates_across_cut.min(half_n).min(30);
    let predicted_peak_bond = 1u32.checked_shl(effective_exponent).unwrap_or(u32::MAX);

    // Allow up to the absolute maximum or 2x the predicted peak.
    let max_bond_dimension = predicted_peak_bond
        .saturating_mul(2)
        .min(ABSOLUTE_MAX_BOND_DIM);

    let truncation_needed = predicted_peak_bond > DEFAULT_MAX_BOND_DIM;

    EntanglementBudget {
        max_bond_dimension,
        predicted_peak_bond,
        truncation_needed,
    }
}

// ---------------------------------------------------------------------------
// Memory prediction
// ---------------------------------------------------------------------------

/// Predict memory usage for state-vector simulation: 2^n * 16 bytes.
fn predict_memory_statevector(num_qubits: u32) -> u64 {
    if num_qubits >= 64 {
        return u64::MAX;
    }
    (1u64 << num_qubits).saturating_mul(16)
}

/// Predict memory usage for stabilizer simulation: n^2 / 4 bytes.
fn predict_memory_stabilizer(num_qubits: u32) -> u64 {
    let n = num_qubits as u64;
    // Stabilizer tableau stores 2n rows of n bits each, packed.
    // Approximately n^2 / 4 bytes.
    n.saturating_mul(n) / 4
}

/// Predict memory usage for tensor-network simulation: n * chi^2 * 16 bytes.
fn predict_memory_tensor_network(num_qubits: u32, chi: u32) -> u64 {
    let n = num_qubits as u64;
    let c = chi as u64;
    n.saturating_mul(c).saturating_mul(c).saturating_mul(16)
}

// ---------------------------------------------------------------------------
// Runtime prediction
// ---------------------------------------------------------------------------

/// Predict runtime for state-vector simulation in milliseconds.
///
/// Base: 2^n * gates * 4ns for n <= 25.
/// Each qubit above 25 doubles the runtime (cache pressure, no SIMD benefit).
fn predict_runtime_statevector(num_qubits: u32, total_gates: usize) -> f64 {
    if num_qubits >= 64 {
        return f64::INFINITY;
    }
    let base_ops = (1u64 << num_qubits) as f64 * total_gates as f64;
    let ns = base_ops * SV_NS_PER_GATE;

    // Scale up for qubits beyond the SIMD-comfortable threshold.
    let scaling = if num_qubits > SV_COMFORT_QUBITS {
        2.0_f64.powi((num_qubits - SV_COMFORT_QUBITS) as i32)
    } else {
        1.0
    };

    ns * scaling / 1_000_000.0 // Convert ns to ms
}

/// Predict runtime for stabilizer simulation in milliseconds.
///
/// n^2 * gates * 0.1ns.
fn predict_runtime_stabilizer(num_qubits: u32, total_gates: usize) -> f64 {
    let n = num_qubits as f64;
    let ns = n * n * total_gates as f64 * STAB_NS_PER_GATE;
    ns / 1_000_000.0
}

/// Predict runtime for tensor-network simulation in milliseconds.
///
/// n * chi^3 * gates * 2ns.
fn predict_runtime_tensor_network(num_qubits: u32, total_gates: usize, chi: u32) -> f64 {
    let n = num_qubits as f64;
    let c = chi as f64;
    let ns = n * c * c * c * total_gates as f64 * TN_NS_PER_GATE;
    ns / 1_000_000.0
}

/// Predict memory for Clifford+T: terms * n^2 / 4 bytes.
fn predict_memory_clifford_t(num_qubits: u32, terms: u64) -> u64 {
    let n = num_qubits as u64;
    // Each stabilizer term needs a tableau of ~n^2/4 bytes + 16 bytes for the coefficient.
    let per_term = n.saturating_mul(n) / 4 + 16;
    terms.saturating_mul(per_term)
}

/// Predict runtime for Clifford+T in milliseconds.
///
/// terms * n^2 * gates * 0.15ns.
fn predict_runtime_clifford_t(num_qubits: u32, total_gates: usize, terms: u64) -> f64 {
    let n = num_qubits as f64;
    let ns = terms as f64 * n * n * total_gates as f64 * CT_NS_PER_GATE;
    ns / 1_000_000.0
}

// ---------------------------------------------------------------------------
// Backend selection logic
// ---------------------------------------------------------------------------

/// Select the optimal backend given cost estimates and constraints.
///
/// Priority order:
/// 1. Stabilizer for pure-Clifford circuits (any qubit count).
/// 2. StateVector when it fits in memory and qubit count is manageable.
/// 3. TensorNetwork when StateVector exceeds memory.
/// 4. TensorNetwork as last resort for large circuits.
#[allow(clippy::too_many_arguments)]
fn select_optimal_backend(
    analysis: &CircuitAnalysis,
    entanglement: &EntanglementBudget,
    config: &PlannerConfig,
    stab_viable: bool,
    stab_memory: u64,
    stab_runtime: f64,
    sv_viable: bool,
    sv_memory: u64,
    sv_runtime: f64,
    _tn_viable: bool,
    tn_memory: u64,
    tn_runtime: f64,
    chi: u32,
    ct_viable: bool,
    ct_memory: u64,
    ct_runtime: f64,
    ct_terms: u64,
) -> (BackendType, u64, f64, f64, String) {
    let n = analysis.num_qubits;

    // Rule 1: Pure Clifford -> Stabilizer (efficient for any qubit count).
    if stab_viable {
        return (
            BackendType::Stabilizer,
            stab_memory,
            stab_runtime,
            0.99,
            format!(
                "Pure Clifford circuit ({} qubits, {} gates): stabilizer simulation in \
                 O(n^2) per gate. Predicted {:.1} ms, {} bytes memory.",
                n, analysis.total_gates, stab_runtime, stab_memory
            ),
        );
    }

    // Rule 2: Mostly Clifford with very few non-Clifford on large circuits.
    if analysis.clifford_fraction >= 0.95 && n > 32 && analysis.non_clifford_gates <= 10 {
        return (
            BackendType::Stabilizer,
            stab_memory,
            stab_runtime,
            0.85,
            format!(
                "{:.0}% Clifford with only {} non-Clifford gates on {} qubits: \
                 stabilizer backend with approximate decomposition.",
                analysis.clifford_fraction * 100.0,
                analysis.non_clifford_gates,
                n
            ),
        );
    }

    // Rule 2b: Moderate T-count on large circuits -> CliffordT.
    if ct_viable && ct_memory <= config.available_memory_bytes {
        return (
            BackendType::CliffordT,
            ct_memory,
            ct_runtime,
            0.90,
            format!(
                "{} qubits with {} T-gates: Clifford+T decomposition with {} stabilizer terms. \
                 Predicted {:.2} ms, {} bytes.",
                n, analysis.non_clifford_gates, ct_terms, ct_runtime, ct_memory
            ),
        );
    }

    // Rule 3: StateVector fits in available memory.
    if sv_viable && n <= 32 {
        let conf = if n <= SV_COMFORT_QUBITS { 0.95 } else { 0.80 };
        return (
            BackendType::StateVector,
            sv_memory,
            sv_runtime,
            conf,
            format!(
                "{} qubits fits in state vector ({} bytes). Predicted {:.2} ms runtime.",
                n, sv_memory, sv_runtime
            ),
        );
    }

    // Rule 4: StateVector would exceed memory -> fall back to TensorNetwork.
    if !sv_viable || n > 32 {
        let conf = if analysis.is_nearest_neighbor && analysis.depth < n * 2 {
            0.85
        } else if analysis.is_nearest_neighbor {
            0.75
        } else {
            0.55
        };

        let used_memory = tn_memory;
        let used_runtime = tn_runtime;

        let truncation_note = if entanglement.truncation_needed {
            " Results will be approximate due to bond dimension truncation."
        } else {
            ""
        };

        return (
            BackendType::TensorNetwork,
            used_memory,
            used_runtime,
            conf,
            format!(
                "{} qubits exceeds state vector capacity ({} bytes > {} bytes available). \
                 Using tensor network with chi={}.{} Predicted {:.2} ms.",
                n,
                predict_memory_statevector(n),
                config.available_memory_bytes,
                chi,
                truncation_note,
                used_runtime
            ),
        );
    }

    // Fallback: state vector.
    (
        BackendType::StateVector,
        sv_memory,
        sv_runtime,
        0.70,
        "Default to exact state vector simulation.".into(),
    )
}

// ---------------------------------------------------------------------------
// Verification policy selection
// ---------------------------------------------------------------------------

/// Select a verification policy based on circuit properties.
///
/// - Pure Clifford: exact cross-check with stabilizer.
/// - High confidence and small circuits: no verification.
/// - Medium confidence: downscaled state-vector spot check.
/// - Low confidence: statistical sampling.
fn select_verification_policy(
    analysis: &CircuitAnalysis,
    backend: BackendType,
    num_qubits: u32,
) -> VerificationPolicy {
    // Pure Clifford: always verify with stabilizer (it's cheap).
    if analysis.clifford_fraction >= 1.0 {
        return VerificationPolicy::ExactCliffordCheck;
    }

    // High Clifford fraction on a non-stabilizer backend: downscale check.
    if analysis.clifford_fraction >= 0.9 && num_qubits > 20 {
        let downscale_qubits = num_qubits.min(16);
        return VerificationPolicy::DownscaledStateVector(downscale_qubits);
    }

    // Small state-vector circuits: no verification needed.
    if backend == BackendType::StateVector && num_qubits <= SV_COMFORT_QUBITS {
        return VerificationPolicy::None;
    }

    // Medium-sized state-vector: statistical sampling with a few observables.
    if backend == BackendType::StateVector && num_qubits <= 32 {
        return VerificationPolicy::StatisticalSampling(10);
    }

    // Tensor network: always verify since results may be approximate.
    if backend == BackendType::TensorNetwork {
        if num_qubits <= 20 {
            // Small enough to cross-check with state vector.
            return VerificationPolicy::DownscaledStateVector(num_qubits);
        }
        return VerificationPolicy::StatisticalSampling((num_qubits / 2).max(5).min(50));
    }

    VerificationPolicy::None
}

// ---------------------------------------------------------------------------
// Mitigation strategy selection
// ---------------------------------------------------------------------------

/// Select the error mitigation strategy based on noise level and shot budget.
///
/// - No noise: no mitigation.
/// - Low noise (< 0.01): measurement correction only.
/// - Medium noise (0.01-0.1): ZNE with 3 scale factors.
/// - High noise (0.1-0.5): ZNE + measurement correction.
/// - Very high noise (> 0.5): full pipeline with CDR.
fn select_mitigation_strategy(
    noise_level: Option<f64>,
    shot_budget: u32,
    analysis: &CircuitAnalysis,
) -> MitigationStrategy {
    let noise = match noise_level {
        Some(n) if n > 0.0 => n,
        _ => return MitigationStrategy::None,
    };

    // Low noise: measurement correction is sufficient.
    if noise < 0.01 {
        return MitigationStrategy::MeasurementCorrectionOnly;
    }

    // Standard ZNE scale factors.
    let zne_scales_3 = vec![1.0, 1.5, 2.0];
    let zne_scales_5 = vec![1.0, 1.25, 1.5, 1.75, 2.0];

    // Medium noise: ZNE with 3 scale factors.
    if noise < 0.1 {
        // If we have enough shots, use 5 scale factors for better extrapolation.
        let scales = if shot_budget >= 50_000 {
            zne_scales_5
        } else {
            zne_scales_3.clone()
        };
        return MitigationStrategy::ZneWithScales(scales);
    }

    // High noise: ZNE + measurement correction.
    if noise < 0.5 {
        let scales = if shot_budget >= 50_000 {
            zne_scales_5
        } else {
            zne_scales_3
        };
        return MitigationStrategy::ZnePlusMeasurementCorrection(scales);
    }

    // Very high noise: full pipeline with CDR.
    // CDR circuits scale with circuit complexity.
    let cdr_circuits = (analysis.non_clifford_gates * 2).max(10).min(100);
    MitigationStrategy::Full {
        zne_scales: vec![1.0, 1.5, 2.0, 2.5, 3.0],
        cdr_circuits,
    }
}

// ---------------------------------------------------------------------------
// Cost breakdown computation
// ---------------------------------------------------------------------------

/// Compute a cost breakdown for the execution plan.
fn compute_cost_breakdown(
    _backend: BackendType,
    predicted_runtime_ms: f64,
    mitigation: &MitigationStrategy,
    verification: &VerificationPolicy,
    shot_budget: u32,
    target_precision: f64,
) -> CostBreakdown {
    // Simulation cost in GFLOPs (rough estimate from runtime).
    // Assume ~1 GFLOP/ms on a modern CPU.
    let simulation_cost = predicted_runtime_ms.max(0.001);

    // Mitigation overhead multiplier.
    let mitigation_overhead = match mitigation {
        MitigationStrategy::None => 1.0,
        MitigationStrategy::MeasurementCorrectionOnly => 1.1, // slight overhead
        MitigationStrategy::ZneWithScales(scales) => scales.len() as f64,
        MitigationStrategy::ZnePlusMeasurementCorrection(scales) => scales.len() as f64 * 1.1,
        MitigationStrategy::Full {
            zne_scales,
            cdr_circuits,
        } => zne_scales.len() as f64 + *cdr_circuits as f64 * 0.5,
    };

    // Verification overhead multiplier.
    let verification_overhead = match verification {
        VerificationPolicy::None => 1.0,
        VerificationPolicy::ExactCliffordCheck => 1.05, // cheap stabilizer check
        VerificationPolicy::DownscaledStateVector(_) => 1.1,
        VerificationPolicy::StatisticalSampling(n) => 1.0 + (*n as f64) * 0.01,
    };

    // Total shots: base shots * mitigation overhead.
    // Base shots from precision: 1 / precision^2 (Hoeffding bound).
    let base_shots = (1.0 / (target_precision * target_precision)).ceil() as u32;
    let mitigated_shots = (base_shots as f64 * mitigation_overhead).ceil() as u32;
    let total_shots_needed = mitigated_shots.min(shot_budget);

    CostBreakdown {
        simulation_cost,
        mitigation_overhead,
        verification_overhead,
        total_shots_needed,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::QuantumCircuit;

    /// Helper to build a default planner config.
    fn default_config() -> PlannerConfig {
        PlannerConfig::default()
    }

    // -----------------------------------------------------------------------
    // test_pure_clifford_plan
    // -----------------------------------------------------------------------

    #[test]
    fn test_pure_clifford_plan() {
        // A pure Clifford circuit should route to Stabilizer with
        // ExactCliffordCheck verification.
        let mut circ = QuantumCircuit::new(50);
        for q in 0..50 {
            circ.h(q);
        }
        for q in 0..49 {
            circ.cnot(q, q + 1);
        }

        let config = default_config();
        let plan = plan_execution(&circ, &config);

        assert_eq!(
            plan.backend,
            BackendType::Stabilizer,
            "Pure Clifford circuit should use Stabilizer backend"
        );
        assert_eq!(
            plan.verification_policy,
            VerificationPolicy::ExactCliffordCheck,
            "Pure Clifford should use ExactCliffordCheck verification"
        );
        assert_eq!(
            plan.mitigation_strategy,
            MitigationStrategy::None,
            "Noiseless config should have no mitigation"
        );
        assert!(
            plan.confidence > 0.9,
            "Confidence should be high for pure Clifford"
        );
        assert!(
            plan.entanglement_budget.is_none(),
            "Stabilizer backend should not have entanglement budget"
        );
    }

    // -----------------------------------------------------------------------
    // test_small_circuit_plan
    // -----------------------------------------------------------------------

    #[test]
    fn test_small_circuit_plan() {
        // A small circuit with non-Clifford gates should route to StateVector
        // with no mitigation.
        let mut circ = QuantumCircuit::new(5);
        circ.h(0).t(1).cnot(0, 1).rx(2, 0.5);

        let config = default_config();
        let plan = plan_execution(&circ, &config);

        assert_eq!(
            plan.backend,
            BackendType::StateVector,
            "Small non-Clifford circuit should use StateVector"
        );
        assert_eq!(
            plan.mitigation_strategy,
            MitigationStrategy::None,
            "Noiseless config should have no mitigation"
        );
        assert_eq!(
            plan.verification_policy,
            VerificationPolicy::None,
            "Small SV circuit should not need verification"
        );
        assert!(plan.entanglement_budget.is_none());

        // Memory should be 2^5 * 16 = 512 bytes
        assert_eq!(plan.predicted_memory_bytes, 512);
        assert!(plan.predicted_runtime_ms > 0.0);
        assert!(plan.confidence >= 0.9);
    }

    // -----------------------------------------------------------------------
    // test_large_mps_plan
    // -----------------------------------------------------------------------

    #[test]
    fn test_large_mps_plan() {
        // A large circuit with nearest-neighbor connectivity and many
        // non-Clifford gates (exceeding CT_MAX_T_COUNT) should route to
        // TensorNetwork with an entanglement budget.
        let mut circ = QuantumCircuit::new(64);
        // Build a nearest-neighbor circuit with non-Clifford gates.
        for q in 0..63 {
            circ.cnot(q, q + 1);
        }
        // Use 50 T-gates to exceed CT_MAX_T_COUNT (40), forcing TensorNetwork.
        for q in 0..50 {
            circ.t(q % 64);
        }

        let config = PlannerConfig {
            available_memory_bytes: 8 * 1024 * 1024 * 1024,
            noise_level: Option::None,
            shot_budget: 10_000,
            target_precision: 0.01,
        };
        let plan = plan_execution(&circ, &config);

        assert_eq!(
            plan.backend,
            BackendType::TensorNetwork,
            "Large non-Clifford circuit should use TensorNetwork"
        );
        assert!(
            plan.entanglement_budget.is_some(),
            "TensorNetwork backend should have entanglement budget"
        );
        let budget = plan.entanglement_budget.as_ref().unwrap();
        assert!(
            budget.predicted_peak_bond >= 2,
            "Entangling gates should produce bond dimension >= 2"
        );
        assert!(
            budget.max_bond_dimension >= budget.predicted_peak_bond,
            "Max bond dimension should be >= predicted peak"
        );
    }

    // -----------------------------------------------------------------------
    // test_memory_overflow_fallback
    // -----------------------------------------------------------------------

    #[test]
    fn test_memory_overflow_fallback() {
        // When StateVector would exceed available memory, the planner should
        // fall back to TensorNetwork.
        let mut circ = QuantumCircuit::new(30);
        circ.h(0).t(1).cnot(0, 1);

        // Give only 1 MiB of memory -- not enough for 2^30 * 16 = 16 GiB.
        let config = PlannerConfig {
            available_memory_bytes: 1024 * 1024, // 1 MiB
            noise_level: Option::None,
            shot_budget: 10_000,
            target_precision: 0.01,
        };
        let plan = plan_execution(&circ, &config);

        assert_eq!(
            plan.backend,
            BackendType::TensorNetwork,
            "When SV exceeds memory, should fall back to TensorNetwork"
        );
        // The predicted memory for TN should fit within the budget.
        assert!(
            plan.predicted_memory_bytes <= config.available_memory_bytes,
            "TensorNetwork memory ({}) should fit within budget ({})",
            plan.predicted_memory_bytes,
            config.available_memory_bytes
        );
    }

    // -----------------------------------------------------------------------
    // test_noisy_circuit_plan
    // -----------------------------------------------------------------------

    #[test]
    fn test_noisy_circuit_plan() {
        // When noise_level > 0, the planner should add ZNE mitigation.
        let mut circ = QuantumCircuit::new(5);
        circ.h(0).cnot(0, 1).t(2);

        let config = PlannerConfig {
            available_memory_bytes: 8 * 1024 * 1024 * 1024,
            noise_level: Some(0.05), // medium noise
            shot_budget: 10_000,
            target_precision: 0.01,
        };
        let plan = plan_execution(&circ, &config);

        // Should have ZNE mitigation.
        match &plan.mitigation_strategy {
            MitigationStrategy::ZneWithScales(scales) => {
                assert!(
                    scales.len() >= 3,
                    "ZNE should have at least 3 scale factors"
                );
                assert!(
                    scales.contains(&1.0),
                    "ZNE scales must include the baseline 1.0"
                );
            }
            other => panic!("Expected ZneWithScales for noise=0.05, got {:?}", other),
        }

        assert!(
            plan.cost_breakdown.mitigation_overhead > 1.0,
            "Mitigation should add overhead"
        );
    }

    // -----------------------------------------------------------------------
    // test_entanglement_estimate
    // -----------------------------------------------------------------------

    #[test]
    fn test_entanglement_estimate() {
        // Bell state circuit: H on qubit 0, CNOT(0,1).
        // One two-qubit gate crossing the single cut -> chi = 2.
        let mut circ = QuantumCircuit::new(2);
        circ.h(0).cnot(0, 1);

        let budget = estimate_entanglement(&circ);
        assert_eq!(
            budget.predicted_peak_bond, 2,
            "Bell state should have bond dimension 2"
        );
        assert!(
            !budget.truncation_needed,
            "Bell state bond dimension 2 should not need truncation"
        );
    }

    #[test]
    fn test_entanglement_estimate_single_qubit() {
        // Single-qubit circuit: no entanglement possible.
        let mut circ = QuantumCircuit::new(1);
        circ.h(0);

        let budget = estimate_entanglement(&circ);
        assert_eq!(budget.predicted_peak_bond, 1);
        assert_eq!(budget.max_bond_dimension, 1);
        assert!(!budget.truncation_needed);
    }

    #[test]
    fn test_entanglement_estimate_no_two_qubit_gates() {
        // Multi-qubit circuit but no two-qubit gates: bond dim = 1.
        let mut circ = QuantumCircuit::new(10);
        for q in 0..10 {
            circ.h(q);
        }

        let budget = estimate_entanglement(&circ);
        assert_eq!(budget.predicted_peak_bond, 1);
    }

    #[test]
    fn test_entanglement_estimate_ghz_chain() {
        // GHZ-like circuit: H(0), CNOT(0,1), CNOT(1,2), CNOT(2,3).
        // Each gate crosses one additional cut.
        // Cut 0-1: gates CNOT(0,1) = 1 crossing -> chi=2
        // Cut 1-2: gates CNOT(0,1) does not cross (both on same side),
        //          CNOT(1,2) crosses = 1 -> chi=2
        // Cut 2-3: CNOT(2,3) crosses = 1 -> chi=2
        let mut circ = QuantumCircuit::new(4);
        circ.h(0).cnot(0, 1).cnot(1, 2).cnot(2, 3);

        let budget = estimate_entanglement(&circ);
        assert_eq!(
            budget.predicted_peak_bond, 2,
            "GHZ chain should have peak bond dim 2 (nearest-neighbor only)"
        );
    }

    // -----------------------------------------------------------------------
    // test_workload_routing_accuracy
    // -----------------------------------------------------------------------

    #[test]
    fn test_workload_routing_accuracy() {
        let config = default_config();

        // 1. Empty circuit: pure Clifford -> Stabilizer
        let circ_empty = QuantumCircuit::new(10);
        let plan = plan_execution(&circ_empty, &config);
        assert_eq!(plan.backend, BackendType::Stabilizer);

        // 2. Single H gate: Clifford -> Stabilizer
        let mut circ_h = QuantumCircuit::new(3);
        circ_h.h(0);
        let plan = plan_execution(&circ_h, &config);
        assert_eq!(plan.backend, BackendType::Stabilizer);

        // 3. Bell state (Clifford) -> Stabilizer
        let mut circ_bell = QuantumCircuit::new(2);
        circ_bell.h(0).cnot(0, 1);
        let plan = plan_execution(&circ_bell, &config);
        assert_eq!(plan.backend, BackendType::Stabilizer);

        // 4. Small with T gate -> StateVector
        let mut circ_small_t = QuantumCircuit::new(5);
        circ_small_t.h(0).t(1).cnot(0, 1);
        let plan = plan_execution(&circ_small_t, &config);
        assert_eq!(plan.backend, BackendType::StateVector);

        // 5. 20-qubit variational ansatz -> StateVector
        let mut circ_vqe = QuantumCircuit::new(20);
        for q in 0..20 {
            circ_vqe.ry(q, 0.5);
        }
        for q in 0..19 {
            circ_vqe.cnot(q, q + 1);
        }
        let plan = plan_execution(&circ_vqe, &config);
        assert_eq!(plan.backend, BackendType::StateVector);

        // 6. 40-qubit pure Clifford -> Stabilizer
        let mut circ_40_cliff = QuantumCircuit::new(40);
        for q in 0..40 {
            circ_40_cliff.h(q);
        }
        for q in 0..39 {
            circ_40_cliff.cnot(q, q + 1);
        }
        let plan = plan_execution(&circ_40_cliff, &config);
        assert_eq!(plan.backend, BackendType::Stabilizer);

        // 7. 100-qubit nearest-neighbor with many non-Clifford -> TensorNetwork
        let mut circ_100 = QuantumCircuit::new(100);
        for q in 0..99 {
            circ_100.cnot(q, q + 1);
        }
        for q in 0..50 {
            circ_100.rx(q, 1.0);
        }
        let plan = plan_execution(&circ_100, &config);
        assert_eq!(plan.backend, BackendType::TensorNetwork);

        // 8. 50-qubit mostly-Clifford (few non-Clifford) -> Stabilizer
        let mut circ_mostly_cliff = QuantumCircuit::new(50);
        for q in 0..50 {
            circ_mostly_cliff.h(q);
        }
        for q in 0..49 {
            circ_mostly_cliff.cnot(q, q + 1);
        }
        // Add only a handful of non-Clifford gates (< 10).
        for q in 0..5 {
            circ_mostly_cliff.t(q);
        }
        let plan = plan_execution(&circ_mostly_cliff, &config);
        assert_eq!(
            plan.backend,
            BackendType::Stabilizer,
            "Mostly-Clifford 50-qubit circuit should use Stabilizer"
        );

        // 9. Medium circuit (25 qubits) with non-Clifford -> StateVector
        let mut circ_25 = QuantumCircuit::new(25);
        for q in 0..25 {
            circ_25.h(q);
        }
        for q in 0..24 {
            circ_25.cnot(q, q + 1);
        }
        circ_25.t(0).t(1).rx(2, 0.5);
        let plan = plan_execution(&circ_25, &config);
        assert_eq!(plan.backend, BackendType::StateVector);

        // 10. Large circuit forced into TN by memory constraint.
        let mut circ_28 = QuantumCircuit::new(28);
        circ_28.h(0).t(1).cnot(0, 1);
        let tight_config = PlannerConfig {
            available_memory_bytes: 1024, // absurdly small
            noise_level: Option::None,
            shot_budget: 1000,
            target_precision: 0.01,
        };
        let plan = plan_execution(&circ_28, &tight_config);
        assert_eq!(
            plan.backend,
            BackendType::TensorNetwork,
            "Should fall back to TN when memory is too tight for SV"
        );

        // 11. Very high noise should trigger full mitigation.
        let mut circ_noisy = QuantumCircuit::new(5);
        circ_noisy.h(0).t(0).cnot(0, 1);
        let noisy_config = PlannerConfig {
            available_memory_bytes: 8 * 1024 * 1024 * 1024,
            noise_level: Some(0.7),
            shot_budget: 100_000,
            target_precision: 0.01,
        };
        let plan = plan_execution(&circ_noisy, &noisy_config);
        match &plan.mitigation_strategy {
            MitigationStrategy::Full {
                zne_scales,
                cdr_circuits,
            } => {
                assert!(zne_scales.len() >= 3);
                assert!(*cdr_circuits >= 2);
            }
            other => panic!("Expected Full mitigation for noise=0.7, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Memory prediction tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_memory_prediction_statevector() {
        assert_eq!(predict_memory_statevector(1), 32); // 2 * 16
        assert_eq!(predict_memory_statevector(10), 1024 * 16); // 2^10 * 16
        assert_eq!(predict_memory_statevector(20), 1048576 * 16); // 2^20 * 16
    }

    #[test]
    fn test_memory_prediction_stabilizer() {
        // n^2 / 4
        assert_eq!(predict_memory_stabilizer(100), 2500);
        assert_eq!(predict_memory_stabilizer(1000), 250_000);
    }

    #[test]
    fn test_memory_prediction_tensor_network() {
        // n * chi^2 * 16
        assert_eq!(predict_memory_tensor_network(10, 4), 10 * 16 * 16);
        assert_eq!(predict_memory_tensor_network(100, 32), 100 * 1024 * 16);
    }

    // -----------------------------------------------------------------------
    // Runtime prediction tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_runtime_prediction_statevector() {
        let rt = predict_runtime_statevector(10, 100);
        // 2^10 * 100 * 4ns = 409600 ns = ~0.41 ms
        let expected = (1024.0 * 100.0 * 4.0) / 1_000_000.0;
        assert!(
            (rt - expected).abs() < 1e-6,
            "SV runtime for 10 qubits: expected {expected}, got {rt}"
        );
    }

    #[test]
    fn test_runtime_prediction_stabilizer() {
        let rt = predict_runtime_stabilizer(100, 200);
        // 100^2 * 200 * 0.1 ns = 200000 ns = 0.2 ms
        let expected = (10000.0 * 200.0 * 0.1) / 1_000_000.0;
        assert!(
            (rt - expected).abs() < 1e-6,
            "Stabilizer runtime: expected {expected}, got {rt}"
        );
    }

    #[test]
    fn test_runtime_scales_above_25_qubits() {
        let rt_25 = predict_runtime_statevector(25, 100);
        let rt_26 = predict_runtime_statevector(26, 100);
        // 26 qubits: 2x the amplitudes and 2x the scaling factor = 4x total.
        let ratio = rt_26 / rt_25;
        assert!(
            (ratio - 4.0).abs() < 0.1,
            "Going from 25 to 26 qubits should ~4x the runtime, got {ratio}x"
        );
    }

    // -----------------------------------------------------------------------
    // Cost breakdown tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_cost_breakdown_no_mitigation() {
        let breakdown = compute_cost_breakdown(
            BackendType::StateVector,
            1.0,
            &MitigationStrategy::None,
            &VerificationPolicy::None,
            10_000,
            0.01,
        );
        assert_eq!(breakdown.mitigation_overhead, 1.0);
        assert_eq!(breakdown.verification_overhead, 1.0);
        assert!(breakdown.total_shots_needed <= 10_000);
    }

    #[test]
    fn test_cost_breakdown_with_zne() {
        let scales = vec![1.0, 1.5, 2.0];
        let breakdown = compute_cost_breakdown(
            BackendType::StateVector,
            1.0,
            &MitigationStrategy::ZneWithScales(scales),
            &VerificationPolicy::None,
            100_000,
            0.01,
        );
        assert_eq!(
            breakdown.mitigation_overhead, 3.0,
            "3 ZNE scales -> 3x overhead"
        );
        assert!(breakdown.total_shots_needed > 10_000);
    }

    // -----------------------------------------------------------------------
    // Mitigation strategy selection tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_mitigation_none_for_noiseless() {
        let analysis = make_analysis(5, 10, 1.0);
        let strat = select_mitigation_strategy(Option::None, 10_000, &analysis);
        assert_eq!(strat, MitigationStrategy::None);
    }

    #[test]
    fn test_mitigation_measurement_correction_low_noise() {
        let analysis = make_analysis(5, 10, 0.5);
        let strat = select_mitigation_strategy(Some(0.005), 10_000, &analysis);
        assert_eq!(strat, MitigationStrategy::MeasurementCorrectionOnly);
    }

    #[test]
    fn test_mitigation_zne_medium_noise() {
        let analysis = make_analysis(5, 10, 0.5);
        let strat = select_mitigation_strategy(Some(0.05), 10_000, &analysis);
        match strat {
            MitigationStrategy::ZneWithScales(scales) => {
                assert!(scales.contains(&1.0));
                assert!(scales.len() >= 3);
            }
            other => panic!("Expected ZneWithScales, got {:?}", other),
        }
    }

    #[test]
    fn test_mitigation_full_for_high_noise() {
        let analysis = make_analysis(5, 10, 0.5);
        let strat = select_mitigation_strategy(Some(0.7), 100_000, &analysis);
        match strat {
            MitigationStrategy::Full {
                zne_scales,
                cdr_circuits,
            } => {
                assert!(zne_scales.len() >= 3);
                assert!(cdr_circuits >= 2);
            }
            other => panic!("Expected Full mitigation, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Verification policy tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_verification_clifford_check() {
        let analysis = make_analysis(10, 50, 1.0);
        let policy = select_verification_policy(&analysis, BackendType::Stabilizer, 10);
        assert_eq!(policy, VerificationPolicy::ExactCliffordCheck);
    }

    #[test]
    fn test_verification_none_for_small_sv() {
        let analysis = make_analysis(5, 10, 0.5);
        let policy = select_verification_policy(&analysis, BackendType::StateVector, 5);
        assert_eq!(policy, VerificationPolicy::None);
    }

    #[test]
    fn test_verification_statistical_for_tn() {
        let analysis = make_analysis(50, 100, 0.5);
        let policy = select_verification_policy(&analysis, BackendType::TensorNetwork, 50);
        match policy {
            VerificationPolicy::StatisticalSampling(n) => {
                assert!(n >= 5, "Should sample at least 5 observables");
            }
            other => panic!("Expected StatisticalSampling for TN, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // PlannerConfig default test
    // -----------------------------------------------------------------------

    #[test]
    fn test_planner_config_default() {
        let config = PlannerConfig::default();
        assert_eq!(config.available_memory_bytes, 8 * 1024 * 1024 * 1024);
        assert!(config.noise_level.is_none());
        assert_eq!(config.shot_budget, 10_000);
        assert!((config.target_precision - 0.01).abs() < 1e-12);
    }

    // -----------------------------------------------------------------------
    // ExecutionPlan explanation test
    // -----------------------------------------------------------------------

    #[test]
    fn test_plan_has_nonempty_explanation() {
        let mut circ = QuantumCircuit::new(3);
        circ.h(0).cnot(0, 1);
        let plan = plan_execution(&circ, &default_config());
        assert!(
            !plan.explanation.is_empty(),
            "Plan explanation should not be empty"
        );
    }

    // -----------------------------------------------------------------------
    // Edge case: 0-qubit circuit
    // -----------------------------------------------------------------------

    #[test]
    fn test_zero_qubit_circuit() {
        let circ = QuantumCircuit::new(0);
        let plan = plan_execution(&circ, &default_config());
        // Should not panic; stabilizer since it's "pure Clifford" (no gates).
        assert_eq!(plan.backend, BackendType::Stabilizer);
    }

    // -----------------------------------------------------------------------
    // Helper: build a CircuitAnalysis stub for unit tests of sub-functions
    // -----------------------------------------------------------------------

    fn make_analysis(
        num_qubits: u32,
        total_gates: usize,
        clifford_fraction: f64,
    ) -> CircuitAnalysis {
        let clifford_gates = (total_gates as f64 * clifford_fraction).round() as usize;
        let non_clifford_gates = total_gates - clifford_gates;

        CircuitAnalysis {
            num_qubits,
            total_gates,
            clifford_gates,
            non_clifford_gates,
            clifford_fraction,
            measurement_gates: 0,
            depth: total_gates as u32,
            max_connectivity: 1,
            is_nearest_neighbor: true,
            recommended_backend: BackendType::Auto,
            confidence: 0.5,
            explanation: String::new(),
        }
    }

    // -----------------------------------------------------------------------
    // CliffordT routing test
    // -----------------------------------------------------------------------

    #[test]
    fn test_clifford_t_routing() {
        // A large circuit with moderate T-count should route to CliffordT.
        let mut circ = QuantumCircuit::new(50);
        for q in 0..50 {
            circ.h(q);
        }
        for q in 0..49 {
            circ.cnot(q, q + 1);
        }
        // Add 15 T-gates (moderate count, 2^15 = 32768 terms).
        for q in 0..15 {
            circ.t(q);
        }

        let config = default_config();
        let plan = plan_execution(&circ, &config);
        assert_eq!(
            plan.backend,
            BackendType::CliffordT,
            "50 qubits with 15 T-gates should use CliffordT backend"
        );
        assert!(plan.confidence >= 0.85);
    }
}
