# Search Results

**Query:** Google Willow chip technical specifications: qubit count, error rates, coherence times, gate fidelities, operating temperature, fabrication process, quantum volume, CLOPS benchmarks
**Date:** 2025-09-28T21:26:19.511Z

## Answer

**Google's Willow quantum chip** is a 105-qubit superconducting processor that sets new standards in quantum error correction and computational benchmarks. Below are the detailed technical specifications and performance metrics, with direct citations for each parameter:

---

### **Qubit Count**
- **105 physical qubits** arranged in a square grid (surface code architecture)[2][3][7].

### **Error Rates**
- **Single-qubit gate error (mean, simultaneous):**
  - **0.035% ± 0.029%** (QEC mode, Chip 1)
  - **0.036% ± 0.013%** (RCS mode, Chip 2)[2]
- **Two-qubit gate error (mean, simultaneous):**
  - **0.33% ± 0.18%** (CZ, QEC mode, Chip 1)
  - **0.14% ± 0.052%** (iswap-like, RCS mode, Chip 2)[2]
- **Measurement error (mean, simultaneous):**
  - **0.77% ± 0.21%** (QEC mode, Chip 1)
  - **0.67% ± 0.51%** (RCS mode, Chip 2)[2]
- **Logical error rates:** Exponential suppression demonstrated; error rate halved each time the surface code distance is increased (e.g., from 3×3 to 5×5 to 7×7 grids)[1][2][7].

### **Coherence Times**
- **T₁ time (energy relaxation time):**
  - **68 µs ± 13 µs** (QEC mode, Chip 1)
  - **98 µs ± 32 µs** (RCS mode, Chip 2)
  - **Approaching 100 µs** (average, ~5× improvement over Sycamore’s 20 µs)[2][3][7]
- **T₂ times** are not explicitly reported, but T₁ is the primary quoted metric.

### **Gate Fidelities**
- **Single-qubit gate fidelity:** >99.96%
- **Two-qubit gate fidelity:** >99.67% (CZ), >99.86% (iswap-like)[2]
- These are among the highest reported for large-scale superconducting chips.

### **Operating Temperature**
- **Millikelvin regime** (close to absolute zero, typically ~10–20 mK), as required for superconducting transmon qubits[4][7].

### **Fabrication Process**
- **Superconducting transmon qubits** fabricated in Google’s dedicated quantum chip facility in Santa Barbara, California[2][3][4][7].
- Uses advanced materials engineering, electromagnetic shielding, and custom packaging to minimize noise and maximize coherence[4].

### **Quantum Volume**
- **Not explicitly reported** in the available sources. However, Willow’s performance on holistic benchmarks (error correction and RCS) is considered best-in-class for its generation[2][7].
- **Application performance metric:** Λ₃,₅,₇ = 2.14 ± 0.02 (a system-level benchmark for error correction cycles)[2].

### **CLOPS Benchmarks**
- **Not directly reported** for Willow. Instead, Google emphasizes holistic system benchmarks:
  - **Error correction cycles per second:** 909,000 (surface code cycle = 1.1 µs)[2]
  - **Circuit repetitions per second:** 63,000 (RCS mode)[2]

### **Random Circuit Sampling (RCS) Benchmark**
- **Willow completed an RCS task in under 5 minutes** that would take the world’s fastest classical supercomputer (Frontier) an estimated **10²⁵ years**[1][2][7].
- **XEB fidelity (cross-entropy benchmarking):** 0.1% for 103 qubits at depth 40[2].

### **Other Key Metrics**
- **Average connectivity:** 3.47 (4-way typical, 2D grid)[2][3][7]
- **Reset options:** Multi-level reset (|1⟩ state and above), leakage removal (|2⟩ state only)[2]
- **Measurement rate:** High, supporting rapid error correction and benchmarking cycles[2][7]

---

### **Summary Table**

| Metric                         | Value/Description                                   | Source(s)      |
|------------------------------- |----------------------------------------------------|----------------|
| **Qubit count**                | 105                                                | [2][3][7]   |
| **Qubit type**                 | Superconducting transmon                           | [3][4][7]      |
| **Single-qubit gate error**    | 0.035% (QEC), 0.036% (RCS)                         | [2]            |
| **Two-qubit gate error**       | 0.33% (CZ, QEC), 0.14% (iswap, RCS)                | [2]            |
| **Measurement error**          | 0.77% (QEC), 0.67% (RCS)                           | [2]            |
| **T₁ coherence time**          | 68–98 µs (mean), up to ~100 µs                     | [2][3][7]   |
| **Gate fidelities**            | >99.96% (1Q), >99.67% (2Q)                         | [2]            |
| **Operating temperature**      | ~10–20 mK (millikelvin)                            | [4][7]      |
| **Fabrication**                | Google Santa Barbara facility, advanced shielding  | [2][3][4][7]   |
| **Quantum volume**             | Not reported; best-in-class on system benchmarks   | [2][7]      |
| **CLOPS**                      | Not reported; error correction cycles: 909,000/s   | [2]            |
| **RCS benchmark**              | 5 min (Willow) vs. 10²⁵ years (Frontier)           | [1][2][7] |
| **XEB fidelity**               | 0.1% (103 qubits, depth 40)                        | [2]            |
| **Connectivity**               | 3.47 (4-way typical)                               | [2][3][7]      |

---

### **Context and Limitations**
- **Willow is the first chip to demonstrate exponential error suppression (“below threshold”) as qubit count increases, a milestone for scalable quantum error correction**[1][2][7].
- **Logical error rates** are still above the threshold needed for large-scale, fault-tolerant quantum algorithms, but Willow’s results represent a major step forward[3].
- **Quantum volume and CLOPS** (circuit layer operations per second) are not directly reported, but Willow’s holistic system performance is considered state-of-the-art for its era[2][7].
- **Commercial quantum advantage** (solving practical real-world problems faster than classical computers) has not yet been achieved; Willow’s results are a demonstration of quantum supremacy on synthetic benchmarks.

---

**In summary:**  
Willow is a 105-qubit superconducting chip with best-in-class error rates, coherence times approaching 100 µs, and system-level performance that sets new records in quantum error correction and random circuit sampling. It operates at millikelvin temperatures, is fabricated in Google’s custom facility, and represents a major milestone toward scalable, fault-tolerant quantum computing[2][3][7].

## Citations

### 1. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 2. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

**URL:** https://en.wikipedia.org/wiki/Willow_processor

The **Willow processor** is a 105-qubit superconducting quantum computing processor developed by Google Quantum AI and manufactured in Santa Barbara, California.

## Overview

On December 9, 2024, Google Quantum AI announced Willow in a *Nature* paper and company blogpost, and claiming two accomplishments: First, that Willow can reduce errors exponentially as the number of qubits is scaled, achieving below threshold quantum error correction. Second, that Willow completed a Random Circuit Sampling (RCS) benchmark task in 5 minutes that would take today's fastest supercomputers 10 septillion (10^25^) years.

Willow is constructed with a square grid of superconducting transmon physical qubits. Improvements over past work were attributed to improved fabrication techniques, participation ratio engineering, and circuit parameter optimization.

Willow prompted optimism in accelerating applications in pharmaceuticals, material science, logistics, drug discovery, and energy grid allocation. Popular media responses discussed its risk in breaking cryptographic systems, but a Google spokesman said that they were still at least 10 years out from breaking RSA. Hartmut Neven, founder and lead of Google Quantum AI, told the BBC that Willow would be used in practical applications, and in the announcement blogpost expressed the belief that advanced AI will benefit from quantum computing.

Willow follows the release of Foxtail in 2017, Bristlecone in 2018, and Sycamore in 2019. Willow has twice as many qubits as Sycamore and improves upon T1 coherence time from Sycamore's 20 microseconds to 100 microseconds. Willow's 105 qubits have an average connectivity of 3.47.

Hartmut Neven, founder of Google Quantum AI, prompted controversy by claiming that the success of Willow "lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch."... ## Criticism

Per Google company's claim, Willow is the first chip to achieve below threshold quantum error correction.

However, a number of critics have pointed out several limitations:

- The logical error rates reported (around 0.14% per cycle) remain orders of magnitude above the 10^−6^ levels believed necessary for running meaningful, large-scale quantum algorithms.
- To date, demonstrations have been limited to quantum memory and the preservation of logical qubits—without yet showing below‑threshold performance of logical gate operations required for universal fault‑tolerant computation.
- Media coverage has been accused of overstating Willow’s practical significance; although error suppression scales exponentially with qubit count, no large‑scale quantum algorithms or commercial applications have yet been demonstrated on Willow.
- Observers caution that achieving below‑threshold error correction is only one milestone on the path to practical quantum computing—further hardware improvements (lower physical error rates) and vastly larger qubit arrays will be required before industrially relevant problem‑solving is possible.
- Some experts note that Willow remains a research prototype within the Noisy intermediate-scale quantum era, still far from delivering the practical, fault‑tolerant performance required for real‑world applications.

## References

### 3. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 4. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 5. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 6. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 7. Meet Willow, our state-of-the-art quantum chip - Google Blog

**URL:** https://blog.google/technology/research/google-willow-quantum-chip/

# Meet Willow, our state-of-the-art quantum chip

*Last updated: June 12, 2025*

Today I’m delighted to announce Willow, our latest quantum chip. Willow has state-of-the-art performance across a number of metrics, enabling two major achievements.

- The first is that Willow can reduce errors exponentially as we scale up using

*more*qubits. This cracks a key challenge in quantum error correction that the field has pursued for almost 30 years.

- Second, Willow performed a standard benchmark computation in under five minutes that would take one of today’s fastest supercomputers 10 septillion (that is, 10

25) years — a number that vastly exceeds the age of the Universe.

The Willow chip is a major step on a journey that began over 10 years ago. When I founded Google Quantum AI in 2012, the vision was to build a useful, large-scale quantum computer that could harness quantum mechanics — the “operating system” of nature to the extent we know it today — to benefit society by advancing scientific discovery, developing helpful applications, and tackling some of society's greatest challenges. As part of Google Research, our team has charted a long-term roadmap, and Willow moves us significantly along that path towards commercially relevant applications.... ## Exponential quantum error correction — below threshold!

Errors are one of the greatest challenges in quantum computing, since qubits, the units of computation in quantum computers, have a tendency to rapidly exchange information with their environment, making it difficult to protect the information needed to complete a computation. Typically the more qubits you use, the more errors will occur, and the system becomes classical.

Today in Nature, we published results showing that

**the more qubits we use in Willow, the more we** *reduce* ** errors** **, and the more quantum the system becomes**. We tested ever-larger arrays of physical qubits, scaling up from a grid of 3x3 encoded qubits, to a grid of 5x5, to a grid of 7x7 — and each time, using our latest advances in quantum error correction, we were able to cut the error rate in half. In other words, we achieved an exponential reduction in the error rate. This historic accomplishment is known in the field as “below threshold” — being able to drive errors down while scaling up the number of qubits. You must demonstrate being below threshold to show real progress on error correction, and this has been an outstanding challenge since quantum error correction was introduced by Peter Shor in 1995.

There are other scientific “firsts” involved in this result as well. For example, it’s also one of the first compelling examples of real-time error correction on a superconducting quantum system — crucial for any useful computation, because if you can’t correct errors fast enough, they ruin your computation before it’s done. And it’s a "beyond breakeven" demonstration, where our arrays of qubits have longer lifetimes than the individual physical qubits do, an unfakable sign that error correction is improving the system overall.

As the first system below threshold, this is the most convincing prototype for a scalable logical qubit built to date. It’s a strong sign that useful, very large quantum computers can indeed be built. Willow brings us closer to running practical, commercially-relevant algorithms that can’t be replicated on conventional computers.... ## 10 septillion years on one of today’s fastest supercomputers

As a measure of Willow’s performance, we used the random circuit sampling (RCS) benchmark. Pioneered by our team and now widely used as a standard in the field, RCS is the classically hardest benchmark that can be done on a quantum computer today. You can think of this as an entry point for quantum computing — it checks whether a quantum computer is doing something that couldn’t be done on a classical computer. Any team building a quantum computer should check first if it can beat classical computers on RCS; otherwise there is strong reason for skepticism that it can tackle more complex quantum tasks. We’ve consistently used this benchmark to assess progress from one generation of chip to the next — we reported Sycamore results in October 2019 and again recently in October 2024.

Willow’s performance on this benchmark is astonishing: It performed a computation in under five minutes that would take one of today’s fastest supercomputers 10

25 or 10 septillion years. If you want to write it out, it’s 10,000,000,000,000,000,000,000,000 years. This mind-boggling number exceeds known timescales in physics and vastly exceeds the age of the universe. It lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch.

These latest results for Willow, as shown in the plot below, are our best so far, but we’ll continue to make progress.

Computational costs are heavily influenced by available memory. Our estimates therefore consider a range of scenarios, from an ideal situation with unlimited memory (▲) to a more practical, embarrassingly parallelizable implementation on GPUs (⬤).

Our assessment of how Willow outpaces one of the world’s most powerful classical supercomputers, Frontier, was based on conservative assumptions. For example, we assumed full access to secondary storage, i.e., hard drives, without any bandwidth overhead — a generous and unrealistic allowance for Frontier. Of course, as happened after we announced the first beyond-classical computation in 2019, we expect classical computers to keep improving on this benchmark, but the rapidly growing gap shows that quantum processors are peeling away at a double exponential rate and will continue to vastly outperform classical computers as we scale up.... ## State-of-the-art performance

Willow was fabricated in our new, state-of-the-art fabrication facility in Santa Barbara — one of only a few facilities in the world built from the ground up for this purpose. System engineering is key when designing and fabricating quantum chips: All components of a chip, such as single and two-qubit gates, qubit reset, and readout, have to be simultaneously well engineered and integrated. If any component lags or if two components don't function well together, it drags down system performance. Therefore, maximizing system performance informs all aspects of our process, from chip architecture and fabrication to gate development and calibration. The achievements we report assess quantum computing systems holistically, not just one factor at a time.

We’re focusing on quality, not just quantity — because just producing larger numbers of qubits doesn’t help if they’re not high enough quality. With 105 qubits, Willow now has best-in-class performance across the two system benchmarks discussed above: quantum error correction and random circuit sampling. Such algorithmic benchmarks are the best way to measure overall chip performance. Other more specific performance metrics are also important; for example, our T

1 times, which measure how long qubits can retain an excitation — the key quantum computational resource — are now approaching 100 µs (microseconds). This is an impressive ~5x improvement over our previous generation of chips. If you want to evaluate quantum hardware and compare across platforms, here is a table of key specifications:

Willow’s performance across a number of metrics.... ## What’s next with Willow and beyond

The next challenge for the field is to demonstrate a first "useful, beyond-classical" computation on today's quantum chips that is relevant to a real-world application. We’re optimistic that the Willow generation of chips can help us achieve this goal. So far, there have been two separate types of experiments. On the one hand, we’ve run the RCS benchmark, which measures performance against classical computers but has no known real-world applications. On the other hand, we’ve done scientifically interesting simulations of quantum systems, which have led to new scientific discoveries but are still within the reach of classical computers. Our goal is to do both at the same time — to step into the realm of algorithms that are beyond the reach of classical computers

**and** that are useful for real-world, commercially relevant problems.

Random circuit sampling (RCS), while extremely challenging for classical computers, has yet to demonstrate practical commercial applications.

We invite researchers, engineers, and developers to join us on this journey by checking out our open source software and educational resources, including our new course on Coursera, where developers can learn the essentials of quantum error correction and help us create algorithms that can solve the problems of the future.

My colleagues sometimes ask me why I left the burgeoning field of AI to focus on quantum computing. My answer is that both will prove to be the most transformational technologies of our time, but advanced AI will significantly benefit from access to quantum computing. This is why I named our lab Quantum AI. Quantum algorithms have fundamental scaling laws on their side, as we’re seeing with RCS. There are similar scaling advantages for many foundational computational tasks that are essential for AI. So quantum computation will be indispensable for collecting training data that’s inaccessible to classical machines, training and optimizing certain learning architectures, and modeling systems where quantum effects are important. This includes helping us discover new medicines, designing more efficient batteries for electric cars, and accelerating progress in fusion and new energy alternatives. Many of these future game-changing applications won’t be feasible on classical computers; they’re waiting to be unlocked with quantum computing.

### 8. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 9. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 10. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 11. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

**URL:** https://en.wikipedia.org/wiki/Willow_processor

The **Willow processor** is a 105-qubit superconducting quantum computing processor developed by Google Quantum AI and manufactured in Santa Barbara, California.

## Overview

On December 9, 2024, Google Quantum AI announced Willow in a *Nature* paper and company blogpost, and claiming two accomplishments: First, that Willow can reduce errors exponentially as the number of qubits is scaled, achieving below threshold quantum error correction. Second, that Willow completed a Random Circuit Sampling (RCS) benchmark task in 5 minutes that would take today's fastest supercomputers 10 septillion (10^25^) years.

Willow is constructed with a square grid of superconducting transmon physical qubits. Improvements over past work were attributed to improved fabrication techniques, participation ratio engineering, and circuit parameter optimization.

Willow prompted optimism in accelerating applications in pharmaceuticals, material science, logistics, drug discovery, and energy grid allocation. Popular media responses discussed its risk in breaking cryptographic systems, but a Google spokesman said that they were still at least 10 years out from breaking RSA. Hartmut Neven, founder and lead of Google Quantum AI, told the BBC that Willow would be used in practical applications, and in the announcement blogpost expressed the belief that advanced AI will benefit from quantum computing.

Willow follows the release of Foxtail in 2017, Bristlecone in 2018, and Sycamore in 2019. Willow has twice as many qubits as Sycamore and improves upon T1 coherence time from Sycamore's 20 microseconds to 100 microseconds. Willow's 105 qubits have an average connectivity of 3.47.

Hartmut Neven, founder of Google Quantum AI, prompted controversy by claiming that the success of Willow "lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch."... ## Criticism

Per Google company's claim, Willow is the first chip to achieve below threshold quantum error correction.

However, a number of critics have pointed out several limitations:

- The logical error rates reported (around 0.14% per cycle) remain orders of magnitude above the 10^−6^ levels believed necessary for running meaningful, large-scale quantum algorithms.
- To date, demonstrations have been limited to quantum memory and the preservation of logical qubits—without yet showing below‑threshold performance of logical gate operations required for universal fault‑tolerant computation.
- Media coverage has been accused of overstating Willow’s practical significance; although error suppression scales exponentially with qubit count, no large‑scale quantum algorithms or commercial applications have yet been demonstrated on Willow.
- Observers caution that achieving below‑threshold error correction is only one milestone on the path to practical quantum computing—further hardware improvements (lower physical error rates) and vastly larger qubit arrays will be required before industrially relevant problem‑solving is possible.
- Some experts note that Willow remains a research prototype within the Noisy intermediate-scale quantum era, still far from delivering the practical, fault‑tolerant performance required for real‑world applications.

## References

### 12. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 13. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 14. Google Willow Quantum Chip - Revolutionizing Quantum Computing

**URL:** https://googlewillow.org

A groundbreaking quantum chip that promises to transform computational capabilities across multiple industries

Discover Google Willow, a quantum chip that outperforms supercomputers and reduces errors exponentially, paving the way for practical quantum computing applications

The Google Willow quantum chip is a significant breakthrough in the field of quantum computing, with the following capabilities:

Willow can complete certain computational tasks in less than 5 minutes, which would take the most powerful supercomputers billions of years to accomplish. In the RCS standard test benchmark, Willow can complete the calculation test in 5 minutes, while the fastest supercomputers today would need at least 10 septillion years (10^25 years).

Willow can complete certain computational tasks in less than 5 minutes, which would take the most powerful supercomputers billions of years to accomplish. In the RCS standard test benchmark, Willow can complete the calculation test in 5 minutes, while the fastest supercomputers today would need at least 10 septillion years (10^25 years).... Willow features 105 quantum bits (qubits) and reduces error rates by half while expanding the scale of qubits. Google's research published in Nature shows that they tested quantum bit grids of different sizes, from 3x3 to 5x5 to 7x7, each time halving the error rate.

Willow organizes qubits into a grid configuration known as 'logical qubits,' enabling real-time error correction. The larger the scale, the better the error correction effect; if the scale is sufficient, the error rate can approach zero.

Willow operates more efficiently at extremely low temperatures. Qubits are powerful but extremely fragile, requiring operation in an ultra-low temperature environment close to absolute zero to avoid external temperature influences.

Julian Kelly, Google's Quantum AI Hardware Director, stated, 'This will push the boundaries of science and exploration. With future commercial applications in medicine, batteries, and nuclear fusion, we are excited to solve problems that were previously unsolvable.'... Delve into the world of Google Willow, where we uncover the latest advancements in quantum computing and their implications for the future of technology

The introduction of Google Willow quantum chip is set to revolutionize various sectors with its unprecedented quantum computing capabilities, offering solutions to complex problems and enhancing efficiency in the following industries:

Quantum computing poses a potential threat to traditional encryption methods used in cryptocurrencies, necessitating the development of quantum-resistant encryption to secure digital assets against future threats.

Willow's ability to simulate molecular interactions at the atomic level can significantly accelerate drug discovery, reducing development timelines and costs, and potentially leading to breakthroughs in treatment.

Quantum computing can enhance AI capabilities by processing vast amounts of data more efficiently, leading to advancements in deep learning and data analysis, and solving complex problems beyond the reach of classical computers.

Nuclear fusion research and other energy technologies can benefit from quantum computing's ability to model complex physical dynamics, potentially leading to more efficient and sustainable energy solutions.... Quantum computing can optimize investment portfolios and provide precise risk analysis for financial institutions, offering a significant advantage in the competitive financial sector.

The Google Willow quantum chip, with 105 qubits, excels in error correction and random circuit sampling, completing tasks in minutes that would take supercomputers over 10^25 years.

The Google Willow quantum chip features 105 physical qubits and achieves best-in-class performance in quantum error correction and random circuit sampling. The Willow chip has accomplished two major milestones: It significantly reduced errors while increasing the number of qubits. It completed a standard benchmark calculation in under 5 minutes, whereas the fastest supercomputers today would require over 10^25 years to perform the same task.

The Willow chip's groundbreaking achievements include: Achieving 'below-threshold' error rates, meaning it reduced error rates while increasing qubit count. This has been a goal in the quantum computing field for nearly 30 years.... In the random circuit sampling (RCS) benchmark test, the Willow chip excelled, completing a calculation that would take the fastest supercomputers today over 10^25 years to perform

Google Willow represents a monumental breakthrough in quantum computing technology. This innovative quantum chip has demonstrated unprecedented capabilities that push the boundaries of computational science. By achieving 'below threshold' performance, Willow can dramatically reduce errors while scaling up the number of qubits. This is a critical advancement that brings us closer to practical, commercially viable quantum computing applications.

The Google Willow quantum chip surpasses its predecessors by achieving a 'below-threshold' error rate while increasing the number of qubits. This means it can reduce errors as the system scales, which is a significant advancement in making quantum computing more reliable and practical for real-world applications.... The 105 qubits in the Willow chip are crucial as they allow for more complex computations and improved error correction. The number of qubits is directly related to the computational power of a quantum computer, with more qubits enabling the processing of more intricate problems.

The Willow chip uses a method of quantum error correction that involves encoding logical qubits across multiple physical qubits. This allows the system to detect and correct errors in real-time, which is essential for the stability and accuracy of quantum computations.

The Willow chip has the potential to revolutionize scientific research by enabling simulations and calculations that are currently impossible with classical computers. This could lead to new discoveries in materials science, quantum physics, and other fields.

Yes, the Willow chip's advanced computational abilities can be used to create more accurate climate models. This could help in understanding climate change patterns and developing strategies to mitigate its effects.... Quantum computers, including the Willow chip, have the potential to solve optimization problems much faster than classical computers. This could be particularly useful in logistics, supply chain management, and other areas where efficiency is critical.

As with any powerful technology, quantum computing raises ethical questions, particularly around data security and privacy. It's important to develop guidelines and regulations to ensure that quantum computing is used responsibly and ethically.

The Willow chip stands out for its ability to perform complex calculations at an unprecedented speed, completing tasks in minutes that would take supercomputers billions of years. This makes it one of the most efficient quantum computing technologies to date.

The Willow chip requires a highly controlled environment, including extremely low temperatures close to absolute zero, to function optimally. This means that significant infrastructure is needed to maintain the chip's operating conditions.

### 15. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 16. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 17. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

**URL:** https://en.wikipedia.org/wiki/Willow_processor

The **Willow processor** is a 105-qubit superconducting quantum computing processor developed by Google Quantum AI and manufactured in Santa Barbara, California.

## Overview

On December 9, 2024, Google Quantum AI announced Willow in a *Nature* paper and company blogpost, and claiming two accomplishments: First, that Willow can reduce errors exponentially as the number of qubits is scaled, achieving below threshold quantum error correction. Second, that Willow completed a Random Circuit Sampling (RCS) benchmark task in 5 minutes that would take today's fastest supercomputers 10 septillion (10^25^) years.

Willow is constructed with a square grid of superconducting transmon physical qubits. Improvements over past work were attributed to improved fabrication techniques, participation ratio engineering, and circuit parameter optimization.

Willow prompted optimism in accelerating applications in pharmaceuticals, material science, logistics, drug discovery, and energy grid allocation. Popular media responses discussed its risk in breaking cryptographic systems, but a Google spokesman said that they were still at least 10 years out from breaking RSA. Hartmut Neven, founder and lead of Google Quantum AI, told the BBC that Willow would be used in practical applications, and in the announcement blogpost expressed the belief that advanced AI will benefit from quantum computing.

Willow follows the release of Foxtail in 2017, Bristlecone in 2018, and Sycamore in 2019. Willow has twice as many qubits as Sycamore and improves upon T1 coherence time from Sycamore's 20 microseconds to 100 microseconds. Willow's 105 qubits have an average connectivity of 3.47.

Hartmut Neven, founder of Google Quantum AI, prompted controversy by claiming that the success of Willow "lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch."... ## Criticism

Per Google company's claim, Willow is the first chip to achieve below threshold quantum error correction.

However, a number of critics have pointed out several limitations:

- The logical error rates reported (around 0.14% per cycle) remain orders of magnitude above the 10^−6^ levels believed necessary for running meaningful, large-scale quantum algorithms.
- To date, demonstrations have been limited to quantum memory and the preservation of logical qubits—without yet showing below‑threshold performance of logical gate operations required for universal fault‑tolerant computation.
- Media coverage has been accused of overstating Willow’s practical significance; although error suppression scales exponentially with qubit count, no large‑scale quantum algorithms or commercial applications have yet been demonstrated on Willow.
- Observers caution that achieving below‑threshold error correction is only one milestone on the path to practical quantum computing—further hardware improvements (lower physical error rates) and vastly larger qubit arrays will be required before industrially relevant problem‑solving is possible.
- Some experts note that Willow remains a research prototype within the Noisy intermediate-scale quantum era, still far from delivering the practical, fault‑tolerant performance required for real‑world applications.

## References

### 18. Google Willow Quantum Chip - Revolutionizing Quantum Computing

**URL:** https://googlewillow.org

A groundbreaking quantum chip that promises to transform computational capabilities across multiple industries

Discover Google Willow, a quantum chip that outperforms supercomputers and reduces errors exponentially, paving the way for practical quantum computing applications

The Google Willow quantum chip is a significant breakthrough in the field of quantum computing, with the following capabilities:

Willow can complete certain computational tasks in less than 5 minutes, which would take the most powerful supercomputers billions of years to accomplish. In the RCS standard test benchmark, Willow can complete the calculation test in 5 minutes, while the fastest supercomputers today would need at least 10 septillion years (10^25 years).

Willow can complete certain computational tasks in less than 5 minutes, which would take the most powerful supercomputers billions of years to accomplish. In the RCS standard test benchmark, Willow can complete the calculation test in 5 minutes, while the fastest supercomputers today would need at least 10 septillion years (10^25 years).... Willow features 105 quantum bits (qubits) and reduces error rates by half while expanding the scale of qubits. Google's research published in Nature shows that they tested quantum bit grids of different sizes, from 3x3 to 5x5 to 7x7, each time halving the error rate.

Willow organizes qubits into a grid configuration known as 'logical qubits,' enabling real-time error correction. The larger the scale, the better the error correction effect; if the scale is sufficient, the error rate can approach zero.

Willow operates more efficiently at extremely low temperatures. Qubits are powerful but extremely fragile, requiring operation in an ultra-low temperature environment close to absolute zero to avoid external temperature influences.

Julian Kelly, Google's Quantum AI Hardware Director, stated, 'This will push the boundaries of science and exploration. With future commercial applications in medicine, batteries, and nuclear fusion, we are excited to solve problems that were previously unsolvable.'... Delve into the world of Google Willow, where we uncover the latest advancements in quantum computing and their implications for the future of technology

The introduction of Google Willow quantum chip is set to revolutionize various sectors with its unprecedented quantum computing capabilities, offering solutions to complex problems and enhancing efficiency in the following industries:

Quantum computing poses a potential threat to traditional encryption methods used in cryptocurrencies, necessitating the development of quantum-resistant encryption to secure digital assets against future threats.

Willow's ability to simulate molecular interactions at the atomic level can significantly accelerate drug discovery, reducing development timelines and costs, and potentially leading to breakthroughs in treatment.

Quantum computing can enhance AI capabilities by processing vast amounts of data more efficiently, leading to advancements in deep learning and data analysis, and solving complex problems beyond the reach of classical computers.

Nuclear fusion research and other energy technologies can benefit from quantum computing's ability to model complex physical dynamics, potentially leading to more efficient and sustainable energy solutions.... Quantum computing can optimize investment portfolios and provide precise risk analysis for financial institutions, offering a significant advantage in the competitive financial sector.

The Google Willow quantum chip, with 105 qubits, excels in error correction and random circuit sampling, completing tasks in minutes that would take supercomputers over 10^25 years.

The Google Willow quantum chip features 105 physical qubits and achieves best-in-class performance in quantum error correction and random circuit sampling. The Willow chip has accomplished two major milestones: It significantly reduced errors while increasing the number of qubits. It completed a standard benchmark calculation in under 5 minutes, whereas the fastest supercomputers today would require over 10^25 years to perform the same task.

The Willow chip's groundbreaking achievements include: Achieving 'below-threshold' error rates, meaning it reduced error rates while increasing qubit count. This has been a goal in the quantum computing field for nearly 30 years.... In the random circuit sampling (RCS) benchmark test, the Willow chip excelled, completing a calculation that would take the fastest supercomputers today over 10^25 years to perform

Google Willow represents a monumental breakthrough in quantum computing technology. This innovative quantum chip has demonstrated unprecedented capabilities that push the boundaries of computational science. By achieving 'below threshold' performance, Willow can dramatically reduce errors while scaling up the number of qubits. This is a critical advancement that brings us closer to practical, commercially viable quantum computing applications.

The Google Willow quantum chip surpasses its predecessors by achieving a 'below-threshold' error rate while increasing the number of qubits. This means it can reduce errors as the system scales, which is a significant advancement in making quantum computing more reliable and practical for real-world applications.... The 105 qubits in the Willow chip are crucial as they allow for more complex computations and improved error correction. The number of qubits is directly related to the computational power of a quantum computer, with more qubits enabling the processing of more intricate problems.

The Willow chip uses a method of quantum error correction that involves encoding logical qubits across multiple physical qubits. This allows the system to detect and correct errors in real-time, which is essential for the stability and accuracy of quantum computations.

The Willow chip has the potential to revolutionize scientific research by enabling simulations and calculations that are currently impossible with classical computers. This could lead to new discoveries in materials science, quantum physics, and other fields.

Yes, the Willow chip's advanced computational abilities can be used to create more accurate climate models. This could help in understanding climate change patterns and developing strategies to mitigate its effects.... Quantum computers, including the Willow chip, have the potential to solve optimization problems much faster than classical computers. This could be particularly useful in logistics, supply chain management, and other areas where efficiency is critical.

As with any powerful technology, quantum computing raises ethical questions, particularly around data security and privacy. It's important to develop guidelines and regulations to ensure that quantum computing is used responsibly and ethically.

The Willow chip stands out for its ability to perform complex calculations at an unprecedented speed, completing tasks in minutes that would take supercomputers billions of years. This makes it one of the most efficient quantum computing technologies to date.

The Willow chip requires a highly controlled environment, including extremely low temperatures close to absolute zero, to function optimally. This means that significant infrastructure is needed to maintain the chip's operating conditions.

### 19. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 20. Google Willow Quantum Chip - Revolutionizing Quantum Computing

**URL:** https://googlewillow.org

A groundbreaking quantum chip that promises to transform computational capabilities across multiple industries

Discover Google Willow, a quantum chip that outperforms supercomputers and reduces errors exponentially, paving the way for practical quantum computing applications

The Google Willow quantum chip is a significant breakthrough in the field of quantum computing, with the following capabilities:

Willow can complete certain computational tasks in less than 5 minutes, which would take the most powerful supercomputers billions of years to accomplish. In the RCS standard test benchmark, Willow can complete the calculation test in 5 minutes, while the fastest supercomputers today would need at least 10 septillion years (10^25 years).

Willow can complete certain computational tasks in less than 5 minutes, which would take the most powerful supercomputers billions of years to accomplish. In the RCS standard test benchmark, Willow can complete the calculation test in 5 minutes, while the fastest supercomputers today would need at least 10 septillion years (10^25 years).... Willow features 105 quantum bits (qubits) and reduces error rates by half while expanding the scale of qubits. Google's research published in Nature shows that they tested quantum bit grids of different sizes, from 3x3 to 5x5 to 7x7, each time halving the error rate.

Willow organizes qubits into a grid configuration known as 'logical qubits,' enabling real-time error correction. The larger the scale, the better the error correction effect; if the scale is sufficient, the error rate can approach zero.

Willow operates more efficiently at extremely low temperatures. Qubits are powerful but extremely fragile, requiring operation in an ultra-low temperature environment close to absolute zero to avoid external temperature influences.

Julian Kelly, Google's Quantum AI Hardware Director, stated, 'This will push the boundaries of science and exploration. With future commercial applications in medicine, batteries, and nuclear fusion, we are excited to solve problems that were previously unsolvable.'... Delve into the world of Google Willow, where we uncover the latest advancements in quantum computing and their implications for the future of technology

The introduction of Google Willow quantum chip is set to revolutionize various sectors with its unprecedented quantum computing capabilities, offering solutions to complex problems and enhancing efficiency in the following industries:

Quantum computing poses a potential threat to traditional encryption methods used in cryptocurrencies, necessitating the development of quantum-resistant encryption to secure digital assets against future threats.

Willow's ability to simulate molecular interactions at the atomic level can significantly accelerate drug discovery, reducing development timelines and costs, and potentially leading to breakthroughs in treatment.

Quantum computing can enhance AI capabilities by processing vast amounts of data more efficiently, leading to advancements in deep learning and data analysis, and solving complex problems beyond the reach of classical computers.

Nuclear fusion research and other energy technologies can benefit from quantum computing's ability to model complex physical dynamics, potentially leading to more efficient and sustainable energy solutions.... Quantum computing can optimize investment portfolios and provide precise risk analysis for financial institutions, offering a significant advantage in the competitive financial sector.

The Google Willow quantum chip, with 105 qubits, excels in error correction and random circuit sampling, completing tasks in minutes that would take supercomputers over 10^25 years.

The Google Willow quantum chip features 105 physical qubits and achieves best-in-class performance in quantum error correction and random circuit sampling. The Willow chip has accomplished two major milestones: It significantly reduced errors while increasing the number of qubits. It completed a standard benchmark calculation in under 5 minutes, whereas the fastest supercomputers today would require over 10^25 years to perform the same task.

The Willow chip's groundbreaking achievements include: Achieving 'below-threshold' error rates, meaning it reduced error rates while increasing qubit count. This has been a goal in the quantum computing field for nearly 30 years.... In the random circuit sampling (RCS) benchmark test, the Willow chip excelled, completing a calculation that would take the fastest supercomputers today over 10^25 years to perform

Google Willow represents a monumental breakthrough in quantum computing technology. This innovative quantum chip has demonstrated unprecedented capabilities that push the boundaries of computational science. By achieving 'below threshold' performance, Willow can dramatically reduce errors while scaling up the number of qubits. This is a critical advancement that brings us closer to practical, commercially viable quantum computing applications.

The Google Willow quantum chip surpasses its predecessors by achieving a 'below-threshold' error rate while increasing the number of qubits. This means it can reduce errors as the system scales, which is a significant advancement in making quantum computing more reliable and practical for real-world applications.... The 105 qubits in the Willow chip are crucial as they allow for more complex computations and improved error correction. The number of qubits is directly related to the computational power of a quantum computer, with more qubits enabling the processing of more intricate problems.

The Willow chip uses a method of quantum error correction that involves encoding logical qubits across multiple physical qubits. This allows the system to detect and correct errors in real-time, which is essential for the stability and accuracy of quantum computations.

The Willow chip has the potential to revolutionize scientific research by enabling simulations and calculations that are currently impossible with classical computers. This could lead to new discoveries in materials science, quantum physics, and other fields.

Yes, the Willow chip's advanced computational abilities can be used to create more accurate climate models. This could help in understanding climate change patterns and developing strategies to mitigate its effects.... Quantum computers, including the Willow chip, have the potential to solve optimization problems much faster than classical computers. This could be particularly useful in logistics, supply chain management, and other areas where efficiency is critical.

As with any powerful technology, quantum computing raises ethical questions, particularly around data security and privacy. It's important to develop guidelines and regulations to ensure that quantum computing is used responsibly and ethically.

The Willow chip stands out for its ability to perform complex calculations at an unprecedented speed, completing tasks in minutes that would take supercomputers billions of years. This makes it one of the most efficient quantum computing technologies to date.

The Willow chip requires a highly controlled environment, including extremely low temperatures close to absolute zero, to function optimally. This means that significant infrastructure is needed to maintain the chip's operating conditions.

### 21. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 22. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 23. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 24. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 25. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 26. Meet Willow, our state-of-the-art quantum chip - Google Blog

**URL:** https://blog.google/technology/research/google-willow-quantum-chip/

# Meet Willow, our state-of-the-art quantum chip

*Last updated: June 12, 2025*

Today I’m delighted to announce Willow, our latest quantum chip. Willow has state-of-the-art performance across a number of metrics, enabling two major achievements.

- The first is that Willow can reduce errors exponentially as we scale up using

*more*qubits. This cracks a key challenge in quantum error correction that the field has pursued for almost 30 years.

- Second, Willow performed a standard benchmark computation in under five minutes that would take one of today’s fastest supercomputers 10 septillion (that is, 10

25) years — a number that vastly exceeds the age of the Universe.

The Willow chip is a major step on a journey that began over 10 years ago. When I founded Google Quantum AI in 2012, the vision was to build a useful, large-scale quantum computer that could harness quantum mechanics — the “operating system” of nature to the extent we know it today — to benefit society by advancing scientific discovery, developing helpful applications, and tackling some of society's greatest challenges. As part of Google Research, our team has charted a long-term roadmap, and Willow moves us significantly along that path towards commercially relevant applications.... ## Exponential quantum error correction — below threshold!

Errors are one of the greatest challenges in quantum computing, since qubits, the units of computation in quantum computers, have a tendency to rapidly exchange information with their environment, making it difficult to protect the information needed to complete a computation. Typically the more qubits you use, the more errors will occur, and the system becomes classical.

Today in Nature, we published results showing that

**the more qubits we use in Willow, the more we** *reduce* ** errors** **, and the more quantum the system becomes**. We tested ever-larger arrays of physical qubits, scaling up from a grid of 3x3 encoded qubits, to a grid of 5x5, to a grid of 7x7 — and each time, using our latest advances in quantum error correction, we were able to cut the error rate in half. In other words, we achieved an exponential reduction in the error rate. This historic accomplishment is known in the field as “below threshold” — being able to drive errors down while scaling up the number of qubits. You must demonstrate being below threshold to show real progress on error correction, and this has been an outstanding challenge since quantum error correction was introduced by Peter Shor in 1995.

There are other scientific “firsts” involved in this result as well. For example, it’s also one of the first compelling examples of real-time error correction on a superconducting quantum system — crucial for any useful computation, because if you can’t correct errors fast enough, they ruin your computation before it’s done. And it’s a "beyond breakeven" demonstration, where our arrays of qubits have longer lifetimes than the individual physical qubits do, an unfakable sign that error correction is improving the system overall.

As the first system below threshold, this is the most convincing prototype for a scalable logical qubit built to date. It’s a strong sign that useful, very large quantum computers can indeed be built. Willow brings us closer to running practical, commercially-relevant algorithms that can’t be replicated on conventional computers.... ## 10 septillion years on one of today’s fastest supercomputers

As a measure of Willow’s performance, we used the random circuit sampling (RCS) benchmark. Pioneered by our team and now widely used as a standard in the field, RCS is the classically hardest benchmark that can be done on a quantum computer today. You can think of this as an entry point for quantum computing — it checks whether a quantum computer is doing something that couldn’t be done on a classical computer. Any team building a quantum computer should check first if it can beat classical computers on RCS; otherwise there is strong reason for skepticism that it can tackle more complex quantum tasks. We’ve consistently used this benchmark to assess progress from one generation of chip to the next — we reported Sycamore results in October 2019 and again recently in October 2024.

Willow’s performance on this benchmark is astonishing: It performed a computation in under five minutes that would take one of today’s fastest supercomputers 10

25 or 10 septillion years. If you want to write it out, it’s 10,000,000,000,000,000,000,000,000 years. This mind-boggling number exceeds known timescales in physics and vastly exceeds the age of the universe. It lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch.

These latest results for Willow, as shown in the plot below, are our best so far, but we’ll continue to make progress.

Computational costs are heavily influenced by available memory. Our estimates therefore consider a range of scenarios, from an ideal situation with unlimited memory (▲) to a more practical, embarrassingly parallelizable implementation on GPUs (⬤).

Our assessment of how Willow outpaces one of the world’s most powerful classical supercomputers, Frontier, was based on conservative assumptions. For example, we assumed full access to secondary storage, i.e., hard drives, without any bandwidth overhead — a generous and unrealistic allowance for Frontier. Of course, as happened after we announced the first beyond-classical computation in 2019, we expect classical computers to keep improving on this benchmark, but the rapidly growing gap shows that quantum processors are peeling away at a double exponential rate and will continue to vastly outperform classical computers as we scale up.... ## State-of-the-art performance

Willow was fabricated in our new, state-of-the-art fabrication facility in Santa Barbara — one of only a few facilities in the world built from the ground up for this purpose. System engineering is key when designing and fabricating quantum chips: All components of a chip, such as single and two-qubit gates, qubit reset, and readout, have to be simultaneously well engineered and integrated. If any component lags or if two components don't function well together, it drags down system performance. Therefore, maximizing system performance informs all aspects of our process, from chip architecture and fabrication to gate development and calibration. The achievements we report assess quantum computing systems holistically, not just one factor at a time.

We’re focusing on quality, not just quantity — because just producing larger numbers of qubits doesn’t help if they’re not high enough quality. With 105 qubits, Willow now has best-in-class performance across the two system benchmarks discussed above: quantum error correction and random circuit sampling. Such algorithmic benchmarks are the best way to measure overall chip performance. Other more specific performance metrics are also important; for example, our T

1 times, which measure how long qubits can retain an excitation — the key quantum computational resource — are now approaching 100 µs (microseconds). This is an impressive ~5x improvement over our previous generation of chips. If you want to evaluate quantum hardware and compare across platforms, here is a table of key specifications:

Willow’s performance across a number of metrics.... ## What’s next with Willow and beyond

The next challenge for the field is to demonstrate a first "useful, beyond-classical" computation on today's quantum chips that is relevant to a real-world application. We’re optimistic that the Willow generation of chips can help us achieve this goal. So far, there have been two separate types of experiments. On the one hand, we’ve run the RCS benchmark, which measures performance against classical computers but has no known real-world applications. On the other hand, we’ve done scientifically interesting simulations of quantum systems, which have led to new scientific discoveries but are still within the reach of classical computers. Our goal is to do both at the same time — to step into the realm of algorithms that are beyond the reach of classical computers

**and** that are useful for real-world, commercially relevant problems.

Random circuit sampling (RCS), while extremely challenging for classical computers, has yet to demonstrate practical commercial applications.

We invite researchers, engineers, and developers to join us on this journey by checking out our open source software and educational resources, including our new course on Coursera, where developers can learn the essentials of quantum error correction and help us create algorithms that can solve the problems of the future.

My colleagues sometimes ask me why I left the burgeoning field of AI to focus on quantum computing. My answer is that both will prove to be the most transformational technologies of our time, but advanced AI will significantly benefit from access to quantum computing. This is why I named our lab Quantum AI. Quantum algorithms have fundamental scaling laws on their side, as we’re seeing with RCS. There are similar scaling advantages for many foundational computational tasks that are essential for AI. So quantum computation will be indispensable for collecting training data that’s inaccessible to classical machines, training and optimizing certain learning architectures, and modeling systems where quantum effects are important. This includes helping us discover new medicines, designing more efficient batteries for electric cars, and accelerating progress in fusion and new energy alternatives. Many of these future game-changing applications won’t be feasible on classical computers; they’re waiting to be unlocked with quantum computing.

### 27. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 28. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 29. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 30. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 31. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

**URL:** https://en.wikipedia.org/wiki/Willow_processor

The **Willow processor** is a 105-qubit superconducting quantum computing processor developed by Google Quantum AI and manufactured in Santa Barbara, California.

## Overview

On December 9, 2024, Google Quantum AI announced Willow in a *Nature* paper and company blogpost, and claiming two accomplishments: First, that Willow can reduce errors exponentially as the number of qubits is scaled, achieving below threshold quantum error correction. Second, that Willow completed a Random Circuit Sampling (RCS) benchmark task in 5 minutes that would take today's fastest supercomputers 10 septillion (10^25^) years.

Willow is constructed with a square grid of superconducting transmon physical qubits. Improvements over past work were attributed to improved fabrication techniques, participation ratio engineering, and circuit parameter optimization.

Willow prompted optimism in accelerating applications in pharmaceuticals, material science, logistics, drug discovery, and energy grid allocation. Popular media responses discussed its risk in breaking cryptographic systems, but a Google spokesman said that they were still at least 10 years out from breaking RSA. Hartmut Neven, founder and lead of Google Quantum AI, told the BBC that Willow would be used in practical applications, and in the announcement blogpost expressed the belief that advanced AI will benefit from quantum computing.

Willow follows the release of Foxtail in 2017, Bristlecone in 2018, and Sycamore in 2019. Willow has twice as many qubits as Sycamore and improves upon T1 coherence time from Sycamore's 20 microseconds to 100 microseconds. Willow's 105 qubits have an average connectivity of 3.47.

Hartmut Neven, founder of Google Quantum AI, prompted controversy by claiming that the success of Willow "lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch."... ## Criticism

Per Google company's claim, Willow is the first chip to achieve below threshold quantum error correction.

However, a number of critics have pointed out several limitations:

- The logical error rates reported (around 0.14% per cycle) remain orders of magnitude above the 10^−6^ levels believed necessary for running meaningful, large-scale quantum algorithms.
- To date, demonstrations have been limited to quantum memory and the preservation of logical qubits—without yet showing below‑threshold performance of logical gate operations required for universal fault‑tolerant computation.
- Media coverage has been accused of overstating Willow’s practical significance; although error suppression scales exponentially with qubit count, no large‑scale quantum algorithms or commercial applications have yet been demonstrated on Willow.
- Observers caution that achieving below‑threshold error correction is only one milestone on the path to practical quantum computing—further hardware improvements (lower physical error rates) and vastly larger qubit arrays will be required before industrially relevant problem‑solving is possible.
- Some experts note that Willow remains a research prototype within the Noisy intermediate-scale quantum era, still far from delivering the practical, fault‑tolerant performance required for real‑world applications.

## References

### 32. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 33. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 34. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 35. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 36. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 37. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

**URL:** https://en.wikipedia.org/wiki/Willow_processor

The **Willow processor** is a 105-qubit superconducting quantum computing processor developed by Google Quantum AI and manufactured in Santa Barbara, California.

## Overview

On December 9, 2024, Google Quantum AI announced Willow in a *Nature* paper and company blogpost, and claiming two accomplishments: First, that Willow can reduce errors exponentially as the number of qubits is scaled, achieving below threshold quantum error correction. Second, that Willow completed a Random Circuit Sampling (RCS) benchmark task in 5 minutes that would take today's fastest supercomputers 10 septillion (10^25^) years.

Willow is constructed with a square grid of superconducting transmon physical qubits. Improvements over past work were attributed to improved fabrication techniques, participation ratio engineering, and circuit parameter optimization.

Willow prompted optimism in accelerating applications in pharmaceuticals, material science, logistics, drug discovery, and energy grid allocation. Popular media responses discussed its risk in breaking cryptographic systems, but a Google spokesman said that they were still at least 10 years out from breaking RSA. Hartmut Neven, founder and lead of Google Quantum AI, told the BBC that Willow would be used in practical applications, and in the announcement blogpost expressed the belief that advanced AI will benefit from quantum computing.

Willow follows the release of Foxtail in 2017, Bristlecone in 2018, and Sycamore in 2019. Willow has twice as many qubits as Sycamore and improves upon T1 coherence time from Sycamore's 20 microseconds to 100 microseconds. Willow's 105 qubits have an average connectivity of 3.47.

Hartmut Neven, founder of Google Quantum AI, prompted controversy by claiming that the success of Willow "lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch."... ## Criticism

Per Google company's claim, Willow is the first chip to achieve below threshold quantum error correction.

However, a number of critics have pointed out several limitations:

- The logical error rates reported (around 0.14% per cycle) remain orders of magnitude above the 10^−6^ levels believed necessary for running meaningful, large-scale quantum algorithms.
- To date, demonstrations have been limited to quantum memory and the preservation of logical qubits—without yet showing below‑threshold performance of logical gate operations required for universal fault‑tolerant computation.
- Media coverage has been accused of overstating Willow’s practical significance; although error suppression scales exponentially with qubit count, no large‑scale quantum algorithms or commercial applications have yet been demonstrated on Willow.
- Observers caution that achieving below‑threshold error correction is only one milestone on the path to practical quantum computing—further hardware improvements (lower physical error rates) and vastly larger qubit arrays will be required before industrially relevant problem‑solving is possible.
- Some experts note that Willow remains a research prototype within the Noisy intermediate-scale quantum era, still far from delivering the practical, fault‑tolerant performance required for real‑world applications.

## References

### 38. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 39. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

**URL:** https://en.wikipedia.org/wiki/Willow_processor

The **Willow processor** is a 105-qubit superconducting quantum computing processor developed by Google Quantum AI and manufactured in Santa Barbara, California.

## Overview

On December 9, 2024, Google Quantum AI announced Willow in a *Nature* paper and company blogpost, and claiming two accomplishments: First, that Willow can reduce errors exponentially as the number of qubits is scaled, achieving below threshold quantum error correction. Second, that Willow completed a Random Circuit Sampling (RCS) benchmark task in 5 minutes that would take today's fastest supercomputers 10 septillion (10^25^) years.

Willow is constructed with a square grid of superconducting transmon physical qubits. Improvements over past work were attributed to improved fabrication techniques, participation ratio engineering, and circuit parameter optimization.

Willow prompted optimism in accelerating applications in pharmaceuticals, material science, logistics, drug discovery, and energy grid allocation. Popular media responses discussed its risk in breaking cryptographic systems, but a Google spokesman said that they were still at least 10 years out from breaking RSA. Hartmut Neven, founder and lead of Google Quantum AI, told the BBC that Willow would be used in practical applications, and in the announcement blogpost expressed the belief that advanced AI will benefit from quantum computing.

Willow follows the release of Foxtail in 2017, Bristlecone in 2018, and Sycamore in 2019. Willow has twice as many qubits as Sycamore and improves upon T1 coherence time from Sycamore's 20 microseconds to 100 microseconds. Willow's 105 qubits have an average connectivity of 3.47.

Hartmut Neven, founder of Google Quantum AI, prompted controversy by claiming that the success of Willow "lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch."... ## Criticism

Per Google company's claim, Willow is the first chip to achieve below threshold quantum error correction.

However, a number of critics have pointed out several limitations:

- The logical error rates reported (around 0.14% per cycle) remain orders of magnitude above the 10^−6^ levels believed necessary for running meaningful, large-scale quantum algorithms.
- To date, demonstrations have been limited to quantum memory and the preservation of logical qubits—without yet showing below‑threshold performance of logical gate operations required for universal fault‑tolerant computation.
- Media coverage has been accused of overstating Willow’s practical significance; although error suppression scales exponentially with qubit count, no large‑scale quantum algorithms or commercial applications have yet been demonstrated on Willow.
- Observers caution that achieving below‑threshold error correction is only one milestone on the path to practical quantum computing—further hardware improvements (lower physical error rates) and vastly larger qubit arrays will be required before industrially relevant problem‑solving is possible.
- Some experts note that Willow remains a research prototype within the Noisy intermediate-scale quantum era, still far from delivering the practical, fault‑tolerant performance required for real‑world applications.

## References

### 40. Google Willow Quantum Chip - Revolutionizing Quantum Computing

**URL:** https://googlewillow.org

A groundbreaking quantum chip that promises to transform computational capabilities across multiple industries

Discover Google Willow, a quantum chip that outperforms supercomputers and reduces errors exponentially, paving the way for practical quantum computing applications

The Google Willow quantum chip is a significant breakthrough in the field of quantum computing, with the following capabilities:

Willow can complete certain computational tasks in less than 5 minutes, which would take the most powerful supercomputers billions of years to accomplish. In the RCS standard test benchmark, Willow can complete the calculation test in 5 minutes, while the fastest supercomputers today would need at least 10 septillion years (10^25 years).

Willow can complete certain computational tasks in less than 5 minutes, which would take the most powerful supercomputers billions of years to accomplish. In the RCS standard test benchmark, Willow can complete the calculation test in 5 minutes, while the fastest supercomputers today would need at least 10 septillion years (10^25 years).... Willow features 105 quantum bits (qubits) and reduces error rates by half while expanding the scale of qubits. Google's research published in Nature shows that they tested quantum bit grids of different sizes, from 3x3 to 5x5 to 7x7, each time halving the error rate.

Willow organizes qubits into a grid configuration known as 'logical qubits,' enabling real-time error correction. The larger the scale, the better the error correction effect; if the scale is sufficient, the error rate can approach zero.

Willow operates more efficiently at extremely low temperatures. Qubits are powerful but extremely fragile, requiring operation in an ultra-low temperature environment close to absolute zero to avoid external temperature influences.

Julian Kelly, Google's Quantum AI Hardware Director, stated, 'This will push the boundaries of science and exploration. With future commercial applications in medicine, batteries, and nuclear fusion, we are excited to solve problems that were previously unsolvable.'... Delve into the world of Google Willow, where we uncover the latest advancements in quantum computing and their implications for the future of technology

The introduction of Google Willow quantum chip is set to revolutionize various sectors with its unprecedented quantum computing capabilities, offering solutions to complex problems and enhancing efficiency in the following industries:

Quantum computing poses a potential threat to traditional encryption methods used in cryptocurrencies, necessitating the development of quantum-resistant encryption to secure digital assets against future threats.

Willow's ability to simulate molecular interactions at the atomic level can significantly accelerate drug discovery, reducing development timelines and costs, and potentially leading to breakthroughs in treatment.

Quantum computing can enhance AI capabilities by processing vast amounts of data more efficiently, leading to advancements in deep learning and data analysis, and solving complex problems beyond the reach of classical computers.

Nuclear fusion research and other energy technologies can benefit from quantum computing's ability to model complex physical dynamics, potentially leading to more efficient and sustainable energy solutions.... Quantum computing can optimize investment portfolios and provide precise risk analysis for financial institutions, offering a significant advantage in the competitive financial sector.

The Google Willow quantum chip, with 105 qubits, excels in error correction and random circuit sampling, completing tasks in minutes that would take supercomputers over 10^25 years.

The Google Willow quantum chip features 105 physical qubits and achieves best-in-class performance in quantum error correction and random circuit sampling. The Willow chip has accomplished two major milestones: It significantly reduced errors while increasing the number of qubits. It completed a standard benchmark calculation in under 5 minutes, whereas the fastest supercomputers today would require over 10^25 years to perform the same task.

The Willow chip's groundbreaking achievements include: Achieving 'below-threshold' error rates, meaning it reduced error rates while increasing qubit count. This has been a goal in the quantum computing field for nearly 30 years.... In the random circuit sampling (RCS) benchmark test, the Willow chip excelled, completing a calculation that would take the fastest supercomputers today over 10^25 years to perform

Google Willow represents a monumental breakthrough in quantum computing technology. This innovative quantum chip has demonstrated unprecedented capabilities that push the boundaries of computational science. By achieving 'below threshold' performance, Willow can dramatically reduce errors while scaling up the number of qubits. This is a critical advancement that brings us closer to practical, commercially viable quantum computing applications.

The Google Willow quantum chip surpasses its predecessors by achieving a 'below-threshold' error rate while increasing the number of qubits. This means it can reduce errors as the system scales, which is a significant advancement in making quantum computing more reliable and practical for real-world applications.... The 105 qubits in the Willow chip are crucial as they allow for more complex computations and improved error correction. The number of qubits is directly related to the computational power of a quantum computer, with more qubits enabling the processing of more intricate problems.

The Willow chip uses a method of quantum error correction that involves encoding logical qubits across multiple physical qubits. This allows the system to detect and correct errors in real-time, which is essential for the stability and accuracy of quantum computations.

The Willow chip has the potential to revolutionize scientific research by enabling simulations and calculations that are currently impossible with classical computers. This could lead to new discoveries in materials science, quantum physics, and other fields.

Yes, the Willow chip's advanced computational abilities can be used to create more accurate climate models. This could help in understanding climate change patterns and developing strategies to mitigate its effects.... Quantum computers, including the Willow chip, have the potential to solve optimization problems much faster than classical computers. This could be particularly useful in logistics, supply chain management, and other areas where efficiency is critical.

As with any powerful technology, quantum computing raises ethical questions, particularly around data security and privacy. It's important to develop guidelines and regulations to ensure that quantum computing is used responsibly and ethically.

The Willow chip stands out for its ability to perform complex calculations at an unprecedented speed, completing tasks in minutes that would take supercomputers billions of years. This makes it one of the most efficient quantum computing technologies to date.

The Willow chip requires a highly controlled environment, including extremely low temperatures close to absolute zero, to function optimally. This means that significant infrastructure is needed to maintain the chip's operating conditions.

### 41. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 42. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 43. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 44. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 45. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 46. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

**URL:** https://en.wikipedia.org/wiki/Willow_processor

The **Willow processor** is a 105-qubit superconducting quantum computing processor developed by Google Quantum AI and manufactured in Santa Barbara, California.

## Overview

On December 9, 2024, Google Quantum AI announced Willow in a *Nature* paper and company blogpost, and claiming two accomplishments: First, that Willow can reduce errors exponentially as the number of qubits is scaled, achieving below threshold quantum error correction. Second, that Willow completed a Random Circuit Sampling (RCS) benchmark task in 5 minutes that would take today's fastest supercomputers 10 septillion (10^25^) years.

Willow is constructed with a square grid of superconducting transmon physical qubits. Improvements over past work were attributed to improved fabrication techniques, participation ratio engineering, and circuit parameter optimization.

Willow prompted optimism in accelerating applications in pharmaceuticals, material science, logistics, drug discovery, and energy grid allocation. Popular media responses discussed its risk in breaking cryptographic systems, but a Google spokesman said that they were still at least 10 years out from breaking RSA. Hartmut Neven, founder and lead of Google Quantum AI, told the BBC that Willow would be used in practical applications, and in the announcement blogpost expressed the belief that advanced AI will benefit from quantum computing.

Willow follows the release of Foxtail in 2017, Bristlecone in 2018, and Sycamore in 2019. Willow has twice as many qubits as Sycamore and improves upon T1 coherence time from Sycamore's 20 microseconds to 100 microseconds. Willow's 105 qubits have an average connectivity of 3.47.

Hartmut Neven, founder of Google Quantum AI, prompted controversy by claiming that the success of Willow "lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch."... ## Criticism

Per Google company's claim, Willow is the first chip to achieve below threshold quantum error correction.

However, a number of critics have pointed out several limitations:

- The logical error rates reported (around 0.14% per cycle) remain orders of magnitude above the 10^−6^ levels believed necessary for running meaningful, large-scale quantum algorithms.
- To date, demonstrations have been limited to quantum memory and the preservation of logical qubits—without yet showing below‑threshold performance of logical gate operations required for universal fault‑tolerant computation.
- Media coverage has been accused of overstating Willow’s practical significance; although error suppression scales exponentially with qubit count, no large‑scale quantum algorithms or commercial applications have yet been demonstrated on Willow.
- Observers caution that achieving below‑threshold error correction is only one milestone on the path to practical quantum computing—further hardware improvements (lower physical error rates) and vastly larger qubit arrays will be required before industrially relevant problem‑solving is possible.
- Some experts note that Willow remains a research prototype within the Noisy intermediate-scale quantum era, still far from delivering the practical, fault‑tolerant performance required for real‑world applications.

## References

### 47. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 48. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 49. Google Willow Quantum Chip - Revolutionizing Quantum Computing

**URL:** https://googlewillow.org

A groundbreaking quantum chip that promises to transform computational capabilities across multiple industries

Discover Google Willow, a quantum chip that outperforms supercomputers and reduces errors exponentially, paving the way for practical quantum computing applications

The Google Willow quantum chip is a significant breakthrough in the field of quantum computing, with the following capabilities:

Willow can complete certain computational tasks in less than 5 minutes, which would take the most powerful supercomputers billions of years to accomplish. In the RCS standard test benchmark, Willow can complete the calculation test in 5 minutes, while the fastest supercomputers today would need at least 10 septillion years (10^25 years).

Willow can complete certain computational tasks in less than 5 minutes, which would take the most powerful supercomputers billions of years to accomplish. In the RCS standard test benchmark, Willow can complete the calculation test in 5 minutes, while the fastest supercomputers today would need at least 10 septillion years (10^25 years).... Willow features 105 quantum bits (qubits) and reduces error rates by half while expanding the scale of qubits. Google's research published in Nature shows that they tested quantum bit grids of different sizes, from 3x3 to 5x5 to 7x7, each time halving the error rate.

Willow organizes qubits into a grid configuration known as 'logical qubits,' enabling real-time error correction. The larger the scale, the better the error correction effect; if the scale is sufficient, the error rate can approach zero.

Willow operates more efficiently at extremely low temperatures. Qubits are powerful but extremely fragile, requiring operation in an ultra-low temperature environment close to absolute zero to avoid external temperature influences.

Julian Kelly, Google's Quantum AI Hardware Director, stated, 'This will push the boundaries of science and exploration. With future commercial applications in medicine, batteries, and nuclear fusion, we are excited to solve problems that were previously unsolvable.'... Delve into the world of Google Willow, where we uncover the latest advancements in quantum computing and their implications for the future of technology

The introduction of Google Willow quantum chip is set to revolutionize various sectors with its unprecedented quantum computing capabilities, offering solutions to complex problems and enhancing efficiency in the following industries:

Quantum computing poses a potential threat to traditional encryption methods used in cryptocurrencies, necessitating the development of quantum-resistant encryption to secure digital assets against future threats.

Willow's ability to simulate molecular interactions at the atomic level can significantly accelerate drug discovery, reducing development timelines and costs, and potentially leading to breakthroughs in treatment.

Quantum computing can enhance AI capabilities by processing vast amounts of data more efficiently, leading to advancements in deep learning and data analysis, and solving complex problems beyond the reach of classical computers.

Nuclear fusion research and other energy technologies can benefit from quantum computing's ability to model complex physical dynamics, potentially leading to more efficient and sustainable energy solutions.... Quantum computing can optimize investment portfolios and provide precise risk analysis for financial institutions, offering a significant advantage in the competitive financial sector.

The Google Willow quantum chip, with 105 qubits, excels in error correction and random circuit sampling, completing tasks in minutes that would take supercomputers over 10^25 years.

The Google Willow quantum chip features 105 physical qubits and achieves best-in-class performance in quantum error correction and random circuit sampling. The Willow chip has accomplished two major milestones: It significantly reduced errors while increasing the number of qubits. It completed a standard benchmark calculation in under 5 minutes, whereas the fastest supercomputers today would require over 10^25 years to perform the same task.

The Willow chip's groundbreaking achievements include: Achieving 'below-threshold' error rates, meaning it reduced error rates while increasing qubit count. This has been a goal in the quantum computing field for nearly 30 years.... In the random circuit sampling (RCS) benchmark test, the Willow chip excelled, completing a calculation that would take the fastest supercomputers today over 10^25 years to perform

Google Willow represents a monumental breakthrough in quantum computing technology. This innovative quantum chip has demonstrated unprecedented capabilities that push the boundaries of computational science. By achieving 'below threshold' performance, Willow can dramatically reduce errors while scaling up the number of qubits. This is a critical advancement that brings us closer to practical, commercially viable quantum computing applications.

The Google Willow quantum chip surpasses its predecessors by achieving a 'below-threshold' error rate while increasing the number of qubits. This means it can reduce errors as the system scales, which is a significant advancement in making quantum computing more reliable and practical for real-world applications.... The 105 qubits in the Willow chip are crucial as they allow for more complex computations and improved error correction. The number of qubits is directly related to the computational power of a quantum computer, with more qubits enabling the processing of more intricate problems.

The Willow chip uses a method of quantum error correction that involves encoding logical qubits across multiple physical qubits. This allows the system to detect and correct errors in real-time, which is essential for the stability and accuracy of quantum computations.

The Willow chip has the potential to revolutionize scientific research by enabling simulations and calculations that are currently impossible with classical computers. This could lead to new discoveries in materials science, quantum physics, and other fields.

Yes, the Willow chip's advanced computational abilities can be used to create more accurate climate models. This could help in understanding climate change patterns and developing strategies to mitigate its effects.... Quantum computers, including the Willow chip, have the potential to solve optimization problems much faster than classical computers. This could be particularly useful in logistics, supply chain management, and other areas where efficiency is critical.

As with any powerful technology, quantum computing raises ethical questions, particularly around data security and privacy. It's important to develop guidelines and regulations to ensure that quantum computing is used responsibly and ethically.

The Willow chip stands out for its ability to perform complex calculations at an unprecedented speed, completing tasks in minutes that would take supercomputers billions of years. This makes it one of the most efficient quantum computing technologies to date.

The Willow chip requires a highly controlled environment, including extremely low temperatures close to absolute zero, to function optimally. This means that significant infrastructure is needed to maintain the chip's operating conditions.

### 50. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 51. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 52. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

**URL:** https://en.wikipedia.org/wiki/Willow_processor

The **Willow processor** is a 105-qubit superconducting quantum computing processor developed by Google Quantum AI and manufactured in Santa Barbara, California.

## Overview

On December 9, 2024, Google Quantum AI announced Willow in a *Nature* paper and company blogpost, and claiming two accomplishments: First, that Willow can reduce errors exponentially as the number of qubits is scaled, achieving below threshold quantum error correction. Second, that Willow completed a Random Circuit Sampling (RCS) benchmark task in 5 minutes that would take today's fastest supercomputers 10 septillion (10^25^) years.

Willow is constructed with a square grid of superconducting transmon physical qubits. Improvements over past work were attributed to improved fabrication techniques, participation ratio engineering, and circuit parameter optimization.

Willow prompted optimism in accelerating applications in pharmaceuticals, material science, logistics, drug discovery, and energy grid allocation. Popular media responses discussed its risk in breaking cryptographic systems, but a Google spokesman said that they were still at least 10 years out from breaking RSA. Hartmut Neven, founder and lead of Google Quantum AI, told the BBC that Willow would be used in practical applications, and in the announcement blogpost expressed the belief that advanced AI will benefit from quantum computing.

Willow follows the release of Foxtail in 2017, Bristlecone in 2018, and Sycamore in 2019. Willow has twice as many qubits as Sycamore and improves upon T1 coherence time from Sycamore's 20 microseconds to 100 microseconds. Willow's 105 qubits have an average connectivity of 3.47.

Hartmut Neven, founder of Google Quantum AI, prompted controversy by claiming that the success of Willow "lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch."... ## Criticism

Per Google company's claim, Willow is the first chip to achieve below threshold quantum error correction.

However, a number of critics have pointed out several limitations:

- The logical error rates reported (around 0.14% per cycle) remain orders of magnitude above the 10^−6^ levels believed necessary for running meaningful, large-scale quantum algorithms.
- To date, demonstrations have been limited to quantum memory and the preservation of logical qubits—without yet showing below‑threshold performance of logical gate operations required for universal fault‑tolerant computation.
- Media coverage has been accused of overstating Willow’s practical significance; although error suppression scales exponentially with qubit count, no large‑scale quantum algorithms or commercial applications have yet been demonstrated on Willow.
- Observers caution that achieving below‑threshold error correction is only one milestone on the path to practical quantum computing—further hardware improvements (lower physical error rates) and vastly larger qubit arrays will be required before industrially relevant problem‑solving is possible.
- Some experts note that Willow remains a research prototype within the Noisy intermediate-scale quantum era, still far from delivering the practical, fault‑tolerant performance required for real‑world applications.

## References

### 53. Google Willow Quantum Chip - Revolutionizing Quantum Computing

**URL:** https://googlewillow.org

A groundbreaking quantum chip that promises to transform computational capabilities across multiple industries

Discover Google Willow, a quantum chip that outperforms supercomputers and reduces errors exponentially, paving the way for practical quantum computing applications

The Google Willow quantum chip is a significant breakthrough in the field of quantum computing, with the following capabilities:

Willow can complete certain computational tasks in less than 5 minutes, which would take the most powerful supercomputers billions of years to accomplish. In the RCS standard test benchmark, Willow can complete the calculation test in 5 minutes, while the fastest supercomputers today would need at least 10 septillion years (10^25 years).

Willow can complete certain computational tasks in less than 5 minutes, which would take the most powerful supercomputers billions of years to accomplish. In the RCS standard test benchmark, Willow can complete the calculation test in 5 minutes, while the fastest supercomputers today would need at least 10 septillion years (10^25 years).... Willow features 105 quantum bits (qubits) and reduces error rates by half while expanding the scale of qubits. Google's research published in Nature shows that they tested quantum bit grids of different sizes, from 3x3 to 5x5 to 7x7, each time halving the error rate.

Willow organizes qubits into a grid configuration known as 'logical qubits,' enabling real-time error correction. The larger the scale, the better the error correction effect; if the scale is sufficient, the error rate can approach zero.

Willow operates more efficiently at extremely low temperatures. Qubits are powerful but extremely fragile, requiring operation in an ultra-low temperature environment close to absolute zero to avoid external temperature influences.

Julian Kelly, Google's Quantum AI Hardware Director, stated, 'This will push the boundaries of science and exploration. With future commercial applications in medicine, batteries, and nuclear fusion, we are excited to solve problems that were previously unsolvable.'... Delve into the world of Google Willow, where we uncover the latest advancements in quantum computing and their implications for the future of technology

The introduction of Google Willow quantum chip is set to revolutionize various sectors with its unprecedented quantum computing capabilities, offering solutions to complex problems and enhancing efficiency in the following industries:

Quantum computing poses a potential threat to traditional encryption methods used in cryptocurrencies, necessitating the development of quantum-resistant encryption to secure digital assets against future threats.

Willow's ability to simulate molecular interactions at the atomic level can significantly accelerate drug discovery, reducing development timelines and costs, and potentially leading to breakthroughs in treatment.

Quantum computing can enhance AI capabilities by processing vast amounts of data more efficiently, leading to advancements in deep learning and data analysis, and solving complex problems beyond the reach of classical computers.

Nuclear fusion research and other energy technologies can benefit from quantum computing's ability to model complex physical dynamics, potentially leading to more efficient and sustainable energy solutions.... Quantum computing can optimize investment portfolios and provide precise risk analysis for financial institutions, offering a significant advantage in the competitive financial sector.

The Google Willow quantum chip, with 105 qubits, excels in error correction and random circuit sampling, completing tasks in minutes that would take supercomputers over 10^25 years.

The Google Willow quantum chip features 105 physical qubits and achieves best-in-class performance in quantum error correction and random circuit sampling. The Willow chip has accomplished two major milestones: It significantly reduced errors while increasing the number of qubits. It completed a standard benchmark calculation in under 5 minutes, whereas the fastest supercomputers today would require over 10^25 years to perform the same task.

The Willow chip's groundbreaking achievements include: Achieving 'below-threshold' error rates, meaning it reduced error rates while increasing qubit count. This has been a goal in the quantum computing field for nearly 30 years.... In the random circuit sampling (RCS) benchmark test, the Willow chip excelled, completing a calculation that would take the fastest supercomputers today over 10^25 years to perform

Google Willow represents a monumental breakthrough in quantum computing technology. This innovative quantum chip has demonstrated unprecedented capabilities that push the boundaries of computational science. By achieving 'below threshold' performance, Willow can dramatically reduce errors while scaling up the number of qubits. This is a critical advancement that brings us closer to practical, commercially viable quantum computing applications.

The Google Willow quantum chip surpasses its predecessors by achieving a 'below-threshold' error rate while increasing the number of qubits. This means it can reduce errors as the system scales, which is a significant advancement in making quantum computing more reliable and practical for real-world applications.... The 105 qubits in the Willow chip are crucial as they allow for more complex computations and improved error correction. The number of qubits is directly related to the computational power of a quantum computer, with more qubits enabling the processing of more intricate problems.

The Willow chip uses a method of quantum error correction that involves encoding logical qubits across multiple physical qubits. This allows the system to detect and correct errors in real-time, which is essential for the stability and accuracy of quantum computations.

The Willow chip has the potential to revolutionize scientific research by enabling simulations and calculations that are currently impossible with classical computers. This could lead to new discoveries in materials science, quantum physics, and other fields.

Yes, the Willow chip's advanced computational abilities can be used to create more accurate climate models. This could help in understanding climate change patterns and developing strategies to mitigate its effects.... Quantum computers, including the Willow chip, have the potential to solve optimization problems much faster than classical computers. This could be particularly useful in logistics, supply chain management, and other areas where efficiency is critical.

As with any powerful technology, quantum computing raises ethical questions, particularly around data security and privacy. It's important to develop guidelines and regulations to ensure that quantum computing is used responsibly and ethically.

The Willow chip stands out for its ability to perform complex calculations at an unprecedented speed, completing tasks in minutes that would take supercomputers billions of years. This makes it one of the most efficient quantum computing technologies to date.

The Willow chip requires a highly controlled environment, including extremely low temperatures close to absolute zero, to function optimally. This means that significant infrastructure is needed to maintain the chip's operating conditions.

### 54. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 55. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 56. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 57. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 58. Meet Willow, our state-of-the-art quantum chip - Google Blog

**URL:** https://blog.google/technology/research/google-willow-quantum-chip/

# Meet Willow, our state-of-the-art quantum chip

*Last updated: June 12, 2025*

Today I’m delighted to announce Willow, our latest quantum chip. Willow has state-of-the-art performance across a number of metrics, enabling two major achievements.

- The first is that Willow can reduce errors exponentially as we scale up using

*more*qubits. This cracks a key challenge in quantum error correction that the field has pursued for almost 30 years.

- Second, Willow performed a standard benchmark computation in under five minutes that would take one of today’s fastest supercomputers 10 septillion (that is, 10

25) years — a number that vastly exceeds the age of the Universe.

The Willow chip is a major step on a journey that began over 10 years ago. When I founded Google Quantum AI in 2012, the vision was to build a useful, large-scale quantum computer that could harness quantum mechanics — the “operating system” of nature to the extent we know it today — to benefit society by advancing scientific discovery, developing helpful applications, and tackling some of society's greatest challenges. As part of Google Research, our team has charted a long-term roadmap, and Willow moves us significantly along that path towards commercially relevant applications.... ## Exponential quantum error correction — below threshold!

Errors are one of the greatest challenges in quantum computing, since qubits, the units of computation in quantum computers, have a tendency to rapidly exchange information with their environment, making it difficult to protect the information needed to complete a computation. Typically the more qubits you use, the more errors will occur, and the system becomes classical.

Today in Nature, we published results showing that

**the more qubits we use in Willow, the more we** *reduce* ** errors** **, and the more quantum the system becomes**. We tested ever-larger arrays of physical qubits, scaling up from a grid of 3x3 encoded qubits, to a grid of 5x5, to a grid of 7x7 — and each time, using our latest advances in quantum error correction, we were able to cut the error rate in half. In other words, we achieved an exponential reduction in the error rate. This historic accomplishment is known in the field as “below threshold” — being able to drive errors down while scaling up the number of qubits. You must demonstrate being below threshold to show real progress on error correction, and this has been an outstanding challenge since quantum error correction was introduced by Peter Shor in 1995.

There are other scientific “firsts” involved in this result as well. For example, it’s also one of the first compelling examples of real-time error correction on a superconducting quantum system — crucial for any useful computation, because if you can’t correct errors fast enough, they ruin your computation before it’s done. And it’s a "beyond breakeven" demonstration, where our arrays of qubits have longer lifetimes than the individual physical qubits do, an unfakable sign that error correction is improving the system overall.

As the first system below threshold, this is the most convincing prototype for a scalable logical qubit built to date. It’s a strong sign that useful, very large quantum computers can indeed be built. Willow brings us closer to running practical, commercially-relevant algorithms that can’t be replicated on conventional computers.... ## 10 septillion years on one of today’s fastest supercomputers

As a measure of Willow’s performance, we used the random circuit sampling (RCS) benchmark. Pioneered by our team and now widely used as a standard in the field, RCS is the classically hardest benchmark that can be done on a quantum computer today. You can think of this as an entry point for quantum computing — it checks whether a quantum computer is doing something that couldn’t be done on a classical computer. Any team building a quantum computer should check first if it can beat classical computers on RCS; otherwise there is strong reason for skepticism that it can tackle more complex quantum tasks. We’ve consistently used this benchmark to assess progress from one generation of chip to the next — we reported Sycamore results in October 2019 and again recently in October 2024.

Willow’s performance on this benchmark is astonishing: It performed a computation in under five minutes that would take one of today’s fastest supercomputers 10

25 or 10 septillion years. If you want to write it out, it’s 10,000,000,000,000,000,000,000,000 years. This mind-boggling number exceeds known timescales in physics and vastly exceeds the age of the universe. It lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch.

These latest results for Willow, as shown in the plot below, are our best so far, but we’ll continue to make progress.

Computational costs are heavily influenced by available memory. Our estimates therefore consider a range of scenarios, from an ideal situation with unlimited memory (▲) to a more practical, embarrassingly parallelizable implementation on GPUs (⬤).

Our assessment of how Willow outpaces one of the world’s most powerful classical supercomputers, Frontier, was based on conservative assumptions. For example, we assumed full access to secondary storage, i.e., hard drives, without any bandwidth overhead — a generous and unrealistic allowance for Frontier. Of course, as happened after we announced the first beyond-classical computation in 2019, we expect classical computers to keep improving on this benchmark, but the rapidly growing gap shows that quantum processors are peeling away at a double exponential rate and will continue to vastly outperform classical computers as we scale up.... ## State-of-the-art performance

Willow was fabricated in our new, state-of-the-art fabrication facility in Santa Barbara — one of only a few facilities in the world built from the ground up for this purpose. System engineering is key when designing and fabricating quantum chips: All components of a chip, such as single and two-qubit gates, qubit reset, and readout, have to be simultaneously well engineered and integrated. If any component lags or if two components don't function well together, it drags down system performance. Therefore, maximizing system performance informs all aspects of our process, from chip architecture and fabrication to gate development and calibration. The achievements we report assess quantum computing systems holistically, not just one factor at a time.

We’re focusing on quality, not just quantity — because just producing larger numbers of qubits doesn’t help if they’re not high enough quality. With 105 qubits, Willow now has best-in-class performance across the two system benchmarks discussed above: quantum error correction and random circuit sampling. Such algorithmic benchmarks are the best way to measure overall chip performance. Other more specific performance metrics are also important; for example, our T

1 times, which measure how long qubits can retain an excitation — the key quantum computational resource — are now approaching 100 µs (microseconds). This is an impressive ~5x improvement over our previous generation of chips. If you want to evaluate quantum hardware and compare across platforms, here is a table of key specifications:

Willow’s performance across a number of metrics.... ## What’s next with Willow and beyond

The next challenge for the field is to demonstrate a first "useful, beyond-classical" computation on today's quantum chips that is relevant to a real-world application. We’re optimistic that the Willow generation of chips can help us achieve this goal. So far, there have been two separate types of experiments. On the one hand, we’ve run the RCS benchmark, which measures performance against classical computers but has no known real-world applications. On the other hand, we’ve done scientifically interesting simulations of quantum systems, which have led to new scientific discoveries but are still within the reach of classical computers. Our goal is to do both at the same time — to step into the realm of algorithms that are beyond the reach of classical computers

**and** that are useful for real-world, commercially relevant problems.

Random circuit sampling (RCS), while extremely challenging for classical computers, has yet to demonstrate practical commercial applications.

We invite researchers, engineers, and developers to join us on this journey by checking out our open source software and educational resources, including our new course on Coursera, where developers can learn the essentials of quantum error correction and help us create algorithms that can solve the problems of the future.

My colleagues sometimes ask me why I left the burgeoning field of AI to focus on quantum computing. My answer is that both will prove to be the most transformational technologies of our time, but advanced AI will significantly benefit from access to quantum computing. This is why I named our lab Quantum AI. Quantum algorithms have fundamental scaling laws on their side, as we’re seeing with RCS. There are similar scaling advantages for many foundational computational tasks that are essential for AI. So quantum computation will be indispensable for collecting training data that’s inaccessible to classical machines, training and optimizing certain learning architectures, and modeling systems where quantum effects are important. This includes helping us discover new medicines, designing more efficient batteries for electric cars, and accelerating progress in fusion and new energy alternatives. Many of these future game-changing applications won’t be feasible on classical computers; they’re waiting to be unlocked with quantum computing.

### 59. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 60. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 61. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 62. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 63. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

**URL:** https://en.wikipedia.org/wiki/Willow_processor

The **Willow processor** is a 105-qubit superconducting quantum computing processor developed by Google Quantum AI and manufactured in Santa Barbara, California.

## Overview

On December 9, 2024, Google Quantum AI announced Willow in a *Nature* paper and company blogpost, and claiming two accomplishments: First, that Willow can reduce errors exponentially as the number of qubits is scaled, achieving below threshold quantum error correction. Second, that Willow completed a Random Circuit Sampling (RCS) benchmark task in 5 minutes that would take today's fastest supercomputers 10 septillion (10^25^) years.

Willow is constructed with a square grid of superconducting transmon physical qubits. Improvements over past work were attributed to improved fabrication techniques, participation ratio engineering, and circuit parameter optimization.

Willow prompted optimism in accelerating applications in pharmaceuticals, material science, logistics, drug discovery, and energy grid allocation. Popular media responses discussed its risk in breaking cryptographic systems, but a Google spokesman said that they were still at least 10 years out from breaking RSA. Hartmut Neven, founder and lead of Google Quantum AI, told the BBC that Willow would be used in practical applications, and in the announcement blogpost expressed the belief that advanced AI will benefit from quantum computing.

Willow follows the release of Foxtail in 2017, Bristlecone in 2018, and Sycamore in 2019. Willow has twice as many qubits as Sycamore and improves upon T1 coherence time from Sycamore's 20 microseconds to 100 microseconds. Willow's 105 qubits have an average connectivity of 3.47.

Hartmut Neven, founder of Google Quantum AI, prompted controversy by claiming that the success of Willow "lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch."... ## Criticism

Per Google company's claim, Willow is the first chip to achieve below threshold quantum error correction.

However, a number of critics have pointed out several limitations:

- The logical error rates reported (around 0.14% per cycle) remain orders of magnitude above the 10^−6^ levels believed necessary for running meaningful, large-scale quantum algorithms.
- To date, demonstrations have been limited to quantum memory and the preservation of logical qubits—without yet showing below‑threshold performance of logical gate operations required for universal fault‑tolerant computation.
- Media coverage has been accused of overstating Willow’s practical significance; although error suppression scales exponentially with qubit count, no large‑scale quantum algorithms or commercial applications have yet been demonstrated on Willow.
- Observers caution that achieving below‑threshold error correction is only one milestone on the path to practical quantum computing—further hardware improvements (lower physical error rates) and vastly larger qubit arrays will be required before industrially relevant problem‑solving is possible.
- Some experts note that Willow remains a research prototype within the Noisy intermediate-scale quantum era, still far from delivering the practical, fault‑tolerant performance required for real‑world applications.

## References

### 64. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 65. Meet Willow, our state-of-the-art quantum chip - Google Blog

**URL:** https://blog.google/technology/research/google-willow-quantum-chip/

# Meet Willow, our state-of-the-art quantum chip

*Last updated: June 12, 2025*

Today I’m delighted to announce Willow, our latest quantum chip. Willow has state-of-the-art performance across a number of metrics, enabling two major achievements.

- The first is that Willow can reduce errors exponentially as we scale up using

*more*qubits. This cracks a key challenge in quantum error correction that the field has pursued for almost 30 years.

- Second, Willow performed a standard benchmark computation in under five minutes that would take one of today’s fastest supercomputers 10 septillion (that is, 10

25) years — a number that vastly exceeds the age of the Universe.

The Willow chip is a major step on a journey that began over 10 years ago. When I founded Google Quantum AI in 2012, the vision was to build a useful, large-scale quantum computer that could harness quantum mechanics — the “operating system” of nature to the extent we know it today — to benefit society by advancing scientific discovery, developing helpful applications, and tackling some of society's greatest challenges. As part of Google Research, our team has charted a long-term roadmap, and Willow moves us significantly along that path towards commercially relevant applications.... ## Exponential quantum error correction — below threshold!

Errors are one of the greatest challenges in quantum computing, since qubits, the units of computation in quantum computers, have a tendency to rapidly exchange information with their environment, making it difficult to protect the information needed to complete a computation. Typically the more qubits you use, the more errors will occur, and the system becomes classical.

Today in Nature, we published results showing that

**the more qubits we use in Willow, the more we** *reduce* ** errors** **, and the more quantum the system becomes**. We tested ever-larger arrays of physical qubits, scaling up from a grid of 3x3 encoded qubits, to a grid of 5x5, to a grid of 7x7 — and each time, using our latest advances in quantum error correction, we were able to cut the error rate in half. In other words, we achieved an exponential reduction in the error rate. This historic accomplishment is known in the field as “below threshold” — being able to drive errors down while scaling up the number of qubits. You must demonstrate being below threshold to show real progress on error correction, and this has been an outstanding challenge since quantum error correction was introduced by Peter Shor in 1995.

There are other scientific “firsts” involved in this result as well. For example, it’s also one of the first compelling examples of real-time error correction on a superconducting quantum system — crucial for any useful computation, because if you can’t correct errors fast enough, they ruin your computation before it’s done. And it’s a "beyond breakeven" demonstration, where our arrays of qubits have longer lifetimes than the individual physical qubits do, an unfakable sign that error correction is improving the system overall.

As the first system below threshold, this is the most convincing prototype for a scalable logical qubit built to date. It’s a strong sign that useful, very large quantum computers can indeed be built. Willow brings us closer to running practical, commercially-relevant algorithms that can’t be replicated on conventional computers.... ## 10 septillion years on one of today’s fastest supercomputers

As a measure of Willow’s performance, we used the random circuit sampling (RCS) benchmark. Pioneered by our team and now widely used as a standard in the field, RCS is the classically hardest benchmark that can be done on a quantum computer today. You can think of this as an entry point for quantum computing — it checks whether a quantum computer is doing something that couldn’t be done on a classical computer. Any team building a quantum computer should check first if it can beat classical computers on RCS; otherwise there is strong reason for skepticism that it can tackle more complex quantum tasks. We’ve consistently used this benchmark to assess progress from one generation of chip to the next — we reported Sycamore results in October 2019 and again recently in October 2024.

Willow’s performance on this benchmark is astonishing: It performed a computation in under five minutes that would take one of today’s fastest supercomputers 10

25 or 10 septillion years. If you want to write it out, it’s 10,000,000,000,000,000,000,000,000 years. This mind-boggling number exceeds known timescales in physics and vastly exceeds the age of the universe. It lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch.

These latest results for Willow, as shown in the plot below, are our best so far, but we’ll continue to make progress.

Computational costs are heavily influenced by available memory. Our estimates therefore consider a range of scenarios, from an ideal situation with unlimited memory (▲) to a more practical, embarrassingly parallelizable implementation on GPUs (⬤).

Our assessment of how Willow outpaces one of the world’s most powerful classical supercomputers, Frontier, was based on conservative assumptions. For example, we assumed full access to secondary storage, i.e., hard drives, without any bandwidth overhead — a generous and unrealistic allowance for Frontier. Of course, as happened after we announced the first beyond-classical computation in 2019, we expect classical computers to keep improving on this benchmark, but the rapidly growing gap shows that quantum processors are peeling away at a double exponential rate and will continue to vastly outperform classical computers as we scale up.... ## State-of-the-art performance

Willow was fabricated in our new, state-of-the-art fabrication facility in Santa Barbara — one of only a few facilities in the world built from the ground up for this purpose. System engineering is key when designing and fabricating quantum chips: All components of a chip, such as single and two-qubit gates, qubit reset, and readout, have to be simultaneously well engineered and integrated. If any component lags or if two components don't function well together, it drags down system performance. Therefore, maximizing system performance informs all aspects of our process, from chip architecture and fabrication to gate development and calibration. The achievements we report assess quantum computing systems holistically, not just one factor at a time.

We’re focusing on quality, not just quantity — because just producing larger numbers of qubits doesn’t help if they’re not high enough quality. With 105 qubits, Willow now has best-in-class performance across the two system benchmarks discussed above: quantum error correction and random circuit sampling. Such algorithmic benchmarks are the best way to measure overall chip performance. Other more specific performance metrics are also important; for example, our T

1 times, which measure how long qubits can retain an excitation — the key quantum computational resource — are now approaching 100 µs (microseconds). This is an impressive ~5x improvement over our previous generation of chips. If you want to evaluate quantum hardware and compare across platforms, here is a table of key specifications:

Willow’s performance across a number of metrics.... ## What’s next with Willow and beyond

The next challenge for the field is to demonstrate a first "useful, beyond-classical" computation on today's quantum chips that is relevant to a real-world application. We’re optimistic that the Willow generation of chips can help us achieve this goal. So far, there have been two separate types of experiments. On the one hand, we’ve run the RCS benchmark, which measures performance against classical computers but has no known real-world applications. On the other hand, we’ve done scientifically interesting simulations of quantum systems, which have led to new scientific discoveries but are still within the reach of classical computers. Our goal is to do both at the same time — to step into the realm of algorithms that are beyond the reach of classical computers

**and** that are useful for real-world, commercially relevant problems.

Random circuit sampling (RCS), while extremely challenging for classical computers, has yet to demonstrate practical commercial applications.

We invite researchers, engineers, and developers to join us on this journey by checking out our open source software and educational resources, including our new course on Coursera, where developers can learn the essentials of quantum error correction and help us create algorithms that can solve the problems of the future.

My colleagues sometimes ask me why I left the burgeoning field of AI to focus on quantum computing. My answer is that both will prove to be the most transformational technologies of our time, but advanced AI will significantly benefit from access to quantum computing. This is why I named our lab Quantum AI. Quantum algorithms have fundamental scaling laws on their side, as we’re seeing with RCS. There are similar scaling advantages for many foundational computational tasks that are essential for AI. So quantum computation will be indispensable for collecting training data that’s inaccessible to classical machines, training and optimizing certain learning architectures, and modeling systems where quantum effects are important. This includes helping us discover new medicines, designing more efficient batteries for electric cars, and accelerating progress in fusion and new energy alternatives. Many of these future game-changing applications won’t be feasible on classical computers; they’re waiting to be unlocked with quantum computing.

### 66. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 67. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 68. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

**URL:** https://en.wikipedia.org/wiki/Willow_processor

The **Willow processor** is a 105-qubit superconducting quantum computing processor developed by Google Quantum AI and manufactured in Santa Barbara, California.

## Overview

On December 9, 2024, Google Quantum AI announced Willow in a *Nature* paper and company blogpost, and claiming two accomplishments: First, that Willow can reduce errors exponentially as the number of qubits is scaled, achieving below threshold quantum error correction. Second, that Willow completed a Random Circuit Sampling (RCS) benchmark task in 5 minutes that would take today's fastest supercomputers 10 septillion (10^25^) years.

Willow is constructed with a square grid of superconducting transmon physical qubits. Improvements over past work were attributed to improved fabrication techniques, participation ratio engineering, and circuit parameter optimization.

Willow prompted optimism in accelerating applications in pharmaceuticals, material science, logistics, drug discovery, and energy grid allocation. Popular media responses discussed its risk in breaking cryptographic systems, but a Google spokesman said that they were still at least 10 years out from breaking RSA. Hartmut Neven, founder and lead of Google Quantum AI, told the BBC that Willow would be used in practical applications, and in the announcement blogpost expressed the belief that advanced AI will benefit from quantum computing.

Willow follows the release of Foxtail in 2017, Bristlecone in 2018, and Sycamore in 2019. Willow has twice as many qubits as Sycamore and improves upon T1 coherence time from Sycamore's 20 microseconds to 100 microseconds. Willow's 105 qubits have an average connectivity of 3.47.

Hartmut Neven, founder of Google Quantum AI, prompted controversy by claiming that the success of Willow "lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch."... ## Criticism

Per Google company's claim, Willow is the first chip to achieve below threshold quantum error correction.

However, a number of critics have pointed out several limitations:

- The logical error rates reported (around 0.14% per cycle) remain orders of magnitude above the 10^−6^ levels believed necessary for running meaningful, large-scale quantum algorithms.
- To date, demonstrations have been limited to quantum memory and the preservation of logical qubits—without yet showing below‑threshold performance of logical gate operations required for universal fault‑tolerant computation.
- Media coverage has been accused of overstating Willow’s practical significance; although error suppression scales exponentially with qubit count, no large‑scale quantum algorithms or commercial applications have yet been demonstrated on Willow.
- Observers caution that achieving below‑threshold error correction is only one milestone on the path to practical quantum computing—further hardware improvements (lower physical error rates) and vastly larger qubit arrays will be required before industrially relevant problem‑solving is possible.
- Some experts note that Willow remains a research prototype within the Noisy intermediate-scale quantum era, still far from delivering the practical, fault‑tolerant performance required for real‑world applications.

## References

### 69. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 70. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

### 71. Willow Spec Sheet

**URL:** https://quantumai.google/static/site-assets/downloads/willow-spec-sheet.pdf

Published Dec 9, 2024
Willow, Google Quantum AI's latest quantum chip, features 
breakthrough improvements that enable major advances in quantum 
error correction and random circuit sampling. This spec sheet 
summarizes Willow's performance across key hardware metrics.
Willow System Metrics
Number of qubits
105
Average connectivity
3.47  (4-way typical)
Quantum Error Correction  (Chip 1)
Single-qubit gate error 1  (mean, simultaneous)
0.035% ± 0.029%
Two-qubit gate error 1  (mean, simultaneous)
0.33% ± 0.18%  (CZ)
Measurement error  (mean, simultaneous)
0.77% ± 0.21%  (repetitive, measure qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
68 µs ± 13 µs2
Error correction cycles per second
909,000  (surface code cycle = 1.1 µs)
Application performance
Λ3,5,7 = 2.14 ± 0.02
Random Circuit Sampling  (Chip 2)
Single-qubit gate error 1  (mean, simultaneous)
0.036% ± 0.013%
Two-qubit gate error 1  (mean, simultaneous)
0.14% ± 0.052%  (iswap-like)
Measurement error  (mean, simultaneous)
0.67% ± 0.51%  (terminal, all qubits)
Reset options
Multi-level reset (|1) state and above)
Leakage removal (|2) state only)
T1 time  (mean)
98 µs ± 32 µs2
Circuit repetitions per second
63,000
Application performance
103 qubits, depth 40, XEB fidelity = 0.1%
Estimated time on Willow vs. classical supercomputer
5 minutes vs. 1025 years
1 Operation errors measured with randomized benchmarking techniques and reported as “average error”
2 Chip 1 and 2 exhibit different T1 due to a tradeoff between optimizing qubit geometry for electromagnetic shielding and maximizing coherence... Qubit grid
Full Distributions
Willow Spec Sheet  |  Dec 9 2024
2
Willow Chip 1: QEC
Willow Chip 2: RCS
Coherence
Note: 
The numbers inside the qubit 
grid refer to the row and 
column index

### 72. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

**URL:** https://en.wikipedia.org/wiki/Willow_processor

The **Willow processor** is a 105-qubit superconducting quantum computing processor developed by Google Quantum AI and manufactured in Santa Barbara, California.

## Overview

On December 9, 2024, Google Quantum AI announced Willow in a *Nature* paper and company blogpost, and claiming two accomplishments: First, that Willow can reduce errors exponentially as the number of qubits is scaled, achieving below threshold quantum error correction. Second, that Willow completed a Random Circuit Sampling (RCS) benchmark task in 5 minutes that would take today's fastest supercomputers 10 septillion (10^25^) years.

Willow is constructed with a square grid of superconducting transmon physical qubits. Improvements over past work were attributed to improved fabrication techniques, participation ratio engineering, and circuit parameter optimization.

Willow prompted optimism in accelerating applications in pharmaceuticals, material science, logistics, drug discovery, and energy grid allocation. Popular media responses discussed its risk in breaking cryptographic systems, but a Google spokesman said that they were still at least 10 years out from breaking RSA. Hartmut Neven, founder and lead of Google Quantum AI, told the BBC that Willow would be used in practical applications, and in the announcement blogpost expressed the belief that advanced AI will benefit from quantum computing.

Willow follows the release of Foxtail in 2017, Bristlecone in 2018, and Sycamore in 2019. Willow has twice as many qubits as Sycamore and improves upon T1 coherence time from Sycamore's 20 microseconds to 100 microseconds. Willow's 105 qubits have an average connectivity of 3.47.

Hartmut Neven, founder of Google Quantum AI, prompted controversy by claiming that the success of Willow "lends credence to the notion that quantum computation occurs in many parallel universes, in line with the idea that we live in a multiverse, a prediction first made by David Deutsch."... ## Criticism

Per Google company's claim, Willow is the first chip to achieve below threshold quantum error correction.

However, a number of critics have pointed out several limitations:

- The logical error rates reported (around 0.14% per cycle) remain orders of magnitude above the 10^−6^ levels believed necessary for running meaningful, large-scale quantum algorithms.
- To date, demonstrations have been limited to quantum memory and the preservation of logical qubits—without yet showing below‑threshold performance of logical gate operations required for universal fault‑tolerant computation.
- Media coverage has been accused of overstating Willow’s practical significance; although error suppression scales exponentially with qubit count, no large‑scale quantum algorithms or commercial applications have yet been demonstrated on Willow.
- Observers caution that achieving below‑threshold error correction is only one milestone on the path to practical quantum computing—further hardware improvements (lower physical error rates) and vastly larger qubit arrays will be required before industrially relevant problem‑solving is possible.
- Some experts note that Willow remains a research prototype within the Noisy intermediate-scale quantum era, still far from delivering the practical, fault‑tolerant performance required for real‑world applications.

## References

### 73. Google Announces Willow Quantum Chip

**URL:** https://postquantum.com/industry-news/google-willow-quantum-chip/

## Table of Contents

**Santa Barbara, CA, USA (Dec 2024)** – Google has unveiled a new quantum processor named “Willow”, marking a major milestone in the race toward practical quantum computing. The 105-qubit Willow chip demonstrates two breakthroughs that have long eluded researchers: it dramatically reduces error rates as qubit count scales up, and it completed a computational task in minutes that would take a classical supercomputer longer than the age of the universe. These achievements suggest Google’s quantum hardware is edging closer to the threshold of useful quantum advantage, paving the way for large-scale systems that could outperform classical computers on real-world problems.... ## Pushing Quantum Performance to New Heights

Google’s Quantum AI team built Willow as the successor to its 2019 Sycamore chip, roughly doubling the qubit count from 53 to 105 while vastly improving qubit quality. Crucially, Willow’s design isn’t just about adding more qubits – it’s about better qubits. In quantum computing, more qubits mean nothing if they’re too error-prone. Willow tackles this with engineering refinements that boost qubit coherence times to ~100 microseconds, about 5× longer than Sycamore’s 20 μs. That stability, combined with an average qubit connectivity of 3.47 in a 2D grid, gives Willow “best-in-class” performance on holistic benchmarks like quantum error correction and random circuit sampling.

In a standard benchmark test known as Random Circuit Sampling (RCS), Willow proved its mettle. It churned through a complex random circuit in under five minutes – an instance so computationally hard that today’s fastest classical supercomputer would need an estimated 10 septillion ($$10^{25}$$) years to do the same. This isn’t just a parlor trick; it’s a strong indicator that Willow has achieved a quantum “beyond-classical” regime. Hartmut Neven, founder of Google Quantum AI, noted that RCS is currently “... *the classically hardest benchmark*” for a quantum processor, essentially a stress test to prove the quantum machine is doing something no normal computer could. The result builds on Google’s 2019 quantum supremacy experiment, but with a chip far more powerful than before – and it hints that useful quantum computing may arrive sooner than skeptics expect.

Perhaps Willow’s most significant feat is in quantum error correction – the decades-long quest to tame quantum errors. In tests, Google showed that by grouping physical qubits into a logical qubit “surface” and gradually enlarging that group, the error rate dropped instead of rising. Starting with a 3×3 qubit patch and scaling up to a 7×7 patch, Willow was able to cut logical error rates roughly in half. “

*This historic accomplishment is known in the field as below threshold – being able to drive errors down while scaling up the number of qubits,*” Neven explained, calling it an “ *unfakeable sign*” that error correction is materially improving the system. In practical terms, Willow is the first quantum chip to demonstrate error rates that improve (exponentially) with added qubits , a key proof-of-concept for building much larger fault-tolerant quantum computers. Google reports it even ran real-time error correction cycles on the chip during calculations, a notable first for superconducting qubits.

For more in-depth information about the Google’s error correction achievement, see the Nature paper accompanying the announcement: Quantum error correction below the surface code threshold.... ## Under the Hood of Willow’s Design

The Willow processor is built on Google’s preferred platform: superconducting transmon qubits arranged in a square lattice. Each qubit is a tiny circuit on a chip cooled to millikelvin temperatures. Willow was fabricated end-to-end in Google’s new custom quantum chip facility in Santa Barbara. This tight vertical integration – from materials to fabrication to cryogenics – was key to its success. Anthony Megrant, Google Quantum AI’s chief architect, noted that the company moved from a shared university fab into its own cleanroom to produce Willow, which speeds up the iteration cycle for new designs. All quantum operations (single-qubit gates, two-qubit gates, state reset, and readout) were co-optimized in Willow’s design, ensuring no one component lags behind. The result is a balanced system where every part works in harmony – critical, because any weak link would drag down the overall fidelity.

To appreciate Willow’s performance, consider its coherence and gate quality metrics. Google reports qubit

*T1* coherence times (how long a qubit can retain its state) approaching 100 µs. Gate fidelities are not explicitly stated in the announcement, but the successful error-correction experiment implies extremely high fidelity two-qubit gates and measurement reliability. In fact, Willow’s holistic benchmark results now rival or exceed other platforms. For example, in random circuit sampling, Willow outpaces one of the world’s most powerful classical supercomputers by an overwhelming margin. A performance table released by Google shows Willow leading in key specs among contemporary quantum chips, underscoring that the focus on “quality, not just quantity” of qubits has paid off.

This chip is still a far cry from a general-purpose quantum computer, but it bridges an important gap. So far, quantum demos have fallen into two buckets: contrived mathematical challenges beyond classical reach (like RCS), or useful simulations that could still be done with classical supercomputers given enough time. Willow aims to do both at once – reach beyond-classical computational power and tackle problems with real-world relevance. “

*The next challenge for the field is to demonstrate a first ‘useful, beyond-classical’ computation on today’s quantum chips that is relevant to a real-world application,*” Neven wrote, expressing optimism that the Willow generation can hit that goal.... ## Google vs. IBM, and the Quantum Competition

The quantum computing race has several heavyweights, and Google’s announcement comes on the heels of notable advances by others. IBM, for instance, recently introduced “Condor,” the world’s first quantum processor to break the 1,000-qubit barrier with 1,121 superconducting qubits. IBM’s approach has emphasized scaling up qubit counts and linking smaller chips into larger ensembles. Its roadmap envisions modular systems and fault-tolerant quantum computing by around 2030. In fact, IBM has publicly targeted having useful error-corrected qubits by the end of this decade, enabled by iterative improvements in qubit design (their next-gen chips called Flamingo, etc.). IBM’s current hardware (433-qubit “Osprey” and 127-qubit “Eagle”, among others) still operates in the noisy, error-prone regime, but IBM has shown steady progress in error mitigation techniques and complexity of circuits. Earlier this year, IBM demonstrated it could run quantum circuits of 100+ qubits and 3,000 gates that defy brute-force classical simulation – a similar “beyond classical” milestone, albeit without full error correction. Condor, with its record qubit count, has performance comparable to IBM’s earlier 433-qubit device, indicating that simply adding qubits isn’t enough without boosting fidelity. This is where Google’s Willow differs: Google chose to keep qubit count modest while achieving an exponential reduction in errors through better engineering. It’s a quality-vs-quantity trade-off playing out in real time.... ## Scaling Up: Challenges on the Road to Quantum Utility

Even with the excitement around Willow, formidable challenges remain before quantum computers become a mainstream tool. Google’s latest accomplishment, while impressive, was essentially a one-off demonstration on a specialized benchmark. Researchers caution that general-purpose, fault-tolerant quantum computers will require orders of magnitude more qubits and further breakthroughs in error correction. As a sober reminder, Google’s own team estimates that breaking modern cryptography (like 2048-bit RSA encryption) with a quantum computer is at least 10 years away. Hartmut Neven told BBC News that he doesn’t expect a commercial quantum chip to be available before the end of the decade. In other words, 2020s quantum chips are still experimental prototypes, not ready to run your everyday computing tasks. Willow’s error-correction win involved a relatively small logical qubit (a 49-qubit surface code); achieving error-corrected multi-qubit operations and scaling to thousands or millions of physical qubits for complex algorithms is a whole new mountain to climb.

One big challenge is scaling without introducing new errors. As more qubits and components are added, maintaining ultra-low noise in a cryogenic environment becomes exponentially harder. The Willow chip’s 105 qubits already demand extremely precise control systems and cryostat cooling to around 10 millikelvins. Future devices may need integrated cryo-control electronics, more microwave lines, and perhaps modular architectures to keep things manageable. IBM, for instance, is planning to connect smaller chips (like its 133-qubit “Heron” processors) into larger ensembles to scale up while isolating error zones. Google may pursue a similar modular strategy or refine its surface code further so that each logical qubit is built from, say, 100 physical qubits instead of 1,000+. Manufacturing yield is another issue – building hundreds or thousands of high-quality qubits on a chip without defects will tax even cutting-edge fabrication processes. Google did invest in a dedicated fab for this reason, to iterate faster and learn how to manufacture at scale.

## Metadata

```json
{
  "planId": "plan_2",
  "executionTime": 21165,
  "replanned": false
}
```

## Reasoning Insights

- Query complexity: high (2.60)
- Detected domains: physics
- Using advanced heuristic analysis

**Confidence:** 85.0%

## Planning Log

```
🎯 GOAP Planning & Execution Log
================================
🧠 Strange Loop Reasoning:
  • Query complexity: high (2.60)
  • Detected domains: physics
  • Using advanced heuristic analysis
  • Confidence: 85.0%

📋 Plan Execution Summary:
  • Steps executed: 4
  • Success: Yes
  • Replanned: No
```
