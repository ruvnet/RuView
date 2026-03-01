# Implementation Roadmap: GNN Latent-Graph Interplay

## Executive Summary

This document provides a comprehensive, actionable roadmap for implementing the research findings from the previous documents. We prioritize enhancements based on impact, effort, and alignment with RuVector's HNSW-based architecture.

**Timeline**: 12-month roadmap with quarterly milestones
**Focus**: Practical implementation, benchmarking, and incremental deployment

---

## 1. Current State Assessment

### 1.1 Existing Strengths

**Code Locations**:
- `crates/ruvector-gnn/src/layer.rs` - Core GNN layers
- `crates/ruvector-gnn/src/search.rs` - Differentiable search
- `crates/ruvector-gnn/src/training.rs` - Loss functions, optimizers
- `crates/ruvector-gnn/src/ewc.rs` - Continual learning

**What Works Well**:
- ✓ Robust multi-head attention
- ✓ GRU-based temporal updates
- ✓ Contrastive learning (InfoNCE, local contrastive)
- ✓ EWC for catastrophic forgetting prevention
- ✓ Differentiable hierarchical search
- ✓ Numerical stability (softmax tricks, normalization)

### 1.2 Current Limitations

**Architecture**:
- ✗ No edge feature support (only edge weights)
- ✗ Fixed Euclidean geometry (no hyperbolic option)
- ✗ Limited structural encoding (degree, centrality not used)
- ✗ O(d·h²) attention complexity

**Training**:
- ✗ Random negative sampling only
- ✗ No spectral regularization
- ✗ Manual loss weight tuning
- ✗ No curriculum learning

**Scalability**:
- ✗ Full batch processing (no mini-batching)
- ✗ Single-device training only
- ✗ No Flash Attention (memory-inefficient)

---

## 2. Prioritization Framework

### 2.1 Scoring Criteria

Each enhancement scored 1-5 on:
- **Impact**: Effect on performance/quality
- **Effort**: Implementation complexity
- **Risk**: Potential for breakage/regression
- **Alignment**: Fit with HNSW topology

**Priority Formula**:
```
Priority = (Impact × Alignment) / (Effort × Risk)
```

### 2.2 Priority Tiers

**P0 (Critical)**: Immediate implementation (Weeks 1-4)
**P1 (High)**: Q1 implementation (Months 1-3)
**P2 (Medium)**: Q2-Q3 implementation (Months 4-9)
**P3 (Low)**: Q4 or future (Months 10-12+)

---

## 3. Q1: Foundation Enhancements (Months 1-3)

### 3.1 Week 1-2: Edge Feature Support (P0)

**Goal**: Extend attention to incorporate edge features

**File**: `crates/ruvector-gnn/src/layer.rs`

**Changes**:
```rust
// Before (line 115-154)
pub fn forward(&self, query: &[f32], keys: &[Vec<f32>], values: &[Vec<f32>]) -> Vec<f32>

// After
pub fn forward_with_edges(
    &self,
    query: &[f32],
    keys: &[Vec<f32>],
    values: &[Vec<f32>],
    edge_features: Option<&[Vec<f32>]>,  // NEW
) -> Vec<f32> {
    if let Some(edge_feats) = edge_features {
        // Modify attention scores to include edge features
        let scores: Vec<f32> = keys.iter()
            .zip(edge_feats.iter())
            .map(|(k, e)| {
                let node_score = dot_product(query, k) / scale;
                let edge_score = self.edge_linear.forward(e)[0];
                node_score + edge_score  // Combine
            })
            .collect();
        // ... rest of forward pass
    } else {
        // Fall back to current implementation
        self.forward(query, keys, values)
    }
}
```

**Testing**:
- Unit tests with synthetic edge features
- Benchmark on HNSW with layer-as-edge-feature
- Ablation study: with/without edge features

**Deliverable**: PR with edge-featured attention

---

### 3.2 Week 3-4: Hard Negative Sampling (P0)

**Goal**: Replace random negatives with hard negatives

**File**: `crates/ruvector-gnn/src/training.rs`

**New Function**:
```rust
// Add to training.rs
pub fn sample_hard_negatives(
    anchor: &[f32],
    all_embeddings: &[Vec<f32>],
    true_positives: &[usize],
    k: usize,
    strategy: HardNegativeStrategy,
) -> Vec<Vec<f32>> {
    match strategy {
        HardNegativeStrategy::Distance => {
            // Most similar non-neighbors
            sample_by_similarity(anchor, all_embeddings, true_positives, k)
        },
        HardNegativeStrategy::Degree => {
            // Similar degree distribution
            sample_by_degree(anchor, all_embeddings, degrees, k)
        },
        HardNegativeStrategy::Mixed => {
            // Combination
            sample_mixed(anchor, all_embeddings, true_positives, degrees, k)
        },
    }
}

pub enum HardNegativeStrategy {
    Distance,
    Degree,
    Mixed,
}
```

**Integration**:
```rust
// Modify local_contrastive_loss (line 444)
pub fn local_contrastive_loss_v2(
    node_embedding: &[f32],
    neighbor_embeddings: &[Vec<f32>],
    all_embeddings: &[Vec<f32>],  // NEW: for hard negative mining
    neighbor_indices: &[usize],   // NEW
    temperature: f32,
    use_hard_negatives: bool,     // NEW
) -> f32 {
    let negatives = if use_hard_negatives {
        sample_hard_negatives(
            node_embedding,
            all_embeddings,
            neighbor_indices,
            64,  // k negatives
            HardNegativeStrategy::Distance,
        )
    } else {
        // Current behavior: random sampling
        sample_random_negatives(all_embeddings, neighbor_indices, 64)
    };

    let positives: Vec<&[f32]> = neighbor_embeddings.iter().map(|n| n.as_slice()).collect();
    let negative_refs: Vec<&[f32]> = negatives.iter().map(|n| n.as_slice()).collect();

    info_nce_loss(node_embedding, &positives, &negative_refs, temperature)
}
```

**Testing**:
- Convergence speed comparison (random vs. hard negatives)
- Final performance on link prediction
- Compute cost analysis

**Deliverable**: Enhanced contrastive loss with hard negatives

---

### 3.3 Week 5-6: Spectral Regularization (P1)

**Goal**: Add Laplacian smoothing to preserve graph structure

**File**: `crates/ruvector-gnn/src/training.rs`

**New Function**:
```rust
// Add to training.rs
pub fn laplacian_regularization(
    embeddings: &[Vec<f32>],
    edges: &[(usize, usize)],
    edge_weights: Option<&[f32]>,
    normalized: bool,
) -> f32 {
    let mut loss = 0.0;

    for (idx, &(i, j)) in edges.iter().enumerate() {
        let diff = subtract(&embeddings[i], &embeddings[j]);
        let norm_sq = l2_norm_squared(&diff);

        let weight = edge_weights.map(|w| w[idx]).unwrap_or(1.0);

        loss += if normalized {
            weight * norm_sq / (degrees[i] as f32 * degrees[j] as f32).sqrt()
        } else {
            weight * norm_sq
        };
    }

    loss / edges.len() as f32
}
```

**Integration**:
```rust
// In training loop
let spectral_loss = laplacian_regularization(
    &embeddings,
    &graph.edges,
    Some(&edge_weights),
    true,  // normalized
);

total_loss = task_loss + lambda_contrastive * contrastive_loss + lambda_spectral * spectral_loss;
```

**Hyperparameter**:
- `lambda_spectral`: Start at 0.01, tune via validation

**Testing**:
- Smoothness metric: Avg ||h_i - h_j|| for edges
- Community detection quality (modularity)
- Link prediction AUC

**Deliverable**: Spectral regularization option

---

### 3.4 Week 7-8: Structural Feature Encoding (P1)

**Goal**: Augment embeddings with graph structural features

**File**: `crates/ruvector-gnn/src/layer.rs`

**New Struct**:
```rust
// Add to layer.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralFeatures {
    pub degree: f32,
    pub clustering_coefficient: f32,
    pub hnsw_layer: usize,
    pub betweenness_centrality: f32,
}

impl StructuralFeatures {
    pub fn to_vector(&self) -> Vec<f32> {
        vec![
            self.degree,
            self.clustering_coefficient,
            self.hnsw_layer as f32,
            self.betweenness_centrality,
        ]
    }

    pub fn compute(
        node_id: usize,
        graph: &Graph,
        hnsw_structure: &HNSW,
    ) -> Self {
        let degree = graph.degree(node_id) as f32;
        let clustering_coefficient = graph.clustering_coefficient(node_id);
        let hnsw_layer = hnsw_structure.get_layer(node_id).unwrap_or(0);
        let betweenness_centrality = 0.0;  // Expensive, optional

        Self {
            degree,
            clustering_coefficient,
            hnsw_layer,
            betweenness_centrality,
        }
    }
}
```

**Integration**:
```rust
// Modify RuvectorLayer::forward (line 362)
pub fn forward_with_structural(
    &self,
    node_embedding: &[f32],
    neighbor_embeddings: &[Vec<f32>],
    edge_weights: &[f32],
    structural_features: Option<&StructuralFeatures>,  // NEW
) -> Vec<f32> {
    let augmented_embedding = if let Some(struct_feat) = structural_features {
        // Concatenate structural features
        [node_embedding, &struct_feat.to_vector()].concat()
    } else {
        node_embedding.to_vec()
    };

    // Proceed with standard forward pass using augmented_embedding
    // ...
}
```

**Testing**:
- Ablation: With/without each structural feature
- Downstream task performance
- Embedding dimensionality analysis

**Deliverable**: Structural feature integration

---

### 3.5 Week 9-12: Multi-Objective Loss & Curriculum Learning (P1)

**Goal**: Automated loss balancing and training schedules

**File**: `crates/ruvector-gnn/src/training.rs`

**New Structs**:
```rust
// Multi-objective loss manager
#[derive(Debug, Clone)]
pub struct MultiObjectiveLoss {
    pub lambda_task: f32,
    pub lambda_contrastive: f32,
    pub lambda_reconstruction: f32,
    pub lambda_spectral: f32,
    pub lambda_ewc: f32,
}

impl MultiObjectiveLoss {
    pub fn compute_total(
        &self,
        losses: &LossComponents,
    ) -> f32 {
        self.lambda_task * losses.task
            + self.lambda_contrastive * losses.contrastive
            + self.lambda_reconstruction * losses.reconstruction
            + self.lambda_spectral * losses.spectral
            + self.lambda_ewc * losses.ewc
    }

    pub fn auto_balance(&mut self, losses: &LossComponents) {
        // Normalize so each loss contributes equally initially
        let total = self.compute_total(losses);
        let target = total / 5.0;

        self.lambda_task = target / losses.task.max(1e-10);
        self.lambda_contrastive = target / losses.contrastive.max(1e-10);
        self.lambda_reconstruction = target / losses.reconstruction.max(1e-10);
        self.lambda_spectral = target / losses.spectral.max(1e-10);
        self.lambda_ewc = target / losses.ewc.max(1e-10);
    }
}

// Curriculum scheduler
#[derive(Debug)]
pub struct CurriculumSchedule {
    current_epoch: usize,
    schedules: HashMap<String, Box<dyn Fn(usize) -> f32>>,
}

impl CurriculumSchedule {
    pub fn new_default() -> Self {
        let mut schedules: HashMap<String, Box<dyn Fn(usize) -> f32>> = HashMap::new();

        // Reconstruction: High early, decrease
        schedules.insert("reconstruction".to_string(), Box::new(|e| {
            1.0 * (-e as f32 / 50.0).exp()
        }));

        // Contrastive: Low early, ramp up
        schedules.insert("contrastive".to_string(), Box::new(|e| {
            if e < 10 { 0.1 + 0.9 * e as f32 / 10.0 } else { 1.0 }
        }));

        // Task: Low early, ramp up late
        schedules.insert("task".to_string(), Box::new(|e| {
            if e < 50 { 0.1 } else { 0.1 + 0.9 * (e - 50) as f32 / 50.0 }
        }));

        Self { current_epoch: 0, schedules }
    }

    pub fn get_weight(&self, loss_name: &str) -> f32 {
        self.schedules.get(loss_name)
            .map(|f| f(self.current_epoch))
            .unwrap_or(1.0)
    }

    pub fn step(&mut self) {
        self.current_epoch += 1;
    }
}
```

**Training Loop Integration**:
```rust
// Main training loop
let mut curriculum = CurriculumSchedule::new_default();
let mut loss_manager = MultiObjectiveLoss::default();

for epoch in 0..num_epochs {
    // Get curriculum weights
    let lambda_recon = curriculum.get_weight("reconstruction");
    let lambda_contrast = curriculum.get_weight("contrastive");
    let lambda_task = curriculum.get_weight("task");

    // Update loss manager
    loss_manager.lambda_reconstruction = lambda_recon;
    loss_manager.lambda_contrastive = lambda_contrast;
    loss_manager.lambda_task = lambda_task;

    // Optionally auto-balance
    if epoch % 10 == 0 {
        loss_manager.auto_balance(&current_losses);
    }

    // Train
    let total_loss = loss_manager.compute_total(&losses);

    curriculum.step();
}
```

**Testing**:
- Convergence speed with/without curriculum
- Final performance comparison
- Hyperparameter sensitivity

**Deliverable**: Automated training orchestration

---

## 4. Q2: Advanced Attention Mechanisms (Months 4-6)

### 4.1 Month 4: Sparse Attention (Local + Global) (P1)

**Goal**: Reduce O(n²) to O(k_local + k_global)

**File**: `crates/ruvector-gnn/src/layer.rs`

**New Struct**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparseGraphAttention {
    local_attn: MultiHeadAttention,
    global_attn: MultiHeadAttention,
    gating: Linear,  // Learn how to combine local + global
}

impl SparseGraphAttention {
    pub fn forward(
        &self,
        query: &[f32],
        all_neighbors: &[Vec<f32>],
        neighbor_layers: &[usize],  // HNSW layer for each neighbor
    ) -> Vec<f32> {
        // Split by HNSW layer
        let (local_neighbors, global_neighbors) = split_by_layer(
            all_neighbors,
            neighbor_layers,
        );

        // Local attention (layer 0)
        let local_out = if !local_neighbors.is_empty() {
            self.local_attn.forward(query, &local_neighbors, &local_neighbors)
        } else {
            vec![0.0; query.len()]
        };

        // Global attention (higher layers)
        let global_out = if !global_neighbors.is_empty() {
            self.global_attn.forward(query, &global_neighbors, &global_neighbors)
        } else {
            vec![0.0; query.len()]
        };

        // Learned gating
        let gate_input = [query, &local_out[..], &global_out[..]].concat();
        let gate = sigmoid_vec(&self.gating.forward(&gate_input));

        // Combine: output = gate ⊙ local + (1-gate) ⊙ global
        local_out.iter().zip(global_out.iter()).zip(gate.iter())
            .map(|((&l, &g), &alpha)| alpha * l + (1.0 - alpha) * g)
            .collect()
    }
}
```

**Testing**:
- Complexity measurement (FLOPs, memory)
- Quality comparison vs. full attention
- Scalability to large graphs (10K+ nodes)

**Deliverable**: Sparse attention variant

---

### 4.2 Month 5: Rotary Position Embeddings (RoPE) (P1)

**Goal**: Encode distances without explicit features

**File**: `crates/ruvector-gnn/src/layer.rs`

**New Module**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRoPE {
    dim: usize,
    base: f32,  // Frequency base (default 10000)
}

impl GraphRoPE {
    pub fn apply_rotation(
        &self,
        embedding: &[f32],
        distance: f32,  // Graph distance or edge weight
    ) -> Vec<f32> {
        let mut rotated = vec![0.0; self.dim];

        for i in (0..self.dim).step_by(2) {
            let theta = distance / self.base.powf(2.0 * i as f32 / self.dim as f32);
            let cos_theta = theta.cos();
            let sin_theta = theta.sin();

            rotated[i] = embedding[i] * cos_theta - embedding[i+1] * sin_theta;
            rotated[i+1] = embedding[i] * sin_theta + embedding[i+1] * cos_theta;
        }

        rotated
    }

    pub fn forward_attention(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
        distances: &[f32],  // NEW: distances to encode
    ) -> Vec<f32> {
        let q_rotated = self.apply_rotation(query, 0.0);

        let scores: Vec<f32> = keys.iter()
            .zip(distances.iter())
            .map(|(k, &dist)| {
                let k_rotated = self.apply_rotation(k, dist);
                dot_product(&q_rotated, &k_rotated)
            })
            .collect();

        let weights = softmax(&scores);
        weighted_sum(values, &weights)
    }
}
```

**Integration**:
- Add to `RuvectorLayer` as optional module
- Use HNSW layer index or edge weight as distance

**Testing**:
- Ablation: With/without RoPE
- Sensitivity to base frequency
- Performance on HNSW navigation

**Deliverable**: RoPE-enhanced attention

---

### 4.3 Month 6: Flash Attention (Memory Optimization) (P2)

**Goal**: O(n) memory attention

**File**: `crates/ruvector-gnn/src/layer.rs`

**Simplified Tiled Attention**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TiledAttention {
    block_size: usize,
}

impl TiledAttention {
    pub fn forward_tiled(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Vec<f32> {
        let n = keys.len();
        let mut output = vec![0.0; query.len()];
        let mut row_sum = 0.0;
        let mut row_max = f32::NEG_INFINITY;

        // Process keys in blocks
        for chunk_start in (0..n).step_by(self.block_size) {
            let chunk_end = (chunk_start + self.block_size).min(n);
            let chunk_keys = &keys[chunk_start..chunk_end];
            let chunk_values = &values[chunk_start..chunk_end];

            // Compute scores for this block
            let scores: Vec<f32> = chunk_keys.iter()
                .map(|k| dot_product(query, k))
                .collect();

            // Online softmax update
            let new_max = scores.iter().copied().fold(row_max, f32::max);
            let exp_scores: Vec<f32> = scores.iter()
                .map(|&s| (s - new_max).exp())
                .collect();

            let correction = (row_max - new_max).exp();
            row_sum = row_sum * correction + exp_scores.iter().sum::<f32>();
            row_max = new_max;

            // Accumulate weighted values
            for (v, &weight) in chunk_values.iter().zip(exp_scores.iter()) {
                for (o, &v_i) in output.iter_mut().zip(v.iter()) {
                    *o = *o * correction + weight * v_i;
                }
            }
        }

        // Final normalization
        output.iter().map(|&o| o / row_sum).collect()
    }
}
```

**Testing**:
- Memory profiling (peak usage)
- Speed comparison (small vs. large neighborhoods)
- Numerical accuracy vs. standard attention

**Deliverable**: Memory-efficient attention option

---

## 5. Q3: Geometric and Hierarchical Enhancements (Months 7-9)

### 5.1 Month 7-8: Hyperbolic Embeddings (P2)

**Goal**: Hyperbolic geometry for HNSW higher layers

**File**: New file `crates/ruvector-gnn/src/hyperbolic.rs`

**Core Operations**:
```rust
pub struct HyperbolicOps {
    curvature: f32,  // Negative curvature
}

impl HyperbolicOps {
    // See advanced-architectures.md for full implementation
    pub fn mobius_add(&self, x: &[f32], y: &[f32]) -> Vec<f32> { ... }
    pub fn exp_map(&self, x: &[f32], v: &[f32]) -> Vec<f32> { ... }
    pub fn log_map(&self, x: &[f32], y: &[f32]) -> Vec<f32> { ... }
    pub fn distance(&self, x: &[f32], y: &[f32]) -> f32 { ... }
}

pub struct HyperbolicLayer {
    weight: Array2<f32>,
    bias: Vec<f32>,
    ops: HyperbolicOps,
}

impl HyperbolicLayer {
    pub fn forward(
        &self,
        node_embedding: &[f32],
        neighbor_embeddings: &[Vec<f32>],
    ) -> Vec<f32> {
        // Aggregate in hyperbolic space
        let aggregated = self.ops.einstein_midpoint(neighbor_embeddings);

        // Hyperbolic linear transformation
        self.hyperbolic_linear(&aggregated)
    }
}
```

**Integration**:
```rust
// Dual-geometry RuvectorLayer
pub enum RuvectorLayerVariant {
    Euclidean(RuvectorLayer),
    Hyperbolic(HyperbolicLayer),
}

pub struct HierarchicalRuvector {
    layer_0: RuvectorLayerVariant,  // Euclidean (local)
    layer_1_plus: RuvectorLayerVariant,  // Hyperbolic (hierarchical)
}
```

**Testing**:
- Tree embedding distortion
- HNSW navigation quality
- Numerical stability (boundary issues)

**Deliverable**: Hyperbolic layer variant

---

### 5.2 Month 9: Hierarchical Multi-Scale Embeddings (P2)

**Goal**: Different embeddings per HNSW layer

**File**: `crates/ruvector-gnn/src/hierarchical.rs`

**New Struct**:
```rust
pub struct HierarchicalEmbedding {
    embeddings_by_layer: HashMap<usize, Vec<f32>>,
}

impl HierarchicalEmbedding {
    pub fn get_embedding(&self, layer: usize) -> &Vec<f32> {
        self.embeddings_by_layer.get(&layer)
            .expect("Node not in this layer")
    }

    pub fn interpolate(&self, layer: f32) -> Vec<f32> {
        let low = layer.floor() as usize;
        let high = layer.ceil() as usize;

        if low == high {
            return self.get_embedding(low).clone();
        }

        let alpha = layer - low as f32;
        let emb_low = self.get_embedding(low);
        let emb_high = self.get_embedding(high);

        // Linear interpolation
        emb_low.iter().zip(emb_high.iter())
            .map(|(&l, &h)| (1.0 - alpha) * l + alpha * h)
            .collect()
    }
}

pub struct HierarchicalGNN {
    gnn_layers_by_hnsw_level: Vec<RuvectorLayer>,
}

impl HierarchicalGNN {
    pub fn forward_hierarchical(
        &self,
        node_features: &[f32],
        neighbors_by_layer: &HashMap<usize, Vec<Vec<f32>>>,
    ) -> HierarchicalEmbedding {
        let mut embeddings_by_layer = HashMap::new();

        for (hnsw_layer, layer_neighbors) in neighbors_by_layer {
            let embedding = self.gnn_layers_by_hnsw_level[*hnsw_layer].forward(
                node_features,
                layer_neighbors,
                &vec![1.0; layer_neighbors.len()],
            );

            embeddings_by_layer.insert(*hnsw_layer, embedding);
        }

        HierarchicalEmbedding { embeddings_by_layer }
    }
}
```

**Testing**:
- Multi-scale consistency
- Interpolation quality
- Search performance at different layers

**Deliverable**: Hierarchical embedding system

---

## 6. Q4: Advanced Architectures (Months 10-12)

### 6.1 Month 10-11: GPS (Graph + Global Attention) (P2)

**Goal**: Combine message passing with global attention

**File**: `crates/ruvector-gnn/src/layer.rs`

**Implementation**: See `advanced-architectures.md` Section 1.3

**Testing**:
- Benchmark vs. pure message passing
- Scalability analysis
- Ablation: Local-only vs. Global-only vs. Both

**Deliverable**: GPS layer variant

---

### 6.2 Month 12: Neural ODE (Continuous Depth) (P3)

**Goal**: Adaptive depth GNN

**File**: New `crates/ruvector-gnn/src/neural_ode.rs`

**Implementation**: See `advanced-architectures.md` Section 3.1

**Testing**:
- Learned depth (optimal T)
- Memory efficiency (adjoint method)
- Performance vs. fixed depth

**Deliverable**: Neural ODE variant

---

## 7. Continuous Integration & Testing

### 7.1 Benchmarking Suite

**Datasets**:
1. **Synthetic Trees**: Test hierarchical properties
2. **Social Networks**: Community detection
3. **Biological Graphs**: Protein-protein interaction
4. **HNSW-specific**: Real vector search tasks

**Metrics**:
- Link prediction AUC-ROC
- Node classification accuracy
- Clustering modularity
- Search recall@K
- Training time, memory usage

**Benchmark Script** (`benchmarks/gnn_benchmark.rs`):
```rust
pub struct GNNBenchmark {
    datasets: Vec<Dataset>,
    models: Vec<ModelConfig>,
}

impl GNNBenchmark {
    pub fn run_full_suite(&self) -> BenchmarkResults {
        let mut results = BenchmarkResults::new();

        for dataset in &self.datasets {
            for model_config in &self.models {
                let model = build_model(model_config);

                // Train
                let start = Instant::now();
                train_model(&model, dataset);
                let train_time = start.elapsed();

                // Evaluate
                let metrics = evaluate_model(&model, dataset);

                results.add_entry(
                    dataset.name(),
                    model_config.name(),
                    metrics,
                    train_time,
                );
            }
        }

        results
    }
}
```

### 7.2 Regression Testing

**Tests to Run on Every PR**:
- All existing unit tests
- Performance regression (max 5% slowdown)
- Memory regression (max 10% increase)
- Accuracy floor (must maintain current AUC)

**CI Pipeline** (`.github/workflows/gnn-tests.yml`):
```yaml
name: GNN Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run unit tests
        run: cargo test --package ruvector-gnn
      - name: Run benchmarks
        run: cargo bench --package ruvector-gnn
      - name: Check regression
        run: python scripts/check_regression.py
```

---

## 8. Documentation & Knowledge Transfer

### 8.1 API Documentation

**For Each New Feature**:
- Rustdoc comments with examples
- Mathematical background
- Usage recommendations
- Performance characteristics

**Example**:
```rust
/// Edge-featured multi-head attention mechanism.
///
/// Extends standard scaled dot-product attention to incorporate edge features
/// into the attention score computation. This is particularly useful for graphs
/// where edges carry meaningful information (e.g., HNSW layer indices, distances).
///
/// # Mathematical Formulation
///
/// ```text
/// score(i, j) = (q_i · k_j) / √d_k + MLP(edge_features_{ij})
/// attention_weights = softmax(scores)
/// output = Σ_j attention_weights[j] · v_j
/// ```
///
/// # Example
///
/// ```
/// use ruvector_gnn::layer::EdgeFeaturedAttention;
///
/// let attention = EdgeFeaturedAttention::new(128, 4);
/// let edge_features = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
///
/// let output = attention.forward_with_edges(
///     &query,
///     &keys,
///     &values,
///     Some(&edge_features),
/// );
/// ```
///
/// # Performance
///
/// - Time: O(d · h² + e · h) where e = edge feature dimension
/// - Space: O(d · h + e · h)
pub struct EdgeFeaturedAttention { ... }
```

### 8.2 Research Summaries

**Quarterly Reports**:
- What was implemented
- Benchmarking results
- Lessons learned
- Next quarter priorities

**Format** (`docs/quarterly-reports/Q1-2025.md`):
```markdown
# Q1 2025 GNN Research Report

## Summary
Implemented edge features, hard negatives, spectral regularization, and structural encoding.

## Results
- Edge features: +3.2% link prediction AUC
- Hard negatives: 1.5x faster convergence
- Spectral reg: +2.1% community detection modularity
- Structural encoding: +1.8% node classification

## Challenges
- Hypergraph complexity higher than expected
- Numerical instability in hyperbolic near boundary

## Q2 Priorities
- Sparse attention
- RoPE
- Flash Attention
```

---

## 9. Risk Mitigation

### 9.1 Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Hyperbolic instability | High | Medium | Clipping, careful initialization |
| Flash Attention bugs | Medium | Low | Extensive unit tests, fallback |
| Performance regression | Medium | High | Continuous benchmarking |
| API breaking changes | Low | High | Semver, deprecation warnings |

### 9.2 Schedule Risks

**Buffer Time**: 20% buffer in each quarter for:
- Unexpected bugs
- Performance tuning
- Community feedback integration

**Fallback Plan**: If feature blocked, move to next priority

---

## 10. Success Metrics

### 10.1 Technical Metrics

**Performance**:
- Link prediction AUC: Target +5% over baseline
- Node classification: Target +3%
- Training speed: Max 2x slowdown for advanced features
- Memory usage: Max 1.5x increase

**Code Quality**:
- Test coverage: >90%
- Documentation coverage: 100% public APIs
- No regressions in CI

### 10.2 Research Metrics

**Publications/Reports**:
- 4 quarterly research reports
- 1 comprehensive year-end summary
- Blog posts for major milestones

**Community**:
- GitHub stars, forks
- Issue resolution time
- External contributions

---

## 11. Resource Allocation

### 11.1 Team Requirements

**Roles**:
- 1 Senior ML Researcher (architecture design)
- 2 Software Engineers (implementation)
- 1 DevOps Engineer (CI/CD, benchmarking)

**Time Allocation**:
- 60% implementation
- 20% testing & benchmarking
- 10% documentation
- 10% research & exploration

### 11.2 Infrastructure

**Compute**:
- GPU server for training (NVIDIA A100 or equivalent)
- CPU cluster for large-scale benchmarks
- CI/CD runners

**Storage**:
- Datasets: 100 GB
- Model checkpoints: 50 GB
- Benchmark results: 10 GB

---

## 12. Next Steps (Week 1 Actions)

### Immediate Actions

**1. Set Up Infrastructure**:
- [ ] Provision benchmark server
- [ ] Set up CI/CD pipeline
- [ ] Create benchmark datasets

**2. Baseline Measurement**:
- [ ] Run full benchmark suite on current codebase
- [ ] Document baseline metrics
- [ ] Identify performance bottlenecks

**3. Begin P0 Implementation**:
- [ ] Edge feature support (Week 1-2)
- [ ] Hard negative sampling (Week 3-4)

**4. Team Alignment**:
- [ ] Roadmap review meeting
- [ ] Assign ownership of Q1 tasks
- [ ] Set up weekly progress meetings

---

## Appendix A: File Structure

```
crates/ruvector-gnn/src/
├── layer.rs                    # Core GNN layers (MODIFIED)
├── search.rs                   # Differentiable search
├── training.rs                 # Loss functions (MODIFIED)
├── ewc.rs                      # Continual learning
├── hyperbolic.rs              # NEW: Hyperbolic operations
├── hierarchical.rs            # NEW: Multi-scale embeddings
├── neural_ode.rs              # NEW: Continuous depth
├── attention/
│   ├── mod.rs
│   ├── edge_featured.rs       # NEW: Edge-featured attention
│   ├── sparse.rs              # NEW: Sparse attention
│   ├── rope.rs                # NEW: Rotary position embeddings
│   └── flash.rs               # NEW: Flash attention
└── benchmarks/
    ├── gnn_benchmark.rs       # NEW: Benchmark suite
    └── datasets/              # NEW: Test datasets
```

---

## Appendix B: Configuration

**Feature Flags** (`Cargo.toml`):
```toml
[features]
default = ["euclidean"]
euclidean = []
hyperbolic = ["hyperbolic-ops"]
edge-features = ["edge-attention"]
sparse-attention = ["sparse-attn"]
flash-attention = ["flash-attn"]
neural-ode = ["ode-solver"]
```

**Runtime Config** (`config/gnn_config.toml`):
```toml
[architecture]
layer_type = "ruvector"  # or "hyperbolic", "gps", "neural_ode"
num_layers = 3
hidden_dim = 256
num_heads = 8

[attention]
type = "multi_head"  # or "edge_featured", "sparse", "flash"
use_rope = false

[training]
use_hard_negatives = true
hard_negative_strategy = "distance"  # or "degree", "mixed"
use_spectral_reg = true
lambda_spectral = 0.01

[curriculum]
enabled = true
schedule = "default"  # or "custom"
```

---

**Document Version**: 1.0
**Last Updated**: 2025-11-30
**Author**: RuVector Research Team
**Review Date**: Every Quarter
