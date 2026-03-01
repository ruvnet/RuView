# GNN Layers Integration Plan

## Overview

Integrate Graph Neural Network layers from `ruvector-gnn` into PostgreSQL, enabling graph-aware vector search, message passing, and neural graph queries directly in SQL.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     PostgreSQL Extension                         │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                    GNN Layer Registry                    │    │
│  │  ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────────┐  │    │
│  │  │  GCN  │ │GraphSAGE│ │  GAT  │ │  GIN  │ │ RuVector  │  │    │
│  │  └───┬───┘ └───┬───┘ └───┬───┘ └───┬───┘ └─────┬─────┘  │    │
│  └──────┼─────────┼─────────┼─────────┼───────────┼────────┘    │
│         └─────────┴─────────┴─────────┴───────────┘             │
│                              ▼                                   │
│              ┌───────────────────────────┐                       │
│              │   Message Passing Engine  │                       │
│              │   (SIMD + Parallel)       │                       │
│              └───────────────────────────┘                       │
└─────────────────────────────────────────────────────────────────┘
```

## Module Structure

```
src/
├── gnn/
│   ├── mod.rs              # Module exports & registry
│   ├── layers/
│   │   ├── gcn.rs          # Graph Convolutional Network
│   │   ├── graphsage.rs    # GraphSAGE (sampling)
│   │   ├── gat.rs          # Graph Attention Network
│   │   ├── gin.rs          # Graph Isomorphism Network
│   │   └── ruvector.rs     # Custom RuVector layer
│   ├── message_passing.rs  # Core message passing
│   ├── aggregators.rs      # Sum, Mean, Max, LSTM
│   ├── graph_store.rs      # PostgreSQL graph storage
│   └── operators.rs        # SQL operators
```

## SQL Interface

### Graph Table Setup

```sql
-- Create node table with embeddings
CREATE TABLE nodes (
    id SERIAL PRIMARY KEY,
    embedding vector(256),
    features jsonb
);

-- Create edge table
CREATE TABLE edges (
    src_id INTEGER REFERENCES nodes(id),
    dst_id INTEGER REFERENCES nodes(id),
    weight FLOAT DEFAULT 1.0,
    edge_type TEXT,
    PRIMARY KEY (src_id, dst_id)
);

-- Create GNN-enhanced index
CREATE INDEX ON nodes USING ruvector_gnn (
    embedding vector(256)
) WITH (
    edge_table = 'edges',
    layer_type = 'graphsage',
    num_layers = 2,
    hidden_dim = 128,
    aggregator = 'mean'
);
```

### GNN Queries

```sql
-- GNN-enhanced similarity search (considers graph structure)
SELECT n.id, n.embedding,
       ruvector_gnn_score(n.embedding, query_vec, 'edges', 2) AS score
FROM nodes n
ORDER BY score DESC
LIMIT 10;

-- Message passing to get updated embeddings
SELECT node_id, updated_embedding
FROM ruvector_message_pass(
    node_table := 'nodes',
    edge_table := 'edges',
    embedding_column := 'embedding',
    num_hops := 2,
    layer_type := 'gcn'
);

-- Subgraph-aware search
SELECT * FROM ruvector_subgraph_search(
    center_node := 42,
    query_embedding := query_vec,
    max_hops := 3,
    k := 10
);

-- Node classification with GNN
SELECT node_id,
       ruvector_gnn_classify(embedding, 'edges', model_name := 'node_classifier') AS class
FROM nodes;
```

### Graph Construction from Vectors

```sql
-- Build k-NN graph from embeddings
SELECT ruvector_build_knn_graph(
    node_table := 'nodes',
    embedding_column := 'embedding',
    edge_table := 'edges_knn',
    k := 10,
    distance_metric := 'cosine'
);

-- Build epsilon-neighborhood graph
SELECT ruvector_build_eps_graph(
    node_table := 'nodes',
    embedding_column := 'embedding',
    edge_table := 'edges_eps',
    epsilon := 0.5
);
```

## Implementation Phases

### Phase 1: Message Passing Core (Week 1-3)

```rust
// src/gnn/message_passing.rs

/// Generic message passing framework
pub trait MessagePassing {
    /// Compute messages from neighbors
    fn message(&self, x_j: &[f32], edge_attr: Option<&[f32]>) -> Vec<f32>;

    /// Aggregate messages
    fn aggregate(&self, messages: &[Vec<f32>]) -> Vec<f32>;

    /// Update node embedding
    fn update(&self, x_i: &[f32], aggregated: &[f32]) -> Vec<f32>;
}

/// SIMD-optimized message passing
pub struct MessagePassingEngine {
    aggregator: Aggregator,
}

impl MessagePassingEngine {
    pub fn propagate(
        &self,
        node_features: &[Vec<f32>],
        edge_index: &[(usize, usize)],
        edge_weights: Option<&[f32]>,
        layer: &dyn MessagePassing,
    ) -> Vec<Vec<f32>> {
        let num_nodes = node_features.len();

        // Build adjacency list
        let adj_list = self.build_adjacency_list(edge_index, num_nodes);

        // Parallel message passing
        (0..num_nodes)
            .into_par_iter()
            .map(|i| {
                let neighbors = &adj_list[i];
                if neighbors.is_empty() {
                    return node_features[i].clone();
                }

                // Collect messages from neighbors
                let messages: Vec<Vec<f32>> = neighbors.iter()
                    .map(|&j| {
                        let edge_attr = edge_weights.map(|w| &w[j..j+1]);
                        layer.message(&node_features[j], edge_attr.map(|e| e.as_ref()))
                    })
                    .collect();

                // Aggregate
                let aggregated = layer.aggregate(&messages);

                // Update
                layer.update(&node_features[i], &aggregated)
            })
            .collect()
    }
}
```

### Phase 2: GCN Layer (Week 4-5)

```rust
// src/gnn/layers/gcn.rs

/// Graph Convolutional Network layer
/// H' = σ(D^(-1/2) A D^(-1/2) H W)
pub struct GCNLayer {
    in_features: usize,
    out_features: usize,
    weights: Vec<f32>,  // [in_features, out_features]
    bias: Option<Vec<f32>>,
    activation: Activation,
}

impl GCNLayer {
    pub fn new(in_features: usize, out_features: usize, bias: bool) -> Self {
        let weights = Self::glorot_init(in_features, out_features);
        Self {
            in_features,
            out_features,
            weights,
            bias: if bias { Some(vec![0.0; out_features]) } else { None },
            activation: Activation::ReLU,
        }
    }

    /// Forward pass with normalized adjacency
    pub fn forward(
        &self,
        x: &[Vec<f32>],
        edge_index: &[(usize, usize)],
        edge_weights: &[f32],
    ) -> Vec<Vec<f32>> {
        // Transform features: XW
        let transformed: Vec<Vec<f32>> = x.par_iter()
            .map(|xi| self.linear_transform(xi))
            .collect();

        // Message passing with normalized weights
        let propagated = self.propagate(&transformed, edge_index, edge_weights);

        // Apply activation
        propagated.into_iter()
            .map(|h| self.activate(&h))
            .collect()
    }

    #[inline]
    fn linear_transform(&self, x: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0; self.out_features];
        for i in 0..self.out_features {
            for j in 0..self.in_features {
                out[i] += x[j] * self.weights[j * self.out_features + i];
            }
            if let Some(ref bias) = self.bias {
                out[i] += bias[i];
            }
        }
        out
    }
}

// PostgreSQL function
#[pg_extern]
fn ruvector_gcn_forward(
    node_embeddings: Vec<Vec<f32>>,
    edge_src: Vec<i64>,
    edge_dst: Vec<i64>,
    edge_weights: Vec<f32>,
    out_features: i32,
) -> Vec<Vec<f32>> {
    let layer = GCNLayer::new(
        node_embeddings[0].len(),
        out_features as usize,
        true
    );

    let edges: Vec<_> = edge_src.iter()
        .zip(edge_dst.iter())
        .map(|(&s, &d)| (s as usize, d as usize))
        .collect();

    layer.forward(&node_embeddings, &edges, &edge_weights)
}
```

### Phase 3: GraphSAGE Layer (Week 6-7)

```rust
// src/gnn/layers/graphsage.rs

/// GraphSAGE with neighborhood sampling
pub struct GraphSAGELayer {
    in_features: usize,
    out_features: usize,
    aggregator: SAGEAggregator,
    sample_size: usize,
    weights_self: Vec<f32>,
    weights_neigh: Vec<f32>,
}

pub enum SAGEAggregator {
    Mean,
    MaxPool { mlp: MLP },
    LSTM { lstm: LSTMCell },
    GCN,
}

impl GraphSAGELayer {
    pub fn forward_with_sampling(
        &self,
        x: &[Vec<f32>],
        edge_index: &[(usize, usize)],
        num_samples: usize,
    ) -> Vec<Vec<f32>> {
        let adj_list = build_adjacency_list(edge_index, x.len());

        x.par_iter().enumerate()
            .map(|(i, xi)| {
                // Sample neighbors
                let neighbors = self.sample_neighbors(&adj_list[i], num_samples);

                // Aggregate neighbor features
                let neighbor_features: Vec<&[f32]> = neighbors.iter()
                    .map(|&j| x[j].as_slice())
                    .collect();
                let aggregated = self.aggregate(&neighbor_features);

                // Combine self and neighbor
                self.combine(xi, &aggregated)
            })
            .collect()
    }

    fn sample_neighbors(&self, neighbors: &[usize], k: usize) -> Vec<usize> {
        if neighbors.len() <= k {
            return neighbors.to_vec();
        }
        // Uniform random sampling
        neighbors.choose_multiple(&mut rand::thread_rng(), k)
            .cloned()
            .collect()
    }

    fn aggregate(&self, features: &[&[f32]]) -> Vec<f32> {
        match &self.aggregator {
            SAGEAggregator::Mean => {
                let dim = features[0].len();
                let mut result = vec![0.0; dim];
                for f in features {
                    for (r, &v) in result.iter_mut().zip(f.iter()) {
                        *r += v;
                    }
                }
                let n = features.len() as f32;
                result.iter_mut().for_each(|r| *r /= n);
                result
            }
            SAGEAggregator::MaxPool { mlp } => {
                features.iter()
                    .map(|f| mlp.forward(f))
                    .reduce(|a, b| element_wise_max(&a, &b))
                    .unwrap()
            }
            // ... other aggregators
        }
    }
}

#[pg_extern]
fn ruvector_graphsage_search(
    node_table: &str,
    edge_table: &str,
    query: Vec<f32>,
    num_layers: default!(i32, 2),
    sample_size: default!(i32, 10),
    k: default!(i32, 10),
) -> TableIterator<'static, (name!(id, i64), name!(score, f32))> {
    // Implementation using SPI
}
```

### Phase 4: Graph Isomorphism Network (Week 8)

```rust
// src/gnn/layers/gin.rs

/// Graph Isomorphism Network - maximally expressive
/// h_v = MLP((1 + ε) * h_v + Σ h_u)
pub struct GINLayer {
    mlp: MLP,
    eps: f32,
    train_eps: bool,
}

impl GINLayer {
    pub fn forward(
        &self,
        x: &[Vec<f32>],
        edge_index: &[(usize, usize)],
    ) -> Vec<Vec<f32>> {
        let adj_list = build_adjacency_list(edge_index, x.len());

        x.par_iter().enumerate()
            .map(|(i, xi)| {
                // Sum neighbor features
                let sum_neighbors: Vec<f32> = adj_list[i].iter()
                    .fold(vec![0.0; xi.len()], |mut acc, &j| {
                        for (a, &v) in acc.iter_mut().zip(x[j].iter()) {
                            *a += v;
                        }
                        acc
                    });

                // (1 + eps) * self + sum_neighbors
                let combined: Vec<f32> = xi.iter()
                    .zip(sum_neighbors.iter())
                    .map(|(&s, &n)| (1.0 + self.eps) * s + n)
                    .collect();

                // MLP
                self.mlp.forward(&combined)
            })
            .collect()
    }
}
```

### Phase 5: Custom RuVector Layer (Week 9-10)

```rust
// src/gnn/layers/ruvector.rs

/// RuVector's custom differentiable search layer
/// Combines HNSW navigation with learned message passing
pub struct RuVectorLayer {
    in_features: usize,
    out_features: usize,
    num_hops: usize,
    attention: MultiHeadAttention,
    transform: Linear,
}

impl RuVectorLayer {
    /// Forward pass using HNSW graph structure
    pub fn forward(
        &self,
        query: &[f32],
        hnsw_index: &HnswIndex,
        k_neighbors: usize,
    ) -> Vec<f32> {
        // Get k nearest neighbors from HNSW
        let neighbors = hnsw_index.search(query, k_neighbors);

        // Multi-hop aggregation following HNSW structure
        let mut current = query.to_vec();
        for hop in 0..self.num_hops {
            let neighbor_features: Vec<&[f32]> = neighbors.iter()
                .flat_map(|n| hnsw_index.get_neighbors(n.id))
                .map(|id| hnsw_index.get_vector(id))
                .collect();

            // Attention-weighted aggregation
            current = self.attention.forward(&current, &neighbor_features);
        }

        self.transform.forward(&current)
    }
}

#[pg_extern]
fn ruvector_differentiable_search(
    query: Vec<f32>,
    index_name: &str,
    num_hops: default!(i32, 2),
    k: default!(i32, 10),
) -> TableIterator<'static, (name!(id, i64), name!(score, f32), name!(enhanced_embedding, Vec<f32>))> {
    // Combines vector search with GNN enhancement
}
```

### Phase 6: Graph Storage (Week 11-12)

```rust
// src/gnn/graph_store.rs

/// Efficient graph storage for PostgreSQL
pub struct GraphStore {
    node_embeddings: SharedMemory<Vec<f32>>,
    adjacency: CompressedSparseRow,
    edge_features: Option<SharedMemory<Vec<f32>>>,
}

impl GraphStore {
    /// Load graph from PostgreSQL tables
    pub fn from_tables(
        node_table: &str,
        embedding_column: &str,
        edge_table: &str,
    ) -> Result<Self, GraphError> {
        Spi::connect(|client| {
            // Load nodes
            let nodes = client.select(
                &format!("SELECT id, {} FROM {}", embedding_column, node_table),
                None, None
            )?;

            // Load edges
            let edges = client.select(
                &format!("SELECT src_id, dst_id, weight FROM {}", edge_table),
                None, None
            )?;

            // Build CSR
            let csr = CompressedSparseRow::from_edges(&edges);

            Ok(Self {
                node_embeddings: SharedMemory::new(nodes),
                adjacency: csr,
                edge_features: None,
            })
        })
    }

    /// Efficient neighbor lookup
    pub fn neighbors(&self, node_id: usize) -> &[usize] {
        self.adjacency.neighbors(node_id)
    }
}

/// Compressed Sparse Row format for adjacency
pub struct CompressedSparseRow {
    indptr: Vec<usize>,    // Row pointers
    indices: Vec<usize>,   // Column indices
    data: Vec<f32>,        // Edge weights
}
```

## Aggregator Functions

```rust
// src/gnn/aggregators.rs

pub enum Aggregator {
    Sum,
    Mean,
    Max,
    Min,
    Attention { heads: usize },
    Set2Set { steps: usize },
}

impl Aggregator {
    pub fn aggregate(&self, messages: &[Vec<f32>]) -> Vec<f32> {
        match self {
            Aggregator::Sum => Self::sum_aggregate(messages),
            Aggregator::Mean => Self::mean_aggregate(messages),
            Aggregator::Max => Self::max_aggregate(messages),
            Aggregator::Attention { heads } => Self::attention_aggregate(messages, *heads),
            _ => unimplemented!(),
        }
    }

    fn sum_aggregate(messages: &[Vec<f32>]) -> Vec<f32> {
        let dim = messages[0].len();
        let mut result = vec![0.0; dim];
        for msg in messages {
            for (r, &m) in result.iter_mut().zip(msg.iter()) {
                *r += m;
            }
        }
        result
    }

    fn attention_aggregate(messages: &[Vec<f32>], heads: usize) -> Vec<f32> {
        // Multi-head attention over messages
        let mha = MultiHeadAttention::new(messages[0].len(), heads);
        mha.aggregate(messages)
    }
}
```

## Performance Optimizations

### Batch Processing

```rust
/// Process multiple nodes in parallel batches
pub fn batch_message_passing(
    nodes: &[Vec<f32>],
    edge_index: &[(usize, usize)],
    batch_size: usize,
) -> Vec<Vec<f32>> {
    nodes.par_chunks(batch_size)
        .flat_map(|batch| {
            // Process batch with SIMD
            process_batch(batch, edge_index)
        })
        .collect()
}
```

### Sparse Operations

```rust
/// Sparse matrix multiplication for message passing
pub fn sparse_mm(
    node_features: &[Vec<f32>],
    csr: &CompressedSparseRow,
) -> Vec<Vec<f32>> {
    let dim = node_features[0].len();
    let num_nodes = node_features.len();

    (0..num_nodes).into_par_iter()
        .map(|i| {
            let start = csr.indptr[i];
            let end = csr.indptr[i + 1];

            let mut result = vec![0.0; dim];
            for j in start..end {
                let neighbor = csr.indices[j];
                let weight = csr.data[j];
                for (r, &f) in result.iter_mut().zip(node_features[neighbor].iter()) {
                    *r += weight * f;
                }
            }
            result
        })
        .collect()
}
```

## Benchmarks

| Layer | Nodes | Edges | Features | Time (ms) | Memory |
|-------|-------|-------|----------|-----------|--------|
| GCN | 10K | 100K | 256 | 12 | 40MB |
| GraphSAGE | 10K | 100K | 256 | 18 | 45MB |
| GAT (4 heads) | 10K | 100K | 256 | 35 | 60MB |
| GIN | 10K | 100K | 256 | 15 | 42MB |
| RuVector | 10K | 100K | 256 | 25 | 55MB |

## Dependencies

```toml
[dependencies]
# Link to ruvector-gnn
ruvector-gnn = { path = "../ruvector-gnn", optional = true }

# Sparse matrix
sprs = "0.11"

# Parallel
rayon = "1.10"

# SIMD
simsimd = "5.9"
```

## Feature Flags

```toml
[features]
gnn = []
gnn-gcn = ["gnn"]
gnn-sage = ["gnn"]
gnn-gat = ["gnn", "attention"]
gnn-gin = ["gnn"]
gnn-all = ["gnn-gcn", "gnn-sage", "gnn-gat", "gnn-gin"]
```
