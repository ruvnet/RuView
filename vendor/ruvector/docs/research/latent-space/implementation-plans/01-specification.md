# SPARC Specification: ruvector-attention Crate

**Version**: 1.0.0
**Date**: 2025-11-30
**Status**: Draft
**Authors**: RuVector Research Team
**SPARC Phase**: Specification

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Requirements Analysis](#2-requirements-analysis)
3. [Module Architecture](#3-module-architecture)
4. [API Design](#4-api-design)
5. [Performance Targets](#5-performance-targets)
6. [Compatibility Matrix](#6-compatibility-matrix)
7. [Testing Strategy](#7-testing-strategy)
8. [Success Criteria](#8-success-criteria)
9. [Constraints and Dependencies](#9-constraints-and-dependencies)
10. [Risk Assessment](#10-risk-assessment)

---

## 1. Executive Summary

### 1.1 Vision

Create a modular, high-performance attention mechanism library specifically designed for GNN latent space operations in RuVector. The `ruvector-attention` crate will implement **10 distinct attention mechanisms** from research literature, enabling researchers and practitioners to experiment with different attention strategies for graph-structured data.

**Core Mission**: Bridge the gap between latent space representations and graph topology through specialized attention mechanisms optimized for HNSW-based vector databases.

### 1.2 Goals

**Primary Goals**:
1. **Modularity**: Each attention mechanism is a standalone, composable component
2. **Performance**: Achieve <200ms latency for 95% of attention operations on 1000-neighbor graphs
3. **Compatibility**: Support WASM, NAPI-RS (Node.js), CLI, and Rust SDK environments
4. **Extensibility**: Easy to add new attention mechanisms without modifying core APIs
5. **Research-Driven**: Implement cutting-edge attention mechanisms from academic literature

**Secondary Goals**:
1. Provide benchmarking tools for comparing attention mechanisms
2. Enable automatic mechanism selection based on graph properties
3. Support distributed/parallel attention computation
4. Maintain numerical stability across all implementations

### 1.3 Performance Targets

| Metric | Target | Stretch Goal |
|--------|--------|--------------|
| **Latency (p95)** | <200ms @ 1K neighbors | <100ms @ 1K neighbors |
| **Throughput** | 5,000 ops/sec | 10,000 ops/sec |
| **Memory (per op)** | <50MB @ 1K neighbors | <25MB @ 1K neighbors |
| **WASM Binary Size** | <2MB (gzipped) | <1MB (gzipped) |
| **Compilation Time** | <60s (release) | <30s (release) |
| **Test Coverage** | >90% | >95% |

### 1.4 Timeline Overview

**Phase 1 (Weeks 1-4)**: Core attention primitives + Multi-head attention
**Phase 2 (Weeks 5-8)**: Geometric attention (Hyperbolic, Edge-featured)
**Phase 3 (Weeks 9-12)**: Sparse and efficient mechanisms (Flash, Linear)
**Phase 4 (Weeks 13-16)**: Adaptive mechanisms (MoE, Cross-attention)
**Phase 5 (Weeks 17-20)**: Integration, optimization, documentation

---

## 2. Requirements Analysis

### 2.1 Functional Requirements

#### FR-001: Core Attention Mechanisms

**Priority**: CRITICAL
**Description**: Implement foundational attention mechanisms

**Acceptance Criteria**:
- [x] FR-001.1: Scaled Dot-Product Attention (baseline)
- [ ] FR-001.2: Multi-Head Attention (2-16 heads configurable)
- [ ] FR-001.3: Supports variable-length input sequences
- [ ] FR-001.4: Numerically stable softmax implementation
- [ ] FR-001.5: Gradient computation for backpropagation

**Test Cases**:
```rust
#[test]
fn test_scaled_dot_product_attention() {
    let attn = ScaledDotProductAttention::new(128);
    let query = vec![1.0; 128];
    let keys = vec![vec![1.0; 128]; 10];
    let values = vec![vec![1.0; 128]; 10];

    let output = attn.forward(&query, &keys, &values);
    assert_eq!(output.len(), 128);
    assert!(output.iter().all(|&x| x.is_finite()));
}
```

---

#### FR-002: Geometric Attention Mechanisms

**Priority**: HIGH
**Description**: Implement attention mechanisms aware of geometric structure

**Acceptance Criteria**:
- [ ] FR-002.1: Edge-Featured Attention (GAT-style with edge attributes)
- [ ] FR-002.2: Hyperbolic Attention (Poincaré ball model)
- [ ] FR-002.3: Mixed-Curvature Attention (Euclidean + Hyperbolic fusion)
- [ ] FR-002.4: Manifold-Aware Attention

**Edge-Featured Attention**:
```
score(i, j) = LeakyReLU(a^T [W·h_i || W·h_j || W_e·edge_ij])
```

**Hyperbolic Attention**:
```
distance_poincare(x, y) = arccosh(1 + 2||x-y||² / ((1-||x||²)(1-||y||²)))
score(i, j) = -distance_poincare(q_i, k_j)
```

**Test Cases**:
```rust
#[test]
fn test_edge_featured_attention() {
    let attn = EdgeFeaturedAttention::new(128, 32);
    let edge_features = vec![vec![1.0; 32]; 10];

    let output = attn.forward_with_edges(
        &query, &keys, &values, Some(&edge_features)
    );
    assert!(output.len() == 128);
}

#[test]
fn test_hyperbolic_attention_bounds() {
    let attn = HyperbolicAttention::new(128, -1.0);
    let query = vec![0.5; 128]; // Inside Poincaré ball

    // Ensure all embeddings stay in ball (||x|| < 1)
    let output = attn.forward(&query, &keys, &values);
    assert!(l2_norm(&output) < 0.99);
}
```

---

#### FR-003: Sparse Attention Patterns

**Priority**: HIGH
**Description**: Reduce O(n²) complexity through sparsity

**Acceptance Criteria**:
- [ ] FR-003.1: Local + Global Attention (Longformer-style)
- [ ] FR-003.2: Linear Attention (Performer/FAVOR+)
- [ ] FR-003.3: Flash Attention (memory-efficient tiling)
- [ ] FR-003.4: Configurable sparsity patterns

**Local + Global Pattern**:
```
Attention Matrix:
  [L L L G 0 0 0 0]  L = Local (1-hop neighbors)
  [L L L L G 0 0 0]  G = Global (HNSW higher layers)
  [L L L L L G 0 0]  0 = No attention
  ...
```

**Complexity Requirements**:
- Local + Global: O(k_local + k_global) where k << n
- Linear: O(n·d) where d = feature dimension
- Flash: O(n) memory (vs O(n²) standard)

**Test Cases**:
```rust
#[test]
fn test_sparse_attention_complexity() {
    let sparse_attn = SparseGraphAttention::new(
        local_window: 10,
        global_nodes: 5
    );

    // Should only attend to 15 nodes, not all 1000
    let num_neighbors = 1000;
    let actual_attention = sparse_attn.get_attention_mask(num_neighbors);
    assert!(actual_attention.count_nonzero() <= 15);
}
```

---

#### FR-004: Graph-Aware Mechanisms

**Priority**: HIGH
**Description**: Attention specialized for graph structure

**Acceptance Criteria**:
- [ ] FR-004.1: RoPE (Rotary Position Embeddings) for graph distance
- [ ] FR-004.2: HNSW-layer encoding in attention
- [ ] FR-004.3: Cross-Attention (Dual-Space: graph + latent)
- [ ] FR-004.4: Structural feature integration (degree, centrality)

**RoPE for Graphs**:
```rust
// Encode graph distance via rotation
rotation_angle = graph_distance / base^(2i/d)
rotated[i] = emb[i] * cos(θ) - emb[i+1] * sin(θ)
```

**Cross-Attention**:
```
graph_attn = Attention(h, N_graph(h), N_graph(h))
latent_attn = Attention(h, N_latent(h), N_latent(h))
cross_attn = Attention(graph_attn, N_latent(h), N_latent(h))
output = Fusion(graph_attn, latent_attn, cross_attn)
```

---

#### FR-005: Adaptive Mechanisms

**Priority**: MEDIUM
**Description**: Attention that adapts to input patterns

**Acceptance Criteria**:
- [ ] FR-005.1: Mixture of Experts (MoE) Attention
- [ ] FR-005.2: Learned routing between attention types
- [ ] FR-005.3: RL-based navigation function learning
- [ ] FR-005.4: Dynamic head count adjustment

**MoE Attention**:
```rust
router_scores = Router(query)
expert_indices = topk(router_scores, k=2)
output = Σ router_scores[i] * Expert[i](query, keys, values)
```

**Experts**:
1. Local Expert: Standard attention for 1-hop neighbors
2. Hierarchical Expert: Hyperbolic attention for HNSW layers
3. Global Expert: Linear attention for distant nodes
4. Structural Expert: Edge-featured attention

---

#### FR-006: Training and Optimization Utilities

**Priority**: HIGH
**Description**: Tools for training attention-based models

**Acceptance Criteria**:
- [ ] FR-006.1: Contrastive losses (InfoNCE, Local Contrastive)
- [ ] FR-006.2: Spectral regularization (Laplacian smoothness)
- [ ] FR-006.3: Multi-objective loss balancing
- [ ] FR-006.4: Curriculum learning schedules
- [ ] FR-006.5: Hard negative mining

---

#### FR-007: Tensor Compression

**Priority**: MEDIUM
**Description**: Memory-efficient tensor operations

**Acceptance Criteria**:
- [ ] FR-007.1: Quantization (INT8, INT4)
- [ ] FR-007.2: Low-rank factorization
- [ ] FR-007.3: Sparse tensor storage
- [ ] FR-007.4: Hierarchical compression for HNSW layers

---

#### FR-008: SIMD Optimizations

**Priority**: MEDIUM
**Description**: Vectorized operations for performance

**Acceptance Criteria**:
- [ ] FR-008.1: AVX2/AVX-512 support for x86_64
- [ ] FR-008.2: NEON support for ARM
- [ ] FR-008.3: WASM SIMD support
- [ ] FR-008.4: Automatic fallback to scalar operations

---

### 2.2 Non-Functional Requirements

#### NFR-001: Performance

**NFR-001.1**: Latency
- **Requirement**: p95 latency <200ms for 1000-neighbor attention
- **Measurement**: Benchmark suite with synthetic graphs
- **Verification**: CI/CD performance regression tests

**NFR-001.2**: Throughput
- **Requirement**: 5,000 attention operations per second
- **Measurement**: Batch processing benchmarks
- **Verification**: Load testing with real HNSW graphs

**NFR-001.3**: Memory
- **Requirement**: Peak memory <50MB per operation
- **Measurement**: Memory profiling with valgrind/heaptrack
- **Verification**: Memory regression tests in CI

**NFR-001.4**: Scalability
- **Requirement**: Linear scaling up to 10K neighbors
- **Measurement**: Complexity analysis and empirical benchmarks
- **Verification**: Big-O complexity proofs + empirical validation

---

#### NFR-002: Reliability

**NFR-002.1**: Numerical Stability
- **Requirement**: All outputs finite (no NaN, Inf) across 10M operations
- **Measurement**: Fuzzing with random inputs
- **Verification**: Property-based testing with proptest

**NFR-002.2**: Error Handling
- **Requirement**: All errors recoverable, 100% error path coverage
- **Measurement**: Error injection testing
- **Verification**: Unit tests for error cases

**NFR-002.3**: Determinism
- **Requirement**: Same inputs produce same outputs (no random behavior)
- **Measurement**: Repeated execution tests
- **Verification**: Determinism tests in CI

---

#### NFR-003: Maintainability

**NFR-003.1**: Code Quality
- **Requirement**: Clippy clean (zero warnings), rustfmt formatted
- **Measurement**: CI linting checks
- **Verification**: Pre-commit hooks + CI gates

**NFR-003.2**: Documentation
- **Requirement**: 100% public API documented with examples
- **Measurement**: rustdoc coverage tool
- **Verification**: Doc tests pass, examples compile

**NFR-003.3**: Test Coverage
- **Requirement**: >90% line coverage, >95% branch coverage
- **Measurement**: cargo-tarpaulin
- **Verification**: CI coverage reports

---

#### NFR-004: Portability

**NFR-004.1**: Platform Support
- **Requirement**: Linux, macOS, Windows support
- **Measurement**: CI testing on all platforms
- **Verification**: Cross-platform integration tests

**NFR-004.2**: WASM Compatibility
- **Requirement**: Full functionality in WASM (wasm32-unknown-unknown)
- **Measurement**: WASM-specific test suite
- **Verification**: Browser and Node.js WASM tests

**NFR-004.3**: NAPI-RS Support
- **Requirement**: All attention mechanisms callable from Node.js
- **Measurement**: Node.js integration tests
- **Verification**: NPM package smoke tests

---

#### NFR-005: Security

**NFR-005.1**: Memory Safety
- **Requirement**: Zero unsafe code blocks (or 100% audited unsafe)
- **Measurement**: Manual code review
- **Verification**: MIRI checks, cargo-geiger

**NFR-005.2**: Dependency Audit
- **Requirement**: All dependencies audited, no known CVEs
- **Measurement**: cargo-audit
- **Verification**: Automated dependency scanning in CI

---

### 2.3 Constraints

#### C-001: Compatibility Constraints
- **Rust Version**: MSRV 1.77+ (per workspace configuration)
- **No GPU**: All implementations must run on CPU (WASM/NAPI-RS requirement)
- **No Standard Library in WASM**: Must support `#![no_std]` for WASM32

#### C-002: API Constraints
- **Backwards Compatibility**: Once 1.0 released, follow SemVer strictly
- **Trait Consistency**: All attention mechanisms implement common `Attention` trait
- **Builder Pattern**: Configuration via builders, not constructors

#### C-003: Performance Constraints
- **Compilation Time**: Release build <60s on CI runners
- **Binary Size**: WASM bundle <2MB gzipped
- **Memory Footprint**: No global allocators, stack-preferred where possible

#### C-004: Licensing Constraints
- **License**: MIT (per workspace)
- **Dependency Licenses**: MIT/Apache-2.0 only (no GPL/LGPL)

---

## 3. Module Architecture

### 3.1 Crate Structure

```
ruvector-attention/
├── Cargo.toml
├── README.md
├── LICENSE
│
├── src/
│   ├── lib.rs                     # Public API, re-exports
│   │
│   ├── core/                      # Core attention primitives
│   │   ├── mod.rs                 # Core module exports
│   │   ├── base.rs                # Attention trait definition
│   │   ├── scaled_dot.rs          # Scaled dot-product attention
│   │   ├── multi_head.rs          # Multi-head attention
│   │   └── config.rs              # Configuration structs
│   │
│   ├── geometric/                 # Geometric attention
│   │   ├── mod.rs
│   │   ├── hyperbolic.rs          # Poincaré ball attention
│   │   ├── edge_featured.rs       # GAT-style edge attention
│   │   ├── mixed_curvature.rs     # Euclidean + Hyperbolic
│   │   └── manifold.rs            # General manifold attention
│   │
│   ├── sparse/                    # Sparse patterns
│   │   ├── mod.rs
│   │   ├── local_global.rs        # Longformer-style
│   │   ├── linear.rs              # Performer/FAVOR+
│   │   ├── flash.rs               # Flash Attention (tiled)
│   │   └── patterns.rs            # Sparsity pattern utilities
│   │
│   ├── graph/                     # Graph-aware attention
│   │   ├── mod.rs
│   │   ├── rope_graph.rs          # RoPE for graph distances
│   │   ├── cross_space.rs         # Dual-space cross-attention
│   │   ├── hnsw_aware.rs          # HNSW layer encoding
│   │   └── structural.rs          # Degree/centrality features
│   │
│   ├── adaptive/                  # Adaptive/learned mechanisms
│   │   ├── mod.rs
│   │   ├── moe.rs                 # Mixture of Experts
│   │   ├── learned_routing.rs     # Attention routing
│   │   ├── rl_navigator.rs        # RL-based graph navigation
│   │   └── dynamic_heads.rs       # Adaptive head count
│   │
│   ├── training/                  # Training utilities
│   │   ├── mod.rs
│   │   ├── losses.rs              # Contrastive, reconstruction
│   │   ├── optimizers.rs          # SGD, Adam, etc.
│   │   ├── regularizers.rs        # Spectral, L2, etc.
│   │   ├── curriculum.rs          # Curriculum learning
│   │   └── hard_negatives.rs      # Negative sampling
│   │
│   ├── compression/               # Tensor compression
│   │   ├── mod.rs
│   │   ├── quantization.rs        # INT8/INT4 quantization
│   │   ├── low_rank.rs            # SVD/Tucker decomposition
│   │   ├── sparse_storage.rs      # CSR/COO sparse tensors
│   │   └── hierarchical.rs        # Layer-wise compression
│   │
│   ├── simd/                      # SIMD optimizations
│   │   ├── mod.rs
│   │   ├── avx2.rs                # AVX2 kernels
│   │   ├── avx512.rs              # AVX-512 kernels
│   │   ├── neon.rs                # ARM NEON kernels
│   │   ├── wasm_simd.rs           # WASM SIMD
│   │   └── dispatch.rs            # Runtime detection
│   │
│   ├── utils/                     # Utilities
│   │   ├── mod.rs
│   │   ├── math.rs                # Math primitives
│   │   ├── tensor.rs              # Tensor ops
│   │   ├── softmax.rs             # Numerically stable softmax
│   │   └── distances.rs           # Distance metrics
│   │
│   └── prelude.rs                 # Common imports
│
├── benches/                       # Benchmarks
│   ├── attention_benchmark.rs     # Core attention benchmarks
│   ├── geometric_benchmark.rs     # Geometric attention
│   ├── sparse_benchmark.rs        # Sparse patterns
│   └── comparison_benchmark.rs    # Mechanism comparison
│
├── tests/                         # Integration tests
│   ├── core_tests.rs              # Core attention tests
│   ├── geometric_tests.rs         # Geometric tests
│   ├── sparse_tests.rs            # Sparse pattern tests
│   ├── numerical_stability.rs     # Stability tests
│   └── property_tests.rs          # Property-based tests
│
├── ffi/                           # Foreign Function Interface
│   ├── wasm/                      # WASM bindings
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   └── lib.rs             # wasm-bindgen exports
│   │   └── tests/
│   │       └── web.rs             # Browser tests
│   │
│   └── napi/                      # NAPI-RS bindings
│       ├── Cargo.toml
│       ├── src/
│       │   └── lib.rs             # napi-derive exports
│       └── index.d.ts             # TypeScript definitions
│
├── cli/                           # CLI interface
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                # CLI entry point
│       ├── commands/              # CLI commands
│       │   ├── benchmark.rs       # Run benchmarks
│       │   ├── compare.rs         # Compare mechanisms
│       │   └── analyze.rs         # Analyze attention patterns
│       └── output.rs              # Formatting
│
├── examples/                      # Examples
│   ├── basic_attention.rs         # Hello world
│   ├── graph_attention.rs         # Graph-aware usage
│   ├── hnsw_integration.rs        # HNSW integration
│   ├── custom_mechanism.rs        # Extending the library
│   └── distributed_attention.rs   # Parallel processing
│
└── docs/                          # Documentation
    ├── design/                    # Design documents
    │   ├── architecture.md        # Architecture overview
    │   ├── api_design.md          # API design rationale
    │   └── performance.md         # Performance analysis
    │
    ├── guides/                    # User guides
    │   ├── getting_started.md     # Quick start
    │   ├── mechanism_guide.md     # Choosing mechanisms
    │   └── integration.md         # Integration guide
    │
    └── research/                  # Research notes
        ├── attention_mechanisms.md
        ├── benchmarks.md
        └── experiments.md
```

### 3.2 Module Responsibilities

#### Core Module (`src/core/`)
**Responsibility**: Foundational attention mechanisms and trait definitions

**Key Components**:
- `Attention` trait: Common interface for all mechanisms
- `ScaledDotProductAttention`: Baseline implementation
- `MultiHeadAttention`: Standard multi-head decomposition
- `AttentionConfig`: Configuration builders

**Dependencies**: `utils` only

---

#### Geometric Module (`src/geometric/`)
**Responsibility**: Geometry-aware attention mechanisms

**Key Components**:
- `HyperbolicAttention`: Poincaré ball operations
- `EdgeFeaturedAttention`: GAT-style with edge features
- `MixedCurvatureAttention`: Product space (Euclidean × Hyperbolic)

**Dependencies**: `core`, `utils`

---

#### Sparse Module (`src/sparse/`)
**Responsibility**: Efficient sparse attention patterns

**Key Components**:
- `LocalGlobalAttention`: Longformer-style
- `LinearAttention`: Kernel-based approximation
- `FlashAttention`: Memory-efficient tiling

**Dependencies**: `core`, `utils`, `simd` (optional)

---

#### Graph Module (`src/graph/`)
**Responsibility**: Graph structure-aware mechanisms

**Key Components**:
- `GraphRoPE`: Rotary embeddings for graph distance
- `CrossSpaceAttention`: Dual topology + latent space
- `HNSWAwareAttention`: HNSW layer encoding

**Dependencies**: `core`, `geometric`, `utils`

---

#### Adaptive Module (`src/adaptive/`)
**Responsibility**: Learned and adaptive attention

**Key Components**:
- `MoEAttention`: Mixture of experts routing
- `RLNavigator`: Reinforcement learning-based navigation
- `DynamicHeadAttention`: Runtime head count adjustment

**Dependencies**: `core`, `geometric`, `sparse`, `graph`, `training`

---

#### Training Module (`src/training/`)
**Responsibility**: Loss functions and optimization

**Key Components**:
- `ContrastiveLoss`: InfoNCE, Triplet
- `SpectralRegularizer`: Laplacian smoothness
- `HardNegativeSampler`: Mining hard negatives
- `CurriculumScheduler`: Loss weight scheduling

**Dependencies**: `utils` only

---

#### Compression Module (`src/compression/`)
**Responsibility**: Memory-efficient tensor storage

**Key Components**:
- `Quantizer`: INT8/INT4 quantization
- `LowRankFactorizer`: SVD compression
- `SparseStorage`: CSR/COO formats

**Dependencies**: `utils`, `simd` (optional)

---

#### SIMD Module (`src/simd/`)
**Responsibility**: Vectorized operations

**Key Components**:
- `SimdDispatcher`: Runtime CPU feature detection
- Platform-specific kernels: AVX2, AVX-512, NEON, WASM SIMD

**Dependencies**: `utils` only

---

### 3.3 Dependency Graph

```
┌─────────────────────────────────────────────────────────────┐
│                        Public API (lib.rs)                  │
└─────────────────────────────────────────────────────────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        v                    v                    v
  ┌──────────┐         ┌──────────┐        ┌──────────┐
  │   core   │         │ training │        │  utils   │
  └──────────┘         └──────────┘        └──────────┘
        │                                        │
        └────────┬───────────┬─────────┬────────┘
                 │           │         │
                 v           v         v
          ┌──────────┐ ┌─────────┐ ┌──────┐
          │geometric │ │ sparse  │ │ simd │
          └──────────┘ └─────────┘ └──────┘
                 │           │
                 └─────┬─────┘
                       │
                       v
                 ┌──────────┐
                 │  graph   │
                 └──────────┘
                       │
                       v
                 ┌──────────┐
                 │ adaptive │
                 └──────────┘
                       │
                       v
                ┌─────────────┐
                │ compression │
                └─────────────┘
```

**Design Principles**:
1. **Acyclic**: No circular dependencies
2. **Layered**: Lower layers have fewer dependencies
3. **Optional Features**: SIMD, compression via feature flags
4. **Core Stability**: `core` and `utils` are most stable

---

## 4. API Design

### 4.1 Core Trait: `Attention`

```rust
/// Core trait for all attention mechanisms
pub trait Attention: Send + Sync {
    /// Forward pass: compute attention over keys/values given query
    ///
    /// # Arguments
    /// * `query` - Query vector (d-dimensional)
    /// * `keys` - Key vectors (n × d)
    /// * `values` - Value vectors (n × d)
    ///
    /// # Returns
    /// Attention-weighted aggregation of values (d-dimensional)
    ///
    /// # Example
    /// ```
    /// use ruvector_attention::core::ScaledDotProductAttention;
    /// use ruvector_attention::Attention;
    ///
    /// let attn = ScaledDotProductAttention::new(128);
    /// let query = vec![1.0; 128];
    /// let keys = vec![vec![0.5; 128]; 10];
    /// let values = keys.clone();
    ///
    /// let output = attn.forward(&query, &keys, &values);
    /// assert_eq!(output.len(), 128);
    /// ```
    fn forward(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Result<Vec<f32>, AttentionError>;

    /// Get attention weights without computing weighted sum
    ///
    /// Useful for visualization and debugging
    fn attention_weights(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
    ) -> Result<Vec<f32>, AttentionError>;

    /// Get hidden dimension
    fn hidden_dim(&self) -> usize;

    /// Check if mechanism supports variable-length inputs
    fn supports_variable_length(&self) -> bool {
        true
    }

    /// Estimated computational complexity (for documentation)
    fn complexity(&self) -> Complexity {
        Complexity::Quadratic
    }
}

/// Computational complexity categories
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Complexity {
    Linear,      // O(n)
    Linearithmic, // O(n log n)
    Quadratic,   // O(n²)
    Custom(&'static str), // Custom complexity description
}
```

---

### 4.2 Error Handling

```rust
/// Errors that can occur during attention computation
#[derive(Debug, thiserror::Error)]
pub enum AttentionError {
    /// Input dimension mismatch
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Empty input
    #[error("Empty input: {context}")]
    EmptyInput { context: String },

    /// Numerical instability detected
    #[error("Numerical instability: {message}")]
    NumericalInstability { message: String },

    /// Invalid configuration
    #[error("Invalid configuration: {message}")]
    InvalidConfig { message: String },

    /// Out of bounds access
    #[error("Index out of bounds: {index} >= {len}")]
    OutOfBounds { index: usize, len: usize },

    /// Unsupported operation
    #[error("Unsupported operation: {operation}")]
    Unsupported { operation: String },

    /// Internal error
    #[error("Internal error: {message}")]
    Internal { message: String },
}

pub type Result<T> = std::result::Result<T, AttentionError>;
```

---

### 4.3 Builder Pattern

```rust
/// Builder for ScaledDotProductAttention
#[derive(Debug, Clone)]
pub struct ScaledDotProductAttentionBuilder {
    hidden_dim: usize,
    dropout: Option<f32>,
    temperature: f32,
    normalize: bool,
}

impl ScaledDotProductAttentionBuilder {
    pub fn new(hidden_dim: usize) -> Self {
        Self {
            hidden_dim,
            dropout: None,
            temperature: 1.0,
            normalize: true,
        }
    }

    pub fn dropout(mut self, rate: f32) -> Self {
        assert!((0.0..=1.0).contains(&rate), "Dropout must be in [0, 1]");
        self.dropout = Some(rate);
        self
    }

    pub fn temperature(mut self, temp: f32) -> Self {
        assert!(temp > 0.0, "Temperature must be positive");
        self.temperature = temp;
        self
    }

    pub fn normalize(mut self, normalize: bool) -> Self {
        self.normalize = normalize;
        self
    }

    pub fn build(self) -> Result<ScaledDotProductAttention> {
        if self.hidden_dim == 0 {
            return Err(AttentionError::InvalidConfig {
                message: "hidden_dim must be > 0".to_string(),
            });
        }

        Ok(ScaledDotProductAttention {
            hidden_dim: self.hidden_dim,
            scale: (self.hidden_dim as f32).sqrt().recip(),
            dropout: self.dropout,
            temperature: self.temperature,
            normalize: self.normalize,
        })
    }
}

// Usage:
let attn = ScaledDotProductAttention::builder(128)
    .dropout(0.1)
    .temperature(0.07)
    .build()?;
```

---

### 4.4 Multi-Head Attention API

```rust
/// Multi-head attention with configurable heads
pub struct MultiHeadAttention {
    num_heads: usize,
    head_dim: usize,
    hidden_dim: usize,
    w_q: Vec<Linear>,  // Query projections per head
    w_k: Vec<Linear>,  // Key projections per head
    w_v: Vec<Linear>,  // Value projections per head
    w_o: Linear,       // Output projection
    dropout: Option<f32>,
}

impl MultiHeadAttention {
    pub fn builder(hidden_dim: usize, num_heads: usize) -> MultiHeadAttentionBuilder {
        MultiHeadAttentionBuilder::new(hidden_dim, num_heads)
    }

    /// Get attention patterns for all heads
    pub fn head_attention_weights(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
    ) -> Result<Vec<Vec<f32>>> {
        // Returns [num_heads × num_keys] attention weights
        // Useful for interpretability
    }

    /// Get specific head output
    pub fn head_output(
        &self,
        head_idx: usize,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Result<Vec<f32>> {
        // Get output of a single head (for debugging)
    }
}

impl Attention for MultiHeadAttention {
    fn forward(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Result<Vec<f32>> {
        // 1. Project to heads: Q_i, K_i, V_i for each head i
        // 2. Compute attention per head: head_i = Attention(Q_i, K_i, V_i)
        // 3. Concatenate heads: concat(head_1, ..., head_h)
        // 4. Output projection: W_o @ concat
    }
}
```

---

### 4.5 Geometric Attention API

```rust
/// Hyperbolic attention in Poincaré ball
pub struct HyperbolicAttention {
    hidden_dim: usize,
    curvature: f32,  // Negative curvature (e.g., -1.0)
    w_q: Linear,
    w_k: Linear,
    w_v: Linear,
}

impl HyperbolicAttention {
    /// Create new hyperbolic attention
    ///
    /// # Arguments
    /// * `hidden_dim` - Embedding dimension
    /// * `curvature` - Curvature of hyperbolic space (must be negative)
    pub fn new(hidden_dim: usize, curvature: f32) -> Result<Self> {
        if curvature >= 0.0 {
            return Err(AttentionError::InvalidConfig {
                message: "Hyperbolic curvature must be negative".to_string(),
            });
        }
        // ...
    }

    /// Poincaré distance between two points
    pub fn poincare_distance(&self, x: &[f32], y: &[f32]) -> f32 {
        // d(x,y) = arccosh(1 + 2||x-y||² / ((1-||x||²)(1-||y||²)))
    }

    /// Möbius addition (hyperbolic vector addition)
    pub fn mobius_add(&self, x: &[f32], y: &[f32]) -> Vec<f32> {
        // (1+2⟨x,y⟩+||y||²)x + (1-||x||²)y / (1+2⟨x,y⟩+||x||²||y||²)
    }

    /// Project point onto Poincaré ball (clip to ||x|| < 1)
    pub fn project_to_ball(&self, x: &mut [f32], eps: f32) {
        let norm = l2_norm(x);
        if norm >= 1.0 - eps {
            let scale = (1.0 - eps) / norm;
            for xi in x.iter_mut() {
                *xi *= scale;
            }
        }
    }
}

impl Attention for HyperbolicAttention {
    fn forward(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Result<Vec<f32>> {
        // 1. Compute hyperbolic similarities: -d_poincare(q, k_j)
        // 2. Softmax attention weights
        // 3. Aggregate in hyperbolic space via Möbius operations
    }
}
```

---

### 4.6 Graph-Aware Attention API

```rust
/// Attention with graph-specific features
pub trait GraphAttention: Attention {
    /// Forward pass with edge features
    fn forward_with_edges(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
        edge_features: &[Vec<f32>],
    ) -> Result<Vec<f32>>;

    /// Forward pass with graph metadata
    fn forward_with_metadata(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
        metadata: &GraphMetadata,
    ) -> Result<Vec<f32>>;
}

/// Graph metadata for attention
#[derive(Debug, Clone)]
pub struct GraphMetadata {
    /// Graph distances (e.g., shortest path lengths)
    pub distances: Option<Vec<f32>>,

    /// HNSW layer indices
    pub hnsw_layers: Option<Vec<usize>>,

    /// Edge weights
    pub edge_weights: Option<Vec<f32>>,

    /// Structural features (degree, centrality, etc.)
    pub structural_features: Option<Vec<Vec<f32>>>,
}

/// RoPE-enhanced attention for graphs
pub struct GraphRoPE {
    hidden_dim: usize,
    base: f32,  // Frequency base (default 10000)
    w_q: Linear,
    w_k: Linear,
    w_v: Linear,
}

impl GraphRoPE {
    /// Apply rotation based on graph distance
    pub fn apply_rotation(&self, embedding: &[f32], distance: f32) -> Vec<f32> {
        // Rotate embedding by angle proportional to distance
    }
}

impl GraphAttention for GraphRoPE {
    fn forward_with_metadata(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
        metadata: &GraphMetadata,
    ) -> Result<Vec<f32>> {
        let distances = metadata.distances.as_ref()
            .ok_or_else(|| AttentionError::InvalidConfig {
                message: "GraphRoPE requires distance metadata".to_string(),
            })?;

        // Apply rotations based on distances
        // Compute attention with rotated embeddings
    }
}
```

---

### 4.7 Adaptive Attention API

```rust
/// Mixture of Experts attention
pub struct MoEAttention {
    router: Linear,  // Maps query to expert scores
    experts: Vec<Box<dyn Attention>>,
    top_k: usize,    // Number of experts to activate
}

impl MoEAttention {
    pub fn builder() -> MoEAttentionBuilder {
        MoEAttentionBuilder::new()
    }

    /// Get routing decisions
    pub fn get_routing(
        &self,
        query: &[f32],
    ) -> Result<Vec<(usize, f32)>> {
        // Returns (expert_index, weight) pairs
    }

    /// Add an expert to the mixture
    pub fn add_expert(&mut self, expert: Box<dyn Attention>) {
        self.experts.push(expert);
    }
}

impl Attention for MoEAttention {
    fn forward(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Result<Vec<f32>> {
        // 1. Route: scores = Router(query)
        // 2. Select top-k experts
        // 3. Weighted combination of expert outputs
    }
}

/// Builder for MoE attention
pub struct MoEAttentionBuilder {
    router_hidden_dim: usize,
    experts: Vec<Box<dyn Attention>>,
    top_k: usize,
}

impl MoEAttentionBuilder {
    pub fn add_local_expert(mut self, hidden_dim: usize) -> Self {
        self.experts.push(Box::new(
            ScaledDotProductAttention::new(hidden_dim).unwrap()
        ));
        self
    }

    pub fn add_hyperbolic_expert(mut self, hidden_dim: usize, curvature: f32) -> Self {
        self.experts.push(Box::new(
            HyperbolicAttention::new(hidden_dim, curvature).unwrap()
        ));
        self
    }

    pub fn add_sparse_expert(mut self, local_window: usize, global_nodes: usize) -> Self {
        self.experts.push(Box::new(
            LocalGlobalAttention::new(local_window, global_nodes).unwrap()
        ));
        self
    }

    pub fn top_k(mut self, k: usize) -> Self {
        self.top_k = k;
        self
    }

    pub fn build(self) -> Result<MoEAttention> {
        // Validation and construction
    }
}
```

---

### 4.8 Training Utilities API

```rust
/// Contrastive loss functions
pub mod losses {
    /// InfoNCE contrastive loss
    pub fn info_nce(
        anchor: &[f32],
        positives: &[&[f32]],
        negatives: &[&[f32]],
        temperature: f32,
    ) -> f32;

    /// Triplet loss
    pub fn triplet(
        anchor: &[f32],
        positive: &[f32],
        negative: &[f32],
        margin: f32,
    ) -> f32;

    /// Local contrastive loss (graph-specific)
    pub fn local_contrastive(
        node_embedding: &[f32],
        neighbor_embeddings: &[Vec<f32>],
        non_neighbor_embeddings: &[Vec<f32>],
        temperature: f32,
    ) -> f32;
}

/// Hard negative mining
pub mod hard_negatives {
    pub enum SamplingStrategy {
        Distance,      // Most similar non-neighbors
        Degree,        // Similar degree distribution
        Mixed,         // Combination
    }

    pub fn sample_hard_negatives(
        anchor: &[f32],
        all_embeddings: &[Vec<f32>],
        positive_indices: &[usize],
        k: usize,
        strategy: SamplingStrategy,
    ) -> Vec<Vec<f32>>;
}

/// Spectral regularization
pub mod regularizers {
    /// Laplacian smoothness
    pub fn laplacian(
        embeddings: &[Vec<f32>],
        edges: &[(usize, usize)],
        edge_weights: Option<&[f32]>,
    ) -> f32;

    /// Orthogonality regularization
    pub fn orthogonality(embeddings: &[Vec<f32>]) -> f32;

    /// Embedding norm regularization
    pub fn norm_penalty(embeddings: &[Vec<f32>], target_norm: f32) -> f32;
}
```

---

## 5. Performance Targets

### 5.1 Latency Targets

| Operation | Input Size | p50 | p95 | p99 |
|-----------|------------|-----|-----|-----|
| Scaled Dot-Product | 100 neighbors | <5ms | <10ms | <20ms |
| Scaled Dot-Product | 1K neighbors | <50ms | <100ms | <150ms |
| Multi-Head (4 heads) | 100 neighbors | <10ms | <20ms | <30ms |
| Multi-Head (4 heads) | 1K neighbors | <80ms | <150ms | <200ms |
| Hyperbolic | 100 neighbors | <15ms | <30ms | <50ms |
| Sparse (Local+Global) | 1K neighbors | <30ms | <60ms | <100ms |
| Flash Attention | 1K neighbors | <40ms | <80ms | <120ms |
| MoE (4 experts, top-2) | 1K neighbors | <100ms | <180ms | <250ms |

**Measurement Method**: Criterion.rs benchmarks with 1000 iterations, warm cache

---

### 5.2 Throughput Targets

| Mechanism | Target (ops/sec) | Stretch (ops/sec) |
|-----------|------------------|-------------------|
| Scaled Dot-Product | 10,000 | 20,000 |
| Multi-Head | 5,000 | 10,000 |
| Hyperbolic | 3,000 | 6,000 |
| Sparse | 8,000 | 15,000 |
| Flash | 7,000 | 12,000 |

**Measurement**: Batch processing of 1000 operations, averaged over 10 runs

---

### 5.3 Memory Targets

| Mechanism | Peak Memory (1K neighbors) | Target | Stretch |
|-----------|---------------------------|--------|---------|
| Scaled Dot-Product | Full attention matrix | <50MB | <25MB |
| Multi-Head (4 heads) | 4× attention matrices | <100MB | <50MB |
| Flash Attention | Tiled computation | <20MB | <10MB |
| Sparse | Sparse patterns only | <15MB | <8MB |

**Measurement**: Valgrind/heaptrack during benchmark execution

---

### 5.4 Compilation Targets

| Configuration | Target | Stretch |
|---------------|--------|---------|
| Debug build | <10s | <5s |
| Release build (--release) | <60s | <30s |
| Release with LTO | <120s | <60s |
| WASM build | <90s | <45s |

**Measurement**: CI build times on GitHub Actions standard runners

---

### 5.5 Binary Size Targets

| Target | Size (uncompressed) | Size (gzipped) | Target | Stretch |
|--------|---------------------|----------------|--------|---------|
| WASM | 5-8 MB | 1.5-2 MB | <2MB | <1MB |
| Native (Linux x86_64) | 10-15 MB | N/A | <15MB | <10MB |
| NAPI-RS addon | 8-12 MB | N/A | <12MB | <8MB |

**Measurement**: `wasm-opt` for WASM, `strip` for native

---

### 5.6 Scalability Targets

**Linear Scaling**:
- Operations should scale O(n) or better up to 10K neighbors
- No quadratic blowup in standard use cases

**Benchmark**:
```rust
#[bench]
fn bench_scalability_attention(b: &mut Bencher) {
    for n in [100, 500, 1000, 5000, 10000] {
        let attn = ScaledDotProductAttention::new(128).unwrap();
        let query = vec![1.0; 128];
        let keys = vec![vec![1.0; 128]; n];
        let values = keys.clone();

        let start = Instant::now();
        b.iter(|| attn.forward(&query, &keys, &values));
        let elapsed = start.elapsed();

        println!("n={}: {:?}", n, elapsed);
        // Assert linear or sub-quadratic scaling
    }
}
```

---

## 6. Compatibility Matrix

### 6.1 Rust Version Support

| Rust Version | Support Status | Notes |
|--------------|----------------|-------|
| 1.77.0 (MSRV) | ✅ Supported | Minimum supported version |
| 1.78.x | ✅ Supported | |
| 1.79.x | ✅ Supported | |
| 1.80.x+ | ✅ Supported | Latest stable |
| Nightly | ⚠️ Best-effort | May use unstable features behind flags |

**Testing**: CI runs on MSRV, stable, and nightly

---

### 6.2 Platform Support

#### Desktop Platforms

| Platform | Tier | Support Status | CI Testing |
|----------|------|----------------|------------|
| Linux x86_64 | Tier 1 | ✅ Full support | Yes |
| Linux ARM64 | Tier 2 | ✅ Full support | Yes |
| macOS x86_64 | Tier 1 | ✅ Full support | Yes |
| macOS ARM64 (M1/M2) | Tier 1 | ✅ Full support | Yes |
| Windows x86_64 | Tier 1 | ✅ Full support | Yes |
| Windows ARM64 | Tier 3 | ⚠️ Best-effort | No |

#### WASM Targets

| Target | Support Status | Notes |
|--------|----------------|-------|
| wasm32-unknown-unknown | ✅ Full support | Browser + Node.js |
| wasm32-wasi | ✅ Full support | WASI runtime |
| wasm32-unknown-emscripten | ⚠️ Untested | Should work |

**WASM Features**:
- ✅ All attention mechanisms
- ✅ SIMD support (where available)
- ✅ Multi-threading via Web Workers
- ❌ File I/O (not needed)

#### Mobile Platforms

| Platform | Support Status | Notes |
|----------|----------------|-------|
| iOS ARM64 | ⚠️ Untested | Should work via FFI |
| Android ARM64 | ⚠️ Untested | Should work via FFI |

---

### 6.3 Node.js Support (NAPI-RS)

| Node.js Version | Support Status | Notes |
|-----------------|----------------|-------|
| 18.x LTS | ✅ Supported | NAPI-RS requires N-API 9+ |
| 20.x LTS | ✅ Supported | Recommended |
| 21.x+ Current | ✅ Supported | Latest features |

**NAPI-RS Features**:
- ✅ All attention mechanisms exposed
- ✅ TypeScript definitions
- ✅ Async operations (Tokio runtime)
- ✅ Buffer zero-copy where possible

**Package Platforms**:
```json
{
  "napi": {
    "triples": {
      "defaults": true,
      "additional": [
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc"
      ]
    }
  }
}
```

---

### 6.4 Feature Flags

| Feature | Default | Description | Dependencies |
|---------|---------|-------------|--------------|
| `std` | ✅ | Standard library support | None |
| `simd` | ❌ | SIMD optimizations | `std` |
| `rayon` | ❌ | Parallel processing | `std`, `rayon` |
| `compression` | ❌ | Tensor compression | `std` |
| `wasm` | ❌ | WASM-specific bindings | `wasm-bindgen` |
| `napi` | ❌ | Node.js bindings | `napi-rs` |
| `cli` | ❌ | CLI interface | `std`, `clap` |
| `serde` | ✅ | Serialization support | `serde` |

**Example**:
```toml
[dependencies]
ruvector-attention = { version = "0.1", features = ["simd", "rayon"] }
```

---

## 7. Testing Strategy

### 7.1 Unit Tests

**Coverage Target**: >90% line coverage, >95% branch coverage

**Test Categories**:

#### 7.1.1 Correctness Tests
```rust
#[cfg(test)]
mod correctness_tests {
    use super::*;

    #[test]
    fn test_attention_output_dimension() {
        let attn = ScaledDotProductAttention::new(128).unwrap();
        let output = attn.forward(&query, &keys, &values).unwrap();
        assert_eq!(output.len(), 128);
    }

    #[test]
    fn test_attention_weights_sum_to_one() {
        let attn = ScaledDotProductAttention::new(128).unwrap();
        let weights = attn.attention_weights(&query, &keys).unwrap();
        let sum: f32 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_empty_neighbors_handling() {
        let attn = ScaledDotProductAttention::new(128).unwrap();
        let result = attn.forward(&query, &[], &[]);
        assert!(result.is_err());
        assert!(matches!(result, Err(AttentionError::EmptyInput { .. })));
    }
}
```

#### 7.1.2 Numerical Stability Tests
```rust
#[cfg(test)]
mod stability_tests {
    #[test]
    fn test_large_scores_softmax() {
        // Test softmax with very large scores (overflow risk)
        let scores = vec![1000.0, 999.0, 998.0];
        let weights = softmax(&scores);
        assert!(weights.iter().all(|&w| w.is_finite()));
    }

    #[test]
    fn test_small_scores_softmax() {
        // Test softmax with very small scores (underflow risk)
        let scores = vec![-1000.0, -999.0, -998.0];
        let weights = softmax(&scores);
        assert!(weights.iter().all(|&w| w.is_finite()));
    }

    #[test]
    fn test_hyperbolic_boundary() {
        let attn = HyperbolicAttention::new(128, -1.0).unwrap();
        let query = vec![0.99; 128]; // Near ball boundary
        let output = attn.forward(&query, &keys, &values).unwrap();

        // Output must stay inside ball
        assert!(l2_norm(&output) < 1.0);
    }
}
```

#### 7.1.3 Edge Case Tests
```rust
#[cfg(test)]
mod edge_case_tests {
    #[test]
    fn test_single_neighbor() {
        let attn = ScaledDotProductAttention::new(128).unwrap();
        let keys = vec![vec![1.0; 128]];
        let output = attn.forward(&query, &keys, &keys).unwrap();
        // With single neighbor, attention weight should be 1.0
    }

    #[test]
    fn test_identical_keys() {
        // All keys identical -> uniform attention
        let keys = vec![vec![1.0; 128]; 10];
        let weights = attn.attention_weights(&query, &keys).unwrap();
        for w in &weights {
            assert!((w - 0.1).abs() < 1e-5); // 1/10
        }
    }

    #[test]
    fn test_zero_vectors() {
        let query = vec![0.0; 128];
        let keys = vec![vec![0.0; 128]; 10];
        let result = attn.forward(&query, &keys, &keys);
        // Should handle gracefully (may return error or uniform weights)
    }
}
```

---

### 7.2 Integration Tests

**Goal**: Test interactions between modules

#### 7.2.1 Multi-Mechanism Pipeline
```rust
#[test]
fn test_moe_with_multiple_experts() {
    let moe = MoEAttention::builder()
        .add_local_expert(128)
        .add_hyperbolic_expert(128, -1.0)
        .add_sparse_expert(10, 5)
        .top_k(2)
        .build()
        .unwrap();

    let output = moe.forward(&query, &keys, &values).unwrap();
    assert_eq!(output.len(), 128);
}
```

#### 7.2.2 Graph Attention with HNSW
```rust
#[test]
fn test_graph_rope_with_hnsw_layers() {
    let rope = GraphRoPE::new(128, 10000.0).unwrap();

    let metadata = GraphMetadata {
        distances: Some(vec![1.0, 2.0, 3.0]),
        hnsw_layers: Some(vec![0, 1, 2]),
        ..Default::default()
    };

    let output = rope.forward_with_metadata(
        &query, &keys, &values, &metadata
    ).unwrap();

    assert_eq!(output.len(), 128);
}
```

---

### 7.3 Property-Based Tests

**Tool**: `proptest`

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_attention_weights_normalized(
        query in prop::collection::vec(-10.0f32..10.0, 128),
        keys in prop::collection::vec(
            prop::collection::vec(-10.0f32..10.0, 128),
            1..100
        )
    ) {
        let attn = ScaledDotProductAttention::new(128).unwrap();
        let weights = attn.attention_weights(&query, &keys).unwrap();

        let sum: f32 = weights.iter().sum();
        prop_assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn prop_attention_output_finite(
        query in prop::collection::vec(-100.0f32..100.0, 128),
        keys in prop::collection::vec(
            prop::collection::vec(-100.0f32..100.0, 128),
            1..100
        )
    ) {
        let attn = ScaledDotProductAttention::new(128).unwrap();
        let values = keys.clone();
        let output = attn.forward(&query, &keys, &values).unwrap();

        prop_assert!(output.iter().all(|&x| x.is_finite()));
    }
}
```

---

### 7.4 Benchmark Tests

**Tool**: `criterion`

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_scaled_dot_product(c: &mut Criterion) {
    let attn = ScaledDotProductAttention::new(128).unwrap();
    let query = vec![1.0; 128];
    let keys = vec![vec![1.0; 128]; 1000];
    let values = keys.clone();

    c.bench_function("scaled_dot_product_1k", |b| {
        b.iter(|| {
            attn.forward(
                black_box(&query),
                black_box(&keys),
                black_box(&values)
            )
        })
    });
}

fn bench_multi_head(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_head_attention");

    for num_heads in [1, 2, 4, 8] {
        let attn = MultiHeadAttention::builder(128, num_heads)
            .build()
            .unwrap();

        group.bench_function(format!("heads_{}", num_heads), |b| {
            b.iter(|| {
                attn.forward(
                    black_box(&query),
                    black_box(&keys),
                    black_box(&values)
                )
            })
        });
    }

    group.finish();
}

criterion_group!(benches, bench_scaled_dot_product, bench_multi_head);
criterion_main!(benches);
```

---

### 7.5 Fuzzing

**Tool**: `cargo-fuzz`

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use ruvector_attention::core::ScaledDotProductAttention;
use ruvector_attention::Attention;

fuzz_target!(|data: &[u8]| {
    if data.len() < 512 {
        return;
    }

    // Parse fuzzer input into query, keys, values
    let query: Vec<f32> = data[0..128]
        .chunks(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    // ... similar for keys and values

    let attn = ScaledDotProductAttention::new(32).unwrap();

    // Fuzz target: should never panic
    let _ = attn.forward(&query, &keys, &values);
});
```

---

### 7.6 WASM Tests

```rust
#[cfg(target_arch = "wasm32")]
#[cfg(test)]
mod wasm_tests {
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_attention_in_wasm() {
        let attn = ScaledDotProductAttention::new(128).unwrap();
        let output = attn.forward(&query, &keys, &values).unwrap();
        assert_eq!(output.len(), 128);
    }

    #[wasm_bindgen_test]
    fn test_simd_in_wasm() {
        #[cfg(feature = "simd")]
        {
            // Test WASM SIMD operations
        }
    }
}
```

---

### 7.7 Performance Regression Tests

**CI Check**: Fail if performance degrades >5% from baseline

```rust
#[test]
fn test_performance_regression() {
    let baseline_latency_ms = 100.0; // From previous run

    let start = Instant::now();
    let attn = ScaledDotProductAttention::new(128).unwrap();
    for _ in 0..1000 {
        attn.forward(&query, &keys, &values).unwrap();
    }
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

    let current_latency_ms = elapsed / 1000.0;
    let regression = (current_latency_ms - baseline_latency_ms) / baseline_latency_ms;

    assert!(
        regression < 0.05,
        "Performance regression detected: {}%", regression * 100.0
    );
}
```

---

## 8. Success Criteria

### 8.1 Quantifiable Metrics

#### 8.1.1 Functional Completeness
- [ ] **10/10 attention mechanisms implemented** (100%)
- [ ] **All mechanisms pass unit tests** (100% pass rate)
- [ ] **Integration tests pass** (100% pass rate)

#### 8.1.2 Performance
- [ ] **Latency**: p95 <200ms @ 1K neighbors for all mechanisms
- [ ] **Throughput**: >5,000 ops/sec for scaled dot-product
- [ ] **Memory**: Peak usage <50MB per operation
- [ ] **Scalability**: Linear or sub-quadratic up to 10K neighbors

#### 8.1.3 Quality
- [ ] **Test coverage**: >90% line coverage
- [ ] **Documentation coverage**: 100% public APIs documented
- [ ] **Zero compiler warnings**: Clippy clean
- [ ] **Zero unsafe code**: Or 100% audited and justified

#### 8.1.4 Compatibility
- [ ] **Platforms**: Linux, macOS, Windows passing CI
- [ ] **WASM**: All tests pass in wasm32-unknown-unknown
- [ ] **NAPI-RS**: Node.js 18+, all platforms published
- [ ] **MSRV**: Rust 1.77+ supported

#### 8.1.5 Adoption
- [ ] **Examples**: 5+ runnable examples
- [ ] **Documentation**: Getting started guide, API docs, tutorials
- [ ] **Integration**: Used in ruvector-gnn crate

---

### 8.2 Acceptance Tests

#### Phase 1 Acceptance (Weeks 1-4)
```
✅ Core attention mechanisms (scaled dot-product, multi-head)
✅ Unit tests passing (>80% coverage)
✅ Basic benchmarks established
✅ API design finalized
```

#### Phase 2 Acceptance (Weeks 5-8)
```
✅ Geometric attention (hyperbolic, edge-featured)
✅ Integration tests with graph structures
✅ Performance targets met for core mechanisms
✅ WASM compatibility verified
```

#### Phase 3 Acceptance (Weeks 9-12)
```
✅ Sparse mechanisms (flash, linear, local+global)
✅ Memory targets met
✅ NAPI-RS bindings complete
✅ Documentation 50% complete
```

#### Phase 4 Acceptance (Weeks 13-16)
```
✅ Adaptive mechanisms (MoE, cross-attention)
✅ Training utilities complete
✅ CLI interface functional
✅ All performance targets met
```

#### Phase 5 Acceptance (Weeks 17-20)
```
✅ Full integration with ruvector-gnn
✅ Documentation 100% complete
✅ Optimization passes complete
✅ Ready for 1.0 release
```

---

### 8.3 Release Criteria (v1.0)

**Blocker Issues** (must fix before release):
- [ ] Zero failing tests
- [ ] Zero compiler warnings
- [ ] All performance targets met
- [ ] 100% public API documented
- [ ] Security audit complete
- [ ] Cross-platform CI passing

**Nice-to-Have** (can defer to 1.1):
- [ ] GPU acceleration (CUDA/Metal)
- [ ] Additional attention variants
- [ ] Advanced SIMD optimizations
- [ ] Distributed attention

---

## 9. Constraints and Dependencies

### 9.1 Technical Constraints

#### C-001: No GPU Dependency
**Constraint**: All implementations must run on CPU
**Rationale**: WASM and NAPI-RS environments lack GPU access
**Impact**: May limit performance for very large graphs
**Mitigation**: SIMD optimizations, algorithm choice (sparse/linear attention)

#### C-002: Memory Constraints in WASM
**Constraint**: WASM has limited memory (typically 2-4GB)
**Rationale**: Browser and Node.js WASM environments
**Impact**: Cannot materialize large attention matrices
**Mitigation**: Flash Attention, sparse patterns, streaming computation

#### C-003: Serialization Requirements
**Constraint**: All types must be serializable (serde)
**Rationale**: Model saving/loading, network transfer
**Impact**: Design complexity, trait object limitations
**Mitigation**: Enum-based polymorphism, careful trait design

---

### 9.2 Dependencies

#### Core Dependencies

```toml
[dependencies]
# Math and numerics
ndarray = { version = "0.16", default-features = false }
rand = { version = "0.8", default-features = false }
rand_distr = { version = "0.4", default-features = false }

# Serialization
serde = { version = "1.0", features = ["derive"], optional = true }
rkyv = { version = "0.8", optional = true }

# Error handling
thiserror = "2.0"

# Optional: SIMD
simsimd = { version = "5.9", optional = true, features = ["nightly"] }

# Optional: Parallel processing
rayon = { version = "1.10", optional = true }

# Optional: WASM
wasm-bindgen = { version = "0.2", optional = true }
js-sys = { version = "0.3", optional = true }

# Optional: NAPI-RS
napi = { version = "2.16", optional = true }
napi-derive = { version = "2.16", optional = true }
```

**Dependency Audit**: All dependencies must be MIT/Apache-2.0 licensed

---

### 9.3 Integration Dependencies

#### Upstream (Used By)
- `ruvector-gnn`: Uses attention mechanisms in GNN layers
- `ruvector-graph`: Graph construction with attention-based edge selection

#### Downstream (Depends On)
- `ruvector-core`: Core vector operations, distance metrics
- `hnsw_rs`: HNSW graph structure (optional, for examples)

---

## 10. Risk Assessment

### 10.1 Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **Hyperbolic numerical instability** | High | Medium | Careful boundary handling, epsilon clipping, extensive testing |
| **WASM performance degradation** | Medium | High | WASM SIMD, algorithmic optimizations, benchmarking |
| **Memory bloat in large graphs** | Medium | High | Flash Attention, sparse patterns, streaming |
| **API breaking changes** | Low | High | Careful API design, SemVer, deprecation warnings |
| **Dependency conflicts** | Low | Medium | Minimal dependencies, version pinning |

---

### 10.2 Schedule Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **Hyperbolic implementation complexity** | Medium | Medium | 20% buffer time, fallback to Euclidean |
| **Performance targets not met** | Low | High | Early benchmarking, iterative optimization |
| **WASM/NAPI-RS compatibility issues** | Low | Medium | Early CI setup, continuous testing |

**Buffer**: 20% time buffer in each phase for unexpected issues

---

### 10.3 Operational Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **CI infrastructure failures** | Low | Low | GitHub Actions redundancy, local testing |
| **Documentation drift** | Medium | Medium | Doc tests, CI doc generation checks |
| **Contributor onboarding difficulty** | Medium | Low | Comprehensive docs, clear examples |

---

## 11. Open Questions

### 11.1 Design Questions

**Q1**: Should we support dynamic mechanism selection at runtime?
**Options**:
- A) Enum-based (`AttentionMechanism::ScaledDotProduct`)
- B) Trait objects (`Box<dyn Attention>`)
- C) Both

**Q2**: How to handle attention visualization?
**Options**:
- A) Return attention weights separately
- B) Integrate with vis library (e.g., `plotters`)
- C) Export to JSON for external tools

**Q3**: Should we support distributed attention computation?
**Options**:
- A) In-crate via `rayon`
- B) External crate (e.g., `ruvector-attention-distributed`)
- C) Defer to v2.0

---

### 11.2 API Questions

**Q4**: Naming convention for attention mechanisms?
**Options**:
- A) Descriptive (`ScaledDotProductAttention`)
- B) Abbreviated (`SDPAttention`)
- C) Mixed (long in code, short in docs)

**Q5**: Should builders be mandatory or optional?
**Options**:
- A) Mandatory (always use builder)
- B) Optional (provide `new()` for defaults)
- C) Hybrid (simple types use `new()`, complex use builder)

---

## Appendix A: Glossary

| Term | Definition |
|------|------------|
| **Attention** | Mechanism for weighted aggregation based on learned similarities |
| **Scaled Dot-Product** | `Attention(Q,K,V) = softmax(QK^T/√d) V` |
| **Multi-Head** | Parallel attention mechanisms with different projections |
| **Hyperbolic** | Non-Euclidean geometry with negative curvature |
| **Poincaré Ball** | Model of hyperbolic space as unit ball |
| **GAT** | Graph Attention Networks |
| **RoPE** | Rotary Position Embeddings |
| **Flash Attention** | Memory-efficient tiled attention computation |
| **MoE** | Mixture of Experts (learned routing between mechanisms) |
| **InfoNCE** | Contrastive loss function |
| **HNSW** | Hierarchical Navigable Small World graphs |

---

## Appendix B: References

### Research Papers

1. **Attention Mechanism**: Vaswani et al. (2017) - "Attention Is All You Need"
2. **GAT**: Veličković et al. (2018) - "Graph Attention Networks"
3. **Hyperbolic**: Chami et al. (2019) - "Hyperbolic Graph Convolutional Neural Networks"
4. **Flash Attention**: Dao et al. (2022) - "FlashAttention: Fast and Memory-Efficient Exact Attention"
5. **Performer**: Choromanski et al. (2020) - "Rethinking Attention with Performers"
6. **RoPE**: Su et al. (2021) - "RoFormer: Enhanced Transformer with Rotary Position Embedding"
7. **MoE**: Shazeer et al. (2017) - "Outrageously Large Neural Networks: The Sparsely-Gated Mixture-of-Experts Layer"

### RuVector Research Documents

- `/docs/latent-space/attention-mechanisms-research.md`
- `/docs/latent-space/gnn-architecture-analysis.md`
- `/docs/latent-space/optimization-strategies.md`
- `/docs/latent-space/implementation-roadmap.md`

### External Resources

- [Rust WASM Book](https://rustwasm.github.io/book/)
- [NAPI-RS Documentation](https://napi.rs/)
- [Criterion.rs Guide](https://bheisler.github.io/criterion.rs/book/)
- [Proptest Book](https://proptest-rs.github.io/proptest/)

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-11-30 | RuVector Team | Initial specification |

---

## Approvals

| Role | Name | Signature | Date |
|------|------|-----------|------|
| **Technical Lead** | | | |
| **Architecture Review** | | | |
| **QA Lead** | | | |
| **Product Owner** | | | |

---

**END OF SPECIFICATION**

This document represents the complete specification for the `ruvector-attention` crate. Implementation should proceed according to the SPARC methodology:
- **S**pecification ✅ (this document)
- **P**seudocode (next phase)
- **A**rchitecture (detailed design)
- **R**efinement (iterative TDD)
- **C**ompletion (integration and release)
