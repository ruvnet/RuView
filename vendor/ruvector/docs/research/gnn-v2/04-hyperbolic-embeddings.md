# Hyperbolic Embeddings for Hierarchical Vector Representations

## Overview

### Problem Statement

Traditional Euclidean embeddings struggle to represent hierarchical structures efficiently. Tree-like and scale-free graphs (common in knowledge graphs, social networks, and taxonomies) require exponentially growing dimensions in Euclidean space to preserve hierarchical distances. This leads to:

- **High dimensionality requirements**: 100+ dimensions for modest hierarchies
- **Poor distance preservation**: Hierarchical relationships get distorted
- **Inefficient similarity search**: HNSW performance degrades with unnecessary dimensions
- **Loss of structural information**: Parent-child relationships not explicitly encoded

### Proposed Solution

Implement a **Hybrid Euclidean-Hyperbolic Embedding System** that combines:

1. **Poincaré Ball Model** for hyperbolic space (hierarchy representation)
2. **Euclidean Space** for traditional similarity features
3. **Möbius Gyrovector Algebra** for vector operations in hyperbolic space
4. **Adaptive Blending** to balance hierarchical vs. similarity features

The system maintains dual representations:
- Hyperbolic component: Captures tree-like hierarchies (20-40% of vector)
- Euclidean component: Captures semantic similarity (60-80% of vector)

### Expected Benefits

**Quantified Improvements:**
- **Dimension Reduction**: 30-50% fewer dimensions for hierarchical data
- **Hierarchy Preservation**: 85-95% hierarchy accuracy vs. 60-70% in Euclidean
- **Search Speed**: 1.5-2x faster due to reduced dimensionality
- **Memory Savings**: 25-40% reduction in total storage
- **Distortion**: 2-3x lower distortion for tree-like structures

**Use Cases:**
- Knowledge graph embeddings (WordNet, Wikidata)
- Organizational hierarchies
- Taxonomy classification
- Document topic hierarchies

## Technical Design

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    HybridEmbedding<T>                        │
├─────────────────────────────────────────────────────────────┤
│  - euclidean_component: Vec<T>     [60-80% of dimensions]   │
│  - hyperbolic_component: Vec<T>    [20-40% of dimensions]   │
│  - blend_ratio: f32                                          │
│  - curvature: f32                  [typically -1.0]          │
└─────────────────────────────────────────────────────────────┘
                          ▲
                          │
          ┌───────────────┴───────────────┐
          │                               │
┌─────────▼──────────┐         ┌─────────▼──────────┐
│ PoincareOps<T>     │         │ EuclideanOps<T>    │
├────────────────────┤         ├────────────────────┤
│ - mobius_add()     │         │ - dot_product()    │
│ - exp_map()        │         │ - cosine_sim()     │
│ - log_map()        │         │ - l2_norm()        │
│ - distance()       │         │ - normalize()      │
│ - gyration()       │         └────────────────────┘
└────────────────────┘
          │
          ▼
┌─────────────────────┐
│ HyperbolicHNSW<T>   │
├─────────────────────┤
│ - hybrid_distance() │ ← Combines both distances
│ - insert()          │
│ - search()          │
└─────────────────────┘
```

### Core Data Structures

```rust
/// Hybrid embedding combining Euclidean and Hyperbolic spaces
#[derive(Clone, Debug)]
pub struct HybridEmbedding<T: Float> {
    /// Euclidean component (semantic similarity)
    pub euclidean: Vec<T>,

    /// Hyperbolic component (hierarchy in Poincaré ball)
    /// Each coordinate constrained to ||x|| < 1
    pub hyperbolic: Vec<T>,

    /// Blend ratio (0.0 = pure Euclidean, 1.0 = pure hyperbolic)
    pub blend_ratio: f32,

    /// Hyperbolic space curvature (typically -1.0)
    pub curvature: f32,

    /// Total dimension
    pub dimension: usize,
}

/// Poincaré ball operations (Möbius gyrovector algebra)
pub struct PoincareOps<T: Float> {
    curvature: T,
    epsilon: T, // Numerical stability (1e-8)
}

impl<T: Float> PoincareOps<T> {
    /// Möbius addition: x ⊕ y
    /// (x⊕y) = ((1+2⟨x,y⟩+||y||²)x + (1-||x||²)y) / (1+2⟨x,y⟩+||x||²||y||²)
    pub fn mobius_add(&self, x: &[T], y: &[T]) -> Vec<T>;

    /// Exponential map: TₓM → M (tangent to manifold)
    pub fn exp_map(&self, x: &[T], v: &[T]) -> Vec<T>;

    /// Logarithmic map: M → TₓM (manifold to tangent)
    pub fn log_map(&self, x: &[T], y: &[T]) -> Vec<T>;

    /// Poincaré distance
    /// d(x,y) = acosh(1 + 2||x-y||²/((1-||x||²)(1-||y||²)))
    pub fn distance(&self, x: &[T], y: &[T]) -> T;

    /// Project vector to Poincaré ball (ensure ||x|| < 1)
    pub fn project(&self, x: &[T]) -> Vec<T>;
}

/// Hybrid HNSW index supporting both distance metrics
pub struct HybridHNSW<T: Float> {
    /// Standard HNSW graph structure
    layers: Vec<HNSWLayer>,

    /// Hybrid embeddings
    embeddings: Vec<HybridEmbedding<T>>,

    /// Distance computation strategy
    distance_fn: HybridDistanceFunction,

    /// HNSW parameters
    params: HNSWParams,
}

/// Distance function combining Euclidean and hyperbolic metrics
pub enum HybridDistanceFunction {
    /// Weighted combination
    Weighted { euclidean_weight: f32, hyperbolic_weight: f32 },

    /// Adaptive based on query context
    Adaptive,

    /// Hierarchical first, then Euclidean for tie-breaking
    Hierarchical,
}

/// Configuration for hybrid embeddings
#[derive(Clone)]
pub struct HybridConfig {
    /// Total embedding dimension
    pub total_dim: usize,

    /// Fraction allocated to hyperbolic space (0.2-0.4)
    pub hyperbolic_ratio: f32,

    /// Hyperbolic space curvature
    pub curvature: f32,

    /// Distance blending strategy
    pub distance_strategy: HybridDistanceFunction,

    /// Numerical stability epsilon
    pub epsilon: f32,
}
```

### Key Algorithms

#### Algorithm 1: Hybrid Distance Computation

```pseudocode
function hybrid_distance(emb1: HybridEmbedding, emb2: HybridEmbedding) -> float:
    // Compute Euclidean component distance
    d_euclidean = cosine_distance(emb1.euclidean, emb2.euclidean)

    // Compute hyperbolic component distance (Poincaré)
    d_hyperbolic = poincare_distance(emb1.hyperbolic, emb2.hyperbolic)

    // Normalize distances to [0, 1] range
    d_euclidean_norm = d_euclidean / 2.0  // cosine ∈ [0, 2]
    d_hyperbolic_norm = tanh(d_hyperbolic / 2.0)  // hyperbolic ∈ [0, ∞)

    // Blend based on strategy
    match emb1.blend_strategy:
        Weighted(w_e, w_h):
            return w_e * d_euclidean_norm + w_h * d_hyperbolic_norm

        Adaptive:
            // Use hyperbolic more for hierarchical queries
            hierarchy_score = detect_hierarchy(emb1, emb2)
            w_h = hierarchy_score
            w_e = 1.0 - hierarchy_score
            return w_e * d_euclidean_norm + w_h * d_hyperbolic_norm

        Hierarchical:
            // Use hyperbolic for pruning, Euclidean for ranking
            if d_hyperbolic_norm > threshold:
                return d_hyperbolic_norm
            else:
                return 0.3 * d_hyperbolic_norm + 0.7 * d_euclidean_norm
```

#### Algorithm 2: Poincaré Distance (Optimized)

```pseudocode
function poincare_distance(x: Vec<T>, y: Vec<T>, curvature: T) -> T:
    // Compute ||x - y||²
    diff_norm_sq = 0.0
    for i in 0..x.len():
        diff = x[i] - y[i]
        diff_norm_sq += diff * diff

    // Compute ||x||² and ||y||²
    x_norm_sq = dot(x, x)
    y_norm_sq = dot(y, y)

    // Numerical stability: ensure norms < 1
    x_norm_sq = min(x_norm_sq, 1.0 - epsilon)
    y_norm_sq = min(y_norm_sq, 1.0 - epsilon)

    // Poincaré distance formula
    numerator = 2.0 * diff_norm_sq
    denominator = (1.0 - x_norm_sq) * (1.0 - y_norm_sq)

    ratio = numerator / (denominator + epsilon)

    // d = acosh(1 + ratio)
    // Numerically stable: acosh(x) = log(x + sqrt(x²-1))
    inner = 1.0 + ratio
    if inner < 1.0 + epsilon:
        return 0.0  // Points are identical

    return log(inner + sqrt(inner * inner - 1.0)) / sqrt(abs(curvature))
```

#### Algorithm 3: Möbius Addition (Core Operation)

```pseudocode
function mobius_add(x: Vec<T>, y: Vec<T>, curvature: T) -> Vec<T]:
    // Compute scalar products
    xy_dot = dot(x, y)
    x_norm_sq = dot(x, x)
    y_norm_sq = dot(y, y)

    // Conformal factor
    denominator = 1.0 + 2.0 * curvature * xy_dot +
                  curvature² * x_norm_sq * y_norm_sq

    // Numerator terms
    numerator_x_coeff = 1.0 + 2.0 * curvature * xy_dot +
                        curvature * y_norm_sq
    numerator_y_coeff = 1.0 - curvature * x_norm_sq

    // Result
    result = Vec::new()
    for i in 0..x.len():
        value = (numerator_x_coeff * x[i] + numerator_y_coeff * y[i]) /
                (denominator + epsilon)
        result.push(value)

    // Project back to ball (ensure ||result|| < 1)
    return project_to_ball(result)

function project_to_ball(x: Vec<T>) -> Vec<T]:
    norm = sqrt(dot(x, x))
    if norm >= 1.0:
        // Project to ball with radius 1 - epsilon
        scale = (1.0 - epsilon) / norm
        return x.map(|xi| xi * scale)
    return x
```

### API Design

```rust
// Public API for hybrid embeddings
pub mod hybrid {
    use super::*;

    /// Create hybrid embedding from separate components
    pub fn create_hybrid<T: Float>(
        euclidean: Vec<T>,
        hyperbolic: Vec<T>,
        config: HybridConfig,
    ) -> Result<HybridEmbedding<T>, Error>;

    /// Convert standard embedding to hybrid (automatic split)
    pub fn euclidean_to_hybrid<T: Float>(
        embedding: &[T],
        config: HybridConfig,
    ) -> Result<HybridEmbedding<T>, Error>;

    /// Compute distance between hybrid embeddings
    pub fn distance<T: Float>(
        a: &HybridEmbedding<T>,
        b: &HybridEmbedding<T>,
    ) -> T;

    /// Create HNSW index with hybrid embeddings
    pub fn build_index<T: Float>(
        embeddings: Vec<HybridEmbedding<T>>,
        config: HybridConfig,
        hnsw_params: HNSWParams,
    ) -> Result<HybridHNSW<T>, Error>;
}

// Poincaré ball operations (advanced users)
pub mod poincare {
    /// Möbius addition in Poincaré ball
    pub fn mobius_add<T: Float>(
        x: &[T],
        y: &[T],
        curvature: T,
    ) -> Vec<T>;

    /// Exponential map (tangent to manifold)
    pub fn exp_map<T: Float>(
        base: &[T],
        tangent: &[T],
        curvature: T,
    ) -> Vec<T>;

    /// Logarithmic map (manifold to tangent)
    pub fn log_map<T: Float>(
        base: &[T],
        point: &[T],
        curvature: T,
    ) -> Vec<T>;

    /// Poincaré distance
    pub fn distance<T: Float>(
        x: &[T],
        y: &[T],
        curvature: T,
    ) -> T;
}
```

## Integration Points

### Affected Crates/Modules

1. **ruvector-core** (Major Changes)
   - Add `hybrid_embedding.rs` module
   - Extend `Distance` trait with `HybridDistance` variant
   - Update `Embedding` enum to include `Hybrid` variant

2. **ruvector-hnsw** (Moderate Changes)
   - Modify distance computation in `hnsw/search.rs`
   - Add hybrid-aware layer construction
   - Update serialization for hybrid embeddings

3. **ruvector-gnn-node** (Minor Changes)
   - Add TypeScript bindings for hybrid embeddings
   - Export Poincaré operations to JavaScript

4. **ruvector-quantization** (Future Integration)
   - Separate quantization strategies for Euclidean vs. hyperbolic components
   - Hyperbolic component needs special handling (preserve ball constraint)

### New Modules to Create

```
crates/ruvector-hyperbolic/
├── src/
│   ├── lib.rs                          # Public API
│   ├── poincare/
│   │   ├── mod.rs                      # Poincaré ball model
│   │   ├── ops.rs                      # Möbius operations
│   │   ├── distance.rs                 # Distance computation
│   │   └── projection.rs               # Ball projection
│   ├── hybrid/
│   │   ├── mod.rs                      # Hybrid embeddings
│   │   ├── embedding.rs                # HybridEmbedding struct
│   │   ├── distance.rs                 # Hybrid distance
│   │   └── conversion.rs               # Euclidean ↔ Hybrid
│   ├── hnsw/
│   │   ├── mod.rs                      # Hybrid HNSW
│   │   └── index.rs                    # HybridHNSW implementation
│   └── math/
│       ├── gyrovector.rs               # Gyrovector algebra
│       └── numerics.rs                 # Numerical stability
├── tests/
│   ├── poincare_tests.rs               # Poincaré operations
│   ├── hierarchy_tests.rs              # Hierarchy preservation
│   └── integration_tests.rs            # End-to-end
├── benches/
│   ├── distance_bench.rs               # Distance computation
│   └── hnsw_bench.rs                   # HNSW performance
└── Cargo.toml
```

### Dependencies on Other Features

- **Independent**: Can be implemented standalone
- **Synergies**:
  - **Adaptive Precision** (Feature 5): Hyperbolic components may benefit from higher precision near ball boundary
  - **Temporal GNN** (Feature 6): Time-evolving hierarchies (e.g., organizational changes)
  - **Attention Mechanisms** (Existing): Attention weights could adapt based on hierarchy depth

## Regression Prevention

### What Existing Functionality Could Break

1. **HNSW Search Performance**
   - Risk: Hybrid distance computation is more expensive
   - Impact: 10-20% search latency increase

2. **Serialization Format**
   - Risk: Existing indexes won't deserialize
   - Impact: Breaking change for stored indexes

3. **Memory Layout**
   - Risk: Hybrid embeddings require metadata (blend ratio, curvature)
   - Impact: 5-10% memory overhead

4. **Distance Metric Assumptions**
   - Risk: Some code assumes Euclidean properties (triangle inequality)
   - Impact: Graph construction may be affected

### Test Cases to Prevent Regressions

```rust
#[cfg(test)]
mod regression_tests {
    use super::*;

    #[test]
    fn test_pure_euclidean_mode_matches_original() {
        // Hybrid with blend_ratio=0.0 should match Euclidean exactly
        let config = HybridConfig {
            hyperbolic_ratio: 0.0,  // No hyperbolic component
            ..Default::default()
        };

        let euclidean_dist = cosine_distance(&emb1, &emb2);
        let hybrid_dist = hybrid_distance(&hybrid_emb1, &hybrid_emb2);

        assert!((euclidean_dist - hybrid_dist).abs() < 1e-6);
    }

    #[test]
    fn test_hnsw_recall_not_degraded() {
        // HNSW recall should remain >= 95% with hybrid embeddings
        let recall = benchmark_hnsw_recall(&hybrid_index, &queries);
        assert!(recall >= 0.95);
    }

    #[test]
    fn test_backward_compatibility_serialization() {
        // Old indexes should still deserialize
        let legacy_index = deserialize_legacy_index("test.hnsw");
        assert!(legacy_index.is_ok());
    }

    #[test]
    fn test_numerical_stability_edge_cases() {
        // Test with points near ball boundary (||x|| ≈ 1)
        let near_boundary = vec![0.999, 0.0, 0.0];
        let result = mobius_add(&near_boundary, &near_boundary);

        // Should not produce NaN or overflow
        assert!(result.iter().all(|x| x.is_finite()));
        assert!(l2_norm(&result) < 1.0);  // Still in ball
    }
}
```

### Backward Compatibility Strategy

1. **Versioned Serialization**
   ```rust
   enum EmbeddingFormat {
       V1Euclidean,     // Legacy format
       V2Hybrid,        // New format
   }
   ```

2. **Feature Flag**
   ```toml
   [features]
   default = ["euclidean"]
   hyperbolic = ["dep:special-functions"]
   ```

3. **Migration Path**
   ```rust
   // Automatic conversion utility
   pub fn migrate_index_to_hybrid(
       old_index: &Path,
       config: HybridConfig,
   ) -> Result<HybridHNSW, Error> {
       // Read old Euclidean index
       // Convert embeddings to hybrid
       // Rebuild graph structure
   }
   ```

## Implementation Phases

### Phase 1: Core Implementation (Weeks 1-2)

**Goal**: Implement Poincaré ball operations and hybrid embeddings

**Tasks**:
1. Create `ruvector-hyperbolic` crate
2. Implement `PoincareOps`:
   - Möbius addition
   - Exponential/logarithmic maps
   - Distance computation
   - Projection to ball
3. Implement `HybridEmbedding` struct
4. Write comprehensive unit tests
5. Add numerical stability tests

**Deliverables**:
- Working Poincaré operations (100% test coverage)
- Hybrid embedding data structure
- Benchmark suite for distance computation

**Success Criteria**:
- All Poincaré operations pass property tests (associativity, etc.)
- Numerical stability for edge cases (||x|| → 1)
- Distance computation < 2µs per pair (f32)

### Phase 2: Integration (Weeks 3-4)

**Goal**: Integrate hybrid embeddings with HNSW

**Tasks**:
1. Extend `Distance` trait with `HybridDistance`
2. Implement `HybridHNSW` index
3. Add serialization/deserialization
4. Create migration utilities for legacy indexes
5. Add TypeScript/JavaScript bindings

**Deliverables**:
- Functioning `HybridHNSW` index
- Backward-compatible serialization
- Node.js bindings with examples

**Success Criteria**:
- HNSW search works with hybrid embeddings
- Recall >= 95% (compared to brute force)
- Legacy indexes still load correctly

### Phase 3: Optimization (Weeks 5-6)

**Goal**: Optimize performance and memory usage

**Tasks**:
1. SIMD optimization for Poincaré distance
2. Cache-friendly memory layout
3. Parallel distance computation
4. Benchmark against pure Euclidean baseline
5. Profile and optimize hotspots

**Deliverables**:
- SIMD-accelerated distance computation
- Performance benchmarks
- Memory profiling report

**Success Criteria**:
- Distance computation within 1.5x of Euclidean baseline
- Memory overhead < 10%
- Parallel search scales linearly to 8 threads

### Phase 4: Production Hardening (Weeks 7-8)

**Goal**: Production-ready with documentation and examples

**Tasks**:
1. Write comprehensive documentation
2. Create example applications:
   - Knowledge graph embeddings
   - Hierarchical taxonomy search
3. Add monitoring/observability
4. Performance tuning for specific use cases
5. Create migration guide

**Deliverables**:
- API documentation
- 3+ example applications
- Migration guide from Euclidean
- Production deployment checklist

**Success Criteria**:
- Documentation completeness score > 90%
- Examples run successfully
- Zero P0/P1 bugs in testing

## Success Metrics

### Performance Benchmarks

**Latency Targets**:
- Poincaré distance computation: < 2.0µs (f32), < 1.0µs (SIMD)
- Hybrid distance computation: < 2.5µs (f32)
- HNSW search (100k vectors): < 500µs (p95)
- Index construction: < 10 minutes (1M vectors)

**Comparison Baseline** (Pure Euclidean):
- Distance computation slowdown: < 1.5x
- Search latency slowdown: < 1.3x
- Index size increase: < 10%

**Throughput Targets**:
- Distance computation: > 400k pairs/sec (single thread)
- HNSW search: > 2000 QPS (8 threads)

### Accuracy Metrics

**Hierarchy Preservation**:
- Tree reconstruction accuracy: > 90%
- Parent-child relationship recall: > 85%
- Hierarchy depth correlation: > 0.90

**HNSW Recall**:
- Top-10 recall @ ef=50: >= 95%
- Top-100 recall @ ef=200: >= 98%

**Distance Distortion**:
- Average distortion (vs. ground truth): < 0.15
- Max distortion (99th percentile): < 0.30

### Memory/Latency Targets

**Memory Reduction** (vs. pure Euclidean with same hierarchy quality):
- Total embedding size: 30-50% reduction
- HNSW index size: 25-40% reduction
- Runtime memory: < 5% overhead for metadata

**Latency Breakdown**:
- Euclidean component: 40-50% of time
- Hyperbolic component: 40-50% of time
- Blending/normalization: < 10% of time

**Scalability**:
- Linear scaling to 10M vectors
- Sub-linear scaling to 100M vectors (with sharding)

## Risks and Mitigations

### Technical Risks

**Risk 1: Numerical Instability near Ball Boundary**
- **Severity**: High
- **Impact**: NaN/Inf values, incorrect distances
- **Probability**: Medium
- **Mitigation**:
  - Use epsilon-buffered projection (||x|| < 1 - ε)
  - Employ numerically stable formulas (log-sum-exp tricks)
  - Add extensive edge case tests
  - Use higher precision (f64) for critical operations

**Risk 2: Performance Degradation**
- **Severity**: Medium
- **Impact**: Slower search, higher latency
- **Probability**: High
- **Mitigation**:
  - SIMD optimization for distance computation
  - Precompute and cache norm squares
  - Profile-guided optimization
  - Provide performance tuning guide

**Risk 3: Complex API Confusion**
- **Severity**: Medium
- **Impact**: User adoption issues, misconfiguration
- **Probability**: Medium
- **Mitigation**:
  - Provide sensible defaults (blend_ratio=0.3, curvature=-1.0)
  - Create configuration presets (taxonomy, knowledge-graph, etc.)
  - Write comprehensive examples
  - Add validation with helpful error messages

**Risk 4: Serialization Compatibility**
- **Severity**: High
- **Impact**: Breaking changes, migration pain
- **Probability**: High
- **Mitigation**:
  - Version serialization format
  - Provide automatic migration tool
  - Support reading legacy formats
  - Comprehensive migration guide

**Risk 5: Integration with Quantization**
- **Severity**: Medium
- **Impact**: Quantization may break ball constraints
- **Probability**: High
- **Mitigation**:
  - Defer quantization for hyperbolic component
  - Research hyperbolic-aware quantization schemes
  - Document incompatibilities clearly
  - Provide fallback to f32 for hyperbolic

**Risk 6: Limited Use Case Applicability**
- **Severity**: Low
- **Impact**: Feature underutilized if data isn't hierarchical
- **Probability**: Medium
- **Mitigation**:
  - Provide hierarchy detection tool
  - Make hyperbolic component optional (blend_ratio=0)
  - Document ideal use cases clearly
  - Add auto-configuration based on data analysis

### Mitigation Summary Table

| Risk | Mitigation Strategy | Owner | Timeline |
|------|-------------------|-------|----------|
| Numerical instability | Epsilon buffering + stable formulas | Core team | Phase 1 |
| Performance degradation | SIMD + profiling + caching | Optimization team | Phase 3 |
| API complexity | Defaults + examples + validation | API team | Phase 4 |
| Serialization breaks | Versioning + migration tool | Integration team | Phase 2 |
| Quantization conflict | Defer integration + research | Research team | Post-v1 |
| Limited applicability | Detection tool + documentation | Product team | Phase 4 |

---

## References

1. **Nickel & Kiela (2017)**: "Poincaré Embeddings for Learning Hierarchical Representations"
2. **Sala et al. (2018)**: "Representation Tradeoffs for Hyperbolic Embeddings"
3. **Chami et al. (2019)**: "Hyperbolic Graph Convolutional Neural Networks"
4. **Ganea et al. (2018)**: "Hyperbolic Neural Networks"

## Appendix: Mathematical Foundations

### Poincaré Ball Model

The Poincaré ball model represents hyperbolic space as:
```
B^n = {x ∈ ℝ^n : ||x|| < 1}
```

with metric tensor:
```
g_x = (2 / (1 - ||x||²))² δ_ij
```

### Möbius Addition Formula

```
x ⊕_c y = ((1 + 2c⟨x,y⟩ + c||y||²)x + (1 - c||x||²)y) / (1 + 2c⟨x,y⟩ + c²||x||²||y||²)
```

where c is the absolute curvature (typically c = 1, curvature = -1).

### Distance Formula

```
d_c(x, y) = (1/√c) acosh(1 + 2c ||x - y||² / ((1 - c||x||²)(1 - c||y||²)))
```

### Exponential Map (Tangent to Manifold)

```
exp_x^c(v) = x ⊕_c (tanh(√c ||v|| / 2) / (√c ||v||)) v
```

### Logarithmic Map (Manifold to Tangent)

```
log_x^c(y) = (2 / (√c λ_x)) atanh(√c ||(-x) ⊕_c y||) · ((-x) ⊕_c y) / ||(-x) ⊕_c y||
```

where `λ_x = 1 / (1 - c||x||²)` is the conformal factor.
