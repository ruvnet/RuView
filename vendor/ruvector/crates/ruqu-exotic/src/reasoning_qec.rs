//! # Reasoning QEC -- Quantum Error Correction for Reasoning Traces
//!
//! Treats reasoning steps like qubits. Each step is encoded as a quantum state
//! (high confidence = close to |0>, low confidence = rotated toward |1>).
//! Noise is injected to simulate reasoning errors, then a repetition-code-style
//! syndrome extraction detects when adjacent steps become incoherent.
//!
//! This provides **structural** reasoning integrity checks, not semantic ones.
//! The 1D repetition code uses:
//! - N data qubits (one per reasoning step)
//! - N-1 ancilla qubits (parity checks between adjacent steps)
//! - Total: 2N - 1 qubits (maximum N = 13 to stay within 25-qubit limit)

use ruqu_core::error::QuantumError;
use ruqu_core::gate::Gate;
use ruqu_core::state::QuantumState;
use ruqu_core::types::Complex;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// A single reasoning step encoded as a quantum state.
/// The step is either "valid" (close to |0>) or "flawed" (close to |1>).
#[derive(Debug, Clone)]
pub struct ReasoningStep {
    pub label: String,
    pub confidence: f64, // 0.0 = completely uncertain, 1.0 = fully confident
}

/// Configuration for reasoning QEC
pub struct ReasoningQecConfig {
    /// Number of reasoning steps (data qubits)
    pub num_steps: usize,
    /// Noise rate per step (probability of error per step)
    pub noise_rate: f64,
    /// Seed for reproducibility
    pub seed: Option<u64>,
}

/// Result of a reasoning QEC analysis
#[derive(Debug)]
pub struct ReasoningQecResult {
    /// Which steps had errors detected (indices)
    pub error_steps: Vec<usize>,
    /// Syndrome bits (one per stabilizer)
    pub syndrome: Vec<bool>,
    /// Whether the overall reasoning trace is decodable (correctable)
    pub is_decodable: bool,
    /// Fidelity of the reasoning trace after correction
    pub corrected_fidelity: f64,
    /// Number of steps total
    pub num_steps: usize,
}

/// A reasoning trace with QEC-style error detection.
///
/// Maps reasoning steps to a 1D repetition code:
/// - Each step is a data qubit
/// - Stabilizers check parity between adjacent steps
/// - If adjacent steps disagree (one flipped, one not), syndrome fires
///
/// This is simpler than a full surface code but captures the key idea:
/// structural detection of reasoning incoherence.
pub struct ReasoningTrace {
    steps: Vec<ReasoningStep>,
    state: QuantumState,
    config: ReasoningQecConfig,
}

impl ReasoningTrace {
    /// Create a new reasoning trace from steps.
    /// Each step's confidence maps to a rotation: high confidence = close to |0>.
    /// Total qubits = num_steps (data) + (num_steps - 1) (ancilla for parity checks)
    pub fn new(
        steps: Vec<ReasoningStep>,
        config: ReasoningQecConfig,
    ) -> Result<Self, QuantumError> {
        let num_steps = steps.len();

        if num_steps == 0 {
            return Err(QuantumError::CircuitError(
                "reasoning trace requires at least one step".into(),
            ));
        }

        // Total qubits: data (0..num_steps) + ancillas (num_steps..2*num_steps-1)
        let total_qubits = (2 * num_steps - 1) as u32;

        // Check qubit limit early (MAX_QUBITS = 25)
        if total_qubits > 25 {
            return Err(QuantumError::QubitLimitExceeded {
                requested: total_qubits,
                maximum: 25,
            });
        }

        let seed = config.seed.unwrap_or(42);
        let mut state = QuantumState::new_with_seed(total_qubits, seed)?;

        // Encode each step: rotate by angle based on confidence
        // confidence=1.0 -> |0> (no rotation), confidence=0.0 -> equal superposition (pi/2)
        for (i, step) in steps.iter().enumerate() {
            let angle = std::f64::consts::FRAC_PI_2 * (1.0 - step.confidence);
            if angle.abs() > 1e-15 {
                state.apply_gate(&Gate::Ry(i as u32, angle))?;
            }
        }

        Ok(Self {
            steps,
            state,
            config,
        })
    }

    /// Inject noise into the reasoning trace.
    /// Each step independently suffers a bit flip (X error) with probability noise_rate.
    pub fn inject_noise(&mut self) -> Result<(), QuantumError> {
        let seed = self.config.seed.unwrap_or(42).wrapping_add(12345);
        let mut rng = StdRng::seed_from_u64(seed);
        for i in 0..self.steps.len() {
            if rng.gen::<f64>() < self.config.noise_rate {
                self.state.apply_gate(&Gate::X(i as u32))?;
            }
        }
        Ok(())
    }

    /// Extract syndrome by checking parity between adjacent reasoning steps.
    /// Uses ancilla qubits to perform non-destructive parity measurement.
    /// Syndrome bit i fires if steps i and i+1 disagree (ZZ stabilizer = -1).
    pub fn extract_syndrome(&mut self) -> Result<Vec<bool>, QuantumError> {
        let num_steps = self.steps.len();
        let mut syndrome = Vec::with_capacity(num_steps.saturating_sub(1));

        for i in 0..(num_steps - 1) {
            let data1 = i as u32;
            let data2 = (i + 1) as u32;
            let ancilla = (num_steps + i) as u32;

            // Reset ancilla to |0>
            self.state.reset_qubit(ancilla)?;

            // CNOT from data1 to ancilla, CNOT from data2 to ancilla
            // Ancilla will be |1> if data1 != data2
            self.state.apply_gate(&Gate::CNOT(data1, ancilla))?;
            self.state.apply_gate(&Gate::CNOT(data2, ancilla))?;

            // Measure ancilla
            let outcome = self.state.measure(ancilla)?;
            syndrome.push(outcome.result);
        }

        Ok(syndrome)
    }

    /// Decode syndrome and attempt correction.
    /// Simple decoder: if syndrome\[i\] fires, flip step i+1 (rightmost error assumption).
    pub fn decode_and_correct(&mut self, syndrome: &[bool]) -> Result<Vec<usize>, QuantumError> {
        let mut corrected = Vec::new();
        // Simple decoder: for each fired syndrome, the error is likely
        // between the two data qubits. Correct the right one.
        for (i, &fired) in syndrome.iter().enumerate() {
            if fired {
                let step_to_correct = i + 1;
                self.state.apply_gate(&Gate::X(step_to_correct as u32))?;
                corrected.push(step_to_correct);
            }
        }
        Ok(corrected)
    }

    /// Run the full QEC pipeline: inject noise, extract syndrome, decode, correct.
    pub fn run_qec(&mut self) -> Result<ReasoningQecResult, QuantumError> {
        // Save state before noise for fidelity comparison
        let clean_sv: Vec<Complex> = self.state.state_vector().to_vec();
        let clean_state = QuantumState::from_amplitudes(clean_sv, self.state.num_qubits())?;

        // Inject noise
        self.inject_noise()?;

        // Extract syndrome
        let syndrome = self.extract_syndrome()?;

        // Determine which steps have errors
        let mut error_steps = Vec::new();
        for (i, &s) in syndrome.iter().enumerate() {
            if s {
                error_steps.push(i + 1);
            }
        }

        let is_decodable = error_steps.len() <= self.steps.len() / 2;

        // Attempt correction
        if is_decodable {
            self.decode_and_correct(&syndrome)?;
        }

        let corrected_fidelity = self.state.fidelity(&clean_state);

        Ok(ReasoningQecResult {
            error_steps,
            syndrome,
            is_decodable,
            corrected_fidelity,
            num_steps: self.steps.len(),
        })
    }

    /// Get the number of reasoning steps
    pub fn num_steps(&self) -> usize {
        self.steps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_steps(n: usize, confidence: f64) -> Vec<ReasoningStep> {
        (0..n)
            .map(|i| ReasoningStep {
                label: format!("step_{}", i),
                confidence,
            })
            .collect()
    }

    #[test]
    fn test_new_creates_trace() {
        let steps = make_steps(5, 1.0);
        let config = ReasoningQecConfig {
            num_steps: 5,
            noise_rate: 0.0,
            seed: Some(42),
        };
        let trace = ReasoningTrace::new(steps, config);
        assert!(trace.is_ok());
        assert_eq!(trace.unwrap().num_steps(), 5);
    }

    #[test]
    fn test_empty_steps_rejected() {
        let config = ReasoningQecConfig {
            num_steps: 0,
            noise_rate: 0.0,
            seed: Some(42),
        };
        let result = ReasoningTrace::new(vec![], config);
        assert!(result.is_err());
    }

    #[test]
    fn test_qubit_limit_exceeded() {
        // 14 steps -> 2*14-1 = 27 qubits > 25
        let steps = make_steps(14, 1.0);
        let config = ReasoningQecConfig {
            num_steps: 14,
            noise_rate: 0.0,
            seed: Some(42),
        };
        let result = ReasoningTrace::new(steps, config);
        assert!(result.is_err());
    }

    #[test]
    fn test_max_allowed_steps() {
        // 13 steps -> 2*13-1 = 25 qubits = MAX_QUBITS (should succeed)
        let steps = make_steps(13, 1.0);
        let config = ReasoningQecConfig {
            num_steps: 13,
            noise_rate: 0.0,
            seed: Some(42),
        };
        let result = ReasoningTrace::new(steps, config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_no_noise_no_syndrome() {
        let steps = make_steps(5, 1.0);
        let config = ReasoningQecConfig {
            num_steps: 5,
            noise_rate: 0.0,
            seed: Some(42),
        };
        let mut trace = ReasoningTrace::new(steps, config).unwrap();
        let syndrome = trace.extract_syndrome().unwrap();
        // All steps fully confident (|0>) and no noise: parity checks should not fire
        assert!(syndrome.iter().all(|&s| !s));
    }

    #[test]
    fn test_run_qec_zero_noise() {
        let steps = make_steps(5, 1.0);
        let config = ReasoningQecConfig {
            num_steps: 5,
            noise_rate: 0.0,
            seed: Some(42),
        };
        let mut trace = ReasoningTrace::new(steps, config).unwrap();
        let result = trace.run_qec().unwrap();
        assert!(result.error_steps.is_empty());
        assert!(result.is_decodable);
    }

    #[test]
    fn test_run_qec_with_noise() {
        let steps = make_steps(5, 1.0);
        let config = ReasoningQecConfig {
            num_steps: 5,
            noise_rate: 0.5,
            seed: Some(100),
        };
        let mut trace = ReasoningTrace::new(steps, config).unwrap();
        let result = trace.run_qec().unwrap();
        assert_eq!(result.num_steps, 5);
        // Syndrome length = num_steps - 1
        assert_eq!(result.syndrome.len(), 4);
    }

    #[test]
    fn test_single_step_trace() {
        let steps = make_steps(1, 0.8);
        let config = ReasoningQecConfig {
            num_steps: 1,
            noise_rate: 0.0,
            seed: Some(42),
        };
        let mut trace = ReasoningTrace::new(steps, config).unwrap();
        let syndrome = trace.extract_syndrome().unwrap();
        // Single step -> no parity checks -> empty syndrome
        assert!(syndrome.is_empty());
    }

    #[test]
    fn test_partial_confidence_encoding() {
        // Steps with 50% confidence should produce superposition states
        let steps = make_steps(3, 0.5);
        let config = ReasoningQecConfig {
            num_steps: 3,
            noise_rate: 0.0,
            seed: Some(42),
        };
        let trace = ReasoningTrace::new(steps, config).unwrap();
        // State should not be purely |000...0>
        let probs = trace.state.probabilities();
        assert!(probs[0] < 1.0);
    }
}
