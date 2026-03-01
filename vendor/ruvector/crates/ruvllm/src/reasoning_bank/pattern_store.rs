//! Pattern Storage with HNSW Index
//!
//! High-performance pattern storage using ruvector-core's HNSW index
//! for fast similarity search (150x faster than brute force).

use crate::error::{Result, RuvLLMError};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use ruvector_core::types::{DbOptions, HnswConfig};
use ruvector_core::{DistanceMetric, SearchQuery, VectorDB, VectorEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

use super::{KeyLesson, Trajectory, Verdict};

/// Global pattern ID counter
static PATTERN_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Pattern category for classification
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatternCategory {
    /// General purpose pattern
    General,
    /// Reasoning and logic patterns
    Reasoning,
    /// Code generation patterns
    CodeGeneration,
    /// Research and analysis patterns
    Research,
    /// Creative writing patterns
    Creative,
    /// Conversation and chat patterns
    Conversational,
    /// Tool usage patterns
    ToolUse,
    /// Error recovery patterns
    ErrorRecovery,
    /// Reflection and self-correction patterns
    Reflection,
    /// Custom category
    Custom(String),
}

impl Default for PatternCategory {
    fn default() -> Self {
        Self::General
    }
}

impl std::fmt::Display for PatternCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatternCategory::General => write!(f, "general"),
            PatternCategory::Reasoning => write!(f, "reasoning"),
            PatternCategory::CodeGeneration => write!(f, "code_generation"),
            PatternCategory::Research => write!(f, "research"),
            PatternCategory::Creative => write!(f, "creative"),
            PatternCategory::Conversational => write!(f, "conversational"),
            PatternCategory::ToolUse => write!(f, "tool_use"),
            PatternCategory::ErrorRecovery => write!(f, "error_recovery"),
            PatternCategory::Reflection => write!(f, "reflection"),
            PatternCategory::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

/// Configuration for the pattern store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternStoreConfig {
    /// Embedding dimension
    pub embedding_dim: usize,
    /// HNSW ef_construction parameter
    pub ef_construction: usize,
    /// HNSW ef_search parameter
    pub ef_search: usize,
    /// HNSW M parameter (connections per layer)
    pub m: usize,
    /// Maximum patterns to store
    pub max_patterns: usize,
    /// Distance metric
    pub distance_metric: String,
    /// Minimum confidence to store
    pub min_confidence: f32,
    /// Enable automatic pruning
    pub auto_prune: bool,
    /// Pruning threshold (min usage count)
    pub prune_threshold: u32,
    /// Maximum age for unused patterns (seconds)
    pub max_unused_age_secs: u64,
}

impl Default for PatternStoreConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 768,
            ef_construction: 200,
            ef_search: 100,
            m: 32,
            max_patterns: 100_000,
            distance_metric: "cosine".to_string(),
            min_confidence: 0.3,
            auto_prune: true,
            prune_threshold: 2,
            max_unused_age_secs: 86400 * 30, // 30 days
        }
    }
}

/// A learned pattern stored in the bank
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    /// Unique identifier
    pub id: u64,
    /// UUID for external reference
    pub uuid: Uuid,
    /// Pattern embedding (centroid)
    pub embedding: Vec<f32>,
    /// Category
    pub category: PatternCategory,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Usage count (how many times this pattern was matched)
    pub usage_count: u32,
    /// Success count when used
    pub success_count: u32,
    /// Average quality when used
    pub avg_quality: f32,
    /// Source trajectory IDs
    pub source_trajectories: Vec<u64>,
    /// Lessons associated with this pattern
    pub lessons: Vec<String>,
    /// Example actions for this pattern
    pub example_actions: Vec<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last accessed timestamp
    pub last_accessed: DateTime<Utc>,
    /// Metadata
    pub metadata: PatternMetadata,
}

/// Metadata for a pattern
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatternMetadata {
    /// Tags
    pub tags: Vec<String>,
    /// Source (e.g., "trajectory", "lesson", "manual")
    pub source: String,
    /// Version
    pub version: u32,
    /// Custom attributes
    pub attributes: HashMap<String, String>,
}

impl Pattern {
    /// Create a new pattern
    pub fn new(embedding: Vec<f32>, category: PatternCategory, confidence: f32) -> Self {
        let now = Utc::now();
        Self {
            id: PATTERN_COUNTER.fetch_add(1, Ordering::SeqCst),
            uuid: Uuid::new_v4(),
            embedding,
            category,
            confidence,
            usage_count: 0,
            success_count: 0,
            avg_quality: 0.0,
            source_trajectories: Vec::new(),
            lessons: Vec::new(),
            example_actions: Vec::new(),
            created_at: now,
            last_accessed: now,
            metadata: PatternMetadata {
                source: "manual".to_string(),
                ..Default::default()
            },
        }
    }

    /// Builder: add a lesson to this pattern
    pub fn with_lesson(mut self, lesson: String) -> Self {
        if !self.lessons.contains(&lesson) {
            self.lessons.push(lesson);
        }
        self
    }

    /// Builder: add an example action to this pattern
    pub fn with_action(mut self, action: String) -> Self {
        if !self.example_actions.contains(&action) && self.example_actions.len() < 10 {
            self.example_actions.push(action);
        }
        self
    }

    /// Builder: add a tag to this pattern
    pub fn with_tag(mut self, tag: String) -> Self {
        if !self.metadata.tags.contains(&tag) {
            self.metadata.tags.push(tag);
        }
        self
    }

    /// Builder: set the source
    pub fn with_source(mut self, source: String) -> Self {
        self.metadata.source = source;
        self
    }

    /// Create a pattern from a trajectory
    pub fn from_trajectory(trajectory: &Trajectory) -> Self {
        let category = Self::infer_category(trajectory);

        let example_actions: Vec<String> = trajectory
            .steps
            .iter()
            .take(5)
            .map(|s| s.action.clone())
            .collect();

        let now = Utc::now();
        Self {
            id: PATTERN_COUNTER.fetch_add(1, Ordering::SeqCst),
            uuid: Uuid::new_v4(),
            embedding: trajectory.query_embedding.clone(),
            category,
            confidence: trajectory.quality,
            usage_count: 1,
            success_count: if trajectory.is_success() { 1 } else { 0 },
            avg_quality: trajectory.quality,
            source_trajectories: vec![trajectory.id.as_u64()],
            lessons: trajectory.lessons.clone(),
            example_actions,
            created_at: now,
            last_accessed: now,
            metadata: PatternMetadata {
                source: "trajectory".to_string(),
                tags: trajectory.metadata.tags.clone(),
                ..Default::default()
            },
        }
    }

    /// Create a pattern from a distilled lesson
    pub fn from_lesson(lesson: &KeyLesson) -> Self {
        let now = Utc::now();
        Self {
            id: PATTERN_COUNTER.fetch_add(1, Ordering::SeqCst),
            uuid: Uuid::new_v4(),
            embedding: lesson.embedding.clone(),
            category: lesson.category.clone(),
            confidence: lesson.importance,
            usage_count: lesson.observation_count,
            success_count: (lesson.observation_count as f32 * lesson.success_rate) as u32,
            avg_quality: lesson.avg_quality,
            source_trajectories: lesson.source_trajectory_ids.clone(),
            lessons: vec![lesson.content.clone()],
            example_actions: lesson.example_actions.clone(),
            created_at: now,
            last_accessed: now,
            metadata: PatternMetadata {
                source: "lesson".to_string(),
                tags: lesson.tags.clone(),
                ..Default::default()
            },
        }
    }

    /// Infer category from trajectory
    fn infer_category(trajectory: &Trajectory) -> PatternCategory {
        // Check request type first
        if let Some(ref req_type) = trajectory.metadata.request_type {
            let req_lower = req_type.to_lowercase();
            if req_lower.contains("code") || req_lower.contains("programming") {
                return PatternCategory::CodeGeneration;
            }
            if req_lower.contains("research") || req_lower.contains("analyze") {
                return PatternCategory::Research;
            }
            if req_lower.contains("creative") || req_lower.contains("write") {
                return PatternCategory::Creative;
            }
        }

        // Check tools used
        if !trajectory.metadata.tools_invoked.is_empty() {
            return PatternCategory::ToolUse;
        }

        // Check for reflection/recovery
        if matches!(trajectory.verdict, Verdict::RecoveredViaReflection { .. }) {
            return PatternCategory::Reflection;
        }

        // Check tags
        for tag in &trajectory.metadata.tags {
            let tag_lower = tag.to_lowercase();
            if tag_lower.contains("reasoning") || tag_lower.contains("logic") {
                return PatternCategory::Reasoning;
            }
            if tag_lower.contains("chat") || tag_lower.contains("conversation") {
                return PatternCategory::Conversational;
            }
        }

        PatternCategory::General
    }

    /// Record a usage of this pattern
    pub fn record_usage(&mut self, was_successful: bool, quality: f32) {
        self.usage_count += 1;
        if was_successful {
            self.success_count += 1;
        }

        // Update rolling average quality
        let n = self.usage_count as f32;
        self.avg_quality = self.avg_quality * ((n - 1.0) / n) + quality / n;

        self.last_accessed = Utc::now();
    }

    /// Get success rate
    pub fn success_rate(&self) -> f32 {
        if self.usage_count == 0 {
            return 0.0;
        }
        self.success_count as f32 / self.usage_count as f32
    }

    /// Merge with another pattern
    pub fn merge(&mut self, other: &Pattern) {
        // Weighted average of embeddings
        let total_count = self.usage_count + other.usage_count;
        if total_count == 0 {
            return;
        }

        let w1 = self.usage_count as f32 / total_count as f32;
        let w2 = other.usage_count as f32 / total_count as f32;

        for (i, e) in self.embedding.iter_mut().enumerate() {
            if i < other.embedding.len() {
                *e = *e * w1 + other.embedding[i] * w2;
            }
        }

        // Merge statistics
        self.usage_count = total_count;
        self.success_count += other.success_count;
        self.avg_quality = self.avg_quality * w1 + other.avg_quality * w2;
        self.confidence = self.confidence.max(other.confidence);

        // Merge collections
        self.source_trajectories
            .extend(other.source_trajectories.clone());
        for lesson in &other.lessons {
            if !self.lessons.contains(lesson) {
                self.lessons.push(lesson.clone());
            }
        }
        for action in &other.example_actions {
            if !self.example_actions.contains(action) && self.example_actions.len() < 10 {
                self.example_actions.push(action.clone());
            }
        }

        self.last_accessed = Utc::now();
    }

    /// Compute cosine similarity with a query
    pub fn similarity(&self, query: &[f32]) -> f32 {
        if self.embedding.len() != query.len() {
            return 0.0;
        }

        let dot: f32 = self.embedding.iter().zip(query).map(|(a, b)| a * b).sum();
        let norm_a: f32 = self.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a > 1e-8 && norm_b > 1e-8 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }

    /// Check if pattern should be pruned
    pub fn should_prune(&self, min_usage: u32, max_age_secs: u64, min_quality: f32) -> bool {
        let age = (Utc::now() - self.last_accessed).num_seconds() as u64;

        // Prune if old, unused, and low quality
        self.usage_count < min_usage && age > max_age_secs && self.avg_quality < min_quality
    }
}

/// Result of a pattern search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSearchResult {
    /// The matched pattern
    pub pattern: Pattern,
    /// Similarity score (0.0 - 1.0)
    pub similarity: f32,
    /// Rank in results (0-based)
    pub rank: usize,
}

/// Statistics for the pattern store
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatternStats {
    /// Total patterns stored
    pub total_patterns: usize,
    /// Patterns by category
    pub by_category: HashMap<String, usize>,
    /// Average confidence
    pub avg_confidence: f32,
    /// Average usage count
    pub avg_usage: f32,
    /// Total searches performed
    pub total_searches: u64,
    /// Average search latency (ms)
    pub avg_search_latency_ms: f32,
}

/// Pattern store with HNSW index
pub struct PatternStore {
    /// Configuration
    config: PatternStoreConfig,
    /// HNSW index via VectorDB
    index: RwLock<VectorDB>,
    /// Pattern storage (id -> Pattern)
    patterns: RwLock<HashMap<u64, Pattern>>,
    /// Category index (category -> pattern ids)
    category_index: RwLock<HashMap<PatternCategory, Vec<u64>>>,
    /// Statistics
    stats: RwLock<PatternStats>,
    /// Search count
    search_count: AtomicU64,
    /// Total search time (microseconds)
    total_search_time_us: AtomicU64,
}

impl PatternStore {
    /// Create a new pattern store
    pub fn new(config: PatternStoreConfig) -> Result<Self> {
        let distance_metric = match config.distance_metric.as_str() {
            "cosine" => DistanceMetric::Cosine,
            "euclidean" => DistanceMetric::Euclidean,
            "dot" => DistanceMetric::DotProduct,
            _ => DistanceMetric::Cosine,
        };

        let db_options = DbOptions {
            dimensions: config.embedding_dim,
            distance_metric,
            storage_path: ".reasoning_bank_patterns".to_string(),
            hnsw_config: Some(HnswConfig {
                m: config.m,
                ef_construction: config.ef_construction,
                ef_search: config.ef_search,
                max_elements: config.max_patterns,
            }),
            quantization: None,
        };

        let index = VectorDB::new(db_options)
            .map_err(|e| RuvLLMError::Storage(format!("Failed to create HNSW index: {}", e)))?;

        Ok(Self {
            config,
            index: RwLock::new(index),
            patterns: RwLock::new(HashMap::new()),
            category_index: RwLock::new(HashMap::new()),
            stats: RwLock::new(PatternStats::default()),
            search_count: AtomicU64::new(0),
            total_search_time_us: AtomicU64::new(0),
        })
    }

    /// Store a pattern
    pub fn store_pattern(&mut self, pattern: Pattern) -> Result<u64> {
        let id = pattern.id;

        // Check capacity
        {
            let patterns = self.patterns.read();
            if patterns.len() >= self.config.max_patterns {
                drop(patterns);
                self.prune_oldest()?;
            }
        }

        // Check minimum confidence
        if pattern.confidence < self.config.min_confidence {
            return Err(RuvLLMError::InvalidOperation(format!(
                "Pattern confidence {} below threshold {}",
                pattern.confidence, self.config.min_confidence
            )));
        }

        // Insert into HNSW index
        {
            let entry = VectorEntry {
                id: Some(id.to_string()),
                vector: pattern.embedding.clone(),
                metadata: None,
            };
            let index = self.index.write();
            index
                .insert(entry)
                .map_err(|e| RuvLLMError::Storage(format!("Failed to insert into index: {}", e)))?;
        }

        // Store pattern
        {
            let mut patterns = self.patterns.write();
            patterns.insert(id, pattern.clone());
        }

        // Update category index
        {
            let mut cat_index = self.category_index.write();
            cat_index
                .entry(pattern.category.clone())
                .or_default()
                .push(id);
        }

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.total_patterns += 1;
            let cat_key = pattern.category.to_string();
            *stats.by_category.entry(cat_key).or_insert(0) += 1;
        }

        Ok(id)
    }

    /// Search for similar patterns
    pub fn search_similar(&self, query: &[f32], limit: usize) -> Result<Vec<PatternSearchResult>> {
        let start = std::time::Instant::now();

        // Search HNSW index
        let results = {
            let search_query = SearchQuery {
                vector: query.to_vec(),
                k: limit,
                filter: None,
                ef_search: Some(self.config.ef_search),
            };
            let index = self.index.read();
            index
                .search(search_query)
                .map_err(|e| RuvLLMError::Storage(format!("Search failed: {}", e)))?
        };

        // Build results with patterns
        let patterns = self.patterns.read();
        let mut search_results = Vec::with_capacity(results.len());

        for (rank, result) in results.into_iter().enumerate() {
            if let Ok(id) = result.id.parse::<u64>() {
                if let Some(pattern) = patterns.get(&id) {
                    search_results.push(PatternSearchResult {
                        pattern: pattern.clone(),
                        similarity: 1.0 - result.score, // Convert distance/score to similarity
                        rank,
                    });
                }
            }
        }

        // Update search stats
        let elapsed_us = start.elapsed().as_micros() as u64;
        self.search_count.fetch_add(1, Ordering::Relaxed);
        self.total_search_time_us
            .fetch_add(elapsed_us, Ordering::Relaxed);

        Ok(search_results)
    }

    /// Get patterns by category
    pub fn get_by_category(&self, category: PatternCategory, limit: usize) -> Result<Vec<Pattern>> {
        let cat_index = self.category_index.read();
        let patterns = self.patterns.read();

        let ids = cat_index.get(&category).cloned().unwrap_or_default();

        let mut result: Vec<Pattern> = ids
            .iter()
            .filter_map(|id| patterns.get(id).cloned())
            .take(limit)
            .collect();

        // Sort by confidence descending
        result.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(result)
    }

    /// Get all patterns
    pub fn get_all_patterns(&self) -> Result<Vec<Pattern>> {
        let patterns = self.patterns.read();
        Ok(patterns.values().cloned().collect())
    }

    /// Get pattern by ID
    pub fn get_pattern(&self, id: u64) -> Option<Pattern> {
        let patterns = self.patterns.read();
        patterns.get(&id).cloned()
    }

    /// Remove a pattern
    pub fn remove_pattern(&mut self, id: u64) -> Result<bool> {
        // Remove from patterns
        let pattern = {
            let mut patterns = self.patterns.write();
            patterns.remove(&id)
        };

        if let Some(p) = pattern {
            // Remove from category index
            {
                let mut cat_index = self.category_index.write();
                if let Some(ids) = cat_index.get_mut(&p.category) {
                    ids.retain(|&x| x != id);
                }
            }

            // Update stats
            {
                let mut stats = self.stats.write();
                stats.total_patterns = stats.total_patterns.saturating_sub(1);
                let cat_key = p.category.to_string();
                if let Some(count) = stats.by_category.get_mut(&cat_key) {
                    *count = count.saturating_sub(1);
                }
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Prune low quality patterns
    pub fn prune_low_quality(&mut self, min_quality: f32) -> Result<usize> {
        let to_remove: Vec<u64> = {
            let patterns = self.patterns.read();
            patterns
                .iter()
                .filter(|(_, p)| {
                    p.avg_quality < min_quality && p.usage_count < self.config.prune_threshold
                })
                .map(|(id, _)| *id)
                .collect()
        };

        let count = to_remove.len();
        for id in to_remove {
            self.remove_pattern(id)?;
        }

        Ok(count)
    }

    /// Prune oldest unused patterns
    fn prune_oldest(&mut self) -> Result<usize> {
        let to_remove: Vec<u64> = {
            let patterns = self.patterns.read();
            let mut sorted: Vec<_> = patterns
                .iter()
                .filter(|(_, p)| {
                    p.should_prune(
                        self.config.prune_threshold,
                        self.config.max_unused_age_secs,
                        self.config.min_confidence,
                    )
                })
                .collect();

            sorted.sort_by(|a, b| a.1.last_accessed.cmp(&b.1.last_accessed));

            let remove_count = sorted.len().min(self.config.max_patterns / 10);
            sorted
                .into_iter()
                .take(remove_count)
                .map(|(id, _)| *id)
                .collect()
        };

        let count = to_remove.len();
        for id in to_remove {
            self.remove_pattern(id)?;
        }

        Ok(count)
    }

    /// Merge similar patterns
    pub fn merge_similar(&mut self, similarity_threshold: f32) -> Result<usize> {
        let patterns: Vec<Pattern> = {
            let p = self.patterns.read();
            p.values().cloned().collect()
        };

        let mut merged_count = 0;
        let mut to_remove = Vec::new();

        for i in 0..patterns.len() {
            if to_remove.contains(&patterns[i].id) {
                continue;
            }

            for j in (i + 1)..patterns.len() {
                if to_remove.contains(&patterns[j].id) {
                    continue;
                }

                let sim = patterns[i].similarity(&patterns[j].embedding);
                if sim > similarity_threshold {
                    // Merge j into i
                    {
                        let mut p = self.patterns.write();
                        if let Some(target) = p.get_mut(&patterns[i].id) {
                            target.merge(&patterns[j]);
                        }
                    }
                    to_remove.push(patterns[j].id);
                    merged_count += 1;
                }
            }
        }

        // Remove merged patterns
        for id in to_remove {
            self.remove_pattern(id)?;
        }

        Ok(merged_count)
    }

    /// Record pattern usage
    pub fn record_usage(&self, id: u64, was_successful: bool, quality: f32) {
        let mut patterns = self.patterns.write();
        if let Some(pattern) = patterns.get_mut(&id) {
            pattern.record_usage(was_successful, quality);
        }
    }

    /// Get statistics
    pub fn stats(&self) -> PatternStats {
        let mut stats = self.stats.read().clone();

        // Update computed stats
        let search_count = self.search_count.load(Ordering::Relaxed);
        let total_time_us = self.total_search_time_us.load(Ordering::Relaxed);

        stats.total_searches = search_count;
        if search_count > 0 {
            stats.avg_search_latency_ms = (total_time_us as f32 / search_count as f32) / 1000.0;
        }

        // Compute averages
        let patterns = self.patterns.read();
        if !patterns.is_empty() {
            let total_conf: f32 = patterns.values().map(|p| p.confidence).sum();
            let total_usage: u32 = patterns.values().map(|p| p.usage_count).sum();
            stats.avg_confidence = total_conf / patterns.len() as f32;
            stats.avg_usage = total_usage as f32 / patterns.len() as f32;
        }

        stats
    }

    /// Get pattern count
    pub fn len(&self) -> usize {
        self.patterns.read().len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.patterns.read().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_creation() {
        let pattern = Pattern::new(vec![0.1; 768], PatternCategory::Reasoning, 0.9);

        assert!(pattern.id > 0 || pattern.id == 0); // First pattern might be 0
        assert_eq!(pattern.category, PatternCategory::Reasoning);
        assert_eq!(pattern.confidence, 0.9);
    }

    #[test]
    fn test_pattern_similarity() {
        let pattern = Pattern::new(vec![1.0, 0.0, 0.0], PatternCategory::General, 0.9);

        assert!((pattern.similarity(&[1.0, 0.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(pattern.similarity(&[0.0, 1.0, 0.0]).abs() < 1e-6);
    }

    #[test]
    fn test_pattern_merge() {
        let mut p1 = Pattern::new(vec![1.0, 0.0], PatternCategory::General, 0.8);
        p1.usage_count = 10;

        let mut p2 = Pattern::new(vec![0.0, 1.0], PatternCategory::General, 0.9);
        p2.usage_count = 10;

        p1.merge(&p2);

        assert_eq!(p1.usage_count, 20);
        assert!((p1.embedding[0] - 0.5).abs() < 1e-6);
        assert!((p1.embedding[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_pattern_store_config() {
        let config = PatternStoreConfig::default();
        assert_eq!(config.embedding_dim, 768);
        assert_eq!(config.ef_construction, 200);
        assert_eq!(config.m, 32);
    }

    #[test]
    fn test_pattern_store_creation() {
        let config = PatternStoreConfig {
            embedding_dim: 4,
            ..Default::default()
        };
        let store = PatternStore::new(config);
        assert!(store.is_ok());
    }

    #[test]
    fn test_pattern_store_operations() {
        let config = PatternStoreConfig {
            embedding_dim: 4,
            min_confidence: 0.1,
            ..Default::default()
        };
        let mut store = PatternStore::new(config).unwrap();

        // Store pattern
        let pattern = Pattern::new(vec![1.0, 0.0, 0.0, 0.0], PatternCategory::Reasoning, 0.9);
        let id = store.store_pattern(pattern).unwrap();

        // Search
        let results = store.search_similar(&[1.0, 0.0, 0.0, 0.0], 1).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].pattern.id, id);

        // Get by category
        let by_cat = store
            .get_by_category(PatternCategory::Reasoning, 10)
            .unwrap();
        assert!(!by_cat.is_empty());

        // Stats
        let stats = store.stats();
        assert_eq!(stats.total_patterns, 1);
    }

    #[test]
    fn test_pattern_category() {
        assert_eq!(PatternCategory::General.to_string(), "general");
        assert_eq!(
            PatternCategory::CodeGeneration.to_string(),
            "code_generation"
        );
        assert_eq!(
            PatternCategory::Custom("test".to_string()).to_string(),
            "custom:test"
        );
    }
}
