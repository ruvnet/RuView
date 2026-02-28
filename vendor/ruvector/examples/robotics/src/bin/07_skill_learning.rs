/// Example 07: Skill Learning - Learn from demos, execute, improve
///
/// Demonstrates:
/// - Recording expert demonstrations as 3D trajectories
/// - Learning a skill from multiple demonstrations (averaging)
/// - Executing the learned skill and tracking its trajectory
/// - Improving confidence through positive/negative feedback
/// - Using the SkillLibrary from ruvector_robotics::cognitive

use ruvector_robotics::cognitive::{Demonstration, SkillLibrary};

fn main() {
    println!("=== Example 07: Skill Learning ===");
    println!();

    let mut library = SkillLibrary::new();

    // -- Step 1: Record demonstrations for "reach" --
    let demos = vec![
        Demonstration {
            trajectory: vec![[0.0, 0.0, 0.0], [1.0, 0.5, 0.0], [2.0, 1.0, 0.0], [3.0, 1.5, 0.0]],
            timestamps: vec![0, 100, 200, 300],
            metadata: "expert_1".into(),
        },
        Demonstration {
            trajectory: vec![[0.0, 0.0, 0.0], [1.2, 0.4, 0.0], [2.1, 0.9, 0.0], [3.1, 1.6, 0.0]],
            timestamps: vec![0, 110, 210, 310],
            metadata: "expert_2".into(),
        },
        Demonstration {
            trajectory: vec![[0.0, 0.0, 0.0], [0.8, 0.6, 0.0], [1.9, 1.1, 0.0], [2.9, 1.4, 0.0]],
            timestamps: vec![0, 90, 190, 290],
            metadata: "expert_3".into(),
        },
    ];

    println!("[1] Recorded {} demonstrations for 'reach':", demos.len());
    for (i, demo) in demos.iter().enumerate() {
        println!(
            "    Demo {} ({}): {} waypoints, duration={}us",
            i,
            demo.metadata,
            demo.trajectory.len(),
            demo.timestamps.last().unwrap_or(&0)
        );
    }

    // -- Step 2: Learn the skill --
    println!();
    let skill = library.learn_from_demonstration("reach", &demos);
    println!("[2] Learned skill 'reach':");
    println!("    Trajectory length: {} waypoints", skill.trajectory.len());
    println!("    Initial confidence: {:.3}", skill.confidence);
    println!("    Averaged trajectory:");
    for (i, pt) in skill.trajectory.iter().enumerate() {
        println!("      wp {}: ({:.2}, {:.2}, {:.2})", i, pt[0], pt[1], pt[2]);
    }

    // -- Step 3: Learn another skill with a single demo --
    println!();
    let wave_demo = Demonstration {
        trajectory: vec![[0.0, 0.0, 1.0], [0.5, 0.0, 1.5], [0.0, 0.0, 1.0], [-0.5, 0.0, 1.5]],
        timestamps: vec![0, 200, 400, 600],
        metadata: "single_demo".into(),
    };
    let wave_skill = library.learn_from_demonstration("wave", &[wave_demo]);
    println!("[3] Learned skill 'wave' from 1 demo:");
    println!("    Confidence: {:.3} (lower with fewer demos)", wave_skill.confidence);
    println!("    Library now has {} skills", library.len());

    // -- Step 4: Execute skills --
    println!();
    println!("[4] Executing skills:");
    for _ in 0..3 {
        if let Some(traj) = library.execute_skill("reach") {
            println!("    'reach' executed: {} waypoints", traj.len());
        }
    }
    let reach = library.get("reach").unwrap();
    println!("    'reach' execution count: {}", reach.execution_count);

    if let Some(traj) = library.execute_skill("wave") {
        println!("    'wave' executed: {} waypoints", traj.len());
    }
    let wave = library.get("wave").unwrap();
    println!("    'wave' execution count: {}", wave.execution_count);

    // Try non-existent skill
    let missing = library.execute_skill("backflip");
    println!("    'backflip' exists: {}", missing.is_some());

    // -- Step 5: Improve through feedback --
    println!();
    println!("[5] Improving 'reach' through feedback:");
    let before = library.get("reach").unwrap().confidence;
    println!("    Before: confidence={:.4}", before);

    // Positive feedback (5 successes)
    for _ in 0..5 {
        library.improve_skill("reach", 0.03);
    }
    let after_positive = library.get("reach").unwrap().confidence;
    println!("    After 5 successes: confidence={:.4} (+{:.4})", after_positive, after_positive - before);

    // Negative feedback (2 failures)
    for _ in 0..2 {
        library.improve_skill("reach", -0.05);
    }
    let after_negative = library.get("reach").unwrap().confidence;
    println!("    After 2 failures:  confidence={:.4} ({:.4})", after_negative, after_negative - after_positive);

    // -- Step 6: Summary --
    println!();
    println!("[6] Skill library summary:");
    for name in &["reach", "wave"] {
        if let Some(skill) = library.get(name) {
            println!(
                "    '{}': {} waypoints, confidence={:.3}, executed {} times",
                skill.name,
                skill.trajectory.len(),
                skill.confidence,
                skill.execution_count
            );
        }
    }

    println!();
    println!("[done] Skill learning example complete.");
}
