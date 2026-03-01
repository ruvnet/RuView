# System Architecture: Real-Time Graph Monitoring with Subpolynomial-Time Dynamic Minimum Cut

**Document Version:** 1.0
**Date:** 2025-12-21
**Status:** Draft
**Author:** System Architecture Designer

---

## 1. Executive Summary

This document defines the system architecture for `ruvector-mincut`, a new crate implementing state-of-the-art dynamic minimum cut algorithms with subpolynomial update time n^{o(1)}. The crate will provide real-time graph monitoring capabilities with efficient support for edge insertions and deletions, both exact and (1+ε)-approximate minimum cuts, and WASM compatibility for browser-based applications.

### Key Architecture Decisions

| Decision | Rationale |
|----------|-----------|
| Separate crate `ruvector-mincut` | Clear separation of concerns, optional dependency for ruvector-graph |
| Hierarchical decomposition as primary structure | Enables subpolynomial update time via expander decomposition |
| Link-cut trees for dynamic connectivity | Industry-standard dynamic tree structure with O(log n) operations |
| Event-driven monitoring system | Non-blocking, real-time alerts without polling overhead |
| Dual API (exact + approximate) | Flexibility for performance vs accuracy trade-offs |
| WASM-first design | Enable browser visualization and distributed monitoring |

---

## 2. Module Architecture

### 2.1 Crate Structure

```
ruvector-mincut/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs                    # Public API and re-exports
│   ├── error.rs                  # Error types
│   ├── types.rs                  # Core type definitions
│   │
│   ├── core/                     # Core algorithm implementations
│   │   ├── mod.rs
│   │   ├── mincut.rs            # Main MinCut struct and algorithms
│   │   ├── exact.rs             # Exact minimum cut (n^{o(1)} update)
│   │   ├── approximate.rs       # (1+ε)-approximate minimum cut
│   │   └── query.rs             # Query operations (value, edges, witnesses)
│   │
│   ├── data_structures/          # Advanced data structures
│   │   ├── mod.rs
│   │   ├── link_cut_tree.rs     # Link-cut tree (dynamic connectivity)
│   │   ├── euler_tour_tree.rs   # Euler tour tree (alternative)
│   │   ├── hierarchical.rs      # Hierarchical graph decomposition
│   │   ├── expander.rs          # Expander decomposition
│   │   ├── edge_index.rs        # Efficient edge indexing
│   │   └── cache.rs             # LRU cache for cut values
│   │
│   ├── monitoring/               # Real-time monitoring system
│   │   ├── mod.rs
│   │   ├── monitor.rs           # Main monitoring coordinator
│   │   ├── alerts.rs            # Alert generation and dispatch
│   │   ├── thresholds.rs        # Threshold management
│   │   ├── callbacks.rs         # Callback registry and execution
│   │   └── metrics.rs           # Performance metrics collection
│   │
│   ├── integration/              # Integration with ruvector-graph
│   │   ├── mod.rs
│   │   ├── adapter.rs           # Adapter for GraphDB
│   │   ├── sync.rs              # Synchronization with graph updates
│   │   └── storage.rs           # Persistent storage integration
│   │
│   ├── concurrency/              # Concurrency and parallelism
│   │   ├── mod.rs
│   │   ├── rwlock.rs            # Custom RwLock with priority
│   │   ├── parallel.rs          # Parallel cut computation
│   │   ├── batch.rs             # Batch update processing
│   │   └── snapshot.rs          # Snapshot isolation for queries
│   │
│   ├── wasm/                     # WASM bindings and browser support
│   │   ├── mod.rs
│   │   ├── bindings.rs          # wasm-bindgen exports
│   │   ├── worker.rs            # Web Worker support
│   │   └── visualization.rs     # Visualization helpers
│   │
│   └── utils/                    # Utilities
│       ├── mod.rs
│       ├── graph_utils.rs       # Graph helper functions
│       ├── random.rs            # Randomized algorithms utilities
│       └── benchmark.rs         # Benchmarking utilities
│
├── benches/
│   ├── update_performance.rs    # Update time benchmarks
│   ├── query_performance.rs     # Query time benchmarks
│   └── memory_usage.rs          # Memory profiling
│
├── tests/
│   ├── integration_tests.rs     # Integration tests
│   ├── correctness.rs           # Correctness verification
│   └── stress_tests.rs          # Stress testing
│
└── examples/
    ├── basic_usage.rs           # Basic API usage
    ├── real_time_monitor.rs     # Real-time monitoring example
    ├── wasm_demo.rs             # WASM demo (browser)
    └── distributed_graph.rs     # Integration with distributed graph
```

### 2.2 Module Dependency Graph

```
┌─────────────────────────────────────────────────────────────┐
│                        Public API                            │
│                      (lib.rs, types.rs)                      │
└─────────────────────────┬───────────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
┌─────────▼──────┐ ┌─────▼─────┐ ┌──────▼─────────┐
│ Core Algorithms│ │ Monitoring │ │  Integration   │
│  (exact/approx)│ │  (alerts)  │ │ (ruvector-graph│
└─────────┬──────┘ └─────┬─────┘ └──────┬─────────┘
          │               │               │
          │       ┌───────▼───────┐       │
          │       │  Concurrency  │       │
          │       │ (rwlock/batch)│       │
          │       └───────┬───────┘       │
          │               │               │
┌─────────▼───────────────▼───────────────▼─────────┐
│            Data Structures Layer                   │
│  (link-cut tree, hierarchical, expander, cache)    │
└─────────────────────┬──────────────────────────────┘
                      │
          ┌───────────┼───────────┐
          │           │           │
    ┌─────▼────┐ ┌───▼────┐ ┌───▼────┐
    │   WASM   │ │ Utils  │ │Storage │
    └──────────┘ └────────┘ └────────┘
```

---

## 3. Core Data Structures Design

### 3.1 Link-Cut Tree

**Purpose:** Maintain dynamic connectivity and support cut/link operations in O(log n) time.

**Implementation Strategy:**
```rust
pub struct LinkCutTree {
    nodes: Vec<LCTNode>,
    forest_roots: Vec<NodeId>,
}

struct LCTNode {
    parent: Option<NodeId>,
    left: Option<NodeId>,
    right: Option<NodeId>,
    path_parent: Option<NodeId>,  // Virtual edge for tree connectivity
    subtree_size: usize,
    weight: f64,                   // Edge weight in minimum cut
    reversed: bool,                // Lazy reversal flag
}
```

**Key Operations:**
- `link(u, v, weight)` - O(log n) - Connect nodes u and v
- `cut(u, v)` - O(log n) - Disconnect edge (u,v)
- `connected(u, v)` - O(log n) - Check connectivity
- `find_root(u)` - O(log n) - Find tree root containing u
- `path_aggregate(u, v)` - O(log n) - Aggregate weights on path

**Concurrency:** Read operations use shared locks, structural modifications use exclusive locks.

### 3.2 Hierarchical Graph Decomposition

**Purpose:** Decompose graph into hierarchy of expanders to achieve subpolynomial update time.

**Structure:**
```rust
pub struct HierarchicalDecomposition {
    levels: Vec<DecompositionLevel>,
    level_map: HashMap<NodeId, usize>,  // Node to level mapping
    connectivity_oracle: LinkCutTree,
}

struct DecompositionLevel {
    graph: SparseGraph,              // Subgraph at this level
    expander_components: Vec<Expander>,
    inter_level_edges: EdgeSet,      // Edges to higher/lower levels
    cut_value_cache: LruCache<(NodeId, NodeId), f64>,
}

struct Expander {
    nodes: Vec<NodeId>,
    conductance: f64,                // φ(G) >= threshold
    internal_edges: EdgeSet,
    boundary_edges: EdgeSet,
}
```

**Decomposition Strategy:**
1. **Level 0:** Original graph
2. **Level k:** Contract expanders from level k-1
3. **Stop:** When graph has O(polylog n) vertices

**Update Algorithm:**
```
UPDATE(edge e, operation):
  1. Identify affected levels (typically O(polylog n))
  2. Update local structures at each level
  3. Recompute expanders if conductance drops
  4. Propagate changes up hierarchy
  Total: O(n^{o(1)}) amortized time
```

### 3.3 Efficient Edge Indexing

**Purpose:** Support O(1) edge lookup and efficient iteration.

```rust
pub struct EdgeIndex {
    edges: Vec<Edge>,
    edge_map: HashMap<(NodeId, NodeId), EdgeId>,
    adjacency: Vec<Vec<EdgeId>>,     // Adjacency lists
    degree: Vec<usize>,
    total_edges: AtomicUsize,
}

impl EdgeIndex {
    pub fn insert(&mut self, u: NodeId, v: NodeId, weight: f64) -> EdgeId;
    pub fn remove(&mut self, edge_id: EdgeId) -> Option<Edge>;
    pub fn get(&self, u: NodeId, v: NodeId) -> Option<&Edge>;
    pub fn neighbors(&self, u: NodeId) -> &[EdgeId];
    pub fn degree(&self, u: NodeId) -> usize;
}
```

**Memory Layout:**
- Edges stored in dense vector for cache efficiency
- HashMap for O(1) lookup by vertex pair
- Adjacency lists for iteration
- Atomic counters for lock-free statistics

---

## 4. Algorithm Design

### 4.1 Exact Dynamic Minimum Cut

**Algorithm:** Based on Nanongkai-Saranurak-Yingchareonthawornchai (2022)

**Core Invariant:** Maintain hierarchical decomposition where:
- Each level has φ-expander decomposition (φ = 1/polylog n)
- Minimum cut is either internal to an expander or crosses levels

**Update Time:** O(n^{o(1)}) amortized per edge insertion/deletion

**Implementation:**
```rust
pub struct ExactMinCut {
    decomposition: HierarchicalDecomposition,
    cut_value: AtomicF64,              // Current minimum cut value
    cut_edges: RwLock<Vec<EdgeId>>,    // Edges in minimum cut
    update_count: AtomicUsize,
    rebuild_threshold: usize,           // Trigger full rebuild
}

impl ExactMinCut {
    pub fn insert_edge(&mut self, u: NodeId, v: NodeId, weight: f64) -> Result<()> {
        // 1. Add edge to appropriate level(s)
        let levels = self.decomposition.find_levels(u, v);
        for level in levels {
            level.graph.add_edge(u, v, weight);
        }

        // 2. Update link-cut tree
        if !self.decomposition.connectivity_oracle.connected(u, v) {
            self.decomposition.connectivity_oracle.link(u, v, weight);
        }

        // 3. Check if expander properties violated
        self.rebalance_if_needed()?;

        // 4. Recompute cut value if necessary
        self.update_cut_value()?;

        Ok(())
    }

    pub fn delete_edge(&mut self, edge_id: EdgeId) -> Result<()> {
        // Similar structure to insert_edge
        // Handle tree edge deletion with replacement
    }

    pub fn query_cut_value(&self) -> f64 {
        self.cut_value.load(Ordering::Acquire)
    }

    pub fn query_cut_edges(&self) -> Vec<EdgeId> {
        self.cut_edges.read().clone()
    }
}
```

### 4.2 Approximate Minimum Cut

**Algorithm:** Karger-Stein randomized algorithm with dynamic maintenance

**Guarantee:** Returns (1+ε)-approximation with high probability

**Update Time:** O(polylog n) per update (faster than exact)

**Space-Time Tradeoff:**
```rust
pub struct ApproximateMinCut {
    epsilon: f64,                      // Approximation factor
    confidence: f64,                   // Success probability (1-δ)
    num_samples: usize,                // O(log n / ε²)

    samples: Vec<ContractedGraph>,     // Karger contractions
    best_cut: AtomicF64,
    best_cut_edges: RwLock<Vec<EdgeId>>,
}

impl ApproximateMinCut {
    pub fn new(epsilon: f64, confidence: f64) -> Self {
        let num_samples = Self::compute_samples(epsilon, confidence);
        // Initialize Karger samples
    }

    pub fn update(&mut self, edge: EdgeUpdate) -> Result<()> {
        // Update all samples in parallel
        self.samples.par_iter_mut()
            .for_each(|sample| sample.apply_update(edge));

        // Find best cut among samples
        self.update_best_cut();
    }
}
```

---

## 5. Concurrency Model

### 5.1 Read-Write Locking Strategy

**Requirements:**
- Concurrent queries should not block each other
- Updates should not block queries (use snapshot isolation)
- Updates are serialized (for correctness)

**Design:**
```rust
pub struct ConcurrentMinCut<T> {
    current: Arc<RwLock<T>>,           // Current version
    snapshot: Arc<RwLock<T>>,          // Snapshot for queries
    update_lock: Mutex<()>,            // Serializes updates
    version: AtomicUsize,              // Version counter
}

impl<T: Clone> ConcurrentMinCut<T> {
    pub fn query<F, R>(&self, f: F) -> R
    where F: FnOnce(&T) -> R
    {
        let snapshot = self.snapshot.read().unwrap();
        f(&snapshot)
    }

    pub fn update<F>(&self, f: F) -> Result<()>
    where F: FnOnce(&mut T) -> Result<()>
    {
        let _guard = self.update_lock.lock().unwrap();

        // Apply update to current
        {
            let mut current = self.current.write().unwrap();
            f(&mut current)?;
        }

        // Update snapshot asynchronously
        self.refresh_snapshot();

        Ok(())
    }

    fn refresh_snapshot(&self) {
        let current = self.current.read().unwrap();
        let mut snapshot = self.snapshot.write().unwrap();
        *snapshot = current.clone();
        self.version.fetch_add(1, Ordering::Release);
    }
}
```

### 5.2 Parallel Cut Computation

**Strategy:** Parallelize across decomposition levels and components

```rust
pub fn compute_cut_parallel(&self) -> (f64, Vec<EdgeId>) {
    use rayon::prelude::*;

    // Compute cut for each level in parallel
    let level_cuts: Vec<_> = self.decomposition.levels
        .par_iter()
        .map(|level| level.compute_local_cut())
        .collect();

    // Find global minimum
    level_cuts.into_iter()
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        .unwrap()
}
```

### 5.3 Batch Update Processing

**Optimization:** Process multiple edge updates together to amortize overhead

```rust
pub struct BatchProcessor {
    batch: Vec<EdgeUpdate>,
    batch_size: usize,
    auto_flush: bool,
}

impl BatchProcessor {
    pub fn add_update(&mut self, update: EdgeUpdate) -> Result<Option<BatchResult>> {
        self.batch.push(update);

        if self.batch.len() >= self.batch_size {
            self.flush()
        } else {
            Ok(None)
        }
    }

    pub fn flush(&mut self) -> Result<BatchResult> {
        // Sort updates for better cache locality
        self.batch.sort_by_key(|u| (u.level, u.component));

        // Apply in one pass
        let result = self.apply_batch(&self.batch)?;
        self.batch.clear();

        Ok(result)
    }
}
```

---

## 6. Monitoring System Design

### 6.1 Event-Driven Architecture

**Components:**
1. **Monitor Core:** Tracks cut value and triggers alerts
2. **Alert Dispatcher:** Routes alerts to registered callbacks
3. **Threshold Manager:** Manages multiple thresholds and conditions
4. **Metrics Collector:** Gathers performance statistics

```rust
pub struct MinCutMonitor {
    mincut: Arc<ConcurrentMinCut<ExactMinCut>>,
    thresholds: Vec<Threshold>,
    callbacks: CallbackRegistry,
    metrics: Arc<MetricsCollector>,
    monitor_thread: Option<JoinHandle<()>>,
}

#[derive(Clone)]
pub struct Threshold {
    pub value: f64,
    pub condition: ThresholdCondition,  // Below, Above, Changed
    pub callback_id: CallbackId,
}

pub enum AlertEvent {
    CutDroppedBelow { value: f64, threshold: f64, edges: Vec<EdgeId> },
    CutIncreasedAbove { value: f64, threshold: f64 },
    CutChanged { old_value: f64, new_value: f64, delta: f64 },
    EdgeInserted { edge_id: EdgeId, new_cut: f64 },
    EdgeDeleted { edge_id: EdgeId, new_cut: f64 },
}

impl MinCutMonitor {
    pub fn register_threshold(&mut self, threshold: Threshold) -> ThresholdId;

    pub fn register_callback<F>(&mut self, f: F) -> CallbackId
    where F: Fn(AlertEvent) + Send + 'static;

    pub fn start_monitoring(&mut self);

    pub fn stop_monitoring(&mut self);
}
```

### 6.2 Callback System

**Design Goals:**
- Non-blocking callback execution
- Error isolation (callback failure doesn't affect others)
- Priority-based execution

```rust
pub struct CallbackRegistry {
    callbacks: HashMap<CallbackId, CallbackEntry>,
    executor: ThreadPool,
    next_id: AtomicUsize,
}

struct CallbackEntry {
    callback: Box<dyn Fn(AlertEvent) + Send>,
    priority: i32,
    error_count: AtomicUsize,
    max_errors: usize,
}

impl CallbackRegistry {
    pub fn dispatch(&self, event: AlertEvent) {
        let mut entries: Vec<_> = self.callbacks.values().collect();
        entries.sort_by_key(|e| -e.priority);  // Higher priority first

        for entry in entries {
            let event = event.clone();
            self.executor.execute(move || {
                if let Err(e) = std::panic::catch_unwind(|| {
                    (entry.callback)(event);
                }) {
                    entry.error_count.fetch_add(1, Ordering::Relaxed);
                    eprintln!("Callback error: {:?}", e);
                }
            });
        }
    }
}
```

### 6.3 Performance Metrics

**Tracked Metrics:**
```rust
pub struct Metrics {
    // Update metrics
    pub total_updates: AtomicUsize,
    pub insert_time_ns: AtomicU64,      // Total nanoseconds
    pub delete_time_ns: AtomicU64,

    // Query metrics
    pub total_queries: AtomicUsize,
    pub query_time_ns: AtomicU64,

    // Structure metrics
    pub num_nodes: AtomicUsize,
    pub num_edges: AtomicUsize,
    pub num_levels: AtomicUsize,
    pub max_component_size: AtomicUsize,

    // Alert metrics
    pub alerts_triggered: AtomicUsize,
    pub alerts_suppressed: AtomicUsize,  // Rate limiting
}

impl Metrics {
    pub fn update_rate(&self) -> f64 {
        let total = self.total_updates.load(Ordering::Relaxed) as f64;
        let time = (self.insert_time_ns.load(Ordering::Relaxed)
                  + self.delete_time_ns.load(Ordering::Relaxed)) as f64;
        total / (time / 1e9)  // Updates per second
    }

    pub fn avg_update_time_us(&self) -> f64 {
        let total = self.total_updates.load(Ordering::Relaxed) as f64;
        let time = (self.insert_time_ns.load(Ordering::Relaxed)
                  + self.delete_time_ns.load(Ordering::Relaxed)) as f64;
        (time / total) / 1000.0  // Microseconds
    }
}
```

---

## 7. Integration with ruvector-graph

### 7.1 Adapter Pattern

**Goal:** Seamless integration without tight coupling

```rust
pub struct GraphDBAdapter {
    graph_db: Arc<GraphDB>,
    mincut: Arc<ConcurrentMinCut<ExactMinCut>>,
    sync_mode: SyncMode,
}

pub enum SyncMode {
    Immediate,      // Sync on every graph update
    Batched,        // Sync in batches
    Manual,         // Explicit sync calls
    Subscribe,      // Event-based subscription
}

impl GraphDBAdapter {
    pub fn new(graph_db: Arc<GraphDB>, sync_mode: SyncMode) -> Self {
        let adapter = Self {
            graph_db: graph_db.clone(),
            mincut: Arc::new(ConcurrentMinCut::new(ExactMinCut::new())),
            sync_mode,
        };

        // Subscribe to graph events if needed
        if matches!(sync_mode, SyncMode::Subscribe) {
            adapter.subscribe_to_graph_events();
        }

        adapter
    }

    pub fn sync_from_graph(&mut self) -> Result<()> {
        // Build initial cut structure from graph
        let nodes = self.graph_db.all_nodes()?;
        let edges = self.graph_db.all_edges()?;

        self.mincut.update(|mc| {
            for edge in edges {
                mc.insert_edge(edge.source, edge.target, edge.weight)?;
            }
            Ok(())
        })
    }

    fn subscribe_to_graph_events(&self) {
        // Hook into GraphDB transaction log
        self.graph_db.subscribe_to_changes(|change| {
            match change {
                GraphChange::EdgeAdded(e) => {
                    self.mincut.update(|mc| mc.insert_edge(e.source, e.target, e.weight))
                }
                GraphChange::EdgeRemoved(e) => {
                    self.mincut.update(|mc| mc.delete_edge(e.id))
                }
                _ => Ok(())  // Ignore node changes
            }
        });
    }
}
```

### 7.2 Storage Backend Integration

**Strategy:** Leverage ruvector-graph's storage for persistence

```rust
pub struct MinCutStorage {
    backend: GraphStorage,
    table_prefix: String,
}

impl MinCutStorage {
    pub fn save(&self, mincut: &ExactMinCut) -> Result<()> {
        // Serialize hierarchical decomposition
        let serialized = rkyv::to_bytes::<_, 256>(
            &mincut.decomposition
        )?;

        self.backend.put(
            &format!("{}/decomposition", self.table_prefix),
            &serialized
        )?;

        // Save metadata
        let metadata = Metadata {
            cut_value: mincut.cut_value.load(Ordering::Relaxed),
            update_count: mincut.update_count.load(Ordering::Relaxed),
            version: VERSION,
        };

        self.backend.put(
            &format!("{}/metadata", self.table_prefix),
            &rkyv::to_bytes::<_, 64>(&metadata)?
        )?;

        Ok(())
    }

    pub fn load(&self) -> Result<ExactMinCut> {
        let decomposition_bytes = self.backend.get(
            &format!("{}/decomposition", self.table_prefix)
        )?;

        let decomposition = unsafe {
            rkyv::from_bytes_unchecked(&decomposition_bytes)?
        };

        // Reconstruct MinCut from saved state
        Ok(ExactMinCut::from_decomposition(decomposition))
    }
}
```

---

## 8. WASM Support

### 8.1 WASM Bindings

**Exposed API:**
```rust
#[wasm_bindgen]
pub struct MinCutWasm {
    inner: Arc<ConcurrentMinCut<ExactMinCut>>,
}

#[wasm_bindgen]
impl MinCutWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ConcurrentMinCut::new(ExactMinCut::new()))
        }
    }

    #[wasm_bindgen(js_name = insertEdge)]
    pub fn insert_edge(&mut self, u: u32, v: u32, weight: f64) -> Result<(), JsValue> {
        self.inner.update(|mc| {
            mc.insert_edge(u as NodeId, v as NodeId, weight)
        }).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = deleteEdge)]
    pub fn delete_edge(&mut self, edge_id: u32) -> Result<(), JsValue> {
        self.inner.update(|mc| {
            mc.delete_edge(edge_id as EdgeId)
        }).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = getCutValue)]
    pub fn get_cut_value(&self) -> f64 {
        self.inner.query(|mc| mc.query_cut_value())
    }

    #[wasm_bindgen(js_name = getCutEdges)]
    pub fn get_cut_edges(&self) -> Vec<u32> {
        self.inner.query(|mc| {
            mc.query_cut_edges().into_iter()
                .map(|e| e as u32)
                .collect()
        })
    }

    #[wasm_bindgen(js_name = onCutChanged)]
    pub fn on_cut_changed(&mut self, callback: js_sys::Function) -> u32 {
        // Register JS callback for alerts
        // Return callback ID
    }
}
```

### 8.2 Web Worker Support

**Design:** Offload computation to Web Worker for non-blocking UI

```rust
// In worker.rs
#[wasm_bindgen]
pub fn worker_init() {
    // Initialize worker-side structures
    utils::set_panic_hook();
}

#[wasm_bindgen]
pub fn worker_handle_message(msg: JsValue) -> JsValue {
    let request: WorkerRequest = serde_wasm_bindgen::from_value(msg).unwrap();

    match request {
        WorkerRequest::InsertEdge { u, v, weight } => {
            // Process in worker
            GLOBAL_MINCUT.with(|mc| mc.insert_edge(u, v, weight));

            // Send result back to main thread
            WorkerResponse::UpdateComplete {
                new_cut_value: GLOBAL_MINCUT.with(|mc| mc.query_cut_value())
            }
        }
        // ... other operations
    }
}
```

### 8.3 Memory Management for WASM

**Challenges:**
- Limited memory in browser
- No virtual memory
- Manual memory management

**Strategy:**
```rust
pub struct WasmMemoryManager {
    max_memory_mb: usize,
    current_usage: AtomicUsize,
    eviction_strategy: EvictionStrategy,
}

impl WasmMemoryManager {
    pub fn allocate(&self, size: usize) -> Result<*mut u8, MemoryError> {
        // Check if we have enough memory
        let current = self.current_usage.load(Ordering::Acquire);
        if current + size > self.max_memory_mb * 1024 * 1024 {
            // Try eviction
            self.evict_until_available(size)?;
        }

        // Allocate
        let ptr = unsafe { alloc(Layout::from_size_align_unchecked(size, 8)) };
        self.current_usage.fetch_add(size, Ordering::Release);

        Ok(ptr)
    }

    fn evict_until_available(&self, needed: usize) -> Result<(), MemoryError> {
        match self.eviction_strategy {
            EvictionStrategy::LRU => self.evict_lru(needed),
            EvictionStrategy::CompressOld => self.compress_old_levels(needed),
        }
    }
}
```

---

## 9. Performance Characteristics

### 9.1 Time Complexity

| Operation | Exact Algorithm | Approximate (ε=0.1) | WASM Overhead |
|-----------|----------------|---------------------|---------------|
| Insert Edge | O(n^{o(1)}) amortized | O(polylog n) | +10-20% |
| Delete Edge | O(n^{o(1)}) amortized | O(polylog n) | +10-20% |
| Query Cut Value | O(1) | O(1) | +5% |
| Query Cut Edges | O(k) where k = cut size | O(k) | +10% |
| Full Rebuild | O(n² polylog n) worst | O(n log³ n) | +30% |

**Note:** n^{o(1)} means subpolynomial, typically O(n^{1/loglog n}) or similar

### 9.2 Space Complexity

| Component | Space Usage |
|-----------|-------------|
| Link-Cut Tree | O(n) |
| Hierarchical Decomposition | O(n polylog n) |
| Edge Index | O(m) where m = edges |
| LRU Cache | Configurable O(k) |
| **Total** | **O(n polylog n + m)** |

### 9.3 Scalability Targets

| Graph Size | Expected Update Time | Query Time | Memory Usage |
|------------|---------------------|------------|--------------|
| 1K nodes | < 100 μs | < 1 μs | ~10 MB |
| 10K nodes | < 500 μs | < 10 μs | ~100 MB |
| 100K nodes | < 5 ms | < 50 μs | ~1 GB |
| 1M nodes | < 50 ms | < 100 μs | ~10 GB |

**WASM Targets:**
- Max graph size: 100K nodes (browser memory constraints)
- Update latency: < 10 ms (for 60 FPS visualization)
- Memory footprint: < 500 MB

---

## 10. Technology Stack

### 10.1 Core Dependencies

```toml
[dependencies]
# Existing ruvector dependencies
ruvector-core = { version = "0.1.25", path = "../ruvector-core" }
ruvector-graph = { version = "0.1.25", path = "../ruvector-graph", optional = true }

# Data structures
petgraph = "0.6"           # Graph algorithms utilities
roaring = "0.10"           # Bitmap sets for node sets

# Concurrency
rayon = "1.10"             # Parallel iteration
crossbeam = "0.8"          # Lock-free data structures
parking_lot = "0.12"       # Better RwLock
dashmap = "6.1"            # Concurrent HashMap

# Serialization
rkyv = { workspace = true }
serde = { workspace = true }

# Error handling
thiserror = { workspace = true }
anyhow = { workspace = true }

# Numerics
ordered-float = "4.2"      # Floating point ordering
rand = { workspace = true }

# Async (optional)
tokio = { workspace = true, optional = true }

# WASM (optional)
wasm-bindgen = { workspace = true, optional = true }
wasm-bindgen-futures = { workspace = true, optional = true }
js-sys = { workspace = true, optional = true }
web-sys = { workspace = true, optional = true }

# Monitoring
tracing = { workspace = true }

[dev-dependencies]
criterion = { workspace = true }
proptest = { workspace = true }
```

### 10.2 Feature Flags

```toml
[features]
default = ["exact", "approximate"]

# Core algorithms
exact = []                 # Exact n^{o(1)} algorithm
approximate = []           # (1+ε)-approximate algorithm

# Integration
integration = ["ruvector-graph"]  # GraphDB integration
storage = ["ruvector-graph/storage"]  # Persistent storage

# Monitoring
monitoring = ["tokio"]     # Real-time monitoring
metrics = []               # Performance metrics

# WASM support
wasm = ["wasm-bindgen", "wasm-bindgen-futures", "js-sys", "web-sys"]

# Full feature set
full = ["exact", "approximate", "integration", "storage", "monitoring", "metrics"]
```

---

## 11. API Design

### 11.1 Public API Surface

```rust
// Core types
pub use types::{NodeId, EdgeId, CutValue, Edge};
pub use error::{MinCutError, Result};

// Main structures
pub use core::mincut::MinCut;
pub use core::exact::ExactMinCut;
pub use core::approximate::ApproximateMinCut;

// Monitoring
pub use monitoring::{MinCutMonitor, AlertEvent, Threshold, ThresholdCondition};

// Integration
#[cfg(feature = "integration")]
pub use integration::GraphDBAdapter;

// WASM
#[cfg(feature = "wasm")]
pub use wasm::MinCutWasm;
```

### 11.2 Main API

```rust
/// Main minimum cut interface
pub trait MinCut: Send + Sync {
    /// Insert an edge with given weight
    fn insert_edge(&mut self, u: NodeId, v: NodeId, weight: f64) -> Result<()>;

    /// Delete an edge by ID
    fn delete_edge(&mut self, edge_id: EdgeId) -> Result<()>;

    /// Query current minimum cut value
    fn query_cut_value(&self) -> CutValue;

    /// Query edges in minimum cut
    fn query_cut_edges(&self) -> Vec<EdgeId>;

    /// Query vertex partition (S, T) of minimum cut
    fn query_cut_partition(&self) -> (Vec<NodeId>, Vec<NodeId>);

    /// Get performance statistics
    fn statistics(&self) -> Statistics;
}

/// Builder pattern for configuration
pub struct MinCutBuilder {
    algorithm: Algorithm,
    epsilon: Option<f64>,
    enable_monitoring: bool,
    cache_size: usize,
    num_threads: Option<usize>,
}

impl MinCutBuilder {
    pub fn new() -> Self;
    pub fn exact(mut self) -> Self;
    pub fn approximate(mut self, epsilon: f64) -> Self;
    pub fn with_monitoring(mut self) -> Self;
    pub fn with_cache_size(mut self, size: usize) -> Self;
    pub fn with_threads(mut self, n: usize) -> Self;
    pub fn build(self) -> Result<Box<dyn MinCut>>;
}

// Usage example:
// let mincut = MinCutBuilder::new()
//     .exact()
//     .with_monitoring()
//     .with_cache_size(10000)
//     .build()?;
```

### 11.3 Monitoring API

```rust
pub struct MonitorBuilder {
    mincut: Arc<dyn MinCut>,
}

impl MonitorBuilder {
    pub fn new(mincut: Arc<dyn MinCut>) -> Self;

    pub fn threshold_below(self, value: f64, callback: impl Fn(AlertEvent) + Send + 'static) -> Self;

    pub fn threshold_above(self, value: f64, callback: impl Fn(AlertEvent) + Send + 'static) -> Self;

    pub fn on_change(self, callback: impl Fn(AlertEvent) + Send + 'static) -> Self;

    pub fn build(self) -> MinCutMonitor;
}

// Usage:
// let monitor = MonitorBuilder::new(mincut.clone())
//     .threshold_below(10.0, |event| {
//         println!("ALERT: Cut dropped to {}", event.value);
//     })
//     .on_change(|event| {
//         log::info!("Cut changed: {:?}", event);
//     })
//     .build();
```

---

## 12. Testing Strategy

### 12.1 Correctness Testing

```rust
#[cfg(test)]
mod correctness_tests {
    // Test against known minimum cuts
    #[test]
    fn test_complete_graph() {
        let mut mc = ExactMinCut::new();
        // K_n has min cut = n-1
        for i in 0..n {
            for j in i+1..n {
                mc.insert_edge(i, j, 1.0);
            }
        }
        assert_eq!(mc.query_cut_value(), (n - 1) as f64);
    }

    // Test against Karger's algorithm (randomized verification)
    #[test]
    fn test_random_graphs() {
        use proptest::prelude::*;
        proptest!(|(graph in random_graph_strategy())| {
            let mc_cut = exact_mincut(&graph);
            let karger_cut = karger_mincut(&graph, 1000);  // Many trials
            prop_assert!((mc_cut - karger_cut).abs() < 1e-6);
        });
    }
}
```

### 12.2 Performance Testing

```rust
// benches/update_performance.rs
fn benchmark_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("updates");

    for size in [100, 1000, 10000, 100000] {
        group.bench_function(BenchmarkId::new("insert", size), |b| {
            let mut mc = ExactMinCut::new();
            // Pre-populate with random graph
            populate_random(&mut mc, size);

            b.iter(|| {
                let (u, v) = random_edge(size);
                mc.insert_edge(u, v, 1.0)
            });
        });
    }

    group.finish();
}
```

### 12.3 Stress Testing

```rust
#[test]
#[ignore]  // Long-running
fn stress_test_concurrent_updates() {
    let mc = Arc::new(ConcurrentMinCut::new(ExactMinCut::new()));
    let num_threads = 16;
    let updates_per_thread = 100000;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let mc = mc.clone();
            thread::spawn(move || {
                for i in 0..updates_per_thread {
                    let u = random_node();
                    let v = random_node();

                    if i % 2 == 0 {
                        mc.update(|m| m.insert_edge(u, v, 1.0));
                    } else {
                        let edges = mc.query(|m| m.query_cut_edges());
                        if let Some(&edge_id) = edges.first() {
                            mc.update(|m| m.delete_edge(edge_id));
                        }
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify structure is still consistent
    mc.query(|m| {
        assert!(m.validate_structure());
    });
}
```

---

## 13. Deployment and Operations

### 13.1 Integration Patterns

**Pattern 1: Direct Integration**
```rust
use ruvector_graph::GraphDB;
use ruvector_mincut::{MinCutBuilder, MonitorBuilder};

let graph = GraphDB::new();
let mincut = MinCutBuilder::new()
    .exact()
    .build()?;

// Manually sync
for edge in graph.all_edges()? {
    mincut.insert_edge(edge.source, edge.target, edge.weight)?;
}
```

**Pattern 2: Adapter Integration**
```rust
use ruvector_mincut::GraphDBAdapter;

let graph = Arc::new(GraphDB::new());
let adapter = GraphDBAdapter::new(
    graph.clone(),
    SyncMode::Subscribe  // Auto-sync
);

// Automatically stays in sync with graph
graph.add_edge(1, 2, 5.0)?;
// mincut is automatically updated
```

**Pattern 3: WASM Integration**
```javascript
import init, { MinCutWasm } from './ruvector_mincut.js';

async function main() {
    await init();

    const mincut = new MinCutWasm();

    // Add edges
    mincut.insertEdge(0, 1, 5.0);
    mincut.insertEdge(1, 2, 3.0);

    // Query
    const cutValue = mincut.getCutValue();
    const cutEdges = mincut.getCutEdges();

    // Set up monitoring
    mincut.onCutChanged((event) => {
        console.log('Cut changed:', event);
        updateVisualization(event);
    });
}
```

### 13.2 Monitoring and Observability

**Metrics Export:**
```rust
#[cfg(feature = "metrics")]
pub fn export_prometheus_metrics() -> String {
    format!(
        "# HELP mincut_update_total Total number of updates\n\
         # TYPE mincut_update_total counter\n\
         mincut_update_total {}\n\
         # HELP mincut_update_duration_seconds Update duration\n\
         # TYPE mincut_update_duration_seconds histogram\n\
         mincut_update_duration_seconds_sum {}\n\
         mincut_update_duration_seconds_count {}\n",
        METRICS.total_updates.load(Ordering::Relaxed),
        METRICS.insert_time_ns.load(Ordering::Relaxed) as f64 / 1e9,
        METRICS.total_updates.load(Ordering::Relaxed),
    )
}
```

**Tracing Integration:**
```rust
use tracing::{info, warn, debug, instrument};

#[instrument(skip(self))]
pub fn insert_edge(&mut self, u: NodeId, v: NodeId, weight: f64) -> Result<()> {
    debug!("Inserting edge ({}, {}) with weight {}", u, v, weight);

    let start = Instant::now();
    let result = self.insert_edge_impl(u, v, weight);
    let elapsed = start.elapsed();

    if elapsed > Duration::from_millis(10) {
        warn!("Slow update: {} ms", elapsed.as_millis());
    }

    info!(
        edge = %format!("({}, {})", u, v),
        weight = %weight,
        duration_us = %elapsed.as_micros(),
        "Edge inserted"
    );

    result
}
```

---

## 14. Risk Assessment and Mitigation

### 14.1 Technical Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Subpolynomial algorithm too slow in practice | High | Medium | Implement approximate algorithm as fallback; extensive benchmarking |
| Memory overhead exceeds limits in WASM | High | Medium | Aggressive compression; level eviction; configurable limits |
| Concurrent updates cause race conditions | Critical | Low | Extensive concurrent testing; formal verification of critical sections |
| Integration breaks existing graph functionality | High | Low | Loose coupling via adapter pattern; comprehensive integration tests |
| WASM compilation size too large | Medium | Medium | Feature flags to exclude unused code; wasm-opt optimization |

### 14.2 Mitigation Strategies

**Performance Risk Mitigation:**
1. Implement both exact and approximate algorithms
2. Adaptive algorithm selection based on graph properties
3. Extensive profiling and optimization before release
4. Performance regression tests in CI

**Memory Risk Mitigation:**
1. Configurable memory limits
2. LRU eviction for decomposition levels
3. Compression of old data
4. Memory profiling in benchmarks

**Correctness Risk Mitigation:**
1. Property-based testing with proptest
2. Comparison against known algorithms (Karger, Stoer-Wagner)
3. Formal invariant checking
4. Extensive fuzzing

---

## 15. Future Extensions

### 15.1 Planned Enhancements

1. **Directed Graphs:** Extend to support minimum s-t cuts in directed graphs
2. **Weighted Vertices:** Support vertex-weighted minimum cuts
3. **k-Way Cuts:** Generalize to minimum k-way partition
4. **Streaming:** Support streaming edge updates with bounded memory
5. **Distributed:** Partition large graphs across multiple machines

### 15.2 Research Opportunities

1. **Improved Update Time:** Explore recent advances (Chuzhoy et al. 2023)
2. **Quantum Algorithms:** Investigate quantum speedups for minimum cut
3. **Machine Learning:** Learn optimal decomposition strategies
4. **Approximate Counting:** Estimate number of minimum cuts

### 15.3 Integration Roadmap

```
Phase 1 (MVP):
- Core exact algorithm
- Basic monitoring
- WASM bindings
- Integration with ruvector-graph

Phase 2 (Enhancement):
- Approximate algorithm
- Advanced monitoring (dashboards)
- Distributed support
- Performance optimizations

Phase 3 (Advanced):
- Streaming support
- k-way cuts
- Machine learning enhancements
- Quantum algorithms exploration
```

---

## 16. Architecture Decision Records (ADRs)

### ADR-001: Use Link-Cut Trees over Euler Tour Trees

**Context:** Need dynamic tree data structure for connectivity queries.

**Decision:** Use Link-Cut Trees instead of Euler Tour Trees.

**Rationale:**
- Link-Cut Trees have cleaner implementation
- Better documented in literature
- Easier to debug and maintain
- Performance is comparable (both O(log n))

**Consequences:**
- Well-tested implementation available
- May be slightly slower for path aggregation
- Easier to add new features

**Status:** Accepted

---

### ADR-002: Separate Crate Instead of Module in ruvector-graph

**Context:** Minimum cut functionality could be a module or separate crate.

**Decision:** Create separate `ruvector-mincut` crate.

**Rationale:**
- Clear separation of concerns
- Optional dependency for users who don't need it
- Independent versioning and releases
- Easier to test and benchmark in isolation
- Can be used independently of ruvector-graph

**Consequences:**
- Additional crate maintenance overhead
- Need for explicit integration code
- More flexible dependency management

**Status:** Accepted

---

### ADR-003: Event-Driven Monitoring over Polling

**Context:** Need real-time alerts when cut value changes.

**Decision:** Use event-driven callback system instead of polling.

**Rationale:**
- Lower latency for alerts
- No wasted CPU on polling
- More scalable for many monitors
- Standard pattern in Rust

**Consequences:**
- More complex implementation
- Need to handle callback failures
- Thread safety considerations

**Status:** Accepted

---

### ADR-004: Support Both Exact and Approximate Algorithms

**Context:** Trade-off between accuracy and performance.

**Decision:** Provide both exact and (1+ε)-approximate implementations.

**Rationale:**
- Users have different requirements
- Approximate is much faster (polylog vs n^{o(1)})
- Exact provides guarantees when needed
- Can choose at runtime

**Consequences:**
- More code to maintain
- Need to test both paths
- Flexibility for users

**Status:** Accepted

---

## 17. Success Criteria

### 17.1 Functional Requirements

- [ ] Correct minimum cut computation verified against known algorithms
- [ ] Subpolynomial update time O(n^{o(1)}) for exact algorithm
- [ ] Polylog update time for approximate algorithm
- [ ] Real-time alerts with < 1ms latency from cut change to callback
- [ ] WASM bindings work in major browsers (Chrome, Firefox, Safari)
- [ ] Integration with ruvector-graph via adapter pattern
- [ ] Persistent storage and recovery

### 17.2 Performance Requirements

| Metric | Target | Stretch Goal |
|--------|--------|--------------|
| Update time (1K nodes) | < 200 μs | < 100 μs |
| Update time (10K nodes) | < 1 ms | < 500 μs |
| Update time (100K nodes) | < 10 ms | < 5 ms |
| Query time | < 10 μs | < 1 μs |
| Memory overhead | < 2x graph size | < 1.5x graph size |
| WASM load time | < 1s | < 500ms |

### 17.3 Quality Requirements

- [ ] 90%+ code coverage
- [ ] No memory leaks (verified with valgrind)
- [ ] No data races (verified with thread sanitizer)
- [ ] Benchmarks in CI (no regressions > 10%)
- [ ] Documentation coverage > 80%
- [ ] All public APIs have examples

---

## 18. Conclusion

This architecture provides a solid foundation for implementing a state-of-the-art dynamic minimum cut system in Rust. The key design decisions prioritize:

1. **Correctness:** Rigorous testing and verification
2. **Performance:** Subpolynomial update time with practical optimizations
3. **Flexibility:** Both exact and approximate algorithms
4. **Integration:** Seamless integration with ruvector-graph
5. **Usability:** Clean API and excellent WASM support

The modular design allows for incremental development and future enhancements while maintaining a stable public API.

### Next Steps

1. **Implement Core Data Structures** (Week 1-2)
   - Link-cut tree
   - Edge indexing
   - Hierarchical decomposition skeleton

2. **Exact Algorithm MVP** (Week 3-4)
   - Basic insert/delete
   - Cut value computation
   - Correctness tests

3. **Monitoring System** (Week 5)
   - Threshold management
   - Callback system
   - Metrics collection

4. **Integration & WASM** (Week 6-7)
   - GraphDB adapter
   - WASM bindings
   - Browser demo

5. **Optimization & Release** (Week 8)
   - Performance tuning
   - Documentation
   - Publish v0.1.0

---

## Appendix A: References

1. Nanongkai, D., Saranurak, T., & Yingchareonthawornchai, S. (2022). "Dynamic Minimum Cut in O(n^{o(1)}) Update Time"
2. Karger, D. R. (2000). "Minimum Cuts in Near-Linear Time"
3. Sleator, D. D., & Tarjan, R. E. (1983). "A Data Structure for Dynamic Trees"
4. Thorup, M. (2007). "Fully-Dynamic Min-Cut"

## Appendix B: Glossary

- **Subpolynomial:** o(n^c) for any constant c > 0
- **Expander:** Graph with high conductance (φ ≥ threshold)
- **Link-Cut Tree:** Dynamic tree data structure supporting link, cut, and path queries
- **Amortized Time:** Average time per operation over sequence
- **Cut:** Partition of vertices into two sets; edges crossing = cut edges
- **Minimum Cut:** Cut with minimum total edge weight

---

**Document Status:** Ready for Review
**Reviewed By:** [Pending]
**Approved By:** [Pending]
**Version:** 1.0
**Last Updated:** 2025-12-21
