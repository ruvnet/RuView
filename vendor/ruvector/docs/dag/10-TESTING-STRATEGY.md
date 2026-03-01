# Testing Strategy

## Overview

Comprehensive testing strategy for the Neural DAG Learning system, ensuring correctness, performance, and reliability across all components.

## Testing Layers

```
┌─────────────────────────────────────────────────────────────┐
│                    End-to-End Tests                         │
│              (Full system integration)                      │
├─────────────────────────────────────────────────────────────┤
│                  Integration Tests                          │
│           (Component interaction testing)                   │
├─────────────────────────────────────────────────────────────┤
│                     Unit Tests                              │
│             (Individual function/module)                    │
├─────────────────────────────────────────────────────────────┤
│                   Property Tests                            │
│            (Invariant verification)                         │
├─────────────────────────────────────────────────────────────┤
│                  Benchmark Tests                            │
│             (Performance validation)                        │
└─────────────────────────────────────────────────────────────┘
```

## Test Categories

### 1. Unit Tests

#### DAG Attention Mechanisms

```rust
// tests/unit/attention/mod.rs

#[cfg(test)]
mod topological_attention_tests {
    use super::*;
    use ruvector_dag::attention::TopologicalAttention;

    #[test]
    fn test_topological_sort_simple_dag() {
        let mut dag = QueryDag::new();
        dag.add_node(0, OperatorNode::seq_scan("users"));
        dag.add_node(1, OperatorNode::index_scan("idx_users_email"));
        dag.add_node(2, OperatorNode::hash_join());
        dag.add_edge(0, 2);
        dag.add_edge(1, 2);

        let attention = TopologicalAttention::new(TopologicalConfig::default());
        let scores = attention.forward(&dag).unwrap();

        // Root nodes should have highest attention
        assert!(scores[&2] > scores[&0]);
        assert!(scores[&2] > scores[&1]);
    }

    #[test]
    fn test_topological_attention_decay() {
        let config = TopologicalConfig {
            decay_factor: 0.5,
            max_depth: 10,
        };
        let attention = TopologicalAttention::new(config);

        // Deep DAG test
        let dag = create_linear_dag(10);
        let scores = attention.forward(&dag).unwrap();

        // Verify decay: score[depth] ≈ base * decay^depth
        for depth in 1..10 {
            let ratio = scores[&depth] / scores[&(depth - 1)];
            assert!((ratio - 0.5).abs() < 0.01);
        }
    }

    #[test]
    fn test_topological_handles_cycles_gracefully() {
        // This should not happen in query DAGs, but test robustness
        let dag = create_dag_with_back_edge();
        let attention = TopologicalAttention::new(TopologicalConfig::default());

        let result = attention.forward(&dag);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AttentionError::CycleDetected));
    }
}

#[cfg(test)]
mod causal_cone_attention_tests {
    #[test]
    fn test_causal_cone_future_discount() {
        let config = CausalConeConfig {
            future_discount: 0.5,
            time_window_ms: 1000,
        };
        let attention = CausalConeAttention::new(config);

        let dag = create_temporal_dag();
        let scores = attention.forward(&dag).unwrap();

        // Future operators should be discounted
        assert!(scores[&"past_op"] > scores[&"future_op"]);
    }

    #[test]
    fn test_causal_cone_respects_time_window() {
        let config = CausalConeConfig {
            future_discount: 0.5,
            time_window_ms: 100,
        };
        let attention = CausalConeAttention::new(config);

        let dag = create_wide_time_range_dag();
        let scores = attention.forward(&dag).unwrap();

        // Operators outside time window should have near-zero attention
        assert!(scores[&"ancient_op"] < 0.01);
    }
}

#[cfg(test)]
mod mincut_attention_tests {
    #[test]
    fn test_mincut_identifies_bottleneck() {
        // Graph with clear bottleneck
        //     [A]     [B]
        //      \     /
        //       [C]       <- bottleneck
        //      /   \
        //    [D]   [E]
        let dag = create_bottleneck_dag();

        let attention = MinCutGatedAttention::new(MinCutConfig::default());
        let scores = attention.forward(&dag).unwrap();

        // Bottleneck node should have highest gating weight
        assert!(scores[&"C"].gate_value > 0.9);
    }

    #[test]
    fn test_mincut_parallel_paths() {
        // Graph with parallel paths (no single bottleneck)
        let dag = create_parallel_dag();

        let attention = MinCutGatedAttention::new(MinCutConfig::default());
        let scores = attention.forward(&dag).unwrap();

        // All paths should have similar gating
        let variance = compute_variance(&scores.values().map(|s| s.gate_value));
        assert!(variance < 0.1);
    }
}
```

#### SONA Learning Tests

```rust
// tests/unit/sona/mod.rs

#[cfg(test)]
mod micro_lora_tests {
    #[test]
    fn test_micro_lora_rank_constraint() {
        let config = MicroLoraConfig {
            rank: 2,
            alpha: 1.0,
        };
        let lora = MicroLora::new(config, 256);

        assert_eq!(lora.a_matrix.shape(), (256, 2));
        assert_eq!(lora.b_matrix.shape(), (2, 256));
    }

    #[test]
    fn test_micro_lora_adaptation_speed() {
        let lora = MicroLora::new(MicroLoraConfig::default(), 256);

        let start = Instant::now();
        for _ in 0..1000 {
            let gradient = random_gradient(256);
            lora.adapt(&gradient);
        }
        let elapsed = start.elapsed();

        // Should complete 1000 adaptations in < 100ms total
        assert!(elapsed < Duration::from_millis(100));
    }

    #[test]
    fn test_micro_lora_gradient_flow() {
        let lora = MicroLora::new(MicroLoraConfig::default(), 256);

        // Forward pass
        let input = random_vector(256);
        let output = lora.forward(&input);

        // Verify output is modified
        let diff: f32 = input.iter().zip(output.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(diff > 0.0);
    }
}

#[cfg(test)]
mod ewc_tests {
    #[test]
    fn test_ewc_prevents_forgetting() {
        let mut ewc = EwcPlusPlus::new(EwcConfig {
            lambda: 5000.0,
            decay: 0.99,
        });

        // Learn task A
        let task_a_params = train_on_task_a();
        ewc.consolidate(&task_a_params);

        // Learn task B
        let task_b_params = train_on_task_b_with_ewc(&ewc);

        // Verify task A performance is preserved
        let task_a_accuracy = evaluate_task_a(&task_b_params);
        assert!(task_a_accuracy > 0.8, "EWC should preserve task A performance");
    }

    #[test]
    fn test_fisher_information_computation() {
        let ewc = EwcPlusPlus::new(EwcConfig::default());

        let trajectories = generate_sample_trajectories(100);
        let fisher = ewc.compute_fisher(&trajectories);

        // Fisher diagonal should be non-negative
        assert!(fisher.iter().all(|&f| f >= 0.0));

        // Important parameters should have higher Fisher values
        let important_idx = 0; // Assume first param is important
        assert!(fisher[important_idx] > fisher.iter().sum::<f32>() / fisher.len() as f32);
    }
}

#[cfg(test)]
mod reasoning_bank_tests {
    #[test]
    fn test_pattern_clustering() {
        let mut bank = DagReasoningBank::new(ReasoningBankConfig {
            num_clusters: 10,
            pattern_dim: 256,
        });

        // Add patterns from different categories
        for _ in 0..100 {
            bank.store_pattern(random_pattern_category_a());
        }
        for _ in 0..100 {
            bank.store_pattern(random_pattern_category_b());
        }

        bank.recompute_clusters();

        // Patterns from same category should be in same cluster
        let pattern_a = random_pattern_category_a();
        let pattern_b = random_pattern_category_b();

        let cluster_a = bank.find_cluster(&pattern_a);
        let cluster_a2 = bank.find_cluster(&random_pattern_category_a());

        assert_eq!(cluster_a, cluster_a2, "Same category should cluster together");
        assert_ne!(cluster_a, bank.find_cluster(&pattern_b));
    }

    #[test]
    fn test_similarity_search_accuracy() {
        let mut bank = DagReasoningBank::new(ReasoningBankConfig::default());

        // Store known patterns
        let known_pattern = vec![1.0; 256];
        bank.store_pattern(DagPattern::new(known_pattern.clone(), 0.9));

        // Query with similar pattern
        let query = known_pattern.iter().map(|x| x + 0.01).collect();
        let results = bank.query_similar(&query, 1);

        assert!(!results.is_empty());
        assert!(results[0].similarity > 0.99);
    }
}
```

### 2. Integration Tests

#### PostgreSQL Integration

```rust
// tests/integration/postgres/mod.rs

#[tokio::test]
async fn test_dag_extension_lifecycle() {
    let pool = create_test_pool().await;

    // Create extension
    sqlx::query("CREATE EXTENSION IF NOT EXISTS ruvector_dag CASCADE")
        .execute(&pool)
        .await
        .expect("Extension creation failed");

    // Verify functions exist
    let result: (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM pg_proc WHERE proname = 'dag_set_enabled')"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(result.0);

    // Enable DAG learning
    sqlx::query("SELECT ruvector.dag_set_enabled(true)")
        .execute(&pool)
        .await
        .expect("Enable failed");

    // Verify enabled
    let config: (bool,) = sqlx::query_as(
        "SELECT enabled FROM ruvector.dag_config()"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(config.0);
}

#[tokio::test]
async fn test_query_analysis_flow() {
    let pool = setup_dag_extension().await;

    // Create test table with vectors
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS test_vectors (
            id SERIAL PRIMARY KEY,
            embedding vector(128),
            category TEXT
        )
    "#)
    .execute(&pool)
    .await
    .unwrap();

    // Insert test data
    for i in 0..1000 {
        sqlx::query(r#"
            INSERT INTO test_vectors (embedding, category)
            VALUES ($1::vector, $2)
        "#)
        .bind(format!("[{}]", (0..128).map(|_| rand::random::<f32>()).map(|x| x.to_string()).collect::<Vec<_>>().join(",")))
        .bind(format!("cat_{}", i % 10))
        .execute(&pool)
        .await
        .unwrap();
    }

    // Analyze query
    let analysis: Vec<DagAnalysisRow> = sqlx::query_as(r#"
        SELECT * FROM ruvector.dag_analyze_plan($1)
    "#)
    .bind("SELECT * FROM test_vectors WHERE embedding <-> '[0.1, 0.2, ...]' < 0.5 LIMIT 10")
    .fetch_all(&pool)
    .await
    .unwrap();

    assert!(!analysis.is_empty());
    assert!(analysis.iter().any(|r| r.operator_type == "SeqScan" || r.operator_type == "HnswScan"));
}

#[tokio::test]
async fn test_learning_trajectory_recording() {
    let pool = setup_dag_extension().await;

    // Execute queries to generate trajectories
    for _ in 0..10 {
        sqlx::query("SELECT * FROM test_vectors ORDER BY embedding <-> $1 LIMIT 5")
            .bind(random_vector_string(128))
            .execute(&pool)
            .await
            .unwrap();
    }

    // Wait for background learning
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check trajectories were recorded
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM ruvector.dag_trajectory_history()"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(count.0 >= 10);
}

#[tokio::test]
async fn test_attention_mechanism_switching() {
    let pool = setup_dag_extension().await;

    let mechanisms = ["topological", "causal_cone", "critical_path", "mincut_gated"];

    for mechanism in mechanisms {
        // Set mechanism
        sqlx::query("SELECT ruvector.dag_set_attention($1)")
            .bind(mechanism)
            .execute(&pool)
            .await
            .unwrap();

        // Execute query and verify it uses correct mechanism
        let result: (String,) = sqlx::query_as(
            "SELECT attention_mechanism FROM ruvector.dag_config()"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(result.0, mechanism);

        // Verify attention scores are computed
        let scores: Vec<AttentionScoreRow> = sqlx::query_as(r#"
            SELECT * FROM ruvector.dag_attention_scores(
                'SELECT * FROM test_vectors LIMIT 10',
                $1
            )
        "#)
        .bind(mechanism)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert!(!scores.is_empty());
        assert!(scores.iter().all(|s| s.attention_weight >= 0.0 && s.attention_weight <= 1.0));
    }
}
```

#### QuDAG Integration

```rust
// tests/integration/qudag/mod.rs

#[tokio::test]
async fn test_qudag_connection() {
    // Start mock QuDAG server
    let mock_server = MockQuDagServer::start().await;

    let pool = setup_dag_extension().await;

    // Connect to mock server
    let result: (bool, String) = sqlx::query_as(
        "SELECT connected, node_id FROM ruvector.qudag_connect($1)"
    )
    .bind(&mock_server.endpoint())
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(result.0);
    assert!(!result.1.is_empty());
}

#[tokio::test]
async fn test_pattern_proposal_flow() {
    let mock_server = MockQuDagServer::start().await;
    let pool = setup_dag_extension().await;

    // Connect
    sqlx::query("SELECT * FROM ruvector.qudag_connect($1)")
        .bind(&mock_server.endpoint())
        .execute(&pool)
        .await
        .unwrap();

    // Propose pattern
    let result: (String, String) = sqlx::query_as(r#"
        SELECT proposal_id, status
        FROM ruvector.qudag_propose_pattern($1::vector, $2::jsonb, 10.0)
    "#)
    .bind(random_vector_string(256))
    .bind(r#"{"source": "test", "quality": 0.9}"#)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(!result.0.is_empty());
    assert_eq!(result.1, "pending");

    // Simulate consensus
    mock_server.finalize_proposal(&result.0).await;

    // Check status
    let status: (String, bool) = sqlx::query_as(
        "SELECT status, finalized FROM ruvector.qudag_proposal_status($1)"
    )
    .bind(&result.0)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(status.1);
}

#[tokio::test]
async fn test_ml_kem_encryption() {
    let pool = setup_dag_extension().await;

    // Generate keypair
    let keypair: (Vec<u8>, String) = sqlx::query_as(
        "SELECT public_key, secret_key_id FROM ruvector.qudag_generate_kem_keypair()"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Encrypt
    let plaintext = b"secret pattern data";
    let encrypted: (Vec<u8>, Vec<u8>) = sqlx::query_as(
        "SELECT ciphertext, encapsulated_key FROM ruvector.qudag_encrypt($1, $2)"
    )
    .bind(plaintext.as_slice())
    .bind(&keypair.0)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Decrypt
    let decrypted: (Vec<u8>,) = sqlx::query_as(
        "SELECT ruvector.qudag_decrypt($1, $2, $3)"
    )
    .bind(&encrypted.0)
    .bind(&encrypted.1)
    .bind(&keypair.1)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(&decrypted.0, plaintext);
}
```

### 3. Property-Based Tests

```rust
// tests/property/mod.rs

use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn attention_scores_sum_to_one(
        nodes in prop::collection::vec(any::<OperatorNode>(), 1..100)
    ) {
        let dag = QueryDag::from_nodes(nodes);
        let attention = TopologicalAttention::new(TopologicalConfig::default());

        if let Ok(scores) = attention.forward(&dag) {
            let sum: f32 = scores.values().sum();
            prop_assert!((sum - 1.0).abs() < 0.001, "Scores should sum to 1.0, got {}", sum);
        }
    }

    #[test]
    fn mincut_capacity_is_positive(
        edges in prop::collection::vec((0usize..100, 0usize..100, 0.0f32..100.0), 1..500)
    ) {
        let mut engine = DagMinCutEngine::new();
        for (u, v, w) in edges {
            if u != v {
                engine.add_edge(u, v, w);
            }
        }

        if let Ok(cut) = engine.compute_mincut(0, 99) {
            prop_assert!(cut.capacity >= 0.0);
        }
    }

    #[test]
    fn ewc_loss_increases_with_deviation(
        original in prop::collection::vec(-1.0f32..1.0, 256),
        deviation in 0.0f32..1.0
    ) {
        let ewc = EwcPlusPlus::new(EwcConfig::default());
        ewc.consolidate(&original);

        let deviated: Vec<f32> = original.iter()
            .map(|x| x + deviation)
            .collect();

        let loss_original = ewc.penalty(&original);
        let loss_deviated = ewc.penalty(&deviated);

        prop_assert!(
            loss_deviated >= loss_original,
            "EWC loss should increase with deviation"
        );
    }

    #[test]
    fn pattern_similarity_is_symmetric(
        pattern_a in prop::collection::vec(-1.0f32..1.0, 256),
        pattern_b in prop::collection::vec(-1.0f32..1.0, 256)
    ) {
        let bank = DagReasoningBank::new(ReasoningBankConfig::default());

        let sim_ab = bank.compute_similarity(&pattern_a, &pattern_b);
        let sim_ba = bank.compute_similarity(&pattern_b, &pattern_a);

        prop_assert!(
            (sim_ab - sim_ba).abs() < 1e-6,
            "Similarity should be symmetric"
        );
    }

    #[test]
    fn trajectory_buffer_maintains_capacity(
        trajectories in prop::collection::vec(any::<DagTrajectory>(), 0..2000)
    ) {
        let buffer = DagTrajectoryBuffer::new(1000);

        for t in trajectories {
            buffer.push(t);
        }

        prop_assert!(
            buffer.len() <= 1000,
            "Buffer should not exceed capacity"
        );
    }
}
```

### 4. Benchmark Tests

```rust
// benches/dag_benchmarks.rs

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn attention_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("attention_mechanisms");

    for size in [10, 100, 500, 1000] {
        let dag = create_random_dag(size);

        group.bench_with_input(
            BenchmarkId::new("topological", size),
            &dag,
            |b, dag| {
                let attention = TopologicalAttention::new(TopologicalConfig::default());
                b.iter(|| attention.forward(dag))
            }
        );

        group.bench_with_input(
            BenchmarkId::new("causal_cone", size),
            &dag,
            |b, dag| {
                let attention = CausalConeAttention::new(CausalConeConfig::default());
                b.iter(|| attention.forward(dag))
            }
        );

        group.bench_with_input(
            BenchmarkId::new("critical_path", size),
            &dag,
            |b, dag| {
                let attention = CriticalPathAttention::new(CriticalPathConfig::default());
                b.iter(|| attention.forward(dag))
            }
        );

        group.bench_with_input(
            BenchmarkId::new("mincut_gated", size),
            &dag,
            |b, dag| {
                let attention = MinCutGatedAttention::new(MinCutConfig::default());
                b.iter(|| attention.forward(dag))
            }
        );
    }

    group.finish();
}

fn sona_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("sona_learning");

    group.bench_function("micro_lora_adapt", |b| {
        let lora = MicroLora::new(MicroLoraConfig::default(), 256);
        let gradient = random_gradient(256);
        b.iter(|| lora.adapt(&gradient))
    });

    group.bench_function("ewc_penalty_256", |b| {
        let ewc = EwcPlusPlus::new(EwcConfig::default());
        ewc.consolidate(&random_params(256));
        let params = random_params(256);
        b.iter(|| ewc.penalty(&params))
    });

    for pattern_count in [100, 1000, 10000] {
        let mut bank = DagReasoningBank::new(ReasoningBankConfig::default());
        for _ in 0..pattern_count {
            bank.store_pattern(random_pattern());
        }

        group.bench_with_input(
            BenchmarkId::new("pattern_search", pattern_count),
            &bank,
            |b, bank| {
                let query = random_pattern_vector();
                b.iter(|| bank.query_similar(&query, 5))
            }
        );
    }

    group.finish();
}

fn mincut_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("mincut_operations");

    for nodes in [100, 500, 1000, 5000] {
        let graph = create_random_graph(nodes, nodes * 3);

        group.bench_with_input(
            BenchmarkId::new("compute_mincut", nodes),
            &graph,
            |b, graph| {
                let engine = DagMinCutEngine::from_graph(graph);
                b.iter(|| engine.compute_mincut(0, nodes - 1))
            }
        );

        group.bench_with_input(
            BenchmarkId::new("dynamic_update", nodes),
            &graph,
            |b, graph| {
                let mut engine = DagMinCutEngine::from_graph(graph);
                engine.compute_mincut(0, nodes - 1).unwrap();
                b.iter(|| engine.update_edge(rand::random::<usize>() % nodes, rand::random::<usize>() % nodes, rand::random()))
            }
        );
    }

    group.finish();
}

fn postgres_benchmarks(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = rt.block_on(setup_benchmark_pool());

    let mut group = c.benchmark_group("postgres_operations");

    group.bench_function("dag_analyze_plan", |b| {
        b.to_async(&rt).iter(|| async {
            sqlx::query("SELECT * FROM ruvector.dag_analyze_plan($1)")
                .bind(BENCHMARK_QUERY)
                .execute(&pool)
                .await
        })
    });

    group.bench_function("dag_attention_scores", |b| {
        b.to_async(&rt).iter(|| async {
            sqlx::query("SELECT * FROM ruvector.dag_attention_scores($1, 'auto')")
                .bind(BENCHMARK_QUERY)
                .execute(&pool)
                .await
        })
    });

    group.bench_function("pattern_similarity_search", |b| {
        let query_vec = random_vector_string(256);
        b.to_async(&rt).iter(|| async {
            sqlx::query("SELECT * FROM ruvector.dag_query_patterns($1::vector, 10, 0.5)")
                .bind(&query_vec)
                .execute(&pool)
                .await
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    attention_benchmarks,
    sona_benchmarks,
    mincut_benchmarks,
    postgres_benchmarks
);
criterion_main!(benches);
```

### 5. Performance Targets

| Component | Metric | Target | Method |
|-----------|--------|--------|--------|
| TopologicalAttention | Latency (100 nodes) | < 50μs | Benchmark |
| CausalConeAttention | Latency (100 nodes) | < 100μs | Benchmark |
| CriticalPathAttention | Latency (100 nodes) | < 75μs | Benchmark |
| MinCutGatedAttention | Latency (100 nodes) | < 200μs | Benchmark |
| MicroLoRA | Adaptation | < 100μs | Benchmark |
| EWC++ | Penalty computation | < 10μs | Benchmark |
| Pattern search | 10K patterns | < 2ms | Benchmark |
| MinCut update | 5K nodes | O(n^0.12) amortized | Theoretical |
| Query analysis | End-to-end | < 5ms | Integration |
| Learning cycle | Full | < 100ms | Integration |

### 6. Continuous Integration

```yaml
# .github/workflows/dag-tests.yml

name: Neural DAG Tests

on:
  push:
    paths:
      - 'ruvector-dag/**'
      - 'ruvector-postgres/**'
  pull_request:
    paths:
      - 'ruvector-dag/**'
      - 'ruvector-postgres/**'

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run unit tests
        run: cargo test -p ruvector-dag --lib

  integration-tests:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_PASSWORD: test
        ports:
          - 5432:5432
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run integration tests
        run: cargo test -p ruvector-dag --test '*'
        env:
          DATABASE_URL: postgres://postgres:test@localhost/postgres

  property-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run property tests
        run: cargo test -p ruvector-dag --test property -- --test-threads=1
        env:
          PROPTEST_CASES: 10000

  benchmarks:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run benchmarks
        run: cargo bench -p ruvector-dag -- --noplot
      - name: Check performance regression
        run: |
          cargo bench -p ruvector-dag -- --noplot --save-baseline new
          cargo bench -p ruvector-dag -- --noplot --baseline main --load-baseline new

  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: llvm-tools-preview
      - uses: taiki-e/install-action@cargo-llvm-cov
      - name: Generate coverage
        run: cargo llvm-cov -p ruvector-dag --lcov --output-path lcov.info
      - uses: codecov/codecov-action@v3
        with:
          files: lcov.info
```

### 7. Test Data Generation

```rust
// tests/fixtures/mod.rs

/// Generate realistic query DAGs for testing
pub fn generate_realistic_dag(complexity: DagComplexity) -> QueryDag {
    match complexity {
        DagComplexity::Simple => {
            // SELECT * FROM t WHERE x = 1
            let mut dag = QueryDag::new();
            dag.add_node(0, OperatorNode::seq_scan("t"));
            dag.add_node(1, OperatorNode::filter("x = 1"));
            dag.add_edge(0, 1);
            dag
        }
        DagComplexity::JoinQuery => {
            // SELECT * FROM a JOIN b ON a.id = b.aid
            let mut dag = QueryDag::new();
            dag.add_node(0, OperatorNode::seq_scan("a"));
            dag.add_node(1, OperatorNode::seq_scan("b"));
            dag.add_node(2, OperatorNode::hash_join());
            dag.add_edge(0, 2);
            dag.add_edge(1, 2);
            dag
        }
        DagComplexity::VectorSearch => {
            // Vector similarity search with join
            let mut dag = QueryDag::new();
            dag.add_node(0, OperatorNode::hnsw_scan("idx_vectors"));
            dag.add_node(1, OperatorNode::seq_scan("metadata"));
            dag.add_node(2, OperatorNode::nested_loop_join());
            dag.add_node(3, OperatorNode::sort("similarity"));
            dag.add_node(4, OperatorNode::limit(100));
            dag.add_edge(0, 2);
            dag.add_edge(1, 2);
            dag.add_edge(2, 3);
            dag.add_edge(3, 4);
            dag
        }
        DagComplexity::Complex => {
            // Multi-table join with aggregation
            generate_complex_dag(10, 20)
        }
    }
}

/// Generate patterns that simulate learned behavior
pub fn generate_learned_patterns(count: usize) -> Vec<DagPattern> {
    (0..count)
        .map(|i| {
            let category = i % 5;
            let base_vector = match category {
                0 => generate_scan_pattern_vector(),
                1 => generate_join_pattern_vector(),
                2 => generate_aggregate_pattern_vector(),
                3 => generate_sort_pattern_vector(),
                _ => generate_mixed_pattern_vector(),
            };

            DagPattern {
                vector: add_noise(&base_vector, 0.1),
                quality_score: 0.7 + (rand::random::<f32>() * 0.3),
                metadata: json!({
                    "category": category,
                    "source": "synthetic",
                    "created": chrono::Utc::now()
                }),
            }
        })
        .collect()
}
```

## Test Execution Commands

```bash
# Run all tests
cargo test -p ruvector-dag

# Run unit tests only
cargo test -p ruvector-dag --lib

# Run integration tests
cargo test -p ruvector-dag --test '*'

# Run property tests with more cases
PROPTEST_CASES=10000 cargo test -p ruvector-dag --test property

# Run benchmarks
cargo bench -p ruvector-dag

# Run with coverage
cargo llvm-cov -p ruvector-dag

# Run specific test
cargo test -p ruvector-dag test_topological_attention_decay

# Run tests with logging
RUST_LOG=debug cargo test -p ruvector-dag -- --nocapture
```

---

*Document: 10-TESTING-STRATEGY.md | Version: 1.0 | Last Updated: 2025-01-XX*
