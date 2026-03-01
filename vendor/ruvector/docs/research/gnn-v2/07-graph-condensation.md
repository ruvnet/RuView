# Graph Condensation (SFGC) - Implementation Plan

## Overview

### Problem Statement

HNSW graphs in production vector databases face critical deployment challenges:

1. **Memory Footprint**: Full HNSW graphs require 40-120 bytes per vector for connectivity metadata
2. **Edge Deployment**: Mobile/IoT devices cannot store million-node graphs (400MB-4.8GB overhead)
3. **Federated Learning**: Transferring full graphs between nodes is bandwidth-prohibitive
4. **Cold Start**: Initial graph construction is expensive for dynamic applications

### Proposed Solution

Implement Structure-Preserving Graph Condensation (SFGC) that creates synthetic "super-nodes" representing clusters of original nodes. The condensed graph:

- Reduces graph size by 10-100x (configurable compression ratio)
- Preserves topological properties (small-world, scale-free characteristics)
- Maintains search accuracy within 2-5% of full graph
- Enables progressive graph expansion from condensed to full representation

**Core Innovation**: Unlike naive graph coarsening, SFGC learns synthetic node embeddings that maximize structural fidelity using a differentiable graph neural network.

### Expected Benefits (Quantified)

| Metric | Current (Full HNSW) | With SFGC (50x) | Improvement |
|--------|---------------------|-----------------|-------------|
| Memory footprint | 4.8GB (1M vectors) | 96MB | 50x reduction |
| Transfer bandwidth | 4.8GB | 96MB | 50x reduction |
| Edge device compatibility | Limited to 100K vectors | 5M vectors | 50x capacity |
| Cold start time | 120s | 8s + progressive | 15x faster |
| Search accuracy (recall@10) | 0.95 | 0.92-0.94 | 2-3% degradation |
| Search latency | 1.2ms | 1.5ms (initial), 1.2ms (expanded) | 25% slower → same |

**ROI Calculation**:
- Edge deployment: enables $500 devices vs $2000 workstations
- Federated learning: 50x faster synchronization (2.4s vs 120s)
- Multi-tenant SaaS: 50x more graphs per server

## Technical Design

### Architecture Diagram (ASCII)

```
┌─────────────────────────────────────────────────────────────────┐
│                     Graph Condensation Pipeline                  │
└─────────────────────────────────────────────────────────────────┘
                                  │
                    ┌─────────────┴─────────────┐
                    ▼                           ▼
        ┌───────────────────────┐   ┌───────────────────────┐
        │  Offline Condensation │   │  Online Expansion     │
        │      (Training)       │   │     (Runtime)         │
        └───────────────────────┘   └───────────────────────┘
                    │                           │
        ┌───────────┼───────────┐              │
        ▼           ▼           ▼              ▼
    ┌────────┐ ┌────────┐ ┌─────────┐  ┌─────────────┐
    │Cluster │ │Synth   │ │Edge     │  │Progressive  │
    │ ing    │ │Node    │ │Preserv  │  │Decompression│
    │        │ │Learn   │ │ation    │  │             │
    └────────┘ └────────┘ └─────────┘  └─────────────┘
        │           │           │              │
        └───────────┼───────────┘              │
                    ▼                          ▼
            ┌──────────────┐          ┌──────────────┐
            │  Condensed   │─────────▶│  Hybrid      │
            │  Graph File  │  Load    │  Graph Store │
            │  (.cgraph)   │          │              │
            └──────────────┘          └──────────────┘
                                              │
                                              ▼
                                      ┌──────────────┐
                                      │  Search API  │
                                      │  (adaptive)  │
                                      └──────────────┘
```

**Component Flow**:

1. **Offline Condensation** (Training Phase):
   - Hierarchical clustering of original graph
   - GNN-based synthetic node embedding learning
   - Edge weight optimization via structure preservation loss
   - Export to `.cgraph` format

2. **Online Expansion** (Runtime):
   - Load condensed graph for fast cold start
   - Progressive decompression on cache misses
   - Adaptive switching between condensed/full graph

### Core Data Structures (Rust)

```rust
/// Condensed graph representation with synthetic nodes
#[derive(Clone, Debug)]
pub struct CondensedGraph {
    /// Synthetic node embeddings (learned via GNN)
    pub synthetic_nodes: Vec<SyntheticNode>,

    /// Condensed HNSW layers (smaller topology)
    pub condensed_layers: Vec<HnswLayer>,

    /// Compression ratio (e.g., 50.0 for 50x)
    pub compression_ratio: f32,

    /// Mapping from synthetic node to original node IDs
    pub expansion_map: HashMap<NodeId, Vec<NodeId>>,

    /// Graph statistics for adaptive expansion
    pub stats: GraphStatistics,
}

/// Synthetic node representing a cluster of original nodes
#[derive(Clone, Debug)]
pub struct SyntheticNode {
    /// Learned embedding (centroid of cluster, refined via GNN)
    pub embedding: Vec<f32>,

    /// Original node IDs in this cluster
    pub cluster_members: Vec<NodeId>,

    /// Cluster radius (for expansion threshold)
    pub radius: f32,

    /// Connectivity in condensed graph
    pub neighbors: Vec<(NodeId, f32)>, // (neighbor_id, edge_weight)

    /// Access frequency (for adaptive expansion)
    pub access_count: AtomicU64,
}

/// Configuration for graph condensation process
#[derive(Clone, Debug)]
pub struct CondensationConfig {
    /// Target compression ratio (10-100)
    pub compression_ratio: f32,

    /// Clustering method
    pub clustering_method: ClusteringMethod,

    /// GNN training epochs for synthetic nodes
    pub gnn_epochs: usize,

    /// Structure preservation weight (vs embedding quality)
    pub structure_weight: f32,

    /// Edge preservation strategy
    pub edge_strategy: EdgePreservationStrategy,
}

#[derive(Clone, Debug)]
pub enum ClusteringMethod {
    /// Hierarchical agglomerative clustering
    Hierarchical { linkage: LinkageType },

    /// Louvain modularity-based clustering
    Louvain { resolution: f32 },

    /// Spectral clustering via graph Laplacian
    Spectral { n_components: usize },

    /// Custom clustering function
    Custom(Box<dyn Fn(&HnswIndex) -> Vec<Vec<NodeId>>>),
}

#[derive(Clone, Debug)]
pub enum EdgePreservationStrategy {
    /// Keep edges if both endpoints map to different synthetic nodes
    InterCluster,

    /// Weighted by cluster similarity
    WeightedSimilarity,

    /// Learn edge weights via GNN
    Learned,
}

/// Hybrid graph store supporting both condensed and full graphs
pub struct HybridGraphStore {
    /// Condensed graph (always loaded)
    condensed: CondensedGraph,

    /// Full graph (lazily loaded regions)
    full_graph: Option<Arc<RwLock<HnswIndex>>>,

    /// Expanded regions cache
    expanded_cache: LruCache<NodeId, ExpandedRegion>,

    /// Expansion policy
    policy: ExpansionPolicy,
}

/// Policy for when to expand condensed nodes to full graph
#[derive(Clone, Debug)]
pub enum ExpansionPolicy {
    /// Never expand (use condensed graph only)
    Never,

    /// Expand on cache miss
    OnDemand { cache_size: usize },

    /// Expand regions with high query frequency
    Adaptive { threshold: f64 },

    /// Always use full graph
    Always,
}

/// Expanded region of the full graph
struct ExpandedRegion {
    /// Full node data for this region
    nodes: Vec<FullNode>,

    /// Last access timestamp
    last_access: Instant,

    /// Access count
    access_count: u64,
}

/// Statistics for monitoring condensation quality
#[derive(Clone, Debug, Default)]
pub struct GraphStatistics {
    /// Average cluster size
    pub avg_cluster_size: f32,

    /// Cluster size variance
    pub cluster_variance: f32,

    /// Edge preservation ratio (condensed edges / original edges)
    pub edge_preservation: f32,

    /// Average path length increase
    pub path_length_delta: f32,

    /// Clustering coefficient preservation
    pub clustering_coef_ratio: f32,
}
```

### Key Algorithms (Pseudocode)

#### Algorithm 1: Graph Condensation (Offline Training)

```
function condense_graph(hnsw_index, config):
    # Step 1: Hierarchical clustering
    clusters = hierarchical_cluster(
        hnsw_index.nodes,
        target_clusters = hnsw_index.size / config.compression_ratio
    )

    # Step 2: Initialize synthetic node embeddings
    synthetic_nodes = []
    for cluster in clusters:
        centroid = compute_centroid(cluster.members)
        synthetic_nodes.append(SyntheticNode {
            embedding: centroid,
            cluster_members: cluster.members,
            radius: compute_cluster_radius(cluster),
            neighbors: [],
            access_count: 0
        })

    # Step 3: Build condensed edges
    condensed_edges = build_condensed_edges(
        hnsw_index,
        clusters,
        config.edge_strategy
    )

    # Step 4: GNN-based refinement
    gnn_model = GraphNeuralNetwork(
        input_dim = embedding_dim,
        hidden_dims = [128, 64],
        output_dim = embedding_dim
    )

    optimizer = Adam(gnn_model.parameters(), lr=0.001)

    for epoch in 1..config.gnn_epochs:
        # Forward pass: refine synthetic embeddings
        refined_embeddings = gnn_model.forward(
            synthetic_nodes.embeddings,
            condensed_edges
        )

        # Compute structure preservation loss
        loss = compute_structure_loss(
            refined_embeddings,
            condensed_edges,
            original_graph = hnsw_index,
            expansion_map = clusters,
            structure_weight = config.structure_weight
        )

        # Backward pass
        loss.backward()
        optimizer.step()

        # Update synthetic embeddings
        for i, node in enumerate(synthetic_nodes):
            node.embedding = refined_embeddings[i]

    # Step 5: Build condensed HNSW layers
    condensed_layers = build_hnsw_layers(
        synthetic_nodes,
        condensed_edges,
        max_layer = hnsw_index.max_layer
    )

    return CondensedGraph {
        synthetic_nodes,
        condensed_layers,
        compression_ratio: config.compression_ratio,
        expansion_map: clusters,
        stats: compute_statistics(...)
    }

function compute_structure_loss(embeddings, edges, original_graph, expansion_map, structure_weight):
    # Part 1: Embedding quality (centroid fidelity)
    embedding_loss = 0
    for i, synthetic_node in enumerate(embeddings):
        cluster_members = expansion_map[i]
        original_embeddings = [original_graph.get_embedding(id) for id in cluster_members]
        true_centroid = mean(original_embeddings)
        embedding_loss += mse(synthetic_node, true_centroid)

    # Part 2: Structure preservation (edge connectivity)
    structure_loss = 0
    for (u, v, weight) in edges:
        # Check if original graph had path between clusters u and v
        cluster_u = expansion_map[u]
        cluster_v = expansion_map[v]
        original_connectivity = compute_inter_cluster_connectivity(
            original_graph, cluster_u, cluster_v
        )
        predicted_connectivity = cosine_similarity(embeddings[u], embeddings[v])
        structure_loss += mse(predicted_connectivity, original_connectivity)

    # Part 3: Topological invariants
    topo_loss = 0
    condensed_clustering_coef = compute_clustering_coefficient(embeddings, edges)
    original_clustering_coef = original_graph.clustering_coefficient
    topo_loss += abs(condensed_clustering_coef - original_clustering_coef)

    return (1 - structure_weight) * embedding_loss +
           structure_weight * (structure_loss + 0.1 * topo_loss)
```

#### Algorithm 2: Progressive Expansion (Online Runtime)

```
function search_hybrid_graph(query, k, hybrid_store):
    # Step 1: Search in condensed graph
    condensed_results = search_condensed(
        query,
        hybrid_store.condensed,
        k_initial = k * 2  # oversample
    )

    # Step 2: Decide whether to expand
    if hybrid_store.policy == ExpansionPolicy::Never:
        return refine_condensed_results(condensed_results, k)

    # Step 3: Identify expansion candidates
    expansion_candidates = []
    for result in condensed_results:
        synthetic_node = result.node

        # Expand if: high uncertainty OR cache miss OR high query frequency
        should_expand = (
            result.distance < synthetic_node.radius * 1.5 OR  # uncertainty
            not hybrid_store.expanded_cache.contains(synthetic_node.id) OR  # cache miss
            synthetic_node.access_count.load() > adaptive_threshold  # hot region
        )

        if should_expand:
            expansion_candidates.append(synthetic_node.id)

    # Step 4: Expand regions (lazily load from full graph)
    if len(expansion_candidates) > 0:
        expanded_regions = hybrid_store.expand_regions(expansion_candidates)

        # Step 5: Refine search in expanded regions
        refined_results = []
        for region in expanded_regions:
            local_results = search_full_graph(
                query,
                region.nodes,
                k_local = k
            )
            refined_results.extend(local_results)

        # Merge condensed and expanded results
        all_results = merge_results(condensed_results, refined_results)
        return top_k(all_results, k)
    else:
        # No expansion needed
        return refine_condensed_results(condensed_results, k)

function expand_regions(hybrid_store, synthetic_node_ids):
    expanded = []
    for node_id in synthetic_node_ids:
        # Check cache first
        if hybrid_store.expanded_cache.contains(node_id):
            expanded.append(hybrid_store.expanded_cache.get(node_id))
            continue

        # Load from full graph (disk or memory)
        synthetic_node = hybrid_store.condensed.synthetic_nodes[node_id]
        cluster_member_ids = synthetic_node.cluster_members

        full_nodes = []
        if hybrid_store.full_graph.is_some():
            # Full graph in memory
            full_graph = hybrid_store.full_graph.unwrap()
            for member_id in cluster_member_ids:
                full_nodes.append(full_graph.get_node(member_id))
        else:
            # Load from disk (mmap)
            full_nodes = load_nodes_from_disk(cluster_member_ids)

        region = ExpandedRegion {
            nodes: full_nodes,
            last_access: now(),
            access_count: 1
        }

        # Add to cache (evict LRU if full)
        hybrid_store.expanded_cache.put(node_id, region)
        expanded.append(region)

    return expanded
```

### API Design (Function Signatures)

```rust
// ============================================================
// Public API for Graph Condensation
// ============================================================

pub trait GraphCondensation {
    /// Condense an HNSW index into a smaller graph
    fn condense(
        &self,
        config: CondensationConfig,
    ) -> Result<CondensedGraph, CondensationError>;

    /// Save condensed graph to disk
    fn save_condensed(&self, path: &Path) -> Result<(), io::Error>;

    /// Load condensed graph from disk
    fn load_condensed(path: &Path) -> Result<CondensedGraph, io::Error>;

    /// Validate condensation quality
    fn validate_condensation(
        &self,
        condensed: &CondensedGraph,
        test_queries: &[Vec<f32>],
    ) -> ValidationMetrics;
}

pub trait HybridGraphSearch {
    /// Search using hybrid condensed/full graph
    fn search_hybrid(
        &self,
        query: &[f32],
        k: usize,
        policy: ExpansionPolicy,
    ) -> Result<Vec<SearchResult>, SearchError>;

    /// Adaptive search with automatic expansion
    fn search_adaptive(
        &self,
        query: &[f32],
        k: usize,
        recall_target: f32,  // e.g., 0.95
    ) -> Result<Vec<SearchResult>, SearchError>;

    /// Get current cache statistics
    fn cache_stats(&self) -> CacheStatistics;

    /// Preload hot regions into cache
    fn warmup_cache(&mut self, query_log: &[Vec<f32>]) -> Result<(), CacheError>;
}

// ============================================================
// Configuration API
// ============================================================

impl CondensationConfig {
    /// Default configuration for 50x compression
    pub fn default_50x() -> Self {
        Self {
            compression_ratio: 50.0,
            clustering_method: ClusteringMethod::Hierarchical {
                linkage: LinkageType::Ward,
            },
            gnn_epochs: 100,
            structure_weight: 0.7,
            edge_strategy: EdgePreservationStrategy::Learned,
        }
    }

    /// Aggressive compression for edge devices (100x)
    pub fn edge_device() -> Self {
        Self {
            compression_ratio: 100.0,
            clustering_method: ClusteringMethod::Louvain {
                resolution: 1.2,
            },
            gnn_epochs: 50,
            structure_weight: 0.5,
            edge_strategy: EdgePreservationStrategy::InterCluster,
        }
    }

    /// Conservative compression for high accuracy (10x)
    pub fn high_accuracy() -> Self {
        Self {
            compression_ratio: 10.0,
            clustering_method: ClusteringMethod::Spectral {
                n_components: 128,
            },
            gnn_epochs: 200,
            structure_weight: 0.9,
            edge_strategy: EdgePreservationStrategy::Learned,
        }
    }
}

// ============================================================
// Monitoring and Metrics
// ============================================================

#[derive(Clone, Debug)]
pub struct ValidationMetrics {
    /// Recall at different k values
    pub recall_at_k: HashMap<usize, f32>,

    /// Average path length increase
    pub avg_path_length_ratio: f32,

    /// Search latency comparison
    pub latency_ratio: f32,

    /// Memory reduction achieved
    pub memory_reduction: f32,

    /// Graph property preservation
    pub property_preservation: PropertyPreservation,
}

#[derive(Clone, Debug)]
pub struct PropertyPreservation {
    pub clustering_coefficient: f32,
    pub average_degree: f32,
    pub diameter_ratio: f32,
}

#[derive(Clone, Debug)]
pub struct CacheStatistics {
    pub hit_rate: f32,
    pub eviction_count: u64,
    pub avg_expansion_time: Duration,
    pub total_expansions: u64,
}
```

## Integration Points

### Affected Crates/Modules

1. **`ruvector-gnn` (Core GNN crate)**:
   - Add `condensation/` module for graph compression
   - Extend `HnswIndex` with `condense()` method
   - Add GNN training loop for synthetic node refinement

2. **`ruvector-core`**:
   - Add `CondensedGraph` serialization format (`.cgraph`)
   - Extend search API with hybrid search modes
   - Add `HybridGraphStore` as alternative index backend

3. **`ruvector-gnn-node` (Node.js bindings)**:
   - Expose `condense()` API to JavaScript/TypeScript
   - Add configuration builder for condensation parameters
   - Provide progress callbacks for offline condensation

4. **`ruvector-cli`**:
   - Add `ruvector condense` command for offline condensation
   - Add `ruvector validate-condensed` for quality testing
   - Add visualization for condensed graph statistics

5. **`ruvector-distributed`**:
   - Use condensed graphs for federated learning synchronization
   - Implement condensed graph transfer protocol
   - Add merge logic for condensed graphs from multiple nodes

### New Modules to Create

```
crates/ruvector-gnn/src/condensation/
├── mod.rs                    # Public API
├── clustering.rs             # Hierarchical/Louvain/Spectral clustering
├── synthetic_node.rs         # Synthetic node learning via GNN
├── edge_preservation.rs      # Edge weight computation
├── gnn_trainer.rs            # GNN training loop
├── structure_loss.rs         # Loss functions for structure preservation
├── serialization.rs          # .cgraph format I/O
└── validation.rs             # Quality metrics

crates/ruvector-core/src/hybrid/
├── mod.rs                    # HybridGraphStore
├── expansion_policy.rs       # Adaptive expansion logic
├── cache.rs                  # LRU cache for expanded regions
└── search.rs                 # Hybrid search algorithms

crates/ruvector-gnn-node/condensation/
├── bindings.rs               # NAPI bindings
└── typescript/
    └── condensation.d.ts     # TypeScript definitions
```

### Dependencies on Other Features

1. **Prerequisite: Attention Mechanisms (Tier 1)**:
   - SFGC uses attention-weighted clustering
   - Synthetic node embeddings benefit from attention-based aggregation
   - **Action**: Ensure attention module is stable before SFGC integration

2. **Synergy: Adaptive HNSW (Tier 2, Feature #5)**:
   - Adaptive HNSW can use condensed graph for cold start
   - Layer-wise compression ratios (compress higher layers more aggressively)
   - **Integration**: Shared `ExpansionPolicy` trait

3. **Optional: Neuromorphic Spiking (Tier 2, Feature #6)**:
   - Spiking networks can accelerate GNN training for synthetic nodes
   - **Integration**: Conditional compilation flag for spiking backend

4. **Complementary: Sparse Attention (Tier 3, Feature #8)**:
   - Sparse attention patterns can guide clustering
   - **Integration**: Use learned attention masks as clustering hints

## Regression Prevention

### Existing Functionality at Risk

1. **HNSW Search Accuracy**:
   - **Risk**: Condensed graph returns lower-quality results
   - **Mitigation**:
     - Validate recall@10 >= 0.92 on standard benchmarks (SIFT1M, GIST1M)
     - Add A/B testing framework for condensed vs full graph
     - Default to conservative 10x compression

2. **Memory Safety (Rust)**:
   - **Risk**: Expansion cache causes use-after-free or data races
   - **Mitigation**:
     - Use `Arc<RwLock<...>>` for shared ownership
     - Fuzz testing with ThreadSanitizer
     - Property-based testing with `proptest`

3. **Serialization Format Compatibility**:
   - **Risk**: `.cgraph` format breaks existing index loading
   - **Mitigation**:
     - Separate file extension (`.cgraph` vs `.hnsw`)
     - Version magic number in header
     - Fallback to full graph if condensation fails

4. **Node.js Bindings Performance**:
   - **Risk**: Condensation adds latency to JavaScript API
   - **Mitigation**:
     - Make condensation opt-in (separate method)
     - Async/non-blocking condensation API
     - Progress callbacks to avoid blocking event loop

### Test Cases to Prevent Regressions

```rust
// Test 1: Search quality preservation
#[test]
fn test_condensed_search_recall() {
    let full_index = build_test_index(10000);
    let condensed = full_index.condense(CondensationConfig::default_50x()).unwrap();

    let test_queries = generate_test_queries(100);

    for query in test_queries {
        let full_results = full_index.search(&query, 10);
        let condensed_results = condensed.search(&query, 10);

        let recall = compute_recall(&full_results, &condensed_results);
        assert!(recall >= 0.92, "Recall dropped below 92%: {}", recall);
    }
}

// Test 2: Memory reduction
#[test]
fn test_memory_footprint() {
    let full_index = build_test_index(100000);
    let condensed = full_index.condense(CondensationConfig::default_50x()).unwrap();

    let full_size = full_index.memory_usage();
    let condensed_size = condensed.memory_usage();

    let reduction = full_size as f32 / condensed_size as f32;
    assert!(reduction >= 40.0, "Memory reduction below 40x: {}", reduction);
}

// Test 3: Serialization round-trip
#[test]
fn test_condensed_serialization() {
    let original = build_test_index(1000).condense(CondensationConfig::default_50x()).unwrap();

    let path = "/tmp/test.cgraph";
    original.save_condensed(Path::new(path)).unwrap();
    let loaded = CondensedGraph::load_condensed(Path::new(path)).unwrap();

    assert_eq!(original.synthetic_nodes.len(), loaded.synthetic_nodes.len());
    assert_eq!(original.compression_ratio, loaded.compression_ratio);
}

// Test 4: Hybrid search correctness
#[test]
fn test_hybrid_search_equivalence() {
    let full_index = build_test_index(5000);
    let condensed = full_index.condense(CondensationConfig::default_50x()).unwrap();

    let hybrid_store = HybridGraphStore::new(condensed, Some(Arc::new(RwLock::new(full_index))));

    let query = generate_random_query();

    // With ExpansionPolicy::Always, hybrid should match full graph
    let hybrid_results = hybrid_store.search_hybrid(&query, 10, ExpansionPolicy::Always).unwrap();
    let full_results = full_index.search(&query, 10);

    assert_eq!(hybrid_results, full_results);
}

// Test 5: Concurrent expansion safety
#[test]
fn test_concurrent_expansion() {
    let hybrid_store = Arc::new(RwLock::new(build_hybrid_store()));

    let handles: Vec<_> = (0..10).map(|_| {
        let store = Arc::clone(&hybrid_store);
        thread::spawn(move || {
            let query = generate_random_query();
            let results = store.write().unwrap().search_hybrid(
                &query, 10, ExpansionPolicy::OnDemand { cache_size: 100 }
            );
            assert!(results.is_ok());
        })
    }).collect();

    for handle in handles {
        handle.join().unwrap();
    }
}
```

### Backward Compatibility Strategy

1. **API Level**:
   - Keep existing `HnswIndex::search()` unchanged
   - Add new `HnswIndex::condense()` method (opt-in)
   - Condensed search via separate `HybridGraphStore` type

2. **File Format**:
   - Condensed graphs use `.cgraph` extension
   - Original `.hnsw` format unchanged
   - Metadata includes version + compression ratio

3. **Node.js Bindings**:
   - Add `index.condense(config)` method (returns new `CondensedIndex` instance)
   - Keep `index.search()` behavior identical
   - Add `condensedIndex.searchHybrid()` for hybrid mode

4. **CLI**:
   - `ruvector build` unchanged (builds full graph)
   - New `ruvector condense` command (separate step)
   - Auto-detect `.cgraph` vs `.hnsw` on load

## Implementation Phases

### Phase 1: Core Implementation (Weeks 1-3)

**Goals**:
- Implement clustering algorithms (hierarchical, Louvain)
- Build basic synthetic node creation (centroid-based, no GNN)
- Implement condensed HNSW layer construction
- Basic serialization (`.cgraph` format)

**Deliverables**:
```rust
// Week 1: Clustering
crates/ruvector-gnn/src/condensation/clustering.rs
  ✓ hierarchical_cluster()
  ✓ louvain_cluster()
  ✓ spectral_cluster()

// Week 2: Synthetic nodes + edges
crates/ruvector-gnn/src/condensation/synthetic_node.rs
  ✓ create_synthetic_nodes() // centroid-based
  ✓ build_condensed_edges()

// Week 3: Condensed graph + serialization
crates/ruvector-gnn/src/condensation/mod.rs
  ✓ CondensedGraph::from_hnsw()
  ✓ save_condensed() / load_condensed()
```

**Success Criteria**:
- Can condense 100K vector index to 2K synthetic nodes
- Serialization round-trip preserves graph structure
- Unit tests pass for clustering algorithms

### Phase 2: Integration (Weeks 4-6)

**Goals**:
- Integrate with `HnswIndex` API
- Add GNN-based synthetic node refinement
- Implement hybrid search with basic expansion policy
- Node.js bindings

**Deliverables**:
```rust
// Week 4: HNSW integration
crates/ruvector-gnn/src/hnsw/index.rs
  ✓ impl GraphCondensation for HnswIndex

// Week 5: GNN training
crates/ruvector-gnn/src/condensation/gnn_trainer.rs
  ✓ train_synthetic_embeddings()
  ✓ structure_preservation_loss()

// Week 6: Hybrid search
crates/ruvector-core/src/hybrid/
  ✓ HybridGraphStore::search_hybrid()
  ✓ ExpansionPolicy::OnDemand
```

**Success Criteria**:
- Recall@10 >= 0.90 on SIFT1M benchmark
- GNN training converges in <100 epochs
- Hybrid search passes correctness tests

### Phase 3: Optimization (Weeks 7-9)

**Goals**:
- Performance tuning (SIMD, caching)
- Adaptive expansion policy (query frequency tracking)
- Distributed condensation for federated learning
- CLI tool for offline condensation

**Deliverables**:
```rust
// Week 7: Performance optimization
crates/ruvector-gnn/src/condensation/
  ✓ SIMD-optimized centroid computation
  ✓ Parallel clustering (rayon)

// Week 8: Adaptive expansion
crates/ruvector-core/src/hybrid/
  ✓ ExpansionPolicy::Adaptive
  ✓ Query frequency tracking
  ✓ LRU cache tuning

// Week 9: CLI + distributed
crates/ruvector-cli/src/commands/condense.rs
  ✓ ruvector condense --ratio 50
crates/ruvector-distributed/src/sync.rs
  ✓ Condensed graph synchronization
```

**Success Criteria**:
- Condensation time <10s for 1M vectors
- Adaptive expansion improves latency by 20%+
- CLI can condense production-scale graphs

### Phase 4: Production Hardening (Weeks 10-12)

**Goals**:
- Comprehensive testing (property-based, fuzz, benchmarks)
- Documentation + examples
- Performance regression suite
- Multi-platform validation

**Deliverables**:
```rust
// Week 10: Testing
tests/condensation/
  ✓ Property-based tests (proptest)
  ✓ Fuzz testing (cargo-fuzz)
  ✓ Regression test suite

// Week 11: Documentation
docs/
  ✓ Graph Condensation Guide (user-facing)
  ✓ API documentation (rustdoc)
  ✓ Examples (edge device deployment)

// Week 12: Benchmarks + validation
benches/condensation.rs
  ✓ Condensation time benchmarks
  ✓ Search quality benchmarks
  ✓ Memory footprint benchmarks
```

**Success Criteria**:
- 100% code coverage for condensation module
- Passes all regression tests
- Documentation complete with 3+ examples
- Validated on ARM64, x86-64, WASM targets

## Success Metrics

### Performance Benchmarks

| Benchmark | Metric | Target | Measurement Method |
|-----------|--------|--------|-------------------|
| Condensation Time | Time to condense 1M vectors | <10s | `cargo bench condense_1m` |
| Memory Reduction | Footprint ratio (full/condensed) | 50x | `malloc_count` |
| Search Latency (condensed only) | p99 latency | <2ms | `criterion` benchmark |
| Search Latency (hybrid, cold) | p99 latency on first query | <3ms | Cache miss scenario |
| Search Latency (hybrid, warm) | p99 latency after warmup | <1.5ms | Cache hit scenario |
| Expansion Time | Time to expand 1 cluster | <0.5ms | `expand_regions()` profiling |

### Accuracy Metrics

| Dataset | Metric | Target | Baseline (Full Graph) |
|---------|--------|--------|-----------------------|
| SIFT1M | Recall@10 (50x compression) | >=0.92 | 0.95 |
| SIFT1M | Recall@100 (50x compression) | >=0.90 | 0.94 |
| GIST1M | Recall@10 (50x compression) | >=0.90 | 0.93 |
| GloVe-200 | Recall@10 (100x compression) | >=0.85 | 0.92 |
| Custom high-dim (1536d) | Recall@10 (50x compression) | >=0.88 | 0.94 |

### Memory/Latency Targets

| Configuration | Memory Footprint | Search Latency (p99) | Use Case |
|---------------|------------------|----------------------|----------|
| Full HNSW (1M vectors) | 4.8GB | 1.2ms | Server deployment |
| Condensed 50x (baseline) | 96MB | 1.5ms (cold), 1.2ms (warm) | Edge device |
| Condensed 100x (aggressive) | 48MB | 2.0ms (cold), 1.5ms (warm) | IoT device |
| Condensed 10x (conservative) | 480MB | 1.3ms (cold), 1.2ms (warm) | Embedded system |
| Hybrid (50x + on-demand) | 96MB + cache | 1.3ms (adaptive) | Mobile app |

**Measurement Tools**:
- Memory: `massif` (Valgrind), `heaptrack`, custom `malloc_count`
- Latency: `criterion` (Rust), `perf` (Linux profiling)
- Accuracy: Custom recall calculator against ground truth

### Quality Gates

All gates must pass before production release:

1. **Functional**:
   - ✓ All unit tests pass (100% coverage for core logic)
   - ✓ Integration tests pass on 3+ datasets
   - ✓ Serialization round-trip is lossless

2. **Performance**:
   - ✓ Memory reduction >= 40x (for 50x target config)
   - ✓ Condensation time <= 15s for 1M vectors
   - ✓ Search latency penalty <= 30% (cold start)

3. **Accuracy**:
   - ✓ Recall@10 >= 0.92 on SIFT1M (50x compression)
   - ✓ Recall@10 >= 0.85 on GIST1M (100x compression)
   - ✓ No catastrophic failures (recall < 0.5)

4. **Compatibility**:
   - ✓ Works on Linux x86-64, ARM64, macOS
   - ✓ Node.js bindings pass all tests
   - ✓ Backward compatible with existing indexes

## Risks and Mitigations

### Technical Risks

#### Risk 1: GNN Training Instability

**Description**:
Synthetic node embeddings may not converge during GNN training, leading to poor structure preservation.

**Probability**: Medium (30%)

**Impact**: High (blocks Phase 2)

**Mitigation**:
1. **Fallback**: Start with centroid-only embeddings (no GNN) in Phase 1
2. **Hyperparameter Tuning**: Grid search over learning rates (1e-4 to 1e-2)
3. **Loss Function Design**: Add regularization term to prevent mode collapse
4. **Early Stopping**: Monitor validation recall and stop if plateauing
5. **Alternative**: Use pre-trained graph embeddings (Node2Vec, GraphSAGE) if GNN fails

**Contingency Plan**:
If GNN training is unstable after 2 weeks of tuning, fall back to attention-weighted centroids (use existing attention mechanisms from Tier 1).

#### Risk 2: Cold Start Latency Regression

**Description**:
Condensed graph search may be slower than expected due to poor synthetic node placement.

**Probability**: Medium (40%)

**Impact**: Medium (user-facing latency)

**Mitigation**:
1. **Profiling**: Use `perf` to identify bottlenecks (likely distance computations)
2. **SIMD Optimization**: Vectorize distance calculations for synthetic nodes
3. **Caching**: Precompute pairwise distances between synthetic nodes
4. **Pruning**: Reduce condensed graph connectivity (fewer edges per node)
5. **Hybrid Strategy**: Always expand top-3 synthetic nodes to reduce uncertainty

**Contingency Plan**:
If cold start latency exceeds 2x full graph, add "warm cache" mode that preloads frequently accessed clusters based on query distribution.

#### Risk 3: Memory Overhead from Expansion Cache

**Description**:
LRU cache for expanded regions may consume more memory than expected, negating compression benefits.

**Probability**: Low (20%)

**Impact**: Medium (defeats purpose on edge devices)

**Mitigation**:
1. **Adaptive Cache Size**: Dynamically adjust cache size based on available memory
2. **Partial Expansion**: Only expand k-nearest neighbors within cluster (not full cluster)
3. **Compression**: Store expanded regions in quantized format (int8 instead of float32)
4. **Eviction Policy**: Evict based on access frequency + recency (LFU + LRU hybrid)

**Contingency Plan**:
If cache overhead exceeds 20% of condensed graph size, make expansion fully on-demand (no caching) and optimize expansion from disk (mmap).

#### Risk 4: Clustering Quality for High-Dimensional Data

**Description**:
Hierarchical clustering may produce imbalanced clusters in high-dimensional spaces (curse of dimensionality).

**Probability**: High (60%)

**Impact**: Medium (poor compression or accuracy)

**Mitigation**:
1. **Dimensionality Reduction**: Apply PCA or UMAP before clustering
2. **Alternative Algorithms**: Try spectral clustering or Louvain (graph-based, not distance-based)
3. **Cluster Validation**: Measure silhouette score and reject poor clusterings
4. **Adaptive Compression**: Use variable compression ratios per region (dense regions = higher compression)

**Contingency Plan**:
If clustering quality is poor (silhouette score < 0.3), switch to graph-based Louvain clustering using HNSW edges as adjacency matrix.

#### Risk 5: Serialization Format Bloat

**Description**:
`.cgraph` format may be larger than expected due to storing expansion maps and GNN weights.

**Probability**: Medium (35%)

**Impact**: Low (reduces compression benefits)

**Mitigation**:
1. **Sparse Storage**: Use sparse matrix formats (CSR) for expansion maps
2. **Quantization**: Store GNN embeddings in int8 (8x smaller)
3. **Compression**: Apply zstd compression to `.cgraph` file
4. **Lazy Loading**: Only load expansion map on-demand (not upfront)

**Contingency Plan**:
If `.cgraph` file exceeds 50% of condensed graph target size, remove GNN weights from serialization and recompute on load (trade disk space for CPU time).

### Operational Risks

#### Risk 6: User Confusion with Hybrid API

**Description**:
Users may not understand when to use condensed vs full vs hybrid graphs.

**Probability**: High (70%)

**Impact**: Low (documentation issue)

**Mitigation**:
1. **Clear Documentation**: Add decision tree (edge device → condensed, server → full, mobile → hybrid)
2. **Smart Defaults**: Auto-detect environment (check available memory) and choose policy
3. **Examples**: Provide 3 reference implementations (edge, mobile, server)
4. **Validation**: Add `validate_condensed()` method that warns if recall is too low

#### Risk 7: Debugging Difficulty

**Description**:
When condensed search returns wrong results, debugging is harder (no direct mapping to original nodes).

**Probability**: Medium (50%)

**Impact**: Medium (developer experience)

**Mitigation**:
1. **Logging**: Add verbose logging for expansion decisions
2. **Visualization**: Provide tool to visualize condensed graph + clusters
3. **Explain API**: Add `explain_search()` method that shows which clusters were searched
4. **Metrics**: Expose per-cluster recall metrics

---

## Appendix: Related Research

This design is based on:

1. **Graph Condensation for GNNs** (Jin et al., 2021): Core SFGC algorithm
2. **Structure-Preserving Graph Coarsening** (Loukas, 2019): Topological invariants
3. **Hierarchical Navigable Small Worlds** (Malkov & Yashunin, 2018): HNSW baseline
4. **Federated Graph Learning** (Wu et al., 2022): Distributed graph synchronization

Key differences from prior work:
- **Novel**: GNN-based synthetic node learning (prior work used simple centroids)
- **Novel**: Hybrid search with adaptive expansion (prior work only used condensed graph)
- **Engineering**: Production-ready Rust implementation with SIMD optimization
