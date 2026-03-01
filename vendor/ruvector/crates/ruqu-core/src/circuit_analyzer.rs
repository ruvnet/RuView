//! Circuit analysis utilities for simulation backend selection.
//!
//! Provides detailed structural analysis of quantum circuits to enable
//! intelligent routing to the optimal simulation backend. This module
//! complements [`crate::backend`] by exposing lower-level classification
//! and structural queries that advanced users or future optimisation passes
//! may need independently.

use crate::circuit::QuantumCircuit;
use crate::gate::Gate;
use crate::types::QubitIndex;
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Gate classification
// ---------------------------------------------------------------------------

/// Detailed gate classification for routing decisions.
///
/// Every [`Gate`] variant maps to exactly one `GateClass`, making it easy to
/// partition a circuit by gate type without pattern-matching on every variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateClass {
    /// Clifford gate (H, S, Sdg, X, Y, Z, CNOT, CZ, SWAP).
    Clifford,
    /// Non-Clifford unitary (T, Tdg, rotations, custom unitary).
    NonClifford,
    /// Measurement operation.
    Measurement,
    /// Reset operation.
    Reset,
    /// Barrier (scheduling hint, no physical effect).
    Barrier,
}

/// Classify a single gate for backend routing.
///
/// # Example
///
/// ```
/// use ruqu_core::gate::Gate;
/// use ruqu_core::circuit_analyzer::{classify_gate, GateClass};
///
/// assert_eq!(classify_gate(&Gate::H(0)), GateClass::Clifford);
/// assert_eq!(classify_gate(&Gate::T(0)), GateClass::NonClifford);
/// assert_eq!(classify_gate(&Gate::Measure(0)), GateClass::Measurement);
/// ```
pub fn classify_gate(gate: &Gate) -> GateClass {
    match gate {
        Gate::H(_)
        | Gate::X(_)
        | Gate::Y(_)
        | Gate::Z(_)
        | Gate::S(_)
        | Gate::Sdg(_)
        | Gate::CNOT(_, _)
        | Gate::CZ(_, _)
        | Gate::SWAP(_, _) => GateClass::Clifford,

        Gate::T(_)
        | Gate::Tdg(_)
        | Gate::Rx(_, _)
        | Gate::Ry(_, _)
        | Gate::Rz(_, _)
        | Gate::Phase(_, _)
        | Gate::Rzz(_, _, _)
        | Gate::Unitary1Q(_, _) => GateClass::NonClifford,

        Gate::Measure(_) => GateClass::Measurement,
        Gate::Reset(_) => GateClass::Reset,
        Gate::Barrier => GateClass::Barrier,
    }
}

// ---------------------------------------------------------------------------
// Clifford analysis
// ---------------------------------------------------------------------------

/// Check if a circuit is entirely Clifford-compatible.
///
/// A circuit is Clifford-compatible when every gate is either a Clifford
/// unitary, a measurement, a reset, or a barrier. Such circuits can be
/// simulated in polynomial time using the stabilizer formalism.
///
/// # Example
///
/// ```
/// use ruqu_core::circuit::QuantumCircuit;
/// use ruqu_core::circuit_analyzer::is_clifford_circuit;
///
/// let mut circ = QuantumCircuit::new(3);
/// circ.h(0).cnot(0, 1).cnot(1, 2);
/// assert!(is_clifford_circuit(&circ));
///
/// circ.t(0);
/// assert!(!is_clifford_circuit(&circ));
/// ```
pub fn is_clifford_circuit(circuit: &QuantumCircuit) -> bool {
    circuit.gates().iter().all(|g| {
        let class = classify_gate(g);
        class == GateClass::Clifford
            || class == GateClass::Measurement
            || class == GateClass::Reset
            || class == GateClass::Barrier
    })
}

/// Count the number of non-Clifford gates in a circuit.
///
/// This is the primary cost metric for stabilizer-based simulation with
/// magic-state injection: each non-Clifford gate requires exponentially
/// more resources to handle exactly.
pub fn count_non_clifford(circuit: &QuantumCircuit) -> usize {
    circuit
        .gates()
        .iter()
        .filter(|g| classify_gate(g) == GateClass::NonClifford)
        .count()
}

// ---------------------------------------------------------------------------
// Entanglement and connectivity analysis
// ---------------------------------------------------------------------------

/// Analyze the entanglement structure of a circuit.
///
/// Returns the set of qubit pairs that are directly entangled by at least
/// one two-qubit gate. Pairs are returned with the smaller index first.
///
/// # Example
///
/// ```
/// use ruqu_core::circuit::QuantumCircuit;
/// use ruqu_core::circuit_analyzer::entanglement_pairs;
///
/// let mut circ = QuantumCircuit::new(4);
/// circ.cnot(0, 2).cz(1, 3);
/// let pairs = entanglement_pairs(&circ);
/// assert!(pairs.contains(&(0, 2)));
/// assert!(pairs.contains(&(1, 3)));
/// assert_eq!(pairs.len(), 2);
/// ```
pub fn entanglement_pairs(circuit: &QuantumCircuit) -> HashSet<(QubitIndex, QubitIndex)> {
    let mut pairs = HashSet::new();
    for gate in circuit.gates() {
        let qubits = gate.qubits();
        if qubits.len() == 2 {
            let (a, b) = if qubits[0] < qubits[1] {
                (qubits[0], qubits[1])
            } else {
                (qubits[1], qubits[0])
            };
            pairs.insert((a, b));
        }
    }
    pairs
}

/// Check if all two-qubit gates act on nearest-neighbor qubits.
///
/// A circuit with only nearest-neighbor interactions maps efficiently to
/// linear qubit topologies and is a good candidate for Matrix Product State
/// (MPS) tensor-network simulation.
pub fn is_nearest_neighbor(circuit: &QuantumCircuit) -> bool {
    circuit.gates().iter().all(|gate| {
        let qubits = gate.qubits();
        if qubits.len() == 2 {
            let dist = if qubits[0] > qubits[1] {
                qubits[0] - qubits[1]
            } else {
                qubits[1] - qubits[0]
            };
            dist <= 1
        } else {
            true
        }
    })
}

// ---------------------------------------------------------------------------
// Bond dimension estimation
// ---------------------------------------------------------------------------

/// Estimate the maximum bond dimension needed for MPS simulation.
///
/// Scans every possible bipartition of the qubit register (cuts between
/// position `k-1` and `k` for `k` in `1..n`) and counts how many two-qubit
/// gates straddle each cut. The bond dimension grows exponentially with the
/// number of entangling gates across the worst-case cut, capped at 2^20
/// (roughly 1 million) as a practical limit.
///
/// This is a rough *upper bound*; cancellations and limited entanglement
/// growth mean the actual bond dimension required may be much lower.
pub fn estimate_bond_dimension(circuit: &QuantumCircuit) -> usize {
    let n = circuit.num_qubits();
    let mut max_entanglement_across_cut = 0usize;

    // For each possible bipartition cut position.
    for cut in 1..n {
        let mut gates_crossing_cut = 0usize;
        for gate in circuit.gates() {
            let qubits = gate.qubits();
            if qubits.len() == 2 {
                let (lo, hi) = if qubits[0] < qubits[1] {
                    (qubits[0], qubits[1])
                } else {
                    (qubits[1], qubits[0])
                };
                if lo < cut && hi >= cut {
                    gates_crossing_cut += 1;
                }
            }
        }
        if gates_crossing_cut > max_entanglement_across_cut {
            max_entanglement_across_cut = gates_crossing_cut;
        }
    }

    // Bond dimension is 2^(gates across cut), bounded to avoid overflow.
    let exponent = max_entanglement_across_cut.min(20) as u32;
    2usize.saturating_pow(exponent)
}

// ---------------------------------------------------------------------------
// Circuit summary
// ---------------------------------------------------------------------------

/// Summary of circuit characteristics for display and diagnostics.
#[derive(Debug, Clone)]
pub struct CircuitSummary {
    /// Number of qubits in the register.
    pub num_qubits: u32,
    /// Circuit depth (longest qubit timeline).
    pub depth: u32,
    /// Total number of gates (including measurements and barriers).
    pub total_gates: usize,
    /// Number of Clifford gates.
    pub clifford_count: usize,
    /// Number of non-Clifford unitary gates.
    pub non_clifford_count: usize,
    /// Number of measurement gates.
    pub measurement_count: usize,
    /// Whether the circuit contains only Clifford gates (plus measurements/resets).
    pub is_clifford_only: bool,
    /// Whether all two-qubit gates are nearest-neighbor.
    pub is_nearest_neighbor: bool,
    /// Estimated maximum MPS bond dimension.
    pub estimated_bond_dim: usize,
    /// Human-readable state-vector memory requirement.
    pub state_vector_memory: String,
}

/// Generate a comprehensive summary of a circuit.
///
/// Collects all structural statistics in a single pass and returns them
/// in a [`CircuitSummary`] suitable for logging or display.
///
/// # Example
///
/// ```
/// use ruqu_core::circuit::QuantumCircuit;
/// use ruqu_core::circuit_analyzer::summarize_circuit;
///
/// let mut circ = QuantumCircuit::new(4);
/// circ.h(0).cnot(0, 1).t(2).measure(3);
/// let summary = summarize_circuit(&circ);
/// assert_eq!(summary.num_qubits, 4);
/// assert_eq!(summary.clifford_count, 2);
/// assert_eq!(summary.non_clifford_count, 1);
/// assert_eq!(summary.measurement_count, 1);
/// ```
pub fn summarize_circuit(circuit: &QuantumCircuit) -> CircuitSummary {
    let num_qubits = circuit.num_qubits();
    let total_gates = circuit.gate_count();
    let depth = circuit.depth();

    let mut clifford_count = 0;
    let mut non_clifford_count = 0;
    let mut measurement_count = 0;

    for gate in circuit.gates() {
        match classify_gate(gate) {
            GateClass::Clifford => clifford_count += 1,
            GateClass::NonClifford => non_clifford_count += 1,
            GateClass::Measurement => measurement_count += 1,
            _ => {}
        }
    }

    let state_vector_memory = format_sv_memory(num_qubits);

    CircuitSummary {
        num_qubits,
        depth,
        total_gates,
        clifford_count,
        non_clifford_count,
        measurement_count,
        is_clifford_only: non_clifford_count == 0,
        is_nearest_neighbor: is_nearest_neighbor(circuit),
        estimated_bond_dim: estimate_bond_dimension(circuit),
        state_vector_memory,
    }
}

/// Format the state-vector memory requirement for display.
fn format_sv_memory(num_qubits: u32) -> String {
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::QuantumCircuit;

    #[test]
    fn classify_all_gate_types() {
        assert_eq!(classify_gate(&Gate::H(0)), GateClass::Clifford);
        assert_eq!(classify_gate(&Gate::X(0)), GateClass::Clifford);
        assert_eq!(classify_gate(&Gate::Y(0)), GateClass::Clifford);
        assert_eq!(classify_gate(&Gate::Z(0)), GateClass::Clifford);
        assert_eq!(classify_gate(&Gate::S(0)), GateClass::Clifford);
        assert_eq!(classify_gate(&Gate::Sdg(0)), GateClass::Clifford);
        assert_eq!(classify_gate(&Gate::CNOT(0, 1)), GateClass::Clifford);
        assert_eq!(classify_gate(&Gate::CZ(0, 1)), GateClass::Clifford);
        assert_eq!(classify_gate(&Gate::SWAP(0, 1)), GateClass::Clifford);

        assert_eq!(classify_gate(&Gate::T(0)), GateClass::NonClifford);
        assert_eq!(classify_gate(&Gate::Tdg(0)), GateClass::NonClifford);
        assert_eq!(classify_gate(&Gate::Rx(0, 1.0)), GateClass::NonClifford);
        assert_eq!(classify_gate(&Gate::Ry(0, 1.0)), GateClass::NonClifford);
        assert_eq!(classify_gate(&Gate::Rz(0, 1.0)), GateClass::NonClifford);
        assert_eq!(classify_gate(&Gate::Phase(0, 1.0)), GateClass::NonClifford);
        assert_eq!(classify_gate(&Gate::Rzz(0, 1, 1.0)), GateClass::NonClifford);

        assert_eq!(classify_gate(&Gate::Measure(0)), GateClass::Measurement);
        assert_eq!(classify_gate(&Gate::Reset(0)), GateClass::Reset);
        assert_eq!(classify_gate(&Gate::Barrier), GateClass::Barrier);
    }

    #[test]
    fn clifford_circuit_detection() {
        let mut circ = QuantumCircuit::new(4);
        circ.h(0).cnot(0, 1).s(2).cz(2, 3).measure(0);
        assert!(is_clifford_circuit(&circ));

        circ.t(0);
        assert!(!is_clifford_circuit(&circ));
    }

    #[test]
    fn non_clifford_count() {
        let mut circ = QuantumCircuit::new(3);
        circ.h(0).t(0).t(1).rx(2, 0.5);
        assert_eq!(count_non_clifford(&circ), 3);
    }

    #[test]
    fn entanglement_pair_tracking() {
        let mut circ = QuantumCircuit::new(5);
        circ.cnot(0, 3).cz(1, 4).swap(0, 3);
        let pairs = entanglement_pairs(&circ);
        assert!(pairs.contains(&(0, 3)));
        assert!(pairs.contains(&(1, 4)));
        // Duplicate pair (0,3) should not increase count.
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn nearest_neighbor_detection() {
        let mut circ = QuantumCircuit::new(4);
        circ.cnot(0, 1).cnot(1, 2).cnot(2, 3);
        assert!(is_nearest_neighbor(&circ));

        circ.cnot(0, 3);
        assert!(!is_nearest_neighbor(&circ));
    }

    #[test]
    fn bond_dimension_empty_circuit() {
        let circ = QuantumCircuit::new(5);
        assert_eq!(estimate_bond_dimension(&circ), 1);
    }

    #[test]
    fn bond_dimension_linear_chain() {
        let mut circ = QuantumCircuit::new(4);
        // Single CNOT across cut at position 2: only one gate crosses.
        circ.cnot(1, 2);
        // Expected: 2^1 = 2
        assert_eq!(estimate_bond_dimension(&circ), 2);
    }

    #[test]
    fn bond_dimension_multiple_crossings() {
        let mut circ = QuantumCircuit::new(4);
        // Three gates cross the cut between qubit 1 and qubit 2.
        circ.cnot(0, 2).cnot(1, 3).cnot(0, 3);
        // Cut at position 2: all three gates cross -> 2^3 = 8
        assert_eq!(estimate_bond_dimension(&circ), 8);
    }

    #[test]
    fn summary_basic() {
        let mut circ = QuantumCircuit::new(4);
        circ.h(0).t(1).cnot(0, 1).measure(0).measure(1);
        let summary = summarize_circuit(&circ);

        assert_eq!(summary.num_qubits, 4);
        assert_eq!(summary.total_gates, 5);
        assert_eq!(summary.clifford_count, 2); // H + CNOT
        assert_eq!(summary.non_clifford_count, 1); // T
        assert_eq!(summary.measurement_count, 2);
        assert!(!summary.is_clifford_only);
        assert!(summary.is_nearest_neighbor);
    }

    #[test]
    fn summary_clifford_only_flag() {
        let mut circ = QuantumCircuit::new(2);
        circ.h(0).cnot(0, 1);
        let summary = summarize_circuit(&circ);
        assert!(summary.is_clifford_only);
    }

    #[test]
    fn summary_memory_string() {
        let circ = QuantumCircuit::new(10);
        let summary = summarize_circuit(&circ);
        // 2^10 * 16 = 16384 bytes
        assert_eq!(summary.state_vector_memory, "16384 bytes");
    }
}
