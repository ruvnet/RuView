//! Store status reporting.

/// Compaction state as reported in store status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactionState {
    /// No compaction in progress.
    Idle,
    /// Normal compaction running.
    Running,
    /// Emergency compaction (dead_space > 70%).
    Emergency,
}

/// A snapshot of the store's current state.
#[derive(Clone, Debug)]
pub struct StoreStatus {
    /// Total number of live (non-deleted) vectors.
    pub total_vectors: u64,
    /// Total number of segments in the file.
    pub total_segments: u32,
    /// Total file size in bytes.
    pub file_size: u64,
    /// Current manifest epoch.
    pub current_epoch: u32,
    /// Hardware profile identifier.
    pub profile_id: u8,
    /// Current compaction state.
    pub compaction_state: CompactionState,
    /// Ratio of dead space to total file size (0.0 - 1.0).
    pub dead_space_ratio: f64,
    /// Whether the store is open in read-only mode.
    pub read_only: bool,
}
