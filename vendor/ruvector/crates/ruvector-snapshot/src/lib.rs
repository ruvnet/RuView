//! Snapshot and restore functionality for rUvector collections
//!
//! This crate provides backup and restore capabilities for vector collections,
//! including compression, checksums, and multiple storage backends.

mod error;
mod manager;
mod snapshot;
mod storage;

pub use error::{Result, SnapshotError};
pub use manager::SnapshotManager;
pub use snapshot::{Snapshot, SnapshotData, SnapshotMetadata, VectorRecord};
pub use storage::{LocalStorage, SnapshotStorage};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all public exports are accessible
        let _: Option<SnapshotError> = None;
        let _: Option<SnapshotManager> = None;
        let _: Option<Snapshot> = None;
    }
}
