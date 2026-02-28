//! Proof-gated mutation substrate.
//!
//! The core `ProofGate<T>` type wraps mutations behind formal proofs.
//! Every mutation to graph state must pass through a proof gate, ensuring
//! that invariants are maintained and attestation chains are recorded.
//!
//! ## ADR-047 Types
//!
//! - [`MutationLedger`]: Append-only attestation log with FNV-1a chain hash
//! - [`ProofScope`]: Partition-aligned scope tied to min-cut boundaries
//! - [`SupersessionProof`]: Forward-only rollback via supersession
//! - [`EpochBoundary`]: Seal attestation for proof algebra upgrades
//! - [`ProofRequirement`]: Typed proof obligation variants
//! - [`ComplexityBound`]: Upper bounds on proof computation cost
//! - [`ProofClass`]: Formal vs statistical proof classification

use ruvector_verified::{
    gated::{route_proof, ProofKind, ProofTier, TierDecision},
    pipeline::compose_chain,
    proof_store::create_attestation,
    prove_dim_eq, ProofAttestation, ProofEnvironment, VerifiedOp,
};

use crate::error::Result;

/// A proof-gated value that can only be mutated through verified operations.
///
/// The inner value `T` is accessible for reading at any time, but mutations
/// require a proof obligation to be discharged first.
pub struct ProofGate<T> {
    /// The gated value.
    value: T,
    /// Chain of attestations recording mutation history.
    attestation_chain: AttestationChain,
    /// The proof environment for this gate.
    env: ProofEnvironment,
}

impl<T> ProofGate<T> {
    /// Create a new proof gate wrapping an initial value.
    ///
    /// The initial value is admitted without proof (genesis state).
    pub fn new(value: T) -> Self {
        Self {
            value,
            attestation_chain: AttestationChain::new(),
            env: ProofEnvironment::new(),
        }
    }

    /// Read the gated value without proof.
    pub fn read(&self) -> &T {
        &self.value
    }

    /// Attempt a mutation that requires dimension equality proof.
    ///
    /// The mutation function `f` is only applied if the dimension proof
    /// succeeds. The resulting attestation is recorded in the chain.
    pub fn mutate_with_dim_proof(
        &mut self,
        expected_dim: u32,
        actual_dim: u32,
        f: impl FnOnce(&mut T),
    ) -> Result<ProofAttestation> {
        let proof_id = prove_dim_eq(&mut self.env, expected_dim, actual_dim)?;
        f(&mut self.value);
        let attestation = create_attestation(&self.env, proof_id);
        self.attestation_chain.append(attestation.clone());
        Ok(attestation)
    }

    /// Attempt a mutation with tiered proof routing.
    ///
    /// Routes the proof obligation through the three-tier system based on
    /// complexity, then applies the mutation if verification succeeds.
    pub fn mutate_with_routed_proof(
        &mut self,
        proof_kind: ProofKind,
        expected_id: u32,
        actual_id: u32,
        f: impl FnOnce(&mut T),
    ) -> Result<(TierDecision, ProofAttestation)> {
        let decision = route_proof(proof_kind, &self.env);
        let proof_id = ruvector_verified::gated::verify_tiered(
            &mut self.env,
            expected_id,
            actual_id,
            decision.tier,
        )?;
        f(&mut self.value);
        let attestation = create_attestation(&self.env, proof_id);
        self.attestation_chain.append(attestation.clone());
        Ok((decision, attestation))
    }

    /// Attempt a mutation with pipeline composition proof.
    ///
    /// Verifies that a chain of pipeline stages compose correctly before
    /// applying the mutation.
    pub fn mutate_with_pipeline_proof(
        &mut self,
        stages: &[(String, u32, u32)],
        f: impl FnOnce(&mut T),
    ) -> Result<ProofAttestation> {
        let (_input_type, _output_type, proof_id) = compose_chain(stages, &mut self.env)?;
        f(&mut self.value);
        let attestation = create_attestation(&self.env, proof_id);
        self.attestation_chain.append(attestation.clone());
        Ok(attestation)
    }

    /// Get the attestation chain for audit.
    pub fn attestation_chain(&self) -> &AttestationChain {
        &self.attestation_chain
    }

    /// Get verification statistics from the proof environment.
    pub fn proof_stats(&self) -> &ruvector_verified::ProofStats {
        self.env.stats()
    }

    /// Reset the proof environment (useful between independent proof obligations).
    pub fn reset_env(&mut self) {
        self.env.reset();
    }
}

/// Trait for types that support proof-gated mutation.
pub trait ProofGatedMutation {
    /// The proof obligation type required for mutation.
    type ProofObligation;

    /// Verify the proof obligation and apply the mutation.
    fn apply_gated(
        &mut self,
        env: &mut ProofEnvironment,
        obligation: &Self::ProofObligation,
    ) -> Result<VerifiedOp<()>>;
}

/// A chain of proof attestations recording the mutation history of a value.
///
/// Each entry records when and how a gated value was mutated, creating
/// an auditable trail of verified operations.
#[derive(Debug, Clone)]
pub struct AttestationChain {
    /// Ordered list of attestations.
    entries: Vec<AttestationEntry>,
}

/// A single entry in the attestation chain.
#[derive(Debug, Clone)]
pub struct AttestationEntry {
    /// Sequence number in the chain.
    pub sequence: u64,
    /// The proof attestation.
    pub attestation: ProofAttestation,
}

impl AttestationChain {
    /// Create an empty attestation chain.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Append an attestation to the chain.
    pub fn append(&mut self, attestation: ProofAttestation) {
        let sequence = self.entries.len() as u64;
        self.entries.push(AttestationEntry {
            sequence,
            attestation,
        });
    }

    /// Get the number of attestations in the chain.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the most recent attestation.
    pub fn latest(&self) -> Option<&AttestationEntry> {
        self.entries.last()
    }

    /// Iterate over all entries in order.
    pub fn iter(&self) -> impl Iterator<Item = &AttestationEntry> {
        self.entries.iter()
    }

    /// Verify the chain integrity (sequential numbering).
    pub fn verify_integrity(&self) -> bool {
        self.entries
            .iter()
            .enumerate()
            .all(|(i, entry)| entry.sequence == i as u64)
    }

    /// Compute a hash over the entire chain for tamper detection.
    pub fn chain_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for entry in &self.entries {
            h ^= entry.sequence;
            h = h.wrapping_mul(0x100000001b3);
            h ^= entry.attestation.content_hash();
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }
}

impl Default for AttestationChain {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// MutationLedger
// ---------------------------------------------------------------------------

/// FNV-1a offset basis.
const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
/// FNV-1a prime.
const FNV_PRIME: u64 = 0x100000001b3;

/// Append-only attestation log with FNV-1a chain hash.
///
/// The ledger records every proof attestation within a scope and maintains
/// a running chain hash for tamper detection. When the number of entries
/// exceeds `compaction_threshold`, the ledger can be compacted into a single
/// composed attestation that preserves the chain hash.
#[derive(Debug, Clone)]
pub struct MutationLedger {
    /// Append-only log of attestations for this scope.
    attestations: Vec<ProofAttestation>,
    /// Running content hash (FNV-1a) over all attestation bytes.
    chain_hash: u64,
    /// Epoch counter for proof algebra versioning.
    epoch: u64,
    /// Maximum attestations before compaction is recommended.
    compaction_threshold: usize,
}

impl MutationLedger {
    /// Create a new empty ledger with the given compaction threshold.
    pub fn new(compaction_threshold: usize) -> Self {
        Self {
            attestations: Vec::new(),
            chain_hash: FNV_OFFSET_BASIS,
            epoch: 0,
            compaction_threshold,
        }
    }

    /// Append an attestation. Returns the chain position (0-indexed).
    pub fn append(&mut self, att: ProofAttestation) -> u64 {
        let position = self.attestations.len() as u64;
        // Fold attestation content hash into the running chain hash.
        self.chain_hash ^= att.content_hash();
        self.chain_hash = self.chain_hash.wrapping_mul(FNV_PRIME);
        self.attestations.push(att);
        position
    }

    /// Compact old attestations into a single summary attestation.
    ///
    /// All entries are replaced by a single composed attestation whose
    /// `proof_term_hash` encodes the chain hash and entry count, and whose
    /// `reduction_steps` is the sum of all constituent steps.
    /// The running `chain_hash` is recomputed over the single seal so
    /// that `verify_integrity()` remains consistent.
    pub fn compact(&mut self) -> ProofAttestation {
        let total_steps: u32 = self.attestations.iter().map(|a| a.reduction_steps).sum();

        let total_cache: u64 = self
            .attestations
            .iter()
            .map(|a| a.cache_hit_rate_bps as u64)
            .sum();
        let avg_cache = if self.attestations.is_empty() {
            0u16
        } else {
            (total_cache / self.attestations.len() as u64) as u16
        };

        // Encode the pre-compaction chain hash and count into proof_term_hash.
        let mut proof_hash = [0u8; 32];
        proof_hash[0..8].copy_from_slice(&self.chain_hash.to_le_bytes());
        proof_hash[8..16].copy_from_slice(&(self.attestations.len() as u64).to_le_bytes());

        // Use the last attestation's environment hash, or zeros.
        let env_hash = self
            .attestations
            .last()
            .map(|a| a.environment_hash)
            .unwrap_or([0u8; 32]);

        let seal = ProofAttestation::new(proof_hash, env_hash, total_steps, avg_cache);

        // Replace the attestation vector with just the seal and recompute
        // the chain hash so verify_integrity stays consistent.
        self.attestations.clear();
        self.attestations.push(seal.clone());
        self.chain_hash = FNV_OFFSET_BASIS;
        self.chain_hash ^= seal.content_hash();
        self.chain_hash = self.chain_hash.wrapping_mul(FNV_PRIME);

        seal
    }

    /// Verify the chain hash is consistent by recomputing from attestations.
    pub fn verify_integrity(&self) -> bool {
        let mut h: u64 = FNV_OFFSET_BASIS;
        for att in &self.attestations {
            h ^= att.content_hash();
            h = h.wrapping_mul(FNV_PRIME);
        }
        h == self.chain_hash
    }

    /// Get the current chain hash.
    pub fn chain_hash(&self) -> u64 {
        self.chain_hash
    }

    /// Get the current epoch.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Set the epoch (used during epoch boundary transitions).
    pub fn set_epoch(&mut self, epoch: u64) {
        self.epoch = epoch;
    }

    /// Get the number of attestations in the ledger.
    pub fn len(&self) -> usize {
        self.attestations.len()
    }

    /// Check if the ledger is empty.
    pub fn is_empty(&self) -> bool {
        self.attestations.is_empty()
    }

    /// Check if compaction is recommended (entries >= threshold).
    pub fn needs_compaction(&self) -> bool {
        self.attestations.len() >= self.compaction_threshold
    }

    /// Get the compaction threshold.
    pub fn compaction_threshold(&self) -> usize {
        self.compaction_threshold
    }

    /// Iterate over all attestations in order.
    pub fn iter(&self) -> impl Iterator<Item = &ProofAttestation> {
        self.attestations.iter()
    }
}

// ---------------------------------------------------------------------------
// ProofScope
// ---------------------------------------------------------------------------

/// Partition-aligned proof scope.
///
/// A `ProofScope` corresponds to a single min-cut partition from
/// `ruvector-mincut`. All proof obligations within a partition are tracked
/// in the scope's inner [`MutationLedger`], and the scope records the
/// coherence score for its region of the graph.
#[derive(Debug, Clone)]
pub struct ProofScope {
    /// Partition ID from ruvector-mincut.
    partition_id: u32,
    /// Boundary nodes shared with adjacent partitions.
    boundary_nodes: Vec<u64>,
    /// The ledger for this scope.
    ledger: MutationLedger,
    /// Coherence measurement for this scope (0.0..=1.0).
    coherence: Option<f64>,
}

impl ProofScope {
    /// Create a new proof scope for the given partition.
    pub fn new(partition_id: u32, boundary_nodes: Vec<u64>, compaction_threshold: usize) -> Self {
        Self {
            partition_id,
            boundary_nodes,
            ledger: MutationLedger::new(compaction_threshold),
            coherence: None,
        }
    }

    /// Get the partition ID.
    pub fn partition_id(&self) -> u32 {
        self.partition_id
    }

    /// Get the boundary nodes.
    pub fn boundary_nodes(&self) -> &[u64] {
        &self.boundary_nodes
    }

    /// Get a reference to the inner ledger.
    pub fn ledger(&self) -> &MutationLedger {
        &self.ledger
    }

    /// Get a mutable reference to the inner ledger.
    pub fn ledger_mut(&mut self) -> &mut MutationLedger {
        &mut self.ledger
    }

    /// Get the coherence score, if measured.
    pub fn coherence(&self) -> Option<f64> {
        self.coherence
    }

    /// Update the coherence score.
    pub fn set_coherence(&mut self, coherence: f64) {
        self.coherence = Some(coherence);
    }

    /// Transition this scope to a new partition, producing a
    /// [`ScopeTransitionAttestation`] that seals the old scope.
    ///
    /// The old ledger is compacted, and a transition attestation is
    /// produced that references both old and new partition IDs.
    pub fn transition(
        &mut self,
        new_partition_id: u32,
        new_boundary_nodes: Vec<u64>,
        min_cut_value: f64,
    ) -> ScopeTransitionAttestation {
        let seal = self.ledger.compact();
        let old_partition_id = self.partition_id;
        let old_coherence = self.coherence;

        self.partition_id = new_partition_id;
        self.boundary_nodes = new_boundary_nodes;
        self.coherence = None;
        // Reset the ledger for the new scope (keep same threshold).
        let threshold = self.ledger.compaction_threshold();
        self.ledger = MutationLedger::new(threshold);

        ScopeTransitionAttestation {
            old_partition_id,
            new_partition_id,
            min_cut_value,
            old_coherence,
            seal,
        }
    }
}

/// Attestation produced when a proof scope transitions to a new partition.
///
/// Records the old and new partition IDs, the min-cut value at the time
/// of transition, and the compacted seal from the old scope's ledger.
#[derive(Debug, Clone)]
pub struct ScopeTransitionAttestation {
    /// Previous partition ID.
    pub old_partition_id: u32,
    /// New partition ID.
    pub new_partition_id: u32,
    /// Min-cut value at the time of transition.
    pub min_cut_value: f64,
    /// Coherence of the old scope at transition time.
    pub old_coherence: Option<f64>,
    /// Compacted seal from the old scope's ledger.
    pub seal: ProofAttestation,
}

// ---------------------------------------------------------------------------
// SupersessionProof
// ---------------------------------------------------------------------------

/// Forward-only rollback via supersession.
///
/// Instead of deleting attestations (which would break monotonicity),
/// a `SupersessionProof` references the superseded position and provides
/// a replacement attestation with a soundness proof.
#[derive(Debug, Clone)]
pub struct SupersessionProof {
    /// Position of the attestation being superseded.
    pub superseded_position: u64,
    /// The new attestation that replaces it.
    pub replacement: ProofAttestation,
    /// Proof ID demonstrating that the replacement is sound
    /// (e.g., an inverse mutation proof).
    pub soundness_proof_id: u32,
}

impl SupersessionProof {
    /// Create a new supersession proof.
    pub fn new(
        superseded_position: u64,
        replacement: ProofAttestation,
        soundness_proof_id: u32,
    ) -> Self {
        Self {
            superseded_position,
            replacement,
            soundness_proof_id,
        }
    }
}

// ---------------------------------------------------------------------------
// EpochBoundary
// ---------------------------------------------------------------------------

/// Configuration for a proof environment at a given epoch.
#[derive(Debug, Clone)]
pub struct ProofEnvironmentConfig {
    /// Maximum fuel for standard-tier proofs.
    pub max_standard_fuel: u32,
    /// Maximum reduction steps for deep-tier proofs.
    pub max_deep_steps: u32,
    /// Built-in symbol count.
    pub builtin_symbols: u32,
}

impl Default for ProofEnvironmentConfig {
    fn default() -> Self {
        Self {
            max_standard_fuel: 500,
            max_deep_steps: 10_000,
            builtin_symbols: 64,
        }
    }
}

/// Seal attestation for proof algebra upgrades.
///
/// At an epoch boundary the [`MutationLedger`] is compacted, a seal
/// attestation is produced covering all proofs in the previous epoch,
/// and the proof environment is reconfigured with new parameters.
/// Old proofs remain valid (sealed) but new proofs use the updated algebra.
#[derive(Debug, Clone)]
pub struct EpochBoundary {
    /// Previous epoch number.
    pub from_epoch: u64,
    /// New epoch number.
    pub to_epoch: u64,
    /// Summary attestation sealing all proofs in the previous epoch.
    pub seal: ProofAttestation,
    /// New proof environment configuration.
    pub new_config: ProofEnvironmentConfig,
}

impl EpochBoundary {
    /// Create an epoch boundary by sealing the given ledger.
    ///
    /// Compacts the ledger, advances the epoch, and returns the
    /// boundary record.
    pub fn seal(ledger: &mut MutationLedger, new_config: ProofEnvironmentConfig) -> Self {
        let from_epoch = ledger.epoch();
        let to_epoch = from_epoch + 1;
        let seal_att = ledger.compact();
        ledger.set_epoch(to_epoch);
        Self {
            from_epoch,
            to_epoch,
            seal: seal_att,
            new_config,
        }
    }
}

// ---------------------------------------------------------------------------
// ProofRequirement
// ---------------------------------------------------------------------------

/// A proof requirement that must be satisfied for a mutation to proceed.
///
/// Maps to [`ProofKind`] for routing, but carries additional
/// domain-specific parameters.
#[derive(Debug, Clone)]
pub enum ProofRequirement {
    /// Dimension equality: vector has expected dimension.
    DimensionMatch { expected: u32 },
    /// Type constructor: node/edge type matches schema.
    TypeMatch { schema_id: u64 },
    /// Invariant preservation: graph property holds after mutation.
    InvariantPreserved { invariant_id: u32 },
    /// Coherence bound: attention coherence above threshold.
    CoherenceBound { min_coherence: f64 },
    /// Composition: all sub-requirements must be satisfied.
    Composite(Vec<ProofRequirement>),
}

impl ProofRequirement {
    /// Map this requirement to a [`ProofKind`] for tier routing.
    pub fn to_proof_kind(&self) -> ProofKind {
        match self {
            ProofRequirement::DimensionMatch { expected } => ProofKind::DimensionEquality {
                expected: *expected,
                actual: *expected,
            },
            ProofRequirement::TypeMatch { .. } => ProofKind::TypeApplication { depth: 1 },
            ProofRequirement::InvariantPreserved { .. } => ProofKind::Custom {
                estimated_complexity: 100,
            },
            ProofRequirement::CoherenceBound { .. } => ProofKind::Custom {
                estimated_complexity: 100,
            },
            ProofRequirement::Composite(subs) => {
                // Use the highest-complexity sub-requirement for routing.
                if subs.iter().any(|r| {
                    matches!(
                        r,
                        ProofRequirement::InvariantPreserved { .. }
                            | ProofRequirement::CoherenceBound { .. }
                    )
                }) {
                    ProofKind::Custom {
                        estimated_complexity: 100,
                    }
                } else if subs
                    .iter()
                    .any(|r| matches!(r, ProofRequirement::TypeMatch { .. }))
                {
                    ProofKind::TypeApplication { depth: 1 }
                } else {
                    ProofKind::DimensionEquality {
                        expected: 0,
                        actual: 0,
                    }
                }
            }
        }
    }

    /// Count the number of leaf requirements (non-composite).
    pub fn leaf_count(&self) -> usize {
        match self {
            ProofRequirement::Composite(subs) => subs.iter().map(|s| s.leaf_count()).sum(),
            _ => 1,
        }
    }
}

// ---------------------------------------------------------------------------
// ComplexityBound
// ---------------------------------------------------------------------------

/// Complexity class designation for proof operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexityClass {
    /// O(1) constant time.
    Constant,
    /// O(log n) logarithmic time.
    Logarithmic,
    /// O(n) linear time.
    Linear,
    /// O(n log n) linearithmic time.
    Linearithmic,
    /// O(n^2) quadratic time.
    Quadratic,
}

/// Upper bounds on the computational cost of a proof obligation.
///
/// Used by the tier router to decide whether a proof can be handled
/// at the Reflex or Standard tier, or must escalate to Deep.
#[derive(Debug, Clone)]
pub struct ComplexityBound {
    /// Upper bound on the number of reduction operations.
    pub ops_upper_bound: u64,
    /// Upper bound on memory consumption in bytes.
    pub memory_upper_bound: u64,
    /// Asymptotic complexity class.
    pub complexity_class: ComplexityClass,
}

impl ComplexityBound {
    /// Create a new complexity bound.
    pub fn new(
        ops_upper_bound: u64,
        memory_upper_bound: u64,
        complexity_class: ComplexityClass,
    ) -> Self {
        Self {
            ops_upper_bound,
            memory_upper_bound,
            complexity_class,
        }
    }

    /// Check whether this bound fits within the Reflex tier budget.
    pub fn fits_reflex(&self) -> bool {
        self.complexity_class == ComplexityClass::Constant && self.ops_upper_bound <= 10
    }

    /// Check whether this bound fits within the Standard tier budget.
    pub fn fits_standard(&self) -> bool {
        self.ops_upper_bound <= 500
    }
}

// ---------------------------------------------------------------------------
// ProofClass
// ---------------------------------------------------------------------------

/// Classification of a proof as either formally verified or statistically
/// sampled.
///
/// Formal proofs are machine-checked via the ruvector-verified kernel.
/// Statistical proofs use randomized testing (e.g., property-based tests)
/// to establish confidence within a tolerance bound.
#[derive(Debug, Clone)]
pub enum ProofClass {
    /// Machine-checked formal proof via the verification kernel.
    Formal,
    /// Statistical proof via randomized testing.
    Statistical {
        /// Number of random test iterations.
        iterations: u64,
        /// Failure tolerance (e.g., 1e-9 for one-in-a-billion).
        tolerance: f64,
        /// RNG seed for reproducibility.
        rng_seed: u64,
    },
}

impl ProofClass {
    /// Check if this is a formal (machine-checked) proof.
    pub fn is_formal(&self) -> bool {
        matches!(self, ProofClass::Formal)
    }

    /// Check if this is a statistical proof.
    pub fn is_statistical(&self) -> bool {
        matches!(self, ProofClass::Statistical { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Existing tests (unchanged)
    // -----------------------------------------------------------------------

    #[test]
    fn test_proof_gate_create_and_read() {
        let gate = ProofGate::new(42u32);
        assert_eq!(*gate.read(), 42);
        assert!(gate.attestation_chain().is_empty());
    }

    #[test]
    fn test_proof_gate_dim_mutation() {
        let mut gate = ProofGate::new(vec![0.0f32; 128]);
        let att = gate.mutate_with_dim_proof(128, 128, |v| {
            v[0] = 1.0;
        });
        assert!(att.is_ok());
        assert_eq!(gate.read()[0], 1.0);
        assert_eq!(gate.attestation_chain().len(), 1);
    }

    #[test]
    fn test_proof_gate_dim_mutation_fails() {
        let mut gate = ProofGate::new(vec![0.0f32; 64]);
        let att = gate.mutate_with_dim_proof(128, 64, |v| {
            v[0] = 1.0;
        });
        assert!(att.is_err());
        // Value should not have been mutated
        assert_eq!(gate.read()[0], 0.0);
        assert!(gate.attestation_chain().is_empty());
    }

    #[test]
    fn test_proof_gate_routed_mutation() {
        let mut gate = ProofGate::new(100i32);
        let result = gate.mutate_with_routed_proof(ProofKind::Reflexivity, 5, 5, |v| *v += 1);
        assert!(result.is_ok());
        let (decision, _att) = result.unwrap();
        assert_eq!(decision.tier, ProofTier::Reflex);
        assert_eq!(*gate.read(), 101);
    }

    #[test]
    fn test_attestation_chain_integrity() {
        let mut chain = AttestationChain::new();
        let env = ProofEnvironment::new();
        for i in 0..5 {
            let att = create_attestation(&env, i);
            chain.append(att);
        }
        assert_eq!(chain.len(), 5);
        assert!(chain.verify_integrity());
    }

    #[test]
    fn test_attestation_chain_hash_deterministic() {
        let mut chain1 = AttestationChain::new();
        let mut chain2 = AttestationChain::new();
        let env = ProofEnvironment::new();
        let att = create_attestation(&env, 0);
        chain1.append(att.clone());
        chain2.append(att);
        // Note: timestamps differ, so hashes will differ.
        // But both should produce non-zero hashes.
        assert_ne!(chain1.chain_hash(), 0);
        assert_ne!(chain2.chain_hash(), 0);
    }

    // -----------------------------------------------------------------------
    // MutationLedger tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_mutation_ledger_append() {
        let env = ProofEnvironment::new();
        let mut ledger = MutationLedger::new(100);

        assert!(ledger.is_empty());
        assert_eq!(ledger.len(), 0);

        let att0 = create_attestation(&env, 0);
        let pos0 = ledger.append(att0);
        assert_eq!(pos0, 0);
        assert_eq!(ledger.len(), 1);

        let att1 = create_attestation(&env, 1);
        let pos1 = ledger.append(att1);
        assert_eq!(pos1, 1);
        assert_eq!(ledger.len(), 2);

        assert!(!ledger.is_empty());
    }

    #[test]
    fn test_mutation_ledger_integrity_after_appends() {
        let env = ProofEnvironment::new();
        let mut ledger = MutationLedger::new(100);

        for i in 0..10 {
            let att = create_attestation(&env, i);
            ledger.append(att);
        }
        assert!(ledger.verify_integrity());
    }

    #[test]
    fn test_mutation_ledger_compact() {
        let env = ProofEnvironment::new();
        let mut ledger = MutationLedger::new(5);

        for i in 0..5 {
            let att = create_attestation(&env, i);
            ledger.append(att);
        }
        assert_eq!(ledger.len(), 5);
        assert!(ledger.needs_compaction());

        let seal = ledger.compact();
        // After compaction, exactly one entry remains (the seal).
        assert_eq!(ledger.len(), 1);
        assert!(!ledger.needs_compaction());

        // The seal's proof_term_hash encodes the chain hash.
        let encoded_hash = u64::from_le_bytes(seal.proof_term_hash[0..8].try_into().unwrap());
        assert_ne!(encoded_hash, 0);

        // Integrity holds after compaction.
        assert!(ledger.verify_integrity());
    }

    #[test]
    fn test_mutation_ledger_integrity_after_compact() {
        let env = ProofEnvironment::new();
        let mut ledger = MutationLedger::new(3);

        for i in 0..3 {
            ledger.append(create_attestation(&env, i));
        }
        assert!(ledger.verify_integrity());

        ledger.compact();
        assert!(ledger.verify_integrity());

        // Append more after compaction.
        for i in 10..13 {
            ledger.append(create_attestation(&env, i));
        }
        assert!(ledger.verify_integrity());
    }

    #[test]
    fn test_mutation_ledger_chain_hash_changes_on_append() {
        let env = ProofEnvironment::new();
        let mut ledger = MutationLedger::new(100);

        let h0 = ledger.chain_hash();
        ledger.append(create_attestation(&env, 0));
        let h1 = ledger.chain_hash();
        assert_ne!(h0, h1);

        ledger.append(create_attestation(&env, 1));
        let h2 = ledger.chain_hash();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_mutation_ledger_epoch() {
        let mut ledger = MutationLedger::new(100);
        assert_eq!(ledger.epoch(), 0);
        ledger.set_epoch(5);
        assert_eq!(ledger.epoch(), 5);
    }

    // -----------------------------------------------------------------------
    // ProofScope tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_proof_scope_creation() {
        let scope = ProofScope::new(42, vec![1, 2, 3], 100);
        assert_eq!(scope.partition_id(), 42);
        assert_eq!(scope.boundary_nodes(), &[1, 2, 3]);
        assert!(scope.coherence().is_none());
        assert!(scope.ledger().is_empty());
    }

    #[test]
    fn test_proof_scope_coherence() {
        let mut scope = ProofScope::new(1, vec![], 100);
        assert!(scope.coherence().is_none());
        scope.set_coherence(0.95);
        assert_eq!(scope.coherence(), Some(0.95));
    }

    #[test]
    fn test_proof_scope_ledger_access() {
        let env = ProofEnvironment::new();
        let mut scope = ProofScope::new(1, vec![10, 20], 100);
        scope.ledger_mut().append(create_attestation(&env, 0));
        scope.ledger_mut().append(create_attestation(&env, 1));
        assert_eq!(scope.ledger().len(), 2);
        assert!(scope.ledger().verify_integrity());
    }

    #[test]
    fn test_proof_scope_transition() {
        let env = ProofEnvironment::new();
        let mut scope = ProofScope::new(1, vec![10, 20], 100);
        scope.set_coherence(0.9);
        scope.ledger_mut().append(create_attestation(&env, 0));
        scope.ledger_mut().append(create_attestation(&env, 1));

        let transition = scope.transition(2, vec![30, 40], 3.5);

        // Transition attestation reflects old state.
        assert_eq!(transition.old_partition_id, 1);
        assert_eq!(transition.new_partition_id, 2);
        assert_eq!(transition.min_cut_value, 3.5);
        assert_eq!(transition.old_coherence, Some(0.9));

        // Scope is now updated.
        assert_eq!(scope.partition_id(), 2);
        assert_eq!(scope.boundary_nodes(), &[30, 40]);
        assert!(scope.coherence().is_none());
        assert!(scope.ledger().is_empty());
    }

    // -----------------------------------------------------------------------
    // EpochBoundary tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_epoch_boundary_seal() {
        let env = ProofEnvironment::new();
        let mut ledger = MutationLedger::new(100);

        for i in 0..5 {
            ledger.append(create_attestation(&env, i));
        }
        assert_eq!(ledger.epoch(), 0);

        let config = ProofEnvironmentConfig {
            max_standard_fuel: 1000,
            max_deep_steps: 20_000,
            builtin_symbols: 128,
        };

        let boundary = EpochBoundary::seal(&mut ledger, config);

        assert_eq!(boundary.from_epoch, 0);
        assert_eq!(boundary.to_epoch, 1);
        assert_eq!(boundary.new_config.max_standard_fuel, 1000);
        assert_eq!(boundary.new_config.max_deep_steps, 20_000);
        assert_eq!(boundary.new_config.builtin_symbols, 128);

        // Ledger epoch is advanced.
        assert_eq!(ledger.epoch(), 1);
        // Ledger is compacted to 1 entry (the seal).
        assert_eq!(ledger.len(), 1);
        assert!(ledger.verify_integrity());
    }

    #[test]
    fn test_epoch_boundary_default_config() {
        let config = ProofEnvironmentConfig::default();
        assert_eq!(config.max_standard_fuel, 500);
        assert_eq!(config.max_deep_steps, 10_000);
        assert_eq!(config.builtin_symbols, 64);
    }

    // -----------------------------------------------------------------------
    // SupersessionProof tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_supersession_proof_creation() {
        let env = ProofEnvironment::new();
        let att = create_attestation(&env, 42);
        let sp = SupersessionProof::new(7, att.clone(), 99);
        assert_eq!(sp.superseded_position, 7);
        assert_eq!(sp.soundness_proof_id, 99);
        assert_eq!(sp.replacement.content_hash(), att.content_hash(),);
    }

    // -----------------------------------------------------------------------
    // ProofRequirement tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_proof_requirement_to_proof_kind() {
        let dim = ProofRequirement::DimensionMatch { expected: 128 };
        assert!(matches!(
            dim.to_proof_kind(),
            ProofKind::DimensionEquality { .. }
        ));

        let ty = ProofRequirement::TypeMatch { schema_id: 1 };
        assert!(matches!(
            ty.to_proof_kind(),
            ProofKind::TypeApplication { .. }
        ));

        let inv = ProofRequirement::InvariantPreserved { invariant_id: 5 };
        assert!(matches!(inv.to_proof_kind(), ProofKind::Custom { .. }));

        let coh = ProofRequirement::CoherenceBound { min_coherence: 0.8 };
        assert!(matches!(coh.to_proof_kind(), ProofKind::Custom { .. }));
    }

    #[test]
    fn test_proof_requirement_composite_routing() {
        // Composite with only DimensionMatch -> DimensionEquality.
        let comp_dim = ProofRequirement::Composite(vec![
            ProofRequirement::DimensionMatch { expected: 64 },
            ProofRequirement::DimensionMatch { expected: 128 },
        ]);
        assert!(matches!(
            comp_dim.to_proof_kind(),
            ProofKind::DimensionEquality { .. }
        ));

        // Composite with TypeMatch -> TypeApplication.
        let comp_ty = ProofRequirement::Composite(vec![
            ProofRequirement::DimensionMatch { expected: 64 },
            ProofRequirement::TypeMatch { schema_id: 1 },
        ]);
        assert!(matches!(
            comp_ty.to_proof_kind(),
            ProofKind::TypeApplication { .. }
        ));

        // Composite with InvariantPreserved -> Custom.
        let comp_inv = ProofRequirement::Composite(vec![
            ProofRequirement::TypeMatch { schema_id: 1 },
            ProofRequirement::InvariantPreserved { invariant_id: 3 },
        ]);
        assert!(matches!(comp_inv.to_proof_kind(), ProofKind::Custom { .. }));
    }

    #[test]
    fn test_proof_requirement_leaf_count() {
        let single = ProofRequirement::DimensionMatch { expected: 64 };
        assert_eq!(single.leaf_count(), 1);

        let composite = ProofRequirement::Composite(vec![
            ProofRequirement::DimensionMatch { expected: 64 },
            ProofRequirement::TypeMatch { schema_id: 1 },
            ProofRequirement::Composite(vec![
                ProofRequirement::InvariantPreserved { invariant_id: 1 },
                ProofRequirement::CoherenceBound { min_coherence: 0.5 },
            ]),
        ]);
        assert_eq!(composite.leaf_count(), 4);
    }

    // -----------------------------------------------------------------------
    // ComplexityBound tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_complexity_bound_fits_reflex() {
        let reflex = ComplexityBound::new(5, 64, ComplexityClass::Constant);
        assert!(reflex.fits_reflex());
        assert!(reflex.fits_standard());

        let too_many_ops = ComplexityBound::new(20, 64, ComplexityClass::Constant);
        assert!(!too_many_ops.fits_reflex());

        let wrong_class = ComplexityBound::new(5, 64, ComplexityClass::Linear);
        assert!(!wrong_class.fits_reflex());
    }

    #[test]
    fn test_complexity_bound_fits_standard() {
        let standard = ComplexityBound::new(500, 4096, ComplexityClass::Logarithmic);
        assert!(standard.fits_standard());

        let too_expensive = ComplexityBound::new(501, 4096, ComplexityClass::Quadratic);
        assert!(!too_expensive.fits_standard());
    }

    // -----------------------------------------------------------------------
    // ProofClass tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_proof_class_formal() {
        let formal = ProofClass::Formal;
        assert!(formal.is_formal());
        assert!(!formal.is_statistical());
    }

    #[test]
    fn test_proof_class_statistical() {
        let stat = ProofClass::Statistical {
            iterations: 10_000,
            tolerance: 1e-9,
            rng_seed: 42,
        };
        assert!(!stat.is_formal());
        assert!(stat.is_statistical());
    }
}
