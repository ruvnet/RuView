# Attention Mechanisms for RF Topological Sensing

## A Comprehensive Survey for WiFi-DensePose / RuView

**Document**: 03-attention-mechanisms-rf-sensing
**Date**: 2026-03-08
**Status**: Research Reference
**Scope**: Attention architectures for graph-based RF sensing where ESP32 nodes
form a dynamic signal topology and minimum cut partitioning detects human
presence, pose, and activity.

---

## Table of Contents

1. [Introduction and Problem Setting](#1-introduction-and-problem-setting)
2. [Graph Attention Networks for RF Sensing Graphs](#2-graph-attention-networks-for-rf-sensing-graphs)
3. [Self-Attention for CSI Sequences](#3-self-attention-for-csi-sequences)
4. [Cross-Attention for Multi-Link Fusion](#4-cross-attention-for-multi-link-fusion)
5. [Attention-Weighted Minimum Cut](#5-attention-weighted-minimum-cut)
6. [Spatial Attention for Node Importance](#6-spatial-attention-for-node-importance)
7. [Antenna-Level Attention](#7-antenna-level-attention)
8. [Efficient Attention for Resource-Constrained Deployment](#8-efficient-attention-for-resource-constrained-deployment)
9. [Unified Architecture](#9-unified-architecture)
10. [References and Further Reading](#10-references-and-further-reading)

---

## 1. Introduction and Problem Setting

### 1.1 RF Topological Sensing Model

RF topological sensing models a physical space as a dynamic signal graph
G = (V, E, W) where:

- **Vertices V**: ESP32 nodes placed in the environment (typically 4-8 nodes)
- **Edges E**: Bidirectional TX-RX links between node pairs
- **Weights W**: Signal coherence metrics derived from Channel State Information (CSI)

A person moving through the space perturbs the RF field, causing coherence
drops along links whose Fresnel zones intersect the person's body. Minimum
cut partitioning of this weighted graph identifies the boundary between
perturbed and unperturbed subgraphs, localizing the person.

```
    RF Topological Sensing — Conceptual Model
    ==========================================

    Physical Space                Signal Graph G = (V, E, W)
    +-----------------------+
    |                       |         N1 ----0.92---- N2
    |  [N1]          [N2]   |        / \              / \
    |       \      /        |      0.31  0.87      0.45  0.91
    |        \ P  /         |      /       \      /       \
    |         \../          |    N4 --0.28-- N5 --0.89-- N3
    |  [N4]...[P]....[N3]  |         \              /
    |         /  \          |          0.93 ------ 0.90
    |        /    \         |
    |  [N5]        [N6]     |    Low weights (0.28, 0.31, 0.45) indicate
    |                       |    links crossing the person P's position.
    +-----------------------+    Mincut separates {N4,N5} from {N1,N2,N3,N6}.
```

### 1.2 Why Attention Mechanisms

Traditional RF sensing uses hand-crafted features: amplitude variance,
phase difference, subcarrier correlation. These have three fundamental
limitations:

1. **Static edge weighting**: Fixed formulas cannot adapt to environment
   changes (furniture moved, temperature drift, multipath evolution).
2. **Uniform link treatment**: All TX-RX pairs contribute equally regardless
   of geometric information content.
3. **No temporal context**: Each CSI frame is processed independently,
   ignoring the sequential structure of human motion.

Attention mechanisms address all three by learning to weight information
sources — subcarriers, time steps, links, and nodes — according to their
relevance for the downstream task.

### 1.3 Notation

| Symbol | Meaning |
|--------|---------|
| N | Number of ESP32 nodes |
| L = N(N-1)/2 | Number of bidirectional links |
| S | Number of OFDM subcarriers (typically 52 or 114) |
| T | Number of time steps in a CSI window |
| H(s,t) in C^S | CSI vector for link l at time t |
| d_k | Attention key/query dimension |
| h | Number of attention heads |

---

## 2. Graph Attention Networks for RF Sensing Graphs

### 2.1 From Static Weights to Learned Attention

In a standard graph formulation, the adjacency matrix A has entries a_ij
representing signal coherence between nodes i and j. Graph Attention Networks
(GATs) replace these fixed weights with learned attention coefficients that
adapt based on the node features.

Given node feature vectors x_i in R^F for each ESP32 node i, GAT computes
attention coefficients:

```
    e_ij = LeakyReLU(a^T [W x_i || W x_j])

    alpha_ij = softmax_j(e_ij) = exp(e_ij) / sum_k(exp(e_ik))
```

where:
- W in R^{F' x F} is a learnable weight matrix
- a in R^{2F'} is a learnable attention vector
- || denotes concatenation
- The softmax normalizes over all neighbors j of node i

The updated node representation becomes:

```
    x_i' = sigma( sum_j alpha_ij W x_j )
```

### 2.2 Node Features from CSI

For RF sensing, node features are not given directly. Each ESP32 node
participates in multiple links, and each link produces CSI streams. We
construct node features by aggregating incoming link information:

```
    x_i = AGG({ f(H_ij(t)) : j in N(i), t in [T] })
```

where f is a feature extractor (e.g., amplitude statistics, phase slope)
and AGG is mean or max pooling over neighbors and time.

```
    Node Feature Construction
    =========================

    Links to Node N1:          Feature Extraction:       Node Feature:

    N2->N1: H_21(1..T)  --->  f(H_21) = [amp_var,   \
    N3->N1: H_31(1..T)  --->  f(H_31) =  phase_slope, > AGG --> x_1 in R^F
    N4->N1: H_41(1..T)  --->  f(H_41) =  corr, ...]  /
    N5->N1: H_51(1..T)  --->  f(H_51)               /
```

### 2.3 Multi-Head Attention for RF Graphs

Single-head attention captures one notion of relevance. Multi-head attention
runs h independent attention computations and concatenates or averages:

```
    x_i' = ||_{k=1}^{h} sigma( sum_j alpha_ij^(k) W^(k) x_j )
```

For RF sensing, different heads can specialize in different phenomena:

| Head | Learned Specialization |
|------|----------------------|
| Head 1 | Line-of-sight path quality |
| Head 2 | Multipath richness (scattering) |
| Head 3 | Temporal stability (static vs dynamic) |
| Head 4 | Frequency selectivity (subcarrier variance) |

### 2.4 Edge-Featured GAT for RF Links

Standard GAT only uses node features to compute attention. In RF sensing,
edges carry rich information (the CSI itself). Edge-featured GAT
incorporates edge attributes e_ij directly:

```
    e_ij = LeakyReLU(a^T [W_n x_i || W_n x_j || W_e e_ij])
```

where e_ij in R^E contains link-level features:
- Mean amplitude across subcarriers
- Phase coherence (circular variance)
- Doppler shift estimate
- Signal-to-noise ratio
- Fresnel zone geometry (distance, angle)

```
    Edge-Featured GAT — RF Sensing
    ================================

         x_i                    x_j
          |                      |
          v                      v
       [W_n x_i]            [W_n x_j]
          |                      |
          +--- CONCAT ---+--- CONCAT ---+
                         |              |
                      [W_e e_ij]        |
                         |              |
                    [ a^T [...] ]       |
                         |              |
                    LeakyReLU           |
                         |              |
                    alpha_ij            |
                         |              |
                    alpha_ij * W x_j ---+---> contribution to x_i'
```

### 2.5 GATv2: Dynamic Attention

The original GAT has a "static attention" limitation: the ranking of
attention coefficients is fixed for a given query node regardless of the
key. GATv2 fixes this by applying the nonlinearity after concatenation
but before the dot product:

```
    e_ij = a^T LeakyReLU(W [x_i || x_j])
```

This is strictly more expressive and important for RF sensing where the
same node should attend differently depending on which neighbor it is
evaluating — a dynamic property essential for tracking moving targets.

---

## 3. Self-Attention for CSI Sequences

### 3.1 Temporal Structure of CSI

CSI measurements arrive as time series at 100-1000 Hz. Human motion creates
characteristic temporal patterns: periodic breathing modulates amplitude
at 0.2-0.5 Hz, walking creates 1-2 Hz Doppler signatures, and gestures
produce transient bursts. Self-attention over CSI sequences identifies
which time steps carry the most information for graph weight updates.

### 3.2 Transformer Self-Attention on CSI

Given a CSI sequence H = [h_1, h_2, ..., h_T] where h_t in R^S is the
CSI vector at time t, self-attention computes:

```
    Q = H W_Q,    K = H W_K,    V = H W_V

    Attention(Q, K, V) = softmax(Q K^T / sqrt(d_k)) V
```

The attention matrix A in R^{T x T} has entry A_st representing how much
time step t attends to time step s. This captures:

- **Periodic structure**: Breathing cycles create diagonal band patterns
- **Motion onset**: Sudden movements create high attention to transition frames
- **Static periods**: Uniformly low attention during no-activity intervals

```
    Self-Attention on CSI Time Series
    ==================================

    Input: T time steps of S-dimensional CSI vectors

    h_1  h_2  h_3  ...  h_T        Time steps
     |    |    |         |
     v    v    v         v
    [  Linear Projections Q, K, V  ]
     |    |    |         |
     v    v    v         v
    [    Scaled Dot-Product Attention    ]
     |    |    |         |
     v    v    v         v
    z_1  z_2  z_3  ...  z_T        Contextualized representations

    Attention Pattern (breathing example):

         t1  t2  t3  t4  t5  t6  t7  t8
    t1 [ .9  .3  .1  .0  .7  .2  .1  .0 ]   <-- attends to t1, t5
    t2 [ .3  .9  .3  .1  .2  .7  .3  .1 ]       (same phase of
    t3 [ .1  .3  .9  .3  .1  .2  .7  .3 ]        breathing cycle)
    t4 [ .0  .1  .3  .9  .0  .1  .3  .8 ]
    ...
    Diagonal bands indicate periodic self-similarity.
```

### 3.3 Positional Encoding for CSI

CSI time series require positional encoding to preserve temporal ordering.
Sinusoidal positional encodings work well, but learnable encodings tuned
to the CSI sampling rate can capture hardware-specific timing patterns:

```
    PE(t, 2i)   = sin(t / 10000^{2i/d})
    PE(t, 2i+1) = cos(t / 10000^{2i/d})
```

For 100 Hz CSI with T=128 window, the positional encoding must resolve
10 ms differences. An alternative is relative positional encoding (RPE)
which encodes the time difference (t - s) rather than absolute position,
making the model invariant to window start time.

### 3.4 Causal vs. Bidirectional Attention

For real-time sensing, causal (masked) attention is necessary — time step t
can only attend to steps 1..t:

```
    Mask_st = { 0    if s <= t
              { -inf  if s > t

    A = softmax((Q K^T + Mask) / sqrt(d_k))
```

For offline analysis (e.g., training data labeling), bidirectional attention
provides richer context by allowing each step to attend to the full window.

### 3.5 Temporal Attention Pooling for Edge Weights

The key application is collapsing the time dimension into a single edge
weight for graph construction. Attention-weighted temporal pooling:

```
    w_ij = sum_t alpha_t * g(z_t^{ij})

    where alpha_t = softmax(v^T tanh(W_a z_t^{ij}))
```

Here z_t^{ij} is the contextualized CSI representation for link (i,j)
at time t, and g maps to a scalar coherence score. The attention weights
alpha_t learn to focus on the most informative moments — for example,
the peak of a Doppler burst during a gesture.

---

## 4. Cross-Attention for Multi-Link Fusion

### 4.1 Inter-Link Dependencies

In a multistatic RF sensing setup, links are not independent. A person
walking between nodes N1 and N3 simultaneously affects links (N1,N3),
(N2,N3), and (N1,N4) to varying degrees. Cross-attention captures these
correlations by allowing each link's representation to attend to all
other links.

### 4.2 Formulation

Let Z^{ij} in R^{T x d} be the temporal CSI embedding for link (i,j)
after self-attention. Cross-attention between link (i,j) and all other
links:

```
    Q = Z^{ij} W_Q          (query from target link)
    K = [Z^{kl}] W_K        (keys from all links, stacked)
    V = [Z^{kl}] W_V        (values from all links, stacked)

    CrossAttn(ij) = softmax(Q K^T / sqrt(d_k)) V
```

### 4.3 Architecture

```
    Cross-Attention for Multi-Link Fusion
    ======================================

    Link (1,2)    Link (1,3)    Link (2,3)    Link (2,4)   ...
       |              |              |              |
    [Self-Attn]   [Self-Attn]   [Self-Attn]   [Self-Attn]
       |              |              |              |
       v              v              v              v
      Z^12          Z^13          Z^23          Z^24
       |              |              |              |
       +------+-------+------+------+------+------+
              |              |              |
         [Cross-Attn]  [Cross-Attn]  [Cross-Attn]   ...
              |              |              |
              v              v              v
            C^12           C^13           C^23
              |              |              |
         [Edge Score]  [Edge Score]  [Edge Score]
              |              |              |
              v              v              v
            w_12           w_13           w_23

    Each link attends to all other links to capture
    spatial correlations from shared human targets.
```

### 4.4 Geometric Bias in Cross-Attention

Links that are physically close or share a node should have baseline
higher attention. We introduce a geometric bias G_bias:

```
    A = softmax((Q K^T + G_bias) / sqrt(d_k)) V
```

where G_bias_mn encodes the geometric relationship between link m and
link n:

```
    G_bias_mn = -beta * d_Fresnel(m, n) + gamma * shared_node(m, n)
```

- d_Fresnel: distance between Fresnel zone centers
- shared_node: 1 if links share an endpoint, 0 otherwise
- beta, gamma: learnable parameters

This is the concept implemented in RuVector's `CrossViewpointAttention`
with `GeometricBias` — the attention mechanism is biased toward
geometrically meaningful link combinations while still allowing the model
to discover non-obvious correlations.

### 4.5 Hierarchical Cross-Attention

For N nodes with L = N(N-1)/2 links, full cross-attention is O(L^2).
A hierarchical approach reduces this:

1. **Node-local fusion**: Each node aggregates its incident links (O(N) links per node)
2. **Node-to-node attention**: Cross-attention between node representations (O(N^2))
3. **Back-projection**: Node attention weights propagate back to link scores

```
    Level 1 (Link -> Node):    Links incident to Ni --> aggregate --> n_i
    Level 2 (Node -> Node):    {n_1, ..., n_N} --> Cross-Attn --> {n_1', ..., n_N'}
    Level 3 (Node -> Link):    n_i', n_j' --> project --> w_ij
```

This reduces complexity from O(L^2) = O(N^4) to O(N^2), critical for
dense meshes with 6-8 nodes (15-28 links).

---

## 5. Attention-Weighted Minimum Cut

### 5.1 Classical Minimum Cut

Given graph G = (V, E, W), the minimum s-t cut partitions V into S and T
such that s in S, t in T, and the cut weight is minimized:

```
    mincut(S, T) = sum_{(i,j): i in S, j in T} w_ij
```

For RF sensing, we seek the normalized cut (Ncut) which balances partition
sizes:

```
    Ncut(S, T) = cut(S,T)/assoc(S,V) + cut(S,T)/assoc(T,V)
```

where assoc(S,V) = sum of all edge weights incident to S.

### 5.2 Differentiable Relaxation

The discrete mincut problem is NP-hard. The spectral relaxation uses the
graph Laplacian L = D - W (D is the degree matrix):

```
    min_y  y^T L y / y^T D y     subject to y in {-1, +1}^N

    Relaxed: min_y  y^T L y / y^T D y,  y in R^N
```

The solution is the Fiedler vector — the eigenvector of the smallest
nonzero eigenvalue of the normalized Laplacian.

### 5.3 Attention as Edge Scoring for MinCut

The key insight: replace fixed edge weights with attention-computed scores
that are differentiable end-to-end. Given raw CSI features, attention
produces edge weights, which feed into a differentiable mincut layer:

```
    Attention-Weighted Differentiable MinCut Pipeline
    ==================================================

    Raw CSI Frames                    Differentiable MinCut
    per link (i,j)

    H_12 --+                          W = {w_ij}
    H_13 --+--> [Attention    ] -->      |
    H_23 --+    [  Modules    ]       [Build Laplacian L = D - W]
    H_24 --+    [Sec 2,3,4,7 ]          |
    H_34 --+                          [Soft assignment S = softmax(X)]
    ...  --+                             |
                                      [MinCut loss: Tr(S^T L S) / Tr(S^T D S)]
                                         |
                                      [Backprop through attention weights]
```

### 5.4 Soft MinCut Assignment

Instead of hard cluster assignments, use a soft assignment matrix
S in R^{N x K} where K is the number of clusters:

```
    S = softmax(MLP(X))     where X = GNN(node_features, W)

    L_cut = -Tr(S^T A S) / Tr(S^T D S)     (MinCut loss)
    L_orth = || S^T S / ||S^T S||_F - I/sqrt(K) ||_F   (Orthogonality)

    L_total = L_cut + lambda * L_orth
```

The attention-computed edge weights W flow into A (adjacency), D (degree),
and through the GNN into S. The entire pipeline is differentiable, allowing
the attention mechanism to learn edge weights that produce meaningful cuts.

### 5.5 Mincut Attention Loss

The training signal for attention comes from two sources:

1. **Supervised**: Ground-truth person location determines which links
   should have low weights (those crossing the person's body).

2. **Self-supervised**: The mincut objective itself provides a training
   signal — attention weights that produce cleaner cuts (lower Ncut value
   with balanced partitions) are reinforced.

```
    L_attention = L_supervised + alpha * L_mincut + beta * L_regularization

    L_supervised   = BCE(w_ij, y_ij)           (y_ij = 1 if link unobstructed)
    L_mincut       = Ncut(S*, T*)              (quality of resulting partition)
    L_regularization = sum_ij |alpha_ij| * H(alpha_ij)  (attention entropy)
```

The entropy regularization H(alpha) prevents attention collapse (all weight
on one link) or uniform attention (no discrimination).

---

## 6. Spatial Attention for Node Importance

### 6.1 Motivation

Not all ESP32 nodes contribute equally. A node in a corner has fewer
intersecting Fresnel zones than a central node. A node with hardware
degradation may produce noisy CSI. Spatial attention learns to weight
nodes by their information contribution.

### 6.2 Node Importance Scoring

For each node i, compute an importance score:

```
    s_i = sigma(w^T [x_i || g_i || q_i])
```

where:
- x_i: node feature vector (from CSI aggregation)
- g_i: geometric feature (position, angle coverage, Fresnel density)
- q_i: quality feature (SNR, packet loss rate, timing jitter)

The importance score gates the node's contribution:

```
    x_i_gated = s_i * x_i
```

### 6.3 Squeeze-and-Excitation for Node Graphs

Adapted from channel attention in CNNs, Squeeze-and-Excitation (SE)
for node graphs:

```
    1. Squeeze:   z = (1/N) sum_i x_i          (global node pooling)
    2. Excite:    s = sigma(W_2 ReLU(W_1 z))   (per-node importance)
    3. Scale:     x_i' = s_i * x_i             (reweight nodes)
```

```
    Squeeze-and-Excitation for ESP32 Node Graph
    =============================================

    Node features:  x_1   x_2   x_3   x_4   x_5   x_6
                     |     |     |     |     |     |
                     +--+--+--+--+--+--+--+--+--+--+
                        |
                  [Global Pool z]
                        |
                  [FC -> ReLU -> FC -> Sigmoid]
                        |
                  s_1  s_2  s_3  s_4  s_5  s_6
                   |    |    |    |    |    |
                   *    *    *    *    *    *
                   |    |    |    |    |    |
                  x_1' x_2' x_3' x_4' x_5' x_6'

    Example: Node 3 (occluded corner) gets s_3 = 0.2
             Node 5 (central, clear LoS) gets s_5 = 0.9
```

### 6.4 Fisher Information-Based Attention

From estimation theory, the Fisher Information quantifies how much a
measurement contributes to parameter estimation. For node i observing
target at position theta:

```
    FI_i(theta) = E[ (d/d_theta log p(H_i | theta))^2 ]
```

Nodes with higher Fisher Information provide more localization accuracy.
This can be computed analytically for simple signal models or approximated
via the Cramer-Rao bound. The Geometric Diversity Index from RuVector's
`geometry.rs` module implements a related concept.

### 6.5 Dynamic Node Dropout

Spatial attention naturally enables dynamic node dropout — nodes with
importance below a threshold are excluded from graph construction:

```
    V_active = { i in V : s_i > tau }
    E_active = { (i,j) in E : i in V_active AND j in V_active }
```

This provides robustness to node failures and reduces computation when
some nodes are uninformative (e.g., all links from a node are in deep
shadow).

---

## 7. Antenna-Level Attention

### 7.1 Subcarrier-Level CSI Features

Each CSI measurement contains S subcarriers (52 for 20 MHz, 114 for 40 MHz
802.11n). Not all subcarriers are equally informative:

- Subcarriers near null frequencies carry noise
- Subcarriers in frequency-selective fading notches are unreliable
- Subcarriers near the band edges have lower SNR
- Different subcarriers have different sensitivity to motion at different
  distances (wavelength-dependent Fresnel zone widths)

### 7.2 Antenna Attention Mechanism

RuVector's `apply_antenna_attention` concept applies attention at the
subcarrier level before any graph construction. For a CSI vector
h in C^S:

```
    h_real = [Re(h) || Im(h)]                 in R^{2S}
    a = softmax(W_2 ReLU(W_1 h_real + b_1) + b_2)   in R^S
    h_attended = a odot h                      in C^S
```

where odot is element-wise multiplication (the attention weights are
real-valued but applied to complex CSI).

```
    Antenna-Level Attention (Before Graph Construction)
    ====================================================

    Raw CSI:     h = [h_1, h_2, ..., h_S]     (S complex subcarriers)
                      |    |          |
                 [Re/Im decompose + concat]
                      |
                 [FC -> ReLU -> FC -> Softmax]
                      |
    Attention:   a = [a_1, a_2, ..., a_S]     (S real weights, sum = 1)
                      |    |          |
                      *    *          *        (element-wise)
                      |    |          |
    Attended:    h' = [a_1*h_1, a_2*h_2, ..., a_S*h_S]
                      |
                 [Feature extraction]
                      |
                 [Graph edge weight w_ij]

    Subcarrier attention map (example, 52 subcarriers):

    Attention  ^
    weight     |       **                              **
               |      *  *          *****             *  *
               |     *    *        *     *           *    *
               |    *      *      *       *         *      *
               |***        ******         *********        ***
               +------------------------------------------------->
                    10        20        30        40        50
                                  Subcarrier index

    Peaks at subcarriers most affected by target motion.
    Nulls at subcarriers dominated by static multipath.
```

### 7.3 Multi-Antenna Attention

With multiple antennas (MIMO), attention operates across both antenna
and subcarrier dimensions. For an A-antenna, S-subcarrier system,
the CSI tensor H in C^{A x S}:

```
    Antenna attention:     a_ant in R^A     (which antennas matter)
    Subcarrier attention:  a_sub in R^S     (which frequencies matter)

    Joint attention:       A_joint = a_ant * a_sub^T   in R^{A x S}
    Attended CSI:          H' = A_joint odot H          in C^{A x S}
```

This factored attention (rank-1) is parameter-efficient. A full attention
matrix A in R^{A*S x A*S} is more expressive but requires A*S times more
computation.

### 7.4 Temporal-Spectral Attention

Combining subcarrier attention with temporal attention creates a 2D
attention map over the time-frequency representation of CSI:

```
    Time-Frequency Attention Map
    =============================

    Subcarrier ^
    (freq)     |  .  .  .  .  .  .  .  .  .  .  .  .
         52    |  .  .  .  .  .  .  .  .  .  .  .  .
               |  .  .  .  .  #  #  .  .  .  .  .  .
         40    |  .  .  .  #  #  #  #  .  .  .  .  .
               |  .  .  .  #  #  #  #  .  .  .  .  .
         30    |  .  .  #  #  #  #  #  #  .  .  .  .
               |  .  .  .  #  #  #  #  .  .  .  .  .
         20    |  .  .  .  .  #  #  .  .  .  .  .  .
               |  .  .  .  .  .  .  .  .  .  .  .  .
         10    |  .  .  .  .  .  .  .  .  .  .  .  .
               |  .  .  .  .  .  .  .  .  .  .  .  .
          1    |  .  .  .  .  .  .  .  .  .  .  .  .
               +---+---+---+---+---+---+---+---+---+--->
                   20  40  60  80 100 120 140 160 180
                              Time step

    '#' = high attention (motion event at t=60-120, f=20-45)
    '.' = low attention (static or noise)
```

This is essentially a learned spectrogram filter that isolates the
time-frequency regions containing target motion signatures.

### 7.5 Connection to Sparse Subcarrier Selection

RuVector's `subcarrier_selection.rs` uses mincut-based selection to reduce
114 subcarriers to 56 for efficiency. Antenna-level attention provides a
soft version of this: instead of hard selection, it continuously weights
subcarriers. The hard selection can be derived from attention weights:

```
    selected_subcarriers = top_k(a, k=56)
```

Or using Gumbel-Softmax for differentiable discrete selection during
training.

---

## 8. Efficient Attention for Resource-Constrained Deployment

### 8.1 The Quadratic Bottleneck

Standard self-attention has O(T^2) time and memory complexity. For
CSI sequences with T=512 at 100 Hz (5.12 seconds), the attention matrix
has 262,144 entries per head. On ESP32 with 520 KB SRAM, this is
prohibitive.

### 8.2 Linear Attention

Linear attention replaces the softmax with kernel decomposition:

```
    Standard:  Attn(Q,K,V) = softmax(QK^T/sqrt(d)) V     O(T^2 d)

    Linear:    Attn(Q,K,V) = phi(Q) (phi(K)^T V)          O(T d^2)
```

where phi is a feature map (e.g., elu(x) + 1, or random Fourier features).
The key insight is associativity: computing (K^T V) first yields a
d x d matrix, then multiplying by Q is O(T d^2), which is linear in T
when d << T.

For CSI with d_k = 64 and T = 512, this reduces computation by 8x.

```
    Standard vs Linear Attention
    =============================

    Standard (O(T^2 d)):           Linear (O(T d^2)):

    Q [T x d]                      phi(Q) [T x d']
       \                              \
        * K^T [d x T]                  * (phi(K)^T V) [d' x d]
         \                              \
      [T x T] (large!)              [T x d] (small!)
           \                            |
            * V [T x d]                 | (done)
             \                          |
          [T x d]                    [T x d]
```

### 8.3 Sparse Attention Patterns

Instead of full T x T attention, use structured sparsity:

**Local Window Attention**: Each position attends to a window of w neighbors:

```
    A_st = { QK^T/sqrt(d)  if |s - t| <= w/2
           { -inf           otherwise
```

Complexity: O(T * w) with w << T. For CSI at 100 Hz, w = 32 covers
320 ms — sufficient for most motion events.

**Dilated Attention**: Attend to positions at exponentially increasing gaps:

```
    Attend to: t-1, t-2, t-4, t-8, t-16, t-32, ...
```

This provides O(T log T) complexity while maintaining long-range context.

**Strided Attention**: Combine local and strided patterns (as in Longformer):

```
    Attention Pattern (T=16, window=3, stride=4):

         1  2  3  4  5  6  7  8  9 10 11 12 13 14 15 16
    1  [ x  x  .  x  .  .  .  .  x  .  .  .  x  .  .  . ]
    2  [ x  x  x  .  x  .  .  .  .  x  .  .  .  x  .  . ]
    3  [ .  x  x  x  .  x  .  .  .  .  x  .  .  .  x  . ]
    4  [ x  .  x  x  x  .  x  .  .  .  .  x  .  .  .  x ]
    ...
    x = attends, . = masked
    Local window (3) + every 4th position for global context
```

### 8.4 Locality-Sensitive Hashing (LSH) Attention

LSH attention (from Reformer) groups similar queries and keys into buckets,
computing attention only within buckets:

```
    1. Hash Q and K into b buckets using LSH
    2. Sort by bucket assignment
    3. Compute attention within each bucket

    Complexity: O(T * T/b) per bucket, O(T * T/b * b) total
    With b = sqrt(T): O(T * sqrt(T))
```

For RF sensing, LSH naturally groups similar CSI patterns — time steps
with similar signal characteristics attend to each other, which is
physically meaningful (similar body poses produce similar CSI).

### 8.5 Quantized Attention for ESP32

For edge deployment on ESP32:

```
    INT8 Quantized Attention:

    Q_int8 = clamp(round(Q / scale_Q), -128, 127)
    K_int8 = clamp(round(K / scale_K), -128, 127)

    Scores_int16 = Q_int8 * K_int8^T       (INT8 matmul -> INT16)
    A = softmax(dequantize(Scores_int16))   (back to FP32 for softmax)

    Memory: Q,K in INT8 uses 1/4 the SRAM of FP32
    Compute: INT8 matmul is 2-4x faster on ESP32-S3
```

### 8.6 Attention-Free Alternatives

For the most constrained scenarios, attention-free architectures that
approximate attention behavior:

**Gated Linear Units (GLU)**:
```
    y = (X W_1 + b_1) odot sigma(X W_2 + b_2)
```

**State Space Models (S4/Mamba)**:
```
    x_t = A x_{t-1} + B u_t
    y_t = C x_t + D u_t

    With structured A matrix: O(T log T) via FFT
```

S4 models are particularly promising for CSI sequences because:
- O(T) inference (vs O(T^2) for attention)
- Natural handling of continuous-time signals
- Long-range dependency capture through structured state matrices
- Efficient on sequential hardware (no parallel attention needed)

### 8.7 Deployment Decision Matrix

```
    +--------------------+--------+---------+--------+----------+
    | Method             | Memory | Compute | Range  | Platform |
    +--------------------+--------+---------+--------+----------+
    | Full Attention     | O(T^2) | O(T^2d) | Global | Server   |
    | Linear Attention   | O(Td)  | O(Td^2) | Global | Edge GPU |
    | Window Attention   | O(Tw)  | O(Twd)  | Local  | RPi/Jetson|
    | Dilated Attention  | O(TlgT)| O(TlgTd)| Global | RPi      |
    | LSH Attention      | O(TsqT)| O(TsqTd)| Global | Edge GPU |
    | INT8 Quantized     | O(T^2) | O(T^2d) | Global | ESP32-S3 |
    | GLU (no attention) | O(Td)  | O(Td)   | Local  | ESP32    |
    | S4/Mamba           | O(d^2) | O(Td)   | Global | ESP32    |
    +--------------------+--------+---------+--------+----------+

    T = sequence length, d = model dimension, w = window size
```

---

## 9. Unified Architecture

### 9.1 Full Pipeline

Combining all attention mechanisms into a unified RF sensing pipeline:

```
    Unified Attention Architecture for RF Topological Sensing
    ==========================================================

    LAYER 0: RAW CSI ACQUISITION
    +-----------------------------------------------------------+
    |  ESP32 Node i <---> ESP32 Node j                          |
    |  H_ij in C^{A x S x T}  (antennas x subcarriers x time)  |
    +-----------------------------------------------------------+
                              |
                              v
    LAYER 1: ANTENNA-LEVEL ATTENTION (Section 7)
    +-----------------------------------------------------------+
    |  Per-link subcarrier weighting                             |
    |  a_sub = SoftAttn(H_ij) in R^S                            |
    |  H_ij' = a_sub odot H_ij                                  |
    |  Reduces noise, emphasizes motion-sensitive subcarriers    |
    +-----------------------------------------------------------+
                              |
                              v
    LAYER 2: TEMPORAL SELF-ATTENTION (Section 3)
    +-----------------------------------------------------------+
    |  Per-link temporal context                                 |
    |  Z_ij = SelfAttn(H_ij'[t=1..T])                          |
    |  Captures breathing, gait, gesture patterns                |
    |  Uses efficient attention (Section 8) for long sequences   |
    +-----------------------------------------------------------+
                              |
                              v
    LAYER 3: CROSS-LINK ATTENTION (Section 4)
    +-----------------------------------------------------------+
    |  Inter-link dependency modeling                            |
    |  C_ij = CrossAttn(Z_ij, {Z_kl : all links})              |
    |  With geometric bias G_bias from node positions            |
    |  Captures multi-link correlations from shared targets      |
    +-----------------------------------------------------------+
                              |
                              v
    LAYER 4: EDGE WEIGHT COMPUTATION
    +-----------------------------------------------------------+
    |  w_ij = MLP(TemporalPool(C_ij))                           |
    |  Temporal pooling with attention (Section 3.5)             |
    |  Produces scalar edge weight per link                      |
    +-----------------------------------------------------------+
                              |
                              v
    LAYER 5: GRAPH ATTENTION NETWORK (Section 2)
    +-----------------------------------------------------------+
    |  Multi-head GAT with edge features                        |
    |  x_i' = GAT(x_i, {x_j, w_ij, e_ij})                     |
    |  Refines node representations using graph structure        |
    +-----------------------------------------------------------+
                              |
                              v
    LAYER 6: SPATIAL NODE ATTENTION (Section 6)
    +-----------------------------------------------------------+
    |  Node importance weighting                                 |
    |  s_i = SE_Block(x_i')                                     |
    |  Suppresses noisy or uninformative nodes                   |
    +-----------------------------------------------------------+
                              |
                              v
    LAYER 7: DIFFERENTIABLE MINCUT (Section 5)
    +-----------------------------------------------------------+
    |  Soft cluster assignment with attention-weighted edges     |
    |  S = softmax(MLP(x'))                                     |
    |  L = L_cut + L_orth + L_supervised                        |
    |  Partitions graph at human body boundaries                 |
    +-----------------------------------------------------------+
                              |
                              v
    OUTPUT: Person detection, localization, pose estimation
```

### 9.2 Training Strategy

**Stage 1: Pretrain antenna attention** (Section 7) on single-link CSI
with signal quality labels. This bootstraps meaningful subcarrier
weighting before full pipeline training.

**Stage 2: Train temporal + cross-link attention** (Sections 3-4) with
link-level activity labels. The model learns to identify active links.

**Stage 3: End-to-end fine-tuning** with mincut loss (Section 5) and
person location supervision. All attention mechanisms adapt jointly.

**Stage 4: Distillation for edge deployment** — train efficient variants
(Section 8) to match the full model's attention patterns using KL
divergence between attention distributions.

### 9.3 Computational Budget

For a 6-node mesh (15 links, 52 subcarriers, T=128 time steps):

```
    Component              | FLOPs/frame   | Parameters | Memory
    -----------------------+---------------+------------+---------
    Antenna attention (x15)| 15 * 5K       | 5K         | 15 KB
    Temporal self-attn     | 15 * 1M       | 50K        | 200 KB
    Cross-link attention   | 15^2 * 100K   | 100K       | 500 KB
    GAT (2 layers)         | 6 * 50K       | 30K        | 50 KB
    Spatial attention      | 6 * 1K        | 2K         | 5 KB
    MinCut MLP             | 6 * 10K       | 10K        | 10 KB
    -----------------------+---------------+------------+---------
    Total                  | ~40M          | ~200K      | ~800 KB
```

This fits within a Raspberry Pi 4 (1 GB RAM, 4-core ARM Cortex-A72) for
real-time inference at 10 Hz. For ESP32 deployment, the efficient variants
from Section 8 reduce this by 10-50x.

### 9.4 Relation to RuView Codebase

The unified architecture maps directly to existing RuView modules:

| Architecture Layer | RuView Module | File |
|---|---|---|
| Antenna Attention | ruvector-attn-mincut | `model.rs` (apply_antenna_attention) |
| Temporal Self-Attention | ruvsense | `gesture.rs`, `intention.rs` |
| Cross-Link Attention | ruvector viewpoint | `attention.rs` (CrossViewpointAttention) |
| Geometric Bias | ruvector viewpoint | `geometry.rs` (GeometricDiversityIndex) |
| Edge Weight Computation | ruvsense | `coherence.rs`, `coherence_gate.rs` |
| Graph Attention | ruvector-mincut | `metrics.rs` (DynamicPersonMatcher) |
| Spatial Node Attention | ruvsense | `multistatic.rs` (attention-weighted fusion) |
| Differentiable MinCut | ruvector-mincut | core mincut algorithm |

---

## 10. References and Further Reading

### Foundational Attention Papers

1. Vaswani et al., "Attention Is All You Need," NeurIPS 2017.
   - Original transformer self-attention mechanism.

2. Velickovic et al., "Graph Attention Networks," ICLR 2018.
   - GAT: attention-based message passing on graphs.

3. Brody et al., "How Attentive are Graph Attention Networks?" ICLR 2022.
   - GATv2: dynamic attention fixing GAT's static limitation.

### Efficient Attention

4. Katharopoulos et al., "Transformers are RNNs: Fast Autoregressive
   Transformers with Linear Attention," ICML 2020.
   - Linear attention via kernel feature maps.

5. Kitaev et al., "Reformer: The Efficient Transformer," ICLR 2020.
   - LSH attention for subquadratic complexity.

6. Beltagy et al., "Longformer: The Long-Document Transformer," 2020.
   - Windowed + global attention patterns.

7. Gu et al., "Efficiently Modeling Long Sequences with Structured State
   Spaces (S4)," ICLR 2022.
   - State space models as attention alternatives.

8. Gu and Dao, "Mamba: Linear-Time Sequence Modeling with Selective State
   Spaces," 2023.
   - Selective SSM with input-dependent gating.

### WiFi Sensing

9. Wang et al., "Wi-Pose: WiFi-based Multi-Person Pose Estimation," 2021.
   - WiFi CSI for human pose estimation.

10. Yang et al., "MM-Fi: Multi-Modal Non-Intrusive 4D Human Dataset," 2024.
    - Large-scale WiFi sensing dataset with multi-modal ground truth.

11. Wang et al., "Person-in-WiFi: Fine-Grained Person Perception Using
    WiFi," ICCV 2019.
    - Dense body surface estimation from WiFi signals.

### Graph Partitioning

12. Bianchi et al., "Spectral Clustering with Graph Neural Networks for
    Graph Pooling," ICML 2020.
    - Differentiable mincut pooling with GNNs.

13. Stoer and Wagner, "A Simple Min-Cut Algorithm," JACM 1997.
    - Classical efficient mincut algorithm.

### RF Sensing Theory

14. Adib and Katabi, "See Through Walls with WiFi!" SIGCOMM 2013.
    - Foundational work on WiFi-based sensing.

15. Wang et al., "Placement Matters: Understanding the Effects of Device
    Placement for WiFi Sensing," 2022.
    - Fresnel zone analysis for optimal node placement.

---

*End of document. This research reference supports the attention mechanism
design choices in the RuView/WiFi-DensePose RF topological sensing system.*
