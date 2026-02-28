# SONA API Reference

## Overview

This document provides complete API documentation for all SONA public interfaces.

---

## Core Types

### LearningSignal

Learning signal generated from inference trajectory.

```rust
/// Signal for online learning from inference
#[derive(Clone, Debug)]
pub struct LearningSignal {
    /// Query embedding vector
    pub query_embedding: Vec<f32>,

    /// Estimated gradient direction
    pub gradient_estimate: Vec<f32>,

    /// Quality score [0.0, 1.0]
    pub quality_score: f32,

    /// Signal generation timestamp
    pub timestamp: Instant,

    /// Additional metadata
    pub metadata: SignalMetadata,
}

impl LearningSignal {
    /// Create signal from query trajectory
    ///
    /// # Arguments
    /// * `trajectory` - Completed query trajectory
    ///
    /// # Returns
    /// Learning signal with estimated gradients
    ///
    /// # Example
    /// ```rust
    /// let trajectory = builder.build(0.8);
    /// let signal = LearningSignal::from_trajectory(&trajectory);
    /// assert!(signal.quality_score > 0.0);
    /// ```
    pub fn from_trajectory(trajectory: &QueryTrajectory) -> Self;

    /// Create signal with custom gradient
    ///
    /// # Arguments
    /// * `embedding` - Query embedding
    /// * `gradient` - Pre-computed gradient
    /// * `quality` - Quality score
    pub fn with_gradient(
        embedding: Vec<f32>,
        gradient: Vec<f32>,
        quality: f32
    ) -> Self;
}
```

### QueryTrajectory

Recording of inference execution path.

```rust
/// Complete trajectory of a query through the model
#[derive(Clone, Debug)]
pub struct QueryTrajectory {
    /// Unique trajectory identifier
    pub id: u64,

    /// Query embedding vector
    pub query_embedding: Vec<f32>,

    /// Execution steps
    pub steps: Vec<TrajectoryStep>,

    /// Final quality score [0.0, 1.0]
    pub final_quality: f32,

    /// Total latency in microseconds
    pub latency_us: u64,
}

/// Single step in a trajectory
#[derive(Clone, Debug)]
pub struct TrajectoryStep {
    /// Layer activations
    pub activations: Vec<f32>,

    /// Attention weights
    pub attention_weights: Vec<f32>,

    /// Reward signal for this step
    pub reward: f32,

    /// Step timestamp
    pub timestamp: Instant,
}
```

### LearnedPattern

Pattern extracted from trajectory clustering.

```rust
/// Pattern learned from trajectory analysis
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
}

impl LearnedPattern {
    /// Merge two patterns
    ///
    /// Creates a new pattern with weighted average centroid.
    ///
    /// # Arguments
    /// * `other` - Pattern to merge with
    ///
    /// # Returns
    /// New merged pattern
    pub fn merge(&self, other: &Self) -> Self;

    /// Decay pattern importance
    ///
    /// # Arguments
    /// * `factor` - Decay factor [0.0, 1.0]
    pub fn decay(&mut self, factor: f32);

    /// Check if pattern should be pruned
    ///
    /// # Arguments
    /// * `min_quality` - Minimum quality threshold
    /// * `min_accesses` - Minimum access count
    pub fn should_prune(&self, min_quality: f32, min_accesses: u32) -> bool;
}
```

---

## LoRA Module

### MicroLoRA

Ultra-low latency adapter for per-request updates.

```rust
/// Micro-LoRA with rank 1-2 for instant adaptation
pub struct MicroLoRA {
    // Private fields
}

impl MicroLoRA {
    /// Create new Micro-LoRA adapter
    ///
    /// # Arguments
    /// * `hidden_dim` - Model hidden dimension
    /// * `rank` - LoRA rank (must be 1-2)
    ///
    /// # Panics
    /// Panics if rank > 2
    ///
    /// # Example
    /// ```rust
    /// let lora = MicroLoRA::new(256, 1);
    /// assert_eq!(lora.rank(), 1);
    /// ```
    pub fn new(hidden_dim: usize, rank: usize) -> Self;

    /// SIMD-optimized forward pass
    ///
    /// Applies LoRA adaptation: output += scale * (input @ down) @ up
    ///
    /// # Safety
    /// Requires AVX2 CPU support.
    ///
    /// # Arguments
    /// * `input` - Input tensor [hidden_dim]
    /// * `output` - Output tensor [hidden_dim] (modified in place)
    ///
    /// # Example
    /// ```rust
    /// let lora = MicroLoRA::new(256, 1);
    /// let input = vec![0.1f32; 256];
    /// let mut output = vec![0.0f32; 256];
    ///
    /// unsafe { lora.forward_simd(&input, &mut output) };
    /// ```
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    pub unsafe fn forward_simd(&self, input: &[f32], output: &mut [f32]);

    /// Scalar fallback forward pass
    pub fn forward_scalar(&self, input: &[f32], output: &mut [f32]);

    /// Accumulate gradient for batch update
    ///
    /// # Arguments
    /// * `signal` - Learning signal with gradient estimate
    pub fn accumulate_gradient(&mut self, signal: &LearningSignal);

    /// Apply accumulated gradients
    ///
    /// # Arguments
    /// * `learning_rate` - Learning rate for update
    pub fn apply_accumulated(&mut self, learning_rate: f32);

    /// Reset accumulated gradients
    pub fn reset(&mut self);

    /// Get current rank
    pub fn rank(&self) -> usize;

    /// Get hidden dimension
    pub fn hidden_dim(&self) -> usize;

    /// Get total parameter count
    pub fn param_count(&self) -> usize;

    /// Get scale factor
    pub fn scale(&self) -> f32;

    /// Set scale factor
    pub fn set_scale(&mut self, scale: f32);
}
```

### BaseLoRA

Standard LoRA for hourly background updates.

```rust
/// Base LoRA with rank 4-16 for background adaptation
pub struct BaseLoRA {
    // Private fields
}

impl BaseLoRA {
    /// Create new Base LoRA
    ///
    /// # Arguments
    /// * `hidden_dim` - Model hidden dimension
    /// * `rank` - LoRA rank (typically 4-16)
    /// * `num_layers` - Number of model layers
    pub fn new(hidden_dim: usize, rank: usize, num_layers: usize) -> Self;

    /// Forward pass for single layer
    ///
    /// # Arguments
    /// * `layer_idx` - Layer index
    /// * `input` - Input tensor
    /// * `output` - Output tensor (modified in place)
    pub fn forward_layer(&self, layer_idx: usize, input: &[f32], output: &mut [f32]);

    /// Merge LoRA weights into model
    ///
    /// # Arguments
    /// * `model_weights` - Model weight matrix
    /// * `layer_idx` - Layer to merge
    pub fn merge_weights(&self, model_weights: &mut [f32], layer_idx: usize);

    /// Get number of layers
    pub fn num_layers(&self) -> usize;

    /// Get rank
    pub fn rank(&self) -> usize;

    /// Get alpha scaling factor
    pub fn alpha(&self) -> f32;

    /// Set alpha scaling factor
    pub fn set_alpha(&mut self, alpha: f32);

    /// Save to file
    pub fn save(&self, path: &Path) -> Result<(), IoError>;

    /// Load from file
    pub fn load(path: &Path) -> Result<Self, IoError>;
}
```

---

## Trajectory Module

### TrajectoryBuffer

Lock-free buffer for trajectory collection.

```rust
/// Lock-free circular buffer for trajectories
pub struct TrajectoryBuffer {
    // Private fields
}

impl TrajectoryBuffer {
    /// Create new buffer
    ///
    /// # Arguments
    /// * `capacity` - Maximum trajectories to store
    pub fn new(capacity: usize) -> Self;

    /// Record trajectory (non-blocking)
    ///
    /// # Arguments
    /// * `trajectory` - Trajectory to record
    ///
    /// # Returns
    /// `true` if recorded, `false` if buffer full
    pub fn record(&self, trajectory: QueryTrajectory) -> bool;

    /// Drain all trajectories
    ///
    /// # Returns
    /// Vector of all buffered trajectories
    pub fn drain(&self) -> Vec<QueryTrajectory>;

    /// Get current count
    pub fn len(&self) -> usize;

    /// Check if empty
    pub fn is_empty(&self) -> bool;

    /// Get dropped count
    pub fn dropped_count(&self) -> u64;

    /// Get capacity
    pub fn capacity(&self) -> usize;
}
```

### TrajectoryBuilder

Builder pattern for constructing trajectories.

```rust
/// Builder for constructing trajectories during inference
pub struct TrajectoryBuilder {
    // Private fields
}

impl TrajectoryBuilder {
    /// Start new trajectory
    ///
    /// # Arguments
    /// * `id` - Unique trajectory ID
    /// * `query_embedding` - Query embedding vector
    pub fn new(id: u64, query_embedding: Vec<f32>) -> Self;

    /// Add execution step
    ///
    /// # Arguments
    /// * `activations` - Layer activations
    /// * `attention_weights` - Attention weights
    /// * `reward` - Step reward
    pub fn add_step(
        &mut self,
        activations: Vec<f32>,
        attention_weights: Vec<f32>,
        reward: f32
    );

    /// Finalize trajectory
    ///
    /// # Arguments
    /// * `final_quality` - Overall quality score
    ///
    /// # Returns
    /// Complete trajectory
    pub fn build(self, final_quality: f32) -> QueryTrajectory;

    /// Get current step count
    pub fn step_count(&self) -> usize;

    /// Get elapsed time
    pub fn elapsed(&self) -> Duration;
}
```

---

## Learning Loops

### InstantLoop

Per-request learning (Loop A).

```rust
/// Instant learning loop for per-request adaptation
pub struct InstantLoop {
    // Private fields
}

impl InstantLoop {
    /// Create new instant loop
    ///
    /// # Arguments
    /// * `hidden_dim` - Model hidden dimension
    /// * `config` - Loop configuration
    pub fn new(hidden_dim: usize, config: InstantLoopConfig) -> Self;

    /// Process inference event
    ///
    /// Records trajectory and updates micro-LoRA.
    ///
    /// # Arguments
    /// * `trajectory` - Completed trajectory
    pub fn on_inference(&self, trajectory: QueryTrajectory);

    /// Flush accumulated updates
    ///
    /// Applies micro-LoRA gradients and commits edge weights.
    pub fn flush_updates(&self);

    /// Drain trajectories for background processing
    pub fn drain_trajectories(&self) -> Vec<QueryTrajectory>;

    /// Get micro-LoRA reference
    pub fn micro_lora(&self) -> &RwLock<MicroLoRA>;

    /// Get metrics
    pub fn metrics(&self) -> InstantLoopMetrics;
}

/// Configuration for instant loop
#[derive(Clone)]
pub struct InstantLoopConfig {
    /// Micro-LoRA rank (default: 1)
    pub micro_lora_rank: usize,

    /// Learning rate (default: 0.001)
    pub micro_lora_lr: f32,

    /// Edge update scale (default: 0.01)
    pub edge_update_scale: f32,

    /// Maximum pending signals (default: 1000)
    pub max_pending_signals: usize,
}
```

### BackgroundLoop

Hourly learning (Loop B).

```rust
/// Background learning loop for hourly pattern extraction
pub struct BackgroundLoop {
    // Private fields
}

impl BackgroundLoop {
    /// Create new background loop
    ///
    /// # Arguments
    /// * `config` - Loop configuration
    /// * `hidden_dim` - Model hidden dimension
    pub fn new(config: BackgroundLoopConfig, hidden_dim: usize) -> Self;

    /// Run background cycle
    ///
    /// # Arguments
    /// * `trajectories` - Trajectories to process
    ///
    /// # Returns
    /// Cycle result with metrics
    pub async fn run_cycle(&self, trajectories: Vec<QueryTrajectory>) -> BackgroundResult;

    /// Get reasoning bank reference
    pub fn reasoning_bank(&self) -> &Arc<RwLock<ReasoningBank>>;

    /// Get EWC++ reference
    pub fn ewc(&self) -> &Arc<RwLock<EwcPlusPlus>>;

    /// Get base LoRA reference
    pub fn base_lora(&self) -> &Arc<RwLock<BaseLoRA>>;
}

/// Configuration for background loop
#[derive(Clone)]
pub struct BackgroundLoopConfig {
    /// Extraction interval (default: 1 hour)
    pub extraction_interval: Duration,

    /// Minimum trajectories required (default: 100)
    pub min_trajectories: usize,

    /// Base LoRA learning rate (default: 0.0001)
    pub base_lora_lr: f32,

    /// EWC lambda (default: 1000.0)
    pub ewc_lambda: f32,
}
```

### DeepLoop

Weekly deep learning (Loop C).

```rust
/// Deep learning loop for weekly consolidation
pub struct DeepLoop {
    // Private fields
}

impl DeepLoop {
    /// Create new deep loop
    pub fn new(config: DeepLoopConfig) -> Self;

    /// Run deep cycle
    ///
    /// Generates dreams, evaluates with Φ, consolidates memory.
    pub async fn run_cycle(&self) -> DeepResult;

    /// Get dream engine reference
    pub fn dream_engine(&self) -> &Arc<RwLock<DreamEngine>>;
}

/// Configuration for deep loop
#[derive(Clone)]
pub struct DeepLoopConfig {
    /// Dreams per cycle (default: 50)
    pub dreams_per_cycle: usize,

    /// Consolidation threshold (default: 0.7)
    pub consolidation_threshold: f32,

    /// Φ threshold (default: 0.3)
    pub phi_threshold: f64,

    /// Maximum cycle duration (default: 10 minutes)
    pub max_cycle_duration: Duration,
}
```

---

## ReasoningBank

### ReasoningBank

Pattern storage and extraction.

```rust
/// Bank for storing and extracting reasoning patterns
pub struct ReasoningBank {
    // Private fields
}

impl ReasoningBank {
    /// Create new reasoning bank
    ///
    /// # Arguments
    /// * `config` - Pattern configuration
    pub fn new(config: PatternConfig) -> Self;

    /// Add trajectory to bank
    ///
    /// # Arguments
    /// * `trajectory` - Trajectory to add
    pub fn add_trajectory(&mut self, trajectory: &QueryTrajectory);

    /// Extract patterns using K-means++
    ///
    /// # Returns
    /// Vector of learned patterns
    pub fn extract_patterns(&mut self) -> Vec<LearnedPattern>;

    /// Get trajectory count
    pub fn trajectory_count(&self) -> usize;

    /// Clear all trajectories
    pub fn clear(&mut self);

    /// Get pattern by ID
    pub fn get_pattern(&self, id: u64) -> Option<&LearnedPattern>;
}

/// Configuration for pattern extraction
#[derive(Clone)]
pub struct PatternConfig {
    /// Number of clusters (default: 50)
    pub k_clusters: usize,

    /// Embedding dimension (default: 256)
    pub embedding_dim: usize,

    /// Maximum iterations (default: 100)
    pub max_iterations: usize,

    /// Convergence threshold (default: 0.001)
    pub convergence_threshold: f32,

    /// Minimum cluster size (default: 5)
    pub min_cluster_size: usize,
}
```

---

## EWC++ Module

### EwcPlusPlus

Enhanced Elastic Weight Consolidation.

```rust
/// EWC++ with online Fisher estimation and multi-task memory
pub struct EwcPlusPlus {
    // Private fields
}

impl EwcPlusPlus {
    /// Create new EWC++
    ///
    /// # Arguments
    /// * `config` - EWC configuration
    pub fn new(config: EwcConfig) -> Self;

    /// Apply constraints to gradients
    ///
    /// Projects gradients to preserve important parameters.
    ///
    /// # Arguments
    /// * `gradients` - Raw gradients
    ///
    /// # Returns
    /// Constrained gradients
    pub fn apply_constraints(&self, gradients: &[f32]) -> Vec<f32>;

    /// Update Fisher information
    ///
    /// # Arguments
    /// * `gradients` - Gradients from current batch
    pub fn update_fisher(&mut self, gradients: &[f32]);

    /// Detect task boundary
    ///
    /// # Arguments
    /// * `gradients` - Current gradients
    ///
    /// # Returns
    /// `true` if task boundary detected
    pub fn detect_task_boundary(&mut self, gradients: &[f32]) -> bool;

    /// Start new task
    ///
    /// Saves current Fisher to task memory.
    pub fn start_new_task(&mut self);

    /// Consolidate all tasks
    ///
    /// Merges multi-task Fisher information.
    pub fn consolidate_all_tasks(&mut self);

    /// Get current lambda
    pub fn lambda(&self) -> f32;

    /// Set lambda
    pub fn set_lambda(&mut self, lambda: f32);

    /// Get task count
    pub fn task_count(&self) -> usize;
}

/// Configuration for EWC++
#[derive(Clone)]
pub struct EwcConfig {
    /// Number of parameters (required)
    pub param_count: usize,

    /// Maximum tasks to remember (default: 10)
    pub max_tasks: usize,

    /// Initial lambda (default: 1000.0)
    pub initial_lambda: f32,

    /// Fisher EMA decay (default: 0.999)
    pub fisher_ema_decay: f32,

    /// Task boundary threshold (default: 2.0)
    pub boundary_threshold: f32,

    /// Minimum lambda (default: 100.0)
    pub min_lambda: f32,

    /// Maximum lambda (default: 10000.0)
    pub max_lambda: f32,
}
```

---

## Dream Engine

### DreamEngine

Dream generation and integration.

```rust
/// Engine for generating and evaluating dreams
pub struct DreamEngine {
    // Private fields
}

impl DreamEngine {
    /// Create new dream engine
    ///
    /// # Arguments
    /// * `config` - Dream configuration
    pub fn new(config: DreamConfig) -> Self;

    /// Add memory node
    ///
    /// # Arguments
    /// * `node` - Memory node to add
    pub fn add_memory_node(&mut self, node: MemoryNode);

    /// Generate single dream
    ///
    /// # Returns
    /// Generated dream
    pub fn generate_dream(&self) -> Dream;

    /// Generate multiple dreams
    ///
    /// # Arguments
    /// * `count` - Number of dreams
    ///
    /// # Returns
    /// Vector of dreams
    pub fn generate_dreams(&self, count: usize) -> Vec<Dream>;

    /// Integrate dream into memory
    ///
    /// Creates weak edges for creative connections.
    ///
    /// # Arguments
    /// * `dream` - Dream to integrate
    pub fn integrate_dream(&mut self, dream: &Dream);

    /// Get memory node count
    pub fn node_count(&self) -> usize;
}

/// Dream representation
#[derive(Clone, Debug)]
pub struct Dream {
    /// Dream identifier
    pub id: u64,

    /// Path through memory
    pub path: Vec<MemoryNode>,

    /// Number of creative jumps
    pub creative_jumps: usize,

    /// Total novelty score
    pub total_novelty: f32,
}

/// Memory node in dream graph
#[derive(Clone, Debug)]
pub struct MemoryNode {
    /// Node identifier
    pub id: u64,

    /// Node embedding
    pub embedding: Vec<f32>,

    /// Last access time
    pub timestamp: Instant,

    /// Access count
    pub access_count: u32,

    /// Importance score
    pub importance: f32,
}

/// Dream configuration
#[derive(Clone)]
pub struct DreamConfig {
    /// Path length (default: 15)
    pub path_length: usize,

    /// Creative jump probability (default: 0.3)
    pub creative_jump_prob: f32,

    /// Random walk restart prob (default: 0.1)
    pub restart_prob: f32,

    /// Novelty weight (default: 0.3)
    pub novelty_weight: f32,

    /// Coherence weight (default: 0.4)
    pub coherence_weight: f32,

    /// Utility weight (default: 0.3)
    pub utility_weight: f32,
}
```

---

## Main Engine

### SonaEngine

Unified SONA interface.

```rust
/// Main SONA engine integrating all components
pub struct SonaEngine {
    // Private fields
}

impl SonaEngine {
    /// Create new SONA engine
    ///
    /// # Arguments
    /// * `config` - Engine configuration
    ///
    /// # Returns
    /// Initialized engine
    pub async fn new(config: SonaConfig) -> Result<Self, SonaError>;

    /// Process query
    ///
    /// # Arguments
    /// * `query` - Query string
    /// * `context` - Query context
    ///
    /// # Returns
    /// Response with confidence and metadata
    pub async fn process(&mut self, query: &str, context: &Context) -> Result<Response, SonaError>;

    /// Run background learning cycle
    ///
    /// Extracts patterns, updates LoRA, consolidates memory.
    pub async fn background_learn(&mut self) -> Result<LearningResult, SonaError>;

    /// Run deep learning cycle
    ///
    /// Generates dreams, evaluates Φ, full consolidation.
    pub async fn deep_learn(&mut self) -> Result<DeepLearningResult, SonaError>;

    /// Get metrics
    pub fn metrics(&self) -> EngineMetrics;

    /// Save state
    pub async fn save(&self, path: &Path) -> Result<(), SonaError>;

    /// Load state
    pub async fn load(path: &Path) -> Result<Self, SonaError>;
}

/// SONA configuration
#[derive(Clone)]
pub struct SonaConfig {
    /// Hidden dimension
    pub hidden_dim: usize,

    /// Embedding dimension
    pub embedding_dim: usize,

    /// Number of attention heads
    pub num_heads: usize,

    /// Number of model layers
    pub num_layers: usize,

    /// LoRA configuration
    pub lora_config: LoraConfig,

    /// Pattern configuration
    pub pattern_config: PatternConfig,

    /// EWC configuration
    pub ewc_config: EwcConfig,

    /// Dream configuration
    pub dream_config: DreamConfig,

    /// Database URL for persistence
    pub database_url: Option<String>,

    /// Φ threshold for quality
    pub phi_threshold: f64,
}

/// Query context
#[derive(Clone, Default)]
pub struct Context {
    /// User ID
    pub user_id: Option<String>,

    /// Session ID
    pub session_id: Option<String>,

    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Query response
#[derive(Clone, Debug)]
pub struct Response {
    /// Response text
    pub text: String,

    /// Confidence score
    pub confidence: f32,

    /// Patterns used
    pub patterns_used: usize,

    /// Latency in microseconds
    pub latency_us: u64,
}
```

---

## Error Types

```rust
/// SONA error types
#[derive(Debug, thiserror::Error)]
pub enum SonaError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Pattern extraction failed: {0}")]
    PatternExtraction(String),

    #[error("Learning failed: {0}")]
    Learning(String),

    #[error("Memory error: {0}")]
    Memory(String),

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}
```

---

## Feature Flags

```toml
# Cargo.toml
[features]
default = ["std"]
std = []

# SIMD optimizations
simd = []
avx2 = ["simd"]
avx512 = ["simd"]
neon = ["simd"]

# Optional integrations
postgres = ["sqlx", "ruvector-postgres"]
exo = ["exo-core", "exo-temporal", "exo-exotic"]

# All features
full = ["avx2", "postgres", "exo"]
```

---

## Usage Examples

### Basic Usage

```rust
use sona::{SonaEngine, SonaConfig, Context};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create engine
    let config = SonaConfig {
        hidden_dim: 256,
        embedding_dim: 256,
        num_heads: 8,
        num_layers: 12,
        ..Default::default()
    };

    let mut sona = SonaEngine::new(config).await?;

    // Process queries
    for i in 0..100 {
        let response = sona.process(
            &format!("Query {}", i),
            &Context::default()
        ).await?;

        println!("Response: {} (confidence: {:.2})", response.text, response.confidence);
    }

    // Run background learning
    let result = sona.background_learn().await?;
    println!("Learned {} patterns", result.patterns_learned);

    Ok(())
}
```

### Custom LoRA Configuration

```rust
use sona::{MicroLoRA, BaseLoRA, LearningSignal};

fn custom_lora_example() {
    // Create micro-LoRA
    let mut micro = MicroLoRA::new(256, 1);

    // Forward pass
    let input = vec![0.1f32; 256];
    let mut output = vec![0.0f32; 256];

    unsafe { micro.forward_simd(&input, &mut output) };

    // Accumulate gradients
    let signal = LearningSignal {
        query_embedding: input.clone(),
        gradient_estimate: vec![0.01; 256],
        quality_score: 0.8,
        timestamp: std::time::Instant::now(),
        metadata: Default::default(),
    };

    micro.accumulate_gradient(&signal);

    // Apply updates
    micro.apply_accumulated(0.001);
}
```

### Learning Loop Integration

```rust
use sona::{InstantLoop, BackgroundLoop, DeepLoop};
use sona::{InstantLoopConfig, BackgroundLoopConfig, DeepLoopConfig};

async fn learning_loop_example() {
    // Create loops
    let instant = InstantLoop::new(256, InstantLoopConfig::default());
    let background = BackgroundLoop::new(BackgroundLoopConfig::default(), 256);
    let deep = DeepLoop::new(DeepLoopConfig::default());

    // Instant learning (per-request)
    let trajectory = create_trajectory();
    instant.on_inference(trajectory);
    instant.flush_updates();

    // Background learning (hourly)
    let trajectories = instant.drain_trajectories();
    if trajectories.len() >= 100 {
        let result = background.run_cycle(trajectories).await;
        println!("Background: {} patterns", result.patterns_extracted);
    }

    // Deep learning (weekly)
    let result = deep.run_cycle().await;
    println!("Deep: {} dreams integrated", result.dreams_integrated);
}
```

---

## Version History

| Version | Changes |
|---------|---------|
| 0.1.0   | Initial release with Micro-LoRA |
| 0.2.0   | Added EWC++ and ReasoningBank |
| 0.3.0   | Dream engine and Φ evaluation |
| 0.4.0   | Full three-tier learning loops |
| 1.0.0   | Production release |
