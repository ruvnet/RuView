# Topology-Aware Gradient Routing (TAGR)

## Overview

### Problem Statement
Current vector search routing relies solely on embedding similarity, ignoring the rich topological structure of the graph. This leads to:
1. **Inefficient routing**: Missing "highway" nodes with high betweenness centrality
2. **Local optima**: Getting trapped in dense clusters without global context
3. **Uniform traversal**: Treating all graph regions identically despite varying structure
4. **Poor scalability**: Not leveraging graph properties for large-scale search

### Proposed Solution
Route search queries based on local graph topology metrics (degree, clustering coefficient, betweenness centrality) in addition to embedding similarity. Automatically identify:
- **Highway nodes**: High betweenness for long-range routing
- **Hub nodes**: High degree for local exploration
- **Bridge nodes**: Low clustering, connecting communities
- **Dense regions**: High clustering for specialized searches

### Expected Benefits
- **40-60% reduction** in path length for long-range queries
- **25-35% improvement** in search efficiency (fewer hops)
- **Automatic adaptation** to graph structure (no manual tuning)
- **Better load balancing** across graph regions
- **Hierarchical routing**: Global highways → local hubs → targets

### Novelty Claim
First integration of graph topology metrics directly into vector search routing. Unlike:
- **Community detection**: TAGR uses local metrics, no global clustering needed
- **Graph neural networks**: TAGR routes using topology, not learned representations
- **Hierarchical graphs**: TAGR adapts to natural topology, no imposed hierarchy

TAGR creates an adaptive routing strategy that respects the graph's intrinsic structure.

## Technical Design

### Architecture Diagram
```
┌────────────────────────────────────────────────────────────────────┐
│                 Topology-Aware Gradient Routing                     │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │              Topology Metric Computation                    │   │
│  │                                                             │   │
│  │  For each node i:                                          │   │
│  │  • Degree: deg(i) = |neighbors(i)|                        │   │
│  │  • Clustering: C(i) = triangles(i) / potential_triangles  │   │
│  │  • Betweenness: B(i) = Σ(σ_st(i) / σ_st)                 │   │
│  │  • PageRank: PR(i) = (1-d)/N + d·Σ(PR(j)/deg(j))        │   │
│  └────────────────────────────────────────────────────────────┘   │
│                            │                                        │
│                            ▼                                        │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │              Node Classification by Topology                │   │
│  │                                                             │   │
│  │   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │   │
│  │   │  HIGHWAY    │  │    HUB      │  │   BRIDGE    │      │   │
│  │   │             │  │             │  │             │      │   │
│  │   │ High B(i)   │  │ High deg(i) │  │ Low C(i)    │      │   │
│  │   │ Low C(i)    │  │ Med C(i)    │  │ Med B(i)    │      │   │
│  │   │             │  │             │  │             │      │   │
│  │   │ ●═══════●  │  │    ●───●    │  │  ●     ●    │      │   │
│  │   │         ║   │  │   ╱│╲  │    │  │  │     │    │      │   │
│  │   │         ║   │  │  ● │ ● │    │  │  ●─────●    │      │   │
│  │   │         ●   │  │   ╲│╱  │    │  │             │      │   │
│  │   │             │  │    ●───●    │  │             │      │   │
│  │   └─────────────┘  └─────────────┘  └─────────────┘      │   │
│  └────────────────────────────────────────────────────────────┘   │
│                            │                                        │
│                            ▼                                        │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │                  Adaptive Routing Strategy                  │   │
│  │                                                             │   │
│  │  Phase 1: Global Navigation                                │   │
│  │  ┌─────────────────────────────────────┐                  │   │
│  │  │ Route via HIGHWAY nodes             │                  │   │
│  │  │ Objective: minimize(distance to     │                  │   │
│  │  │            target community)        │                  │   │
│  │  │ Weight: 0.7·similarity +            │                  │   │
│  │  │         0.3·betweenness             │                  │   │
│  │  └─────────────────────────────────────┘                  │   │
│  │                    │                                        │   │
│  │                    ▼                                        │   │
│  │  Phase 2: Local Exploration                                │   │
│  │  ┌─────────────────────────────────────┐                  │   │
│  │  │ Route via HUB nodes                 │                  │   │
│  │  │ Objective: explore dense region     │                  │   │
│  │  │ Weight: 0.8·similarity +            │                  │   │
│  │  │         0.2·degree                  │                  │   │
│  │  └─────────────────────────────────────┘                  │   │
│  │                    │                                        │   │
│  │                    ▼                                        │   │
│  │  Phase 3: Precision Targeting                              │   │
│  │  ┌─────────────────────────────────────┐                  │   │
│  │  │ Pure similarity-based search        │                  │   │
│  │  │ Weight: 1.0·similarity              │                  │   │
│  │  └─────────────────────────────────────┘                  │   │
│  └────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────┘
```

### Core Data Structures

```rust
/// Topology metrics for each node
#[derive(Clone, Debug)]
pub struct NodeTopology {
    /// Node identifier
    pub node_id: NodeId,

    /// Degree (number of neighbors)
    pub degree: usize,

    /// Clustering coefficient (0.0-1.0)
    pub clustering: f32,

    /// Betweenness centrality (normalized)
    pub betweenness: f32,

    /// PageRank score
    pub pagerank: f32,

    /// Closeness centrality
    pub closeness: f32,

    /// Eigenvector centrality
    pub eigenvector: f32,

    /// Node classification
    pub classification: NodeClass,
}

/// Node classification based on topology
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeClass {
    /// High betweenness, low clustering (long-range routing)
    Highway,

    /// High degree, medium clustering (local exploration)
    Hub,

    /// Low clustering, medium betweenness (community connector)
    Bridge,

    /// High clustering (dense local region)
    Dense,

    /// Low degree, high clustering (leaf node)
    Leaf,

    /// Doesn't fit other categories
    Ordinary,
}

/// Configuration for topology-aware routing
#[derive(Clone, Debug)]
pub struct TagrConfig {
    /// Metrics to compute (performance vs. accuracy trade-off)
    pub metrics: MetricSet,

    /// Node classification thresholds
    pub classification_thresholds: ClassificationThresholds,

    /// Routing strategy
    pub routing_strategy: RoutingStrategy,

    /// Update frequency for topology metrics
    pub update_interval: Duration,

    /// Enable adaptive weight tuning
    pub adaptive_weights: bool,
}

/// Which topology metrics to compute
#[derive(Clone, Debug)]
pub struct MetricSet {
    pub degree: bool,
    pub clustering: bool,
    pub betweenness: bool,
    pub pagerank: bool,
    pub closeness: bool,
    pub eigenvector: bool,
}

/// Thresholds for node classification
#[derive(Clone, Debug)]
pub struct ClassificationThresholds {
    /// Betweenness threshold for highways (top X%)
    pub highway_betweenness_percentile: f32,  // default: 0.95

    /// Degree threshold for hubs (top X%)
    pub hub_degree_percentile: f32,  // default: 0.90

    /// Clustering threshold for dense regions
    pub dense_clustering_threshold: f32,  // default: 0.7

    /// Maximum clustering for bridges
    pub bridge_clustering_max: f32,  // default: 0.3
}

/// Routing strategy configuration
#[derive(Clone, Debug)]
pub enum RoutingStrategy {
    /// Three-phase: highway → hub → target
    ThreePhase {
        phase1_weight: PhaseWeights,
        phase2_weight: PhaseWeights,
        phase3_weight: PhaseWeights,
    },

    /// Adaptive: dynamically choose weights based on query progress
    Adaptive {
        initial_weights: PhaseWeights,
        adaptation_rate: f32,
    },

    /// Custom strategy
    Custom(fn(&SearchState) -> PhaseWeights),
}

/// Weights for combining similarity and topology
#[derive(Clone, Debug)]
pub struct PhaseWeights {
    pub similarity: f32,
    pub degree: f32,
    pub clustering: f32,
    pub betweenness: f32,
    pub pagerank: f32,
}

/// Current search state for adaptive routing
#[derive(Clone, Debug)]
pub struct SearchState {
    /// Nodes visited so far
    pub visited: Vec<NodeId>,

    /// Current position
    pub current: NodeId,

    /// Best similarity seen so far
    pub best_similarity: f32,

    /// Number of hops taken
    pub hops: usize,

    /// Estimated distance to target (embedding space)
    pub estimated_distance: f32,
}

/// Topology-aware router
pub struct TopologyRouter {
    /// Topology metrics for all nodes
    metrics: Vec<NodeTopology>,

    /// Fast lookup by node class
    class_index: HashMap<NodeClass, Vec<NodeId>>,

    /// Configuration
    config: TagrConfig,

    /// Cached routing decisions
    routing_cache: LruCache<(NodeId, NodeId), Vec<NodeId>>,
}
```

### Key Algorithms

```rust
// Pseudocode for topology-aware routing

/// Compute topology metrics for graph
fn compute_topology_metrics(graph: &HnswGraph) -> Vec<NodeTopology> {
    let n = graph.node_count();
    let mut metrics = vec![NodeTopology::default(); n];

    // Phase 1: Local metrics (degree, clustering)
    for node in 0..n {
        let neighbors = graph.get_neighbors(node, layer=0);
        metrics[node].degree = neighbors.len();

        // Clustering coefficient: fraction of neighbor pairs connected
        let mut triangles = 0;
        let mut possible = 0;

        for i in 0..neighbors.len() {
            for j in (i+1)..neighbors.len() {
                possible += 1;
                if graph.has_edge(neighbors[i], neighbors[j]) {
                    triangles += 1;
                }
            }
        }

        metrics[node].clustering = if possible > 0 {
            triangles as f32 / possible as f32
        } else {
            0.0
        };
    }

    // Phase 2: Global metrics (betweenness, PageRank)
    // Betweenness: fraction of shortest paths passing through node
    metrics = compute_betweenness(graph, metrics);

    // PageRank: iterative link analysis
    metrics = compute_pagerank(graph, metrics);

    // Phase 3: Classify nodes
    for i in 0..n {
        metrics[i].classification = classify_node(&metrics[i], &metrics);
    }

    metrics
}

/// Betweenness centrality using Brandes' algorithm
fn compute_betweenness(
    graph: &HnswGraph,
    mut metrics: Vec<NodeTopology>
) -> Vec<NodeTopology> {
    let n = graph.node_count();
    let mut betweenness = vec![0.0; n];

    // For each source node
    for s in 0..n {
        let mut stack = Vec::new();
        let mut paths = vec![Vec::new(); n];
        let mut sigma = vec![0.0; n];
        sigma[s] = 1.0;
        let mut dist = vec![-1; n];
        dist[s] = 0;

        // BFS from s
        let mut queue = VecDeque::new();
        queue.push_back(s);

        while let Some(v) = queue.pop_front() {
            stack.push(v);

            for w in graph.get_neighbors(v, layer=0) {
                // First visit to w?
                if dist[w] < 0 {
                    dist[w] = dist[v] + 1;
                    queue.push_back(w);
                }

                // Shortest path to w via v?
                if dist[w] == dist[v] + 1 {
                    sigma[w] += sigma[v];
                    paths[w].push(v);
                }
            }
        }

        // Accumulate betweenness
        let mut delta = vec![0.0; n];
        while let Some(w) = stack.pop() {
            for v in &paths[w] {
                delta[*v] += (sigma[*v] / sigma[w]) * (1.0 + delta[w]);
            }
            if w != s {
                betweenness[w] += delta[w];
            }
        }
    }

    // Normalize
    let max_betweenness = betweenness.iter().cloned().fold(0.0, f32::max);
    for i in 0..n {
        metrics[i].betweenness = betweenness[i] / max_betweenness;
    }

    metrics
}

/// Classify node based on topology metrics
fn classify_node(
    node: &NodeTopology,
    all_metrics: &[NodeTopology]
) -> NodeClass {
    // Compute percentiles
    let betweenness_percentile = compute_percentile(
        all_metrics.iter().map(|m| m.betweenness),
        node.betweenness
    );

    let degree_percentile = compute_percentile(
        all_metrics.iter().map(|m| m.degree as f32),
        node.degree as f32
    );

    // Classification logic
    if betweenness_percentile > 0.95 && node.clustering < 0.3 {
        NodeClass::Highway
    } else if degree_percentile > 0.90 && node.clustering > 0.4 {
        NodeClass::Hub
    } else if node.clustering < 0.3 && betweenness_percentile > 0.7 {
        NodeClass::Bridge
    } else if node.clustering > 0.7 {
        NodeClass::Dense
    } else if node.degree < 5 && node.clustering > 0.6 {
        NodeClass::Leaf
    } else {
        NodeClass::Ordinary
    }
}

/// Topology-aware search with three-phase routing
fn tagr_search(
    query: &[f32],
    graph: &HnswGraph,
    router: &TopologyRouter,
    k: usize
) -> Vec<SearchResult> {
    let mut current = graph.entry_point;
    let mut visited = HashSet::new();
    let mut best_similarity = -1.0;
    let mut hops = 0;

    let state = SearchState {
        visited: Vec::new(),
        current,
        best_similarity,
        hops,
        estimated_distance: f32::MAX,
    };

    // Phase 1: Global navigation via highways
    while in_phase_1(&state) {
        let neighbors = graph.get_neighbors(current, layer=0);
        let mut best_neighbor = None;
        let mut best_score = f32::MIN;

        for neighbor in neighbors {
            if visited.contains(&neighbor) { continue; }

            let topo = &router.metrics[neighbor];
            let embedding = graph.get_embedding(neighbor);
            let similarity = cosine_similarity(query, embedding);

            // Phase 1 weights: favor highways
            let score = 0.6 * similarity + 0.4 * topo.betweenness;

            if score > best_score {
                best_score = score;
                best_neighbor = Some(neighbor);
            }
        }

        if let Some(next) = best_neighbor {
            current = next;
            visited.insert(current);
            hops += 1;

            let similarity = cosine_similarity(
                query,
                graph.get_embedding(current)
            );
            best_similarity = best_similarity.max(similarity);
        } else {
            break;
        }
    }

    // Phase 2: Local exploration via hubs
    while in_phase_2(&state) {
        let neighbors = graph.get_neighbors(current, layer=0);
        let mut best_neighbor = None;
        let mut best_score = f32::MIN;

        for neighbor in neighbors {
            if visited.contains(&neighbor) { continue; }

            let topo = &router.metrics[neighbor];
            let embedding = graph.get_embedding(neighbor);
            let similarity = cosine_similarity(query, embedding);

            // Phase 2 weights: favor hubs and similarity
            let degree_score = topo.degree as f32 / graph.max_degree() as f32;
            let score = 0.8 * similarity + 0.2 * degree_score;

            if score > best_score {
                best_score = score;
                best_neighbor = Some(neighbor);
            }
        }

        if let Some(next) = best_neighbor {
            current = next;
            visited.insert(current);
            hops += 1;

            let similarity = cosine_similarity(
                query,
                graph.get_embedding(current)
            );
            best_similarity = best_similarity.max(similarity);
        } else {
            break;
        }
    }

    // Phase 3: Pure similarity search
    standard_greedy_search(query, graph, current, k, visited)
}

/// Adaptive weight tuning based on search progress
fn adaptive_routing(
    state: &SearchState,
    router: &TopologyRouter
) -> PhaseWeights {
    let progress = estimate_progress(state);

    // Early (global navigation): emphasize topology
    // Middle (local exploration): balanced
    // Late (precision targeting): emphasize similarity

    let topology_weight = (1.0 - progress) * 0.5;
    let similarity_weight = 0.5 + progress * 0.5;

    PhaseWeights {
        similarity: similarity_weight,
        degree: topology_weight * 0.3,
        clustering: topology_weight * 0.2,
        betweenness: topology_weight * 0.4,
        pagerank: topology_weight * 0.1,
    }
}
```

### API Design

```rust
/// Public API for Topology-Aware Gradient Routing
pub trait TopologyAwareRouting {
    /// Create topology router for graph
    fn new(graph: &HnswGraph, config: TagrConfig) -> Self;

    /// Search with topology-aware routing
    fn search(
        &self,
        query: &[f32],
        k: usize,
        options: TagrSearchOptions,
    ) -> Result<Vec<SearchResult>, TagrError>;

    /// Get topology metrics for node
    fn get_metrics(&self, node_id: NodeId) -> &NodeTopology;

    /// Find nearest highway nodes
    fn find_highways(&self, point: &[f32], k: usize) -> Vec<NodeId>;

    /// Find hubs in region
    fn find_hubs(&self, center: &[f32], radius: f32) -> Vec<NodeId>;

    /// Get nodes by classification
    fn get_by_class(&self, class: NodeClass) -> &[NodeId];

    /// Update topology metrics (incremental)
    fn update_metrics(&mut self, changed_nodes: &[NodeId]) -> Result<(), TagrError>;

    /// Recompute all metrics (full update)
    fn recompute_metrics(&mut self) -> Result<(), TagrError>;

    /// Export topology visualization
    fn export_topology(&self) -> TopologyVisualization;

    /// Get routing statistics
    fn statistics(&self) -> RoutingStatistics;
}

/// Search options for TAGR
#[derive(Clone, Debug)]
pub struct TagrSearchOptions {
    /// Routing strategy override
    pub strategy: Option<RoutingStrategy>,

    /// Prefer specific node classes
    pub prefer_classes: Vec<NodeClass>,

    /// Avoid specific node classes
    pub avoid_classes: Vec<NodeClass>,

    /// Enable path recording
    pub record_path: bool,

    /// Maximum hops
    pub max_hops: usize,
}

/// Routing statistics
#[derive(Clone, Debug)]
pub struct RoutingStatistics {
    /// Total searches performed
    pub total_searches: usize,

    /// Average path length
    pub avg_path_length: f32,

    /// Highway usage rate
    pub highway_usage: f32,

    /// Hub usage rate
    pub hub_usage: f32,

    /// Average hops by phase
    pub hops_by_phase: [f32; 3],

    /// Node class distribution
    pub class_distribution: HashMap<NodeClass, usize>,
}

/// Topology visualization export
#[derive(Clone, Debug, Serialize)]
pub struct TopologyVisualization {
    pub nodes: Vec<TopoNode>,
    pub highways: Vec<NodeId>,
    pub hubs: Vec<NodeId>,
    pub bridges: Vec<NodeId>,
    pub metrics_summary: MetricsSummary,
}

#[derive(Clone, Debug, Serialize)]
pub struct TopoNode {
    pub id: NodeId,
    pub class: NodeClass,
    pub degree: usize,
    pub betweenness: f32,
    pub clustering: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct MetricsSummary {
    pub total_nodes: usize,
    pub avg_degree: f32,
    pub avg_clustering: f32,
    pub max_betweenness: f32,
}
```

## Integration Points

### Affected Crates/Modules

1. **`crates/ruvector-core/src/hnsw/`**
   - Add topology metadata to nodes
   - Modify routing to use topology metrics
   - Extend search API for topology options

2. **`crates/ruvector-gnn/src/routing/`**
   - Create new routing module
   - Integrate with existing GNN layers

3. **`crates/ruvector-core/src/metrics/`**
   - Implement graph centrality algorithms
   - Add metric computation utilities

### New Modules to Create

1. **`crates/ruvector-gnn/src/topology/`**
   - `metrics.rs` - Topology metric computation
   - `classification.rs` - Node classification
   - `router.rs` - Topology-aware routing
   - `adaptive.rs` - Adaptive weight tuning
   - `cache.rs` - Metric caching and updates

2. **`crates/ruvector-core/src/graph/`**
   - `centrality.rs` - Centrality algorithms (betweenness, PageRank)
   - `clustering.rs` - Clustering coefficient
   - `analysis.rs` - Graph analysis utilities

### Dependencies on Other Features

- **Feature 10 (Gravitational Fields)**: Combine topology routing with gravitational pull
- **Feature 11 (Causal Networks)**: Adapt topology metrics for DAGs
- **Feature 13 (Crystallization)**: Use topology to identify hierarchy levels

## Regression Prevention

### Existing Functionality at Risk

1. **Search Performance**
   - Risk: Topology computation overhead
   - Prevention: Incremental updates, caching, optional feature

2. **Search Quality**
   - Risk: Poor topology routing on certain graph structures
   - Prevention: Adaptive fallback to pure similarity

3. **Memory Usage**
   - Risk: Storing topology metrics per node
   - Prevention: Lazy computation, sparse storage

### Test Cases

```rust
#[cfg(test)]
mod regression_tests {
    /// Verify highways reduce path length
    #[test]
    fn test_highway_routing_efficiency() {
        let graph = create_scale_free_graph(10000);
        let router = TopologyRouter::new(&graph, TagrConfig::default());

        let query = random_vector(128);

        // Standard search
        let (standard_results, standard_path) = graph.search_with_path(&query, 10);

        // TAGR search
        let (tagr_results, tagr_path) = router.search_with_path(&query, 10);

        // TAGR should take fewer hops
        assert!(tagr_path.len() < standard_path.len());

        // But maintain similar quality
        let standard_recall = compute_recall(&standard_results, &ground_truth);
        let tagr_recall = compute_recall(&tagr_results, &ground_truth);
        assert!((tagr_recall - standard_recall).abs() < 0.05);
    }

    /// Verify correct node classification
    #[test]
    fn test_node_classification() {
        let graph = create_test_graph_with_known_structure();
        let router = TopologyRouter::new(&graph, TagrConfig::default());

        // Verify known highways
        let highways = router.get_by_class(NodeClass::Highway);
        assert!(highways.contains(&known_highway_node));

        // Verify known hubs
        let hubs = router.get_by_class(NodeClass::Hub);
        assert!(hubs.contains(&known_hub_node));
    }

    /// Incremental metric updates
    #[test]
    fn test_incremental_updates() {
        let mut graph = create_test_graph(1000);
        let mut router = TopologyRouter::new(&graph, TagrConfig::default());

        let original_metrics = router.get_metrics(0).clone();

        // Add edges
        graph.add_edge(0, 500);
        graph.add_edge(0, 501);

        // Incremental update
        router.update_metrics(&[0, 500, 501]).unwrap();

        let updated_metrics = router.get_metrics(0);

        // Degree should increase
        assert!(updated_metrics.degree > original_metrics.degree);
    }
}
```

## Implementation Phases

### Phase 1: Research Validation (2 weeks)
- Implement basic topology metrics (degree, clustering)
- Test on synthetic graphs with known structure
- Measure routing efficiency improvements
- **Deliverable**: Research report with benchmarks

### Phase 2: Core Implementation (3 weeks)
- Implement all centrality metrics (betweenness, PageRank)
- Develop node classification
- Build three-phase routing
- Add caching and optimization
- **Deliverable**: Working TAGR module

### Phase 3: Integration (2 weeks)
- Integrate with HNSW search
- Add adaptive weight tuning
- Create API bindings
- Write integration tests
- **Deliverable**: Integrated TAGR feature

### Phase 4: Optimization (2 weeks)
- Profile and optimize metric computation
- Implement incremental updates
- Add visualization tools
- Write documentation
- **Deliverable**: Production-ready feature

## Success Metrics

### Performance Benchmarks

| Metric | Baseline | Target | Dataset |
|--------|----------|--------|---------|
| Path length reduction | 0% | >40% | Scale-free graph, 1M nodes |
| Search hops | 15.2 | <10.0 | Wikipedia embeddings |
| Metric computation time | N/A | <5s | Per 100K nodes |
| Memory overhead | 0MB | <200MB | Per 1M nodes |

### Accuracy Metrics

1. **Highway Identification**: Correlation with true betweenness
   - Target: Spearman correlation >0.85

2. **Routing Efficiency**: Hops saved vs. baseline
   - Target: >30% reduction for long-range queries

3. **Search Quality**: Recall maintained
   - Target: Recall degradation <5%

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Expensive betweenness computation | Approximate algorithms, sampling |
| Poor generalization | Test on diverse graph types |
| Classification instability | Regularization, threshold tuning |
| Metric staleness | Incremental updates, change detection |

## References

- Brandes' betweenness algorithm
- PageRank and graph centrality
- Small-world and scale-free networks
- Graph-based routing in P2P networks
