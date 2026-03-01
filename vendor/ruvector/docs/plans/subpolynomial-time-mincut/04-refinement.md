# SPARC Phase 4: Refinement - Test-Driven Development Plan

## Overview

This phase details the Test-Driven Development (TDD) strategy for implementing the subpolynomial-time dynamic minimum cut algorithm. We follow a rigorous red-green-refactor cycle with comprehensive test coverage.

## 1. TDD Strategy

### 1.1 Development Cycles

**Cycle Structure**:
1. **RED**: Write failing test that specifies desired behavior
2. **GREEN**: Write minimal code to make test pass
3. **REFACTOR**: Improve code quality without changing behavior
4. **VALIDATE**: Run full test suite + benchmarks

### 1.2 Test-First Order

Implement modules in dependency order:
1. `error.rs` - Error types (foundation)
2. `graph/representation.rs` - Basic graph structure
3. `linkcut/node.rs` - Link-cut tree nodes
4. `linkcut/operations.rs` - LCT operations
5. `tree/decomposition.rs` - Hierarchical tree
6. `algorithm/insert.rs` - Edge insertion
7. `algorithm/delete.rs` - Edge deletion
8. `algorithm/query.rs` - Cut queries
9. `sparsify/sampler.rs` - Sparsification
10. `monitoring/callbacks.rs` - Monitoring system

## 2. Unit Tests

### 2.1 Error Handling Tests

**File**: `tests/unit/error_tests.rs`

```rust
#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn test_invalid_vertex_error() {
        let err = MinCutError::InvalidVertex(999);
        assert_eq!(err.to_string(), "Invalid vertex ID: 999");
    }

    #[test]
    fn test_edge_not_found_error() {
        let err = MinCutError::EdgeNotFound(1, 2);
        assert!(err.to_string().contains("Edge (1, 2) does not exist"));
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MinCutError>();
    }
}
```

### 2.2 Graph Representation Tests

**File**: `tests/unit/graph_tests.rs`

```rust
#[cfg(test)]
mod graph_tests {
    use ruvector_mincut::graph::*;

    #[test]
    fn test_empty_graph() {
        let graph = DynamicGraph::new(0);
        assert_eq!(graph.vertex_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_add_edge() {
        let mut graph = DynamicGraph::new(3);
        assert!(graph.add_edge(0, 1).is_ok());
        assert_eq!(graph.edge_count(), 1);
        assert!(graph.has_edge(0, 1));
        assert!(graph.has_edge(1, 0)); // Undirected
    }

    #[test]
    fn test_add_duplicate_edge() {
        let mut graph = DynamicGraph::new(3);
        graph.add_edge(0, 1).unwrap();
        let result = graph.add_edge(0, 1);
        assert!(result.is_err());
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn test_remove_edge() {
        let mut graph = DynamicGraph::new(3);
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(1, 2).unwrap();

        assert!(graph.remove_edge(0, 1).is_ok());
        assert_eq!(graph.edge_count(), 1);
        assert!(!graph.has_edge(0, 1));
    }

    #[test]
    fn test_remove_nonexistent_edge() {
        let mut graph = DynamicGraph::new(3);
        let result = graph.remove_edge(0, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_neighbors() {
        let mut graph = DynamicGraph::new(4);
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(0, 2).unwrap();
        graph.add_edge(0, 3).unwrap();

        let neighbors: Vec<_> = graph.neighbors(0).collect();
        assert_eq!(neighbors.len(), 3);
        assert!(neighbors.contains(&1));
        assert!(neighbors.contains(&2));
        assert!(neighbors.contains(&3));
    }

    #[test]
    fn test_degree() {
        let mut graph = DynamicGraph::new(4);
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(0, 2).unwrap();

        assert_eq!(graph.degree(0), 2);
        assert_eq!(graph.degree(1), 1);
        assert_eq!(graph.degree(2), 1);
        assert_eq!(graph.degree(3), 0);
    }
}
```

### 2.3 Link-Cut Tree Tests

**File**: `tests/unit/linkcut_tests.rs`

```rust
#[cfg(test)]
mod linkcut_tests {
    use ruvector_mincut::linkcut::*;

    #[test]
    fn test_make_tree() {
        let mut lct = LinkCutTree::new();
        lct.make_tree(0);
        lct.make_tree(1);

        assert!(!lct.connected(0, 1));
    }

    #[test]
    fn test_link() {
        let mut lct = LinkCutTree::new();
        lct.make_tree(0);
        lct.make_tree(1);

        lct.link(0, 1);
        assert!(lct.connected(0, 1));
    }

    #[test]
    fn test_cut() {
        let mut lct = LinkCutTree::new();
        lct.make_tree(0);
        lct.make_tree(1);
        lct.link(0, 1);

        lct.cut(0, 1);
        assert!(!lct.connected(0, 1));
    }

    #[test]
    fn test_connected_transitive() {
        let mut lct = LinkCutTree::new();
        for i in 0..5 {
            lct.make_tree(i);
        }

        // Build path: 0-1-2-3-4
        lct.link(0, 1);
        lct.link(1, 2);
        lct.link(2, 3);
        lct.link(3, 4);

        assert!(lct.connected(0, 4));
        assert!(lct.connected(1, 3));
    }

    #[test]
    fn test_lca() {
        let mut lct = LinkCutTree::new();
        for i in 0..7 {
            lct.make_tree(i);
        }

        // Build tree:
        //       0
        //      / \
        //     1   2
        //    / \
        //   3   4
        //      / \
        //     5   6

        lct.link(1, 0);
        lct.link(2, 0);
        lct.link(3, 1);
        lct.link(4, 1);
        lct.link(5, 4);
        lct.link(6, 4);

        assert_eq!(lct.lca(3, 4), 1);
        assert_eq!(lct.lca(3, 2), 0);
        assert_eq!(lct.lca(5, 6), 4);
    }

    #[test]
    fn test_path_aggregate() {
        let mut lct = LinkCutTree::new();
        for i in 0..4 {
            lct.make_tree(i);
        }

        lct.link(0, 1);
        lct.link(1, 2);
        lct.link(2, 3);

        // Test aggregate queries on path
        let path_size = lct.path_aggregate(0, 3, |agg| agg.size);
        assert_eq!(path_size, 4);
    }
}
```

### 2.4 Decomposition Tree Tests

**File**: `tests/unit/decomposition_tests.rs`

```rust
#[cfg(test)]
mod decomposition_tests {
    use ruvector_mincut::tree::*;

    #[test]
    fn test_empty_tree() {
        let tree = DecompositionTree::new();
        assert_eq!(tree.height(), 0);
    }

    #[test]
    fn test_build_from_graph() {
        let mut graph = DynamicGraph::new(4);
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(1, 2).unwrap();
        graph.add_edge(2, 3).unwrap();
        graph.add_edge(3, 0).unwrap();

        let tree = DecompositionTree::from_graph(&graph);

        assert_eq!(tree.height(), 2); // log_2(4) = 2
        assert_eq!(tree.leaf_count(), 4);
    }

    #[test]
    fn test_find_leaf() {
        let mut graph = DynamicGraph::new(4);
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(2, 3).unwrap();

        let tree = DecompositionTree::from_graph(&graph);

        let leaf_0 = tree.find_leaf(0);
        assert!(leaf_0.is_some());
        assert_eq!(leaf_0.unwrap().vertices(), &[0]);
    }

    #[test]
    fn test_local_cut_values() {
        // Create complete graph K4
        let mut graph = DynamicGraph::new(4);
        for i in 0..4 {
            for j in i+1..4 {
                graph.add_edge(i, j).unwrap();
            }
        }

        let tree = DecompositionTree::from_graph(&graph);

        // In K4, minimum cut is 3 (any single vertex)
        let root = tree.root();
        assert_eq!(root.local_cut(), 3);
    }
}
```

## 3. Integration Tests

### 3.1 End-to-End Dynamic Updates

**File**: `tests/integration/dynamic_updates_test.rs`

```rust
#[cfg(test)]
mod dynamic_updates_tests {
    use ruvector_mincut::*;

    #[test]
    fn test_insert_edges_sequential() {
        let mut mincut = DynamicMinCut::new(MinCutConfig::default());

        // Build path graph: 0-1-2-3
        mincut.insert_edge(0, 1).unwrap();
        assert_eq!(mincut.min_cut_value(), 1);

        mincut.insert_edge(1, 2).unwrap();
        assert_eq!(mincut.min_cut_value(), 1);

        mincut.insert_edge(2, 3).unwrap();
        assert_eq!(mincut.min_cut_value(), 1);
    }

    #[test]
    fn test_delete_edges_creates_bottleneck() {
        let mut mincut = DynamicMinCut::new(MinCutConfig::default());

        // Build K4
        for i in 0..4 {
            for j in i+1..4 {
                mincut.insert_edge(i, j).unwrap();
            }
        }
        assert_eq!(mincut.min_cut_value(), 3);

        // Remove edges to create bottleneck
        mincut.delete_edge(0, 2).unwrap();
        mincut.delete_edge(0, 3).unwrap();
        mincut.delete_edge(1, 2).unwrap();
        mincut.delete_edge(1, 3).unwrap();

        // Now min cut is 1 (between {0,1} and {2,3})
        assert_eq!(mincut.min_cut_value(), 1);
    }

    #[test]
    fn test_partition_correctness() {
        let mut mincut = DynamicMinCut::new(MinCutConfig::default());

        // Build dumbbell graph: K3 - single edge - K3
        // Left clique
        mincut.insert_edge(0, 1).unwrap();
        mincut.insert_edge(1, 2).unwrap();
        mincut.insert_edge(2, 0).unwrap();

        // Bridge
        mincut.insert_edge(2, 3).unwrap();

        // Right clique
        mincut.insert_edge(3, 4).unwrap();
        mincut.insert_edge(4, 5).unwrap();
        mincut.insert_edge(5, 3).unwrap();

        let result = mincut.min_cut();
        assert_eq!(result.value, 1);
        assert_eq!(result.cut_edges.len(), 1);
        assert!(result.cut_edges.contains(&(2, 3)) ||
                result.cut_edges.contains(&(3, 2)));

        // Verify partition sizes
        assert_eq!(result.partition_a.len(), 3);
        assert_eq!(result.partition_b.len(), 3);
    }
}
```

### 3.2 Correctness Verification

**File**: `tests/integration/correctness_test.rs`

```rust
#[cfg(test)]
mod correctness_tests {
    use ruvector_mincut::*;
    use ruvector_mincut::testing::*;

    #[test]
    fn test_against_stoer_wagner() {
        // Verify our algorithm matches Stoer-Wagner on various graphs
        for size in [10, 20, 50, 100] {
            for density in [0.1, 0.3, 0.5, 0.7] {
                let graph = generate_random_graph(size, density);

                let mut mincut = DynamicMinCut::from_graph(&graph, Default::default());
                let our_result = mincut.min_cut_value();

                let stoer_wagner = brute_force_mincut(&graph);

                assert_eq!(our_result, stoer_wagner,
                    "Mismatch for n={}, density={}", size, density);
            }
        }
    }

    #[test]
    fn test_random_update_sequences() {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for trial in 0..100 {
            let mut mincut = DynamicMinCut::new(MinCutConfig::default());
            let n = 20;

            // Random sequence of 100 updates
            for _ in 0..100 {
                let u = rng.gen_range(0..n);
                let v = rng.gen_range(0..n);
                if u == v { continue; }

                if rng.gen_bool(0.5) {
                    // Insert
                    mincut.insert_edge(u, v).ok();
                } else {
                    // Delete
                    mincut.delete_edge(u, v).ok();
                }

                // Verify invariants
                #[cfg(debug_assertions)]
                mincut.validate().unwrap();
            }

            // Final verification against ground truth
            let computed = mincut.min_cut_value();
            let actual = brute_force_mincut(&mincut.graph);
            assert_eq!(computed, actual, "Trial {} failed", trial);
        }
    }
}
```

### 3.3 Sparsification Tests

**File**: `tests/integration/sparsification_test.rs`

```rust
#[cfg(test)]
mod sparsification_tests {
    use ruvector_mincut::*;

    #[test]
    fn test_approximate_within_epsilon() {
        let epsilon = 0.1;
        let config = MinCutConfig {
            epsilon,
            use_sparsification: true,
            ..Default::default()
        };

        for size in [100, 500, 1000] {
            let graph = generate_random_graph(size, 0.1);

            let mut mincut = DynamicMinCut::from_graph(&graph, config);
            let approximate = mincut.min_cut_value() as f64;

            let exact = brute_force_mincut(&graph) as f64;

            let ratio = approximate / exact;
            assert!(ratio >= 1.0 - epsilon && ratio <= 1.0 + epsilon,
                "Approximation ratio {} outside bounds for n={}", ratio, size);
        }
    }

    #[test]
    fn test_sparse_graph_size() {
        let epsilon = 0.05;
        let config = MinCutConfig {
            epsilon,
            use_sparsification: true,
            ..Default::default()
        };

        let n = 1000;
        let graph = generate_random_graph(n, 0.5); // Dense graph

        let mut mincut = DynamicMinCut::from_graph(&graph, config);

        // Sparse graph should have O(n log n / ε²) edges
        let sparse_size = mincut.sparse_graph_size();
        let expected_max = ((n as f64) * (n as f64).ln() / (epsilon * epsilon)) as usize;

        assert!(sparse_size < expected_max * 2,
            "Sparse graph too large: {} > {}", sparse_size, expected_max);
    }
}
```

## 4. Performance Tests

### 4.1 Benchmark Suite

**File**: `benches/mincut_bench.rs`

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ruvector_mincut::*;

fn bench_insert_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_operations");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("insert_edge", size),
            size,
            |b, &n| {
                b.iter_batched(
                    || {
                        let mut mincut = DynamicMinCut::new(Default::default());
                        let updates = generate_random_insertions(n, 0.1);
                        (mincut, updates)
                    },
                    |(mut mincut, updates)| {
                        for (u, v) in updates {
                            black_box(mincut.insert_edge(u, v).ok());
                        }
                    },
                    criterion::BatchSize::SmallInput
                );
            }
        );
    }

    group.finish();
}

fn bench_delete_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete_operations");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("delete_edge", size),
            size,
            |b, &n| {
                b.iter_batched(
                    || {
                        let graph = generate_random_graph(n, 0.1);
                        let mut mincut = DynamicMinCut::from_graph(&graph, Default::default());
                        let edges: Vec<_> = graph.edges().collect();
                        (mincut, edges)
                    },
                    |(mut mincut, edges)| {
                        for (u, v) in edges.iter().take(100) {
                            black_box(mincut.delete_edge(*u, *v).ok());
                        }
                    },
                    criterion::BatchSize::SmallInput
                );
            }
        );
    }

    group.finish();
}

fn bench_query_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_operations");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("min_cut_value", size),
            size,
            |b, &n| {
                let graph = generate_random_graph(n, 0.1);
                let mincut = DynamicMinCut::from_graph(&graph, Default::default());

                b.iter(|| {
                    black_box(mincut.min_cut_value());
                });
            }
        );

        group.bench_with_input(
            BenchmarkId::new("min_cut_partition", size),
            size,
            |b, &n| {
                let graph = generate_random_graph(n, 0.1);
                let mincut = DynamicMinCut::from_graph(&graph, Default::default());

                b.iter(|| {
                    black_box(mincut.min_cut());
                });
            }
        );
    }

    group.finish();
}

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("realistic_workload", size),
            size,
            |b, &n| {
                b.iter_batched(
                    || {
                        let graph = generate_random_graph(n, 0.05);
                        let mut mincut = DynamicMinCut::from_graph(&graph, Default::default());
                        let ops = generate_mixed_operations(1000, n);
                        (mincut, ops)
                    },
                    |(mut mincut, ops)| {
                        for op in ops {
                            match op {
                                Op::Insert(u, v) => {
                                    black_box(mincut.insert_edge(u, v).ok());
                                },
                                Op::Delete(u, v) => {
                                    black_box(mincut.delete_edge(u, v).ok());
                                },
                                Op::Query => {
                                    black_box(mincut.min_cut_value());
                                }
                            }
                        }
                    },
                    criterion::BatchSize::SmallInput
                );
            }
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_insert_operations,
    bench_delete_operations,
    bench_query_operations,
    bench_mixed_workload
);
criterion_main!(benches);
```

### 4.2 Performance Regression Tests

**File**: `tests/performance/regression_test.rs`

```rust
#[cfg(test)]
mod performance_regression_tests {
    use ruvector_mincut::*;
    use std::time::Instant;

    #[test]
    #[ignore] // Run with: cargo test --release -- --ignored
    fn test_update_time_subpolynomial() {
        let n = 10_000;
        let num_updates = 10_000;

        let mut mincut = DynamicMinCut::new(Default::default());
        let updates = generate_random_mixed_operations(num_updates, n);

        let start = Instant::now();
        for op in updates {
            match op {
                Op::Insert(u, v) => mincut.insert_edge(u, v).ok(),
                Op::Delete(u, v) => mincut.delete_edge(u, v).ok(),
                _ => continue,
            };
        }
        let duration = start.elapsed();

        let avg_ns = duration.as_nanos() / num_updates as u128;
        let avg_ms = avg_ns as f64 / 1_000_000.0;

        println!("Average update time: {:.3} ms", avg_ms);

        // Target: <10ms per update for n=10,000
        assert!(avg_ms < 10.0,
            "Update time {} ms exceeds target of 10 ms", avg_ms);
    }

    #[test]
    #[ignore]
    fn test_query_time_constant() {
        let n = 10_000;
        let graph = generate_random_graph(n, 0.1);
        let mincut = DynamicMinCut::from_graph(&graph, Default::default());

        let num_queries = 100_000;
        let start = Instant::now();
        for _ in 0..num_queries {
            black_box(mincut.min_cut_value());
        }
        let duration = start.elapsed();

        let avg_ns = duration.as_nanos() / num_queries;

        println!("Average query time: {} ns", avg_ns);

        // Target: <100ns per query (essentially O(1))
        assert!(avg_ns < 100,
            "Query time {} ns exceeds target of 100 ns", avg_ns);
    }

    #[test]
    #[ignore]
    fn test_throughput_target() {
        let n = 10_000;
        let mut mincut = DynamicMinCut::new(Default::default());
        let updates = generate_random_mixed_operations(10_000, n);

        let start = Instant::now();
        for op in updates {
            match op {
                Op::Insert(u, v) => mincut.insert_edge(u, v).ok(),
                Op::Delete(u, v) => mincut.delete_edge(u, v).ok(),
                _ => continue,
            };
        }
        let duration = start.elapsed();

        let throughput = 10_000.0 / duration.as_secs_f64();

        println!("Throughput: {:.0} updates/second", throughput);

        // Target: >1,000 updates/second for n=10,000
        assert!(throughput > 1000.0,
            "Throughput {} ops/s below target of 1000 ops/s", throughput);
    }
}
```

## 5. Property-Based Testing

**File**: `tests/property/quickcheck_tests.rs`

```rust
use quickcheck::{quickcheck, TestResult};
use ruvector_mincut::*;

#[cfg(test)]
mod property_tests {
    use super::*;

    #[quickcheck]
    fn prop_cut_value_nonnegative(ops: Vec<GraphOp>) -> TestResult {
        if ops.len() > 1000 {
            return TestResult::discard();
        }

        let mut mincut = DynamicMinCut::new(Default::default());

        for op in ops {
            apply_operation(&mut mincut, op);
        }

        TestResult::from_bool(mincut.min_cut_value() >= 0)
    }

    #[quickcheck]
    fn prop_cut_bounded_by_min_degree(ops: Vec<GraphOp>) -> TestResult {
        if ops.len() > 500 {
            return TestResult::discard();
        }

        let mut mincut = DynamicMinCut::new(Default::default());

        for op in ops {
            apply_operation(&mut mincut, op);
        }

        let cut_value = mincut.min_cut_value();
        let min_degree = compute_min_degree(&mincut.graph);

        TestResult::from_bool(cut_value <= min_degree)
    }

    #[quickcheck]
    fn prop_insert_delete_inverse(u: u32, v: u32) -> TestResult {
        if u == v || u > 100 || v > 100 {
            return TestResult::discard();
        }

        let mut mincut = DynamicMinCut::new(Default::default());

        let cut_before = mincut.min_cut_value();

        mincut.insert_edge(u, v).ok();
        let cut_after_insert = mincut.min_cut_value();

        mincut.delete_edge(u, v).ok();
        let cut_after_delete = mincut.min_cut_value();

        TestResult::from_bool(cut_before == cut_after_delete)
    }
}
```

## 6. Fuzzing

**File**: `fuzz/fuzz_targets/mincut_fuzz.rs`

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use ruvector_mincut::*;

fuzz_target!(|data: &[u8]| {
    if data.len() < 10 {
        return;
    }

    let mut mincut = DynamicMinCut::new(Default::default());
    let n = 100;

    let mut i = 0;
    while i + 2 < data.len() {
        let u = (data[i] as usize) % n;
        let v = (data[i+1] as usize) % n;
        let op_type = data[i+2] % 3;

        match op_type {
            0 => {
                // Insert
                mincut.insert_edge(u, v).ok();
            },
            1 => {
                // Delete
                mincut.delete_edge(u, v).ok();
            },
            2 => {
                // Query
                let _ = mincut.min_cut_value();
            },
            _ => unreachable!()
        }

        // Verify invariants
        #[cfg(debug_assertions)]
        mincut.validate().expect("Invariant violated");

        i += 3;
    }
});
```

## 7. Test Coverage Goals

### 7.1 Coverage Targets

- **Unit tests**: >90% line coverage
- **Integration tests**: >80% branch coverage
- **Property tests**: >100 successful runs per property
- **Fuzzing**: >1 million iterations without crash

### 7.2 Coverage Measurement

```bash
# Install coverage tool
cargo install cargo-tarpaulin

# Run coverage
cargo tarpaulin --out Html --output-dir coverage/

# View coverage report
open coverage/index.html
```

## 8. Continuous Integration

### 8.1 CI Pipeline

```yaml
# .github/workflows/test.yml
name: Test Suite

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run unit tests
        run: cargo test --lib

      - name: Run integration tests
        run: cargo test --test '*'

      - name: Run benchmarks (sanity check)
        run: cargo bench --no-run

      - name: Check code coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Lcov

      - name: Upload coverage
        uses: codecov/codecov-action@v2

  bench:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run performance tests
        run: cargo test --release -- --ignored
```

## 9. Test Data Generation

**File**: `tests/fixtures/test_graphs.rs`

```rust
use rand::Rng;

pub fn generate_random_graph(n: usize, density: f64) -> DynamicGraph {
    let mut graph = DynamicGraph::new(n);
    let mut rng = rand::thread_rng();

    for u in 0..n {
        for v in u+1..n {
            if rng.gen_bool(density) {
                graph.add_edge(u, v).unwrap();
            }
        }
    }

    graph
}

pub fn generate_complete_graph(n: usize) -> DynamicGraph {
    let mut graph = DynamicGraph::new(n);
    for u in 0..n {
        for v in u+1..n {
            graph.add_edge(u, v).unwrap();
        }
    }
    graph
}

pub fn generate_path_graph(n: usize) -> DynamicGraph {
    let mut graph = DynamicGraph::new(n);
    for i in 0..n-1 {
        graph.add_edge(i, i+1).unwrap();
    }
    graph
}

pub fn generate_cycle_graph(n: usize) -> DynamicGraph {
    let mut graph = DynamicGraph::new(n);
    for i in 0..n {
        graph.add_edge(i, (i+1) % n).unwrap();
    }
    graph
}

pub fn generate_dumbbell_graph(clique_size: usize) -> DynamicGraph {
    let n = clique_size * 2;
    let mut graph = DynamicGraph::new(n);

    // Left clique
    for u in 0..clique_size {
        for v in u+1..clique_size {
            graph.add_edge(u, v).unwrap();
        }
    }

    // Bridge
    graph.add_edge(clique_size-1, clique_size).unwrap();

    // Right clique
    for u in clique_size..n {
        for v in u+1..n {
            graph.add_edge(u, v).unwrap();
        }
    }

    graph
}
```

## 10. Validation & Debugging

### 10.1 Invariant Checking

```rust
impl DynamicMinCut {
    #[cfg(debug_assertions)]
    pub fn validate(&self) -> Result<()> {
        // Check tree structure
        self.tree.validate()?;

        // Check LCT consistency
        self.lct.validate()?;

        // Verify cut value
        let computed = self.current_cut;
        let actual = self.compute_min_cut_brute_force();
        if computed != actual {
            return Err(MinCutError::InvariantViolation(
                format!("Cut value mismatch: {} != {}", computed, actual)
            ));
        }

        Ok(())
    }
}
```

---

**Next Phase**: Proceed to `05-completion.md` for integration, deployment, and documentation.
