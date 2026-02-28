# Architecture Compatibility and Synergy Analysis

**Document Class:** Technical Architecture Review
**Version:** 1.0.0
**Date:** 2026-02-27

---

## 1. Dependency Compatibility Matrix

### Shared Dependencies (Exact or Compatible Versions)

| Dependency | agentic-robotics | ruvector | Status | Resolution |
|-----------|-----------------|----------|--------|------------|
| tokio | 1.47 (full) | 1.41 (rt-multi-thread, sync, macros) | Minor mismatch | Upgrade ruvector to 1.47 |
| serde | 1.0 (derive) | 1.0 (derive) | Compatible | No action |
| serde_json | 1.0 | 1.0 | Compatible | No action |
| rkyv | 0.8 | 0.8 | Compatible | No action |
| crossbeam | 0.8 | 0.8 | Compatible | No action |
| rayon | 1.10 | 1.10 | Compatible | No action |
| parking_lot | 0.12 | 0.12 | Compatible | No action |
| nalgebra | 0.33 | 0.33 (no-default-features) | Compatible | Unify feature flags |
| thiserror | 2.0 | 2.0 | Compatible | No action |
| anyhow | 1.0 | 1.0 | Compatible | No action |
| tracing | 0.1 | 0.1 | Compatible | No action |
| tracing-subscriber | 0.3 | 0.3 (env-filter) | Compatible | No action |
| criterion | 0.5 (html_reports) | 0.5 (html_reports) | Compatible | No action |
| rand | 0.8 | 0.8 | Compatible | No action |

### agentic-robotics-Unique Dependencies

| Dependency | Version | Size Impact | Feature-Gate Strategy |
|-----------|---------|------------|----------------------|
| zenoh | 1.0 | Large (~50+ transitive) | `feature = "robotics"` |
| rustdds | 0.11 | Medium (~20 transitive) | `feature = "robotics-dds"` |
| cdr | 0.2 | Small | `feature = "robotics"` |
| hdrhistogram | 7.5 | Small | `feature = "robotics-rt"` |
| wide | 0.7 | Small | `feature = "robotics-simd"` |
| axum | 0.7 | Medium | `feature = "robotics-sse"` |

### ruvector-Unique Dependencies

| Dependency | Version | Notes |
|-----------|---------|-------|
| redb | 2.1 | Storage backend |
| memmap2 | 0.9 | Memory-mapped files |
| hnsw_rs | 0.3 (patched) | HNSW index (patched for WASM) |
| simsimd | 5.9 | SIMD distance functions |
| ndarray | 0.16 | N-dimensional arrays |
| dashmap | 6.1 | Concurrent hashmap |
| lean-agentic | 0.1.0 | Formal verification |
| wasm-bindgen | 0.2 | WASM interop |

### Version Conflict Resolution Plan

**tokio 1.41 -> 1.47:**
- Minor version bump, fully backward compatible
- New features in 1.47 (improved multi-thread scheduling) benefit both
- Change: `Cargo.toml` workspace `tokio = { version = "1.47", ... }`

**napi 2.16 -> 3.0:**
- Breaking change: napi 3.0 has different macro syntax
- Strategy: Maintain separate NAPI versions per crate until coordinated upgrade
- OR: Upgrade all ruvector -node crates to napi 3.0 (recommended)

---

## 2. Architecture Layer Mapping

```
+=========================================================================+
|                    UNIFIED COGNITIVE ROBOTICS PLATFORM                    |
+=========================================================================+
|                                                                           |
|  APPLICATION LAYER                                                        |
|  +----------------------------+  +------------------------------------+  |
|  | Robot Applications         |  | ML/AI Applications                 |  |
|  | - Autonomous navigation    |  | - Vector search                    |  |
|  | - Swarm coordination       |  | - Graph reasoning                  |  |
|  | - Manipulation control     |  | - Attention inference              |  |
|  +----------------------------+  +------------------------------------+  |
|                |                              |                           |
|  MCP LAYER (AI TOOL INTERFACE)                                           |
|  +-------------------------------------------------------------------+  |
|  | agentic-robotics-mcp + ruvector MCP tools                         |  |
|  | - robot_move, sensor_read    | vector_search, gnn_classify        |  |
|  | - path_plan, status_query    | attention_focus, memory_recall     |  |
|  +-------------------------------------------------------------------+  |
|                |                              |                           |
|  SCHEDULING LAYER                                                        |
|  +-------------------------------------------------------------------+  |
|  | agentic-robotics-rt (Dual Runtime)                                |  |
|  | HIGH-PRIORITY (2 threads)   | LOW-PRIORITY (4 threads)            |  |
|  | - Control loops (<1ms)      | - Planning (>1ms)                   |  |
|  | - Sensor processing         | - Index rebuilds                    |  |
|  | - GNN inference (urgent)    | - Batch vector ops                  |  |
|  | - Attention (time-critical) | - Training updates                  |  |
|  +-------------------------------------------------------------------+  |
|                |                              |                           |
|  MESSAGING LAYER                                                         |
|  +----------------------------+  +------------------------------------+  |
|  | agentic-robotics-core      |  | ruvector-cluster                   |  |
|  | - Publisher<T>/Subscriber<T>|  | - Raft consensus                   |  |
|  | - Zenoh pub/sub             |  | - Replication                      |  |
|  | - CDR/JSON serialization    |  | - Delta consensus                  |  |
|  +----------------------------+  +------------------------------------+  |
|                |                              |                           |
|  COMPUTE LAYER                                                           |
|  +----------------------------+  +------------------------------------+  |
|  | Robotics Compute           |  | ML Compute                         |  |
|  | - Kinematic solvers         |  | - HNSW indexing (ruvector-core)    |  |
|  | - Path planning             |  | - GNN forward (ruvector-gnn)       |  |
|  | - State estimation          |  | - Attention (ruvector-attention)   |  |
|  |                             |  | - Graph transformer                |  |
|  |                             |  | - Sparse inference                 |  |
|  +----------------------------+  +------------------------------------+  |
|                |                              |                           |
|  STORAGE LAYER                                                           |
|  +-------------------------------------------------------------------+  |
|  | ruvector-core (redb + memmap2) | ruvector-postgres                |  |
|  | - Vector persistence            | - SQL storage backend            |  |
|  | - Index snapshots               | - Graph persistence              |  |
|  +-------------------------------------------------------------------+  |
|                                                                           |
|  BINDING LAYER                                                           |
|  +----------------------------+  +------------------------------------+  |
|  | NAPI (Node.js)             |  | WASM (Browser/Edge)                |  |
|  | agentic-robotics-node      |  | ruvector-*-wasm (20+ crates)       |  |
|  | ruvector-*-node (10+)      |  | agentic-robotics-embedded          |  |
|  +----------------------------+  +------------------------------------+  |
|                                                                           |
+=========================================================================+
```

---

## 3. Data Flow Integration

### Sensor-to-Decision Pipeline

```
[LiDAR Sensor]
     |
     v
[PointCloud Message]  ──> agentic-robotics-core Publisher
     |
     | (zero-copy via shared memory / crossbeam channel)
     v
[BRIDGE: PointCloud -> Vec<f32>]  ──> ruvector-robotics-bridge
     |
     ├──> [HNSW Spatial Index]  ──> ruvector-core
     |         |
     |         v
     |    [Nearest Obstacles]  (k-NN search, <500us)
     |         |
     ├──> [Scene Graph Build]  ──> ruvector-graph
     |         |
     |         v
     |    [Graph Transformer]  ──> ruvector-graph-transformer
     |         |
     |         v
     |    [Scene Understanding]  (spatial reasoning)
     |         |
     └──> [GNN Classification]  ──> ruvector-gnn
               |
               v
          [Object Classes + Confidence]
               |
               v
     [Decision Fusion]  ──> ruvector-attention (weighted)
               |
               v
     [Action Command]  ──> agentic-robotics-core Publisher -> /cmd_vel
```

### Data Type Mappings

```rust
// Bridge: PointCloud -> HNSW-indexable vectors
impl From<&PointCloud> for Vec<Vec<f32>> {
    fn from(cloud: &PointCloud) -> Self {
        cloud.points.iter()
            .map(|p| vec![p.x, p.y, p.z])
            .collect()
    }
}

// Bridge: RobotState -> feature vector for temporal tensor
impl From<&RobotState> for Vec<f64> {
    fn from(state: &RobotState) -> Self {
        let mut v = Vec::with_capacity(7);
        v.extend_from_slice(&state.position);
        v.extend_from_slice(&state.velocity);
        v.push(state.timestamp as f64);
        v
    }
}

// Bridge: Pose -> graph node features
impl From<&Pose> for Vec<f64> {
    fn from(pose: &Pose) -> Self {
        let mut v = Vec::with_capacity(7);
        v.extend_from_slice(&pose.position);
        v.extend_from_slice(&pose.orientation);
        v
    }
}
```

---

## 4. Shared Pattern Analysis

### Concurrency Patterns

Both frameworks extensively use the same concurrency primitives:

**Arc<RwLock<T>> (read-heavy shared state):**
```rust
// agentic-robotics-mcp: tool registry
tools: Arc<RwLock<HashMap<String, (McpTool, ToolHandler)>>>

// ruvector-core: vector index (conceptual)
index: Arc<RwLock<HnswIndex>>
```

**Arc<Mutex<T>> (write-heavy shared state):**
```rust
// agentic-robotics-rt: scheduler queue
scheduler: Arc<Mutex<PriorityScheduler>>

// agentic-robotics-rt: latency histogram
histogram: Arc<Mutex<Histogram<u64>>>
```

**Crossbeam channels (message passing):**
```rust
// agentic-robotics-core: subscriber
let (sender, receiver) = channel::unbounded();

// ruvector uses crossbeam for parallel processing pipelines
```

### Serialization Strategies

Both frameworks support the same serialization stack:

| Format | agentic-robotics | ruvector | Use Case |
|--------|-----------------|----------|----------|
| serde (JSON) | Primary for NAPI | Configuration | Debug, interop |
| rkyv 0.8 | Derive macros on all types | Storage backend | Zero-copy persistence |
| CDR | Robot message wire format | N/A | DDS compatibility |
| bincode | N/A | Compact binary | Network transfer |

**Key insight:** Both derive `rkyv::{Archive, Serialize, Deserialize}` on core types, enabling zero-copy data sharing.

### Error Handling

```rust
// Both use thiserror for typed errors
#[derive(Error, Debug)]
pub enum Error {
    #[error("...")] Variant(String),
    #[error("...")] Io(#[from] std::io::Error),
    #[error("...")] Other(#[from] anyhow::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
```

### NAPI Binding Patterns

```rust
// agentic-robotics-node (napi 3.0)
#[napi]
pub struct AgenticNode { ... }
#[napi]
impl AgenticNode {
    #[napi(constructor)]
    pub fn new(name: String) -> Result<Self> { ... }
    #[napi]
    pub async fn create_publisher(&self, topic: String) -> Result<AgenticPublisher> { ... }
}

// ruvector-node (napi 2.16) - same pattern, older version
#[napi]
pub struct VectorIndex { ... }
#[napi]
impl VectorIndex {
    #[napi(constructor)]
    pub fn new(dimensions: u32) -> Result<Self> { ... }
    #[napi]
    pub async fn search(&self, query: Vec<f64>, k: u32) -> Result<SearchResults> { ... }
}
```

### Benchmark Patterns

Both use Criterion 0.5 with identical patterns:
```rust
fn benchmark_operation(c: &mut Criterion) {
    let mut group = c.benchmark_group("GroupName");
    group.bench_function("name", |b| {
        b.iter(|| { black_box(operation()); })
    });
    group.finish();
}
criterion_group!(benches, benchmark_operation);
criterion_main!(benches);
```

---

## 5. Integration Architecture Proposal

### Tier 1: Bridge Layer (Minimal Integration)

New crate: `ruvector-robotics-bridge`

```rust
//! Bridge between agentic-robotics messages and ruvector operations

use agentic_robotics_core::message::{PointCloud, RobotState, Pose, Message};
use ruvector_core::types::Vector;

/// Convert PointCloud to indexable vectors
pub fn pointcloud_to_vectors(cloud: &PointCloud) -> Vec<Vector> {
    cloud.points.iter()
        .map(|p| Vector::from_slice(&[p.x as f64, p.y as f64, p.z as f64]))
        .collect()
}

/// Convert RobotState to feature vector
pub fn state_to_vector(state: &RobotState) -> Vector {
    let mut data = Vec::with_capacity(7);
    data.extend_from_slice(&state.position);
    data.extend_from_slice(&state.velocity);
    data.push(state.timestamp as f64);
    Vector::from_vec(data)
}

/// Auto-indexing subscriber: indexes incoming PointClouds
pub struct IndexingSubscriber {
    subscriber: Subscriber<PointCloud>,
    index: Arc<RwLock<HnswIndex>>,
}

impl IndexingSubscriber {
    pub async fn run(&self) {
        loop {
            if let Ok(cloud) = self.subscriber.recv_async().await {
                let vectors = pointcloud_to_vectors(&cloud);
                let mut idx = self.index.write();
                for v in vectors { idx.insert(&v); }
            }
        }
    }
}
```

### Tier 2: Fusion Layer (Deep Integration)

New crate: `ruvector-robotics-perception`

```rust
//! Perception pipeline: sensor data -> ML inference -> decisions

use agentic_robotics_rt::{ROS3Executor, Priority, Deadline};
use ruvector_gnn::GraphNeuralNetwork;
use ruvector_attention::AttentionMechanism;

pub struct PerceptionPipeline {
    executor: ROS3Executor,
    gnn: Arc<GraphNeuralNetwork>,
    attention: Arc<AttentionMechanism>,
}

impl PerceptionPipeline {
    /// Process sensor data with RT-scheduled ML inference
    pub fn process(&self, cloud: PointCloud) {
        let gnn = self.gnn.clone();
        let attention = self.attention.clone();

        // High-priority: real-time obstacle detection
        self.executor.spawn_high(async move {
            let scene_graph = build_scene_graph(&cloud);
            let gnn_output = gnn.forward(&scene_graph);
            let focused = attention.apply(&gnn_output);
            // Publish decision
        });
    }
}
```

### Tier 3: Unified Cognitive Platform (Long-term)

```
ruvector-cognitive-robotics
    |-- agentic-robotics-core     (sensing + actuation)
    |-- agentic-robotics-rt       (scheduling)
    |-- agentic-robotics-mcp      (AI interface)
    |-- ruvector-core              (vector memory)
    |-- ruvector-gnn               (spatial reasoning)
    |-- ruvector-attention          (selective focus)
    |-- ruvector-nervous-system     (cognitive architecture)
    |-- ruvector-temporal-tensor    (temporal reasoning)
    |-- sona                        (self-learning)
```

---

## 6. Build System Integration

### Workspace Member Additions

Add to `Cargo.toml` `[workspace] members`:
```toml
members = [
    # ... existing 114 members ...
    "crates/agentic-robotics-core",
    "crates/agentic-robotics-rt",
    "crates/agentic-robotics-mcp",
    "crates/agentic-robotics-embedded",
    "crates/agentic-robotics-node",
    "crates/agentic-robotics-benchmarks",
    # New integration crates
    "crates/ruvector-robotics-bridge",
]
```

### Workspace Dependency Additions

```toml
[workspace.dependencies]
# New robotics dependencies
zenoh = { version = "1.0", optional = true }
rustdds = { version = "0.11", optional = true }
cdr = { version = "0.2", optional = true }
hdrhistogram = "7.5"
wide = "0.7"
```

### Feature Flag Strategy

```toml
# In ruvector-core/Cargo.toml
[features]
default = ["storage"]
robotics = ["agentic-robotics-core"]
robotics-rt = ["robotics", "agentic-robotics-rt"]
robotics-mcp = ["robotics", "agentic-robotics-mcp"]
robotics-full = ["robotics-rt", "robotics-mcp", "agentic-robotics-embedded"]
```

---

## 7. Performance Budget Analysis

### Latency Budget for Sensor-to-Decision Pipeline

| Stage | Budget | Mechanism |
|-------|--------|-----------|
| Sensor deserialization | 540ns | CDR zero-copy |
| PointCloud -> vectors | ~100ns | Direct memory map, no allocation |
| HNSW k-NN search (10K) | ~400us | O(log n) with SIMD distance |
| Scene graph construction | ~50us | Pre-allocated graph structures |
| GNN forward pass | ~200us | Small model, RT-scheduled |
| Attention application | ~100us | Single-head, focused features |
| Decision serialization | ~540ns | CDR output |
| **Total** | **<1ms** | Meets hard RT requirement |

### Memory Layout Optimization

```
Shared memory region (mmap):
+------------------------------------------+
| PointCloud (rkyv archived)               |
|   points: [Point3D; N]  <-- contiguous   |
|   intensities: [f32; N] <-- SIMD-ready   |
+------------------------------------------+
| HNSW Index Vectors                       |
|   [f32; 3] x N  <-- same memory layout   |
+------------------------------------------+
| GNN Graph (adjacency + features)         |
|   nodes: [f32; D] x M                    |
|   edges: [(u32, u32)] x E               |
+------------------------------------------+
```

Both Point3D `{x, y, z}: f32` and HNSW vectors `[f32; 3]` have identical memory layout, enabling zero-copy conversion via `unsafe { std::slice::from_raw_parts(...) }` when performance is critical.

---

## 8. NAPI/WASM Binding Unification Strategy

### Current State

| Binding | agentic-robotics | ruvector | Count |
|---------|-----------------|----------|-------|
| NAPI (Node.js) | 1 crate (napi 3.0) | 10+ crates (napi 2.16) | 11+ |
| WASM | 0 (planned) | 20+ crates (wasm-bindgen 0.2) | 20+ |

### Unified TypeScript API Design

```typescript
// @ruvector/platform - unified package
import { RobotNode, VectorIndex, GnnModel } from '@ruvector/platform';

// Create robot node with integrated vector search
const node = new RobotNode('perception_bot');
const index = new VectorIndex({ dimensions: 3, metric: 'l2' });
const gnn = await GnnModel.load('./scene_classifier.model');

// Subscribe to LiDAR, auto-index, classify
const lidar = await node.subscribe('/lidar/points');
lidar.onMessage(async (cloud) => {
    // Index points for spatial search
    await index.insertBatch(cloud.points);

    // Find nearest obstacles
    const obstacles = await index.search(robot.position, { k: 20 });

    // Classify scene
    const scene = gnn.classify(obstacles);

    // Publish decision
    await node.publish('/nav/command', scene.safePath);
});
```

### WASM Build Strategy

```
Phase 1: ruvector WASM crates work standalone (current)
Phase 2: agentic-robotics-core builds to WASM (remove Zenoh, use web-sys channels)
Phase 3: Combined ruvector-robotics-wasm with unified API
Phase 4: Web-based robot simulator using combined WASM + WebGL
```

---

## Conclusion

The architectural compatibility between agentic-robotics and ruvector is exceptionally high:
- 14/16 shared dependencies are version-compatible
- Identical Rust edition, build profiles, and coding patterns
- Complementary rather than overlapping functionality
- Both use rkyv 0.8 enabling zero-copy data sharing
- NAPI binding patterns are structurally identical

The primary integration challenges are manageable:
1. Zenoh dependency tree size (mitigated by feature flags)
2. NAPI version mismatch (coordinated upgrade to 3.0)
3. tokio minor version bump (backward compatible)

The synergy potential is substantial: no existing framework combines real-time robotics middleware with native vector database operations, GNN inference, and MCP tool exposure in a unified Rust workspace.
