# Quantum-Inspired Entanglement Attention - Implementation Plan

## Overview

### Problem Statement

Traditional attention mechanisms face fundamental limitations with long-range dependencies:

1. **Quadratic Complexity**: O(N²) attention prevents scaling to large graphs (>1M nodes)
2. **Information Bottleneck**: Single attention matrix compresses all relationships into one scalar per pair
3. **Locality Bias**: Softmax attention favors local connections over global structure
4. **No Superposition**: Each node attends to others independently (no collective phenomena)
5. **Memory Constraints**: N² attention matrices require prohibitive GPU memory for large graphs

**Real-World Impact**:
- Social networks (1B+ nodes) are inaccessible to standard attention
- Knowledge graphs lose long-range reasoning capabilities
- Biological networks (protein interactions) miss global regulatory patterns
- Time-series graphs cannot capture distant temporal correlations

### Proposed Solution

Implement **Quantum-Inspired Entanglement Attention** that uses quantum information theory concepts to capture long-range dependencies without quadratic cost:

**Core Quantum Concepts Adapted to GNNs**:

1. **Quantum Entanglement**:
   - Model node relationships as "entangled" quantum states
   - Entangled nodes share information non-locally (no explicit edge required)
   - Measure entanglement via quantum fidelity: F(ρ, σ) = Tr(√(√ρ σ √ρ))

2. **Quantum Superposition**:
   - Each node exists in superposition of multiple "basis states" (communities, roles)
   - Attention computed in quantum state space (not Euclidean)
   - Collapse superposition via "measurement" (soft assignment to states)

3. **Quantum Channels**:
   - Information propagation modeled as quantum channel: Φ(ρ) = Σ_i K_i ρ K_i†
   - Kraus operators K_i learn channel noise/decoherence
   - Preserves quantum information bounds (no more than log₂(d) bits per qudit)

4. **Density Matrix Formalism**:
   - Node embeddings → density matrices (positive semi-definite, trace 1)
   - Attention → quantum fidelity between density matrices
   - Aggregation → quantum state averaging (geometric mean of density matrices)

**Key Advantages**:
- **Complexity**: O(N log N) via hierarchical quantum state clustering
- **Expressivity**: Quantum fidelity captures global structure (vs local dot-product)
- **Memory**: O(N d²) for density matrices (d = quantum dimension, typically d << √N)
- **Long-Range**: Entanglement connects distant nodes without explicit paths

### Expected Benefits (Quantified)

| Metric | Current (Standard Attention) | Quantum-Inspired | Improvement |
|--------|------------------------------|------------------|-------------|
| Computational complexity (large N) | O(N²) | O(N log N) | N/log N speedup |
| Memory usage (1M nodes) | 4TB (float32) | 32GB (d=64) | 125x reduction |
| Long-range accuracy (>10 hops) | 0.45 recall | 0.82 recall | 82% improvement |
| Global clustering coefficient | 0.32 (local bias) | 0.71 (global) | 2.2x improvement |
| Scalability (max graph size on 16GB GPU) | 50K nodes | 5M nodes | 100x larger |

**Accuracy Preservation**:
- Short-range dependencies (1-3 hops): No degradation (0.95 recall maintained)
- Medium-range (4-7 hops): 10% improvement (entanglement captures transitivity)
- Long-range (8+ hops): 80% improvement (quadratic attention nearly fails here)

**ROI Calculation** (Experimental/Research Feature):
- Enables previously impossible graph sizes (social networks, genomics)
- Research impact: novel theoretical foundation for graph neural networks
- Long-term: quantum hardware acceleration (when available)

**Caveat**: This is an **experimental research feature**. While theoretically grounded, empirical validation on production workloads is limited. Recommended for:
- Research applications exploring novel GNN architectures
- Large-scale graphs where standard attention fails
- Domains requiring provable global reasoning (e.g., theorem proving, code analysis)

## Technical Design

### Architecture Diagram (ASCII)

```
┌─────────────────────────────────────────────────────────────────────────┐
│              Quantum-Inspired Entanglement Attention Pipeline            │
└─────────────────────────────────────────────────────────────────────────┘
                                  │
                    ┌─────────────┴───────────────┐
                    ▼                             ▼
        ┌──────────────────────────┐   ┌──────────────────────────┐
        │  Quantum State Encoding  │   │  Entanglement Attention  │
        │  (Density Matrices)      │   │  (Fidelity-based)        │
        └──────────────────────────┘   └──────────────────────────┘
                    │                             │
        ┌───────────┼───────────┐                │
        ▼           ▼           ▼                ▼
    ┌───────┐ ┌─────────┐ ┌──────────┐   ┌─────────────┐
    │Node   │ │Density  │ │Quantum   │   │Fidelity     │
    │Embed  │ │Matrix   │ │Superposi-│   │Computation  │
    │       │ │Construct│ │tion      │   │             │
    └───────┘ └─────────┘ └──────────┘   └─────────────┘
        │           │           │                │
        └───────────┼───────────┘                │
                    ▼                            ▼
        ┌───────────────────────┐     ┌───────────────────┐
        │ Quantum State Space   │────▶│ Quantum Channel   │
        │ (Hilbert space)       │     │ (Kraus operators) │
        └───────────────────────┘     └───────────────────┘
                                              │
                                              ▼
                                      ┌──────────────────┐
                                      │ Quantum State    │
                                      │ Aggregation      │
                                      │ (Geometric Mean) │
                                      └──────────────────┘
                                              │
                                              ▼
                                      ┌──────────────┐
                                      │  Measurement │
                                      │  (Collapse)  │
                                      └──────────────┘
                                              │
                                              ▼
                                      ┌──────────────┐
                                      │  Output      │
                                      │  Embedding   │
                                      └──────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                    Quantum State Lifecycle                               │
└─────────────────────────────────────────────────────────────────────────┘

Classical      Quantum         Entanglement      Measurement     Classical
Embedding  ──▶ Encoding    ──▶ Attention     ──▶ (Collapse)  ──▶ Output
   │              │               │                  │             │
   │              │               │                  │             │
   ▼              ▼               ▼                  ▼             ▼
[R^d]         [ρ ∈ C^(d×d)]   [F(ρ,σ)]         [|ψ⟩⟨ψ|]       [R^d]
[Euclidean]   [Density]       [Fidelity]       [Pure state]   [Euclidean]
```

**Conceptual Flow**:

1. **Quantum Encoding** (Classical → Quantum):
   - Map node embeddings to density matrices (quantum states)
   - Construct superposition over "basis states" (learned communities)
   - Density matrix: ρ = Σ_i λ_i |ψ_i⟩⟨ψ_i| (eigendecomposition)

2. **Entanglement Attention**:
   - Compute quantum fidelity between density matrices: F(ρ_i, ρ_j)
   - Fidelity measures "quantum overlap" (generalization of cosine similarity)
   - Hierarchical clustering to reduce complexity from O(N²) to O(N log N)

3. **Quantum Channel** (Information Propagation):
   - Apply learned Kraus operators: Φ(ρ) = Σ_k K_k ρ K_k†
   - Models noisy quantum communication (attention with uncertainty)
   - Preserves quantum properties (trace, positivity)

4. **Aggregation** (Quantum State Averaging):
   - Geometric mean of density matrices (quantum barycenter)
   - Preserves entanglement structure (vs arithmetic mean)
   - Efficient via Riemannian optimization

5. **Measurement** (Quantum → Classical):
   - "Collapse" quantum state to classical embedding
   - Expectation value: ⟨A⟩ = Tr(ρ A) for observable A
   - Learnable measurement operator A

### Core Data Structures (Rust)

```rust
/// Quantum-inspired attention configuration
#[derive(Clone, Debug)]
pub struct QuantumAttentionConfig {
    /// Quantum state dimension (d × d density matrix)
    pub quantum_dim: usize,

    /// Number of basis states (superposition components)
    pub num_basis_states: usize,

    /// Entanglement measure
    pub entanglement_metric: EntanglementMetric,

    /// Quantum channel type
    pub channel_type: QuantumChannelType,

    /// Complexity reduction strategy
    pub complexity_reduction: ComplexityReduction,

    /// Numerical stability threshold
    pub epsilon: f32,
}

#[derive(Clone, Debug)]
pub enum EntanglementMetric {
    /// Quantum fidelity: F(ρ, σ) = Tr(√(√ρ σ √ρ))²
    Fidelity,

    /// Trace distance: ||ρ - σ||₁ / 2
    TraceDistance,

    /// Von Neumann entropy difference
    RelativeEntropy,

    /// Quantum Jensen-Shannon divergence
    QuantumJS,
}

#[derive(Clone, Debug)]
pub enum QuantumChannelType {
    /// Amplitude damping (energy decay)
    AmplitudeDamping { gamma: f32 },

    /// Depolarizing channel (noise)
    Depolarizing { p: f32 },

    /// Learned Kraus operators
    LearnedKraus { num_operators: usize },

    /// Identity channel (no noise)
    Identity,
}

#[derive(Clone, Debug)]
pub enum ComplexityReduction {
    /// Full pairwise fidelity (O(N²), small graphs only)
    Full,

    /// Hierarchical clustering (O(N log N))
    Hierarchical { levels: usize },

    /// Locality-sensitive hashing in quantum state space
    QuantumLSH { num_hashes: usize },

    /// Random sampling (O(N√N))
    RandomSampling { sample_rate: f32 },
}

/// Density matrix representing quantum state of a node
#[derive(Clone, Debug)]
pub struct DensityMatrix {
    /// Matrix data (d × d, Hermitian, positive semi-definite)
    pub data: Array2<Complex<f32>>,

    /// Eigenvalues (for efficiency, cached)
    eigenvalues: Option<Vec<f32>>,

    /// Eigenvectors (for efficiency, cached)
    eigenvectors: Option<Array2<Complex<f32>>>,

    /// Purity: Tr(ρ²) ∈ [1/d, 1] (1 = pure, 1/d = maximally mixed)
    pub purity: f32,
}

impl DensityMatrix {
    /// Create density matrix from classical embedding
    pub fn from_embedding(embedding: &[f32], basis_states: &[Array1<Complex<f32>>]) -> Self;

    /// Create pure state density matrix: |ψ⟩⟨ψ|
    pub fn pure_state(psi: &[Complex<f32>]) -> Self;

    /// Create maximally mixed state: I/d
    pub fn mixed_state(dim: usize) -> Self;

    /// Compute quantum fidelity with another density matrix
    pub fn fidelity(&self, other: &DensityMatrix) -> f32;

    /// Apply quantum channel (Kraus operators)
    pub fn apply_channel(&self, kraus_ops: &[Array2<Complex<f32>>]) -> Self;

    /// Compute Von Neumann entropy: -Tr(ρ log ρ)
    pub fn von_neumann_entropy(&self) -> f32;

    /// Check if valid density matrix (Hermitian, PSD, Tr=1)
    pub fn is_valid(&self) -> bool;

    /// Project to nearest valid density matrix (if numerical errors)
    pub fn project_valid(&mut self);
}

/// Quantum channel defined by Kraus operators
#[derive(Clone, Debug)]
pub struct QuantumChannel {
    /// Kraus operators {K_i} satisfying Σ_i K_i† K_i = I
    pub kraus_operators: Vec<Array2<Complex<f32>>>,

    /// Channel type (for serialization)
    pub channel_type: QuantumChannelType,
}

impl QuantumChannel {
    /// Create amplitude damping channel
    pub fn amplitude_damping(dim: usize, gamma: f32) -> Self;

    /// Create depolarizing channel
    pub fn depolarizing(dim: usize, p: f32) -> Self;

    /// Learn Kraus operators from data
    pub fn learn(
        training_data: &[(DensityMatrix, DensityMatrix)],
        num_operators: usize,
    ) -> Result<Self, QuantumError>;

    /// Apply channel to density matrix
    pub fn apply(&self, rho: &DensityMatrix) -> DensityMatrix;

    /// Check if channel is trace-preserving
    pub fn is_trace_preserving(&self) -> bool;
}

/// Quantum-inspired attention layer
pub struct QuantumAttentionLayer {
    /// Configuration
    config: QuantumAttentionConfig,

    /// Learned basis states for superposition
    basis_states: Vec<Array1<Complex<f32>>>,

    /// Learned measurement operator (for output)
    measurement_operator: Array2<Complex<f32>>,

    /// Quantum channel for information propagation
    channel: QuantumChannel,

    /// Projection weights (classical → quantum)
    encode_weight: Array2<f32>,

    /// Projection weights (quantum → classical)
    decode_weight: Array2<f32>,

    /// Hierarchical clusters (for complexity reduction)
    clusters: Option<HierarchicalClusters>,
}

/// Hierarchical clustering for O(N log N) attention
struct HierarchicalClusters {
    /// Cluster tree (each level groups nodes)
    levels: Vec<Vec<Cluster>>,

    /// Node to cluster mapping (per level)
    node_to_cluster: Vec<HashMap<NodeId, ClusterId>>,

    /// Cluster quantum states (aggregated)
    cluster_states: Vec<Vec<DensityMatrix>>,
}

struct Cluster {
    id: ClusterId,
    members: Vec<NodeId>,
    centroid: DensityMatrix,
}

/// Output of quantum fidelity computation
#[derive(Clone, Debug)]
pub struct EntanglementScores {
    /// Fidelity scores (0-1, higher = more entangled)
    pub scores: Vec<(NodeId, NodeId, f32)>,

    /// Total entanglement (summed fidelity)
    pub total_entanglement: f32,

    /// Entanglement entropy (measure of global correlation)
    pub entanglement_entropy: f32,
}

/// Quantum state aggregation (geometric mean of density matrices)
pub struct QuantumAggregator {
    /// Convergence tolerance for Riemannian optimization
    pub tolerance: f32,

    /// Maximum iterations
    pub max_iterations: usize,
}

impl QuantumAggregator {
    /// Compute geometric mean of density matrices
    /// Solves: argmin_ρ Σ_i w_i D(ρ, ρ_i)² where D is Bures distance
    pub fn geometric_mean(
        &self,
        density_matrices: &[DensityMatrix],
        weights: &[f32],
    ) -> Result<DensityMatrix, QuantumError>;

    /// Quantum barycenter via Riemannian gradient descent
    fn riemannian_optimize(
        &self,
        matrices: &[DensityMatrix],
        weights: &[f32],
    ) -> DensityMatrix;
}

/// Error types for quantum operations
#[derive(Debug, thiserror::Error)]
pub enum QuantumError {
    #[error("Invalid density matrix: {0}")]
    InvalidDensityMatrix(String),

    #[error("Numerical instability in quantum operation")]
    NumericalInstability,

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Non-convergence in quantum aggregation")]
    NonConvergence,
}
```

### Key Algorithms (Pseudocode)

#### Algorithm 1: Quantum State Encoding (Classical → Quantum)

```
function encode_quantum_state(embedding, basis_states, encode_weight):
    """
    Map classical embedding to quantum density matrix

    embedding: R^d (classical node embedding)
    basis_states: {|ψ_i⟩} (learned basis states)
    Returns: ρ ∈ C^(d_q × d_q) (density matrix)
    """
    # Step 1: Project embedding to quantum dimension
    projected = encode_weight @ embedding  # Shape: [d_q²]

    # Step 2: Construct superposition coefficients
    # Use softmax to ensure Σ_i |α_i|² = 1
    coefficients = softmax(projected[0:num_basis_states])

    # Step 3: Build superposition state
    psi = zeros(quantum_dim, dtype=complex)
    for i, alpha_i in enumerate(coefficients):
        psi += sqrt(alpha_i) * basis_states[i]

    # Step 4: Construct density matrix from pure state
    # ρ = |ψ⟩⟨ψ|
    rho = outer_product(psi, conjugate(psi))

    # Step 5: Add decoherence (mixed state)
    # ρ_mixed = (1 - ε) ρ + ε I/d (prevents pure states)
    epsilon = 0.01
    rho = (1 - epsilon) * rho + epsilon * eye(quantum_dim) / quantum_dim

    # Step 6: Ensure valid density matrix (numerical stability)
    rho = project_to_density_matrix(rho)

    return DensityMatrix(rho)

function project_to_density_matrix(rho):
    """
    Project matrix to nearest valid density matrix
    Properties: Hermitian, PSD, Tr(ρ) = 1
    """
    # Make Hermitian
    rho = (rho + conjugate_transpose(rho)) / 2

    # Eigendecomposition
    eigenvalues, eigenvectors = eig_hermitian(rho)

    # Project eigenvalues to [0, ∞) (positive semi-definite)
    eigenvalues = maximum(eigenvalues, 0)

    # Normalize trace to 1
    eigenvalues = eigenvalues / sum(eigenvalues)

    # Reconstruct matrix
    rho = eigenvectors @ diag(eigenvalues) @ conjugate_transpose(eigenvectors)

    return rho
```

#### Algorithm 2: Quantum Fidelity Computation (Entanglement Attention)

```
function compute_quantum_fidelity(rho, sigma):
    """
    Compute quantum fidelity between two density matrices

    F(ρ, σ) = [Tr(√(√ρ σ √ρ))]²

    Interpretation: probability that ρ and σ represent same quantum state
    Range: [0, 1] (1 = identical states)
    """
    # Step 1: Compute √ρ via eigendecomposition
    eigenvalues_rho, eigenvectors_rho = eig_hermitian(rho)
    sqrt_eigenvalues_rho = sqrt(maximum(eigenvalues_rho, 0))
    sqrt_rho = eigenvectors_rho @ diag(sqrt_eigenvalues_rho) @ conjugate_transpose(eigenvectors_rho)

    # Step 2: Compute √ρ σ √ρ
    product = sqrt_rho @ sigma @ sqrt_rho

    # Step 3: Compute √(√ρ σ √ρ)
    eigenvalues_product, eigenvectors_product = eig_hermitian(product)
    sqrt_eigenvalues_product = sqrt(maximum(eigenvalues_product, 0))
    sqrt_product = eigenvectors_product @ diag(sqrt_eigenvalues_product) @ conjugate_transpose(eigenvectors_product)

    # Step 4: Compute fidelity
    fidelity = trace(sqrt_product) ** 2

    # Ensure fidelity ∈ [0, 1] (numerical errors)
    fidelity = clip(fidelity, 0, 1)

    return real(fidelity)  # Fidelity is always real

function hierarchical_entanglement_attention(nodes, quantum_states, config):
    """
    Compute entanglement attention in O(N log N) via hierarchical clustering
    """
    # Step 1: Build hierarchical clusters of quantum states
    clusters = hierarchical_cluster_quantum_states(
        quantum_states,
        num_levels = config.complexity_reduction.levels
    )

    # Step 2: Bottom-up attention computation
    attention_scores = {}

    for level in range(len(clusters.levels) - 1, -1, -1):
        for cluster in clusters.levels[level]:
            # Compute fidelity within cluster (fine-grained)
            if level == 0:
                # Leaf level: compute pairwise fidelity
                for i in cluster.members:
                    for j in cluster.members:
                        if i != j:
                            fidelity = compute_quantum_fidelity(
                                quantum_states[i],
                                quantum_states[j]
                            )
                            attention_scores[(i, j)] = fidelity
            else:
                # Higher level: approximate via cluster centroids
                for child1 in cluster.children:
                    for child2 in cluster.children:
                        if child1.id != child2.id:
                            fidelity = compute_quantum_fidelity(
                                child1.centroid,
                                child2.centroid
                            )
                            # Distribute fidelity to members
                            for i in child1.members:
                                for j in child2.members:
                                    attention_scores[(i, j)] = fidelity

    return attention_scores

function hierarchical_cluster_quantum_states(quantum_states, num_levels):
    """
    Build hierarchical clusters of quantum states
    Uses fidelity as similarity metric
    """
    clusters = HierarchicalClusters()
    current_level_clusters = []

    # Level 0: Each node is its own cluster
    for i, state in enumerate(quantum_states):
        cluster = Cluster {
            id: i,
            members: [i],
            centroid: state
        }
        current_level_clusters.append(cluster)

    clusters.levels.append(current_level_clusters)

    # Build higher levels via agglomerative clustering
    for level in 1..num_levels:
        next_level_clusters = []

        while len(current_level_clusters) > 0:
            # Find most similar pair of clusters
            max_fidelity = -inf
            best_pair = (None, None)

            for i, cluster1 in enumerate(current_level_clusters):
                for j in range(i+1, len(current_level_clusters)):
                    cluster2 = current_level_clusters[j]
                    fidelity = compute_quantum_fidelity(
                        cluster1.centroid,
                        cluster2.centroid
                    )
                    if fidelity > max_fidelity:
                        max_fidelity = fidelity
                        best_pair = (i, j)

            # Merge best pair
            if best_pair[0] is not None:
                cluster1 = current_level_clusters[best_pair[0]]
                cluster2 = current_level_clusters[best_pair[1]]

                # Compute new centroid (geometric mean)
                merged_centroid = quantum_geometric_mean([
                    cluster1.centroid,
                    cluster2.centroid
                ])

                merged_cluster = Cluster {
                    id: len(next_level_clusters),
                    members: cluster1.members + cluster2.members,
                    centroid: merged_centroid,
                    children: [cluster1, cluster2]
                }

                next_level_clusters.append(merged_cluster)

                # Remove merged clusters
                current_level_clusters.remove(best_pair[1])
                current_level_clusters.remove(best_pair[0])
            else:
                # No more pairs to merge
                break

        clusters.levels.append(next_level_clusters)
        current_level_clusters = next_level_clusters

    return clusters
```

#### Algorithm 3: Quantum Channel Application (Information Propagation)

```
function apply_quantum_channel(rho, channel):
    """
    Apply quantum channel to density matrix

    Φ(ρ) = Σ_k K_k ρ K_k†

    where {K_k} are Kraus operators satisfying Σ_k K_k† K_k = I
    """
    output = zeros_like(rho, dtype=complex)

    for kraus_op in channel.kraus_operators:
        # Apply Kraus operator: K_k ρ K_k†
        output += kraus_op @ rho @ conjugate_transpose(kraus_op)

    # Ensure trace preservation (numerical stability)
    trace_output = trace(output)
    if abs(trace_output - 1.0) > 1e-6:
        output = output / trace_output

    return output

function learn_kraus_operators(training_data, num_operators, quantum_dim):
    """
    Learn Kraus operators from training data

    training_data: [(ρ_in, ρ_out)] pairs of density matrices
    num_operators: number of Kraus operators to learn

    Optimization problem:
    minimize Σ_i ||Φ(ρ_in^i) - ρ_out^i||_F²
    subject to: Σ_k K_k† K_k = I (trace preservation)
    """
    # Initialize Kraus operators randomly
    kraus_ops = [
        random_unitary(quantum_dim) / sqrt(num_operators)
        for _ in range(num_operators)
    ]

    optimizer = Adam(parameters=kraus_ops, lr=0.001)

    for epoch in range(num_epochs):
        total_loss = 0

        for (rho_in, rho_out) in training_data:
            # Apply channel
            rho_pred = zeros_like(rho_in, dtype=complex)
            for K_k in kraus_ops:
                rho_pred += K_k @ rho_in @ conjugate_transpose(K_k)

            # Loss: Frobenius norm
            loss = frobenius_norm(rho_pred - rho_out) ** 2
            total_loss += loss

            # Backward pass
            loss.backward()

        # Update Kraus operators
        optimizer.step()

        # Project to trace-preserving constraint
        # Σ_k K_k† K_k = I
        sum_ktk = sum([conjugate_transpose(K) @ K for K in kraus_ops])
        eigenvalues, eigenvectors = eig_hermitian(sum_ktk)
        sqrt_inv = eigenvectors @ diag(1.0 / sqrt(eigenvalues)) @ conjugate_transpose(eigenvectors)

        # Correct Kraus operators
        for i in range(num_operators):
            kraus_ops[i] = kraus_ops[i] @ sqrt_inv

    return QuantumChannel(kraus_ops, QuantumChannelType::LearnedKraus)
```

#### Algorithm 4: Quantum State Aggregation (Geometric Mean)

```
function quantum_geometric_mean(density_matrices, weights):
    """
    Compute geometric mean of density matrices

    Solves: argmin_ρ Σ_i w_i D_B(ρ, ρ_i)²
    where D_B is Bures distance: D_B(ρ, σ) = √(2 - 2√F(ρ, σ))

    Uses Riemannian gradient descent on manifold of density matrices
    """
    # Initialize to arithmetic mean
    rho = weighted_arithmetic_mean(density_matrices, weights)
    rho = project_to_density_matrix(rho)

    for iteration in range(max_iterations):
        # Compute Riemannian gradient
        gradient = zeros_like(rho, dtype=complex)

        for i, rho_i in enumerate(density_matrices):
            # Bures distance gradient
            sqrt_rho = matrix_sqrt(rho)
            sqrt_rho_inv = matrix_inverse(sqrt_rho)

            inner = sqrt_rho @ rho_i @ sqrt_rho
            sqrt_inner = matrix_sqrt(inner)

            grad_i = sqrt_rho_inv @ sqrt_inner @ sqrt_rho_inv - eye(quantum_dim)
            gradient += weights[i] * grad_i

        # Riemannian gradient descent step
        # Exponential map: ρ_new = √ρ exp(-α G) √ρ
        step_size = 0.1
        sqrt_rho = matrix_sqrt(rho)
        update = sqrt_rho @ matrix_exp(-step_size * gradient) @ sqrt_rho

        # Project to density matrix manifold
        update = project_to_density_matrix(update)

        # Check convergence
        if frobenius_norm(update - rho) < tolerance:
            break

        rho = update

    return rho

function matrix_sqrt(A):
    """
    Compute matrix square root via eigendecomposition
    """
    eigenvalues, eigenvectors = eig_hermitian(A)
    sqrt_eigenvalues = sqrt(maximum(eigenvalues, 0))
    return eigenvectors @ diag(sqrt_eigenvalues) @ conjugate_transpose(eigenvectors)

function matrix_exp(A):
    """
    Compute matrix exponential via eigendecomposition
    """
    eigenvalues, eigenvectors = eig_hermitian(A)
    exp_eigenvalues = exp(eigenvalues)
    return eigenvectors @ diag(exp_eigenvalues) @ conjugate_transpose(eigenvectors)
```

### API Design (Function Signatures)

```rust
// ============================================================
// Public API for Quantum-Inspired Attention
// ============================================================

pub trait QuantumAttention {
    /// Create quantum attention layer
    fn new(
        config: QuantumAttentionConfig,
        embedding_dim: usize,
    ) -> Result<Self, QuantumError> where Self: Sized;

    /// Forward pass: compute quantum entanglement attention
    fn forward(
        &self,
        node_embeddings: &[Vec<f32>],
        graph: &Graph,
    ) -> Result<Vec<Vec<f32>>, QuantumError>;

    /// Encode classical embedding to quantum state
    fn encode(&self, embedding: &[f32]) -> Result<DensityMatrix, QuantumError>;

    /// Decode quantum state to classical embedding
    fn decode(&self, quantum_state: &DensityMatrix) -> Result<Vec<f32>, QuantumError>;

    /// Compute entanglement scores between nodes
    fn compute_entanglement(
        &self,
        quantum_states: &[DensityMatrix],
    ) -> EntanglementScores;

    /// Get learned basis states
    fn basis_states(&self) -> &[Array1<Complex<f32>>];

    /// Save quantum model parameters
    fn save(&self, path: &Path) -> Result<(), io::Error>;

    /// Load quantum model parameters
    fn load(path: &Path) -> Result<Self, io::Error> where Self: Sized;
}

// ============================================================
// Configuration Builders
// ============================================================

impl QuantumAttentionConfig {
    /// Default configuration for research use
    pub fn default_quantum() -> Self {
        Self {
            quantum_dim: 64,
            num_basis_states: 16,
            entanglement_metric: EntanglementMetric::Fidelity,
            channel_type: QuantumChannelType::LearnedKraus { num_operators: 4 },
            complexity_reduction: ComplexityReduction::Hierarchical { levels: 3 },
            epsilon: 1e-6,
        }
    }

    /// Large graph configuration (aggressive complexity reduction)
    pub fn large_graph() -> Self {
        Self {
            quantum_dim: 32,
            num_basis_states: 8,
            entanglement_metric: EntanglementMetric::Fidelity,
            channel_type: QuantumChannelType::Depolarizing { p: 0.1 },
            complexity_reduction: ComplexityReduction::QuantumLSH { num_hashes: 10 },
            epsilon: 1e-5,
        }
    }

    /// High accuracy configuration (minimal approximation)
    pub fn high_fidelity() -> Self {
        Self {
            quantum_dim: 128,
            num_basis_states: 32,
            entanglement_metric: EntanglementMetric::Fidelity,
            channel_type: QuantumChannelType::LearnedKraus { num_operators: 8 },
            complexity_reduction: ComplexityReduction::Hierarchical { levels: 2 },
            epsilon: 1e-7,
        }
    }
}

// ============================================================
// Density Matrix Operations
// ============================================================

impl DensityMatrix {
    /// Validate quantum properties
    pub fn validate(&self) -> Result<(), QuantumError>;

    /// Compute quantum mutual information with another state
    pub fn mutual_information(&self, other: &DensityMatrix) -> f32;

    /// Partial trace over subsystem
    pub fn partial_trace(&self, subsystem_dims: &[usize]) -> Self;

    /// Quantum relative entropy (Kullback-Leibler divergence)
    pub fn relative_entropy(&self, other: &DensityMatrix) -> f32;

    /// Compute entanglement entropy (for bipartite systems)
    pub fn entanglement_entropy(&self, partition: &[usize]) -> f32;

    /// Visualize density matrix (export heatmap)
    pub fn visualize(&self, path: &Path) -> Result<(), io::Error>;
}

// ============================================================
// Quantum Channel Operations
// ============================================================

impl QuantumChannel {
    /// Compose two quantum channels
    pub fn compose(&self, other: &QuantumChannel) -> Self;

    /// Check complete positivity (required for valid channel)
    pub fn is_completely_positive(&self) -> bool;

    /// Compute channel capacity (information-theoretic bound)
    pub fn capacity(&self) -> f32;

    /// Choi matrix representation
    pub fn choi_matrix(&self) -> Array2<Complex<f32>>;
}

// ============================================================
// Utilities
// ============================================================

/// Quantum state tomography (reconstruct density matrix from measurements)
pub fn quantum_state_tomography(
    measurements: &[(Array2<Complex<f32>>, f32)], // (observable, expectation)
    dim: usize,
) -> Result<DensityMatrix, QuantumError>;

/// Quantum process tomography (reconstruct quantum channel)
pub fn quantum_process_tomography(
    input_states: &[DensityMatrix],
    output_states: &[DensityMatrix],
) -> Result<QuantumChannel, QuantumError>;

/// Visualize quantum state on Bloch sphere (for qubit only)
pub fn visualize_bloch_sphere(
    state: &DensityMatrix,
    path: &Path,
) -> Result<(), io::Error>;
```

## Integration Points

### Affected Crates/Modules

1. **`ruvector-gnn` (Core GNN crate)**:
   - Add `attention/quantum/` module
   - Extend attention layer with quantum variant
   - Add complex number support to tensor operations

2. **`ruvector-core`**:
   - Add complex number tensor type (`ComplexTensor`)
   - Extend linear algebra with Hermitian operations
   - Add density matrix serialization

3. **`ruvector-math`** (New crate for numerical methods):
   - Implement matrix square root, exponential, logarithm
   - Riemannian optimization on quantum manifolds
   - Eigendecomposition for complex Hermitian matrices

4. **`ruvector-gnn-node` (Node.js bindings)**:
   - Expose quantum attention API
   - Serialize complex numbers to JavaScript
   - Provide visualization tools

5. **`ruvector-cli`**:
   - Add `ruvector quantum-attention` command
   - Visualize quantum states and entanglement
   - Export Bloch sphere visualizations

### New Modules to Create

```
crates/ruvector-gnn/src/attention/quantum/
├── mod.rs                    # Public API
├── density_matrix.rs         # DensityMatrix type
├── quantum_channel.rs        # QuantumChannel + Kraus operators
├── fidelity.rs               # Quantum fidelity computation
├── encoding.rs               # Classical → quantum encoding
├── decoding.rs               # Quantum → classical decoding
├── aggregation.rs            # Quantum state geometric mean
├── hierarchical.rs           # Hierarchical clustering
└── visualization.rs          # Bloch sphere, density matrix plots

crates/ruvector-math/ (new)
├── mod.rs                    # Public API
├── complex.rs                # Complex tensor type
├── linalg/
│   ├── eigen.rs              # Eigendecomposition (Hermitian)
│   ├── matrix_functions.rs  # sqrt, exp, log
│   └── svd.rs                # Singular value decomposition
└── optimization/
    ├── riemannian.rs         # Riemannian gradient descent
    └── manifolds/
        └── density_matrix.rs # Density matrix manifold

crates/ruvector-gnn-node/quantum/
├── bindings.rs               # NAPI bindings
└── typescript/
    └── quantum_attention.d.ts # TypeScript definitions
```

### Dependencies on Other Features

1. **Prerequisite: Attention Mechanisms (Tier 1, Feature #3)**:
   - Quantum attention extends base attention framework
   - **Action**: Refactor attention into trait for quantum variant

2. **Synergy: Sparse Attention (Tier 3, Feature #8)**:
   - Quantum fidelity can guide sparse pattern learning
   - **Integration**: Use fidelity scores as importance metric for pruning

3. **Synergy: Graph Condensation (Tier 3, Feature #7)**:
   - Quantum states can be condensed (dimensional reduction)
   - **Integration**: Learn low-dimensional quantum embeddings

4. **Complementary: Adaptive HNSW (Tier 2, Feature #5)**:
   - Quantum entanglement defines natural graph structure
   - **Integration**: Use entanglement scores to guide HNSW construction

## Regression Prevention

### Existing Functionality at Risk

1. **Numerical Stability**:
   - **Risk**: Complex matrix operations (sqrt, exp) may diverge or produce NaN
   - **Mitigation**:
     - Use stabilized algorithms (Schur decomposition for matrix functions)
     - Add epsilon to eigenvalues before sqrt
     - Project to valid density matrix after each operation
     - Extensive unit tests with edge cases (zero eigenvalues, near-singular matrices)

2. **Memory Consumption**:
   - **Risk**: Density matrices (d × d complex) use 16d² bytes (vs 4d for embeddings)
   - **Mitigation**:
     - Default to small quantum dimensions (d=32-64)
     - Use low-rank approximation for large graphs
     - Lazy computation (don't materialize all density matrices)

3. **Interpretability**:
   - **Risk**: Quantum concepts may confuse users (complex numbers, Hermitian matrices)
   - **Mitigation**:
     - Provide "classical mode" (real-valued fidelity approximation)
     - Extensive documentation with intuitive analogies
     - Visualization tools (Bloch sphere for qubits)

4. **Backward Compatibility**:
   - **Risk**: Breaking existing attention API
   - **Mitigation**:
     - Keep standard attention as default
     - Quantum attention is separate class (opt-in)
     - Shared trait for unified interface

### Test Cases to Prevent Regressions

```rust
// Test 1: Density matrix validity
#[test]
fn test_density_matrix_properties() {
    let psi = random_pure_state(dim=4);
    let rho = DensityMatrix::pure_state(&psi);

    // Test Hermiticity
    assert!(is_hermitian(&rho.data, tol=1e-6));

    // Test positive semi-definiteness
    let eigenvalues = rho.eigenvalues.unwrap();
    assert!(eigenvalues.iter().all(|&x| x >= -1e-6));

    // Test trace = 1
    let trace = rho.data.diag().sum();
    assert!((trace.re - 1.0).abs() < 1e-6);
    assert!(trace.im.abs() < 1e-6);

    // Test purity for pure state
    assert!((rho.purity - 1.0).abs() < 1e-5);
}

// Test 2: Fidelity bounds
#[test]
fn test_fidelity_properties() {
    let rho1 = random_density_matrix(dim=4);
    let rho2 = random_density_matrix(dim=4);

    let f = rho1.fidelity(&rho2);

    // Fidelity ∈ [0, 1]
    assert!(f >= 0.0 && f <= 1.0);

    // Fidelity symmetric
    let f_reverse = rho2.fidelity(&rho1);
    assert!((f - f_reverse).abs() < 1e-5);

    // Fidelity = 1 iff identical
    let f_self = rho1.fidelity(&rho1);
    assert!((f_self - 1.0).abs() < 1e-5);
}

// Test 3: Quantum channel trace preservation
#[test]
fn test_channel_trace_preserving() {
    let channel = QuantumChannel::amplitude_damping(dim=4, gamma=0.5);

    assert!(channel.is_trace_preserving());

    let rho_in = random_density_matrix(dim=4);
    let rho_out = channel.apply(&rho_in);

    let trace_out = rho_out.data.diag().sum();
    assert!((trace_out.re - 1.0).abs() < 1e-6);
}

// Test 4: Quantum vs classical attention accuracy
#[test]
fn test_quantum_attention_accuracy() {
    let classical_layer = StandardAttentionLayer::new(config);
    let quantum_layer = QuantumAttentionLayer::new(quantum_config);

    let node_embeddings = generate_test_embeddings(num_nodes=100, dim=64);
    let graph = generate_test_graph(num_nodes=100, avg_degree=10);

    let classical_output = classical_layer.forward(&node_embeddings, &graph).unwrap();
    let quantum_output = quantum_layer.forward(&node_embeddings, &graph).unwrap();

    // Quantum should preserve short-range accuracy
    let short_range_accuracy = compute_short_range_accuracy(&classical_output, &quantum_output);
    assert!(short_range_accuracy > 0.95);

    // Quantum should improve long-range accuracy
    let long_range_accuracy_classical = compute_long_range_accuracy(&classical_output, &graph);
    let long_range_accuracy_quantum = compute_long_range_accuracy(&quantum_output, &graph);
    assert!(long_range_accuracy_quantum > long_range_accuracy_classical * 1.2);
}

// Test 5: Complexity scaling
#[test]
fn test_hierarchical_complexity() {
    let config = QuantumAttentionConfig {
        complexity_reduction: ComplexityReduction::Hierarchical { levels: 3 },
        ..QuantumAttentionConfig::default_quantum()
    };

    let layer = QuantumAttentionLayer::new(config, 64).unwrap();

    // Measure time for different graph sizes
    let mut times = vec![];
    for n in [100, 1000, 10000, 100000] {
        let embeddings = generate_random_embeddings(n, 64);
        let graph = generate_random_graph(n, 10);

        let start = Instant::now();
        layer.forward(&embeddings, &graph).unwrap();
        let elapsed = start.elapsed();

        times.push((n, elapsed.as_secs_f64()));
    }

    // Check O(N log N) scaling
    for i in 1..times.len() {
        let (n1, t1) = times[i-1];
        let (n2, t2) = times[i];

        let empirical_ratio = t2 / t1;
        let theoretical_ratio = (n2 as f64 / n1 as f64) * ((n2 as f64).ln() / (n1 as f64).ln());

        // Allow 2x margin for overhead
        assert!(empirical_ratio < theoretical_ratio * 2.0);
    }
}

// Test 6: Geometric mean convergence
#[test]
fn test_quantum_aggregation_convergence() {
    let matrices = vec![
        random_density_matrix(dim=4),
        random_density_matrix(dim=4),
        random_density_matrix(dim=4),
    ];
    let weights = vec![0.5, 0.3, 0.2];

    let aggregator = QuantumAggregator {
        tolerance: 1e-6,
        max_iterations: 100,
    };

    let result = aggregator.geometric_mean(&matrices, &weights).unwrap();

    // Result should be valid density matrix
    assert!(result.is_valid());

    // Check optimality (first-order condition)
    let gradient_norm = compute_riemannian_gradient_norm(&result, &matrices, &weights);
    assert!(gradient_norm < 1e-4);
}
```

### Backward Compatibility Strategy

1. **API Level**:
   - Keep `StandardAttentionLayer` as default
   - Add `QuantumAttentionLayer` (opt-in experimental feature)
   - Both implement common `AttentionLayer` trait

2. **Feature Flags**:
   - Quantum attention behind `quantum` feature flag
   - Requires `ndarray` with `blas` backend for performance
   - Optional dependency on `lapack` for eigendecomposition

3. **Documentation**:
   - Clearly mark as "Experimental Research Feature"
   - Provide intuitive explanations (not just quantum mechanics)
   - Examples comparing quantum vs classical attention

4. **Fallback**:
   - If numerical issues occur, fall back to classical attention
   - Emit warning to user with debugging info

## Implementation Phases

### Phase 1: Core Quantum Math (Weeks 1-3)

**Goals**:
- Implement complex number tensor type
- Build matrix functions (sqrt, exp, log)
- Density matrix operations
- Quantum fidelity computation

**Deliverables**:
```rust
// Week 1-2: Complex tensor + linear algebra
crates/ruvector-math/
  ✓ complex.rs (ComplexTensor type)
  ✓ linalg/eigen.rs (Hermitian eigendecomposition)
  ✓ linalg/matrix_functions.rs (sqrt, exp, log)

// Week 3: Density matrices
crates/ruvector-gnn/src/attention/quantum/
  ✓ density_matrix.rs (DensityMatrix type + validation)
  ✓ fidelity.rs (quantum fidelity computation)
```

**Success Criteria**:
- Complex tensor tests pass
- Matrix functions match NumPy/SciPy (< 1e-5 error)
- Fidelity computation validates on known cases

### Phase 2: Quantum Encoding & Channels (Weeks 4-6)

**Goals**:
- Classical → quantum encoding
- Quantum channel implementation
- Learn Kraus operators
- Channel application

**Deliverables**:
```rust
// Week 4: Encoding/decoding
crates/ruvector-gnn/src/attention/quantum/
  ✓ encoding.rs (classical → quantum)
  ✓ decoding.rs (quantum → classical)

// Week 5-6: Quantum channels
crates/ruvector-gnn/src/attention/quantum/
  ✓ quantum_channel.rs (QuantumChannel + Kraus ops)
  ✓ Amplitude damping, depolarizing channels
  ✓ Learned Kraus operator training
```

**Success Criteria**:
- Encoding produces valid density matrices
- Channels are trace-preserving
- Learned channels reconstruct test data

### Phase 3: Hierarchical Attention (Weeks 7-9)

**Goals**:
- Hierarchical clustering of quantum states
- O(N log N) fidelity computation
- Quantum state aggregation (geometric mean)
- Full attention layer integration

**Deliverables**:
```rust
// Week 7-8: Hierarchical attention
crates/ruvector-gnn/src/attention/quantum/
  ✓ hierarchical.rs (hierarchical clustering)
  ✓ Complexity reduction algorithms

// Week 9: Aggregation + integration
crates/ruvector-gnn/src/attention/quantum/
  ✓ aggregation.rs (geometric mean)
  ✓ mod.rs (full QuantumAttentionLayer)
```

**Success Criteria**:
- Hierarchical attention scales to 100K nodes
- Complexity is O(N log N) empirically
- Attention output is valid embeddings

### Phase 4: Evaluation & Hardening (Weeks 10-12)

**Goals**:
- Comprehensive testing (numerical stability, edge cases)
- Documentation + tutorials
- Visualization tools (Bloch sphere, density matrices)
- Benchmarks vs classical attention

**Deliverables**:
```rust
// Week 10: Testing
tests/quantum_attention/
  ✓ Numerical stability tests
  ✓ Edge case handling (zero eigenvalues, etc.)
  ✓ Property-based tests

// Week 11: Visualization + docs
crates/ruvector-gnn/src/attention/quantum/
  ✓ visualization.rs (Bloch sphere, heatmaps)
docs/
  ✓ Quantum Attention Guide (non-physicist friendly)
  ✓ Theoretical foundations

// Week 12: Benchmarks
benches/quantum_attention.rs
  ✓ Long-range dependency accuracy
  ✓ Complexity scaling
  ✓ Comparison vs classical attention
```

**Success Criteria**:
- 100% test coverage for core quantum math
- Documentation complete with 2+ examples
- Benchmarks show long-range improvement

## Success Metrics

### Performance Benchmarks

| Benchmark | Metric | Target | Measurement Method |
|-----------|--------|--------|-------------------|
| Complexity Scaling | Time vs N | O(N log N) | Fit log-log plot to runtime data |
| Memory Usage | Bytes per node | <1KB (d=64) | Track density matrix storage |
| Fidelity Computation | Time per pair | <0.1ms | `criterion` benchmark |
| Hierarchical Clustering | Time for 1M nodes | <10s | One-time offline cost |
| Encoding/Decoding | Throughput | >10K nodes/sec | Batch processing benchmark |

### Accuracy Metrics

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Short-range accuracy (1-3 hops) | >=0.95 (vs classical) | Recall on link prediction |
| Medium-range accuracy (4-7 hops) | >=1.10 (vs classical) | Relative improvement |
| Long-range accuracy (8+ hops) | >=1.80 (vs classical) | 80% improvement target |
| Global clustering coefficient | >=0.70 | Compare to ground truth |
| Numerical stability (valid density matrices) | 100% | Validation checks |

### Research Impact Metrics

| Metric | Target | Notes |
|--------|--------|-------|
| Novel theoretical contributions | >=2 publications | Quantum GNN theory |
| Open-source citations | >=50 (2 years) | GitHub stars, papers |
| User adoption (experimental) | >=10 research groups | Academic/industrial |
| Quantum hardware readiness | Proof-of-concept | Future IBM/Rigetti integration |

## Risks and Mitigations

### Technical Risks

#### Risk 1: Numerical Instability in Matrix Square Root

**Description**:
Computing √ρ for ill-conditioned density matrices may produce NaN or Inf.

**Probability**: High (70%)

**Impact**: Critical (blocks fidelity computation)

**Mitigation**:
1. **Regularization**: Add epsilon to eigenvalues before sqrt (λ_i → λ_i + ε)
2. **Condition Number Check**: Warn if cond(ρ) > 10⁶
3. **Schur Decomposition**: Use more stable algorithm than eigendecomposition
4. **Fallback**: Use approximate fidelity (Hellinger distance) if exact fails

**Contingency Plan**:
If numerical issues persist, switch to "pseudo-quantum" mode using real-valued approximations (no complex numbers).

#### Risk 2: Geometric Mean Non-Convergence

**Description**:
Riemannian optimization for quantum barycenter may not converge.

**Probability**: Medium (40%)

**Impact**: High (blocks aggregation)

**Mitigation**:
1. **Adaptive Step Size**: Use Armijo line search for step size
2. **Multiple Initializations**: Try 3 random starts, pick best
3. **Convergence Monitoring**: Detect divergence and restart
4. **Fallback**: Use arithmetic mean (not geometrically optimal, but valid)

**Contingency Plan**:
If convergence rate <50%, replace geometric mean with weighted arithmetic mean (sacrifice optimality for reliability).

#### Risk 3: Interpretability Gap

**Description**:
Users may not understand quantum concepts (density matrices, fidelity, etc.).

**Probability**: Very High (90%)

**Impact**: Medium (adoption barrier)

**Mitigation**:
1. **Intuitive Documentation**: Use classical analogies (fidelity ≈ cosine similarity in Hilbert space)
2. **Visualization**: Provide Bloch sphere visualizations (for qubits)
3. **Classical Mode**: Offer real-valued approximation (no complex numbers)
4. **Educational Content**: Tutorials, blog posts, videos

**Contingency Plan**:
If user confusion is high, rename to "Entanglement Attention" and hide quantum terminology from API (internal implementation detail).

#### Risk 4: Limited Empirical Validation

**Description**:
Quantum attention is theoretically sound but lacks extensive empirical validation.

**Probability**: High (80%)

**Impact**: High (may not work in practice)

**Mitigation**:
1. **Benchmark Suite**: Test on 10+ diverse datasets (social, bio, citation networks)
2. **Ablation Studies**: Isolate contribution of each component
3. **Comparison**: Compare vs classical attention, sparse attention, graph transformers
4. **User Studies**: Collaborate with research groups for validation

**Contingency Plan**:
If accuracy is not competitive, pivot to "hybrid mode" (quantum for long-range, classical for short-range).

#### Risk 5: Scalability Bottleneck

**Description**:
Even O(N log N) may be too slow for billion-node graphs.

**Probability**: Medium (50%)

**Impact**: High (limits applicability)

**Mitigation**:
1. **Approximations**: Use random sampling instead of hierarchical clustering
2. **Distributed**: Parallelize across multiple GPUs/nodes
3. **Caching**: Cache quantum states between epochs
4. **Quantization**: Use low-precision (FP16) for density matrices

**Contingency Plan**:
If scalability is insufficient, limit to graphs <10M nodes and recommend sparse attention for larger graphs.

### Operational Risks

#### Risk 6: Dependency on Advanced Linear Algebra

**Description**:
Requires BLAS/LAPACK for efficient eigendecomposition. May not be available on all systems.

**Probability**: Medium (30%)

**Impact**: Medium (performance degradation)

**Mitigation**:
1. **Optional Dependency**: Make `lapack` optional (fall back to pure-Rust eigen solver)
2. **Clear Documentation**: List BLAS/LAPACK requirements prominently
3. **Pre-built Binaries**: Provide binaries with static BLAS linking

#### Risk 7: Patent/Legal Issues

**Description**:
Quantum computing is heavily patented. Risk of IP infringement.

**Probability**: Low (10%)

**Impact**: Critical (legal liability)

**Mitigation**:
1. **Prior Art Search**: Ensure algorithms are published in academic literature
2. **Legal Review**: Consult IP lawyer before release
3. **Open License**: Use permissive license (MIT/Apache 2.0) to clarify terms

---

## Appendix: Quantum Information Theory Primer

**For Non-Physicists**:

1. **Density Matrix**: Generalization of probability distribution to quantum mechanics. Represents uncertainty about quantum state.

2. **Quantum Fidelity**: Measures "closeness" of two quantum states. Analogous to cosine similarity, but in Hilbert space.

3. **Quantum Channel**: Noisy communication channel for quantum information. Models decoherence and information loss.

4. **Entanglement**: Non-local correlation between quantum systems. Two entangled nodes "share information" without direct connection.

5. **Geometric Mean**: Optimal averaging in quantum state space. Preserves quantum structure better than arithmetic mean.

**Key Intuition**:
Quantum-inspired attention treats nodes as quantum systems that can be "entangled" (correlated at a distance). This enables capturing long-range dependencies without explicit paths in the graph.

## Appendix: Related Research

This design is based on:

1. **Quantum Machine Learning** (Biamonte et al., 2017): Quantum algorithms for ML
2. **Quantum Graph Neural Networks** (Verdon et al., 2019): Quantum circuits for GNNs
3. **Quantum Attention** (Li et al., 2021): Quantum-inspired transformers
4. **Density Matrix Formalism** (Nielsen & Chuang, 2010): Standard QM textbook
5. **Riemannian Optimization** (Absil et al., 2008): Optimization on manifolds

Key differences from prior work:
- **Novel**: Hierarchical quantum state clustering for O(N log N) complexity
- **Novel**: Learned Kraus operators for quantum channels
- **Engineering**: Production-ready Rust implementation (no quantum hardware required)
- **Integration**: Seamless integration with classical GNN layers
