# Search Results

**Query:** Comprehensive analysis of quantum computing breakthroughs 2024: Google Willow technical specs, error correction advances, commercial applications, comparative benchmarks with IBM and competitors
**Date:** 2025-09-28T21:29:30.413Z

## Answer

Google’s Willow quantum chip, announced in December 2024, represents a major breakthrough in quantum computing, particularly in error correction and benchmarking, and sets new standards for the field. Willow’s technical specifications, error correction advances, commercial potential, and comparative position against IBM and other competitors are detailed below.

---

**Google Willow Technical Specifications**

- **Qubit Count:** 105 superconducting qubits[1][2][4].
- **Average Connectivity:** 3.47 (typical 4-way in a 2D grid)[1].
- **Single-Qubit Gate Error:** 0.035% ± 0.029% (mean, simultaneous)[1].
- **Two-Qubit Gate Error:** 0.33% ± 0.18% (CZ gate, mean, simultaneous)[1].
- **Measurement Error:** 0.77% ± 0.21% (mean, simultaneous)[1].
- **Qubit Coherence Time (T1):** 68 µs ± 13 µs (Chip 1), 98 µs ± 32 µs (Chip 2)[1][4]. This is a ~5x improvement over Google’s previous Sycamore chip (20 µs)[4][3].
- **Error Correction Cycles per Second:** 909,000 (surface code cycle = 1.1 µs)[1].
- **Random Circuit Sampling (RCS) Benchmark:** 103 qubits, depth 40, XEB fidelity = 0.1%[1].
- **Estimated Time for RCS Benchmark:** Willow: 5 minutes; fastest classical supercomputer (Frontier): \(10^{25}\) years[1][2][4][3].
- **Fabrication:** Built in Google’s dedicated superconducting quantum chip facility in Santa Barbara[2][4].

---

**Quantum Error Correction Advances**

- **Exponential Error Suppression:** Willow is the first quantum processor to demonstrate that increasing the number of physical qubits in a surface code lattice *reduces* the logical error rate exponentially, rather than increasing it[2][4][3][5]. Scaling from 3×3 to 5×5 to 7×7 grids, the encoded error rate was suppressed by a factor of 2.14 each time[1][2][3].
- **Below Threshold Achievement:** Willow operates “below threshold,” meaning error correction becomes more effective as the system scales, a milestone pursued since Peter Shor’s 1995 proposal[4][3][2][5].
- **Real-Time Error Correction:** Willow demonstrated real-time error correction on a superconducting quantum system, crucial for practical computation[4][3].
- **Logical Qubit Lifetimes:** Arrays of qubits have longer lifetimes than individual physical qubits, indicating error correction is improving overall system stability[4][3].
- **Surface Code Implementation:** Willow uses surface code quantum error correction, grouping physical qubits into logical qubits for robust error suppression[3][2].

---

**Commercial Applications and Potential**

- **Benchmarking Beyond Classical:** Willow performed random circuit sampling in under five minutes—a task that would take the fastest classical supercomputer 10 septillion years, vastly exceeding the age of the universe[1][2][4][3]. This demonstrates quantum supremacy for specific benchmarks, though not yet quantum advantage for practical tasks[5].
- **Industry Impact:** Willow’s advances pave the way for quantum computers to tackle problems in drug discovery, materials science, battery design, nuclear fusion, and quantum machine learning—applications currently infeasible for classical computers[2][4][7].
- **Commercialization Strategy:** Google is focused on perfecting the technology before broad commercialization, with hints at future quantum cloud services and partnerships for real-world problem solving[2][7].
- **Quantum Security:** The exponential speedup and error correction advances signal the need for post-quantum cryptography, though practical code-breaking is still at least a decade away[5].

---

**Comparative Benchmarks: Google Willow vs. IBM and Competitors**

| Metric/Feature                | Google Willow (2024)         | IBM Condor (2023)         | IBM Osprey/Eagle (2022) | SpinQ Superconducting QPU | Comments |
|-------------------------------|------------------------------|---------------------------|-------------------------|---------------------------|----------|
| **Qubit Count**               | 105                          | 1,121                     | 433/127                 | Configurable (various)    | IBM leads in raw qubit count[8][9] |
| **Qubit Quality (T1 time)**   | 68–98 µs                     | Not specified (lower)     | Not specified           | Up to 100 µs              | Willow’s coherence time is best-in-class[1][9] |
| **Error Correction**          | Exponential suppression, below threshold | Error mitigation, not full error correction | Error mitigation         | High-fidelity gates, scalable error correction | Willow is first to demonstrate exponential error suppression with scaling[2][4][8][9] |
| **Benchmark (RCS)**           | 5 min (Willow) vs. \(10^{25}\) yrs (Frontier) | Not specified            | Circuits of 100+ qubits, 3,000 gates | Not specified            | Willow sets new standard for quantum supremacy[1][2][4][8] |
| **Fabrication**               | Dedicated facility, Santa Barbara | Modular scaling, ensemble chips | Modular scaling         | Dedicated production line | Both Google and IBM invest in scalable manufacturing[4][8][9] |
| **Commercialization**         | Focused on technology perfection, future cloud services | Modular systems, roadmap to fault-tolerance by 2030 | Cloud access, partnerships | Industrial-grade, customizable QPUs | IBM is more aggressive in commercialization; Google prioritizes quality[8][9] |

- **IBM’s Approach:** IBM’s Condor chip broke the 1,000-qubit barrier, focusing on modular scaling and linking chips into larger ensembles. However, IBM’s chips still operate in a noisy, error-prone regime, with error mitigation but not full error correction. IBM’s roadmap targets useful error-corrected qubits by 2030[8].
- **Google’s Approach:** Willow prioritizes qubit quality and error correction over raw qubit count, achieving exponential error suppression and setting new benchmarks for quantum supremacy. Google’s strategy is to perfect the technology before broad commercialization[8][4][2].
- **SpinQ and Others:** SpinQ offers industrial-grade superconducting QPUs with high coherence times and customizable configurations, aiming for practical, scalable quantum solutions[9].

---

**Limitations and Outlook**

- **Quantum Advantage Not Yet Achieved:** Willow demonstrates quantum supremacy for specific benchmarks but has not yet reached quantum advantage—solving practical, real-world problems faster and cheaper than classical computers. Achieving quantum advantage will require thousands of logical qubits and further advances in error correction and scaling[5][8].
- **Scaling Challenges:** Building large-scale, fault-tolerant quantum computers will require overcoming engineering challenges in fabrication, control, and error management as systems grow to thousands or millions of qubits[8][4].
- **Commercial Applications Timeline:** Experts estimate that “very useful” quantum computers may be 10–30 years away, with intermediate-scale quantum advantage possible in 2–5 years for specialized tasks[5].

---

**Key Takeaways**

- **Willow’s breakthroughs in error correction and benchmarking mark a turning point for quantum computing, demonstrating exponential error suppression and computational power far beyond classical systems[1][2][4][3][8].**
- **Google’s focus on quality over quantity sets Willow apart from competitors like IBM, which emphasize scaling qubit counts[8].**
- **Commercial applications are on the horizon, but practical quantum advantage and widespread deployment remain several years away[5].**
- **The quantum computing race is accelerating, with Google, IBM, SpinQ, and others pushing the boundaries of hardware, error correction, and commercialization[8][9].**

Willow’s debut signals that the era of practical quantum computing is approaching, with exponential advances in error correction and computational benchmarks laying the foundation for future real-world applications.

## Citations

### 1. Meet Willow, our state-of-the-art quantum chip - Google Blog

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

### 2. Google Quantum AI

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

### 3. Meet Willow, our state-of-the-art quantum chip

**URL:** https://www.youtube.com/watch?v=W7ppd_RY-UE

## Google Quantum AI
##### Dec 09, 2024 (0:06:39)
The Google Quantum AI team is proud to announce Willow, our latest quantum chip. Willow has state-of-the-art performance across a number of metrics, enabling two major achievements. 

First, Willow can reduce errors exponentially as quantum processors scale up using more qubits. This cracks a key challenge in quantum error correction that the field has pursued for almost 30 years. 

Second, Willow performed a standard benchmark computation in under five minutes that would take one of today’s most powerful supercomputers 10 septillion years! That is 10^25 or 10,000,000,000,000,000,000,000,000 years — a number that vastly exceeds the age of the universe.

With these and other breakthroughs, the Willow chip is a major step toward building a useful, large-scale quantum computer that could harness quantum mechanics by advancing scientific discovery and tackling some of society's greatest challenges.

Hear directly from Director of Hardware, Julian Kelly, who leads the Google Quantum AI team driving this groundbreaking research.

Additional Resources:
        - Explore the full publication in Nature → https://goo.gle/3VoqLka 
        - Read more on The Keyword → https://goo.gle/3Bik6Bk
        - Get the technical details from the Quantum AI team → https://goo.gle/3ZCPNyB
        - Read about our new free Coursera course on quantum error correction → https://goo.gle/3DdRjyc
         - Learn more about our roadmap → https://goo.gle/3Vtk9AY

Subscribe to Google Quantum AI → https://goo.gle/QuantumAI

#GoogleQuantumAI  #QuantumAI... {ts:0} (inspiring music)
(waves rolling) - [Narrator] The systems
behind our universe
{ts:5} are quantum mechanical. They shift and change with the problems they're tasked to solve.
{ts:11} Exploring a vast array
of options all at once. And like nature, quantum
computing is responsive to the environments it works within,
{ts:21} leading us to new breakthroughs for tomorrow's most challenging problems. Introducing our latest
quantum computing chip,
{ts:31} developed to learn and evolve like the natural world around us. Willow from Google Quantum AI.
{ts:40} - Hi, I'm Julian Kelly, Director of Hardware at Google Quantum AI. And today, on behalf of our amazing team,
{ts:49} I'm proud to announce Willow. Willow is Google's newest and most powerful superconducting
quantum computing chip,
{ts:56} and the next step in our path towards building large-scale
quantum computers and exploring your applications.
{ts:63} I have been fascinated
with quantum computing since I first experimented
with qubits in 2008. And since coming to Google in 2015,
{ts:70} it has been a dream to... make our mission a reality, building quantum computers for otherwise unsolvable problems.
{ts:79} We launched our first
chip Foxtail in 2017, followed by Bristlecone in
2018 and Sycamore in 2019, which powered our milestone one,
{ts:90} the first quantum computer to surpass the best
classical supercomputer on a computational task,
random circuit sampling.
{ts:98} Over the years with Sycamore, we have been able to squeeze a remarkable amount of
performance from our hardware,
{ts:104} including achieving a
scalable logical qubit in our milestone two. But we've ultimately been limited
{ts:111} by quantum coherence times, the length of time qubits
maintain their intended state. With Willow, we've made
a huge step forward.
{ts:120} We've increased quantum coherence times by a factor of 5, going from 20 microseconds in Sycamore
{ts:126} to 100 microseconds in Willow, and we've accomplished this all without sacrificing any of the features
{ts:133} that made our systems so successful. This advancement was enabled by our new dedicated
superconducting quantum chip
{ts:141} fabrication facility in Santa Barbara, one of only a few in the world. And we... 're seeing exciting
developments coming from Willow,
{ts:149} which has already surpassed Sycamore's breakthrough demonstrations. Our logical qubits now operate
{ts:156} below the critical quantum
error correction threshold, a long sought-after goal for
the quantum computing field since the theory was
discovered in the '90s,
{ts:167} and we've achieved it for
the first time with Willow. Errors are exponentially
suppressed in our logical qubits as error rates are halved each
time we add physical qubits
{ts:176} and scale from distance 3
to 5 to 7 surface codes. Additionally, our logical qubit lifetimes are now much longer than
all of the lifetimes
{ts:187} of the physical qubits that compose them. This means that even as
we make our quantum chips larger and more complex
by adding more cubits,
{ts:195} we can use quantum error correction to actually improve their accuracy. We've pitted Willow
{ts:201} against one of the world's
most powerful supercomputers with the random circuit
sampling benchmark. The results are pretty surprising.
{ts:210} By our best estimates, a calculation that takes... Willow under 5 minutes would take the fastest
supercomputer 10 to the 25 years.
{ts:221} That's a 1 with 25 zeros following it, or a timescale way longer
than the age of the universe. This result highlights the
exponentially growing gap
{ts:232} between classical and quantum computation for certain applications. Let's talk about the hardware approach
{ts:239} we've pioneered at Google Quantum AI that makes these things possible. Our tuneable qubits and couplers
{ts:246} enable super fast gates and operations to achieve low error rates, reconfigurability to
optimize hardware in situ
{ts:254} and run multiple applications and high connectivity to
efficiently express algorithms. We leverage this tunability
{ts:262} to enable reproducible high
performance across the device. Let me explain. A challenge in superconducting qubits is
{ts:270} that not all of them are created equal. Some are outliers with
uncharacteristically high errors, but here's where our
tunable cubits really shine.
{ts:280} We're able to fix these outlier qubits by reconfiguring them to perform in line with the rest of the device.... {ts:286} And we can go one step further by having our researchers use tunability to continuously develop
new calibration strategies
{ts:292} that push errors down across
all qubits with software. Let's quantify this and
nerd out for a minute on a quantum computer text specs.
{ts:303} We have number of qubits. Connectivity is the average
number of interactions each qubit can perform with its neighbors.
{ts:312} We quantify error probabilities for running simultaneous operations. Single qubit gates, two
qubit gates, and measurement.
{ts:320} Coherence time measures
how long each qubit can retain its information. Measurement rate is
{ts:325} how many computations
we can run per second. An application performance
is a full system benchmark. Willow hits a sweet spot
across the full list.
{ts:337} It has a large number of
qubits with high connectivity and can run diverse applications. We measure low mean error
rates across all operations
{ts:346} with multiple native two qubit gates. We have greatly increased T1 times, we have very high measurement rates,
{ts:354} and Willow is below the
error correction threshold and performs random circuit sampling far beyond what is possible... with classical computers.
{ts:363} Looking to the future with Willow, we continue our journey
towards building large scale and useful error-corrected
quantum computers
{ts:371} that will push the boundaries of science and the exploration of nature. With future commercially
useful applications
{ts:377} in areas like pharmaceuticals,
batteries, and fusion power, we are excited to solve the otherwise unsolvable
problems of tomorrow.
{ts:389} (inspiring music)

## Metadata

```json
{
  "planId": "plan_1",
  "executionTime": 15458,
  "replanned": false
}
```

## Reasoning Insights

- Query complexity: high (3.00)
- Detected domains: physics, recent
- Temporal focus: specific timeframe
- Using advanced heuristic analysis

**Confidence:** 85.0%

## Planning Log

```
🎯 GOAP Planning & Execution Log
================================
🧠 Strange Loop Reasoning:
  • Query complexity: high (3.00)
  • Detected domains: physics, recent
  • Temporal focus: specific timeframe
  • Using advanced heuristic analysis
  • Confidence: 85.0%

📋 Plan Execution Summary:
  • Steps executed: 4
  • Success: Yes
  • Replanned: No
```
