//! Grover's Search Algorithm
//!
//! Provides a quadratic speedup for **unstructured search**: given an oracle
//! that marks M target states out of N = 2^n total states, Grover's algorithm
//! finds a marked state with high probability in O(sqrt(N/M)) queries.
//!
//! # Implementation strategy
//!
//! Because this is a *simulation* library (not a hardware backend), the oracle
//! and diffusion operator are implemented via **direct state-vector
//! manipulation** through [`QuantumState::amplitudes_mut`]. This gives O(M)
//! oracle cost and O(N) diffuser cost per iteration -- far cheaper than
//! decomposing a general multi-controlled-Z into elementary gates.
//!
//! Single-qubit Hadamard gates are still applied through the normal gate
//! pipeline so that the simulator's bookkeeping (metrics, noise, etc.)
//! remains consistent.

use ruqu_core::gate::Gate;
use ruqu_core::state::QuantumState;
use ruqu_core::types::{Complex, QubitIndex};

// ---------------------------------------------------------------------------
// Configuration and result types
// ---------------------------------------------------------------------------

/// Configuration for Grover's search.
pub struct GroverConfig {
    /// Number of qubits (search space has 2^num_qubits states).
    pub num_qubits: u32,
    /// Indices of the marked (target) basis states. Each index must be in
    /// `0 .. 2^num_qubits`.
    pub target_states: Vec<usize>,
    /// Number of Grover iterations. When `None`, the theoretically optimal
    /// count is computed from [`optimal_iterations`].
    pub num_iterations: Option<u32>,
    /// Optional RNG seed forwarded to [`QuantumState::new_with_seed`].
    pub seed: Option<u64>,
}

/// Result of a Grover search run.
pub struct GroverResult {
    /// The basis-state index obtained by measuring all qubits.
    pub measured_state: usize,
    /// Whether `measured_state` is one of the target states.
    pub target_found: bool,
    /// Pre-measurement probability of observing *any* target state.
    pub success_probability: f64,
    /// Number of Grover iterations that were executed.
    pub num_iterations: u32,
    /// Post-measurement quantum state (collapsed).
    pub state: QuantumState,
}

// ---------------------------------------------------------------------------
// Optimal iteration count
// ---------------------------------------------------------------------------

/// Compute the theoretically optimal number of Grover iterations.
///
/// For N = 2^n states and M marked targets the optimal count is:
///
/// ```text
/// k = round( (pi / 4) * sqrt(N / M) - 0.5 )
/// ```
///
/// which maximizes the success probability (close to 1 when M << N).
/// Returns at least 1.
pub fn optimal_iterations(num_qubits: u32, num_targets: usize) -> u32 {
    let n = 1usize << num_qubits;
    let theta = (num_targets as f64 / n as f64).sqrt().asin();
    let k = (std::f64::consts::FRAC_PI_4 / theta - 0.5).round().max(1.0);
    k as u32
}

// ---------------------------------------------------------------------------
// Core algorithm
// ---------------------------------------------------------------------------

/// Run Grover's search algorithm.
///
/// # Algorithm outline
///
/// 1. Prepare equal superposition |s> = H^n |0>.
/// 2. Repeat for `num_iterations`:
///    a. **Oracle** -- negate the amplitude of every target state.
///    b. **Diffuser** -- reflect about |s>:
///       i.  Apply H on all qubits.
///       ii. Negate all amplitudes except the |0...0> component.
///       iii.Apply H on all qubits.
/// 3. Compute success probability from the final state.
/// 4. Measure all qubits to obtain a classical bitstring.
///
/// # Errors
///
/// Returns a [`ruqu_core::error::QuantumError`] if the qubit count exceeds
/// simulator limits or any gate application fails.
pub fn run_grover(config: &GroverConfig) -> ruqu_core::error::Result<GroverResult> {
    let n = config.num_qubits;
    let dim = 1usize << n;

    // Validate target indices.
    for &t in &config.target_states {
        assert!(
            t < dim,
            "target state index {} out of range for {} qubits (max {})",
            t,
            n,
            dim - 1,
        );
    }

    let iterations = config
        .num_iterations
        .unwrap_or_else(|| optimal_iterations(n, config.target_states.len()));

    // ----- Step 1: Initialize to equal superposition -----
    let mut state = match config.seed {
        Some(s) => QuantumState::new_with_seed(n, s)?,
        None => QuantumState::new(n)?,
    };
    for q in 0..n {
        state.apply_gate(&Gate::H(q))?;
    }

    // ----- Step 2: Grover iterations -----
    for _ in 0..iterations {
        // (a) Oracle: negate amplitudes of target states.
        {
            let amps = state.amplitudes_mut();
            for &target in &config.target_states {
                let a = amps[target];
                amps[target] = Complex {
                    re: -a.re,
                    im: -a.im,
                };
            }
        }

        // (b) Diffuser: 2|s><s| - I = H^n (2|0><0| - I) H^n
        //     (2|0><0| - I) keeps |0> unchanged and negates everything else.
        for q in 0..n {
            state.apply_gate(&Gate::H(q))?;
        }
        {
            let amps = state.amplitudes_mut();
            for i in 1..amps.len() {
                let a = amps[i];
                amps[i] = Complex {
                    re: -a.re,
                    im: -a.im,
                };
            }
        }
        for q in 0..n {
            state.apply_gate(&Gate::H(q))?;
        }
    }

    // ----- Step 3: Compute success probability before measurement -----
    let probs = state.probabilities();
    let success_probability: f64 = config.target_states.iter().map(|&t| probs[t]).sum();

    // ----- Step 4: Measure all qubits -----
    let measured = measure_all_qubits(&mut state, n)?;
    let target_found = config.target_states.contains(&measured);

    Ok(GroverResult {
        measured_state: measured,
        target_found,
        success_probability,
        num_iterations: iterations,
        state,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Measure every qubit (0 through `num_qubits - 1`) and assemble the results
/// into a single `usize` where bit `q` is 1 when qubit `q` measured |1>.
///
/// Measurements are performed in ascending qubit order. Each measurement
/// collapses the state, so subsequent outcomes are conditioned on earlier
/// ones. The joint distribution over all qubits matches `probabilities()`.
fn measure_all_qubits(
    state: &mut QuantumState,
    num_qubits: u32,
) -> ruqu_core::error::Result<usize> {
    let mut result: usize = 0;
    for q in 0..num_qubits {
        let outcome = state.measure(q as QubitIndex)?;
        if outcome.result {
            result |= 1 << q;
        }
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimal_iterations_single_target() {
        // N=8, M=1 -> k = round(pi/4 * sqrt(8) - 0.5) = round(1.72) = 2
        let k = optimal_iterations(3, 1);
        assert_eq!(k, 2);
    }

    #[test]
    fn test_optimal_iterations_half_marked() {
        // N=4, M=2 -> theta = asin(sqrt(0.5)) = pi/4
        // k = round(pi/4 / (pi/4) - 0.5) = round(0.5) = 1
        let k = optimal_iterations(2, 2);
        assert!(k >= 1);
    }

    #[test]
    fn test_optimal_iterations_minimum_one() {
        // Even pathological inputs should produce at least 1.
        let k = optimal_iterations(1, 1);
        assert!(k >= 1);
    }
}
