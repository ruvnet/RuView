//! Density Matrix Representation
//!
//! Mixed quantum states represented as density matrices (positive semidefinite,
//! trace-one Hermitian operators).

use super::complex_matrix::{Complex64, ComplexMatrix, ComplexVector};
use super::quantum_state::QuantumState;
use super::{constants, QuantumTopologyError, Result};

/// Mixed quantum state representation
#[derive(Debug, Clone)]
pub struct MixedState {
    /// Ensemble of pure states with probabilities
    pub states: Vec<(f64, QuantumState)>,
}

impl MixedState {
    /// Create a mixed state from an ensemble
    pub fn new(states: Vec<(f64, QuantumState)>) -> Result<Self> {
        let total_prob: f64 = states.iter().map(|(p, _)| p).sum();
        if (total_prob - 1.0).abs() > constants::EPSILON {
            return Err(QuantumTopologyError::InvalidDensityMatrix(
                format!("Probabilities sum to {} instead of 1", total_prob),
            ));
        }

        Ok(Self { states })
    }

    /// Create a pure state (single state with probability 1)
    pub fn pure(state: QuantumState) -> Self {
        Self {
            states: vec![(1.0, state)],
        }
    }

    /// Create maximally mixed state (I/d)
    pub fn maximally_mixed(dimension: usize) -> Self {
        let prob = 1.0 / dimension as f64;
        let states: Vec<(f64, QuantumState)> = (0..dimension)
            .map(|i| (prob, QuantumState::basis_state(dimension, i).unwrap()))
            .collect();
        Self { states }
    }

    /// Convert to density matrix
    pub fn to_density_matrix(&self) -> DensityMatrix {
        if self.states.is_empty() {
            return DensityMatrix::zeros(1);
        }

        let dim = self.states[0].1.dimension;
        let mut matrix = ComplexMatrix::zeros(dim, dim);

        for (prob, state) in &self.states {
            let outer = state.to_vector().outer(&state.to_vector());
            matrix = matrix.add(&outer.scale(Complex64::new(*prob, 0.0)));
        }

        DensityMatrix { matrix }
    }

    /// Check if this is a pure state
    pub fn is_pure(&self) -> bool {
        self.states.len() == 1 && (self.states[0].0 - 1.0).abs() < constants::EPSILON
    }
}

/// Density matrix representation of a quantum state
#[derive(Debug, Clone)]
pub struct DensityMatrix {
    /// The density matrix ρ
    pub matrix: ComplexMatrix,
}

impl DensityMatrix {
    /// Create a new density matrix, validating it's a valid quantum state
    pub fn new(matrix: ComplexMatrix) -> Result<Self> {
        // Check square
        if !matrix.is_square() {
            return Err(QuantumTopologyError::InvalidDensityMatrix(
                "Matrix must be square".to_string(),
            ));
        }

        // Check Hermitian
        if !matrix.is_hermitian(constants::EPSILON) {
            return Err(QuantumTopologyError::InvalidDensityMatrix(
                "Matrix must be Hermitian".to_string(),
            ));
        }

        // Check trace = 1
        let trace = matrix.trace();
        if (trace.re - 1.0).abs() > constants::EPSILON || trace.im.abs() > constants::EPSILON {
            return Err(QuantumTopologyError::InvalidDensityMatrix(
                format!("Trace must be 1, got {}", trace),
            ));
        }

        Ok(Self { matrix })
    }

    /// Create without validation (for internal use)
    pub fn new_unchecked(matrix: ComplexMatrix) -> Self {
        Self { matrix }
    }

    /// Create a zero density matrix
    pub fn zeros(dimension: usize) -> Self {
        Self {
            matrix: ComplexMatrix::zeros(dimension, dimension),
        }
    }

    /// Create a pure state density matrix |ψ⟩⟨ψ|
    pub fn from_pure_state(state: &QuantumState) -> Self {
        Self {
            matrix: state.to_density_matrix(),
        }
    }

    /// Create a pure state density matrix from a vector
    pub fn from_vector(v: &ComplexVector) -> Self {
        Self {
            matrix: v.outer(v),
        }
    }

    /// Create the maximally mixed state I/d
    pub fn maximally_mixed(dimension: usize) -> Self {
        let mut matrix = ComplexMatrix::identity(dimension);
        let scale = Complex64::new(1.0 / dimension as f64, 0.0);
        matrix = matrix.scale(scale);
        Self { matrix }
    }

    /// Create a thermal (Gibbs) state ρ = exp(-βH) / Z
    pub fn thermal_state(hamiltonian: &ComplexMatrix, beta: f64) -> Result<Self> {
        if !hamiltonian.is_square() {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: hamiltonian.rows,
                got: hamiltonian.cols,
            });
        }

        // For diagonal Hamiltonians, this is straightforward
        // For general case, we need matrix exponential
        let dim = hamiltonian.rows;
        let eigenvalues = hamiltonian.eigenvalues(100, 1e-10);

        // Compute partition function Z = Σ exp(-β Eᵢ)
        let partition: f64 = eigenvalues.iter().map(|ev| (-beta * ev.re).exp()).sum();

        // Create thermal state (diagonal approximation)
        let mut matrix = ComplexMatrix::zeros(dim, dim);
        for (i, ev) in eigenvalues.iter().enumerate().take(dim) {
            let prob = (-beta * ev.re).exp() / partition;
            matrix.set(i, i, Complex64::new(prob, 0.0));
        }

        Ok(Self { matrix })
    }

    /// Dimension of the Hilbert space
    pub fn dimension(&self) -> usize {
        self.matrix.rows
    }

    /// Compute the trace
    pub fn trace(&self) -> Complex64 {
        self.matrix.trace()
    }

    /// Compute the purity Tr(ρ²)
    pub fn purity(&self) -> f64 {
        self.matrix.matmul(&self.matrix).trace().re
    }

    /// Check if this is a pure state (purity ≈ 1)
    pub fn is_pure(&self, tolerance: f64) -> bool {
        (self.purity() - 1.0).abs() < tolerance
    }

    /// Compute the von Neumann entropy S(ρ) = -Tr(ρ log ρ)
    pub fn von_neumann_entropy(&self) -> f64 {
        let eigenvalues = self.matrix.eigenvalues(100, 1e-10);

        let mut entropy = 0.0;
        for ev in eigenvalues {
            let lambda = ev.re.max(0.0); // Eigenvalues should be non-negative
            if lambda > constants::EPSILON {
                entropy -= lambda * lambda.ln();
            }
        }

        entropy
    }

    /// Compute the linear entropy S_L(ρ) = 1 - Tr(ρ²)
    pub fn linear_entropy(&self) -> f64 {
        1.0 - self.purity()
    }

    /// Expectation value ⟨A⟩ = Tr(ρA)
    pub fn expectation(&self, observable: &ComplexMatrix) -> Result<Complex64> {
        if observable.rows != self.dimension() || observable.cols != self.dimension() {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: self.dimension(),
                got: observable.rows,
            });
        }

        Ok(self.matrix.matmul(observable).trace())
    }

    /// Apply a unitary transformation: ρ → U ρ U†
    pub fn apply_unitary(&self, unitary: &ComplexMatrix) -> Result<Self> {
        if unitary.rows != self.dimension() {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: self.dimension(),
                got: unitary.rows,
            });
        }

        let u_rho = unitary.matmul(&self.matrix);
        let result = u_rho.matmul(&unitary.adjoint());

        Ok(Self { matrix: result })
    }

    /// Partial trace over subsystem B (ρ_AB → ρ_A)
    pub fn partial_trace_b(&self, dim_a: usize, dim_b: usize) -> Result<Self> {
        if dim_a * dim_b != self.dimension() {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: dim_a * dim_b,
                got: self.dimension(),
            });
        }

        Ok(Self {
            matrix: self.matrix.partial_trace_b(dim_a, dim_b),
        })
    }

    /// Partial trace over subsystem A (ρ_AB → ρ_B)
    pub fn partial_trace_a(&self, dim_a: usize, dim_b: usize) -> Result<Self> {
        if dim_a * dim_b != self.dimension() {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: dim_a * dim_b,
                got: self.dimension(),
            });
        }

        Ok(Self {
            matrix: self.matrix.partial_trace_a(dim_a, dim_b),
        })
    }

    /// Compute quantum fidelity F(ρ, σ) = (Tr√(√ρ σ √ρ))²
    /// For classical simulation, we use a simplified formula
    pub fn fidelity(&self, other: &DensityMatrix) -> Result<f64> {
        if self.dimension() != other.dimension() {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: self.dimension(),
                got: other.dimension(),
            });
        }

        // For pure states: F(|ψ⟩⟨ψ|, |φ⟩⟨φ|) = |⟨ψ|φ⟩|²
        // For general states, use F = Tr(ρσ) + 2√(det(ρ)det(σ)) for 2x2
        // For larger dimensions, approximate with Tr(ρσ)

        let dim = self.dimension();
        if dim == 2 {
            // Closed form for 2x2
            let trace_product = self.matrix.matmul(&other.matrix).trace().re;
            let det_self = self.determinant_2x2();
            let det_other = other.determinant_2x2();

            let fidelity = trace_product + 2.0 * (det_self * det_other).sqrt();
            Ok(fidelity.max(0.0).min(1.0))
        } else {
            // Approximate with Tr(ρσ) for larger dimensions
            // This is the Hilbert-Schmidt fidelity
            let trace_product = self.matrix.matmul(&other.matrix).trace().re;
            Ok(trace_product.max(0.0).min(1.0))
        }
    }

    /// Compute 2x2 determinant
    fn determinant_2x2(&self) -> f64 {
        if self.dimension() != 2 {
            return 0.0;
        }
        let a = self.matrix.get(0, 0);
        let b = self.matrix.get(0, 1);
        let c = self.matrix.get(1, 0);
        let d = self.matrix.get(1, 1);

        (a * d - b * c).re
    }

    /// Compute trace distance D(ρ, σ) = (1/2)||ρ - σ||₁
    pub fn trace_distance(&self, other: &DensityMatrix) -> Result<f64> {
        if self.dimension() != other.dimension() {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: self.dimension(),
                got: other.dimension(),
            });
        }

        let diff = self.matrix.sub(&other.matrix);

        // Compute eigenvalues of difference
        let eigenvalues = diff.eigenvalues(100, 1e-10);

        // Trace norm is sum of absolute values of eigenvalues
        let trace_norm: f64 = eigenvalues.iter().map(|ev| ev.norm()).sum();

        Ok(trace_norm / 2.0)
    }

    /// Compute relative entropy S(ρ||σ) = Tr(ρ(log ρ - log σ))
    /// Only defined when supp(ρ) ⊆ supp(σ)
    pub fn relative_entropy(&self, other: &DensityMatrix) -> Result<f64> {
        if self.dimension() != other.dimension() {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: self.dimension(),
                got: other.dimension(),
            });
        }

        // For diagonal matrices (classical distributions)
        // D(ρ||σ) = Σᵢ ρᵢᵢ (log ρᵢᵢ - log σᵢᵢ)

        let mut rel_entropy = 0.0;
        for i in 0..self.dimension() {
            let rho_ii = self.matrix.get(i, i).re;
            let sigma_ii = other.matrix.get(i, i).re;

            if rho_ii > constants::EPSILON {
                if sigma_ii < constants::EPSILON {
                    // Infinite relative entropy
                    return Ok(f64::INFINITY);
                }
                rel_entropy += rho_ii * (rho_ii.ln() - sigma_ii.ln());
            }
        }

        Ok(rel_entropy)
    }

    /// Tensor product ρ ⊗ σ
    pub fn tensor(&self, other: &DensityMatrix) -> Self {
        Self {
            matrix: self.matrix.tensor(&other.matrix),
        }
    }

    /// Compute the Bloch vector for a single qubit (2x2 density matrix)
    pub fn bloch_vector(&self) -> Result<[f64; 3]> {
        if self.dimension() != 2 {
            return Err(QuantumTopologyError::InvalidDensityMatrix(
                "Bloch vector only defined for qubits".to_string(),
            ));
        }

        // ρ = (I + r·σ)/2
        // r_x = 2 Re(ρ₀₁), r_y = 2 Im(ρ₀₁), r_z = ρ₀₀ - ρ₁₁
        let rho_01 = self.matrix.get(0, 1);
        let rx = 2.0 * rho_01.re;
        let ry = 2.0 * rho_01.im;
        let rz = self.matrix.get(0, 0).re - self.matrix.get(1, 1).re;

        Ok([rx, ry, rz])
    }

    /// Create a qubit density matrix from Bloch vector
    pub fn from_bloch_vector(r: [f64; 3]) -> Result<Self> {
        let [rx, ry, rz] = r;

        // Check |r| ≤ 1
        let norm = (rx * rx + ry * ry + rz * rz).sqrt();
        if norm > 1.0 + constants::EPSILON {
            return Err(QuantumTopologyError::InvalidDensityMatrix(
                "Bloch vector magnitude must be ≤ 1".to_string(),
            ));
        }

        // ρ = (I + r·σ)/2 = [[1+rz, rx-iry], [rx+iry, 1-rz]]/2
        let matrix = ComplexMatrix::new(
            vec![
                Complex64::new((1.0 + rz) / 2.0, 0.0),
                Complex64::new(rx / 2.0, -ry / 2.0),
                Complex64::new(rx / 2.0, ry / 2.0),
                Complex64::new((1.0 - rz) / 2.0, 0.0),
            ],
            2,
            2,
        );

        Ok(Self { matrix })
    }

    /// Add two density matrices (for mixtures)
    /// Note: result may not be normalized
    pub fn add(&self, other: &DensityMatrix) -> Result<Self> {
        if self.dimension() != other.dimension() {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: self.dimension(),
                got: other.dimension(),
            });
        }

        Ok(Self {
            matrix: self.matrix.add(&other.matrix),
        })
    }

    /// Scale the density matrix
    pub fn scale(&self, factor: f64) -> Self {
        Self {
            matrix: self.matrix.scale(Complex64::new(factor, 0.0)),
        }
    }

    /// Compute the spectral decomposition once, caching eigenvalues for reuse.
    ///
    /// This is the recommended way to compute multiple spectral quantities
    /// (entropy, purity, effective rank) from a single eigenvalue computation.
    /// Each call to `von_neumann_entropy()` or `purity()` separately costs O(n³);
    /// using this method computes everything in a single O(n³) pass.
    pub fn spectral_decomposition(&self) -> SpectralDecomposition {
        let eigenvalues: Vec<f64> = self
            .matrix
            .eigenvalues(constants::MAX_ITERATIONS, constants::DEFAULT_TOLERANCE)
            .into_iter()
            .map(|ev| ev.re.max(0.0)) // Clamp to non-negative (numerical safety)
            .collect();

        // Entropy: S = -Σ λ_i ln(λ_i)
        let entropy = eigenvalues
            .iter()
            .filter(|&&lambda| lambda > constants::EPSILON)
            .map(|&lambda| -lambda * lambda.ln())
            .sum();

        // Purity: Tr(ρ²) = Σ λ_i²
        let purity = eigenvalues.iter().map(|&lambda| lambda * lambda).sum();

        // Effective rank
        let effective_rank = eigenvalues
            .iter()
            .filter(|&&lambda| lambda > constants::EPSILON)
            .count();

        SpectralDecomposition {
            eigenvalues,
            entropy,
            purity,
            effective_rank,
        }
    }

    /// Compute purity using Frobenius norm: Tr(ρ²) = ||ρ||²_F for Hermitian ρ.
    ///
    /// This is O(n²) compared to the O(n³) matmul-based approach.
    pub fn purity_fast(&self) -> f64 {
        self.matrix.data.iter().map(|c| c.norm_sqr()).sum()
    }

    /// Compute von Neumann entropy from pre-computed eigenvalues.
    pub fn entropy_from_eigenvalues(eigenvalues: &[f64]) -> f64 {
        eigenvalues
            .iter()
            .filter(|&&lambda| lambda > constants::EPSILON)
            .map(|&lambda| -lambda * lambda.ln())
            .sum()
    }

    /// Compute purity from pre-computed eigenvalues: Tr(ρ²) = Σ λ_i².
    pub fn purity_from_eigenvalues(eigenvalues: &[f64]) -> f64 {
        eigenvalues.iter().map(|&lambda| lambda * lambda).sum()
    }
}

/// Pre-computed spectral decomposition of a density matrix.
///
/// Computing eigenvalues is O(n³). This struct caches the result so that
/// entropy, purity, trace distance, and other spectral quantities can be
/// derived in O(n) from the same decomposition.
#[derive(Debug, Clone)]
pub struct SpectralDecomposition {
    /// Real eigenvalues (density matrices are Hermitian → real eigenvalues)
    pub eigenvalues: Vec<f64>,
    /// Von Neumann entropy: S(ρ) = -Σ λ_i ln(λ_i)
    pub entropy: f64,
    /// Purity: Tr(ρ²) = Σ λ_i²
    pub purity: f64,
    /// Effective rank (number of eigenvalues above EPSILON)
    pub effective_rank: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_state_density_matrix() {
        let state = QuantumState::ground_state(1);
        let rho = DensityMatrix::from_pure_state(&state);

        // Pure state has purity 1
        assert!((rho.purity() - 1.0).abs() < 1e-10);

        // Pure state has zero entropy
        assert!(rho.von_neumann_entropy().abs() < 1e-10);
    }

    #[test]
    fn test_maximally_mixed_state() {
        let rho = DensityMatrix::maximally_mixed(2);

        // Trace = 1
        assert!((rho.trace().re - 1.0).abs() < 1e-10);

        // Purity = 1/d for maximally mixed
        assert!((rho.purity() - 0.5).abs() < 1e-10);

        // Entropy = log(d) for maximally mixed
        let expected_entropy = 2.0_f64.ln();
        assert!((rho.von_neumann_entropy() - expected_entropy).abs() < 1e-5);
    }

    #[test]
    fn test_fidelity() {
        let rho = DensityMatrix::from_pure_state(&QuantumState::ground_state(1));
        let sigma = DensityMatrix::maximally_mixed(2);

        // Fidelity with itself is 1
        let self_fidelity = rho.fidelity(&rho).unwrap();
        assert!((self_fidelity - 1.0).abs() < 1e-10);

        // Fidelity between |0⟩ and maximally mixed
        let fid = rho.fidelity(&sigma).unwrap();
        assert!(fid > 0.0 && fid < 1.0);
    }

    #[test]
    fn test_trace_distance() {
        let rho = DensityMatrix::from_pure_state(&QuantumState::basis_state(2, 0).unwrap());
        let sigma = DensityMatrix::from_pure_state(&QuantumState::basis_state(2, 1).unwrap());

        // Orthogonal pure states have trace distance 1
        let dist = rho.trace_distance(&sigma).unwrap();
        assert!((dist - 1.0).abs() < 1e-5);

        // Same state has trace distance 0
        let self_dist = rho.trace_distance(&rho).unwrap();
        assert!(self_dist < 1e-10);
    }

    #[test]
    fn test_bloch_vector() {
        // |0⟩ state has Bloch vector (0, 0, 1)
        let rho_0 = DensityMatrix::from_pure_state(&QuantumState::basis_state(2, 0).unwrap());
        let bloch = rho_0.bloch_vector().unwrap();
        assert!((bloch[2] - 1.0).abs() < 1e-10);

        // |1⟩ state has Bloch vector (0, 0, -1)
        let rho_1 = DensityMatrix::from_pure_state(&QuantumState::basis_state(2, 1).unwrap());
        let bloch = rho_1.bloch_vector().unwrap();
        assert!((bloch[2] + 1.0).abs() < 1e-10);

        // Maximally mixed has Bloch vector (0, 0, 0)
        let rho_mm = DensityMatrix::maximally_mixed(2);
        let bloch = rho_mm.bloch_vector().unwrap();
        assert!(bloch.iter().all(|x| x.abs() < 1e-10));
    }

    #[test]
    fn test_partial_trace() {
        // Create a product state |00⟩
        let state_00 = QuantumState::basis_state(4, 0).unwrap();
        let rho_ab = DensityMatrix::from_pure_state(&state_00);

        // Partial trace over B should give |0⟩⟨0|
        let rho_a = rho_ab.partial_trace_b(2, 2).unwrap();
        assert_eq!(rho_a.dimension(), 2);
        assert!((rho_a.purity() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_tensor_product() {
        let rho_0 = DensityMatrix::from_pure_state(&QuantumState::basis_state(2, 0).unwrap());
        let rho_1 = DensityMatrix::from_pure_state(&QuantumState::basis_state(2, 1).unwrap());

        let rho_01 = rho_0.tensor(&rho_1);
        assert_eq!(rho_01.dimension(), 4);

        // Should be |01⟩⟨01|
        assert!((rho_01.matrix.get(1, 1).re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_spectral_decomposition() {
        // Pure state: eigenvalues should be [1, 0], entropy 0, purity 1
        let rho = DensityMatrix::from_pure_state(&QuantumState::ground_state(1));
        let spectral = rho.spectral_decomposition();

        assert!((spectral.purity - 1.0).abs() < 1e-5);
        assert!(spectral.entropy.abs() < 1e-5);
        assert_eq!(spectral.effective_rank, 1);
    }

    #[test]
    fn test_spectral_decomposition_mixed() {
        // Maximally mixed 2-qubit: eigenvalues [0.5, 0.5], entropy = ln(2), purity = 0.5
        let rho = DensityMatrix::maximally_mixed(2);
        let spectral = rho.spectral_decomposition();

        assert!((spectral.purity - 0.5).abs() < 1e-5);
        assert!((spectral.entropy - 2.0_f64.ln()).abs() < 1e-3);
        assert_eq!(spectral.effective_rank, 2);
    }

    #[test]
    fn test_purity_fast() {
        let rho = DensityMatrix::from_pure_state(&QuantumState::ground_state(1));
        let purity_fast = rho.purity_fast();
        let purity_orig = rho.purity();

        assert!((purity_fast - purity_orig).abs() < 1e-10);
    }

    #[test]
    fn test_purity_fast_mixed() {
        let rho = DensityMatrix::maximally_mixed(2);
        let purity_fast = rho.purity_fast();
        let purity_orig = rho.purity();

        assert!((purity_fast - purity_orig).abs() < 1e-10);
    }
}
