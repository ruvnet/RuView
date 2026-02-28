/// Example 05: Cognitive Robot - Full perceive-think-act-learn loop
///
/// Demonstrates:
/// - Using CognitiveCore for autonomous decision making
/// - The perceive -> think -> act -> learn cycle
/// - Attention threshold adaptation from feedback
/// - Decision history and cumulative reward tracking

use ruvector_robotics::cognitive::{
    ActionType, CognitiveConfig, CognitiveCore, CognitiveMode, Outcome, Percept,
};

fn main() {
    println!("=== Example 05: Cognitive Robot ===");
    println!();

    // Step 1: Initialize cognitive core
    let config = CognitiveConfig {
        mode: CognitiveMode::Reactive,
        attention_threshold: 0.5,
        learning_rate: 0.01,
        max_percepts: 100,
    };
    let mut core = CognitiveCore::new(config);

    println!("[1] Cognitive core initialized:");
    println!("    Mode: {:?}", core.mode());
    println!("    State: {:?}", core.state());
    println!();

    // Step 2: Simulate 10 cognitive cycles
    println!("[2] Running 10 cognitive cycles:");
    println!();

    let sensor_data: Vec<(&str, Vec<f64>, f64)> = vec![
        ("lidar", vec![2.0, 1.5, 0.0], 0.9),
        ("camera", vec![-1.0, 3.0, 0.5], 0.85),
        ("imu", vec![0.1, 0.2], 0.3),        // below threshold -- will be dropped
        ("lidar", vec![4.0, 0.0, 0.0], 0.95),
        ("camera", vec![0.0, 5.0, 1.0], 0.7),
        ("sonar", vec![1.0, 1.0, 0.0], 0.6),
        ("camera", vec![-3.0, -2.0, 0.0], 0.88),
        ("lidar", vec![0.5, 0.5, 0.0], 0.92),
        ("depth", vec![2.5, 2.5, 1.0], 0.75),
        ("camera", vec![6.0, 0.0, 0.0], 0.4), // near threshold
    ];

    for (i, (source, data, confidence)) in sensor_data.iter().enumerate() {
        println!("--- Cycle {} ---", i);

        // PERCEIVE
        let percept = Percept {
            source: source.to_string(),
            data: data.clone(),
            confidence: *confidence,
            timestamp: (i * 100) as i64,
        };

        let state = core.perceive(percept);
        println!(
            "  Perceive: source='{}', conf={:.2}, buffered={} [state={:?}]",
            source, confidence, core.percept_count(), state
        );

        // THINK
        if let Some(decision) = core.think() {
            let action_desc = match &decision.action.action {
                ActionType::Move(pos) => format!("Move({:.1}, {:.1}, {:.1})", pos[0], pos[1], pos[2]),
                ActionType::Wait(ms) => format!("Wait({}ms)", ms),
                _ => format!("{:?}", decision.action.action),
            };
            println!(
                "  Think:    {} (utility={:.2}, priority={})",
                decision.reasoning, decision.utility, decision.action.priority
            );

            // ACT
            let _command = core.act(decision);
            println!("  Act:      {} [state={:?}]", action_desc, core.state());

            // LEARN
            let success = *confidence > 0.7;
            let reward = if success { 1.0 } else { -0.5 };
            core.learn(Outcome {
                success,
                reward,
                description: format!("cycle_{}", i),
            });
            println!(
                "  Learn:    success={}, reward={:.1}, cumulative={:.4}",
                success, reward, core.cumulative_reward()
            );
        } else {
            println!("  Think:    no percepts to reason about");
        }
        println!();
    }

    // Step 3: Summary
    println!("[3] Cognitive summary after 10 cycles:");
    println!("    State: {:?}", core.state());
    println!("    Decisions made: {}", core.decision_count());
    println!("    Cumulative reward: {:.6}", core.cumulative_reward());
    println!("    Buffered percepts: {}", core.percept_count());

    // Step 4: Emergency mode demonstration
    println!();
    println!("[4] Emergency mode:");
    let mut emergency_core = CognitiveCore::new(CognitiveConfig {
        mode: CognitiveMode::Emergency,
        attention_threshold: 0.1,
        learning_rate: 0.05,
        max_percepts: 10,
    });
    emergency_core.perceive(Percept {
        source: "collision_sensor".into(),
        data: vec![0.0, 0.0, 0.0],
        confidence: 0.99,
        timestamp: 0,
    });
    if let Some(decision) = emergency_core.think() {
        println!("    Priority: {} (max for emergency)", decision.action.priority);
        println!("    Reasoning: {}", decision.reasoning);
    }

    println!();
    println!("[done] Cognitive robot example complete.");
}
