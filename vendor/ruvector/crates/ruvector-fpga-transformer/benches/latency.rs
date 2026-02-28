//! Latency benchmarks for FPGA Transformer

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;

use ruvector_fpga_transformer::{
    artifact::{Manifest, ModelArtifact},
    backend::native_sim::NativeSimBackend,
    backend::TransformerBackend,
    gating::DefaultCoherenceGate,
    types::{FixedShape, GateHint, InferenceRequest, ModelId, QuantSpec},
};

fn create_test_artifact(shape: FixedShape) -> ModelArtifact {
    let manifest = Manifest {
        name: "bench_model".into(),
        model_hash: String::new(),
        shape,
        quant: QuantSpec::int8(),
        io: Default::default(),
        backend: Default::default(),
        tests: Default::default(),
    };

    // Create minimal weights
    let embedding_size = shape.vocab as usize * shape.d_model as usize;
    let weights = vec![0u8; embedding_size];

    ModelArtifact::new(manifest, weights, None, None, vec![])
}

fn bench_inference(c: &mut Criterion) {
    let gate = Arc::new(DefaultCoherenceGate::new());
    let backend = NativeSimBackend::new(gate);

    let shape = FixedShape::micro();
    let artifact = create_test_artifact(shape);
    let model_id = backend.load(&artifact).unwrap();

    let tokens: Vec<u16> = (0..shape.seq_len).collect();
    let mask = vec![1u8; shape.seq_len as usize];

    c.bench_function("native_sim_micro_inference", |b| {
        b.iter(|| {
            let req = InferenceRequest::new(
                model_id,
                shape,
                black_box(&tokens),
                &mask,
                GateHint::allow_all(),
            );
            backend.infer(req).unwrap()
        })
    });
}

fn bench_inference_shapes(c: &mut Criterion) {
    let gate = Arc::new(DefaultCoherenceGate::new());

    let shapes = [
        ("micro", FixedShape::micro()),
        ("small", FixedShape::small()),
        ("baseline", FixedShape::baseline()),
    ];

    let mut group = c.benchmark_group("inference_by_shape");

    for (name, shape) in shapes {
        let backend = NativeSimBackend::new(gate.clone());
        let artifact = create_test_artifact(shape);
        let model_id = backend.load(&artifact).unwrap();

        let tokens: Vec<u16> = (0..shape.seq_len).collect();
        let mask = vec![1u8; shape.seq_len as usize];

        group.bench_with_input(BenchmarkId::new("native_sim", name), &shape, |b, &shape| {
            b.iter(|| {
                let req = InferenceRequest::new(
                    model_id,
                    shape,
                    black_box(&tokens),
                    &mask,
                    GateHint::allow_all(),
                );
                backend.infer(req).unwrap()
            })
        });
    }

    group.finish();
}

fn bench_load_unload(c: &mut Criterion) {
    let gate = Arc::new(DefaultCoherenceGate::new());
    let backend = NativeSimBackend::new(gate);

    let artifact = create_test_artifact(FixedShape::micro());

    c.bench_function("model_load", |b| {
        b.iter(|| {
            let id = backend.load(black_box(&artifact)).unwrap();
            backend.unload(id).unwrap();
        })
    });
}

fn bench_gating(c: &mut Criterion) {
    use ruvector_fpga_transformer::gating::{CoherenceConfig, CoherenceGate};

    let gate = DefaultCoherenceGate::with_config(CoherenceConfig::default());

    let hints = [
        ("allow_all", GateHint::allow_all()),
        ("reflex_only", GateHint::reflex_only()),
        (
            "low_coherence",
            GateHint::new(
                -500,
                true,
                ruvector_fpga_transformer::types::ComputeClass::Deliberative,
            ),
        ),
    ];

    let mut group = c.benchmark_group("gating_preflight");

    for (name, hint) in hints {
        group.bench_with_input(BenchmarkId::new("preflight", name), &hint, |b, hint| {
            b.iter(|| gate.preflight(black_box(hint)))
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_inference,
    bench_inference_shapes,
    bench_load_unload,
    bench_gating
);
criterion_main!(benches);
