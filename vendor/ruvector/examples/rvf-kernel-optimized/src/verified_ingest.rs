//! Verified vector ingest pipeline using ruvector-verified ultra-optimizations.
//!
//! Every vector batch passes through:
//! 1. Gated proof routing (Reflex/Standard/Deep tier selection)
//! 2. FastTermArena dedup (4-wide linear probe, 95%+ hit rate)
//! 3. Dimension proof generation (prove_dim_eq with FxHash cache)
//! 4. ConversionCache (open-addressing equality cache)
//! 5. Thread-local pool resource acquisition
//! 6. ProofAttestation creation (82-byte witness, type 0x0E)

use anyhow::{anyhow, Result};
use ruvector_verified::{
    cache::ConversionCache,
    fast_arena::FastTermArena,
    gated::{self, ProofKind},
    pools,
    proof_store::create_attestation,
    vector_types, ProofAttestation, ProofEnvironment,
};
use rvf_runtime::RvfStore;
use tracing::{debug, info};

/// Statistics from a verified ingest run.
#[derive(Debug, Clone)]
pub struct IngestStats {
    /// Total vectors verified and ingested.
    pub vectors_verified: u64,
    /// Total proof terms generated.
    pub proofs_generated: u64,
    /// Arena dedup cache hit rate (0.0-1.0).
    pub arena_hit_rate: f64,
    /// Conversion cache hit rate (0.0-1.0).
    pub conversion_cache_hit_rate: f64,
    /// Proof routing tier distribution [reflex, standard, deep].
    pub tier_distribution: [u64; 3],
    /// Number of attestations created.
    pub attestations_created: u64,
    /// Total ingest wall time in microseconds.
    pub total_time_us: u64,
}

/// Verified ingest pipeline combining all ruvector-verified optimizations.
pub struct VerifiedIngestPipeline {
    env: ProofEnvironment,
    arena: FastTermArena,
    cache: ConversionCache,
    dim: u32,
    tier_counts: [u64; 3],
    attestations: Vec<ProofAttestation>,
}

impl VerifiedIngestPipeline {
    /// Create a new pipeline for vectors of the given dimension.
    pub fn new(dim: u32) -> Self {
        Self {
            env: ProofEnvironment::new(),
            arena: FastTermArena::with_capacity(4096),
            cache: ConversionCache::with_capacity(1024),
            dim,
            tier_counts: [0; 3],
            attestations: Vec::new(),
        }
    }

    /// Verify a batch of vectors and ingest into the RVF store.
    ///
    /// Returns the number of vectors successfully ingested.
    pub fn verify_and_ingest(
        &mut self,
        store: &mut RvfStore,
        vectors: &[Vec<f32>],
        ids: &[u64],
    ) -> Result<u64> {
        // Acquire thread-local pooled resources (auto-returned on drop)
        let _pooled = pools::acquire();

        // Route proof to cheapest tier
        let decision = gated::route_proof(
            ProofKind::DimensionEquality {
                expected: self.dim,
                actual: self.dim,
            },
            &self.env,
        );
        match decision.tier {
            ruvector_verified::gated::ProofTier::Reflex => self.tier_counts[0] += 1,
            ruvector_verified::gated::ProofTier::Standard { .. } => self.tier_counts[1] += 1,
            ruvector_verified::gated::ProofTier::Deep => self.tier_counts[2] += 1,
        }

        // Check arena dedup cache for dimension proof
        let dim_hash = ruvector_verified::fast_arena::fx_hash_pair(self.dim, self.dim);
        let (_term_id, was_cached) = self.arena.intern(dim_hash);

        if was_cached {
            debug!("arena cache hit for dim proof");
        }

        // Check conversion cache
        let cached_proof = self.cache.get(_term_id, self.dim);
        let proof_id = if let Some(pid) = cached_proof {
            debug!(pid, "conversion cache hit");
            pid
        } else {
            // Generate dimension equality proof (~500ns)
            let pid = vector_types::prove_dim_eq(&mut self.env, self.dim, self.dim)?;
            self.cache.insert(_term_id, self.dim, pid);
            pid
        };

        // Verify all vectors in the batch have correct dimensions
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let _verified = vector_types::verify_batch_dimensions(&mut self.env, self.dim, &refs)?;

        debug!(count = vectors.len(), proof_id, "batch verified");

        // Ingest into RVF store
        store
            .ingest_batch(&refs, ids, None)
            .map_err(|e| anyhow!("ingest: {e:?}"))?;

        // Create proof attestation for this batch
        let attestation = create_attestation(&self.env, proof_id);
        self.attestations.push(attestation);

        Ok(vectors.len() as u64)
    }

    /// Get current statistics.
    pub fn stats(&self) -> IngestStats {
        let arena_stats = self.arena.stats();
        let cache_stats = self.cache.stats();
        let (_pool_hits, _pool_misses, _) = pools::pool_stats();

        IngestStats {
            vectors_verified: self.env.stats().proofs_constructed,
            proofs_generated: self.env.stats().proofs_constructed,
            arena_hit_rate: arena_stats.cache_hit_rate(),
            conversion_cache_hit_rate: cache_stats.hit_rate(),
            tier_distribution: self.tier_counts,
            attestations_created: self.attestations.len() as u64,
            total_time_us: 0, // filled by caller
        }
    }

    /// Get all attestations created during ingest.
    pub fn attestations(&self) -> &[ProofAttestation] {
        &self.attestations
    }

    /// Get the proof environment for inspection.
    pub fn env(&self) -> &ProofEnvironment {
        &self.env
    }

    /// Reset the pipeline for a new ingest cycle.
    pub fn reset(&mut self) {
        self.env.reset();
        self.arena.reset();
        self.cache.clear();
        self.tier_counts = [0; 3];
        self.attestations.clear();
    }
}

/// Run a complete verified ingest cycle: generate vectors, verify, ingest.
///
/// Returns (IngestStats, store_file_size_bytes).
pub fn run_verified_ingest(
    store: &mut RvfStore,
    store_path: &std::path::Path,
    dim: u32,
    vec_count: usize,
    seed: u64,
) -> Result<(IngestStats, u64)> {
    use rand::prelude::*;

    let start = std::time::Instant::now();
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut pipeline = VerifiedIngestPipeline::new(dim);

    // Generate vectors in batches of 1000
    let batch_size = 1000.min(vec_count);
    let mut total_ingested = 0u64;

    for batch_start in (0..vec_count).step_by(batch_size) {
        let batch_end = (batch_start + batch_size).min(vec_count);
        let count = batch_end - batch_start;

        let vectors: Vec<Vec<f32>> = (0..count)
            .map(|_| (0..dim as usize).map(|_| rng.gen::<f32>()).collect())
            .collect();
        let ids: Vec<u64> = (batch_start as u64..batch_end as u64).collect();

        let ingested = pipeline.verify_and_ingest(store, &vectors, &ids)?;
        total_ingested += ingested;
    }

    let elapsed = start.elapsed();
    let mut stats = pipeline.stats();
    stats.total_time_us = elapsed.as_micros() as u64;
    stats.vectors_verified = total_ingested;

    info!(
        vectors = total_ingested,
        proofs = stats.proofs_generated,
        arena_hit = format!("{:.1}%", stats.arena_hit_rate * 100.0),
        cache_hit = format!("{:.1}%", stats.conversion_cache_hit_rate * 100.0),
        tiers = format!(
            "R:{}/S:{}/D:{}",
            stats.tier_distribution[0], stats.tier_distribution[1], stats.tier_distribution[2]
        ),
        attestations = stats.attestations_created,
        time_us = stats.total_time_us,
        "verified ingest complete"
    );

    // Get store file size
    let store_size = std::fs::metadata(store_path).map(|m| m.len()).unwrap_or(0);

    Ok((stats, store_size))
}
