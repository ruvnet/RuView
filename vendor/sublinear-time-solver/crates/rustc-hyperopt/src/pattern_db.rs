//! Ecosystem pattern database for intelligent optimization

use crate::{
    error::{OptimizerError, Result},
    optimizer::PatternDbConfig,
    signature::ProjectSignature,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

/// Database of compilation patterns from the Rust ecosystem
pub struct EcosystemPatternDatabase {
    config: PatternDbConfig,
    patterns: Arc<RwLock<HashMap<String, CompilationPattern>>>,
    pattern_index: Arc<RwLock<PatternIndex>>,
}

impl EcosystemPatternDatabase {
    /// Create a new pattern database with default configuration
    pub async fn new() -> Result<Self> {
        Self::with_config(PatternDbConfig::default()).await
    }

    /// Create with custom configuration
    pub async fn with_config(config: PatternDbConfig) -> Result<Self> {
        let patterns = Arc::new(RwLock::new(HashMap::new()));
        let pattern_index = Arc::new(RwLock::new(PatternIndex::new()));

        let db = Self {
            config,
            patterns,
            pattern_index,
        };

        // Load built-in patterns
        db.load_builtin_patterns().await?;

        Ok(db)
    }

    /// Find patterns matching a project signature
    pub async fn find_matching_patterns(&self, signature: &ProjectSignature) -> Result<Vec<CompilationPattern>> {
        let index = self.pattern_index.read().await;
        let patterns = self.patterns.read().await;

        let mut matches = Vec::new();

        // Match by dependencies
        for dep in &signature.dependencies.direct_deps {
            if let Some(pattern_ids) = index.dependency_patterns.get(dep) {
                for pattern_id in pattern_ids {
                    if let Some(pattern) = patterns.get(pattern_id) {
                        if pattern.confidence >= self.config.confidence_threshold {
                            matches.push(pattern.clone());
                        }
                    }
                }
            }
        }

        // Match by features
        if signature.features.has_proc_macros {
            if let Some(pattern_ids) = index.feature_patterns.get("proc_macros") {
                for pattern_id in pattern_ids {
                    if let Some(pattern) = patterns.get(pattern_id) {
                        if pattern.confidence >= self.config.confidence_threshold {
                            matches.push(pattern.clone());
                        }
                    }
                }
            }
        }

        if signature.features.has_async {
            if let Some(pattern_ids) = index.feature_patterns.get("async") {
                for pattern_id in pattern_ids {
                    if let Some(pattern) = patterns.get(pattern_id) {
                        if pattern.confidence >= self.config.confidence_threshold {
                            matches.push(pattern.clone());
                        }
                    }
                }
            }
        }

        // Remove duplicates and sort by confidence
        matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        matches.dedup_by(|a, b| a.pattern_id == b.pattern_id);

        Ok(matches)
    }

    /// Add a new pattern to the database
    pub async fn add_pattern(&self, pattern: CompilationPattern) -> Result<()> {
        let mut patterns = self.patterns.write().await;
        let mut index = self.pattern_index.write().await;

        // Update dependency index
        for dep in &pattern.dependencies {
            index.dependency_patterns
                .entry(dep.clone())
                .or_insert_with(Vec::new)
                .push(pattern.pattern_id.clone());
        }

        // Update feature index
        for feature in &pattern.features {
            index.feature_patterns
                .entry(feature.clone())
                .or_insert_with(Vec::new)
                .push(pattern.pattern_id.clone());
        }

        patterns.insert(pattern.pattern_id.clone(), pattern);

        Ok(())
    }

    /// Get pattern database statistics
    pub async fn get_stats(&self) -> PatternDbStats {
        let patterns = self.patterns.read().await;
        let index = self.pattern_index.read().await;

        PatternDbStats {
            total_patterns: patterns.len(),
            indexed_dependencies: index.dependency_patterns.len(),
            indexed_features: index.feature_patterns.len(),
            average_confidence: patterns.values()
                .map(|p| p.confidence)
                .sum::<f64>() / patterns.len() as f64,
        }
    }

    async fn load_builtin_patterns(&self) -> Result<()> {
        // Load common patterns for popular crates
        let serde_pattern = CompilationPattern {
            pattern_id: "serde_v1".to_string(),
            name: "Serde Serialization".to_string(),
            description: "Common pattern for serde-based serialization".to_string(),
            dependencies: vec!["serde".to_string(), "serde_json".to_string()],
            features: vec!["derive".to_string()],
            fingerprint: vec![1, 2, 3, 4], // Simplified fingerprint
            confidence: 0.95,
            usage_count: 50000,
            created_at: chrono::Utc::now(),
        };

        let tokio_pattern = CompilationPattern {
            pattern_id: "tokio_v1".to_string(),
            name: "Tokio Async Runtime".to_string(),
            description: "Common pattern for tokio-based async applications".to_string(),
            dependencies: vec!["tokio".to_string()],
            features: vec!["async".to_string(), "runtime".to_string()],
            fingerprint: vec![5, 6, 7, 8], // Simplified fingerprint
            confidence: 0.92,
            usage_count: 30000,
            created_at: chrono::Utc::now(),
        };

        let proc_macro_pattern = CompilationPattern {
            pattern_id: "proc_macro_v1".to_string(),
            name: "Procedural Macros".to_string(),
            description: "Common pattern for procedural macro usage".to_string(),
            dependencies: vec!["proc-macro2".to_string(), "syn".to_string(), "quote".to_string()],
            features: vec!["proc_macros".to_string()],
            fingerprint: vec![9, 10, 11, 12], // Simplified fingerprint
            confidence: 0.88,
            usage_count: 20000,
            created_at: chrono::Utc::now(),
        };

        self.add_pattern(serde_pattern).await?;
        self.add_pattern(tokio_pattern).await?;
        self.add_pattern(proc_macro_pattern).await?;

        Ok(())
    }
}

/// A compilation pattern from the ecosystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationPattern {
    /// Unique pattern identifier
    pub pattern_id: String,
    /// Human-readable pattern name
    pub name: String,
    /// Pattern description
    pub description: String,
    /// Associated dependencies
    pub dependencies: Vec<String>,
    /// Associated features
    pub features: Vec<String>,
    /// Blake3 fingerprint of the pattern
    pub fingerprint: Vec<u8>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Number of times this pattern has been observed
    pub usage_count: u64,
    /// When this pattern was created
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Index for fast pattern lookups
#[derive(Debug, Default)]
struct PatternIndex {
    /// Dependency name -> pattern IDs
    dependency_patterns: HashMap<String, Vec<String>>,
    /// Feature name -> pattern IDs
    feature_patterns: HashMap<String, Vec<String>>,
}

impl PatternIndex {
    fn new() -> Self {
        Self::default()
    }
}

/// Statistics about the pattern database
#[derive(Debug, Clone)]
pub struct PatternDbStats {
    /// Total number of patterns
    pub total_patterns: usize,
    /// Number of indexed dependencies
    pub indexed_dependencies: usize,
    /// Number of indexed features
    pub indexed_features: usize,
    /// Average confidence score
    pub average_confidence: f64,
}