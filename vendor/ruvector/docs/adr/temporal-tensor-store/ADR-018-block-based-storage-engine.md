# ADR-018: Block-Based Storage Engine Architecture for the Temporal Tensor Store

**Status**: Proposed
**Date**: 2026-02-08
**Parent**: ADR-017 Temporal Tensor Compression, ADR-001 RuVector Core Architecture, ADR-004 KV Cache Management
**Author**: System Architecture Team
**SDK**: Claude-Flow

**Note**: The block-based storage engine described in this ADR is superseded by ADR-029 (RVF). The RVF VEC_SEG block model with delta-encoded adjacency lists replaces this approach.

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-08 | Architecture Team | Initial proposal |

---

## Abstract

This ADR defines the **block-based storage engine** that underpins the Temporal Tensor
Store (TTS). Where ADR-017 introduced the temporal tensor compression pipeline
(quantization, segment encoding, tier policy), this document specifies how
compressed tensor data is **organized on disk and in memory**, how blocks are
**identified, indexed, and persisted**, and how the engine **maintains integrity
through checksums and an append-only metadata log**.

The engine departs from ADR-017's segment-centric model -- which treats each
segment as an opaque byte blob keyed by time range -- and instead introduces a
**fixed-size block abstraction** that provides:

1. Stable, predictable I/O granularity (16 KB or 32 KB).
2. Per-block metadata with access-pattern tracking for tier migration.
3. An in-memory index rebuilt from an append-only MetaLog on startup.
4. Deterministic ordering by `(tensor_id, block_index)` for scan-friendly layout.
5. CRC32 checksums on quantized payloads for bit-flip detection.
6. A trait-based I/O boundary that supports both `mmap` on servers and
   in-memory buffers for WASM targets.

The design targets KV cache tensors, embedding streams, and attention
intermediates in agent workloads. It integrates with AgentDB for metadata
persistence and draws on the RIPPLE++ (2026) model for streaming incremental
inference and OMEGA for low-latency GNN serving.

---

## 1. Context and Motivation

### 1.1 Segment-Based vs. Block-Based Storage

ADR-017 established a segment-based compression pipeline. Each segment is a
self-contained byte blob containing a header, shared scales, and packed
quantized codes for one or more frames. Segments are stored in AgentDB keyed by
`{tensor_id}:{start_ts}:{end_ts}`.

This approach has several limitations when scaling to production workloads:

| Limitation | Impact |
|------------|--------|
| Variable segment sizes | Unpredictable I/O patterns; fragmentation on disk |
| No sub-segment random access beyond `decode_single_frame` | Cannot efficiently read a slice of a large segment |
| No per-block access tracking | Tier migration decisions must be made at the tensor level, not block level |
| No integrity verification | A single bit flip corrupts the entire segment silently |
| Tight coupling to AgentDB blob storage | Cannot use `mmap` or tiered file layout |

### 1.2 Why Fixed-Size Blocks

Fixed-size blocks are a proven primitive in storage systems (ext4, RocksDB SST
blocks, TiKV, Apache Arrow IPC). They provide:

- **Predictable I/O**: Every read and write is aligned to the same granularity.
- **Simple caching**: Block-sized buffers slot into page caches and slab allocators.
- **Locality**: Blocks within the same tensor are contiguous, enabling prefetch.
- **Independent checksums**: A corrupted block does not invalidate its neighbors.
- **Tier-granular migration**: Individual blocks can move between tiers independently.

### 1.3 Alignment to KV Cache Access Patterns

For attention KV cache (the primary workload per ADR-004), access patterns are
highly structured:

```
Attention head h, layer l, token position range [p0, p1]:
  Read key   block: tensor_id = hash(layer=l, head=h, type=key),   block_index = p0 / block_elements
  Read value block: tensor_id = hash(layer=l, head=h, type=value), block_index = p0 / block_elements
```

Aligning block boundaries to head-dimension multiples ensures that a single
attention head's data for a contiguous token range lives in a single block,
minimizing cross-block reads during prefill and decode.

### 1.4 RIPPLE++ and OMEGA Context

RIPPLE++ (2026) proposes streaming incremental inference where KV cache
entries are produced and consumed in a pipelined fashion. The block-based
engine supports this by allowing append-only writes to the tail block while
older blocks are concurrently read for attention computation.

OMEGA (2026) targets low-latency GNN serving with tiered tensor storage.
Its block-aligned eviction strategy directly inspired the tier-bucket design
in this ADR.

---

## 2. Decision

### 2.1 Introduce a Block-Based Storage Engine as a New Crate Layer

We introduce the `temporal_tensor_store` crate that sits above
`ruvector-temporal-tensor` (ADR-017) and provides:

1. **Block identity**: Stable 128-bit tensor IDs with per-tensor block indexing.
2. **BlockMeta**: Rich per-block metadata including access tracking, tier, quantization
   parameters, checksums, and reconstruction policy.
3. **Tiered data files**: Separate files per tier for scan-friendly eviction.
4. **Append-only MetaLog**: Crash-recoverable metadata persistence.
5. **In-memory index**: HashMap + tier buckets + min-heap for fast lookup and eviction.
6. **Trait-based I/O**: `BlockIO`, `MetaLog`, and `Clock` traits abstract the storage
   backend for server (`mmap`) and WASM (in-memory buffer) targets.

### 2.2 Relationship to ADR-017

ADR-017's compression pipeline remains the **codec layer**. This ADR adds the
**storage layer** on top:

```
+===================================================================+
|                    TEMPORAL TENSOR STORE (ADR-018)                  |
|                                                                    |
|  Block identity  |  BlockMeta  |  MetaLog  |  Tiered files        |
|  In-memory index |  Eviction   |  Checksums                       |
+===================================================================+
          |                    |                    |
          v                    v                    v
+===================================================================+
|              TEMPORAL TENSOR COMPRESSION (ADR-017)                  |
|                                                                    |
|  Groupwise quantization  |  Bitstream packing  |  Segment format  |
|  Tier policy scoring     |  Drift detection    |  f16 scales      |
+===================================================================+
          |
          v
+===================================================================+
|                    RUVECTOR CORE (ADR-001)                          |
|                                                                    |
|  Distance functions  |  HNSW index  |  Scalar/Product quantization |
+===================================================================+
```

The segment format from ADR-017 is used **within** each block as the payload
encoding. A block's `q` payload is a TQTC segment (or a raw byte region for
Tier0 uncompressed data).

---

## 3. Detailed Design

### 3.1 Tensor Identity

Every tensor managed by the store has a stable 128-bit identifier.

**Option A -- UUID v4**: Random, globally unique, no collision risk. Requires
an external registry to map logical names to UUIDs.

**Option B -- Deterministic hash of lineage + logical name**: Computed as
`blake3(tenant_id || collection || logical_name || lineage_parent)` truncated
to 128 bits. Reproducible, collision-resistant (128-bit birthday bound is
~2^64 tensors), and allows the same tensor to be identified across restarts
without a registry.

**Decision**: Option B (deterministic hash). The reproducibility property is
essential for crash recovery -- the MetaLog can be validated against recomputed
IDs. For tensors with no lineage parent, the parent field is zeroed.

### 3.2 Block Key

A block is uniquely identified by the pair `(tensor_id, block_index)`:

```rust
/// Unique identifier for a single block within the store.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BlockKey {
    /// 128-bit tensor identity (deterministic hash of lineage + name).
    pub tensor_id: u128,
    /// Zero-based index of this block within the tensor's block sequence.
    pub block_index: u32,
}

impl BlockKey {
    /// Deterministic total ordering: tensor_id first, then block_index.
    /// Used for scan-friendly layout and MetaLog replay ordering.
    pub fn sort_key(&self) -> (u128, u32) {
        (self.tensor_id, self.block_index)
    }
}

impl Ord for BlockKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sort_key().cmp(&other.sort_key())
    }
}

impl PartialOrd for BlockKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
```

**Stable ordering guarantee**: All scans, MetaLog entries, and data file
layouts use `(tensor_id, block_index)` lexicographic order. This ensures
deterministic replay and enables range scans over a tensor's blocks.

### 3.3 Chunking Strategy

Tensors are divided into fixed-size blocks before storage.

| Parameter | Default | Rationale |
|-----------|---------|-----------|
| `BLOCK_RAW_BYTES` | 16384 (16 KB) | Matches typical OS page size; good L2 cache fit |
| `BLOCK_RAW_BYTES` (KV cache) | 32768 (32 KB) | Aligned to head_dim * num_tokens_per_block * sizeof(f16) |

For KV cache tensors, the block boundary is aligned to head-dimension
multiples:

```
block_elements = BLOCK_RAW_BYTES / bytes_per_element
// Round down to nearest multiple of head_dim:
block_elements = (block_elements / head_dim) * head_dim
```

For a typical head_dim=128 with f16 values:
```
block_elements = 32768 / 2 = 16384 elements
16384 / 128 = 128 token positions per block (exact alignment)
```

This ensures that every block boundary falls on a token-position boundary,
so attention over a contiguous token range never crosses a block.

### 3.4 Tier Enumeration

```rust
/// Storage tier indicating compression level and access latency.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum Tier {
    /// Tier 0: Uncompressed f32/f16. Resident in memory or fastest storage.
    Tier0 = 0,
    /// Tier 1: 8-bit quantized (hot). ~4x compression.
    Tier1 = 1,
    /// Tier 2: 5-bit or 7-bit quantized (warm). ~4.5x-6.4x compression.
    Tier2 = 2,
    /// Tier 3: 3-bit quantized (cold). ~10.7x compression.
    Tier3 = 3,
}

impl Tier {
    /// Convert from raw u8. Returns None for invalid values.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Tier::Tier0),
            1 => Some(Tier::Tier1),
            2 => Some(Tier::Tier2),
            3 => Some(Tier::Tier3),
            _ => None,
        }
    }
}
```

**Tier0** is new relative to ADR-017. It holds uncompressed tensor data for
blocks that are actively being written or that require bit-exact access (e.g.,
during gradient accumulation). Tier0 blocks are never persisted to tier data
files -- they exist only in the in-memory buffer or page cache.

### 3.5 Data Type Enumeration

```rust
/// Element data type for the original (unquantized) tensor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum DType {
    F32 = 0,
    F16 = 1,
    BF16 = 2,
    I8 = 3,
    U8 = 4,
}
```

### 3.6 Reconstruction Policy

```rust
/// Policy for reconstructing a block's full-precision data.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ReconstructPolicy {
    /// No reconstruction needed; block payload is self-contained.
    None = 0,
    /// Reconstruct by applying a delta to the lineage parent block.
    Delta = 1,
    /// Reconstruct by multiplying factors with a base block.
    Factor = 2,
}
```

The `Delta` policy enables storing only the difference from a parent block
(useful for KV cache entries that change incrementally across decoding steps).
The `Factor` policy supports factorized representations where a block stores
low-rank factors that reconstruct the full tensor via matrix multiplication.

### 3.7 Block Metadata (BlockMeta)

```rust
/// Complete metadata for a single block in the store.
///
/// This structure is stored in the in-memory index and persisted
/// via the append-only MetaLog. It contains everything needed to
/// locate, decode, verify, and score a block for tier migration.
pub struct BlockMeta {
    // ---- Identity ----
    /// Unique block identifier.
    pub key: BlockKey,

    // ---- Tensor shape (encoded once per tensor, stored per block for self-containment) ----
    /// Original tensor shape, encoded as a compact dimension list.
    /// For a 2D tensor [rows, cols], shape = [rows as u32, cols as u32].
    /// Maximum 8 dimensions.
    pub shape: [u32; 8],
    /// Number of valid entries in the shape array.
    pub shape_ndim: u8,

    // ---- Data type ----
    /// Element type of the original unquantized tensor.
    pub dtype: DType,

    // ---- Tier and quantization ----
    /// Current storage tier.
    pub tier: Tier,
    /// Quantization bit width (3, 5, 7, 8, or 32 for uncompressed).
    pub bits: u8,
    /// Quantization scale (from ADR-017 groupwise symmetric quantization).
    /// For multi-group blocks, this is the maximum group scale.
    pub scale: f32,
    /// Quantization zero point (0 for symmetric quantization).
    pub zero_point: i16,

    // ---- Timestamps ----
    /// Tick at which this block was created.
    pub created_at: u64,
    /// Tick of the most recent read or write access.
    pub last_access_at: u64,

    // ---- Access tracking ----
    /// Total number of accesses since creation.
    pub access_count: u32,
    /// Exponential moving average of the access rate (accesses per tick).
    pub ema_access_rate: f32,
    /// Bitset window: bit i is set if the block was accessed at tick (now - i).
    /// Provides a compact 64-tick access history.
    pub access_window: u64,

    // ---- Integrity ----
    /// CRC32 checksum over the quantized payload bytes concatenated with
    /// the scale bytes. Detects bit flips in storage.
    pub checksum: u32,

    // ---- Lineage ----
    /// Optional tensor_id of the parent block for delta/factor reconstruction.
    /// Zero means no parent (self-contained block).
    pub lineage_parent: u128,
    /// Reconstruction policy.
    pub reconstruct: ReconstructPolicy,

    // ---- Tier migration bookkeeping ----
    /// Number of ticks the block has spent in its current tier.
    pub tier_age: u32,
}
```

### 3.8 Access History Tracking

Three complementary mechanisms track access patterns with different tradeoffs:

```
+------------------------------------------------------------------+
|                   ACCESS HISTORY TRACKING                         |
+------------------------------------------------------------------+
|                                                                    |
|  1. Bitset Window (u64)                                           |
|     +-------------------------------------------------------+    |
|     | bit 0 | bit 1 | bit 2 | ... | bit 62 | bit 63        |    |
|     | now   | now-1 | now-2 | ... | now-62 | now-63        |    |
|     +-------------------------------------------------------+    |
|     Compact. O(1) update. Exact for last 64 ticks.               |
|     Use: Burst detection, recent activity check.                  |
|                                                                    |
|  2. EMA Access Rate (f32)                                         |
|     rate_new = alpha * (1/dt) + (1-alpha) * rate_old             |
|     alpha = 0.1 (configurable)                                    |
|     Use: Smooth scoring for tier migration.                       |
|                                                                    |
|  3. Access Count + Last Access Timestamp                          |
|     score = access_count * 1024 / (now - last_access_at + 1)    |
|     Use: Coarse tier selection (compatible with ADR-017 policy).  |
|                                                                    |
+------------------------------------------------------------------+
```

**Bitset window update**:
```rust
impl BlockMeta {
    /// Shift the window by `elapsed` ticks and set bit 0 (current tick).
    pub fn record_access(&mut self, now: u64) {
        let elapsed = now.saturating_sub(self.last_access_at);
        if elapsed > 0 {
            // Shift old bits; bits older than 64 ticks fall off.
            if elapsed >= 64 {
                self.access_window = 1; // Only current tick survives.
            } else {
                self.access_window = (self.access_window >> elapsed) | 1;
            }
        } else {
            self.access_window |= 1; // Same tick, just set bit 0.
        }
        self.last_access_at = now;
        self.access_count = self.access_count.saturating_add(1);

        // Update EMA: rate = alpha * instantaneous + (1-alpha) * old
        let dt = elapsed.max(1) as f32;
        let instantaneous = 1.0 / dt;
        const ALPHA: f32 = 0.1;
        self.ema_access_rate = ALPHA * instantaneous + (1.0 - ALPHA) * self.ema_access_rate;
    }

    /// Number of ticks (out of the last 64) in which this block was accessed.
    pub fn recent_access_density(&self) -> u32 {
        self.access_window.count_ones()
    }

    /// Tier migration score combining EMA rate and access density.
    /// Higher score = hotter block = keep in higher tier.
    pub fn migration_score(&self, now: u64) -> f32 {
        let age = (now.saturating_sub(self.created_at)).max(1) as f32;
        let density = self.recent_access_density() as f32 / 64.0;
        // Weighted combination: EMA dominates long-term, density captures bursts.
        0.7 * self.ema_access_rate * 1000.0 + 0.3 * density * 1000.0 / age.sqrt()
    }
}
```

### 3.9 Storage Layout

```
<root>/
  <tenant_id>/
    <collection>/
      meta.log              # Append-only MetaLog (MetaRecord entries)
      tier1.dat             # Tier 1 data file (8-bit quantized blocks)
      tier2.dat             # Tier 2 data file (5/7-bit quantized blocks)
      tier3.dat             # Tier 3 data file (3-bit quantized blocks)
      delta.dat             # Optional: delta payloads for ReconstructPolicy::Delta
      factor.dat            # Optional: factor payloads for ReconstructPolicy::Factor
```

**ASCII diagram of on-disk layout**:

```
meta.log (append-only)
+--------+--------+--------+--------+--------+------->
| rec[0] | rec[1] | rec[2] | rec[3] | rec[4] | ...
+--------+--------+--------+--------+--------+------->
  ^create   ^create  ^update  ^migrate  ^delete
   block A   block B  block A  block A   block C
                      access   tier1->2

tier1.dat (8-bit blocks, sorted by BlockKey)
+============+============+============+============+
| Block A.0  | Block A.1  | Block D.0  | Block D.1  |
| 16 KB      | 16 KB      | 16 KB      | 16 KB      |
+============+============+============+============+
  q payload    q payload    q payload    q payload
  + scales     + scales     + scales     + scales

tier2.dat (5/7-bit blocks)
+============+============+
| Block B.0  | Block E.0  |
| 16 KB      | 16 KB      |
+============+============+

tier3.dat (3-bit blocks)
+============+============+============+
| Block C.0  | Block C.1  | Block F.0  |
| 16 KB      | 16 KB      | 16 KB      |
+============+============+============+
```

Each block slot in a tier data file is padded to the configured
`BLOCK_RAW_BYTES` size. This wastes up to `BLOCK_RAW_BYTES - 1` bytes per
block but guarantees that every block can be read with a single aligned I/O
operation.

**Memory mapping (server targets)**: Tier data files are opened with
`mmap(MAP_SHARED)` for zero-copy reads. The OS page cache handles eviction.
Writes use `mmap(MAP_PRIVATE)` with explicit `msync` on flush.

**WASM targets**: Data is held in `Vec<u8>` buffers. A host-provided
persistence hook (`fn persist(tier: Tier, data: &[u8])`) is called on flush
to write buffers to IndexedDB, OPFS, or a host filesystem.

### 3.10 MetaLog Format

The MetaLog is an append-only file of fixed-size records. Each record
describes a single state transition for a block.

```rust
/// A single record in the append-only MetaLog.
#[derive(Clone, Debug)]
pub enum MetaRecord {
    /// A new block was created.
    Create {
        meta: BlockMeta,
        /// Byte offset within the tier data file where the block payload starts.
        data_offset: u64,
        /// Length of the block payload in bytes.
        data_len: u32,
    },
    /// A block's access metadata was updated.
    Access {
        key: BlockKey,
        last_access_at: u64,
        access_count: u32,
        ema_access_rate: f32,
        access_window: u64,
    },
    /// A block was migrated to a different tier.
    Migrate {
        key: BlockKey,
        old_tier: Tier,
        new_tier: Tier,
        new_bits: u8,
        new_scale: f32,
        new_checksum: u32,
        new_data_offset: u64,
        new_data_len: u32,
    },
    /// A block was deleted.
    Delete {
        key: BlockKey,
    },
}
```

**Record binary format** (little-endian, fixed 128-byte records with padding):

```
Offset  Size  Field
------  ----  -----
0       1     record_type (0=Create, 1=Access, 2=Migrate, 3=Delete)
1       16    tensor_id (u128 LE)
17      4     block_index (u32 LE)
21      ...   record-type-specific fields
120     4     record_crc32 (CRC32 over bytes 0..120)
124     4     padding (0x00)
```

On startup, the engine replays every record sequentially to rebuild the
in-memory index. Invalid records (CRC32 mismatch) are skipped with a warning.
This replay is O(N) in the number of records and typically completes in
<100ms for stores with fewer than 1 million blocks.

### 3.11 In-Memory Index

```rust
use std::collections::{BinaryHeap, HashMap};

/// The in-memory index provides O(1) block lookup and O(1) tier-bucket access.
pub struct BlockIndex {
    /// Primary index: BlockKey -> BlockMeta.
    /// Uses hashbrown internally for better cache performance on large maps.
    map: HashMap<BlockKey, BlockMeta>,

    /// Per-tier block lists for fast candidate selection during migration.
    tier_buckets: [Vec<BlockKey>; 4],

    /// Min-heap of (score, BlockKey) for eviction candidates.
    /// The block with the lowest migration_score is at the top.
    eviction_heap: BinaryHeap<std::cmp::Reverse<(OrderedFloat, BlockKey)>>,

    /// Data file offsets: BlockKey -> (data_offset, data_len) per tier.
    offsets: HashMap<BlockKey, (u64, u32)>,
}

/// Wrapper for f32 that implements Ord (NaN-safe).
#[derive(Clone, Copy, PartialEq)]
struct OrderedFloat(f32);

impl Eq for OrderedFloat {}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl BlockIndex {
    /// Create an empty index.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            tier_buckets: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            eviction_heap: BinaryHeap::new(),
            offsets: HashMap::new(),
        }
    }

    /// Insert or update a block's metadata.
    pub fn upsert(&mut self, meta: BlockMeta, data_offset: u64, data_len: u32) {
        let key = meta.key;
        let tier_idx = meta.tier as usize;

        // Remove from old tier bucket if present.
        if let Some(old) = self.map.get(&key) {
            let old_tier_idx = old.tier as usize;
            if old_tier_idx != tier_idx {
                self.tier_buckets[old_tier_idx].retain(|k| k != &key);
            }
        }

        self.tier_buckets[tier_idx].push(key);
        self.offsets.insert(key, (data_offset, data_len));
        self.map.insert(key, meta);
    }

    /// Look up a block's metadata by key.
    pub fn get(&self, key: &BlockKey) -> Option<&BlockMeta> {
        self.map.get(key)
    }

    /// Look up a block's data file location.
    pub fn get_offset(&self, key: &BlockKey) -> Option<(u64, u32)> {
        self.offsets.get(key).copied()
    }

    /// Remove a block from the index.
    pub fn remove(&mut self, key: &BlockKey) -> Option<BlockMeta> {
        if let Some(meta) = self.map.remove(key) {
            let tier_idx = meta.tier as usize;
            self.tier_buckets[tier_idx].retain(|k| k != key);
            self.offsets.remove(key);
            Some(meta)
        } else {
            None
        }
    }

    /// Return all block keys in a given tier.
    pub fn blocks_in_tier(&self, tier: Tier) -> &[BlockKey] {
        &self.tier_buckets[tier as usize]
    }

    /// Total number of blocks across all tiers.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Rebuild eviction heap from current metadata.
    pub fn rebuild_eviction_heap(&mut self, now: u64) {
        self.eviction_heap.clear();
        for (key, meta) in &self.map {
            let score = meta.migration_score(now);
            self.eviction_heap
                .push(std::cmp::Reverse((OrderedFloat(score), *key)));
        }
    }

    /// Pop the block with the lowest migration score (best eviction candidate).
    pub fn pop_coldest(&mut self) -> Option<BlockKey> {
        self.eviction_heap.pop().map(|std::cmp::Reverse((_, key))| key)
    }
}
```

### 3.12 Checksums and Integrity

Every block's quantized payload is protected by a CRC32 checksum:

```rust
/// Compute CRC32 over the quantized payload concatenated with scale bytes.
///
/// This detects:
/// - Bit flips in the compressed data (storage media errors).
/// - Corrupted scale values (which would cause wild dequantization errors).
/// - Truncated writes (partial block).
pub fn compute_block_checksum(q_payload: &[u8], scale_bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in q_payload.iter().chain(scale_bytes.iter()) {
        crc = crc32_update(crc, byte);
    }
    crc ^ 0xFFFF_FFFF
}

/// CRC32 (Castagnoli) single-byte update.
/// Uses a lookup table for performance; the table is generated at compile time.
fn crc32_update(crc: u32, byte: u8) -> u32 {
    let idx = ((crc ^ byte as u32) & 0xFF) as usize;
    CRC32_TABLE[idx] ^ (crc >> 8)
}

/// CRC32-C lookup table (256 entries, generated at compile time).
const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0x82F6_3B78; // Castagnoli polynomial
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i as usize] = crc;
        i += 1;
    }
    table
};
```

**On read**: After reading a block from a tier data file, recompute the
checksum and compare against `BlockMeta::checksum`. On mismatch:

1. Log a `CHECKSUM_MISMATCH` event with the block key and tier.
2. If `reconstruct != None`, attempt to rehydrate from the parent block.
3. If rehydration fails or `reconstruct == None`, return `StoreErr::Corruption`.
4. Emit a metric counter for monitoring.

### 3.13 Public Traits

The storage engine defines three traits to abstract the I/O boundary:

```rust
/// Monotonic tick source for timestamps.
///
/// On native targets this wraps `std::time::Instant` or a hardware TSC.
/// On WASM targets this wraps `performance.now()` via the host.
pub trait Clock {
    /// Return the current tick value. Must be monotonically non-decreasing.
    fn now_ticks(&self) -> u64;
}

/// Block-level I/O operations.
///
/// Implementations:
/// - `MmapBlockIO`: Memory-mapped files for server targets.
/// - `BufferBlockIO`: In-memory `Vec<u8>` for WASM targets.
pub trait BlockIO {
    /// Read a block's payload into `dst`. Returns the number of bytes read.
    ///
    /// # Errors
    /// - `StoreErr::NotFound` if the block does not exist in the given tier.
    /// - `StoreErr::Corruption` if the read data fails checksum validation.
    /// - `StoreErr::Io` for underlying I/O errors.
    fn read_block(
        &self,
        tier: Tier,
        key: BlockKey,
        offset: u64,
        len: u32,
        dst: &mut [u8],
    ) -> Result<usize, StoreErr>;

    /// Write a block's payload to the given tier. Returns the byte offset
    /// at which the block was written.
    ///
    /// The implementation must guarantee that after a successful return,
    /// the data is durable (flushed to storage or committed to the
    /// WASM host persistence hook).
    fn write_block(
        &mut self,
        tier: Tier,
        key: BlockKey,
        src: &[u8],
    ) -> Result<(u64, u32), StoreErr>;

    /// Mark a block's storage slot as free in the given tier.
    ///
    /// The implementation may reclaim space immediately or defer to compaction.
    fn delete_block(
        &mut self,
        tier: Tier,
        key: BlockKey,
        offset: u64,
        len: u32,
    ) -> Result<(), StoreErr>;
}

/// Append-only metadata log.
///
/// Implementations:
/// - `FileMetaLog`: Append to a file with CRC32-protected records.
/// - `MemMetaLog`: In-memory `Vec<MetaRecord>` for WASM or testing.
pub trait MetaLog {
    /// Append a metadata record to the log.
    ///
    /// Must be atomic: either the full record is written or nothing is.
    fn append(&mut self, rec: MetaRecord) -> Result<(), StoreErr>;

    /// Iterate over all records in the log in order.
    ///
    /// Used during startup to replay and rebuild the in-memory index.
    fn iter(&self) -> Box<dyn Iterator<Item = Result<MetaRecord, StoreErr>> + '_>;

    /// Number of records in the log.
    fn record_count(&self) -> u64;
}
```

### 3.14 Error Type

```rust
/// Errors returned by the storage engine.
#[derive(Debug)]
pub enum StoreErr {
    /// Block not found in the specified tier.
    NotFound { key: BlockKey, tier: Tier },
    /// Checksum mismatch detected on read.
    Corruption {
        key: BlockKey,
        expected: u32,
        actual: u32,
    },
    /// Underlying I/O error.
    Io(std::io::Error),
    /// MetaLog record is malformed or has invalid CRC.
    InvalidRecord { offset: u64, reason: String },
    /// Capacity exceeded (e.g., tier data file is full).
    CapacityExceeded { tier: Tier },
}
```

### 3.15 Store Engine (Orchestration)

```rust
/// The main storage engine that coordinates blocks, metadata, and I/O.
pub struct TensorStore<C: Clock, B: BlockIO, M: MetaLog> {
    clock: C,
    block_io: B,
    meta_log: M,
    index: BlockIndex,
    config: StoreConfig,
}

/// Configuration for the storage engine.
pub struct StoreConfig {
    /// Raw block size in bytes (before quantization).
    pub block_raw_bytes: usize,
    /// Maximum number of blocks per tier before eviction triggers.
    pub max_blocks_per_tier: [usize; 4],
    /// EMA alpha for access rate smoothing.
    pub ema_alpha: f32,
    /// Score threshold for tier promotion (cold -> warm, warm -> hot).
    pub promote_threshold: f32,
    /// Score threshold for tier demotion (hot -> warm, warm -> cold).
    pub demote_threshold: f32,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            block_raw_bytes: 16384,
            max_blocks_per_tier: [1024, 4096, 8192, 16384],
            ema_alpha: 0.1,
            promote_threshold: 512.0,
            demote_threshold: 32.0,
        }
    }
}

impl<C: Clock, B: BlockIO, M: MetaLog> TensorStore<C, B, M> {
    /// Create a new store, replaying the MetaLog to rebuild the index.
    pub fn open(clock: C, block_io: B, meta_log: M, config: StoreConfig) -> Result<Self, StoreErr> {
        let mut index = BlockIndex::new();

        // Replay MetaLog to rebuild in-memory state.
        for record in meta_log.iter() {
            let record = record?;
            match record {
                MetaRecord::Create { meta, data_offset, data_len } => {
                    index.upsert(meta, data_offset, data_len);
                }
                MetaRecord::Access { key, last_access_at, access_count, ema_access_rate, access_window } => {
                    if let Some(meta) = index.map.get_mut(&key) {
                        meta.last_access_at = last_access_at;
                        meta.access_count = access_count;
                        meta.ema_access_rate = ema_access_rate;
                        meta.access_window = access_window;
                    }
                }
                MetaRecord::Migrate { key, new_tier, new_bits, new_scale, new_checksum, new_data_offset, new_data_len, .. } => {
                    if let Some(meta) = index.map.get_mut(&key) {
                        let old_tier = meta.tier;
                        meta.tier = new_tier;
                        meta.bits = new_bits;
                        meta.scale = new_scale;
                        meta.checksum = new_checksum;
                        meta.tier_age = 0;
                        // Update tier buckets.
                        index.tier_buckets[old_tier as usize].retain(|k| k != &key);
                        index.tier_buckets[new_tier as usize].push(key);
                        index.offsets.insert(key, (new_data_offset, new_data_len));
                    }
                }
                MetaRecord::Delete { key } => {
                    index.remove(&key);
                }
            }
        }

        let now = clock.now_ticks();
        index.rebuild_eviction_heap(now);

        Ok(Self { clock, block_io, meta_log, index, config })
    }

    /// Write a new block to the store.
    pub fn put_block(
        &mut self,
        key: BlockKey,
        tier: Tier,
        dtype: DType,
        shape: &[u32],
        q_payload: &[u8],
        scale_bytes: &[u8],
        bits: u8,
        scale: f32,
        zero_point: i16,
        lineage_parent: u128,
        reconstruct: ReconstructPolicy,
    ) -> Result<(), StoreErr> {
        let now = self.clock.now_ticks();
        let checksum = compute_block_checksum(q_payload, scale_bytes);

        // Write payload to tier data file.
        let (data_offset, data_len) = self.block_io.write_block(tier, key, q_payload)?;

        // Build metadata.
        let mut shape_arr = [0u32; 8];
        let ndim = shape.len().min(8);
        shape_arr[..ndim].copy_from_slice(&shape[..ndim]);

        let meta = BlockMeta {
            key,
            shape: shape_arr,
            shape_ndim: ndim as u8,
            dtype,
            tier,
            bits,
            scale,
            zero_point,
            created_at: now,
            last_access_at: now,
            access_count: 0,
            ema_access_rate: 0.0,
            access_window: 1, // Accessed at creation tick.
            checksum,
            lineage_parent,
            reconstruct,
            tier_age: 0,
        };

        // Persist to MetaLog.
        self.meta_log.append(MetaRecord::Create {
            meta: meta.clone(),
            data_offset,
            data_len,
        })?;

        // Update in-memory index.
        self.index.upsert(meta, data_offset, data_len);

        Ok(())
    }

    /// Read a block's payload, validating its checksum.
    pub fn get_block(
        &mut self,
        key: &BlockKey,
        dst: &mut [u8],
    ) -> Result<usize, StoreErr> {
        let now = self.clock.now_ticks();

        let meta = self.index.get(key)
            .ok_or(StoreErr::NotFound { key: *key, tier: Tier::Tier0 })?;
        let tier = meta.tier;
        let expected_checksum = meta.checksum;

        let (offset, len) = self.index.get_offset(key)
            .ok_or(StoreErr::NotFound { key: *key, tier })?;

        let bytes_read = self.block_io.read_block(tier, *key, offset, len, dst)?;

        // Validate checksum.
        let actual_checksum = compute_block_checksum(&dst[..bytes_read], &[]);
        if actual_checksum != expected_checksum {
            return Err(StoreErr::Corruption {
                key: *key,
                expected: expected_checksum,
                actual: actual_checksum,
            });
        }

        // Update access metadata.
        if let Some(meta) = self.index.map.get_mut(key) {
            meta.record_access(now);
        }

        Ok(bytes_read)
    }

    /// Migrate a block from its current tier to a new tier.
    ///
    /// This re-quantizes the data at the new tier's bit width,
    /// writes to the new tier file, and updates metadata.
    pub fn migrate_block(
        &mut self,
        key: &BlockKey,
        new_tier: Tier,
        new_bits: u8,
        re_quantized_payload: &[u8],
        new_scale_bytes: &[u8],
        new_scale: f32,
    ) -> Result<(), StoreErr> {
        let meta = self.index.get(key)
            .ok_or(StoreErr::NotFound { key: *key, tier: Tier::Tier0 })?;
        let old_tier = meta.tier;

        let new_checksum = compute_block_checksum(re_quantized_payload, new_scale_bytes);

        // Write to new tier.
        let (new_offset, new_len) = self.block_io.write_block(new_tier, *key, re_quantized_payload)?;

        // Delete from old tier.
        if let Some((old_offset, old_len)) = self.index.get_offset(key) {
            let _ = self.block_io.delete_block(old_tier, *key, old_offset, old_len);
        }

        // Persist migration record.
        self.meta_log.append(MetaRecord::Migrate {
            key: *key,
            old_tier,
            new_tier,
            new_bits,
            new_scale,
            new_checksum,
            new_data_offset: new_offset,
            new_data_len: new_len,
        })?;

        // Update in-memory state.
        if let Some(meta) = self.index.map.get_mut(key) {
            meta.tier = new_tier;
            meta.bits = new_bits;
            meta.scale = new_scale;
            meta.checksum = new_checksum;
            meta.tier_age = 0;
            // Update tier buckets.
            self.index.tier_buckets[old_tier as usize].retain(|k| k != key);
            self.index.tier_buckets[new_tier as usize].push(*key);
            self.index.offsets.insert(*key, (new_offset, new_len));
        }

        Ok(())
    }
}
```

### 3.16 Data Flow: Write Path

```
                         put_block()
                             |
                             v
+--------------------------------------------------------------------+
|  1. Compute CRC32 checksum over q_payload + scale_bytes            |
+--------------------------------------------------------------------+
                             |
                             v
+--------------------------------------------------------------------+
|  2. BlockIO::write_block(tier, key, payload)                       |
|     - Server: append to mmap'd tier file, return offset            |
|     - WASM: append to Vec<u8> buffer, schedule persist hook        |
+--------------------------------------------------------------------+
                             |
                             v
+--------------------------------------------------------------------+
|  3. Build BlockMeta with timestamps, checksum, tier, quant params  |
+--------------------------------------------------------------------+
                             |
                             v
+--------------------------------------------------------------------+
|  4. MetaLog::append(Create { meta, data_offset, data_len })        |
|     - Serialize 128-byte record with CRC32 trailer                 |
|     - Append to meta.log file / memory buffer                      |
+--------------------------------------------------------------------+
                             |
                             v
+--------------------------------------------------------------------+
|  5. BlockIndex::upsert(meta, data_offset, data_len)                |
|     - Insert into HashMap                                          |
|     - Add to tier bucket                                           |
|     - Update offsets map                                           |
+--------------------------------------------------------------------+
```

### 3.17 Data Flow: Read Path

```
                         get_block()
                             |
                             v
+--------------------------------------------------------------------+
|  1. BlockIndex::get(key) -> BlockMeta                              |
|     - O(1) HashMap lookup                                          |
|     - Extract tier, checksum, data offset                          |
+--------------------------------------------------------------------+
                             |
                             v
+--------------------------------------------------------------------+
|  2. BlockIO::read_block(tier, key, offset, len, dst)               |
|     - Server: read from mmap (zero-copy page fault)                |
|     - WASM: memcpy from Vec<u8> buffer                             |
+--------------------------------------------------------------------+
                             |
                             v
+--------------------------------------------------------------------+
|  3. Validate CRC32: compute_block_checksum(dst) == meta.checksum?  |
|     - YES: proceed to step 4                                       |
|     - NO:  attempt rehydrate from lineage_parent                   |
|            if rehydrate fails -> return StoreErr::Corruption        |
+--------------------------------------------------------------------+
                             |
                             v
+--------------------------------------------------------------------+
|  4. Update access metadata:                                        |
|     - meta.record_access(now)                                      |
|     - (Optionally) append Access record to MetaLog                 |
|       (batched every N reads to reduce log growth)                 |
+--------------------------------------------------------------------+
                             |
                             v
+--------------------------------------------------------------------+
|  5. Return payload bytes to caller for dequantization              |
|     (dequantization via ADR-017 pipeline)                          |
+--------------------------------------------------------------------+
```

### 3.18 Determinism Guarantees

The storage engine provides the following determinism properties:

1. **Stable ordering**: Given the same sequence of `put_block` and
   `migrate_block` calls, the MetaLog will contain the same records in
   the same order, and the in-memory index will be identical after replay.

2. **Reproducible IDs**: Tensor IDs derived via `blake3(lineage + name)`
   produce the same ID for the same inputs across platforms and restarts.

3. **Deterministic eviction**: The eviction heap ordering is a pure function
   of `(migration_score, BlockKey)`. Ties are broken by BlockKey's total
   order `(tensor_id, block_index)`, ensuring the same block is evicted
   given the same access history.

4. **Platform-independent encoding**: All on-disk formats use little-endian
   byte order. The MetaLog record size is fixed at 128 bytes regardless
   of record type.

### 3.19 Differences from ADR-017 Segment-Based Approach

| Aspect | ADR-017 (Segment) | ADR-018 (Block) |
|--------|-------------------|-----------------|
| Granularity | Variable-size segments (header + N frames) | Fixed-size blocks (16 KB or 32 KB) |
| Identity | Time-range key `{tensor_id}:{start_ts}:{end_ts}` | `BlockKey(tensor_id, block_index)` |
| Metadata | Embedded in segment header | Separate `BlockMeta` + MetaLog |
| Access tracking | Per-compressor `access_count` and `last_access_ts` | Per-block EMA, bitset window, counters |
| Checksums | None | CRC32 per block |
| Tier migration | Tier determined at segment creation time | Blocks migrate independently between tiers |
| Random access | `decode_single_frame` within a segment | Direct block read by `(tensor_id, block_index)` |
| Crash recovery | Segments stored as AgentDB blobs | Append-only MetaLog replay |
| I/O pattern | Variable-size blob reads | Fixed-size aligned reads (page-cache friendly) |
| WASM support | Handle-based FFI in compressor | Trait-based `BlockIO` with host persistence hooks |
| Lineage | Optional DAG edges on segments | Built-in `lineage_parent` + `ReconstructPolicy` |

The segment format from ADR-017 is **not replaced** -- it continues to serve
as the codec within each block. A block's quantized payload may contain one
or more TQTC-encoded segments, or may use a simpler packed format when
temporal scale reuse is not applicable (e.g., single-frame embedding blocks).

---

## 4. Alternatives Considered

### 4.1 Variable-Size Blocks (LSM-Style)

**Considered**: Use variable-size blocks like an LSM tree's SSTable blocks,
where each block is as large as needed to hold one tensor's data.

**Rejected**: Variable-size blocks complicate I/O alignment, make space
reclamation harder, and prevent simple offset-based addressing. The fixed-size
approach wastes some space to padding but gains significant simplicity and
performance predictability.

### 4.2 Page-Aligned I/O Without Blocks

**Considered**: Store raw quantized data in flat files and use offset-based
addressing without a block abstraction.

**Rejected**: Without blocks, metadata (checksums, access tracking, tier
assignment) must be stored separately in a parallel structure with no natural
co-location. Blocks provide a clean unit of metadata attachment.

### 4.3 SQLite for Metadata

**Considered**: Use SQLite (via sql.js for WASM) instead of an append-only
MetaLog for metadata persistence.

**Rejected**: SQLite adds a dependency (contrary to ADR-017's zero-dependency
philosophy), introduces write amplification for append-heavy workloads, and
is slower than a simple sequential log for the replay-on-startup pattern.
The MetaLog can be compacted periodically by writing a snapshot and
truncating old records.

### 4.4 Content-Addressable Blocks (CAS)

**Considered**: Address blocks by the hash of their content, like a
content-addressable store (git objects, IPFS).

**Rejected**: Tensor blocks are mutable in the sense that their tier and
quantization parameters change during migration. CAS would require creating
new block identities on every migration, breaking references. The
`(tensor_id, block_index)` identity is stable across migrations.

### 4.5 Ring Buffer for Access History

**Considered**: Use a ring buffer of `u16` timestamps (last 16 access
timestamps) instead of the u64 bitset window.

**Rejected as primary**: The ring buffer uses 32 bytes per block vs. 8 bytes
for the bitset. For stores with millions of blocks, this adds significant
memory overhead. The bitset provides sufficient resolution for tier migration
decisions. The ring buffer may be added as an optional diagnostic mode in the
future.

---

## 5. Acceptance Criteria

### 5.1 Functional Requirements

- [ ] `put_block` writes a block to the correct tier data file and appends a
      `Create` record to the MetaLog.
- [ ] `get_block` reads a block, validates its CRC32 checksum, and updates
      access metadata.
- [ ] `migrate_block` moves a block between tiers, re-quantizes its payload,
      and persists a `Migrate` record.
- [ ] MetaLog replay on startup reconstructs the exact same in-memory index
      as existed before shutdown.
- [ ] Corrupted blocks (CRC32 mismatch) are detected and reported via
      `StoreErr::Corruption`.
- [ ] Blocks with `ReconstructPolicy::Delta` can be rehydrated from their
      lineage parent when corruption is detected.
- [ ] BlockKey ordering is deterministic: sorting by `(tensor_id, block_index)`
      produces the same order on all platforms.
- [ ] The engine operates correctly with both `MmapBlockIO` (server) and
      `BufferBlockIO` (WASM) implementations.

### 5.2 Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| `put_block` latency (16 KB, SSD) | < 50 us | p50, sequential writes |
| `get_block` latency (16 KB, warm cache) | < 10 us | p50, random reads after warmup |
| `get_block` latency (16 KB, cold cache) | < 200 us | p50, random reads without warmup |
| MetaLog replay (1M records) | < 500 ms | Wall-clock time from open to ready |
| In-memory index lookup | < 100 ns | p50, `BlockIndex::get` |
| CRC32 checksum (16 KB) | < 5 us | Single block verification |
| Migration (Tier1 -> Tier3, 16 KB) | < 100 us | Including re-quantization and MetaLog append |
| Memory per block (metadata only) | < 256 bytes | `size_of::<BlockMeta>()` + index overhead |

### 5.3 Compression Targets (Inherited from ADR-017)

| Tier | Bits | Target Ratio vs. f32 | After Block Overhead |
|------|------|---------------------|---------------------|
| Tier0 | 32 (raw) | 1.0x | ~0.98x (block padding) |
| Tier1 | 8 | ~4.0x | ~3.9x |
| Tier2 | 5 or 7 | ~4.5x-6.4x | ~4.4x-6.2x |
| Tier3 | 3 | ~10.7x | ~10.3x |

### 5.4 Integrity Targets

- [ ] Zero undetected bit flips: every corrupted block is caught by CRC32.
- [ ] MetaLog records with invalid CRC are skipped during replay without
      crashing the engine.
- [ ] After a crash mid-write, the MetaLog is consistent up to the last
      fully-written record (no torn records).

---

## 6. Risks and Mitigations

| Risk | Severity | Likelihood | Mitigation |
|------|----------|------------|------------|
| Fixed block size wastes space for small tensors | Medium | High | Allow sub-block packing for tensors < block_size/4; track fill ratio in BlockMeta |
| MetaLog grows unboundedly | Medium | Medium | Periodic compaction: write a snapshot of current index, truncate log; compact every N records or on startup |
| CRC32 is not cryptographically secure | Low | Low | CRC32 detects accidental corruption. If tamper resistance is needed, add HMAC-SHA256 (future ADR) |
| Mmap on 32-bit WASM limited to 4 GB address space | Medium | Medium | WASM uses BufferBlockIO (in-memory) with host persistence; no mmap. Tier data files are segmented to stay within limits |
| Eviction heap becomes stale between rebuilds | Low | Medium | Rebuild heap on every N-th get_block call or timer-based; lazy invalidation acceptable for tier migration |
| Deterministic ordering assumption broken by concurrent writes | Medium | Low | Single-writer model for MetaLog (no concurrent appends). Multi-writer requires fencing (future ADR) |
| Block padding wastes disk space | Low | High | Expected overhead is < 5% for typical workloads. Acceptable tradeoff for I/O alignment benefits |

---

## 7. Crate Structure

The block-based storage engine is organized as a Rust workspace with
focused crates:

```
crates/
  temporal_tensor_store/       # Orchestration: TensorStore, BlockIndex, read/write paths
    src/
      lib.rs                   # Public API, re-exports
      store.rs                 # TensorStore<C, B, M> implementation
      index.rs                 # BlockIndex: HashMap + tier buckets + eviction heap
      meta_log.rs              # MetaLog trait + FileMetaLog + MemMetaLog
      block_io.rs              # BlockIO trait + MmapBlockIO + BufferBlockIO
      types.rs                 # BlockKey, BlockMeta, Tier, DType, ReconstructPolicy, StoreErr
      checksum.rs              # CRC32 computation (zero-dependency, const table)
      config.rs                # StoreConfig
    Cargo.toml

  quant/                       # Quantization formats (re-exports ADR-017 quantizer)
    src/
      lib.rs
      symmetric.rs             # Groupwise symmetric quantization
      bitpack.rs               # Bit packing/unpacking
      f16.rs                   # Software f16 conversion
    Cargo.toml

  tiering/                     # Tier scoring, migration scheduling
    src/
      lib.rs
      scorer.rs                # Migration score computation
      scheduler.rs             # Background migration scheduler
      policy.rs                # Tier thresholds, hysteresis
    Cargo.toml

  codec_bits/                  # Bit-level packing/unpacking utilities
    src/
      lib.rs
      pack.rs                  # Bitstream packer (accumulator-based)
      unpack.rs                # Bitstream unpacker
      simd.rs                  # Optional SIMD-accelerated paths
    Cargo.toml

  metrics/                     # Witness logs, audit trail
    src/
      lib.rs
      witness.rs               # Immutable operation log
      counters.rs              # Atomic counters for monitoring
      export.rs                # Prometheus/OpenTelemetry export
    Cargo.toml

  wasm_api/                    # WASM FFI surface
    src/
      lib.rs
      ffi.rs                   # extern "C" functions for WASM hosts
      host_hooks.rs            # Trait for host-provided persistence
    Cargo.toml
```

**Dependency graph**:

```
wasm_api
  |
  +---> temporal_tensor_store
  |       |
  |       +---> quant (re-exports ruvector-temporal-tensor quantizer)
  |       +---> tiering
  |       +---> codec_bits
  |       +---> metrics
  |
  +---> (host-provided persistence via trait)

temporal_tensor_store
  |
  +---> ruvector-temporal-tensor (ADR-017, codec layer)
  +---> tiering
  +---> codec_bits
  +---> metrics
```

All crates maintain zero external dependencies for the core paths,
preserving WASM compatibility as established in ADR-017.

---

## 8. Integration Context

### 8.1 AgentDB Integration

AgentDB serves as the **external metadata persistence** layer for deployments
that do not use the file-based MetaLog:

```
+------------------+         +------------------+
| TensorStore      |         | AgentDB          |
|                  |         |                  |
| MetaLog (trait)  |-------->| Key-Value Store  |
|                  |         | HNSW Index       |
| BlockIO (trait)  |----+    | B-Tree Index     |
+------------------+    |    +------------------+
                        |
                        v
              +------------------+
              | Tier Data Files  |
              | (or OPFS/IDB    |
              |  via WASM host) |
              +------------------+
```

The `AgentDbMetaLog` implementation wraps AgentDB's key-value store:
- Key: `meta:{tenant}:{collection}:{record_sequence}`
- Value: Serialized `MetaRecord` bytes
- Tags: `type=metalog`, `tenant={id}`, `collection={id}`

### 8.2 KV Cache Integration (ADR-004)

The three-tier KV cache from ADR-004 maps directly to the block store's tiers:

| KV Cache Tier (ADR-004) | Block Store Tier (ADR-018) | Bits |
|-------------------------|---------------------------|------|
| High-Precision Tail Buffer (FP16) | Tier0 (uncompressed) | 16/32 |
| Moderate Quantization Zone (4-bit) | Tier1 (8-bit) or Tier2 (5-bit) | 5-8 |
| Aggressive Compression Zone (2-bit) | Tier3 (3-bit) | 3 |

The block store's per-block access tracking replaces ADR-004's per-token
staleness heuristic with a more granular mechanism that operates at the
block level (covering multiple tokens).

### 8.3 Coherence Engine Integration (ADR-014, ADR-015)

The coherence engine can trigger block-level operations:

- **Force migration**: When coherence score drops below threshold, demote
  affected blocks to force re-quantization with fresh scales.
- **Lineage validation**: Verify that blocks in a delta chain are consistent
  by checking parent-child checksum chains.
- **Anomaly detection**: Flag blocks whose access patterns deviate
  significantly from their tensor's historical baseline.

### 8.4 Delta-Behavior System (ADR-016)

The `ReconstructPolicy::Delta` directly supports ADR-016's delta-behavior
model. A block with delta reconstruction stores only the difference from
its lineage parent, enabling:

- Efficient incremental updates (write only the changed portion).
- Temporal queries (reconstruct any version by replaying the delta chain).
- Space savings when consecutive blocks are highly correlated.

---

## 9. Implementation Roadmap

### Phase 1: Core Types and Index (Week 1)
- [ ] Define `BlockKey`, `BlockMeta`, `Tier`, `DType`, `ReconstructPolicy`, `StoreErr`
- [ ] Implement `BlockIndex` with HashMap, tier buckets, and eviction heap
- [ ] Implement `BlockMeta::record_access` and `migration_score`
- [ ] Implement CRC32 checksum computation (const lookup table)
- [ ] Unit tests for all types, ordering, and index operations

### Phase 2: MetaLog and Persistence (Week 1-2)
- [ ] Define `MetaLog` trait and `MetaRecord` enum
- [ ] Implement `MemMetaLog` (in-memory, for WASM and testing)
- [ ] Implement `FileMetaLog` (append-only file with CRC32 records)
- [ ] MetaLog replay tests: create -> access -> migrate -> delete sequences
- [ ] Crash recovery tests: truncated records, corrupted CRC

### Phase 3: BlockIO Backends (Week 2)
- [ ] Define `BlockIO` trait
- [ ] Implement `BufferBlockIO` (in-memory Vec<u8>, WASM-compatible)
- [ ] Implement `MmapBlockIO` (memory-mapped files, server target)
- [ ] I/O round-trip tests for both backends

### Phase 4: TensorStore Orchestration (Week 2-3)
- [ ] Implement `TensorStore::open` with MetaLog replay
- [ ] Implement `put_block`, `get_block`, `migrate_block`
- [ ] Checksum validation on read path
- [ ] Access metadata batching (every N reads)
- [ ] Integration tests: full write -> read -> migrate -> read cycle

### Phase 5: Tiering Engine (Week 3)
- [ ] Implement migration scorer in `tiering` crate
- [ ] Implement background migration scheduler
- [ ] Hysteresis logic for promote/demote thresholds
- [ ] End-to-end test: blocks auto-migrate based on access patterns

### Phase 6: WASM API (Week 3-4)
- [ ] Define host persistence hooks trait
- [ ] Implement `wasm_api` FFI surface
- [ ] wasm-pack integration tests
- [ ] Binary size validation (< 150 KB for store + codec)

### Phase 7: AgentDB Integration (Week 4)
- [ ] Implement `AgentDbMetaLog`
- [ ] Implement `AgentDbBlockIO` (blob storage backend)
- [ ] End-to-end benchmark on representative KV cache workload
- [ ] Acceptance test: MetaLog replay produces identical index

---

## 10. References

1. ADR-017: Temporal Tensor Compression with Tiered Quantization. RuVector, 2026.
2. ADR-001: RuVector Core Architecture. RuVector, 2026.
3. ADR-004: KV Cache Management Strategy for RuvLLM. RuVector, 2026.
4. ADR-016: Delta-Behavior System - Domain-Driven Design Architecture. RuVector, 2026.
5. ADR-005: WASM Runtime Integration. RuVector, 2026.
6. O'Neil, P., et al. "The Log-Structured Merge-Tree (LSM-Tree)." Acta Informatica, 1996.
7. Pelkonen, T., et al. "Gorilla: A Fast, Scalable, In-Memory Time Series Database." VLDB, 2015.
8. Liu, Z., et al. "KIVI: A Tuning-Free Asymmetric 2bit Quantization for KV Cache." ICML, 2024.
9. RIPPLE++. "Streaming Incremental Inference for Large Language Models." arXiv, 2026.
10. OMEGA. "Low-Latency GNN Serving with Tiered Tensor Storage." arXiv, 2026.
11. Dong, S., et al. "RocksDB: Evolution of Development Priorities in a Key-Value Store Serving Large-Scale Applications." ACM TODS, 2021.
12. Apache Arrow IPC Format Specification. https://arrow.apache.org/docs/format/IPC.html

---

## Appendix A: MetaLog Record Binary Layout

```
128-byte fixed record (little-endian):

Byte 0:      record_type     (u8: 0=Create, 1=Access, 2=Migrate, 3=Delete)
Bytes 1-16:  tensor_id       (u128 LE)
Bytes 17-20: block_index     (u32 LE)

--- Create (type=0) ---
Bytes 21:    dtype           (u8)
Bytes 22:    tier            (u8)
Bytes 23:    bits            (u8)
Bytes 24-27: scale           (f32 LE)
Bytes 28-29: zero_point      (i16 LE)
Bytes 30-37: created_at      (u64 LE)
Bytes 38-45: data_offset     (u64 LE)
Bytes 46-49: data_len        (u32 LE)
Bytes 50-53: checksum        (u32 LE)
Bytes 54-69: lineage_parent  (u128 LE)
Bytes 70:    reconstruct     (u8)
Bytes 71-119: reserved       (zero-padded)

--- Access (type=1) ---
Bytes 21-28: last_access_at  (u64 LE)
Bytes 29-32: access_count    (u32 LE)
Bytes 33-36: ema_access_rate (f32 LE)
Bytes 37-44: access_window   (u64 LE)
Bytes 45-119: reserved       (zero-padded)

--- Migrate (type=2) ---
Bytes 21:    old_tier        (u8)
Bytes 22:    new_tier        (u8)
Bytes 23:    new_bits        (u8)
Bytes 24-27: new_scale       (f32 LE)
Bytes 28-31: new_checksum    (u32 LE)
Bytes 32-39: new_data_offset (u64 LE)
Bytes 40-43: new_data_len    (u32 LE)
Bytes 44-119: reserved       (zero-padded)

--- Delete (type=3) ---
Bytes 21-119: reserved       (zero-padded)

--- All records ---
Bytes 120-123: record_crc32  (CRC32 over bytes 0..120)
Bytes 124-127: padding       (0x00000000)
```

## Appendix B: Tier Migration Score Examples

| Scenario | access_count | EMA rate | Window density | Score | Tier Decision |
|----------|-------------|----------|---------------|-------|---------------|
| Active KV cache head | 10000 | 50.0 | 60/64 | ~35700 | Tier0/Tier1 (hot) |
| Recently used embedding | 500 | 5.0 | 32/64 | ~4050 | Tier1 (hot) |
| Periodic batch access | 100 | 0.5 | 8/64 | ~425 | Tier2 (warm) |
| Stale attention cache | 10 | 0.01 | 1/64 | ~12 | Tier3 (cold) |
| Archived gradient sketch | 2 | 0.001 | 0/64 | ~0.7 | Tier3 (cold, eviction candidate) |

## Appendix C: Block Size Selection Rationale

```
                Block Size vs. Overhead Tradeoff

  Overhead %   |
  (padding     |
   waste)      |  *
               |   *
  10% ---------|----*----------------------------
               |     *
               |      *
   5% ---------|-------*-------------------------
               |         *
               |            *     *     *     *
   1% ---------|----------------------------------
               +----+----+----+----+----+----+-->
                 4K   8K  16K  32K  64K 128K
                          Block Size

  At 16 KB: ~3% average padding waste for typical tensor sizes.
  At 32 KB: ~1.5% average padding waste.
  At  4 KB: ~12% average padding waste (too many blocks, high metadata cost).
  At 64 KB: ~0.8% waste but poor L2 cache utilization.

  Decision: 16 KB default, 32 KB for KV cache aligned to head dimensions.
```

## Appendix D: Comparison with Existing Storage Engines

| Feature | RocksDB | TiKV | Arrow IPC | TTS (this ADR) |
|---------|---------|------|-----------|----------------|
| Block size | 4-64 KB (configurable) | 4 KB default | Variable | 16-32 KB (fixed) |
| Compression | LZ4/Zstd/Snappy | LZ4/Zstd | None/LZ4 | Quantization (3-8 bit) |
| Checksums | CRC32 per block | CRC32 per block | None | CRC32 per block |
| Index | LSM tree | LSM tree | Footer metadata | HashMap + tier buckets |
| Write pattern | Log-structured | Log-structured | Append-only | Append-only per tier |
| Compaction | Background merge | Background merge | N/A | MetaLog snapshot |
| Access tracking | None | None | None | Per-block EMA + bitset |
| Tier migration | Manual (column families) | Manual | N/A | Automatic (score-based) |
| WASM support | No | No | Limited | Full (trait-based I/O) |
| Tensor-aware | No | No | Schema-aware | Quantization-aware |
