//! Integration tests for the ruvector-robotics crate.
//!
//! Tests the real crate APIs across bridge types, perception pipeline,
//! cognitive loop, MCP tool registry + executor, planning, sensor fusion,
//! Gaussian splatting, and swarm coordination.
//!
//! Run with: cargo test --test robotics_integration -p ruvector-robotics

use std::sync::{Arc, Mutex};
use std::time::Instant;

use ruvector_robotics::bridge::{
    GaussianConfig, Point3D, PointCloud, SceneObject, SpatialIndex,
};
use ruvector_robotics::bridge::gaussian::gaussians_from_cloud;
use ruvector_robotics::cognitive::behavior_tree::{
    BehaviorNode, BehaviorStatus, BehaviorTree, DecoratorType,
};
use ruvector_robotics::cognitive::{
    ActionOption, DecisionConfig, DecisionEngine, Demonstration, EpisodicMemory,
    Episode, SkillLibrary, SwarmConfig, SwarmCoordinator, SwarmTask, RobotCapabilities,
    TrackedObject, WorldModel,
};
use ruvector_robotics::mcp::{RoboticsToolRegistry, ToolRequest};
use ruvector_robotics::mcp::executor::ToolExecutor;
use ruvector_robotics::perception::PerceptionPipeline;
use ruvector_robotics::perception::sensor_fusion::{fuse_clouds, FusionConfig};
use ruvector_robotics::planning;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn pseudo_random_f32(seed: u64, index: usize) -> f32 {
    let h = seed
        .wrapping_mul(index as u64 + 1)
        .wrapping_mul(0x5DEECE66D)
        .wrapping_add(0xB);
    ((h % 10000) as f32) / 10000.0
}

fn generate_point_cloud_around(center: Point3D, n: usize, spread: f32) -> Vec<Point3D> {
    (0..n)
        .map(|i| {
            Point3D::new(
                center.x + (pseudo_random_f32(42, i * 3) - 0.5) * spread,
                center.y + (pseudo_random_f32(42, i * 3 + 1) - 0.5) * spread,
                center.z + (pseudo_random_f32(42, i * 3 + 2) - 0.5) * spread,
            )
        })
        .collect()
}

// ===========================================================================
// TESTS
// ===========================================================================

/// Test 1: End-to-end perception pipeline using actual crate API.
#[test]
fn test_perception_pipeline_end_to_end() {
    let mut points = generate_point_cloud_around(Point3D::new(2.0, 3.0, 0.0), 50, 0.8);
    points.extend(generate_point_cloud_around(Point3D::new(8.0, 7.0, 0.0), 40, 0.6));
    let cloud = PointCloud::new(points, 1_000_000);

    let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);

    // Detect obstacles using the real API
    let obstacles = pipe.detect_obstacles(&cloud, [0.0, 0.0, 0.0], 20.0).unwrap();
    assert!(!obstacles.is_empty(), "Should detect at least one obstacle");

    // Build scene graph using the real API
    let scene_objects: Vec<SceneObject> = obstacles
        .iter()
        .map(|obs| SceneObject::new(obs.id as usize, obs.position, [obs.radius; 3]))
        .collect();
    let graph = pipe.build_scene_graph(&scene_objects, 10.0).unwrap();
    assert_eq!(graph.objects.len(), scene_objects.len());
    if scene_objects.len() > 1 {
        assert!(!graph.edges.is_empty(), "Should have edges between nearby objects");
    }

    // Predict trajectory using the real API
    let traj = pipe
        .predict_trajectory([0.0, 0.0, 0.0], [1.0, 0.5, 0.0], 10, 0.1)
        .unwrap();
    assert_eq!(traj.len(), 10);
    assert!((traj.waypoints[0][0] - 0.1).abs() < 1e-9);
}

/// Test 2: Cognitive pipeline — decision engine selects action from obstacles.
#[test]
fn test_cognitive_perceive_think_act() {
    let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);

    let mut points = generate_point_cloud_around(Point3D::new(6.0, 5.0, 0.0), 20, 0.3);
    points.extend(generate_point_cloud_around(Point3D::new(10.0, 10.0, 0.0), 15, 0.3));
    let cloud = PointCloud::new(points, 0);

    let obstacles = pipe.detect_obstacles(&cloud, [5.0, 5.0, 0.0], 20.0).unwrap();
    let min_dist = obstacles.iter().map(|o| o.distance).fold(f64::MAX, f64::min);

    // Use the real DecisionEngine
    let engine = DecisionEngine::new(DecisionConfig::default());
    let options = vec![
        ActionOption { name: "proceed_fast".into(), reward: 8.0, risk: 0.9, energy_cost: 0.5, novelty: 0.0 },
        ActionOption { name: "slow_down".into(), reward: 5.0, risk: 0.2, energy_cost: 0.3, novelty: 0.0 },
        ActionOption { name: "stop".into(), reward: 2.0, risk: 0.0, energy_cost: 0.0, novelty: 0.0 },
    ];
    let (best_idx, _utility) = engine.evaluate(&options).unwrap();
    assert!(!options[best_idx].name.is_empty());

    // With obstacles nearby, a conservative engine should not pick "proceed_fast"
    if min_dist < 3.0 {
        let conservative = DecisionEngine::new(DecisionConfig {
            risk_aversion: 5.0,
            ..Default::default()
        });
        let (best_idx, _) = conservative.evaluate(&options).unwrap();
        assert_ne!(options[best_idx].name, "proceed_fast");
    }
}

/// Test 3: Episodic memory store and recall using actual crate API.
#[test]
fn test_episodic_memory_store_recall() {
    let mut memory = EpisodicMemory::new();

    for i in 0..20 {
        let mut percept = vec![0.0f64; 32];
        percept[i % 32] = 1.0;
        percept[(i + 1) % 32] = 0.5;
        memory.store(Episode {
            percepts: vec![percept],
            actions: vec![format!("action_{}", i)],
            reward: i as f64 * 0.1,
            timestamp: i as i64 * 1000,
        });
    }

    let mut query = vec![0.0f64; 32];
    query[5] = 0.9;
    query[6] = 0.4;
    let results = memory.recall_similar(&query, 3);
    assert_eq!(results.len(), 3);
    // Most similar episode should be the one with percept[5]=1.0
    assert!(results[0].reward > 0.0);
}

/// Test 4: Behavior tree patrol execution.
#[test]
fn test_behavior_tree_patrol() {
    let tree_node = BehaviorNode::Selector(vec![
        BehaviorNode::Sequence(vec![
            BehaviorNode::Condition("battery_ok".into()),
            BehaviorNode::Action("patrol".into()),
        ]),
        BehaviorNode::Action("return_to_base".into()),
    ]);
    let mut tree = BehaviorTree::new(tree_node);
    tree.set_condition("battery_ok", true);
    tree.set_action_result("patrol", BehaviorStatus::Running);
    tree.set_action_result("return_to_base", BehaviorStatus::Running);

    for _ in 0..5 {
        assert_eq!(tree.tick(), BehaviorStatus::Running);
    }
    tree.set_action_result("patrol", BehaviorStatus::Success);
    assert_eq!(tree.tick(), BehaviorStatus::Success);
}

/// Test 5: Swarm task assignment using actual SwarmCoordinator.
#[test]
fn test_swarm_task_assignment() {
    let mut coordinator = SwarmCoordinator::new(SwarmConfig::default());

    for i in 0..5 {
        coordinator.register_robot(RobotCapabilities {
            id: i,
            max_speed: 1.0 + i as f64 * 0.2,
            payload: 10.0,
            sensors: vec!["lidar".into()],
        });
    }

    let tasks: Vec<SwarmTask> = (0..5)
        .map(|i| SwarmTask {
            id: i,
            description: format!("task_{}", i),
            location: [i as f64 * 2.0, 0.0, 0.0],
            required_capabilities: vec!["lidar".into()],
            priority: (i % 3) as u8,
        })
        .collect();

    let assignments = coordinator.assign_tasks(&tasks);
    assert!(!assignments.is_empty(), "Should assign at least one task");
}

/// Test 6: World model update and predict using actual WorldModel.
#[test]
fn test_world_model_update_predict() {
    let mut model = WorldModel::new(50, 0.5);

    for i in 0..10 {
        model.update_object(TrackedObject {
            id: i,
            position: [i as f64 * 0.5, 0.0, 0.0],
            velocity: [0.5, 0.0, 0.0],
            last_seen: i as i64 * 100_000,
            confidence: 0.9,
            label: format!("obj_{}", i),
        });
    }

    // Predict state of object 5 forward 2 seconds
    let predicted = model.predict_state(5, 2.0).unwrap();
    let expected_x = 5.0 * 0.5 + 0.5 * 2.0;
    assert!((predicted.position[0] - expected_x).abs() < 1e-6);

    // Stale removal
    let removed = model.remove_stale_objects(1_000_000, 500_000);
    assert!(removed > 0, "Should remove early objects");
}

/// Test 7: Skill learning using actual SkillLibrary.
#[test]
fn test_skill_learning_from_demo() {
    let mut library = SkillLibrary::new();

    let demos = vec![
        Demonstration {
            trajectory: vec![[0.0, 0.0, 0.0], [0.5, 0.0, 0.2], [1.0, 0.0, 0.5], [1.0, 0.0, 0.0]],
            timestamps: vec![0, 100, 200, 300],
            metadata: "demo1".into(),
        },
        Demonstration {
            trajectory: vec![[0.0, 0.1, 0.0], [0.5, 0.1, 0.25], [1.0, 0.1, 0.55], [1.0, 0.1, 0.0]],
            timestamps: vec![0, 100, 200, 300],
            metadata: "demo2".into(),
        },
    ];

    let skill = library.learn_from_demonstration("pick_up", &demos);
    assert_eq!(skill.trajectory.len(), 4);
    assert!(skill.confidence > 0.0);

    let avg_z1 = (0.2 + 0.25) / 2.0;
    assert!((skill.trajectory[1][2] - avg_z1).abs() < 1e-6);
}

/// Test 8: Anomaly detection using actual PerceptionPipeline.
#[test]
fn test_anomaly_detection_accuracy() {
    let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
    let mut pts: Vec<[f32; 3]> = (0..20).map(|i| [i as f32 * 0.1, 0.0, 0.0]).collect();
    pts.push([100.0, 100.0, 100.0]); // outlier
    let cloud = PointCloud::new(
        pts.iter().map(|a| Point3D::new(a[0], a[1], a[2])).collect(),
        0,
    );
    let anomalies = pipe.detect_anomalies(&cloud).unwrap();
    assert!(!anomalies.is_empty());
    assert!(anomalies.iter().any(|a| a.score > 2.0));
}

/// Test 9: Decision engine selects optimal action using actual DecisionEngine.
#[test]
fn test_decision_engine_selects_best() {
    let engine = DecisionEngine::new(DecisionConfig {
        risk_aversion: 1.0,
        energy_weight: 0.0,
        curiosity_weight: 0.0,
    });
    let options = vec![
        ActionOption { name: "proceed_fast".into(), reward: 8.0, risk: 0.7, energy_cost: 0.5, novelty: 0.0 },
        ActionOption { name: "detour".into(), reward: 7.0, risk: 0.3, energy_cost: 0.3, novelty: 0.0 },
        ActionOption { name: "stop".into(), reward: 3.0, risk: 0.0, energy_cost: 0.0, novelty: 0.0 },
    ];
    let (best_idx, _) = engine.evaluate(&options).unwrap();
    // With risk_aversion=1, proceed_fast (reward=8 - risk*1=7.3) beats detour (7 - 0.3=6.7)
    assert_eq!(options[best_idx].name, "proceed_fast");

    // Conservative: higher risk_aversion
    let conservative = DecisionEngine::new(DecisionConfig {
        risk_aversion: 10.0,
        energy_weight: 0.0,
        curiosity_weight: 0.0,
    });
    let (best_idx, _) = conservative.evaluate(&options).unwrap();
    // proceed_fast: 8 - 7.0 = 1.0, detour: 7 - 3.0 = 4.0, stop: 3 - 0 = 3.0
    assert_eq!(options[best_idx].name, "detour");
}

/// Test 10: MCP tool listing -- verify all 15 tools are registered.
#[test]
fn test_mcp_tool_listing() {
    let registry = RoboticsToolRegistry::new();
    assert_eq!(registry.list_tools().len(), 15);

    let expected = [
        "detect_obstacles", "build_scene_graph", "predict_trajectory",
        "focus_attention", "detect_anomalies", "spatial_search", "insert_points",
        "store_memory", "recall_memory", "learn_skill", "execute_skill",
        "plan_behavior", "coordinate_swarm", "update_world_model", "get_world_state",
    ];
    for name in &expected {
        assert!(registry.get_tool(name).is_some(), "Tool '{}' should be registered", name);
    }
}

/// Test 11: MCP tool execution via ToolExecutor.
#[test]
fn test_mcp_tool_execution() {
    let mut executor = ToolExecutor::new();

    // Predict trajectory
    let req = ToolRequest {
        tool_name: "predict_trajectory".into(),
        arguments: [
            ("position".into(), serde_json::json!([0.0, 0.0, 0.0])),
            ("velocity".into(), serde_json::json!([1.0, 0.0, 0.0])),
            ("steps".into(), serde_json::json!(5)),
            ("dt".into(), serde_json::json!(0.5)),
        ].into(),
    };
    let resp = executor.execute(&req);
    assert!(resp.success, "predict_trajectory should succeed: {:?}", resp.error);

    // Unknown tool
    let req = ToolRequest { tool_name: "nonexistent".into(), arguments: Default::default() };
    let resp = executor.execute(&req);
    assert!(!resp.success, "nonexistent tool should fail");
}

/// Test 12: Gaussian splatting from point cloud.
#[test]
fn test_gaussian_splatting() {
    let mut points = generate_point_cloud_around(Point3D::new(2.0, 0.0, 0.0), 30, 0.5);
    points.extend(generate_point_cloud_around(Point3D::new(8.0, 0.0, 0.0), 30, 0.5));
    let cloud = PointCloud::new(points, 1000);

    let gaussians = gaussians_from_cloud(&cloud, &GaussianConfig::default());
    assert!(gaussians.len() >= 2, "Should produce at least 2 Gaussians, got {}", gaussians.len());

    for g in &gaussians.gaussians {
        assert!(g.point_count >= 2);
        assert!(g.opacity > 0.0);
        assert!(g.scale[0] > 0.0);
    }

    // Verify JSON export
    let json = ruvector_robotics::bridge::gaussian::to_viewer_json(&gaussians);
    assert_eq!(json["count"].as_u64().unwrap(), gaussians.len() as u64);
}

/// Test 13: A* pathfinding.
#[test]
fn test_astar_planning() {
    let mut grid = ruvector_robotics::bridge::OccupancyGrid::new(20, 20, 0.5);

    // Add a wall
    for y in 0..15 {
        grid.set(10, y, 1.0);
    }

    let path = planning::astar(&grid, (5, 5), (15, 5)).unwrap();
    assert_eq!(*path.cells.first().unwrap(), (5, 5));
    assert_eq!(*path.cells.last().unwrap(), (15, 5));
    assert!(path.cost > 10.0, "Path around wall should be longer than straight line");

    // Verify no cell in path is occupied
    for &(x, y) in &path.cells {
        assert!(grid.get(x, y).unwrap() < 0.5, "Path cell ({},{}) is occupied", x, y);
    }
}

/// Test 14: Potential field planner.
#[test]
fn test_potential_field_planning() {
    let cmd = planning::potential_field(
        &[0.0, 0.0, 0.0],
        &[10.0, 0.0, 0.0],
        &[[3.0, 0.0, 0.0]],
        &planning::PotentialFieldConfig::default(),
    );
    // Should still move forward but with some deflection from obstacle
    assert!(cmd.vx > 0.0, "Should move toward goal");

    // No obstacles: straight toward goal
    let cmd_free = planning::potential_field(
        &[0.0, 0.0, 0.0],
        &[10.0, 0.0, 0.0],
        &[],
        &planning::PotentialFieldConfig::default(),
    );
    assert!(cmd_free.vy.abs() < 1e-9);
}

/// Test 15: Sensor fusion.
#[test]
fn test_sensor_fusion() {
    let c1 = PointCloud::new(
        vec![Point3D::new(1.0, 0.0, 0.0), Point3D::new(2.0, 0.0, 0.0)],
        1000,
    );
    let c2 = PointCloud::new(
        vec![Point3D::new(3.0, 0.0, 0.0)],
        1010,
    );
    let c3_stale = PointCloud::new(
        vec![Point3D::new(99.0, 0.0, 0.0)],
        200_000, // 199ms later — too stale
    );

    let config = FusionConfig { max_time_delta_us: 50_000, ..Default::default() };
    let fused = fuse_clouds(&[c1, c2, c3_stale], &config);
    assert_eq!(fused.len(), 3, "Should include c1+c2 but skip c3");
}

/// Test 16: Full pipeline stress test — 100 frames using real APIs.
#[test]
fn test_full_pipeline_100_frames() {
    let start = Instant::now();
    let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
    let mut total_obstacles = 0usize;

    for frame in 0..100 {
        let center = Point3D::new(
            3.0 + frame as f32 * 0.1,
            pseudo_random_f32(frame as u64, 0) * 4.0 - 2.0,
            0.0,
        );
        let points = generate_point_cloud_around(center, 100, 1.0);
        let cloud = PointCloud::new(points, frame as i64 * 33_333);

        let obstacles = pipe
            .detect_obstacles(&cloud, [frame as f64 * 0.1, 0.0, 0.0], 10.0)
            .unwrap();
        total_obstacles += obstacles.len();
    }

    let elapsed = start.elapsed();
    assert!(total_obstacles > 0, "Should detect obstacles across 100 frames");
    assert!(elapsed.as_secs() < 5, "100 frames should complete in < 5s, took {:?}", elapsed);
}

/// Test 17: Concurrent spatial search from multiple threads.
#[test]
fn test_concurrent_spatial_search() {
    let points: Vec<Point3D> = (0..5000)
        .map(|i| {
            Point3D::new(
                pseudo_random_f32(42, i * 3) * 10.0,
                pseudo_random_f32(42, i * 3 + 1) * 10.0,
                pseudo_random_f32(42, i * 3 + 2) * 10.0,
            )
        })
        .collect();

    let cloud = PointCloud::new(points, 0);
    let mut index = SpatialIndex::new(3);
    index.insert_point_cloud(&cloud);
    let shared_index = Arc::new(index);
    let results = Arc::new(Mutex::new(Vec::new()));

    let mut handles = Vec::new();
    for thread_id in 0..4 {
        let idx = Arc::clone(&shared_index);
        let res = Arc::clone(&results);
        let handle = std::thread::spawn(move || {
            let query = [(thread_id as f32) * 2.5, (thread_id as f32) * 2.5, 5.0_f32];
            let neighbors = idx.search_nearest(&query, 5).unwrap();
            res.lock().unwrap().push((thread_id, neighbors));
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread should not panic");
    }

    let final_results = results.lock().unwrap();
    assert_eq!(final_results.len(), 4, "All 4 threads should complete");
    for (tid, neighbors) in final_results.iter() {
        assert_eq!(neighbors.len(), 5, "Thread {} should return 5 neighbors", tid);
        for window in neighbors.windows(2) {
            assert!(window[0].1 <= window[1].1, "Thread {} results should be distance-sorted", tid);
        }
    }
}

/// Test 18: Edge cases — empty inputs and boundary conditions.
#[test]
fn test_edge_cases() {
    let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);

    // Empty point cloud
    let empty = PointCloud::default();
    let obs = pipe.detect_obstacles(&empty, [0.0; 3], 10.0).unwrap();
    assert!(obs.is_empty());

    // Scene graph with invalid distance
    assert!(pipe.build_scene_graph(&[], -1.0).is_err());

    // Trajectory with zero steps
    assert!(pipe.predict_trajectory([0.0; 3], [1.0, 0.0, 0.0], 0, 1.0).is_err());

    // Attention with negative radius
    assert!(pipe.focus_attention(&empty, [0.0; 3], -1.0).is_err());

    // Anomaly with < 2 points
    let small = PointCloud::new(vec![Point3D::new(1.0, 0.0, 0.0)], 0);
    assert!(pipe.detect_anomalies(&small).unwrap().is_empty());

    // Empty spatial index
    let index = SpatialIndex::new(3);
    assert!(index.search_nearest(&[0.0_f32, 0.0, 0.0], 5).is_err());
    // Radius search on empty index returns Ok(empty)
    assert!(index.search_radius(&[0.0_f32, 0.0, 0.0], 1.0).unwrap().is_empty());

    // Behavior tree decorator
    let node = BehaviorNode::Decorator(
        DecoratorType::Inverter,
        Box::new(BehaviorNode::Action("a".into())),
    );
    let mut tree = BehaviorTree::new(node);
    tree.set_action_result("a", BehaviorStatus::Success);
    assert_eq!(tree.tick(), BehaviorStatus::Failure);

    // Empty Gaussian conversion
    let gs = gaussians_from_cloud(&PointCloud::default(), &GaussianConfig::default());
    assert!(gs.is_empty());

    // A* on same start/goal
    let grid = ruvector_robotics::bridge::OccupancyGrid::new(5, 5, 1.0);
    let path = planning::astar(&grid, (2, 2), (2, 2)).unwrap();
    assert_eq!(path.cells.len(), 1);
}
