# ADR-004: Spectral Invariants for Representation Analysis

**Status**: Accepted
**Date**: 2024-12-15
**Authors**: RuVector Team
**Supersedes**: None

---

## Context

Neural network representations form high-dimensional vector spaces where geometric and spectral properties encode semantic meaning. Understanding these representations requires mathematical tools that can:

1. **Extract invariant features**: Properties preserved under transformations
2. **Detect representation quality**: Distinguish good embeddings from degenerate ones
3. **Track representation evolution**: Monitor how representations change during training
4. **Compare representations**: Measure similarity between different models

Traditional approaches focus on:
- Cosine similarity (ignores global structure)
- t-SNE/UMAP (non-linear, non-invertible projections)
- Probing classifiers (task-specific, not general)

We need invariants that are mathematically well-defined and computationally tractable.

---

## Decision

We implement **spectral invariants** based on eigenvalue analysis of representation matrices, covariance structures, and graph Laplacians.

### Core Spectral Invariants

#### 1. Eigenvalue Spectrum

For a representation matrix X (n samples x d dimensions):

```rust
/// Compute eigenvalue spectrum of covariance matrix
pub struct EigenvalueSpectrum {
    /// Eigenvalues in descending order
    pub eigenvalues: Vec<f64>,
    /// Cumulative explained variance
    pub cumulative_variance: Vec<f64>,
    /// Effective dimensionality
    pub effective_dim: f64,
}

impl EigenvalueSpectrum {
    pub fn from_covariance(cov: &DMatrix<f64>) -> Result<Self> {
        let eigen = cov.symmetric_eigenvalues();
        let mut eigenvalues: Vec<f64> = eigen.iter().cloned().collect();
        eigenvalues.sort_by(|a, b| b.partial_cmp(a).unwrap());

        let total: f64 = eigenvalues.iter().sum();
        let cumulative_variance: Vec<f64> = eigenvalues.iter()
            .scan(0.0, |acc, &x| {
                *acc += x / total;
                Some(*acc)
            })
            .collect();

        // Effective dimensionality via participation ratio
        let sum_sq: f64 = eigenvalues.iter().map(|x| x * x).sum();
        let effective_dim = (total * total) / sum_sq;

        Ok(Self { eigenvalues, cumulative_variance, effective_dim })
    }
}
```

#### 2. Spectral Gap

The spectral gap measures separation between clusters:

```rust
/// Spectral gap analysis
pub struct SpectralGap {
    /// Gap between first and second eigenvalues
    pub primary_gap: f64,
    /// Normalized gap (invariant to scale)
    pub normalized_gap: f64,
    /// Location of largest gap in spectrum
    pub largest_gap_index: usize,
}

impl SpectralGap {
    pub fn from_eigenvalues(eigenvalues: &[f64]) -> Self {
        let gaps: Vec<f64> = eigenvalues.windows(2)
            .map(|w| w[0] - w[1])
            .collect();

        let largest_gap_index = gaps.iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        let primary_gap = gaps.first().copied().unwrap_or(0.0);
        let normalized_gap = primary_gap / eigenvalues[0].max(1e-10);

        Self { primary_gap, normalized_gap, largest_gap_index }
    }
}
```

#### 3. Condition Number

Measures numerical stability of representations:

```rust
/// Condition number for representation stability
pub fn condition_number(eigenvalues: &[f64]) -> f64 {
    let max_eig = eigenvalues.first().copied().unwrap_or(1.0);
    let min_eig = eigenvalues.last().copied().unwrap_or(1e-10).max(1e-10);
    max_eig / min_eig
}
```

### Graph Laplacian Spectrum

For representation similarity graphs:

```rust
/// Laplacian spectral analysis
pub struct LaplacianSpectrum {
    /// Number of connected components (multiplicity of 0 eigenvalue)
    pub num_components: usize,
    /// Fiedler value (second smallest eigenvalue)
    pub fiedler_value: f64,
    /// Cheeger constant bound
    pub cheeger_bound: (f64, f64),
}

impl LaplacianSpectrum {
    pub fn from_graph(adjacency: &DMatrix<f64>) -> Self {
        // Compute degree matrix
        let degrees = adjacency.row_sum();
        let degree_matrix = DMatrix::from_diagonal(&degrees);

        // Laplacian L = D - A
        let laplacian = &degree_matrix - adjacency;

        // Compute spectrum
        let eigen = laplacian.symmetric_eigenvalues();
        let mut eigenvalues: Vec<f64> = eigen.iter().cloned().collect();
        eigenvalues.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Count zero eigenvalues (connected components)
        let num_components = eigenvalues.iter()
            .filter(|&&e| e.abs() < 1e-10)
            .count();

        let fiedler_value = eigenvalues.get(num_components)
            .copied()
            .unwrap_or(0.0);

        // Cheeger inequality bounds
        let cheeger_lower = fiedler_value / 2.0;
        let cheeger_upper = (2.0 * fiedler_value).sqrt();

        Self {
            num_components,
            fiedler_value,
            cheeger_bound: (cheeger_lower, cheeger_upper),
        }
    }
}
```

### Invariant Fingerprints

Combine spectral invariants into a fingerprint for comparison:

```rust
/// Spectral fingerprint for representation comparison
#[derive(Debug, Clone)]
pub struct SpectralFingerprint {
    /// Top k eigenvalues (normalized)
    pub top_eigenvalues: Vec<f64>,
    /// Effective dimensionality
    pub effective_dim: f64,
    /// Condition number (log scale)
    pub log_condition: f64,
    /// Spectral entropy
    pub spectral_entropy: f64,
}

impl SpectralFingerprint {
    pub fn new(spectrum: &EigenvalueSpectrum, k: usize) -> Self {
        let total: f64 = spectrum.eigenvalues.iter().sum();
        let top_eigenvalues: Vec<f64> = spectrum.eigenvalues.iter()
            .take(k)
            .map(|e| e / total)
            .collect();

        // Spectral entropy
        let probs: Vec<f64> = spectrum.eigenvalues.iter()
            .map(|e| e / total)
            .filter(|&p| p > 1e-10)
            .collect();
        let spectral_entropy: f64 = -probs.iter()
            .map(|p| p * p.ln())
            .sum::<f64>();

        Self {
            top_eigenvalues,
            effective_dim: spectrum.effective_dim,
            log_condition: condition_number(&spectrum.eigenvalues).ln(),
            spectral_entropy,
        }
    }

    /// Compare two fingerprints
    pub fn distance(&self, other: &Self) -> f64 {
        let eigenvalue_dist: f64 = self.top_eigenvalues.iter()
            .zip(other.top_eigenvalues.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();

        let dim_diff = (self.effective_dim - other.effective_dim).abs();
        let cond_diff = (self.log_condition - other.log_condition).abs();
        let entropy_diff = (self.spectral_entropy - other.spectral_entropy).abs();

        // Weighted combination
        eigenvalue_dist + 0.1 * dim_diff + 0.05 * cond_diff + 0.1 * entropy_diff
    }
}
```

---

## Consequences

### Positive

1. **Mathematically rigorous**: Based on linear algebra with well-understood properties
2. **Computationally efficient**: SVD/eigendecomposition is O(d^3) but highly optimized
3. **Invariant to orthogonal transformations**: Eigenvalues don't change under rotation
4. **Interpretable**: Effective dimensionality, spectral gap have clear meanings
5. **Composable**: Can combine multiple invariants into fingerprints

### Negative

1. **Not invariant to non-orthogonal transforms**: Scaling changes condition number
2. **Requires full spectrum**: Approximations lose information
3. **Sensitive to outliers**: Single extreme point can dominate covariance
4. **Memory intensive**: Storing covariance matrices is O(d^2)

### Mitigations

1. **Normalization**: Pre-normalize representations to unit variance
2. **Lanczos iteration**: Compute only top-k eigenvalues for large d
3. **Robust covariance**: Use median-of-means or trimmed estimators
4. **Streaming updates**: Maintain running covariance estimates

---

## Implementation Notes

### Lanczos Algorithm for Large Matrices

```rust
/// Compute top-k eigenvalues using Lanczos iteration
pub fn lanczos_eigenvalues(
    matrix: &DMatrix<f64>,
    k: usize,
    max_iter: usize,
) -> Vec<f64> {
    let n = matrix.nrows();
    let k = k.min(n);

    // Initialize with random vector
    let mut v = DVector::from_fn(n, |_, _| rand::random::<f64>());
    v.normalize_mut();

    let mut alpha = Vec::with_capacity(max_iter);
    let mut beta = Vec::with_capacity(max_iter);
    let mut v_prev = DVector::zeros(n);

    for i in 0..max_iter {
        let w = matrix * &v;
        let a = v.dot(&w);
        alpha.push(a);

        let mut w = w - a * &v - if i > 0 { beta[i-1] * &v_prev } else { DVector::zeros(n) };
        let b = w.norm();

        if b < 1e-10 { break; }
        beta.push(b);

        v_prev = v.clone();
        v = w / b;
    }

    // Build tridiagonal matrix and compute eigenvalues
    tridiagonal_eigenvalues(&alpha, &beta, k)
}
```

---

## Related Decisions

- [ADR-001: Sheaf Cohomology](ADR-001-sheaf-cohomology.md) - Uses spectral gap for coherence
- [ADR-002: Category Theory](ADR-002-category-topos.md) - Spectral invariants as functors
- [ADR-006: Quantum Topology](ADR-006-quantum-topology.md) - Density matrix eigenvalues

---

## References

1. Belkin, M., & Niyogi, P. (2003). "Laplacian Eigenmaps for Dimensionality Reduction." Neural Computation.

2. Von Luxburg, U. (2007). "A Tutorial on Spectral Clustering." Statistics and Computing.

3. Roy, O., & Vetterli, M. (2007). "The Effective Rank: A Measure of Effective Dimensionality." EUSIPCO.

4. Kornblith, S., et al. (2019). "Similarity of Neural Network Representations Revisited." ICML.
