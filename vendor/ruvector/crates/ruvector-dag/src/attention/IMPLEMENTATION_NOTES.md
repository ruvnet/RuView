# Advanced Attention Mechanisms - Implementation Notes

## Agent #3 Implementation Summary

This implementation provides 5 advanced attention mechanisms for DAG query optimization:

### 1. Hierarchical Lorentz Attention (`hierarchical_lorentz.rs`)
- **Lines**: 274
- **Complexity**: O(n²·d)
- Uses hyperbolic geometry (Lorentz model) to embed DAG nodes
- Deeper nodes in hierarchy receive higher attention
- Implements Lorentz inner product and distance metrics

### 2. Parallel Branch Attention (`parallel_branch.rs`)
- **Lines**: 291
- **Complexity**: O(n² + b·n)
- Detects and coordinates parallel execution branches
- Balances workload across branches
- Applies synchronization penalties

### 3. Temporal BTSP Attention (`temporal_btsp.rs`)
- **Lines**: 291
- **Complexity**: O(n + t)
- Behavioral Timescale Synaptic Plasticity
- Uses eligibility traces for temporal learning
- Implements plateau state boosting

### 4. Attention Selector (`selector.rs`)
- **Lines**: 281
- **Complexity**: O(1) for selection
- UCB1 bandit algorithm for mechanism selection
- Tracks rewards and counts for each mechanism
- Balances exploration vs exploitation

### 5. Attention Cache (`cache.rs`)
- **Lines**: 316
- **Complexity**: O(1) for get/insert
- LRU eviction policy
- TTL support for entries
- Hash-based DAG fingerprinting

## Trait Definition

**`trait_def.rs`** (75 lines):
- Defines `DagAttentionMechanism` trait
- `AttentionScores` structure with edge weights
- `AttentionError` for error handling

## Integration

All mechanisms are exported from `mod.rs` with appropriate type aliases to avoid conflicts with existing attention system.

## Tests

Each mechanism includes comprehensive unit tests:
- Hierarchical Lorentz: 3 tests
- Parallel Branch: 2 tests
- Temporal BTSP: 3 tests
- Selector: 4 tests
- Cache: 7 tests

Total: 19 new test functions

## Performance Targets

| Mechanism | Target | Notes |
|-----------|--------|-------|
| HierarchicalLorentz | <150μs | For 100 nodes |
| ParallelBranch | <100μs | For 100 nodes |
| TemporalBTSP | <120μs | For 100 nodes |
| Selector.select() | <1μs | UCB1 computation |
| Cache.get() | <1μs | LRU lookup |

## Compatibility Notes

The implementation is designed to work with the updated QueryDag API that uses:
- HashMap-based node storage
- `estimated_cost` and `estimated_rows` instead of `cost` and `selectivity`
- `children(id)` and `parents(id)` methods
- No direct edges iterator

Some test fixtures may need updates to match the current OperatorNode structure.

## Future Work

- Add SIMD optimizations for hyperbolic distance calculations
- Implement GPU acceleration for large DAGs
- Add more sophisticated caching strategies
- Integrate with existing TopologicalAttention, CausalConeAttention, etc.
