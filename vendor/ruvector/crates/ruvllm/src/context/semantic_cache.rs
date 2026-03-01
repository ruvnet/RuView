//! Semantic Tool Cache - Caches tool results with similarity-based retrieval
//!
//! Provides intelligent caching of tool execution results using HNSW-indexed
//! embeddings for semantic similarity matching.

use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
use ruvector_core::index::hnsw::HnswIndex;
use ruvector_core::index::VectorIndex;
use ruvector_core::types::{DistanceMetric, HnswConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::error::{Result, RuvLLMError};

/// Configuration for semantic tool cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticCacheConfig {
    /// Embedding dimension
    pub embedding_dim: usize,
    /// Maximum cached entries
    pub max_entries: usize,
    /// Similarity threshold for cache hits (0.0 - 1.0)
    pub similarity_threshold: f32,
    /// Default TTL in seconds
    pub default_ttl_seconds: i64,
    /// HNSW M parameter
    pub hnsw_m: usize,
    /// HNSW ef_construction parameter
    pub hnsw_ef_construction: usize,
    /// HNSW ef_search parameter
    pub hnsw_ef_search: usize,
    /// Enable LRU eviction
    pub enable_lru: bool,
}

impl Default for SemanticCacheConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 768,
            max_entries: 1_000,
            similarity_threshold: 0.85,
            default_ttl_seconds: 3600, // 1 hour
            hnsw_m: 16,
            hnsw_ef_construction: 100,
            hnsw_ef_search: 50,
            enable_lru: true,
        }
    }
}

/// A cached tool result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedToolResult {
    /// Cache entry ID
    pub id: String,
    /// Tool name
    pub tool_name: String,
    /// Input hash for exact matching
    pub input_hash: String,
    /// Input embedding for similarity matching
    pub embedding: Vec<f32>,
    /// Tool result
    pub result: String,
    /// Success status
    pub success: bool,
    /// Similarity score (1.0 for exact match)
    pub similarity: f32,
    /// Access count
    pub access_count: u64,
    /// Cached timestamp
    pub cached_at: DateTime<Utc>,
    /// Last accessed timestamp
    pub last_accessed: DateTime<Utc>,
    /// Time-to-live
    pub ttl: Duration,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total entries
    pub total_entries: usize,
    /// Total lookups
    pub total_lookups: u64,
    /// Cache hits
    pub hits: u64,
    /// Cache misses
    pub misses: u64,
    /// Hit rate (0.0 - 1.0)
    pub hit_rate: f32,
    /// Exact matches (hash-based)
    pub exact_matches: u64,
    /// Semantic matches (embedding-based)
    pub semantic_matches: u64,
    /// Evictions
    pub evictions: u64,
    /// Expirations
    pub expirations: u64,
}

/// Internal statistics tracking
#[derive(Debug, Default)]
struct StatsInternal {
    lookups: AtomicU64,
    hits: AtomicU64,
    misses: AtomicU64,
    exact_matches: AtomicU64,
    semantic_matches: AtomicU64,
    evictions: AtomicU64,
    expirations: AtomicU64,
}

/// Semantic tool cache with HNSW indexing
pub struct SemanticToolCache {
    /// Configuration
    config: SemanticCacheConfig,
    /// HNSW index for similarity search
    index: Arc<RwLock<HnswIndex>>,
    /// Cache storage
    cache: Arc<RwLock<HashMap<String, CachedToolResult>>>,
    /// Hash to ID mapping for exact matches
    hash_index: Arc<RwLock<HashMap<String, String>>>,
    /// Statistics
    stats: StatsInternal,
}

impl SemanticToolCache {
    /// Create new semantic cache with configuration
    pub fn new(config: SemanticCacheConfig) -> Result<Self> {
        let hnsw_config = HnswConfig {
            m: config.hnsw_m,
            ef_construction: config.hnsw_ef_construction,
            ef_search: config.hnsw_ef_search,
            max_elements: config.max_entries,
        };

        let index = HnswIndex::new(config.embedding_dim, DistanceMetric::Cosine, hnsw_config)
            .map_err(|e| RuvLLMError::Ruvector(e.to_string()))?;

        Ok(Self {
            config,
            index: Arc::new(RwLock::new(index)),
            cache: Arc::new(RwLock::new(HashMap::new())),
            hash_index: Arc::new(RwLock::new(HashMap::new())),
            stats: StatsInternal::default(),
        })
    }

    /// Store a tool result in cache
    pub fn store(
        &self,
        tool_name: &str,
        input: &str,
        result: &str,
        embedding: Vec<f32>,
    ) -> Result<()> {
        self.store_with_options(
            tool_name,
            input,
            result,
            embedding,
            true,
            Duration::seconds(self.config.default_ttl_seconds),
            HashMap::new(),
        )
    }

    /// Store with custom options
    pub fn store_with_options(
        &self,
        tool_name: &str,
        input: &str,
        result: &str,
        embedding: Vec<f32>,
        success: bool,
        ttl: Duration,
        metadata: HashMap<String, String>,
    ) -> Result<()> {
        let input_hash = format!("{:x}", md5::compute(input));
        let id = format!("{}:{}", tool_name, uuid::Uuid::new_v4());
        let now = Utc::now();

        let entry = CachedToolResult {
            id: id.clone(),
            tool_name: tool_name.to_string(),
            input_hash: input_hash.clone(),
            embedding: embedding.clone(),
            result: result.to_string(),
            success,
            similarity: 1.0, // Exact match for stored entry
            access_count: 0,
            cached_at: now,
            last_accessed: now,
            ttl,
            metadata,
        };

        // Add to HNSW index
        {
            let mut index = self.index.write();
            index.add(id.clone(), embedding)?;
        }

        // Store entry
        {
            let mut cache = self.cache.write();
            cache.insert(id.clone(), entry);
        }

        // Update hash index
        {
            let mut hash_idx = self.hash_index.write();
            hash_idx.insert(input_hash, id);
        }

        // Enforce limit
        self.enforce_limit()?;

        Ok(())
    }

    /// Get cached result by embedding similarity
    pub fn get(&self, query_embedding: &[f32]) -> Result<Option<CachedToolResult>> {
        self.stats.lookups.fetch_add(1, Ordering::SeqCst);

        // Search for similar entries
        let results = {
            let index = self.index.read();
            index.search(query_embedding, 1)?
        };

        if results.is_empty() {
            self.stats.misses.fetch_add(1, Ordering::SeqCst);
            return Ok(None);
        }

        let best = &results[0];
        let similarity = 1.0 - best.score; // Convert distance to similarity

        if similarity < self.config.similarity_threshold {
            self.stats.misses.fetch_add(1, Ordering::SeqCst);
            return Ok(None);
        }

        // Get the entry
        let mut cache = self.cache.write();
        if let Some(entry) = cache.get_mut(&best.id) {
            // Check TTL
            if Utc::now() - entry.cached_at > entry.ttl {
                // Expired
                self.stats.expirations.fetch_add(1, Ordering::SeqCst);
                self.stats.misses.fetch_add(1, Ordering::SeqCst);

                // Remove expired entry
                let id = entry.id.clone();
                drop(cache);
                self.remove(&id)?;
                return Ok(None);
            }

            // Update access stats
            entry.access_count += 1;
            entry.last_accessed = Utc::now();
            entry.similarity = similarity;

            self.stats.hits.fetch_add(1, Ordering::SeqCst);
            self.stats.semantic_matches.fetch_add(1, Ordering::SeqCst);

            return Ok(Some(entry.clone()));
        }

        self.stats.misses.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }

    /// Get by exact input hash
    pub fn get_exact(&self, tool_name: &str, input: &str) -> Result<Option<CachedToolResult>> {
        self.stats.lookups.fetch_add(1, Ordering::SeqCst);

        let input_hash = format!("{:x}", md5::compute(input));

        // Look up by hash
        let id = {
            let hash_idx = self.hash_index.read();
            hash_idx.get(&input_hash).cloned()
        };

        if let Some(id) = id {
            let mut cache = self.cache.write();
            if let Some(entry) = cache.get_mut(&id) {
                // Verify tool name
                if entry.tool_name != tool_name {
                    self.stats.misses.fetch_add(1, Ordering::SeqCst);
                    return Ok(None);
                }

                // Check TTL
                if Utc::now() - entry.cached_at > entry.ttl {
                    self.stats.expirations.fetch_add(1, Ordering::SeqCst);
                    self.stats.misses.fetch_add(1, Ordering::SeqCst);

                    let id = entry.id.clone();
                    drop(cache);
                    self.remove(&id)?;
                    return Ok(None);
                }

                // Update access stats
                entry.access_count += 1;
                entry.last_accessed = Utc::now();
                entry.similarity = 1.0; // Exact match

                self.stats.hits.fetch_add(1, Ordering::SeqCst);
                self.stats.exact_matches.fetch_add(1, Ordering::SeqCst);

                return Ok(Some(entry.clone()));
            }
        }

        self.stats.misses.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }

    /// Get or execute - returns cached result or executes function
    pub fn get_or_execute<F, E>(
        &self,
        tool_name: &str,
        input: &str,
        embedding: Vec<f32>,
        execute: F,
    ) -> std::result::Result<String, E>
    where
        F: FnOnce() -> std::result::Result<String, E>,
        E: std::fmt::Debug,
    {
        // Try exact match first
        if let Ok(Some(cached)) = self.get_exact(tool_name, input) {
            return Ok(cached.result);
        }

        // Try semantic match
        if let Ok(Some(cached)) = self.get(&embedding) {
            if cached.tool_name == tool_name {
                return Ok(cached.result);
            }
        }

        // Execute and cache
        let result = execute()?;

        // Store result (ignore errors)
        let _ = self.store(tool_name, input, &result, embedding);

        Ok(result)
    }

    /// Remove entry by ID
    pub fn remove(&self, id: &str) -> Result<bool> {
        let entry = {
            let mut cache = self.cache.write();
            cache.remove(id)
        };

        if let Some(entry) = entry {
            // Remove from hash index
            {
                let mut hash_idx = self.hash_index.write();
                hash_idx.remove(&entry.input_hash);
            }

            // Remove from HNSW index
            {
                let mut index = self.index.write();
                let _ = index.remove(&id.to_string());
            }

            return Ok(true);
        }

        Ok(false)
    }

    /// Invalidate entries by tool name
    pub fn invalidate_tool(&self, tool_name: &str) -> Result<usize> {
        let to_remove: Vec<String> = {
            let cache = self.cache.read();
            cache
                .iter()
                .filter(|(_, e)| e.tool_name == tool_name)
                .map(|(id, _)| id.clone())
                .collect()
        };

        let count = to_remove.len();
        for id in to_remove {
            self.remove(&id)?;
        }

        Ok(count)
    }

    /// Clean expired entries
    pub fn clean_expired(&self) -> Result<usize> {
        let now = Utc::now();
        let expired: Vec<String> = {
            let cache = self.cache.read();
            cache
                .iter()
                .filter(|(_, e)| now - e.cached_at > e.ttl)
                .map(|(id, _)| id.clone())
                .collect()
        };

        let count = expired.len();
        for id in expired {
            self.remove(&id)?;
            self.stats.expirations.fetch_add(1, Ordering::SeqCst);
        }

        Ok(count)
    }

    /// Enforce storage limit
    fn enforce_limit(&self) -> Result<()> {
        let cache = self.cache.read();

        if cache.len() <= self.config.max_entries {
            return Ok(());
        }

        drop(cache);

        if self.config.enable_lru {
            // Remove least recently accessed
            let to_remove: Option<String> = {
                let cache = self.cache.read();
                cache
                    .iter()
                    .min_by_key(|(_, e)| e.last_accessed)
                    .map(|(id, _)| id.clone())
            };

            if let Some(id) = to_remove {
                self.remove(&id)?;
                self.stats.evictions.fetch_add(1, Ordering::SeqCst);
            }
        } else {
            // Remove oldest
            let to_remove: Option<String> = {
                let cache = self.cache.read();
                cache
                    .iter()
                    .min_by_key(|(_, e)| e.cached_at)
                    .map(|(id, _)| id.clone())
            };

            if let Some(id) = to_remove {
                self.remove(&id)?;
                self.stats.evictions.fetch_add(1, Ordering::SeqCst);
            }
        }

        Ok(())
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let total = self.cache.read().len();
        let lookups = self.stats.lookups.load(Ordering::SeqCst);
        let hits = self.stats.hits.load(Ordering::SeqCst);
        let misses = self.stats.misses.load(Ordering::SeqCst);

        CacheStats {
            total_entries: total,
            total_lookups: lookups,
            hits,
            misses,
            hit_rate: if lookups > 0 {
                hits as f32 / lookups as f32
            } else {
                0.0
            },
            exact_matches: self.stats.exact_matches.load(Ordering::SeqCst),
            semantic_matches: self.stats.semantic_matches.load(Ordering::SeqCst),
            evictions: self.stats.evictions.load(Ordering::SeqCst),
            expirations: self.stats.expirations.load(Ordering::SeqCst),
        }
    }

    /// Clear all entries
    pub fn clear(&self) -> Result<()> {
        self.cache.write().clear();
        self.hash_index.write().clear();

        // Recreate index
        let hnsw_config = HnswConfig {
            m: self.config.hnsw_m,
            ef_construction: self.config.hnsw_ef_construction,
            ef_search: self.config.hnsw_ef_search,
            max_elements: self.config.max_entries,
        };

        *self.index.write() = HnswIndex::new(
            self.config.embedding_dim,
            DistanceMetric::Cosine,
            hnsw_config,
        )
        .map_err(|e| RuvLLMError::Ruvector(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_embedding(dim: usize) -> Vec<f32> {
        vec![0.1; dim]
    }

    #[test]
    fn test_cache_creation() {
        let config = SemanticCacheConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let cache = SemanticToolCache::new(config).unwrap();
        assert_eq!(cache.stats().total_entries, 0);
    }

    #[test]
    fn test_store_and_get_exact() {
        let config = SemanticCacheConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let cache = SemanticToolCache::new(config).unwrap();

        let embedding = test_embedding(128);
        cache
            .store("read_file", "/path/to/file.rs", "file contents", embedding)
            .unwrap();

        let result = cache.get_exact("read_file", "/path/to/file.rs").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().result, "file contents");

        // Different input should not match
        let result = cache.get_exact("read_file", "/other/file.rs").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_store_and_get_semantic() {
        let config = SemanticCacheConfig {
            embedding_dim: 128,
            similarity_threshold: 0.8,
            ..Default::default()
        };
        let cache = SemanticToolCache::new(config).unwrap();

        let embedding = test_embedding(128);
        cache
            .store(
                "read_file",
                "/path/to/file.rs",
                "file contents",
                embedding.clone(),
            )
            .unwrap();

        // Same embedding should match
        let result = cache.get(&embedding).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().result, "file contents");
    }

    #[test]
    fn test_get_or_execute() {
        let config = SemanticCacheConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let cache = SemanticToolCache::new(config).unwrap();

        let embedding = test_embedding(128);

        // First call should execute
        let result: std::result::Result<String, &str> =
            cache.get_or_execute("test_tool", "input", embedding.clone(), || {
                Ok("executed".to_string())
            });
        assert_eq!(result.unwrap(), "executed");

        // Second call should return cached
        let result: std::result::Result<String, &str> =
            cache.get_or_execute("test_tool", "input", embedding, || {
                Ok("should not execute".to_string())
            });
        assert_eq!(result.unwrap(), "executed");
    }

    #[test]
    fn test_invalidate_tool() {
        let config = SemanticCacheConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let cache = SemanticToolCache::new(config).unwrap();

        let embedding = test_embedding(128);
        cache
            .store("tool_a", "input1", "result1", embedding.clone())
            .unwrap();
        cache
            .store("tool_b", "input2", "result2", embedding.clone())
            .unwrap();

        assert_eq!(cache.stats().total_entries, 2);

        let removed = cache.invalidate_tool("tool_a").unwrap();
        assert_eq!(removed, 1);
        assert_eq!(cache.stats().total_entries, 1);
    }

    #[test]
    fn test_stats() {
        let config = SemanticCacheConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let cache = SemanticToolCache::new(config).unwrap();

        let embedding = test_embedding(128);
        cache
            .store("tool", "input", "result", embedding.clone())
            .unwrap();

        // Hit
        cache.get_exact("tool", "input").unwrap();

        // Miss
        cache.get_exact("tool", "other").unwrap();

        let stats = cache.stats();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.total_lookups, 2);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_clear() {
        let config = SemanticCacheConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let cache = SemanticToolCache::new(config).unwrap();

        let embedding = test_embedding(128);
        cache.store("tool", "input", "result", embedding).unwrap();

        assert_eq!(cache.stats().total_entries, 1);
        cache.clear().unwrap();
        assert_eq!(cache.stats().total_entries, 0);
    }
}
