# Axis 4: Quantum Graph Attention

**Document:** 24 of 30
**Series:** Graph Transformers: 2026-2036 and Beyond
**Last Updated:** 2026-02-25
**Status:** Research Prospectus

---

## 1. Problem Statement

Quantum computing offers the prospect of exponential speedups for certain graph problems: graph isomorphism, maximum clique, graph coloring, and shortest paths all have quantum algorithms with provable advantages. The quantum axis asks: can we build graph attention mechanisms that run on quantum hardware and achieve genuine quantum advantage?

This is distinct from "quantum-inspired" classical algorithms (covered in Doc 09). Here we mean actual quantum circuits on actual quantum hardware.

### 1.1 The Quantum Advantage Landscape for Graphs

| Problem | Best Classical | Best Quantum | Speedup | Status (2026) |
|---------|---------------|-------------|---------|---------------|
| Unstructured search | O(n) | O(sqrt(n)) | Quadratic | Proven (Grover) |
| Graph isomorphism | quasi-polynomial | O(n^{1/3}) (conj.) | Polynomial | Conjectured |
| Max-Cut | NP-hard | QAOA approx | Unknown | Experimental |
| Shortest path | O(n^2) | O(n^{3/2}) | Quadratic | Proven (quantum walk) |
| PageRank | O(n * |E|) | O(sqrt(n) * polylog) | Quadratic+ | Proven |
| Spectral gap estimation | O(n^3) | O(polylog(n)) | Exponential | Proven (QPE) |

### 1.2 RuVector Baseline

- **`ruQu`**: Surface codes, syndrome extraction, adaptive decoding, logical qubits, stabilizer circuits
- **`ruqu-core`**: Quantum circuit primitives, gate decomposition
- **`ruqu-algorithms`**: Quantum algorithmic building blocks
- **`ruqu-exotic`**: Exotic quantum codes (color codes, topological codes)
- **`ruvector-attention`**: 18+ classical attention mechanisms as starting points
- **`ruvector-mincut-gated-transformer`**: Spectral methods that connect to quantum eigenvalue problems

---

## 2. Quantum Graph Attention Mechanisms

### 2.1 Amplitude-Encoded Graph Attention

**Core idea.** Encode graph features as quantum amplitudes. Attention weights computed via quantum interference.

**Setup:**
- n nodes, d-dimensional features
- Feature matrix X in R^{n x d}
- Encode row i as quantum state: |psi_i> = sum_j X[i,j] |j> / ||X[i]||

**Quantum attention circuit:**

```
|0>^{log n} ─┬─ H^{log n} ─── Query Oracle ──── QFT^{-1} ──── Measure
              │
|0>^{log n} ─┘─ H^{log n} ─── Key Oracle ────── QFT^{-1} ──── Measure
              │
|0>^{log d} ─┘─ H^{log d} ─── Value Oracle ──── QFT^{-1} ──── Measure

Where:
  Query Oracle: |i>|0> -> |i>|q_i>  (prepares query vectors)
  Key Oracle:   |j>|0> -> |j>|k_j>  (prepares key vectors)
  Value Oracle: |j>|0> -> |j>|v_j>  (prepares value vectors)
```

**Attention computation via SWAP test:**

```
For nodes u, v:
  1. Prepare |q_u> and |k_v>
  2. Apply SWAP test: measures |<q_u|k_v>|^2
  3. This gives attention weight alpha_{uv} = |<q_u|k_v>|^2

For all pairs simultaneously:
  1. Prepare superposition: sum_{u,v} |u>|v>|q_u>|k_v>
  2. Apply controlled-SWAP across query/key registers
  3. Measure ancilla to get attention distribution
```

**Complexity:**
- State preparation: O(n * d) classical, or O(polylog(n*d)) with QRAM
- SWAP test: O(1) per pair, but requires O(sqrt(n)) repetitions for precision
- Total without QRAM: O(n * sqrt(n) * d) -- quadratic speedup over O(n^2 * d) classical
- Total with QRAM: O(sqrt(n) * polylog(n*d)) -- near-quadratic speedup

### 2.2 Quantum Walk Attention

**Core idea.** Replace random walk message passing (standard in GNNs) with quantum walks. Quantum walks explore graphs quadratically faster than classical random walks.

**Continuous-time quantum walk (CTQW):**

```
State evolution: |psi(t)> = exp(-i * A * t) |psi(0)>

where A is the graph adjacency matrix (or Laplacian).
```

**Quantum walk attention weights:**

```
alpha_{uv}(t) = |<v| exp(-i * A * t) |u>|^2
```

This is the probability of the quantum walker starting at u being found at v after time t.

**Key properties of quantum walk attention:**
1. **Quadratic speedup in hitting time**: quantum walker reaches target nodes sqrt faster
2. **Interference effects**: quantum walker can take "all paths simultaneously"
3. **No locality bias**: quantum walk can reach distant nodes in O(sqrt(diameter)) steps
4. **Ballistic transport**: quantum walks on regular graphs spread as t (not sqrt(t) as classical)

**Quantum walk graph transformer layer:**

```
Input: Graph G = (V, E), features X
Output: Attention-weighted features Z

1. Prepare initial state: |psi_u> = |u> tensor |x_u>
2. Evolve under quantum walk: |psi_u(t)> = exp(-i * H * t) |psi_u>
   where H = A tensor I + I tensor H_feature (graph + feature Hamiltonian)
3. Measure in computational basis:
   alpha_{uv} = |<v|psi_u(t)>|^2
4. Aggregate: z_u = sum_v alpha_{uv} * x_v
```

### 2.3 Variational Quantum Graph Transformer (VQGT)

**Core idea.** Use a parameterized quantum circuit (PQC) as a trainable graph transformer layer. The circuit structure reflects the graph structure.

**Circuit design:**

```
Layer l of VQGT:

For each node v:
  R_y(theta_v^l) on qubit v          // Single-qubit rotation (node feature)

For each edge (u,v) in E:
  CNOT(u, v)                          // Entangling gate (graph structure)
  R_z(phi_{uv}^l) on qubit v         // Edge-conditioned rotation
  CNOT(u, v)                          // Unentangle

// This creates a parameterized unitary U(theta, phi) that:
// 1. Respects graph structure (entanglement only along edges)
// 2. Has learnable parameters (theta, phi)
// 3. Computes graph attention implicitly via quantum interference
```

**Training:**
- Forward: Run circuit, measure output qubits
- Loss: Compare measurement statistics to target
- Backward: Parameter shift rule for gradients:
  ```
  dL/d(theta_k) = (L(theta_k + pi/2) - L(theta_k - pi/2)) / 2
  ```

**Complexity:**
- Circuit depth: O(L * |E|) -- linear in edges per layer
- Measurement: O(shots) for statistical estimation
- Training: O(|params| * shots) per gradient step
- Total: O(L * |E| * shots * epochs)

---

## 3. Topological Quantum Error Correction for Graph Transformers

### 3.1 Why QEC Matters for Graph Attention

Quantum graph attention circuits are sensitive to noise. A single bit-flip error can completely corrupt attention weights. For practical quantum graph transformers, we need quantum error correction.

**The connection to `ruQu`:** RuVector's quantum error correction crate already implements surface codes, which are the leading candidates for fault-tolerant quantum computing. The key insight is that surface codes are themselves defined on graphs -- they are graph codes. We can use the same graph structure for both the data and the error correction.

### 3.2 Graph-Structured Quantum Codes

**Idea.** Use the input graph's structure to define the quantum error correcting code. Each node is a logical qubit. The graph's edges define stabilizer operators.

**Construction:**

```
Given graph G = (V, E):

1. Assign one physical qubit to each node and each edge:
   - Node qubits: |n_v> for v in V
   - Edge qubits: |e_{uv}> for (u,v) in E

2. Define stabilizers from graph structure:
   - Vertex stabilizer: X_v = Product of Z operators on edges incident to v
   - Face stabilizer: Z_f = Product of X operators on edges around face f

3. Logical qubits encoded in code space:
   - Number of logical qubits: k = |V| - |E| + |F| (Euler characteristic)
   - Code distance: d = min cycle length in G
```

**Connection to attention:** The syndrome of errors (detected by stabilizer measurements) can be used as an attention signal -- nodes near errors get extra attention for error correction.

### 3.3 Fault-Tolerant Quantum Graph Attention

```
Protocol:

1. ENCODE: Encode graph features into logical qubits using graph code
   |psi_logical> = Encode(X, G)

2. COMPUTE: Apply quantum attention circuit on logical qubits
   - Use transversal gates where possible (automatically fault-tolerant)
   - Use magic state distillation for non-Clifford gates

3. DETECT: Measure syndromes periodically
   syndrome = MeasureStabilizers(|psi>)

4. CORRECT: Decode syndrome and apply corrections
   correction = Decode(syndrome)  // Uses ruQu's adaptive decoder
   |psi_corrected> = ApplyCorrection(|psi>, correction)

5. MEASURE: Extract attention weights from corrected state
   alpha = Measure(|psi_corrected>)
```

**RuVector integration:**

```rust
/// Fault-tolerant quantum graph attention
pub trait FaultTolerantQuantumAttention {
    type Code: QuantumCode;
    type Decoder: SyndromeDecoder;

    /// Encode graph features into quantum error correcting code
    fn encode(
        &self,
        graph: &PropertyGraph,
        features: &Tensor,
    ) -> Result<LogicalState, QECError>;

    /// Apply attention circuit on encoded state
    fn apply_attention(
        &self,
        state: &mut LogicalState,
        params: &AttentionParams,
    ) -> Result<(), QECError>;

    /// Syndrome extraction and error correction
    fn error_correct(
        &self,
        state: &mut LogicalState,
        decoder: &Self::Decoder,
    ) -> Result<CorrectionReport, QECError>;

    /// Measure attention weights from corrected state
    fn measure_attention(
        &self,
        state: &LogicalState,
        shots: usize,
    ) -> Result<AttentionMatrix, QECError>;
}

/// Integration with ruQu crate
pub struct RuQuGraphAttention {
    /// Surface code from ruQu
    code: SurfaceCode,
    /// Adaptive decoder from ruQu
    decoder: AdaptiveDecoder,
    /// Circuit compiler
    compiler: GraphCircuitCompiler,
    /// Noise model
    noise: NoiseModel,
}
```

---

## 4. Quantum Advantage Analysis

### 4.1 Where Quantum Wins

**Problem 1: Global attention on large graphs.**
- Classical: O(n^2) for full attention
- Quantum: O(n * sqrt(n)) via Grover-accelerated attention search
- Speedup: Quadratic

**Problem 2: Spectral attention (eigenvalue-based).**
- Classical: O(n^3) for full eigendecomposition
- Quantum: O(polylog(n)) for quantum phase estimation of graph Laplacian eigenvalues
- Speedup: Exponential (but requires QRAM)

**Problem 3: Graph isomorphism testing in attention.**
- Classical: quasi-polynomial
- Quantum: polynomial (conjectured, related to hidden subgroup problem)
- Speedup: Super-polynomial (conjectured)

**Problem 4: Subgraph pattern matching for attention routing.**
- Classical: O(n^k) for k-node pattern
- Quantum: O(n^{k/2}) via quantum walk search
- Speedup: Quadratic in pattern size

### 4.2 Where Quantum Loses

**Problem A: Sparse graph attention.**
- Classical: O(n * k) for k-sparse attention
- Quantum: O(n * sqrt(k)) -- marginal gain when k is small
- Verdict: Not worth quantum overhead for k < 100

**Problem B: Local neighborhood attention.**
- Classical: O(n * avg_degree) -- already efficient
- Quantum: No advantage for local operations
- Verdict: Quantum advantage requires global or long-range attention

**Problem C: Training (gradient computation).**
- Classical: O(params * n * d) per step
- Quantum: O(params * shots * n) -- shots add constant overhead
- Verdict: Quantum gradient estimation may be slower than classical for moderate model sizes

### 4.3 The QRAM Question

Many quantum speedups for graph attention require QRAM (Quantum Random Access Memory) -- the ability to load classical data into quantum superposition in polylog(n) time.

**Status of QRAM (2026):**
- Theoretical proposals exist (bucket brigade, hybrid approaches)
- No large-scale physical QRAM has been built
- Active research area with conflicting feasibility assessments

**If QRAM is available:** Exponential speedups for spectral graph attention, PageRank attention, and other global operations.

**If QRAM is not available:** Speedups limited to quadratic (Grover-type). Still significant for n > 10^6.

**RuVector strategy:** Design algorithms that degrade gracefully with QRAM availability. Use classical preprocessing to reduce the quantum circuit depth where possible.

---

## 5. Quantum Walk Graph Transformers

### 5.1 Discrete-Time Quantum Walk (DTQW)

```
State: |psi> = sum_{v, c} a_{v,c} |v, c>

where v is position (graph node) and c is coin state (internal degree of freedom)

Update rule:
  1. COIN: Apply coin operator C to internal state
     |v, c> -> |v, C * c>

  2. SHIFT: Move to neighbor based on coin state
     |v, c> -> |neighbor(v, c), c>

One step: S * (I tensor C) * |psi>
```

**DTQW attention:** After t steps, the probability distribution P(v, t) = sum_c |<v,c|psi(t)>|^2 defines attention weights. Unlike classical random walks that converge to the stationary distribution, quantum walks exhibit rich interference patterns that capture graph structure.

### 5.2 Quantum Walk Attention Properties

**Theorem.** For a graph G with spectral gap Delta, the quantum walk mixes in time O(1/Delta), compared to O(1/Delta^2) for classical random walks.

**Corollary.** On expander graphs (large spectral gap), quantum walk attention requires O(1) steps. On poorly-connected graphs, the advantage is quadratic.

**Theorem.** Quantum walk attention can distinguish non-isomorphic regular graphs that 1-WL (Weisfeiler-Leman) graph isomorphism test cannot.

**Implication:** Quantum walk attention is strictly more expressive than message-passing GNNs for graph-level tasks.

### 5.3 Multi-Scale Quantum Walk Attention

```
Short-range attention: t = 1 (single quantum walk step)
  - Captures local neighborhood structure
  - Similar to 1-hop message passing

Medium-range attention: t = O(log n) steps
  - Captures community structure
  - Quantum interference reveals clusters

Long-range attention: t = O(sqrt(n)) steps
  - Captures global graph properties
  - Quantum speedup over classical long-range attention

Multi-scale combination:
  alpha_{uv}^{multi} = sum_t w_t * |<v|U^t|u>|^2
  where w_t are learned scale weights
```

---

## 6. Projections

### 6.1 By 2030

**Likely:**
- Quantum graph attention demonstrated on 50-100 qubit systems
- Variational quantum graph transformers for molecular property prediction
- Hybrid classical-quantum pipelines where quantum handles global attention
- `ruQu` extended with graph-structured quantum codes

**Possible:**
- Quantum walk attention showing measurable advantage over classical on specific tasks
- Fault-tolerant quantum graph attention on error-corrected logical qubits (small scale)
- Quantum graph attention as a cloud API (quantum computing as a service)

**Speculative:**
- QRAM-enabled exponential speedups for graph spectral attention
- Quantum advantage for training graph transformers (not just inference)

### 6.2 By 2033

**Likely:**
- 1000+ logical qubit systems capable of meaningful quantum graph attention
- Standard quantum graph transformer implementations in quantum ML frameworks
- Fault-tolerant quantum attention circuits compiled from high-level descriptions

**Possible:**
- Quantum advantage for graph problems of practical size (10^4+ nodes)
- Topological quantum codes custom-designed for graph transformer error correction
- Quantum graph transformers discovering new molecular structures

**Speculative:**
- Quantum graph attention running on room-temperature quantum hardware
- Quantum supremacy for graph attention (provably better than any classical approach)

### 6.3 By 2036+

**Possible:**
- Production quantum graph transformers for drug discovery, materials science
- Quantum graph attention on million-qubit machines
- Hybrid quantum-neuromorphic graph transformers

**Speculative:**
- Fault-tolerant quantum graph attention with arbitrary circuit depth
- Quantum graph transformers simulating quantum systems (quantum simulation of quantum attention)
- Quantum consciousness in graph transformers (quantum effects in artificial cognition)

---

## 7. RuVector Implementation Roadmap

### Phase 1: Quantum Circuits for Graph Attention (2026-2027)
- Extend `ruQu` with graph-structured quantum circuits
- Implement SWAP-test attention protocol
- Add variational quantum graph transformer circuits
- Simulation backend (classical simulation of quantum attention for testing)

### Phase 2: Quantum Walk Integration (2027-2028)
- Implement continuous-time and discrete-time quantum walk attention
- Multi-scale quantum walk attention layer
- Integration with `ruvector-attention` trait system
- Benchmark against classical attention on standard graph benchmarks

### Phase 3: Fault-Tolerant Graph Attention (2028-2030)
- Graph-structured quantum error correcting codes using `ruQu` surface codes
- Fault-tolerant quantum attention compilation pipeline
- Cloud deployment targeting IBM Quantum / Google Quantum AI backends
- Hardware-aware circuit optimization

### Phase 4: Quantum Advantage (2030-2033)
- Target practical quantum advantage on specific graph problems
- Custom quantum codes for graph transformer error patterns
- Quantum-classical hybrid optimization loops
- Integration with formal verification (`ruvector-verified` + quantum proofs)

---

## References

1. Verdon et al., "Quantum Graph Neural Networks," 2019
2. Dernbach et al., "Quantum Walk Neural Networks with Feature Dependent Coins," Applied Network Science 2019
3. Zheng et al., "Quantum Computing Enhanced GNN," 2023
4. Childs et al., "Universal Computation by Quantum Walk," PRL 2009
5. Farhi & Gutmann, "Quantum computation and decision trees," PRA 1998
6. Gottesman, "Stabilizer codes and quantum error correction," Caltech PhD thesis 1997
7. RuVector `ruQu` documentation (internal)

---

**End of Document 24**

**Next:** [Doc 25 - Self-Organizing Morphogenetic Networks](25-self-organizing-morphogenetic-nets.md)
