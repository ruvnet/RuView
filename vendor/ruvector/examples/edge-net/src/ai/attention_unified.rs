//! Unified Attention Architecture for Edge-Net P2P AI
//!
//! Integrates four attention paradigms to answer fundamental questions:
//!
//! 1. **Neural Attention** - What words/tokens matter? (Multi-head self-attention)
//! 2. **DAG Attention** - What steps matter? (Topological attention over DAGs)
//! 3. **Graph Attention** - What relationships matter? (GAT-style edge attention)
//! 4. **State Space** - What history matters? (Selective state space models)
//!
//! ## Features
//!
//! - WASM-compatible (no std::thread)
//! - SIMD acceleration via compute module
//! - Unified `AttentionOutput` with importance scores
//! - O(n) sequence processing for state space
//! - Position-aware attention masks
//! - Critical path identification for DAGs
//! - Multi-hop graph attention
//!
//! ## References
//!
//! - Vaswani et al. (2017) - Attention Is All You Need
//! - Velickovic et al. (2018) - Graph Attention Networks
//! - Gu & Dao (2023) - Mamba: Linear-Time Sequence Modeling

use crate::compute::simd::SimdCompute;
use std::collections::HashMap;

// ============================================================================
// Common Output Structure
// ============================================================================

/// Unified attention output with importance scores
#[derive(Clone, Debug)]
pub struct AttentionOutput {
    /// Attended representation / output embeddings
    pub embeddings: Vec<f32>,
    /// Importance scores per input element [num_elements]
    pub importance: Vec<f32>,
    /// Attention weights matrix (optional) [query_len, key_len]
    pub attention_weights: Option<Vec<Vec<f32>>>,
    /// Top-k important indices (sorted by importance)
    pub top_k_indices: Vec<usize>,
    /// Metadata about attention computation
    pub metadata: AttentionMetadata,
}

impl AttentionOutput {
    /// Create new attention output with computed metrics
    pub fn new(embeddings: Vec<f32>, scores: Vec<f32>, top_k: usize) -> Self {
        let top_k_indices = Self::get_top_k_indices(&scores, top_k);
        let max_score = scores.iter().cloned().fold(0.0f32, f32::max);
        let entropy = Self::compute_entropy(&scores);
        let sparsity = Self::compute_sparsity(&scores, 0.01);

        Self {
            embeddings,
            top_k_indices,
            attention_weights: None,
            metadata: AttentionMetadata {
                entropy,
                max_score,
                attended_count: scores.len(),
                sparsity,
                attention_type: AttentionType::Neural,
            },
            importance: scores,
        }
    }

    /// Create with full attention weights matrix
    pub fn with_weights(mut self, weights: Vec<Vec<f32>>) -> Self {
        self.attention_weights = Some(weights);
        self
    }

    /// Set attention type metadata
    pub fn with_type(mut self, attention_type: AttentionType) -> Self {
        self.metadata.attention_type = attention_type;
        self
    }

    fn compute_entropy(scores: &[f32]) -> f32 {
        if scores.is_empty() {
            return 0.0;
        }
        -scores
            .iter()
            .filter(|&&p| p > 1e-10)
            .map(|&p| p * p.ln())
            .sum::<f32>()
    }

    fn compute_sparsity(scores: &[f32], threshold: f32) -> f32 {
        if scores.is_empty() {
            return 0.0;
        }
        scores.iter().filter(|&&s| s < threshold).count() as f32 / scores.len() as f32
    }

    fn get_top_k_indices(scores: &[f32], k: usize) -> Vec<usize> {
        let mut indexed: Vec<(usize, f32)> = scores.iter().copied().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        indexed.into_iter().take(k).map(|(i, _)| i).collect()
    }

    /// Get normalized importance (0-1 range)
    pub fn normalized_importance(&self) -> Vec<f32> {
        let max = self.importance.iter().cloned().fold(0.0f32, f32::max);
        if max > 1e-10 {
            self.importance.iter().map(|&s| s / max).collect()
        } else {
            self.importance.clone()
        }
    }
}

/// Metadata about attention computation
#[derive(Clone, Debug, Default)]
pub struct AttentionMetadata {
    /// Entropy of attention distribution (lower = more focused)
    pub entropy: f32,
    /// Max attention score
    pub max_score: f32,
    /// Number of attended positions
    pub attended_count: usize,
    /// Sparsity ratio (0-1)
    pub sparsity: f32,
    /// Attention type used
    pub attention_type: AttentionType,
}

// ============================================================================
// Configuration
// ============================================================================

/// Type of attention mechanism
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum AttentionType {
    #[default]
    Neural,     // What words matter
    DAG,        // What steps matter
    Graph,      // What relationships matter
    StateSpace, // What history matters
}

/// Unified attention configuration
#[derive(Clone, Debug)]
pub struct UnifiedAttentionConfig {
    /// Hidden dimension for all projections
    pub hidden_dim: usize,
    /// Number of attention heads
    pub num_heads: usize,
    /// State dimension for SSM
    pub state_dim: usize,
    /// Dropout rate (training only)
    pub dropout: f32,
    /// Layer normalization epsilon
    pub layer_norm_eps: f32,
    /// Top-k for importance output
    pub top_k: usize,
    /// Enable residual connections
    pub residual: bool,
    /// Enable causal masking
    pub causal: bool,
    /// Enable layer normalization
    pub use_layer_norm: bool,
}

impl Default for UnifiedAttentionConfig {
    fn default() -> Self {
        Self {
            hidden_dim: 128,
            num_heads: 8,
            state_dim: 16,
            dropout: 0.0,
            layer_norm_eps: 1e-5,
            top_k: 5,
            residual: true,
            causal: false,
            use_layer_norm: true,
        }
    }
}

// ============================================================================
// 1. Neural Attention (Multi-Head Self-Attention)
// ============================================================================

/// Multi-head self-attention with learned Q/K/V projections
///
/// Answers: "What words/tokens matter?"
///
/// Implements:
/// - Scaled dot-product attention with softmax
/// - Multi-head parallelism
/// - Position-aware attention masks
/// - Token importance scoring
pub struct NeuralAttention {
    config: UnifiedAttentionConfig,
    /// Query projection [hidden_dim, hidden_dim]
    w_q: Vec<f32>,
    /// Key projection [hidden_dim, hidden_dim]
    w_k: Vec<f32>,
    /// Value projection [hidden_dim, hidden_dim]
    w_v: Vec<f32>,
    /// Output projection [hidden_dim, hidden_dim]
    w_o: Vec<f32>,
    /// Layer norm weights
    ln_weight: Vec<f32>,
    /// Layer norm bias
    ln_bias: Vec<f32>,
    /// Learned positional embeddings (optional)
    pos_embeddings: Option<Vec<Vec<f32>>>,
}

impl NeuralAttention {
    /// Create with default configuration
    pub fn new(hidden_dim: usize, num_heads: usize) -> Result<Self, String> {
        Self::with_config(UnifiedAttentionConfig {
            hidden_dim,
            num_heads,
            ..Default::default()
        })
    }

    /// Create with custom configuration
    pub fn with_config(config: UnifiedAttentionConfig) -> Result<Self, String> {
        let h = config.hidden_dim;
        if h % config.num_heads != 0 {
            return Err(format!(
                "hidden_dim {} must be divisible by num_heads {}",
                h, config.num_heads
            ));
        }

        let size = h * h;
        let scale = (2.0 / (h + h) as f32).sqrt();

        Ok(Self {
            w_q: Self::init_weights(size, scale, 0.1),
            w_k: Self::init_weights(size, scale, 0.13),
            w_v: Self::init_weights(size, scale, 0.17),
            w_o: Self::init_weights(size, scale, 0.19),
            ln_weight: vec![1.0; h],
            ln_bias: vec![0.0; h],
            pos_embeddings: None,
            config,
        })
    }

    fn init_weights(size: usize, scale: f32, seed: f32) -> Vec<f32> {
        (0..size)
            .map(|i| ((i as f32 * seed).sin() * scale).clamp(-scale, scale))
            .collect()
    }

    /// Enable learnable positional embeddings (sinusoidal)
    pub fn with_positions(mut self, max_len: usize) -> Self {
        let dim = self.config.hidden_dim;
        self.pos_embeddings = Some(
            (0..max_len)
                .map(|pos| {
                    (0..dim)
                        .map(|i| {
                            let angle = pos as f32 / 10000_f32.powf(2.0 * (i / 2) as f32 / dim as f32);
                            if i % 2 == 0 { angle.sin() } else { angle.cos() }
                        })
                        .collect()
                })
                .collect(),
        );
        self
    }

    /// Forward pass with optional attention mask
    ///
    /// # Arguments
    /// * `tokens` - Token embeddings [seq_len, hidden_dim]
    /// * `mask` - Optional attention mask [seq_len, seq_len] (1.0 = attend, 0.0 = mask)
    pub fn forward(&self, tokens: &[Vec<f32>]) -> AttentionOutput {
        self.forward_with_mask(tokens, None)
    }

    /// Forward with explicit mask
    pub fn forward_with_mask(&self, tokens: &[Vec<f32>], mask: Option<&[Vec<f32>]>) -> AttentionOutput {
        let seq_len = tokens.len();
        let h = self.config.hidden_dim;
        let num_heads = self.config.num_heads;
        let head_dim = h / num_heads;

        if seq_len == 0 {
            return AttentionOutput::new(vec![], vec![], self.config.top_k)
                .with_type(AttentionType::Neural);
        }

        // Add positional embeddings if available
        let tokens_with_pos: Vec<Vec<f32>> = if let Some(ref pos_emb) = self.pos_embeddings {
            tokens
                .iter()
                .enumerate()
                .map(|(i, tok)| {
                    let pos = &pos_emb[i.min(pos_emb.len() - 1)];
                    tok.iter().zip(pos.iter()).map(|(t, p)| t + p).collect()
                })
                .collect()
        } else {
            tokens.to_vec()
        };

        // Project all tokens to Q, K, V using SIMD
        let queries: Vec<Vec<f32>> = tokens_with_pos
            .iter()
            .map(|t| SimdCompute::matvec_simd(&self.w_q, t, h, t.len()))
            .collect();
        let keys: Vec<Vec<f32>> = tokens_with_pos
            .iter()
            .map(|t| SimdCompute::matvec_simd(&self.w_k, t, h, t.len()))
            .collect();
        let values: Vec<Vec<f32>> = tokens_with_pos
            .iter()
            .map(|t| SimdCompute::matvec_simd(&self.w_v, t, h, t.len()))
            .collect();

        // Compute attention scores [seq_len, seq_len]
        let scale = (head_dim as f32).sqrt();
        let mut attention_weights = vec![vec![0.0f32; seq_len]; seq_len];
        let mut all_scores = vec![0.0f32; seq_len];

        for (q_idx, query) in queries.iter().enumerate() {
            for (k_idx, key) in keys.iter().enumerate() {
                let mut score = SimdCompute::dot_product(query, key) / scale;

                // Apply external mask if provided
                if let Some(m) = mask {
                    if q_idx < m.len() && k_idx < m[q_idx].len() && m[q_idx][k_idx] < 0.5 {
                        score = f32::NEG_INFINITY;
                    }
                }

                // Apply causal mask
                if self.config.causal && k_idx > q_idx {
                    score = f32::NEG_INFINITY;
                }

                attention_weights[q_idx][k_idx] = score;
            }

            // Softmax over row
            SimdCompute::softmax_simd(&mut attention_weights[q_idx]);

            // Accumulate importance scores
            for (k_idx, &weight) in attention_weights[q_idx].iter().enumerate() {
                all_scores[k_idx] += weight / seq_len as f32;
            }
        }

        // Weighted sum of values
        let mut outputs = vec![vec![0.0f32; h]; seq_len];
        for i in 0..seq_len {
            for j in 0..seq_len {
                for d in 0..h.min(values[j].len()) {
                    outputs[i][d] += attention_weights[i][j] * values[j][d];
                }
            }
        }

        // Output projection
        let projected: Vec<Vec<f32>> = outputs
            .iter()
            .map(|o| SimdCompute::matvec_simd(&self.w_o, o, h, o.len()))
            .collect();

        // Residual + LayerNorm
        let final_outputs: Vec<Vec<f32>> = if self.config.residual {
            projected
                .iter()
                .zip(tokens.iter())
                .map(|(proj, tok)| {
                    let mut res: Vec<f32> = proj
                        .iter()
                        .zip(tok.iter())
                        .map(|(p, t)| p + t)
                        .collect();
                    if self.config.use_layer_norm {
                        res = SimdCompute::layer_norm_simd(
                            &res,
                            &self.ln_weight,
                            Some(&self.ln_bias),
                            self.config.layer_norm_eps,
                        );
                    }
                    res
                })
                .collect()
        } else {
            projected
        };

        let embeddings: Vec<f32> = final_outputs.into_iter().flatten().collect();

        AttentionOutput::new(embeddings, all_scores, self.config.top_k)
            .with_weights(attention_weights)
            .with_type(AttentionType::Neural)
    }
}

// ============================================================================
// 2. DAG Attention (Topological Attention)
// ============================================================================

/// DAG node for topological attention
#[derive(Clone, Debug)]
pub struct DAGNode {
    /// Node identifier
    pub id: usize,
    /// Node embedding
    pub embedding: Vec<f32>,
    /// Incoming edge indices (dependencies)
    pub dependencies: Vec<usize>,
}

/// Topological attention over directed acyclic graphs
///
/// Answers: "What steps/dependencies matter?"
///
/// Implements:
/// - Step dependency weighting
/// - Critical path identification
/// - Causal attention masks for sequential steps
pub struct DAGAttention {
    config: UnifiedAttentionConfig,
    /// Step importance weights [max_levels]
    step_weights: Vec<f32>,
    /// Dependency scoring matrix [hidden_dim, hidden_dim]
    w_step: Vec<f32>,
    /// Critical path scoring
    w_dep: Vec<f32>,
    /// Query projection
    w_query: Vec<f32>,
    /// Max levels supported
    max_levels: usize,
}

impl DAGAttention {
    /// Create with default hidden dimension
    pub fn new(hidden_dim: usize) -> Self {
        Self::with_config(
            UnifiedAttentionConfig {
                hidden_dim,
                ..Default::default()
            },
            32,
        )
    }

    /// Create with custom configuration
    pub fn with_config(config: UnifiedAttentionConfig, max_levels: usize) -> Self {
        let h = config.hidden_dim;
        let size = h * h;
        let scale = (2.0 / (h + h) as f32).sqrt();

        Self {
            step_weights: (0..max_levels)
                .map(|l| 1.0 / (1.0 + l as f32 * 0.5))
                .collect(),
            w_step: (0..size).map(|i| (i as f32 * 0.1).sin() * scale).collect(),
            w_dep: (0..size).map(|i| (i as f32 * 0.2).cos() * scale).collect(),
            w_query: (0..size).map(|i| (i as f32 * 0.15).sin() * scale).collect(),
            max_levels,
            config,
        }
    }

    /// Forward pass over DAG with query
    pub fn forward(&self, nodes: &[DAGNode]) -> AttentionOutput {
        let query = vec![0.5; self.config.hidden_dim];
        self.forward_with_query(nodes, &query)
    }

    /// Forward pass over DAG with explicit query
    pub fn forward_with_query(&self, nodes: &[DAGNode], query: &[f32]) -> AttentionOutput {
        let n = nodes.len();
        if n == 0 {
            return AttentionOutput::new(vec![], vec![], self.config.top_k)
                .with_type(AttentionType::DAG);
        }

        let h = self.config.hidden_dim;
        let mut scores = vec![0.0f32; n];
        let mut dependency_weights = vec![vec![0.0f32; n]; n];

        // Compute topological levels
        let mut topo_levels = vec![0usize; n];
        for (i, node) in nodes.iter().enumerate() {
            let max_dep_level = node
                .dependencies
                .iter()
                .filter_map(|&d| if d < n { Some(topo_levels[d]) } else { None })
                .max()
                .unwrap_or(0);
            topo_levels[i] = max_dep_level + 1;
        }

        // Find critical path
        let critical_path = self.find_critical_path(nodes, &topo_levels);

        // Count dependents
        let mut dependent_count = vec![0usize; n];
        for node in nodes {
            for &dep in &node.dependencies {
                if dep < n {
                    dependent_count[dep] += 1;
                }
            }
        }

        // Project query
        let query_proj = SimdCompute::matvec_simd(&self.w_query, query, h, query.len());

        for (i, node) in nodes.iter().enumerate() {
            // 1. Topological level weight
            let level_weight = self
                .step_weights
                .get(topo_levels[i])
                .copied()
                .unwrap_or(0.1);

            // 2. Dependency weight (more dependents = more important)
            let dep_weight = 1.0 + dependent_count[i] as f32 * 0.3;

            // 3. Query-node relevance using SIMD
            let node_proj = SimdCompute::matvec_simd(&self.w_step, &node.embedding, h, node.embedding.len());
            let relevance = SimdCompute::dot_product(&query_proj, &node_proj).max(0.0);

            // 4. Critical path bonus
            let critical_bonus = if critical_path.contains(&i) { 1.5 } else { 1.0 };

            scores[i] = level_weight * dep_weight * relevance * critical_bonus;

            // Build dependency attention weights
            for &dep_idx in &node.dependencies {
                if dep_idx < n {
                    let diff = (topo_levels[i] - topo_levels[dep_idx]) as f32;
                    dependency_weights[i][dep_idx] = 1.0 / (1.0 + diff);
                }
            }
        }

        // Normalize scores
        let sum: f32 = scores.iter().sum();
        if sum > 1e-10 {
            for s in &mut scores {
                *s /= sum;
            }
        } else {
            scores.fill(1.0 / n as f32);
        }

        // Compute attended representation
        let mut attended = vec![0.0f32; h];
        for (i, node) in nodes.iter().enumerate() {
            for j in 0..h.min(node.embedding.len()) {
                attended[j] += scores[i] * node.embedding[j];
            }
        }

        // Residual with query
        if self.config.residual {
            for j in 0..h.min(query.len()) {
                attended[j] += query[j];
            }
        }

        AttentionOutput::new(attended, scores, self.config.top_k)
            .with_weights(dependency_weights)
            .with_type(AttentionType::DAG)
    }

    /// Find critical path (longest path through DAG)
    fn find_critical_path(&self, nodes: &[DAGNode], topo_levels: &[usize]) -> Vec<usize> {
        let n = nodes.len();
        if n == 0 {
            return vec![];
        }

        let mut longest = vec![0usize; n];
        let mut predecessor: Vec<Option<usize>> = vec![None; n];

        // Sort by topo level
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&i| topo_levels[i]);

        for &i in &order {
            for &dep in &nodes[i].dependencies {
                if dep < n && longest[dep] + 1 > longest[i] {
                    longest[i] = longest[dep] + 1;
                    predecessor[i] = Some(dep);
                }
            }
        }

        // Find end of critical path
        let end = longest
            .iter()
            .enumerate()
            .max_by_key(|(_, &l)| l)
            .map(|(i, _)| i)
            .unwrap_or(0);

        // Backtrack
        let mut path = vec![end];
        let mut current = end;
        while let Some(prev) = predecessor[current] {
            path.push(prev);
            current = prev;
        }
        path.reverse();
        path
    }

    /// Get step importance weights
    pub fn get_step_weights(&self) -> &[f32] {
        &self.step_weights
    }
}

// ============================================================================
// 3. Graph Attention (GAT-style)
// ============================================================================

/// Edge in a graph for attention
#[derive(Clone, Debug)]
pub struct Edge {
    /// Source node index
    pub source: usize,
    /// Destination node index
    pub target: usize,
    /// Edge type/relation
    pub edge_type: u8,
    /// Edge weight
    pub weight: f32,
    /// Edge features (optional)
    pub features: Option<Vec<f32>>,
}

/// Graph Attention Network (GAT) style attention
///
/// Answers: "What relationships matter?"
///
/// Implements:
/// - Edge-aware attention weights
/// - Multi-hop relationship scoring
/// - Node importance via attention aggregation
pub struct GraphAttentionNetwork {
    config: UnifiedAttentionConfig,
    /// Node feature projection
    w_node: Vec<f32>,
    /// Attention mechanism (source side)
    a_src: Vec<f32>,
    /// Attention mechanism (target side)
    a_tgt: Vec<f32>,
    /// Edge type embeddings
    edge_embeddings: Vec<Vec<f32>>,
    /// Edge feature projection
    w_edge: Vec<f32>,
    /// LeakyReLU negative slope
    leaky_slope: f32,
    /// Edge dimension
    edge_dim: usize,
}

impl GraphAttentionNetwork {
    /// Create with default configuration
    pub fn new(hidden_dim: usize, num_heads: usize, num_edge_types: usize) -> Result<Self, String> {
        Self::with_config(
            UnifiedAttentionConfig {
                hidden_dim,
                num_heads,
                ..Default::default()
            },
            16,
            num_edge_types,
        )
    }

    /// Create with custom configuration
    pub fn with_config(
        config: UnifiedAttentionConfig,
        edge_dim: usize,
        num_edge_types: usize,
    ) -> Result<Self, String> {
        let h = config.hidden_dim;
        if h % config.num_heads != 0 {
            return Err(format!(
                "hidden_dim {} must be divisible by num_heads {}",
                h, config.num_heads
            ));
        }

        let size = h * h;
        let scale = (2.0 / h as f32).sqrt();

        Ok(Self {
            w_node: (0..size).map(|i| (i as f32 * 0.07).sin() * scale).collect(),
            a_src: (0..h).map(|i| (i as f32 * 0.09).cos() * scale).collect(),
            a_tgt: (0..h).map(|i| (i as f32 * 0.11).sin() * scale).collect(),
            edge_embeddings: (0..num_edge_types.max(1))
                .map(|t| (0..h).map(|i| ((t * i) as f32 * 0.1).sin() * scale).collect())
                .collect(),
            w_edge: (0..edge_dim * h).map(|i| (i as f32 * 0.13).sin() * scale * 0.3).collect(),
            leaky_slope: 0.2,
            edge_dim,
            config,
        })
    }

    /// Forward pass: compute attention over graph
    pub fn forward(&self, node_features: &[Vec<f32>], edges: &[Edge]) -> AttentionOutput {
        self.forward_with_query(node_features, edges, None)
    }

    /// Forward with specific query node
    pub fn forward_with_query(
        &self,
        node_features: &[Vec<f32>],
        edges: &[Edge],
        query_node: Option<usize>,
    ) -> AttentionOutput {
        let n = node_features.len();
        if n == 0 {
            return AttentionOutput::new(vec![], vec![], self.config.top_k)
                .with_type(AttentionType::Graph);
        }

        let h = self.config.hidden_dim;

        // Project all nodes using SIMD
        let projected: Vec<Vec<f32>> = node_features
            .iter()
            .map(|f| SimdCompute::matvec_simd(&self.w_node, f, h, f.len()))
            .collect();

        // Compute attention scores per edge
        let mut attention_weights = vec![vec![0.0f32; n]; n];
        let mut edge_scores: Vec<f32> = Vec::with_capacity(edges.len());

        // Filter edges if query node specified
        let relevant_edges: Vec<&Edge> = if let Some(q) = query_node {
            edges.iter().filter(|e| e.source == q || e.target == q).collect()
        } else {
            edges.iter().collect()
        };

        for edge in &relevant_edges {
            if edge.source >= n || edge.target >= n {
                edge_scores.push(0.0);
                continue;
            }

            let src = &projected[edge.source];
            let tgt = &projected[edge.target];

            // Attention score: a_src * h_src + a_tgt * h_tgt + edge_emb
            let mut score = SimdCompute::dot_product(src, &self.a_src)
                + SimdCompute::dot_product(tgt, &self.a_tgt);

            // Add edge type embedding
            let edge_type_idx = edge.edge_type as usize % self.edge_embeddings.len();
            let edge_emb = &self.edge_embeddings[edge_type_idx];
            score += SimdCompute::dot_product(src, edge_emb) * 0.2;

            // Add edge features if present
            if let Some(ref ef) = edge.features {
                let edge_proj = SimdCompute::matvec_simd(&self.w_edge, ef, h, ef.len().min(self.edge_dim));
                score += SimdCompute::dot_product(&edge_proj, tgt) * 0.3;
            }

            // Apply edge weight and LeakyReLU
            score *= edge.weight;
            score = if score > 0.0 { score } else { score * self.leaky_slope };

            attention_weights[edge.source][edge.target] = score;
            edge_scores.push(score.abs());
        }

        // Softmax per source node
        let mut node_importance = vec![0.0f32; n];
        for src in 0..n {
            let neighbors: Vec<usize> = relevant_edges
                .iter()
                .filter(|e| e.source == src && e.target < n)
                .map(|e| e.target)
                .collect();

            if neighbors.is_empty() {
                continue;
            }

            let mut scores: Vec<f32> = neighbors.iter().map(|&j| attention_weights[src][j]).collect();
            SimdCompute::softmax_simd(&mut scores);

            for (i, &dst) in neighbors.iter().enumerate() {
                attention_weights[src][dst] = scores[i];
                node_importance[dst] += scores[i];
            }
        }

        // Normalize node importance
        let sum: f32 = node_importance.iter().sum();
        if sum > 1e-10 {
            for imp in &mut node_importance {
                *imp /= sum;
            }
        }

        // Compute attended representation
        let mut attended = vec![0.0f32; h];
        if let Some(q) = query_node {
            // Aggregate neighbors of query
            for edge in &relevant_edges {
                if edge.source == q && edge.target < n {
                    let weight = attention_weights[edge.source][edge.target];
                    for j in 0..h.min(projected[edge.target].len()) {
                        attended[j] += weight * projected[edge.target][j];
                    }
                }
            }
            if self.config.residual && q < n {
                for j in 0..h.min(node_features[q].len()) {
                    attended[j] += node_features[q][j];
                }
            }
        } else {
            // Global aggregation
            for (i, feat) in projected.iter().enumerate() {
                for j in 0..h.min(feat.len()) {
                    attended[j] += node_importance[i] * feat[j];
                }
            }
        }

        // Normalize edge scores
        let max_edge = edge_scores.iter().cloned().fold(0.0f32, f32::max);
        if max_edge > 1e-10 {
            for s in &mut edge_scores {
                *s /= max_edge;
            }
        }

        AttentionOutput::new(attended, edge_scores, self.config.top_k)
            .with_weights(attention_weights)
            .with_type(AttentionType::Graph)
    }

    /// Multi-hop attention (aggregates k hops)
    pub fn forward_multihop(
        &self,
        node_features: &[Vec<f32>],
        edges: &[Edge],
        query_node: usize,
        hops: usize,
    ) -> AttentionOutput {
        let h = self.config.hidden_dim;
        let mut current = node_features.get(query_node).cloned().unwrap_or(vec![0.0; h]);
        let mut cumulative_importance = vec![0.0f32; node_features.len()];

        for hop in 0..hops {
            let hop_weight = 1.0 / (1.0 + hop as f32);
            let output = self.forward_with_query(node_features, edges, Some(query_node));

            for (i, &imp) in output.importance.iter().enumerate() {
                if i < cumulative_importance.len() {
                    cumulative_importance[i] += imp * hop_weight;
                }
            }

            for j in 0..current.len().min(output.embeddings.len()) {
                current[j] = current[j] * 0.5 + output.embeddings[j] * 0.5;
            }
        }

        // Normalize
        let sum: f32 = cumulative_importance.iter().sum();
        if sum > 1e-10 {
            for imp in &mut cumulative_importance {
                *imp /= sum;
            }
        }

        AttentionOutput::new(current, cumulative_importance, self.config.top_k)
            .with_type(AttentionType::Graph)
    }
}

// ============================================================================
// 4. State Space Attention (Mamba-style)
// ============================================================================

/// Selective State Space Model (Mamba-inspired)
///
/// Answers: "What history matters?"
///
/// Implements:
/// - Input-dependent gating for history relevance
/// - Exponential decay with learned rates (HiPPO-inspired)
/// - Efficient O(n) sequence processing
pub struct StateSpaceModel {
    config: UnifiedAttentionConfig,
    /// State dimension
    state_dim: usize,
    /// Input projection to state update [hidden_dim, state_dim]
    w_b: Vec<f32>,
    /// State to output projection [state_dim, hidden_dim]
    w_c: Vec<f32>,
    /// Input-dependent delta (selection) [hidden_dim, state_dim]
    w_delta: Vec<f32>,
    /// Discretization factor base
    delta_base: Vec<f32>,
    /// Decay rates (learned A matrix diagonal)
    a_diag: Vec<f32>,
    /// Skip connection weight
    d_skip: Vec<f32>,
}

impl StateSpaceModel {
    /// Create with default configuration
    pub fn new(hidden_dim: usize, state_dim: usize) -> Self {
        Self::with_config(
            UnifiedAttentionConfig {
                hidden_dim,
                state_dim,
                ..Default::default()
            },
        )
    }

    /// Create with custom configuration
    pub fn with_config(config: UnifiedAttentionConfig) -> Self {
        let h = config.hidden_dim;
        let s = config.state_dim;
        let scale = (2.0 / (h + s) as f32).sqrt();

        // Initialize A with HiPPO-inspired exponential decay
        let a_diag: Vec<f32> = (0..s).map(|i| -0.5 - (i as f32 * 0.1)).collect();

        Self {
            state_dim: s,
            w_b: (0..h * s).map(|i| (i as f32 * 0.05).sin() * scale).collect(),
            w_c: (0..s * h).map(|i| (i as f32 * 0.07).cos() * scale).collect(),
            w_delta: (0..h * s).map(|i| (i as f32 * 0.03).sin() * scale * 0.5).collect(),
            delta_base: vec![0.1; s],
            a_diag,
            d_skip: vec![0.1; h],
            config,
        }
    }

    /// Forward pass over sequence with O(n) complexity
    pub fn forward(&self, sequence: &[Vec<f32>]) -> AttentionOutput {
        let seq_len = sequence.len();
        let h = self.config.hidden_dim;
        let s = self.state_dim;

        if seq_len == 0 {
            return AttentionOutput::new(vec![], vec![], self.config.top_k)
                .with_type(AttentionType::StateSpace);
        }

        // Initialize state
        let mut state = vec![0.0f32; s];
        let mut outputs = vec![vec![0.0f32; h]; seq_len];
        let mut history_importance = vec![0.0f32; seq_len];
        let mut history_weights = vec![vec![0.0f32; seq_len]; seq_len];

        for (t, x_t) in sequence.iter().enumerate() {
            // Compute input-dependent selection (delta) using SIMD
            let delta_raw = SimdCompute::matvec_simd(&self.w_delta, x_t, s, x_t.len());
            let delta: Vec<f32> = delta_raw
                .iter()
                .zip(&self.delta_base)
                .map(|(d, base)| {
                    // Softplus for positive delta
                    let softplus = if *d > 20.0 { *d } else { (1.0 + d.exp()).ln() };
                    softplus * base
                })
                .collect();

            // Compute B * x (input contribution)
            let b_x = SimdCompute::matvec_simd(&self.w_b, x_t, s, x_t.len());

            // Selective state update: state = exp(A * delta) * state + delta * B * x
            for i in 0..s {
                let decay = (self.a_diag[i] * delta[i]).exp();
                state[i] = decay * state[i] + delta[i] * b_x[i];
            }

            // Track how much this input affected the state
            let input_contribution: f32 = delta.iter().zip(&b_x).map(|(d, b)| (d * b).abs()).sum();
            history_importance[t] = input_contribution;

            // Compute contribution from past to current
            for past in 0..=t {
                let distance = (t - past) as f32;  // Always non-negative since past <= t
                let decay = (-distance / (seq_len as f32 / 2.0).max(1.0)).exp();
                history_weights[t][past] = history_importance[past] * decay;
            }

            // Normalize history weights
            let hw_sum: f32 = history_weights[t].iter().sum();
            if hw_sum > 1e-10 {
                for w in &mut history_weights[t] {
                    *w /= hw_sum;
                }
            }

            // Compute output: y = C * state + D * x (skip connection)
            let y = SimdCompute::matvec_simd(&self.w_c, &state, h, s);
            for j in 0..h {
                outputs[t][j] = y[j];
                if j < x_t.len() {
                    outputs[t][j] += self.d_skip[j] * x_t[j];
                }
            }
        }

        // Normalize history importance
        let sum: f32 = history_importance.iter().sum();
        if sum > 1e-10 {
            for imp in &mut history_importance {
                *imp /= sum;
            }
        }

        let embeddings: Vec<f32> = outputs.into_iter().flatten().collect();

        AttentionOutput::new(embeddings, history_importance, self.config.top_k)
            .with_weights(history_weights)
            .with_type(AttentionType::StateSpace)
    }

    /// Get state dimension
    pub fn get_state_dim(&self) -> usize {
        self.state_dim
    }
}

// ============================================================================
// Unified Attention Module
// ============================================================================

/// Unified attention combining all four paradigms
///
/// Provides a single interface to:
/// - Process tokens with neural attention
/// - Process DAGs with topological attention
/// - Process graphs with GAT-style attention
/// - Process sequences with state space models
pub struct UnifiedAttention {
    /// Neural (token) attention
    pub neural: NeuralAttention,
    /// DAG (step) attention
    pub dag: DAGAttention,
    /// Graph (relationship) attention
    pub graph: GraphAttentionNetwork,
    /// State space (history) attention
    pub state_space: StateSpaceModel,
    /// Configuration
    config: UnifiedAttentionConfig,
    /// Fusion weights [neural, dag, graph, ssm]
    fusion_weights: [f32; 4],
}

impl UnifiedAttention {
    /// Create with default configuration
    pub fn new(hidden_dim: usize, num_heads: usize) -> Result<Self, String> {
        Self::with_config(UnifiedAttentionConfig {
            hidden_dim,
            num_heads,
            ..Default::default()
        })
    }

    /// Create with custom configuration
    pub fn with_config(config: UnifiedAttentionConfig) -> Result<Self, String> {
        Ok(Self {
            neural: NeuralAttention::with_config(config.clone())?,
            dag: DAGAttention::with_config(config.clone(), 32),
            graph: GraphAttentionNetwork::with_config(config.clone(), 16, 8)?,
            state_space: StateSpaceModel::with_config(config.clone()),
            fusion_weights: [0.25, 0.25, 0.25, 0.25],
            config,
        })
    }

    /// Set fusion weights for combining attention outputs
    pub fn with_fusion_weights(mut self, weights: [f32; 4]) -> Self {
        let sum: f32 = weights.iter().sum();
        if sum > 0.0 {
            self.fusion_weights = weights.map(|w| w / sum);
        }
        self
    }

    /// Forward pass with all available context
    pub fn forward(
        &self,
        tokens: Option<&[Vec<f32>]>,
        dag_nodes: Option<&[DAGNode]>,
        graph_data: Option<(&[Vec<f32>], &[Edge])>,
        history: Option<&[Vec<f32>]>,
        query: &[f32],
    ) -> AttentionOutput {
        let h = self.config.hidden_dim;
        let mut fused_output = vec![0.0f32; h];
        let mut combined_importance = Vec::new();
        let mut active_weights = 0.0f32;

        // 1. Neural attention on tokens
        if let Some(toks) = tokens {
            if !toks.is_empty() {
                let neural_out = self.neural.forward(toks);
                for j in 0..h.min(neural_out.embeddings.len() / toks.len().max(1)) {
                    fused_output[j] += self.fusion_weights[0] * neural_out.embeddings[j];
                }
                combined_importance.extend(
                    neural_out.importance.iter().map(|&s| s * self.fusion_weights[0]),
                );
                active_weights += self.fusion_weights[0];
            }
        }

        // 2. DAG attention
        if let Some(nodes) = dag_nodes {
            if !nodes.is_empty() {
                let dag_out = self.dag.forward_with_query(nodes, query);
                for j in 0..h.min(dag_out.embeddings.len()) {
                    fused_output[j] += self.fusion_weights[1] * dag_out.embeddings[j];
                }
                combined_importance.extend(
                    dag_out.importance.iter().map(|&s| s * self.fusion_weights[1]),
                );
                active_weights += self.fusion_weights[1];
            }
        }

        // 3. Graph attention
        if let Some((nodes, edges)) = graph_data {
            if !nodes.is_empty() {
                let graph_out = self.graph.forward(nodes, edges);
                for j in 0..h.min(graph_out.embeddings.len()) {
                    fused_output[j] += self.fusion_weights[2] * graph_out.embeddings[j];
                }
                combined_importance.extend(
                    graph_out.importance.iter().map(|&s| s * self.fusion_weights[2]),
                );
                active_weights += self.fusion_weights[2];
            }
        }

        // 4. State space on history
        if let Some(hist) = history {
            if !hist.is_empty() {
                let ssm_out = self.state_space.forward(hist);
                for j in 0..h.min(ssm_out.embeddings.len() / hist.len().max(1)) {
                    fused_output[j] += self.fusion_weights[3] * ssm_out.embeddings[j];
                }
                combined_importance.extend(
                    ssm_out.importance.iter().map(|&s| s * self.fusion_weights[3]),
                );
                active_weights += self.fusion_weights[3];
            }
        }

        // Renormalize if not all types used
        if active_weights > 0.0 && active_weights < 1.0 {
            let scale = 1.0 / active_weights;
            for o in &mut fused_output {
                *o *= scale;
            }
        }

        AttentionOutput::new(fused_output, combined_importance, self.config.top_k)
    }

    /// Process with all attention types and return individual results
    pub fn forward_all(
        &self,
        tokens: &[Vec<f32>],
        dag_nodes: Option<&[DAGNode]>,
        graph_data: Option<(&[Vec<f32>], &[Edge])>,
    ) -> HashMap<AttentionType, AttentionOutput> {
        let mut results = HashMap::new();

        if !tokens.is_empty() {
            results.insert(AttentionType::Neural, self.neural.forward(tokens));
            results.insert(AttentionType::StateSpace, self.state_space.forward(tokens));
        }

        if let Some(nodes) = dag_nodes {
            results.insert(AttentionType::DAG, self.dag.forward(nodes));
        }

        if let Some((node_features, edges)) = graph_data {
            results.insert(AttentionType::Graph, self.graph.forward(node_features, edges));
        }

        results
    }

    /// Get unified importance scores
    pub fn get_unified_importance(&self, results: &HashMap<AttentionType, AttentionOutput>) -> Vec<f32> {
        let max_len = results.values().map(|r| r.importance.len()).max().unwrap_or(0);

        if max_len == 0 {
            return vec![];
        }

        let mut unified = vec![0.0f32; max_len];
        let mut weight_sum = 0.0f32;

        let types = [
            (AttentionType::Neural, self.fusion_weights[0]),
            (AttentionType::DAG, self.fusion_weights[1]),
            (AttentionType::Graph, self.fusion_weights[2]),
            (AttentionType::StateSpace, self.fusion_weights[3]),
        ];

        for (attention_type, weight) in types {
            if let Some(output) = results.get(&attention_type) {
                for (i, &imp) in output.importance.iter().enumerate() {
                    if i < max_len {
                        unified[i] += weight * imp;
                    }
                }
                weight_sum += weight;
            }
        }

        if weight_sum > 0.0 {
            for u in &mut unified {
                *u /= weight_sum;
            }
        }

        unified
    }

    /// Get configuration
    pub fn config(&self) -> &UnifiedAttentionConfig {
        &self.config
    }
}

impl Default for UnifiedAttention {
    fn default() -> Self {
        Self::new(128, 8).expect("Default config should be valid")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_output_creation() {
        let embeddings = vec![1.0, 2.0, 3.0];
        let scores = vec![0.1, 0.3, 0.2, 0.4];
        let output = AttentionOutput::new(embeddings.clone(), scores.clone(), 2);

        assert_eq!(output.embeddings, embeddings);
        assert_eq!(output.importance.len(), 4);
        assert_eq!(output.top_k_indices.len(), 2);
        assert_eq!(output.top_k_indices[0], 3);
        assert!(output.metadata.entropy >= 0.0);
    }

    #[test]
    fn test_neural_attention_creation() {
        let attn = NeuralAttention::new(64, 8);
        assert!(attn.is_ok());
    }

    #[test]
    fn test_neural_attention_invalid_heads() {
        let attn = NeuralAttention::new(100, 8);
        assert!(attn.is_err());
    }

    #[test]
    fn test_neural_attention_forward() {
        let attn = NeuralAttention::new(32, 4).unwrap();
        let tokens = vec![vec![0.5; 32], vec![0.3; 32], vec![0.7; 32]];
        let output = attn.forward(&tokens);

        assert!(!output.embeddings.is_empty());
        assert_eq!(output.importance.len(), 3);
        assert!(output.importance.iter().all(|&s| s >= 0.0 && s <= 1.0));
        assert!(output.attention_weights.is_some());
    }

    #[test]
    fn test_neural_attention_with_positions() {
        let attn = NeuralAttention::new(64, 8).unwrap().with_positions(100);
        let tokens = vec![vec![1.0; 64], vec![1.0; 64]];
        let output = attn.forward(&tokens);
        assert!(!output.embeddings.is_empty());
    }

    #[test]
    fn test_dag_attention() {
        let attn = DAGAttention::new(64);
        let nodes = vec![
            DAGNode { id: 0, embedding: vec![1.0; 64], dependencies: vec![] },
            DAGNode { id: 1, embedding: vec![0.5; 64], dependencies: vec![0] },
            DAGNode { id: 2, embedding: vec![0.2; 64], dependencies: vec![0, 1] },
        ];

        let output = attn.forward(&nodes);
        assert_eq!(output.importance.len(), 3);
        assert!(output.importance[0] > 0.0);
    }

    #[test]
    fn test_graph_attention() {
        let attn = GraphAttentionNetwork::new(64, 8, 4).unwrap();
        let features = vec![vec![1.0; 64], vec![0.5; 64], vec![0.2; 64]];
        let edges = vec![
            Edge { source: 0, target: 1, edge_type: 0, weight: 1.0, features: None },
            Edge { source: 1, target: 2, edge_type: 1, weight: 0.5, features: None },
        ];

        let output = attn.forward(&features, &edges);
        assert_eq!(output.importance.len(), 2);
        assert!(output.attention_weights.is_some());
    }

    #[test]
    fn test_graph_attention_multihop() {
        let attn = GraphAttentionNetwork::new(64, 8, 4).unwrap();
        let nodes: Vec<Vec<f32>> = (0..5).map(|i| vec![(i as f32 + 1.0) * 0.2; 64]).collect();
        let edges: Vec<Edge> = (1..5)
            .map(|i| Edge { source: 0, target: i, edge_type: 0, weight: 1.0, features: None })
            .collect();

        let output = attn.forward_multihop(&nodes, &edges, 0, 2);
        assert_eq!(output.importance.len(), 5);
    }

    #[test]
    fn test_state_space() {
        let ssm = StateSpaceModel::new(64, 16);
        let sequence = vec![vec![1.0; 64], vec![0.5; 64], vec![0.2; 64], vec![0.1; 64]];

        let output = ssm.forward(&sequence);
        assert_eq!(output.importance.len(), 4);
        assert!(output.attention_weights.is_some());
    }

    #[test]
    fn test_state_space_empty() {
        let ssm = StateSpaceModel::new(64, 16);
        let output = ssm.forward(&[]);
        assert!(output.importance.is_empty());
    }

    #[test]
    fn test_unified_attention_creation() {
        let attn = UnifiedAttention::new(64, 8);
        assert!(attn.is_ok());
    }

    #[test]
    fn test_unified_attention_forward() {
        let unified = UnifiedAttention::new(64, 8).unwrap();
        let tokens = vec![vec![1.0; 64], vec![0.5; 64]];
        let dag_nodes = vec![
            DAGNode { id: 0, embedding: vec![1.0; 64], dependencies: vec![] },
            DAGNode { id: 1, embedding: vec![0.5; 64], dependencies: vec![0] },
        ];
        let features = vec![vec![1.0; 64], vec![0.5; 64]];
        let edges = vec![Edge { source: 0, target: 1, edge_type: 0, weight: 1.0, features: None }];

        let results = unified.forward_all(&tokens, Some(&dag_nodes), Some((&features, &edges)));

        assert!(results.contains_key(&AttentionType::Neural));
        assert!(results.contains_key(&AttentionType::DAG));
        assert!(results.contains_key(&AttentionType::Graph));
        assert!(results.contains_key(&AttentionType::StateSpace));

        let unified_importance = unified.get_unified_importance(&results);
        assert!(!unified_importance.is_empty());
    }

    #[test]
    fn test_unified_forward_combined() {
        let unified = UnifiedAttention::new(64, 8).unwrap();
        let tokens = vec![vec![0.5; 64]];
        let query = vec![0.6; 64];

        let output = unified.forward(Some(&tokens), None, None, None, &query);
        assert!(!output.embeddings.is_empty());
    }

    #[test]
    fn test_attention_output_normalized() {
        let scores = vec![0.1, 0.5, 0.4];
        let output = AttentionOutput::new(vec![1.0, 2.0], scores, 2);

        let normalized = output.normalized_importance();
        assert!((normalized[1] - 1.0).abs() < 0.01);
        assert!(normalized[0] < normalized[1]);
    }

    #[test]
    fn test_fusion_weight_normalization() {
        let unified = UnifiedAttention::new(64, 8)
            .unwrap()
            .with_fusion_weights([2.0, 1.0, 1.0, 0.0]);

        let sum: f32 = unified.fusion_weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.01);
    }
}
