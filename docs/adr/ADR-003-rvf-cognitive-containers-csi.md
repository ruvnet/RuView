# ADR-003: RVF Cognitive Containers for CSI Data

## Status
Proposed

## Date
2026-02-28

## Context

### Problem

WiFi-DensePose processes CSI (Channel State Information) data through a multi-stage pipeline: raw capture → preprocessing → feature extraction → neural inference → pose output. Each stage produces intermediate data that is currently ephemeral:

1. **Raw CSI measurements** (`CsiData`): Amplitude matrices (num_antennas x num_subcarriers), phase arrays, SNR values, metadata. Stored only in a bounded `VecDeque` (max 500 entries in Python, similar in Rust).

2. **Extracted features** (`CsiFeatures`): Amplitude mean/variance, phase differences, correlation matrices, Doppler shifts, power spectral density. Discarded after single-pass inference.

3. **Trained model weights**: Static ONNX/PyTorch files loaded from disk. No mechanism to persist adapted weights or experimental variations.

4. **Detection results** (`HumanDetectionResult`): Confidence scores, motion scores, detection booleans. Logged but not indexed for pattern retrieval.

5. **Environment fingerprints**: Each physical space has a unique CSI signature affected by room geometry, furniture, building materials. No persistent fingerprint database exists.

### Opportunity

RuVector's RVF (Cognitive Container) format provides a single-file packaging solution with 25 segment types that can encapsulate the entire WiFi-DensePose operational state:

```
RVF Cognitive Container Structure:
┌─────────────────────────────────────────────┐
│ HEADER    │ Magic, version, segment count   │
├───────────┼─────────────────────────────────┤
│ VEC       │ CSI feature vectors             │
│ INDEX     │ HNSW index over vectors         │
│ WASM      │ Inference runtime               │
│ COW_MAP   │ Copy-on-write branch state      │
│ WITNESS   │ Audit chain entries             │
│ CRYPTO    │ Signature keys, attestations    │
│ KERNEL    │ Bootable runtime (optional)     │
│ EBPF      │ Hardware-accelerated filters    │
│ ...       │ (25 total segment types)        │
└─────────────────────────────────────────────┘
```

## Decision

We will adopt the RVF Cognitive Container format as the primary persistence and deployment unit for WiFi-DensePose operational data, implementing the following container types:

### 1. CSI Fingerprint Container (`.rvf.csi`)

Packages environment-specific CSI signatures for location recognition:

```rust
/// CSI Fingerprint container storing environment signatures
pub struct CsiFingerprintContainer {
    /// Container metadata
    metadata: ContainerMetadata,

    /// VEC segment: Normalized CSI feature vectors
    /// Each vector = [amplitude_mean(N) | amplitude_var(N) | phase_diff(N-1) | doppler(10) | psd(128)]
    /// Typical dimensionality: 64 subcarriers → 64+64+63+10+128 = 329 dimensions
    fingerprint_vectors: VecSegment,

    /// INDEX segment: HNSW index for O(log n) nearest-neighbor lookup
    hnsw_index: IndexSegment,

    /// COW_MAP: Branches for different times-of-day, occupancy levels
    branches: CowMapSegment,

    /// Metadata per vector: room_id, timestamp, occupancy_count, furniture_hash
    annotations: AnnotationSegment,
}
```

**Vector encoding**: Each CSI snapshot is encoded as a fixed-dimension vector:
```
CSI Feature Vector (329-dim for 64 subcarriers):
┌──────────────────┬──────────────────┬─────────────────┬──────────┬─────────┐
│ amplitude_mean   │ amplitude_var    │ phase_diff      │ doppler  │ psd     │
│ [f32; 64]        │ [f32; 64]        │ [f32; 63]       │ [f32; 10]│ [f32;128│
└──────────────────┴──────────────────┴─────────────────┴──────────┴─────────┘
```

### 2. Model Container (`.rvf.model`)

Packages neural network weights with versioning:

```rust
/// Model container with version tracking and A/B comparison
pub struct ModelContainer {
    /// Container metadata with model version history
    metadata: ContainerMetadata,

    /// Primary model weights (ONNX serialized)
    primary_weights: BlobSegment,

    /// SONA adaptation deltas (LoRA low-rank matrices)
    adaptation_deltas: VecSegment,

    /// COW branches for model experiments
    /// e.g., "baseline", "adapted-office-env", "adapted-warehouse"
    branches: CowMapSegment,

    /// Performance metrics per branch
    metrics: AnnotationSegment,

    /// Witness chain: every weight update recorded
    audit_trail: WitnessSegment,
}
```

### 3. Session Container (`.rvf.session`)

Captures a complete sensing session for replay and analysis:

```rust
/// Session container for recording and replaying sensing sessions
pub struct SessionContainer {
    /// Session metadata (start time, duration, hardware config)
    metadata: ContainerMetadata,

    /// Time-series CSI vectors at capture rate
    csi_timeseries: VecSegment,

    /// Detection results aligned to CSI timestamps
    detections: AnnotationSegment,

    /// Pose estimation outputs
    poses: VecSegment,

    /// Index for temporal range queries
    temporal_index: IndexSegment,

    /// Cryptographic integrity proof
    witness_chain: WitnessSegment,
}
```

### Container Lifecycle

```
  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
  │  Create   │───▶│  Ingest  │───▶│  Query   │───▶│  Branch  │
  │ Container │    │ Vectors  │    │  (HNSW)  │    │  (COW)   │
  └──────────┘    └──────────┘    └──────────┘    └──────────┘
       │                                                │
       │         ┌──────────┐    ┌──────────┐          │
       │         │  Merge   │◀───│  Compare │◀─────────┘
       │         │ Branches │    │ Results  │
       │         └────┬─────┘    └──────────┘
       │              │
       ▼              ▼
  ┌──────────┐   ┌──────────┐
  │  Export   │   │  Deploy  │
  │  (.rvf)  │   │  (Edge)  │
  └──────────┘   └──────────┘
```

### Integration with Existing Crates

The container system integrates through adapter traits:

```rust
/// Trait for types that can be vectorized into RVF containers
pub trait RvfVectorizable {
    /// Encode self as a fixed-dimension f32 vector
    fn to_rvf_vector(&self) -> Vec<f32>;

    /// Reconstruct from an RVF vector
    fn from_rvf_vector(vec: &[f32]) -> Result<Self, RvfError> where Self: Sized;

    /// Vector dimensionality
    fn vector_dim() -> usize;
}

// Implementation for existing types
impl RvfVectorizable for CsiFeatures {
    fn to_rvf_vector(&self) -> Vec<f32> {
        let mut vec = Vec::with_capacity(Self::vector_dim());
        vec.extend(self.amplitude_mean.iter().map(|&x| x as f32));
        vec.extend(self.amplitude_variance.iter().map(|&x| x as f32));
        vec.extend(self.phase_difference.iter().map(|&x| x as f32));
        vec.extend(self.doppler_shift.iter().map(|&x| x as f32));
        vec.extend(self.power_spectral_density.iter().map(|&x| x as f32));
        vec
    }

    fn vector_dim() -> usize {
        // 64 + 64 + 63 + 10 + 128 = 329 (for 64 subcarriers)
        329
    }
    // ...
}
```

### Storage Characteristics

| Container Type | Typical Size | Vector Count | Use Case |
|----------------|-------------|-------------|----------|
| Fingerprint | 5-50 MB | 10K-100K | Room/building fingerprint DB |
| Model | 50-500 MB | N/A (blob) | Neural network deployment |
| Session | 10-200 MB | 50K-500K | 1-hour recording at 100 Hz |

### COW Branching for Environment Adaptation

The copy-on-write mechanism enables zero-overhead experimentation:

```
main (office baseline: 50K vectors)
  ├── branch/morning (delta: 500 vectors, ~15 KB)
  ├── branch/afternoon (delta: 800 vectors, ~24 KB)
  ├── branch/occupied-10 (delta: 2K vectors, ~60 KB)
  └── branch/furniture-moved (delta: 5K vectors, ~150 KB)
```

Total overhead for 4 branches on a 50K-vector container: ~250 KB additional (0.5%).

## Consequences

### Positive
- **Single-file deployment**: Move a fingerprint database between sites by copying one `.rvf` file
- **Versioned models**: A/B test model variants without duplicating full weight sets
- **Session replay**: Reproduce detection results from recorded CSI data
- **Atomic operations**: Container writes are transactional; no partial state corruption
- **Cross-platform**: Same container format works on server, WASM, and embedded
- **Storage efficient**: COW branching avoids duplicating unchanged data

### Negative
- **Format lock-in**: RVF is not yet a widely-adopted standard
- **Serialization overhead**: Converting between native types and RVF vectors adds latency (~0.1-0.5 ms per vector)
- **Learning curve**: Team must understand segment types and container lifecycle
- **File size for sessions**: High-rate CSI capture (1000 Hz) generates large session containers

### Performance Targets

| Operation | Target Latency | Notes |
|-----------|---------------|-------|
| Container open | <10 ms | Memory-mapped I/O |
| Vector insert | <0.1 ms | Append to VEC segment |
| HNSW query (100K vectors) | <1 ms | See ADR-004 |
| Branch create | <1 ms | COW metadata only |
| Branch merge | <100 ms | Delta application |
| Container export | ~1 ms/MB | Sequential write |

## References

- [RuVector Cognitive Container Specification](https://github.com/ruvnet/ruvector)
- [Memory-Mapped I/O in Rust](https://docs.rs/memmap2)
- [Copy-on-Write Data Structures](https://en.wikipedia.org/wiki/Copy-on-write)
- ADR-002: RuVector RVF Integration Strategy
