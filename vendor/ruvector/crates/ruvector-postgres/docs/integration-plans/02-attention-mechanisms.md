# Attention Mechanisms Integration Plan

## Overview

Integrate 39 attention mechanisms from `ruvector-attention` into PostgreSQL, enabling attention-weighted vector search, transformer-style queries, and neural reranking directly in SQL.

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                      PostgreSQL Extension                         │
├──────────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────────┐    │
│  │                  Attention Registry                       │    │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────────────┐ │    │
│  │  │  Flash  │ │ Linear  │ │  MoE    │ │   Hyperbolic    │ │    │
│  │  └────┬────┘ └────┬────┘ └────┬────┘ └────────┬────────┘ │    │
│  └───────┼───────────┼───────────┼───────────────┼──────────┘    │
│          └───────────┴───────────┴───────────────┘               │
│                              ▼                                    │
│              ┌───────────────────────────┐                        │
│              │   SIMD-Accelerated Core   │                        │
│              │   (AVX-512/AVX2/NEON)     │                        │
│              └───────────────────────────┘                        │
└──────────────────────────────────────────────────────────────────┘
```

## Module Structure

```
src/
├── attention/
│   ├── mod.rs              # Module exports & registry
│   ├── core/
│   │   ├── scaled_dot.rs   # Scaled dot-product attention
│   │   ├── multi_head.rs   # Multi-head attention
│   │   ├── flash.rs        # Flash Attention v2
│   │   └── linear.rs       # Linear attention O(n)
│   ├── graph/
│   │   ├── gat.rs          # Graph Attention
│   │   ├── gatv2.rs        # GATv2 (dynamic)
│   │   └── sparse.rs       # Sparse attention patterns
│   ├── specialized/
│   │   ├── moe.rs          # Mixture of Experts
│   │   ├── cross.rs        # Cross-attention
│   │   └── sliding.rs      # Sliding window
│   ├── hyperbolic/
│   │   ├── poincare.rs     # Poincaré attention
│   │   └── lorentz.rs      # Lorentzian attention
│   └── operators.rs        # PostgreSQL operators
```

## SQL Interface

### Basic Attention Operations

```sql
-- Create attention-weighted index
CREATE INDEX ON documents USING ruvector_attention (
    embedding vector(768)
) WITH (
    attention_type = 'flash',
    num_heads = 8,
    head_dim = 96
);

-- Attention-weighted search
SELECT id, content,
       ruvector_attention_score(embedding, query_vec, 'scaled_dot') AS score
FROM documents
ORDER BY score DESC
LIMIT 10;

-- Multi-head attention search
SELECT * FROM ruvector_mha_search(
    table_name := 'documents',
    query := query_embedding,
    num_heads := 8,
    k := 10
);
```

### Advanced Attention Queries

```sql
-- Cross-attention between two tables (Q from queries, K/V from documents)
SELECT q.id AS query_id, d.id AS doc_id, score
FROM ruvector_cross_attention(
    query_table := 'queries',
    query_column := 'embedding',
    document_table := 'documents',
    document_column := 'embedding',
    attention_type := 'scaled_dot'
) AS (query_id int, doc_id int, score float);

-- Mixture of Experts routing
SELECT id,
       ruvector_moe_route(embedding, num_experts := 8, top_k := 2) AS expert_weights
FROM documents;

-- Sliding window attention for long sequences
SELECT * FROM ruvector_sliding_attention(
    embeddings := embedding_array,
    window_size := 256,
    stride := 128
);
```

### Attention Types

```sql
-- List available attention mechanisms
SELECT * FROM ruvector_attention_types();

-- Result:
-- | name              | complexity | best_for                    |
-- |-------------------|------------|-----------------------------|
-- | scaled_dot        | O(n²)      | Small sequences (<512)      |
-- | flash_v2          | O(n²)      | GPU, memory-efficient       |
-- | linear            | O(n)       | Long sequences (>4K)        |
-- | sparse            | O(n√n)     | Very long sequences         |
-- | gat               | O(E)       | Graph-structured data       |
-- | moe               | O(n*k)     | Conditional computation     |
-- | hyperbolic        | O(n²)      | Hierarchical data           |
```

## Implementation Phases

### Phase 1: Core Attention (Week 1-3)

```rust
// src/attention/core/scaled_dot.rs

use simsimd::SpatialSimilarity;

pub struct ScaledDotAttention {
    scale: f32,
    dropout: Option<f32>,
}

impl ScaledDotAttention {
    pub fn new(head_dim: usize) -> Self {
        Self {
            scale: 1.0 / (head_dim as f32).sqrt(),
            dropout: None,
        }
    }

    /// Compute attention scores between query and keys
    /// Returns softmax(Q·K^T / √d_k)
    #[inline]
    pub fn attention_scores(&self, query: &[f32], keys: &[&[f32]]) -> Vec<f32> {
        let mut scores: Vec<f32> = keys.iter()
            .map(|k| self.dot_product(query, k) * self.scale)
            .collect();

        softmax_inplace(&mut scores);
        scores
    }

    /// SIMD-accelerated dot product
    #[inline]
    fn dot_product(&self, a: &[f32], b: &[f32]) -> f32 {
        f32::dot(a, b).unwrap_or_else(|| {
            a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
        })
    }
}

// PostgreSQL function
#[pg_extern(immutable, parallel_safe)]
fn ruvector_attention_score(
    query: Vec<f32>,
    key: Vec<f32>,
    attention_type: default!(&str, "'scaled_dot'"),
) -> f32 {
    let attention = get_attention_impl(attention_type);
    attention.score(&query, &key)
}
```

### Phase 2: Multi-Head Attention (Week 4-5)

```rust
// src/attention/core/multi_head.rs

pub struct MultiHeadAttention {
    num_heads: usize,
    head_dim: usize,
    w_q: Matrix,
    w_k: Matrix,
    w_v: Matrix,
    w_o: Matrix,
}

impl MultiHeadAttention {
    pub fn forward(&self, query: &[f32], keys: &[&[f32]], values: &[&[f32]]) -> Vec<f32> {
        // Project to heads
        let q_heads = self.split_heads(&self.project(query, &self.w_q));
        let k_heads: Vec<_> = keys.iter()
            .map(|k| self.split_heads(&self.project(k, &self.w_k)))
            .collect();
        let v_heads: Vec<_> = values.iter()
            .map(|v| self.split_heads(&self.project(v, &self.w_v)))
            .collect();

        // Attention per head (parallelizable)
        let head_outputs: Vec<Vec<f32>> = (0..self.num_heads)
            .into_par_iter()
            .map(|h| {
                let scores = self.attention_scores(&q_heads[h], &k_heads, h);
                self.weighted_sum(&scores, &v_heads, h)
            })
            .collect();

        // Concatenate and project
        let concat = self.concat_heads(&head_outputs);
        self.project(&concat, &self.w_o)
    }
}

// PostgreSQL aggregate for batch attention
#[pg_extern]
fn ruvector_mha_search(
    table_name: &str,
    query: Vec<f32>,
    num_heads: default!(i32, 8),
    k: default!(i32, 10),
) -> TableIterator<'static, (name!(id, i64), name!(score, f32))> {
    // Implementation using SPI
}
```

### Phase 3: Flash Attention (Week 6-7)

```rust
// src/attention/core/flash.rs

/// Flash Attention v2 - memory-efficient attention
/// Processes attention in blocks to minimize memory bandwidth
pub struct FlashAttention {
    block_size_q: usize,
    block_size_kv: usize,
    scale: f32,
}

impl FlashAttention {
    /// Tiled attention computation
    /// Memory: O(√N) instead of O(N²)
    pub fn forward(
        &self,
        q: &[f32],      // [seq_len, head_dim]
        k: &[f32],      // [seq_len, head_dim]
        v: &[f32],      // [seq_len, head_dim]
    ) -> Vec<f32> {
        let seq_len = q.len() / self.head_dim;
        let mut output = vec![0.0; q.len()];
        let mut row_max = vec![f32::NEG_INFINITY; seq_len];
        let mut row_sum = vec![0.0; seq_len];

        // Process in blocks
        for q_block in (0..seq_len).step_by(self.block_size_q) {
            for kv_block in (0..seq_len).step_by(self.block_size_kv) {
                self.process_block(
                    q, k, v,
                    q_block, kv_block,
                    &mut output, &mut row_max, &mut row_sum
                );
            }
        }

        output
    }
}
```

### Phase 4: Graph Attention (Week 8-9)

```rust
// src/attention/graph/gat.rs

/// Graph Attention Network layer
pub struct GATLayer {
    num_heads: usize,
    in_features: usize,
    out_features: usize,
    attention_weights: Vec<Vec<f32>>,  // [num_heads, 2 * out_features]
    leaky_relu_slope: f32,
}

impl GATLayer {
    /// Compute attention coefficients for graph edges
    pub fn forward(
        &self,
        node_features: &[Vec<f32>],  // [num_nodes, in_features]
        edge_index: &[(usize, usize)],  // [(src, dst), ...]
    ) -> Vec<Vec<f32>> {
        // Transform features
        let h = self.linear_transform(node_features);

        // Compute attention for each edge
        let edge_attention: Vec<Vec<f32>> = edge_index.par_iter()
            .map(|(src, dst)| {
                (0..self.num_heads)
                    .map(|head| self.edge_attention(head, &h[*src], &h[*dst]))
                    .collect()
            })
            .collect();

        // Aggregate with attention weights
        self.aggregate(&h, edge_index, &edge_attention)
    }
}

// PostgreSQL function for graph-based search
#[pg_extern]
fn ruvector_gat_search(
    node_table: &str,
    edge_table: &str,
    query_node_id: i64,
    num_heads: default!(i32, 4),
    k: default!(i32, 10),
) -> TableIterator<'static, (name!(node_id, i64), name!(attention_score, f32))> {
    // Implementation
}
```

### Phase 5: Hyperbolic Attention (Week 10-11)

```rust
// src/attention/hyperbolic/poincare.rs

/// Poincaré ball attention for hierarchical data
pub struct PoincareAttention {
    curvature: f32,  // -1/c² where c is the ball radius
    head_dim: usize,
}

impl PoincareAttention {
    /// Möbius addition in Poincaré ball
    fn mobius_add(&self, x: &[f32], y: &[f32]) -> Vec<f32> {
        let x_norm_sq = self.norm_sq(x);
        let y_norm_sq = self.norm_sq(y);
        let xy_dot = self.dot(x, y);

        let c = -self.curvature;
        let num_coef = 1.0 + 2.0 * c * xy_dot + c * y_norm_sq;
        let denom = 1.0 + 2.0 * c * xy_dot + c * c * x_norm_sq * y_norm_sq;

        x.iter().zip(y.iter())
            .map(|(xi, yi)| (num_coef * xi + (1.0 - c * x_norm_sq) * yi) / denom)
            .collect()
    }

    /// Hyperbolic distance
    fn distance(&self, x: &[f32], y: &[f32]) -> f32 {
        let diff = self.mobius_add(x, &self.negate(y));
        let c = -self.curvature;
        let norm = self.norm(&diff);
        (2.0 / c.sqrt()) * (c.sqrt() * norm).atanh()
    }

    /// Attention in hyperbolic space
    pub fn attention_scores(&self, query: &[f32], keys: &[&[f32]]) -> Vec<f32> {
        let distances: Vec<f32> = keys.iter()
            .map(|k| -self.distance(query, k))  // Negative distance as similarity
            .collect();

        softmax(&distances)
    }
}

#[pg_extern(immutable, parallel_safe)]
fn ruvector_hyperbolic_distance(
    a: Vec<f32>,
    b: Vec<f32>,
    curvature: default!(f32, 1.0),
) -> f32 {
    let attention = PoincareAttention::new(curvature, a.len());
    attention.distance(&a, &b)
}
```

### Phase 6: Mixture of Experts (Week 12)

```rust
// src/attention/specialized/moe.rs

/// Mixture of Experts with learned routing
pub struct MixtureOfExperts {
    num_experts: usize,
    top_k: usize,
    gate: GatingNetwork,
    experts: Vec<Expert>,
}

impl MixtureOfExperts {
    /// Route input to top-k experts
    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        // Get routing weights
        let gate_logits = self.gate.forward(input);
        let (top_k_indices, top_k_weights) = self.top_k_gating(&gate_logits);

        // Aggregate expert outputs
        let mut output = vec![0.0; self.experts[0].output_dim()];
        for (idx, weight) in top_k_indices.iter().zip(top_k_weights.iter()) {
            let expert_output = self.experts[*idx].forward(input);
            for (o, e) in output.iter_mut().zip(expert_output.iter()) {
                *o += weight * e;
            }
        }

        output
    }
}

#[pg_extern]
fn ruvector_moe_route(
    embedding: Vec<f32>,
    num_experts: default!(i32, 8),
    top_k: default!(i32, 2),
) -> pgrx::JsonB {
    let moe = get_moe_model(num_experts as usize, top_k as usize);
    let (indices, weights) = moe.route(&embedding);

    pgrx::JsonB(serde_json::json!({
        "expert_indices": indices,
        "expert_weights": weights,
    }))
}
```

## Attention Type Registry

```rust
// src/attention/mod.rs

pub enum AttentionType {
    // Core
    ScaledDot,
    MultiHead { num_heads: usize },
    FlashV2 { block_size: usize },
    Linear,

    // Graph
    GAT { num_heads: usize },
    GATv2 { num_heads: usize },
    Sparse { pattern: SparsePattern },

    // Specialized
    MoE { num_experts: usize, top_k: usize },
    Cross,
    SlidingWindow { size: usize },

    // Hyperbolic
    Poincare { curvature: f32 },
    Lorentz { curvature: f32 },
}

pub fn get_attention(attention_type: AttentionType) -> Box<dyn Attention> {
    match attention_type {
        AttentionType::ScaledDot => Box::new(ScaledDotAttention::default()),
        AttentionType::FlashV2 { block_size } => Box::new(FlashAttention::new(block_size)),
        // ... etc
    }
}
```

## Performance Optimizations

### SIMD Acceleration

```rust
// Use simsimd for all vector operations
use simsimd::{SpatialSimilarity, BinarySimilarity};

#[inline]
fn batched_dot_products(query: &[f32], keys: &[&[f32]]) -> Vec<f32> {
    keys.iter()
        .map(|k| f32::dot(query, k).unwrap())
        .collect()
}
```

### Memory Layout

```rust
// Contiguous memory for cache efficiency
pub struct AttentionCache {
    // Keys stored in column-major for efficient attention
    keys: Vec<f32>,      // [num_keys * head_dim]
    values: Vec<f32>,    // [num_keys * head_dim]
    num_keys: usize,
    head_dim: usize,
}
```

### Parallel Processing

```rust
// Parallel attention across heads
let head_outputs: Vec<_> = (0..num_heads)
    .into_par_iter()
    .map(|h| compute_head_attention(h, query, keys, values))
    .collect();
```

## Benchmarks

| Operation | Sequence Length | Heads | Time (μs) | Memory |
|-----------|-----------------|-------|-----------|--------|
| ScaledDot | 512 | 8 | 45 | 2MB |
| Flash | 512 | 8 | 38 | 0.5MB |
| Linear | 4096 | 8 | 120 | 4MB |
| GAT | 1000 nodes | 4 | 85 | 1MB |
| MoE (8 experts) | 512 | 8 | 95 | 3MB |

## Dependencies

```toml
[dependencies]
# Link to ruvector-attention for implementations
ruvector-attention = { path = "../ruvector-attention", optional = true }

# SIMD
simsimd = "5.9"

# Parallel processing
rayon = "1.10"

# Matrix operations (optional, for weight matrices)
ndarray = { version = "0.15", optional = true }
```

## Feature Flags

```toml
[features]
attention = []
attention-flash = ["attention"]
attention-graph = ["attention"]
attention-hyperbolic = ["attention"]
attention-moe = ["attention"]
attention-all = ["attention-flash", "attention-graph", "attention-hyperbolic", "attention-moe"]
```
