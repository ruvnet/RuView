//! Collection types and operations

use ruvector_core::types::{DistanceMetric, HnswConfig, QuantizationConfig};
use ruvector_core::vector_db::VectorDB;
use serde::{Deserialize, Serialize};

use crate::error::{CollectionError, Result};

/// Configuration for creating a collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionConfig {
    /// Vector dimensions
    pub dimensions: usize,

    /// Distance metric for similarity calculation
    pub distance_metric: DistanceMetric,

    /// HNSW index configuration
    pub hnsw_config: Option<HnswConfig>,

    /// Quantization configuration
    pub quantization: Option<QuantizationConfig>,

    /// Whether to store payload data on disk
    pub on_disk_payload: bool,
}

impl CollectionConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        if self.dimensions == 0 {
            return Err(CollectionError::InvalidConfiguration {
                message: "Dimensions must be greater than 0".to_string(),
            });
        }

        if self.dimensions > 100_000 {
            return Err(CollectionError::InvalidConfiguration {
                message: "Dimensions exceeds maximum of 100,000".to_string(),
            });
        }

        // Validate HNSW config if present
        if let Some(ref hnsw_config) = self.hnsw_config {
            if hnsw_config.m == 0 {
                return Err(CollectionError::InvalidConfiguration {
                    message: "HNSW M parameter must be greater than 0".to_string(),
                });
            }

            if hnsw_config.ef_construction < hnsw_config.m {
                return Err(CollectionError::InvalidConfiguration {
                    message: "HNSW ef_construction must be >= M".to_string(),
                });
            }

            if hnsw_config.ef_search == 0 {
                return Err(CollectionError::InvalidConfiguration {
                    message: "HNSW ef_search must be greater than 0".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Create a default configuration for the given dimensions
    pub fn with_dimensions(dimensions: usize) -> Self {
        Self {
            dimensions,
            distance_metric: DistanceMetric::Cosine,
            hnsw_config: Some(HnswConfig::default()),
            quantization: Some(QuantizationConfig::Scalar),
            on_disk_payload: true,
        }
    }
}

/// A collection of vectors with its own configuration
pub struct Collection {
    /// Collection name
    pub name: String,

    /// Collection configuration
    pub config: CollectionConfig,

    /// Underlying vector database
    pub db: VectorDB,

    /// When the collection was created (Unix timestamp in seconds)
    pub created_at: i64,

    /// When the collection was last updated (Unix timestamp in seconds)
    pub updated_at: i64,
}

impl std::fmt::Debug for Collection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Collection")
            .field("name", &self.name)
            .field("config", &self.config)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .field("db", &"<VectorDB>")
            .finish()
    }
}

impl Collection {
    /// Create a new collection
    pub fn new(name: String, config: CollectionConfig, storage_path: String) -> Result<Self> {
        // Validate configuration
        config.validate()?;

        // Create VectorDB with the configuration
        let db_options = ruvector_core::types::DbOptions {
            dimensions: config.dimensions,
            distance_metric: config.distance_metric,
            storage_path,
            hnsw_config: config.hnsw_config.clone(),
            quantization: config.quantization.clone(),
        };

        let db = VectorDB::new(db_options)?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        Ok(Self {
            name,
            config,
            db,
            created_at: now,
            updated_at: now,
        })
    }

    /// Get collection statistics
    pub fn stats(&self) -> Result<CollectionStats> {
        let vectors_count = self.db.len()?;

        Ok(CollectionStats {
            vectors_count,
            segments_count: 1,  // Single segment for now
            disk_size_bytes: 0, // TODO: Implement disk size calculation
            ram_size_bytes: 0,  // TODO: Implement RAM size calculation
        })
    }

    /// Update the last modified timestamp
    pub fn touch(&mut self) {
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
    }
}

/// Statistics about a collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionStats {
    /// Number of vectors in the collection
    pub vectors_count: usize,

    /// Number of segments (partitions) in the collection
    pub segments_count: usize,

    /// Total disk space used (bytes)
    pub disk_size_bytes: u64,

    /// Total RAM used (bytes)
    pub ram_size_bytes: u64,
}

impl CollectionStats {
    /// Check if the collection is empty
    pub fn is_empty(&self) -> bool {
        self.vectors_count == 0
    }

    /// Get human-readable disk size
    pub fn disk_size_human(&self) -> String {
        format_bytes(self.disk_size_bytes)
    }

    /// Get human-readable RAM size
    pub fn ram_size_human(&self) -> String {
        format_bytes(self.ram_size_bytes)
    }
}

/// Format bytes into human-readable size
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_config_validation() {
        // Valid config
        let config = CollectionConfig::with_dimensions(384);
        assert!(config.validate().is_ok());

        // Invalid: zero dimensions
        let config = CollectionConfig {
            dimensions: 0,
            distance_metric: DistanceMetric::Cosine,
            hnsw_config: None,
            quantization: None,
            on_disk_payload: true,
        };
        assert!(config.validate().is_err());

        // Invalid: dimensions too large
        let config = CollectionConfig {
            dimensions: 200_000,
            distance_metric: DistanceMetric::Cosine,
            hnsw_config: None,
            quantization: None,
            on_disk_payload: true,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512.00 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }
}
