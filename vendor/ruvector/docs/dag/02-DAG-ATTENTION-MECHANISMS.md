# DAG Attention Mechanisms

## Overview

This document specifies seven specialized attention mechanisms designed for Directed Acyclic Graph (DAG) structures. Each mechanism leverages unique DAG properties for specific optimization scenarios.

## Attention Trait Definition

```rust
/// Core trait for all DAG attention mechanisms
pub trait DagAttention: Send + Sync {
    /// Compute attention weights for a query node over its context
    fn forward(
        &self,
        query: &DagNode,
        context: &DagContext,
        config: &AttentionConfig,
    ) -> Result<AttentionOutput, AttentionError>;

    /// Update internal state based on execution feedback
    fn update(&mut self, feedback: &AttentionFeedback) -> Result<(), AttentionError>;

    /// Get attention type identifier
    fn attention_type(&self) -> DagAttentionType;

    /// Estimated computation complexity
    fn complexity(&self, context_size: usize) -> usize;
}

/// Output from attention computation
pub struct AttentionOutput {
    /// Attention weights (sum to 1.0)
    pub weights: Vec<f32>,

    /// Weighted aggregation of context values
    pub aggregated: Vec<f32>,

    /// Auxiliary information for learning
    pub metadata: AttentionMetadata,
}

/// Context for DAG attention
pub struct DagContext {
    /// All nodes in the DAG
    pub nodes: Vec<DagNode>,

    /// Adjacency list (node_id -> children)
    pub edges: HashMap<NodeId, Vec<NodeId>>,

    /// Reverse adjacency (node_id -> parents)
    pub reverse_edges: HashMap<NodeId, Vec<NodeId>>,

    /// Node depths (topological distance from roots)
    pub depths: HashMap<NodeId, usize>,

    /// Optional: timestamps for temporal attention
    pub timestamps: Option<HashMap<NodeId, f64>>,

    /// Optional: min-cut criticality scores
    pub criticalities: Option<HashMap<NodeId, f32>>,
}
```

---

## 1. Topological Attention

### Purpose
Respects DAG ordering by allowing nodes to only attend to their ancestors. This maintains causal consistency in query plans.

### Algorithm

```rust
pub struct TopologicalAttention {
    /// Hidden dimension for projections
    hidden_dim: usize,

    /// Number of attention heads
    num_heads: usize,

    /// Query/Key/Value projection weights
    w_query: Array2<f32>,  // [hidden_dim, hidden_dim]
    w_key: Array2<f32>,
    w_value: Array2<f32>,

    /// Precomputed ancestor masks (lazily computed)
    ancestor_cache: DashMap<NodeId, BitVec>,
}

impl TopologicalAttention {
    pub fn forward(&self, query_node: &DagNode, ctx: &DagContext) -> AttentionOutput {
        // 1. Get or compute ancestor mask
        let ancestors = self.get_ancestors(query_node.id, ctx);

        // 2. Project query
        let q = self.project_query(&query_node.embedding);

        // 3. Compute attention scores (only for ancestors)
        let mut scores = Vec::with_capacity(ctx.nodes.len());
        let scale = (self.hidden_dim as f32).sqrt();

        for (i, node) in ctx.nodes.iter().enumerate() {
            if ancestors.get(i).unwrap_or(false) {
                let k = self.project_key(&node.embedding);
                let score = dot(&q, &k) / scale;
                scores.push(score);
            } else {
                scores.push(f32::NEG_INFINITY);  // Mask non-ancestors
            }
        }

        // 4. Softmax
        let weights = softmax(&scores);

        // 5. Weighted aggregation of values
        let mut aggregated = vec![0.0; self.hidden_dim];
        for (i, node) in ctx.nodes.iter().enumerate() {
            if weights[i] > 1e-8 {
                let v = self.project_value(&node.embedding);
                for (j, val) in v.iter().enumerate() {
                    aggregated[j] += weights[i] * val;
                }
            }
        }

        AttentionOutput {
            weights,
            aggregated,
            metadata: AttentionMetadata::new(DagAttentionType::Topological),
        }
    }

    fn get_ancestors(&self, node_id: NodeId, ctx: &DagContext) -> BitVec {
        if let Some(cached) = self.ancestor_cache.get(&node_id) {
            return cached.clone();
        }

        // BFS to find all ancestors
        let mut ancestors = BitVec::repeat(false, ctx.nodes.len());
        let mut queue = VecDeque::new();

        // Start with direct parents
        if let Some(parents) = ctx.reverse_edges.get(&node_id) {
            for &parent in parents {
                queue.push_back(parent);
            }
        }

        while let Some(current) = queue.pop_front() {
            let idx = ctx.node_index(current);
            if !ancestors.get(idx).unwrap_or(false) {
                ancestors.set(idx, true);
                if let Some(parents) = ctx.reverse_edges.get(&current) {
                    for &parent in parents {
                        queue.push_back(parent);
                    }
                }
            }
        }

        self.ancestor_cache.insert(node_id, ancestors.clone());
        ancestors
    }
}
```

### Complexity
- Time: O(n·k) where n = nodes, k = average ancestors
- Space: O(n) for ancestor mask

### SQL Interface

```sql
-- Compute topological attention
SELECT ruvector_attention_topological(
    query_embedding,           -- VECTOR: query node embedding
    ancestor_embeddings,       -- VECTOR[]: ancestor embeddings
    '{"num_heads": 8, "hidden_dim": 256}'::jsonb  -- config
) AS attention_weights;
```

---

## 2. Causal Cone Attention

### Purpose
Extends topological attention with distance-weighted decay. Closer ancestors get more attention.

### Algorithm

```rust
pub struct CausalConeAttention {
    hidden_dim: usize,
    num_heads: usize,

    /// Decay rate per DAG hop
    decay_rate: f32,

    /// Maximum lookback depth
    max_depth: usize,

    /// Projection weights
    w_query: Array2<f32>,
    w_key: Array2<f32>,
    w_value: Array2<f32>,
}

impl CausalConeAttention {
    pub fn forward(&self, query_node: &DagNode, ctx: &DagContext) -> AttentionOutput {
        let query_depth = ctx.depths[&query_node.id];

        // 1. Compute distances from query to all ancestors
        let distances = self.compute_distances(query_node.id, ctx);

        // 2. Project query
        let q = self.project_query(&query_node.embedding);
        let scale = (self.hidden_dim as f32).sqrt();

        // 3. Compute distance-weighted attention
        let mut scores = Vec::with_capacity(ctx.nodes.len());

        for (i, node) in ctx.nodes.iter().enumerate() {
            if let Some(&dist) = distances.get(&node.id) {
                if dist > 0 && dist <= self.max_depth {
                    let k = self.project_key(&node.embedding);
                    let base_score = dot(&q, &k) / scale;

                    // Exponential decay with distance
                    let decay = (-self.decay_rate * dist as f32).exp();
                    scores.push(base_score * decay);
                } else {
                    scores.push(f32::NEG_INFINITY);
                }
            } else {
                scores.push(f32::NEG_INFINITY);
            }
        }

        // 4. Softmax and aggregate
        let weights = softmax(&scores);
        let aggregated = self.aggregate_values(&weights, ctx);

        AttentionOutput {
            weights,
            aggregated,
            metadata: AttentionMetadata::new(DagAttentionType::CausalCone),
        }
    }

    fn compute_distances(&self, from: NodeId, ctx: &DagContext) -> HashMap<NodeId, usize> {
        let mut distances = HashMap::new();
        let mut queue = VecDeque::new();

        // BFS from query node going backward
        if let Some(parents) = ctx.reverse_edges.get(&from) {
            for &parent in parents {
                queue.push_back((parent, 1));
            }
        }

        while let Some((current, dist)) = queue.pop_front() {
            if dist > self.max_depth {
                continue;
            }

            if !distances.contains_key(&current) {
                distances.insert(current, dist);

                if let Some(parents) = ctx.reverse_edges.get(&current) {
                    for &parent in parents {
                        queue.push_back((parent, dist + 1));
                    }
                }
            }
        }

        distances
    }
}
```

### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `decay_rate` | 0.1 | Exponential decay per hop |
| `max_depth` | 10 | Maximum ancestor distance |

### SQL Interface

```sql
SELECT ruvector_attention_causal_cone(
    query_embedding,
    ancestor_embeddings,
    ancestor_distances,        -- INT[]: hop distances
    0.1,                       -- decay_rate
    10                         -- max_depth
) AS attention_weights;
```

---

## 3. Critical Path Attention

### Purpose
Focuses attention on DAG critical path (longest path). Useful for identifying and optimizing bottleneck operators.

### Algorithm

```rust
pub struct CriticalPathAttention {
    hidden_dim: usize,
    num_heads: usize,

    /// Boost factor for critical path nodes
    critical_path_boost: f32,

    /// Projection weights
    w_query: Array2<f32>,
    w_key: Array2<f32>,
    w_value: Array2<f32>,

    /// Cached critical paths
    critical_path_cache: DashMap<NodeId, HashSet<NodeId>>,
}

impl CriticalPathAttention {
    pub fn forward(&self, query_node: &DagNode, ctx: &DagContext) -> AttentionOutput {
        // 1. Compute critical path through query node
        let critical_path = self.find_critical_path(query_node.id, ctx);

        // 2. Project query
        let q = self.project_query(&query_node.embedding);
        let scale = (self.hidden_dim as f32).sqrt();

        // 3. Compute attention with critical path boost
        let ancestors = self.get_ancestors(query_node.id, ctx);
        let mut scores = Vec::with_capacity(ctx.nodes.len());

        for (i, node) in ctx.nodes.iter().enumerate() {
            if ancestors.contains(&node.id) {
                let k = self.project_key(&node.embedding);
                let mut score = dot(&q, &k) / scale;

                // Boost critical path nodes
                if critical_path.contains(&node.id) {
                    score += self.critical_path_boost;
                }

                scores.push(score);
            } else {
                scores.push(f32::NEG_INFINITY);
            }
        }

        let weights = softmax(&scores);
        let aggregated = self.aggregate_values(&weights, ctx);

        AttentionOutput {
            weights,
            aggregated,
            metadata: AttentionMetadata::with_critical_path(critical_path),
        }
    }

    fn find_critical_path(&self, through: NodeId, ctx: &DagContext) -> HashSet<NodeId> {
        if let Some(cached) = self.critical_path_cache.get(&through) {
            return cached.clone();
        }

        // Find longest path through this node using DP
        let mut longest_to = HashMap::new();   // longest path ending at node
        let mut longest_from = HashMap::new(); // longest path starting from node

        // Topological order
        let topo_order = self.topological_sort(ctx);

        // Forward pass: longest path TO each node
        for &node in &topo_order {
            let mut max_len = 0;
            if let Some(parents) = ctx.reverse_edges.get(&node) {
                for &parent in parents {
                    max_len = max_len.max(longest_to.get(&parent).unwrap_or(&0) + 1);
                }
            }
            longest_to.insert(node, max_len);
        }

        // Backward pass: longest path FROM each node
        for &node in topo_order.iter().rev() {
            let mut max_len = 0;
            if let Some(children) = ctx.edges.get(&node) {
                for &child in children {
                    max_len = max_len.max(longest_from.get(&child).unwrap_or(&0) + 1);
                }
            }
            longest_from.insert(node, max_len);
        }

        // Find nodes on critical path through 'through'
        let total_through = longest_to[&through] + longest_from[&through];
        let global_longest = topo_order.iter()
            .map(|n| longest_to[n] + longest_from[n])
            .max()
            .unwrap_or(0);

        let mut critical = HashSet::new();
        if total_through == global_longest {
            // 'through' is on a global critical path
            for &node in &topo_order {
                if longest_to[&node] + longest_from[&node] == global_longest {
                    critical.insert(node);
                }
            }
        }

        self.critical_path_cache.insert(through, critical.clone());
        critical
    }
}
```

### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `critical_path_boost` | 1.5 | Additive boost for critical nodes |

### SQL Interface

```sql
SELECT ruvector_attention_critical_path(
    query_embedding,
    ancestor_embeddings,
    is_critical,               -- BOOLEAN[]: critical path membership
    1.5                        -- boost factor
) AS attention_weights;
```

---

## 4. MinCut Gated Attention

### Purpose
Uses min-cut analysis to gate information flow. Focuses learning on bottleneck operators that dominate execution time.

### Algorithm

```rust
pub struct MinCutGatedAttention {
    hidden_dim: usize,
    num_heads: usize,

    /// Threshold for full attention (criticality > threshold)
    gate_threshold: f32,

    /// Min-cut engine for criticality computation
    mincut_engine: Arc<SubpolynomialMinCut>,

    /// Projection weights
    w_query: Array2<f32>,
    w_key: Array2<f32>,
    w_value: Array2<f32>,

    /// Gate projection (learns to predict criticality)
    w_gate: Array2<f32>,
}

impl MinCutGatedAttention {
    pub fn forward(&self, query_node: &DagNode, ctx: &DagContext) -> AttentionOutput {
        // 1. Get or compute criticalities
        let criticalities = self.get_criticalities(ctx);

        // 2. Project query
        let q = self.project_query(&query_node.embedding);
        let scale = (self.hidden_dim as f32).sqrt();

        // 3. Compute gated attention
        let ancestors = self.get_ancestors(query_node.id, ctx);
        let mut scores = Vec::with_capacity(ctx.nodes.len());
        let mut gates = Vec::with_capacity(ctx.nodes.len());

        for (i, node) in ctx.nodes.iter().enumerate() {
            if ancestors.contains(&node.id) {
                let k = self.project_key(&node.embedding);
                let base_score = dot(&q, &k) / scale;

                // Compute gate based on criticality
                let criticality = criticalities.get(&node.id).unwrap_or(&0.0);
                let gate = if *criticality > self.gate_threshold {
                    1.0  // Full attention for critical nodes
                } else {
                    criticality / self.gate_threshold  // Scaled attention
                };

                scores.push(base_score);
                gates.push(gate);
            } else {
                scores.push(f32::NEG_INFINITY);
                gates.push(0.0);
            }
        }

        // 4. Apply gates before softmax
        let gated_scores: Vec<f32> = scores.iter()
            .zip(gates.iter())
            .map(|(s, g)| if *s > f32::NEG_INFINITY { s * g } else { *s })
            .collect();

        let weights = softmax(&gated_scores);
        let aggregated = self.aggregate_values(&weights, ctx);

        AttentionOutput {
            weights,
            aggregated,
            metadata: AttentionMetadata::with_criticalities(criticalities),
        }
    }

    fn get_criticalities(&self, ctx: &DagContext) -> HashMap<NodeId, f32> {
        // Use precomputed if available
        if let Some(ref crit) = ctx.criticalities {
            return crit.clone();
        }

        // Compute via min-cut engine
        let mut criticalities = HashMap::new();
        let global_cut = self.mincut_engine.query();

        for node in &ctx.nodes {
            // LocalKCut query around this node
            let local_query = LocalKCutQuery {
                seed_vertices: vec![node.id],
                budget_k: global_cut,
                radius: 3,
            };

            match self.mincut_engine.local_query(local_query) {
                LocalKCutResult::Found { cut_value, .. } => {
                    let criticality = (global_cut - cut_value) as f32 / global_cut as f32;
                    criticalities.insert(node.id, criticality);
                }
                LocalKCutResult::NoneInLocality => {
                    criticalities.insert(node.id, 0.0);
                }
            }
        }

        criticalities
    }
}
```

### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `gate_threshold` | 0.5 | Criticality threshold for full attention |

### Complexity
- Time: O(n^0.12 + n·k) - subpolynomial min-cut + attention
- Space: O(n) for criticality map

### SQL Interface

```sql
SELECT ruvector_attention_mincut_gated(
    query_embedding,
    ancestor_embeddings,
    ruvector_mincut_criticality(dag_id),  -- precomputed criticalities
    0.5                                    -- threshold
) AS attention_weights;
```

---

## 5. Hierarchical Lorentz Attention

### Purpose
Uses hyperbolic geometry (Lorentz hyperboloid) to naturally represent DAG hierarchy. Deeper nodes embed further from origin.

### Algorithm

```rust
pub struct HierarchicalLorentzAttention {
    hidden_dim: usize,
    num_heads: usize,

    /// Curvature of hyperbolic space (negative)
    curvature: f32,

    /// Temperature for attention softmax
    temperature: f32,

    /// Lorentz model for hyperbolic operations
    lorentz: LorentzModel,

    /// Projection to hyperbolic space
    w_to_hyperbolic: Array2<f32>,
}

impl HierarchicalLorentzAttention {
    pub fn forward(&self, query_node: &DagNode, ctx: &DagContext) -> AttentionOutput {
        let query_depth = ctx.depths[&query_node.id];

        // 1. Map query to hyperbolic space at its depth
        let query_hyper = self.to_lorentz(
            &query_node.embedding,
            query_depth,
        );

        // 2. Compute Lorentz attention scores
        let ancestors = self.get_ancestors(query_node.id, ctx);
        let mut scores = Vec::with_capacity(ctx.nodes.len());

        for (i, node) in ctx.nodes.iter().enumerate() {
            if ancestors.contains(&node.id) {
                let node_depth = ctx.depths[&node.id];
                let node_hyper = self.to_lorentz(&node.embedding, node_depth);

                // Busemann function for O(d) hierarchy scoring
                let score = self.lorentz.busemann_score(&query_hyper, &node_hyper);
                scores.push(score / self.temperature);
            } else {
                scores.push(f32::NEG_INFINITY);
            }
        }

        // 3. Hyperbolic softmax
        let weights = self.lorentz.hyperbolic_softmax(&scores);

        // 4. Einstein midpoint aggregation (closed-form, no iteration)
        let aggregated = self.einstein_midpoint_aggregation(&weights, ctx);

        AttentionOutput {
            weights,
            aggregated,
            metadata: AttentionMetadata::new(DagAttentionType::HierarchicalLorentz),
        }
    }

    fn to_lorentz(&self, embedding: &[f32], depth: usize) -> LorentzPoint {
        // Project to hyperbolic space
        let projected = self.w_to_hyperbolic.dot(&Array1::from_vec(embedding.to_vec()));

        // Map depth to hyperbolic radius
        let radius = (depth as f32 + 1.0).ln() / (-self.curvature).sqrt();

        self.lorentz.exp_map_at_origin(&projected.to_vec(), radius)
    }

    fn einstein_midpoint_aggregation(
        &self,
        weights: &[f32],
        ctx: &DagContext,
    ) -> Vec<f32> {
        // Closed-form weighted centroid in Lorentz model
        // Much faster than iterative Fréchet mean

        let mut numerator = vec![0.0; self.hidden_dim + 1];
        let mut denominator = 0.0;

        for (i, node) in ctx.nodes.iter().enumerate() {
            if weights[i] > 1e-8 {
                let depth = ctx.depths[&node.id];
                let hyper = self.to_lorentz(&node.embedding, depth);

                // Lorentz factor
                let gamma = hyper.lorentz_factor();
                let weight = weights[i] * gamma;

                for (j, &coord) in hyper.coordinates().iter().enumerate() {
                    numerator[j] += weight * coord;
                }
                denominator += weight;
            }
        }

        // Normalize and project back to Euclidean
        let midpoint: Vec<f32> = numerator.iter()
            .map(|x| x / denominator)
            .collect();

        self.lorentz.log_map_to_euclidean(&midpoint)
    }
}
```

### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `curvature` | -1.0 | Hyperbolic space curvature |
| `temperature` | 1.0 | Softmax temperature |

### Benefits
- 5-10x faster than Poincaré attention
- Numerically stable (no tanh clipping)
- Natural hierarchy representation

### SQL Interface

```sql
SELECT ruvector_attention_hierarchical_lorentz(
    query_embedding,
    ancestor_embeddings,
    ancestor_depths,           -- INT[]: DAG depths
    -1.0,                      -- curvature
    1.0                        -- temperature
) AS attention_weights;
```

---

## 6. Parallel Branch Attention

### Purpose
Coordinates across parallel DAG branches. Essential for parallel query execution where branches need to share information.

### Algorithm

```rust
pub struct ParallelBranchAttention {
    hidden_dim: usize,
    num_heads: usize,

    /// Weight for cross-branch attention
    cross_branch_weight: f32,

    /// Boost for common ancestors (synchronization points)
    common_ancestor_boost: f32,

    /// Projection weights
    w_query: Array2<f32>,
    w_key: Array2<f32>,
    w_value: Array2<f32>,

    /// Cross-attention projection
    w_cross: Array2<f32>,
}

impl ParallelBranchAttention {
    pub fn forward(&self, query_node: &DagNode, ctx: &DagContext) -> AttentionOutput {
        let query_depth = ctx.depths[&query_node.id];

        // 1. Find parallel branches (same depth, different lineage)
        let parallel_nodes = self.find_parallel_nodes(query_node.id, query_depth, ctx);

        // 2. Find common ancestors (synchronization points)
        let common_ancestors = self.find_common_ancestors(
            query_node.id,
            &parallel_nodes,
            ctx,
        );

        // 3. Project query
        let q = self.project_query(&query_node.embedding);
        let scale = (self.hidden_dim as f32).sqrt();

        // 4. Compute multi-source attention
        let ancestors = self.get_ancestors(query_node.id, ctx);
        let mut scores = Vec::new();
        let mut node_types = Vec::new();  // Track source type

        // Own ancestors
        for node in &ctx.nodes {
            if ancestors.contains(&node.id) {
                let k = self.project_key(&node.embedding);
                let mut score = dot(&q, &k) / scale;

                // Boost common ancestors
                if common_ancestors.contains(&node.id) {
                    score += self.common_ancestor_boost;
                }

                scores.push(score);
                node_types.push(NodeType::Ancestor);
            }
        }

        // Parallel branch nodes (cross-attention)
        for &parallel_id in &parallel_nodes {
            let parallel_node = ctx.get_node(parallel_id);
            let k = self.project_cross(&parallel_node.embedding);
            let score = dot(&q, &k) / scale * self.cross_branch_weight;

            scores.push(score);
            node_types.push(NodeType::ParallelBranch);
        }

        // 5. Softmax over all sources
        let weights = softmax(&scores);

        // 6. Aggregation (separate paths for different types)
        let aggregated = self.multi_source_aggregate(&weights, &node_types, ctx);

        AttentionOutput {
            weights,
            aggregated,
            metadata: AttentionMetadata::with_parallel_info(parallel_nodes, common_ancestors),
        }
    }

    fn find_parallel_nodes(
        &self,
        query_id: NodeId,
        query_depth: usize,
        ctx: &DagContext,
    ) -> Vec<NodeId> {
        let query_ancestors = self.get_ancestors(query_id, ctx);

        ctx.nodes.iter()
            .filter(|n| {
                n.id != query_id &&
                ctx.depths[&n.id] == query_depth &&
                !query_ancestors.contains(&n.id)  // Not an ancestor
            })
            .map(|n| n.id)
            .collect()
    }

    fn find_common_ancestors(
        &self,
        query_id: NodeId,
        parallel_ids: &[NodeId],
        ctx: &DagContext,
    ) -> HashSet<NodeId> {
        if parallel_ids.is_empty() {
            return HashSet::new();
        }

        // Start with query's ancestors
        let mut common = self.get_ancestors(query_id, ctx);

        // Intersect with each parallel node's ancestors
        for &parallel_id in parallel_ids {
            let parallel_ancestors = self.get_ancestors(parallel_id, ctx);
            common = common.intersection(&parallel_ancestors).cloned().collect();
        }

        common
    }
}
```

### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `cross_branch_weight` | 0.5 | Weight for cross-branch attention |
| `common_ancestor_boost` | 2.0 | Boost for synchronization points |

### SQL Interface

```sql
SELECT ruvector_attention_parallel_branch(
    query_embedding,
    ancestor_embeddings,
    parallel_embeddings,       -- VECTOR[]: parallel branch embeddings
    common_ancestor_mask,      -- BOOLEAN[]: common ancestor indicators
    0.5,                       -- cross_weight
    2.0                        -- common_ancestor_boost
) AS attention_weights;
```

---

## 7. Temporal BTSP Attention

### Purpose
Combines DAG structure with temporal spike patterns. Uses BTSP (Behavioral Timescale Synaptic Plasticity) for one-shot learning of time-correlated query patterns.

### Algorithm

```rust
pub struct TemporalBTSPAttention {
    hidden_dim: usize,
    num_heads: usize,

    /// Coincidence window for temporal grouping (ms)
    coincidence_window_ms: f32,

    /// Boost for temporally coincident nodes
    coincidence_boost: f32,

    /// BTSP memory for one-shot pattern recall
    btsp_memory: BTSPLayer,

    /// Projection weights
    w_query: Array2<f32>,
    w_key: Array2<f32>,
    w_value: Array2<f32>,
}

impl TemporalBTSPAttention {
    pub fn forward(
        &self,
        query_node: &DagNode,
        ctx: &DagContext,
    ) -> AttentionOutput {
        let timestamps = ctx.timestamps.as_ref()
            .expect("Temporal attention requires timestamps");

        let query_time = timestamps[&query_node.id];

        // 1. Try BTSP recall first (one-shot memory)
        if let Some(recalled) = self.btsp_memory.recall(&query_node.embedding) {
            return AttentionOutput {
                weights: recalled.weights,
                aggregated: recalled.aggregated,
                metadata: AttentionMetadata::btsp_recalled(),
            };
        }

        // 2. Find temporally coincident nodes
        let coincident: HashSet<NodeId> = ctx.nodes.iter()
            .filter(|n| {
                let t = timestamps[&n.id];
                (query_time - t).abs() < self.coincidence_window_ms as f64
            })
            .map(|n| n.id)
            .collect();

        // 3. Compute attention with temporal boost
        let ancestors = self.get_ancestors(query_node.id, ctx);
        let q = self.project_query(&query_node.embedding);
        let scale = (self.hidden_dim as f32).sqrt();

        let mut scores = Vec::with_capacity(ctx.nodes.len());

        for node in &ctx.nodes {
            if ancestors.contains(&node.id) {
                let k = self.project_key(&node.embedding);
                let mut score = dot(&q, &k) / scale;

                // Boost temporally coincident nodes
                if coincident.contains(&node.id) {
                    score *= self.coincidence_boost;
                }

                scores.push(score);
            } else {
                scores.push(f32::NEG_INFINITY);
            }
        }

        let weights = softmax(&scores);
        let aggregated = self.aggregate_values(&weights, ctx);

        // 4. One-shot learning via BTSP plateau
        if self.should_learn(&weights) {
            self.btsp_memory.associate(
                &query_node.embedding,
                &weights,
                &aggregated,
            );
        }

        AttentionOutput {
            weights,
            aggregated,
            metadata: AttentionMetadata::with_temporal_info(coincident),
        }
    }

    fn should_learn(&self, weights: &[f32]) -> bool {
        // Learn if attention is confident (low entropy)
        let entropy: f32 = weights.iter()
            .filter(|&&w| w > 1e-8)
            .map(|&w| -w * w.ln())
            .sum();

        let max_entropy = (weights.len() as f32).ln();
        let normalized_entropy = entropy / max_entropy;

        normalized_entropy < 0.5  // Confident attention pattern
    }
}
```

### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `coincidence_window_ms` | 50.0 | Temporal coincidence window |
| `coincidence_boost` | 1.5 | Boost for coincident nodes |

### BTSP Memory Structure

```rust
pub struct BTSPLayer {
    /// Synaptic weights: pattern → attention
    synapses: Vec<BTSPSynapse>,

    /// Number of stored patterns
    pattern_count: usize,

    /// Maximum patterns before consolidation
    max_patterns: usize,

    /// Similarity threshold for recall
    recall_threshold: f32,
}

pub struct BTSPSynapse {
    /// Input pattern embedding
    pattern: Vec<f32>,

    /// Learned attention weights
    attention_weights: Vec<f32>,

    /// Learned aggregated output
    aggregated: Vec<f32>,

    /// Confidence score
    confidence: f32,

    /// Usage count
    usage_count: usize,
}
```

### SQL Interface

```sql
SELECT ruvector_attention_temporal_btsp(
    query_embedding,
    ancestor_embeddings,
    ancestor_timestamps,       -- FLOAT[]: timestamps in ms
    50.0,                      -- coincidence_window_ms
    1.5                        -- coincidence_boost
) AS attention_weights;
```

---

## Ensemble Attention

### Purpose
Combines multiple attention types for robust performance.

### Algorithm

```rust
pub struct EnsembleAttention {
    /// Component attention mechanisms
    components: Vec<Box<dyn DagAttention>>,

    /// Combination weights (learned or fixed)
    weights: Vec<f32>,

    /// Weight learning mode
    weight_mode: WeightMode,
}

pub enum WeightMode {
    /// Fixed weights
    Fixed,

    /// Learn weights via gradient descent
    Learned,

    /// Adaptive based on query pattern
    Adaptive(AdaptiveWeightSelector),
}

impl EnsembleAttention {
    pub fn forward(&self, query_node: &DagNode, ctx: &DagContext) -> AttentionOutput {
        // Get weights for this query
        let weights = match &self.weight_mode {
            WeightMode::Fixed => self.weights.clone(),
            WeightMode::Learned => self.weights.clone(),
            WeightMode::Adaptive(selector) => {
                selector.select_weights(&query_node.embedding)
            }
        };

        // Compute attention from each component
        let outputs: Vec<AttentionOutput> = self.components.iter()
            .map(|c| c.forward(query_node, ctx))
            .collect();

        // Weighted combination
        let n = outputs[0].weights.len();
        let mut combined_weights = vec![0.0; n];
        let mut combined_aggregated = vec![0.0; outputs[0].aggregated.len()];

        for (i, output) in outputs.iter().enumerate() {
            let w = weights[i];
            for (j, &ow) in output.weights.iter().enumerate() {
                combined_weights[j] += w * ow;
            }
            for (j, &oa) in output.aggregated.iter().enumerate() {
                combined_aggregated[j] += w * oa;
            }
        }

        // Renormalize weights
        let sum: f32 = combined_weights.iter().sum();
        if sum > 0.0 {
            for w in &mut combined_weights {
                *w /= sum;
            }
        }

        AttentionOutput {
            weights: combined_weights,
            aggregated: combined_aggregated,
            metadata: AttentionMetadata::ensemble(
                self.components.iter().map(|c| c.attention_type()).collect()
            ),
        }
    }
}
```

### SQL Interface

```sql
SELECT ruvector_attention_ensemble(
    query_embedding,
    ancestor_embeddings,
    ARRAY['topological', 'critical_path', 'mincut_gated'],  -- types
    ARRAY[0.4, 0.3, 0.3]::FLOAT[]                           -- weights (optional)
) AS attention_weights;
```

---

## Attention Selector (UCB Bandit)

### Purpose
Automatically selects the best attention type for each query pattern.

### Algorithm

```rust
pub struct AttentionSelector {
    /// Performance history per (pattern_type, attention_type)
    history: DashMap<(PatternTypeId, DagAttentionType), PerformanceStats>,

    /// UCB exploration coefficient
    ucb_c: f32,

    /// Exploration probability (epsilon-greedy)
    epsilon: f32,

    /// Pattern type classifier
    pattern_classifier: PatternClassifier,
}

impl AttentionSelector {
    pub fn select(&self, query_embedding: &[f32]) -> DagAttentionType {
        // Classify query pattern
        let pattern_type = self.pattern_classifier.classify(query_embedding);

        // Epsilon-greedy exploration
        if rand::random::<f32>() < self.epsilon {
            return DagAttentionType::random();
        }

        // UCB selection
        let total_trials: usize = self.history.iter()
            .filter(|e| e.key().0 == pattern_type)
            .map(|e| e.value().trials)
            .sum();

        let mut best_ucb = f32::NEG_INFINITY;
        let mut best_type = DagAttentionType::Topological;

        for attention_type in DagAttentionType::all() {
            let key = (pattern_type, attention_type.clone());

            let (mean_reward, trials) = self.history.get(&key)
                .map(|s| (s.mean_reward(), s.trials))
                .unwrap_or((0.5, 1));  // Optimistic initialization

            // UCB formula
            let exploration = self.ucb_c *
                ((total_trials as f32).ln() / trials as f32).sqrt();
            let ucb = mean_reward + exploration;

            if ucb > best_ucb {
                best_ucb = ucb;
                best_type = attention_type;
            }
        }

        best_type
    }

    pub fn update(
        &self,
        query_embedding: &[f32],
        attention_type: DagAttentionType,
        reward: f32,
    ) {
        let pattern_type = self.pattern_classifier.classify(query_embedding);
        let key = (pattern_type, attention_type);

        self.history.entry(key)
            .or_insert_with(PerformanceStats::new)
            .record(reward);
    }
}
```

### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `ucb_c` | 1.414 | UCB exploration coefficient (√2) |
| `epsilon` | 0.1 | Random exploration probability |

---

## Performance Summary

| Attention Type | Time Complexity | Space Complexity | Best For |
|----------------|-----------------|------------------|----------|
| Topological | O(n·k) | O(n) | Causal consistency |
| Causal Cone | O(n·d) | O(n) | Distance-weighted |
| Critical Path | O(n + crit_len) | O(n) | Bottleneck focus |
| MinCut Gated | O(n^0.12 + n·k) | O(n) | Bottleneck detection |
| Hierarchical Lorentz | O(n·d) | O(n·d) | Deep hierarchies |
| Parallel Branch | O(n·b) | O(n) | Parallel execution |
| Temporal BTSP | O(n·w) | O(patterns) | Repeated queries |
| Ensemble | O(e·n·k) | O(e·n) | Robust performance |

Where:
- n = number of nodes
- k = average ancestors
- d = DAG depth
- b = parallel branches
- w = coincidence window
- e = ensemble components
