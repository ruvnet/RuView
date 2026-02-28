//! WASM bindings for `ruvector-verified`: proof-carrying vector operations in the browser.
//!
//! # Quick Start (JavaScript)
//!
//! ```js
//! import init, { JsProofEnv } from "ruvector-verified-wasm";
//!
//! await init();
//! const env = new JsProofEnv();
//!
//! // Prove dimension equality (~500ns)
//! const proofId = env.prove_dim_eq(384, 384);  // Ok -> proof ID
//!
//! // Verify a batch of vectors
//! const vectors = [new Float32Array(384).fill(0.5)];
//! const result = env.verify_batch(384, vectors);
//!
//! // Get statistics
//! console.log(env.stats());
//!
//! // Create attestation (82 bytes)
//! const att = env.create_attestation(proofId);
//! console.log(att.bytes.length); // 82
//! ```

mod utils;

use ruvector_verified::{
    cache::ConversionCache,
    fast_arena::FastTermArena,
    gated::{self, ProofKind, ProofTier},
    proof_store, vector_types, ProofEnvironment,
};
use serde::Serialize;
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Module init
// ---------------------------------------------------------------------------

/// Called automatically when the WASM module is loaded.
#[wasm_bindgen(start)]
pub fn init() {
    utils::set_panic_hook();
    utils::console_log("ruvector-verified-wasm loaded");
}

/// Return the crate version.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ---------------------------------------------------------------------------
// JsProofEnv — main entry point
// ---------------------------------------------------------------------------

/// Proof environment for the browser. Wraps `ProofEnvironment` + ultra caches.
#[wasm_bindgen]
pub struct JsProofEnv {
    env: ProofEnvironment,
    arena: FastTermArena,
    cache: ConversionCache,
}

#[wasm_bindgen]
impl JsProofEnv {
    /// Create a new proof environment with all optimizations.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            env: ProofEnvironment::new(),
            arena: FastTermArena::with_capacity(4096),
            cache: ConversionCache::with_capacity(1024),
        }
    }

    /// Prove that two dimensions are equal. Returns proof term ID.
    ///
    /// Throws if dimensions don't match.
    pub fn prove_dim_eq(&mut self, expected: u32, actual: u32) -> Result<u32, JsError> {
        vector_types::prove_dim_eq(&mut self.env, expected, actual)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Build a `RuVec n` type term. Returns term ID.
    pub fn mk_vector_type(&mut self, dim: u32) -> Result<u32, JsError> {
        vector_types::mk_vector_type(&mut self.env, dim).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Build a distance metric type term. Supported: "L2", "Cosine", "Dot".
    pub fn mk_distance_metric(&mut self, metric: &str) -> Result<u32, JsError> {
        vector_types::mk_distance_metric(&mut self.env, metric)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Verify that a single vector has the expected dimension.
    pub fn verify_dim_check(&mut self, index_dim: u32, vector: &[f32]) -> Result<u32, JsError> {
        vector_types::verified_dim_check(&mut self.env, index_dim, vector)
            .map(|op| op.proof_id)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Verify a batch of vectors (passed as flat f32 array + dimension).
    ///
    /// `flat_vectors` is a contiguous f32 array; each vector is `dim` elements.
    /// Returns the number of vectors verified.
    pub fn verify_batch_flat(&mut self, dim: u32, flat_vectors: &[f32]) -> Result<u32, JsError> {
        let d = dim as usize;
        if flat_vectors.len() % d != 0 {
            return Err(JsError::new(&format!(
                "flat_vectors length {} not divisible by dim {}",
                flat_vectors.len(),
                dim
            )));
        }
        let slices: Vec<&[f32]> = flat_vectors.chunks_exact(d).collect();
        vector_types::verify_batch_dimensions(&mut self.env, dim, &slices)
            .map(|op| op.value as u32)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Intern a hash into the FastTermArena. Returns `[term_id, was_cached]`.
    pub fn arena_intern(&self, hash_hi: u32, hash_lo: u32) -> Vec<u32> {
        let hash = (hash_hi as u64) << 32 | hash_lo as u64;
        let (id, cached) = self.arena.intern(hash);
        vec![id, if cached { 1 } else { 0 }]
    }

    /// Route a proof to the cheapest tier. Returns tier name.
    pub fn route_proof(&self, kind: &str) -> Result<JsValue, JsError> {
        let proof_kind = match kind {
            "reflexivity" => ProofKind::Reflexivity,
            "dimension" => ProofKind::DimensionEquality {
                expected: 0,
                actual: 0,
            },
            "pipeline" => ProofKind::PipelineComposition { stages: 1 },
            other => ProofKind::Custom {
                estimated_complexity: other.parse().unwrap_or(10),
            },
        };
        let decision = gated::route_proof(proof_kind, &self.env);
        let tier_name = match decision.tier {
            ProofTier::Reflex => "reflex",
            ProofTier::Standard { .. } => "standard",
            ProofTier::Deep => "deep",
        };
        let result = JsRoutingResult {
            tier: tier_name.to_string(),
            reason: decision.reason.to_string(),
            estimated_steps: decision.estimated_steps,
        };
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Create a proof attestation (82 bytes). Returns serializable object.
    pub fn create_attestation(&self, proof_id: u32) -> Result<JsValue, JsError> {
        let att = proof_store::create_attestation(&self.env, proof_id);
        let bytes = att.to_bytes();
        let result = JsAttestation {
            bytes,
            proof_term_hash: hex_encode(&att.proof_term_hash),
            environment_hash: hex_encode(&att.environment_hash),
            verifier_version: format!("{:#010x}", att.verifier_version),
            reduction_steps: att.reduction_steps,
            cache_hit_rate_bps: att.cache_hit_rate_bps,
        };
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get verification statistics.
    pub fn stats(&self) -> Result<JsValue, JsError> {
        let s = self.env.stats();
        let arena_stats = self.arena.stats();
        let cache_stats = self.cache.stats();
        let result = JsStats {
            proofs_constructed: s.proofs_constructed,
            proofs_verified: s.proofs_verified,
            cache_hits: s.cache_hits,
            cache_misses: s.cache_misses,
            total_reductions: s.total_reductions,
            terms_allocated: self.env.terms_allocated(),
            arena_hit_rate: arena_stats.cache_hit_rate(),
            conversion_cache_hit_rate: cache_stats.hit_rate(),
        };
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Reset the environment (clears cache, resets counters, re-registers builtins).
    pub fn reset(&mut self) {
        self.env.reset();
        self.arena.reset();
        self.cache.clear();
    }

    /// Number of terms currently allocated.
    pub fn terms_allocated(&self) -> u32 {
        self.env.terms_allocated()
    }
}

// ---------------------------------------------------------------------------
// JSON result types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct JsRoutingResult {
    tier: String,
    reason: String,
    estimated_steps: u32,
}

#[derive(Serialize)]
struct JsAttestation {
    bytes: Vec<u8>,
    proof_term_hash: String,
    environment_hash: String,
    verifier_version: String,
    reduction_steps: u32,
    cache_hit_rate_bps: u16,
}

#[derive(Serialize)]
struct JsStats {
    proofs_constructed: u64,
    proofs_verified: u64,
    cache_hits: u64,
    cache_misses: u64,
    total_reductions: u64,
    terms_allocated: u32,
    arena_hit_rate: f64,
    conversion_cache_hit_rate: f64,
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
