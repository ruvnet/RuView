# Graph-Theoretic Foundations for RF Topological Sensing Using Minimum Cut

**Research Document RD-001**
**Date**: 2026-03-08
**Status**: Draft
**Authors**: RuView Research Team
**Related ADRs**: ADR-029 (RuvSense Multistatic Sensing), ADR-017 (RuVector Signal Integration)

---

## Abstract

This document establishes the mathematical and algorithmic foundations for a
graph-theoretic approach to RF sensing using minimum cut decomposition. We model
a mesh of 16 ESP32 WiFi nodes as a weighted graph where edges represent TX-RX
link pairs and edge weights encode CSI (Channel State Information) coherence. When
physical objects or people perturb the RF field, edge weights destabilize
non-uniformly, and minimum cut algorithms reveal the topological boundary of the
perturbation. This approach — which we term **RF topological sensing** — differs
fundamentally from classical RF localization techniques (RSSI triangulation,
fingerprinting, CSI-based positioning) in that it detects *coherence boundaries*
rather than estimating *positions*. We develop the formal mathematical framework,
survey relevant algorithms from combinatorial optimization and spectral graph
theory, and identify open research questions for this largely unexplored domain.

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Mathematical Framework](#2-mathematical-framework)
3. [Max-Flow/Min-Cut Theorem for RF Networks](#3-max-flowmin-cut-theorem-for-rf-networks)
4. [RF Mesh as Dynamic Weighted Graph](#4-rf-mesh-as-dynamic-weighted-graph)
5. [Topological Change Detection via Spectral Methods](#5-topological-change-detection-via-spectral-methods)
6. [Dynamic Graph Algorithms for Real-Time RF Sensing](#6-dynamic-graph-algorithms-for-real-time-rf-sensing)
7. [Comparison to Classical RF Sensing](#7-comparison-to-classical-rf-sensing)
8. [Open Research Questions](#8-open-research-questions)
9. [Conclusion](#9-conclusion)
10. [References](#10-references)

---

## 1. Introduction

Consider 16 ESP32 nodes deployed in a room, each capable of transmitting and
receiving WiFi CSI frames. Every ordered TX-RX pair yields a channel measurement
— amplitude and phase across OFDM subcarriers. In the absence of perturbation,
these measurements exhibit stable coherence patterns determined by room geometry,
multipath structure, and hardware characteristics.

When a person enters the room, they scatter, absorb, and reflect RF energy along
certain propagation paths. The key insight is that this perturbation is
**spatially localized**: only links whose Fresnel zones intersect the person's
body experience significant coherence degradation. The affected links form a
connected subgraph whose boundary — the set of edges connecting "disturbed" and
"undisturbed" regions of the link graph — constitutes a topological signature of
the perturbation.

We propose that **minimum cut algorithms** are the natural computational tool for
extracting this boundary. The minimum cut of a graph partitions its vertices into
two sets such that the total weight of edges crossing the partition is minimized.
When edge weights encode coherence (high weight = stable link), the minimum cut
passes through the destabilized edges, precisely identifying the perturbation
boundary.

This document develops this idea rigorously across three axes:

- **Algorithmic**: Which min-cut algorithms are suitable for real-time RF sensing?
- **Spectral**: How do eigenvalue methods complement combinatorial min-cut?
- **Comparative**: Why is topological sensing fundamentally different from
  position estimation?

### 1.1 Notation Conventions

Throughout this document we use the following conventions:

| Symbol | Meaning |
|--------|---------|
| `G = (V, E, w)` | Weighted undirected graph |
| `n = \|V\|` | Number of vertices (nodes), here n = 16 |
| `m = \|E\|` | Number of edges (TX-RX links), here m <= n(n-1)/2 = 120 |
| `w: E -> R+` | Edge weight function (CSI coherence) |
| `L` | Graph Laplacian matrix |
| `D` | Degree matrix |
| `A` | Adjacency (weight) matrix |
| `lambda_k` | k-th smallest eigenvalue of L |
| `v_k` | Eigenvector corresponding to lambda_k (Fiedler vector when k=2) |
| `C(S, V\S)` | Cut capacity: sum of weights crossing partition (S, V\S) |

---

## 2. Mathematical Framework

### 2.1 Graph Definition

We define the RF sensing graph as:

```
G = (V, E, w)
```

where:

- **V** = {v_1, v_2, ..., v_n} is the set of ESP32 nodes. In our deployment,
  n = 16.

- **E** ⊆ V × V is the set of edges. Each edge e = (v_i, v_j) represents a
  bidirectional TX-RX link between nodes i and j. For a fully connected mesh of
  16 nodes, |E| = C(16,2) = 120 edges.

- **w: E → R≥0** is the edge weight function. We define w(e) as the CSI
  coherence metric for edge e, detailed in Section 2.3.

### 2.2 Adjacency and Laplacian Matrices

The **weighted adjacency matrix** A ∈ R^{n×n} is defined as:

```
A[i,j] = w(v_i, v_j)    if (v_i, v_j) ∈ E
A[i,j] = 0               otherwise
```

The **degree matrix** D ∈ R^{n×n} is diagonal with:

```
D[i,i] = Σ_j A[i,j]
```

The **graph Laplacian** L is:

```
L = D - A
```

The Laplacian has the fundamental property that for any vector x ∈ R^n:

```
x^T L x = Σ_{(i,j) ∈ E} w(i,j) * (x_i - x_j)^2
```

This quadratic form measures the "smoothness" of x with respect to the graph
structure. Functions that vary slowly across heavily-weighted edges have small
Laplacian quadratic form.

The **normalized Laplacian** is:

```
L_norm = D^{-1/2} L D^{-1/2} = I - D^{-1/2} A D^{-1/2}
```

Its eigenvalues lie in [0, 2], making spectral comparisons across different
graph sizes more meaningful.

### 2.3 CSI Coherence as Edge Weight

For each TX-RX pair (v_i, v_j), we observe a CSI vector h_{ij}(t) ∈ C^K at
time t, where K is the number of OFDM subcarriers (typically K = 52 for
802.11n on ESP32).

We define the **temporal coherence** over a sliding window of T frames as:

```
γ_{ij}(t) = | (1/T) Σ_{τ=0}^{T-1} h_{ij}(t-τ) / |h_{ij}(t-τ)| |
```

This is the magnitude of the average normalized CSI phasor. When the channel is
static, phase vectors align and γ → 1. When the channel fluctuates (due to
movement in the Fresnel zone), phases decorrelate and γ → 0.

The **subcarrier coherence** provides a frequency-domain view:

```
ρ_{ij}(t) = |corr(|h_{ij}(t)|, |h_{ij}(t-1)|)|
```

where corr denotes the Pearson correlation across subcarrier amplitudes.

The composite edge weight is:

```
w(v_i, v_j) = α * γ_{ij}(t) + (1 - α) * ρ_{ij}(t)
```

where α ∈ [0,1] is a mixing parameter (empirically α ≈ 0.6 works well).

**Key property**: High w means a stable, unperturbed link. Low w means the link's
Fresnel zone is occupied by a scatterer.

### 2.4 Cut Definitions

A **cut** of G is a partition of V into two non-empty disjoint sets S and
S̄ = V \ S. The **capacity** (or weight) of the cut is:

```
C(S, S̄) = Σ_{(u,v) ∈ E : u ∈ S, v ∈ S̄} w(u, v)
```

The **global minimum cut** (or simply mincut) is:

```
mincut(G) = min_{∅ ⊂ S ⊂ V} C(S, S̄)
```

For a source-sink pair (s, t), the **minimum s-t cut** is:

```
mincut(s, t) = min_{S : s ∈ S, t ∈ S̄} C(S, S̄)
```

The **normalized cut** (Shi-Malik, 2000) penalizes imbalanced partitions:

```
Ncut(S, S̄) = C(S, S̄) / vol(S) + C(S, S̄) / vol(S̄)
```

where vol(S) = Σ_{v ∈ S} d(v) is the volume (total degree) of S.

### 2.5 Multi-way Cuts and k-Partitioning

For detecting multiple simultaneous perturbations (e.g., two people in different
parts of the room), we generalize to k-way cuts:

```
kcut(G) = min partition V into S_1, ..., S_k of Σ_{i<j} C(S_i, S_j)
```

The k-way minimum cut problem is NP-hard for general k, but spectral relaxation
provides practical approximate solutions via the first k eigenvectors of L.

---

## 3. Max-Flow/Min-Cut Theorem for RF Networks

### 3.1 The Max-Flow/Min-Cut Theorem

The Max-Flow/Min-Cut Theorem (Ford and Fulkerson, 1956) is one of the
foundational results in combinatorial optimization:

**Theorem**: In a flow network with source s and sink t, the maximum flow from
s to t equals the capacity of the minimum s-t cut.

```
max_flow(s, t) = mincut(s, t)
```

This duality is profound for RF sensing: the minimum cut capacity tells us the
"bottleneck" of information flow between two regions of the sensor mesh. When a
person bisects the mesh, they reduce this bottleneck by degrading the links they
occlude.

### 3.2 Ford-Fulkerson and Augmenting Paths

The Ford-Fulkerson method (1956) computes max-flow (and hence min-cut) by
repeatedly finding augmenting paths from s to t and pushing flow along them.

**Algorithm sketch**:
```
1. Initialize flow f = 0 on all edges
2. While there exists an augmenting path P from s to t in the residual graph:
   a. Find bottleneck capacity: δ = min_{e ∈ P} (capacity(e) - f(e))
   b. Augment: for each e ∈ P, f(e) += δ
3. Return f (max flow) and the reachable set from s in residual graph (min cut)
```

**Complexity**: O(m * max_flow) for integer capacities. For real-valued coherence
weights, this must be combined with Edmonds-Karp (BFS-based path selection) for
O(nm^2) worst case, or Dinic's algorithm for O(n^2 * m).

**RF application**: Ford-Fulkerson is useful when we want the minimum s-t cut
between a specific pair of node groups — for example, asking "what is the weakest
coherence boundary separating the north wall sensors from the south wall sensors?"

### 3.3 Stoer-Wagner Algorithm for Global Minimum Cut

For RF topological sensing, we typically want the **global** minimum cut — the
weakest boundary in the entire mesh — without pre-specifying source and sink.
The Stoer-Wagner algorithm (1997) computes this efficiently.

**Algorithm**:
```
STOER-WAGNER(G = (V, E, w)):
  best_cut = ∞
  while |V| > 1:
    (s, t, cut_weight) = MINIMUM_CUT_PHASE(G)
    if cut_weight < best_cut:
      best_cut = cut_weight
      best_partition = ({t}, V \ {t})  // record the cut
    G = CONTRACT(G, s, t)  // merge s and t into a single vertex
  return best_cut, best_partition

MINIMUM_CUT_PHASE(G):
  A = {arbitrary start vertex}
  while A ≠ V:
    add to A the vertex v ∈ V \ A most tightly connected to A
    // i.e., v = argmax_{u ∈ V\A} Σ_{a ∈ A} w(u, a)
  s = second-to-last vertex added
  t = last vertex added
  return (s, t, w(t))  // w(t) = Σ_{a ∈ A\{t}} w(t, a)
```

**Complexity**: O(nm + n^2 log n) using a Fibonacci heap, or O(nm log n) with a
binary heap. For our n = 16, m = 120 mesh, this is trivially fast — roughly
16 phases of 16 vertex additions = 256 operations.

**Why Stoer-Wagner is ideal for RF sensing**:

1. **No source/sink required**: The algorithm finds the global minimum cut, which
   corresponds to the weakest coherence boundary in the mesh.
2. **Deterministic**: Produces the exact minimum cut, not an approximation.
3. **Efficient for small dense graphs**: With n = 16, Stoer-Wagner runs in
   microseconds, well within real-time constraints.
4. **Returns the partition**: We get both the cut weight and the vertex partition,
   directly telling us which nodes are on each side of the perturbation boundary.

### 3.4 Karger's Randomized Algorithm

Karger's contraction algorithm (1993) provides a probabilistic approach:

**Algorithm**:
```
KARGER(G = (V, E, w)):
  while |V| > 2:
    select edge e = (u, v) with probability proportional to w(e)
    CONTRACT(G, u, v)
  return the cut defined by the two remaining super-vertices
```

A single run returns the minimum cut with probability >= 2/n^2. Repeating
O(n^2 log n) times and taking the minimum achieves high probability of
correctness.

**Complexity**: O(n^2 m) per run, O(n^4 m log n) total. Karger-Stein improves
this to O(n^2 log^3 n).

**RF application**: Karger's algorithm has an interesting property for RF sensing:
by running it multiple times, we obtain not just the minimum cut but a
**distribution over near-minimum cuts**. This distribution reveals:

- The "rigidity" of the topological boundary: if most runs return the same cut,
  the boundary is well-defined.
- Alternative boundaries: near-minimum cuts may correspond to secondary
  perturbation regions.
- Confidence intervals: the fraction of runs returning a given cut estimates
  the probability that it is the true minimum.

### 3.5 Gomory-Hu Trees for All-Pairs Min-Cut

The Gomory-Hu tree (1961) is a weighted tree T on the same vertex set V such that
for every pair (s, t), the minimum s-t cut in G equals the minimum weight edge on
the unique s-t path in T.

**Construction**: Requires n-1 max-flow computations.

**RF application**: Pre-computing the Gomory-Hu tree for the 16-node mesh
(requiring 15 max-flow computations) gives us instant access to the minimum cut
between *any* pair of nodes. This supports queries like:

- "Which node pair has the weakest mutual coherence?"
- "If I place a transmitter at node 3, which node is most 'separated' from it
  by the perturbation?"

With n = 16, the Gomory-Hu tree has 15 edges and can be computed once per
sensing frame (approximately every 100ms).

---

## 4. RF Mesh as Dynamic Weighted Graph

### 4.1 Physical Deployment Geometry

The 16 ESP32 nodes are deployed to maximize spatial coverage and link diversity.
Consider a rectangular room of dimensions L × W. A natural deployment uses:

```
Node placement (4×4 grid):

    v1 ------- v2 ------- v3 ------- v4
    |  \     / |  \     / |  \     / |
    |   \   /  |   \   /  |   \   /  |
    |    \ /   |    \ /   |    \ /   |
    v5 ------- v6 ------- v7 ------- v8
    |    / \   |    / \   |    / \   |
    |   /   \  |   /   \  |   /   \  |
    |  /     \ |  /     \ |  /     \ |
    v9 ------- v10 ------ v11 ------ v12
    |  \     / |  \     / |  \     / |
    |   \   /  |   \   /  |   \   /  |
    |    \ /   |    \ /   |    \ /   |
    v13 ------ v14 ------ v15 ------ v16
```

Every pair of nodes forms a potential link, giving a complete graph K_16 with
120 edges. However, not all links carry equal geometric information:

- **Short links** (adjacent nodes): High SNR, sensitive to nearby perturbations,
  narrow Fresnel zones.
- **Long links** (diagonal/cross-room): Lower SNR, sensitive to perturbations
  anywhere along the path, wide Fresnel zones.
- **Parallel links**: Correlated sensitivity — a perturbation affecting one likely
  affects the other.
- **Crossing links**: Complementary sensitivity — their Fresnel zone intersection
  localizes perturbations.

### 4.2 Fresnel Zone Geometry and Edge Semantics

The first Fresnel zone for a link of length d at wavelength λ is an ellipsoid
with semi-minor axis:

```
r_F = sqrt(λ * d / 4)
```

At 2.4 GHz (λ ≈ 0.125 m), a 5-meter link has r_F ≈ 0.40 m. A 10-meter link
has r_F ≈ 0.56 m.

A human body (roughly 0.4 m wide, 0.3 m deep) fully occupies the Fresnel zone
of a short link but only partially occludes a long link. This creates a natural
**spatial resolution** determined by the mesh geometry.

**Edge semantics**: An edge (v_i, v_j) in the graph represents not just a
communication link but a **spatial sensing region** — the Fresnel ellipsoid
between v_i and v_j. The edge weight w(v_i, v_j) encodes whether this sensing
region is perturbed.

### 4.3 Temporal Dynamics

The graph G(t) evolves over time as edge weights change. We sample CSI at rate
f_s (typically 10-100 Hz per link). At each time step:

```
G(t) = (V, E, w_t)
```

where w_t is the coherence vector at time t. The vertex set V and edge set E
remain constant (all 16 nodes, all 120 links), but the weight function changes.

Key temporal patterns:

- **Static environment**: All weights stable near 1.0. Minimum cut has high
  capacity (the graph is "uniformly strong").

- **Single person entering**: A cluster of edges experience weight drops. The
  minimum cut capacity decreases, and the cut partition reveals which side of
  the perturbation each node lies on.

- **Person moving**: The weight depression region migrates across the graph. The
  minimum cut tracks this migration, producing a time series of partitions.

- **Multiple people**: Multiple weight depression regions create a more complex
  landscape. Multi-way cuts or hierarchical decomposition may be needed.

### 4.4 Graph Sparsification for Scalability

While n = 16 yields a manageable 120 edges, larger deployments require
sparsification. Two approaches:

**Geometric sparsification**: Only include edges shorter than a threshold d_max,
where d_max is chosen to ensure graph connectivity. For uniformly deployed nodes,
this produces O(n) edges.

**Spectral sparsification** (Spielman-Teng, 2011): Construct a sparse graph H
with O(n log n / ε^2) edges such that for all cuts:

```
(1-ε) * C_G(S, S̄) <= C_H(S, S̄) <= (1+ε) * C_G(S, S̄)
```

This preserves all cut values within (1 ± ε) while dramatically reducing edge
count for large meshes.

### 4.5 Weighted Graph Properties Specific to RF

RF coherence graphs have distinctive properties that affect algorithm choice:

1. **Non-negative weights**: Coherence is always in [0, 1], satisfying the
   non-negativity requirement of most min-cut algorithms.

2. **Smoothness**: Edge weights change continuously (no abrupt jumps in coherence),
   meaning G(t) and G(t+1) differ by small perturbations.

3. **Spatial correlation**: Nearby edges (links with overlapping Fresnel zones)
   tend to have correlated weights.

4. **Dense but structured**: K_16 is dense (120 edges), but the weight structure
   is determined by physical geometry, making it far from a random weighted graph.

5. **Symmetry**: w(v_i, v_j) ≈ w(v_j, v_i) due to channel reciprocity
   (same frequency, same environment), so the graph is effectively undirected.

---

## 5. Topological Change Detection via Spectral Methods

### 5.1 Spectral Graph Theory Foundations

The eigenvalues of the graph Laplacian L encode fundamental structural
properties. Let 0 = λ_1 <= λ_2 <= ... <= λ_n be the eigenvalues of L with
corresponding eigenvectors v_1, v_2, ..., v_n.

Key spectral properties:

- **λ_1 = 0 always**, with v_1 = (1, 1, ..., 1) / sqrt(n).
- **λ_2 > 0 iff G is connected**. λ_2 is called the **algebraic connectivity**
  or **Fiedler value**.
- **Multiplicity of 0**: The number of zero eigenvalues equals the number of
  connected components.
- **λ_2 is a measure of graph robustness**: Higher λ_2 means the graph is harder
  to disconnect (all cuts have high capacity).

### 5.2 The Fiedler Vector and Spectral Bisection

The eigenvector v_2 corresponding to λ_2 is the **Fiedler vector**. It provides
the optimal continuous relaxation of the minimum bisection problem:

```
min_{x ∈ R^n} x^T L x    subject to    x ⊥ 1,  ||x|| = 1
```

The solution is x = v_2, and the optimal value is λ_2.

**Spectral bisection**: Partition V into S = {v : v_2[i] <= 0} and
S̄ = {v : v_2[i] > 0}. This provides an approximate minimum bisection (balanced
cut) of the graph.

**RF interpretation**: The Fiedler vector assigns each node a real value that
represents its position along the "weakest axis" of the graph. Nodes on opposite
sides of a perturbation boundary receive opposite-sign values. The magnitude
|v_2[i]| indicates how strongly node i is associated with its side of the
partition — nodes near the boundary have small |v_2[i]|.

### 5.3 Cheeger Inequality

The Cheeger constant h(G) relates the combinatorial minimum cut to spectral
properties:

```
h(G) = min_{S ⊂ V, vol(S) <= vol(V)/2}  C(S, S̄) / vol(S)
```

The **Cheeger inequality** bounds h(G) using λ_2:

```
λ_2 / 2  <=  h(G)  <=  sqrt(2 * λ_2)
```

This is powerful for RF sensing because:

1. **Lower bound (λ_2 / 2 <= h(G))**: A small Fiedler value guarantees the
   existence of a sparse cut — i.e., a coherence boundary.

2. **Upper bound (h(G) <= sqrt(2 * λ_2))**: Spectral bisection produces a cut
   whose normalized capacity is within a sqrt(λ_2) factor of optimal.

3. **Monitoring λ_2 over time**: A dropping Fiedler value signals that the
   graph's connectivity is weakening — someone is entering the room or moving to
   a position that bisects the mesh.

### 5.4 Higher Eigenvectors and Multi-Way Partitioning

For k-way partitioning (detecting multiple perturbation regions), we use the
first k eigenvectors V_k = [v_1, v_2, ..., v_k] ∈ R^{n×k}. Each node v_i gets
an embedding in R^k:

```
f(v_i) = (v_1[i], v_2[i], ..., v_k[i])
```

Running k-means clustering on these embeddings yields a spectral k-way partition.

The **higher-order Cheeger inequality** (Lee, Oveis Gharan, Trevisan, 2014)
generalizes:

```
λ_k / 2  <=  ρ_k(G)  <=  O(k^2) * sqrt(λ_k)
```

where ρ_k(G) is the k-way expansion constant.

**RF interpretation**: If the first three eigenvalues are 0, 0.05, 0.08, and
then λ_4 jumps to 0.6, this indicates two natural clusters in the coherence
graph (two perturbation regions), with the spectral gap between λ_3 and λ_4
confirming a 3-way partition is natural.

### 5.5 Spectral Change Detection

Rather than computing min-cuts from scratch each frame, we can monitor spectral
changes efficiently.

**Eigenvalue tracking**: Let λ_2(t) be the Fiedler value at time t. Define the
**spectral instability signal**:

```
Δ_λ(t) = |λ_2(t) - λ_2(t-1)| / λ_2(t-1)
```

A spike in Δ_λ(t) indicates a topological change — a new perturbation or a
significant movement event.

**Eigenvector tracking**: For smooth graph evolution, we can use eigenvalue
perturbation theory. If edge (i,j) changes weight by δw, the first-order change
in λ_2 is:

```
δλ_2 ≈ δw * (v_2[i] - v_2[j])^2
```

This means edges with large (v_2[i] - v_2[j])^2 — edges that cross the Fiedler
cut — have the most impact on algebraic connectivity. These are precisely the
boundary edges we care about.

### 5.6 Normalized Spectral Clustering (Shi-Malik)

The normalized cut objective:

```
Ncut(S, S̄) = C(S, S̄) / vol(S) + C(S, S̄) / vol(S̄)
```

is relaxed to:

```
min_{x} x^T L x / x^T D x    subject to    x ⊥ D * 1
```

The solution is the generalized eigenvector problem Lx = λDx, i.e., the
eigenvectors of the normalized Laplacian L_norm = D^{-1/2} L D^{-1/2}.

**Why normalized cut matters for RF**: In a mesh with heterogeneous link
densities (e.g., corner nodes with fewer strong links), the unnormalized minimum
cut may trivially separate a low-degree node. The normalized cut penalizes this,
preferring balanced partitions that correspond to genuine physical boundaries
rather than geometric artifacts of node placement.

---

## 6. Dynamic Graph Algorithms for Real-Time RF Sensing

### 6.1 The Real-Time Constraint

RF sensing requires processing at the CSI frame rate. For 16 nodes transmitting
round-robin at 10 Hz each, we get 16 frames per 100 ms cycle, yielding an
update rate of 10 Hz for the full graph. Each update changes up to 15 edge
weights (all links from the transmitting node).

**Latency budget**: To support real-time applications (gesture recognition,
intrusion detection), we need total processing time under 10 ms per update cycle.
On a modern processor, this is generous — but motivates efficient algorithms for
future scaling to larger meshes.

### 6.2 Incremental Min-Cut Algorithms

When only a few edge weights change between frames, recomputing the global
min-cut from scratch is wasteful. Incremental algorithms maintain the min-cut
under edge updates.

**Weight increase (edge strengthening)**:
If an edge weight increases, the minimum cut can only increase or stay the same.
If the modified edge does not cross the current min-cut, the cut is unchanged.
If it does cross the cut, the new min-cut value is at least the old value — we
need to verify whether the current partition is still optimal, potentially by
running a single max-flow computation in the residual graph.

**Weight decrease (edge weakening)**:
If an edge weight decreases and it crosses the current min-cut, the cut capacity
decreases by the weight change — no recomputation needed. If the edge is internal
to one side of the cut, the cut is unchanged. However, a new lower-capacity cut
may have emerged, requiring recomputation.

### 6.3 Decremental Min-Cut Maintenance

The critical case for RF sensing is edge weight *decreases* (a link becoming
less coherent due to a new perturbation). This is the "decremental" case, which
is harder than incremental.

**Approach 1: Lazy recomputation with certificate**

Maintain the Gomory-Hu tree T. When edge (u, v) in G decreases weight by δ:

1. If (u, v) is not on any minimum-weight path in T, the tree is unchanged.
2. If (u, v) is in the Gomory-Hu tree or affects a bottleneck path, recompute
   only the affected subtree.

For our n = 16 graph, full Gomory-Hu tree recomputation (15 max-flow instances)
is fast enough that lazy strategies provide limited benefit. But for larger
meshes (64+ nodes), this becomes important.

**Approach 2: Threshold-triggered recomputation**

Only recompute when the total weight change since last computation exceeds a
threshold θ:

```
Σ_{e ∈ E} |w_t(e) - w_{t_last}(e)| > θ
```

This trades accuracy for computational savings, appropriate when small weight
fluctuations (thermal noise) should not trigger topology updates.

### 6.4 Sliding Window Algorithms

Rather than tracking instantaneous coherence, we maintain a sliding window of
T frames and compute the average coherence graph:

```
w̄(e, t) = (1/T) Σ_{τ=0}^{T-1} w(e, t-τ)
```

This provides temporal smoothing but introduces latency. The exponential moving
average is a better alternative:

```
w̄(e, t) = α * w(e, t) + (1-α) * w̄(e, t-1)
```

with α ∈ (0, 1) controlling the memory. For RF sensing, α ≈ 0.3 balances
responsiveness with noise rejection.

### 6.5 Batched Updates for Round-Robin TDM

In the TDM (Time Division Multiplexing) protocol, each ESP32 node transmits in
turn. After node v_k transmits, we receive updated CSI for all 15 links incident
to v_k. This suggests a **batched update** model:

```
At time step k (mod 16):
  Update edges: {(v_k, v_j) : j ≠ k}  (15 edges)
  Recompute min-cut if significant changes detected
```

This batched structure can be exploited: the 15 updated edges all share a common
endpoint v_k, constraining where the min-cut can change.

**Lemma**: If v_k is entirely on one side of the current min-cut (say v_k ∈ S),
then changes to edges (v_k, v_j) where v_j ∈ S cannot affect the cut capacity.
Only edges crossing the cut — (v_k, v_j) where v_j ∈ S̄ — matter.

In a balanced bisection of 16 nodes, at most 8 of the 15 updated edges cross
the cut, reducing the effective update size.

### 6.6 Perturbation Theory for Eigenvalue Updates

For spectral methods, rank-1 perturbation theory provides efficient eigenvalue
updates. When a single edge (i, j) changes weight by δ, the Laplacian changes
by:

```
δL = δ * (e_i - e_j)(e_i - e_j)^T
```

which is a rank-1 update. The eigenvalues of the perturbed Laplacian satisfy
the secular equation:

```
1 + δ * Σ_k (v_k[i] - v_k[j])^2 / (λ_k - μ) = 0
```

where μ is the perturbed eigenvalue. For the Fiedler value specifically:

```
λ_2' ≈ λ_2 + δ * (v_2[i] - v_2[j])^2
```

This O(1) update is vastly cheaper than O(n^3) full eigendecomposition and
provides an excellent approximation when |δ| is small relative to the spectral
gap λ_3 - λ_2.

For batched updates (15 edges from one TDM slot), the perturbation has rank at
most 15, and iterative refinement methods (Lanczos, LOBPCG) converge in a few
iterations when warm-started from the previous eigenvectors.

---

## 7. Comparison to Classical RF Sensing

### 7.1 Taxonomy of RF Sensing Approaches

| Approach | Signal | Method | Output | Model |
|----------|--------|--------|--------|-------|
| RSSI Triangulation | Received power | Path loss + trilateration | (x, y) position | Distance estimation |
| RSSI Fingerprinting | Received power | Database matching | Room-level location | Pattern matching |
| CSI Localization | Channel matrix | AoA/ToF estimation | (x, y, z) position | Propagation model |
| CSI Activity Recognition | Channel matrix | ML classification | Activity label | Learned patterns |
| **RF Topological Sensing** | **CSI coherence** | **Graph min-cut** | **Boundary partition** | **Graph structure** |

### 7.2 Fundamental Differences

**Position estimation** (classical approaches) asks: *"Where is the target?"*

It requires:
- A propagation model (path loss exponent, multipath model)
- Calibration (fingerprint database, anchor positions)
- Sufficient geometric diversity (non-degenerate anchor geometry)
- Explicit coordinate system

**Topological sensing** (our approach) asks: *"What has changed in the RF field
structure?"*

It requires:
- A baseline coherence graph (self-calibrating from static measurements)
- Graph algorithms (min-cut, spectral decomposition)
- Sufficient link density for topological resolution

It does NOT require:
- A propagation model
- Knowledge of node positions (only connectivity matters)
- An external coordinate system
- Fingerprint databases

### 7.3 Advantages of Topological Sensing

**1. Model-free operation**

RSSI triangulation requires knowing the path loss exponent n in:

```
RSSI(d) = RSSI(d_0) - 10n * log_10(d/d_0)
```

This exponent varies from 1.6 (free space) to 4+ (cluttered indoor) and changes
with environment, humidity, and furniture rearrangement. Topological sensing
uses only coherence *ratios* relative to baseline, avoiding this model dependency.

**2. Self-calibrating**

The baseline graph G_0 is learned from the static (unoccupied) environment.
When the environment changes (furniture moved), the baseline updates
automatically. There is no need for war-driving or fingerprint collection.

**3. Graceful degradation**

Position estimation fails catastrophically when the geometric model is wrong
(e.g., NLOS bias in RSSI causing meters of error). Topological sensing degrades
gracefully: fewer functional links reduce spatial resolution but do not produce
false localizations.

**4. Privacy-preserving**

Topological sensing reports *that* a boundary exists and *which nodes* it
separates, not *where* a person is standing. This is a qualitative, structural
output that inherently preserves privacy while still enabling applications like
occupancy detection and room segmentation.

**5. Inherent multi-target support**

Position estimation for multiple targets requires data association (which
measurements correspond to which target). Topological sensing naturally handles
multiple targets: each creates a separate coherence depression, and k-way
min-cut or hierarchical decomposition reveals all boundaries simultaneously.

### 7.4 Limitations of Topological Sensing

**1. Coarse spatial resolution**

With 16 nodes, the topological resolution is limited to distinguishing regions
separated by at least one link. Fine-grained positioning (sub-meter accuracy)
is not achievable through topology alone — though it can be augmented with
classical methods.

**2. Ambiguity in cut interpretation**

A minimum cut identifies a boundary but does not directly indicate which side
contains the perturbation source. Additional heuristics (e.g., comparing cut
side volumes, using temporal ordering) are needed.

**3. Sensitivity to graph density**

Sparse graphs may have trivial minimum cuts unrelated to physical perturbations.
The mesh must be sufficiently dense that the "natural" minimum cut (without
perturbation) has high capacity, making perturbation-induced cuts stand out.

### 7.5 Hybrid Approaches

Topological sensing and classical methods are complementary. A practical system
might:

1. Use topological sensing (min-cut) for coarse boundary detection and
   multi-target segmentation.
2. Use CSI-based methods (AoA, ToF, or learned models) within each topological
   region for fine-grained localization.
3. Use the topological boundary to constrain the localization search space,
   reducing computational cost and improving accuracy.

This hierarchical approach mirrors how the human sensory system works: first
detect that something is present (topological change), then resolve its precise
location (focused attention).

---

## 8. Open Research Questions

### 8.1 Optimal Node Placement for Topological Resolution

**Question**: Given a room geometry and n nodes, what placement maximizes
topological resolution — the ability to distinguish different perturbation
locations via distinct min-cut partitions?

This is related to sensor placement optimization but with a graph-theoretic
objective function (e.g., maximize the number of distinct minimum cut partitions
achievable) rather than a geometric one (minimize DOP).

**Conjecture**: Regular polygon placements are suboptimal. The optimal placement
should maximize the Fiedler value of the baseline graph while ensuring that
different perturbation locations yield distinct spectral signatures.

### 8.2 Spectral Fingerprinting of Perturbations

**Question**: Can the Laplacian spectrum λ_1, ..., λ_n serve as a "fingerprint"
for different types of perturbations (standing person vs. walking person vs.
furniture vs. door opening)?

The full spectrum encodes more information than just the Fiedler value. Different
perturbation types may create characteristic spectral signatures:

- A person standing still: primarily affects λ_2 (weakens one cut).
- A person walking: creates a time-varying spectral signature with characteristic
  dynamics.
- A door opening: affects a specific subset of eigenvalues corresponding to edges
  near the door.

### 8.3 Information-Theoretic Limits

**Question**: What is the maximum number of distinguishable perturbation states
for a given mesh topology?

Information theory provides bounds: with n nodes and m = O(n^2) edges, each
edge providing b bits of coherence information, the total information is
O(n^2 * b) bits per frame. The number of distinguishable topological states is
at most 2^{O(n^2 * b)}, but the actual number is constrained by the physical
correlation structure (nearby edges provide redundant information).

### 8.4 Dynamic Min-Cut Under Adversarial Perturbations

**Question**: How robust is min-cut based sensing to adversarial manipulation?

An adversary who knows the node positions could potentially create RF
perturbations that manipulate the min-cut to produce a desired (false) topology.
Understanding the attack surface requires analysis of which edge weight
modifications change the min-cut partition — the "critical edges" of the graph.

Connection to the `adversarial.rs` module in RuvSense: physically impossible
signal patterns (e.g., coherence dropping on a link whose Fresnel zone is
geometrically blocked from the detected perturbation region) may indicate
adversarial manipulation.

### 8.5 Temporal Graph Sequences and Trajectory Reconstruction

**Question**: Can a time series of min-cut partitions {(S(t), S̄(t))} be
inverted to reconstruct a continuous trajectory?

As a person moves through the mesh, the min-cut partition evolves. The sequence
of partitions defines a trajectory in the "partition space" of the graph. Whether
this trajectory can be projected back to physical space (even approximately)
remains open. The key challenge is that different physical positions can produce
the same partition (topological aliasing).

### 8.6 Multi-Resolution Topological Decomposition

**Question**: Can hierarchical min-cut decomposition (Gomory-Hu tree) provide
multi-resolution sensing — coarse room segmentation at the top level, fine-grained
boundary detection at lower levels?

The Gomory-Hu tree naturally provides a hierarchy: the minimum weight edge in the
tree gives the global min-cut (coarsest partition), removing it and finding the
minimum in each subtree gives a 3-way partition, and so on. This hierarchical
decomposition might correspond to spatial resolution levels.

### 8.7 Graph Neural Networks for Learned Topological Features

**Question**: Can GNNs operating on the coherence graph learn richer topological
features than hand-crafted min-cut/spectral methods?

Graph convolutional networks (GCNs) and graph attention networks (GATs) can
learn node embeddings from graph structure. Training a GNN on labeled coherence
graphs (with known perturbation locations) might produce features that outperform
spectral methods, especially for complex multi-person scenarios.

This connects to the `wifi-densepose-nn` crate and the broader neural network
inference pipeline.

### 8.8 Non-Euclidean RF Topology

**Question**: When the RF propagation environment is strongly non-line-of-sight
(e.g., multi-room deployment with walls), the coherence graph may have a
fundamentally non-Euclidean structure. How do graph-theoretic methods perform
when the graph does not embed naturally in R^2?

In multi-room settings, the effective topology might be better modeled as a
graph with a non-trivial genus or as a hyperbolic graph. Spectral methods on
such graphs have different convergence properties, and the Cheeger constant
may relate differently to physical boundaries.

### 8.9 Minimum Cut Stability and Phase Transitions

**Question**: Is there a phase transition in min-cut behavior as a perturbation
grows in strength?

In percolation theory, random graphs exhibit sharp phase transitions in
connectivity. Similarly, as an RF perturbation intensifies (edge weights in
the affected region approach zero), the min-cut may undergo a sudden transition
from a "diffuse" cut (spread across many edges) to a "concentrated" cut (few
edges with very low weight). Understanding this transition would inform threshold
selection for detection algorithms.

---

## 9. Conclusion

This document has established that graph-theoretic methods — particularly minimum
cut algorithms and spectral decomposition — provide a rigorous mathematical
foundation for RF topological sensing. The key contributions are:

1. **Formal framework**: Modeling the ESP32 mesh as a weighted graph G = (V, E, w)
   with CSI coherence as edge weights, and defining perturbation detection as a
   minimum cut problem.

2. **Algorithm selection**: Stoer-Wagner for global min-cut (deterministic,
   efficient for n = 16), Karger for probabilistic analysis of cut stability,
   and Gomory-Hu trees for all-pairs queries.

3. **Spectral characterization**: The Fiedler value as a real-time indicator of
   topological change, with the Cheeger inequality providing theoretical
   guarantees on cut quality.

4. **Dynamic algorithms**: Incremental/decremental strategies, perturbation
   theory for eigenvalue updates, and batched processing aligned with TDM
   scheduling.

5. **Fundamental distinction**: Topological sensing (boundary detection via
   graph structure) is categorically different from position estimation (RSSI,
   CSI localization), offering model-free, self-calibrating, privacy-preserving
   sensing at the cost of coarser spatial resolution.

6. **Open questions**: Nine research directions spanning optimal placement,
   spectral fingerprinting, information-theoretic limits, adversarial robustness,
   trajectory reconstruction, multi-resolution decomposition, GNN integration,
   non-Euclidean topology, and phase transitions.

The practical implementation of these foundations is underway in the
`wifi-densepose-signal` crate (RuvSense modules) and `wifi-densepose-ruvector`
crate (cross-viewpoint fusion), with the `ruvector-mincut` crate providing the
core graph algorithms.

---

## 10. References

### Graph Theory and Algorithms

1. Ford, L.R. and Fulkerson, D.R. (1956). "Maximal Flow through a Network."
   *Canadian Journal of Mathematics*, 8, 399-404.

2. Stoer, M. and Wagner, F. (1997). "A Simple Min-Cut Algorithm." *Journal of
   the ACM*, 44(4), 585-591.

3. Karger, D.R. (1993). "Global Min-cuts in RNC, and Other Ramifications of a
   Simple Min-cut Algorithm." *Proceedings of SODA*, 21-30.

4. Gomory, R.E. and Hu, T.C. (1961). "Multi-terminal Network Flows." *Journal
   of the Society for Industrial and Applied Mathematics*, 9(4), 551-570.

5. Karger, D.R. and Stein, C. (1996). "A New Approach to the Minimum Cut
   Problem." *Journal of the ACM*, 43(4), 601-640.

### Spectral Graph Theory

6. Fiedler, M. (1973). "Algebraic Connectivity of Graphs." *Czechoslovak
   Mathematical Journal*, 23(98), 298-305.

7. Cheeger, J. (1970). "A Lower Bound for the Smallest Eigenvalue of the
   Laplacian." *Problems in Analysis*, Princeton University Press, 195-199.

8. Shi, J. and Malik, J. (2000). "Normalized Cuts and Image Segmentation."
   *IEEE Transactions on Pattern Analysis and Machine Intelligence*, 22(8),
   888-905.

9. Lee, J.R., Oveis Gharan, S., and Trevisan, L. (2014). "Multiway Spectral
   Partitioning and Higher-Order Cheeger Inequalities." *Journal of the ACM*,
   61(6), Article 37.

10. Spielman, D.A. and Teng, S.-H. (2011). "Spectral Sparsification of Graphs."
    *SIAM Journal on Computing*, 40(4), 981-1025.

### RF Sensing and CSI

11. Wang, W., Liu, A.X., Shahzad, M., Ling, K., and Lu, S. (2015).
    "Understanding and Modeling of WiFi Signal Based Human Activity Recognition."
    *Proceedings of MobiCom*, 65-76.

12. Ma, Y., Zhou, G., and Wang, S. (2019). "WiFi Sensing with Channel State
    Information: A Survey." *ACM Computing Surveys*, 52(3), Article 46.

13. Yang, Z., Zhou, Z., and Liu, Y. (2013). "From RSSI to CSI: Indoor
    Localization via Channel Response." *ACM Computing Surveys*, 46(2),
    Article 25.

### Network Flow and Dynamic Graphs

14. Goldberg, A.V. and Rao, S. (1998). "Beyond the Flow Decomposition Barrier."
    *Journal of the ACM*, 45(5), 783-797.

15. Thorup, M. (2007). "Minimum k-way Cuts via Deterministic Greedy Tree
    Packing." *Proceedings of STOC*, 159-166.

16. Goranci, G., Henzinger, M., and Thorup, M. (2018). "Incremental Exact
    Min-Cut in Polylogarithmic Amortized Update Time." *ACM Transactions on
    Algorithms*, 14(2), Article 17.

---

*This research document is part of the RuView project. It provides theoretical
foundations for the RF topological sensing approach implemented in the
wifi-densepose-signal and wifi-densepose-ruvector crates.*
