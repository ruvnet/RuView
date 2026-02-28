//! Coherence gate: read-after-write validation for the temporal tensor store.
//!
//! Ensures data integrity by verifying that a `get()` immediately after `put()`
//! returns data within the expected quantization error bounds for the tier.
//!
//! # Overview
//!
//! Quantization is lossy -- the error introduced depends on the tier's bit
//! width (8-bit for Tier1, 7-bit for Tier2, 3-bit for Tier3).  The coherence
//! gate validates that the round-trip error stays within configurable
//! per-tier bounds, catching silent corruption or encoding bugs.
//!
//! # Epoch Tracking
//!
//! [`EpochTracker`] provides a lightweight write-epoch mechanism so that
//! readers can detect stale data (i.e. data that was overwritten between
//! the time it was read and the time it was consumed).

use std::collections::HashMap;

use crate::store::{BlockKey, StoreError, Tier, TieredStore};

// ---------------------------------------------------------------------------
// CoherenceResult
// ---------------------------------------------------------------------------

/// Outcome of a coherence check.
#[derive(Clone, Debug, PartialEq)]
pub struct CoherenceResult {
    /// Maximum relative error observed across all elements.
    pub max_error: f32,
    /// The tier at which the block is stored.
    pub tier: Tier,
    /// Whether the observed error is within the configured bound for this tier.
    pub passed: bool,
}

// ---------------------------------------------------------------------------
// CoherenceCheck
// ---------------------------------------------------------------------------

/// Per-tier maximum relative error bounds for read-after-write validation.
///
/// After a `put()`, the block is immediately read back and the maximum
/// relative error (per-element `|orig - decoded| / |orig|`) is compared
/// against the bound for the block's current tier.
#[derive(Clone, Debug)]
pub struct CoherenceCheck {
    /// Maximum acceptable relative error for each tier, indexed by
    /// `Tier as usize`: `[Tier0, Tier1, Tier2, Tier3]`.
    ///
    /// Tier0 (evicted) has no payload, so any read will fail before the
    /// error comparison is reached.  The bound is set to `f32::MAX` as a
    /// sentinel.
    pub max_relative_errors: [f32; 4],
}

impl Default for CoherenceCheck {
    fn default() -> Self {
        Self {
            // Tier0: evicted, reads always fail (sentinel value).
            // Tier1: 8-bit, very tight bound.
            // Tier2: 7-bit, slightly looser.
            // Tier3: 3-bit, aggressive quantization allows up to 35% error.
            max_relative_errors: [f32::MAX, 0.01, 0.02, 0.35],
        }
    }
}

impl CoherenceCheck {
    /// Create a `CoherenceCheck` with custom per-tier error bounds.
    pub fn new(max_relative_errors: [f32; 4]) -> Self {
        Self {
            max_relative_errors,
        }
    }

    /// Validate read-after-write coherence for a block that was just written.
    ///
    /// Reads the block back from `store`, computes the maximum relative
    /// error against `original_data`, and checks whether it falls within
    /// the configured bound for the block's tier.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::BlockNotFound`] if the key does not exist,
    /// [`StoreError::TensorEvicted`] if the block is in Tier0, or any
    /// other `StoreError` from the underlying read.
    pub fn check_coherence(
        &self,
        store: &mut TieredStore,
        key: BlockKey,
        original_data: &[f32],
        now: u64,
    ) -> Result<CoherenceResult, StoreError> {
        // Look up the tier before reading (needed for the error bound).
        let tier = store.meta(key).ok_or(StoreError::BlockNotFound)?.tier;

        // Read back the block.
        let mut buf = vec![0.0f32; original_data.len()];
        let n = store.get(key, &mut buf, now)?;

        // Compute the maximum relative error.
        let max_error = compute_max_relative_error(original_data, &buf[..n]);

        let tier_idx = tier as usize;
        let bound = if tier_idx < self.max_relative_errors.len() {
            self.max_relative_errors[tier_idx]
        } else {
            f32::MAX
        };

        Ok(CoherenceResult {
            max_error,
            tier,
            passed: max_error <= bound,
        })
    }

    /// Convenience: `put` followed by `check_coherence` in one call.
    ///
    /// Stores the data at the given tier, then immediately reads it back
    /// and validates the round-trip error.  Returns the coherence result
    /// so the caller can decide whether to retry at a higher-fidelity tier.
    ///
    /// # Errors
    ///
    /// Propagates errors from both `put` and the subsequent `get`.
    pub fn verify_put(
        &self,
        store: &mut TieredStore,
        key: BlockKey,
        data: &[f32],
        tier: Tier,
        now: u64,
    ) -> Result<CoherenceResult, StoreError> {
        store.put(key, data, tier, now)?;
        self.check_coherence(store, key, data, now)
    }
}

// ---------------------------------------------------------------------------
// Helper: relative error computation
// ---------------------------------------------------------------------------

/// Compute the maximum element-wise relative error between `original` and
/// `decoded`.
///
/// For elements where `|original| < epsilon` (near-zero), the absolute
/// error is used directly to avoid division-by-zero amplification.
fn compute_max_relative_error(original: &[f32], decoded: &[f32]) -> f32 {
    const EPSILON: f32 = 1e-6;

    let len = original.len().min(decoded.len());
    let mut max_err: f32 = 0.0;

    for i in 0..len {
        let orig = original[i];
        let dec = decoded[i];
        let abs_err = (orig - dec).abs();

        let rel_err = if orig.abs() > EPSILON {
            abs_err / orig.abs()
        } else {
            abs_err
        };

        if rel_err > max_err {
            max_err = rel_err;
        }
    }

    max_err
}

// ---------------------------------------------------------------------------
// EpochTracker
// ---------------------------------------------------------------------------

/// Monotonic write-epoch tracker keyed by [`BlockKey`].
///
/// Each call to [`record_write`](EpochTracker::record_write) increments a
/// global counter and associates the new epoch with the given key.  Readers
/// can later check whether their snapshot is stale via
/// [`is_stale`](EpochTracker::is_stale).
#[derive(Clone, Debug)]
pub struct EpochTracker {
    /// Global monotonically increasing write counter.
    next_epoch: u64,
    /// Per-key latest write epoch.
    epochs: HashMap<BlockKey, u64>,
}

impl EpochTracker {
    /// Create a new tracker with epoch starting at 1.
    pub fn new() -> Self {
        Self {
            next_epoch: 1,
            epochs: HashMap::new(),
        }
    }

    /// Record a write for `key`, returning the new epoch number.
    ///
    /// The epoch is strictly monotonically increasing across all keys.
    pub fn record_write(&mut self, key: BlockKey) -> u64 {
        let epoch = self.next_epoch;
        self.next_epoch += 1;
        self.epochs.insert(key, epoch);
        epoch
    }

    /// Return the latest write epoch for `key`, if any write has been recorded.
    pub fn check_epoch(&self, key: BlockKey) -> Option<u64> {
        self.epochs.get(&key).copied()
    }

    /// Returns `true` if the block identified by `key` has been written
    /// after `read_epoch`, meaning the reader's snapshot is stale.
    ///
    /// Returns `false` if no write has been recorded for `key` (the key
    /// does not exist in the tracker).
    pub fn is_stale(&self, key: BlockKey, read_epoch: u64) -> bool {
        match self.epochs.get(&key) {
            Some(&write_epoch) => write_epoch > read_epoch,
            None => false,
        }
    }
}

impl Default for EpochTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{BlockKey, Tier, TieredStore};

    fn make_key(tid: u128, idx: u32) -> BlockKey {
        BlockKey {
            tensor_id: tid,
            block_index: idx,
        }
    }

    // -- CoherenceCheck -----------------------------------------------------

    #[test]
    fn test_coherence_check_default_bounds() {
        let cc = CoherenceCheck::default();
        assert_eq!(cc.max_relative_errors[0], f32::MAX);
        assert!((cc.max_relative_errors[1] - 0.01).abs() < 1e-9);
        assert!((cc.max_relative_errors[2] - 0.02).abs() < 1e-9);
        assert!((cc.max_relative_errors[3] - 0.35).abs() < 1e-9);
    }

    #[test]
    fn test_coherence_check_custom_bounds() {
        let bounds = [0.0, 0.05, 0.10, 0.50];
        let cc = CoherenceCheck::new(bounds);
        assert_eq!(cc.max_relative_errors, bounds);
    }

    #[test]
    fn test_check_coherence_tier1_passes() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        let data: Vec<f32> = (0..64).map(|i| (i as f32 + 1.0) * 0.25).collect();

        store.put(key, &data, Tier::Tier1, 0).unwrap();

        let cc = CoherenceCheck::default();
        let result = cc.check_coherence(&mut store, key, &data, 1).unwrap();

        assert_eq!(result.tier, Tier::Tier1);
        assert!(
            result.passed,
            "Tier1 coherence should pass; max_error={}, bound={}",
            result.max_error, cc.max_relative_errors[1],
        );
        assert!(
            result.max_error < cc.max_relative_errors[1],
            "max_error {} should be < bound {}",
            result.max_error,
            cc.max_relative_errors[1],
        );
    }

    #[test]
    fn test_check_coherence_tier3_passes() {
        let mut store = TieredStore::new(4096);
        let key = make_key(2, 0);
        // Use values with large magnitude to keep relative error low under
        // 3-bit quantization (only 7 levels).  Avoid near-zero values where
        // even small absolute error produces large relative error.
        let data: Vec<f32> = (0..32).map(|i| 10.0 + (i as f32) * 0.1).collect();

        store.put(key, &data, Tier::Tier3, 0).unwrap();

        let cc = CoherenceCheck::default();
        let result = cc.check_coherence(&mut store, key, &data, 1).unwrap();

        assert_eq!(result.tier, Tier::Tier3);
        assert!(
            result.passed,
            "Tier3 coherence should pass with default 0.35 bound; max_error={}",
            result.max_error,
        );
    }

    #[test]
    fn test_check_coherence_missing_block() {
        let mut store = TieredStore::new(4096);
        let key = make_key(99, 0);
        let data = vec![1.0f32; 8];
        let cc = CoherenceCheck::default();

        let err = cc.check_coherence(&mut store, key, &data, 0);
        assert_eq!(err, Err(StoreError::BlockNotFound));
    }

    #[test]
    fn test_check_coherence_evicted_block() {
        use crate::store::ReconstructPolicy;

        let mut store = TieredStore::new(4096);
        let key = make_key(3, 0);
        let data = vec![1.0f32; 16];

        store.put(key, &data, Tier::Tier1, 0).unwrap();
        store.evict(key, ReconstructPolicy::None).unwrap();

        let cc = CoherenceCheck::default();
        let err = cc.check_coherence(&mut store, key, &data, 1);
        assert_eq!(err, Err(StoreError::TensorEvicted));
    }

    #[test]
    fn test_check_coherence_tight_bound_fails() {
        let mut store = TieredStore::new(4096);
        let key = make_key(4, 0);
        // Data with large dynamic range to maximize quantization error.
        let data: Vec<f32> = (0..64).map(|i| (i as f32 - 32.0) * 10.0).collect();

        // Store at Tier3 (3-bit) for maximum quantization error.
        store.put(key, &data, Tier::Tier3, 0).unwrap();

        // Use an extremely tight bound that 3-bit quantization cannot meet.
        let cc = CoherenceCheck::new([f32::MAX, 0.001, 0.001, 0.001]);
        let result = cc.check_coherence(&mut store, key, &data, 1).unwrap();

        assert_eq!(result.tier, Tier::Tier3);
        assert!(
            !result.passed,
            "Tier3 with 0.001 bound should fail; max_error={}",
            result.max_error,
        );
    }

    // -- verify_put ---------------------------------------------------------

    #[test]
    fn test_verify_put_tier1() {
        let mut store = TieredStore::new(4096);
        let key = make_key(10, 0);
        let data: Vec<f32> = (0..64).map(|i| (i as f32 + 1.0) * 0.1).collect();

        let cc = CoherenceCheck::default();
        let result = cc
            .verify_put(&mut store, key, &data, Tier::Tier1, 0)
            .unwrap();

        assert_eq!(result.tier, Tier::Tier1);
        assert!(result.passed, "verify_put Tier1 should pass");
        assert_eq!(store.block_count(), 1);
    }

    #[test]
    fn test_verify_put_tier0_rejected() {
        let mut store = TieredStore::new(4096);
        let key = make_key(11, 0);
        let data = vec![1.0f32; 16];

        let cc = CoherenceCheck::default();
        let err = cc.verify_put(&mut store, key, &data, Tier::Tier0, 0);
        assert_eq!(err, Err(StoreError::InvalidBlock));
    }

    #[test]
    fn test_verify_put_tier2() {
        let mut store = TieredStore::new(4096);
        let key = make_key(12, 0);
        let data: Vec<f32> = (0..64).map(|i| (i as f32 + 1.0) * 0.3).collect();

        let cc = CoherenceCheck::default();
        let result = cc
            .verify_put(&mut store, key, &data, Tier::Tier2, 0)
            .unwrap();

        assert_eq!(result.tier, Tier::Tier2);
        assert!(
            result.passed,
            "verify_put Tier2 should pass; max_error={}",
            result.max_error
        );
    }

    // -- compute_max_relative_error -----------------------------------------

    #[test]
    fn test_relative_error_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(compute_max_relative_error(&a, &b), 0.0);
    }

    #[test]
    fn test_relative_error_known() {
        let original = vec![10.0, 20.0, 50.0];
        let decoded = vec![10.5, 20.0, 48.0];
        let err = compute_max_relative_error(&original, &decoded);
        // Element 0: |0.5| / 10.0 = 0.05
        // Element 1: 0.0
        // Element 2: |2.0| / 50.0 = 0.04
        assert!((err - 0.05).abs() < 1e-6, "expected 0.05, got {err}");
    }

    #[test]
    fn test_relative_error_near_zero() {
        // Near-zero original values should use absolute error.
        let original = vec![0.0, 1e-8, 1.0];
        let decoded = vec![0.001, 0.0, 1.0];
        let err = compute_max_relative_error(&original, &decoded);
        // Element 0: |0.001| (absolute, since orig < epsilon)
        // Element 1: |1e-8| (absolute, since orig < epsilon)
        // Element 2: 0.0
        assert!((err - 0.001).abs() < 1e-6, "expected ~0.001, got {err}");
    }

    #[test]
    fn test_relative_error_empty() {
        assert_eq!(compute_max_relative_error(&[], &[]), 0.0);
    }

    #[test]
    fn test_relative_error_mismatched_lengths() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0];
        // Should only compare up to min(len(a), len(b)) = 2 elements.
        let err = compute_max_relative_error(&a, &b);
        assert_eq!(err, 0.0);
    }

    // -- EpochTracker -------------------------------------------------------

    #[test]
    fn test_epoch_tracker_new() {
        let tracker = EpochTracker::new();
        let key = make_key(1, 0);
        assert_eq!(tracker.check_epoch(key), None);
        assert!(!tracker.is_stale(key, 0));
    }

    #[test]
    fn test_epoch_tracker_record_write() {
        let mut tracker = EpochTracker::new();
        let key = make_key(1, 0);

        let e1 = tracker.record_write(key);
        assert_eq!(e1, 1);
        assert_eq!(tracker.check_epoch(key), Some(1));

        let e2 = tracker.record_write(key);
        assert_eq!(e2, 2);
        assert_eq!(tracker.check_epoch(key), Some(2));
    }

    #[test]
    fn test_epoch_tracker_monotonic_across_keys() {
        let mut tracker = EpochTracker::new();
        let key_a = make_key(1, 0);
        let key_b = make_key(2, 0);

        let e1 = tracker.record_write(key_a);
        let e2 = tracker.record_write(key_b);
        let e3 = tracker.record_write(key_a);

        assert_eq!(e1, 1);
        assert_eq!(e2, 2);
        assert_eq!(e3, 3);

        assert_eq!(tracker.check_epoch(key_a), Some(3));
        assert_eq!(tracker.check_epoch(key_b), Some(2));
    }

    #[test]
    fn test_epoch_tracker_is_stale() {
        let mut tracker = EpochTracker::new();
        let key = make_key(1, 0);

        let epoch = tracker.record_write(key);
        assert!(
            !tracker.is_stale(key, epoch),
            "same epoch should not be stale"
        );
        assert!(
            !tracker.is_stale(key, epoch + 1),
            "future epoch should not be stale"
        );

        // Write again -> epoch advances.
        let _e2 = tracker.record_write(key);
        assert!(
            tracker.is_stale(key, epoch),
            "old epoch should now be stale after a new write"
        );
    }

    #[test]
    fn test_epoch_tracker_unknown_key_not_stale() {
        let tracker = EpochTracker::new();
        let key = make_key(99, 0);
        assert!(!tracker.is_stale(key, 0));
        assert!(!tracker.is_stale(key, u64::MAX));
    }

    #[test]
    fn test_epoch_tracker_multiple_keys_independent() {
        let mut tracker = EpochTracker::new();
        let key_a = make_key(1, 0);
        let key_b = make_key(2, 0);

        let ea = tracker.record_write(key_a);
        let _eb = tracker.record_write(key_b);

        // Writing key_b should not make key_a stale at its own epoch.
        assert!(!tracker.is_stale(key_a, ea));
    }

    #[test]
    fn test_epoch_tracker_default_trait() {
        let tracker = EpochTracker::default();
        assert_eq!(tracker.check_epoch(make_key(1, 0)), None);
    }
}
