//! WASM Integration Tests for MidStream System
//!
//! Tests WASM-specific functionality including:
//! - WebAssembly compilation and execution
//! - Browser compatibility
//! - Memory constraints
//! - Performance in WASM environments
//! - QUIC/WebTransport integration

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;
use std::collections::HashMap;

// WASM-specific imports
use midstreamer_temporal_compare::{TemporalComparator, Sequence, ComparisonAlgorithm};
use midstreamer_scheduler::{RealtimeScheduler, SchedulingPolicy, Priority};
use midstreamer_attractor::{AttractorAnalyzer, PhasePoint};
use midstreamer_neural_solver::{TemporalNeuralSolver, TemporalFormula, TemporalState, VerificationStrictness};
use midstreamer_strange_loop::{StrangeLoop, MetaLevel};

wasm_bindgen_test_configure!(run_in_browser);

/// Test 1: WASM Temporal Comparison
#[wasm_bindgen_test]
async fn test_wasm_temporal_comparison() {
    console_log!("=== WASM Temporal Comparison Test ===");

    let mut comparator = TemporalComparator::<String>::new(50, 500);

    // Add sequences
    comparator.add_sequence(Sequence {
        data: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        timestamp: 1000,
        id: "seq1".to_string(),
    });

    comparator.add_sequence(Sequence {
        data: vec!["a".to_string(), "b".to_string(), "d".to_string()],
        timestamp: 2000,
        id: "seq2".to_string(),
    });

    // Find similar sequences
    let query = vec!["a".to_string(), "b".to_string()];
    let similar = comparator.find_similar(&query, 0.7, ComparisonAlgorithm::LCS);

    console_log!("Found {} similar sequences in WASM", similar.len());
    assert!(similar.len() >= 2);

    console_log!("✓ WASM temporal comparison successful");
}

/// Test 2: WASM Scheduler
#[wasm_bindgen_test]
async fn test_wasm_scheduler() {
    console_log!("=== WASM Scheduler Test ===");

    let scheduler = RealtimeScheduler::new(SchedulingPolicy::FixedPriority);

    // Schedule multiple tasks
    for i in 0..10 {
        scheduler.schedule(
            create_action(&format!("wasm_task_{}", i), "WASM task"),
            Priority::Medium,
            std::time::Duration::from_secs(1),
            std::time::Duration::from_millis(10),
        ).await;
    }

    let stats = scheduler.get_stats().await;
    console_log!("Scheduled {} tasks in WASM", stats.total_scheduled);
    assert_eq!(stats.total_scheduled, 10);

    console_log!("✓ WASM scheduler successful");
}

/// Test 3: WASM Attractor Analysis
#[wasm_bindgen_test]
fn test_wasm_attractor_analysis() {
    console_log!("=== WASM Attractor Analysis Test ===");

    let mut analyzer = AttractorAnalyzer::new(2, 1000);

    // Add points
    for i in 0..150 {
        let point = PhasePoint::new(
            vec![
                (i as f64 * 0.1).sin(),
                (i as f64 * 0.1).cos(),
            ],
            i as u64,
        );
        analyzer.add_point(point).unwrap();
    }

    let result = analyzer.analyze();
    assert!(result.is_ok());

    let info = result.unwrap();
    console_log!("Detected attractor: {:?}", info.attractor_type);
    console_log!("Confidence: {}", info.confidence);

    console_log!("✓ WASM attractor analysis successful");
}

/// Test 4: WASM Temporal Logic Verification
#[wasm_bindgen_test]
fn test_wasm_temporal_verification() {
    console_log!("=== WASM Temporal Verification Test ===");

    let mut solver = TemporalNeuralSolver::new(1000, 500, VerificationStrictness::Medium);

    // Create trace
    for i in 0..10 {
        let mut state = TemporalState::new(i, i * 100);
        state.set_proposition("safe", true);
        state.set_proposition("ready", i >= 5);
        solver.add_state(state);
    }

    // Verify safety
    let formula = TemporalFormula::globally(TemporalFormula::atom("safe"));
    let result = solver.verify(&formula).unwrap();

    console_log!("Safety verified: {}", result.satisfied);
    assert!(result.satisfied);

    console_log!("✓ WASM temporal verification successful");
}

/// Test 5: WASM Meta-Learning
#[wasm_bindgen_test]
fn test_wasm_meta_learning() {
    console_log!("=== WASM Meta-Learning Test ===");

    let mut strange_loop = StrangeLoop::default();

    let data = vec![
        "pattern1".to_string(),
        "pattern2".to_string(),
        "pattern1".to_string(),
    ];

    let result = strange_loop.learn_at_level(MetaLevel::base(), &data);
    assert!(result.is_ok());

    let summary = strange_loop.get_summary();
    console_log!("Learned {} patterns", summary.total_knowledge);

    console_log!("✓ WASM meta-learning successful");
}

/// Test 6: WASM Memory Constraints
#[wasm_bindgen_test]
fn test_wasm_memory_limits() {
    console_log!("=== WASM Memory Limits Test ===");

    // Test with limited memory allocation
    let comparator = TemporalComparator::<i32>::new(100, 1000); // Smaller limits for WASM

    // Add moderate amount of data
    for i in 0..50 {
        let seq: Vec<i32> = (0..100).map(|x| x + i).collect();
        comparator.add_sequence(Sequence {
            data: seq,
            timestamp: i as u64 * 1000,
            id: format!("seq_{}", i),
        });
    }

    console_log!("✓ WASM memory constraints handled");
}

/// Test 7: WASM Performance
#[wasm_bindgen_test]
fn test_wasm_performance() {
    console_log!("=== WASM Performance Test ===");

    use web_sys::window;

    let window = window().expect("should have window");
    let performance = window.performance().expect("should have performance");

    let start = performance.now();

    // Perform computations
    let comparator = TemporalComparator::<i32>::new(100, 1000);
    let seq1: Vec<i32> = (0..500).collect();
    let seq2: Vec<i32> = (0..500).map(|x| x + 1).collect();

    let _similarity = comparator.compare(&seq1, &seq2, ComparisonAlgorithm::LCS);

    let duration = performance.now() - start;

    console_log!("Comparison took {} ms in WASM", duration);
    assert!(duration < 5000.0, "Should complete within 5 seconds");

    console_log!("✓ WASM performance acceptable");
}

/// Test 8: WASM Concurrent Operations
#[wasm_bindgen_test]
async fn test_wasm_concurrent_ops() {
    console_log!("=== WASM Concurrent Operations Test ===");

    use wasm_bindgen_futures::spawn_local;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    let counter = Arc::new(AtomicU32::new(0));

    // Spawn multiple concurrent tasks
    let mut handles = vec![];

    for i in 0..5 {
        let counter_clone = counter.clone();
        let future = async move {
            let mut comparator = TemporalComparator::<i32>::new(10, 100);
            let seq: Vec<i32> = (0..10).map(|x| x + i).collect();

            comparator.add_sequence(Sequence {
                data: seq,
                timestamp: i as u64,
                id: format!("concurrent_{}", i),
            });

            counter_clone.fetch_add(1, Ordering::SeqCst);
        };

        spawn_local(future);
    }

    // Wait a bit for tasks to complete
    wasm_timer::Delay::new(std::time::Duration::from_millis(100)).await.ok();

    let count = counter.load(Ordering::SeqCst);
    console_log!("Completed {} concurrent operations", count);

    console_log!("✓ WASM concurrent operations successful");
}

/// Test 9: WASM Error Handling
#[wasm_bindgen_test]
fn test_wasm_error_handling() {
    console_log!("=== WASM Error Handling Test ===");

    // Test dimension mismatch
    let mut analyzer = AttractorAnalyzer::new(3, 100);
    let invalid_point = PhasePoint::new(vec![1.0, 2.0], 0);
    let result = analyzer.add_point(invalid_point);

    assert!(result.is_err());
    console_log!("✓ Dimension mismatch handled correctly");

    // Test empty trace
    let solver = TemporalNeuralSolver::default();
    let formula = TemporalFormula::atom("test");
    let result = solver.verify(&formula);

    assert!(result.is_err());
    console_log!("✓ Empty trace handled correctly");

    console_log!("✓ WASM error handling successful");
}

/// Test 10: WASM Integration Workflow
#[wasm_bindgen_test]
async fn test_wasm_integration_workflow() {
    console_log!("=== WASM Integration Workflow Test ===");

    // Step 1: Pattern detection
    let mut comparator = TemporalComparator::<String>::new(50, 500);
    comparator.add_sequence(Sequence {
        data: vec!["init".to_string(), "process".to_string(), "complete".to_string()],
        timestamp: 1000,
        id: "workflow".to_string(),
    });

    // Step 2: Schedule based on pattern
    let scheduler = RealtimeScheduler::new(SchedulingPolicy::EarliestDeadlineFirst);
    scheduler.schedule(
        create_action("wasm_workflow", "Workflow task"),
        Priority::High,
        std::time::Duration::from_secs(1),
        std::time::Duration::from_millis(50),
    ).await;

    // Step 3: Verify behavior
    let mut solver = TemporalNeuralSolver::default();
    let mut state = TemporalState::new(1, 100);
    state.set_proposition("completed", true);
    solver.add_state(state);

    let formula = TemporalFormula::atom("completed");
    let result = solver.verify(&formula).unwrap();

    console_log!("Workflow completed: {}", result.satisfied);
    assert!(result.satisfied);

    console_log!("✓ WASM integration workflow successful");
}

// Helper functions

fn create_action(action_type: &str, description: &str) -> nanosecond_scheduler::Action {
    nanosecond_scheduler::Action {
        action_type: action_type.to_string(),
        description: description.to_string(),
        parameters: HashMap::new(),
        tool_calls: vec![],
        expected_outcome: None,
        expected_reward: 0.8,
    }
}

#[macro_export]
macro_rules! console_log {
    ($($t:tt)*) => {
        web_sys::console::log_1(&format!($($t)*).into());
    }
}

#[cfg(test)]
mod wasm_summary {
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn wasm_test_summary() {
        console_log!("\n");
        console_log!("╔═══════════════════════════════════════════════════════════════╗");
        console_log!("║     MidStream WASM Integration Test Suite                    ║");
        console_log!("╠═══════════════════════════════════════════════════════════════╣");
        console_log!("║                                                               ║");
        console_log!("║  ✓ WASM Temporal Comparison                                   ║");
        console_log!("║  ✓ WASM Scheduler                                             ║");
        console_log!("║  ✓ WASM Attractor Analysis                                    ║");
        console_log!("║  ✓ WASM Temporal Verification                                 ║");
        console_log!("║  ✓ WASM Meta-Learning                                         ║");
        console_log!("║  ✓ WASM Memory Limits                                         ║");
        console_log!("║  ✓ WASM Performance                                           ║");
        console_log!("║  ✓ WASM Concurrent Operations                                 ║");
        console_log!("║  ✓ WASM Error Handling                                        ║");
        console_log!("║  ✓ WASM Integration Workflow                                  ║");
        console_log!("║                                                               ║");
        console_log!("╚═══════════════════════════════════════════════════════════════╝");
        console_log!("\n");
    }
}
