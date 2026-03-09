# Transformer Architectures for RF Topological Graph Sensing

**Research Document 04** | March 2026
**Context**: RuView / wifi-densepose — 16-node ESP32 mesh, CSI coherence-weighted graphs, mincut-based boundary detection, real-time inference requirements.

---

## Abstract

This document surveys transformer architectures applicable to RF topological graph sensing, where a mesh of 16 ESP32 nodes forms a dynamic graph with edges weighted by Channel State Information (CSI) coherence. The primary inference task is mincut prediction — identifying physical boundaries (walls, doors, human bodies) that partition the radio field. We examine graph transformers, temporal graph networks, vision transformers applied to RF spectrograms, transformer-based mincut prediction, positional encoding strategies for RF graphs, foundation model pre-training, and efficient edge deployment. The goal is to identify architectures that can replace or augment combinatorial mincut solvers with learned models capable of real-time inference on resource-constrained hardware.

---

## Table of Contents

1. [Graph Transformers](#1-graph-transformers)
2. [Temporal Graph Transformers](#2-temporal-graph-transformers)
3. [ViT for RF Spectrograms](#3-vit-for-rf-spectrograms)
4. [Transformer-Based Mincut Prediction](#4-transformer-based-mincut-prediction)
5. [Positional Encoding for RF Graphs](#5-positional-encoding-for-rf-graphs)
6. [Foundation Models for RF](#6-foundation-models-for-rf)
7. [Efficient Edge Deployment](#7-efficient-edge-deployment)
8. [Synthesis and Recommendations](#8-synthesis-and-recommendations)

---

## 1. Graph Transformers

### 1.1 The Structural Gap Between Sequences and Graphs

Standard transformers operate on sequences where positional encoding captures order. Graphs have no canonical ordering — nodes are permutation-invariant, and structure is encoded in adjacency rather than position. This creates a fundamental tension: the self-attention mechanism in vanilla transformers treats all token pairs equally, ignoring the graph topology that carries critical information in RF sensing.

For RF topological sensing, graph structure IS the signal. An edge between ESP32 nodes 3 and 7 weighted by CSI coherence of 0.92 means the radio path between them is unobstructed. A weight of 0.31 suggests an intervening boundary. The transformer must respect this structure, not flatten it away.

### 1.2 Graphormer

Graphormer (Ying et al., NeurIPS 2021) introduced three structural encodings that inject graph topology into the transformer:

**Centrality Encoding.** Each node receives a learnable embedding based on its in-degree and out-degree. For an RF mesh, this captures how many strong coherence links a node maintains. Corner nodes in a room typically have lower effective degree (fewer high-coherence links) than central nodes.

```
h_i^(0) = x_i + z_deg+(v_i) + z_deg-(v_i)
```

Where `z_deg+` and `z_deg-` are learnable vectors indexed by degree. In our 16-node mesh, degree ranges from 0 to 15, requiring at most 16 embedding vectors per direction.

**Spatial Encoding.** The attention bias between nodes i and j depends on their shortest-path distance in the graph. This is added directly to the attention logits:

```
A_ij = (Q_i * K_j) / sqrt(d) + b_SPD(i,j)
```

Where `b_SPD(i,j)` is a learnable scalar indexed by the shortest-path distance. For a 16-node graph, the maximum shortest-path distance is 15 (in a chain), though typical RF meshes have diameter 3-5. This encoding forces the transformer to distinguish between directly connected nodes (1-hop neighbors sharing a line-of-sight path) and distant nodes.

**Edge Encoding.** Edge features along the shortest path between two nodes are aggregated into the attention bias. For RF graphs, edge features include CSI amplitude, phase coherence, signal-to-noise ratio, and temporal stability. This is particularly powerful because the shortest path between two nodes often traverses intermediate links whose coherence values reveal intervening geometry.

**Applicability to RF sensing.** Graphormer's all-pairs attention with structural bias is well-suited to our 16-node mesh because N=16 makes O(N^2) attention tractable (256 pairs). The spatial encoding naturally captures the radio topology — nodes separated by many low-coherence hops are likely in different rooms.

**Limitation.** Graphormer was designed for molecular property prediction with static graphs. RF graphs evolve at 10-100 Hz as people move, doors open, and multipath conditions change. The model needs temporal extension.

### 1.3 Spectral Attention Network (SAN)

SAN (Kreuzer et al., NeurIPS 2021) uses the graph Laplacian eigenvectors as positional encodings, then applies full transformer attention. The key insight is that Laplacian eigenvectors provide a canonical coordinate system for graphs analogous to Fourier modes.

For an RF mesh with adjacency matrix W (CSI coherence weights), the normalized Laplacian is:

```
L = I - D^(-1/2) W D^(-1/2)
```

The eigenvectors of L with the smallest non-zero eigenvalues capture the low-frequency structure of the graph — precisely the large-scale partitions that correspond to room boundaries. The Fiedler vector (eigenvector of the second-smallest eigenvalue) directly encodes the mincut partition.

SAN computes attention separately over the original graph edges ("sparse attention") and all node pairs ("full attention"), then combines them. This dual mechanism lets the model simultaneously exploit local CSI patterns and global graph structure.

**RF relevance.** The spectral decomposition of the CSI coherence graph is physically meaningful. Low-frequency eigenvectors correspond to room-level partitions. Mid-frequency eigenvectors capture furniture and body positions. High-frequency eigenvectors encode multipath scattering details. SAN's spectral positional encoding gives the transformer direct access to these physically grounded features.

### 1.4 General, Powerful, Scalable (GPS) Framework

GPS (Rampasek et al., NeurIPS 2022) unifies message-passing GNNs and transformers into a single framework. Each layer combines:

1. A local message-passing step (MPNN) operating on graph neighbors
2. A global self-attention step operating on all node pairs
3. A positional/structural encoding module

```
h_i^(l+1) = MLP( h_i^(l) + MPNN(h_i^(l), {h_j : j in N(i)}) + Attn(h_i^(l), {h_j : j in V}) )
```

This is particularly relevant for RF sensing because:

- **Local MPNN** captures immediate CSI relationships (direct link coherence, adjacent-link patterns)
- **Global attention** captures long-range dependencies (a person blocking one link affects coherence patterns across the entire mesh)
- **Positional encoding** can be chosen from multiple options (Laplacian, random walk, learned)

For a 16-node mesh, GPS is efficient because both the MPNN (sparse, up to 120 edges for a complete graph) and attention (256 pairs) components are small. The framework's modularity allows systematic ablation of each component's contribution to mincut prediction accuracy.

### 1.5 TokenGT

TokenGT (Kim et al., NeurIPS 2022) takes a radical approach: it represents graphs as pure sequences of tokens (node tokens + edge tokens) and applies a standard transformer without any graph-specific attention modifications.

For each node, TokenGT creates a token from the node features concatenated with a type identifier and orthonormal positional encoding. For each edge, it creates a token from the edge features and the identifiers of its endpoints.

**Token sequence for a 16-node RF mesh:**
- 16 node tokens (each carrying node features: device ID, antenna configuration, noise floor)
- Up to 120 edge tokens for a complete graph (each carrying CSI coherence, amplitude, phase, SNR)
- Total: up to 136 tokens — well within standard transformer capacity

The advantage is simplicity: no custom attention mechanisms, no graph-specific modules. The disadvantage is that all structural information must be learned from the positional encodings and edge tokens rather than being architecturally enforced.

**RF applicability.** TokenGT's approach is attractive for deployment because it uses a vanilla transformer, enabling direct use of optimized inference runtimes (ONNX, TensorRT, CoreML). However, the loss of architectural inductive bias may require more training data to achieve equivalent accuracy.

### 1.6 Comparative Assessment for RF Topological Sensing

| Architecture | Structural Bias | Temporal Support | N=16 Complexity | Deployment Simplicity |
|-------------|----------------|-----------------|-----------------|----------------------|
| Graphormer  | Strong (3 encodings) | None (static) | Low (256 pairs) | Moderate |
| SAN         | Spectral (Laplacian PE) | None (static) | Low | Moderate |
| GPS         | Hybrid (MPNN + attention) | Extensible | Low | Moderate |
| TokenGT     | Minimal (learned) | Extensible | Low (136 tokens) | High (vanilla transformer) |

For the RuView 16-node mesh, all four architectures are computationally feasible. The choice depends on whether we prioritize structural inductive bias (Graphormer, SAN) or deployment simplicity (TokenGT).

---

## 2. Temporal Graph Transformers

### 2.1 The Temporal Dimension of RF Graphs

RF topological graphs are inherently dynamic. A person walking through a room changes CSI coherence on multiple links simultaneously. A door opening creates a sudden topology change. Breathing modulates coherence at 0.1-0.5 Hz. The temporal evolution of the graph IS the sensing signal.

Static graph transformers process one snapshot at a time, discarding temporal correlations. Temporal graph transformers explicitly model how graph structure evolves, enabling:

- Detection of transient events (person crossing a link) vs. persistent changes (furniture rearrangement)
- Velocity estimation from the rate of coherence change across sequential links
- Prediction of future graph states for proactive sensing

### 2.2 Temporal Graph Networks (TGN)

TGN (Rossi et al., ICML 2020 Workshop) maintains a memory state for each node that is updated upon each interaction (edge event). The architecture has four components:

**Message Function.** When an edge event occurs between nodes i and j at time t (e.g., a CSI coherence measurement), a message is computed:

```
m_i(t) = msg(s_i(t-), s_j(t-), delta_t, e_ij(t))
```

Where `s_i(t-)` is node i's memory before the event, `delta_t` is the time since the last event, and `e_ij(t)` is the edge feature (CSI coherence vector).

**Memory Updater.** Node memory is updated via a GRU or LSTM:

```
s_i(t) = GRU(s_i(t-), m_i(t))
```

This persistent memory captures the temporal context of each ESP32 node — its recent coherence history, drift patterns, and interaction frequency.

**Embedding Module.** To compute the embedding for node i at time t, TGN aggregates information from temporal neighbors using attention:

```
z_i(t) = sum_j alpha(s_i, s_j, e_ij, delta_t_ij) * W * s_j(t_j)
```

The attention weights depend on both node memories and the time elapsed since each neighbor's last update.

**Link Predictor / Graph Classifier.** The embeddings are used for downstream tasks — in our case, predicting which edges will be cut (mincut prediction) or classifying graph topology (room occupancy).

**RF sensing adaptation.** TGN's event-driven architecture maps naturally to CSI measurements, which arrive as discrete edge events (node i measures coherence to node j). The persistent memory per node captures slow-changing context (room geometry, device calibration drift) while the embedding module captures fast dynamics (person movement).

For 16 nodes with measurements at 100 Hz across all 120 links, TGN processes approximately 12,000 edge events per second — feasible for the architecture but requiring careful batching.

### 2.3 Temporal Graph Attention (TGAT)

TGAT (Xu et al., ICLR 2020) introduces time-aware attention using a functional time encoding inspired by Bochner's theorem:

```
Phi(t) = sqrt(1/d) * [cos(omega_1 * t), sin(omega_1 * t), ..., cos(omega_d * t), sin(omega_d * t)]
```

This continuous-time encoding allows TGAT to handle irregular sampling — critical for RF sensing where different links may be measured at different rates due to the TDM (Time-Division Multiplexing) protocol on the ESP32 mesh.

The attention mechanism incorporates time explicitly:

```
alpha_ij(t) = softmax( (W_Q * [h_i || Phi(0)]) * (W_K * [h_j || Phi(t - t_j)])^T )
```

Where `t - t_j` is the time elapsed since node j's last measurement. Links measured more recently receive higher attention weight, naturally handling the staleness problem in TDM scheduling.

**RF sensing advantage.** The ESP32 TDM protocol means each node pair is measured at different times within the measurement cycle. TGAT's continuous time encoding elegantly handles this non-uniform sampling without requiring interpolation or resampling.

### 2.4 DyRep: Learning Representations over Dynamic Graphs

DyRep (Trivedi et al., ICLR 2019) models graph dynamics as a temporal point process, learning when edges will change (not just how). The intensity function for an edge event between nodes i and j is:

```
lambda_ij(t) = f(z_i(t), z_j(t), t - t_last)
```

Where `z_i(t)` is node i's representation at time t and `t_last` is the time of the last event on this edge.

For RF sensing, DyRep's point process formulation captures the physics:
- A person walking toward a link increases the event intensity (coherence will change)
- A static environment has low event intensity (coherence is stable)
- The rate of change carries information about movement speed and direction

DyRep maintains two propagation mechanisms:
1. **Localized** (association): immediate neighbor updates when a link changes
2. **Global** (communication): attention-based updates across the entire graph

This dual propagation mirrors the RF sensing reality: a person blocking one link directly affects adjacent links (localized) while also changing the global multipath environment (communication).

### 2.5 Adapting Temporal Graph Transformers for RF Sensing

The key adaptation required for RF topological sensing is bridging the gap between the edge-event paradigm of TGN/TGAT/DyRep and the periodic measurement paradigm of the ESP32 mesh.

**Measurement-as-event mapping.** Each CSI measurement on link (i,j) at time t generates an edge event with features:
- CSI amplitude vector (56 subcarriers after sparse interpolation)
- Phase coherence score
- Signal-to-noise ratio
- Doppler shift estimate
- Coherence change magnitude from previous measurement

**Temporal batching.** Rather than processing events one at a time, batch all measurements from a single TDM cycle (approximately 10ms for 16 nodes) and process them as a temporal graph snapshot. This trades strict event ordering for computational efficiency.

**Hybrid architecture recommendation.** Combine TGN's persistent per-node memory with TGAT's continuous time encoding:
- Node memory captures slow context (room geometry, calibration)
- Time encoding handles irregular TDM sampling
- Graph attention operates on the current snapshot with temporal features
- Mincut prediction head outputs partition probabilities

---

## 3. ViT for RF Spectrograms

### 3.1 CSI-to-Spectrogram Conversion

Channel State Information from a single link is a time series of complex-valued vectors (one complex value per OFDM subcarrier). This naturally maps to a 2D representation:

**Time-Frequency Spectrogram.** For each link (i,j):
- X-axis: time (measurement index)
- Y-axis: subcarrier index (frequency)
- Value: CSI amplitude or phase
- Dimensions: T timesteps x 56 subcarriers (after sparse interpolation from 114)

**Doppler Spectrogram.** Apply short-time Fourier transform along the time axis for each subcarrier:
- X-axis: time window center
- Y-axis: Doppler frequency
- Value: spectral power
- This reveals movement velocities — human walking produces 2-6 Hz Doppler, breathing 0.1-0.5 Hz

**Cross-Link Spectrogram.** Stack spectrograms from multiple links:
- For all 120 links in a 16-node complete graph: a 120 x 56 x T tensor
- Or reshape to a 2D image: (120*56) x T = 6720 x T

### 3.2 Vision Transformer Architecture for RF

ViT (Dosovitskiy et al., ICLR 2021) divides an image into fixed-size patches and processes them as a sequence of tokens. For RF spectrograms:

**Patch extraction.** A spectrogram of dimensions H x W (e.g., 56 subcarriers x 128 timesteps) is divided into patches of size P x P:
- P = 8: yields (56/8) x (128/8) = 7 x 16 = 112 patches
- Each patch captures a local time-frequency region

**Patch embedding.** Each P x P patch is flattened and linearly projected to the transformer dimension d:

```
z_patch = W_embed * flatten(patch) + b_embed
```

**Positional encoding.** Learned 2D positional embeddings encode both the frequency position (which subcarriers) and temporal position (which time window) of each patch.

**Transformer encoder.** Standard multi-head self-attention and feed-forward layers process the sequence of patch tokens.

**Classification head.** For mincut prediction, the [CLS] token output is projected to predict which edges belong to the cut set.

### 3.3 Multi-Link ViT

A single link's spectrogram provides limited spatial information. To capture the full RF topology, we need to process spectrograms from all links jointly.

**Approach 1: Channel stacking.** Treat each link's spectrogram as a separate channel of a multi-channel image. With 120 links and 56 subcarriers over 128 timesteps, this creates a 120-channel 56x128 image. Patch extraction operates across all channels simultaneously.

**Approach 2: Token concatenation.** Process each link's spectrogram independently through shared patch extraction and embedding, then concatenate all link tokens into a single sequence. With 112 patches per link and 120 links, this yields 13,440 tokens — too many for standard attention.

**Approach 3: Hierarchical ViT.** Two-stage processing:
1. **Link-level ViT**: Process each link's spectrogram independently (shared weights), producing one embedding per link (120 embeddings)
2. **Graph-level transformer**: Process the 120 link embeddings with graph-aware attention (using the RF topology as structural bias)

This hierarchical approach is the most promising because:
- The link-level ViT captures local time-frequency patterns (Doppler signatures, phase variations)
- The graph-level transformer captures spatial relationships between links
- Total token count stays manageable (112 for link-level, 120 for graph-level)

### 3.4 ViT Variants for RF

**DeiT (Data-efficient Image Transformers).** Uses knowledge distillation from a CNN teacher, relevant when training data is limited — a common constraint in RF sensing where labeled datasets require manual annotation of room layouts and occupancy.

**Swin Transformer.** Hierarchical ViT with shifted windows, reducing attention complexity from O(N^2) to O(N). For large spectrograms, Swin's local attention windows align with the locality of time-frequency patterns.

**CvT (Convolutional Vision Transformer).** Replaces linear patch embedding with convolutional tokenization, providing translation equivariance. This is beneficial for Doppler spectrograms where the same movement pattern can appear at different time offsets.

### 3.5 Limitations and Trade-offs

The spectrogram/ViT approach has significant limitations for RF topological sensing:

1. **Loss of graph structure.** Converting CSI to spectrograms discards the explicit graph topology. The spatial relationship between links must be re-learned from data.

2. **Fixed temporal window.** ViT processes a fixed-size spectrogram, requiring a choice of temporal window. Too short misses slow events; too long blurs fast events.

3. **Redundant computation.** In a 16-node mesh, many link spectrograms share similar information due to spatial correlation. A graph-native approach avoids this redundancy.

4. **Complementary value.** Despite these limitations, ViT excels at extracting micro-Doppler signatures and time-frequency patterns that graph transformers may miss. The recommended approach uses ViT as a feature extractor feeding into a graph transformer, combining the strengths of both paradigms.

---

## 4. Transformer-Based Mincut Prediction

### 4.1 Problem Formulation

Given a weighted graph G = (V, E, w) where V is 16 ESP32 nodes, E is up to 120 edges, and w: E -> R+ is CSI coherence, the mincut problem is to find a partition (S, V\S) minimizing:

```
cut(S, V\S) = sum_{(i,j) in E: i in S, j in V\S} w(i,j)
```

The exact solution requires O(V^3) max-flow computation (e.g., push-relabel) or O(V * E) augmenting paths. For N=16 and E=120, exact computation takes microseconds — so why use a learned model?

**Reasons for learned mincut prediction:**
1. **Temporal smoothing.** Exact mincut on noisy CSI measurements is unstable. A learned model can produce temporally smooth partitions.
2. **Multi-scale partitioning.** The 2nd, 3rd, ..., kth eigenvectors of the Laplacian encode hierarchical partitions. A transformer can learn to output multi-scale partitions jointly.
3. **Semantic enrichment.** Beyond minimum cut value, a learned model can predict what caused the partition (person, wall, furniture) based on CSI signatures.
4. **Amortized inference.** For real-time deployment at 100 Hz, a single forward pass through a small transformer may be faster than repeated exact computation, especially when targeting k-way partitions.
5. **Differentiable pipeline.** A learned mincut module can be trained end-to-end with downstream tasks (pose estimation, occupancy detection) through gradient flow.

### 4.2 MinCutPool as a Foundation

MinCutPool (Bianchi et al., ICML 2020) formulates graph pooling as a continuous relaxation of the mincut problem. The assignment matrix S is learned:

```
S = softmax(GNN(X, A))
```

Where S[i,k] is the probability that node i belongs to cluster k. The loss function is:

```
L_mincut = -Tr(S^T A S) / Tr(S^T D S)   +   ||S^T S / ||S^T S||_F - I/sqrt(K)||_F
```

The first term minimizes normalized cut. The second term encourages balanced partitions (orthogonality regularization).

**Transformer adaptation.** Replace the GNN in MinCutPool with a graph transformer:

```
S = softmax(GraphTransformer(X, A))
```

This leverages the transformer's global attention to capture long-range dependencies in the RF topology that local GNN message passing may miss.

### 4.3 Architecture: MinCut Transformer

We propose a MinCut Transformer architecture for RF topological sensing:

**Input representation.** For each node i:
- Node features: device configuration, noise floor, antenna pattern (d_node = 32)
- For each edge (i,j): CSI coherence vector, amplitude statistics, temporal gradient (d_edge = 64)

**Encoder.** GPS-style graph transformer with L=4 layers:
- Local MPNN: 2-layer GCN on the CSI coherence graph
- Global attention: multi-head attention with Graphormer-style spatial encoding
- Hidden dimension: d = 128
- Heads: 8

**Mincut prediction head.** Two output branches:

Branch 1 — **Partition assignment**:
```
S = softmax(MLP(h_nodes))  [16 x K matrix for K-way partition]
```

Branch 2 — **Cut edge prediction**:
```
p_cut(i,j) = sigmoid(MLP([h_i || h_j || e_ij]))  [probability that edge (i,j) is cut]
```

**Training objective.** Multi-task loss combining:
1. MinCutPool loss (continuous relaxation of normalized cut)
2. Binary cross-entropy on cut edge prediction (supervised, from exact mincut labels)
3. Temporal consistency loss (penalize rapid partition changes between adjacent frames)
4. Spectral loss (predicted partition should align with Fiedler vector)

### 4.4 Spectral Supervision

A key insight is that the Fiedler vector of the CSI coherence Laplacian provides a strong supervisory signal:

```
L = D - W
Lv_2 = lambda_2 * v_2
```

The sign of v_2 directly encodes the optimal 2-way partition. Training the transformer to predict v_2 (and higher eigenvectors for k-way partitions) provides:
- Dense supervision (every node gets a continuous target, not just a binary label)
- Multi-scale targets (each eigenvector encodes a different partition granularity)
- Physically grounded learning (eigenvectors correspond to room modes of the RF field)

### 4.5 Comparison: Exact vs. Learned Mincut

| Property | Exact (Push-Relabel) | Learned (MinCut Transformer) |
|----------|---------------------|------------------------------|
| Accuracy | Optimal | Near-optimal (after training) |
| Latency (N=16) | ~5 us | ~50 us (forward pass) |
| Temporal smoothness | None (per-frame) | Built-in (temporal loss) |
| Multi-scale output | Requires k runs | Single forward pass |
| Semantic labels | None | Learnable |
| Differentiable | No | Yes |
| Noise robustness | Sensitive | Robust (learned denoising) |

For N=16, exact computation is fast enough for real-time use. The value of the learned approach lies in temporal smoothness, multi-scale output, and end-to-end differentiability rather than raw speed.

---

## 5. Positional Encoding for RF Graphs

### 5.1 Why Positional Encoding Matters

Graph transformers without positional encoding treat graphs as sets of nodes, ignoring topology. For RF sensing, topology IS the primary information carrier. Positional encoding injects structural information that enables the transformer to reason about spatial relationships, path connectivity, and partition structure.

### 5.2 Laplacian Eigenvector Positional Encoding (LapPE)

The eigenvectors of the graph Laplacian L provide a spectral coordinate system:

```
L = U * Lambda * U^T
PE_i = [u_1(i), u_2(i), ..., u_k(i)]
```

Where u_j(i) is the i-th component of the j-th eigenvector.

**Sign ambiguity.** Eigenvectors are defined up to sign flip: if v is an eigenvector, so is -v. This creates a 2^k ambiguity for k eigenvectors. Solutions:
- **SignNet** (Lim et al., ICML 2022): learn a sign-invariant function phi(|v|) + phi(-|v|)
- **BasisNet**: learn in the span of eigenvectors rather than individual vectors
- **Random sign augmentation**: flip signs randomly during training

**RF-specific considerations.** For the CSI coherence graph:
- The first eigenvector (constant) is uninformative
- The Fiedler vector (2nd eigenvector) directly encodes the primary room partition
- Eigenvectors 3-5 encode secondary partitions (sub-rooms, corridors)
- Higher eigenvectors encode local structure (furniture, body positions)
- Using k=8 eigenvectors captures the practically relevant structural scales for a 16-node mesh

**Computational cost.** Eigendecomposition of a 16x16 matrix is negligible (microseconds). For larger meshes, only the bottom-k eigenvectors are needed, computable via Lanczos iteration in O(k * |E|) time.

### 5.3 Random Walk Positional Encoding (RWPE)

RWPE (Dwivedi et al., JMLR 2023) uses the diagonal of random walk powers as node features:

```
PE_i = [RW_ii^1, RW_ii^2, ..., RW_ii^k]
```

Where RW = D^(-1)A is the random walk matrix and RW_ii^t is the probability of returning to node i after t random walk steps.

**Physical interpretation for RF.** In the CSI coherence graph:
- RW_ii^1 = 0 always (no self-loops in measurement graph)
- RW_ii^2 captures local connectivity density (high return probability means node i is in a tightly connected cluster, i.e., a single room)
- RW_ii^t for large t captures global graph structure (convergence rate relates to spectral gap, which relates to how well-separated the rooms are)

**Advantages over LapPE:**
- No sign ambiguity (diagonal elements are always positive)
- Computationally cheaper (matrix powers vs. eigendecomposition)
- Naturally multi-scale (different powers capture different structural scales)

**For 16-node RF mesh:** Use k=16 random walk steps (powers 1 through 16). The return probabilities form a characteristic "fingerprint" for each node's position in the radio topology.

### 5.4 Spatial Encoding (Physical Coordinates)

Unlike many graph learning problems, RF mesh nodes have known physical positions (or positions estimable from CSI). This enables spatial positional encoding:

**Direct coordinate encoding.** If ESP32 nodes have known (x, y, z) coordinates:
```
PE_i = MLP([x_i, y_i, z_i])
```

**Pairwise distance encoding.** For attention between nodes i and j:
```
bias_ij = MLP(||pos_i - pos_j||_2)
```

This injects physical distance into the attention mechanism. Two nodes 1 meter apart with low CSI coherence (suggesting an intervening wall) produce a different attention pattern than two nodes 10 meters apart with the same low coherence (expected signal attenuation).

**Combined encoding.** The most powerful approach combines spectral (LapPE) and spatial (coordinate) encodings:
```
PE_i = concat(LapPE_i, RWPE_i, MLP([x_i, y_i, z_i]))
```

This gives the transformer access to both the topological structure (from spectral encoding) and the physical layout (from spatial encoding).

### 5.5 Relative Positional Encoding

Rather than absolute node positions, relative encodings capture pairwise relationships:

**Graphormer's edge encoding along shortest paths:**
```
b_ij = mean(w_e : e in shortest_path(i, j))
```

For RF graphs, the shortest path in the coherence graph between two distant nodes reveals the "radio corridor" connecting them — the sequence of high-coherence links that radio signals can traverse.

**Rotary Position Embedding (RoPE) for graphs.** Adapt RoPE from language models by using spectral coordinates:
```
RoPE(q, k, theta) where theta is derived from Laplacian eigenvector differences
```

This injects relative spectral position into the attention mechanism without modifying the attention computation, maintaining compatibility with efficient attention implementations.

### 5.6 Encoding Comparison for RF Sensing

| Encoding | Sign Invariant | Multi-scale | Physical Grounding | Computational Cost |
|----------|---------------|-------------|-------------------|-------------------|
| LapPE | No (needs SignNet) | Yes (eigenvector index) | Strong (spectral = partition) | O(N^3) eigendecomp |
| RWPE | Yes | Yes (walk length) | Moderate | O(k * N^2) mat-mul |
| Spatial | N/A | No | Direct (coordinates) | O(N) lookup |
| Combined | Configurable | Yes | Strong | Sum of components |

**Recommendation for RuView:** Use combined encoding (LapPE with SignNet + RWPE + spatial coordinates). The 16-node mesh makes computational cost irrelevant, and the combined encoding provides the richest structural information for mincut prediction.

---

## 6. Foundation Models for RF

### 6.1 The Case for RF Foundation Models

Current RF sensing models are trained from scratch for each environment, task, and hardware configuration. A foundation model pre-trained on diverse RF environments could:

1. **Transfer across environments.** A model pre-trained on 1000 rooms transfers to a new room with minimal fine-tuning.
2. **Transfer across tasks.** Pre-train on self-supervised RF features, fine-tune for specific tasks (mincut, pose estimation, occupancy counting).
3. **Transfer across hardware.** Pre-train on diverse antenna configurations, adapt to specific ESP32 deployments.
4. **Reduce labeling requirements.** Self-supervised pre-training uses unlabeled CSI data (abundant), with only task-specific fine-tuning requiring labels (scarce).

### 6.2 Pre-training Objectives

**Masked CSI Modeling (MCM).** Analogous to masked language modeling in BERT:
- Randomly mask 15% of CSI subcarrier values across links
- Train the transformer to predict masked values from unmasked context
- This forces the model to learn CSI correlation structure across links, subcarriers, and time

**Contrastive Link Prediction.** For each pair of links:
- Positive pairs: links that share a node or are in the same room
- Negative pairs: links in different rooms or with low coherence correlation
- Contrastive loss pushes similar links together in embedding space
- This is related to the AETHER contrastive embedding framework (ADR-024)

**Graph-Level Contrastive Learning.** Augment graphs by:
- Dropping edges below a coherence threshold
- Adding Gaussian noise to edge weights
- Subgraph sampling
- Temporal shifting (comparing t and t+delta)
- Train the model to produce similar embeddings for augmented versions of the same graph

**Temporal Prediction.** Given CSI graphs at times t-k, ..., t-1, t, predict the graph at time t+1:
- Edge weight prediction (CSI coherence at next timestep)
- Topology prediction (which edges will appear/disappear)
- This forces the model to learn physical dynamics of RF propagation

**Spectral Prediction.** Predict Laplacian eigenvalues from node/edge features:
- The eigenvalue spectrum encodes global graph properties (connectivity, partition quality)
- This objective directly trains the model for partition-related downstream tasks

### 6.3 Architecture for RF Foundation Model

**Input tokenization.** Each CSI measurement frame consists of:
- 16 nodes with device features
- Up to 120 edges with CSI feature vectors
- Temporal context window of W frames

**Encoder.** GPS-style graph transformer:
- 12 layers, 512 hidden dimensions, 8 attention heads
- LapPE + RWPE + spatial positional encoding
- Per-node memory (TGN-style) for temporal context
- Estimated parameters: approximately 25M

**Pre-training data requirements.** For effective pre-training:
- Minimum 100 diverse environments (rooms, corridors, open spaces, multi-room apartments)
- Minimum 1000 hours of CSI data per environment
- Diverse conditions: empty rooms, 1-5 occupants, various furniture configurations
- Multiple hardware configurations (antenna counts, node densities, frequencies)

**Data sources.** Combination of:
- Real CSI data from deployed ESP32 meshes (highest quality, limited quantity)
- Simulated CSI using ray-tracing (unlimited quantity, limited fidelity)
- Hybrid: real data augmented with simulated variations

### 6.4 Fine-tuning Strategies

**Linear probing.** Freeze the pre-trained encoder, train only a linear classification head. Tests whether pre-trained representations already encode task-relevant information. For mincut prediction, linear probing on the Fiedler vector prediction provides a diagnostic.

**Low-rank adaptation (LoRA).** Add low-rank update matrices to attention weights:
```
W' = W + alpha * BA
```
Where B is d x r and A is r x d with r << d. This enables task-specific adaptation with minimal additional parameters (typically r=4-16).

**Full fine-tuning.** Update all parameters on task-specific data. Most expressive but requires more labeled data and risks catastrophic forgetting.

**Prompt tuning.** Prepend learnable "prompt" tokens to the input sequence that steer the pre-trained model toward the desired task. For RF sensing, prompts could encode the environment type (residential, commercial, industrial) or task specification (2-way cut, k-way cut, occupancy count).

### 6.5 Cross-Environment Generalization

A critical challenge for RF foundation models is domain shift between environments. The MERIDIAN framework (ADR-027) addresses this through:

1. **Environment fingerprinting.** Learn a compact representation of each environment's RF characteristics (room dimensions, material properties, multipath richness).
2. **Domain-invariant features.** Train the encoder to produce representations that are invariant to environment-specific characteristics while preserving task-relevant information.
3. **Few-shot adaptation.** Given 5-10 minutes of data in a new environment, adapt the model to the new domain using meta-learning techniques.

The foundation model's pre-training across diverse environments naturally supports MERIDIAN-style generalization by exposing the model to the full distribution of RF environments during pre-training.

### 6.6 Scaling Laws

Based on analogies to language and vision foundation models, expected scaling behavior for RF foundation models:

| Model Size | Parameters | Pre-training Data | Expected Mincut F1 (zero-shot) |
|-----------|-----------|-------------------|-------------------------------|
| Tiny | 1M | 100 hours | 0.60 |
| Small | 10M | 1K hours | 0.72 |
| Base | 25M | 10K hours | 0.80 |
| Large | 100M | 100K hours | 0.86 |

These are rough estimates. The key question is whether RF sensing exhibits the same favorable scaling behavior as language and vision. The lower dimensionality of RF data (16 nodes, 120 edges, 56 subcarriers) compared to images (millions of pixels) or text (50K+ vocabulary) suggests that smaller models may suffice.

---

## 7. Efficient Edge Deployment

### 7.1 Deployment Constraints

The ESP32 mesh operates under severe resource constraints:

| Resource | ESP32 | ESP32-S3 | Target Budget |
|----------|-------|----------|--------------|
| RAM | 520 KB | 512 KB + 8MB PSRAM | <2 MB model |
| Flash | 4 MB | 16 MB | <4 MB model |
| Clock | 240 MHz | 240 MHz | <10ms inference |
| FPU | Single-precision | Single-precision | FP32 or INT8 |
| SIMD | None | PIE (128-bit) | Use where available |

Real-time inference at 100 Hz requires completing a forward pass in under 10ms. For on-device inference, this is extremely challenging. The practical deployment model is:

1. **Edge aggregator** (ESP32-S3 with PSRAM): runs the inference model
2. **Sensor nodes** (ESP32): collect CSI and transmit to aggregator
3. **Optional cloud fallback**: for complex models exceeding edge capacity

### 7.2 Knowledge Distillation

Train a small "student" model to mimic a large "teacher" model:

**Teacher.** Full-size graph transformer (GPS, 4 layers, d=128, approximately 2M parameters):
- Trained on labeled CSI data with exact mincut targets
- Achieves best accuracy but too large for edge deployment

**Student.** Tiny graph network (2 layers, d=32, approximately 50K parameters):
- Trained to minimize KL divergence between its output distribution and the teacher's:
```
L_distill = alpha * KL(p_student || p_teacher) + (1-alpha) * L_task
```
- Temperature scaling softens the teacher's predictions, exposing inter-class relationships

**Distillation strategies for RF sensing:**

1. **Output distillation.** Student mimics teacher's mincut partition probabilities.
2. **Feature distillation.** Student's intermediate representations match teacher's (after projection):
```
L_feature = ||proj(h_student^l) - h_teacher^l||_2
```
3. **Attention distillation.** Student's attention patterns match teacher's:
```
L_attention = KL(A_student || A_teacher)
```
This is particularly valuable because the teacher's attention patterns encode which node pairs are most informative for the partition decision.

4. **Spectral distillation.** Student matches teacher's predicted Laplacian eigenvalues. This is a compact, information-dense target that encodes the entire partition structure.

### 7.3 Quantization

**Post-Training Quantization (PTQ).** Convert FP32 weights and activations to INT8 after training:
- Weight quantization: symmetric per-channel quantization for linear layers
- Activation quantization: asymmetric per-tensor with calibration data
- Expected accuracy loss: 1-3% on mincut F1
- Model size reduction: 4x (FP32 to INT8)
- Inference speedup: 2-4x on INT8-capable hardware

**Quantization-Aware Training (QAT).** Simulate quantization during training using straight-through estimators:
- Fake-quantize weights and activations during forward pass
- Backpropagate through the quantization operation using straight-through gradient
- Expected accuracy loss: <1% on mincut F1
- Same size/speed benefits as PTQ

**Mixed-Precision Quantization.** Different layers tolerate different quantization levels:
- Attention QK computation: sensitive, keep FP16
- Attention values and FFN: tolerant, use INT8
- Positional encodings: very sensitive, keep FP32
- Output projection: tolerant, use INT8

For the ESP32-S3, the optimal strategy is INT8 quantization with FP32 positional encodings, yielding approximately 100KB model size for a 2-layer, d=32 student network.

### 7.4 Pruning

**Structured Pruning.** Remove entire attention heads or FFN neurons:
- Score each head by its average attention entropy (low entropy = specialized = important)
- Remove heads with highest entropy (most diffuse attention)
- For a 2-layer, 4-head model: pruning to 2 heads per layer halves attention computation

**Unstructured Pruning.** Zero out individual weights:
- Magnitude pruning: remove weights with smallest absolute value
- 80% sparsity achievable with minimal accuracy loss for graph transformers
- Requires sparse matrix support for inference speedup (not available on ESP32)

**Token Pruning.** For ViT-based approaches, remove uninformative patches:
- Score each patch token by its attention received from the [CLS] token
- Remove bottom 50% of patches after the first transformer layer
- Reduces computation by approximately 2x in subsequent layers

**Structured pruning is recommended** for ESP32 deployment because it reduces model size and computation without requiring sparse matrix hardware support.

### 7.5 Architecture-Level Efficiency

Beyond compression, architectural choices dramatically affect edge efficiency:

**Efficient attention variants:**
- **Linear attention** (Katharopoulos et al., ICML 2020): replaces softmax attention with kernel-based approximation, reducing O(N^2) to O(N). For N=16, the savings are minimal, but it eliminates the softmax computation.
- **Performer** (Choromanski et al., ICLR 2021): random feature approximation of softmax attention. Similar linear complexity.
- For N=16 nodes, standard quadratic attention (256 operations) is already fast enough. Efficient variants matter only for the ViT spectrogram path with many patches.

**Lightweight feed-forward networks:**
- Replace standard 4d FFN with depthwise separable convolutions
- Use GLU (Gated Linear Unit) activation instead of GELU to reduce hidden dimension

**Weight sharing:**
- Share weights across transformer layers (ALBERT-style)
- For a 2-layer model, this halves the parameter count
- Accuracy loss is minimal when combined with distillation

### 7.6 Deployment Pipeline

The recommended deployment pipeline for RuView:

```
1. Train large teacher model (GPU server)
   - GPS graph transformer, 4 layers, d=128
   - Full precision, all data augmentation
   - Target: best possible accuracy

2. Distill to student model (GPU server)
   - 2-layer graph network, d=32
   - Output + attention distillation
   - QAT with INT8 simulation

3. Export to ONNX
   - Fixed input shape (16 nodes, 120 edges)
   - INT8 weights, FP32 positional encodings

4. Convert to TFLite Micro or custom C inference
   - Flatten attention to static matrix operations
   - Pre-compute positional encodings
   - Inline all operations (no dynamic dispatch)

5. Deploy to ESP32-S3 aggregator
   - Model in flash, activations in PSRAM
   - Inference budget: 8ms per frame at 100 Hz
   - Fallback: reduce to 50 Hz if budget exceeded
```

### 7.7 Model Size Estimates

| Configuration | Parameters | INT8 Size | FP32 Size | Estimated Latency (ESP32-S3) |
|--------------|-----------|-----------|-----------|------------------------------|
| 2L, d=16, 2H | 8K | 8 KB | 32 KB | <1 ms |
| 2L, d=32, 4H | 50K | 50 KB | 200 KB | 2-3 ms |
| 2L, d=64, 4H | 180K | 180 KB | 720 KB | 5-8 ms |
| 4L, d=32, 4H | 100K | 100 KB | 400 KB | 4-6 ms |
| 4L, d=64, 8H | 400K | 400 KB | 1.6 MB | 10-15 ms |

The sweet spot for ESP32-S3 deployment is the 2-layer, d=32, 4-head configuration: 50K parameters, 50 KB INT8 model, 2-3 ms inference latency. This fits comfortably within the hardware constraints while providing sufficient model capacity for mincut prediction on a 16-node graph.

---

## 8. Synthesis and Recommendations

### 8.1 Recommended Architecture Stack

Based on the analysis across all seven dimensions, we recommend a layered architecture:

**Layer 1: Feature Extraction (Per-Link)**
- Lightweight 1D CNN or linear projection on raw CSI vectors
- Extracts link-level features: coherence, Doppler, phase gradient
- Runs on each ESP32 sensor node or on the aggregator
- Output: 32-dimensional feature vector per link

**Layer 2: Graph Transformer (Graph-Level)**
- GPS-style architecture with MPNN + global attention
- Combined positional encoding (LapPE + RWPE + spatial)
- 2 layers, d=32, 4 attention heads
- Processes the 16-node graph with link features as edge attributes
- Output: 32-dimensional embedding per node

**Layer 3: MinCut Prediction Head**
- Continuous relaxation (MinCutPool-style) for partition assignment
- Edge-level binary prediction for cut edges
- Spectral supervision from Fiedler vector
- Temporal consistency regularization

**Layer 4: Temporal Integration**
- TGN-style persistent per-node memory (GRU, d=16)
- TGAT-style continuous time encoding for irregular TDM sampling
- Sliding window of 10 frames for temporal context

### 8.2 Training Strategy

**Phase 1: Self-supervised pre-training.**
- Masked CSI modeling on unlabeled data from diverse environments
- Graph contrastive learning with topology augmentation
- Duration: until convergence on held-out environments

**Phase 2: Supervised fine-tuning.**
- Exact mincut labels computed offline
- Fiedler vector regression for spectral supervision
- Multi-task: mincut + occupancy count + room classification
- Duration: until validation plateau

**Phase 3: Distillation and compression.**
- Distill to edge-deployable student model
- Quantization-aware training with INT8
- Structured pruning of attention heads
- Validate accuracy within 3% of teacher model

**Phase 4: Deployment and adaptation.**
- Deploy INT8 model to ESP32-S3 aggregator
- Online few-shot adaptation using LoRA weights stored in PSRAM
- Continuous monitoring of prediction quality vs. exact mincut

### 8.3 Open Research Questions

1. **Spectral vs. spatial positional encoding.** For RF graphs where both the topology and physical coordinates are known, what is the optimal combination? Does one subsume the other?

2. **Scaling laws for RF transformers.** Do RF foundation models follow the same scaling laws as language models, or does the lower intrinsic dimensionality of RF data plateau earlier?

3. **Temporal attention span.** How many past frames should the transformer attend to? Too few misses slow dynamics (breathing); too many wastes computation on stale information.

4. **Adversarial robustness.** Can an attacker manipulate CSI measurements on a few links to fool the mincut predictor? How do we harden the model against adversarial RF injection? This connects to the adversarial detection module in RuvSense.

5. **Graph size generalization.** A model trained on 16-node graphs should ideally generalize to 8-node or 32-node deployments. Graph transformers with relative positional encoding (rather than absolute) are better positioned for this.

6. **Real-time continual learning.** Can the model update itself online as the environment changes (furniture moved, walls added/removed) without catastrophic forgetting of general RF knowledge?

### 8.4 Expected Performance Targets

| Metric | Target | Baseline (Exact Mincut) |
|--------|--------|------------------------|
| Mincut F1 (2-way) | >0.92 | 1.00 (by definition) |
| Mincut F1 (k-way, k=4) | >0.85 | 1.00 |
| Temporal smoothness (jitter) | <0.05 | 0.15 (noisy) |
| Inference latency (ESP32-S3) | <5 ms | <0.1 ms |
| Model size (INT8) | <100 KB | N/A (algorithm) |
| Adaptation to new room | <5 min data | N/A |
| Zero-shot transfer (new room) | >0.75 F1 | 1.00 |

### 8.5 Integration with RuView Pipeline

The transformer-based mincut predictor integrates into the existing RuView architecture at the following points:

- **Input**: CSI frames from `wifi-densepose-signal` (after phase alignment and coherence scoring via RuvSense modules)
- **Graph construction**: `ruvector-mincut` provides the coherence-weighted graph
- **Inference**: New `wifi-densepose-nn` backend for the graph transformer model
- **Output**: Partition assignments consumed by `wifi-densepose-mat` for mass casualty assessment and `pose_tracker` for multi-person tracking
- **Training**: `wifi-densepose-train` with ruvector integration for dataset management

The differentiable mincut predictor enables end-to-end gradient flow from downstream pose estimation loss through the partition decision back to the CSI feature extractor, potentially improving the entire pipeline's accuracy.

---

## References

1. Ying et al. "Do Transformers Really Perform Bad for Graph Representation?" NeurIPS 2021. (Graphormer)
2. Kreuzer et al. "Rethinking Graph Transformers with Spectral Attention." NeurIPS 2021. (SAN)
3. Rampasek et al. "Recipe for a General, Powerful, Scalable Graph Transformer." NeurIPS 2022. (GPS)
4. Kim et al. "Pure Transformers are Powerful Graph Learners." NeurIPS 2022. (TokenGT)
5. Rossi et al. "Temporal Graph Networks for Deep Learning on Dynamic Graphs." ICML Workshop 2020. (TGN)
6. Xu et al. "Inductive Representation Learning on Temporal Graphs." ICLR 2020. (TGAT)
7. Trivedi et al. "DyRep: Learning Representations over Dynamic Graphs." ICLR 2019.
8. Dosovitskiy et al. "An Image is Worth 16x16 Words." ICLR 2021. (ViT)
9. Bianchi et al. "Spectral Clustering with Graph Neural Networks for Graph Pooling." ICML 2020. (MinCutPool)
10. Dwivedi et al. "Benchmarking Graph Neural Networks." JMLR 2023.
11. Lim et al. "Sign and Basis Invariant Networks for Spectral Graph Representation Learning." ICML 2022. (SignNet)
12. Katharopoulos et al. "Transformers are RNNs." ICML 2020. (Linear Attention)
13. Choromanski et al. "Rethinking Attention with Performers." ICLR 2021.
14. Hu et al. "LoRA: Low-Rank Adaptation of Large Language Models." ICLR 2022.

---

*This document supports ADR-029 (RuvSense multistatic sensing mode) and ADR-031 (RuView sensing-first RF mode) by providing the theoretical foundation for transformer-based inference on RF topological graphs.*
