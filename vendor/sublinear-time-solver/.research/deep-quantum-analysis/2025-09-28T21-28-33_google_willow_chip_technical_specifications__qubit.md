# Search Results

**Query:** Google Willow chip technical specifications: qubit count, error rates, coherence times, gate fidelities, operating temperature, fabrication process, quantum volume, CLOPS benchmarks
**Date:** 2025-09-28T21:28:33.370Z

## Answer

**Google’s Willow quantum chip features 105 superconducting qubits, with state-of-the-art error rates, coherence times near 100 microseconds, and best-in-class performance on quantum error correction and random circuit sampling benchmarks.** Below are the detailed technical specifications and context for each requested metric:

---

### Qubit Count
- **105 physical qubits** arranged in a square grid, with an average connectivity of 3.47 (typically 4-way)[1][2][3].

### Error Rates
- **Single-qubit gate error (mean, simultaneous):**
  - 0.035% ± 0.029% (QEC chip)
  - 0.036% ± 0.013% (RCS chip)[1]
- **Two-qubit gate error (mean, simultaneous):**
  - 0.33% ± 0.18% (CZ, QEC chip)
  - 0.14% ± 0.052% (iswap-like, RCS chip)[1]
- **Measurement error (mean, simultaneous):**
  - 0.77% ± 0.21% (QEC chip)
  - 0.67% ± 0.51% (RCS chip)[1]

### Coherence Times
- **T1 (energy relaxation time, mean):**
  - 68 µs ± 13 µs (QEC chip)
  - 98 µs ± 32 µs (RCS chip)[1]
- **T1 times approach 100 µs**, representing a ~5× improvement over the previous Sycamore chip (which had ~20 µs)[2][3].

### Gate Fidelities
- **Single-qubit gate fidelity:** ~99.965% (from error rates above)[1]
- **Two-qubit gate fidelity:** ~99.67% (QEC chip), ~99.86% (RCS chip)[1]
- These are among the highest reported for large-scale superconducting devices, supporting robust error correction[1][2].

### Operating Temperature
- **Millikelvin regime, close to absolute zero** (typical for superconducting qubits; precise value not specified, but standard is ~10–20 mK)[4].
- Requires advanced cryogenic infrastructure to maintain qubit coherence and suppress noise[4].

### Fabrication Process
- **Superconducting transmon qubits** fabricated in Google’s dedicated quantum chip facility in Santa Barbara[2][3][4].
- Uses advanced materials engineering, Josephson junctions, and custom electromagnetic shielding to maximize qubit quality and minimize noise[4].

### Quantum Volume
- **Not explicitly reported** for Willow. However, Willow’s performance on holistic benchmarks (quantum error correction and random circuit sampling) is described as “best-in-class,” and the chip is positioned as a leader in system-level quantum performance[2].
- Quantum volume is a holistic metric, but Google emphasizes algorithmic/system benchmarks over this single-number metric[2].

### CLOPS Benchmarks
- **Not explicitly reported** in available sources. CLOPS (Circuit Layer Operations Per Second) is an IBM-defined metric; Google instead reports:
  - **Error correction cycles per second:** 909,000 (surface code cycle = 1.1 µs)[1]
  - **Circuit repetitions per second:** 63,000 (RCS chip)[1]
- These rates reflect Willow’s high-throughput capability for both error correction and sampling tasks[1].

### Random Circuit Sampling (RCS) Benchmark
- **Willow completed a standard RCS benchmark (103 qubits, depth 40, XEB fidelity = 0.1%) in under 5 minutes**—a task estimated to take the world’s fastest classical supercomputer (Frontier) over 10^25 years[1][2][3].
- This demonstrates a quantum “beyond-classical” regime and is a major milestone in quantum computational supremacy[2].

### Quantum Error Correction (QEC) Performance
- **First demonstration of “below threshold” error correction:** As the number of physical qubits in a logical qubit increases (from 3×3 to 5×5 to 7×7), the logical error rate is halved each time, showing exponential suppression of errors[1][2].
- **Logical qubit lifetimes now exceed those of the underlying physical qubits**, a key milestone for scalable, fault-tolerant quantum computing[1][2].

---

#### Additional Notes
- **System-level focus:** Google emphasizes holistic system performance (integration of gates, readout, reset, and error correction) rather than isolated component metrics[2][3].
- **Comparison to previous chips:** Willow doubles the qubit count of Sycamore and achieves a ~5× improvement in coherence time, with significantly lower error rates[2][3].
- **Commercial and scientific potential:** Willow’s advances are expected to accelerate applications in pharmaceuticals, materials science, energy, and AI, though practical quantum advantage for real-world problems remains a future goal[2].

---

**Summary Table: Google Willow Chip Key Specifications**

| Metric                        | Value / Description                                             |
|-------------------------------|---------------------------------------------------------------|
| Qubit count                   | 105 (superconducting transmons)                               |
| Qubit connectivity            | 3.47 (average, 4-way typical)                                 |
| Single-qubit gate error       | 0.035% (QEC), 0.036% (RCS)                                    |
| Two-qubit gate error          | 0.33% (QEC, CZ), 0.14% (RCS, iswap-like)                      |
| Measurement error             | 0.77% (QEC), 0.67% (RCS)                                      |
| T1 coherence time             | 68 µs (QEC), 98 µs (RCS); up to ~100 µs                       |
| Operating temperature         | ~10–20 mK (millikelvin regime)                                |
| Fabrication process           | Superconducting transmons, custom facility, advanced shielding|
| Quantum volume                | Not specified; best-in-class system benchmarks                |
| CLOPS                         | Not specified; 909,000 QEC cycles/sec; 63,000 RCS reps/sec    |
| RCS benchmark                 | 103 qubits, depth 40, XEB fidelity 0.1%, 5 min (vs. 10^25 yrs)|
| QEC performance               | Below threshold; error halved with each code distance increase|

---

**Citations:**  
[1] Willow Spec Sheet  
[2] Google Blog: Meet Willow  
[3] Willow processor - Wikipedia  
[4] Google Quantum AI Lab: Fabrication  
 PostQuantum.com: Google Willow Chip  
 The Quantum Insider: Willow Chip  
 Forrester: Willow Chip Analysis

If you need further breakdowns (e.g., logical vs. physical error rates, or more on fabrication), please specify.

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

### 2. Meet Willow, our state-of-the-art quantum chip - Google Blog

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

### 3. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

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

### 7. Willow Spec Sheet

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

### 8. Meet Willow, our state-of-the-art quantum chip - Google Blog

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

### 9. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

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

### 11. Willow Spec Sheet

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

### 12. Willow Spec Sheet

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

### 13. Meet Willow, our state-of-the-art quantum chip - Google Blog

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

### 15. Google Willow Quantum Chip - Revolutionizing Quantum Computing

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

### 16. Meet Willow, our state-of-the-art quantum chip - Google Blog

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

### 19. Google Willow Quantum Chip - Revolutionizing Quantum Computing

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

### 20. Meet Willow, our state-of-the-art quantum chip - Google Blog

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

### 21. Meet Willow, our state-of-the-art quantum chip - Google Blog

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

### 22. Willow Spec Sheet

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

### 27. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

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

### 28. Meet Willow, our state-of-the-art quantum chip - Google Blog

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

### 30. Meet Willow, our state-of-the-art quantum chip - Google Blog

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

### 31. Willow Spec Sheet

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

### 32. Meet Willow, our state-of-the-art quantum chip - Google Blog

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

### 33. Meet Willow, our state-of-the-art quantum chip - Google Blog

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

### 34. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

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

### 35. Meet Willow, our state-of-the-art quantum chip - Google Blog

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

### 36. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

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

### 37. Meet Willow, our state-of-the-art quantum chip - Google Blog

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

### 38. Willow Spec Sheet

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

### 39. Meet Willow, our state-of-the-art quantum chip - Google Blog

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

### 40. Willow processor - Wikipediaen.wikipedia.org › wiki › Willow_processor

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

### 41. Google Willow Quantum Chip - Revolutionizing Quantum Computing

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

## Metadata

```json
{
  "planId": "plan_4",
  "executionTime": 28813,
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
