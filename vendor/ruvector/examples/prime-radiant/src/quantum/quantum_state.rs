//! Quantum State Representation
//!
//! Pure quantum states represented as normalized complex vectors in Hilbert space.

use super::complex_matrix::{gates, Complex64, ComplexMatrix, ComplexVector};
use super::{constants, QuantumTopologyError, Result};

/// Computational basis for qubits
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantumBasis {
    /// Computational basis |0⟩, |1⟩
    Computational,
    /// Hadamard basis |+⟩, |-⟩
    Hadamard,
    /// Circular basis |R⟩, |L⟩
    Circular,
}

/// Single qubit state
#[derive(Debug, Clone)]
pub struct Qubit {
    /// Amplitude for |0⟩
    pub alpha: Complex64,
    /// Amplitude for |1⟩
    pub beta: Complex64,
}

impl Qubit {
    /// Create a new qubit state |ψ⟩ = α|0⟩ + β|1⟩
    /// Normalizes the state automatically
    pub fn new(alpha: Complex64, beta: Complex64) -> Self {
        let norm = (alpha.norm_sqr() + beta.norm_sqr()).sqrt();
        if norm < constants::EPSILON {
            // Default to |0⟩ if both amplitudes are zero
            Self {
                alpha: Complex64::new(1.0, 0.0),
                beta: Complex64::new(0.0, 0.0),
            }
        } else {
            Self {
                alpha: alpha / norm,
                beta: beta / norm,
            }
        }
    }

    /// Create |0⟩ state
    pub fn zero() -> Self {
        Self {
            alpha: Complex64::new(1.0, 0.0),
            beta: Complex64::new(0.0, 0.0),
        }
    }

    /// Create |1⟩ state
    pub fn one() -> Self {
        Self {
            alpha: Complex64::new(0.0, 0.0),
            beta: Complex64::new(1.0, 0.0),
        }
    }

    /// Create |+⟩ = (|0⟩ + |1⟩)/√2 state
    pub fn plus() -> Self {
        let s = 1.0 / 2.0_f64.sqrt();
        Self {
            alpha: Complex64::new(s, 0.0),
            beta: Complex64::new(s, 0.0),
        }
    }

    /// Create |-⟩ = (|0⟩ - |1⟩)/√2 state
    pub fn minus() -> Self {
        let s = 1.0 / 2.0_f64.sqrt();
        Self {
            alpha: Complex64::new(s, 0.0),
            beta: Complex64::new(-s, 0.0),
        }
    }

    /// Create a state from Bloch sphere coordinates (θ, φ)
    /// |ψ⟩ = cos(θ/2)|0⟩ + e^{iφ}sin(θ/2)|1⟩
    pub fn from_bloch(theta: f64, phi: f64) -> Self {
        let alpha = Complex64::new((theta / 2.0).cos(), 0.0);
        let beta = Complex64::from_polar((theta / 2.0).sin(), phi);
        Self { alpha, beta }
    }

    /// Get Bloch sphere coordinates (θ, φ)
    pub fn to_bloch(&self) -> (f64, f64) {
        let theta = 2.0 * self.alpha.norm().acos();
        let phi = if self.beta.norm() < constants::EPSILON {
            0.0
        } else {
            (self.beta / self.alpha).arg()
        };
        (theta, phi)
    }

    /// Probability of measuring |0⟩
    pub fn prob_zero(&self) -> f64 {
        self.alpha.norm_sqr()
    }

    /// Probability of measuring |1⟩
    pub fn prob_one(&self) -> f64 {
        self.beta.norm_sqr()
    }

    /// Convert to a ComplexVector representation
    pub fn to_vector(&self) -> ComplexVector {
        ComplexVector::new(vec![self.alpha, self.beta])
    }

    /// Apply a single-qubit gate
    pub fn apply_gate(&self, gate: &ComplexMatrix) -> Result<Self> {
        if gate.rows != 2 || gate.cols != 2 {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: 2,
                got: gate.rows,
            });
        }

        let new_alpha = gate.get(0, 0) * self.alpha + gate.get(0, 1) * self.beta;
        let new_beta = gate.get(1, 0) * self.alpha + gate.get(1, 1) * self.beta;

        Ok(Self::new(new_alpha, new_beta))
    }

    /// Apply Hadamard gate
    pub fn hadamard(&self) -> Self {
        self.apply_gate(&gates::hadamard()).unwrap()
    }

    /// Apply Pauli X gate (NOT)
    pub fn pauli_x(&self) -> Self {
        Self::new(self.beta, self.alpha)
    }

    /// Apply Pauli Y gate
    pub fn pauli_y(&self) -> Self {
        let i = Complex64::new(0.0, 1.0);
        Self::new(-i * self.beta, i * self.alpha)
    }

    /// Apply Pauli Z gate
    pub fn pauli_z(&self) -> Self {
        Self::new(self.alpha, -self.beta)
    }

    /// Compute inner product ⟨self|other⟩
    pub fn inner(&self, other: &Qubit) -> Complex64 {
        self.alpha.conj() * other.alpha + self.beta.conj() * other.beta
    }

    /// Compute fidelity |⟨self|other⟩|²
    pub fn fidelity(&self, other: &Qubit) -> f64 {
        self.inner(other).norm_sqr()
    }
}

/// N-qubit quantum state (pure state)
#[derive(Debug, Clone)]
pub struct QuantumState {
    /// State amplitudes in computational basis
    pub amplitudes: Vec<Complex64>,
    /// Hilbert space dimension (2^n for n qubits)
    pub dimension: usize,
}

impl QuantumState {
    /// Create a new quantum state from amplitudes
    /// Normalizes the state automatically
    pub fn new(amplitudes: Vec<Complex64>) -> Result<Self> {
        if amplitudes.is_empty() {
            return Err(QuantumTopologyError::InvalidQuantumState(
                "Empty amplitude vector".to_string(),
            ));
        }

        // Check if dimension is a power of 2
        let dimension = amplitudes.len();
        if dimension != 1 && (dimension & (dimension - 1)) != 0 {
            return Err(QuantumTopologyError::InvalidQuantumState(
                format!("Dimension {} is not a power of 2", dimension),
            ));
        }

        let mut state = Self {
            amplitudes,
            dimension,
        };
        state.normalize();
        Ok(state)
    }

    /// Create a quantum state without dimension check (for non-qubit systems)
    pub fn new_unchecked(amplitudes: Vec<Complex64>) -> Self {
        let dimension = amplitudes.len();
        let mut state = Self {
            amplitudes,
            dimension,
        };
        state.normalize();
        state
    }

    /// Create computational basis state |i⟩
    pub fn basis_state(dimension: usize, index: usize) -> Result<Self> {
        if index >= dimension {
            return Err(QuantumTopologyError::InvalidQuantumState(
                format!("Index {} out of bounds for dimension {}", index, dimension),
            ));
        }

        let mut amplitudes = vec![Complex64::new(0.0, 0.0); dimension];
        amplitudes[index] = Complex64::new(1.0, 0.0);

        Ok(Self {
            amplitudes,
            dimension,
        })
    }

    /// Create the ground state |0...0⟩ for n qubits
    pub fn ground_state(num_qubits: usize) -> Self {
        let dimension = 1 << num_qubits;
        let mut amplitudes = vec![Complex64::new(0.0, 0.0); dimension];
        amplitudes[0] = Complex64::new(1.0, 0.0);

        Self {
            amplitudes,
            dimension,
        }
    }

    /// Create a uniform superposition state
    pub fn uniform_superposition(num_qubits: usize) -> Self {
        let dimension = 1 << num_qubits;
        let amplitude = Complex64::new(1.0 / (dimension as f64).sqrt(), 0.0);
        let amplitudes = vec![amplitude; dimension];

        Self {
            amplitudes,
            dimension,
        }
    }

    /// Create a GHZ state (|0...0⟩ + |1...1⟩)/√2
    pub fn ghz_state(num_qubits: usize) -> Self {
        let dimension = 1 << num_qubits;
        let mut amplitudes = vec![Complex64::new(0.0, 0.0); dimension];
        let amplitude = Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0);

        amplitudes[0] = amplitude;
        amplitudes[dimension - 1] = amplitude;

        Self {
            amplitudes,
            dimension,
        }
    }

    /// Create a W state (|10...0⟩ + |01...0⟩ + ... + |0...01⟩)/√n
    pub fn w_state(num_qubits: usize) -> Self {
        let dimension = 1 << num_qubits;
        let mut amplitudes = vec![Complex64::new(0.0, 0.0); dimension];
        let amplitude = Complex64::new(1.0 / (num_qubits as f64).sqrt(), 0.0);

        for i in 0..num_qubits {
            amplitudes[1 << i] = amplitude;
        }

        Self {
            amplitudes,
            dimension,
        }
    }

    /// Number of qubits in the system
    pub fn num_qubits(&self) -> usize {
        (self.dimension as f64).log2() as usize
    }

    /// Normalize the state in place
    pub fn normalize(&mut self) {
        let norm: f64 = self.amplitudes.iter().map(|c| c.norm_sqr()).sum::<f64>().sqrt();
        if norm > constants::EPSILON {
            for c in &mut self.amplitudes {
                *c /= norm;
            }
        }
    }

    /// Get the norm of the state vector
    pub fn norm(&self) -> f64 {
        self.amplitudes.iter().map(|c| c.norm_sqr()).sum::<f64>().sqrt()
    }

    /// Convert to a ComplexVector
    pub fn to_vector(&self) -> ComplexVector {
        ComplexVector::new(self.amplitudes.clone())
    }

    /// Create from a ComplexVector
    pub fn from_vector(v: ComplexVector) -> Result<Self> {
        Self::new(v.data)
    }

    /// Inner product ⟨self|other⟩
    pub fn inner(&self, other: &QuantumState) -> Result<Complex64> {
        if self.dimension != other.dimension {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: self.dimension,
                got: other.dimension,
            });
        }

        Ok(self
            .amplitudes
            .iter()
            .zip(other.amplitudes.iter())
            .map(|(a, b)| a.conj() * b)
            .sum())
    }

    /// Fidelity |⟨self|other⟩|² (for pure states)
    pub fn fidelity(&self, other: &QuantumState) -> Result<f64> {
        Ok(self.inner(other)?.norm_sqr())
    }

    /// Tensor product |self⟩ ⊗ |other⟩
    pub fn tensor(&self, other: &QuantumState) -> Self {
        let new_dimension = self.dimension * other.dimension;
        let mut new_amplitudes = Vec::with_capacity(new_dimension);

        for a in &self.amplitudes {
            for b in &other.amplitudes {
                new_amplitudes.push(a * b);
            }
        }

        Self {
            amplitudes: new_amplitudes,
            dimension: new_dimension,
        }
    }

    /// Apply a unitary operator to the state
    pub fn apply_operator(&self, operator: &ComplexMatrix) -> Result<Self> {
        if operator.rows != self.dimension || operator.cols != self.dimension {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: self.dimension,
                got: operator.rows,
            });
        }

        let result = operator.matvec(&self.to_vector());
        Ok(Self {
            amplitudes: result.data,
            dimension: self.dimension,
        })
    }

    /// Apply a single-qubit gate to qubit at position `target`
    pub fn apply_single_qubit_gate(&self, gate: &ComplexMatrix, target: usize) -> Result<Self> {
        let num_qubits = self.num_qubits();
        if target >= num_qubits {
            return Err(QuantumTopologyError::InvalidQuantumState(
                format!("Target qubit {} out of range for {}-qubit system", target, num_qubits),
            ));
        }

        // Build the full operator using tensor products
        let mut full_operator = ComplexMatrix::identity(1);

        for i in 0..num_qubits {
            let op = if i == target {
                gate.clone()
            } else {
                ComplexMatrix::identity(2)
            };
            full_operator = full_operator.tensor(&op);
        }

        self.apply_operator(&full_operator)
    }

    /// Probability of measuring basis state |i⟩
    pub fn probability(&self, index: usize) -> f64 {
        if index >= self.dimension {
            return 0.0;
        }
        self.amplitudes[index].norm_sqr()
    }

    /// Get probability distribution over all basis states
    pub fn probability_distribution(&self) -> Vec<f64> {
        self.amplitudes.iter().map(|c| c.norm_sqr()).collect()
    }

    /// Measure the state in computational basis (collapses state)
    /// Returns the measured index and the collapsed state
    pub fn measure(&self, random_value: f64) -> (usize, Self) {
        let probs = self.probability_distribution();
        let mut cumulative = 0.0;
        let mut result = 0;

        for (i, &p) in probs.iter().enumerate() {
            cumulative += p;
            if random_value < cumulative {
                result = i;
                break;
            }
        }

        // Collapse to measured state
        let collapsed = Self::basis_state(self.dimension, result).unwrap();
        (result, collapsed)
    }

    /// Partial measurement of qubit at position `target`
    pub fn measure_qubit(&self, target: usize, random_value: f64) -> (bool, Self) {
        let num_qubits = self.num_qubits();
        if target >= num_qubits {
            return (false, self.clone());
        }

        // Calculate probability of measuring |0⟩
        let mut prob_zero = 0.0;
        for i in 0..self.dimension {
            if (i >> target) & 1 == 0 {
                prob_zero += self.amplitudes[i].norm_sqr();
            }
        }

        let measured_one = random_value >= prob_zero;
        let normalization = if measured_one {
            (1.0 - prob_zero).sqrt()
        } else {
            prob_zero.sqrt()
        };

        // Collapse the state
        let mut new_amplitudes = vec![Complex64::new(0.0, 0.0); self.dimension];
        for i in 0..self.dimension {
            let qubit_val = (i >> target) & 1;
            if (qubit_val == 1) == measured_one {
                new_amplitudes[i] = self.amplitudes[i] / normalization;
            }
        }

        let collapsed = Self {
            amplitudes: new_amplitudes,
            dimension: self.dimension,
        };

        (measured_one, collapsed)
    }

    /// Compute von Neumann entropy (for pure states, this is 0)
    /// For entanglement entropy, use partial trace first
    pub fn von_neumann_entropy(&self) -> f64 {
        // For a pure state, von Neumann entropy is 0
        0.0
    }

    /// Compute the density matrix |ψ⟩⟨ψ|
    pub fn to_density_matrix(&self) -> ComplexMatrix {
        self.to_vector().outer(&self.to_vector())
    }

    /// Compute the reduced density matrix by tracing out specified qubits
    pub fn reduced_density_matrix(&self, keep_qubits: &[usize]) -> ComplexMatrix {
        let num_qubits = self.num_qubits();
        let trace_qubits: Vec<usize> = (0..num_qubits)
            .filter(|q| !keep_qubits.contains(q))
            .collect();

        if trace_qubits.is_empty() {
            return self.to_density_matrix();
        }

        let keep_dim = 1 << keep_qubits.len();
        let trace_dim = 1 << trace_qubits.len();

        let mut reduced = ComplexMatrix::zeros(keep_dim, keep_dim);

        for i in 0..keep_dim {
            for j in 0..keep_dim {
                let mut sum = Complex64::new(0.0, 0.0);

                for k in 0..trace_dim {
                    // Reconstruct full indices
                    let full_i = self.reconstruct_index(i, k, keep_qubits, &trace_qubits);
                    let full_j = self.reconstruct_index(j, k, keep_qubits, &trace_qubits);

                    sum += self.amplitudes[full_i] * self.amplitudes[full_j].conj();
                }

                reduced.set(i, j, sum);
            }
        }

        reduced
    }

    /// Helper to reconstruct full index from partial indices
    fn reconstruct_index(
        &self,
        keep_idx: usize,
        trace_idx: usize,
        keep_qubits: &[usize],
        trace_qubits: &[usize],
    ) -> usize {
        let num_qubits = self.num_qubits();
        let mut full_idx = 0;

        for (i, &q) in keep_qubits.iter().enumerate() {
            if (keep_idx >> i) & 1 == 1 {
                full_idx |= 1 << q;
            }
        }

        for (i, &q) in trace_qubits.iter().enumerate() {
            if (trace_idx >> i) & 1 == 1 {
                full_idx |= 1 << q;
            }
        }

        full_idx.min(self.dimension - 1)
    }

    /// Compute entanglement entropy between subsystems
    /// Splits the system at `split_point` qubits from the left
    pub fn entanglement_entropy(&self, split_point: usize) -> f64 {
        let num_qubits = self.num_qubits();
        if split_point == 0 || split_point >= num_qubits {
            return 0.0;
        }

        let keep_qubits: Vec<usize> = (0..split_point).collect();
        let reduced = self.reduced_density_matrix(&keep_qubits);

        // Compute eigenvalues of reduced density matrix
        let eigenvalues = reduced.eigenvalues(100, 1e-10);

        // Compute von Neumann entropy: S = -Σ λᵢ log(λᵢ)
        let mut entropy = 0.0;
        for ev in eigenvalues {
            let lambda = ev.re.max(0.0); // Eigenvalues should be non-negative
            if lambda > constants::EPSILON {
                entropy -= lambda * lambda.ln();
            }
        }

        entropy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qubit_basics() {
        let q0 = Qubit::zero();
        assert!((q0.prob_zero() - 1.0).abs() < 1e-10);
        assert!(q0.prob_one() < 1e-10);

        let q1 = Qubit::one();
        assert!(q1.prob_zero() < 1e-10);
        assert!((q1.prob_one() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_qubit_plus_minus() {
        let plus = Qubit::plus();
        assert!((plus.prob_zero() - 0.5).abs() < 1e-10);
        assert!((plus.prob_one() - 0.5).abs() < 1e-10);

        let minus = Qubit::minus();
        assert!((minus.prob_zero() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_qubit_hadamard() {
        let q0 = Qubit::zero();
        let h_q0 = q0.hadamard();

        // H|0⟩ = |+⟩
        assert!((h_q0.prob_zero() - 0.5).abs() < 1e-10);
        assert!((h_q0.prob_one() - 0.5).abs() < 1e-10);

        // H²|0⟩ = |0⟩
        let hh_q0 = h_q0.hadamard();
        assert!((hh_q0.prob_zero() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_qubit_fidelity() {
        let q0 = Qubit::zero();
        let q1 = Qubit::one();
        let plus = Qubit::plus();

        // Orthogonal states have zero fidelity
        assert!(q0.fidelity(&q1) < 1e-10);

        // Same state has fidelity 1
        assert!((q0.fidelity(&q0) - 1.0).abs() < 1e-10);

        // |⟨0|+⟩|² = 0.5
        assert!((q0.fidelity(&plus) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_quantum_state_ground() {
        let state = QuantumState::ground_state(2);
        assert_eq!(state.dimension, 4);
        assert!((state.probability(0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_quantum_state_ghz() {
        let ghz = QuantumState::ghz_state(3);
        assert_eq!(ghz.dimension, 8);

        // GHZ state has 50% probability on |000⟩ and |111⟩
        assert!((ghz.probability(0) - 0.5).abs() < 1e-10);
        assert!((ghz.probability(7) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_quantum_state_tensor() {
        let q0 = QuantumState::basis_state(2, 0).unwrap();
        let q1 = QuantumState::basis_state(2, 1).unwrap();

        let product = q0.tensor(&q1);
        assert_eq!(product.dimension, 4);

        // |0⟩ ⊗ |1⟩ = |01⟩ (index 1 in 2-qubit system)
        assert!((product.probability(1) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_quantum_state_fidelity() {
        let s1 = QuantumState::ground_state(2);
        let s2 = QuantumState::uniform_superposition(2);

        // Ground state vs uniform superposition
        let fid = s1.fidelity(&s2).unwrap();
        assert!((fid - 0.25).abs() < 1e-10); // |⟨00|++++⟩|² = 1/4

        // Self-fidelity is 1
        assert!((s1.fidelity(&s1).unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_entanglement_entropy() {
        // Product state has zero entanglement entropy
        let product = QuantumState::ground_state(2);
        let entropy = product.entanglement_entropy(1);
        assert!(entropy < 1e-5);

        // Bell state has maximum entanglement entropy (log 2)
        let mut bell = QuantumState::basis_state(4, 0).unwrap();
        bell.amplitudes[0] = Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0);
        bell.amplitudes[3] = Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0);

        let bell_entropy = bell.entanglement_entropy(1);
        // Should be close to ln(2) ≈ 0.693
        assert!(bell_entropy > 0.5);
    }
}
