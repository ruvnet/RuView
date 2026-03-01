# Integration Roadmap: agentic-robotics into ruvector

**Document Class:** Implementation Plan
**Version:** 1.0.0
**Date:** 2026-02-27
**Timeline:** 18 weeks (6 phases)

---

## Phase 0: Foundation (Week 1-2)

### Objective
Integrate agentic-robotics crates into the ruvector workspace with clean builds and passing tests.

### 0.1 Workspace Integration

Add the 6 agentic-robotics crates as workspace members. Required changes to root `Cargo.toml`:

```toml
[workspace]
members = [
    # ... existing 114 members ...
    # Agentic Robotics Integration
    "crates/agentic-robotics-core",
    "crates/agentic-robotics-rt",
    "crates/agentic-robotics-mcp",
    "crates/agentic-robotics-embedded",
    "crates/agentic-robotics-node",
    "crates/agentic-robotics-benchmarks",
]
```

### 0.2 Dependency Resolution

New workspace dependencies to add:

```toml
[workspace.dependencies]
# Robotics middleware
zenoh = "1.0"
rustdds = "0.11"
cdr = "0.2"
hdrhistogram = "7.5"
wide = "0.7"
```

### 0.3 Version Alignment

| Dependency | Current ruvector | agentic-robotics | Action |
|-----------|-----------------|-----------------|--------|
| tokio | 1.41 | 1.47 | Upgrade to 1.47 |
| napi | 2.16 | 3.0 | Keep separate initially |
| thiserror | 2.0 | 2.0 | No action |
| rkyv | 0.8 | 0.8 | No action |

### 0.4 Crate Cargo.toml Adaptation

Each agentic-robotics crate needs its Cargo.toml updated to reference ruvector workspace dependencies instead of its own workspace:

```toml
# Before (agentic-robotics workspace)
[dependencies]
tokio = { workspace = true }

# After (ruvector workspace - add explicit versions where needed)
[dependencies]
tokio = { version = "1.47", features = ["full", "rt-multi-thread", "time"] }
```

Or better: add robotics-specific deps to the ruvector workspace `[workspace.dependencies]` section.

### 0.5 CI Pipeline Updates

Add to `.github/workflows/`:
- Build agentic-robotics crates in CI
- Run agentic-robotics tests
- Feature-gate robotics builds to avoid slowing default CI

### Deliverables
- [ ] All 6 crates compile within ruvector workspace
- [ ] All existing ruvector tests still pass
- [ ] All agentic-robotics tests pass
- [ ] CI pipeline updated

---

## Phase 1: Bridge Layer (Week 3-4)

### Objective
Create adapter crate that converts between agentic-robotics and ruvector data types.

### New Crate: `ruvector-robotics-bridge`

```toml
[package]
name = "ruvector-robotics-bridge"
version.workspace = true
edition.workspace = true

[dependencies]
agentic-robotics-core = { path = "../agentic-robotics-core" }
ruvector-core = { path = "../ruvector-core", features = ["storage"] }
tokio = { workspace = true }
serde = { workspace = true }
tracing = { workspace = true }
```

### 1.1 Data Type Converters

```rust
//! src/converters.rs

use agentic_robotics_core::message::{PointCloud, Point3D, RobotState, Pose};

/// Convert PointCloud to Vec of 3D vectors for HNSW indexing
pub fn pointcloud_to_vectors(cloud: &PointCloud) -> Vec<Vec<f32>> {
    cloud.points.iter()
        .map(|p| vec![p.x, p.y, p.z])
        .collect()
}

/// Convert PointCloud to Vec of f64 vectors (ruvector native format)
pub fn pointcloud_to_f64_vectors(cloud: &PointCloud) -> Vec<Vec<f64>> {
    cloud.points.iter()
        .map(|p| vec![p.x as f64, p.y as f64, p.z as f64])
        .collect()
}

/// Convert RobotState to 7D feature vector [px, py, pz, vx, vy, vz, t]
pub fn state_to_vector(state: &RobotState) -> Vec<f64> {
    let mut v = Vec::with_capacity(7);
    v.extend_from_slice(&state.position);
    v.extend_from_slice(&state.velocity);
    v.push(state.timestamp as f64);
    v
}

/// Convert Pose to 7D feature vector [px, py, pz, qx, qy, qz, qw]
pub fn pose_to_vector(pose: &Pose) -> Vec<f64> {
    let mut v = Vec::with_capacity(7);
    v.extend_from_slice(&pose.position);
    v.extend_from_slice(&pose.orientation);
    v
}

/// Convert HNSW search results back to Point3D
pub fn vectors_to_points(vectors: &[Vec<f64>]) -> Vec<Point3D> {
    vectors.iter()
        .map(|v| Point3D {
            x: v[0] as f32,
            y: v[1] as f32,
            z: v.get(2).copied().unwrap_or(0.0) as f32,
        })
        .collect()
}
```

### 1.2 Auto-Indexing Subscriber

```rust
//! src/indexing.rs

use agentic_robotics_core::subscriber::Subscriber;
use agentic_robotics_core::message::PointCloud;
use ruvector_core::vector_db::VectorDb;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Subscriber that automatically indexes incoming PointCloud data
pub struct IndexingSubscriber {
    subscriber: Subscriber<PointCloud>,
    db: Arc<RwLock<VectorDb>>,
    frame_count: u64,
}

impl IndexingSubscriber {
    pub fn new(topic: &str, db: Arc<RwLock<VectorDb>>) -> Self {
        Self {
            subscriber: Subscriber::new(topic),
            db,
            frame_count: 0,
        }
    }

    /// Run the indexing loop (call from RT executor)
    pub async fn run(&mut self) {
        loop {
            match self.subscriber.recv_async().await {
                Ok(cloud) => {
                    let vectors = super::converters::pointcloud_to_f64_vectors(&cloud);
                    let mut db = self.db.write().await;
                    for vector in &vectors {
                        let _ = db.insert(vector);
                    }
                    self.frame_count += 1;
                    tracing::debug!("Indexed frame {} ({} points)", self.frame_count, vectors.len());
                }
                Err(e) => {
                    tracing::warn!("Subscriber error: {}", e);
                }
            }
        }
    }
}
```

### 1.3 Search Publisher

```rust
//! src/search.rs

use agentic_robotics_core::publisher::Publisher;
use agentic_robotics_core::message::Message;
use ruvector_core::vector_db::VectorDb;
use serde::{Serialize, Deserialize};

/// Search result message published back to robot topics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub query_id: u64,
    pub neighbors: Vec<Neighbor>,
    pub latency_us: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Neighbor {
    pub id: u64,
    pub distance: f64,
    pub vector: Vec<f64>,
}

impl Message for SearchResult {
    fn type_name() -> &'static str {
        "ruvector_msgs/SearchResult"
    }
}
```

### Deliverables
- [ ] `ruvector-robotics-bridge` crate created
- [ ] Converter functions with unit tests
- [ ] IndexingSubscriber with integration test
- [ ] SearchResult message type defined
- [ ] Documentation with usage examples

---

## Phase 2: Perception Pipeline (Week 5-7)

### Objective
Build GNN-based perception using ruvector ML modules with RT scheduling.

### New Crate: `ruvector-robotics-perception`

### 2.1 Scene Graph Builder

```rust
//! Convert PointCloud + obstacles into ruvector graph for GNN processing

pub struct SceneGraphBuilder {
    adjacency_radius: f64,
    max_nodes: usize,
}

impl SceneGraphBuilder {
    pub fn build(&self, cloud: &PointCloud, obstacles: &[Obstacle]) -> SceneGraph {
        let mut graph = SceneGraph::new();

        // Add obstacle nodes with spatial features
        for (i, obs) in obstacles.iter().enumerate() {
            graph.add_node(i, obs.to_features());
        }

        // Add edges based on spatial proximity
        for i in 0..obstacles.len() {
            for j in (i+1)..obstacles.len() {
                let dist = distance(&obstacles[i].position, &obstacles[j].position);
                if dist < self.adjacency_radius {
                    graph.add_edge(i, j, dist);
                }
            }
        }

        graph
    }
}
```

### 2.2 RT-Scheduled Inference

```rust
//! Schedule GNN inference on the high-priority runtime

use agentic_robotics_rt::{ROS3Executor, Priority, Deadline};

pub struct PerceptionEngine {
    executor: ROS3Executor,
    scene_builder: SceneGraphBuilder,
}

impl PerceptionEngine {
    /// Process point cloud with deadline-aware inference
    pub fn process(&self, cloud: PointCloud, deadline_us: u64) {
        let builder = self.scene_builder.clone();
        let deadline = Duration::from_micros(deadline_us);

        self.executor.spawn_rt(
            Priority(3),
            Deadline(deadline),
            async move {
                let scene = builder.build(&cloud, &[]);
                // GNN forward pass here
                tracing::info!("Scene processed: {} nodes", scene.node_count());
            },
        );
    }
}
```

### Deliverables
- [ ] SceneGraphBuilder from PointCloud data
- [ ] GNN-based object classification pipeline
- [ ] RT-scheduled inference with latency tracking
- [ ] Attention-weighted decision fusion
- [ ] Benchmark: sensor-to-decision < 2ms target

---

## Phase 3: MCP Tool Exposure (Week 8-9)

### Objective
Register ruvector capabilities as MCP tools accessible to AI assistants.

### 3.1 Tool Definitions

Register these 10 tools with the MCP server:

```rust
use agentic_robotics_mcp::{McpServer, McpTool, ToolHandler, tool, text_response};
use serde_json::json;

pub async fn register_ruvector_tools(server: &McpServer) -> anyhow::Result<()> {
    // 1. Vector Search
    server.register_tool(
        McpTool {
            name: "vector_search".into(),
            description: "Search for nearest vectors in HNSW index".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "array", "items": { "type": "number" } },
                    "k": { "type": "integer", "default": 10 },
                    "metric": { "type": "string", "enum": ["l2", "cosine", "dot"] }
                },
                "required": ["query"]
            }),
        },
        tool(|args| {
            let query: Vec<f64> = serde_json::from_value(args["query"].clone())?;
            let k = args["k"].as_u64().unwrap_or(10) as usize;
            // Perform HNSW search
            Ok(text_response(format!("Found {} nearest neighbors", k)))
        }),
    ).await?;

    // 2. GNN Classify
    server.register_tool(
        McpTool {
            name: "gnn_classify".into(),
            description: "Classify a graph structure using GNN".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "nodes": { "type": "array" },
                    "edges": { "type": "array" }
                }
            }),
        },
        tool(|args| Ok(text_response("Classification: obstacle"))),
    ).await?;

    // 3. Attention Focus
    server.register_tool(
        McpTool {
            name: "attention_focus".into(),
            description: "Apply attention to select important features".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "features": { "type": "array", "items": { "type": "array" } },
                    "query": { "type": "array", "items": { "type": "number" } }
                }
            }),
        },
        tool(|args| Ok(text_response("Attention weights computed"))),
    ).await?;

    // 4. Trajectory Predict
    server.register_tool(
        McpTool {
            name: "trajectory_predict".into(),
            description: "Predict future trajectory from state history".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "states": { "type": "array" },
                    "horizon": { "type": "integer", "default": 10 }
                }
            }),
        },
        tool(|args| Ok(text_response("Trajectory predicted: 10 waypoints"))),
    ).await?;

    // 5. Scene Graph Analyze
    server.register_tool(
        McpTool {
            name: "scene_analyze".into(),
            description: "Build and analyze scene graph from sensor data".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "points": { "type": "array" },
                    "radius": { "type": "number", "default": 1.0 }
                }
            }),
        },
        tool(|args| Ok(text_response("Scene: 12 objects, 3 obstacles"))),
    ).await?;

    // 6. Anomaly Detect
    server.register_tool(
        McpTool {
            name: "anomaly_detect".into(),
            description: "Detect anomalies using sparse inference".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "data": { "type": "array", "items": { "type": "number" } },
                    "threshold": { "type": "number", "default": 0.95 }
                }
            }),
        },
        tool(|args| Ok(text_response("No anomalies detected (score: 0.12)"))),
    ).await?;

    // 7. Memory Store
    server.register_tool(
        McpTool {
            name: "memory_store".into(),
            description: "Store an experience/episode in AgentDB memory".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string" },
                    "content": { "type": "string" },
                    "embedding": { "type": "array", "items": { "type": "number" } }
                },
                "required": ["key", "content"]
            }),
        },
        tool(|args| Ok(text_response("Episode stored successfully"))),
    ).await?;

    // 8. Memory Recall
    server.register_tool(
        McpTool {
            name: "memory_recall".into(),
            description: "Recall similar memories via semantic search".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "k": { "type": "integer", "default": 5 }
                },
                "required": ["query"]
            }),
        },
        tool(|args| Ok(text_response("Recalled 5 similar episodes"))),
    ).await?;

    // 9. Cluster Status
    server.register_tool(
        McpTool {
            name: "cluster_status".into(),
            description: "Get distributed cluster health and status".into(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        tool(|_| Ok(text_response("Cluster: 3 nodes, leader: node-0, healthy"))),
    ).await?;

    // 10. Model Update
    server.register_tool(
        McpTool {
            name: "model_update".into(),
            description: "Trigger online model fine-tuning with new data".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "model_id": { "type": "string" },
                    "training_data": { "type": "array" }
                },
                "required": ["model_id"]
            }),
        },
        tool(|args| {
            let model_id = args["model_id"].as_str().unwrap_or("default");
            Ok(text_response(format!("Model {} update queued", model_id)))
        }),
    ).await?;

    Ok(())
}
```

### Deliverables
- [ ] 10 MCP tools registered and functional
- [ ] Each tool backed by actual ruvector operations
- [ ] MCP tool integration tests
- [ ] TypeScript example using tools via MCP client

---

## Phase 4: Cognitive Robotics (Week 10-13)

### Objective
Integrate ruvector's cognitive architecture modules for autonomous robot intelligence.

### 4.1 Nervous System Integration

Connect `ruvector-nervous-system` to robot sensing/actuation cycle:
- Sensory cortex: processes PointCloud and sensor data
- Motor cortex: generates movement commands
- Prefrontal cortex: planning and decision making
- Hippocampus: spatial memory via HNSW index
- Cerebellum: fine motor control calibration

### 4.2 SONA Self-Learning

Integrate `sona` crate for autonomous skill acquisition:
- Robot experiences stored as episodes in AgentDB
- Self-optimizing neural architecture learns from rewards
- Policy improvement without manual retraining
- Transfer learning across robot configurations

### 4.3 Economy System

Use `ruvector-economy-wasm` for resource-aware planning:
- Energy budget for robot operations
- Computational budget for inference tasks
- Task prioritization based on resource availability
- Multi-robot resource negotiation

### 4.4 Delta Consensus for Multi-Robot

Use `ruvector-delta-consensus` for fleet coordination:
- Shared world model across robot fleet
- Consistent task assignment via distributed consensus
- Fault-tolerant operation with Raft leader election
- Incremental state synchronization

### Deliverables
- [ ] Nervous system integration crate
- [ ] SONA learning pipeline for robot skills
- [ ] Resource-aware task planner
- [ ] Multi-robot consensus coordination
- [ ] Demo: autonomous navigation with learning

---

## Phase 5: Edge Deployment (Week 14-16)

### Objective
Optimize for constrained deployment targets.

### 5.1 WASM Builds
- `ruvector-robotics-bridge-wasm` for browser-based simulation
- Web-based robot control interface
- WASM-compiled GNN for edge inference

### 5.2 Embedded Deployment
- Integrate `agentic-robotics-embedded` with `rvlite`
- Minimal vector search on ARM Cortex-M
- `ruvector-sparse-inference` for compact models
- Feature-flag everything non-essential

### 5.3 FPGA Acceleration
- `ruvector-fpga-transformer` for hardware-accelerated inference
- Custom attention kernels for real-time processing
- FPGA + ARM SoC deployment profile

### 5.4 Resource Profiles

| Profile | Memory | CPU | Capabilities |
|---------|--------|-----|-------------|
| Full | 1GB+ | 4+ cores | All features |
| Standard | 256MB | 2 cores | Core + GNN + search |
| Lite | 64MB | 1 core | Search + sparse inference |
| Embedded | 4MB | MCU | Minimal vector search |

### Deliverables
- [ ] WASM build for browser simulation
- [ ] Embedded build for ARM targets
- [ ] FPGA deployment configuration
- [ ] Resource profile documentation

---

## Phase 6: Benchmarking and Validation (Week 17-18)

### Objective
Comprehensive performance validation of the integrated platform.

### 6.1 Latency Targets

| Pipeline | Target | Measurement Method |
|----------|--------|-------------------|
| Sensor ingestion | <1us | CDR deserialization benchmark |
| PointCloud -> vectors | <10us | Conversion benchmark |
| HNSW search (10K vectors) | <500us | Criterion benchmark |
| GNN inference (small graph) | <1ms | RT-scheduled benchmark |
| End-to-end sensor->decision | <2ms | Full pipeline benchmark |
| MCP tool call | <5ms | JSON-RPC round-trip |

### 6.2 Throughput Targets

| Operation | Target | Conditions |
|-----------|--------|-----------|
| Message serialization | >1M/sec | CDR format |
| Vector insertions | >100K/sec | During RT control |
| Vector searches | >10K/sec | Concurrent with control |
| GNN classifications | >500/sec | RT-scheduled |

### 6.3 Memory Targets

| Deployment | Target | Includes |
|-----------|--------|---------|
| Full platform | <500MB | All modules loaded |
| Core + perception | <200MB | Without cognitive |
| Edge deployment | <100MB | rvlite + sparse |
| Embedded | <4MB | Minimal search |

### 6.4 Competitive Comparison

| Metric | ruvector+robotics | ROS2+PyTorch | Isaac Sim | Drake |
|--------|------------------|-------------|-----------|-------|
| Sensor->Decision | <2ms | 10-50ms | GPU-dependent | 5-20ms |
| Memory (edge) | <100MB | >1GB | >4GB | >500MB |
| ML native | Yes | Via bridge | CUDA only | No |
| MCP support | Yes | No | No | No |
| WASM deploy | Yes | No | No | No |
| Language safety | Rust | C++/Python | Python | C++ |

### Deliverables
- [ ] Complete benchmark suite (Criterion)
- [ ] CI-gated performance regression tests
- [ ] Competitive comparison report
- [ ] Performance optimization guide

---

## Dependency Resolution Table

| Dependency | ruvector Version | agentic-robotics Version | Unified Version | Notes |
|-----------|-----------------|-------------------------|----------------|-------|
| tokio | 1.41 | 1.47 | 1.47 | Minor bump, compatible |
| serde | 1.0 | 1.0 | 1.0 | Identical |
| serde_json | 1.0 | 1.0 | 1.0 | Identical |
| rkyv | 0.8 | 0.8 | 0.8 | Identical |
| crossbeam | 0.8 | 0.8 | 0.8 | Identical |
| rayon | 1.10 | 1.10 | 1.10 | Identical |
| parking_lot | 0.12 | 0.12 | 0.12 | Identical |
| nalgebra | 0.33 | 0.33 | 0.33 | Unify features |
| napi | 2.16 | 3.0 | Separate | Coordinate upgrade later |
| napi-derive | 2.16 | 3.0 | Separate | Coordinate upgrade later |
| criterion | 0.5 | 0.5 | 0.5 | Identical |
| thiserror | 2.0 | 2.0 | 2.0 | Identical |
| anyhow | 1.0 | 1.0 | 1.0 | Identical |
| tracing | 0.1 | 0.1 | 0.1 | Identical |
| rand | 0.8 | 0.8 | 0.8 | Identical |
| zenoh | N/A | 1.0 | 1.0 | New addition |
| rustdds | N/A | 0.11 | 0.11 | New addition |
| cdr | N/A | 0.2 | 0.2 | New addition |
| hdrhistogram | N/A | 7.5 | 7.5 | New addition |
| wide | N/A | 0.7 | 0.7 | New addition |

---

## Risk Register

| # | Risk | Probability | Impact | Mitigation |
|---|------|------------|--------|------------|
| 1 | Zenoh dependency tree adds 50+ transitive deps | High | Medium | Feature-gate behind `robotics` flag |
| 2 | Tokio version mismatch causes runtime conflicts | Low | High | Upgrade to 1.47 in Phase 0 |
| 3 | NAPI 2.16 vs 3.0 prevents unified Node.js package | Medium | Medium | Separate npm packages initially |
| 4 | Combined workspace compile time exceeds CI limits | High | Medium | Incremental builds, feature flags, split CI |
| 5 | Zenoh runtime conflicts with ruvector async code | Low | High | Isolate Zenoh in dedicated Tokio runtime |
| 6 | GNN inference exceeds RT deadline budget | Medium | High | Model quantization, early exit, async fallback |
| 7 | Memory pressure from combined modules on edge | Medium | Medium | rvlite profile, lazy module loading |
| 8 | Benchmark API drift in agentic-robotics-benchmarks | High | Low | Fix benchmarks in Phase 0 |
| 9 | MCP tool handlers need async but current API is sync | Medium | Medium | Add `AsyncToolHandler` variant |
| 10 | CDR serialization overhead for large vector payloads | Low | Low | Use rkyv for internal paths, CDR only at boundary |
| 11 | Embedded targets incompatible with ruvector std deps | Medium | Medium | Strict no_std boundary in rvlite |
| 12 | Multi-robot consensus overhead exceeds latency budget | Low | Medium | Async consensus, eventual consistency |

---

## Success Metrics

### Phase 0
- All 6 agentic-robotics crates compile in ruvector workspace
- Zero test regressions in existing ruvector tests
- CI pipeline includes robotics builds

### Phase 1
- Bridge crate converts all 3 message types (PointCloud, RobotState, Pose)
- IndexingSubscriber indexes 10K points/frame at >100 FPS
- Search latency <500us for 10K indexed vectors

### Phase 2
- End-to-end perception pipeline: sensor -> GNN -> decision
- Inference latency <1ms on standard hardware
- Attention mechanism reduces feature space by >50%

### Phase 3
- 10 MCP tools registered and callable
- Tool call round-trip <5ms
- TypeScript client can invoke all tools

### Phase 4
- Robot learns new navigation skill from 100 episodes
- Multi-robot fleet maintains consistent world model
- Resource-aware planner reduces energy usage by >20%

### Phase 5
- WASM build under 5MB
- Embedded build under 4MB
- FPGA inference <100us

### Phase 6
- All latency targets met
- All throughput targets met
- Performance regression CI gates active

---

## Open Questions

1. **Zenoh vs custom transport**: Should we use Zenoh for all inter-module communication, or keep crossbeam channels for intra-process and Zenoh only for inter-process?

2. **NAPI version strategy**: Should we upgrade all ruvector -node crates to napi 3.0 in Phase 0, or maintain version separation?

3. **Workspace partitioning**: Should agentic-robotics crates live under `crates/agentic-robotics-*/` or be renamed to `crates/ruvector-robotics-*/` for consistency?

4. **Feature flag granularity**: One `robotics` feature flag or separate flags per capability (`robotics-core`, `robotics-rt`, `robotics-mcp`)?

5. **GNN model format**: What format for pre-trained GNN models? ONNX? Custom ruvector format? In-memory only?

6. **MCP async handlers**: The current `ToolHandler` type is synchronous. Should we extend to `AsyncToolHandler` for ruvector operations that are inherently async?

7. **Testing strategy**: Integration tests between robotics and ML modules -- how to mock sensor data realistically?

8. **Multi-robot consensus protocol**: Raft (agentic-robotics-core via Zenoh) vs Delta Consensus (ruvector-delta-consensus)? Or both for different consistency levels?

9. **WASM deployment scope**: Which ruvector modules should be available in the WASM robotics build? Full GNN or inference-only?

10. **Formal verification**: Can `lean-agentic` verify safety properties of the combined robotics+ML pipeline?
