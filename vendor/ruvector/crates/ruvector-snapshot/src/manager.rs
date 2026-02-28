use crate::error::{Result, SnapshotError};
use crate::snapshot::{Snapshot, SnapshotData};
use crate::storage::SnapshotStorage;

/// Manages snapshot operations for collections
pub struct SnapshotManager {
    storage: Box<dyn SnapshotStorage>,
}

impl SnapshotManager {
    /// Create a new snapshot manager with the given storage backend
    pub fn new(storage: Box<dyn SnapshotStorage>) -> Self {
        Self { storage }
    }

    /// Create a snapshot of a collection
    ///
    /// # Arguments
    /// * `snapshot_data` - The complete snapshot data including vectors and configuration
    ///
    /// # Returns
    /// * `Snapshot` - Metadata about the created snapshot
    pub async fn create_snapshot(&self, snapshot_data: SnapshotData) -> Result<Snapshot> {
        // Validate snapshot data
        if snapshot_data.vectors.is_empty() {
            return Err(SnapshotError::storage(
                "Cannot create snapshot of empty collection",
            ));
        }

        // Verify all vectors have the same dimension
        let expected_dim = snapshot_data.config.dimension;
        for (idx, vector) in snapshot_data.vectors.iter().enumerate() {
            if vector.vector.len() != expected_dim {
                return Err(SnapshotError::storage(format!(
                    "Vector {} has dimension {} but expected {}",
                    idx,
                    vector.vector.len(),
                    expected_dim
                )));
            }
        }

        // Save the snapshot
        self.storage.save(&snapshot_data).await
    }

    /// Restore a snapshot by ID
    ///
    /// # Arguments
    /// * `id` - The unique snapshot identifier
    ///
    /// # Returns
    /// * `SnapshotData` - The complete snapshot data including vectors and configuration
    pub async fn restore_snapshot(&self, id: &str) -> Result<SnapshotData> {
        if id.is_empty() {
            return Err(SnapshotError::storage("Snapshot ID cannot be empty"));
        }

        self.storage.load(id).await
    }

    /// List all available snapshots
    ///
    /// # Returns
    /// * `Vec<Snapshot>` - List of all snapshot metadata, sorted by creation date (newest first)
    pub async fn list_snapshots(&self) -> Result<Vec<Snapshot>> {
        self.storage.list().await
    }

    /// List snapshots for a specific collection
    ///
    /// # Arguments
    /// * `collection_name` - Name of the collection to filter by
    ///
    /// # Returns
    /// * `Vec<Snapshot>` - List of snapshots for the specified collection
    pub async fn list_snapshots_for_collection(
        &self,
        collection_name: &str,
    ) -> Result<Vec<Snapshot>> {
        let all_snapshots = self.storage.list().await?;
        Ok(all_snapshots
            .into_iter()
            .filter(|s| s.collection_name == collection_name)
            .collect())
    }

    /// Delete a snapshot by ID
    ///
    /// # Arguments
    /// * `id` - The unique snapshot identifier
    pub async fn delete_snapshot(&self, id: &str) -> Result<()> {
        if id.is_empty() {
            return Err(SnapshotError::storage("Snapshot ID cannot be empty"));
        }

        self.storage.delete(id).await
    }

    /// Get snapshot metadata by ID
    ///
    /// # Arguments
    /// * `id` - The unique snapshot identifier
    ///
    /// # Returns
    /// * `Snapshot` - Metadata about the snapshot
    pub async fn get_snapshot_info(&self, id: &str) -> Result<Snapshot> {
        let snapshots = self.storage.list().await?;
        snapshots
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| SnapshotError::SnapshotNotFound(id.to_string()))
    }

    /// Delete old snapshots, keeping only the N most recent
    ///
    /// # Arguments
    /// * `collection_name` - Name of the collection
    /// * `keep_count` - Number of recent snapshots to keep
    ///
    /// # Returns
    /// * `usize` - Number of snapshots deleted
    pub async fn cleanup_old_snapshots(
        &self,
        collection_name: &str,
        keep_count: usize,
    ) -> Result<usize> {
        let snapshots = self.list_snapshots_for_collection(collection_name).await?;

        if snapshots.len() <= keep_count {
            return Ok(0);
        }

        let to_delete = &snapshots[keep_count..];
        let mut deleted = 0;

        for snapshot in to_delete {
            if self.storage.delete(&snapshot.id).await.is_ok() {
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    /// Get the total size of all snapshots in bytes
    pub async fn total_size(&self) -> Result<u64> {
        let snapshots = self.storage.list().await?;
        Ok(snapshots.iter().map(|s| s.size_bytes).sum())
    }

    /// Get the total size of snapshots for a specific collection
    pub async fn collection_size(&self, collection_name: &str) -> Result<u64> {
        let snapshots = self.list_snapshots_for_collection(collection_name).await?;
        Ok(snapshots.iter().map(|s| s.size_bytes).sum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{CollectionConfig, DistanceMetric, VectorRecord};
    use crate::storage::LocalStorage;
    use std::path::PathBuf;

    fn create_test_snapshot_data(name: &str, vector_count: usize) -> SnapshotData {
        let config = CollectionConfig {
            dimension: 3,
            metric: DistanceMetric::Cosine,
            hnsw_config: None,
        };

        let vectors = (0..vector_count)
            .map(|i| {
                VectorRecord::new(
                    format!("v{}", i),
                    vec![i as f32, (i + 1) as f32, (i + 2) as f32],
                    None,
                )
            })
            .collect();

        SnapshotData::new(name.to_string(), config, vectors)
    }

    #[tokio::test]
    async fn test_create_and_restore_snapshot() {
        let temp_dir = std::env::temp_dir().join("ruvector-manager-test");
        let storage = Box::new(LocalStorage::new(temp_dir.clone()));
        let manager = SnapshotManager::new(storage);

        let snapshot_data = create_test_snapshot_data("test-collection", 5);
        let id = snapshot_data.id().to_string();

        // Create snapshot
        let snapshot = manager.create_snapshot(snapshot_data).await.unwrap();
        assert_eq!(snapshot.id, id);
        assert_eq!(snapshot.vectors_count, 5);

        // Restore snapshot
        let restored = manager.restore_snapshot(&id).await.unwrap();
        assert_eq!(restored.id(), id);
        assert_eq!(restored.vectors_count(), 5);

        // Cleanup
        let _ = manager.delete_snapshot(&id).await;
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_list_snapshots() {
        let temp_dir = std::env::temp_dir().join("ruvector-list-test");
        let storage = Box::new(LocalStorage::new(temp_dir.clone()));
        let manager = SnapshotManager::new(storage);

        // Create multiple snapshots
        let snapshot1 = create_test_snapshot_data("collection-1", 3);
        let snapshot2 = create_test_snapshot_data("collection-2", 5);

        let id1 = snapshot1.id().to_string();
        let id2 = snapshot2.id().to_string();

        manager.create_snapshot(snapshot1).await.unwrap();
        manager.create_snapshot(snapshot2).await.unwrap();

        // List all
        let all_snapshots = manager.list_snapshots().await.unwrap();
        assert!(all_snapshots.len() >= 2);

        // List by collection
        let collection1_snapshots = manager
            .list_snapshots_for_collection("collection-1")
            .await
            .unwrap();
        assert_eq!(collection1_snapshots.len(), 1);

        // Cleanup
        let _ = manager.delete_snapshot(&id1).await;
        let _ = manager.delete_snapshot(&id2).await;
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_cleanup_old_snapshots() {
        let temp_dir = std::env::temp_dir().join("ruvector-cleanup-test");
        let storage = Box::new(LocalStorage::new(temp_dir.clone()));
        let manager = SnapshotManager::new(storage);

        // Create multiple snapshots for the same collection
        for i in 0..5 {
            let snapshot_data = create_test_snapshot_data("test-collection", i + 1);
            manager.create_snapshot(snapshot_data).await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Cleanup, keeping only 2 most recent
        let deleted = manager
            .cleanup_old_snapshots("test-collection", 2)
            .await
            .unwrap();
        assert_eq!(deleted, 3);

        // Verify only 2 remain
        let remaining = manager
            .list_snapshots_for_collection("test-collection")
            .await
            .unwrap();
        assert_eq!(remaining.len(), 2);

        // Cleanup
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_snapshot_validation() {
        let temp_dir = std::env::temp_dir().join("ruvector-validation-test");
        let storage = Box::new(LocalStorage::new(temp_dir.clone()));
        let manager = SnapshotManager::new(storage);

        // Test empty collection
        let config = CollectionConfig {
            dimension: 3,
            metric: DistanceMetric::Cosine,
            hnsw_config: None,
        };
        let empty_data = SnapshotData::new("empty".to_string(), config, vec![]);
        let result = manager.create_snapshot(empty_data).await;
        assert!(result.is_err());

        // Cleanup
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
