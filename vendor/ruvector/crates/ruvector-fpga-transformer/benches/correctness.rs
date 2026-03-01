//! Correctness and determinism benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::Arc;

use ruvector_fpga_transformer::{
    artifact::{Manifest, ModelArtifact},
    backend::native_sim::NativeSimBackend,
    backend::TransformerBackend,
    gating::DefaultCoherenceGate,
    types::{FixedShape, GateHint, InferenceRequest, QuantSpec},
};

fn create_test_artifact() -> ModelArtifact {
    let shape = FixedShape::micro();
    let manifest = Manifest {
        name: "determinism_test".into(),
        model_hash: String::new(),
        shape,
        quant: QuantSpec::int8(),
        io: Default::default(),
        backend: Default::default(),
        tests: Default::default(),
    };

    let embedding_size = shape.vocab as usize * shape.d_model as usize;
    let weights: Vec<u8> = (0..embedding_size).map(|i| (i % 256) as u8).collect();

    ModelArtifact::new(manifest, weights, None, None, vec![])
}

fn bench_determinism(c: &mut Criterion) {
    let gate = Arc::new(DefaultCoherenceGate::new());
    let backend = NativeSimBackend::new(gate);

    let artifact = create_test_artifact();
    let model_id = backend.load(&artifact).unwrap();
    let shape = FixedShape::micro();

    let tokens: Vec<u16> = (0..shape.seq_len)
        .map(|i| (i * 7) % shape.vocab as u16)
        .collect();
    let mask = vec![1u8; shape.seq_len as usize];

    c.bench_function("determinism_check_1000", |b| {
        b.iter(|| {
            let mut first_hash: Option<u64> = None;

            for _ in 0..1000 {
                let req = InferenceRequest::new(
                    model_id,
                    shape,
                    black_box(&tokens),
                    &mask,
                    GateHint::allow_all(),
                );
                let result = backend.infer(req).unwrap();

                // Hash the logits
                let hash = result
                    .logits_q
                    .iter()
                    .fold(0u64, |acc, &v| acc.wrapping_mul(31).wrapping_add(v as u64));

                match first_hash {
                    None => first_hash = Some(hash),
                    Some(expected) => assert_eq!(hash, expected, "Non-deterministic output"),
                }
            }
        })
    });
}

fn bench_golden_vectors(c: &mut Criterion) {
    let gate = Arc::new(DefaultCoherenceGate::new());
    let backend = NativeSimBackend::new(gate);

    let artifact = create_test_artifact();
    let model_id = backend.load(&artifact).unwrap();
    let shape = FixedShape::micro();

    // Create golden vectors
    let test_inputs: Vec<Vec<u16>> = (0..128)
        .map(|seed| {
            (0..shape.seq_len)
                .map(|i| ((i as usize * seed + 1) % shape.vocab as usize) as u16)
                .collect()
        })
        .collect();

    let mask = vec![1u8; shape.seq_len as usize];

    // Compute expected outputs
    let expected: Vec<Vec<i16>> = test_inputs
        .iter()
        .map(|tokens| {
            let req = InferenceRequest::new(model_id, shape, tokens, &mask, GateHint::allow_all());
            backend.infer(req).unwrap().logits_q
        })
        .collect();

    c.bench_function("golden_vector_validation", |b| {
        b.iter(|| {
            for (tokens, exp) in test_inputs.iter().zip(&expected) {
                let req = InferenceRequest::new(
                    model_id,
                    shape,
                    black_box(tokens),
                    &mask,
                    GateHint::allow_all(),
                );
                let result = backend.infer(req).unwrap();

                // Compute max abs error
                let max_err: i32 = result
                    .logits_q
                    .iter()
                    .zip(exp)
                    .map(|(&a, &b)| (a as i32 - b as i32).abs())
                    .max()
                    .unwrap_or(0);

                assert_eq!(max_err, 0, "Golden vector mismatch");
            }
        })
    });
}

fn bench_quantization_accuracy(c: &mut Criterion) {
    use ruvector_fpga_transformer::quant::qformat::{quantize_symmetric_i8, QuantizedMatrix};

    c.bench_function("quantize_matrix_256x256", |b| {
        let data: Vec<f32> = (0..256 * 256).map(|i| (i as f32 * 0.001).sin()).collect();

        b.iter(|| {
            let matrix = QuantizedMatrix::from_f32(black_box(&data), 256, 256);
            let dequant = matrix.to_f32();

            // Check reconstruction error
            let max_err: f32 = data
                .iter()
                .zip(&dequant)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f32, f32::max);

            assert!(max_err < 0.1, "Quantization error too high: {}", max_err);
        })
    });
}

criterion_group!(
    benches,
    bench_determinism,
    bench_golden_vectors,
    bench_quantization_accuracy
);
criterion_main!(benches);
