//! Formal verification layer for RuVector using lean-agentic dependent types.
//!
//! This crate provides proof-carrying vector operations, verified pipeline
//! composition, and formal attestation for RuVector's safety-critical paths.
//!
//! # Feature Flags
//!
//! - `hnsw-proofs`: Enable verified HNSW insert/query operations
//! - `rvf-proofs`: Enable RVF witness chain integration
//! - `coherence-proofs`: Enable coherence verification
//! - `serde`: Enable serialization of proof attestations
//! - `fast-arena`: SolverArena-style bump allocator
//! - `simd-hash`: AVX2/NEON accelerated hash-consing
//! - `gated-proofs`: Coherence-gated proof depth routing
//! - `ultra`: All optimizations (fast-arena + simd-hash + gated-proofs)
//! - `all-proofs`: All proof integrations (hnsw + rvf + coherence)

pub mod error;
pub mod invariants;
pub mod pipeline;
pub mod proof_store;
pub mod vector_types;

pub mod cache;
#[cfg(feature = "fast-arena")]
pub mod fast_arena;
#[cfg(feature = "gated-proofs")]
pub mod gated;
pub mod pools;

// Re-exports
pub use error::{Result, VerificationError};
pub use invariants::BuiltinDecl;
pub use pipeline::VerifiedStage;
pub use proof_store::ProofAttestation;
pub use vector_types::{mk_nat_literal, mk_vector_type, prove_dim_eq};

/// The proof environment bundles verification state.
///
/// One instance per thread (not `Sync` due to interior state).
/// Create with `ProofEnvironment::new()` which pre-loads RuVector type
/// declarations.
///
/// # Example
///
/// ```rust,ignore
/// use ruvector_verified::ProofEnvironment;
///
/// let mut env = ProofEnvironment::new();
/// let proof = env.prove_dim_eq(128, 128).unwrap();
/// ```
pub struct ProofEnvironment {
    /// Registered built-in symbol names.
    pub symbols: Vec<String>,
    /// Proof term counter (monotonically increasing).
    term_counter: u32,
    /// Cache of recently verified proofs: (input_hash, proof_id).
    proof_cache: std::collections::HashMap<u64, u32>,
    /// Statistics.
    pub stats: ProofStats,
}

/// Verification statistics.
#[derive(Debug, Clone, Default)]
pub struct ProofStats {
    /// Total proofs constructed.
    pub proofs_constructed: u64,
    /// Total proofs verified.
    pub proofs_verified: u64,
    /// Cache hits (proof reused).
    pub cache_hits: u64,
    /// Cache misses (new proof constructed).
    pub cache_misses: u64,
    /// Total reduction steps consumed.
    pub total_reductions: u64,
}

impl ProofEnvironment {
    /// Create a new proof environment pre-loaded with RuVector type declarations.
    pub fn new() -> Self {
        let mut symbols = Vec::with_capacity(32);
        invariants::register_builtin_symbols(&mut symbols);

        Self {
            symbols,
            term_counter: 0,
            proof_cache: std::collections::HashMap::with_capacity(256),
            stats: ProofStats::default(),
        }
    }

    /// Allocate a new proof term ID.
    pub fn alloc_term(&mut self) -> u32 {
        let id = self.term_counter;
        self.term_counter = self
            .term_counter
            .checked_add(1)
            .ok_or(VerificationError::ArenaExhausted { allocated: id })
            .expect("arena overflow");
        self.stats.proofs_constructed += 1;
        id
    }

    /// Look up a symbol index by name.
    pub fn symbol_id(&self, name: &str) -> Option<usize> {
        self.symbols.iter().position(|s| s == name)
    }

    /// Require a symbol index, or return DeclarationNotFound.
    pub fn require_symbol(&self, name: &str) -> Result<usize> {
        self.symbol_id(name)
            .ok_or_else(|| VerificationError::DeclarationNotFound {
                name: name.to_string(),
            })
    }

    /// Check the proof cache for a previously verified proof.
    pub fn cache_lookup(&mut self, key: u64) -> Option<u32> {
        if let Some(&id) = self.proof_cache.get(&key) {
            self.stats.cache_hits += 1;
            Some(id)
        } else {
            self.stats.cache_misses += 1;
            None
        }
    }

    /// Insert a verified proof into the cache.
    pub fn cache_insert(&mut self, key: u64, proof_id: u32) {
        self.proof_cache.insert(key, proof_id);
    }

    /// Get verification statistics.
    pub fn stats(&self) -> &ProofStats {
        &self.stats
    }

    /// Number of terms allocated.
    pub fn terms_allocated(&self) -> u32 {
        self.term_counter
    }

    /// Reset the environment (clear cache, reset counters).
    /// Useful between independent proof obligations.
    pub fn reset(&mut self) {
        self.term_counter = 0;
        self.proof_cache.clear();
        self.stats = ProofStats::default();
        // Re-register builtins
        self.symbols.clear();
        invariants::register_builtin_symbols(&mut self.symbols);
    }
}

impl Default for ProofEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

/// A vector operation with a machine-checked type proof.
#[derive(Debug, Clone, Copy)]
pub struct VerifiedOp<T> {
    /// The operation result.
    pub value: T,
    /// Proof term ID in the environment.
    pub proof_id: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_env_new_has_builtins() {
        let env = ProofEnvironment::new();
        assert!(env.symbol_id("Nat").is_some());
        assert!(env.symbol_id("RuVec").is_some());
        assert!(env.symbol_id("Eq").is_some());
        assert!(env.symbol_id("Eq.refl").is_some());
        assert!(env.symbol_id("HnswIndex").is_some());
    }

    #[test]
    fn proof_env_alloc_term() {
        let mut env = ProofEnvironment::new();
        assert_eq!(env.alloc_term(), 0);
        assert_eq!(env.alloc_term(), 1);
        assert_eq!(env.alloc_term(), 2);
        assert_eq!(env.terms_allocated(), 3);
    }

    #[test]
    fn proof_env_cache() {
        let mut env = ProofEnvironment::new();
        assert!(env.cache_lookup(42).is_none());
        env.cache_insert(42, 7);
        assert_eq!(env.cache_lookup(42), Some(7));
        assert_eq!(env.stats().cache_hits, 1);
        assert_eq!(env.stats().cache_misses, 1);
    }

    #[test]
    fn proof_env_reset() {
        let mut env = ProofEnvironment::new();
        env.alloc_term();
        env.cache_insert(1, 2);
        env.reset();
        assert_eq!(env.terms_allocated(), 0);
        assert!(env.cache_lookup(1).is_none());
        // Builtins restored after reset
        assert!(env.symbol_id("Nat").is_some());
    }

    #[test]
    fn proof_env_require_symbol() {
        let env = ProofEnvironment::new();
        assert!(env.require_symbol("Nat").is_ok());
        assert!(env.require_symbol("NonExistent").is_err());
    }

    #[test]
    fn verified_op_copy() {
        let op = VerifiedOp {
            value: 42u32,
            proof_id: 1,
        };
        let op2 = op; // Copy
        assert_eq!(op.value, op2.value);
    }
}
