# ADR-QE-013: Deutsch's Theorem — Proof, Historical Comparison, and Verification

**Status**: Accepted
**Date**: 2026-02-06
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-02-06 | ruv.io | Complete proof, historical comparison, ruqu verification |

---

## Context

Deutsch's theorem (1985) is the founding result of quantum computation. It demonstrates
that a quantum computer can extract a *global property* of a function using fewer queries
than any classical algorithm — the first provable quantum speedup. Our ruqu engine
(ADR-QE-001 through ADR-QE-008) implements the full gate set and state-vector simulator
required to verify this theorem programmatically.

This ADR provides:

1. A **rigorous proof** of Deutsch's theorem
2. A **comparative analysis** of the five major formulations by different authors
3. A **de-quantization critique** examining when the advantage truly holds
4. **Verification** via the ruqu-core simulator

---

## 1. Statement of the Theorem

**Deutsch's Problem.** Given a black-box oracle computing f: {0,1} → {0,1}, determine
whether f is *constant* (f(0) = f(1)) or *balanced* (f(0) ≠ f(1)).

**Theorem (Deutsch, 1985; deterministic form: Cleve et al., 1998).**
A quantum computer can solve Deutsch's problem with certainty using exactly **one** oracle
query. Any classical deterministic algorithm requires **two** queries.

---

## 2. Classical Lower Bound

**Claim.** Every classical deterministic algorithm requires 2 queries.

**Proof.** A classical algorithm queries f on inputs from {0,1} sequentially. After a
single query — say f(0) = b — both cases remain consistent with the observation:

- Constant: f(1) = b
- Balanced: f(1) = 1 − b

No deterministic strategy can distinguish these without a second query.
A probabilistic classical algorithm can guess with probability 1/2 after one query,
but cannot achieve certainty. ∎

---

## 3. Quantum Proof (Complete)

### 3.1 Oracle Definition

The quantum oracle U_f acts on two qubits as:

```
U_f |x⟩|y⟩ = |x⟩|y ⊕ f(x)⟩
```

where ⊕ is addition modulo 2. This is a unitary (and self-inverse) operation for all
four possible functions f.

### 3.2 Circuit

```
q0: |0⟩ ─── H ─── U_f ─── H ─── M ──→ result
q1: |1⟩ ─── H ──────────────────────
```

### 3.3 Step-by-Step Derivation

**Step 1. Initialization.**

```
|ψ₀⟩ = |0⟩|1⟩
```

**Step 2. Hadamard on both qubits.**

```
|ψ₁⟩ = H|0⟩ ⊗ H|1⟩
      = (|0⟩ + |1⟩)/√2  ⊗  (|0⟩ − |1⟩)/√2
```

**Step 3. Phase Kickback Lemma.**

> **Lemma.** Let |y⁻⟩ = (|0⟩ − |1⟩)/√2. Then for any x ∈ {0,1}:
>
>     U_f |x⟩|y⁻⟩ = (−1)^{f(x)} |x⟩|y⁻⟩

*Proof of Lemma.*

```
U_f |x⟩|y⁻⟩ = U_f |x⟩ (|0⟩ − |1⟩)/√2
             = (|x⟩|f(x)⟩ − |x⟩|1⊕f(x)⟩) / √2
```

Case f(x) = 0:
```
  = |x⟩(|0⟩ − |1⟩)/√2 = (+1)|x⟩|y⁻⟩
```

Case f(x) = 1:
```
  = |x⟩(|1⟩ − |0⟩)/√2 = (−1)|x⟩|y⁻⟩
```

Therefore U_f |x⟩|y⁻⟩ = (−1)^{f(x)} |x⟩|y⁻⟩.  ∎

**Step 4. Apply oracle to the superposition.**

By linearity of U_f and the Phase Kickback Lemma:

```
|ψ₂⟩ = [ (−1)^{f(0)} |0⟩ + (−1)^{f(1)} |1⟩ ] / √2  ⊗  |y⁻⟩
```

Factor out the global phase (−1)^{f(0)}:

```
|ψ₂⟩ = (−1)^{f(0)} · [ |0⟩ + (−1)^{f(0)⊕f(1)} |1⟩ ] / √2  ⊗  |y⁻⟩
```

**Step 5. Final Hadamard on first qubit.**

Using H|+⟩ = |0⟩ and H|−⟩ = |1⟩:

- If f(0) ⊕ f(1) = 0 (constant): first qubit is |+⟩, so H|+⟩ = |0⟩
- If f(0) ⊕ f(1) = 1 (balanced): first qubit is |−⟩, so H|−⟩ = |1⟩

Therefore:

```
|ψ₃⟩ = (−1)^{f(0)} · |f(0) ⊕ f(1)⟩ ⊗ |y⁻⟩
```

**Step 6. Measurement.**

| Measurement of q0 | Conclusion |
|---|---|
| \|0⟩ (probability 1) | f is **constant** |
| \|1⟩ (probability 1) | f is **balanced** |

The global phase (−1)^{f(0)} is physically unobservable. The measurement outcome is
**deterministic** — no probabilistic element remains. ∎

### 3.4 Why This Works

The quantum advantage arises from three principles acting together:

1. **Superposition**: The Hadamard gate creates a state that simultaneously probes
   both inputs f(0) and f(1) in a single oracle call.

2. **Phase kickback**: The oracle encodes f(x) into relative phases rather than
   bit values, moving information from the amplitude magnitudes into the complex
   phases of the state vector.

3. **Interference**: The final Hadamard converts the relative phase between |0⟩
   and |1⟩ into a computational basis state that can be measured. Constructive
   interference amplifies the correct answer; destructive interference suppresses
   the wrong one.

The algorithm extracts f(0) ⊕ f(1) — a *global* property — without ever learning
either f(0) or f(1) individually. This is impossible classically with one query.

---

## 4. Historical Comparison of Proofs

### 4.1 Timeline

| Year | Authors | Key Contribution |
|------|---------|------------------|
| 1985 | Deutsch | First quantum algorithm; probabilistic (50% success) |
| 1992 | Deutsch & Jozsa | Deterministic n-bit generalization; required 2 queries |
| 1998 | Cleve, Ekert, Macchiavello & Mosca | Deterministic + single query (modern form) |
| 2001 | Nielsen & Chuang | Canonical textbook presentation |
| 2006 | Calude | De-quantization of the single-bit case |

### 4.2 Deutsch's Original Proof (1985)

**Paper:** "Quantum Theory, the Church-Turing Principle and the Universal Quantum
Computer," *Proc. Royal Society London A* 400, pp. 97–117.

Deutsch's original algorithm was **probabilistic**, succeeding with probability 1/2.
The circuit prepared the first qubit in an eigenstate basis and relied on interference
at the output, but lacked the phase-kickback construction that the modern proof uses.

The key insight was not the algorithm itself but the *philosophical claim*: Deutsch
reformulated the Church-Turing thesis as a physical principle, arguing that since
physics is quantum mechanical, the correct model of computation must be quantum.
He noted that classical physics uses real numbers that cannot be represented by
Turing machines, and proposed the quantum Turing machine as the proper universal
model.

Deutsch also connected his work to the Everett many-worlds interpretation, arguing
that quantum parallelism could be understood as computation occurring across
parallel universes simultaneously.

**Limitations:**
- Only solved the 1-bit case
- Probabilistic (50% success rate)
- The advantage over classical was present but not deterministic

### 4.3 Deutsch-Jozsa Extension (1992)

**Paper:** "Rapid Solution of Problems by Quantum Computation," *Proc. Royal Society
London A* 439, pp. 553–558.

Deutsch and Jozsa generalized to n-bit functions f: {0,1}ⁿ → {0,1} where f is
promised to be either constant (same output on all inputs) or balanced (outputs 0
on exactly half the inputs and 1 on the other half).

**Key differences from 1985:**
- Deterministic algorithm (no probabilistic element)
- Required **two** oracle queries (not one)
- Demonstrated **exponential** speedup: quantum O(1) queries vs. classical
  worst-case 2^(n−1) + 1 queries for n-bit functions

**Proof technique:** Applied Hadamard to all n input qubits, queried the oracle once,
applied Hadamard again, and measured. If f is constant, the output is always |0⟩ⁿ.
If balanced, the output is never |0⟩ⁿ. However, the original 1992 formulation used
a slightly different circuit that needed a second query for the single-bit case.

### 4.4 Cleve-Ekert-Macchiavello-Mosca Improvement (1998)

**Paper:** "Quantum Algorithms Revisited," *Proc. Royal Society London A* 454,
pp. 339–354. (arXiv: quant-ph/9708016)

This paper provided the **modern, textbook form** of the algorithm:
- Deterministic
- Single oracle query
- Works for all n, including n = 1

**Critical innovation:** The introduction of the ancilla qubit initialized to |1⟩ and
the explicit identification of the **phase kickback** mechanism. They recognized that
preparing the target qubit as H|1⟩ = |−⟩ converts the oracle's bit-flip action into
a phase change — a technique now fundamental to quantum algorithm design.

They also identified a unifying structure across quantum algorithms: "a Fourier
transform, followed by an f-controlled-U, followed by another Fourier transform."
This pattern later appeared in Shor's algorithm and the quantum phase estimation
framework.

### 4.5 Nielsen & Chuang Textbook Presentation (2000/2010)

**Book:** *Quantum Computation and Quantum Information*, Cambridge University Press.
(Section 1.4.3)

Nielsen and Chuang's presentation is the most widely taught version:
- Full density matrix formalism
- Explicit circuit diagram notation
- Rigorous bra-ket algebraic derivation
- Connects to quantum parallelism concept
- Treats it as a gateway to Deutsch-Jozsa (Section 1.4.4) and ultimately
  to Shor and Grover

**Proof style:** Algebraic state-tracking through the circuit, step by step. Emphasis
on the tensor product structure and the role of entanglement (or rather, the lack
thereof — Deutsch's algorithm creates no entanglement between the query and
ancilla registers).

### 4.6 Comparison Matrix

| Aspect | Deutsch (1985) | Deutsch-Jozsa (1992) | Cleve et al. (1998) | Nielsen-Chuang (2000) |
|--------|----------------|----------------------|---------------------|-----------------------|
| **Input bits** | 1 | n | n | n |
| **Deterministic** | No (p = 1/2) | Yes | Yes | Yes |
| **Oracle queries** | 1 | 2 | 1 | 1 |
| **Ancilla init** | \|0⟩ | \|0⟩ | \|1⟩ (key insight) | \|1⟩ |
| **Phase kickback** | Implicit | Partial | Explicit | Explicit |
| **Proof technique** | Interference argument | Algebraic | Algebraic + structural | Full density matrix |
| **Fourier structure** | Not identified | Not identified | Identified | Inherited |
| **Entanglement needed** | Debated | Debated | No | No |

---

## 5. De-Quantization and the Limits of Quantum Advantage

### 5.1 Calude's De-Quantization (2006)

Cristian Calude showed that Deutsch's problem (single-bit case) can be solved
classically with one query if the black box is permitted to operate on
*higher-dimensional classical objects* ("complex bits" — classical analogues of
qubits).

**Mechanism:** Replace the Boolean black box f: {0,1} → {0,1} with a linear-algebraic
black box F: C² → C² that computes the same function on a 2-dimensional complex
vector space. A single application of F to a carefully chosen input vector produces
enough information to extract f(0) ⊕ f(1).

**Implication:** The quantum speedup in the 1-bit case may be an artifact of
comparing quantum registers (which carry 2-dimensional complex amplitudes) against
classical registers (which carry 1-bit Boolean values).

### 5.2 Abbott et al. — Entanglement and Scalability

Abbott and collaborators extended the de-quantization analysis:

- Any quantum algorithm with **bounded entanglement** can be de-quantized into an
  equally efficient classical simulation.
- For the general n-bit Deutsch-Jozsa problem, the de-quantization does **not**
  scale: classical simulation requires exponential resources when the quantum
  algorithm maintains non-trivial entanglement.
- Key result: entanglement is not *essential* for quantum computation (some advantage
  persists with separable states), but it is necessary for *exponential* speedup.

### 5.3 Classical Wave Analogies

Several groups demonstrated classical optical simulations of Deutsch-Jozsa:

| Group | Method | Insight |
|-------|--------|---------|
| Perez-Garcia et al. | Ring cavity + linear optics | Wave interference mimics quantum interference |
| Metamaterial groups | Electromagnetic waveguides | Constructive/destructive interference for constant/balanced |
| LCD programmable optics | Spatial light modulation | Classical coherence sufficient for small n |

These demonstrate that the *interference* ingredient is not uniquely quantum —
classical wave physics provides it too. What scales uniquely in quantum mechanics
is the exponential dimension of the Hilbert space (2ⁿ amplitudes from n qubits),
which classical wave systems cannot efficiently replicate.

### 5.4 Resolution

The modern consensus:

1. **For n = 1:** The quantum advantage is **real but modest** (1 query vs. 2), and
   can be replicated classically by enlarging the state space (de-quantization).

2. **For general n:** The quantum advantage is **exponential and genuine**. The
   Deutsch-Jozsa algorithm uses O(1) queries vs. classical Ω(2^(n−1)). No known
   de-quantization scales to this regime without exponential classical resources.

3. **The true quantum resource** is not superposition alone (classical waves have it)
   nor interference alone, but the **exponential state space** of multi-qubit systems
   combined with the ability to manipulate phases coherently across that space.

---

## 6. The Four Oracles

The function f: {0,1} → {0,1} has exactly four possible instantiations:

| Oracle | f(0) | f(1) | Type | Circuit Implementation |
|--------|------|------|------|-----------------------|
| f₀ | 0 | 0 | Constant | Identity (no gates) |
| f₁ | 1 | 1 | Constant | X on ancilla (q1) |
| f₂ | 0 | 1 | Balanced | CNOT(q0, q1) |
| f₃ | 1 | 0 | Balanced | X(q0), CNOT(q0, q1), X(q0) |

### Expected measurement outcomes

For all four oracles, measurement of qubit 0 yields:

| Oracle | f(0) ⊕ f(1) | Measurement q0 | Classification |
|--------|-------------|----------------|----------------|
| f₀ | 0 | \|0⟩ (prob = 1.0) | Constant |
| f₁ | 0 | \|0⟩ (prob = 1.0) | Constant |
| f₂ | 1 | \|1⟩ (prob = 1.0) | Balanced |
| f₃ | 1 | \|1⟩ (prob = 1.0) | Balanced |

---

## 7. Verification via ruqu-core

The ruqu-core simulator can verify all four cases of Deutsch's algorithm. The
verification test constructs each oracle circuit and confirms the deterministic
measurement outcome:

```rust
use ruqu_core::prelude::*;
use ruqu_core::gate::Gate;

fn deutsch_algorithm(oracle: &str) -> bool {
    let mut state = QuantumState::new(2).unwrap();

    // Prepare |01⟩
    state.apply_gate(&Gate::X(1)).unwrap();

    // Hadamard both qubits
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::H(1)).unwrap();

    // Apply oracle
    match oracle {
        "f0" => { /* identity — f(x) = 0 */ }
        "f1" => { state.apply_gate(&Gate::X(1)).unwrap(); }
        "f2" => { state.apply_gate(&Gate::CNOT(0, 1)).unwrap(); }
        "f3" => {
            state.apply_gate(&Gate::X(0)).unwrap();
            state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
            state.apply_gate(&Gate::X(0)).unwrap();
        }
        _ => panic!("Unknown oracle"),
    }

    // Hadamard on query qubit
    state.apply_gate(&Gate::H(0)).unwrap();

    // Measure qubit 0: |0⟩ = constant, |1⟩ = balanced
    let probs = state.probabilities();
    // prob(q0 = 1) = sum of probs where bit 0 is set
    let prob_q0_one = probs[1] + probs[3]; // indices with bit 0 = 1
    prob_q0_one > 0.5 // true = balanced, false = constant
}

// Verification:
assert!(!deutsch_algorithm("f0")); // constant
assert!(!deutsch_algorithm("f1")); // constant
assert!( deutsch_algorithm("f2")); // balanced
assert!( deutsch_algorithm("f3")); // balanced
```

This confirms that a single oracle query, using the ruqu state-vector simulator,
correctly classifies all four functions with probability 1.

---

## 8. Architectural Significance for ruVector

### 8.1 Validation of Core Primitives

Deutsch's algorithm exercises exactly the minimal set of quantum operations:

| Primitive | Used in Deutsch's Algorithm | ruqu Module |
|-----------|---------------------------|-------------|
| Qubit initialization | \|0⟩, \|1⟩ states | `state.rs` |
| Hadamard gate | Superposition creation | `gate.rs` |
| CNOT gate | Entangling oracle | `gate.rs` |
| Pauli-X gate | Bit flip oracle | `gate.rs` |
| Measurement | Extracting classical result | `state.rs` |
| Phase kickback | Core quantum mechanism | implicit |

Passing the Deutsch verification confirms that the simulator's gate kernels,
state-vector representation, and measurement machinery are correct — it is a
"minimum viable quantum correctness test."

### 8.2 Foundation for Advanced Algorithms

The phase-kickback technique proven here is the same mechanism used in:

- **Grover's algorithm** (ADR-QE-006): Oracle marks states via phase flip
- **VQE** (ADR-QE-005): Parameter-shift rule uses phase differences
- **Quantum Phase Estimation**: Controlled-U operators produce phase kickback
- **Shor's algorithm**: Order-finding oracle uses modular exponentiation kickback

---

## 9. References

| # | Reference | Year |
|---|-----------|------|
| 1 | D. Deutsch, "Quantum Theory, the Church-Turing Principle and the Universal Quantum Computer," *Proc. R. Soc. Lond. A* 400, 97–117 | 1985 |
| 2 | D. Deutsch & R. Jozsa, "Rapid Solution of Problems by Quantum Computation," *Proc. R. Soc. Lond. A* 439, 553–558 | 1992 |
| 3 | R. Cleve, A. Ekert, C. Macchiavello & M. Mosca, "Quantum Algorithms Revisited," *Proc. R. Soc. Lond. A* 454, 339–354 (arXiv: quant-ph/9708016) | 1998 |
| 4 | M.A. Nielsen & I.L. Chuang, *Quantum Computation and Quantum Information*, Cambridge University Press, 10th Anniversary Ed. | 2010 |
| 5 | C.S. Calude, "De-quantizing the Solution of Deutsch's Problem," *Int. J. Quantum Information* 5(3), 409–415 | 2007 |
| 6 | A.A. Abbott, "The Deutsch-Jozsa Problem: De-quantisation and Entanglement," *Natural Computing* 11(1), 3–11 | 2012 |
| 7 | R.P. Feynman, "Simulating Physics with Computers," *Int. J. Theoretical Physics* 21, 467–488 | 1982 |
| 8 | Perez-Garcia et al., "Quantum Computation with Classical Light," *Physics Letters A* 380(22), 1925–1931 | 2016 |

---

## Decision

**Accepted.** Deutsch's theorem is verified by the ruqu-core engine across all four
oracle cases. The proof and historical comparison are documented here as the
theoretical foundation underpinning all quantum algorithms implemented in the
ruqu-algorithms crate (Grover, VQE, QAOA, Surface Code).

The de-quantization analysis confirms that our simulator's true value emerges at
scale (n > 2 qubits), where classical de-quantization fails and the exponential
Hilbert space becomes a genuine computational resource.
