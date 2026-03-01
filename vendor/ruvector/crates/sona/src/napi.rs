//! NAPI-RS bindings for Node.js
//! Enable with feature flag: `napi`

#![cfg(feature = "napi")]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use crate::{
    SonaEngine as RustSonaEngine,
    SonaConfig,
    TrajectoryBuilder as RustTrajectoryBuilder,
    LearnedPattern,
    PatternType,
};

/// Node.js SONA Engine wrapper
#[napi]
pub struct SonaEngine {
    inner: RustSonaEngine,
}

#[napi]
impl SonaEngine {
    /// Create a new SONA engine with default configuration
    /// @param hidden_dim - Hidden dimension size (e.g., 256, 512)
    #[napi(constructor)]
    pub fn new(hidden_dim: u32) -> Self {
        Self {
            inner: RustSonaEngine::new(hidden_dim as usize),
        }
    }

    /// Create with custom configuration
    /// @param config - Custom SONA configuration object
    #[napi(factory)]
    pub fn with_config(config: JsSonaConfig) -> Self {
        let rust_config = SonaConfig {
            hidden_dim: config.hidden_dim as usize,
            embedding_dim: config.embedding_dim.unwrap_or(config.hidden_dim) as usize,
            micro_lora_rank: config.micro_lora_rank.unwrap_or(1) as usize,
            base_lora_rank: config.base_lora_rank.unwrap_or(8) as usize,
            micro_lora_lr: config.micro_lora_lr.unwrap_or(0.001) as f32,
            base_lora_lr: config.base_lora_lr.unwrap_or(0.0001) as f32,
            ewc_lambda: config.ewc_lambda.unwrap_or(1000.0) as f32,
            pattern_clusters: config.pattern_clusters.unwrap_or(50) as usize,
            trajectory_capacity: config.trajectory_capacity.unwrap_or(10000) as usize,
            background_interval_ms: config.background_interval_ms.unwrap_or(3600000) as u64,
            quality_threshold: config.quality_threshold.unwrap_or(0.5) as f32,
            enable_simd: config.enable_simd.unwrap_or(true),
        };
        Self {
            inner: RustSonaEngine::with_config(rust_config),
        }
    }

    /// Start a new trajectory recording
    /// @param query_embedding - Query embedding vector (Float64Array)
    /// @returns TrajectoryBuilder for adding steps
    #[napi]
    pub fn begin_trajectory(&self, query_embedding: Vec<f64>) -> TrajectoryBuilder {
        let embedding: Vec<f32> = query_embedding.iter().map(|&x| x as f32).collect();
        let builder = self.inner.begin_trajectory(embedding);
        TrajectoryBuilder { inner: builder }
    }

    /// Complete a trajectory and submit for learning
    /// @param builder - TrajectoryBuilder instance (consumed)
    /// @param quality - Final quality score [0.0, 1.0]
    #[napi]
    pub fn end_trajectory(&self, mut builder: TrajectoryBuilder, quality: f64) {
        let trajectory = builder.inner.build(quality as f32);
        self.inner.submit_trajectory(trajectory);
    }

    /// Apply micro-LoRA transformation to input
    /// @param input - Input vector (Float64Array)
    /// @returns Transformed output vector
    #[napi]
    pub fn apply_micro_lora(&self, input: Vec<f64>) -> Vec<f64> {
        let input_f32: Vec<f32> = input.iter().map(|&x| x as f32).collect();
        let mut output = vec![0.0f32; input_f32.len()];
        self.inner.apply_micro_lora(&input_f32, &mut output);
        output.iter().map(|&x| x as f64).collect()
    }

    /// Apply base-LoRA transformation to layer output
    /// @param layer_idx - Layer index
    /// @param input - Input vector (Float64Array)
    /// @returns Transformed output vector
    #[napi]
    pub fn apply_base_lora(&self, layer_idx: u32, input: Vec<f64>) -> Vec<f64> {
        let input_f32: Vec<f32> = input.iter().map(|&x| x as f32).collect();
        let mut output = vec![0.0f32; input_f32.len()];
        self.inner.apply_base_lora(layer_idx as usize, &input_f32, &mut output);
        output.iter().map(|&x| x as f64).collect()
    }

    /// Run background learning cycle if due
    /// @returns Optional status message if cycle was executed
    #[napi]
    pub fn tick(&self) -> Option<String> {
        self.inner.tick()
    }

    /// Force background learning cycle immediately
    /// @returns Status message with learning results
    #[napi]
    pub fn force_learn(&self) -> String {
        self.inner.force_learn()
    }

    /// Flush instant loop updates
    #[napi]
    pub fn flush(&self) {
        self.inner.flush();
    }

    /// Find similar learned patterns to query
    /// @param query_embedding - Query embedding vector
    /// @param k - Number of patterns to return
    /// @returns Array of learned patterns
    #[napi]
    pub fn find_patterns(&self, query_embedding: Vec<f64>, k: u32) -> Vec<JsLearnedPattern> {
        let query: Vec<f32> = query_embedding.iter().map(|&x| x as f32).collect();
        self.inner.find_patterns(&query, k as usize)
            .into_iter()
            .map(JsLearnedPattern::from)
            .collect()
    }

    /// Get engine statistics as JSON string
    /// @returns Statistics object as JSON string
    #[napi]
    pub fn get_stats(&self) -> String {
        serde_json::to_string(&self.inner.stats()).unwrap_or_else(|e| {
            format!("{{\"error\": \"{}\"}}", e)
        })
    }

    /// Enable or disable the engine
    /// @param enabled - Whether to enable the engine
    #[napi]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.inner.set_enabled(enabled);
    }

    /// Check if engine is enabled
    /// @returns Whether the engine is enabled
    #[napi]
    pub fn is_enabled(&self) -> bool {
        self.inner.is_enabled()
    }
}

/// Trajectory builder for Node.js
#[napi]
pub struct TrajectoryBuilder {
    inner: RustTrajectoryBuilder,
}

#[napi]
impl TrajectoryBuilder {
    /// Add a step to the trajectory
    /// @param activations - Layer activations (Float64Array)
    /// @param attention_weights - Attention weights (Float64Array)
    /// @param reward - Reward signal for this step
    #[napi]
    pub fn add_step(&mut self, activations: Vec<f64>, attention_weights: Vec<f64>, reward: f64) {
        let act: Vec<f32> = activations.iter().map(|&x| x as f32).collect();
        let att: Vec<f32> = attention_weights.iter().map(|&x| x as f32).collect();
        self.inner.add_step(act, att, reward as f32);
    }

    /// Set model route for this trajectory
    /// @param route - Model route identifier
    #[napi]
    pub fn set_route(&mut self, route: String) {
        self.inner.set_model_route(&route);
    }

    /// Add context ID to trajectory
    /// @param context_id - Context identifier
    #[napi]
    pub fn add_context(&mut self, context_id: String) {
        self.inner.add_context(&context_id);
    }
}

/// SONA configuration for Node.js
#[napi(object)]
pub struct JsSonaConfig {
    /// Hidden dimension size
    pub hidden_dim: u32,
    /// Embedding dimension (defaults to hidden_dim)
    pub embedding_dim: Option<u32>,
    /// Micro-LoRA rank (1-2, default: 1)
    pub micro_lora_rank: Option<u32>,
    /// Base LoRA rank (default: 8)
    pub base_lora_rank: Option<u32>,
    /// Micro-LoRA learning rate (default: 0.001)
    pub micro_lora_lr: Option<f64>,
    /// Base LoRA learning rate (default: 0.0001)
    pub base_lora_lr: Option<f64>,
    /// EWC lambda regularization (default: 1000.0)
    pub ewc_lambda: Option<f64>,
    /// Number of pattern clusters (default: 50)
    pub pattern_clusters: Option<u32>,
    /// Trajectory buffer capacity (default: 10000)
    pub trajectory_capacity: Option<u32>,
    /// Background learning interval in ms (default: 3600000 = 1 hour)
    pub background_interval_ms: Option<i64>,
    /// Quality threshold for learning (default: 0.5)
    pub quality_threshold: Option<f64>,
    /// Enable SIMD optimizations (default: true)
    pub enable_simd: Option<bool>,
}

/// Learned pattern for Node.js
#[napi(object)]
pub struct JsLearnedPattern {
    /// Pattern identifier
    pub id: String,
    /// Cluster centroid embedding
    pub centroid: Vec<f64>,
    /// Number of trajectories in cluster
    pub cluster_size: u32,
    /// Total weight of trajectories
    pub total_weight: f64,
    /// Average quality of member trajectories
    pub avg_quality: f64,
    /// Creation timestamp (Unix seconds)
    pub created_at: String,
    /// Last access timestamp (Unix seconds)
    pub last_accessed: String,
    /// Total access count
    pub access_count: u32,
    /// Pattern type
    pub pattern_type: String,
}

impl From<LearnedPattern> for JsLearnedPattern {
    fn from(pattern: LearnedPattern) -> Self {
        Self {
            id: pattern.id.to_string(),
            centroid: pattern.centroid.iter().map(|&x| x as f64).collect(),
            cluster_size: pattern.cluster_size as u32,
            total_weight: pattern.total_weight as f64,
            avg_quality: pattern.avg_quality as f64,
            created_at: pattern.created_at.to_string(),
            last_accessed: pattern.last_accessed.to_string(),
            access_count: pattern.access_count,
            pattern_type: format!("{:?}", pattern.pattern_type),
        }
    }
}

/// Pattern type enumeration
#[napi]
pub enum JsPatternType {
    General,
    Reasoning,
    Factual,
    Creative,
    CodeGen,
    Conversational,
}

impl From<JsPatternType> for PatternType {
    fn from(js_type: JsPatternType) -> Self {
        match js_type {
            JsPatternType::General => PatternType::General,
            JsPatternType::Reasoning => PatternType::Reasoning,
            JsPatternType::Factual => PatternType::Factual,
            JsPatternType::Creative => PatternType::Creative,
            JsPatternType::CodeGen => PatternType::CodeGen,
            JsPatternType::Conversational => PatternType::Conversational,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_napi_engine_creation() {
        let engine = SonaEngine::new(256);
        assert!(engine.is_enabled());
    }

    #[test]
    fn test_napi_trajectory() {
        let engine = SonaEngine::new(64);
        let mut builder = engine.begin_trajectory(vec![0.1; 64]);
        builder.add_step(vec![0.5; 64], vec![0.4; 32], 0.8);
        engine.end_trajectory(&builder, 0.85);
    }
}
