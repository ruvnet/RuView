# ADR-021: Delta Compression and Reconstruction Policies

**Status**: Proposed
**Date**: 2026-02-08
**Parent**: ADR-017 Temporal Tensor Compression, ADR-018 Block-Based Storage Engine
**Author**: System Architecture Team

**Note**: Delta compression is now implemented via RVF OVERLAY_SEG as part of ADR-029. See the overlay epochs specification (docs/research/rvf/spec/05-overlay-epochs.md).

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-08 | Architecture Team | Initial proposal |

---

## Abstract

This ADR defines delta compression, reconstruction policies, and the associated
read/write data paths for the Temporal Tensor Store. It extends the tiered
quantization system from ADR-017 with a fourth logical tier -- Tier0 -- that
compresses data to zero resident bits while preserving the ability to
reconstruct on demand via delta chains or low-rank factor decomposition. The
design adds sparse delta encoding for incremental writes, bounded-depth delta
chain management with automatic compaction, and three explicit reconstruction
policies (`None`, `Delta`, `Factor`) that control what happens when a reader
requests a block that has been evicted to Tier0.

All structures target Rust with `#[no_std]` compatibility for the WASM path,
consistent with the zero-dependency constraint established in ADR-017.

---

## 1. Context and Motivation

### 1.1 The Eviction Gap

ADR-017 introduced three quantization tiers (8-bit hot, 7/5-bit warm, 3-bit
cold) that trade precision for storage. However, it provides no mechanism for
tensors that have become completely stale -- data that has not been accessed in
a long time and whose storage cost exceeds its value. Today the only option is
full deletion, which is irreversible.

Production workloads produce tensor streams where the vast majority of blocks
become irrelevant within minutes but a small fraction are needed hours or days
later for debugging, auditing, or replay. We need a tier that retains the
ability to reconstruct without paying any per-block storage cost during steady
state.

### 1.2 The Incremental Update Problem

The current write path (ADR-017 `push_frame`) always stores a full quantized
representation. When a tensor block changes by only a few elements --
common during fine-tuning steps or incremental embedding updates -- writing the
entire block wastes bandwidth and storage. Delta encoding captures only the
changed elements as sparse pairs.

### 1.3 Design Goals

1. **Zero-cost eviction**: Tier0 blocks consume zero data bytes; only metadata
   survives.
2. **Configurable reconstruction**: Callers choose whether evicted blocks are
   reconstructable, and by which method.
3. **Bounded delta chains**: Delta reads are O(K) where K is a small,
   configurable constant (default 8), not O(history_length).
4. **Sparse delta writes**: Incremental changes below a threshold are stored as
   sparse vectors, saving up to 90% over full-block rewrites.
5. **WASM-safe**: All structures use fixed-size integers and simple layouts
   compatible with `wasm32-unknown-unknown`.

---

## 2. Tier Model Extension

The tier model from ADR-017 is extended with Tier0:

```
Tier1 (Hot)   -- 8-bit quantized  -- full fidelity, fast access
Tier2 (Warm)  -- 7/5-bit quantized -- reduced fidelity, moderate access
Tier3 (Cold)  -- 3-bit quantized  -- low fidelity, infrequent access
Tier0 (Zero)  -- 0-bit evicted    -- metadata only, reconstructable on demand
```

Tier0 is reached when the tier score from `TierPolicy::select_bits` falls
below a new configurable threshold `evict_min_score` (default: 4), or when the
storage engine triggers explicit eviction under memory pressure.

---

## 3. Reconstruction Policies

### 3.1 Enum Definition

```rust
/// Controls how a Tier0 (evicted) block is handled on read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ReconstructPolicy {
    /// No reconstruction. Reads return an error or zeros depending on
    /// `zero_fill_on_evict` in the global config.
    None = 0,

    /// Reconstruct from a base block plus a bounded-depth delta chain.
    /// The base is stored in the factor file or an older tier snapshot.
    Delta = 1,

    /// Reconstruct from stored low-rank factors (SVD decomposition).
    /// Factors are stored in a dedicated factor file: U, S, V matrices.
    Factor = 2,
}

/// Error returned when a Tier0 block cannot be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// Block has been evicted and the reconstruction policy is None.
    TensorEvicted,
    /// Delta chain is corrupted or a link is missing.
    DeltaChainBroken { depth: u16 },
    /// Factor file is missing or corrupt.
    FactorMissing,
    /// Block metadata not found.
    BlockNotFound,
    /// Supplied output buffer is too small.
    BufferTooSmall { needed: usize, provided: usize },
}
```

### 3.2 Policy Selection Rationale

| Policy | Storage Cost | Read Latency | Quality | Best For |
|--------|-------------|-------------|---------|----------|
| None | 0 | N/A (error) | N/A | Truly disposable data |
| Delta | O(K * nnz) | O(K * N) | Exact at base tier | Audit trails, debugging replay |
| Factor | O(k*(m+n)) | O(k*m + k*n) | Bounded by truncation rank | Attention weight matrices |

---

## 4. Delta Format

### 4.1 Binary Layout

```
Delta Record (variable length):

Offset  Size   Field         Description
------  -----  ------------- ------------------------------------------
0       16     tensor_id     u128 LE - identifies the tensor
16      4      block_index   u32 LE  - block within the tensor
20      8      base_epoch    u64 LE  - epoch of the base this delta applies to
28      2      nnz           u16 LE  - number of non-zero delta entries
30      4      delta_scale   f32 LE  - scale factor for i16 delta values
34      nnz*4  pairs         Array of (index: u16, value: i16) pairs
```

Total size per delta: `34 + 4 * nnz` bytes.

For WASM targets, delta values are stored as `i16` with a shared `delta_scale`
(f32) to keep the arithmetic simple and avoid f64 in the critical path.

### 4.2 Rust Structures

```rust
/// On-disk header for a single delta record.
#[derive(Clone, Debug)]
#[repr(C, packed)]
pub struct DeltaHeader {
    pub tensor_id: u128,
    pub block_index: u32,
    pub base_epoch: u64,
    pub nnz: u16,
    pub delta_scale: f32,
}

/// A single sparse delta entry: position and quantized value.
#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct DeltaPair {
    pub index: u16,
    pub value: i16,
}

/// In-memory representation of a delta record.
#[derive(Clone, Debug)]
pub struct DeltaRecord {
    pub header: DeltaHeader,
    pub pairs: Vec<DeltaPair>,
}

impl DeltaRecord {
    /// Serialise to bytes (little-endian, WASM-safe).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(34 + self.pairs.len() * 4);
        buf.extend_from_slice(&self.header.tensor_id.to_le_bytes());
        buf.extend_from_slice(&self.header.block_index.to_le_bytes());
        buf.extend_from_slice(&self.header.base_epoch.to_le_bytes());
        buf.extend_from_slice(&self.header.nnz.to_le_bytes());
        buf.extend_from_slice(&self.header.delta_scale.to_le_bytes());
        for p in &self.pairs {
            buf.extend_from_slice(&p.index.to_le_bytes());
            buf.extend_from_slice(&p.value.to_le_bytes());
        }
        buf
    }

    /// Deserialise from bytes. Returns None on truncated input.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 34 {
            return None;
        }
        let tensor_id = u128::from_le_bytes(data[0..16].try_into().ok()?);
        let block_index = u32::from_le_bytes(data[16..20].try_into().ok()?);
        let base_epoch = u64::from_le_bytes(data[20..28].try_into().ok()?);
        let nnz = u16::from_le_bytes(data[28..30].try_into().ok()?);
        let delta_scale = f32::from_le_bytes(data[30..34].try_into().ok()?);

        let pairs_len = nnz as usize;
        if data.len() < 34 + pairs_len * 4 {
            return None;
        }
        let mut pairs = Vec::with_capacity(pairs_len);
        let mut off = 34;
        for _ in 0..pairs_len {
            let index = u16::from_le_bytes(data[off..off + 2].try_into().ok()?);
            let value = i16::from_le_bytes(data[off + 2..off + 4].try_into().ok()?);
            pairs.push(DeltaPair { index, value });
            off += 4;
        }

        Some(Self {
            header: DeltaHeader {
                tensor_id,
                block_index,
                base_epoch,
                nnz,
                delta_scale,
            },
            pairs,
        })
    }
}
```

---

## 5. Block Metadata Extension

The per-block metadata from ADR-018 is extended with reconstruction fields:

```rust
/// Extended block metadata supporting Tier0 and reconstruction.
#[derive(Clone, Debug)]
pub struct BlockMeta {
    pub tensor_id: u128,
    pub block_index: u32,
    pub epoch: u64,

    /// Current storage tier: 0 = evicted, 1 = hot, 2 = warm, 3 = cold.
    pub tier: u8,
    /// Bit width of the stored representation (0 for Tier0).
    pub bits: u8,
    /// Reconstruction policy when tier == 0.
    pub reconstruct_policy: ReconstructPolicy,

    /// Number of deltas chained on top of the base for this block.
    pub delta_chain_len: u16,
    /// Epoch of the base block at the root of the delta chain.
    pub base_epoch: u64,

    /// Byte offset into the tier data file (unused when tier == 0).
    pub data_offset: u64,
    /// Byte length in the tier data file (0 when tier == 0).
    pub data_len: u32,

    /// Access tracking for tier policy.
    pub access_count: u32,
    pub last_access_ts: u32,
}
```

---

## 6. Read Path

### 6.1 Sequence Diagram

```
Caller              BlockStore           TierDataFile        DeltaStore       FactorStore
  |                     |                     |                  |                 |
  |-- read_block(id) -->|                     |                  |                 |
  |                     |-- lookup_meta(id) ->|                  |                 |
  |                     |<--- BlockMeta ------|                  |                 |
  |                     |                     |                  |                 |
  |          [tier 1/2/3?]                    |                  |                 |
  |                     |-- read_bytes ------>|                  |                 |
  |                     |<--- quantized ------|                  |                 |
  |                     |-- dequantize ------>|                  |                 |
  |<-- f32 buffer ------|                     |                  |                 |
  |                     |                     |                  |                 |
  |         [tier 0, policy=None?]            |                  |                 |
  |<-- Err(TensorEvicted)                     |                  |                 |
  |                     |                     |                  |                 |
  |         [tier 0, policy=Delta?]           |                  |                 |
  |                     |-- load_base ------->|                  |                 |
  |                     |<--- base block -----|                  |                 |
  |                     |-- load_deltas ------|----------------->|                 |
  |                     |<--- delta chain ----|------------------|                 |
  |                     |-- apply_chain ----->|                  |                 |
  |<-- reconstructed ---|                     |                  |                 |
  |                     |                     |                  |                 |
  |         [tier 0, policy=Factor?]          |                  |                 |
  |                     |-- load_factors -----|------------------|---------------->|
  |                     |<--- U, S, V --------|------------------|-----------------|
  |                     |-- reconstruct_svd ->|                  |                 |
  |<-- reconstructed ---|                     |                  |                 |
```

### 6.2 Read Implementation

```rust
/// Result of reading a block. Contains the f32 data or an error.
pub type ReadResult = Result<Vec<f32>, ReadError>;

/// Read a block, performing reconstruction if necessary.
pub fn read_block(
    meta: &BlockMeta,
    tier_files: &TierDataFiles,
    delta_store: &DeltaStore,
    factor_store: &FactorStore,
    zero_fill_on_evict: bool,
    out: &mut Vec<f32>,
) -> Result<(), ReadError> {
    match meta.tier {
        // --- Tier 1/2/3: quantized data present ---
        1 | 2 | 3 => {
            let raw = tier_files
                .read_range(meta.tier, meta.data_offset, meta.data_len)
                .map_err(|_| ReadError::BlockNotFound)?;

            // Dequantize into caller buffer using the segment decode path
            // from ADR-017. The raw bytes include the TQTC segment header.
            out.clear();
            crate::segment::decode(&raw, out);
            if out.is_empty() {
                return Err(ReadError::BlockNotFound);
            }
            Ok(())
        }

        // --- Tier 0: evicted, attempt reconstruction ---
        0 => match meta.reconstruct_policy {
            ReconstructPolicy::None => {
                if zero_fill_on_evict {
                    // Return a zero-filled buffer of the expected size.
                    // The block_size is derived from tensor metadata.
                    out.clear();
                    out.resize(block_size_from_meta(meta), 0.0);
                    Ok(())
                } else {
                    Err(ReadError::TensorEvicted)
                }
            }

            ReconstructPolicy::Delta => {
                reconstruct_via_delta(meta, tier_files, delta_store, out)
            }

            ReconstructPolicy::Factor => {
                reconstruct_via_factor(meta, factor_store, out)
            }
        },

        _ => Err(ReadError::BlockNotFound),
    }
}

/// Reconstruct a Tier0 block by loading the base and applying the
/// delta chain up to the target epoch.
fn reconstruct_via_delta(
    meta: &BlockMeta,
    tier_files: &TierDataFiles,
    delta_store: &DeltaStore,
    out: &mut Vec<f32>,
) -> Result<(), ReadError> {
    // 1. Load the base block (stored in an older tier or factor file).
    let base_raw = tier_files
        .read_base(meta.tensor_id, meta.base_epoch)
        .map_err(|_| ReadError::DeltaChainBroken { depth: 0 })?;

    out.clear();
    crate::segment::decode(&base_raw, out);
    if out.is_empty() {
        return Err(ReadError::DeltaChainBroken { depth: 0 });
    }

    // 2. Load and apply deltas sequentially (oldest to newest).
    let deltas = delta_store
        .load_chain(meta.tensor_id, meta.block_index, meta.base_epoch, meta.epoch)
        .map_err(|_| ReadError::DeltaChainBroken {
            depth: meta.delta_chain_len,
        })?;

    for (i, delta) in deltas.iter().enumerate() {
        apply_delta(out, delta).map_err(|_| ReadError::DeltaChainBroken {
            depth: i as u16 + 1,
        })?;
    }

    Ok(())
}

/// Apply a single sparse delta to a mutable f32 buffer.
fn apply_delta(buf: &mut [f32], delta: &DeltaRecord) -> Result<(), ReadError> {
    let scale = delta.header.delta_scale;
    for pair in &delta.pairs {
        let idx = pair.index as usize;
        if idx >= buf.len() {
            return Err(ReadError::BufferTooSmall {
                needed: idx + 1,
                provided: buf.len(),
            });
        }
        buf[idx] += (pair.value as f32) * scale;
    }
    Ok(())
}

/// Reconstruct a Tier0 block from stored SVD factors.
fn reconstruct_via_factor(
    meta: &BlockMeta,
    factor_store: &FactorStore,
    out: &mut Vec<f32>,
) -> Result<(), ReadError> {
    let factors = factor_store
        .load(meta.tensor_id, meta.block_index)
        .map_err(|_| ReadError::FactorMissing)?;

    // factors.u: [m x k], factors.s: [k], factors.v: [k x n]
    // Reconstruct: out[i][j] = sum_r( U[i][r] * S[r] * V[r][j] )
    let m = factors.m;
    let n = factors.n;
    let k = factors.k;

    out.clear();
    out.resize(m * n, 0.0);

    for r in 0..k {
        let s_r = factors.s[r];
        for i in 0..m {
            let u_ir = factors.u[i * k + r];
            let u_s = u_ir * s_r;
            for j in 0..n {
                out[i * n + j] += u_s * factors.v[r * n + j];
            }
        }
    }

    Ok(())
}
```

---

## 7. Write Path

### 7.1 Write Path -- Full Replace

```
Caller                BlockStore           Quantizer           TierDataFile
  |                       |                    |                    |
  |-- write_block(data) ->|                    |                    |
  |                       |-- select_tier ---->|                    |
  |                       |<-- bits, tier -----|                    |
  |                       |-- quantize ------->|                    |
  |                       |<-- segment bytes --|                    |
  |                       |-- write_segment ---|-------------------->|
  |                       |-- update_meta ---->|                    |
  |<-- Ok ----------------|                    |                    |
```

```rust
/// Write a full block replacement. Quantizes at the current tier and
/// stores the complete representation, discarding any prior data.
pub fn write_block_full(
    meta: &mut BlockMeta,
    data: &[f32],
    policy: &TierPolicy,
    tier_files: &mut TierDataFiles,
    now_ts: u32,
) -> Result<(), WriteError> {
    // 1. Determine tier from access pattern.
    let bits = policy.select_bits(meta.access_count, meta.last_access_ts, now_ts);
    let tier = tier_from_bits(bits);

    // 2. Quantize via ADR-017 segment encoding.
    let group_len = policy.group_len as usize;
    let scales = crate::quantizer::compute_scales(data, group_len, bits);
    let mut packed = Vec::new();
    crate::quantizer::quantize_and_pack(&scales, &scales, group_len, bits, &mut packed);

    let mut segment = Vec::new();
    crate::segment::encode(
        bits,
        policy.group_len,
        data.len() as u32,
        1, // single frame
        &scales,
        &packed,
        &mut segment,
    );

    // 3. Write segment bytes to the appropriate tier data file.
    let (offset, len) = tier_files.append(tier, &segment)?;

    // 4. Update metadata.
    meta.tier = tier;
    meta.bits = bits;
    meta.data_offset = offset;
    meta.data_len = len as u32;
    meta.epoch += 1;
    meta.delta_chain_len = 0;
    meta.base_epoch = meta.epoch;

    Ok(())
}
```

### 7.2 Write Path -- Delta Write

```
Caller                BlockStore           DeltaEncoder         DeltaStore
  |                       |                    |                    |
  |-- write_delta(data) ->|                    |                    |
  |                       |-- diff vs current->|                    |
  |                       |<-- changed_frac ---|                    |
  |                       |                    |                    |
  |            [changed_frac < p?]             |                    |
  |                       |-- encode_sparse -->|                    |
  |                       |<-- DeltaRecord ----|                    |
  |                       |-- store_delta -----|-------------------->|
  |                       |-- update_meta ---->|                    |
  |<-- Ok(DeltaStored) ---|                    |                    |
  |                       |                    |                    |
  |            [changed_frac >= p?]            |                    |
  |                       |-- write_block_full (see 7.1)            |
  |<-- Ok(FullReplace) ---|                    |                    |
```

```rust
/// Decision thresholds for delta vs full write.
#[derive(Clone, Copy, Debug)]
pub struct DeltaPolicy {
    /// Maximum fraction of changed elements to use delta encoding.
    /// If the fraction exceeds this, a full write is performed instead.
    pub max_changed_fraction: f32,   // default: 0.10 (10%)

    /// Maximum L2 norm of the delta relative to the block norm.
    /// Prevents delta encoding when the change is large in magnitude.
    pub max_relative_delta_norm: f32, // default: 0.05 (5%)

    /// Maximum number of deltas in a chain before compaction is forced.
    pub max_delta_chain: u16,         // default: 8
}

impl Default for DeltaPolicy {
    fn default() -> Self {
        Self {
            max_changed_fraction: 0.10,
            max_relative_delta_norm: 0.05,
            max_delta_chain: 8,
        }
    }
}

/// Outcome of a write operation.
#[derive(Debug)]
pub enum WriteOutcome {
    DeltaStored,
    FullReplace,
}

/// Attempt a delta write. Falls back to full replace when the change is
/// too large or the delta chain has reached its maximum depth.
pub fn write_block_delta(
    meta: &mut BlockMeta,
    old_data: &[f32],
    new_data: &[f32],
    delta_policy: &DeltaPolicy,
    tier_policy: &TierPolicy,
    tier_files: &mut TierDataFiles,
    delta_store: &mut DeltaStore,
    now_ts: u32,
) -> Result<WriteOutcome, WriteError> {
    assert_eq!(old_data.len(), new_data.len());
    let n = old_data.len();

    // 1. Compute diff statistics.
    let mut changed_count: usize = 0;
    let mut delta_norm_sq: f64 = 0.0;
    let mut block_norm_sq: f64 = 0.0;

    for i in 0..n {
        let diff = (new_data[i] - old_data[i]) as f64;
        block_norm_sq += (old_data[i] as f64) * (old_data[i] as f64);
        if diff.abs() > 1e-9 {
            changed_count += 1;
            delta_norm_sq += diff * diff;
        }
    }

    let changed_frac = changed_count as f32 / n as f32;
    let relative_norm = if block_norm_sq > 0.0 {
        (delta_norm_sq / block_norm_sq).sqrt() as f32
    } else {
        f32::MAX
    };

    // 2. Decision: delta or full replace?
    let chain_full = meta.delta_chain_len >= delta_policy.max_delta_chain;
    let change_too_large = changed_frac > delta_policy.max_changed_fraction
        || relative_norm > delta_policy.max_relative_delta_norm;

    if chain_full || change_too_large {
        write_block_full(meta, new_data, tier_policy, tier_files, now_ts)?;
        return Ok(WriteOutcome::FullReplace);
    }

    // 3. Encode sparse delta.
    let max_abs_delta = old_data
        .iter()
        .zip(new_data.iter())
        .map(|(a, b)| (b - a).abs())
        .fold(0.0f32, f32::max);

    let delta_scale = if max_abs_delta == 0.0 {
        1.0
    } else {
        max_abs_delta / i16::MAX as f32
    };
    let inv_scale = 1.0 / delta_scale;

    let mut pairs = Vec::with_capacity(changed_count);
    for i in 0..n {
        let diff = new_data[i] - old_data[i];
        if diff.abs() > 1e-9 {
            let quantized = (diff * inv_scale).round() as i16;
            pairs.push(DeltaPair {
                index: i as u16,
                value: quantized,
            });
        }
    }

    let record = DeltaRecord {
        header: DeltaHeader {
            tensor_id: meta.tensor_id,
            block_index: meta.block_index,
            base_epoch: meta.base_epoch,
            nnz: pairs.len() as u16,
            delta_scale,
        },
        pairs,
    };

    // 4. Store delta and update metadata.
    delta_store.append(&record)?;
    meta.epoch += 1;
    meta.delta_chain_len += 1;

    Ok(WriteOutcome::DeltaStored)
}
```

---

## 8. Delta Chain Management

### 8.1 Chain Depth Bound

The `max_delta_chain` parameter (default: 8) bounds the number of deltas that
can be chained before compaction. This guarantees that delta-based
reconstruction is bounded by O(K * N) where K <= `max_delta_chain` and N is
the block size.

At 8 deltas with an average sparsity of 10%, the read amplification is:

```
base_read + 8 * 0.10 * N * 4 bytes  =  base_read + 3.2 * N bytes
```

For a 512-element block this is `base_read + 6.4 KB`, well within acceptable
latency.

### 8.2 Compaction Algorithm

```
DeltaStore         Compactor           TierDataFile        MetadataStore
    |                  |                    |                    |
    |-- chain_len > K? |                    |                    |
    |                  |                    |                    |
    |-- load_base ---->|                    |                    |
    |<-- base f32 -----|                    |                    |
    |                  |                    |                    |
    |-- load_deltas -->|                    |                    |
    |<-- [d0..dK] -----|                    |                    |
    |                  |                    |                    |
    |       [apply d0, d1, ..., dK]         |                    |
    |                  |                    |                    |
    |-- quantize ----->|                    |                    |
    |<-- new segment --|                    |                    |
    |                  |-- write_segment -->|                    |
    |                  |-- delete_deltas -->|                    |
    |                  |-- update_meta -----|-------------------->|
    |<-- compacted ----|                    |                    |
```

```rust
/// Compact a delta chain into a new base block.
///
/// This is the primary mechanism for bounding read latency. When
/// `meta.delta_chain_len` exceeds `max_delta_chain`, the compactor:
///   1. Loads the base block and decodes it to f32.
///   2. Applies all deltas in epoch order.
///   3. Re-quantizes at the current tier.
///   4. Stores the result as a new base, deletes old deltas.
pub fn compact_delta_chain(
    meta: &mut BlockMeta,
    tier_policy: &TierPolicy,
    tier_files: &mut TierDataFiles,
    delta_store: &mut DeltaStore,
    now_ts: u32,
) -> Result<(), CompactionError> {
    // 1. Load and decode the base block.
    let base_raw = tier_files
        .read_base(meta.tensor_id, meta.base_epoch)
        .map_err(|_| CompactionError::BaseMissing)?;

    let mut buffer = Vec::new();
    crate::segment::decode(&base_raw, &mut buffer);
    if buffer.is_empty() {
        return Err(CompactionError::BaseDecodeFailed);
    }

    // 2. Load and apply all deltas in order.
    let deltas = delta_store
        .load_chain(
            meta.tensor_id,
            meta.block_index,
            meta.base_epoch,
            meta.epoch,
        )
        .map_err(|_| CompactionError::DeltaLoadFailed)?;

    for delta in &deltas {
        let scale = delta.header.delta_scale;
        for pair in &delta.pairs {
            let idx = pair.index as usize;
            if idx < buffer.len() {
                buffer[idx] += (pair.value as f32) * scale;
            }
        }
    }

    // 3. Re-quantize at the current tier.
    let bits = tier_policy.select_bits(meta.access_count, meta.last_access_ts, now_ts);
    let tier = tier_from_bits(bits);
    let group_len = tier_policy.group_len as usize;

    let scales = crate::quantizer::compute_scales(&buffer, group_len, bits);
    let mut packed = Vec::new();
    crate::quantizer::quantize_and_pack(&scales, &scales, group_len, bits, &mut packed);

    let mut segment = Vec::new();
    crate::segment::encode(
        bits,
        tier_policy.group_len,
        buffer.len() as u32,
        1,
        &scales,
        &packed,
        &mut segment,
    );

    let (offset, len) = tier_files.append(tier, &segment)?;

    // 4. Delete old deltas and the old base.
    delta_store.delete_chain(
        meta.tensor_id,
        meta.block_index,
        meta.base_epoch,
        meta.epoch,
    )?;

    // 5. Update metadata to reflect the new base.
    meta.tier = tier;
    meta.bits = bits;
    meta.data_offset = offset;
    meta.data_len = len as u32;
    meta.base_epoch = meta.epoch;
    meta.delta_chain_len = 0;

    Ok(())
}

/// Map bit width to tier number.
fn tier_from_bits(bits: u8) -> u8 {
    match bits {
        8 => 1,
        7 | 5 => 2,
        3 => 3,
        0 => 0,
        _ => 3, // conservative fallback
    }
}
```

---

## 9. Compression to Zero (Tier0 Eviction)

When a block is evicted to Tier0:

1. The data bytes in the tier data file are logically deleted (marked free for
   reuse or physically removed during compaction).
2. `meta.bits` is set to 0 and `meta.tier` is set to 0.
3. `meta.data_len` is set to 0.
4. The reconstruction policy determines whether a base snapshot and/or delta
   chain are preserved.

```rust
/// Evict a block to Tier0. Optionally preserves reconstruction data.
pub fn evict_to_tier0(
    meta: &mut BlockMeta,
    policy: ReconstructPolicy,
    tier_files: &mut TierDataFiles,
) -> Result<(), EvictionError> {
    // Delete the data from the tier file.
    if meta.data_len > 0 {
        tier_files.mark_free(meta.tier, meta.data_offset, meta.data_len)?;
    }

    meta.tier = 0;
    meta.bits = 0;
    meta.data_offset = 0;
    meta.data_len = 0;
    meta.reconstruct_policy = policy;

    // When policy is None, also delete any delta chain and factors
    // to reclaim storage immediately.
    // When policy is Delta or Factor, the associated stores are preserved.

    Ok(())
}
```

---

## 10. Factor Reconstruction (SVD-Based)

### 10.1 Factor File Format

```
FactorRecord:

Offset  Size      Field    Description
------  --------  -------- ------------------------------------------
0       16        id       u128 LE - tensor_id
16      4         block    u32 LE  - block_index
20      4         m        u32 LE  - rows of U
24      4         n        u32 LE  - cols of V
28      4         k        u32 LE  - truncation rank
32      m*k*4     u_data   f32 LE  - U matrix (row-major)
32+m*k*4  k*4     s_data   f32 LE  - singular values
...       k*n*4   v_data   f32 LE  - V matrix (row-major)
```

### 10.2 Factor Store Structures

```rust
/// Stored low-rank factors for SVD-based reconstruction.
#[derive(Clone, Debug)]
pub struct FactorRecord {
    pub tensor_id: u128,
    pub block_index: u32,
    pub m: usize,   // rows
    pub n: usize,   // cols
    pub k: usize,   // truncation rank, k << min(m, n)
    pub u: Vec<f32>, // m x k, row-major
    pub s: Vec<f32>, // k singular values
    pub v: Vec<f32>, // k x n, row-major
}

impl FactorRecord {
    /// Storage cost in bytes (excluding header overhead).
    pub fn storage_bytes(&self) -> usize {
        (self.m * self.k + self.k + self.k * self.n) * 4
    }

    /// Reconstruction error bound: sum of discarded singular values
    /// (Eckart-Young theorem). The caller computes the full SVD and
    /// provides only the top-k factors.
    pub fn is_worthwhile(&self, full_block_bytes: usize) -> bool {
        self.storage_bytes() < full_block_bytes / 2
    }
}
```

Factor reconstruction is most effective for tensors with low effective rank,
such as attention weight matrices where the top 32-64 singular values capture
over 95% of the Frobenius norm.

---

## 11. Failure Modes and Mitigations

### 11.1 Delta Chain Blowup

**Symptom**: Reads become progressively slower as chains grow.

**Root cause**: Compaction not triggered, or `max_delta_chain` set too high.

**Mitigation**: The write path checks `delta_chain_len >= max_delta_chain`
before every delta write and forces a full replace (which resets the chain).
Background compaction runs when `chain_len > max_delta_chain / 2` to stay
ahead of the threshold.

**Monitoring**: Expose `max_chain_len` and `avg_chain_len` as metrics on the
`BlockStore`. Alert when `max_chain_len` approaches 80% of `max_delta_chain`.

### 11.2 Scale Instability (Outlier Sensitivity)

**Symptom**: Quality drops sharply on blocks with outlier values, particularly
at 3-bit quantization where `qmax = 3`.

**Root cause**: A single outlier in a group inflates the scale, crushing the
dynamic range available for all other values.

**Mitigation**:

1. **Outlier clamping**: Before computing scales, clamp values at the 99.9th
   percentile of absolute values within each group. Outliers beyond the clamp
   are stored separately as sparse corrections (same format as delta pairs).

2. **Two-level scale for 3-bit**: Use a per-block coarse scale and a per-group
   fine scale. The fine scale is a 4-bit multiplier (0.25x to 4.0x) applied on
   top of the coarse scale. This provides 16 sub-ranges within the block's
   dynamic range.

3. **Per-group scale inside block**: Already implemented in ADR-017. Groups of
   64 elements each get their own scale, limiting outlier blast radius to 64
   values.

### 11.3 Base Block Loss

**Symptom**: Delta reconstruction fails with `DeltaChainBroken { depth: 0 }`.

**Root cause**: The base block referenced by the delta chain was deleted or
corrupted.

**Mitigation**: Base blocks referenced by active delta chains are pinned and
cannot be freed by tier file compaction. The eviction path must verify that no
active delta chains reference a base before releasing it. The metadata field
`base_epoch` serves as the foreign key for this reference check.

---

## 12. Configuration

All parameters described in this ADR are consolidated into `DeltaPolicy` and
`ReconstructPolicy`, both attached to the per-tensor or per-collection
`TierPolicy`. The full configuration surface:

| Parameter | Location | Default | Description |
|-----------|----------|---------|-------------|
| `evict_min_score` | TierPolicy | 4 | Score threshold for Tier0 eviction |
| `reconstruct_policy` | BlockMeta | None | Per-block reconstruction strategy |
| `zero_fill_on_evict` | Global config | false | Return zeros instead of error for Tier0/None |
| `max_changed_fraction` | DeltaPolicy | 0.10 | Fraction threshold for delta vs full write |
| `max_relative_delta_norm` | DeltaPolicy | 0.05 | Norm threshold for delta vs full write |
| `max_delta_chain` | DeltaPolicy | 8 | Maximum chain depth before compaction |

---

## 13. Alternatives Considered

### 13.1 Unbounded Delta Chains with Periodic Checkpoints

**Rejected**. Periodic checkpoints (every N epochs regardless of chain length)
waste storage when the tensor is not being modified. Bounded chains with
on-demand compaction are more space-efficient and simpler to reason about.

### 13.2 Full Copy-on-Write for Every Update

**Rejected**. For tensors changing by less than 10% per update, COW quadruples
write amplification compared to sparse deltas. The delta path reduces write
volume by 80-90% for typical incremental updates.

### 13.3 LZ4/Zstd Compression Instead of Delta Encoding

**Rejected**. General-purpose compression does not exploit the semantic
structure of tensor updates (sparse changes, known value distributions). Delta
encoding provides better compression for the specific access pattern, and
avoids adding external dependencies to the WASM-compatible core.

### 13.4 Unlimited Factor Rank

**Rejected**. Storing factors with rank k = min(m, n) provides exact
reconstruction but offers no compression. The truncation rank must be bounded
such that `factor_bytes < 0.5 * full_block_bytes` for the factor policy to be
worthwhile.

---

## 14. Acceptance Criteria

- [ ] Tier0 eviction reduces per-block storage to metadata only (0 data bytes)
- [ ] Delta reconstruction produces correct output for chain depths 1 through `max_delta_chain`
- [ ] Factor reconstruction matches SVD reference within floating-point tolerance
- [ ] Delta writes with <10% change use <20% of the bytes of a full write
- [ ] Compaction reduces chain length to 0 and produces a valid base block
- [ ] Read latency for delta reconstruction at chain depth 8 is under 50us for 512-dim blocks
- [ ] All structures serialise/deserialise correctly on both native and WASM targets
- [ ] `ReconstructPolicy::None` with `zero_fill_on_evict = false` returns `TensorEvicted` error
- [ ] `ReconstructPolicy::None` with `zero_fill_on_evict = true` returns a zero-filled buffer

---

## 15. References

1. ADR-017: Temporal Tensor Compression with Tiered Quantization (2026-02-06)
2. ADR-018: Block-Based Storage Engine (parent, in progress)
3. Eckart, C. and Young, G. "The approximation of one matrix by another of lower rank." Psychometrika 1(3), 1936.
4. Pelkonen, T., et al. "Gorilla: A Fast, Scalable, In-Memory Time Series Database." VLDB 2015.
5. Frantar, E., et al. "GPTQ: Accurate Post-Training Quantization for Generative Pre-trained Transformers." ICLR 2023.
6. Liu, Z., et al. "KIVI: A Tuning-Free Asymmetric 2bit Quantization for KV Cache." ICML 2024.
