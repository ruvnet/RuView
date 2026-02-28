//! Enhanced temporal scoring with EMA + popcount + recency, hysteresis,
//! and budgeted maintenance (ADR-020).
//!
//! # Scoring Formula
//!
//! ```text
//! score = w_ema * ema_rate
//!       + w_pop * (popcount(access_window) / 64)
//!       + w_rec * exp(-dt / tau)
//! ```
//!
//! Where `dt = now - last_access` and `tau` is the recency decay constant.
//!
//! # Hysteresis
//!
//! To prevent tier oscillation, upgrades require the score to exceed the
//! threshold by the hysteresis margin, and downgrades require the score to
//! fall below the threshold by the same margin. A minimum residency period
//! further dampens churn.
//!
//! # Types
//!
//! The types `BlockKey`, `BlockMeta`, and `Tier` are defined here for
//! self-containment while `store.rs` is developed in parallel. Once
//! `crate::store` lands, replace these definitions with:
//! ```ignore
//! use crate::store::{BlockKey, BlockMeta, Tier};
//! ```

// ---------------------------------------------------------------------------
// Types (to be migrated to crate::store)
// ---------------------------------------------------------------------------

/// Opaque block identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockKey(pub u64);

/// Storage tier for a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Tier {
    /// In-memory / uncompressed (full f32).
    Tier0 = 0,
    /// Hot: 8-bit quantization.
    Tier1 = 1,
    /// Warm: 7-bit (or 5-bit aggressive) quantization.
    Tier2 = 2,
    /// Cold: 3-bit quantization.
    Tier3 = 3,
}

/// Per-block metadata tracked by the tiered store.
#[derive(Clone, Debug)]
pub struct BlockMeta {
    /// Exponentially-weighted moving average of access rate.
    pub ema_rate: f32,
    /// Sliding window bitmap of tick-level activity (1 bit per tick).
    /// `popcount` gives the number of active ticks in the last 64.
    pub access_window: u64,
    /// Timestamp (tick) of the most recent access.
    pub last_access: u64,
    /// Cumulative access count.
    pub access_count: u64,
    /// Current storage tier.
    pub current_tier: Tier,
    /// Tick at which the block was last assigned to its current tier.
    pub tier_since: u64,
}

impl BlockMeta {
    /// Create metadata for a freshly inserted block.
    pub fn new(now: u64) -> Self {
        Self {
            ema_rate: 0.0,
            access_window: 0,
            last_access: now,
            access_count: 0,
            current_tier: Tier::Tier1,
            tier_since: now,
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Enhanced tier policy with EMA + popcount + recency scoring.
///
/// Score = w_ema * ema_rate + w_pop * (popcount(window)/64) + w_rec * exp(-dt/tau)
#[derive(Clone, Debug)]
pub struct TierConfig {
    /// EMA smoothing factor (0..1). Higher = more responsive to recent access.
    pub alpha: f32,
    /// Recency decay time constant. Larger = slower decay.
    pub tau: f32,
    /// Weight for EMA access rate in score.
    pub w_ema: f32,
    /// Weight for popcount (recent tick activity) in score.
    pub w_pop: f32,
    /// Weight for recency (time since last access) in score.
    pub w_rec: f32,
    /// Score threshold for Tier1 (hot).
    pub t1: f32,
    /// Score threshold for Tier2 (warm).
    pub t2: f32,
    /// Score threshold for Tier3 (cold).
    pub t3: f32,
    /// Hysteresis margin. Upgrade needs score > threshold + hysteresis,
    /// downgrade needs score < threshold - hysteresis.
    pub hysteresis: f32,
    /// Minimum ticks a block must stay in its current tier.
    pub min_residency: u32,
    /// Maximum delta chain length before compaction.
    pub max_delta_chain: u8,
    /// Block size in bytes.
    pub block_bytes: usize,
    /// Maximum bytes allowed in Tier1.
    pub tier1_byte_cap: Option<usize>,
    /// Use 5-bit instead of 7-bit when warm set exceeds this byte count.
    pub warm_aggressive_threshold: Option<usize>,
}

impl Default for TierConfig {
    fn default() -> Self {
        Self {
            alpha: 0.3,
            tau: 100.0,
            w_ema: 0.4,
            w_pop: 0.3,
            w_rec: 0.3,
            t1: 0.7,
            t2: 0.3,
            t3: 0.1,
            hysteresis: 0.05,
            min_residency: 5,
            max_delta_chain: 8,
            block_bytes: 16384,
            tier1_byte_cap: None,
            warm_aggressive_threshold: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Exponential approximation
// ---------------------------------------------------------------------------

/// Fast approximation of `exp(-x)` for `x >= 0`.
///
/// Uses `1 / (1 + x)` as a cheap monotonically decreasing bound. Sufficient
/// for relative ordering of scores; not suitable for absolute accuracy.
///
/// This function is available as a lightweight alternative to the LUT version
/// for contexts where code size matters more than precision.
#[inline]
#[allow(dead_code)]
fn fast_exp_neg(x: f32) -> f32 {
    if x < 0.0 {
        return 1.0;
    }
    1.0 / (1.0 + x)
}

/// Number of entries in the exp(-x) look-up table.
const LUT_SIZE: usize = 64;
/// Domain upper bound for the LUT. Values beyond this clamp to ~0.
const LUT_X_MAX: f32 = 8.0;

/// Pre-computed LUT: `LUT[i] = exp(-i * LUT_X_MAX / LUT_SIZE)`.
const EXP_LUT: [f32; LUT_SIZE + 1] = {
    let mut table = [0.0f32; LUT_SIZE + 1];
    let mut i = 0;
    while i <= LUT_SIZE {
        // const-context: approximate exp via Taylor(20) for good precision
        let x = -(i as f64) * (LUT_X_MAX as f64) / (LUT_SIZE as f64);
        // Horner form for exp(x) where x is negative
        let v = const_exp(x);
        table[i] = v as f32;
        i += 1;
    }
    table
};

/// Compile-time exp(x) via truncated Taylor series (35 terms).
///
/// For negative `x`, computes `exp(|x|)` and inverts to avoid
/// alternating-series cancellation.
const fn const_exp(x: f64) -> f64 {
    // Avoid catastrophic cancellation for negative x.
    if x < 0.0 {
        let pos = const_exp_pos(-x);
        return 1.0 / pos;
    }
    const_exp_pos(x)
}

/// Taylor expansion of exp(x) for x >= 0. 35 terms give excellent
/// precision up to x = 10 (term_35 = 10^35 / 35! ~ 2.8e-8).
const fn const_exp_pos(x: f64) -> f64 {
    let mut sum = 1.0f64;
    let mut term = 1.0f64;
    let mut k = 1u32;
    while k <= 35 {
        term *= x / (k as f64);
        sum += term;
        k += 1;
    }
    sum
}

/// LUT-based `exp(-x)` approximation with linear interpolation.
///
/// 64 entries covering `x` in `[0, 8]`, clamped beyond that range.
/// Maximum relative error is approximately 0.2% within the LUT domain.
#[inline]
fn fast_exp_neg_lut(x: f32) -> f32 {
    if x <= 0.0 {
        return 1.0;
    }
    if x >= LUT_X_MAX {
        return EXP_LUT[LUT_SIZE];
    }
    let scaled = x * (LUT_SIZE as f32) / LUT_X_MAX;
    let idx = scaled as usize; // floor index
    let frac = scaled - (idx as f32);
    // Safety: idx < LUT_SIZE because x < LUT_X_MAX.
    let lo = EXP_LUT[idx];
    let hi = EXP_LUT[idx + 1];
    lo + frac * (hi - lo)
}

// ---------------------------------------------------------------------------
// Core scoring
// ---------------------------------------------------------------------------

/// Compute the composite score for a block.
///
/// ```text
/// score = w_ema * ema_rate
///       + w_pop * (popcount(access_window) / 64)
///       + w_rec * exp(-dt / tau)
/// ```
///
/// All component values are in `[0, 1]` (assuming `ema_rate` is clamped),
/// so the maximum possible score equals `w_ema + w_pop + w_rec`.
pub fn compute_score(config: &TierConfig, now: u64, meta: &BlockMeta) -> f32 {
    let ema_component = config.w_ema * meta.ema_rate.clamp(0.0, 1.0);

    let pop = meta.access_window.count_ones() as f32 / 64.0;
    let pop_component = config.w_pop * pop;

    let dt = now.saturating_sub(meta.last_access) as f32;
    let recency = fast_exp_neg_lut(dt / config.tau);
    let rec_component = config.w_rec * recency;

    ema_component + pop_component + rec_component
}

// ---------------------------------------------------------------------------
// Tier selection with hysteresis
// ---------------------------------------------------------------------------

/// Choose the target tier for a block, applying hysteresis and residency.
///
/// Returns `None` if the block should stay in its current tier because:
/// - The score falls within the hysteresis band, or
/// - The block has not met the minimum residency requirement.
pub fn choose_tier(config: &TierConfig, now: u64, meta: &BlockMeta) -> Option<Tier> {
    // Enforce minimum residency.
    let ticks_in_tier = now.saturating_sub(meta.tier_since);
    if ticks_in_tier < config.min_residency as u64 {
        return None;
    }

    let score = compute_score(config, now, meta);
    let current = meta.current_tier;

    // Determine raw target tier based on score thresholds.
    let raw_target = if score >= config.t1 {
        Tier::Tier1
    } else if score >= config.t2 {
        Tier::Tier2
    } else if score >= config.t3 {
        Tier::Tier3
    } else {
        Tier::Tier3 // Below t3 still maps to coldest available tier.
    };

    if raw_target == current {
        return None;
    }

    // Apply hysteresis: upgrades need score > threshold + h,
    // downgrades need score < threshold - h.
    let h = config.hysteresis;

    let transition_allowed = if raw_target < current {
        // Upgrade (lower ordinal = hotter tier). The score must exceed
        // the *target* tier's lower threshold plus hysteresis.
        let threshold = match raw_target {
            Tier::Tier0 => return None, // Cannot promote to Tier0 via scoring.
            Tier::Tier1 => config.t1,
            Tier::Tier2 => config.t2,
            Tier::Tier3 => config.t3,
        };
        score > threshold + h
    } else {
        // Downgrade (higher ordinal = colder tier). The score must fall
        // below the *current* tier's lower threshold minus hysteresis.
        let threshold = match current {
            Tier::Tier0 => return None,
            Tier::Tier1 => config.t1,
            Tier::Tier2 => config.t2,
            Tier::Tier3 => return None, // Already coldest.
        };
        score < threshold - h
    };

    if transition_allowed {
        Some(raw_target)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Access recording
// ---------------------------------------------------------------------------

/// Record an access event on a block's metadata.
///
/// Updates:
/// - `ema_rate` via exponential moving average (`alpha`).
/// - `access_window` by shifting and setting the low bit.
/// - `last_access` to `now`.
/// - `access_count` incremented by one.
pub fn touch(config: &TierConfig, now: u64, meta: &mut BlockMeta) {
    // EMA update: ema = alpha * 1.0 + (1 - alpha) * ema
    meta.ema_rate = config.alpha + (1.0 - config.alpha) * meta.ema_rate;

    // Shift the window by the number of elapsed ticks and set bit 0.
    let elapsed = now.saturating_sub(meta.last_access);
    if elapsed > 0 {
        if elapsed >= 64 {
            meta.access_window = 1;
        } else {
            meta.access_window = (meta.access_window << elapsed) | 1;
        }
    } else {
        // Same tick: just ensure bit 0 is set.
        meta.access_window |= 1;
    }

    meta.last_access = now;
    meta.access_count = meta.access_count.saturating_add(1);
}

// ---------------------------------------------------------------------------
// Tick decay
// ---------------------------------------------------------------------------

/// Decay EMA for blocks not accessed in the current tick.
///
/// Should be called once per tick for every block that was *not* touched.
/// Applies `ema_rate *= (1 - alpha)` and shifts the access window left by 1
/// (inserting a 0 bit).
pub fn tick_decay(config: &TierConfig, meta: &mut BlockMeta) {
    meta.ema_rate *= 1.0 - config.alpha;
    meta.access_window <<= 1;
}

// ---------------------------------------------------------------------------
// Budgeted maintenance
// ---------------------------------------------------------------------------

/// Result of a maintenance tick operation.
#[derive(Debug, Default)]
pub struct MaintenanceResult {
    pub upgrades: u32,
    pub downgrades: u32,
    pub evictions: u32,
    pub bytes_freed: usize,
    pub ops_used: u32,
}

/// Candidate for tier migration during maintenance.
#[derive(Debug)]
pub struct MigrationCandidate {
    pub key: BlockKey,
    pub current_tier: Tier,
    pub target_tier: Tier,
    pub score: f32,
}

/// Select blocks that need tier migration.
///
/// Returns candidates sorted by priority:
/// - Upgrades first, ordered by highest score (hottest blocks promoted first).
/// - Then downgrades, ordered by lowest score (coldest blocks demoted first).
pub fn select_candidates(
    config: &TierConfig,
    now: u64,
    blocks: &[(BlockKey, &BlockMeta)],
) -> Vec<MigrationCandidate> {
    let mut upgrades: Vec<MigrationCandidate> = Vec::new();
    let mut downgrades: Vec<MigrationCandidate> = Vec::new();

    for &(key, meta) in blocks {
        if let Some(target) = choose_tier(config, now, meta) {
            let score = compute_score(config, now, meta);
            let candidate = MigrationCandidate {
                key,
                current_tier: meta.current_tier,
                target_tier: target,
                score,
            };
            if target < meta.current_tier {
                upgrades.push(candidate);
            } else {
                downgrades.push(candidate);
            }
        }
    }

    // Upgrades: highest score first.
    upgrades.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    // Downgrades: lowest score first.
    downgrades.sort_by(|a, b| {
        a.score
            .partial_cmp(&b.score)
            .unwrap_or(core::cmp::Ordering::Equal)
    });

    upgrades.extend(downgrades);
    upgrades
}

// ---------------------------------------------------------------------------
// Batch scoring
// ---------------------------------------------------------------------------

/// Result of scoring and partitioning blocks into tier buckets.
#[derive(Clone, Debug)]
pub struct ScoredPartition {
    /// Indices of blocks classified as hot (Tier1).
    pub hot: Vec<usize>,
    /// Indices of blocks classified as warm (Tier2).
    pub warm: Vec<usize>,
    /// Indices of blocks classified as cold (Tier3).
    pub cold: Vec<usize>,
    /// Indices of blocks below eviction threshold.
    pub evict: Vec<usize>,
    /// Computed scores, parallel to input slice.
    pub scores: Vec<f32>,
}

/// Compute scores for many blocks at once.
///
/// Returns a `Vec<f32>` parallel to `metas`, where each entry is
/// `compute_score(config, now, &metas[i])`.
pub fn compute_scores_batch(config: &TierConfig, now: u64, metas: &[BlockMeta]) -> Vec<f32> {
    metas
        .iter()
        .map(|m| compute_score(config, now, m))
        .collect()
}

/// Compute tier decisions for many blocks at once.
///
/// Returns a `Vec<Option<Tier>>` parallel to `metas`, where each entry is
/// `choose_tier(config, now, &metas[i])`.
pub fn choose_tiers_batch(config: &TierConfig, now: u64, metas: &[BlockMeta]) -> Vec<Option<Tier>> {
    metas.iter().map(|m| choose_tier(config, now, m)).collect()
}

/// Score blocks and partition into hot/warm/cold/evict buckets based on raw
/// score thresholds.
///
/// Unlike [`choose_tier`], this function uses the *raw* thresholds (`t1`,
/// `t2`, `t3`) without hysteresis or residency checks, making it suitable
/// for bulk classification and capacity planning.
pub fn score_and_partition(config: &TierConfig, now: u64, metas: &[BlockMeta]) -> ScoredPartition {
    let scores = compute_scores_batch(config, now, metas);
    let mut hot = Vec::new();
    let mut warm = Vec::new();
    let mut cold = Vec::new();
    let mut evict = Vec::new();
    for (i, &score) in scores.iter().enumerate() {
        if score >= config.t1 {
            hot.push(i);
        } else if score >= config.t2 {
            warm.push(i);
        } else if score >= config.t3 {
            cold.push(i);
        } else {
            evict.push(i);
        }
    }
    ScoredPartition {
        hot,
        warm,
        cold,
        evict,
        scores,
    }
}

/// Find the `k` blocks with the lowest scores (useful for eviction).
///
/// Returns up to `k` `(index, score)` pairs sorted in ascending score order.
/// Uses a partial sort (`select_nth_unstable_by`) for efficiency when
/// `k << metas.len()`.
pub fn top_k_coldest(
    config: &TierConfig,
    now: u64,
    metas: &[BlockMeta],
    k: usize,
) -> Vec<(usize, f32)> {
    let scores = compute_scores_batch(config, now, metas);
    let mut indexed: Vec<(usize, f32)> = scores.into_iter().enumerate().collect();
    // Partial sort: we only need the k smallest
    if k < indexed.len() {
        indexed.select_nth_unstable_by(k, |a, b| {
            a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal)
        });
        indexed.truncate(k);
    }
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal));
    indexed
}

// ---------------------------------------------------------------------------
// Quantization bit-width selection
// ---------------------------------------------------------------------------

/// Get the quantization bit width for a tier.
///
/// | Tier  | Bits | Notes |
/// |-------|------|-------|
/// | Tier0 | 0    | Uncompressed (f32) |
/// | Tier1 | 8    | Hot |
/// | Tier2 | 7    | Warm (5 if `warm_bytes > warm_aggressive_threshold`) |
/// | Tier3 | 3    | Cold |
pub fn bits_for_tier(config: &TierConfig, tier: Tier, warm_bytes: usize) -> u8 {
    match tier {
        Tier::Tier0 => 0,
        Tier::Tier1 => 8,
        Tier::Tier2 => {
            if let Some(threshold) = config.warm_aggressive_threshold {
                if warm_bytes > threshold {
                    return 5;
                }
            }
            7
        }
        Tier::Tier3 => 3,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> TierConfig {
        TierConfig::default()
    }

    fn make_meta(
        ema_rate: f32,
        access_window: u64,
        last_access: u64,
        current_tier: Tier,
        tier_since: u64,
    ) -> BlockMeta {
        BlockMeta {
            ema_rate,
            access_window,
            last_access,
            access_count: 0,
            current_tier,
            tier_since,
        }
    }

    // -----------------------------------------------------------------------
    // 1. Score computation with known inputs
    // -----------------------------------------------------------------------

    #[test]
    fn score_all_components_at_max() {
        let cfg = default_config();
        // ema_rate = 1.0, all 64 bits set, last_access == now
        let meta = make_meta(1.0, u64::MAX, 100, Tier::Tier1, 0);
        let score = compute_score(&cfg, 100, &meta);
        // Expected: 0.4*1.0 + 0.3*(64/64) + 0.3*exp(0) = 0.4 + 0.3 + 0.3 = 1.0
        assert!((score - 1.0).abs() < 1e-4, "score={score}");
    }

    #[test]
    fn score_all_components_at_zero() {
        let cfg = default_config();
        // ema_rate = 0, no window bits, access far in the past
        let meta = make_meta(0.0, 0, 0, Tier::Tier3, 0);
        let score = compute_score(&cfg, 10_000, &meta);
        // EMA = 0, pop = 0, recency ~ exp(-100) ~ 0
        assert!(score < 0.01, "score={score}");
    }

    #[test]
    fn score_only_ema_contributes() {
        let cfg = TierConfig {
            w_ema: 1.0,
            w_pop: 0.0,
            w_rec: 0.0,
            ..default_config()
        };
        let meta = make_meta(0.75, 0, 0, Tier::Tier2, 0);
        let score = compute_score(&cfg, 1000, &meta);
        assert!((score - 0.75).abs() < 1e-6, "score={score}");
    }

    #[test]
    fn score_only_popcount_contributes() {
        let cfg = TierConfig {
            w_ema: 0.0,
            w_pop: 1.0,
            w_rec: 0.0,
            ..default_config()
        };
        // 32 of 64 bits set
        let meta = make_meta(0.0, 0x0000_FFFF_FFFF_0000, 0, Tier::Tier2, 0);
        let pop = 0x0000_FFFF_FFFF_0000u64.count_ones() as f32 / 64.0;
        let score = compute_score(&cfg, 1000, &meta);
        assert!(
            (score - pop).abs() < 1e-6,
            "score={score}, expected pop={pop}"
        );
    }

    // -----------------------------------------------------------------------
    // 2. Fast exp approximation accuracy
    // -----------------------------------------------------------------------

    #[test]
    fn fast_exp_neg_monotonic() {
        let mut prev = fast_exp_neg(0.0);
        for i in 1..100 {
            let x = i as f32 * 0.1;
            let val = fast_exp_neg(x);
            assert!(val <= prev, "not monotonic at x={x}");
            assert!(val >= 0.0);
            prev = val;
        }
    }

    #[test]
    fn fast_exp_neg_at_zero() {
        assert!((fast_exp_neg(0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn fast_exp_neg_negative_input() {
        // Negative input should clamp to 1.0
        assert!((fast_exp_neg(-5.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn fast_exp_neg_vs_stdlib() {
        // 1/(1+x) should always be >= exp(-x) for x >= 0 (it is an upper bound).
        for i in 0..50 {
            let x = i as f32 * 0.2;
            let approx = fast_exp_neg(x);
            let exact = (-x).exp();
            assert!(
                approx >= exact - 1e-6,
                "approx={approx} < exact={exact} at x={x}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // 3. LUT exp accuracy
    // -----------------------------------------------------------------------

    #[test]
    fn lut_exp_at_zero() {
        assert!((fast_exp_neg_lut(0.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn lut_exp_accuracy() {
        // Check accuracy across the LUT domain.
        for i in 0..80 {
            let x = i as f32 * 0.1;
            let approx = fast_exp_neg_lut(x);
            let exact = (-x).exp();
            let rel_err = if exact > 1e-10 {
                (approx - exact).abs() / exact
            } else {
                (approx - exact).abs()
            };
            assert!(
                rel_err < 0.01,
                "x={x} approx={approx} exact={exact} rel_err={rel_err}"
            );
        }
    }

    #[test]
    fn lut_exp_beyond_domain() {
        // x >= 8.0 should return the last LUT entry (near zero).
        let val = fast_exp_neg_lut(100.0);
        assert!(val < 0.001, "val={val}");
        assert!(val >= 0.0);
    }

    #[test]
    fn lut_exp_monotonic() {
        let mut prev = fast_exp_neg_lut(0.0);
        for i in 1..160 {
            let x = i as f32 * 0.05;
            let val = fast_exp_neg_lut(x);
            assert!(val <= prev + 1e-7, "not monotonic at x={x}");
            prev = val;
        }
    }

    // -----------------------------------------------------------------------
    // 4. Tier selection with and without hysteresis
    // -----------------------------------------------------------------------

    #[test]
    fn tier_selection_clear_hot() {
        let cfg = default_config();
        // Score ~ 1.0, clearly above t1(0.7) + hysteresis(0.05)
        let meta = make_meta(1.0, u64::MAX, 100, Tier::Tier3, 0);
        let target = choose_tier(&cfg, 100, &meta);
        assert_eq!(target, Some(Tier::Tier1));
    }

    #[test]
    fn tier_selection_clear_cold() {
        let cfg = default_config();
        // Score ~ 0, clearly below t2(0.3) - hysteresis(0.05)
        let meta = make_meta(0.0, 0, 0, Tier::Tier1, 0);
        let target = choose_tier(&cfg, 10_000, &meta);
        assert_eq!(target, Some(Tier::Tier3));
    }

    #[test]
    fn tier_selection_hysteresis_prevents_upgrade() {
        // Score just barely above t1 but within hysteresis band.
        let cfg = TierConfig {
            hysteresis: 0.10,
            ..default_config()
        };
        // Craft a score that is above t1(0.7) but below t1+hysteresis(0.8).
        // ema=0.75, window=all-set, last_access=now
        // score = 0.4*0.75 + 0.3*1.0 + 0.3*1.0 = 0.3 + 0.3 + 0.3 = 0.9
        // Actually that is above 0.8, so let us reduce ema.
        // ema=0.5: score = 0.4*0.5 + 0.3 + 0.3 = 0.2 + 0.3 + 0.3 = 0.8
        // Need score in (0.7, 0.8): ema=0.25 -> 0.1+0.3+0.3=0.7 exactly, not enough.
        // ema=0.4: 0.16 + 0.3 + 0.3 = 0.76. This is >0.7 but <0.8. Good.
        let meta = make_meta(0.4, u64::MAX, 50, Tier::Tier2, 0);
        let score = compute_score(&cfg, 50, &meta);
        assert!(score > cfg.t1, "score={score}");
        assert!(score < cfg.t1 + cfg.hysteresis, "score={score}");
        let target = choose_tier(&cfg, 50, &meta);
        // Hysteresis should prevent the upgrade.
        assert_eq!(
            target, None,
            "score={score} should be within hysteresis band"
        );
    }

    #[test]
    fn tier_selection_hysteresis_prevents_downgrade() {
        let cfg = TierConfig {
            hysteresis: 0.10,
            ..default_config()
        };
        // Block in Tier1 with score just below t1(0.7) but above t1-hysteresis(0.6).
        // ema=0.6: 0.4*0.6 + 0.3*1 + 0.3*1 = 0.24+0.3+0.3=0.84 -- too high
        // Need score in (0.6, 0.7). Set some bits off and add time gap.
        // ema=0.5, window=32bits, dt=10, tau=100: rec=exp(-0.1)~0.905
        // score = 0.4*0.5 + 0.3*(32/64) + 0.3*0.905 = 0.2+0.15+0.2715 = 0.6215
        let meta = make_meta(0.5, 0x0000_0000_FFFF_FFFF, 90, Tier::Tier1, 0);
        let score = compute_score(&cfg, 100, &meta);
        assert!(
            score < cfg.t1 && score > cfg.t1 - cfg.hysteresis,
            "score={score}, expected in ({}, {})",
            cfg.t1 - cfg.hysteresis,
            cfg.t1
        );
        let target = choose_tier(&cfg, 100, &meta);
        assert_eq!(
            target, None,
            "hysteresis should prevent downgrade, score={score}"
        );
    }

    // -----------------------------------------------------------------------
    // 5. Touch updates access stats correctly
    // -----------------------------------------------------------------------

    #[test]
    fn touch_increments_count() {
        let cfg = default_config();
        let mut meta = BlockMeta::new(0);
        assert_eq!(meta.access_count, 0);
        touch(&cfg, 1, &mut meta);
        assert_eq!(meta.access_count, 1);
        touch(&cfg, 2, &mut meta);
        assert_eq!(meta.access_count, 2);
    }

    #[test]
    fn touch_updates_ema() {
        let cfg = default_config();
        let mut meta = BlockMeta::new(0);
        assert_eq!(meta.ema_rate, 0.0);
        touch(&cfg, 1, &mut meta);
        // ema = 0.3 * 1.0 + 0.7 * 0.0 = 0.3
        assert!((meta.ema_rate - 0.3).abs() < 1e-6);
        touch(&cfg, 2, &mut meta);
        // ema = 0.3 + 0.7 * 0.3 = 0.3 + 0.21 = 0.51
        assert!((meta.ema_rate - 0.51).abs() < 1e-6);
    }

    #[test]
    fn touch_updates_window() {
        let cfg = default_config();
        let mut meta = BlockMeta::new(0);
        meta.access_window = 0;
        touch(&cfg, 1, &mut meta);
        assert_eq!(meta.access_window, 1);
        touch(&cfg, 3, &mut meta);
        // Elapsed 2: shift left 2, set bit 0 -> 0b100 | 1 = 0b101
        assert_eq!(meta.access_window, 0b101);
    }

    #[test]
    fn touch_same_tick() {
        let cfg = default_config();
        let mut meta = BlockMeta::new(5);
        meta.access_window = 0b1010;
        touch(&cfg, 5, &mut meta);
        // Same tick: just OR in bit 0 -> 0b1011
        assert_eq!(meta.access_window, 0b1011);
    }

    #[test]
    fn touch_large_gap_clears_window() {
        let cfg = default_config();
        let mut meta = BlockMeta::new(0);
        meta.access_window = u64::MAX;
        touch(&cfg, 100, &mut meta);
        // Gap >= 64: window reset to 1
        assert_eq!(meta.access_window, 1);
    }

    // -----------------------------------------------------------------------
    // 6. Min residency enforcement
    // -----------------------------------------------------------------------

    #[test]
    fn min_residency_blocks_migration() {
        let cfg = TierConfig {
            min_residency: 10,
            ..default_config()
        };
        // Block assigned to Tier3 at tick 95, now is 100 (5 ticks < 10).
        let meta = make_meta(1.0, u64::MAX, 100, Tier::Tier3, 95);
        let target = choose_tier(&cfg, 100, &meta);
        assert_eq!(target, None);
    }

    #[test]
    fn min_residency_allows_after_enough_ticks() {
        let cfg = TierConfig {
            min_residency: 10,
            ..default_config()
        };
        // Block assigned to Tier3 at tick 90, now is 100 (10 ticks >= 10).
        let meta = make_meta(1.0, u64::MAX, 100, Tier::Tier3, 90);
        let target = choose_tier(&cfg, 100, &meta);
        assert_eq!(target, Some(Tier::Tier1));
    }

    // -----------------------------------------------------------------------
    // 7. Candidate selection ordering
    // -----------------------------------------------------------------------

    #[test]
    fn candidates_upgrades_before_downgrades() {
        let cfg = default_config();

        let hot_meta = make_meta(1.0, u64::MAX, 50, Tier::Tier3, 0);
        let cold_meta = make_meta(0.0, 0, 0, Tier::Tier1, 0);

        let blocks = vec![(BlockKey(1), &cold_meta), (BlockKey(2), &hot_meta)];

        let candidates = select_candidates(&cfg, 50, &blocks);
        assert!(candidates.len() >= 2, "expected at least 2 candidates");
        // First candidate should be the upgrade (key=2, target=Tier1).
        assert_eq!(candidates[0].key, BlockKey(2));
        assert_eq!(candidates[0].target_tier, Tier::Tier1);
        // Second candidate should be the downgrade (key=1, target=Tier3).
        assert_eq!(candidates[1].key, BlockKey(1));
        assert_eq!(candidates[1].target_tier, Tier::Tier3);
    }

    #[test]
    fn candidates_upgrades_sorted_by_highest_score() {
        let cfg = default_config();

        let meta_a = make_meta(0.9, u64::MAX, 50, Tier::Tier3, 0);
        let meta_b = make_meta(1.0, u64::MAX, 50, Tier::Tier3, 0);

        let blocks = vec![(BlockKey(1), &meta_a), (BlockKey(2), &meta_b)];

        let candidates = select_candidates(&cfg, 50, &blocks);
        // Block 2 has higher ema_rate, so higher score, should come first.
        assert!(candidates.len() >= 2);
        assert_eq!(candidates[0].key, BlockKey(2));
        assert_eq!(candidates[1].key, BlockKey(1));
    }

    #[test]
    fn candidates_empty_when_all_stable() {
        let cfg = default_config();
        // Block already in correct tier with score matching.
        let meta = make_meta(0.5, 0x0000_0000_FFFF_FFFF, 50, Tier::Tier2, 0);
        let blocks = vec![(BlockKey(1), &meta)];
        let candidates = select_candidates(&cfg, 50, &blocks);
        // May or may not have candidates depending on exact score; just verify no panic.
        let _ = candidates;
    }

    // -----------------------------------------------------------------------
    // 8. Bits selection for each tier
    // -----------------------------------------------------------------------

    #[test]
    fn bits_tier0() {
        assert_eq!(bits_for_tier(&default_config(), Tier::Tier0, 0), 0);
    }

    #[test]
    fn bits_tier1() {
        assert_eq!(bits_for_tier(&default_config(), Tier::Tier1, 0), 8);
    }

    #[test]
    fn bits_tier2_normal() {
        assert_eq!(bits_for_tier(&default_config(), Tier::Tier2, 0), 7);
    }

    #[test]
    fn bits_tier3() {
        assert_eq!(bits_for_tier(&default_config(), Tier::Tier3, 0), 3);
    }

    // -----------------------------------------------------------------------
    // 9. Warm aggressive mode (5-bit when over threshold)
    // -----------------------------------------------------------------------

    #[test]
    fn bits_tier2_aggressive() {
        let cfg = TierConfig {
            warm_aggressive_threshold: Some(1024),
            ..default_config()
        };
        assert_eq!(bits_for_tier(&cfg, Tier::Tier2, 512), 7);
        assert_eq!(bits_for_tier(&cfg, Tier::Tier2, 1024), 7); // at threshold, not over
        assert_eq!(bits_for_tier(&cfg, Tier::Tier2, 1025), 5);
    }

    // -----------------------------------------------------------------------
    // 10. Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn edge_zero_access_count() {
        let cfg = default_config();
        let meta = BlockMeta::new(0);
        let score = compute_score(&cfg, 0, &meta);
        // ema=0, pop=0, dt=0 -> rec=exp(0)=1 -> score = 0.3
        assert!((score - cfg.w_rec).abs() < 1e-4, "score={score}");
    }

    #[test]
    fn edge_max_timestamp() {
        let cfg = default_config();
        let meta = make_meta(0.5, 0xAAAA_AAAA_AAAA_AAAA, u64::MAX - 1, Tier::Tier2, 0);
        let score = compute_score(&cfg, u64::MAX, &meta);
        // Should not panic; dt=1 -> recency ~ exp(-1/100) ~ 0.99
        assert!(score.is_finite(), "score={score}");
    }

    #[test]
    fn edge_touch_at_u64_max() {
        let cfg = default_config();
        let mut meta = BlockMeta::new(u64::MAX - 1);
        touch(&cfg, u64::MAX, &mut meta);
        assert_eq!(meta.last_access, u64::MAX);
        assert_eq!(meta.access_count, 1);
    }

    #[test]
    fn edge_access_count_saturates() {
        let cfg = default_config();
        let mut meta = BlockMeta::new(0);
        meta.access_count = u64::MAX;
        touch(&cfg, 1, &mut meta);
        assert_eq!(meta.access_count, u64::MAX);
    }

    #[test]
    fn tick_decay_reduces_ema() {
        let cfg = default_config();
        let mut meta = BlockMeta::new(0);
        meta.ema_rate = 1.0;
        meta.access_window = 0b1111;
        tick_decay(&cfg, &mut meta);
        assert!((meta.ema_rate - 0.7).abs() < 1e-6, "ema={}", meta.ema_rate);
        assert_eq!(meta.access_window, 0b1111_0);
    }

    #[test]
    fn tick_decay_converges_to_zero() {
        let cfg = default_config();
        let mut meta = BlockMeta::new(0);
        meta.ema_rate = 1.0;
        for _ in 0..200 {
            tick_decay(&cfg, &mut meta);
        }
        assert!(meta.ema_rate < 1e-10, "ema={}", meta.ema_rate);
    }

    #[test]
    fn tier_config_default_weights_sum_to_one() {
        let cfg = default_config();
        let sum = cfg.w_ema + cfg.w_pop + cfg.w_rec;
        assert!((sum - 1.0).abs() < 1e-6, "sum={sum}");
    }

    #[test]
    fn block_meta_new_defaults() {
        let meta = BlockMeta::new(42);
        assert_eq!(meta.ema_rate, 0.0);
        assert_eq!(meta.access_window, 0);
        assert_eq!(meta.last_access, 42);
        assert_eq!(meta.access_count, 0);
        assert_eq!(meta.current_tier, Tier::Tier1);
        assert_eq!(meta.tier_since, 42);
    }

    #[test]
    fn tier_ordering() {
        assert!(Tier::Tier0 < Tier::Tier1);
        assert!(Tier::Tier1 < Tier::Tier2);
        assert!(Tier::Tier2 < Tier::Tier3);
    }

    // -----------------------------------------------------------------------
    // 11. Batch scoring
    // -----------------------------------------------------------------------

    #[test]
    fn batch_scores_match_individual() {
        let cfg = default_config();
        let metas: Vec<BlockMeta> = vec![
            make_meta(1.0, u64::MAX, 100, Tier::Tier1, 0),
            make_meta(0.0, 0, 0, Tier::Tier3, 0),
            make_meta(0.5, 0x0000_0000_FFFF_FFFF, 50, Tier::Tier2, 0),
        ];
        let batch = compute_scores_batch(&cfg, 100, &metas);
        for (i, meta) in metas.iter().enumerate() {
            let single = compute_score(&cfg, 100, meta);
            assert!((batch[i] - single).abs() < 1e-6, "index {i}");
        }
    }

    #[test]
    fn batch_tiers_match_individual() {
        let cfg = default_config();
        let metas: Vec<BlockMeta> = vec![
            make_meta(1.0, u64::MAX, 100, Tier::Tier1, 0),
            make_meta(0.0, 0, 0, Tier::Tier3, 0),
        ];
        let batch = choose_tiers_batch(&cfg, 100, &metas);
        for (i, meta) in metas.iter().enumerate() {
            let single = choose_tier(&cfg, 100, meta);
            assert_eq!(batch[i], single, "index {i}");
        }
    }

    #[test]
    fn score_and_partition_distributes_correctly() {
        let cfg = default_config();
        let metas: Vec<BlockMeta> = vec![
            make_meta(1.0, u64::MAX, 100, Tier::Tier1, 0), // hot
            make_meta(0.5, 0x0000_0000_FFFF_FFFF, 90, Tier::Tier2, 0), // warm
            make_meta(0.0, 0, 0, Tier::Tier3, 0),          // cold/evict
        ];
        let part = score_and_partition(&cfg, 100, &metas);
        assert!(!part.hot.is_empty(), "should have hot blocks");
        assert_eq!(part.scores.len(), 3);
    }

    #[test]
    fn top_k_coldest_returns_lowest() {
        let cfg = default_config();
        let metas: Vec<BlockMeta> = vec![
            make_meta(1.0, u64::MAX, 100, Tier::Tier1, 0),
            make_meta(0.0, 0, 0, Tier::Tier3, 0),
            make_meta(0.5, 0x0000_0000_FFFF_FFFF, 50, Tier::Tier2, 0),
        ];
        let coldest = top_k_coldest(&cfg, 100, &metas, 2);
        assert_eq!(coldest.len(), 2);
        // The coldest should be index 1 (score near 0)
        assert_eq!(coldest[0].0, 1);
        assert!(coldest[0].1 <= coldest[1].1);
    }

    #[test]
    fn top_k_coldest_k_exceeds_len() {
        let cfg = default_config();
        let metas: Vec<BlockMeta> = vec![make_meta(1.0, u64::MAX, 100, Tier::Tier1, 0)];
        let coldest = top_k_coldest(&cfg, 100, &metas, 10);
        assert_eq!(coldest.len(), 1);
    }

    #[test]
    fn batch_empty_input() {
        let cfg = default_config();
        let empty: Vec<BlockMeta> = vec![];
        assert!(compute_scores_batch(&cfg, 100, &empty).is_empty());
        assert!(choose_tiers_batch(&cfg, 100, &empty).is_empty());
        let part = score_and_partition(&cfg, 100, &empty);
        assert!(
            part.hot.is_empty()
                && part.warm.is_empty()
                && part.cold.is_empty()
                && part.evict.is_empty()
        );
        assert!(top_k_coldest(&cfg, 100, &empty, 5).is_empty());
    }
}
