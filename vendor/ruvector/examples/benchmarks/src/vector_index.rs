//! Dense Vector Index with IVF and Coherence Gate
//!
//! Fast in-memory vector index implementation with:
//! 1. Fast cosine search
//! 2. Optional IVF-style coarse quantization using kmeans centroids
//! 3. Persistence to disk using serde + bincode
//! 4. Coherence gate for filtering (connects to mincut signals)

use anyhow::{anyhow, Result};
use rand::seq::SliceRandom;
use rand::Rng;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for vectors in the index
pub type VectorId = u64;

/// Dense vector representation with basic operations and cached norm
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DenseVec {
    pub values: Vec<f32>,
    /// Cached L2 norm (computed lazily)
    #[serde(skip, default)]
    cached_norm: Option<f32>,
}

impl DenseVec {
    /// Create a new dense vector from values
    pub fn new(values: Vec<f32>) -> Self {
        Self {
            values,
            cached_norm: None,
        }
    }

    /// Create a new dense vector with precomputed norm
    pub fn with_norm(values: Vec<f32>, norm: f32) -> Self {
        Self {
            values,
            cached_norm: Some(norm),
        }
    }

    /// Create a zero vector of given dimension
    pub fn zeros(dim: usize) -> Self {
        Self {
            values: vec![0.0; dim],
            cached_norm: Some(0.0),
        }
    }

    /// Create a random vector of given dimension
    pub fn random(dim: usize) -> Self {
        let mut rng = rand::thread_rng();
        Self {
            values: (0..dim).map(|_| rng.gen::<f32>()).collect(),
            cached_norm: None,
        }
    }

    /// Create a random normalized vector
    pub fn random_normalized(dim: usize) -> Self {
        let mut v = Self::random(dim);
        v.normalize();
        v
    }

    /// Get vector dimensionality
    pub fn dim(&self) -> usize {
        self.values.len()
    }

    /// Compute L2 norm (SIMD-optimized with caching)
    pub fn l2_norm(&self) -> f32 {
        // Return cached norm if available
        if let Some(norm) = self.cached_norm {
            return norm;
        }

        let n = self.values.len();
        let chunks = n / 4;

        // Process 4 elements at a time
        let mut sum0 = 0.0f32;
        let mut sum1 = 0.0f32;
        let mut sum2 = 0.0f32;
        let mut sum3 = 0.0f32;

        for i in 0..chunks {
            let base = i * 4;
            sum0 += self.values[base] * self.values[base];
            sum1 += self.values[base + 1] * self.values[base + 1];
            sum2 += self.values[base + 2] * self.values[base + 2];
            sum3 += self.values[base + 3] * self.values[base + 3];
        }

        let mut s = sum0 + sum1 + sum2 + sum3;
        for i in (chunks * 4)..n {
            s += self.values[i] * self.values[i];
        }

        s.sqrt()
    }

    /// Compute and cache L2 norm
    pub fn compute_norm(&mut self) -> f32 {
        if self.cached_norm.is_none() {
            let norm = self.l2_norm();
            self.cached_norm = Some(norm);
            norm
        } else {
            self.cached_norm.unwrap()
        }
    }

    /// Compute dot product with another vector (SIMD-optimized for 4-wide chunks)
    pub fn dot(&self, other: &DenseVec) -> Result<f32> {
        if self.dim() != other.dim() {
            return Err(anyhow!(
                "dimension mismatch: {} vs {}",
                self.dim(),
                other.dim()
            ));
        }

        let n = self.values.len();
        let chunks = n / 4;

        // Process 4 elements at a time (auto-vectorizable)
        let mut sum0 = 0.0f32;
        let mut sum1 = 0.0f32;
        let mut sum2 = 0.0f32;
        let mut sum3 = 0.0f32;

        for i in 0..chunks {
            let base = i * 4;
            sum0 += self.values[base] * other.values[base];
            sum1 += self.values[base + 1] * other.values[base + 1];
            sum2 += self.values[base + 2] * other.values[base + 2];
            sum3 += self.values[base + 3] * other.values[base + 3];
        }

        // Handle remainder
        let mut s = sum0 + sum1 + sum2 + sum3;
        for i in (chunks * 4)..n {
            s += self.values[i] * other.values[i];
        }

        Ok(s)
    }

    /// Compute cosine similarity with another vector
    pub fn cosine(&self, other: &DenseVec) -> Result<f32> {
        let d = self.dot(other)?;
        let a = self.l2_norm();
        let b = other.l2_norm();
        if a == 0.0 || b == 0.0 {
            return Ok(0.0);
        }
        Ok(d / (a * b))
    }

    /// Add scaled vector to self (invalidates norm cache)
    pub fn add_scaled(&mut self, other: &DenseVec, scale: f32) -> Result<()> {
        if self.dim() != other.dim() {
            return Err(anyhow!(
                "dimension mismatch: {} vs {}",
                self.dim(),
                other.dim()
            ));
        }
        for i in 0..self.values.len() {
            self.values[i] += other.values[i] * scale;
        }
        self.cached_norm = None; // Invalidate cache
        Ok(())
    }

    /// Scale vector by scalar (invalidates norm cache)
    pub fn scale(&mut self, s: f32) {
        for v in &mut self.values {
            *v *= s;
        }
        // Update cached norm if it exists
        if let Some(norm) = self.cached_norm {
            self.cached_norm = Some(norm * s.abs());
        }
    }

    /// Normalize to unit length (caches norm as 1.0)
    pub fn normalize(&mut self) {
        let norm = self.l2_norm();
        if norm > 0.0 {
            for v in &mut self.values {
                *v /= norm;
            }
            self.cached_norm = Some(1.0);
        }
    }

    /// Invalidate the cached norm (call after modifying values directly)
    pub fn invalidate_cache(&mut self) {
        self.cached_norm = None;
    }
}

/// Search result with ID and score
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScoredId {
    pub id: VectorId,
    pub score: f32,
}

impl ScoredId {
    pub fn new(id: VectorId, score: f32) -> Self {
        Self { id, score }
    }
}

/// Coherence gate configuration
///
/// Filters search results based on coherence score from external signals
/// (e.g., mincut coherence from graph attention)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoherenceGate {
    /// Whether the gate is enabled
    pub enabled: bool,
    /// Minimum coherence score to allow search
    pub min_score: f32,
}

impl Default for CoherenceGate {
    fn default() -> Self {
        Self {
            enabled: false,
            min_score: 0.0,
        }
    }
}

impl CoherenceGate {
    /// Create a new coherence gate
    pub fn new(min_score: f32) -> Self {
        Self {
            enabled: true,
            min_score,
        }
    }

    /// Check if search is allowed given coherence score
    pub fn allow(&self, coherence_score: f32) -> bool {
        if !self.enabled {
            return true;
        }
        coherence_score >= self.min_score
    }
}

/// IVF (Inverted File Index) configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IvfConfig {
    /// Whether IVF is enabled
    pub enabled: bool,
    /// Number of clusters/centroids
    pub clusters: usize,
    /// Number of clusters to probe during search
    pub probes: usize,
    /// K-means iterations for training
    pub iters: usize,
    /// Rebuild IVF after this many inserts
    pub rebuild_every: usize,
}

impl Default for IvfConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            clusters: 64,
            probes: 4,
            iters: 15,
            rebuild_every: 5000,
        }
    }
}

impl IvfConfig {
    /// Create IVF config with specified parameters
    pub fn new(clusters: usize, probes: usize) -> Self {
        Self {
            enabled: true,
            clusters,
            probes,
            iters: 15,
            rebuild_every: 5000,
        }
    }
}

/// Internal IVF state
#[derive(Clone, Serialize, Deserialize)]
struct IvfState {
    /// Cluster centroids
    centroids: Vec<DenseVec>,
    /// Inverted lists: cluster index -> vector IDs
    lists: Vec<Vec<VectorId>>,
    /// Vector ID -> cluster assignment
    assignment: HashMap<VectorId, usize>,
}

/// Main vector index with IVF and coherence gating
#[derive(Clone, Serialize, Deserialize)]
pub struct VectorIndex {
    /// Vector dimensionality
    dim: usize,
    /// Next available vector ID
    next_id: VectorId,
    /// Vector storage
    vectors: HashMap<VectorId, DenseVec>,
    /// Deleted vector IDs
    deleted: HashSet<VectorId>,
    /// Coherence gate configuration
    gate: CoherenceGate,
    /// IVF configuration
    ivf: IvfConfig,
    /// IVF state (centroids and inverted lists)
    ivf_state: Option<IvfState>,
    /// Inserts since last IVF rebuild
    inserts_since_rebuild: usize,
}

impl VectorIndex {
    /// Create a new vector index with given dimensionality
    pub fn new(dim: usize) -> Self {
        Self {
            dim,
            next_id: 1,
            vectors: HashMap::new(),
            deleted: HashSet::new(),
            gate: CoherenceGate::default(),
            ivf: IvfConfig::default(),
            ivf_state: None,
            inserts_since_rebuild: 0,
        }
    }

    /// Configure coherence gate
    pub fn with_gate(mut self, gate: CoherenceGate) -> Self {
        self.gate = gate;
        self
    }

    /// Configure IVF indexing
    pub fn with_ivf(mut self, ivf: IvfConfig) -> Self {
        self.ivf = ivf;
        self
    }

    /// Get vector dimensionality
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Get number of active vectors
    pub fn len(&self) -> usize {
        self.vectors.len().saturating_sub(self.deleted.len())
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Insert a vector and return its ID
    pub fn insert(&mut self, v: DenseVec) -> Result<VectorId> {
        if v.dim() != self.dim {
            return Err(anyhow!(
                "dimension mismatch: expected {}, got {}",
                self.dim,
                v.dim()
            ));
        }
        let id = self.next_id;
        self.next_id += 1;
        self.vectors.insert(id, v);
        self.deleted.remove(&id);
        self.inserts_since_rebuild += 1;

        // Auto-rebuild IVF if threshold reached
        if self.ivf.enabled && self.inserts_since_rebuild >= self.ivf.rebuild_every {
            self.rebuild_ivf()?;
            self.inserts_since_rebuild = 0;
        } else if self.ivf.enabled && self.ivf_state.is_some() {
            // Assign to nearest centroid without full rebuild
            self.ivf_assign_one(id)?;
        }

        Ok(id)
    }

    /// Insert multiple vectors
    pub fn insert_batch(&mut self, vectors: Vec<DenseVec>) -> Result<Vec<VectorId>> {
        let mut ids = Vec::with_capacity(vectors.len());
        for v in vectors {
            ids.push(self.insert(v)?);
        }
        Ok(ids)
    }

    /// Mark a vector as deleted
    pub fn delete(&mut self, id: VectorId) {
        self.deleted.insert(id);
    }

    /// Get a vector by ID
    pub fn get(&self, id: VectorId) -> Option<&DenseVec> {
        if self.deleted.contains(&id) {
            return None;
        }
        self.vectors.get(&id)
    }

    /// Rebuild IVF index using k-means
    pub fn rebuild_ivf(&mut self) -> Result<()> {
        if !self.ivf.enabled {
            return Ok(());
        }

        let k = self.ivf.clusters.max(1);
        let ids: Vec<VectorId> = self
            .vectors
            .keys()
            .copied()
            .filter(|id| !self.deleted.contains(id))
            .collect();

        if ids.is_empty() {
            self.ivf_state = Some(IvfState {
                centroids: Vec::new(),
                lists: Vec::new(),
                assignment: HashMap::new(),
            });
            return Ok(());
        }

        // Collect vectors for k-means
        let samples: Vec<DenseVec> = ids
            .iter()
            .filter_map(|id| self.vectors.get(id).cloned())
            .collect();

        // Run k-means
        let centroids = kmeans(&samples, k, self.ivf.iters)?;

        // Build inverted lists
        let mut lists: Vec<Vec<VectorId>> = vec![Vec::new(); centroids.len()];
        let mut assignment: HashMap<VectorId, usize> = HashMap::new();

        for id in ids {
            let v = self
                .vectors
                .get(&id)
                .ok_or_else(|| anyhow!("missing vector {}", id))?;
            let c = nearest_centroid(v, &centroids)?;
            lists[c].push(id);
            assignment.insert(id, c);
        }

        self.ivf_state = Some(IvfState {
            centroids,
            lists,
            assignment,
        });

        Ok(())
    }

    /// Assign a single vector to its nearest centroid
    fn ivf_assign_one(&mut self, id: VectorId) -> Result<()> {
        let Some(state) = self.ivf_state.as_mut() else {
            return Ok(());
        };
        if state.centroids.is_empty() {
            return Ok(());
        }
        let v = self
            .vectors
            .get(&id)
            .ok_or_else(|| anyhow!("missing vector {}", id))?;
        let c = nearest_centroid(v, &state.centroids)?;
        state.lists[c].push(id);
        state.assignment.insert(id, c);
        Ok(())
    }

    /// Search for nearest neighbors
    ///
    /// # Arguments
    /// * `q` - Query vector
    /// * `top_k` - Number of results to return
    /// * `coherence_score` - External coherence signal (e.g., from mincut)
    pub fn search(
        &self,
        q: &DenseVec,
        top_k: usize,
        coherence_score: f32,
    ) -> Result<Vec<ScoredId>> {
        if q.dim() != self.dim {
            return Err(anyhow!(
                "dimension mismatch: expected {}, got {}",
                self.dim,
                q.dim()
            ));
        }

        // Check coherence gate
        if !self.gate.allow(coherence_score) {
            return Ok(Vec::new());
        }

        if top_k == 0 {
            return Ok(Vec::new());
        }

        // Use IVF search if available
        if self.ivf.enabled {
            if let Some(state) = &self.ivf_state {
                return self.search_ivf(q, top_k, state);
            }
        }

        // Fall back to flat search
        self.search_flat(q, top_k)
    }

    /// Flat (brute-force) search
    fn search_flat(&self, q: &DenseVec, top_k: usize) -> Result<Vec<ScoredId>> {
        let mut best: Vec<ScoredId> = Vec::new();
        for (id, v) in &self.vectors {
            if self.deleted.contains(id) {
                continue;
            }
            let s = q.cosine(v)?;
            push_topk(&mut best, *id, s, top_k);
        }
        best.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(best)
    }

    /// IVF-based search (probe nearest clusters)
    fn search_ivf(&self, q: &DenseVec, top_k: usize, state: &IvfState) -> Result<Vec<ScoredId>> {
        if state.centroids.is_empty() {
            return Ok(Vec::new());
        }

        // Find nearest centroids
        let mut centroid_scores: Vec<(usize, f32)> = Vec::with_capacity(state.centroids.len());
        for i in 0..state.centroids.len() {
            let s = q.cosine(&state.centroids[i])?;
            centroid_scores.push((i, s));
        }
        centroid_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Probe top clusters
        let probes = self.ivf.probes.max(1).min(centroid_scores.len());
        let mut candidates: Vec<VectorId> = Vec::new();
        for i in 0..probes {
            let c = centroid_scores[i].0;
            for id in &state.lists[c] {
                if !self.deleted.contains(id) {
                    candidates.push(*id);
                }
            }
        }

        // Score candidates
        let mut best: Vec<ScoredId> = Vec::new();
        for id in candidates {
            let v = self
                .vectors
                .get(&id)
                .ok_or_else(|| anyhow!("missing vector {}", id))?;
            let s = q.cosine(v)?;
            push_topk(&mut best, id, s, top_k);
        }
        best.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(best)
    }

    /// Parallel batch search for multiple queries
    ///
    /// Processes multiple queries in parallel using rayon
    pub fn search_batch(
        &self,
        queries: &[DenseVec],
        top_k: usize,
        coherence_score: f32,
    ) -> Vec<Result<Vec<ScoredId>>> {
        queries
            .par_iter()
            .map(|q| self.search(q, top_k, coherence_score))
            .collect()
    }

    /// Parallel flat search using rayon
    fn search_flat_parallel(&self, q: &DenseVec, top_k: usize) -> Result<Vec<ScoredId>> {
        // Collect active vectors
        let active: Vec<(VectorId, &DenseVec)> = self
            .vectors
            .iter()
            .filter(|(id, _)| !self.deleted.contains(id))
            .map(|(id, v)| (*id, v))
            .collect();

        // Parallel scoring
        let mut scores: Vec<ScoredId> = active
            .par_iter()
            .filter_map(|(id, v)| q.cosine(v).ok().map(|s| ScoredId::new(*id, s)))
            .collect();

        // Sort and truncate
        scores.par_sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scores.truncate(top_k);
        Ok(scores)
    }

    /// Search with adaptive probe count based on query difficulty
    pub fn search_adaptive(
        &self,
        q: &DenseVec,
        top_k: usize,
        coherence_score: f32,
        min_candidates: usize,
    ) -> Result<Vec<ScoredId>> {
        if q.dim() != self.dim {
            return Err(anyhow!(
                "dimension mismatch: expected {}, got {}",
                self.dim,
                q.dim()
            ));
        }

        if !self.gate.allow(coherence_score) {
            return Ok(Vec::new());
        }

        if top_k == 0 {
            return Ok(Vec::new());
        }

        // Use adaptive IVF search if available
        if self.ivf.enabled {
            if let Some(state) = &self.ivf_state {
                return self.search_ivf_adaptive(q, top_k, state, min_candidates);
            }
        }

        // Large indices benefit from parallel search
        if self.len() > 1000 {
            return self.search_flat_parallel(q, top_k);
        }

        self.search_flat(q, top_k)
    }

    /// IVF search with adaptive probe count (SIMD-optimized centroid scoring)
    fn search_ivf_adaptive(
        &self,
        q: &DenseVec,
        top_k: usize,
        state: &IvfState,
        min_candidates: usize,
    ) -> Result<Vec<ScoredId>> {
        if state.centroids.is_empty() {
            return Ok(Vec::new());
        }

        // SIMD-optimized centroid scoring using parallel iterator
        let centroid_scores: Vec<(usize, f32)> = state
            .centroids
            .par_iter()
            .enumerate()
            .filter_map(|(i, c)| q.cosine(c).ok().map(|score| (i, score)))
            .collect();

        // Sort by score descending
        let mut sorted_scores = centroid_scores;
        sorted_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Adaptive probe count: start with default, increase if needed
        let initial_probes = self.ivf.probes.min(sorted_scores.len());
        let max_probes = sorted_scores.len().min(initial_probes * 2);

        let mut all_candidates: Vec<ScoredId> = Vec::new();
        let mut _probed = 0;

        // First pass with initial probes
        for &(cluster_idx, _) in sorted_scores.iter().take(initial_probes) {
            if cluster_idx < state.lists.len() {
                for &id in &state.lists[cluster_idx] {
                    if !self.deleted.contains(&id) {
                        if let Some(v) = self.vectors.get(&id) {
                            if let Ok(score) = q.cosine(v) {
                                all_candidates.push(ScoredId::new(id, score));
                            }
                        }
                    }
                }
            }
            _probed += 1;
        }

        // If not enough candidates, probe more clusters
        if all_candidates.len() < min_candidates && _probed < max_probes {
            for &(cluster_idx, _) in sorted_scores
                .iter()
                .skip(initial_probes)
                .take(max_probes - initial_probes)
            {
                if cluster_idx < state.lists.len() {
                    for &id in &state.lists[cluster_idx] {
                        if !self.deleted.contains(&id) {
                            if let Some(v) = self.vectors.get(&id) {
                                if let Ok(score) = q.cosine(v) {
                                    all_candidates.push(ScoredId::new(id, score));
                                }
                            }
                        }
                    }
                }
                _probed += 1;
                if all_candidates.len() >= min_candidates {
                    break;
                }
            }
        }

        // Sort and return top-k
        all_candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        all_candidates.truncate(top_k);
        Ok(all_candidates)
    }

    /// Original IVF search (kept for compatibility)
    #[allow(dead_code)]
    fn search_ivf_adaptive_original(
        &self,
        q: &DenseVec,
        top_k: usize,
        state: &IvfState,
        min_candidates: usize,
    ) -> Result<Vec<ScoredId>> {
        if state.centroids.is_empty() {
            return Ok(Vec::new());
        }

        // Score all centroids
        let mut centroid_scores: Vec<(usize, f32)> = Vec::with_capacity(state.centroids.len());
        for i in 0..state.centroids.len() {
            let s = q.cosine(&state.centroids[i])?;
            centroid_scores.push((i, s));
        }
        centroid_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Adaptive probing: probe until we have enough candidates
        let mut candidates: Vec<VectorId> = Vec::new();
        let mut probed = 0;

        for (c, _) in &centroid_scores {
            for id in &state.lists[*c] {
                if !self.deleted.contains(id) {
                    candidates.push(*id);
                }
            }
            probed += 1;

            // Stop if we have enough candidates
            if candidates.len() >= min_candidates.max(top_k * 10) {
                break;
            }
        }

        // Parallel scoring of candidates
        let best: Vec<ScoredId> = candidates
            .par_iter()
            .filter_map(|id| {
                self.vectors
                    .get(id)
                    .and_then(|v| q.cosine(v).ok().map(|s| ScoredId::new(*id, s)))
            })
            .collect();

        // Sort and truncate
        let mut sorted = best;
        sorted.par_sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.truncate(top_k);
        Ok(sorted)
    }

    /// Save index to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let bytes = bincode::serde::encode_to_vec(self, bincode::config::standard())?;
        fs::write(path, bytes)?;
        Ok(())
    }

    /// Load index from file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let bytes = fs::read(path)?;
        let (idx, _): (Self, _) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::standard())?;
        Ok(idx)
    }

    /// Get index statistics
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            dim: self.dim,
            total_vectors: self.vectors.len(),
            active_vectors: self.len(),
            deleted_vectors: self.deleted.len(),
            ivf_enabled: self.ivf.enabled,
            ivf_clusters: self
                .ivf_state
                .as_ref()
                .map(|s| s.centroids.len())
                .unwrap_or(0),
            gate_enabled: self.gate.enabled,
            gate_min_score: self.gate.min_score,
        }
    }
}

/// Index statistics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexStats {
    pub dim: usize,
    pub total_vectors: usize,
    pub active_vectors: usize,
    pub deleted_vectors: usize,
    pub ivf_enabled: bool,
    pub ivf_clusters: usize,
    pub gate_enabled: bool,
    pub gate_min_score: f32,
}

/// Maintain top-k results
fn push_topk(best: &mut Vec<ScoredId>, id: VectorId, score: f32, top_k: usize) {
    if best.len() < top_k {
        best.push(ScoredId { id, score });
        return;
    }
    // Find worst score
    let mut worst_i = 0usize;
    let mut worst_s = best[0].score;
    for i in 1..best.len() {
        if best[i].score < worst_s {
            worst_s = best[i].score;
            worst_i = i;
        }
    }
    // Replace if better
    if score > worst_s {
        best[worst_i] = ScoredId { id, score };
    }
}

/// Find nearest centroid by cosine similarity
fn nearest_centroid(v: &DenseVec, centroids: &[DenseVec]) -> Result<usize> {
    let mut best_i = 0usize;
    let mut best_s = f32::NEG_INFINITY;
    for i in 0..centroids.len() {
        let s = v.cosine(&centroids[i])?;
        if s > best_s {
            best_s = s;
            best_i = i;
        }
    }
    Ok(best_i)
}

/// K-means clustering
fn kmeans(points: &[DenseVec], k: usize, iters: usize) -> Result<Vec<DenseVec>> {
    if points.is_empty() {
        return Ok(Vec::new());
    }
    let dim = points[0].dim();
    for p in points {
        if p.dim() != dim {
            return Err(anyhow!("dimension mismatch in kmeans"));
        }
    }

    let mut rng = rand::thread_rng();

    // Initialize centroids with k-means++
    let mut centroids: Vec<DenseVec> = Vec::new();
    let mut pool: Vec<usize> = (0..points.len()).collect();
    pool.shuffle(&mut rng);

    let k_eff = k.min(points.len());
    for i in 0..k_eff {
        centroids.push(points[pool[i]].clone());
    }

    // Iterate
    for _ in 0..iters {
        let mut sums: Vec<DenseVec> = (0..centroids.len()).map(|_| DenseVec::zeros(dim)).collect();
        let mut counts: Vec<usize> = vec![0; centroids.len()];

        // Assign points to centroids
        for p in points {
            let c = nearest_centroid(p, &centroids)?;
            sums[c].add_scaled(p, 1.0)?;
            counts[c] += 1;
        }

        // Update centroids
        for i in 0..centroids.len() {
            if counts[i] == 0 {
                // Reinitialize empty cluster
                let pick = rng.gen_range(0..points.len());
                centroids[i] = points[pick].clone();
            } else {
                sums[i].scale(1.0 / counts[i] as f32);
                centroids[i] = sums[i].clone();
            }
        }
    }

    Ok(centroids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dense_vec_operations() {
        let v1 = DenseVec::new(vec![1.0, 0.0, 0.0]);
        let v2 = DenseVec::new(vec![0.0, 1.0, 0.0]);
        let v3 = DenseVec::new(vec![1.0, 0.0, 0.0]);

        assert!((v1.cosine(&v3).unwrap() - 1.0).abs() < 1e-6);
        assert!((v1.cosine(&v2).unwrap() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_vector_index_basic() {
        let mut idx = VectorIndex::new(4);

        let id1 = idx.insert(DenseVec::new(vec![1.0, 0.0, 0.0, 0.0])).unwrap();
        let id2 = idx.insert(DenseVec::new(vec![0.9, 0.1, 0.0, 0.0])).unwrap();
        let id3 = idx.insert(DenseVec::new(vec![0.0, 1.0, 0.0, 0.0])).unwrap();

        assert_eq!(idx.len(), 3);

        let q = DenseVec::new(vec![1.0, 0.0, 0.0, 0.0]);
        let results = idx.search(&q, 2, 1.0).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, id1);
        assert!(results[0].score > 0.99);
    }

    #[test]
    fn test_coherence_gate() {
        let gate = CoherenceGate::new(0.5);
        let mut idx = VectorIndex::new(4).with_gate(gate);

        idx.insert(DenseVec::new(vec![1.0, 0.0, 0.0, 0.0])).unwrap();

        let q = DenseVec::new(vec![1.0, 0.0, 0.0, 0.0]);

        // Low coherence - should return empty
        let results = idx.search(&q, 1, 0.3).unwrap();
        assert!(results.is_empty());

        // High coherence - should return result
        let results = idx.search(&q, 1, 0.7).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_ivf_index() {
        let ivf = IvfConfig::new(4, 2);
        let mut idx = VectorIndex::new(4).with_ivf(ivf);

        // Insert vectors
        for _ in 0..100 {
            idx.insert(DenseVec::random(4)).unwrap();
        }

        // Build IVF
        idx.rebuild_ivf().unwrap();

        let stats = idx.stats();
        assert!(stats.ivf_enabled);
        assert!(stats.ivf_clusters > 0);

        // Search should work
        let q = DenseVec::random(4);
        let results = idx.search(&q, 5, 1.0).unwrap();
        assert!(results.len() <= 5);
    }
}
