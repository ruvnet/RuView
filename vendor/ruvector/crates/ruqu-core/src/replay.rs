/// Deterministic replay engine for quantum simulation reproducibility.
///
/// Captures all parameters that affect simulation output (circuit structure,
/// seed, noise model, shots) into an [`ExecutionRecord`] so that any run can
/// be replayed bit-for-bit. Also provides [`StateCheckpoint`] for snapshotting
/// the raw amplitude vector mid-simulation.
use crate::circuit::QuantumCircuit;
use crate::gate::Gate;
use crate::simulator::{SimConfig, Simulator};
use crate::types::{Complex, NoiseModel};

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// NoiseConfig (serialisable snapshot of a NoiseModel)
// ---------------------------------------------------------------------------

/// Snapshot of a noise model configuration suitable for storage and replay.
#[derive(Debug, Clone, PartialEq)]
pub struct NoiseConfig {
    pub depolarizing_rate: f64,
    pub bit_flip_rate: f64,
    pub phase_flip_rate: f64,
}

impl NoiseConfig {
    /// Create a `NoiseConfig` from the simulator's [`NoiseModel`].
    pub fn from_noise_model(m: &NoiseModel) -> Self {
        Self {
            depolarizing_rate: m.depolarizing_rate,
            bit_flip_rate: m.bit_flip_rate,
            phase_flip_rate: m.phase_flip_rate,
        }
    }

    /// Convert back to a [`NoiseModel`] for replay.
    pub fn to_noise_model(&self) -> NoiseModel {
        NoiseModel {
            depolarizing_rate: self.depolarizing_rate,
            bit_flip_rate: self.bit_flip_rate,
            phase_flip_rate: self.phase_flip_rate,
        }
    }
}

// ---------------------------------------------------------------------------
// ExecutionRecord
// ---------------------------------------------------------------------------

/// Complete record of every parameter that can influence simulation output.
///
/// Two runs with the same `ExecutionRecord` and the same circuit must produce
/// identical measurement outcomes (assuming deterministic seeding).
#[derive(Debug, Clone)]
pub struct ExecutionRecord {
    /// Deterministic hash of the circuit structure (gate types, parameters,
    /// qubit indices). Computed via [`ReplayEngine::circuit_hash`].
    pub circuit_hash: [u8; 32],
    /// RNG seed used for measurement sampling and noise channels.
    pub seed: u64,
    /// Backend identifier string (e.g. `"state_vector"`).
    pub backend: String,
    /// Noise model parameters, if noise was enabled.
    pub noise_config: Option<NoiseConfig>,
    /// Number of measurement shots.
    pub shots: u32,
    /// Software version that produced this record.
    pub software_version: String,
    /// UTC timestamp (seconds since UNIX epoch) when the record was created.
    pub timestamp_utc: u64,
}

// ---------------------------------------------------------------------------
// ReplayEngine
// ---------------------------------------------------------------------------

/// Engine that records execution parameters and replays simulations for
/// reproducibility verification.
pub struct ReplayEngine {
    /// Software version embedded in every record.
    version: String,
}

impl ReplayEngine {
    /// Create a new `ReplayEngine` using the crate version from `Cargo.toml`.
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Capture all parameters needed to deterministically replay a simulation.
    ///
    /// The returned [`ExecutionRecord`] is self-contained: given the same
    /// circuit, the record holds enough information to reproduce the exact
    /// measurement outcomes.
    pub fn record_execution(
        &self,
        circuit: &QuantumCircuit,
        config: &SimConfig,
        shots: u32,
    ) -> ExecutionRecord {
        let seed = config.seed.unwrap_or(0);
        let noise_config = config.noise.as_ref().map(NoiseConfig::from_noise_model);

        let timestamp_utc = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        ExecutionRecord {
            circuit_hash: Self::circuit_hash(circuit),
            seed,
            backend: "state_vector".to_string(),
            noise_config,
            shots,
            software_version: self.version.clone(),
            timestamp_utc,
        }
    }

    /// Replay a simulation using the parameters in `record` and verify that
    /// the measurement outcomes match a fresh run.
    ///
    /// Returns `true` when the replayed results are identical to a reference
    /// run seeded with the same parameters. Both runs use the exact same seed
    /// so the RNG sequences must agree.
    pub fn replay(&self, record: &ExecutionRecord, circuit: &QuantumCircuit) -> bool {
        // Verify circuit hash matches the record.
        let current_hash = Self::circuit_hash(circuit);
        if current_hash != record.circuit_hash {
            return false;
        }

        let noise = record
            .noise_config
            .as_ref()
            .map(NoiseConfig::to_noise_model);

        let config = SimConfig {
            seed: Some(record.seed),
            noise: noise.clone(),
            shots: None,
        };

        // Run twice with the same config and compare measurements.
        let run_a = Simulator::run_with_config(circuit, &config);
        let config_b = SimConfig {
            seed: Some(record.seed),
            noise,
            shots: None,
        };
        let run_b = Simulator::run_with_config(circuit, &config_b);

        match (run_a, run_b) {
            (Ok(a), Ok(b)) => {
                if a.measurements.len() != b.measurements.len() {
                    return false;
                }
                a.measurements
                    .iter()
                    .zip(b.measurements.iter())
                    .all(|(ma, mb)| {
                        ma.qubit == mb.qubit
                            && ma.result == mb.result
                            && (ma.probability - mb.probability).abs() < 1e-12
                    })
            }
            _ => false,
        }
    }

    /// Compute a deterministic 32-byte hash of a circuit's structure.
    ///
    /// The hash captures, for every gate: its type discriminant, the qubit
    /// indices it acts on, and any continuous parameters (rotation angles).
    /// Two circuits with the same gate sequence produce the same hash.
    ///
    /// Uses `DefaultHasher` (SipHash-based) run twice with different seeds to
    /// fill 32 bytes.
    pub fn circuit_hash(circuit: &QuantumCircuit) -> [u8; 32] {
        // Build a canonical byte representation of the circuit.
        let canonical = Self::circuit_canonical_bytes(circuit);

        let mut result = [0u8; 32];

        // First 8 bytes: hash with seed 0.
        let h0 = hash_bytes_with_seed(&canonical, 0);
        result[0..8].copy_from_slice(&h0.to_le_bytes());

        // Next 8 bytes: hash with seed 1.
        let h1 = hash_bytes_with_seed(&canonical, 1);
        result[8..16].copy_from_slice(&h1.to_le_bytes());

        // Next 8 bytes: hash with seed 2.
        let h2 = hash_bytes_with_seed(&canonical, 2);
        result[16..24].copy_from_slice(&h2.to_le_bytes());

        // Final 8 bytes: hash with seed 3.
        let h3 = hash_bytes_with_seed(&canonical, 3);
        result[24..32].copy_from_slice(&h3.to_le_bytes());

        result
    }

    /// Serialise the circuit into a canonical byte sequence.
    ///
    /// The encoding is: `[num_qubits:4 bytes LE]` followed by, for each gate,
    /// `[discriminant:1 byte][qubit indices][f64 parameters as LE bytes]`.
    fn circuit_canonical_bytes(circuit: &QuantumCircuit) -> Vec<u8> {
        let mut buf = Vec::new();

        // Circuit metadata.
        buf.extend_from_slice(&circuit.num_qubits().to_le_bytes());

        for gate in circuit.gates() {
            // Push a discriminant byte for the gate variant.
            let (disc, qubits, params) = gate_components(gate);
            buf.push(disc);

            for q in &qubits {
                buf.extend_from_slice(&q.to_le_bytes());
            }
            for p in &params {
                buf.extend_from_slice(&p.to_le_bytes());
            }
        }

        buf
    }
}

impl Default for ReplayEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// StateCheckpoint
// ---------------------------------------------------------------------------

/// Snapshot of a quantum state-vector that can be serialised and restored.
///
/// The internal representation stores amplitudes as interleaved `(re, im)` f64
/// pairs in little-endian byte order so that the checkpoint is
/// platform-independent.
#[derive(Debug, Clone)]
pub struct StateCheckpoint {
    data: Vec<u8>,
    num_amplitudes: usize,
}

impl StateCheckpoint {
    /// Capture the current state-vector amplitudes into a checkpoint.
    pub fn capture(amplitudes: &[Complex]) -> Self {
        let mut data = Vec::with_capacity(amplitudes.len() * 16);
        for amp in amplitudes {
            data.extend_from_slice(&amp.re.to_le_bytes());
            data.extend_from_slice(&amp.im.to_le_bytes());
        }
        Self {
            data,
            num_amplitudes: amplitudes.len(),
        }
    }

    /// Restore the amplitudes from this checkpoint.
    pub fn restore(&self) -> Vec<Complex> {
        let mut amps = Vec::with_capacity(self.num_amplitudes);
        for i in 0..self.num_amplitudes {
            let offset = i * 16;
            let re = f64::from_le_bytes(
                self.data[offset..offset + 8]
                    .try_into()
                    .expect("checkpoint data corrupted"),
            );
            let im = f64::from_le_bytes(
                self.data[offset + 8..offset + 16]
                    .try_into()
                    .expect("checkpoint data corrupted"),
            );
            amps.push(Complex::new(re, im));
        }
        amps
    }

    /// Total size of the serialised checkpoint in bytes.
    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Hash a byte slice using `DefaultHasher` seeded deterministically.
///
/// `DefaultHasher` does not expose a seed parameter so we prepend the seed
/// bytes to the data to obtain different digests for different seeds.
fn hash_bytes_with_seed(data: &[u8], seed: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    data.hash(&mut hasher);
    hasher.finish()
}

/// Decompose a `Gate` into a discriminant byte, qubit indices, and f64
/// parameters. This is the single source of truth for the canonical encoding.
fn gate_components(gate: &Gate) -> (u8, Vec<u32>, Vec<f64>) {
    match gate {
        Gate::H(q) => (0, vec![*q], vec![]),
        Gate::X(q) => (1, vec![*q], vec![]),
        Gate::Y(q) => (2, vec![*q], vec![]),
        Gate::Z(q) => (3, vec![*q], vec![]),
        Gate::S(q) => (4, vec![*q], vec![]),
        Gate::Sdg(q) => (5, vec![*q], vec![]),
        Gate::T(q) => (6, vec![*q], vec![]),
        Gate::Tdg(q) => (7, vec![*q], vec![]),
        Gate::Rx(q, angle) => (8, vec![*q], vec![*angle]),
        Gate::Ry(q, angle) => (9, vec![*q], vec![*angle]),
        Gate::Rz(q, angle) => (10, vec![*q], vec![*angle]),
        Gate::Phase(q, angle) => (11, vec![*q], vec![*angle]),
        Gate::CNOT(c, t) => (12, vec![*c, *t], vec![]),
        Gate::CZ(a, b) => (13, vec![*a, *b], vec![]),
        Gate::SWAP(a, b) => (14, vec![*a, *b], vec![]),
        Gate::Rzz(a, b, angle) => (15, vec![*a, *b], vec![*angle]),
        Gate::Measure(q) => (16, vec![*q], vec![]),
        Gate::Reset(q) => (17, vec![*q], vec![]),
        Gate::Barrier => (18, vec![], vec![]),
        Gate::Unitary1Q(q, m) => {
            // Encode the 4 complex entries (8 f64 values).
            let params = vec![
                m[0][0].re, m[0][0].im, m[0][1].re, m[0][1].im, m[1][0].re, m[1][0].im, m[1][1].re,
                m[1][1].im,
            ];
            (19, vec![*q], params)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::QuantumCircuit;
    use crate::simulator::SimConfig;
    use crate::types::Complex;

    /// Same seed produces identical measurement results.
    #[test]
    fn same_seed_identical_results() {
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0).cnot(0, 1).measure(0).measure(1);

        let config = SimConfig {
            seed: Some(42),
            noise: None,
            shots: None,
        };

        let r1 = Simulator::run_with_config(&circuit, &config).unwrap();
        let r2 = Simulator::run_with_config(&circuit, &config).unwrap();

        assert_eq!(r1.measurements.len(), r2.measurements.len());
        for (a, b) in r1.measurements.iter().zip(r2.measurements.iter()) {
            assert_eq!(a.qubit, b.qubit);
            assert_eq!(a.result, b.result);
            assert!((a.probability - b.probability).abs() < 1e-12);
        }
    }

    /// Different seeds produce different results (probabilistically; with
    /// measurements on a Bell state the chance of accidental agreement is
    /// non-zero but small over many runs).
    #[test]
    fn different_seed_different_results() {
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0).cnot(0, 1).measure(0).measure(1);

        let mut any_differ = false;
        // Try several seed pairs to reduce flakiness.
        for offset in 0..20 {
            let c1 = SimConfig {
                seed: Some(100 + offset),
                noise: None,
                shots: None,
            };
            let c2 = SimConfig {
                seed: Some(200 + offset),
                noise: None,
                shots: None,
            };
            let r1 = Simulator::run_with_config(&circuit, &c1).unwrap();
            let r2 = Simulator::run_with_config(&circuit, &c2).unwrap();
            if r1
                .measurements
                .iter()
                .zip(r2.measurements.iter())
                .any(|(a, b)| a.result != b.result)
            {
                any_differ = true;
                break;
            }
        }
        assert!(
            any_differ,
            "expected at least one pair of seeds to disagree"
        );
    }

    /// Record + replay round-trip succeeds.
    #[test]
    fn record_replay_roundtrip() {
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0).cnot(0, 1).measure(0).measure(1);

        let config = SimConfig {
            seed: Some(99),
            noise: None,
            shots: None,
        };

        let engine = ReplayEngine::new();
        let record = engine.record_execution(&circuit, &config, 1);

        assert!(engine.replay(&record, &circuit));
    }

    /// Circuit hash is deterministic: calling it twice yields the same value.
    #[test]
    fn circuit_hash_deterministic() {
        let mut circuit = QuantumCircuit::new(3);
        circuit.h(0).rx(1, 1.234).cnot(0, 2).measure(0);

        let h1 = ReplayEngine::circuit_hash(&circuit);
        let h2 = ReplayEngine::circuit_hash(&circuit);
        assert_eq!(h1, h2);
    }

    /// Two structurally different circuits produce different hashes.
    #[test]
    fn circuit_hash_differs_for_different_circuits() {
        let mut c1 = QuantumCircuit::new(2);
        c1.h(0).cnot(0, 1);

        let mut c2 = QuantumCircuit::new(2);
        c2.x(0).cnot(0, 1);

        let h1 = ReplayEngine::circuit_hash(&c1);
        let h2 = ReplayEngine::circuit_hash(&c2);
        assert_ne!(h1, h2);
    }

    /// Checkpoint capture/restore preserves amplitudes exactly.
    #[test]
    fn checkpoint_capture_restore() {
        let amplitudes = vec![
            Complex::new(0.5, 0.5),
            Complex::new(-0.3, 0.1),
            Complex::new(0.0, -0.7),
            Complex::new(0.2, 0.0),
        ];

        let checkpoint = StateCheckpoint::capture(&amplitudes);
        let restored = checkpoint.restore();

        assert_eq!(amplitudes.len(), restored.len());
        for (orig, rest) in amplitudes.iter().zip(restored.iter()) {
            assert_eq!(orig.re, rest.re);
            assert_eq!(orig.im, rest.im);
        }
    }

    /// Checkpoint size is 16 bytes per amplitude (re: 8 + im: 8).
    #[test]
    fn checkpoint_size_bytes() {
        let amplitudes = vec![Complex::ZERO; 8];
        let checkpoint = StateCheckpoint::capture(&amplitudes);
        assert_eq!(checkpoint.size_bytes(), 8 * 16);
    }

    /// Replay fails if the circuit has been modified after recording.
    #[test]
    fn replay_fails_on_modified_circuit() {
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0).cnot(0, 1).measure(0).measure(1);

        let config = SimConfig {
            seed: Some(42),
            noise: None,
            shots: None,
        };

        let engine = ReplayEngine::new();
        let record = engine.record_execution(&circuit, &config, 1);

        // Modify the circuit.
        let mut modified = QuantumCircuit::new(2);
        modified.x(0).cnot(0, 1).measure(0).measure(1);

        assert!(!engine.replay(&record, &modified));
    }

    /// ExecutionRecord captures noise config when present.
    #[test]
    fn record_captures_noise() {
        let circuit = QuantumCircuit::new(1);
        let config = SimConfig {
            seed: Some(7),
            noise: Some(NoiseModel {
                depolarizing_rate: 0.01,
                bit_flip_rate: 0.005,
                phase_flip_rate: 0.002,
            }),
            shots: None,
        };

        let engine = ReplayEngine::new();
        let record = engine.record_execution(&circuit, &config, 100);

        let nc = record.noise_config.as_ref().unwrap();
        assert!((nc.depolarizing_rate - 0.01).abs() < 1e-15);
        assert!((nc.bit_flip_rate - 0.005).abs() < 1e-15);
        assert!((nc.phase_flip_rate - 0.002).abs() < 1e-15);
        assert_eq!(record.shots, 100);
        assert_eq!(record.seed, 7);
    }

    /// Empty circuit hashes deterministically and differently from non-empty.
    #[test]
    fn empty_circuit_hash() {
        let empty = QuantumCircuit::new(2);
        let mut non_empty = QuantumCircuit::new(2);
        non_empty.h(0);

        let h1 = ReplayEngine::circuit_hash(&empty);
        let h2 = ReplayEngine::circuit_hash(&non_empty);
        assert_ne!(h1, h2);

        // Determinism.
        assert_eq!(h1, ReplayEngine::circuit_hash(&empty));
    }

    /// Rotation angle differences produce different hashes.
    #[test]
    fn rotation_angle_changes_hash() {
        let mut c1 = QuantumCircuit::new(1);
        c1.rx(0, 1.0);

        let mut c2 = QuantumCircuit::new(1);
        c2.rx(0, 1.0001);

        assert_ne!(
            ReplayEngine::circuit_hash(&c1),
            ReplayEngine::circuit_hash(&c2)
        );
    }
}
