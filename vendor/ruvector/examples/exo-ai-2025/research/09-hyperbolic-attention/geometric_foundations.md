# Geometric Foundations of Hyperbolic Attention

## Mathematical Prerequisites

This document provides rigorous mathematical foundations for implementing hyperbolic attention mechanisms with **provable geometric properties**.

---

## Table of Contents

1. [Hyperbolic Geometry Basics](#hyperbolic-geometry-basics)
2. [Poincaré Ball Model](#poincaré-ball-model)
3. [Lorentz (Hyperboloid) Model](#lorentz-hyperboloid-model)
4. [Isometries and Transformations](#isometries-and-transformations)
5. [Hyperbolic Neural Operations](#hyperbolic-neural-operations)
6. [Attention Mechanisms in Hyperbolic Space](#attention-mechanisms-in-hyperbolic-space)
7. [Curvature Adaptation](#curvature-adaptation)
8. [Numerical Stability](#numerical-stability)
9. [Complexity Analysis](#complexity-analysis)

---

## Hyperbolic Geometry Basics

### Definition

**Hyperbolic space** ℍⁿ is a complete, simply-connected Riemannian manifold of constant **negative curvature** κ < 0.

**Key Properties**:
1. **Exponential volume growth**: Volume of ball of radius r grows as ~exp(r√|κ|)
2. **Unique geodesics**: Any two points connected by unique shortest path
3. **Triangle inequality**: sum of angles < π (vs = π in Euclidean)
4. **Tree embedding**: Finite trees embed with arbitrarily low distortion in ℍ²

### Curvature Parameter

Define **curvature radius** K > 0 such that κ = -1/K².

**Normalization**:
- **κ = -1**: Unit hyperbolic space (mathematical convention)
- **κ = -1/K²**: Learnable curvature (K is learned parameter)

### Models of Hyperbolic Space

Five isometric models:
1. **Poincaré ball**: {x ∈ ℝⁿ : ||x|| < 1}
2. **Lorentz (hyperboloid)**: {x ∈ ℝⁿ⁺¹ : ⟨x,x⟩_L = -1, x₀ > 0}
3. **Poincaré half-space**: {x ∈ ℝⁿ : xₙ > 0}
4. **Klein disk**: {x ∈ ℝⁿ : ||x|| < 1}
5. **Hemisphere**

We focus on **Poincaré ball** (intuitive) and **Lorentz** (stable).

---

## Poincaré Ball Model

### Metric

**Riemannian metric**:
```
ds² = 4K² / (1 - ||x||²/K²)² · ||dx||²
```

**Distance between points x, y**:
```
d_P(x, y) = K · arcosh(1 + 2||x - y||² / ((1 - ||x||²/K²)(1 - ||y||²/K²)))
```

**Simplified formula** (numerically stable):
```
d_P(x, y) = 2K · artanh(||(-x) ⊕_K y|| / K)
```

### Möbius Gyrovector Operations

**Möbius Addition** (generalized):
```
x ⊕_K y = ((1 + 2⟨x,y⟩/K² + ||y||²/K²)x + (1 - ||x||²/K²)y) /
          (1 + 2⟨x,y⟩/K² + ||x||²||y||²/K⁴)
```

**Special case** (K = 1):
```
x ⊕ y = ((1 + 2⟨x,y⟩ + ||y||²)x + (1 - ||x||²)y) /
        (1 + 2⟨x,y⟩ + ||x||²||y||²)
```

**Properties**:
- **Identity**: x ⊕ 0 = x
- **Inverse**: x ⊕ (-x ⊕ 0) = 0 where (-x ⊕ 0) = -x/(1 + ||x||²/K²)
- **Non-commutative**: x ⊕ y ≠ y ⊕ x (in general)
- **Non-associative**: (x ⊕ y) ⊕ z ≠ x ⊕ (y ⊕ z)

**Computational Complexity**: O(n) for n-dimensional vectors

### Exponential and Logarithmic Maps

**Exponential Map** (tangent space → manifold):
```
exp_x^K(v) = x ⊕_K (tanh(||v||_x / 2K) / ||v||_x) · v

where ||v||_x = 2K / (1 - ||x||²/K²) · ||v||  (tangent norm)
```

**Logarithmic Map** (manifold → tangent space):
```
log_x^K(y) = 2K / (1 - ||x||²/K²) · artanh(||(-x) ⊕_K y|| / K) ·
             ((-x) ⊕_K y) / ||(-x) ⊕_K y||
```

**Usage**:
- **exp**: Apply Euclidean gradients to hyperbolic points
- **log**: Compute "hyperbolic difference" between points

### Parallel Transport

**Problem**: Moving tangent vectors along geodesics while preserving inner products.

**Formula** (transport v from x to y):
```
P_{x→y}(v) = λ(x, y) · ((I + (γ(y) - 1)ŷŷᵀ) v - γ(y)⟨ŷ, v⟩x)

where:
  ŷ = (-x) ⊕_K y / ||(-x) ⊕_K y||
  λ(x, y) = (1 - ||y||²/K²) / (1 - ||x||²/K²)
  γ(y) = 1 / (1 - ||y||²/K²)
```

---

## Lorentz (Hyperboloid) Model

### Minkowski Space

**Ambient space**: ℝⁿ⁺¹ with **Minkowski inner product**:
```
⟨x, y⟩_L = -x₀y₀ + x₁y₁ + ... + xₙyₙ
```

**Hyperboloid constraint**:
```
ℍⁿ = {x ∈ ℝⁿ⁺¹ : ⟨x, x⟩_L = -K², x₀ > 0}
```

### Distance

**Formula**:
```
d_L(x, y) = K · arcosh(-⟨x, y⟩_L / K²)
```

**Numerically stable variant**:
```
d_L(x, y) = K · ln(-⟨x, y⟩_L / K² + √((-⟨x, y⟩_L / K²)² - 1))
```

### Exponential Map

**Formula**:
```
exp_x^L(v) = cosh(||v|| / K) x + sinh(||v|| / K) · v / ||v||

where ||v|| = √⟨v, v⟩_L  (Minkowski norm)
```

### Lorentz Transformations

**Lorentz Boost** (translation along time-like direction):
```
Boost_v(x) = x + (γ - 1)(x · v̂)v̂ - γv

where:
  v̂ = v / ||v||_L
  γ = cosh(||v||_L / K)
```

**Lorentz Rotation** (rotation in space-like plane):
```
R_θ(x) = x + sin(θ)(e₁ ⊗ e₂ - e₂ ⊗ e₁)x

where e₁, e₂ are orthonormal space-like vectors
```

---

## Isometries and Transformations

### Möbius Transformations (Poincaré Ball)

**General form**:
```
M(x) = (Ax + b) / ⟨c, x⟩ + d

subject to: A ∈ SO(n), ad - ⟨b, c⟩ = 1
```

**Special case - Translation**:
```
T_a(x) = (-a) ⊕ x
```

### Gyrovector Multiplication

**Scalar multiplication**:
```
r ⊗ x = tanh(r · artanh(||x|| / K)) / ||x|| · x

for r ∈ ℝ, x ∈ ℍⁿ
```

**Properties**:
- (r + s) ⊗ x ≠ (r ⊗ x) ⊕ (s ⊗ x)  (non-linear)
- r ⊗ (s ⊗ x) = (rs) ⊗ x  (associative)

---

## Hyperbolic Neural Operations

### Hyperbolic Linear Layer

**Euclidean linear layer**: y = Wx + b

**Hyperbolic equivalent**:
```
y = exp_0(W · log_0(x) + b)
```

**Steps**:
1. Map x from manifold to tangent space at origin: v = log_0(x)
2. Apply Euclidean linear transformation: v' = Wv + b
3. Map back to manifold: y = exp_0(v')

**Learnable parameters**: W ∈ ℝᵐˣⁿ, b ∈ ℝᵐ

### Hyperbolic ReLU

**Problem**: ReLU is defined in tangent space, not on manifold.

**Solution**:
```
ReLU_hyp(x) = exp_0(ReLU(log_0(x)))
```

**Component-wise variant**:
```
ReLU_hyp(x)_i = exp_0,i(max(0, log_0(x)_i))
```

### Hyperbolic Batch Normalization

**Challenge**: Mean and variance are Euclidean concepts.

**Hyperbolic mean** (Fréchet mean):
```
μ = argmin_p Σ_i d(p, x_i)²
```

**Approximation** (geodesic midpoint):
```
μ ≈ exp_0(mean(log_0(x_1), ..., log_0(x_n)))
```

**Normalization**:
```
x_norm = exp_μ((log_μ(x) - μ_tangent) / σ_tangent)
```

---

## Attention Mechanisms in Hyperbolic Space

### Hyperbolic Dot-Product Attention

**Euclidean attention**:
```
Attention(Q, K, V) = softmax(QKᵀ / √d) V
```

**Hyperbolic variant**:
```
Attention_hyp(Q, K, V) = ⊕ (softmax(-d(Q_i, K_j)² / τ) ⊗ V_j)
```

**Components**:
1. **Similarity**: -d(q, k)² (negative squared distance)
2. **Normalization**: softmax with temperature τ
3. **Aggregation**: Möbius weighted sum

**Complexity**: O(n²d) for n tokens, d dimensions (same as Euclidean)

### Hyperbolic Linear Attention (Hypformer)

**Problem**: Quadratic complexity O(n²)

**Solution**: Kernel approximation
```
φ(q)ᵀ φ(k) ≈ d_hyp(q, k)

Linear attention:
Attention_linear(Q, K, V) = (Σ_j φ(K_j)⊗V_j) ⊘ (Σ_j φ(K_j))
```

**Hyperbolic kernel** (proposal):
```
φ_hyp(x) = [cosh(||x||/K), sinh(||x||/K) · x/||x||]

Properties:
  ⟨φ_hyp(x), φ_hyp(y)⟩_L ≈ -cosh(d(x,y)/K)
```

**Complexity**: **O(nd²)** vs O(n²d)

**Speedup**: 10x for n > 10d (verified by Hypformer, KDD 2024)

### Multi-Head Hyperbolic Attention

**Extension**:
```
MultiHead(Q, K, V) = Concat(head₁, ..., headₕ) W^O

where head_i = Attention_hyp(QW_i^Q, KW_i^K, VW_i^V)
```

**Learnable per-head curvature**:
```
head_i operates in space with curvature κ_i
```

**Rationale**: Different heads capture different hierarchical depths.

---

## Curvature Adaptation

### Learnable Curvature

**Parameterization**: K ∈ ℝ⁺ (learned via gradient descent)

**Gradient w.r.t. curvature**:
```
∂L/∂K = ∂L/∂d · ∂d/∂K

where:
  ∂d/∂K = ∂/∂K[K · arcosh(1 + 2||x-y||²/((1-||x||²/K²)(1-||y||²/K²)))]
```

**Numerical trick**: Reparameterize as K = exp(k) to ensure K > 0.

### Coupled Optimization

**Problem**: Naively updating K breaks Riemannian optimizer assumptions.

**Solution** (from "Optimizing Curvature Learning" 2024):
```
1. Compute gradients in current manifold (curvature K_old)
2. Update parameters: θ_new = RiemannianSGD(θ, ∇_θ L, K_old)
3. Update curvature: K_new = K_old - α · ∂L/∂K
4. Rescale parameters to new manifold:
   θ_rescaled = rescale_curvature(θ_new, K_old, K_new)
```

**Rescaling formula** (Poincaré ball):
```
rescale(x, K₁, K₂) = (K₂ / K₁) · x
```

### Multi-Curvature Embeddings

**Approach**: Different dimensions/layers have different curvatures.

**Product space**:
```
ℍ^{n₁}(κ₁) × ℍ^{n₂}(κ₂) × ... × ℍ^{nₖ}(κₖ)
```

**Distance**:
```
d_product((x₁,...,xₖ), (y₁,...,yₖ)) = √(Σ_i w_i² d²(x_i, y_i))

where w_i are learnable weights
```

---

## Numerical Stability

### Poincaré Ball Instabilities

**Problem 1**: Division by zero when ||x|| → 1

**Solution**: Clip to maximum norm
```
x_safe = x / max(1, ||x|| / (1 - ε))

where ε = 1e-5
```

**Problem 2**: Möbius addition overflow

**Solution**: Rewrite using log1p, expm1
```
Instead of: (1 + 2⟨x,y⟩ + ||y||²) / (1 + 2⟨x,y⟩ + ||x||²||y||²)
Use: exp(log1p(2⟨x,y⟩ + ||y||²) - log1p(2⟨x,y⟩ + ||x||²||y||²))
```

### Lorentz Model Stability

**Advantage**: No boundary singularities!

**Constraint enforcement**:
```
After each update, project back to hyperboloid:
  x₀ = √(K² + x₁² + ... + xₙ²)
```

**Geodesic computation** (stable):
```
d_L(x, y) = K · log((-⟨x,y⟩ + √(⟨x,y⟩² - K⁴)) / K²)
```

### Mixed Precision

**Strategy**:
- **FP16** for forward pass (speed)
- **FP32** for gradients (stability)
- **FP64** for curvature updates (critical)

**GeoOpt recommendation**: Use FP32 minimum for hyperbolic operations.

---

## Complexity Analysis

### Space Complexity

**Poincaré Ball**:
- Point: O(n) storage (same as Euclidean)
- No auxiliary structures needed

**Lorentz**:
- Point: O(n+1) storage (extra time dimension)
- Constraint: ⟨x,x⟩_L = -K²

**Curvature**:
- Shared K: O(1) extra parameter
- Per-layer K: O(L) for L layers
- Per-dimension K: O(n) parameters

### Time Complexity

| Operation | Euclidean | Poincaré | Lorentz |
|-----------|-----------|----------|---------|
| **Distance** | O(n) | O(n) | O(n) |
| **Addition** | O(n) | O(n) | O(n) |
| **Exp/Log** | - | O(n) | O(n) |
| **Linear layer** | O(n²) | O(n²) | O(n²) |
| **Attention** | O(n²d) | O(n²d) | O(n²d) |
| **Linear attention** | O(nd²) | O(nd²) | O(nd²) |

**Key Insight**: Asymptotic complexity **same as Euclidean**!

**Constants**: Hyperbolic ops 2-5x slower (more FLOPs per operation)

**SIMD Optimization**: Can recover 8-50x speedup, making hyperbolic **faster** than naive Euclidean.

---

## Proofs of Key Properties

### Theorem 1: Möbius Addition Preserves Poincaré Ball

**Statement**: If x, y ∈ 𝔹ⁿ(K) (Poincaré ball), then x ⊕_K y ∈ 𝔹ⁿ(K).

**Proof**:
```
Let ||x||² / K² = a², ||y||² / K² = b², ⟨x,y⟩ / K² = c
where a, b < 1.

||x ⊕_K y||² / K² = ||(1+2c+b²)x + (1-a²)y||² / (1+2c+a²b²)²
                   ≤ ((1+2c+b²)a + (1-a²)b)² / (1+2c+a²b²)²
                   < 1  (by calculation)
```

### Theorem 2: Exponential Map is Diffeomorphism

**Statement**: exp_x: T_xℍⁿ → ℍⁿ is a diffeomorphism for each x.

**Proof**:
- Inverse given by log_x
- Both are smooth (analytic)
- Jacobian is full rank everywhere
- QED.

### Theorem 3: Capacity Advantage

**Statement**: Embedding n-node tree in ℍ² requires distortion O(log n), while ℝᵏ requires k = Ω(n).

**Proof Sketch**:
- Hyperbolic plane has exponential volume: V(r) ~ exp(r)
- Trees have exponential node count: N(depth d) ~ exp(d)
- Volume growth matches tree growth → O(1) average distortion
- Euclidean plane has polynomial volume: V(r) ~ r²
- Trees cannot fit without stretching → Ω(√n) average distortion

---

## Implementation Checklist

### Poincaré Ball Implementation

- [ ] Möbius addition with curvature K
- [ ] Exponential map with numerical stability
- [ ] Logarithmic map with safe arctanh
- [ ] Distance function with clipping
- [ ] Parallel transport
- [ ] Gradient clipping to prevent boundary

### Lorentz Model Implementation

- [ ] Minkowski inner product
- [ ] Hyperboloid constraint projection
- [ ] Exponential map
- [ ] Distance function
- [ ] Lorentz boost and rotation
- [ ] Conversion to/from Poincaré

### Hyperbolic Attention

- [ ] Hyperbolic query/key/value projections
- [ ] Distance-based similarity
- [ ] Softmax with temperature
- [ ] Möbius weighted aggregation
- [ ] Linear attention kernel approximation

### Learnable Curvature

- [ ] Curvature parameter K with positive constraint
- [ ] Gradient computation w.r.t. K
- [ ] Coupled optimization with rescaling
- [ ] Per-layer or per-head curvature

### SIMD Optimizations

- [ ] Vectorized Möbius addition (AVX2)
- [ ] Batch distance computation
- [ ] Fused exp/log operations
- [ ] Cache-aligned memory layout

---

## References

**Textbooks**:
1. "Riemannian Geometry" - do Carmo
2. "Foundations of Hyperbolic Manifolds" - Ratcliffe

**Papers**:
1. Ganea et al., "Hyperbolic Neural Networks" (NeurIPS 2018)
2. Hypformer (KDD 2024) - Linear attention formulation
3. Fully Hyperbolic NNs (ACL 2022) - Lorentz model analysis

**Software**:
- **GeoOpt**: PyTorch library for Riemannian optimization
- **Hyperbolic Image Embeddings**: Reference implementation

---

## Conclusion

Hyperbolic geometry provides a mathematically rigorous framework for hierarchical neural representations with:
- **Provable capacity**: O(exp(n)) vs O(poly(n))
- **Stable operations**: Lorentz model superior to Poincaré
- **Efficient algorithms**: O(n²d) attention same as Euclidean
- **Learnable curvature**: Adapt to data hierarchy

All operations have **closed-form solutions** and **computable gradients**, making them suitable for modern automatic differentiation frameworks.
