//! Self-healing system example

use ruvector_dag::healing::{
    AnomalyConfig, AnomalyDetector, HealingOrchestrator, IndexHealth, IndexHealthChecker,
    IndexThresholds, IndexType, LearningDriftDetector,
};
use std::time::Instant;

fn main() {
    println!("=== Self-Healing System Demo ===\n");

    // Create healing orchestrator
    let mut orchestrator = HealingOrchestrator::new();

    // Add detectors for different metrics
    orchestrator.add_detector(
        "query_latency",
        AnomalyConfig {
            z_threshold: 3.0,
            window_size: 100,
            min_samples: 10,
        },
    );

    orchestrator.add_detector(
        "pattern_quality",
        AnomalyConfig {
            z_threshold: 2.5,
            window_size: 50,
            min_samples: 5,
        },
    );

    orchestrator.add_detector(
        "memory_usage",
        AnomalyConfig {
            z_threshold: 2.0,
            window_size: 50,
            min_samples: 5,
        },
    );

    println!("Orchestrator configured:");
    println!("  Detectors: 3 (query_latency, pattern_quality, memory_usage)");
    println!("  Repair strategies: Built-in cache flush and index rebuild");

    // Simulate normal operation
    println!("\n--- Normal Operation ---");
    for i in 0..50 {
        // Normal query latency: 100ms ± 20ms
        let latency = 100.0 + (rand::random::<f64>() - 0.5) * 40.0;
        orchestrator.observe("query_latency", latency);

        // Normal pattern quality: 0.9 ± 0.1
        let quality = 0.9 + (rand::random::<f64>() - 0.5) * 0.2;
        orchestrator.observe("pattern_quality", quality);

        // Normal memory: 1000 ± 100 MB
        let memory = 1000.0 + (rand::random::<f64>() - 0.5) * 200.0;
        orchestrator.observe("memory_usage", memory);

        if i % 10 == 9 {
            let result = orchestrator.run_cycle();
            let failures = result.repairs_attempted - result.repairs_succeeded;
            println!(
                "Cycle {}: {} anomalies, {} repairs, {} failures",
                i + 1,
                result.anomalies_detected,
                result.repairs_succeeded,
                failures
            );
        }
    }

    println!(
        "\nHealth Score after normal operation: {:.2}",
        orchestrator.health_score()
    );

    // Inject anomalies
    println!("\n--- Injecting Anomalies ---");

    // Spike in latency
    orchestrator.observe("query_latency", 500.0);
    orchestrator.observe("query_latency", 450.0);
    println!("  Injected latency spike: 500ms, 450ms");

    // Drop in quality
    orchestrator.observe("pattern_quality", 0.3);
    orchestrator.observe("pattern_quality", 0.4);
    println!("  Injected quality drop: 0.3, 0.4");

    let result = orchestrator.run_cycle();
    println!("\nAfter anomalies:");
    println!("  Detected: {}", result.anomalies_detected);
    println!("  Repairs succeeded: {}", result.repairs_succeeded);
    println!(
        "  Repairs failed: {}",
        result.repairs_attempted - result.repairs_succeeded
    );
    println!("  Health Score: {:.2}", orchestrator.health_score());

    // Recovery phase
    println!("\n--- Recovery Phase ---");
    for i in 0..20 {
        let latency = 100.0 + (rand::random::<f64>() - 0.5) * 40.0;
        orchestrator.observe("query_latency", latency);

        let quality = 0.9 + (rand::random::<f64>() - 0.5) * 0.2;
        orchestrator.observe("pattern_quality", quality);
    }

    let result = orchestrator.run_cycle();
    println!(
        "After recovery: {} anomalies, health score: {:.2}",
        result.anomalies_detected,
        orchestrator.health_score()
    );

    // Demonstrate index health checking
    println!("\n--- Index Health Check ---");
    let checker = IndexHealthChecker::new(IndexThresholds::default());

    let healthy_index = IndexHealth {
        index_name: "vectors_hnsw".to_string(),
        index_type: IndexType::Hnsw,
        fragmentation: 0.1,
        recall_estimate: 0.98,
        node_count: 100000,
        last_rebalanced: Some(Instant::now()),
    };

    let result = checker.check_health(&healthy_index);
    println!("\nHealthy HNSW index:");
    println!("  Status: {:?}", result.status);
    println!("  Issues: {}", result.issues.len());

    let fragmented_index = IndexHealth {
        index_name: "vectors_ivf".to_string(),
        index_type: IndexType::IvfFlat,
        fragmentation: 0.45,
        recall_estimate: 0.85,
        node_count: 50000,
        last_rebalanced: None,
    };

    let result = checker.check_health(&fragmented_index);
    println!("\nFragmented IVF-Flat index:");
    println!("  Status: {:?}", result.status);
    println!("  Issues: {:?}", result.issues);
    println!("  Recommendations:");
    for rec in &result.recommendations {
        println!("    - {}", rec);
    }

    // Demonstrate drift detection
    println!("\n--- Learning Drift Detection ---");
    let mut drift = LearningDriftDetector::new(0.1, 20);

    drift.set_baseline("accuracy", 0.95);
    drift.set_baseline("recall", 0.92);

    println!("Baselines set:");
    println!("  accuracy: 0.95");
    println!("  recall: 0.92");

    // Simulate declining accuracy
    println!("\nSimulating accuracy decline...");
    for i in 0..20 {
        let accuracy = 0.95 - (i as f64) * 0.015;
        drift.record("accuracy", accuracy);

        // Recall stays stable
        let recall = 0.92 + (rand::random::<f64>() - 0.5) * 0.02;
        drift.record("recall", recall);
    }

    if let Some(metric) = drift.check_drift("accuracy") {
        println!("\nDrift detected in accuracy:");
        println!("  Current: {:.3}", metric.current_value);
        println!("  Baseline: {:.3}", metric.baseline_value);
        println!("  Magnitude: {:.3}", metric.drift_magnitude);
        println!("  Trend: {:?}", metric.trend);
        println!(
            "  Severity: {}",
            if metric.drift_magnitude > 0.2 {
                "HIGH"
            } else if metric.drift_magnitude > 0.1 {
                "MEDIUM"
            } else {
                "LOW"
            }
        );
    }

    if drift.check_drift("recall").is_none() {
        println!("\nNo drift detected in recall (stable)");
    }

    println!("\n=== Example Complete ===");
}
