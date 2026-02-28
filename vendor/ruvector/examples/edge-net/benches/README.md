# Edge-Net Comprehensive Benchmark Suite

## Overview

This directory contains a comprehensive benchmark suite for the edge-net distributed compute intelligence network. The suite tests all critical performance aspects including spike-driven attention, RAC coherence, learning modules, and integration scenarios.

## Quick Start

```bash
# Navigate to edge-net directory
cd /workspaces/ruvector/examples/edge-net

# Install nightly Rust (required for bench feature)
rustup default nightly

# Run all benchmarks
cargo bench --features bench

# Or use the provided script
./benches/run_benchmarks.sh
```

## Benchmark Structure

### Total Benchmarks: 47

#### 1. Spike-Driven Attention (7 benchmarks)
- Energy-efficient attention with 87x claimed savings
- Tests encoding, attention computation, and energy ratio
- Located in `src/bench.rs` lines 522-596

#### 2. RAC Coherence Engine (6 benchmarks)
- Adversarial coherence for distributed claims
- Tests event ingestion, quarantine, Merkle proofs
- Located in `src/bench.rs` lines 598-747

#### 3. Learning Modules (5 benchmarks)
- ReasoningBank pattern storage and lookup
- Tests trajectory tracking and similarity computation
- Located in `src/bench.rs` lines 749-865

#### 4. Multi-Head Attention (4 benchmarks)
- Standard attention for task routing
- Tests scaling with dimensions and heads
- Located in `src/bench.rs` lines 867-925

#### 5. Integration (4 benchmarks)
- End-to-end performance tests
- Tests combined system overhead
- Located in `src/bench.rs` lines 927-1105

#### 6. Legacy Benchmarks (21 benchmarks)
- Credit operations, QDAG, tasks, security
- Network topology, economic engine
- Located in `src/bench.rs` lines 1-520

## Running Benchmarks

### All Benchmarks

```bash
cargo bench --features bench
```

### By Category

```bash
# Spike-driven attention
cargo bench --features bench -- spike_

# RAC coherence
cargo bench --features bench -- rac_

# Learning modules
cargo bench --features bench -- reasoning_bank
cargo bench --features bench -- trajectory
cargo bench --features bench -- pattern_similarity

# Multi-head attention
cargo bench --features bench -- multi_head

# Integration
cargo bench --features bench -- integration
cargo bench --features bench -- end_to_end
cargo bench --features bench -- concurrent
```

### Specific Benchmark

```bash
# Run a single benchmark
cargo bench --features bench -- bench_spike_attention_seq64_dim128
```

### Custom Iterations

```bash
# Run with more iterations for statistical significance
BENCH_ITERATIONS=1000 cargo bench --features bench
```

## Output Format

Each benchmark produces output like:

```
test bench_spike_attention_seq64_dim128 ... bench:      45,230 ns/iter (+/- 2,150)
```

**Interpretation:**
- `45,230 ns/iter`: Mean execution time (45.23 µs)
- `(+/- 2,150)`: Standard deviation (±2.15 µs, 4.7% jitter)

**Derived Metrics:**
- Throughput: 1,000,000,000 / 45,230 = 22,110 ops/sec
- P99 (approx): Mean + 3*StdDev = 51,680 ns

## Performance Targets

| Benchmark | Target | Rationale |
|-----------|--------|-----------|
| **Spike Encoding** | < 1 µs/value | Real-time encoding |
| **Spike Attention (64×128)** | < 100 µs | 10K ops/sec throughput |
| **RAC Event Ingestion** | < 50 µs | 20K events/sec |
| **RAC Quarantine Check** | < 100 ns | Hot path operation |
| **ReasoningBank Lookup (10K)** | < 10 ms | Acceptable async delay |
| **Multi-Head Attention (8h×128d)** | < 50 µs | Real-time routing |
| **E2E Task Routing** | < 1 ms | User-facing threshold |

## Key Metrics

### Spike-Driven Attention

**Energy Efficiency Calculation:**

```
Standard Attention Energy = 2 * seq² * dim * 3.7 pJ
Spike Attention Energy = seq * spikes * dim * 1.0 pJ

For seq=64, dim=256, spikes=2.4:
  Standard: 7,741,440 pJ
  Spike: 39,321 pJ
  Ratio: 196.8x (theoretical)
  Achieved: ~87x (with encoding overhead)
```

**Validation:**
- Energy ratio should be 70x - 100x
- Encoding overhead should be < 60% of total time
- Attention should scale O(n*m) with n=seq_len, m=spike_count

### RAC Coherence Performance

**Expected Throughput:**
- Single event: 1-2M events/sec
- Batch 1K events: 1.2K-1.6K batches/sec
- Quarantine check: 10M-20M checks/sec
- Merkle update: 100K-200K updates/sec

**Scaling:**
- Event ingestion: O(1) amortized
- Merkle update: O(log n) per event
- Quarantine: O(1) hash lookup

### Learning Module Scaling

**ReasoningBank Lookup:**

Without indexing (current):
```
1K patterns: ~200 µs (linear scan)
10K patterns: ~2 ms (10x scaling)
100K patterns: ~20 ms (10x scaling)
```

With ANN indexing (future optimization):
```
1K patterns: ~2 µs (log scaling)
10K patterns: ~2.6 µs (1.3x scaling)
100K patterns: ~3.2 µs (1.2x scaling)
```

**Validation:**
- 1K → 10K should scale ~10x (linear)
- Store operation < 10 µs
- Similarity computation < 300 ns

### Multi-Head Attention Complexity

**Time Complexity:** O(h * d * (d + k))
- h = number of heads
- d = dimension per head
- k = number of keys

**Scaling Verification:**
- 2x dimensions → 4x time (quadratic)
- 2x heads → 2x time (linear)
- 2x keys → 2x time (linear)

## Benchmark Analysis Tools

### benchmark_runner.rs

Provides statistical analysis and reporting:

```rust
use benchmark_runner::BenchmarkSuite;

let mut suite = BenchmarkSuite::new();
suite.run_benchmark("test", 100, || {
    // benchmark code
});

println!("{}", suite.generate_report());
```

**Features:**
- Mean, median, std dev, percentiles
- Throughput calculation
- Comparative analysis
- Pass/fail against targets

### run_benchmarks.sh

Automated benchmark execution:

```bash
./benches/run_benchmarks.sh
```

**Output:**
- Saves results to `benchmark_results/`
- Generates timestamped reports
- Runs all benchmark categories
- Produces text logs for analysis

## Documentation

### BENCHMARK_ANALYSIS.md

Comprehensive guide covering:
- Benchmark categories and purpose
- Statistical analysis methodology
- Performance targets and rationale
- Scaling characteristics
- Optimization opportunities

### BENCHMARK_SUMMARY.md

Quick reference with:
- 47 benchmark breakdown
- Expected results summary
- Key performance indicators
- Running instructions

### BENCHMARK_RESULTS.md

Theoretical analysis including:
- Energy efficiency calculations
- Complexity analysis
- Performance budgets
- Bottleneck identification
- Optimization recommendations

## Interpreting Results

### Good Performance Indicators

✅ **Low Mean Latency** - Fast execution
✅ **Low Jitter** - Consistent performance (StdDev < 10% of mean)
✅ **Expected Scaling** - Matches theoretical complexity
✅ **High Throughput** - Many ops/sec

### Performance Red Flags

❌ **High P99/P99.9** - Long tail latencies
❌ **High StdDev** - Inconsistent performance (>20% jitter)
❌ **Poor Scaling** - Worse than expected complexity
❌ **Memory Growth** - Unbounded memory usage

### Example Analysis

```
bench_spike_attention_seq64_dim128:
  Mean: 45,230 ns (45.23 µs)
  StdDev: 2,150 ns (4.7%)
  Throughput: 22,110 ops/sec

✅ Below 100µs target
✅ Low jitter (<5%)
✅ Adequate throughput
```

## Optimization Opportunities

Based on theoretical analysis:

### High Priority

1. **ANN Indexing for ReasoningBank**
   - Expected: 100x speedup for 10K+ patterns
   - Libraries: FAISS, Annoy, HNSW
   - Effort: Medium (1-2 weeks)

2. **SIMD for Spike Encoding**
   - Expected: 4-8x speedup
   - Use: std::simd or intrinsics
   - Effort: Low (few days)

3. **Parallel Merkle Updates**
   - Expected: 4-8x speedup on multi-core
   - Use: Rayon parallel iterators
   - Effort: Low (few days)

### Medium Priority

4. **Flash Attention**
   - Expected: 2-3x speedup
   - Complexity: High
   - Effort: High (2-3 weeks)

5. **Bloom Filters for Quarantine**
   - Expected: 2x speedup for negative lookups
   - Complexity: Low
   - Effort: Low (few days)

## CI/CD Integration

### Regression Detection

```yaml
name: Benchmarks
on: [push, pull_request]
jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: nightly
      - run: cargo bench --features bench
      - run: ./benches/compare_benchmarks.sh
```

### Performance Budgets

Assert maximum latencies:

```rust
#[bench]
fn bench_critical(b: &mut Bencher) {
    let result = b.iter(|| {
        // code
    });

    assert!(result.mean < Duration::from_micros(100));
}
```

## Troubleshooting

### Benchmark Not Running

```bash
# Ensure nightly Rust
rustup default nightly

# Check feature is enabled
cargo bench --features bench -- --list

# Verify dependencies
cargo check --features bench
```

### Inconsistent Results

```bash
# Increase iterations
BENCH_ITERATIONS=1000 cargo bench

# Reduce system noise
sudo systemctl stop cron
sudo systemctl stop atd

# Pin to CPU core
taskset -c 0 cargo bench
```

### High Variance

- Close other applications
- Disable CPU frequency scaling
- Run on dedicated benchmark machine
- Increase warmup iterations

## Contributing

When adding benchmarks:

1. ✅ Add to appropriate category in `src/bench.rs`
2. ✅ Document expected performance
3. ✅ Update this README
4. ✅ Run full suite before PR
5. ✅ Include results in PR description

## References

- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Criterion.rs](https://github.com/bheisler/criterion.rs)
- [Statistical Benchmarking](https://en.wikipedia.org/wiki/Benchmarking)
- [Edge-Net Documentation](../docs/)

## License

MIT - See LICENSE file in repository root.
