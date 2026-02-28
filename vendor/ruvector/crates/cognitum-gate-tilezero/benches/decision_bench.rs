//! Benchmarks for the full decision pipeline
//!
//! Target latencies:
//! - Gate decision: p99 < 50ms
//! - E-value computation: < 1ms

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;

use cognitum_gate_tilezero::{
    ActionContext, ActionMetadata, ActionTarget, DecisionOutcome, EvidenceFilter, GateThresholds,
    ReducedGraph, ThreeFilterDecision, TileZero,
};

/// Create a realistic action context for benchmarking
fn create_action_context(id: usize) -> ActionContext {
    ActionContext {
        action_id: format!("action-{}", id),
        action_type: "config_change".to_string(),
        target: ActionTarget {
            device: Some("router-1".to_string()),
            path: Some("/config/routing/policy".to_string()),
            extra: {
                let mut m = HashMap::new();
                m.insert("priority".to_string(), serde_json::json!(100));
                m.insert("region".to_string(), serde_json::json!("us-west-2"));
                m
            },
        },
        context: ActionMetadata {
            agent_id: "agent-001".to_string(),
            session_id: Some("session-12345".to_string()),
            prior_actions: vec!["action-prev-1".to_string(), "action-prev-2".to_string()],
            urgency: "normal".to_string(),
        },
    }
}

/// Create a graph with realistic state
fn create_realistic_graph(coherence_level: f64) -> ReducedGraph {
    let mut graph = ReducedGraph::new();

    // Simulate 255 worker tiles reporting
    for tile_id in 1..=255u8 {
        // Vary coherence slightly around the target
        let tile_coherence = (coherence_level + (tile_id as f64 * 0.001) % 0.1) as f32;
        graph.update_coherence(tile_id, tile_coherence);
    }

    // Set realistic values
    graph.set_global_cut(coherence_level * 15.0);
    graph.set_evidence(coherence_level * 150.0);
    graph.set_shift_pressure(0.1 * (1.0 - coherence_level));

    graph
}

/// Benchmark the full TileZero decision pipeline
fn bench_full_decision_pipeline(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("decision_pipeline");
    group.throughput(Throughput::Elements(1));

    // Benchmark with different threshold configurations
    let thresholds_configs = vec![
        ("default", GateThresholds::default()),
        (
            "strict",
            GateThresholds {
                tau_deny: 0.001,
                tau_permit: 200.0,
                min_cut: 10.0,
                max_shift: 0.3,
                permit_ttl_ns: 30_000_000_000,
                theta_uncertainty: 30.0,
                theta_confidence: 3.0,
            },
        ),
        (
            "relaxed",
            GateThresholds {
                tau_deny: 0.1,
                tau_permit: 50.0,
                min_cut: 2.0,
                max_shift: 0.8,
                permit_ttl_ns: 120_000_000_000,
                theta_uncertainty: 10.0,
                theta_confidence: 10.0,
            },
        ),
    ];

    for (name, thresholds) in thresholds_configs {
        let tilezero = TileZero::new(thresholds);
        let ctx = create_action_context(0);

        group.bench_with_input(BenchmarkId::new("tilezero_decide", name), &ctx, |b, ctx| {
            b.to_async(&rt)
                .iter(|| async { black_box(tilezero.decide(black_box(ctx)).await) });
        });
    }

    group.finish();
}

/// Benchmark the three-filter decision logic
fn bench_three_filter_decision(c: &mut Criterion) {
    let mut group = c.benchmark_group("three_filter_decision");
    group.throughput(Throughput::Elements(1));

    let thresholds = GateThresholds::default();
    let decision = ThreeFilterDecision::new(thresholds);

    // Test different graph states
    let graph_states = vec![
        ("high_coherence", create_realistic_graph(0.95)),
        ("medium_coherence", create_realistic_graph(0.7)),
        ("low_coherence", create_realistic_graph(0.3)),
    ];

    for (name, graph) in graph_states {
        group.bench_with_input(BenchmarkId::new("evaluate", name), &graph, |b, graph| {
            b.iter(|| black_box(decision.evaluate(black_box(graph))))
        });
    }

    group.finish();
}

/// Benchmark E-value computation (scalar)
fn bench_e_value_scalar(c: &mut Criterion) {
    let mut group = c.benchmark_group("e_value_computation");
    group.throughput(Throughput::Elements(1));

    // Test different filter capacities
    for capacity in [10, 100, 1000] {
        let mut filter = EvidenceFilter::new(capacity);

        // Pre-fill the filter
        for i in 0..capacity {
            filter.update(1.0 + (i as f64 * 0.001));
        }

        group.bench_with_input(
            BenchmarkId::new("scalar_update", capacity),
            &capacity,
            |b, _| {
                b.iter(|| {
                    filter.update(black_box(1.5));
                    black_box(filter.current())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark E-value computation with SIMD-friendly patterns
fn bench_e_value_simd(c: &mut Criterion) {
    let mut group = c.benchmark_group("e_value_simd");

    // Simulate SIMD batch processing of 255 tile e-values
    let tile_count = 255;
    group.throughput(Throughput::Elements(tile_count as u64));

    // Generate test data aligned for SIMD
    let e_values: Vec<f64> = (0..tile_count).map(|i| 1.0 + (i as f64 * 0.01)).collect();

    // Scalar baseline
    group.bench_function("aggregate_scalar", |b| {
        b.iter(|| {
            let product: f64 = e_values.iter().product();
            black_box(product)
        })
    });

    // Chunked processing (SIMD-friendly)
    group.bench_function("aggregate_chunked_4", |b| {
        b.iter(|| {
            let mut accumulator = 1.0f64;
            for chunk in e_values.chunks(4) {
                let chunk_product: f64 = chunk.iter().product();
                accumulator *= chunk_product;
            }
            black_box(accumulator)
        })
    });

    // Parallel reduction pattern
    group.bench_function("aggregate_parallel_reduction", |b| {
        b.iter(|| {
            // Split into 8 lanes for potential SIMD
            let mut lanes = [1.0f64; 8];
            for (i, &val) in e_values.iter().enumerate() {
                lanes[i % 8] *= val;
            }
            let result: f64 = lanes.iter().product();
            black_box(result)
        })
    });

    group.finish();
}

/// Benchmark decision outcome creation
fn bench_decision_outcome(c: &mut Criterion) {
    let mut group = c.benchmark_group("decision_outcome");
    group.throughput(Throughput::Elements(1));

    group.bench_function("create_permit", |b| {
        b.iter(|| {
            black_box(DecisionOutcome::permit(
                black_box(0.95),
                black_box(1.0),
                black_box(0.9),
                black_box(0.95),
                black_box(10.0),
            ))
        })
    });

    group.bench_function("create_deny", |b| {
        b.iter(|| {
            black_box(DecisionOutcome::deny(
                cognitum_gate_tilezero::DecisionFilter::Structural,
                "Low coherence".to_string(),
                black_box(0.3),
                black_box(0.5),
                black_box(0.2),
                black_box(2.0),
            ))
        })
    });

    group.bench_function("create_defer", |b| {
        b.iter(|| {
            black_box(DecisionOutcome::defer(
                cognitum_gate_tilezero::DecisionFilter::Shift,
                "High shift pressure".to_string(),
                black_box(0.8),
                black_box(0.3),
                black_box(0.7),
                black_box(6.0),
            ))
        })
    });

    group.finish();
}

/// Benchmark witness summary generation
fn bench_witness_summary(c: &mut Criterion) {
    let mut group = c.benchmark_group("witness_summary");
    group.throughput(Throughput::Elements(1));

    let graph = create_realistic_graph(0.9);

    group.bench_function("generate", |b| {
        b.iter(|| black_box(graph.witness_summary()))
    });

    let summary = graph.witness_summary();
    group.bench_function("hash", |b| b.iter(|| black_box(summary.hash())));

    group.bench_function("to_json", |b| b.iter(|| black_box(summary.to_json())));

    group.finish();
}

/// Benchmark batch decision processing
fn bench_batch_decisions(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("batch_decisions");

    for batch_size in [10, 50, 100] {
        group.throughput(Throughput::Elements(batch_size as u64));

        let thresholds = GateThresholds::default();
        let tilezero = TileZero::new(thresholds);

        let contexts: Vec<_> = (0..batch_size).map(create_action_context).collect();

        group.bench_with_input(
            BenchmarkId::new("sequential", batch_size),
            &contexts,
            |b, contexts| {
                b.to_async(&rt).iter(|| async {
                    for ctx in contexts {
                        black_box(tilezero.decide(ctx).await);
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark graph updates from tile reports
fn bench_graph_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_updates");

    for tile_count in [64, 128, 255] {
        group.throughput(Throughput::Elements(tile_count as u64));

        group.bench_with_input(
            BenchmarkId::new("coherence_updates", tile_count),
            &tile_count,
            |b, &count| {
                b.iter(|| {
                    let mut graph = ReducedGraph::new();
                    for tile_id in 1..=count as u8 {
                        graph.update_coherence(tile_id, black_box(0.9));
                    }
                    black_box(graph)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_full_decision_pipeline,
    bench_three_filter_decision,
    bench_e_value_scalar,
    bench_e_value_simd,
    bench_decision_outcome,
    bench_witness_summary,
    bench_batch_decisions,
    bench_graph_updates,
);

criterion_main!(benches);
