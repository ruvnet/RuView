# Ruvector-Bench

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)

**Comprehensive benchmarking suite for measuring Ruvector performance across different operations and configurations.**

> Professional-grade performance testing tools for validating sub-millisecond vector search, HNSW optimization, quantization efficiency, and cross-system comparisons. Built for developers who demand data-driven insights.

## 🎯 Overview

The `ruvector-bench` crate provides a complete benchmarking infrastructure to measure and analyze Ruvector's performance characteristics. It includes standardized test suites compatible with [ann-benchmarks.com](http://ann-benchmarks.com), comprehensive latency profiling, memory usage analysis, and cross-system performance comparison tools.

### Key Features

- ⚡ **ANN-Benchmarks Compatible**: Standard datasets (SIFT1M, GIST1M, Deep1M) and metrics
- 📊 **Latency Profiling**: High-precision measurement of p50, p95, p99, p99.9 percentiles
- 💾 **Memory Analysis**: Track memory usage with quantization and optimization techniques
- 🔬 **AgenticDB Workloads**: Simulate real-world AI agent memory patterns
- 🏆 **Cross-System Comparison**: Compare against Python baselines and other vector databases
- 📈 **Comprehensive Reporting**: JSON, CSV, and Markdown output formats
- 🔥 **Performance Profiling**: CPU flamegraphs and memory profiling support

## 📦 Installation

Add to your `Cargo.toml`:

```toml
[dev-dependencies]
ruvector-bench = { path = "../ruvector-bench" }

# Optional: Enable profiling features
ruvector-bench = { path = "../ruvector-bench", features = ["profiling"] }

# Optional: Enable HDF5 dataset loading
ruvector-bench = { path = "../ruvector-bench", features = ["hdf5-datasets"] }
```

## 🚀 Available Benchmarks

The suite includes 6 specialized benchmark binaries:

| Benchmark | Purpose | Metrics |
|-----------|---------|---------|
| **ann-benchmark** | ANN-Benchmarks compatibility | QPS, latency, recall@k, memory |
| **agenticdb-benchmark** | AI agent memory workloads | Insert/search/update latency, memory |
| **latency-benchmark** | Detailed latency profiling | p50/p95/p99/p99.9 latencies |
| **memory-benchmark** | Memory usage analysis | Memory per vector, quantization savings |
| **comparison-benchmark** | Cross-system performance | Ruvector vs baselines (10-100x faster) |
| **profiling-benchmark** | CPU/memory profiling | Flamegraphs, allocation tracking |

## ⚡ Quick Start

### Running Basic Benchmarks

```bash
# Run ANN-Benchmarks suite with default settings
cargo run --bin ann-benchmark --release

# Run with custom parameters
cargo run --bin ann-benchmark --release -- \
  --num-vectors 100000 \
  --dimensions 384 \
  --ef-search-values 50,100,200 \
  --output bench_results

# Run latency profiling
cargo run --bin latency-benchmark --release

# Run AgenticDB workload simulation
cargo run --bin agenticdb-benchmark --release

# Run cross-system comparison
cargo run --bin comparison-benchmark --release
```

### Running with Profiling

```bash
# Build with profiling enabled
cargo build --bin profiling-benchmark --release --features profiling

# Run and generate flamegraph
cargo run --bin profiling-benchmark --release --features profiling -- \
  --enable-flamegraph \
  --output profiling_results
```

## 📊 Benchmark Categories

### 1. ANN-Benchmarks Suite (`ann-benchmark`)

Standard benchmarking compatible with [ann-benchmarks.com](http://ann-benchmarks.com) methodology.

**Supported Datasets:**
- **SIFT1M**: 1M vectors, 128 dimensions (image descriptors)
- **GIST1M**: 1M vectors, 960 dimensions (scene recognition)
- **Deep1M**: 1M vectors, 96 dimensions (deep learning embeddings)
- **Synthetic**: Configurable size and distribution

**Usage:**

```bash
# Test with synthetic data (default)
cargo run --bin ann-benchmark --release -- \
  --dataset synthetic \
  --num-vectors 100000 \
  --dimensions 384 \
  --k 10

# Test with SIFT1M (requires dataset download)
cargo run --bin ann-benchmark --release -- \
  --dataset sift1m \
  --ef-search-values 50,100,200,400
```

**Measured Metrics:**
- Queries per second (QPS)
- Latency percentiles (p50, p95, p99, p99.9)
- Recall@1, Recall@10, Recall@100
- Memory usage (MB)
- Build/index time

**Example Output:**

```
╔════════════════════════════════════════╗
║   Ruvector ANN-Benchmarks Suite       ║
╚════════════════════════════════════════╝

✓ Dataset loaded: 100000 vectors, 1000 queries

============================================================
Testing with ef_search = 100
============================================================

┌───────────┬──────┬──────────┬──────────┬───────────┬─────────────┐
│ ef_search │ QPS  │ p50 (ms) │ p99 (ms) │ Recall@10 │ Memory (MB) │
├───────────┼──────┼──────────┼──────────┼───────────┼─────────────┤
│ 100       │ 5243 │ 0.19     │ 0.45     │ 95.23%    │ 246.8       │
└───────────┴──────┴──────────┴──────────┴───────────┴─────────────┘
```

### 2. AgenticDB Workload Simulation (`agenticdb-benchmark`)

Simulates real-world AI agent memory patterns with mixed read/write workloads.

**Workload Types:**
- **Conversational AI**: High read ratio (70/30 read/write)
- **Learning Agents**: Balanced read/write (50/50)
- **Batch Processing**: Write-heavy (30/70 read/write)

**Usage:**

```bash
cargo run --bin agenticdb-benchmark --release -- \
  --workload conversational \
  --num-vectors 50000 \
  --num-operations 10000
```

**Measured Operations:**
- Insert latency
- Search latency
- Update latency
- Batch operation throughput
- Memory efficiency

### 3. Latency Profiling (`latency-benchmark`)

Detailed latency analysis across different configurations and concurrency levels.

**Test Scenarios:**
- Single-threaded vs multi-threaded search
- Effect of `ef_search` parameter on latency
- Effect of quantization on latency/recall tradeoff
- Concurrent query handling

**Usage:**

```bash
# Test with different thread counts
cargo run --bin latency-benchmark --release -- \
  --threads 1,4,8,16 \
  --num-vectors 50000 \
  --queries 1000
```

**Example Output:**

```
Test 1: Single-threaded Latency
- p50: 0.42ms
- p95: 1.23ms
- p99: 2.15ms
- p99.9: 4.87ms

Test 2: Multi-threaded Latency (8 threads)
- p50: 0.38ms
- p95: 1.05ms
- p99: 1.89ms
- p99.9: 3.92ms
```

### 4. Memory Benchmarks (`memory-benchmark`)

Analyzes memory usage with different quantization strategies.

**Quantization Tests:**
- **None**: Full precision (baseline)
- **Scalar**: 4x compression
- **Binary**: 32x compression

**Usage:**

```bash
cargo run --bin memory-benchmark --release -- \
  --num-vectors 100000 \
  --dimensions 384
```

**Measured Metrics:**
- Memory per vector (bytes)
- Compression ratio
- Memory overhead
- Quantization impact on recall

**Example Results:**

```
┌──────────────┬─────────────┬───────────────┬────────────┐
│ Quantization │ Memory (MB) │ Bytes/Vector  │ Recall@10  │
├──────────────┼─────────────┼───────────────┼────────────┤
│ None         │ 147.5       │ 1536          │ 100.00%    │
│ Scalar       │ 38.2        │ 398           │ 95.80%     │
│ Binary       │ 4.7         │ 49            │ 87.20%     │
└──────────────┴─────────────┴───────────────┴────────────┘

✓ Scalar quantization: 4.0x memory reduction, 4.2% recall loss
✓ Binary quantization: 31.4x memory reduction, 12.8% recall loss
```

### 5. Cross-System Comparison (`comparison-benchmark`)

Compare Ruvector against other implementations and baselines.

**Comparison Targets:**
- Ruvector (optimized: SIMD + Quantization + HNSW)
- Ruvector (no quantization)
- Simulated Python baseline (numpy)
- Simulated brute-force search

**Usage:**

```bash
cargo run --bin comparison-benchmark --release -- \
  --num-vectors 50000 \
  --dimensions 384
```

**Example Results:**

```
┌──────────────────────────┬──────┬──────────┬─────────────┬────────────┐
│ System                   │ QPS  │ p50 (ms) │ Memory (MB) │ Speedup    │
├──────────────────────────┼──────┼──────────┼─────────────┼────────────┤
│ Ruvector (optimized)     │ 5243 │ 0.19     │ 38.2        │ 1.0x       │
│ Ruvector (no quant)      │ 4891 │ 0.20     │ 147.5       │ 0.93x      │
│ Python baseline          │ 89   │ 11.2     │ 153.6       │ 58.9x      │
│ Brute-force              │ 12   │ 83.3     │ 147.5       │ 437x       │
└──────────────────────────┴──────┴──────────┴─────────────┴────────────┘

✓ Ruvector is 58.9x faster than Python baseline
✓ Ruvector uses 74.1% less memory with quantization
```

### 6. Performance Profiling (`profiling-benchmark`)

CPU and memory profiling with flamegraph generation (requires `profiling` feature).

**Usage:**

```bash
# Build with profiling support
cargo build --bin profiling-benchmark --release --features profiling

# Run with flamegraph generation
cargo run --bin profiling-benchmark --release --features profiling -- \
  --enable-flamegraph \
  --num-vectors 50000 \
  --output profiling_results

# View flamegraph
open profiling_results/flamegraph.svg
```

**Generated Artifacts:**
- CPU flamegraph (SVG)
- Memory allocation profile
- Hotspot analysis
- Function-level timing breakdown

## 📈 Interpreting Results

### Latency Metrics

| Percentile | Meaning | Target |
|------------|---------|--------|
| **p50** | Median latency - typical query performance | <0.5ms |
| **p95** | 95% of queries complete within this time | <1.5ms |
| **p99** | 99% of queries complete within this time | <3.0ms |
| **p99.9** | 99.9% of queries (tail latency) | <5.0ms |

### Recall Metrics

- **Recall@k**: Fraction of true nearest neighbors found in top-k results
- **Target Recall@10**: ≥95% for most applications
- **Trade-off**: Higher `ef_search` → better recall, higher latency

### Memory Efficiency

```
Memory per vector = Total Memory / Number of Vectors

Typical values:
- No quantization: ~1536 bytes (384D float32)
- Scalar quantization: ~400 bytes (4x compression)
- Binary quantization: ~50 bytes (32x compression)
```

## 🔧 Benchmark Configuration Options

### Common Options (All Benchmarks)

```bash
--num-vectors <N>       # Number of vectors to index (default: 50000)
--dimensions <D>        # Vector dimensions (default: 384)
--output <PATH>         # Output directory for results (default: bench_results)
```

### ANN-Benchmark Specific

```bash
--dataset <NAME>        # Dataset: sift1m, gist1m, deep1m, synthetic
--num-queries <N>       # Number of search queries (default: 1000)
--k <K>                 # Number of nearest neighbors to retrieve (default: 10)
--m <M>                 # HNSW M parameter (default: 32)
--ef-construction <EF>  # HNSW build parameter (default: 200)
--ef-search-values <EF> # Comma-separated ef_search values to test (default: 50,100,200,400)
--metric <METRIC>       # Distance metric: cosine, euclidean, dot (default: cosine)
--quantization <TYPE>   # Quantization: none, scalar, binary (default: scalar)
```

### Latency-Benchmark Specific

```bash
--threads <THREADS>     # Comma-separated thread counts (default: 1,4,8,16)
```

### AgenticDB-Benchmark Specific

```bash
--workload <TYPE>       # Workload type: conversational, learning, batch
--num-operations <N>    # Number of operations to perform (default: 10000)
```

### Profiling-Benchmark Specific

```bash
--enable-flamegraph     # Generate CPU flamegraph (requires profiling feature)
--enable-memory-profile # Enable detailed memory profiling
```

## 🎨 Custom Benchmark Creation

Create your own benchmarks using the `ruvector-bench` library:

```rust
use ruvector_bench::{
    BenchmarkResult, DatasetGenerator, LatencyStats,
    MemoryProfiler, ResultWriter, VectorDistribution,
};
use ruvector_core::{VectorDB, DbOptions, SearchQuery, VectorEntry};
use std::time::Instant;

fn my_custom_benchmark() -> anyhow::Result<()> {
    // Generate test data
    let gen = DatasetGenerator::new(384, VectorDistribution::Normal {
        mean: 0.0,
        std_dev: 1.0,
    });
    let vectors = gen.generate(10000);
    let queries = gen.generate(100);

    // Create database
    let db = VectorDB::new(DbOptions::default())?;

    // Measure indexing
    let mem_profiler = MemoryProfiler::new();
    let build_start = Instant::now();

    for (idx, vector) in vectors.iter().enumerate() {
        db.insert(VectorEntry {
            id: Some(idx.to_string()),
            vector: vector.clone(),
            metadata: None,
        })?;
    }

    let build_time = build_start.elapsed();

    // Measure search performance
    let mut latency_stats = LatencyStats::new()?;

    for query in &queries {
        let start = Instant::now();
        db.search(SearchQuery {
            vector: query.clone(),
            k: 10,
            filter: None,
            ef_search: None,
        })?;
        latency_stats.record(start.elapsed())?;
    }

    // Print results
    println!("Build time: {:.2}s", build_time.as_secs_f64());
    println!("p50 latency: {:.2}ms", latency_stats.percentile(0.50).as_secs_f64() * 1000.0);
    println!("p99 latency: {:.2}ms", latency_stats.percentile(0.99).as_secs_f64() * 1000.0);
    println!("Memory usage: {:.2}MB", mem_profiler.current_usage_mb());

    Ok(())
}
```

## 🔄 CI/CD Integration

### GitHub Actions Example

```yaml
name: Benchmarks

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          profile: minimal

      - name: Run benchmarks
        run: |
          cd crates/ruvector-bench
          cargo run --bin ann-benchmark --release -- --output ci_results
          cargo run --bin latency-benchmark --release -- --output ci_results

      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results
          path: crates/ruvector-bench/ci_results/

      - name: Check performance regression
        run: |
          python scripts/check_regression.py ci_results/ann_benchmark.json
```

## 📉 Performance Regression Testing

Track performance over time using historical benchmark data:

```bash
# Run baseline benchmarks (on main branch)
git checkout main
cargo run --bin ann-benchmark --release -- --output baseline_results

# Run comparison benchmarks (on feature branch)
git checkout feature-branch
cargo run --bin ann-benchmark --release -- --output feature_results

# Compare results
python scripts/compare_benchmarks.py \
  baseline_results/ann_benchmark.json \
  feature_results/ann_benchmark.json
```

**Regression Thresholds:**
- ✅ **Pass**: <5% latency regression, <10% memory regression
- ⚠️ **Warning**: 5-10% latency regression, 10-20% memory regression
- ❌ **Fail**: >10% latency regression, >20% memory regression

## 📊 Results Visualization

Benchmark results are automatically saved in multiple formats:

### JSON Format

```json
{
  "name": "ruvector-ef100",
  "dataset": "synthetic",
  "dimensions": 384,
  "num_vectors": 100000,
  "qps": 5243.2,
  "latency_p50": 0.19,
  "latency_p99": 2.15,
  "recall_at_10": 0.9523,
  "memory_mb": 38.2
}
```

### CSV Format

```csv
name,dataset,dimensions,num_vectors,qps,p50,p99,recall@10,memory_mb
ruvector-ef100,synthetic,384,100000,5243.2,0.19,2.15,0.9523,38.2
```

### Markdown Report

Results include automatically generated markdown reports with detailed performance analysis.

### Custom Visualization

Generate performance charts using the provided data:

```python
import pandas as pd
import matplotlib.pyplot as plt

# Load benchmark results
df = pd.read_csv('bench_results/ann_benchmark.csv')

# Plot QPS vs Recall tradeoff
plt.figure(figsize=(10, 6))
plt.scatter(df['recall@10'] * 100, df['qps'])
plt.xlabel('Recall@10 (%)')
plt.ylabel('Queries per Second')
plt.title('Ruvector Performance: QPS vs Recall')
plt.grid(True)
plt.savefig('qps_vs_recall.png')
```

## 🔗 Links to Benchmark Reports

- [Latest Benchmark Results](../../benchmarks/LOAD_TEST_SCENARIOS.md)
- [Performance Optimization Guide](../../docs/cloud-architecture/PERFORMANCE_OPTIMIZATION_GUIDE.md)
- [Implementation Summary](../../docs/IMPLEMENTATION_SUMMARY.md)
- [ANN-Benchmarks.com](http://ann-benchmarks.com) - Standard vector search benchmarks

## 🎯 Optimization Based on Benchmarks

### Use Benchmark Results to Tune Performance

1. **Optimize for Latency** (sub-millisecond queries):
   ```rust
   HnswConfig {
       m: 16,              // Lower M = faster search, less recall
       ef_construction: 100,
       ef_search: 50,      // Lower ef_search = faster, less recall
       max_elements: 100000,
   }
   ```

2. **Optimize for Recall** (95%+ accuracy):
   ```rust
   HnswConfig {
       m: 64,              // Higher M = better recall
       ef_construction: 400,
       ef_search: 200,     // Higher ef_search = better recall
       max_elements: 100000,
   }
   ```

3. **Optimize for Memory** (minimal footprint):
   ```rust
   DbOptions {
       quantization: Some(QuantizationConfig::Binary),  // 32x compression
       ..Default::default()
   }
   ```

### Recommended Configurations by Use Case

| Use Case | M | ef_construction | ef_search | Quantization | Expected Performance |
|----------|---|----------------|-----------|--------------|----------------------|
| **Low-Latency Search** | 16 | 100 | 50 | Scalar | <0.5ms p50, 90%+ recall |
| **Balanced** | 32 | 200 | 100 | Scalar | <1ms p50, 95%+ recall |
| **High Accuracy** | 64 | 400 | 200 | None | <2ms p50, 98%+ recall |
| **Memory Constrained** | 16 | 100 | 50 | Binary | <1ms p50, 85%+ recall, 32x compression |

## 🛠️ Development

### Running Tests

```bash
# Run unit tests
cargo test -p ruvector-bench

# Run specific benchmark
cargo test -p ruvector-bench --test latency_stats_test
```

### Building Documentation

```bash
# Generate API documentation
cargo doc -p ruvector-bench --open
```

### Adding New Benchmarks

1. Create a new binary in `src/bin/`:
   ```bash
   touch src/bin/my_benchmark.rs
   ```

2. Add to `Cargo.toml`:
   ```toml
   [[bin]]
   name = "my-benchmark"
   path = "src/bin/my_benchmark.rs"
   ```

3. Implement using `ruvector-bench` utilities:
   ```rust
   use ruvector_bench::{LatencyStats, ResultWriter};
   ```

## 📚 API Reference

### Core Types

- **`BenchmarkResult`**: Comprehensive benchmark result structure
- **`LatencyStats`**: HDR histogram-based latency measurement
- **`DatasetGenerator`**: Synthetic vector data generation
- **`MemoryProfiler`**: Memory usage tracking
- **`ResultWriter`**: Multi-format result output (JSON, CSV, Markdown)

### Utilities

- **`calculate_recall()`**: Compute recall@k metric
- **`create_progress_bar()`**: Terminal progress indication
- **`VectorDistribution`**: Uniform, Normal, or Clustered vector generation

See [full API documentation](https://docs.rs/ruvector-bench) for details.

## 🤝 Contributing

We welcome contributions to improve the benchmarking suite!

### Areas for Contribution

- 📊 Additional benchmark scenarios (concurrent writes, updates, deletes)
- 🔌 Integration with other vector databases (Pinecone, Qdrant, Milvus)
- 📈 Enhanced visualization and reporting
- 🎯 Real-world dataset support (SIFT, GIST, Deep1M loaders)
- 🚀 Performance optimization insights

See [Contributing Guidelines](../../docs/development/CONTRIBUTING.md) for details.

## 📜 License

This crate is part of the Ruvector project and is licensed under the MIT License.

---

<div align="center">

**Part of [Ruvector](../../README.md) - Next-generation vector database built in Rust**

Built by [rUv](https://ruv.io) • [GitHub](https://github.com/ruvnet/ruvector) • [Documentation](../../docs/README.md)

</div>
