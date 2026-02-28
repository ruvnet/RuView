# sevensense-benches

[![Crate](https://img.shields.io/badge/crates.io-sevensense--benches-orange.svg)](https://crates.io/crates/sevensense-benches)
[![Docs](https://img.shields.io/badge/docs-sevensense--benches-blue.svg)](https://docs.rs/sevensense-benches)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

> Comprehensive performance benchmarks for the 7sense bioacoustic platform.

**sevensense-benches** contains Criterion.rs benchmark suites that validate performance targets across all 7sense crates. From HNSW search speedup to embedding throughput, these benchmarks ensure the platform meets its ambitious performance goals.

## Features

- **HNSW Benchmarks**: Search, insert, and recall measurements
- **Embedding Benchmarks**: Inference throughput and latency
- **Clustering Benchmarks**: HDBSCAN performance at scale
- **API Benchmarks**: Request throughput and latency
- **Memory Profiling**: Memory usage analysis
- **Regression Detection**: Automatic performance regression alerts

## Benchmark Suites

| Suite | Description | Target Metrics |
|-------|-------------|----------------|
| `hnsw_benchmark` | Vector search performance | 150x speedup, <50ms p99 |
| `embedding_benchmark` | Neural inference | >100 segments/sec |
| `clustering_benchmark` | HDBSCAN clustering | O(n log n) scaling |
| `api_benchmark` | HTTP endpoint latency | <200ms identify, <50ms search |

## Installation

The benchmark crate is part of the workspace:

```bash
cd vibecast
cargo bench -p sevensense-benches
```

## Running Benchmarks

### All Benchmarks

```bash
# Run all benchmark suites
cargo bench -p sevensense-benches

# Generate HTML report
cargo bench -p sevensense-benches -- --save-baseline main
```

### Specific Suite

```bash
# HNSW benchmarks only
cargo bench -p sevensense-benches --bench hnsw_benchmark

# Embedding benchmarks only
cargo bench -p sevensense-benches --bench embedding_benchmark

# Clustering benchmarks only
cargo bench -p sevensense-benches --bench clustering_benchmark

# API benchmarks only
cargo bench -p sevensense-benches --bench api_benchmark
```

### Specific Benchmark

```bash
# Run only HNSW search benchmarks
cargo bench -p sevensense-benches --bench hnsw_benchmark -- "hnsw_search"

# Run benchmarks matching a pattern
cargo bench -p sevensense-benches -- "search"
```

---

<details>
<summary><b>Tutorial: HNSW Benchmarks</b></summary>

### Benchmark Configuration

The HNSW benchmark suite tests various configurations:

```rust
// Index sizes tested
const SMALL_INDEX: usize = 10_000;
const MEDIUM_INDEX: usize = 100_000;
const LARGE_INDEX: usize = 500_000;

// K values for search
const K_VALUES: &[usize] = &[10, 50, 100];
```

### Running Search Benchmarks

```bash
# Search performance at different scales
cargo bench -p sevensense-benches --bench hnsw_benchmark -- "hnsw_search"

# Sample output:
# hnsw_search/small/k10    time:   [0.45 ms 0.47 ms 0.49 ms]
# hnsw_search/medium/k10   time:   [1.89 ms 1.94 ms 1.99 ms]
# hnsw_search/large/k10    time:   [5.23 ms 5.41 ms 5.58 ms]
```

### Measuring Speedup

```bash
# Compare HNSW vs brute force
cargo bench -p sevensense-benches --bench hnsw_benchmark -- "speedup"

# Expected output:
# HNSW (100K):  2.1 ms
# Brute force:  315 ms
# Speedup:      150x ✓
```

### Recall Measurement

```bash
# Measure recall@k accuracy
cargo bench -p sevensense-benches --bench hnsw_benchmark -- "recall"

# Expected:
# recall@10:  0.97 (target: ≥0.95) ✓
# recall@50:  0.98 (target: ≥0.95) ✓
# recall@100: 0.99 (target: ≥0.98) ✓
```

</details>

<details>
<summary><b>Tutorial: Embedding Benchmarks</b></summary>

### Inference Throughput

```bash
# Measure embedding generation speed
cargo bench -p sevensense-benches --bench embedding_benchmark -- "inference"

# Sample output:
# single_inference      time:   [14.2 ms 14.8 ms 15.4 ms]
# batch_inference/32    time:   [112 ms 118 ms 124 ms]
#                       thrpt:  [258/s 271/s 285/s]
```

### Mel Spectrogram Performance

```bash
# Mel computation benchmarks
cargo bench -p sevensense-benches --bench embedding_benchmark -- "mel"

# Expected:
# mel_compute/5s_segment  time:   [12.3 ms 12.8 ms 13.4 ms]
#                         target: <20ms ✓
```

### Memory Usage

```bash
# Profile memory during embedding
cargo bench -p sevensense-benches --bench embedding_benchmark -- "memory"

# Output includes peak memory usage per batch size
```

</details>

<details>
<summary><b>Tutorial: Clustering Benchmarks</b></summary>

### HDBSCAN Performance

```bash
# HDBSCAN at different scales
cargo bench -p sevensense-benches --bench clustering_benchmark -- "hdbscan"

# Sample output:
# hdbscan/fit/500     time:   [45.2 ms 47.1 ms 49.3 ms]
# hdbscan/fit/1000    time:   [123 ms 128 ms 134 ms]
# hdbscan/fit/2000    time:   [412 ms 431 ms 452 ms]
```

### Cluster Assignment

```bash
# Assigning new points to existing clusters
cargo bench -p sevensense-benches --bench clustering_benchmark -- "assignment"

# Output:
# cluster_assignment/single  time:   [1.23 µs 1.28 µs 1.34 µs]
# cluster_assignment/batch   thrpt:  [780K/s 812K/s 845K/s]
```

### Silhouette Score

```bash
# Cluster quality metric computation
cargo bench -p sevensense-benches --bench clustering_benchmark -- "silhouette"
```

</details>

<details>
<summary><b>Tutorial: API Benchmarks</b></summary>

### Endpoint Latency

```bash
# API endpoint benchmarks
cargo bench -p sevensense-benches --bench api_benchmark -- "endpoint"

# Sample output:
# identify_endpoint     time:   [142 ms 148 ms 155 ms]
#                       target: <200ms ✓
# search_endpoint       time:   [32 ms 35 ms 38 ms]
#                       target: <50ms ✓
```

### Concurrent Requests

```bash
# Throughput under load
cargo bench -p sevensense-benches --bench api_benchmark -- "concurrent"

# Output shows requests/second at various concurrency levels
```

</details>

<details>
<summary><b>Tutorial: Regression Testing</b></summary>

### Setting Baseline

```bash
# Save current performance as baseline
cargo bench -p sevensense-benches -- --save-baseline main
```

### Comparing Against Baseline

```bash
# Compare current performance vs baseline
cargo bench -p sevensense-benches -- --baseline main

# Output shows regressions:
# hnsw_search/medium  time:   [+5.2% +7.1% +9.3%] (regression)
```

### CI Integration

```yaml
# .github/workflows/bench.yml
name: Benchmark
on: [push, pull_request]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run benchmarks
        run: cargo bench -p sevensense-benches -- --noplot
      - name: Check for regressions
        run: |
          cargo bench -p sevensense-benches -- --baseline main
          # Fail if >10% regression
```

</details>

---

## Performance Targets

| Metric | Target | How to Verify |
|--------|--------|---------------|
| HNSW Speedup | 150x vs brute force | `cargo bench -- "speedup"` |
| Search p99 | <50ms | `cargo bench -- "hnsw_search"` |
| Recall@10 | ≥0.95 | `cargo bench -- "recall"` |
| Embedding Throughput | >100/s | `cargo bench -- "inference"` |
| Identify Latency | <200ms | `cargo bench -- "identify"` |
| Search Latency | <50ms | `cargo bench -- "search_endpoint"` |

## Benchmark Utilities

The crate provides utilities for benchmark setup:

```rust
use sevensense_benches::{
    generate_random_vectors,
    generate_clustered_vectors,
    SimpleHnswIndex,
    PERCH_EMBEDDING_DIM,
};

// Generate test data
let vectors = generate_random_vectors(10000, PERCH_EMBEDDING_DIM);

// Generate clustered data
let clustered = generate_clustered_vectors(10000, PERCH_EMBEDDING_DIM, 50, 0.1);

// Simple HNSW for benchmarking
let mut index = SimpleHnswIndex::new(16, 200);
index.build(&vectors);
```

## Output Formats

```bash
# Default output (console)
cargo bench -p sevensense-benches

# JSON output
cargo bench -p sevensense-benches -- --format json > results.json

# Generate HTML report
cargo bench -p sevensense-benches -- --save-baseline results
open target/criterion/report/index.html
```

## License

MIT License - see [LICENSE](../../LICENSE) for details.
