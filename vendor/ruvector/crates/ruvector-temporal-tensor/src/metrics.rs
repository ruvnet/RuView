//! Witness logging and decision audit for the temporal tensor store.
//!
//! Provides an append-only [`WitnessLog`] that records every auditable decision
//! (tier changes, evictions, checksum failures, etc.) and aggregate
//! [`StoreMetrics`] for dashboards and alerting.
//!
//! All types are zero-dependency and allocation-minimal so they can live on the
//! hot path without measurable overhead.
//!
//! # Usage
//!
//! ```ignore
//! use ruvector_temporal_tensor::metrics::{WitnessLog, WitnessEvent, StoreMetrics};
//!
//! let mut log = WitnessLog::new(1024);
//! log.record(42, WitnessEvent::Eviction {
//!     key: BlockKey(7),
//!     score: 0.1,
//!     bytes_freed: 4096,
//! });
//! assert_eq!(log.count_evictions(), 1);
//! ```

use crate::store::{BlockKey, ReconstructPolicy, Tier};

// ---------------------------------------------------------------------------
// Witness record types
// ---------------------------------------------------------------------------

/// A witness record for an auditable decision.
///
/// Each record pairs a monotonic timestamp (tick counter) with the event that
/// occurred at that instant. Records are append-only and immutable once stored.
#[derive(Clone, Debug)]
pub struct WitnessRecord {
    /// Monotonic tick at which the event was witnessed.
    pub timestamp: u64,
    /// The event that was witnessed.
    pub event: WitnessEvent,
}

/// Types of witnessed events.
///
/// Every variant captures the minimum context required to reconstruct the
/// decision after the fact (key, scores, tiers, byte counts).
#[derive(Clone, Debug)]
pub enum WitnessEvent {
    /// A block was accessed (read or write).
    Access {
        key: BlockKey,
        score: f32,
        tier: Tier,
    },
    /// A block changed tiers.
    TierChange {
        key: BlockKey,
        from_tier: Tier,
        to_tier: Tier,
        score: f32,
        reason: TierChangeReason,
    },
    /// A block was evicted (compressed to zero).
    Eviction {
        key: BlockKey,
        score: f32,
        bytes_freed: usize,
    },
    /// A maintenance tick was processed.
    Maintenance {
        upgrades: u32,
        downgrades: u32,
        evictions: u32,
        bytes_freed: usize,
        budget_remaining_bytes: u32,
        budget_remaining_ops: u32,
    },
    /// A delta chain was compacted.
    Compaction { key: BlockKey, chain_len_before: u8 },
    /// A checksum mismatch was detected.
    ChecksumFailure {
        key: BlockKey,
        expected: u32,
        actual: u32,
    },
    /// A block was reconstructed from deltas or factors.
    Reconstruction {
        key: BlockKey,
        policy: ReconstructPolicy,
        success: bool,
    },
}

/// Reason a block changed tiers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TierChangeReason {
    /// Score rose above the upgrade threshold.
    ScoreUpgrade,
    /// Score fell below the downgrade threshold.
    ScoreDowngrade,
    /// Byte-cap pressure forced a downgrade.
    ByteCapPressure,
    /// An operator or API caller forced a tier change.
    ManualOverride,
}

// ---------------------------------------------------------------------------
// Aggregate metrics
// ---------------------------------------------------------------------------

/// Aggregate metrics for the temporal tensor store.
///
/// All counters are monotonically increasing over the lifetime of the store.
/// Gauge-style fields (e.g. `tier0_blocks`) reflect the current state.
#[derive(Clone, Debug, Default)]
pub struct StoreMetrics {
    /// Total number of live blocks across all tiers.
    pub total_blocks: u64,
    /// Number of blocks in tier 0 (raw / uncompressed).
    pub tier0_blocks: u64,
    /// Number of blocks in tier 1 (hot, 8-bit).
    pub tier1_blocks: u64,
    /// Number of blocks in tier 2 (warm, 7/5-bit).
    pub tier2_blocks: u64,
    /// Number of blocks in tier 3 (cold, 3-bit).
    pub tier3_blocks: u64,
    /// Total stored bytes in tier 1.
    pub tier1_bytes: u64,
    /// Total stored bytes in tier 2.
    pub tier2_bytes: u64,
    /// Total stored bytes in tier 3.
    pub tier3_bytes: u64,
    /// Cumulative read count.
    pub total_reads: u64,
    /// Cumulative write count.
    pub total_writes: u64,
    /// Cumulative eviction count.
    pub total_evictions: u64,
    /// Cumulative upgrade count.
    pub total_upgrades: u64,
    /// Cumulative downgrade count.
    pub total_downgrades: u64,
    /// Cumulative reconstruction count.
    pub total_reconstructions: u64,
    /// Cumulative checksum failure count.
    pub total_checksum_failures: u64,
    /// Cumulative compaction count.
    pub total_compactions: u64,
    /// Tier flips per block per minute over the last minute.
    pub tier_flips_last_minute: f32,
    /// Average score of tier 1 blocks.
    pub avg_score_tier1: f32,
    /// Average score of tier 2 blocks.
    pub avg_score_tier2: f32,
    /// Average score of tier 3 blocks.
    pub avg_score_tier3: f32,
}

impl StoreMetrics {
    /// Create a new zeroed metrics struct.
    pub fn new() -> Self {
        Self::default()
    }

    /// Compression ratio: raw f32 bytes / stored bytes.
    ///
    /// Raw bytes are estimated as `total_blocks * average_tensor_len * 4`, but
    /// since we lack per-block tensor lengths here, we approximate with the
    /// tier-0 identity: each tier-0 block is already f32, so the stored bytes
    /// for tier 0 equal the raw bytes. The ratio is therefore:
    ///
    /// `(tier0_raw + tier1_raw + tier2_raw + tier3_raw) / total_stored_bytes`
    ///
    /// Because we don't track raw bytes per tier at this level, we report
    /// `total_stored_bytes / total_stored_bytes` as a baseline and let callers
    /// that have richer context compute the true ratio. For a simple heuristic,
    /// we use the known compression ratios: tier1 ~4x, tier2 ~5.5x, tier3 ~10.67x.
    pub fn compression_ratio(&self) -> f32 {
        let stored = self.total_stored_bytes();
        if stored == 0 {
            return 0.0;
        }
        let raw_estimate = (self.tier1_bytes as f64 * 4.0)
            + (self.tier2_bytes as f64 * 5.5)
            + (self.tier3_bytes as f64 * 10.67);
        raw_estimate as f32 / stored as f32
    }

    /// Total stored bytes across all compressed tiers (1, 2, 3).
    ///
    /// Tier 0 blocks are raw f32 and not tracked separately; callers can
    /// compute tier-0 bytes as `tier0_blocks * tensor_len * 4` if needed.
    pub fn total_stored_bytes(&self) -> u64 {
        self.tier1_bytes + self.tier2_bytes + self.tier3_bytes
    }

    /// Generate a human-readable multi-line status report.
    pub fn format_report(&self) -> String {
        let mut s = String::with_capacity(512);
        s.push_str("=== Temporal Tensor Store Report ===\n");
        s.push_str(&format_line("Total blocks", self.total_blocks));
        s.push_str(&format_line("  Tier0 (raw)", self.tier0_blocks));
        s.push_str(&format_line("  Tier1 (hot)", self.tier1_blocks));
        s.push_str(&format_line("  Tier2 (warm)", self.tier2_blocks));
        s.push_str(&format_line("  Tier3 (cold)", self.tier3_blocks));
        s.push_str("--- Storage ---\n");
        s.push_str(&format_line("Tier1 bytes", self.tier1_bytes));
        s.push_str(&format_line("Tier2 bytes", self.tier2_bytes));
        s.push_str(&format_line("Tier3 bytes", self.tier3_bytes));
        s.push_str(&format_line("Total stored", self.total_stored_bytes()));
        s.push_str(&format!(
            "Compression ratio: {:.2}x\n",
            self.compression_ratio()
        ));
        s.push_str("--- Operations ---\n");
        s.push_str(&format_line("Reads", self.total_reads));
        s.push_str(&format_line("Writes", self.total_writes));
        s.push_str(&format_line("Evictions", self.total_evictions));
        s.push_str(&format_line("Upgrades", self.total_upgrades));
        s.push_str(&format_line("Downgrades", self.total_downgrades));
        s.push_str(&format_line("Reconstructions", self.total_reconstructions));
        s.push_str(&format_line("Compactions", self.total_compactions));
        s.push_str(&format_line(
            "Checksum failures",
            self.total_checksum_failures,
        ));
        s.push_str(&format!(
            "Tier flip rate: {:.4}/block/min\n",
            self.tier_flips_last_minute
        ));
        s
    }

    /// Generate a JSON representation (no serde dependency).
    pub fn format_json(&self) -> String {
        format!(
            concat!(
                "{{",
                "\"total_blocks\":{},",
                "\"tier0_blocks\":{},",
                "\"tier1_blocks\":{},",
                "\"tier2_blocks\":{},",
                "\"tier3_blocks\":{},",
                "\"tier1_bytes\":{},",
                "\"tier2_bytes\":{},",
                "\"tier3_bytes\":{},",
                "\"total_reads\":{},",
                "\"total_writes\":{},",
                "\"total_evictions\":{},",
                "\"total_upgrades\":{},",
                "\"total_downgrades\":{},",
                "\"total_reconstructions\":{},",
                "\"total_checksum_failures\":{},",
                "\"total_compactions\":{},",
                "\"compression_ratio\":{:.4},",
                "\"tier_flips_last_minute\":{:.4},",
                "\"avg_score_tier1\":{:.4},",
                "\"avg_score_tier2\":{:.4},",
                "\"avg_score_tier3\":{:.4}",
                "}}"
            ),
            self.total_blocks,
            self.tier0_blocks,
            self.tier1_blocks,
            self.tier2_blocks,
            self.tier3_blocks,
            self.tier1_bytes,
            self.tier2_bytes,
            self.tier3_bytes,
            self.total_reads,
            self.total_writes,
            self.total_evictions,
            self.total_upgrades,
            self.total_downgrades,
            self.total_reconstructions,
            self.total_checksum_failures,
            self.total_compactions,
            self.compression_ratio(),
            self.tier_flips_last_minute,
            self.avg_score_tier1,
            self.avg_score_tier2,
            self.avg_score_tier3,
        )
    }

    /// Automated health assessment.
    pub fn health_check(&self) -> StoreHealthStatus {
        // Critical: checksum failures
        if self.total_checksum_failures > 0 {
            return StoreHealthStatus::Critical(format!(
                "{} checksum failures detected",
                self.total_checksum_failures
            ));
        }
        // Warning: high tier flip rate
        if self.tier_flips_last_minute > 0.5 {
            return StoreHealthStatus::Warning(format!(
                "High tier flip rate: {:.3}/block/min",
                self.tier_flips_last_minute
            ));
        }
        // Warning: mostly evictions
        if self.total_evictions > 0 && self.total_blocks > 0 {
            let eviction_ratio =
                self.total_evictions as f32 / (self.total_reads + self.total_writes).max(1) as f32;
            if eviction_ratio > 0.3 {
                return StoreHealthStatus::Warning(format!(
                    "High eviction ratio: {:.1}%",
                    eviction_ratio * 100.0
                ));
            }
        }
        StoreHealthStatus::Healthy
    }
}

/// Health status of the store.
#[derive(Clone, Debug, PartialEq)]
pub enum StoreHealthStatus {
    /// Everything is operating normally.
    Healthy,
    /// Non-critical issue detected.
    Warning(String),
    /// Critical issue requiring attention.
    Critical(String),
}

// ---------------------------------------------------------------------------
// Witness log (ring buffer)
// ---------------------------------------------------------------------------

/// Append-only witness log with configurable capacity.
///
/// When the log reaches capacity, the oldest records are dropped to make room
/// for new ones, giving ring-buffer semantics. This bounds memory usage while
/// preserving the most recent history for audit trails and flip-rate
/// calculations.
pub struct WitnessLog {
    records: Vec<WitnessRecord>,
    capacity: usize,
}

impl WitnessLog {
    /// Create a new witness log with the given maximum capacity.
    ///
    /// A capacity of zero is treated as one (at least one record can be stored).
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            records: Vec::with_capacity(capacity.min(1024)),
            capacity,
        }
    }

    /// Record a witness event at the given timestamp.
    ///
    /// If the log is at capacity, the oldest record is removed first.
    pub fn record(&mut self, timestamp: u64, event: WitnessEvent) {
        if self.records.len() >= self.capacity {
            self.records.remove(0);
        }
        self.records.push(WitnessRecord { timestamp, event });
    }

    /// Number of recorded events currently in the log.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the log contains no records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Get the most recent `n` records.
    ///
    /// Returns fewer than `n` if the log does not contain that many records.
    pub fn recent(&self, n: usize) -> &[WitnessRecord] {
        let start = self.records.len().saturating_sub(n);
        &self.records[start..]
    }

    /// Get all records currently in the log.
    pub fn all(&self) -> &[WitnessRecord] {
        &self.records
    }

    /// Clear all records from the log.
    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// Count the number of [`WitnessEvent::TierChange`] records.
    pub fn count_tier_changes(&self) -> usize {
        self.records
            .iter()
            .filter(|r| matches!(r.event, WitnessEvent::TierChange { .. }))
            .count()
    }

    /// Count the number of [`WitnessEvent::Eviction`] records.
    pub fn count_evictions(&self) -> usize {
        self.records
            .iter()
            .filter(|r| matches!(r.event, WitnessEvent::Eviction { .. }))
            .count()
    }

    /// Count the number of [`WitnessEvent::ChecksumFailure`] records.
    pub fn count_checksum_failures(&self) -> usize {
        self.records
            .iter()
            .filter(|r| matches!(r.event, WitnessEvent::ChecksumFailure { .. }))
            .count()
    }

    /// Compute tier flip rate: tier changes per block per minute.
    ///
    /// `window_ticks` is the size of the time window to consider (only records
    /// whose timestamp is >= `max_timestamp - window_ticks` are counted).
    /// `num_blocks` is the current total block count (used as the denominator).
    ///
    /// Returns `0.0` when `num_blocks` is zero or when no tier changes fall
    /// within the window.
    pub fn tier_flip_rate(&self, window_ticks: u64, num_blocks: u64) -> f32 {
        if num_blocks == 0 || self.records.is_empty() {
            return 0.0;
        }

        let max_ts = self.records.iter().map(|r| r.timestamp).max().unwrap_or(0);
        let min_ts = max_ts.saturating_sub(window_ticks);

        let flips = self
            .records
            .iter()
            .filter(|r| r.timestamp >= min_ts)
            .filter(|r| matches!(r.event, WitnessEvent::TierChange { .. }))
            .count() as f32;

        flips / num_blocks as f32
    }
}

// ---------------------------------------------------------------------------
// Point-in-time snapshot
// ---------------------------------------------------------------------------

/// A point-in-time snapshot of store state for serialization and export.
///
/// Captures the metrics, tier distribution (block counts), and byte distribution
/// at a single instant.
#[derive(Clone, Debug)]
pub struct StoreSnapshot {
    /// Monotonic tick at which the snapshot was taken.
    pub timestamp: u64,
    /// Aggregate metrics at snapshot time.
    pub metrics: StoreMetrics,
    /// Block count per tier: `[tier0, tier1, tier2, tier3]`.
    pub tier_distribution: [u64; 4],
    /// Byte count per tier: `[tier0, tier1, tier2, tier3]`.
    pub byte_distribution: [u64; 4],
}

impl StoreSnapshot {
    /// Serialize to a simple `key=value` text format.
    ///
    /// Each line is `key=value\n`. Numeric values are printed in decimal.
    /// This format is intentionally trivial to parse so that external tools
    /// (dashboards, log aggregators) can ingest it without pulling in a JSON
    /// library.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(512);

        push_kv(&mut buf, "timestamp", self.timestamp);
        push_kv(&mut buf, "total_blocks", self.metrics.total_blocks);
        push_kv(&mut buf, "tier0_blocks", self.metrics.tier0_blocks);
        push_kv(&mut buf, "tier1_blocks", self.metrics.tier1_blocks);
        push_kv(&mut buf, "tier2_blocks", self.metrics.tier2_blocks);
        push_kv(&mut buf, "tier3_blocks", self.metrics.tier3_blocks);
        push_kv(&mut buf, "tier1_bytes", self.metrics.tier1_bytes);
        push_kv(&mut buf, "tier2_bytes", self.metrics.tier2_bytes);
        push_kv(&mut buf, "tier3_bytes", self.metrics.tier3_bytes);
        push_kv(&mut buf, "total_reads", self.metrics.total_reads);
        push_kv(&mut buf, "total_writes", self.metrics.total_writes);
        push_kv(&mut buf, "total_evictions", self.metrics.total_evictions);
        push_kv(&mut buf, "total_upgrades", self.metrics.total_upgrades);
        push_kv(&mut buf, "total_downgrades", self.metrics.total_downgrades);
        push_kv(
            &mut buf,
            "total_reconstructions",
            self.metrics.total_reconstructions,
        );
        push_kv(
            &mut buf,
            "total_checksum_failures",
            self.metrics.total_checksum_failures,
        );
        push_kv(
            &mut buf,
            "total_compactions",
            self.metrics.total_compactions,
        );
        push_kv_f32(
            &mut buf,
            "tier_flips_last_minute",
            self.metrics.tier_flips_last_minute,
        );
        push_kv_f32(&mut buf, "avg_score_tier1", self.metrics.avg_score_tier1);
        push_kv_f32(&mut buf, "avg_score_tier2", self.metrics.avg_score_tier2);
        push_kv_f32(&mut buf, "avg_score_tier3", self.metrics.avg_score_tier3);
        push_kv_f32(
            &mut buf,
            "compression_ratio",
            self.metrics.compression_ratio(),
        );
        push_kv(
            &mut buf,
            "total_stored_bytes",
            self.metrics.total_stored_bytes(),
        );

        // Distributions
        for (i, &count) in self.tier_distribution.iter().enumerate() {
            push_kv_indexed(&mut buf, "tier_dist", i, count);
        }
        for (i, &bytes) in self.byte_distribution.iter().enumerate() {
            push_kv_indexed(&mut buf, "byte_dist", i, bytes);
        }

        buf
    }
}

// ---------------------------------------------------------------------------
// Time-series metrics ring buffer
// ---------------------------------------------------------------------------

/// Ring buffer of [`StoreMetrics`] snapshots for trend analysis.
pub struct MetricsSeries {
    snapshots: Vec<(u64, StoreMetrics)>,
    capacity: usize,
}

/// Trend analysis computed from a [`MetricsSeries`].
#[derive(Clone, Debug)]
pub struct MetricsTrend {
    /// Evictions per snapshot (rate of change).
    pub eviction_rate: f32,
    /// Whether compression ratio is improving over recent snapshots.
    pub compression_improving: bool,
    /// Whether tier distribution is stable (low variance).
    pub tier_distribution_stable: bool,
}

impl MetricsSeries {
    /// Create a new series with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            snapshots: Vec::with_capacity(capacity.min(256)),
            capacity: capacity.max(1),
        }
    }

    /// Record a metrics snapshot at the given timestamp.
    pub fn record(&mut self, timestamp: u64, metrics: StoreMetrics) {
        if self.snapshots.len() >= self.capacity {
            self.snapshots.remove(0);
        }
        self.snapshots.push((timestamp, metrics));
    }

    /// Number of snapshots stored.
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Whether the series is empty.
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Get the most recent snapshot.
    pub fn latest(&self) -> Option<&(u64, StoreMetrics)> {
        self.snapshots.last()
    }

    /// Compute trend analysis over the stored snapshots.
    pub fn trend(&self) -> MetricsTrend {
        if self.snapshots.len() < 2 {
            return MetricsTrend {
                eviction_rate: 0.0,
                compression_improving: false,
                tier_distribution_stable: true,
            };
        }

        let n = self.snapshots.len();
        let first = &self.snapshots[0].1;
        let last = &self.snapshots[n - 1].1;

        // Eviction rate: evictions delta / number of snapshots
        let eviction_delta = last.total_evictions.saturating_sub(first.total_evictions);
        let eviction_rate = eviction_delta as f32 / n as f32;

        // Compression trend: compare first half average to second half average
        let mid = n / 2;
        let first_half_ratio: f32 = self.snapshots[..mid]
            .iter()
            .map(|(_, m)| m.compression_ratio())
            .sum::<f32>()
            / mid as f32;
        let second_half_ratio: f32 = self.snapshots[mid..]
            .iter()
            .map(|(_, m)| m.compression_ratio())
            .sum::<f32>()
            / (n - mid) as f32;
        let compression_improving = second_half_ratio > first_half_ratio;

        // Tier stability: check if tier1_blocks variance is low
        let avg_tier1: f64 = self
            .snapshots
            .iter()
            .map(|(_, m)| m.tier1_blocks as f64)
            .sum::<f64>()
            / n as f64;
        let var_tier1: f64 = self
            .snapshots
            .iter()
            .map(|(_, m)| {
                let d = m.tier1_blocks as f64 - avg_tier1;
                d * d
            })
            .sum::<f64>()
            / n as f64;
        let tier_distribution_stable = var_tier1.sqrt() < avg_tier1.max(1.0) * 0.3;

        MetricsTrend {
            eviction_rate,
            compression_improving,
            tier_distribution_stable,
        }
    }
}

// ---------------------------------------------------------------------------
// Serialization helpers (no alloc formatting -- we avoid `format!` to stay
// lightweight; instead we write digits manually).
// ---------------------------------------------------------------------------

/// Format a key-value line for the text report.
fn format_line(key: &str, value: u64) -> String {
    format!("{}: {}\n", key, value)
}

/// Push `key=value\n` for a u64 value.
fn push_kv(buf: &mut Vec<u8>, key: &str, value: u64) {
    buf.extend_from_slice(key.as_bytes());
    buf.push(b'=');
    push_u64(buf, value);
    buf.push(b'\n');
}

/// Push `key=value\n` for an f32 value (6 decimal places).
fn push_kv_f32(buf: &mut Vec<u8>, key: &str, value: f32) {
    buf.extend_from_slice(key.as_bytes());
    buf.push(b'=');
    push_f32(buf, value);
    buf.push(b'\n');
}

/// Push `key[index]=value\n`.
fn push_kv_indexed(buf: &mut Vec<u8>, key: &str, index: usize, value: u64) {
    buf.extend_from_slice(key.as_bytes());
    buf.push(b'[');
    push_u64(buf, index as u64);
    buf.push(b']');
    buf.push(b'=');
    push_u64(buf, value);
    buf.push(b'\n');
}

/// Write a `u64` as decimal ASCII digits.
fn push_u64(buf: &mut Vec<u8>, mut v: u64) {
    if v == 0 {
        buf.push(b'0');
        return;
    }
    let start = buf.len();
    while v > 0 {
        buf.push(b'0' + (v % 10) as u8);
        v /= 10;
    }
    buf[start..].reverse();
}

/// Write an `f32` as decimal with 6 fractional digits.
fn push_f32(buf: &mut Vec<u8>, v: f32) {
    if v < 0.0 {
        buf.push(b'-');
        push_f32(buf, -v);
        return;
    }
    let int_part = v as u64;
    push_u64(buf, int_part);
    buf.push(b'.');
    let frac = ((v - int_part as f32) * 1_000_000.0).round() as u64;
    // Pad to 6 digits.
    let s = frac;
    let digits = if s == 0 {
        1
    } else {
        ((s as f64).log10().floor() as usize) + 1
    };
    for _ in 0..(6usize.saturating_sub(digits)) {
        buf.push(b'0');
    }
    push_u64(buf, s);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{BlockKey, Tier};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn bk(id: u64) -> BlockKey {
        BlockKey {
            tensor_id: id as u128,
            block_index: 0,
        }
    }

    fn make_access(key: u64, score: f32, tier: Tier) -> WitnessEvent {
        WitnessEvent::Access {
            key: bk(key),
            score,
            tier,
        }
    }

    fn make_tier_change(key: u64, from: Tier, to: Tier) -> WitnessEvent {
        WitnessEvent::TierChange {
            key: bk(key),
            from_tier: from,
            to_tier: to,
            score: 100.0,
            reason: TierChangeReason::ScoreUpgrade,
        }
    }

    fn make_eviction(key: u64) -> WitnessEvent {
        WitnessEvent::Eviction {
            key: bk(key),
            score: 0.5,
            bytes_freed: 1024,
        }
    }

    fn make_checksum_failure(key: u64) -> WitnessEvent {
        WitnessEvent::ChecksumFailure {
            key: bk(key),
            expected: 0xDEAD,
            actual: 0xBEEF,
        }
    }

    // -----------------------------------------------------------------------
    // WitnessLog: capacity enforcement (ring buffer)
    // -----------------------------------------------------------------------

    #[test]
    fn test_capacity_enforcement() {
        let mut log = WitnessLog::new(3);
        log.record(1, make_access(1, 1.0, Tier::Tier1));
        log.record(2, make_access(2, 2.0, Tier::Tier2));
        log.record(3, make_access(3, 3.0, Tier::Tier3));
        assert_eq!(log.len(), 3);

        // Fourth record should evict the oldest (timestamp=1).
        log.record(4, make_access(4, 4.0, Tier::Tier1));
        assert_eq!(log.len(), 3);
        assert_eq!(log.all()[0].timestamp, 2);
        assert_eq!(log.all()[2].timestamp, 4);
    }

    #[test]
    fn test_capacity_zero_treated_as_one() {
        let mut log = WitnessLog::new(0);
        log.record(1, make_access(1, 1.0, Tier::Tier1));
        assert_eq!(log.len(), 1);
        log.record(2, make_access(2, 2.0, Tier::Tier2));
        assert_eq!(log.len(), 1);
        assert_eq!(log.all()[0].timestamp, 2);
    }

    // -----------------------------------------------------------------------
    // WitnessLog: recording and retrieval
    // -----------------------------------------------------------------------

    #[test]
    fn test_record_and_retrieve_all() {
        let mut log = WitnessLog::new(100);
        log.record(10, make_access(1, 1.0, Tier::Tier1));
        log.record(20, make_eviction(2));
        log.record(30, make_tier_change(3, Tier::Tier3, Tier::Tier2));

        let all = log.all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].timestamp, 10);
        assert_eq!(all[1].timestamp, 20);
        assert_eq!(all[2].timestamp, 30);
    }

    #[test]
    fn test_recent_returns_tail() {
        let mut log = WitnessLog::new(100);
        for i in 0..10 {
            log.record(i, make_access(i, i as f32, Tier::Tier1));
        }

        let recent = log.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].timestamp, 7);
        assert_eq!(recent[1].timestamp, 8);
        assert_eq!(recent[2].timestamp, 9);
    }

    #[test]
    fn test_recent_more_than_available() {
        let mut log = WitnessLog::new(100);
        log.record(1, make_access(1, 1.0, Tier::Tier1));
        let recent = log.recent(50);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn test_clear() {
        let mut log = WitnessLog::new(100);
        log.record(1, make_access(1, 1.0, Tier::Tier1));
        log.record(2, make_eviction(2));
        assert_eq!(log.len(), 2);

        log.clear();
        assert_eq!(log.len(), 0);
        assert!(log.is_empty());
    }

    // -----------------------------------------------------------------------
    // WitnessLog: counting by event type
    // -----------------------------------------------------------------------

    #[test]
    fn test_count_tier_changes() {
        let mut log = WitnessLog::new(100);
        log.record(1, make_tier_change(1, Tier::Tier3, Tier::Tier2));
        log.record(2, make_access(2, 1.0, Tier::Tier1));
        log.record(3, make_tier_change(3, Tier::Tier2, Tier::Tier1));
        log.record(4, make_eviction(4));

        assert_eq!(log.count_tier_changes(), 2);
    }

    #[test]
    fn test_count_evictions() {
        let mut log = WitnessLog::new(100);
        log.record(1, make_eviction(1));
        log.record(2, make_eviction(2));
        log.record(3, make_access(3, 1.0, Tier::Tier1));
        log.record(4, make_eviction(3));

        assert_eq!(log.count_evictions(), 3);
    }

    #[test]
    fn test_count_checksum_failures() {
        let mut log = WitnessLog::new(100);
        log.record(1, make_checksum_failure(1));
        log.record(2, make_access(2, 1.0, Tier::Tier1));
        log.record(3, make_checksum_failure(3));

        assert_eq!(log.count_checksum_failures(), 2);
    }

    // -----------------------------------------------------------------------
    // WitnessLog: tier flip rate
    // -----------------------------------------------------------------------

    #[test]
    fn test_tier_flip_rate_basic() {
        let mut log = WitnessLog::new(100);
        // 4 tier changes in the window, 10 blocks.
        for i in 0..4 {
            log.record(100 + i, make_tier_change(i, Tier::Tier3, Tier::Tier2));
        }
        // Some non-tier-change events.
        log.record(101, make_access(5, 1.0, Tier::Tier1));

        let rate = log.tier_flip_rate(200, 10);
        // 4 tier changes in window / 10 blocks = 0.4
        assert!((rate - 0.4).abs() < 1e-6, "rate={rate}");
    }

    #[test]
    fn test_tier_flip_rate_windowed() {
        let mut log = WitnessLog::new(100);
        // Old tier changes (outside window).
        log.record(10, make_tier_change(1, Tier::Tier3, Tier::Tier2));
        log.record(20, make_tier_change(2, Tier::Tier3, Tier::Tier1));
        // Recent tier changes (inside window of 50 ticks from max=200).
        log.record(160, make_tier_change(3, Tier::Tier2, Tier::Tier1));
        log.record(200, make_tier_change(4, Tier::Tier1, Tier::Tier2));

        let rate = log.tier_flip_rate(50, 5);
        // Window: [200-50, 200] = [150, 200]. Records at 160 and 200 qualify.
        // 2 flips / 5 blocks = 0.4
        assert!((rate - 0.4).abs() < 1e-6, "rate={rate}");
    }

    #[test]
    fn test_tier_flip_rate_zero_blocks() {
        let mut log = WitnessLog::new(100);
        log.record(1, make_tier_change(1, Tier::Tier3, Tier::Tier2));
        assert_eq!(log.tier_flip_rate(100, 0), 0.0);
    }

    #[test]
    fn test_tier_flip_rate_empty_log() {
        let log = WitnessLog::new(100);
        assert_eq!(log.tier_flip_rate(100, 10), 0.0);
    }

    // -----------------------------------------------------------------------
    // StoreMetrics: compression ratio
    // -----------------------------------------------------------------------

    #[test]
    fn test_compression_ratio_zero_bytes() {
        let m = StoreMetrics::new();
        assert_eq!(m.compression_ratio(), 0.0);
    }

    #[test]
    fn test_compression_ratio_nonzero() {
        let m = StoreMetrics {
            tier1_bytes: 1000,
            tier2_bytes: 500,
            tier3_bytes: 200,
            ..Default::default()
        };
        // raw_estimate = 1000*4.0 + 500*5.5 + 200*10.67 = 4000 + 2750 + 2134 = 8884
        // stored = 1000 + 500 + 200 = 1700
        // ratio = 8884 / 1700 ~= 5.226
        let ratio = m.compression_ratio();
        assert!(ratio > 5.0 && ratio < 5.5, "ratio={ratio}");
    }

    #[test]
    fn test_total_stored_bytes() {
        let m = StoreMetrics {
            tier1_bytes: 100,
            tier2_bytes: 200,
            tier3_bytes: 300,
            ..Default::default()
        };
        assert_eq!(m.total_stored_bytes(), 600);
    }

    // -----------------------------------------------------------------------
    // StoreSnapshot: serialization
    // -----------------------------------------------------------------------

    #[test]
    fn test_snapshot_to_bytes_contains_keys() {
        let snap = StoreSnapshot {
            timestamp: 42,
            metrics: StoreMetrics {
                total_blocks: 10,
                tier0_blocks: 2,
                tier1_blocks: 3,
                tier2_blocks: 3,
                tier3_blocks: 2,
                tier1_bytes: 1000,
                tier2_bytes: 500,
                tier3_bytes: 200,
                total_reads: 100,
                total_writes: 50,
                ..Default::default()
            },
            tier_distribution: [2, 3, 3, 2],
            byte_distribution: [8000, 1000, 500, 200],
        };

        let bytes = snap.to_bytes();
        let text = core::str::from_utf8(&bytes).expect("valid utf-8");

        assert!(text.contains("timestamp=42\n"), "missing timestamp");
        assert!(text.contains("total_blocks=10\n"), "missing total_blocks");
        assert!(text.contains("tier1_bytes=1000\n"), "missing tier1_bytes");
        assert!(text.contains("total_reads=100\n"), "missing total_reads");
        assert!(text.contains("total_writes=50\n"), "missing total_writes");
        assert!(text.contains("tier_dist[0]=2\n"), "missing tier_dist[0]");
        assert!(text.contains("tier_dist[3]=2\n"), "missing tier_dist[3]");
        assert!(text.contains("byte_dist[1]=1000\n"), "missing byte_dist[1]");
        assert!(
            text.contains("compression_ratio="),
            "missing compression_ratio"
        );
        assert!(
            text.contains("total_stored_bytes=1700\n"),
            "missing total_stored_bytes"
        );
    }

    #[test]
    fn test_snapshot_empty_metrics() {
        let snap = StoreSnapshot {
            timestamp: 0,
            metrics: StoreMetrics::default(),
            tier_distribution: [0; 4],
            byte_distribution: [0; 4],
        };

        let bytes = snap.to_bytes();
        let text = core::str::from_utf8(&bytes).expect("valid utf-8");

        assert!(text.contains("timestamp=0\n"));
        assert!(text.contains("total_blocks=0\n"));
        assert!(text.contains("total_stored_bytes=0\n"));
    }

    // -----------------------------------------------------------------------
    // Empty log edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_log_len() {
        let log = WitnessLog::new(10);
        assert_eq!(log.len(), 0);
        assert!(log.is_empty());
    }

    #[test]
    fn test_empty_log_recent() {
        let log = WitnessLog::new(10);
        assert!(log.recent(5).is_empty());
    }

    #[test]
    fn test_empty_log_counts() {
        let log = WitnessLog::new(10);
        assert_eq!(log.count_tier_changes(), 0);
        assert_eq!(log.count_evictions(), 0);
        assert_eq!(log.count_checksum_failures(), 0);
    }

    #[test]
    fn test_empty_log_clear_is_noop() {
        let mut log = WitnessLog::new(10);
        log.clear();
        assert!(log.is_empty());
    }

    // -----------------------------------------------------------------------
    // Serialization helpers
    // -----------------------------------------------------------------------

    #[test]
    fn test_push_u64_zero() {
        let mut buf = Vec::new();
        push_u64(&mut buf, 0);
        assert_eq!(&buf, b"0");
    }

    #[test]
    fn test_push_u64_large() {
        let mut buf = Vec::new();
        push_u64(&mut buf, 123456789);
        assert_eq!(&buf, b"123456789");
    }

    #[test]
    fn test_push_f32_positive() {
        let mut buf = Vec::new();
        push_f32(&mut buf, 3.14);
        let s = core::str::from_utf8(&buf).unwrap();
        // Should start with "3." and have fractional digits close to 140000.
        assert!(s.starts_with("3."), "got: {s}");
        let frac: u64 = s.split('.').nth(1).unwrap().parse().unwrap();
        // Allow rounding: 3.14 -> frac ~= 140000 (within 100 of 140000).
        assert!(
            (frac as i64 - 140000).unsigned_abs() < 200,
            "frac={frac}, expected ~140000"
        );
    }

    #[test]
    fn test_push_f32_negative() {
        let mut buf = Vec::new();
        push_f32(&mut buf, -1.5);
        let s = core::str::from_utf8(&buf).unwrap();
        assert!(s.starts_with("-1."), "got: {s}");
    }

    // -----------------------------------------------------------------------
    // StoreMetrics: format_report
    // -----------------------------------------------------------------------

    #[test]
    fn test_format_report_contains_sections() {
        let m = StoreMetrics {
            total_blocks: 100,
            tier1_blocks: 50,
            tier2_blocks: 30,
            tier3_blocks: 20,
            tier1_bytes: 5000,
            tier2_bytes: 3000,
            tier3_bytes: 1000,
            total_reads: 1000,
            total_writes: 500,
            ..Default::default()
        };
        let report = m.format_report();
        assert!(report.contains("Temporal Tensor Store Report"));
        assert!(report.contains("Total blocks: 100"));
        assert!(report.contains("Reads: 1000"));
        assert!(report.contains("Compression ratio:"));
    }

    #[test]
    fn test_format_json_valid_structure() {
        let m = StoreMetrics {
            total_blocks: 10,
            tier1_bytes: 100,
            ..Default::default()
        };
        let json = m.format_json();
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
        assert!(json.contains("\"total_blocks\":10"));
        assert!(json.contains("\"tier1_bytes\":100"));
    }

    // -----------------------------------------------------------------------
    // StoreMetrics: health_check
    // -----------------------------------------------------------------------

    #[test]
    fn test_health_check_healthy() {
        let m = StoreMetrics {
            total_blocks: 100,
            total_reads: 1000,
            total_writes: 500,
            ..Default::default()
        };
        assert_eq!(m.health_check(), StoreHealthStatus::Healthy);
    }

    #[test]
    fn test_health_check_critical_checksum() {
        let m = StoreMetrics {
            total_checksum_failures: 5,
            ..Default::default()
        };
        match m.health_check() {
            StoreHealthStatus::Critical(msg) => assert!(msg.contains("checksum")),
            other => panic!("expected Critical, got {:?}", other),
        }
    }

    #[test]
    fn test_health_check_warning_flip_rate() {
        let m = StoreMetrics {
            tier_flips_last_minute: 0.8,
            ..Default::default()
        };
        match m.health_check() {
            StoreHealthStatus::Warning(msg) => assert!(msg.contains("flip rate")),
            other => panic!("expected Warning, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // MetricsSeries
    // -----------------------------------------------------------------------

    #[test]
    fn test_metrics_series_record_and_latest() {
        let mut series = MetricsSeries::new(10);
        assert!(series.is_empty());
        series.record(
            1,
            StoreMetrics {
                total_blocks: 10,
                ..Default::default()
            },
        );
        series.record(
            2,
            StoreMetrics {
                total_blocks: 20,
                ..Default::default()
            },
        );
        assert_eq!(series.len(), 2);
        assert_eq!(series.latest().unwrap().1.total_blocks, 20);
    }

    #[test]
    fn test_metrics_series_capacity() {
        let mut series = MetricsSeries::new(3);
        for i in 0..5 {
            series.record(
                i as u64,
                StoreMetrics {
                    total_blocks: i,
                    ..Default::default()
                },
            );
        }
        assert_eq!(series.len(), 3);
        assert_eq!(series.latest().unwrap().1.total_blocks, 4);
    }

    #[test]
    fn test_metrics_trend_empty() {
        let series = MetricsSeries::new(10);
        let trend = series.trend();
        assert_eq!(trend.eviction_rate, 0.0);
        assert!(trend.tier_distribution_stable);
    }

    #[test]
    fn test_metrics_trend_with_data() {
        let mut series = MetricsSeries::new(10);
        for i in 0..6u64 {
            series.record(
                i,
                StoreMetrics {
                    total_blocks: 100,
                    tier1_blocks: 50,
                    total_evictions: i * 2,
                    tier1_bytes: 5000 + i * 100,
                    tier2_bytes: 3000,
                    tier3_bytes: 1000,
                    ..Default::default()
                },
            );
        }
        let trend = series.trend();
        assert!(trend.eviction_rate > 0.0);
    }
}
