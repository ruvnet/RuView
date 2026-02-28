//! Gating subsystem benchmarks

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;

use ruvector_fpga_transformer::{
    artifact::{Manifest, ModelArtifact},
    backend::native_sim::NativeSimBackend,
    backend::TransformerBackend,
    gating::{CoherenceConfig, CoherenceGate, DefaultCoherenceGate},
    types::{ComputeClass, FixedShape, GateDecision, GateHint, InferenceRequest, QuantSpec},
};

fn bench_skip_rate_distribution(c: &mut Criterion) {
    let gate = DefaultCoherenceGate::new();

    // Generate synthetic coherence distribution
    let coherence_values: Vec<i16> = (-500..500).collect();

    c.bench_function("skip_rate_uniform_distribution", |b| {
        b.iter(|| {
            let mut skipped = 0u32;
            let mut ran = 0u32;

            for &coherence in &coherence_values {
                let hint = GateHint::new(coherence, false, ComputeClass::Deliberative);
                match gate.preflight(black_box(&hint)) {
                    GateDecision::Skipped { .. } => skipped += 1,
                    _ => ran += 1,
                }
            }

            (skipped, ran)
        })
    });
}

fn bench_early_exit_histogram(c: &mut Criterion) {
    let gate = Arc::new(DefaultCoherenceGate::new());
    let backend = NativeSimBackend::new(gate);

    let shape = FixedShape::micro();
    let manifest = Manifest {
        name: "early_exit_test".into(),
        model_hash: String::new(),
        shape,
        quant: QuantSpec::int8(),
        io: Default::default(),
        backend: Default::default(),
        tests: Default::default(),
    };

    let embedding_size = shape.vocab as usize * shape.d_model as usize;
    let artifact = ModelArtifact::new(manifest, vec![0u8; embedding_size], None, None, vec![]);
    let model_id = backend.load(&artifact).unwrap();

    let tokens: Vec<u16> = (0..shape.seq_len).collect();
    let mask = vec![1u8; shape.seq_len as usize];

    // Test with varying coherence levels
    let coherence_levels: Vec<i16> = vec![-500, -200, 0, 200, 500, 1000, 2000];

    let mut group = c.benchmark_group("early_exit_by_coherence");

    for coherence in coherence_levels {
        group.bench_with_input(
            BenchmarkId::new("coherence", coherence),
            &coherence,
            |b, &coherence| {
                let hint = GateHint::new(coherence, false, ComputeClass::Deliberative);

                b.iter(|| {
                    let req =
                        InferenceRequest::new(model_id, shape, black_box(&tokens), &mask, hint);
                    let result = backend.infer(req).unwrap();
                    result.witness.gate_decision
                })
            },
        );
    }

    group.finish();
}

fn bench_checkpoint_overhead(c: &mut Criterion) {
    let configs = [
        ("default", CoherenceConfig::default()),
        ("strict", CoherenceConfig::strict()),
        ("permissive", CoherenceConfig::permissive()),
    ];

    let mut group = c.benchmark_group("checkpoint_overhead");

    for (name, config) in configs {
        let gate = DefaultCoherenceGate::with_config(config);

        group.bench_with_input(BenchmarkId::new("config", name), &gate, |b, gate| {
            b.iter(|| {
                let mut decision = None;
                for layer in 0u8..8 {
                    let signal = (layer as i16) * 150;
                    if let Some(d) = gate.checkpoint(black_box(layer), black_box(signal)) {
                        decision = Some(d);
                        break;
                    }
                }
                decision
            })
        });
    }

    group.finish();
}

fn bench_mincut_gating(c: &mut Criterion) {
    use ruvector_fpga_transformer::gating::coherence_gate::MincutCoherenceGate;

    let config = CoherenceConfig::default();
    let gate = MincutCoherenceGate::new(config, 50, 200);

    let hints = [
        (
            "high_lambda",
            GateHint::new(500, false, ComputeClass::Deliberative),
        ),
        (
            "low_lambda",
            GateHint::new(100, false, ComputeClass::Deliberative),
        ),
        (
            "boundary_crossed",
            GateHint::new(300, true, ComputeClass::Deliberative),
        ),
    ];

    let mut group = c.benchmark_group("mincut_gating");

    for (name, hint) in hints {
        group.bench_with_input(BenchmarkId::new("preflight", name), &hint, |b, hint| {
            b.iter(|| gate.preflight(black_box(hint)))
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_skip_rate_distribution,
    bench_early_exit_histogram,
    bench_checkpoint_overhead,
    bench_mincut_gating
);
criterion_main!(benches);
