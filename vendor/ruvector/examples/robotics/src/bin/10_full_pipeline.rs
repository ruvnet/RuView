/// Example 10: Full Pipeline - Sensor -> Perception -> Cognition -> Action
///
/// Combines all modules into an integrated simulation:
///   1. Generate sensor point cloud
///   2. Detect and classify obstacles via PerceptionPipeline
///   3. Feed percepts into CognitiveCore (perceive-think-act-learn)
///   4. Track objects in WorldModel
///   5. Report comprehensive statistics

use rand::Rng;
use ruvector_robotics::bridge::{Point3D, PointCloud, SpatialIndex};
use ruvector_robotics::cognitive::{
    CognitiveConfig, CognitiveCore, CognitiveMode, Outcome, Percept, TrackedObject, WorldModel,
};
use ruvector_robotics::mcp::RoboticsToolRegistry;
use ruvector_robotics::perception::{PerceptionConfig, PerceptionPipeline};

fn main() {
    println!("============================================================");
    println!("  Example 10: Full Cognitive Robotics Pipeline");
    println!("============================================================");
    println!();

    let mut rng = rand::thread_rng();

    // -- Initialize modules --
    let mut pipeline = PerceptionPipeline::new(PerceptionConfig::default());
    let mut core = CognitiveCore::new(CognitiveConfig {
        mode: CognitiveMode::Reactive,
        attention_threshold: 0.4,
        learning_rate: 0.02,
        max_percepts: 50,
    });
    let mut world = WorldModel::new(20, 0.5);
    let mut index = SpatialIndex::new(3);
    let registry = RoboticsToolRegistry::new();

    let mut robot_pos = [1.0_f64, 1.0, 0.0];
    let mut total_distance = 0.0_f64;
    let mut decisions_made = 0usize;

    println!("[1] Modules initialized:");
    println!("    PerceptionPipeline: default config");
    println!("    CognitiveCore:     mode={:?}, state={:?}", core.mode(), core.state());
    println!("    WorldModel:        {}x{} grid", world.grid_size(), world.grid_size());
    println!("    MCP registry:      {} tools", registry.list_tools().len());
    println!();

    // -- Static obstacle points --
    let mut obstacle_pts = Vec::new();
    // Wall at y=4
    for i in 0..40 {
        obstacle_pts.push(Point3D::new(2.0 + i as f32 * 0.1, 4.0, 0.0));
    }
    // Box near (7, 2)
    for dx in 0..5 {
        for dy in 0..5 {
            obstacle_pts.push(Point3D::new(7.0 + dx as f32 * 0.1, 2.0 + dy as f32 * 0.1, 0.0));
        }
    }

    let total_steps = 30;
    println!("[2] Running {} simulation steps...", total_steps);
    println!();

    for step in 0..total_steps {
        // SENSOR: Build point cloud
        let mut pts = obstacle_pts.clone();
        for _ in 0..5 {
            pts.push(Point3D::new(
                rng.gen_range(0.0..10.0),
                rng.gen_range(0.0..10.0),
                0.0,
            ));
        }
        let cloud = PointCloud::new(pts, step as i64 * 100);

        // PERCEIVE: Run perception pipeline
        let (obstacles, _scene_graph) = pipeline.process(&cloud, &robot_pos);
        index.insert_point_cloud(&cloud);

        // Track obstacles in world model
        for (i, obs) in obstacles.iter().enumerate() {
            world.update_object(TrackedObject {
                id: i as u64,
                position: obs.center,
                velocity: [0.0, 0.0, 0.0],
                last_seen: step as i64 * 100,
                confidence: 0.9,
                label: format!("obs_{}", i),
            });
        }

        // THINK: Feed percept to cognitive core
        let nearest_dist = obstacles.first().map(|o| o.min_distance).unwrap_or(f64::MAX);
        core.perceive(Percept {
            source: "perception_pipeline".into(),
            data: vec![robot_pos[0], robot_pos[1], nearest_dist],
            confidence: 0.85,
            timestamp: step as i64 * 100,
        });

        let action_label;
        if let Some(decision) = core.think() {
            decisions_made += 1;
            let _cmd = core.act(decision);
            action_label = if nearest_dist < 2.0 { "avoid" } else { "patrol" };
        } else {
            action_label = "idle";
        }

        // ACT: Move robot
        let old_pos = robot_pos;
        match action_label {
            "avoid" => {
                if let Some(obs) = obstacles.first() {
                    let dx = robot_pos[0] - obs.center[0];
                    let dy = robot_pos[1] - obs.center[1];
                    let d = (dx * dx + dy * dy).sqrt().max(0.01);
                    robot_pos[0] += 0.3 * dx / d;
                    robot_pos[1] += 0.3 * dy / d;
                }
            }
            "patrol" => {
                let angle: f64 = rng.gen_range(0.0..std::f64::consts::TAU);
                robot_pos[0] += 0.25 * angle.cos();
                robot_pos[1] += 0.25 * angle.sin();
            }
            _ => {}
        }
        robot_pos[0] = robot_pos[0].clamp(0.0, 10.0);
        robot_pos[1] = robot_pos[1].clamp(0.0, 10.0);

        let step_dist =
            ((robot_pos[0] - old_pos[0]).powi(2) + (robot_pos[1] - old_pos[1]).powi(2)).sqrt();
        total_distance += step_dist;

        // LEARN: Feedback
        let success = nearest_dist > 1.0;
        core.learn(Outcome {
            success,
            reward: if success { 1.0 } else { -0.5 },
            description: format!("step_{}", step),
        });

        if step % 5 == 0 {
            println!(
                "    Step {:>3}: pos=({:5.2},{:5.2}) action={:<8} obstacles={} state={:?}",
                step,
                robot_pos[0],
                robot_pos[1],
                action_label,
                obstacles.len(),
                core.state()
            );
        }
    }

    // -- Statistics --
    println!();
    println!("============================================================");
    println!("  Results ({} steps)", total_steps);
    println!("============================================================");
    println!();

    println!("[A] Movement:");
    println!("    Final position: ({:.2}, {:.2})", robot_pos[0], robot_pos[1]);
    println!("    Total distance: {:.2}m", total_distance);
    println!(
        "    Avg speed:      {:.3}m/step",
        total_distance / total_steps as f64
    );
    println!();

    println!("[B] Perception:");
    println!("    Frames processed: {}", pipeline.frames_processed());
    println!("    Spatial index:    {} points", index.len());
    println!();

    println!("[C] Cognition:");
    println!("    State:            {:?}", core.state());
    println!("    Decisions made:   {}", decisions_made);
    println!("    Cumulative reward: {:.4}", core.cumulative_reward());
    println!();

    println!("[D] World Model:");
    println!("    Tracked objects: {}", world.object_count());
    println!("    Grid size:       {}x{}", world.grid_size(), world.grid_size());
    println!();

    println!("[E] MCP tools available: {}", registry.list_tools().len());
    println!();

    println!("[done] Full pipeline example complete.");
}
