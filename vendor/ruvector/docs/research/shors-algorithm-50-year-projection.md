# Shor's Algorithm in 50 Years: A Speculative Projection (2026 → 2076)

> **Context**: Peter Shor published his factoring algorithm in 1994. It is now
> 32 years old and has never been used to break a real cryptographic key. What
> does the *next* 50 years look like? This document extrapolates from current
> trends, ruQu's architectural patterns, and theoretical computer science to
> imagine where Shor's algorithm — and its successors — might be in 2076.

## 1. Where We Are Today (2026)

### 1.1 The State of Play

| Milestone | Year | Largest Number Factored | Qubits Used |
|-----------|------|------------------------|-------------|
| Shor's original paper | 1994 | Theoretical | 0 |
| First experimental demo | 2001 | 15 = 3 × 5 | 7 (NMR) |
| Photonic factoring | 2012 | 21 = 3 × 7 | 10 |
| IBM superconducting | 2019 | 35 = 5 × 7 | 16 |
| Variational hybrid | 2023 | 261,980,999 (claim disputed) | 10 |
| Current NISQ frontier | 2026 | ~1,000-10,000 range (noisy) | 50-100 |
| ruQu simulator | 2026 | ~32,767 (15-bit, clean sim) | 25 |

### 1.2 The Gap to RSA-2048

```
RSA-2048 requires factoring a 617-digit number.
Best classical: ~2^112 operations (General Number Field Sieve)
Shor's algorithm: ~4,096 logical qubits, ~10^9 gates
With surface code (d=23): ~4 million physical qubits
Current hardware: ~1,000 noisy physical qubits

Gap: ~4,000× in qubit count, ~10,000× in error rate improvement
```

## 2. Decade 1: 2026-2036 — The NISQ-to-Fault-Tolerant Transition

### 2.1 Predicted Hardware Trajectory

| Year | Physical Qubits | Error Rate | Logical Qubits | Factoring Capability |
|------|----------------|------------|-----------------|---------------------|
| 2026 | 1,000 | 10⁻³ | ~1 (barely) | 15-bit (demonstration) |
| 2028 | 5,000 | 5×10⁻⁴ | ~5 | 30-bit (academic) |
| 2030 | 10,000 | 10⁻⁴ | ~20-50 | 64-bit (RSA-64 falls) |
| 2033 | 50,000 | 5×10⁻⁵ | ~200 | 256-bit (ECDSA-128 threatened) |
| 2036 | 100,000 | 10⁻⁵ | ~1,000 | 512-bit (RSA-512 falls) |

### 2.2 The Variational Shortcut

The table above assumes standard Shor's. But variational approaches
(VQE-based factoring, QAOA-enhanced number field sieve) trade qubits
for classical computation:

```
Standard Shor's:     4,096 logical qubits for RSA-2048
Variational hybrid:  ~500-1,000 logical qubits + massive classical compute
```

**Prediction**: By 2032-2035, variational hybrid approaches factor RSA-1024
on ~10,000 physical qubits. Not because the quantum computer is big enough
for standard Shor's, but because the classical-quantum interplay finds a
more efficient decomposition.

ruQu's VQE + 256-tile fabric + adaptive coherence gating is exactly this
architecture at 25-qubit scale. At 10,000 qubits, the same software
framework orchestrates the attack.

### 2.3 The Crypto Migration Race

```
Timeline:
  2026: NIST publishes FIPS 203/204/205 (ML-KEM, ML-DSA, SLH-DSA)
  2027-2030: Enterprise migration begins (banks, governments)
  2030: RSA-64 falls to quantum computers
  2031-2033: Consumer migration (browsers, phones, IoT)
  2033: ECDSA-128 equivalent threatened
  2035: RSA-512 falls
  2036: NIST deprecates all pre-quantum public key crypto
```

**The question**: Does migration complete before capability arrives?

**Historical precedent**: SHA-1 was deprecated in 2011, but real attacks
emerged in 2017 (SHAttered). Migration took ~10 years. If quantum threats
materialize ~2033, and migration started ~2026, the race is tight.

## 3. Decade 2: 2036-2046 — Shor's Becomes Routine

### 3.1 Quantum Computing Matures

By 2040, quantum computers are expected to reach the "utility" phase:

| Metric | 2026 | 2040 (projected) |
|--------|------|-------------------|
| Logical qubits | ~1 | 10,000+ |
| Gate fidelity | 99.9% | 99.9999% |
| Coherence time | microseconds | seconds-minutes |
| Clock speed | kHz | MHz |
| Access model | Cloud (limited) | Cloud (commodity) |

### 3.2 Shor's Implications at Scale

```
By ~2038: RSA-2048 is factored by a quantum computer.
By ~2040: RSA-4096 is factored.
By ~2042: All classical public-key crypto is broken.
```

**But this is not the interesting part.**

The interesting part is what happens to Shor's algorithm itself.
In 50 years, Shor's algorithm will be viewed the way we view
Euclid's algorithm today — a foundational result that spawned
an entire field, but long since superseded by more powerful tools.

### 3.3 Post-Shor Algorithms

By 2040, we will likely have:

**Quantum algorithms for problems we don't yet know are vulnerable**:
- Lattice problems (currently "post-quantum safe" — but are they?)
- Isogeny-based crypto (SIDH already broken classically in 2022)
- Code-based crypto (McEliece — 45 years and still standing, but for how long?)
- Multivariate crypto (known quantum speedups exist but not polynomial-time breaks)

**Meta-algorithmic tools**:
- Quantum algorithm discovery by AI (using systems like ruQu's self-learning
  framework to *find new quantum algorithms* automatically)
- Quantum machine learning applied to cryptanalysis
- Hybrid quantum-classical attacks that don't map to any single "named" algorithm

### 3.4 The "Harvest Now, Decrypt Later" Reckoning

Data encrypted today with RSA/ECDSA and intercepted by adversaries will
be decryptable ~2038. This means:

```
Sensitive data encrypted in 2020-2030 with pre-quantum crypto:
  - Government secrets (classified for 25-75 years)
  - Medical records (protected for lifetime + 50 years in some jurisdictions)
  - Financial records (retention: 7-25 years)
  - Diplomatic communications
  - Corporate trade secrets

All of this becomes readable when Shor's becomes practical.
```

**This is not a future problem. It is a present problem with a future deadline.**

## 4. Decade 3: 2046-2056 — The Post-Cryptographic Era

### 4.1 Cryptography Transforms

By 2050, the cryptographic landscape will look fundamentally different:

**Symmetric crypto survives** (with larger keys):
- AES-256 → AES-512 or successor (Grover reduces to 256-bit security)
- SHA-3-512 → SHA-4-1024 or successor
- Symmetric primitives are "quantum-resistant" with key doubling

**Public-key crypto is entirely lattice/code/hash-based**:
- ML-KEM-1024 or successor (if lattices survive)
- Hash-based signatures (SLH-DSA descendants — provably secure under hash assumptions)
- Code-based encryption (McEliece descendants)
- Possibly: quantum key distribution (QKD) for highest-security channels

**Or — more radically**:

### 4.2 Quantum Cryptography Replaces Classical

If quantum hardware is ubiquitous by 2050:

```
Today (2026):
  Security = computational hardness (factoring, lattices)
  Assumption: adversary has limited compute

2050:
  Security = physical law (quantum mechanics)
  Assumption: adversary cannot violate physics
```

**Quantum Key Distribution (QKD)**: Information-theoretically secure key
exchange. No computational assumption. Security proven by quantum mechanics.
Already deployed in limited settings (China's 4,600km QKD network, 2022).

**Quantum money**: Unforgeable currency based on no-cloning theorem.
Theoretical since 1983 (Wiesner), practical implementation by 2050.

**Quantum signatures**: Signatures where forgery is physically impossible,
not just computationally hard.

### 4.3 Shor's Algorithm Becomes a Teaching Example

By 2050, Shor's algorithm is:
- Taught in undergraduate CS courses (like RSA is today)
- Historically interesting but not "cutting edge"
- Superseded by more efficient quantum factoring algorithms
- A component in larger quantum algorithm suites

The research frontier will have moved to:
- Quantum algorithms for NP-hard optimization
- Quantum machine learning with provable advantages
- Quantum simulation of physical systems (chemistry, materials)
- Quantum error correction beyond surface codes (topological, LDPC)
- Fault-tolerant quantum computing at scale

## 5. Decade 4-5: 2056-2076 — Shor's Algorithm at 80 Years Old

### 5.1 The Most Likely Scenario

```
2076 view of Shor's algorithm:

"Shor's 1994 factoring algorithm was the first polynomial-time quantum
algorithm for a problem believed to be classically hard. It triggered
the post-quantum cryptography migration of the 2020s-2030s and remains
a foundational result in quantum complexity theory. Modern quantum
computers can factor million-digit numbers in seconds using descendants
of Shor's approach, but this capability has been irrelevant to
cryptography since the completion of the PQC migration in ~2040.

Shor's lasting impact was not the algorithm itself but the
demonstration that quantum computers could solve problems outside BQP
as classically understood, which opened the field of quantum
cryptanalysis and ultimately led to the physics-based security
paradigm that replaced computational hardness assumptions."

— Hypothetical textbook, 2076
```

### 5.2 The Wildcard Scenarios

#### Wildcard 1: Lattice Problems Fall to Quantum Algorithms

If someone discovers a quantum polynomial-time algorithm for SVP/LWE
(the basis of current post-quantum crypto), then:

```
2040s: Second "crypto emergency" — migrate from lattice-based to ???
2050s: Only hash-based and code-based crypto survive
2060s: Possibly only information-theoretic security (QKD, one-time pads)
```

**Probability**: Low (~10-20%), but non-zero. Lattice problems have a
different structure from factoring, and quantum algorithms for them
are an active research area.

#### Wildcard 2: Quantum Computing Hits a Wall

If quantum hardware cannot scale beyond ~10,000 logical qubits due to
fundamental engineering constraints:

```
2040: RSA-2048 falls (barely — requires most of the world's quantum compute)
2050: RSA-4096 still standing
2060: Hybrid crypto (classical + quantum) becomes the norm
2076: Shor's algorithm works but is resource-constrained, not universal
```

**Probability**: Moderate (~20-30%). There may be engineering limits
we haven't encountered yet.

#### Wildcard 3: Post-Quantum Crypto Has Classical Breaks

If ML-KEM or ML-DSA falls to a *classical* algorithm (like SIDH fell
to Castryck-Decru in 2022):

```
2030s: Emergency re-migration to backup PQC schemes
2040s: Diversified crypto stack (multiple independent assumptions)
2076: Security based on algorithm diversity, not single hard problem
```

**Probability**: Moderate for specific schemes (~30%), low for all
lattice-based schemes simultaneously (~5%).

#### Wildcard 4: Breakthrough in Quantum Error Correction

If a radically more efficient QEC scheme is discovered (e.g., requiring
only 10:1 physical-to-logical ratio instead of 1000:1):

```
2030: 100,000 physical qubits → 10,000 logical qubits (vs. 100 today)
2032: RSA-2048 falls a decade early
2035: All classical public-key crypto broken
2040: Quantum supremacy in optimization, simulation, ML — not just crypto
```

**Probability**: Low-moderate (~15-25%). Surface codes are known to be
suboptimal; LDPC and topological codes are improving rapidly.

## 6. How ruQu Positions for This Future

### 6.1 Decade 1 (2026-2036): Simulation and Preparation

ruQu's 25-qubit simulator validates attack circuits and develops the
software stack. As hardware scales to 100-1,000 qubits, ruQu's
architecture (256-tile fabric, surface code QEC, three-filter pipeline)
transfers directly to hardware backends.

**Key deliverable**: Variational factoring proof-of-concept that
demonstrates the hybrid classical-quantum attack framework works.

### 6.2 Decade 2 (2036-2046): Hardware Integration

ruQu's fabric architecture maps to real quantum hardware:
- Each tile → a quantum processing unit (QPU)
- TileZero → classical controller
- Three-filter pipeline → real-time coherence monitoring
- Witness chain → auditable quantum computation

**Key deliverable**: First open-source framework for monitored,
auditable quantum cryptanalysis on real hardware.

### 6.3 Decade 3+ (2046-2076): Legacy and Evolution

ruQu's architectural patterns — coherence gating, structural analysis,
anytime-valid testing — become standard practice in quantum computing,
not just cryptanalysis. The *defensive* applications (monitoring quantum
computer health, certifying computation correctness) outlast the
*offensive* applications (which become unnecessary after PQC migration).

**Key deliverable**: Coherence gating becomes an industry standard
for quantum computer reliability, independent of cryptanalysis.

## 7. The Deepest Question: Does Shor's Algorithm Become Irrelevant?

### 7.1 Yes — For Cryptography

By 2076, Shor's is irrelevant to cryptography because:
1. PQC migration completed decades ago
2. Quantum key distribution handles the highest-security use cases
3. No one uses RSA/ECDSA for anything important

### 7.2 No — For Science

By 2076, Shor's is *more* relevant to science than ever because:
1. It proved that quantum computers can solve "hard" problems efficiently
2. It motivated the entire field of quantum complexity theory
3. Its techniques (quantum Fourier transform, phase estimation) underpin
   hundreds of later algorithms
4. It drove the largest coordinated cryptographic migration in history

### 7.3 The Analogy

Shor's algorithm in 2076 will be like the **Enigma break in 2026**:

- Historically pivotal (changed the course of cryptography)
- Technically elegant (still taught and admired)
- Practically irrelevant (no one uses Enigma)
- Culturally significant (reminded us that "secure" is always relative)

The lesson Shor's teaches — that security assumptions can be invalidated
by new models of computation — will be more relevant in 2076 than ever,
as we face whatever the *next* computational paradigm brings.

## 8. Conclusion: The 50-Year Arc

```
1994: Shor publishes. Theorists panic. Practitioners shrug.
2001: First demo (15 = 3 × 5). Interesting but irrelevant.
2020s: NIST PQC competition. Migration begins slowly.
2026: ruQu implements the full software stack at 25 qubits.
2030s: Hardware reaches 10,000+ physical qubits. RSA-64 falls.
2035: Enterprise PQC migration urgency peaks.
2038: RSA-2048 factored. Headlines, but migration mostly complete.
2040s: All pre-quantum public-key crypto broken. Shor's is routine.
2050s: Quantum computers are commodity infrastructure.
2060s: Shor's is a textbook example, not a research frontier.
2076: Shor's algorithm is 82 years old. Still beautiful.
        Still taught. Completely harmless.
        The world moved on because it had to — and it did.
```

The real legacy of Shor's algorithm is not the numbers it will factor.
It is the *urgency* it created to build quantum-resistant systems
*before* the capability arrived. That urgency, right now in 2026,
is the most important thing about Shor's algorithm — more important
than any future factorization.
