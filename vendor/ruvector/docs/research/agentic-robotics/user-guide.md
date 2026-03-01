# ruvector-robotics User Guide

## Table of Contents

1. [Getting Started](#getting-started)
2. [Bridge Module](#bridge-module)
3. [Perception Module](#perception-module)
4. [Cognitive Module](#cognitive-module)
5. [MCP Module](#mcp-module)
6. [Integration Patterns](#integration-patterns)
7. [Advanced Usage](#advanced-usage)

---

## Getting Started

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ruvector-robotics = { path = "crates/ruvector-robotics" }
```

### Minimal Example

```rust
use ruvector_robotics::bridge::{Point3D, PointCloud, SpatialIndex};
use ruvector_robotics::perception::PerceptionPipeline;

fn main() {
    // 1. Create sensor data
    let points = vec![
        Point3D::new(1.0, 0.0, 0.0),
        Point3D::new(0.0, 1.0, 0.0),
        Point3D::new(5.0, 5.0, 5.0),
    ];
    let cloud = PointCloud::new(points, 1000);

    // 2. Index for spatial search
    let mut index = SpatialIndex::new(3);
    index.insert_point_cloud(&cloud);

    // 3. Find nearest obstacles
    let nearest = index.search_nearest(&[0.0, 0.0, 0.0], 2).unwrap();
    println!("Nearest 2 points: {:?}", nearest);

    // 4. Detect obstacles
    let pipeline = PerceptionPipeline::default();
    let obstacles = pipeline
        .detect_obstacles(&cloud, [0.0, 0.0, 0.0], 10.0)
        .unwrap();
    println!("Detected {} obstacles", obstacles.len());
}
```

---

## Bridge Module

The bridge module provides core types shared across all robotics subsystems.

### Core Types

| Type | Description | Fields |
|------|-------------|--------|
| `Point3D` | 3D point (f32) | x, y, z |
| `PointCloud` | Collection of points | points, intensities, normals, timestamp_us |
| `RobotState` | Kinematic state | position, velocity, acceleration, timestamp_us |
| `Pose` | 6-DOF pose | position, orientation (Quaternion) |
| `SensorFrame` | Synchronized sensor bundle | cloud, state, pose |
| `OccupancyGrid` | 2D occupancy map | width, height, resolution, data |
| `SceneObject` | Detected object | id, center, extent, confidence, label |
| `SceneGraph` | Object relationships | objects, edges |
| `Trajectory` | Predicted path | waypoints, timestamps, confidence |

### SpatialIndex

A flat brute-force index for nearest-neighbor search:

```rust
use ruvector_robotics::bridge::{SpatialIndex, DistanceMetric};

// Create index with cosine distance
let mut index = SpatialIndex::with_metric(128, DistanceMetric::Cosine);

// Insert vectors
index.insert_vectors(&[vec![1.0; 128], vec![0.5; 128]]);

// k-NN search
let results = index.search_nearest(&vec![0.9; 128], 5).unwrap();

// Radius search
let within = index.search_radius(&vec![0.9; 128], 0.5).unwrap();
```

### Converters

Convert between robotics types and flat vectors:

```rust
use ruvector_robotics::bridge::{PointCloud, Point3D, converters};

let cloud = PointCloud::new(vec![Point3D::new(1.0, 2.0, 3.0)], 0);

// To vectors for indexing
let vecs = converters::point_cloud_to_vectors(&cloud);
// -> [[1.0, 2.0, 3.0]]

// Back to point cloud
let cloud2 = converters::vectors_to_point_cloud(&vecs, 0).unwrap();
```

---

## Perception Module

### Obstacle Detection

```rust
use ruvector_robotics::perception::{ObstacleDetector, PerceptionConfig};

let config = PerceptionConfig::default();
let detector = ObstacleDetector::new(config.obstacle);

// Detect from point cloud
let obstacles = detector.detect(&cloud, &[0.0, 0.0, 0.0]);

// Classify obstacles
let classified = detector.classify_obstacles(&obstacles);
for c in &classified {
    println!("{:?}: {:?} (confidence: {:.2})", c.class, c.obstacle.center, c.confidence);
}
```

### Scene Graph Construction

```rust
use ruvector_robotics::perception::{SceneGraphBuilder, PerceptionConfig};
use ruvector_robotics::bridge::SceneObject;

let config = PerceptionConfig::default();
let builder = SceneGraphBuilder::new(config.scene_graph);

// From point cloud (clusters -> objects -> edges)
let graph = builder.build_from_point_cloud(&cloud);

// From pre-detected objects
let objects = vec![
    SceneObject::new(0, [0.0, 0.0, 0.0], [0.5, 0.5, 0.5]),
    SceneObject::new(1, [3.0, 0.0, 0.0], [0.5, 0.5, 0.5]),
];
let graph = builder.build_from_objects(&objects);
```

### Full Perception Pipeline

```rust
use ruvector_robotics::perception::PerceptionPipeline;

let pipeline = PerceptionPipeline::default();

// Obstacle detection + clustering
let obstacles = pipeline.detect_obstacles(&cloud, [0.0, 0.0, 0.0], 20.0).unwrap();

// Trajectory prediction
let traj = pipeline.predict_trajectory([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 10, 0.1).unwrap();

// Attention focusing
let focused = pipeline.focus_attention(&cloud, [5.0, 5.0, 0.0], 2.0).unwrap();

// Anomaly detection
let anomalies = pipeline.detect_anomalies(&cloud).unwrap();
```

---

## Cognitive Module

### Behavior Trees

Composable reactive control structures:

```rust
use ruvector_robotics::cognitive::{BehaviorTree, BehaviorNode, BehaviorStatus};

// Build a patrol tree
let root = BehaviorNode::Sequence(vec![
    BehaviorNode::Condition("is_battery_ok".into()),
    BehaviorNode::Selector(vec![
        BehaviorNode::Sequence(vec![
            BehaviorNode::Condition("obstacle_detected".into()),
            BehaviorNode::Action("avoid_obstacle".into()),
        ]),
        BehaviorNode::Action("move_forward".into()),
    ]),
    BehaviorNode::Action("update_map".into()),
]);

let mut tree = BehaviorTree::new(root);

// Set condition and action states
tree.set_condition("is_battery_ok", true);
tree.set_condition("obstacle_detected", false);
tree.set_action_result("move_forward", BehaviorStatus::Success);
tree.set_action_result("update_map", BehaviorStatus::Success);

let status = tree.tick();
assert_eq!(status, BehaviorStatus::Success);
```

### Cognitive Core

The central perceive-think-act-learn loop:

```rust
use ruvector_robotics::cognitive::{
    CognitiveCore, CognitiveConfig, CognitiveMode, Percept, Outcome,
};

let config = CognitiveConfig {
    mode: CognitiveMode::Deliberative,
    attention_threshold: 0.5,
    learning_rate: 0.01,
    max_percepts: 100,
};
let mut core = CognitiveCore::new(config);

// 1. Perceive
let percept = Percept {
    source: "lidar".into(),
    data: vec![1.0, 2.0, 3.0],
    confidence: 0.95,
    timestamp: 1000,
};
core.perceive(percept);

// 2. Think -> Decision
if let Some(decision) = core.think() {
    println!("Decision: {} (utility: {:.2})", decision.reasoning, decision.utility);

    // 3. Act
    let cmd = core.act(decision);
    println!("Action: {:?}", cmd.action);

    // 4. Learn
    core.learn(Outcome {
        success: true,
        reward: 1.0,
        description: "Obstacle avoided".into(),
    });
}
```

### Memory System

Three-tier memory architecture:

```rust
use ruvector_robotics::cognitive::{WorkingMemory, EpisodicMemory, SemanticMemory, MemoryItem, Episode};

// Working memory (bounded buffer)
let mut working = WorkingMemory::new(10);
working.add(MemoryItem {
    key: "obstacle_1".into(),
    data: vec![1.0, 2.0, 3.0],
    importance: 0.8,
    timestamp: 1000,
    access_count: 0,
});

// Episodic memory (experience replay)
let mut episodic = EpisodicMemory::new(100);
episodic.store(Episode {
    percepts: vec![vec![1.0, 2.0], vec![3.0, 4.0]],
    actions: vec!["move".into(), "turn".into()],
    reward: 1.0,
    timestamp: 1000,
});
let similar = episodic.recall_similar(&[1.0, 2.0], 3);

// Semantic memory (concept storage)
let mut semantic = SemanticMemory::new();
semantic.store("obstacle", vec![1.0, 0.0, 0.0]);
semantic.store("goal", vec![0.0, 1.0, 0.0]);
let nearest = semantic.find_similar(&[0.9, 0.1, 0.0], 1);
```

### Swarm Coordination

```rust
use ruvector_robotics::cognitive::{
    SwarmCoordinator, SwarmConfig, RobotCapabilities, SwarmTask, Formation, FormationType,
};

let mut swarm = SwarmCoordinator::new(SwarmConfig {
    max_robots: 10,
    communication_range: 50.0,
    consensus_threshold: 0.6,
});

// Register robots
swarm.register_robot(RobotCapabilities {
    id: 1,
    max_speed: 2.0,
    payload: 5.0,
    sensors: vec!["lidar".into(), "camera".into()],
});

// Assign tasks
let tasks = vec![SwarmTask {
    id: 1,
    description: "Survey area A".into(),
    location: [10.0, 20.0, 0.0],
    required_capabilities: vec!["camera".into()],
    priority: 5,
}];
let assignments = swarm.assign_tasks(&tasks);

// Compute formation
let formation = Formation {
    formation_type: FormationType::Circle,
    spacing: 3.0,
    center: [0.0, 0.0, 0.0],
};
let positions = swarm.compute_formation(&formation);
```

---

## MCP Module

### Tool Registry

```rust
use ruvector_robotics::mcp::{RoboticsToolRegistry, ToolCategory};

let registry = RoboticsToolRegistry::new();

// List all 15 tools
for tool in registry.list_tools() {
    println!("{}: {}", tool.name, tool.description);
}

// Filter by category
let perception_tools = registry.list_by_category(ToolCategory::Perception);
println!("Perception tools: {}", perception_tools.len());

// Get MCP schema
let schema = registry.to_mcp_schema();
println!("{}", serde_json::to_string_pretty(&schema).unwrap());
```

---

## Integration Patterns

### Sensor → Perception → Cognition → Action

```rust
use ruvector_robotics::bridge::*;
use ruvector_robotics::perception::*;
use ruvector_robotics::cognitive::*;

// 1. Sensor data arrives
let cloud = PointCloud::new(/* sensor points */, timestamp);

// 2. Perception processes it
let pipeline = PerceptionPipeline::default();
let obstacles = pipeline.detect_obstacles(&cloud, robot_pos, 20.0).unwrap();

// 3. Cognitive core makes decisions
let mut core = CognitiveCore::new(CognitiveConfig::default());
for obs in &obstacles {
    core.perceive(Percept {
        source: "perception".into(),
        data: obs.position.to_vec(),
        confidence: obs.confidence as f64,
        timestamp: 0,
    });
}

// 4. Think and act
if let Some(decision) = core.think() {
    let action = core.act(decision);
    // Send action to robot motors
}
```

### Multi-Robot Coordination

```rust
// Each robot runs its own cognitive core
// SwarmCoordinator manages task allocation across robots
// ConsensusResult enables group decision-making
```

---

## Advanced Usage

### Custom Distance Metrics

The SpatialIndex supports Euclidean, Cosine, and Manhattan distances. Choose based on your data:
- **Euclidean**: Best for spatial point clouds (default)
- **Cosine**: Best for high-dimensional feature vectors
- **Manhattan**: Best for grid-aligned environments

### Behavior Tree Patterns

Common patterns:
- **Patrol**: `Sequence[CheckBattery, Selector[AvoidObstacle, MoveForward], UpdateMap]`
- **Explore**: `Selector[GoToFrontier, RandomWalk, ReturnToBase]`
- **Emergency**: `Sequence[StopMotors, SendAlert, WaitForHelp]`

### Memory Consolidation

The three-tier memory system models human memory:
- **Working Memory**: Current sensor data, bounded to prevent overload
- **Episodic Memory**: Past experiences for pattern matching
- **Semantic Memory**: Learned concepts and relationships

### Performance Tuning

Key parameters to adjust:
- `SceneGraphConfig::cluster_radius` — Smaller = more objects, slower
- `ObstacleConfig::safety_margin` — Larger = more conservative
- `CognitiveConfig::attention_threshold` — Higher = focus on important percepts
- `SwarmConfig::consensus_threshold` — Higher = more agreement required

---

## API Reference

Full API documentation is generated with `cargo doc -p ruvector-robotics --open`.
