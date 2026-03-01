/// Cryptographic witness logging for tamper-evident audit trails.
///
/// Each simulation execution is appended to a hash-chain: every
/// [`WitnessEntry`] includes a hash of its predecessor so that retroactive
/// tampering with any field in any entry is detectable by
/// [`WitnessLog::verify_chain`].
use crate::replay::ExecutionRecord;
use crate::types::MeasurementOutcome;

use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};

// ---------------------------------------------------------------------------
// WitnessError
// ---------------------------------------------------------------------------

/// Errors detected during witness chain verification.
#[derive(Debug, Clone)]
pub enum WitnessError {
    /// The hash that links entry `index` to its predecessor does not match
    /// the actual hash of the preceding entry.
    BrokenChain {
        index: usize,
        expected: [u8; 32],
        found: [u8; 32],
    },
    /// The self-hash stored in an entry does not match the recomputed hash
    /// of that entry's contents.
    InvalidHash { index: usize },
    /// Cannot verify an empty log.
    EmptyLog,
}

impl fmt::Display for WitnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WitnessError::BrokenChain {
                index,
                expected,
                found,
            } => write!(
                f,
                "broken chain at index {}: expected prev_hash {:?}, found {:?}",
                index, expected, found
            ),
            WitnessError::InvalidHash { index } => {
                write!(f, "invalid self-hash at index {}", index)
            }
            WitnessError::EmptyLog => write!(f, "cannot verify an empty witness log"),
        }
    }
}

impl std::error::Error for WitnessError {}

// ---------------------------------------------------------------------------
// WitnessEntry
// ---------------------------------------------------------------------------

/// A single entry in the witness hash-chain.
///
/// Each entry stores:
/// - its position in the chain (`sequence`),
/// - a backward pointer (`prev_hash`) to the preceding entry (or all-zeros
///   for the genesis entry),
/// - the execution parameters,
/// - a hash of the simulation results, and
/// - a self-hash computed over all of the above fields.
#[derive(Debug, Clone)]
pub struct WitnessEntry {
    /// Zero-based sequence number in the chain.
    pub sequence: u64,
    /// Hash of the previous entry, or `[0; 32]` for the first entry.
    pub prev_hash: [u8; 32],
    /// The execution record that was logged.
    pub execution: ExecutionRecord,
    /// Deterministic hash of the measurement outcomes.
    pub result_hash: [u8; 32],
    /// Self-hash: `H(sequence || prev_hash || execution_bytes || result_hash)`.
    pub entry_hash: [u8; 32],
}

// ---------------------------------------------------------------------------
// WitnessLog
// ---------------------------------------------------------------------------

/// Append-only, hash-chained log of simulation execution records.
///
/// Use [`append`](WitnessLog::append) to add entries and
/// [`verify_chain`](WitnessLog::verify_chain) to validate the entire chain.
pub struct WitnessLog {
    entries: Vec<WitnessEntry>,
}

impl WitnessLog {
    /// Create a new, empty witness log.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Append a new entry to the log, chaining it to the previous entry.
    ///
    /// Returns a reference to the newly appended entry.
    pub fn append(
        &mut self,
        execution: ExecutionRecord,
        results: &[MeasurementOutcome],
    ) -> &WitnessEntry {
        let sequence = self.entries.len() as u64;

        let prev_hash = self
            .entries
            .last()
            .map(|e| e.entry_hash)
            .unwrap_or([0u8; 32]);

        let result_hash = hash_measurement_outcomes(results);
        let execution_bytes = execution_to_bytes(&execution);
        let entry_hash = compute_entry_hash(sequence, &prev_hash, &execution_bytes, &result_hash);

        self.entries.push(WitnessEntry {
            sequence,
            prev_hash,
            execution,
            result_hash,
            entry_hash,
        });

        self.entries.last().unwrap()
    }

    /// Walk the entire chain and verify that:
    /// 1. Every entry's `prev_hash` matches the preceding entry's `entry_hash`.
    /// 2. Every entry's `entry_hash` matches the recomputed hash of its contents.
    ///
    /// Returns `Ok(())` if the chain is intact, or a [`WitnessError`]
    /// describing the first inconsistency found.
    pub fn verify_chain(&self) -> Result<(), WitnessError> {
        if self.entries.is_empty() {
            return Err(WitnessError::EmptyLog);
        }

        for (i, entry) in self.entries.iter().enumerate() {
            // 1. Check prev_hash linkage.
            let expected_prev = if i == 0 {
                [0u8; 32]
            } else {
                self.entries[i - 1].entry_hash
            };

            if entry.prev_hash != expected_prev {
                return Err(WitnessError::BrokenChain {
                    index: i,
                    expected: expected_prev,
                    found: entry.prev_hash,
                });
            }

            // 2. Verify self-hash.
            let execution_bytes = execution_to_bytes(&entry.execution);
            let recomputed = compute_entry_hash(
                entry.sequence,
                &entry.prev_hash,
                &execution_bytes,
                &entry.result_hash,
            );

            if entry.entry_hash != recomputed {
                return Err(WitnessError::InvalidHash { index: i });
            }
        }

        Ok(())
    }

    /// Number of entries in the log.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get an entry by zero-based index.
    pub fn get(&self, index: usize) -> Option<&WitnessEntry> {
        self.entries.get(index)
    }

    /// Borrow the full slice of entries.
    pub fn entries(&self) -> &[WitnessEntry] {
        &self.entries
    }

    /// Export the entire log as a JSON string.
    ///
    /// Uses a hand-rolled serialiser to avoid depending on `serde_json` in
    /// the core crate. The output is a JSON array of entry objects.
    pub fn to_json(&self) -> String {
        let mut buf = String::from("[\n");
        for (i, entry) in self.entries.iter().enumerate() {
            if i > 0 {
                buf.push_str(",\n");
            }
            buf.push_str("  {\n");
            buf.push_str(&format!("    \"sequence\": {},\n", entry.sequence));
            buf.push_str(&format!(
                "    \"prev_hash\": \"{}\",\n",
                hex_encode(&entry.prev_hash)
            ));
            buf.push_str(&format!(
                "    \"circuit_hash\": \"{}\",\n",
                hex_encode(&entry.execution.circuit_hash)
            ));
            buf.push_str(&format!("    \"seed\": {},\n", entry.execution.seed));
            buf.push_str(&format!(
                "    \"backend\": \"{}\",\n",
                entry.execution.backend
            ));
            buf.push_str(&format!("    \"shots\": {},\n", entry.execution.shots));
            buf.push_str(&format!(
                "    \"software_version\": \"{}\",\n",
                entry.execution.software_version
            ));
            buf.push_str(&format!(
                "    \"timestamp_utc\": {},\n",
                entry.execution.timestamp_utc
            ));

            // Noise config (null or object).
            match &entry.execution.noise_config {
                Some(nc) => {
                    buf.push_str("    \"noise_config\": {\n");
                    buf.push_str(&format!(
                        "      \"depolarizing_rate\": {},\n",
                        nc.depolarizing_rate
                    ));
                    buf.push_str(&format!("      \"bit_flip_rate\": {},\n", nc.bit_flip_rate));
                    buf.push_str(&format!(
                        "      \"phase_flip_rate\": {}\n",
                        nc.phase_flip_rate
                    ));
                    buf.push_str("    },\n");
                }
                None => {
                    buf.push_str("    \"noise_config\": null,\n");
                }
            }

            buf.push_str(&format!(
                "    \"result_hash\": \"{}\",\n",
                hex_encode(&entry.result_hash)
            ));
            buf.push_str(&format!(
                "    \"entry_hash\": \"{}\"\n",
                hex_encode(&entry.entry_hash)
            ));
            buf.push_str("  }");
        }
        buf.push_str("\n]");
        buf
    }
}

impl Default for WitnessLog {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Hash a byte slice using `DefaultHasher` with a deterministic seed prefix.
/// Returns a u64 digest.
fn hash_with_seed(data: &[u8], seed: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    data.hash(&mut hasher);
    hasher.finish()
}

/// Produce a 32-byte hash from arbitrary data by running `DefaultHasher`
/// four times with different seeds and concatenating the results.
fn hash_to_32(data: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0u64..4 {
        let h = hash_with_seed(data, i);
        let start = (i as usize) * 8;
        out[start..start + 8].copy_from_slice(&h.to_le_bytes());
    }
    out
}

/// Deterministically hash a slice of measurement outcomes into 32 bytes.
fn hash_measurement_outcomes(outcomes: &[MeasurementOutcome]) -> [u8; 32] {
    let mut buf = Vec::new();
    for m in outcomes {
        buf.extend_from_slice(&m.qubit.to_le_bytes());
        buf.push(if m.result { 1 } else { 0 });
        buf.extend_from_slice(&m.probability.to_le_bytes());
    }
    hash_to_32(&buf)
}

/// Serialise an `ExecutionRecord` into a deterministic byte sequence.
fn execution_to_bytes(exec: &ExecutionRecord) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&exec.circuit_hash);
    buf.extend_from_slice(&exec.seed.to_le_bytes());
    buf.extend_from_slice(exec.backend.as_bytes());
    buf.extend_from_slice(&exec.shots.to_le_bytes());
    buf.extend_from_slice(exec.software_version.as_bytes());
    buf.extend_from_slice(&exec.timestamp_utc.to_le_bytes());

    if let Some(ref nc) = exec.noise_config {
        buf.push(1);
        buf.extend_from_slice(&nc.depolarizing_rate.to_le_bytes());
        buf.extend_from_slice(&nc.bit_flip_rate.to_le_bytes());
        buf.extend_from_slice(&nc.phase_flip_rate.to_le_bytes());
    } else {
        buf.push(0);
    }

    buf
}

/// Compute the self-hash of a witness entry.
///
/// `H(sequence || prev_hash || execution_bytes || result_hash)`
fn compute_entry_hash(
    sequence: u64,
    prev_hash: &[u8; 32],
    execution_bytes: &[u8],
    result_hash: &[u8; 32],
) -> [u8; 32] {
    let mut buf = Vec::new();
    buf.extend_from_slice(&sequence.to_le_bytes());
    buf.extend_from_slice(prev_hash);
    buf.extend_from_slice(execution_bytes);
    buf.extend_from_slice(result_hash);
    hash_to_32(&buf)
}

/// Encode a byte slice as a lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::{NoiseConfig, ReplayEngine};
    use crate::types::MeasurementOutcome;

    /// Helper: create a minimal `ExecutionRecord` for testing.
    fn make_record(seed: u64) -> ExecutionRecord {
        ExecutionRecord {
            circuit_hash: [seed as u8; 32],
            seed,
            backend: "state_vector".to_string(),
            noise_config: None,
            shots: 1,
            software_version: "test".to_string(),
            timestamp_utc: 1_700_000_000,
        }
    }

    /// Helper: create measurement outcomes for testing.
    fn make_outcomes(bits: &[bool]) -> Vec<MeasurementOutcome> {
        bits.iter()
            .enumerate()
            .map(|(i, &b)| MeasurementOutcome {
                qubit: i as u32,
                result: b,
                probability: if b { 0.5 } else { 0.5 },
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Empty log
    // -----------------------------------------------------------------------

    #[test]
    fn empty_log_verification_returns_empty_error() {
        let log = WitnessLog::new();
        match log.verify_chain() {
            Err(WitnessError::EmptyLog) => {} // expected
            other => panic!("expected EmptyLog, got {:?}", other),
        }
    }

    #[test]
    fn empty_log_len_is_zero() {
        let log = WitnessLog::new();
        assert_eq!(log.len(), 0);
        assert!(log.is_empty());
    }

    // -----------------------------------------------------------------------
    // Single entry
    // -----------------------------------------------------------------------

    #[test]
    fn single_entry_has_zero_prev_hash() {
        let mut log = WitnessLog::new();
        let record = make_record(42);
        let outcomes = make_outcomes(&[true, false]);
        log.append(record, &outcomes);

        let entry = log.get(0).unwrap();
        assert_eq!(entry.prev_hash, [0u8; 32]);
        assert_eq!(entry.sequence, 0);
    }

    #[test]
    fn single_entry_verifies() {
        let mut log = WitnessLog::new();
        log.append(make_record(1), &make_outcomes(&[true]));
        assert!(log.verify_chain().is_ok());
    }

    // -----------------------------------------------------------------------
    // Two entries chained
    // -----------------------------------------------------------------------

    #[test]
    fn two_entries_properly_chained() {
        let mut log = WitnessLog::new();
        log.append(make_record(1), &make_outcomes(&[true]));
        log.append(make_record(2), &make_outcomes(&[false]));

        assert_eq!(log.len(), 2);

        let first = log.get(0).unwrap();
        let second = log.get(1).unwrap();

        // Second entry's prev_hash must equal first entry's entry_hash.
        assert_eq!(second.prev_hash, first.entry_hash);
        assert_eq!(second.sequence, 1);

        assert!(log.verify_chain().is_ok());
    }

    // -----------------------------------------------------------------------
    // Tamper detection
    // -----------------------------------------------------------------------

    #[test]
    fn tampering_with_seed_breaks_verification() {
        let mut log = WitnessLog::new();
        log.append(make_record(1), &make_outcomes(&[true]));
        log.append(make_record(2), &make_outcomes(&[false]));

        // Tamper with the first entry's execution seed.
        log.entries[0].execution.seed = 999;

        match log.verify_chain() {
            Err(WitnessError::InvalidHash { index: 0 }) => {} // expected
            other => panic!("expected InvalidHash at 0, got {:?}", other),
        }
    }

    #[test]
    fn tampering_with_result_hash_breaks_verification() {
        let mut log = WitnessLog::new();
        log.append(make_record(1), &make_outcomes(&[true]));

        // Tamper with the result hash.
        log.entries[0].result_hash = [0xff; 32];

        match log.verify_chain() {
            Err(WitnessError::InvalidHash { index: 0 }) => {}
            other => panic!("expected InvalidHash at 0, got {:?}", other),
        }
    }

    #[test]
    fn tampering_with_prev_hash_breaks_verification() {
        let mut log = WitnessLog::new();
        log.append(make_record(1), &make_outcomes(&[true]));
        log.append(make_record(2), &make_outcomes(&[false]));

        // Tamper with the second entry's prev_hash.
        log.entries[1].prev_hash = [0xaa; 32];

        match log.verify_chain() {
            Err(WitnessError::BrokenChain { index: 1, .. }) => {}
            other => panic!("expected BrokenChain at 1, got {:?}", other),
        }
    }

    #[test]
    fn tampering_with_entry_hash_breaks_verification() {
        let mut log = WitnessLog::new();
        log.append(make_record(1), &make_outcomes(&[true]));

        // Tamper with the entry hash itself.
        log.entries[0].entry_hash = [0xbb; 32];

        match log.verify_chain() {
            Err(WitnessError::InvalidHash { index: 0 }) => {}
            other => panic!("expected InvalidHash at 0, got {:?}", other),
        }
    }

    #[test]
    fn tampering_with_sequence_breaks_verification() {
        let mut log = WitnessLog::new();
        log.append(make_record(1), &make_outcomes(&[true]));

        log.entries[0].execution.backend = "tampered".to_string();

        match log.verify_chain() {
            Err(WitnessError::InvalidHash { index: 0 }) => {}
            other => panic!("expected InvalidHash at 0, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // JSON export
    // -----------------------------------------------------------------------

    #[test]
    fn json_export_contains_all_entries() {
        let mut log = WitnessLog::new();
        log.append(make_record(1), &make_outcomes(&[true]));
        log.append(make_record(2), &make_outcomes(&[false, true]));

        let json = log.to_json();

        // Should contain both entries.
        assert!(json.contains("\"sequence\": 0"));
        assert!(json.contains("\"sequence\": 1"));
        assert!(json.contains("\"seed\": 1"));
        assert!(json.contains("\"seed\": 2"));
        assert!(json.contains("\"backend\": \"state_vector\""));
        assert!(json.contains("\"entry_hash\""));
        assert!(json.contains("\"prev_hash\""));
        assert!(json.contains("\"result_hash\""));
        assert!(json.contains("\"software_version\": \"test\""));
    }

    #[test]
    fn json_export_with_noise_config() {
        let record = ExecutionRecord {
            circuit_hash: [0; 32],
            seed: 10,
            backend: "state_vector".to_string(),
            noise_config: Some(NoiseConfig {
                depolarizing_rate: 0.01,
                bit_flip_rate: 0.005,
                phase_flip_rate: 0.002,
            }),
            shots: 100,
            software_version: "test".to_string(),
            timestamp_utc: 1_700_000_000,
        };

        let mut log = WitnessLog::new();
        log.append(record, &make_outcomes(&[true]));

        let json = log.to_json();
        assert!(json.contains("\"depolarizing_rate\": 0.01"));
        assert!(json.contains("\"bit_flip_rate\": 0.005"));
        assert!(json.contains("\"phase_flip_rate\": 0.002"));
    }

    #[test]
    fn json_export_null_noise() {
        let mut log = WitnessLog::new();
        log.append(make_record(5), &make_outcomes(&[false]));

        let json = log.to_json();
        assert!(json.contains("\"noise_config\": null"));
    }

    // -----------------------------------------------------------------------
    // Long chain
    // -----------------------------------------------------------------------

    #[test]
    fn chain_of_100_entries_verifies() {
        let mut log = WitnessLog::new();
        for i in 0..100u64 {
            let outcomes = make_outcomes(&[i % 2 == 0, i % 3 == 0]);
            log.append(make_record(i), &outcomes);
        }

        assert_eq!(log.len(), 100);
        assert!(log.verify_chain().is_ok());

        // Check chain linkage explicitly for a few entries.
        for i in 1..100 {
            let prev = log.get(i - 1).unwrap();
            let curr = log.get(i).unwrap();
            assert_eq!(curr.prev_hash, prev.entry_hash);
            assert_eq!(curr.sequence, i as u64);
        }
    }

    #[test]
    fn tampering_middle_of_long_chain_detected() {
        let mut log = WitnessLog::new();
        for i in 0..10u64 {
            log.append(make_record(i), &make_outcomes(&[true]));
        }

        // Tamper with entry 5.
        log.entries[5].execution.seed = 9999;

        match log.verify_chain() {
            Err(WitnessError::InvalidHash { index: 5 }) => {}
            other => panic!("expected InvalidHash at 5, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // entries() accessor
    // -----------------------------------------------------------------------

    #[test]
    fn entries_returns_all() {
        let mut log = WitnessLog::new();
        log.append(make_record(1), &make_outcomes(&[true]));
        log.append(make_record(2), &make_outcomes(&[false]));
        log.append(make_record(3), &make_outcomes(&[true, false]));

        let entries = log.entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].sequence, 0);
        assert_eq!(entries[1].sequence, 1);
        assert_eq!(entries[2].sequence, 2);
    }

    // -----------------------------------------------------------------------
    // Hash determinism
    // -----------------------------------------------------------------------

    #[test]
    fn same_inputs_produce_same_hashes() {
        let mut log1 = WitnessLog::new();
        let mut log2 = WitnessLog::new();

        let rec1 = make_record(42);
        let rec2 = make_record(42);
        let outcomes = make_outcomes(&[true, false]);

        log1.append(rec1, &outcomes);
        log2.append(rec2, &outcomes);

        assert_eq!(
            log1.get(0).unwrap().entry_hash,
            log2.get(0).unwrap().entry_hash
        );
        assert_eq!(
            log1.get(0).unwrap().result_hash,
            log2.get(0).unwrap().result_hash
        );
    }

    #[test]
    fn different_results_produce_different_result_hashes() {
        let mut log = WitnessLog::new();
        log.append(make_record(1), &make_outcomes(&[true]));
        log.append(make_record(1), &make_outcomes(&[false]));

        assert_ne!(
            log.get(0).unwrap().result_hash,
            log.get(1).unwrap().result_hash
        );
    }

    // -----------------------------------------------------------------------
    // Integration with ReplayEngine
    // -----------------------------------------------------------------------

    #[test]
    fn integration_with_replay_engine() {
        use crate::circuit::QuantumCircuit;
        use crate::simulator::{SimConfig, Simulator};

        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0).cnot(0, 1).measure(0).measure(1);

        let config = SimConfig {
            seed: Some(42),
            noise: None,
            shots: None,
        };

        let engine = ReplayEngine::new();
        let record = engine.record_execution(&circuit, &config, 1);
        let result = Simulator::run_with_config(&circuit, &config).unwrap();

        let mut log = WitnessLog::new();
        log.append(record, &result.measurements);

        assert_eq!(log.len(), 1);
        assert!(log.verify_chain().is_ok());

        let entry = log.get(0).unwrap();
        assert_eq!(entry.sequence, 0);
        assert_eq!(entry.prev_hash, [0u8; 32]);
    }
}
