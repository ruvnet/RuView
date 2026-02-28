# SONA Implementation Roadmap

## Overview

This document outlines the **optimized, prioritized** implementation strategy for SONA (Self-Optimizing Neural Architecture). The roadmap leverages existing ruvLLM infrastructure and focuses on maximum value with minimum disruption.

## Gap Analysis: Existing vs Required

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     EXISTING INFRASTRUCTURE                             │
├─────────────────────────────────────────────────────────────────────────┤
│  ✅ LearningService      │ Has EWC skeleton, replay buffer, feedback   │
│  ✅ FastGRNNRouter       │ Low-rank decomposition, 7 output heads      │
│  ✅ MemoryService        │ HNSW graph, node storage, edge weights      │
│  ✅ SIMD Infrastructure  │ AVX2 softmax, matmul, RMS norm              │
│  ✅ Three-Loop Design    │ Loop A/B/C conceptually defined             │
├─────────────────────────────────────────────────────────────────────────┤
│                         GAPS TO FILL                                    │
├─────────────────────────────────────────────────────────────────────────┤
│  ❌ Micro-LoRA           │ Per-request adaptation (NEW)                │
│  ❌ Trajectory Recording │ Step-by-step inference capture              │
│  ❌ EWC++ Enhancements   │ Online Fisher, task boundary detection      │
│  ❌ ReasoningBank        │ K-means++ pattern extraction                │
│  ❌ Dream Engine         │ Random walk + Φ evaluation                  │
│  ❌ Loop Coordinator     │ Temporal orchestration of A/B/C             │
└─────────────────────────────────────────────────────────────────────────┘
```

## Optimized Priority Matrix

| Priority | Component | Impact | Effort | Build On |
|----------|-----------|--------|--------|----------|
| **P0** | Trajectory Recording | High | Low | types.rs |
| **P0** | Micro-LoRA | High | Medium | simd_inference.rs |
| **P1** | EWC++ Enhancement | High | Medium | learning.rs (existing) |
| **P1** | ReasoningBank | High | Medium | memory.rs |
| **P2** | Loop Coordinator | Medium | Low | learning.rs |
| **P2** | Dream Engine | Medium | High | exo-ai crates |
| **P3** | Φ Measurement | Low | High | exo-core |

## Implementation Philosophy

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     Implementation Principles                           │
├─────────────────────────────────────────────────────────────────────────┤
│  1. Leverage Existing    │ Build on learning.rs, router.rs, memory.rs  │
│  2. Incremental Value    │ Each phase delivers working functionality   │
│  3. Test-First           │ TDD with comprehensive coverage             │
│  4. Benchmark-Driven     │ Performance validated at each step          │
│  5. Backward Compatible  │ No breaking changes to existing API         │
│  6. Modular Design       │ Components can be used independently        │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## OPTIMIZED PHASE STRUCTURE

### Sprint 1: Foundation (P0) - Core Data Flow

**Goal**: Enable trajectory capture and micro-adaptation without breaking existing API.

**Files to Create**:
- `src/sona/mod.rs` - SONA module entry point
- `src/sona/types.rs` - Core types (LearningSignal, QueryTrajectory)
- `src/sona/lora.rs` - MicroLoRA implementation
- `src/sona/trajectory.rs` - Lock-free trajectory buffer

**Files to Modify**:
- `src/lib.rs` - Add `pub mod sona;`
- `src/orchestrator.rs` - Inject trajectory recording hooks

### Sprint 2: Learning Enhancement (P1) - EWC++ & Patterns

**Goal**: Upgrade existing EWC to EWC++, add pattern extraction.

**Files to Modify**:
- `src/learning.rs` - Upgrade EWCState → EwcPlusPlus
- `src/memory.rs` - Add pattern extraction methods

**Files to Create**:
- `src/sona/ewc.rs` - Full EWC++ with online Fisher
- `src/sona/reasoning_bank.rs` - K-means++ pattern storage

### Sprint 3: Loop Orchestration (P2) - Temporal Coordination

**Goal**: Unify instant/background/deep learning cycles.

**Files to Create**:
- `src/sona/loops/mod.rs` - Loop module
- `src/sona/loops/instant.rs` - Loop A
- `src/sona/loops/background.rs` - Loop B
- `src/sona/loops/deep.rs` - Loop C
- `src/sona/coordinator.rs` - LoopCoordinator

### Sprint 4: Dream & Φ (P3) - Creative Exploration

**Goal**: Add dream-based consolidation with quality measurement.

**Files to Create**:
- `src/sona/dreams.rs` - DreamEngine
- `src/sona/phi.rs` - Φ evaluator (optional exo-core integration)

---

## SPRINT 1: Foundation (P0) - Detailed Implementation

### 1.1 Core Data Structures (SIMPLIFIED)

**Deliverables**:
- [ ] `LearningSignal` struct with gradient estimation
- [ ] `QueryTrajectory` for inference recording
- [ ] `LearnedPattern` for pattern storage
- [ ] SIMD-optimized tensor operations

**Implementation**:

```rust
// src/sona/types.rs

/// Learning signal from inference
#[derive(Clone, Debug)]
pub struct LearningSignal {
    pub query_embedding: Vec<f32>,
    pub gradient_estimate: Vec<f32>,
    pub quality_score: f32,
    pub timestamp: Instant,
    pub metadata: SignalMetadata,
}

impl LearningSignal {
    /// Create from query trajectory
    pub fn from_trajectory(trajectory: &QueryTrajectory) -> Self {
        let gradient = Self::estimate_gradient(trajectory);

        Self {
            query_embedding: trajectory.query_embedding.clone(),
            gradient_estimate: gradient,
            quality_score: trajectory.final_quality,
            timestamp: Instant::now(),
            metadata: SignalMetadata {
                trajectory_id: trajectory.id,
                step_count: trajectory.steps.len(),
            },
        }
    }

    /// Estimate gradient from trajectory using REINFORCE
    fn estimate_gradient(trajectory: &QueryTrajectory) -> Vec<f32> {
        let dim = trajectory.query_embedding.len();
        let mut gradient = vec![0.0; dim];

        let baseline = trajectory.steps.iter()
            .map(|s| s.reward)
            .sum::<f32>() / trajectory.steps.len() as f32;

        for step in &trajectory.steps {
            let advantage = step.reward - baseline;
            for (i, &activation) in step.activations.iter().enumerate() {
                gradient[i] += advantage * activation;
            }
        }

        // Normalize
        let norm: f32 = gradient.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-6 {
            gradient.iter_mut().for_each(|x| *x /= norm);
        }

        gradient
    }
}

/// Query trajectory recording
#[derive(Clone, Debug)]
pub struct QueryTrajectory {
    pub id: u64,
    pub query_embedding: Vec<f32>,
    pub steps: Vec<TrajectoryStep>,
    pub final_quality: f32,
    pub latency_us: u64,
}

#[derive(Clone, Debug)]
pub struct TrajectoryStep {
    pub activations: Vec<f32>,
    pub attention_weights: Vec<f32>,
    pub reward: f32,
    pub timestamp: Instant,
}

/// Learned pattern from pattern extraction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearnedPattern {
    pub id: u64,
    pub centroid: Vec<f32>,
    pub cluster_size: usize,
    pub total_weight: f32,
    pub avg_quality: f32,
    pub created_at: u64,
    pub last_accessed: u64,
    pub access_count: u32,
}

impl LearnedPattern {
    /// Merge two patterns
    pub fn merge(&self, other: &Self) -> Self {
        let total_size = self.cluster_size + other.cluster_size;
        let w1 = self.cluster_size as f32 / total_size as f32;
        let w2 = other.cluster_size as f32 / total_size as f32;

        let centroid: Vec<f32> = self.centroid.iter()
            .zip(&other.centroid)
            .map(|(&a, &b)| a * w1 + b * w2)
            .collect();

        Self {
            id: self.id,  // Keep original ID
            centroid,
            cluster_size: total_size,
            total_weight: self.total_weight + other.total_weight,
            avg_quality: self.avg_quality * w1 + other.avg_quality * w2,
            created_at: self.created_at.min(other.created_at),
            last_accessed: self.last_accessed.max(other.last_accessed),
            access_count: self.access_count + other.access_count,
        }
    }

    /// Decay pattern importance over time
    pub fn decay(&mut self, factor: f32) {
        self.total_weight *= factor;
    }
}
```

**Tests**:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learning_signal_creation() {
        let trajectory = QueryTrajectory {
            id: 1,
            query_embedding: vec![0.1, 0.2, 0.3],
            steps: vec![
                TrajectoryStep {
                    activations: vec![0.5, 0.3, 0.2],
                    attention_weights: vec![0.4, 0.4, 0.2],
                    reward: 0.8,
                    timestamp: Instant::now(),
                },
            ],
            final_quality: 0.8,
            latency_us: 1000,
        };

        let signal = LearningSignal::from_trajectory(&trajectory);
        assert_eq!(signal.quality_score, 0.8);
        assert_eq!(signal.gradient_estimate.len(), 3);
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
        };

        let merged = p1.merge(&p2);
        assert_eq!(merged.cluster_size, 20);
        assert!((merged.centroid[0] - 0.5).abs() < 1e-6);
        assert!((merged.centroid[1] - 0.5).abs() < 1e-6);
        assert!((merged.avg_quality - 0.85).abs() < 1e-6);
    }
}
```

### 1.2 Micro-LoRA Implementation

**Deliverables**:
- [ ] `MicroLoRA` struct with rank 1-2 adapters
- [ ] SIMD-optimized forward pass
- [ ] Gradient accumulation buffer
- [ ] Sub-100μs update mechanism

**Implementation**:

```rust
// src/sona/lora.rs

/// Micro-LoRA for per-request adaptation
pub struct MicroLoRA {
    /// Down projection (hidden_dim -> rank)
    pub down_proj: Vec<f32>,
    /// Up projection (rank -> hidden_dim)
    pub up_proj: Vec<f32>,
    /// Rank (1-2 for micro updates)
    pub rank: usize,
    /// Hidden dimension
    pub hidden_dim: usize,
    /// Accumulated gradients
    gradient_buffer: Vec<f32>,
    /// Update count for averaging
    update_count: usize,
    /// Scaling factor
    pub scale: f32,
}

impl MicroLoRA {
    pub fn new(hidden_dim: usize, rank: usize) -> Self {
        assert!(rank <= 2, "MicroLoRA rank should be 1-2");

        // Initialize with small random values
        let mut rng = rand::thread_rng();
        let down_proj: Vec<f32> = (0..hidden_dim * rank)
            .map(|_| rng.gen::<f32>() * 0.01)
            .collect();
        let up_proj = vec![0.0; rank * hidden_dim];  // Initialize to zero

        Self {
            down_proj,
            up_proj,
            rank,
            hidden_dim,
            gradient_buffer: vec![0.0; (hidden_dim * rank) * 2],
            update_count: 0,
            scale: 1.0 / (rank as f32).sqrt(),
        }
    }

    /// SIMD-optimized forward pass
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    pub unsafe fn forward_simd(&self, input: &[f32], output: &mut [f32]) {
        use std::arch::x86_64::*;

        assert_eq!(input.len(), self.hidden_dim);
        assert_eq!(output.len(), self.hidden_dim);

        // Down projection: hidden_dim -> rank
        let mut intermediate = vec![0.0f32; self.rank];

        for r in 0..self.rank {
            let mut sum = _mm256_setzero_ps();
            let down_offset = r * self.hidden_dim;

            let mut i = 0;
            while i + 8 <= self.hidden_dim {
                let inp = _mm256_loadu_ps(input[i..].as_ptr());
                let weight = _mm256_loadu_ps(self.down_proj[down_offset + i..].as_ptr());
                sum = _mm256_fmadd_ps(inp, weight, sum);
                i += 8;
            }

            // Horizontal sum
            let mut result = [0.0f32; 8];
            _mm256_storeu_ps(result.as_mut_ptr(), sum);
            intermediate[r] = result.iter().sum();

            // Handle remaining elements
            for j in i..self.hidden_dim {
                intermediate[r] += input[j] * self.down_proj[down_offset + j];
            }
        }

        // Up projection: rank -> hidden_dim
        let mut i = 0;
        while i + 8 <= self.hidden_dim {
            let mut sum = _mm256_setzero_ps();

            for r in 0..self.rank {
                let up_offset = r * self.hidden_dim;
                let weight = _mm256_loadu_ps(self.up_proj[up_offset + i..].as_ptr());
                let inter = _mm256_set1_ps(intermediate[r]);
                sum = _mm256_fmadd_ps(inter, weight, sum);
            }

            // Scale and add to output
            let scale_vec = _mm256_set1_ps(self.scale);
            sum = _mm256_mul_ps(sum, scale_vec);
            let existing = _mm256_loadu_ps(output[i..].as_ptr());
            let result = _mm256_add_ps(existing, sum);
            _mm256_storeu_ps(output[i..].as_mut_ptr(), result);

            i += 8;
        }

        // Handle remaining elements
        for j in i..self.hidden_dim {
            let mut val = 0.0;
            for r in 0..self.rank {
                val += intermediate[r] * self.up_proj[r * self.hidden_dim + j];
            }
            output[j] += val * self.scale;
        }
    }

    /// Accumulate gradient for later update
    pub fn accumulate_gradient(&mut self, signal: &LearningSignal) {
        assert_eq!(signal.gradient_estimate.len(), self.hidden_dim);

        // Accumulate into buffer (simplified outer product update)
        for r in 0..self.rank {
            for i in 0..self.hidden_dim {
                let grad_idx = r * self.hidden_dim + i;
                self.gradient_buffer[grad_idx] +=
                    signal.gradient_estimate[i] * signal.quality_score;
            }
        }

        self.update_count += 1;
    }

    /// Apply accumulated gradients with learning rate
    pub fn apply_accumulated(&mut self, learning_rate: f32) {
        if self.update_count == 0 {
            return;
        }

        let scale = learning_rate / self.update_count as f32;

        // Update up projection (main adaptation target)
        for (i, grad) in self.gradient_buffer.iter().enumerate() {
            if i < self.up_proj.len() {
                self.up_proj[i] += grad * scale;
            }
        }

        // Reset buffer
        self.gradient_buffer.fill(0.0);
        self.update_count = 0;
    }

    /// Get current parameter count
    pub fn param_count(&self) -> usize {
        self.down_proj.len() + self.up_proj.len()
    }
}

/// Base LoRA for hourly adaptation
pub struct BaseLoRA {
    pub layers: Vec<LoRALayer>,
    pub rank: usize,
    pub hidden_dim: usize,
    pub alpha: f32,
}

#[derive(Clone)]
pub struct LoRALayer {
    pub down_proj: Vec<f32>,
    pub up_proj: Vec<f32>,
    pub layer_idx: usize,
}

impl BaseLoRA {
    pub fn new(hidden_dim: usize, rank: usize, num_layers: usize) -> Self {
        let layers = (0..num_layers)
            .map(|idx| LoRALayer {
                down_proj: vec![0.0; hidden_dim * rank],
                up_proj: vec![0.0; rank * hidden_dim],
                layer_idx: idx,
            })
            .collect();

        Self {
            layers,
            rank,
            hidden_dim,
            alpha: rank as f32,
        }
    }

    /// Merge base LoRA into model weights
    pub fn merge_weights(&self, model_weights: &mut [f32], layer_idx: usize) {
        if layer_idx >= self.layers.len() {
            return;
        }

        let layer = &self.layers[layer_idx];
        let scale = self.alpha / self.rank as f32;

        // W' = W + scale * (down @ up)
        for i in 0..self.hidden_dim {
            for j in 0..self.hidden_dim {
                let mut delta = 0.0;
                for r in 0..self.rank {
                    delta += layer.down_proj[i * self.rank + r]
                           * layer.up_proj[r * self.hidden_dim + j];
                }
                model_weights[i * self.hidden_dim + j] += delta * scale;
            }
        }
    }
}
```

### 1.3 Trajectory Recording

**Deliverables**:
- [ ] Lock-free trajectory buffer
- [ ] Efficient step recording
- [ ] Quality signal extraction

**Implementation**:

```rust
// src/sona/trajectory.rs

use crossbeam::queue::ArrayQueue;

/// Lock-free trajectory buffer
pub struct TrajectoryBuffer {
    buffer: ArrayQueue<QueryTrajectory>,
    capacity: usize,
    dropped: AtomicU64,
}

impl TrajectoryBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: ArrayQueue::new(capacity),
            capacity,
            dropped: AtomicU64::new(0),
        }
    }

    /// Record trajectory (non-blocking)
    pub fn record(&self, trajectory: QueryTrajectory) -> bool {
        match self.buffer.push(trajectory) {
            Ok(()) => true,
            Err(_) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    /// Drain all trajectories for processing
    pub fn drain(&self) -> Vec<QueryTrajectory> {
        let mut result = Vec::with_capacity(self.capacity);
        while let Some(t) = self.buffer.pop() {
            result.push(t);
        }
        result
    }

    /// Get dropped count
    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

/// Builder for constructing trajectories during inference
pub struct TrajectoryBuilder {
    id: u64,
    query_embedding: Vec<f32>,
    steps: Vec<TrajectoryStep>,
    start_time: Instant,
}

impl TrajectoryBuilder {
    pub fn new(id: u64, query_embedding: Vec<f32>) -> Self {
        Self {
            id,
            query_embedding,
            steps: Vec::with_capacity(16),
            start_time: Instant::now(),
        }
    }

    /// Record a step
    pub fn add_step(&mut self, activations: Vec<f32>, attention_weights: Vec<f32>, reward: f32) {
        self.steps.push(TrajectoryStep {
            activations,
            attention_weights,
            reward,
            timestamp: Instant::now(),
        });
    }

    /// Finalize trajectory
    pub fn build(self, final_quality: f32) -> QueryTrajectory {
        QueryTrajectory {
            id: self.id,
            query_embedding: self.query_embedding,
            steps: self.steps,
            final_quality,
            latency_us: self.start_time.elapsed().as_micros() as u64,
        }
    }
}
```

---

## Phase 2: Learning Loops

### 2.1 Loop A (Instant Learning)

**Deliverables**:
- [ ] Per-request trajectory recording
- [ ] Micro-LoRA gradient accumulation
- [ ] Edge weight updates

**Implementation**:

```rust
// src/sona/loops/instant.rs

/// Instant learning loop (per-request)
pub struct InstantLoop {
    trajectory_buffer: Arc<TrajectoryBuffer>,
    micro_lora: RwLock<MicroLoRA>,
    edge_weights: RwLock<EdgeWeights>,
    config: InstantLoopConfig,
    metrics: InstantLoopMetrics,
}

#[derive(Clone)]
pub struct InstantLoopConfig {
    pub micro_lora_rank: usize,
    pub micro_lora_lr: f32,
    pub edge_update_scale: f32,
    pub max_pending_signals: usize,
}

impl Default for InstantLoopConfig {
    fn default() -> Self {
        Self {
            micro_lora_rank: 1,
            micro_lora_lr: 0.001,
            edge_update_scale: 0.01,
            max_pending_signals: 1000,
        }
    }
}

impl InstantLoop {
    pub fn new(hidden_dim: usize, config: InstantLoopConfig) -> Self {
        Self {
            trajectory_buffer: Arc::new(TrajectoryBuffer::new(config.max_pending_signals)),
            micro_lora: RwLock::new(MicroLoRA::new(hidden_dim, config.micro_lora_rank)),
            edge_weights: RwLock::new(EdgeWeights::new()),
            config,
            metrics: InstantLoopMetrics::default(),
        }
    }

    /// Process inference request (called during forward pass)
    pub fn on_inference(&self, trajectory: QueryTrajectory) {
        // Record trajectory
        self.trajectory_buffer.record(trajectory.clone());

        // Generate learning signal
        let signal = LearningSignal::from_trajectory(&trajectory);

        // Accumulate gradient (non-blocking)
        if let Ok(mut lora) = self.micro_lora.try_write() {
            lora.accumulate_gradient(&signal);
        }

        // Update edge weights (non-blocking)
        if let Ok(mut edges) = self.edge_weights.try_write() {
            edges.update_from_signal(&signal, self.config.edge_update_scale);
        }
    }

    /// Apply accumulated updates (called periodically)
    pub fn flush_updates(&self) {
        // Apply micro-LoRA updates
        if let Ok(mut lora) = self.micro_lora.write() {
            lora.apply_accumulated(self.config.micro_lora_lr);
        }

        // Commit edge weight updates
        if let Ok(mut edges) = self.edge_weights.write() {
            edges.commit();
        }
    }

    /// Get trajectory buffer for background processing
    pub fn drain_trajectories(&self) -> Vec<QueryTrajectory> {
        self.trajectory_buffer.drain()
    }
}

/// Edge weights for knowledge graph
pub struct EdgeWeights {
    weights: HashMap<(NodeId, NodeId), f32>,
    pending_updates: Vec<(NodeId, NodeId, f32)>,
}

impl EdgeWeights {
    pub fn new() -> Self {
        Self {
            weights: HashMap::new(),
            pending_updates: Vec::new(),
        }
    }

    pub fn update_from_signal(&mut self, signal: &LearningSignal, scale: f32) {
        // Extract node pairs from signal (simplified)
        let nodes = Self::extract_activated_nodes(signal);

        for i in 0..nodes.len() {
            for j in i+1..nodes.len() {
                let delta = signal.quality_score * scale;
                self.pending_updates.push((nodes[i], nodes[j], delta));
            }
        }
    }

    pub fn commit(&mut self) {
        for (from, to, delta) in self.pending_updates.drain(..) {
            *self.weights.entry((from, to)).or_insert(0.0) += delta;
        }
    }

    fn extract_activated_nodes(signal: &LearningSignal) -> Vec<NodeId> {
        // Simplified: top-k indices from gradient
        signal.gradient_estimate.iter()
            .enumerate()
            .filter(|(_, &v)| v.abs() > 0.1)
            .take(5)
            .map(|(i, _)| i as NodeId)
            .collect()
    }
}
```

### 2.2 Loop B (Background Learning)

**Deliverables**:
- [ ] Hourly pattern extraction
- [ ] EWC++ gradient constraints
- [ ] Base LoRA updates

**Implementation**:

```rust
// src/sona/loops/background.rs

/// Background learning loop (hourly)
pub struct BackgroundLoop {
    reasoning_bank: Arc<RwLock<ReasoningBank>>,
    ewc: Arc<RwLock<EwcPlusPlus>>,
    base_lora: Arc<RwLock<BaseLoRA>>,
    scheduler: BackgroundScheduler,
    config: BackgroundLoopConfig,
}

#[derive(Clone)]
pub struct BackgroundLoopConfig {
    pub extraction_interval: Duration,
    pub min_trajectories: usize,
    pub base_lora_lr: f32,
    pub ewc_lambda: f32,
}

impl Default for BackgroundLoopConfig {
    fn default() -> Self {
        Self {
            extraction_interval: Duration::from_secs(3600),  // 1 hour
            min_trajectories: 100,
            base_lora_lr: 0.0001,
            ewc_lambda: 1000.0,
        }
    }
}

impl BackgroundLoop {
    pub fn new(config: BackgroundLoopConfig, hidden_dim: usize) -> Self {
        Self {
            reasoning_bank: Arc::new(RwLock::new(ReasoningBank::new(PatternConfig::default()))),
            ewc: Arc::new(RwLock::new(EwcPlusPlus::new(EwcConfig::default()))),
            base_lora: Arc::new(RwLock::new(BaseLoRA::new(hidden_dim, 8, 12))),
            scheduler: BackgroundScheduler::new(config.extraction_interval),
            config,
        }
    }

    /// Run background learning cycle
    pub async fn run_cycle(&self, trajectories: Vec<QueryTrajectory>) -> BackgroundResult {
        if trajectories.len() < self.config.min_trajectories {
            return BackgroundResult::skipped("insufficient trajectories");
        }

        let start = Instant::now();

        // 1. Add trajectories to reasoning bank
        {
            let mut bank = self.reasoning_bank.write().await;
            for trajectory in &trajectories {
                bank.add_trajectory(trajectory);
            }
        }

        // 2. Extract patterns
        let patterns = {
            let mut bank = self.reasoning_bank.write().await;
            bank.extract_patterns()
        };

        // 3. Compute gradients from patterns
        let gradients = self.compute_pattern_gradients(&patterns);

        // 4. Apply EWC++ constraints
        let constrained_gradients = {
            let ewc = self.ewc.read().await;
            ewc.apply_constraints(&gradients)
        };

        // 5. Update base LoRA
        {
            let mut lora = self.base_lora.write().await;
            self.apply_gradients_to_lora(&mut lora, &constrained_gradients);
        }

        // 6. Update EWC++ Fisher information
        {
            let mut ewc = self.ewc.write().await;
            ewc.update_fisher(&constrained_gradients);
        }

        BackgroundResult {
            trajectories_processed: trajectories.len(),
            patterns_extracted: patterns.len(),
            elapsed: start.elapsed(),
            status: "completed".to_string(),
        }
    }

    fn compute_pattern_gradients(&self, patterns: &[LearnedPattern]) -> Vec<f32> {
        // Aggregate pattern centroids weighted by quality
        let mut gradient = vec![0.0f32; patterns.first().map(|p| p.centroid.len()).unwrap_or(0)];
        let mut total_weight = 0.0;

        for pattern in patterns {
            let weight = pattern.avg_quality * pattern.cluster_size as f32;
            for (i, &v) in pattern.centroid.iter().enumerate() {
                gradient[i] += v * weight;
            }
            total_weight += weight;
        }

        if total_weight > 0.0 {
            gradient.iter_mut().for_each(|v| *v /= total_weight);
        }

        gradient
    }

    fn apply_gradients_to_lora(&self, lora: &mut BaseLoRA, gradients: &[f32]) {
        // Distribute gradients across layers
        let per_layer = gradients.len() / lora.layers.len();

        for (layer_idx, layer) in lora.layers.iter_mut().enumerate() {
            let start = layer_idx * per_layer;
            let end = (start + per_layer).min(gradients.len());

            // Update up projection
            for (i, &grad) in gradients[start..end].iter().enumerate() {
                if i < layer.up_proj.len() {
                    layer.up_proj[i] += grad * self.config.base_lora_lr;
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct BackgroundResult {
    pub trajectories_processed: usize,
    pub patterns_extracted: usize,
    pub elapsed: Duration,
    pub status: String,
}

impl BackgroundResult {
    fn skipped(reason: &str) -> Self {
        Self {
            trajectories_processed: 0,
            patterns_extracted: 0,
            elapsed: Duration::ZERO,
            status: format!("skipped: {}", reason),
        }
    }
}
```

### 2.3 Loop C (Deep Learning)

**Deliverables**:
- [ ] Weekly dream generation
- [ ] Memory consolidation
- [ ] Full EWC++ update

**Implementation**:

```rust
// src/sona/loops/deep.rs

/// Deep learning loop (weekly)
pub struct DeepLoop {
    dream_engine: Arc<RwLock<DreamEngine>>,
    memory_consolidator: Arc<RwLock<MemoryConsolidator>>,
    ewc: Arc<RwLock<EwcPlusPlus>>,
    phi_evaluator: Arc<PhiEvaluator>,
    config: DeepLoopConfig,
}

#[derive(Clone)]
pub struct DeepLoopConfig {
    pub dreams_per_cycle: usize,
    pub consolidation_threshold: f32,
    pub phi_threshold: f64,
    pub max_cycle_duration: Duration,
}

impl Default for DeepLoopConfig {
    fn default() -> Self {
        Self {
            dreams_per_cycle: 50,
            consolidation_threshold: 0.7,
            phi_threshold: 0.3,
            max_cycle_duration: Duration::from_secs(600),  // 10 minutes
        }
    }
}

impl DeepLoop {
    pub async fn run_cycle(&self) -> DeepResult {
        let start = Instant::now();
        let deadline = start + self.config.max_cycle_duration;

        // 1. Generate dreams
        let dreams = {
            let engine = self.dream_engine.read().await;
            engine.generate_dreams(self.config.dreams_per_cycle)
        };

        // 2. Evaluate dreams with Φ
        let mut evaluated_dreams = Vec::new();
        for dream in &dreams {
            if Instant::now() > deadline {
                break;
            }

            let phi = self.phi_evaluator.evaluate_dream(dream);
            if phi >= self.config.phi_threshold {
                evaluated_dreams.push((dream.clone(), phi));
            }
        }

        // 3. Integrate high-quality dreams
        {
            let mut engine = self.dream_engine.write().await;
            for (dream, _phi) in &evaluated_dreams {
                engine.integrate_dream(dream);
            }
        }

        // 4. Consolidate memory
        let consolidation_result = {
            let mut consolidator = self.memory_consolidator.write().await;
            consolidator.consolidate(self.config.consolidation_threshold).await
        };

        // 5. Full EWC++ consolidation
        {
            let mut ewc = self.ewc.write().await;
            ewc.consolidate_all_tasks();
        }

        DeepResult {
            dreams_generated: dreams.len(),
            dreams_integrated: evaluated_dreams.len(),
            patterns_strengthened: consolidation_result.strengthened,
            patterns_pruned: consolidation_result.pruned,
            elapsed: start.elapsed(),
        }
    }
}

#[derive(Debug)]
pub struct DeepResult {
    pub dreams_generated: usize,
    pub dreams_integrated: usize,
    pub patterns_strengthened: usize,
    pub patterns_pruned: usize,
    pub elapsed: Duration,
}
```

---

## Phase 3: Pattern Learning

### 3.1 ReasoningBank Implementation

**Deliverables**:
- [ ] Trajectory storage with circular buffer
- [ ] K-means++ pattern extraction
- [ ] Verdict judgment system

### 3.2 EWC++ Implementation

**Deliverables**:
- [ ] Online Fisher information estimation
- [ ] Multi-task memory with circular buffer
- [ ] Automatic task boundary detection
- [ ] Adaptive lambda scheduling

### 3.3 Dream Engine

**Deliverables**:
- [ ] Random walk dream generation
- [ ] Quality evaluation (novelty, coherence, utility)
- [ ] Dream integration with weak edges

---

## Phase 4: Integration

### 4.1 Unified Pipeline

**Deliverables**:
- [ ] `SonaEngine` main interface
- [ ] Loop coordinator
- [ ] Metrics collection

### 4.2 ruvector Integration

**Deliverables**:
- [ ] Pattern index with HNSW
- [ ] Knowledge graph with GNN
- [ ] Persistent storage with PostgreSQL

### 4.3 exo-ai Integration

**Deliverables**:
- [ ] Φ measurement for quality
- [ ] Temporal pattern learning
- [ ] Quantum-inspired exploration

---

## Phase 5: Optimization

### 5.1 SIMD Optimization

**Deliverables**:
- [ ] AVX2 LoRA forward pass
- [ ] SIMD pattern matching
- [ ] Vectorized gradient computation

### 5.2 Memory Optimization

**Deliverables**:
- [ ] Lock-free data structures
- [ ] Memory pooling
- [ ] Gradient checkpointing

### 5.3 Latency Optimization

**Deliverables**:
- [ ] Sub-100μs micro-updates
- [ ] Async background processing
- [ ] Batched operations

---

## Testing Strategy

### Unit Tests

```rust
// Every public function gets a test
#[cfg(test)]
mod tests {
    // Pattern extraction tests
    #[test]
    fn test_pattern_extraction_empty() { }
    #[test]
    fn test_pattern_extraction_single() { }
    #[test]
    fn test_pattern_extraction_multiple() { }

    // LoRA tests
    #[test]
    fn test_micro_lora_forward() { }
    #[test]
    fn test_micro_lora_gradient_accumulation() { }
    #[test]
    fn test_base_lora_merge() { }

    // EWC tests
    #[test]
    fn test_ewc_constraint_application() { }
    #[test]
    fn test_fisher_update() { }
    #[test]
    fn test_task_boundary_detection() { }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_learning_cycle() {
    let sona = SonaEngine::new(SonaConfig::default()).await.unwrap();

    // Simulate queries
    for i in 0..100 {
        let response = sona.process(&format!("query {}", i), &Context::default()).await;
        assert!(response.is_ok());
    }

    // Trigger background learning
    let result = sona.background_learn().await.unwrap();
    assert!(result.patterns_learned > 0);
}
```

### Benchmarks

```rust
#[bench]
fn bench_micro_lora_forward(b: &mut Bencher) {
    let lora = MicroLoRA::new(256, 1);
    let input = vec![0.1f32; 256];
    let mut output = vec![0.0f32; 256];

    b.iter(|| {
        unsafe { lora.forward_simd(&input, &mut output) };
    });
}

#[bench]
fn bench_pattern_extraction(b: &mut Bencher) {
    let mut bank = ReasoningBank::new(PatternConfig::default());
    // Pre-populate with trajectories

    b.iter(|| {
        bank.extract_patterns()
    });
}
```

---

## Success Criteria

| Metric | Target | Measurement |
|--------|--------|-------------|
| Micro-LoRA latency | <50μs | Benchmark |
| Background cycle | <30s | Benchmark |
| Deep cycle | <10min | Benchmark |
| Pattern quality | >0.7 avg | Metrics |
| Memory overhead | <100MB | Profiling |
| Φ threshold | >0.3 | IIT measurement |

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| SIMD portability | Feature flags for fallback |
| Memory pressure | Configurable buffer sizes |
| Learning instability | EWC++ constraints |
| Catastrophic forgetting | Multi-task Fisher memory |
| Latency regression | Continuous benchmarking |

---

## QUICK-START: Minimal Viable SONA

For immediate value, implement this **minimal 3-file addition**:

### File 1: `src/sona/mod.rs`

```rust
//! SONA - Self-Optimizing Neural Architecture
pub mod types;
pub mod lora;

pub use types::*;
pub use lora::MicroLoRA;
```

### File 2: `src/sona/types.rs` (Minimal)

```rust
use std::time::Instant;

/// Minimal learning signal
#[derive(Clone, Debug)]
pub struct LearningSignal {
    pub embedding: Vec<f32>,
    pub quality: f32,
}

/// Minimal trajectory step
#[derive(Clone, Debug)]
pub struct TrajectoryStep {
    pub hidden_state: Vec<f32>,
    pub reward: f32,
}

/// Query trajectory
#[derive(Clone, Debug)]
pub struct QueryTrajectory {
    pub id: u64,
    pub steps: Vec<TrajectoryStep>,
    pub final_quality: f32,
}

impl LearningSignal {
    pub fn from_trajectory(t: &QueryTrajectory) -> Self {
        // Simple: use last hidden state, weighted by quality
        let embedding = t.steps.last()
            .map(|s| s.hidden_state.clone())
            .unwrap_or_default();
        Self {
            embedding,
            quality: t.final_quality,
        }
    }
}
```

### File 3: `src/sona/lora.rs` (Minimal MicroLoRA)

```rust
/// Minimal Micro-LoRA (rank-1)
pub struct MicroLoRA {
    pub down: Vec<f32>,  // [hidden_dim]
    pub up: Vec<f32>,    // [hidden_dim]
    accum: Vec<f32>,
    count: usize,
}

impl MicroLoRA {
    pub fn new(dim: usize) -> Self {
        Self {
            down: vec![0.01; dim],
            up: vec![0.0; dim],
            accum: vec![0.0; dim],
            count: 0,
        }
    }

    /// Forward: output += scale * (input · down) * up
    pub fn forward(&self, input: &[f32], output: &mut [f32]) {
        let dot: f32 = input.iter().zip(&self.down).map(|(a, b)| a * b).sum();
        let scale = 0.1;
        for (o, &u) in output.iter_mut().zip(&self.up) {
            *o += dot * u * scale;
        }
    }

    /// Accumulate gradient signal
    pub fn accumulate(&mut self, signal: &super::types::LearningSignal) {
        for (a, &e) in self.accum.iter_mut().zip(&signal.embedding) {
            *a += e * signal.quality;
        }
        self.count += 1;
    }

    /// Apply accumulated updates
    pub fn apply(&mut self, lr: f32) {
        if self.count == 0 { return; }
        let scale = lr / self.count as f32;
        for (u, &a) in self.up.iter_mut().zip(&self.accum) {
            *u += a * scale;
        }
        self.accum.fill(0.0);
        self.count = 0;
    }
}
```

### Integration Point: `src/learning.rs`

Add to `LearningService`:

```rust
use crate::sona::{MicroLoRA, QueryTrajectory, LearningSignal};

impl LearningService {
    // Add field: micro_lora: RwLock<MicroLoRA>

    pub fn on_inference_complete(&self, trajectory: QueryTrajectory) {
        let signal = LearningSignal::from_trajectory(&trajectory);
        if let Ok(mut lora) = self.micro_lora.try_write() {
            lora.accumulate(&signal);
        }
    }

    pub fn flush_micro_updates(&self) {
        if let Ok(mut lora) = self.micro_lora.write() {
            lora.apply(0.001);
        }
    }
}
```

**This gives you**:
- ✅ Trajectory recording structure
- ✅ Per-request gradient accumulation
- ✅ Micro-LoRA adaptation
- ✅ No breaking changes to existing API

**Total: ~150 lines of new code**

---

## Critical Success Metrics

| Metric | Sprint 1 | Sprint 2 | Sprint 3 | Sprint 4 |
|--------|----------|----------|----------|----------|
| Micro-LoRA latency | <50μs | - | - | - |
| Trajectory overhead | <10μs | - | - | - |
| EWC++ constraint | - | <500μs | - | - |
| Pattern extraction | - | <1s/1000 | - | - |
| Loop A total | - | - | <1ms | - |
| Loop B cycle | - | - | <30s | - |
| Dream generation | - | - | - | <100ms |

---

## Risk Mitigation (Updated)

| Risk | Mitigation | Owner |
|------|------------|-------|
| SIMD portability | Feature flag `#[cfg(target_arch)]` with scalar fallback | Sprint 1 |
| Memory pressure | Circular buffers with configurable capacity | Sprint 1 |
| Learning instability | Start with conservative lr=0.0001 | Sprint 1 |
| Breaking changes | All SONA code in separate module | All |
| Integration complexity | Inject via trait, not inheritance | Sprint 2+ |

---

## Recommended Execution Order

```
Week 1: Sprint 1 - Foundation
├── Day 1-2: src/sona/types.rs + tests
├── Day 3-4: src/sona/lora.rs + SIMD + benchmarks
└── Day 5: Integration into orchestrator

Week 2: Sprint 2 - Learning
├── Day 1-2: Upgrade EWCState → EwcPlusPlus
├── Day 3-4: ReasoningBank with K-means++
└── Day 5: Integration + benchmarks

Week 3: Sprint 3 - Loops
├── Day 1-2: Loop A (InstantLoop)
├── Day 3-4: Loop B (BackgroundLoop)
└── Day 5: LoopCoordinator

Week 4: Sprint 4 - Dreams (Optional)
├── Day 1-3: DreamEngine
├── Day 4-5: Φ integration (if exo-ai available)
```

---

## Next Steps

1. **08-BENCHMARKS.md** - Detailed performance targets
2. **09-API-REFERENCE.md** - Complete API documentation
