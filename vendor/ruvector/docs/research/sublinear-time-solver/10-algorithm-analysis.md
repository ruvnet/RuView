# Sublinear-Time Solver Algorithm Deep-Dive Analysis

**Agent 10 -- Algorithm Analysis**
**Date**: 2026-02-20
**Scope**: Mathematical algorithms in sublinear-time-solver and their applicability to ruvector

---

## Table of Contents

1. [Mathematical Operations in RuVector](#1-mathematical-operations-in-ruvector)
2. [Sublinear Algorithm Explanations with Complexity Analysis](#2-sublinear-algorithm-explanations-with-complexity-analysis)
3. [Applicability to RuVector Problem Domains](#3-applicability-to-ruvector-problem-domains)
4. [Algorithm Selection Criteria](#4-algorithm-selection-criteria)
5. [Numerical Stability Considerations](#5-numerical-stability-considerations)
6. [Convergence Guarantees](#6-convergence-guarantees)
7. [Error Bounds and Precision Tradeoffs](#7-error-bounds-and-precision-tradeoffs)
8. [Recommended Algorithm Mapping to RuVector Use Cases](#8-recommended-algorithm-mapping-to-ruvector-use-cases)

---

## 1. Mathematical Operations in RuVector

RuVector is a vector database with graph, GNN, attention, and numerical optimization subsystems. The mathematical operations span a wide range, organized by crate.

### 1.1 ruvector-core -- Vector Distance and Indexing

**Source**: `/home/user/ruvector/crates/ruvector-core/src/`

The core crate performs four primary distance computations, all with SIMD acceleration (AVX2/AVX-512/NEON):

| Operation | Formula | Complexity | Files |
|-----------|---------|------------|-------|
| Euclidean (L2) | sqrt(sum((a_i - b_i)^2)) | O(d) | `distance.rs`, `simd_intrinsics.rs` |
| Cosine | 1 - (a . b) / (norm(a) * norm(b)) | O(d) | `distance.rs`, `simd_intrinsics.rs` |
| Dot Product | -sum(a_i * b_i) | O(d) | `distance.rs`, `simd_intrinsics.rs` |
| Manhattan (L1) | sum(abs(a_i - b_i)) | O(d) | `distance.rs`, `simd_intrinsics.rs` |

**HNSW indexing** (`index/hnsw.rs`): Uses the hnsw_rs library for Hierarchical Navigable Small World graph construction. Insert is O(M * log(n)) amortized, search is O(log(n) * ef_search) where M is the connectivity parameter and ef_search controls quality-speed tradeoff.

**Quantization** (`quantization.rs`): Four tiers of lossy compression:
- Scalar (u8): 4x compression, per-element uniform quantization
- Int4: 8x compression, nibble-packed quantization
- Product Quantization: 8-16x compression, k-means codebook per subspace
- Binary: 32x compression, sign-bit encoding with Hamming distance

**Batch operations** (`simd_intrinsics.rs`): Cache-tiled batch distance with TILE_SIZE=16, INT8 quantized dot products, and 4x loop-unrolled accumulators.

### 1.2 ruvector-math -- Advanced Mathematical Foundations

**Source**: `/home/user/ruvector/crates/ruvector-math/src/`

This crate contains theoretical mathematical machinery:

**Spectral Methods** (`spectral/`):
- **Chebyshev polynomial expansion**: Approximates filter functions h(lambda) on graph Laplacian eigenvalues using O(K) Chebyshev terms. The graph filter applies as h(L)x via three-term recurrence: T_{k+1}(L)x = 2L*T_k(L)x - T_{k-1}(L)x, costing O(K * nnz(L)) per signal.
- **Spectral clustering**: Power iteration with deflation to find Fiedler vector and k smallest eigenvectors of the graph Laplacian, followed by k-means.
- **Graph wavelets**: Multi-scale heat diffusion filters exp(-t*L) at varying time scales.
- **Scaled Laplacian**: Constructs L_sym = I - D^{-1/2}AD^{-1/2} from sparse adjacency.

**Optimal Transport** (`optimal_transport/`):
- **Sinkhorn algorithm**: Log-stabilized O(n^2 * iterations) entropic-regularized optimal transport. Solves min_{gamma in Pi(a,b)} <gamma, C> - eps*H(gamma).
- **Sliced Wasserstein**: O(P * n*log(n)) 1D projections for approximate Wasserstein distance.
- **Gromov-Wasserstein**: Structural distance between graphs of different sizes.

**Information Geometry** (`information_geometry/`):
- **Fisher Information Matrix**: Empirical FIM from gradient outer products, F = E[g*g^T].
- **Natural Gradient**: theta_{t+1} = theta_t - eta * F^{-1} * grad_L. Requires solving or approximating the linear system F*x = grad.
- **K-FAC**: Kronecker-factored approximate curvature for efficient Fisher inversion.

**Tensor Networks** (`tensor_networks/`):
- **Tensor Train (TT)**: Represents d-dimensional tensor as chain of 3D cores, storage O(d * n * r^2) instead of O(n^d).
- **Tucker decomposition**: Core tensor plus factor matrices per mode.
- **CP decomposition**: Rank-R canonical polyadic decomposition.

**Topological Data Analysis** (`homology/`):
- **Persistent homology**: Vietoris-Rips filtration for topological drift detection.
- **Bottleneck and Wasserstein distances** on persistence diagrams.

**Tropical Algebra** (`tropical/`):
- Max-plus semiring for shortest path analysis and piecewise linear neural network analysis.

**Polynomial Optimization** (`optimization/`):
- Sum-of-squares (SOS) certificates for provable bounds on attention policies.
- Semidefinite programming relaxations.

### 1.3 ruvector-gnn -- Graph Neural Network Inference

**Source**: `/home/user/ruvector/crates/ruvector-gnn/src/`

- **GNN layer** (`layer.rs`): Multi-head attention with Q/K/V linear projections (Xavier init), layer normalization, gated recurrent updates. Forward pass: O(n * d^2) for linear transforms, O(n^2 * d / h) for attention.
- **Training** (`training.rs`): SGD with momentum and Adam optimizer. Standard gradient-based parameter updates.
- **EWC** (`ewc.rs`): Elastic Weight Consolidation using Fisher information diagonal. Penalty: L_EWC = lambda/2 * sum(F_i * (theta_i - theta*_i)^2). Prevents catastrophic forgetting during continual learning.
- **Tensor operations** (`tensor.rs`): ndarray-based matrix multiplication, element-wise operations.

### 1.4 ruvector-graph -- Graph Database and Query Engine

**Source**: `/home/user/ruvector/crates/ruvector-graph/src/`

- **GraphDB** (`graph.rs`): Concurrent DashMap-backed graph with label, property, and adjacency indices.
- **Cypher query engine** (`cypher/`): Lexer, parser, AST, semantic analysis, query optimizer, and parallel executor pipeline.
- **Graph traversal**: BFS/DFS, neighbor iteration, path finding via adjacency index.
- **Hybrid vector-graph search** (`hybrid/`): Combines vector similarity (k-NN) with graph structure for semantic search and RAG integration.

### 1.5 ruvector-attention -- 40+ Attention Mechanisms

**Source**: `/home/user/ruvector/crates/ruvector-attention/src/`

- **Flash Attention** (`sparse/flash.rs`): Block-tiled O(n * block_size) memory attention with online softmax. Avoids materializing full n x n attention matrix.
- **Linear Attention** (`sparse/linear.rs`): O(n * d) kernel-based attention approximation.
- **PDE Attention** (`pde_attention/`): Graph Laplacian-based diffusion attention. Constructs L from key similarities via Gaussian kernel, applies as diffusion process.
- **Hyperbolic Attention** (`hyperbolic/`): Poincare ball model attention using Mobius addition and hyperbolic distance.
- **Optimal Transport Attention** (`transport/`): Sinkhorn-based attention with Sliced Wasserstein distances.
- **Sheaf Attention** (`sheaf/`): Cellular sheaf theory-based attention with restriction maps and early exit.
- **Information Geometry Attention** (`info_geometry/`): Fisher metric-based attention.
- **Mixture of Experts** (`moe/`): Gated routing with expert selection.
- **Topology-aware Attention** (`topology/`): Gated attention with topological coherence.

### 1.6 ruvector-mincut -- Min-Cut and Graph Algorithms

**Source**: `/home/user/ruvector/crates/ruvector-mincut/src/`

- **Subpolynomial dynamic min-cut** (`subpolynomial/`): Implements the December 2024 breakthrough (arXiv:2512.13105). Update time O(n^{o(1)}), query time O(1). Uses multi-level hierarchy, expander decomposition, deterministic LocalKCut, and witness trees.
- **Approximate min-cut** (`algorithm/approximate.rs`): Spectral sparsification with edge sampling achieving (1+eps)-approximate cuts. Preprocessing O(m * log^2(n) / eps^2), query O(n * polylog(n) / eps^2).
- **Spectral sparsification** (`sparsify/`): Benczur-Karger randomized sparsification and Nagamochi-Ibaraki deterministic sparsification. Produces O(n * log(n) / eps^2) edges preserving all cuts within (1 +/- eps).
- **Spiking Neural Networks** (`snn/`): Event-driven neuromorphic computing with attractor dynamics.

### 1.7 ruvector-sparse-inference -- Sparse Neural Inference

**Source**: `/home/user/ruvector/crates/ruvector-sparse-inference/src/`

- **Sparse FFN** (`sparse/ffn.rs`): Two-layer feed-forward with neuron subset selection. Only computes active neurons, with transposed W2 for contiguous memory access. Achieves 15-25% speedup in accumulation.
- **Low-rank activation predictor** (`predictor/lowrank.rs`): P*Q factorization to predict active neurons. Compress input: z = P*x (rank r), score neurons: s = Q*z.

### 1.8 ruvector-hyperbolic-hnsw -- Hyperbolic Space Indexing

**Source**: `/home/user/ruvector/crates/ruvector-hyperbolic-hnsw/src/`

- **Poincare ball operations** (`poincare.rs`): Mobius addition, exponential/logarithmic maps, geodesic distance d(x,y) = (2/sqrt(c)) * arctanh(sqrt(c) * norm(mobius_add(-x, y))). Numerically stabilized with eps = 1e-5.
- **Hyperbolic HNSW** (`hnsw.rs`): HNSW index using Poincare distance instead of Euclidean.

### 1.9 ruqu-algorithms -- Quantum Algorithms

**Source**: `/home/user/ruvector/crates/ruqu-algorithms/src/`

- **Grover's search**: Quadratic speedup for unstructured search O(sqrt(N)).
- **QAOA**: Quantum Approximate Optimization Algorithm for combinatorial problems.
- **VQE**: Variational Quantum Eigensolver.
- **Surface code**: Quantum error correction.

---

## 2. Sublinear Algorithm Explanations with Complexity Analysis

### 2.1 Neumann Series -- O(k * nnz)

**Mathematical Foundation**: For a diagonally dominant matrix A = D - B where D is diagonal and the spectral radius rho(D^{-1}B) < 1, the inverse can be approximated via:

```
A^{-1} = D^{-1} * sum_{i=0}^{k} (D^{-1}B)^i
```

This is the matrix geometric series truncated at k terms.

**Complexity**: Each iteration costs O(nnz(A)) for a sparse matrix-vector multiply, so k iterations cost O(k * nnz(A)). This is strictly sublinear in n^2 when the matrix is sparse with nnz << n^2.

**Convergence**: The series converges geometrically with rate rho(D^{-1}B). After k terms, the error is bounded by:

```
||A^{-1} - S_k|| <= ||D^{-1}|| * rho^{k+1} / (1 - rho)
```

where S_k is the k-term partial sum.

**Key Property**: No matrix factorization required. Each iteration is a simple sparse matvec. Works in-place with O(n) auxiliary storage.

### 2.2 Forward Push -- O(1/eps)

**Mathematical Foundation**: Computes an approximate personalized PageRank (PPR) vector pi_s for a single source vertex s. The algorithm maintains a residual vector r and an estimate vector p:

```
Initialize: r[s] = 1, p = 0
While exists v with |r[v]| / deg(v) > eps:
    p[v] += alpha * r[v]
    For each neighbor u of v:
        r[u] += (1 - alpha) * r[v] / (2 * deg(v))
    r[v] = (1 - alpha) * r[v] / 2
```

**Complexity**: O(1/eps) total push operations. Each push distributes residual mass to neighbors. The total work is bounded because the L1 norm of the residual decreases monotonically.

**Key Property**: Output-sensitive -- the running time depends on the desired precision, not the graph size. For a single query vertex, this is dramatically faster than solving the full system.

### 2.3 Backward Push -- O(1/eps)

**Mathematical Foundation**: Reverse direction of Forward Push. Instead of propagating from source to all nodes, it propagates importance backward from a target vertex. For a target t, it approximates the column of the PPR matrix:

```
pi[s, t] for all s
```

by pushing mass backward along edges.

**Complexity**: O(1/eps) total operations, same as Forward Push but targeting different queries.

**Key Property**: Dual to Forward Push. Useful when the query is "which sources have high relevance to target t?" rather than "which targets are relevant to source s?"

### 2.4 Hybrid Random Walk -- O(sqrt(n)/eps)

**Mathematical Foundation**: Combines Forward/Backward Push with Monte Carlo random walks to achieve better complexity than either approach alone.

**Algorithm**:
1. Run Forward Push from source s with threshold eps_f, obtaining estimate p_f and residual r_f.
2. Run Backward Push from target t with threshold eps_b, obtaining estimate p_b and residual r_b.
3. Sample O(sqrt(n) / eps) random walks from vertices with nonzero residual.
4. Combine: pi_approx = p_f + p_b + MC_correction.

**Complexity**: O(sqrt(n) / eps) by balancing the push thresholds with walk count:
- Set eps_f = eps_b = sqrt(eps * sqrt(n))
- Push cost: O(sqrt(n/eps))
- Random walk cost: O(sqrt(n/eps))
- Total: O(sqrt(n) / eps)

**Key Property**: Breaks the 1/eps barrier of pure push methods and the n barrier of pure random walk methods by hybridizing.

### 2.5 TRUE -- O(log n)

**Mathematical Foundation**: Combines three techniques for near-logarithmic time Laplacian solving:

1. **Johnson-Lindenstrauss dimension reduction**: Project high-dimensional vectors to O(log(n) / eps^2) dimensions while preserving distances within (1 +/- eps). Projection matrix: random Gaussian or sparse random.

2. **Adaptive Neumann series**: Instead of fixed k iterations, adaptively choose expansion depth based on spectral gap. Uses local graph structure to estimate convergence rate and terminate early.

3. **Spectral sparsification**: Reduce graph to O(n * log(n) / eps^2) edges while preserving all cut values within (1 +/- eps). This makes each Neumann iteration cheaper by reducing nnz.

**Combined Complexity**: O(log n) amortized per solve via:
- Sparsification preprocessing: O(m * log(n) / eps^2)
- JL reduction: O(n * log^2(n) / eps^2)
- Adaptive Neumann: O(log(n)) iterations on sparsified graph
- Per-query: O(log(n) * n * log(n) / eps^2) total, but amortized over n queries yields O(log n).

**Key Property**: The fastest known approach for approximate Laplacian solving. The logarithmic complexity makes it suitable for very large graphs.

### 2.6 Conjugate Gradient (CG) -- OptimizedConjugateGradientSolver

**Mathematical Foundation**: The classic Krylov subspace method for solving Ax = b where A is symmetric positive definite. CG minimizes the A-norm of the error at each step:

```
||x - x_k||_A = min over Krylov space K_k(A, r_0) of ||x - y||_A
```

**Algorithm per iteration**:
```
r_k = b - A*x_k        (residual)
beta_k = r_k^T r_k / r_{k-1}^T r_{k-1}
p_k = r_k + beta_k * p_{k-1}  (search direction)
alpha_k = r_k^T r_k / (p_k^T A p_k)
x_{k+1} = x_k + alpha_k * p_k
r_{k+1} = r_k - alpha_k * A * p_k
```

**Complexity**: Each iteration costs O(nnz(A)) for the matvec, O(n) for dot products and vector updates. Total iterations to reach eps-relative residual: O(sqrt(kappa(A)) * log(1/eps)) where kappa is the condition number.

**With preconditioning**: Using a preconditioner M ~ A^{-1}, the effective condition number becomes kappa(M*A), which can dramatically reduce iteration count. Diagonal preconditioning is O(n), incomplete Cholesky is O(nnz).

**Key Property**: CG is the gold standard for sparse SPD systems. It is deterministic, has well-understood convergence, and its memory footprint is O(n).

### 2.7 BMSSP -- Balanced Multilevel Sparse Solver

**Mathematical Foundation**: A multigrid-inspired approach that constructs a hierarchy of coarsened graphs:

```
Level 0: Original graph G_0 (n vertices, m edges)
Level 1: Coarsened G_1 (n/r vertices, m' edges)
Level 2: Coarsened G_2 (n/r^2 vertices, m'' edges)
...
Level L: Coarsened G_L (O(1) vertices)
```

**Algorithm**:
1. **Coarsening**: Group vertices into supernodes using matching or aggregation. Merge edges between supernodes. Repeat until graph is small.
2. **Solve at coarsest level**: Direct solve or dense solver on the small system.
3. **Prolongation**: Interpolate coarse solution back to finer levels.
4. **Smoothing**: Apply a few iterations of local solver (Jacobi, Gauss-Seidel) at each level to reduce high-frequency error.

**Complexity**: With r-fold coarsening and O(1) smoothing steps per level:
- Levels: L = O(log_r(n))
- Work per level: O(nnz_level)
- Total: O(nnz * log(n) / (r-1)) ideally O(nnz) if coarsening is balanced.

**Key Property**: Near-linear time for well-structured problems. Effectiveness depends heavily on the quality of coarsening (i.e., whether the coarsened graph preserves spectral properties).

---

## 3. Applicability to RuVector Problem Domains

### 3.1 Graph Laplacian Systems in ruvector-math and ruvector-attention

**Problem**: Multiple ruvector subsystems solve or apply graph Laplacian-related operations:

| Subsystem | Operation | Current Approach | Bottleneck |
|-----------|-----------|-----------------|------------|
| Spectral filtering (`ruvector-math/spectral/graph_filter.rs`) | h(L)x | Chebyshev recurrence O(K*nnz) | K scales with filter sharpness |
| PDE attention (`ruvector-attention/pde_attention/laplacian.rs`) | Diffusion on L | Dense L construction O(n^2) | L construction quadratic |
| Spectral clustering (`ruvector-math/spectral/clustering.rs`) | Smallest eigenvectors of L | Power iteration O(k * nnz * iters) | Slow convergence for clustered graphs |
| Normalized cut (`ruvector-math/spectral/clustering.rs`) | L^{-1} implicitly | Iterative | Condition number dependent |

**Sublinear Applicability**:
- **Neumann Series**: Directly applicable when the graph Laplacian is diagonally dominant (always true for L = D - A with positive weights). Can replace power iteration in spectral clustering's eigenvector computation by solving (shift*I - L)x = b iteratively.
- **CG**: Drop-in replacement for any Lx = b system. The PDE attention diffusion step can be reformulated as Laplacian solve. Preconditioning with diagonal of L gives sqrt(d_max/d_min) condition number.
- **TRUE**: For large-scale spectral filtering (n > 100k), the JL + sparsification + adaptive Neumann pipeline would reduce Chebyshev filtering cost from O(K * nnz) to O(K * n * polylog(n)).
- **BMSSP**: Natural fit for hierarchical spectral clustering. The coarsening hierarchy mirrors the spectral clustering hierarchy, and the multilevel solve gives near-linear time eigenvector approximation.

### 3.2 Personalized Search and Graph Traversal in ruvector-graph

**Problem**: The hybrid vector-graph search in `ruvector-graph/src/hybrid/` combines vector similarity with graph neighborhood expansion. Cypher queries traverse the graph via adjacency.

**Sublinear Applicability**:
- **Forward Push**: Directly applicable to personalized graph search. Given a query vector match to node s, Forward Push computes approximate PPR from s in O(1/eps) time regardless of graph size. This replaces BFS/DFS-based expansion.
- **Backward Push**: When the query is "find all vectors relevant to a given target," Backward Push provides O(1/eps) reverse reachability.
- **Hybrid Random Walk**: For two-hop relevance queries (source s to target t), the hybrid method achieves O(sqrt(n)/eps), superior to both push methods for pairwise queries.

### 3.3 GNN Message Passing in ruvector-gnn

**Problem**: GNN layers (`ruvector-gnn/src/layer.rs`) perform message passing: for each node, aggregate features from neighbors weighted by attention scores. With multi-head attention, cost is O(n^2 * d / h) per layer.

**Sublinear Applicability**:
- **Forward Push**: Can be used to compute approximate attention-weighted aggregation. Instead of computing attention over all n nodes, Forward Push propagates attention mass from query node, touching only O(1/eps) nodes.
- **Neumann Series**: When GNN uses multiple layers, the effective receptive field is A^L where L is depth. This is a matrix power, and for sparse A, the Neumann approach computes a truncated version efficiently.

### 3.4 Optimal Transport in ruvector-math and ruvector-attention

**Problem**: The Sinkhorn solver (`ruvector-math/optimal_transport/sinkhorn.rs`) has O(n^2 * iterations) complexity due to the dense cost matrix.

**Sublinear Applicability**:
- **TRUE (JL dimension reduction)**: Reduce the embedding dimension before computing cost matrices. A d-dimensional point cloud can be projected to O(log(n)/eps^2) dimensions, reducing cost matrix computation from O(n^2 * d) to O(n^2 * log(n)/eps^2).
- **Forward Push on transport graph**: Sparse transport problems (where the cost matrix has structure) can be reformulated as graph problems. Forward Push on the bipartite transport graph can approximate optimal plans.

### 3.5 Min-Cut and Sparsification in ruvector-mincut

**Problem**: The subpolynomial min-cut algorithm already uses spectral sparsification. The sparsifier (`ruvector-mincut/src/sparsify/`) uses Benczur-Karger randomized sampling.

**Sublinear Applicability**:
- **TRUE (spectral sparsification component)**: The TRUE algorithm's sparsification step is precisely what ruvector-mincut already does. The connection is bidirectional: TRUE uses sparsification to speed up Laplacian solving, and sparsification uses effective resistance computation (which requires Laplacian solving).
- **CG for effective resistance**: Computing effective resistances for sparsification requires solving O(log n) Laplacian systems. CG with diagonal preconditioning gives O(sqrt(kappa) * log(1/eps) * nnz) per system.
- **BMSSP**: The multilevel hierarchy of BMSSP mirrors the multi-level decomposition in the subpolynomial min-cut algorithm. Sharing infrastructure between the two would reduce code complexity and improve cache utilization.

### 3.6 Quantized Vector Operations in ruvector-core

**Problem**: Product quantization (`ruvector-core/src/quantization.rs`) uses k-means clustering on subspaces. The codebook search is O(n * K) per subspace where K is codebook size.

**Sublinear Applicability**:
- **Forward Push for codebook search**: If codebook entries are connected in a similarity graph, Forward Push can find nearest codebook entries in O(1/eps) instead of linear scan.
- **JL for high-dimensional PQ**: When embedding dimensions are very high (d > 1024), JL projection to O(log(K)/eps^2) dimensions before codebook search preserves nearest-neighbor relationships.

### 3.7 Hyperbolic HNSW in ruvector-hyperbolic-hnsw

**Problem**: HNSW in hyperbolic space uses Poincare distance, which involves expensive arctanh and norm computations.

**Sublinear Applicability**:
- **TRUE (JL component)**: JL projections can be adapted to hyperbolic space via tangent space projections. Project from the tangent space at the origin using JL, then map back. This reduces dimension for neighbor candidate evaluation.
- **Forward Push on HNSW graph**: The HNSW graph itself is a navigable small world. Forward Push can be used for approximate k-NN search on this graph, potentially faster than standard greedy search for high-recall requirements.

---

## 4. Algorithm Selection Criteria

### 4.1 Decision Matrix

| Criterion | Neumann | Forward Push | Backward Push | Hybrid RW | TRUE | CG | BMSSP |
|-----------|---------|-------------|---------------|-----------|------|-----|-------|
| **Input type** | Sparse SPD matrix | Graph + source vertex | Graph + target vertex | Graph + (s,t) pair | Sparse Laplacian | Sparse SPD matrix | Sparse Laplacian |
| **Output** | Approximate inverse * vector | PPR vector from s | PPR column to t | Pairwise PPR(s,t) | Approximate Laplacian solve | Exact (to tolerance) solve | Approximate solve |
| **Best n range** | 1K - 1M | Any | Any | > 10K | > 100K | 1K - 10M | > 50K |
| **Sparsity requirement** | nnz << n^2 | Natural graphs | Natural graphs | Natural graphs | Any sparse | nnz << n^2 | Hierarchical structure |
| **Preprocessing** | None | None | None | None | O(m log n / eps^2) | Preconditioner construction | O(m log n) coarsening |
| **Deterministic?** | Yes | Yes | Yes | No (Monte Carlo) | No (JL, sparsification) | Yes | Partially |
| **Parallelizable?** | Matvec parallelism | Push parallelism limited | Push parallelism limited | Walk parallelism good | High parallelism | Matvec parallelism | Level parallelism |
| **WASM compatible?** | Yes (pure arithmetic) | Yes (graph traversal) | Yes (graph traversal) | Yes (random walks) | Needs careful RNG | Yes (pure arithmetic) | Yes (multi-level) |

### 4.2 Selection Rules

**Rule 1 -- Single-source graph exploration**: Use Forward Push. Rationale: O(1/eps) independent of graph size, deterministic, no preprocessing.

**Rule 2 -- Laplacian system solve (well-conditioned)**: Use CG with diagonal preconditioning. Rationale: deterministic convergence guarantees, minimal memory, well-understood error bounds.

**Rule 3 -- Laplacian system solve (ill-conditioned, large scale)**: Use BMSSP or TRUE. Rationale: near-linear time independent of condition number.

**Rule 4 -- Batch Laplacian solves (same graph, multiple RHS)**: Use TRUE with precomputed sparsifier. Rationale: amortize preprocessing over many solves, each costing O(log n).

**Rule 5 -- Spectral graph filtering**: Use Neumann Series when the filter is a rational function of L, CG when it requires inversion, Chebyshev (existing) when it is a general polynomial.

**Rule 6 -- Pairwise relevance between two specific nodes**: Use Hybrid Random Walk. Rationale: O(sqrt(n)/eps) is optimal for this query type.

**Rule 7 -- Dimension reduction before distance computation**: Use TRUE's JL component. Rationale: O(log(n)/eps^2) target dimensions preserve distances.

---

## 5. Numerical Stability Considerations

### 5.1 Neumann Series

**Risk**: Divergence when spectral radius rho(D^{-1}B) >= 1. For graph Laplacians L = D - A with positive weights, rho(D^{-1}A) = max eigenvalue of random walk matrix, which is exactly 1 for connected graphs.

**Mitigation**: Never apply Neumann directly to L. Instead, apply to (L + delta*I) for some regularization delta > 0. This shifts the spectrum away from zero. The ruvector Sinkhorn solver already uses similar log-domain stabilization.

**Practical check**: Compute max degree ratio max(A_ij / D_ii) across rows. If > 0.99, increase regularization.

### 5.2 Forward/Backward Push

**Risk**: Floating-point residual mass can accumulate rounding errors over many push operations.

**Mitigation**: Use compensated summation (Kahan) for residual updates. The ruvector codebase already uses `f64` for math operations and `f32` for storage, which provides sufficient precision for push operations with eps > 1e-8.

**Practical check**: Monitor total mass invariant: sum(p) + sum(r) should remain constant (equal to 1 for PPR). Warn if drift exceeds eps/10.

### 5.3 Hybrid Random Walk

**Risk**: Monte Carlo variance can be large for small sample counts. Rare events (walks reaching isolated nodes) introduce high-variance estimates.

**Mitigation**: Stratified sampling -- group walks by starting residual mass, sample proportionally. Variance reduction via control variates using the push estimates.

**Practical check**: Compute empirical variance of walk estimates. If coefficient of variation > 1, double sample count.

### 5.4 TRUE

**Risk**: JL projection introduces multiplicative (1 +/- eps) distortion. Compounding JL error with Neumann truncation error and sparsification error gives total error ~ 3*eps (assuming independence).

**Mitigation**: Use eps/3 for each component to achieve overall eps accuracy. The ruvector math utilities already include `EPS = 1e-15` and `LOG_MIN` constants for stable log-domain operations.

**Practical check**: Verify sparsifier quality by sampling random cuts and checking (1-eps) <= w(S,V\S)_sparse / w(S,V\S)_original <= (1+eps).

### 5.5 Conjugate Gradient

**Risk**: Loss of orthogonality in Krylov basis due to floating-point arithmetic. This can cause stagnation or divergence in ill-conditioned systems.

**Mitigation**: Reorthogonalization every O(sqrt(n)) steps. Alternatively, use the three-term recurrence variant which is more stable. For graph Laplacians, the condition number is lambda_max/lambda_2, which can be estimated cheaply via power iteration.

**Practical check**: Monitor ||r_k|| / ||b||. If it increases for > 5 consecutive iterations, trigger reorthogonalization or switch to MINRES.

### 5.6 BMSSP

**Risk**: Poor coarsening can produce pathological hierarchies where the coarsened graph does not preserve spectral properties. This manifests as divergence of the multigrid cycle.

**Mitigation**: Use algebraic multigrid (AMG) coarsening with strength-of-connection threshold. Verify coarsening quality by checking that the smoothing property holds: ||A * e_smooth|| <= sigma * ||e_smooth||_A for sigma < 1.

**Practical check**: If V-cycle residual reduction factor exceeds 0.5, switch to W-cycle or refine coarsening.

### 5.7 Quantization Interaction

**Special concern for ruvector**: Distance computations use quantized vectors (u8, int4, binary). Sublinear algorithms operating on these representations must account for quantization noise.

**Recommendation**: Run sublinear algorithms on full-precision (f32) representations. Use quantized representations only for the final distance computation in the search phase. The `ScalarQuantized::reconstruct()` and `Int4Quantized::reconstruct()` methods in `ruvector-core/src/quantization.rs` can dequantize when needed for solver inputs.

---

## 6. Convergence Guarantees

### 6.1 Neumann Series

**Guarantee**: Converges if and only if rho(D^{-1}B) < 1. The error after k terms is:

```
||error|| <= ||D^{-1}|| * rho^{k+1} / (1 - rho)
```

For ruvector's regularized Laplacian (L + delta*I) with delta > 0:
- rho = lambda_max(L) / (lambda_max(L) + delta) < 1 always.
- Convergence rate: log(1/eps) / log(1/rho) iterations.
- For delta = 0.01 and lambda_max ~ 2*d_max, approximately k = 200 * log(1/eps) iterations for typical graphs.

### 6.2 Forward Push

**Guarantee**: Terminates with L1 residual error <= eps * sum(degree). The output satisfies:

```
||pi_exact - pi_approx||_1 <= eps * vol(G)
```

where vol(G) = sum of all degrees. This is an *absolute* error bound, not relative.

**For ruvector**: When using Forward Push for approximate attention aggregation, the absolute error bound translates to bounded attention weight error per node.

### 6.3 Backward Push

**Guarantee**: Same as Forward Push but for the transpose operation. Terminates with:

```
||pi_exact[:,t] - pi_approx[:,t]||_1 <= eps * vol(G)
```

### 6.4 Hybrid Random Walk

**Guarantee**: With C * sqrt(n) / eps walks, the estimate satisfies:

```
P(|pi_estimate(s,t) - pi_exact(s,t)| > eps) <= delta
```

where C depends on delta. For delta = 0.01 (99% confidence), C ~ 10.

**For ruvector**: This probabilistic guarantee means approximately 1% of pairwise relevance scores may exceed the eps error bound. For search applications, this is acceptable since top-k results are robust to small perturbations.

### 6.5 TRUE

**Guarantee**: With probability >= 1 - 1/n, the output x satisfies:

```
||x - L^{-1}b||_L <= eps * ||L^{-1}b||_L
```

This is relative error in the Laplacian norm (energy norm). The 1/n failure probability comes from the JL projection and sparsification.

### 6.6 Conjugate Gradient

**Guarantee**: Deterministic convergence. After k iterations:

```
||x_k - x*||_A <= 2 * ((sqrt(kappa) - 1) / (sqrt(kappa) + 1))^k * ||x_0 - x*||_A
```

For graph Laplacians with condition number kappa = lambda_max / lambda_2:
- kappa for expander graphs: O(1) => O(log(1/eps)) iterations
- kappa for path graphs: O(n^2) => O(n * log(1/eps)) iterations
- kappa for typical social networks: O(n^{0.3-0.5}) => O(n^{0.15-0.25} * log(1/eps)) iterations

### 6.7 BMSSP

**Guarantee**: Under the smoothing assumption, the V-cycle convergence factor is:

```
||e_{k+1}||_A / ||e_k||_A <= sigma < 1
```

where sigma depends on the coarsening quality and smoother. For algebraic multigrid with good coarsening:
- sigma ~ 0.1-0.3 for most graph Laplacians
- Total iterations to eps: O(log(1/eps)) V-cycles
- Each V-cycle: O(nnz) work
- Overall: O(nnz * log(1/eps)), near-linear in input size

---

## 7. Error Bounds and Precision Tradeoffs

### 7.1 Error Budget Decomposition

For a ruvector pipeline that combines multiple sublinear algorithms, the total error accumulates:

```
eps_total <= eps_quantization + eps_jl + eps_sparsify + eps_solver + eps_push
```

Recommended budget allocation for eps_total = 0.1:

| Component | Budget | Rationale |
|-----------|--------|-----------|
| Quantization (ruvector-core) | 0.03 | Scalar u8 quantization error ~ (range/255) |
| JL projection (TRUE) | 0.02 | Need high fidelity for distances |
| Sparsification (TRUE/mincut) | 0.02 | Cut preservation critical for mincut |
| Solver (CG/Neumann/BMSSP) | 0.02 | Residual tolerance |
| Push approximation | 0.01 | Tight for search quality |

### 7.2 Precision-Performance Tradeoff Curves

**Neumann Series**:
```
Time = c1 * k * nnz
Error = c2 * rho^k
=> Time = c1 * nnz * log(1/error) / log(1/rho)
```
Doubling precision requires constant additive work.

**Forward Push**:
```
Time = c3 / eps
Error = eps * vol(G)
=> Time = c3 * vol(G) / error
```
Halving error doubles time. Linear tradeoff.

**CG**:
```
Time = c4 * sqrt(kappa) * log(1/eps) * nnz
Error = eps * ||x*||_A
```
Doubling precision costs only O(log(2)) more iterations. Highly efficient precision refinement.

**TRUE**:
```
Time = c5 * log(n) * n * polylog(n) / eps^2
Error = eps * ||x*||_L
```
Quadratic dependence on 1/eps. Expensive to push below eps = 0.01.

### 7.3 Precision Requirements by RuVector Use Case

| Use Case | Required eps | Recommended Algorithm | Justification |
|----------|-------------|----------------------|---------------|
| k-NN vector search | 0.1 | Forward Push + quantized distances | Top-k robust to 10% distance error |
| Spectral clustering | 0.05 | CG with diagonal preconditioner | Eigenvector sign determines partition |
| GNN attention weights | 0.01 | CG or Neumann | Attention softmax amplifies small errors |
| Optimal transport plan | 0.001 | CG (high precision) | Transport marginal constraints are strict |
| Min-cut value | 0.01 | Sparsification + exact on sparsifier | Cut value used for structural decisions |
| Natural gradient (FIM inverse) | 0.1 | Diagonal approximation (existing) or CG | FIM is ill-conditioned; diagonal is safer |

---

## 8. Recommended Algorithm Mapping to RuVector Use Cases

### 8.1 Primary Recommendations

#### Recommendation 1: Forward Push for Hybrid Vector-Graph Search

**Target**: `ruvector-graph/src/hybrid/semantic_search.rs`, `ruvector-graph/src/hybrid/rag_integration.rs`

**Current approach**: Vector k-NN followed by BFS/DFS graph expansion.

**Proposed change**: After vector k-NN identifies seed nodes, use Forward Push from each seed to compute approximate PPR. Return nodes with highest PPR score instead of raw BFS neighbors.

**Expected improvement**:
- Search quality: PPR naturally balances proximity and connectivity (vs. BFS which is purely topological).
- Performance: O(k_seeds / eps) total work, independent of graph size. Current BFS is O(k_seeds * avg_degree^depth).
- Memory: O(nonzero PPR entries) vs O(BFS frontier size).

**Integration point**: Add `ForwardPushSearcher` alongside existing `SemanticSearch` in the hybrid module.

#### Recommendation 2: CG for PDE Attention Laplacian Solves

**Target**: `ruvector-attention/src/pde_attention/diffusion.rs`, `ruvector-attention/src/pde_attention/laplacian.rs`

**Current approach**: Dense Laplacian construction O(n^2) followed by dense diffusion O(n^2).

**Proposed change**: Build sparse k-NN Laplacian (already supported via `from_keys_knn`). Solve the diffusion equation exp(-t*L)*v using CG on (I + t*L)*u = v (first-order approximation) or Chebyshev expansion on the sparse L.

**Expected improvement**:
- Complexity: O(k * n * iterations) instead of O(n^2). For k=16 neighbors and 20 CG iterations, this is 320n vs n^2, a 3x speedup at n=1000 and 300x at n=100000.
- Memory: O(k*n) sparse Laplacian vs O(n^2) dense.

**Integration point**: The `GraphLaplacian::from_keys_knn` method already exists. Add a CG solver method to `GraphLaplacian`.

#### Recommendation 3: Neumann Series for Spectral Graph Filtering

**Target**: `ruvector-math/src/spectral/graph_filter.rs`

**Current approach**: Chebyshev polynomial expansion with three-term recurrence, O(K * nnz) per filtered signal.

**Proposed change**: For rational filters (e.g., (I + alpha*L)^{-1} for low-pass), replace Chebyshev with Neumann series on (I + alpha*L) which is guaranteed to converge (spectral radius < 1 for alpha > 0).

**Expected improvement**:
- Fewer iterations for smooth filters: Neumann converges in O(1/alpha) iterations for heat-like filters, vs K=20-50 Chebyshev terms.
- Simpler implementation: No Chebyshev coefficient computation needed.
- Better composability: Neumann can be nested (filter of filter) without recomputing coefficients.

**Integration point**: Add `NeumannFilter` alongside existing `SpectralFilter` in `ruvector-math/src/spectral/`.

#### Recommendation 4: TRUE for Large-Scale Spectral Clustering

**Target**: `ruvector-math/src/spectral/clustering.rs`

**Current approach**: Power iteration with deflation, O(k * nnz * iters) for k eigenvectors.

**Proposed change**: For n > 100K:
1. Sparsify the graph to O(n * log(n) / eps^2) edges.
2. Apply JL projection to reduce vector dimension.
3. Use adaptive Neumann to solve (shift*I - L_sparse)*x = b for eigenvector estimation.

**Expected improvement**:
- Time: O(n * polylog(n)) instead of O(n * nnz * iters). For sparse graphs (nnz ~ 10n), improvement is polylog factor. For dense similarity graphs (nnz ~ n^2), improvement is n / polylog(n).
- Quality: Sparsification preserves cut structure, so cluster quality is maintained within (1+eps).

**Integration point**: Add `TrueClusteringSolver` as backend for `SpectralClustering` when n exceeds threshold.

#### Recommendation 5: BMSSP for Multi-Scale Graph Processing

**Target**: `ruvector-math/src/spectral/wavelets.rs`, `ruvector-mincut/src/jtree/hierarchy.rs`

**Current approach**: Multi-scale heat diffusion filters at fixed scales. Min-cut hierarchy via expander decomposition.

**Proposed change**: Build a BMSSP hierarchy that serves both purposes:
1. The coarsening hierarchy provides natural scale decomposition for wavelets.
2. The same hierarchy supports multilevel Laplacian solving for any graph operation.
3. Min-cut hierarchy levels can reuse the BMSSP levels.

**Expected improvement**:
- Shared infrastructure: One hierarchy construction serves wavelets, clustering, and min-cut.
- Cache efficiency: Hierarchical processing has better data locality than flat operations.
- Near-linear total time: O(nnz * log(n)) for all multi-scale operations combined.

**Integration point**: Implement `MultilevelHierarchy` in a shared module, referenced by spectral, mincut, and attention subsystems.

#### Recommendation 6: Hybrid Random Walk for Pairwise Node Relevance

**Target**: `ruvector-graph/src/executor/operators.rs` (for Cypher path queries)

**Current approach**: BFS/Dijkstra for shortest path, followed by relevance scoring.

**Proposed change**: For "relevance between node A and node B" queries, use Hybrid Random Walk to compute approximate PPR(A, B) in O(sqrt(n)/eps) time.

**Expected improvement**:
- Avoids full shortest-path computation for relevance scoring.
- Natural handling of multi-path relevance (not just shortest path).
- O(sqrt(n)/eps) vs O(n + m) for Dijkstra.

**Integration point**: Add `RelevanceEstimator` to the Cypher executor as a new operator.

#### Recommendation 7: JL Projection for High-Dimensional Distance Computation

**Target**: `ruvector-core/src/distance.rs`, `ruvector-core/src/simd_intrinsics.rs`

**Current approach**: Full-dimensional distance computation with SIMD acceleration.

**Proposed change**: For d > 512, optionally apply JL projection to target dimension k = O(log(n)/eps^2) before batch distance computation.

**Expected improvement**:
- Dimension reduction from d to k = ceil(24 * log(n) / eps^2). For n=1M and eps=0.1, k = 24 * 20 / 0.01 = 48000. This is worse than d=768 for typical embeddings.
- **However**, for d > 2048 (e.g., protein embeddings, genomic features), JL reduces to ~5000 dimensions, saving 60%+ compute.
- SIMD-friendly: JL projection is a dense matvec, fully vectorizable.

**Integration point**: Add `JLProjector` to `ruvector-core` with pre-computed projection matrices.

### 8.2 Implementation Priority

| Priority | Recommendation | Effort | Impact | Risk |
|----------|---------------|--------|--------|------|
| **P0** | Forward Push for hybrid search | Medium | High -- core search quality | Low -- well-understood algorithm |
| **P0** | CG for PDE attention | Low | High -- O(n^2) -> O(kn) | Low -- standard solver |
| **P1** | Neumann for spectral filtering | Low | Medium -- simpler filters | Low -- drop-in replacement |
| **P1** | Hybrid RW for pairwise relevance | Medium | Medium -- new query type | Medium -- Monte Carlo variance |
| **P2** | TRUE for large-scale clustering | High | High for large n | Medium -- complex implementation |
| **P2** | BMSSP for multi-scale processing | High | High -- shared infrastructure | Medium -- coarsening quality |
| **P3** | JL for high-dimensional distances | Low | Situational | Low -- optional projection |

### 8.3 Integration Architecture

```
ruvector-core (distances, quantization)
    |-- JL projector (TRUE component)
    |-- Batch distance with optional projection
    |
ruvector-math (spectral, transport, optimization)
    |-- NeumannFilter (new)
    |-- CG solver (new)
    |-- MultilevelHierarchy (new, shared by BMSSP)
    |-- Existing: Chebyshev, Sinkhorn, SpectralClustering
    |
ruvector-graph (graph DB, Cypher, hybrid search)
    |-- ForwardPushSearcher (new)
    |-- HybridRandomWalkEstimator (new)
    |-- BackwardPushRanker (new)
    |-- Existing: BFS/DFS, adjacency index
    |
ruvector-attention (40+ attention mechanisms)
    |-- CG-based PDE attention (new)
    |-- Sparse Laplacian attention (enhanced)
    |-- Existing: Flash, linear, hyperbolic, sheaf, ...
    |
ruvector-mincut (min-cut, sparsification, SNN)
    |-- Shared sparsifier with TRUE (link to ruvector-math)
    |-- CG for effective resistance computation
    |-- Existing: subpolynomial min-cut, spectral sparsifier
    |
ruvector-gnn (GNN layers, training, EWC)
    |-- Forward Push message passing (new)
    |-- Existing: multi-head attention, layer norm, Adam/SGD
```

### 8.4 Summary Table

| RuVector Subsystem | Current Bottleneck | Best Sublinear Algorithm | Complexity Improvement | Key File |
|---|---|---|---|---|
| Hybrid graph search | BFS O(k * d^L) | Forward Push O(1/eps) | Independent of graph size | `hybrid/semantic_search.rs` |
| PDE attention | Dense Laplacian O(n^2) | CG on sparse L O(k*n*iters) | n / (k*iters) speedup | `pde_attention/diffusion.rs` |
| Spectral filtering | Chebyshev O(K*nnz) | Neumann O(k*nnz), k < K for smooth filters | 2-5x for heat/low-pass | `spectral/graph_filter.rs` |
| Spectral clustering | Power iteration O(k*nnz*iters) | TRUE O(n*polylog(n)) | nnz/polylog(n) for dense graphs | `spectral/clustering.rs` |
| Pairwise relevance | BFS/Dijkstra O(n+m) | Hybrid RW O(sqrt(n)/eps) | sqrt(n) speedup | `executor/operators.rs` |
| Multi-scale processing | Independent per scale | BMSSP shared hierarchy O(nnz*log(n)) | Shared amortization | `spectral/wavelets.rs` |
| Effective resistance | Per-edge solve O(m*nnz) | CG batch solve O(log(n)*nnz*sqrt(kappa)) | m/log(n) speedup | `sparsify/mod.rs` |
| High-dim distances | O(n*d) per query | JL projection O(n*k), k << d | d/k speedup when d >> 1024 | `distance.rs` |

---

## Implementation Notes

The following documents the actual implementation approach for each algorithm in the `ruvector-solver` crate, noting where the implementation diverges from or refines the theoretical descriptions in Section 2.

### Neumann Series -- Jacobi-Preconditioned

The implementation uses **Jacobi preconditioning** with a D^{-1} splitting rather than the raw (I - A) expansion described in the literature. The matrix A is decomposed as A = D - B where D is the diagonal. The iteration computes x_{k+1} = D^{-1} * B * x_k + D^{-1} * b, which is equivalent to the Neumann series on D^{-1}A but with guaranteed convergence for all diagonally dominant systems. The diagonal inverse is precomputed once at solver setup. This approach is strictly superior to the raw Neumann series for graph Laplacian systems where diagonal dominance is inherent.

### Conjugate Gradient -- Hestenes-Stiefel with Residual Monitoring

The CG implementation follows the standard **Hestenes-Stiefel** formulation with explicit residual monitoring. The residual norm ||r_k|| / ||b|| is tracked at every iteration and compared against the convergence tolerance. If the residual increases for 5 consecutive iterations (indicating loss of orthogonality or numerical instability), the solver logs a warning and terminates with the best solution found. The implementation uses the fused kernel optimization to compute the matvec and residual update in a single pass.

### Forward Push -- Queue-Based with Epsilon Threshold

The Forward Push algorithm uses a **queue-based** implementation with an epsilon threshold for push activation. Vertices with |r[v]| / deg(v) > epsilon are maintained in a priority queue (max-heap by residual magnitude). This avoids scanning all vertices for push candidates and ensures O(1/epsilon) total work. The queue is implemented with a `BinaryHeap` and lazy deletion for efficiency.

### TRUE -- JL Projection, Sparsification, Neumann Solve

The TRUE solver implements the three-stage pipeline: (1) **Johnson-Lindenstrauss projection** to reduce dimension to O(log(n) / eps^2) using sparse random projection matrices, (2) **spectral sparsification** to reduce the graph to O(n * log(n) / eps^2) edges while preserving cut structure, and (3) **adaptive Neumann solve** on the sparsified system using the Jacobi-preconditioned Neumann iteration. The JL projection uses a sparse Rademacher matrix for efficiency.

### BMSSP -- V-Cycle Multigrid with Jacobi Smoothing

The BMSSP implementation uses a **V-cycle multigrid** scheme with Jacobi smoothing at each level. Coarsening is performed via heavy-edge matching, which groups vertices connected by the strongest edges into supernodes. The coarsest level (< 64 vertices) is solved directly. Prolongation uses piecewise constant interpolation from supernode to constituent vertices. Two Jacobi smoothing sweeps are applied at each level (pre- and post-smoothing) to reduce high-frequency error components.

### Router -- Characteristic-Based Selection

The algorithm router selects the optimal solver based on **matrix characteristics**: system size (n), density (nnz / n^2), symmetry (SPD detection), and diagonal dominance ratio (min(D_ii / sum(|A_ij|, j != i))). The selection rules are:

| Characteristic | Selected Solver |
|---|---|
| n < 64 | Direct (dense) solve |
| Diagonally dominant, sparse | Neumann (Jacobi-preconditioned) |
| SPD, well-conditioned (kappa < 1000) | Conjugate Gradient |
| SPD, ill-conditioned, n > 50K | BMSSP (multigrid) |
| Graph Laplacian, n > 100K, batch RHS | TRUE (JL + sparsify + Neumann) |
| Single-source graph query | Forward Push |

---

## Appendix A: Notation Reference

| Symbol | Meaning |
|--------|---------|
| n | Number of vertices / vectors |
| m | Number of edges |
| d | Vector dimensionality |
| nnz | Number of nonzero entries in sparse matrix |
| L | Graph Laplacian L = D - A |
| D | Degree matrix |
| A | Adjacency matrix |
| kappa | Condition number lambda_max / lambda_min |
| eps | Approximation parameter |
| rho | Spectral radius |
| PPR | Personalized PageRank |
| alpha | PPR teleportation probability |
| K | Chebyshev polynomial degree or codebook size |
| k | Number of clusters / neighbors / eigenvectors |
| r | Tensor Train rank / low-rank dimension |

## Appendix B: Source Files Analyzed

Key files examined during this analysis:

- `/home/user/ruvector/crates/ruvector-core/src/distance.rs` -- Distance metric implementations
- `/home/user/ruvector/crates/ruvector-core/src/simd_intrinsics.rs` -- SIMD-optimized operations (1605 lines)
- `/home/user/ruvector/crates/ruvector-core/src/quantization.rs` -- Tiered quantization (935 lines)
- `/home/user/ruvector/crates/ruvector-core/src/index/hnsw.rs` -- HNSW index wrapper
- `/home/user/ruvector/crates/ruvector-math/src/lib.rs` -- Math crate module structure
- `/home/user/ruvector/crates/ruvector-math/src/spectral/clustering.rs` -- Spectral clustering
- `/home/user/ruvector/crates/ruvector-math/src/spectral/graph_filter.rs` -- Chebyshev graph filtering
- `/home/user/ruvector/crates/ruvector-math/src/optimal_transport/sinkhorn.rs` -- Log-stabilized Sinkhorn
- `/home/user/ruvector/crates/ruvector-math/src/information_geometry/natural_gradient.rs` -- Natural gradient descent
- `/home/user/ruvector/crates/ruvector-math/src/tensor_networks/tensor_train.rs` -- TT decomposition
- `/home/user/ruvector/crates/ruvector-gnn/src/layer.rs` -- GNN layers with multi-head attention
- `/home/user/ruvector/crates/ruvector-gnn/src/training.rs` -- SGD and Adam optimizers
- `/home/user/ruvector/crates/ruvector-gnn/src/ewc.rs` -- Elastic Weight Consolidation
- `/home/user/ruvector/crates/ruvector-graph/src/graph.rs` -- Concurrent graph database
- `/home/user/ruvector/crates/ruvector-attention/src/sparse/flash.rs` -- Flash attention
- `/home/user/ruvector/crates/ruvector-attention/src/pde_attention/laplacian.rs` -- Graph Laplacian for attention
- `/home/user/ruvector/crates/ruvector-mincut/src/algorithm/approximate.rs` -- Approximate min-cut
- `/home/user/ruvector/crates/ruvector-mincut/src/sparsify/mod.rs` -- Spectral sparsification
- `/home/user/ruvector/crates/ruvector-mincut/src/subpolynomial/mod.rs` -- Subpolynomial dynamic min-cut
- `/home/user/ruvector/crates/ruvector-sparse-inference/src/sparse/ffn.rs` -- Sparse FFN
- `/home/user/ruvector/crates/ruvector-sparse-inference/src/predictor/lowrank.rs` -- Low-rank activation predictor
- `/home/user/ruvector/crates/ruvector-hyperbolic-hnsw/src/poincare.rs` -- Poincare ball operations
- `/home/user/ruvector/crates/ruqu-algorithms/src/grover.rs` -- Grover's quantum search
- `/home/user/ruvector/examples/subpolynomial-time/src/fusion/optimizer.rs` -- Fusion optimizer
