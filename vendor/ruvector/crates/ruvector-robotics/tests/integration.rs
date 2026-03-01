//! Integration tests for the unified ruvector-robotics crate.
//!
//! Each test exercises a cross-module workflow to verify that the public API
//! composes correctly.

use ruvector_robotics::bridge::{
    OccupancyGrid, Point3D, PointCloud, SceneEdge, SceneGraph, SceneObject, SpatialIndex,
};
use ruvector_robotics::cognitive::{
    BehaviorNode, BehaviorStatus, BehaviorTree, CognitiveConfig, CognitiveCore, CognitiveMode,
    Demonstration, EpisodicMemory, Episode, Formation, FormationType, MemoryItem, Outcome,
    Percept, RobotCapabilities, SkillLibrary, SwarmConfig, SwarmCoordinator, SwarmTask,
    TrackedObject, WorkingMemory, WorldModel,
};
use ruvector_robotics::mcp::{RoboticsToolRegistry, ToolCategory};
use ruvector_robotics::perception::{PerceptionConfig, PerceptionPipeline};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_cloud(pts: &[[f32; 3]], timestamp: i64) -> PointCloud {
    let points = pts.iter().map(|p| Point3D::new(p[0], p[1], p[2])).collect();
    PointCloud::new(points, timestamp)
}

fn cluster_pts(center: [f32; 3], n: usize, spread: f32) -> Vec<[f32; 3]> {
    let mut pts = Vec::new();
    for i in 0..n {
        let f = i as f32 / n as f32;
        pts.push([
            center[0] + spread * (f * 6.28).cos(),
            center[1] + spread * (f * 6.28).sin(),
            center[2],
        ]);
    }
    pts
}

// ---------------------------------------------------------------------------
// 1. Bridge types roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_bridge_types_roundtrip() {
    // Point3D
    let p = Point3D::new(1.0, 2.0, 3.0);
    let json = serde_json::to_string(&p).unwrap();
    let p2: Point3D = serde_json::from_str(&json).unwrap();
    assert!((p.x - p2.x).abs() < f32::EPSILON);

    // PointCloud
    let cloud = PointCloud::new(vec![p, Point3D::new(4.0, 5.0, 6.0)], 999);
    let json = serde_json::to_string(&cloud).unwrap();
    let cloud2: PointCloud = serde_json::from_str(&json).unwrap();
    assert_eq!(cloud2.len(), 2);
    assert_eq!(cloud2.timestamp_us, 999);

    // SceneObject
    let obj = SceneObject::new(0, [1.0, 2.0, 3.0], [0.5, 0.5, 0.5]);
    let json = serde_json::to_string(&obj).unwrap();
    let obj2: SceneObject = serde_json::from_str(&json).unwrap();
    assert_eq!(obj2.id, 0);

    // SceneGraph
    let graph = SceneGraph::new(
        vec![obj.clone()],
        vec![SceneEdge {
            from: 0,
            to: 0,
            distance: 0.0,
            relation: "self".into(),
        }],
        1000,
    );
    let json = serde_json::to_string(&graph).unwrap();
    let graph2: SceneGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(graph2.objects.len(), 1);
    assert_eq!(graph2.edges.len(), 1);

    // OccupancyGrid
    let mut grid = OccupancyGrid::new(5, 5, 0.5);
    grid.set(2, 2, 0.9);
    let json = serde_json::to_string(&grid).unwrap();
    let grid2: OccupancyGrid = serde_json::from_str(&json).unwrap();
    assert!((grid2.get(2, 2).unwrap() - 0.9).abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// 2. Spatial index insert & search
// ---------------------------------------------------------------------------

#[test]
fn test_spatial_index_insert_search() {
    let mut index = SpatialIndex::new(3);
    let n = 1000;
    let vecs: Vec<Vec<f32>> = (0..n)
        .map(|i| {
            let f = i as f32;
            vec![f * 0.01, f * 0.02, f * 0.03]
        })
        .collect();
    index.insert_vectors(&vecs);
    assert_eq!(index.len(), n);

    // kNN: nearest to origin should be index 0
    let results = index.search_nearest(&[0.0, 0.0, 0.0], 5).unwrap();
    assert_eq!(results.len(), 5);
    assert_eq!(results[0].0, 0);
    assert!(results[0].1 < 0.001);

    // Results sorted by distance
    for w in results.windows(2) {
        assert!(w[0].1 <= w[1].1);
    }

    // Radius search
    let within = index.search_radius(&[0.0, 0.0, 0.0], 1.0).unwrap();
    assert!(!within.is_empty());
    for (_, d) in &within {
        assert!(*d <= 1.0);
    }
}

// ---------------------------------------------------------------------------
// 3. Perception pipeline end-to-end
// ---------------------------------------------------------------------------

#[test]
fn test_perception_pipeline_end_to_end() {
    let mut pipeline = PerceptionPipeline::new(PerceptionConfig::default());

    let mut pts = Vec::new();
    pts.extend(cluster_pts([2.0, 0.0, 0.0], 10, 0.2));
    pts.extend(cluster_pts([8.0, 5.0, 0.0], 10, 0.2));
    let cloud = make_cloud(&pts, 500);

    let (obstacles, graph) = pipeline.process(&cloud, &[0.0, 0.0, 0.0]);
    assert!(!obstacles.is_empty());
    assert!(!graph.objects.is_empty());
    assert_eq!(pipeline.frames_processed(), 1);

    // Classify
    let classified = pipeline.classify(&obstacles);
    assert_eq!(classified.len(), obstacles.len());

    // Second frame increments counter
    let _ = pipeline.process(&cloud, &[0.0, 0.0, 0.0]);
    assert_eq!(pipeline.frames_processed(), 2);
}

// ---------------------------------------------------------------------------
// 4. Cognitive loop
// ---------------------------------------------------------------------------

#[test]
fn test_cognitive_loop() {
    let mut core = CognitiveCore::new(CognitiveConfig {
        mode: CognitiveMode::Reactive,
        attention_threshold: 0.3,
        learning_rate: 0.05,
        max_percepts: 10,
    });

    // Perceive
    core.perceive(Percept {
        source: "lidar".into(),
        data: vec![2.0, 1.0, 0.0],
        confidence: 0.9,
        timestamp: 100,
    });
    assert_eq!(core.percept_count(), 1);

    // Think
    let decision = core.think().expect("should produce a decision");
    assert!(decision.utility > 0.0);

    // Act
    let cmd = core.act(decision);
    assert!(cmd.confidence > 0.0);

    // Learn
    core.learn(Outcome {
        success: true,
        reward: 1.0,
        description: "test".into(),
    });
    assert!(core.cumulative_reward() > 0.0);
    assert_eq!(core.decision_count(), 1);
}

// ---------------------------------------------------------------------------
// 5. Behavior tree sequence
// ---------------------------------------------------------------------------

#[test]
fn test_behavior_tree_sequence() {
    let seq = BehaviorNode::Sequence(vec![
        BehaviorNode::Condition("battery_ok".into()),
        BehaviorNode::Action("move".into()),
        BehaviorNode::Action("report".into()),
    ]);
    let mut tree = BehaviorTree::new(seq);

    // All success
    tree.set_condition("battery_ok", true);
    tree.set_action_result("move", BehaviorStatus::Success);
    tree.set_action_result("report", BehaviorStatus::Success);
    assert_eq!(tree.tick(), BehaviorStatus::Success);

    // Condition fails -> Failure
    tree.set_condition("battery_ok", false);
    assert_eq!(tree.tick(), BehaviorStatus::Failure);

    // Running propagates
    tree.set_condition("battery_ok", true);
    tree.set_action_result("move", BehaviorStatus::Running);
    assert_eq!(tree.tick(), BehaviorStatus::Running);

    assert_eq!(tree.context().tick_count, 3);
}

// ---------------------------------------------------------------------------
// 6. Memory store & recall
// ---------------------------------------------------------------------------

#[test]
fn test_memory_store_recall() {
    // Working memory
    let mut wm = WorkingMemory::new(3);
    for i in 0..5 {
        wm.add(MemoryItem {
            key: format!("item_{}", i),
            data: vec![i as f64],
            importance: i as f64 * 0.2,
            timestamp: i as i64 * 100,
            access_count: 0,
        });
    }
    assert_eq!(wm.len(), 3); // bounded

    // Access increments count
    let item = wm.get("item_4").expect("most important should survive");
    assert_eq!(item.access_count, 1);

    // Episodic memory
    let mut em = EpisodicMemory::new();
    em.store(Episode {
        percepts: vec![vec![1.0, 0.0, 0.0]],
        actions: vec!["move".into()],
        reward: 1.0,
        timestamp: 100,
    });
    em.store(Episode {
        percepts: vec![vec![0.0, 1.0, 0.0]],
        actions: vec!["turn".into()],
        reward: 0.5,
        timestamp: 200,
    });
    let recalled = em.recall_similar(&[1.0, 0.0, 0.0], 1);
    assert_eq!(recalled.len(), 1);
    assert_eq!(recalled[0].actions[0], "move");
}

// ---------------------------------------------------------------------------
// 7. Skill learning cycle
// ---------------------------------------------------------------------------

#[test]
fn test_skill_learning_cycle() {
    let mut lib = SkillLibrary::new();

    let demos = vec![
        Demonstration {
            trajectory: vec![[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 2.0, 0.0]],
            timestamps: vec![0, 100, 200],
            metadata: "demo_1".into(),
        },
        Demonstration {
            trajectory: vec![[0.0, 0.0, 0.0], [1.2, 0.8, 0.0], [2.1, 1.9, 0.0]],
            timestamps: vec![0, 110, 210],
            metadata: "demo_2".into(),
        },
    ];

    // Learn
    let skill = lib.learn_from_demonstration("reach", &demos);
    assert_eq!(skill.trajectory.len(), 3);
    assert!(skill.confidence > 0.0);

    // Execute
    let traj = lib.execute_skill("reach").unwrap();
    assert_eq!(traj.len(), 3);
    assert_eq!(lib.get("reach").unwrap().execution_count, 1);

    // Improve
    let before = lib.get("reach").unwrap().confidence;
    lib.improve_skill("reach", 0.1);
    let after = lib.get("reach").unwrap().confidence;
    assert!(after > before);

    // Missing skill
    assert!(lib.execute_skill("nonexistent").is_none());
}

// ---------------------------------------------------------------------------
// 8. Swarm task assignment
// ---------------------------------------------------------------------------

#[test]
fn test_swarm_task_assignment() {
    let mut coord = SwarmCoordinator::new(SwarmConfig::default());

    for i in 0..4 {
        coord.register_robot(RobotCapabilities {
            id: i,
            max_speed: 1.0 + i as f64 * 0.5,
            payload: 5.0,
            sensors: vec!["lidar".into(), "camera".into()],
        });
    }
    assert_eq!(coord.robot_count(), 4);

    let tasks = vec![
        SwarmTask {
            id: 10,
            description: "scan".into(),
            location: [3.0, 4.0, 0.0],
            required_capabilities: vec!["lidar".into()],
            priority: 8,
        },
        SwarmTask {
            id: 11,
            description: "photo".into(),
            location: [5.0, 0.0, 0.0],
            required_capabilities: vec!["camera".into()],
            priority: 5,
        },
    ];

    let assignments = coord.assign_tasks(&tasks);
    assert_eq!(assignments.len(), 2);

    // Formation
    let formation = Formation {
        formation_type: FormationType::Circle,
        spacing: 2.0,
        center: [0.0, 0.0, 0.0],
    };
    let positions = coord.compute_formation(&formation);
    assert_eq!(positions.len(), 4);
}

// ---------------------------------------------------------------------------
// 9. World model tracking
// ---------------------------------------------------------------------------

#[test]
fn test_world_model_tracking() {
    let mut world = WorldModel::new(20, 0.5);

    // Update objects
    world.update_object(TrackedObject {
        id: 1,
        position: [2.0, 3.0, 0.0],
        velocity: [1.0, 0.0, 0.0],
        last_seen: 1000,
        confidence: 0.9,
        label: "rover".into(),
    });
    world.update_object(TrackedObject {
        id: 2,
        position: [8.0, 1.0, 0.0],
        velocity: [0.0, 0.5, 0.0],
        last_seen: 500,
        confidence: 0.7,
        label: "box".into(),
    });
    assert_eq!(world.object_count(), 2);

    // Predict
    let pred = world.predict_state(1, 2.0).unwrap();
    assert!((pred.position[0] - 4.0).abs() < 1e-6);
    assert!(pred.confidence < 0.9); // decayed

    // Missing object
    assert!(world.predict_state(99, 1.0).is_none());

    // Occupancy
    world.update_occupancy(5, 5, 1.0);
    assert!((world.get_occupancy(5, 5).unwrap() - 1.0).abs() < f32::EPSILON);

    // Path clearance
    assert!(world.is_path_clear([0, 0], [4, 4])); // no obstacle in path
    assert!(!world.is_path_clear([0, 5], [19, 5])); // (5,5) is blocked

    // Remove stale
    let removed = world.remove_stale_objects(1200, 300);
    assert_eq!(removed, 1); // id=2 is stale
    assert!(world.get_object(2).is_none());
    assert!(world.get_object(1).is_some());
}

// ---------------------------------------------------------------------------
// 10. MCP registry
// ---------------------------------------------------------------------------

#[test]
fn test_mcp_registry() {
    let registry = RoboticsToolRegistry::new();

    // Has built-in tools
    assert!(registry.list_tools().len() >= 10);

    // Look up by name
    let tool = registry.get_tool("detect_obstacles").unwrap();
    assert_eq!(tool.category, ToolCategory::Perception);
    assert!(!tool.parameters.is_empty());

    // Category filtering
    let perception = registry.list_by_category(ToolCategory::Perception);
    assert!(!perception.is_empty());
    for t in &perception {
        assert_eq!(t.category, ToolCategory::Perception);
    }

    // MCP schema
    let schema = registry.to_mcp_schema();
    let tools = schema["tools"].as_array().unwrap();
    assert!(!tools.is_empty());
    for tool_schema in tools {
        assert!(tool_schema["name"].is_string());
        assert!(tool_schema["inputSchema"].is_object());
    }
}
