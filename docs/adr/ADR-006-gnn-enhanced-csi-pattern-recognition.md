# ADR-006: GNN-Enhanced CSI Pattern Recognition

## Status
Proposed

## Date
2026-02-28

## Context

### Limitations of Independent Vector Search

ADR-004 introduces HNSW-based similarity search for CSI pattern matching. While HNSW provides fast nearest-neighbor retrieval, it treats each vector independently. CSI patterns, however, have rich relational structure:

1. **Temporal adjacency**: CSI frames captured 10ms apart are more related than frames 10s apart. Sequential patterns reveal motion trajectories.

2. **Spatial correlation**: CSI readings from adjacent subcarriers are highly correlated due to frequency proximity. Antenna pairs capture different spatial perspectives.

3. **Cross-session similarity**: The "walking to kitchen" pattern from Tuesday should inform Wednesday's recognition, but the environment baseline may have shifted.

4. **Multi-person entanglement**: When multiple people are present, CSI patterns are superpositions. Disentangling requires understanding which pattern fragments co-occur.

Standard HNSW cannot capture these relationships. Each query returns neighbors based solely on vector distance, ignoring the graph structure of how patterns relate to each other.

### RuVector's GNN Enhancement

RuVector implements a Graph Neural Network layer that sits on top of the HNSW index:

```
Standard HNSW: Query → Distance-based neighbors → Results
GNN-Enhanced:  Query → Distance-based neighbors → GNN refinement → Improved results
```

The GNN performs three operations in <1ms:
1. **Message passing**: Each node aggregates information from its HNSW neighbors
2. **Attention weighting**: Multi-head attention identifies which neighbors are most relevant for the current query context
3. **Representation update**: Node embeddings are refined based on neighborhood context

Additionally, **temporal learning** tracks query sequences to discover:
- Vectors that frequently appear together in sessions
- Temporal ordering patterns (A usually precedes B)
- Session context that changes relevance rankings

## Decision

We will integrate RuVector's GNN layer to enhance CSI pattern recognition with three core capabilities: relational search, temporal sequence modeling, and multi-person disentanglement.

### GNN Architecture for CSI

```
┌─────────────────────────────────────────────────────────────────────┐
│                 GNN-Enhanced CSI Pattern Graph                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Layer 1: HNSW Spatial Graph                                        │
│  ┌───────────────────────────────────────────────────────┐          │
│  │  Nodes = CSI feature vectors                          │          │
│  │  Edges = HNSW neighbor connections (distance-based)   │          │
│  │  Node features = [amplitude | phase | doppler | PSD]  │          │
│  └───────────────────────────────────────────────────────┘          │
│                          │                                           │
│                          ▼                                           │
│  Layer 2: Temporal Edges                                            │
│  ┌───────────────────────────────────────────────────────┐          │
│  │  Additional edges between temporally adjacent vectors  │          │
│  │  Edge weight = 1/Δt (closer in time = stronger)       │          │
│  │  Direction = causal (past → future)                    │          │
│  └───────────────────────────────────────────────────────┘          │
│                          │                                           │
│                          ▼                                           │
│  Layer 3: GNN Message Passing (2 rounds)                            │
│  ┌───────────────────────────────────────────────────────┐          │
│  │  Round 1: h_i = σ(W₁·h_i + Σⱼ α_ij · W₂·h_j)       │          │
│  │  Round 2: h_i = σ(W₃·h_i + Σⱼ α'_ij · W₄·h_j)      │          │
│  │  α_ij = softmax(LeakyReLU(a^T[W·h_i || W·h_j]))     │          │
│  │  (Graph Attention Network mechanism)                   │          │
│  └───────────────────────────────────────────────────────┘          │
│                          │                                           │
│                          ▼                                           │
│  Layer 4: Refined Representations                                   │
│  ┌───────────────────────────────────────────────────────┐          │
│  │  Updated vectors incorporate neighborhood context      │          │
│  │  Re-rank search results using refined distances       │          │
│  └───────────────────────────────────────────────────────┘          │
└─────────────────────────────────────────────────────────────────────┘
```

### Three Integration Modes

#### Mode 1: Query-Time Refinement (Default)

GNN refines HNSW results after retrieval. No modifications to stored vectors.

```rust
pub struct GnnQueryRefiner {
    /// GNN weights (small: ~50K parameters)
    gnn_weights: GnnModel,

    /// Number of message passing rounds
    num_rounds: usize,  // 2

    /// Attention heads for neighbor weighting
    num_heads: usize,  // 4

    /// How many HNSW neighbors to consider in GNN
    neighborhood_size: usize,  // 20 (retrieve 20, GNN selects best 5)
}

impl GnnQueryRefiner {
    /// Refine HNSW results using graph context
    pub fn refine(&self, query: &[f32], hnsw_results: &[SearchResult]) -> Vec<SearchResult> {
        // Build local subgraph from query + HNSW results
        let subgraph = self.build_local_subgraph(query, hnsw_results);

        // Run message passing
        let refined = self.message_pass(&subgraph, self.num_rounds);

        // Re-rank based on refined representations
        self.rerank(query, &refined)
    }
}
```

**Latency**: +0.2ms on top of HNSW search (total <1.5ms for 100K vectors).

#### Mode 2: Temporal Sequence Recognition

Tracks CSI vector sequences to recognize activity patterns that span multiple frames:

```rust
/// Temporal pattern recognizer using GNN edges
pub struct TemporalPatternRecognizer {
    /// Sliding window of recent query vectors
    window: VecDeque<TimestampedVector>,

    /// Maximum window size (in frames)
    max_window: usize,  // 100 (10 seconds at 10 Hz)

    /// Temporal edge decay factor
    decay: f64,  // 0.95 (edges weaken with time)

    /// Known activity sequences (learned from data)
    activity_templates: HashMap<String, Vec<Vec<f32>>>,
}

impl TemporalPatternRecognizer {
    /// Feed new CSI vector and check for activity pattern matches
    pub fn observe(&mut self, vector: &[f32], timestamp: f64) -> Vec<ActivityMatch> {
        self.window.push_back(TimestampedVector { vector: vector.to_vec(), timestamp });

        // Build temporal subgraph from window
        let temporal_graph = self.build_temporal_graph();

        // GNN aggregates temporal context
        let sequence_embedding = self.gnn_aggregate(&temporal_graph);

        // Match against known activity templates
        self.match_activities(&sequence_embedding)
    }
}
```

**Activity patterns detectable**:

| Activity | Frames Needed | CSI Signature |
|----------|--------------|---------------|
| Walking | 10-30 | Periodic Doppler oscillation |
| Falling | 5-15 | Sharp amplitude spike → stillness |
| Sitting down | 10-20 | Gradual descent in reflection height |
| Breathing (still) | 30-100 | Micro-periodic phase variation |
| Gesture (wave) | 5-15 | Localized high-frequency amplitude variation |

#### Mode 3: Multi-Person Disentanglement

When N>1 people are present, CSI is a superposition. The GNN learns to cluster pattern fragments:

```rust
/// Multi-person CSI disentanglement using GNN clustering
pub struct MultiPersonDisentangler {
    /// Maximum expected simultaneous persons
    max_persons: usize,  // 10

    /// GNN-based spectral clustering
    cluster_gnn: GnnModel,

    /// Per-person tracking state
    person_tracks: Vec<PersonTrack>,
}

impl MultiPersonDisentangler {
    /// Separate CSI features into per-person components
    pub fn disentangle(&mut self, features: &CsiFeatures) -> Vec<PersonFeatures> {
        // Decompose CSI into subcarrier groups using GNN attention
        let subcarrier_graph = self.build_subcarrier_graph(features);

        // GNN clusters subcarriers by person contribution
        let clusters = self.cluster_gnn.cluster(&subcarrier_graph, self.max_persons);

        // Extract per-person features from clustered subcarriers
        clusters.iter().map(|c| self.extract_person_features(features, c)).collect()
    }
}
```

### GNN Learning Loop

The GNN improves with every query through RuVector's built-in learning:

```
Query → HNSW retrieval → GNN refinement → User action (click/confirm/reject)
                                              │
                                              ▼
                                    Update GNN weights via:
                                    1. Positive: confirmed results get higher attention
                                    2. Negative: rejected results get lower attention
                                    3. Temporal: successful sequences reinforce edges
```

For WiFi-DensePose, "user action" is replaced by:
- **Temporal consistency**: If frame N+1 confirms frame N's detection, reinforce
- **Multi-AP agreement**: If two APs agree on detection, reinforce both
- **Physical plausibility**: If pose satisfies skeletal constraints, reinforce

### Performance Budget

| Component | Parameters | Memory | Latency (per query) |
|-----------|-----------|--------|-------------------|
| GNN weights (2 layers, 4 heads) | 52K | 208 KB | 0.15 ms |
| Temporal graph (100-frame window) | N/A | ~130 KB | 0.05 ms |
| Multi-person clustering | 18K | 72 KB | 0.3 ms |
| **Total GNN overhead** | **70K** | **410 KB** | **0.5 ms** |

## Consequences

### Positive
- **Context-aware search**: Results account for temporal and spatial relationships, not just vector distance
- **Activity recognition**: Temporal GNN enables sequence-level pattern matching
- **Multi-person support**: GNN clustering separates overlapping CSI patterns
- **Self-improving**: Every query provides learning signal to refine attention weights
- **Lightweight**: 70K parameters, 410 KB memory, 0.5ms latency overhead

### Negative
- **Training data needed**: GNN weights require initial training on CSI pattern graphs
- **Complexity**: Three modes increase testing and debugging surface
- **Graph maintenance**: Temporal edges must be pruned to prevent unbounded growth
- **Approximation**: GNN clustering for multi-person is approximate; may merge/split incorrectly

### Interaction with Other ADRs
- **ADR-004** (HNSW): GNN operates on HNSW graph structure; depends on HNSW being available
- **ADR-005** (SONA): GNN weights can be adapted via SONA LoRA for environment-specific tuning
- **ADR-003** (RVF): GNN weights stored in model container alongside inference weights
- **ADR-010** (Witness): GNN weight updates recorded in witness chain

## References

- [Graph Attention Networks (GAT)](https://arxiv.org/abs/1710.10903)
- [Temporal Graph Networks](https://arxiv.org/abs/2006.10637)
- [Spectral Clustering with Graph Neural Networks](https://arxiv.org/abs/1907.00481)
- [WiFi-based Multi-Person Sensing](https://dl.acm.org/doi/10.1145/3534592)
- [RuVector GNN Implementation](https://github.com/ruvnet/ruvector)
- ADR-004: HNSW Vector Search for Signal Fingerprinting
