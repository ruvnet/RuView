# ruvector-robotics

Unified cognitive robotics platform built on ruvector's vector database, graph neural networks, and self-learning infrastructure.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                ruvector-robotics                 │
├────────────┬────────────┬──────────┬────────────┤
│   bridge   │ perception │cognitive │    mcp     │
├────────────┼────────────┼──────────┼────────────┤
│ Point3D    │ SceneGraph │ Behavior │ Tool       │
│ PointCloud │  Builder   │  Trees   │ Registry   │
│ RobotState │ Obstacle   │ Cognitive│ 15+ Tools  │
│ Pose       │  Detector  │  Core    │ MCP Schema │
│ SceneGraph │ Anomaly    │ Memory   │            │
│ Trajectory │  Detection │ Skills   │            │
│ Spatial    │ Trajectory │ Swarm    │            │
│  Index     │  Predict   │ World    │            │
│ Pipeline   │            │  Model   │            │
│ Converters │            │ Decision │            │
└────────────┴────────────┴──────────┴────────────┘
```

## Modules

### bridge — Core Types & Spatial Operations
- **Types**: Point3D, PointCloud, RobotState, Pose, Quaternion, SensorFrame, OccupancyGrid, SceneObject, SceneGraph, Trajectory
- **SpatialIndex**: Brute-force kNN and radius search with Euclidean/Cosine/Manhattan metrics
- **Converters**: Bidirectional conversion between robotics messages and flat vectors
- **Pipeline**: Lightweight perception pipeline with obstacle detection and trajectory prediction

### perception — Scene Understanding
- **SceneGraphBuilder**: Spatial hash clustering with union-find for point cloud segmentation
- **ObstacleDetector**: Grid-based obstacle detection with heuristic classification (Static/Dynamic/Unknown)
- **PerceptionPipeline**: Full perception stack with obstacle detection, scene graph construction, attention focusing, anomaly detection

### cognitive — Autonomous Intelligence
- **BehaviorTree**: Composable reactive control structures (Sequence, Selector, Parallel, Decorators)
- **CognitiveCore**: Perceive-Think-Act-Learn loop with dual-process theory (Reactive/Deliberative/Emergency modes)
- **DecisionEngine**: Multi-criteria utility-based action selection (reward, risk, energy, curiosity)
- **MemorySystem**: Three-tier memory (Working, Episodic, Semantic) with similarity-based recall
- **SkillLearning**: Learning-from-demonstration with trajectory averaging and reinforcement
- **SwarmIntelligence**: Multi-robot coordination with task allocation and formation control
- **WorldModel**: Object tracking, occupancy mapping, and state prediction

### mcp — AI Agent Integration
- **ToolRegistry**: 15 registered MCP tools across 6 categories
- **Categories**: Perception, Navigation, Cognition, Swarm, Memory, Planning
- **Schema**: Full MCP-compatible JSON schema generation

## Quick Start

```rust
use ruvector_robotics::bridge::{Point3D, PointCloud, SpatialIndex};

// Create sensor data
let cloud = PointCloud::new(
    vec![Point3D::new(1.0, 2.0, 3.0), Point3D::new(4.0, 5.0, 6.0)],
    1000,
);

// Index and search
let mut index = SpatialIndex::new(3);
index.insert_point_cloud(&cloud);
let nearest = index.search_nearest(&[2.0, 3.0, 4.0], 1).unwrap();
```

```rust
use ruvector_robotics::cognitive::{BehaviorTree, BehaviorNode, BehaviorStatus};

// Build a patrol behavior tree
let tree = BehaviorTree::new(BehaviorNode::Sequence(vec![
    BehaviorNode::Action("scan_environment".into()),
    BehaviorNode::Action("move_to_waypoint".into()),
    BehaviorNode::Action("report_status".into()),
]));
```

## Examples

Run any example from the repository root:

```bash
# Practical
cargo run -p ruvector-robotics-examples --bin 01_basic_perception
cargo run -p ruvector-robotics-examples --bin 02_obstacle_avoidance

# Intermediate
cargo run -p ruvector-robotics-examples --bin 03_scene_graph
cargo run -p ruvector-robotics-examples --bin 04_behavior_tree

# Advanced
cargo run -p ruvector-robotics-examples --bin 05_cognitive_robot
cargo run -p ruvector-robotics-examples --bin 06_swarm_coordination
cargo run -p ruvector-robotics-examples --bin 07_skill_learning

# Exotic
cargo run -p ruvector-robotics-examples --bin 08_world_model
cargo run -p ruvector-robotics-examples --bin 09_mcp_tools
cargo run -p ruvector-robotics-examples --bin 10_full_pipeline
```

## Testing

```bash
# Run all tests
cargo test -p ruvector-robotics

# Run benchmarks
cargo bench -p ruvector-robotics
```

## Design Philosophy

This crate is designed thinking 50 years into the future while running on today's hardware:

1. **Zero external robotics deps** — All types are self-contained. No ROS/ROS2 dependency.
2. **Vector-first architecture** — Everything converts to flat vectors for indexing and search.
3. **Cognitive-inspired** — Dual-process theory, episodic memory, behavior trees from cognitive science.
4. **Swarm-native** — Multi-robot coordination built in from the start.
5. **MCP-ready** — All capabilities exposed as AI-agent-callable tools.
6. **No-std friendly core types** — Bridge types use only serde + standard library.

## Performance Targets

| Operation | Target | Notes |
|-----------|--------|-------|
| Point cloud indexing | 10K pts < 5ms | Brute-force flat index |
| kNN search (k=10) | < 1ms on 10K pts | Sorted partial select |
| Obstacle detection | < 10ms on 10K pts | Spatial hash + union-find |
| Scene graph build | < 5ms for 100 objects | Pairwise distance |
| Behavior tree tick | < 100μs for 50 nodes | Recursive evaluation |
| Memory recall | < 1ms for 1K items | Dot-product similarity |

## License

MIT
