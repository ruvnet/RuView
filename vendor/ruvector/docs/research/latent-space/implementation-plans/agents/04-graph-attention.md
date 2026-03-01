# Agent 4: Graph Attention Implementations

## Overview

This agent implements graph-aware attention mechanisms that leverage both structural and latent space information from HNSW indices. The implementations bridge traditional GNN attention with modern transformer-style mechanisms adapted for graph structures.

## Architecture Components

### 1. EdgeFeaturedAttention
### 2. GraphRoPE (Rotary Position Embeddings for Graphs)
### 3. DualSpaceAttention (Cross-Attention between Graph and Latent Space)

---

## 1. EdgeFeaturedAttention

Integrates edge features directly into attention computation, extending GAT to handle rich edge information stored in HNSW connections.

### Design Principles

- **Edge-Aware Scoring**: Attention coefficients incorporate edge features
- **LeakyReLU Activation**: Standard GAT activation for gradient flow
- **HNSW Integration**: Leverages edge weights and multi-level connections

### Implementation

```rust
use ndarray::{Array1, Array2, ArrayView1, ArrayView2, s};
use std::collections::HashMap;

/// Edge-Featured Attention Mechanism
/// Computes attention over graph neighbors with edge feature integration
pub struct EdgeFeaturedAttention {
    /// Query projection weights [hidden_dim, hidden_dim]
    w_query: Array2<f32>,

    /// Key projection weights [hidden_dim, hidden_dim]
    w_key: Array2<f32>,

    /// Value projection weights [hidden_dim, hidden_dim]
    w_value: Array2<f32>,

    /// Edge feature projection [edge_dim, hidden_dim]
    w_edge: Array2<f32>,

    /// Attention scoring vector [2 * hidden_dim + hidden_dim]
    /// Concatenates [query || key || edge_features]
    a: Array1<f32>,

    /// LeakyReLU negative slope
    negative_slope: f32,

    /// Hidden dimension size
    hidden_dim: usize,

    /// Edge feature dimension
    edge_dim: usize,
}

impl EdgeFeaturedAttention {
    /// Create new EdgeFeaturedAttention layer
    pub fn new(hidden_dim: usize, edge_dim: usize, negative_slope: f32) -> Self {
        // Initialize with Xavier/Glorot uniform initialization
        let bound_h = (6.0 / (hidden_dim as f32 * 2.0)).sqrt();
        let bound_e = (6.0 / (edge_dim as f32 + hidden_dim as f32)).sqrt();
        let bound_a = (6.0 / (3.0 * hidden_dim as f32)).sqrt();

        Self {
            w_query: Array2::from_shape_fn((hidden_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound_h - bound_h
            }),
            w_key: Array2::from_shape_fn((hidden_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound_h - bound_h
            }),
            w_value: Array2::from_shape_fn((hidden_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound_h - bound_h
            }),
            w_edge: Array2::from_shape_fn((edge_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound_e - bound_e
            }),
            a: Array1::from_shape_fn(3 * hidden_dim, |_| {
                rand::random::<f32>() * 2.0 * bound_a - bound_a
            }),
            negative_slope,
            hidden_dim,
            edge_dim,
        }
    }

    /// Apply LeakyReLU activation
    #[inline]
    fn leaky_relu(&self, x: f32) -> f32 {
        if x >= 0.0 {
            x
        } else {
            self.negative_slope * x
        }
    }

    /// Compute attention coefficients for a single node
    ///
    /// # HNSW Integration Points:
    /// - `neighbor_ids`: Retrieved from HNSW.get_neighbors(node_id, layer)
    /// - `edge_features`: Stored in HNSW edge metadata or computed from distance
    /// - `layer`: HNSW layer level affects neighbor selection
    fn compute_attention_scores(
        &self,
        query_node: ArrayView1<f32>,           // [hidden_dim]
        neighbor_features: ArrayView2<f32>,   // [num_neighbors, hidden_dim]
        edge_features: ArrayView2<f32>,       // [num_neighbors, edge_dim]
    ) -> Array1<f32> {
        let num_neighbors = neighbor_features.nrows();

        // Project query, keys, and values
        let query = query_node.dot(&self.w_query);  // [hidden_dim]
        let keys = neighbor_features.dot(&self.w_key.t());  // [num_neighbors, hidden_dim]
        let edges_proj = edge_features.dot(&self.w_edge.t());  // [num_neighbors, hidden_dim]

        // Compute attention logits for each neighbor
        let mut logits = Array1::zeros(num_neighbors);

        for i in 0..num_neighbors {
            // Concatenate [query || key_i || edge_i]
            let mut concat = Array1::zeros(3 * self.hidden_dim);
            concat.slice_mut(s![0..self.hidden_dim]).assign(&query);
            concat.slice_mut(s![self.hidden_dim..2*self.hidden_dim])
                .assign(&keys.row(i));
            concat.slice_mut(s![2*self.hidden_dim..])
                .assign(&edges_proj.row(i));

            // Compute attention score: a^T * LeakyReLU(concat)
            let score: f32 = concat.iter()
                .zip(self.a.iter())
                .map(|(x, a)| self.leaky_relu(*x) * a)
                .sum();

            logits[i] = score;
        }

        // Apply softmax
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Array1<f32> = logits.mapv(|x| (x - max_logit).exp());
        let sum_exp = exp_logits.sum();

        exp_logits / sum_exp
    }

    /// Forward pass: compute attended features for all nodes
    ///
    /// # HNSW Integration:
    /// ```rust
    /// // Pseudo-code for HNSW integration:
    /// for node_id in graph.nodes() {
    ///     // Get neighbors from HNSW at specific layer
    ///     let neighbors = hnsw.get_neighbors(node_id, layer);
    ///
    ///     // Extract edge features from HNSW metadata
    ///     let edge_feats = neighbors.iter().map(|&n| {
    ///         hnsw.get_edge_features(node_id, n, layer)
    ///     }).collect();
    ///
    ///     // Compute attention
    ///     let attended = self.forward_single(
    ///         node_features.row(node_id),
    ///         neighbor_features,
    ///         edge_feats
    ///     );
    /// }
    /// ```
    pub fn forward(
        &self,
        node_features: ArrayView2<f32>,       // [num_nodes, hidden_dim]
        adjacency: &HashMap<usize, Vec<usize>>,  // node_id -> neighbor_ids
        edge_features_map: &HashMap<(usize, usize), Array1<f32>>,  // (src, dst) -> edge_feat
    ) -> Array2<f32> {
        let num_nodes = node_features.nrows();
        let mut output = Array2::zeros((num_nodes, self.hidden_dim));

        for node_id in 0..num_nodes {
            if let Some(neighbors) = adjacency.get(&node_id) {
                if neighbors.is_empty() {
                    // No neighbors, apply self-loop
                    output.row_mut(node_id).assign(&node_features.row(node_id));
                    continue;
                }

                // Gather neighbor features
                let num_neighbors = neighbors.len();
                let mut neighbor_feats = Array2::zeros((num_neighbors, self.hidden_dim));
                let mut edge_feats = Array2::zeros((num_neighbors, self.edge_dim));

                for (i, &neighbor_id) in neighbors.iter().enumerate() {
                    neighbor_feats.row_mut(i).assign(&node_features.row(neighbor_id));

                    if let Some(edge_feat) = edge_features_map.get(&(node_id, neighbor_id)) {
                        edge_feats.row_mut(i).assign(edge_feat);
                    }
                }

                // Compute attention scores
                let attention_weights = self.compute_attention_scores(
                    node_features.row(node_id),
                    neighbor_feats.view(),
                    edge_feats.view(),
                );

                // Project neighbor features to values
                let values = neighbor_feats.dot(&self.w_value.t());  // [num_neighbors, hidden_dim]

                // Weighted sum of values
                let mut attended = Array1::zeros(self.hidden_dim);
                for i in 0..num_neighbors {
                    attended = attended + &(values.row(i).to_owned() * attention_weights[i]);
                }

                output.row_mut(node_id).assign(&attended);
            } else {
                // Isolated node
                output.row_mut(node_id).assign(&node_features.row(node_id));
            }
        }

        output
    }

    /// Forward pass for a single node (useful for online inference)
    pub fn forward_single(
        &self,
        query_node: ArrayView1<f32>,
        neighbor_features: ArrayView2<f32>,
        edge_features: ArrayView2<f32>,
    ) -> Array1<f32> {
        let attention_weights = self.compute_attention_scores(
            query_node,
            neighbor_features,
            edge_features,
        );

        let values = neighbor_features.dot(&self.w_value.t());

        let mut attended = Array1::zeros(self.hidden_dim);
        for i in 0..values.nrows() {
            attended = attended + &(values.row(i).to_owned() * attention_weights[i]);
        }

        attended
    }
}

// HNSW Integration Helper
pub struct HNSWEdgeFeatureExtractor {
    /// Extract edge features from HNSW metadata
    /// Features can include:
    /// - Edge weight (inverse of distance)
    /// - Layer level
    /// - Neighbor degree
    /// - Edge directionality
}

impl HNSWEdgeFeatureExtractor {
    pub fn extract_features(
        &self,
        src_id: usize,
        dst_id: usize,
        distance: f32,
        layer: usize,
        dst_degree: usize,
    ) -> Array1<f32> {
        // Example edge features [edge_dim = 4]
        Array1::from_vec(vec![
            1.0 / (distance + 1e-6),  // Edge weight (inverse distance)
            layer as f32,              // HNSW layer
            (dst_degree as f32).ln(),  // Log degree of neighbor
            1.0,                       // Bias term
        ])
    }
}
```

### Usage Example

```rust
// Initialize attention layer
let attention = EdgeFeaturedAttention::new(
    128,    // hidden_dim
    4,      // edge_dim
    0.2,    // negative_slope for LeakyReLU
);

// HNSW Integration Example:
// 1. Query HNSW for neighbors
// let neighbors = hnsw.get_neighbors(node_id, layer);
//
// 2. Extract edge features
// let edge_extractor = HNSWEdgeFeatureExtractor::new();
// for &neighbor in neighbors {
//     let distance = hnsw.distance(node_id, neighbor);
//     let edge_feat = edge_extractor.extract_features(
//         node_id, neighbor, distance, layer, hnsw.degree(neighbor)
//     );
// }
//
// 3. Apply attention
// let output = attention.forward(node_features, adjacency, edge_features);
```

---

## 2. GraphRoPE (Rotary Position Embeddings for Graphs)

Adapts RoPE from transformers to encode structural positions in graphs using HNSW distances and layer information.

### Design Principles

- **Distance-Based Rotation**: Rotation angles based on graph distance
- **Layer-Aware Encoding**: Different frequencies per HNSW layer
- **Relative Positioning**: Encodes relative structural positions

### Implementation

```rust
use ndarray::{Array1, Array2, ArrayView1, s};
use std::f32::consts::PI;

/// Graph Rotary Position Embeddings
/// Encodes graph structural positions via rotation in embedding space
pub struct GraphRoPE {
    /// Dimension of embeddings
    dim: usize,

    /// Base frequency for rotations
    base: f32,

    /// Maximum distance to encode
    max_distance: f32,

    /// Number of HNSW layers to support
    num_layers: usize,

    /// Precomputed frequency bands [dim/2]
    inv_freq: Array1<f32>,
}

impl GraphRoPE {
    /// Create new GraphRoPE encoder
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension (must be even)
    /// * `base` - Base frequency (default: 10000.0 like in transformers)
    /// * `max_distance` - Maximum graph distance to encode
    /// * `num_layers` - Number of HNSW layers
    pub fn new(dim: usize, base: f32, max_distance: f32, num_layers: usize) -> Self {
        assert!(dim % 2 == 0, "Dimension must be even for RoPE");

        // Compute inverse frequencies: θ_i = base^(-2i/d) for i in [0, d/2)
        let inv_freq = Array1::from_shape_fn(dim / 2, |i| {
            1.0 / base.powf(2.0 * i as f32 / dim as f32)
        });

        Self {
            dim,
            base,
            max_distance,
            num_layers,
            inv_freq,
        }
    }

    /// Compute rotation matrix for a given distance and layer
    ///
    /// # HNSW Integration:
    /// - `distance`: Graph distance or HNSW distance metric
    /// - `layer`: HNSW layer level (0 = bottom, higher = more abstract)
    ///
    /// Returns: Rotation matrix [dim, dim] (block diagonal with 2x2 rotation blocks)
    fn compute_rotation_matrix(&self, distance: f32, layer: usize) -> Array2<f32> {
        // Layer-dependent frequency scaling
        // Higher layers = lower frequencies = coarser position encoding
        let layer_scale = 1.0 / (1.0 + layer as f32);

        // Normalize distance
        let normalized_dist = (distance / self.max_distance).min(1.0);

        let mut rotation = Array2::eye(self.dim);

        // Create 2D rotation blocks
        for i in 0..self.dim / 2 {
            let freq = self.inv_freq[i] * layer_scale;
            let theta = normalized_dist * freq;

            let cos_theta = theta.cos();
            let sin_theta = theta.sin();

            // 2x2 rotation block
            let idx1 = 2 * i;
            let idx2 = 2 * i + 1;

            rotation[[idx1, idx1]] = cos_theta;
            rotation[[idx1, idx2]] = -sin_theta;
            rotation[[idx2, idx1]] = sin_theta;
            rotation[[idx2, idx2]] = cos_theta;
        }

        rotation
    }

    /// Apply rotary position encoding to embeddings
    ///
    /// # Arguments
    /// * `embeddings` - Input embeddings [batch_size, dim]
    /// * `distances` - Graph distances for each embedding [batch_size]
    /// * `layers` - HNSW layer for each embedding [batch_size]
    ///
    /// # HNSW Integration Example:
    /// ```rust
    /// // For each node, compute distance from query node
    /// let distances = nodes.iter().map(|&node_id| {
    ///     hnsw.shortest_path_distance(query_id, node_id)
    /// }).collect();
    ///
    /// // Use HNSW layer information
    /// let layers = nodes.iter().map(|&node_id| {
    ///     hnsw.get_node_layer(node_id)
    /// }).collect();
    ///
    /// let rotated = rope.apply_rotation(embeddings, &distances, &layers);
    /// ```
    pub fn apply_rotation(
        &self,
        embeddings: ArrayView2<f32>,  // [batch_size, dim]
        distances: &[f32],             // [batch_size]
        layers: &[usize],              // [batch_size]
    ) -> Array2<f32> {
        let batch_size = embeddings.nrows();
        assert_eq!(batch_size, distances.len());
        assert_eq!(batch_size, layers.len());

        let mut output = Array2::zeros((batch_size, self.dim));

        for i in 0..batch_size {
            let rotation = self.compute_rotation_matrix(distances[i], layers[i]);
            let rotated = rotation.dot(&embeddings.row(i));
            output.row_mut(i).assign(&rotated);
        }

        output
    }

    /// Apply rotation to a single embedding
    pub fn apply_rotation_single(
        &self,
        embedding: ArrayView1<f32>,
        distance: f32,
        layer: usize,
    ) -> Array1<f32> {
        let rotation = self.compute_rotation_matrix(distance, layer);
        rotation.dot(&embedding)
    }

    /// Compute relative rotary embeddings between two nodes
    /// This encodes the relative position in graph space
    ///
    /// # HNSW Integration:
    /// ```rust
    /// let query_emb = node_embeddings.row(query_id);
    /// let key_emb = node_embeddings.row(key_id);
    ///
    /// // Get HNSW distance
    /// let distance = hnsw.distance(query_id, key_id);
    /// let layer = hnsw.get_common_layer(query_id, key_id);
    ///
    /// let (rotated_q, rotated_k) = rope.apply_relative_rotation(
    ///     query_emb, key_emb, distance, layer
    /// );
    ///
    /// // Compute attention with relative position encoding
    /// let score = rotated_q.dot(&rotated_k);
    /// ```
    pub fn apply_relative_rotation(
        &self,
        query_emb: ArrayView1<f32>,
        key_emb: ArrayView1<f32>,
        distance: f32,
        layer: usize,
    ) -> (Array1<f32>, Array1<f32>) {
        let rotation = self.compute_rotation_matrix(distance, layer);

        // Apply rotation to both query and key
        let rotated_query = rotation.dot(&query_emb);
        let rotated_key = rotation.dot(&key_emb);

        (rotated_query, rotated_key)
    }

    /// Encode distances as sinusoidal features (alternative to rotation)
    /// Useful for edge features or distance embeddings
    pub fn encode_distance(&self, distance: f32, layer: usize) -> Array1<f32> {
        let layer_scale = 1.0 / (1.0 + layer as f32);
        let normalized_dist = (distance / self.max_distance).min(1.0);

        let mut encoding = Array1::zeros(self.dim);

        for i in 0..self.dim / 2 {
            let freq = self.inv_freq[i] * layer_scale;
            let angle = normalized_dist * freq;

            encoding[2 * i] = angle.sin();
            encoding[2 * i + 1] = angle.cos();
        }

        encoding
    }
}

/// HNSW-Aware Distance Computer
pub struct HNSWDistanceComputer {
    /// Compute graph distances using HNSW structure
}

impl HNSWDistanceComputer {
    /// Compute shortest path distance using HNSW layers
    /// Higher layers provide shortcuts for faster distance computation
    pub fn shortest_path_distance(
        &self,
        hnsw: &dyn HNSWInterface,
        source: usize,
        target: usize,
    ) -> f32 {
        // Start from highest layer for efficiency
        let max_layer = hnsw.get_max_level();

        // BFS with layer-aware traversal
        // Implementation would use HNSW's hierarchical structure
        // to compute distances efficiently

        // Placeholder implementation
        hnsw.distance(source, target)
    }

    /// Get the highest common layer between two nodes
    pub fn get_common_layer(
        &self,
        hnsw: &dyn HNSWInterface,
        node1: usize,
        node2: usize,
    ) -> usize {
        let layer1 = hnsw.get_node_layer(node1);
        let layer2 = hnsw.get_node_layer(node2);
        layer1.min(layer2)
    }
}

// Trait for HNSW interface (abstraction for integration)
pub trait HNSWInterface {
    fn distance(&self, id1: usize, id2: usize) -> f32;
    fn get_max_level(&self) -> usize;
    fn get_node_layer(&self, id: usize) -> usize;
    fn get_neighbors(&self, id: usize, layer: usize) -> Vec<usize>;
}
```

### Usage Example

```rust
// Initialize GraphRoPE
let rope = GraphRoPE::new(
    128,      // dim
    10000.0,  // base (same as transformer RoPE)
    20.0,     // max_distance (graph hops)
    8,        // num_layers (HNSW layers)
);

// HNSW Integration:
// 1. Compute distances from query node
// let distance_computer = HNSWDistanceComputer::new();
// let distances: Vec<f32> = nodes.iter().map(|&node_id| {
//     distance_computer.shortest_path_distance(&hnsw, query_id, node_id)
// }).collect();
//
// 2. Get layer information
// let layers: Vec<usize> = nodes.iter().map(|&node_id| {
//     hnsw.get_node_layer(node_id)
// }).collect();
//
// 3. Apply rotary embeddings
// let rotated_embeddings = rope.apply_rotation(
//     embeddings.view(),
//     &distances,
//     &layers,
// );

// For attention computation with relative positions:
// let (rotated_q, rotated_k) = rope.apply_relative_rotation(
//     query_embedding,
//     key_embedding,
//     distance,
//     layer,
// );
// let attention_score = rotated_q.dot(&rotated_k) / (dim as f32).sqrt();
```

---

## 3. DualSpaceAttention (Cross-Attention)

Performs cross-attention between graph-space neighbors (from original graph structure) and latent-space neighbors (from HNSW index), fusing both structural and semantic information.

### Design Principles

- **Dual Neighbor Sets**: Graph neighbors (structure) + Latent neighbors (semantics)
- **Cross-Attention Fusion**: Attend across both spaces simultaneously
- **HNSW Latent Search**: Leverage HNSW for efficient semantic neighbor retrieval

### Implementation

```rust
use ndarray::{Array1, Array2, ArrayView1, ArrayView2, Axis, s};
use std::collections::{HashMap, HashSet};

/// Dual-Space Cross-Attention
/// Attends to both graph-structure neighbors and latent-space neighbors
pub struct DualSpaceAttention {
    /// Graph-space attention head
    graph_attention: GraphAttentionHead,

    /// Latent-space attention head
    latent_attention: LatentAttentionHead,

    /// Cross-attention fusion layer
    fusion: CrossAttentionFusion,

    /// Hidden dimension
    hidden_dim: usize,

    /// Number of latent neighbors to retrieve from HNSW
    k_latent: usize,
}

impl DualSpaceAttention {
    pub fn new(hidden_dim: usize, k_latent: usize) -> Self {
        Self {
            graph_attention: GraphAttentionHead::new(hidden_dim),
            latent_attention: LatentAttentionHead::new(hidden_dim),
            fusion: CrossAttentionFusion::new(hidden_dim),
            hidden_dim,
            k_latent,
        }
    }

    /// Forward pass with dual-space attention
    ///
    /// # HNSW Integration:
    /// ```rust
    /// // 1. Graph neighbors (from original graph structure)
    /// let graph_neighbors = graph.get_neighbors(node_id);
    ///
    /// // 2. Latent neighbors (from HNSW semantic search)
    /// let latent_neighbors = hnsw.search(
    ///     node_embeddings.row(node_id),
    ///     k_latent,
    ///     layer
    /// );
    ///
    /// // 3. Apply dual-space attention
    /// let output = dual_attention.forward(
    ///     node_id,
    ///     &node_embeddings,
    ///     &graph_neighbors,
    ///     &latent_neighbors,
    /// );
    /// ```
    pub fn forward(
        &self,
        node_id: usize,
        node_embeddings: ArrayView2<f32>,        // [num_nodes, hidden_dim]
        graph_neighbors: &[usize],                // From graph structure
        latent_neighbors: &[(usize, f32)],        // From HNSW (id, distance)
    ) -> Array1<f32> {
        // Query embedding
        let query = node_embeddings.row(node_id);

        // 1. Graph-space attention
        let graph_context = self.graph_attention.attend(
            query,
            graph_neighbors,
            node_embeddings,
        );

        // 2. Latent-space attention
        let latent_context = self.latent_attention.attend(
            query,
            latent_neighbors,
            node_embeddings,
        );

        // 3. Cross-attention fusion
        let fused = self.fusion.fuse(
            query,
            graph_context.view(),
            latent_context.view(),
        );

        fused
    }

    /// Find latent neighbors using HNSW
    ///
    /// # HNSW Integration:
    /// This is a critical integration point where we query HNSW index
    /// to find semantically similar nodes in the latent space
    pub fn find_latent_neighbors(
        &self,
        hnsw: &dyn HNSWInterface,
        query_embedding: ArrayView1<f32>,
        k: usize,
        layer: usize,
    ) -> Vec<(usize, f32)> {
        // HNSW search for k nearest neighbors in latent space
        // Returns: Vec<(node_id, distance)>

        // Pseudo-code for HNSW integration:
        // let results = hnsw.search_layer(
        //     query_embedding,
        //     k,
        //     layer,
        //     ef_search=50  // Search parameter
        // );

        // Placeholder implementation
        vec![]
    }

    /// Batch forward pass for multiple nodes
    pub fn forward_batch(
        &self,
        node_embeddings: ArrayView2<f32>,
        graph_adjacency: &HashMap<usize, Vec<usize>>,
        hnsw: &dyn HNSWInterface,
        layer: usize,
    ) -> Array2<f32> {
        let num_nodes = node_embeddings.nrows();
        let mut output = Array2::zeros((num_nodes, self.hidden_dim));

        for node_id in 0..num_nodes {
            // Get graph neighbors
            let graph_neighbors = graph_adjacency
                .get(&node_id)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            // Get latent neighbors from HNSW
            let latent_neighbors = self.find_latent_neighbors(
                hnsw,
                node_embeddings.row(node_id),
                self.k_latent,
                layer,
            );

            // Apply dual-space attention
            let node_output = self.forward(
                node_id,
                node_embeddings,
                graph_neighbors,
                &latent_neighbors,
            );

            output.row_mut(node_id).assign(&node_output);
        }

        output
    }
}

/// Graph-space attention head (attends to structural neighbors)
struct GraphAttentionHead {
    w_query: Array2<f32>,
    w_key: Array2<f32>,
    w_value: Array2<f32>,
    hidden_dim: usize,
}

impl GraphAttentionHead {
    fn new(hidden_dim: usize) -> Self {
        let bound = (6.0 / (hidden_dim as f32 * 2.0)).sqrt();

        Self {
            w_query: Array2::from_shape_fn((hidden_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound - bound
            }),
            w_key: Array2::from_shape_fn((hidden_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound - bound
            }),
            w_value: Array2::from_shape_fn((hidden_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound - bound
            }),
            hidden_dim,
        }
    }

    fn attend(
        &self,
        query: ArrayView1<f32>,
        neighbor_ids: &[usize],
        node_embeddings: ArrayView2<f32>,
    ) -> Array1<f32> {
        if neighbor_ids.is_empty() {
            return query.to_owned();
        }

        // Project query
        let q = query.dot(&self.w_query);  // [hidden_dim]

        // Gather and project keys and values
        let num_neighbors = neighbor_ids.len();
        let mut keys = Array2::zeros((num_neighbors, self.hidden_dim));
        let mut values = Array2::zeros((num_neighbors, self.hidden_dim));

        for (i, &neighbor_id) in neighbor_ids.iter().enumerate() {
            let neighbor_emb = node_embeddings.row(neighbor_id);
            keys.row_mut(i).assign(&neighbor_emb.dot(&self.w_key));
            values.row_mut(i).assign(&neighbor_emb.dot(&self.w_value));
        }

        // Compute attention scores
        let scale = 1.0 / (self.hidden_dim as f32).sqrt();
        let mut scores = Array1::zeros(num_neighbors);
        for i in 0..num_neighbors {
            scores[i] = q.dot(&keys.row(i)) * scale;
        }

        // Softmax
        let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_scores: Array1<f32> = scores.mapv(|x| (x - max_score).exp());
        let sum_exp = exp_scores.sum();
        let attention_weights = exp_scores / sum_exp;

        // Weighted sum of values
        let mut output = Array1::zeros(self.hidden_dim);
        for i in 0..num_neighbors {
            output = output + &(values.row(i).to_owned() * attention_weights[i]);
        }

        output
    }
}

/// Latent-space attention head (attends to semantic neighbors from HNSW)
struct LatentAttentionHead {
    w_query: Array2<f32>,
    w_key: Array2<f32>,
    w_value: Array2<f32>,
    hidden_dim: usize,
}

impl LatentAttentionHead {
    fn new(hidden_dim: usize) -> Self {
        let bound = (6.0 / (hidden_dim as f32 * 2.0)).sqrt();

        Self {
            w_query: Array2::from_shape_fn((hidden_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound - bound
            }),
            w_key: Array2::from_shape_fn((hidden_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound - bound
            }),
            w_value: Array2::from_shape_fn((hidden_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound - bound
            }),
            hidden_dim,
        }
    }

    fn attend(
        &self,
        query: ArrayView1<f32>,
        latent_neighbors: &[(usize, f32)],  // (neighbor_id, distance)
        node_embeddings: ArrayView2<f32>,
    ) -> Array1<f32> {
        if latent_neighbors.is_empty() {
            return query.to_owned();
        }

        // Project query
        let q = query.dot(&self.w_query);

        let num_neighbors = latent_neighbors.len();
        let mut keys = Array2::zeros((num_neighbors, self.hidden_dim));
        let mut values = Array2::zeros((num_neighbors, self.hidden_dim));

        // Distance-weighted attention bias
        let mut distance_weights = Array1::zeros(num_neighbors);

        for (i, &(neighbor_id, distance)) in latent_neighbors.iter().enumerate() {
            let neighbor_emb = node_embeddings.row(neighbor_id);
            keys.row_mut(i).assign(&neighbor_emb.dot(&self.w_key));
            values.row_mut(i).assign(&neighbor_emb.dot(&self.w_value));

            // Convert HNSW distance to attention bias
            // Closer neighbors (smaller distance) get positive bias
            distance_weights[i] = -distance;
        }

        // Compute attention scores with distance bias
        let scale = 1.0 / (self.hidden_dim as f32).sqrt();
        let mut scores = Array1::zeros(num_neighbors);
        for i in 0..num_neighbors {
            scores[i] = q.dot(&keys.row(i)) * scale + distance_weights[i];
        }

        // Softmax
        let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_scores: Array1<f32> = scores.mapv(|x| (x - max_score).exp());
        let sum_exp = exp_scores.sum();
        let attention_weights = exp_scores / sum_exp;

        // Weighted sum
        let mut output = Array1::zeros(self.hidden_dim);
        for i in 0..num_neighbors {
            output = output + &(values.row(i).to_owned() * attention_weights[i]);
        }

        output
    }
}

/// Cross-attention fusion layer
/// Fuses information from graph-space and latent-space contexts
struct CrossAttentionFusion {
    // Cross-attention: query=original, keys/values=graph_context
    w_graph_key: Array2<f32>,
    w_graph_value: Array2<f32>,

    // Cross-attention: query=original, keys/values=latent_context
    w_latent_key: Array2<f32>,
    w_latent_value: Array2<f32>,

    // Fusion weights
    w_fusion: Array2<f32>,

    hidden_dim: usize,
}

impl CrossAttentionFusion {
    fn new(hidden_dim: usize) -> Self {
        let bound = (6.0 / (hidden_dim as f32 * 2.0)).sqrt();

        Self {
            w_graph_key: Array2::from_shape_fn((hidden_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound - bound
            }),
            w_graph_value: Array2::from_shape_fn((hidden_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound - bound
            }),
            w_latent_key: Array2::from_shape_fn((hidden_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound - bound
            }),
            w_latent_value: Array2::from_shape_fn((hidden_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound - bound
            }),
            w_fusion: Array2::from_shape_fn((2 * hidden_dim, hidden_dim), |_| {
                rand::random::<f32>() * 2.0 * bound - bound
            }),
            hidden_dim,
        }
    }

    fn fuse(
        &self,
        query: ArrayView1<f32>,           // Original node embedding
        graph_context: ArrayView1<f32>,   // From graph-space attention
        latent_context: ArrayView1<f32>,  // From latent-space attention
    ) -> Array1<f32> {
        // Cross-attention with graph context
        let graph_key = graph_context.dot(&self.w_graph_key);
        let graph_value = graph_context.dot(&self.w_graph_value);
        let graph_score = query.dot(&graph_key) / (self.hidden_dim as f32).sqrt();
        let graph_attended = graph_value * graph_score.tanh();  // Gated by attention

        // Cross-attention with latent context
        let latent_key = latent_context.dot(&self.w_latent_key);
        let latent_value = latent_context.dot(&self.w_latent_value);
        let latent_score = query.dot(&latent_key) / (self.hidden_dim as f32).sqrt();
        let latent_attended = latent_value * latent_score.tanh();

        // Concatenate and fuse
        let mut concat = Array1::zeros(2 * self.hidden_dim);
        concat.slice_mut(s![0..self.hidden_dim]).assign(&graph_attended);
        concat.slice_mut(s![self.hidden_dim..]).assign(&latent_attended);

        let fused = concat.dot(&self.w_fusion);

        // Residual connection
        query.to_owned() + fused
    }
}

/// HNSW Search Integration Helper
pub struct HNSWLatentSearch {
    /// Search parameters
    ef_search: usize,
}

impl HNSWLatentSearch {
    pub fn new(ef_search: usize) -> Self {
        Self { ef_search }
    }

    /// Search HNSW for k nearest neighbors in latent space
    ///
    /// # Arguments
    /// * `hnsw` - HNSW index
    /// * `query_embedding` - Query vector
    /// * `k` - Number of neighbors to return
    /// * `layer` - HNSW layer to search (higher = more abstract)
    ///
    /// # Returns
    /// Vec<(node_id, distance)> sorted by distance (ascending)
    pub fn search(
        &self,
        hnsw: &dyn HNSWInterface,
        query_embedding: ArrayView1<f32>,
        k: usize,
        layer: usize,
    ) -> Vec<(usize, f32)> {
        // Pseudo-code for HNSW integration:
        //
        // 1. Start from entry point at given layer
        // let entry_point = hnsw.get_entry_point(layer);
        //
        // 2. Greedy search to find closest node
        // let mut current = entry_point;
        // loop {
        //     let neighbors = hnsw.get_neighbors(current, layer);
        //     let (closest, dist) = find_closest(neighbors, query_embedding);
        //     if dist >= current_dist { break; }
        //     current = closest;
        // }
        //
        // 3. Beam search for k neighbors
        // let mut candidates = PriorityQueue::new();
        // let mut results = PriorityQueue::new();
        // candidates.push(current, distance);
        //
        // while !candidates.is_empty() && results.len() < ef_search {
        //     let (node, dist) = candidates.pop();
        //     if dist > results.peek().dist { break; }
        //
        //     for neighbor in hnsw.get_neighbors(node, layer) {
        //         let neighbor_dist = distance(query_embedding, neighbor_embedding);
        //         if neighbor_dist < results.peek().dist {
        //             candidates.push(neighbor, neighbor_dist);
        //             results.push(neighbor, neighbor_dist);
        //             if results.len() > ef_search {
        //                 results.pop();
        //             }
        //         }
        //     }
        // }
        //
        // 4. Return top-k results
        // results.into_iter().take(k).collect()

        // Placeholder
        vec![]
    }
}
```

### Usage Example

```rust
// Initialize dual-space attention
let dual_attention = DualSpaceAttention::new(
    128,  // hidden_dim
    16,   // k_latent neighbors
);

// HNSW Integration Example:
// 1. Get graph neighbors (from original graph)
// let graph_neighbors = graph.adjacency.get(&node_id).unwrap();
//
// 2. Search HNSW for latent neighbors
// let hnsw_search = HNSWLatentSearch::new(50);  // ef_search=50
// let latent_neighbors = hnsw_search.search(
//     &hnsw,
//     node_embeddings.row(node_id),
//     16,    // k
//     layer,
// );
//
// 3. Apply dual-space attention
// let output = dual_attention.forward(
//     node_id,
//     node_embeddings.view(),
//     graph_neighbors,
//     &latent_neighbors,
// );

// Batch processing:
// let output_embeddings = dual_attention.forward_batch(
//     node_embeddings.view(),
//     &graph_adjacency,
//     &hnsw,
//     layer,
// );
```

---

## Integration Architecture

### Complete Pipeline with HNSW

```rust
pub struct GraphAttentionPipeline {
    /// Edge-featured attention for local neighborhood
    edge_attention: EdgeFeaturedAttention,

    /// Graph RoPE for positional encoding
    rope: GraphRoPE,

    /// Dual-space attention for graph-latent fusion
    dual_attention: DualSpaceAttention,

    /// HNSW index for latent space search
    hnsw: Box<dyn HNSWInterface>,
}

impl GraphAttentionPipeline {
    pub fn forward(
        &mut self,
        node_features: ArrayView2<f32>,
        graph_adjacency: &HashMap<usize, Vec<usize>>,
        edge_features: &HashMap<(usize, usize), Array1<f32>>,
        layer: usize,
    ) -> Array2<f32> {
        let num_nodes = node_features.nrows();

        // 1. Apply EdgeFeaturedAttention for local context
        let local_context = self.edge_attention.forward(
            node_features,
            graph_adjacency,
            edge_features,
        );

        // 2. Compute distances for RoPE
        let query_id = 0;  // Example: use node 0 as reference
        let distances: Vec<f32> = (0..num_nodes)
            .map(|node_id| {
                self.hnsw.distance(query_id, node_id)
            })
            .collect();

        let layers: Vec<usize> = (0..num_nodes)
            .map(|node_id| self.hnsw.get_node_layer(node_id))
            .collect();

        // 3. Apply GraphRoPE positional encoding
        let positioned = self.rope.apply_rotation(
            local_context.view(),
            &distances,
            &layers,
        );

        // 4. Apply DualSpaceAttention for graph-latent fusion
        let output = self.dual_attention.forward_batch(
            positioned.view(),
            graph_adjacency,
            self.hnsw.as_ref(),
            layer,
        );

        output
    }
}
```

---

## Performance Considerations

### Memory Efficiency
- **Sparse Attention**: Only attend to k-nearest neighbors
- **Layer-wise Processing**: Process HNSW layers incrementally
- **Batch Operations**: Vectorize attention computation

### Computational Complexity
- **EdgeFeaturedAttention**: O(|E| × d²) where |E| is number of edges
- **GraphRoPE**: O(n × d²) where n is number of nodes
- **DualSpaceAttention**: O(n × (k_graph + k_latent) × d²)

### HNSW Integration Benefits
1. **Fast Neighbor Search**: O(log n) latent neighbor retrieval
2. **Multi-Scale Structure**: Layer-aware attention at different resolutions
3. **Distance Metrics**: Pre-computed distances for efficient RoPE
4. **Dynamic Updates**: Add new nodes without full retraining

---

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_featured_attention() {
        let attention = EdgeFeaturedAttention::new(64, 4, 0.2);
        // Test with synthetic graph
    }

    #[test]
    fn test_graph_rope() {
        let rope = GraphRoPE::new(64, 10000.0, 10.0, 4);
        // Test rotation properties
    }

    #[test]
    fn test_dual_space_attention() {
        let dual = DualSpaceAttention::new(64, 8);
        // Test with mock HNSW
    }
}
```

### Integration Tests
- Test with real HNSW indices
- Validate attention distributions
- Benchmark search performance
- Verify gradient flow

---

## Next Steps

1. **Implement HNSW Interface**: Create concrete implementation or adapter
2. **Gradient Computation**: Add backward pass for training
3. **Multi-Head Attention**: Extend to multi-head versions
4. **Layer Normalization**: Add normalization for stable training
5. **Benchmarking**: Compare with baseline GNN attention mechanisms

## References

- GAT (Graph Attention Networks): Veličković et al., 2018
- RoPE (Rotary Position Embeddings): Su et al., 2021
- HNSW (Hierarchical Navigable Small World): Malkov & Yashunin, 2018
- Cross-Attention Mechanisms: Vaswani et al., 2017 (Transformers)
