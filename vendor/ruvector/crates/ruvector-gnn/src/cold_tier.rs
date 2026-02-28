//! Cold-tier GNN training via hyperbatch I/O for graphs exceeding RAM.
//!
//! Implements AGNES-style block-aligned I/O with hotset caching
//! for training on large-scale graphs that don't fit in memory.

#![cfg(all(feature = "cold-tier", not(target_arch = "wasm32")))]

use crate::error::{GnnError, Result};
use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Size of an f32 in bytes.
const F32_SIZE: usize = std::mem::size_of::<f32>();

/// Header size in bytes: dim (u64) + num_nodes (u64) + block_size (u64).
const HEADER_SIZE: u64 = 24;

/// Return the system page size, falling back to 4096.
fn system_page_size() -> usize {
    page_size::get()
}

/// Align `value` up to the nearest multiple of `alignment`.
fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) / alignment * alignment
}

// ---------------------------------------------------------------------------
// FeatureStorage
// ---------------------------------------------------------------------------

/// Block-aligned feature file for storing node feature vectors on disk.
pub struct FeatureStorage {
    path: PathBuf,
    dim: usize,
    num_nodes: usize,
    block_size: usize,
    file: Option<File>,
}

impl FeatureStorage {
    /// Create a new feature file at `path` for `num_nodes` with dimension `dim`.
    pub fn create(path: &Path, dim: usize, num_nodes: usize) -> Result<Self> {
        if dim == 0 {
            return Err(GnnError::invalid_input("dim must be > 0"));
        }
        let block_size = align_up(dim * F32_SIZE, system_page_size());
        let data_size = num_nodes as u64 * block_size as u64;

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .map_err(|e| GnnError::Io(e))?;

        // Write header
        file.write_all(&(dim as u64).to_le_bytes())?;
        file.write_all(&(num_nodes as u64).to_le_bytes())?;
        file.write_all(&(block_size as u64).to_le_bytes())?;

        // Extend file to full size
        file.set_len(HEADER_SIZE + data_size)?;

        Ok(Self {
            path: path.to_path_buf(),
            dim,
            num_nodes,
            block_size,
            file: Some(file),
        })
    }

    /// Open an existing feature file.
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| GnnError::Io(e))?;

        let mut buf = [0u8; 8];
        file.read_exact(&mut buf)?;
        let dim = u64::from_le_bytes(buf) as usize;
        file.read_exact(&mut buf)?;
        let num_nodes = u64::from_le_bytes(buf) as usize;
        file.read_exact(&mut buf)?;
        let block_size = u64::from_le_bytes(buf) as usize;

        Ok(Self {
            path: path.to_path_buf(),
            dim,
            num_nodes,
            block_size,
            file: Some(file),
        })
    }

    /// Write feature vector for a single node.
    pub fn write_features(&mut self, node_id: usize, features: &[f32]) -> Result<()> {
        if node_id >= self.num_nodes {
            return Err(GnnError::invalid_input(format!(
                "node_id {} out of bounds (num_nodes={})",
                node_id, self.num_nodes
            )));
        }
        if features.len() != self.dim {
            return Err(GnnError::dimension_mismatch(
                self.dim.to_string(),
                features.len().to_string(),
            ));
        }
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| GnnError::other("file not open"))?;
        let offset = HEADER_SIZE + (node_id as u64) * (self.block_size as u64);
        file.seek(SeekFrom::Start(offset))?;
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(features.as_ptr() as *const u8, features.len() * F32_SIZE)
        };
        file.write_all(bytes)?;
        Ok(())
    }

    /// Read feature vector for a single node.
    pub fn read_features(&mut self, node_id: usize) -> Result<Vec<f32>> {
        if node_id >= self.num_nodes {
            return Err(GnnError::invalid_input(format!(
                "node_id {} out of bounds (num_nodes={})",
                node_id, self.num_nodes
            )));
        }
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| GnnError::other("file not open"))?;
        let offset = HEADER_SIZE + (node_id as u64) * (self.block_size as u64);
        file.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; self.dim * F32_SIZE];
        file.read_exact(&mut buf)?;
        let features: Vec<f32> = buf
            .chunks_exact(F32_SIZE)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        Ok(features)
    }

    /// Batch-read features for multiple nodes with block-aligned I/O.
    pub fn read_batch(&mut self, node_ids: &[usize]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(node_ids.len());
        // Sort node_ids to improve sequential I/O locality
        let mut sorted: Vec<usize> = node_ids.to_vec();
        sorted.sort_unstable();
        // Read in sorted order, then reorder to match input
        let mut map: HashMap<usize, Vec<f32>> = HashMap::with_capacity(sorted.len());
        for &nid in &sorted {
            if !map.contains_key(&nid) {
                map.insert(nid, self.read_features(nid)?);
            }
        }
        for &nid in node_ids {
            results.push(map[&nid].clone());
        }
        Ok(results)
    }

    /// Flush pending writes to disk.
    pub fn flush(&mut self) -> Result<()> {
        if let Some(ref mut f) = self.file {
            f.flush()?;
        }
        Ok(())
    }

    /// Dimension of each feature vector.
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Number of nodes in the storage.
    pub fn num_nodes(&self) -> usize {
        self.num_nodes
    }

    /// Path to the underlying file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

// ---------------------------------------------------------------------------
// HyperbatchConfig / HyperbatchResult
// ---------------------------------------------------------------------------

/// Configuration for hyperbatch I/O.
#[derive(Debug, Clone)]
pub struct HyperbatchConfig {
    /// Nodes per hyperbatch (default: 4096).
    pub batch_size: usize,
    /// Prefetch multiplier (default: 2).
    pub prefetch_factor: usize,
    /// I/O block alignment in bytes (default: 4096).
    pub block_align: usize,
    /// Double-buffering count (default: 2).
    pub num_buffers: usize,
    /// Fraction of nodes kept in the hotset (default: 0.05).
    pub hotset_fraction: f64,
}

impl Default for HyperbatchConfig {
    fn default() -> Self {
        Self {
            batch_size: 4096,
            prefetch_factor: 2,
            block_align: 4096,
            num_buffers: 2,
            hotset_fraction: 0.05,
        }
    }
}

/// Result from a single hyperbatch iteration.
#[derive(Debug, Clone)]
pub struct HyperbatchResult {
    /// Node identifiers in this batch.
    pub node_ids: Vec<usize>,
    /// Feature vectors for each node.
    pub features: Vec<Vec<f32>>,
    /// Zero-based index of this batch within the epoch.
    pub batch_index: usize,
}

// ---------------------------------------------------------------------------
// HyperbatchIterator
// ---------------------------------------------------------------------------

/// Yields batches from disk following BFS vertex ordering for I/O locality.
pub struct HyperbatchIterator {
    storage: FeatureStorage,
    config: HyperbatchConfig,
    node_order: Vec<usize>,
    current_offset: usize,
    buffers: Vec<Vec<Vec<f32>>>,
    active_buffer: usize,
    batch_counter: usize,
}

impl HyperbatchIterator {
    /// Create a new iterator with BFS-ordered node traversal.
    pub fn new(
        storage: FeatureStorage,
        config: HyperbatchConfig,
        adjacency: &[(usize, usize)],
    ) -> Self {
        let num_nodes = storage.num_nodes();
        let node_order = Self::reorder_bfs(adjacency, num_nodes);
        let num_buffers = config.num_buffers.max(1);
        let buffers = vec![Vec::new(); num_buffers];
        Self {
            storage,
            config,
            node_order,
            current_offset: 0,
            buffers,
            active_buffer: 0,
            batch_counter: 0,
        }
    }

    /// Get the next batch, or `None` when the epoch is complete.
    pub fn next_batch(&mut self) -> Option<HyperbatchResult> {
        if self.current_offset >= self.node_order.len() {
            return None;
        }
        let end = (self.current_offset + self.config.batch_size).min(self.node_order.len());
        let node_ids: Vec<usize> = self.node_order[self.current_offset..end].to_vec();
        let features = self.storage.read_batch(&node_ids).ok()?;

        // Store in active buffer for potential re-use
        let buf_idx = self.active_buffer % self.buffers.len();
        self.buffers[buf_idx] = features.clone();
        self.active_buffer += 1;

        let batch_index = self.batch_counter;
        self.batch_counter += 1;
        self.current_offset = end;

        Some(HyperbatchResult {
            node_ids,
            features,
            batch_index,
        })
    }

    /// Reset the iterator to the beginning of the epoch.
    pub fn reset(&mut self) {
        self.current_offset = 0;
        self.batch_counter = 0;
        self.active_buffer = 0;
    }

    /// Produce a BFS vertex ordering for better I/O locality.
    pub fn reorder_bfs(adjacency: &[(usize, usize)], num_nodes: usize) -> Vec<usize> {
        if num_nodes == 0 {
            return Vec::new();
        }
        // Build adjacency list
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); num_nodes];
        for &(u, v) in adjacency {
            if u < num_nodes && v < num_nodes {
                adj[u].push(v);
                adj[v].push(u);
            }
        }

        let mut visited = vec![false; num_nodes];
        let mut order = Vec::with_capacity(num_nodes);
        let mut queue = VecDeque::new();

        // BFS from node 0; handle disconnected components
        for start in 0..num_nodes {
            if visited[start] {
                continue;
            }
            visited[start] = true;
            queue.push_back(start);
            while let Some(node) = queue.pop_front() {
                order.push(node);
                for &neighbor in &adj[node] {
                    if !visited[neighbor] {
                        visited[neighbor] = true;
                        queue.push_back(neighbor);
                    }
                }
            }
        }
        order
    }
}

// ---------------------------------------------------------------------------
// AdaptiveHotset
// ---------------------------------------------------------------------------

/// In-memory cache of frequently accessed node features.
pub struct AdaptiveHotset {
    features: HashMap<usize, Vec<f32>>,
    access_counts: HashMap<usize, u64>,
    capacity: usize,
    decay_factor: f64,
    total_lookups: u64,
    hits: u64,
}

impl AdaptiveHotset {
    /// Create a new hotset with the given capacity and decay factor.
    pub fn new(capacity: usize, decay_factor: f64) -> Self {
        Self {
            features: HashMap::with_capacity(capacity),
            access_counts: HashMap::with_capacity(capacity),
            capacity,
            decay_factor,
            total_lookups: 0,
            hits: 0,
        }
    }

    /// O(1) lookup of cached features.
    pub fn get(&mut self, node_id: usize) -> Option<&[f32]> {
        self.total_lookups += 1;
        if self.features.contains_key(&node_id) {
            self.hits += 1;
            *self.access_counts.entry(node_id).or_insert(0) += 1;
            // Safety: we just confirmed the key exists
            Some(self.features.get(&node_id).unwrap().as_slice())
        } else {
            None
        }
    }

    /// Insert features, evicting the coldest entry if at capacity.
    pub fn insert(&mut self, node_id: usize, features: Vec<f32>) {
        if self.features.len() >= self.capacity && !self.features.contains_key(&node_id) {
            self.evict_cold();
        }
        self.access_counts.entry(node_id).or_insert(0);
        self.features.insert(node_id, features);
    }

    /// Record an access without returning features (for tracking frequency).
    pub fn record_access(&mut self, node_id: usize) {
        *self.access_counts.entry(node_id).or_insert(0) += 1;
    }

    /// Evict the least-accessed node from the hotset.
    pub fn evict_cold(&mut self) {
        if self.access_counts.is_empty() {
            return;
        }
        // Find the node with the lowest access count that is cached
        let coldest = self
            .features
            .keys()
            .min_by_key(|nid| self.access_counts.get(nid).copied().unwrap_or(0))
            .copied();
        if let Some(nid) = coldest {
            self.features.remove(&nid);
            self.access_counts.remove(&nid);
        }
    }

    /// Cache hit rate since creation.
    pub fn hit_rate(&self) -> f64 {
        if self.total_lookups == 0 {
            return 0.0;
        }
        self.hits as f64 / self.total_lookups as f64
    }

    /// Multiply all access counts by `decay_factor` to age out stale entries.
    pub fn decay_counts(&mut self) {
        for count in self.access_counts.values_mut() {
            *count = (*count as f64 * self.decay_factor) as u64;
        }
    }

    /// Number of nodes currently cached.
    pub fn len(&self) -> usize {
        self.features.len()
    }

    /// Whether the hotset is empty.
    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ColdTierEpochResult
// ---------------------------------------------------------------------------

/// Statistics from one cold-tier training epoch.
#[derive(Debug, Clone)]
pub struct ColdTierEpochResult {
    /// Epoch number.
    pub epoch: usize,
    /// Average loss across all batches.
    pub avg_loss: f64,
    /// Number of batches processed.
    pub batches: usize,
    /// Hotset hit rate during this epoch.
    pub hotset_hit_rate: f64,
    /// Milliseconds spent on I/O.
    pub io_time_ms: u64,
    /// Milliseconds spent on compute.
    pub compute_time_ms: u64,
}

// ---------------------------------------------------------------------------
// ColdTierTrainer
// ---------------------------------------------------------------------------

/// Orchestrates cold-tier training with hyperbatch I/O and hotset caching.
pub struct ColdTierTrainer {
    storage: FeatureStorage,
    hotset: AdaptiveHotset,
    config: HyperbatchConfig,
    epoch: usize,
    total_loss: f64,
    batches_processed: usize,
}

impl ColdTierTrainer {
    /// Create a new trainer, initializing feature storage and hotset.
    pub fn new(
        storage_path: &Path,
        dim: usize,
        num_nodes: usize,
        config: HyperbatchConfig,
    ) -> Result<Self> {
        let storage = FeatureStorage::create(storage_path, dim, num_nodes)?;
        let hotset_cap = ((num_nodes as f64) * config.hotset_fraction).max(1.0) as usize;
        let hotset = AdaptiveHotset::new(hotset_cap, 0.95);
        Ok(Self {
            storage,
            hotset,
            config,
            epoch: 0,
            total_loss: 0.0,
            batches_processed: 0,
        })
    }

    /// Run one training epoch over all hyperbatches.
    ///
    /// For each batch a simple gradient-descent step is simulated:
    /// the loss is the L2 norm of the feature vector, and the gradient
    /// nudges each element toward zero by `learning_rate`.
    pub fn train_epoch(
        &mut self,
        adjacency: &[(usize, usize)],
        learning_rate: f64,
    ) -> ColdTierEpochResult {
        let io_start = std::time::Instant::now();

        // Build a fresh iterator each epoch (re-shuffles BFS ordering)
        let storage_for_iter = FeatureStorage::open(self.storage.path()).ok();
        let mut epoch_loss = 0.0;
        let mut batch_count: usize = 0;
        let mut io_ms: u64 = 0;
        let mut compute_ms: u64 = 0;

        if let Some(iter_storage) = storage_for_iter {
            let mut iter = HyperbatchIterator::new(iter_storage, self.config.clone(), adjacency);

            while let Some(batch) = iter.next_batch() {
                let io_elapsed = io_start.elapsed().as_millis() as u64;

                let compute_start = std::time::Instant::now();

                // Process each node in the batch
                for (i, node_id) in batch.node_ids.iter().enumerate() {
                    let features = &batch.features[i];

                    // Simple L2 loss for demonstration
                    let loss: f64 = features
                        .iter()
                        .map(|&x| (x as f64) * (x as f64))
                        .sum::<f64>()
                        * 0.5;
                    epoch_loss += loss;

                    // Gradient: d(0.5 * x^2)/dx = x; step: x' = x - lr * x
                    let updated: Vec<f32> = features
                        .iter()
                        .map(|&x| x - (learning_rate as f32) * x)
                        .collect();

                    let _ = self.storage.write_features(*node_id, &updated);
                    self.hotset.insert(*node_id, updated);
                }

                compute_ms += compute_start.elapsed().as_millis() as u64;
                io_ms = io_elapsed;
                batch_count += 1;
            }
        }

        let _ = self.storage.flush();
        self.hotset.decay_counts();
        self.epoch += 1;
        self.total_loss = if batch_count > 0 {
            epoch_loss / batch_count as f64
        } else {
            0.0
        };
        self.batches_processed = batch_count;

        ColdTierEpochResult {
            epoch: self.epoch,
            avg_loss: self.total_loss,
            batches: batch_count,
            hotset_hit_rate: self.hotset.hit_rate(),
            io_time_ms: io_ms,
            compute_time_ms: compute_ms,
        }
    }

    /// Retrieve features for a node, checking the hotset first.
    pub fn get_features(&mut self, node_id: usize) -> Result<Vec<f32>> {
        if let Some(cached) = self.hotset.get(node_id) {
            return Ok(cached.to_vec());
        }
        let features = self.storage.read_features(node_id)?;
        self.hotset.insert(node_id, features.clone());
        Ok(features)
    }

    /// Save a checkpoint (header + storage path + hotset metadata).
    pub fn save_checkpoint(&self, path: &Path) -> Result<()> {
        let data = serde_json::json!({
            "storage_path": self.storage.path().to_string_lossy(),
            "dim": self.storage.dim(),
            "num_nodes": self.storage.num_nodes(),
            "epoch": self.epoch,
            "total_loss": self.total_loss,
            "batches_processed": self.batches_processed,
            "config": {
                "batch_size": self.config.batch_size,
                "prefetch_factor": self.config.prefetch_factor,
                "block_align": self.config.block_align,
                "num_buffers": self.config.num_buffers,
                "hotset_fraction": self.config.hotset_fraction,
            }
        });
        let content = serde_json::to_string_pretty(&data)
            .map_err(|e| GnnError::other(format!("serialize checkpoint: {}", e)))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Load a trainer from a checkpoint file.
    pub fn load_checkpoint(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let v: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| GnnError::other(format!("deserialize checkpoint: {}", e)))?;

        let storage_path = PathBuf::from(
            v["storage_path"]
                .as_str()
                .ok_or_else(|| GnnError::other("missing storage_path"))?,
        );
        let _dim = v["dim"].as_u64().unwrap_or(0) as usize;
        let num_nodes = v["num_nodes"].as_u64().unwrap_or(0) as usize;
        let epoch = v["epoch"].as_u64().unwrap_or(0) as usize;
        let total_loss = v["total_loss"].as_f64().unwrap_or(0.0);
        let batches_processed = v["batches_processed"].as_u64().unwrap_or(0) as usize;

        let cfg_val = &v["config"];
        let config = HyperbatchConfig {
            batch_size: cfg_val["batch_size"].as_u64().unwrap_or(4096) as usize,
            prefetch_factor: cfg_val["prefetch_factor"].as_u64().unwrap_or(2) as usize,
            block_align: cfg_val["block_align"].as_u64().unwrap_or(4096) as usize,
            num_buffers: cfg_val["num_buffers"].as_u64().unwrap_or(2) as usize,
            hotset_fraction: cfg_val["hotset_fraction"].as_f64().unwrap_or(0.05),
        };

        let storage = FeatureStorage::open(&storage_path).map_err(|_| {
            // If the storage file no longer exists, recreate it
            GnnError::other("storage file not found; re-create before loading")
        })?;

        let hotset_cap = ((num_nodes as f64) * config.hotset_fraction).max(1.0) as usize;
        let hotset = AdaptiveHotset::new(hotset_cap, 0.95);

        Ok(Self {
            storage,
            hotset,
            config,
            epoch,
            total_loss,
            batches_processed,
        })
    }
}

// ---------------------------------------------------------------------------
// ColdTierEwc
// ---------------------------------------------------------------------------

/// Disk-backed Elastic Weight Consolidation using FeatureStorage.
///
/// Stores Fisher information diagonal and anchor weights on disk
/// so that EWC can scale to models that do not fit in RAM.
pub struct ColdTierEwc {
    fisher_storage: FeatureStorage,
    anchor_storage: FeatureStorage,
    lambda: f64,
    active: bool,
    dim: usize,
    num_params: usize,
}

impl ColdTierEwc {
    /// Create a new disk-backed EWC instance.
    ///
    /// `dim` is the width of each parameter "row" (analogous to feature dim),
    /// and `num_params` is the number of such rows.
    pub fn new(path: &Path, dim: usize, num_params: usize, lambda: f64) -> Result<Self> {
        let fisher_path = path.join("fisher.bin");
        let anchor_path = path.join("anchor.bin");
        std::fs::create_dir_all(path)?;
        let fisher_storage = FeatureStorage::create(&fisher_path, dim, num_params)?;
        let anchor_storage = FeatureStorage::create(&anchor_path, dim, num_params)?;
        Ok(Self {
            fisher_storage,
            anchor_storage,
            lambda,
            active: false,
            dim,
            num_params,
        })
    }

    /// Compute Fisher information diagonal from gradient samples.
    ///
    /// Each entry in `gradients` is one sample's gradient for one parameter row.
    pub fn compute_fisher(&mut self, gradients: &[Vec<f32>], sample_count: usize) -> Result<()> {
        if gradients.is_empty() {
            return Ok(());
        }
        let rows = gradients.len() / self.num_params;
        if rows == 0 {
            return Ok(());
        }
        let norm = 1.0 / (sample_count as f32).max(1.0);

        for param_idx in 0..self.num_params {
            let mut fisher_row = vec![0.0f32; self.dim];
            for sample in 0..rows {
                let idx = sample * self.num_params + param_idx;
                if idx < gradients.len() {
                    let grad = &gradients[idx];
                    for (i, &g) in grad.iter().enumerate().take(self.dim) {
                        fisher_row[i] += g * g;
                    }
                }
            }
            for v in &mut fisher_row {
                *v *= norm;
            }
            self.fisher_storage.write_features(param_idx, &fisher_row)?;
        }
        self.fisher_storage.flush()?;
        Ok(())
    }

    /// Consolidate current weights as anchors and activate EWC.
    pub fn consolidate(&mut self, current_weights: &[Vec<f32>]) -> Result<()> {
        if current_weights.len() != self.num_params {
            return Err(GnnError::dimension_mismatch(
                self.num_params.to_string(),
                current_weights.len().to_string(),
            ));
        }
        for (i, w) in current_weights.iter().enumerate() {
            self.anchor_storage.write_features(i, w)?;
        }
        self.anchor_storage.flush()?;
        self.active = true;
        Ok(())
    }

    /// Compute the EWC penalty: lambda/2 * sum(F_i * (w_i - w*_i)^2).
    pub fn penalty(&mut self, current_weights: &[Vec<f32>]) -> Result<f64> {
        if !self.active {
            return Ok(0.0);
        }
        let mut total = 0.0f64;
        for i in 0..self.num_params {
            let fisher = self.fisher_storage.read_features(i)?;
            let anchor = self.anchor_storage.read_features(i)?;
            let w = &current_weights[i];
            for j in 0..self.dim.min(w.len()) {
                let diff = w[j] - anchor[j];
                total += (fisher[j] as f64) * (diff as f64) * (diff as f64);
            }
        }
        Ok(total * self.lambda * 0.5)
    }

    /// Compute the EWC gradient for a specific parameter row.
    pub fn gradient(&mut self, current_weights: &[Vec<f32>], param_idx: usize) -> Result<Vec<f32>> {
        if !self.active || param_idx >= self.num_params {
            return Ok(vec![0.0; self.dim]);
        }
        let fisher = self.fisher_storage.read_features(param_idx)?;
        let anchor = self.anchor_storage.read_features(param_idx)?;
        let w = &current_weights[param_idx];
        let grad: Vec<f32> = (0..self.dim)
            .map(|j| (self.lambda as f32) * fisher[j] * (w[j] - anchor[j]))
            .collect();
        Ok(grad)
    }

    /// Whether EWC is active.
    pub fn is_active(&self) -> bool {
        self.active
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_feature_storage_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("features.bin");

        let dim = 8;
        let num_nodes = 10;
        let mut storage = FeatureStorage::create(&path, dim, num_nodes).unwrap();

        // Write features for several nodes
        for nid in 0..num_nodes {
            let features: Vec<f32> = (0..dim).map(|j| (nid * dim + j) as f32).collect();
            storage.write_features(nid, &features).unwrap();
        }
        storage.flush().unwrap();

        // Re-open and read back
        let mut storage2 = FeatureStorage::open(&path).unwrap();
        assert_eq!(storage2.dim(), dim);
        assert_eq!(storage2.num_nodes(), num_nodes);

        for nid in 0..num_nodes {
            let features = storage2.read_features(nid).unwrap();
            assert_eq!(features.len(), dim);
            for j in 0..dim {
                assert!((features[j] - (nid * dim + j) as f32).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn test_hyperbatch_ordering() {
        // Build a simple chain: 0-1-2-3-4
        let adjacency = vec![(0, 1), (1, 2), (2, 3), (3, 4)];
        let order = HyperbatchIterator::reorder_bfs(&adjacency, 5);

        // BFS from 0 should visit 0, 1, 2, 3, 4 in order
        assert_eq!(order, vec![0, 1, 2, 3, 4]);

        // Star graph: 0 connected to 1..4
        let star = vec![(0, 1), (0, 2), (0, 3), (0, 4)];
        let star_order = HyperbatchIterator::reorder_bfs(&star, 5);
        // 0 first, then neighbors (order may vary but 0 must be first)
        assert_eq!(star_order[0], 0);
        assert_eq!(star_order.len(), 5);
    }

    #[test]
    fn test_hotset_eviction() {
        let mut hotset = AdaptiveHotset::new(3, 0.9);

        hotset.insert(0, vec![1.0, 2.0]);
        hotset.insert(1, vec![3.0, 4.0]);
        hotset.insert(2, vec![5.0, 6.0]);

        // Access node 0 and 1 more frequently
        for _ in 0..10 {
            hotset.record_access(0);
            hotset.record_access(1);
        }
        // Node 2 has fewest accesses (only the initial 0)

        // Insert a 4th node -> should evict node 2 (coldest)
        hotset.insert(3, vec![7.0, 8.0]);

        assert_eq!(hotset.len(), 3);
        // Node 2 should be gone
        assert!(hotset.get(2).is_none());
        // Nodes 0, 1, 3 should still be present
        assert!(hotset.get(0).is_some());
        assert!(hotset.get(1).is_some());
        assert!(hotset.get(3).is_some());
    }

    #[test]
    fn test_cold_tier_epoch() {
        let tmp = TempDir::new().unwrap();
        let storage_path = tmp.path().join("train_features.bin");

        let dim = 4;
        let num_nodes = 16;
        let config = HyperbatchConfig {
            batch_size: 4,
            hotset_fraction: 0.25,
            ..Default::default()
        };

        let mut trainer = ColdTierTrainer::new(&storage_path, dim, num_nodes, config).unwrap();

        // Write initial features
        for nid in 0..num_nodes {
            let features = vec![1.0f32; dim];
            trainer.storage.write_features(nid, &features).unwrap();
        }
        trainer.storage.flush().unwrap();

        // Build a simple chain adjacency
        let adjacency: Vec<(usize, usize)> = (0..num_nodes.saturating_sub(1))
            .map(|i| (i, i + 1))
            .collect();

        let result = trainer.train_epoch(&adjacency, 0.1);

        assert_eq!(result.epoch, 1);
        assert!(result.batches > 0);
        // All 16 nodes in batches of 4 = 4 batches
        assert_eq!(result.batches, 4);
        // Loss should be positive (features started at 1.0)
        assert!(result.avg_loss > 0.0);
    }

    #[test]
    fn test_cold_tier_ewc() {
        let tmp = TempDir::new().unwrap();
        let ewc_dir = tmp.path().join("ewc");

        let dim = 4;
        let num_params = 3;
        let lambda = 100.0;

        let mut ewc = ColdTierEwc::new(&ewc_dir, dim, num_params, lambda).unwrap();

        // Compute Fisher from gradients (1 sample, 3 param rows)
        let gradients = vec![
            vec![1.0, 2.0, 3.0, 4.0],
            vec![0.5, 0.5, 0.5, 0.5],
            vec![2.0, 1.0, 0.0, 1.0],
        ];
        ewc.compute_fisher(&gradients, 1).unwrap();

        // Verify Fisher was stored correctly
        let fisher0 = ewc.fisher_storage.read_features(0).unwrap();
        assert!((fisher0[0] - 1.0).abs() < 1e-6); // 1^2 / 1
        assert!((fisher0[1] - 4.0).abs() < 1e-6); // 2^2 / 1

        // Consolidate
        let weights = vec![
            vec![0.0, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 0.0],
        ];
        ewc.consolidate(&weights).unwrap();
        assert!(ewc.is_active());

        // Penalty should be 0 at anchor
        let penalty = ewc.penalty(&weights).unwrap();
        assert!(penalty.abs() < 1e-6);

        // Deviation should produce a penalty
        let deviated = vec![
            vec![1.0, 1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0, 1.0],
        ];
        let penalty = ewc.penalty(&deviated).unwrap();
        assert!(penalty > 0.0);

        // Gradient for param 0 should be lambda * fisher * diff
        let grad = ewc.gradient(&deviated, 0).unwrap();
        assert!((grad[0] - 100.0 * 1.0 * 1.0).abs() < 1e-4);
        assert!((grad[1] - 100.0 * 4.0 * 1.0).abs() < 1e-4);
    }
}
