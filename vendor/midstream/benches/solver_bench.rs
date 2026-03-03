//! Comprehensive benchmarks for temporal-neural-solver crate
//!
//! Benchmarks cover:
//! - LTL formula encoding (target: <10ms)
//! - Verification performance (target: <100ms)
//! - Formula parsing
//! - State checking
//! - Neural network inference
//! - Temporal logic operations
//!
//! Performance targets:
//! - Formula encoding: <10ms
//! - Verification: <100ms
//! - Parsing: <5ms

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use midstreamer_neural_solver::{
    TemporalSolver, LTLFormula, Formula, State, Trace,
    parser::parse_ltl,
    encoder::encode_formula,
    verifier::verify_trace,
    neural::NeuralVerifier,
};

// ============================================================================
// LTL Formula Generators
// ============================================================================

fn create_simple_formula() -> LTLFormula {
    // G (a -> F b)  (Globally: if a then eventually b)
    LTLFormula::Globally(Box::new(
        LTLFormula::Implies(
            Box::new(LTLFormula::Atom("a".to_string())),
            Box::new(LTLFormula::Finally(Box::new(
                LTLFormula::Atom("b".to_string())
            )))
        )
    ))
}

fn create_complex_formula() -> LTLFormula {
    // G ((a & b) -> X (c U d))
    LTLFormula::Globally(Box::new(
        LTLFormula::Implies(
            Box::new(LTLFormula::And(
                Box::new(LTLFormula::Atom("a".to_string())),
                Box::new(LTLFormula::Atom("b".to_string()))
            )),
            Box::new(LTLFormula::Next(Box::new(
                LTLFormula::Until(
                    Box::new(LTLFormula::Atom("c".to_string())),
                    Box::new(LTLFormula::Atom("d".to_string()))
                )
            )))
        )
    ))
}

fn create_nested_formula(depth: usize) -> LTLFormula {
    if depth == 0 {
        LTLFormula::Atom("p".to_string())
    } else {
        LTLFormula::Globally(Box::new(
            LTLFormula::Finally(Box::new(
                create_nested_formula(depth - 1)
            ))
        ))
    }
}

fn create_safety_property() -> LTLFormula {
    // G (request -> F grant)
    LTLFormula::Globally(Box::new(
        LTLFormula::Implies(
            Box::new(LTLFormula::Atom("request".to_string())),
            Box::new(LTLFormula::Finally(Box::new(
                LTLFormula::Atom("grant".to_string())
            )))
        )
    ))
}

fn create_liveness_property() -> LTLFormula {
    // G F ready
    LTLFormula::Globally(Box::new(
        LTLFormula::Finally(Box::new(
            LTLFormula::Atom("ready".to_string())
        ))
    ))
}

// ============================================================================
// Trace Generators
// ============================================================================

fn generate_simple_trace(len: usize) -> Trace {
    let mut trace = Trace::new();

    for i in 0..len {
        let mut state = State::new();
        state.set("a", i % 3 == 0);
        state.set("b", i % 5 == 0);
        trace.add_state(state);
    }

    trace
}

fn generate_complex_trace(len: usize) -> Trace {
    let mut trace = Trace::new();

    for i in 0..len {
        let mut state = State::new();
        state.set("a", i % 2 == 0);
        state.set("b", i % 3 == 0);
        state.set("c", i % 5 == 0);
        state.set("d", i % 7 == 0);
        trace.add_state(state);
    }

    trace
}

fn generate_satisfying_trace(len: usize) -> Trace {
    let mut trace = Trace::new();

    for i in 0..len {
        let mut state = State::new();
        state.set("request", i % 10 == 0);
        state.set("grant", i % 10 == 1 || i % 10 == 2);
        state.set("ready", true);
        trace.add_state(state);
    }

    trace
}

// ============================================================================
// Formula Encoding Benchmarks
// ============================================================================

fn bench_formula_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("formula_encoding");

    // Simple formulas
    group.bench_function("simple", |b| {
        let formula = create_simple_formula();
        b.iter(|| {
            black_box(encode_formula(black_box(&formula)))
        });
    });

    // Complex formulas
    group.bench_function("complex", |b| {
        let formula = create_complex_formula();
        b.iter(|| {
            black_box(encode_formula(black_box(&formula)))
        });
    });

    // Safety properties
    group.bench_function("safety", |b| {
        let formula = create_safety_property();
        b.iter(|| {
            black_box(encode_formula(black_box(&formula)))
        });
    });

    // Liveness properties
    group.bench_function("liveness", |b| {
        let formula = create_liveness_property();
        b.iter(|| {
            black_box(encode_formula(black_box(&formula)))
        });
    });

    // Nested formulas
    for depth in [1, 3, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::new("nested", depth),
            depth,
            |b, &d| {
                let formula = create_nested_formula(d);
                b.iter(|| {
                    black_box(encode_formula(black_box(&formula)))
                });
            }
        );
    }

    group.finish();
}

// ============================================================================
// Formula Parsing Benchmarks
// ============================================================================

fn bench_formula_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("formula_parsing");

    let test_cases = vec![
        ("simple", "G (a -> F b)"),
        ("complex", "G ((a & b) -> X (c U d))"),
        ("safety", "G (request -> F grant)"),
        ("liveness", "G F ready"),
        ("nested", "G F G F G F p"),
        ("boolean", "(a & b) | (c & d)"),
        ("temporal", "X X X X p"),
    ];

    for (name, formula_str) in test_cases {
        group.bench_function(name, |b| {
            b.iter(|| {
                black_box(parse_ltl(black_box(formula_str)))
            });
        });
    }

    group.finish();
}

// ============================================================================
// Verification Benchmarks
// ============================================================================

fn bench_trace_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("trace_verification");

    // Simple formula, varying trace lengths
    let simple_formula = create_simple_formula();
    for trace_len in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*trace_len as u64));
        group.bench_with_input(
            BenchmarkId::new("simple", trace_len),
            trace_len,
            |b, &len| {
                let trace = generate_simple_trace(len);
                b.iter(|| {
                    black_box(verify_trace(
                        black_box(&simple_formula),
                        black_box(&trace)
                    ))
                });
            }
        );
    }

    // Complex formula
    let complex_formula = create_complex_formula();
    for trace_len in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("complex", trace_len),
            trace_len,
            |b, &len| {
                let trace = generate_complex_trace(len);
                b.iter(|| {
                    black_box(verify_trace(
                        black_box(&complex_formula),
                        black_box(&trace)
                    ))
                });
            }
        );
    }

    group.finish();
}

fn bench_verification_outcomes(c: &mut Criterion) {
    let mut group = c.benchmark_group("verification_outcomes");

    let formula = create_safety_property();

    // Satisfying trace
    group.bench_function("satisfying", |b| {
        let trace = generate_satisfying_trace(100);
        b.iter(|| {
            black_box(verify_trace(
                black_box(&formula),
                black_box(&trace)
            ))
        });
    });

    // Violating trace (early termination)
    group.bench_function("violating_early", |b| {
        let mut trace = Trace::new();
        let mut state = State::new();
        state.set("request", true);
        state.set("grant", false);
        trace.add_state(state);

        for _ in 0..100 {
            let mut state = State::new();
            state.set("request", false);
            state.set("grant", false);
            trace.add_state(state);
        }

        b.iter(|| {
            black_box(verify_trace(
                black_box(&formula),
                black_box(&trace)
            ))
        });
    });

    group.finish();
}

// ============================================================================
// State Checking Benchmarks
// ============================================================================

fn bench_state_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_operations");

    // State creation
    group.bench_function("creation", |b| {
        b.iter(|| {
            let mut state = State::new();
            state.set("a", true);
            state.set("b", false);
            state.set("c", true);
            black_box(state)
        });
    });

    // State checking
    group.bench_function("checking", |b| {
        let mut state = State::new();
        state.set("a", true);
        state.set("b", false);
        state.set("c", true);

        b.iter(|| {
            black_box(state.get("a") && !state.get("b") && state.get("c"))
        });
    });

    // State comparison
    group.bench_function("comparison", |b| {
        let mut state1 = State::new();
        state1.set("a", true);
        state1.set("b", false);

        let mut state2 = State::new();
        state2.set("a", true);
        state2.set("b", false);

        b.iter(|| {
            black_box(state1 == state2)
        });
    });

    // Trace operations
    for num_states in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("trace_ops", num_states),
            num_states,
            |b, &n| {
                b.iter(|| {
                    let mut trace = Trace::new();
                    for i in 0..n {
                        let mut state = State::new();
                        state.set("var", i % 2 == 0);
                        trace.add_state(state);
                    }
                    black_box(trace)
                });
            }
        );
    }

    group.finish();
}

// ============================================================================
// Neural Verifier Benchmarks
// ============================================================================

fn bench_neural_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("neural_verification");

    // Neural encoding overhead
    group.bench_function("encoding_overhead", |b| {
        let formula = create_simple_formula();
        let trace = generate_simple_trace(100);
        let verifier = NeuralVerifier::new();

        b.iter(|| {
            black_box(verifier.encode_for_neural(
                black_box(&formula),
                black_box(&trace)
            ))
        });
    });

    // Inference time
    group.bench_function("inference", |b| {
        let formula = create_simple_formula();
        let trace = generate_simple_trace(100);
        let mut verifier = NeuralVerifier::new();
        verifier.train(&formula, &trace);

        b.iter(|| {
            black_box(verifier.verify(
                black_box(&formula),
                black_box(&trace)
            ))
        });
    });

    // Training overhead
    group.bench_function("training", |b| {
        let formula = create_simple_formula();
        let trace = generate_simple_trace(100);

        b.iter(|| {
            let mut verifier = NeuralVerifier::new();
            black_box(verifier.train(
                black_box(&formula),
                black_box(&trace)
            ))
        });
    });

    group.finish();
}

// ============================================================================
// Temporal Logic Operations Benchmarks
// ============================================================================

fn bench_temporal_operators(c: &mut Criterion) {
    let mut group = c.benchmark_group("temporal_operators");

    let trace = generate_complex_trace(100);

    // Next operator
    group.bench_function("next", |b| {
        let formula = LTLFormula::Next(Box::new(
            LTLFormula::Atom("a".to_string())
        ));
        b.iter(|| {
            black_box(verify_trace(black_box(&formula), black_box(&trace)))
        });
    });

    // Globally operator
    group.bench_function("globally", |b| {
        let formula = LTLFormula::Globally(Box::new(
            LTLFormula::Atom("a".to_string())
        ));
        b.iter(|| {
            black_box(verify_trace(black_box(&formula), black_box(&trace)))
        });
    });

    // Finally operator
    group.bench_function("finally", |b| {
        let formula = LTLFormula::Finally(Box::new(
            LTLFormula::Atom("d".to_string())
        ));
        b.iter(|| {
            black_box(verify_trace(black_box(&formula), black_box(&trace)))
        });
    });

    // Until operator
    group.bench_function("until", |b| {
        let formula = LTLFormula::Until(
            Box::new(LTLFormula::Atom("a".to_string())),
            Box::new(LTLFormula::Atom("d".to_string()))
        );
        b.iter(|| {
            black_box(verify_trace(black_box(&formula), black_box(&trace)))
        });
    });

    group.finish();
}

// ============================================================================
// Complete Pipeline Benchmarks
// ============================================================================

fn bench_complete_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("complete_pipeline");

    group.bench_function("parse_encode_verify", |b| {
        let formula_str = "G (request -> F grant)";
        let trace = generate_satisfying_trace(100);

        b.iter(|| {
            let formula = parse_ltl(formula_str).unwrap();
            let encoded = encode_formula(&formula);
            let result = verify_trace(&formula, &trace);
            black_box((encoded, result))
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group! {
    name = encoding_benches;
    config = Criterion::default()
        .sample_size(200)
        .measurement_time(std::time::Duration::from_secs(8))
        .warm_up_time(std::time::Duration::from_secs(3));
    targets = bench_formula_encoding
}

criterion_group! {
    name = parsing_benches;
    config = Criterion::default()
        .sample_size(500)
        .measurement_time(std::time::Duration::from_secs(5));
    targets = bench_formula_parsing
}

criterion_group! {
    name = verification_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(std::time::Duration::from_secs(12));
    targets = bench_trace_verification, bench_verification_outcomes
}

criterion_group! {
    name = state_benches;
    config = Criterion::default()
        .sample_size(500);
    targets = bench_state_operations
}

criterion_group! {
    name = neural_benches;
    config = Criterion::default()
        .sample_size(50)
        .measurement_time(std::time::Duration::from_secs(10));
    targets = bench_neural_verification
}

criterion_group! {
    name = operator_benches;
    config = Criterion::default()
        .sample_size(200);
    targets = bench_temporal_operators
}

criterion_group! {
    name = pipeline_benches;
    config = Criterion::default()
        .sample_size(100);
    targets = bench_complete_pipeline
}

criterion_main!(
    encoding_benches,
    parsing_benches,
    verification_benches,
    state_benches,
    neural_benches,
    operator_benches,
    pipeline_benches
);
