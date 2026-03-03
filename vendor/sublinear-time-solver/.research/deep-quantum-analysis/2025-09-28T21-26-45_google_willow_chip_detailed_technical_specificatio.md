# Search Results

**Query:** Google Willow chip detailed technical specifications and performance metrics December 2024
**Date:** 2025-09-28T21:26:45.145Z

## Answer

Google’s Willow quantum chip, announced in December 2024, is a 105-qubit superconducting processor that sets new records in quantum error correction and computational performance, marking a major milestone toward scalable, fault-tolerant quantum computing[1][2][5].

**Key Technical Specifications (December 2024):**

| Metric                                 | Value / Description                                                                                   |
|-----------------------------------------|------------------------------------------------------------------------------------------------------|
| **Number of qubits**                   | 105                                                                                                  |
| **Qubit type**                         | Superconducting transmon qubits in a 2D square grid                                                  |
| **Average connectivity**                | 3.47 (4-way typical)                                                                                 |
| **Single-qubit gate error (QEC chip)**  | 0.035% ± 0.029% (mean, simultaneous)                                                                 |
| **Two-qubit gate error (QEC chip)**     | 0.33% ± 0.18% (CZ gate, mean, simultaneous)                                                          |
| **Measurement error (QEC chip)**        | 0.77% ± 0.21% (mean, simultaneous)                                                                   |
| **T1 coherence time (QEC chip)**        | 68 µs ± 13 µs                                                                                        |
| **Error correction cycles/sec**         | 909,000 (surface code cycle = 1.1 µs)                                                                |
| **Application performance (QEC chip)**  | Λ₃,₅,₇ = 2.14 ± 0.02                                                                                 |
| **Single-qubit gate error (RCS chip)**  | 0.036% ± 0.013%                                                                                      |
| **Two-qubit gate error (RCS chip)**     | 0.14% ± 0.052% (iswap-like)                                                                          |
| **Measurement error (RCS chip)**        | 0.67% ± 0.51%                                                                                        |
| **T1 coherence time (RCS chip)**        | 98 µs ± 32 µs                                                                                        |
| **Circuit repetitions/sec (RCS chip)**  | 63,000                                                                                               |
| **Random Circuit Sampling (RCS)**       | 103 qubits, depth 40, XEB fidelity = 0.1%                                                            |
| **Estimated time (Willow vs. classical)**| 5 minutes (Willow) vs. 10²⁵ years (classical supercomputer)                                          |
| **Reset options**                      | Multi-level reset (|1⟩ state and above), leakage removal (|2⟩ state only)                           |

**Performance Metrics and Achievements:**

- **Quantum Error Correction (QEC):**
  - Willow is the first chip to demonstrate *below-threshold* quantum error correction: as more qubits are added, logical error rates decrease exponentially, a key milestone for scalable quantum computing[1][2][5][6][7][9].
  - Demonstrated exponential error suppression by scaling surface code patches from 3×3 to 5×5 to 7×7 qubits, halving the error rate at each step[2][5][6][7][9].
  - Logical qubit lifetimes now exceed those of the underlying physical qubits, a “beyond breakeven” demonstration[2][5][6][7][9].

- **Random Circuit Sampling (RCS) Benchmark:**
  - Willow performed a computation in under five minutes that would take the world’s fastest classical supercomputer (Frontier) an estimated 10 septillion (10²⁵) years, vastly exceeding the age of the universe[1][2][3][4][5][6][7][8][9].
  - This benchmark is currently the hardest classically, and Willow’s result is considered a clear demonstration of quantum supremacy (though not yet quantum advantage for practical problems)[2][4][5][7][9].

- **Coherence and Gate Quality:**
  - T1 coherence times approach 100 µs, a ~5× improvement over the previous Sycamore chip (20 µs)[1][3][4][6][9].
  - Gate fidelities (single- and two-qubit) are among the best reported for large-scale superconducting devices[1][3][9].

- **Fabrication and Architecture:**
  - Willow was fabricated in Google’s dedicated quantum chip facility in Santa Barbara, enabling rapid iteration and high device quality[2][3][4][5][6][9].
  - The chip uses tunable qubits and couplers for high connectivity, fast gates, and in-situ hardware optimization[3].

**Limitations and Context:**

- **Logical Error Rates:** While Willow achieves below-threshold error correction, logical error rates (~0.14% per cycle) remain orders of magnitude above the 10⁻⁶ level needed for large-scale, fault-tolerant quantum algorithms[4].
- **Scope of Demonstrations:** To date, Willow’s below-threshold performance has been shown for quantum memory and logical qubit preservation, not for universal logical gate operations[4].
- **Quantum Advantage:** Willow demonstrates quantum supremacy (outperforming classical computers on contrived benchmarks), but not yet quantum advantage (solving practical, real-world problems faster than classical systems)[9].
- **Scaling Challenges:** Achieving practical quantum computing will require further reductions in error rates and scaling to thousands or millions of qubits[4][9].

**Summary Table: Willow vs. Sycamore**

| Chip         | Qubits | T1 (µs) | Avg. Connectivity | Single-Qubit Error | Two-Qubit Error | RCS Benchmark Time | Error Correction |
|--------------|--------|---------|------------------|--------------------|-----------------|--------------------|-----------------|
| Sycamore     | 53     | ~20     | ~3               | ~0.1%              | ~0.6%           | 200s (2019)        | Not below threshold |
| Willow       | 105    | ~100    | 3.47             | 0.035%             | 0.14–0.33%      | <5 min (2024)      | Below threshold     |

**Conclusion:**  
Willow represents a significant leap in quantum hardware, achieving exponential error suppression and quantum supremacy on the RCS benchmark, with best-in-class coherence and gate fidelity for a chip of its scale. However, practical, fault-tolerant quantum computing for real-world applications remains a future goal, requiring further advances in error rates and system size[1][2][4][5][9].

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

### 3. Google Quantum AI

**URL:** https://quantumai.google

Our mission is to build quantum computing for otherwise unsolvable problems.



Featured

# Introducing Willow, the next generation of quantum chips

Willow, Google Quantum AI's latest state-of-the-art quantum chip, is a big step towards developing a large-scale, error-corrected quantum computer. Read the blog and watch the video to learn more about Willow and its breakthrough achievements.



Featured

# Learn about developing for quantum error correction with Coursera

Get a free introduction into the world of quantum error correction with our new Coursera course, designed for everyone from curious undergrads to seasoned software engineers. Gain hands-on experience with industry-standard software tools like Stim and Crumble. Enroll today and start your quantum journey.

# News & Resources

View published research, blog posts, and educational resources from the Quantum AI team.

## Metadata

```json
{
  "planId": "plan_3",
  "executionTime": 20658,
  "replanned": false
}
```

## Reasoning Insights

- Query complexity: high (1.90)
- Detected domains: ai, recent
- Temporal focus: specific timeframe
- Using advanced heuristic analysis

**Confidence:** 85.0%

## Planning Log

```
🎯 GOAP Planning & Execution Log
================================
🧠 Strange Loop Reasoning:
  • Query complexity: high (1.90)
  • Detected domains: ai, recent
  • Temporal focus: specific timeframe
  • Using advanced heuristic analysis
  • Confidence: 85.0%

📋 Plan Execution Summary:
  • Steps executed: 4
  • Success: Yes
  • Replanned: No
```
