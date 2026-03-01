# Temporal Tensor Store: Domain-Driven Design Architecture

**Version**: 0.1
**Date**: 2026-02-08
**Status**: Draft
**Parent ADRs**: ADR-017, ADR-018, ADR-019, ADR-020, ADR-021, ADR-022, ADR-023

---

## Strategic Design

### Domain Vision

The Temporal Tensor Store unifies caching, compression, and eviction into a single primitive. Each tensor chunk has an access history. Access history drives tier choice. Tier choice drives quantization bits and whether data stays materialized, stays compressed, or becomes reconstructable only via factors or deltas.

> **This is not a cache.** The system answers: "At what fidelity should this block exist right now?" not "Is this block present?"

The fundamental insight is that tensors in agent workloads exhibit temporal locality: most frames reuse the same value distribution, and access frequency decays predictably. By treating quantization tier as a continuous lifecycle state rather than a static configuration, the store compresses data in proportion to its staleness while guaranteeing bounded reconstruction error at every tier.

### Core Domain

**Tensor Lifecycle Management** -- The heart of the system. Manages the full lifecycle of tensor blocks from creation through tiered compression to eviction (compression to zero). Every block transitions through a state machine: Created -> Hot -> Warm -> Cold -> Evicted. The transition function is driven by a composite access score and bounded by configurable hysteresis to prevent oscillation.

### Supporting Domains

1. **Quantization Domain** -- Bit-packing, scale computation, encode/decode. Owns the mathematical transforms that convert between f32 values and packed bitstream representations at arbitrary bit widths (3, 5, 7, 8). Manages groupwise symmetric quantization with f16 scales.

2. **Scoring & Migration Domain** -- Access tracking, score computation, tier decisions. Owns the temporal access profile for each block and the policy that maps scores to tiers. Responsible for maintenance scheduling and budgeted tick processing.

3. **Storage Domain** -- Block IO, metadata persistence, checksums. Owns the physical layout of tier data files, the metadata log for crash recovery, and the in-memory index structures for fast lookup.

### Generic Domains

1. **Clock/Time** -- Tick-based time progression. Provides a monotonic tick counter that all scoring and maintenance operations reference. Decoupled from wall-clock time for deterministic replay.

2. **Metrics/Witness** -- Audit logging, decision witnesses. Records every tiering decision with sufficient context to reconstruct the reasoning (score at time of decision, thresholds applied, resulting tier). Enables post-hoc analysis without affecting hot-path performance.

3. **Configuration** -- Policy management. Versioned, immutable policy bundles that define thresholds, group sizes, drift tolerances, and tier-to-bit mappings. Policy changes create new bundles; active bundles cannot be modified.

---

## Ubiquitous Language

| Term | Definition |
|------|------------|
| **Block** | Fixed-size chunk of a tensor (16KB/32KB), the atomic unit of storage and tiering |
| **Tier** | Quantization level: Hot (8-bit), Warm (7/5-bit), Cold (3-bit), Absent (0-bit/evicted) |
| **Touch** | Record an access event on a block, incrementing its access count and updating its timestamp |
| **Score** | Composite metric combining EMA, popcount, and recency: `access_count * 1024 / (age + 1)` |
| **Drift** | When a tensor's value distribution changes beyond the scale tolerance, forcing a new segment |
| **Eviction** | Compression to zero bits; only metadata survives. Data is reconstructable via deltas or factors |
| **Reconstruction** | Rebuilding evicted data from delta chains or low-rank factor sets |
| **Compaction** | Collapsing a delta chain into a new base block to bound chain length |
| **Witness** | Audit log entry recording a tiering or eviction decision with full context |
| **Tick** | Time quantum for maintenance budget processing; one tick = one unit of the logical clock |
| **Segment** | Multi-frame compressed blob sharing quantization scales; the on-disk unit for temporal data |
| **Group** | Contiguous slice of tensor elements sharing one quantization scale (default: 64 elements) |
| **Scale** | f16 value representing `max(|v_i|) / qmax` for a group; shared across all frames in a segment |
| **qmax** | Maximum quantized integer for a bit width: `2^(bits-1) - 1` (127, 63, 15, 3 for 8/7/5/3-bit) |
| **Frame** | One tensor snapshot at a point in time; the input unit for temporal compression |

---

## Bounded Contexts

### Bounded Context Map

```
+============================================================================+
|                     TEMPORAL TENSOR STORE                                    |
+============================================================================+
|                                                                              |
|  +--------------------+       +---------------------+                        |
|  | BC1: BLOCK         |       | BC2: QUANTIZATION   |                        |
|  | MANAGEMENT         |<----->| CONTEXT             |                        |
|  |                    |Shared | (codec_bits, quant)  |                        |
|  | - TensorBlock      |Kernel |                     |                        |
|  | - BlockMeta        |       | - QuantizationCodec |                        |
|  | - State machine    |       | - BitPacking        |                        |
|  | - Lifecycle        |       | - f16 conversion    |                        |
|  +--------+-----------+       +----------+----------+                        |
|           |                              |                                   |
|           | Shared                       | Shared                            |
|           | Kernel                       | Kernel                            |
|           v                              |                                   |
|  +--------------------+                  |                                   |
|  | BC3: TEMPORAL       |<----------------+                                   |
|  | SCORING CONTEXT     |                                                     |
|  |                     |                                                     |
|  | - AccessProfile     |                                                     |
|  | - TierPolicy        |                                                     |
|  | - Maintenance       |                                                     |
|  +--------+------------+                                                     |
|           |                                                                  |
|           | Customer/Supplier                                                |
|           v                                                                  |
|  +--------------------+       +---------------------+                        |
|  | BC4: STORAGE        |       | BC5: DELTA &        |                        |
|  | ENGINE CONTEXT      |<----->| RECONSTRUCTION      |                        |
|  |                     |Cust/  | CONTEXT             |                        |
|  | - TieredStore       |Suppl  |                     |                        |
|  | - BlockIO           |       | - DeltaChain        |                        |
|  | - MetaLog           |       | - FactorSet         |                        |
|  | - Index             |       | - Reconstruction    |                        |
|  +--------------------+       +---------------------+                        |
|                                                                              |
+============================================================================+

Integration Patterns:
  <----->  Shared Kernel (shared types, co-owned)
  ------> Customer/Supplier (downstream consumes upstream API)
  ======> Published Language (stable, versioned contract)
```

### Event Flow Diagram

```
  External Write                  Timer Tick
       |                              |
       v                              v
  +----------+                 +-------------+
  | BC1:     |  BlockAccessed  | BC3:        |
  | Block    |---------------->| Temporal    |
  | Mgmt     |                 | Scoring     |
  +----+-----+                 +------+------+
       |                              |
       | BlockCreated                 | TierUpgradeRequested
       | BlockTierChanged             | TierDowngradeRequested
       v                              v
  +----------+                 +-------------+
  | BC2:     |  quantize()     | BC3:        |
  | Quant    |<----------------| choose_tier |
  | Context  |                 +------+------+
  +----+-----+                        |
       |                              | MaintenanceCompleted
       | packed bytes                 v
       v                       +-------------+
  +----------+                 | BC4:        |
  | BC4:     |  BlockWritten   | Storage     |
  | Storage  |<----------------| Engine      |
  | Engine   |                 +------+------+
  +----+-----+                        |
       |                              | BlockEvicted
       | BlockDeleted                 v
       v                       +-------------+
  +----------+                 | BC5:        |
  | BC5:     |  DeltaAppended  | Delta &     |
  | Delta &  |<----------------| Recon       |
  | Recon    |                 +-------------+
  +----------+
```

---

## Bounded Context 1: Block Management Context

### Purpose

Responsible for tensor block lifecycle: creation, chunking, metadata management, identity. This is the aggregate that owns the block state machine and enforces the invariant that blocks transition through tiers in a well-defined order.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **TensorBlock** | Aggregate root owning a block's identity, metadata, and state |
| **BlockKey** | Composite identity: (tensor_id: u128, block_index: u32) |
| **BlockMeta** | All metadata for a block: tier, checksums, timestamps, access stats |
| **TensorIdentity** | The parent tensor: id, shape, dtype, lineage parent |
| **BlockData** | The raw quantized bytes for a block at its current tier |

### Aggregates

#### TensorBlock (Aggregate Root)

```rust
/// The primary aggregate root for the Block Management context.
/// Owns the full lifecycle of a tensor block from creation through eviction.
///
/// Invariants:
///   - block_bytes must match configured block size
///   - checksum must be valid (CRC32 of quantized data)
///   - state transitions follow: Created -> Hot -> Warm -> Cold -> Evicted
///   - tier can only degrade by one step per maintenance tick (hysteresis)
///   - block_key is immutable after creation
pub struct TensorBlock {
    /// Composite identity: (tensor_id, block_index)
    key: BlockKey,
    /// All metadata fields
    meta: BlockMeta,
    /// Current quantized data (None if evicted)
    data: Option<BlockData>,
    /// Reference to parent tensor identity
    tensor_identity: TensorIdentity,
    /// Domain events pending publication
    pending_events: Vec<BlockDomainEvent>,
}

impl TensorBlock {
    /// Create a new block from raw f32 data.
    /// Initial tier is determined by the current access profile.
    pub fn create(
        key: BlockKey,
        identity: TensorIdentity,
        raw_data: &[f32],
        initial_tier: Tier,
        now_tick: u64,
    ) -> Result<Self, BlockError> {
        let data = BlockData::from_raw(raw_data, initial_tier)?;
        let checksum = Checksum::compute(&data.bytes);

        let meta = BlockMeta {
            tier: initial_tier,
            checksum,
            created_at: now_tick,
            last_accessed_at: now_tick,
            last_tier_change_at: now_tick,
            access_count: 0,
            byte_size: data.bytes.len() as u32,
            reconstruct_policy: ReconstructPolicy::None,
        };

        let mut block = Self {
            key,
            meta,
            data: Some(data),
            tensor_identity: identity,
            pending_events: Vec::new(),
        };

        block.pending_events.push(BlockDomainEvent::BlockCreated {
            key,
            tier: initial_tier,
            tick: now_tick,
        });

        Ok(block)
    }

    /// Record an access. Updates count and timestamp.
    pub fn touch(&mut self, now_tick: u64) {
        self.meta.access_count = self.meta.access_count.wrapping_add(1);
        self.meta.last_accessed_at = now_tick;
        self.pending_events.push(BlockDomainEvent::BlockAccessed {
            key: self.key,
            tick: now_tick,
        });
    }

    /// Transition to a new tier. Enforces hysteresis invariant.
    pub fn change_tier(
        &mut self,
        new_tier: Tier,
        new_data: Option<BlockData>,
        now_tick: u64,
    ) -> Result<(), BlockError> {
        if new_tier == self.meta.tier {
            return Ok(());
        }

        let old_tier = self.meta.tier;
        self.meta.tier = new_tier;
        self.meta.last_tier_change_at = now_tick;
        self.data = new_data;

        if new_tier == Tier::Absent {
            self.meta.reconstruct_policy = ReconstructPolicy::DeltaChain;
        }

        self.pending_events.push(BlockDomainEvent::BlockTierChanged {
            key: self.key,
            old_tier,
            new_tier,
            tick: now_tick,
        });

        Ok(())
    }

    /// Evict the block: data is dropped, metadata retained.
    pub fn evict(&mut self, now_tick: u64) -> Result<(), BlockError> {
        if self.meta.tier == Tier::Absent {
            return Err(BlockError::AlreadyEvicted);
        }

        self.pending_events.push(BlockDomainEvent::BlockEvicted {
            key: self.key,
            previous_tier: self.meta.tier,
            tick: now_tick,
        });

        self.meta.tier = Tier::Absent;
        self.meta.last_tier_change_at = now_tick;
        self.meta.reconstruct_policy = ReconstructPolicy::DeltaChain;
        self.data = None;

        Ok(())
    }

    /// Verify data integrity via checksum.
    pub fn verify_checksum(&self) -> bool {
        match &self.data {
            Some(data) => Checksum::compute(&data.bytes) == self.meta.checksum,
            None => true, // Evicted blocks have no data to verify
        }
    }

    /// Drain pending domain events for publication.
    pub fn take_events(&mut self) -> Vec<BlockDomainEvent> {
        std::mem::take(&mut self.pending_events)
    }
}
```

### Entities

```rust
/// Identity of the parent tensor that this block belongs to.
pub struct TensorIdentity {
    /// Unique tensor identifier (128-bit UUID)
    pub id: u128,
    /// Shape of the full tensor (e.g., [1024, 768])
    pub shape: Shape,
    /// Data type of the original tensor
    pub dtype: DType,
    /// Optional lineage parent (for delta chains)
    pub lineage_parent: Option<u128>,
}

/// Raw quantized bytes for a block at a specific tier.
pub struct BlockData {
    /// Packed quantized bytes
    pub bytes: Vec<u8>,
    /// Tier at which this data was quantized
    pub quantized_at_tier: Tier,
}

impl BlockData {
    pub fn from_raw(data: &[f32], tier: Tier) -> Result<Self, BlockError> {
        // Delegate to QuantizationCodec for encoding
        let bytes = Vec::new(); // placeholder: actual encoding via BC2
        Ok(Self {
            bytes,
            quantized_at_tier: tier,
        })
    }
}
```

### Value Objects

```rust
/// Composite block identity. Immutable after creation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BlockKey {
    pub tensor_id: u128,
    pub block_index: u32,
}

/// Quantization tier determining bit width and compression level.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Tier {
    /// 8-bit quantization, ~4.0x compression
    Hot = 0,
    /// 7-bit or 5-bit quantization, ~4.57x or ~6.4x compression
    Warm = 1,
    /// 3-bit quantization, ~10.67x compression
    Cold = 2,
    /// Evicted: 0 bits, metadata only, reconstructable via deltas/factors
    Absent = 3,
}

/// Element data type of the original tensor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DType {
    F32,
    F16,
    BF16,
    I8,
}

/// Policy for reconstructing evicted block data.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReconstructPolicy {
    /// No reconstruction available (data loss accepted)
    None,
    /// Reconstruct from delta chain (base block + deltas)
    DeltaChain,
    /// Reconstruct from low-rank factors (U * S * V^T)
    LowRankFactors,
    /// Reconstruct from both deltas and factors (best-effort)
    Hybrid,
}

/// Tensor shape descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape(pub Vec<u32>);

/// CRC32 checksum for data integrity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Checksum(pub u32);

impl Checksum {
    pub fn compute(data: &[u8]) -> Self {
        let mut crc: u32 = 0xFFFF_FFFF;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB8_8320;
                } else {
                    crc >>= 1;
                }
            }
        }
        Self(!crc)
    }
}
```

### Domain Events

| Event | Trigger | Payload | Consumers |
|-------|---------|---------|-----------|
| `BlockCreated` | New block materialized | key, tier, tick | Storage Engine, Scoring |
| `BlockAccessed` | Touch on a block | key, tick | Temporal Scoring |
| `BlockTierChanged` | Tier transition | key, old_tier, new_tier, tick | Storage, Metrics |
| `BlockEvicted` | Block compressed to zero | key, previous_tier, tick | Delta & Reconstruction |
| `BlockCorrupted` | Checksum mismatch | key, expected, actual | Alerting, Recovery |
| `BlockCompacted` | Delta chain collapsed | key, new_base_tier, tick | Storage Engine |

```rust
#[derive(Clone, Debug)]
pub enum BlockDomainEvent {
    BlockCreated { key: BlockKey, tier: Tier, tick: u64 },
    BlockAccessed { key: BlockKey, tick: u64 },
    BlockTierChanged { key: BlockKey, old_tier: Tier, new_tier: Tier, tick: u64 },
    BlockEvicted { key: BlockKey, previous_tier: Tier, tick: u64 },
    BlockCorrupted { key: BlockKey, expected: Checksum, actual: Checksum },
    BlockCompacted { key: BlockKey, new_base_tier: Tier, tick: u64 },
}
```

---

## Bounded Context 2: Quantization Context

### Purpose

Responsible for all encoding/decoding operations across bit widths. Owns the groupwise symmetric quantization algorithm, f16 scale management, and bitstream packing. This context is a **shared kernel** with Block Management: both contexts reference the same quantization types, but the Quantization Context owns the encode/decode logic.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **QuantizationCodec** | Aggregate root encapsulating format selection and parameters |
| **QuantParams** | Value object: bits, scale, zero_point (always 0 for symmetric), group_len |
| **PackedBlock** | Value object: encoded bytes with format metadata |
| **GroupScale** | f16 scale for a group: `max(abs(v_i)) / qmax` |

### Aggregates

#### QuantizationCodec (Aggregate Root)

```rust
/// Encapsulates groupwise symmetric quantization for all supported bit widths.
///
/// Invariants:
///   - bits must be one of {3, 5, 7, 8}
///   - group_len must be >= 1
///   - scales are stored as f16 (u16 bit pattern) to minimize metadata overhead
///   - qmax = 2^(bits-1) - 1
pub struct QuantizationCodec {
    /// Bit width for quantization
    bits: u8,
    /// Elements per quantization group
    group_len: usize,
    /// Cached qmax value
    qmax: i32,
}

impl QuantizationCodec {
    pub fn new(bits: u8, group_len: usize) -> Self {
        let qmax = qmax_from_bits(bits);
        Self { bits, group_len, qmax }
    }

    /// Quantize f32 values to packed bytes with f16 group scales.
    ///
    /// Returns (scales_f16, packed_bytes).
    pub fn quantize(&self, values: &[f32]) -> (Vec<u16>, Vec<u8>) {
        let scales = compute_scales(values, self.group_len, self.bits);
        let scales_f32 = scales_to_f32(&scales);
        let mut packed = Vec::new();
        quantize_and_pack_f32(values, &scales_f32, self.group_len, self.bits, &mut packed);
        (scales, packed)
    }

    /// Dequantize packed bytes back to f32 values.
    pub fn dequantize(
        &self,
        packed: &[u8],
        scales_f16: &[u16],
        tensor_len: usize,
        frame_count: usize,
    ) -> Vec<f32> {
        let scales_f32 = scales_to_f32(scales_f16);
        let mut out = Vec::new();
        dequantize_f32(
            packed, &scales_f32, self.group_len,
            self.bits, tensor_len, frame_count, &mut out,
        );
        out
    }

    /// Check if a frame fits within existing scales (drift tolerance).
    pub fn frame_fits_scales(
        &self,
        frame: &[f32],
        scales_f32: &[f32],
        drift_factor: f32,
    ) -> bool {
        frame_fits_scales_f32(frame, scales_f32, self.group_len, self.bits, drift_factor)
    }
}

/// Compute qmax for a given bit width: 2^(bits-1) - 1.
/// Returns 0 for invalid bit widths (0 or >8).
#[inline]
pub fn qmax_from_bits(bits: u8) -> i32 {
    if bits == 0 || bits > 8 { return 0; }
    (1i32 << (bits - 1)) - 1
}
```

### Value Objects

```rust
/// Quantization parameters for a single encoding operation.
#[derive(Clone, Debug, PartialEq)]
pub struct QuantParams {
    /// Bit width (3, 5, 7, or 8)
    pub bits: u8,
    /// f16-encoded group scales (one per group)
    pub scales_f16: Vec<u16>,
    /// Cached f32 conversion of scales (for hot-path use)
    pub scales_f32: Vec<f32>,
    /// Elements per group
    pub group_len: usize,
}

/// Packed quantized block with format metadata.
#[derive(Clone, Debug)]
pub struct PackedBlock {
    /// Packed bitstream bytes
    pub bytes: Vec<u8>,
    /// Quantization parameters used
    pub params: QuantParams,
    /// Number of frames encoded
    pub frame_count: u32,
    /// Number of f32 elements per frame
    pub tensor_len: u32,
}

/// Two-level scale for hierarchical quantization (future extension).
#[derive(Clone, Debug, PartialEq)]
pub struct TwoLevelScale {
    pub primary_scale: f32,
    pub secondary_scale: f32,
    pub flags: u8,
}
```

### Domain Services

```rust
/// Service orchestrating encode/decode for all quantization formats.
pub struct QuantizationService {
    /// Codec instances keyed by bit width
    codecs: [QuantizationCodec; 4], // indices 0-3 for bits 3,5,7,8
}

impl QuantizationService {
    pub fn new(group_len: usize) -> Self {
        Self {
            codecs: [
                QuantizationCodec::new(3, group_len),
                QuantizationCodec::new(5, group_len),
                QuantizationCodec::new(7, group_len),
                QuantizationCodec::new(8, group_len),
            ],
        }
    }

    pub fn codec_for_tier(&self, tier: Tier) -> &QuantizationCodec {
        match tier {
            Tier::Hot => &self.codecs[3],    // 8-bit
            Tier::Warm => &self.codecs[2],   // 7-bit (configurable to 5-bit)
            Tier::Cold => &self.codecs[0],   // 3-bit
            Tier::Absent => &self.codecs[0], // N/A but provide fallback
        }
    }
}

/// Service for packing and unpacking arbitrary-width bit codes.
pub struct BitPackingService;

impl BitPackingService {
    /// Pack unsigned codes of `bits` width into a byte stream.
    /// Uses a 64-bit accumulator with no alignment padding.
    pub fn pack(codes: &[u32], bits: u32, out: &mut Vec<u8>) {
        let mut acc: u64 = 0;
        let mut acc_bits: u32 = 0;
        for &code in codes {
            acc |= (code as u64) << acc_bits;
            acc_bits += bits;
            while acc_bits >= 8 {
                out.push((acc & 0xFF) as u8);
                acc >>= 8;
                acc_bits -= 8;
            }
        }
        if acc_bits > 0 {
            out.push((acc & 0xFF) as u8);
        }
    }

    /// Unpack `count` unsigned codes of `bits` width from a byte stream.
    pub fn unpack(data: &[u8], bits: u32, count: usize, out: &mut Vec<u32>) {
        let mask = (1u64 << bits) - 1;
        let mut acc: u64 = 0;
        let mut acc_bits: u32 = 0;
        let mut byte_idx = 0usize;
        let mut decoded = 0usize;
        while decoded < count {
            while acc_bits < bits && byte_idx < data.len() {
                acc |= (data[byte_idx] as u64) << acc_bits;
                acc_bits += 8;
                byte_idx += 1;
            }
            if acc_bits < bits { break; }
            out.push((acc & mask) as u32);
            acc >>= bits;
            acc_bits -= bits;
            decoded += 1;
        }
    }
}
```

---

## Bounded Context 3: Temporal Scoring Context

### Purpose

Responsible for access tracking, score computation, tier selection, and hysteresis. Owns the per-block access profile and the policy that determines when blocks migrate between tiers. This context is a **shared kernel** with Block Management: the scoring context produces tier recommendations that Block Management consumes.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **AccessProfile** | Aggregate root tracking per-block access history |
| **Score** | Composite metric: `access_count * 1024 / (age + 1)` |
| **AccessWindow** | u64 bitset representing access pattern over recent ticks |
| **EMARate** | Exponential moving average decay rate for smoothed scoring |
| **TierPolicy** | Configurable thresholds mapping scores to tiers |

### Aggregates

#### AccessProfile (Aggregate Root)

```rust
/// Tracks per-block access history and computes tiering decisions.
///
/// Invariants:
///   - access_count is monotonically non-decreasing
///   - last_access_at <= current tick
///   - tier_age tracks ticks since last tier change (hysteresis input)
///   - ema_rate is in (0.0, 1.0]
pub struct AccessProfile {
    /// Block this profile tracks
    key: BlockKey,
    /// Exponential moving average decay rate
    ema_rate: f32,
    /// Sliding window bitset: bit i = access in tick (now - i)
    window: u64,
    /// Total access count (wrapping)
    access_count: u32,
    /// Tick of last access
    last_access_at: u64,
    /// Ticks since last tier change (for hysteresis)
    tier_age: u64,
    /// Current tier as determined by last scoring
    current_tier: Tier,
    /// Pending domain events
    pending_events: Vec<ScoringDomainEvent>,
}

impl AccessProfile {
    pub fn new(key: BlockKey, initial_tier: Tier, now_tick: u64) -> Self {
        Self {
            key,
            ema_rate: 0.9,
            window: 0,
            access_count: 0,
            last_access_at: now_tick,
            tier_age: 0,
            current_tier: initial_tier,
            pending_events: Vec::new(),
        }
    }

    /// Record an access event. Shifts the window and sets the current bit.
    pub fn touch(&mut self, now_tick: u64) {
        let elapsed = now_tick.saturating_sub(self.last_access_at);
        if elapsed > 0 {
            self.window = self.window.checked_shl(elapsed as u32).unwrap_or(0);
        }
        self.window |= 1;
        self.access_count = self.access_count.wrapping_add(1);
        self.last_access_at = now_tick;

        self.pending_events.push(ScoringDomainEvent::AccessRecorded {
            key: self.key,
            tick: now_tick,
        });
    }

    /// Compute the composite access score.
    pub fn compute_score(&self, now_tick: u64) -> f32 {
        let age = now_tick.saturating_sub(self.last_access_at) + 1;
        let popcount = self.window.count_ones() as f32;
        let recency = self.access_count as f32 * 1024.0 / age as f32;
        let ema_weight = self.ema_rate;

        // Composite: weighted combination of popcount and recency
        ema_weight * recency + (1.0 - ema_weight) * popcount * 64.0
    }

    /// Determine the recommended tier based on current score.
    pub fn choose_tier(&mut self, now_tick: u64, policy: &TierPolicy) -> Tier {
        let score = self.compute_score(now_tick);
        let score_u32 = score as u32;

        let recommended = if score_u32 >= policy.hot_min_score {
            Tier::Hot
        } else if score_u32 >= policy.warm_min_score {
            Tier::Warm
        } else {
            Tier::Cold
        };

        if recommended != self.current_tier {
            let old = self.current_tier;
            self.current_tier = recommended;
            self.tier_age = 0;

            let event = if recommended > old {
                ScoringDomainEvent::TierDowngradeRequested {
                    key: self.key,
                    from: old,
                    to: recommended,
                    score,
                    tick: now_tick,
                }
            } else {
                ScoringDomainEvent::TierUpgradeRequested {
                    key: self.key,
                    from: old,
                    to: recommended,
                    score,
                    tick: now_tick,
                }
            };
            self.pending_events.push(event);
        } else {
            self.tier_age += 1;
        }

        recommended
    }

    pub fn take_events(&mut self) -> Vec<ScoringDomainEvent> {
        std::mem::take(&mut self.pending_events)
    }
}
```

#### TierPolicy (Value Object, from implementation)

```rust
/// Configurable scoring weights and thresholds for tier selection.
/// Directly corresponds to the TierPolicy struct in tier_policy.rs.
///
/// Score = access_count * 1024 / (now_ts - last_access_ts + 1)
///
/// | Tier | Condition                 | Bits |
/// |------|---------------------------|------|
/// | Hot  | score >= hot_min_score    | 8    |
/// | Warm | score >= warm_min_score   | warm_bits (7 or 5) |
/// | Cold | otherwise                | 3    |
#[derive(Clone, Copy, Debug)]
pub struct TierPolicy {
    pub hot_min_score: u32,
    pub warm_min_score: u32,
    pub warm_bits: u8,
    /// Drift tolerance as Q8 fixed-point. 26 means ~10.2% (26/256).
    pub drift_pct_q8: u32,
    pub group_len: u32,
}

impl Default for TierPolicy {
    fn default() -> Self {
        Self {
            hot_min_score: 512,
            warm_min_score: 64,
            warm_bits: 7,
            drift_pct_q8: 26,
            group_len: 64,
        }
    }
}

impl TierPolicy {
    /// Select bit width based on access pattern.
    pub fn select_bits(&self, access_count: u32, last_access_ts: u32, now_ts: u32) -> u8 {
        let age = now_ts.wrapping_sub(last_access_ts).wrapping_add(1);
        let score = access_count.saturating_mul(1024).wrapping_div(age);
        if score >= self.hot_min_score {
            8
        } else if score >= self.warm_min_score {
            self.warm_bits
        } else {
            3
        }
    }

    /// Drift factor: 1.0 + drift_pct_q8/256
    pub fn drift_factor(&self) -> f32 {
        1.0 + (self.drift_pct_q8 as f32) / 256.0
    }
}
```

### Domain Services

```rust
/// Budgeted tick processing: processes a limited number of blocks per tick
/// to avoid latency spikes during maintenance windows.
pub struct MaintenanceScheduler {
    /// Maximum blocks to process per tick
    budget_per_tick: usize,
    /// Round-robin cursor into the block list
    cursor: usize,
    /// Tick counter
    current_tick: u64,
}

impl MaintenanceScheduler {
    pub fn new(budget_per_tick: usize) -> Self {
        Self { budget_per_tick, cursor: 0, current_tick: 0 }
    }

    /// Process one maintenance tick. Returns the set of tier-change recommendations.
    pub fn tick(
        &mut self,
        profiles: &mut [AccessProfile],
        policy: &TierPolicy,
    ) -> Vec<ScoringDomainEvent> {
        self.current_tick += 1;
        let mut events = Vec::new();
        let n = profiles.len().min(self.budget_per_tick);

        for _ in 0..n {
            if self.cursor >= profiles.len() {
                self.cursor = 0;
            }
            let profile = &mut profiles[self.cursor];
            profile.choose_tier(self.current_tick, policy);
            events.extend(profile.take_events());
            self.cursor += 1;
        }

        events.push(ScoringDomainEvent::MaintenanceCompleted {
            tick: self.current_tick,
            blocks_processed: n as u32,
        });

        events
    }
}
```

### Domain Events

| Event | Trigger | Consumers |
|-------|---------|-----------|
| `AccessRecorded` | Block touched | Score recomputation |
| `ScoreComputed` | Periodic scoring pass | Tier decision |
| `TierUpgradeRequested` | Score crossed upward threshold | Block Management |
| `TierDowngradeRequested` | Score dropped below threshold | Block Management |
| `MaintenanceCompleted` | Tick budget exhausted | Metrics |

```rust
#[derive(Clone, Debug)]
pub enum ScoringDomainEvent {
    AccessRecorded { key: BlockKey, tick: u64 },
    ScoreComputed { key: BlockKey, score: f32, tick: u64 },
    TierUpgradeRequested { key: BlockKey, from: Tier, to: Tier, score: f32, tick: u64 },
    TierDowngradeRequested { key: BlockKey, from: Tier, to: Tier, score: f32, tick: u64 },
    MaintenanceCompleted { tick: u64, blocks_processed: u32 },
}
```

---

## Bounded Context 4: Storage Engine Context

### Purpose

Responsible for persistent block IO, metadata logging, and index management. Owns the physical layout of tier data, the append-only metadata log for crash recovery, and the in-memory index structures (HashMap + per-tier candidate lists + min-heap for eviction).

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **TieredStore** | Aggregate root managing all storage operations |
| **BlockIO** | Trait for reading/writing block data to tier-specific storage |
| **MetaLog** | Append-only log of metadata records for crash recovery |
| **StoreLayout** | Directory paths per tenant/collection |

### Aggregates

#### TieredStore (Aggregate Root)

```rust
/// Manages tier data files, metadata log, and in-memory index.
///
/// Invariants:
///   - Every block in the index has a valid metadata record in the log
///   - Per-tier candidate lists are consistent with the index
///   - Eviction candidates are ordered by score (min-heap)
///   - Checksums are verified on read (configurable)
pub struct TieredStore {
    /// Primary index: BlockKey -> BlockMeta
    index: HashMap<BlockKey, BlockMeta>,
    /// Per-tier candidate lists for migration scanning
    tier_lists: [Vec<BlockKey>; 4], // Hot, Warm, Cold, Absent
    /// Min-heap for eviction candidates (sorted by score ascending)
    eviction_heap: BinaryHeap<Reverse<ScoredBlock>>,
    /// Block IO backend (trait object for testability)
    io: Box<dyn BlockIO>,
    /// Metadata log for crash recovery
    meta_log: Box<dyn MetaLog>,
    /// Clock source
    clock: Box<dyn Clock>,
    /// Pending domain events
    pending_events: Vec<StorageDomainEvent>,
}

impl TieredStore {
    /// Write a block to its tier. Updates index and meta log atomically.
    pub fn write_block(
        &mut self,
        key: BlockKey,
        tier: Tier,
        data: &[u8],
        meta: BlockMeta,
    ) -> Result<(), StoreErr> {
        self.io.write_block(tier, key, data)?;
        self.meta_log.append(MetaRecord::Write { key, tier, meta: meta.clone() })?;
        self.index.insert(key, meta);
        self.tier_lists[tier as usize].push(key);

        self.pending_events.push(StorageDomainEvent::BlockWritten {
            key, tier, byte_count: data.len() as u32,
        });

        Ok(())
    }

    /// Read a block from its tier. Optionally verifies checksum.
    pub fn read_block(
        &self,
        key: BlockKey,
        verify_checksum: bool,
    ) -> Result<Vec<u8>, StoreErr> {
        let meta = self.index.get(&key)
            .ok_or(StoreErr::NotFound(key))?;

        let mut buf = vec![0u8; meta.byte_size as usize];
        let n = self.io.read_block(meta.tier, key, &mut buf)?;
        buf.truncate(n);

        if verify_checksum {
            let actual = Checksum::compute(&buf);
            if actual != meta.checksum {
                return Err(StoreErr::ChecksumMismatch { key, expected: meta.checksum, actual });
            }
        }

        Ok(buf)
    }

    /// Delete a block from storage. Metadata is retained in the log.
    pub fn delete_block(&mut self, key: BlockKey) -> Result<(), StoreErr> {
        let meta = self.index.get(&key)
            .ok_or(StoreErr::NotFound(key))?;
        let tier = meta.tier;

        self.io.delete_block(tier, key)?;
        self.meta_log.append(MetaRecord::Delete { key, tier })?;
        self.index.remove(&key);

        self.pending_events.push(StorageDomainEvent::BlockDeleted { key, tier });

        Ok(())
    }

    /// Rebuild index from metadata log (crash recovery).
    pub fn rebuild_index(&mut self) -> Result<u64, StoreErr> {
        self.index.clear();
        for list in &mut self.tier_lists {
            list.clear();
        }

        let mut count = 0u64;
        // Replay meta log to reconstruct index
        // (implementation depends on MetaLog backend)
        self.pending_events.push(StorageDomainEvent::IndexRebuilt { entries: count });

        Ok(count)
    }
}
```

### Repository Interfaces (Traits)

```rust
/// Block-level IO operations. Implemented by filesystem, memory, or AgentDB backends.
pub trait BlockIO {
    fn read_block(&self, tier: Tier, key: BlockKey, dst: &mut [u8]) -> Result<usize, StoreErr>;
    fn write_block(&mut self, tier: Tier, key: BlockKey, src: &[u8]) -> Result<(), StoreErr>;
    fn delete_block(&mut self, tier: Tier, key: BlockKey) -> Result<(), StoreErr>;
}

/// Append-only metadata log for crash recovery and audit.
pub trait MetaLog {
    fn append(&mut self, rec: MetaRecord) -> Result<(), StoreErr>;
    fn get(&self, key: BlockKey) -> Option<BlockMeta>;
    fn iter(&self) -> Box<dyn Iterator<Item = MetaRecord> + '_>;
}

/// Clock abstraction for deterministic testing and replay.
pub trait Clock {
    fn now_ticks(&self) -> u64;
}
```

### Value Objects

```rust
/// Physical storage layout per tenant/collection.
#[derive(Clone, Debug)]
pub struct StoreLayout {
    pub hot_dir: String,
    pub warm_dir: String,
    pub cold_dir: String,
    pub meta_log_path: String,
}

/// Metadata record for the append-only log.
#[derive(Clone, Debug)]
pub enum MetaRecord {
    Write { key: BlockKey, tier: Tier, meta: BlockMeta },
    Delete { key: BlockKey, tier: Tier },
    TierChange { key: BlockKey, old_tier: Tier, new_tier: Tier },
}

/// Block metadata (all non-data fields).
#[derive(Clone, Debug)]
pub struct BlockMeta {
    pub tier: Tier,
    pub checksum: Checksum,
    pub created_at: u64,
    pub last_accessed_at: u64,
    pub last_tier_change_at: u64,
    pub access_count: u32,
    pub byte_size: u32,
    pub reconstruct_policy: ReconstructPolicy,
}
```

### Domain Events

| Event | Trigger | Consumers |
|-------|---------|-----------|
| `BlockWritten` | Block stored to tier | Metrics |
| `BlockRead` | Block retrieved from tier | Metrics, Scoring (touch) |
| `BlockDeleted` | Block removed from storage | Index cleanup |
| `MetaLogAppended` | New record in meta log | Crash recovery |
| `IndexRebuilt` | Index reconstructed from log | Startup, Recovery |
| `ChecksumFailed` | CRC mismatch on read | Alerting, Block Management |

```rust
#[derive(Clone, Debug)]
pub enum StorageDomainEvent {
    BlockWritten { key: BlockKey, tier: Tier, byte_count: u32 },
    BlockRead { key: BlockKey, tier: Tier },
    BlockDeleted { key: BlockKey, tier: Tier },
    MetaLogAppended { record_type: &'static str },
    IndexRebuilt { entries: u64 },
    ChecksumFailed { key: BlockKey, expected: Checksum, actual: Checksum },
}
```

---

## Bounded Context 5: Delta & Reconstruction Context

### Purpose

Responsible for delta writes, delta chain management, factor storage, and reconstruction. When a block is evicted (Tier::Absent), it becomes reconstructable via a delta chain (base block + ordered deltas) or low-rank factor sets (U, S, V matrices). This context owns the chain length invariant and the compaction operation that collapses long chains.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **DeltaChain** | Aggregate root: base block reference + ordered list of deltas |
| **DeltaRecord** | Sparse vector: pairs of (index, quantized value) with delta_scale |
| **FactorSet** | Low-rank matrices (U, S, V) for reconstruction via U * S * V^T |
| **Compaction** | Collapsing a delta chain into a new base block |
| **SparseEntry** | Single (index: u16, value: i16) pair in a delta |

### Aggregates

#### DeltaChain (Aggregate Root)

```rust
/// A chain of deltas anchored to a base block.
///
/// Invariants:
///   - chain_length <= max_delta_chain (configurable, default 8)
///   - deltas are ordered by epoch (ascending)
///   - base block reference must be valid (either materialized or itself a chain)
///   - compaction produces a new base block and resets the chain
pub struct DeltaChain {
    /// Block this chain belongs to
    key: BlockKey,
    /// Reference to the base block (tier and epoch)
    base_ref: BaseBlockRef,
    /// Ordered list of deltas from base
    deltas: Vec<DeltaRecord>,
    /// Maximum allowed chain length before forced compaction
    max_chain_length: usize,
    /// Pending domain events
    pending_events: Vec<DeltaDomainEvent>,
}

impl DeltaChain {
    pub fn new(key: BlockKey, base_ref: BaseBlockRef, max_chain_length: usize) -> Self {
        Self {
            key,
            base_ref,
            deltas: Vec::new(),
            max_chain_length,
            pending_events: Vec::new(),
        }
    }

    /// Append a new delta to the chain.
    /// Returns Err if chain is at max length (must compact first).
    pub fn append_delta(&mut self, delta: DeltaRecord) -> Result<(), DeltaError> {
        if self.deltas.len() >= self.max_chain_length {
            return Err(DeltaError::ChainFull {
                key: self.key,
                length: self.deltas.len(),
                max: self.max_chain_length,
            });
        }

        self.pending_events.push(DeltaDomainEvent::DeltaAppended {
            key: self.key,
            epoch: delta.header.base_epoch,
            nnz: delta.entries.len() as u32,
        });

        self.deltas.push(delta);
        Ok(())
    }

    /// Apply the full chain to reconstruct the current block data.
    /// Starts from the base block and applies each delta in order.
    pub fn apply_chain(&self, base_data: &mut [f32]) -> Result<(), DeltaError> {
        for delta in &self.deltas {
            for entry in &delta.entries {
                let idx = entry.index as usize;
                if idx < base_data.len() {
                    let delta_val = (entry.value as f32) * delta.header.delta_scale;
                    base_data[idx] += delta_val;
                }
            }
        }

        self.pending_events.iter().for_each(|_| {}); // events already recorded
        Ok(())
    }

    /// Compact the chain: collapse all deltas into the base block.
    /// Returns the new base data for storage.
    pub fn compact(&mut self, base_data: &mut [f32]) -> Result<Vec<f32>, DeltaError> {
        self.apply_chain(base_data)?;
        let compacted = base_data.to_vec();

        self.pending_events.push(DeltaDomainEvent::ChainCompacted {
            key: self.key,
            collapsed_deltas: self.deltas.len() as u32,
        });

        self.deltas.clear();
        Ok(compacted)
    }

    /// Current chain length.
    pub fn chain_length(&self) -> usize {
        self.deltas.len()
    }

    /// Whether compaction is needed.
    pub fn needs_compaction(&self) -> bool {
        self.deltas.len() >= self.max_chain_length
    }

    pub fn take_events(&mut self) -> Vec<DeltaDomainEvent> {
        std::mem::take(&mut self.pending_events)
    }
}
```

### Entities

```rust
/// A single delta record: sparse vector of changes from the previous state.
#[derive(Clone, Debug)]
pub struct DeltaRecord {
    /// Header with provenance metadata
    pub header: DeltaHeader,
    /// Sparse entries: (index, quantized delta value)
    pub entries: Vec<SparseEntry>,
}

/// Low-rank factor set for reconstruction via U * diag(S) * V^T.
/// Used when the block was evicted but its structure can be approximated
/// by a low-rank decomposition.
#[derive(Clone, Debug)]
pub struct FactorSet {
    /// Left singular vectors (rows x rank)
    pub u_matrix: Vec<f32>,
    /// Singular values (rank)
    pub s_values: Vec<f32>,
    /// Right singular vectors (rank x cols)
    pub v_matrix: Vec<f32>,
    /// Rank of the approximation
    pub rank: u32,
    /// Original tensor dimensions
    pub rows: u32,
    pub cols: u32,
}

impl FactorSet {
    /// Reconstruct the full tensor from factors.
    pub fn reconstruct(&self) -> Vec<f32> {
        let mut result = vec![0.0f32; (self.rows * self.cols) as usize];
        for r in 0..self.rank as usize {
            let s = self.s_values[r];
            for i in 0..self.rows as usize {
                let u_val = self.u_matrix[i * self.rank as usize + r] * s;
                for j in 0..self.cols as usize {
                    let v_val = self.v_matrix[r * self.cols as usize + j];
                    result[i * self.cols as usize + j] += u_val * v_val;
                }
            }
        }
        result
    }
}
```

### Value Objects

```rust
/// Header for a delta record with provenance metadata.
#[derive(Clone, Debug)]
pub struct DeltaHeader {
    pub tensor_id: u128,
    pub block_index: u32,
    pub base_epoch: u64,
    /// Number of non-zero entries
    pub nnz: u32,
    /// Scale factor for quantized delta values
    pub delta_scale: f32,
}

/// Single sparse entry in a delta: (index, quantized value).
#[derive(Clone, Copy, Debug)]
pub struct SparseEntry {
    /// Index into the block data (0-based)
    pub index: u16,
    /// Quantized delta value (signed)
    pub value: i16,
}

/// Reference to a base block for delta chain anchoring.
#[derive(Clone, Debug)]
pub struct BaseBlockRef {
    pub key: BlockKey,
    pub tier: Tier,
    pub epoch: u64,
}
```

### Domain Events

| Event | Trigger | Consumers |
|-------|---------|-----------|
| `DeltaAppended` | New delta added to chain | Storage Engine |
| `ChainCompacted` | Delta chain collapsed | Block Management, Storage |
| `FactorStored` | Low-rank factors computed and saved | Storage Engine |
| `ReconstructionAttempted` | Block rebuild from chain/factors | Metrics |
| `ReconstructionFailed` | Rebuild failed (missing base/factors) | Alerting |

```rust
#[derive(Clone, Debug)]
pub enum DeltaDomainEvent {
    DeltaAppended { key: BlockKey, epoch: u64, nnz: u32 },
    ChainCompacted { key: BlockKey, collapsed_deltas: u32 },
    FactorStored { key: BlockKey, rank: u32 },
    ReconstructionAttempted { key: BlockKey, method: ReconstructPolicy },
    ReconstructionFailed { key: BlockKey, reason: String },
}
```

---

## Context Map (Integration Patterns)

```
Block Management <--[Shared Kernel]--> Quantization
  - Shared types: BlockKey, Tier, DType, Checksum
  - Co-owned by both teams; changes require bilateral agreement
  - Boundary: QuantizationCodec is owned by BC2, TensorBlock by BC1

Block Management <--[Shared Kernel]--> Temporal Scoring
  - Shared types: BlockKey, Tier, BlockMeta
  - Scoring produces tier recommendations; Block Mgmt enforces transitions
  - Boundary: AccessProfile is owned by BC3, TensorBlock by BC1

Block Management <--[Customer/Supplier]--> Storage Engine
  - BC1 (customer) calls BC4 (supplier) for persistence
  - BC4 provides stable BlockIO and MetaLog traits
  - BC1 depends on BC4's write guarantees; BC4 is independent

Block Management <--[Customer/Supplier]--> Delta & Reconstruction
  - BC1 (customer) requests reconstruction from BC5 (supplier)
  - BC5 provides apply_chain() and reconstruct() operations
  - BC5 depends on BC4 (Storage) for reading base blocks

Temporal Scoring <--[Conformist]--> Storage Engine
  - BC3 reads metadata from BC4's index; conforms to BC4's data model
  - BC3 does not write to storage; read-only conformist

Storage Engine <--[Published Language]--> WASM API (host bindings)
  - The FFI layer (ffi.rs) provides a stable C ABI
  - Host code calls ttc_create, ttc_push_frame, ttc_flush, ttc_decode_segment
  - Handle-based resource management (Vec<Option<Compressor>>)
```

### Context Map Diagram

```
+--------------------+      Shared Kernel       +--------------------+
|                    |<========================>|                    |
|  BC1: Block        |  BlockKey, Tier, DType   |  BC2: Quantization |
|  Management        |  Checksum                |  Context           |
|                    |                          |                    |
+--------+-----------+      Shared Kernel       +--------------------+
         |           |<========================>|
         |           |  BlockKey, Tier,         |
         |           |  BlockMeta               |
         |           +------+                   |
         |                  |                   |
         | Customer         | BC3: Temporal     |
         | /Supplier        | Scoring Context   |
         v                  +--------+----------+
+--------------------+               |
|  BC4: Storage      |<--------------+  Conformist (reads metadata)
|  Engine Context    |
|                    |      Published Language
|  BlockIO, MetaLog  |=========================> WASM API (ffi.rs)
+--------+-----------+
         |
         | Customer/Supplier
         v
+--------------------+
|  BC5: Delta &      |
|  Reconstruction    |
|  Context           |
+--------------------+
```

---

## Rust Module Mapping

### Crate-to-Bounded-Context Mapping

```
Crate                              Bounded Context(s)
-----------------------------------+------------------------------------------
temporal_tensor_store              BC1 (Block Management) + orchestration
  src/lib.rs                         Public API, re-exports
  src/compressor.rs                  BC1: TemporalTensorCompressor aggregate

quant (ruvector-temporal-tensor)   BC2 (Quantization)
  src/quantizer.rs                   Groupwise symmetric quantization
  src/bitpack.rs                     Bitstream packer/unpacker
  src/f16.rs                         Software f16 conversion

tiering (ruvector-temporal-tensor) BC3 (Temporal Scoring)
  src/tier_policy.rs                 TierPolicy, score computation

codec_bits (shared)                BC2 (Quantization, shared kernel)
  src/bitpack.rs                     pack(), unpack(), qmax_from_bits()

metrics (ruvector-metrics)         Cross-cutting (witnesses, audit)

wasm_api                           BC4 (Storage, WASM layer)
  src/ffi.rs                         Handle store, extern "C" exports
```

### Module Structure

```
crates/ruvector-temporal-tensor/
+-- Cargo.toml
+-- src/
    +-- lib.rs              # Public API (BC1 orchestration)
    +-- compressor.rs       # BC1: TemporalTensorCompressor aggregate root
    +-- tier_policy.rs      # BC3: TierPolicy, score computation
    +-- quantizer.rs        # BC2: Groupwise symmetric quantization
    +-- bitpack.rs          # BC2: Bitstream packer/unpacker (shared kernel)
    +-- f16.rs              # BC2: Software f16 conversion (shared kernel)
    +-- segment.rs          # BC4: Segment encode/decode, binary format
    +-- ffi.rs              # BC4: WASM FFI, handle-based store

crates/ruvector-temporal-tensor-wasm/
+-- Cargo.toml              # wasm32-unknown-unknown target
+-- src/
    +-- lib.rs              # Re-exports FFI functions for WASM
```

### Dependency Graph

```
ruvector-temporal-tensor (zero external deps)
+-- bitpack.rs       (no deps)
+-- f16.rs           (no deps)
+-- quantizer.rs     (depends on: bitpack, f16)
+-- tier_policy.rs   (no deps)
+-- segment.rs       (depends on: quantizer)
+-- compressor.rs    (depends on: quantizer, segment, tier_policy)
+-- ffi.rs           (depends on: compressor, segment, tier_policy)

ruvector-temporal-tensor-wasm
+-- ruvector-temporal-tensor (the only dependency)
```

---

## Anti-Corruption Layers

### WASM FFI Anti-Corruption Layer

The `ffi.rs` module provides an ACL between the host environment and the domain model. The host interacts exclusively through opaque handles (u32 indices into `Vec<Option<TemporalTensorCompressor>>`), raw pointers, and C-compatible scalars. The ACL translates these into domain operations:

```rust
// Host calls this C ABI function:
extern "C" fn ttc_push_frame(
    handle: u32,           // opaque handle
    now_ts: u32,           // scalar timestamp
    in_ptr: *const f32,    // raw pointer to frame data
    len: u32,              // frame length
    out_ptr: *mut u8,      // output buffer
    out_cap: u32,          // output capacity
    out_written: *mut u32, // bytes written
);

// ACL translates to domain operation:
// compressor.push_frame(&frame_slice, now_ts, &mut segment_vec)
```

### AgentDB Integration Adapter

When integrating with AgentDB for persistent segment storage, an adapter implements the `BlockIO` trait, translating between the Temporal Tensor Store's domain model and AgentDB's key-value API:

```rust
/// Adapter implementing BlockIO over AgentDB's KV store.
pub struct AgentDbBlockIO {
    db: AgentDbClient,
    tenant: String,
}

impl BlockIO for AgentDbBlockIO {
    fn read_block(&self, tier: Tier, key: BlockKey, dst: &mut [u8]) -> Result<usize, StoreErr> {
        let db_key = format!("{}:{}:{}", self.tenant, key.tensor_id, key.block_index);
        let data = self.db.get(&db_key)?;
        let n = data.len().min(dst.len());
        dst[..n].copy_from_slice(&data[..n]);
        Ok(n)
    }

    fn write_block(&mut self, tier: Tier, key: BlockKey, src: &[u8]) -> Result<(), StoreErr> {
        let db_key = format!("{}:{}:{}", self.tenant, key.tensor_id, key.block_index);
        self.db.put(&db_key, src, &[("tier", &tier.as_str())])?;
        Ok(())
    }

    fn delete_block(&mut self, tier: Tier, key: BlockKey) -> Result<(), StoreErr> {
        let db_key = format!("{}:{}:{}", self.tenant, key.tensor_id, key.block_index);
        self.db.delete(&db_key)?;
        Ok(())
    }
}
```

### Coherence Engine Integration

The Coherence Engine (ADR-014, ADR-015) integrates via an event-driven boundary. When the coherence engine detects structural disagreement for a tensor, it emits a `DriftDetected` event that the Temporal Tensor Store consumes to force segment boundaries:

```rust
/// Event handler bridging Coherence Engine events to Temporal Tensor Store.
pub struct CoherenceBridge {
    compressors: HashMap<u128, TemporalTensorCompressor>,
}

impl CoherenceBridge {
    /// Called when coherence engine detects tensor drift.
    pub fn on_coherence_drift(&mut self, tensor_id: u128) -> Vec<Vec<u8>> {
        let mut flushed_segments = Vec::new();
        if let Some(comp) = self.compressors.get_mut(&tensor_id) {
            let mut seg = Vec::new();
            comp.flush(&mut seg);
            if !seg.is_empty() {
                flushed_segments.push(seg);
            }
        }
        flushed_segments
    }
}
```

---

## Relationship to ADR-016 Delta-Behavior DDD

The Temporal Tensor Store DDD and the Delta-Behavior DDD (ADR-016) are complementary systems that share a conceptual boundary around the notion of "delta" but operate at different abstraction levels.

### Shared Concepts

| Concept | ADR-016 (Delta-Behavior) | This DDD (Temporal Tensor Store) |
|---------|--------------------------|----------------------------------|
| **Delta** | Immutable record of differential change between two vector states | Sparse vector of (index, quantized_value) pairs within a block |
| **Ordering** | Causal ordering via Lamport timestamps | Epoch ordering within a chain |
| **Compaction** | Checkpoint creation to bound replay | Chain collapse into new base block |
| **Temporal window** | DeltaWindow for batching within time/count | Temporal Segment for amortizing scales across frames |

### Key Differences

1. **Granularity**: ADR-016 operates on full vector states (embeddings, graph nodes). The Temporal Tensor Store operates on fixed-size blocks (16KB/32KB chunks of tensors).

2. **Compression model**: ADR-016 delta vectors are sparse diffs between states. The Temporal Tensor Store uses quantization-based compression where "delta" is a secondary mechanism for evicted blocks only.

3. **Distribution model**: ADR-016 is designed for distributed propagation across nodes. The Temporal Tensor Store is designed for local storage tiering within a single node.

4. **ADR-016 term mapping**: What ADR-016 calls a "DeltaCheckpoint" maps to what this DDD calls a "base block" in a delta chain. ADR-016's "DeltaGraph" (DAG of dependencies) maps to the chain ordering invariant in BC5.

### Integration Surface

The two systems integrate at the **Delta & Reconstruction Context (BC5)**. When a block is evicted from the Temporal Tensor Store, the delta chain mechanism shares the same conceptual foundation as ADR-016's delta capture:

```
ADR-016 Delta-Behavior System
  |
  | DeltaVector (sparse change)
  v
BC5: Delta & Reconstruction Context
  |
  | DeltaRecord (sparse entries + quantized scale)
  v
BC4: Storage Engine
```

ADR-016's `DeltaChecksum` (tamper-evident chaining) can be adopted by BC5 for verifying delta chain integrity. ADR-016's `DeltaWindow` concept informs the Temporal Tensor Store's segment boundary logic (both batch changes within a temporal window to amortize metadata).

### Term Disambiguation

| ADR-017 Term | ADR-016 Term | Meaning |
|-------------|-------------|---------|
| Segment | (no equivalent) | Multi-frame compressed blob sharing quantization scales |
| Block | (closest: DeltaCheckpoint) | Fixed-size chunk of a tensor with tiered compression |
| Delta chain | DeltaStream | Ordered sequence of incremental changes from a base |
| Compaction | Checkpoint creation | Collapsing incremental changes into a new baseline |
| Drift | (closest: ChangeEvent) | Distribution shift exceeding scale tolerance |
| Tick | (closest: DeltaTimestamp.logical) | Logical time quantum for maintenance processing |

---

## Segment Binary Format Reference

For completeness, the on-disk segment format as defined in ADR-017 section 3.3:

```
Offset  Size    Field           Description
------  ------  --------------- ------------------------------------------
0       4       magic           0x43545154 ("TQTC" in LE ASCII)
4       1       version         Format version (currently 1)
5       1       bits            Bit width (3, 5, 7, or 8)
6       4       group_len       Elements per quantization group
10      4       tensor_len      Number of f32 elements per frame
14      4       frame_count     Number of frames in this segment
18      4       scale_count     Number of f16 group scales
22      2*S     scales          f16 scale values (S = scale_count)
22+2S   4       data_len        Length of packed bitstream in bytes
26+2S   D       data            Packed quantized codes (D = data_len)

Total: 26 + 2*ceil(tensor_len/group_len) + ceil(tensor_len * frame_count * bits / 8)
```

---

## Testing Strategy

### Property-Based Tests

```rust
#[quickcheck]
fn roundtrip_preserves_length(bits: TierBits, len: TensorLen) -> bool {
    let bits = bits.0; // constrained to {3, 5, 7, 8}
    let frame: Vec<f32> = (0..len.0).map(|i| (i as f32) * 0.1).collect();
    let scales = compute_scales(&frame, 64, bits);
    let mut packed = Vec::new();
    quantize_and_pack(&frame, &scales, 64, bits, &mut packed);
    let mut decoded = Vec::new();
    dequantize(&packed, &scales, 64, bits, frame.len(), 1, &mut decoded);
    decoded.len() == frame.len()
}

#[quickcheck]
fn error_bounded_by_tier(bits: TierBits, frame: SmallFrame) -> bool {
    let qmax = qmax_from_bits(bits.0);
    let max_relative_error = 1.0 / (2.0 * qmax as f32);
    let scales = compute_scales(&frame.0, 64, bits.0);
    let mut packed = Vec::new();
    quantize_and_pack(&frame.0, &scales, 64, bits.0, &mut packed);
    let mut decoded = Vec::new();
    dequantize(&packed, &scales, 64, bits.0, frame.0.len(), 1, &mut decoded);

    frame.0.iter().zip(decoded.iter()).all(|(&orig, &dec)| {
        let max_abs = frame.0.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        if max_abs < 1e-10 { return true; }
        let err = (orig - dec).abs() / max_abs;
        err < max_relative_error * 2.0 // 2x margin for f16 scale rounding
    })
}

#[quickcheck]
fn segment_encode_decode_deterministic(frame: SmallFrame, bits: TierBits) -> bool {
    let scales = compute_scales(&frame.0, 64, bits.0);
    let mut packed = Vec::new();
    quantize_and_pack(&frame.0, &scales, 64, bits.0, &mut packed);
    let mut seg1 = Vec::new();
    encode(bits.0, 64, frame.0.len() as u32, 1, &scales, &packed, &mut seg1);
    let mut seg2 = Vec::new();
    encode(bits.0, 64, frame.0.len() as u32, 1, &scales, &packed, &mut seg2);
    seg1 == seg2
}
```

### Tier Transition Tests

```rust
#[test]
fn tier_transitions_are_monotonic_within_tick() {
    let mut comp = TemporalTensorCompressor::new(TierPolicy::default(), 64, 0);
    comp.set_access(100, 0); // Hot
    let frame = vec![1.0f32; 64];
    let mut seg = Vec::new();

    // Hot -> push frame
    comp.push_frame(&frame, 1, &mut seg);
    assert_eq!(comp.active_bits(), 8);

    // Decay to cold
    comp.set_access(1, 0);
    comp.push_frame(&frame, 10000, &mut seg);
    assert_eq!(comp.active_bits(), 3);

    // Previous segment was flushed
    assert!(!seg.is_empty());
}
```

### Replay Determinism

```rust
#[test]
fn segment_decode_is_deterministic() {
    let mut comp = TemporalTensorCompressor::new(TierPolicy::default(), 128, 0);
    comp.set_access(100, 0);
    let frame: Vec<f32> = (0..128).map(|i| (i as f32 - 64.0) * 0.01).collect();
    let mut seg = Vec::new();

    for _ in 0..10 {
        comp.push_frame(&frame, 1, &mut seg);
    }
    comp.flush(&mut seg);

    let mut decoded1 = Vec::new();
    segment::decode(&seg, &mut decoded1);

    let mut decoded2 = Vec::new();
    segment::decode(&seg, &mut decoded2);

    assert_eq!(decoded1, decoded2);
}
```

---

## Aggregate Relationship Diagram

```
+===============================================================+
|                     AGGREGATE RELATIONSHIPS                     |
+===============================================================+
|                                                                 |
|  TensorBlock (BC1)                                              |
|  +-- owns --> BlockMeta                                         |
|  +-- owns --> BlockData (optional, None if evicted)             |
|  +-- refs --> TensorIdentity                                    |
|  +-- produces --> BlockDomainEvent                              |
|  |                                                              |
|  +---[tier change requires]---> QuantizationCodec (BC2)         |
|  |     +-- uses --> QuantParams                                 |
|  |     +-- uses --> PackedBlock                                 |
|  |     +-- delegates to --> BitPackingService                   |
|  |                                                              |
|  +---[score drives tier]------> AccessProfile (BC3)             |
|  |     +-- uses --> TierPolicy                                  |
|  |     +-- uses --> MaintenanceScheduler                        |
|  |     +-- produces --> ScoringDomainEvent                      |
|  |                                                              |
|  +---[persists via]-----------> TieredStore (BC4)               |
|  |     +-- uses --> BlockIO (trait)                              |
|  |     +-- uses --> MetaLog (trait)                              |
|  |     +-- uses --> Clock (trait)                                |
|  |     +-- produces --> StorageDomainEvent                      |
|  |                                                              |
|  +---[eviction creates]-------> DeltaChain (BC5)                |
|        +-- owns --> DeltaRecord[]                               |
|        +-- refs --> BaseBlockRef                                |
|        +-- alt --> FactorSet                                    |
|        +-- produces --> DeltaDomainEvent                        |
|                                                                 |
+===============================================================+
```

---

## References

1. Evans, E. (2003). "Domain-Driven Design: Tackling Complexity in the Heart of Software."
2. Vernon, V. (2013). "Implementing Domain-Driven Design."
3. ADR-017: Temporal Tensor Compression with Tiered Quantization (2026-02-06).
4. ADR-016: Delta-Behavior System DDD Architecture (2026-01-28).
5. ADR-014: Coherence Engine (2026-01-22).
6. ADR-004: KV Cache Management.
7. ADR-005: WASM Runtime Integration.
8. Frantar, E., et al. "GPTQ: Accurate Post-Training Quantization." ICLR 2023.
9. Lin, J., et al. "AWQ: Activation-aware Weight Quantization." MLSys 2024.
10. Liu, Z., et al. "KIVI: A Tuning-Free Asymmetric 2bit Quantization for KV Cache." ICML 2024.
11. Pelkonen, T., et al. "Gorilla: A Fast, Scalable, In-Memory Time Series Database." VLDB 2015.
