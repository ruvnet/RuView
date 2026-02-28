/// RuVector Cognitive Robotics Examples
///
/// Each example demonstrates a distinct robotics capability built on top of
/// the unified ruvector-robotics crate.
///
/// Run any example with:
///
///     cargo run --bin <example_name>
fn main() {
    println!("==========================================================");
    println!("  RuVector Cognitive Robotics Examples");
    println!("==========================================================");
    println!();
    println!("Available examples:");
    println!();
    println!("  PRACTICAL");
    println!("  ---------");
    println!("  01_basic_perception      Point cloud creation, kNN and radius search");
    println!("  02_obstacle_avoidance    Detect obstacles, classify, compute distances");
    println!("  03_scene_graph           Build scene graphs, compute edges, merge scenes");
    println!();
    println!("  INTERMEDIATE");
    println!("  ------------");
    println!("  04_behavior_tree         Patrol behavior tree with status transitions");
    println!("  05_cognitive_robot       Perceive-think-act-learn cognitive loop");
    println!("  06_swarm_coordination    Multi-robot task assignment and formations");
    println!();
    println!("  ADVANCED");
    println!("  --------");
    println!("  07_skill_learning        Learn skills from demos, execute, improve");
    println!("  08_world_model           Occupancy grid, object tracking, path clearance");
    println!("  09_mcp_tools             MCP tool registry and JSON schema generation");
    println!();
    println!("  FULL SYSTEM");
    println!("  -----------");
    println!("  10_full_pipeline         Sensor -> Perception -> Cognition -> Action");
    println!();
    println!("Run an example:");
    println!("  cargo run --bin 01_basic_perception");
    println!("  cargo run --bin 10_full_pipeline");
    println!();
}
