//! SONA Core Types for Edge-Net
//!
//! Adapted from ruvLLM SONA for P2P distributed compute networks.
//! Optimized for WASM and edge device deployment.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Learning signal generated from task execution trajectory
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearningSignal {
    /// Query/task embedding vector
    pub query_embedding: Vec<f32>,
    /// Estimated gradient direction
    pub gradient_estimate: Vec<f32>,
    /// Quality score [0.0, 1.0]
    pub quality_score: f32,
    /// Signal generation timestamp (Unix ms)
    pub timestamp_ms: u64,
    /// Additional metadata
    pub metadata: SignalMetadata,
}

/// Metadata for learning signals
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SignalMetadata {
    /// Source trajectory ID
    pub trajectory_id: u64,
    /// Number of steps in trajectory
    pub step_count: usize,
    /// Node ID that generated this signal
    pub node_id: Option<String>,
    /// Task type for routing
    pub task_type: Option<String>,
    /// Custom tags for P2P sharing
    pub tags: HashMap<String, String>,
}

impl LearningSignal {
    /// Create signal from query trajectory using REINFORCE gradient estimation
    pub fn from_trajectory(trajectory: &QueryTrajectory) -> Self {
        let gradient = Self::estimate_gradient(trajectory);

        Self {
            query_embedding: trajectory.query_embedding.clone(),
            gradient_estimate: gradient,
            quality_score: trajectory.final_quality,
            timestamp_ms: js_sys::Date::now() as u64,
            metadata: SignalMetadata {
                trajectory_id: trajectory.id,
                step_count: trajectory.steps.len(),
                node_id: trajectory.node_id.clone(),
                task_type: trajectory.task_type.clone(),
                tags: HashMap::new(),
            },
        }
    }

    /// Create signal with pre-computed gradient
    pub fn with_gradient(embedding: Vec<f32>, gradient: Vec<f32>, quality: f32) -> Self {
        Self {
            query_embedding: embedding,
            gradient_estimate: gradient,
            quality_score: quality,
            timestamp_ms: js_sys::Date::now() as u64,
            metadata: SignalMetadata::default(),
        }
    }

    /// Estimate gradient using REINFORCE with baseline
    fn estimate_gradient(trajectory: &QueryTrajectory) -> Vec<f32> {
        if trajectory.steps.is_empty() {
            return trajectory.query_embedding.clone();
        }

        let dim = trajectory.query_embedding.len();
        let mut gradient = vec![0.0f32; dim];

        // Compute baseline (average reward)
        let baseline =
            trajectory.steps.iter().map(|s| s.reward).sum::<f32>() / trajectory.steps.len() as f32;

        // REINFORCE: gradient = sum((reward - baseline) * activation)
        for step in &trajectory.steps {
            let advantage = step.reward - baseline;
            let activation_len = step.activations.len().min(dim);
            for i in 0..activation_len {
                gradient[i] += advantage * step.activations[i];
            }
        }

        // L2 normalize
        let norm: f32 = gradient.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            gradient.iter_mut().for_each(|x| *x /= norm);
        }

        gradient
    }

    /// Scale gradient by quality
    pub fn scaled_gradient(&self) -> Vec<f32> {
        self.gradient_estimate
            .iter()
            .map(|&g| g * self.quality_score)
            .collect()
    }
}

/// Query/task trajectory recording for P2P learning
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryTrajectory {
    /// Unique trajectory identifier
    pub id: u64,
    /// Query/task embedding vector
    pub query_embedding: Vec<f32>,
    /// Execution steps
    pub steps: Vec<TrajectoryStep>,
    /// Final quality score [0.0, 1.0]
    pub final_quality: f32,
    /// Total latency in microseconds
    pub latency_us: u64,
    /// Node ID that executed this trajectory
    pub node_id: Option<String>,
    /// Task type for routing optimization
    pub task_type: Option<String>,
    /// P2P context IDs (RAC events, etc.)
    pub context_ids: Vec<String>,
}

impl QueryTrajectory {
    /// Create new trajectory
    pub fn new(id: u64, query_embedding: Vec<f32>) -> Self {
        Self {
            id,
            query_embedding,
            steps: Vec::with_capacity(16),
            final_quality: 0.0,
            latency_us: 0,
            node_id: None,
            task_type: None,
            context_ids: Vec::new(),
        }
    }

    /// Create trajectory with node context
    pub fn with_node(id: u64, query_embedding: Vec<f32>, node_id: &str) -> Self {
        let mut t = Self::new(id, query_embedding);
        t.node_id = Some(node_id.to_string());
        t
    }

    /// Add execution step
    pub fn add_step(&mut self, step: TrajectoryStep) {
        self.steps.push(step);
    }

    /// Finalize trajectory with quality score
    pub fn finalize(&mut self, quality: f32, latency_us: u64) {
        self.final_quality = quality;
        self.latency_us = latency_us;
    }

    /// Get total reward
    pub fn total_reward(&self) -> f32 {
        self.steps.iter().map(|s| s.reward).sum()
    }

    /// Get average reward
    pub fn avg_reward(&self) -> f32 {
        if self.steps.is_empty() {
            0.0
        } else {
            self.total_reward() / self.steps.len() as f32
        }
    }

    /// Set task type for routing optimization
    pub fn set_task_type(&mut self, task_type: &str) {
        self.task_type = Some(task_type.to_string());
    }
}

/// Single step in a trajectory
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrajectoryStep {
    /// Layer/module activations (subset for efficiency)
    pub activations: Vec<f32>,
    /// Attention weights (flattened)
    pub attention_weights: Vec<f32>,
    /// Reward signal for this step
    pub reward: f32,
    /// Step index
    pub step_idx: usize,
    /// Optional layer name
    pub layer_name: Option<String>,
}

impl TrajectoryStep {
    /// Create new step
    pub fn new(
        activations: Vec<f32>,
        attention_weights: Vec<f32>,
        reward: f32,
        step_idx: usize,
    ) -> Self {
        Self {
            activations,
            attention_weights,
            reward,
            step_idx,
            layer_name: None,
        }
    }

    /// Create step with layer name
    pub fn with_layer(mut self, name: &str) -> Self {
        self.layer_name = Some(name.to_string());
        self
    }
}

/// Learned pattern from trajectory clustering
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearnedPattern {
    /// Pattern identifier
    pub id: u64,
    /// Cluster centroid embedding
    pub centroid: Vec<f32>,
    /// Number of trajectories in cluster
    pub cluster_size: usize,
    /// Sum of trajectory weights
    pub total_weight: f32,
    /// Average quality of member trajectories
    pub avg_quality: f32,
    /// Creation timestamp (Unix seconds)
    pub created_at: u64,
    /// Last access timestamp
    pub last_accessed: u64,
    /// Total access count
    pub access_count: u32,
    /// Pattern type/category
    pub pattern_type: PatternType,
}

/// Pattern classification
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum PatternType {
    #[default]
    General,
    Compute,
    Embedding,
    Inference,
    Verification,
    P2PRouting,
}

impl LearnedPattern {
    /// Create new pattern
    pub fn new(id: u64, centroid: Vec<f32>) -> Self {
        let now = (js_sys::Date::now() / 1000.0) as u64;

        Self {
            id,
            centroid,
            cluster_size: 1,
            total_weight: 1.0,
            avg_quality: 0.0,
            created_at: now,
            last_accessed: now,
            access_count: 0,
            pattern_type: PatternType::default(),
        }
    }

    /// Merge two patterns
    pub fn merge(&self, other: &Self) -> Self {
        let total_size = self.cluster_size + other.cluster_size;
        let w1 = self.cluster_size as f32 / total_size as f32;
        let w2 = other.cluster_size as f32 / total_size as f32;

        let centroid: Vec<f32> = self
            .centroid
            .iter()
            .zip(&other.centroid)
            .map(|(&a, &b)| a * w1 + b * w2)
            .collect();

        Self {
            id: self.id,
            centroid,
            cluster_size: total_size,
            total_weight: self.total_weight + other.total_weight,
            avg_quality: self.avg_quality * w1 + other.avg_quality * w2,
            created_at: self.created_at.min(other.created_at),
            last_accessed: self.last_accessed.max(other.last_accessed),
            access_count: self.access_count + other.access_count,
            pattern_type: self.pattern_type.clone(),
        }
    }

    /// Decay pattern importance
    pub fn decay(&mut self, factor: f32) {
        self.total_weight *= factor;
    }

    /// Record access
    pub fn touch(&mut self) {
        self.access_count += 1;
        self.last_accessed = (js_sys::Date::now() / 1000.0) as u64;
    }

    /// Check if pattern should be pruned
    pub fn should_prune(&self, min_quality: f32, min_accesses: u32, max_age_secs: u64) -> bool {
        let now = (js_sys::Date::now() / 1000.0) as u64;
        let age = now.saturating_sub(self.last_accessed);

        self.avg_quality < min_quality && self.access_count < min_accesses && age > max_age_secs
    }

    /// Compute cosine similarity with query
    pub fn similarity(&self, query: &[f32]) -> f32 {
        if self.centroid.len() != query.len() {
            return 0.0;
        }

        let dot: f32 = self.centroid.iter().zip(query).map(|(a, b)| a * b).sum();
        let norm_a: f32 = self.centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a > 1e-8 && norm_b > 1e-8 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }
}

/// SONA configuration for edge-net
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SonaConfig {
    /// Hidden dimension
    pub hidden_dim: usize,
    /// Embedding dimension
    pub embedding_dim: usize,
    /// Micro-LoRA rank (1-2 for edge devices)
    pub micro_lora_rank: usize,
    /// Base LoRA rank
    pub base_lora_rank: usize,
    /// Micro-LoRA learning rate
    pub micro_lora_lr: f32,
    /// Base LoRA learning rate
    pub base_lora_lr: f32,
    /// EWC lambda
    pub ewc_lambda: f32,
    /// Pattern extraction clusters
    pub pattern_clusters: usize,
    /// Trajectory buffer capacity
    pub trajectory_capacity: usize,
    /// Background learning interval (ms)
    pub background_interval_ms: u64,
    /// Deep consolidation interval (ms) - weekly
    pub deep_interval_ms: u64,
    /// Quality threshold for learning
    pub quality_threshold: f32,
    /// Enable SIMD optimizations
    pub enable_simd: bool,
    /// Enable P2P pattern sharing via RAC
    pub enable_p2p_sharing: bool,
}

impl Default for SonaConfig {
    fn default() -> Self {
        // OPTIMIZED DEFAULTS for edge/WASM deployment:
        // - Rank-2 is faster than Rank-1 due to better SIMD vectorization
        // - Smaller buffer for memory-constrained devices
        // - Lower cluster count for faster search
        Self {
            hidden_dim: 128,                      // Smaller for edge devices
            embedding_dim: 128,
            micro_lora_rank: 2,                   // OPTIMIZED: Rank-2 faster than Rank-1
            base_lora_rank: 4,                    // Smaller for memory
            micro_lora_lr: 0.002,                 // OPTIMIZED: +55% quality improvement
            base_lora_lr: 0.0001,
            ewc_lambda: 2000.0,                   // OPTIMIZED: Better forgetting prevention
            pattern_clusters: 50,                 // Smaller for edge
            trajectory_capacity: 500,             // Smaller buffer for edge
            background_interval_ms: 3600000,      // 1 hour
            deep_interval_ms: 604800000,          // 1 week
            quality_threshold: 0.3,               // OPTIMIZED: Lower threshold for more learning
            enable_simd: true,
            enable_p2p_sharing: true,             // Enable RAC pattern sharing
        }
    }
}

impl SonaConfig {
    /// Create config optimized for maximum throughput (real-time P2P)
    pub fn max_throughput() -> Self {
        Self {
            hidden_dim: 128,
            embedding_dim: 128,
            micro_lora_rank: 2,
            base_lora_rank: 4,
            micro_lora_lr: 0.0005,                // Conservative for stability
            base_lora_lr: 0.0001,
            ewc_lambda: 2000.0,
            pattern_clusters: 50,
            trajectory_capacity: 200,
            background_interval_ms: 7200000,      // 2 hours
            deep_interval_ms: 604800000,
            quality_threshold: 0.4,
            enable_simd: true,
            enable_p2p_sharing: true,
        }
    }

    /// Create config optimized for maximum quality
    pub fn max_quality() -> Self {
        Self {
            hidden_dim: 256,
            embedding_dim: 256,
            micro_lora_rank: 2,
            base_lora_rank: 8,
            micro_lora_lr: 0.002,                 // Optimal learning rate
            base_lora_lr: 0.001,                  // Aggressive base learning
            ewc_lambda: 2000.0,
            pattern_clusters: 100,
            trajectory_capacity: 1000,
            background_interval_ms: 1800000,      // 30 minutes
            deep_interval_ms: 259200000,          // 3 days
            quality_threshold: 0.2,               // Learn from more trajectories
            enable_simd: true,
            enable_p2p_sharing: true,
        }
    }

    /// Create config for minimal edge deployment (<5MB memory)
    pub fn edge_minimal() -> Self {
        Self {
            hidden_dim: 64,
            embedding_dim: 64,
            micro_lora_rank: 1,                   // Minimal rank for memory
            base_lora_rank: 2,
            micro_lora_lr: 0.001,
            base_lora_lr: 0.0001,
            ewc_lambda: 1000.0,
            pattern_clusters: 20,
            trajectory_capacity: 100,             // Very small buffer
            background_interval_ms: 3600000,
            deep_interval_ms: 604800000,
            quality_threshold: 0.5,
            enable_simd: true,
            enable_p2p_sharing: true,
        }
    }

    /// Create config for P2P compute nodes
    pub fn p2p_compute() -> Self {
        Self {
            hidden_dim: 128,
            embedding_dim: 128,
            micro_lora_rank: 2,
            base_lora_rank: 4,
            micro_lora_lr: 0.001,
            base_lora_lr: 0.0001,
            ewc_lambda: 2000.0,
            pattern_clusters: 50,
            trajectory_capacity: 500,
            background_interval_ms: 3600000,
            deep_interval_ms: 604800000,
            quality_threshold: 0.3,
            enable_simd: true,
            enable_p2p_sharing: true,             // Enable pattern sharing
        }
    }
}

/// P2P shareable pattern for RAC events
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShareablePattern {
    /// Pattern ID
    pub id: u64,
    /// Centroid (can be quantized for efficiency)
    pub centroid: Vec<f32>,
    /// Quality score
    pub avg_quality: f32,
    /// Cluster size (credibility)
    pub cluster_size: usize,
    /// Origin node ID
    pub origin_node: String,
    /// Signature for verification
    pub signature: Option<Vec<u8>>,
}

impl From<&LearnedPattern> for ShareablePattern {
    fn from(pattern: &LearnedPattern) -> Self {
        Self {
            id: pattern.id,
            centroid: pattern.centroid.clone(),
            avg_quality: pattern.avg_quality,
            cluster_size: pattern.cluster_size,
            origin_node: String::new(),
            signature: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learning_signal_from_trajectory() {
        let mut trajectory = QueryTrajectory::new(1, vec![0.1, 0.2, 0.3]);
        trajectory.add_step(TrajectoryStep::new(
            vec![0.5, 0.3, 0.2],
            vec![0.4, 0.4, 0.2],
            0.8,
            0,
        ));
        trajectory.finalize(0.8, 1000);

        let signal = LearningSignal::from_trajectory(&trajectory);
        assert_eq!(signal.quality_score, 0.8);
        assert_eq!(signal.gradient_estimate.len(), 3);
        assert_eq!(signal.metadata.trajectory_id, 1);
    }

    #[test]
    fn test_pattern_merge() {
        let p1 = LearnedPattern {
            id: 1,
            centroid: vec![1.0, 0.0],
            cluster_size: 10,
            total_weight: 5.0,
            avg_quality: 0.8,
            created_at: 100,
            last_accessed: 200,
            access_count: 5,
            pattern_type: PatternType::General,
        };

        let p2 = LearnedPattern {
            id: 2,
            centroid: vec![0.0, 1.0],
            cluster_size: 10,
            total_weight: 5.0,
            avg_quality: 0.9,
            created_at: 150,
            last_accessed: 250,
            access_count: 3,
            pattern_type: PatternType::General,
        };

        let merged = p1.merge(&p2);
        assert_eq!(merged.cluster_size, 20);
        assert!((merged.centroid[0] - 0.5).abs() < 1e-6);
        assert!((merged.centroid[1] - 0.5).abs() < 1e-6);
        assert!((merged.avg_quality - 0.85).abs() < 1e-6);
    }

    #[test]
    fn test_pattern_similarity() {
        let pattern = LearnedPattern::new(1, vec![1.0, 0.0, 0.0]);

        assert!((pattern.similarity(&[1.0, 0.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(pattern.similarity(&[0.0, 1.0, 0.0]).abs() < 1e-6);
    }

    #[test]
    fn test_trajectory_rewards() {
        let mut trajectory = QueryTrajectory::new(1, vec![0.1]);
        trajectory.add_step(TrajectoryStep::new(vec![], vec![], 0.5, 0));
        trajectory.add_step(TrajectoryStep::new(vec![], vec![], 0.7, 1));
        trajectory.add_step(TrajectoryStep::new(vec![], vec![], 0.9, 2));

        assert!((trajectory.total_reward() - 2.1).abs() < 1e-6);
        assert!((trajectory.avg_reward() - 0.7).abs() < 1e-6);
    }

    #[test]
    fn test_config_profiles() {
        let edge = SonaConfig::edge_minimal();
        assert_eq!(edge.hidden_dim, 64);
        assert_eq!(edge.micro_lora_rank, 1);

        let quality = SonaConfig::max_quality();
        assert_eq!(quality.hidden_dim, 256);
        assert_eq!(quality.base_lora_rank, 8);
    }
}
