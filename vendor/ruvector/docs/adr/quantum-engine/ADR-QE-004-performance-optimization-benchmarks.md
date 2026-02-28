# ADR-QE-004: Performance Optimization & Benchmarks

**Status**: Proposed
**Date**: 2026-02-06
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board

## Context

### Problem Statement

Quantum state-vector simulation is computationally expensive. Every gate
application touches the full amplitude vector of 2^n complex numbers, making
gate application O(2^n) per gate for n qubits. For the quantum engine to be
practical on edge devices and in browser environments, it must achieve
competitive performance: millions of gates per second for small circuits,
interactive latency for 10-20 qubit workloads, and the ability to handle
moderately deep circuits (thousands of gates) without unacceptable delays.

### Computational Cost Model

For a circuit with n qubits, g gates, and s measurement shots:

```
Total operations (approximate):

  Single-qubit gate:   2^n complex multiplications + 2^n complex additions
  Two-qubit gate:      2^(n+1) complex multiplications + 2^(n+1) complex additions
  Measurement (1 shot): 2^n probability calculations + sampling
  Full circuit:        sum_i(cost(gate_i)) + s * 2^n

  Example: 20-qubit circuit, 500 gates, 1024 shots
    Gate cost:  500 * 2^20 * ~4 FLOP = ~2.1 billion FLOP
    Measure:    1024 * 2^20 * ~2 FLOP = ~2.1 billion FLOP
    Total:      ~4.2 billion FLOP
```

At 10 GFLOP/s (realistic single-core throughput), this is ~420 ms. With SIMD
and multi-threading, we target 10-50x improvement.

### Performance Baseline from Comparable Systems

| Simulator | Language | 20-qubit H gate | Notes |
|-----------|----------|-----------------|-------|
| Qiskit Aer | C++/Python | ~50 ns | Heavily optimized, OpenMP |
| Cirq | Python/C++ | ~200 ns | Google, less optimized |
| QuantRS2 | Rust | ~57 ns | Rust-native, AVX2 |
| Quest | C | ~40 ns | GPU-capable, highly tuned |
| Target (ruQu) | Rust | < 60 ns | Competitive with QuantRS2 |

These benchmarks measure per-gate time on a single-qubit Hadamard applied to
a 20-qubit state vector. Our target is to match or beat QuantRS2, the closest
comparable pure-Rust implementation.

## Decision

Implement a **multi-layered optimization strategy** with six complementary
techniques, each addressing a different performance bottleneck.

### Layer 1: SIMD Operations

Use `ruvector-math` SIMD utilities to vectorize amplitude manipulation.
Gate application fundamentally involves applying a 2x2 or 4x4 unitary matrix
to pairs/quadruples of complex amplitudes. SIMD processes multiple amplitude
components simultaneously.

**Native SIMD dispatch**:

```
Architecture     Instruction Set     Complex f64 per Cycle
-----------      ---------------     ---------------------
x86_64           AVX-512             4 (512-bit / 128-bit per complex)
x86_64           AVX2                2 (256-bit / 128-bit per complex)
ARM64            NEON                1 (128-bit / 128-bit per complex)
WASM             SIMD128             1 (128-bit / 128-bit per complex)
Fallback         Scalar              1 (sequential)
```

**Single-qubit gate application with AVX2**:

```
For each pair of amplitudes (a[i], a[i + 2^target]):

  Load:  a_re, a_im = load_f64x4([a[i].re, a[i].im, a[i+step].re, a[i+step].im])

  Compute c0 = u00 * a + u01 * b:
    mul_re = u00_re * a_re - u00_im * a_im + u01_re * b_re - u01_im * b_im
    mul_im = u00_re * a_im + u00_im * a_re + u01_re * b_im + u01_im * b_re

  Compute c1 = u10 * a + u11 * b:
    (analogous)

  Store: [c0.re, c0.im, c1.re, c1.im]
```

With AVX2 (256-bit), we process 2 complex f64 values per instruction,
yielding a theoretical 2x speedup over scalar. With AVX-512, this doubles to
4x. Practical speedup is 1.5-3.5x due to instruction latency and memory
bandwidth.

**Target per-gate throughput**:

| Qubits | Amplitudes | AVX2 (est.) | AVX-512 (est.) | WASM SIMD (est.) |
|--------|------------|-------------|----------------|-------------------|
| 10 | 1,024 | ~15 ns | ~10 ns | ~30 ns |
| 15 | 32,768 | ~1 us | ~0.5 us | ~2 us |
| 20 | 1,048,576 | ~50 us | ~25 us | ~100 us |
| 25 | 33,554,432 | ~1.5 ms | ~0.8 ms | ~3 ms |

### Layer 2: Multithreading

Rayon-based data parallelism splits the state vector across CPU cores for
gate application. Each thread processes an independent contiguous block of
amplitudes.

**Parallelization strategy**:

```
State vector: [amp_0, amp_1, ..., amp_{2^n - 1}]

Thread 0:  [amp_0          ... amp_{2^n/T - 1}]
Thread 1:  [amp_{2^n/T}    ... amp_{2*2^n/T - 1}]
  ...
Thread T-1:[amp_{(T-1)*2^n/T} ... amp_{2^n - 1}]

Where T = number of threads (Rayon work-stealing pool)
```

**Gate application requires care with target qubit position**:

- If `target < log2(chunk_size)`: each chunk contains complete amplitude pairs.
  Threads are fully independent. No synchronization needed.
- If `target >= log2(chunk_size)`: amplitude pairs span chunk boundaries.
  Must adjust chunk boundaries to align with gate structure.

**Expected scaling**:

```
Qubits    Amps         1 thread    8 threads    Speedup
------    ----         --------    ---------    -------
15        32K          1 us        ~200 ns      ~5x
20        1M           50 us       ~8 us        ~6x
22        4M           200 us      ~30 us       ~6.5x
24        16M          800 us      ~120 us      ~6.7x
25        32M          1.5 ms      ~220 us      ~6.8x
```

Speedup plateaus below linear (8x for 8 threads) due to memory bandwidth
saturation. At 24+ qubits, the state vector exceeds L3 cache and performance
becomes memory-bound.

**Parallelism threshold**: Do not parallelize below 14 qubits (16K amplitudes).
The overhead of Rayon's work-stealing exceeds the benefit for small states.

### Layer 3: Gate Fusion

Preprocess circuits to combine consecutive gates into single matrix
operations, reducing the number of state vector passes.

**Fusion rules**:

```
Rule 1: Consecutive single-qubit gates on the same qubit
  Rz(a) -> Rx(b) -> Rz(c)  ==>  U3(a, b, c)  [single matrix multiply]

Rule 2: Consecutive two-qubit gates on the same pair
  CNOT(0,1) -> CZ(0,1)  ==>  Fused_2Q(0,1)  [4x4 matrix]

Rule 3: Single-qubit gate followed by controlled gate
  H(0) -> CNOT(0,1)  ==>  Fused operation (absorb H into CNOT matrix)

Rule 4: Identity cancellation
  H -> H  ==>  Identity (remove both)
  X -> X  ==>  Identity
  S -> S_dag  ==>  Identity
  CNOT -> CNOT (same control/target)  ==>  Identity
```

**Fusion effectiveness by algorithm**:

| Algorithm | Typical Fusion Ratio | Gate Reduction |
|-----------|----------------------|----------------|
| VQE (UCCSD ansatz) | 1.8-2.5x | 30-50% fewer state passes |
| Grover's | 1.2-1.5x | 15-25% |
| QAOA | 1.5-2.0x | 25-40% |
| QFT | 2.0-3.0x | 40-60% |
| Random circuit | 1.1-1.3x | 5-15% |

**Implementation**:

```rust
pub struct FusionPass;

impl CircuitOptimizer for FusionPass {
    fn optimize(&self, circuit: &mut QuantumCircuit) {
        let mut i = 0;
        while i < circuit.gates.len() - 1 {
            let current = &circuit.gates[i];
            let next = &circuit.gates[i + 1];

            if can_fuse(current, next) {
                let fused = compute_fused_matrix(current, next);
                circuit.gates[i] = fused;
                circuit.gates.remove(i + 1);
                // Don't advance i; check if we can fuse again
            } else {
                i += 1;
            }
        }
    }
}
```

### Layer 4: Entanglement-Aware Splitting

Track which qubits have interacted via entangling gates. Simulate independent
qubit subsets as separate, smaller state vectors. Merge subsets when an
entangling gate connects them.

**Concept**:

```
Circuit: q0 --[H]--[CNOT(0,1)]--[Rz]--
         q1 --[H]--[CNOT(0,1)]--[Ry]--
         q2 --[H]--[X]---------[Rz]---[CNOT(2,0)]--
         q3 --[H]--[Y]---------[Rx]--

Initially: {q0}, {q1}, {q2}, {q3}  -- four 2^1 vectors (2 amps each)
After CNOT(0,1): {q0,q1}, {q2}, {q3}  -- one 2^2 + two 2^1 vectors
After CNOT(2,0): {q0,q1,q2}, {q3}  -- one 2^3 + one 2^1 vector

Memory: 8 + 2 = 10 amplitudes  vs  2^4 = 16 amplitudes (full)
```

**Savings scale dramatically for circuits with late entanglement**:

```
Scenario: 20-qubit circuit, first 100 gates are local, then entangling

Without splitting: 2^20 = 1M amplitudes from gate 1
With splitting:    20 * 2^1 = 40 amplitudes until first entangling gate
                   Progressively merge as entanglement grows
```

**Data structure**:

```rust
pub struct SplitState {
    /// Each subset: (qubit indices, state vector)
    subsets: Vec<(Vec<usize>, QuantumState)>,
    /// Union-Find structure for tracking connectivity
    connectivity: UnionFind,
}

impl SplitState {
    pub fn apply_gate(&mut self, gate: &Gate, targets: &[usize]) {
        if gate.is_entangling() {
            // Merge subsets containing target qubits
            let merged = self.merge_subsets(targets);
            // Apply gate to merged state
            merged.apply_gate(gate, targets);
        } else {
            // Apply to the subset containing the target qubit
            let subset = self.find_subset(targets[0]);
            subset.apply_gate(gate, targets);
        }
    }
}
```

**When splitting helps vs. hurts**:

| Circuit Type | Splitting Benefit |
|-------------|-------------------|
| Shallow QAOA (p=1-3) | High (qubits entangle gradually) |
| VQE with local ansatz | High (many local rotations) |
| Grover's (full oracle) | Low (oracle entangles all qubits early) |
| QFT | Low (all-to-all entanglement) |
| Random circuits | Low (entangles quickly) |

The engine automatically disables splitting when all qubits are connected,
falling back to full state-vector simulation with zero overhead.

### Layer 5: Cache-Local Processing

For large state vectors (>20 qubits), cache utilization becomes critical.
The state vector exceeds L2 cache (typically 256 KB - 1 MB) and potentially
L3 cache (8-32 MB).

**Cache analysis**:

```
Qubits    State Size     L2 (512KB)    L3 (16MB)
------    ----------     ----------    ---------
18        4 MB           8x oversize   in cache
20        16 MB          32x           in cache
22        64 MB          128x          4x oversize
24        256 MB         512x          16x oversize
25        512 MB         1024x         32x oversize
```

**Techniques**:

1. **Aligned allocation**: State vector aligned to cache line boundaries (64
   bytes) for optimal prefetch behavior. Uses `ruvector-math` aligned allocator.

2. **Blocking/tiling**: For gates on high-index qubits, the stride between
   amplitude pairs is large (2^target). Tiling the access pattern to process
   cache-line-sized blocks sequentially improves spatial locality.

   ```
   Without tiling (target qubit = 20):
     Access pattern: amp[0], amp[1M], amp[1], amp[1M+1], ...
     Cache misses: ~every access (stride = 16 MB)

   With tiling (block size = L2/4):
     Process block [0..64K], then [64K..128K], ...
     Cache misses: ~1 per block (sequential within block)
   ```

3. **Prefetch hints**: Insert software prefetch instructions for the next block
   of amplitudes while processing the current block.

   ```rust
   // Prefetch next cache line while processing current
   #[cfg(target_arch = "x86_64")]
   unsafe {
       core::arch::x86_64::_mm_prefetch(
           state.as_ptr().add(i + CACHE_LINE_AMPS) as *const i8,
           core::arch::x86_64::_MM_HINT_T0,
       );
   }
   ```

### Layer 6: Lazy Evaluation

Accumulate commuting rotations and defer their application until a
non-commuting gate appears. This reduces the number of full state-vector
passes for rotation-heavy circuits common in variational algorithms.

**Commutation rules**:

```
Rz(a) commutes with Rz(b)  =>  Rz(a+b)
Rx(a) commutes with Rx(b)  =>  Rx(a+b)
Rz commutes with CZ        =>  Defer Rz
Diagonal gates commute      =>  Combine phases

But:
Rz does NOT commute with H
Rx does NOT commute with CNOT (on target)
```

**Implementation sketch**:

```rust
pub struct LazyAccumulator {
    /// Pending rotations per qubit: (axis, total_angle)
    pending: HashMap<usize, Vec<(RotationAxis, f64)>>,
}

impl LazyAccumulator {
    pub fn push_gate(&mut self, gate: &Gate, target: usize) -> Option<FlushedGate> {
        if let Some(rotation) = gate.as_rotation() {
            if let Some(existing) = self.pending.get_mut(&target) {
                if existing.last().map_or(false, |(axis, _)| *axis == rotation.axis) {
                    // Same axis: accumulate angle
                    existing.last_mut().unwrap().1 += rotation.angle;
                    return None; // No gate emitted
                }
            }
            self.pending.entry(target).or_default().push((rotation.axis, rotation.angle));
            None
        } else {
            // Non-commuting gate: flush pending rotations for affected qubits
            let flushed = self.flush(target);
            Some(flushed)
        }
    }
}
```

**Effectiveness**: VQE circuits with alternating Rz-Rx-Rz layers see 20-40%
reduction in state-vector passes. QAOA circuits with repeated ZZ-rotation
layers see 15-30% reduction.

## Benchmark Targets

### Primary Benchmark Suite

| ID | Workload | Qubits | Gates | Target Time | Notes |
|----|----------|--------|-------|-------------|-------|
| B1 | Grover (8 qubits) | 8 | ~200 | < 1 ms | 3 Grover iterations |
| B2 | Grover (16 qubits) | 16 | ~3,000 | < 10 ms | ~64 iterations |
| B3 | VQE iteration (12 qubits) | 12 | ~120 | < 5 ms | Single parameter update |
| B4 | VQE iteration (20 qubits) | 20 | ~300 | < 50 ms | UCCSD ansatz |
| B5 | QAOA p=3 (10 nodes) | 10 | ~75 | < 1 ms | MaxCut on random graph |
| B6 | QAOA p=5 (20 nodes) | 20 | ~200 | < 200 ms | MaxCut on random graph |
| B7 | Surface code cycle (d=3) | 17 | ~20 | < 10 ms | Single syndrome round |
| B8 | 1000 surface code cycles | 17 | ~20,000 | < 2 s | Repeated error correction |
| B9 | QFT (20 qubits) | 20 | ~210 | < 30 ms | Full quantum Fourier transform |
| B10 | Random circuit (25 qubits) | 25 | 100 | < 10 s | Worst-case memory test |

### Micro-Benchmarks

Per-gate timing for individual operations:

| Gate | 10 qubits | 15 qubits | 20 qubits | 25 qubits |
|------|-----------|-----------|-----------|-----------|
| H | < 20 ns | < 0.5 us | < 50 us | < 1.5 ms |
| CNOT | < 30 ns | < 1 us | < 80 us | < 2.5 ms |
| Rz(theta) | < 15 ns | < 0.4 us | < 40 us | < 1.2 ms |
| Toffoli | < 50 ns | < 1.5 us | < 120 us | < 4 ms |
| Measure | < 10 ns | < 0.3 us | < 30 us | < 1 ms |

### WASM-Specific Benchmarks

| ID | Workload | Qubits | Target (WASM) | Target (Native) | Expected Ratio |
|----|----------|--------|---------------|-----------------|----------------|
| W1 | Grover (8) | 8 | < 3 ms | < 1 ms | ~3x |
| W2 | VQE iter (12) | 12 | < 12 ms | < 5 ms | ~2.5x |
| W3 | QAOA p=3 (10) | 10 | < 2.5 ms | < 1 ms | ~2.5x |
| W4 | Random (20) | 20 | < 500 ms | < 200 ms | ~2.5x |
| W5 | Random (25) | 25 | < 25 s | < 10 s | ~2.5x |

### Benchmark Infrastructure

Benchmarks use Criterion.rs for native and a custom timing harness for WASM:

```rust
// Native benchmarks (Criterion)
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_grover_8(c: &mut Criterion) {
    c.bench_function("grover_8_qubits", |b| {
        b.iter(|| {
            let mut state = QuantumState::new(8).unwrap();
            let circuit = grover_circuit(8, &target_state);
            state.execute(&circuit)
        })
    });
}

fn bench_single_gate_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("hadamard_scaling");
    for n in [10, 12, 14, 16, 18, 20, 22, 24] {
        group.bench_with_input(
            BenchmarkId::from_parameter(n),
            &n,
            |b, &n| {
                let mut state = QuantumState::new(n).unwrap();
                let mut circuit = QuantumCircuit::new(n).unwrap();
                circuit.gate(Gate::H, &[0]);
                b.iter(|| state.execute(&circuit))
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_grover_8, bench_single_gate_scaling);
criterion_main!(benches);
```

**WASM benchmark harness**:

```javascript
// Browser-based benchmark using performance.now()
async function benchmarkGrover8() {
    const { QuantumCircuit, QuantumState } = await import('./ruqu_wasm.js');

    const iterations = 100;
    const start = performance.now();

    for (let i = 0; i < iterations; i++) {
        const circuit = QuantumCircuit.grover(8, 42);
        const state = new QuantumState(8);
        state.execute(circuit);
        state.free();
        circuit.free();
    }

    const elapsed = performance.now() - start;
    console.log(`Grover 8-qubit: ${(elapsed / iterations).toFixed(3)} ms/iteration`);
}
```

### Performance Regression Detection

CI runs benchmark suite on every PR. Regressions exceeding 10% trigger a
warning; regressions exceeding 25% block the merge.

```yaml
# In CI pipeline
- name: Run benchmarks
  run: |
    cargo bench --package ruqu-core -- --save-baseline pr
    cargo bench --package ruqu-core -- --baseline main --load-baseline pr
    # critcmp compares and flags regressions
    critcmp main pr --threshold 10
```

### Optimization Priority Matrix

Not all optimizations apply equally to all workloads. The priority matrix
guides implementation order:

| Optimization | Impact (small circuits) | Impact (large circuits) | Impl Effort | Priority |
|-------------|------------------------|------------------------|-------------|----------|
| SIMD | Medium (1.5-2x) | High (2-3.5x) | Medium | P0 |
| Multithreading | Low (overhead > benefit) | High (5-7x) | Medium | P1 |
| Gate fusion | High (30-50% fewer passes) | Medium (15-30%) | Low | P0 |
| Entanglement splitting | Variable (0-100x) | Low (quickly entangled) | High | P2 |
| Cache tiling | Low (fits in cache) | High (2-4x) | Medium | P1 |
| Lazy evaluation | Medium (20-40%) | Low (10-20%) | Low | P2 |

**Implementation order**: SIMD -> Gate Fusion -> Multithreading -> Cache Tiling
-> Lazy Evaluation -> Entanglement Splitting

## Consequences

### Positive

- **Competitive performance**: Multi-layered approach targets performance
  parity with state-of-the-art Rust simulators (QuantRS2)
- **Interactive latency**: Most practical workloads (8-20 qubits) complete
  in single-digit milliseconds, enabling real-time experimentation
- **Scalable**: Each optimization layer addresses a different bottleneck,
  providing compounding benefits
- **Measurable**: Concrete benchmark targets enable objective progress tracking
  and regression detection

### Negative

- **Optimization complexity**: Six optimization layers create significant
  implementation and maintenance complexity
- **Ongoing tuning**: Performance characteristics vary across hardware;
  benchmarks must cover representative platforms
- **Diminishing returns**: For >20 qubits, memory bandwidth dominates and
  compute optimizations yield marginal gains
- **Testing burden**: Each optimization must be validated for numerical
  correctness across all gate types

### Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Memory bandwidth bottleneck at >20 qubits | High | Medium | Document expected scaling; recommend native for large circuits |
| Gate fusion introducing numerical error | Low | High | Comprehensive numerical tests comparing fused vs. unfused results |
| Entanglement tracking overhead exceeding savings | Medium | Low | Automatic disable when all qubits connected within first 10 gates |
| WASM SIMD not available in target runtime | Low | Medium | Graceful fallback to scalar; runtime feature detection |
| Benchmark targets too aggressive for edge hardware | Medium | Low | Separate targets for edge (Cognitum) vs. desktop; scale expectations |

## References

- [ADR-QE-001: Quantum Engine Core Architecture](./ADR-QE-001-quantum-engine-core-architecture.md)
- [ADR-QE-002: Crate Structure & Integration](./ADR-QE-002-crate-structure-integration.md)
- [ADR-QE-003: WASM Compilation Strategy](./ADR-QE-003-wasm-compilation-strategy.md)
- [ADR-003: SIMD Optimization Strategy](/docs/adr/ADR-003-simd-optimization-strategy.md)
- [ruvector-math crate](/crates/ruvector-math/)
- Guerreschi & Hogaboam, "Intel Quantum Simulator: A cloud-ready high-performance
  simulator of quantum circuits" (2020)
- Jones et al., "QuEST and High Performance Simulation of Quantum Computers" (2019)
- QuantRS2 benchmark data (internal comparison)
