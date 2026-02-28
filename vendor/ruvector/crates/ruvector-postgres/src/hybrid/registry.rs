//! Hybrid Collections Registry
//!
//! Tracks collections with hybrid search enabled and stores:
//! - BM25 corpus statistics
//! - Per-collection fusion settings
//! - Column mappings for vector and FTS

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::bm25::{BM25Config, CorpusStats};
use super::fusion::FusionConfig;
#[cfg(test)]
use super::fusion::FusionMethod;

/// Hybrid collection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridCollectionConfig {
    /// Collection ID (from ruvector.collections table)
    pub collection_id: i32,
    /// Table name
    pub table_name: String,
    /// Schema name (default: public)
    pub schema_name: String,
    /// Vector column name
    pub vector_column: String,
    /// FTS tsvector column name
    pub fts_column: String,
    /// Original text column name (for BM25 stats)
    pub text_column: String,
    /// Primary key column name
    pub pk_column: String,

    /// BM25 configuration
    pub bm25_config: BM25Config,
    /// Fusion configuration
    pub fusion_config: FusionConfig,
    /// Corpus statistics
    pub corpus_stats: CorpusStats,

    /// Prefetch size for each branch
    pub prefetch_k: usize,
    /// Stats refresh interval in seconds
    pub stats_refresh_interval: i64,
    /// Enable parallel branch execution
    pub parallel_enabled: bool,

    /// Created timestamp (Unix epoch)
    pub created_at: i64,
    /// Last modified timestamp
    pub updated_at: i64,
}

impl HybridCollectionConfig {
    /// Create a new hybrid collection configuration
    pub fn new(
        collection_id: i32,
        table_name: String,
        vector_column: String,
        fts_column: String,
        text_column: String,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        Self {
            collection_id,
            table_name,
            schema_name: "public".to_string(),
            vector_column,
            fts_column,
            text_column,
            pk_column: "id".to_string(),
            bm25_config: BM25Config::default(),
            fusion_config: FusionConfig::default(),
            corpus_stats: CorpusStats::default(),
            prefetch_k: 100,
            stats_refresh_interval: 3600, // 1 hour
            parallel_enabled: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// Get fully qualified table name
    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.schema_name, self.table_name)
    }

    /// Check if stats need refresh
    pub fn needs_stats_refresh(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        now - self.corpus_stats.last_update > self.stats_refresh_interval
    }

    /// Update corpus statistics
    pub fn update_stats(&mut self, stats: CorpusStats) {
        self.corpus_stats = stats;
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
    }
}

/// Registry entry for a hybrid collection
#[derive(Debug)]
struct RegistryEntry {
    /// Configuration
    config: HybridCollectionConfig,
    /// Cached IDF values (term -> idf)
    idf_cache: HashMap<String, f32>,
    /// Document frequency cache (term -> doc count)
    df_cache: HashMap<String, u64>,
}

impl RegistryEntry {
    fn new(config: HybridCollectionConfig) -> Self {
        Self {
            config,
            idf_cache: HashMap::new(),
            df_cache: HashMap::new(),
        }
    }
}

/// Hybrid Collections Registry
///
/// Global registry for hybrid-enabled collections.
/// In the PostgreSQL extension, this is backed by the ruvector.hybrid_collections table.
pub struct HybridRegistry {
    /// Collections by ID
    collections_by_id: RwLock<HashMap<i32, RegistryEntry>>,
    /// Collections by name (schema.table -> id)
    collections_by_name: RwLock<HashMap<String, i32>>,
}

impl HybridRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            collections_by_id: RwLock::new(HashMap::new()),
            collections_by_name: RwLock::new(HashMap::new()),
        }
    }

    /// Register a collection for hybrid search
    pub fn register(&self, config: HybridCollectionConfig) -> Result<(), RegistryError> {
        let qualified_name = config.qualified_name();
        let collection_id = config.collection_id;

        // Check for duplicates
        {
            let by_name = self.collections_by_name.read();
            if by_name.contains_key(&qualified_name) {
                return Err(RegistryError::AlreadyRegistered(qualified_name));
            }
        }

        // Insert into both maps
        let entry = RegistryEntry::new(config);

        self.collections_by_id.write().insert(collection_id, entry);
        self.collections_by_name
            .write()
            .insert(qualified_name, collection_id);

        Ok(())
    }

    /// Unregister a collection
    pub fn unregister(&self, collection_id: i32) -> Result<(), RegistryError> {
        let entry = self.collections_by_id.write().remove(&collection_id);

        if let Some(entry) = entry {
            let qualified_name = entry.config.qualified_name();
            self.collections_by_name.write().remove(&qualified_name);
            Ok(())
        } else {
            Err(RegistryError::NotFound(collection_id.to_string()))
        }
    }

    /// Get collection by ID
    pub fn get(&self, collection_id: i32) -> Option<HybridCollectionConfig> {
        self.collections_by_id
            .read()
            .get(&collection_id)
            .map(|e| e.config.clone())
    }

    /// Get collection by name
    pub fn get_by_name(&self, name: &str) -> Option<HybridCollectionConfig> {
        let collection_id = self.collections_by_name.read().get(name).copied()?;
        self.get(collection_id)
    }

    /// Update collection configuration
    pub fn update(&self, config: HybridCollectionConfig) -> Result<(), RegistryError> {
        let collection_id = config.collection_id;

        let mut by_id = self.collections_by_id.write();
        if let Some(entry) = by_id.get_mut(&collection_id) {
            entry.config = config;
            Ok(())
        } else {
            Err(RegistryError::NotFound(collection_id.to_string()))
        }
    }

    /// Update corpus statistics for a collection
    pub fn update_stats(
        &self,
        collection_id: i32,
        stats: CorpusStats,
    ) -> Result<(), RegistryError> {
        let mut by_id = self.collections_by_id.write();
        if let Some(entry) = by_id.get_mut(&collection_id) {
            entry.config.update_stats(stats);
            // Clear caches when stats change
            entry.idf_cache.clear();
            entry.df_cache.clear();
            Ok(())
        } else {
            Err(RegistryError::NotFound(collection_id.to_string()))
        }
    }

    /// Set document frequency for a term in a collection
    pub fn set_doc_freq(
        &self,
        collection_id: i32,
        term: &str,
        doc_freq: u64,
    ) -> Result<(), RegistryError> {
        let mut by_id = self.collections_by_id.write();
        if let Some(entry) = by_id.get_mut(&collection_id) {
            entry.df_cache.insert(term.to_string(), doc_freq);
            // Invalidate IDF cache for this term
            entry.idf_cache.remove(term);
            Ok(())
        } else {
            Err(RegistryError::NotFound(collection_id.to_string()))
        }
    }

    /// Get IDF for a term, computing if not cached
    pub fn get_idf(&self, collection_id: i32, term: &str) -> Option<f32> {
        let mut by_id = self.collections_by_id.write();
        let entry = by_id.get_mut(&collection_id)?;

        // Check cache
        if let Some(&idf) = entry.idf_cache.get(term) {
            return Some(idf);
        }

        // Compute IDF
        let df = entry.df_cache.get(term).copied().unwrap_or(0);
        let n = entry.config.corpus_stats.doc_count as f32;
        let df_f = df as f32;

        let idf = if df == 0 {
            (n + 0.5).ln()
        } else {
            ((n - df_f + 0.5) / (df_f + 0.5) + 1.0).ln()
        };

        // Cache and return
        entry.idf_cache.insert(term.to_string(), idf);
        Some(idf)
    }

    /// List all registered collections
    pub fn list(&self) -> Vec<HybridCollectionConfig> {
        self.collections_by_id
            .read()
            .values()
            .map(|e| e.config.clone())
            .collect()
    }

    /// Check if a collection is registered
    pub fn is_registered(&self, collection_id: i32) -> bool {
        self.collections_by_id.read().contains_key(&collection_id)
    }

    /// Get collections needing stats refresh
    pub fn collections_needing_refresh(&self) -> Vec<i32> {
        self.collections_by_id
            .read()
            .iter()
            .filter(|(_, e)| e.config.needs_stats_refresh())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Clear all caches
    pub fn clear_caches(&self) {
        let mut by_id = self.collections_by_id.write();
        for entry in by_id.values_mut() {
            entry.idf_cache.clear();
            entry.df_cache.clear();
        }
    }
}

impl Default for HybridRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry error types
#[derive(Debug, Clone)]
pub enum RegistryError {
    /// Collection already registered
    AlreadyRegistered(String),
    /// Collection not found
    NotFound(String),
    /// Invalid configuration
    InvalidConfig(String),
    /// Database error
    DatabaseError(String),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::AlreadyRegistered(name) => {
                write!(
                    f,
                    "Collection '{}' is already registered for hybrid search",
                    name
                )
            }
            RegistryError::NotFound(name) => {
                write!(f, "Hybrid collection '{}' not found", name)
            }
            RegistryError::InvalidConfig(msg) => {
                write!(f, "Invalid hybrid configuration: {}", msg)
            }
            RegistryError::DatabaseError(msg) => {
                write!(f, "Database error: {}", msg)
            }
        }
    }
}

impl std::error::Error for RegistryError {}

// Global registry instance
lazy_static::lazy_static! {
    /// Global hybrid collections registry
    pub static ref HYBRID_REGISTRY: Arc<HybridRegistry> = Arc::new(HybridRegistry::new());
}

/// Get the global hybrid registry
pub fn get_registry() -> Arc<HybridRegistry> {
    HYBRID_REGISTRY.clone()
}

/// Configuration update from JSONB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridConfigUpdate {
    /// New fusion method
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_fusion: Option<String>,
    /// New alpha value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_alpha: Option<f32>,
    /// New RRF k value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rrf_k: Option<usize>,
    /// New prefetch k value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefetch_k: Option<usize>,
    /// BM25 k1 parameter
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm25_k1: Option<f32>,
    /// BM25 b parameter
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm25_b: Option<f32>,
    /// Stats refresh interval (e.g., "1 hour", "30 minutes")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats_refresh_interval: Option<String>,
    /// Enable parallel execution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_enabled: Option<bool>,
}

impl HybridConfigUpdate {
    /// Apply updates to a configuration
    pub fn apply(&self, config: &mut HybridCollectionConfig) -> Result<(), RegistryError> {
        if let Some(ref fusion) = self.default_fusion {
            config.fusion_config.method = fusion
                .parse()
                .map_err(|e: String| RegistryError::InvalidConfig(e))?;
        }

        if let Some(alpha) = self.default_alpha {
            if !(0.0..=1.0).contains(&alpha) {
                return Err(RegistryError::InvalidConfig(
                    "alpha must be between 0 and 1".into(),
                ));
            }
            config.fusion_config.alpha = alpha;
        }

        if let Some(rrf_k) = self.rrf_k {
            if rrf_k == 0 {
                return Err(RegistryError::InvalidConfig(
                    "rrf_k must be positive".into(),
                ));
            }
            config.fusion_config.rrf_k = rrf_k;
        }

        if let Some(prefetch_k) = self.prefetch_k {
            if prefetch_k == 0 {
                return Err(RegistryError::InvalidConfig(
                    "prefetch_k must be positive".into(),
                ));
            }
            config.prefetch_k = prefetch_k;
        }

        if let Some(k1) = self.bm25_k1 {
            config.bm25_config.k1 = k1.max(0.0);
        }

        if let Some(b) = self.bm25_b {
            config.bm25_config.b = b.clamp(0.0, 1.0);
        }

        if let Some(ref interval) = self.stats_refresh_interval {
            config.stats_refresh_interval = parse_interval(interval)?;
        }

        if let Some(parallel) = self.parallel_enabled {
            config.parallel_enabled = parallel;
        }

        config.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        Ok(())
    }
}

/// Parse interval string to seconds
fn parse_interval(s: &str) -> Result<i64, RegistryError> {
    let s = s.trim().to_lowercase();

    // Try common formats
    if let Some(hours) = s.strip_suffix(" hour").or_else(|| s.strip_suffix(" hours")) {
        return hours
            .trim()
            .parse::<i64>()
            .map(|h| h * 3600)
            .map_err(|_| RegistryError::InvalidConfig(format!("Invalid interval: {}", s)));
    }

    if let Some(mins) = s
        .strip_suffix(" minute")
        .or_else(|| s.strip_suffix(" minutes"))
    {
        return mins
            .trim()
            .parse::<i64>()
            .map(|m| m * 60)
            .map_err(|_| RegistryError::InvalidConfig(format!("Invalid interval: {}", s)));
    }

    if let Some(secs) = s
        .strip_suffix(" second")
        .or_else(|| s.strip_suffix(" seconds"))
    {
        return secs
            .trim()
            .parse::<i64>()
            .map_err(|_| RegistryError::InvalidConfig(format!("Invalid interval: {}", s)));
    }

    // Try as plain seconds
    s.parse::<i64>()
        .map_err(|_| RegistryError::InvalidConfig(format!("Invalid interval: {}", s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_register_get() {
        let registry = HybridRegistry::new();

        let config = HybridCollectionConfig::new(
            1,
            "documents".to_string(),
            "embedding".to_string(),
            "fts".to_string(),
            "content".to_string(),
        );

        registry.register(config.clone()).unwrap();

        let retrieved = registry.get(1).unwrap();
        assert_eq!(retrieved.table_name, "documents");
        assert_eq!(retrieved.vector_column, "embedding");
    }

    #[test]
    fn test_registry_duplicate() {
        let registry = HybridRegistry::new();

        let config = HybridCollectionConfig::new(
            1,
            "documents".to_string(),
            "embedding".to_string(),
            "fts".to_string(),
            "content".to_string(),
        );

        registry.register(config.clone()).unwrap();
        let result = registry.register(config);

        assert!(matches!(result, Err(RegistryError::AlreadyRegistered(_))));
    }

    #[test]
    fn test_registry_get_by_name() {
        let registry = HybridRegistry::new();

        let config = HybridCollectionConfig::new(
            42,
            "my_table".to_string(),
            "vec".to_string(),
            "tsv".to_string(),
            "text".to_string(),
        );

        registry.register(config).unwrap();

        let retrieved = registry.get_by_name("public.my_table").unwrap();
        assert_eq!(retrieved.collection_id, 42);
    }

    #[test]
    fn test_registry_update_stats() {
        let registry = HybridRegistry::new();

        let config = HybridCollectionConfig::new(
            1,
            "test".to_string(),
            "vec".to_string(),
            "fts".to_string(),
            "text".to_string(),
        );

        registry.register(config).unwrap();

        let new_stats = CorpusStats {
            avg_doc_length: 150.0,
            doc_count: 5000,
            total_terms: 500000,
            last_update: 12345,
        };

        registry.update_stats(1, new_stats).unwrap();

        let updated = registry.get(1).unwrap();
        assert!((updated.corpus_stats.avg_doc_length - 150.0).abs() < 0.01);
        assert_eq!(updated.corpus_stats.doc_count, 5000);
    }

    #[test]
    fn test_config_update() {
        let mut config = HybridCollectionConfig::new(
            1,
            "test".to_string(),
            "vec".to_string(),
            "fts".to_string(),
            "text".to_string(),
        );

        let update = HybridConfigUpdate {
            default_fusion: Some("linear".to_string()),
            default_alpha: Some(0.7),
            rrf_k: Some(40),
            prefetch_k: Some(200),
            bm25_k1: Some(1.5),
            bm25_b: Some(0.8),
            stats_refresh_interval: Some("2 hours".to_string()),
            parallel_enabled: Some(false),
        };

        update.apply(&mut config).unwrap();

        assert_eq!(config.fusion_config.method, FusionMethod::Linear);
        assert!((config.fusion_config.alpha - 0.7).abs() < 0.01);
        assert_eq!(config.fusion_config.rrf_k, 40);
        assert_eq!(config.prefetch_k, 200);
        assert!((config.bm25_config.k1 - 1.5).abs() < 0.01);
        assert!((config.bm25_config.b - 0.8).abs() < 0.01);
        assert_eq!(config.stats_refresh_interval, 7200);
        assert!(!config.parallel_enabled);
    }

    #[test]
    fn test_parse_interval() {
        assert_eq!(parse_interval("1 hour").unwrap(), 3600);
        assert_eq!(parse_interval("2 hours").unwrap(), 7200);
        assert_eq!(parse_interval("30 minutes").unwrap(), 1800);
        assert_eq!(parse_interval("60 seconds").unwrap(), 60);
        assert_eq!(parse_interval("120").unwrap(), 120);
    }

    #[test]
    fn test_needs_refresh() {
        let mut config = HybridCollectionConfig::new(
            1,
            "test".to_string(),
            "vec".to_string(),
            "fts".to_string(),
            "text".to_string(),
        );

        // Fresh stats should not need refresh
        config.corpus_stats.last_update = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        config.stats_refresh_interval = 3600;

        assert!(!config.needs_stats_refresh());

        // Old stats should need refresh
        config.corpus_stats.last_update -= 7200;
        assert!(config.needs_stats_refresh());
    }
}
