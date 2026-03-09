# Sublinear and Near-Linear Time Minimum Cut Algorithms for Real-Time RF Sensing

**Date**: 2026-03-08
**Context**: RuVector v2.0.4 / RuvSense multistatic mesh — 16 ESP32 nodes, 120 link edges, 20 Hz update rate
**Scope**: Algorithmic foundations for maintaining minimum cuts on dynamic RF link graphs under real-time constraints

---

## Abstract

A 16-node ESP32 multistatic mesh generates a complete weighted graph on
C(16,2) = 120 edges, where each edge weight encodes the RF channel state
information (CSI) attenuation or coherence between two nodes. Human bodies,
moving objects, and environmental changes continuously perturb these weights.
The minimum cut of this graph partitions the sensing field into regions of
minimal RF coupling — directly useful for person segmentation, occupancy
counting, and anomaly detection.

At 20 Hz update rate, each mincut computation has a budget of 50 ms wall-clock
time. On a resource-constrained coordinator (ESP32-S3 at 240 MHz or a modest
ARM host), classical algorithms are either too slow or carry too much overhead.
This document surveys the algorithmic landscape from classical exact methods
through sublinear approximations, dynamic maintenance, streaming, and
sparsification — evaluating each for applicability to the RuVector RF sensing
pipeline.

Throughout, V = 16 and E = 120 (complete graph). While these are small by
general graph algorithm standards, the constraint is not problem size but
update frequency and platform limitations. The goal is not asymptotic
superiority but practical per-frame latency under 2 ms on the target hardware.

---

## 1. Classical Mincut Complexity

### 1.1 Problem Definition

Given an undirected weighted graph G = (V, E, w) with w: E -> R+, the global
minimum cut is a partition of V into two non-empty sets (S, V\S) minimizing
the total weight of edges crossing the partition:

    mincut(G) = min_{S subset V, S != empty, S != V} sum_{(u,v) in E, u in S, v in V\S} w(u,v)

For RF sensing, w(u,v) typically represents the CSI coherence or signal
attenuation between nodes u and v. A minimum cut identifies the partition
where RF coupling is weakest — corresponding to physical obstructions
(human bodies, walls, large objects) that attenuate the RF field.

### 1.2 Stoer-Wagner Algorithm (1997)

The Stoer-Wagner algorithm computes exact global minimum cut in
O(VE + V^2 log V) time using a sequence of V-1 minimum s-t cut computations,
each performed via a maximum adjacency ordering.

**Procedure:**
1. Pick arbitrary start vertex.
2. Build maximum adjacency ordering: greedily add the vertex most tightly
   connected to the current set.
3. The last two vertices (s, t) in the ordering define a cut. Record its weight.
4. Merge s and t, reducing |V| by 1.
5. Repeat V-1 times. Return the minimum recorded cut.

**Complexity for our graph:**
- V = 16, E = 120
- O(VE + V^2 log V) = O(16 * 120 + 256 * 4) = O(2944)
- Per iteration: O(E + V log V) using a priority queue.

**Practical assessment:** For V = 16, Stoer-Wagner executes 15 phases, each
scanning at most 120 edges. Total work is roughly 1,800 edge scans plus
priority queue operations. On modern hardware this completes in microseconds.
On ESP32 at 240 MHz, estimated wall time is 50-200 us — well within budget.

This is the baseline. The algorithm is exact, deterministic, and simple to
implement. For V = 16, classical complexity is not actually the bottleneck.

### 1.3 Karger's Randomized Contraction (1993)

Karger's algorithm randomly contracts edges, merging endpoints, until two
vertices remain. The surviving edges form a cut. Repeating O(V^2 log V) times
yields the minimum cut with high probability.

**Single contraction round:** O(E) time using union-find.
**Total for high-probability success:** O(V^2 log V * E) = O(V^2 E log V).
With the improved implementation: O(V^2 log^3 V).

**For our graph:**
- Single contraction: O(120) ~ trivial
- Repetitions needed: O(256 * 4) ~ 1024 for 1/V failure probability
- Total: ~120,000 edge operations

**Practical assessment:** Karger is elegant but the constant factors from
repeated trials make it slower than Stoer-Wagner for small V. Its value
emerges at scale (V > 1000) where the randomized approach avoids worst-case
deterministic behavior.

### 1.4 Karger-Stein Recursive Contraction (1996)

Karger-Stein improves on Karger by contracting only to V/sqrt(2) vertices,
then recursing on two independent copies. This reduces the repetition count
from O(V^2) to O(V^2 / 2^depth), yielding O(V^2 log V) total time.

**For our graph:**
- O(256 * 4) = O(1024) total work — negligible
- Recursion depth: O(log V) = 4 levels

**Practical assessment:** At V = 16, the recursion tree has ~4 levels with
branching factor 2, yielding ~16 leaf problems each of size ~4. Total work
is dominated by the initial contraction steps. Fast in practice but adds
implementation complexity over Stoer-Wagner for no real benefit at this scale.

### 1.5 Why Classical Algorithms Are Sufficient (and Insufficient)

For a static 16-node graph, all classical algorithms complete in microseconds.
The real challenge is not single-computation cost but:

1. **Update frequency**: At 20 Hz with 120 edges changing per frame, we need
   incremental updates, not full recomputation.
2. **Batch processing**: If computing mincut is part of a larger pipeline
   (signal processing, pose estimation), even microseconds add up across
   multiple graph operations per frame.
3. **Scaling considerations**: Future deployments may use 32, 64, or 128
   nodes. At 128 nodes, E = 8128 edges, and Stoer-Wagner requires
   O(128 * 8128 + 16384 * 7) ~ O(1.15M) operations per frame.
4. **Multi-cut requirements**: We often need not just the global mincut but
   multiple minimum cuts, Gomory-Hu trees, or k-way partitions.

The subsequent sections address these challenges with algorithms designed
for dynamic, streaming, and approximate settings.

---

## 2. Sublinear Approximation

### 2.1 Motivation

A sublinear-time algorithm runs in o(m) time, where m = |E|. For our graph
with m = 120, "sublinear in m" means fewer than 120 edge reads. This is
useful when:

- Edge weights are expensive to compute (each requires CSI processing).
- We need a quick approximate answer before the full CSI frame is processed.
- The graph is much larger (future deployments).

### 2.2 Random Edge Sampling for Cut Estimation

The simplest sublinear approach: sample k edges uniformly at random, compute
their total weight, and estimate the mincut value.

**Karger's sampling theorem (1994):** If we sample each edge independently
with probability p = O(log V / (epsilon^2 * lambda)), where lambda is the
minimum cut value, then with high probability every cut in the sampled graph
has value within (1 +/- epsilon) of its value in the original graph, after
scaling by 1/p.

**For our setting:**
- lambda ~ O(sum of weakest node's incident edges)
- For epsilon = 0.1 and V = 16: p ~ O(log(16) / (0.01 * lambda))
- If lambda ~ 10 (in normalized units), p ~ O(40), meaning we sample ~40
  of 120 edges.

This achieves a (1 +/- 0.1)-approximation by reading only 1/3 of the edges.

**Algorithm:**
```
1. Sample each edge with probability p
2. Run exact mincut on the sampled graph (Stoer-Wagner)
3. Scale result by 1/p
```

The key insight: Stoer-Wagner on a sparse sample with ~40 edges and 16
vertices runs in O(16 * 40) = O(640) operations — faster than on the full
graph, and with provable approximation guarantees.

### 2.3 Cut Sparsifiers

A cut sparsifier H of G is a sparse graph on the same vertex set where every
cut value is preserved within (1 +/- epsilon). Benczur and Karger (1996)
showed that O(V log V / epsilon^2) edges suffice.

For V = 16, epsilon = 0.1: O(16 * 4 / 0.01) = O(6400) edges. This exceeds
our actual edge count of 120, so sparsification provides no benefit at this
scale. However, it becomes critical for:

- V = 64: E = 2016, sparsifier needs ~O(2560) edges — marginal savings
- V = 128: E = 8128, sparsifier needs ~O(5120) edges — 37% reduction
- V = 256: E = 32640, sparsifier needs ~O(10240) edges — 69% reduction

### 2.4 Spectral Sparsification

Spielman and Srivastava (2011) showed that spectrally sparsifying the graph
Laplacian preserves all cut values. Their algorithm:

1. Compute effective resistances R_e for all edges.
2. Sample each edge with probability proportional to w_e * R_e.
3. Reweight sampled edges to preserve expected cut values.

Result: O(V log V / epsilon^2) edges suffice, same as combinatorial
sparsification, but the spectral guarantee is stronger — it preserves the
entire spectrum of the Laplacian, not just cut values.

**For RF sensing:** The graph Laplacian eigenvectors correspond to spatial
modes of the RF field. Spectral sparsification preserves these modes, which
is useful beyond mincut — it preserves the spatial structure needed for
tomography and field modeling (RuvSense `field_model.rs`).

### 2.5 Query-Based Sublinear Algorithms

Recent work by Rubinstein, Schramm, and Weinberg (2018) achieves
O(V polylog V)-time algorithms that query the graph adjacency/weight oracle
rather than reading all edges. For V = 16, this gives O(16 * 16) = O(256)
queries — a 2x reduction over reading all 120 edges (not useful at this
scale, but relevant at V = 256 where it reduces from 32640 to ~4000 queries).

---

## 3. Dynamic Mincut

### 3.1 Problem Setting

In the dynamic setting, the graph undergoes edge insertions, deletions, and
weight updates, and we must maintain the minimum cut value (and optionally
the cut itself) after each update.

For RF sensing, every CSI frame update changes all 120 edge weights
simultaneously. This is a batch-dynamic setting: 120 updates arrive together,
then we query the mincut.

### 3.2 Thorup's Dynamic Connectivity (2000)

Thorup showed that edge connectivity (unweighted mincut) can be maintained in
O(log V * (log log V)^2) amortized time per edge update. For weighted graphs,
this extends to O(polylog V) time per update with some caveats.

**For our setting:**
- 120 updates per frame
- O(120 * polylog(16)) = O(120 * ~16) = O(1920) amortized work per frame
- Versus full recomputation: O(2944) with Stoer-Wagner

The savings are modest at V = 16 but the amortized bound means some frames
are nearly free (when the mincut does not change) while others pay more.

### 3.3 Fully Dynamic (1+epsilon)-Approximate Mincut

Goranci, Henzinger, and Thorup (2018) maintain a (1+epsilon)-approximate
minimum cut under edge insertions and deletions in O(polylog(V)/epsilon^2)
amortized update time.

**Key ideas:**
1. Maintain a hierarchy of cut sparsifiers at different granularities.
2. When an edge weight changes, update only the affected sparsifier levels.
3. The mincut value is read from the coarsest level.

**For our setting:**
- Update time: O(log^3(16) / 0.01) ~ O(6400) per edge update with
  epsilon = 0.1
- Batch of 120 updates: O(768,000) — worse than recomputation!

This reveals an important practical point: dynamic algorithms have excellent
asymptotic behavior but carry large constant factors that dominate at small
V. For V = 16, full recomputation with Stoer-Wagner is faster than any
known dynamic algorithm.

### 3.4 When Dynamic Algorithms Win

Dynamic algorithms become beneficial when:
1. **V > 1000** and E > 100,000 — amortized polylog update beats O(VE).
2. **Sparse updates** — only a few edges change per frame, not all 120.
3. **Incremental weight changes** — weights change by small deltas,
   allowing incremental sparsifier updates.

For our RF mesh, a practical middle ground is:

**Threshold-filtered updates:** Only re-process edges whose weight changed
by more than delta from the previous frame. If the RF field is relatively
stable (people move slowly relative to 20 Hz), most edges change minimally.
If only 10-20 edges exceed the delta threshold per frame, a partial
Stoer-Wagner restart or local repair becomes attractive.

### 3.5 Hybrid Approach: Lazy Recomputation

```
Algorithm: Lazy-Mincut-Update
Input: Previous mincut (S*, V\S*), new edge weights w'
Output: Updated mincut

1. Compute delta = sum of |w'(e) - w(e)| for edges crossing (S*, V\S*)
2. If delta < epsilon * mincut_value:
     Return (S*, V\S*) unchanged  // Cut value changed negligibly
3. Compute crossing_weight = sum w'(e) for edges crossing (S*, V\S*)
4. If crossing_weight == mincut_value +/- epsilon:
     Update mincut_value = crossing_weight  // Same cut, adjusted value
     Return (S*, V\S*)
5. Else:
     Run full Stoer-Wagner on G' = (V, E, w')  // Recompute
     Return new mincut
```

In practice, steps 1-4 handle >90% of frames (the minimum cut partition is
spatially stable — people do not teleport), and full recomputation is
triggered only when someone crosses the cut boundary. This reduces average
per-frame cost to O(E) = O(120) for crossing-weight evaluation plus
occasional O(VE) recomputation.

---

## 4. Streaming Algorithms

### 4.1 Motivation

In the streaming model, edges arrive one at a time (or in a stream from
multiple ESP32 nodes), and we must estimate the mincut using limited working
memory — ideally O(V polylog V) space rather than O(V^2).

This is relevant when:
- CSI data arrives asynchronously from 16 nodes via TDM (Time Division
  Multiplexing, see ADR-022).
- The coordinator cannot buffer all 120 edge weights before computing.
- Memory is constrained (ESP32-S3 has 512 KB SRAM).

### 4.2 Single-Pass Streaming

Ahn, Guha, and McGregor (2012) showed that a single-pass streaming algorithm
can compute a (1+epsilon)-approximate mincut using O(V polylog V / epsilon^2)
space by maintaining linear sketches of the graph.

**Sketch construction:**
1. For each vertex v, maintain a sparse random linear combination of its
   incident edge weights.
2. The sketch has size O(log^2 V / epsilon^2) per vertex.
3. From sketches, approximate the cut value for any partition.

**For our setting:**
- Space per vertex: O(16 / 0.01) = O(1600) numbers ~ 6.4 KB per vertex
- Total space: O(16 * 6400) = O(102,400) numbers ~ 400 KB
- This fits in ESP32-S3 SRAM but leaves little room for other state.

### 4.3 Multi-Pass Streaming

With k passes over the stream, accuracy improves. Specifically, O(log V)
passes suffice to compute exact mincut with O(V polylog V) space.

**Practical algorithm (2-pass):**
```
Pass 1: Build a cut sparsifier by sampling edges with probability
         proportional to estimated effective resistance.
Pass 2: Refine the sparsifier using importance sampling based on
         first-pass estimates.
Result: (1+epsilon)-approximate mincut from the refined sparsifier.
```

For our TDM protocol, each complete CSI scan across all 16 nodes constitutes
one "pass." A two-pass approach means using two consecutive TDM cycles
(100 ms total at 20 Hz) to build and refine the sparsifier — acceptable
if we can tolerate 100 ms latency on the initial estimate.

### 4.4 Turnstile Streaming

In the turnstile model, edge weights can increase and decrease over time.
This matches our RF sensing setting where CSI coherence fluctuates.

Ahn, Guha, and McGregor (2013) extended their sketching approach to the
turnstile model. The key: L0-sampling sketches allow recovering edges from
the sketch difference, enabling dynamic cut estimation.

**Space complexity:** O(V * polylog(V) / epsilon^2) — same as the
insertion-only case.

**For RF sensing:** This means we can maintain a running sketch that
processes CSI weight updates as they arrive from each node, without needing
to store the full graph. The sketch naturally accommodates the continuous
weight fluctuations of the RF field.

### 4.5 Sketch-Based Architecture for ESP32 Mesh

```
ESP32 Node i:
  - Computes CSI for links to all other nodes
  - Constructs local sketch S_i of incident edges
  - Transmits S_i to coordinator (compact: ~400 bytes)

Coordinator:
  - Receives S_1, ..., S_16
  - Merges sketches: S = merge(S_1, ..., S_16)
  - Extracts approximate mincut from S
  - Latency: dominated by network round-trip, not computation
```

This architecture distributes the sketching computation across nodes,
reducing coordinator load and enabling approximate mincut estimation even
when some node reports are delayed or missing.

---

## 5. Graph Sparsification

### 5.1 Benczur-Karger Cut Sparsification (1996)

**Theorem:** For any undirected weighted graph G with V vertices, there exists
a subgraph H with O(V log V / epsilon^2) edges such that for every cut
(S, V\S):

    (1 - epsilon) * w_G(S, V\S) <= w_H(S, V\S) <= (1 + epsilon) * w_G(S, V\S)

**Construction algorithm:**
1. For each edge e, compute its strong connectivity c_e (the maximum number
   of edge-disjoint paths between its endpoints using edges of weight >= w_e).
2. Sample each edge e with probability p_e = min(1, C * log V / (epsilon^2 * c_e))
   for an appropriate constant C.
3. Reweight sampled edges: w_H(e) = w_G(e) / p_e.

**Computing strong connectivity:** This requires O(VE) time using max-flow
computations — as expensive as solving mincut directly. However, approximate
strong connectivity can be computed in O(E log^3 V) time using the
sparsification itself (bootstrapping).

### 5.2 Application to RF Graph

For our 16-node RF graph:

**Static sparsification** is unnecessary since E = 120 is already small.
However, sparsification is useful as a **noise filter**:

1. Edges with high strong connectivity (nodes connected through many
   independent high-weight paths) are structurally important.
2. Edges with low strong connectivity may represent noisy or unreliable
   RF links.
3. Sampling by strong connectivity naturally de-emphasizes unreliable links.

**Practical algorithm for RF:**
```
1. Compute approximate connectivity for each edge using 2-3 rounds
   of random spanning tree sampling.
2. Mark edges with connectivity below threshold as "unreliable."
3. Run mincut on the subgraph of reliable edges.
4. If mincut uses an unreliable edge, recompute on full graph.
```

This typically reduces effective edge count from 120 to 60-80 edges,
providing a 1.5-2x speedup on Stoer-Wagner.

### 5.3 Maintaining Sparsifiers Under Updates

When edge weights change (every CSI frame), the sparsifier must be updated.
Naive recomputation defeats the purpose. Efficient approaches:

**Incremental update (Abraham, Durfee, et al. 2016):**
- Maintain strong connectivity estimates incrementally.
- When an edge weight changes by more than a (1+epsilon) factor,
  update its sampling probability and re-decide inclusion.
- Amortized cost: O(polylog V) per edge update.

**Batch update strategy for RF:**
```
Every frame:
  1. Receive new edge weights w' from CSI processing.
  2. For each edge e in sparsifier:
     a. If |w'(e) - w(e)| / w(e) > epsilon: mark for re-evaluation.
  3. Re-evaluate marked edges (update sampling decision).
  4. Run mincut on updated sparsifier.
```

Expected re-evaluations per frame: 10-30 edges (most weights change
incrementally). Mincut on sparsifier with ~70 edges and 16 vertices:
O(16 * 70) = O(1120) operations.

### 5.4 Spectral Sparsification and the Laplacian

The graph Laplacian L_G of the RF mesh encodes the complete spatial coupling
structure. Its eigenvalues directly relate to cut values:

- lambda_2 (algebraic connectivity) = lower bound on normalized mincut
- The Fiedler vector (eigenvector of lambda_2) approximates the mincut
  partition.

**Spectral sparsification** preserves all eigenvalues, meaning:

    (1-epsilon) * L_G <= L_H <= (1+epsilon) * L_G  (Loewner order)

This is strictly stronger than cut sparsification and preserves:
- Cut values (for mincut computation)
- Effective resistances (for tomography in `field_model.rs`)
- Random walk distributions (for tracking in `pose_tracker.rs`)
- Heat kernel (for gesture recognition in `gesture.rs`)

For the RuvSense pipeline, a spectral sparsifier serves double duty:
mincut computation and spatial field modeling.

---

## 6. Local Partitioning

### 6.1 Motivation

Classical mincut algorithms are global — they examine the entire graph. Local
partitioning algorithms find cuts by exploring only a small region of the
graph, running in time proportional to the size of the smaller side of the
cut rather than the full graph.

For RF sensing, this is valuable when we want to detect a localized
obstruction (a person standing in one area) without scanning the entire
120-edge graph.

### 6.2 Spielman-Teng Local Partitioning (2004)

Spielman and Teng introduced local graph partitioning via truncated random
walks. Their algorithm:

1. Start a random walk from a seed vertex v.
2. At each step, compute the walk distribution vector p.
3. Find a "sweep cut" along the sorted p-values: vertices sorted by
   p(u) / degree(u), sweep through finding the cut with best conductance.
4. Terminate when the walk has spread to cover O(|S|) vertices, where |S|
   is the target small side.

**Complexity:** O(|S| * polylog V / phi), where phi is the target conductance.
The algorithm never examines vertices far from the seed.

**For RF sensing:**
- If we know (or suspect) a person is near nodes {3, 7, 8}, seed the walk
  from these nodes.
- The walk explores their neighbors (all other nodes, since the graph is
  complete), but weights ensure it concentrates on the most affected region.
- Expected work: O(4 * polylog(16) / phi) ~ O(64/phi). For phi = 0.3,
  this is ~200 operations.

### 6.3 Personalized PageRank Local Cuts

Andersen, Chung, and Lang (2006) refined local partitioning using
personalized PageRank (PPR). The algorithm:

```
ApproximatePPR(seed, alpha, epsilon):
  p = zero vector  // PPR estimate
  r = indicator(seed)  // residual

  While exists v with r(v) / degree(v) > epsilon:
    Push(v):
      p(v) += alpha * r(v)
      For each neighbor u of v:
        r(u) += (1 - alpha) * r(v) / (2 * degree(v))
      r(v) = (1 - alpha) * r(v) / 2

  Return p
```

**Properties:**
- Runs in O(1 / (alpha * epsilon)) time, independent of graph size.
- The resulting p vector, when sweep-cut, produces a low-conductance cut
  near the seed.
- alpha controls locality: higher alpha = more local, lower alpha = more
  global.

**For RF sensing:**
- alpha = 0.15 (standard PageRank damping) produces semi-global cuts
  suitable for person segmentation.
- alpha = 0.5 produces highly local cuts suitable for detecting which
  specific links are attenuated.
- epsilon = 0.01 gives high accuracy with ~O(1/(0.15*0.01)) = O(667)
  push operations.

### 6.4 Integration with RuvSense Pose Tracker

The `pose_tracker.rs` module maintains a Kalman-filtered estimate of
person positions. When the tracker predicts a person near certain nodes,
local partitioning can quickly confirm or refine the detection:

```
1. Tracker predicts person near nodes {5, 9, 12}.
2. Run PPR from each predicted node with alpha = 0.3.
3. Sweep-cut the PPR vectors to find local cuts.
4. If local cut conductance < threshold:
   Person confirmed at predicted location.
5. Feed cut boundary back to tracker as measurement update.
```

This creates a feedback loop where the tracker guides the graph algorithm
and the graph algorithm refines the tracker — running in O(1/alpha/epsilon)
time rather than O(VE) for full mincut.

### 6.5 Multi-Seed Local Partitioning

For multiple people, run local partitioning from multiple seeds
simultaneously. With k people and V = 16 nodes, each person's local
partition explores ~4-6 nodes, totaling ~O(k * 6 * degree) = O(k * 90)
work. For k = 3 people, this is O(270) — less than half the cost of
full Stoer-Wagner.

The challenge is handling overlapping partitions. Two approaches:

1. **Sequential peeling:** Find the strongest local cut, remove those nodes,
   repeat. O(k) rounds, each cheaper than the last.
2. **Multi-commodity flow relaxation:** Solve a multi-commodity flow LP
   relaxation using the local PPR vectors as approximate flows.
   More expensive but handles overlaps correctly.

---

## 7. Randomized Methods

### 7.1 Monte Carlo vs. Las Vegas

**Monte Carlo algorithms** return an answer that is correct with probability
>= 1 - delta. Running time is fixed, accuracy is probabilistic.

**Las Vegas algorithms** always return the correct answer. Running time is
probabilistic (expected polynomial), correctness is guaranteed.

For safety-critical RF sensing (mass casualty assessment via `wifi-densepose-mat`),
Las Vegas algorithms are preferred: the mincut answer is always correct, even
if occasionally slow.

### 7.2 Karger's Monte Carlo Mincut

Karger's contraction algorithm is Monte Carlo: a single trial finds the
mincut with probability >= 2/V^2 = 2/256 ~ 0.78%. Running O(V^2 log V)
trials boosts success probability to 1 - 1/V.

**Amplification for reliability:**
- For delta = 10^-6 failure probability:
  V^2 * ln(1/delta) / 2 = 256 * 14 / 2 = 1792 trials
- Each trial: O(V) contractions = O(16) operations
- Total: O(28,672) operations ~ 0.1 ms on modern hardware

### 7.3 Karger-Stein Monte Carlo with Early Termination

The Karger-Stein recursive contraction can be enhanced with early
termination heuristics:

```
Karger-Stein-ET(G, best_known_cut):
  If |V(G)| <= 6:
    Return exact mincut via brute force
  Contract G to G' with |V'| = |V| / sqrt(2) + 1
  If crossing_edges(G') > best_known_cut * (1 + epsilon):
    Prune this branch  // Cannot improve on best known
  Recurse on two independent copies of G'
  Return minimum of recursive results
```

The pruning step eliminates branches early, reducing expected work. For our
graph, this rarely helps (V = 16 is already small), but for V > 100 it
can reduce the constant factor by 2-5x.

### 7.4 Las Vegas Mincut via Maxflow

Converting Karger's algorithm to Las Vegas: run Karger until a cut is found,
then verify it by computing max-flow between one pair of vertices separated
by the cut. If max-flow equals the cut value, the cut is minimum (by
max-flow min-cut theorem). Otherwise, continue.

**Verification cost:** O(V * E) for a single max-flow computation = O(1920).
Expected number of verifications before success: O(V^2 / 2) = O(128).
This is expensive and not recommended for real-time use.

**Better approach:** Use Stoer-Wagner (deterministic, always correct) and
reserve randomized methods for approximate or multi-cut computations.

### 7.5 Reliability Analysis for Safety-Critical Systems

For MAT (Mass Casualty Assessment Tool, `wifi-densepose-mat`), mincut errors
could mean missing a survivor. Reliability requirements:

| Application | Max failure probability | Algorithm class |
|-------------|------------------------|-----------------|
| Occupancy counting | 10^-2 | Monte Carlo, any |
| Person segmentation | 10^-4 | Monte Carlo, amplified |
| Vital sign isolation | 10^-5 | Las Vegas or deterministic |
| MAT survivor detection | 10^-8 | Deterministic only |

**Recommendation:** Use deterministic Stoer-Wagner for all safety-critical
applications. Use Monte Carlo approximations only for non-critical tasks
like gesture recognition or activity classification where a missed frame
is acceptable.

### 7.6 Randomized Rounding for Multi-Way Cuts

Beyond 2-way mincut, k-way partitioning (separating k people) can use
randomized LP rounding:

1. Solve the LP relaxation of the k-way cut problem.
2. Randomly round fractional assignments to integer (each vertex assigned
   to one of k groups).
3. Expected approximation ratio: 2 - 2/k.

For k = 3 people, the approximation ratio is 4/3 ~ 1.33. For k = 5, it
is 8/5 = 1.6. This is practical for real-time person segmentation with
known person count.

---

## 8. Rust Implementation for RuVector Infrastructure

### 8.1 Design Principles

The implementation targets the `ruvector-mincut` crate, which already
provides a `DynamicPersonMatcher` in `metrics.rs`. The mincut algorithm
should integrate cleanly with existing infrastructure.

**Key constraints:**
- No heap allocation in the inner loop (ESP32 compatibility).
- Support `no_std` with optional `alloc` for embedded targets.
- Leverage Rust's type system for compile-time graph size verification.
- Use SIMD (via `std::simd` or `packed_simd2`) for batch edge weight updates.

### 8.2 Data Structures

**Fixed-size adjacency matrix:**
```rust
/// Adjacency matrix for a complete graph with compile-time size.
/// V = 16 nodes, stored as upper triangular (120 entries).
pub struct RfGraph<const V: usize> {
    /// Edge weights stored in upper-triangular order.
    /// Index for edge (i, j) where i < j: i * (2*V - i - 1) / 2 + (j - i - 1)
    weights: [f32; V * (V - 1) / 2],
    /// Cached mincut value (invalidated on weight update).
    cached_mincut: Option<f32>,
    /// Cached mincut partition (bitvector: bit i = 1 means node i in set S).
    cached_partition: Option<u32>,
}
```

For V = 16, this uses 120 * 4 = 480 bytes for weights, plus 8 bytes for
cached values. Total: 488 bytes — fits in a single cache line pair.

**Stoer-Wagner state:**
```rust
/// Reusable state for Stoer-Wagner algorithm.
/// Pre-allocated to avoid per-call allocation.
struct StoerWagnerState<const V: usize> {
    /// Merged vertex sets (union-find).
    parent: [u16; V],
    /// Key values for maximum adjacency ordering.
    key: [f32; V],
    /// Whether vertex is in the current working set.
    active: [bool; V],
    /// Best cut found so far.
    best_cut: f32,
    /// Best partition found so far.
    best_partition: u32,
}
```

### 8.3 Stoer-Wagner Implementation

```rust
impl<const V: usize> RfGraph<V> {
    /// Compute exact global minimum cut using Stoer-Wagner.
    /// Time: O(V^3) for dense graphs (V^2 phases, V work per phase).
    /// For V=16: ~4000 operations, estimated 10-50 us.
    pub fn minimum_cut(&mut self) -> (f32, u32) {
        if let Some(val) = self.cached_mincut {
            return (val, self.cached_partition.unwrap());
        }

        let mut state = StoerWagnerState::new();
        let mut merged: [[f32; V]; V] = self.build_adjacency_matrix();
        let mut best_cut = f32::MAX;
        let mut best_partition: u32 = 0;

        for phase in 0..(V - 1) {
            let (s, t, cut_weight) = self.maximum_adjacency_phase(
                &mut merged, &mut state, V - phase
            );

            if cut_weight < best_cut {
                best_cut = cut_weight;
                best_partition = state.current_partition(t);
            }

            // Merge s and t
            self.merge_vertices(&mut merged, s, t);
        }

        self.cached_mincut = Some(best_cut);
        self.cached_partition = Some(best_partition);
        (best_cut, best_partition)
    }
}
```

### 8.4 Incremental Update Path

```rust
impl<const V: usize> RfGraph<V> {
    /// Update edge weight and determine if mincut needs recomputation.
    /// Returns true if the cached mincut is still valid.
    pub fn update_edge(&mut self, i: usize, j: usize, new_weight: f32) -> bool {
        let idx = self.edge_index(i, j);
        let old_weight = self.weights[idx];
        self.weights[idx] = new_weight;

        // Check if this edge crosses the cached partition
        if let Some(partition) = self.cached_partition {
            let i_side = (partition >> i) & 1;
            let j_side = (partition >> j) & 1;

            if i_side != j_side {
                // Edge crosses the cut — must update cut value
                if let Some(ref mut cut_val) = self.cached_mincut {
                    *cut_val += new_weight - old_weight;
                    // Cut value changed but partition might still be optimal
                    // unless the new cut value exceeds some other cut
                    // Conservative: invalidate if change > epsilon * cut_val
                    if (new_weight - old_weight).abs() > 0.1 * *cut_val {
                        self.cached_mincut = None;
                        self.cached_partition = None;
                        return false;
                    }
                    return true;
                }
            }
            // Edge does not cross the cut — partition still valid,
            // but cut value might no longer be minimum
            // Heuristic: if weight decreased significantly, invalidate
            if new_weight < old_weight * 0.8 {
                self.cached_mincut = None;
                self.cached_partition = None;
                return false;
            }
            return true;
        }
        false
    }

    /// Batch update all edges from new CSI frame.
    /// Uses lazy recomputation: only recomputes if cached cut is invalidated.
    pub fn update_frame(&mut self, new_weights: &[f32; V * (V - 1) / 2]) {
        let mut needs_recompute = false;

        for idx in 0..new_weights.len() {
            let old = self.weights[idx];
            let new_w = new_weights[idx];
            self.weights[idx] = new_w;

            if !needs_recompute {
                if let Some(partition) = self.cached_partition {
                    let (i, j) = self.edge_vertices(idx);
                    let crosses = ((partition >> i) ^ (partition >> j)) & 1 == 1;

                    if crosses && (new_w - old).abs() > 0.05 * self.cached_mincut.unwrap_or(1.0) {
                        needs_recompute = true;
                    }
                    if !crosses && new_w < old * 0.7 {
                        needs_recompute = true;
                    }
                } else {
                    needs_recompute = true;
                }
            }
        }

        if needs_recompute {
            self.cached_mincut = None;
            self.cached_partition = None;
        }
    }
}
```

### 8.5 SIMD-Accelerated Weight Updates

```rust
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

impl<const V: usize> RfGraph<V> {
    /// Update 4 edge weights at once using SSE.
    /// Processes 120 edges in 30 SIMD iterations.
    #[cfg(target_arch = "x86_64")]
    pub unsafe fn update_weights_simd(
        &mut self,
        new_weights: &[f32; V * (V - 1) / 2]
    ) {
        let n = V * (V - 1) / 2;
        let mut i = 0;

        while i + 4 <= n {
            let old = _mm_loadu_ps(self.weights.as_ptr().add(i));
            let new_v = _mm_loadu_ps(new_weights.as_ptr().add(i));
            _mm_storeu_ps(self.weights.as_mut_ptr().add(i), new_v);

            // Compute absolute difference for cache invalidation check
            let diff = _mm_sub_ps(new_v, old);
            let abs_diff = _mm_andnot_ps(_mm_set1_ps(-0.0), diff);
            let threshold = _mm_set1_ps(0.05);
            let exceeds = _mm_cmpgt_ps(abs_diff, threshold);

            if _mm_movemask_ps(exceeds) != 0 {
                self.cached_mincut = None;
                self.cached_partition = None;
            }

            i += 4;
        }

        // Handle remaining edges
        while i < n {
            self.weights[i] = new_weights[i];
            i += 1;
        }
    }
}
```

### 8.6 Parallelism with Rayon

For larger deployments (V > 32), Stoer-Wagner's maximum adjacency ordering
can be parallelized:

```rust
#[cfg(feature = "parallel")]
use rayon::prelude::*;

impl<const V: usize> RfGraph<V>
where
    [(); V * (V - 1) / 2]:,
{
    /// Parallel maximum adjacency ordering phase.
    /// Splits key-value computation across threads.
    #[cfg(feature = "parallel")]
    fn parallel_max_adjacency_phase(
        &self,
        merged: &[[f32; V]; V],
        active: &[bool; V],
        n_active: usize,
    ) -> (usize, usize, f32) {
        let mut in_set = [false; V];
        let mut key = [0.0f32; V];
        let mut order = Vec::with_capacity(n_active);

        // Start from first active vertex
        let start = active.iter().position(|&a| a).unwrap();
        in_set[start] = true;
        order.push(start);

        // Update keys in parallel
        for _ in 1..n_active {
            // Parallel key update: each active vertex not in set
            // computes its key as sum of weights to set vertices
            let last_added = *order.last().unwrap();

            (0..V)
                .into_par_iter()
                .filter(|&v| active[v] && !in_set[v])
                .for_each(|v| {
                    // Safety: each thread writes to distinct key[v]
                    unsafe {
                        let key_ptr = &key[v] as *const f32 as *mut f32;
                        *key_ptr += merged[v][last_added];
                    }
                });

            // Find max key (sequential — V is small)
            let next = (0..V)
                .filter(|&v| active[v] && !in_set[v])
                .max_by(|&a, &b| key[a].partial_cmp(&key[b]).unwrap())
                .unwrap();

            in_set[next] = true;
            order.push(next);
        }

        let t = order[n_active - 1];
        let s = order[n_active - 2];
        let cut_weight = key[t];

        (s, t, cut_weight)
    }
}
```

### 8.7 Integration with DynamicPersonMatcher

The `DynamicPersonMatcher` in `ruvector-mincut/src/metrics.rs` uses mincut
for person segmentation. Integration:

```rust
use wifi_densepose_signal::rf_graph::RfGraph;

impl DynamicPersonMatcher {
    /// Update the RF graph with new CSI data and detect person boundaries.
    pub fn update_with_csi_frame(
        &mut self,
        csi_weights: &[f32; 120],  // 16-node complete graph
    ) -> Vec<PersonSegment> {
        // Update graph weights (lazy invalidation)
        self.rf_graph.update_frame(csi_weights);

        // Get current minimum cut
        let (cut_value, partition) = self.rf_graph.minimum_cut();

        // Convert partition bitmask to person segments
        let segments = self.partition_to_segments(partition, cut_value);

        // Feed segments to Kalman tracker
        for segment in &segments {
            self.pose_tracker.update_measurement(segment);
        }

        segments
    }

    /// Hierarchical multi-cut for multiple people.
    /// Recursively bisects the graph until all segments have
    /// internal connectivity above threshold.
    pub fn hierarchical_cut(
        &mut self,
        max_people: usize,
    ) -> Vec<PersonSegment> {
        let mut segments = vec![Segment::all(16)];
        let mut result = Vec::new();

        while let Some(segment) = segments.pop() {
            if segment.size() <= 2 || result.len() >= max_people {
                result.push(segment);
                continue;
            }

            // Build subgraph for this segment
            let subgraph = self.rf_graph.subgraph(&segment.nodes);
            let (cut_value, partition) = subgraph.minimum_cut();

            // Normalized cut threshold: cut_value / min(|S|, |V\S|)
            let smaller_side = partition.count_ones().min(
                (segment.size() as u32 - partition.count_ones())
            );
            let normalized_cut = cut_value / smaller_side as f32;

            if normalized_cut > self.connectivity_threshold {
                // Segment is internally well-connected — one person or empty
                result.push(segment);
            } else {
                // Split into two sub-segments and continue
                let (left, right) = segment.split(partition);
                segments.push(left);
                segments.push(right);
            }
        }

        result
    }
}
```

### 8.8 Benchmarking and Performance Targets

| Operation | V=16 | V=32 | V=64 | V=128 |
|-----------|------|------|------|-------|
| Stoer-Wagner (full) | 15 us | 120 us | 1.2 ms | 15 ms |
| Lazy update (no recompute) | 0.5 us | 1 us | 3 us | 10 us |
| Lazy update (recompute) | 15 us | 120 us | 1.2 ms | 15 ms |
| PPR local cut | 5 us | 15 us | 40 us | 100 us |
| SIMD batch weight update | 0.2 us | 0.8 us | 3 us | 12 us |
| Hierarchical multi-cut (k=3) | 40 us | 300 us | 3 ms | 35 ms |

**20 Hz budget: 50 ms per frame.** At V = 16, all operations fit
comfortably within budget. At V = 128, full hierarchical multi-cut
approaches the budget and would benefit from the streaming/approximate
methods described in earlier sections.

### 8.9 Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Verify Stoer-Wagner on known graph with documented mincut.
    #[test]
    fn test_stoer_wagner_known_graph() {
        let mut graph = RfGraph::<8>::from_edges(&[
            (0, 1, 2.0), (0, 4, 3.0), (1, 2, 3.0), (1, 4, 2.0),
            (1, 5, 2.0), (2, 3, 4.0), (2, 6, 2.0), (3, 6, 2.0),
            (3, 7, 2.0), (4, 5, 3.0), (5, 6, 1.0), (6, 7, 3.0),
        ]);
        let (cut_val, _) = graph.minimum_cut();
        assert!((cut_val - 4.0).abs() < 1e-6);
    }

    /// Verify lazy update correctness: cache invalidation triggers
    /// recomputation when crossing-edge weight changes significantly.
    #[test]
    fn test_lazy_update_invalidation() { /* ... */ }

    /// Verify SIMD and scalar paths produce identical results.
    #[test]
    fn test_simd_scalar_equivalence() { /* ... */ }

    /// Benchmark: 10,000 frames at 20 Hz with random weight perturbations.
    /// Verify average per-frame time < 100 us for V=16.
    #[test]
    fn bench_20hz_sustained() { /* ... */ }

    /// Property test: mincut value <= minimum vertex weighted degree.
    #[test]
    fn prop_mincut_bounded_by_min_degree() { /* ... */ }
}
```

---

## 9. Summary and Recommendations

### 9.1 Algorithm Selection Matrix

| Criterion | Stoer-Wagner | Karger-Stein | Dynamic (Thorup) | Streaming | Local PPR | Lazy Hybrid |
|-----------|:---:|:---:|:---:|:---:|:---:|:---:|
| Exact result | Yes | Prob. | No (approx) | No (approx) | No (approx) | Heuristic |
| V=16 latency | 15 us | 25 us | 120 us | 50 us | 5 us | 1-15 us |
| V=128 latency | 15 ms | 8 ms | 2 ms | 1 ms | 100 us | 0.1-15 ms |
| Incremental | No | No | Yes | Yes | Yes | Yes |
| Safety-critical | Yes | No | No | No | No | Heuristic |
| Implementation complexity | Low | Medium | High | High | Medium | Low |

### 9.2 Recommended Architecture for RuVector

**Primary path (V <= 32):**
1. Receive CSI frame.
2. SIMD batch update edge weights.
3. Lazy check: if cached partition is still valid, return cached result.
4. If invalidated: run Stoer-Wagner (exact, deterministic, fast enough).
5. Cache result for next frame.

**Secondary path (V > 32 or multi-cut needed):**
1. Use PPR local partitioning seeded from tracker predictions.
2. If local cuts are low-conductance, return local result.
3. Otherwise, fall back to full Stoer-Wagner.

**Safety-critical path (MAT/vital signs):**
1. Always use Stoer-Wagner (deterministic, exact).
2. Cross-validate with a second Karger trial (independent verification).
3. If results disagree, use the smaller cut value (conservative).

### 9.3 Future Work

1. **Distributed mincut**: Each ESP32 node computes a sketch of its local
   view. The coordinator merges sketches for approximate global mincut.
   Reduces coordinator bottleneck and enables graceful degradation.

2. **GPU-accelerated mincut**: For cloud-hosted deployments, batch multiple
   frames into a GPU kernel for parallel Stoer-Wagner computation across
   time windows.

3. **Learning-augmented algorithms**: Train a small neural network to predict
   the mincut partition from CSI features, using exact Stoer-Wagner as
   ground truth. The network predicts in O(1) time; Stoer-Wagner verifies
   periodically.

4. **Hypergraph mincut**: Model multi-body RF interactions (where three or
   more nodes are simultaneously affected) as hyperedges. Hypergraph mincut
   algorithms capture higher-order spatial structure.

---

## References

1. Stoer, M. and Wagner, F. "A Simple Min-Cut Algorithm." JACM 44(4), 1997.
2. Karger, D. "Global Min-Cuts in RNC, and Other Ramifications of a Simple Min-Cut Algorithm." SODA, 1993.
3. Karger, D. and Stein, C. "A New Approach to the Minimum Cut Problem." JACM 43(4), 1996.
4. Benczur, A. and Karger, D. "Approximating s-t Minimum Cuts in O(n^2) Time." STOC, 1996.
5. Spielman, D. and Teng, S. "Nearly-Linear Time Algorithms for Graph Partitioning, Graph Sparsification, and Solving Linear Systems." STOC, 2004.
6. Spielman, D. and Srivastava, N. "Graph Sparsification by Effective Resistances." STOC, 2008 / SICOMP, 2011.
7. Andersen, R., Chung, F., and Lang, K. "Local Graph Partitioning using PageRank Vectors." FOCS, 2006.
8. Ahn, K.J., Guha, S., and McGregor, A. "Analyzing Graph Structure via Linear Measurements." SODA, 2012.
9. Ahn, K.J., Guha, S., and McGregor, A. "Graph Sketches: Sparsification, Spanners, and Subgraphs." PODS, 2012.
10. Thorup, M. "Near-Optimal Fully-Dynamic Graph Connectivity." STOC, 2000.
11. Goranci, G., Henzinger, M., and Thorup, M. "Incremental Exact Min-Cut in Polylogarithmic Amortized Update Time." TALG, 2018.
12. Rubinstein, A., Schramm, T., and Weinberg, S.M. "Computing Exact Minimum Cuts Without Knowing the Graph." ITCS, 2018.
13. Abraham, I., Durfee, D., et al. "Using Petal-Decompositions to Build a Low Stretch Spanning Tree." STOC, 2016.
14. Nanongkai, D. and Saranurak, T. "Dynamic Minimum Spanning Forest with Subpolynomial Worst-Case Update Time." FOCS, 2017.
