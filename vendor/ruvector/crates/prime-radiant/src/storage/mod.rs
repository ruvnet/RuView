//! # Storage Layer Module
//!
//! Hybrid storage with PostgreSQL for transactional authority and ruvector for
//! high-performance vector and graph queries.
//!
//! ## Architecture
//!
//! ```text
//! +----------------------------------------------+
//! |                Storage Layer                  |
//! +----------------------------------------------+
//! |                                              |
//! |  +------------------+  +------------------+  |
//! |  |   PostgreSQL     |  |    ruvector      |  |
//! |  |   (Authority)    |  |  (Graph/Vector)  |  |
//! |  |                  |  |                  |  |
//! |  | - Policy bundles |  | - Node states    |  |
//! |  | - Witnesses      |  | - Edge data      |  |
//! |  | - Lineage        |  | - HNSW index     |  |
//! |  | - Event log      |  | - Residual cache |  |
//! |  +------------------+  +------------------+  |
//! |                                              |
//! +----------------------------------------------+
//! ```
//!
//! ## Storage Backends
//!
//! | Backend | Use Case | Features |
//! |---------|----------|----------|
//! | `InMemoryStorage` | Testing, Development | Thread-safe, fast, no persistence |
//! | `FileStorage` | Embedded, Edge | WAL, JSON/bincode, persistence |
//! | `PostgresStorage` | Production | ACID, indexes, concurrent access |
//!
//! ## Usage
//!
//! ```rust,ignore
//! use prime_radiant::storage::{
//!     InMemoryStorage, FileStorage, GraphStorage, GovernanceStorage,
//! };
//!
//! // In-memory for testing
//! let memory_storage = InMemoryStorage::new();
//! memory_storage.store_node("node-1", &[1.0, 0.0, 0.0])?;
//!
//! // File-based for persistence
//! let file_storage = FileStorage::new("./data")?;
//! file_storage.store_node("node-1", &[1.0, 0.0, 0.0])?;
//!
//! // PostgreSQL for production (feature-gated)
//! #[cfg(feature = "postgres")]
//! let pg_storage = PostgresStorage::connect("postgresql://localhost/db").await?;
//! ```

// Module declarations
mod file;
mod memory;

#[cfg(feature = "postgres")]
#[cfg_attr(docsrs, doc(cfg(feature = "postgres")))]
mod postgres;

// Re-exports
pub use file::{FileStorage, StorageFormat, StorageMetadata, StorageStats, WalEntry, WalOperation};
pub use memory::{InMemoryStorage, IndexedInMemoryStorage, StorageEvent, StorageEventType};

#[cfg(feature = "postgres")]
pub use postgres::{
    AsyncGraphStorageAdapter, EdgeRow, EventLogEntry, LineageRecordRow, NodeStateRow,
    PolicyBundleRow, PostgresConfig, PostgresStats, PostgresStorage, WitnessRecordRow,
};

use serde::{Deserialize, Serialize};

/// Storage configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// PostgreSQL connection string (optional).
    pub postgres_url: Option<String>,
    /// Path for local graph storage.
    pub graph_path: String,
    /// Path for event log.
    pub event_log_path: String,
    /// Enable write-ahead logging.
    pub enable_wal: bool,
    /// Cache size in MB.
    pub cache_size_mb: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            postgres_url: None,
            graph_path: "./data/graph".to_string(),
            event_log_path: "./data/events".to_string(),
            enable_wal: true,
            cache_size_mb: 256,
        }
    }
}

impl StorageConfig {
    /// Create a configuration for in-memory storage only.
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            postgres_url: None,
            graph_path: String::new(),
            event_log_path: String::new(),
            enable_wal: false,
            cache_size_mb: 256,
        }
    }

    /// Create a configuration for file-based storage.
    #[must_use]
    pub fn file_based(path: impl Into<String>) -> Self {
        let path = path.into();
        Self {
            postgres_url: None,
            graph_path: path.clone(),
            event_log_path: format!("{}/events", path),
            enable_wal: true,
            cache_size_mb: 256,
        }
    }

    /// Create a configuration for PostgreSQL storage.
    #[must_use]
    pub fn postgres(url: impl Into<String>) -> Self {
        Self {
            postgres_url: Some(url.into()),
            graph_path: "./data/graph".to_string(),
            event_log_path: "./data/events".to_string(),
            enable_wal: false,
            cache_size_mb: 256,
        }
    }

    /// Set the cache size.
    #[must_use]
    pub const fn with_cache_size(mut self, size_mb: usize) -> Self {
        self.cache_size_mb = size_mb;
        self
    }

    /// Enable or disable WAL.
    #[must_use]
    pub const fn with_wal(mut self, enable: bool) -> Self {
        self.enable_wal = enable;
        self
    }
}

/// Storage backend trait for graph operations.
///
/// This trait defines the interface for storing and retrieving graph data
/// including node states and edges. Implementations must be thread-safe.
pub trait GraphStorage: Send + Sync {
    /// Store a node state.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Unique identifier for the node
    /// * `state` - State vector (typically f32 values representing the node's state)
    ///
    /// # Errors
    ///
    /// Returns error if the storage operation fails.
    fn store_node(&self, node_id: &str, state: &[f32]) -> Result<(), StorageError>;

    /// Retrieve a node state.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Unique identifier for the node
    ///
    /// # Returns
    ///
    /// `Some(state)` if the node exists, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns error if the storage operation fails.
    fn get_node(&self, node_id: &str) -> Result<Option<Vec<f32>>, StorageError>;

    /// Store an edge between two nodes.
    ///
    /// # Arguments
    ///
    /// * `source` - Source node ID
    /// * `target` - Target node ID
    /// * `weight` - Edge weight (typically representing constraint strength)
    ///
    /// # Errors
    ///
    /// Returns error if the storage operation fails.
    fn store_edge(&self, source: &str, target: &str, weight: f32) -> Result<(), StorageError>;

    /// Delete an edge between two nodes.
    ///
    /// # Arguments
    ///
    /// * `source` - Source node ID
    /// * `target` - Target node ID
    ///
    /// # Errors
    ///
    /// Returns error if the storage operation fails.
    fn delete_edge(&self, source: &str, target: &str) -> Result<(), StorageError>;

    /// Find nodes similar to a query vector.
    ///
    /// This method performs approximate nearest neighbor search using cosine similarity.
    /// For production workloads with large datasets, consider using HNSW-indexed storage.
    ///
    /// # Arguments
    ///
    /// * `query` - Query vector to search for similar nodes
    /// * `k` - Maximum number of results to return
    ///
    /// # Returns
    ///
    /// Vector of (node_id, similarity_score) tuples, sorted by similarity descending.
    ///
    /// # Errors
    ///
    /// Returns error if the search operation fails.
    fn find_similar(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>, StorageError>;
}

/// Storage backend trait for governance data.
///
/// This trait defines the interface for storing and retrieving governance objects
/// including policy bundles, witness records, and lineage records.
pub trait GovernanceStorage: Send + Sync {
    /// Store a policy bundle.
    ///
    /// # Arguments
    ///
    /// * `bundle` - Serialized policy bundle data
    ///
    /// # Returns
    ///
    /// Unique identifier for the stored bundle.
    ///
    /// # Errors
    ///
    /// Returns error if the storage operation fails.
    fn store_policy(&self, bundle: &[u8]) -> Result<String, StorageError>;

    /// Retrieve a policy bundle.
    ///
    /// # Arguments
    ///
    /// * `id` - Policy bundle identifier
    ///
    /// # Returns
    ///
    /// `Some(data)` if the policy exists, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns error if the storage operation fails.
    fn get_policy(&self, id: &str) -> Result<Option<Vec<u8>>, StorageError>;

    /// Store a witness record.
    ///
    /// Witness records provide immutable proof of gate decisions.
    ///
    /// # Arguments
    ///
    /// * `witness` - Serialized witness record data
    ///
    /// # Returns
    ///
    /// Unique identifier for the stored witness.
    ///
    /// # Errors
    ///
    /// Returns error if the storage operation fails.
    fn store_witness(&self, witness: &[u8]) -> Result<String, StorageError>;

    /// Retrieve witness records for an action.
    ///
    /// # Arguments
    ///
    /// * `action_id` - Action identifier to search for
    ///
    /// # Returns
    ///
    /// Vector of witness record data for the given action.
    ///
    /// # Errors
    ///
    /// Returns error if the search operation fails.
    fn get_witnesses_for_action(&self, action_id: &str) -> Result<Vec<Vec<u8>>, StorageError>;

    /// Store a lineage record.
    ///
    /// Lineage records track provenance for authoritative writes.
    ///
    /// # Arguments
    ///
    /// * `lineage` - Serialized lineage record data
    ///
    /// # Returns
    ///
    /// Unique identifier for the stored lineage.
    ///
    /// # Errors
    ///
    /// Returns error if the storage operation fails.
    fn store_lineage(&self, lineage: &[u8]) -> Result<String, StorageError>;
}

/// Storage error type.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// Connection error (database or file system)
    #[error("Connection error: {0}")]
    Connection(String),

    /// Entity not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid data format or content
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Transaction or operation failed
    #[error("Transaction failed: {0}")]
    Transaction(String),

    /// Integrity constraint violation
    #[error("Integrity violation: {0}")]
    IntegrityViolation(String),

    /// Resource exhausted (e.g., disk space)
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

/// Hybrid storage that combines multiple backends.
///
/// Uses file storage for graph data and optionally PostgreSQL for governance data.
/// This provides the best of both worlds: fast local access for frequently accessed
/// data and ACID guarantees for critical governance data.
#[derive(Debug)]
pub struct HybridStorage {
    /// File storage for graph data
    file_storage: FileStorage,
    /// Configuration
    config: StorageConfig,
}

impl HybridStorage {
    /// Create a new hybrid storage instance.
    ///
    /// # Errors
    ///
    /// Returns error if file storage cannot be initialized.
    pub fn new(config: StorageConfig) -> Result<Self, StorageError> {
        let file_storage = FileStorage::from_config(&config)?;

        Ok(Self {
            file_storage,
            config,
        })
    }

    /// Get the file storage backend.
    #[must_use]
    pub fn file_storage(&self) -> &FileStorage {
        &self.file_storage
    }

    /// Get the configuration.
    #[must_use]
    pub fn config(&self) -> &StorageConfig {
        &self.config
    }

    /// Check if PostgreSQL is configured.
    #[must_use]
    pub fn has_postgres(&self) -> bool {
        self.config.postgres_url.is_some()
    }

    /// Sync all storage backends.
    ///
    /// # Errors
    ///
    /// Returns error if sync fails.
    pub fn sync(&self) -> Result<(), StorageError> {
        self.file_storage.sync()
    }
}

impl GraphStorage for HybridStorage {
    fn store_node(&self, node_id: &str, state: &[f32]) -> Result<(), StorageError> {
        self.file_storage.store_node(node_id, state)
    }

    fn get_node(&self, node_id: &str) -> Result<Option<Vec<f32>>, StorageError> {
        self.file_storage.get_node(node_id)
    }

    fn store_edge(&self, source: &str, target: &str, weight: f32) -> Result<(), StorageError> {
        self.file_storage.store_edge(source, target, weight)
    }

    fn delete_edge(&self, source: &str, target: &str) -> Result<(), StorageError> {
        self.file_storage.delete_edge(source, target)
    }

    fn find_similar(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>, StorageError> {
        self.file_storage.find_similar(query, k)
    }
}

impl GovernanceStorage for HybridStorage {
    fn store_policy(&self, bundle: &[u8]) -> Result<String, StorageError> {
        // For now, use file storage. In production, this would delegate to PostgreSQL.
        self.file_storage.store_policy(bundle)
    }

    fn get_policy(&self, id: &str) -> Result<Option<Vec<u8>>, StorageError> {
        self.file_storage.get_policy(id)
    }

    fn store_witness(&self, witness: &[u8]) -> Result<String, StorageError> {
        self.file_storage.store_witness(witness)
    }

    fn get_witnesses_for_action(&self, action_id: &str) -> Result<Vec<Vec<u8>>, StorageError> {
        self.file_storage.get_witnesses_for_action(action_id)
    }

    fn store_lineage(&self, lineage: &[u8]) -> Result<String, StorageError> {
        self.file_storage.store_lineage(lineage)
    }
}

/// Factory for creating storage instances based on configuration.
pub struct StorageFactory;

impl StorageFactory {
    /// Create a storage instance based on configuration.
    ///
    /// # Errors
    ///
    /// Returns error if storage cannot be created.
    pub fn create_graph_storage(
        config: &StorageConfig,
    ) -> Result<Box<dyn GraphStorage>, StorageError> {
        if config.graph_path.is_empty() {
            Ok(Box::new(InMemoryStorage::new()))
        } else {
            Ok(Box::new(FileStorage::from_config(config)?))
        }
    }

    /// Create a governance storage instance.
    ///
    /// # Errors
    ///
    /// Returns error if storage cannot be created.
    pub fn create_governance_storage(
        config: &StorageConfig,
    ) -> Result<Box<dyn GovernanceStorage>, StorageError> {
        if config.graph_path.is_empty() {
            Ok(Box::new(InMemoryStorage::new()))
        } else {
            Ok(Box::new(FileStorage::from_config(config)?))
        }
    }

    /// Create an in-memory storage (convenience method).
    #[must_use]
    pub fn in_memory() -> InMemoryStorage {
        InMemoryStorage::new()
    }

    /// Create a file storage (convenience method).
    ///
    /// # Errors
    ///
    /// Returns error if storage cannot be created.
    pub fn file(path: impl AsRef<std::path::Path>) -> Result<FileStorage, StorageError> {
        FileStorage::new(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_storage_config_builders() {
        let config = StorageConfig::in_memory();
        assert!(config.graph_path.is_empty());
        assert!(!config.enable_wal);

        let config = StorageConfig::file_based("/tmp/test");
        assert_eq!(config.graph_path, "/tmp/test");
        assert!(config.enable_wal);

        let config = StorageConfig::postgres("postgresql://localhost/db");
        assert!(config.postgres_url.is_some());
    }

    #[test]
    fn test_storage_factory_in_memory() {
        let config = StorageConfig::in_memory();
        let storage = StorageFactory::create_graph_storage(&config).unwrap();

        storage.store_node("test", &[1.0, 2.0]).unwrap();
        let state = storage.get_node("test").unwrap();
        assert!(state.is_some());
    }

    #[test]
    fn test_storage_factory_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig::file_based(temp_dir.path().to_str().unwrap());
        let storage = StorageFactory::create_graph_storage(&config).unwrap();

        storage.store_node("test", &[1.0, 2.0]).unwrap();
        let state = storage.get_node("test").unwrap();
        assert!(state.is_some());
    }

    #[test]
    fn test_hybrid_storage() {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig::file_based(temp_dir.path().to_str().unwrap());
        let storage = HybridStorage::new(config).unwrap();

        // Graph operations
        storage.store_node("node-1", &[1.0, 0.0, 0.0]).unwrap();
        let state = storage.get_node("node-1").unwrap();
        assert!(state.is_some());

        // Governance operations
        let policy_id = storage.store_policy(b"test policy").unwrap();
        let policy = storage.get_policy(&policy_id).unwrap();
        assert!(policy.is_some());

        storage.sync().unwrap();
    }

    #[test]
    fn test_trait_object_usage() {
        // Verify that storage types can be used as trait objects
        let memory: Box<dyn GraphStorage> = Box::new(InMemoryStorage::new());
        memory.store_node("test", &[1.0]).unwrap();

        let memory: Box<dyn GovernanceStorage> = Box::new(InMemoryStorage::new());
        let _ = memory.store_policy(b"test").unwrap();
    }
}
