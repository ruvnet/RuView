//! Unified simulation backend trait and automatic backend selection.
//!
//! ruqu-core supports multiple simulation backends, each optimal for
//! different circuit structures:
//!
//! | Backend | Qubits | Best for |
//! |---------|--------|----------|
//! | StateVector | up to ~32 | General circuits, exact simulation |
//! | Stabilizer | millions | Clifford circuits + measurement |
//! | TensorNetwork | hundreds-thousands | Low-depth, local connectivity |

use crate::circuit::QuantumCircuit;
use crate::gate::Gate;

// ---------------------------------------------------------------------------
// Backend type enum
// ---------------------------------------------------------------------------

/// Which backend to use for simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    /// Dense state-vector (exact, up to ~32 qubits).
    StateVector,
    /// Aaronson-Gottesman stabilizer tableau (Clifford-only, millions of qubits).
    Stabilizer,
    /// Matrix Product State tensor network (bounded entanglement, hundreds+).
    TensorNetwork,
    /// Clifford+T stabilizer rank decomposition (moderate T-count, many qubits).
    CliffordT,
    /// Automatically select the best backend based on circuit analysis.
    Auto,
}

// ---------------------------------------------------------------------------
// Circuit analysis result
// ---------------------------------------------------------------------------

/// Result of circuit analysis, used for backend selection.
///
/// Produced by [`analyze_circuit`] and contains both raw statistics about the
/// circuit (gate counts, depth, connectivity) and a recommended backend with
/// a confidence score and human-readable explanation.
#[derive(Debug, Clone)]
pub struct CircuitAnalysis {
    /// Number of qubits in the circuit.
    pub num_qubits: u32,
    /// Total number of gates.
    pub total_gates: usize,
    /// Number of Clifford gates (H, S, CNOT, CZ, SWAP, X, Y, Z, Sdg).
    pub clifford_gates: usize,
    /// Number of non-Clifford gates (T, Tdg, Rx, Ry, Rz, Phase, Rzz, Unitary1Q).
    pub non_clifford_gates: usize,
    /// Fraction of unitary gates that are Clifford (0.0 to 1.0).
    pub clifford_fraction: f64,
    /// Number of measurement gates.
    pub measurement_gates: usize,
    /// Circuit depth (longest qubit timeline).
    pub depth: u32,
    /// Maximum qubit distance in any two-qubit gate.
    pub max_connectivity: u32,
    /// Whether all two-qubit gates are between adjacent qubits.
    pub is_nearest_neighbor: bool,
    /// Recommended backend based on the analysis heuristics.
    pub recommended_backend: BackendType,
    /// Confidence in the recommendation (0.0 to 1.0).
    pub confidence: f64,
    /// Human-readable explanation of the recommendation.
    pub explanation: String,
}

// ---------------------------------------------------------------------------
// Public analysis entry point
// ---------------------------------------------------------------------------

/// Analyze a quantum circuit to determine the optimal simulation backend.
///
/// Walks the gate list once to collect statistics, then applies a series of
/// heuristic rules to recommend a [`BackendType`]. The returned
/// [`CircuitAnalysis`] contains both the raw numbers and the recommendation.
///
/// # Example
///
/// ```
/// use ruqu_core::circuit::QuantumCircuit;
/// use ruqu_core::backend::{analyze_circuit, BackendType};
///
/// // A small circuit with a non-Clifford gate routes to StateVector.
/// let mut circ = QuantumCircuit::new(3);
/// circ.h(0).t(1).cnot(0, 1);
/// let analysis = analyze_circuit(&circ);
/// assert_eq!(analysis.recommended_backend, BackendType::StateVector);
/// ```
pub fn analyze_circuit(circuit: &QuantumCircuit) -> CircuitAnalysis {
    let num_qubits = circuit.num_qubits();
    let gates = circuit.gates();
    let total_gates = gates.len();

    let mut clifford_gates = 0usize;
    let mut non_clifford_gates = 0usize;
    let mut measurement_gates = 0usize;
    let mut max_connectivity: u32 = 0;
    let mut is_nearest_neighbor = true;

    for gate in gates {
        match gate {
            // Clifford gates
            Gate::H(_)
            | Gate::X(_)
            | Gate::Y(_)
            | Gate::Z(_)
            | Gate::S(_)
            | Gate::Sdg(_)
            | Gate::CNOT(_, _)
            | Gate::CZ(_, _)
            | Gate::SWAP(_, _) => {
                clifford_gates += 1;
            }
            // Non-Clifford gates
            Gate::T(_)
            | Gate::Tdg(_)
            | Gate::Rx(_, _)
            | Gate::Ry(_, _)
            | Gate::Rz(_, _)
            | Gate::Phase(_, _)
            | Gate::Rzz(_, _, _)
            | Gate::Unitary1Q(_, _) => {
                non_clifford_gates += 1;
            }
            Gate::Measure(_) => {
                measurement_gates += 1;
            }
            Gate::Reset(_) | Gate::Barrier => {}
        }

        // Check connectivity for two-qubit gates.
        let qubits = gate.qubits();
        if qubits.len() == 2 {
            let dist = if qubits[0] > qubits[1] {
                qubits[0] - qubits[1]
            } else {
                qubits[1] - qubits[0]
            };
            if dist > max_connectivity {
                max_connectivity = dist;
            }
            if dist > 1 {
                is_nearest_neighbor = false;
            }
        }
    }

    let unitary_gates = clifford_gates + non_clifford_gates;
    let clifford_fraction = if unitary_gates > 0 {
        clifford_gates as f64 / unitary_gates as f64
    } else {
        1.0
    };

    let depth = circuit.depth();

    // Decide which backend fits best.
    let (recommended_backend, confidence, explanation) = select_backend(
        num_qubits,
        clifford_fraction,
        non_clifford_gates,
        depth,
        is_nearest_neighbor,
        max_connectivity,
    );

    CircuitAnalysis {
        num_qubits,
        total_gates,
        clifford_gates,
        non_clifford_gates,
        clifford_fraction,
        measurement_gates,
        depth,
        max_connectivity,
        is_nearest_neighbor,
        recommended_backend,
        confidence,
        explanation,
    }
}

// ---------------------------------------------------------------------------
// Internal selection heuristics
// ---------------------------------------------------------------------------

/// Internal backend selection logic.
///
/// Returns `(backend, confidence, explanation)` based on a priority-ordered
/// set of heuristic rules.
fn select_backend(
    num_qubits: u32,
    clifford_fraction: f64,
    non_clifford_gates: usize,
    depth: u32,
    is_nearest_neighbor: bool,
    max_connectivity: u32,
) -> (BackendType, f64, String) {
    // Rule 1: Pure Clifford circuits -> Stabilizer (any size).
    if clifford_fraction >= 1.0 {
        return (
            BackendType::Stabilizer,
            0.99,
            format!(
                "Pure Clifford circuit: stabilizer backend handles {} qubits in O(n^2) per gate",
                num_qubits
            ),
        );
    }

    // Rule 2: Mostly Clifford with very few non-Clifford gates and too many
    // qubits for state vector -> Stabilizer with approximate decomposition.
    if clifford_fraction >= 0.95 && num_qubits > 32 && non_clifford_gates <= 10 {
        return (
            BackendType::Stabilizer,
            0.85,
            format!(
                "{}% Clifford with only {} non-Clifford gates: \
                 stabilizer backend recommended for {} qubits",
                (clifford_fraction * 100.0) as u32,
                non_clifford_gates,
                num_qubits
            ),
        );
    }

    // Rule 3: Small enough for state vector -> use it (exact, comfortable).
    if num_qubits <= 25 {
        return (
            BackendType::StateVector,
            0.95,
            format!(
                "{} qubits fits comfortably in state vector ({})",
                num_qubits,
                format_memory(num_qubits)
            ),
        );
    }

    // Rule 4: State vector possible but tight on memory.
    if num_qubits <= 32 {
        return (
            BackendType::StateVector,
            0.80,
            format!(
                "{} qubits requires {} for state vector - verify available memory",
                num_qubits,
                format_memory(num_qubits)
            ),
        );
    }

    // Rule 5: Low depth, local connectivity -> tensor network.
    if is_nearest_neighbor && depth < num_qubits * 2 {
        return (
            BackendType::TensorNetwork,
            0.85,
            format!(
                "Nearest-neighbor connectivity with depth {} on {} qubits: \
                 tensor network efficient",
                depth, num_qubits
            ),
        );
    }

    // Rule 6: General large circuit -> tensor network as best approximation.
    if num_qubits > 32 {
        let conf = if is_nearest_neighbor { 0.75 } else { 0.55 };
        return (
            BackendType::TensorNetwork,
            conf,
            format!(
                "{} qubits exceeds state vector capacity. \
                 Tensor network with connectivity {} - results are approximate",
                num_qubits, max_connectivity
            ),
        );
    }

    // Fallback: exact state vector simulation.
    (
        BackendType::StateVector,
        0.70,
        "Default to exact state vector simulation".into(),
    )
}

// ---------------------------------------------------------------------------
// Memory formatting helper
// ---------------------------------------------------------------------------

/// Format the state-vector memory requirement for a given qubit count.
///
/// Each amplitude is a `Complex` (16 bytes), and there are `2^n` of them.
fn format_memory(num_qubits: u32) -> String {
    // Use u128 to avoid overflow for up to 127 qubits.
    let bytes = (1u128 << num_qubits) * 16;
    if bytes >= 1 << 40 {
        format!("{:.1} TiB", bytes as f64 / (1u128 << 40) as f64)
    } else if bytes >= 1 << 30 {
        format!("{:.1} GiB", bytes as f64 / (1u128 << 30) as f64)
    } else if bytes >= 1 << 20 {
        format!("{:.1} MiB", bytes as f64 / (1u128 << 20) as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

// ---------------------------------------------------------------------------
// Scaling information
// ---------------------------------------------------------------------------

/// Scaling characteristics for a single simulation backend.
#[derive(Debug, Clone)]
pub struct ScalingInfo {
    /// The backend this info describes.
    pub backend: BackendType,
    /// Maximum qubits for exact (zero-error) simulation.
    pub max_qubits_exact: u32,
    /// Maximum qubits for approximate simulation with truncation.
    pub max_qubits_approximate: u32,
    /// Time complexity in big-O notation.
    pub time_complexity: String,
    /// Space complexity in big-O notation.
    pub space_complexity: String,
}

/// Get scaling information for all supported backends.
///
/// Returns a `Vec` with one [`ScalingInfo`] per backend (StateVector,
/// Stabilizer, TensorNetwork, CliffordT) in that order.
pub fn scaling_report() -> Vec<ScalingInfo> {
    vec![
        ScalingInfo {
            backend: BackendType::StateVector,
            max_qubits_exact: 32,
            max_qubits_approximate: 36,
            time_complexity: "O(2^n * gates)".into(),
            space_complexity: "O(2^n)".into(),
        },
        ScalingInfo {
            backend: BackendType::Stabilizer,
            max_qubits_exact: 10_000_000,
            max_qubits_approximate: 10_000_000,
            time_complexity: "O(n^2 * gates) for Clifford".into(),
            space_complexity: "O(n^2)".into(),
        },
        ScalingInfo {
            backend: BackendType::TensorNetwork,
            max_qubits_exact: 100,
            max_qubits_approximate: 10_000,
            time_complexity: "O(n * chi^3 * gates)".into(),
            space_complexity: "O(n * chi^2)".into(),
        },
        ScalingInfo {
            backend: BackendType::CliffordT,
            max_qubits_exact: 1000,
            max_qubits_approximate: 10_000,
            time_complexity: "O(2^t * n^2 * gates) for t T-gates".into(),
            space_complexity: "O(2^t * n^2)".into(),
        },
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::QuantumCircuit;

    #[test]
    fn pure_clifford_selects_stabilizer() {
        let mut circ = QuantumCircuit::new(50);
        for q in 0..50 {
            circ.h(q);
        }
        for q in 0..49 {
            circ.cnot(q, q + 1);
        }
        let analysis = analyze_circuit(&circ);
        assert_eq!(analysis.recommended_backend, BackendType::Stabilizer);
        assert!(analysis.clifford_fraction >= 1.0);
        assert!(analysis.confidence > 0.9);
    }

    #[test]
    fn small_circuit_selects_state_vector() {
        let mut circ = QuantumCircuit::new(5);
        circ.h(0).t(1).cnot(0, 1);
        let analysis = analyze_circuit(&circ);
        assert_eq!(analysis.recommended_backend, BackendType::StateVector);
        assert!(analysis.confidence > 0.9);
    }

    #[test]
    fn medium_circuit_selects_state_vector() {
        let mut circ = QuantumCircuit::new(30);
        circ.h(0).rx(1, 1.0).cnot(0, 1);
        let analysis = analyze_circuit(&circ);
        assert_eq!(analysis.recommended_backend, BackendType::StateVector);
        assert!(analysis.confidence >= 0.80);
    }

    #[test]
    fn large_nearest_neighbor_selects_tensor_network() {
        let mut circ = QuantumCircuit::new(64);
        // Low depth, nearest-neighbor only.
        for q in 0..63 {
            circ.cnot(q, q + 1);
        }
        // Add enough non-Clifford gates to avoid the "mostly Clifford" Rule 2
        // (which requires non_clifford_gates <= 10).
        for q in 0..12 {
            circ.t(q);
        }
        let analysis = analyze_circuit(&circ);
        assert_eq!(analysis.recommended_backend, BackendType::TensorNetwork);
    }

    #[test]
    fn empty_circuit_defaults() {
        let circ = QuantumCircuit::new(10);
        let analysis = analyze_circuit(&circ);
        // Empty circuit is "pure Clifford" (no non-Clifford gates).
        assert_eq!(analysis.total_gates, 0);
        assert!(analysis.clifford_fraction >= 1.0);
    }

    #[test]
    fn measurement_counted() {
        let mut circ = QuantumCircuit::new(3);
        circ.h(0).measure(0).measure(1).measure(2);
        let analysis = analyze_circuit(&circ);
        assert_eq!(analysis.measurement_gates, 3);
    }

    #[test]
    fn connectivity_detected() {
        let mut circ = QuantumCircuit::new(10);
        circ.cnot(0, 5); // distance = 5
        let analysis = analyze_circuit(&circ);
        assert_eq!(analysis.max_connectivity, 5);
        assert!(!analysis.is_nearest_neighbor);
    }

    #[test]
    fn scaling_report_has_four_entries() {
        let report = scaling_report();
        assert_eq!(report.len(), 4);
        assert_eq!(report[0].backend, BackendType::StateVector);
        assert_eq!(report[1].backend, BackendType::Stabilizer);
        assert_eq!(report[2].backend, BackendType::TensorNetwork);
        assert_eq!(report[3].backend, BackendType::CliffordT);
    }

    #[test]
    fn format_memory_values() {
        // 10 qubits => 2^10 * 16 = 16384 bytes
        assert_eq!(format_memory(10), "16384 bytes");
        // 20 qubits => 2^20 * 16 = 16 MiB
        assert_eq!(format_memory(20), "16.0 MiB");
        // 30 qubits => 2^30 * 16 = 16 GiB
        assert_eq!(format_memory(30), "16.0 GiB");
    }
}
