# ADR-QE-009: Tensor Network Evaluation Mode

**Status**: Proposed
**Date**: 2026-02-06
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board

---

## Context

Full state-vector simulation stores all 2^n complex amplitudes explicitly, yielding
O(2^n) memory and O(G * 2^n) time for G gates. At n=30 this is 16 GiB; at n=40 it
exceeds 16 TiB. Many practically interesting circuits, however, contain limited
entanglement:

| Circuit family | Entanglement structure | Treewidth |
|---|---|---|
| Shallow QAOA on sparse graphs | Bounded by graph degree | Low (often < 20) |
| Separate-register circuits | Disjoint qubit subsets | Sum of sub-widths |
| Near-Clifford circuits | Stabilizer + few T gates | Depends on T count |
| 1D brickwork (finite depth) | Area-law entanglement | O(depth) |
| Random deep circuits (all-to-all) | Volume-law entanglement | O(n) -- no gain |

For the first four families, tensor network (TN) methods can trade increased
computation for drastically reduced memory by representing each gate as a tensor and
contracting the resulting network in an optimized order. The contraction cost scales
exponentially in the *treewidth* of the circuit's line graph rather than in the total
qubit count.

QuantRS2 (the Rust quantum simulation reference) demonstrated tensor network
contraction for circuits up to 60 qubits on commodity hardware when treewidth
remained below ~25. ruVector's existing `ruvector-mincut` crate already solves graph
partitioning problems that are structurally identical to contraction-order
optimization, providing a natural integration point.

The ruQu engine needs this capability to support:

1. Surface code simulations at distance d >= 7 (49+ data qubits) for decoder
   validation, where the syndrome extraction circuit is shallow and geometrically
   local.
2. Variational algorithm prototyping (VQE, QAOA) on graphs larger than 30 nodes.
3. Hybrid workflows where part of the circuit is simulated via state vector and part
   via tensor contraction.

## Decision

### 1. Feature-Gated Backend

Tensor network evaluation is implemented as an optional backend behind the
`tensor-network` feature flag in `ruqu-core`:

```toml
# ruqu-core/Cargo.toml
[features]
default = ["state-vector"]
state-vector = []
tensor-network = ["dep:ndarray", "dep:petgraph"]
all-backends = ["state-vector", "tensor-network"]
```

When both backends are compiled in, the engine selects the backend at runtime based
on circuit analysis (see Section 4 below).

### 2. Tensor Representation

Every gate becomes a tensor connecting the qubit wire indices it acts on:

| Gate type | Tensor rank | Shape | Example |
|---|---|---|---|
| Single-qubit (H, X, Rz, ...) | 2 | [2, 2] | Input wire -> output wire |
| Two-qubit (CNOT, CZ, ...) | 4 | [2, 2, 2, 2] | Two input wires -> two output wires |
| Three-qubit (Toffoli) | 6 | [2, 2, 2, 2, 2, 2] | Three input -> three output |
| Measurement projector | 2 | [2, 2] | Diagonal in computational basis |
| Initial state |0> | 1 | [2] | Single output wire |

The circuit is converted into a tensor network graph where:
- Each tensor is a node.
- Each shared index (qubit wire between consecutive gates) is an edge.
- Open indices represent initial states and final measurement outcomes.

```
  |0>---[H]---[CNOT_ctrl]---[Rz]---<meas>
                  |
  |0>-----------[CNOT_tgt]---------<meas>
```

Becomes:

```
  Node: init_0 (rank 1)
    |
  Node: H_0 (rank 2)
    |
  Node: CNOT_01 (rank 4)
   / \
  |   Node: Rz_0 (rank 2)
  |     |
  |   Node: meas_0 (rank 2)
  |
  Node: init_1 (rank 1)
    ... (connected via CNOT shared index)
  Node: meas_1 (rank 2)
```

### 3. Contraction Strategy

Contraction order determines whether the computation is tractable. The cost of
contracting two tensors is the product of the dimensions of all indices involved.
Finding the optimal contraction order is NP-hard (equivalent to finding minimum
treewidth), so we use heuristics.

#### Contraction Path Optimization Pseudocode

```
function find_contraction_path(tensor_network: TN) -> ContractionPath:
    // Phase 1: Simplify the network
    apply_trivial_contractions(tensor_network)  // rank-1 tensors, diagonal pairs

    // Phase 2: Detect community structure
    communities = detect_communities(tensor_network.graph)

    // Phase 3: Contract within communities first (small subproblems)
    intra_paths = []
    for community in communities:
        subgraph = tensor_network.subgraph(community)
        if subgraph.num_tensors <= 20:
            // Exact dynamic programming for small subgraphs
            path = optimal_einsum_dp(subgraph)
        else:
            // Greedy with lookahead for larger subgraphs
            path = greedy_with_lookahead(subgraph, lookahead=2)
        intra_paths.append(path)

    // Phase 4: Contract inter-community edges
    // Each community is now a single large tensor
    meta_graph = contract_communities(tensor_network, intra_paths)
    inter_path = greedy_with_lookahead(meta_graph, lookahead=3)

    // Phase 5: Compose the full path
    return compose_paths(intra_paths, inter_path)


function greedy_with_lookahead(tn: TN, lookahead: int) -> Path:
    path = []
    remaining = tn.clone()

    while remaining.num_tensors > 1:
        best_cost = INFINITY
        best_pair = None

        // Evaluate all candidate contractions
        for (i, j) in remaining.candidate_pairs():
            cost = contraction_cost(remaining, i, j)

            // Lookahead: estimate cost of subsequent contractions
            if lookahead > 0:
                simulated = remaining.simulate_contraction(i, j)
                future_cost = estimate_future_cost(simulated, lookahead - 1)
                cost += future_cost * DISCOUNT_FACTOR

            if cost < best_cost:
                best_cost = cost
                best_pair = (i, j)

        path.append(best_pair)
        remaining.contract(best_pair)

    return path
```

#### Community Detection via ruvector-mincut

The `ruvector-mincut` crate provides graph partitioning that is directly applicable
to contraction ordering:

```rust
use ruvector_mincut::{partition, PartitionConfig};

fn partition_tensor_network(tn: &TensorNetwork) -> Vec<Vec<TensorId>> {
    let graph = tn.to_adjacency_graph();
    let config = PartitionConfig {
        num_partitions: estimate_optimal_partitions(tn),
        balance_factor: 1.1,  // Allow 10% imbalance
        minimize: Objective::EdgeCut,  // Minimize inter-partition wires
    };
    partition(&graph, &config)
}
```

The edge cut directly corresponds to the bond dimension of the inter-community
contraction, so minimizing edge cut minimizes the most expensive contraction step.

### 4. MPS (Matrix Product State) Mode

For circuits with 1D-like connectivity (nearest-neighbor gates on a line), a Matrix
Product State representation is more efficient than general tensor contraction.

```
    A[1] -- A[2] -- A[3] -- ... -- A[n]
     |       |       |               |
   phys_1  phys_2  phys_3         phys_n
```

Each site tensor A[i] has shape `[bond_left, physical, bond_right]` where:
- `physical` = 2 (qubit dimension)
- `bond_left`, `bond_right` = bond dimension chi

| Bond dimension (chi) | Memory per site | Total memory (n qubits) | Approximation |
|---|---|---|---|
| 1 | 16 bytes | 16n bytes | Product state only |
| 16 | 4 KiB | 4n KiB | Low entanglement |
| 64 | 64 KiB | 64n KiB | Moderate entanglement |
| 256 | 1 MiB | n MiB | High entanglement |
| 1024 | 16 MiB | 16n MiB | Near exact for many circuits |

**Truncation policy**: After each two-qubit gate, perform SVD on the updated bond.
If the bond dimension exceeds `chi_max`, truncate the smallest singular values.
Track the total discarded weight (sum of squared discarded singular values) as a
fidelity estimate:

```rust
pub struct MpsConfig {
    /// Maximum bond dimension. Truncation occurs above this.
    pub chi_max: usize,
    /// Minimum singular value to retain (relative to largest).
    pub svd_cutoff: f64,
    /// Accumulated truncation error (updated during simulation).
    pub fidelity_estimate: f64,
}

impl Default for MpsConfig {
    fn default() -> Self {
        Self {
            chi_max: 256,
            svd_cutoff: 1e-12,
            fidelity_estimate: 1.0,
        }
    }
}
```

### 5. Automatic Mode Selection

The engine analyzes the circuit before execution to recommend a backend:

```rust
pub enum RecommendedBackend {
    StateVector { reason: &'static str },
    TensorNetwork { estimated_treewidth: usize, reason: &'static str },
    Mps { estimated_max_bond: usize, reason: &'static str },
}

pub fn recommend_backend(circuit: &QuantumCircuit) -> RecommendedBackend {
    let n = circuit.num_qubits();
    let depth = circuit.depth();
    let connectivity = circuit.connectivity_graph();

    // Rule 1: Small circuits always use state vector
    if n <= 20 {
        return RecommendedBackend::StateVector {
            reason: "Small circuit; state vector is fastest below 20 qubits",
        };
    }

    // Rule 2: Check for 1D connectivity (MPS candidate)
    if connectivity.max_degree() <= 2 && connectivity.is_path_graph() {
        let estimated_bond = 2_usize.pow(depth.min(20) as u32);
        return RecommendedBackend::Mps {
            estimated_max_bond: estimated_bond,
            reason: "1D nearest-neighbor connectivity detected",
        };
    }

    // Rule 3: Estimate treewidth for general TN
    let estimated_tw = estimate_treewidth(&connectivity, depth);
    if estimated_tw < 25 && n > 25 {
        return RecommendedBackend::TensorNetwork {
            estimated_treewidth: estimated_tw,
            reason: "Low treewidth relative to qubit count",
        };
    }

    // Rule 4: Check memory feasibility for state vector
    let sv_memory = 16 * (1_usize << n);  // bytes
    let available = estimate_available_memory();
    if sv_memory > available {
        // Force TN even if treewidth is high -- at least it has a chance
        return RecommendedBackend::TensorNetwork {
            estimated_treewidth: estimated_tw,
            reason: "State vector exceeds available memory; TN is only option",
        };
    }

    RecommendedBackend::StateVector {
        reason: "High treewidth circuit; state vector is more efficient",
    }
}
```

### 6. When Tensor Networks Win vs Lose

**Tensor networks win when:**

| Scenario | Why TN wins | Example |
|---|---|---|
| Shallow circuits on many qubits | Treewidth ~ depth, not n | 50-qubit depth-4 QAOA |
| Sparse graph connectivity | Low treewidth from graph structure | MaxCut on 3-regular graph |
| Separate registers | Independent contractions | n/2 Bell pairs |
| Near-Clifford | Stabilizer + few non-Clifford gates | Clifford + 5 T gates |
| Amplitude computation | Contract to single output, not full state | Sampling one bitstring |

**Tensor networks lose when:**

| Scenario | Why TN loses | Fallback |
|---|---|---|
| Deep random circuits | Treewidth ~ n | State vector (if n <= 30) |
| All-to-all connectivity | No structure to exploit | State vector |
| Full state tomography needed | Must contract once per amplitude | State vector |
| Very small circuits (n < 20) | Overhead exceeds state vector | State vector |
| High-fidelity MPS needed | Bond dimension grows exponentially | State vector or exact TN |

### 7. Example: 50-Qubit Shallow QAOA

Consider QAOA depth p=1 on a 50-node 3-regular graph:

```
Circuit structure:
  - 50 qubits, initialized to |+>
  - 75 ZZ gates (one per edge), parameterized by gamma
  - 50 Rx gates, parameterized by beta
  - Total: 125 + 50 = 175 gates
  - Circuit depth: 4 (H layer, ZZ layer (3-colorable), Rx layer, measure)

Graph treewidth of 3-regular graph: typically 8-15

Tensor network contraction:
  - Community detection finds ~5-8 communities of 6-10 nodes
  - Intra-community contraction: O(2^10) ~ 1024 per community
  - Inter-community bonds: ~15 edges cut
  - Effective contraction complexity: O(2^15) = 32768
  - Compare to state vector: O(2^50) = 1.1 * 10^15

Memory comparison:
  - State vector: 2^50 * 16 bytes = 16 PiB (impossible)
  - Tensor network: ~100 MiB working memory
  - Speedup factor: practically infinite (feasible vs infeasible)
```

```
Contraction Diagram (simplified):

  Community A        Community B        Community C
  [q0-q9]           [q10-q19]          [q20-q29]
     |                  |                   |
     +--- bond=2^3 ----+---- bond=2^4 -----+
                        |
  Community D        Community E
  [q30-q39]          [q40-q49]
     |                  |
     +--- bond=2^3 ----+

  Peak intermediate tensor: 2^15 elements = 512 KiB
```

### 8. Integration with State Vector Backend

Both backends implement the same trait:

```rust
pub trait SimulationBackend {
    /// Execute the circuit and return measurement results.
    fn execute(
        &self,
        circuit: &QuantumCircuit,
        shots: usize,
        config: &SimulationConfig,
    ) -> Result<SimulationResult, SimulationError>;

    /// Compute expectation value of an observable.
    fn expectation_value(
        &self,
        circuit: &QuantumCircuit,
        observable: &Observable,
        config: &SimulationConfig,
    ) -> Result<f64, SimulationError>;

    /// Return the backend name for logging.
    fn name(&self) -> &'static str;
}
```

Users interact through `QuantumCircuit` and never need to know which backend is
active:

```rust
let circuit = QuantumCircuit::new(50)
    .h_all()
    .append_qaoa_layer(graph, gamma, beta)
    .measure_all();

// Automatic backend selection
let result = ruqu::execute(&circuit, 1000)?;
// -> Internally selects TensorNetwork backend due to n=50, low treewidth

// Or explicit backend override
let result = ruqu::execute_with_backend(
    &circuit,
    1000,
    Backend::TensorNetwork(TnConfig::default()),
)?;
```

### 9. Future: ruvector-mincut Integration for Contraction Ordering

The `ruvector-mincut` crate currently solves balanced graph partitioning for vector
index sharding. The same algorithm directly applies to tensor network contraction
ordering via the following correspondence:

| Graph partitioning concept | TN contraction concept |
|---|---|
| Vertex | Tensor |
| Edge weight | Bond dimension (log2) |
| Partition | Contraction subtree |
| Edge cut | Inter-partition bond cost |
| Balanced partition | Balanced contraction tree |

Phase 1 (this ADR): Use `ruvector-mincut` for community detection in contraction
path optimization.

Phase 2 (future): Extend `ruvector-mincut` with hypergraph partitioning for
multi-index tensor contractions, enabling handling of higher-order tensor networks
(e.g., PEPS for 2D circuits).

## Consequences

### Positive

1. **Dramatically expanded qubit range**: Shallow circuits on 40-60 qubits become
   tractable on commodity hardware.
2. **Surface code simulation**: Distance-7 surface codes (49 data + 48 ancilla = 97
   qubits) can be simulated for decoder validation using MPS (the circuit is
   geometrically local).
3. **Unified interface**: Users write circuits once; backend selection is automatic.
4. **Synergy with ruvector-mincut**: Leverages existing graph partitioning
   investment.
5. **Complementary to state vector**: Each backend covers the other's weakness.

### Negative

1. **Implementation complexity**: Tensor contraction, SVD truncation, and path
   optimization are non-trivial to implement correctly and efficiently.
2. **Approximation risk**: MPS truncation introduces controlled but nonzero error.
   Users must understand fidelity estimates.
3. **Compilation time**: The `ndarray` and `petgraph` dependencies add to compile
   time when the feature is enabled.
4. **Testing surface**: Two backends doubles the testing matrix for correctness
   validation.
5. **Performance unpredictability**: Contraction cost depends on circuit structure
   in ways that are hard to predict without running the path optimizer.

### Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Path optimizer finds poor ordering | Medium | High cost | Multiple heuristics + timeout fallback to greedy |
| MPS fidelity silently degrades | Medium | Incorrect results | Track discarded weight; warn if fidelity < 0.99 |
| Feature interaction bugs | Low | Incorrect results | Shared test suite: both backends must agree on small circuits |
| Memory spike during contraction | Medium | OOM | Pre-estimate peak intermediate tensor size; abort if too large |

## References

- QuantRS2 tensor network implementation: internal reference
- Markov & Shi, "Simulating Quantum Computation by Contracting Tensor Networks" (2008)
- Gray & Kourtis, "Hyper-optimized tensor network contraction" (2021) -- cotengra
- Schollwock, "The density-matrix renormalization group in the age of matrix product states" (2011)
- ADR-QE-001: Core Engine Architecture (state vector backend)
- ADR-QE-005: WASM Compilation Target
- `ruvector-mincut` crate documentation
- ADR-014: Coherence Engine (graph partitioning reuse)
