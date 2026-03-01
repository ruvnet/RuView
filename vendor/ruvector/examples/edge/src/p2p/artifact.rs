//! Artifact Store - Local CID-based Storage
//!
//! Security principles:
//! - LOCAL-ONLY mode by default (no gateway fetch)
//! - Real IPFS CID support requires multiformats (not yet implemented)
//! - LRU cache with configurable size limits
//! - Chunk-based storage for large artifacts

use crate::p2p::crypto::CryptoV2;
use crate::p2p::identity::IdentityManager;
use crate::p2p::envelope::{ArtifactPointer, ArtifactType};
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use lz4_flex;

/// Chunk size for large artifacts (256KB)
pub const CHUNK_SIZE: usize = 256 * 1024;

/// Maximum artifact size (10MB)
pub const MAX_ARTIFACT_SIZE: usize = 10 * 1024 * 1024;

/// Default cache size (100MB)
pub const DEFAULT_CACHE_SIZE: usize = 100 * 1024 * 1024;

/// Stored artifact with metadata
#[derive(Debug, Clone)]
struct StoredArtifact {
    data: Vec<u8>,
    compressed: bool,
    created_at: u64,
}

/// Artifact Store with LRU eviction
pub struct ArtifactStore {
    cache: Arc<RwLock<HashMap<String, StoredArtifact>>>,
    lru_order: Arc<RwLock<VecDeque<String>>>,
    current_size: Arc<RwLock<usize>>,
    max_cache_size: usize,
    max_artifact_size: usize,

    /// If true, allows gateway fetch (requires real IPFS CIDs)
    /// Default: false (local-only mode)
    enable_gateway_fetch: bool,
}

impl ArtifactStore {
    /// Create new artifact store with default settings (local-only)
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            lru_order: Arc::new(RwLock::new(VecDeque::new())),
            current_size: Arc::new(RwLock::new(0)),
            max_cache_size: DEFAULT_CACHE_SIZE,
            max_artifact_size: MAX_ARTIFACT_SIZE,
            enable_gateway_fetch: false, // LOCAL-ONLY by default
        }
    }

    /// Create with custom settings
    pub fn with_config(max_cache_size: usize, max_artifact_size: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            lru_order: Arc::new(RwLock::new(VecDeque::new())),
            current_size: Arc::new(RwLock::new(0)),
            max_cache_size,
            max_artifact_size,
            enable_gateway_fetch: false,
        }
    }

    /// Store artifact and return local CID
    pub fn store(&self, data: &[u8], compress: bool) -> Result<StoredArtifactInfo, String> {
        if data.len() > self.max_artifact_size {
            return Err(format!(
                "Artifact too large: {} > {}",
                data.len(),
                self.max_artifact_size
            ));
        }

        let (stored_data, compressed) = if compress && data.len() > 1024 {
            // Compress if larger than 1KB
            match lz4_flex::compress_prepend_size(data) {
                compressed if compressed.len() < data.len() => (compressed, true),
                _ => (data.to_vec(), false),
            }
        } else {
            (data.to_vec(), false)
        };

        // Generate local CID (NOT a real IPFS CID)
        let cid = CryptoV2::generate_local_cid(data); // Hash of original, not compressed

        let artifact = StoredArtifact {
            data: stored_data.clone(),
            compressed,
            created_at: chrono::Utc::now().timestamp_millis() as u64,
        };

        // Evict if necessary
        self.evict_if_needed(stored_data.len());

        // Store
        {
            let mut cache = self.cache.write();
            cache.insert(cid.clone(), artifact);
        }

        // Update LRU
        {
            let mut lru = self.lru_order.write();
            lru.push_back(cid.clone());
        }

        // Update size
        {
            let mut size = self.current_size.write();
            *size += stored_data.len();
        }

        let chunks = (data.len() + CHUNK_SIZE - 1) / CHUNK_SIZE;

        Ok(StoredArtifactInfo {
            cid,
            original_size: data.len(),
            stored_size: stored_data.len(),
            compressed,
            chunks,
        })
    }

    /// Retrieve artifact by CID (local-only)
    pub fn retrieve(&self, cid: &str) -> Option<Vec<u8>> {
        // Only check local cache in local-only mode
        let cache = self.cache.read();
        let artifact = cache.get(cid)?;

        // Update LRU (move to back)
        {
            let mut lru = self.lru_order.write();
            if let Some(pos) = lru.iter().position(|c| c == cid) {
                lru.remove(pos);
                lru.push_back(cid.to_string());
            }
        }

        // Decompress if needed
        if artifact.compressed {
            lz4_flex::decompress_size_prepended(&artifact.data).ok()
        } else {
            Some(artifact.data.clone())
        }
    }

    /// Check if artifact exists locally
    pub fn exists(&self, cid: &str) -> bool {
        self.cache.read().contains_key(cid)
    }

    /// Delete artifact
    pub fn delete(&self, cid: &str) -> bool {
        let mut cache = self.cache.write();
        if let Some(artifact) = cache.remove(cid) {
            let mut size = self.current_size.write();
            *size = size.saturating_sub(artifact.data.len());

            let mut lru = self.lru_order.write();
            if let Some(pos) = lru.iter().position(|c| c == cid) {
                lru.remove(pos);
            }
            true
        } else {
            false
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> ArtifactStoreStats {
        let cache = self.cache.read();
        ArtifactStoreStats {
            artifact_count: cache.len(),
            current_size: *self.current_size.read(),
            max_size: self.max_cache_size,
            utilization: *self.current_size.read() as f64 / self.max_cache_size as f64,
        }
    }

    /// Evict oldest artifacts if needed
    fn evict_if_needed(&self, new_size: usize) {
        let mut size = self.current_size.write();
        let mut lru = self.lru_order.write();
        let mut cache = self.cache.write();

        while *size + new_size > self.max_cache_size && !lru.is_empty() {
            if let Some(oldest_cid) = lru.pop_front() {
                if let Some(artifact) = cache.remove(&oldest_cid) {
                    *size = size.saturating_sub(artifact.data.len());
                    tracing::debug!("Evicted artifact: {}", oldest_cid);
                }
            }
        }
    }

    /// Create artifact pointer for publishing
    pub fn create_pointer(
        &self,
        artifact_type: ArtifactType,
        agent_id: &str,
        cid: &str,
        dimensions: &str,
        identity: &IdentityManager,
    ) -> Option<ArtifactPointer> {
        let cache = self.cache.read();
        let artifact = cache.get(cid)?;

        let data = if artifact.compressed {
            lz4_flex::decompress_size_prepended(&artifact.data).ok()?
        } else {
            artifact.data.clone()
        };

        // Compute checksum (first 16 bytes of hash)
        let hash = CryptoV2::hash(&data);
        let mut checksum = [0u8; 16];
        checksum.copy_from_slice(&hash[..16]);

        // Schema hash (first 8 bytes of type name hash)
        let type_name = format!("{:?}", artifact_type);
        let type_hash = CryptoV2::hash(type_name.as_bytes());
        let mut schema_hash = [0u8; 8];
        schema_hash.copy_from_slice(&type_hash[..8]);

        let mut pointer = ArtifactPointer {
            artifact_type,
            agent_id: agent_id.to_string(),
            cid: cid.to_string(),
            version: 1,
            schema_hash,
            dimensions: dimensions.to_string(),
            checksum,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            signature: [0u8; 64],
        };

        // Sign the pointer
        let canonical = pointer.canonical_for_signing();
        pointer.signature = identity.sign(canonical.as_bytes());

        Some(pointer)
    }

    /// Clear all artifacts
    pub fn clear(&self) {
        self.cache.write().clear();
        self.lru_order.write().clear();
        *self.current_size.write() = 0;
    }
}

impl Default for ArtifactStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about stored artifact
#[derive(Debug, Clone)]
pub struct StoredArtifactInfo {
    pub cid: String,
    pub original_size: usize,
    pub stored_size: usize,
    pub compressed: bool,
    pub chunks: usize,
}

/// Artifact store statistics
#[derive(Debug, Clone)]
pub struct ArtifactStoreStats {
    pub artifact_count: usize,
    pub current_size: usize,
    pub max_size: usize,
    pub utilization: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_retrieve() {
        let store = ArtifactStore::new();
        let data = b"Hello, World!";

        let info = store.store(data, false).unwrap();
        assert!(info.cid.starts_with("local:"));

        let retrieved = store.retrieve(&info.cid).unwrap();
        assert_eq!(data.as_slice(), retrieved.as_slice());
    }

    #[test]
    fn test_compression() {
        let store = ArtifactStore::new();
        // Create data that compresses well
        let data = vec![0u8; 10000];

        let info = store.store(&data, true).unwrap();
        assert!(info.compressed);
        assert!(info.stored_size < info.original_size);

        let retrieved = store.retrieve(&info.cid).unwrap();
        assert_eq!(data, retrieved);
    }

    #[test]
    fn test_size_limit() {
        // Set max_artifact_size to 100 bytes
        let store = ArtifactStore::with_config(1024, 100);

        // Store artifact too large should fail
        let data = vec![0u8; 200];
        let result = store.store(&data, false);
        assert!(result.is_err()); // Should fail due to max_artifact_size

        // Small artifact should succeed
        let small_data = vec![0u8; 50];
        let result = store.store(&small_data, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_eviction() {
        // Small cache for testing eviction
        let store = ArtifactStore::with_config(200, 100);

        // Store multiple small artifacts
        let info1 = store.store(b"artifact 1", false).unwrap();
        let info2 = store.store(b"artifact 2", false).unwrap();
        let info3 = store.store(b"artifact 3", false).unwrap();

        // All should be retrievable (cache is large enough)
        assert!(store.exists(&info1.cid));
        assert!(store.exists(&info2.cid));
        assert!(store.exists(&info3.cid));
    }

    #[test]
    fn test_local_only_mode() {
        let store = ArtifactStore::new();

        // Non-existent CID should return None (no gateway fetch)
        let result = store.retrieve("local:nonexistent");
        assert!(result.is_none());
    }
}
