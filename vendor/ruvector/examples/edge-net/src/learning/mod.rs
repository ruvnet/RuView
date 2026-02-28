//! Learning and Attention Module for Edge-Net
//!
//! Integrates RuVector's self-learning intelligence and attention mechanisms
//! for distributed compute optimization. This module enables edge nodes to:
//!
//! - **Learn patterns** from task execution trajectories
//! - **Store knowledge** in a ReasoningBank for retrieval
//! - **Route tasks** using multi-head attention
//! - **Optimize energy** with spike-driven attention (87x more efficient)
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │              Learning Intelligence                   │
//! ├─────────────────────────────────────────────────────┤
//! │  ┌──────────────┐  ┌──────────────┐  ┌───────────┐ │
//! │  │ ReasoningBank│  │  Trajectory  │  │  Pattern  │ │
//! │  │   Storage    │◄─┤   Tracker    │──┤ Extractor │ │
//! │  └──────────────┘  └──────────────┘  └───────────┘ │
//! ├─────────────────────────────────────────────────────┤
//! │  ┌──────────────┐  ┌──────────────┐                │
//! │  │  Multi-Head  │  │ Spike-Driven │                │
//! │  │  Attention   │  │  Attention   │                │
//! │  │ (Task Route) │  │ (87x Energy) │                │
//! │  └──────────────┘  └──────────────┘                │
//! └─────────────────────────────────────────────────────┘
//! ```

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use rustc_hash::FxHashMap;
use std::sync::RwLock;

// ============================================================================
// Learned Patterns
// ============================================================================

/// A learned pattern from task execution
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearnedPattern {
    /// Centroid vector representing the pattern
    pub centroid: Vec<f32>,
    /// Optimal task allocation score
    pub optimal_allocation: f32,
    /// Optimal energy budget for this pattern
    pub optimal_energy: u64,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Number of samples in this pattern
    pub sample_count: usize,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Average success rate
    pub avg_success_rate: Option<f64>,
}

impl LearnedPattern {
    /// Create a new learned pattern
    pub fn new(
        centroid: Vec<f32>,
        optimal_allocation: f32,
        optimal_energy: u64,
        confidence: f64,
        sample_count: usize,
        avg_latency_ms: f64,
        avg_success_rate: Option<f64>,
    ) -> Self {
        Self {
            centroid,
            optimal_allocation,
            optimal_energy,
            confidence,
            sample_count,
            avg_latency_ms,
            avg_success_rate,
        }
    }

    /// Calculate cosine similarity to a query vector
    pub fn similarity(&self, query: &[f32]) -> f64 {
        if query.len() != self.centroid.len() {
            return 0.0;
        }

        let dot: f32 = query.iter().zip(&self.centroid).map(|(a, b)| a * b).sum();
        let norm_q: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_c: f32 = self.centroid.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_q == 0.0 || norm_c == 0.0 {
            return 0.0;
        }

        (dot / (norm_q * norm_c)) as f64
    }
}

// ============================================================================
// Task Trajectory
// ============================================================================

/// A single task execution trajectory
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskTrajectory {
    /// Task feature vector
    pub task_vector: Vec<f32>,
    /// Execution latency in milliseconds
    pub latency_ms: u64,
    /// Energy consumed (rUv)
    pub energy_spent: u64,
    /// Energy earned (rUv)
    pub energy_earned: u64,
    /// Task success flag
    pub success: bool,
    /// Node that executed the task
    pub executor_id: String,
    /// Timestamp (ms since epoch)
    pub timestamp: u64,
}

impl TaskTrajectory {
    /// Create a new task trajectory
    pub fn new(
        task_vector: Vec<f32>,
        latency_ms: u64,
        energy_spent: u64,
        energy_earned: u64,
        success: bool,
        executor_id: String,
    ) -> Self {
        Self {
            task_vector,
            latency_ms,
            energy_spent,
            energy_earned,
            success,
            executor_id,
            timestamp: js_sys::Date::now() as u64,
        }
    }

    /// Calculate efficiency ratio (earned/spent)
    pub fn efficiency(&self) -> f64 {
        if self.energy_spent == 0 {
            return 0.0;
        }
        self.energy_earned as f64 / self.energy_spent as f64
    }
}

// ============================================================================
// Trajectory Tracker
// ============================================================================

/// Ring buffer tracker for task trajectories
#[wasm_bindgen]
pub struct TrajectoryTracker {
    /// Ring buffer of trajectories
    trajectories: RwLock<Vec<TaskTrajectory>>,
    /// Maximum size
    max_size: usize,
    /// Current write position
    write_pos: RwLock<usize>,
}

#[wasm_bindgen]
impl TrajectoryTracker {
    /// Create a new trajectory tracker
    #[wasm_bindgen(constructor)]
    pub fn new(max_size: usize) -> Self {
        Self {
            trajectories: RwLock::new(Vec::with_capacity(max_size)),
            max_size,
            write_pos: RwLock::new(0),
        }
    }

    /// Record a new trajectory
    #[wasm_bindgen]
    pub fn record(&self, trajectory_json: &str) -> bool {
        let trajectory: TaskTrajectory = match serde_json::from_str(trajectory_json) {
            Ok(t) => t,
            Err(_) => return false,
        };

        let mut trajectories = self.trajectories.write().unwrap();
        let mut pos = self.write_pos.write().unwrap();

        if trajectories.len() < self.max_size {
            trajectories.push(trajectory);
        } else {
            trajectories[*pos] = trajectory;
        }

        *pos = (*pos + 1) % self.max_size;
        true
    }

    /// Get statistics as JSON
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> String {
        let trajectories = self.trajectories.read().unwrap();

        if trajectories.is_empty() {
            return r#"{"total":0}"#.to_string();
        }

        let total = trajectories.len();
        let successful = trajectories.iter().filter(|t| t.success).count();
        let avg_latency = trajectories.iter().map(|t| t.latency_ms).sum::<u64>() as f64 / total as f64;
        let avg_efficiency = trajectories.iter().map(|t| t.efficiency()).sum::<f64>() / total as f64;

        format!(
            r#"{{"total":{},"successful":{},"success_rate":{:.4},"avg_latency_ms":{:.2},"avg_efficiency":{:.4}}}"#,
            total,
            successful,
            successful as f64 / total as f64,
            avg_latency,
            avg_efficiency
        )
    }

    /// Get count of trajectories
    #[wasm_bindgen]
    pub fn count(&self) -> usize {
        self.trajectories.read().unwrap().len()
    }
}

// ============================================================================
// Reasoning Bank
// ============================================================================

/// Pattern entry with usage tracking
#[derive(Clone)]
struct PatternEntry {
    pattern: LearnedPattern,
    usage_count: usize,
    last_used: u64,
}

/// Spatial bucket for fast approximate nearest neighbor search
struct SpatialBucket {
    pattern_ids: Vec<usize>,
}

/// ReasoningBank for storing and retrieving learned patterns
/// Optimized with spatial indexing for O(1) approximate lookups
#[wasm_bindgen]
pub struct ReasoningBank {
    /// Stored patterns indexed by ID
    patterns: RwLock<FxHashMap<usize, PatternEntry>>,
    /// Next pattern ID
    next_id: RwLock<usize>,
    /// Spatial index for fast approximate nearest neighbor
    /// Maps quantized vector hash to pattern IDs
    spatial_index: RwLock<FxHashMap<u64, SpatialBucket>>,
}

#[wasm_bindgen]
impl ReasoningBank {
    /// Create a new ReasoningBank
    #[wasm_bindgen(constructor)]
    pub fn new() -> ReasoningBank {
        ReasoningBank {
            patterns: RwLock::new(FxHashMap::default()),
            next_id: RwLock::new(0),
            spatial_index: RwLock::new(FxHashMap::default()),
        }
    }

    /// Hash a vector into a spatial bucket (locality-sensitive hashing)
    fn spatial_hash(vector: &[f32]) -> u64 {
        // Simple grid-based quantization for fast approximate matching
        // Quantize each dimension to 8 levels (3 bits)
        let mut hash = 0u64;
        for (i, &val) in vector.iter().take(20).enumerate() {
            // Normalize to [0, 7] range
            let quantized = ((val + 1.0) * 3.5).clamp(0.0, 7.0) as u64;
            hash |= quantized << (i * 3);
        }
        hash
    }

    /// Store a new pattern (JSON format)
    #[wasm_bindgen]
    pub fn store(&self, pattern_json: &str) -> i32 {
        let pattern: LearnedPattern = match serde_json::from_str(pattern_json) {
            Ok(p) => p,
            Err(_) => return -1,
        };

        // Compute spatial hash for indexing
        let hash = Self::spatial_hash(&pattern.centroid);

        let mut next_id = self.next_id.write().unwrap();
        let id = *next_id;
        *next_id += 1;

        let entry = PatternEntry {
            pattern,
            usage_count: 0,
            last_used: js_sys::Date::now() as u64,
        };

        self.patterns.write().unwrap().insert(id, entry);

        // Add to spatial index
        let mut index = self.spatial_index.write().unwrap();
        index.entry(hash)
            .or_insert_with(|| SpatialBucket { pattern_ids: Vec::with_capacity(10) })
            .pattern_ids.push(id);

        id as i32
    }

    /// Lookup most similar patterns (OPTIMIZED with spatial indexing)
    #[wasm_bindgen]
    pub fn lookup(&self, query_json: &str, k: usize) -> String {
        let query: Vec<f32> = match serde_json::from_str(query_json) {
            Ok(q) => q,
            Err(_) => return "[]".to_string(),
        };

        let query_hash = Self::spatial_hash(&query);
        let now = js_sys::Date::now() as u64;

        // Step 1: Fast approximate search using spatial index
        let index = self.spatial_index.read().unwrap();
        let mut candidate_ids = Vec::with_capacity(k * 3);  // Pre-allocate

        // Get patterns from same bucket
        if let Some(bucket) = index.get(&query_hash) {
            candidate_ids.extend_from_slice(&bucket.pattern_ids);
        }

        // Check neighboring buckets (increase recall)
        // Flip 1-2 bits in hash to find nearby buckets
        for bit_flip in 0..6 {
            let neighbor_hash = query_hash ^ (1u64 << (bit_flip * 3));
            if let Some(bucket) = index.get(&neighbor_hash) {
                candidate_ids.extend_from_slice(&bucket.pattern_ids);
            }
        }

        // Fallback: if too few candidates, scan more buckets
        if candidate_ids.len() < k * 2 {
            for bucket in index.values().take(10) {
                candidate_ids.extend_from_slice(&bucket.pattern_ids);
                if candidate_ids.len() >= k * 3 {
                    break;
                }
            }
        }

        // Step 2: Exact similarity computation only for candidates
        let mut patterns = self.patterns.write().unwrap();
        let mut similarities = Vec::with_capacity(candidate_ids.len());

        for &id in &candidate_ids {
            if let Some(entry) = patterns.get_mut(&id) {
                let similarity = entry.pattern.similarity(&query);
                entry.usage_count += 1;
                entry.last_used = now;
                similarities.push((id, entry.pattern.clone(), similarity));
            }
        }

        // Sort by weighted score (similarity * confidence)
        similarities.sort_unstable_by(|a, b| {
            let score_a = a.2 * a.1.confidence;
            let score_b = b.2 * b.1.confidence;
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        similarities.truncate(k);

        // Pre-allocate string with estimated capacity
        let mut result = String::with_capacity(k * 120);
        result.push('[');

        for (i, (id, pattern, sim)) in similarities.iter().enumerate() {
            if i > 0 {
                result.push(',');
            }
            use std::fmt::Write;
            let _ = write!(
                result,
                r#"{{"id":{},"similarity":{:.4},"confidence":{:.4},"optimal_allocation":{:.4},"optimal_energy":{}}}"#,
                id, sim, pattern.confidence, pattern.optimal_allocation, pattern.optimal_energy
            );
        }

        result.push(']');
        result
    }

    /// Prune low-quality patterns
    #[wasm_bindgen]
    pub fn prune(&self, min_usage: usize, min_confidence: f64) -> usize {
        let mut patterns = self.patterns.write().unwrap();
        let before = patterns.len();

        patterns.retain(|_, entry| {
            entry.usage_count >= min_usage && entry.pattern.confidence >= min_confidence
        });

        before - patterns.len()
    }

    /// Get total pattern count
    #[wasm_bindgen]
    pub fn count(&self) -> usize {
        self.patterns.read().unwrap().len()
    }

    /// Get bank statistics
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> String {
        let patterns = self.patterns.read().unwrap();

        if patterns.is_empty() {
            return r#"{"total":0}"#.to_string();
        }

        let total = patterns.len();
        let total_samples: usize = patterns.values().map(|e| e.pattern.sample_count).sum();
        let avg_confidence: f64 = patterns.values().map(|e| e.pattern.confidence).sum::<f64>() / total as f64;
        let total_usage: usize = patterns.values().map(|e| e.usage_count).sum();

        format!(
            r#"{{"total_patterns":{},"total_samples":{},"avg_confidence":{:.4},"total_usage":{}}}"#,
            total, total_samples, avg_confidence, total_usage
        )
    }
}

impl Default for ReasoningBank {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Spike Train for Energy-Efficient Attention
// ============================================================================

/// Spike train representation for temporal coding
#[derive(Clone, Debug, Default)]
pub struct SpikeTrain {
    /// Spike times within temporal window
    pub times: Vec<u8>,
    /// Spike polarities: +1 for positive, -1 for negative
    pub polarities: Vec<i8>,
}

impl SpikeTrain {
    /// Create empty spike train
    pub fn new() -> Self {
        Self {
            times: Vec::new(),
            polarities: Vec::new(),
        }
    }

    /// Create spike train with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            times: Vec::with_capacity(capacity),
            polarities: Vec::with_capacity(capacity),
        }
    }

    /// Add a spike at given time with polarity
    pub fn add_spike(&mut self, time: u8, polarity: i8) {
        self.times.push(time);
        self.polarities.push(polarity);
    }

    /// Number of spikes
    pub fn len(&self) -> usize {
        self.times.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }
}

// ============================================================================
// Spike-Driven Attention
// ============================================================================

/// Configuration for spike-driven attention
#[derive(Clone, Debug)]
pub struct SpikeDrivenConfig {
    /// Spike threshold in Q15 fixed-point
    pub spike_threshold_q15: u16,
    /// Number of temporal coding steps
    pub temporal_coding_steps: u8,
    /// Use binary quantization
    pub binary_qkv: bool,
    /// Refractory period after spike
    pub refractory_period: u8,
}

impl Default for SpikeDrivenConfig {
    fn default() -> Self {
        Self {
            spike_threshold_q15: 16384, // 0.5 in Q15
            temporal_coding_steps: 8,
            binary_qkv: true,
            refractory_period: 2,
        }
    }
}

/// Spike-driven attention for energy-efficient compute (87x savings)
#[wasm_bindgen]
pub struct SpikeDrivenAttention {
    config: SpikeDrivenConfig,
}

#[wasm_bindgen]
impl SpikeDrivenAttention {
    /// Create new spike-driven attention with default config
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            config: SpikeDrivenConfig::default(),
        }
    }

    /// Create with custom parameters
    #[wasm_bindgen(js_name = withConfig)]
    pub fn with_config(threshold: u16, steps: u8, refractory: u8) -> Self {
        Self {
            config: SpikeDrivenConfig {
                spike_threshold_q15: threshold,
                temporal_coding_steps: steps,
                binary_qkv: true,
                refractory_period: refractory,
            },
        }
    }

    /// Estimate energy savings ratio compared to standard attention
    #[wasm_bindgen(js_name = energyRatio)]
    pub fn energy_ratio(&self, seq_len: usize, hidden_dim: usize) -> f32 {
        if seq_len == 0 || hidden_dim == 0 {
            return 1.0;
        }

        // Standard attention operations (multiplications)
        let standard_mults = 2 * seq_len * seq_len * hidden_dim;

        // Spike-driven operations (additions only)
        let avg_spikes_per_neuron = (self.config.temporal_coding_steps as f32) * 0.3;
        let spike_adds = (seq_len as f32) * avg_spikes_per_neuron * (hidden_dim as f32);

        // Energy ratio (multiplication ~3.7x more expensive than addition)
        let mult_energy_factor = 3.7;

        let standard_energy = (standard_mults as f32) * mult_energy_factor;
        let spike_energy = spike_adds;

        if spike_energy == 0.0 {
            return 1.0;
        }

        standard_energy / spike_energy
    }
}

impl Default for SpikeDrivenAttention {
    fn default() -> Self {
        Self::new()
    }
}

impl SpikeDrivenAttention {
    /// Encode values to spike trains using rate coding (OPTIMIZED with pre-allocation)
    pub fn encode_spikes(&self, values: &[i8]) -> Vec<SpikeTrain> {
        let steps = self.config.temporal_coding_steps as usize;
        let mut trains = Vec::with_capacity(values.len());

        for &value in values {
            // Pre-allocate spike train capacity (max possible spikes)
            let mut train = SpikeTrain::with_capacity(steps);

            let abs_val = if value == i8::MIN { 128u16 } else { value.abs() as u16 };
            let polarity = value.signum();

            if abs_val == 0 {
                trains.push(train);
                continue;
            }

            // Rate coding: spike frequency proportional to magnitude
            let rate_q15 = ((abs_val as u32) * 32768 / 128) as u16;

            let mut refractory_counter = 0u8;
            let mut membrane_potential = 0u32;

            for step in 0..steps {
                if refractory_counter > 0 {
                    refractory_counter -= 1;
                    continue;
                }

                membrane_potential = membrane_potential.saturating_add(rate_q15 as u32);

                if membrane_potential >= self.config.spike_threshold_q15 as u32 {
                    train.add_spike(step as u8, polarity);
                    membrane_potential = 0;
                    refractory_counter = self.config.refractory_period;
                }
            }

            trains.push(train);
        }

        trains
    }

    /// Compute spike-driven attention (no multiplications)
    pub fn attention(
        &self,
        q_spikes: &[SpikeTrain],
        k_spikes: &[SpikeTrain],
        v_spikes: &[SpikeTrain],
    ) -> Vec<i32> {
        let seq_len = q_spikes.len().min(k_spikes.len());
        let hidden_dim = v_spikes.len();
        let mut output = vec![0i32; hidden_dim];

        if seq_len == 0 || hidden_dim == 0 {
            return output;
        }

        for q_idx in 0..seq_len {
            let q_train = &q_spikes[q_idx];

            // Compute attention weights via spike coincidence
            for k_idx in 0..=q_idx.min(seq_len - 1) {
                let k_train = &k_spikes[k_idx];

                let mut coincidence_score = 0i32;
                for (&q_time, &q_pol) in q_train.times.iter().zip(q_train.polarities.iter()) {
                    for (&k_time, &k_pol) in k_train.times.iter().zip(k_train.polarities.iter()) {
                        if q_time == k_time {
                            coincidence_score += (q_pol as i32) * (k_pol as i32);
                        }
                    }
                }

                if coincidence_score != 0 {
                    for (d, v_train) in v_spikes.iter().enumerate().take(hidden_dim) {
                        let value_contrib: i32 = v_train.polarities.iter()
                            .map(|&p| (p as i32).saturating_mul(coincidence_score))
                            .sum();
                        output[d] += value_contrib;
                    }
                }
            }
        }

        output
    }
}

// ============================================================================
// Multi-Head Attention for Task Routing
// ============================================================================

/// Multi-head attention for distributed task routing
#[wasm_bindgen]
pub struct MultiHeadAttention {
    dim: usize,
    num_heads: usize,
    head_dim: usize,
}

#[wasm_bindgen]
impl MultiHeadAttention {
    /// Create new multi-head attention
    #[wasm_bindgen(constructor)]
    pub fn new(dim: usize, num_heads: usize) -> Self {
        let head_dim = dim / num_heads;
        Self { dim, num_heads, head_dim }
    }

    /// Get embedding dimension
    #[wasm_bindgen]
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Get number of heads
    #[wasm_bindgen(js_name = numHeads)]
    pub fn num_heads(&self) -> usize {
        self.num_heads
    }
}

impl MultiHeadAttention {
    /// Split input into multiple heads
    fn split_heads(&self, input: &[f32]) -> Vec<Vec<f32>> {
        (0..self.num_heads)
            .map(|h| {
                let start = h * self.head_dim;
                let end = start + self.head_dim;
                input[start..end].to_vec()
            })
            .collect()
    }

    /// Compute scaled dot-product attention for a single head
    fn scaled_dot_product(&self, query: &[f32], keys: &[&[f32]], values: &[&[f32]]) -> Vec<f32> {
        let scale = (self.head_dim as f32).sqrt();

        // Compute attention scores
        let scores: Vec<f32> = keys.iter()
            .map(|k| {
                let dot: f32 = query.iter().zip(*k).map(|(q, k)| q * k).sum();
                dot / scale
            })
            .collect();

        // Softmax
        let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_scores: Vec<f32> = scores.iter().map(|s| (s - max_score).exp()).collect();
        let sum_exp: f32 = exp_scores.iter().sum();
        let attention_weights: Vec<f32> = exp_scores.iter().map(|e| e / sum_exp).collect();

        // Weighted sum of values
        let mut output = vec![0.0f32; self.head_dim];
        for (weight, value) in attention_weights.iter().zip(values.iter()) {
            for (o, v) in output.iter_mut().zip(value.iter()) {
                *o += weight * v;
            }
        }

        output
    }

    /// Compute multi-head attention
    pub fn compute(&self, query: &[f32], keys: &[&[f32]], values: &[&[f32]]) -> Vec<f32> {
        if query.len() != self.dim {
            return vec![0.0; self.dim];
        }

        // Split query into heads
        let query_heads = self.split_heads(query);

        // Split keys and values
        let key_heads: Vec<Vec<Vec<f32>>> = keys.iter().map(|k| self.split_heads(k)).collect();
        let value_heads: Vec<Vec<Vec<f32>>> = values.iter().map(|v| self.split_heads(v)).collect();

        // Compute attention for each head
        let mut head_outputs = Vec::new();
        for h in 0..self.num_heads {
            let head_keys: Vec<&[f32]> = key_heads.iter().map(|kh| kh[h].as_slice()).collect();
            let head_values: Vec<&[f32]> = value_heads.iter().map(|vh| vh[h].as_slice()).collect();
            let head_out = self.scaled_dot_product(&query_heads[h], &head_keys, &head_values);
            head_outputs.push(head_out);
        }

        // Concatenate head outputs
        head_outputs.into_iter().flatten().collect()
    }
}

// ============================================================================
// Network Learning Intelligence
// ============================================================================

/// Unified learning intelligence for edge-net nodes
#[wasm_bindgen]
pub struct NetworkLearning {
    /// Pattern storage
    reasoning_bank: ReasoningBank,
    /// Trajectory tracking
    trajectory_tracker: TrajectoryTracker,
    /// Spike-driven attention for energy efficiency
    spike_attention: SpikeDrivenAttention,
    /// Multi-head attention for task routing
    multi_head: MultiHeadAttention,
    /// Learning rate for online updates
    learning_rate: f32,
}

#[wasm_bindgen]
impl NetworkLearning {
    /// Create new network learning intelligence
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            reasoning_bank: ReasoningBank::new(),
            trajectory_tracker: TrajectoryTracker::new(1000),
            spike_attention: SpikeDrivenAttention::new(),
            multi_head: MultiHeadAttention::new(64, 4), // 64-dim, 4 heads
            learning_rate: 0.01,
        }
    }

    /// Record a task execution trajectory
    #[wasm_bindgen(js_name = recordTrajectory)]
    pub fn record_trajectory(&self, trajectory_json: &str) -> bool {
        self.trajectory_tracker.record(trajectory_json)
    }

    /// Store a learned pattern
    #[wasm_bindgen(js_name = storePattern)]
    pub fn store_pattern(&self, pattern_json: &str) -> i32 {
        self.reasoning_bank.store(pattern_json)
    }

    /// Look up similar patterns
    #[wasm_bindgen(js_name = lookupPatterns)]
    pub fn lookup_patterns(&self, query_json: &str, k: usize) -> String {
        self.reasoning_bank.lookup(query_json, k)
    }

    /// Get energy savings ratio for spike-driven attention
    #[wasm_bindgen(js_name = getEnergyRatio)]
    pub fn get_energy_ratio(&self, seq_len: usize, hidden_dim: usize) -> f32 {
        self.spike_attention.energy_ratio(seq_len, hidden_dim)
    }

    /// Get combined statistics
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> String {
        let bank_stats = self.reasoning_bank.get_stats();
        let traj_stats = self.trajectory_tracker.get_stats();
        let energy_ratio = self.spike_attention.energy_ratio(64, 256);

        format!(
            r#"{{"reasoning_bank":{},"trajectories":{},"spike_energy_ratio":{:.2},"learning_rate":{}}}"#,
            bank_stats, traj_stats, energy_ratio, self.learning_rate
        )
    }

    /// Prune low-quality patterns
    #[wasm_bindgen]
    pub fn prune(&self, min_usage: usize, min_confidence: f64) -> usize {
        self.reasoning_bank.prune(min_usage, min_confidence)
    }

    /// Get trajectory count
    #[wasm_bindgen(js_name = trajectoryCount)]
    pub fn trajectory_count(&self) -> usize {
        self.trajectory_tracker.count()
    }

    /// Get pattern count
    #[wasm_bindgen(js_name = patternCount)]
    pub fn pattern_count(&self) -> usize {
        self.reasoning_bank.count()
    }
}

impl Default for NetworkLearning {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learned_pattern_similarity() {
        let pattern = LearnedPattern::new(
            vec![1.0, 0.0, 0.0],
            0.8,
            100,
            0.9,
            10,
            50.0,
            Some(0.95),
        );

        let query_same = vec![1.0, 0.0, 0.0];
        let query_perp = vec![0.0, 1.0, 0.0];

        assert!((pattern.similarity(&query_same) - 1.0).abs() < 0.001);
        assert!((pattern.similarity(&query_perp) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_task_trajectory_efficiency() {
        let traj = TaskTrajectory {
            task_vector: vec![1.0, 2.0],
            latency_ms: 100,
            energy_spent: 50,
            energy_earned: 100,
            success: true,
            executor_id: "node-1".to_string(),
            timestamp: 0,
        };

        assert!((traj.efficiency() - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_spike_train() {
        let mut train = SpikeTrain::new();
        assert!(train.is_empty());

        train.add_spike(0, 1);
        train.add_spike(3, -1);

        assert_eq!(train.len(), 2);
        assert_eq!(train.times, vec![0, 3]);
        assert_eq!(train.polarities, vec![1, -1]);
    }

    #[test]
    fn test_spike_encoding() {
        let attn = SpikeDrivenAttention::new();
        let values = vec![64i8, 0, -64];
        let trains = attn.encode_spikes(&values);

        assert_eq!(trains.len(), 3);
        assert!(trains[0].len() > 0); // High positive
        assert!(trains[1].is_empty()); // Zero
        assert!(trains[2].len() > 0); // High negative
        assert!(trains[2].polarities.iter().all(|&p| p == -1));
    }

    #[test]
    fn test_multi_head_attention() {
        let attn = MultiHeadAttention::new(8, 2);
        let query = vec![1.0_f32; 8];
        let key1 = vec![0.5_f32; 8];
        let val1 = vec![1.0_f32; 8];
        let keys: Vec<&[f32]> = vec![key1.as_slice()];
        let values: Vec<&[f32]> = vec![val1.as_slice()];

        let result = attn.compute(&query, &keys, &values);
        assert_eq!(result.len(), 8);
    }

    #[test]
    fn test_energy_ratio() {
        let attn = SpikeDrivenAttention::new();
        let ratio = attn.energy_ratio(64, 256);

        // Should show significant energy savings
        assert!(ratio > 10.0);
        assert!(ratio < 200.0);
    }
}
