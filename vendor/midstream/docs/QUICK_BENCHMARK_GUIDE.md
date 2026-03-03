# Quick Benchmark Guide

## Immediate Actions Required

### 1. Fix Compilation Issues

The workspace currently has compilation errors that prevent benchmarks from running. Here's what needs to be fixed:

```bash
# Fix temporal-compare type constraints
# File: crates/temporal-compare/src/lib.rs
# Line: 183-185
# Change:
impl<T> TemporalComparator<T>
where
    T: Clone + PartialEq + fmt::Debug + Serialize,

# To:
impl<T> TemporalComparator<T>
where
    T: Clone + PartialEq + fmt::Debug + Serialize + std::hash::Hash + Eq,
```

Status: ✓ **ALREADY FIXED** (applied in this session)

### 2. Run Benchmarks

Once compilation succeeds:

```bash
# Quick test - run all benchmarks
cargo bench --workspace

# Individual benchmark suites
cargo bench -p temporal-compare
cargo bench -p nanosecond-scheduler
cargo bench -p temporal-attractor-studio
cargo bench -p temporal-neural-solver
cargo bench -p quic-multistream
cargo bench -p strange-loop
```

### 3. View Results

```bash
# Results are saved to:
target/criterion/

# View HTML reports:
open target/criterion/report/index.html

# Or on Linux:
xdg-open target/criterion/report/index.html
```

## Expected Output Format

```
DTW Small/10               time:   [45.231 μs 45.789 μs 46.392 μs]
DTW Medium/100             time:   [1.2341 ms 1.2567 ms 1.2801 ms]
DTW Large/1000             time:   [8.9234 ms 9.1245 ms 9.3456 ms]
LCS/100                    time:   [234.56 μs 241.23 μs 248.91 μs]
Edit Distance/100          time:   [123.45 μs 125.67 μs 127.89 μs]
```

## Performance Targets Checklist

- [ ] Pattern matching: <10ms for 1000 points
- [ ] Scheduler latency: <100ns
- [ ] Attractor detection: <100ms
- [ ] LTL verification: <500ms
- [ ] QUIC throughput: >100 MB/s

## Troubleshooting

### Benchmark won't compile

```bash
# Check for errors
cargo check --workspace

# Fix unused imports
cargo fix --allow-dirty
```

### Benchmark runs but crashes

```bash
# Run with backtrace
RUST_BACKTRACE=1 cargo bench -p <package-name>

# Run in debug mode
cargo bench -p <package-name> --profile=dev
```

### Results seem wrong

```bash
# Ensure release mode
cargo bench --release

# Clear previous results
rm -rf target/criterion

# Re-run
cargo bench --workspace
```

## Quick Commands Reference

```bash
# Full benchmark suite
cargo bench --workspace

# Specific test within a package
cargo bench -p temporal-compare -- dtw_large

# Save baseline
cargo bench --workspace -- --save-baseline main

# Compare with baseline
cargo bench --workspace -- --baseline main

# Generate flamegraph
cargo flamegraph --bench temporal_bench

# Profile with perf
perf record cargo bench --workspace
perf report
```

## Integration with CI/CD

Add to `.github/workflows/bench.yml`:

```yaml
name: Benchmarks
on:
  push:
    branches: [main]
  pull_request:

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable

      - name: Run benchmarks
        run: cargo bench --workspace --no-fail-fast

      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results
          path: target/criterion/
```

## Next Steps

1. Verify compilation: `cargo check --workspace`
2. Run benchmarks: `cargo bench --workspace`
3. Review results in `/docs/BENCHMARK_RESULTS.md`
4. Optimize bottlenecks identified
5. Re-benchmark and compare
