// Plaid ZK Proof & Learning Performance Benchmarks
//
// Run with: cargo bench --bench plaid_performance
//
// Expected results:
// - Proof generation: ~8μs per proof (32-bit range)
// - Transaction processing: ~1.5μs per transaction
// - Feature extraction: ~0.1μs
// - LSH hashing: ~0.05μs

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use ruvector_edge::plaid::*;
use ruvector_edge::plaid::zkproofs::{RangeProof, PedersenCommitment, FinancialProofBuilder};
use std::collections::HashMap;

// ============================================================================
// Proof Generation Benchmarks
// ============================================================================

fn bench_proof_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("proof_generation");

    // Test different range sizes (affects bit count and proof complexity)
    for range_bits in [8, 16, 32, 64] {
        let max = if range_bits == 64 {
            u64::MAX / 2  // Avoid overflow
        } else {
            (1u64 << range_bits) - 1
        };
        let value = max / 2;
        let blinding = PedersenCommitment::random_blinding();

        group.throughput(Throughput::Elements(1));

        group.bench_with_input(
            BenchmarkId::new("range_proof", range_bits),
            &(value, max, blinding),
            |b, (v, m, bl)| {
                b.iter(|| {
                    RangeProof::prove(
                        black_box(*v),
                        0,
                        black_box(*m),
                        bl,
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_proof_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("proof_verification");

    // Pre-generate proofs of different sizes
    let proofs: Vec<_> = [8, 16, 32, 64]
        .iter()
        .map(|&bits| {
            let max = if bits == 64 {
                u64::MAX / 2
            } else {
                (1u64 << bits) - 1
            };
            let value = max / 2;
            let blinding = PedersenCommitment::random_blinding();
            (bits, RangeProof::prove(value, 0, max, &blinding).unwrap())
        })
        .collect();

    for (bits, proof) in &proofs {
        group.throughput(Throughput::Elements(1));

        group.bench_with_input(
            BenchmarkId::new("verify", bits),
            proof,
            |b, p| {
                b.iter(|| RangeProof::verify(black_box(p)));
            },
        );
    }

    group.finish();
}

fn bench_pedersen_commitment(c: &mut Criterion) {
    let mut group = c.benchmark_group("pedersen_commitment");

    let value = 50000u64;
    let blinding = PedersenCommitment::random_blinding();

    group.bench_function("commit", |b| {
        b.iter(|| {
            PedersenCommitment::commit(black_box(value), black_box(&blinding))
        });
    });

    group.bench_function("verify_opening", |b| {
        let commitment = PedersenCommitment::commit(value, &blinding);
        b.iter(|| {
            PedersenCommitment::verify_opening(
                black_box(&commitment),
                black_box(value),
                black_box(&blinding),
            )
        });
    });

    group.finish();
}

fn bench_financial_proofs(c: &mut Criterion) {
    let mut group = c.benchmark_group("financial_proofs");

    let builder = FinancialProofBuilder::new()
        .with_income(vec![6500, 6500, 6800, 6500])
        .with_balances(vec![5000, 5200, 4800, 5100, 5300, 5000, 5500]);

    group.bench_function("prove_income_above", |b| {
        b.iter(|| {
            builder.prove_income_above(black_box(5000))
        });
    });

    group.bench_function("prove_affordability", |b| {
        b.iter(|| {
            builder.prove_affordability(black_box(2000), black_box(3))
        });
    });

    group.bench_function("prove_no_overdrafts", |b| {
        b.iter(|| {
            builder.prove_no_overdrafts(black_box(30))
        });
    });

    group.bench_function("prove_savings_above", |b| {
        b.iter(|| {
            builder.prove_savings_above(black_box(4000))
        });
    });

    group.finish();
}

// ============================================================================
// Learning Algorithm Benchmarks
// ============================================================================

fn bench_feature_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("feature_extraction");

    let tx = Transaction {
        transaction_id: "tx123".to_string(),
        account_id: "acc456".to_string(),
        amount: 50.0,
        date: "2024-03-15".to_string(),
        name: "Starbucks Coffee Shop".to_string(),
        merchant_name: Some("Starbucks".to_string()),
        category: vec!["Food".to_string(), "Coffee".to_string()],
        pending: false,
        payment_channel: "in_store".to_string(),
    };

    group.throughput(Throughput::Elements(1));

    group.bench_function("extract_features", |b| {
        b.iter(|| extract_features(black_box(&tx)));
    });

    group.bench_function("to_embedding", |b| {
        let features = extract_features(&tx);
        b.iter(|| features.to_embedding());
    });

    group.bench_function("full_pipeline", |b| {
        b.iter(|| {
            let features = extract_features(black_box(&tx));
            features.to_embedding()
        });
    });

    group.finish();
}

fn bench_lsh_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsh_hashing");

    let test_cases = vec![
        ("Short", "Starbucks"),
        ("Medium", "Amazon.com Services LLC"),
        ("Long", "Whole Foods Market Store #12345 Manhattan"),
        ("VeryLong", "Shell Gas Station #12345 - 123 Main Street, City Name, State 12345"),
    ];

    for (name, text) in &test_cases {
        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("simple_lsh", name),
            text,
            |b, t| {
                b.iter(|| {
                    // LSH is internal, so we extract features which calls it
                    let tx = Transaction {
                        transaction_id: "tx".to_string(),
                        account_id: "acc".to_string(),
                        amount: 50.0,
                        date: "2024-01-01".to_string(),
                        name: t.to_string(),
                        merchant_name: Some(t.to_string()),
                        category: vec!["Test".to_string()],
                        pending: false,
                        payment_channel: "online".to_string(),
                    };
                    extract_features(black_box(&tx))
                });
            },
        );
    }

    group.finish();
}

fn bench_q_learning(c: &mut Criterion) {
    let mut group = c.benchmark_group("q_learning");

    let mut state = FinancialLearningState::default();

    // Pre-populate with some Q-values
    for i in 0..100 {
        let key = format!("category_{}|under_budget", i % 10);
        state.q_values.insert(key, 0.5 + (i as f64 * 0.01));
    }

    group.bench_function("update_q_value", |b| {
        b.iter(|| {
            update_q_value(
                black_box(&state),
                "Food",
                "under_budget",
                1.0,
                0.1,
            )
        });
    });

    group.bench_function("get_recommendation", |b| {
        b.iter(|| {
            get_recommendation(
                black_box(&state),
                "Food",
                500.0,
                600.0,
            )
        });
    });

    group.bench_function("q_value_lookup", |b| {
        b.iter(|| {
            black_box(&state).q_values.get("category_5|under_budget")
        });
    });

    group.finish();
}

// ============================================================================
// End-to-End Transaction Processing
// ============================================================================

fn bench_transaction_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("transaction_processing");

    // Test different batch sizes
    for batch_size in [1, 10, 100, 1000] {
        let transactions: Vec<Transaction> = (0..batch_size)
            .map(|i| Transaction {
                transaction_id: format!("tx{}", i),
                account_id: "acc456".to_string(),
                amount: 50.0 + (i as f64 % 100.0),
                date: format!("2024-03-{:02}", (i % 28) + 1),
                name: format!("Merchant {}", i % 20),
                merchant_name: Some(format!("Merchant {}", i % 20)),
                category: vec![
                    format!("Category {}", i % 5),
                    "Subcategory".to_string()
                ],
                pending: false,
                payment_channel: if i % 2 == 0 { "in_store" } else { "online" }.to_string(),
            })
            .collect();

        group.throughput(Throughput::Elements(batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("feature_extraction_batch", batch_size),
            &transactions,
            |b, txs| {
                b.iter(|| {
                    for tx in txs {
                        let _ = extract_features(black_box(tx));
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("full_pipeline_batch", batch_size),
            &transactions,
            |b, txs| {
                b.iter(|| {
                    for tx in txs {
                        let features = extract_features(black_box(tx));
                        let _ = features.to_embedding();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Serialization Benchmarks
// ============================================================================

fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");

    // Create states with varying sizes
    for tx_count in [100, 1000, 10000] {
        let mut state = FinancialLearningState::default();

        // Populate state to simulate real usage
        for i in 0..tx_count {
            let category_key = format!("category_{}", i % 10);
            let pattern = SpendingPattern {
                pattern_id: format!("pat_{}", i),
                category: category_key.clone(),
                avg_amount: 50.0 + (i as f64 % 100.0),
                frequency_days: 7.0,
                confidence: 0.8,
                last_seen: i,
            };
            state.patterns.insert(category_key.clone(), pattern);

            // Add Q-values
            let q_key = format!("{}|under_budget", category_key);
            state.q_values.insert(q_key, 0.5 + (i as f64 * 0.001));

            // Add embedding (this will expose the memory leak!)
            state.category_embeddings.push((
                category_key,
                vec![0.1 * (i as f32 % 10.0); 21]
            ));
        }

        state.version = tx_count;

        let json_string = serde_json::to_string(&state).unwrap();
        let state_size = json_string.len();

        group.throughput(Throughput::Bytes(state_size as u64));

        group.bench_with_input(
            BenchmarkId::new("json_serialize", tx_count),
            &state,
            |b, s| {
                b.iter(|| serde_json::to_string(black_box(s)).unwrap());
            },
        );

        group.bench_with_input(
            BenchmarkId::new("json_deserialize", tx_count),
            &json_string,
            |b, json| {
                b.iter(|| {
                    serde_json::from_str::<FinancialLearningState>(black_box(json)).unwrap()
                });
            },
        );

        // Benchmark bincode for comparison
        let bincode_data = bincode::serialize(&state).unwrap();

        group.bench_with_input(
            BenchmarkId::new("bincode_serialize", tx_count),
            &state,
            |b, s| {
                b.iter(|| bincode::serialize(black_box(s)).unwrap());
            },
        );

        group.bench_with_input(
            BenchmarkId::new("bincode_deserialize", tx_count),
            &bincode_data,
            |b, data| {
                b.iter(|| {
                    bincode::deserialize::<FinancialLearningState>(black_box(data)).unwrap()
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Memory Footprint Benchmarks
// ============================================================================

fn bench_memory_footprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_footprint");

    group.bench_function("proof_size_8bit", |b| {
        b.iter_custom(|iters| {
            let mut total_size = 0;
            let start = std::time::Instant::now();

            for _ in 0..iters {
                let blinding = PedersenCommitment::random_blinding();
                let proof = RangeProof::prove(128, 0, 255, &blinding).unwrap();
                let size = bincode::serialize(&proof).unwrap().len();
                total_size += size;
                black_box(size);
            }

            println!("Average proof size (8-bit): {} bytes", total_size / iters as usize);
            start.elapsed()
        });
    });

    group.bench_function("proof_size_32bit", |b| {
        b.iter_custom(|iters| {
            let mut total_size = 0;
            let start = std::time::Instant::now();

            for _ in 0..iters {
                let blinding = PedersenCommitment::random_blinding();
                let proof = RangeProof::prove(50000, 0, 100000, &blinding).unwrap();
                let size = bincode::serialize(&proof).unwrap().len();
                total_size += size;
                black_box(size);
            }

            println!("Average proof size (32-bit): {} bytes", total_size / iters as usize);
            start.elapsed()
        });
    });

    group.bench_function("state_growth_simulation", |b| {
        b.iter_custom(|iters| {
            let mut state = FinancialLearningState::default();
            let start = std::time::Instant::now();

            for i in 0..iters {
                // Simulate transaction processing (THIS WILL LEAK MEMORY!)
                let key = format!("cat_{}", i % 10);
                state.category_embeddings.push((key.clone(), vec![0.0; 21]));

                // Also add pattern and Q-value
                let pattern = SpendingPattern {
                    pattern_id: format!("pat_{}", i),
                    category: key.clone(),
                    avg_amount: 50.0,
                    frequency_days: 7.0,
                    confidence: 0.8,
                    last_seen: i,
                };
                state.patterns.insert(key.clone(), pattern);
                state.q_values.insert(format!("{}|action", key), 0.5);
            }

            let size = bincode::serialize(&state).unwrap().len();
            println!("State size after {} transactions: {} KB", iters, size / 1024);
            println!("Embeddings count: {}", state.category_embeddings.len());

            start.elapsed()
        });
    });

    group.finish();
}

// ============================================================================
// Regression Tests (detect performance degradation)
// ============================================================================

fn bench_regression_tests(c: &mut Criterion) {
    let mut group = c.benchmark_group("regression_tests");

    // These benchmarks establish baseline performance
    // CI can fail if they regress significantly

    group.bench_function("baseline_proof_32bit", |b| {
        let blinding = PedersenCommitment::random_blinding();
        b.iter(|| {
            RangeProof::prove(black_box(50000), 0, black_box(100000), &blinding)
        });
    });

    group.bench_function("baseline_feature_extraction", |b| {
        let tx = Transaction {
            transaction_id: "tx".to_string(),
            account_id: "acc".to_string(),
            amount: 50.0,
            date: "2024-01-01".to_string(),
            name: "Test".to_string(),
            merchant_name: Some("Test Merchant".to_string()),
            category: vec!["Food".to_string()],
            pending: false,
            payment_channel: "online".to_string(),
        };

        b.iter(|| {
            let features = extract_features(black_box(&tx));
            features.to_embedding()
        });
    });

    group.bench_function("baseline_json_serialize_1k", |b| {
        let mut state = FinancialLearningState::default();
        for i in 0..1000 {
            let key = format!("cat_{}", i % 10);
            state.category_embeddings.push((key, vec![0.0; 21]));
        }

        b.iter(|| {
            serde_json::to_string(black_box(&state))
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark Groups
// ============================================================================

criterion_group!(
    proof_benches,
    bench_proof_generation,
    bench_proof_verification,
    bench_pedersen_commitment,
    bench_financial_proofs,
);

criterion_group!(
    learning_benches,
    bench_feature_extraction,
    bench_lsh_hashing,
    bench_q_learning,
    bench_transaction_processing,
);

criterion_group!(
    overhead_benches,
    bench_serialization,
    bench_memory_footprint,
);

criterion_group!(
    regression_benches,
    bench_regression_tests,
);

criterion_main!(
    proof_benches,
    learning_benches,
    overhead_benches,
    regression_benches,
);
