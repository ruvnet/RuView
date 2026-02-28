//! Example: composable behavior trees for robot task execution.
//!
//! Demonstrates:
//! - Building a patrol behavior with sequence/selector/decorator nodes
//! - Ticking the tree and observing status changes
//! - Using the blackboard for inter-node communication

use ruvector_robotics::cognitive::{
    BehaviorNode, BehaviorStatus, BehaviorTree, DecoratorType,
};

fn main() {
    println!("=== Behavior Tree Demo ===\n");

    // Build a patrol behavior:
    //   Selector [
    //     Sequence [has_obstacle → avoid_obstacle]
    //     Sequence [has_target → move_to_target → interact]
    //     Repeat(3) [ patrol_waypoint ]
    //   ]
    let tree_root = BehaviorNode::Selector(vec![
        // Priority 1: avoid obstacles
        BehaviorNode::Sequence(vec![
            BehaviorNode::Condition("has_obstacle".into()),
            BehaviorNode::Action("avoid_obstacle".into()),
        ]),
        // Priority 2: pursue target
        BehaviorNode::Sequence(vec![
            BehaviorNode::Condition("has_target".into()),
            BehaviorNode::Action("move_to_target".into()),
            BehaviorNode::Action("interact".into()),
        ]),
        // Priority 3: patrol
        BehaviorNode::Decorator(
            DecoratorType::Repeat(3),
            Box::new(BehaviorNode::Action("patrol_waypoint".into())),
        ),
    ]);

    let mut tree = BehaviorTree::new(tree_root);

    // Scenario 1: No obstacle, no target → patrol
    println!("--- Scenario 1: Patrolling ---");
    tree.set_action_result("patrol_waypoint", BehaviorStatus::Success);
    tree.set_action_result("avoid_obstacle", BehaviorStatus::Success);
    tree.set_action_result("move_to_target", BehaviorStatus::Success);
    tree.set_action_result("interact", BehaviorStatus::Success);

    let status = tree.tick();
    println!("  Tick 1 result: {:?}", status);
    println!("  (Should patrol since no conditions are true)\n");

    // Scenario 2: Obstacle detected
    println!("--- Scenario 2: Obstacle detected ---");
    tree.set_condition("has_obstacle", true);
    let status = tree.tick();
    println!("  Tick 2 result: {:?}", status);
    println!("  (Should avoid obstacle via selector priority)\n");

    // Scenario 3: Obstacle cleared, target found
    println!("--- Scenario 3: Target acquired ---");
    tree.set_condition("has_obstacle", false);
    tree.set_condition("has_target", true);
    let status = tree.tick();
    println!("  Tick 3 result: {:?}", status);
    println!("  (Should move to target and interact)\n");

    // Scenario 4: Timeout decorator
    println!("--- Scenario 4: Timeout behavior ---");
    let timeout_tree = BehaviorNode::Decorator(
        DecoratorType::Timeout(2),
        Box::new(BehaviorNode::Action("long_task".into())),
    );
    let mut t2 = BehaviorTree::new(timeout_tree);
    t2.set_action_result("long_task", BehaviorStatus::Running);

    for i in 1..=4 {
        let s = t2.tick();
        println!("  Tick {}: {:?}{}", i, s,
            if s == BehaviorStatus::Failure { " (TIMED OUT)" } else { "" }
        );
    }

    println!("\nFinal tick count: {}", tree.context().tick_count);
}
