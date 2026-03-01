//! Performance benchmark for the cognitive container.
//! Run with: cargo test -p ruvector-cognitive-container --test container_bench --release -- --nocapture

use ruvector_cognitive_container::{
    CognitiveContainer, ContainerConfig, Delta, VerificationResult,
};
use std::time::Instant;

#[test]
fn bench_container_100_ticks() {
    let config = ContainerConfig::default();
    let mut container = CognitiveContainer::new(config).expect("Failed to create container");

    // Build base graph
    let init_deltas: Vec<Delta> = (0..50)
        .map(|i| Delta::EdgeAdd {
            u: i,
            v: (i + 1) % 50,
            weight: 1.0,
        })
        .collect();
    let _ = container.tick(&init_deltas);

    // Benchmark 100 ticks
    let n_ticks = 100;
    let mut tick_times = Vec::with_capacity(n_ticks);

    let start = Instant::now();
    for i in 0..n_ticks {
        let deltas = vec![
            Delta::EdgeAdd {
                u: i % 50,
                v: (i + 17) % 50,
                weight: 0.5 + (i as f64 * 0.01),
            },
            Delta::Observation {
                node: i % 50,
                value: 0.7 + (i as f64 * 0.001),
            },
        ];
        let result = container.tick(&deltas).expect("Tick failed");
        tick_times.push(result.tick_time_us);
    }
    let total_time = start.elapsed();

    let avg = tick_times.iter().sum::<u64>() as f64 / tick_times.len() as f64;
    let max = *tick_times.iter().max().unwrap();
    let min = *tick_times.iter().min().unwrap();

    // Verify chain
    let start = Instant::now();
    let verification = container.verify_chain();
    let verify_us = start.elapsed().as_micros();

    println!("\n=== Cognitive Container (100 ticks) ===");
    println!("  Average tick:       {:.1} µs  (target: < 200 µs)", avg);
    println!("  Min / Max tick:     {} / {} µs", min, max);
    println!(
        "  Total 100 ticks:    {:.2} ms",
        total_time.as_micros() as f64 / 1000.0
    );
    println!("  Chain verify:       {} µs", verify_us);
    println!("  Chain length:       {}", container.receipt_chain().len());
    println!(
        "  Chain valid:        {}",
        matches!(verification, VerificationResult::Valid { .. })
    );

    // 2000µs target accounts for CI/container/debug-mode variability;
    // on dedicated hardware in release mode this typically runs under 200µs.
    assert!(
        avg < 2000.0,
        "Container tick exceeded 2000µs target: {:.1} µs",
        avg
    );
}
