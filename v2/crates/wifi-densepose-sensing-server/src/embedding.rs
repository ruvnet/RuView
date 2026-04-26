//! Contrastive CSI Embedding Model (ADR-024).
//!
//! Implements self-supervised contrastive learning for WiFi CSI feature extraction:
//! - ProjectionHead: 2-layer MLP for contrastive embedding space
//! - CsiAugmenter: domain-specific augmentations for SimCLR-style pretraining
//! - InfoNCE loss: normalized temperature-scaled cross-entropy
//! - FingerprintIndex: brute-force nearest-neighbour (HNSW-compatible interface)
//! - PoseEncoder: lightweight encoder for cross-modal alignment
//! - EmbeddingExtractor: full pipeline (backbone + projection)
//!
//! All arithmetic uses `f32`. No external ML dependencies.

use crate::graph_transformer::{CsiToPoseTransformer, TransformerConfig, Linear};
use crate::sona::{LoraAdapter, EnvironmentDetector, DriftInfo};

// ── SimpleRng (xorshift64) ──────────────────────────────────────────────────

/// Deterministic xorshift64 PRNG to avoid external dependency.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: if seed == 0 { 0xBAAD_CAFE_DEAD_BEEFu64 } else { seed } }
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
    /// Uniform f32 in [0, 1).
    fn next_f32_unit(&mut self) -> f32 {
        (self.next_u64() >> 11) as f32 / (1u64 << 53) as f32
    }
    /// Gaussian approximation via Box-Muller (pair, returns first).
    fn next_gaussian(&mut self) -> f32 {
        let u1 = self.next_f32_unit().max(1e-10);
        let u2 = self.next_f32_unit();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
    }
}

// ── EmbeddingConfig ─────────────────────────────────────────────────────────

/// Configuration for the contrastive embedding model.
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    /// Hidden dimension (must match transformer d_model).
    pub d_model: usize,
    /// Projection/embedding dimension.
    pub d_proj: usize,
    /// InfoNCE temperature.
    pub temperature: f32,
    /// Whether to L2-normalize output embeddings.
    pub normalize: bool,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self { d_model: 64, d_proj: 128, temperature: 0.07, normalize: true }
    }
}

// ── ProjectionHead ──────────────────────────────────────────────────────────

/// 2-layer MLP projection head: d_model -> d_proj -> d_proj with ReLU + L2-norm.
#[derive(Debug, Clone)]
pub struct ProjectionHead {
    pub proj_1: Linear,
    pub proj_2: Linear,
    pub config: EmbeddingConfig,
    /// Optional rank-4 LoRA adapter for proj_1 (environment-specific fine-tuning).
    pub lora_1: Option<LoraAdapter>,
    /// Optional rank-4 LoRA adapter for proj_2 (environment-specific fine-tuning).
    pub lora_2: Option<LoraAdapter>,
}

impl ProjectionHead {
    /// Xavier-initialized projection head.
    pub fn new(config: EmbeddingConfig) -> Self {
        Self {
            proj_1: Linear::with_seed(config.d_model, config.d_proj, 2024),
            proj_2: Linear::with_seed(config.d_proj, config.d_proj, 2025),
            config,
            lora_1: None,
            lora_2: None,
        }
    }

    /// Zero-initialized projection head (for gradient estimation).
    pub fn zeros(config: EmbeddingConfig) -> Self {
        Self {
            proj_1: Linear::zeros(config.d_model, config.d_proj),
            proj_2: Linear::zeros(config.d_proj, config.d_proj),
            config,
            lora_1: None,
            lora_2: None,
        }
    }

    /// Construct a projection head with LoRA adapters enabled at the given rank.
    pub fn with_lora(config: EmbeddingConfig, rank: usize) -> Self {
        let alpha = rank as f32 * 2.0;
        Self {
            proj_1: Linear::with_seed(config.d_model, config.d_proj, 2024),
            proj_2: Linear::with_seed(config.d_proj, config.d_proj, 2025),
            lora_1: Some(LoraAdapter::new(config.d_model, config.d_proj, rank, alpha)),
            lora_2: Some(LoraAdapter::new(config.d_proj, config.d_proj, rank, alpha)),
            config,
        }
    }

    /// Forward pass: ReLU between layers, optional L2-normalize output.
    /// When LoRA adapters are present, their output is added to the base
    /// linear output before the activation.
    pub fn forward(&self, x: &[f32]) -> Vec<f32> {
        let mut h = self.proj_1.forward(x);
        if let Some(ref lora) = self.lora_1 {
            let delta = lora.forward(x);
            for (h_i, &d_i) in h.iter_mut().zip(delta.iter()) {
                *h_i += d_i;
            }
        }
        // ReLU
        for v in h.iter_mut() {
            if *v < 0.0 { *v = 0.0; }
        }
        let mut out = self.proj_2.forward(&h);
        if let Some(ref lora) = self.lora_2 {
            let delta = lora.forward(&h);
            for (o_i, &d_i) in out.iter_mut().zip(delta.iter()) {
                *o_i += d_i;
            }
        }
        if self.config.normalize {
            l2_normalize(&mut out);
        }
        out
    }

    /// Push all weights into a flat vec.
    pub fn flatten_into(&self, out: &mut Vec<f32>) {
        self.proj_1.flatten_into(out);
        self.proj_2.flatten_into(out);
    }

    /// Restore from a flat slice. Returns (Self, number of f32s consumed).
    pub fn unflatten_from(data: &[f32], config: &EmbeddingConfig) -> (Self, usize) {
        let mut offset = 0;
        let (p1, n) = Linear::unflatten_from(&data[offset..], config.d_model, config.d_proj);
        offset += n;
        let (p2, n) = Linear::unflatten_from(&data[offset..], config.d_proj, config.d_proj);
        offset += n;
        (Self { proj_1: p1, proj_2: p2, config: config.clone(), lora_1: None, lora_2: None }, offset)
    }

    /// Total trainable parameters.
    pub fn param_count(&self) -> usize {
        self.proj_1.param_count() + self.proj_2.param_count()
    }

    /// Merge LoRA deltas into the base Linear weights for fast inference.
    /// After merging, the LoRA adapters remain but are effectively accounted for.
    pub fn merge_lora(&mut self) {
        if let Some(ref lora) = self.lora_1 {
            let delta = lora.delta_weights(); // (in_features, out_features)
            let mut w = self.proj_1.weights().to_vec(); // (out_features, in_features)
            for i in 0..delta.len() {
                for j in 0..delta[i].len() {
                    if j < w.len() && i < w[j].len() {
                        w[j][i] += delta[i][j];
                    }
                }
            }
            self.proj_1.set_weights(w);
        }
        if let Some(ref lora) = self.lora_2 {
            let delta = lora.delta_weights();
            let mut w = self.proj_2.weights().to_vec();
            for i in 0..delta.len() {
                for j in 0..delta[i].len() {
                    if j < w.len() && i < w[j].len() {
                        w[j][i] += delta[i][j];
                    }
                }
            }
            self.proj_2.set_weights(w);
        }
    }

    /// Reverse the LoRA merge to restore original base weights for continued training.
    pub fn unmerge_lora(&mut self) {
        if let Some(ref lora) = self.lora_1 {
            let delta = lora.delta_weights();
            let mut w = self.proj_1.weights().to_vec();
            for i in 0..delta.len() {
                for j in 0..delta[i].len() {
                    if j < w.len() && i < w[j].len() {
                        w[j][i] -= delta[i][j];
                    }
                }
            }
            self.proj_1.set_weights(w);
        }
        if let Some(ref lora) = self.lora_2 {
            let delta = lora.delta_weights();
            let mut w = self.proj_2.weights().to_vec();
            for i in 0..delta.len() {
                for j in 0..delta[i].len() {
                    if j < w.len() && i < w[j].len() {
                        w[j][i] -= delta[i][j];
                    }
                }
            }
            self.proj_2.set_weights(w);
        }
    }

    /// Forward using only the LoRA path (base weights frozen), for LoRA-only training.
    /// Returns zero vector if no LoRA adapters are set.
    pub fn freeze_base_train_lora(&self, input: &[f32]) -> Vec<f32> {
        let d_proj = self.config.d_proj;
        // Layer 1: only LoRA contribution + ReLU
        let h = match self.lora_1 {
            Some(ref lora) => {
                let delta = lora.forward(input);
                delta.into_iter().map(|v| if v > 0.0 { v } else { 0.0 }).collect::<Vec<_>>()
            }
            None => vec![0.0f32; d_proj],
        };
        // Layer 2: only LoRA contribution
        let mut out = match self.lora_2 {
            Some(ref lora) => lora.forward(&h),
            None => vec![0.0f32; d_proj],
        };
        if self.config.normalize {
            l2_normalize(&mut out);
        }
        out
    }

    /// Count only the LoRA parameters (not the base weights).
    pub fn lora_param_count(&self) -> usize {
        let c1 = self.lora_1.as_ref().map_or(0, |l| l.n_params());
        let c2 = self.lora_2.as_ref().map_or(0, |l| l.n_params());
        c1 + c2
    }

    /// Flatten only the LoRA weights into a flat vector (A then B for each adapter).
    pub fn flatten_lora(&self) -> Vec<f32> {
        let mut out = Vec::new();
        if let Some(ref lora) = self.lora_1 {
            for row in &lora.a { out.extend_from_slice(row); }
            for row in &lora.b { out.extend_from_slice(row); }
        }
        if let Some(ref lora) = self.lora_2 {
            for row in &lora.a { out.extend_from_slice(row); }
            for row in &lora.b { out.extend_from_slice(row); }
        }
        out
    }

    /// Restore LoRA weights from a flat slice (must match flatten_lora layout).
    pub fn unflatten_lora(&mut self, data: &[f32]) {
        let mut offset = 0;
        if let Some(ref mut lora) = self.lora_1 {
            for row in lora.a.iter_mut() {
                let n = row.len();
                row.copy_from_slice(&data[offset..offset + n]);
                offset += n;
            }
            for row in lora.b.iter_mut() {
                let n = row.len();
                row.copy_from_slice(&data[offset..offset + n]);
                offset += n;
            }
        }
        if let Some(ref mut lora) = self.lora_2 {
            for row in lora.a.iter_mut() {
                let n = row.len();
                row.copy_from_slice(&data[offset..offset + n]);
                offset += n;
            }
            for row in lora.b.iter_mut() {
                let n = row.len();
                row.copy_from_slice(&data[offset..offset + n]);
                offset += n;
            }
        }
    }
}

// ── CsiAugmenter ────────────────────────────────────────────────────────────

/// CSI augmentation strategies for contrastive pretraining.
#[derive(Debug, Clone)]
pub struct CsiAugmenter {
    /// +/- frames to shift (temporal jitter).
    pub temporal_jitter: i32,
    /// Fraction of subcarriers to zero out.
    pub subcarrier_mask_ratio: f32,
    /// Gaussian noise sigma.
    pub noise_std: f32,
    /// Max phase offset in radians.
    pub phase_rotation_max: f32,
    /// Amplitude scale range (min, max).
    pub amplitude_scale_range: (f32, f32),
}

impl CsiAugmenter {
    pub fn new() -> Self {
        Self {
            temporal_jitter: 2,
            subcarrier_mask_ratio: 0.15,
            noise_std: 0.05,
            phase_rotation_max: std::f32::consts::FRAC_PI_4,
            amplitude_scale_range: (0.8, 1.2),
        }
    }

    /// Apply random augmentations to a CSI window, returning two different views.
    /// Each view receives a different random subset of augmentations.
    pub fn augment_pair(
        &self,
        csi_window: &[Vec<f32>],
        rng_seed: u64,
    ) -> (Vec<Vec<f32>>, Vec<Vec<f32>>) {
        let mut rng_a = SimpleRng::new(rng_seed);
        let mut rng_b = SimpleRng::new(rng_seed.wrapping_add(0x1234_5678_9ABC_DEF0));

        // View A: temporal jitter + noise + subcarrier mask
        let mut view_a = self.apply_temporal_jitter(csi_window, &mut rng_a);
        self.apply_gaussian_noise(&mut view_a, &mut rng_a);
        self.apply_subcarrier_mask(&mut view_a, &mut rng_a);

        // View B: amplitude scaling + phase rotation + different noise
        let mut view_b = self.apply_temporal_jitter(csi_window, &mut rng_b);
        self.apply_amplitude_scaling(&mut view_b, &mut rng_b);
        self.apply_phase_rotation(&mut view_b, &mut rng_b);
        self.apply_gaussian_noise(&mut view_b, &mut rng_b);

        (view_a, view_b)
    }

    fn apply_temporal_jitter(
        &self,
        window: &[Vec<f32>],
        rng: &mut SimpleRng,
    ) -> Vec<Vec<f32>> {
        if window.is_empty() || self.temporal_jitter == 0 {
            return window.to_vec();
        }
        let range = 2 * self.temporal_jitter + 1;
        let shift = (rng.next_u64() % range as u64) as i32 - self.temporal_jitter;
        let n = window.len() as i32;
        (0..window.len())
            .map(|i| {
                let src = (i as i32 + shift).clamp(0, n - 1) as usize;
                window[src].clone()
            })
            .collect()
    }

    fn apply_subcarrier_mask(&self, window: &mut [Vec<f32>], rng: &mut SimpleRng) {
        for frame in window.iter_mut() {
            for v in frame.iter_mut() {
                if rng.next_f32_unit() < self.subcarrier_mask_ratio {
                    *v = 0.0;
                }
            }
        }
    }

    fn apply_gaussian_noise(&self, window: &mut [Vec<f32>], rng: &mut SimpleRng) {
        for frame in window.iter_mut() {
            for v in frame.iter_mut() {
                *v += rng.next_gaussian() * self.noise_std;
            }
        }
    }

    fn apply_phase_rotation(&self, window: &mut [Vec<f32>], rng: &mut SimpleRng) {
        let offset = (rng.next_f32_unit() * 2.0 - 1.0) * self.phase_rotation_max;
        for frame in window.iter_mut() {
            for v in frame.iter_mut() {
                // Approximate phase rotation on amplitude: multiply by cos(offset)
                *v *= offset.cos();
            }
        }
    }

    fn apply_amplitude_scaling(&self, window: &mut [Vec<f32>], rng: &mut SimpleRng) {
        let (lo, hi) = self.amplitude_scale_range;
        let scale = lo + rng.next_f32_unit() * (hi - lo);
        for frame in window.iter_mut() {
            for v in frame.iter_mut() {
                *v *= scale;
            }
        }
    }
}

impl Default for CsiAugmenter {
    fn default() -> Self { Self::new() }
}

// ── Vector math utilities ───────────────────────────────────────────────────

/// L2-normalize a vector in-place.
fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-10 {
        let inv = 1.0 / norm;
        for x in v.iter_mut() {
            *x *= inv;
        }
    }
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let dot: f32 = (0..n).map(|i| a[i] * b[i]).sum();
    let na = (0..n).map(|i| a[i] * a[i]).sum::<f32>().sqrt();
    let nb = (0..n).map(|i| b[i] * b[i]).sum::<f32>().sqrt();
    if na > 1e-10 && nb > 1e-10 { dot / (na * nb) } else { 0.0 }
}

// ── InfoNCE loss ────────────────────────────────────────────────────────────

/// InfoNCE contrastive loss (NT-Xent / SimCLR objective).
///
/// For batch of N pairs (a_i, b_i):
///   loss = -1/N sum_i log( exp(sim(a_i, b_i)/t) / sum_j exp(sim(a_i, b_j)/t) )
pub fn info_nce_loss(
    embeddings_a: &[Vec<f32>],
    embeddings_b: &[Vec<f32>],
    temperature: f32,
) -> f32 {
    let n = embeddings_a.len().min(embeddings_b.len());
    if n == 0 {
        return 0.0;
    }
    let t = temperature.max(1e-6);
    let mut total_loss = 0.0f32;

    for i in 0..n {
        // Compute similarity of anchor a_i with all b_j
        let mut logits = Vec::with_capacity(n);
        for j in 0..n {
            logits.push(cosine_similarity(&embeddings_a[i], &embeddings_b[j]) / t);
        }
        // Numerically stable log-softmax
        let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let log_sum_exp = logits.iter()
            .map(|&l| (l - max_logit).exp())
            .sum::<f32>()
            .ln() + max_logit;
        total_loss += -logits[i] + log_sum_exp;
    }

    total_loss / n as f32
}

// ── FingerprintIndex ────────────────────────────────────────────────────────

/// Fingerprint index type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IndexType {
    EnvironmentFingerprint,
    ActivityPattern,
    TemporalBaseline,
    PersonTrack,
}

/// A single index entry.
pub struct IndexEntry {
    pub embedding: Vec<f32>,
    pub metadata: String,
    pub timestamp_ms: u64,
    pub index_type: IndexType,
    /// Whether this entry was inserted during a detected environment drift.
    pub anomalous: bool,
}

/// Search result from the fingerprint index.
pub struct SearchResult {
    /// Index into the entries vec.
    pub entry: usize,
    /// Cosine distance (1 - similarity).
    pub distance: f32,
    /// Metadata string from the matching entry.
    pub metadata: String,
}

/// Brute-force fingerprint index with HNSW-compatible interface.
///
/// Stores embeddings and supports nearest-neighbour search via cosine distance.
/// Can be replaced with a proper HNSW implementation for production scale.
pub struct FingerprintIndex {
    entries: Vec<IndexEntry>,
    index_type: IndexType,
}

impl FingerprintIndex {
    pub fn new(index_type: IndexType) -> Self {
        Self { entries: Vec::new(), index_type }
    }

    /// Insert an embedding with metadata and timestamp.
    pub fn insert(&mut self, embedding: Vec<f32>, metadata: String, timestamp_ms: u64) {
        self.entries.push(IndexEntry {
            embedding,
            metadata,
            timestamp_ms,
            index_type: self.index_type,
            anomalous: false,
        });
    }

    /// Insert an embedding with drift-awareness: marks the entry as anomalous
    /// if the provided drift flag is true.
    pub fn insert_with_drift(
        &mut self,
        embedding: Vec<f32>,
        metadata: String,
        timestamp_ms: u64,
        drift_detected: bool,
    ) {
        self.entries.push(IndexEntry {
            embedding,
            metadata,
            timestamp_ms,
            index_type: self.index_type,
            anomalous: drift_detected,
        });
    }

    /// Count the number of entries marked as anomalous.
    pub fn anomalous_count(&self) -> usize {
        self.entries.iter().filter(|e| e.anomalous).count()
    }

    /// Search for the top-k nearest embeddings by cosine distance.
    pub fn search(&self, query: &[f32], top_k: usize) -> Vec<SearchResult> {
        let mut results: Vec<(usize, f32)> = self.entries.iter().enumerate()
            .map(|(i, e)| (i, 1.0 - cosine_similarity(query, &e.embedding)))
            .collect();
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results.into_iter().map(|(i, d)| SearchResult {
            entry: i,
            distance: d,
            metadata: self.entries[i].metadata.clone(),
        }).collect()
    }

    /// Number of entries in the index.
    pub fn len(&self) -> usize { self.entries.len() }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    /// Detect anomaly: returns true if query is farther than threshold from all entries.
    pub fn is_anomaly(&self, query: &[f32], threshold: f32) -> bool {
        if self.entries.is_empty() {
            return true;
        }
        self.entries.iter()
            .all(|e| (1.0 - cosine_similarity(query, &e.embedding)) > threshold)
    }
}

// ── PoseEncoder (cross-modal alignment) ─────────────────────────────────────

/// Lightweight pose encoder for cross-modal alignment.
/// Maps 51-dim pose vector (17 keypoints * 3 coords) to d_proj embedding.
#[derive(Debug, Clone)]
pub struct PoseEncoder {
    pub layer_1: Linear,
    pub layer_2: Linear,
    d_proj: usize,
}

impl PoseEncoder {
    /// Create a new pose encoder mapping 51-dim input to d_proj-dim embedding.
    pub fn new(d_proj: usize) -> Self {
        Self {
            layer_1: Linear::with_seed(51, d_proj, 3001),
            layer_2: Linear::with_seed(d_proj, d_proj, 3002),
            d_proj,
        }
    }

    /// Forward pass: ReLU + L2-normalize.
    pub fn forward(&self, pose_flat: &[f32]) -> Vec<f32> {
        let h: Vec<f32> = self.layer_1.forward(pose_flat).into_iter()
            .map(|v| if v > 0.0 { v } else { 0.0 })
            .collect();
        let mut out = self.layer_2.forward(&h);
        l2_normalize(&mut out);
        out
    }

    /// Push all weights into a flat vec.
    pub fn flatten_into(&self, out: &mut Vec<f32>) {
        self.layer_1.flatten_into(out);
        self.layer_2.flatten_into(out);
    }

    /// Restore from a flat slice. Returns (Self, number of f32s consumed).
    pub fn unflatten_from(data: &[f32], d_proj: usize) -> (Self, usize) {
        let mut offset = 0;
        let (l1, n) = Linear::unflatten_from(&data[offset..], 51, d_proj);
        offset += n;
        let (l2, n) = Linear::unflatten_from(&data[offset..], d_proj, d_proj);
        offset += n;
        (Self { layer_1: l1, layer_2: l2, d_proj }, offset)
    }

    /// Total trainable parameters.
    pub fn param_count(&self) -> usize {
        self.layer_1.param_count() + self.layer_2.param_count()
    }
}

/// Cross-modal contrastive loss: aligns CSI embeddings with pose embeddings.
/// Same as info_nce_loss but between two different modalities.
pub fn cross_modal_loss(
    csi_embeddings: &[Vec<f32>],
    pose_embeddings: &[Vec<f32>],
    temperature: f32,
) -> f32 {
    info_nce_loss(csi_embeddings, pose_embeddings, temperature)
}

// ── EmbeddingExtractor ──────────────────────────────────────────────────────

/// Full embedding extractor: CsiToPoseTransformer backbone + ProjectionHead.
pub struct EmbeddingExtractor {
    pub transformer: CsiToPoseTransformer,
    pub projection: ProjectionHead,
    pub config: EmbeddingConfig,
    /// Optional drift detector for environment change detection.
    pub drift_detector: Option<EnvironmentDetector>,
}

impl EmbeddingExtractor {
    /// Create a new embedding extractor with given configs.
    pub fn new(t_config: TransformerConfig, e_config: EmbeddingConfig) -> Self {
        Self {
            transformer: CsiToPoseTransformer::new(t_config),
            projection: ProjectionHead::new(e_config.clone()),
            config: e_config,
            drift_detector: None,
        }
    }

    /// Create an embedding extractor with environment drift detection enabled.
    pub fn with_drift_detection(
        t_config: TransformerConfig,
        e_config: EmbeddingConfig,
        window_size: usize,
    ) -> Self {
        Self {
            transformer: CsiToPoseTransformer::new(t_config),
            projection: ProjectionHead::new(e_config.clone()),
            config: e_config,
            drift_detector: Some(EnvironmentDetector::new(window_size)),
        }
    }

    /// Extract embedding from CSI features.
    /// Mean-pools the 17 body_part_features from the transformer backbone,
    /// then projects through the ProjectionHead.
    /// When a drift detector is present, updates it with CSI statistics.
    pub fn extract(&mut self, csi_features: &[Vec<f32>]) -> Vec<f32> {
        // Feed drift detector with CSI statistics if present
        if let Some(ref mut detector) = self.drift_detector {
            let (mean, var) = csi_feature_stats(csi_features);
            detector.update(mean, var);
        }
        let body_feats = self.transformer.embed(csi_features);
        let d = self.config.d_model;
        // Mean-pool across 17 keypoints
        let mut pooled = vec![0.0f32; d];
        for feat in &body_feats {
            for (p, &f) in pooled.iter_mut().zip(feat.iter()) {
                *p += f;
            }
        }
        let n = body_feats.len() as f32;
        if n > 0.0 {
            for p in pooled.iter_mut() {
                *p /= n;
            }
        }
        self.projection.forward(&pooled)
    }

    /// Batch extract embeddings.
    pub fn extract_batch(&mut self, batch: &[Vec<Vec<f32>>]) -> Vec<Vec<f32>> {
        let mut results = Vec::with_capacity(batch.len());
        for csi in batch {
            results.push(self.extract(csi));
        }
        results
    }

    /// Whether an environment drift has been detected.
    pub fn drift_detected(&self) -> bool {
        self.drift_detector.as_ref().map_or(false, |d| d.drift_detected())
    }

    /// Get drift information if a detector is present.
    pub fn drift_info(&self) -> Option<DriftInfo> {
        self.drift_detector.as_ref().map(|d| d.drift_info())
    }

    /// Total parameter count (transformer + projection).
    pub fn param_count(&self) -> usize {
        self.transformer.param_count() + self.projection.param_count()
    }

    /// Flatten all weights (transformer + projection).
    pub fn flatten_weights(&self) -> Vec<f32> {
        let mut out = self.transformer.flatten_weights();
        self.projection.flatten_into(&mut out);
        out
    }

    /// Unflatten all weights from a flat slice.
    pub fn unflatten_weights(&mut self, params: &[f32]) -> Result<(), String> {
        let t_count = self.transformer.param_count();
        let p_count = self.projection.param_count();
        let expected = t_count + p_count;
        if params.len() != expected {
            return Err(format!(
                "expected {} params ({}+{}), got {}",
                expected, t_count, p_count, params.len()
            ));
        }
        self.transformer.unflatten_weights(&params[..t_count])?;
        let (proj, consumed) = ProjectionHead::unflatten_from(&params[t_count..], &self.config);
        if consumed != p_count {
            return Err(format!(
                "projection consumed {consumed} params, expected {p_count}"
            ));
        }
        self.projection = proj;
        Ok(())
    }
}

// ── CSI feature statistics ─────────────────────────────────────────────────

/// Compute mean and variance of all values in a CSI feature matrix.
fn csi_feature_stats(features: &[Vec<f32>]) -> (f32, f32) {
    let mut sum = 0.0f32;
    let mut sum_sq = 0.0f32;
    let mut count = 0usize;
    for row in features {
        for &v in row {
            sum += v;
            sum_sq += v * v;
            count += 1;
        }
    }
    if count == 0 {
        return (0.0, 0.0);
    }
    let mean = sum / count as f32;
    let var = sum_sq / count as f32 - mean * mean;
    (mean, var.max(0.0))
}

// ── Hard-Negative Mining ──────────────────────────────────────────────────

/// Selects the hardest negative pairs from a similarity matrix to improve
/// contrastive training efficiency. During warmup epochs, all negatives
/// are used to ensure stable early training.
pub struct HardNegativeMiner {
    /// Ratio of hardest negatives to select (0.5 = top 50%).
    pub ratio: f32,
    /// Number of epochs to use all negatives before mining.
    pub warmup_epochs: usize,
}

impl HardNegativeMiner {
    pub fn new(ratio: f32, warmup_epochs: usize) -> Self {
        Self {
            ratio: ratio.clamp(0.01, 1.0),
            warmup_epochs,
        }
    }

    /// From a cosine similarity matrix (N x N), select the hardest negative pairs.
    /// Returns indices of selected negative pairs (i, j) where i != j.
    /// During warmup, returns all negative pairs.
    pub fn mine(&self, sim_matrix: &[Vec<f32>], epoch: usize) -> Vec<(usize, usize)> {
        let n = sim_matrix.len();
        if n <= 1 {
            return Vec::new();
        }

        // Collect all negative pairs with their similarity
        let mut neg_pairs: Vec<(usize, usize, f32)> = Vec::new();
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    let sim = if j < sim_matrix[i].len() { sim_matrix[i][j] } else { 0.0 };
                    neg_pairs.push((i, j, sim));
                }
            }
        }

        if epoch < self.warmup_epochs {
            // During warmup, return all negative pairs
            return neg_pairs.into_iter().map(|(i, j, _)| (i, j)).collect();
        }

        // Sort by similarity descending (hardest negatives have highest similarity)
        neg_pairs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        // Take the top ratio fraction
        let k = ((neg_pairs.len() as f32 * self.ratio).ceil() as usize).max(1);
        neg_pairs.truncate(k);
        neg_pairs.into_iter().map(|(i, j, _)| (i, j)).collect()
    }
}

/// InfoNCE loss with optional hard-negative mining support.
/// When a miner is provided and past warmup, only the hardest negatives
/// contribute to the denominator.
pub fn info_nce_loss_mined(
    embeddings_a: &[Vec<f32>],
    embeddings_b: &[Vec<f32>],
    temperature: f32,
    miner: Option<&HardNegativeMiner>,
    epoch: usize,
) -> f32 {
    let n = embeddings_a.len().min(embeddings_b.len());
    if n == 0 {
        return 0.0;
    }
    let t = temperature.max(1e-6);

    // If no miner or in warmup, delegate to standard InfoNCE
    let use_mining = match miner {
        Some(m) => epoch >= m.warmup_epochs,
        None => false,
    };

    if !use_mining {
        return info_nce_loss(embeddings_a, embeddings_b, temperature);
    }

    let miner = match miner {
        Some(m) => m,
        None => return info_nce_loss(embeddings_a, embeddings_b, temperature),
    };

    // Build similarity matrix for mining
    let mut sim_matrix = vec![vec![0.0f32; n]; n];
    for i in 0..n {
        for j in 0..n {
            sim_matrix[i][j] = cosine_similarity(&embeddings_a[i], &embeddings_b[j]);
        }
    }

    let mined_pairs = miner.mine(&sim_matrix, epoch);

    // Build per-anchor set of active negative indices
    let mut neg_indices: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(i, j) in &mined_pairs {
        if i < n && j < n {
            neg_indices[i].push(j);
        }
    }

    let mut total_loss = 0.0f32;
    for i in 0..n {
        let pos_sim = sim_matrix[i][i] / t;

        // Build logits: positive + selected hard negatives
        let mut logits = vec![pos_sim];
        for &j in &neg_indices[i] {
            if j != i {
                logits.push(sim_matrix[i][j] / t);
            }
        }

        // Log-softmax for the positive (index 0)
        let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let log_sum_exp = logits.iter()
            .map(|&l| (l - max_logit).exp())
            .sum::<f32>()
            .ln() + max_logit;
        total_loss += -pos_sim + log_sum_exp;
    }

    total_loss / n as f32
}

// ── Quantized embedding validation ─────────────────────────────────────────

use crate::sparse_inference::Quantizer;

/// Validate that INT8 quantization preserves embedding ranking.
/// Returns Spearman rank correlation between FP32 and INT8 distance rankings.
pub fn validate_quantized_embeddings(
    embeddings_fp32: &[Vec<f32>],
    query_fp32: &[f32],
    _quantizer: &Quantizer,
) -> f32 {
    if embeddings_fp32.is_empty() {
        return 1.0;
    }
    let n = embeddings_fp32.len();

    // 1. FP32 cosine distances
    let fp32_distances: Vec<f32> = embeddings_fp32.iter()
        .map(|e| 1.0 - cosine_similarity(query_fp32, e))
        .collect();

    // 2. Quantize each embedding and query, compute approximate distances
    let query_quant = Quantizer::quantize_symmetric(query_fp32);
    let query_deq = Quantizer::dequantize(&query_quant);
    let int8_distances: Vec<f32> = embeddings_fp32.iter()
        .map(|e| {
            let eq = Quantizer::quantize_symmetric(e);
            let ed = Quantizer::dequantize(&eq);
            1.0 - cosine_similarity(&query_deq, &ed)
        })
        .collect();

    // 3. Compute rank arrays
    let fp32_ranks = rank_array(&fp32_distances);
    let int8_ranks = rank_array(&int8_distances);

    // 4. Spearman rank correlation: 1 - 6*sum(d^2) / (n*(n^2-1))
    let d_sq_sum: f32 = fp32_ranks.iter().zip(int8_ranks.iter())
        .map(|(&a, &b)| (a - b) * (a - b))
        .sum();
    let n_f = n as f32;
    if n <= 1 {
        return 1.0;
    }
    1.0 - (6.0 * d_sq_sum) / (n_f * (n_f * n_f - 1.0))
}

/// Compute ranks for an array of values (1-based, average ties).
fn rank_array(values: &[f32]) -> Vec<f32> {
    let n = values.len();
    let mut indexed: Vec<(usize, f32)> = values.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut ranks = vec![0.0f32; n];
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j < n && (indexed[j].1 - indexed[i].1).abs() < 1e-10 {
            j += 1;
        }
        let avg_rank = (i + j + 1) as f32 / 2.0; // 1-based average
        for k in i..j {
            ranks[indexed[k].0] = avg_rank;
        }
        i = j;
    }
    ranks
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn small_config() -> TransformerConfig {
        TransformerConfig {
            n_subcarriers: 16,
            n_keypoints: 17,
            d_model: 8,
            n_heads: 2,
            n_gnn_layers: 1,
        }
    }

    fn small_embed_config() -> EmbeddingConfig {
        EmbeddingConfig {
            d_model: 8,
            d_proj: 128,
            temperature: 0.07,
            normalize: true,
        }
    }

    fn make_csi(n_pairs: usize, n_sub: usize, seed: u64) -> Vec<Vec<f32>> {
        let mut rng = SimpleRng::new(seed);
        (0..n_pairs)
            .map(|_| (0..n_sub).map(|_| rng.next_f32_unit()).collect())
            .collect()
    }

    // ── ProjectionHead tests ────────────────────────────────────────────

    #[test]
    fn test_projection_head_output_shape() {
        let config = small_embed_config();
        let proj = ProjectionHead::new(config);
        let input = vec![0.5f32; 8];
        let output = proj.forward(&input);
        assert_eq!(output.len(), 128);
    }

    #[test]
    fn test_projection_head_l2_normalized() {
        let config = small_embed_config();
        let proj = ProjectionHead::new(config);
        let input = vec![1.0f32; 8];
        let output = proj.forward(&input);
        let norm: f32 = output.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-4,
            "expected unit norm, got {norm}"
        );
    }

    #[test]
    fn test_projection_head_weight_roundtrip() {
        let config = small_embed_config();
        let proj = ProjectionHead::new(config.clone());
        let mut flat = Vec::new();
        proj.flatten_into(&mut flat);
        assert_eq!(flat.len(), proj.param_count());

        let (restored, consumed) = ProjectionHead::unflatten_from(&flat, &config);
        assert_eq!(consumed, flat.len());

        let input = vec![0.3f32; 8];
        let out_orig = proj.forward(&input);
        let out_rest = restored.forward(&input);
        for (a, b) in out_orig.iter().zip(out_rest.iter()) {
            assert!((a - b).abs() < 1e-6, "mismatch: {a} vs {b}");
        }
    }

    // ── InfoNCE loss tests ──────────────────────────────────────────────

    #[test]
    fn test_info_nce_loss_positive_pairs() {
        // Identical embeddings should give low loss (close to log(1) = 0)
        let emb = vec![vec![1.0, 0.0, 0.0]; 4];
        let loss = info_nce_loss(&emb, &emb, 0.07);
        // When all embeddings are identical, all similarities are 1.0,
        // so loss = log(N) per sample
        let expected = (4.0f32).ln();
        assert!(
            (loss - expected).abs() < 0.1,
            "identical embeddings: expected ~{expected}, got {loss}"
        );
    }

    #[test]
    fn test_info_nce_loss_random_pairs() {
        // Random embeddings should give higher loss than well-aligned ones
        let aligned_a = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
        ];
        let aligned_b = vec![
            vec![0.9, 0.1, 0.0, 0.0],
            vec![0.1, 0.9, 0.0, 0.0],
        ];
        let random_b = vec![
            vec![0.0, 0.0, 1.0, 0.0],
            vec![0.0, 0.0, 0.0, 1.0],
        ];
        let loss_aligned = info_nce_loss(&aligned_a, &aligned_b, 0.5);
        let loss_random = info_nce_loss(&aligned_a, &random_b, 0.5);
        assert!(
            loss_random > loss_aligned,
            "random should have higher loss: {loss_random} vs {loss_aligned}"
        );
    }

    // ── CsiAugmenter tests ──────────────────────────────────────────────

    #[test]
    fn test_augmenter_produces_different_views() {
        let aug = CsiAugmenter::new();
        let csi = vec![vec![1.0f32; 16]; 5];
        let (view_a, view_b) = aug.augment_pair(&csi, 42);
        // Views should differ (different augmentation pipelines)
        let mut any_diff = false;
        for (a, b) in view_a.iter().zip(view_b.iter()) {
            for (&va, &vb) in a.iter().zip(b.iter()) {
                if (va - vb).abs() > 1e-6 {
                    any_diff = true;
                    break;
                }
            }
            if any_diff { break; }
        }
        assert!(any_diff, "augmented views should differ");
    }

    #[test]
    fn test_augmenter_preserves_shape() {
        let aug = CsiAugmenter::new();
        let csi = vec![vec![0.5f32; 20]; 8];
        let (view_a, view_b) = aug.augment_pair(&csi, 99);
        assert_eq!(view_a.len(), 8);
        assert_eq!(view_b.len(), 8);
        for frame in &view_a {
            assert_eq!(frame.len(), 20);
        }
        for frame in &view_b {
            assert_eq!(frame.len(), 20);
        }
    }

    // ── EmbeddingExtractor tests ────────────────────────────────────────

    #[test]
    fn test_embedding_extractor_output_shape() {
        let mut ext = EmbeddingExtractor::new(small_config(), small_embed_config());
        let csi = make_csi(4, 16, 42);
        let emb = ext.extract(&csi);
        assert_eq!(emb.len(), 128);
    }

    #[test]
    fn test_embedding_extractor_weight_roundtrip() {
        let mut ext = EmbeddingExtractor::new(small_config(), small_embed_config());
        let weights = ext.flatten_weights();
        assert_eq!(weights.len(), ext.param_count());

        let mut ext2 = EmbeddingExtractor::new(small_config(), small_embed_config());
        ext2.unflatten_weights(&weights).expect("unflatten should succeed");

        let csi = make_csi(4, 16, 42);
        let emb1 = ext.extract(&csi);
        let emb2 = ext2.extract(&csi);
        for (a, b) in emb1.iter().zip(emb2.iter()) {
            assert!((a - b).abs() < 1e-5, "mismatch: {a} vs {b}");
        }
    }

    // ── FingerprintIndex tests ──────────────────────────────────────────

    #[test]
    fn test_fingerprint_index_insert_search() {
        let mut idx = FingerprintIndex::new(IndexType::EnvironmentFingerprint);
        // Insert 10 unit vectors along different axes
        for i in 0..10 {
            let mut emb = vec![0.0f32; 10];
            emb[i] = 1.0;
            idx.insert(emb, format!("entry_{i}"), i as u64 * 100);
        }
        assert_eq!(idx.len(), 10);

        // Search for vector close to axis 3
        let mut query = vec![0.0f32; 10];
        query[3] = 1.0;
        let results = idx.search(&query, 3);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].entry, 3, "nearest should be entry_3");
        assert!(results[0].distance < 0.01, "distance should be ~0");
    }

    #[test]
    fn test_fingerprint_index_anomaly_detection() {
        let mut idx = FingerprintIndex::new(IndexType::ActivityPattern);
        // Insert clustered embeddings
        for i in 0..5 {
            let emb = vec![1.0 + i as f32 * 0.01; 8];
            idx.insert(emb, format!("normal_{i}"), 0);
        }

        // Normal query (similar to cluster)
        let normal = vec![1.0f32; 8];
        assert!(!idx.is_anomaly(&normal, 0.1), "normal should not be anomaly");

        // Anomalous query (very different)
        let anomaly = vec![-1.0f32; 8];
        assert!(idx.is_anomaly(&anomaly, 0.5), "distant should be anomaly");
    }

    #[test]
    fn test_fingerprint_index_types() {
        let types = [
            IndexType::EnvironmentFingerprint,
            IndexType::ActivityPattern,
            IndexType::TemporalBaseline,
            IndexType::PersonTrack,
        ];
        for &it in &types {
            let mut idx = FingerprintIndex::new(it);
            idx.insert(vec![1.0, 2.0, 3.0], "test".into(), 0);
            assert_eq!(idx.len(), 1);
            let results = idx.search(&[1.0, 2.0, 3.0], 1);
            assert_eq!(results.len(), 1);
            assert!(results[0].distance < 0.01);
        }
    }

    // ── PoseEncoder tests ───────────────────────────────────────────────

    #[test]
    fn test_pose_encoder_output_shape() {
        let enc = PoseEncoder::new(128);
        let pose_flat = vec![0.5f32; 51]; // 17 * 3
        let out = enc.forward(&pose_flat);
        assert_eq!(out.len(), 128);
    }

    #[test]
    fn test_pose_encoder_l2_normalized() {
        let enc = PoseEncoder::new(128);
        let pose_flat = vec![1.0f32; 51];
        let out = enc.forward(&pose_flat);
        let norm: f32 = out.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-4,
            "expected unit norm, got {norm}"
        );
    }

    #[test]
    fn test_cross_modal_loss_aligned_pairs() {
        // Create CSI and pose embeddings that are aligned
        let csi_emb = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, 0.0],
        ];
        let pose_emb_aligned = vec![
            vec![0.95, 0.05, 0.0, 0.0],
            vec![0.05, 0.95, 0.0, 0.0],
            vec![0.0, 0.05, 0.95, 0.0],
        ];
        let pose_emb_shuffled = vec![
            vec![0.0, 0.05, 0.95, 0.0],
            vec![0.95, 0.05, 0.0, 0.0],
            vec![0.05, 0.95, 0.0, 0.0],
        ];
        let loss_aligned = cross_modal_loss(&csi_emb, &pose_emb_aligned, 0.5);
        let loss_shuffled = cross_modal_loss(&csi_emb, &pose_emb_shuffled, 0.5);
        assert!(
            loss_aligned < loss_shuffled,
            "aligned should have lower loss: {loss_aligned} vs {loss_shuffled}"
        );
    }

    // ── Quantized embedding validation ──────────────────────────────────

    #[test]
    fn test_quantized_embedding_rank_correlation() {
        let mut rng = SimpleRng::new(12345);
        let embeddings: Vec<Vec<f32>> = (0..20)
            .map(|_| (0..32).map(|_| rng.next_gaussian()).collect())
            .collect();
        let query: Vec<f32> = (0..32).map(|_| rng.next_gaussian()).collect();

        let corr = validate_quantized_embeddings(&embeddings, &query, &Quantizer);
        assert!(
            corr > 0.90,
            "rank correlation should be > 0.90, got {corr}"
        );
    }

    // ── Transformer embed() test ────────────────────────────────────────

    #[test]
    fn test_transformer_embed_shape() {
        let t = CsiToPoseTransformer::new(small_config());
        let csi = make_csi(4, 16, 42);
        let body_feats = t.embed(&csi);
        assert_eq!(body_feats.len(), 17);
        for f in &body_feats {
            assert_eq!(f.len(), 8); // d_model = 8
        }
    }

    // ── Phase 7: LoRA on ProjectionHead tests ─────────────────────────

    #[test]
    fn test_projection_head_with_lora_changes_output() {
        let config = EmbeddingConfig {
            d_model: 64, d_proj: 128, temperature: 0.07, normalize: true,
        };
        let base = ProjectionHead::new(config.clone());
        let mut lora = ProjectionHead::with_lora(config, 4);
        // Set some non-zero LoRA weights so output differs
        if let Some(ref mut l) = lora.lora_1 {
            for i in 0..l.in_features.min(l.a.len()) {
                for r in 0..l.rank.min(l.a[i].len()) {
                    l.a[i][r] = (i as f32 * 0.01 + r as f32 * 0.02).sin();
                }
            }
            for r in 0..l.rank.min(l.b.len()) {
                for j in 0..l.out_features.min(l.b[r].len()) {
                    l.b[r][j] = (r as f32 * 0.03 + j as f32 * 0.01).cos() * 0.1;
                }
            }
        }
        let input = vec![0.5f32; 64];
        let out_base = base.forward(&input);
        let out_lora = lora.forward(&input);
        let mut any_diff = false;
        for (a, b) in out_base.iter().zip(out_lora.iter()) {
            if (a - b).abs() > 1e-6 { any_diff = true; break; }
        }
        assert!(any_diff, "LoRA should change the output");
    }

    #[test]
    fn test_projection_head_merge_unmerge_roundtrip() {
        let config = EmbeddingConfig {
            d_model: 64, d_proj: 128, temperature: 0.07, normalize: false,
        };
        let mut proj = ProjectionHead::with_lora(config, 4);
        // Set non-zero LoRA weights
        if let Some(ref mut l) = proj.lora_1 {
            l.a[0][0] = 1.0; l.b[0][0] = 0.5;
        }
        if let Some(ref mut l) = proj.lora_2 {
            l.a[0][0] = 0.3; l.b[0][0] = 0.2;
        }
        let input = vec![0.3f32; 64];
        let out_before = proj.forward(&input);

        // Merge, then unmerge -- output should match original (with LoRA still in forward)
        proj.merge_lora();
        proj.unmerge_lora();
        let out_after = proj.forward(&input);

        for (a, b) in out_before.iter().zip(out_after.iter()) {
            assert!(
                (a - b).abs() < 1e-4,
                "merge/unmerge roundtrip failed: {a} vs {b}"
            );
        }
    }

    #[test]
    fn test_projection_head_lora_param_count() {
        let config = EmbeddingConfig {
            d_model: 64, d_proj: 128, temperature: 0.07, normalize: true,
        };
        let proj = ProjectionHead::with_lora(config, 4);
        // lora_1: rank=4, in=64, out=128 => 4*(64+128) = 768
        // lora_2: rank=4, in=128, out=128 => 4*(128+128) = 1024
        // Total = 768 + 1024 = 1792
        assert_eq!(proj.lora_param_count(), 1792);
    }

    #[test]
    fn test_projection_head_flatten_unflatten_lora() {
        let config = EmbeddingConfig {
            d_model: 64, d_proj: 128, temperature: 0.07, normalize: true,
        };
        let mut proj = ProjectionHead::with_lora(config.clone(), 4);
        // Set recognizable LoRA weights
        if let Some(ref mut l) = proj.lora_1 {
            l.a[0][0] = 1.5; l.a[1][1] = -0.3;
            l.b[0][0] = 2.0; l.b[1][5] = -1.0;
        }
        if let Some(ref mut l) = proj.lora_2 {
            l.a[3][2] = 0.7;
            l.b[2][10] = 0.42;
        }
        let flat = proj.flatten_lora();
        assert_eq!(flat.len(), 1792);

        // Restore into a fresh LoRA-enabled projection head
        let mut proj2 = ProjectionHead::with_lora(config, 4);
        proj2.unflatten_lora(&flat);

        // Verify round-trip by re-flattening
        let flat2 = proj2.flatten_lora();
        for (a, b) in flat.iter().zip(flat2.iter()) {
            assert!((a - b).abs() < 1e-6, "flatten/unflatten mismatch: {a} vs {b}");
        }
    }

    // ── Phase 7: Hard-Negative Mining tests ───────────────────────────

    #[test]
    fn test_hard_negative_miner_warmup() {
        let miner = HardNegativeMiner::new(0.5, 5);
        let sim = vec![
            vec![1.0, 0.8, 0.2],
            vec![0.8, 1.0, 0.3],
            vec![0.2, 0.3, 1.0],
        ];
        // During warmup (epoch 0 < 5), all negative pairs should be returned
        let pairs = miner.mine(&sim, 0);
        // 3 anchors * 2 negatives each = 6 negative pairs
        assert_eq!(pairs.len(), 6, "warmup should return all negative pairs");
    }

    #[test]
    fn test_hard_negative_miner_selects_hardest() {
        let miner = HardNegativeMiner::new(0.5, 0); // no warmup, 50% ratio
        let sim = vec![
            vec![1.0, 0.9, 0.1, 0.05],
            vec![0.9, 1.0, 0.8, 0.2],
            vec![0.1, 0.8, 1.0, 0.3],
            vec![0.05, 0.2, 0.3, 1.0],
        ];
        let pairs = miner.mine(&sim, 10);
        // 4*3 = 12 total negative pairs, 50% => 6
        assert_eq!(pairs.len(), 6, "should select top 50% hardest negatives");
        // The hardest negatives should have high similarity values
        // (0,1)=0.9, (1,0)=0.9, (1,2)=0.8, (2,1)=0.8 should be among the selected
        assert!(pairs.contains(&(0, 1)), "should contain (0,1) sim=0.9");
        assert!(pairs.contains(&(1, 0)), "should contain (1,0) sim=0.9");
    }

    #[test]
    fn test_info_nce_loss_mined_equals_standard_during_warmup() {
        let emb_a = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let emb_b = vec![
            vec![0.9, 0.1, 0.0],
            vec![0.1, 0.9, 0.0],
            vec![0.0, 0.1, 0.9],
        ];
        let miner = HardNegativeMiner::new(0.5, 10); // warmup=10
        let loss_std = info_nce_loss(&emb_a, &emb_b, 0.5);
        let loss_mined = info_nce_loss_mined(&emb_a, &emb_b, 0.5, Some(&miner), 0);
        assert!(
            (loss_std - loss_mined).abs() < 1e-6,
            "during warmup, mined loss should equal standard: {loss_std} vs {loss_mined}"
        );
    }

    // ── Phase 7: Drift detection tests ────────────────────────────────

    #[test]
    fn test_embedding_extractor_drift_detection() {
        let mut ext = EmbeddingExtractor::with_drift_detection(
            small_config(), small_embed_config(), 10,
        );
        // Feed stable CSI for baseline
        for _ in 0..10 {
            let csi = vec![vec![1.0f32; 16]; 4];
            let _ = ext.extract(&csi);
        }
        assert!(!ext.drift_detected(), "stable input should not trigger drift");

        // Feed shifted CSI
        for _ in 0..10 {
            let csi = vec![vec![100.0f32; 16]; 4];
            let _ = ext.extract(&csi);
        }
        assert!(ext.drift_detected(), "large shift should trigger drift");
        let info = ext.drift_info().expect("drift_info should be Some");
        assert!(info.magnitude > 3.0, "drift magnitude should be > 3 sigma");
    }

    #[test]
    fn test_fingerprint_index_anomalous_flag() {
        let mut idx = FingerprintIndex::new(IndexType::EnvironmentFingerprint);
        // Insert normal entries
        idx.insert(vec![1.0, 0.0], "normal".into(), 0);
        idx.insert_with_drift(vec![0.0, 1.0], "drifted".into(), 1, true);
        idx.insert_with_drift(vec![1.0, 1.0], "stable".into(), 2, false);

        assert_eq!(idx.len(), 3);
        assert_eq!(idx.anomalous_count(), 1);
        assert!(!idx.entries[0].anomalous);
        assert!(idx.entries[1].anomalous);
        assert!(!idx.entries[2].anomalous);
    }

    #[test]
    fn test_drift_detector_stable_input_no_drift() {
        let mut ext = EmbeddingExtractor::with_drift_detection(
            small_config(), small_embed_config(), 10,
        );
        // All inputs are the same -- no drift should ever be detected
        for _ in 0..30 {
            let csi = vec![vec![0.5f32; 16]; 4];
            let _ = ext.extract(&csi);
        }
        assert!(!ext.drift_detected(), "constant input should never trigger drift");
    }
}
