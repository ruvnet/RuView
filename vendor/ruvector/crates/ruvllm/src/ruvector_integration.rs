//! Ruvector Integration Layer
//!
//! This module provides a unified integration layer for all Ruvector capabilities:
//!
//! - **ruvector-core**: HNSW index, vector storage, similarity search
//! - **ruvector-attention**: Flash Attention for efficient inference
//! - **ruvector-graph**: Knowledge graph for relationship learning
//! - **ruvector-gnn**: Graph neural networks for complex reasoning
//! - **ruvector-sona**: SONA (Self-Optimizing Neural Architecture) learning
//!
//! ## Architecture
//!
//! ```text
//! +---------------------+
//! | RuvectorIntegration |
//! |                     |
//! | +-------+ +-------+ |     +---------------+
//! | | HNSW  | | SONA  | |---->| UnifiedIndex  |
//! | +-------+ +-------+ |     +---------------+
//! |                     |            |
//! | +-------+ +-------+ |     +------v--------+
//! | | Flash | | Graph | |---->| Intelligence  |
//! | | Attn  | | +GNN  | |     | Layer         |
//! | +-------+ +-------+ |     +---------------+
//! +---------------------+
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ruvllm::ruvector_integration::{
//!     RuvectorIntegration, IntegrationConfig, UnifiedIndex
//! };
//!
//! // Detect capabilities and create integration
//! let config = IntegrationConfig::default();
//! let integration = RuvectorIntegration::new(config)?;
//!
//! // Create unified index
//! let index = integration.create_unified_index()?;
//!
//! // Route with intelligence
//! let decision = integration.route_with_intelligence("implement auth", &embedding)?;
//!
//! // Learn from outcome
//! integration.learn_from_outcome(&task, decision.agent, true)?;
//! ```

use crate::capabilities::{
    RuvectorCapabilities, ATTENTION_AVAILABLE, GNN_AVAILABLE, GRAPH_AVAILABLE, HNSW_AVAILABLE,
    SONA_AVAILABLE,
};
use crate::claude_flow::{AgentRouter, AgentType};
use crate::error::{Result, RuvLLMError};
use crate::sona::{RoutingRecommendation, SonaConfig, SonaIntegration, SonaStats, Trajectory};
use parking_lot::RwLock;
use ruvector_core::index::hnsw::HnswIndex;
use ruvector_core::index::VectorIndex;
use ruvector_core::types::{DistanceMetric, HnswConfig, VectorId};
use ruvector_sona::{LearnedPattern, PatternConfig, ReasoningBank};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Configuration for the Ruvector integration layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationConfig {
    /// Embedding dimension for vector operations
    pub embedding_dim: usize,
    /// HNSW index configuration
    pub hnsw_config: HnswConfig,
    /// SONA learning configuration
    pub sona_config: SonaConfig,
    /// Distance metric for similarity search
    pub distance_metric: DistanceMetric,
    /// Enable Flash Attention if available
    pub enable_attention: bool,
    /// Enable knowledge graph if available
    pub enable_graph: bool,
    /// Enable GNN reasoning if available
    pub enable_gnn: bool,
    /// Minimum confidence threshold for routing decisions
    pub routing_confidence_threshold: f32,
    /// Maximum patterns to search in ReasoningBank
    pub max_pattern_search: usize,
    /// Learning rate for online adaptation
    pub learning_rate: f32,
    /// EWC lambda for catastrophic forgetting prevention
    pub ewc_lambda: f32,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        let caps = RuvectorCapabilities::detect();
        let (m, ef_construction, ef_search) = caps.recommended_hnsw_params();

        Self {
            embedding_dim: 768,
            hnsw_config: HnswConfig {
                m,
                ef_construction,
                ef_search,
                max_elements: 100_000,
            },
            sona_config: SonaConfig::default(),
            distance_metric: DistanceMetric::Cosine,
            enable_attention: ATTENTION_AVAILABLE,
            enable_graph: GRAPH_AVAILABLE,
            enable_gnn: GNN_AVAILABLE,
            routing_confidence_threshold: 0.6,
            max_pattern_search: 10,
            learning_rate: 0.01,
            ewc_lambda: 0.1,
        }
    }
}

/// Unified index combining HNSW + optional graph + attention
///
/// This provides a single interface for vector operations with optional
/// graph relationships and attention-weighted similarity.
pub struct UnifiedIndex {
    /// HNSW index for approximate nearest neighbor search
    hnsw: Arc<RwLock<HnswIndex>>,
    /// ReasoningBank for pattern storage and retrieval
    reasoning_bank: Arc<RwLock<ReasoningBank>>,
    /// Vector metadata storage
    metadata: Arc<RwLock<HashMap<VectorId, VectorMetadata>>>,
    /// Configuration
    config: IntegrationConfig,
    /// Statistics
    stats: UnifiedIndexStats,
}

/// Metadata associated with indexed vectors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMetadata {
    /// Source of the vector (e.g., "task", "pattern", "agent")
    pub source: String,
    /// Task type if applicable
    pub task_type: Option<String>,
    /// Agent type if applicable
    pub agent_type: Option<AgentType>,
    /// Quality score from learning
    pub quality_score: f32,
    /// Number of times accessed
    pub access_count: u64,
    /// Timestamp of creation
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp of last access
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    /// Custom tags
    pub tags: Vec<String>,
}

impl Default for VectorMetadata {
    fn default() -> Self {
        let now = chrono::Utc::now();
        Self {
            source: "unknown".to_string(),
            task_type: None,
            agent_type: None,
            quality_score: 0.0,
            access_count: 0,
            created_at: now,
            last_accessed: now,
            tags: Vec::new(),
        }
    }
}

/// Statistics for the unified index
#[derive(Debug, Default)]
pub struct UnifiedIndexStats {
    /// Total vectors indexed
    pub total_vectors: AtomicU64,
    /// Total searches performed
    pub total_searches: AtomicU64,
    /// Total successful matches
    pub successful_matches: AtomicU64,
    /// Average search latency in microseconds
    pub avg_search_latency_us: AtomicU64,
    /// Patterns learned
    pub patterns_learned: AtomicU64,
}

impl Clone for UnifiedIndexStats {
    fn clone(&self) -> Self {
        Self {
            total_vectors: AtomicU64::new(self.total_vectors.load(Ordering::Relaxed)),
            total_searches: AtomicU64::new(self.total_searches.load(Ordering::Relaxed)),
            successful_matches: AtomicU64::new(self.successful_matches.load(Ordering::Relaxed)),
            avg_search_latency_us: AtomicU64::new(
                self.avg_search_latency_us.load(Ordering::Relaxed),
            ),
            patterns_learned: AtomicU64::new(self.patterns_learned.load(Ordering::Relaxed)),
        }
    }
}

impl UnifiedIndex {
    /// Create a new unified index
    pub fn new(config: IntegrationConfig) -> Result<Self> {
        let hnsw = HnswIndex::new(
            config.embedding_dim,
            config.distance_metric,
            config.hnsw_config.clone(),
        )
        .map_err(|e| RuvLLMError::Ruvector(e.to_string()))?;

        let pattern_config = PatternConfig {
            k_clusters: 100,
            embedding_dim: config.embedding_dim.min(256),
            max_trajectories: 10000,
            quality_threshold: config.routing_confidence_threshold,
            ..Default::default()
        };

        let reasoning_bank = ReasoningBank::new(pattern_config);

        Ok(Self {
            hnsw: Arc::new(RwLock::new(hnsw)),
            reasoning_bank: Arc::new(RwLock::new(reasoning_bank)),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            config,
            stats: UnifiedIndexStats::default(),
        })
    }

    /// Add a vector to the index
    pub fn add(&self, id: VectorId, vector: Vec<f32>, metadata: VectorMetadata) -> Result<()> {
        // Add to HNSW index
        {
            let mut hnsw = self.hnsw.write();
            hnsw.add(id.clone(), vector)?;
        }

        // Store metadata
        {
            let mut meta = self.metadata.write();
            meta.insert(id, metadata);
        }

        self.stats.total_vectors.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Add a batch of vectors
    pub fn add_batch(&self, entries: Vec<(VectorId, Vec<f32>, VectorMetadata)>) -> Result<()> {
        let vectors: Vec<(VectorId, Vec<f32>)> = entries
            .iter()
            .map(|(id, vec, _)| (id.clone(), vec.clone()))
            .collect();

        // Add to HNSW index
        {
            let mut hnsw = self.hnsw.write();
            hnsw.add_batch(vectors)?;
        }

        // Store metadata
        {
            let mut meta = self.metadata.write();
            for (id, _, metadata) in entries.iter() {
                meta.insert(id.clone(), metadata.clone());
            }
        }

        self.stats
            .total_vectors
            .fetch_add(entries.len() as u64, Ordering::SeqCst);
        Ok(())
    }

    /// Search for similar vectors
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResultWithMetadata>> {
        let start = std::time::Instant::now();

        let results = {
            let hnsw = self.hnsw.read();
            hnsw.search(query, k)?
        };

        let metadata = self.metadata.read();
        let enriched: Vec<SearchResultWithMetadata> = results
            .into_iter()
            .map(|r| {
                let meta = metadata.get(&r.id).cloned();
                SearchResultWithMetadata {
                    id: r.id,
                    score: r.score,
                    metadata: meta,
                }
            })
            .collect();

        let latency = start.elapsed().as_micros() as u64;
        self.stats.total_searches.fetch_add(1, Ordering::SeqCst);

        // Update running average
        let current_avg = self.stats.avg_search_latency_us.load(Ordering::SeqCst);
        let searches = self.stats.total_searches.load(Ordering::SeqCst);
        let new_avg = (current_avg * (searches - 1) + latency) / searches;
        self.stats
            .avg_search_latency_us
            .store(new_avg, Ordering::SeqCst);

        if !enriched.is_empty() {
            self.stats.successful_matches.fetch_add(1, Ordering::SeqCst);
        }

        Ok(enriched)
    }

    /// Search with attention-weighted similarity (if available)
    #[cfg(feature = "attention")]
    pub fn search_with_attention(
        &self,
        query: &[f32],
        k: usize,
        attention_context: Option<&[f32]>,
    ) -> Result<Vec<SearchResultWithMetadata>> {
        // Apply attention-weighted transformation if context provided
        let effective_query = if let Some(ctx) = attention_context {
            // Simplified attention: weighted combination
            let alpha = 0.7; // Query weight
            query
                .iter()
                .zip(ctx.iter())
                .map(|(q, c)| alpha * q + (1.0 - alpha) * c)
                .collect::<Vec<_>>()
        } else {
            query.to_vec()
        };

        self.search(&effective_query, k)
    }

    /// Search without attention (fallback)
    #[cfg(not(feature = "attention"))]
    pub fn search_with_attention(
        &self,
        query: &[f32],
        k: usize,
        _attention_context: Option<&[f32]>,
    ) -> Result<Vec<SearchResultWithMetadata>> {
        self.search(query, k)
    }

    /// Get index statistics
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            total_vectors: self.stats.total_vectors.load(Ordering::SeqCst),
            total_searches: self.stats.total_searches.load(Ordering::SeqCst),
            successful_matches: self.stats.successful_matches.load(Ordering::SeqCst),
            avg_search_latency_us: self.stats.avg_search_latency_us.load(Ordering::SeqCst),
            patterns_learned: self.stats.patterns_learned.load(Ordering::SeqCst),
            hnsw_config: self.config.hnsw_config.clone(),
        }
    }

    /// Get underlying ReasoningBank for pattern operations
    pub fn reasoning_bank(&self) -> &Arc<RwLock<ReasoningBank>> {
        &self.reasoning_bank
    }
}

/// Search result with associated metadata
#[derive(Debug, Clone)]
pub struct SearchResultWithMetadata {
    /// Vector ID
    pub id: VectorId,
    /// Similarity score
    pub score: f32,
    /// Associated metadata
    pub metadata: Option<VectorMetadata>,
}

/// Index statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    /// Total vectors indexed
    pub total_vectors: u64,
    /// Total searches performed
    pub total_searches: u64,
    /// Total successful matches
    pub successful_matches: u64,
    /// Average search latency in microseconds
    pub avg_search_latency_us: u64,
    /// Patterns learned
    pub patterns_learned: u64,
    /// HNSW configuration
    pub hnsw_config: HnswConfig,
}

/// Intelligence layer combining SONA + ReasoningBank + HNSW routing
///
/// This provides the core intelligence capabilities for agent routing
/// and continuous learning.
pub struct IntelligenceLayer {
    /// SONA integration for learning
    sona: Arc<RwLock<SonaIntegration>>,
    /// Agent router for task routing
    router: Arc<RwLock<AgentRouter>>,
    /// Unified index for pattern matching
    index: Arc<UnifiedIndex>,
    /// Configuration
    config: IntegrationConfig,
    /// Statistics
    stats: IntelligenceStats,
}

/// Statistics for the intelligence layer
#[derive(Debug, Default)]
pub struct IntelligenceStats {
    /// Total routing decisions
    pub routing_decisions: AtomicU64,
    /// Successful routings
    pub successful_routings: AtomicU64,
    /// Pattern-based routings
    pub pattern_based_routings: AtomicU64,
    /// Learning updates
    pub learning_updates: AtomicU64,
    /// EWC consolidations
    pub ewc_consolidations: AtomicU64,
}

impl Clone for IntelligenceStats {
    fn clone(&self) -> Self {
        Self {
            routing_decisions: AtomicU64::new(self.routing_decisions.load(Ordering::Relaxed)),
            successful_routings: AtomicU64::new(self.successful_routings.load(Ordering::Relaxed)),
            pattern_based_routings: AtomicU64::new(
                self.pattern_based_routings.load(Ordering::Relaxed),
            ),
            learning_updates: AtomicU64::new(self.learning_updates.load(Ordering::Relaxed)),
            ewc_consolidations: AtomicU64::new(self.ewc_consolidations.load(Ordering::Relaxed)),
        }
    }
}

/// Routing decision with reasoning
#[derive(Debug, Clone)]
pub struct IntelligentRoutingDecision {
    /// Recommended agent type
    pub agent: AgentType,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Alternative agents with scores
    pub alternatives: Vec<(AgentType, f32)>,
    /// Reasoning chain
    pub reasoning: Vec<String>,
    /// Patterns that influenced the decision
    pub influencing_patterns: Vec<LearnedPattern>,
    /// Was this based on learned patterns?
    pub pattern_based: bool,
    /// Recommended model tier (0=fast, 1=balanced, 2=powerful)
    pub model_tier: usize,
}

impl IntelligenceLayer {
    /// Create a new intelligence layer
    pub fn new(config: IntegrationConfig) -> Result<Self> {
        let sona = SonaIntegration::new(config.sona_config.clone());
        let router = AgentRouter::new(config.sona_config.clone());
        let index = UnifiedIndex::new(config.clone())?;

        Ok(Self {
            sona: Arc::new(RwLock::new(sona)),
            router: Arc::new(RwLock::new(router)),
            index: Arc::new(index),
            config,
            stats: IntelligenceStats::default(),
        })
    }

    /// Route a task to the optimal agent with full reasoning
    pub fn route(&self, task_description: &str, embedding: &[f32]) -> IntelligentRoutingDecision {
        self.stats.routing_decisions.fetch_add(1, Ordering::SeqCst);

        let mut reasoning = Vec::new();

        // Step 1: Get SONA recommendation
        let sona_rec = {
            let sona = self.sona.read();
            sona.get_routing_recommendation(embedding)
        };

        // Step 2: Search for similar patterns in the index
        let similar_results = self
            .index
            .search(embedding, self.config.max_pattern_search)
            .unwrap_or_default();

        // Step 3: Get keyword-based routing
        let keyword_decision = {
            let mut router = self.router.write();
            router.route(task_description, Some(embedding))
        };

        // Collect patterns that influenced the decision
        let mut influencing_patterns: Vec<LearnedPattern> = Vec::new();
        {
            let rb = self.index.reasoning_bank().read();
            let patterns = rb.find_similar(embedding, 5);
            influencing_patterns = patterns.into_iter().cloned().collect();
        }

        reasoning.push(format!(
            "Task analyzed: '{}'",
            task_description.chars().take(50).collect::<String>()
        ));

        // Step 4: Combine signals
        let (agent, confidence, pattern_based) = if sona_rec.based_on_patterns > 0
            && sona_rec.confidence > self.config.routing_confidence_threshold
        {
            self.stats
                .pattern_based_routings
                .fetch_add(1, Ordering::SeqCst);
            reasoning.push(format!(
                "SONA pattern match: {} patterns, avg quality {:.2}",
                sona_rec.based_on_patterns, sona_rec.average_quality
            ));

            let agent = Self::model_index_to_agent(sona_rec.suggested_model);
            (agent, sona_rec.confidence, true)
        } else if !similar_results.is_empty() && similar_results[0].score < 0.3 {
            // High similarity (low distance) to known vectors
            self.stats
                .pattern_based_routings
                .fetch_add(1, Ordering::SeqCst);

            let best = &similar_results[0];
            let agent = best
                .metadata
                .as_ref()
                .and_then(|m| m.agent_type)
                .unwrap_or(keyword_decision.primary_agent);

            reasoning.push(format!(
                "Vector similarity match: score={:.3}, source={}",
                best.score,
                best.metadata
                    .as_ref()
                    .map(|m| m.source.as_str())
                    .unwrap_or("unknown")
            ));

            (agent, 0.8 * (1.0 - best.score), true)
        } else {
            reasoning.push(format!(
                "Keyword routing: matched {} keywords, confidence={:.2}",
                keyword_decision.learned_patterns, keyword_decision.confidence
            ));

            (
                keyword_decision.primary_agent,
                keyword_decision.confidence,
                false,
            )
        };

        // Determine model tier based on task complexity
        let model_tier = Self::determine_model_tier(task_description, confidence);
        reasoning.push(format!(
            "Model tier selected: {} ({})",
            model_tier,
            match model_tier {
                0 => "haiku/fast",
                1 => "sonnet/balanced",
                _ => "opus/powerful",
            }
        ));

        // Build alternatives
        let alternatives = keyword_decision.alternatives;

        IntelligentRoutingDecision {
            agent,
            confidence,
            alternatives,
            reasoning,
            influencing_patterns,
            pattern_based,
            model_tier,
        }
    }

    /// Learn from task outcome
    pub fn learn_from_outcome(
        &self,
        task_description: &str,
        embedding: &[f32],
        agent_used: AgentType,
        success: bool,
        quality_score: f32,
    ) -> Result<()> {
        self.stats.learning_updates.fetch_add(1, Ordering::SeqCst);

        // Record trajectory for SONA learning
        let trajectory = Trajectory {
            request_id: uuid::Uuid::new_v4().to_string(),
            session_id: "ruvector-integration".to_string(),
            query_embedding: embedding.to_vec(),
            response_embedding: embedding.to_vec(),
            quality_score,
            routing_features: vec![
                agent_used as u8 as f32 / 10.0,
                if success { 1.0 } else { 0.0 },
                quality_score,
            ],
            model_index: agent_used as usize,
            timestamp: chrono::Utc::now(),
        };

        {
            let sona = self.sona.read();
            sona.record_trajectory(trajectory)?;
        }

        // Update agent router
        {
            let mut router = self.router.write();
            router.record_feedback(task_description, embedding, agent_used, success);
        }

        // Store successful patterns in the index
        if success && quality_score > self.config.routing_confidence_threshold {
            let metadata = VectorMetadata {
                source: "learning".to_string(),
                task_type: Some(task_description.chars().take(50).collect()),
                agent_type: Some(agent_used),
                quality_score,
                ..Default::default()
            };

            let id = format!("pattern-{}", uuid::Uuid::new_v4());
            self.index.add(id, embedding.to_vec(), metadata)?;

            self.stats
                .successful_routings
                .fetch_add(1, Ordering::SeqCst);
        }

        Ok(())
    }

    /// Trigger background learning loop
    pub fn trigger_background_learning(&self) -> Result<()> {
        let sona = self.sona.read();
        sona.trigger_background_loop()?;
        self.stats.ewc_consolidations.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Trigger deep learning loop
    pub fn trigger_deep_learning(&self) -> Result<()> {
        let sona = self.sona.read();
        sona.trigger_deep_loop()?;
        Ok(())
    }

    /// Get SONA statistics
    pub fn sona_stats(&self) -> SonaStats {
        self.sona.read().stats()
    }

    /// Get intelligence layer statistics
    pub fn stats(&self) -> IntelligenceLayerStats {
        IntelligenceLayerStats {
            routing_decisions: self.stats.routing_decisions.load(Ordering::SeqCst),
            successful_routings: self.stats.successful_routings.load(Ordering::SeqCst),
            pattern_based_routings: self.stats.pattern_based_routings.load(Ordering::SeqCst),
            learning_updates: self.stats.learning_updates.load(Ordering::SeqCst),
            ewc_consolidations: self.stats.ewc_consolidations.load(Ordering::SeqCst),
            sona_stats: self.sona_stats(),
            index_stats: self.index.stats(),
            router_accuracy: self.router.read().accuracy(),
        }
    }

    /// Convert model index to agent type
    fn model_index_to_agent(index: usize) -> AgentType {
        match index {
            0 => AgentType::Coder,
            1 => AgentType::Researcher,
            2 => AgentType::Tester,
            3 => AgentType::Reviewer,
            4 => AgentType::Architect,
            5 => AgentType::Security,
            6 => AgentType::Performance,
            _ => AgentType::Coder,
        }
    }

    /// Determine model tier based on task complexity
    fn determine_model_tier(task: &str, confidence: f32) -> usize {
        let lower = task.to_lowercase();

        // Tier 2 (opus/powerful) for complex tasks
        if lower.contains("architect")
            || lower.contains("design")
            || lower.contains("security")
            || lower.contains("complex")
            || lower.contains("refactor")
        {
            return 2;
        }

        // Tier 0 (haiku/fast) for simple tasks with high confidence
        if confidence > 0.8
            && (lower.contains("simple")
                || lower.contains("fix")
                || lower.contains("typo")
                || lower.contains("format")
                || lower.len() < 50)
        {
            return 0;
        }

        // Tier 1 (sonnet/balanced) by default
        1
    }
}

/// Combined intelligence layer statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceLayerStats {
    /// Total routing decisions
    pub routing_decisions: u64,
    /// Successful routings
    pub successful_routings: u64,
    /// Pattern-based routings
    pub pattern_based_routings: u64,
    /// Learning updates
    pub learning_updates: u64,
    /// EWC consolidations
    pub ewc_consolidations: u64,
    /// SONA statistics
    pub sona_stats: SonaStats,
    /// Index statistics
    pub index_stats: IndexStats,
    /// Router accuracy
    pub router_accuracy: f32,
}

/// Main Ruvector integration entry point
///
/// This struct provides the single entry point for all Ruvector capabilities
/// in RuvLTRA.
pub struct RuvectorIntegration {
    /// Detected capabilities
    capabilities: RuvectorCapabilities,
    /// Integration configuration
    config: IntegrationConfig,
    /// Intelligence layer
    intelligence: IntelligenceLayer,
    /// Unified index
    unified_index: Arc<UnifiedIndex>,
}

impl RuvectorIntegration {
    /// Create a new Ruvector integration
    ///
    /// This initializes all available subsystems based on detected capabilities.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ruvllm::ruvector_integration::{RuvectorIntegration, IntegrationConfig};
    ///
    /// let config = IntegrationConfig::default();
    /// let integration = RuvectorIntegration::new(config)?;
    ///
    /// println!("Capabilities: {}", integration.capabilities_summary());
    /// ```
    pub fn new(config: IntegrationConfig) -> Result<Self> {
        let capabilities = RuvectorCapabilities::detect();
        let intelligence = IntelligenceLayer::new(config.clone())?;
        let unified_index = Arc::new(UnifiedIndex::new(config.clone())?);

        tracing::info!(
            "RuvectorIntegration initialized: {}",
            capabilities.summary()
        );

        Ok(Self {
            capabilities,
            config,
            intelligence,
            unified_index,
        })
    }

    /// Get detected capabilities
    pub fn capabilities(&self) -> &RuvectorCapabilities {
        &self.capabilities
    }

    /// Get capabilities summary string
    pub fn capabilities_summary(&self) -> String {
        self.capabilities.summary()
    }

    /// Create a new unified index with current configuration
    pub fn create_unified_index(&self) -> Result<UnifiedIndex> {
        UnifiedIndex::new(self.config.clone())
    }

    /// Get the shared unified index
    pub fn unified_index(&self) -> &Arc<UnifiedIndex> {
        &self.unified_index
    }

    /// Route with intelligence
    ///
    /// Routes a task to the optimal agent using all available intelligence:
    /// - SONA pattern matching
    /// - HNSW similarity search
    /// - Keyword-based fallback
    ///
    /// # Arguments
    ///
    /// * `task` - Task description
    /// * `embedding` - Task embedding vector
    ///
    /// # Returns
    ///
    /// Intelligent routing decision with reasoning chain
    pub fn route_with_intelligence(
        &self,
        task: &str,
        embedding: &[f32],
    ) -> IntelligentRoutingDecision {
        self.intelligence.route(task, embedding)
    }

    /// Learn from outcome
    ///
    /// Updates all learning systems based on task outcome:
    /// - SONA trajectory learning
    /// - Router Q-learning
    /// - Pattern storage
    ///
    /// # Arguments
    ///
    /// * `task` - Task description
    /// * `embedding` - Task embedding vector
    /// * `agent` - Agent that was used
    /// * `success` - Whether the task succeeded
    /// * `quality` - Quality score (0.0 - 1.0)
    pub fn learn_from_outcome(
        &self,
        task: &str,
        embedding: &[f32],
        agent: AgentType,
        success: bool,
        quality: f32,
    ) -> Result<()> {
        self.intelligence
            .learn_from_outcome(task, embedding, agent, success, quality)
    }

    /// Trigger background learning
    ///
    /// Manually triggers the background learning loop (normally runs hourly).
    pub fn trigger_background_learning(&self) -> Result<()> {
        self.intelligence.trigger_background_learning()
    }

    /// Trigger deep learning
    ///
    /// Manually triggers the deep learning loop (normally runs weekly).
    pub fn trigger_deep_learning(&self) -> Result<()> {
        self.intelligence.trigger_deep_learning()
    }

    /// Get comprehensive statistics
    pub fn stats(&self) -> IntegrationStats {
        IntegrationStats {
            capabilities: self.capabilities,
            intelligence: self.intelligence.stats(),
            index: self.unified_index.stats(),
        }
    }

    /// Search unified index
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResultWithMetadata>> {
        self.unified_index.search(query, k)
    }

    /// Add vector to unified index
    pub fn add_vector(
        &self,
        id: VectorId,
        vector: Vec<f32>,
        metadata: VectorMetadata,
    ) -> Result<()> {
        self.unified_index.add(id, vector, metadata)
    }

    /// Check if feature is available
    pub fn has_feature(&self, feature: &str) -> bool {
        match feature.to_lowercase().as_str() {
            "hnsw" => self.capabilities.hnsw,
            "attention" | "flash" => self.capabilities.attention,
            "graph" => self.capabilities.graph,
            "gnn" => self.capabilities.gnn,
            "sona" => self.capabilities.sona,
            "simd" => self.capabilities.simd,
            "parallel" => self.capabilities.parallel,
            _ => false,
        }
    }

    /// Get feature-gated attention computation
    #[cfg(feature = "attention")]
    pub fn compute_attention(&self, query: &[f32], keys: &[&[f32]], values: &[&[f32]]) -> Vec<f32> {
        use ruvector_attention::{traits::Attention, ScaledDotProductAttention};

        let attention = ScaledDotProductAttention::new(query.len());
        attention.compute(query, keys, values).unwrap_or_default()
    }

    #[cfg(not(feature = "attention"))]
    pub fn compute_attention(
        &self,
        query: &[f32],
        _keys: &[&[f32]],
        values: &[&[f32]],
    ) -> Vec<f32> {
        // Fallback: average of values
        if values.is_empty() {
            return query.to_vec();
        }

        let dim = query.len();
        let mut result = vec![0.0; dim];
        for v in values {
            for (i, val) in v.iter().take(dim).enumerate() {
                result[i] += val;
            }
        }
        for r in &mut result {
            *r /= values.len() as f32;
        }
        result
    }
}

/// Comprehensive integration statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationStats {
    /// Detected capabilities
    pub capabilities: RuvectorCapabilities,
    /// Intelligence layer stats
    pub intelligence: IntelligenceLayerStats,
    /// Unified index stats
    pub index: IndexStats,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_embedding() -> Vec<f32> {
        vec![0.1; 768]
    }

    #[test]
    fn test_integration_config_default() {
        let config = IntegrationConfig::default();
        assert_eq!(config.embedding_dim, 768);
        assert!(config.routing_confidence_threshold > 0.0);
    }

    #[test]
    fn test_unified_index_creation() {
        let config = IntegrationConfig::default();
        let index = UnifiedIndex::new(config).unwrap();
        assert_eq!(index.stats().total_vectors, 0);
    }

    #[test]
    fn test_unified_index_add_and_search() {
        let config = IntegrationConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let index = UnifiedIndex::new(config).unwrap();

        let embedding = vec![0.1; 128];
        let metadata = VectorMetadata {
            source: "test".to_string(),
            ..Default::default()
        };

        index
            .add("test-1".to_string(), embedding.clone(), metadata)
            .unwrap();

        let results = index.search(&embedding, 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "test-1");
    }

    #[test]
    fn test_intelligence_layer_routing() {
        let config = IntegrationConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let intelligence = IntelligenceLayer::new(config).unwrap();

        let embedding = vec![0.1; 128];
        let decision = intelligence.route("implement a REST API", &embedding);

        assert!(decision.confidence > 0.0);
        assert!(!decision.reasoning.is_empty());
    }

    #[test]
    fn test_ruvector_integration() {
        let config = IntegrationConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let integration = RuvectorIntegration::new(config).unwrap();

        assert!(integration.capabilities().hnsw);
        assert!(integration.capabilities().sona);

        let summary = integration.capabilities_summary();
        assert!(summary.contains("HNSW"));
        assert!(summary.contains("SONA"));
    }

    #[test]
    fn test_route_with_intelligence() {
        let config = IntegrationConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let integration = RuvectorIntegration::new(config).unwrap();

        let embedding = vec![0.1; 128];
        let decision = integration.route_with_intelligence("write unit tests", &embedding);

        assert!(decision.confidence > 0.0);
        assert!(decision.model_tier <= 2);
    }

    #[test]
    fn test_learn_from_outcome() {
        let config = IntegrationConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let integration = RuvectorIntegration::new(config).unwrap();

        let embedding = vec![0.1; 128];
        integration
            .learn_from_outcome("test task", &embedding, AgentType::Tester, true, 0.9)
            .unwrap();

        let stats = integration.stats();
        assert_eq!(stats.intelligence.learning_updates, 1);
    }

    #[test]
    fn test_model_tier_determination() {
        // Complex tasks should get tier 2
        assert_eq!(
            IntelligenceLayer::determine_model_tier("architect a microservices system", 0.5),
            2
        );

        // Simple tasks with high confidence should get tier 0
        assert_eq!(IntelligenceLayer::determine_model_tier("fix typo", 0.9), 0);

        // Default should be tier 1
        assert_eq!(
            IntelligenceLayer::determine_model_tier("implement feature", 0.7),
            1
        );
    }

    #[test]
    fn test_has_feature() {
        let config = IntegrationConfig::default();
        let integration = RuvectorIntegration::new(config).unwrap();

        assert!(integration.has_feature("hnsw"));
        assert!(integration.has_feature("sona"));
        assert!(!integration.has_feature("unknown_feature"));
    }
}
