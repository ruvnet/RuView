# ADR-QE-007: QAOA MaxCut Implementation

**Status**: Proposed
**Date**: 2026-02-06
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-06 | ruv.io | Initial QAOA MaxCut architecture proposal |

---

## Context

### Combinatorial Optimization on Quantum Computers

The Quantum Approximate Optimization Algorithm (QAOA), introduced by Farhi, Goldstone,
and Gutmann (2014), is a leading candidate for demonstrating quantum advantage on
combinatorial optimization problems. QAOA constructs a parameterized quantum circuit that
encodes the cost function of an optimization problem and uses classical outer-loop
optimization to find parameters that maximize the expected cost.

### MaxCut as the Canonical QAOA Problem

MaxCut is the prototypical problem for QAOA: given a graph G = (V, E), partition the
vertices into two sets S and S-complement to maximize the number of edges crossing the
partition.

```
MaxCut Example (5 vertices, 6 edges):

    0 ─── 1
    │ \   │
    │   \ │
    3 ─── 2
          │
          4

Optimal cut: S = {0, 2, 4}, S' = {1, 3}
Cut value: 5 edges crossing (0-1, 0-3, 1-2, 2-3, 2-4)
```

The cost function is:

```
C(z) = sum_{(i,j) in E} (1 - z_i * z_j) / 2
```

where z_i in {+1, -1} encodes the partition assignment.

### QAOA Circuit Structure

A depth-p QAOA circuit alternates two types of layers:

1. **Phase separation** (encodes the problem): For each edge (i,j), apply
   exp(-i * gamma * Z_i Z_j / 2)
2. **Mixing** (explores the solution space): For each qubit i, apply
   exp(-i * beta * X_i) = Rx(2*beta)

```
QAOA Circuit (p layers):

|+>  ──[Phase(gamma_1)]──[Mix(beta_1)]──[Phase(gamma_2)]──[Mix(beta_2)]── ... ──[Measure]
                                                                                      │
Parameters: gamma = [gamma_1, ..., gamma_p], beta = [beta_1, ..., beta_p]            │
                                                                                      ▼
                                                                              Classical
                                                                              Optimizer
```

### Why QAOA Matters for ruQu

| Motivation | Details |
|------------|---------|
| Optimization benchmarks | Standard workload for evaluating quantum simulator performance |
| Graph problems | Natural integration with ruVector graph database (ruvector-graph) |
| Variational algorithm | Shares infrastructure with VQE (ADR-QE-005): parameterized circuits, expectation values, classical optimizers |
| Scalability study | QAOA depth and graph size provide tunable complexity for benchmarking |
| Agent integration | ruVector agents can use QAOA to solve graph optimization tasks autonomously |

---

## Decision

### 1. Phase Separation Operator: Native Rzz Gate

The phase separation operator for MaxCut applies exp(-i * gamma * Z_i Z_j / 2) for
each edge (i,j). We implement this as a native two-qubit operation via direct amplitude
manipulation, avoiding CNOT decomposition.

**Mathematical basis**:

```
exp(-i * theta * Z_i Z_j / 2) acts on computational basis states as:

  |00> -> e^{-i*theta/2} |00>    (Z_i Z_j = +1)
  |01> -> e^{+i*theta/2} |01>    (Z_i Z_j = -1)
  |10> -> e^{+i*theta/2} |10>    (Z_i Z_j = -1)
  |11> -> e^{-i*theta/2} |11>    (Z_i Z_j = +1)
```

In the state vector, for each amplitude at index k:
- Extract bits i and j from k
- Compute parity = bit_i XOR bit_j
- Apply phase: `amp[k] *= exp(-i * theta * (-1)^parity / 2)`
  - If parity = 0 (same bits): `amp[k] *= exp(-i * theta / 2)`
  - If parity = 1 (different bits): `amp[k] *= exp(+i * theta / 2)`

```rust
impl QuantumState {
    /// Apply Rzz(theta) = exp(-i * theta * Z_i Z_j / 2) via direct amplitude
    /// manipulation.
    ///
    /// For each basis state |k>:
    ///   - Compute parity of bits i and j in k
    ///   - Apply phase e^{-i * theta * (-1)^parity / 2}
    ///
    /// Complexity: O(2^n) -- single pass over state vector.
    /// Vectorizable: all amplitudes are independent (no swaps).
    ///
    /// Hardware equivalent: CNOT(i,j) + Rz(theta, j) + CNOT(i,j) = 3 gates.
    pub fn rzz(&mut self, theta: f64, qubit_i: usize, qubit_j: usize) {
        let phase_same = Complex64::from_polar(1.0, -theta / 2.0);
        let phase_diff = Complex64::from_polar(1.0, theta / 2.0);

        let mask_i = 1_usize << qubit_i;
        let mask_j = 1_usize << qubit_j;

        for k in 0..self.amplitudes.len() {
            let bit_i = (k & mask_i) >> qubit_i;
            let bit_j = (k & mask_j) >> qubit_j;
            let parity = bit_i ^ bit_j;

            if parity == 0 {
                self.amplitudes[k] *= phase_same;
            } else {
                self.amplitudes[k] *= phase_diff;
            }
        }
    }
}
```

**Vectorization opportunity**: The inner loop is a streaming operation over the amplitude
array with no data dependencies between iterations. This is ideal for SIMD vectorization
(AVX-512 can process 8 complex64 values per instruction) and parallelization across
cores.

### 2. Mixing Operator

The mixing operator applies Rx(2*beta) to each qubit:

```
Rx(2*beta) = exp(-i * beta * X) = [[cos(beta), -i*sin(beta)],
                                     [-i*sin(beta), cos(beta)]]
```

This uses the standard single-qubit gate application from the simulator core:

```rust
impl QuantumState {
    /// Apply the QAOA mixing operator: Rx(2*beta) on each qubit.
    ///
    /// Complexity: O(n * 2^n) for n qubits.
    pub fn qaoa_mixing(&mut self, beta: f64) {
        for qubit in 0..self.n_qubits {
            self.rx(2.0 * beta, qubit);
        }
    }
}
```

### 3. QAOA Circuit Construction

A convenience function builds the full QAOA circuit from a graph and parameters:

```rust
/// A graph represented as an edge list with optional weights.
pub struct Graph {
    /// Number of vertices
    pub n_vertices: usize,
    /// Edges: (vertex_i, vertex_j, weight)
    pub edges: Vec<(usize, usize, f64)>,
}

impl Graph {
    /// Construct from adjacency list.
    pub fn from_adjacency_list(adj: &[Vec<usize>]) -> Self;

    /// Construct from edge list (unweighted, weight = 1.0).
    pub fn from_edge_list(n_vertices: usize, edges: &[(usize, usize)]) -> Self;

    /// Load from ruVector graph query result.
    pub fn from_ruvector_query(result: &GraphQueryResult) -> Self;
}

/// QAOA configuration.
pub struct QaoaConfig {
    /// Graph defining the MaxCut instance
    pub graph: Graph,
    /// QAOA depth (number of layers)
    pub p: usize,
    /// Gamma parameters (phase separation angles), length = p
    pub gammas: Vec<f64>,
    /// Beta parameters (mixing angles), length = p
    pub betas: Vec<f64>,
}

/// Build and simulate a QAOA circuit for MaxCut.
///
/// Circuit structure for depth p:
///   1. Initialize |+>^n (Hadamard on all qubits)
///   2. For layer l = 1..p:
///      a. Phase separation: Rzz(gamma_l, i, j) for each edge (i,j)
///      b. Mixing: Rx(2*beta_l) on each qubit
///   3. Return final state
pub fn build_qaoa_circuit(config: &QaoaConfig) -> QuantumState {
    let n = config.graph.n_vertices;
    let mut state = QuantumState::new(n);

    // Step 1: Initialize uniform superposition
    state.hadamard_all();

    // Step 2: Alternating phase separation and mixing layers
    for layer in 0..config.p {
        let gamma = config.gammas[layer];
        let beta = config.betas[layer];

        // Phase separation: apply Rzz for each edge
        for &(i, j, weight) in &config.graph.edges {
            state.rzz(gamma * weight, i, j);
        }

        // Mixing: Rx(2*beta) on each qubit
        state.qaoa_mixing(beta);
    }

    state
}
```

**Pseudocode for the complete QAOA MaxCut solver**:

```rust
pub fn qaoa_maxcut(
    graph: &Graph,
    p: usize,
    optimizer: &mut dyn ClassicalOptimizer,
    config: &QaoaOptConfig,
) -> QaoaResult {
    let n_params = 2 * p; // p gammas + p betas
    optimizer.initialize(n_params);

    let mut params = config.initial_params.clone()
        .unwrap_or_else(|| {
            // Standard initialization: gamma in [0, pi], beta in [0, pi/2]
            let mut p_init = vec![0.0; n_params];
            for i in 0..p {
                p_init[i] = 0.5;          // gamma_i
                p_init[p + i] = 0.25;     // beta_i
            }
            p_init
        });

    let mut best_cost = f64::NEG_INFINITY;
    let mut best_params = params.clone();
    let mut history = Vec::new();

    for iteration in 0..config.max_iterations {
        let gammas = params[..p].to_vec();
        let betas = params[p..].to_vec();

        // Build and simulate circuit
        let qaoa_config = QaoaConfig {
            graph: graph.clone(),
            p,
            gammas,
            betas,
        };
        let state = build_qaoa_circuit(&qaoa_config);

        // Evaluate MaxCut cost function
        let cost = maxcut_expectation(&state, graph);

        if cost > best_cost {
            best_cost = cost;
            best_params = params.clone();
        }

        // Gradient computation (parameter-shift rule, same as VQE)
        let grad = if optimizer.needs_gradient() {
            Some(qaoa_gradient(graph, p, &params))
        } else {
            None
        };

        history.push(QaoaIteration { iteration, cost, params: params.clone() });

        let result = optimizer.step(&params, -cost, grad.as_deref());
        // Note: negate cost because optimizer minimizes
        params = result.new_params;

        if result.converged {
            break;
        }
    }

    // Sample the final state to get candidate cuts
    let final_state = build_qaoa_circuit(&QaoaConfig {
        graph: graph.clone(),
        p,
        gammas: best_params[..p].to_vec(),
        betas: best_params[p..].to_vec(),
    });
    let best_cut = sample_maxcut(&final_state, graph, config.sample_shots);

    QaoaResult {
        best_cost,
        best_params,
        best_cut,
        iterations: history.len(),
        history,
        approximation_ratio: best_cost / graph.max_cut_upper_bound(),
    }
}
```

### 4. Cost Function Evaluation

The MaxCut cost function in Pauli operator form is:

```
C = sum_{(i,j) in E} w_{ij} * (1 - Z_i Z_j) / 2
```

This reuses the PauliSum expectation API from ADR-QE-005:

```rust
/// Compute the MaxCut cost as the expectation value of the cost Hamiltonian.
///
/// C = sum_{(i,j) in E} w_ij * (1 - Z_i Z_j) / 2
///   = sum_{(i,j) in E} w_ij/2 - sum_{(i,j) in E} w_ij/2 * Z_i Z_j
///   = const - sum_{(i,j)} w_ij/2 * <Z_i Z_j>
///
/// Each Z_i Z_j expectation is computed via the efficient diagonal trick:
/// <psi| Z_i Z_j |psi> = sum_k |amp_k|^2 * (-1)^{bit_i(k) XOR bit_j(k)}
pub fn maxcut_expectation(state: &QuantumState, graph: &Graph) -> f64 {
    let mut cost = 0.0;

    for &(i, j, weight) in &graph.edges {
        let mask_i = 1_usize << i;
        let mask_j = 1_usize << j;

        let mut zz_expectation = 0.0;
        for k in 0..state.amplitudes.len() {
            let bit_i = (k & mask_i) >> i;
            let bit_j = (k & mask_j) >> j;
            let parity = bit_i ^ bit_j;
            let sign = 1.0 - 2.0 * parity as f64; // +1 if same, -1 if different
            zz_expectation += state.amplitudes[k].norm_sqr() * sign;
        }

        cost += weight * (1.0 - zz_expectation) / 2.0;
    }

    cost
}
```

**Optimization**: Since Z_i Z_j is diagonal in the computational basis, the expectation
reduces to a weighted sum over probabilities. No amplitude swapping is needed, and the
computation is embarrassingly parallel.

### 5. Sampling Mode

In addition to exact expectation values, we support sampling the final state to
obtain candidate cuts:

```rust
/// Sample the QAOA state to find candidate MaxCut solutions.
///
/// Returns the best cut found across `shots` samples.
pub fn sample_maxcut(
    state: &QuantumState,
    graph: &Graph,
    shots: usize,
) -> MaxCutSolution {
    let probabilities: Vec<f64> = state.amplitudes.iter()
        .map(|a| a.norm_sqr())
        .collect();

    let mut best_cut_value = 0.0;
    let mut best_bitstring = 0_usize;
    let mut rng = thread_rng();

    for _ in 0..shots {
        // Sample from probability distribution
        let sample = sample_from_distribution(&probabilities, &mut rng);

        // Evaluate cut value for this bitstring
        let cut_value = evaluate_cut(sample, graph);

        if cut_value > best_cut_value {
            best_cut_value = cut_value;
            best_bitstring = sample;
        }
    }

    MaxCutSolution {
        partition: best_bitstring,
        cut_value: best_cut_value,
        set_s: (0..graph.n_vertices)
            .filter(|&v| (best_bitstring >> v) & 1 == 1)
            .collect(),
        set_s_complement: (0..graph.n_vertices)
            .filter(|&v| (best_bitstring >> v) & 1 == 0)
            .collect(),
    }
}
```

### 6. Graph Interface

Three input modes cover common use cases:

```rust
impl Graph {
    /// From adjacency list (unweighted).
    ///
    /// Example: adj[0] = [1, 3] means vertex 0 connects to 1 and 3.
    pub fn from_adjacency_list(adj: &[Vec<usize>]) -> Self {
        let n = adj.len();
        let mut edges = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for (u, neighbors) in adj.iter().enumerate() {
            for &v in neighbors {
                let edge = if u < v { (u, v) } else { (v, u) };
                if seen.insert(edge) {
                    edges.push((edge.0, edge.1, 1.0));
                }
            }
        }

        Self { n_vertices: n, edges }
    }

    /// From edge list with uniform weight.
    pub fn from_edge_list(n_vertices: usize, edge_list: &[(usize, usize)]) -> Self {
        Self {
            n_vertices,
            edges: edge_list.iter().map(|&(u, v)| (u, v, 1.0)).collect(),
        }
    }

    /// From ruVector graph database query result.
    ///
    /// Enables QAOA MaxCut on graphs stored in ruvector-graph.
    pub fn from_ruvector_query(result: &GraphQueryResult) -> Self {
        // Convert ruvector-graph nodes and edges to QAOA format
        // Vertex IDs are remapped to contiguous 0..n range
        todo!()
    }
}
```

### 7. Tensor Network Optimization for Sparse Graphs

For sparse or planar graphs, the QAOA state can be represented more efficiently using
tensor network contraction. The key insight is that QAOA circuits have a structure
dictated by the graph topology:

```
Tensor Network View of QAOA:

  Qubit 0: ──[H]──[Rzz(0,1)]──[Rzz(0,3)]──[Rx]── ...
  Qubit 1: ──[H]──[Rzz(0,1)]──[Rzz(1,2)]──[Rx]── ...
  Qubit 2: ──[H]──[Rzz(1,2)]──[Rzz(2,3)]──[Rx]── ...
  Qubit 3: ──[H]──[Rzz(0,3)]──[Rzz(2,3)]──[Rx]── ...

For a planar graph with treewidth w, tensor contraction costs O(2^w * poly(n))
instead of O(2^n). For many practical graphs, w << n.
```

```rust
/// Detect graph treewidth and decide simulation strategy.
pub fn select_simulation_strategy(graph: &Graph) -> SimulationStrategy {
    let treewidth = estimate_treewidth(graph);
    let n = graph.n_vertices;

    if treewidth <= 20 && n > 24 {
        // Tensor network contraction is cheaper than full state vector
        SimulationStrategy::TensorNetwork {
            contraction_order: compute_contraction_order(graph),
            estimated_cost: (1 << treewidth) * n * n,
        }
    } else {
        SimulationStrategy::StateVector {
            estimated_cost: 1 << n,
        }
    }
}

pub enum SimulationStrategy {
    StateVector { estimated_cost: usize },
    TensorNetwork {
        contraction_order: Vec<ContractionStep>,
        estimated_cost: usize,
    },
}
```

### 8. Performance Analysis

#### Gate Counts and Timing

For a graph with n vertices, m edges, and QAOA depth p:

| Operation | Gate Count per Layer | Total Gates (p layers) |
|-----------|---------------------|----------------------|
| Phase separation (Rzz) | m | p * m |
| Mixing (Rx) | n | p * n |
| **Total per layer** | **m + n** | **p * (m + n)** |

**Benchmark estimates**:

| Configuration | n | m | p | Total Gates | Estimated Time |
|---------------|---|---|---|-------------|---------------|
| Small triangle | 3 | 3 | 1 | 6 | <0.01ms |
| Petersen graph | 10 | 15 | 3 | 75 | <0.1ms |
| Random d-reg (d=3) | 10 | 15 | 5 | 125 | <0.5ms |
| Grid 4x5 | 20 | 31 | 3 | 189 | ~50ms |
| Grid 4x5 | 20 | 31 | 5 | 315 | ~100ms |
| Random d-reg (d=4) | 20 | 40 | 5 | 400 | ~200ms |
| Dense (complete) | 20 | 190 | 3 | 630 | ~300ms |
| Sparse large | 24 | 36 | 3 | 216 | ~5s |
| Dense large | 24 | 276 | 5 | 1500 | ~30s |

**Memory requirements**:

| Qubits | State Vector Size | Memory |
|--------|------------------|--------|
| 10 | 1,024 | 16 KB |
| 16 | 65,536 | 1 MB |
| 20 | 1,048,576 | 16 MB |
| 24 | 16,777,216 | 256 MB |
| 28 | 268,435,456 | 4 GB |

### 9. Integration with ruvector-graph

The connection to ruVector's graph database enables a powerful workflow:

```
┌─────────────────────────────────────────────────────────────────────┐
│                  QAOA MaxCut Pipeline                                 │
│                                                                       │
│  ┌──────────────┐     ┌────────────────┐     ┌──────────────────┐   │
│  │ ruvector-graph│     │  QAOA Engine   │     │  Result Store    │   │
│  │              │     │                │     │                  │   │
│  │  Query:      │────>│  Build circuit │────>│  Optimal cut     │   │
│  │  "find all   │     │  Optimize      │     │  Partition       │   │
│  │   connected  │     │  Sample        │     │  Approximation   │   │
│  │   subgraphs  │     │                │     │  ratio           │   │
│  │   of size k" │     │                │     │                  │   │
│  └──────────────┘     └────────────────┘     └──────────────────┘   │
│                                                                       │
│  Data Flow:                                                           │
│  1. Agent queries ruvector-graph for subgraph                        │
│  2. Graph converted to QAOA format via Graph::from_ruvector_query()  │
│  3. QAOA optimizer runs with configurable depth p                     │
│  4. Results stored in ruVector memory for pattern learning            │
│  5. Agent uses learned patterns to choose p and initial parameters    │
└─────────────────────────────────────────────────────────────────────┘
```

The ruvector-mincut integration is particularly relevant: the existing
`SubpolynomialMinCut` algorithm (El-Hayek/Henzinger/Li, O(n^{o(1)}) amortized) provides
exact min-cut values that serve as a lower bound for MaxCut verification. QAOA solutions
can be validated against this classical baseline.

---

## Consequences

### Benefits

1. **Native Rzz gate** via direct amplitude manipulation avoids CNOT decomposition,
   yielding a simpler and faster phase separation implementation
2. **PauliSum expectation API reuse** from ADR-QE-005 provides a unified interface for
   all variational algorithms (VQE, QAOA, and future extensions)
3. **Graph interface flexibility** supports adjacency lists, edge lists, and ruVector
   graph queries, covering the most common input formats
4. **Tensor network fallback** for low-treewidth graphs extends QAOA to larger problem
   instances than pure state vector simulation allows
5. **ruvector-graph integration** enables a seamless pipeline from graph storage to
   quantum optimization to result analysis

### Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| QAOA at low depth p gives poor approximation ratios | High | Medium | Support high-p QAOA, classical warm-starting |
| Treewidth estimation is NP-hard in general | Medium | Low | Use heuristic upper bounds (min-degree, greedy) |
| Parameter landscape has many local minima | Medium | Medium | Multi-start optimization, INTERP initialization |
| Large dense graphs exhaust memory | Medium | High | Tensor network fallback, graph coarsening |

### Trade-offs

| Decision | Advantage | Disadvantage |
|----------|-----------|--------------|
| Direct Rzz over CNOT decomposition | Simpler, faster | Not a one-to-one hardware circuit mapping |
| Exact expectation over sampling | No statistical noise | Does not model real hardware shot noise |
| Automatic strategy selection | Transparent to user | Additional complexity in simulation backend |
| Integrated graph interface | Seamless workflow | Coupling to ruvector-graph API |

---

## References

- Farhi, E., Goldstone, J., Gutmann, S. "A Quantum Approximate Optimization Algorithm." arXiv:1411.4028 (2014)
- Hadfield, S. et al. "From the Quantum Approximate Optimization Algorithm to a Quantum Alternating Operator Ansatz." Algorithms 12, 34 (2019)
- Zhou, L. et al. "Quantum Approximate Optimization Algorithm: Performance, Mechanism, and Implementation on Near-Term Devices." Physical Review X 10, 021067 (2020)
- Guerreschi, G.G., Matsuura, A.Y. "QAOA for Max-Cut requires hundreds of qubits for quantum speed-up." Scientific Reports 9, 6903 (2019)
- ADR-001: ruQu Architecture - Classical Nervous System for Quantum Machines
- ADR-QE-005: VQE Algorithm Support (shared parameterized circuit and optimizer infrastructure)
- ADR-QE-006: Grover's Search Implementation (quantum state manipulation primitives)
- ruvector-mincut: `crates/ruvector-mincut/` - El-Hayek/Henzinger/Li subpolynomial min-cut
- ruvector-graph: graph database integration for sourcing MaxCut instances
