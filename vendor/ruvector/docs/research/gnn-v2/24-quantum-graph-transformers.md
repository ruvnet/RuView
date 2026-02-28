# Quantum Graph Transformers: From NISQ to Fault-Tolerant Graph Attention

## Overview

### Quantum Advantage for Graph Problems

Graphs are among the most natural computational structures for quantum computers. This is not a coincidence: the mathematical framework of quantum mechanics -- Hilbert spaces, unitary evolution, entanglement -- maps directly onto graph-theoretic concepts. Specifically:

1. **Graph isomorphism.** Determining whether two graphs are structurally identical is believed to be in the complexity class between P and NP-complete. Quantum walks on graphs can distinguish non-isomorphic graphs exponentially faster than classical random walks in certain cases (strongly regular graphs).

2. **Subgraph matching.** Finding a subgraph pattern within a larger graph requires exponential classical time in the worst case. Grover's algorithm provides a quadratic speedup, and structured quantum search on graph databases can achieve further improvement.

3. **Spectral analysis.** The eigenvalues of a graph's adjacency or Laplacian matrix encode fundamental structural properties (connectivity, clustering, communities). Quantum phase estimation computes eigenvalues exponentially faster than classical spectral methods for certain matrix structures.

4. **Max-Cut and combinatorial optimization.** QAOA (Quantum Approximate Optimization Algorithm) provides a quantum-native approach to graph optimization problems that classical algorithms struggle with at scale.

RuVector already implements classical versions of these in multiple crates:
- `ruqu-algorithms` provides QAOA for MaxCut (`qaoa.rs`) and surface code error correction (`surface_code.rs`)
- `ruqu-core` provides quantum circuits, simulators, and error mitigation
- `ruvector-solver` provides sublinear graph algorithms (forward/backward push, conjugate gradient, random walks)
- `ruvector-attention` provides 18+ attention mechanisms including quantum-inspired variants
- `ruvector-verified` provides proof-carrying computation for verifiable results

This document proposes a 10-year roadmap (2026-2036) for Quantum Graph Transformers that progressively leverage quantum hardware to accelerate graph attention, from near-term NISQ hybrid approaches through fault-tolerant quantum graph processing.

### Quantum vs. Classical Complexity for Graph Operations

| Operation | Best Classical | Quantum | Speedup |
|-----------|---------------|---------|---------|
| Graph isomorphism | O(2^(sqrt(n log n))) | O(n^2 poly(log n))* | Exponential* |
| Subgraph matching | O(n^k) for k-node pattern | O(n^(k/2)) via Grover | Quadratic |
| Spectral decomposition (top-k) | O(n^2) for sparse graphs | O(n poly(log n)) via QPE | Quadratic+ |
| Max-Cut | NP-hard (exact) | QAOA p-round: O(p * |E|) | Approximate |
| PageRank / PPR | O(|E| / epsilon) | O(sqrt(|E|) / epsilon) | Quadratic |
| Graph attention (all pairs) | O(N^2 d) | O(N sqrt(N) d) via quantum sampling | Quadratic |

*Conjectured; rigorous proof only for specific graph families.

---

## 1. Quantum Walk Transformers

### 1.1 Continuous-Time Quantum Walks as Attention

A continuous-time quantum walk (CTQW) on a graph G with adjacency matrix A is defined by the unitary evolution operator:

```
U(t) = exp(-i * A * t)
```

The state of the walker at time t, starting from node s, is:

```
|psi(t)> = U(t) |s> = exp(-i * A * t) |s>
```

The probability of being at node j at time t is `|<j|psi(t)>|^2`. This probability distribution acts as an "attention pattern" over the graph: the quantum walker "attends" to nodes based on the spectral structure of A.

**Key insight:** The quantum walk attention pattern captures global graph structure (through the matrix exponential) in time O(poly(log N)), whereas classical graph attention requires O(N^2) time to compute all pairwise scores.

**Quantum Walk Attention Score:**

```
alpha(s, j, t) = |<j| exp(-i * A * t) |s>|^2
```

This is a natural attention mechanism: it is (1) non-negative, (2) sums to 1 over all j, (3) depends on graph topology, and (4) is parameterized by t (analogous to temperature in softmax).

```rust
/// Quantum Walk Graph Attention
/// Uses CTQW probability distribution as attention weights
pub struct QuantumWalkAttention {
    /// Walk time parameter (analogous to softmax temperature)
    walk_time: f64,
    /// Number of qubits (log2 of graph size)
    num_qubits: u32,
    /// Quantum circuit for walk simulation
    circuit_cache: Option<QuantumCircuit>,
}

impl QuantumWalkAttention {
    /// Build quantum circuit for CTQW on graph with adjacency A
    ///
    /// Uses Hamiltonian simulation: exp(-iAt) via Trotter-Suzuki
    /// decomposition into native gate set.
    pub fn build_walk_circuit(
        &self,
        graph: &Graph,
        source_node: u32,
        trotter_steps: u32,
    ) -> QuantumCircuit {
        let n = graph.num_nodes;
        let num_qubits = (n as f64).log2().ceil() as u32;
        let mut circuit = QuantumCircuit::new(num_qubits);

        // Encode source node in binary
        for bit in 0..num_qubits {
            if (source_node >> bit) & 1 == 1 {
                circuit.x(bit);
            }
        }

        // Trotterized Hamiltonian simulation: exp(-iAt)
        let dt = self.walk_time / trotter_steps as f64;
        for _step in 0..trotter_steps {
            // Each edge (i,j,w) contributes exp(-i * w * dt * Z_i Z_j)
            for &(i, j, w) in &graph.edges {
                circuit.rzz(i, j, 2.0 * w * dt);
            }
            // Mixing terms for non-diagonal Hamiltonian
            for q in 0..num_qubits {
                circuit.rx(q, 2.0 * dt);
            }
        }

        circuit
    }

    /// Compute quantum walk attention scores via simulation
    /// Returns attention distribution over all nodes from source
    pub fn attention_scores(
        &self,
        graph: &Graph,
        source_node: u32,
    ) -> Result<Vec<f64>, QuantumError> {
        let circuit = self.build_walk_circuit(graph, source_node, 10);
        let result = Simulator::run(&circuit)?;
        let probs = result.state.probabilities();

        // Probabilities over basis states = attention over nodes
        Ok(probs[..graph.num_nodes as usize].to_vec())
    }
}
```

### 1.2 Interference Patterns as Message Aggregation

Quantum interference -- the constructive and destructive combination of probability amplitudes -- provides a natural message aggregation mechanism for graph transformers:

- **Constructive interference:** Messages from correlated neighbors amplify each other (analogous to high attention weight)
- **Destructive interference:** Messages from anti-correlated neighbors cancel (analogous to zero attention weight)
- **Superposition:** A node simultaneously "attends" to all neighbors in quantum superposition, with interference determining the final attention pattern

This is fundamentally different from classical softmax attention, which cannot cancel messages -- it can only reduce their weight to near-zero.

---

## 2. Variational Quantum Graph Circuits

### 2.1 Parameterized Quantum Circuits for Graph Classification

Variational Quantum Eigensolvers (VQE) and QAOA represent the most promising near-term (NISQ-era) quantum approaches to graph problems. RuVector's `ruqu-algorithms/src/qaoa.rs` already implements the full QAOA pipeline:

```rust
// Existing RuVector QAOA implementation
pub fn build_qaoa_circuit(graph: &Graph, gammas: &[f64], betas: &[f64]) -> QuantumCircuit {
    // |+>^n --[C(gamma_1)][B(beta_1)]--...--[C(gamma_p)][B(beta_p)]-- measure
    //
    // Phase separator: Rzz(2 * gamma * w) for each edge
    // Mixer: Rx(2 * beta) for each qubit
}
```

**Extension to Graph Attention:** We can generalize QAOA to a Variational Quantum Graph Transformer (VQGT) where:

1. **Phase separator** encodes graph structure (edges as Rzz interactions)
2. **Mixer** enables exploration of attention patterns (Rx rotations)
3. **Variational parameters** (gamma, beta) are optimized to maximize a task-specific objective
4. **Measurement** produces the attention distribution

```rust
/// Variational Quantum Graph Transformer layer
pub struct VQGTLayer {
    /// QAOA-style depth
    p: u32,
    /// Learnable phase parameters [p]
    gammas: Vec<f64>,
    /// Learnable mixer parameters [p]
    betas: Vec<f64>,
    /// Additional rotation parameters for expressivity [p * n_qubits]
    thetas: Vec<f64>,
}

impl VQGTLayer {
    /// Build parameterized circuit for one graph attention layer
    pub fn build_circuit(&self, graph: &Graph) -> QuantumCircuit {
        let n = graph.num_nodes;
        let mut circuit = QuantumCircuit::new(n);

        // Initial superposition
        for q in 0..n {
            circuit.h(q);
        }

        for layer in 0..self.p as usize {
            // Phase separator: encode graph topology
            for &(i, j, w) in &graph.edges {
                circuit.rzz(i, j, 2.0 * self.gammas[layer] * w);
            }

            // Node-specific rotations for expressivity
            for q in 0..n {
                let theta_idx = layer * n as usize + q as usize;
                if theta_idx < self.thetas.len() {
                    circuit.ry(q, self.thetas[theta_idx]);
                }
            }

            // Mixer
            for q in 0..n {
                circuit.rx(q, 2.0 * self.betas[layer]);
            }
        }

        circuit
    }

    /// Classical optimization step using parameter-shift rule
    /// Returns gradient for all parameters
    pub fn compute_gradient(
        &self,
        graph: &Graph,
        cost_fn: &dyn Fn(&[f64]) -> f64,
    ) -> Vec<f64> {
        let shift = std::f64::consts::FRAC_PI_2;
        let mut gradients = Vec::new();

        // Gradient for each gamma
        for i in 0..self.p as usize {
            let mut params_plus = self.gammas.clone();
            params_plus[i] += shift;
            let mut params_minus = self.gammas.clone();
            params_minus[i] -= shift;

            let grad = (cost_fn(&params_plus) - cost_fn(&params_minus)) / 2.0;
            gradients.push(grad);
        }

        // Similar for betas and thetas...
        gradients
    }
}
```

### 2.2 Quantum Approximate Optimization on Graph Attention

QAOA can directly optimize graph attention patterns. Given a graph and a task-specific objective (e.g., node classification accuracy), QAOA finds the partition (attention pattern) that approximately maximizes the objective:

| QAOA Depth (p) | Approximation Ratio | Circuit Depth | Classical Equivalent |
|----------------|--------------------:|---------------|---------------------|
| 1 | 0.692 | O(|E|) | Random 0.5 |
| 2 | 0.756 | O(2|E|) | Simple heuristic |
| 5 | 0.85+ | O(5|E|) | Greedy algorithm |
| 10 | 0.95+ | O(10|E|) | Simulated annealing |
| poly(n) | 1.0 - epsilon | O(poly(n)|E|) | Exponential time |

---

## 3. Topological Quantum Error Correction on Graphs

### 3.1 Surface Codes as Graph Transformers

Surface codes -- the leading quantum error correction architecture -- are inherently graph-structured. RuVector's `ruqu-algorithms/src/surface_code.rs` implements a distance-3 rotated surface code:

```rust
// Existing: Surface code as a graph structure
pub struct SurfaceCodeLayout {
    data_qubits: Vec<QubitIndex>,      // 9 data qubits (3x3 grid)
    x_ancillas: Vec<QubitIndex>,       // 4 X-type stabilizers
    z_ancillas: Vec<QubitIndex>,       // 4 Z-type stabilizers
    x_stabilizers: Vec<Vec<QubitIndex>>, // Plaquette operators
    z_stabilizers: Vec<Vec<QubitIndex>>, // Vertex operators
}
```

**Insight:** A surface code is a graph transformer where:
- **Nodes** = data qubits + ancilla qubits
- **Edges** = stabilizer interactions (CNOT gates)
- **Attention** = syndrome extraction (measuring which stabilizers detect errors)
- **Message passing** = error correction (applying Pauli gates based on syndrome)

The syndrome decoder (`decode_syndrome` in `surface_code.rs`) is a graph attention mechanism: it receives a syndrome vector (which stabilizers fired) and must determine which data qubit caused the error -- this requires attending to the graph structure of stabilizer overlaps.

### 3.2 Anyonic Braiding as Attention Routing

In topological quantum computation, information is encoded in the worldlines of anyonic quasiparticles. Braiding two anyons -- swapping their positions -- implements a quantum gate. This maps to graph attention:

- **Anyons** = attention heads
- **Braiding** = attention routing (which heads attend to which nodes)
- **Topological protection** = the attention pattern is robust to local perturbations (noise)

```
Anyonic Attention Routing:

Time ↓
  |  Head 1    Head 2    Head 3
  |    |         |         |
  |    |    ╲    |         |       <- Braid 1-2: swap attention targets
  |    |     ╲   |         |
  |    |      ╲  |         |
  |    |       ╳ |         |
  |    |      ╱  |         |
  |    |     ╱   |         |
  |    |    ╱    |    ╲    |       <- Braid 2-3: swap attention targets
  |    |         |     ╲   |
  |    |         |      ╳  |
  |    |         |     ╱   |
  |    |         |    ╱    |
  |    v         v         v
  |  Node A    Node C    Node B      (permuted attention assignment)
```

The topological protection means this attention routing is inherently fault-tolerant: small perturbations (noise in attention weights) cannot change the braiding pattern (topological invariant).

---

## 4. Quantum-Classical Hybrid Architectures

### 4.1 Quantum Kernel Methods for Graph Attention

Quantum kernel methods use a quantum computer to compute a kernel function K(G1, G2) between two graphs, then use classical machine learning (SVM, kernel PCA) on the quantum-computed kernel:

```
Quantum Kernel for Graphs:
K(G1, G2) = |<0| U†(G1) U(G2) |0>|^2
```

Where U(G) is a parameterized quantum circuit encoding graph G. The kernel value measures the "overlap" between the quantum states encoding the two graphs -- a natural similarity measure.

```rust
/// Quantum kernel for graph similarity
pub struct QuantumGraphKernel {
    /// Circuit depth for graph encoding
    encoding_depth: u32,
    /// Simulator for kernel evaluation
    seed: Option<u64>,
}

impl QuantumGraphKernel {
    /// Encode a graph into a quantum state
    fn encode_graph(&self, graph: &Graph) -> QuantumCircuit {
        let n = graph.num_nodes;
        let mut circuit = QuantumCircuit::new(n);

        // Encode node features as rotations
        for q in 0..n {
            circuit.ry(q, std::f64::consts::FRAC_PI_4);
        }

        // Encode edges as entangling gates
        for &(i, j, w) in &graph.edges {
            circuit.rzz(i, j, w * std::f64::consts::FRAC_PI_2);
        }

        circuit
    }

    /// Compute quantum kernel between two graphs
    pub fn kernel(
        &self,
        g1: &Graph,
        g2: &Graph,
    ) -> Result<f64, QuantumError> {
        // Build circuit: U†(G1) U(G2)
        let c1 = self.encode_graph(g1);
        let c2 = self.encode_graph(g2);

        // Compose circuits: U(G2) followed by U†(G1)
        let mut combined = c2;
        combined.append_inverse(&c1);

        // Measure probability of all-zero state
        let sim_config = SimConfig {
            seed: self.seed,
            noise: None,
            shots: None,
        };
        let result = Simulator::run_with_config(&combined, &sim_config)?;
        let probs = result.state.probabilities();

        // Kernel value = probability of returning to |0>
        Ok(probs[0])
    }
}
```

### 4.2 Classical Pre/Post-Processing with Quantum Core

The most practical near-term architecture separates the pipeline into classical and quantum components:

```
┌──────────────────────────────────────────────────┐
│              Classical Pre-Processing             │
│                                                   │
│  1. Graph sparsification (ruvector-solver)        │
│  2. Subgraph extraction (interesting regions)     │
│  3. Feature encoding (node/edge embeddings)       │
│  4. Problem reduction (< 100 qubits)             │
└──────────────────────┬───────────────────────────┘
                       │
                       v
┌──────────────────────────────────────────────────┐
│              Quantum Core                         │
│                                                   │
│  5. Quantum walk attention (CTQW)                │
│  6. QAOA optimization (graph partitioning)       │
│  7. Quantum kernel evaluation (graph matching)   │
│  8. Quantum spectral analysis (QPE)             │
└──────────────────────┬───────────────────────────┘
                       │
                       v
┌──────────────────────────────────────────────────┐
│              Classical Post-Processing            │
│                                                   │
│  9. Measurement decoding                         │
│  10. Error mitigation (ruqu-core mitigation.rs)  │
│  11. Result verification (ruvector-verified)      │
│  12. Integration with graph transformer layers   │
└──────────────────────────────────────────────────┘
```

**Critical insight:** The quantum core needs only 50-1000 qubits for meaningful graph attention on subgraphs of 50-1000 nodes. Classical pre-processing (via `ruvector-solver`) reduces billion-node graphs to tractable subproblems. Classical post-processing (via `ruvector-verified`) ensures the quantum results are correct.

---

## 5. Quantum Advantage Timeline

### 5.1 NISQ Era (2024-2028)

**Hardware:** 50-1000 noisy qubits, error rates ~10^-3, no error correction.

**Viable graph operations:**
- QAOA for graph optimization on small instances (< 100 nodes)
- Quantum kernel evaluation for graph classification (< 50 nodes per graph)
- Variational quantum graph circuits (VQE-style, < 100 parameters)

**RuVector integration:**
- Hybrid classical-quantum pipeline using `ruqu-core` simulator
- Error mitigation via `ruqu-core/src/mitigation.rs`
- Subgraph extraction via `ruvector-solver` to reduce problem size
- Proof-carrying results via `ruvector-verified`

**Limitations:**
- Noise limits circuit depth (< 100 gates per qubit)
- No quantum error correction (results have ~1-10% error rate)
- Classical simulation is competitive for most problem sizes

### 5.2 Early Fault-Tolerant Era (2028-2032)

**Hardware:** 1,000-100,000 physical qubits, 100-1,000 logical qubits, error rates ~10^-6.

**Viable graph operations:**
- Quantum walks on graphs with 1,000+ nodes
- Quantum phase estimation for graph spectral analysis
- Quantum-enhanced graph attention for molecular graphs (drug discovery)
- Grover search on graph databases

**RuVector integration:**
- Surface code error correction using `ruqu-algorithms/src/surface_code.rs`
- Hardware-aware circuit compilation via `ruqu-core/src/transpiler.rs`
- Mixed-precision quantum-classical computation via `ruqu-core/src/mixed_precision.rs`
- QEC scheduling via `ruqu-core/src/qec_scheduler.rs`

**2030 milestone: 1,000-qubit graph attention on molecular graphs.** A quantum graph transformer processing molecular interaction graphs for drug discovery. Each molecule is a graph (atoms = nodes, bonds = edges). Quantum attention captures quantum mechanical properties (electron orbitals, bond energies) that classical attention cannot.

### 5.3 Full Fault-Tolerant Era (2032-2040)

**Hardware:** 1M+ physical qubits, 10,000+ logical qubits, error rates ~10^-12.

**Viable graph operations:**
- Polynomial-time graph isomorphism testing
- Exponentially faster subgraph matching
- Quantum-advantage graph attention for any graph size
- Fault-tolerant quantum graph transformer layers

**RuVector integration:**
- Full quantum graph transformer compilation
- Tensor network simulation for classical verification (`ruqu-core/src/tensor_network.rs`)
- Lean-verified quantum circuits (`ruvector-verified` + `ruvector-verified-wasm`)

**2036 milestone: Fault-tolerant quantum graph transformers solving NP-intermediate problems.** Graph isomorphism, certain subgraph matching instances, and graph property testing at scales impossible for classical computers. Proven quantum advantage (not just quantum utility).

---

## 6. Concrete Quantum Circuit Designs

### 6.1 Quantum Graph Attention Circuit

```
Quantum Graph Attention for N-node graph, d-dimensional features:

Qubits: N node qubits + d feature qubits + 1 ancilla

Step 1: Feature Encoding
  |0>^d ──[Ry(f_0)]──[Ry(f_1)]──...──[Ry(f_d)]──  (encode features)

Step 2: Graph Structure Encoding
  For each edge (i,j,w):
    ──[Rzz(w)]── on qubits i,j  (encode adjacency)

Step 3: Quantum Attention (parameterized)
  For p rounds:
    ──[Phase(gamma_p)]──[Mix(beta_p)]──
  Where:
    Phase: Rzz on all edges (graph-aware)
    Mix: Rx on all nodes (exploration)

Step 4: Measurement
  Measure all node qubits → attention distribution
  Measure feature qubits → transformed features

Total gates: O(p * |E| + N * d)
Total depth: O(p * (|E|/parallelism + d))
```

### 6.2 Quantum-Enhanced Graph Spectral Attention

```rust
/// Quantum Phase Estimation for graph spectral attention
/// Computes eigenvalues of graph Laplacian to determine attention
pub struct QuantumSpectralAttention {
    /// Number of precision qubits for QPE
    precision_qubits: u32,
    /// Number of Trotter steps for Hamiltonian simulation
    trotter_steps: u32,
}

impl QuantumSpectralAttention {
    /// Build QPE circuit for graph Laplacian eigenvalue estimation
    ///
    /// The Laplacian eigenvalues directly encode graph structure:
    /// - lambda_0 = 0 always (connected components)
    /// - lambda_1 = algebraic connectivity (Fiedler value)
    /// - lambda_max = spectral radius
    ///
    /// Attention weight for node j from source s:
    /// alpha(s,j) = sum_k |<j|v_k>|^2 * f(lambda_k)
    /// where v_k are eigenvectors, lambda_k are eigenvalues,
    /// and f is a learned spectral filter.
    pub fn build_qpe_circuit(
        &self,
        graph: &Graph,
    ) -> QuantumCircuit {
        let n = graph.num_nodes;
        let total_qubits = n + self.precision_qubits;
        let mut circuit = QuantumCircuit::new(total_qubits);

        // Initialize precision register in superposition
        for q in 0..self.precision_qubits {
            circuit.h(q);
        }

        // Controlled Hamiltonian simulation
        // H = L (graph Laplacian)
        // U = exp(-i L t) for increasing powers of t
        for k in 0..self.precision_qubits {
            let power = 1 << k;
            let time = 2.0 * std::f64::consts::PI * power as f64;
            let dt = time / self.trotter_steps as f64;

            for _step in 0..self.trotter_steps {
                // Controlled Laplacian evolution
                for &(i, j, w) in &graph.edges {
                    // Controlled-Rzz: precision qubit k controls
                    // the interaction between node qubits i,j
                    circuit.crzz(
                        k,
                        self.precision_qubits + i,
                        self.precision_qubits + j,
                        2.0 * w * dt,
                    );
                }
            }
        }

        // Inverse QFT on precision register
        circuit.inverse_qft(0, self.precision_qubits);

        circuit
    }
}
```

---

## 7. Connection to RuVector Crates

### 7.1 Existing Quantum Infrastructure

| Crate | Module | Quantum Graph Transformer Role |
|-------|--------|-------------------------------|
| `ruqu-core` | `circuit.rs` | Quantum circuit construction |
| `ruqu-core` | `simulator.rs` | Classical simulation of quantum circuits |
| `ruqu-core` | `gate.rs` | Native gate set (H, CNOT, Rx, Ry, Rz, Rzz) |
| `ruqu-core` | `transpiler.rs` | Circuit optimization and compilation |
| `ruqu-core` | `mitigation.rs` | Error mitigation for NISQ results |
| `ruqu-core` | `mixed_precision.rs` | Hybrid precision quantum-classical |
| `ruqu-core` | `qec_scheduler.rs` | QEC cycle scheduling |
| `ruqu-core` | `tensor_network.rs` | Tensor network simulation |
| `ruqu-core` | `verification.rs` | Quantum result verification |
| `ruqu-core` | `witness.rs` | Quantum witness generation |
| `ruqu-algorithms` | `qaoa.rs` | QAOA for MaxCut (graph optimization) |
| `ruqu-algorithms` | `surface_code.rs` | Surface code error correction |
| `ruqu-algorithms` | `vqe.rs` | Variational quantum eigensolver |
| `ruqu-algorithms` | `grover.rs` | Grover search (graph database queries) |
| `ruqu-exotic` | `interference_search.rs` | Quantum interference search |
| `ruqu-exotic` | `swarm_interference.rs` | Multi-agent quantum interference |

### 7.2 Classical Crates Supporting Quantum Graph Transformers

| Crate | Module | Role |
|-------|--------|------|
| `ruvector-solver` | `forward_push.rs` | Sublinear graph pre-processing |
| `ruvector-solver` | `cg.rs` | Conjugate gradient for spectral analysis |
| `ruvector-solver` | `random_walk.rs` | Classical random walk baseline |
| `ruvector-attention` | `graph/` | Classical graph attention baseline |
| `ruvector-attention` | `sparse/` | Sparse attention (classical fallback) |
| `ruvector-verified` | `pipeline.rs` | Proof-carrying verification pipeline |
| `ruvector-verified` | `invariants.rs` | Mathematical invariant verification |
| `ruvector-gnn` | `layer.rs` | GNN layers for pre-/post-processing |

### 7.3 Proposed New Modules

```
crates/ruqu-algorithms/src/
    quantum_walk.rs            -- Continuous-time quantum walk attention
    quantum_graph_kernel.rs    -- Quantum kernel for graph similarity
    quantum_spectral.rs        -- QPE-based spectral graph attention
    vqgt.rs                    -- Variational Quantum Graph Transformer

crates/ruqu-core/src/
    graph_encoding.rs          -- Graph-to-circuit encoding strategies
    crzz.rs                    -- Controlled-Rzz gate implementation

crates/ruvector-attention/src/
    quantum/mod.rs             -- Quantum attention module
    quantum/walk_attention.rs  -- CTQW-based attention
    quantum/kernel_attention.rs -- Quantum kernel attention
    quantum/spectral_attention.rs -- QPE spectral attention
```

---

## 8. Hybrid Quantum-Classical Graph Transformer: Full Design

### 8.1 Architecture

```
┌─────────────────────────────────────────────────────┐
│  Hybrid Quantum-Classical Graph Transformer (HQCGT) │
│                                                      │
│  Classical Input: Graph G = (V, E), node features X │
│                                                      │
│  Layer 1: Classical GNN Encoder                      │
│  ┌───────────────────────────────────────────────┐  │
│  │  ruvector-gnn layer.rs                         │  │
│  │  Input: X (N x d_in)                          │  │
│  │  Output: H (N x d_hidden) -- node embeddings  │  │
│  └───────────────────────────────────────────────┘  │
│                                                      │
│  Layer 2: Quantum Attention Core                     │
│  ┌───────────────────────────────────────────────┐  │
│  │  For each node s:                              │  │
│  │    1. Extract k-hop subgraph around s          │  │
│  │       (ruvector-solver forward_push.rs)        │  │
│  │    2. Build QAOA circuit for subgraph          │  │
│  │       (ruqu-algorithms qaoa.rs)                │  │
│  │    3. Run quantum attention on subgraph        │  │
│  │    4. Error mitigate results                   │  │
│  │       (ruqu-core mitigation.rs)                │  │
│  │    5. Verify results                           │  │
│  │       (ruvector-verified pipeline.rs)           │  │
│  │  Output: A (N x N) -- quantum attention matrix │  │
│  └───────────────────────────────────────────────┘  │
│                                                      │
│  Layer 3: Classical Transformer Decoder              │
│  ┌───────────────────────────────────────────────┐  │
│  │  ruvector-attention multi_head.rs              │  │
│  │  Input: H, A                                   │  │
│  │  Output: Z (N x d_out)                         │  │
│  └───────────────────────────────────────────────┘  │
│                                                      │
│  EWC Continual Learning (ruvector-gnn ewc.rs)       │
│  Replay Buffer (ruvector-gnn replay.rs)              │
└─────────────────────────────────────────────────────┘
```

### 8.2 Complexity Analysis

| Component | Classical | Quantum Hybrid | Speedup |
|-----------|----------|----------------|---------|
| GNN encoding | O(|E| d) | O(|E| d) | 1x (classical) |
| Attention computation | O(N^2 d) | O(N * k^2 * p) | N/k^2 for k-hop subgraphs |
| Spectral analysis | O(N^2) | O(N poly(log N)) | Exponential (QPE) |
| Error mitigation | -- | O(shots * circuit_depth) | Overhead |
| Verification | O(1) | O(proof_size) | Overhead |
| **Total** | **O(N^2 d)** | **O(N k^2 p + N log N)** | **N/k^2 for local, exp for spectral** |

For a 1M-node graph with k=100 hop subgraphs, p=5 QAOA rounds:
- Classical: O(10^12) operations
- Quantum hybrid: O(10^6 * 10^4 * 5) = O(5 * 10^10) operations
- Speedup: ~20x from quantum attention alone
- With QPE spectral: exponential speedup for eigenvalue computation

---

## 9. Proof-Carrying Quantum Circuits

### 9.1 Verified Quantum Graph Attention

A unique advantage of RuVector is the `ruvector-verified` crate, which provides proof-carrying computation. This extends naturally to quantum circuits:

1. **Circuit correctness:** Verify that the quantum circuit correctly encodes the graph structure
2. **Result validity:** Verify that measurement outcomes are consistent with quantum mechanics
3. **Error bound certification:** Prove that error mitigation reduces error below a threshold
4. **Attention validity:** Verify that quantum attention scores form a valid probability distribution

```rust
/// Proof-carrying quantum graph attention
pub struct VerifiedQuantumAttention {
    /// Quantum attention engine
    quantum_attn: QuantumWalkAttention,
    /// Verification pipeline
    verifier: VerificationPipeline,
}

impl VerifiedQuantumAttention {
    /// Compute quantum attention with proof of correctness
    pub fn attend_verified(
        &self,
        graph: &Graph,
        source: u32,
    ) -> Result<(Vec<f64>, Proof), Error> {
        // 1. Compute quantum attention
        let attention = self.quantum_attn.attention_scores(graph, source)?;

        // 2. Generate proof of validity
        let proof = self.verifier.prove(ProofGoal::AttentionValid {
            scores: &attention,
            graph,
            source,
            invariants: vec![
                Invariant::NonNegative,       // all scores >= 0
                Invariant::SumsToOne,         // scores sum to ~1.0
                Invariant::GraphConsistent,   // non-zero only for reachable nodes
                Invariant::ErrorBounded(1e-6), // error < threshold
            ],
        })?;

        Ok((attention, proof))
    }
}
```

### 9.2 Connection to Lean Formal Verification

The `ruvector-verified` and `ruvector-verified-wasm` crates (currently under development on this branch) provide the foundation for formally verified quantum graph transformers. The integration with Lean 4 enables:

- **Theorem:** For any graph G and quantum walk time t, the attention scores alpha(s,j,t) form a valid probability distribution.
- **Theorem:** QAOA at depth p >= poly(n) achieves optimal Max-Cut on G with probability approaching 1.
- **Theorem:** Surface code with distance d corrects all errors of weight < d/2.

These theorems, proved in Lean 4, can be compiled to WASM via `ruvector-verified-wasm` and checked at runtime.

---

## 10. Research Timeline and Milestones

### Phase 1: NISQ Hybrid (2026-2028)
- Implement quantum kernel for graph similarity using `ruqu-core`
- QAOA-based graph attention on molecular graphs (< 100 nodes)
- Classical simulator benchmarking
- Error mitigation integration
- **Milestone:** Quantum-advantage demonstration on graph classification benchmark

### Phase 2: Quantum Walk Attention (2028-2030)
- Continuous-time quantum walk attention circuits
- Hardware deployment on 100-1000 qubit devices
- Integration with `ruvector-solver` for subgraph extraction
- **Milestone:** 1,000-qubit graph attention on drug discovery molecular graphs

### Phase 3: Fault-Tolerant Spectral (2030-2033)
- QPE-based spectral graph attention
- Surface code integration for error correction
- Verified quantum circuits via `ruvector-verified` + Lean 4
- **Milestone:** Fault-tolerant quantum spectral analysis surpassing classical

### Phase 4: Full Quantum Graph Transformer (2033-2036)
- Complete quantum graph transformer layer (encode-attend-decode)
- Topological protection via anyonic braiding
- Hybrid quantum-classical continual learning (quantum EWC)
- **Milestone:** Solving NP-intermediate graph problems with proven quantum advantage

---

## 11. Open Questions

1. **Barren plateaus.** Variational quantum circuits for large graphs may exhibit barren plateaus (exponentially vanishing gradients). Does graph structure provide enough inductive bias to avoid this? Preliminary evidence from QAOA suggests yes for bounded-degree graphs.

2. **Quantum noise vs. graph noise.** Real graphs are noisy (missing edges, incorrect weights). Does quantum noise interact constructively or destructively with graph noise? Could quantum error correction simultaneously correct both?

3. **Optimal graph-to-circuit encoding.** How to best encode a graph into a quantum circuit? Direct adjacency encoding (Rzz per edge) scales as O(|E|) circuit depth. Are there more efficient encodings using graph compression?

4. **Quantum advantage threshold.** At what graph size does quantum graph attention surpass classical? Current estimates: ~100-1000 nodes for NISQ, ~10,000 nodes for early fault-tolerant. This depends heavily on problem structure.

5. **Classical simulability.** Tensor network methods can efficiently simulate quantum circuits on graphs with low treewidth. What fraction of real-world graphs have low enough treewidth to be classically simulable?

6. **Integration overhead.** The quantum-classical interface (encoding/decoding, error mitigation, verification) adds overhead. At what problem size does the quantum speedup dominate the interface cost?

---

## References

- Farhi, E. & Goldstone, J. (2014). A Quantum Approximate Optimization Algorithm. arXiv:1411.4028.
- Childs, A. (2009). Universal computation by quantum walk. Physical Review Letters.
- Schuld, M. & Killoran, N. (2019). Quantum machine learning in feature Hilbert spaces. Physical Review Letters.
- Aharonov, D. & Ben-Or, M. (1999). Fault-tolerant quantum computation with constant error rate. arXiv:quant-ph/9906129.
- Kitaev, A. (2003). Fault-tolerant quantum computation by anyons. Annals of Physics.
- Fowler, A., et al. (2012). Surface codes: Towards practical large-scale quantum computation. Physical Review A.
- Bharti, K., et al. (2022). Noisy intermediate-scale quantum algorithms. Reviews of Modern Physics.
- Cerezo, M., et al. (2021). Variational quantum algorithms. Nature Reviews Physics.
- Preskill, J. (2018). Quantum computing in the NISQ era and beyond. Quantum.
- Abbas, A., et al. (2021). The power of quantum neural networks. Nature Computational Science.

---

**Document Status:** Research Proposal
**Target Integration:** RuVector GNN v2 Phase 3-5 (Quantum Track)
**Estimated Effort:** 24-36 months (phased over 10 years)
**Risk Level:** Very High (Phase 1-2), Extreme (Phase 3-4)
**Dependencies:** ruqu-core, ruqu-algorithms, ruqu-exotic, ruvector-solver, ruvector-attention, ruvector-verified
