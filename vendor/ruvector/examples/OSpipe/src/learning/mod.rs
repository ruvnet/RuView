//! Continual learning for search improvement.
//!
//! This module integrates `ruvector-gnn` to provide:
//!
//! - **[`SearchLearner`]** -- records user relevance feedback and uses Elastic
//!   Weight Consolidation (EWC) to prevent catastrophic forgetting when the
//!   embedding model is fine-tuned over time.
//! - **[`EmbeddingQuantizer`]** -- compresses stored embeddings based on their
//!   age, trading precision for storage savings on cold data.
//!
//! Both structs compile to no-op stubs on `wasm32` targets where the native
//! `ruvector-gnn` crate is unavailable.

// ---------------------------------------------------------------------------
// Native implementation (non-WASM)
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use ruvector_gnn::compress::TensorCompress;
    use ruvector_gnn::ewc::ElasticWeightConsolidation;
    use ruvector_gnn::replay::ReplayBuffer;

    /// Minimum number of feedback entries before learning data is considered
    /// sufficient for a consolidation step.
    const MIN_FEEDBACK_ENTRIES: usize = 32;

    /// Records search relevance feedback and manages continual-learning state.
    ///
    /// Internally the learner maintains:
    /// - A [`ReplayBuffer`] that stores (query, result, relevance) triples via
    ///   reservoir sampling so old feedback is not forgotten.
    /// - An [`ElasticWeightConsolidation`] instance whose Fisher diagonal and
    ///   anchor weights track which embedding dimensions are important.
    /// - A simple parameter vector (`weights`) that represents a learned
    ///   relevance projection (one weight per embedding dimension).
    pub struct SearchLearner {
        replay_buffer: ReplayBuffer,
        ewc: ElasticWeightConsolidation,
        /// Learned relevance-projection weights (one per embedding dimension).
        weights: Vec<f32>,
    }

    impl SearchLearner {
        /// Create a new learner.
        ///
        /// # Arguments
        /// * `embedding_dim`   - Dimensionality of the embedding vectors.
        /// * `replay_capacity` - Maximum number of feedback entries retained.
        pub fn new(embedding_dim: usize, replay_capacity: usize) -> Self {
            Self {
                replay_buffer: ReplayBuffer::new(replay_capacity),
                ewc: ElasticWeightConsolidation::new(100.0),
                weights: vec![1.0; embedding_dim],
            }
        }

        /// Record a single piece of user feedback.
        ///
        /// The query and result embeddings are concatenated and stored in the
        /// replay buffer.  Positive feedback entries use `positive_ids = [1]`,
        /// negative ones use `positive_ids = [0]`, which allows downstream
        /// training loops to distinguish them.
        ///
        /// # Arguments
        /// * `query_embedding`  - Embedding of the search query.
        /// * `result_embedding` - Embedding of the search result.
        /// * `relevant`         - Whether the user considered the result relevant.
        pub fn record_feedback(
            &mut self,
            query_embedding: Vec<f32>,
            result_embedding: Vec<f32>,
            relevant: bool,
        ) {
            let mut combined = query_embedding;
            combined.extend_from_slice(&result_embedding);
            let positive_id: usize = if relevant { 1 } else { 0 };
            self.replay_buffer.add(&combined, &[positive_id]);
        }

        /// Return the current size of the replay buffer.
        pub fn replay_buffer_len(&self) -> usize {
            self.replay_buffer.len()
        }

        /// Returns `true` when the buffer contains enough data for a
        /// meaningful consolidation step (>= 32 entries).
        pub fn has_sufficient_data(&self) -> bool {
            self.replay_buffer.len() >= MIN_FEEDBACK_ENTRIES
        }

        /// Lock the current parameter state with EWC.
        ///
        /// This computes the Fisher information diagonal from sampled replay
        /// entries and saves the current weights as the EWC anchor.  Future
        /// EWC penalties will discourage large deviations from these weights.
        pub fn consolidate(&mut self) {
            if self.replay_buffer.is_empty() {
                return;
            }

            // Sample gradients -- we approximate them as the difference between
            // query and result portions of each stored entry.
            let samples = self.replay_buffer.sample(self.replay_buffer.len().min(64));

            let dim = self.weights.len();
            let gradients: Vec<Vec<f32>> = samples
                .iter()
                .filter_map(|entry| {
                    // Each entry stores [query || result]; extract gradient proxy.
                    if entry.query.len() >= dim * 2 {
                        let query_part = &entry.query[..dim];
                        let result_part = &entry.query[dim..dim * 2];
                        let grad: Vec<f32> = query_part
                            .iter()
                            .zip(result_part.iter())
                            .map(|(q, r)| q - r)
                            .collect();
                        Some(grad)
                    } else {
                        None
                    }
                })
                .collect();

            if gradients.is_empty() {
                return;
            }

            let grad_refs: Vec<&[f32]> = gradients.iter().map(|g| g.as_slice()).collect();
            let sample_count = grad_refs.len();

            self.ewc.compute_fisher(&grad_refs, sample_count);
            self.ewc.consolidate(&self.weights);
        }

        /// Return the current EWC penalty for the learned weights.
        ///
        /// Returns `0.0` if [`consolidate`](Self::consolidate) has not been
        /// called yet.
        pub fn ewc_penalty(&self) -> f32 {
            self.ewc.penalty(&self.weights)
        }
    }

    // -----------------------------------------------------------------------
    // EmbeddingQuantizer
    // -----------------------------------------------------------------------

    /// Age-aware embedding quantizer backed by [`TensorCompress`].
    ///
    /// Older embeddings are compressed more aggressively:
    ///
    /// | Age            | Compression          |
    /// |----------------|----------------------|
    /// | < 1 hour       | Full precision       |
    /// | 1 h -- 24 h    | Half precision (FP16)|
    /// | 1 d -- 7 d     | PQ8                  |
    /// | > 7 d          | Binary               |
    pub struct EmbeddingQuantizer {
        compressor: TensorCompress,
    }

    impl Default for EmbeddingQuantizer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl EmbeddingQuantizer {
        /// Create a new quantizer instance.
        pub fn new() -> Self {
            Self {
                compressor: TensorCompress::new(),
            }
        }

        /// Compress an embedding based on its age.
        ///
        /// The age determines the access-frequency proxy passed to the
        /// underlying `TensorCompress`:
        /// - `< 1 h`   -> freq `1.0` (no compression)
        /// - `1-24 h`   -> freq `0.5` (half precision)
        /// - `1-7 d`    -> freq `0.2` (PQ8)
        /// - `> 7 d`    -> freq `0.005` (binary)
        ///
        /// # Arguments
        /// * `embedding`  - The raw embedding vector.
        /// * `age_hours`  - Age of the embedding in hours.
        ///
        /// # Returns
        /// Serialised compressed bytes.  Use [`dequantize`](Self::dequantize)
        /// to recover the original (lossy) vector.
        pub fn quantize_by_age(&self, embedding: &[f32], age_hours: u64) -> Vec<u8> {
            let access_freq = Self::age_to_freq(age_hours);
            match self.compressor.compress(embedding, access_freq) {
                Ok(compressed) => {
                    serde_json::to_vec(&compressed).unwrap_or_else(|_| {
                        // Fallback: store raw f32 bytes.
                        embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
                    })
                }
                Err(_) => {
                    // Fallback: store raw f32 bytes.
                    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
                }
            }
        }

        /// Decompress bytes produced by [`quantize_by_age`](Self::quantize_by_age).
        ///
        /// # Arguments
        /// * `data`         - Compressed byte representation.
        /// * `original_dim` - Expected dimensionality of the output vector.
        ///
        /// # Returns
        /// The decompressed embedding (lossy).  If decompression fails, a
        /// zero-vector of `original_dim` length is returned.
        pub fn dequantize(&self, data: &[u8], original_dim: usize) -> Vec<f32> {
            if let Ok(compressed) =
                serde_json::from_slice::<ruvector_gnn::compress::CompressedTensor>(data)
            {
                if let Ok(decompressed) = self.compressor.decompress(&compressed) {
                    if decompressed.len() == original_dim {
                        return decompressed;
                    }
                }
            }

            // Fallback: try interpreting as raw f32 bytes.
            if data.len() == original_dim * 4 {
                return data
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();
            }

            vec![0.0; original_dim]
        }

        /// Map an age in hours to an access-frequency proxy in [0, 1].
        fn age_to_freq(age_hours: u64) -> f32 {
            match age_hours {
                0 => 1.0,        // Fresh -- full precision
                1..=24 => 0.5,   // Warm  -- half precision
                25..=168 => 0.2, // Cool  -- PQ8
                _ => 0.005,      // Cold  -- binary
            }
        }
    }
}

// ---------------------------------------------------------------------------
// WASM stub implementation
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
mod wasm_stub {
    /// No-op search learner for WASM targets.
    pub struct SearchLearner {
        buffer_len: usize,
    }

    impl SearchLearner {
        pub fn new(_embedding_dim: usize, _replay_capacity: usize) -> Self {
            Self { buffer_len: 0 }
        }

        pub fn record_feedback(
            &mut self,
            _query_embedding: Vec<f32>,
            _result_embedding: Vec<f32>,
            _relevant: bool,
        ) {
            self.buffer_len += 1;
        }

        pub fn replay_buffer_len(&self) -> usize {
            self.buffer_len
        }

        pub fn has_sufficient_data(&self) -> bool {
            self.buffer_len >= 32
        }

        pub fn consolidate(&mut self) {}

        pub fn ewc_penalty(&self) -> f32 {
            0.0
        }
    }

    /// No-op embedding quantizer for WASM targets.
    ///
    /// Returns the original embedding bytes without compression.
    pub struct EmbeddingQuantizer;

    impl EmbeddingQuantizer {
        pub fn new() -> Self {
            Self
        }

        pub fn quantize_by_age(&self, embedding: &[f32], _age_hours: u64) -> Vec<u8> {
            embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
        }

        pub fn dequantize(&self, data: &[u8], original_dim: usize) -> Vec<f32> {
            if data.len() == original_dim * 4 {
                data.chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect()
            } else {
                vec![0.0; original_dim]
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
pub use native::{EmbeddingQuantizer, SearchLearner};

#[cfg(target_arch = "wasm32")]
pub use wasm_stub::{EmbeddingQuantizer, SearchLearner};
