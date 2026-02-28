# ADR-004: HNSW Vector Search for Signal Fingerprinting

## Status
Proposed

## Date
2026-02-28

## Context

### Current Signal Matching Limitations

The WiFi-DensePose system needs to match incoming CSI patterns against known signatures for:

1. **Environment recognition**: Identifying which room/area the device is in based on CSI characteristics
2. **Activity classification**: Matching current CSI patterns to known human activities (walking, sitting, falling)
3. **Anomaly detection**: Determining whether current readings deviate significantly from baseline
4. **Survivor re-identification** (MAT module): Tracking individual survivors across scan sessions

Current approach in `CSIProcessor._calculate_detection_confidence()`:
```python
# Fixed thresholds, no similarity search
amplitude_indicator = np.mean(features.amplitude_mean) > 0.1
phase_indicator = np.std(features.phase_difference) > 0.05
motion_indicator = motion_score > 0.3
confidence = (0.4 * amplitude_indicator + 0.3 * phase_indicator + 0.3 * motion_indicator)
```

This is a **O(1) fixed-threshold check** that:
- Cannot learn from past observations
- Has no concept of "similar patterns seen before"
- Requires manual threshold tuning per environment
- Produces binary indicators (above/below threshold) losing gradient information

### What HNSW Provides

Hierarchical Navigable Small World (HNSW) graphs enable approximate nearest-neighbor search in high-dimensional vector spaces with:

- **O(log n) query time** vs O(n) brute-force
- **High recall**: >95% recall at 10x speed of exact search
- **Dynamic insertion**: New vectors added without full rebuild
- **SIMD acceleration**: RuVector's implementation uses AVX2/NEON for distance calculations

RuVector extends standard HNSW with:
- **Hyperbolic HNSW**: Search in Poincaré ball space for hierarchy-aware results (e.g., "walking" is closer to "running" than to "sitting" in activity hierarchy)
- **GNN enhancement**: Graph neural networks refine neighbor connections after queries
- **Tiered compression**: 2-32x memory reduction through adaptive quantization

## Decision

We will integrate RuVector's HNSW implementation as the primary similarity search engine for all CSI pattern matching operations, replacing fixed-threshold detection with similarity-based retrieval.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    HNSW Search Pipeline                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  CSI Input     Feature           Vector         HNSW            │
│  ────────▶    Extraction   ────▶ Encode   ────▶ Search          │
│              (existing)        (new)          (new)             │
│                                                  │              │
│                                    ┌─────────────┤              │
│                                    ▼             ▼              │
│                              Top-K Results    Confidence        │
│                              [vec_id, dist,   Score from        │
│                               metadata]       Distance Dist.   │
│                                    │                            │
│                                    ▼                            │
│                              ┌────────────┐                     │
│                              │ Decision   │                     │
│                              │ Fusion     │                     │
│                              └────────────┘                     │
│                               Combines HNSW similarity with     │
│                               existing threshold-based logic    │
└─────────────────────────────────────────────────────────────────┘
```

### Index Configuration

```rust
/// HNSW configuration tuned for CSI vector characteristics
pub struct CsiHnswConfig {
    /// Vector dimensionality (matches CsiFeatures encoding)
    dim: usize,  // 329 for 64 subcarriers

    /// Maximum number of connections per node per layer
    /// Higher M = better recall, more memory
    /// CSI vectors are moderately dimensional; M=16 balances well
    m: usize,  // 16

    /// Size of dynamic candidate list during construction
    /// ef_construction = 200 gives >99% recall for 329-dim vectors
    ef_construction: usize,  // 200

    /// Size of dynamic candidate list during search
    /// ef_search = 64 gives >95% recall with <1ms latency at 100K vectors
    ef_search: usize,  // 64

    /// Distance metric
    /// Cosine similarity works best for normalized CSI features
    metric: DistanceMetric,  // Cosine

    /// Maximum elements (pre-allocated for performance)
    max_elements: usize,  // 1_000_000

    /// Enable SIMD acceleration
    simd: bool,  // true

    /// Quantization level for memory reduction
    quantization: Quantization,  // PQ8 (product quantization, 8-bit)
}
```

### Multiple Index Strategy

Different use cases require different index configurations:

| Index Name | Vectors | Dim | Distance | Use Case |
|-----------|---------|-----|----------|----------|
| `env_fingerprint` | 10K-1M | 329 | Cosine | Environment/room identification |
| `activity_pattern` | 1K-50K | 329 | Euclidean | Activity classification |
| `temporal_pattern` | 10K-500K | 329 | Cosine | Temporal anomaly detection |
| `survivor_track` | 100-10K | 329 | Cosine | MAT survivor re-identification |

### Similarity-Based Detection Enhancement

Replace fixed thresholds with distance-based confidence:

```rust
/// Enhanced detection using HNSW similarity search
pub struct SimilarityDetector {
    /// HNSW index of known human-present CSI patterns
    human_patterns: HnswIndex,

    /// HNSW index of known empty-room CSI patterns
    empty_patterns: HnswIndex,

    /// Fusion weight between similarity and threshold methods
    fusion_alpha: f64,  // 0.7 = 70% similarity, 30% threshold
}

impl SimilarityDetector {
    /// Detect human presence using similarity search + threshold fusion
    pub fn detect(&self, features: &CsiFeatures) -> DetectionResult {
        let query_vec = features.to_rvf_vector();

        // Search both indices
        let human_neighbors = self.human_patterns.search(&query_vec, k=5);
        let empty_neighbors = self.empty_patterns.search(&query_vec, k=5);

        // Distance-based confidence
        let avg_human_dist = human_neighbors.mean_distance();
        let avg_empty_dist = empty_neighbors.mean_distance();

        // Similarity confidence: how much closer to human patterns vs empty
        let similarity_confidence = avg_empty_dist / (avg_human_dist + avg_empty_dist);

        // Fuse with traditional threshold-based confidence
        let threshold_confidence = self.traditional_threshold_detect(features);
        let fused_confidence = self.fusion_alpha * similarity_confidence
                             + (1.0 - self.fusion_alpha) * threshold_confidence;

        DetectionResult {
            human_detected: fused_confidence > 0.5,
            confidence: fused_confidence,
            similarity_confidence,
            threshold_confidence,
            nearest_human_pattern: human_neighbors[0].metadata.clone(),
            nearest_empty_pattern: empty_neighbors[0].metadata.clone(),
        }
    }
}
```

### Incremental Learning Loop

Every confirmed detection enriches the index:

```
1. CSI captured → features extracted → vector encoded
2. HNSW search returns top-K neighbors + distances
3. Detection decision made (similarity + threshold fusion)
4. If confirmed (by temporal consistency or ground truth):
   a. Insert vector into appropriate index (human/empty)
   b. GNN layer updates neighbor relationships (ADR-006)
   c. SONA adapts fusion weights (ADR-005)
5. Periodically: prune stale vectors, rebuild index layers
```

### Performance Analysis

**Memory requirements** (PQ8 quantization):

| Vector Count | Raw Size | PQ8 Compressed | HNSW Overhead | Total |
|-------------|----------|----------------|---------------|-------|
| 10,000 | 12.9 MB | 1.6 MB | 2.5 MB | 4.1 MB |
| 100,000 | 129 MB | 16 MB | 25 MB | 41 MB |
| 1,000,000 | 1.29 GB | 160 MB | 250 MB | 410 MB |

**Latency expectations** (329-dim vectors, ef_search=64):

| Vector Count | Brute Force | HNSW | Speedup |
|-------------|-------------|------|---------|
| 10,000 | 3.2 ms | 0.08 ms | 40x |
| 100,000 | 32 ms | 0.3 ms | 107x |
| 1,000,000 | 320 ms | 0.9 ms | 356x |

### Hyperbolic Extension for Activity Hierarchy

WiFi-sensed activities have natural hierarchy:

```
                    motion
                   /      \
              locomotion   stationary
              /    \         /    \
         walking  running  sitting  lying
         /    \
      normal  shuffling
```

Hyperbolic HNSW in Poincaré ball space preserves this hierarchy during search, so a query for "shuffling" returns "walking" before "sitting" even if Euclidean distances are similar.

```rust
/// Hyperbolic HNSW for hierarchy-aware activity matching
pub struct HyperbolicActivityIndex {
    index: HnswIndex,
    curvature: f64,  // -1.0 for unit Poincaré ball
}

impl HyperbolicActivityIndex {
    pub fn search(&self, query: &[f32], k: usize) -> Vec<SearchResult> {
        // Uses Poincaré distance: d(u,v) = arcosh(1 + 2||u-v||²/((1-||u||²)(1-||v||²)))
        self.index.search_hyperbolic(query, k, self.curvature)
    }
}
```

## Consequences

### Positive
- **Adaptive detection**: System improves with more data; no manual threshold tuning
- **Sub-millisecond search**: HNSW provides <1ms queries even at 1M vectors
- **Memory efficient**: PQ8 reduces storage 8x with <5% recall loss
- **Hierarchy-aware**: Hyperbolic mode respects activity relationships
- **Incremental**: New patterns added without full index rebuild
- **Explainable**: "This detection matched pattern X from room Y at time Z"

### Negative
- **Cold-start problem**: Need initial fingerprint data before similarity search is useful
- **Index maintenance**: Periodic pruning and layer rebalancing needed
- **Approximation**: HNSW is approximate; may miss exact nearest neighbor (mitigated by high ef_search)
- **Memory for indices**: HNSW graph structure adds 2.5x overhead on top of vectors

### Migration Strategy

1. **Phase 1**: Run HNSW search in parallel with existing threshold detection, log both results
2. **Phase 2**: A/B test fusion weights (alpha parameter) on labeled data
3. **Phase 3**: Gradually increase fusion_alpha from 0.0 (pure threshold) to 0.7 (primarily similarity)
4. **Phase 4**: Threshold detection becomes fallback for cold-start/empty-index scenarios

## References

- [HNSW: Efficient and Robust Approximate Nearest Neighbor](https://arxiv.org/abs/1603.09320)
- [Product Quantization for Nearest Neighbor Search](https://hal.inria.fr/inria-00514462)
- [Poincaré Embeddings for Learning Hierarchical Representations](https://arxiv.org/abs/1705.08039)
- [RuVector HNSW Implementation](https://github.com/ruvnet/ruvector)
- ADR-003: RVF Cognitive Containers for CSI Data
