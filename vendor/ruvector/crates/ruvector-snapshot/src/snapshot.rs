use bincode::{Decode, Encode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Snapshot metadata and information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Unique snapshot identifier
    pub id: String,

    /// Name of the collection this snapshot represents
    pub collection_name: String,

    /// Timestamp when the snapshot was created
    pub created_at: DateTime<Utc>,

    /// Number of vectors in the snapshot
    pub vectors_count: usize,

    /// SHA-256 checksum of the snapshot data
    pub checksum: String,

    /// Size of the snapshot in bytes (compressed)
    pub size_bytes: u64,
}

/// Complete snapshot data including metadata and vectors
#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct SnapshotData {
    /// Snapshot metadata
    pub metadata: SnapshotMetadata,

    /// Collection configuration
    pub config: CollectionConfig,

    /// All vectors in the collection
    pub vectors: Vec<VectorRecord>,
}

impl SnapshotData {
    /// Create a new snapshot data instance
    pub fn new(
        collection_name: String,
        config: CollectionConfig,
        vectors: Vec<VectorRecord>,
    ) -> Self {
        Self {
            metadata: SnapshotMetadata {
                id: uuid::Uuid::new_v4().to_string(),
                collection_name,
                created_at: Utc::now().to_rfc3339(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            config,
            vectors,
        }
    }

    /// Get the number of vectors in this snapshot
    pub fn vectors_count(&self) -> usize {
        self.vectors.len()
    }

    /// Get the snapshot ID
    pub fn id(&self) -> &str {
        &self.metadata.id
    }

    /// Get the collection name
    pub fn collection_name(&self) -> &str {
        &self.metadata.collection_name
    }
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct SnapshotMetadata {
    /// Unique snapshot identifier
    pub id: String,

    /// Name of the collection
    pub collection_name: String,

    /// Creation timestamp (RFC3339 format)
    pub created_at: String,

    /// Version of the snapshot format
    pub version: String,
}

/// Collection configuration stored in snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct CollectionConfig {
    /// Vector dimension
    pub dimension: usize,

    /// Distance metric
    pub metric: DistanceMetric,

    /// HNSW configuration
    pub hnsw_config: Option<HnswConfig>,
}

/// Distance metric for vector similarity
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct,
}

/// HNSW index configuration
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct HnswConfig {
    pub m: usize,
    pub ef_construction: usize,
    pub ef_search: usize,
}

/// Individual vector record in a snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct VectorRecord {
    /// Unique vector identifier
    pub id: String,

    /// Vector data
    pub vector: Vec<f32>,

    /// Optional metadata payload (stored as JSON string for bincode compatibility)
    #[serde(skip)]
    #[bincode(with_serde)]
    payload_json: Option<String>,
}

impl VectorRecord {
    /// Create a new vector record
    pub fn new(id: String, vector: Vec<f32>, payload: Option<Value>) -> Self {
        let payload_json = payload.and_then(|v| serde_json::to_string(&v).ok());
        Self {
            id,
            vector,
            payload_json,
        }
    }

    /// Get the payload as a serde_json::Value
    pub fn payload(&self) -> Option<Value> {
        self.payload_json
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
    }

    /// Set the payload from a serde_json::Value
    pub fn set_payload(&mut self, payload: Option<Value>) {
        self.payload_json = payload.and_then(|v| serde_json::to_string(&v).ok());
    }

    /// Get the dimension of this vector
    pub fn dimension(&self) -> usize {
        self.vector.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_record_creation() {
        let record = VectorRecord::new("test-1".to_string(), vec![1.0, 2.0, 3.0], None);
        assert_eq!(record.id, "test-1");
        assert_eq!(record.dimension(), 3);
    }

    #[test]
    fn test_snapshot_data_creation() {
        let config = CollectionConfig {
            dimension: 3,
            metric: DistanceMetric::Cosine,
            hnsw_config: None,
        };

        let vectors = vec![
            VectorRecord::new("v1".to_string(), vec![1.0, 0.0, 0.0], None),
            VectorRecord::new("v2".to_string(), vec![0.0, 1.0, 0.0], None),
        ];

        let data = SnapshotData::new("test-collection".to_string(), config, vectors);

        assert_eq!(data.vectors_count(), 2);
        assert_eq!(data.collection_name(), "test-collection");
        assert!(!data.id().is_empty());
    }
}
