# Benchmark Quick Reference Card

## 🚀 Run Benchmarks

```bash
# All benchmarks
./scripts/run_benchmarks.sh

# Individual crate
cargo bench --bench temporal_bench      # Temporal Compare
cargo bench --bench scheduler_bench     # Nanosecond Scheduler
cargo bench --bench attractor_bench     # Attractor Studio
cargo bench --bench solver_bench        # Neural Solver
cargo bench --bench meta_bench          # Strange Loop
cargo bench --bench quic_bench          # QUIC Multistream

# Specific group
cargo bench --bench temporal_bench dtw
cargo bench --bench scheduler_bench overhead
```

## 📊 Performance Targets Cheatsheet

| Benchmark | Key Metric | Target |
|-----------|-----------|--------|
| `temporal_bench` | DTW n=100 | <10ms |
| | LCS n=100 | <5ms |
| | Edit n=100 | <3ms |
| `scheduler_bench` | Schedule | <100ns |
| | Execute | <1μs |
| | Stats | <10μs |
| `attractor_bench` | Phase space | <20ms |
| | Lyapunov | <500ms |
| | Detection | <100ms |
| `solver_bench` | Encode | <10ms |
| | Verify | <100ms |
| | Parse | <5ms |
| `meta_bench` | Learn | <50ms |
| | Extract | <20ms |
| | Integrate | <100ms |
| `quic_bench` | Stream | <1ms |
| | Multiplex | <100μs |
| | Throughput | >1GB/s |

## 🎯 Common Commands

```bash
# Compare branches
./scripts/benchmark_comparison.sh main feature-branch

# Save baseline
cargo bench -- --save-baseline main

# Compare with baseline
cargo bench -- --baseline main

# View HTML reports
open target/criterion/*/report/index.html

# View summary
cat target/criterion/SUMMARY.md
```

## 🔍 Profiling

```bash
# Flamegraph
cargo flamegraph --bench temporal_bench

# perf
perf record -g cargo bench --bench temporal_bench
perf report

# Valgrind
valgrind --tool=cachegrind target/release/deps/temporal_bench-*
```

## 📈 Benchmark Groups

### temporal_bench
- `dtw_benches` - DTW performance
- `lcs_benches` - LCS algorithms
- `edit_benches` - Edit distance
- `cache_benches` - Cache scenarios
- `memory_benches` - Memory patterns

### scheduler_bench
- `overhead_benches` - Schedule overhead
- `latency_benches` - Task execution
- `queue_benches` - Priority queue
- `stats_benches` - Statistics
- `threading_benches` - Multi-threading

### attractor_bench
- `embedding_benches` - Phase space
- `lyapunov_benches` - Lyapunov calc
- `detection_benches` - Attractor detection
- `trajectory_benches` - Trajectory analysis
- `dimension_benches` - Dimension estimation
- `chaos_benches` - Chaos detection
- `pipeline_benches` - Complete analysis

### solver_bench
- `encoding_benches` - Formula encoding
- `parsing_benches` - Formula parsing
- `verification_benches` - Trace verification
- `state_benches` - State operations
- `neural_benches` - Neural verification
- `operator_benches` - Temporal operators
- `pipeline_benches` - Complete pipeline

### meta_bench
- `learning_benches` - Meta-learning
- `pattern_benches` - Pattern extraction
- `hierarchy_benches` - Multi-level learning
- `integration_benches` - Cross-crate
- `recursive_benches` - Self-referential
- `pipeline_benches` - Complete cycle

### quic_bench
- `stream_benches` - Stream operations
- `multiplexing_benches` - Multiplexing
- `connection_benches` - Connection setup
- `throughput_benches` - Data throughput
- `concurrent_benches` - Concurrent streams
- `error_benches` - Error handling

## 📂 File Locations

```
/workspaces/midstream/
├── benches/
│   ├── temporal_bench.rs        (~450 lines)
│   ├── scheduler_bench.rs       (~520 lines)
│   ├── attractor_bench.rs       (~480 lines)
│   ├── solver_bench.rs          (~490 lines)
│   ├── meta_bench.rs            (~500 lines)
│   ├── quic_bench.rs            (~420 lines)
│   ├── README.md                (Overview)
│   └── QUICK_REFERENCE.md       (This file)
├── scripts/
│   ├── run_benchmarks.sh        (Run all)
│   └── benchmark_comparison.sh  (Compare)
├── docs/
│   └── BENCHMARK_GUIDE.md       (Full guide)
├── BENCHMARKS_SUMMARY.md        (Summary)
└── Cargo.toml                   (Config)
```

## 🎨 Output Interpretation

### Terminal Colors
- 🟢 Green: Performance improved
- 🟡 Yellow: Within noise threshold
- 🔴 Red: Performance regressed

### Key Metrics
- **Mean**: Average execution time
- **Std Dev**: Consistency (lower is better)
- **Median**: Central tendency
- **Throughput**: Operations/second

## ⚡ Best Practices

1. **Close unnecessary apps** before benchmarking
2. **Run multiple times** for consistency
3. **Check std dev** - high values indicate noise
4. **Use baselines** for regression detection
5. **Profile hotspots** when optimizing
6. **Document changes** that affect performance

## 🔧 Troubleshooting

### High Variance
```bash
# Increase sample size
cargo bench -- --sample-size 200

# Disable frequency scaling
sudo cpupower frequency-set --governor performance
```

### Slow Benchmarks
```bash
# Reduce measurement time
cargo bench -- --measurement-time 5

# Run specific tests only
cargo bench --bench temporal_bench dtw_performance/linear/10
```

### Memory Issues
```bash
# Run with more memory
cargo bench --release -- --test-threads=1

# Profile memory usage
valgrind --tool=massif cargo bench
```

## 📊 Example Output

```
dtw_performance/linear/100
                        time:   [8.234 ms 8.567 ms 8.912 ms]
                        thrpt:  [11.22 Kelem/s 11.67 Kelem/s 12.14 Kelem/s]
                 change:
                        time:   [-5.2341% -3.1234% -1.0123%] (p = 0.00 < 0.05)
                        thrpt:  [+1.0226% +3.2345% +5.5234%]
                        Performance has improved.
```

## 🎯 Quick Wins

1. **First time?** Run: `./scripts/run_benchmarks.sh`
2. **Optimizing?** Profile: `cargo flamegraph --bench <name>`
3. **Comparing?** Run: `./scripts/benchmark_comparison.sh main feature`
4. **Debugging?** Check: `target/criterion/*/report/index.html`

---

**Total Benchmarks**: 6 files | **Total Lines**: ~2,860 | **Groups**: 45+ | **Scenarios**: 150+
