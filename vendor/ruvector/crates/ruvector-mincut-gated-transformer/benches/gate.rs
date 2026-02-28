//! Gate overhead benchmarks.
//!
//! Measures the cost of gate evaluation separate from inference.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_mincut_gated_transformer::{
    gate::GateController, spike::SpikeScheduler, GatePacket, GatePolicy, SpikePacket,
};

fn bench_gate_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_evaluation");

    let policy = GatePolicy::default();
    let controller = GateController::new(policy);

    // Allow case
    let gate_allow = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    group.bench_function("allow", |b| {
        b.iter(|| {
            let decision = controller.evaluate(black_box(&gate_allow), None);
            black_box(decision)
        })
    });

    // ReduceScope case
    let gate_reduce = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 30, // Triggers boundary spike
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    group.bench_function("reduce_scope", |b| {
        b.iter(|| {
            let decision = controller.evaluate(black_box(&gate_reduce), None);
            black_box(decision)
        })
    });

    // FlushKv case
    let gate_flush = GatePacket {
        lambda: 30,
        lambda_prev: 100, // 70% drop
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    group.bench_function("flush_kv", |b| {
        b.iter(|| {
            let decision = controller.evaluate(black_box(&gate_flush), None);
            black_box(decision)
        })
    });

    // QuarantineUpdates case
    let gate_quarantine = GatePacket {
        lambda: 10, // Below min
        lambda_prev: 100,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    group.bench_function("quarantine", |b| {
        b.iter(|| {
            let decision = controller.evaluate(black_box(&gate_quarantine), None);
            black_box(decision)
        })
    });

    group.finish();
}

fn bench_gate_with_spikes(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_with_spikes");

    let policy = GatePolicy::default();
    let controller = GateController::new(policy);

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    // Active spike
    let spike_active = SpikePacket {
        fired: 1,
        rate_q15: 10000,
        novelty_q15: 15000,
        ..Default::default()
    };

    group.bench_function("with_active_spike", |b| {
        b.iter(|| {
            let decision = controller.evaluate(black_box(&gate), Some(black_box(&spike_active)));
            black_box(decision)
        })
    });

    // Inactive spike
    let spike_inactive = SpikePacket {
        fired: 0,
        ..Default::default()
    };

    group.bench_function("with_inactive_spike", |b| {
        b.iter(|| {
            let decision = controller.evaluate(black_box(&gate), Some(black_box(&spike_inactive)));
            black_box(decision)
        })
    });

    // Storm spike
    let spike_storm = SpikePacket {
        fired: 1,
        rate_q15: 30000, // Very high
        novelty_q15: 5000,
        ..Default::default()
    };

    group.bench_function("with_storm_spike", |b| {
        b.iter(|| {
            let decision = controller.evaluate(black_box(&gate), Some(black_box(&spike_storm)));
            black_box(decision)
        })
    });

    group.finish();
}

fn bench_spike_scheduler(c: &mut Criterion) {
    let mut group = c.benchmark_group("spike_scheduler");

    let scheduler = SpikeScheduler::new();

    // Active spike
    let spike_active = SpikePacket {
        fired: 1,
        rate_q15: 10000,
        novelty_q15: 15000,
        top_len: 8,
        top_idx: [1, 5, 10, 15, 20, 25, 30, 35, 0, 0, 0, 0, 0, 0, 0, 0],
        top_w_q15: [
            16384, 8192, 4096, 2048, 1024, 512, 256, 128, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        flags: SpikePacket::FLAG_SPARSE_MASK,
    };

    group.bench_function("evaluate_active", |b| {
        b.iter(|| {
            let decision = scheduler.evaluate(black_box(&spike_active));
            black_box(decision)
        })
    });

    group.bench_function("build_sparse_mask", |b| {
        b.iter(|| {
            let mask = scheduler.build_sparse_mask(black_box(&spike_active), 64);
            black_box(mask)
        })
    });

    group.bench_function("get_weighted_positions", |b| {
        b.iter(|| {
            let positions = scheduler.get_weighted_positions(black_box(&spike_active));
            black_box(positions)
        })
    });

    group.finish();
}

fn bench_policy_variants(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy_variants");

    let gate = GatePacket {
        lambda: 50,
        lambda_prev: 80,
        boundary_edges: 15,
        boundary_concentration_q15: 15000,
        partition_count: 8,
        flags: 0,
    };

    // Default policy
    let default_controller = GateController::new(GatePolicy::default());

    group.bench_function("default_policy", |b| {
        b.iter(|| {
            let decision = default_controller.evaluate(black_box(&gate), None);
            black_box(decision)
        })
    });

    // Conservative policy
    let conservative_controller = GateController::new(GatePolicy::conservative());

    group.bench_function("conservative_policy", |b| {
        b.iter(|| {
            let decision = conservative_controller.evaluate(black_box(&gate), None);
            black_box(decision)
        })
    });

    // Permissive policy
    let permissive_controller = GateController::new(GatePolicy::permissive());

    group.bench_function("permissive_policy", |b| {
        b.iter(|| {
            let decision = permissive_controller.evaluate(black_box(&gate), None);
            black_box(decision)
        })
    });

    group.finish();
}

fn bench_drop_ratio_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("drop_ratio");

    for drop_percent in [10, 25, 50, 75].iter() {
        let gate = GatePacket {
            lambda: 100 - drop_percent,
            lambda_prev: 100,
            ..Default::default()
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(drop_percent),
            drop_percent,
            |b, _| {
                b.iter(|| {
                    let ratio = black_box(&gate).drop_ratio_q15();
                    black_box(ratio)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_gate_evaluation,
    bench_gate_with_spikes,
    bench_spike_scheduler,
    bench_policy_variants,
    bench_drop_ratio_calculation,
);

criterion_main!(benches);
