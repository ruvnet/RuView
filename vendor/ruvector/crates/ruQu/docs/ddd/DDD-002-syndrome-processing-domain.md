# DDD-002: Syndrome Processing Domain Model

**Status**: Proposed
**Date**: 2026-01-17
**Authors**: ruv.io, RuVector Team
**Related ADR**: ADR-001-ruqu-architecture
**Related DDD**: DDD-001-coherence-gate-domain

---

## Overview

This document defines the Domain-Driven Design model for the Syndrome Processing subsystem—the high-throughput data pipeline that ingests, buffers, and transforms quantum error syndromes into coherence-relevant signals.

---

## Strategic Design

### Domain Vision Statement

> The Syndrome Processing domain provides reliable, low-latency ingestion and transformation of quantum syndrome data, enabling the Coherence Gate to make real-time structural assessments at microsecond timescales.

### Supporting Domain

Syndrome Processing is a **supporting domain** to the core Coherence Gate domain. It provides:

- Data acquisition infrastructure
- Buffering and flow control
- Format transformation
- Temporal alignment

### Relationship to Core Domain

```
┌─────────────────────────────────────────────────────────────────┐
│                    COHERENCE GATE (Core)                        │
│                                                                 │
│  Consumes: SyndromeEvents, GraphDeltas                          │
│  Produces: Decisions, Permits                                   │
└─────────────────────────────────────────────────────────────────┘
                              ▲
                              │ Conforms
                              │
┌─────────────────────────────────────────────────────────────────┐
│               SYNDROME PROCESSING (Supporting)                  │
│                                                                 │
│  Consumes: RawSyndromes, DetectorMaps                           │
│  Produces: SyndromeEvents, GraphDeltas                          │
└─────────────────────────────────────────────────────────────────┘
                              ▲
                              │ Upstream
                              │
┌─────────────────────────────────────────────────────────────────┐
│                  HARDWARE INTERFACE (External)                  │
│                                                                 │
│  Produces: RawSyndromes, Timestamps, Status                     │
└─────────────────────────────────────────────────────────────────┘
```

---

## Ubiquitous Language

### Core Terms

| Term | Definition | Context |
|------|------------|---------|
| **Syndrome** | A binary vector indicating which stabilizer measurements detected errors | Data |
| **Round** | A complete cycle of syndrome measurements (typically 1μs) | Temporal |
| **Detector** | A single stabilizer measurement outcome (0 or 1) | Atomic |
| **Flipped Detector** | A detector that fired (value = 1), indicating potential error | Signal |

### Buffer Terms

| Term | Definition | Context |
|------|------------|---------|
| **Ring Buffer** | Circular buffer holding recent syndrome rounds | Storage |
| **Window** | A sliding view over recent rounds for analysis | View |
| **Watermark** | The oldest round still in the buffer | Temporal |
| **Backpressure** | Flow control when buffer nears capacity | Control |

### Transform Terms

| Term | Definition | Context |
|------|------------|---------|
| **Delta** | Change in syndrome state between rounds | Derivative |
| **Correlation** | Statistical relationship between detector firings | Analysis |
| **Cluster** | Group of spatially correlated detector firings | Pattern |
| **Hot Spot** | Region with elevated detector firing rate | Anomaly |

### Graph Integration Terms

| Term | Definition | Context |
|------|------------|---------|
| **Graph Delta** | Update to operational graph from syndrome analysis | Output |
| **Edge Weight Update** | Modification to edge weight based on correlations | Output |
| **Vertex Health Update** | Modification to vertex health based on syndromes | Output |

---

## Bounded Context

### Context Map

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       SYNDROME PROCESSING CONTEXT                           │
│                          (Supporting Domain)                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │  Ingestion  │  │   Buffer    │  │  Transform  │  │   Publish   │        │
│  │   Layer     │──│   Layer     │──│   Layer     │──│   Layer     │        │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────────────────────┘
          ▲                                                   │
          │ Raw Data                                          │ Events
          │                                                   ▼
┌─────────────────┐                                 ┌─────────────────┐
│    HARDWARE     │                                 │  COHERENCE GATE │
│    INTERFACE    │                                 │    CONTEXT      │
└─────────────────┘                                 └─────────────────┘
```

---

## Aggregates

### SyndromeRound (Root Aggregate)

Represents a complete syndrome measurement cycle.

```
┌─────────────────────────────────────────────────────────────────┐
│                     SYNDROME ROUND                              │
│                    (Aggregate Root)                             │
├─────────────────────────────────────────────────────────────────┤
│  round_id: RoundId                                              │
│  cycle: CycleId                                                 │
│  timestamp: Timestamp (hardware clock)                          │
│  received_at: Timestamp (local clock)                           │
│  detectors: DetectorBitmap                                      │
│  source_tile: TileId                                            │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ DetectorBitmap (Value Object)                           │   │
│  │  bits: [u64; N]  // Packed detector values              │   │
│  │  detector_count: usize                                  │   │
│  │                                                         │   │
│  │  fn fired_count(&self) -> usize                         │   │
│  │  fn get(&self, idx: usize) -> bool                      │   │
│  │  fn iter_fired(&self) -> impl Iterator<Item = usize>    │   │
│  └─────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────┤
│  Invariants:                                                    │
│  - round_id unique per tile                                     │
│  - timestamp monotonically increasing per tile                  │
│  - detector_count matches configured detector map               │
└─────────────────────────────────────────────────────────────────┘
```

### SyndromeBuffer (Aggregate)

Ring buffer holding recent syndrome history.

```
┌─────────────────────────────────────────────────────────────────┐
│                     SYNDROME BUFFER                             │
│                    (Aggregate Root)                             │
├─────────────────────────────────────────────────────────────────┤
│  buffer_id: BufferId                                            │
│  tile_id: TileId                                                │
│  capacity: usize (typically 1024 rounds)                        │
│  write_index: usize                                             │
│  watermark: RoundId                                             │
│  rounds: CircularArray<SyndromeRound>                           │
├─────────────────────────────────────────────────────────────────┤
│  Methods:                                                       │
│  fn push(&mut self, round: SyndromeRound)                       │
│  fn window(&self, size: usize) -> &[SyndromeRound]              │
│  fn get(&self, round_id: RoundId) -> Option<&SyndromeRound>     │
│  fn statistics(&self) -> BufferStatistics                       │
├─────────────────────────────────────────────────────────────────┤
│  Invariants:                                                    │
│  - capacity fixed at creation                                   │
│  - watermark ≤ oldest round in buffer                           │
│  - write_index wraps at capacity                                │
└─────────────────────────────────────────────────────────────────┘
```

### DetectorMap (Aggregate)

Configuration mapping detectors to physical qubits.

```
┌─────────────────────────────────────────────────────────────────┐
│                      DETECTOR MAP                               │
│                    (Aggregate Root)                             │
├─────────────────────────────────────────────────────────────────┤
│  map_id: MapId                                                  │
│  version: Version                                               │
│  detector_count: usize                                          │
│  mappings: Vec<DetectorMapping>                                 │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ DetectorMapping (Entity)                                │   │
│  │  detector_idx: usize                                    │   │
│  │  qubit_ids: Vec<QubitId>   // Qubits in support         │   │
│  │  detector_type: DetectorType { X | Z | Flag }           │   │
│  │  coordinates: Option<(f64, f64, f64)>                   │   │
│  └─────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────┤
│  Methods:                                                       │
│  fn qubits_for_detector(&self, idx: usize) -> &[QubitId]        │
│  fn detectors_for_qubit(&self, qubit: QubitId) -> Vec<usize>    │
│  fn neighbors(&self, idx: usize) -> Vec<usize>                  │
├─────────────────────────────────────────────────────────────────┤
│  Invariants:                                                    │
│  - detector_idx unique                                          │
│  - All referenced qubits exist in hardware                      │
│  - Version increments on any change                             │
└─────────────────────────────────────────────────────────────────┘
```

---

## Value Objects

### DetectorBitmap

Efficient packed representation of detector values.

```rust
struct DetectorBitmap {
    bits: [u64; 16],  // 1024 detectors max
    count: usize,
}

impl DetectorBitmap {
    fn new(count: usize) -> Self;
    fn set(&mut self, idx: usize, value: bool);
    fn get(&self, idx: usize) -> bool;
    fn fired_count(&self) -> usize;
    fn iter_fired(&self) -> impl Iterator<Item = usize>;
    fn xor(&self, other: &DetectorBitmap) -> DetectorBitmap;
    fn popcount(&self) -> usize;
}
```

### SyndromeDelta

Change between consecutive rounds.

```rust
struct SyndromeDelta {
    from_round: RoundId,
    to_round: RoundId,
    flipped: DetectorBitmap,  // XOR of consecutive rounds
    new_firings: Vec<usize>,
    cleared_firings: Vec<usize>,
}

impl SyndromeDelta {
    fn is_quiet(&self) -> bool {
        self.flipped.popcount() == 0
    }

    fn activity_level(&self) -> f64 {
        self.flipped.popcount() as f64 / self.flipped.count as f64
    }
}
```

### CorrelationMatrix

Pairwise detector correlations.

```rust
struct CorrelationMatrix {
    size: usize,
    // Packed upper triangle (symmetric)
    correlations: Vec<f32>,
}

impl CorrelationMatrix {
    fn get(&self, i: usize, j: usize) -> f32;
    fn update(&mut self, i: usize, j: usize, value: f32);
    fn significant_pairs(&self, threshold: f32) -> Vec<(usize, usize, f32)>;
}
```

### DetectorCluster

Group of correlated detectors.

```rust
struct DetectorCluster {
    cluster_id: ClusterId,
    detectors: Vec<usize>,
    centroid: Option<(f64, f64, f64)>,
    firing_rate: f64,
}

impl DetectorCluster {
    fn size(&self) -> usize;
    fn is_hot_spot(&self, threshold: f64) -> bool;
    fn spatial_extent(&self) -> f64;
}
```

---

## Domain Events

### Ingestion Events

| Event | Trigger | Payload |
|-------|---------|---------|
| `RoundReceived` | New syndrome arrives | round_id, timestamp, raw_data |
| `RoundDropped` | Buffer overflow | round_id, reason |
| `IngestionPaused` | Backpressure | buffer_fill_level |
| `IngestionResumed` | Buffer drains | buffer_fill_level |

### Buffer Events

| Event | Trigger | Payload |
|-------|---------|---------|
| `BufferFull` | Capacity reached | watermark, oldest_round |
| `WatermarkAdvanced` | Old data evicted | old_watermark, new_watermark |
| `WindowExtracted` | Analysis requested | start_round, end_round, size |

### Transform Events

| Event | Trigger | Payload |
|-------|---------|---------|
| `DeltaComputed` | Round processed | delta |
| `ClusterDetected` | Spatial correlation | cluster |
| `HotSpotIdentified` | Elevated activity | region, rate, duration |
| `CorrelationUpdated` | Statistics refresh | matrix_hash |

### Output Events

| Event | Trigger | Payload |
|-------|---------|---------|
| `GraphDeltaPublished` | Transform complete | graph_delta |
| `SyndromeEventPublished` | For gate consumption | syndrome_event |
| `StatisticsPublished` | Periodic | statistics |

---

## Domain Services

### SyndromeIngestionService

High-throughput syndrome ingestion.

```rust
trait SyndromeIngestionService {
    /// Receive raw syndrome packet from hardware
    async fn receive(&self, packet: RawSyndromePacket) -> Result<RoundId, IngestError>;

    /// Get current ingestion rate
    fn throughput(&self) -> f64;

    /// Apply backpressure
    fn pause(&self);
    fn resume(&self);
}
```

### SyndromeBufferService

Buffer management and windowing.

```rust
trait SyndromeBufferService {
    /// Get current buffer for a tile
    fn buffer(&self, tile: TileId) -> &SyndromeBuffer;

    /// Extract window for analysis
    fn window(&self, tile: TileId, size: usize) -> Window;

    /// Get statistics
    fn statistics(&self, tile: TileId) -> BufferStatistics;

    /// Force eviction of old data
    fn evict(&self, tile: TileId, before: RoundId);
}
```

### SyndromeTransformService

Transform syndromes to coherence signals.

```rust
trait SyndromeTransformService {
    /// Compute delta between consecutive rounds
    fn compute_delta(&self, from: &SyndromeRound, to: &SyndromeRound) -> SyndromeDelta;

    /// Update correlation matrix with new round
    fn update_correlations(&self, round: &SyndromeRound);

    /// Detect clusters in current window
    fn detect_clusters(&self, window: &Window) -> Vec<DetectorCluster>;

    /// Generate graph delta from syndrome analysis
    fn to_graph_delta(&self, delta: &SyndromeDelta, clusters: &[DetectorCluster]) -> GraphDelta;
}
```

### SyndromePublishService

Publish events to Coherence Gate context.

```rust
trait SyndromePublishService {
    /// Publish syndrome event
    async fn publish_syndrome(&self, event: SyndromeEvent);

    /// Publish graph delta
    async fn publish_graph_delta(&self, delta: GraphDelta);

    /// Publish statistics
    async fn publish_statistics(&self, stats: SyndromeStatistics);
}
```

---

## Repositories

### SyndromeRoundRepository

```rust
trait SyndromeRoundRepository {
    /// Store round (typically in ring buffer)
    fn store(&self, round: SyndromeRound);

    /// Find by round ID
    fn find_by_id(&self, id: RoundId) -> Option<&SyndromeRound>;

    /// Find rounds in range
    fn find_in_range(&self, start: RoundId, end: RoundId) -> Vec<&SyndromeRound>;

    /// Get most recent N rounds
    fn recent(&self, n: usize) -> Vec<&SyndromeRound>;
}
```

### DetectorMapRepository

```rust
trait DetectorMapRepository {
    /// Get current detector map
    fn current(&self) -> &DetectorMap;

    /// Get map at specific version
    fn at_version(&self, version: Version) -> Option<&DetectorMap>;

    /// Update map
    fn update(&self, map: DetectorMap) -> Result<(), UpdateError>;
}
```

### CorrelationRepository

```rust
trait CorrelationRepository {
    /// Get current correlation matrix
    fn current(&self) -> &CorrelationMatrix;

    /// Update correlation
    fn update(&self, i: usize, j: usize, value: f32);

    /// Get historical snapshot
    fn snapshot_at(&self, round: RoundId) -> Option<&CorrelationMatrix>;
}
```

---

## Processing Pipeline

### Pipeline Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SYNDROME PROCESSING PIPELINE                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌───────────┐   ┌───────────┐   ┌───────────┐   ┌───────────┐             │
│  │  Receive  │──▶│  Decode   │──▶│   Store   │──▶│  Window   │             │
│  │   (DMA)   │   │  (Unpack) │   │  (Ring)   │   │ (Extract) │             │
│  └───────────┘   └───────────┘   └───────────┘   └───────────┘             │
│       50ns           100ns           50ns           50ns                    │
│                                                                             │
│                                        │                                    │
│                                        ▼                                    │
│  ┌───────────┐   ┌───────────┐   ┌───────────┐   ┌───────────┐             │
│  │  Publish  │◀──│   Graph   │◀──│  Cluster  │◀──│   Delta   │             │
│  │  (Event)  │   │  (Update) │   │  (Find)   │   │ (Compute) │             │
│  └───────────┘   └───────────┘   └───────────┘   └───────────┘             │
│       50ns           100ns          200ns           100ns                   │
│                                                                             │
│  Total Pipeline Latency: ~700ns                                             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Stage Details

#### Stage 1: Receive
- DMA transfer from hardware
- CRC validation
- Timestamp extraction

#### Stage 2: Decode
- Unpack compressed syndrome format
- Map to detector indices
- Validate against detector map

#### Stage 3: Store
- Append to ring buffer
- Handle buffer wrap
- Evict old entries if needed

#### Stage 4: Window
- Extract sliding window
- Compute running statistics
- Prepare for analysis

#### Stage 5: Delta
- XOR consecutive rounds
- Identify new/cleared firings
- Calculate activity level

#### Stage 6: Cluster
- Spatial clustering of firings
- Identify hot spots
- Track cluster evolution

#### Stage 7: Graph Update
- Map clusters to graph regions
- Compute edge weight updates
- Compute vertex health updates

#### Stage 8: Publish
- Emit SyndromeEvent
- Emit GraphDelta
- Update statistics

---

## Memory Layout

### Per-Tile Memory Budget (16 KB for Syndrome Processing)

```
0x8000 - 0xBFFF : Syndrome Ring Buffer (16 KB)
  ├── 0x8000 - 0x800F : Buffer metadata (16 bytes)
  │     write_index: u32
  │     watermark: u32
  │     capacity: u32
  │     flags: u32
  │
  ├── 0x8010 - 0xBFEF : Round storage (16,352 bytes)
  │     1024 rounds × 16 bytes per round
  │     Each round:
  │       round_id: u32
  │       timestamp: u32
  │       detector_bitmap: [u8; 8] (64 detectors per tile)
  │
  └── 0xBFF0 - 0xBFFF : Statistics cache (16 bytes)
        firing_rate: f32
        activity_mean: f32
        activity_variance: f32
        padding: u32
```

### Published Language (to Coherence Gate)

```rust
/// Event published to Coherence Gate context
struct SyndromeEvent {
    round_id: RoundId,
    tile_id: TileId,
    timestamp: Timestamp,
    activity_level: f64,
    hot_spots: Vec<HotSpot>,
    delta_summary: DeltaSummary,
}

/// Graph update derived from syndrome analysis
struct GraphDelta {
    source_round: RoundId,
    vertex_updates: Vec<VertexUpdate>,
    edge_updates: Vec<EdgeUpdate>,
}

struct VertexUpdate {
    vertex_id: VertexId,
    health_delta: f64,
}

struct EdgeUpdate {
    edge_id: EdgeId,
    weight_delta: f64,
}
```

---

## Invariants and Business Rules

### Ingestion Invariants

1. **Temporal Ordering**: Rounds must arrive in timestamp order per tile
2. **No Gaps**: Round IDs must be consecutive (gaps indicate data loss)
3. **CRC Validity**: Invalid CRCs cause round rejection
4. **Rate Bounded**: Ingestion rate ≤ 1M rounds/second

### Buffer Invariants

1. **Fixed Capacity**: Buffer size constant after creation
2. **FIFO Ordering**: Oldest data evicted first
3. **Watermark Monotonicity**: Watermark only advances
4. **Window Containment**: Window must be within buffer

### Transform Invariants

1. **Deterministic**: Same input always produces same output
2. **Bounded Latency**: Transform ≤ 500ns
3. **Conservation**: Delta popcount ≤ sum of round popcounts

---

## Integration Patterns

### Published Language

The Syndrome Processing context publishes a well-defined language consumed by Coherence Gate:

```rust
// The contract between Syndrome Processing and Coherence Gate
mod syndrome_events {
    pub struct SyndromeEvent { /* ... */ }
    pub struct GraphDelta { /* ... */ }
    pub struct SyndromeStatistics { /* ... */ }
}
```

### Conformist Pattern

Syndrome Processing conforms to Coherence Gate's needs:

- Event format defined by consumer
- Latency requirements set by consumer
- Graph delta structure matches gate's graph model

### Anticorruption Layer (ACL)

Between Hardware Interface and Syndrome Processing:

```rust
impl HardwareAcl {
    /// Translate hardware-specific format to domain model
    fn translate(&self, raw: HardwarePacket) -> Result<SyndromeRound, AclError> {
        SyndromeRound {
            round_id: self.extract_round_id(raw),
            cycle: self.extract_cycle(raw),
            timestamp: self.normalize_timestamp(raw),
            detectors: self.unpack_detectors(raw),
            source_tile: self.identify_tile(raw),
        }
    }
}
```

---

## Performance Considerations

### Throughput Requirements

| Metric | Target | Rationale |
|--------|--------|-----------|
| Ingestion rate | 1M rounds/sec | 1 MHz syndrome rate |
| Buffer depth | 1024 rounds | 1ms history at 1MHz |
| Transform latency | ≤ 500ns | Leave margin for gate |
| Memory per tile | 16 KB | Fits in FPGA BRAM |

### Optimization Strategies

1. **SIMD for bitmap operations**: Use AVX2/NEON for XOR, popcount
2. **Zero-copy ring buffer**: Avoid allocation on hot path
3. **Incremental correlation**: Update only changed pairs
4. **Lazy clustering**: Only cluster when activity exceeds threshold

---

## References

- DDD-001: Coherence Gate Domain Model
- ADR-001: ruQu Architecture
- Stim: Quantum Error Correction Simulator
- Google Cirq: Detector Annotation Format
