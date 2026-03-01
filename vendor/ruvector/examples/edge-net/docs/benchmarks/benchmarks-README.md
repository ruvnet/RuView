# Edge-Net Performance Benchmarks

## Overview

Comprehensive benchmark suite for the edge-net distributed compute network. Tests all critical operations including credit management, QDAG transactions, task processing, security operations, and network coordination.

## Quick Start

### Running All Benchmarks

```bash
# Standard benchmarks
cargo bench --features=bench

# With unstable features (for better stats)
cargo +nightly bench --features=bench

# Specific benchmark
cargo bench --features=bench bench_credit_operation
```

### Running Specific Suites

```bash
# Credit operations only
cargo bench --features=bench credit

# QDAG operations only
cargo bench --features=bench qdag

# Security operations only
cargo bench --features=bench security

# Network topology only
cargo bench --features=bench topology
```

## Benchmark Categories

### 1. Credit Operations (6 benchmarks)

Tests the CRDT-based credit ledger performance:

- **bench_credit_operation**: Adding credits (rewards)
- **bench_deduct_operation**: Spending credits (tasks)
- **bench_balance_calculation**: Computing current balance
- **bench_ledger_merge**: CRDT synchronization between nodes

**Key Metrics**:
- Target: <1µs per credit/deduct
- Target: <100ns per balance check (with optimizations)
- Target: <10ms for merging 100 transactions

### 2. QDAG Transaction Operations (3 benchmarks)

Tests the quantum-resistant DAG currency performance:

- **bench_qdag_transaction_creation**: Creating new QDAG transactions
- **bench_qdag_balance_query**: Querying account balances
- **bench_qdag_tip_selection**: Selecting tips for validation

**Key Metrics**:
- Target: <5ms per transaction (includes PoW)
- Target: <1µs per balance query
- Target: <10µs for tip selection (100 tips)

### 3. Task Queue Operations (3 benchmarks)

Tests distributed task processing performance:

- **bench_task_creation**: Creating task objects
- **bench_task_queue_operations**: Submit/claim cycle
- **bench_parallel_task_processing**: Concurrent task handling

**Key Metrics**:
- Target: <100µs per task creation
- Target: <1ms per submit/claim
- Target: 100+ tasks/second throughput

### 4. Security Operations (6 benchmarks)

Tests adaptive security and Q-learning performance:

- **bench_qlearning_decision**: Q-learning action selection
- **bench_qlearning_update**: Q-table updates
- **bench_attack_pattern_matching**: Pattern similarity detection
- **bench_threshold_updates**: Adaptive threshold adjustment
- **bench_rate_limiter**: Rate limiting checks
- **bench_reputation_update**: Reputation score updates

**Key Metrics**:
- Target: <1µs per Q-learning decision
- Target: <5µs per attack detection
- Target: <100ns per rate limit check

### 5. Network Topology Operations (4 benchmarks)

Tests network organization and peer selection:

- **bench_node_registration_1k**: Registering 1,000 nodes
- **bench_node_registration_10k**: Registering 10,000 nodes
- **bench_optimal_peer_selection**: Finding best peers
- **bench_cluster_assignment**: Capability-based clustering

**Key Metrics**:
- Target: <50ms for 1K node registration
- Target: <500ms for 10K node registration
- Target: <10µs per peer selection

### 6. Economic Engine Operations (3 benchmarks)

Tests reward distribution and sustainability:

- **bench_reward_distribution**: Processing task rewards
- **bench_epoch_processing**: Economic epoch transitions
- **bench_sustainability_check**: Network health verification

**Key Metrics**:
- Target: <5µs per reward distribution
- Target: <100µs per epoch processing
- Target: <1µs per sustainability check

### 7. Evolution Engine Operations (3 benchmarks)

Tests network evolution and optimization:

- **bench_performance_recording**: Recording node metrics
- **bench_replication_check**: Checking if nodes should replicate
- **bench_evolution_step**: Evolution generation advancement

**Key Metrics**:
- Target: <1µs per performance record
- Target: <100ns per replication check
- Target: <10µs per evolution step

### 8. Optimization Engine Operations (2 benchmarks)

Tests intelligent task routing:

- **bench_routing_record**: Recording routing outcomes
- **bench_optimal_node_selection**: Selecting best node for task

**Key Metrics**:
- Target: <5µs per routing record
- Target: <10µs per optimal node selection

### 9. Network Manager Operations (2 benchmarks)

Tests P2P peer management:

- **bench_peer_registration**: Adding new peers
- **bench_worker_selection**: Selecting workers for tasks

**Key Metrics**:
- Target: <1µs per peer registration
- Target: <20µs for selecting 5 workers from 100

### 10. End-to-End Operations (2 benchmarks)

Tests complete workflows:

- **bench_full_task_lifecycle**: Create → Submit → Claim → Complete
- **bench_network_coordination**: Multi-node coordination

**Key Metrics**:
- Target: <10ms per complete task lifecycle
- Target: <100µs for coordinating 50 nodes

## Interpreting Results

### Sample Output

```
test bench_credit_operation           ... bench:         847 ns/iter (+/- 23)
test bench_balance_calculation         ... bench:      12,450 ns/iter (+/- 340)
test bench_qdag_transaction_creation   ... bench:   4,567,890 ns/iter (+/- 89,234)
```

### Understanding Metrics

- **ns/iter**: Nanoseconds per iteration (1ns = 0.000001ms)
- **(+/- N)**: Standard deviation (lower is more consistent)
- **Throughput**: Calculate as 1,000,000,000 / ns_per_iter ops/second

### Performance Grades

| ns/iter Range | Grade | Assessment |
|---------------|-------|------------|
| < 1,000 | A+ | Excellent - sub-microsecond |
| 1,000 - 10,000 | A | Good - low microsecond |
| 10,000 - 100,000 | B | Acceptable - tens of microseconds |
| 100,000 - 1,000,000 | C | Needs optimization - hundreds of µs |
| > 1,000,000 | D | Critical - millisecond range |

## Optimization Tracking

### Known Bottlenecks (Pre-Optimization)

1. **balance_calculation**: ~12µs (1000 transactions)
   - **Issue**: O(n) iteration over all transactions
   - **Fix**: Cached balance field
   - **Target**: <100ns

2. **attack_pattern_matching**: ~500µs (100 patterns)
   - **Issue**: Linear scan through patterns
   - **Fix**: KD-Tree spatial index
   - **Target**: <5µs

3. **optimal_node_selection**: ~1ms (1000 history items)
   - **Issue**: Filter + aggregate on every call
   - **Fix**: Pre-aggregated routing stats
   - **Target**: <10µs

### Optimization Roadmap

See [performance-analysis.md](./performance-analysis.md) for detailed breakdown.

## Continuous Benchmarking

### CI/CD Integration

```yaml
# .github/workflows/benchmarks.yml
name: Performance Benchmarks

on:
  push:
    branches: [main, develop]
  pull_request:

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@nightly
      - name: Run benchmarks
        run: cargo +nightly bench --features=bench
      - name: Compare to baseline
        run: cargo benchcmp baseline.txt current.txt
```

### Local Baseline Tracking

```bash
# Save baseline
cargo bench --features=bench > baseline.txt

# After optimizations
cargo bench --features=bench > optimized.txt

# Compare
cargo install cargo-benchcmp
cargo benchcmp baseline.txt optimized.txt
```

## Profiling

### CPU Profiling

```bash
# Using cargo-flamegraph
cargo install flamegraph
cargo flamegraph --bench benchmarks --features=bench

# Using perf (Linux)
perf record --call-graph dwarf cargo bench --features=bench
perf report
```

### Memory Profiling

```bash
# Using valgrind/massif
valgrind --tool=massif target/release/deps/edge_net_benchmarks
ms_print massif.out.* > memory-profile.txt

# Using heaptrack
heaptrack target/release/deps/edge_net_benchmarks
heaptrack_gui heaptrack.edge_net_benchmarks.*
```

### WASM Profiling

```bash
# Build WASM with profiling
wasm-pack build --profiling

# Profile in browser
# 1. Load WASM module
# 2. Open Chrome DevTools > Performance
# 3. Record while running operations
# 4. Analyze flame graph
```

## Load Testing

### Stress Test Scenarios

```rust
#[test]
fn stress_test_10k_transactions() {
    let mut ledger = WasmCreditLedger::new("stress-node".to_string()).unwrap();

    let start = Instant::now();
    for i in 0..10_000 {
        ledger.credit(100, &format!("task-{}", i)).unwrap();
    }
    let duration = start.elapsed();

    println!("10K transactions: {:?}", duration);
    println!("Throughput: {:.0} tx/sec", 10_000.0 / duration.as_secs_f64());

    assert!(duration < Duration::from_secs(1)); // <1s for 10K transactions
}
```

### Concurrency Testing

```rust
#[test]
fn concurrent_task_processing() {
    use tokio::runtime::Runtime;

    let rt = Runtime::new().unwrap();
    let start = Instant::now();

    rt.block_on(async {
        let mut handles = vec![];

        for _ in 0..100 {
            handles.push(tokio::spawn(async {
                // Simulate task processing
                for _ in 0..100 {
                    // Process task
                }
            }));
        }

        futures::future::join_all(handles).await;
    });

    let duration = start.elapsed();
    println!("100 concurrent workers, 100 tasks each: {:?}", duration);
}
```

## Benchmark Development

### Adding New Benchmarks

```rust
#[bench]
fn bench_new_operation(b: &mut Bencher) {
    // Setup
    let mut state = setup_test_state();

    // Benchmark
    b.iter(|| {
        // Operation to benchmark
        state.perform_operation();
    });

    // Optional: teardown
    drop(state);
}
```

### Best Practices

1. **Minimize setup**: Do setup outside `b.iter()`
2. **Use `test::black_box()`**: Prevent compiler optimizations
3. **Consistent state**: Reset state between iterations if needed
4. **Realistic data**: Use production-like data sizes
5. **Multiple scales**: Test with 10, 100, 1K, 10K items

### Example with black_box

```rust
#[bench]
fn bench_with_black_box(b: &mut Bencher) {
    let input = vec![1, 2, 3, 4, 5];

    b.iter(|| {
        let result = expensive_computation(test::black_box(&input));
        test::black_box(result) // Prevent optimization of result
    });
}
```

## Performance Targets by Scale

### Small Network (< 100 nodes)

- Task throughput: 1,000 tasks/sec
- Balance queries: 100,000 ops/sec
- Attack detection: 10,000 requests/sec

### Medium Network (100 - 10K nodes)

- Task throughput: 10,000 tasks/sec
- Balance queries: 50,000 ops/sec (with caching)
- Peer selection: 1,000 selections/sec

### Large Network (> 10K nodes)

- Task throughput: 100,000 tasks/sec
- Balance queries: 10,000 ops/sec (distributed)
- Network coordination: 500 ops/sec

## Troubleshooting

### Benchmarks Won't Compile

```bash
# Ensure nightly toolchain
rustup install nightly
rustup default nightly

# Update dependencies
cargo update

# Clean build
cargo clean
cargo bench --features=bench
```

### Inconsistent Results

```bash
# Increase iteration count
BENCHER_ITERS=10000 cargo bench --features=bench

# Disable CPU frequency scaling (Linux)
sudo cpupower frequency-set --governor performance

# Close background applications
# Run multiple times and average
```

### Memory Issues

```bash
# Increase stack size
RUST_MIN_STACK=16777216 cargo bench --features=bench

# Reduce test data size
# Check for memory leaks with valgrind
```

## References

- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Criterion.rs](https://github.com/bheisler/criterion.rs) (alternative framework)
- [cargo-bench documentation](https://doc.rust-lang.org/cargo/commands/cargo-bench.html)
- [Performance Analysis Document](./performance-analysis.md)

## Contributing

When adding features, include benchmarks:

1. Add benchmark in `src/bench.rs`
2. Document expected performance in this README
3. Run baseline before optimization
4. Run after optimization and document improvement
5. Add to CI/CD pipeline

---

**Last Updated**: 2025-01-01
**Benchmark Count**: 40+
**Coverage**: All critical operations
