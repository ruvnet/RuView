/// Example 04: Behavior Tree - Declarative robot control with status transitions
///
/// Demonstrates:
/// - Building a patrol behavior tree with Selector and Sequence nodes
/// - Setting conditions and action results dynamically
/// - Observing tick-by-tick status transitions
/// - Using decorators (Inverter, Repeat, Timeout)
///
/// Tree structure:
///   Root (Selector)
///     |-- Avoid (Sequence): [Condition("obstacle_near")] -> [Action("evade")]
///     |-- Patrol (Sequence): [Action("select_wp")] -> [Action("move")] -> [Action("wait")]

use ruvector_robotics::cognitive::{BehaviorNode, BehaviorStatus, BehaviorTree, DecoratorType};

fn main() {
    println!("=== Example 04: Behavior Tree ===");
    println!();

    // Step 1: Build the behavior tree
    let avoid_subtree = BehaviorNode::Sequence(vec![
        BehaviorNode::Condition("obstacle_near".into()),
        BehaviorNode::Action("evade".into()),
    ]);

    let patrol_subtree = BehaviorNode::Sequence(vec![
        BehaviorNode::Action("select_waypoint".into()),
        BehaviorNode::Action("move_to_waypoint".into()),
        BehaviorNode::Action("wait_at_waypoint".into()),
    ]);

    let root = BehaviorNode::Selector(vec![avoid_subtree, patrol_subtree]);
    let mut tree = BehaviorTree::new(root);

    println!("[1] Behavior tree built:");
    println!("    Root (Selector)");
    println!("      Avoid (Sequence): [obstacle_near?] -> [evade]");
    println!("      Patrol (Sequence): [select_wp] -> [move] -> [wait]");
    println!();

    // Step 2: Simulate normal patrol (no obstacle)
    println!("[2] Normal patrol (no obstacle):");
    tree.set_condition("obstacle_near", false);
    tree.set_action_result("select_waypoint", BehaviorStatus::Success);
    tree.set_action_result("move_to_waypoint", BehaviorStatus::Success);
    tree.set_action_result("wait_at_waypoint", BehaviorStatus::Success);

    for tick in 0..3 {
        let status = tree.tick();
        println!(
            "    Tick {}: {:?} (tick_count={})",
            tick,
            status,
            tree.context().tick_count
        );
    }
    println!();

    // Step 3: Simulate obstacle detection
    println!("[3] Obstacle detected:");
    tree.set_condition("obstacle_near", true);
    tree.set_action_result("evade", BehaviorStatus::Running);

    let status = tree.tick();
    println!("    Tick: {:?} (evade is Running)", status);

    tree.set_action_result("evade", BehaviorStatus::Success);
    let status = tree.tick();
    println!("    Tick: {:?} (evade succeeded, obstacle handled)", status);

    // Clear obstacle
    tree.set_condition("obstacle_near", false);
    let status = tree.tick();
    println!("    Tick: {:?} (back to patrol)", status);
    println!();

    // Step 4: Movement failure scenario
    println!("[4] Movement failure scenario:");
    tree.set_action_result("move_to_waypoint", BehaviorStatus::Failure);
    let status = tree.tick();
    println!(
        "    Tick: {:?} (movement failed, Sequence returns Failure)",
        status
    );

    tree.set_action_result("move_to_waypoint", BehaviorStatus::Success);
    let status = tree.tick();
    println!("    Tick: {:?} (movement recovered)", status);
    println!();

    // Step 5: Decorator examples
    println!("[5] Decorator examples:");

    // Inverter: turn success into failure
    let inverted = BehaviorNode::Decorator(
        DecoratorType::Inverter,
        Box::new(BehaviorNode::Action("check".into())),
    );
    let mut inv_tree = BehaviorTree::new(inverted);
    inv_tree.set_action_result("check", BehaviorStatus::Success);
    let status = inv_tree.tick();
    println!("    Inverter(Success) = {:?}", status);

    // Repeat: run an action 3 times
    let repeated = BehaviorNode::Decorator(
        DecoratorType::Repeat(3),
        Box::new(BehaviorNode::Action("step".into())),
    );
    let mut rep_tree = BehaviorTree::new(repeated);
    rep_tree.set_action_result("step", BehaviorStatus::Success);
    let status = rep_tree.tick();
    println!("    Repeat(3, Success) = {:?}", status);

    // Timeout: fail after N ticks
    let timed = BehaviorNode::Decorator(
        DecoratorType::Timeout(2),
        Box::new(BehaviorNode::Action("slow_task".into())),
    );
    let mut time_tree = BehaviorTree::new(timed);
    time_tree.set_action_result("slow_task", BehaviorStatus::Running);
    println!("    Timeout(2):");
    for i in 0..4 {
        let status = time_tree.tick();
        println!("      Tick {}: {:?}", i, status);
    }

    // Step 6: Parallel node
    println!();
    println!("[6] Parallel node (threshold=2 of 3):");
    let parallel = BehaviorNode::Parallel(
        2,
        vec![
            BehaviorNode::Action("sensor_a".into()),
            BehaviorNode::Action("sensor_b".into()),
            BehaviorNode::Action("sensor_c".into()),
        ],
    );
    let mut par_tree = BehaviorTree::new(parallel);
    par_tree.set_action_result("sensor_a", BehaviorStatus::Success);
    par_tree.set_action_result("sensor_b", BehaviorStatus::Success);
    par_tree.set_action_result("sensor_c", BehaviorStatus::Failure);
    let status = par_tree.tick();
    println!("    [S, S, F] = {:?} (2 >= threshold 2)", status);

    par_tree.set_action_result("sensor_b", BehaviorStatus::Failure);
    let status = par_tree.tick();
    println!("    [S, F, F] = {:?} (1 < threshold 2)", status);

    println!();
    println!("[done] Behavior tree example complete.");
}
