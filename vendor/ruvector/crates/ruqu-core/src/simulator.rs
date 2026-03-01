//! High-level simulator that executes quantum circuits

use crate::circuit::QuantumCircuit;
use crate::error::Result;
use crate::gate::Gate;
use crate::state::QuantumState;
use crate::types::*;

use rand::Rng;
use std::collections::HashMap;
use std::time::Instant;

/// Configuration for a simulation run.
pub struct SimConfig {
    /// Deterministic seed. `None` uses OS entropy.
    pub seed: Option<u64>,
    /// Optional noise model applied after every gate.
    pub noise: Option<NoiseModel>,
    /// Number of repeated shots (`None` = single run returning state).
    pub shots: Option<u32>,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            seed: None,
            noise: None,
            shots: None,
        }
    }
}

/// Result of a single simulation run (state + measurements).
pub struct SimulationResult {
    pub state: QuantumState,
    pub measurements: Vec<MeasurementOutcome>,
    pub metrics: SimulationMetrics,
}

/// Result of a multi-shot simulation (histogram of outcomes).
pub struct ShotResult {
    pub counts: HashMap<Vec<bool>, usize>,
    pub metrics: SimulationMetrics,
}

/// Stateless simulator entry-point.
pub struct Simulator;

impl Simulator {
    /// Run a circuit once with default configuration.
    pub fn run(circuit: &QuantumCircuit) -> Result<SimulationResult> {
        Self::run_with_config(circuit, &SimConfig::default())
    }

    /// Run a circuit once with explicit configuration.
    pub fn run_with_config(
        circuit: &QuantumCircuit,
        config: &SimConfig,
    ) -> Result<SimulationResult> {
        let start = Instant::now();

        let mut state = match config.seed {
            Some(seed) => QuantumState::new_with_seed(circuit.num_qubits(), seed)?,
            None => QuantumState::new(circuit.num_qubits())?,
        };

        let mut measurements = Vec::new();
        let mut gate_count: usize = 0;

        for gate in circuit.gates() {
            let outcomes = state.apply_gate(gate)?;
            measurements.extend(outcomes);
            if !gate.is_non_unitary() {
                gate_count += 1;
            }
            // Apply noise channel after each gate when a model is provided.
            if let Some(ref noise) = config.noise {
                apply_noise(&mut state, gate, noise);
            }
        }

        let elapsed = start.elapsed();
        let metrics = SimulationMetrics {
            num_qubits: circuit.num_qubits(),
            gate_count,
            execution_time_ns: elapsed.as_nanos() as u64,
            peak_memory_bytes: QuantumState::estimate_memory(circuit.num_qubits()),
            gates_per_second: if elapsed.as_secs_f64() > 0.0 {
                gate_count as f64 / elapsed.as_secs_f64()
            } else {
                0.0
            },
            gates_fused: 0,
        };

        Ok(SimulationResult {
            state,
            measurements,
            metrics,
        })
    }

    /// Run a circuit `shots` times, collecting a histogram of measurement outcomes.
    ///
    /// If the circuit contains no `Measure` gates, all qubits are measured
    /// automatically at the end of each shot.
    pub fn run_shots(
        circuit: &QuantumCircuit,
        shots: u32,
        seed: Option<u64>,
    ) -> Result<ShotResult> {
        let start = Instant::now();
        let mut counts: HashMap<Vec<bool>, usize> = HashMap::new();
        let base_seed = seed.unwrap_or(42);
        let mut total_gates: usize = 0;
        let n_qubits = circuit.num_qubits();

        let has_measurements = circuit
            .gates()
            .iter()
            .any(|g| matches!(g, Gate::Measure(_)));

        for shot in 0..shots {
            let config = SimConfig {
                seed: Some(base_seed.wrapping_add(shot as u64)),
                noise: None,
                shots: None,
            };

            let mut result = Self::run_with_config(circuit, &config)?;
            total_gates += result.metrics.gate_count;

            // Implicit measurement when the circuit has none.
            if !has_measurements {
                let outcomes = result.state.measure_all()?;
                result.measurements.extend(outcomes);
            }

            // Build a bit-vector keyed by qubit index.
            let mut bits = vec![false; n_qubits as usize];
            for m in &result.measurements {
                if (m.qubit as usize) < bits.len() {
                    bits[m.qubit as usize] = m.result;
                }
            }
            *counts.entry(bits).or_insert(0) += 1;
        }

        let elapsed = start.elapsed();
        let metrics = SimulationMetrics {
            num_qubits: n_qubits,
            gate_count: total_gates,
            execution_time_ns: elapsed.as_nanos() as u64,
            peak_memory_bytes: QuantumState::estimate_memory(n_qubits),
            gates_per_second: if elapsed.as_secs_f64() > 0.0 {
                total_gates as f64 / elapsed.as_secs_f64()
            } else {
                0.0
            },
            gates_fused: 0,
        };

        Ok(ShotResult { counts, metrics })
    }
}

// ---------------------------------------------------------------------------
// Noise channel
// ---------------------------------------------------------------------------

/// Apply a stochastic noise channel to the state after a gate.
///
/// For each qubit that the gate touches:
///   - with probability `depolarizing_rate`, apply a random Pauli (X, Y, or Z
///     each with probability 1/3);
///   - with probability `bit_flip_rate`, apply X;
///   - with probability `phase_flip_rate`, apply Z.
fn apply_noise(state: &mut QuantumState, gate: &Gate, noise: &NoiseModel) {
    let qubits = gate.qubits();
    if qubits.is_empty() {
        return;
    }

    for &qubit in &qubits {
        // Depolarising channel
        if noise.depolarizing_rate > 0.0 {
            let r: f64 = state.rng_mut().gen();
            if r < noise.depolarizing_rate {
                let choice: f64 = state.rng_mut().gen();
                let pauli = if choice < 1.0 / 3.0 {
                    Gate::X(qubit)
                } else if choice < 2.0 / 3.0 {
                    Gate::Y(qubit)
                } else {
                    Gate::Z(qubit)
                };
                if let Some(m) = pauli.matrix_1q() {
                    state.apply_single_qubit_gate(qubit, &m);
                }
            }
        }

        // Bit-flip channel
        if noise.bit_flip_rate > 0.0 {
            let r: f64 = state.rng_mut().gen();
            if r < noise.bit_flip_rate {
                let m = Gate::X(qubit).matrix_1q().unwrap();
                state.apply_single_qubit_gate(qubit, &m);
            }
        }

        // Phase-flip channel
        if noise.phase_flip_rate > 0.0 {
            let r: f64 = state.rng_mut().gen();
            if r < noise.phase_flip_rate {
                let m = Gate::Z(qubit).matrix_1q().unwrap();
                state.apply_single_qubit_gate(qubit, &m);
            }
        }
    }
}
