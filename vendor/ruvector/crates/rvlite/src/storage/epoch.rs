//! Epoch-based reconciliation for hybrid RVF + IndexedDB persistence.
//!
//! RVF is the source of truth for vectors. IndexedDB is a rebuildable
//! cache for metadata. Both stores share a monotonic epoch counter.
//!
//! Write order:
//! 1. Write vectors to RVF (append-only, crash-safe)
//! 2. Write metadata to IndexedDB
//! 3. Commit shared epoch in both stores
//!
//! On startup: compare epochs and rebuild the lagging side.

use std::sync::atomic::{AtomicU64, Ordering};

/// Monotonic epoch counter shared between RVF and metadata stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Epoch(pub u64);

impl Epoch {
    pub const ZERO: Self = Self(0);

    pub fn next(self) -> Self {
        Self(self.0.checked_add(1).expect("epoch overflow"))
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

/// State describing the relationship between RVF and metadata epochs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpochState {
    /// Both stores agree on the current epoch.
    Synchronized,
    /// RVF store is ahead of metadata by the given delta.
    RvfAhead(u64),
    /// Metadata store is ahead of RVF by the given delta (anomalous).
    MetadataAhead(u64),
}

/// Action to take after comparing epochs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileAction {
    /// No reconciliation needed -- both stores are in sync.
    None,
    /// Metadata is stale; rebuild it from the authoritative RVF store.
    RebuildMetadata,
    /// RVF is somehow behind metadata; rebuild vectors from RVF file.
    /// This should not normally happen and indicates a prior incomplete write.
    RebuildFromRvf,
    /// Metadata is ahead which should never happen under correct operation.
    /// Log a warning and trust RVF as the source of truth.
    LogWarningTrustRvf,
}

/// Result of comparing epochs between RVF and metadata stores.
///
/// Kept for backward compatibility with existing callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconciliationAction {
    /// Both stores are in sync -- no action needed.
    InSync,
    /// RVF is ahead -- rebuild metadata from RVF vectors.
    RebuildMetadata {
        rvf_epoch: Epoch,
        metadata_epoch: Epoch,
    },
    /// Metadata is ahead (should not happen) -- log warning, trust RVF.
    TrustRvf {
        rvf_epoch: Epoch,
        metadata_epoch: Epoch,
    },
}

/// Compare raw epoch values and return the relationship state.
pub fn compare_epochs(rvf_epoch: u64, metadata_epoch: u64) -> EpochState {
    if rvf_epoch == metadata_epoch {
        EpochState::Synchronized
    } else if rvf_epoch > metadata_epoch {
        EpochState::RvfAhead(rvf_epoch - metadata_epoch)
    } else {
        EpochState::MetadataAhead(metadata_epoch - rvf_epoch)
    }
}

/// Determine the reconciliation action for a given epoch state.
pub fn reconcile_action(state: &EpochState) -> ReconcileAction {
    match state {
        EpochState::Synchronized => ReconcileAction::None,
        EpochState::RvfAhead(delta) => {
            if *delta == 1 {
                // Common case: a single write committed to RVF but metadata
                // update was lost (e.g. crash between step 1 and step 2).
                ReconcileAction::RebuildMetadata
            } else {
                // Multiple epochs behind -- still rebuild metadata, but the
                // gap is larger so more data must be replayed.
                ReconcileAction::RebuildMetadata
            }
        }
        EpochState::MetadataAhead(delta) => {
            if *delta == 1 {
                // Metadata committed but RVF write was lost. This means the
                // RVF file is still valid at its own epoch -- rebuild from it.
                ReconcileAction::RebuildFromRvf
            } else {
                // Large gap with metadata ahead is anomalous. Trust RVF.
                ReconcileAction::LogWarningTrustRvf
            }
        }
    }
}

/// Compare epochs and determine reconciliation action (legacy API).
pub fn reconcile(rvf_epoch: Epoch, metadata_epoch: Epoch) -> ReconciliationAction {
    match rvf_epoch.cmp(&metadata_epoch) {
        std::cmp::Ordering::Equal => ReconciliationAction::InSync,
        std::cmp::Ordering::Greater => ReconciliationAction::RebuildMetadata {
            rvf_epoch,
            metadata_epoch,
        },
        std::cmp::Ordering::Less => ReconciliationAction::TrustRvf {
            rvf_epoch,
            metadata_epoch,
        },
    }
}

/// Thread-safe monotonic epoch tracker.
///
/// Uses `AtomicU64` internally so it can be shared across threads without
/// a mutex. The counter is strictly monotonic: it can only move forward.
///
/// # Write protocol
///
/// Callers must follow the three-phase commit:
/// 1. Call `begin_write()` to get the next epoch value.
/// 2. Write vectors to RVF with that epoch.
/// 3. Write metadata to IndexedDB with that epoch.
/// 4. Call `commit(epoch)` to advance the tracker.
///
/// If step 2 or 3 fails, do NOT call `commit` -- the tracker stays at the
/// previous epoch so that the next startup triggers reconciliation.
pub struct EpochTracker {
    /// Current committed epoch.
    current: AtomicU64,
}

impl EpochTracker {
    /// Create a new tracker starting at the given epoch.
    pub fn new(initial: u64) -> Self {
        Self {
            current: AtomicU64::new(initial),
        }
    }

    /// Create a tracker starting at epoch zero.
    pub fn zero() -> Self {
        Self::new(0)
    }

    /// Read the current committed epoch.
    pub fn current(&self) -> u64 {
        self.current.load(Ordering::Acquire)
    }

    /// Return the next epoch value for a pending write.
    ///
    /// This does NOT advance the tracker. The caller must call `commit`
    /// after both RVF and metadata writes succeed.
    pub fn begin_write(&self) -> u64 {
        self.current
            .load(Ordering::Acquire)
            .checked_add(1)
            .expect("epoch overflow")
    }

    /// Commit the given epoch, advancing the tracker.
    ///
    /// Returns `true` if the commit succeeded (epoch was exactly current + 1).
    /// Returns `false` if the epoch was stale or out of order, which means
    /// another writer committed first or the caller passed a wrong value.
    pub fn commit(&self, epoch: u64) -> bool {
        let expected = epoch.checked_sub(1).unwrap_or(0);
        self.current
            .compare_exchange(expected, epoch, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Force-set the epoch to a specific value.
    ///
    /// Used during recovery/reconciliation when we need to align the
    /// tracker with a known-good state read from disk.
    pub fn force_set(&self, epoch: u64) {
        self.current.store(epoch, Ordering::Release);
    }

    /// Check the relationship between the RVF epoch stored on disk and the
    /// metadata epoch, then return the appropriate reconciliation action.
    pub fn check_and_reconcile(&self, rvf_epoch: u64, metadata_epoch: u64) -> ReconcileAction {
        let state = compare_epochs(rvf_epoch, metadata_epoch);
        let action = reconcile_action(&state);

        // After reconciliation, align the tracker to the authoritative epoch.
        match &action {
            ReconcileAction::None => {
                self.force_set(rvf_epoch);
            }
            ReconcileAction::RebuildMetadata | ReconcileAction::RebuildFromRvf => {
                // After rebuild, both sides will match the RVF epoch.
                self.force_set(rvf_epoch);
            }
            ReconcileAction::LogWarningTrustRvf => {
                // Trust RVF -- set tracker to RVF epoch.
                self.force_set(rvf_epoch);
            }
        }

        action
    }
}

impl std::fmt::Debug for EpochTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpochTracker")
            .field("current", &self.current.load(Ordering::Relaxed))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Legacy API tests (preserved) ----

    #[test]
    fn in_sync() {
        let e = Epoch(5);
        assert_eq!(reconcile(e, e), ReconciliationAction::InSync);
    }

    #[test]
    fn rvf_ahead_rebuilds_metadata() {
        let action = reconcile(Epoch(3), Epoch(2));
        assert_eq!(
            action,
            ReconciliationAction::RebuildMetadata {
                rvf_epoch: Epoch(3),
                metadata_epoch: Epoch(2),
            }
        );
    }

    #[test]
    fn metadata_ahead_trusts_rvf() {
        let action = reconcile(Epoch(1), Epoch(3));
        assert_eq!(
            action,
            ReconciliationAction::TrustRvf {
                rvf_epoch: Epoch(1),
                metadata_epoch: Epoch(3),
            }
        );
    }

    #[test]
    fn epoch_increment() {
        assert_eq!(Epoch::ZERO.next(), Epoch(1));
        assert_eq!(Epoch(99).next(), Epoch(100));
    }

    // ---- New epoch state / reconcile tests ----

    #[test]
    fn compare_epochs_synchronized() {
        assert_eq!(compare_epochs(5, 5), EpochState::Synchronized);
        assert_eq!(compare_epochs(0, 0), EpochState::Synchronized);
    }

    #[test]
    fn compare_epochs_rvf_ahead() {
        assert_eq!(compare_epochs(10, 7), EpochState::RvfAhead(3));
        assert_eq!(compare_epochs(1, 0), EpochState::RvfAhead(1));
    }

    #[test]
    fn compare_epochs_metadata_ahead() {
        assert_eq!(compare_epochs(3, 8), EpochState::MetadataAhead(5));
        assert_eq!(compare_epochs(0, 1), EpochState::MetadataAhead(1));
    }

    #[test]
    fn reconcile_action_none_when_synchronized() {
        let state = EpochState::Synchronized;
        assert_eq!(reconcile_action(&state), ReconcileAction::None);
    }

    #[test]
    fn reconcile_action_rebuild_metadata_when_rvf_ahead() {
        assert_eq!(
            reconcile_action(&EpochState::RvfAhead(1)),
            ReconcileAction::RebuildMetadata
        );
        assert_eq!(
            reconcile_action(&EpochState::RvfAhead(5)),
            ReconcileAction::RebuildMetadata
        );
    }

    #[test]
    fn reconcile_action_rebuild_from_rvf_when_metadata_ahead_by_one() {
        assert_eq!(
            reconcile_action(&EpochState::MetadataAhead(1)),
            ReconcileAction::RebuildFromRvf
        );
    }

    #[test]
    fn reconcile_action_log_warning_when_metadata_far_ahead() {
        assert_eq!(
            reconcile_action(&EpochState::MetadataAhead(3)),
            ReconcileAction::LogWarningTrustRvf
        );
    }

    // ---- EpochTracker tests ----

    #[test]
    fn tracker_zero_starts_at_zero() {
        let tracker = EpochTracker::zero();
        assert_eq!(tracker.current(), 0);
    }

    #[test]
    fn tracker_new_starts_at_initial() {
        let tracker = EpochTracker::new(42);
        assert_eq!(tracker.current(), 42);
    }

    #[test]
    fn tracker_begin_write_returns_next() {
        let tracker = EpochTracker::new(10);
        assert_eq!(tracker.begin_write(), 11);
        // begin_write is idempotent until commit
        assert_eq!(tracker.begin_write(), 11);
    }

    #[test]
    fn tracker_commit_advances_epoch() {
        let tracker = EpochTracker::zero();
        let next = tracker.begin_write();
        assert_eq!(next, 1);
        assert!(tracker.commit(next));
        assert_eq!(tracker.current(), 1);

        let next2 = tracker.begin_write();
        assert_eq!(next2, 2);
        assert!(tracker.commit(next2));
        assert_eq!(tracker.current(), 2);
    }

    #[test]
    fn tracker_commit_rejects_stale_epoch() {
        let tracker = EpochTracker::new(5);
        // Try to commit epoch 3 which is behind current
        assert!(!tracker.commit(3));
        assert_eq!(tracker.current(), 5);
    }

    #[test]
    fn tracker_commit_rejects_skip() {
        let tracker = EpochTracker::new(5);
        // Try to commit epoch 8, skipping 6 and 7
        assert!(!tracker.commit(8));
        assert_eq!(tracker.current(), 5);
    }

    #[test]
    fn tracker_force_set() {
        let tracker = EpochTracker::new(10);
        tracker.force_set(100);
        assert_eq!(tracker.current(), 100);
        // Can also go backward with force_set (recovery scenario)
        tracker.force_set(5);
        assert_eq!(tracker.current(), 5);
    }

    #[test]
    fn tracker_check_and_reconcile_in_sync() {
        let tracker = EpochTracker::zero();
        let action = tracker.check_and_reconcile(7, 7);
        assert_eq!(action, ReconcileAction::None);
        assert_eq!(tracker.current(), 7);
    }

    #[test]
    fn tracker_check_and_reconcile_rvf_ahead() {
        let tracker = EpochTracker::zero();
        let action = tracker.check_and_reconcile(10, 8);
        assert_eq!(action, ReconcileAction::RebuildMetadata);
        assert_eq!(tracker.current(), 10);
    }

    #[test]
    fn tracker_check_and_reconcile_metadata_far_ahead() {
        let tracker = EpochTracker::zero();
        let action = tracker.check_and_reconcile(3, 8);
        assert_eq!(action, ReconcileAction::LogWarningTrustRvf);
        assert_eq!(tracker.current(), 3);
    }

    #[test]
    fn tracker_debug_format() {
        let tracker = EpochTracker::new(42);
        let debug = format!("{:?}", tracker);
        assert!(debug.contains("EpochTracker"));
        assert!(debug.contains("42"));
    }

    // ---- Thread safety (basic) ----

    #[test]
    fn tracker_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<EpochTracker>();
    }
}
