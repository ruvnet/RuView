# Ruvector Benchmark Suite Documentation

Comprehensive benchmarking tools for measuring and analyzing Ruvector's performance across various workloads and configurations.

## Table of Contents

1. [Overview](#overview)
2. [Installation](#installation)
3. [Benchmark Tools](#benchmark-tools)
4. [Quick Start](#quick-start)
5. [Detailed Usage](#detailed-usage)
6. [Understanding Results](#understanding-results)
7. [Performance Targets](#performance-targets)
8. [Troubleshooting](#troubleshooting)

## Overview

The Ruvector benchmark suite provides:

- **ANN-Benchmarks Compatibility**: Standard SIFT1M, GIST1M, Deep1M testing
- **AgenticDB Workloads**: Reflexion episodes, skill libraries, causal graphs
- **Latency Analysis**: p50, p95, p99, p99.9 percentile measurements
- **Memory Profiling**: Usage at various scales with quantization effects
- **System Comparison**: Ruvector vs other implementations
- **Performance Profiling**: CPU flamegraphs and hotspot analysis

## Installation

### Prerequisites

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Optional: HDF5 for loading real ANN benchmark datasets
# Ubuntu/Debian
sudo apt-get install libhdf5-dev

# macOS
brew install hdf5

# Optional: Profiling tools
sudo apt-get install linux-perf  # Linux only
```

### Build Benchmarks

```bash
cd crates/ruvector-bench

# Standard build
cargo build --release

# With profiling support
cargo build --release --features profiling

# With HDF5 dataset support
cargo build --release --features hdf5-datasets
```

## Benchmark Tools

### 1. ANN Benchmark (`ann-benchmark`)

Tests standard ANN benchmark datasets with configurable HNSW parameters.

**Features:**
- SIFT1M (128D, 1M vectors)
- GIST1M (960D, 1M vectors)
- Deep1M (96D, 1M vectors)
- Synthetic dataset generation
- Recall-QPS curves at 90%, 95%, 99%
- Multiple ef_search values

### 2. AgenticDB Benchmark (`agenticdb-benchmark`)

Simulates agentic AI workloads.

**Workloads:**
- Reflexion episode storage/retrieval
- Skill library search
- Causal graph queries
- Learning session throughput (mixed read/write)

### 3. Latency Benchmark (`latency-benchmark`)

Measures detailed latency characteristics.

**Tests:**
- Single-threaded latency
- Multi-threaded latency (configurable thread counts)
- Effect of ef_search on latency
- Effect of quantization on latency/recall tradeoff

### 4. Memory Benchmark (`memory-benchmark`)

Profiles memory usage at scale.

**Tests:**
- Memory at 10K, 100K, 1M vectors
- Effect of quantization (none, scalar, binary)
- Index overhead analysis
- Memory per vector calculation

### 5. Comparison Benchmark (`comparison-benchmark`)

Compares Ruvector against other systems.

**Comparisons:**
- Ruvector (optimized)
- Ruvector (no quantization)
- Simulated Python baseline
- Simulated brute-force search

### 6. Profiling Benchmark (`profiling-benchmark`)

Generates performance profiles.

**Outputs:**
- CPU flamegraphs (SVG)
- Profiling reports
- Hotspot identification
- SIMD utilization analysis

## Quick Start

### Run All Benchmarks

```bash
# Full benchmark suite
./scripts/run_all_benchmarks.sh

# Quick mode (smaller datasets)
./scripts/run_all_benchmarks.sh --quick

# With profiling
./scripts/run_all_benchmarks.sh --profile
```

### Run Individual Benchmarks

```bash
# ANN benchmarks
cargo run --release --bin ann-benchmark -- \
    --dataset synthetic \
    --num-vectors 100000 \
    --queries 1000

# AgenticDB workloads
cargo run --release --bin agenticdb-benchmark -- \
    --episodes 10000 \
    --queries 500

# Latency profiling
cargo run --release --bin latency-benchmark -- \
    --num-vectors 50000 \
    --threads "1,4,8,16"

# Memory profiling
cargo run --release --bin memory-benchmark -- \
    --scales "1000,10000,100000"

# System comparison
cargo run --release --bin comparison-benchmark -- \
    --num-vectors 50000

# Performance profiling
cargo run --release --features profiling --bin profiling-benchmark -- \
    --flamegraph
```

## Detailed Usage

### ANN Benchmark Options

```bash
cargo run --release --bin ann-benchmark -- --help

Options:
  -d, --dataset <DATASET>              Dataset: sift1m, gist1m, deep1m, synthetic [default: synthetic]
  -n, --num-vectors <NUM_VECTORS>      Number of vectors [default: 100000]
  -q, --queries <NUM_QUERIES>          Number of queries [default: 1000]
  -d, --dimensions <DIMENSIONS>        Vector dimensions [default: 128]
  -k, --k <K>                          K nearest neighbors [default: 10]
  -m, --m <M>                          HNSW M parameter [default: 32]
      --ef-construction <VALUE>        HNSW ef_construction [default: 200]
      --ef-search-values <VALUES>      HNSW ef_search values (comma-separated) [default: 50,100,200,400]
  -o, --output <OUTPUT>                Output directory [default: bench_results]
      --metric <METRIC>                Distance metric [default: cosine]
      --quantization <QUANT>           Quantization: none, scalar, binary [default: scalar]
```

### AgenticDB Benchmark Options

```bash
cargo run --release --bin agenticdb-benchmark -- --help

Options:
      --episodes <EPISODES>    Number of episodes [default: 10000]
      --skills <SKILLS>        Number of skills [default: 1000]
  -q, --queries <QUERIES>      Number of queries [default: 500]
  -o, --output <OUTPUT>        Output directory [default: bench_results]
```

### Latency Benchmark Options

```bash
cargo run --release --bin latency-benchmark -- --help

Options:
  -n, --num-vectors <NUM_VECTORS>    Number of vectors [default: 50000]
  -q, --queries <QUERIES>            Number of queries [default: 1000]
  -d, --dimensions <DIMENSIONS>      Vector dimensions [default: 384]
  -t, --threads <THREADS>            Thread counts to test [default: 1,4,8,16]
  -o, --output <OUTPUT>              Output directory [default: bench_results]
```

## Understanding Results

### Output Files

Each benchmark generates three output files:

1. **JSON** (`{benchmark}_benchmark.json`): Raw data for programmatic analysis
2. **CSV** (`{benchmark}_benchmark.csv`): Tabular data for spreadsheet analysis
3. **Markdown** (`{benchmark}_benchmark.md`): Human-readable report

### Key Metrics

#### QPS (Queries Per Second)
- Higher is better
- Measures throughput
- Target: >10,000 QPS for 100K vectors

#### Latency Percentiles
- **p50**: Median latency (typical user experience)
- **p95**: 95th percentile (captures most outliers)
- **p99**: 99th percentile (worst-case for most users)
- **p99.9**: 99.9th percentile (extreme outliers)
- Lower is better
- Target: <5ms p99 for 100K vectors

#### Recall
- **Recall@1**: Percentage of times the true nearest neighbor is found
- **Recall@10**: Percentage of true top-10 neighbors found
- **Recall@100**: Percentage of true top-100 neighbors found
- Higher is better
- Target: >95% recall@10

#### Memory
- Total memory usage in MB
- Memory per vector in KB
- Compression ratio with quantization
- Target: <2KB per vector with quantization

### Reading Benchmark Reports

Example output interpretation:

```
ef_search  QPS    p50 (ms)  p99 (ms)  Recall@10  Memory (MB)
50         15234  0.05      0.12      92.5%      156.2
100        12456  0.06      0.15      96.8%      156.2
200        8932   0.08      0.20      98.9%      156.2
```

**Analysis:**
- Increasing ef_search improves recall but reduces QPS
- ef_search=100 offers good balance (96.8% recall, 12K QPS)
- Memory usage constant across ef_search values

## Performance Targets

### AgenticDB Replacement Goals

Ruvector targets **10-100x performance improvement** over AgenticDB:

| Metric | AgenticDB (Python) | Ruvector (Target) | Speedup |
|--------|-------------------|-------------------|---------|
| Reflexion Retrieval | ~100 QPS | >5,000 QPS | 50x |
| Skill Search | ~50 QPS | >2,000 QPS | 40x |
| Index Build Time | ~60s/10K | <5s/10K | 12x |
| Memory Usage | ~500MB/100K | <100MB/100K | 5x |

### ANN-Benchmarks Targets

Competitive with state-of-the-art implementations:

| Dataset | Recall@10 | QPS Target | Latency p99 |
|---------|-----------|------------|-------------|
| SIFT1M | >95% | >10,000 | <1ms |
| GIST1M | >95% | >5,000 | <2ms |
| Deep1M | >95% | >15,000 | <0.5ms |

## Advanced Topics

### Profiling with Flamegraphs

Generate CPU flamegraphs to identify performance bottlenecks:

```bash
cargo run --release --features profiling --bin profiling-benchmark -- \
    --flamegraph \
    --output bench_results/profiling

# View flamegraph
firefox bench_results/profiling/flamegraph.svg
```

**Interpreting Flamegraphs:**
- Width = CPU time spent
- Height = call stack depth
- Look for wide plateaus (hotspots)
- Focus optimization on top 20% of time

### Custom Benchmark Scenarios

Create custom benchmarks by modifying the tools:

```rust
// Example: Custom dimension test
let dimensions = vec![64, 128, 256, 512, 768, 1024];
for dim in dimensions {
    let result = bench_custom(dim)?;
    results.push(result);
}
```

### Continuous Benchmarking

Integrate with CI/CD:

```yaml
# .github/workflows/benchmark.yml
name: Benchmarks
on: [push]
jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run benchmarks
        run: |
          cd crates/ruvector-bench
          ./scripts/run_all_benchmarks.sh --quick
      - name: Upload results
        uses: actions/upload-artifact@v2
        with:
          name: benchmark-results
          path: crates/ruvector-bench/bench_results/
```

## Troubleshooting

### Common Issues

#### "HDF5 not found"

```bash
# Install HDF5 development libraries
sudo apt-get install libhdf5-dev  # Ubuntu/Debian
brew install hdf5                 # macOS

# Or build without HDF5 support
cargo build --release --no-default-features
```

#### "Out of memory"

```bash
# Reduce dataset size
cargo run --release --bin ann-benchmark -- --num-vectors 10000

# Or use quick mode
./scripts/run_all_benchmarks.sh --quick
```

#### "Profiling not working"

```bash
# Ensure profiling feature is enabled
cargo build --release --features profiling

# Linux: May need perf permissions
echo -1 | sudo tee /proc/sys/kernel/perf_event_paranoid
```

#### "Benchmarks taking too long"

```bash
# Use quick mode
./scripts/run_all_benchmarks.sh --quick

# Or run individual benchmarks
cargo run --release --bin latency-benchmark -- --queries 100
```

### Performance Debugging

If benchmarks show unexpectedly slow results:

1. **Check CPU governor:**
   ```bash
   # Linux: Use performance mode
   sudo cpupower frequency-set -g performance
   ```

2. **Verify release build:**
   ```bash
   cargo build --release  # Not --debug!
   ```

3. **Check system load:**
   ```bash
   htop  # Ensure no other heavy processes
   ```

4. **Review HNSW parameters:**
   - Reduce ef_construction for faster indexing
   - Reduce ef_search for faster queries (at cost of recall)

## Results Analysis

### Comparing Runs

```bash
# Compare two benchmark runs
diff -u bench_results_old/ann_benchmark.csv bench_results_new/ann_benchmark.csv

# Plot results with Python
python3 scripts/plot_results.py bench_results/
```

### Statistical Significance

For reliable benchmarks:
- Run multiple iterations (3-5 times)
- Use appropriate dataset sizes (>10K vectors)
- Ensure consistent system load
- Record system specs in metadata

## Contributing

To add new benchmarks:

1. Create new binary in `src/bin/`
2. Use `ruvector_bench` utilities
3. Output results in standard format
4. Update this documentation
5. Add to `run_all_benchmarks.sh`

## References

- [ANN-Benchmarks](http://ann-benchmarks.com)
- [HNSW Paper](https://arxiv.org/abs/1603.09320)
- [AgenticDB Documentation](https://github.com/agenticdb/agenticdb)
- [Ruvector Repository](https://github.com/ruvnet/ruvector)

## Support

For issues or questions:
- GitHub Issues: https://github.com/ruvnet/ruvector/issues
- Documentation: https://github.com/ruvnet/ruvector/docs

---

Last updated: 2025-11-19
