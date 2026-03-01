//! RLM-Style Recursive Sentence Transformer Embedder (AD-24)
//!
//! An inference strategy that wraps a base embedding model in a short iterative
//! loop: embed → retrieve neighbors → contextualize → re-embed → merge.
//!
//! This produces embeddings that are:
//! - Structurally aware (conditioned on RuVector neighborhood)
//! - Contradiction-sensitive (twin embeddings at low-cut boundaries)
//! - Domain-adaptive (without full fine-tuning)
//!
//! Three variants:
//! - **A: Query-Conditioned** — optimized for retrieval under a specific query
//! - **B: Corpus-Conditioned** — stable over time, less phrasing-sensitive
//! - **C: Contradiction-Aware Twin** — bimodal for disputed claims

use crate::error::{Result, RuvLLMError};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the RLM recursive embedder.
#[derive(Debug, Clone)]
pub struct RlmEmbedderConfig {
    /// Embedding dimension of the base model
    pub embed_dim: usize,
    /// Maximum iterations in the recursive loop
    pub max_iterations: usize,
    /// Convergence threshold: stop if cosine(iter_n, iter_n-1) > this value
    pub convergence_threshold: f32,
    /// Number of neighbors to retrieve per iteration
    pub num_neighbors: usize,
    /// Merge weight for base embedding
    pub w_base: f32,
    /// Merge weight for contextualized embedding
    pub w_context: f32,
    /// Merge weight for anti-cluster embedding
    pub w_anti: f32,
    /// Contradiction detection threshold (cosine similarity below this = contested)
    pub contradiction_threshold: f32,
    /// Embedding variant to use
    pub variant: EmbeddingVariant,
}

impl Default for RlmEmbedderConfig {
    fn default() -> Self {
        Self {
            embed_dim: 384,
            max_iterations: 2,
            convergence_threshold: 0.98,
            num_neighbors: 5,
            w_base: 0.6,
            w_context: 0.3,
            w_anti: 0.1,
            contradiction_threshold: 0.3,
            variant: EmbeddingVariant::CorpusConditioned,
        }
    }
}

/// Embedding variant (AD-24).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmbeddingVariant {
    /// Variant A: query-conditioned, optimized for retrieval under specific query
    QueryConditioned,
    /// Variant B: corpus-conditioned, stable over time
    CorpusConditioned,
    /// Variant C: contradiction-aware twin embeddings at low-cut boundaries
    ContradictionAwareTwin,
}

// ============================================================================
// Output Schema
// ============================================================================

/// Stop reason for the recursive loop.
#[derive(Debug, Clone, PartialEq)]
pub enum EmbedStopReason {
    /// Cosine similarity between iterations exceeded convergence threshold
    Converged,
    /// Maximum iterations reached
    MaxIterations,
    /// Contradiction detected — produced twin embeddings (Variant C only)
    Contested,
}

/// Neighbor context used during embedding.
#[derive(Debug, Clone)]
pub struct NeighborContext {
    /// Chunk ID in the evidence corpus
    pub chunk_id: String,
    /// Pre-computed embedding of this neighbor
    pub embedding: Vec<f32>,
    /// Whether this neighbor is in an opposing cluster
    pub is_contradicting: bool,
    /// Cosine similarity to the base embedding of the target chunk
    pub similarity: f32,
}

/// Result of the RLM embedding process.
#[derive(Debug, Clone)]
pub struct RlmEmbeddingResult {
    /// Primary embedding vector (normalized)
    pub embedding: Vec<f32>,
    /// Secondary embedding for Variant C (contradiction-aware twin)
    /// None for Variants A and B.
    pub twin_embedding: Option<Vec<f32>>,
    /// Confidence: cosine similarity between final and penultimate iteration
    pub confidence: f32,
    /// IDs of neighbors used as context
    pub evidence_neighbor_ids: Vec<String>,
    /// Per-neighbor contradiction flag
    pub contradiction_flags: Vec<bool>,
    /// Primary cluster assignment (if available)
    pub cluster_id: Option<usize>,
    /// Why the loop terminated
    pub stop_reason: EmbedStopReason,
    /// Number of iterations actually executed
    pub iterations_used: usize,
}

// ============================================================================
// Base Embedder Trait
// ============================================================================

/// Trait for the base embedding model. Implementations can wrap any sentence
/// transformer (MiniLM, BGE, nomic-embed, or even a ternary-quantized model).
pub trait BaseEmbedder {
    /// Embed a single text chunk into a fixed-dimension vector.
    fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Embedding dimension.
    fn embed_dim(&self) -> usize;
}

/// Trait for retrieving neighbors from the evidence store (e.g., RuVector).
pub trait NeighborRetriever {
    /// Retrieve the k nearest neighbors for a given embedding.
    fn retrieve(&self, embedding: &[f32], k: usize) -> Result<Vec<NeighborContext>>;
}

// ============================================================================
// RLM Embedder
// ============================================================================

/// RLM-style recursive embedder.
///
/// Wraps a `BaseEmbedder` and `NeighborRetriever` to produce context-aware,
/// contradiction-sensitive embeddings via a bounded iterative loop.
pub struct RlmEmbedder<E: BaseEmbedder, R: NeighborRetriever> {
    embedder: E,
    retriever: R,
    config: RlmEmbedderConfig,
}

impl<E: BaseEmbedder, R: NeighborRetriever> RlmEmbedder<E, R> {
    /// Create a new RLM embedder with the given base embedder and retriever.
    pub fn new(embedder: E, retriever: R, config: RlmEmbedderConfig) -> Self {
        Self {
            embedder,
            retriever,
            config,
        }
    }

    /// Embed a text chunk using the RLM recursive strategy.
    ///
    /// For Variant A (query-conditioned), pass the query as `query_context`.
    /// For Variants B and C, `query_context` can be None.
    pub fn embed(&self, text: &str, query_context: Option<&str>) -> Result<RlmEmbeddingResult> {
        let dim = self.config.embed_dim;

        // Step 1: Base embedding
        let base_embedding = self.embedder.embed(text)?;
        if base_embedding.len() != dim {
            return Err(RuvLLMError::Model(format!(
                "Base embedder returned {} dims, expected {}",
                base_embedding.len(),
                dim
            )));
        }

        let mut current = base_embedding.clone();
        let mut prev = base_embedding.clone();
        let mut all_neighbors: Vec<NeighborContext> = Vec::new();
        let mut iterations_used = 0;
        let mut stop_reason = EmbedStopReason::MaxIterations;

        // Recursive loop (bounded)
        for iter in 0..self.config.max_iterations {
            iterations_used = iter + 1;

            // Step 2: Retrieve neighbors
            let neighbors = self
                .retriever
                .retrieve(&current, self.config.num_neighbors)?;

            // Store neighbor info
            for n in &neighbors {
                if !all_neighbors
                    .iter()
                    .any(|existing| existing.chunk_id == n.chunk_id)
                {
                    all_neighbors.push(n.clone());
                }
            }

            // Step 3: Contextualize — compute context embedding from neighbors
            let ctx_embedding =
                self.compute_context_embedding(&current, &neighbors, query_context)?;

            // Step 4: Check for contradiction (Variant C)
            if self.config.variant == EmbeddingVariant::ContradictionAwareTwin {
                let contradicting: Vec<&NeighborContext> =
                    neighbors.iter().filter(|n| n.is_contradicting).collect();

                if !contradicting.is_empty() {
                    // Produce twin embeddings
                    let anti_embedding = self.compute_anti_embedding(&contradicting)?;
                    let twin_a =
                        self.merge_embedding(&current, &ctx_embedding, &anti_embedding, 1.0);
                    let twin_b =
                        self.merge_embedding(&current, &ctx_embedding, &anti_embedding, -1.0);

                    return Ok(RlmEmbeddingResult {
                        embedding: twin_a,
                        twin_embedding: Some(twin_b),
                        confidence: cosine_similarity(&current, &prev),
                        evidence_neighbor_ids: all_neighbors
                            .iter()
                            .map(|n| n.chunk_id.clone())
                            .collect(),
                        contradiction_flags: all_neighbors
                            .iter()
                            .map(|n| n.is_contradicting)
                            .collect(),
                        cluster_id: None,
                        stop_reason: EmbedStopReason::Contested,
                        iterations_used,
                    });
                }
            }

            // Step 5: Merge
            let zero_anti = vec![0.0f32; dim];
            let anti_embedding = if self.config.w_anti > 0.0 {
                let contradicting: Vec<&NeighborContext> =
                    neighbors.iter().filter(|n| n.is_contradicting).collect();
                if contradicting.is_empty() {
                    zero_anti.clone()
                } else {
                    self.compute_anti_embedding(&contradicting)?
                }
            } else {
                zero_anti.clone()
            };

            prev = current.clone();
            current = self.merge_embedding(&current, &ctx_embedding, &anti_embedding, 1.0);

            // Step 6: Check convergence
            let sim = cosine_similarity(&current, &prev);
            if sim > self.config.convergence_threshold {
                stop_reason = EmbedStopReason::Converged;
                break;
            }
        }

        let confidence = cosine_similarity(&current, &prev);

        Ok(RlmEmbeddingResult {
            embedding: current,
            twin_embedding: None,
            confidence,
            evidence_neighbor_ids: all_neighbors.iter().map(|n| n.chunk_id.clone()).collect(),
            contradiction_flags: all_neighbors.iter().map(|n| n.is_contradicting).collect(),
            cluster_id: None,
            stop_reason,
            iterations_used,
        })
    }

    /// Compute context embedding by averaging neighbor embeddings,
    /// optionally weighted by similarity. For Variant A, also factor
    /// in the query embedding.
    fn compute_context_embedding(
        &self,
        _base: &[f32],
        neighbors: &[NeighborContext],
        query_context: Option<&str>,
    ) -> Result<Vec<f32>> {
        let dim = self.config.embed_dim;

        if neighbors.is_empty() {
            return Ok(vec![0.0f32; dim]);
        }

        // Weighted average of neighbor embeddings (weight = similarity)
        let mut ctx = vec![0.0f32; dim];
        let mut total_weight = 0.0f32;

        for n in neighbors {
            if n.is_contradicting {
                continue; // Skip contradicting neighbors for context
            }
            let w = n.similarity.max(0.0);
            for (i, &val) in n.embedding.iter().enumerate() {
                if i < dim {
                    ctx[i] += val * w;
                }
            }
            total_weight += w;
        }

        if total_weight > 0.0 {
            for v in ctx.iter_mut() {
                *v /= total_weight;
            }
        }

        // Variant A: blend with query embedding
        if let (EmbeddingVariant::QueryConditioned, Some(query)) =
            (self.config.variant, query_context)
        {
            let query_emb = self.embedder.embed(query)?;
            let query_weight = 0.3;
            for (i, v) in ctx.iter_mut().enumerate() {
                if i < query_emb.len() {
                    *v = *v * (1.0 - query_weight) + query_emb[i] * query_weight;
                }
            }
        }

        Ok(ctx)
    }

    /// Compute anti-cluster embedding from contradicting neighbors.
    fn compute_anti_embedding(&self, contradicting: &[&NeighborContext]) -> Result<Vec<f32>> {
        let dim = self.config.embed_dim;
        let mut anti = vec![0.0f32; dim];
        let count = contradicting.len() as f32;

        if count == 0.0 {
            return Ok(anti);
        }

        for n in contradicting {
            for (i, &val) in n.embedding.iter().enumerate() {
                if i < dim {
                    anti[i] += val;
                }
            }
        }

        for v in anti.iter_mut() {
            *v /= count;
        }

        Ok(anti)
    }

    /// Merge base, context, and anti-cluster embeddings using the auditable merge rule.
    ///
    /// `anti_sign` controls whether anti pushes away (+1.0) or toward (-1.0).
    /// For twin embedding Variant C, the second twin uses anti_sign = -1.0.
    fn merge_embedding(&self, base: &[f32], ctx: &[f32], anti: &[f32], anti_sign: f32) -> Vec<f32> {
        let dim = self.config.embed_dim;
        let mut merged = vec![0.0f32; dim];

        for i in 0..dim {
            let b = if i < base.len() { base[i] } else { 0.0 };
            let c = if i < ctx.len() { ctx[i] } else { 0.0 };
            let a = if i < anti.len() { anti[i] } else { 0.0 };
            merged[i] = self.config.w_base * b
                + self.config.w_context * c
                + self.config.w_anti * anti_sign * a;
        }

        l2_normalize(&mut merged);
        merged
    }

    /// Get the current configuration.
    pub fn config(&self) -> &RlmEmbedderConfig {
        &self.config
    }
}

// ============================================================================
// Appliance Configuration (Pi 5 + STM32 — AD-25)
// ============================================================================

/// Appliance-specific configuration preset for Pi 5 + 7 STM32 deployment.
///
/// Memory budget: ~512 MB total for embeddings on Pi 5 (8GB model).
/// Latency target: < 50ms per embedding (2 iterations).
/// STM32s handle: hash computation, neighbor pre-filtering, watchdog.
impl RlmEmbedderConfig {
    /// Configuration optimized for Raspberry Pi 5 (Cortex-A76, 8GB).
    ///
    /// - 384-dim embeddings (MiniLM-L6-v2 compatible)
    /// - 2 iterations max (keeps latency under 50ms)
    /// - 3 neighbors (reduces retrieval overhead)
    /// - Aggressive convergence threshold (early exit)
    pub fn pi5_optimized() -> Self {
        Self {
            embed_dim: 384,
            max_iterations: 2,
            convergence_threshold: 0.95, // More aggressive early exit
            num_neighbors: 3,            // Fewer neighbors = faster retrieval
            w_base: 0.65,
            w_context: 0.25,
            w_anti: 0.10,
            contradiction_threshold: 0.3,
            variant: EmbeddingVariant::CorpusConditioned,
        }
    }

    /// Ultra-low-latency configuration for streaming ingestion on Pi 5.
    ///
    /// - Single iteration only
    /// - 2 neighbors
    /// - Suitable for real-time embedding during data ingestion
    pub fn pi5_streaming() -> Self {
        Self {
            embed_dim: 384,
            max_iterations: 1,
            convergence_threshold: 0.99,
            num_neighbors: 2,
            w_base: 0.7,
            w_context: 0.2,
            w_anti: 0.1,
            contradiction_threshold: 0.3,
            variant: EmbeddingVariant::CorpusConditioned,
        }
    }
}

// ============================================================================
// STM32 Offload Protocol
// ============================================================================

/// Command sent from Pi 5 to an STM32 coprocessor.
///
/// STM32s handle low-level compute tasks: hashing, gating, neighbor
/// pre-filtering, and watchdog monitoring. Communication is via
/// UART/SPI/I2C at the protocol level.
#[derive(Debug, Clone)]
pub enum Stm32Command {
    /// Compute a 64-bit hash of the given data for dedup detection.
    /// STM32 returns the hash via `Stm32Response::Hash`.
    ComputeHash { data: Vec<u8> },

    /// Pre-filter neighbor candidates by hash proximity.
    /// STM32 returns candidate indices that pass the hash filter.
    FilterNeighbors {
        target_hash: u64,
        candidate_hashes: Vec<u64>,
        max_candidates: usize,
    },

    /// Gate decision: should this chunk be embedded or skipped?
    /// Based on hash dedup, staleness, and priority.
    GateCheck {
        chunk_hash: u64,
        priority: u8,
        age_seconds: u32,
    },

    /// Watchdog ping — STM32 monitors embedding latency and raises
    /// alert if a single embedding exceeds the timeout.
    WatchdogPing { timeout_ms: u32 },

    /// Scheduling hint: reorder pending embedding jobs by priority.
    ScheduleReorder { job_priorities: Vec<(usize, u8)> },
}

/// Response from an STM32 coprocessor.
#[derive(Debug, Clone)]
pub enum Stm32Response {
    /// 64-bit hash result
    Hash(u64),
    /// Filtered candidate indices
    FilteredIndices(Vec<usize>),
    /// Gate decision: true = proceed with embedding, false = skip
    GatePass(bool),
    /// Watchdog acknowledged
    WatchdogAck,
    /// Reordered job indices
    ScheduleOrder(Vec<usize>),
    /// Error from STM32
    Error(String),
}

/// Trait for STM32 coprocessor communication.
///
/// Implementations handle the actual UART/SPI/I2C transport.
/// A `NullStm32` no-op implementation is provided for environments
/// without STM32 hardware (development, testing, cloud).
pub trait Stm32Offload {
    fn send_command(&self, command: Stm32Command) -> Result<Stm32Response>;
}

/// No-op STM32 offload — returns sensible defaults for all commands.
/// Used when running without STM32 hardware.
pub struct NullStm32;

impl Stm32Offload for NullStm32 {
    fn send_command(&self, command: Stm32Command) -> Result<Stm32Response> {
        match command {
            Stm32Command::ComputeHash { data } => Ok(Stm32Response::Hash(simple_hash(&data))),
            Stm32Command::FilterNeighbors {
                candidate_hashes,
                max_candidates,
                ..
            } => {
                let indices: Vec<usize> = (0..candidate_hashes.len().min(max_candidates)).collect();
                Ok(Stm32Response::FilteredIndices(indices))
            }
            Stm32Command::GateCheck { .. } => Ok(Stm32Response::GatePass(true)),
            Stm32Command::WatchdogPing { .. } => Ok(Stm32Response::WatchdogAck),
            Stm32Command::ScheduleReorder { mut job_priorities } => {
                job_priorities.sort_by(|a, b| b.1.cmp(&a.1));
                let order = job_priorities.iter().map(|(idx, _)| *idx).collect();
                Ok(Stm32Response::ScheduleOrder(order))
            }
        }
    }
}

/// Simple 64-bit hash (FNV-1a variant) for software fallback.
#[inline]
fn simple_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ============================================================================
// Batch Embedding
// ============================================================================

/// Result of batch embedding with per-chunk latency tracking.
pub struct BatchEmbeddingResult {
    /// Per-chunk results
    pub results: Vec<RlmEmbeddingResult>,
    /// Per-chunk latency in microseconds
    pub latencies_us: Vec<u64>,
    /// Total batch time in microseconds
    pub total_us: u64,
    /// Mean latency per chunk in microseconds
    pub mean_us: u64,
    /// Chunks skipped by gate check
    pub skipped: usize,
}

impl<E: BaseEmbedder, R: NeighborRetriever> RlmEmbedder<E, R> {
    /// Embed a batch of text chunks with latency tracking and optional
    /// STM32 gate-checking for dedup/priority filtering.
    pub fn embed_batch(
        &self,
        chunks: &[&str],
        query_context: Option<&str>,
        stm32: &dyn Stm32Offload,
    ) -> Result<BatchEmbeddingResult> {
        let batch_start = std::time::Instant::now();
        let mut results = Vec::with_capacity(chunks.len());
        let mut latencies = Vec::with_capacity(chunks.len());
        let mut skipped = 0;

        for &chunk in chunks {
            // Gate check via STM32
            let chunk_hash = simple_hash(chunk.as_bytes());
            let gate_response = stm32.send_command(Stm32Command::GateCheck {
                chunk_hash,
                priority: 128, // default priority
                age_seconds: 0,
            })?;

            if let Stm32Response::GatePass(false) = gate_response {
                skipped += 1;
                continue;
            }

            let chunk_start = std::time::Instant::now();
            let result = self.embed(chunk, query_context)?;
            let elapsed = chunk_start.elapsed().as_micros() as u64;

            latencies.push(elapsed);
            results.push(result);
        }

        let total_us = batch_start.elapsed().as_micros() as u64;
        let mean_us = if latencies.is_empty() {
            0
        } else {
            total_us / latencies.len() as u64
        };

        Ok(BatchEmbeddingResult {
            results,
            latencies_us: latencies,
            total_us,
            mean_us,
            skipped,
        })
    }

    /// Embed a batch with STM32-driven priority scheduling.
    /// Reorders chunks by priority before embedding.
    pub fn embed_batch_scheduled(
        &self,
        chunks: &[(&str, u8)], // (text, priority)
        query_context: Option<&str>,
        stm32: &dyn Stm32Offload,
    ) -> Result<BatchEmbeddingResult> {
        // Ask STM32 to determine optimal processing order
        let priorities: Vec<(usize, u8)> = chunks
            .iter()
            .enumerate()
            .map(|(i, (_, p))| (i, *p))
            .collect();
        let order_response = stm32.send_command(Stm32Command::ScheduleReorder {
            job_priorities: priorities,
        })?;

        let order = match order_response {
            Stm32Response::ScheduleOrder(o) => o,
            _ => (0..chunks.len()).collect(),
        };

        let ordered_chunks: Vec<&str> = order
            .iter()
            .filter_map(|&i| chunks.get(i).map(|(text, _)| *text))
            .collect();

        self.embed_batch(&ordered_chunks, query_context, stm32)
    }
}

// ============================================================================
// Lightweight Hash-Based Embedder (for testing / ultra-low resource)
// ============================================================================

/// A hash-based pseudo-embedder that produces deterministic embeddings
/// from text using a simple hash function. NOT a real language model —
/// this is for testing, benchmarking, and as a baseline.
///
/// On Pi 5: ~0.1ms per embedding (just hashing + normalize).
pub struct HashEmbedder {
    dim: usize,
}

impl HashEmbedder {
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }
}

impl BaseEmbedder for HashEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut emb = vec![0.0f32; self.dim];
        let bytes = text.as_bytes();

        // FNV-1a hash with dimensional rotation
        let mut state: u64 = 0xcbf29ce484222325;
        for (i, &byte) in bytes.iter().enumerate() {
            state ^= byte as u64;
            state = state.wrapping_mul(0x100000001b3);
            // Distribute hash bits across embedding dimensions
            let dim_idx = i % self.dim;
            let val = ((state >> 16) as i32 as f32) / (i32::MAX as f32);
            emb[dim_idx] += val;
        }

        // Character n-gram features (bigrams)
        if bytes.len() >= 2 {
            for window in bytes.windows(2) {
                let bigram_hash = (window[0] as u64) * 256 + window[1] as u64;
                let dim_idx = (bigram_hash as usize) % self.dim;
                emb[dim_idx] += 0.1;
            }
        }

        l2_normalize(&mut emb);
        Ok(emb)
    }

    fn embed_dim(&self) -> usize {
        self.dim
    }
}

// ============================================================================
// In-Memory Neighbor Store (for testing / small corpora)
// ============================================================================

/// Simple in-memory neighbor retriever backed by a flat vector store.
/// Suitable for small corpora (< 100K chunks) on Pi 5.
///
/// For larger corpora, use RuVector's HNSW index as the retriever.
pub struct FlatNeighborStore {
    chunks: Vec<StoredChunk>,
    dim: usize,
}

/// A chunk stored in the flat neighbor store.
#[derive(Clone)]
struct StoredChunk {
    id: String,
    embedding: Vec<f32>,
    cluster_id: Option<usize>,
}

impl FlatNeighborStore {
    pub fn new(dim: usize) -> Self {
        Self {
            chunks: Vec::new(),
            dim,
        }
    }

    /// Add a chunk with its pre-computed embedding and optional cluster.
    pub fn add(&mut self, id: &str, embedding: Vec<f32>, cluster_id: Option<usize>) {
        self.chunks.push(StoredChunk {
            id: id.to_string(),
            embedding,
            cluster_id,
        });
    }

    /// Number of stored chunks.
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Memory usage in bytes (approximate).
    pub fn memory_bytes(&self) -> usize {
        self.chunks.len() * (self.dim * 4 + 64) // embedding + overhead
    }
}

impl NeighborRetriever for FlatNeighborStore {
    fn retrieve(&self, embedding: &[f32], k: usize) -> Result<Vec<NeighborContext>> {
        if self.chunks.is_empty() {
            return Ok(Vec::new());
        }

        // Compute similarities to all stored chunks
        let mut scored: Vec<(usize, f32)> = self
            .chunks
            .iter()
            .enumerate()
            .map(|(i, chunk)| (i, cosine_similarity(embedding, &chunk.embedding)))
            .collect();

        // Sort by descending similarity
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Return top-k
        let results: Vec<NeighborContext> = scored
            .into_iter()
            .take(k)
            .map(|(idx, sim)| {
                let chunk = &self.chunks[idx];
                // Detect contradiction: different cluster from most similar chunk
                let is_contradicting = if let (Some(query_cluster), Some(chunk_cluster)) = (
                    self.chunks.first().and_then(|c| c.cluster_id),
                    chunk.cluster_id,
                ) {
                    query_cluster != chunk_cluster
                } else {
                    false
                };

                NeighborContext {
                    chunk_id: chunk.id.clone(),
                    embedding: chunk.embedding.clone(),
                    is_contradicting,
                    similarity: sim,
                }
            })
            .collect();

        Ok(results)
    }
}

// ============================================================================
// Appliance Benchmark
// ============================================================================

/// Benchmark results for the RLM embedder on target hardware.
pub struct EmbedderBenchmark {
    /// Embeddings per second
    pub throughput: f64,
    /// Mean latency per embedding in microseconds
    pub mean_latency_us: u64,
    /// P95 latency in microseconds
    pub p95_latency_us: u64,
    /// P99 latency in microseconds
    pub p99_latency_us: u64,
    /// Peak memory usage in bytes (estimated)
    pub peak_memory_bytes: usize,
    /// Number of embeddings computed
    pub count: usize,
}

impl EmbedderBenchmark {
    /// Run a benchmark with the given embedder, store, and test corpus.
    pub fn run<E: BaseEmbedder, R: NeighborRetriever>(
        embedder: &RlmEmbedder<E, R>,
        test_texts: &[&str],
        warmup: usize,
    ) -> Result<Self> {
        // Warmup
        for &text in test_texts.iter().take(warmup) {
            let _ = embedder.embed(text, None)?;
        }

        // Timed run
        let mut latencies: Vec<u64> = Vec::with_capacity(test_texts.len());

        let start = std::time::Instant::now();
        for &text in test_texts {
            let t = std::time::Instant::now();
            let _ = embedder.embed(text, None)?;
            latencies.push(t.elapsed().as_micros() as u64);
        }
        let total = start.elapsed();

        latencies.sort();
        let count = latencies.len();
        let mean_latency_us = if count > 0 {
            latencies.iter().sum::<u64>() / count as u64
        } else {
            0
        };
        let p95_latency_us = if count > 0 {
            latencies[(count * 95 / 100).min(count - 1)]
        } else {
            0
        };
        let p99_latency_us = if count > 0 {
            latencies[(count * 99 / 100).min(count - 1)]
        } else {
            0
        };

        let throughput = if total.as_secs_f64() > 0.0 {
            count as f64 / total.as_secs_f64()
        } else {
            0.0
        };

        // Estimate peak memory: dim * 4 bytes * (neighbors + iterations + buffers)
        let dim = embedder.config().embed_dim;
        let max_iter = embedder.config().max_iterations;
        let max_neighbors = embedder.config().num_neighbors;
        let peak_memory_bytes = dim * 4 * (max_neighbors + max_iter * 3 + 4);

        Ok(Self {
            throughput,
            mean_latency_us,
            p95_latency_us,
            p99_latency_us,
            peak_memory_bytes,
            count,
        })
    }

    /// Human-readable report.
    pub fn report(&self) -> String {
        format!(
            "RLM Embedder Benchmark\n\
             ======================\n\
             Embeddings:    {}\n\
             Throughput:    {:.1} emb/s\n\
             Mean latency:  {} us\n\
             P95 latency:   {} us\n\
             P99 latency:   {} us\n\
             Peak memory:   {} bytes ({:.1} KB)",
            self.count,
            self.throughput,
            self.mean_latency_us,
            self.p95_latency_us,
            self.p99_latency_us,
            self.peak_memory_bytes,
            self.peak_memory_bytes as f64 / 1024.0
        )
    }
}

// ============================================================================
// Math Helpers (NEON-optimizable hot paths)
// ============================================================================

/// Cosine similarity between two vectors.
///
/// This is the #1 hot path in the embedder. On aarch64, the compiler
/// auto-vectorizes this loop to NEON instructions with `-C target-feature=+neon`.
#[inline]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    // Process 4 elements at a time for auto-vectorization
    let chunks = len / 4;
    let remainder = len % 4;

    for i in 0..chunks {
        let base = i * 4;
        let a0 = a[base];
        let a1 = a[base + 1];
        let a2 = a[base + 2];
        let a3 = a[base + 3];
        let b0 = b[base];
        let b1 = b[base + 1];
        let b2 = b[base + 2];
        let b3 = b[base + 3];

        dot += a0 * b0 + a1 * b1 + a2 * b2 + a3 * b3;
        norm_a += a0 * a0 + a1 * a1 + a2 * a2 + a3 * a3;
        norm_b += b0 * b0 + b1 * b1 + b2 * b2 + b3 * b3;
    }

    let tail_start = chunks * 4;
    for i in 0..remainder {
        let idx = tail_start + i;
        dot += a[idx] * b[idx];
        norm_a += a[idx] * a[idx];
        norm_b += b[idx] * b[idx];
    }

    let denom = (norm_a.sqrt() * norm_b.sqrt()).max(1e-10);
    dot / denom
}

/// L2 normalize a vector in-place.
///
/// Auto-vectorizes on aarch64 with NEON.
#[inline]
pub fn l2_normalize(v: &mut [f32]) {
    let mut norm = 0.0f32;

    // Unrolled accumulation for auto-vectorization
    let chunks = v.len() / 4;
    let remainder = v.len() % 4;

    for i in 0..chunks {
        let base = i * 4;
        norm += v[base] * v[base]
            + v[base + 1] * v[base + 1]
            + v[base + 2] * v[base + 2]
            + v[base + 3] * v[base + 3];
    }
    for i in 0..remainder {
        let idx = chunks * 4 + i;
        norm += v[idx] * v[idx];
    }

    let inv_norm = 1.0 / norm.sqrt().max(1e-10);
    for x in v.iter_mut() {
        *x *= inv_norm;
    }
}

/// Weighted vector accumulate: dst[i] += src[i] * weight.
///
/// Used in context embedding computation. Auto-vectorizes.
#[inline]
pub fn vec_accumulate_weighted(dst: &mut [f32], src: &[f32], weight: f32) {
    let len = dst.len().min(src.len());
    for i in 0..len {
        dst[i] += src[i] * weight;
    }
}

/// Compute the mean of a set of embeddings.
pub fn mean_embedding(embeddings: &[&[f32]], dim: usize) -> Vec<f32> {
    let mut result = vec![0.0f32; dim];
    if embeddings.is_empty() {
        return result;
    }
    let count = embeddings.len() as f32;
    for emb in embeddings {
        vec_accumulate_weighted(&mut result, emb, 1.0);
    }
    let inv_count = 1.0 / count;
    for v in result.iter_mut() {
        *v *= inv_count;
    }
    result
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Test implementations of traits --

    struct MockEmbedder {
        dim: usize,
    }

    impl BaseEmbedder for MockEmbedder {
        fn embed(&self, text: &str) -> Result<Vec<f32>> {
            // Deterministic embedding: hash text bytes into a vector
            let mut emb = vec![0.0f32; self.dim];
            for (i, byte) in text.bytes().enumerate() {
                emb[i % self.dim] += (byte as f32 - 128.0) / 128.0;
            }
            l2_normalize(&mut emb);
            Ok(emb)
        }

        fn embed_dim(&self) -> usize {
            self.dim
        }
    }

    struct MockRetriever {
        neighbors: Vec<NeighborContext>,
    }

    impl NeighborRetriever for MockRetriever {
        fn retrieve(&self, _embedding: &[f32], k: usize) -> Result<Vec<NeighborContext>> {
            Ok(self.neighbors.iter().take(k).cloned().collect())
        }
    }

    fn make_neighbor(id: &str, dim: usize, is_contradicting: bool, sim: f32) -> NeighborContext {
        let mut emb = vec![0.0f32; dim];
        // Deterministic based on id
        for (i, byte) in id.bytes().enumerate() {
            emb[i % dim] = (byte as f32 - 100.0) / 100.0;
        }
        l2_normalize(&mut emb);
        NeighborContext {
            chunk_id: id.to_string(),
            embedding: emb,
            is_contradicting,
            similarity: sim,
        }
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize() {
        let mut v = vec![3.0, 4.0];
        l2_normalize(&mut v);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize_zero_vector() {
        let mut v = vec![0.0, 0.0, 0.0];
        l2_normalize(&mut v);
        // Should not panic, values stay near zero
        assert!(v.iter().all(|&x| x.abs() < 1e-5));
    }

    #[test]
    fn test_mean_embedding() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let mean = mean_embedding(&[&a, &b], 2);
        assert!((mean[0] - 0.5).abs() < 1e-6);
        assert!((mean[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_embed_corpus_conditioned() {
        let dim = 8;
        let embedder = MockEmbedder { dim };
        let retriever = MockRetriever {
            neighbors: vec![
                make_neighbor("doc-1", dim, false, 0.9),
                make_neighbor("doc-2", dim, false, 0.8),
            ],
        };
        let config = RlmEmbedderConfig {
            embed_dim: dim,
            max_iterations: 2,
            variant: EmbeddingVariant::CorpusConditioned,
            ..Default::default()
        };

        let rlm = RlmEmbedder::new(embedder, retriever, config);
        let result = rlm.embed("test chunk text", None).unwrap();

        assert_eq!(result.embedding.len(), dim);
        assert!(result.confidence > 0.0);
        assert_eq!(result.evidence_neighbor_ids.len(), 2);
        assert!(result.twin_embedding.is_none());
        assert!(result.iterations_used <= 2);
    }

    #[test]
    fn test_embed_query_conditioned() {
        let dim = 8;
        let embedder = MockEmbedder { dim };
        let retriever = MockRetriever {
            neighbors: vec![make_neighbor("doc-1", dim, false, 0.9)],
        };
        let config = RlmEmbedderConfig {
            embed_dim: dim,
            max_iterations: 2,
            variant: EmbeddingVariant::QueryConditioned,
            ..Default::default()
        };

        let rlm = RlmEmbedder::new(embedder, retriever, config);
        let result = rlm.embed("chunk", Some("what is X?")).unwrap();

        assert_eq!(result.embedding.len(), dim);
        assert!(result.twin_embedding.is_none());
    }

    #[test]
    fn test_embed_contradiction_aware_twin() {
        let dim = 8;
        let embedder = MockEmbedder { dim };
        let retriever = MockRetriever {
            neighbors: vec![
                make_neighbor("agree-1", dim, false, 0.9),
                make_neighbor("contra-1", dim, true, 0.7),
            ],
        };
        let config = RlmEmbedderConfig {
            embed_dim: dim,
            max_iterations: 2,
            variant: EmbeddingVariant::ContradictionAwareTwin,
            ..Default::default()
        };

        let rlm = RlmEmbedder::new(embedder, retriever, config);
        let result = rlm.embed("contested claim", None).unwrap();

        assert_eq!(result.embedding.len(), dim);
        assert!(result.twin_embedding.is_some());
        assert_eq!(result.stop_reason, EmbedStopReason::Contested);

        // Twin embeddings should differ
        let twin = result.twin_embedding.as_ref().unwrap();
        let sim = cosine_similarity(&result.embedding, twin);
        assert!(
            sim < 0.99,
            "Twin embeddings should differ, got cosine={}",
            sim
        );
    }

    #[test]
    fn test_embed_no_neighbors() {
        let dim = 8;
        let embedder = MockEmbedder { dim };
        let retriever = MockRetriever { neighbors: vec![] };
        let config = RlmEmbedderConfig {
            embed_dim: dim,
            max_iterations: 2,
            variant: EmbeddingVariant::CorpusConditioned,
            ..Default::default()
        };

        let rlm = RlmEmbedder::new(embedder, retriever, config);
        let result = rlm.embed("isolated chunk", None).unwrap();

        assert_eq!(result.embedding.len(), dim);
        assert!(result.evidence_neighbor_ids.is_empty());
    }

    #[test]
    fn test_embed_convergence_stops_early() {
        let dim = 8;
        let embedder = MockEmbedder { dim };
        // Same neighbor every time → should converge quickly
        let retriever = MockRetriever {
            neighbors: vec![make_neighbor("stable-1", dim, false, 0.95)],
        };
        let config = RlmEmbedderConfig {
            embed_dim: dim,
            max_iterations: 10, // High max, but should converge before
            convergence_threshold: 0.95,
            variant: EmbeddingVariant::CorpusConditioned,
            ..Default::default()
        };

        let rlm = RlmEmbedder::new(embedder, retriever, config);
        let result = rlm.embed("converging chunk", None).unwrap();

        // Should stop before 10 iterations
        assert!(result.iterations_used < 10);
        assert_eq!(result.stop_reason, EmbedStopReason::Converged);
    }

    #[test]
    fn test_embed_output_is_normalized() {
        let dim = 8;
        let embedder = MockEmbedder { dim };
        let retriever = MockRetriever {
            neighbors: vec![make_neighbor("doc-1", dim, false, 0.8)],
        };
        let config = RlmEmbedderConfig {
            embed_dim: dim,
            ..Default::default()
        };

        let rlm = RlmEmbedder::new(embedder, retriever, config);
        let result = rlm.embed("test", None).unwrap();

        let norm: f32 = result.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-4,
            "Output embedding should be L2-normalized, got norm={}",
            norm
        );
    }

    #[test]
    fn test_contradiction_flags_populated() {
        let dim = 8;
        let embedder = MockEmbedder { dim };
        let retriever = MockRetriever {
            neighbors: vec![
                make_neighbor("agree", dim, false, 0.9),
                make_neighbor("contra", dim, true, 0.7),
                make_neighbor("agree2", dim, false, 0.6),
            ],
        };
        let config = RlmEmbedderConfig {
            embed_dim: dim,
            max_iterations: 1,
            variant: EmbeddingVariant::CorpusConditioned,
            ..Default::default()
        };

        let rlm = RlmEmbedder::new(embedder, retriever, config);
        let result = rlm.embed("chunk", None).unwrap();

        assert_eq!(result.contradiction_flags.len(), 3);
        assert!(!result.contradiction_flags[0]); // agree
        assert!(result.contradiction_flags[1]); // contra
        assert!(!result.contradiction_flags[2]); // agree2
    }

    #[test]
    fn test_embedding_result_metadata() {
        let dim = 4;
        let embedder = MockEmbedder { dim };
        let retriever = MockRetriever {
            neighbors: vec![make_neighbor("n1", dim, false, 0.5)],
        };
        let config = RlmEmbedderConfig {
            embed_dim: dim,
            max_iterations: 2,
            variant: EmbeddingVariant::CorpusConditioned,
            ..Default::default()
        };

        let rlm = RlmEmbedder::new(embedder, retriever, config);
        let result = rlm.embed("meta test", None).unwrap();

        assert!(!result.evidence_neighbor_ids.is_empty());
        assert!(result.confidence >= -1.0 && result.confidence <= 1.0);
        assert!(result.iterations_used >= 1);
    }

    // ================================================================
    // Appliance config presets
    // ================================================================

    #[test]
    fn test_pi5_optimized_config() {
        let cfg = RlmEmbedderConfig::pi5_optimized();
        assert_eq!(cfg.embed_dim, 384);
        assert_eq!(cfg.max_iterations, 2);
        assert_eq!(cfg.num_neighbors, 3);
        assert!(cfg.convergence_threshold < 1.0);
        // Weight sum should be 1.0
        let sum = cfg.w_base + cfg.w_context + cfg.w_anti;
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "Weights should sum to 1.0, got {}",
            sum
        );
    }

    #[test]
    fn test_pi5_streaming_config() {
        let cfg = RlmEmbedderConfig::pi5_streaming();
        assert_eq!(cfg.embed_dim, 384);
        assert_eq!(cfg.max_iterations, 1);
        assert_eq!(cfg.num_neighbors, 2);
        // Streaming should be faster than optimized: fewer iterations + neighbors
        let opt = RlmEmbedderConfig::pi5_optimized();
        assert!(cfg.max_iterations <= opt.max_iterations);
        assert!(cfg.num_neighbors <= opt.num_neighbors);
        let sum = cfg.w_base + cfg.w_context + cfg.w_anti;
        assert!((sum - 1.0).abs() < 1e-6);
    }

    // ================================================================
    // STM32 offload protocol (NullStm32)
    // ================================================================

    #[test]
    fn test_null_stm32_compute_hash() {
        let stm32 = NullStm32;
        let resp = stm32
            .send_command(Stm32Command::ComputeHash {
                data: b"hello world".to_vec(),
            })
            .unwrap();
        match resp {
            Stm32Response::Hash(h) => assert_ne!(h, 0),
            other => panic!("Expected Hash, got {:?}", other),
        }
    }

    #[test]
    fn test_null_stm32_hash_deterministic() {
        let stm32 = NullStm32;
        let h1 = match stm32
            .send_command(Stm32Command::ComputeHash {
                data: b"test".to_vec(),
            })
            .unwrap()
        {
            Stm32Response::Hash(h) => h,
            _ => panic!("Expected Hash"),
        };
        let h2 = match stm32
            .send_command(Stm32Command::ComputeHash {
                data: b"test".to_vec(),
            })
            .unwrap()
        {
            Stm32Response::Hash(h) => h,
            _ => panic!("Expected Hash"),
        };
        assert_eq!(h1, h2, "Hash should be deterministic");
    }

    #[test]
    fn test_null_stm32_hash_distinct() {
        let stm32 = NullStm32;
        let h1 = match stm32
            .send_command(Stm32Command::ComputeHash {
                data: b"alpha".to_vec(),
            })
            .unwrap()
        {
            Stm32Response::Hash(h) => h,
            _ => panic!("Expected Hash"),
        };
        let h2 = match stm32
            .send_command(Stm32Command::ComputeHash {
                data: b"beta".to_vec(),
            })
            .unwrap()
        {
            Stm32Response::Hash(h) => h,
            _ => panic!("Expected Hash"),
        };
        assert_ne!(h1, h2, "Different inputs should produce different hashes");
    }

    #[test]
    fn test_null_stm32_filter_neighbors() {
        let stm32 = NullStm32;
        let resp = stm32
            .send_command(Stm32Command::FilterNeighbors {
                target_hash: 42,
                candidate_hashes: vec![10, 20, 30, 40, 50],
                max_candidates: 3,
            })
            .unwrap();
        match resp {
            Stm32Response::FilteredIndices(indices) => {
                assert_eq!(indices.len(), 3);
                assert_eq!(indices, vec![0, 1, 2]);
            }
            other => panic!("Expected FilteredIndices, got {:?}", other),
        }
    }

    #[test]
    fn test_null_stm32_gate_check_always_passes() {
        let stm32 = NullStm32;
        let resp = stm32
            .send_command(Stm32Command::GateCheck {
                chunk_hash: 123,
                priority: 128,
                age_seconds: 0,
            })
            .unwrap();
        match resp {
            Stm32Response::GatePass(pass) => assert!(pass),
            other => panic!("Expected GatePass, got {:?}", other),
        }
    }

    #[test]
    fn test_null_stm32_watchdog_ack() {
        let stm32 = NullStm32;
        let resp = stm32
            .send_command(Stm32Command::WatchdogPing { timeout_ms: 50 })
            .unwrap();
        match resp {
            Stm32Response::WatchdogAck => {}
            other => panic!("Expected WatchdogAck, got {:?}", other),
        }
    }

    #[test]
    fn test_null_stm32_schedule_reorder_by_priority() {
        let stm32 = NullStm32;
        let resp = stm32
            .send_command(Stm32Command::ScheduleReorder {
                job_priorities: vec![(0, 10), (1, 90), (2, 50)],
            })
            .unwrap();
        match resp {
            Stm32Response::ScheduleOrder(order) => {
                // Highest priority first: job 1 (90), job 2 (50), job 0 (10)
                assert_eq!(order, vec![1, 2, 0]);
            }
            other => panic!("Expected ScheduleOrder, got {:?}", other),
        }
    }

    // ================================================================
    // simple_hash
    // ================================================================

    #[test]
    fn test_simple_hash_fnv1a() {
        let h1 = simple_hash(b"");
        let h2 = simple_hash(b"a");
        let h3 = simple_hash(b"b");
        assert_ne!(h1, h2);
        assert_ne!(h2, h3);
        // FNV-1a offset basis for empty input
        assert_eq!(h1, 0xcbf29ce484222325);
    }

    // ================================================================
    // HashEmbedder
    // ================================================================

    #[test]
    fn test_hash_embedder_dim() {
        let he = HashEmbedder::new(128);
        assert_eq!(he.embed_dim(), 128);
    }

    #[test]
    fn test_hash_embedder_output_normalized() {
        let he = HashEmbedder::new(64);
        let emb = he.embed("some text for embedding").unwrap();
        assert_eq!(emb.len(), 64);
        let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-4,
            "HashEmbedder output should be L2-normalized, got norm={}",
            norm
        );
    }

    #[test]
    fn test_hash_embedder_deterministic() {
        let he = HashEmbedder::new(32);
        let e1 = he.embed("determinism check").unwrap();
        let e2 = he.embed("determinism check").unwrap();
        assert_eq!(e1, e2);
    }

    #[test]
    fn test_hash_embedder_distinct_inputs() {
        let he = HashEmbedder::new(32);
        let e1 = he.embed("alpha text").unwrap();
        let e2 = he.embed("beta text").unwrap();
        let sim = cosine_similarity(&e1, &e2);
        assert!(
            sim < 0.99,
            "Different texts should produce different embeddings, cosine={}",
            sim
        );
    }

    // ================================================================
    // FlatNeighborStore
    // ================================================================

    #[test]
    fn test_flat_neighbor_store_empty() {
        let store = FlatNeighborStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        let results = store.retrieve(&[0.0; 8], 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_flat_neighbor_store_add_and_retrieve() {
        let mut store = FlatNeighborStore::new(4);
        let mut emb1 = vec![1.0, 0.0, 0.0, 0.0];
        l2_normalize(&mut emb1);
        let mut emb2 = vec![0.0, 1.0, 0.0, 0.0];
        l2_normalize(&mut emb2);
        let mut emb3 = vec![0.9, 0.1, 0.0, 0.0];
        l2_normalize(&mut emb3);

        store.add("chunk-1", emb1.clone(), None);
        store.add("chunk-2", emb2, None);
        store.add("chunk-3", emb3, None);

        assert_eq!(store.len(), 3);
        assert!(!store.is_empty());

        // Query closest to chunk-1
        let results = store.retrieve(&emb1, 2).unwrap();
        assert_eq!(results.len(), 2);
        // First result should be chunk-1 (exact match)
        assert_eq!(results[0].chunk_id, "chunk-1");
        assert!((results[0].similarity - 1.0).abs() < 1e-4);
        // Second should be chunk-3 (most similar to chunk-1)
        assert_eq!(results[1].chunk_id, "chunk-3");
    }

    #[test]
    fn test_flat_neighbor_store_memory_bytes() {
        let mut store = FlatNeighborStore::new(384);
        for i in 0..100 {
            let emb = vec![0.1f32; 384];
            store.add(&format!("c-{}", i), emb, None);
        }
        let mem = store.memory_bytes();
        // 100 chunks * (384 * 4 + 64) = 100 * 1600 = 160_000
        assert_eq!(mem, 160_000);
    }

    // ================================================================
    // Batch embedding
    // ================================================================

    #[test]
    fn test_embed_batch_basic() {
        let dim = 8;
        let embedder = MockEmbedder { dim };
        let retriever = MockRetriever {
            neighbors: vec![make_neighbor("n1", dim, false, 0.8)],
        };
        let config = RlmEmbedderConfig {
            embed_dim: dim,
            max_iterations: 1,
            ..Default::default()
        };
        let rlm = RlmEmbedder::new(embedder, retriever, config);
        let stm32 = NullStm32;

        let chunks = vec!["chunk one", "chunk two", "chunk three"];
        let batch = rlm.embed_batch(&chunks, None, &stm32).unwrap();

        assert_eq!(batch.results.len(), 3);
        assert_eq!(batch.latencies_us.len(), 3);
        assert_eq!(batch.skipped, 0);
        assert!(batch.total_us > 0);
        assert!(batch.mean_us > 0);
    }

    #[test]
    fn test_embed_batch_scheduled_priority_order() {
        let dim = 8;
        let embedder = MockEmbedder { dim };
        let retriever = MockRetriever {
            neighbors: vec![make_neighbor("n1", dim, false, 0.8)],
        };
        let config = RlmEmbedderConfig {
            embed_dim: dim,
            max_iterations: 1,
            ..Default::default()
        };
        let rlm = RlmEmbedder::new(embedder, retriever, config);
        let stm32 = NullStm32;

        // Different priorities: low, high, medium
        let chunks = vec![
            ("low priority", 10u8),
            ("high priority", 200),
            ("medium priority", 100),
        ];
        let batch = rlm.embed_batch_scheduled(&chunks, None, &stm32).unwrap();

        // All 3 should be processed
        assert_eq!(batch.results.len(), 3);
        assert_eq!(batch.skipped, 0);
    }

    // ================================================================
    // Benchmark
    // ================================================================

    #[test]
    fn test_embedder_benchmark_run() {
        let dim = 8;
        let embedder = MockEmbedder { dim };
        let retriever = MockRetriever {
            neighbors: vec![make_neighbor("n1", dim, false, 0.8)],
        };
        let config = RlmEmbedderConfig {
            embed_dim: dim,
            max_iterations: 1,
            num_neighbors: 2,
            ..Default::default()
        };
        let rlm = RlmEmbedder::new(embedder, retriever, config);

        let texts: Vec<&str> = vec![
            "text one",
            "text two",
            "text three",
            "text four",
            "text five",
        ];
        let bench = EmbedderBenchmark::run(&rlm, &texts, 1).unwrap();

        assert_eq!(bench.count, 5);
        assert!(bench.throughput > 0.0);
        assert!(bench.p95_latency_us >= bench.mean_latency_us || bench.count < 20);
        assert!(bench.peak_memory_bytes > 0);
    }

    #[test]
    fn test_embedder_benchmark_report_format() {
        let dim = 8;
        let embedder = MockEmbedder { dim };
        let retriever = MockRetriever {
            neighbors: vec![make_neighbor("n1", dim, false, 0.8)],
        };
        let config = RlmEmbedderConfig {
            embed_dim: dim,
            max_iterations: 1,
            ..Default::default()
        };
        let rlm = RlmEmbedder::new(embedder, retriever, config);

        let texts = vec!["a", "b", "c"];
        let bench = EmbedderBenchmark::run(&rlm, &texts, 0).unwrap();
        let report = bench.report();

        assert!(report.contains("RLM Embedder Benchmark"));
        assert!(report.contains("Throughput"));
        assert!(report.contains("P95"));
        assert!(report.contains("P99"));
    }

    // ================================================================
    // vec_accumulate_weighted
    // ================================================================

    #[test]
    fn test_vec_accumulate_weighted_basic() {
        let mut dst = vec![1.0, 2.0, 3.0];
        let src = vec![10.0, 20.0, 30.0];
        vec_accumulate_weighted(&mut dst, &src, 0.5);
        assert!((dst[0] - 6.0).abs() < 1e-6);
        assert!((dst[1] - 12.0).abs() < 1e-6);
        assert!((dst[2] - 18.0).abs() < 1e-6);
    }

    #[test]
    fn test_vec_accumulate_weighted_different_lengths() {
        let mut dst = vec![1.0, 2.0, 3.0, 4.0];
        let src = vec![10.0, 20.0]; // shorter
        vec_accumulate_weighted(&mut dst, &src, 1.0);
        assert!((dst[0] - 11.0).abs() < 1e-6);
        assert!((dst[1] - 22.0).abs() < 1e-6);
        assert!((dst[2] - 3.0).abs() < 1e-6); // untouched
        assert!((dst[3] - 4.0).abs() < 1e-6); // untouched
    }

    // ================================================================
    // Integration: HashEmbedder + FlatNeighborStore + RlmEmbedder
    // ================================================================

    #[test]
    fn test_full_appliance_pipeline() {
        let dim = 64;
        let he = HashEmbedder::new(dim);

        // Build a small corpus in the flat store
        let mut store = FlatNeighborStore::new(dim);
        let corpus = [
            "The CPU temperature is 42 degrees",
            "Memory usage stands at 3.2 GB",
            "Network latency measured at 12ms",
            "Disk throughput exceeds 500 MB/s",
            "GPU utilization is at 0 percent",
        ];
        for (i, text) in corpus.iter().enumerate() {
            let emb = he.embed(text).unwrap();
            store.add(&format!("corpus-{}", i), emb, Some(0));
        }

        let config = RlmEmbedderConfig {
            embed_dim: dim,
            max_iterations: 2,
            num_neighbors: 3,
            ..Default::default()
        };

        let rlm = RlmEmbedder::new(he, store, config);
        let result = rlm.embed("What is the CPU temperature?", None).unwrap();

        assert_eq!(result.embedding.len(), dim);
        // Should have found neighbors from corpus
        assert!(!result.evidence_neighbor_ids.is_empty());
        // Output should be normalized
        let norm: f32 = result.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_full_appliance_batch_pipeline() {
        let dim = 32;
        let he = HashEmbedder::new(dim);

        let mut store = FlatNeighborStore::new(dim);
        let corpus = ["doc alpha", "doc beta", "doc gamma"];
        for (i, text) in corpus.iter().enumerate() {
            let emb = he.embed(text).unwrap();
            store.add(&format!("d-{}", i), emb, None);
        }

        let config = RlmEmbedderConfig {
            embed_dim: dim,
            max_iterations: 1,
            num_neighbors: 2,
            ..Default::default()
        };

        let rlm = RlmEmbedder::new(he, store, config);
        let stm32 = NullStm32;

        let queries = vec!["query one", "query two"];
        let batch = rlm.embed_batch(&queries, None, &stm32).unwrap();

        assert_eq!(batch.results.len(), 2);
        assert_eq!(batch.skipped, 0);

        // All outputs normalized
        for r in &batch.results {
            let n: f32 = r.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((n - 1.0).abs() < 1e-4);
        }
    }

    // ================================================================
    // Cosine similarity with 4-element unrolled hot path
    // ================================================================

    #[test]
    fn test_cosine_similarity_large_vector() {
        // Tests the 4-element unrolled path + remainder path
        let n = 100; // 25 chunks of 4 + 0 remainder
        let a: Vec<f32> = (0..n).map(|i| (i as f32).sin()).collect();
        let b: Vec<f32> = (0..n).map(|i| (i as f32).cos()).collect();
        let sim = cosine_similarity(&a, &b);
        assert!(sim > -1.0 && sim < 1.0);

        // Self-similarity should be 1.0
        let self_sim = cosine_similarity(&a, &a);
        assert!((self_sim - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_non_multiple_of_4() {
        // 7 elements: 1 chunk of 4 + 3 remainder
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        let b = vec![7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        // dot = 7+12+15+16+15+12+7 = 84
        // norm_a = sqrt(1+4+9+16+25+36+49) = sqrt(140)
        // norm_b = sqrt(49+36+25+16+9+4+1) = sqrt(140)
        // cos = 84 / 140 = 0.6
        assert!((sim - 0.6).abs() < 1e-5, "Expected ~0.6, got {}", sim);
    }
}
