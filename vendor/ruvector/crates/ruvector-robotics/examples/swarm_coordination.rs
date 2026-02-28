//! Example: multi-robot swarm coordination.
//!
//! Demonstrates:
//! - Registering robots with capabilities
//! - Task assignment based on capability matching
//! - Formation computation (line, circle, grid)
//! - Consensus voting

use ruvector_robotics::cognitive::{
    Formation, FormationType, RobotCapabilities, SwarmConfig, SwarmCoordinator, SwarmTask,
};

fn main() {
    println!("=== Swarm Coordination Demo ===\n");

    let config = SwarmConfig {
        max_robots: 10,
        communication_range: 100.0,
        consensus_threshold: 0.5,
    };
    let mut coordinator = SwarmCoordinator::new(config);

    // Register robots
    let robots = vec![
        RobotCapabilities {
            id: 1,
            max_speed: 2.0,
            payload: 10.0,
            sensors: vec!["lidar".into(), "camera".into()],
        },
        RobotCapabilities {
            id: 2,
            max_speed: 1.5,
            payload: 50.0,
            sensors: vec!["lidar".into(), "gripper".into()],
        },
        RobotCapabilities {
            id: 3,
            max_speed: 3.0,
            payload: 5.0,
            sensors: vec!["camera".into(), "thermal".into()],
        },
        RobotCapabilities {
            id: 4,
            max_speed: 1.0,
            payload: 100.0,
            sensors: vec!["lidar".into(), "gripper".into(), "camera".into()],
        },
    ];

    for robot in &robots {
        let registered = coordinator.register_robot(robot.clone());
        println!("Registered robot {} (speed={}, sensors={:?}): {}",
            robot.id, robot.max_speed, robot.sensors, registered);
    }
    println!("\nActive robots: {}\n", coordinator.robot_count());

    // Task assignment
    println!("--- Task Assignment ---");
    let tasks = vec![
        SwarmTask {
            id: 100,
            description: "Inspect corridor A".into(),
            location: [10.0, 0.0, 0.0],
            required_capabilities: vec!["camera".into()],
            priority: 5,
        },
        SwarmTask {
            id: 101,
            description: "Move pallet B".into(),
            location: [5.0, 5.0, 0.0],
            required_capabilities: vec!["gripper".into()],
            priority: 8,
        },
        SwarmTask {
            id: 102,
            description: "Map area C".into(),
            location: [0.0, 10.0, 0.0],
            required_capabilities: vec!["lidar".into()],
            priority: 3,
        },
    ];

    let assignments = coordinator.assign_tasks(&tasks);
    for a in &assignments {
        let task = tasks.iter().find(|t| t.id == a.task_id).unwrap();
        println!(
            "  Robot {} -> Task {} ({}) [ETA: {:.1}s]",
            a.robot_id, a.task_id, task.description, a.estimated_completion
        );
    }

    // Formation computation
    println!("\n--- Formations ---");
    for (name, ftype) in [
        ("Line", FormationType::Line),
        ("Circle", FormationType::Circle),
        ("Grid", FormationType::Grid),
    ] {
        let formation = Formation {
            formation_type: ftype,
            spacing: 3.0,
            center: [0.0, 0.0, 0.0],
        };
        let positions = coordinator.compute_formation(&formation);
        println!("  {} formation:", name);
        for (i, pos) in positions.iter().enumerate() {
            println!("    Robot {}: [{:.2}, {:.2}, {:.2}]", i, pos[0], pos[1], pos[2]);
        }
    }

    // Consensus
    println!("\n--- Consensus Voting ---");
    let result = coordinator.propose_consensus("Explore sector 7");
    println!(
        "  Proposal: '{}' -> {} (for={}, against={})",
        result.proposal,
        if result.accepted { "ACCEPTED" } else { "REJECTED" },
        result.votes_for,
        result.votes_against,
    );
}
