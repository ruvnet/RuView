//! Gate-fusion optimiser
//!
//! Scans a circuit for runs of consecutive single-qubit gates acting on the
//! same qubit and fuses them into a single `Unitary1Q` gate by multiplying
//! their 2x2 matrices.

use crate::circuit::QuantumCircuit;
use crate::gate::Gate;
use crate::types::Complex;

/// Multiply two 2x2 complex matrices: C = A * B.
pub fn mat_mul_2x2(a: &[[Complex; 2]; 2], b: &[[Complex; 2]; 2]) -> [[Complex; 2]; 2] {
    [
        [
            a[0][0] * b[0][0] + a[0][1] * b[1][0],
            a[0][0] * b[0][1] + a[0][1] * b[1][1],
        ],
        [
            a[1][0] * b[0][0] + a[1][1] * b[1][0],
            a[1][0] * b[0][1] + a[1][1] * b[1][1],
        ],
    ]
}

/// Check whether two gates can be fused.
///
/// Both must be non-measurement single-qubit unitaries acting on the same qubit.
pub fn can_fuse(a: &Gate, b: &Gate) -> bool {
    if a.is_non_unitary() || b.is_non_unitary() {
        return false;
    }
    match (a.matrix_1q(), b.matrix_1q()) {
        (Some(_), Some(_)) => {
            let qa = a.qubits();
            let qb = b.qubits();
            qa.len() == 1 && qb.len() == 1 && qa[0] == qb[0]
        }
        _ => false,
    }
}

/// Optimise a circuit by greedily fusing consecutive single-qubit gates
/// that act on the same qubit.
///
/// Returns a new, potentially shorter circuit.
pub fn fuse_gates(circuit: &QuantumCircuit) -> QuantumCircuit {
    let mut result = QuantumCircuit::new(circuit.num_qubits());
    let gates = circuit.gates();
    let len = gates.len();
    let mut i = 0;

    while i < len {
        // Attempt to start a fusion run if the current gate is a fusable 1Q gate.
        if !gates[i].is_non_unitary() {
            if let Some(first_matrix) = gates[i].matrix_1q() {
                let q = gates[i].qubits()[0];
                let mut fused = first_matrix;
                let mut count = 1usize;

                // Greedily absorb subsequent fusable 1Q gates on the same qubit.
                while i + count < len {
                    let next = &gates[i + count];
                    if next.is_non_unitary() {
                        break;
                    }
                    if let Some(next_m) = next.matrix_1q() {
                        let nq = next.qubits();
                        if nq.len() == 1 && nq[0] == q {
                            // next_m is applied *after* fused, so C = next_m * fused.
                            fused = mat_mul_2x2(&next_m, &fused);
                            count += 1;
                            continue;
                        }
                    }
                    break;
                }

                if count > 1 {
                    result.add_gate(Gate::Unitary1Q(q, fused));
                } else {
                    result.add_gate(gates[i].clone());
                }
                i += count;
                continue;
            }
        }

        // Non-fusable gate: pass through unchanged.
        result.add_gate(gates[i].clone());
        i += 1;
    }

    result
}
