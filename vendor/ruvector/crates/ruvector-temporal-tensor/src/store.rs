//! Block-based storage engine for temporal tensor compression (ADR-018).
//!
//! Provides tiered quantized storage with CRC32 integrity checking,
//! access-pattern tracking, and eviction support. Each block of tensor
//! data is quantized at the bit width appropriate for its storage tier
//! and tracked with rich metadata for tier-promotion/demotion decisions.
//!
//! # Storage Tiers
//!
//! | Tier  | Bits | Description                         |
//! |-------|------|-------------------------------------|
//! | Tier0 | 0    | Evicted: metadata only, no payload  |
//! | Tier1 | 8    | Hot: full fidelity quantization      |
//! | Tier2 | 7    | Warm: moderate compression           |
//! | Tier3 | 3    | Cold: aggressive compression         |
//!
//! # Example
//!
//! ```rust
//! use ruvector_temporal_tensor::store::{BlockKey, Tier, TieredStore, ReconstructPolicy};
//!
//! let mut store = TieredStore::new(4096);
//! let key = BlockKey { tensor_id: 1, block_index: 0 };
//! let data = vec![1.0f32; 64];
//!
//! store.put(key, &data, Tier::Tier1, 0).unwrap();
//! assert_eq!(store.block_count(), 1);
//!
//! let mut out = vec![0.0f32; 64];
//! let n = store.get(key, &mut out, 1).unwrap();
//! assert_eq!(n, 64);
//! ```

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Unique identifier for a tensor block.
///
/// Composed of the owning tensor's 128-bit ID and a block index within
/// that tensor, allowing fine-grained block-level storage and retrieval.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BlockKey {
    pub tensor_id: u128,
    pub block_index: u32,
}

/// Storage tier for a block.
///
/// Tiers form a hierarchy from hot (high fidelity, fast access) to evicted
/// (metadata-only, zero payload bytes).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum Tier {
    /// Evicted: compressed to zero bits, only metadata remains.
    Tier0 = 0,
    /// Hot: 8-bit quantization, full fidelity.
    Tier1 = 1,
    /// Warm: 7-bit quantization.
    Tier2 = 2,
    /// Cold: 3-bit quantization.
    Tier3 = 3,
}

/// Data type of the original tensor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum DType {
    F32 = 0,
    F16 = 1,
    BF16 = 2,
}

/// Reconstruction policy for evicted (Tier0) blocks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ReconstructPolicy {
    /// No reconstruction possible. Reads fail or return zeros.
    None = 0,
    /// Reconstruct from base + delta chain.
    Delta = 1,
    /// Reconstruct from stored low-rank factors.
    Factor = 2,
}

/// Complete metadata for a single block.
#[derive(Clone, Debug)]
pub struct BlockMeta {
    pub key: BlockKey,
    pub dtype: DType,
    pub tier: Tier,
    /// Quantization bit width (8, 7, 5, or 3).
    pub bits: u8,
    /// Quantization scale: `max(|v|) / qmax`.
    pub scale: f32,
    /// Quantization zero point (0 for symmetric).
    pub zero_point: i16,
    /// Tick at which this block was created.
    pub created_at: u64,
    /// Tick of the most recent access.
    pub last_access_at: u64,
    /// Cumulative access count.
    pub access_count: u32,
    /// Exponential moving average of access rate.
    pub ema_rate: f32,
    /// Sliding-window bitset for the last 64 ticks.
    pub window: u64,
    /// CRC32 checksum of quantized payload concatenated with scale bytes.
    pub checksum: u32,
    /// How to reconstruct if evicted.
    pub reconstruct: ReconstructPolicy,
    /// Number of ticks spent in the current tier.
    pub tier_age: u32,
    /// Optional parent tensor ID for delta-chain lineage.
    pub lineage_parent: Option<u128>,
    /// Size of this block's quantized payload in bytes.
    pub block_bytes: u32,
}

/// Errors produced by the storage engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    /// The block has been evicted to Tier0 and cannot be read directly.
    TensorEvicted,
    /// No block exists for the given key.
    BlockNotFound,
    /// CRC32 verification failed after read.
    ChecksumMismatch,
    /// An underlying I/O operation failed.
    IOError,
    /// The memory budget has been exhausted.
    BudgetExhausted,
    /// The block data is malformed or invalid.
    InvalidBlock,
    /// A delta reconstruction chain exceeded the maximum depth.
    DeltaChainTooLong,
    /// Reconstruction of an evicted block failed.
    ReconstructionFailed,
    /// The provided data is malformed or could not be parsed.
    InvalidData,
    /// The delta chain is at maximum length and cannot accept more deltas.
    ChainFull,
}

// ---------------------------------------------------------------------------
// Traits
// ---------------------------------------------------------------------------

/// Clock abstraction for deterministic time in tests and production.
pub trait Clock {
    /// Returns the current tick count.
    fn now_ticks(&self) -> u64;
}

/// Block I/O for reading and writing raw quantized data.
pub trait BlockIO {
    /// Read quantized bytes for `key` from the given `tier` into `dst`.
    /// Returns the number of bytes written to `dst`.
    fn read_block(&self, tier: Tier, key: BlockKey, dst: &mut [u8]) -> Result<usize, StoreError>;

    /// Write raw quantized bytes `src` for `key` into the given `tier`.
    fn write_block(&mut self, tier: Tier, key: BlockKey, src: &[u8]) -> Result<(), StoreError>;

    /// Delete the raw data for `key` from the given `tier`.
    fn delete_block(&mut self, tier: Tier, key: BlockKey) -> Result<(), StoreError>;
}

/// Metadata log for append-only persistence of block metadata.
pub trait MetaLog {
    /// Append (or upsert) a metadata record.
    fn append(&mut self, rec: &BlockMeta) -> Result<(), StoreError>;

    /// Look up metadata by key.
    fn get(&self, key: BlockKey) -> Option<&BlockMeta>;

    /// Iterate over all metadata records.
    fn iter(&self) -> Box<dyn Iterator<Item = &BlockMeta> + '_>;
}

// ---------------------------------------------------------------------------
// CRC32
// ---------------------------------------------------------------------------

/// Compute CRC32 using the standard reflected polynomial (0xEDB88320).
///
/// This is the same algorithm used by zlib/gzip/PNG. No lookup table is
/// used to keep the binary small; the byte-at-a-time loop is sufficient
/// for the block sizes involved.
pub fn crc32(data: &[u8]) -> u32 {
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
    !crc
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the default bit width for a storage tier.
fn bits_for_tier(tier: Tier) -> u8 {
    match tier {
        Tier::Tier0 => 0,
        Tier::Tier1 => 8,
        Tier::Tier2 => 7,
        Tier::Tier3 => 3,
    }
}

/// Compute the maximum representable signed magnitude for a given bit width.
///
/// `qmax = 2^(bits-1) - 1`. Returns 0 for invalid widths.
#[inline]
fn qmax(bits: u8) -> i32 {
    if bits == 0 || bits > 8 {
        return 0;
    }
    (1i32 << (bits - 1)) - 1
}

/// Internal representation of a stored quantized block.
struct BlockData {
    /// Number of original f32 elements (needed for exact dequantization).
    element_count: u32,
    /// Packed quantized bytes.
    packed: Vec<u8>,
}

/// Quantize an f32 slice using symmetric quantization at the given bit width.
///
/// Returns the packed byte vector and the computed scale factor.
fn quantize_block(data: &[f32], bits: u8) -> (Vec<u8>, f32) {
    let qm = qmax(bits);
    if qm == 0 || data.is_empty() {
        return (Vec::new(), 0.0);
    }
    let qm_f = qm as f32;

    // Find the maximum finite absolute value.
    let max_abs = data
        .iter()
        .filter(|v| v.is_finite())
        .fold(0.0f32, |acc, v| acc.max(v.abs()));

    let scale = if max_abs == 0.0 { 0.0 } else { max_abs / qm_f };
    let inv_scale = if scale == 0.0 { 0.0 } else { 1.0 / scale };

    let bits_u32 = bits as u32;
    let needed = (data.len() * bits as usize).div_ceil(8);
    let mut packed = Vec::with_capacity(needed);

    let mut acc: u64 = 0;
    let mut acc_bits: u32 = 0;

    for &v in data {
        let q = if v.is_finite() {
            (v * inv_scale).round() as i32
        } else {
            0
        }
        .clamp(-qm, qm);

        let u = (q + qm) as u32;
        acc |= (u as u64) << acc_bits;
        acc_bits += bits_u32;

        while acc_bits >= 8 {
            packed.push((acc & 0xFF) as u8);
            acc >>= 8;
            acc_bits -= 8;
        }
    }

    if acc_bits > 0 {
        packed.push((acc & 0xFF) as u8);
    }

    (packed, scale)
}

/// Dequantize packed bytes back to f32 using the given scale and bit width.
///
/// Writes up to `count` values into `out` and returns how many were written.
fn dequantize_block(packed: &[u8], scale: f32, bits: u8, count: usize, out: &mut [f32]) -> usize {
    let qm = qmax(bits);
    if qm == 0 || packed.is_empty() {
        return 0;
    }

    let bits_u32 = bits as u32;
    let mask = (1u64 << bits_u32) - 1;
    let limit = count.min(out.len());

    let mut acc: u64 = 0;
    let mut acc_bits: u32 = 0;
    let mut byte_idx: usize = 0;
    let mut written: usize = 0;

    while written < limit {
        while acc_bits < bits_u32 && byte_idx < packed.len() {
            acc |= (packed[byte_idx] as u64) << acc_bits;
            acc_bits += 8;
            byte_idx += 1;
        }
        if acc_bits < bits_u32 {
            break;
        }

        let u = (acc & mask) as i32;
        acc >>= bits_u32;
        acc_bits -= bits_u32;

        out[written] = (u - qm) as f32 * scale;
        written += 1;
    }

    written
}

/// Compute the CRC32 checksum over quantized payload concatenated with scale.
fn block_checksum(packed: &[u8], scale: f32) -> u32 {
    let scale_bytes = scale.to_le_bytes();
    let total = packed.len() + scale_bytes.len();
    let mut buf = Vec::with_capacity(total);
    buf.extend_from_slice(packed);
    buf.extend_from_slice(&scale_bytes);
    crc32(&buf)
}

// ---------------------------------------------------------------------------
// TickResult
// ---------------------------------------------------------------------------

/// Summary of actions taken during a budgeted maintenance tick.
#[derive(Debug, Default)]
pub struct TickResult {
    /// Number of blocks promoted to a hotter tier.
    pub upgrades: u32,
    /// Number of blocks demoted to a colder tier.
    pub downgrades: u32,
    /// Number of blocks evicted to Tier0.
    pub evictions: u32,
    /// Total bytes freed by evictions and downgrades.
    pub bytes_freed: usize,
    /// Number of budget operations consumed.
    pub ops_used: u32,
    /// Total migration candidates identified before budget limits.
    pub candidates_found: u32,
}

// ---------------------------------------------------------------------------
// Type adapters: store types <-> tiering types
// ---------------------------------------------------------------------------

/// Convert a store [`Tier`] to a [`crate::tiering::Tier`].
fn to_tiering_tier(tier: Tier) -> crate::tiering::Tier {
    match tier {
        Tier::Tier0 => crate::tiering::Tier::Tier0,
        Tier::Tier1 => crate::tiering::Tier::Tier1,
        Tier::Tier2 => crate::tiering::Tier::Tier2,
        Tier::Tier3 => crate::tiering::Tier::Tier3,
    }
}

/// Convert a [`crate::tiering::Tier`] to a store [`Tier`].
fn from_tiering_tier(tier: crate::tiering::Tier) -> Tier {
    match tier {
        crate::tiering::Tier::Tier0 => Tier::Tier0,
        crate::tiering::Tier::Tier1 => Tier::Tier1,
        crate::tiering::Tier::Tier2 => Tier::Tier2,
        crate::tiering::Tier::Tier3 => Tier::Tier3,
    }
}

/// Build a [`crate::tiering::BlockMeta`] from a store [`BlockMeta`] at time `now`.
fn to_tiering_meta(meta: &BlockMeta, now: u64) -> crate::tiering::BlockMeta {
    crate::tiering::BlockMeta {
        ema_rate: meta.ema_rate,
        access_window: meta.window,
        last_access: meta.last_access_at,
        access_count: meta.access_count as u64,
        current_tier: to_tiering_tier(meta.tier),
        tier_since: now.saturating_sub(meta.tier_age as u64),
    }
}

// ---------------------------------------------------------------------------
// TieredStore
// ---------------------------------------------------------------------------

/// In-memory tiered storage engine for quantized tensor blocks.
///
/// Provides put/get with automatic quantization and dequantization,
/// per-block metadata tracking, access-pattern statistics, and
/// eviction to Tier0.
pub struct TieredStore {
    /// Nominal block size hint (bytes). Stored for reference; actual block
    /// sizes are determined by the data passed to [`put`].
    block_bytes: usize,

    /// Block metadata index keyed by [`BlockKey`].
    index: HashMap<BlockKey, BlockMeta>,

    /// Tier1 (hot, 8-bit) quantized data.
    tier1_data: HashMap<BlockKey, BlockData>,
    /// Tier2 (warm, 7-bit) quantized data.
    tier2_data: HashMap<BlockKey, BlockData>,
    /// Tier3 (cold, 3-bit) quantized data.
    tier3_data: HashMap<BlockKey, BlockData>,

    /// Keys present in each tier, for candidate-selection scans.
    tier1_keys: Vec<BlockKey>,
    tier2_keys: Vec<BlockKey>,
    tier3_keys: Vec<BlockKey>,

    /// Witness log for auditing tiering decisions.
    witness_log: crate::metrics::WitnessLog,

    /// Optional coherence checker for read-after-write validation.
    coherence: Option<crate::coherence::CoherenceCheck>,
    /// Epoch tracker for staleness detection.
    epoch_tracker: crate::coherence::EpochTracker,
    /// Metrics time-series for trend analysis.
    metrics_series: crate::metrics::MetricsSeries,
}

/// Smoothing constant for the exponential moving average of access rate.
const EMA_ALPHA: f32 = 0.1;

impl TieredStore {
    /// Create a new store with the given nominal block size (in bytes).
    pub fn new(block_bytes: usize) -> Self {
        Self {
            block_bytes,
            index: HashMap::new(),
            tier1_data: HashMap::new(),
            tier2_data: HashMap::new(),
            tier3_data: HashMap::new(),
            tier1_keys: Vec::new(),
            tier2_keys: Vec::new(),
            tier3_keys: Vec::new(),
            witness_log: crate::metrics::WitnessLog::new(10_000),
            coherence: None,
            epoch_tracker: crate::coherence::EpochTracker::new(),
            metrics_series: crate::metrics::MetricsSeries::new(256),
        }
    }

    /// Nominal block size hint (bytes) configured at construction.
    #[inline]
    pub fn block_bytes(&self) -> usize {
        self.block_bytes
    }

    /// Access the witness log.
    pub fn witness_log(&self) -> &crate::metrics::WitnessLog {
        &self.witness_log
    }

    /// Access the witness log mutably.
    pub fn witness_log_mut(&mut self) -> &mut crate::metrics::WitnessLog {
        &mut self.witness_log
    }

    /// Enable coherence checking with the given configuration.
    ///
    /// When enabled, every `put()` records a write epoch in the epoch tracker.
    /// Callers can use [`coherence_check()`](Self::coherence_check) to validate
    /// read-after-write consistency.
    pub fn enable_coherence(&mut self, check: crate::coherence::CoherenceCheck) {
        self.coherence = Some(check);
    }

    /// Disable coherence checking.
    pub fn disable_coherence(&mut self) {
        self.coherence = None;
    }

    /// Access the epoch tracker.
    pub fn epoch_tracker(&self) -> &crate::coherence::EpochTracker {
        &self.epoch_tracker
    }

    /// Access the epoch tracker mutably.
    pub fn epoch_tracker_mut(&mut self) -> &mut crate::coherence::EpochTracker {
        &mut self.epoch_tracker
    }

    /// Access the metrics time-series.
    pub fn metrics_series(&self) -> &crate::metrics::MetricsSeries {
        &self.metrics_series
    }

    /// Access the metrics time-series mutably.
    pub fn metrics_series_mut(&mut self) -> &mut crate::metrics::MetricsSeries {
        &mut self.metrics_series
    }

    /// Perform a coherence check on a recently written block.
    ///
    /// Returns `None` if coherence checking is not enabled.
    /// Returns `Some(Err(...))` if the block doesn't exist or is evicted.
    /// Returns `Some(Ok(result))` with the coherence result.
    pub fn coherence_check(
        &mut self,
        key: BlockKey,
        original_data: &[f32],
        now: u64,
    ) -> Option<Result<crate::coherence::CoherenceResult, StoreError>> {
        let check = self.coherence.clone()?;
        Some(check.check_coherence(self, key, original_data, now))
    }

    /// Compute current aggregate metrics.
    pub fn metrics(&self) -> crate::metrics::StoreMetrics {
        let mut m = crate::metrics::StoreMetrics::new();
        m.total_blocks = self.index.len() as u64;
        m.tier0_blocks = self
            .index
            .values()
            .filter(|b| b.tier == Tier::Tier0)
            .count() as u64;
        m.tier1_blocks = self.tier1_keys.len() as u64;
        m.tier2_blocks = self.tier2_keys.len() as u64;
        m.tier3_blocks = self.tier3_keys.len() as u64;
        m.tier1_bytes = self
            .tier1_data
            .values()
            .map(|d| d.packed.len() as u64)
            .sum();
        m.tier2_bytes = self
            .tier2_data
            .values()
            .map(|d| d.packed.len() as u64)
            .sum();
        m.tier3_bytes = self
            .tier3_data
            .values()
            .map(|d| d.packed.len() as u64)
            .sum();
        m.total_evictions = self.witness_log.count_evictions() as u64;
        m.tier_flips_last_minute = self.witness_log.tier_flip_rate(60, self.index.len() as u64);
        m
    }

    /// Quantize `data` at the bit width for `tier` and store the block.
    ///
    /// If a block with the same key already exists, it is replaced (the old
    /// data is removed from whatever tier it resided in).
    ///
    /// Returns [`StoreError::InvalidBlock`] if `tier` is [`Tier::Tier0`]
    /// (you cannot directly write to the evicted tier).
    pub fn put(
        &mut self,
        key: BlockKey,
        data: &[f32],
        tier: Tier,
        now: u64,
    ) -> Result<(), StoreError> {
        if tier == Tier::Tier0 {
            return Err(StoreError::InvalidBlock);
        }

        let bits = bits_for_tier(tier);
        let (packed, scale) = quantize_block(data, bits);
        let checksum = block_checksum(&packed, scale);

        // If the key already exists, remove old data first.
        if let Some(old_meta) = self.index.get(&key) {
            let old_tier = old_meta.tier;
            self.remove_data(old_tier, key);
            self.remove_from_bucket(old_tier, key);
        }

        let byte_count = packed.len() as u32;
        let block = BlockData {
            element_count: data.len() as u32,
            packed,
        };

        match tier {
            Tier::Tier1 => {
                self.tier1_data.insert(key, block);
            }
            Tier::Tier2 => {
                self.tier2_data.insert(key, block);
            }
            Tier::Tier3 => {
                self.tier3_data.insert(key, block);
            }
            Tier::Tier0 => unreachable!(),
        }
        self.add_to_bucket(tier, key);

        let meta = BlockMeta {
            key,
            dtype: DType::F32,
            tier,
            bits,
            scale,
            zero_point: 0,
            created_at: now,
            last_access_at: now,
            access_count: 1,
            ema_rate: 0.0,
            window: 1,
            checksum,
            reconstruct: ReconstructPolicy::None,
            tier_age: 0,
            lineage_parent: None,
            block_bytes: byte_count,
        };
        self.index.insert(key, meta);

        // Record witness event for the write.
        self.witness_log.record(
            now,
            crate::metrics::WitnessEvent::Access {
                key,
                score: 0.0,
                tier,
            },
        );

        // Record write epoch for staleness detection.
        self.epoch_tracker.record_write(key);

        Ok(())
    }

    /// Dequantize the block identified by `key` into `out`.
    ///
    /// `now` is the current tick counter, used to update access statistics
    /// and record a witness event.
    ///
    /// Returns the number of f32 elements written to `out`.
    ///
    /// # Errors
    ///
    /// - [`StoreError::TensorEvicted`] if the block resides in Tier0.
    /// - [`StoreError::BlockNotFound`] if no block exists for `key`.
    /// - [`StoreError::ChecksumMismatch`] if the stored checksum does not
    ///   match a freshly computed checksum of the payload.
    pub fn get(&mut self, key: BlockKey, out: &mut [f32], now: u64) -> Result<usize, StoreError> {
        let meta = self.index.get(&key).ok_or(StoreError::BlockNotFound)?;

        if meta.tier == Tier::Tier0 {
            return Err(StoreError::TensorEvicted);
        }

        let tier = meta.tier;
        let scale = meta.scale;
        let bits = meta.bits;
        let checksum = meta.checksum;

        let block = self
            .data_map(tier)
            .and_then(|m| m.get(&key))
            .ok_or(StoreError::BlockNotFound)?;

        // Verify integrity.
        let actual_crc = block_checksum(&block.packed, scale);
        if actual_crc != checksum {
            return Err(StoreError::ChecksumMismatch);
        }

        let n = dequantize_block(
            &block.packed,
            scale,
            bits,
            block.element_count as usize,
            out,
        );

        // Update access statistics.
        self.touch(key, now);

        // Record witness event.
        self.witness_log.record(
            now,
            crate::metrics::WitnessEvent::Access {
                key,
                score: 0.0, // score not computed during basic get
                tier,
            },
        );

        Ok(n)
    }

    /// Update access statistics for `key` at tick `now`.
    ///
    /// Increments `access_count`, refreshes `last_access_at`, updates the
    /// sliding-window bitset, and recalculates the EMA access rate.
    /// Does nothing if the key is not present.
    pub fn touch(&mut self, key: BlockKey, now: u64) {
        if let Some(meta) = self.index.get_mut(&key) {
            let delta = now.saturating_sub(meta.last_access_at);

            // Update sliding-window bitset.
            if delta >= 64 {
                meta.window = 1;
            } else if delta > 0 {
                meta.window = (meta.window << delta) | 1;
            }
            // delta == 0: same tick, window unchanged but count still bumps.

            // Update EMA access rate.
            if delta > 0 {
                let instant_rate = 1.0 / delta as f32;
                meta.ema_rate = EMA_ALPHA * instant_rate + (1.0 - EMA_ALPHA) * meta.ema_rate;
            }

            meta.last_access_at = now;
            meta.access_count = meta.access_count.saturating_add(1);
        }
    }

    /// Return a reference to the metadata for `key`, if it exists.
    pub fn meta(&self, key: BlockKey) -> Option<&BlockMeta> {
        self.index.get(&key)
    }

    /// Total number of blocks tracked (including Tier0 evicted blocks).
    pub fn block_count(&self) -> usize {
        self.index.len()
    }

    /// Number of blocks currently in the given tier.
    pub fn tier_count(&self, tier: Tier) -> usize {
        match tier {
            Tier::Tier0 => self
                .index
                .values()
                .filter(|m| m.tier == Tier::Tier0)
                .count(),
            Tier::Tier1 => self.tier1_keys.len(),
            Tier::Tier2 => self.tier2_keys.len(),
            Tier::Tier3 => self.tier3_keys.len(),
        }
    }

    /// Total bytes of quantized data stored across all active tiers.
    pub fn total_bytes(&self) -> usize {
        let sum = |map: &HashMap<BlockKey, BlockData>| -> usize {
            map.values().map(|b| b.packed.len()).sum()
        };
        sum(&self.tier1_data) + sum(&self.tier2_data) + sum(&self.tier3_data)
    }

    /// Slice of block keys currently residing in the given tier.
    ///
    /// Returns an empty slice for [`Tier::Tier0`].
    pub fn blocks_in_tier(&self, tier: Tier) -> &[BlockKey] {
        match tier {
            Tier::Tier0 => &[],
            Tier::Tier1 => &self.tier1_keys,
            Tier::Tier2 => &self.tier2_keys,
            Tier::Tier3 => &self.tier3_keys,
        }
    }

    /// Evict a block to Tier0, removing its quantized payload.
    ///
    /// The block's metadata is preserved with the specified
    /// [`ReconstructPolicy`] so that higher-level code can decide how
    /// (or whether) to reconstruct the data on future reads.
    ///
    /// Returns [`StoreError::BlockNotFound`] if the key does not exist.
    pub fn evict(&mut self, key: BlockKey, policy: ReconstructPolicy) -> Result<(), StoreError> {
        let meta = self.index.get_mut(&key).ok_or(StoreError::BlockNotFound)?;
        let old_tier = meta.tier;

        if old_tier == Tier::Tier0 {
            // Already evicted; just update the policy.
            meta.reconstruct = policy;
            return Ok(());
        }

        let bytes_freed = meta.block_bytes as usize;
        let evict_ts = meta.last_access_at;

        // Mutate metadata before touching the data maps (avoids a second
        // lookup since we already have the mutable reference).
        meta.tier = Tier::Tier0;
        meta.reconstruct = policy;
        meta.tier_age = 0;
        meta.block_bytes = 0;
        meta.bits = 0;

        // Drop the mutable borrow so we can call helper methods.
        self.remove_data(old_tier, key);
        self.remove_from_bucket(old_tier, key);

        // Record witness event for the eviction.
        self.witness_log.record(
            evict_ts,
            crate::metrics::WitnessEvent::Eviction {
                key,
                score: 0.0,
                bytes_freed,
            },
        );

        Ok(())
    }

    // -- private helpers ----------------------------------------------------

    /// Return a reference to the data map for the given tier.
    fn data_map(&self, tier: Tier) -> Option<&HashMap<BlockKey, BlockData>> {
        match tier {
            Tier::Tier0 => None,
            Tier::Tier1 => Some(&self.tier1_data),
            Tier::Tier2 => Some(&self.tier2_data),
            Tier::Tier3 => Some(&self.tier3_data),
        }
    }

    /// Remove raw data for `key` from the given tier's map.
    fn remove_data(&mut self, tier: Tier, key: BlockKey) {
        match tier {
            Tier::Tier1 => {
                self.tier1_data.remove(&key);
            }
            Tier::Tier2 => {
                self.tier2_data.remove(&key);
            }
            Tier::Tier3 => {
                self.tier3_data.remove(&key);
            }
            Tier::Tier0 => {}
        }
    }

    /// Remove `key` from the tier's candidate-selection bucket.
    fn remove_from_bucket(&mut self, tier: Tier, key: BlockKey) {
        let bucket = match tier {
            Tier::Tier1 => &mut self.tier1_keys,
            Tier::Tier2 => &mut self.tier2_keys,
            Tier::Tier3 => &mut self.tier3_keys,
            Tier::Tier0 => return,
        };
        if let Some(pos) = bucket.iter().position(|k| *k == key) {
            bucket.swap_remove(pos);
        }
    }

    /// Add `key` to the tier's candidate-selection bucket.
    fn add_to_bucket(&mut self, tier: Tier, key: BlockKey) {
        match tier {
            Tier::Tier1 => self.tier1_keys.push(key),
            Tier::Tier2 => self.tier2_keys.push(key),
            Tier::Tier3 => self.tier3_keys.push(key),
            Tier::Tier0 => {}
        }
    }

    // -- tiering-aware methods -----------------------------------------------

    /// Run a budgeted maintenance tick.
    ///
    /// Evaluates all blocks, selects migration candidates, and executes
    /// tier transitions within the given byte and operation budgets.
    /// Returns a summary of actions taken.
    pub fn tick(
        &mut self,
        config: &crate::tiering::TierConfig,
        now: u64,
        budget_bytes: usize,
        budget_ops: u32,
    ) -> TickResult {
        let mut result = TickResult::default();

        // Step 1: Collect all blocks and convert to tiering types.
        // Use sequential indices as tiering::BlockKey values to avoid collisions.
        let store_keys: Vec<BlockKey> = self.index.keys().copied().collect();
        if store_keys.is_empty() {
            return result;
        }

        let tiering_blocks: Vec<(crate::tiering::BlockKey, crate::tiering::BlockMeta)> = store_keys
            .iter()
            .enumerate()
            .map(|(idx, key)| {
                let meta = &self.index[key];
                (
                    crate::tiering::BlockKey(idx as u64),
                    to_tiering_meta(meta, now),
                )
            })
            .collect();

        let blocks_ref: Vec<(crate::tiering::BlockKey, &crate::tiering::BlockMeta)> =
            tiering_blocks.iter().map(|(k, m)| (*k, m)).collect();

        // Step 2: Select migration candidates (upgrades first by highest score,
        // then downgrades by lowest score).
        let candidates = crate::tiering::select_candidates(config, now, &blocks_ref);
        result.candidates_found = candidates.len() as u32;

        // Step 3: Process candidates within budget.
        let mut remaining_bytes = budget_bytes;
        let mut remaining_ops = budget_ops;
        let mut migrated = std::collections::HashSet::new();

        for candidate in &candidates {
            if remaining_ops == 0 {
                break;
            }

            let store_key = store_keys[candidate.key.0 as usize];
            let target_tier = from_tiering_tier(candidate.target_tier);
            let current_tier = from_tiering_tier(candidate.current_tier);

            let old_bytes = self
                .index
                .get(&store_key)
                .map(|m| m.block_bytes as usize)
                .unwrap_or(0);

            // Check byte budget.
            if old_bytes > remaining_bytes {
                continue;
            }

            if target_tier == Tier::Tier0 {
                // Eviction.
                if self.evict(store_key, ReconstructPolicy::None).is_ok() {
                    result.evictions += 1;
                    result.bytes_freed += old_bytes;
                    remaining_ops -= 1;
                    result.ops_used += 1;
                    remaining_bytes = remaining_bytes.saturating_sub(old_bytes);
                    migrated.insert(store_key);
                }
            } else {
                // Tier migration.
                let warm_bytes: usize = self.tier2_data.values().map(|b| b.packed.len()).sum();
                let target_bits =
                    crate::tiering::bits_for_tier(config, to_tiering_tier(target_tier), warm_bytes);

                let old_tier_u8 = current_tier as u8;
                let new_tier_u8 = target_tier as u8;

                if self
                    .migrate_block(store_key, target_tier, target_bits)
                    .is_ok()
                {
                    let new_bytes = self
                        .index
                        .get(&store_key)
                        .map(|m| m.block_bytes as usize)
                        .unwrap_or(0);

                    if new_tier_u8 < old_tier_u8 {
                        // Upgrade (hotter tier).
                        result.upgrades += 1;
                    } else {
                        // Downgrade (colder tier).
                        result.downgrades += 1;
                        result.bytes_freed += old_bytes.saturating_sub(new_bytes);
                    }

                    // Record witness event for the tier change.
                    let reason = if new_tier_u8 < old_tier_u8 {
                        crate::metrics::TierChangeReason::ScoreUpgrade
                    } else {
                        crate::metrics::TierChangeReason::ScoreDowngrade
                    };
                    self.witness_log.record(
                        now,
                        crate::metrics::WitnessEvent::TierChange {
                            key: store_key,
                            from_tier: current_tier,
                            to_tier: target_tier,
                            score: candidate.score,
                            reason,
                        },
                    );

                    remaining_ops -= 1;
                    result.ops_used += 1;
                    remaining_bytes = remaining_bytes.saturating_sub(old_bytes);
                    migrated.insert(store_key);
                }
            }
        }

        // Step 4: For blocks not migrated, increment tier_age and call tick_decay.
        for key in &store_keys {
            if migrated.contains(key) {
                continue;
            }
            if let Some(meta) = self.index.get_mut(key) {
                meta.tier_age = meta.tier_age.saturating_add(1);
                // Apply tick_decay via the tiering module.
                let mut tm = crate::tiering::BlockMeta {
                    ema_rate: meta.ema_rate,
                    access_window: meta.window,
                    last_access: meta.last_access_at,
                    access_count: meta.access_count as u64,
                    current_tier: to_tiering_tier(meta.tier),
                    tier_since: now.saturating_sub(meta.tier_age as u64),
                };
                crate::tiering::tick_decay(config, &mut tm);
                meta.ema_rate = tm.ema_rate;
                meta.window = tm.access_window;
            }
        }

        // Record a maintenance witness event.
        self.witness_log.record(
            now,
            crate::metrics::WitnessEvent::Maintenance {
                upgrades: result.upgrades,
                downgrades: result.downgrades,
                evictions: result.evictions,
                bytes_freed: result.bytes_freed,
                budget_remaining_bytes: remaining_bytes.min(u32::MAX as usize) as u32,
                budget_remaining_ops: remaining_ops,
            },
        );

        // Auto-record a metrics snapshot for trend analysis.
        let snapshot_metrics = self.metrics();
        self.metrics_series.record(now, snapshot_metrics);

        result
    }

    /// Migrate a single block from one tier to another.
    ///
    /// Re-quantizes the data at the target tier's bit width. The block's
    /// metadata is updated with the new tier, bits, scale, checksum, and
    /// `tier_age` is reset to 0.
    fn migrate_block(
        &mut self,
        key: BlockKey,
        target_tier: Tier,
        target_bits: u8,
    ) -> Result<(), StoreError> {
        // Read current metadata (copy fields to release the borrow).
        let meta = self.index.get(&key).ok_or(StoreError::BlockNotFound)?;
        let old_tier = meta.tier;
        let old_bits = meta.bits;
        let old_scale = meta.scale;

        if old_tier == Tier::Tier0 {
            return Err(StoreError::TensorEvicted);
        }
        if target_tier == Tier::Tier0 {
            return Err(StoreError::InvalidBlock);
        }

        // Dequantize the old data to f32 within a limited scope so the
        // immutable borrow on self (through data_map) is released before
        // we need mutable access.
        let (element_count, f32_data) = {
            let block = self
                .data_map(old_tier)
                .and_then(|m| m.get(&key))
                .ok_or(StoreError::BlockNotFound)?;
            let ec = block.element_count;
            let mut data = vec![0.0f32; ec as usize];
            dequantize_block(&block.packed, old_scale, old_bits, ec as usize, &mut data);
            (ec, data)
        };

        // Re-quantize at the target bit width.
        let (packed, scale) = quantize_block(&f32_data, target_bits);
        let checksum = block_checksum(&packed, scale);
        let byte_count = packed.len() as u32;
        let new_block = BlockData {
            element_count,
            packed,
        };

        // Remove from old tier.
        self.remove_data(old_tier, key);
        self.remove_from_bucket(old_tier, key);

        // Insert into target tier.
        match target_tier {
            Tier::Tier1 => {
                self.tier1_data.insert(key, new_block);
            }
            Tier::Tier2 => {
                self.tier2_data.insert(key, new_block);
            }
            Tier::Tier3 => {
                self.tier3_data.insert(key, new_block);
            }
            Tier::Tier0 => unreachable!(),
        }
        self.add_to_bucket(target_tier, key);

        // Update metadata.
        let meta = self.index.get_mut(&key).unwrap();
        meta.tier = target_tier;
        meta.bits = target_bits;
        meta.scale = scale;
        meta.checksum = checksum;
        meta.tier_age = 0;
        meta.block_bytes = byte_count;

        Ok(())
    }

    /// Compute the current score for a block using the enhanced tiering
    /// algorithm (EMA + popcount + recency).
    ///
    /// Returns `None` if the block does not exist.
    pub fn score_block(
        &self,
        key: BlockKey,
        config: &crate::tiering::TierConfig,
        now: u64,
    ) -> Option<f32> {
        let meta = self.index.get(&key)?;
        let tm = to_tiering_meta(meta, now);
        Some(crate::tiering::compute_score(config, now, &tm))
    }

    /// Record an access event using the enhanced tiering algorithm.
    ///
    /// Updates `ema_rate`, `access_window`, `last_access_at`, and
    /// `access_count` using the configurable alpha from [`TierConfig`].
    /// Does nothing if the key is not present.
    pub fn touch_block(&mut self, key: BlockKey, config: &crate::tiering::TierConfig, now: u64) {
        if let Some(meta) = self.index.get_mut(&key) {
            let mut tm = crate::tiering::BlockMeta {
                ema_rate: meta.ema_rate,
                access_window: meta.window,
                last_access: meta.last_access_at,
                access_count: meta.access_count as u64,
                current_tier: to_tiering_tier(meta.tier),
                tier_since: now.saturating_sub(meta.tier_age as u64),
            };
            crate::tiering::touch(config, now, &mut tm);
            meta.ema_rate = tm.ema_rate;
            meta.window = tm.access_window;
            meta.last_access_at = tm.last_access;
            meta.access_count = tm.access_count.min(u32::MAX as u64) as u32;
        }
    }
}

// ---------------------------------------------------------------------------
// Trait implementations for TieredStore
// ---------------------------------------------------------------------------

impl BlockIO for TieredStore {
    fn read_block(&self, tier: Tier, key: BlockKey, dst: &mut [u8]) -> Result<usize, StoreError> {
        let map = self.data_map(tier).ok_or(StoreError::BlockNotFound)?;
        let block = map.get(&key).ok_or(StoreError::BlockNotFound)?;
        let n = block.packed.len().min(dst.len());
        dst[..n].copy_from_slice(&block.packed[..n]);
        Ok(n)
    }

    fn write_block(&mut self, tier: Tier, key: BlockKey, src: &[u8]) -> Result<(), StoreError> {
        if tier == Tier::Tier0 {
            return Err(StoreError::InvalidBlock);
        }
        let block = BlockData {
            element_count: 0, // raw write; element count unknown
            packed: src.to_vec(),
        };
        match tier {
            Tier::Tier1 => {
                self.tier1_data.insert(key, block);
            }
            Tier::Tier2 => {
                self.tier2_data.insert(key, block);
            }
            Tier::Tier3 => {
                self.tier3_data.insert(key, block);
            }
            Tier::Tier0 => unreachable!(),
        }
        Ok(())
    }

    fn delete_block(&mut self, tier: Tier, key: BlockKey) -> Result<(), StoreError> {
        let removed = match tier {
            Tier::Tier1 => self.tier1_data.remove(&key).is_some(),
            Tier::Tier2 => self.tier2_data.remove(&key).is_some(),
            Tier::Tier3 => self.tier3_data.remove(&key).is_some(),
            Tier::Tier0 => false,
        };
        if removed {
            Ok(())
        } else {
            Err(StoreError::BlockNotFound)
        }
    }
}

impl MetaLog for TieredStore {
    fn append(&mut self, rec: &BlockMeta) -> Result<(), StoreError> {
        self.index.insert(rec.key, rec.clone());
        Ok(())
    }

    fn get(&self, key: BlockKey) -> Option<&BlockMeta> {
        self.index.get(&key)
    }

    fn iter(&self) -> Box<dyn Iterator<Item = &BlockMeta> + '_> {
        Box::new(self.index.values())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn make_key(tid: u128, idx: u32) -> BlockKey {
        BlockKey {
            tensor_id: tid,
            block_index: idx,
        }
    }

    // -- CRC32 -------------------------------------------------------------

    #[test]
    fn test_crc32_known_vector() {
        // The CRC32 of the ASCII string "123456789" is 0xCBF43926.
        let data = b"123456789";
        assert_eq!(crc32(data), 0xCBF4_3926);
    }

    #[test]
    fn test_crc32_empty() {
        assert_eq!(crc32(&[]), 0x0000_0000);
    }

    #[test]
    fn test_crc32_single_byte() {
        // CRC32 of [0x00] is 0xD202EF8D.
        assert_eq!(crc32(&[0x00]), 0xD202_EF8D);
    }

    // -- BlockKey hashing --------------------------------------------------

    #[test]
    fn test_block_key_equality() {
        let a = make_key(1, 0);
        let b = make_key(1, 0);
        let c = make_key(1, 1);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_block_key_hash_differs() {
        fn hash_of(k: &BlockKey) -> u64 {
            let mut h = DefaultHasher::new();
            k.hash(&mut h);
            h.finish()
        }
        let a = make_key(1, 0);
        let b = make_key(2, 0);
        let c = make_key(1, 1);
        // Different keys should (almost certainly) hash differently.
        assert_ne!(hash_of(&a), hash_of(&b));
        assert_ne!(hash_of(&a), hash_of(&c));
    }

    #[test]
    fn test_block_key_hash_stable() {
        fn hash_of(k: &BlockKey) -> u64 {
            let mut h = DefaultHasher::new();
            k.hash(&mut h);
            h.finish()
        }
        let a = make_key(42, 7);
        let b = make_key(42, 7);
        assert_eq!(hash_of(&a), hash_of(&b));
    }

    // -- qmax helper -------------------------------------------------------

    #[test]
    fn test_qmax_values() {
        assert_eq!(qmax(8), 127);
        assert_eq!(qmax(7), 63);
        assert_eq!(qmax(5), 15);
        assert_eq!(qmax(3), 3);
        assert_eq!(qmax(1), 0);
        assert_eq!(qmax(0), 0);
        assert_eq!(qmax(9), 0);
    }

    // -- Quantization roundtrip --------------------------------------------

    #[test]
    fn test_quantize_roundtrip_8bit() {
        let data: Vec<f32> = (0..128).map(|i| (i as f32 - 64.0) * 0.1).collect();
        let (packed, scale) = quantize_block(&data, 8);
        let mut out = vec![0.0f32; 128];
        let n = dequantize_block(&packed, scale, 8, 128, &mut out);
        assert_eq!(n, 128);
        for (i, (&orig, &dec)) in data.iter().zip(out.iter()).enumerate() {
            let err = (orig - dec).abs();
            let tol = if orig.abs() > 0.01 {
                orig.abs() * 0.02
            } else {
                0.1
            };
            assert!(err < tol, "i={i} orig={orig} dec={dec} err={err}");
        }
    }

    #[test]
    fn test_quantize_roundtrip_3bit() {
        let data: Vec<f32> = (0..64).map(|i| (i as f32 - 32.0) * 0.5).collect();
        let (packed, scale) = quantize_block(&data, 3);
        let mut out = vec![0.0f32; 64];
        let n = dequantize_block(&packed, scale, 3, 64, &mut out);
        assert_eq!(n, 64);
        let max_val = data.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        for (&orig, &dec) in data.iter().zip(out.iter()) {
            let err = (orig - dec).abs();
            assert!(err < max_val * 0.35, "orig={orig} dec={dec} err={err}");
        }
    }

    #[test]
    fn test_quantize_zeros() {
        let data = vec![0.0f32; 64];
        let (packed, scale) = quantize_block(&data, 8);
        assert_eq!(scale, 0.0);
        let mut out = vec![1.0f32; 64];
        let n = dequantize_block(&packed, scale, 8, 64, &mut out);
        assert_eq!(n, 64);
        for &v in &out {
            assert_eq!(v, 0.0);
        }
    }

    // -- TieredStore put/get -----------------------------------------------

    #[test]
    fn test_store_put_get_roundtrip() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        let data: Vec<f32> = (0..64).map(|i| i as f32 * 0.25).collect();

        store.put(key, &data, Tier::Tier1, 0).unwrap();

        let mut out = vec![0.0f32; 64];
        let n = TieredStore::get(&mut store, key, &mut out, 1).unwrap();
        assert_eq!(n, 64);

        for (i, (&orig, &dec)) in data.iter().zip(out.iter()).enumerate() {
            let err = (orig - dec).abs();
            let tol = if orig.abs() > 0.01 {
                orig.abs() * 0.02
            } else {
                0.15
            };
            assert!(err < tol, "i={i} orig={orig} dec={dec} err={err}");
        }
    }

    #[test]
    fn test_store_put_tier3_roundtrip() {
        let mut store = TieredStore::new(4096);
        let key = make_key(10, 5);
        let data: Vec<f32> = (0..32).map(|i| (i as f32 - 16.0) * 0.5).collect();

        store.put(key, &data, Tier::Tier3, 100).unwrap();

        let meta = store.meta(key).unwrap();
        assert_eq!(meta.tier, Tier::Tier3);
        assert_eq!(meta.bits, 3);
        assert_eq!(meta.created_at, 100);

        let mut out = vec![0.0f32; 32];
        let n = TieredStore::get(&mut store, key, &mut out, 101).unwrap();
        assert_eq!(n, 32);

        let max_val = data.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        for (&orig, &dec) in data.iter().zip(out.iter()) {
            let err = (orig - dec).abs();
            assert!(err < max_val * 0.35, "orig={orig} dec={dec} err={err}");
        }
    }

    #[test]
    fn test_store_get_not_found() {
        let mut store = TieredStore::new(4096);
        let key = make_key(99, 0);
        let mut out = vec![0.0f32; 8];
        assert_eq!(
            TieredStore::get(&mut store, key, &mut out, 0),
            Err(StoreError::BlockNotFound)
        );
    }

    #[test]
    fn test_store_put_tier0_rejected() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        let data = vec![1.0f32; 8];
        assert_eq!(
            store.put(key, &data, Tier::Tier0, 0),
            Err(StoreError::InvalidBlock)
        );
    }

    // -- Eviction ----------------------------------------------------------

    #[test]
    fn test_eviction() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        let data = vec![1.0f32; 64];

        store.put(key, &data, Tier::Tier1, 0).unwrap();
        assert_eq!(store.tier_count(Tier::Tier1), 1);
        assert!(store.total_bytes() > 0);

        store.evict(key, ReconstructPolicy::Delta).unwrap();

        let meta = store.meta(key).unwrap();
        assert_eq!(meta.tier, Tier::Tier0);
        assert_eq!(meta.reconstruct, ReconstructPolicy::Delta);
        assert_eq!(meta.block_bytes, 0);
        assert_eq!(meta.bits, 0);
        assert_eq!(meta.tier_age, 0);

        // Data is gone; read should fail with TensorEvicted.
        let mut out = vec![0.0f32; 64];
        assert_eq!(
            TieredStore::get(&mut store, key, &mut out, 1),
            Err(StoreError::TensorEvicted)
        );

        // Tier1 should be empty; Tier0 count should be 1.
        assert_eq!(store.tier_count(Tier::Tier1), 0);
        assert_eq!(store.tier_count(Tier::Tier0), 1);

        // Block still exists in the index (metadata preserved).
        assert_eq!(store.block_count(), 1);
    }

    #[test]
    fn test_eviction_not_found() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        assert_eq!(
            store.evict(key, ReconstructPolicy::None),
            Err(StoreError::BlockNotFound),
        );
    }

    #[test]
    fn test_eviction_idempotent() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        store.put(key, &[1.0; 16], Tier::Tier2, 0).unwrap();

        store.evict(key, ReconstructPolicy::None).unwrap();
        // Evicting again should succeed and update the policy.
        store.evict(key, ReconstructPolicy::Factor).unwrap();

        let meta = store.meta(key).unwrap();
        assert_eq!(meta.reconstruct, ReconstructPolicy::Factor);
    }

    // -- Tier counts -------------------------------------------------------

    #[test]
    fn test_tier_counts() {
        let mut store = TieredStore::new(4096);
        let data = vec![1.0f32; 16];

        store.put(make_key(1, 0), &data, Tier::Tier1, 0).unwrap();
        store.put(make_key(2, 0), &data, Tier::Tier1, 0).unwrap();
        store.put(make_key(3, 0), &data, Tier::Tier2, 0).unwrap();
        store.put(make_key(4, 0), &data, Tier::Tier3, 0).unwrap();
        store.put(make_key(5, 0), &data, Tier::Tier3, 0).unwrap();
        store.put(make_key(6, 0), &data, Tier::Tier3, 0).unwrap();

        assert_eq!(store.block_count(), 6);
        assert_eq!(store.tier_count(Tier::Tier0), 0);
        assert_eq!(store.tier_count(Tier::Tier1), 2);
        assert_eq!(store.tier_count(Tier::Tier2), 1);
        assert_eq!(store.tier_count(Tier::Tier3), 3);

        assert_eq!(store.blocks_in_tier(Tier::Tier1).len(), 2);
        assert_eq!(store.blocks_in_tier(Tier::Tier0).len(), 0);
    }

    // -- Total bytes -------------------------------------------------------

    #[test]
    fn test_total_bytes() {
        let mut store = TieredStore::new(4096);
        assert_eq!(store.total_bytes(), 0);

        let data = vec![1.0f32; 64];
        store.put(make_key(1, 0), &data, Tier::Tier1, 0).unwrap();
        let bytes_after_one = store.total_bytes();
        assert!(bytes_after_one > 0);

        store.put(make_key(2, 0), &data, Tier::Tier2, 0).unwrap();
        assert!(store.total_bytes() > bytes_after_one);
    }

    #[test]
    fn test_total_bytes_decreases_on_evict() {
        let mut store = TieredStore::new(4096);
        let data = vec![1.0f32; 64];
        let key = make_key(1, 0);

        store.put(key, &data, Tier::Tier1, 0).unwrap();
        let before = store.total_bytes();

        store.evict(key, ReconstructPolicy::None).unwrap();
        assert_eq!(store.total_bytes(), before - before); // back to 0
    }

    // -- Touch / access stats ----------------------------------------------

    #[test]
    fn test_touch_updates_stats() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        store.put(key, &[1.0; 16], Tier::Tier1, 0).unwrap();

        // Initial state after put.
        let meta = store.meta(key).unwrap();
        assert_eq!(meta.access_count, 1);
        assert_eq!(meta.last_access_at, 0);
        assert_eq!(meta.window, 1);

        // Touch at tick 5.
        store.touch(key, 5);
        let meta = store.meta(key).unwrap();
        assert_eq!(meta.access_count, 2);
        assert_eq!(meta.last_access_at, 5);
        // Window should have shifted left by 5 and gained bit 0.
        assert_eq!(meta.window, (1u64 << 5) | 1);
        assert!(meta.ema_rate > 0.0);

        // Touch at tick 5 again (same tick).
        store.touch(key, 5);
        let meta = store.meta(key).unwrap();
        assert_eq!(meta.access_count, 3);
        // Window unchanged on same-tick touch.
        assert_eq!(meta.window, (1u64 << 5) | 1);
    }

    #[test]
    fn test_touch_window_overflow() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        store.put(key, &[1.0; 16], Tier::Tier1, 0).unwrap();

        // Touch after more than 64 ticks clears the window entirely.
        store.touch(key, 100);
        let meta = store.meta(key).unwrap();
        assert_eq!(meta.window, 1);
        assert_eq!(meta.last_access_at, 100);
    }

    #[test]
    fn test_touch_nonexistent_noop() {
        let mut store = TieredStore::new(4096);
        // Should not panic.
        store.touch(make_key(42, 0), 10);
    }

    // -- Overwrite ---------------------------------------------------------

    #[test]
    fn test_put_overwrite() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);

        store.put(key, &[1.0; 16], Tier::Tier1, 0).unwrap();
        assert_eq!(store.tier_count(Tier::Tier1), 1);

        // Overwrite into a different tier.
        store.put(key, &[2.0; 16], Tier::Tier3, 10).unwrap();
        assert_eq!(store.block_count(), 1);
        assert_eq!(store.tier_count(Tier::Tier1), 0);
        assert_eq!(store.tier_count(Tier::Tier3), 1);

        let meta = store.meta(key).unwrap();
        assert_eq!(meta.tier, Tier::Tier3);
        assert_eq!(meta.created_at, 10);
    }

    // -- Checksum ----------------------------------------------------------

    #[test]
    fn test_checksum_stored_correctly() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        let data: Vec<f32> = (0..32).map(|i| i as f32).collect();

        store.put(key, &data, Tier::Tier1, 0).unwrap();

        let meta = store.meta(key).unwrap();
        assert_ne!(meta.checksum, 0);

        // Manually verify the checksum matches.
        let (packed, scale) = quantize_block(&data, 8);
        let expected = block_checksum(&packed, scale);
        assert_eq!(meta.checksum, expected);
    }

    // -- BlockIO trait ------------------------------------------------------

    #[test]
    fn test_block_io_write_read() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        let raw = vec![0xAA, 0xBB, 0xCC, 0xDD];

        store.write_block(Tier::Tier1, key, &raw).unwrap();

        let mut dst = vec![0u8; 8];
        let n = store.read_block(Tier::Tier1, key, &mut dst).unwrap();
        assert_eq!(n, 4);
        assert_eq!(&dst[..4], &raw);
    }

    #[test]
    fn test_block_io_delete() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        store.write_block(Tier::Tier2, key, &[1, 2, 3]).unwrap();

        store.delete_block(Tier::Tier2, key).unwrap();

        let mut dst = vec![0u8; 4];
        assert_eq!(
            store.read_block(Tier::Tier2, key, &mut dst),
            Err(StoreError::BlockNotFound),
        );
    }

    #[test]
    fn test_block_io_write_tier0_rejected() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        assert_eq!(
            store.write_block(Tier::Tier0, key, &[1]),
            Err(StoreError::InvalidBlock),
        );
    }

    // -- MetaLog trait ------------------------------------------------------

    #[test]
    fn test_meta_log_append_get() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        let meta = BlockMeta {
            key,
            dtype: DType::F32,
            tier: Tier::Tier1,
            bits: 8,
            scale: 0.5,
            zero_point: 0,
            created_at: 42,
            last_access_at: 42,
            access_count: 1,
            ema_rate: 0.0,
            window: 1,
            checksum: 0,
            reconstruct: ReconstructPolicy::None,
            tier_age: 0,
            lineage_parent: None,
            block_bytes: 64,
        };

        MetaLog::append(&mut store, &meta).unwrap();
        let retrieved = MetaLog::get(&store, key).unwrap();
        assert_eq!(retrieved.key, key);
        assert_eq!(retrieved.created_at, 42);
    }

    #[test]
    fn test_meta_log_iter() {
        let mut store = TieredStore::new(4096);
        let data = vec![1.0f32; 8];

        store.put(make_key(1, 0), &data, Tier::Tier1, 0).unwrap();
        store.put(make_key(2, 0), &data, Tier::Tier2, 0).unwrap();
        store.put(make_key(3, 0), &data, Tier::Tier3, 0).unwrap();

        let entries: Vec<_> = MetaLog::iter(&store).collect();
        assert_eq!(entries.len(), 3);
    }

    // -- bits_for_tier -----------------------------------------------------

    #[test]
    fn test_bits_for_tier() {
        assert_eq!(bits_for_tier(Tier::Tier0), 0);
        assert_eq!(bits_for_tier(Tier::Tier1), 8);
        assert_eq!(bits_for_tier(Tier::Tier2), 7);
        assert_eq!(bits_for_tier(Tier::Tier3), 3);
    }

    // -- Tier enum ---------------------------------------------------------

    #[test]
    fn test_tier_repr() {
        assert_eq!(Tier::Tier0 as u8, 0);
        assert_eq!(Tier::Tier1 as u8, 1);
        assert_eq!(Tier::Tier2 as u8, 2);
        assert_eq!(Tier::Tier3 as u8, 3);
    }

    #[test]
    fn test_dtype_repr() {
        assert_eq!(DType::F32 as u8, 0);
        assert_eq!(DType::F16 as u8, 1);
        assert_eq!(DType::BF16 as u8, 2);
    }

    #[test]
    fn test_reconstruct_policy_repr() {
        assert_eq!(ReconstructPolicy::None as u8, 0);
        assert_eq!(ReconstructPolicy::Delta as u8, 1);
        assert_eq!(ReconstructPolicy::Factor as u8, 2);
    }

    // -- Integration: multi-block workflow ---------------------------------

    #[test]
    fn test_multi_block_workflow() {
        let mut store = TieredStore::new(4096);

        // Insert 10 blocks across tiers.
        for i in 0..10u32 {
            let key = make_key(1, i);
            let data: Vec<f32> = (0..32).map(|j| (i * 32 + j) as f32 * 0.1).collect();
            let tier = match i % 3 {
                0 => Tier::Tier1,
                1 => Tier::Tier2,
                _ => Tier::Tier3,
            };
            store.put(key, &data, tier, i as u64).unwrap();
        }

        assert_eq!(store.block_count(), 10);
        assert_eq!(store.tier_count(Tier::Tier1), 4); // 0,3,6,9
        assert_eq!(store.tier_count(Tier::Tier2), 3); // 1,4,7
        assert_eq!(store.tier_count(Tier::Tier3), 3); // 2,5,8

        // Touch some blocks.
        store.touch(make_key(1, 0), 20);
        store.touch(make_key(1, 5), 25);

        // Evict a cold block.
        store
            .evict(make_key(1, 8), ReconstructPolicy::Delta)
            .unwrap();
        assert_eq!(store.tier_count(Tier::Tier3), 2);
        assert_eq!(store.tier_count(Tier::Tier0), 1);
        assert_eq!(store.block_count(), 10); // metadata preserved

        // Read back a hot block.
        let mut out = vec![0.0f32; 32];
        let n = TieredStore::get(&mut store, make_key(1, 0), &mut out, 30).unwrap();
        assert_eq!(n, 32);
    }

    // -- tick / score / touch_block -----------------------------------------

    #[test]
    fn test_tick_empty_store() {
        let mut store = TieredStore::new(4096);
        let config = crate::tiering::TierConfig::default();
        let result = store.tick(&config, 100, 1_000_000, 100);
        assert_eq!(result.upgrades, 0);
        assert_eq!(result.downgrades, 0);
        assert_eq!(result.evictions, 0);
        assert_eq!(result.bytes_freed, 0);
        assert_eq!(result.ops_used, 0);
        assert_eq!(result.candidates_found, 0);
    }

    #[test]
    fn test_tick_migrates_cold_to_hot() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        let data: Vec<f32> = (0..64).map(|i| i as f32 * 0.1).collect();

        // Put block in Tier3 (cold).
        store.put(key, &data, Tier::Tier3, 0).unwrap();
        assert_eq!(store.tier_count(Tier::Tier3), 1);

        // Simulate a highly-accessed block by directly setting metadata
        // fields so that the tiering score exceeds t1 + hysteresis.
        if let Some(meta) = store.index.get_mut(&key) {
            meta.ema_rate = 1.0;
            meta.window = u64::MAX; // all 64 bits set
            meta.last_access_at = 100;
            meta.access_count = 100;
            meta.tier_age = 10; // past default min_residency (5)
        }

        let config = crate::tiering::TierConfig::default();
        let result = store.tick(&config, 100, 1_000_000, 100);

        assert!(
            result.upgrades > 0,
            "expected at least one upgrade, got {}",
            result.upgrades
        );
        assert_eq!(result.downgrades, 0);
        assert!(result.candidates_found > 0);

        let meta = store.meta(key).unwrap();
        assert_eq!(
            meta.tier,
            Tier::Tier1,
            "block should be in Tier1 after upgrade"
        );
        assert_eq!(meta.bits, 8, "Tier1 should use 8-bit quantization");
        assert_eq!(meta.tier_age, 0, "tier_age should reset after migration");

        // The block should still be readable.
        let mut out = vec![0.0f32; 64];
        let n = TieredStore::get(&mut store, key, &mut out, 101).unwrap();
        assert_eq!(n, 64);
    }

    #[test]
    fn test_tick_respects_budget_ops() {
        let mut store = TieredStore::new(4096);
        let data: Vec<f32> = (0..64).map(|i| i as f32 * 0.1).collect();

        // Create 5 blocks in Tier3, all hot enough to warrant migration.
        for i in 0..5u32 {
            let key = make_key(i as u128 + 1, 0);
            store.put(key, &data, Tier::Tier3, 0).unwrap();
            if let Some(meta) = store.index.get_mut(&key) {
                meta.ema_rate = 1.0;
                meta.window = u64::MAX;
                meta.last_access_at = 100;
                meta.access_count = 100;
                meta.tier_age = 10;
            }
        }

        let config = crate::tiering::TierConfig::default();
        // Budget only 2 ops.
        let result = store.tick(&config, 100, 1_000_000, 2);

        assert_eq!(result.ops_used, 2, "should use exactly 2 ops");
        assert_eq!(result.upgrades, 2, "should upgrade only 2 blocks");
        assert!(result.candidates_found >= 5, "should find all 5 candidates");
    }

    #[test]
    fn test_touch_block_updates_ema_and_window() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        store.put(key, &[1.0; 16], Tier::Tier1, 0).unwrap();

        let config = crate::tiering::TierConfig::default();

        // Initial state: ema_rate is 0 after put.
        let meta = store.meta(key).unwrap();
        assert_eq!(meta.ema_rate, 0.0);

        // Touch at tick 5.
        store.touch_block(key, &config, 5);
        let meta = store.meta(key).unwrap();

        // tiering::touch sets ema_rate = alpha + (1 - alpha) * old_ema
        // = 0.3 + 0.7 * 0.0 = 0.3
        assert!(
            (meta.ema_rate - config.alpha).abs() < 1e-6,
            "ema_rate={}, expected={}",
            meta.ema_rate,
            config.alpha,
        );
        assert_eq!(meta.last_access_at, 5);
        // Window should have bit 0 set after touch.
        assert_ne!(meta.window & 1, 0, "bit 0 should be set");
        // Elapsed = 5 ticks from 0, so window = (initial << 5) | 1.
        // Initial window from put is 1, so: (1 << 5) | 1 = 0b100001.
        assert_eq!(meta.window, (1u64 << 5) | 1);
    }

    #[test]
    fn test_score_block_none_for_missing() {
        let store = TieredStore::new(4096);
        let config = crate::tiering::TierConfig::default();
        let result = store.score_block(make_key(99, 0), &config, 100);
        assert_eq!(result, None);
    }

    // -----------------------------------------------------------------------
    // Coherence integration
    // -----------------------------------------------------------------------

    #[test]
    fn test_epoch_tracker_wired_into_put() {
        let mut store = TieredStore::new(4096);
        let key = BlockKey {
            tensor_id: 1,
            block_index: 0,
        };
        let data = vec![1.0f32; 64];

        assert_eq!(store.epoch_tracker().check_epoch(key), None);

        store.put(key, &data, Tier::Tier1, 0).unwrap();
        assert!(store.epoch_tracker().check_epoch(key).is_some());

        let epoch1 = store.epoch_tracker().check_epoch(key).unwrap();
        store.put(key, &data, Tier::Tier1, 1).unwrap();
        let epoch2 = store.epoch_tracker().check_epoch(key).unwrap();
        assert!(epoch2 > epoch1, "epoch should increment on overwrite");
    }

    #[test]
    fn test_coherence_disabled_by_default() {
        let mut store = TieredStore::new(4096);
        let key = BlockKey {
            tensor_id: 1,
            block_index: 0,
        };
        let data = vec![1.0f32; 64];
        store.put(key, &data, Tier::Tier1, 0).unwrap();

        assert!(store.coherence_check(key, &data, 1).is_none());
    }

    #[test]
    fn test_coherence_enabled_passes() {
        let mut store = TieredStore::new(4096);
        store.enable_coherence(crate::coherence::CoherenceCheck::default());

        let key = BlockKey {
            tensor_id: 1,
            block_index: 0,
        };
        let data: Vec<f32> = (0..64).map(|i| (i as f32 + 1.0) * 0.25).collect();
        store.put(key, &data, Tier::Tier1, 0).unwrap();

        let result = store.coherence_check(key, &data, 1).unwrap().unwrap();
        assert!(
            result.passed,
            "Tier1 coherence should pass; err={}",
            result.max_error
        );
    }

    // -----------------------------------------------------------------------
    // MetricsSeries integration
    // -----------------------------------------------------------------------

    #[test]
    fn test_metrics_series_wired_into_tick() {
        use crate::tiering::TierConfig;

        let mut store = TieredStore::new(4096);
        let config = TierConfig::default();

        // Put a few blocks.
        for i in 0..5u128 {
            let key = BlockKey {
                tensor_id: i,
                block_index: 0,
            };
            store.put(key, &vec![1.0f32; 64], Tier::Tier1, 0).unwrap();
        }

        assert!(store.metrics_series().is_empty());

        // Run a tick -- should auto-record a metrics snapshot.
        store.tick(&config, 100, 1_000_000, 100);
        assert_eq!(store.metrics_series().len(), 1);

        // Run another tick.
        store.tick(&config, 200, 1_000_000, 100);
        assert_eq!(store.metrics_series().len(), 2);

        // Latest snapshot should reflect current state.
        let (ts, m) = store.metrics_series().latest().unwrap();
        assert_eq!(*ts, 200);
        assert_eq!(m.total_blocks, 5);
    }

    // -----------------------------------------------------------------------
    // Benchmarks
    // -----------------------------------------------------------------------
    //
    // Run with: cargo test bench_ -- --nocapture
    // These use std::time::Instant and std::hint::black_box for stable timing.

    #[test]
    fn bench_batch_scoring_10k() {
        use crate::tiering::{
            compute_score, compute_scores_batch, BlockMeta as TBlockMeta, Tier as TTier, TierConfig,
        };
        use std::time::Instant;

        let cfg = TierConfig::default();
        let metas: Vec<TBlockMeta> = (0..10_000)
            .map(|i| TBlockMeta {
                ema_rate: (i as f32) * 0.0001,
                access_window: 0x5555_5555_5555_5555,
                last_access: 50 + (i as u64 % 100),
                access_count: i as u64,
                current_tier: TTier::Tier1,
                tier_since: 0,
            })
            .collect();

        let iters = 1000;

        // Individual scoring
        let start = Instant::now();
        for _ in 0..iters {
            for m in &metas {
                std::hint::black_box(compute_score(&cfg, 100, m));
            }
        }
        let individual = start.elapsed();

        // Batch scoring
        let start = Instant::now();
        for _ in 0..iters {
            std::hint::black_box(compute_scores_batch(&cfg, 100, &metas));
        }
        let batch = start.elapsed();

        eprintln!(
            "Individual scoring 10k x {iters}: {:?} ({:.0} ns/block)",
            individual,
            individual.as_nanos() as f64 / (iters * 10_000) as f64
        );
        eprintln!(
            "Batch scoring 10k x {iters}: {:?} ({:.0} ns/block)",
            batch,
            batch.as_nanos() as f64 / (iters * 10_000) as f64
        );
    }

    #[test]
    fn bench_dequant_5bit_4096() {
        use std::time::Instant;

        let data: Vec<f32> = (0..4096).map(|i| (i as f32 - 2048.0) * 0.01).collect();
        let (packed, scale) = quantize_block(&data, 5);
        let mut out = vec![0.0f32; 4096];

        let iters = 10_000;
        let start = Instant::now();
        for _ in 0..iters {
            std::hint::black_box(dequantize_block(&packed, scale, 5, 4096, &mut out));
        }
        let elapsed = start.elapsed();

        let total_bytes = 4096u64 * 4 * iters as u64;
        let gbs = total_bytes as f64 / elapsed.as_secs_f64() / 1e9;
        eprintln!(
            "Dequant 5-bit 4096 x {iters}: {:?} ({:.2} GB/s output throughput)",
            elapsed, gbs
        );
    }

    #[test]
    fn bench_dequant_7bit_4096() {
        use std::time::Instant;

        let data: Vec<f32> = (0..4096).map(|i| (i as f32 - 2048.0) * 0.01).collect();
        let (packed, scale) = quantize_block(&data, 7);
        let mut out = vec![0.0f32; 4096];

        let iters = 10_000;
        let start = Instant::now();
        for _ in 0..iters {
            std::hint::black_box(dequantize_block(&packed, scale, 7, 4096, &mut out));
        }
        let elapsed = start.elapsed();

        let total_bytes = 4096u64 * 4 * iters as u64;
        let gbs = total_bytes as f64 / elapsed.as_secs_f64() / 1e9;
        eprintln!(
            "Dequant 7-bit 4096 x {iters}: {:?} ({:.2} GB/s output throughput)",
            elapsed, gbs
        );
    }

    #[test]
    fn bench_quant_5bit_4096() {
        use std::time::Instant;

        let data: Vec<f32> = (0..4096).map(|i| (i as f32 - 2048.0) * 0.01).collect();

        let iters = 10_000;
        let start = Instant::now();
        for _ in 0..iters {
            std::hint::black_box(quantize_block(&data, 5));
        }
        let elapsed = start.elapsed();

        let total_bytes = 4096u64 * 4 * iters as u64;
        let gbs = total_bytes as f64 / elapsed.as_secs_f64() / 1e9;
        eprintln!(
            "Quant 5-bit 4096 x {iters}: {:?} ({:.2} GB/s input throughput)",
            elapsed, gbs
        );
    }

    #[test]
    fn bench_svd_adaptive_64x64() {
        use crate::delta::FactorSet;
        use std::time::Instant;

        let (rows, cols) = (64, 64);
        let data: Vec<f32> = (0..rows * cols)
            .map(|i| (i as f32 * 0.37).sin() + (i as f32 * 0.73).cos())
            .collect();

        let iters = 100;
        let start = Instant::now();
        for _ in 0..iters {
            std::hint::black_box(FactorSet::from_data_adaptive(&data, rows, cols, 16, 0.05));
        }
        let elapsed = start.elapsed();

        eprintln!(
            "SVD adaptive 64x64 (max_rank=16, target=0.05) x {iters}: {:?} ({:.2} ms/iter)",
            elapsed,
            elapsed.as_secs_f64() * 1000.0 / iters as f64
        );
    }

    #[test]
    fn bench_format_report() {
        use crate::metrics::StoreMetrics;
        use std::time::Instant;

        let m = StoreMetrics {
            total_blocks: 10_000,
            tier0_blocks: 500,
            tier1_blocks: 4000,
            tier2_blocks: 3500,
            tier3_blocks: 2000,
            tier1_bytes: 4_000_000,
            tier2_bytes: 2_500_000,
            tier3_bytes: 750_000,
            total_reads: 1_000_000,
            total_writes: 500_000,
            total_evictions: 5000,
            total_upgrades: 12_000,
            total_downgrades: 8000,
            total_reconstructions: 200,
            total_checksum_failures: 0,
            total_compactions: 150,
            tier_flips_last_minute: 0.023,
            avg_score_tier1: 0.85,
            avg_score_tier2: 0.45,
            avg_score_tier3: 0.12,
        };

        let iters = 10_000;
        let start = Instant::now();
        for _ in 0..iters {
            std::hint::black_box(m.format_report());
        }
        let elapsed = start.elapsed();

        eprintln!(
            "format_report x {iters}: {:?} ({:.0} ns/call)",
            elapsed,
            elapsed.as_nanos() as f64 / iters as f64
        );
    }

    #[test]
    fn bench_format_json() {
        use crate::metrics::StoreMetrics;
        use std::time::Instant;

        let m = StoreMetrics {
            total_blocks: 10_000,
            tier0_blocks: 500,
            tier1_blocks: 4000,
            tier2_blocks: 3500,
            tier3_blocks: 2000,
            tier1_bytes: 4_000_000,
            tier2_bytes: 2_500_000,
            tier3_bytes: 750_000,
            total_reads: 1_000_000,
            total_writes: 500_000,
            total_evictions: 5000,
            total_upgrades: 12_000,
            total_downgrades: 8000,
            total_reconstructions: 200,
            total_checksum_failures: 0,
            total_compactions: 150,
            tier_flips_last_minute: 0.023,
            avg_score_tier1: 0.85,
            avg_score_tier2: 0.45,
            avg_score_tier3: 0.12,
        };

        let iters = 10_000;
        let start = Instant::now();
        for _ in 0..iters {
            std::hint::black_box(m.format_json());
        }
        let elapsed = start.elapsed();

        eprintln!(
            "format_json x {iters}: {:?} ({:.0} ns/call)",
            elapsed,
            elapsed.as_nanos() as f64 / iters as f64
        );
    }

    #[test]
    fn bench_metrics_series_trend_100() {
        use crate::metrics::{MetricsSeries, StoreMetrics};
        use std::time::Instant;

        let mut series = MetricsSeries::new(256);
        for i in 0..100u64 {
            series.record(
                i,
                StoreMetrics {
                    total_blocks: 1000 + i,
                    tier1_blocks: 400 + i % 50,
                    tier2_blocks: 350,
                    tier3_blocks: 250,
                    tier1_bytes: 400_000 + i * 100,
                    tier2_bytes: 250_000,
                    tier3_bytes: 75_000,
                    total_evictions: i * 3,
                    ..Default::default()
                },
            );
        }

        let iters = 10_000;
        let start = Instant::now();
        for _ in 0..iters {
            std::hint::black_box(series.trend());
        }
        let elapsed = start.elapsed();

        eprintln!(
            "MetricsSeries trend (100 snapshots) x {iters}: {:?} ({:.0} ns/call)",
            elapsed,
            elapsed.as_nanos() as f64 / iters as f64
        );
    }
}
