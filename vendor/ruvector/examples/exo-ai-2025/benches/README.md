# EXO-AI 2025 Performance Benchmarks

This directory contains comprehensive criterion-based benchmarks for the EXO-AI cognitive substrate.

## Benchmark Suites

### 1. Manifold Benchmarks (`manifold_bench.rs`)

**Purpose**: Measure geometric manifold operations for concept embedding and retrieval.

**Benchmarks**:
- `manifold_retrieval`: Query performance across different concept counts (100-5000)
- `manifold_deformation`: Batch embedding throughput (10-500 concepts)
- `manifold_local_adaptation`: Adaptive learning speed
- `manifold_curvature`: Geometric computation performance

**Expected Baselines** (on modern CPU):
- Retrieval @ 1000 concepts: < 100μs
- Deformation batch (100): < 1ms
- Local adaptation: < 50μs
- Curvature computation: < 10μs

### 2. Hypergraph Benchmarks (`hypergraph_bench.rs`)

**Purpose**: Measure higher-order relational reasoning performance.

**Benchmarks**:
- `hypergraph_edge_creation`: Hyperedge creation rate (2-50 nodes per edge)
- `hypergraph_query`: Incident edge queries (100-5000 edges)
- `hypergraph_pattern_match`: Pattern matching latency
- `hypergraph_subgraph_extraction`: Subgraph extraction speed

**Expected Baselines**:
- Edge creation (5 nodes): < 5μs
- Query @ 1000 edges: < 50μs
- Pattern matching: < 100μs
- Subgraph extraction (depth 2): < 200μs

### 3. Temporal Benchmarks (`temporal_bench.rs`)

**Purpose**: Measure temporal coordination and causal reasoning.

**Benchmarks**:
- `temporal_causal_query`: Causal ancestor queries (100-5000 events)
- `temporal_consolidation`: Memory consolidation time (100-1000 events)
- `temporal_range_query`: Time range query performance
- `temporal_causal_path`: Causal path finding
- `temporal_event_pruning`: Old event pruning speed

**Expected Baselines**:
- Causal query @ 1000 events: < 100μs
- Consolidation (500 events): < 5ms
- Range query: < 200μs
- Path finding (100 hops): < 500μs
- Pruning (5000 events): < 2ms

### 4. Federation Benchmarks (`federation_bench.rs`)

**Purpose**: Measure distributed coordination and consensus.

**Benchmarks**:
- `federation_crdt_merge`: CRDT operation throughput (10-500 ops)
- `federation_consensus`: Consensus round latency (3-10 nodes)
- `federation_state_sync`: State synchronization time
- `federation_crypto_sign`: Cryptographic signing speed
- `federation_crypto_verify`: Signature verification speed
- `federation_gossip`: Gossip propagation performance (5-50 nodes)

**Expected Baselines** (async operations):
- CRDT merge (100 ops): < 5ms
- Consensus (5 nodes): < 50ms
- State sync (100 items): < 10ms
- Sign operation: < 100μs
- Verify operation: < 150μs
- Gossip (10 nodes): < 20ms

## Running Benchmarks

### Run All Benchmarks
```bash
cargo bench
```

### Run Specific Suite
```bash
cargo bench --bench manifold_bench
cargo bench --bench hypergraph_bench
cargo bench --bench temporal_bench
cargo bench --bench federation_bench
```

### Run Specific Benchmark
```bash
cargo bench --bench manifold_bench -- manifold_retrieval
cargo bench --bench temporal_bench -- causal_query
```

### Generate Detailed Reports
```bash
cargo bench -- --save-baseline initial
cargo bench -- --baseline initial
```

## Benchmark Configuration

Criterion is configured with:
- HTML reports enabled (in `target/criterion/`)
- Statistical significance testing
- Outlier detection
- Performance regression detection

## Performance Targets

### Cognitive Operations (Target: Real-time)
- Single concept retrieval: < 1ms
- Hypergraph query: < 100μs
- Causal inference: < 500μs

### Batch Operations (Target: High throughput)
- Embedding batch (100): < 5ms
- CRDT merges (100): < 10ms
- Pattern matching: < 1ms

### Distributed Operations (Target: Low latency)
- Consensus round (5 nodes): < 100ms
- State synchronization: < 50ms
- Gossip propagation: < 20ms/hop

## Analyzing Results

1. **HTML Reports**: Open `target/criterion/report/index.html`
2. **Statistical Analysis**: Check for confidence intervals
3. **Regression Detection**: Compare against baselines
4. **Scaling Analysis**: Review performance across different input sizes

## Optimization Guidelines

### When to Optimize
- Operations exceeding 2x baseline targets
- Significant performance regressions
- Poor scaling characteristics
- High variance in measurements

### Optimization Priorities
1. **Critical Path**: Manifold retrieval, hypergraph queries
2. **Throughput**: Batch operations, CRDT merges
3. **Latency**: Consensus, synchronization
4. **Scalability**: Large-scale operations

## Continuous Benchmarking

Run benchmarks:
- Before major commits
- After performance optimizations
- During release candidates
- Weekly baseline updates

## Hardware Considerations

Benchmarks are hardware-dependent. For consistent results:
- Use dedicated benchmark machines
- Disable CPU frequency scaling
- Close unnecessary applications
- Run multiple iterations
- Use `--baseline` for comparisons

## Contributing

When adding new benchmarks:
1. Follow existing naming conventions
2. Include multiple input sizes
3. Document expected baselines
4. Add to this README
5. Verify statistical significance

---

**Last Updated**: 2025-11-29
**Benchmark Suite Version**: 0.1.0
**Criterion Version**: 0.5
