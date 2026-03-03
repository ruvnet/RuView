# Comprehensive Benchmark Guide

This guide covers all benchmarks for the Midstream workspace's 6 production crates.

## Overview

All benchmarks use Criterion.rs for statistical analysis and HTML report generation. Each crate has comprehensive benchmarks targeting specific performance goals.

## Running Benchmarks

### Run All Benchmarks
```bash
cargo bench
```

### Run Specific Crate Benchmarks
```bash
# Temporal Compare (DTW, LCS, Edit Distance)
cargo bench --bench temporal_bench

# Nanosecond Scheduler
cargo bench --bench scheduler_bench

# Temporal Attractor Studio
cargo bench --bench attractor_bench

# Temporal Neural Solver
cargo bench --bench solver_bench

# Strange Loop (Meta-Learning)
cargo bench --bench meta_bench

# QUIC Multistream
cargo bench --bench quic_bench
```

### Run Specific Benchmark Groups
```bash
# DTW performance tests
cargo bench --bench temporal_bench dtw

# Scheduler overhead tests
cargo bench --bench scheduler_bench overhead

# Phase space embedding
cargo bench --bench attractor_bench embedding
```

## Performance Targets

### 1. Temporal Compare (`temporal_bench.rs`)

**Targets:**
- DTW n=100: <10ms
- LCS n=100: <5ms
- Edit distance n=100: <3ms
- Cache hit: <1μs

**Benchmark Groups:**
```rust
dtw_benches          // DTW performance across sizes
lcs_benches          // LCS algorithm performance
edit_benches         // Edit distance operations
cache_benches        // Cache hit/miss scenarios
memory_benches       // Memory allocation patterns
```

**Key Metrics:**
- Throughput (elements/second)
- Mean execution time
- Standard deviation
- Memory allocations

### 2. Nanosecond Scheduler (`scheduler_bench.rs`)

**Targets:**
- Schedule overhead: <100ns
- Task execution: <1μs
- Stats calculation: <10μs
- Multi-threaded scaling

**Benchmark Groups:**
```rust
overhead_benches     // Schedule operation overhead
latency_benches      // Task execution latency
queue_benches        // Priority queue operations
stats_benches        // Statistics calculation
threading_benches    // Multi-threaded scenarios
```

**Key Scenarios:**
- High/low contention
- Priority variations
- Batch operations
- Concurrent scheduling

### 3. Temporal Attractor Studio (`attractor_bench.rs`)

**Targets:**
- Phase space embedding: <20ms (n=1000)
- Lyapunov calculation: <500ms
- Attractor detection: <100ms
- Dimension estimation

**Benchmark Groups:**
```rust
embedding_benches    // Phase space reconstruction
lyapunov_benches     // Lyapunov exponent calculation
detection_benches    // Attractor type detection
trajectory_benches   // Trajectory analysis
dimension_benches    // Dimension estimation
chaos_benches        // Chaos detection
pipeline_benches     // Complete analysis pipeline
```

**Test Attractors:**
- Lorenz attractor
- Rössler attractor
- Hénon map
- Periodic signals
- Random data

### 4. Temporal Neural Solver (`solver_bench.rs`)

**Targets:**
- Formula encoding: <10ms
- Verification: <100ms
- Parsing: <5ms
- State checking: <1μs

**Benchmark Groups:**
```rust
encoding_benches     // LTL formula encoding
parsing_benches      // Formula parsing
verification_benches // Trace verification
state_benches        // State operations
neural_benches       // Neural verification
operator_benches     // Temporal operators
pipeline_benches     // Complete pipeline
```

**LTL Operations:**
- Next (X)
- Globally (G)
- Finally (F)
- Until (U)
- Boolean combinations

### 5. Strange Loop (`meta_bench.rs`)

**Targets:**
- Meta-learning iteration: <50ms
- Pattern extraction: <20ms
- Integration overhead: <100ms
- Recursive optimization

**Benchmark Groups:**
```rust
learning_benches     // Meta-learning iteration
pattern_benches      // Pattern extraction/matching
hierarchy_benches    // Multi-level learning
integration_benches  // Cross-crate integration
recursive_benches    // Self-referential operations
pipeline_benches     // Complete meta-learning cycle
```

**Integration Tests:**
- With temporal-compare (DTW)
- With nanosecond-scheduler
- With attractor-studio
- Cross-crate overhead

### 6. QUIC Multistream (`quic_bench.rs`)

**Targets:**
- Stream establishment: <1ms
- Multiplexing overhead: <100μs
- Throughput: >1GB/s
- Connection setup: <10ms

## Benchmark Configuration

### Criterion Settings

Each benchmark group uses optimized Criterion configuration:

```rust
criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(100)           // Statistical samples
        .measurement_time(Duration::from_secs(10))  // Per benchmark
        .warm_up_time(Duration::from_secs(3));      // Warmup period
    targets = ...
}
```

### Custom Configurations

**Fast benchmarks** (overhead, parsing):
- sample_size: 500-1000
- measurement_time: 5s

**Slow benchmarks** (neural, integration):
- sample_size: 30-50
- measurement_time: 15s

## Understanding Results

### HTML Reports

After running benchmarks, view results at:
```
target/criterion/[benchmark_name]/report/index.html
```

### Key Metrics

1. **Mean**: Average execution time
2. **Std Dev**: Consistency indicator
3. **Median**: Central tendency
4. **MAD**: Median Absolute Deviation
5. **Throughput**: Operations per second

### Regression Detection

Criterion automatically detects performance regressions:
- Green: Performance improved
- Yellow: Within noise threshold
- Red: Performance regressed

## Profiling Integration

### With perf
```bash
cargo bench --bench temporal_bench -- --profile-time=10
perf record -g cargo bench --bench temporal_bench
perf report
```

### With flamegraph
```bash
cargo install flamegraph
cargo flamegraph --bench temporal_bench
```

### With valgrind (memory)
```bash
cargo bench --bench temporal_bench -- --profile-time=10
valgrind --tool=cachegrind target/release/temporal_bench
```

## Best Practices

### 1. Consistent Environment
- Close other applications
- Disable CPU frequency scaling
- Use consistent power settings
- Run multiple times

### 2. Baseline Establishment
```bash
# Create baseline
cargo bench -- --save-baseline main

# Compare against baseline
git checkout feature-branch
cargo bench -- --baseline main
```

### 3. Statistical Validity
- Minimum 30 samples for statistical significance
- Watch for outliers (high std dev)
- Multiple runs for consistency

### 4. Realistic Data
- Use production-like data sizes
- Include edge cases
- Test boundary conditions
- Vary input patterns

## CI/CD Integration

### GitHub Actions

```yaml
- name: Run benchmarks
  run: cargo bench --no-fail-fast

- name: Upload benchmark results
  uses: actions/upload-artifact@v3
  with:
    name: benchmark-results
    path: target/criterion/
```

### Performance Tracking

Store baseline results in repo:
```bash
git add target/criterion/*/base/
git commit -m "Update benchmark baselines"
```

## Optimization Workflow

1. **Identify bottlenecks**: Run benchmarks, check reports
2. **Profile**: Use flamegraph/perf for hotspots
3. **Optimize**: Make targeted improvements
4. **Verify**: Re-run benchmarks
5. **Compare**: Check against baseline
6. **Document**: Update if targets change

## Common Issues

### High Variance
- System load too high
- Thermal throttling
- Background processes
- Insufficient samples

**Solution**: Increase sample size, close applications, check CPU frequency.

### Unexpected Regressions
- Compiler version changes
- Dependency updates
- System configuration
- Measurement noise

**Solution**: Compare multiple runs, check git diff, validate hardware.

### Memory Benchmarks Inconsistent
- GC timing (if applicable)
- Allocator behavior
- Page faults
- Cache effects

**Solution**: Increase warmup time, use fixed heap size, minimize allocations.

## Future Enhancements

- [ ] Continuous benchmark tracking
- [ ] Performance regression alerts
- [ ] Cross-platform comparison
- [ ] Memory profiling integration
- [ ] Automated optimization suggestions
- [ ] Benchmark result visualization
- [ ] Historical trend analysis

## Resources

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Linux perf Tutorial](https://perf.wiki.kernel.org/index.php/Tutorial)

---

**Summary**: All 6 crates now have comprehensive benchmarks covering core functionality, edge cases, and integration scenarios. Total ~2,800 lines of benchmark code targeting specific performance goals for each crate.
