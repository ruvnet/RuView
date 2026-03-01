# RuVector Nervous System - Comprehensive Test Plan

## Overview

This test plan defines performance targets, quality metrics, and verification strategies for the RuVector Nervous System. All tests are designed to ensure real-time performance, memory efficiency, and biological plausibility.

## 1. Worst-Case Latency Requirements

### Latency Targets

| Component | Target | P50 | P99 | P99.9 | Measurement Method |
|-----------|--------|-----|-----|-------|-------------------|
| **Event Bus** |
| Event publish | <10μs | <5μs | <15μs | <50μs | Criterion benchmark |
| Event delivery (bounded queue) | <5μs | <2μs | <8μs | <20μs | Criterion benchmark |
| Priority routing | <20μs | <10μs | <30μs | <100μs | Criterion benchmark |
| **HDC (Hyperdimensional Computing)** |
| Vector binding (XOR) | <100ns | <50ns | <150ns | <500ns | Criterion benchmark |
| Vector bundling (majority) | <500ns | <200ns | <1μs | <5μs | Criterion benchmark |
| Hamming distance | <100ns | <50ns | <150ns | <500ns | Criterion benchmark |
| Similarity check | <200ns | <100ns | <300ns | <1μs | Criterion benchmark |
| **WTA (Winner-Take-All)** |
| Single winner selection | <1μs | <500ns | <2μs | <10μs | Criterion benchmark |
| k-WTA (k=5) | <5μs | <2μs | <10μs | <50μs | Criterion benchmark |
| Lateral inhibition update | <10μs | <5μs | <20μs | <100μs | Criterion benchmark |
| **Hopfield Networks** |
| Pattern retrieval (100 patterns) | <1ms | <500μs | <2ms | <10ms | Criterion benchmark |
| Pattern storage | <100μs | <50μs | <200μs | <1ms | Criterion benchmark |
| Energy computation | <50μs | <20μs | <100μs | <500μs | Criterion benchmark |
| **Pattern Separation** |
| Encoding (orthogonalization) | <500μs | <200μs | <1ms | <5ms | Criterion benchmark |
| Collision detection | <100μs | <50μs | <200μs | <1ms | Criterion benchmark |
| Decorrelation | <200μs | <100μs | <500μs | <2ms | Criterion benchmark |
| **Plasticity** |
| E-prop gradient update | <100μs | <50μs | <200μs | <1ms | Criterion benchmark |
| BTSP eligibility trace | <50μs | <20μs | <100μs | <500μs | Criterion benchmark |
| EWC Fisher matrix update | <1ms | <500μs | <2ms | <10ms | Criterion benchmark |
| **Cognitum Integration** |
| Reflex event→action | <100μs | <50μs | <200μs | <1ms | Criterion benchmark |
| v0 adapter dispatch | <50μs | <20μs | <100μs | <500μs | Criterion benchmark |

### Benchmark Implementation

**Location**: `crates/ruvector-nervous-system/benches/latency_benchmarks.rs`

**Key Features**:
- Uses Criterion for statistical rigor
- Measures P50, P99, P99.9 percentiles
- Includes warm-up runs
- Tests under load (concurrent operations)
- Regression detection with baselines

## 2. Memory Bounds Verification

### Memory Targets

| Component | Target per Instance | Verification Method |
|-----------|-------------------|-------------------|
| **Plasticity** |
| E-prop synapse state | 8-12 bytes | `std::mem::size_of` |
| BTSP eligibility window | 32 bytes | `std::mem::size_of` |
| EWC Fisher matrix (per layer) | O(n²) sparse | Allocation tracking |
| **Event Bus** |
| Bounded queue entry | 16-24 bytes | `std::mem::size_of` |
| Regional shard overhead | <1KB | Allocation tracking |
| **HDC** |
| Hypervector (10K dims) | 1.25KB (bit-packed) | Direct calculation |
| Encoding cache | <100KB | Memory profiler |
| **Hopfield** |
| Weight matrix (1000 neurons) | ~4MB (f32) or ~1MB (f16) | Direct calculation |
| Pattern storage | O(n×d) | Allocation tracking |
| **Workspace** |
| Global workspace capacity | 4-7 items × vector size | Capacity test |
| Coherence gating state | <1KB | `std::mem::size_of` |

### Verification Strategy

**Location**: `crates/ruvector-nervous-system/tests/memory_bounds.rs`

**Methods**:
1. **Compile-time checks**: `static_assert` for structure sizes
2. **Runtime verification**: Allocation tracking with custom allocator
3. **Stress tests**: Create maximum capacity scenarios
4. **Leak detection**: Valgrind/MIRI integration

**Example**:
```rust
#[test]
fn verify_eprop_synapse_size() {
    assert!(std::mem::size_of::<EPropSynapse>() <= 12);
}

#[test]
fn btsp_window_bounded() {
    let btsp = BTSPLearner::new(1000, 0.01, 100);
    let initial_mem = get_allocated_bytes();
    btsp.train_episodes(1000);
    let final_mem = get_allocated_bytes();
    assert!(final_mem - initial_mem < 100_000); // <100KB growth
}
```

## 3. Retrieval Quality Benchmarks

### Quality Metrics

| Metric | Target | Baseline Comparison | Test Method |
|--------|--------|-------------------|-------------|
| **HDC Recall** |
| Recall@1 vs HNSW | ≥95% of HNSW | Compare on same dataset | Synthetic corpus |
| Recall@10 vs HNSW | ≥90% of HNSW | Compare on same dataset | Synthetic corpus |
| Noise robustness (20% flip) | >80% accuracy | N/A | Bit-flip test |
| **Hopfield Capacity** |
| Pattern capacity (d=512) | ≥2^(d/2) = 2^256 patterns | Theoretical limit | Stress test |
| Retrieval accuracy (0.1 noise) | >95% | N/A | Noisy retrieval |
| **Pattern Separation** |
| Collision rate | <1% for 10K patterns | Random encoding | Synthetic corpus |
| Orthogonality score | >0.9 cosine distance | N/A | Correlation test |
| **Associative Memory** |
| One-shot learning accuracy | >90% | N/A | Single-shot test |
| Multi-pattern interference | <5% accuracy drop | Isolated patterns | Capacity test |

### Test Implementation

**Location**: `crates/ruvector-nervous-system/tests/retrieval_quality.rs`

**Datasets**:
1. **Synthetic**: Controlled distributions (uniform, gaussian, clustered)
2. **Real-world proxy**: MNIST embeddings, SIFT features
3. **Adversarial**: Designed to stress collision detection

**Comparison Baselines**:
- HNSW index (via ruvector-core)
- Exact k-NN (brute force)
- Theoretical limits (Hopfield capacity)

**Example**:
```rust
#[test]
fn hdc_recall_vs_hnsw() {
    let vectors: Vec<Vec<f32>> = generate_synthetic_dataset(10000, 512);
    let queries: Vec<Vec<f32>> = &vectors[0..100];

    // HDC results
    let hdc = HDCIndex::new(512, 10000);
    for (i, v) in vectors.iter().enumerate() {
        hdc.encode_and_store(i, v);
    }
    let hdc_results = queries.iter().map(|q| hdc.search(q, 10)).collect();

    // HNSW results (ground truth)
    let hnsw = HNSWIndex::new(512);
    for (i, v) in vectors.iter().enumerate() {
        hnsw.insert(i, v);
    }
    let hnsw_results = queries.iter().map(|q| hnsw.search(q, 10)).collect();

    // Compare recall
    let recall = calculate_recall(&hdc_results, &hnsw_results);
    assert!(recall >= 0.90, "HDC recall@10 {} < 90% of HNSW", recall);
}
```

## 4. Throughput Benchmarks

### Throughput Targets

| Component | Target | Measurement Condition | Test Method |
|-----------|--------|---------------------|-------------|
| **Event Bus** |
| Event throughput | >10,000 events/ms | Sustained load | Load generator |
| Multi-producer scaling | Linear to 8 cores | Concurrent publishers | Parallel bench |
| Backpressure handling | Graceful degradation | Queue saturation | Stress test |
| **Plasticity** |
| Consolidation replay | >100 samples/sec | Batch processing | Batch timer |
| Meta-learning update | >50 tasks/sec | Task distribution | Task timer |
| **HDC** |
| Encoding throughput | >1M ops/sec | Batch encoding | Throughput bench |
| Similarity checks | >10M ops/sec | SIMD acceleration | Throughput bench |
| **Hopfield** |
| Parallel retrieval | >1000 queries/sec | Batch queries | Throughput bench |

### Sustained Load Tests

**Location**: `crates/ruvector-nervous-system/tests/throughput.rs`

**Duration**: Minimum 60 seconds per test
**Metrics**:
- Operations per second (mean, min, max)
- Latency distribution under load
- CPU utilization
- Memory growth rate

**Example**:
```rust
#[test]
fn event_bus_sustained_throughput() {
    let bus = EventBus::new(1000);
    let start = Instant::now();
    let duration = Duration::from_secs(60);
    let mut count = 0u64;

    while start.elapsed() < duration {
        bus.publish(Event::new("test", vec![0.0; 128]));
        count += 1;
    }

    let events_per_sec = count as f64 / duration.as_secs_f64();
    assert!(events_per_sec > 10_000.0,
            "Event bus throughput {} < 10K/sec", events_per_sec);
}
```

## 5. Integration Tests

### End-to-End Scenarios

**Location**: `crates/ruvector-nervous-system/tests/integration.rs`

| Scenario | Components Tested | Success Criteria |
|----------|------------------|-----------------|
| **DVS Event Processing** | EventBus → HDC → WTA → Hopfield | <1ms end-to-end latency |
| **Associative Recall** | Hopfield → PatternSeparation → EventBus | >95% retrieval accuracy |
| **Adaptive Learning** | BTSP → E-prop → EWC → Memory | Positive transfer, <10% catastrophic forgetting |
| **Cognitive Routing** | Workspace → Coherence → Attention | Correct priority selection |
| **Reflex Arc** | Cognitum → EventBus → WTA → Action | <100μs reflex latency |

### Integration Test Structure

```rust
#[test]
fn test_dvs_to_classification_pipeline() {
    // Setup
    let event_bus = EventBus::new(1000);
    let hdc_encoder = HDCEncoder::new(10000);
    let wta = WTALayer::new(100, 0.5, 0.1);
    let hopfield = ModernHopfield::new(512, 100.0);

    // Train on patterns
    for (label, events) in training_data {
        let hv = hdc_encoder.encode_events(&events);
        let sparse = wta.compete(&hv);
        hopfield.store_labeled(label, &sparse);
    }

    // Test retrieval
    let test_events = generate_test_dvs_stream();
    let start = Instant::now();
    let hv = hdc_encoder.encode_events(&test_events);
    let sparse = wta.compete(&hv);
    let retrieved = hopfield.retrieve(&sparse);
    let latency = start.elapsed();

    // Verify
    assert!(latency < Duration::from_millis(1), "Latency {} > 1ms", latency.as_micros());
    assert!(retrieved.accuracy > 0.95, "Accuracy {} < 95%", retrieved.accuracy);
}
```

## 6. Property-Based Testing

### Invariants to Verify

**Location**: Uses `proptest` crate throughout test suite

| Property | Component | Verification |
|----------|-----------|--------------|
| **HDC** |
| Binding commutativity | `bind(a, b) == bind(b, a)` | Property test |
| Bundling associativity | `bundle([a, b, c]) invariant to order` | Property test |
| Distance symmetry | `distance(a, b) == distance(b, a)` | Property test |
| **Hopfield** |
| Energy monotonic decrease | Energy never increases during retrieval | Property test |
| Fixed point stability | Stored patterns are attractors | Property test |
| **Pattern Separation** |
| Collision bound | Collision rate < theoretical bound | Statistical test |
| Reversibility | `decode(encode(x))` approximates `x` | Property test |

**Example**:
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn hopfield_energy_decreases(
        pattern in prop::collection::vec(prop::num::f32::NORMAL, 512)
    ) {
        let mut hopfield = ModernHopfield::new(512, 100.0);
        hopfield.store(pattern.clone());

        let mut state = add_noise(&pattern, 0.2);
        let mut prev_energy = hopfield.energy(&state);

        for _ in 0..10 {
            state = hopfield.update(&state);
            let curr_energy = hopfield.energy(&state);
            prop_assert!(curr_energy <= prev_energy,
                        "Energy increased: {} -> {}", prev_energy, curr_energy);
            prev_energy = curr_energy;
        }
    }
}

proptest! {
    #[test]
    fn hdc_binding_commutative(
        a in hypervector_strategy(),
        b in hypervector_strategy()
    ) {
        let ab = a.bind(&b);
        let ba = b.bind(&a);
        prop_assert_eq!(ab, ba, "Binding not commutative");
    }
}
```

## 7. Performance Regression Detection

### Baseline Storage

**Location**: `crates/ruvector-nervous-system/benches/baselines/`

**Format**: JSON files with historical results
```json
{
  "benchmark": "hopfield_retrieve_1000_patterns",
  "date": "2025-12-28",
  "commit": "abc123",
  "mean": 874.3,
  "std_dev": 12.1,
  "p99": 920.5
}
```

### CI Integration

**GitHub Actions Workflow**:
```yaml
name: Performance Regression Check

on: [pull_request]

jobs:
  bench:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run benchmarks
        run: cargo bench --bench latency_benchmarks -- --save-baseline pr
      - name: Compare to main
        run: |
          git checkout main
          cargo bench --bench latency_benchmarks -- --save-baseline main
          cargo bench --bench latency_benchmarks -- --baseline pr --load-baseline main
      - name: Check thresholds
        run: |
          python scripts/check_regression.py --threshold 1.10 # 10% regression limit
```

### Threshold-Based Pass/Fail

| Metric | Warning Threshold | Failure Threshold |
|--------|------------------|------------------|
| Latency increase | +5% | +10% |
| Throughput decrease | -5% | -10% |
| Memory increase | +10% | +20% |
| Accuracy decrease | -2% | -5% |

## 8. Test Execution Matrix

### Local Development

```bash
# Unit tests
cargo test -p ruvector-nervous-system

# Integration tests
cargo test -p ruvector-nervous-system --test integration

# All benchmarks
cargo bench -p ruvector-nervous-system

# Specific benchmark
cargo bench -p ruvector-nervous-system --bench latency_benchmarks

# With profiling
cargo bench -p ruvector-nervous-system -- --profile-time=10

# Memory bounds check
cargo test -p ruvector-nervous-system --test memory_bounds -- --nocapture
```

### CI Pipeline

| Stage | Tests Run | Success Criteria |
|-------|-----------|-----------------|
| **PR Check** | Unit + Integration | 100% pass |
| **Nightly** | Full benchmark suite | No >10% regressions |
| **Release** | Full suite + extended stress | All thresholds met |

### Platform Coverage

- **Linux x86_64**: Primary target (all tests)
- **Linux ARM64**: Throughput + latency (may differ)
- **macOS**: Compatibility check
- **Windows**: Compatibility check

## 9. Test Data Management

### Synthetic Data Generation

**Location**: `crates/ruvector-nervous-system/tests/data/generators.rs`

- **Uniform random**: `generate_uniform(n, d)`
- **Gaussian clusters**: `generate_clusters(n, k, d, sigma)`
- **Temporal sequences**: `generate_spike_trains(n, duration, rate)`
- **Adversarial**: `generate_collisions(n, d, target_rate)`

### Reproducibility

- All tests use fixed seeds: `rand::SeedableRng::seed_from_u64(42)`
- Snapshot testing for golden outputs
- Version-controlled test vectors

## 10. Documentation and Reporting

### Test Reports

**Generated artifacts**:
- `target/criterion/`: HTML benchmark reports
- `target/coverage/`: Code coverage (via `cargo tarpaulin`)
- `target/flamegraph/`: Performance profiles

### Coverage Targets

| Category | Target |
|----------|--------|
| Line coverage | >85% |
| Branch coverage | >75% |
| Function coverage | >90% |

### Continuous Monitoring

- **Benchmark dashboard**: Track trends over time
- **Alerting**: Slack/email on regression detection
- **Historical comparison**: Compare across releases

---

## Appendix: Test Checklist

### Pre-Release Verification

- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] All benchmarks meet latency targets (P99)
- [ ] Memory bounds verified
- [ ] Retrieval quality ≥95% of baseline
- [ ] Throughput targets met under sustained load
- [ ] No performance regressions >5%
- [ ] Property tests pass (10K iterations)
- [ ] Coverage ≥85%
- [ ] Documentation updated
- [ ] CHANGELOG entries added

### Test Maintenance

- [ ] Review and update baselines quarterly
- [ ] Add tests for each new feature
- [ ] Refactor slow tests
- [ ] Archive obsolete benchmarks
- [ ] Update thresholds based on hardware improvements

---

**Version**: 1.0
**Last Updated**: 2025-12-28
**Maintainer**: RuVector Nervous System Team
