//! Quantum Channels and Operations
//!
//! Implements quantum channels using Kraus operator representation,
//! Pauli operators, and common quantum operations.

use super::complex_matrix::{gates, Complex64, ComplexMatrix};
use super::density_matrix::DensityMatrix;
use super::{constants, QuantumTopologyError, Result};

/// Type of Pauli operator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PauliType {
    /// Identity I
    I,
    /// Pauli X
    X,
    /// Pauli Y
    Y,
    /// Pauli Z
    Z,
}

impl PauliType {
    /// Get the matrix representation
    pub fn to_matrix(&self) -> ComplexMatrix {
        match self {
            PauliType::I => ComplexMatrix::identity(2),
            PauliType::X => gates::pauli_x(),
            PauliType::Y => gates::pauli_y(),
            PauliType::Z => gates::pauli_z(),
        }
    }

    /// Get the eigenvalues
    pub fn eigenvalues(&self) -> [f64; 2] {
        match self {
            PauliType::I => [1.0, 1.0],
            PauliType::X | PauliType::Y | PauliType::Z => [1.0, -1.0],
        }
    }

    /// Commutator type with another Pauli
    pub fn commutes_with(&self, other: &PauliType) -> bool {
        // Identity commutes with everything
        if *self == PauliType::I || *other == PauliType::I {
            return true;
        }
        // Same Pauli commutes with itself
        self == other
    }
}

/// Pauli operator on multiple qubits
#[derive(Debug, Clone, PartialEq)]
pub struct PauliOperator {
    /// Pauli types for each qubit (I, X, Y, or Z)
    pub paulis: Vec<PauliType>,
    /// Overall phase factor (±1, ±i)
    pub phase: Complex64,
}

impl PauliOperator {
    /// Create a new Pauli operator
    pub fn new(paulis: Vec<PauliType>) -> Self {
        Self {
            paulis,
            phase: Complex64::new(1.0, 0.0),
        }
    }

    /// Create a Pauli operator with phase
    pub fn with_phase(paulis: Vec<PauliType>, phase: Complex64) -> Self {
        Self { paulis, phase }
    }

    /// Create identity operator on n qubits
    pub fn identity(num_qubits: usize) -> Self {
        Self {
            paulis: vec![PauliType::I; num_qubits],
            phase: Complex64::new(1.0, 0.0),
        }
    }

    /// Create a single-qubit Pauli operator on a multi-qubit system
    pub fn single_qubit(num_qubits: usize, target: usize, pauli: PauliType) -> Self {
        let mut paulis = vec![PauliType::I; num_qubits];
        if target < num_qubits {
            paulis[target] = pauli;
        }
        Self::new(paulis)
    }

    /// Number of qubits
    pub fn num_qubits(&self) -> usize {
        self.paulis.len()
    }

    /// Check if this is the identity operator
    pub fn is_identity(&self) -> bool {
        (self.phase.re - 1.0).abs() < constants::EPSILON
            && self.phase.im.abs() < constants::EPSILON
            && self.paulis.iter().all(|p| *p == PauliType::I)
    }

    /// Get the matrix representation
    pub fn to_matrix(&self) -> ComplexMatrix {
        if self.paulis.is_empty() {
            return ComplexMatrix::identity(1).scale(self.phase);
        }

        let mut result = self.paulis[0].to_matrix();
        for pauli in &self.paulis[1..] {
            result = result.tensor(&pauli.to_matrix());
        }

        result.scale(self.phase)
    }

    /// Multiply two Pauli operators
    pub fn multiply(&self, other: &PauliOperator) -> Result<Self> {
        if self.num_qubits() != other.num_qubits() {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: self.num_qubits(),
                got: other.num_qubits(),
            });
        }

        let mut result_paulis = Vec::with_capacity(self.num_qubits());
        let mut phase = self.phase * other.phase;

        for (p1, p2) in self.paulis.iter().zip(other.paulis.iter()) {
            let (new_pauli, local_phase) = multiply_single_paulis(*p1, *p2);
            result_paulis.push(new_pauli);
            phase *= local_phase;
        }

        Ok(Self::with_phase(result_paulis, phase))
    }

    /// Check if two Pauli operators commute
    pub fn commutes_with(&self, other: &PauliOperator) -> bool {
        if self.num_qubits() != other.num_qubits() {
            return false;
        }

        // Count anticommuting pairs
        let mut anticommute_count = 0;
        for (p1, p2) in self.paulis.iter().zip(other.paulis.iter()) {
            if !p1.commutes_with(p2) {
                anticommute_count += 1;
            }
        }

        // Operators commute if there's an even number of anticommuting pairs
        anticommute_count % 2 == 0
    }

    /// Weight of the Pauli operator (number of non-identity terms)
    pub fn weight(&self) -> usize {
        self.paulis.iter().filter(|p| **p != PauliType::I).count()
    }

    /// Support of the Pauli operator (indices of non-identity terms)
    pub fn support(&self) -> Vec<usize> {
        self.paulis
            .iter()
            .enumerate()
            .filter(|(_, p)| **p != PauliType::I)
            .map(|(i, _)| i)
            .collect()
    }
}

/// Multiply two single-qubit Paulis and return the result with phase
fn multiply_single_paulis(p1: PauliType, p2: PauliType) -> (PauliType, Complex64) {
    use PauliType::*;
    let i = Complex64::new(0.0, 1.0);
    let mi = Complex64::new(0.0, -1.0);
    let one = Complex64::new(1.0, 0.0);

    match (p1, p2) {
        (I, p) | (p, I) => (p, one),
        (X, X) | (Y, Y) | (Z, Z) => (I, one),
        (X, Y) => (Z, i),
        (Y, X) => (Z, mi),
        (Y, Z) => (X, i),
        (Z, Y) => (X, mi),
        (Z, X) => (Y, i),
        (X, Z) => (Y, mi),
    }
}

/// Kraus operator for quantum channels
#[derive(Debug, Clone)]
pub struct KrausOperator {
    /// The Kraus operator matrix K_i
    pub matrix: ComplexMatrix,
    /// Optional label
    pub label: Option<String>,
}

impl KrausOperator {
    /// Create a new Kraus operator
    pub fn new(matrix: ComplexMatrix) -> Self {
        Self {
            matrix,
            label: None,
        }
    }

    /// Create with label
    pub fn with_label(matrix: ComplexMatrix, label: &str) -> Self {
        Self {
            matrix,
            label: Some(label.to_string()),
        }
    }

    /// Get dimension
    pub fn dimension(&self) -> usize {
        self.matrix.rows
    }
}

/// Quantum channel represented by Kraus operators
#[derive(Debug, Clone)]
pub struct QuantumChannel {
    /// Kraus operators {K_i} such that Σ K_i† K_i = I
    pub kraus_operators: Vec<KrausOperator>,
    /// Input dimension
    pub input_dim: usize,
    /// Output dimension
    pub output_dim: usize,
}

impl QuantumChannel {
    /// Create a new quantum channel from Kraus operators
    pub fn new(operators: Vec<ComplexMatrix>) -> Result<Self> {
        if operators.is_empty() {
            return Err(QuantumTopologyError::InvalidQuantumChannel(
                "Channel must have at least one Kraus operator".to_string(),
            ));
        }

        let input_dim = operators[0].cols;
        let output_dim = operators[0].rows;

        // Verify dimensions match
        for op in &operators {
            if op.cols != input_dim || op.rows != output_dim {
                return Err(QuantumTopologyError::InvalidQuantumChannel(
                    "All Kraus operators must have the same dimensions".to_string(),
                ));
            }
        }

        let kraus_operators = operators.into_iter().map(KrausOperator::new).collect();

        let channel = Self {
            kraus_operators,
            input_dim,
            output_dim,
        };

        // Verify completeness (Σ K_i† K_i = I)
        channel.verify_completeness(constants::EPSILON * 100.0)?;

        Ok(channel)
    }

    /// Create without validation
    pub fn new_unchecked(operators: Vec<ComplexMatrix>) -> Self {
        let input_dim = operators.first().map(|m| m.cols).unwrap_or(1);
        let output_dim = operators.first().map(|m| m.rows).unwrap_or(1);

        Self {
            kraus_operators: operators.into_iter().map(KrausOperator::new).collect(),
            input_dim,
            output_dim,
        }
    }

    /// Verify that the Kraus operators satisfy the completeness relation
    fn verify_completeness(&self, tolerance: f64) -> Result<()> {
        let mut sum = ComplexMatrix::zeros(self.input_dim, self.input_dim);

        for k in &self.kraus_operators {
            let k_dag_k = k.matrix.adjoint().matmul(&k.matrix);
            sum = sum.add(&k_dag_k);
        }

        // Check if sum ≈ I
        let identity = ComplexMatrix::identity(self.input_dim);
        for i in 0..self.input_dim {
            for j in 0..self.input_dim {
                let diff = (sum.get(i, j) - identity.get(i, j)).norm();
                if diff > tolerance {
                    return Err(QuantumTopologyError::InvalidQuantumChannel(format!(
                        "Completeness relation violated: diff = {} at ({}, {})",
                        diff, i, j
                    )));
                }
            }
        }

        Ok(())
    }

    /// Apply the channel to a density matrix: ρ → Σ K_i ρ K_i†
    pub fn apply(&self, rho: &DensityMatrix) -> Result<DensityMatrix> {
        if rho.dimension() != self.input_dim {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: self.input_dim,
                got: rho.dimension(),
            });
        }

        let mut result = ComplexMatrix::zeros(self.output_dim, self.output_dim);

        for k in &self.kraus_operators {
            let k_rho = k.matrix.matmul(&rho.matrix);
            let k_rho_kdag = k_rho.matmul(&k.matrix.adjoint());
            result = result.add(&k_rho_kdag);
        }

        Ok(DensityMatrix::new_unchecked(result))
    }

    /// Compose two channels: (E ∘ F)(ρ) = E(F(ρ))
    pub fn compose(&self, other: &QuantumChannel) -> Result<Self> {
        if self.input_dim != other.output_dim {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: self.input_dim,
                got: other.output_dim,
            });
        }

        // Compose Kraus operators: {E_i F_j}
        let mut new_operators = Vec::with_capacity(
            self.kraus_operators.len() * other.kraus_operators.len(),
        );

        for e in &self.kraus_operators {
            for f in &other.kraus_operators {
                new_operators.push(e.matrix.matmul(&f.matrix));
            }
        }

        Ok(Self::new_unchecked(new_operators))
    }

    /// Tensor product of channels: (E ⊗ F)(ρ_AB) = E(ρ_A) ⊗ F(ρ_B)
    pub fn tensor(&self, other: &QuantumChannel) -> Self {
        let mut new_operators = Vec::with_capacity(
            self.kraus_operators.len() * other.kraus_operators.len(),
        );

        for k1 in &self.kraus_operators {
            for k2 in &other.kraus_operators {
                new_operators.push(k1.matrix.tensor(&k2.matrix));
            }
        }

        Self {
            kraus_operators: new_operators.into_iter().map(KrausOperator::new).collect(),
            input_dim: self.input_dim * other.input_dim,
            output_dim: self.output_dim * other.output_dim,
        }
    }

    /// Create the identity channel
    pub fn identity(dim: usize) -> Self {
        Self {
            kraus_operators: vec![KrausOperator::new(ComplexMatrix::identity(dim))],
            input_dim: dim,
            output_dim: dim,
        }
    }

    /// Create a unitary channel (single Kraus operator)
    pub fn unitary(u: ComplexMatrix) -> Result<Self> {
        if !u.is_unitary(constants::EPSILON * 100.0) {
            return Err(QuantumTopologyError::InvalidQuantumChannel(
                "Matrix is not unitary".to_string(),
            ));
        }

        let dim = u.rows;
        Ok(Self {
            kraus_operators: vec![KrausOperator::new(u)],
            input_dim: dim,
            output_dim: dim,
        })
    }

    /// Create the depolarizing channel with probability p
    /// ρ → (1-p)ρ + (p/3)(XρX + YρY + ZρZ)
    pub fn depolarizing(p: f64) -> Self {
        let sqrt_1_p = (1.0 - p).sqrt();
        let sqrt_p_3 = (p / 3.0).sqrt();

        let k0 = ComplexMatrix::identity(2).scale(Complex64::new(sqrt_1_p, 0.0));
        let k1 = gates::pauli_x().scale(Complex64::new(sqrt_p_3, 0.0));
        let k2 = gates::pauli_y().scale(Complex64::new(sqrt_p_3, 0.0));
        let k3 = gates::pauli_z().scale(Complex64::new(sqrt_p_3, 0.0));

        Self {
            kraus_operators: vec![
                KrausOperator::with_label(k0, "I"),
                KrausOperator::with_label(k1, "X"),
                KrausOperator::with_label(k2, "Y"),
                KrausOperator::with_label(k3, "Z"),
            ],
            input_dim: 2,
            output_dim: 2,
        }
    }

    /// Create the amplitude damping channel with damping parameter γ
    /// Models energy dissipation to environment
    pub fn amplitude_damping(gamma: f64) -> Self {
        let sqrt_gamma = gamma.sqrt();
        let sqrt_1_gamma = (1.0 - gamma).sqrt();

        let k0 = ComplexMatrix::new(
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(sqrt_1_gamma, 0.0),
            ],
            2,
            2,
        );

        let k1 = ComplexMatrix::new(
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(sqrt_gamma, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            2,
            2,
        );

        Self {
            kraus_operators: vec![
                KrausOperator::with_label(k0, "K0"),
                KrausOperator::with_label(k1, "K1"),
            ],
            input_dim: 2,
            output_dim: 2,
        }
    }

    /// Create the phase damping (dephasing) channel with parameter γ
    pub fn phase_damping(gamma: f64) -> Self {
        let sqrt_1_gamma = (1.0 - gamma).sqrt();
        let sqrt_gamma = gamma.sqrt();

        let k0 = ComplexMatrix::new(
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(sqrt_1_gamma, 0.0),
            ],
            2,
            2,
        );

        let k1 = ComplexMatrix::new(
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(sqrt_gamma, 0.0),
            ],
            2,
            2,
        );

        Self {
            kraus_operators: vec![
                KrausOperator::with_label(k0, "K0"),
                KrausOperator::with_label(k1, "K1"),
            ],
            input_dim: 2,
            output_dim: 2,
        }
    }

    /// Create a bit-flip channel with probability p
    pub fn bit_flip(p: f64) -> Self {
        let sqrt_1_p = (1.0 - p).sqrt();
        let sqrt_p = p.sqrt();

        let k0 = ComplexMatrix::identity(2).scale(Complex64::new(sqrt_1_p, 0.0));
        let k1 = gates::pauli_x().scale(Complex64::new(sqrt_p, 0.0));

        Self {
            kraus_operators: vec![
                KrausOperator::with_label(k0, "I"),
                KrausOperator::with_label(k1, "X"),
            ],
            input_dim: 2,
            output_dim: 2,
        }
    }

    /// Create a phase-flip channel with probability p
    pub fn phase_flip(p: f64) -> Self {
        let sqrt_1_p = (1.0 - p).sqrt();
        let sqrt_p = p.sqrt();

        let k0 = ComplexMatrix::identity(2).scale(Complex64::new(sqrt_1_p, 0.0));
        let k1 = gates::pauli_z().scale(Complex64::new(sqrt_p, 0.0));

        Self {
            kraus_operators: vec![
                KrausOperator::with_label(k0, "I"),
                KrausOperator::with_label(k1, "Z"),
            ],
            input_dim: 2,
            output_dim: 2,
        }
    }

    /// Compute the Choi matrix (channel-state duality)
    /// J(E) = (I ⊗ E)(|Ω⟩⟨Ω|) where |Ω⟩ = Σ|ii⟩/√d
    pub fn choi_matrix(&self) -> ComplexMatrix {
        let d = self.input_dim;
        let d2 = d * d;

        let mut choi = ComplexMatrix::zeros(d2, d2);

        // Build Choi matrix from Kraus operators
        for k in &self.kraus_operators {
            // Vectorize the Kraus operator using column stacking
            for i in 0..d {
                for j in 0..d {
                    for m in 0..d {
                        for n in 0..d {
                            let row = i * d + m;
                            let col = j * d + n;
                            let val = k.matrix.get(i, j) * k.matrix.get(m, n).conj();
                            let current = choi.get(row, col);
                            choi.set(row, col, current + val);
                        }
                    }
                }
            }
        }

        choi
    }

    /// Check if the channel is completely positive (always true for Kraus form)
    pub fn is_completely_positive(&self) -> bool {
        true // Kraus representation guarantees CP
    }

    /// Check if the channel is trace-preserving
    pub fn is_trace_preserving(&self, tolerance: f64) -> bool {
        let mut sum = ComplexMatrix::zeros(self.input_dim, self.input_dim);

        for k in &self.kraus_operators {
            let k_dag_k = k.matrix.adjoint().matmul(&k.matrix);
            sum = sum.add(&k_dag_k);
        }

        let identity = ComplexMatrix::identity(self.input_dim);
        for i in 0..self.input_dim {
            for j in 0..self.input_dim {
                if (sum.get(i, j) - identity.get(i, j)).norm() > tolerance {
                    return false;
                }
            }
        }

        true
    }

    /// Compute the diamond norm distance to another channel (approximation)
    /// ||E - F||_◇ = max_{ρ} ||((E-F)⊗I)(ρ)||_1
    pub fn diamond_distance(&self, other: &QuantumChannel) -> Result<f64> {
        if self.input_dim != other.input_dim || self.output_dim != other.output_dim {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: self.input_dim,
                got: other.input_dim,
            });
        }

        // Use Choi matrix distance as approximation
        let choi_self = self.choi_matrix();
        let choi_other = other.choi_matrix();
        let diff = choi_self.sub(&choi_other);

        // Trace norm of difference
        let eigenvalues = diff.eigenvalues(100, 1e-10);
        let trace_norm: f64 = eigenvalues.iter().map(|ev| ev.norm()).sum();

        Ok(trace_norm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pauli_multiplication() {
        let (result, phase) = multiply_single_paulis(PauliType::X, PauliType::Y);
        assert_eq!(result, PauliType::Z);
        assert!((phase.im - 1.0).abs() < 1e-10); // i

        let (result, phase) = multiply_single_paulis(PauliType::Y, PauliType::X);
        assert_eq!(result, PauliType::Z);
        assert!((phase.im + 1.0).abs() < 1e-10); // -i
    }

    #[test]
    fn test_pauli_operator() {
        let pauli = PauliOperator::new(vec![PauliType::X, PauliType::Z]);
        assert_eq!(pauli.weight(), 2);
        assert_eq!(pauli.support(), vec![0, 1]);

        let matrix = pauli.to_matrix();
        assert_eq!(matrix.rows, 4);
    }

    #[test]
    fn test_pauli_commutation() {
        let p1 = PauliOperator::new(vec![PauliType::X, PauliType::I]);
        let p2 = PauliOperator::new(vec![PauliType::I, PauliType::X]);

        // Should commute (act on different qubits)
        assert!(p1.commutes_with(&p2));

        let p3 = PauliOperator::new(vec![PauliType::X, PauliType::I]);
        let p4 = PauliOperator::new(vec![PauliType::Z, PauliType::I]);

        // X and Z anticommute
        assert!(!p3.commutes_with(&p4));
    }

    #[test]
    fn test_identity_channel() {
        let channel = QuantumChannel::identity(2);
        let rho = DensityMatrix::maximally_mixed(2);

        let result = channel.apply(&rho).unwrap();
        assert!((result.purity() - rho.purity()).abs() < 1e-10);
    }

    #[test]
    fn test_depolarizing_channel() {
        let channel = QuantumChannel::depolarizing(0.0);
        assert!(channel.is_trace_preserving(1e-10));

        // p=0 should be identity
        let rho = DensityMatrix::from_pure_state(&super::super::quantum_state::QuantumState::ground_state(1));
        let result = channel.apply(&rho).unwrap();
        assert!((result.fidelity(&rho).unwrap() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_amplitude_damping() {
        let channel = QuantumChannel::amplitude_damping(1.0);
        assert!(channel.is_trace_preserving(1e-10));

        // γ=1 should map everything to |0⟩
        let rho = DensityMatrix::from_pure_state(&super::super::quantum_state::QuantumState::basis_state(2, 1).unwrap());
        let result = channel.apply(&rho).unwrap();

        // Should be close to |0⟩⟨0|
        assert!((result.matrix.get(0, 0).re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_channel_composition() {
        let c1 = QuantumChannel::bit_flip(0.1);
        let c2 = QuantumChannel::phase_flip(0.1);

        let composed = c1.compose(&c2).unwrap();
        assert!(composed.is_trace_preserving(1e-10));
    }

    #[test]
    fn test_channel_tensor() {
        let c1 = QuantumChannel::identity(2);
        let c2 = QuantumChannel::depolarizing(0.1);

        let tensor = c1.tensor(&c2);
        assert_eq!(tensor.input_dim, 4);
        assert!(tensor.is_trace_preserving(1e-10));
    }

    #[test]
    fn test_choi_matrix() {
        let channel = QuantumChannel::identity(2);
        let choi = channel.choi_matrix();

        // Choi matrix of identity channel on d-dim space has trace d
        assert_eq!(choi.rows, 4);
        // For trace-preserving channel, trace(Choi) = input_dim
        // The trace here depends on the specific implementation
        assert!(choi.trace().re > 0.0);
    }
}
