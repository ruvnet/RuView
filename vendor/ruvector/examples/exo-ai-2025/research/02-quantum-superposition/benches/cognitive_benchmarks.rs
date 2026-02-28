use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use num_complex::Complex64;
use quantum_cognition::{
    interference_pattern, tensor_product, AttentionOperator, CognitiveState,
    InterferenceDecisionMaker, SuperpositionBuilder,
};
use std::f64::consts::PI;

/// Benchmark: State creation and normalization
fn bench_state_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_creation");

    for &dim in [10, 50, 100, 500].iter() {
        group.bench_with_input(BenchmarkId::new("uniform", dim), &dim, |b, &dim| {
            b.iter(|| {
                let labels: Vec<String> = (0..dim).map(|i| format!("state_{}", i)).collect();
                CognitiveState::uniform(black_box(dim), labels)
            });
        });

        group.bench_with_input(BenchmarkId::new("builder", dim), &dim, |b, &dim| {
            b.iter(|| {
                let mut builder = SuperpositionBuilder::new();
                for i in 0..dim {
                    builder = builder.add_real(1.0 / (dim as f64).sqrt(), format!("state_{}", i));
                }
                builder.build()
            });
        });
    }

    group.finish();
}

/// Benchmark: Probability calculations (Born rule)
fn bench_probabilities(c: &mut Criterion) {
    let mut group = c.benchmark_group("probabilities");

    for &dim in [10, 50, 100, 500, 1000].iter() {
        let labels: Vec<String> = (0..dim).map(|i| format!("state_{}", i)).collect();
        let state = CognitiveState::uniform(dim, labels);

        group.bench_with_input(BenchmarkId::new("born_rule", dim), &state, |b, state| {
            b.iter(|| black_box(state.probabilities()));
        });

        group.bench_with_input(BenchmarkId::new("entropy", dim), &state, |b, state| {
            b.iter(|| black_box(state.von_neumann_entropy()));
        });
    }

    group.finish();
}

/// Benchmark: Inner products and fidelity
fn bench_inner_products(c: &mut Criterion) {
    let mut group = c.benchmark_group("inner_products");

    for &dim in [10, 50, 100, 500].iter() {
        let labels: Vec<String> = (0..dim).map(|i| format!("state_{}", i)).collect();
        let state1 = CognitiveState::uniform(dim, labels.clone());
        let state2 = CognitiveState::uniform(dim, labels);

        group.bench_with_input(
            BenchmarkId::new("inner_product", dim),
            &(state1.clone(), state2.clone()),
            |b, (s1, s2)| {
                b.iter(|| black_box(s1.inner_product(s2)));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("fidelity", dim),
            &(state1, state2),
            |b, (s1, s2)| {
                b.iter(|| black_box(s1.fidelity(s2)));
            },
        );
    }

    group.finish();
}

/// Benchmark: Measurement operations
fn bench_measurements(c: &mut Criterion) {
    let mut group = c.benchmark_group("measurements");

    for &dim in [10, 50, 100].iter() {
        let labels: Vec<String> = (0..dim).map(|i| format!("state_{}", i)).collect();
        let state = CognitiveState::uniform(dim, labels);

        group.bench_with_input(BenchmarkId::new("projective", dim), &state, |b, state| {
            b.iter(|| black_box(state.measure()));
        });

        let observable: Vec<f64> = (0..*dim).map(|i| (i as f64) / (*dim as f64)).collect();
        group.bench_with_input(
            BenchmarkId::new("weak", dim),
            &(state, observable),
            |b, (state, obs)| {
                b.iter(|| black_box(state.weak_measure(obs, 0.5)));
            },
        );
    }

    group.finish();
}

/// Benchmark: Tensor products (composite systems)
fn bench_tensor_products(c: &mut Criterion) {
    let mut group = c.benchmark_group("tensor_products");

    for dim in [5, 10, 20].iter() {
        let labels: Vec<String> = (0..dim).map(|i| format!("state_{}", i)).collect();
        let state1 = CognitiveState::uniform(*dim, labels.clone());
        let state2 = CognitiveState::uniform(*dim, labels);

        group.bench_with_input(
            BenchmarkId::new("product", dim),
            &(state1, state2),
            |b, (s1, s2)| {
                b.iter(|| black_box(tensor_product(s1, s2)));
            },
        );
    }

    group.finish();
}

/// Benchmark: Interference decision making
fn bench_interference_decisions(c: &mut Criterion) {
    let mut group = c.benchmark_group("interference_decisions");

    // Two-alternative choice
    let labels = vec!["option_A".to_string(), "option_B".to_string()];
    let state = CognitiveState::uniform(2, labels);

    group.bench_function("two_alternative", |b| {
        b.iter(|| {
            let mut dm = InterferenceDecisionMaker::new(state.clone());
            black_box(dm.two_alternative_choice("option_A", "option_B", PI / 4.0))
        });
    });

    // Multi-alternative choice
    for n_options in [3, 5, 10].iter() {
        let options: Vec<String> = (0..*n_options).map(|i| format!("option_{}", i)).collect();
        let state = CognitiveState::uniform(*n_options, options.clone());
        let phases: Vec<f64> = (0..*n_options)
            .map(|i| (i as f64) * 2.0 * PI / (*n_options as f64))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("multi_alternative", n_options),
            &(state, options, phases),
            |b, (state, opts, ph)| {
                b.iter(|| {
                    let mut dm = InterferenceDecisionMaker::new(state.clone());
                    black_box(dm.multi_alternative_choice(opts.clone(), ph.clone()))
                });
            },
        );
    }

    // Conjunction decision (Linda problem)
    group.bench_function("conjunction_fallacy", |b| {
        let labels = vec![
            "bank_teller".to_string(),
            "feminist".to_string(),
            "feminist_bank_teller".to_string(),
        ];
        let state = CognitiveState::uniform(3, labels);

        b.iter(|| {
            let mut dm = InterferenceDecisionMaker::new(state.clone());
            black_box(dm.conjunction_decision(
                "bank_teller",
                "feminist",
                "feminist_bank_teller",
                0.8,
            ))
        });
    });

    // Prisoner's dilemma
    for entanglement in [0.3, 0.6, 0.9].iter() {
        group.bench_with_input(
            BenchmarkId::new("prisoners_dilemma", format!("{:.1}", entanglement)),
            entanglement,
            |b, &ent| {
                let labels = vec![
                    "CC".to_string(),
                    "DD".to_string(),
                    "CD".to_string(),
                    "DC".to_string(),
                ];
                let state = CognitiveState::uniform(4, labels);

                b.iter(|| {
                    let mut dm = InterferenceDecisionMaker::new(state.clone());
                    black_box(dm.quantum_prisoners_dilemma("cooperate", ent))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Interference patterns
fn bench_interference_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("interference_patterns");

    for n_points in [50, 100, 500, 1000].iter() {
        let phases: Vec<f64> = (0..*n_points)
            .map(|i| (i as f64) * 2.0 * PI / (*n_points as f64))
            .collect();

        group.bench_with_input(BenchmarkId::new("pattern", n_points), &phases, |b, ph| {
            b.iter(|| black_box(interference_pattern(ph.clone())));
        });
    }

    group.finish();
}

/// Benchmark: Attention operations
fn bench_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("attention");

    for dim in [5, 10, 20, 50].iter() {
        let labels: Vec<String> = (0..dim).map(|i| format!("concept_{}", i)).collect();
        let state = CognitiveState::uniform(*dim, labels);

        // Full attention (projective measurement)
        group.bench_with_input(
            BenchmarkId::new("full_attention", dim),
            &state,
            |b, state| {
                let mut attention = AttentionOperator::full_attention(0, *dim, 10.0);
                b.iter(|| black_box(attention.apply(state)));
            },
        );

        // Distributed attention (weak measurement)
        let weights: Vec<f64> = (0..*dim).map(|i| 1.0 / (1.0 + (i as f64))).collect();
        group.bench_with_input(
            BenchmarkId::new("distributed_attention", dim),
            &(state, weights),
            |b, (state, w)| {
                let mut attention = AttentionOperator::distributed_attention(w.clone(), 0.3, 10.0);
                b.iter(|| black_box(attention.apply(state)));
            },
        );
    }

    group.finish();
}

/// Benchmark: Continuous evolution with attention
fn bench_continuous_evolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("continuous_evolution");

    let labels: Vec<String> = (0..10).map(|i| format!("concept_{}", i)).collect();
    let state = CognitiveState::uniform(10, labels);

    for time_steps in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("evolution", time_steps),
            time_steps,
            |b, &steps| {
                b.iter(|| {
                    let mut attention = AttentionOperator::full_attention(0, 10, 5.0);
                    black_box(attention.continuous_evolution(&state, 1.0, steps))
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_state_creation,
    bench_probabilities,
    bench_inner_products,
    bench_measurements,
    bench_tensor_products,
    bench_interference_decisions,
    bench_interference_patterns,
    bench_attention,
    bench_continuous_evolution,
);

criterion_main!(benches);
