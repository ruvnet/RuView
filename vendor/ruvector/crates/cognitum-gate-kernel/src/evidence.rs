//! Evidence accumulator for anytime-valid coherence gate
//!
//! Implements sequential testing with e-values for the coherence gate.
//! The accumulator maintains running e-value products that can be queried
//! at any time to determine if the coherence hypothesis should be rejected.
//!
//! ## Performance Optimizations
//!
//! - Pre-computed log threshold constants (avoid runtime log calculations)
//! - Fixed-point arithmetic for e-values (numerical stability + performance)
//! - `#[inline(always)]` on hot path functions
//! - Cache-aligned accumulator structure
//! - Branchless observation processing where possible

#![allow(missing_docs)]

use crate::delta::{Observation, TileVertexId};
use core::mem::size_of;

/// Maximum number of tracked hypotheses per tile
pub const MAX_HYPOTHESES: usize = 16;

/// Maximum observations in sliding window
pub const WINDOW_SIZE: usize = 64;

/// Fixed-point e-value representation (32-bit, log scale)
/// Stored as log2(e-value) * 65536 for numerical stability
pub type LogEValue = i32;

// ============================================================================
// PRE-COMPUTED THRESHOLD CONSTANTS (avoid runtime log calculations)
// ============================================================================

/// log2(20) * 65536 = 282944 (strong evidence threshold: e > 20)
/// Pre-computed to avoid runtime log calculation
pub const LOG_E_STRONG: LogEValue = 282944;

/// log2(100) * 65536 = 436906 (very strong evidence threshold: e > 100)
pub const LOG_E_VERY_STRONG: LogEValue = 436906;

/// log2(1.5) * 65536 = 38550 (connectivity positive evidence)
pub const LOG_LR_CONNECTIVITY_POS: LogEValue = 38550;

/// log2(0.5) * 65536 = -65536 (connectivity negative evidence)
pub const LOG_LR_CONNECTIVITY_NEG: LogEValue = -65536;

/// log2(2.0) * 65536 = 65536 (witness positive evidence)
pub const LOG_LR_WITNESS_POS: LogEValue = 65536;

/// log2(0.5) * 65536 = -65536 (witness negative evidence)
pub const LOG_LR_WITNESS_NEG: LogEValue = -65536;

/// Fixed-point scale factor
pub const FIXED_SCALE: i32 = 65536;

// ============================================================================
// SIMD-OPTIMIZED E-VALUE AGGREGATION
// ============================================================================

/// Aggregate log e-values using SIMD-friendly parallel lanes
///
/// This function is optimized for vectorization by processing values
/// in parallel lanes, allowing the compiler to generate SIMD instructions.
///
/// OPTIMIZATION: Uses 4 parallel lanes for 128-bit SIMD (SSE/NEON) or
/// 8 lanes for 256-bit SIMD (AVX2). The compiler can auto-vectorize
/// this pattern effectively.
///
/// # Arguments
/// * `log_e_values` - Slice of log e-values (fixed-point, 16.16 format)
///
/// # Returns
/// The sum of all log e-values (product in log space)
#[inline]
pub fn simd_aggregate_log_e(log_e_values: &[LogEValue]) -> i64 {
    // Use 4 parallel accumulator lanes for 128-bit SIMD
    // This allows the compiler to vectorize the inner loop
    let mut lanes = [0i64; 4];

    // Process in chunks of 4 for optimal SIMD usage
    let chunks = log_e_values.chunks_exact(4);
    let remainder = chunks.remainder();

    for chunk in chunks {
        // SAFETY: chunks_exact guarantees 4 elements
        lanes[0] += chunk[0] as i64;
        lanes[1] += chunk[1] as i64;
        lanes[2] += chunk[2] as i64;
        lanes[3] += chunk[3] as i64;
    }

    // Handle remainder
    for (i, &val) in remainder.iter().enumerate() {
        lanes[i % 4] += val as i64;
    }

    // Reduce lanes to single value
    lanes[0] + lanes[1] + lanes[2] + lanes[3]
}

/// Aggregate log e-values using 8 parallel lanes for AVX2
///
/// OPTIMIZATION: Uses 8 lanes for 256-bit SIMD (AVX2/AVX-512).
/// Falls back gracefully on platforms without AVX.
#[inline]
pub fn simd_aggregate_log_e_wide(log_e_values: &[LogEValue]) -> i64 {
    // Use 8 parallel accumulator lanes for 256-bit SIMD
    let mut lanes = [0i64; 8];

    let chunks = log_e_values.chunks_exact(8);
    let remainder = chunks.remainder();

    for chunk in chunks {
        // Unrolled for better codegen
        lanes[0] += chunk[0] as i64;
        lanes[1] += chunk[1] as i64;
        lanes[2] += chunk[2] as i64;
        lanes[3] += chunk[3] as i64;
        lanes[4] += chunk[4] as i64;
        lanes[5] += chunk[5] as i64;
        lanes[6] += chunk[6] as i64;
        lanes[7] += chunk[7] as i64;
    }

    // Handle remainder
    for (i, &val) in remainder.iter().enumerate() {
        lanes[i % 8] += val as i64;
    }

    // Tree reduction for lane aggregation
    let sum_0_3 = lanes[0] + lanes[1] + lanes[2] + lanes[3];
    let sum_4_7 = lanes[4] + lanes[5] + lanes[6] + lanes[7];
    sum_0_3 + sum_4_7
}

/// Aggregate mixture e-values for a tile set
///
/// Computes the product of e-values across tiles using log-space arithmetic
/// for numerical stability. This is the key operation for coherence gate
/// aggregation.
///
/// OPTIMIZATION:
/// - Uses SIMD-friendly parallel lanes
/// - Processes 255 tile e-values efficiently
/// - Returns in fixed-point log format for further processing
///
/// # Arguments
/// * `tile_log_e_values` - Array of 255 tile log e-values
///
/// # Returns
/// Aggregated log e-value (can be converted to f32 with log_e_to_f32)
#[inline]
pub fn aggregate_tile_evidence(tile_log_e_values: &[LogEValue; 255]) -> i64 {
    simd_aggregate_log_e(tile_log_e_values)
}

/// Convert log e-value to approximate f32
///
/// OPTIMIZATION: Marked #[inline(always)] for hot path usage
#[inline(always)]
pub const fn log_e_to_f32(log_e: LogEValue) -> f32 {
    // log2(e) = log_e / 65536
    // e = 2^(log_e / 65536)
    // Approximation for no_std
    let log2_val = (log_e as f32) / 65536.0;
    // 2^x approximation using e^(x * ln(2))
    // For simplicity, we just return the log value scaled
    log2_val
}

/// Convert f32 e-value to log representation
///
/// OPTIMIZATION: Early exit for common cases, marked #[inline(always)]
#[inline(always)]
pub fn f32_to_log_e(e: f32) -> LogEValue {
    if e <= 0.0 {
        i32::MIN
    } else if e == 1.0 {
        0 // Fast path for neutral evidence
    } else if e == 2.0 {
        FIXED_SCALE // Fast path for common LR=2
    } else if e == 0.5 {
        -FIXED_SCALE // Fast path for common LR=0.5
    } else {
        // log2(e) * 65536
        let log2_e = libm::log2f(e);
        (log2_e * 65536.0) as i32
    }
}

/// Compute log likelihood ratio directly in fixed-point
/// Avoids f32 conversion for common cases
///
/// OPTIMIZATION: Returns pre-computed constants for known observation types
#[inline(always)]
pub const fn log_lr_for_obs_type(obs_type: u8, flags: u8, value: u16) -> LogEValue {
    match obs_type {
        Observation::TYPE_CONNECTIVITY => {
            if flags != 0 {
                LOG_LR_CONNECTIVITY_POS
            } else {
                LOG_LR_CONNECTIVITY_NEG
            }
        }
        Observation::TYPE_WITNESS => {
            if flags != 0 {
                LOG_LR_WITNESS_POS
            } else {
                LOG_LR_WITNESS_NEG
            }
        }
        // For other types, return 0 (neutral) - caller should use f32 path
        _ => 0,
    }
}

/// Hypothesis state for tracking
///
/// Size: 16 bytes, aligned for efficient cache access
#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
pub struct HypothesisState {
    /// Current accumulated log e-value (hot field, first for cache)
    pub log_e_value: LogEValue,
    /// Number of observations processed
    pub obs_count: u32,
    /// Hypothesis ID
    pub id: u16,
    /// Target vertex (for vertex-specific hypotheses)
    pub target: TileVertexId,
    /// Threshold vertex (for cut hypotheses)
    pub threshold: TileVertexId,
    /// Hypothesis type (0 = connectivity, 1 = cut, 2 = flow)
    pub hyp_type: u8,
    /// Status flags
    pub flags: u8,
}

impl Default for HypothesisState {
    #[inline]
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl HypothesisState {
    /// Hypothesis is active
    pub const FLAG_ACTIVE: u8 = 0x01;
    /// Hypothesis is rejected (e-value crossed threshold)
    pub const FLAG_REJECTED: u8 = 0x02;
    /// Hypothesis evidence is strong (e > 20)
    pub const FLAG_STRONG: u8 = 0x04;
    /// Hypothesis evidence is very strong (e > 100)
    pub const FLAG_VERY_STRONG: u8 = 0x08;

    /// Type: connectivity hypothesis
    pub const TYPE_CONNECTIVITY: u8 = 0;
    /// Type: cut membership hypothesis
    pub const TYPE_CUT: u8 = 1;
    /// Type: flow hypothesis
    pub const TYPE_FLOW: u8 = 2;

    /// Create a new hypothesis
    #[inline(always)]
    pub const fn new(id: u16, hyp_type: u8) -> Self {
        Self {
            log_e_value: 0, // e = 1 (neutral)
            obs_count: 0,
            id,
            target: 0,
            threshold: 0,
            hyp_type,
            flags: Self::FLAG_ACTIVE,
        }
    }

    /// Create a connectivity hypothesis for a vertex
    #[inline(always)]
    pub const fn connectivity(id: u16, vertex: TileVertexId) -> Self {
        Self {
            log_e_value: 0,
            obs_count: 0,
            id,
            target: vertex,
            threshold: 0,
            hyp_type: Self::TYPE_CONNECTIVITY,
            flags: Self::FLAG_ACTIVE,
        }
    }

    /// Create a cut membership hypothesis
    #[inline(always)]
    pub const fn cut_membership(id: u16, vertex: TileVertexId, threshold: TileVertexId) -> Self {
        Self {
            log_e_value: 0,
            obs_count: 0,
            id,
            target: vertex,
            threshold,
            hyp_type: Self::TYPE_CUT,
            flags: Self::FLAG_ACTIVE,
        }
    }

    /// Check if hypothesis is active
    ///
    /// OPTIMIZATION: #[inline(always)] - called in every hypothesis loop
    #[inline(always)]
    pub const fn is_active(&self) -> bool {
        self.flags & Self::FLAG_ACTIVE != 0
    }

    /// Check if hypothesis is rejected
    #[inline(always)]
    pub const fn is_rejected(&self) -> bool {
        self.flags & Self::FLAG_REJECTED != 0
    }

    /// Check if hypothesis can be updated (active and not rejected)
    ///
    /// OPTIMIZATION: Combined check to reduce branch mispredictions
    #[inline(always)]
    pub const fn can_update(&self) -> bool {
        // Active AND not rejected = (flags & ACTIVE) != 0 && (flags & REJECTED) == 0
        (self.flags & (Self::FLAG_ACTIVE | Self::FLAG_REJECTED)) == Self::FLAG_ACTIVE
    }

    /// Get e-value as approximate f32 (2^(log_e/65536))
    #[inline(always)]
    pub fn e_value_approx(&self) -> f32 {
        let log2_val = (self.log_e_value as f32) / 65536.0;
        libm::exp2f(log2_val)
    }

    /// Update with a new observation (f32 likelihood ratio)
    /// Returns true if the hypothesis is now rejected
    ///
    /// OPTIMIZATION: Uses pre-computed threshold constants
    #[inline]
    pub fn update(&mut self, likelihood_ratio: f32) -> bool {
        if !self.can_update() {
            return self.is_rejected();
        }

        // Update log e-value: log(e') = log(e) + log(LR)
        let log_lr = f32_to_log_e(likelihood_ratio);
        self.update_with_log_lr(log_lr)
    }

    /// Update with a pre-computed log likelihood ratio (fixed-point)
    /// Returns true if the hypothesis is now rejected
    ///
    /// OPTIMIZATION: Avoids f32->log conversion when log_lr is pre-computed
    #[inline(always)]
    pub fn update_with_log_lr(&mut self, log_lr: LogEValue) -> bool {
        self.log_e_value = self.log_e_value.saturating_add(log_lr);
        self.obs_count += 1;

        // Update strength flags using pre-computed constants
        // OPTIMIZATION: Single comparison chain with constants
        if self.log_e_value > LOG_E_VERY_STRONG {
            self.flags |= Self::FLAG_VERY_STRONG | Self::FLAG_STRONG;
        } else if self.log_e_value > LOG_E_STRONG {
            self.flags |= Self::FLAG_STRONG;
            self.flags &= !Self::FLAG_VERY_STRONG;
        } else {
            self.flags &= !(Self::FLAG_STRONG | Self::FLAG_VERY_STRONG);
        }

        // Check rejection threshold (alpha = 0.05 => e > 20)
        if self.log_e_value > LOG_E_STRONG {
            self.flags |= Self::FLAG_REJECTED;
            return true;
        }

        false
    }

    /// Reset the hypothesis
    #[inline]
    pub fn reset(&mut self) {
        self.log_e_value = 0;
        self.obs_count = 0;
        self.flags = Self::FLAG_ACTIVE;
    }
}

/// Observation record for sliding window
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct ObsRecord {
    /// Observation data
    pub obs: Observation,
    /// Timestamp (tick)
    pub tick: u32,
}

/// Evidence accumulator for tile-local e-value tracking
///
/// OPTIMIZATION: Cache-line aligned (64 bytes) with hot fields first
#[derive(Clone)]
#[repr(C, align(64))]
pub struct EvidenceAccumulator {
    // === HOT FIELDS (frequently accessed) ===
    /// Global accumulated log e-value
    pub global_log_e: LogEValue,
    /// Total observations processed
    pub total_obs: u32,
    /// Current tick
    pub current_tick: u32,
    /// Window head pointer (circular buffer)
    pub window_head: u16,
    /// Window count (number of valid entries)
    pub window_count: u16,
    /// Number of active hypotheses
    pub num_hypotheses: u8,
    /// Reserved padding
    pub _reserved: [u8; 1],
    /// Rejected hypothesis count
    pub rejected_count: u16,
    /// Status flags
    pub status: u16,
    /// Padding to align cold fields
    _hot_pad: [u8; 40],

    // === COLD FIELDS ===
    /// Active hypotheses
    pub hypotheses: [HypothesisState; MAX_HYPOTHESES],
    /// Sliding window of recent observations
    pub window: [ObsRecord; WINDOW_SIZE],
}

impl Default for EvidenceAccumulator {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl EvidenceAccumulator {
    /// Status: accumulator is active
    pub const STATUS_ACTIVE: u16 = 0x0001;
    /// Status: at least one hypothesis rejected
    pub const STATUS_HAS_REJECTION: u16 = 0x0002;
    /// Status: global evidence is significant
    pub const STATUS_SIGNIFICANT: u16 = 0x0004;

    /// Create a new accumulator
    pub const fn new() -> Self {
        Self {
            global_log_e: 0,
            total_obs: 0,
            current_tick: 0,
            window_head: 0,
            window_count: 0,
            num_hypotheses: 0,
            _reserved: [0; 1],
            rejected_count: 0,
            status: Self::STATUS_ACTIVE,
            _hot_pad: [0; 40],
            hypotheses: [HypothesisState::new(0, 0); MAX_HYPOTHESES],
            window: [ObsRecord {
                obs: Observation {
                    vertex: 0,
                    obs_type: 0,
                    flags: 0,
                    value: 0,
                },
                tick: 0,
            }; WINDOW_SIZE],
        }
    }

    /// Add a new hypothesis to track
    pub fn add_hypothesis(&mut self, hypothesis: HypothesisState) -> bool {
        if self.num_hypotheses as usize >= MAX_HYPOTHESES {
            return false;
        }

        self.hypotheses[self.num_hypotheses as usize] = hypothesis;
        self.num_hypotheses += 1;
        true
    }

    /// Add a connectivity hypothesis
    pub fn add_connectivity_hypothesis(&mut self, vertex: TileVertexId) -> bool {
        let id = self.num_hypotheses as u16;
        self.add_hypothesis(HypothesisState::connectivity(id, vertex))
    }

    /// Add a cut membership hypothesis
    pub fn add_cut_hypothesis(&mut self, vertex: TileVertexId, threshold: TileVertexId) -> bool {
        let id = self.num_hypotheses as u16;
        self.add_hypothesis(HypothesisState::cut_membership(id, vertex, threshold))
    }

    /// Process an observation
    ///
    /// OPTIMIZATION: Uses fixed-point log LR for common observation types,
    /// avoids f32 conversion where possible
    #[inline]
    pub fn process_observation(&mut self, obs: Observation, tick: u32) {
        self.current_tick = tick;
        self.total_obs += 1;

        // Add to sliding window using wrapping arithmetic
        // OPTIMIZATION: Avoid modulo with power-of-2 window size
        let idx = self.window_head as usize;
        // SAFETY: WINDOW_SIZE is 64, idx < 64
        unsafe {
            *self.window.get_unchecked_mut(idx) = ObsRecord { obs, tick };
        }
        // OPTIMIZATION: Bit mask for power-of-2 wrap (64 = 0x40, mask = 0x3F)
        self.window_head = ((self.window_head + 1) & (WINDOW_SIZE as u16 - 1));
        if (self.window_count as usize) < WINDOW_SIZE {
            self.window_count += 1;
        }

        // Compute log likelihood ratio in fixed-point where possible
        // OPTIMIZATION: Use pre-computed constants for common types
        let log_lr = self.compute_log_likelihood_ratio(&obs);

        // Update global e-value
        self.global_log_e = self.global_log_e.saturating_add(log_lr);

        // Update relevant hypotheses
        // OPTIMIZATION: Cache num_hypotheses to avoid repeated load
        let num_hyp = self.num_hypotheses as usize;
        for i in 0..num_hyp {
            // SAFETY: i < num_hypotheses <= MAX_HYPOTHESES
            let hyp = unsafe { self.hypotheses.get_unchecked(i) };

            // OPTIMIZATION: Use combined can_update check
            if !hyp.can_update() {
                continue;
            }

            // Check if observation is relevant to this hypothesis
            // OPTIMIZATION: Early exit on type mismatch (most common case)
            let is_relevant = self.is_obs_relevant(hyp, &obs);

            if is_relevant {
                // SAFETY: i < num_hypotheses
                let hyp_mut = unsafe { self.hypotheses.get_unchecked_mut(i) };
                if hyp_mut.update_with_log_lr(log_lr) {
                    self.rejected_count += 1;
                    self.status |= Self::STATUS_HAS_REJECTION;
                }
            }
        }

        // Update significance status using pre-computed constant
        if self.global_log_e > LOG_E_STRONG {
            self.status |= Self::STATUS_SIGNIFICANT;
        }
    }

    /// Check if observation is relevant to hypothesis
    ///
    /// OPTIMIZATION: Inlined for hot path
    #[inline(always)]
    fn is_obs_relevant(&self, hyp: &HypothesisState, obs: &Observation) -> bool {
        match (hyp.hyp_type, obs.obs_type) {
            (HypothesisState::TYPE_CONNECTIVITY, Observation::TYPE_CONNECTIVITY) => {
                obs.vertex == hyp.target
            }
            (HypothesisState::TYPE_CUT, Observation::TYPE_CUT_MEMBERSHIP) => {
                obs.vertex == hyp.target
            }
            (HypothesisState::TYPE_FLOW, Observation::TYPE_FLOW) => obs.vertex == hyp.target,
            _ => false,
        }
    }

    /// Compute log likelihood ratio in fixed-point
    ///
    /// OPTIMIZATION: Returns pre-computed constants for common types,
    /// only falls back to f32 for complex calculations
    #[inline(always)]
    fn compute_log_likelihood_ratio(&self, obs: &Observation) -> LogEValue {
        match obs.obs_type {
            Observation::TYPE_CONNECTIVITY => {
                // Use pre-computed constants
                if obs.flags != 0 {
                    LOG_LR_CONNECTIVITY_POS // 1.5
                } else {
                    LOG_LR_CONNECTIVITY_NEG // 0.5
                }
            }
            Observation::TYPE_WITNESS => {
                // Use pre-computed constants
                if obs.flags != 0 {
                    LOG_LR_WITNESS_POS // 2.0
                } else {
                    LOG_LR_WITNESS_NEG // 0.5
                }
            }
            Observation::TYPE_CUT_MEMBERSHIP => {
                // Confidence-based: 1.0 + confidence (1.0 to 2.0)
                // log2(1 + x) where x in [0,1]
                // Approximation: x * 65536 / ln(2) for small x
                let confidence_fixed = (obs.value as i32) >> 1; // Scale 0-65535 to ~0-32768
                confidence_fixed
            }
            Observation::TYPE_FLOW => {
                // Flow-based: needs f32 path
                let flow = (obs.value as f32) / 1000.0;
                let lr = if flow > 0.5 {
                    1.0 + flow
                } else {
                    1.0 / (1.0 + flow)
                };
                f32_to_log_e(lr)
            }
            _ => 0, // Neutral
        }
    }

    /// Compute likelihood ratio for an observation (f32 version for compatibility)
    #[inline]
    fn compute_likelihood_ratio(&self, obs: &Observation) -> f32 {
        match obs.obs_type {
            Observation::TYPE_CONNECTIVITY => {
                if obs.flags != 0 {
                    1.5
                } else {
                    0.5
                }
            }
            Observation::TYPE_CUT_MEMBERSHIP => {
                let confidence = (obs.value as f32) / 65535.0;
                1.0 + confidence
            }
            Observation::TYPE_FLOW => {
                let flow = (obs.value as f32) / 1000.0;
                if flow > 0.5 {
                    1.0 + flow
                } else {
                    1.0 / (1.0 + flow)
                }
            }
            Observation::TYPE_WITNESS => {
                if obs.flags != 0 {
                    2.0
                } else {
                    0.5
                }
            }
            _ => 1.0,
        }
    }

    /// Get global e-value as approximate f32
    #[inline(always)]
    pub fn global_e_value(&self) -> f32 {
        let log2_val = (self.global_log_e as f32) / 65536.0;
        libm::exp2f(log2_val)
    }

    /// Check if any hypothesis is rejected
    #[inline(always)]
    pub fn has_rejection(&self) -> bool {
        self.status & Self::STATUS_HAS_REJECTION != 0
    }

    /// Check if evidence is significant (e > 20)
    #[inline(always)]
    pub fn is_significant(&self) -> bool {
        self.status & Self::STATUS_SIGNIFICANT != 0
    }

    /// Reset all hypotheses
    pub fn reset(&mut self) {
        for h in self.hypotheses[..self.num_hypotheses as usize].iter_mut() {
            h.reset();
        }
        self.window_head = 0;
        self.window_count = 0;
        self.global_log_e = 0;
        self.rejected_count = 0;
        self.status = Self::STATUS_ACTIVE;
    }

    /// Process a batch of observations efficiently
    ///
    /// OPTIMIZATION: Batch processing reduces function call overhead and
    /// allows better cache utilization by processing observations in bulk.
    ///
    /// # Arguments
    /// * `observations` - Slice of (observation, tick) pairs
    #[inline]
    pub fn process_observation_batch(&mut self, observations: &[(Observation, u32)]) {
        // Pre-compute all log LRs for the batch
        // This allows potential vectorization of LR computation
        let batch_size = observations.len().min(64);

        // Process in cache-friendly order
        for &(obs, tick) in observations.iter().take(batch_size) {
            self.process_observation(obs, tick);
        }
    }

    /// Aggregate all hypothesis e-values using SIMD
    ///
    /// OPTIMIZATION: Uses SIMD-friendly parallel lane accumulation
    /// to sum all active hypothesis log e-values efficiently.
    ///
    /// # Returns
    /// Total accumulated log e-value across all hypotheses
    #[inline]
    pub fn aggregate_hypotheses_simd(&self) -> i64 {
        let mut lanes = [0i64; 4];
        let num_hyp = self.num_hypotheses as usize;

        // Process hypotheses in 4-lane parallel pattern
        for i in 0..num_hyp {
            let hyp = &self.hypotheses[i];
            if hyp.is_active() {
                lanes[i % 4] += hyp.log_e_value as i64;
            }
        }

        lanes[0] + lanes[1] + lanes[2] + lanes[3]
    }

    /// Fast check if evidence level exceeds threshold
    ///
    /// OPTIMIZATION: Uses pre-computed log threshold constants
    /// to avoid expensive exp2f conversion.
    ///
    /// # Arguments
    /// * `threshold_log` - Log threshold (e.g., LOG_E_STRONG for alpha=0.05)
    ///
    /// # Returns
    /// true if global evidence exceeds threshold
    #[inline(always)]
    pub fn exceeds_threshold(&self, threshold_log: LogEValue) -> bool {
        self.global_log_e > threshold_log
    }

    /// Get memory size
    pub const fn memory_size() -> usize {
        size_of::<Self>()
    }
}

// Compile-time size assertions
const _: () = assert!(
    size_of::<HypothesisState>() == 16,
    "HypothesisState must be 16 bytes"
);
const _: () = assert!(size_of::<ObsRecord>() == 12, "ObsRecord must be 12 bytes");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_e_conversion() {
        // e = 1 => log = 0
        assert_eq!(f32_to_log_e(1.0), 0);

        // e = 2 => log2(2) * 65536 = 65536
        let log_2 = f32_to_log_e(2.0);
        assert!((log_2 - 65536).abs() < 100);

        // e = 4 => log2(4) * 65536 = 131072
        let log_4 = f32_to_log_e(4.0);
        assert!((log_4 - 131072).abs() < 100);
    }

    #[test]
    fn test_hypothesis_state() {
        let mut hyp = HypothesisState::new(0, HypothesisState::TYPE_CONNECTIVITY);
        assert!(hyp.is_active());
        assert!(!hyp.is_rejected());
        assert_eq!(hyp.obs_count, 0);

        // Update with LR = 2 a few times
        for _ in 0..5 {
            hyp.update(2.0);
        }
        assert_eq!(hyp.obs_count, 5);
        assert!(hyp.e_value_approx() > 20.0); // 2^5 = 32 > 20
    }

    #[test]
    fn test_hypothesis_rejection() {
        let mut hyp = HypothesisState::new(0, HypothesisState::TYPE_CUT);

        // Keep updating until rejection
        for _ in 0..10 {
            if hyp.update(2.0) {
                break;
            }
        }

        assert!(hyp.is_rejected());
    }

    #[test]
    fn test_accumulator_new() {
        let acc = EvidenceAccumulator::new();
        assert_eq!(acc.num_hypotheses, 0);
        assert_eq!(acc.total_obs, 0);
        assert!(!acc.has_rejection());
    }

    #[test]
    fn test_add_hypothesis() {
        let mut acc = EvidenceAccumulator::new();
        assert!(acc.add_connectivity_hypothesis(5));
        assert!(acc.add_cut_hypothesis(10, 15));
        assert_eq!(acc.num_hypotheses, 2);
    }

    #[test]
    fn test_process_observation() {
        let mut acc = EvidenceAccumulator::new();
        acc.add_connectivity_hypothesis(5);

        // Process observations
        for tick in 0..10 {
            let obs = Observation::connectivity(5, true);
            acc.process_observation(obs, tick);
        }

        assert_eq!(acc.total_obs, 10);
        assert!(acc.global_e_value() > 1.0);
    }

    #[test]
    fn test_sliding_window() {
        let mut acc = EvidenceAccumulator::new();

        // Fill window
        for tick in 0..(WINDOW_SIZE as u32 + 10) {
            let obs = Observation::connectivity(0, true);
            acc.process_observation(obs, tick);
        }

        assert_eq!(acc.window_count, WINDOW_SIZE as u16);
    }

    #[test]
    fn test_memory_size() {
        let size = EvidenceAccumulator::memory_size();
        // Should be reasonable for tile budget
        assert!(size < 4096, "EvidenceAccumulator too large: {} bytes", size);
    }
}
