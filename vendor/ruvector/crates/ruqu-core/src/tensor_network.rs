//! Matrix Product State (MPS) tensor network simulator.
//!
//! Represents an n-qubit quantum state as a chain of tensors:
//!   |psi> = Sum A[1]^{i1} . A[2]^{i2} . ... . A[n]^{in} |i1 i2 ... in>
//!
//! Each A[k] has shape (chi_{k-1}, 2, chi_k) where chi is the bond dimension.
//! Product states have chi=1. Entanglement increases bond dimension up to a
//! configurable maximum, beyond which truncation provides approximate simulation
//! with controlled error.

use crate::error::{QuantumError, Result};
use crate::gate::Gate;
use crate::types::{Complex, MeasurementOutcome, QubitIndex};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Configuration for the MPS simulator.
#[derive(Debug, Clone)]
pub struct MpsConfig {
    /// Maximum bond dimension. Higher values yield more accurate simulation
    /// at the cost of increased memory and computation time.
    /// Typical values: 64, 128, 256, 512, 1024.
    pub max_bond_dim: usize,
    /// Truncation threshold: singular values below this are discarded.
    pub truncation_threshold: f64,
}

impl Default for MpsConfig {
    fn default() -> Self {
        Self {
            max_bond_dim: 256,
            truncation_threshold: 1e-10,
        }
    }
}

// ---------------------------------------------------------------------------
// MPS Tensor
// ---------------------------------------------------------------------------

/// A single MPS tensor for qubit k.
///
/// Shape: (left_dim, 2, right_dim) stored as a flat `Vec<Complex>` in
/// row-major order with index = left * (2 * right_dim) + phys * right_dim + right.
#[derive(Clone)]
struct MpsTensor {
    data: Vec<Complex>,
    left_dim: usize,
    right_dim: usize,
}

impl MpsTensor {
    /// Create a tensor initialized to zero.
    fn new_zero(left_dim: usize, right_dim: usize) -> Self {
        Self {
            data: vec![Complex::ZERO; left_dim * 2 * right_dim],
            left_dim,
            right_dim,
        }
    }

    /// Compute the flat index for element (left, phys, right).
    #[inline]
    fn index(&self, left: usize, phys: usize, right: usize) -> usize {
        left * (2 * self.right_dim) + phys * self.right_dim + right
    }

    /// Read the element at (left, phys, right).
    #[inline]
    fn get(&self, left: usize, phys: usize, right: usize) -> Complex {
        self.data[self.index(left, phys, right)]
    }

    /// Write the element at (left, phys, right).
    #[inline]
    fn set(&mut self, left: usize, phys: usize, right: usize, val: Complex) {
        let idx = self.index(left, phys, right);
        self.data[idx] = val;
    }
}

// ---------------------------------------------------------------------------
// MPS State
// ---------------------------------------------------------------------------

/// Matrix Product State quantum simulator.
///
/// Represents quantum states as a chain of tensors, enabling efficient
/// simulation of circuits with bounded entanglement. Can handle hundreds
/// to thousands of qubits when bond dimension stays manageable.
pub struct MpsState {
    num_qubits: usize,
    tensors: Vec<MpsTensor>,
    config: MpsConfig,
    rng: StdRng,
    measurement_record: Vec<MeasurementOutcome>,
    /// Accumulated truncation error for confidence bounds.
    total_truncation_error: f64,
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

impl MpsState {
    /// Initialize the |00...0> product state.
    ///
    /// Each tensor has bond dimension 1 and physical dimension 2, with the
    /// amplitude concentrated on the |0> basis state.
    pub fn new(num_qubits: usize) -> Result<Self> {
        Self::new_with_config(num_qubits, MpsConfig::default())
    }

    /// Initialize |00...0> with explicit configuration.
    pub fn new_with_config(num_qubits: usize, config: MpsConfig) -> Result<Self> {
        if num_qubits == 0 {
            return Err(QuantumError::CircuitError(
                "cannot create MPS with 0 qubits".into(),
            ));
        }
        let mut tensors = Vec::with_capacity(num_qubits);
        for _ in 0..num_qubits {
            let mut t = MpsTensor::new_zero(1, 1);
            // |0> component = 1, |1> component = 0
            t.set(0, 0, 0, Complex::ONE);
            tensors.push(t);
        }
        Ok(Self {
            num_qubits,
            tensors,
            config,
            rng: StdRng::from_entropy(),
            measurement_record: Vec::new(),
            total_truncation_error: 0.0,
        })
    }

    /// Initialize |00...0> with a deterministic seed for reproducibility.
    pub fn new_with_seed(num_qubits: usize, seed: u64, config: MpsConfig) -> Result<Self> {
        let mut state = Self::new_with_config(num_qubits, config)?;
        state.rng = StdRng::seed_from_u64(seed);
        Ok(state)
    }

    // -------------------------------------------------------------------
    // Accessors
    // -------------------------------------------------------------------

    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    /// Current maximum bond dimension across all bonds in the MPS chain.
    pub fn max_bond_dimension(&self) -> usize {
        self.tensors
            .iter()
            .map(|t| t.left_dim.max(t.right_dim))
            .max()
            .unwrap_or(1)
    }

    /// Accumulated truncation error from bond-dimension truncations.
    pub fn truncation_error(&self) -> f64 {
        self.total_truncation_error
    }

    pub fn measurement_record(&self) -> &[MeasurementOutcome] {
        &self.measurement_record
    }

    // -------------------------------------------------------------------
    // Single-qubit gate
    // -------------------------------------------------------------------

    /// Apply a 2x2 unitary to a single qubit.
    ///
    /// Contracts the gate matrix with the physical index of tensor[qubit]:
    ///   new_tensor(l, i', r) = Sum_i matrix[i'][i] * tensor(l, i, r)
    ///
    /// This does not change bond dimensions.
    pub fn apply_single_qubit_gate(&mut self, qubit: usize, matrix: &[[Complex; 2]; 2]) {
        let t = &self.tensors[qubit];
        let left_dim = t.left_dim;
        let right_dim = t.right_dim;
        let mut new_t = MpsTensor::new_zero(left_dim, right_dim);

        for l in 0..left_dim {
            for r in 0..right_dim {
                let v0 = t.get(l, 0, r);
                let v1 = t.get(l, 1, r);
                new_t.set(l, 0, r, matrix[0][0] * v0 + matrix[0][1] * v1);
                new_t.set(l, 1, r, matrix[1][0] * v0 + matrix[1][1] * v1);
            }
        }
        self.tensors[qubit] = new_t;
    }

    // -------------------------------------------------------------------
    // Two-qubit gate (adjacent)
    // -------------------------------------------------------------------

    /// Apply a 4x4 unitary gate to two adjacent qubits.
    ///
    /// The algorithm:
    /// 1. Contract tensors at q1 and q2 into a combined 4-index tensor.
    /// 2. Apply the 4x4 gate matrix on the two physical indices.
    /// 3. Reshape into a matrix and perform truncated QR decomposition.
    /// 4. Split back into two MPS tensors, respecting max_bond_dim.
    pub fn apply_two_qubit_gate_adjacent(
        &mut self,
        q1: usize,
        q2: usize,
        matrix: &[[Complex; 4]; 4],
    ) -> Result<()> {
        if q1 >= self.num_qubits || q2 >= self.num_qubits {
            return Err(QuantumError::CircuitError(
                "qubit index out of range for MPS".into(),
            ));
        }
        // Ensure q1 < q2 for adjacent gate application.
        let (qa, qb) = if q1 < q2 { (q1, q2) } else { (q2, q1) };
        if qb - qa != 1 {
            return Err(QuantumError::CircuitError(
                "apply_two_qubit_gate_adjacent requires adjacent qubits".into(),
            ));
        }

        let t_a = &self.tensors[qa];
        let t_b = &self.tensors[qb];
        let left_dim = t_a.left_dim;
        let inner_dim = t_a.right_dim; // == t_b.left_dim
        let right_dim = t_b.right_dim;

        // Step 1: Contract over the shared bond index to form a 4-index tensor
        // theta(l, ia, ib, r) = Sum_m A_a(l, ia, m) * A_b(m, ib, r)
        let mut theta = vec![Complex::ZERO; left_dim * 2 * 2 * right_dim];
        let theta_idx = |l: usize, ia: usize, ib: usize, r: usize| -> usize {
            l * (4 * right_dim) + ia * (2 * right_dim) + ib * right_dim + r
        };

        for l in 0..left_dim {
            for ia in 0..2 {
                for ib in 0..2 {
                    for r in 0..right_dim {
                        let mut sum = Complex::ZERO;
                        for m in 0..inner_dim {
                            sum += t_a.get(l, ia, m) * t_b.get(m, ib, r);
                        }
                        theta[theta_idx(l, ia, ib, r)] = sum;
                    }
                }
            }
        }

        // Step 2: Apply the gate matrix on the physical indices.
        // Gate index convention: row = ia' * 2 + ib', col = ia * 2 + ib
        // If q1 > q2, the gate was specified with reversed qubit order;
        // we must transpose the physical indices accordingly.
        let swap_phys = q1 > q2;
        let mut gated = vec![Complex::ZERO; left_dim * 2 * 2 * right_dim];
        for l in 0..left_dim {
            for r in 0..right_dim {
                // Collect the 4 input values
                let mut inp = [Complex::ZERO; 4];
                for ia in 0..2 {
                    for ib in 0..2 {
                        let idx = if swap_phys { ib * 2 + ia } else { ia * 2 + ib };
                        inp[idx] = theta[theta_idx(l, ia, ib, r)];
                    }
                }
                // Apply gate
                for ia_out in 0..2 {
                    for ib_out in 0..2 {
                        let row = if swap_phys {
                            ib_out * 2 + ia_out
                        } else {
                            ia_out * 2 + ib_out
                        };
                        let mut val = Complex::ZERO;
                        for c in 0..4 {
                            val += matrix[row][c] * inp[c];
                        }
                        gated[theta_idx(l, ia_out, ib_out, r)] = val;
                    }
                }
            }
        }

        // Step 3: Reshape into matrix of shape (left_dim * 2) x (2 * right_dim)
        // and perform truncated decomposition.
        let rows = left_dim * 2;
        let cols = 2 * right_dim;
        let mut mat = vec![Complex::ZERO; rows * cols];
        for l in 0..left_dim {
            for ia in 0..2 {
                for ib in 0..2 {
                    for r in 0..right_dim {
                        let row = l * 2 + ia;
                        let col = ib * right_dim + r;
                        mat[row * cols + col] = gated[theta_idx(l, ia, ib, r)];
                    }
                }
            }
        }

        let (q_mat, r_mat, new_bond, trunc_err) = Self::truncated_qr(
            &mat,
            rows,
            cols,
            self.config.max_bond_dim,
            self.config.truncation_threshold,
        );
        self.total_truncation_error += trunc_err;

        // Step 4: Reshape Q into tensor_a (left_dim, 2, new_bond)
        //         and R into tensor_b (new_bond, 2, right_dim).
        let mut new_a = MpsTensor::new_zero(left_dim, new_bond);
        for l in 0..left_dim {
            for ia in 0..2 {
                for nb in 0..new_bond {
                    let row = l * 2 + ia;
                    new_a.set(l, ia, nb, q_mat[row * new_bond + nb]);
                }
            }
        }

        let mut new_b = MpsTensor::new_zero(new_bond, right_dim);
        for nb in 0..new_bond {
            for ib in 0..2 {
                for r in 0..right_dim {
                    let col = ib * right_dim + r;
                    new_b.set(nb, ib, r, r_mat[nb * cols + col]);
                }
            }
        }

        self.tensors[qa] = new_a;
        self.tensors[qb] = new_b;
        Ok(())
    }

    // -------------------------------------------------------------------
    // Two-qubit gate (general, possibly non-adjacent)
    // -------------------------------------------------------------------

    /// Apply a 4x4 gate to any pair of qubits.
    ///
    /// If the qubits are adjacent, delegates directly. Otherwise, uses SWAP
    /// gates to move the qubits next to each other, applies the gate, then
    /// swaps back to restore qubit ordering.
    pub fn apply_two_qubit_gate(
        &mut self,
        q1: usize,
        q2: usize,
        matrix: &[[Complex; 4]; 4],
    ) -> Result<()> {
        if q1 == q2 {
            return Err(QuantumError::CircuitError(
                "two-qubit gate requires distinct qubits".into(),
            ));
        }
        let diff = if q1 > q2 { q1 - q2 } else { q2 - q1 };
        if diff == 1 {
            return self.apply_two_qubit_gate_adjacent(q1, q2, matrix);
        }

        let swap_matrix = Self::swap_matrix();

        // Move q1 adjacent to q2 via SWAP chain.
        // We swap q1 toward q2, keeping track of its current position.
        let (mut pos1, target_pos) = if q1 < q2 { (q1, q2 - 1) } else { (q1, q2 + 1) };

        // Forward swaps: move pos1 toward target_pos
        let forward_steps: Vec<usize> = if pos1 < target_pos {
            (pos1..target_pos).collect()
        } else {
            (target_pos..pos1).rev().collect()
        };

        for &s in &forward_steps {
            self.apply_two_qubit_gate_adjacent(s, s + 1, &swap_matrix)?;
        }
        pos1 = target_pos;

        // Now pos1 and q2 are adjacent: apply the gate.
        self.apply_two_qubit_gate_adjacent(pos1, q2, matrix)?;

        // Reverse swaps to restore original qubit ordering.
        for &s in forward_steps.iter().rev() {
            self.apply_two_qubit_gate_adjacent(s, s + 1, &swap_matrix)?;
        }

        Ok(())
    }

    // -------------------------------------------------------------------
    // Measurement
    // -------------------------------------------------------------------

    /// Measure a single qubit projectively.
    ///
    /// 1. Compute the probability of |0> by locally contracting the MPS.
    /// 2. Sample the outcome.
    /// 3. Collapse the tensor at the measured qubit by projecting.
    /// 4. Renormalize.
    pub fn measure(&mut self, qubit: usize) -> Result<MeasurementOutcome> {
        if qubit >= self.num_qubits {
            return Err(QuantumError::InvalidQubitIndex {
                index: qubit as QubitIndex,
                num_qubits: self.num_qubits as u32,
            });
        }

        // Compute reduced density matrix element rho_00 and rho_11
        // for the target qubit by contracting the MPS from both ends.
        let (p0, p1) = self.qubit_probabilities(qubit);
        let total = p0 + p1;
        let p0_norm = if total > 0.0 { p0 / total } else { 0.5 };

        let random: f64 = self.rng.gen();
        let result = random >= p0_norm; // true => measured |1>
        let prob = if result { 1.0 - p0_norm } else { p0_norm };

        // Collapse: project the tensor at this qubit onto the measured state.
        let t = &self.tensors[qubit];
        let left_dim = t.left_dim;
        let right_dim = t.right_dim;
        let measured_phys: usize = if result { 1 } else { 0 };

        let mut new_t = MpsTensor::new_zero(left_dim, right_dim);
        for l in 0..left_dim {
            for r in 0..right_dim {
                new_t.set(l, measured_phys, r, t.get(l, measured_phys, r));
            }
        }

        // Renormalize the projected tensor.
        let mut norm_sq = 0.0;
        for val in &new_t.data {
            norm_sq += val.norm_sq();
        }
        if norm_sq > 0.0 {
            let inv_norm = 1.0 / norm_sq.sqrt();
            for val in new_t.data.iter_mut() {
                *val = *val * inv_norm;
            }
        }

        self.tensors[qubit] = new_t;

        let outcome = MeasurementOutcome {
            qubit: qubit as QubitIndex,
            result,
            probability: prob,
        };
        self.measurement_record.push(outcome.clone());
        Ok(outcome)
    }

    // -------------------------------------------------------------------
    // Gate dispatch
    // -------------------------------------------------------------------

    /// Apply a gate from the Gate enum, returning any measurement outcomes.
    pub fn apply_gate(&mut self, gate: &Gate) -> Result<Vec<MeasurementOutcome>> {
        for &q in gate.qubits().iter() {
            if (q as usize) >= self.num_qubits {
                return Err(QuantumError::InvalidQubitIndex {
                    index: q,
                    num_qubits: self.num_qubits as u32,
                });
            }
        }

        match gate {
            Gate::Barrier => Ok(vec![]),

            Gate::Measure(q) => {
                let outcome = self.measure(*q as usize)?;
                Ok(vec![outcome])
            }

            Gate::Reset(q) => {
                let outcome = self.measure(*q as usize)?;
                if outcome.result {
                    let x = Gate::X(*q).matrix_1q().unwrap();
                    self.apply_single_qubit_gate(*q as usize, &x);
                }
                Ok(vec![])
            }

            Gate::CNOT(q1, q2) | Gate::CZ(q1, q2) | Gate::SWAP(q1, q2) | Gate::Rzz(q1, q2, _) => {
                if q1 == q2 {
                    return Err(QuantumError::CircuitError(format!(
                        "two-qubit gate requires distinct qubits, got {} and {}",
                        q1, q2
                    )));
                }
                let matrix = gate.matrix_2q().unwrap();
                self.apply_two_qubit_gate(*q1 as usize, *q2 as usize, &matrix)?;
                Ok(vec![])
            }

            other => {
                if let Some(matrix) = other.matrix_1q() {
                    let q = other.qubits()[0];
                    self.apply_single_qubit_gate(q as usize, &matrix);
                    Ok(vec![])
                } else {
                    Err(QuantumError::CircuitError(format!(
                        "unsupported gate for MPS: {:?}",
                        other
                    )))
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Internal: SWAP matrix
    // -------------------------------------------------------------------

    fn swap_matrix() -> [[Complex; 4]; 4] {
        let c0 = Complex::ZERO;
        let c1 = Complex::ONE;
        [
            [c1, c0, c0, c0],
            [c0, c0, c1, c0],
            [c0, c1, c0, c0],
            [c0, c0, c0, c1],
        ]
    }

    // -------------------------------------------------------------------
    // Internal: qubit probability computation
    // -------------------------------------------------------------------

    /// Compute (prob_0, prob_1) for a single qubit by contracting the MPS.
    ///
    /// This builds a partial "environment" from the left and right boundaries,
    /// then contracts through the target qubit tensor for each physical index.
    fn qubit_probabilities(&self, qubit: usize) -> (f64, f64) {
        // Left environment: contract tensors 0..qubit into a matrix.
        // env_left has shape (bond_dim, bond_dim) representing
        // Sum_{physical indices} conj(A) * A contracted from the left.
        let bond_left = self.tensors[qubit].left_dim;
        let mut env_left = vec![Complex::ZERO; bond_left * bond_left];
        // Initialize to identity (boundary condition: left boundary = 1).
        for i in 0..bond_left {
            env_left[i * bond_left + i] = Complex::ONE;
        }
        // Contract from site 0 to qubit-1.
        for site in 0..qubit {
            let t = &self.tensors[site];
            let dim_in = t.left_dim;
            let dim_out = t.right_dim;
            let mut new_env = vec![Complex::ZERO; dim_out * dim_out];
            for ro in 0..dim_out {
                for co in 0..dim_out {
                    let mut sum = Complex::ZERO;
                    for ri in 0..dim_in {
                        for ci in 0..dim_in {
                            let e = env_left[ri * dim_in + ci];
                            if e.norm_sq() == 0.0 {
                                continue;
                            }
                            for p in 0..2 {
                                sum += e.conj() // env^*
                                    * t.get(ri, p, ro).conj()
                                    * t.get(ci, p, co);
                            }
                        }
                    }
                    new_env[ro * dim_out + co] = sum;
                }
            }
            env_left = new_env;
        }

        // Right environment: contract tensors (qubit+1)..num_qubits.
        let bond_right = self.tensors[qubit].right_dim;
        let mut env_right = vec![Complex::ZERO; bond_right * bond_right];
        for i in 0..bond_right {
            env_right[i * bond_right + i] = Complex::ONE;
        }
        for site in (qubit + 1..self.num_qubits).rev() {
            let t = &self.tensors[site];
            let dim_in = t.right_dim;
            let dim_out = t.left_dim;
            let mut new_env = vec![Complex::ZERO; dim_out * dim_out];
            for ro in 0..dim_out {
                for co in 0..dim_out {
                    let mut sum = Complex::ZERO;
                    for ri in 0..dim_in {
                        for ci in 0..dim_in {
                            let e = env_right[ri * dim_in + ci];
                            if e.norm_sq() == 0.0 {
                                continue;
                            }
                            for p in 0..2 {
                                sum += e.conj() * t.get(ro, p, ri).conj() * t.get(co, p, ci);
                            }
                        }
                    }
                    new_env[ro * dim_out + co] = sum;
                }
            }
            env_right = new_env;
        }

        // Contract with the target qubit tensor for each physical index.
        let t = &self.tensors[qubit];
        let mut probs = [0.0f64; 2];
        for phys in 0..2 {
            let mut val = Complex::ZERO;
            for l1 in 0..t.left_dim {
                for l2 in 0..t.left_dim {
                    let e_l = env_left[l1 * t.left_dim + l2];
                    if e_l.norm_sq() == 0.0 {
                        continue;
                    }
                    for r1 in 0..t.right_dim {
                        for r2 in 0..t.right_dim {
                            let e_r = env_right[r1 * t.right_dim + r2];
                            if e_r.norm_sq() == 0.0 {
                                continue;
                            }
                            val +=
                                e_l.conj() * t.get(l1, phys, r1).conj() * t.get(l2, phys, r2) * e_r;
                        }
                    }
                }
            }
            probs[phys] = val.re; // Should be real for a valid density matrix
        }

        (probs[0].max(0.0), probs[1].max(0.0))
    }

    // -------------------------------------------------------------------
    // Internal: Truncated QR decomposition
    // -------------------------------------------------------------------

    /// Perform modified Gram-Schmidt QR on a complex matrix, then truncate.
    ///
    /// Given matrix M of shape (rows x cols), computes M = Q * R where Q has
    /// orthonormal columns and R is upper triangular. Truncates to at most
    /// `max_rank` columns of Q (and rows of R), discarding columns whose
    /// R diagonal magnitude falls below `threshold`.
    ///
    /// Returns (Q_flat, R_flat, rank, truncation_error).
    fn truncated_qr(
        mat: &[Complex],
        rows: usize,
        cols: usize,
        max_rank: usize,
        threshold: f64,
    ) -> (Vec<Complex>, Vec<Complex>, usize, f64) {
        let rank_bound = rows.min(cols).min(max_rank);

        // Modified Gram-Schmidt: build Q column by column, R simultaneously.
        let mut q_cols: Vec<Vec<Complex>> = Vec::with_capacity(rank_bound);
        let mut r_data = vec![Complex::ZERO; rank_bound * cols];
        let mut actual_rank = 0;
        let mut trunc_error = 0.0;

        for j in 0..cols.min(rank_bound + cols) {
            if actual_rank >= rank_bound {
                // Estimate truncation error from remaining columns.
                if j < cols {
                    for jj in j..cols {
                        let mut col_norm_sq = 0.0;
                        for i in 0..rows {
                            col_norm_sq += mat[i * cols + jj].norm_sq();
                        }
                        trunc_error += col_norm_sq;
                    }
                    trunc_error = trunc_error.sqrt();
                }
                break;
            }
            if j >= cols {
                break;
            }

            // Extract column j of the input matrix.
            let mut v: Vec<Complex> = (0..rows).map(|i| mat[i * cols + j]).collect();

            // Orthogonalize against existing Q columns.
            for k in 0..actual_rank {
                let mut dot = Complex::ZERO;
                for i in 0..rows {
                    dot += q_cols[k][i].conj() * v[i];
                }
                r_data[k * cols + j] = dot;
                for i in 0..rows {
                    v[i] = v[i] - dot * q_cols[k][i];
                }
            }

            // Compute norm of residual.
            let mut norm_sq = 0.0;
            for i in 0..rows {
                norm_sq += v[i].norm_sq();
            }
            let norm = norm_sq.sqrt();

            if norm < threshold {
                // Column is (nearly) linearly dependent; skip it.
                trunc_error += norm;
                continue;
            }

            // Normalize and store.
            r_data[actual_rank * cols + j] = Complex::new(norm, 0.0);
            let inv_norm = 1.0 / norm;
            for i in 0..rows {
                v[i] = v[i] * inv_norm;
            }
            q_cols.push(v);
            actual_rank += 1;
        }

        // Ensure at least rank 1 to avoid degenerate tensors.
        if actual_rank == 0 {
            actual_rank = 1;
            q_cols.push(vec![Complex::ZERO; rows]);
            q_cols[0][0] = Complex::ONE;
            // R remains zero.
        }

        // Flatten Q: shape (rows, actual_rank)
        let mut q_flat = vec![Complex::ZERO; rows * actual_rank];
        for i in 0..rows {
            for k in 0..actual_rank {
                q_flat[i * actual_rank + k] = q_cols[k][i];
            }
        }

        // Trim R to shape (actual_rank, cols)
        let mut r_flat = vec![Complex::ZERO; actual_rank * cols];
        for k in 0..actual_rank {
            for j in 0..cols {
                r_flat[k * cols + j] = r_data[k * cols + j];
            }
        }

        (q_flat, r_flat, actual_rank, trunc_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_product_state() {
        let mps = MpsState::new(4).unwrap();
        assert_eq!(mps.num_qubits(), 4);
        assert_eq!(mps.max_bond_dimension(), 1);
        assert_eq!(mps.truncation_error(), 0.0);
    }

    #[test]
    fn test_zero_qubits_errors() {
        assert!(MpsState::new(0).is_err());
    }

    #[test]
    fn test_single_qubit_x_gate() {
        let mut mps = MpsState::new_with_seed(1, 42, MpsConfig::default()).unwrap();
        // X gate: flips |0> to |1>
        let x = [[Complex::ZERO, Complex::ONE], [Complex::ONE, Complex::ZERO]];
        mps.apply_single_qubit_gate(0, &x);
        // After X, tensor should have |1> = 1, |0> = 0
        let t = &mps.tensors[0];
        assert!(t.get(0, 0, 0).norm_sq() < 1e-20);
        assert!((t.get(0, 1, 0).norm_sq() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_single_qubit_h_gate() {
        let mut mps = MpsState::new_with_seed(1, 42, MpsConfig::default()).unwrap();
        let h = std::f64::consts::FRAC_1_SQRT_2;
        let hc = Complex::new(h, 0.0);
        let h_gate = [[hc, hc], [hc, -hc]];
        mps.apply_single_qubit_gate(0, &h_gate);
        // After H|0>, both amplitudes should be 1/sqrt(2)
        let t = &mps.tensors[0];
        assert!((t.get(0, 0, 0).norm_sq() - 0.5).abs() < 1e-10);
        assert!((t.get(0, 1, 0).norm_sq() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_cnot_creates_bell_state() {
        let mut mps = MpsState::new_with_seed(2, 42, MpsConfig::default()).unwrap();
        // Apply H to qubit 0
        let h = std::f64::consts::FRAC_1_SQRT_2;
        let hc = Complex::new(h, 0.0);
        let h_gate = [[hc, hc], [hc, -hc]];
        mps.apply_single_qubit_gate(0, &h_gate);

        // Apply CNOT(0,1)
        let c0 = Complex::ZERO;
        let c1 = Complex::ONE;
        let cnot = [
            [c1, c0, c0, c0],
            [c0, c1, c0, c0],
            [c0, c0, c0, c1],
            [c0, c0, c1, c0],
        ];
        mps.apply_two_qubit_gate(0, 1, &cnot).unwrap();
        // Bond dimension should have increased from 1 to 2
        assert!(mps.max_bond_dimension() >= 2);
    }

    #[test]
    fn test_measurement_deterministic() {
        // |0> state: measuring should always give 0
        let mut mps = MpsState::new_with_seed(1, 42, MpsConfig::default()).unwrap();
        let outcome = mps.measure(0).unwrap();
        assert!(!outcome.result);
        assert!((outcome.probability - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_gate_dispatch() {
        let mut mps = MpsState::new_with_seed(2, 42, MpsConfig::default()).unwrap();
        let outcomes = mps.apply_gate(&Gate::H(0)).unwrap();
        assert!(outcomes.is_empty());
        let outcomes = mps.apply_gate(&Gate::CNOT(0, 1)).unwrap();
        assert!(outcomes.is_empty());
    }

    #[test]
    fn test_non_adjacent_two_qubit_gate() {
        let mut mps = MpsState::new_with_seed(4, 42, MpsConfig::default()).unwrap();
        // Apply CNOT between qubits 0 and 3 (non-adjacent)
        let c0 = Complex::ZERO;
        let c1 = Complex::ONE;
        let cnot = [
            [c1, c0, c0, c0],
            [c0, c1, c0, c0],
            [c0, c0, c0, c1],
            [c0, c0, c1, c0],
        ];
        // Should not error even though qubits are non-adjacent
        mps.apply_two_qubit_gate(0, 3, &cnot).unwrap();
    }
}
