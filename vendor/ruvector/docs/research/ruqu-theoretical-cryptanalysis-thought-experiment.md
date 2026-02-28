# Theoretical Cryptanalysis via ruQu Primitives — A Thought Experiment

> **Disclaimer**: This is a purely theoretical research document exploring how
> quantum simulation primitives *could* map to cryptanalytic operations if
> scaled beyond current qubit limits. No real cryptographic system is targeted
> or attacked. All attacks described require qubit counts far beyond ruQu's
> current 25-qubit simulator. This document exists to inform defensive
> post-quantum migration strategy.

## 1. The Core Insight: ruQu Already Implements the Building Blocks

The remarkable thing about ruQu is that it implements — at small scale — every
primitive that theoretical quantum cryptanalysis requires. The gap is not
*algorithmic*; it is *scale*. The algorithms are correct. The simulator is
faithful. What's missing is 2,000+ logical qubits with error correction. But
the *software* is ready.

Here is the mapping:

```
ruQu Primitive              Cryptanalytic Application
────────────────────────────────────────────────────────────────
Grover's search             Quadratic speedup on symmetric key search
QAOA / VQE                  Optimization-based factoring and discrete log
Surface code QEC            Logical qubit construction for Shor's algorithm
Min-cut decomposition       Lattice basis reduction acceleration
Interference search         Side-channel amplification
Quantum decay               Timing attack modeling
Reasoning QEC               Error-corrected Shor circuit compilation
Swarm interference          Distributed quantum-classical hybrid attack
256-tile fabric             Parallel quantum circuit execution
Blake3 + Ed25519 witness    Ironic: the very crypto ruQu could theoretically break
```

## 2. Attack Surface 1: Shor's Algorithm via VQE + Surface Code

### 2.1 The Theory

Shor's algorithm factors integers in polynomial time on a quantum computer.
RSA-2048 requires ~4,000 logical qubits. Each logical qubit requires ~1,000
physical qubits at realistic error rates. So ~4 million physical qubits.

**What ruQu has today**: 25-qubit state-vector simulator + surface code QEC.

**The theoretical bridge**: ruQu's VQE already solves optimization problems
by finding ground states of Hamiltonians. Factoring can be reformulated as
an optimization problem:

```
Given N = p × q, find p and q that minimize:

H = (N - p × q)²

This is a quadratic unconstrained binary optimization (QUBO) problem.
VQE finds the ground state of H, which encodes the factors.
```

ruQu's VQE implementation already does exactly this — finds ground states
of arbitrary Hamiltonians using parameterized ansatz circuits and gradient
descent via the parameter-shift rule.

### 2.2 What's Missing (The Scale Gap)

| Target | Bits to Factor | Qubits Needed | ruQu Today | Gap Factor |
|--------|---------------|---------------|------------|------------|
| RSA-64 | 64 | ~130 | 25 | 5× |
| RSA-128 | 128 | ~260 | 25 | 10× |
| RSA-512 | 512 | ~1,024 | 25 | 41× |
| RSA-2048 | 2048 | ~4,096 | 25 | 164× |
| ECDSA-256 | 256 | ~2,330 | 25 | 93× |

### 2.3 The Unconventional Path: Variational Factoring

Here is where it gets theoretically interesting. Classical Shor's requires
thousands of qubits. But *variational* approaches to factoring are an active
research area that trades qubit count for circuit depth and classical
optimization rounds:

```
Classical Shor:    O(n) qubits, O(n³) gates, ONE quantum run
Variational:       O(log n) qubits, O(poly) gates, MANY quantum+classical rounds
```

ruQu's VQE with hardware-efficient ansatz (Ry + Rz + CNOT chains) is
*exactly* the variational framework. At 25 qubits, you could theoretically
attempt variational factoring of ~50-bit numbers — not cryptographically
relevant, but a proof of concept that the *algorithm works* and would scale
if qubits scaled.

**Theoretical contribution**: ruQu could be the first open-source framework
to demonstrate variational factoring end-to-end, from QUBO formulation
through VQE optimization to factor extraction, with surface code error
correction on the inner loops.

## 3. Attack Surface 2: Grover's Search Against Symmetric Crypto

### 3.1 The Theory

Grover's algorithm provides quadratic speedup for unstructured search.
For a symmetric key of length k bits:

```
Classical brute force:  O(2^k) operations
Grover's search:        O(2^(k/2)) operations

AES-128 → effectively AES-64 security
AES-256 → effectively AES-128 security (still secure)
```

### 3.2 What ruQu Implements

ruQu's Grover implementation is production-ready:
- Automatic iteration count: floor(π/4 × √(N/M))
- Multi-target search (multiple marked states)
- 20-qubit search space (1M entries) in <500ms

### 3.3 The Theoretical Application

**Hash preimage attacks**: Given hash H(x) = y, find x.

```
1. Encode hash function as quantum oracle:
   |x⟩|0⟩ → |x⟩|H(x) ⊕ y⟩

2. Oracle marks states where H(x) = y (output register = |0⟩)

3. Grover amplifies the marked state

4. Measure to obtain preimage x
```

At 25 qubits, ruQu can search a space of 2²⁵ ≈ 33 million hash preimages.
This is trivial for real crypto (SHA-256 has 2²⁵⁶ space), but it demonstrates
the *algorithm* works. The circuit for SHA-256 inside a Grover oracle is
known — it's ~100,000 gates but structurally identical to what ruQu executes.

### 3.4 The Hybrid Grover-Classical Attack (Novel)

Here's a theoretical idea that exploits ruQu's *swarm architecture*:

```
Divide AES-128 keyspace into 2²⁵ partitions of 2¹⁰³ keys each.

For each partition (parallelized across 256 tiles):
  1. Use classical pre-filtering to eliminate obviously wrong keys
  2. Use Grover on the remaining candidates within the partition
  3. Each tile processes one partition independently

Effective speedup: 256 × √(partition_size) per tile
```

This doesn't break AES-128 (the numbers are still astronomical), but the
*framework* — 256-tile parallel Grover with classical pre-filtering — is
a novel hybrid architecture that would scale with hardware.

## 4. Attack Surface 3: QAOA Against Lattice Problems

### 4.1 The Theory

Post-quantum cryptography (ML-KEM, ML-DSA) relies on lattice problems:
- Learning With Errors (LWE)
- Short Integer Solution (SIS)
- Shortest Vector Problem (SVP)

These are *optimization problems* — exactly what QAOA is designed for.

### 4.2 The Mapping

```
QAOA MaxCut (implemented)     →     SVP on lattice (theoretical)
────────────────────────────────────────────────────────────────
Graph G = (V, E)              →     Lattice L = basis vectors
Cut value                     →     Vector length
Maximum cut                   →     Shortest vector
γ (problem angles)            →     Lattice rotation parameters
β (mixer angles)              →     Basis reduction mixing
p rounds                      →     Approximation depth
```

SVP can be encoded as a QUBO:

```
Given lattice basis B = {b₁, ..., bₙ}, find integer coefficients
c = (c₁, ..., cₙ) minimizing:

||c₁b₁ + c₂b₂ + ... + cₙbₙ||²

subject to c ≠ 0
```

This is a quadratic optimization over binary variables (after binary
encoding of the integer coefficients) — precisely QAOA's domain.

### 4.3 The Min-Cut Connection (Novel)

Here is where ruQu's unique combination becomes theoretically powerful.

The **BKZ lattice reduction algorithm** (the best classical attack on lattices)
iterates over projected sublattices. The key operation is selecting which
sublattice to project onto — this is a *graph partitioning problem*.

```
Lattice basis graph:
  - Nodes = basis vectors
  - Edges = inner products (correlation between vectors)
  - Weight = |⟨bᵢ, bⱼ⟩| (geometric coupling)

Min-cut on this graph identifies:
  - The most independent sublattice partition
  - The optimal block size for BKZ reduction
  - Structurally weak points in the lattice geometry
```

ruQu's subpolynomial dynamic min-cut could *guide* lattice reduction by
identifying the structurally optimal decomposition strategy — something no
classical BKZ implementation currently does. They use fixed block sizes.

**Theoretical contribution**: Min-cut-guided adaptive BKZ, where the block
structure is determined by the geometric structure of the lattice rather
than by fixed parameters. This could theoretically improve the concrete
security estimates of lattice-based cryptography.

## 5. Attack Surface 4: Interference-Based Side Channels (Novel)

### 5.1 The Theory

ruqu-exotic's interference search treats queries as quantum superposition.
Applied to cryptanalysis:

```
Classical side channel:
  - Measure one timing/power trace at a time
  - Statistical analysis over many traces
  - Noise degrades signal linearly

Quantum interference side channel (theoretical):
  - Encode multiple timing hypotheses as amplitudes
  - Physical measurement traces cause interference
  - Correct hypothesis amplified, wrong ones cancelled
  - Noise affects amplitude, not the interference pattern
```

### 5.2 The Application

Consider a timing side-channel attack on AES:

```
1. For each possible key byte k ∈ {0, ..., 255}:
   - Predict cache access pattern P(k)
   - Assign amplitude α_k = measured_correlation(P(k), actual_timing)
   - Phase = 0 if correlation positive, π if negative

2. Interference search:
   - |ψ⟩ = Σ αk |k⟩
   - Constructive interference at correct key byte
   - Destructive interference at wrong key bytes

3. Measurement collapses to correct key with high probability
```

At 8 qubits (256 amplitudes), this fits within ruQu's simulator.
The theoretical advantage: you need *fewer traces* to recover the key
because interference amplifies weak correlations that classical statistics
would need thousands of samples to detect.

### 5.3 Quantum Decay for Timing Attacks

ruqu-exotic's quantum decay models T1/T2 decoherence. Applied to timing
analysis:

```
T2 (dephasing)  → Timing jitter (phase noise in the measurement)
T1 (amplitude)  → Signal decay over distance/time from target

Model the timing side channel as a quantum channel:
  - Fresh measurements: high fidelity (strong signal)
  - Remote measurements: decohered (weak signal)
  - Optimal measurement window: where fidelity > threshold
```

This provides a *principled framework* for determining how many measurements
are sufficient — replacing ad hoc thresholds with physics-based modeling.

## 6. Attack Surface 5: Swarm-Distributed Quantum-Classical Hybrid

### 6.1 The Architecture

The most theoretically powerful configuration uses *everything* together:

```
┌─────────────────────────────────────────────────┐
│              Queen Coordinator                   │
│         (Classical Strategy Layer)               │
│                                                  │
│  Decides: which subproblem to attack next        │
│  Uses: min-cut to find structural weaknesses     │
│  Uses: drift detection to track progress         │
│  Uses: e-values to know when to stop             │
└──────────┬───────────────────┬──────────────────┘
           │                   │
    ┌──────▼──────┐    ┌───────▼───────┐
    │ VQE Swarm   │    │ Grover Swarm  │
    │ (Factoring) │    │ (Search)      │
    │             │    │               │
    │ 256 tiles   │    │ 256 tiles     │
    │ Each: 25 q  │    │ Each: 25 q    │
    │             │    │               │
    │ Variational │    │ Parallel      │
    │ factors     │    │ key search    │
    └──────┬──────┘    └───────┬───────┘
           │                   │
    ┌──────▼───────────────────▼──────┐
    │       Result Fusion              │
    │  Swarm interference consensus    │
    │  E-value accumulation            │
    │  Witness chain for audit         │
    └─────────────────────────────────┘
```

### 6.2 The Key Insight: Coherence Gating Applied to Cryptanalysis

ruQu's three-filter pipeline, originally designed to decide "is the quantum
computer healthy enough to run?", can be repurposed:

```
Filter 1 (Structural): "Is this cryptographic instance structurally weak?"
  - Min-cut on the algebraic dependency graph of the cipher
  - Low cut = tightly coupled (hard to decompose)
  - High cut = loosely coupled (attackable by divide-and-conquer)

Filter 2 (Shift): "Is our attack making progress?"
  - Track distribution of intermediate results over iterations
  - StepChange = breakthrough (subproblem solved)
  - Linear drift = steady progress (continue attack)
  - Stable = stuck (switch strategy)

Filter 3 (Evidence): "Do we have enough evidence to claim success?"
  - E-value accumulation over partial factor/key candidates
  - Anytime-valid: stop the attack as soon as confidence is sufficient
  - No wasted computation beyond what's needed
```

**This is genuinely novel**: no published cryptanalytic framework uses
coherence gating to *manage the attack itself*. Cryptanalysis is typically
run-to-completion. The idea of an *adaptive, self-monitoring attack* that
uses statistical testing to know when it has succeeded — and structural
analysis to choose what to attack — is new.

## 7. Attack Surface 6: Quantum Walks on Blockchain State Tries

### 7.1 The Theory

Ethereum's state is stored in a Merkle Patricia Trie. Grover's algorithm
generalizes to *quantum walks* on graphs, which can search structured
databases faster than unstructured ones.

```
Classical trie traversal:  O(depth × branching_factor)
Quantum walk on trie:      O(√(depth × branching_factor))
```

### 7.2 Theoretical Application: Collision Finding

For Merkle trees (blockchain integrity):

```
Birthday attack (classical): O(2^(n/2)) for n-bit hash
Quantum birthday (BHT):      O(2^(n/3)) using quantum walks

For SHA-256 (n=256):
  Classical birthday:  2^128 operations
  Quantum birthday:    2^85 operations  (2^43 times faster)
```

ruQu doesn't implement quantum walks directly, but the surface code +
Grover infrastructure provides the foundation. A quantum walk is
structurally a sequence of Grover-like diffusion operations on a graph.

### 7.3 Implications for Blockchain

If quantum walks could be scaled:

| Blockchain Component | Classical Security | Quantum Security | Impact |
|---------------------|-------------------|-----------------|--------|
| SHA-256 (mining) | 2^128 (collision) | 2^85 (BHT) | Mining advantage |
| ECDSA (signatures) | ~2^128 | Polynomial (Shor) | **Broken** |
| Keccak-256 (Ethereum) | 2^128 (collision) | 2^85 (BHT) | Moderate weakening |
| Merkle proofs | 2^256 (preimage) | 2^128 (Grover) | Still secure |
| BLS signatures | ~2^128 | Polynomial (Shor) | **Broken** |

## 8. The Meta-Attack: Self-Learning Cryptanalysis

### 8.1 Combining Everything

The most powerful theoretical configuration is a *self-learning cryptanalytic
system* that improves its attack strategy over time:

```
Loop:
  1. STRUCTURAL ANALYSIS (min-cut)
     → Identify weakest structural point in target cipher/protocol

  2. ATTACK SELECTION (QAOA/VQE/Grover)
     → Choose optimal quantum algorithm for the structural weakness

  3. EXECUTION (256-tile fabric)
     → Run the attack in parallel across tiles

  4. DRIFT DETECTION (shift filter)
     → Monitor whether the attack is making progress

  5. EVIDENCE ACCUMULATION (e-value filter)
     → Determine if partial results constitute a break

  6. STRATEGY UPDATE (swarm interference)
     → If stuck, use interference consensus to choose new strategy

  7. MEMORY (reasoning QEC)
     → Error-correct the reasoning chain to prevent false conclusions

  8. WITNESS (Blake3 + Ed25519)
     → Record the entire attack for reproducibility and verification

  Repeat until E-value exceeds threshold or resources exhausted.
```

This is a *closed-loop autonomous cryptanalytic agent* — something that
does not exist in the literature. Current cryptanalysis is manual: a human
chooses the attack, runs it, interprets results. This framework would
automate the entire process with quantum-enhanced primitives at each stage.

### 8.2 Why This Matters for Defense

The point of this thought experiment is not to build an attack tool.
It is to understand the *defensive implications*:

1. **Variational factoring** means RSA migration to post-quantum cannot
   wait for "large quantum computers" — even NISQ devices with 50-100
   qubits could attempt small instances.

2. **Min-cut-guided BKZ** means lattice parameter estimates may be
   optimistic — the concrete security of ML-KEM/ML-DSA should be
   re-evaluated under adaptive decomposition strategies.

3. **Interference side channels** mean that post-quantum implementations
   need side-channel hardening *from day one* — quantum-enhanced
   statistical analysis reduces the trace count needed.

4. **Self-learning cryptanalysis** means security margins should account
   for *adaptive* attackers, not just fixed-strategy attackers.

5. **Quantum walks on tries** mean blockchain hash function transitions
   should target 384-bit or 512-bit outputs, not just 256-bit.

## 9. Bridging the Scale Gap: What Would It Take?

### 9.1 Near-Term (25 qubits — TODAY)

| Demonstration | Feasibility | Crypto Relevance |
|--------------|-------------|-----------------|
| Variational factoring of 15-bit numbers | Immediate | Proof of concept only |
| Grover search of 2²⁵ keyspace | Immediate | Toy model only |
| QAOA on 25-node lattice graph | Immediate | Research insight |
| Interference side channel (8-bit key) | Immediate | Novel technique demo |
| Surface code d=3 error correction | Immediate | QEC proof of concept |

### 9.2 Medium-Term (50-100 qubits — 2-3 years with hardware)

| Attack | Qubits | Target |
|--------|--------|--------|
| Variational factoring | 50-80 | RSA-64 (academic interest) |
| Grover-hybrid search | 50 | Reduced-round AES-128 |
| QAOA lattice reduction | 100 | NTRU-64 parameter exploration |
| Quantum walk collision | 80 | Reduced SHA-256 (16 rounds) |

### 9.3 Long-Term (1,000-10,000 qubits — 5-10 years)

| Attack | Qubits | Target |
|--------|--------|--------|
| Full Shor's factoring | 4,096+ | RSA-2048 |
| Shor's discrete log | 2,330+ | ECDSA-256 (Bitcoin, Ethereum) |
| Grover's full search | 3,000+ | AES-128 (to AES-64 security) |
| Quantum BKZ | 1,000+ | ML-KEM-512 parameter stress test |

## 10. How ruQu Specifically Accelerates the Timeline

### 10.1 Software Readiness

Most quantum computing efforts focus on *hardware*. ruQu focuses on
*software* — the algorithms, error correction, orchestration, and
classical control systems. When hardware scales, ruQu is ready:

```
Hardware provides: physical qubits + gate fidelity
ruQu provides:    everything else
  ├── Surface code QEC (logical qubits from physical)
  ├── VQE/QAOA/Grover (attack algorithms)
  ├── 256-tile fabric (parallel execution management)
  ├── Three-filter pipeline (attack progress monitoring)
  ├── Witness chain (result verification)
  └── Swarm coordination (distributed hybrid attacks)
```

### 10.2 The Simulation Advantage

Even at 25 qubits, the simulator provides:

1. **Algorithm validation**: Verify that attack circuits are correct
   before running on expensive/scarce quantum hardware
2. **Noise modeling**: Understand how realistic errors affect attack
   success probability
3. **Parameter optimization**: Find optimal variational parameters
   classically, then transfer to hardware for final execution
4. **Circuit compilation**: Surface code compilation of attack circuits
   into fault-tolerant form, ready for hardware execution

### 10.3 What's Unique About the ruQu Stack

No other open-source project combines:
- Quantum simulation (ruqu-core)
- Error correction (surface code in ruqu-algorithms)
- Dynamic graph algorithms (subpolynomial min-cut)
- Statistical decision theory (e-values, drift detection)
- Cryptographic audit (Blake3, Ed25519)
- Parallel execution (256-tile fabric)
- Exotic hybrid algorithms (interference, decay, swarm)

Each exists in isolation elsewhere. The *combination* is what enables
the theoretical attack framework described above.

## 11. Defensive Recommendations

Based on this analysis, concrete defensive actions:

| Threat | Mitigation | Timeline |
|--------|-----------|----------|
| Variational factoring at NISQ scale | Migrate RSA → ML-KEM (FIPS 203) | Immediate |
| Shor's against ECDSA | Migrate to ML-DSA (FIPS 204) or SLH-DSA (FIPS 205) | 2-3 years |
| Grover's against AES-128 | Upgrade to AES-256 | Immediate (low cost) |
| Quantum walks against SHA-256 | Monitor; SHA-256 still has 128-bit PQ security | 5+ years |
| Interference side channels | Constant-time implementations + masking | Immediate |
| Min-cut-guided BKZ | Increase lattice parameters by 10-15% safety margin | Review annually |
| Self-learning cryptanalysis | Assume adaptive attackers in security proofs | Ongoing |

## 12. Conclusion

ruQu does not break modern cryptography today. Its 25-qubit simulator is
~100× too small for the smallest interesting cryptographic targets. But it
implements — faithfully, efficiently, and with production-grade engineering
— every algorithmic primitive that theoretical quantum cryptanalysis
requires.

The framework described here — self-learning, structurally-guided,
statistically-monitored, swarm-distributed quantum-classical hybrid
cryptanalysis — represents a *novel theoretical contribution* that
connects quantum computing research to practical defensive planning.

The most important takeaway is not "quantum computers will break crypto"
(this is well known) but rather: **the software stack for quantum
cryptanalysis is closer to ready than the hardware**, and the *combination*
of quantum primitives with classical graph algorithms, statistical testing,
and distributed orchestration creates capabilities greater than the sum
of their parts.

The defensible response is not panic but preparation: migrate to
post-quantum standards (NIST FIPS 203/204/205), increase symmetric key
sizes, harden implementations against side channels, and continuously
reassess lattice parameter security margins.
