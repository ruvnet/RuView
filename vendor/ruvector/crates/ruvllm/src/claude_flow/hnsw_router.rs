//! HNSW-Powered Semantic Router for Claude Flow
//!
//! Provides 150x faster pattern search for task routing using ruvector-core's HNSW index.
//! Integrates with the existing AgentRouter for hybrid keyword + semantic routing.
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +-------------------+
//! | Task Description  |---->| Generate Embedding|
//! +-------------------+     +--------+----------+
//!                                    |
//!                                    v
//!                           +--------+----------+
//!                           | HNSW Index Search |  <-- 150x faster than brute force
//!                           | (Top-K neighbors) |
//!                           +--------+----------+
//!                                    |
//!                                    v
//!                           +--------+----------+
//!                           | Aggregate Votes   |
//!                           | (weighted by sim) |
//!                           +--------+----------+
//!                                    |
//!                                    v
//!                           +--------+----------+
//!                           | Routing Decision  |
//!                           +-------------------+
//! ```
//!
//! ## Online Learning
//!
//! The router supports online learning by adding new patterns as tasks succeed:
//!
//! 1. Task completes successfully
//! 2. Embedding + agent type + success stored in HNSW index
//! 3. Future similar tasks benefit from learned patterns
//!
//! ## Integration with SONA
//!
//! The HNSW router integrates with SONA learning for continuous improvement:
//!
//! - Instant Loop: Updates pattern success rates per-request
//! - Background Loop: Rebalances patterns, prunes low-quality entries
//! - Deep Loop: Consolidates similar patterns, knowledge transfer

use super::{AgentType, ClaudeFlowTask, RoutingDecision};
use crate::error::{Result, RuvLLMError};
use crate::sona::{SonaIntegration, Trajectory};
use dashmap::DashMap;
use parking_lot::RwLock;
use ruvector_core::index::hnsw::HnswIndex;
use ruvector_core::index::VectorIndex;
use ruvector_core::types::{DistanceMetric, HnswConfig, SearchResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Configuration for the HNSW router
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswRouterConfig {
    /// Number of connections per layer (M parameter)
    /// Higher values = better recall but more memory
    /// Typical: 16-64, default: 32
    pub m: usize,

    /// Size of dynamic candidate list during construction
    /// Higher values = better index quality but slower construction
    /// Typical: 100-500, default: 200
    pub ef_construction: usize,

    /// Size of dynamic candidate list during search
    /// Higher values = better recall but slower search
    /// Typical: 50-200, default: 100
    pub ef_search: usize,

    /// Maximum number of patterns to store
    pub max_patterns: usize,

    /// Distance metric for similarity calculation
    pub distance_metric: HnswDistanceMetric,

    /// Embedding dimension
    pub embedding_dim: usize,

    /// Minimum confidence threshold for routing decisions
    pub min_confidence: f32,

    /// Number of nearest neighbors to consider for voting
    pub top_k: usize,

    /// Decay factor for older patterns (0.0 = no decay, 1.0 = instant decay)
    pub success_rate_decay: f32,

    /// Minimum usage count before trusting pattern's success rate
    pub min_usage_for_trust: u32,

    /// Enable online learning (add patterns as tasks succeed)
    pub enable_online_learning: bool,
}

impl Default for HnswRouterConfig {
    fn default() -> Self {
        Self {
            m: 32,
            ef_construction: 200,
            ef_search: 100,
            max_patterns: 100_000,
            distance_metric: HnswDistanceMetric::Cosine,
            embedding_dim: 384,
            min_confidence: 0.5,
            top_k: 10,
            success_rate_decay: 0.01,
            min_usage_for_trust: 5,
            enable_online_learning: true,
        }
    }
}

impl HnswRouterConfig {
    /// Create configuration optimized for high recall
    pub fn high_recall() -> Self {
        Self {
            m: 48,
            ef_construction: 400,
            ef_search: 200,
            top_k: 20,
            ..Default::default()
        }
    }

    /// Create configuration optimized for speed
    pub fn fast() -> Self {
        Self {
            m: 16,
            ef_construction: 100,
            ef_search: 50,
            top_k: 5,
            ..Default::default()
        }
    }

    /// Create configuration for small models (384-dim embeddings)
    pub fn for_small_model() -> Self {
        Self {
            embedding_dim: 384,
            ..Default::default()
        }
    }

    /// Create configuration for large models (768-dim embeddings)
    pub fn for_large_model() -> Self {
        Self {
            embedding_dim: 768,
            ..Default::default()
        }
    }
}

/// Distance metric for HNSW search
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HnswDistanceMetric {
    /// Cosine similarity (recommended for embeddings)
    Cosine,
    /// Euclidean (L2) distance
    Euclidean,
    /// Dot product
    DotProduct,
}

impl From<HnswDistanceMetric> for DistanceMetric {
    #[inline]
    fn from(metric: HnswDistanceMetric) -> Self {
        match metric {
            HnswDistanceMetric::Cosine => DistanceMetric::Cosine,
            HnswDistanceMetric::Euclidean => DistanceMetric::Euclidean,
            HnswDistanceMetric::DotProduct => DistanceMetric::DotProduct,
        }
    }
}

/// A learned routing pattern stored in the HNSW index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPattern {
    /// Unique pattern identifier
    pub id: String,

    /// Task embedding vector
    pub embedding: Vec<f32>,

    /// Agent type that successfully handled this pattern
    pub agent_type: AgentType,

    /// Task type classification
    pub task_type: ClaudeFlowTask,

    /// Success rate for this pattern (0.0 - 1.0)
    pub success_rate: f32,

    /// Number of times this pattern was used
    pub usage_count: u32,

    /// Total successful uses
    pub success_count: u32,

    /// Task description (for debugging/inspection)
    pub task_description: String,

    /// Creation timestamp
    pub created_at: i64,

    /// Last used timestamp
    pub last_used_at: i64,

    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl TaskPattern {
    /// Create a new task pattern
    pub fn new(
        embedding: Vec<f32>,
        agent_type: AgentType,
        task_type: ClaudeFlowTask,
        task_description: String,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            embedding,
            agent_type,
            task_type,
            success_rate: 0.5, // Initial neutral success rate
            usage_count: 0,
            success_count: 0,
            task_description,
            created_at: now,
            last_used_at: now,
            metadata: HashMap::new(),
        }
    }

    /// Update success rate with exponential moving average
    #[inline]
    pub fn update_success(&mut self, success: bool, decay: f32) {
        self.usage_count += 1;
        if success {
            self.success_count += 1;
        }
        self.last_used_at = chrono::Utc::now().timestamp();

        // Exponential moving average
        let outcome = if success { 1.0 } else { 0.0 };
        self.success_rate = (1.0 - decay) * self.success_rate + decay * outcome;
    }

    /// Get weighted confidence based on usage count
    #[inline]
    pub fn confidence(&self, min_usage: u32) -> f32 {
        if self.usage_count < min_usage {
            // Low confidence for underutilized patterns
            0.5 * (self.usage_count as f32 / min_usage as f32)
        } else {
            self.success_rate
        }
    }

    /// Check if pattern is stale (not used recently)
    #[inline]
    pub fn is_stale(&self, max_age_secs: i64) -> bool {
        let now = chrono::Utc::now().timestamp();
        now - self.last_used_at > max_age_secs
    }
}

/// HNSW-based semantic routing result
#[derive(Debug, Clone)]
pub struct HnswRoutingResult {
    /// Primary agent recommendation
    pub primary_agent: AgentType,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,

    /// Task type classification
    pub task_type: ClaudeFlowTask,

    /// Number of patterns used for decision
    pub patterns_considered: usize,

    /// Alternative agents with scores
    pub alternatives: Vec<(AgentType, f32)>,

    /// Nearest neighbor distances
    pub neighbor_distances: Vec<f32>,

    /// Search latency in microseconds
    pub search_latency_us: u64,

    /// Reasoning for the decision
    pub reasoning: String,
}

impl From<HnswRoutingResult> for RoutingDecision {
    fn from(result: HnswRoutingResult) -> Self {
        RoutingDecision {
            primary_agent: result.primary_agent,
            confidence: result.confidence,
            alternatives: result.alternatives,
            task_type: result.task_type,
            reasoning: result.reasoning,
            learned_patterns: result.patterns_considered,
        }
    }
}

/// Serializable router state for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HnswRouterState {
    config: HnswRouterConfig,
    patterns: Vec<TaskPattern>,
    total_queries: u64,
    total_hits: u64,
}

/// HNSW-powered semantic router
///
/// Uses ruvector-core's HNSW index for 150x faster pattern search compared to
/// brute-force similarity computation.
pub struct HnswRouter {
    /// Configuration
    config: HnswRouterConfig,

    /// HNSW index for fast similarity search
    index: Arc<RwLock<HnswIndex>>,

    /// Pattern metadata storage (id -> pattern)
    patterns: DashMap<String, TaskPattern>,

    /// Index ID to pattern ID mapping
    index_to_pattern: DashMap<String, String>,

    /// Statistics
    total_queries: AtomicU64,
    total_hits: AtomicU64,
    total_patterns_added: AtomicU64,

    /// Optional SONA integration for continuous learning
    sona: Option<Arc<RwLock<SonaIntegration>>>,
}

impl HnswRouter {
    /// Create a new HNSW router
    pub fn new(config: HnswRouterConfig) -> Result<Self> {
        let hnsw_config = HnswConfig {
            m: config.m,
            ef_construction: config.ef_construction,
            ef_search: config.ef_search,
            max_elements: config.max_patterns,
        };

        let index = HnswIndex::new(
            config.embedding_dim,
            config.distance_metric.into(),
            hnsw_config,
        )
        .map_err(|e| RuvLLMError::Ruvector(e.to_string()))?;

        Ok(Self {
            config,
            index: Arc::new(RwLock::new(index)),
            patterns: DashMap::new(),
            index_to_pattern: DashMap::new(),
            total_queries: AtomicU64::new(0),
            total_hits: AtomicU64::new(0),
            total_patterns_added: AtomicU64::new(0),
            sona: None,
        })
    }

    /// Get the router configuration
    pub fn config(&self) -> &HnswRouterConfig {
        &self.config
    }

    /// Create with SONA integration for continuous learning
    pub fn with_sona(config: HnswRouterConfig, sona: Arc<RwLock<SonaIntegration>>) -> Result<Self> {
        let mut router = Self::new(config)?;
        router.sona = Some(sona);
        Ok(router)
    }

    /// Add a new pattern to the index
    pub fn add_pattern(&self, pattern: TaskPattern) -> Result<()> {
        // Validate embedding dimension
        if pattern.embedding.len() != self.config.embedding_dim {
            return Err(RuvLLMError::Config(format!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.config.embedding_dim,
                pattern.embedding.len()
            )));
        }

        // Normalize embedding for cosine similarity
        let embedding = self.normalize_embedding(&pattern.embedding);

        // Add to HNSW index
        {
            let mut index = self.index.write();
            index
                .add(pattern.id.clone(), embedding)
                .map_err(|e| RuvLLMError::Ruvector(e.to_string()))?;
        }

        // Store pattern metadata
        self.index_to_pattern
            .insert(pattern.id.clone(), pattern.id.clone());
        self.patterns.insert(pattern.id.clone(), pattern);

        self.total_patterns_added.fetch_add(1, Ordering::SeqCst);

        Ok(())
    }

    /// Add multiple patterns in batch (more efficient)
    pub fn add_patterns(&self, patterns: Vec<TaskPattern>) -> Result<usize> {
        let mut added = 0;
        let mut entries = Vec::with_capacity(patterns.len());

        for pattern in patterns {
            if pattern.embedding.len() != self.config.embedding_dim {
                continue; // Skip invalid patterns
            }

            let embedding = self.normalize_embedding(&pattern.embedding);
            entries.push((pattern.id.clone(), embedding));

            self.index_to_pattern
                .insert(pattern.id.clone(), pattern.id.clone());
            self.patterns.insert(pattern.id.clone(), pattern);
            added += 1;
        }

        if !entries.is_empty() {
            let mut index = self.index.write();
            index
                .add_batch(entries)
                .map_err(|e| RuvLLMError::Ruvector(e.to_string()))?;
        }

        self.total_patterns_added
            .fetch_add(added as u64, Ordering::SeqCst);

        Ok(added)
    }

    /// Search for similar patterns
    pub fn search_similar(&self, query: &[f32], k: usize) -> Result<Vec<(TaskPattern, f32)>> {
        let start = std::time::Instant::now();

        // Validate and normalize query
        if query.len() != self.config.embedding_dim {
            return Err(RuvLLMError::Config(format!(
                "Query dimension mismatch: expected {}, got {}",
                self.config.embedding_dim,
                query.len()
            )));
        }

        let normalized_query = self.normalize_embedding(query);

        // Search HNSW index
        let results: Vec<SearchResult> = {
            let index = self.index.read();
            index
                .search(&normalized_query, k)
                .map_err(|e| RuvLLMError::Ruvector(e.to_string()))?
        };

        self.total_queries.fetch_add(1, Ordering::SeqCst);

        // Convert to patterns with scores
        let mut pattern_results = Vec::with_capacity(results.len());
        for result in results {
            if let Some(pattern) = self.patterns.get(&result.id) {
                // Convert distance to similarity (1 - distance for cosine)
                let similarity: f32 = 1.0 - result.score.max(0.0_f32).min(2.0_f32);
                pattern_results.push((pattern.clone(), similarity));
            }
        }

        if !pattern_results.is_empty() {
            self.total_hits.fetch_add(1, Ordering::SeqCst);
        }

        let _latency = start.elapsed();

        Ok(pattern_results)
    }

    /// Route a task to the optimal agent based on semantic similarity
    pub fn route_by_similarity(&self, query_embedding: &[f32]) -> Result<HnswRoutingResult> {
        let start = std::time::Instant::now();

        // Search for similar patterns
        let similar_patterns = self.search_similar(query_embedding, self.config.top_k)?;

        if similar_patterns.is_empty() {
            return Ok(HnswRoutingResult {
                primary_agent: AgentType::Coder, // Default
                confidence: self.config.min_confidence,
                task_type: ClaudeFlowTask::CodeGeneration,
                patterns_considered: 0,
                alternatives: Vec::new(),
                neighbor_distances: Vec::new(),
                search_latency_us: start.elapsed().as_micros() as u64,
                reasoning: "No similar patterns found, using default".to_string(),
            });
        }

        // Pre-allocate with expected capacity to avoid reallocations
        let patterns_len = similar_patterns.len();
        let mut agent_scores: HashMap<AgentType, f32> = HashMap::with_capacity(8);
        let mut task_type_scores: HashMap<ClaudeFlowTask, f32> = HashMap::with_capacity(8);
        let mut neighbor_distances = Vec::with_capacity(patterns_len);

        // Cache min_usage_for_trust to avoid repeated field access
        let min_usage = self.config.min_usage_for_trust;

        for (pattern, similarity) in &similar_patterns {
            let pattern_confidence = pattern.confidence(min_usage);
            let weight = similarity * pattern_confidence;

            *agent_scores.entry(pattern.agent_type).or_insert(0.0) += weight;
            *task_type_scores.entry(pattern.task_type).or_insert(0.0) += weight;
            neighbor_distances.push(*similarity);
        }

        // Find best agent
        let (primary_agent, primary_score) = agent_scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(a, s)| (*a, *s))
            .unwrap_or((AgentType::Coder, 0.0));

        // Calculate confidence
        let total_score: f32 = agent_scores.values().sum();
        let confidence = if total_score > 0.0 {
            (primary_score / total_score).min(0.99)
        } else {
            self.config.min_confidence
        };

        // Get alternatives
        let mut alternatives: Vec<(AgentType, f32)> = agent_scores
            .into_iter()
            .filter(|(a, _)| *a != primary_agent)
            .map(|(a, s)| (a, s / total_score.max(0.01)))
            .collect();
        alternatives.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        alternatives.truncate(3);

        // Find best task type
        let task_type = task_type_scores
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(t, _)| t)
            .unwrap_or(ClaudeFlowTask::CodeGeneration);

        let latency_us = start.elapsed().as_micros() as u64;

        Ok(HnswRoutingResult {
            primary_agent,
            confidence,
            task_type,
            patterns_considered: similar_patterns.len(),
            alternatives,
            neighbor_distances,
            search_latency_us: latency_us,
            reasoning: format!(
                "HNSW semantic match: {} patterns, confidence {:.2}, latency {}us",
                similar_patterns.len(),
                confidence,
                latency_us
            ),
        })
    }

    /// Update success rate for a pattern
    pub fn update_success_rate(&self, pattern_id: &str, success: bool) -> Result<bool> {
        if let Some(mut pattern) = self.patterns.get_mut(pattern_id) {
            pattern.update_success(success, self.config.success_rate_decay);

            // Record trajectory for SONA if available
            if let Some(sona) = &self.sona {
                let trajectory = Trajectory {
                    request_id: uuid::Uuid::new_v4().to_string(),
                    session_id: "hnsw-router".to_string(),
                    query_embedding: pattern.embedding.clone(),
                    response_embedding: pattern.embedding.clone(),
                    quality_score: if success { 0.9 } else { 0.3 },
                    routing_features: vec![
                        pattern.agent_type as u8 as f32 / 10.0,
                        pattern.success_rate,
                    ],
                    model_index: pattern.agent_type as usize,
                    timestamp: chrono::Utc::now(),
                };

                let sona_guard = sona.read();
                let _ = sona_guard.record_trajectory(trajectory);
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Update success rate by finding the nearest pattern to a query
    pub fn update_nearest_success(&self, query_embedding: &[f32], success: bool) -> Result<bool> {
        let similar = self.search_similar(query_embedding, 1)?;

        if let Some((pattern, similarity)) = similar.first() {
            // Only update if similarity is high enough
            if *similarity > 0.8 {
                return self.update_success_rate(&pattern.id, success);
            }
        }

        Ok(false)
    }

    /// Learn a new pattern from a successful task
    pub fn learn_pattern(
        &self,
        embedding: Vec<f32>,
        agent_type: AgentType,
        task_type: ClaudeFlowTask,
        task_description: String,
        success: bool,
    ) -> Result<Option<String>> {
        if !self.config.enable_online_learning {
            return Ok(None);
        }

        // Check if we already have a very similar pattern
        let similar = self.search_similar(&embedding, 1)?;

        if let Some((existing, similarity)) = similar.first() {
            if *similarity > 0.95 {
                // Update existing pattern instead of adding new one
                self.update_success_rate(&existing.id, success)?;
                return Ok(Some(existing.id.clone()));
            }
        }

        // Add new pattern
        let mut pattern = TaskPattern::new(embedding, agent_type, task_type, task_description);

        if success {
            pattern.success_count = 1;
            pattern.usage_count = 1;
            pattern.success_rate = 0.75; // Start with higher rate for successful task
        } else {
            pattern.usage_count = 1;
            pattern.success_rate = 0.25;
        }

        let pattern_id = pattern.id.clone();
        self.add_pattern(pattern)?;

        Ok(Some(pattern_id))
    }

    /// Remove a pattern from the index
    pub fn remove_pattern(&self, pattern_id: &str) -> Result<bool> {
        if self.patterns.remove(pattern_id).is_some() {
            self.index_to_pattern.remove(pattern_id);

            // Note: HNSW doesn't support true deletion, but we can remove from our metadata
            // The index entry will be ignored on search since pattern won't be found
            let mut index = self.index.write();
            let _ = index.remove(&pattern_id.to_string());

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Prune low-quality and stale patterns
    pub fn prune_patterns(
        &self,
        min_success_rate: f32,
        min_usage: u32,
        max_age_secs: i64,
    ) -> Result<usize> {
        let mut to_remove = Vec::new();

        for entry in self.patterns.iter() {
            let pattern = entry.value();

            // Remove if:
            // 1. Low success rate with enough usage to be confident
            // 2. Too old and never used
            let should_remove = (pattern.usage_count >= min_usage
                && pattern.success_rate < min_success_rate)
                || (pattern.is_stale(max_age_secs) && pattern.usage_count == 0);

            if should_remove {
                to_remove.push(entry.key().clone());
            }
        }

        let removed_count = to_remove.len();
        for id in to_remove {
            self.remove_pattern(&id)?;
        }

        Ok(removed_count)
    }

    /// Consolidate similar patterns
    pub fn consolidate_patterns(&self, similarity_threshold: f32) -> Result<usize> {
        let mut consolidated = 0;
        let mut processed: std::collections::HashSet<String> = std::collections::HashSet::new();

        let pattern_ids: Vec<String> = self.patterns.iter().map(|e| e.key().clone()).collect();

        for id in pattern_ids {
            if processed.contains(&id) {
                continue;
            }

            if let Some(pattern) = self.patterns.get(&id) {
                let similar = self.search_similar(&pattern.embedding, 5)?;

                for (other, similarity) in similar {
                    if other.id != id
                        && similarity > similarity_threshold
                        && !processed.contains(&other.id)
                        && other.agent_type == pattern.agent_type
                    {
                        // Merge: keep the one with higher usage, transfer stats
                        if other.usage_count > pattern.usage_count {
                            // Other is better, update it with our stats
                            if let Some(mut other_mut) = self.patterns.get_mut(&other.id) {
                                other_mut.usage_count += pattern.usage_count;
                                other_mut.success_count += pattern.success_count;
                                // Recalculate success rate
                                if other_mut.usage_count > 0 {
                                    other_mut.success_rate = other_mut.success_count as f32
                                        / other_mut.usage_count as f32;
                                }
                            }
                            processed.insert(id.clone());
                            self.remove_pattern(&id)?;
                            consolidated += 1;
                            break;
                        } else {
                            // We're better, update ourselves and remove other
                            if let Some(mut current) = self.patterns.get_mut(&id) {
                                current.usage_count += other.usage_count;
                                current.success_count += other.success_count;
                                if current.usage_count > 0 {
                                    current.success_rate =
                                        current.success_count as f32 / current.usage_count as f32;
                                }
                            }
                            processed.insert(other.id.clone());
                            self.remove_pattern(&other.id)?;
                            consolidated += 1;
                        }
                    }
                }
            }

            processed.insert(id);
        }

        Ok(consolidated)
    }

    /// Get router statistics
    pub fn stats(&self) -> HnswRouterStats {
        HnswRouterStats {
            total_patterns: self.patterns.len(),
            total_queries: self.total_queries.load(Ordering::SeqCst),
            total_hits: self.total_hits.load(Ordering::SeqCst),
            hit_rate: {
                let queries = self.total_queries.load(Ordering::SeqCst);
                let hits = self.total_hits.load(Ordering::SeqCst);
                if queries > 0 {
                    hits as f32 / queries as f32
                } else {
                    0.0
                }
            },
            patterns_by_agent: self.count_patterns_by_agent(),
            avg_success_rate: self.calculate_avg_success_rate(),
            config: self.config.clone(),
        }
    }

    /// Get all patterns (for inspection/export)
    pub fn get_all_patterns(&self) -> Vec<TaskPattern> {
        self.patterns
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get pattern by ID
    pub fn get_pattern(&self, id: &str) -> Option<TaskPattern> {
        self.patterns.get(id).map(|p| p.clone())
    }

    /// Serialize the router state to bytes
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let patterns: Vec<TaskPattern> = self.get_all_patterns();
        let state = HnswRouterState {
            config: self.config.clone(),
            patterns,
            total_queries: self.total_queries.load(Ordering::SeqCst),
            total_hits: self.total_hits.load(Ordering::SeqCst),
        };

        bincode::serde::encode_to_vec(&state, bincode::config::standard())
            .map_err(|e| RuvLLMError::Serialization(e.to_string()))
    }

    /// Deserialize and restore router state from bytes
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        let (state, _): (HnswRouterState, usize) =
            bincode::serde::decode_from_slice(bytes, bincode::config::standard())
                .map_err(|e| RuvLLMError::Serialization(e.to_string()))?;

        let mut router = Self::new(state.config)?;

        // Restore patterns
        router.add_patterns(state.patterns)?;

        // Restore stats
        router
            .total_queries
            .store(state.total_queries, Ordering::SeqCst);
        router.total_hits.store(state.total_hits, Ordering::SeqCst);

        Ok(router)
    }

    // Private helper methods

    /// Normalize embedding for cosine similarity
    /// Uses SIMD-friendly operations where possible
    #[inline]
    fn normalize_embedding(&self, embedding: &[f32]) -> Vec<f32> {
        if self.config.distance_metric != HnswDistanceMetric::Cosine {
            return embedding.to_vec();
        }

        // Compute squared norm in single pass
        let mut norm_sq: f32 = 0.0;
        for &x in embedding {
            norm_sq += x * x;
        }

        let norm = norm_sq.sqrt();
        if norm > 1e-8 {
            // Pre-compute inverse to avoid repeated division
            let inv_norm = 1.0 / norm;
            embedding.iter().map(|&x| x * inv_norm).collect()
        } else {
            embedding.to_vec()
        }
    }

    #[inline]
    fn count_patterns_by_agent(&self) -> HashMap<AgentType, usize> {
        let mut counts = HashMap::with_capacity(16); // Pre-allocate for typical agent count
        for entry in self.patterns.iter() {
            *counts.entry(entry.value().agent_type).or_insert(0) += 1;
        }
        counts
    }

    #[inline]
    fn calculate_avg_success_rate(&self) -> f32 {
        let mut total = 0.0;
        let mut count = 0;
        for entry in self.patterns.iter() {
            if entry.value().usage_count >= self.config.min_usage_for_trust {
                total += entry.value().success_rate;
                count += 1;
            }
        }
        if count > 0 {
            total / count as f32
        } else {
            0.0
        }
    }
}

/// HNSW router statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswRouterStats {
    /// Total patterns in index
    pub total_patterns: usize,
    /// Total queries processed
    pub total_queries: u64,
    /// Total queries with hits
    pub total_hits: u64,
    /// Hit rate (0.0 - 1.0)
    pub hit_rate: f32,
    /// Patterns by agent type
    pub patterns_by_agent: HashMap<AgentType, usize>,
    /// Average success rate of trusted patterns
    pub avg_success_rate: f32,
    /// Current configuration
    pub config: HnswRouterConfig,
}

/// Hybrid router combining keyword-based AgentRouter with HNSW semantic search
pub struct HybridRouter {
    /// HNSW router for semantic search
    hnsw: HnswRouter,
    /// Keyword weight (0.0 = pure semantic, 1.0 = pure keyword)
    keyword_weight: f32,
    /// Minimum HNSW confidence to trust semantic routing
    min_hnsw_confidence: f32,
}

impl HybridRouter {
    /// Create a new hybrid router
    pub fn new(config: HnswRouterConfig) -> Result<Self> {
        Ok(Self {
            hnsw: HnswRouter::new(config)?,
            keyword_weight: 0.3,
            min_hnsw_confidence: 0.6,
        })
    }

    /// Route using both keyword and semantic methods
    pub fn route(
        &self,
        task_description: &str,
        embedding: &[f32],
        keyword_decision: Option<RoutingDecision>,
    ) -> Result<RoutingDecision> {
        // Get HNSW semantic routing
        let hnsw_result = self.hnsw.route_by_similarity(embedding)?;

        // If no keyword decision provided, use pure semantic
        let keyword = match keyword_decision {
            Some(kw) => kw,
            None => return Ok(hnsw_result.into()),
        };

        // If HNSW has high confidence, prefer it
        if hnsw_result.confidence > self.min_hnsw_confidence && hnsw_result.patterns_considered >= 3
        {
            return Ok(hnsw_result.into());
        }

        // Blend decisions based on weights
        let hnsw_weight = 1.0 - self.keyword_weight;

        // If both agree, high confidence
        if hnsw_result.primary_agent == keyword.primary_agent {
            return Ok(RoutingDecision {
                primary_agent: hnsw_result.primary_agent,
                confidence: (hnsw_result.confidence * hnsw_weight
                    + keyword.confidence * self.keyword_weight)
                    .min(0.99),
                task_type: hnsw_result.task_type,
                alternatives: hnsw_result.alternatives,
                reasoning: format!(
                    "Hybrid: keyword + HNSW agree on {:?}",
                    hnsw_result.primary_agent
                ),
                learned_patterns: hnsw_result.patterns_considered,
            });
        }

        // Disagreement: prefer based on confidence and weights
        let hnsw_score = hnsw_result.confidence * hnsw_weight;
        let keyword_score = keyword.confidence * self.keyword_weight;

        if hnsw_score > keyword_score {
            Ok(hnsw_result.into())
        } else {
            Ok(keyword)
        }
    }

    /// Get HNSW router for direct access
    pub fn hnsw(&self) -> &HnswRouter {
        &self.hnsw
    }

    /// Set keyword weight
    pub fn set_keyword_weight(&mut self, weight: f32) {
        self.keyword_weight = weight.clamp(0.0, 1.0);
    }

    /// Set minimum HNSW confidence
    pub fn set_min_hnsw_confidence(&mut self, confidence: f32) {
        self.min_hnsw_confidence = confidence.clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_embedding(seed: usize, dim: usize) -> Vec<f32> {
        (0..dim)
            .map(|i| ((i + seed) as f32 / dim as f32).sin())
            .collect()
    }

    #[test]
    fn test_hnsw_router_creation() {
        let config = HnswRouterConfig::default();
        let router = HnswRouter::new(config).unwrap();

        let stats = router.stats();
        assert_eq!(stats.total_patterns, 0);
        assert_eq!(stats.total_queries, 0);
    }

    #[test]
    fn test_add_and_search_pattern() {
        let config = HnswRouterConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let router = HnswRouter::new(config).unwrap();

        // Add a pattern
        let embedding = create_test_embedding(42, 128);
        let pattern = TaskPattern::new(
            embedding.clone(),
            AgentType::Coder,
            ClaudeFlowTask::CodeGeneration,
            "implement a function".to_string(),
        );

        router.add_pattern(pattern).unwrap();

        // Search for similar
        let results = router.search_similar(&embedding, 5).unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].0.agent_type, AgentType::Coder);
        assert!(results[0].1 > 0.99); // Should be nearly identical
    }

    #[test]
    fn test_route_by_similarity() {
        let config = HnswRouterConfig {
            embedding_dim: 128,
            min_usage_for_trust: 1,
            ..Default::default()
        };
        let router = HnswRouter::new(config).unwrap();

        // Add patterns for different agents
        for i in 0..10 {
            let embedding = create_test_embedding(i * 100, 128);
            let agent_type = if i < 5 {
                AgentType::Coder
            } else {
                AgentType::Tester
            };
            let task_type = if i < 5 {
                ClaudeFlowTask::CodeGeneration
            } else {
                ClaudeFlowTask::Testing
            };

            let mut pattern =
                TaskPattern::new(embedding, agent_type, task_type, format!("task {}", i));
            pattern.usage_count = 10;
            pattern.success_count = 8;
            pattern.success_rate = 0.8;

            router.add_pattern(pattern).unwrap();
        }

        // Query similar to coder patterns
        let query = create_test_embedding(150, 128); // Between coder embeddings
        let result = router.route_by_similarity(&query).unwrap();

        assert!(result.confidence > 0.0);
        assert!(result.search_latency_us < 10_000); // Should be fast
    }

    #[test]
    fn test_update_success_rate() {
        let config = HnswRouterConfig {
            embedding_dim: 128,
            success_rate_decay: 0.1,
            ..Default::default()
        };
        let router = HnswRouter::new(config).unwrap();

        let embedding = create_test_embedding(42, 128);
        let pattern = TaskPattern::new(
            embedding,
            AgentType::Coder,
            ClaudeFlowTask::CodeGeneration,
            "test task".to_string(),
        );
        let pattern_id = pattern.id.clone();

        router.add_pattern(pattern).unwrap();

        // Update success rate
        router.update_success_rate(&pattern_id, true).unwrap();
        router.update_success_rate(&pattern_id, true).unwrap();
        router.update_success_rate(&pattern_id, false).unwrap();

        let updated_pattern = router.get_pattern(&pattern_id).unwrap();
        assert_eq!(updated_pattern.usage_count, 3);
        assert_eq!(updated_pattern.success_count, 2);
    }

    #[test]
    fn test_learn_pattern() {
        let config = HnswRouterConfig {
            embedding_dim: 128,
            enable_online_learning: true,
            ..Default::default()
        };
        let router = HnswRouter::new(config).unwrap();

        // Learn a new pattern
        let embedding = create_test_embedding(42, 128);
        let pattern_id = router
            .learn_pattern(
                embedding.clone(),
                AgentType::Researcher,
                ClaudeFlowTask::Research,
                "research best practices".to_string(),
                true,
            )
            .unwrap();

        assert!(pattern_id.is_some());

        let stats = router.stats();
        assert_eq!(stats.total_patterns, 1);
        assert_eq!(
            *stats.patterns_by_agent.get(&AgentType::Researcher).unwrap(),
            1
        );
    }

    #[test]
    fn test_prune_patterns() {
        let config = HnswRouterConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let router = HnswRouter::new(config).unwrap();

        // Add low-quality pattern
        let embedding = create_test_embedding(42, 128);
        let mut pattern = TaskPattern::new(
            embedding,
            AgentType::Coder,
            ClaudeFlowTask::CodeGeneration,
            "bad task".to_string(),
        );
        pattern.usage_count = 100;
        pattern.success_count = 10;
        pattern.success_rate = 0.1; // Low success rate

        router.add_pattern(pattern).unwrap();

        // Add good pattern
        let embedding2 = create_test_embedding(100, 128);
        let mut pattern2 = TaskPattern::new(
            embedding2,
            AgentType::Coder,
            ClaudeFlowTask::CodeGeneration,
            "good task".to_string(),
        );
        pattern2.usage_count = 100;
        pattern2.success_count = 90;
        pattern2.success_rate = 0.9;

        router.add_pattern(pattern2).unwrap();

        // Prune low-quality
        let pruned = router.prune_patterns(0.3, 50, 86400).unwrap();

        assert_eq!(pruned, 1);
        assert_eq!(router.stats().total_patterns, 1);
    }

    #[test]
    fn test_serialization() {
        let config = HnswRouterConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let router = HnswRouter::new(config).unwrap();

        // Add some patterns
        for i in 0..5 {
            let embedding = create_test_embedding(i * 10, 128);
            let pattern = TaskPattern::new(
                embedding,
                AgentType::Coder,
                ClaudeFlowTask::CodeGeneration,
                format!("task {}", i),
            );
            router.add_pattern(pattern).unwrap();
        }

        // Serialize
        let bytes = router.serialize().unwrap();

        // Deserialize
        let restored = HnswRouter::deserialize(&bytes).unwrap();

        assert_eq!(restored.stats().total_patterns, 5);
    }

    #[test]
    fn test_config_presets() {
        let fast = HnswRouterConfig::fast();
        assert_eq!(fast.m, 16);
        assert_eq!(fast.ef_search, 50);

        let high_recall = HnswRouterConfig::high_recall();
        assert_eq!(high_recall.m, 48);
        assert_eq!(high_recall.ef_search, 200);
    }

    #[test]
    fn test_hybrid_router() {
        let config = HnswRouterConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let mut router = HybridRouter::new(config).unwrap();

        // Add patterns
        for i in 0..5 {
            let embedding = create_test_embedding(i * 10, 128);
            let pattern = TaskPattern::new(
                embedding,
                AgentType::Coder,
                ClaudeFlowTask::CodeGeneration,
                format!("coding task {}", i),
            );
            router.hnsw.add_pattern(pattern).unwrap();
        }

        // Route with keyword decision
        let query = create_test_embedding(25, 128);
        let keyword_decision = RoutingDecision {
            primary_agent: AgentType::Coder,
            confidence: 0.8,
            alternatives: vec![],
            task_type: ClaudeFlowTask::CodeGeneration,
            reasoning: "keyword match".to_string(),
            learned_patterns: 0,
        };

        let result = router
            .route("implement a function", &query, Some(keyword_decision))
            .unwrap();

        assert_eq!(result.primary_agent, AgentType::Coder);

        // Adjust weights
        router.set_keyword_weight(0.9);
        router.set_min_hnsw_confidence(0.9);
    }
}
