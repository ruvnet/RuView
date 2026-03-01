# Alternative Attention Mechanisms for GNN Latent Space

## Executive Summary

This document explores alternative attention mechanisms beyond the current scaled dot-product multi-head attention used in RuVector. We analyze mechanisms that could better bridge the gap between high-dimensional latent spaces and graph topology, with emphasis on efficiency, expressiveness, and geometric awareness.

**Current**: Multi-head scaled dot-product attention (O(n²) complexity)
**Goal**: Enhance attention to capture graph structure, reduce complexity, and improve latent-graph interplay

---

## 1. Current Attention Mechanism Analysis

### 1.1 Scaled Dot-Product Attention (Current Implementation)

**File**: `crates/ruvector-gnn/src/layer.rs:84-205`

```
Attention(Q, K, V) = softmax(QK^T / √d_k) V
```

**Strengths**:
- ✓ Permutation invariant
- ✓ Differentiable
- ✓ Well-understood training dynamics
- ✓ Parallel computation

**Weaknesses**:
- ✗ No explicit edge features
- ✗ No positional/structural encoding
- ✗ Uniform geometric assumptions (Euclidean)
- ✗ O(d·h²) computational cost
- ✗ Attention scores independent of graph topology

### 1.2 Multi-Head Decomposition (Current)

```
MultiHead(Q, K, V) = Concat(head_1, ..., head_h) W_o
```

**Strengths**:
- ✓ Multiple representation subspaces
- ✓ Different aspects of neighborhood

**Weaknesses**:
- ✗ Fixed number of heads
- ✗ Heads learn similar patterns (redundancy)
- ✗ No explicit head specialization

---

## 2. Graph Attention Networks (GAT) Extensions

### 2.1 Edge-Featured Attention

**Key Innovation**: Incorporate edge attributes into attention computation

```
e_{ij} = LeakyReLU(a^T [W h_i || W h_j || W_e edge_{ij}])
α_{ij} = softmax_j(e_{ij})
h'_i = σ(Σ_{j∈N(i)} α_{ij} W h_j)
```

**Implementation Proposal**:

```rust
pub struct EdgeFeaturedAttention {
    w_node: Linear,      // Node transformation
    w_edge: Linear,      // Edge transformation
    a: Vec<f32>,         // Attention coefficients
    activation: LeakyReLU,
}

impl EdgeFeaturedAttention {
    fn forward(
        &self,
        query_node: &[f32],
        neighbor_nodes: &[Vec<f32>],
        edge_features: &[Vec<f32>],  // NEW
    ) -> Vec<f32> {
        // 1. Transform nodes and edges
        let q_trans = self.w_node.forward(query_node);
        let n_trans: Vec<_> = neighbor_nodes.iter()
            .map(|n| self.w_node.forward(n))
            .collect();
        let e_trans: Vec<_> = edge_features.iter()
            .map(|e| self.w_edge.forward(e))
            .collect();

        // 2. Compute attention with edge features
        let mut scores = Vec::new();
        for (n, e) in n_trans.iter().zip(e_trans.iter()) {
            // Concatenate [query || neighbor || edge]
            let concat = [&q_trans[..], &n[..], &e[..]].concat();
            let score = dot_product(&self.a, &concat);
            scores.push(self.activation.forward(score));
        }

        // 3. Softmax and aggregate
        let weights = softmax(&scores);
        weighted_sum(&n_trans, &weights)
    }
}
```

**Benefits for RuVector**:
- Edge weights (distances) become learnable features
- HNSW layer information can be encoded in edges
- Better captures graph topology in latent space

**Complexity**: O(d·(h_node + h_edge + h_attn))

---

## 3. Hyperbolic Attention

### 3.1 Motivation

**Problem**: HNSW has hierarchical structure, but Euclidean space poorly represents trees/hierarchies

**Solution**: Operate in hyperbolic space (Poincaré ball or hyperboloid model)

### 3.2 Poincaré Ball Attention

**Poincaré Ball Model**:
```
B^d = {x ∈ R^d : ||x|| < 1}
Distance: d(x, y) = arcosh(1 + 2||x - y||² / ((1-||x||²)(1-||y||²)))
```

**Hyperbolic Attention Mechanism**:

```
# Key differences from Euclidean:
1. Use hyperbolic distance for similarity
2. Exponential map for transformations
3. Logarithmic map for aggregation

HyperbolicAttention(q, k, v):
    # Compute hyperbolic similarity
    sim_ij = -d_poincare(q, k_j)  # Negative distance

    # Softmax in tangent space
    α_ij = softmax(sim_ij / τ)

    # Aggregate in hyperbolic space
    result = ⊕_{j} (α_ij ⊗ v_j)  # Möbius addition

    return result
```

**Implementation Sketch**:

```rust
pub struct HyperbolicAttention {
    curvature: f32,  // Negative curvature (e.g., -1.0)
}

impl HyperbolicAttention {
    // Poincaré distance
    fn poincare_distance(&self, x: &[f32], y: &[f32]) -> f32 {
        let diff_norm_sq = l2_norm_squared(&subtract(x, y));
        let x_norm_sq = l2_norm_squared(x);
        let y_norm_sq = l2_norm_squared(y);

        let numerator = 2.0 * diff_norm_sq;
        let denominator = (1.0 - x_norm_sq) * (1.0 - y_norm_sq);

        self.curvature.abs().sqrt() * (1.0 + numerator / denominator).acosh()
    }

    // Möbius addition (hyperbolic vector addition)
    fn mobius_add(&self, x: &[f32], y: &[f32]) -> Vec<f32> {
        let x_norm_sq = l2_norm_squared(x);
        let y_norm_sq = l2_norm_squared(y);
        let xy_dot = dot_product(x, y);

        let numerator_coef = (1.0 + 2.0*xy_dot + y_norm_sq) / (1.0 - x_norm_sq);
        let denominator_coef = (1.0 + 2.0*xy_dot + x_norm_sq*y_norm_sq) / (1.0 - x_norm_sq);

        // (1+2⟨x,y⟩+||y||²)x + (1-||x||²)y / (1+2⟨x,y⟩+||x||²||y||²)
        let numerator = add(
            &scale(x, numerator_coef),
            &scale(y, 1.0 - x_norm_sq)
        );
        scale(&numerator, 1.0 / denominator_coef)
    }

    fn forward(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Vec<f32> {
        // 1. Compute hyperbolic similarities (negative distances)
        let scores: Vec<f32> = keys.iter()
            .map(|k| -self.poincare_distance(query, k))
            .collect();

        // 2. Softmax
        let weights = softmax(&scores);

        // 3. Hyperbolic aggregation
        let mut result = vec![0.0; values[0].len()];
        for (v, &w) in values.iter().zip(weights.iter()) {
            let scaled = self.mobius_scalar_mult(w, v);
            result = self.mobius_add(&result, &scaled);
        }

        result
    }
}
```

**Benefits for HNSW**:
- Natural representation of hierarchical layers
- Exponential capacity (tree-like structures)
- Distance preserves hierarchy

**Challenges**:
- Numerical instability near ball boundary (||x|| → 1)
- More complex backpropagation
- Requires hyperbolic embeddings throughout pipeline

---

## 4. Sparse Attention Patterns

### 4.1 Local + Global Attention (Longformer-style)

**Motivation**: Full attention is O(n²), wasteful for graphs with local structure

**Pattern**:
```
Attention Matrix Structure:
  [L L L G 0 0 0 0]
  [L L L L G 0 0 0]
  [L L L L L G 0 0]
  [G L L L L L G 0]
  [0 G L L L L L G]
  [0 0 G L L L L L]
  [0 0 0 G L L L L]
  [0 0 0 0 G L L L]

L = Local attention (1-hop neighbors)
G = Global attention (HNSW higher layers)
0 = No attention
```

**Implementation**:

```rust
pub struct SparseGraphAttention {
    local_attn: MultiHeadAttention,
    global_attn: MultiHeadAttention,
    local_window: usize,  // K-hop neighborhood
}

impl SparseGraphAttention {
    fn forward(
        &self,
        query: &[f32],
        neighbor_embeddings: &[Vec<f32>],
        neighbor_layers: &[usize],  // HNSW layer for each neighbor
    ) -> Vec<f32> {
        // Split neighbors by locality
        let (local_neighbors, local_indices): (Vec<_>, Vec<_>) =
            neighbor_embeddings.iter().enumerate()
                .filter(|(i, _)| neighbor_layers[*i] == 0)  // Layer 0 = local
                .unzip();

        let (global_neighbors, global_indices): (Vec<_>, Vec<_>) =
            neighbor_embeddings.iter().enumerate()
                .filter(|(i, _)| neighbor_layers[*i] > 0)  // Higher layers = global
                .unzip();

        // Compute local attention
        let local_output = if !local_neighbors.is_empty() {
            self.local_attn.forward(query, &local_neighbors, &local_neighbors)
        } else {
            vec![0.0; query.len()]
        };

        // Compute global attention
        let global_output = if !global_neighbors.is_empty() {
            self.global_attn.forward(query, &global_neighbors, &global_neighbors)
        } else {
            vec![0.0; query.len()]
        };

        // Combine (learned gating)
        combine_local_global(&local_output, &global_output)
    }
}
```

**Complexity**: O(k_local + k_global) instead of O(n²)

---

## 5. Linear Attention (O(n) complexity)

### 5.1 Kernel-Based Linear Attention

**Key Idea**: Replace softmax with kernel feature map

```
Standard: Attention(Q, K, V) = softmax(QK^T) V
Linear:   Attention(Q, K, V) = φ(Q) (φ(K)^T V) / (φ(Q) (φ(K)^T 1))

where φ: R^d → R^D is a feature map
```

**Random Feature Approximation** (Performer):

```rust
pub struct LinearAttention {
    num_features: usize,  // D (typically 256-512)
    random_features: Array2<f32>,  // Random projection matrix
}

impl LinearAttention {
    fn feature_map(&self, x: &[f32]) -> Vec<f32> {
        // Random Fourier Features
        let proj = self.random_features.dot(&Array1::from_vec(x.to_vec()));
        let scale = 1.0 / (self.num_features as f32).sqrt();

        proj.mapv(|z| {
            scale * (z.cos() + z.sin())  // Simplified RFF
        }).to_vec()
    }

    fn forward(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Vec<f32> {
        // 1. Apply feature map
        let q_feat = self.feature_map(query);
        let k_feats: Vec<_> = keys.iter().map(|k| self.feature_map(k)).collect();

        // 2. Compute K^T V (sum over neighbors)
        let mut kv = vec![0.0; values[0].len()];
        for (k_feat, v) in k_feats.iter().zip(values.iter()) {
            for (i, &v_i) in v.iter().enumerate() {
                kv[i] += k_feat.iter().sum::<f32>() * v_i;
            }
        }

        // 3. Compute Q (K^T V)
        let numerator: Vec<f32> = kv.iter()
            .map(|&kv_i| q_feat.iter().sum::<f32>() * kv_i)
            .collect();

        // 4. Normalize by Q (K^T 1)
        let denominator: f32 = q_feat.iter().sum::<f32>()
            * k_feats.iter().map(|k| k.iter().sum::<f32>()).sum::<f32>();

        numerator.iter().map(|&n| n / denominator).collect()
    }
}
```

**Benefits**:
- **O(n) complexity**: Scales linearly with graph size
- **Theoretically grounded**: Approximates softmax attention
- **Parallel friendly**: Matrix operations

**Tradeoffs**:
- Approximation error vs. exact softmax
- Requires more random features for accuracy
- Less interpretable attention weights

---

## 6. Rotary Position Embeddings (RoPE) for Graphs

### 6.1 Motivation

**Problem**: Graph attention has no notion of "position" or "distance" beyond explicit edge features

**Solution**: Encode relative distances/positions via rotation

### 6.2 RoPE Mathematics

**Standard RoPE** (for sequences):
```
RoPE(x, m) = [
    x₀ cos(mθ₀) - x₁ sin(mθ₀),
    x₀ sin(mθ₀) + x₁ cos(mθ₀),
    x₂ cos(mθ₁) - x₃ sin(mθ₁),
    ...
]

where m = position index, θᵢ = 10000^(-2i/d)
```

**Graph RoPE Adaptation**:
```
Instead of sequential position m, use:
- Graph distance (shortest path length)
- HNSW layer index
- Normalized edge weight
```

**Implementation**:

```rust
pub struct GraphRoPE {
    dim: usize,
    base: f32,  // Base frequency (default 10000)
}

impl GraphRoPE {
    fn apply_rotation(&self, embedding: &[f32], distance: f32) -> Vec<f32> {
        let mut rotated = vec![0.0; embedding.len()];

        for i in (0..self.dim).step_by(2) {
            let theta = distance / self.base.powf(2.0 * i as f32 / self.dim as f32);
            let cos_theta = theta.cos();
            let sin_theta = theta.sin();

            rotated[i] = embedding[i] * cos_theta - embedding[i+1] * sin_theta;
            rotated[i+1] = embedding[i] * sin_theta + embedding[i+1] * cos_theta;
        }

        rotated
    }

    fn forward_attention(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
        distances: &[f32],  // NEW: graph distances
    ) -> Vec<f32> {
        // Apply RoPE to query and keys based on relative distance
        let q_rotated = self.apply_rotation(query, 0.0);  // Query at "origin"

        let mut scores = Vec::new();
        for (k, &dist) in keys.iter().zip(distances.iter()) {
            let k_rotated = self.apply_rotation(k, dist);
            let score = dot_product(&q_rotated, &k_rotated);
            scores.push(score);
        }

        let weights = softmax(&scores);
        weighted_sum(values, &weights)
    }
}
```

**Benefits**:
- Encodes distance without explicit features
- Relative position encoding (rotation-invariant)
- Efficient (just rotations, no extra parameters)

**Graph-Specific Applications**:
1. **HNSW Layer Distance**: Encode which layer neighbors come from
2. **Shortest Path Distance**: Penalize far nodes in latent space
3. **Edge Weight Encoding**: Continuous rotation based on edge weight

---

## 7. Flash Attention (Memory-Efficient)

### 7.1 Problem

Standard attention materializes the full attention matrix in memory:
```
Memory: O(n²)  for n neighbors
```

For dense graphs or large neighborhoods, this is prohibitive.

### 7.2 Flash Attention Algorithm

**Key Ideas**:
1. Tile the attention computation
2. Recompute attention on-the-fly during backward pass
3. Never materialize full attention matrix

**Pseudocode**:

```
FlashAttention(Q, K, V):
    # Divide Q, K, V into blocks
    Q_blocks = split(Q, block_size)
    K_blocks = split(K, block_size)
    V_blocks = split(V, block_size)

    O = zeros_like(Q)

    # Outer loop: iterate over query blocks
    for Q_i in Q_blocks:
        row_max = -inf
        row_sum = 0

        # Inner loop: iterate over key blocks
        for K_j, V_j in zip(K_blocks, V_blocks):
            # Compute attention block
            S_ij = Q_i @ K_j^T / sqrt(d)

            # Online softmax (numerically stable)
            new_max = max(row_max, max(S_ij))
            exp_S = exp(S_ij - new_max)

            # Update running statistics
            correction = exp(row_max - new_max)
            row_sum = row_sum * correction + sum(exp_S)
            row_max = new_max

            # Accumulate output
            O_i += exp_S @ V_j

        # Final normalization
        O_i /= row_sum

    return O
```

**Implementation Note**:

Flash Attention requires careful low-level optimization (CUDA kernels, tiling, SRAM management). For RuVector:

```rust
// Simplified tiled version for CPU
pub struct TiledAttention {
    block_size: usize,
}

impl TiledAttention {
    fn forward_tiled(
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

            // Compute attention for this block
            let chunk_keys = &keys[chunk_start..chunk_end];
            let chunk_values = &values[chunk_start..chunk_end];

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

**Benefits**:
- **Memory**: O(n) instead of O(n²)
- **Speed**: Can be faster due to better cache locality
- **Scalability**: Handle larger neighborhoods

---

## 8. Mixture of Experts (MoE) Attention

### 8.1 Concept

Different attention mechanisms for different graph patterns:

```
MoE-Attention(query, keys, values):
    # Router decides which expert(s) to use
    router_scores = Router(query)
    expert_indices = topk(router_scores, k=2)

    # Apply selected experts
    outputs = []
    for expert_idx in expert_indices:
        expert_output = Experts[expert_idx](query, keys, values)
        outputs.append(expert_output * router_scores[expert_idx])

    return sum(outputs)
```

**Graph-Specific Experts**:
1. **Local Expert**: For 1-hop neighbors (standard attention)
2. **Hierarchical Expert**: For HNSW higher layers (hyperbolic attention)
3. **Global Expert**: For distant nodes (linear attention)
4. **Structural Expert**: Edge-featured attention

### 8.2 Implementation

```rust
pub enum AttentionExpert {
    Standard(MultiHeadAttention),
    Hyperbolic(HyperbolicAttention),
    Linear(LinearAttention),
    EdgeFeatured(EdgeFeaturedAttention),
}

pub struct MoEAttention {
    router: Linear,  // Maps query to expert scores
    experts: Vec<AttentionExpert>,
    top_k: usize,
}

impl MoEAttention {
    fn forward(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
        edge_features: Option<&[Vec<f32>]>,
    ) -> Vec<f32> {
        // 1. Route to experts
        let router_scores = self.router.forward(query);
        let expert_weights = softmax(&router_scores);
        let top_experts = topk_indices(&expert_weights, self.top_k);

        // 2. Compute weighted expert outputs
        let mut output = vec![0.0; query.len()];
        for &expert_idx in &top_experts {
            let expert_output = match &self.experts[expert_idx] {
                AttentionExpert::Standard(attn) =>
                    attn.forward(query, keys, values),
                AttentionExpert::Hyperbolic(attn) =>
                    attn.forward(query, keys, values),
                AttentionExpert::Linear(attn) =>
                    attn.forward(query, keys, values),
                AttentionExpert::EdgeFeatured(attn) =>
                    attn.forward(query, keys, values, edge_features.unwrap()),
            };

            let weight = expert_weights[expert_idx];
            for (o, &e) in output.iter_mut().zip(expert_output.iter()) {
                *o += weight * e;
            }
        }

        output
    }
}
```

**Benefits**:
- Adaptive to different graph neighborhoods
- Specialization reduces computation
- Router learns which mechanism suits which context

---

## 9. Cross-Attention Between Graph and Latent

### 9.1 Motivation

**Problem**: Current attention only looks at graph neighbors. What about latent space neighbors?

**Solution**: Cross-attention between topological neighbors (graph) and semantic neighbors (latent)

### 9.2 Dual-Space Attention

```
Given node v:
- Graph neighbors: N_G(v) = {u : (u,v) ∈ E}
- Latent neighbors: N_L(v) = TopK({u : sim(h_u, h_v) > threshold})

CrossAttention(v):
    # Graph attention
    graph_out = Attention(h_v, {h_u}_{u∈N_G}, {h_u}_{u∈N_G})

    # Latent attention
    latent_out = Attention(h_v, {h_u}_{u∈N_L}, {h_u}_{u∈N_L})

    # Cross-attention: graph queries latent
    cross_out = Attention(graph_out, {h_u}_{u∈N_L}, {h_u}_{u∈N_L})

    # Fusion
    return Combine(graph_out, latent_out, cross_out)
```

**Implementation**:

```rust
pub struct DualSpaceAttention {
    graph_attn: MultiHeadAttention,
    latent_attn: MultiHeadAttention,
    cross_attn: MultiHeadAttention,
    fusion: Linear,
}

impl DualSpaceAttention {
    fn forward(
        &self,
        query: &[f32],
        graph_neighbors: &[Vec<f32>],
        all_embeddings: &[Vec<f32>],  // For latent neighbor search
        k_latent: usize,
    ) -> Vec<f32> {
        // 1. Graph attention (topology-based)
        let graph_output = self.graph_attn.forward(
            query,
            graph_neighbors,
            graph_neighbors
        );

        // 2. Find latent neighbors (similarity-based)
        let latent_neighbors = self.find_latent_neighbors(
            query,
            all_embeddings,
            k_latent
        );

        // 3. Latent attention (embedding-based)
        let latent_output = self.latent_attn.forward(
            query,
            &latent_neighbors,
            &latent_neighbors
        );

        // 4. Cross-attention (graph context attends to latent space)
        let cross_output = self.cross_attn.forward(
            &graph_output,
            &latent_neighbors,
            &latent_neighbors
        );

        // 5. Fusion
        let concatenated = [
            &graph_output[..],
            &latent_output[..],
            &cross_output[..],
        ].concat();

        self.fusion.forward(&concatenated)
    }

    fn find_latent_neighbors(
        &self,
        query: &[f32],
        all_embeddings: &[Vec<f32>],
        k: usize,
    ) -> Vec<Vec<f32>> {
        // Compute similarities
        let mut similarities: Vec<(usize, f32)> = all_embeddings
            .iter()
            .enumerate()
            .map(|(i, emb)| (i, cosine_similarity(query, emb)))
            .collect();

        // Sort by similarity
        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Return top-k
        similarities.iter()
            .take(k)
            .map(|(i, _)| all_embeddings[*i].clone())
            .collect()
    }
}
```

**Benefits**:
- Bridges topology and semantics
- Captures "similar but not connected" nodes
- Enriches latent space with graph structure

---

## 10. Comparison Matrix

| Mechanism | Complexity | Edge Features | Geometry | Memory | Use Case |
|-----------|------------|---------------|----------|--------|----------|
| **Current (MHA)** | O(d·h²) | ✗ | Euclidean | O(d·h) | General purpose |
| **GAT + Edges** | O(d·h²) | ✓ | Euclidean | O(d·h) | Rich edge info |
| **Hyperbolic** | O(d·h²) | ✗ | Hyperbolic | O(d·h) | Hierarchical graphs |
| **Sparse (Local+Global)** | O(k_l + k_g) | ✗ | Euclidean | O((k_l+k_g)·h) | Large graphs |
| **Linear (Performer)** | O(d·D) | ✗ | Euclidean | O(D·h) | Scalability |
| **RoPE** | O(d·h²) | Implicit | Euclidean | O(d·h) | Distance encoding |
| **Flash Attention** | O(d·h²) | ✗ | Euclidean | O(h) | Memory efficiency |
| **MoE** | Variable | ✓ | Mixed | Variable | Heterogeneous graphs |
| **Cross (Dual-Space)** | O(d·h² + k²·h) | ✗ | Dual | O((d+k)·h) | Latent-graph bridge |

---

## 11. Recommendations for RuVector

### 11.1 Short-Term (Immediate Implementation)

**1. Edge-Featured Attention**
- **Priority**: HIGH
- **Effort**: LOW-MEDIUM
- **Reason**: HNSW edge weights are currently underutilized
- **Implementation**: Extend current `MultiHeadAttention` to include edge features

**2. Sparse Attention (Local + Global)**
- **Priority**: HIGH
- **Effort**: MEDIUM
- **Reason**: Natural fit for HNSW's layered structure
- **Implementation**: Separate attention for layer 0 (local) vs. higher layers (global)

**3. RoPE for Distance Encoding**
- **Priority**: MEDIUM
- **Effort**: LOW
- **Reason**: Encode HNSW layer or edge distance without extra parameters
- **Implementation**: Apply rotation based on layer index or edge weight

### 11.2 Medium-Term (Next Quarter)

**4. Linear Attention (Performer)**
- **Priority**: MEDIUM
- **Effort**: MEDIUM-HIGH
- **Reason**: Scalability for large graphs
- **Implementation**: Replace softmax with random feature approximation

**5. Flash Attention**
- **Priority**: LOW-MEDIUM
- **Effort**: HIGH
- **Reason**: Memory efficiency for dense neighborhoods
- **Implementation**: Tiled computation, may need GPU optimization

### 11.3 Long-Term (Research Exploration)

**6. Hyperbolic Attention**
- **Priority**: MEDIUM
- **Effort**: HIGH
- **Reason**: Hierarchical HNSW structure naturally hyperbolic
- **Implementation**: Full pipeline change to hyperbolic embeddings

**7. Mixture of Experts**
- **Priority**: LOW
- **Effort**: HIGH
- **Reason**: Heterogeneous graph patterns
- **Implementation**: Multiple attention types with learned routing

**8. Cross-Attention (Dual-Space)**
- **Priority**: HIGH (Research)
- **Effort**: HIGH
- **Reason**: Core to latent-graph interplay
- **Implementation**: Requires efficient latent neighbor search (ANN)

---

## 12. Implementation Roadmap

### Phase 1: Extend Current Attention (1-2 weeks)
```rust
// Add edge features to existing MultiHeadAttention
impl MultiHeadAttention {
    pub fn forward_with_edges(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
        edge_features: &[Vec<f32>],  // NEW
    ) -> Vec<f32> {
        // Modify attention score computation to include edges
    }
}
```

### Phase 2: Sparse Attention Variant (2-3 weeks)
```rust
// Separate local and global attention based on HNSW layer
pub struct HNSWAwareAttention {
    local: MultiHeadAttention,
    global: MultiHeadAttention,
}
```

### Phase 3: Alternative Mechanisms (1-2 months)
- Implement RoPE for distance encoding
- Prototype Linear Attention
- Benchmark all variants

### Phase 4: Research Exploration (Ongoing)
- Hyperbolic embeddings (full pipeline change)
- MoE attention routing
- Cross-attention with latent neighbors

---

## References

### Papers
1. **GAT**: Veličković et al. (2018) - Graph Attention Networks
2. **Hyperbolic**: Chami et al. (2019) - Hyperbolic Graph Convolutional Neural Networks
3. **Longformer**: Beltagy et al. (2020) - Longformer: The Long-Document Transformer
4. **Performer**: Choromanski et al. (2020) - Rethinking Attention with Performers
5. **RoPE**: Su et al. (2021) - RoFormer: Enhanced Transformer with Rotary Position Embedding
6. **Flash Attention**: Dao et al. (2022) - FlashAttention: Fast and Memory-Efficient Exact Attention
7. **MoE**: Shazeer et al. (2017) - Outrageously Large Neural Networks: The Sparsely-Gated MoE

### RuVector Code References
- `crates/ruvector-gnn/src/layer.rs:84-205` - Current MultiHeadAttention
- `crates/ruvector-gnn/src/search.rs:38-86` - Differentiable search with softmax

---

**Document Version**: 1.0
**Last Updated**: 2025-11-30
**Author**: RuVector Research Team
