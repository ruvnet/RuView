//! Example: full perceive-think-act-learn cognitive loop.
//!
//! Demonstrates:
//! - Feeding percepts into the `CognitiveCore`
//! - Producing decisions via the think phase
//! - Executing actions and incorporating feedback

use ruvector_robotics::cognitive::{
    ActionType, CognitiveConfig, CognitiveCore, CognitiveMode, Outcome, Percept,
};

fn main() {
    println!("=== Cognitive Loop Demo ===\n");

    let config = CognitiveConfig {
        mode: CognitiveMode::Deliberative,
        attention_threshold: 0.4,
        learning_rate: 0.05,
        max_percepts: 50,
    };
    let mut core = CognitiveCore::new(config);

    // Simulate 5 cycles.
    let sensor_data = [
        ("lidar", vec![1.0, 0.5, 0.0], 0.9),
        ("camera", vec![0.0, 1.0, 0.2], 0.7),
        ("lidar", vec![2.0, 0.0, 1.5], 0.95),
        ("ir_sensor", vec![0.1, 0.1, 0.1], 0.3), // below threshold
        ("camera", vec![3.0, 2.0, 0.0], 0.85),
    ];

    for (cycle, (source, data, confidence)) in sensor_data.iter().enumerate() {
        println!("--- Cycle {} ---", cycle + 1);

        // Perceive
        let percept = Percept {
            source: source.to_string(),
            data: data.clone(),
            confidence: *confidence,
            timestamp: (cycle as i64 + 1) * 1000,
        };
        let state = core.perceive(percept);
        println!(
            "  Perceived: source={}, confidence={:.2}, state={:?}",
            source, confidence, state
        );
        println!("  Buffer size: {}", core.percept_count());

        // Think
        if let Some(decision) = core.think() {
            println!("  Decision: {}", decision.reasoning);
            println!("  Priority: {}", decision.action.priority);

            // Act
            let cmd = core.act(decision);
            match &cmd.action {
                ActionType::Move(pos) => println!("  Action: Move to [{:.1}, {:.1}, {:.1}]", pos[0], pos[1], pos[2]),
                ActionType::Wait(ms) => println!("  Action: Wait {}ms", ms),
                _ => println!("  Action: {:?}", cmd.action),
            }

            // Learn
            let success = *confidence > 0.8;
            core.learn(Outcome {
                success,
                reward: if success { 1.0 } else { -0.5 },
                description: format!("cycle_{}", cycle),
            });
            println!(
                "  Learned: success={}, cumulative_reward={:.4}",
                success,
                core.cumulative_reward()
            );
        } else {
            println!("  No decision (empty buffer or below threshold)");
            println!("  State: {:?}", core.state());
        }

        println!();
    }

    println!("Total decisions made: {}", core.decision_count());
}
