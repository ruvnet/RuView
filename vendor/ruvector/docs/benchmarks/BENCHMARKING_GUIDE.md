# Benchmarking Guide

This guide explains how to run, interpret, and contribute benchmarks for Ruvector.

## Table of Contents

1. [Running Benchmarks](#running-benchmarks)
2. [Benchmark Suite](#benchmark-suite)
3. [Interpreting Results](#interpreting-results)
4. [Performance Targets](#performance-targets)
5. [Comparison Methodology](#comparison-methodology)
6. [Contributing Benchmarks](#contributing-benchmarks)

## Running Benchmarks

### Quick Start

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench distance_metrics
cargo bench hnsw_search
cargo bench batch_operations

# With flamegraph profiling
cargo flamegraph --bench hnsw_search

# With criterion reports
cargo bench -- --save-baseline main
git checkout feature-branch
cargo bench -- --baseline main
```

### Benchmark Crates

```bash
# Core benchmarks
cd crates/ruvector-bench
cargo bench

# Comparison benchmarks
cargo run --release --bin comparison_benchmark

# Memory benchmarks
cargo run --release --bin memory_benchmark

# Latency benchmarks
cargo run --release --bin latency_benchmark
```

### SIMD Optimization

Enable SIMD for maximum performance:

```bash
RUSTFLAGS="-C target-cpu=native" cargo bench

# Or specific features
RUSTFLAGS="-C target-feature=+avx2,+fma" cargo bench
```

## Benchmark Suite

### 1. Distance Metrics Benchmark

**File**: `crates/ruvector-core/benches/distance_metrics.rs`

**What it measures**: Raw distance calculation performance

**Metrics**:
- Euclidean (L2) distance
- Cosine similarity
- Dot product
- Manhattan (L1) distance
- SIMD vs scalar implementations

**Run**:
```bash
cargo bench distance_metrics
```

**Expected results**:
```
euclidean_128d/simd       time:   [45.234 ns 45.456 ns 45.678 ns]
euclidean_128d/scalar     time:   [312.45 ns 315.23 ns 318.91 ns]
                                  ↑ 7x slower
cosine_128d/simd          time:   [52.123 ns 52.345 ns 52.567 ns]
dotproduct_128d/simd      time:   [38.901 ns 39.123 ns 39.345 ns]
```

### 2. HNSW Search Benchmark

**File**: `crates/ruvector-core/benches/hnsw_search.rs`

**What it measures**: End-to-end search performance

**Metrics**:
- Search latency (p50, p95, p99)
- Queries per second (QPS)
- Recall accuracy
- Different dataset sizes (1K, 10K, 100K, 1M vectors)
- Different ef_search values (50, 100, 200, 500)

**Run**:
```bash
cargo bench hnsw_search
```

**Expected results**:
```
search_1M_vectors_k10_ef100
                        time:   [845.23 µs 856.78 µs 868.45 µs]
                        thrpt:  [1,151 queries/s]
                        recall: [95.2%]

search_1M_vectors_k10_ef200
                        time:   [1.678 ms 1.689 ms 1.701 ms]
                        thrpt:  [587 queries/s]
                        recall: [98.7%]
```

### 3. Batch Operations Benchmark

**File**: `crates/ruvector-core/benches/batch_operations.rs`

**What it measures**: Throughput for bulk operations

**Metrics**:
- Batch insert throughput
- Parallel vs sequential inserts
- Different batch sizes (100, 1K, 10K)

**Run**:
```bash
cargo bench batch_operations
```

**Expected results**:
```
batch_insert_1000_parallel
                        time:   [45.234 ms 46.123 ms 47.012 ms]
                        thrpt:  [21,271 vectors/s]

batch_insert_1000_sequential
                        time:   [234.56 ms 238.91 ms 243.27 ms]
                        thrpt:  [4,111 vectors/s]
                        ↑ 5x slower
```

### 4. Quantization Benchmark

**File**: `crates/ruvector-core/benches/quantization_bench.rs`

**What it measures**: Quantization performance and accuracy

**Metrics**:
- Quantization time
- Dequantization time
- Distance calculation with quantized vectors
- Recall impact

**Run**:
```bash
cargo bench quantization
```

**Expected results**:
```
scalar_quantize_128d      time:   [234.56 ns 236.78 ns 239.01 ns]
product_quantize_128d     time:   [1.234 µs 1.245 µs 1.256 µs]

search_with_scalar_quant  time:   [678.90 µs 685.12 µs 691.34 µs]
                          recall: [97.3%]

search_with_product_quant time:   [523.45 µs 528.67 µs 533.89 µs]
                          recall: [92.8%]
```

### 5. Comprehensive Benchmark

**File**: `crates/ruvector-core/benches/comprehensive_bench.rs`

**What it measures**: End-to-end system performance

**Run**:
```bash
cargo bench comprehensive
```

## Interpreting Results

### Criterion Output

```
test_name               time:   [lower_bound mean upper_bound]
                        thrpt:  [throughput]
                        change: [% change from baseline]
```

**Example**:
```
search_100K_vectors     time:   [234.56 µs 238.91 µs 243.27 µs]
                        thrpt:  [4,111 queries/s]
                        change: [-5.2% -3.8% -2.1%] (faster)
```

**Interpretation**:
- Mean: 238.91 µs
- 95% confidence interval: [234.56 µs, 243.27 µs]
- Throughput: ~4,111 queries/second
- 3.8% faster than baseline

### Latency Percentiles

```bash
cargo run --release --bin latency_benchmark
```

**Output**:
```
Latency percentiles (100K queries):
  p50:  0.85 ms
  p90:  1.23 ms
  p95:  1.67 ms
  p99:  3.45 ms
  p999: 8.91 ms
```

**Interpretation**:
- 50% of queries complete in < 0.85ms
- 95% of queries complete in < 1.67ms
- 99% of queries complete in < 3.45ms

### Memory Usage

```bash
cargo run --release --bin memory_benchmark
```

**Output**:
```
Memory usage (1M vectors, 128D):
  Vectors (full):        512.0 MB
  Vectors (scalar):      128.0 MB (4x compression)
  HNSW graph:           640.0 MB
  Metadata:              50.0 MB
  ──────────────────────────────
  Total:                818.0 MB
```

## Performance Targets

### Search Latency

| Dataset | Target p50 | Target p95 | Target QPS |
|---------|-----------|-----------|-----------|
| 10K vectors | < 100 µs | < 200 µs | 10,000+ |
| 100K vectors | < 500 µs | < 1 ms | 2,000+ |
| 1M vectors | < 1 ms | < 2 ms | 1,000+ |
| 10M vectors | < 2 ms | < 5 ms | 500+ |

### Insert Throughput

| Operation | Target |
|-----------|--------|
| Single insert | 1,000+ ops/sec |
| Batch insert (1K) | 10,000+ vectors/sec |
| Batch insert (10K) | 50,000+ vectors/sec |

### Memory Efficiency

| Configuration | Target Memory per Vector |
|---------------|-------------------------|
| Full precision | 512 bytes (128D) |
| Scalar quant | 128 bytes (4x compression) |
| Product quant | 16-32 bytes (16-32x compression) |

### Recall Accuracy

| Configuration | Target Recall |
|---------------|---------------|
| ef_search=50 | 85%+ |
| ef_search=100 | 90%+ |
| ef_search=200 | 95%+ |
| ef_search=500 | 99%+ |

## Comparison Methodology

### Against FAISS

```bash
cargo run --release --bin comparison_benchmark -- --system faiss
```

**Metrics compared**:
- Search latency (same dataset, same k)
- Memory usage
- Build time
- Recall@10

**Example output**:
```
Benchmark: 1M vectors, 128D, k=10

                  Ruvector    FAISS       Speedup
────────────────────────────────────────────────
Build time        245s        312s        1.27x
Search (p50)      0.85ms      2.34ms      2.75x
Search (p95)      1.67ms      4.56ms      2.73x
Memory            818MB       1,245MB     1.52x
Recall@10         95.2%       95.8%       ~same
```

### Versioned Benchmarks

Track performance over time:

```bash
# Save baseline
git checkout v0.1.0
cargo bench -- --save-baseline v0.1.0

# Compare to new version
git checkout v0.2.0
cargo bench -- --baseline v0.1.0
```

## Contributing Benchmarks

### Adding a New Benchmark

1. Create benchmark file:
```rust
// crates/ruvector-core/benches/my_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ruvector_core::*;

fn my_benchmark(c: &mut Criterion) {
    let db = setup_test_db();

    c.bench_function("my_operation", |b| {
        b.iter(|| {
            // Operation to benchmark
            db.my_operation(black_box(&input))
        })
    });
}

criterion_group!(benches, my_benchmark);
criterion_main!(benches);
```

2. Register in `Cargo.toml`:
```toml
[[bench]]
name = "my_benchmark"
harness = false
```

3. Run and verify:
```bash
cargo bench my_benchmark
```

### Benchmark Best Practices

1. **Use `black_box`**: Prevent compiler optimizations
   ```rust
   b.iter(|| db.search(black_box(&query)))
   ```

2. **Measure what matters**: Focus on user-facing operations

3. **Realistic workloads**: Use representative data sizes

4. **Multiple iterations**: Criterion handles this automatically

5. **Isolate variables**: Benchmark one thing at a time

6. **Document context**: Explain what's being measured

7. **CI integration**: Run benchmarks in CI to catch regressions

### Profiling

```bash
# Flamegraph
cargo flamegraph --bench hnsw_search

# perf (Linux)
perf record -g cargo bench hnsw_search
perf report

# Cachegrind (memory profiling)
valgrind --tool=cachegrind cargo bench hnsw_search
```

## CI/CD Integration

### GitHub Actions

``yaml
- name: Run benchmarks
  run: |
    cargo bench --bench distance_metrics -- --save-baseline main

- name: Compare to baseline
  run: |
    cargo bench --bench distance_metrics -- --baseline main
```

### Performance Regression Detection

Fail CI if performance regresses > 5%:

```rust
// In benchmark code
let previous_mean = load_baseline("main");
let current_mean = measure_current();
let regression = (current_mean - previous_mean) / previous_mean;

assert!(regression < 0.05, "Performance regression > 5%");
```

## Resources

- [Criterion.rs documentation](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Benchmarking Rust programs](https://doc.rust-lang.org/cargo/commands/cargo-bench.html)
- [ANN-Benchmarks](http://ann-benchmarks.com/) - Standard vector search benchmarks

## Questions?

Open an issue: https://github.com/ruvnet/ruvector/issues
