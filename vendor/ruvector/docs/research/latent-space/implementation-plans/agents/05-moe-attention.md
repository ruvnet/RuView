# Agent 5: Mixture of Experts (MoE) Adaptive Attention

## Overview

This implementation provides a flexible Mixture of Experts attention mechanism that dynamically routes queries to specialized attention experts based on learned gating functions. The system supports multiple expert types (standard multi-head, hyperbolic, linear, edge-featured) with load balancing and efficient top-k routing.

## Architecture Components

### 1. Expert Types Enum

```rust
use ndarray::{Array2, Array3, ArrayView2, ArrayView3};
use std::sync::Arc;

/// Types of attention experts available in the mixture
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpertType {
    /// Standard multi-head attention
    Standard,
    /// Hyperbolic geometry attention
    Hyperbolic,
    /// Linear complexity attention (e.g., Performer)
    Linear,
    /// Edge-featured attention for graphs
    EdgeFeatured,
}

impl ExpertType {
    pub fn name(&self) -> &str {
        match self {
            ExpertType::Standard => "standard",
            ExpertType::Hyperbolic => "hyperbolic",
            ExpertType::Linear => "linear",
            ExpertType::EdgeFeatured => "edge_featured",
        }
    }
}
```

### 2. Attention Expert Trait

```rust
/// Trait that all attention experts must implement
pub trait AttentionExpert: Send + Sync {
    /// Forward pass through the expert
    ///
    /// # Arguments
    /// * `queries` - Query embeddings [batch, seq_len, dim]
    /// * `keys` - Key embeddings [batch, seq_len, dim]
    /// * `values` - Value embeddings [batch, seq_len, dim]
    /// * `edge_features` - Optional edge features for graph attention
    ///
    /// # Returns
    /// * Attended output [batch, seq_len, dim]
    /// * Attention weights [batch, num_heads, seq_len, seq_len]
    fn forward(
        &self,
        queries: ArrayView3<f32>,
        keys: ArrayView3<f32>,
        values: ArrayView3<f32>,
        edge_features: Option<ArrayView3<f32>>,
    ) -> (Array3<f32>, Array3<f32>);

    /// Get the expert type
    fn expert_type(&self) -> ExpertType;

    /// Get output dimension
    fn output_dim(&self) -> usize;

    /// Get number of parameters
    fn num_parameters(&self) -> usize;
}
```

### 3. Learned Routing Network

```rust
use ndarray_rand::RandomExt;
use ndarray_rand::rand_distr::Uniform;

/// Learned routing network for expert selection
pub struct LearnedRouter {
    /// Input dimension
    input_dim: usize,
    /// Number of experts
    num_experts: usize,
    /// Hidden dimension for routing network
    hidden_dim: usize,
    /// Temperature for softmax (higher = more uniform)
    temperature: f32,

    // Network parameters
    /// First layer weights [input_dim, hidden_dim]
    w1: Array2<f32>,
    /// First layer bias [hidden_dim]
    b1: Array1<f32>,
    /// Second layer weights [hidden_dim, num_experts]
    w2: Array2<f32>,
    /// Second layer bias [num_experts]
    b2: Array1<f32>,

    /// Load balancing coefficient
    load_balance_loss_coef: f32,
}

impl LearnedRouter {
    pub fn new(
        input_dim: usize,
        num_experts: usize,
        hidden_dim: usize,
        temperature: f32,
    ) -> Self {
        // Initialize with Xavier/Glorot uniform
        let limit1 = (6.0 / (input_dim + hidden_dim) as f32).sqrt();
        let limit2 = (6.0 / (hidden_dim + num_experts) as f32).sqrt();

        Self {
            input_dim,
            num_experts,
            hidden_dim,
            temperature,
            w1: Array2::random((input_dim, hidden_dim), Uniform::new(-limit1, limit1)),
            b1: Array1::zeros(hidden_dim),
            w2: Array2::random((hidden_dim, num_experts), Uniform::new(-limit2, limit2)),
            b2: Array1::zeros(num_experts),
            load_balance_loss_coef: 0.01,
        }
    }

    /// Compute routing scores for each query
    ///
    /// # Arguments
    /// * `queries` - Query embeddings [batch, seq_len, input_dim]
    ///
    /// # Returns
    /// * Routing logits [batch, seq_len, num_experts]
    pub fn route(&self, queries: ArrayView3<f32>) -> Array3<f32> {
        let (batch_size, seq_len, _) = queries.dim();

        // Reshape for matrix multiply: [batch * seq_len, input_dim]
        let queries_flat = queries
            .to_owned()
            .into_shape((batch_size * seq_len, self.input_dim))
            .unwrap();

        // First layer: [batch * seq_len, hidden_dim]
        let hidden = queries_flat.dot(&self.w1) + &self.b1;
        let hidden = hidden.mapv(|x| x.max(0.0)); // ReLU

        // Second layer: [batch * seq_len, num_experts]
        let logits = hidden.dot(&self.w2) + &self.b2;

        // Apply temperature scaling
        let logits = logits / self.temperature;

        // Reshape back: [batch, seq_len, num_experts]
        logits.into_shape((batch_size, seq_len, self.num_experts)).unwrap()
    }

    /// Compute top-k gating with softmax normalization
    ///
    /// # Arguments
    /// * `logits` - Routing logits [batch, seq_len, num_experts]
    /// * `k` - Number of experts to select
    ///
    /// # Returns
    /// * Gating weights [batch, seq_len, num_experts] (sparse, only top-k nonzero)
    /// * Expert indices [batch, seq_len, k]
    pub fn top_k_gating(&self, logits: ArrayView3<f32>, k: usize) -> (Array3<f32>, Array3<usize>) {
        let (batch_size, seq_len, num_experts) = logits.dim();
        assert!(k <= num_experts, "k must be <= num_experts");

        let mut gates = Array3::<f32>::zeros((batch_size, seq_len, num_experts));
        let mut indices = Array3::<usize>::zeros((batch_size, seq_len, k));

        for b in 0..batch_size {
            for s in 0..seq_len {
                let row = logits.slice(s![b, s, ..]);

                // Get top-k indices
                let mut indexed: Vec<(usize, f32)> = row
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| (i, v))
                    .collect();
                indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                let top_k_indices: Vec<usize> = indexed.iter().take(k).map(|(i, _)| *i).collect();
                let top_k_logits: Vec<f32> = indexed.iter().take(k).map(|(_, v)| *v).collect();

                // Softmax over top-k
                let max_logit = top_k_logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let exp_sum: f32 = top_k_logits.iter().map(|&x| (x - max_logit).exp()).sum();

                for (idx, &expert_idx) in top_k_indices.iter().enumerate() {
                    let gate = ((top_k_logits[idx] - max_logit).exp()) / exp_sum;
                    gates[[b, s, expert_idx]] = gate;
                    indices[[b, s, idx]] = expert_idx;
                }
            }
        }

        (gates, indices)
    }

    /// Compute load balancing loss to encourage uniform expert usage
    ///
    /// # Arguments
    /// * `gates` - Gating weights [batch, seq_len, num_experts]
    ///
    /// # Returns
    /// * Load balancing auxiliary loss (scalar)
    pub fn load_balancing_loss(&self, gates: ArrayView3<f32>) -> f32 {
        let (batch_size, seq_len, num_experts) = gates.dim();
        let total_tokens = (batch_size * seq_len) as f32;

        // Compute fraction of tokens routed to each expert
        let mut expert_usage = Array1::<f32>::zeros(num_experts);
        for e in 0..num_experts {
            let usage: f32 = gates.slice(s![.., .., e]).sum();
            expert_usage[e] = usage / total_tokens;
        }

        // Compute importance (average gate value when expert is selected)
        let mut expert_importance = Array1::<f32>::zeros(num_experts);
        for e in 0..num_experts {
            let importance: f32 = gates.slice(s![.., .., e]).mean().unwrap_or(0.0);
            expert_importance[e] = importance;
        }

        // Load balancing loss: encourages uniform usage × importance
        // Loss = num_experts * sum(usage[i] * importance[i])
        // Minimal when usage and importance are balanced
        let loss: f32 = expert_usage.iter()
            .zip(expert_importance.iter())
            .map(|(&u, &i)| u * i)
            .sum::<f32>() * num_experts as f32;

        loss * self.load_balance_loss_coef
    }
}

use ndarray::Array1;
use ndarray::s;
```

### 4. MoE Attention Configuration

```rust
/// Configuration for MoE Attention
#[derive(Debug, Clone)]
pub struct MoEAttentionConfig {
    /// Input/output dimension
    pub dim: usize,
    /// Number of attention heads per expert
    pub num_heads: usize,
    /// Number of experts in the mixture
    pub num_experts: usize,
    /// Number of experts to activate per query (top-k)
    pub top_k: usize,
    /// Expert types to include
    pub expert_types: Vec<ExpertType>,
    /// Hidden dimension for routing network
    pub router_hidden_dim: usize,
    /// Temperature for routing softmax
    pub router_temperature: f32,
    /// Load balancing loss coefficient
    pub load_balance_coef: f32,
    /// Dropout rate
    pub dropout: f32,
}

impl Default for MoEAttentionConfig {
    fn default() -> Self {
        Self {
            dim: 512,
            num_heads: 8,
            num_experts: 4,
            top_k: 2,
            expert_types: vec![
                ExpertType::Standard,
                ExpertType::Hyperbolic,
                ExpertType::Linear,
                ExpertType::EdgeFeatured,
            ],
            router_hidden_dim: 256,
            router_temperature: 1.0,
            load_balance_coef: 0.01,
            dropout: 0.1,
        }
    }
}

impl MoEAttentionConfig {
    pub fn builder() -> MoEAttentionConfigBuilder {
        MoEAttentionConfigBuilder::default()
    }
}

/// Builder for MoEAttentionConfig
#[derive(Default)]
pub struct MoEAttentionConfigBuilder {
    dim: Option<usize>,
    num_heads: Option<usize>,
    num_experts: Option<usize>,
    top_k: Option<usize>,
    expert_types: Option<Vec<ExpertType>>,
    router_hidden_dim: Option<usize>,
    router_temperature: Option<f32>,
    load_balance_coef: Option<f32>,
    dropout: Option<f32>,
}

impl MoEAttentionConfigBuilder {
    pub fn dim(mut self, dim: usize) -> Self {
        self.dim = Some(dim);
        self
    }

    pub fn num_heads(mut self, num_heads: usize) -> Self {
        self.num_heads = Some(num_heads);
        self
    }

    pub fn num_experts(mut self, num_experts: usize) -> Self {
        self.num_experts = Some(num_experts);
        self
    }

    pub fn top_k(mut self, top_k: usize) -> Self {
        self.top_k = Some(top_k);
        self
    }

    pub fn expert_types(mut self, expert_types: Vec<ExpertType>) -> Self {
        self.expert_types = Some(expert_types);
        self
    }

    pub fn router_hidden_dim(mut self, dim: usize) -> Self {
        self.router_hidden_dim = Some(dim);
        self
    }

    pub fn router_temperature(mut self, temp: f32) -> Self {
        self.router_temperature = Some(temp);
        self
    }

    pub fn load_balance_coef(mut self, coef: f32) -> Self {
        self.load_balance_coef = Some(coef);
        self
    }

    pub fn dropout(mut self, dropout: f32) -> Self {
        self.dropout = Some(dropout);
        self
    }

    pub fn build(self) -> MoEAttentionConfig {
        let default = MoEAttentionConfig::default();

        MoEAttentionConfig {
            dim: self.dim.unwrap_or(default.dim),
            num_heads: self.num_heads.unwrap_or(default.num_heads),
            num_experts: self.num_experts.unwrap_or(default.num_experts),
            top_k: self.top_k.unwrap_or(default.top_k),
            expert_types: self.expert_types.unwrap_or(default.expert_types),
            router_hidden_dim: self.router_hidden_dim.unwrap_or(default.router_hidden_dim),
            router_temperature: self.router_temperature.unwrap_or(default.router_temperature),
            load_balance_coef: self.load_balance_coef.unwrap_or(default.load_balance_coef),
            dropout: self.dropout.unwrap_or(default.dropout),
        }
    }
}
```

### 5. MoE Attention Implementation

```rust
/// Main Mixture of Experts Attention module
pub struct MoEAttention {
    config: MoEAttentionConfig,

    /// Routing network
    router: LearnedRouter,

    /// Pool of attention experts
    experts: Vec<Box<dyn AttentionExpert>>,

    /// Output projection
    output_projection: Array2<f32>,
    output_bias: Array1<f32>,

    /// Training mode flag
    training: bool,

    /// Cached auxiliary losses
    aux_loss: f32,
}

impl MoEAttention {
    pub fn new(config: MoEAttentionConfig) -> Self {
        assert_eq!(
            config.expert_types.len(),
            config.num_experts,
            "Number of expert types must match num_experts"
        );
        assert!(
            config.top_k <= config.num_experts,
            "top_k must be <= num_experts"
        );

        // Create router
        let router = LearnedRouter::new(
            config.dim,
            config.num_experts,
            config.router_hidden_dim,
            config.router_temperature,
        );

        // Create experts (placeholder - would instantiate actual expert implementations)
        let experts: Vec<Box<dyn AttentionExpert>> = config
            .expert_types
            .iter()
            .map(|&expert_type| {
                create_expert(expert_type, config.dim, config.num_heads, config.dropout)
            })
            .collect();

        // Output projection
        let limit = (6.0 / (config.dim + config.dim) as f32).sqrt();
        let output_projection = Array2::random(
            (config.dim, config.dim),
            Uniform::new(-limit, limit),
        );
        let output_bias = Array1::zeros(config.dim);

        Self {
            config,
            router,
            experts,
            output_projection,
            output_bias,
            training: true,
            aux_loss: 0.0,
        }
    }

    /// Forward pass through MoE attention
    ///
    /// # Arguments
    /// * `queries` - Query embeddings [batch, seq_len, dim]
    /// * `keys` - Key embeddings [batch, seq_len, dim]
    /// * `values` - Value embeddings [batch, seq_len, dim]
    /// * `edge_features` - Optional edge features [batch, seq_len, seq_len, edge_dim]
    ///
    /// # Returns
    /// * Output embeddings [batch, seq_len, dim]
    /// * Attention weights [batch, num_experts, num_heads, seq_len, seq_len]
    /// * Expert assignment info
    pub fn forward(
        &mut self,
        queries: ArrayView3<f32>,
        keys: ArrayView3<f32>,
        values: ArrayView3<f32>,
        edge_features: Option<ArrayView3<f32>>,
    ) -> MoEAttentionOutput {
        let (batch_size, seq_len, dim) = queries.dim();
        assert_eq!(dim, self.config.dim);

        // 1. Compute routing scores
        let routing_logits = self.router.route(queries);

        // 2. Get top-k gating
        let (gates, expert_indices) = self.router.top_k_gating(
            routing_logits.view(),
            self.config.top_k,
        );

        // 3. Compute load balancing loss (only in training)
        if self.training {
            self.aux_loss = self.router.load_balancing_loss(gates.view());
        }

        // 4. Initialize output accumulator
        let mut output = Array3::<f32>::zeros((batch_size, seq_len, dim));
        let mut all_attention_weights = Vec::new();

        // 5. Process each expert
        for expert_idx in 0..self.config.num_experts {
            // Get the expert
            let expert = &self.experts[expert_idx];

            // Find tokens routed to this expert
            let expert_mask = gates.slice(s![.., .., expert_idx]);

            // Skip if no tokens assigned
            if expert_mask.iter().all(|&x| x == 0.0) {
                continue;
            }

            // Run expert on all queries (could be optimized to only process assigned tokens)
            let (expert_output, expert_attn) = expert.forward(
                queries,
                keys,
                values,
                edge_features,
            );

            // Weight and accumulate expert output
            for b in 0..batch_size {
                for s in 0..seq_len {
                    let gate = expert_mask[[b, s]];
                    if gate > 0.0 {
                        let weighted_output = expert_output.slice(s![b, s, ..]).mapv(|x| x * gate);
                        let mut out_slice = output.slice_mut(s![b, s, ..]);
                        out_slice += &weighted_output;
                    }
                }
            }

            all_attention_weights.push(expert_attn);
        }

        // 6. Apply output projection
        let output_flat = output
            .to_owned()
            .into_shape((batch_size * seq_len, dim))
            .unwrap();
        let projected = output_flat.dot(&self.output_projection) + &self.output_bias;
        let output = projected.into_shape((batch_size, seq_len, dim)).unwrap();

        MoEAttentionOutput {
            output,
            attention_weights: all_attention_weights,
            gates,
            expert_indices,
            aux_loss: self.aux_loss,
        }
    }

    /// Set training mode
    pub fn train(&mut self) {
        self.training = true;
    }

    /// Set evaluation mode
    pub fn eval(&mut self) {
        self.training = false;
    }

    /// Get auxiliary loss (for backprop)
    pub fn get_aux_loss(&self) -> f32 {
        self.aux_loss
    }

    /// Get number of parameters
    pub fn num_parameters(&self) -> usize {
        let router_params = self.router.w1.len() + self.router.b1.len()
            + self.router.w2.len() + self.router.b2.len();
        let expert_params: usize = self.experts.iter().map(|e| e.num_parameters()).sum();
        let output_params = self.output_projection.len() + self.output_bias.len();

        router_params + expert_params + output_params
    }
}

/// Output from MoE attention forward pass
pub struct MoEAttentionOutput {
    /// Main output [batch, seq_len, dim]
    pub output: Array3<f32>,
    /// Attention weights from each expert
    pub attention_weights: Vec<Array3<f32>>,
    /// Gating weights [batch, seq_len, num_experts]
    pub gates: Array3<f32>,
    /// Top-k expert indices [batch, seq_len, k]
    pub expert_indices: Array3<usize>,
    /// Auxiliary load balancing loss
    pub aux_loss: f32,
}
```

### 6. Expert Factory and Implementations

```rust
/// Factory function to create experts
fn create_expert(
    expert_type: ExpertType,
    dim: usize,
    num_heads: usize,
    dropout: f32,
) -> Box<dyn AttentionExpert> {
    match expert_type {
        ExpertType::Standard => Box::new(StandardAttentionExpert::new(dim, num_heads, dropout)),
        ExpertType::Hyperbolic => Box::new(HyperbolicAttentionExpert::new(dim, num_heads, dropout)),
        ExpertType::Linear => Box::new(LinearAttentionExpert::new(dim, num_heads, dropout)),
        ExpertType::EdgeFeatured => Box::new(EdgeAttentionExpert::new(dim, num_heads, dropout)),
    }
}

// Placeholder implementations (would be fully implemented in actual code)

/// Standard multi-head attention expert
pub struct StandardAttentionExpert {
    dim: usize,
    num_heads: usize,
    head_dim: usize,
    dropout: f32,
    // Projection matrices would be here
    wq: Array2<f32>,
    wk: Array2<f32>,
    wv: Array2<f32>,
}

impl StandardAttentionExpert {
    pub fn new(dim: usize, num_heads: usize, dropout: f32) -> Self {
        let head_dim = dim / num_heads;
        let limit = (6.0 / (dim + dim) as f32).sqrt();

        Self {
            dim,
            num_heads,
            head_dim,
            dropout,
            wq: Array2::random((dim, dim), Uniform::new(-limit, limit)),
            wk: Array2::random((dim, dim), Uniform::new(-limit, limit)),
            wv: Array2::random((dim, dim), Uniform::new(-limit, limit)),
        }
    }
}

impl AttentionExpert for StandardAttentionExpert {
    fn forward(
        &self,
        queries: ArrayView3<f32>,
        keys: ArrayView3<f32>,
        values: ArrayView3<f32>,
        _edge_features: Option<ArrayView3<f32>>,
    ) -> (Array3<f32>, Array3<f32>) {
        let (batch_size, seq_len, _) = queries.dim();

        // Standard scaled dot-product attention
        // (Simplified - full implementation would reshape for multi-head)
        let q_flat = queries.to_owned().into_shape((batch_size * seq_len, self.dim)).unwrap();
        let k_flat = keys.to_owned().into_shape((batch_size * seq_len, self.dim)).unwrap();
        let v_flat = values.to_owned().into_shape((batch_size * seq_len, self.dim)).unwrap();

        let q_proj = q_flat.dot(&self.wq);
        let k_proj = k_flat.dot(&self.wk);
        let v_proj = v_flat.dot(&self.wv);

        // Reshape and compute attention
        let output = v_proj; // Placeholder
        let output = output.into_shape((batch_size, seq_len, self.dim)).unwrap();

        let attn_weights = Array3::ones((batch_size, self.num_heads, seq_len, seq_len).f())
            .into_shape((batch_size, self.num_heads, seq_len))
            .unwrap(); // Placeholder

        (output, attn_weights)
    }

    fn expert_type(&self) -> ExpertType {
        ExpertType::Standard
    }

    fn output_dim(&self) -> usize {
        self.dim
    }

    fn num_parameters(&self) -> usize {
        self.wq.len() + self.wk.len() + self.wv.len()
    }
}

/// Hyperbolic attention expert
pub struct HyperbolicAttentionExpert {
    dim: usize,
    num_heads: usize,
    dropout: f32,
    curvature: f32,
}

impl HyperbolicAttentionExpert {
    pub fn new(dim: usize, num_heads: usize, dropout: f32) -> Self {
        Self {
            dim,
            num_heads,
            dropout,
            curvature: -1.0,
        }
    }
}

impl AttentionExpert for HyperbolicAttentionExpert {
    fn forward(
        &self,
        queries: ArrayView3<f32>,
        _keys: ArrayView3<f32>,
        _values: ArrayView3<f32>,
        _edge_features: Option<ArrayView3<f32>>,
    ) -> (Array3<f32>, Array3<f32>) {
        // Placeholder - would implement hyperbolic geometry attention
        let (batch_size, seq_len, _) = queries.dim();
        (
            Array3::zeros((batch_size, seq_len, self.dim)),
            Array3::zeros((batch_size, self.num_heads, seq_len)),
        )
    }

    fn expert_type(&self) -> ExpertType {
        ExpertType::Hyperbolic
    }

    fn output_dim(&self) -> usize {
        self.dim
    }

    fn num_parameters(&self) -> usize {
        0 // Placeholder
    }
}

/// Linear complexity attention expert (e.g., Performer-style)
pub struct LinearAttentionExpert {
    dim: usize,
    num_heads: usize,
    dropout: f32,
}

impl LinearAttentionExpert {
    pub fn new(dim: usize, num_heads: usize, dropout: f32) -> Self {
        Self { dim, num_heads, dropout }
    }
}

impl AttentionExpert for LinearAttentionExpert {
    fn forward(
        &self,
        queries: ArrayView3<f32>,
        _keys: ArrayView3<f32>,
        _values: ArrayView3<f32>,
        _edge_features: Option<ArrayView3<f32>>,
    ) -> (Array3<f32>, Array3<f32>) {
        // Placeholder - would implement kernel-based linear attention
        let (batch_size, seq_len, _) = queries.dim();
        (
            Array3::zeros((batch_size, seq_len, self.dim)),
            Array3::zeros((batch_size, self.num_heads, seq_len)),
        )
    }

    fn expert_type(&self) -> ExpertType {
        ExpertType::Linear
    }

    fn output_dim(&self) -> usize {
        self.dim
    }

    fn num_parameters(&self) -> usize {
        0 // Placeholder
    }
}

/// Edge-featured attention expert for graphs
pub struct EdgeAttentionExpert {
    dim: usize,
    num_heads: usize,
    dropout: f32,
}

impl EdgeAttentionExpert {
    pub fn new(dim: usize, num_heads: usize, dropout: f32) -> Self {
        Self { dim, num_heads, dropout }
    }
}

impl AttentionExpert for EdgeAttentionExpert {
    fn forward(
        &self,
        queries: ArrayView3<f32>,
        _keys: ArrayView3<f32>,
        _values: ArrayView3<f32>,
        edge_features: Option<ArrayView3<f32>>,
    ) -> (Array3<f32>, Array3<f32>) {
        // Placeholder - would incorporate edge features into attention
        let (batch_size, seq_len, _) = queries.dim();
        let _ = edge_features; // Would use this
        (
            Array3::zeros((batch_size, seq_len, self.dim)),
            Array3::zeros((batch_size, self.num_heads, seq_len)),
        )
    }

    fn expert_type(&self) -> ExpertType {
        ExpertType::EdgeFeatured
    }

    fn output_dim(&self) -> usize {
        self.dim
    }

    fn num_parameters(&self) -> usize {
        0 // Placeholder
    }
}
```

## Training Considerations

### 1. Loss Function

```rust
/// Complete loss for training MoE Attention
pub struct MoEAttentionLoss {
    /// Main task loss (e.g., classification, regression)
    task_loss_fn: Box<dyn Fn(&Array2<f32>, &Array2<f32>) -> f32>,
    /// Load balancing loss coefficient
    load_balance_coef: f32,
}

impl MoEAttentionLoss {
    pub fn compute(
        &self,
        predictions: &Array2<f32>,
        targets: &Array2<f32>,
        aux_loss: f32,
    ) -> f32 {
        let task_loss = (self.task_loss_fn)(predictions, targets);
        let total_loss = task_loss + self.load_balance_coef * aux_loss;
        total_loss
    }
}
```

### 2. Training Loop Integration

```rust
/// Training step for MoE Attention
pub fn training_step(
    model: &mut MoEAttention,
    queries: ArrayView3<f32>,
    keys: ArrayView3<f32>,
    values: ArrayView3<f32>,
    targets: &Array2<f32>,
    loss_fn: &MoEAttentionLoss,
    learning_rate: f32,
) -> f32 {
    // Forward pass
    model.train();
    let output = model.forward(queries, keys, values, None);

    // Compute loss
    let predictions = output.output.slice(s![.., 0, ..]).to_owned(); // Simplified
    let total_loss = loss_fn.compute(&predictions, targets, output.aux_loss);

    // Backward pass (would use autograd in real implementation)
    // gradients = compute_gradients(total_loss);
    // update_parameters(model, gradients, learning_rate);

    total_loss
}
```

### 3. Optimization Strategies

```rust
/// Optimizer configuration for MoE training
pub struct MoEOptimizer {
    /// Learning rate for routing network
    router_lr: f32,
    /// Learning rate for experts
    expert_lr: f32,
    /// Learning rate for output projection
    output_lr: f32,
    /// Weight decay
    weight_decay: f32,
    /// Gradient clipping threshold
    grad_clip: f32,
}

impl Default for MoEOptimizer {
    fn default() -> Self {
        Self {
            router_lr: 1e-3,
            expert_lr: 5e-4,
            output_lr: 5e-4,
            weight_decay: 1e-5,
            grad_clip: 1.0,
        }
    }
}
```

### 4. Expert Specialization Monitoring

```rust
/// Monitor expert specialization during training
pub struct ExpertSpecializationMonitor {
    /// Expert usage statistics [num_experts]
    usage_counts: Array1<usize>,
    /// Average gating weights per expert [num_experts]
    avg_gates: Array1<f32>,
    /// Number of steps
    num_steps: usize,
}

impl ExpertSpecializationMonitor {
    pub fn new(num_experts: usize) -> Self {
        Self {
            usage_counts: Array1::zeros(num_experts),
            avg_gates: Array1::zeros(num_experts),
            num_steps: 0,
        }
    }

    pub fn update(&mut self, gates: ArrayView3<f32>) {
        let num_experts = gates.dim().2;

        for e in 0..num_experts {
            let expert_gates = gates.slice(s![.., .., e]);
            let usage = expert_gates.iter().filter(|&&x| x > 0.0).count();
            let avg_gate = expert_gates.mean().unwrap_or(0.0);

            self.usage_counts[e] += usage;
            self.avg_gates[e] += avg_gate;
        }

        self.num_steps += 1;
    }

    pub fn get_statistics(&self) -> ExpertStats {
        let avg_usage = self.usage_counts.mapv(|x| x as f32 / self.num_steps as f32);
        let avg_gates = self.avg_gates.mapv(|x| x / self.num_steps as f32);

        ExpertStats {
            avg_usage,
            avg_gates,
        }
    }
}

pub struct ExpertStats {
    pub avg_usage: Array1<f32>,
    pub avg_gates: Array1<f32>,
}
```

## Usage Examples

### Basic Usage

```rust
fn example_basic_usage() {
    // Create configuration
    let config = MoEAttentionConfig::builder()
        .dim(512)
        .num_heads(8)
        .num_experts(4)
        .top_k(2)
        .expert_types(vec![
            ExpertType::Standard,
            ExpertType::Hyperbolic,
            ExpertType::Linear,
            ExpertType::EdgeFeatured,
        ])
        .router_temperature(1.0)
        .load_balance_coef(0.01)
        .build();

    // Create MoE attention module
    let mut moe_attn = MoEAttention::new(config);

    // Prepare inputs
    let batch_size = 32;
    let seq_len = 128;
    let dim = 512;

    let queries = Array3::<f32>::zeros((batch_size, seq_len, dim));
    let keys = Array3::<f32>::zeros((batch_size, seq_len, dim));
    let values = Array3::<f32>::zeros((batch_size, seq_len, dim));

    // Forward pass
    moe_attn.train();
    let output = moe_attn.forward(
        queries.view(),
        keys.view(),
        values.view(),
        None,
    );

    println!("Output shape: {:?}", output.output.dim());
    println!("Auxiliary loss: {:.6}", output.aux_loss);
    println!("Number of parameters: {}", moe_attn.num_parameters());
}
```

### Advanced Training Loop

```rust
fn example_training_loop() {
    let config = MoEAttentionConfig::default();
    let mut model = MoEAttention::new(config);
    let mut monitor = ExpertSpecializationMonitor::new(4);

    let num_epochs = 10;
    let batch_size = 32;
    let seq_len = 128;
    let dim = 512;

    for epoch in 0..num_epochs {
        let mut epoch_loss = 0.0;
        let num_batches = 100;

        for batch in 0..num_batches {
            // Generate dummy data
            let queries = Array3::<f32>::random((batch_size, seq_len, dim), Uniform::new(0.0, 1.0));
            let keys = Array3::<f32>::random((batch_size, seq_len, dim), Uniform::new(0.0, 1.0));
            let values = Array3::<f32>::random((batch_size, seq_len, dim), Uniform::new(0.0, 1.0));

            // Forward pass
            let output = model.forward(queries.view(), keys.view(), values.view(), None);

            // Track expert usage
            monitor.update(output.gates.view());

            // Compute loss (simplified)
            let task_loss = output.output.mapv(|x| x * x).sum() / (batch_size * seq_len) as f32;
            let total_loss = task_loss + output.aux_loss;

            epoch_loss += total_loss;

            // Backward pass and optimization would go here
        }

        println!("Epoch {}: Loss = {:.6}", epoch, epoch_loss / num_batches as f32);

        // Print expert statistics
        if epoch % 5 == 0 {
            let stats = monitor.get_statistics();
            println!("Expert usage: {:?}", stats.avg_usage);
            println!("Expert gates: {:?}", stats.avg_gates);
        }
    }
}
```

## Integration with Graph Neural Networks

```rust
/// Integrate MoE Attention into GNN layer
pub struct MoEGNNLayer {
    moe_attention: MoEAttention,
    node_transform: Array2<f32>,
    edge_encoder: Array2<f32>,
}

impl MoEGNNLayer {
    pub fn forward(
        &mut self,
        node_features: ArrayView3<f32>,
        edge_features: ArrayView3<f32>,
        adjacency: ArrayView2<f32>,
    ) -> Array3<f32> {
        let (batch_size, num_nodes, node_dim) = node_features.dim();

        // Transform node features
        let queries = node_features;
        let keys = node_features;
        let values = node_features;

        // Encode edge features
        // edge_encoded = edge_features.dot(&self.edge_encoder)

        // Apply MoE attention
        let output = self.moe_attention.forward(
            queries,
            keys,
            values,
            Some(edge_features),
        );

        output.output
    }
}
```

## Performance Characteristics

### Computational Complexity

- **Routing**: O(B × S × (D × H + H × E)) where B=batch, S=sequence, D=dim, H=hidden, E=experts
- **Expert Forward**: O(B × S² × D) per expert (for standard attention)
- **Total**: O(B × S² × D × k) where k=top-k experts activated

### Memory Usage

- **Router**: (D × H + H × E) parameters
- **Experts**: ~(3 × D² + D) parameters per expert × E experts
- **Activations**: O(B × S × D × k) during forward pass

### Load Balancing Benefits

- Prevents expert collapse (all tokens routed to one expert)
- Encourages specialization while maintaining coverage
- Typical coefficient: 0.01-0.1 depending on task

## Best Practices

1. **Expert Selection**
   - Start with k=2 for most tasks
   - Increase k for more complex tasks requiring multiple perspectives
   - Monitor expert usage to ensure balanced routing

2. **Temperature Tuning**
   - Start with temperature=1.0
   - Decrease (0.5-0.8) for sharper routing (more specialization)
   - Increase (1.2-2.0) for softer routing (more collaboration)

3. **Load Balancing**
   - Use coefficient 0.01 as baseline
   - Increase if experts are underutilized
   - Decrease if routing is too uniform

4. **Expert Types**
   - Include diverse expert types for different inductive biases
   - Standard for general patterns
   - Hyperbolic for hierarchical structures
   - Linear for long sequences
   - EdgeFeatured for relational data

## References

- Switch Transformers: Scaling to Trillion Parameter Models (Fedus et al., 2021)
- GShard: Scaling Giant Models with Conditional Computation (Lepikhin et al., 2020)
- Mixture-of-Experts with Expert Choice Routing (Zhou et al., 2022)
- ST-MoE: Designing Stable and Transferable Sparse Expert Models (Zoph et al., 2022)
