# Agent 2: Hyperbolic Attention Implementation

**Agent**: Geometric Attention Specialist
**Status**: Implementation Ready
**Dependencies**: None
**Target Module**: `src/gnn/hyperbolic_attention.rs`

## Overview

Implement hyperbolic attention mechanisms using the Poincaré ball model to capture hierarchical relationships in latent space. This enables the model to learn both hierarchical structures (hyperbolic) and Euclidean features simultaneously.

## Mathematical Foundation

### Poincaré Ball Model

The Poincaré ball model is a conformal model of hyperbolic geometry defined as:

```
B^n_c = {x ∈ ℝ^n : ||x|| < 1/√c}
```

where `c > 0` is the curvature parameter. Key properties:
- **Boundary**: Points approach the boundary as ||x|| → 1/√c
- **Distance**: Grows exponentially near the boundary
- **Geodesics**: Circular arcs orthogonal to the boundary

## 1. Poincaré Ball Operations

### 1.1 Hyperbolic Distance

**Mathematical Formula**:
```
d_c(x, y) = (2/√c) * artanh(√c * ||⊖_c x ⊕_c y||)
```

where `⊖_c` is Möbius subtraction and `⊕_c` is Möbius addition.

**Implementation**:

```rust
use ndarray::{Array1, Array2};

/// Poincaré ball distance with numerical stability
///
/// Formula: d_c(x, y) = (2/√c) * artanh(√c * ||⊖_c x ⊕_c y||)
///
/// Numerical considerations:
/// - Clamp ||x|| and ||y|| to prevent boundary overflow
/// - Use artanh with epsilon for stability
/// - Handle c → 0 (Euclidean limit)
pub fn poincare_distance(
    x: &Array1<f32>,
    y: &Array1<f32>,
    curvature: f32,
) -> f32 {
    const EPSILON: f32 = 1e-7;
    const MAX_NORM: f32 = 0.9999; // Safety margin from boundary

    if curvature.abs() < EPSILON {
        // Euclidean limit: c → 0
        return ((x - y).mapv(|v| v * v).sum()).sqrt();
    }

    let sqrt_c = curvature.sqrt();
    let boundary = 1.0 / sqrt_c;

    // Clamp norms for numerical stability
    let x_clamped = clamp_to_ball(x, boundary * MAX_NORM);
    let y_clamped = clamp_to_ball(y, boundary * MAX_NORM);

    // Möbius subtraction: ⊖_c x ⊕_c y
    let diff = mobius_add(&mobius_negation(&x_clamped, curvature), &y_clamped, curvature);

    // ||diff||
    let norm = diff.mapv(|v| v * v).sum().sqrt();
    let norm_scaled = sqrt_c * norm;

    // artanh with clamping
    let norm_clamped = norm_scaled.min(1.0 - EPSILON);
    let artanh_val = 0.5 * ((1.0 + norm_clamped) / (1.0 - norm_clamped)).ln();

    (2.0 / sqrt_c) * artanh_val
}

/// Clamp vector to stay within Poincaré ball
fn clamp_to_ball(x: &Array1<f32>, max_norm: f32) -> Array1<f32> {
    let norm = x.mapv(|v| v * v).sum().sqrt();
    if norm > max_norm {
        x * (max_norm / norm)
    } else {
        x.clone()
    }
}
```

### 1.2 Möbius Addition

**Mathematical Formula**:
```
x ⊕_c y = [(1 + 2c⟨x,y⟩ + c||y||²)x + (1 - c||x||²)y] / [1 + 2c⟨x,y⟩ + c²||x||²||y||²]
```

**Implementation**:

```rust
/// Möbius addition in Poincaré ball
///
/// Formula: x ⊕_c y = [(1 + 2c⟨x,y⟩ + c||y||²)x + (1 - c||x||²)y] / D
/// where D = 1 + 2c⟨x,y⟩ + c²||x||²||y||²
///
/// Properties:
/// - Non-commutative: x ⊕ y ≠ y ⊕ x (in general)
/// - Origin identity: x ⊕ 0 = 0 ⊕ x = x
/// - Inverse: x ⊕ (⊖x) = 0
pub fn mobius_add(
    x: &Array1<f32>,
    y: &Array1<f32>,
    curvature: f32,
) -> Array1<f32> {
    const EPSILON: f32 = 1e-7;

    if curvature.abs() < EPSILON {
        // Euclidean limit
        return x + y;
    }

    let x_norm_sq = x.mapv(|v| v * v).sum();
    let y_norm_sq = y.mapv(|v| v * v).sum();
    let xy_dot = (x * y).sum();

    // Numerator terms
    let term1_coef = 1.0 + 2.0 * curvature * xy_dot + curvature * y_norm_sq;
    let term2_coef = 1.0 - curvature * x_norm_sq;

    let numerator = x * term1_coef + y * term2_coef;

    // Denominator with numerical stability
    let denominator = 1.0 + 2.0 * curvature * xy_dot
                     + curvature * curvature * x_norm_sq * y_norm_sq;
    let denominator_safe = denominator.max(EPSILON);

    numerator / denominator_safe
}

/// Möbius negation: ⊖_c x = -x
pub fn mobius_negation(x: &Array1<f32>, _curvature: f32) -> Array1<f32> {
    -x
}
```

### 1.3 Möbius Scalar Multiplication

**Mathematical Formula**:
```
r ⊗_c x = (1/√c) * tanh(r * artanh(√c * ||x||)) * (x / ||x||)
```

**Implementation**:

```rust
/// Möbius scalar multiplication in Poincaré ball
///
/// Formula: r ⊗_c x = (1/√c) * tanh(r * artanh(√c||x||)) * (x/||x||)
///
/// Properties:
/// - 0 ⊗ x = 0
/// - 1 ⊗ x = x
/// - (-1) ⊗ x = ⊖x
/// - (r + s) ⊗ x ≠ (r ⊗ x) ⊕ (s ⊗ x)
pub fn mobius_scalar_mult(
    scalar: f32,
    x: &Array1<f32>,
    curvature: f32,
) -> Array1<f32> {
    const EPSILON: f32 = 1e-7;

    let norm = x.mapv(|v| v * v).sum().sqrt();

    if norm < EPSILON {
        return Array1::zeros(x.len());
    }

    if curvature.abs() < EPSILON {
        // Euclidean limit
        return x * scalar;
    }

    let sqrt_c = curvature.sqrt();

    // artanh(√c * ||x||)
    let norm_scaled = (sqrt_c * norm).min(1.0 - EPSILON);
    let artanh_norm = 0.5 * ((1.0 + norm_scaled) / (1.0 - norm_scaled)).ln();

    // tanh(r * artanh(√c * ||x||))
    let scaled_artanh = scalar * artanh_norm;
    let tanh_val = scaled_artanh.tanh();

    // (1/√c) * tanh(...) * (x / ||x||)
    let result_norm = tanh_val / sqrt_c;
    let direction = x / norm;

    direction * result_norm
}
```

### 1.4 Exponential and Logarithmic Maps

**Mathematical Formulas**:

Exponential map (tangent space → manifold):
```
exp_x^c(v) = x ⊕_c [tanh(√c * λ_x^c * ||v|| / 2) * v / (√c * ||v||)]
where λ_x^c = 2 / (1 - c||x||²)
```

Logarithmic map (manifold → tangent space):
```
log_x^c(y) = (2 / √c * λ_x^c) * artanh(√c * ||⊖_c x ⊕_c y||) * [(⊖_c x ⊕_c y) / ||⊖_c x ⊕_c y||]
```

**Implementation**:

```rust
/// Exponential map: maps tangent vector at x to manifold point
///
/// exp_x^c(v) = x ⊕_c [tanh(√c * λ_x^c * ||v|| / 2) / (√c * ||v||)] * v
/// where λ_x^c = 2 / (1 - c||x||²) is the conformal factor
pub fn exp_map(
    x: &Array1<f32>,
    v: &Array1<f32>,
    curvature: f32,
) -> Array1<f32> {
    const EPSILON: f32 = 1e-7;

    let v_norm = v.mapv(|val| val * val).sum().sqrt();

    if v_norm < EPSILON {
        return x.clone();
    }

    if curvature.abs() < EPSILON {
        // Euclidean limit
        return x + v;
    }

    let sqrt_c = curvature.sqrt();
    let x_norm_sq = x.mapv(|val| val * val).sum();

    // Conformal factor: λ_x^c = 2 / (1 - c||x||²)
    let lambda_x = 2.0 / (1.0 - curvature * x_norm_sq).max(EPSILON);

    // tanh(√c * λ_x^c * ||v|| / 2)
    let tanh_arg = sqrt_c * lambda_x * v_norm / 2.0;
    let tanh_val = tanh_arg.tanh();

    // [tanh(...) / (√c * ||v||)] * v
    let transport_coef = tanh_val / (sqrt_c * v_norm);
    let transported = v * transport_coef;

    // x ⊕_c transported
    mobius_add(x, &transported, curvature)
}

/// Logarithmic map: maps manifold point to tangent vector at x
///
/// log_x^c(y) = (2 / √c * λ_x^c) * artanh(√c||diff||) * (diff / ||diff||)
/// where diff = ⊖_c x ⊕_c y
pub fn log_map(
    x: &Array1<f32>,
    y: &Array1<f32>,
    curvature: f32,
) -> Array1<f32> {
    const EPSILON: f32 = 1e-7;

    if curvature.abs() < EPSILON {
        // Euclidean limit
        return y - x;
    }

    let sqrt_c = curvature.sqrt();
    let x_norm_sq = x.mapv(|val| val * val).sum();

    // Conformal factor
    let lambda_x = 2.0 / (1.0 - curvature * x_norm_sq).max(EPSILON);

    // diff = ⊖_c x ⊕_c y
    let diff = mobius_add(&mobius_negation(x, curvature), y, curvature);
    let diff_norm = diff.mapv(|val| val * val).sum().sqrt();

    if diff_norm < EPSILON {
        return Array1::zeros(x.len());
    }

    // artanh(√c * ||diff||)
    let norm_scaled = (sqrt_c * diff_norm).min(1.0 - EPSILON);
    let artanh_val = 0.5 * ((1.0 + norm_scaled) / (1.0 - norm_scaled)).ln();

    // (2 / √c * λ_x^c) * artanh(...) * (diff / ||diff||)
    let coef = (2.0 / (sqrt_c * lambda_x)) * artanh_val;
    let direction = &diff / diff_norm;

    direction * coef
}
```

## 2. HyperbolicAttention Struct

### 2.1 Architecture

```rust
use ndarray::{Array1, Array2, Axis};

/// Hyperbolic attention mechanism for GNN layers
///
/// Architecture:
/// 1. Map node features to Poincaré ball via exp_map
/// 2. Compute hyperbolic distances for attention scores
/// 3. Apply softmax in tangent space
/// 4. Aggregate with Möbius addition
/// 5. Map back to Euclidean space if needed
pub struct HyperbolicAttention {
    /// Curvature parameter (c > 0)
    /// - Larger c → more hyperbolic (stronger hierarchy)
    /// - c → 0 → Euclidean limit
    pub curvature: f32,

    /// Query projection weights (dim_in × dim_out)
    pub w_query: Array2<f32>,

    /// Key projection weights (dim_in × dim_out)
    pub w_key: Array2<f32>,

    /// Value projection weights (dim_in × dim_out)
    pub w_value: Array2<f32>,

    /// Attention temperature for scaling
    pub temperature: f32,

    /// Number of attention heads
    pub num_heads: usize,

    /// Dimension per head
    pub dim_per_head: usize,
}

impl HyperbolicAttention {
    /// Create new hyperbolic attention layer
    pub fn new(
        dim_in: usize,
        dim_out: usize,
        curvature: f32,
        num_heads: usize,
    ) -> Self {
        assert!(curvature > 0.0, "Curvature must be positive");
        assert!(dim_out % num_heads == 0, "dim_out must be divisible by num_heads");

        let dim_per_head = dim_out / num_heads;

        Self {
            curvature,
            w_query: Array2::zeros((dim_in, dim_out)),
            w_key: Array2::zeros((dim_in, dim_out)),
            w_value: Array2::zeros((dim_in, dim_out)),
            temperature: (dim_per_head as f32).sqrt(),
            num_heads,
            dim_per_head,
        }
    }

    /// Initialize weights with Xavier/Glorot initialization
    ///
    /// For hyperbolic networks, we use smaller initialization
    /// to keep embeddings away from the boundary
    pub fn init_weights(&mut self, scale: f32) {
        use rand::Rng;
        use rand_distr::{Distribution, Normal};

        let mut rng = rand::thread_rng();

        // Xavier initialization with hyperbolic scaling
        let std_q = scale * (2.0 / (self.w_query.shape()[0] + self.w_query.shape()[1]) as f32).sqrt();
        let std_k = scale * (2.0 / (self.w_key.shape()[0] + self.w_key.shape()[1]) as f32).sqrt();
        let std_v = scale * (2.0 / (self.w_value.shape()[0] + self.w_value.shape()[1]) as f32).sqrt();

        let normal_q = Normal::new(0.0, std_q as f64).unwrap();
        let normal_k = Normal::new(0.0, std_k as f64).unwrap();
        let normal_v = Normal::new(0.0, std_v as f64).unwrap();

        for val in self.w_query.iter_mut() {
            *val = normal_q.sample(&mut rng) as f32;
        }
        for val in self.w_key.iter_mut() {
            *val = normal_k.sample(&mut rng) as f32;
        }
        for val in self.w_value.iter_mut() {
            *val = normal_v.sample(&mut rng) as f32;
        }
    }

    /// Forward pass with hyperbolic attention
    ///
    /// Input: node_features (num_nodes × dim_in)
    /// Output: attended_features (num_nodes × dim_out)
    ///
    /// Steps:
    /// 1. Project to Q, K, V
    /// 2. Map to Poincaré ball
    /// 3. Compute hyperbolic attention scores
    /// 4. Aggregate in hyperbolic space
    /// 5. Map back to tangent space
    pub fn forward(
        &self,
        node_features: &Array2<f32>,
        edge_index: &[(usize, usize)],
    ) -> Array2<f32> {
        let num_nodes = node_features.shape()[0];
        let dim_out = self.w_query.shape()[1];

        // 1. Project to Q, K, V
        let queries = node_features.dot(&self.w_query); // (num_nodes, dim_out)
        let keys = node_features.dot(&self.w_key);
        let values = node_features.dot(&self.w_value);

        // 2. Initialize output
        let mut output = Array2::zeros((num_nodes, dim_out));

        // 3. Map to Poincaré ball and compute attention for each node
        for target_node in 0..num_nodes {
            let query = queries.row(target_node).to_owned();

            // Get neighbors from edge_index
            let neighbors: Vec<usize> = edge_index
                .iter()
                .filter(|(_, dst)| *dst == target_node)
                .map(|(src, _)| *src)
                .collect();

            if neighbors.is_empty() {
                continue;
            }

            // Map query to Poincaré ball
            let origin = Array1::zeros(dim_out);
            let query_hyp = exp_map(&origin, &query, self.curvature);

            // 4. Compute attention scores using hyperbolic distances
            let mut attention_scores = Vec::with_capacity(neighbors.len());

            for &neighbor_idx in &neighbors {
                let key = keys.row(neighbor_idx).to_owned();
                let key_hyp = exp_map(&origin, &key, self.curvature);

                // Attention score = -distance (closer = higher score)
                let dist = poincare_distance(&query_hyp, &key_hyp, self.curvature);
                let score = -dist / self.temperature;
                attention_scores.push(score);
            }

            // 5. Softmax in tangent space
            let max_score = attention_scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let exp_scores: Vec<f32> = attention_scores
                .iter()
                .map(|s| (s - max_score).exp())
                .collect();
            let sum_exp: f32 = exp_scores.iter().sum();
            let attention_weights: Vec<f32> = exp_scores
                .iter()
                .map(|e| e / sum_exp)
                .collect();

            // 6. Aggregate values in hyperbolic space
            let mut aggregated = Array1::zeros(dim_out);

            for (i, &neighbor_idx) in neighbors.iter().enumerate() {
                let value = values.row(neighbor_idx).to_owned();
                let value_hyp = exp_map(&origin, &value, self.curvature);

                // Weighted Möbius addition
                let weighted_value = mobius_scalar_mult(
                    attention_weights[i],
                    &value_hyp,
                    self.curvature,
                );

                aggregated = mobius_add(&aggregated, &weighted_value, self.curvature);
            }

            // 7. Map back to tangent space
            let output_tangent = log_map(&origin, &aggregated, self.curvature);

            for (j, &val) in output_tangent.iter().enumerate() {
                output[[target_node, j]] = val;
            }
        }

        output
    }
}
```

### 2.2 Numerical Stability Enhancements

```rust
impl HyperbolicAttention {
    /// Adaptive curvature based on data distribution
    ///
    /// Automatically adjusts curvature to prevent boundary overflow
    /// while maintaining hyperbolic properties
    pub fn adaptive_curvature(
        &mut self,
        node_features: &Array2<f32>,
    ) {
        const SAFETY_MARGIN: f32 = 0.85;

        // Compute maximum feature norm
        let max_norm = node_features
            .axis_iter(Axis(0))
            .map(|row| row.mapv(|v| v * v).sum().sqrt())
            .fold(0.0f32, |acc, n| acc.max(n));

        // Adjust curvature: 1/√c should be > max_norm
        let min_boundary = max_norm / SAFETY_MARGIN;
        let max_curvature = 1.0 / (min_boundary * min_boundary);

        if self.curvature > max_curvature {
            self.curvature = max_curvature;
            eprintln!(
                "Warning: Adjusted curvature to {} to maintain numerical stability",
                self.curvature
            );
        }
    }

    /// Gradient clipping for hyperbolic parameters
    ///
    /// Prevents exploding gradients near the boundary
    pub fn clip_gradients(&mut self, max_grad_norm: f32) {
        // Placeholder for gradient clipping logic
        // In practice, this would be implemented in the training loop
        // with automatic differentiation framework
        let _ = max_grad_norm;
    }
}
```

## 3. Mixed-Curvature Attention

### 3.1 Product Space Architecture

```rust
/// Mixed-curvature attention combining Euclidean and hyperbolic spaces
///
/// Architecture:
/// - Euclidean subspace: captures local features, non-hierarchical relations
/// - Hyperbolic subspace: captures hierarchical structures, tree-like relations
///
/// The feature space is partitioned: dim_total = dim_euclidean + dim_hyperbolic
pub struct MixedCurvatureAttention {
    /// Euclidean attention for non-hierarchical features
    pub euclidean_attention: EuclideanAttention,

    /// Hyperbolic attention for hierarchical features
    pub hyperbolic_attention: HyperbolicAttention,

    /// Dimension split: (euclidean_dim, hyperbolic_dim)
    pub dim_split: (usize, usize),

    /// Learnable weight for combining outputs
    pub alpha: f32, // α ∈ [0, 1]: 0=fully Euclidean, 1=fully hyperbolic
}

impl MixedCurvatureAttention {
    pub fn new(
        dim_in: usize,
        dim_euclidean: usize,
        dim_hyperbolic: usize,
        curvature: f32,
        num_heads: usize,
    ) -> Self {
        assert_eq!(
            dim_euclidean + dim_hyperbolic,
            dim_in,
            "Dimension split must sum to input dimension"
        );

        Self {
            euclidean_attention: EuclideanAttention::new(
                dim_euclidean,
                dim_euclidean,
                num_heads,
            ),
            hyperbolic_attention: HyperbolicAttention::new(
                dim_hyperbolic,
                dim_hyperbolic,
                curvature,
                num_heads,
            ),
            dim_split: (dim_euclidean, dim_hyperbolic),
            alpha: 0.5, // Learnable parameter
        }
    }

    /// Forward pass through mixed-curvature space
    pub fn forward(
        &self,
        node_features: &Array2<f32>,
        edge_index: &[(usize, usize)],
    ) -> Array2<f32> {
        let (dim_e, dim_h) = self.dim_split;

        // Split features into Euclidean and hyperbolic subspaces
        let features_euclidean = node_features.slice(s![.., 0..dim_e]).to_owned();
        let features_hyperbolic = node_features.slice(s![.., dim_e..dim_e+dim_h]).to_owned();

        // Process each subspace
        let out_euclidean = self.euclidean_attention.forward(&features_euclidean, edge_index);
        let out_hyperbolic = self.hyperbolic_attention.forward(&features_hyperbolic, edge_index);

        // Concatenate outputs
        let num_nodes = node_features.shape()[0];
        let mut output = Array2::zeros((num_nodes, dim_e + dim_h));

        // Weighted combination (learnable)
        output.slice_mut(s![.., 0..dim_e]).assign(&(&out_euclidean * (1.0 - self.alpha)));
        output.slice_mut(s![.., dim_e..dim_e+dim_h]).assign(&(&out_hyperbolic * self.alpha));

        output
    }
}

/// Simple Euclidean attention for comparison
pub struct EuclideanAttention {
    pub w_query: Array2<f32>,
    pub w_key: Array2<f32>,
    pub w_value: Array2<f32>,
    pub temperature: f32,
    pub num_heads: usize,
}

impl EuclideanAttention {
    pub fn new(dim_in: usize, dim_out: usize, num_heads: usize) -> Self {
        let dim_per_head = dim_out / num_heads;
        Self {
            w_query: Array2::zeros((dim_in, dim_out)),
            w_key: Array2::zeros((dim_in, dim_out)),
            w_value: Array2::zeros((dim_in, dim_out)),
            temperature: (dim_per_head as f32).sqrt(),
            num_heads,
        }
    }

    pub fn forward(
        &self,
        node_features: &Array2<f32>,
        edge_index: &[(usize, usize)],
    ) -> Array2<f32> {
        // Standard scaled dot-product attention
        let queries = node_features.dot(&self.w_query);
        let keys = node_features.dot(&self.w_key);
        let values = node_features.dot(&self.w_value);

        let num_nodes = node_features.shape()[0];
        let dim_out = self.w_query.shape()[1];
        let mut output = Array2::zeros((num_nodes, dim_out));

        for target_node in 0..num_nodes {
            let query = queries.row(target_node);

            let neighbors: Vec<usize> = edge_index
                .iter()
                .filter(|(_, dst)| *dst == target_node)
                .map(|(src, _)| *src)
                .collect();

            if neighbors.is_empty() {
                continue;
            }

            // Compute attention scores (dot product)
            let mut scores = Vec::with_capacity(neighbors.len());
            for &neighbor_idx in &neighbors {
                let key = keys.row(neighbor_idx);
                let score = (query * key).sum() / self.temperature;
                scores.push(score);
            }

            // Softmax
            let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let exp_scores: Vec<f32> = scores.iter().map(|s| (s - max_score).exp()).collect();
            let sum_exp: f32 = exp_scores.iter().sum();
            let weights: Vec<f32> = exp_scores.iter().map(|e| e / sum_exp).collect();

            // Weighted sum
            let mut aggregated = Array1::zeros(dim_out);
            for (i, &neighbor_idx) in neighbors.iter().enumerate() {
                let value = values.row(neighbor_idx);
                aggregated = aggregated + &(value.to_owned() * weights[i]);
            }

            output.row_mut(target_node).assign(&aggregated);
        }

        output
    }
}
```

### 3.2 Automatic Space Selection

```rust
impl MixedCurvatureAttention {
    /// Learn optimal mixing parameter α via gradient descent
    ///
    /// Uses gating mechanism to decide per-feature contribution
    pub fn learn_mixing_weights(
        &mut self,
        node_features: &Array2<f32>,
    ) {
        // Compute feature statistics for each subspace
        let (dim_e, dim_h) = self.dim_split;

        let features_e = node_features.slice(s![.., 0..dim_e]);
        let features_h = node_features.slice(s![.., dim_e..dim_e+dim_h]);

        // Measure "hierarchy score" (e.g., via tree-likeness metric)
        let hierarchy_score = compute_hierarchy_score(&features_h);

        // Update α: higher hierarchy → more hyperbolic weight
        self.alpha = hierarchy_score.clamp(0.0, 1.0);
    }
}

/// Compute hierarchy score based on feature distribution
///
/// Higher score = more tree-like/hierarchical structure
fn compute_hierarchy_score(features: &ArrayView2<f32>) -> f32 {
    // Simplified metric: ratio of max to mean distance
    // More sophisticated: Gromov's δ-hyperbolicity

    let mut distances = Vec::new();
    let num_samples = features.shape()[0].min(100); // Sample for efficiency

    for i in 0..num_samples {
        for j in i+1..num_samples {
            let diff = &features.row(i).to_owned() - &features.row(j).to_owned();
            let dist = diff.mapv(|v| v * v).sum().sqrt();
            distances.push(dist);
        }
    }

    if distances.is_empty() {
        return 0.5;
    }

    let max_dist = distances.iter().cloned().fold(0.0f32, f32::max);
    let mean_dist: f32 = distances.iter().sum::<f32>() / distances.len() as f32;

    // Normalize to [0, 1]
    (max_dist / mean_dist - 1.0).min(1.0).max(0.0)
}
```

## 4. Unit Tests

### 4.1 Poincaré Operations Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;
    use approx::assert_relative_eq;

    #[test]
    fn test_poincare_distance_zero() {
        let x = array![0.1, 0.2, 0.3];
        let dist = poincare_distance(&x, &x, 1.0);
        assert_relative_eq!(dist, 0.0, epsilon = 1e-5);
    }

    #[test]
    fn test_poincare_distance_symmetric() {
        let x = array![0.1, 0.2];
        let y = array![0.3, 0.4];
        let c = 1.0;

        let d_xy = poincare_distance(&x, &y, c);
        let d_yx = poincare_distance(&y, &x, c);

        assert_relative_eq!(d_xy, d_yx, epsilon = 1e-5);
    }

    #[test]
    fn test_poincare_distance_triangle_inequality() {
        let x = array![0.1, 0.1];
        let y = array![0.2, 0.2];
        let z = array![0.3, 0.1];
        let c = 1.0;

        let d_xy = poincare_distance(&x, &y, c);
        let d_yz = poincare_distance(&y, &z, c);
        let d_xz = poincare_distance(&x, &z, c);

        // Triangle inequality: d(x,z) ≤ d(x,y) + d(y,z)
        assert!(d_xz <= d_xy + d_yz + 1e-5);
    }

    #[test]
    fn test_mobius_add_identity() {
        let x = array![0.1, 0.2, 0.3];
        let zero = Array1::zeros(3);
        let c = 1.0;

        let result = mobius_add(&x, &zero, c);

        for i in 0..3 {
            assert_relative_eq!(result[i], x[i], epsilon = 1e-5);
        }
    }

    #[test]
    fn test_mobius_add_inverse() {
        let x = array![0.1, 0.2];
        let c = 1.0;

        let neg_x = mobius_negation(&x, c);
        let result = mobius_add(&x, &neg_x, c);

        for i in 0..2 {
            assert_relative_eq!(result[i], 0.0, epsilon = 1e-5);
        }
    }

    #[test]
    fn test_mobius_scalar_mult_zero() {
        let x = array![0.1, 0.2, 0.3];
        let c = 1.0;

        let result = mobius_scalar_mult(0.0, &x, c);

        for i in 0..3 {
            assert_relative_eq!(result[i], 0.0, epsilon = 1e-5);
        }
    }

    #[test]
    fn test_mobius_scalar_mult_one() {
        let x = array![0.1, 0.2, 0.3];
        let c = 1.0;

        let result = mobius_scalar_mult(1.0, &x, c);

        for i in 0..3 {
            assert_relative_eq!(result[i], x[i], epsilon = 1e-5);
        }
    }

    #[test]
    fn test_exp_log_inverse() {
        let x = array![0.1, 0.2];
        let v = array![0.05, -0.03];
        let c = 1.0;

        // exp_x(v)
        let y = exp_map(&x, &v, c);

        // log_x(y) should recover v
        let v_recovered = log_map(&x, &y, c);

        for i in 0..2 {
            assert_relative_eq!(v_recovered[i], v[i], epsilon = 1e-4);
        }
    }

    #[test]
    fn test_euclidean_limit() {
        let x = array![0.1, 0.2];
        let y = array![0.3, 0.4];
        let c = 1e-10; // Near-zero curvature

        // Should approximate Euclidean distance
        let hyp_dist = poincare_distance(&x, &y, c);
        let euclidean_dist = ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2)).sqrt();

        assert_relative_eq!(hyp_dist, euclidean_dist, epsilon = 1e-3);
    }

    #[test]
    fn test_boundary_stability() {
        let c = 1.0;
        let boundary = 1.0 / c.sqrt();

        // Point very close to boundary
        let x = array![boundary * 0.95, 0.0];
        let y = array![0.0, boundary * 0.95];

        // Should not panic or produce NaN
        let dist = poincare_distance(&x, &y, c);
        assert!(dist.is_finite());
        assert!(dist > 0.0);
    }
}
```

### 4.2 HyperbolicAttention Tests

```rust
#[cfg(test)]
mod attention_tests {
    use super::*;

    #[test]
    fn test_hyperbolic_attention_forward() {
        let mut attention = HyperbolicAttention::new(4, 8, 1.0, 2);
        attention.init_weights(0.1);

        // Simple graph: 0 → 1, 1 → 2
        let node_features = Array2::from_shape_vec(
            (3, 4),
            vec![
                0.1, 0.2, 0.3, 0.4,
                0.5, 0.6, 0.7, 0.8,
                0.2, 0.3, 0.4, 0.5,
            ],
        ).unwrap();

        let edge_index = vec![(0, 1), (1, 2)];

        let output = attention.forward(&node_features, &edge_index);

        assert_eq!(output.shape(), &[3, 8]);
        assert!(output.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_adaptive_curvature() {
        let mut attention = HyperbolicAttention::new(4, 8, 10.0, 2);

        // Large features that would exceed boundary
        let large_features = Array2::from_shape_vec(
            (2, 4),
            vec![5.0, 5.0, 5.0, 5.0, 3.0, 3.0, 3.0, 3.0],
        ).unwrap();

        let original_curvature = attention.curvature;
        attention.adaptive_curvature(&large_features);

        // Curvature should be reduced
        assert!(attention.curvature < original_curvature);
    }

    #[test]
    fn test_mixed_curvature_attention() {
        let attention = MixedCurvatureAttention::new(
            8,  // total dim
            4,  // euclidean dim
            4,  // hyperbolic dim
            1.0,
            2,
        );

        let node_features = Array2::from_shape_vec(
            (3, 8),
            vec![
                0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8,
                0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9,
                0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0,
            ],
        ).unwrap();

        let edge_index = vec![(0, 1), (1, 2), (0, 2)];

        let output = attention.forward(&node_features, &edge_index);

        assert_eq!(output.shape(), &[3, 8]);
        assert!(output.iter().all(|&x| x.is_finite()));
    }
}
```

### 4.3 Gradient and Optimization Tests

```rust
#[cfg(test)]
mod optimization_tests {
    use super::*;

    #[test]
    fn test_weight_initialization_bounds() {
        let mut attention = HyperbolicAttention::new(16, 32, 1.0, 4);
        attention.init_weights(0.01); // Small scale for hyperbolic

        // Weights should be small to avoid boundary
        let max_weight = attention.w_query.iter()
            .chain(attention.w_key.iter())
            .chain(attention.w_value.iter())
            .map(|&x| x.abs())
            .fold(0.0f32, f32::max);

        assert!(max_weight < 0.5, "Weights too large for hyperbolic space");
    }

    #[test]
    fn test_attention_output_magnitude() {
        let mut attention = HyperbolicAttention::new(8, 16, 1.0, 2);
        attention.init_weights(0.01);

        let node_features = Array2::from_shape_vec(
            (5, 8),
            (0..40).map(|i| (i as f32) * 0.01).collect(),
        ).unwrap();

        let edge_index = vec![(0, 1), (1, 2), (2, 3), (3, 4)];

        let output = attention.forward(&node_features, &edge_index);

        // Output should stay within reasonable bounds
        let max_output = output.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        assert!(max_output < 10.0, "Output magnitude too large");
    }
}
```

## 5. Integration Points

### 5.1 GNN Layer Integration

```rust
/// Example GNN layer using hyperbolic attention
pub struct HyperbolicGNNLayer {
    pub attention: HyperbolicAttention,
    pub mlp: Array2<f32>, // Feed-forward network
    pub layer_norm: LayerNorm,
}

impl HyperbolicGNNLayer {
    pub fn forward(
        &self,
        x: &Array2<f32>,
        edge_index: &[(usize, usize)],
    ) -> Array2<f32> {
        // 1. Hyperbolic attention
        let attended = self.attention.forward(x, edge_index);

        // 2. Residual connection
        let residual = x + &attended;

        // 3. Layer normalization
        let normalized = self.layer_norm.forward(&residual);

        // 4. Feed-forward
        let output = normalized.dot(&self.mlp);

        output
    }
}

pub struct LayerNorm {
    epsilon: f32,
}

impl LayerNorm {
    pub fn forward(&self, x: &Array2<f32>) -> Array2<f32> {
        let mean = x.mean_axis(Axis(1)).unwrap();
        let var = x.var_axis(Axis(1), 0.0);

        let mut normalized = Array2::zeros(x.raw_dim());
        for i in 0..x.shape()[0] {
            for j in 0..x.shape()[1] {
                normalized[[i, j]] = (x[[i, j]] - mean[i]) / (var[i] + self.epsilon).sqrt();
            }
        }
        normalized
    }
}
```

## 6. Performance Considerations

### Computational Complexity

| Operation | Time Complexity | Space Complexity |
|-----------|----------------|------------------|
| Poincaré distance | O(d) | O(1) |
| Möbius addition | O(d) | O(d) |
| exp_map / log_map | O(d) | O(d) |
| HyperbolicAttention | O(E · d · h) | O(N · d) |

where:
- N = number of nodes
- E = number of edges
- d = feature dimension
- h = number of attention heads

### Optimization Strategies

1. **Vectorization**: Batch operations across nodes
2. **Sparse computation**: Only compute for existing edges
3. **Curvature caching**: Reuse curvature-dependent constants
4. **Early termination**: Skip zero-weight contributions

## 7. References

- **Chami et al. (2019)**: "Hyperbolic Graph Convolutional Neural Networks"
- **Nickel & Kiela (2017)**: "Poincaré Embeddings for Learning Hierarchical Representations"
- **Ganea et al. (2018)**: "Hyperbolic Neural Networks"
- **Gu et al. (2019)**: "Learning Mixed-Curvature Representations in Product Spaces"

## Next Steps

1. Implement in `/src/gnn/hyperbolic_attention.rs`
2. Add benchmarks comparing Euclidean vs. Hyperbolic attention
3. Integrate with HNSW indexing (Agent 1)
4. Test on hierarchical datasets (trees, taxonomies)
5. Coordinate with Agent 3 (VAE) for latent space compression

---

**Dependencies**: `ndarray`, `rand`, `rand_distr`, `approx` (for tests)
**Estimated LOC**: ~800 lines
**Test Coverage Target**: >90%
