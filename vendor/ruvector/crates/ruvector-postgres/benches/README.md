# RuVector Benchmark Suite

Comprehensive benchmarks comparing ruvector vs pgvector across multiple dimensions.

## Overview

This benchmark suite provides:

1. **Rust Benchmarks** - Low-level performance testing using Criterion
2. **SQL Benchmarks** - Realistic PostgreSQL workload testing
3. **Automated CI** - GitHub Actions workflow for continuous benchmarking

## Quick Start

### Run All Benchmarks

```bash
cd crates/ruvector-postgres
bash benches/scripts/run_benchmarks.sh
```

### Run Individual Benchmarks

```bash
# Distance function benchmarks
cargo bench --bench distance_bench

# HNSW index benchmarks
cargo bench --bench index_bench

# Quantization benchmarks
cargo bench --bench quantization_bench

# Quantized distance benchmarks
cargo bench --bench quantized_distance_bench
```

### Run SQL Benchmarks

```bash
# Setup database
createdb ruvector_bench
psql -d ruvector_bench -c 'CREATE EXTENSION ruvector;'
psql -d ruvector_bench -c 'CREATE EXTENSION pgvector;'

# Quick benchmark (10k vectors)
psql -d ruvector_bench -f benches/sql/quick_benchmark.sql

# Full workload (1M vectors)
psql -d ruvector_bench -f benches/sql/benchmark_workload.sql
```

## Benchmark Categories

### 1. Distance Function Benchmarks (`distance_bench.rs`)

Tests distance calculation performance across different vector dimensions:

- **L2 (Euclidean) Distance**: Scalar vs SIMD implementations
- **Cosine Distance**: Normalized similarity measurement
- **Inner Product**: Dot product for maximum inner product search
- **Batch Operations**: Sequential vs parallel processing

**Dimensions tested**: 128, 384, 768, 1536, 3072

**Key metrics**:
- Single operation latency
- Throughput (ops/sec)
- SIMD speedup vs scalar

### 2. HNSW Index Benchmarks (`index_bench.rs`)

Tests Hierarchical Navigable Small World graph index:

#### Build Benchmarks
- Index construction time vs dataset size (1K, 10K, 100K, 1M vectors)
- Impact of `ef_construction` parameter (16, 32, 64, 128, 256)
- Impact of `M` parameter (8, 12, 16, 24, 32, 48)

#### Search Benchmarks
- Query latency vs dataset size
- Impact of `ef_search` parameter (10, 20, 40, 80, 160, 320)
- Impact of `k` (number of neighbors: 1, 5, 10, 20, 50, 100)

#### Recall Accuracy
- Recall@10 vs `ef_search` values
- Ground truth comparison

#### Memory Usage
- Index size vs dataset size
- Memory per vector overhead

**Dimensions tested**: 128, 384, 768, 1536

### 3. Quantization Benchmarks (`quantization_bench.rs`)

Tests vector compression and quantized search:

#### Scalar Quantization (SQ8)
- Encoding/decoding speed
- Distance calculation speedup
- Recall vs exact search
- Memory reduction (4x compression)

#### Binary Quantization
- Encoding speed
- Hamming distance calculation (SIMD)
- Massive compression (32x for f32)
- Re-ranking strategies

#### Product Quantization (PQ)
- ADC (Asymmetric Distance Computation)
- SIMD vs scalar lookup
- Configurable compression ratios

**Key metrics**:
- Speedup vs exact search
- Recall@10 accuracy
- Compression ratio
- Throughput improvement

### 4. SQL Workload Benchmarks

Realistic PostgreSQL scenarios:

#### Quick Benchmark (`quick_benchmark.sql`)
- 10,000 vectors, 768 dimensions
- Sequential scan baseline
- HNSW index build
- Index search performance
- Distance function comparisons

#### Full Workload (`benchmark_workload.sql`)
- 1,000,000 vectors, 1536 dimensions
- 1,000 queries for statistical significance
- P50, P99 latency measurements
- Memory usage analysis
- Recall accuracy testing
- ruvector vs pgvector comparison

## Understanding Results

### Criterion Output

```
Distance/euclidean/scalar/768
                        time:   [2.1234 µs 2.1456 µs 2.1678 µs]
                        thrpt: [354.23 Melem/s 357.89 Melem/s 361.55 Melem/s]
```

- **time**: Mean execution time with confidence intervals
- **thrpt**: Throughput (operations per second)

### Comparing Implementations

```bash
# Set baseline
cargo bench --bench distance_bench -- --save-baseline main

# Make changes, then compare
cargo bench --bench distance_bench -- --baseline main
```

### SQL Benchmark Interpretation

```sql
 p50_ms | p99_ms | avg_ms | min_ms | max_ms
--------+--------+--------+--------+--------
  0.856 |  1.234 |  0.912 |  0.654 |  2.456
```

- **p50**: Median latency (50th percentile)
- **p99**: 99th percentile latency (worst 1%)
- **avg**: Average latency
- **min/max**: Best and worst case

## Performance Targets

### Distance Functions

| Operation | Dimension | Target Throughput |
|-----------|-----------|-------------------|
| L2 (SIMD) | 768       | > 400 Mops/s     |
| L2 (SIMD) | 1536      | > 200 Mops/s     |
| Cosine    | 768       | > 300 Mops/s     |
| Inner Product | 768   | > 500 Mops/s     |

### HNSW Index

| Dataset Size | Build Time | Search Latency | Recall@10 |
|--------------|------------|----------------|-----------|
| 100K         | < 30s      | < 1ms          | > 0.95    |
| 1M           | < 5min     | < 2ms          | > 0.95    |
| 10M          | < 1hr      | < 5ms          | > 0.90    |

### Quantization

| Method  | Compression | Speedup | Recall@10 |
|---------|-------------|---------|-----------|
| SQ8     | 4x          | 2-3x    | > 0.95    |
| Binary  | 32x         | 10-20x  | > 0.85    |
| PQ(8)   | 16x         | 5-10x   | > 0.90    |

## Continuous Integration

The GitHub Actions workflow runs automatically on:

- Pull requests touching benchmark code
- Pushes to `main` and `develop` branches
- Manual workflow dispatch

Results are:
- Posted as PR comments
- Stored as artifacts (30 day retention)
- Tracked over time on main branch
- Compared against baseline

### Triggering Manual Runs

```bash
# From GitHub UI: Actions → Benchmarks → Run workflow

# Or using gh CLI
gh workflow run benchmarks.yml
```

### Enabling SQL Benchmarks in CI

SQL benchmarks are disabled by default (too slow). Enable via workflow dispatch:

```bash
gh workflow run benchmarks.yml -f run_sql_benchmarks=true
```

## Advanced Usage

### Profiling with Criterion

```bash
# Generate flamegraph
cargo bench --bench distance_bench -- --profile-time=5

# Output to specific format
cargo bench --bench distance_bench -- --output-format bencher
```

### Custom Benchmark Parameters

Edit benchmark files to adjust:

- Vector dimensions
- Dataset sizes
- Number of queries
- HNSW parameters (M, ef_construction, ef_search)
- Quantization settings

### Comparing with pgvector

Ensure pgvector is installed:

```bash
git clone https://github.com/pgvector/pgvector.git
cd pgvector
make
sudo make install
```

Then run SQL benchmarks for side-by-side comparison.

## Interpreting Regressions

### Performance Degradation Alert

If CI fails due to performance regression:

1. **Check the comparison**: Review the baseline vs current results
2. **Validate the change**: Ensure it's not due to measurement noise
3. **Profile the code**: Use flamegraphs to identify bottlenecks
4. **Consider trade-offs**: Sometimes correctness > speed

### Common Causes

- **SIMD disabled**: Check compiler flags
- **Debug build**: Ensure --release mode
- **Thermal throttling**: CPU overheating in CI
- **Cache effects**: Different data access patterns

## Contributing

When adding benchmarks:

1. Add to appropriate `*_bench.rs` file
2. Update this README
3. Ensure benchmarks complete in < 5 minutes
4. Use `black_box()` to prevent optimization
5. Test both small and large inputs

## Resources

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [HNSW Paper](https://arxiv.org/abs/1603.09320)
- [Product Quantization Paper](https://ieeexplore.ieee.org/document/5432202)
- [pgvector Repository](https://github.com/pgvector/pgvector)

## License

Same as ruvector project - MIT
