# GNN-Guided HNSW Routing - Implementation Plan

## Overview

### Problem Statement

Current HNSW (Hierarchical Navigable Small World) graph search uses a greedy routing strategy that selects the nearest neighbor at each step. This approach is locally optimal but often misses globally better paths, resulting in:

- Suboptimal query performance (increased distance computations)
- Redundant edge traversals in dense regions
- Poor scaling with graph size (20-40% performance degradation at 10M+ vectors)
- Inability to learn from query patterns

### Proposed Solution

Replace greedy HNSW routing with a learned GNN-based routing policy that:

1. **Path Learning**: Train on successful search trajectories to learn optimal routing decisions
2. **Context-Aware Selection**: Use graph structure + query context to predict best next hops
3. **Multi-Hop Reasoning**: Consider k-step lookahead instead of greedy single-step
4. **Adaptive Routing**: Adjust routing strategy based on query characteristics

The GNN will output edge selection probabilities for each node during search, replacing the greedy nearest-neighbor heuristic.

### Expected Benefits

**Quantified Performance Improvements:**
- **+25% QPS** (Queries Per Second) through reduced search iterations
- **-30% distance computations** via smarter edge selection
- **-15% average hop count** to reach target nodes
- **+18% recall@10** for challenging queries (edge cases, dense clusters)

**Qualitative Benefits:**
- Learns from query distribution patterns
- Adapts to graph topology changes
- Handles multi-modal embeddings better
- Reduces tail latencies (P99 improvement)

## Technical Design

### Architecture Diagram (ASCII Art)

```
┌─────────────────────────────────────────────────────────────────┐
│                     GNN-Guided HNSW Search                      │
└─────────────────────────────────────────────────────────────────┘

Query Vector (q)
     │
     ▼
┌─────────────────────────────────────────────────────────────────┐
│  Entry Point Selection (standard HNSW top layer)                │
└─────────────────────────────────────────────────────────────────┘
     │
     ▼
┌─────────────────────────────────────────────────────────────────┐
│  Layer L → 0 Search Loop                                        │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  Current Node (c)                                         │  │
│  │         │                                                 │  │
│  │         ▼                                                 │  │
│  │  ┌──────────────────────────────────────────────┐        │  │
│  │  │  GNN Edge Scorer                             │        │  │
│  │  │  ┌────────────────────────────────────────┐  │        │  │
│  │  │  │ Input: [node_feat, query, edge_feat]  │  │        │  │
│  │  │  │ Graph Context: k-hop neighborhood     │  │        │  │
│  │  │  │ Attention Layer: Multi-head GAT       │  │        │  │
│  │  │  │ Output: Edge selection probabilities  │  │        │  │
│  │  │  └────────────────────────────────────────┘  │        │  │
│  │  └──────────────────────────────────────────────┘        │  │
│  │         │                                                 │  │
│  │         ▼                                                 │  │
│  │  ┌──────────────────────────────────────────────┐        │  │
│  │  │  Edge Selection Strategy                     │        │  │
│  │  │  - Top-k by GNN score                        │        │  │
│  │  │  - Temperature-based sampling (exploration)  │        │  │
│  │  │  - Hybrid: GNN score * distance heuristic    │        │  │
│  │  └──────────────────────────────────────────────┘        │  │
│  │         │                                                 │  │
│  │         ▼                                                 │  │
│  │  Candidate Neighbors (N)                                 │  │
│  │         │                                                 │  │
│  │         ▼                                                 │  │
│  │  Update best candidates, move to next node              │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
     │
     ▼
Return Top-K Results

┌─────────────────────────────────────────────────────────────────┐
│                     Training Pipeline                            │
└─────────────────────────────────────────────────────────────────┘

Query Workload
     │
     ▼
┌─────────────────────────────────────────────────────────────────┐
│  Path Collector (on standard HNSW)                              │
│  - Record: (query, node_seq, edges_taken, final_results)        │
│  - Label: edges_on_optimal_path = 1, others = 0                 │
└─────────────────────────────────────────────────────────────────┘
     │
     ▼
┌─────────────────────────────────────────────────────────────────┐
│  Offline Training (PyTorch/candle)                              │
│  - Loss: BCE(GNN_edge_score, optimal_edge_label)                │
│  - Optimizer: AdamW with lr=1e-3                                │
│  - Batch: 256 query trajectories                                │
└─────────────────────────────────────────────────────────────────┘
     │
     ▼
Export ONNX Model → Load in ruvector-gnn (Rust)
```

### Core Data Structures (Rust)

```rust
// File: crates/ruvector-gnn/src/routing/mod.rs

use ndarray::{Array1, Array2};
use ort::{Session, Value};

/// GNN-guided routing policy for HNSW search
pub struct GnnRoutingPolicy {
    /// ONNX runtime session for GNN inference
    session: Session,

    /// Feature extractor for nodes and edges
    feature_extractor: FeatureExtractor,

    /// Configuration for routing behavior
    config: RoutingConfig,

    /// Performance metrics
    metrics: RoutingMetrics,
}

/// Configuration for GNN routing
#[derive(Debug, Clone)]
pub struct RoutingConfig {
    /// Number of top edges to consider per node
    pub top_k_edges: usize,

    /// Temperature for edge selection sampling (0.0 = greedy)
    pub temperature: f32,

    /// Hybrid weight: α * gnn_score + (1-α) * distance_score
    pub hybrid_alpha: f32,

    /// Maximum GNN inference batch size
    pub inference_batch_size: usize,

    /// Enable/disable GNN routing (fallback to greedy)
    pub enabled: bool,

    /// K-hop neighborhood size for graph context
    pub context_hops: usize,
}

/// Feature extraction for nodes and edges
pub struct FeatureExtractor {
    /// Dimensionality of node features
    node_dim: usize,

    /// Dimensionality of edge features
    edge_dim: usize,

    /// Cache for computed features
    cache: FeatureCache,
}

/// Features for a single node in the graph
#[derive(Debug, Clone)]
pub struct NodeFeatures {
    /// Node embedding vector
    pub embedding: Array1<f32>,

    /// Degree (number of neighbors)
    pub degree: usize,

    /// Layer in HNSW hierarchy
    pub layer: usize,

    /// Clustering coefficient
    pub clustering_coef: f32,

    /// Distance to query (dynamic)
    pub query_distance: f32,
}

/// Features for an edge in the graph
#[derive(Debug, Clone)]
pub struct EdgeFeatures {
    /// Euclidean distance between connected nodes
    pub distance: f32,

    /// Angular similarity (cosine)
    pub angular_similarity: f32,

    /// Edge betweenness (precomputed)
    pub betweenness: f32,

    /// Whether edge crosses layers
    pub cross_layer: bool,
}

/// GNN inference result for edge selection
#[derive(Debug)]
pub struct EdgeScore {
    /// Target node ID
    pub target_node: u32,

    /// GNN-predicted score [0, 1]
    pub gnn_score: f32,

    /// Distance-based heuristic score
    pub distance_score: f32,

    /// Final combined score
    pub combined_score: f32,
}

/// Performance tracking for routing
#[derive(Debug, Default)]
pub struct RoutingMetrics {
    /// Total GNN inference calls
    pub total_inferences: u64,

    /// Average inference latency (microseconds)
    pub avg_inference_us: f64,

    /// Total distance computations
    pub distance_computations: u64,

    /// Average hops per query
    pub avg_hops: f64,

    /// Cache hit rate for features
    pub feature_cache_hit_rate: f64,
}

/// Training data collection for offline learning
pub struct PathTrajectory {
    /// Query vector
    pub query: Vec<f32>,

    /// Sequence of nodes visited
    pub node_sequence: Vec<u32>,

    /// Edges taken at each step
    pub edges_taken: Vec<(u32, u32)>,

    /// All candidate edges at each step
    pub candidate_edges: Vec<Vec<(u32, u32)>>,

    /// Final k-NN results
    pub results: Vec<(u32, f32)>,
}
```

### Key Algorithms (Pseudocode)

#### 1. GNN-Guided Search Algorithm

```python
function gnn_guided_search(query: Vector, graph: HNSWGraph, k: int) -> List[Result]:
    """
    HNSW search with GNN-guided routing instead of greedy selection.
    """
    # Initialize from top layer entry point
    current_nodes = {graph.entry_point}
    layer = graph.max_layer

    # Descend through layers
    while layer >= 0:
        # Find best candidates at this layer using GNN
        candidates = priority_queue()
        visited = set()

        for node in current_nodes:
            # Get neighbors at this layer
            neighbors = graph.get_neighbors(node, layer)

            # Extract features for GNN
            node_features = extract_node_features(node, query, graph)
            edge_features = [extract_edge_features(node, neighbor, graph)
                           for neighbor in neighbors]

            # GNN inference: score all edges from current node
            edge_scores = gnn_model.score_edges(
                node_features,
                edge_features,
                query
            )

            # Select edges based on GNN scores (not greedy distance)
            selected = select_edges_by_gnn_score(
                neighbors,
                edge_scores,
                config.top_k_edges,
                config.temperature
            )

            for neighbor in selected:
                if neighbor not in visited:
                    distance = compute_distance(query, graph.get_vector(neighbor))
                    candidates.push(neighbor, distance)
                    visited.add(neighbor)

        # Move to best candidates for next iteration
        current_nodes = candidates.top(config.beam_width)
        layer -= 1

    # Return top-k results from layer 0
    return candidates.top(k)


function select_edges_by_gnn_score(neighbors, scores, top_k, temperature):
    """
    Select edges based on GNN scores with optional exploration.

    Strategies:
    - temperature = 0: greedy top-k
    - temperature > 0: sampling from softmax distribution
    - hybrid mode: combine GNN score with distance heuristic
    """
    if temperature == 0:
        # Greedy: select top-k by GNN score
        return top_k_by_score(neighbors, scores)
    else:
        # Sampling: use temperature-scaled softmax
        probs = softmax(scores / temperature)
        return sample_without_replacement(neighbors, probs, top_k)


function extract_node_features(node, query, graph):
    """
    Extract node-level features for GNN input.
    """
    return NodeFeatures(
        embedding=graph.get_vector(node),
        degree=graph.get_degree(node),
        layer=graph.get_layer(node),
        clustering_coef=graph.get_clustering_coefficient(node),
        query_distance=distance(query, graph.get_vector(node))
    )


function extract_edge_features(source, target, graph):
    """
    Extract edge-level features for GNN input.
    """
    source_vec = graph.get_vector(source)
    target_vec = graph.get_vector(target)

    return EdgeFeatures(
        distance=euclidean_distance(source_vec, target_vec),
        angular_similarity=cosine_similarity(source_vec, target_vec),
        betweenness=graph.get_edge_betweenness(source, target),
        cross_layer=(graph.get_layer(source) != graph.get_layer(target))
    )
```

#### 2. Offline Training Pipeline

```python
function collect_training_data(graph, query_workload, n_samples):
    """
    Collect path trajectories from standard HNSW for training.
    """
    trajectories = []

    for query in query_workload.sample(n_samples):
        # Run standard greedy HNSW search with full logging
        path = instrumented_hnsw_search(query, graph)

        # Label edges: 1 if on optimal path, 0 otherwise
        optimal_edges = set(path.edges_taken)

        # For each node in path, get all candidate edges
        for step in path.node_sequence:
            node = step.node
            neighbors = graph.get_neighbors(node, step.layer)

            # Create training examples
            for neighbor in neighbors:
                edge = (node, neighbor)
                label = 1.0 if edge in optimal_edges else 0.0

                node_feat = extract_node_features(node, query, graph)
                edge_feat = extract_edge_features(node, neighbor, graph)

                trajectories.append({
                    'node_features': node_feat,
                    'edge_features': edge_feat,
                    'query': query,
                    'label': label,
                    'distance_to_query': distance(query, graph.get_vector(neighbor))
                })

    return trajectories


function train_gnn_routing_model(trajectories, config):
    """
    Train GNN model to predict edge selection probabilities.

    Architecture: Graph Attention Network (GAT)
    - 3 attention layers with 4 heads each
    - Hidden dim: 128
    - Edge features concatenated with node features
    - Output: single logit per edge (probability of selection)
    """
    model = GAT(
        node_dim=config.node_feature_dim,
        edge_dim=config.edge_feature_dim,
        hidden_dim=128,
        num_layers=3,
        num_heads=4,
        output_dim=1
    )

    optimizer = AdamW(model.parameters(), lr=1e-3, weight_decay=1e-4)
    loss_fn = BCEWithLogitsLoss()

    for epoch in range(config.num_epochs):
        for batch in DataLoader(trajectories, batch_size=256):
            # Forward pass
            edge_logits = model(
                batch.node_features,
                batch.edge_features,
                batch.query
            )

            # Binary cross-entropy loss
            loss = loss_fn(edge_logits, batch.labels)

            # Add distance-aware regularization
            # Encourage model to respect distance heuristic
            distance_scores = 1.0 / (1.0 + batch.distance_to_query)
            consistency_loss = mse_loss(sigmoid(edge_logits), distance_scores)

            total_loss = loss + 0.1 * consistency_loss

            # Backward pass
            optimizer.zero_grad()
            total_loss.backward()
            optimizer.step()

    return model


function export_to_onnx(model, output_path):
    """
    Export trained PyTorch model to ONNX for Rust inference.
    """
    dummy_input = {
        'node_features': torch.randn(1, node_dim),
        'edge_features': torch.randn(10, edge_dim),  # up to 10 neighbors
        'query': torch.randn(1, embedding_dim)
    }

    torch.onnx.export(
        model,
        dummy_input,
        output_path,
        input_names=['node_features', 'edge_features', 'query'],
        output_names=['edge_scores'],
        dynamic_axes={
            'edge_features': {0: 'num_edges'},
            'edge_scores': {0: 'num_edges'}
        },
        opset_version=14
    )
```

### API Design (Function Signatures)

```rust
// File: crates/ruvector-gnn/src/routing/mod.rs

impl GnnRoutingPolicy {
    /// Create a new GNN routing policy from an ONNX model file
    pub fn from_onnx(
        model_path: impl AsRef<Path>,
        config: RoutingConfig,
    ) -> Result<Self, GnnError>;

    /// Score edges from a given node during HNSW search
    ///
    /// # Arguments
    /// * `current_node` - The node we're currently at
    /// * `candidate_neighbors` - Potential next hops
    /// * `query` - The query vector
    /// * `graph` - Reference to HNSW graph for feature extraction
    ///
    /// # Returns
    /// Vector of `EdgeScore` sorted by combined_score (descending)
    pub fn score_edges(
        &mut self,
        current_node: u32,
        candidate_neighbors: &[u32],
        query: &[f32],
        graph: &HnswGraph,
    ) -> Result<Vec<EdgeScore>, GnnError>;

    /// Select top-k edges based on GNN scores
    pub fn select_top_k(
        &self,
        edge_scores: &[EdgeScore],
        k: usize,
    ) -> Vec<u32>;

    /// Get current routing metrics
    pub fn metrics(&self) -> &RoutingMetrics;

    /// Reset metrics counters
    pub fn reset_metrics(&mut self);

    /// Update configuration at runtime
    pub fn update_config(&mut self, config: RoutingConfig);
}

impl FeatureExtractor {
    /// Create a new feature extractor
    pub fn new(node_dim: usize, edge_dim: usize) -> Self;

    /// Extract node features for GNN input
    pub fn extract_node_features(
        &self,
        node_id: u32,
        query: &[f32],
        graph: &HnswGraph,
    ) -> Result<NodeFeatures, GnnError>;

    /// Extract edge features for GNN input
    pub fn extract_edge_features(
        &self,
        source: u32,
        target: u32,
        graph: &HnswGraph,
    ) -> Result<EdgeFeatures, GnnError>;

    /// Batch extract features for multiple edges
    pub fn batch_extract_edge_features(
        &self,
        edges: &[(u32, u32)],
        graph: &HnswGraph,
    ) -> Result<Vec<EdgeFeatures>, GnnError>;

    /// Clear feature cache
    pub fn clear_cache(&mut self);
}

// Integration with existing HNSW implementation
// File: crates/ruvector-core/src/index/hnsw.rs

impl HnswIndex {
    /// Enable GNN-guided routing
    pub fn set_gnn_routing(
        &mut self,
        policy: GnnRoutingPolicy,
    ) -> Result<(), HnswError>;

    /// Disable GNN routing (fallback to greedy)
    pub fn disable_gnn_routing(&mut self);

    /// Get routing performance metrics
    pub fn routing_metrics(&self) -> Option<&RoutingMetrics>;
}

// Training utilities
// File: crates/ruvector-gnn/src/routing/training.rs

/// Collect path trajectories from HNSW search for training
pub fn collect_training_trajectories(
    graph: &HnswGraph,
    queries: &[Vec<f32>],
    output_path: impl AsRef<Path>,
) -> Result<usize, GnnError>;

/// Validate ONNX model compatibility
pub fn validate_onnx_model(
    model_path: impl AsRef<Path>,
) -> Result<ModelInfo, GnnError>;

#[derive(Debug)]
pub struct ModelInfo {
    pub input_dims: Vec<(String, Vec<i64>)>,
    pub output_dims: Vec<(String, Vec<i64>)>,
    pub opset_version: i64,
}
```

## Integration Points

### Affected Crates/Modules

1. **`ruvector-gnn`** (Primary)
   - New module: `src/routing/mod.rs` - GNN routing policy
   - New module: `src/routing/features.rs` - Feature extraction
   - New module: `src/routing/training.rs` - Training utilities
   - Modified: `src/lib.rs` - Export routing types

2. **`ruvector-core`** (Integration)
   - Modified: `src/index/hnsw.rs` - Integrate GNN routing into search
   - Modified: `src/index/mod.rs` - Add routing configuration
   - New: `src/index/hnsw_gnn.rs` - GNN-specific HNSW extensions

3. **`ruvector-api`** (Configuration)
   - Modified: `src/config.rs` - Add GNN routing config options
   - Modified: `src/index_manager.rs` - Support GNN model loading

4. **`ruvector-bindings`** (Exposure)
   - Modified: `python/src/lib.rs` - Expose routing config to Python
   - Modified: `nodejs/src/lib.rs` - Expose routing config to Node.js

### New Modules to Create

```
crates/ruvector-gnn/
├── src/
│   ├── routing/
│   │   ├── mod.rs              # Core routing policy
│   │   ├── features.rs         # Feature extraction
│   │   ├── training.rs         # Training data collection
│   │   ├── cache.rs            # Feature caching
│   │   └── metrics.rs          # Performance tracking
│   └── models/
│       └── routing_gnn.onnx    # Pre-trained model (optional)

examples/
├── gnn_routing/
│   ├── train_routing_model.py  # Python training script
│   ├── evaluate_routing.rs     # Rust evaluation benchmark
│   └── README.md               # Usage guide
```

### Dependencies on Other Features

**Independent** - Can be implemented standalone

**Synergies with:**
- **Incremental Graph Learning (Feature 2)**: Cached node features can be reused
- **Neuro-Symbolic Query (Feature 3)**: GNN routing can incorporate symbolic constraints
- **Existing Attention Mechanisms**: Reuse attention layers from Issue #38

**External Dependencies:**
- `ort` (ONNX Runtime) - Already in use for GNN inference
- `ndarray` - Already in use for tensor operations
- `parking_lot` - For feature cache concurrency

## Regression Prevention

### What Existing Functionality Could Break

1. **HNSW Search Correctness**
   - Risk: GNN routing might skip true nearest neighbors
   - Impact: Degraded recall, incorrect results

2. **Performance Degradation**
   - Risk: GNN inference overhead exceeds routing savings
   - Impact: Lower QPS than baseline greedy search

3. **Memory Usage**
   - Risk: Feature caching and GNN model consume excessive RAM
   - Impact: OOM on large graphs

4. **Thread Safety**
   - Risk: Feature cache race conditions in concurrent queries
   - Impact: Corrupted features, crashes

5. **Build/Deployment**
   - Risk: ONNX model path resolution failures
   - Impact: Runtime errors, inability to use feature

### Test Cases to Prevent Regressions

```rust
// File: crates/ruvector-gnn/tests/routing_regression_tests.rs

#[test]
fn test_gnn_routing_recall_matches_greedy() {
    // GNN routing must achieve ≥95% of greedy baseline recall
    let graph = build_test_hnsw(10_000, 512);
    let queries = generate_test_queries(1000);

    // Baseline: greedy search
    graph.disable_gnn_routing();
    let greedy_results = run_search_batch(&graph, &queries, k=10);

    // GNN routing
    graph.set_gnn_routing(load_test_model());
    let gnn_results = run_search_batch(&graph, &queries, k=10);

    let recall = compute_recall(&greedy_results, &gnn_results);
    assert!(recall >= 0.95, "GNN recall: {}, expected ≥0.95", recall);
}

#[test]
fn test_gnn_routing_performance_improvement() {
    // GNN routing must achieve ≥10% QPS improvement
    let graph = build_test_hnsw(100_000, 512);
    let queries = generate_test_queries(10_000);

    // Baseline
    graph.disable_gnn_routing();
    let greedy_qps = benchmark_qps(&graph, &queries);

    // GNN
    graph.set_gnn_routing(load_test_model());
    let gnn_qps = benchmark_qps(&graph, &queries);

    let improvement = (gnn_qps - greedy_qps) / greedy_qps;
    assert!(improvement >= 0.10, "QPS improvement: {:.2}%, expected ≥10%", improvement * 100.0);
}

#[test]
fn test_gnn_routing_distance_computation_reduction() {
    // Must reduce distance computations by ≥20%
    let graph = build_test_hnsw(50_000, 512);
    let queries = generate_test_queries(1000);

    graph.disable_gnn_routing();
    graph.reset_metrics();
    run_search_batch(&graph, &queries, k=10);
    let greedy_dists = graph.metrics().distance_computations;

    graph.set_gnn_routing(load_test_model());
    graph.reset_metrics();
    run_search_batch(&graph, &queries, k=10);
    let gnn_dists = graph.metrics().distance_computations;

    let reduction = (greedy_dists - gnn_dists) as f64 / greedy_dists as f64;
    assert!(reduction >= 0.20, "Distance reduction: {:.2}%, expected ≥20%", reduction * 100.0);
}

#[test]
fn test_feature_cache_thread_safety() {
    // Concurrent queries must not corrupt feature cache
    let graph = Arc::new(build_test_hnsw(10_000, 512));
    graph.set_gnn_routing(load_test_model());

    let handles: Vec<_> = (0..16)
        .map(|_| {
            let g = Arc::clone(&graph);
            thread::spawn(move || {
                let queries = generate_test_queries(100);
                run_search_batch(&g, &queries, k=10)
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    // All results should be valid (no panics/corruptions)
    for result_set in results {
        assert!(validate_results(&result_set));
    }
}

#[test]
fn test_graceful_fallback_on_gnn_error() {
    // If GNN fails, must fallback to greedy without crashing
    let graph = build_test_hnsw(1000, 512);

    // Inject faulty GNN model
    graph.set_gnn_routing(create_faulty_model());

    let queries = generate_test_queries(100);
    let results = run_search_batch(&graph, &queries, k=10);

    // Should get valid results (from fallback)
    assert_eq!(results.len(), 100);
    assert!(graph.routing_metrics().unwrap().fallback_count > 0);
}
```

### Backward Compatibility Strategy

1. **Default Disabled**
   - GNN routing is opt-in via configuration
   - Existing deployments unaffected unless explicitly enabled

2. **Configuration Migration**
   ```yaml
   # Old config (still works)
   hnsw:
     ef_construction: 200
     M: 16

   # New config (optional)
   hnsw:
     ef_construction: 200
     M: 16
     gnn_routing:
       enabled: false  # Default: disabled
       model_path: "./models/routing_gnn.onnx"
       top_k_edges: 5
       temperature: 0.0
       hybrid_alpha: 0.8
   ```

3. **Feature Flags**
   ```rust
   #[cfg(feature = "gnn-routing")]
   pub mod routing;
   ```
   - Can be compiled out if not needed
   - Reduces binary size and dependencies

4. **Versioned Model Format**
   - ONNX models include version metadata
   - Runtime checks for compatibility
   - Graceful degradation on version mismatch

## Implementation Phases

### Phase 1: Core Implementation (Week 1-2)

**Goal**: Working GNN routing with ONNX inference

**Tasks**:
1. Implement `FeatureExtractor` for nodes and edges
2. Implement `GnnRoutingPolicy` with ONNX runtime
3. Add basic edge scoring logic
4. Unit tests for feature extraction
5. Unit tests for ONNX inference

**Deliverables**:
- `ruvector-gnn/src/routing/mod.rs`
- `ruvector-gnn/src/routing/features.rs`
- Passing unit tests
- Example ONNX model (mock, not trained)

**Success Criteria**:
- GNN can score edges without crashing
- Feature extraction produces valid tensors
- ONNX model loads and runs inference

### Phase 2: Integration (Week 2-3)

**Goal**: Integrate GNN routing into HNSW search

**Tasks**:
1. Modify `HnswIndex` to support GNN routing
2. Implement routing selection strategies (greedy, sampling, hybrid)
3. Add performance metrics tracking
4. Add feature caching for performance
5. Integration tests with real HNSW graphs

**Deliverables**:
- Modified `ruvector-core/src/index/hnsw.rs`
- Working end-to-end search with GNN
- Performance benchmarks vs baseline
- Feature cache implementation

**Success Criteria**:
- GNN routing produces correct k-NN results
- No crashes or panics in concurrent scenarios
- Metrics collection working

### Phase 3: Optimization (Week 3-4)

**Goal**: Achieve +25% QPS, -30% distance computations

**Tasks**:
1. Profile GNN inference overhead
2. Optimize feature extraction (batching, caching)
3. Tune hybrid_alpha and temperature parameters
4. Implement batch inference for multiple edges
5. Add SIMD optimizations where applicable
6. Train actual GNN model on real query workload

**Deliverables**:
- Trained ONNX model with documented performance
- Python training script (`examples/gnn_routing/train_routing_model.py`)
- Performance tuning guide
- Optimized feature cache

**Success Criteria**:
- +25% QPS improvement on benchmark dataset
- -30% reduction in distance computations
- <2ms average GNN inference latency per query
- >80% feature cache hit rate

### Phase 4: Production Hardening (Week 4-5)

**Goal**: Production-ready feature with safety guarantees

**Tasks**:
1. Add comprehensive error handling
2. Implement graceful fallback to greedy on GNN errors
3. Add configuration validation
4. Write regression tests (prevent regressions)
5. Write documentation and examples
6. Add telemetry/observability hooks
7. Performance benchmarks on large-scale datasets (10M+ vectors)

**Deliverables**:
- Full regression test suite
- User documentation
- Performance benchmark report
- Example configurations
- Migration guide

**Success Criteria**:
- All regression tests passing
- Zero crashes in stress tests
- Documentation complete
- Ready for alpha release

## Success Metrics

### Performance Benchmarks

**Primary Metrics** (Must Achieve):

| Metric | Baseline (Greedy) | Target (GNN) | Measurement |
|--------|-------------------|--------------|-------------|
| QPS (1M vectors) | 10,000 | 12,500 (+25%) | queries/second @ 16 threads |
| Distance Computations | 150/query | 105/query (-30%) | average per query |
| Average Hops | 12.5 | 10.6 (-15%) | hops to reach target |
| P99 Latency | 15ms | 12ms (-20%) | 99th percentile query time |

**Secondary Metrics** (Nice to Have):

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| Feature Cache Hit Rate | N/A | >80% | cache hits / total accesses |
| GNN Inference Time | N/A | <2ms | average per query |
| Memory Overhead | N/A | <5% | additional RAM for GNN + cache |
| Recall@10 | 0.95 | 0.96 (+1pp) | fraction of true neighbors found |

### Accuracy Metrics

**Recall Preservation**:
- GNN routing must achieve ≥95% of greedy baseline recall
- No degradation on edge-case queries (dense clusters, outliers)

**Path Optimality**:
- GNN paths should be ≤5% longer than oracle optimal paths
- Measured by comparing against brute-force ground truth

**Failure Rate**:
- Graceful fallback to greedy on <1% of queries
- Zero crashes or incorrect results

### Memory/Latency Targets

**Memory**:
- GNN model size: <50MB (ONNX file)
- Feature cache: <100MB per 1M vectors
- Total overhead: <5% of base HNSW index size

**Latency**:
- GNN inference: <2ms average, <5ms P99
- Feature extraction: <0.5ms per node
- Total query latency: <15ms P99 (vs 15ms baseline)

**Throughput**:
- Concurrent queries: 16+ threads with linear scaling
- Batch inference: 10+ edges per batch for efficiency

## Risks and Mitigations

### Technical Risks

**Risk 1: GNN Inference Overhead Exceeds Routing Savings**

*Probability: Medium | Impact: High*

**Description**: If GNN model is too complex, inference time could negate benefits of reduced hops.

**Mitigation**:
- Profile GNN inference early in Phase 1
- Set hard latency budget (<2ms per query)
- Use lightweight GNN architecture (3-layer GAT, not deep networks)
- Batch inference across multiple edges
- Implement feature caching to avoid recomputation
- Add fallback to greedy if inference exceeds budget

**Contingency**: If overhead too high, switch to simpler models (MLP instead of GNN) or hybrid mode (GNN only for hard queries).

---

**Risk 2: Training Data Scarcity**

*Probability: Medium | Impact: Medium*

**Description**: May not have enough diverse queries to train robust GNN model.

**Mitigation**:
- Use query augmentation (add noise, rotations)
- Pretrain on synthetic queries (random vectors)
- Fine-tune on actual workload
- Support transfer learning from similar datasets
- Provide pre-trained baseline model

**Contingency**: Start with simple heuristic-based routing (e.g., distance + degree) and upgrade to GNN later.

---

**Risk 3: Model Generalization Failures**

*Probability: Low | Impact: High*

**Description**: GNN trained on one dataset might not generalize to different embedding distributions.

**Mitigation**:
- Train on diverse datasets (text, images, multi-modal)
- Use domain-agnostic features (degree, distance, structure)
- Add online learning to adapt to new query patterns
- Provide model retraining tools
- Extensive evaluation on held-out datasets

**Contingency**: Support per-index model training for critical use cases.

---

**Risk 4: Feature Cache Memory Bloat**

*Probability: Low | Impact: Medium*

**Description**: Caching node/edge features could consume excessive memory on large graphs.

**Mitigation**:
- Use LRU eviction policy (keep only recent features)
- Set cache size limits (e.g., max 100MB)
- Make caching optional (can disable for low-memory environments)
- Use compressed feature representations
- Profile memory usage in Phase 3

**Contingency**: Disable feature caching by default, enable only for latency-critical workloads.

---

**Risk 5: ONNX Compatibility Issues**

*Probability: Low | Impact: Medium*

**Description**: ONNX runtime might not support specific GNN operations or have platform issues.

**Mitigation**:
- Use only standard ONNX ops (opset 14+)
- Test on multiple platforms (Linux, macOS, Windows)
- Provide model validation tool to check compatibility
- Fallback to pure Rust inference if ONNX unavailable

**Contingency**: Implement lightweight Rust-native GNN inference as fallback.

---

**Risk 6: Regression in Recall**

*Probability: Medium | Impact: Critical*

**Description**: GNN routing might skip true nearest neighbors, degrading result quality.

**Mitigation**:
- Extensive recall testing in Phase 2
- Set minimum recall threshold (≥95% of baseline)
- Add recall monitoring in production
- Use hybrid mode (GNN + distance heuristic) for safety
- Comprehensive regression test suite

**Contingency**: If recall drops, increase `hybrid_alpha` to rely more on distance heuristic, or disable GNN routing entirely.

---

### Summary Risk Matrix

| Risk | Probability | Impact | Mitigation Priority |
|------|-------------|--------|---------------------|
| GNN inference overhead | Medium | High | **HIGH** - Profile early |
| Training data scarcity | Medium | Medium | Medium - Augmentation |
| Model generalization | Low | High | Medium - Diverse training |
| Feature cache bloat | Low | Medium | Low - Monitor in Phase 3 |
| ONNX compatibility | Low | Medium | Low - Validation tools |
| Recall regression | Medium | Critical | **HIGH** - Regression tests |

---

## Next Steps

1. **Prototype Phase 1**: Build minimal GNN routing with mock model (1 week)
2. **Collect Training Data**: Run 100K queries on existing HNSW, log trajectories (3 days)
3. **Train Initial Model**: Use collected data to train baseline GAT model (2 days)
4. **Integration Testing**: Plug GNN into HNSW, measure initial performance (1 week)
5. **Iterate**: Optimize based on profiling results (ongoing)

**Key Decision Points**:
- After Phase 1: Is GNN inference fast enough? (<5ms target)
- After Phase 2: Does GNN improve QPS? (>10% required to continue)
- After Phase 3: Does GNN meet all success metrics? (Go/No-Go for Phase 4)
