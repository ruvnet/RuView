# Benchmark Usage Guide

## Quick Start

### Run All Benchmarks
```bash
./benches/run_benchmarks.sh
```

### Run Specific Benchmark Suite
```bash
# Manifold (geometric embedding)
cargo bench --bench manifold_bench

# Hypergraph (relational reasoning)
cargo bench --bench hypergraph_bench

# Temporal (causal memory)
cargo bench --bench temporal_bench

# Federation (distributed consensus)
cargo bench --bench federation_bench
```

### Run Specific Benchmark
```bash
cargo bench --bench manifold_bench -- manifold_retrieval
cargo bench --bench temporal_bench -- causal_query
```

## Baseline Management

### Save Initial Baseline
```bash
cargo bench -- --save-baseline initial
```

### Compare Against Baseline
```bash
# After making optimizations
cargo bench -- --baseline initial
```

### Multiple Baselines
```bash
# Save current as v0.1.0
cargo bench -- --save-baseline v0.1.0

# After changes, compare
cargo bench -- --baseline v0.1.0
```

## Performance Analysis

### HTML Reports
After running benchmarks, open the detailed HTML reports:
```bash
open target/criterion/report/index.html
```

Reports include:
- Performance graphs
- Statistical analysis
- Confidence intervals
- Historical comparisons
- Regression detection

### Command-Line Output
Look for key metrics:
- **time**: Mean execution time
- **change**: Performance delta vs baseline
- **thrpt**: Throughput (operations/second)

Example output:
```
manifold_retrieval/1000
                        time:   [85.234 µs 87.123 µs 89.012 µs]
                        change: [-5.2341% -3.1234% -1.0123%] (p = 0.01 < 0.05)
                        thrpt:  [11234 ops/s 11478 ops/s 11732 ops/s]
```

## Profiling Integration

### CPU Profiling
```bash
# Install cargo-flamegraph
cargo install flamegraph

# Profile a benchmark
cargo flamegraph --bench manifold_bench -- --bench
```

### Memory Profiling
```bash
# Install valgrind and heaptrack
# Run with heaptrack
heaptrack cargo bench --bench manifold_bench
```

## Continuous Benchmarking

### CI Integration
Add to GitHub Actions:
```yaml
name: Benchmarks
on: [push, pull_request]
jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run benchmarks
        run: cargo bench --no-fail-fast
      - name: Archive results
        uses: actions/upload-artifact@v2
        with:
          name: criterion-results
          path: target/criterion/
```

### Pre-commit Hook
```bash
# .git/hooks/pre-commit
#!/bin/bash
cargo bench --no-fail-fast || {
    echo "Benchmarks failed!"
    exit 1
}
```

## Interpreting Results

### Latency Targets
| Component | Operation | Target | Threshold |
|-----------|-----------|--------|-----------|
| Manifold | Retrieval @ 1k | < 100μs | 150μs |
| Hypergraph | Query @ 1k | < 70μs | 100μs |
| Temporal | Causal query @ 1k | < 150μs | 200μs |
| Federation | Consensus @ 5 nodes | < 70ms | 100ms |

### Regression Detection
- **< 5% regression**: Normal variance
- **5-10% regression**: Investigate
- **> 10% regression**: Requires optimization

### Statistical Significance
- **p < 0.05**: Statistically significant
- **p > 0.05**: Within noise range

## Optimization Workflow

1. **Identify Bottleneck**
   ```bash
   cargo bench --bench <suite> | grep "change:"
   ```

2. **Profile Hot Paths**
   ```bash
   cargo flamegraph --bench <suite>
   ```

3. **Optimize Code**
   - Apply optimization
   - Document changes

4. **Measure Impact**
   ```bash
   cargo bench -- --baseline before-optimization
   ```

5. **Validate**
   - Ensure > 5% improvement
   - No regressions in other areas
   - Tests still pass

## Advanced Usage

### Custom Measurement Time
```bash
# Longer measurement for more precision
cargo bench -- --measurement-time=30
```

### Sample Size
```bash
# More samples for stability
cargo bench -- --sample-size=500
```

### Noise Threshold
```bash
# More sensitive regression detection
cargo bench -- --noise-threshold=0.03
```

### Warm-up Time
```bash
# Longer warmup for JIT/caching
cargo bench -- --warm-up-time=10
```

## Troubleshooting

### High Variance
If you see high variance (> 10%):
- Close background applications
- Disable CPU frequency scaling
- Run on dedicated hardware
- Increase sample size

### Compilation Errors
```bash
# Check dependencies
cargo check --benches

# Update dependencies
cargo update

# Clean and rebuild
cargo clean && cargo bench
```

### Missing Reports
```bash
# Ensure criterion is properly configured
cat Cargo.toml | grep criterion

# Check feature flags
cargo bench --features html_reports
```

## Best Practices

1. **Baseline Before Changes**
   - Always save baseline before optimization work

2. **Consistent Environment**
   - Same hardware for comparisons
   - Minimal background processes
   - Disable power management

3. **Multiple Runs**
   - Run benchmarks 3+ times
   - Average results
   - Look for consistency

4. **Document Changes**
   - Note optimizations in commit messages
   - Update baseline documentation
   - Track improvement metrics

5. **Review Regularly**
   - Weekly baseline updates
   - Monthly trend analysis
   - Quarterly performance reviews

## Resources

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Flamegraph Tutorial](https://www.brendangregg.com/flamegraphs.html)

---

**Last Updated**: 2025-11-29
**Maintainer**: Performance Agent
**Questions**: See docs/PERFORMANCE_BASELINE.md
