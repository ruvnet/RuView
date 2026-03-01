//! Surface Code Error Correction Simulation
//!
//! Simulates a **distance-3 rotated surface code** with:
//!
//! - 9 data qubits (3 x 3 grid)
//! - 4 X-type stabilizers (plaquettes, detect Z errors)
//! - 4 Z-type stabilizers (vertices, detect X errors)
//! - 8 ancilla qubits (one per stabilizer)
//!
//! Each QEC cycle performs:
//! 1. **Noise injection** -- random Pauli errors on data qubits.
//! 2. **Stabilizer measurement** -- entangle ancillas with data qubits and
//!    measure the ancillas to extract the error syndrome.
//! 3. **Decoding** -- a simple lookup decoder maps the syndrome to a
//!    correction (placeholder; production systems would use MWPM).
//! 4. **Correction** -- apply compensating Pauli gates.
//!
//! # Qubit layout (distance 3)
//!
//! ```text
//! Data qubits:        Ancilla assignment:
//!   d0  d1  d2          X-anc: 9, 10, 11, 12
//!   d3  d4  d5          Z-anc: 13, 14, 15, 16
//!   d6  d7  d8
//! ```
//!
//! X stabilizers (plaquettes):
//! - X0 (anc 9):  {d0, d1, d3, d4}
//! - X1 (anc 10): {d1, d2, d4, d5}
//! - X2 (anc 11): {d3, d4, d6, d7}
//! - X3 (anc 12): {d4, d5, d7, d8}
//!
//! Z stabilizers (boundary vertices):
//! - Z0 (anc 13): {d0, d1}
//! - Z1 (anc 14): {d2, d5}
//! - Z2 (anc 15): {d3, d6}
//! - Z3 (anc 16): {d7, d8}

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use ruqu_core::gate::Gate;
use ruqu_core::state::QuantumState;
use ruqu_core::types::QubitIndex;

// ---------------------------------------------------------------------------
// Configuration and result types
// ---------------------------------------------------------------------------

/// Configuration for a surface code error correction simulation.
pub struct SurfaceCodeConfig {
    /// Code distance (currently only 3 is supported).
    pub distance: u32,
    /// Number of QEC syndrome-extraction cycles to run.
    pub num_cycles: u32,
    /// Physical error rate per data qubit per cycle. Each data qubit
    /// independently suffers a Pauli-X with probability `noise_rate` and a
    /// Pauli-Z with probability `noise_rate` (simplified depolarizing model).
    pub noise_rate: f64,
    /// Optional RNG seed for reproducibility.
    pub seed: Option<u64>,
}

/// Result of a surface code simulation.
pub struct SurfaceCodeResult {
    /// Number of detected logical errors (simplified check).
    pub logical_errors: u32,
    /// Total QEC cycles executed.
    pub total_cycles: u32,
    /// Logical error rate = `logical_errors / total_cycles`.
    pub logical_error_rate: f64,
    /// Syndrome bit-vector for each cycle. Each inner `Vec<bool>` has
    /// `num_x_stabilizers + num_z_stabilizers` entries.
    pub syndrome_history: Vec<Vec<bool>>,
}

// ---------------------------------------------------------------------------
// Surface code layout
// ---------------------------------------------------------------------------

/// Physical layout of a surface code: which data qubits participate in each
/// stabilizer, and the ancilla qubit assigned to each stabilizer.
pub struct SurfaceCodeLayout {
    /// Indices of data qubits.
    pub data_qubits: Vec<QubitIndex>,
    /// Indices of X-type (plaquette) ancilla qubits.
    pub x_ancillas: Vec<QubitIndex>,
    /// Indices of Z-type (vertex) ancilla qubits.
    pub z_ancillas: Vec<QubitIndex>,
    /// For each X stabilizer, the data qubits it acts on.
    pub x_stabilizers: Vec<Vec<QubitIndex>>,
    /// For each Z stabilizer, the data qubits it acts on.
    pub z_stabilizers: Vec<Vec<QubitIndex>>,
}

impl SurfaceCodeLayout {
    /// Create the layout for a distance-3 rotated surface code.
    ///
    /// Total qubits: 9 data (indices 0..8) + 4 X-ancillas (9..12) +
    /// 4 Z-ancillas (13..16) = 17.
    pub fn distance_3() -> Self {
        Self {
            data_qubits: (0..9).collect(),
            x_ancillas: vec![9, 10, 11, 12],
            z_ancillas: vec![13, 14, 15, 16],
            x_stabilizers: vec![
                vec![0, 1, 3, 4], // X0: top-left plaquette
                vec![1, 2, 4, 5], // X1: top-right plaquette
                vec![3, 4, 6, 7], // X2: bottom-left plaquette
                vec![4, 5, 7, 8], // X3: bottom-right plaquette
            ],
            z_stabilizers: vec![
                vec![0, 1], // Z0: top boundary
                vec![2, 5], // Z1: right boundary
                vec![3, 6], // Z2: left boundary
                vec![7, 8], // Z3: bottom boundary
            ],
        }
    }

    /// Total number of physical qubits (data + ancilla).
    pub fn total_qubits(&self) -> u32 {
        (self.data_qubits.len() + self.x_ancillas.len() + self.z_ancillas.len()) as u32
    }

    /// Total number of stabilizers (X + Z).
    pub fn num_stabilizers(&self) -> usize {
        self.x_stabilizers.len() + self.z_stabilizers.len()
    }
}

// ---------------------------------------------------------------------------
// Noise injection
// ---------------------------------------------------------------------------

/// Inject simplified depolarizing noise on each data qubit.
///
/// For each data qubit, independently:
/// - With probability `noise_rate`: apply X (bit flip)
/// - With probability `noise_rate`: apply Z (phase flip)
/// - Otherwise: no error
///
/// The two error channels are independent (a qubit can get both X and Z = Y).
fn inject_noise(
    state: &mut QuantumState,
    data_qubits: &[QubitIndex],
    noise_rate: f64,
    rng: &mut StdRng,
) -> ruqu_core::error::Result<()> {
    for &q in data_qubits {
        let r: f64 = rng.gen();
        if r < noise_rate {
            state.apply_gate(&Gate::X(q))?;
        } else if r < 2.0 * noise_rate {
            state.apply_gate(&Gate::Z(q))?;
        }
        // else: no error on this qubit in this channel
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Stabilizer measurement (one QEC cycle)
// ---------------------------------------------------------------------------

/// Execute one QEC cycle: reset ancillas, entangle with data qubits via
/// stabilizer circuits, and measure ancillas.
///
/// Returns the syndrome vector (one bool per stabilizer, X-stabilizers first,
/// then Z-stabilizers). A `true` entry means the stabilizer measured -1
/// (error detected).
fn run_cycle(
    state: &mut QuantumState,
    layout: &SurfaceCodeLayout,
) -> ruqu_core::error::Result<Vec<bool>> {
    // Reset all ancilla qubits to |0>.
    for &a in layout.x_ancillas.iter().chain(layout.z_ancillas.iter()) {
        state.reset_qubit(a)?;
    }

    // ---- X-stabilizer measurement circuits ----
    // To measure the product X_a X_b X_c X_d:
    //   1. H(ancilla)
    //   2. CNOT(ancilla, data_a), ..., CNOT(ancilla, data_d)
    //   3. H(ancilla)
    //   4. Measure ancilla
    for (i, stabilizer) in layout.x_stabilizers.iter().enumerate() {
        let ancilla = layout.x_ancillas[i];
        state.apply_gate(&Gate::H(ancilla))?;
        for &data in stabilizer {
            state.apply_gate(&Gate::CNOT(ancilla, data))?;
        }
        state.apply_gate(&Gate::H(ancilla))?;
    }

    // ---- Z-stabilizer measurement circuits ----
    // To measure the product Z_a Z_b Z_c Z_d:
    //   1. CNOT(data_a, ancilla), ..., CNOT(data_d, ancilla)
    //   2. Measure ancilla
    for (i, stabilizer) in layout.z_stabilizers.iter().enumerate() {
        let ancilla = layout.z_ancillas[i];
        for &data in stabilizer {
            state.apply_gate(&Gate::CNOT(data, ancilla))?;
        }
    }

    // Measure all ancillas and collect syndrome bits.
    let mut syndrome = Vec::with_capacity(layout.num_stabilizers());
    for &a in layout.x_ancillas.iter().chain(layout.z_ancillas.iter()) {
        let outcome = state.measure(a)?;
        syndrome.push(outcome.result);
    }

    Ok(syndrome)
}

// ---------------------------------------------------------------------------
// Syndrome decoder
// ---------------------------------------------------------------------------

/// Simple lookup decoder for the distance-3 surface code.
///
/// This is a **placeholder** decoder that applies a single-qubit X correction
/// on the data qubit most likely responsible for the detected syndrome
/// pattern. A production implementation would use Minimum Weight Perfect
/// Matching (MWPM) via e.g. `fusion-blossom`.
///
/// # Decoding strategy
///
/// The syndrome has 8 bits (4 X-stabilizer + 4 Z-stabilizer). The decoder
/// only looks at the X-stabilizer syndrome (bits 0..3) to correct Z errors
/// and the Z-stabilizer syndrome (bits 4..7) to correct X errors.
///
/// For each stabilizer group, if exactly one stabilizer fires, apply a
/// correction on the first data qubit of that stabilizer. If multiple fire,
/// correct the data qubit shared by the most triggered stabilizers (heuristic).
fn decode_syndrome(syndrome: &[bool], layout: &SurfaceCodeLayout) -> Vec<Gate> {
    let mut corrections = Vec::new();
    let n_x = layout.x_stabilizers.len();

    // ---- Correct Z errors using X-stabilizer syndrome (bits 0..n_x) ----
    let x_syndrome = &syndrome[..n_x];
    let x_triggered: Vec<usize> = x_syndrome
        .iter()
        .enumerate()
        .filter(|(_, &s)| s)
        .map(|(i, _)| i)
        .collect();

    if x_triggered.len() == 1 {
        // Single stabilizer fired: correct its first data qubit with Z.
        let data_q = layout.x_stabilizers[x_triggered[0]][0];
        corrections.push(Gate::Z(data_q));
    } else if x_triggered.len() >= 2 {
        // Multiple stabilizers fired: find the data qubit that appears in
        // the most triggered stabilizers and correct it.
        if let Some(q) = most_common_data_qubit(&layout.x_stabilizers, &x_triggered) {
            corrections.push(Gate::Z(q));
        }
    }

    // ---- Correct X errors using Z-stabilizer syndrome (bits n_x..) ----
    let z_syndrome = &syndrome[n_x..];
    let z_triggered: Vec<usize> = z_syndrome
        .iter()
        .enumerate()
        .filter(|(_, &s)| s)
        .map(|(i, _)| i)
        .collect();

    if z_triggered.len() == 1 {
        let data_q = layout.z_stabilizers[z_triggered[0]][0];
        corrections.push(Gate::X(data_q));
    } else if z_triggered.len() >= 2 {
        if let Some(q) = most_common_data_qubit(&layout.z_stabilizers, &z_triggered) {
            corrections.push(Gate::X(q));
        }
    }

    corrections
}

/// Find the data qubit that appears in the most stabilizers among the
/// triggered set. Returns `None` if the triggered list is empty.
fn most_common_data_qubit(
    stabilizers: &[Vec<QubitIndex>],
    triggered_indices: &[usize],
) -> Option<QubitIndex> {
    // Count how many triggered stabilizers each data qubit participates in.
    let mut counts: std::collections::HashMap<QubitIndex, usize> = std::collections::HashMap::new();
    for &idx in triggered_indices {
        for &dq in &stabilizers[idx] {
            *counts.entry(dq).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(qubit, _)| qubit)
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Run a surface code error correction simulation.
///
/// Currently only **distance 3** is supported. The simulation:
/// 1. Initializes all qubits in |0> (the logical |0_L> state).
/// 2. For each cycle: injects noise, extracts the syndrome, decodes, and
///    applies corrections.
/// 3. After all cycles, returns the syndrome history and error statistics.
///
/// # Logical error detection (simplified)
///
/// A logical Z error is detected by checking the parity of a representative
/// row of data qubits. If the initial logical state was |0_L>, a flipped
/// parity indicates a logical error. This is a coarse approximation; a full
/// implementation would track the Pauli frame.
///
/// # Errors
///
/// Returns a [`ruqu_core::error::QuantumError`] on simulator failures.
pub fn run_surface_code(config: &SurfaceCodeConfig) -> ruqu_core::error::Result<SurfaceCodeResult> {
    assert_eq!(
        config.distance, 3,
        "Only distance-3 surface codes are currently supported"
    );

    let layout = SurfaceCodeLayout::distance_3();
    let total_qubits = layout.total_qubits();

    let mut state = match config.seed {
        Some(s) => QuantumState::new_with_seed(total_qubits, s)?,
        None => QuantumState::new(total_qubits)?,
    };

    // Seeded RNG for noise injection.
    let mut rng = match config.seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    };

    let mut logical_errors = 0u32;
    let mut syndrome_history = Vec::with_capacity(config.num_cycles as usize);

    // Record the initial parity of the top row (d0, d1, d2) for logical
    // error detection. For |0_L>, this parity should be even (all |0>).
    // After each cycle we compare against this baseline.
    let logical_row: [QubitIndex; 3] = [0, 1, 2];

    for _cycle in 0..config.num_cycles {
        // 1. Inject noise on data qubits.
        inject_noise(&mut state, &layout.data_qubits, config.noise_rate, &mut rng)?;

        // 2. Syndrome extraction.
        let syndrome = run_cycle(&mut state, &layout)?;
        syndrome_history.push(syndrome.clone());

        // 3. Decode and apply corrections.
        let corrections = decode_syndrome(&syndrome, &layout);
        for gate in &corrections {
            state.apply_gate(gate)?;
        }

        // 4. Simplified logical error check.
        //    Measure Z-parity of the top-row data qubits non-destructively
        //    by reading expectation values. If <Z_0 Z_1 Z_2> < 0, the
        //    row has odd parity -> logical error.
        let mut row_parity = 1.0_f64;
        for &q in &logical_row {
            let z_exp = state.expectation_value(&ruqu_core::types::PauliString {
                ops: vec![(q, ruqu_core::types::PauliOp::Z)],
            });
            // Each Z expectation is in [-1, 1]. For a computational basis
            // state, it is exactly +1 (|0>) or -1 (|1>). For superpositions
            // we approximate: sign of the product captures parity.
            row_parity *= z_exp;
        }
        if row_parity < 0.0 {
            logical_errors += 1;
        }
    }

    let logical_error_rate = if config.num_cycles > 0 {
        logical_errors as f64 / config.num_cycles as f64
    } else {
        0.0
    };

    Ok(SurfaceCodeResult {
        logical_errors,
        total_cycles: config.num_cycles,
        logical_error_rate,
        syndrome_history,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_distance_3() {
        let layout = SurfaceCodeLayout::distance_3();
        assert_eq!(layout.data_qubits.len(), 9);
        assert_eq!(layout.x_ancillas.len(), 4);
        assert_eq!(layout.z_ancillas.len(), 4);
        assert_eq!(layout.total_qubits(), 17);
        assert_eq!(layout.num_stabilizers(), 8);
    }

    #[test]
    fn test_x_stabilizers_cover_all_data() {
        let layout = SurfaceCodeLayout::distance_3();
        let mut covered: std::collections::HashSet<QubitIndex> = std::collections::HashSet::new();
        for stab in &layout.x_stabilizers {
            for &q in stab {
                covered.insert(q);
            }
        }
        // All 9 data qubits should be covered by X stabilizers.
        for q in 0..9u32 {
            assert!(
                covered.contains(&q),
                "data qubit {} not covered by X stabilizers",
                q
            );
        }
    }

    #[test]
    fn test_z_stabilizers_boundary() {
        let layout = SurfaceCodeLayout::distance_3();
        // Z stabilizers are weight-2 boundary stabilizers for d=3.
        for stab in &layout.z_stabilizers {
            assert_eq!(stab.len(), 2, "Z stabilizer should have weight 2");
        }
    }

    #[test]
    fn test_decode_syndrome_no_error() {
        let layout = SurfaceCodeLayout::distance_3();
        let syndrome = vec![false; 8];
        let corrections = decode_syndrome(&syndrome, &layout);
        assert!(
            corrections.is_empty(),
            "no corrections when syndrome is trivial"
        );
    }

    #[test]
    fn test_decode_syndrome_single_x_stabilizer() {
        let layout = SurfaceCodeLayout::distance_3();
        // Only X0 fires -> correct data qubit 0 with Z.
        let mut syndrome = vec![false; 8];
        syndrome[0] = true;
        let corrections = decode_syndrome(&syndrome, &layout);
        assert_eq!(corrections.len(), 1);
    }

    #[test]
    fn test_decode_syndrome_single_z_stabilizer() {
        let layout = SurfaceCodeLayout::distance_3();
        // Only Z0 fires (index 4 in syndrome vector).
        let mut syndrome = vec![false; 8];
        syndrome[4] = true;
        let corrections = decode_syndrome(&syndrome, &layout);
        assert_eq!(corrections.len(), 1);
    }

    #[test]
    fn test_most_common_data_qubit() {
        let stabilizers = vec![vec![0, 1, 3, 4], vec![1, 2, 4, 5]];
        // Both stabilizers 0 and 1 triggered: qubit 1 and 4 appear in both.
        let result = most_common_data_qubit(&stabilizers, &[0, 1]);
        assert!(result == Some(1) || result == Some(4));
    }

    #[test]
    #[should_panic(expected = "Only distance-3")]
    fn test_unsupported_distance() {
        let config = SurfaceCodeConfig {
            distance: 5,
            num_cycles: 1,
            noise_rate: 0.01,
            seed: Some(42),
        };
        let _ = run_surface_code(&config);
    }
}
