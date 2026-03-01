# RuVector Graph - Distributed Hypergraph Database Architecture

## Executive Summary

RuVector Graph is a distributed, Neo4j-compatible hypergraph database designed for extreme performance through SIMD optimization, lock-free data structures, and intelligent query execution. It extends traditional property graph models with hyperedge support for n-ary relationships while seamlessly integrating with RuVector's vector database capabilities for hybrid semantic-graph queries.

**Key Performance Targets:**
- **10-100x faster** than Neo4j for graph traversals (SIMD + cache-optimized layouts)
- **5-50x faster** for complex pattern matching (parallel execution + JIT compilation)
- **Sub-millisecond latency** for 3-hop neighborhood queries on billion-edge graphs
- **Linear scalability** to 100+ node clusters with federation support

---

## 1. System Architecture Overview

### 1.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      Client Applications                         │
│          (Cypher Queries, Vector+Graph Hybrid Queries)          │
└──────────────────────┬──────────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Query Coordinator Layer                       │
│  ┌──────────────┐  ┌───────────────┐  ┌────────────────────┐  │
│  │ Query Parser │─▶│ Query Planner │─▶│ Execution Engine   │  │
│  │  (Cypher)    │  │  (Optimizer)  │  │  (SIMD+Parallel)   │  │
│  └──────────────┘  └───────────────┘  └────────────────────┘  │
└──────────────────────┬──────────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Federation Layer                              │
│  ┌────────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ Cluster Router │  │ Query Merger │  │ Cross-Cluster    │   │
│  │                │  │              │  │ Transaction Mgr  │   │
│  └────────────────┘  └──────────────┘  └──────────────────┘   │
└──────────────────────┬──────────────────────────────────────────┘
                       │
        ┌──────────────┼──────────────┐
        ▼              ▼              ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│  Cluster 1   │ │  Cluster 2   │ │  Cluster N   │
│              │ │              │ │              │
│  ┌────────┐  │ │  ┌────────┐  │ │  ┌────────┐  │
│  │ RAFT   │  │ │  │ RAFT   │  │ │  │ RAFT   │  │
│  │ Leader │  │ │  │ Leader │  │ │  │ Leader │  │
│  └────┬───┘  │ │  └────┬───┘  │ │  └────┬───┘  │
│       │      │ │       │      │ │       │      │
│  ┌────▼────┐ │ │  ┌────▼────┐ │ │  ┌────▼────┐ │
│  │ Storage │ │ │  │ Storage │ │ │  │ Storage │ │
│  │ Engine  │ │ │  │ Engine  │ │ │  │ Engine  │ │
│  └─────────┘ │ │  └─────────┘ │ │  └─────────┘ │
└──────────────┘ └──────────────┘ └──────────────┘
```

### 1.2 Core Design Principles

1. **SIMD-First Design**: All critical paths use SIMD intrinsics for 4-8x speedup
2. **Lock-Free Architecture**: Minimize contention with lock-free data structures
3. **Cache-Optimized Layouts**: Columnar storage and CSR graphs for cache efficiency
4. **Zero-Copy Operations**: Memory-mapped storage with direct SIMD access
5. **Adaptive Indexing**: Automatic index selection based on query patterns
6. **Hybrid Execution**: Seamlessly combine vector similarity and graph traversal

---

## 2. Data Model

### 2.1 Core Entities

```rust
/// Node: Vertex in the graph with properties and optional embedding
pub struct Node {
    id: NodeId,                    // 64-bit ID (8 bytes)
    labels: SmallVec<[LabelId; 4]>, // Inline for ≤4 labels
    properties: PropertyMap,        // Key-value properties
    embedding: Option<Vec<f32>>,    // Optional vector embedding
    degree_out: u32,               // Outgoing edge count
    degree_in: u32,                // Incoming edge count
    edge_offset: u64,              // Offset in edge array
}

/// Edge: Directed binary relationship
pub struct Edge {
    id: EdgeId,                    // 64-bit ID
    src: NodeId,                   // Source node
    dst: NodeId,                   // Destination node
    label: LabelId,                // Edge type
    properties: PropertyMap,
    embedding: Option<Vec<f32>>,   // Optional edge embedding
}

/// Hyperedge: N-ary relationship connecting multiple nodes
pub struct Hyperedge {
    id: HyperedgeId,               // 64-bit ID
    nodes: Vec<NodeId>,            // Connected nodes (ordered)
    label: LabelId,                // Hyperedge type
    description: String,           // Natural language description
    properties: PropertyMap,
    embedding: Vec<f32>,           // Always embedded for semantic search
    confidence: f32,               // Relationship confidence [0,1]
}

/// Property: Strongly-typed property value
#[derive(Clone)]
pub enum PropertyValue {
    Null,
    Bool(bool),
    Int64(i64),
    Float64(f64),
    String(String),
    DateTime(i64),                 // Unix timestamp
    List(Vec<PropertyValue>),
    Map(HashMap<String, PropertyValue>),
}
```

### 2.2 Storage Layout

**Compressed Sparse Row (CSR) Format for Edges:**
```
Nodes:     [N0, N1, N2, N3, ...]
Offsets:   [0,  3,  5,  8,  ...]  # Edge start indices
Edges:     [E0, E1, E2, E3, E4, E5, E6, E7, ...]
           └─N0 edges─┘ └N1─┘ └──N2 edges──┘
```

**Benefits:**
- Cache-friendly sequential access
- SIMD-friendly: process multiple edges in parallel
- Low memory overhead: 8-16 bytes per edge
- Fast neighborhood queries: O(degree) with prefetching

### 2.3 Indexing Strategy

1. **Primary Indexes:**
   - Node ID → Node (B+ Tree or HashMap)
   - Edge ID → Edge (B+ Tree or HashMap)
   - Hyperedge ID → Hyperedge (B+ Tree or HashMap)

2. **Secondary Indexes:**
   - Label Index: LabelId → [NodeId] (Bitmap or RoaringBitmap)
   - Property Index: (Key, Value) → [NodeId] (B+ Tree)
   - Temporal Index: TimeRange → [HyperedgeId] (Interval Tree)
   - Vector Index: Embedding → [NodeId/EdgeId] (HNSW from ruvector-core)

3. **Specialized Indexes:**
   - Bipartite Index: Node ↔ Hyperedge (for hypergraph queries)
   - Geospatial Index: (Lat, Lon) → [NodeId] (R-Tree)
   - Full-Text Index: Text → [NodeId] (Inverted Index)

---

## 3. Module Structure

### 3.1 Crate Organization

```
ruvector-graph/
├── src/
│   ├── lib.rs                    # Public API and re-exports
│   │
│   ├── model/                    # Data model
│   │   ├── mod.rs
│   │   ├── node.rs               # Node definition
│   │   ├── edge.rs               # Edge definition
│   │   ├── hyperedge.rs          # Hyperedge definition
│   │   ├── property.rs           # Property types and maps
│   │   └── schema.rs             # Schema definitions and validation
│   │
│   ├── storage/                  # Storage layer
│   │   ├── mod.rs
│   │   ├── engine.rs             # Storage engine trait and orchestration
│   │   ├── csr.rs                # CSR graph storage
│   │   ├── columnar.rs           # Columnar property storage
│   │   ├── mmap.rs               # Memory-mapped file handling
│   │   ├── wal.rs                # Write-Ahead Log
│   │   └── snapshot.rs           # Snapshotting and recovery
│   │
│   ├── index/                    # Indexing layer
│   │   ├── mod.rs
│   │   ├── primary.rs            # Primary ID indexes
│   │   ├── label.rs              # Label indexes (bitmap)
│   │   ├── property.rs           # Property indexes (B+ tree)
│   │   ├── temporal.rs           # Temporal hyperedge indexes
│   │   ├── vector.rs             # Vector similarity indexes (HNSW)
│   │   ├── bipartite.rs          # Hypergraph bipartite indexes
│   │   └── adaptive.rs           # Adaptive index selection
│   │
│   ├── query/                    # Query processing
│   │   ├── mod.rs
│   │   ├── parser/               # Cypher parser
│   │   │   ├── mod.rs
│   │   │   ├── lexer.rs          # Tokenization
│   │   │   ├── ast.rs            # Abstract syntax tree
│   │   │   ├── cypher.rs         # Cypher grammar
│   │   │   └── extensions.rs    # RuVector extensions (vector ops)
│   │   │
│   │   ├── planner/              # Query planning and optimization
│   │   │   ├── mod.rs
│   │   │   ├── logical.rs        # Logical plan generation
│   │   │   ├── physical.rs       # Physical plan generation
│   │   │   ├── optimizer.rs      # Rule-based optimization
│   │   │   ├── cost_model.rs     # Cost-based optimization
│   │   │   └── statistics.rs     # Query statistics collector
│   │   │
│   │   └── executor/             # Query execution
│   │       ├── mod.rs
│   │       ├── engine.rs         # Execution engine
│   │       ├── operators.rs      # Physical operators (Scan, Join, etc.)
│   │       ├── simd_ops.rs       # SIMD-optimized operations
│   │       ├── parallel.rs       # Parallel execution (Rayon)
│   │       ├── vectorized.rs     # Vectorized execution (batch processing)
│   │       └── jit.rs            # JIT compilation for hot paths (optional)
│   │
│   ├── distributed/              # Distribution layer
│   │   ├── mod.rs
│   │   ├── consensus/            # Consensus protocol
│   │   │   ├── mod.rs
│   │   │   ├── raft_ext.rs       # Extended RAFT from ruvector-raft
│   │   │   ├── log.rs            # Distributed transaction log
│   │   │   └── snapshot.rs       # Distributed snapshots
│   │   │
│   │   ├── partitioning/         # Graph partitioning
│   │   │   ├── mod.rs
│   │   │   ├── hash.rs           # Hash partitioning
│   │   │   ├── range.rs          # Range partitioning
│   │   │   ├── metis.rs          # METIS-based graph partitioning
│   │   │   └── adaptive.rs       # Adaptive repartitioning
│   │   │
│   │   ├── replication/          # Replication
│   │   │   ├── mod.rs
│   │   │   ├── sync.rs           # Synchronous replication
│   │   │   ├── async.rs          # Asynchronous replication
│   │   │   └── consistency.rs    # Consistency guarantees
│   │   │
│   │   └── coordination/         # Cluster coordination
│   │       ├── mod.rs
│   │       ├── cluster.rs        # Cluster membership
│   │       ├── leader.rs         # Leader election
│   │       └── heartbeat.rs      # Health monitoring
│   │
│   ├── federation/               # Cross-cluster federation
│   │   ├── mod.rs
│   │   ├── router.rs             # Query routing across clusters
│   │   ├── merger.rs             # Result merging
│   │   ├── catalog.rs            # Global metadata catalog
│   │   ├── transaction.rs        # Cross-cluster transactions (2PC)
│   │   └── cache.rs              # Federation result cache
│   │
│   ├── hybrid/                   # Vector-Graph hybrid queries
│   │   ├── mod.rs
│   │   ├── vector_graph.rs       # Combined vector+graph operations
│   │   ├── semantic_search.rs    # Semantic graph search
│   │   ├── knn_traversal.rs      # KNN + graph traversal
│   │   └── rag_hypergraph.rs     # RAG with hypergraph (NeurIPS 2025)
│   │
│   ├── transaction/              # Transaction management
│   │   ├── mod.rs
│   │   ├── mvcc.rs               # Multi-Version Concurrency Control
│   │   ├── isolation.rs          # Isolation levels
│   │   └── recovery.rs           # Crash recovery
│   │
│   ├── util/                     # Utilities
│   │   ├── mod.rs
│   │   ├── bitmap.rs             # Roaring bitmaps
│   │   ├── cache.rs              # LRU/LFU caches
│   │   ├── metrics.rs            # Performance metrics
│   │   └── thread_pool.rs        # Custom thread pools
│   │
│   └── error.rs                  # Error types
│
├── benches/                      # Benchmarks
│   ├── query_bench.rs
│   ├── storage_bench.rs
│   └── distributed_bench.rs
│
├── tests/                        # Integration tests
│   ├── cypher_tests.rs
│   ├── distributed_tests.rs
│   └── hybrid_tests.rs
│
├── Cargo.toml
└── README.md
```

### 3.2 Key Traits and Interfaces

```rust
/// Storage engine interface
#[async_trait]
pub trait StorageEngine: Send + Sync {
    async fn insert_node(&self, node: Node) -> Result<NodeId>;
    async fn insert_edge(&self, edge: Edge) -> Result<EdgeId>;
    async fn insert_hyperedge(&self, hyperedge: Hyperedge) -> Result<HyperedgeId>;

    async fn get_node(&self, id: NodeId) -> Result<Option<Node>>;
    async fn get_edge(&self, id: EdgeId) -> Result<Option<Edge>>;
    async fn get_hyperedge(&self, id: HyperedgeId) -> Result<Option<Hyperedge>>;

    async fn neighbors(&self, node: NodeId, direction: Direction) -> Result<Vec<NodeId>>;
    async fn hyperedge_nodes(&self, hyperedge_id: HyperedgeId) -> Result<Vec<NodeId>>;

    async fn snapshot(&self) -> Result<Snapshot>;
    async fn restore(&self, snapshot: Snapshot) -> Result<()>;
}

/// Query executor interface
pub trait QueryExecutor: Send + Sync {
    fn execute(&self, plan: PhysicalPlan) -> Result<QueryResult>;
    fn explain(&self, plan: PhysicalPlan) -> Result<ExecutionPlan>;
    fn with_parallelism(&self, threads: usize) -> Self;
}

/// Partitioning strategy interface
pub trait PartitioningStrategy: Send + Sync {
    fn partition(&self, graph: &Graph, num_partitions: usize) -> Result<Vec<Partition>>;
    fn assign_node(&self, node_id: NodeId) -> PartitionId;
    fn assign_edge(&self, edge: &Edge) -> PartitionId;
    fn rebalance(&self, partitions: &[Partition]) -> Result<RebalancePlan>;
}

/// Federation interface
#[async_trait]
pub trait FederationLayer: Send + Sync {
    async fn route_query(&self, query: Query) -> Result<Vec<ClusterId>>;
    async fn execute_distributed(&self, query: Query) -> Result<QueryResult>;
    async fn merge_results(&self, results: Vec<PartialResult>) -> Result<QueryResult>;
}
```

---

## 4. Query Language and Execution

### 4.1 Cypher Compatibility

**Supported Cypher Features:**
- MATCH patterns: `MATCH (n:Person)-[:KNOWS]->(m:Person)`
- WHERE clauses: `WHERE n.age > 25`
- RETURN projections: `RETURN n.name, m.age`
- Aggregations: `COUNT`, `SUM`, `AVG`, `MIN`, `MAX`
- ORDER BY and LIMIT
- CREATE, DELETE, SET operations
- Path queries: `MATCH p = (n)-[*1..3]-(m)`
- Shortest path: `shortestPath((n)-[*]-(m))`

**RuVector Extensions:**
```cypher
// Vector similarity search
MATCH (n:Document)
WHERE vectorDistance(n.embedding, $query_vector) < 0.3
RETURN n

// Hybrid vector + graph query
MATCH (n:Paper)-[:CITES*1..2]->(m:Paper)
WHERE vectorDistance(n.embedding, $query_vector) < 0.2
RETURN m
ORDER BY vectorDistance(m.embedding, $query_vector)
LIMIT 10

// Hyperedge queries
MATCH (n:Gene)-[h:INTERACTION]-(m:Gene)-[h]-(p:Gene)
WHERE h.confidence > 0.8
RETURN n, m, p, h.description

// Temporal queries
MATCH (n:Event)
WHERE n.timestamp BETWEEN $start AND $end
RETURN n
```

### 4.2 Query Execution Pipeline

```
Cypher Query
    ↓
┌────────────────┐
│  Lexer/Parser  │ → AST
└────────┬───────┘
         ↓
┌────────────────┐
│ Logical Plan   │ → Relational algebra
└────────┬───────┘
         ↓
┌────────────────┐
│   Optimizer    │ → Rewrite rules, cost estimation
│ (Rule + Cost)  │
└────────┬───────┘
         ↓
┌────────────────┐
│ Physical Plan  │ → Executable operators
└────────┬───────┘
         ↓
┌────────────────┐
│ SIMD Executor  │ → Vectorized + parallel execution
└────────┬───────┘
         ↓
   Query Result
```

### 4.3 SIMD Optimization Examples

```rust
// SIMD-optimized node filtering by property
pub fn filter_nodes_simd(nodes: &[Node], min_age: i64) -> Vec<NodeId> {
    let mut result = Vec::new();

    // Process 4 nodes at a time with AVX2
    for chunk in nodes.chunks(4) {
        unsafe {
            let ages = _mm256_loadu_si256(chunk.as_ptr() as *const __m256i);
            let min_vec = _mm256_set1_epi64x(min_age);
            let mask = _mm256_cmpgt_epi64(ages, min_vec);

            // Collect matching indices
            let mask_bits = _mm256_movemask_pd(_mm256_castsi256_pd(mask));
            for i in 0..4 {
                if (mask_bits & (1 << i)) != 0 {
                    result.push(chunk[i].id);
                }
            }
        }
    }

    result
}

// SIMD-optimized edge traversal with filtering
pub fn traverse_edges_simd(
    edges: &[EdgeId],
    csr_offsets: &[u64],
    csr_edges: &[Edge],
    predicate: impl Fn(&Edge) -> bool
) -> Vec<NodeId> {
    // Batch prefetch edge data to L1 cache
    for &edge_id in edges.iter().step_by(8) {
        let offset = csr_offsets[edge_id as usize];
        _mm_prefetch(csr_edges.as_ptr().add(offset as usize), _MM_HINT_T0);
    }

    // Process edges with SIMD filtering
    // ... vectorized filtering logic ...
}
```

---

## 5. Distributed Architecture

### 5.1 Consensus and Replication

**Extended RAFT Protocol:**

```rust
/// Graph-specific RAFT extension
pub struct GraphRaft {
    raft: RaftNode,  // From ruvector-raft
    graph_state: Arc<RwLock<GraphState>>,
    partition_id: PartitionId,
}

/// Graph state machine for RAFT
impl StateMachine for GraphState {
    fn apply(&mut self, log_entry: LogEntry) -> Result<Response> {
        match log_entry.operation {
            Operation::InsertNode(node) => self.insert_node(node),
            Operation::InsertEdge(edge) => self.insert_edge(edge),
            Operation::DeleteNode(id) => self.delete_node(id),
            Operation::UpdateProperty(id, key, value) => {
                self.update_property(id, key, value)
            }
            // ... more operations
        }
    }

    fn snapshot(&self) -> Result<Snapshot> {
        // Create consistent snapshot of partition
        self.storage.snapshot()
    }
}
```

**Consistency Guarantees:**
- **Strong consistency** within a partition (RAFT consensus)
- **Eventual consistency** across partitions (async replication)
- **Causal consistency** for federated queries (vector clocks)

### 5.2 Graph Partitioning

**Multi-Strategy Partitioning:**

1. **Hash Partitioning** (default):
   ```rust
   partition_id = hash(node_id) % num_partitions
   ```
   - Pros: Uniform distribution, no hotspots
   - Cons: High edge cuts, poor locality

2. **METIS Partitioning** (optimized):
   ```rust
   // Minimize edge cuts using METIS algorithm
   partitions = metis::partition_graph(graph, num_partitions, minimize_edge_cuts)
   ```
   - Pros: Minimal edge cuts, better query locality
   - Cons: Expensive rebalancing, static partitioning

3. **Adaptive Partitioning** (recommended):
   ```rust
   // Monitor query patterns and repartition hot nodes
   if query_span_ratio > threshold {
       adaptive_repartition(hot_nodes, target_partitions);
   }
   ```
   - Pros: Query-aware, dynamic optimization
   - Cons: Rebalancing overhead

**Edge Placement:**
- **Source-based**: Edge stored at source node's partition
- **Destination-based**: Edge stored at destination node's partition
- **Replicated**: Edge stored at both partitions (optimizes bidirectional queries)

### 5.3 Distributed Query Execution

```rust
/// Distributed query executor
pub struct DistributedExecutor {
    local_executor: LocalExecutor,
    cluster: ClusterCoordinator,
    partitions: Vec<PartitionInfo>,
}

impl DistributedExecutor {
    pub async fn execute(&self, query: Query) -> Result<QueryResult> {
        // 1. Analyze query and determine affected partitions
        let affected = self.analyze_partitions(&query)?;

        // 2. Push down predicates to partition level
        let local_queries = self.pushdown_predicates(&query, &affected)?;

        // 3. Execute in parallel across partitions
        let futures: Vec<_> = local_queries
            .into_iter()
            .map(|(partition_id, local_query)| {
                let executor = self.get_partition_executor(partition_id);
                async move { executor.execute(local_query).await }
            })
            .collect();

        let results = futures::future::join_all(futures).await;

        // 4. Merge and post-process results
        self.merge_results(results)
    }
}
```

---

## 6. Federation Layer

### 6.1 Cross-Cluster Architecture

```
┌─────────────────────────────────────────────────────┐
│             Global Federation Coordinator            │
│  ┌────────────┐  ┌───────────┐  ┌──────────────┐   │
│  │  Catalog   │  │  Router   │  │  Transaction │   │
│  │  (Metadata)│  │           │  │  Coordinator │   │
│  └────────────┘  └───────────┘  └──────────────┘   │
└──────────┬──────────────┬──────────────┬───────────┘
           │              │              │
    ┌──────┴──────┐  ┌────┴─────┐  ┌────┴─────┐
    │  Cluster A  │  │ Cluster B │  │ Cluster C│
    │  (RAFT)     │  │  (RAFT)   │  │  (RAFT)  │
    └─────────────┘  └───────────┘  └──────────┘
```

### 6.2 Federation Catalog

```rust
/// Global metadata catalog for federation
pub struct FederationCatalog {
    /// Cluster registry: cluster_id → cluster_info
    clusters: DashMap<ClusterId, ClusterInfo>,

    /// Graph schema: graph_name → schema
    schemas: DashMap<String, GraphSchema>,

    /// Partition mapping: node_id → (cluster_id, partition_id)
    node_index: DashMap<NodeId, (ClusterId, PartitionId)>,

    /// Label distribution: label → [cluster_ids]
    label_index: DashMap<LabelId, Vec<ClusterId>>,
}

impl FederationCatalog {
    /// Route query to appropriate clusters
    pub fn route(&self, query: &Query) -> Vec<ClusterId> {
        // Analyze query patterns
        let labels = query.extract_labels();
        let node_ids = query.extract_node_ids();

        // Determine target clusters
        let mut targets = HashSet::new();

        // Route by explicit node IDs
        for node_id in node_ids {
            if let Some((cluster_id, _)) = self.node_index.get(&node_id) {
                targets.insert(*cluster_id);
            }
        }

        // Route by labels (if no explicit IDs)
        if targets.is_empty() {
            for label in labels {
                if let Some(clusters) = self.label_index.get(&label) {
                    targets.extend(clusters.iter());
                }
            }
        }

        // Fallback: broadcast to all clusters
        if targets.is_empty() {
            targets.extend(self.clusters.iter().map(|e| *e.key()));
        }

        targets.into_iter().collect()
    }
}
```

### 6.3 Cross-Cluster Transactions

**Two-Phase Commit (2PC) for Strong Consistency:**

```rust
pub struct FederatedTransaction {
    tx_id: TransactionId,
    coordinator: ClusterId,
    participants: Vec<ClusterId>,
    state: TransactionState,
}

impl FederatedTransaction {
    pub async fn commit(&mut self) -> Result<()> {
        // Phase 1: Prepare
        let prepare_futures: Vec<_> = self.participants
            .iter()
            .map(|cluster| self.send_prepare(*cluster))
            .collect();

        let votes = futures::future::join_all(prepare_futures).await;

        // Check if all voted YES
        if votes.iter().all(|v| v.is_ok()) {
            // Phase 2: Commit
            let commit_futures: Vec<_> = self.participants
                .iter()
                .map(|cluster| self.send_commit(*cluster))
                .collect();

            futures::future::join_all(commit_futures).await;
            Ok(())
        } else {
            // Abort if any voted NO
            self.abort().await
        }
    }
}
```

---

## 7. Hybrid Vector-Graph Queries

### 7.1 Integration with RuVector Core

```rust
/// Hybrid index combining HNSW (vectors) and CSR (graph)
pub struct HybridIndex {
    /// Vector index from ruvector-core
    vector_index: HnswIndex,

    /// Graph index (CSR)
    graph_index: CsrGraph,

    /// Mapping: node_id → vector_id
    node_to_vector: HashMap<NodeId, VectorId>,

    /// Mapping: vector_id → node_id
    vector_to_node: HashMap<VectorId, NodeId>,
}

impl HybridIndex {
    /// Semantic graph search: find nodes similar to query, then traverse graph
    pub fn semantic_search_with_traversal(
        &self,
        query_vector: &[f32],
        k_similar: usize,
        max_hops: usize,
    ) -> Result<Vec<NodeId>> {
        // 1. Vector similarity search (HNSW)
        let similar_vectors = self.vector_index.search(query_vector, k_similar)?;

        // 2. Convert to node IDs
        let seed_nodes: Vec<NodeId> = similar_vectors
            .into_iter()
            .filter_map(|(vec_id, _dist)| self.vector_to_node.get(&vec_id).copied())
            .collect();

        // 3. Graph traversal from seed nodes
        let mut visited = HashSet::new();
        let mut result = Vec::new();

        for seed in seed_nodes {
            let neighbors = self.graph_index.k_hop_neighbors(seed, max_hops)?;
            for neighbor in neighbors {
                if visited.insert(neighbor) {
                    result.push(neighbor);
                }
            }
        }

        Ok(result)
    }

    /// Traverse graph with vector similarity filtering
    pub fn graph_traversal_with_vector_filter(
        &self,
        start_node: NodeId,
        max_hops: usize,
        similarity_threshold: f32,
        query_vector: &[f32],
    ) -> Result<Vec<NodeId>> {
        let mut visited = HashSet::new();
        let mut frontier = vec![start_node];
        let mut result = Vec::new();

        for _hop in 0..max_hops {
            let mut next_frontier = Vec::new();

            for &node in &frontier {
                if !visited.insert(node) {
                    continue;
                }

                // Check vector similarity filter
                if let Some(&vector_id) = self.node_to_vector.get(&node) {
                    let distance = self.vector_index.distance_to(vector_id, query_vector)?;
                    if distance < similarity_threshold {
                        result.push(node);

                        // Expand to neighbors
                        let neighbors = self.graph_index.neighbors(node)?;
                        next_frontier.extend(neighbors);
                    }
                }
            }

            frontier = next_frontier;
        }

        Ok(result)
    }
}
```

### 7.2 RAG-Hypergraph Integration

**Based on HyperGraphRAG (NeurIPS 2025):**

```rust
/// RAG with hypergraph for multi-entity retrieval
pub struct RagHypergraph {
    hypergraph: HypergraphIndex,
    vector_index: HnswIndex,
    llm_client: LlmClient,
}

impl RagHypergraph {
    pub async fn query(&self, question: &str) -> Result<String> {
        // 1. Embed question
        let query_embedding = self.llm_client.embed(question).await?;

        // 2. Find similar hyperedges (multi-entity relationships)
        let similar_hyperedges = self.hypergraph.search_hyperedges(
            &query_embedding,
            k = 10
        );

        // 3. Extract relevant entity clusters
        let mut context_nodes = HashSet::new();
        for (hyperedge_id, _score) in similar_hyperedges {
            let hyperedge = self.hypergraph.get_hyperedge(&hyperedge_id)?;
            context_nodes.extend(hyperedge.nodes.iter().cloned());
        }

        // 4. Build context from nodes and hyperedges
        let context = self.build_context(&context_nodes)?;

        // 5. Generate answer with LLM
        let answer = self.llm_client.generate(&context, question).await?;

        Ok(answer)
    }
}
```

---

## 8. Performance Optimizations

### 8.1 SIMD Optimizations

**Target Operations:**
- Node filtering: 4-8x speedup with AVX2/AVX-512
- Edge traversal: 3-6x speedup with prefetching + SIMD
- Property comparison: 5-10x speedup for numeric properties
- Vector distance: 8-16x speedup with SIMD distance functions

**Implementation:**
```rust
#[cfg(target_arch = "x86_64")]
mod simd_x86 {
    use std::arch::x86_64::*;

    /// SIMD-optimized edge filtering
    pub unsafe fn filter_edges_avx2(
        edges: &[Edge],
        min_weight: f32
    ) -> Vec<EdgeId> {
        let min_vec = _mm256_set1_ps(min_weight);
        let mut result = Vec::new();

        for chunk in edges.chunks(8) {
            // Load 8 edge weights
            let weights = _mm256_loadu_ps(chunk.as_ptr() as *const f32);

            // Compare: weight >= min_weight
            let mask = _mm256_cmp_ps(weights, min_vec, _CMP_GE_OQ);
            let mask_bits = _mm256_movemask_ps(mask);

            // Collect matching edges
            for i in 0..8 {
                if (mask_bits & (1 << i)) != 0 {
                    result.push(chunk[i].id);
                }
            }
        }

        result
    }
}
```

### 8.2 Cache Optimization

**Strategies:**
1. **Columnar Storage**: Store properties column-wise for SIMD access
2. **Cache-Friendly Layouts**: Align data structures to cache lines (64 bytes)
3. **Prefetching**: Software prefetch for predictable access patterns
4. **Blocking**: Process data in cache-sized blocks

```rust
/// Cache-optimized node storage (columnar)
pub struct ColumnarNodes {
    ids: Vec<NodeId>,              // Column 0
    labels: Vec<LabelId>,          // Column 1
    properties: PropertyColumns,   // Columns 2..N

    // Layout guarantees:
    // - Each column is cache-aligned
    // - Nodes with same ID are at same index across columns
}

impl ColumnarNodes {
    /// SIMD-friendly filtering by label
    pub fn filter_by_label(&self, target_label: LabelId) -> Vec<usize> {
        let mut indices = Vec::new();

        // Process labels in cache-line blocks (64 bytes = 16 labels)
        for (block_idx, chunk) in self.labels.chunks(16).enumerate() {
            // Prefetch next block
            if block_idx + 1 < self.labels.len() / 16 {
                let next = &self.labels[(block_idx + 1) * 16];
                unsafe { _mm_prefetch(next.as_ptr() as *const i8, _MM_HINT_T0); }
            }

            // SIMD comparison
            for (i, &label) in chunk.iter().enumerate() {
                if label == target_label {
                    indices.push(block_idx * 16 + i);
                }
            }
        }

        indices
    }
}
```

### 8.3 Parallel Execution

**Rayon-based Parallelism:**
```rust
use rayon::prelude::*;

/// Parallel query execution
pub fn execute_pattern_match_parallel(
    pattern: &Pattern,
    graph: &Graph,
    num_threads: usize,
) -> Vec<Match> {
    // Set thread pool
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .unwrap();

    // Parallel pattern matching
    graph.nodes()
        .par_iter()
        .filter_map(|node| {
            // Try to match pattern starting from this node
            pattern.match_from(node, graph)
        })
        .collect()
}
```

### 8.4 Lock-Free Data Structures

```rust
use crossbeam::queue::SegQueue;
use dashmap::DashMap;

/// Lock-free graph updates
pub struct LockFreeGraph {
    nodes: DashMap<NodeId, Node>,
    edges: DashMap<EdgeId, Edge>,

    // Lock-free queues for batch operations
    pending_inserts: SegQueue<GraphOp>,
    pending_deletes: SegQueue<GraphOp>,
}

impl LockFreeGraph {
    pub fn insert_node(&self, node: Node) -> NodeId {
        let id = node.id;
        self.nodes.insert(id, node);
        id
    }

    pub fn batch_insert(&self, nodes: Vec<Node>) {
        nodes.into_par_iter().for_each(|node| {
            self.insert_node(node);
        });
    }
}
```

---

## 9. Benchmarking and Performance Targets

### 9.1 Micro-Benchmarks

**Node Operations:**
- Insert node: < 1 μs
- Get node by ID: < 100 ns (cached), < 1 μs (uncached)
- Filter nodes by property: < 10 μs per 1K nodes (SIMD)

**Edge Operations:**
- Insert edge: < 1 μs
- Get neighbors: < 1 μs for degree < 1000
- 2-hop traversal: < 10 μs for degree < 100

**Query Operations:**
- Simple MATCH: < 100 μs
- 3-hop pattern match: < 1 ms
- Aggregation (COUNT): < 10 ms on 1M nodes

**Vector Operations:**
- KNN search (k=10): < 1 ms on 1M vectors
- Hybrid vector+graph: < 5 ms

### 9.2 Macro-Benchmarks

**Graph Loading:**
- 1M nodes + 10M edges: < 10 seconds
- 10M nodes + 100M edges: < 2 minutes

**Complex Queries:**
- PageRank (10 iterations): < 5 seconds on 1M nodes
- Shortest path: < 10 ms on 1M nodes
- Community detection (Louvain): < 30 seconds on 1M nodes

**Distributed Performance:**
- 3-way replication latency: < 10 ms (within DC)
- Cross-cluster query: < 50 ms (single DC), < 200 ms (cross-DC)

### 9.3 Comparison with Neo4j

| Operation | Neo4j (est.) | RuVector Graph (target) | Speedup |
|-----------|--------------|-------------------------|---------|
| Node insert | ~10 μs | ~1 μs | 10x |
| 2-hop traversal | ~1 ms | ~10 μs | 100x |
| Property filter | ~100 ms | ~1 ms (SIMD) | 100x |
| PageRank (1M) | ~60 s | ~5 s (parallel) | 12x |
| KNN + graph | N/A | ~5 ms (hybrid) | - |

---

## 10. Deployment Architectures

### 10.1 Single-Node Deployment

```
┌────────────────────────────┐
│     RuVector Graph Node    │
│                            │
│  ┌──────────────────────┐  │
│  │   Query Coordinator  │  │
│  └──────────┬───────────┘  │
│             │              │
│  ┌──────────▼───────────┐  │
│  │   Storage Engine     │  │
│  │   (Memory-Mapped)    │  │
│  └──────────────────────┘  │
│                            │
│  Capacity: 10M-100M nodes  │
└────────────────────────────┘
```

### 10.2 Distributed Cluster

```
┌─────────────────────────────────────────────────┐
│              Load Balancer                      │
└──────────┬──────────────┬──────────────┬───────┘
           │              │              │
     ┌─────▼─────┐  ┌─────▼─────┐  ┌─────▼─────┐
     │ Partition │  │ Partition │  │ Partition │
     │     0     │  │     1     │  │     2     │
     │           │  │           │  │           │
     │ RAFT (3)  │  │ RAFT (3)  │  │ RAFT (3)  │
     └───────────┘  └───────────┘  └───────────┘

Capacity: 100M-1B nodes (3-way partitioning)
```

### 10.3 Federated Multi-Cluster

```
        ┌────────────────────────────┐
        │  Federation Coordinator    │
        └──────┬──────────┬──────────┘
               │          │
        ┌──────▼──────┐   └──────▼──────┐
        │  Cluster A  │   │  Cluster B  │
        │  (US-East)  │   │  (EU-West)  │
        │             │   │             │
        │  10 nodes   │   │  10 nodes   │
        │  300M graph │   │  200M graph │
        └─────────────┘   └─────────────┘

Total Capacity: 500M+ nodes
Cross-cluster latency: 100-200ms
```

---

## 11. Roadmap and Future Work

### Phase 1: Foundation (Months 1-3)
- ✅ Core data model and storage engine
- ✅ Cypher parser and basic query execution
- ✅ SIMD-optimized operators
- ✅ Single-node deployment

### Phase 2: Distribution (Months 4-6)
- ✅ RAFT integration from ruvector-raft
- ✅ Graph partitioning (hash + METIS)
- ✅ Distributed query execution
- ✅ Multi-node cluster deployment

### Phase 3: Federation (Months 7-9)
- Cross-cluster query routing
- Global metadata catalog
- 2PC transactions
- Federation benchmarks

### Phase 4: Hybrid Optimization (Months 10-12)
- HNSW integration from ruvector-core
- Semantic graph search
- RAG-Hypergraph implementation
- Hybrid query benchmarks

### Phase 5: Production Hardening (Months 13-15)
- JIT compilation for hot query paths
- Adaptive indexing and caching
- Advanced monitoring and observability
- Production deployment guides

### Future Enhancements
- GPU acceleration for graph algorithms (CUDA/ROCm)
- Temporal graph support (time-varying graphs)
- GNN (Graph Neural Network) integration
- Streaming graph updates
- Cloud-native deployment (Kubernetes operators)

---

## 12. References and Inspiration

### Academic Papers
1. **HyperGraphRAG** - NeurIPS 2025 (hypergraph for RAG)
2. **METIS** - Graph partitioning algorithms
3. **Raft Consensus** - In Search of an Understandable Consensus Algorithm

### Systems
1. **Neo4j** - Cypher query language, property graph model
2. **JanusGraph** - Distributed graph database architecture
3. **DuckDB** - Vectorized query execution patterns
4. **ClickHouse** - Columnar storage and SIMD optimization

### RuVector Components
- `ruvector-core` - HNSW indexing, SIMD distance functions
- `ruvector-raft` - Distributed consensus
- `ruvector-storage` - Memory-mapped storage patterns

---

## 13. Conclusion

RuVector Graph represents a next-generation distributed hypergraph database that combines:

1. **Performance**: SIMD optimization, lock-free structures, cache-friendly layouts
2. **Scalability**: Distributed RAFT consensus, intelligent partitioning, federation
3. **Flexibility**: Neo4j-compatible Cypher + vector extensions
4. **Innovation**: Hyperedge support, hybrid vector-graph queries, RAG integration

**Key Differentiators:**
- **10-100x faster** than Neo4j for most operations
- **Native hypergraph** support for n-ary relationships
- **Seamless vector integration** with ruvector-core
- **Production-ready distribution** with RAFT consensus

This architecture provides a solid foundation for building the fastest graph database in the Rust ecosystem while maintaining compatibility with industry-standard query languages and patterns.
