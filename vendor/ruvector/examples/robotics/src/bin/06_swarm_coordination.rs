/// Example 06: Swarm Coordination - Multi-robot task assignment and formations
///
/// Demonstrates:
/// - Registering robots with capabilities in a SwarmCoordinator
/// - Assigning tasks based on capability matching
/// - Computing Line, Circle, and Grid formations
/// - Running consensus votes among swarm members

use ruvector_robotics::cognitive::{
    Formation, FormationType, RobotCapabilities, SwarmConfig, SwarmCoordinator, SwarmTask,
};

fn main() {
    println!("=== Example 06: Swarm Coordination ===");
    println!();

    // Step 1: Create and register robots
    let config = SwarmConfig {
        max_robots: 10,
        communication_range: 50.0,
        consensus_threshold: 0.5,
    };
    let mut coordinator = SwarmCoordinator::new(config);

    let robots = vec![
        RobotCapabilities {
            id: 0,
            max_speed: 1.5,
            payload: 10.0,
            sensors: vec!["lidar".into(), "camera".into()],
        },
        RobotCapabilities {
            id: 1,
            max_speed: 2.0,
            payload: 5.0,
            sensors: vec!["camera".into(), "imu".into()],
        },
        RobotCapabilities {
            id: 2,
            max_speed: 1.0,
            payload: 20.0,
            sensors: vec!["lidar".into(), "sonar".into()],
        },
        RobotCapabilities {
            id: 3,
            max_speed: 1.8,
            payload: 8.0,
            sensors: vec!["camera".into(), "depth".into(), "imu".into()],
        },
        RobotCapabilities {
            id: 4,
            max_speed: 1.2,
            payload: 15.0,
            sensors: vec!["lidar".into(), "camera".into(), "sonar".into()],
        },
    ];

    println!("[1] Registering {} robots:", robots.len());
    for robot in robots {
        let registered = coordinator.register_robot(robot.clone());
        println!(
            "    Robot {}: speed={:.1}, payload={:.0}kg, sensors=[{}] -> {}",
            robot.id,
            robot.max_speed,
            robot.payload,
            robot.sensors.join(", "),
            if registered { "OK" } else { "REJECTED" }
        );
    }
    println!("    Total registered: {}", coordinator.robot_count());
    println!();

    // Step 2: Create and assign tasks
    let tasks = vec![
        SwarmTask {
            id: 100,
            description: "Scan area A with lidar".into(),
            location: [3.0, 4.0, 0.0],
            required_capabilities: vec!["lidar".into()],
            priority: 8,
        },
        SwarmTask {
            id: 101,
            description: "Visual inspection of zone B".into(),
            location: [5.0, 0.0, 0.0],
            required_capabilities: vec!["camera".into()],
            priority: 5,
        },
        SwarmTask {
            id: 102,
            description: "Underwater sonar survey".into(),
            location: [0.0, 7.0, -2.0],
            required_capabilities: vec!["sonar".into()],
            priority: 10,
        },
        SwarmTask {
            id: 103,
            description: "Depth mapping of area C".into(),
            location: [8.0, 8.0, 0.0],
            required_capabilities: vec!["depth".into(), "camera".into()],
            priority: 6,
        },
    ];

    println!("[2] Assigning {} tasks:", tasks.len());
    let assignments = coordinator.assign_tasks(&tasks);
    for assignment in &assignments {
        let task = tasks.iter().find(|t| t.id == assignment.task_id).unwrap();
        println!(
            "    Task {} ('{}') -> Robot {} (est. {:.1}s)",
            assignment.task_id, task.description, assignment.robot_id,
            assignment.estimated_completion
        );
    }
    let unassigned = tasks.len() - assignments.len();
    if unassigned > 0 {
        println!("    {} tasks could not be assigned", unassigned);
    }
    println!();

    // Step 3: Formation control
    println!("[3] Formation control:");

    let formations = vec![
        ("Line", FormationType::Line),
        ("Circle", FormationType::Circle),
        ("Grid", FormationType::Grid),
    ];

    for (name, ftype) in &formations {
        let formation = Formation {
            formation_type: ftype.clone(),
            spacing: 2.0,
            center: [5.0, 5.0, 0.0],
        };
        let positions = coordinator.compute_formation(&formation);
        println!("    {} formation (spacing=2.0m, center=(5,5)):", name);
        for (i, pos) in positions.iter().enumerate() {
            println!(
                "      Robot {} -> ({:6.2}, {:6.2}, {:6.2})",
                i, pos[0], pos[1], pos[2]
            );
        }
    }
    println!();

    // Step 4: Consensus voting
    println!("[4] Consensus voting:");
    let proposals = vec![
        "explore-sector-alpha",
        "return-to-base",
        "form-defensive-perimeter",
    ];
    for proposal in &proposals {
        let result = coordinator.propose_consensus(proposal);
        println!(
            "    '{}': for={}, against={}, {}",
            result.proposal,
            result.votes_for,
            result.votes_against,
            if result.accepted { "ACCEPTED" } else { "REJECTED" }
        );
    }

    println!();
    println!("[done] Swarm coordination example complete.");
}
