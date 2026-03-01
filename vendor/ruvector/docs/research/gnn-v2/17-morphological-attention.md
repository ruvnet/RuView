# Feature 17: Morphological Attention

## Overview

### Problem Statement
Traditional attention mechanisms use fixed attention patterns regardless of query characteristics. However, different query types benefit from different attention "shapes": precise queries need sharp, focused attention; exploratory queries need broad, diffuse attention; hierarchical queries need multi-scale attention. The mismatch between query intent and attention pattern leads to suboptimal retrieval.

### Proposed Solution
Morphological Attention dynamically adapts the shape and spread of attention based on query context. The system classifies queries into categories (focused, diffuse, hierarchical, radial) and morphs the attention pattern accordingly. This includes adjusting temperature, kernel shapes, neighborhood sizes, and aggregation strategies on-the-fly.

### Expected Benefits
- **Adaptive Retrieval**: 25-35% improvement in retrieval quality across diverse query types
- **Query-Aware Precision**: Sharp attention for precise queries (90%+ precision)
- **Exploration Support**: Broad attention for discovery queries (3-5x more diverse results)
- **Hierarchical Queries**: Multi-scale attention for taxonomic queries (40% better hierarchy preservation)
- **Computational Efficiency**: Sparse attention for focused mode saves 50-70% computation

### Novelty Claim
**Unique Contribution**: First GNN attention mechanism with dynamic morphological adaptation based on query semantics. Unlike fixed attention patterns or simple temperature scaling, Morphological Attention implements four distinct attention geometries with smooth transitions and query-conditioned shape parameters.

**Differentiators**:
1. Four distinct attention morphologies with semantic meaning
2. Query-conditioned shape parameter learning
3. Smooth morphological transitions (blending)
4. Hierarchical distance-aware attention
5. Interpretable attention visualization

## Technical Design

### Architecture Diagram

```
                    Input Query (q)
                         |
         +---------------+--------------+
         |                              |
    Feature Extract              Context Encode
         |                              |
         v                              v
    Query Features              Morphology Classifier
    (semantics,                       |
     specificity)              +------+------+------+------+
         |                     |      |      |      |      |
         |                  Focused Diffuse Hier  Radial  |
         |                     |      |      |      |      |
         |                     +------+------+------+------+
         |                              |
         |                     Morphology Weights
         |                        (softmax)
         |                              |
         +---------------+--------------+
                         |
                  Morphology Params
                  (shape, spread, etc.)
                         |
         +---------------+--------------+
         |               |              |
    Focused Mode   Diffuse Mode   Hier Mode   Radial Mode
         |               |              |           |
         v               v              v           v
    ┌────────┐     ┌────────┐     ┌────────┐  ┌────────┐
    │ Sharp  │     │ Broad  │     │Multi-  │  │Distance│
    │Gaussian│     │Uniform │     │Scale   │  │-Based  │
    └───┬────┘     └───┬────┘     └───┬────┘  └───┬────┘
        │              │              │           │
        +------+-------+------+-------+           │
               |              |                   │
          Blend Weights   Kernel Mix              │
               |              |                   │
               v              v                   v
          Attention       Attention          Attention
          Kernel 1        Kernel 2           Kernel 3
               |              |                   |
               +------+-------+-------------------+
                      |
               Morphed Attention
                      |
                      v
              Apply to Keys/Values
                      |
                      v
              Weighted Aggregation
                      |
                      v
                  Top-k Results


Morphology Modes Detail:

1. FOCUSED (Sharp Gaussian):

   Attention Weight
        ^
      1 |     *
        |    ***
        |   *****
        |  *******
      0 +─────────────> Distance from Query

   σ = 0.1-0.3 (narrow)
   Top-k = small (5-10)

2. DIFFUSE (Broad/Uniform):

   Attention Weight
        ^
      1 |─────────────
        |─────────────
        |─────────────
      0.5─────────────
        |
      0 +─────────────> Distance from Query

   σ = 1.0-2.0 (wide)
   Top-k = large (50-100)

3. HIERARCHICAL (Multi-Scale):

   Attention Weight
        ^
      1 |  *
        | ***    *
        |***** *** *
        |*********** ***
      0 +─────────────> Graph Distance

   Multiple scales (local, mid, global)
   Combine via learned weights

4. RADIAL (Distance-Based):

   Attention Weight
        ^
      1 |*
        | *
        |  *
        |   **
      0 |    ******───> Distance Threshold
        +─────────────> Euclidean Distance
```

### Core Data Structures

```rust
/// Configuration for Morphological Attention
#[derive(Debug, Clone)]
pub struct MorphologicalConfig {
    /// Base embedding dimension
    pub embed_dim: usize,

    /// Enable all morphology modes
    pub enable_focused: bool,
    pub enable_diffuse: bool,
    pub enable_hierarchical: bool,
    pub enable_radial: bool,

    /// Morphology classification
    pub classifier_hidden_dim: usize,

    /// Smooth transition between modes
    pub blend_modes: bool,

    /// Morphology-specific parameters
    pub focused_params: FocusedParams,
    pub diffuse_params: DiffuseParams,
    pub hierarchical_params: HierarchicalParams,
    pub radial_params: RadialParams,

    /// Learning rate for morphology adaptation
    pub adaptation_lr: f32,
}

/// Morphology type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MorphologyMode {
    /// Sharp, focused attention (high precision)
    Focused,

    /// Broad, exploratory attention (high recall)
    Diffuse,

    /// Multi-scale hierarchical attention
    Hierarchical,

    /// Distance-threshold based attention
    Radial,

    /// Blended combination of modes
    Blended,
}

/// Parameters for focused attention mode
#[derive(Debug, Clone)]
pub struct FocusedParams {
    /// Gaussian sigma (narrow)
    pub sigma: f32,

    /// Top-k results
    pub top_k: usize,

    /// Sharpness temperature
    pub temperature: f32,

    /// Enable sparse computation
    pub sparse: bool,
}

impl Default for FocusedParams {
    fn default() -> Self {
        Self {
            sigma: 0.2,
            top_k: 10,
            temperature: 0.1,
            sparse: true,
        }
    }
}

/// Parameters for diffuse attention mode
#[derive(Debug, Clone)]
pub struct DiffuseParams {
    /// Gaussian sigma (wide)
    pub sigma: f32,

    /// Top-k results
    pub top_k: usize,

    /// Minimum attention weight threshold
    pub min_weight: f32,

    /// Diversity penalty
    pub diversity_weight: f32,
}

impl Default for DiffuseParams {
    fn default() -> Self {
        Self {
            sigma: 1.5,
            top_k: 50,
            min_weight: 0.01,
            diversity_weight: 0.1,
        }
    }
}

/// Parameters for hierarchical attention mode
#[derive(Debug, Clone)]
pub struct HierarchicalParams {
    /// Number of scales
    pub num_scales: usize,

    /// Scale factors (e.g., [1.0, 2.0, 4.0])
    pub scale_factors: Vec<f32>,

    /// Weights for each scale (learned)
    pub scale_weights: Vec<f32>,

    /// Maximum graph distance per scale
    pub max_distances: Vec<usize>,
}

impl Default for HierarchicalParams {
    fn default() -> Self {
        Self {
            num_scales: 3,
            scale_factors: vec![1.0, 2.0, 4.0],
            scale_weights: vec![0.5, 0.3, 0.2],
            max_distances: vec![1, 3, 10],
        }
    }
}

/// Parameters for radial attention mode
#[derive(Debug, Clone)]
pub struct RadialParams {
    /// Distance threshold
    pub distance_threshold: f32,

    /// Falloff rate beyond threshold
    pub falloff_rate: f32,

    /// Use Euclidean vs. cosine distance
    pub distance_metric: DistanceMetric,

    /// Top-k results
    pub top_k: usize,
}

impl Default for RadialParams {
    fn default() -> Self {
        Self {
            distance_threshold: 0.5,
            falloff_rate: 2.0,
            distance_metric: DistanceMetric::Cosine,
            top_k: 20,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DistanceMetric {
    Euclidean,
    Cosine,
    Manhattan,
}

/// Query feature extractor for morphology classification
#[derive(Debug)]
pub struct QueryFeatureExtractor {
    /// Extract semantic features
    pub semantic_encoder: DenseLayer,

    /// Compute query specificity
    pub specificity_encoder: DenseLayer,

    /// Combine features
    pub fusion_layer: DenseLayer,

    /// Feature dimension
    pub feature_dim: usize,
}

/// Morphology classifier
#[derive(Debug)]
pub struct MorphologyClassifier {
    /// Input: query features
    pub input_dim: usize,

    /// Hidden layers
    pub hidden_layers: Vec<DenseLayer>,

    /// Output: morphology probabilities
    pub output_layer: DenseLayer,

    /// Activation
    pub activation: ActivationType,
}

/// Morphology-specific attention kernel
pub trait AttentionKernel: Send + Sync {
    /// Compute attention weights
    fn compute_weights(
        &self,
        query: &[f32],
        keys: &[[f32]],
        params: &MorphologyParams
    ) -> Vec<f32>;

    /// Get kernel type
    fn kernel_type(&self) -> MorphologyMode;
}

/// Focused attention kernel (sharp Gaussian)
#[derive(Debug)]
pub struct FocusedKernel {
    params: FocusedParams,
}

impl AttentionKernel for FocusedKernel {
    fn compute_weights(
        &self,
        query: &[f32],
        keys: &[[f32]],
        _params: &MorphologyParams
    ) -> Vec<f32> {
        // Compute distances
        let distances: Vec<f32> = keys.iter()
            .map(|key| cosine_distance(query, key))
            .collect();

        // Apply sharp Gaussian
        distances.iter()
            .map(|&d| {
                let exp_term = -(d * d) / (2.0 * self.params.sigma * self.params.sigma);
                exp_term.exp() / self.params.temperature
            })
            .collect()
    }

    fn kernel_type(&self) -> MorphologyMode {
        MorphologyMode::Focused
    }
}

/// Diffuse attention kernel (broad/uniform)
#[derive(Debug)]
pub struct DiffuseKernel {
    params: DiffuseParams,
}

impl AttentionKernel for DiffuseKernel {
    fn compute_weights(
        &self,
        query: &[f32],
        keys: &[[f32]],
        _params: &MorphologyParams
    ) -> Vec<f32> {
        // Compute distances
        let distances: Vec<f32> = keys.iter()
            .map(|key| cosine_distance(query, key))
            .collect();

        // Apply broad Gaussian
        let mut weights: Vec<f32> = distances.iter()
            .map(|&d| {
                let exp_term = -(d * d) / (2.0 * self.params.sigma * self.params.sigma);
                exp_term.exp()
            })
            .collect();

        // Apply diversity penalty (reduce weight of similar items)
        if self.params.diversity_weight > 0.0 {
            weights = apply_diversity_penalty(&weights, keys, self.params.diversity_weight);
        }

        weights
    }

    fn kernel_type(&self) -> MorphologyMode {
        MorphologyMode::Diffuse
    }
}

/// Hierarchical attention kernel (multi-scale)
#[derive(Debug)]
pub struct HierarchicalKernel {
    params: HierarchicalParams,
}

impl AttentionKernel for HierarchicalKernel {
    fn compute_weights(
        &self,
        query: &[f32],
        keys: &[[f32]],
        params: &MorphologyParams
    ) -> Vec<f32> {
        let num_keys = keys.len();
        let mut combined_weights = vec![0.0; num_keys];

        // Compute attention at each scale
        for (scale_idx, &scale_factor) in self.params.scale_factors.iter().enumerate() {
            let sigma = scale_factor;
            let scale_weight = self.params.scale_weights[scale_idx];

            // Get graph distances if available
            let graph_distances = params.graph_distances.as_ref();
            let max_dist = self.params.max_distances[scale_idx];

            for (i, key) in keys.iter().enumerate() {
                // Check graph distance constraint
                if let Some(dists) = graph_distances {
                    if dists[i] > max_dist {
                        continue;
                    }
                }

                // Compute semantic distance
                let semantic_dist = cosine_distance(query, key);

                // Scale-specific Gaussian
                let weight = (-(semantic_dist * semantic_dist) / (2.0 * sigma * sigma)).exp();

                // Weighted combination
                combined_weights[i] += scale_weight * weight;
            }
        }

        combined_weights
    }

    fn kernel_type(&self) -> MorphologyMode {
        MorphologyMode::Hierarchical
    }
}

/// Radial attention kernel (distance threshold)
#[derive(Debug)]
pub struct RadialKernel {
    params: RadialParams,
}

impl AttentionKernel for RadialKernel {
    fn compute_weights(
        &self,
        query: &[f32],
        keys: &[[f32]],
        _params: &MorphologyParams
    ) -> Vec<f32> {
        keys.iter()
            .map(|key| {
                let dist = match self.params.distance_metric {
                    DistanceMetric::Euclidean => euclidean_distance(query, key),
                    DistanceMetric::Cosine => cosine_distance(query, key),
                    DistanceMetric::Manhattan => manhattan_distance(query, key),
                };

                if dist <= self.params.distance_threshold {
                    // Inside threshold: full weight
                    1.0
                } else {
                    // Outside threshold: exponential falloff
                    let excess = dist - self.params.distance_threshold;
                    (-self.params.falloff_rate * excess).exp()
                }
            })
            .collect()
    }

    fn kernel_type(&self) -> MorphologyMode {
        MorphologyMode::Radial
    }
}

/// Morphology-specific parameters passed to kernels
#[derive(Debug, Clone)]
pub struct MorphologyParams {
    /// Graph distances (for hierarchical mode)
    pub graph_distances: Option<Vec<usize>>,

    /// Query specificity score (0-1)
    pub specificity: f32,

    /// Additional metadata
    pub metadata: HashMap<String, f32>,
}

/// Main Morphological Attention layer
pub struct MorphologicalAttention {
    /// Configuration
    config: MorphologicalConfig,

    /// Query feature extractor
    feature_extractor: QueryFeatureExtractor,

    /// Morphology classifier
    classifier: MorphologyClassifier,

    /// Attention kernels
    kernels: HashMap<MorphologyMode, Box<dyn AttentionKernel>>,

    /// Metrics
    metrics: MorphologyMetrics,
}

#[derive(Debug, Default)]
pub struct MorphologyMetrics {
    /// Mode usage counts
    pub mode_counts: HashMap<MorphologyMode, usize>,

    /// Average attention entropy per mode
    pub avg_entropy: HashMap<MorphologyMode, f32>,

    /// Query latency per mode
    pub avg_latency_ms: HashMap<MorphologyMode, f32>,

    /// Retrieval precision per mode
    pub precision: HashMap<MorphologyMode, f32>,
}
```

### Key Algorithms

#### 1. Morphological Attention Forward Pass

```rust
/// Forward pass with morphological adaptation
fn forward(
    &mut self,
    query: &[f32],
    keys: &[[f32]],
    values: &[[f32]],
    k: usize,
    graph_distances: Option<Vec<usize>>
) -> Result<(Vec<usize>, Vec<f32>), MorphError> {

    let start_time = Instant::now();

    // Step 1: Extract query features
    let query_features = self.feature_extractor.extract(query);

    // Step 2: Classify morphology
    let morphology_probs = self.classifier.classify(&query_features);

    // Step 3: Determine active mode(s)
    let (active_mode, blend_weights) = if self.config.blend_modes {
        // Blend multiple modes
        (MorphologyMode::Blended, morphology_probs)
    } else {
        // Select single mode (argmax)
        let max_idx = argmax(&morphology_probs);
        let mode = index_to_mode(max_idx);
        let mut weights = vec![0.0; morphology_probs.len()];
        weights[max_idx] = 1.0;
        (mode, weights)
    };

    // Step 4: Prepare morphology parameters
    let specificity = compute_query_specificity(query, &query_features);
    let morph_params = MorphologyParams {
        graph_distances,
        specificity,
        metadata: HashMap::new(),
    };

    // Step 5: Compute attention weights
    let attention_weights = if active_mode == MorphologyMode::Blended {
        // Blend multiple kernels
        self.compute_blended_attention(
            query,
            keys,
            &blend_weights,
            &morph_params
        )
    } else {
        // Single kernel
        let kernel = self.kernels.get(&active_mode).unwrap();
        kernel.compute_weights(query, keys, &morph_params)
    };

    // Step 6: Apply softmax normalization
    let normalized_weights = softmax(&attention_weights);

    // Step 7: Select top-k based on mode
    let top_k = self.get_mode_top_k(active_mode);
    let top_k = top_k.min(k);

    let mut indexed_weights: Vec<(usize, f32)> = normalized_weights
        .iter()
        .enumerate()
        .map(|(i, &w)| (i, w))
        .collect();

    indexed_weights.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    indexed_weights.truncate(top_k);

    let top_indices: Vec<usize> = indexed_weights.iter().map(|&(i, _)| i).collect();
    let top_scores: Vec<f32> = indexed_weights.iter().map(|&(_, w)| w).collect();

    // Step 8: Update metrics
    self.update_metrics(active_mode, &attention_weights, start_time.elapsed());

    Ok((top_indices, top_scores))
}

/// Compute blended attention from multiple kernels
fn compute_blended_attention(
    &self,
    query: &[f32],
    keys: &[[f32]],
    blend_weights: &[f32],
    params: &MorphologyParams
) -> Vec<f32> {

    let num_keys = keys.len();
    let mut blended = vec![0.0; num_keys];

    // Weighted combination of all kernels
    let modes = vec![
        MorphologyMode::Focused,
        MorphologyMode::Diffuse,
        MorphologyMode::Hierarchical,
        MorphologyMode::Radial,
    ];

    for (mode, &weight) in modes.iter().zip(blend_weights.iter()) {
        if weight < 0.01 {
            continue;  // Skip negligible weights
        }

        if let Some(kernel) = self.kernels.get(mode) {
            let kernel_weights = kernel.compute_weights(query, keys, params);

            for i in 0..num_keys {
                blended[i] += weight * kernel_weights[i];
            }
        }
    }

    blended
}

/// Get top-k parameter based on morphology mode
fn get_mode_top_k(&self, mode: MorphologyMode) -> usize {
    match mode {
        MorphologyMode::Focused => self.config.focused_params.top_k,
        MorphologyMode::Diffuse => self.config.diffuse_params.top_k,
        MorphologyMode::Hierarchical => 30,  // Medium
        MorphologyMode::Radial => self.config.radial_params.top_k,
        MorphologyMode::Blended => 20,  // Default
    }
}
```

#### 2. Query Feature Extraction

```rust
/// Extract features from query for morphology classification
fn extract_query_features(query: &[f32]) -> QueryFeatures {

    // Semantic features
    let semantic = semantic_encoder.encode(query);

    // Specificity features
    let specificity = compute_specificity_features(query);

    // Statistical features
    let stats = QueryStats {
        mean: query.iter().sum::<f32>() / query.len() as f32,
        std: compute_std(query),
        sparsity: query.iter().filter(|&&x| x.abs() < 0.01).count() as f32 / query.len() as f32,
        max_val: query.iter().copied().fold(f32::NEG_INFINITY, f32::max),
    };

    QueryFeatures {
        semantic,
        specificity,
        stats,
    }
}

/// Compute query specificity score (0 = broad, 1 = focused)
fn compute_query_specificity(query: &[f32], features: &QueryFeatures) -> f32 {

    // High specificity indicators:
    // - High variance (peaked distribution)
    // - High sparsity (few active dimensions)
    // - High max value

    let variance_score = features.stats.std / (features.stats.mean + 1e-6);
    let sparsity_score = features.specificity.sparsity;
    let peak_score = features.stats.max_val / (features.stats.mean + 1e-6);

    // Weighted combination
    let specificity = 0.4 * variance_score.min(1.0) +
                     0.3 * sparsity_score +
                     0.3 * peak_score.min(1.0);

    specificity.max(0.0).min(1.0)
}

struct QueryFeatures {
    semantic: Vec<f32>,
    specificity: SpecificityFeatures,
    stats: QueryStats,
}

struct SpecificityFeatures {
    sparsity: f32,
    entropy: f32,
    peak_ratio: f32,
}

struct QueryStats {
    mean: f32,
    std: f32,
    sparsity: f32,
    max_val: f32,
}
```

#### 3. Morphology Classification

```rust
/// Classify query into morphology modes
fn classify_morphology(
    &self,
    features: &QueryFeatures
) -> Vec<f32> {

    // Concatenate all features
    let mut feature_vec = features.semantic.clone();
    feature_vec.push(features.specificity.sparsity);
    feature_vec.push(features.specificity.entropy);
    feature_vec.push(features.specificity.peak_ratio);
    feature_vec.push(features.stats.mean);
    feature_vec.push(features.stats.std);
    feature_vec.push(features.stats.sparsity);

    let input = Array1::from(feature_vec);

    // Forward through classifier
    let mut hidden = input;
    for layer in &self.classifier.hidden_layers {
        hidden = layer.forward(&hidden);
        hidden = relu(&hidden);
    }

    // Output layer with softmax
    let logits = self.classifier.output_layer.forward(&hidden);
    let probs = softmax(&logits.to_vec());

    // probs[0] = Focused
    // probs[1] = Diffuse
    // probs[2] = Hierarchical
    // probs[3] = Radial

    probs
}

/// Classify based on heuristics (rule-based fallback)
fn heuristic_classify(specificity: f32) -> MorphologyMode {
    if specificity > 0.75 {
        MorphologyMode::Focused
    } else if specificity < 0.3 {
        MorphologyMode::Diffuse
    } else if specificity >= 0.5 {
        MorphologyMode::Radial
    } else {
        MorphologyMode::Hierarchical
    }
}
```

#### 4. Diversity Penalty for Diffuse Mode

```rust
/// Apply diversity penalty to encourage diverse results
fn apply_diversity_penalty(
    weights: &[f32],
    keys: &[[f32]],
    diversity_weight: f32
) -> Vec<f32> {

    let n = weights.len();
    let mut penalized = weights.to_vec();

    // Compute pairwise similarities
    for i in 0..n {
        for j in (i+1)..n {
            let similarity = cosine_similarity(&keys[i], &keys[j]);

            // Penalize both items proportional to similarity
            let penalty = diversity_weight * similarity * weights[i] * weights[j];

            penalized[i] -= penalty;
            penalized[j] -= penalty;
        }
    }

    // Ensure non-negative
    for w in &mut penalized {
        *w = w.max(0.0);
    }

    penalized
}
```

### API Design

```rust
/// Public API for Morphological Attention
pub trait MorphologicalLayer {
    /// Create new morphological attention layer
    fn new(config: MorphologicalConfig) -> Self;

    /// Forward pass with automatic morphology selection
    fn forward(
        &mut self,
        query: &[f32],
        keys: &[[f32]],
        values: &[[f32]],
        k: usize
    ) -> Result<(Vec<usize>, Vec<f32>), MorphError>;

    /// Forward with explicit mode
    fn forward_with_mode(
        &mut self,
        query: &[f32],
        keys: &[[f32]],
        values: &[[f32]],
        k: usize,
        mode: MorphologyMode
    ) -> Result<(Vec<usize>, Vec<f32>), MorphError>;

    /// Get predicted morphology for query
    fn predict_morphology(&self, query: &[f32]) -> MorphologyMode;

    /// Get morphology probabilities
    fn predict_morphology_probs(&self, query: &[f32]) -> Vec<f32>;

    /// Update morphology parameters based on feedback
    fn update_parameters(
        &mut self,
        query: &[f32],
        feedback: &RetrievalFeedback
    );

    /// Get metrics
    fn get_metrics(&self) -> &MorphologyMetrics;

    /// Visualize attention pattern
    fn visualize_attention(
        &self,
        query: &[f32],
        keys: &[[f32]]
    ) -> AttentionVisualization;
}

#[derive(Debug)]
pub struct RetrievalFeedback {
    pub relevant_indices: Vec<usize>,
    pub irrelevant_indices: Vec<usize>,
    pub user_satisfaction: f32,
}

#[derive(Debug)]
pub struct AttentionVisualization {
    pub mode: MorphologyMode,
    pub weights: Vec<f32>,
    pub top_k_indices: Vec<usize>,
    pub morphology_shape: String,  // ASCII art or JSON
}

#[derive(Debug, thiserror::Error)]
pub enum MorphError {
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Computation error: {0}")]
    ComputationError(String),

    #[error("Feature extraction error: {0}")]
    FeatureError(String),
}
```

## Integration Points

### Affected Crates/Modules

1. **`ruvector-gnn-core/src/attention/`**
   - Add morphological attention as new attention type
   - Extend attention trait with morphology support

2. **`ruvector-graph/`**
   - Add graph distance computation for hierarchical mode

### New Modules to Create

1. **`ruvector-gnn-core/src/attention/morphological/`**
   ```
   morphological/
   ├── mod.rs
   ├── config.rs
   ├── features.rs       # Query feature extraction
   ├── classifier.rs     # Morphology classifier
   ├── kernels/
   │   ├── mod.rs
   │   ├── focused.rs
   │   ├── diffuse.rs
   │   ├── hierarchical.rs
   │   └── radial.rs
   ├── blend.rs          # Kernel blending
   ├── metrics.rs
   └── visualization.rs
   ```

### Dependencies on Other Features

- **Feature 15 (ESA)**: Each subspace can use different morphology
- **Feature 8 (Sparse Attention)**: Focused mode uses sparse computation
- **Feature 3 (Hierarchical)**: Hierarchical mode needs graph distances

## Implementation Phases

### Phase 1: Research & Prototyping (2 weeks)
- Design query feature extraction
- Prototype morphology classification
- Test kernel designs on benchmark datasets
- Validate morphology effectiveness

### Phase 2: Core Implementation (3 weeks)
- Implement all four attention kernels
- Implement feature extraction
- Implement morphology classifier
- Add kernel blending
- Unit tests

### Phase 3: Integration (2 weeks)
- Integrate with GNN attention framework
- Add graph distance support
- Optimize performance
- Integration tests

### Phase 4: Evaluation (1 week)
- Benchmark on diverse query types
- Measure precision/recall per mode
- User study for interpretability
- Production testing

## Success Metrics

| Metric | Target |
|--------|--------|
| Focused Mode Precision | >90% |
| Diffuse Mode Diversity | 3-5x vs. focused |
| Classification Accuracy | >85% |
| Latency Overhead | <20% vs. standard |
| User Satisfaction | >4.5/5 |

## Risks and Mitigations

1. **Risk: Classification Errors**
   - Mitigation: Blended modes, fallback heuristics

2. **Risk: Kernel Design Complexity**
   - Mitigation: Start with simple kernels, iterate

3. **Risk: Performance Overhead**
   - Mitigation: Sparse computation in focused mode, caching

4. **Risk: Limited Interpretability**
   - Mitigation: Visualization tools, clear mode descriptions
