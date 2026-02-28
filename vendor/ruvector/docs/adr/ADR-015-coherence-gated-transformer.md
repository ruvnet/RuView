# ADR-015: Coherence-Gated Transformer (Sheaf Attention)

**Status**: Proposed
**Date**: 2026-01-22
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board
**Target Crate**: `ruvector-attention`

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-01-22 | ruv.io | Initial proposal for coherence-gated attention |

---

## Context

### The Transformer Latency Problem

Standard transformers have fundamental efficiency issues:

1. **Quadratic attention**: O(N²) for sequence length N
2. **Fixed computation**: Every token gets same compute regardless of difficulty
3. **Dense by default**: All attention weights computed even when most are near-zero
4. **Confidence-based exits**: Early exit uses unreliable confidence scores

### Existing Solutions and Their Limits

| Approach | Method | Limitation |
|----------|--------|------------|
| Flash Attention | Memory-efficient matmul | Still O(N²) compute |
| Sparse Attention | Fixed patterns (local, strided) | Patterns don't adapt to content |
| Linear Attention | Kernel approximation | Quality degradation |
| Early Exit | Confidence threshold | Confidence ≠ correctness |
| MoE | Expert routing | Routing is learned, not principled |

### The Coherence Insight

Prime-Radiant's coherence engine provides a **mathematically grounded** measure of consistency. This can be applied to attention:

> **Core idea**: Tokens that are already coherent with context don't need expensive attention. Route computation based on coherence energy, not learned confidence.

---

## Decision

### Implement Coherence-Gated Transformer (CGT) in `ruvector-attention`

A novel attention mechanism that uses sheaf coherence to:
1. **Route tokens** to different compute depths
2. **Sparsify attention** based on residual energy
3. **Exit early** when energy converges
4. **Replace QKV projections** with restriction maps

---

## Architecture

### High-Level Design

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     COHERENCE-GATED TRANSFORMER (CGT)                        │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │                         INPUT PROCESSING                                 ││
│  │  Tokens ──► Embedding ──► Initial Coherence Graph                       ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │                      COHERENCE ROUTER                                    ││
│  │                                                                          ││
│  │  For each token t:                                                       ││
│  │    E(t) = Σ w_e ||ρ_t(x_t) - ρ_ctx(x_ctx)||²                            ││
│  │                                                                          ││
│  │    Route based on energy:                                                ││
│  │    ┌──────────────┬──────────────┬──────────────┐                       ││
│  │    │ E < θ_reflex │ E < θ_std   │ E ≥ θ_std    │                       ││
│  │    │     │        │     │        │     │        │                       ││
│  │    │     ▼        │     ▼        │     ▼        │                       ││
│  │    │  LANE 0      │  LANE 1      │  LANE 2      │                       ││
│  │    │  Reflex      │  Standard    │  Deep        │                       ││
│  │    └──────────────┴──────────────┴──────────────┘                       ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                                    │                                         │
│       ┌────────────────────────────┼────────────────────────────┐           │
│       │                            │                            │           │
│       ▼                            ▼                            ▼           │
│  ┌──────────┐               ┌──────────┐               ┌──────────┐        │
│  │  LANE 0  │               │  LANE 1  │               │  LANE 2  │        │
│  │  REFLEX  │               │ STANDARD │               │   DEEP   │        │
│  │          │               │          │               │          │        │
│  │ • 1-2 layers            │ • 6 layers│               │ • 12+ layers      │
│  │ • Local attention       │ • Sparse  │               │ • Full + MoE     │
│  │   (window=64)           │   sheaf   │               │ • All experts    │
│  │ • No FFN                │   attn    │               │ • Spectral       │
│  │ • <0.1ms                │ • ~1ms    │               │   analysis       │
│  │                         │           │               │ • ~5ms           │
│  └──────────┘               └──────────┘               └──────────┘        │
│       │                            │                            │           │
│       └────────────────────────────┼────────────────────────────┘           │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │                      COHERENCE VERIFICATION                              ││
│  │                                                                          ││
│  │  E_final = compute_energy(output_graph)                                  ││
│  │                                                                          ││
│  │  if E_final > θ_max:                                                     ││
│  │    → Escalate to Lane 2 OR refuse generation                            ││
│  │  else:                                                                   ││
│  │    → Output with witness                                                 ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                                    │                                         │
│                                    ▼                                         │
│                           Output + Witness                                   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Component Details

#### 1. Sheaf Attention Layer

Replace standard scaled dot-product attention with coherence-based attention:

```
Standard Attention:
  Attention(Q, K, V) = softmax(QK^T / √d) V

Sheaf Attention:
  R_ij = ||ρ_i(x_i) - ρ_j(x_j)||²           # Residual energy
  A_ij = exp(-β × R_ij) / Σ_k exp(-β × R_ik) # Coherence-based weight
  Output = A × V
```

**Key difference**: Attention weight is inversely proportional to residual energy.
- High residual (incoherent) → Low attention (don't propagate inconsistency)
- Low residual (coherent) → High attention (reinforce consistency)

#### 2. Restriction Map Projections

Replace learned W_q, W_k, W_v with restriction maps:

```
Standard:
  Q = W_q × x    (learned projection)
  K = W_k × x
  V = W_v × x

Sheaf:
  Q = ρ_q(x)     (restriction map to query manifold)
  K = ρ_k(x)     (restriction map to key manifold)
  V = ρ_v(x)     (restriction map to value manifold)
```

**Benefits**:
- Restriction maps have geometric meaning (project to shared space)
- Can be initialized from domain knowledge
- Residuals are interpretable

#### 3. Token-Level Compute Routing

```python
def route_token(token_embedding, context_graph):
    # Compute coherence energy with context
    energy = compute_token_energy(token_embedding, context_graph)

    if energy < THETA_REFLEX:
        return Lane.REFLEX      # Minimal compute
    elif energy < THETA_STANDARD:
        return Lane.STANDARD    # Normal compute
    else:
        return Lane.DEEP        # Maximum compute
```

**Routing thresholds** (tunable via SONA):

| Threshold | Default | Meaning |
|-----------|---------|---------|
| θ_reflex | 0.01 | Token is highly coherent with context |
| θ_standard | 0.1 | Token has minor inconsistencies |
| θ_deep | 1.0 | Token has major inconsistencies |

#### 4. Residual-Sparse Attention

Only compute attention for token pairs with high residual:

```python
def sparse_sheaf_attention(X, threshold):
    N = len(X)
    attention_mask = zeros(N, N)

    for i in range(N):
        for j in range(N):
            residual = compute_residual(X[i], X[j])
            if residual > threshold:
                # These tokens are incoherent - need attention
                attention_mask[i, j] = 1
            # else: skip attention (already coherent)

    # Compute attention only for non-zero mask entries
    return masked_attention(X, attention_mask)
```

**Sparsity pattern**: Adapts to content, not fixed like local/strided attention.

#### 5. Energy-Based Early Exit

```python
def forward_with_early_exit(x, layers, epsilon=0.001):
    prev_energy = float('inf')

    for layer in layers:
        x = layer(x)
        curr_energy = compute_energy(x)

        delta = abs(curr_energy - prev_energy)
        if delta < epsilon:
            # Energy converged - no need for more layers
            return x

        prev_energy = curr_energy

    return x
```

**Exit criterion**: Energy convergence, not confidence threshold.

---

## Compute Lane Specifications

### Lane 0: Reflex (~0.1ms)

```
Layers: 1-2
Attention: Local only (window=64)
FFN: Skip or minimal
Use case: Common tokens, clear context
Example: "the", "is", "and" in well-formed sentences
```

### Lane 1: Standard (~1ms)

```
Layers: 6
Attention: Sparse sheaf (residual > 0.05)
FFN: Standard
Use case: Normal tokens requiring context integration
Example: Most content words
```

### Lane 2: Deep (~5ms)

```
Layers: 12+
Attention: Full sheaf + MoE routing
FFN: Expert mixture
Spectral: Eigenvalue analysis for structural issues
Use case: Ambiguous, contradictory, or complex tokens
Example: "bank" (river or financial?), negations, rare words
```

### Lane 3: Escalate (async)

```
Action: Return uncertainty, request clarification
Use case: Irreconcilable incoherence
Example: "The cat is not a cat" - logical contradiction
```

---

## Mathematical Foundation

### Sheaf Attention Formula

Given tokens X = {x_1, ..., x_N} and restriction maps ρ_i, ρ_j:

**Residual**:
```
r_ij = ρ_i(x_i) - ρ_j(x_j)
```

**Edge energy**:
```
E_ij = w_ij × ||r_ij||²
```

**Token energy**:
```
E_i = Σ_j E_ij  (sum over edges incident to i)
```

**Attention weight** (coherence-based):
```
A_ij = exp(-β × E_ij) / Σ_k exp(-β × E_ik)
```

**Output**:
```
y_i = Σ_j A_ij × V_j
```

### Complexity Analysis

| Operation | Standard | Sheaf (Dense) | Sheaf (Sparse, s% non-zero) |
|-----------|----------|---------------|----------------------------|
| Attention | O(N²d) | O(N²d) | O(s×N²d) |
| Routing | - | O(Nd) | O(Nd) |
| Early exit | - | O(Ld) per check | O(Ld) per check |
| **Total** | O(N²Ld) | O(N²Ld) | O(s×N²Ld + routing) |

With typical s=10-20% sparsity and 50% early exit: **5-10x speedup**.

---

## Integration with `ruvector-attention`

### New Modules

```
ruvector-attention/
├── src/
│   ├── sheaf/                      # NEW: Sheaf attention
│   │   ├── mod.rs
│   │   ├── attention.rs            # SheafAttention layer
│   │   ├── restriction.rs          # Restriction map projections
│   │   ├── router.rs               # Token-level routing
│   │   ├── sparse.rs               # Residual-sparse attention
│   │   └── early_exit.rs           # Energy-based early exit
│   │
│   ├── coherence_gated/            # NEW: Full CGT implementation
│   │   ├── mod.rs
│   │   ├── transformer.rs          # CoherenceGatedTransformer
│   │   ├── lane.rs                 # ComputeLane enum + configs
│   │   ├── config.rs               # CGTConfig
│   │   └── benchmark.rs            # Latency/quality benchmarks
│   │
│   └── ... (existing modules)
```

### New Types

```rust
/// Sheaf-based attention layer
pub struct SheafAttention {
    /// Restriction map for queries
    pub rho_query: RestrictionMap,
    /// Restriction map for keys
    pub rho_key: RestrictionMap,
    /// Restriction map for values
    pub rho_value: RestrictionMap,
    /// Temperature for attention softmax
    pub beta: f32,
    /// Sparsity threshold
    pub sparsity_threshold: f32,
}

/// Compute lane for token routing
#[derive(Debug, Clone, Copy)]
pub enum ComputeLane {
    /// Minimal compute (<0.1ms)
    Reflex,
    /// Standard compute (~1ms)
    Standard,
    /// Deep compute (~5ms)
    Deep,
    /// Escalate to caller
    Escalate,
}

/// Coherence-Gated Transformer configuration
pub struct CGTConfig {
    /// Embedding dimension
    pub d_model: usize,
    /// Layers per lane
    pub layers_per_lane: [usize; 3],  // [reflex, standard, deep]
    /// Routing thresholds
    pub thresholds: CoherenceThresholds,
    /// Sparsity settings
    pub sparsity: SparsityConfig,
    /// Early exit settings
    pub early_exit: EarlyExitConfig,
}

/// Token routing decision
pub struct RoutingDecision {
    pub token_id: usize,
    pub energy: f32,
    pub lane: ComputeLane,
    pub attention_mask: Option<SparseMask>,
}
```

### Feature Flags

```toml
[features]
# Sheaf attention (requires prime-radiant)
sheaf = ["dep:prime-radiant"]

# Full CGT implementation
coherence-gated = ["sheaf", "sparse", "moe"]

# Benchmarking utilities
cgt-bench = ["coherence-gated", "criterion"]
```

---

## Performance Targets

| Metric | Standard Transformer | CGT Target | Improvement |
|--------|---------------------|------------|-------------|
| Average latency (128 tokens) | 10ms | 1-2ms | 5-10x |
| P99 latency (128 tokens) | 15ms | 8ms | 2x |
| Memory (batch=32) | 2GB | 800MB | 2.5x |
| Quality (perplexity) | Baseline | <5% degradation | Acceptable |

### Latency Breakdown

```
Standard (10ms total):
  Attention: 6ms (60%)
  FFN: 3ms (30%)
  Other: 1ms (10%)

CGT Target (2ms total):
  Routing: 0.1ms (5%)
  Attention (sparse): 1ms (50%)
  FFN (conditional): 0.7ms (35%)
  Other: 0.2ms (10%)
```

---

## Quality Guarantees

### Coherence Bound

Every output is guaranteed to have coherence energy below threshold:

```
E(output) < θ_max  OR  escalate/refuse
```

This is **stronger** than confidence-based systems which can be confidently wrong.

### Graceful Degradation

Under compute pressure:
1. Raise θ_reflex → more tokens to Lane 0
2. Increase sparsity threshold → fewer attention computations
3. Quality degrades **predictably** (energy increases)

### Interpretability

For any output:
- Which tokens went to which lane?
- Which token pairs had high residuals?
- Where did the model "struggle"?

---

## Comparison with Existing Approaches

| Feature | Flash Attention | Sparse Transformers | MoE | CGT (Ours) |
|---------|-----------------|---------------------|-----|------------|
| Adaptive compute | No | No | Yes | Yes |
| Content-based sparsity | No | No | Partial | Yes |
| Mathematical grounding | No | No | No | Yes (sheaf) |
| Quality guarantee | No | No | No | Yes (energy bound) |
| Interpretable routing | N/A | N/A | Partial | Yes |
| Early exit criterion | N/A | N/A | Confidence | Energy convergence |

---

## Research Questions

1. **Restriction map initialization**: Random vs. pre-trained vs. analytical?

2. **Threshold tuning**: Can SONA auto-tune θ values during inference?

3. **Multi-head sheaf attention**: One graph per head, or shared graph?

4. **Training objective**: Standard cross-entropy + energy regularization?

5. **Hardware optimization**: Can residual computation be fused with attention kernels?

---

## Implementation Phases

### Phase 1: Foundation (Weeks 1-4)
- [ ] `SheafAttention` layer with restriction maps
- [ ] Basic residual computation
- [ ] Unit tests for mathematical correctness

### Phase 2: Routing (Weeks 5-8)
- [ ] `ComputeLane` enum and routing logic
- [ ] Token-level energy computation
- [ ] Lane-specific layer configurations

### Phase 3: Sparsity (Weeks 9-12)
- [ ] Residual-sparse attention mask generation
- [ ] Efficient sparse attention kernel
- [ ] Sparsity pattern analysis tools

### Phase 4: Integration (Weeks 13-16)
- [ ] `CoherenceGatedTransformer` full implementation
- [ ] Early exit with energy convergence
- [ ] Benchmarking suite

### Phase 5: Optimization (Weeks 17-20)
- [ ] SIMD optimization for residual computation
- [ ] Kernel fusion opportunities
- [ ] SONA integration for threshold tuning

---

## Dependencies

### Required
- `prime-radiant` (coherence computation)
- `ruvector-core` (vector operations)
- `ndarray` (matrix operations)

### Optional
- `rayon` (parallel routing)
- `criterion` (benchmarking)

---

## References

1. Hansen, J., & Ghrist, R. (2019). "Toward a spectral theory of cellular sheaves."

2. Vaswani et al. (2017). "Attention Is All You Need."

3. Kitaev et al. (2020). "Reformer: The Efficient Transformer."

4. Fedus et al. (2022). "Switch Transformers: Scaling to Trillion Parameter Models."

5. ADR-014: Coherence Engine Architecture

---

## Related Decisions

- **ADR-014**: Coherence Engine Architecture (Prime-Radiant)
- **ADR-003**: SIMD Optimization Strategy
- **ADR-006**: Memory Management

---

## Appendix: Name Options

| Name | Rationale |
|------|-----------|
| **Coherence-Gated Transformer (CGT)** | Descriptive, clear function |
| **Sheaf Attention** | Mathematical foundation |
| **Residual-Routed Transformer** | Emphasizes routing mechanism |
| **Energy-Adaptive Transformer** | Emphasizes efficiency |
| **Prime Transformer** | Connection to Prime-Radiant |

**Recommended**: "Coherence-Gated Transformer (CGT)" for the architecture, "Sheaf Attention" for the attention mechanism.
