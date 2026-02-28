//! Logical deletion: soft-delete via JOURNAL_SEG tombstones.
//!
//! Deletion protocol (two-fsync):
//! 1. Append JOURNAL_SEG with tombstone entries
//! 2. fsync (orphan-safe: no manifest references it yet)
//! 3. Update deletion bitmap in memory
//! 4. Append MANIFEST_SEG with updated bitmap
//! 5. fsync (deletion now visible to all new readers)
//!
//! Physical reclamation happens during compaction.

use std::collections::HashSet;

/// In-memory deletion bitmap.
///
/// Tracks soft-deleted vector IDs. In a production implementation this
/// would use a Roaring bitmap for space efficiency; here we use a HashSet
/// for correctness and clarity.
pub(crate) struct DeletionBitmap {
    deleted: HashSet<u64>,
}

impl DeletionBitmap {
    pub(crate) fn new() -> Self {
        Self {
            deleted: HashSet::new(),
        }
    }

    /// Load from a list of deleted IDs (e.g., from a manifest).
    pub(crate) fn from_ids(ids: &[u64]) -> Self {
        Self {
            deleted: ids.iter().copied().collect(),
        }
    }

    /// Mark a vector ID as soft-deleted.
    pub(crate) fn delete(&mut self, id: u64) {
        self.deleted.insert(id);
    }

    /// Mark multiple vector IDs as soft-deleted.
    #[allow(dead_code)]
    pub(crate) fn delete_batch(&mut self, ids: &[u64]) {
        for &id in ids {
            self.deleted.insert(id);
        }
    }

    /// Check if a vector ID is soft-deleted.
    #[inline]
    pub(crate) fn is_deleted(&self, id: u64) -> bool {
        self.deleted.contains(&id)
    }

    /// Remove vector IDs from the bitmap (after compaction physically removes them).
    #[allow(dead_code)]
    pub(crate) fn clear_ids(&mut self, ids: &[u64]) {
        for &id in ids {
            self.deleted.remove(&id);
        }
    }

    /// Number of soft-deleted vectors.
    pub(crate) fn count(&self) -> usize {
        self.deleted.len()
    }

    /// Return all deleted IDs as a sorted vector.
    pub(crate) fn to_sorted_ids(&self) -> Vec<u64> {
        let mut ids: Vec<u64> = self.deleted.iter().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// Clear all entries.
    pub(crate) fn clear(&mut self) {
        self.deleted.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_basic_ops() {
        let mut bm = DeletionBitmap::new();
        assert!(!bm.is_deleted(42));
        assert_eq!(bm.count(), 0);

        bm.delete(42);
        assert!(bm.is_deleted(42));
        assert_eq!(bm.count(), 1);

        bm.delete_batch(&[100, 200, 300]);
        assert_eq!(bm.count(), 4);
        assert!(bm.is_deleted(200));

        bm.clear_ids(&[42, 200]);
        assert_eq!(bm.count(), 2);
        assert!(!bm.is_deleted(42));
        assert!(!bm.is_deleted(200));
        assert!(bm.is_deleted(100));
    }

    #[test]
    fn bitmap_from_ids() {
        let bm = DeletionBitmap::from_ids(&[1, 2, 3]);
        assert!(bm.is_deleted(1));
        assert!(bm.is_deleted(2));
        assert!(bm.is_deleted(3));
        assert!(!bm.is_deleted(4));
    }

    #[test]
    fn bitmap_to_sorted() {
        let mut bm = DeletionBitmap::new();
        bm.delete_batch(&[50, 10, 30, 20, 40]);
        assert_eq!(bm.to_sorted_ids(), vec![10, 20, 30, 40, 50]);
    }
}
