# RuVector MinCut Benchmarks

Comprehensive benchmark suite for the dynamic minimum cut algorithm implementation.

## Overview

This benchmark suite measures the performance characteristics of the dynamic graph operations that underpin the minimum cut algorithm, including:

- **Edge Operations**: Insert/delete throughput and scaling
- **Query Performance**: Connectivity, degree, edge existence checks
- **Workload Patterns**: Mixed operations, batch processing
- **Scaling Analysis**: Verification of subpolynomial update time (O(n^(2/3)))
- **Graph Types**: Random, grid, complete, sparse, and dense graphs

## Running Benchmarks

### Run All Benchmarks

```bash
cd crates/ruvector-mincut
cargo bench --bench mincut_bench
```

### Run Specific Benchmark Groups

```bash
# Graph operations (insert, delete, queries)
cargo bench --bench mincut_bench graph_ops

# Query operations
cargo bench --bench mincut_bench queries

# Workload patterns
cargo bench --bench mincut_bench workloads

# Scaling analysis
cargo bench --bench mincut_bench scaling
```

### Run Individual Benchmarks

```bash
# Insert edge performance
cargo bench --bench mincut_bench -- insert_edge

# Delete edge performance
cargo bench --bench mincut_bench -- delete_edge

# Mixed workload
cargo bench --bench mincut_bench -- mixed_workload

# Scaling analysis
cargo bench --bench mincut_bench -- scaling_analysis
```

### Filter by Size

```bash
# Benchmark only size 1000
cargo bench --bench mincut_bench -- /1000

# Benchmark sizes 100 and 5000
cargo bench --bench mincut_bench -- "/100|5000"
```

## Benchmark Groups

### 1. Graph Operations (`graph_ops`)

- **`insert_edge`**: Edge insertion throughput at various graph sizes (100 to 10K vertices)
- **`delete_edge`**: Edge deletion throughput
- **`degree_query`**: Vertex degree query performance
- **`has_edge_query`**: Edge existence check performance

### 2. Query Operations (`queries`)

- **`query_connectivity`**: Graph connectivity checking (BFS-based)
- **`stats_computation`**: Graph statistics calculation
- **`connected_components`**: Connected components computation
- **`neighbors_iteration`**: Neighbor list retrieval

### 3. Workload Patterns (`workloads`)

- **`mixed_workload`**: Realistic mixed operations (50% insert, 30% delete, 20% query)
- **`batch_operations`**: Batch insertion performance (10 to 1000 edges)

### 4. Scaling Analysis (`scaling`)

- **`scaling_analysis`**: Verify subpolynomial O(n^(2/3)) update time
  - Tests sizes: 100, 316, 1K, 3.2K, 10K vertices
  - Both insert and delete operations
- **`graph_types`**: Performance across different graph structures
  - Random graphs
  - Grid graphs (structured)
  - Complete graphs (dense)
  - Sparse graphs (avg degree ~4)
  - Dense graphs (~30% edge density)
- **`memory_efficiency`**: Graph creation and memory footprint

## Graph Generators

The benchmark suite includes several graph generators:

- **Random Graph**: `generate_random_graph(n, m, seed)` - n vertices, m random edges
- **Grid Graph**: `generate_grid_graph(width, height)` - Structured 2D grid
- **Complete Graph**: `generate_complete_graph(n)` - All possible edges
- **Sparse Graph**: `generate_sparse_graph(n, seed)` - Average degree ~4
- **Dense Graph**: `generate_dense_graph(n, seed)` - ~30% edge density

## Expected Performance Characteristics

Based on the theoretical analysis:

### Update Operations (Insert/Delete)
- **Time Complexity**: O(n^(2/3)) amortized
- **Scaling**: Should show subpolynomial growth
- **Throughput**: Higher for smaller graphs

### Query Operations
- **Connectivity**: O(n + m) via BFS
- **Degree**: O(1) via adjacency list lookup
- **Has Edge**: O(1) via hash table lookup
- **Connected Components**: O(n + m) via BFS

### Graph Types
- **Sparse graphs**: Better cache locality, faster updates
- **Dense graphs**: More edges to maintain, slower updates
- **Grid graphs**: Good locality, predictable performance
- **Complete graphs**: Maximum edges, highest overhead

## Interpreting Results

### Scaling Verification

The `scaling_analysis` benchmarks test sizes: 100, 316, 1000, 3162, 10000

For O(n^(2/3)) complexity:
- 100 → 316 (3.16x): expect ~2.1x slowdown
- 316 → 1000 (3.16x): expect ~2.1x slowdown
- 1000 → 3162 (3.16x): expect ~2.1x slowdown
- 3162 → 10000 (3.16x): expect ~2.1x slowdown

If you observe linear or worse scaling, there may be algorithmic issues.

### Throughput Metrics

Criterion reports:
- **Time/iteration**: Lower is better
- **Throughput**: Higher is better
- **R²**: Close to 1.0 indicates stable measurements

### Performance Baseline

Typical results on modern hardware (approximate):
- Small graphs (100-500): ~100-500 ns/operation
- Medium graphs (1K-5K): ~500ns-2μs/operation
- Large graphs (10K+): ~2-10μs/operation

## Customizing Benchmarks

### Adjust Sample Sizes

Modify `group.sample_size()` in the benchmark code:

```rust
let mut group = c.benchmark_group("my_group");
group.sample_size(50); // Default is 100
```

### Add Custom Graph Sizes

Edit the size arrays in each benchmark:

```rust
for size in [100, 500, 1000, 2500, 5000, 10000].iter() {
    // ...
}
```

### Change Workload Mix

Modify the `bench_mixed_workload` ratios:

```rust
match op {
    0..=5 => { /* 60% inserts */ },
    6..=8 => { /* 30% deletes */ },
    _ => { /* 10% queries */ }
}
```

## Output

Benchmark results are saved to:
- `target/criterion/`: HTML reports with graphs
- `target/criterion/{benchmark_name}/report/index.html`: Detailed reports

Open in browser:
```bash
open target/criterion/report/index.html
```

## Continuous Integration

Add to CI pipeline:

```yaml
- name: Run benchmarks
  run: |
    cd crates/ruvector-mincut
    cargo bench --bench mincut_bench -- --output-format bencher | tee bench_output.txt
```

Compare against baseline:
```bash
cargo install critcmp
critcmp baseline current
```

## Troubleshooting

### Benchmarks Take Too Long

Reduce sample size or iterations:
```rust
group.sample_size(10);
group.measurement_time(Duration::from_secs(5));
```

### Unstable Results

- Close other applications
- Disable CPU frequency scaling
- Run with `nice -n -20` for higher priority
- Check system load: `uptime`

### Out of Memory

Reduce graph sizes:
```rust
for size in [100, 500, 1000].iter() { // Skip 5K and 10K
```

## References

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- Dynamic Min-Cut Algorithm: O(n^(2/3)) update time
- Graph Connectivity: BFS in O(n + m)
