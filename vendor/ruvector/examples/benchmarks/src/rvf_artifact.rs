//! RVF Artifact Packaging
//!
//! Packages an intelligence experiment as a self-contained, reproducible artifact.
//! Aligns with the "identical graded outcomes, not identical tokens" promise.
//!
//! ## Contents
//!
//! 1. **Manifest**: Engine version, pinned configs, seed set, holdout IDs
//! 2. **Memory Snapshot**: ReasoningBank serialized, KnowledgeCompiler cache, promotion log
//! 3. **Graders**: Deterministic scoring + ContractHealth evaluation
//! 4. **Witness Chain**: Per-episode input/config/grade/memory hashes
//!
//! ## Run Modes
//!
//! - **Replay**: Uses stored tasks, stored grades, verifies witness chain
//! - **Verify**: Regenerates tasks from seeds, reruns grader, must match grades exactly

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::agi_contract::ContractHealth;
use crate::reasoning_bank::{MemoryClass, RollbackWitness};

// ═══════════════════════════════════════════════════════════════════════════
// Manifest
// ═══════════════════════════════════════════════════════════════════════════

/// RVF Artifact Manifest — top-level metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RvfManifest {
    /// Format version
    pub rvf_version: String,
    /// Engine version that produced this artifact
    pub engine_version: String,
    /// Pinned solver configuration
    pub solver_config: SolverConfig,
    /// Pinned generator configuration
    pub generator_config: GeneratorConfig,
    /// Seed set used for generation
    pub seed_set: SeedSet,
    /// Holdout puzzle IDs (frozen set)
    pub holdout_ids: Vec<String>,
    /// Number of training cycles
    pub cycles: usize,
    /// Creation timestamp
    pub created_at: String,
    /// SHA-256 of the full artifact (computed after serialization)
    pub artifact_hash: Option<String>,
}

/// Pinned solver configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolverConfig {
    /// Step budget per task
    pub step_budget: usize,
    /// Noise injection rate
    pub noise_rate: f64,
    /// Retry enabled
    pub retry_enabled: bool,
    /// Beam width
    pub beam_width: usize,
    /// Minimum accuracy threshold
    pub min_accuracy: f64,
}

/// Pinned generator configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneratorConfig {
    /// Min difficulty
    pub min_difficulty: u8,
    /// Max difficulty
    pub max_difficulty: u8,
    /// Constraint density
    pub constraint_density: usize,
    /// Domain type (e.g., "temporal_puzzles", "program_synthesis")
    pub domain: String,
}

/// Seed set for deterministic replay.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeedSet {
    /// Holdout generation seed (frozen)
    pub holdout_seed: u64,
    /// Training base seed
    pub training_seed: u64,
    /// Noise RNG seed
    pub noise_seed: u64,
}

// ═══════════════════════════════════════════════════════════════════════════
// Memory Snapshot
// ═══════════════════════════════════════════════════════════════════════════

/// Serialized memory state at a point in time.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemorySnapshot {
    /// Serialized ReasoningBank (bincode or JSON)
    pub reasoning_bank_data: Vec<u8>,
    /// KnowledgeCompiler cache entries
    pub compiler_cache: Vec<CompiledEntry>,
    /// Promotion log: patterns promoted during this experiment
    pub promotion_log: Vec<PromotionRecord>,
    /// Memory class summary
    pub class_summary: MemoryClassSummary,
}

/// A compiled knowledge entry (from KnowledgeCompiler).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledEntry {
    /// Constraint signature
    pub signature: String,
    /// Compiled solution
    pub solution: String,
    /// Max steps the compiled path takes
    pub max_steps: usize,
    /// Confidence in compiled solution
    pub confidence: f64,
    /// Number of times this entry was used
    pub hit_count: usize,
}

/// Record of a pattern promotion.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromotionRecord {
    /// Constraint type
    pub constraint_type: String,
    /// Strategy name
    pub strategy: String,
    /// From class
    pub from_class: String,
    /// To class
    pub to_class: String,
    /// Number of observations at promotion time
    pub observations: usize,
    /// Number of counterexamples at promotion time
    pub counterexamples: usize,
    /// Cycle when promotion occurred
    pub cycle: usize,
}

/// Summary of memory classes.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MemoryClassSummary {
    pub volatile: usize,
    pub trusted: usize,
    pub quarantined: usize,
    pub total_counterexamples: usize,
    pub total_rollback_witnesses: usize,
}

// ═══════════════════════════════════════════════════════════════════════════
// Witness Chain
// ═══════════════════════════════════════════════════════════════════════════

/// Per-episode witness record for auditability.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WitnessRecord {
    /// Episode/cycle number
    pub episode: usize,
    /// SHA-256 of input (puzzle set)
    pub input_hash: String,
    /// SHA-256 of config
    pub config_hash: String,
    /// SHA-256 of grade outputs
    pub grade_hash: String,
    /// Memory root hash before this episode
    pub memory_root_before: String,
    /// Memory root hash after this episode
    pub memory_root_after: String,
    /// Gate decisions hash
    pub gate_decisions_hash: String,
    /// Contract health at end of episode
    pub contract_health: ContractHealth,
}

/// Complete witness chain for the experiment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WitnessChain {
    /// Ordered witness records (one per cycle)
    pub records: Vec<WitnessRecord>,
    /// Rollback witnesses that occurred during the experiment
    pub rollback_witnesses: Vec<RollbackWitness>,
    /// Final combined hash of the entire chain
    pub chain_hash: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// RVF Artifact (top-level)
// ═══════════════════════════════════════════════════════════════════════════

/// Complete RVF artifact — everything needed to replay or verify an experiment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RvfArtifact {
    /// Manifest with pinned configuration
    pub manifest: RvfManifest,
    /// Memory snapshot
    pub memory: MemorySnapshot,
    /// Witness chain
    pub witness_chain: WitnessChain,
    /// Final contract health
    pub final_health: ContractHealth,
    /// Final IQ score
    pub final_iq: f64,
}

/// Run mode for artifact verification.
#[derive(Clone, Debug, PartialEq)]
pub enum RunMode {
    /// Use stored tasks, stored grades, verify witness chain
    Replay,
    /// Regenerate tasks from seeds, rerun grader, grades must match
    Verify,
}

// ═══════════════════════════════════════════════════════════════════════════
// Builder
// ═══════════════════════════════════════════════════════════════════════════

/// Builder for assembling an RVF artifact from experiment results.
pub struct RvfArtifactBuilder {
    manifest: Option<RvfManifest>,
    memory: Option<MemorySnapshot>,
    witness_records: Vec<WitnessRecord>,
    rollback_witnesses: Vec<RollbackWitness>,
    final_health: Option<ContractHealth>,
    final_iq: f64,
}

impl RvfArtifactBuilder {
    pub fn new() -> Self {
        Self {
            manifest: None,
            memory: None,
            witness_records: Vec::new(),
            rollback_witnesses: Vec::new(),
            final_health: None,
            final_iq: 0.0,
        }
    }

    pub fn manifest(mut self, manifest: RvfManifest) -> Self {
        self.manifest = Some(manifest);
        self
    }

    pub fn memory(mut self, memory: MemorySnapshot) -> Self {
        self.memory = Some(memory);
        self
    }

    pub fn add_witness(&mut self, record: WitnessRecord) {
        self.witness_records.push(record);
    }

    pub fn add_rollback_witness(&mut self, witness: RollbackWitness) {
        self.rollback_witnesses.push(witness);
    }

    pub fn final_health(mut self, health: ContractHealth) -> Self {
        self.final_health = Some(health);
        self
    }

    pub fn final_iq(mut self, iq: f64) -> Self {
        self.final_iq = iq;
        self
    }

    /// Build the artifact. Returns None if required fields are missing.
    pub fn build(self) -> Option<RvfArtifact> {
        let manifest = self.manifest?;
        let memory = self.memory?;
        let final_health = self.final_health?;

        Some(RvfArtifact {
            manifest,
            memory,
            witness_chain: WitnessChain {
                records: self.witness_records,
                rollback_witnesses: self.rollback_witnesses,
                chain_hash: None,
            },
            final_health,
            final_iq: self.final_iq,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Hash utilities (simple deterministic hashing for witness chain)
// ═══════════════════════════════════════════════════════════════════════════

/// Simple deterministic hash for reproducibility checks.
/// Uses a 64-bit FNV-1a hash displayed as hex.
pub fn fnv_hash(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

/// Hash a serializable value.
pub fn hash_value<T: Serialize>(value: &T) -> String {
    let json = serde_json::to_vec(value).unwrap_or_default();
    fnv_hash(&json)
}

// ═══════════════════════════════════════════════════════════════════════════
// Verification
// ═══════════════════════════════════════════════════════════════════════════

/// Result of artifact verification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Overall pass/fail
    pub passed: bool,
    /// Per-witness verification
    pub witness_checks: Vec<WitnessCheck>,
    /// Number of hash mismatches
    pub mismatches: usize,
    /// Chain integrity (each record references previous hash)
    pub chain_intact: bool,
}

/// Single witness check result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WitnessCheck {
    pub episode: usize,
    pub input_hash_ok: bool,
    pub grade_hash_ok: bool,
    pub memory_transition_ok: bool,
}

/// Verify an artifact's witness chain integrity.
pub fn verify_witness_chain(artifact: &RvfArtifact) -> VerificationResult {
    let mut checks = Vec::new();
    let mut mismatches = 0;
    let mut chain_intact = true;

    let mut prev_memory_after = String::new();

    for (i, record) in artifact.witness_chain.records.iter().enumerate() {
        let input_ok = !record.input_hash.is_empty();
        let grade_ok = !record.grade_hash.is_empty();

        // Memory transition: after(N-1) == before(N)
        let memory_ok = if i == 0 {
            true
        } else {
            record.memory_root_before == prev_memory_after
        };

        if !memory_ok {
            chain_intact = false;
            mismatches += 1;
        }
        if !input_ok {
            mismatches += 1;
        }
        if !grade_ok {
            mismatches += 1;
        }

        prev_memory_after = record.memory_root_after.clone();

        checks.push(WitnessCheck {
            episode: record.episode,
            input_hash_ok: input_ok,
            grade_hash_ok: grade_ok,
            memory_transition_ok: memory_ok,
        });
    }

    VerificationResult {
        passed: mismatches == 0 && chain_intact,
        witness_checks: checks,
        mismatches,
        chain_intact,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_hash_deterministic() {
        let h1 = fnv_hash(b"hello world");
        let h2 = fnv_hash(b"hello world");
        assert_eq!(h1, h2);

        let h3 = fnv_hash(b"hello world!");
        assert_ne!(h1, h3);
    }

    #[test]
    fn artifact_builder_works() {
        let manifest = RvfManifest {
            rvf_version: "1.0".to_string(),
            engine_version: "0.1.0".to_string(),
            solver_config: SolverConfig {
                step_budget: 400,
                noise_rate: 0.25,
                retry_enabled: true,
                beam_width: 3,
                min_accuracy: 0.80,
            },
            generator_config: GeneratorConfig {
                min_difficulty: 1,
                max_difficulty: 10,
                constraint_density: 3,
                domain: "temporal_puzzles".to_string(),
            },
            seed_set: SeedSet {
                holdout_seed: 0xDEAD_BEEF,
                training_seed: 42,
                noise_seed: 31337,
            },
            holdout_ids: vec!["p1".into(), "p2".into()],
            cycles: 10,
            created_at: "2026-02-15T00:00:00Z".to_string(),
            artifact_hash: None,
        };

        let memory = MemorySnapshot {
            reasoning_bank_data: vec![1, 2, 3],
            compiler_cache: Vec::new(),
            promotion_log: Vec::new(),
            class_summary: MemoryClassSummary::default(),
        };

        let health = ContractHealth {
            solved_per_cost: 0.85,
            noise_stability: 0.92,
            contradiction_rate: 0.01,
            rollback_correctness: 1.0,
            policy_violations: 0,
            accuracy: 0.95,
            cost_efficiency: 0.85,
            compliant: true,
        };

        let artifact = RvfArtifactBuilder::new()
            .manifest(manifest)
            .memory(memory)
            .final_health(health)
            .final_iq(95.0)
            .build();

        assert!(artifact.is_some());
        let a = artifact.unwrap();
        assert_eq!(a.manifest.rvf_version, "1.0");
        assert_eq!(a.final_iq, 95.0);
        assert!(a.final_health.compliant);
    }

    #[test]
    fn witness_chain_verification() {
        let mut builder = RvfArtifactBuilder::new();

        // Build a 3-episode witness chain with consistent memory transitions
        let mem_root_0 = fnv_hash(b"initial");
        let mem_root_1 = fnv_hash(b"after_cycle_1");
        let mem_root_2 = fnv_hash(b"after_cycle_2");
        let mem_root_3 = fnv_hash(b"after_cycle_3");

        let health = ContractHealth {
            solved_per_cost: 0.9,
            noise_stability: 0.95,
            contradiction_rate: 0.0,
            rollback_correctness: 1.0,
            policy_violations: 0,
            accuracy: 0.95,
            cost_efficiency: 0.90,
            compliant: true,
        };

        builder.add_witness(WitnessRecord {
            episode: 0,
            input_hash: fnv_hash(b"input_0"),
            config_hash: fnv_hash(b"config"),
            grade_hash: fnv_hash(b"grade_0"),
            memory_root_before: mem_root_0.clone(),
            memory_root_after: mem_root_1.clone(),
            gate_decisions_hash: fnv_hash(b"gates_0"),
            contract_health: health.clone(),
        });

        builder.add_witness(WitnessRecord {
            episode: 1,
            input_hash: fnv_hash(b"input_1"),
            config_hash: fnv_hash(b"config"),
            grade_hash: fnv_hash(b"grade_1"),
            memory_root_before: mem_root_1.clone(), // matches prev after
            memory_root_after: mem_root_2.clone(),
            gate_decisions_hash: fnv_hash(b"gates_1"),
            contract_health: health.clone(),
        });

        builder.add_witness(WitnessRecord {
            episode: 2,
            input_hash: fnv_hash(b"input_2"),
            config_hash: fnv_hash(b"config"),
            grade_hash: fnv_hash(b"grade_2"),
            memory_root_before: mem_root_2.clone(), // matches prev after
            memory_root_after: mem_root_3.clone(),
            gate_decisions_hash: fnv_hash(b"gates_2"),
            contract_health: health.clone(),
        });

        let manifest = RvfManifest {
            rvf_version: "1.0".to_string(),
            engine_version: "0.1.0".to_string(),
            solver_config: SolverConfig {
                step_budget: 400,
                noise_rate: 0.25,
                retry_enabled: true,
                beam_width: 3,
                min_accuracy: 0.80,
            },
            generator_config: GeneratorConfig {
                min_difficulty: 1,
                max_difficulty: 10,
                constraint_density: 3,
                domain: "temporal_puzzles".to_string(),
            },
            seed_set: SeedSet {
                holdout_seed: 0xDEAD_BEEF,
                training_seed: 42,
                noise_seed: 31337,
            },
            holdout_ids: Vec::new(),
            cycles: 3,
            created_at: "2026-02-15T00:00:00Z".to_string(),
            artifact_hash: None,
        };

        let artifact = RvfArtifactBuilder::new()
            .manifest(manifest)
            .memory(MemorySnapshot {
                reasoning_bank_data: Vec::new(),
                compiler_cache: Vec::new(),
                promotion_log: Vec::new(),
                class_summary: MemoryClassSummary::default(),
            })
            .final_health(health)
            .final_iq(90.0);

        // Transfer witnesses
        let mut artifact_raw = artifact.build().unwrap();
        artifact_raw.witness_chain.records = builder.witness_records;

        let result = verify_witness_chain(&artifact_raw);
        assert!(result.passed);
        assert!(result.chain_intact);
        assert_eq!(result.mismatches, 0);
        assert_eq!(result.witness_checks.len(), 3);
    }

    #[test]
    fn witness_chain_detects_tampering() {
        let health = ContractHealth {
            solved_per_cost: 0.9,
            noise_stability: 0.95,
            contradiction_rate: 0.0,
            rollback_correctness: 1.0,
            policy_violations: 0,
            accuracy: 0.95,
            cost_efficiency: 0.90,
            compliant: true,
        };

        let mut artifact = RvfArtifact {
            manifest: RvfManifest {
                rvf_version: "1.0".to_string(),
                engine_version: "0.1.0".to_string(),
                solver_config: SolverConfig {
                    step_budget: 400,
                    noise_rate: 0.25,
                    retry_enabled: true,
                    beam_width: 3,
                    min_accuracy: 0.80,
                },
                generator_config: GeneratorConfig {
                    min_difficulty: 1,
                    max_difficulty: 10,
                    constraint_density: 3,
                    domain: "temporal_puzzles".to_string(),
                },
                seed_set: SeedSet {
                    holdout_seed: 0xDEAD_BEEF,
                    training_seed: 42,
                    noise_seed: 31337,
                },
                holdout_ids: Vec::new(),
                cycles: 2,
                created_at: "2026-02-15T00:00:00Z".to_string(),
                artifact_hash: None,
            },
            memory: MemorySnapshot {
                reasoning_bank_data: Vec::new(),
                compiler_cache: Vec::new(),
                promotion_log: Vec::new(),
                class_summary: MemoryClassSummary::default(),
            },
            witness_chain: WitnessChain {
                records: vec![
                    WitnessRecord {
                        episode: 0,
                        input_hash: fnv_hash(b"in_0"),
                        config_hash: fnv_hash(b"cfg"),
                        grade_hash: fnv_hash(b"gr_0"),
                        memory_root_before: fnv_hash(b"mem_0"),
                        memory_root_after: fnv_hash(b"mem_1"),
                        gate_decisions_hash: fnv_hash(b"g_0"),
                        contract_health: health.clone(),
                    },
                    WitnessRecord {
                        episode: 1,
                        input_hash: fnv_hash(b"in_1"),
                        config_hash: fnv_hash(b"cfg"),
                        grade_hash: fnv_hash(b"gr_1"),
                        // TAMPERED: memory_root_before doesn't match previous after
                        memory_root_before: fnv_hash(b"WRONG"),
                        memory_root_after: fnv_hash(b"mem_2"),
                        gate_decisions_hash: fnv_hash(b"g_1"),
                        contract_health: health.clone(),
                    },
                ],
                rollback_witnesses: Vec::new(),
                chain_hash: None,
            },
            final_health: health,
            final_iq: 90.0,
        };

        let result = verify_witness_chain(&artifact);
        assert!(!result.passed);
        assert!(!result.chain_intact);
        assert!(result.mismatches > 0);
    }
}
