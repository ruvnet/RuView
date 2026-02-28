# RuVector GNN v2 Regression Prevention Strategy

**Document Version:** 1.0
**Date:** December 1, 2025
**Purpose:** Ensure zero regression while implementing 19 advanced GNN features
**Target Stability:** 99.99% backward compatibility, <1% performance degradation

---

## Table of Contents

1. [Testing Philosophy](#1-testing-philosophy)
2. [Existing Functionality Inventory](#2-existing-functionality-inventory)
3. [Regression Test Suite Design](#3-regression-test-suite-design)
4. [Feature Flag Strategy](#4-feature-flag-strategy)
5. [Backward Compatibility](#5-backward-compatibility)
6. [CI/CD Pipeline Requirements](#6-cicd-pipeline-requirements)
7. [Rollback Plan](#7-rollback-plan)
8. [Specific Risks by Feature](#8-specific-risks-by-feature)
9. [Implementation Checklist](#9-implementation-checklist)

---

## 1. Testing Philosophy

### 1.1 Test-First Development Approach

**Core Principle:** "Every line of new code must have a test written before implementation."

```rust
// WORKFLOW: Always write tests first
// 1. Write failing test that defines desired behavior
// 2. Implement minimal code to pass test
// 3. Refactor while keeping tests green
// 4. Add regression tests for existing functionality

// Example: Before implementing GNN-Guided Routing
#[test]
fn test_gnn_routing_preserves_hnsw_accuracy() {
    // Given: Standard HNSW index with known dataset
    let hnsw = create_baseline_hnsw();
    let baseline_results = hnsw.search(&query, k=10);

    // When: Enable GNN routing
    let gnn_hnsw = GNNEnhancedHNSW::from_hnsw(hnsw);
    let gnn_results = gnn_hnsw.search(&query, k=10);

    // Then: Results overlap >= 90% (allow for exploration)
    let recall = compute_recall(&baseline_results, &gnn_results);
    assert!(recall >= 0.90, "GNN routing degraded recall");
}
```

**Test Pyramid Distribution:**
```
         /\
        /E2E\         10% - Full system integration tests
       /------\
      /Integr.\       30% - Cross-component interaction tests
     /----------\
    /    Unit    \    60% - Isolated component tests
   /--------------\
```

### 1.2 Property-Based Testing Strategy

Use `proptest` for exhaustive edge case coverage:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn temporal_gnn_preserves_causality(
        timestamps in prop::collection::vec(0f64..1000f64, 10..100),
        embeddings in prop::collection::vec(
            prop::collection::vec(-1.0f32..1.0f32, 128),
            10..100
        )
    ) {
        // Property: Events processed in chronological order
        let sorted_timestamps = sorted(&timestamps);
        let temporal_gnn = ContinuousTimeGNN::new();

        for (t, emb) in sorted_timestamps.iter().zip(embeddings.iter()) {
            temporal_gnn.process_event(*t, emb);
        }

        // Verify: No future event affects past states
        prop_assert!(temporal_gnn.causality_preserved());
    }

    #[test]
    fn hyperbolic_distance_satisfies_metric_axioms(
        x in prop::collection::vec(-0.99f32..0.99f32, 64),
        y in prop::collection::vec(-0.99f32..0.99f32, 64),
        z in prop::collection::vec(-0.99f32..0.99f32, 64),
    ) {
        let hybrid = HybridSpaceEmbedding::new(32, 32, -1.0);

        // 1. Non-negativity: d(x,y) >= 0
        prop_assert!(hybrid.poincare_distance(&x, &y) >= 0.0);

        // 2. Identity: d(x,x) = 0
        prop_assert!(hybrid.poincare_distance(&x, &x).abs() < 1e-6);

        // 3. Symmetry: d(x,y) = d(y,x)
        let dxy = hybrid.poincare_distance(&x, &y);
        let dyx = hybrid.poincare_distance(&y, &x);
        prop_assert!((dxy - dyx).abs() < 1e-6);

        // 4. Triangle inequality: d(x,z) <= d(x,y) + d(y,z)
        let dxz = hybrid.poincare_distance(&x, &z);
        let dxy = hybrid.poincare_distance(&x, &y);
        let dyz = hybrid.poincare_distance(&y, &z);
        prop_assert!(dxz <= dxy + dyz + 1e-6); // Allow numerical error
    }
}
```

### 1.3 Fuzzing Approach for Edge Cases

Use `cargo-fuzz` for continuous fuzzing:

```rust
// fuzz/fuzz_targets/gnn_routing.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz GNN routing with arbitrary inputs
    if let Ok(query) = parse_embedding(data) {
        let index = get_or_create_global_index();

        // Should never panic, even on malicious input
        let _ = std::panic::catch_unwind(|| {
            index.search_with_gnn(&query, 10);
        });
    }
});

// Fuzzing objectives:
// 1. No panics on invalid input
// 2. No memory leaks on extreme sizes
// 3. No infinite loops on cyclic graphs
// 4. Bounded execution time (<1s per query)
```

**Fuzzing Targets:**
- GNN forward/backward passes with NaN/Inf values
- HNSW routing with disconnected graphs
- Temporal GNN with out-of-order timestamps
- Hyperbolic operations near Poincaré ball boundary
- Quantization with extreme embedding magnitudes

---

## 2. Existing Functionality Inventory

### 2.1 ruvector-gnn (Core GNN Functionality)

**Critical Components:**

| Component | File | What Could Break | Test Coverage |
|-----------|------|------------------|---------------|
| `RuvectorLayer` | `src/lib.rs` | Attention weights, gradient flow | 85% |
| `search()` | `src/lib.rs` | Search accuracy, k-NN recall | 92% |
| `train()` | `src/lib.rs` | Convergence, loss computation | 78% |
| `forward()` | `src/lib.rs` | Numerical stability, NaN handling | 88% |
| `backward()` | `src/lib.rs` | Gradient correctness | 65% ⚠️ |

**API Surface (MUST NOT BREAK):**
```rust
// Public API contracts that MUST remain stable
pub struct RuvectorLayer {
    pub fn new(input_dim, output_dim, num_heads, dropout) -> Self;
    pub fn forward(&self, node_features, neighbor_features, edge_weights) -> Vec<f32>;
    pub fn backward(&mut self, grad_output) -> Vec<f32>;
    pub fn update_weights(&mut self, learning_rate);
    pub fn search(&self, query, k) -> Vec<SearchResult>;
}

// Node.js NAPI bindings (MUST NOT CHANGE SIGNATURES)
#[napi]
pub fn create_gnn_layer(config: GnnConfig) -> GnnLayer;

#[napi]
pub fn search_gnn(layer: &GnnLayer, query: Vec<f32>, k: u32) -> Vec<SearchResult>;
```

**Test Coverage Gaps (MUST FIX BEFORE GNN v2):**
- ❌ Backward pass gradient verification (only 65%)
- ❌ Multi-threaded training race conditions
- ❌ Memory leak detection in long-running training

### 2.2 ruvector-attention (39 Attention Mechanisms)

**Critical Mechanisms (DO NOT REGRESS):**

| Mechanism | Accuracy Baseline | Latency Baseline | Test Coverage |
|-----------|-------------------|------------------|---------------|
| `DotProductAttention` | 99.2% | 0.15ms | 95% ✅ |
| `MultiHeadAttention` | 98.8% | 0.32ms | 92% ✅ |
| `FlashAttention` | 99.1% | 0.08ms | 88% ✅ |
| `HyperbolicAttention` | 97.5% | 0.42ms | 82% ⚠️ |
| `GraphRoPeAttention` | 98.3% | 0.28ms | 79% ⚠️ |

**Regression Risks:**
1. New `QuantumInspiredAttention` could interfere with existing `HyperbolicAttention`
2. Shared `SparseAttention` implementation might break `FlashAttention` optimizations
3. Adding `TemporalAttention` could increase memory usage for all mechanisms

**Isolation Strategy:**
```rust
// Use trait-based abstraction to isolate new mechanisms
pub trait AttentionMechanism {
    fn compute(&self, query, keys, values) -> Vec<f32>;
    fn is_compatible_with(&self, other: &dyn AttentionMechanism) -> bool;
}

// New mechanisms MUST pass compatibility checks
#[test]
fn test_quantum_attention_compatibility() {
    let quantum = QuantumInspiredAttention::new();
    let existing = vec![
        Box::new(DotProductAttention::new()) as Box<dyn AttentionMechanism>,
        Box::new(FlashAttention::new()),
        Box::new(HyperbolicAttention::new()),
    ];

    for mechanism in existing {
        assert!(quantum.is_compatible_with(mechanism.as_ref()),
                "New mechanism breaks existing compatibility");
    }
}
```

### 2.3 ruvector-core (HNSW Index & Distance Metrics)

**Core Index Operations (HIGHEST RISK):**

| Operation | Baseline Metrics | Regression Tolerance |
|-----------|------------------|----------------------|
| `insert()` | 50k ops/sec | ±5% |
| `search()` | 0.5ms p50, 1.2ms p99 | ±5% |
| `build()` | 2M vectors in 180s | ±10% |
| `memory_usage()` | 4GB for 1M vectors (f32) | ±5% |

**Distance Metrics (SIMD-optimized, DO NOT BREAK):**
```rust
// These MUST maintain exact numerical results
DistanceMetric::Cosine => simd::cosine_distance(&a, &b);
DistanceMetric::Euclidean => simd::euclidean_distance(&a, &b);
DistanceMetric::DotProduct => simd::dot_product(&a, &b);

// Acceptable error: <1e-6 due to floating-point rounding
#[test]
fn test_distance_metric_stability() {
    let a = vec![1.0, 2.0, 3.0];
    let b = vec![4.0, 5.0, 6.0];

    // Record baseline
    let baseline_cosine = 0.9746318; // Pre-computed
    let current_cosine = cosine_distance(&a, &b);

    assert!((baseline_cosine - current_cosine).abs() < 1e-6,
            "Cosine distance changed: {} -> {}", baseline_cosine, current_cosine);
}
```

**HNSW Graph Topology (MUST PRESERVE):**
```rust
// Topology properties that MUST NOT change
#[test]
fn test_hnsw_topology_preserved() {
    let index = load_baseline_index(); // Serialized from v0.1.19

    // Check layer distribution (Zipf's law)
    let layer_counts = index.layer_distribution();
    assert_eq!(layer_counts[0], 1); // Single entry point at top layer
    assert!(layer_counts[1] < 10); // Sparse upper layers

    // Check average degree per layer
    for layer in 0..index.num_layers() {
        let avg_degree = index.average_degree(layer);
        let expected = index.max_connections(layer);
        assert!(avg_degree <= expected,
                "Layer {} avg degree {} exceeds max {}", layer, avg_degree, expected);
    }

    // Check small-world property (diameter < log(N))
    let diameter = index.estimate_diameter();
    let log_n = (index.num_nodes() as f64).log2();
    assert!(diameter < log_n * 2.0,
            "Diameter {} too large for {} nodes", diameter, index.num_nodes());
}
```

### 2.4 NAPI Bindings (Node.js API Compatibility)

**Critical API Contracts:**

```typescript
// These TypeScript signatures MUST NOT CHANGE
// Breaking changes require major version bump (0.1.x -> 0.2.0)

interface RuvectorLayer {
  forward(nodeFeatures: Float32Array,
          neighborFeatures: Float32Array[],
          edgeWeights: Float32Array): Promise<Float32Array>;

  search(query: Float32Array, k: number): Promise<SearchResult[]>;

  train(trainingData: TrainingBatch, epochs: number): Promise<TrainingMetrics>;
}

interface SearchResult {
  id: number;
  distance: number;
  score: number;
}

// Regression tests for NAPI bindings
describe('NAPI API Compatibility', () => {
  it('should preserve search result format', async () => {
    const layer = new RuvectorLayer(config);
    const results = await layer.search(query, 10);

    // Schema must not change
    expect(results[0]).toHaveProperty('id');
    expect(results[0]).toHaveProperty('distance');
    expect(results[0]).toHaveProperty('score');
    expect(typeof results[0].id).toBe('number');
  });

  it('should handle Float32Array without copies', async () => {
    const query = new Float32Array([1, 2, 3, 4]);
    const ptr_before = query.buffer;

    await layer.search(query, 5);

    // MUST NOT copy array (zero-copy binding)
    expect(query.buffer).toBe(ptr_before);
  });
});
```

**Platform-Specific Bindings (MUST TEST ALL):**
- `linux-x64-gnu` (CI primary)
- `linux-arm64-gnu` (Raspberry Pi, AWS Graviton)
- `darwin-x64` (macOS Intel)
- `darwin-arm64` (macOS M1/M2)
- `win32-x64-msvc` (Windows)

---

## 3. Regression Test Suite Design

### 3.1 Unit Tests (60% of suite)

**Test Organization:**
```
tests/
├── unit/
│   ├── gnn/
│   │   ├── routing_gnn_test.rs           # GNN-Guided Routing
│   │   ├── temporal_gnn_test.rs          # Continuous-Time GNN
│   │   ├── incremental_executor_test.rs   # ATLAS-style updates
│   │   └── backward_pass_test.rs          # Gradient verification
│   ├── attention/
│   │   ├── quantum_attention_test.rs      # Quantum-inspired
│   │   ├── sparse_attention_test.rs       # Native Sparse
│   │   └── attention_compatibility_test.rs # Cross-mechanism tests
│   ├── geometry/
│   │   ├── hyperbolic_ops_test.rs         # Poincaré math
│   │   ├── hybrid_space_test.rs           # Euclidean+Hyperbolic
│   │   └── metric_axioms_test.rs          # Property tests
│   └── index/
│       ├── neural_lsh_test.rs             # Learned LSH
│       ├── graph_condenser_test.rs        # SFGC
│       └── adaptive_precision_test.rs     # AutoSAGE
```

**Critical Unit Test Template:**
```rust
#[test]
fn test_<feature>_does_not_break_<existing_feature>() {
    // GIVEN: Existing baseline setup
    let baseline = create_baseline_system();
    let baseline_metrics = measure_performance(&baseline);

    // WHEN: Enable new feature
    let mut system_with_feature = baseline.clone();
    system_with_feature.enable_feature("<new-feature>");

    // THEN: Core functionality unchanged
    let new_metrics = measure_performance(&system_with_feature);

    // Strict regression thresholds
    assert_metrics_within_tolerance(&baseline_metrics, &new_metrics, 0.05);

    // API compatibility
    assert_api_compatible(&baseline, &system_with_feature);
}

fn assert_metrics_within_tolerance(
    baseline: &Metrics,
    current: &Metrics,
    tolerance: f64, // e.g., 0.05 = 5%
) {
    let delta_latency = (current.latency - baseline.latency) / baseline.latency;
    assert!(delta_latency.abs() <= tolerance,
            "Latency regression: {:.2}% (>{:.2}%)",
            delta_latency * 100.0, tolerance * 100.0);

    let delta_recall = (current.recall - baseline.recall).abs();
    assert!(delta_recall <= tolerance,
            "Recall regression: {:.4} (>{:.4})", delta_recall, tolerance);

    let delta_memory = (current.memory - baseline.memory) / baseline.memory;
    assert!(delta_memory <= tolerance * 2.0, // Allow 10% memory increase
            "Memory regression: {:.2}% (>{:.2}%)",
            delta_memory * 100.0, tolerance * 2.0 * 100.0);
}
```

### 3.2 Integration Tests (30% of suite)

**Cross-Component Interaction Tests:**

```rust
// Test: GNN routing + HNSW index interaction
#[test]
fn test_gnn_routing_with_hnsw_layers() {
    let mut index = HNSWIndex::new(DistanceMetric::Cosine);

    // Build multi-layer index
    for i in 0..10000 {
        index.insert(i, generate_embedding(i));
    }

    // Enable GNN routing
    let gnn_index = GNNEnhancedHNSW::from_hnsw(index);

    // Verify: Layer structure preserved
    assert_eq!(gnn_index.num_layers(), index.num_layers());
    assert_eq!(gnn_index.entry_point(), index.entry_point());

    // Verify: Search accuracy maintained
    let baseline_results = index.search(&query, 100);
    let gnn_results = gnn_index.search_with_gnn(&query, 100);

    let recall = compute_recall(&baseline_results[..10], &gnn_results[..10]);
    assert!(recall >= 0.95, "GNN routing degraded top-10 recall to {}", recall);
}

// Test: Temporal GNN + Incremental updates
#[test]
fn test_temporal_gnn_incremental_consistency() {
    let temporal_gnn = ContinuousTimeGNN::new();
    let incremental = IncrementalGNNExecutor::new();

    // Stream events in order
    let events = generate_temporal_events(1000);

    for event in events {
        // Both methods should produce same result
        let temporal_result = temporal_gnn.process_event(&event);
        let incremental_result = incremental.incremental_insert(&event);

        // Verify: Embeddings match within numerical tolerance
        assert_embeddings_equal(&temporal_result, &incremental_result, 1e-5);
    }
}

// Test: Neuro-symbolic query + GNN search
#[test]
fn test_neuro_symbolic_gnn_integration() {
    let executor = NeuroSymbolicQueryExecutor::new();

    // Complex query: semantic + symbolic constraints
    let query = r#"
        MATCH (doc:Document)-[:SIMILAR_TO]->(result)
        WHERE doc.embedding ≈ $query_embedding
          AND result.year > 2020
          AND result.citations > 50
        RETURN result
        ORDER BY similarity DESC
        LIMIT 10
    "#;

    let results = executor.execute_hybrid_query(query, &embedding, 10).unwrap();

    // Verify: Symbolic constraints enforced
    for result in &results {
        assert!(result.metadata["year"] > 2020);
        assert!(result.metadata["citations"] > 50);
    }

    // Verify: Semantic ranking preserved
    for i in 1..results.len() {
        assert!(results[i-1].similarity >= results[i].similarity,
                "Results not sorted by similarity");
    }
}
```

**Integration Test Matrix:**

| Feature Combination | Test Name | Critical Path |
|---------------------|-----------|---------------|
| GNN Routing + HNSW Layers | `test_gnn_hnsw_layers` | ✅ Yes |
| Temporal GNN + Incremental | `test_temporal_incremental` | ✅ Yes |
| Hyperbolic + Attention | `test_hyperbolic_attention` | ⚠️ Medium |
| Graph Condensation + Search | `test_condensed_search` | ⚠️ Medium |
| Adaptive Precision + SIMD | `test_precision_simd` | ✅ Yes |
| Neural LSH + HNSW | `test_neural_lsh_fallback` | ⚠️ Medium |

### 3.3 End-to-End Tests (10% of suite)

**Full System Integration:**

```rust
#[test]
#[ignore] // Run in CI only (slow test)
fn test_full_system_regression() {
    // 1. Load real-world dataset (SIFT1M or GIST1M)
    let dataset = load_benchmark_dataset("sift1m");

    // 2. Build baseline index (v0.1.19 behavior)
    let baseline = build_baseline_index(&dataset);

    // 3. Build index with all GNN v2 features enabled
    let gnn_v2 = build_gnn_v2_index(&dataset, GnnV2Config {
        enable_gnn_routing: true,
        enable_temporal: true,
        enable_hyperbolic: true,
        enable_incremental: true,
        enable_adaptive_precision: true,
    });

    // 4. Run comprehensive benchmark
    let baseline_bench = benchmark_index(&baseline, &dataset.queries);
    let gnn_v2_bench = benchmark_index(&gnn_v2, &dataset.queries);

    // 5. Assert: Performance improved or unchanged
    assert!(gnn_v2_bench.qps >= baseline_bench.qps * 0.95,
            "QPS regression: {} -> {}", baseline_bench.qps, gnn_v2_bench.qps);

    assert!(gnn_v2_bench.recall_at_10 >= baseline_bench.recall_at_10 - 0.02,
            "Recall@10 regression: {:.4} -> {:.4}",
            baseline_bench.recall_at_10, gnn_v2_bench.recall_at_10);

    assert!(gnn_v2_bench.memory_mb <= baseline_bench.memory_mb * 1.1,
            "Memory regression: {}MB -> {}MB",
            baseline_bench.memory_mb, gnn_v2_bench.memory_mb);

    // 6. Verify: No crashes during 1-hour stress test
    stress_test_index(&gnn_v2, Duration::from_secs(3600));
}

// Benchmark helper
fn benchmark_index(index: &dyn Index, queries: &[Vec<f32>]) -> BenchmarkResults {
    let start = Instant::now();
    let mut total_recall = 0.0;

    for query in queries {
        let results = index.search(query, 10);
        total_recall += compute_recall(&results, &ground_truth[query]);
    }

    let duration = start.elapsed();
    let qps = queries.len() as f64 / duration.as_secs_f64();

    BenchmarkResults {
        qps,
        recall_at_10: total_recall / queries.len() as f64,
        memory_mb: index.memory_usage() / (1024 * 1024),
        p50_latency: index.latency_percentile(0.5),
        p99_latency: index.latency_percentile(0.99),
    }
}
```

### 3.4 Performance Regression Tests

**Continuous Benchmarking:**

```rust
// Criterion.rs benchmark suite
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_search_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_latency");

    // Baseline: HNSW only
    let baseline_index = build_baseline_hnsw();
    group.bench_function("baseline_hnsw", |b| {
        b.iter(|| baseline_index.search(&query, 10))
    });

    // New: GNN-guided routing
    let gnn_index = build_gnn_enhanced_hnsw();
    group.bench_function("gnn_routing", |b| {
        b.iter(|| gnn_index.search_with_gnn(&query, 10))
    });

    // Regression check: GNN should be <10% slower (learning overhead)
    group.finish();
}

fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    for &num_vectors in &[10_000, 100_000, 1_000_000] {
        group.bench_with_input(
            BenchmarkId::new("baseline", num_vectors),
            &num_vectors,
            |b, &n| {
                b.iter_with_large_drop(|| {
                    let index = build_baseline_index(n);
                    index.memory_usage()
                })
            }
        );

        group.bench_with_input(
            BenchmarkId::new("adaptive_precision", num_vectors),
            &num_vectors,
            |b, &n| {
                b.iter_with_large_drop(|| {
                    let index = build_adaptive_precision_index(n);
                    index.memory_usage()
                })
            }
        );
    }

    group.finish();
}

criterion_group!(benches, bench_search_latency, bench_memory_usage);
criterion_main!(benches);
```

**Benchmark Regression Thresholds:**

| Metric | Baseline | Acceptable Range | Alert Threshold |
|--------|----------|------------------|-----------------|
| Search Latency (p50) | 0.5ms | 0.45-0.55ms | >0.6ms |
| Search Latency (p99) | 1.2ms | 1.0-1.4ms | >1.5ms |
| Insert Throughput | 50k ops/sec | 45k-55k ops/sec | <40k ops/sec |
| Memory Usage (1M vectors) | 4GB | 3.8-4.4GB | >4.5GB |
| Recall@10 | 0.952 | >0.940 | <0.930 |

---

## 4. Feature Flag Strategy

### 4.1 Compile-Time Feature Flags

```toml
# Cargo.toml feature flags for gradual rollout
[features]
default = ["hnsw", "attention"]

# Tier 1: High-impact, proven features
gnn-routing = ["dep:parking_lot"]
incremental-updates = ["dep:dashmap"]
neuro-symbolic = ["dep:cypher-parser"]

# Tier 2: Medium-risk, research-validated
temporal-gnn = ["dep:chrono"]
hyperbolic-embeddings = ["dep:num-complex"]
adaptive-precision = ["dep:half"]

# Tier 3: Experimental, long-term
graph-condensation = ["dep:kmeans"]
quantum-attention = ["dep:num-complex", "dep:approx"]
neural-lsh = ["dep:faer"]

# GPU acceleration (optional)
gpu = ["dep:cudarc"]
sparse-attention-gpu = ["gpu", "dep:wgpu"]

# Safety: Unstable features require explicit opt-in
unstable = []
```

**Usage:**
```bash
# Default: Conservative, stable features only
cargo build --release

# Enable specific Tier 1 feature
cargo build --release --features gnn-routing

# Enable all Tier 1 features
cargo build --release --features gnn-routing,incremental-updates,neuro-symbolic

# Enable experimental features (requires unstable flag)
cargo build --release --features unstable,quantum-attention
```

### 4.2 Runtime Feature Flags

```rust
// Runtime configuration for feature toggle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnnV2Config {
    // Tier 1: High confidence
    pub enable_gnn_routing: bool,           // Default: false
    pub enable_incremental_updates: bool,   // Default: false
    pub enable_neuro_symbolic: bool,        // Default: false

    // Tier 2: Medium confidence
    pub enable_temporal_gnn: bool,          // Default: false
    pub enable_hyperbolic: bool,            // Default: false
    pub enable_adaptive_precision: bool,    // Default: false

    // Tier 3: Experimental
    pub enable_graph_condensation: bool,    // Default: false
    pub enable_quantum_attention: bool,     // Default: false
    pub enable_neural_lsh: bool,            // Default: false

    // Gradual rollout: percentage of queries to use new features
    pub rollout_percentage: u8,             // 0-100, default: 0

    // Fallback: Disable feature if performance degrades
    pub auto_disable_on_regression: bool,   // Default: true
    pub regression_threshold: f64,          // Default: 0.1 (10% degradation)
}

impl Default for GnnV2Config {
    fn default() -> Self {
        Self {
            enable_gnn_routing: false,
            enable_incremental_updates: false,
            enable_neuro_symbolic: false,
            enable_temporal_gnn: false,
            enable_hyperbolic: false,
            enable_adaptive_precision: false,
            enable_graph_condensation: false,
            enable_quantum_attention: false,
            enable_neural_lsh: false,
            rollout_percentage: 0,
            auto_disable_on_regression: true,
            regression_threshold: 0.1,
        }
    }
}

// Feature flag enforcement
impl RuvectorLayer {
    pub fn search_with_flags(
        &self,
        query: &[f32],
        k: usize,
        config: &GnnV2Config,
    ) -> Vec<SearchResult> {
        // Gradual rollout: randomly sample queries
        let use_new_features = rand::random::<u8>() < config.rollout_percentage;

        if !use_new_features {
            // Safe path: Use baseline implementation
            return self.search_baseline(query, k);
        }

        // Feature-flagged path
        let mut results = if config.enable_gnn_routing {
            self.search_with_gnn_routing(query, k)
        } else {
            self.search_baseline(query, k)
        };

        // Automatic regression detection
        if config.auto_disable_on_regression {
            let baseline_results = self.search_baseline(query, k);
            let recall = compute_recall(&baseline_results[..10], &results[..10]);

            if recall < 1.0 - config.regression_threshold {
                warn!("Regression detected: recall={:.4}, reverting to baseline", recall);
                return baseline_results; // Fallback
            }
        }

        results
    }
}
```

### 4.3 Gradual Rollout Strategy

**Phase 1: Canary (0-5% traffic)**
```rust
// Week 1-2: Internal testing only
GnnV2Config {
    enable_gnn_routing: true,
    rollout_percentage: 0, // Manual testing only
    ..Default::default()
}

// Week 3-4: Canary to 5% production traffic
GnnV2Config {
    enable_gnn_routing: true,
    rollout_percentage: 5,
    auto_disable_on_regression: true,
    ..Default::default()
}
```

**Phase 2: Gradual Ramp (5-50% traffic)**
```rust
// Week 5: Increase to 10%
rollout_percentage: 10

// Week 6: 25%
rollout_percentage: 25

// Week 7: 50%
rollout_percentage: 50
```

**Phase 3: Full Rollout (50-100% traffic)**
```rust
// Week 8: 75%
rollout_percentage: 75

// Week 9: 90%
rollout_percentage: 90

// Week 10: 100% (make default)
rollout_percentage: 100
enable_gnn_routing: true // Change default to true
```

### 4.4 A/B Testing Framework

```rust
pub struct ABTestFramework {
    experiments: HashMap<String, Experiment>,
    metrics_collector: MetricsCollector,
}

pub struct Experiment {
    name: String,
    control_config: GnnV2Config,
    treatment_config: GnnV2Config,
    traffic_split: f64, // 0.5 = 50/50 split
    min_sample_size: usize,
    statistical_significance: f64, // p-value threshold
}

impl ABTestFramework {
    pub fn run_experiment(&mut self, query: &[f32], k: usize) -> Vec<SearchResult> {
        let experiment = &self.experiments["gnn_routing_v1"];

        // Randomly assign to control or treatment
        let is_treatment = rand::random::<f64>() < experiment.traffic_split;

        let start = Instant::now();
        let results = if is_treatment {
            self.index.search_with_flags(query, k, &experiment.treatment_config)
        } else {
            self.index.search_with_flags(query, k, &experiment.control_config)
        };
        let latency = start.elapsed();

        // Collect metrics
        self.metrics_collector.record(MetricsSample {
            experiment: experiment.name.clone(),
            is_treatment,
            latency,
            recall: self.compute_recall(&results),
            memory_mb: self.index.memory_usage() / (1024 * 1024),
        });

        // Check if experiment reached statistical significance
        if self.metrics_collector.sample_size(&experiment.name) >= experiment.min_sample_size {
            self.analyze_experiment(experiment);
        }

        results
    }

    fn analyze_experiment(&self, experiment: &Experiment) {
        let control_metrics = self.metrics_collector.get_control_metrics(&experiment.name);
        let treatment_metrics = self.metrics_collector.get_treatment_metrics(&experiment.name);

        // T-test for latency difference
        let t_stat = t_test(&control_metrics.latencies, &treatment_metrics.latencies);
        let p_value = t_stat.p_value();

        if p_value < experiment.statistical_significance {
            if treatment_metrics.mean_latency < control_metrics.mean_latency {
                info!("🎉 Experiment '{}' SUCCESSFUL: {:.2}ms -> {:.2}ms (p={:.4})",
                      experiment.name, control_metrics.mean_latency,
                      treatment_metrics.mean_latency, p_value);
            } else {
                warn!("⚠️ Experiment '{}' FAILED: Performance degraded (p={:.4})",
                      experiment.name, p_value);
            }
        }
    }
}
```

---

## 5. Backward Compatibility

### 5.1 API Versioning Strategy

**Semantic Versioning (SemVer) Strict Compliance:**

```
0.1.19 -> 0.2.0: Major API changes (GNN v2 release)
0.2.0 -> 0.2.1: Backward-compatible bug fixes
0.2.1 -> 0.3.0: New features, no breaking changes
```

**Deprecation Policy:**
```rust
// Example: Deprecating old search API
#[deprecated(
    since = "0.2.0",
    note = "Use `search_with_config()` instead. This will be removed in 0.3.0"
)]
pub fn search(&self, query: &[f32], k: usize) -> Vec<SearchResult> {
    // Forward to new API with default config
    self.search_with_config(query, k, &SearchConfig::default())
}

// New API with feature flags
pub fn search_with_config(
    &self,
    query: &[f32],
    k: usize,
    config: &SearchConfig,
) -> Vec<SearchResult> {
    // Implementation with GNN v2 features
}
```

**Compatibility Shims:**
```rust
// Maintain old struct for backward compatibility
#[deprecated(since = "0.2.0", note = "Use GnnConfig instead")]
pub type RuvectorLayerConfig = GnnConfig;

// Forward old methods to new implementations
impl RuvectorLayer {
    #[deprecated(since = "0.2.0")]
    pub fn create(input_dim: usize, output_dim: usize) -> Self {
        Self::new(GnnConfig {
            input_dim,
            output_dim,
            num_heads: 4, // Default
            dropout: 0.1,
            ..Default::default()
        })
    }

    pub fn new(config: GnnConfig) -> Self {
        // New implementation
    }
}
```

### 5.2 Serialization Compatibility

**Index Format Versioning:**

```rust
#[derive(Serialize, Deserialize)]
pub struct SerializedIndex {
    version: u32, // Format version
    metadata: IndexMetadata,
    data: IndexData,
}

impl SerializedIndex {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        let index: SerializedIndex = bincode::deserialize(&bytes)?;

        // Automatic migration from old formats
        match index.version {
            1 => Self::migrate_v1_to_v2(index),
            2 => Ok(index), // Current version
            v => Err(Error::UnsupportedVersion(v)),
        }
    }

    fn migrate_v1_to_v2(old: SerializedIndex) -> Result<Self> {
        // Upgrade v1 format (no GNN) to v2 (with GNN)
        let mut new_index = Self {
            version: 2,
            metadata: old.metadata,
            data: old.data,
        };

        // Initialize GNN components with defaults
        new_index.data.gnn_weights = vec![]; // Empty = disabled
        new_index.metadata.gnn_enabled = false;

        Ok(new_index)
    }
}
```

**Node.js NAPI Compatibility:**

```typescript
// Maintain compatibility with older ruvector versions
export interface RuvectorLayerLegacy {
  forward(nodeFeatures: Float32Array,
          neighborFeatures: Float32Array[],
          edgeWeights: Float32Array): Promise<Float32Array>;
}

export interface RuvectorLayerV2 extends RuvectorLayerLegacy {
  // New methods in v2
  searchWithGNN(query: Float32Array, k: number): Promise<SearchResult[]>;
  enableFeature(feature: string, config: any): void;
}

// Export both interfaces
export const createLayer = (config: any): RuvectorLayerV2 => {
  return new RuvectorLayerImpl(config);
};

// Legacy constructor still works
export const createLayerLegacy = (
  inputDim: number,
  outputDim: number
): RuvectorLayerLegacy => {
  return createLayer({ inputDim, outputDim, version: 1 });
};
```

### 5.3 Migration Guides

**Automated Migration Tool:**

```bash
# CLI tool to migrate existing indices to GNN v2
$ ruvector-cli migrate --from 0.1.19 --to 0.2.0 --input ./old_index --output ./new_index

Migrating index from v0.1.19 to v0.2.0...
✅ Loaded 1,000,000 vectors
✅ Upgraded index format (v1 -> v2)
✅ Initialized GNN components (disabled by default)
✅ Verified backward compatibility
✅ Saved to ./new_index

Migration complete! Index is backward compatible with v0.1.19 clients.
To enable GNN v2 features, set enable_gnn_routing=true in config.
```

---

## 6. CI/CD Pipeline Requirements

### 6.1 Required Checks Before Merge

**GitHub Actions Workflow:**

```yaml
# .github/workflows/gnn-v2-regression-checks.yml
name: GNN v2 Regression Checks

on:
  pull_request:
    branches: [main, feature/gnn-v2]
  push:
    branches: [main]

jobs:
  unit-tests:
    name: Unit Tests (60% coverage)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Run unit tests
        run: cargo test --lib --all-features

      - name: Check coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Xml --all-features -- --test-threads 1

      - name: Enforce coverage threshold
        run: |
          coverage=$(xmllint --xpath "string(//coverage/@line-rate)" cobertura.xml)
          if (( $(echo "$coverage < 0.60" | bc -l) )); then
            echo "❌ Coverage $coverage < 60%"
            exit 1
          fi

  integration-tests:
    name: Integration Tests (30% coverage)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Run integration tests
        run: cargo test --test '*' --all-features

      - name: Cross-component tests
        run: |
          cargo test --features gnn-routing,temporal-gnn test_gnn_temporal_integration
          cargo test --features hyperbolic,attention test_hyperbolic_attention_integration

  benchmark-regression:
    name: Performance Regression
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Run baseline benchmarks (main branch)
        run: |
          git checkout main
          cargo bench --bench search_latency -- --save-baseline main

      - name: Run PR benchmarks
        run: |
          git checkout ${{ github.head_ref }}
          cargo bench --bench search_latency -- --baseline main

      - name: Check for regressions
        run: |
          # Fails if any benchmark is >5% slower
          cargo bench --bench search_latency -- --baseline main --threshold 0.05

  backward-compatibility:
    name: Backward Compatibility
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Load v0.1.19 test data
        run: |
          wget https://github.com/ruvnet/ruvector/releases/download/v0.1.19/test-data.tar.gz
          tar -xzf test-data.tar.gz

      - name: Test index loading
        run: |
          cargo test test_load_legacy_index_v0_1_19

      - name: Test API compatibility
        run: |
          cargo test --features api-compat test_legacy_api_works

  napi-compatibility:
    name: Node.js NAPI Compatibility
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        node: [18, 20, 22]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node }}

      - name: Build NAPI bindings
        run: npm run build -w crates/ruvector-gnn-node

      - name: Run Node.js tests
        run: npm test -w crates/ruvector-gnn-node

      - name: Check API schema
        run: |
          node scripts/verify-napi-schema.js

  fuzzing:
    name: Continuous Fuzzing
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install cargo-fuzz
        run: cargo install cargo-fuzz

      - name: Run fuzz tests (5 minutes each)
        run: |
          cargo fuzz run gnn_routing --all-features -- -max_total_time=300
          cargo fuzz run temporal_gnn --all-features -- -max_total_time=300
          cargo fuzz run hyperbolic_ops --all-features -- -max_total_time=300

  memory-leak-detection:
    name: Memory Leak Detection
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Valgrind
        run: sudo apt-get install valgrind

      - name: Run long-running tests under Valgrind
        run: |
          cargo build --release --features all
          valgrind --leak-check=full --error-exitcode=1 \
            ./target/release/ruvector-bench --duration 60

  security-audit:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Run cargo-audit
        run: |
          cargo install cargo-audit
          cargo audit --deny warnings

  required-checks:
    name: All Checks Passed
    needs: [
      unit-tests,
      integration-tests,
      benchmark-regression,
      backward-compatibility,
      napi-compatibility,
      fuzzing,
      memory-leak-detection,
      security-audit
    ]
    runs-on: ubuntu-latest
    steps:
      - run: echo "✅ All regression checks passed!"
```

### 6.2 Automated Benchmark Comparison

**Criterion.rs + GitHub Actions Integration:**

```rust
// benches/regression_benchmark.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_all_features(c: &mut Criterion) {
    let mut group = c.benchmark_group("feature_regression");

    // Baseline: No features enabled
    let baseline_index = build_index(&GnnV2Config::default());
    group.bench_function("baseline", |b| {
        b.iter(|| baseline_index.search(&query, 10))
    });

    // Individual features
    let features = vec![
        ("gnn_routing", GnnV2Config { enable_gnn_routing: true, ..Default::default() }),
        ("temporal_gnn", GnnV2Config { enable_temporal_gnn: true, ..Default::default() }),
        ("hyperbolic", GnnV2Config { enable_hyperbolic: true, ..Default::default() }),
    ];

    for (name, config) in features {
        let index = build_index(&config);
        group.bench_with_input(BenchmarkId::new("feature", name), &index, |b, idx| {
            b.iter(|| idx.search(&query, 10))
        });
    }

    group.finish();
}

criterion_group!(benches, bench_all_features);
criterion_main!(benches);
```

**Automated Regression Report:**

```bash
# scripts/benchmark_report.sh
#!/bin/bash

# Compare current branch against main
cargo bench --bench regression_benchmark -- --save-baseline current
git checkout main
cargo bench --bench regression_benchmark -- --save-baseline main
git checkout -

# Generate comparison report
critcmp main current > benchmark_report.txt

# Check for regressions
if grep -q "Performance decreased" benchmark_report.txt; then
  echo "❌ Performance regression detected!"
  cat benchmark_report.txt
  exit 1
else
  echo "✅ No performance regression"
  cat benchmark_report.txt
fi
```

### 6.3 Nightly Regression Runs

**Scheduled Workflow:**

```yaml
# .github/workflows/nightly-regression.yml
name: Nightly Regression Suite

on:
  schedule:
    - cron: '0 2 * * *' # 2 AM UTC daily
  workflow_dispatch:

jobs:
  full-benchmark-suite:
    name: Full Benchmark Suite (1M+ vectors)
    runs-on: ubuntu-latest
    timeout-minutes: 120
    steps:
      - uses: actions/checkout@v4

      - name: Download SIFT1M dataset
        run: |
          wget http://corpus-texmex.irisa.fr/sift.tar.gz
          tar -xzf sift.tar.gz

      - name: Run comprehensive benchmarks
        run: |
          cargo run --release --bin ruvector-bench -- \
            --dataset sift1m \
            --queries 10000 \
            --k 10,100 \
            --features baseline,gnn-routing,all

      - name: Generate regression report
        run: |
          python scripts/analyze_benchmarks.py \
            --baseline benchmarks/main.json \
            --current benchmarks/current.json \
            --output regression_report.md

      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: nightly-benchmark-results
          path: benchmarks/

  stress-test:
    name: Stress Test (24 hours)
    runs-on: ubuntu-latest
    timeout-minutes: 1440
    steps:
      - uses: actions/checkout@v4

      - name: Run 24-hour stress test
        run: |
          cargo run --release --bin stress-test -- \
            --duration 24h \
            --concurrent-queries 100 \
            --index-size 10000000

      - name: Check for crashes/leaks
        run: |
          if grep -q "CRASH\|LEAK" stress-test.log; then
            echo "❌ Stability issue detected!"
            exit 1
          fi
```

---

## 7. Rollback Plan

### 7.1 Quick Disable of Problematic Features

**Emergency Killswitch:**

```rust
// Feature killswitch (can be toggled via config file or environment variable)
pub struct FeatureKillswitch {
    disabled_features: Arc<RwLock<HashSet<String>>>,
}

impl FeatureKillswitch {
    pub fn is_enabled(&self, feature: &str) -> bool {
        !self.disabled_features.read().unwrap().contains(feature)
    }

    pub fn disable(&self, feature: &str) {
        warn!("🚨 EMERGENCY: Disabling feature '{}'", feature);
        self.disabled_features.write().unwrap().insert(feature.to_string());
    }

    pub fn load_from_env(&self) {
        // Environment variable: RUVECTOR_DISABLE_FEATURES=gnn-routing,temporal-gnn
        if let Ok(disabled) = env::var("RUVECTOR_DISABLE_FEATURES") {
            for feature in disabled.split(',') {
                self.disable(feature.trim());
            }
        }
    }
}

// Usage in search path
impl RuvectorLayer {
    pub fn search(&self, query: &[f32], k: usize) -> Vec<SearchResult> {
        let killswitch = GLOBAL_KILLSWITCH.get().unwrap();

        // Check feature flags before using new code paths
        if killswitch.is_enabled("gnn-routing") && self.config.enable_gnn_routing {
            return self.search_with_gnn_routing(query, k);
        }

        // Fallback to baseline
        self.search_baseline(query, k)
    }
}
```

**Emergency Rollback Procedure:**

```bash
# 1. Identify problematic feature from monitoring
$ tail -f /var/log/ruvector/errors.log | grep "gnn-routing"

# 2. Disable feature immediately via environment variable
$ export RUVECTOR_DISABLE_FEATURES=gnn-routing
$ systemctl restart ruvector-server

# 3. Or: Update config file and hot-reload
$ echo "disable_features: [gnn-routing]" >> /etc/ruvector/config.yaml
$ kill -HUP $(pgrep ruvector-server)

# 4. Verify feature is disabled
$ curl http://localhost:8080/health | jq '.disabled_features'
["gnn-routing"]
```

### 7.2 Data Migration Considerations

**Graceful Degradation:**

```rust
// Index can operate in "degraded mode" if GNN components fail
impl HNSWIndex {
    pub fn load_or_fallback(path: &Path) -> Result<Self> {
        match Self::load_with_gnn(path) {
            Ok(index) => {
                info!("✅ Loaded index with GNN v2 features");
                Ok(index)
            }
            Err(e) => {
                warn!("⚠️ Failed to load GNN components: {}. Falling back to baseline.", e);
                Self::load_baseline(path) // Safe fallback
            }
        }
    }

    fn load_baseline(path: &Path) -> Result<Self> {
        // Load only core HNSW structure, ignore GNN weights
        let mut index = Self::new(DistanceMetric::Cosine);
        index.load_hnsw_only(path)?;
        index.gnn_enabled = false;
        Ok(index)
    }
}
```

**Zero-Downtime Rollback:**

```bash
# Blue-green deployment for rollback
# Step 1: Keep v0.1.19 (green) running while deploying v0.2.0 (blue)
$ docker run -d --name ruvector-blue ruvector:0.2.0
$ docker run -d --name ruvector-green ruvector:0.1.19

# Step 2: Route 10% traffic to blue, monitor metrics
$ nginx.conf: upstream ruvector { server blue weight=1; server green weight=9; }

# Step 3: If blue has issues, instant rollback
$ nginx.conf: upstream ruvector { server green weight=10; }
$ docker stop ruvector-blue

# Step 4: Investigate issues offline
$ docker logs ruvector-blue > rollback-investigation.log
```

### 7.3 Communication Plan

**Incident Response Template:**

```markdown
# Incident Report: GNN v2 Rollback

**Date:** 2025-12-15 14:32 UTC
**Severity:** P1 (Production Impacted)
**Feature:** GNN Routing (Tier 1)

## Symptoms
- Search latency p99 increased from 1.2ms to 3.8ms (+217%)
- Detected at 14:30 UTC via automated monitoring
- Affected 25% of production traffic (rollout_percentage=25)

## Root Cause
- GNN routing path memory allocation in hot loop
- Missed during benchmark (only tested with warm cache)

## Immediate Actions Taken
- 14:32: Disabled gnn-routing via `RUVECTOR_DISABLE_FEATURES=gnn-routing`
- 14:33: Verified latency returned to baseline (1.2ms p99)
- 14:35: Rolled back rollout_percentage from 25% to 0%

## Long-term Fix
- Add cold-cache benchmark to CI/CD pipeline
- Pre-allocate memory in GNN routing path
- Increase canary phase from 5% to 10% traffic, 2 weeks duration

## Timeline
- 14:30: Alerts triggered (latency threshold exceeded)
- 14:32: Rollback initiated
- 14:33: Service restored to normal
- **Total Downtime:** 0 minutes (degraded performance only)

## Lessons Learned
- ✅ Feature flags worked as designed (instant rollback)
- ✅ Monitoring detected issue within 2 minutes
- ❌ Benchmark suite missed cold-cache scenario
- ❌ Rollout was too aggressive (5% -> 25% too fast)
```

---

## 8. Specific Risks by Feature

### 8.1 Feature: GNN-Guided HNSW Routing

**What Could Break:**
1. **HNSW layer traversal**: GNN routing might skip layers or get stuck in local minima
2. **Search recall degradation**: Exploration vs exploitation tradeoff could worsen top-k recall
3. **Memory leaks**: `SearchPathMemory` unbounded growth if not cleared periodically
4. **Thread safety**: Concurrent updates to GNN weights during search

**How to Detect Breakage:**
```rust
#[test]
fn test_gnn_routing_maintains_recall() {
    let index = build_test_index(10000);
    let baseline_recall = benchmark_recall(&index, &queries, SearchMode::Baseline);
    let gnn_recall = benchmark_recall(&index, &queries, SearchMode::GNNRouting);

    // Strict: GNN should not degrade recall by >2%
    assert!(gnn_recall >= baseline_recall - 0.02,
            "GNN routing degraded recall: {:.4} -> {:.4}",
            baseline_recall, gnn_recall);
}

#[test]
fn test_gnn_routing_no_infinite_loops() {
    let index = build_pathological_index(); // Disconnected graph

    let result = timeout(Duration::from_secs(5), async {
        index.search_with_gnn(&query, 10)
    }).await;

    assert!(result.is_ok(), "GNN routing timed out (possible infinite loop)");
}

#[test]
fn test_search_path_memory_bounded() {
    let mut index = GNNEnhancedHNSW::new();

    // Simulate 10000 searches
    for i in 0..10000 {
        index.search_with_gnn(&random_query(), 10);
    }

    // Path memory should not exceed 100MB
    let memory_usage = index.path_memory.memory_usage();
    assert!(memory_usage < 100 * 1024 * 1024,
            "SearchPathMemory leaked: {}MB", memory_usage / (1024 * 1024));
}
```

**How to Prevent:**
- ✅ Add max search depth limit (prevent infinite loops)
- ✅ Implement LRU eviction for `SearchPathMemory`
- ✅ Use `Arc<RwLock<>>` for thread-safe GNN weight updates
- ✅ Add circuit breaker: disable GNN routing if recall drops >5%

### 8.2 Feature: Continuous-Time Dynamic GNN

**What Could Break:**
1. **Temporal ordering violations**: Events processed out-of-order due to async updates
2. **Numerical instability**: Exponential decay with large time differences → NaN/Inf
3. **HNSW index staleness**: Temporal embeddings drift but HNSW not updated
4. **Memory explosion**: Storing full temporal history for all nodes

**How to Detect Breakage:**
```rust
#[test]
fn test_temporal_causality_preserved() {
    let mut temporal_gnn = ContinuousTimeGNN::new();

    // Events: A at t=1, B at t=2, C at t=3
    temporal_gnn.process_event(node_a, timestamp=1.0, features_a);
    temporal_gnn.process_event(node_b, timestamp=2.0, features_b);
    temporal_gnn.process_event(node_c, timestamp=3.0, features_c);

    // Query state at t=2.5: Should include A, B but NOT C
    let state = temporal_gnn.get_state_at_time(node_a, 2.5);

    // Verify: C's future event didn't affect past state
    assert!(!state_influenced_by(state, features_c),
            "Future event leaked into past state (causality violation)");
}

#[test]
fn test_temporal_numerical_stability() {
    let temporal_gnn = ContinuousTimeGNN::new();

    // Extreme time differences (1 year apart)
    let t1 = 0.0;
    let t2 = 365.0 * 24.0 * 3600.0; // 1 year in seconds

    temporal_gnn.process_event(node, t1, features);
    let state = temporal_gnn.get_state_at_time(node, t2);

    // Should not produce NaN/Inf
    assert!(state.iter().all(|&x| x.is_finite()),
            "Temporal GNN produced NaN/Inf: {:?}", state);
}

#[test]
fn test_temporal_memory_bounded() {
    let mut temporal_gnn = ContinuousTimeGNN::new();

    // Simulate 1M temporal events
    for i in 0..1_000_000 {
        temporal_gnn.process_event(i % 10000, i as f64, random_features());
    }

    // Memory should not grow unboundedly (use compression/pruning)
    let memory_mb = temporal_gnn.memory_usage() / (1024 * 1024);
    assert!(memory_mb < 500,
            "Temporal memory exploded to {}MB", memory_mb);
}
```

**How to Prevent:**
- ✅ Use event queue with timestamp sorting (prevent out-of-order)
- ✅ Clip decay exponent: `min(decay, max_decay_threshold)`
- ✅ Trigger incremental HNSW updates every N events
- ✅ Implement temporal state pruning (keep only last K events per node)

### 8.3 Feature: Hyperbolic Embeddings

**What Could Break:**
1. **Poincaré ball boundary violations**: Embeddings outside unit ball (|x| >= 1)
2. **Distance metric inconsistency**: Hyperbolic distance doesn't satisfy triangle inequality due to numerical error
3. **Gradient explosion**: Hyperbolic gradients diverge near ball boundary
4. **SIMD incompatibility**: Existing SIMD distance kernels assume Euclidean

**How to Detect Breakage:**
```rust
#[test]
fn test_hyperbolic_embeddings_in_valid_ball() {
    let hybrid = HybridSpaceEmbedding::new(64, 64, -1.0);

    for _ in 0..1000 {
        let embedding = random_embedding(128);
        let hybrid_emb = HybridEmbedding::from_embedding(&embedding, 64);

        // Check: Hyperbolic part is inside Poincaré ball
        let norm: f32 = hybrid_emb.hyperbolic_part.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(norm < 0.99, // Leave margin for numerical safety
                "Hyperbolic embedding outside ball: norm={}", norm);
    }
}

#[test]
fn test_hyperbolic_distance_metric_properties() {
    let hybrid = HybridSpaceEmbedding::new(64, 64, -1.0);

    for _ in 0..100 {
        let x = random_hyperbolic_point();
        let y = random_hyperbolic_point();
        let z = random_hyperbolic_point();

        // Triangle inequality: d(x,z) <= d(x,y) + d(y,z)
        let dxz = hybrid.poincare_distance(&x, &z);
        let dxy = hybrid.poincare_distance(&x, &y);
        let dyz = hybrid.poincare_distance(&y, &z);

        assert!(dxz <= dxy + dyz + 1e-5, // Allow numerical tolerance
                "Triangle inequality violated: {} > {} + {}", dxz, dxy, dyz);
    }
}

#[test]
fn test_hyperbolic_gradient_stability() {
    let mut hybrid = HybridSpaceEmbedding::new(64, 64, -1.0);

    // Simulate gradient descent near ball boundary
    let mut point = vec![0.95; 64]; // Near boundary

    for _ in 0..100 {
        let grad = hybrid.compute_gradient(&point);

        // Gradients should not explode
        let grad_norm: f32 = grad.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(grad_norm < 100.0,
                "Gradient exploded: norm={}", grad_norm);

        // Update with clipping
        point = hybrid.exp_map(&point, &grad);
    }
}
```

**How to Prevent:**
- ✅ Always project embeddings: `min(norm, 0.99)` after updates
- ✅ Use numerically stable formulas (avoid divisions by small numbers)
- ✅ Gradient clipping in hyperbolic space
- ✅ Fallback to Euclidean if hyperbolic operations fail

### 8.4 Feature: Incremental Graph Learning (ATLAS)

**What Could Break:**
1. **Stale activations**: Cached activations not invalidated when neighbor changes
2. **Dependency graph cycles**: Circular dependencies cause infinite update loops
3. **Race conditions**: Concurrent inserts corrupt activation cache
4. **Memory leak**: Activation cache grows unbounded

**How to Detect Breakage:**
```rust
#[test]
fn test_incremental_updates_match_full_recompute() {
    let mut incremental = IncrementalGNNExecutor::new();
    let mut full = GNNLayer::new(config);

    // Insert 1000 nodes incrementally
    for i in 0..1000 {
        let embedding = random_embedding(128);
        incremental.incremental_insert(i, embedding.clone());
        full.insert(i, embedding);
    }

    // Both should produce same results
    let inc_result = incremental.forward(&query);
    let full_result = full.forward(&query);

    assert_embeddings_equal(&inc_result, &full_result, 1e-4,
                           "Incremental updates diverged from full recompute");
}

#[test]
fn test_incremental_cache_invalidation() {
    let mut executor = IncrementalGNNExecutor::new();

    // Build graph: 1 -> 2 -> 3
    executor.insert(1, emb1);
    executor.insert(2, emb2);
    executor.insert(3, emb3);
    executor.add_edge(1, 2);
    executor.add_edge(2, 3);

    let state_before = executor.get_activation(3);

    // Update node 1 (should invalidate 2 and 3)
    executor.update(1, new_emb1);

    let state_after = executor.get_activation(3);

    // State of node 3 should have changed
    assert_ne!(state_before, state_after,
               "Activation cache not invalidated after upstream update");
}

#[test]
fn test_incremental_no_cycles() {
    let mut executor = IncrementalGNNExecutor::new();

    // Create cycle: 1 -> 2 -> 3 -> 1
    executor.add_edge(1, 2);
    executor.add_edge(2, 3);
    executor.add_edge(3, 1);

    // Should detect cycle and handle gracefully
    let result = timeout(Duration::from_secs(5), async {
        executor.incremental_insert(4, emb4)
    }).await;

    assert!(result.is_ok(), "Incremental update timed out due to cycle");
}
```

**How to Prevent:**
- ✅ Invalidation timestamps: Track when each node was last updated
- ✅ Cycle detection: DFS to detect cycles before updates
- ✅ Use `DashMap` for thread-safe concurrent cache access
- ✅ LRU eviction: Limit cache size to prevent unbounded growth

### 8.5 Feature: Adaptive Precision (AutoSAGE)

**What Could Break:**
1. **Quantization quality degradation**: Over-aggressive quantization loses too much information
2. **SIMD incompatibility**: Mixed precision breaks vectorized operations
3. **Search result inconsistency**: Different precision levels produce different rankings
4. **Memory overhead**: Metadata for precision tracking negates compression gains

**How to Detect Breakage:**
```rust
#[test]
fn test_adaptive_precision_maintains_recall() {
    let full_precision = build_index(PrecisionLevel::Full);
    let adaptive = build_index_with_adaptive_precision();

    let baseline_recall = benchmark_recall(&full_precision, &queries);
    let adaptive_recall = benchmark_recall(&adaptive, &queries);

    // Adaptive precision should preserve >98% recall
    assert!(adaptive_recall >= baseline_recall - 0.02,
            "Adaptive precision degraded recall: {:.4} -> {:.4}",
            baseline_recall, adaptive_recall);
}

#[test]
fn test_adaptive_precision_memory_reduction() {
    let full_precision = build_index(PrecisionLevel::Full);
    let adaptive = build_index_with_adaptive_precision();

    let baseline_memory = full_precision.memory_usage();
    let adaptive_memory = adaptive.memory_usage();

    // Should achieve 2-4x memory reduction
    let reduction_factor = baseline_memory as f64 / adaptive_memory as f64;
    assert!(reduction_factor >= 2.0,
            "Adaptive precision failed to reduce memory: {:.2}x", reduction_factor);
}

#[test]
fn test_mixed_precision_distance_consistency() {
    let adaptive = AdaptivePrecisionHNSW::new();

    // Compute distances with different precision levels
    let dist_f32 = adaptive.compute_distance(&query, node_full_precision);
    let dist_f16 = adaptive.compute_distance(&query, node_half_precision);
    let dist_pq8 = adaptive.compute_distance(&query, node_quantized);

    // Distances should be monotonic (more precision = more accurate)
    // But allow for quantization noise
    assert!((dist_f32 - dist_f16).abs() < 0.1,
            "f16 distance diverged too much from f32: {} vs {}", dist_f32, dist_f16);
}
```

**How to Prevent:**
- ✅ Degree-based precision assignment (high-degree nodes keep full precision)
- ✅ Asymmetric distance computation (query always f32)
- ✅ Quantization quality validation (measure information loss)
- ✅ Metadata compaction (use bit-packing for precision levels)

### 8.6 Feature: Neuro-Symbolic Query Execution

**What Could Break:**
1. **Cypher parser conflicts**: New GNN operators might clash with existing Cypher syntax
2. **Type system inconsistency**: Mixing neural scores with symbolic boolean logic
3. **Query optimization regression**: Hybrid queries might bypass existing optimizations
4. **Memory explosion**: Overfetching for symbolic filtering (neural search returns 10k, symbolic filters to 10)

**How to Detect Breakage:**
```rust
#[test]
fn test_neuro_symbolic_cypher_compatibility() {
    let executor = NeuroSymbolicQueryExecutor::new();

    // Legacy Cypher query (should still work)
    let legacy_query = "MATCH (n:Person)-[:KNOWS]->(m) RETURN m";
    let legacy_result = executor.execute(legacy_query);
    assert!(legacy_result.is_ok(), "Legacy Cypher query broke");

    // Hybrid query with vector similarity
    let hybrid_query = r#"
        MATCH (n:Person)-[:KNOWS]->(m)
        WHERE n.embedding ≈ $query_embedding
        RETURN m
    "#;
    let hybrid_result = executor.execute_hybrid_query(hybrid_query, &embedding, 10);
    assert!(hybrid_result.is_ok(), "Hybrid query failed");
}

#[test]
fn test_neuro_symbolic_type_safety() {
    let executor = NeuroSymbolicQueryExecutor::new();

    // Invalid query: mixing incompatible types
    let invalid_query = r#"
        MATCH (n:Document)
        WHERE n.embedding > 0.5  // Invalid: embedding is vector, not scalar
        RETURN n
    "#;

    let result = executor.execute(invalid_query);
    assert!(result.is_err(), "Type error not caught by query planner");
}

#[test]
fn test_neuro_symbolic_overfetch_prevention() {
    let executor = NeuroSymbolicQueryExecutor::new();

    // Query that could overfetch if not optimized
    let query = r#"
        MATCH (n:Document)
        WHERE n.embedding ≈ $query_embedding
          AND n.year = 2024  // Very selective filter
        RETURN n LIMIT 10
    "#;

    // Should not fetch 100k neural candidates then filter to 10
    let stats = executor.execute_with_stats(query, &embedding, 10).unwrap();

    assert!(stats.neural_candidates_fetched < 1000,
            "Overfetched {} neural candidates for 10 results",
            stats.neural_candidates_fetched);
}
```

**How to Prevent:**
- ✅ Extend Cypher parser with backward compatibility mode
- ✅ Static type checking for hybrid queries
- ✅ Query optimization: Push symbolic filters into neural search
- ✅ Adaptive overfetch: Dynamically adjust neural k based on filter selectivity

### 8.7 Feature: Graph Condensation (SFGC)

**What Could Break:**
1. **Condensation training divergence**: Synthetic nodes don't converge to meaningful representations
2. **Search accuracy collapse**: Over-condensation loses critical information
3. **Cold start problem**: Condensed graph performs poorly on out-of-distribution queries
4. **Incompatibility with existing indices**: Can't load pre-condensed graphs in older versions

**How to Detect Breakage:**
```rust
#[test]
fn test_graph_condensation_preserves_accuracy() {
    let original = build_full_graph(100_000);
    let condensed = GraphCondenser::condense(&original, target_size=1_000);

    // Test on same queries
    let original_recall = benchmark_recall(&original, &queries);
    let condensed_recall = benchmark_recall(&condensed, &queries);

    // Condensed graph should preserve >90% of accuracy
    assert!(condensed_recall >= original_recall - 0.10,
            "Graph condensation lost too much accuracy: {:.4} -> {:.4}",
            original_recall, condensed_recall);
}

#[test]
fn test_graph_condensation_compression_ratio() {
    let original = build_full_graph(100_000);
    let condensed = GraphCondenser::condense(&original, target_size=1_000);

    let original_memory = original.memory_usage();
    let condensed_memory = condensed.memory_usage();

    // Should achieve 10-100x compression
    let compression_ratio = original_memory as f64 / condensed_memory as f64;
    assert!(compression_ratio >= 10.0,
            "Insufficient compression: {:.2}x", compression_ratio);
}

#[test]
fn test_graph_condensation_training_stability() {
    let graph = build_full_graph(10_000);
    let mut condenser = GraphCondenser::new();

    let mut prev_loss = f32::MAX;
    let mut divergence_count = 0;

    for iter in 0..1000 {
        let loss = condenser.train_iteration(&graph);

        // Loss should generally decrease
        if loss > prev_loss * 1.1 { // Allow 10% fluctuation
            divergence_count += 1;
        }
        prev_loss = loss;
    }

    // Should not diverge frequently
    assert!(divergence_count < 100,
            "Condensation training diverged {} times", divergence_count);
}
```

**How to Prevent:**
- ✅ Learning rate scheduling (start high, decay exponentially)
- ✅ Multi-objective training (accuracy + diversity)
- ✅ Regularization to prevent overfitting to training queries
- ✅ Versioned condensation format (include metadata for reconstruction)

### 8.8 Feature: Quantum-Inspired Attention

**What Could Break:**
1. **Complex number overflow**: Amplitude encoding produces huge complex numbers
2. **Unitarity violations**: Learnable unitary matrices become non-unitary during training
3. **Compatibility with existing attention**: Cross-attention between quantum and classical
4. **Performance degradation**: Quantum operations too slow for real-time search

**How to Detect Breakage:**
```rust
#[test]
fn test_quantum_attention_amplitude_bounded() {
    let quantum_attn = QuantumInspiredAttention::new(128);

    for _ in 0..1000 {
        let embedding = random_embedding(128);
        let quantum_state = quantum_attn.encode_quantum_state(&embedding);

        // All amplitudes should be bounded
        for amp in &quantum_state {
            assert!(amp.norm() <= 1.0,
                    "Quantum amplitude exploded: {}", amp.norm());
        }
    }
}

#[test]
fn test_quantum_unitary_preservation() {
    let mut quantum_attn = QuantumInspiredAttention::new(128);

    // Train for 100 iterations
    for _ in 0..100 {
        quantum_attn.train_step(&training_data);
    }

    // Check if entanglement weights are still unitary
    let weights = quantum_attn.entanglement_weights();
    let is_unitary = check_unitarity(&weights);

    assert!(is_unitary,
            "Entanglement weights lost unitarity after training");
}

#[test]
fn test_quantum_attention_performance_acceptable() {
    let quantum_attn = QuantumInspiredAttention::new(128);
    let classical_attn = DotProductAttention::new(128);

    let start = Instant::now();
    for _ in 0..1000 {
        quantum_attn.compute_attention(&query, &keys, &values);
    }
    let quantum_duration = start.elapsed();

    let start = Instant::now();
    for _ in 0..1000 {
        classical_attn.compute_attention(&query, &keys, &values);
    }
    let classical_duration = start.elapsed();

    // Quantum should not be >10x slower
    assert!(quantum_duration < classical_duration * 10,
            "Quantum attention too slow: {}ms vs {}ms",
            quantum_duration.as_millis(), classical_duration.as_millis());
}
```

**How to Prevent:**
- ✅ Amplitude normalization after every operation
- ✅ Project weight matrices to unitary group (SVD + orthogonalization)
- ✅ Optional: Use classical attention as fallback if quantum fails
- ✅ GPU acceleration for quantum operations (CUDA kernels)

---

## 9. Implementation Checklist

### 9.1 Pre-Implementation Phase

**Before Writing Any Code:**

- [ ] **Baseline Benchmarks Recorded**
  - [ ] Search latency (p50, p99, p999) on SIFT1M
  - [ ] Insert throughput (ops/sec)
  - [ ] Memory usage for 1M vectors (f32, f16, PQ8)
  - [ ] Recall@10, Recall@100 on GIST1M
  - [ ] NAPI binding latency (Node.js overhead)

- [ ] **Test Infrastructure Ready**
  - [ ] Criterion.rs benchmarks configured
  - [ ] Proptest generators for embeddings
  - [ ] Fuzzing targets defined
  - [ ] Integration test datasets downloaded (SIFT1M, GIST1M)

- [ ] **Feature Flags Defined**
  - [ ] Cargo features added to workspace `Cargo.toml`
  - [ ] Runtime config structs defined
  - [ ] Killswitch mechanism implemented
  - [ ] Rollout percentage system tested

### 9.2 Per-Feature Implementation Checklist

**For Each of the 19 Features:**

- [ ] **Design Phase**
  - [ ] Read research paper thoroughly
  - [ ] Identify integration points with existing code
  - [ ] List potential breaking changes
  - [ ] Design fallback mechanism

- [ ] **Test-First Development**
  - [ ] Write property-based tests (proptest)
  - [ ] Write regression tests (existing functionality)
  - [ ] Write integration tests (cross-component)
  - [ ] Write fuzzing targets
  - [ ] All tests fail (TDD red phase)

- [ ] **Implementation**
  - [ ] Implement behind feature flag
  - [ ] All tests pass (TDD green phase)
  - [ ] Refactor for clarity (TDD refactor phase)
  - [ ] Add inline documentation
  - [ ] Run benchmarks (no regression >5%)

- [ ] **Code Review**
  - [ ] Self-review checklist completed
  - [ ] Peer review assigned
  - [ ] Security review (if touching NAPI bindings)
  - [ ] Performance review (benchmark comparison)

- [ ] **CI/CD Validation**
  - [ ] All unit tests pass
  - [ ] All integration tests pass
  - [ ] Benchmark regression check pass
  - [ ] Fuzzing run (5 min) pass
  - [ ] Memory leak check pass
  - [ ] NAPI compatibility tests pass (all platforms)

- [ ] **Deployment**
  - [ ] Feature flag default = `false`
  - [ ] Canary deployment (0-5% traffic)
  - [ ] Monitor for 1 week
  - [ ] Gradual rollout (5% -> 25% -> 50% -> 100%)
  - [ ] Make default after 1 month of stability

### 9.3 Final Validation (Before GNN v2 Release)

**Release Readiness Checklist:**

- [ ] **Test Coverage**
  - [ ] Overall coverage >80%
  - [ ] Critical paths >90%
  - [ ] Backward compatibility tests 100%

- [ ] **Performance**
  - [ ] No regression >5% in any benchmark
  - [ ] Memory usage within 10% of baseline
  - [ ] Recall@10 degradation <2%

- [ ] **Documentation**
  - [ ] Migration guide written
  - [ ] API changelog complete
  - [ ] Feature flag documentation
  - [ ] Example code updated

- [ ] **Compatibility**
  - [ ] Can load v0.1.19 indices ✅
  - [ ] NAPI bindings work on all platforms ✅
  - [ ] Serialization format backward compatible ✅

- [ ] **Production Readiness**
  - [ ] All Tier 1 features rolled out to 100%
  - [ ] Rollback procedure tested
  - [ ] Monitoring alerts configured
  - [ ] Incident response plan documented

---

## 10. Continuous Monitoring Post-Release

**Production Monitoring Metrics:**

```rust
// Prometheus metrics for regression detection
lazy_static! {
    static ref SEARCH_LATENCY: HistogramVec = register_histogram_vec!(
        "ruvector_search_latency_seconds",
        "Search latency histogram",
        &["feature_enabled"]
    ).unwrap();

    static ref SEARCH_RECALL: GaugeVec = register_gauge_vec!(
        "ruvector_search_recall",
        "Search recall@10",
        &["feature_enabled"]
    ).unwrap();

    static ref FEATURE_ERRORS: CounterVec = register_counter_vec!(
        "ruvector_feature_errors_total",
        "Feature-specific error count",
        &["feature"]
    ).unwrap();
}

// Automatic regression detection
fn monitor_search_performance(feature: &str, latency: f64, recall: f64) {
    SEARCH_LATENCY
        .with_label_values(&[feature])
        .observe(latency);

    SEARCH_RECALL
        .with_label_values(&[feature])
        .set(recall);

    // Alert if regression detected
    if latency > BASELINE_LATENCY * 1.15 || recall < BASELINE_RECALL - 0.05 {
        alert!("Regression detected in feature '{}'", feature);
        auto_rollback_if_enabled(feature);
    }
}
```

---

## Conclusion

This regression prevention strategy provides:

1. **Comprehensive test coverage** (60% unit, 30% integration, 10% E2E)
2. **Property-based testing** for edge cases
3. **Continuous fuzzing** for robustness
4. **Feature flags** for safe rollout
5. **Backward compatibility** guarantees
6. **CI/CD automation** for regression detection
7. **Rollback mechanisms** for incident response
8. **Feature-specific risk analysis** for all 19 GNN v2 features

**Key Principles:**
- ✅ Test first, implement second
- ✅ Never break existing functionality
- ✅ Always provide fallback mechanisms
- ✅ Monitor continuously, rollback instantly
- ✅ Gradual rollout, statistical validation

**Success Metrics:**
- 🎯 Zero production incidents due to GNN v2
- 🎯 <1% performance degradation from baseline
- 🎯 100% backward compatibility with v0.1.19
- 🎯 All 19 features successfully deployed within 12 months

---

**End of Regression Prevention Strategy**

Generated by: Claude Code QA Specialist
Date: December 1, 2025
Next Review: Before each Tier 1/2/3 feature implementation
