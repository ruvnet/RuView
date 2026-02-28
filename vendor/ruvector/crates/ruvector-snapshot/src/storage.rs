use async_trait::async_trait;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::PathBuf;
use tokio::fs;

use crate::error::{Result, SnapshotError};
use crate::snapshot::{Snapshot, SnapshotData};

/// Trait for snapshot storage backends
#[async_trait]
pub trait SnapshotStorage: Send + Sync {
    /// Save a snapshot to storage
    async fn save(&self, snapshot: &SnapshotData) -> Result<Snapshot>;

    /// Load a snapshot from storage
    async fn load(&self, id: &str) -> Result<SnapshotData>;

    /// List all available snapshots
    async fn list(&self) -> Result<Vec<Snapshot>>;

    /// Delete a snapshot from storage
    async fn delete(&self, id: &str) -> Result<()>;
}

/// Local filesystem storage backend
pub struct LocalStorage {
    base_path: PathBuf,
}

impl LocalStorage {
    /// Create a new local storage instance
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Get the path for a snapshot file
    fn snapshot_path(&self, id: &str) -> PathBuf {
        self.base_path.join(format!("{}.snapshot.gz", id))
    }

    /// Get the path for a snapshot metadata file
    fn metadata_path(&self, id: &str) -> PathBuf {
        self.base_path.join(format!("{}.metadata.json", id))
    }

    /// Compress data using gzip
    fn compress(data: &[u8]) -> Result<Vec<u8>> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(data)
            .map_err(|e| SnapshotError::compression(format!("Compression failed: {}", e)))?;
        encoder
            .finish()
            .map_err(|e| SnapshotError::compression(format!("Finish compression failed: {}", e)))
    }

    /// Decompress gzip data
    fn decompress(data: &[u8]) -> Result<Vec<u8>> {
        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| SnapshotError::compression(format!("Decompression failed: {}", e)))?;
        Ok(decompressed)
    }

    /// Calculate SHA-256 checksum
    fn calculate_checksum(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Ensure the base directory exists
    async fn ensure_dir(&self) -> Result<()> {
        if !self.base_path.exists() {
            fs::create_dir_all(&self.base_path).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl SnapshotStorage for LocalStorage {
    async fn save(&self, snapshot_data: &SnapshotData) -> Result<Snapshot> {
        self.ensure_dir().await?;

        let id = snapshot_data.id().to_string();
        let snapshot_path = self.snapshot_path(&id);
        let metadata_path = self.metadata_path(&id);

        // Serialize snapshot data
        let config = bincode::config::standard();
        let serialized = bincode::encode_to_vec(snapshot_data, config)
            .map_err(|e| SnapshotError::SerializationError(e.to_string()))?;

        // Calculate checksum before compression
        let checksum = Self::calculate_checksum(&serialized);

        // Compress data
        let compressed = Self::compress(&serialized)?;
        let size_bytes = compressed.len() as u64;

        // Write compressed data
        fs::write(&snapshot_path, &compressed).await?;

        // Create snapshot metadata
        let created_at = chrono::DateTime::parse_from_rfc3339(&snapshot_data.metadata.created_at)
            .map_err(|e| SnapshotError::storage(format!("Invalid timestamp: {}", e)))?
            .with_timezone(&chrono::Utc);

        let snapshot = Snapshot {
            id: id.clone(),
            collection_name: snapshot_data.collection_name().to_string(),
            created_at,
            vectors_count: snapshot_data.vectors_count(),
            checksum,
            size_bytes,
        };

        // Write metadata
        let metadata_json = serde_json::to_string_pretty(&snapshot)?;
        fs::write(&metadata_path, metadata_json).await?;

        Ok(snapshot)
    }

    async fn load(&self, id: &str) -> Result<SnapshotData> {
        let snapshot_path = self.snapshot_path(id);
        let metadata_path = self.metadata_path(id);

        // Check if files exist
        if !snapshot_path.exists() {
            return Err(SnapshotError::SnapshotNotFound(id.to_string()));
        }

        // Load and verify metadata
        let metadata_json = fs::read_to_string(&metadata_path).await?;
        let snapshot: Snapshot = serde_json::from_str(&metadata_json)?;

        // Load compressed data
        let compressed = fs::read(&snapshot_path).await?;

        // Decompress
        let decompressed = Self::decompress(&compressed)?;

        // Verify checksum
        let actual_checksum = Self::calculate_checksum(&decompressed);
        if actual_checksum != snapshot.checksum {
            return Err(SnapshotError::InvalidChecksum {
                expected: snapshot.checksum,
                actual: actual_checksum,
            });
        }

        // Deserialize
        let config = bincode::config::standard();
        let (snapshot_data, _): (SnapshotData, usize) =
            bincode::decode_from_slice(&decompressed, config)
                .map_err(|e| SnapshotError::SerializationError(e.to_string()))?;

        Ok(snapshot_data)
    }

    async fn list(&self) -> Result<Vec<Snapshot>> {
        self.ensure_dir().await?;

        let mut snapshots = Vec::new();
        let mut entries = fs::read_dir(&self.base_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                if extension == "json" {
                    if let Some(file_name) = path.file_stem() {
                        let file_name_str = file_name.to_string_lossy();
                        if file_name_str.ends_with(".metadata") {
                            let contents = fs::read_to_string(&path).await?;
                            if let Ok(snapshot) = serde_json::from_str::<Snapshot>(&contents) {
                                snapshots.push(snapshot);
                            }
                        }
                    }
                }
            }
        }

        // Sort by creation date (newest first)
        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(snapshots)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let snapshot_path = self.snapshot_path(id);
        let metadata_path = self.metadata_path(id);

        if !snapshot_path.exists() {
            return Err(SnapshotError::SnapshotNotFound(id.to_string()));
        }

        // Delete both files
        fs::remove_file(&snapshot_path).await?;

        if metadata_path.exists() {
            fs::remove_file(&metadata_path).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{CollectionConfig, DistanceMetric, VectorRecord};

    #[test]
    fn test_compression_roundtrip() {
        let data = b"Hello, World! This is test data for compression.";
        let compressed = LocalStorage::compress(data).unwrap();
        let decompressed = LocalStorage::decompress(&compressed).unwrap();
        assert_eq!(data.to_vec(), decompressed);
    }

    #[test]
    fn test_checksum_calculation() {
        let data = b"test data";
        let checksum = LocalStorage::calculate_checksum(data);
        assert_eq!(checksum.len(), 64); // SHA-256 produces 64 hex characters
    }

    #[tokio::test]
    async fn test_local_storage_roundtrip() {
        let temp_dir = std::env::temp_dir().join("ruvector-snapshot-test");
        let storage = LocalStorage::new(temp_dir.clone());

        let config = CollectionConfig {
            dimension: 3,
            metric: DistanceMetric::Cosine,
            hnsw_config: None,
        };

        let vectors = vec![
            VectorRecord::new("v1".to_string(), vec![1.0, 0.0, 0.0], None),
            VectorRecord::new("v2".to_string(), vec![0.0, 1.0, 0.0], None),
        ];

        let snapshot_data = SnapshotData::new("test-collection".to_string(), config, vectors);
        let id = snapshot_data.id().to_string();

        // Save
        let snapshot = storage.save(&snapshot_data).await.unwrap();
        assert_eq!(snapshot.id, id);
        assert_eq!(snapshot.vectors_count, 2);

        // List
        let snapshots = storage.list().await.unwrap();
        assert!(!snapshots.is_empty());

        // Load
        let loaded = storage.load(&id).await.unwrap();
        assert_eq!(loaded.id(), id);
        assert_eq!(loaded.vectors_count(), 2);

        // Delete
        storage.delete(&id).await.unwrap();

        // Cleanup
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
