//! Abstract trait interface for tensor block storage.
//!
//! Defines [`TensorStore`] so that other crates can depend on a thin
//! abstraction rather than the concrete [`crate::store::TieredStore`].
//! An extension trait [`TensorStoreExt`] provides convenience helpers
//! via a blanket implementation for all `TensorStore` implementors.

#![allow(dead_code)]

use crate::store::{BlockKey, BlockMeta, ReconstructPolicy, StoreError, Tier, TieredStore};

// ---------------------------------------------------------------------------
// TensorStore trait
// ---------------------------------------------------------------------------

/// Abstract interface for a tiered tensor block store.
///
/// All methods mirror the public API of [`TieredStore`] so that higher-level
/// crates can interact with the store without depending on the concrete type.
pub trait TensorStore {
    /// Quantize `data` at the bit width for `tier` and store the block.
    ///
    /// Replaces any existing block with the same `key`.
    fn put(&mut self, key: BlockKey, data: &[f32], tier: Tier, now: u64) -> Result<(), StoreError>;

    /// Dequantize the block identified by `key` into `out`.
    ///
    /// Returns the number of f32 elements written.
    fn get(&mut self, key: BlockKey, out: &mut [f32], now: u64) -> Result<usize, StoreError>;

    /// Update access statistics for `key` at tick `now`.
    fn touch(&mut self, key: BlockKey, now: u64);

    /// Evict a block to Tier0, preserving metadata with the given policy.
    fn evict(&mut self, key: BlockKey, policy: ReconstructPolicy) -> Result<(), StoreError>;

    /// Return a reference to the metadata for `key`, if it exists.
    fn meta(&self, key: BlockKey) -> Option<&BlockMeta>;

    /// Total number of blocks tracked (including Tier0 evicted blocks).
    fn block_count(&self) -> usize;

    /// Number of blocks currently in the given tier.
    fn tier_count(&self, tier: Tier) -> usize;

    /// Total bytes of quantized data stored across all active tiers.
    fn total_bytes(&self) -> usize;

    /// Whether a block with the given key exists in the store.
    fn contains(&self, key: BlockKey) -> bool;

    /// Capture a read-only snapshot of the store's current state.
    fn snapshot(&self) -> TensorStoreSnapshot;
}

// ---------------------------------------------------------------------------
// TensorStore impl for TieredStore
// ---------------------------------------------------------------------------

impl TensorStore for TieredStore {
    fn put(&mut self, key: BlockKey, data: &[f32], tier: Tier, now: u64) -> Result<(), StoreError> {
        TieredStore::put(self, key, data, tier, now)
    }

    fn get(&mut self, key: BlockKey, out: &mut [f32], now: u64) -> Result<usize, StoreError> {
        TieredStore::get(self, key, out, now)
    }

    fn touch(&mut self, key: BlockKey, now: u64) {
        TieredStore::touch(self, key, now);
    }

    fn evict(&mut self, key: BlockKey, policy: ReconstructPolicy) -> Result<(), StoreError> {
        TieredStore::evict(self, key, policy)
    }

    fn meta(&self, key: BlockKey) -> Option<&BlockMeta> {
        TieredStore::meta(self, key)
    }

    fn block_count(&self) -> usize {
        TieredStore::block_count(self)
    }

    fn tier_count(&self, tier: Tier) -> usize {
        TieredStore::tier_count(self, tier)
    }

    fn total_bytes(&self) -> usize {
        TieredStore::total_bytes(self)
    }

    fn contains(&self, key: BlockKey) -> bool {
        TieredStore::meta(self, key).is_some()
    }

    fn snapshot(&self) -> TensorStoreSnapshot {
        let tier_counts = [
            TieredStore::tier_count(self, Tier::Tier0),
            TieredStore::tier_count(self, Tier::Tier1),
            TieredStore::tier_count(self, Tier::Tier2),
            TieredStore::tier_count(self, Tier::Tier3),
        ];

        // Compute per-tier byte totals from the store metrics.
        let metrics = TieredStore::metrics(self);
        let tier_bytes = [
            0, // Tier0 holds no payload data
            metrics.tier1_bytes as usize,
            metrics.tier2_bytes as usize,
            metrics.tier3_bytes as usize,
        ];

        TensorStoreSnapshot {
            block_count: TieredStore::block_count(self),
            tier_counts,
            total_bytes: TieredStore::total_bytes(self),
            tier_bytes,
        }
    }
}

// ---------------------------------------------------------------------------
// TensorStoreSnapshot
// ---------------------------------------------------------------------------

/// Read-only snapshot of the store's current state.
///
/// Captures block counts, byte totals, and per-tier breakdowns at a single
/// point in time. Useful for monitoring, dashboards, and tiering decisions
/// that need a consistent view without holding a borrow on the store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorStoreSnapshot {
    /// Total number of blocks tracked (including evicted Tier0 blocks).
    pub block_count: usize,
    /// Number of blocks in each tier, indexed as `[Tier0, Tier1, Tier2, Tier3]`.
    pub tier_counts: [usize; 4],
    /// Total bytes of quantized data across all active tiers.
    pub total_bytes: usize,
    /// Bytes of quantized data per tier, indexed as `[Tier0, Tier1, Tier2, Tier3]`.
    pub tier_bytes: [usize; 4],
}

impl TensorStoreSnapshot {
    /// Fraction of total blocks that reside in the given tier.
    ///
    /// Returns 0.0 if the store is empty.
    pub fn tier_fraction(&self, tier: Tier) -> f64 {
        if self.block_count == 0 {
            return 0.0;
        }
        self.tier_counts[tier as usize] as f64 / self.block_count as f64
    }

    /// Fraction of total bytes stored in the given tier.
    ///
    /// Returns 0.0 if the store holds no data.
    pub fn byte_fraction(&self, tier: Tier) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        self.tier_bytes[tier as usize] as f64 / self.total_bytes as f64
    }
}

// ---------------------------------------------------------------------------
// TensorStoreExt extension trait
// ---------------------------------------------------------------------------

/// Convenience methods available on every [`TensorStore`] implementor.
pub trait TensorStoreExt: TensorStore {
    /// Allocate a `Vec<f32>` of length `len` and read the block into it.
    ///
    /// This is a convenience wrapper around [`TensorStore::get`] for callers
    /// that do not want to manage the output buffer themselves.
    fn get_vec(&mut self, key: BlockKey, len: usize, now: u64) -> Result<Vec<f32>, StoreError>;

    /// Store a block in Tier1 (hot, 8-bit quantization).
    ///
    /// Shorthand for `put(key, data, Tier::Tier1, now)`.
    fn put_tier1(&mut self, key: BlockKey, data: &[f32], now: u64) -> Result<(), StoreError>;

    /// Check whether a block has been evicted to Tier0.
    ///
    /// Returns `false` if the block does not exist.
    fn is_evicted(&self, key: BlockKey) -> bool;
}

/// Blanket implementation of [`TensorStoreExt`] for all `TensorStore` types.
impl<T: TensorStore> TensorStoreExt for T {
    fn get_vec(&mut self, key: BlockKey, len: usize, now: u64) -> Result<Vec<f32>, StoreError> {
        let mut buf = vec![0.0f32; len];
        let n = self.get(key, &mut buf, now)?;
        buf.truncate(n);
        Ok(buf)
    }

    fn put_tier1(&mut self, key: BlockKey, data: &[f32], now: u64) -> Result<(), StoreError> {
        self.put(key, data, Tier::Tier1, now)
    }

    fn is_evicted(&self, key: BlockKey) -> bool {
        self.meta(key)
            .map(|m| m.tier == Tier::Tier0)
            .unwrap_or(false)
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

    // -- TensorStore trait delegation ----------------------------------------

    #[test]
    fn test_trait_put_get_roundtrip() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        let data: Vec<f32> = (0..64).map(|i| i as f32 * 0.25).collect();

        // Use trait method
        TensorStore::put(&mut store, key, &data, Tier::Tier1, 0).unwrap();
        assert_eq!(TensorStore::block_count(&store), 1);
        assert!(TensorStore::contains(&store, key));

        let mut out = vec![0.0f32; 64];
        let n = TensorStore::get(&mut store, key, &mut out, 1).unwrap();
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
    fn test_trait_touch_updates_access() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        TensorStore::put(&mut store, key, &[1.0; 16], Tier::Tier1, 0).unwrap();

        let meta = TensorStore::meta(&store, key).unwrap();
        assert_eq!(meta.access_count, 1);

        TensorStore::touch(&mut store, key, 10);
        let meta = TensorStore::meta(&store, key).unwrap();
        assert_eq!(meta.access_count, 2);
        assert_eq!(meta.last_access_at, 10);
    }

    #[test]
    fn test_trait_evict() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        TensorStore::put(&mut store, key, &[1.0; 32], Tier::Tier1, 0).unwrap();
        assert_eq!(TensorStore::tier_count(&store, Tier::Tier1), 1);

        TensorStore::evict(&mut store, key, ReconstructPolicy::Delta).unwrap();

        let meta = TensorStore::meta(&store, key).unwrap();
        assert_eq!(meta.tier, Tier::Tier0);
        assert_eq!(meta.reconstruct, ReconstructPolicy::Delta);
        assert_eq!(TensorStore::tier_count(&store, Tier::Tier0), 1);
        assert_eq!(TensorStore::tier_count(&store, Tier::Tier1), 0);
    }

    #[test]
    fn test_trait_contains_false_for_missing() {
        let store = TieredStore::new(4096);
        assert!(!TensorStore::contains(&store, make_key(99, 0)));
    }

    #[test]
    fn test_trait_total_bytes() {
        let mut store = TieredStore::new(4096);
        assert_eq!(TensorStore::total_bytes(&store), 0);

        TensorStore::put(&mut store, make_key(1, 0), &[1.0; 64], Tier::Tier1, 0).unwrap();
        assert!(TensorStore::total_bytes(&store) > 0);
    }

    // -- TensorStoreSnapshot -------------------------------------------------

    #[test]
    fn test_snapshot_empty_store() {
        let store = TieredStore::new(4096);
        let snap = TensorStore::snapshot(&store);

        assert_eq!(snap.block_count, 0);
        assert_eq!(snap.tier_counts, [0, 0, 0, 0]);
        assert_eq!(snap.total_bytes, 0);
        assert_eq!(snap.tier_bytes, [0, 0, 0, 0]);
    }

    #[test]
    fn test_snapshot_populated_store() {
        let mut store = TieredStore::new(4096);
        let data = vec![1.0f32; 32];

        TensorStore::put(&mut store, make_key(1, 0), &data, Tier::Tier1, 0).unwrap();
        TensorStore::put(&mut store, make_key(2, 0), &data, Tier::Tier1, 0).unwrap();
        TensorStore::put(&mut store, make_key(3, 0), &data, Tier::Tier2, 0).unwrap();
        TensorStore::put(&mut store, make_key(4, 0), &data, Tier::Tier3, 0).unwrap();

        let snap = TensorStore::snapshot(&store);

        assert_eq!(snap.block_count, 4);
        assert_eq!(snap.tier_counts[0], 0); // Tier0
        assert_eq!(snap.tier_counts[1], 2); // Tier1
        assert_eq!(snap.tier_counts[2], 1); // Tier2
        assert_eq!(snap.tier_counts[3], 1); // Tier3
        assert!(snap.total_bytes > 0);
        assert!(snap.tier_bytes[1] > 0); // Tier1 bytes
        assert!(snap.tier_bytes[2] > 0); // Tier2 bytes
        assert!(snap.tier_bytes[3] > 0); // Tier3 bytes
        assert_eq!(snap.tier_bytes[0], 0); // Tier0 holds no data
    }

    #[test]
    fn test_snapshot_tier_fraction() {
        let mut store = TieredStore::new(4096);
        let data = vec![1.0f32; 16];

        TensorStore::put(&mut store, make_key(1, 0), &data, Tier::Tier1, 0).unwrap();
        TensorStore::put(&mut store, make_key(2, 0), &data, Tier::Tier1, 0).unwrap();
        TensorStore::put(&mut store, make_key(3, 0), &data, Tier::Tier2, 0).unwrap();
        TensorStore::put(&mut store, make_key(4, 0), &data, Tier::Tier3, 0).unwrap();

        let snap = TensorStore::snapshot(&store);

        assert!((snap.tier_fraction(Tier::Tier1) - 0.5).abs() < 1e-10);
        assert!((snap.tier_fraction(Tier::Tier2) - 0.25).abs() < 1e-10);
        assert!((snap.tier_fraction(Tier::Tier3) - 0.25).abs() < 1e-10);
        assert!((snap.tier_fraction(Tier::Tier0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_snapshot_tier_fraction_empty() {
        let snap = TensorStoreSnapshot {
            block_count: 0,
            tier_counts: [0; 4],
            total_bytes: 0,
            tier_bytes: [0; 4],
        };
        assert_eq!(snap.tier_fraction(Tier::Tier1), 0.0);
    }

    #[test]
    fn test_snapshot_byte_fraction_empty() {
        let snap = TensorStoreSnapshot {
            block_count: 0,
            tier_counts: [0; 4],
            total_bytes: 0,
            tier_bytes: [0; 4],
        };
        assert_eq!(snap.byte_fraction(Tier::Tier1), 0.0);
    }

    #[test]
    fn test_snapshot_after_eviction() {
        let mut store = TieredStore::new(4096);
        let data = vec![1.0f32; 32];

        TensorStore::put(&mut store, make_key(1, 0), &data, Tier::Tier1, 0).unwrap();
        TensorStore::put(&mut store, make_key(2, 0), &data, Tier::Tier2, 0).unwrap();

        TensorStore::evict(&mut store, make_key(1, 0), ReconstructPolicy::None).unwrap();

        let snap = TensorStore::snapshot(&store);

        assert_eq!(snap.block_count, 2); // metadata preserved
        assert_eq!(snap.tier_counts[0], 1); // one evicted
        assert_eq!(snap.tier_counts[1], 0); // tier1 now empty
        assert_eq!(snap.tier_counts[2], 1); // tier2 still has one
        assert_eq!(snap.tier_bytes[0], 0); // evicted holds no data
        assert_eq!(snap.tier_bytes[1], 0); // tier1 bytes gone
        assert!(snap.tier_bytes[2] > 0); // tier2 bytes remain
    }

    // -- TensorStoreExt convenience methods ----------------------------------

    #[test]
    fn test_ext_get_vec() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        let data: Vec<f32> = (0..32).map(|i| i as f32 * 0.5).collect();

        TensorStore::put(&mut store, key, &data, Tier::Tier1, 0).unwrap();

        let result = TensorStoreExt::get_vec(&mut store, key, 32, 1).unwrap();
        assert_eq!(result.len(), 32);

        for (i, (&orig, &dec)) in data.iter().zip(result.iter()).enumerate() {
            let err = (orig - dec).abs();
            let tol = if orig.abs() > 0.01 {
                orig.abs() * 0.05
            } else {
                0.15
            };
            assert!(err < tol, "i={i} orig={orig} dec={dec} err={err}");
        }
    }

    #[test]
    fn test_ext_get_vec_truncates_to_actual() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        TensorStore::put(&mut store, key, &[1.0; 16], Tier::Tier1, 0).unwrap();

        // Request a larger buffer than the block contains; vec should be truncated.
        let result = TensorStoreExt::get_vec(&mut store, key, 64, 1).unwrap();
        assert_eq!(result.len(), 16);
    }

    #[test]
    fn test_ext_get_vec_not_found() {
        let mut store = TieredStore::new(4096);
        let result = TensorStoreExt::get_vec(&mut store, make_key(99, 0), 16, 0);
        assert_eq!(result, Err(StoreError::BlockNotFound));
    }

    #[test]
    fn test_ext_put_tier1() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        let data = vec![2.0f32; 16];

        TensorStoreExt::put_tier1(&mut store, key, &data, 0).unwrap();

        let meta = TensorStore::meta(&store, key).unwrap();
        assert_eq!(meta.tier, Tier::Tier1);
        assert_eq!(meta.bits, 8);
    }

    #[test]
    fn test_ext_is_evicted_false_when_active() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        TensorStore::put(&mut store, key, &[1.0; 8], Tier::Tier1, 0).unwrap();

        assert!(!TensorStoreExt::is_evicted(&store, key));
    }

    #[test]
    fn test_ext_is_evicted_true_after_evict() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);
        TensorStore::put(&mut store, key, &[1.0; 8], Tier::Tier1, 0).unwrap();

        TensorStore::evict(&mut store, key, ReconstructPolicy::None).unwrap();
        assert!(TensorStoreExt::is_evicted(&store, key));
    }

    #[test]
    fn test_ext_is_evicted_false_when_missing() {
        let store = TieredStore::new(4096);
        assert!(!TensorStoreExt::is_evicted(&store, make_key(99, 0)));
    }

    // -- Trait object safety check -------------------------------------------

    #[test]
    fn test_trait_object_usable() {
        let mut store = TieredStore::new(4096);
        let key = make_key(1, 0);

        // Ensure TensorStore can be used as a trait object for the subset
        // of methods that are object-safe. Since &BlockMeta borrows prevent
        // full dyn dispatch for meta(), we verify the non-borrowing methods.
        fn use_store(s: &mut dyn TensorStore) -> usize {
            s.block_count()
        }

        TensorStore::put(&mut store, key, &[1.0; 8], Tier::Tier1, 0).unwrap();
        assert_eq!(use_store(&mut store), 1);
    }

    // -- Integration: mixed trait + ext usage --------------------------------

    #[test]
    fn test_integration_mixed_usage() {
        let mut store = TieredStore::new(4096);
        let k1 = make_key(1, 0);
        let k2 = make_key(2, 0);
        let k3 = make_key(3, 0);

        // Insert via ext shorthand and trait method.
        TensorStoreExt::put_tier1(&mut store, k1, &[1.0; 32], 0).unwrap();
        TensorStore::put(&mut store, k2, &[2.0; 32], Tier::Tier2, 0).unwrap();
        TensorStore::put(&mut store, k3, &[3.0; 32], Tier::Tier3, 0).unwrap();

        assert_eq!(TensorStore::block_count(&store), 3);
        assert!(TensorStore::contains(&store, k1));
        assert!(TensorStore::contains(&store, k2));
        assert!(TensorStore::contains(&store, k3));

        // Evict k3 and verify via ext method.
        TensorStore::evict(&mut store, k3, ReconstructPolicy::Delta).unwrap();
        assert!(TensorStoreExt::is_evicted(&store, k3));
        assert!(!TensorStoreExt::is_evicted(&store, k1));

        // Read back via ext.
        let v1 = TensorStoreExt::get_vec(&mut store, k1, 32, 10).unwrap();
        assert_eq!(v1.len(), 32);

        // Snapshot should reflect the current state.
        let snap = TensorStore::snapshot(&store);
        assert_eq!(snap.block_count, 3);
        assert_eq!(snap.tier_counts[0], 1); // k3 evicted
        assert_eq!(snap.tier_counts[1], 1); // k1
        assert_eq!(snap.tier_counts[2], 1); // k2
        assert_eq!(snap.tier_counts[3], 0); // k3 was here but evicted
    }
}
