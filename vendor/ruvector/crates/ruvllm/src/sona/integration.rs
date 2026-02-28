//! SONA Learning Integration
//!
//! Integrates RuvLLM with the SONA (Self-Optimizing Neural Architecture) framework
//! for continuous learning and adaptation. SONA provides three learning loops:
//!
//! - **Instant Loop**: Per-request learning (<1ms)
//! - **Background Loop**: Hourly batch learning (~10s)
//! - **Deep Loop**: Weekly consolidation (~10min)
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +-------------------+
//! | Request           |---->| Instant Loop      |
//! | (trajectory)      |     | - Ring buffer     |
//! +-------------------+     | - MicroLoRA       |
//!                           | - Edge weights    |
//!                           +--------+----------+
//!                                    |
//!                                    v (async)
//!                           +--------+----------+
//!                           | Background Loop   |
//!                           | - Router training |
//!                           | - EWC++ Fisher    |
//!                           | - BaseLoRA update |
//!                           +--------+----------+
//!                                    |
//!                                    v (scheduled)
//!                           +--------+----------+
//!                           | Deep Loop         |
//!                           | - Pattern bank    |
//!                           | - Memory prune    |
//!                           | - Knowledge xfer  |
//!                           +-------------------+
//! ```

use crate::error::{Result, RuvLLMError};
use crate::policy_store::{PolicyEntry, PolicySource, PolicyStore, PolicyType};
use crate::witness_log::WitnessEntry;
use parking_lot::RwLock;
use ruvector_sona::{
    EwcConfig, EwcPlusPlus, LearnedPattern, PatternConfig, ReasoningBank,
    SonaConfig as SonaCoreConfig, SonaEngine,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// SONA configuration for RuvLLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonaConfig {
    /// Hidden dimension for LoRA
    pub hidden_dim: usize,
    /// Embedding dimension
    pub embedding_dim: usize,
    /// MicroLoRA rank (1-2 for instant learning)
    pub micro_lora_rank: usize,
    /// BaseLoRA rank (4-8 for background learning)
    pub base_lora_rank: usize,
    /// Learning rate for instant loop
    pub instant_learning_rate: f32,
    /// Learning rate for background loop
    pub background_learning_rate: f32,
    /// EWC lambda (regularization strength)
    pub ewc_lambda: f32,
    /// ReasoningBank capacity
    pub pattern_capacity: usize,
    /// Background loop interval (seconds)
    pub background_interval_secs: u64,
    /// Deep loop interval (seconds)
    pub deep_interval_secs: u64,
    /// Minimum quality threshold for learning
    pub quality_threshold: f32,
}

impl Default for SonaConfig {
    fn default() -> Self {
        Self {
            hidden_dim: 256,
            embedding_dim: 768,
            micro_lora_rank: 2,
            base_lora_rank: 8,
            instant_learning_rate: 0.01,
            background_learning_rate: 0.001,
            ewc_lambda: 0.1,
            pattern_capacity: 10000,
            background_interval_secs: 3600, // 1 hour
            deep_interval_secs: 604800,     // 1 week
            quality_threshold: 0.5,
        }
    }
}

/// Learning loop type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LearningLoop {
    /// Per-request instant learning
    Instant,
    /// Hourly background learning
    Background,
    /// Weekly deep consolidation
    Deep,
}

/// Learning trajectory for SONA
#[derive(Debug, Clone)]
pub struct Trajectory {
    /// Request ID
    pub request_id: String,
    /// Session ID
    pub session_id: String,
    /// Query embedding
    pub query_embedding: Vec<f32>,
    /// Response embedding
    pub response_embedding: Vec<f32>,
    /// Quality score
    pub quality_score: f32,
    /// Routing decision features
    pub routing_features: Vec<f32>,
    /// Model used
    pub model_index: usize,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// SONA integration for RuvLLM
#[derive(Debug)]
pub struct SonaIntegration {
    /// Configuration
    config: SonaConfig,
    /// SONA engine
    engine: Arc<RwLock<SonaEngine>>,
    /// EWC++ for catastrophic forgetting prevention
    ewc: Arc<RwLock<EwcPlusPlus>>,
    /// ReasoningBank for pattern storage
    reasoning_bank: Arc<RwLock<ReasoningBank>>,
    /// Trajectory buffer for instant loop
    trajectory_buffer: Arc<RwLock<Vec<Trajectory>>>,
    /// Total trajectories processed
    total_trajectories: AtomicU64,
    /// Instant loop updates
    instant_updates: AtomicU64,
    /// Background loop updates
    background_updates: AtomicU64,
    /// Deep loop updates
    deep_updates: AtomicU64,
    /// Last background loop timestamp
    last_background: AtomicU64,
    /// Last deep loop timestamp
    last_deep: AtomicU64,
}

impl SonaIntegration {
    /// Create a new SONA integration
    pub fn new(config: SonaConfig) -> Self {
        let core_config = SonaCoreConfig {
            hidden_dim: config.hidden_dim,
            embedding_dim: config.embedding_dim,
            micro_lora_rank: config.micro_lora_rank,
            base_lora_rank: config.base_lora_rank,
            micro_lora_lr: config.instant_learning_rate,
            base_lora_lr: config.background_learning_rate,
            ewc_lambda: config.ewc_lambda,
            quality_threshold: config.quality_threshold,
            ..Default::default()
        };

        let engine = SonaEngine::with_config(core_config);

        let ewc_config = EwcConfig {
            param_count: config.hidden_dim,
            initial_lambda: config.ewc_lambda,
            ..Default::default()
        };
        let ewc = EwcPlusPlus::new(ewc_config);

        let pattern_config = PatternConfig {
            k_clusters: 100,
            embedding_dim: config.embedding_dim.min(256), // PatternConfig uses smaller embedding dim
            max_trajectories: config.pattern_capacity,
            quality_threshold: config.quality_threshold,
            ..Default::default()
        };
        let reasoning_bank = ReasoningBank::new(pattern_config);

        Self {
            config,
            engine: Arc::new(RwLock::new(engine)),
            ewc: Arc::new(RwLock::new(ewc)),
            reasoning_bank: Arc::new(RwLock::new(reasoning_bank)),
            trajectory_buffer: Arc::new(RwLock::new(Vec::new())),
            total_trajectories: AtomicU64::new(0),
            instant_updates: AtomicU64::new(0),
            background_updates: AtomicU64::new(0),
            deep_updates: AtomicU64::new(0),
            last_background: AtomicU64::new(0),
            last_deep: AtomicU64::new(0),
        }
    }

    /// Record a trajectory for learning
    pub fn record_trajectory(&self, trajectory: Trajectory) -> Result<()> {
        self.total_trajectories.fetch_add(1, Ordering::SeqCst);

        // Add to buffer
        {
            let mut buffer = self.trajectory_buffer.write();
            buffer.push(trajectory.clone());
        }

        // Run instant loop if quality is good enough
        if trajectory.quality_score >= self.config.quality_threshold {
            self.run_instant_loop(&trajectory)?;
        }

        // Check if background loop should run
        let now = chrono::Utc::now().timestamp() as u64;
        let last_bg = self.last_background.load(Ordering::SeqCst);
        if now - last_bg >= self.config.background_interval_secs {
            self.trigger_background_loop()?;
        }

        // Check if deep loop should run
        let last_deep = self.last_deep.load(Ordering::SeqCst);
        if now - last_deep >= self.config.deep_interval_secs {
            self.trigger_deep_loop()?;
        }

        Ok(())
    }

    /// Run instant loop (per-request, <1ms target)
    fn run_instant_loop(&self, trajectory: &Trajectory) -> Result<()> {
        let mut engine = self.engine.write();

        // Begin trajectory in SONA engine
        let mut builder = engine.begin_trajectory(trajectory.query_embedding.clone());

        // Add step with routing features
        builder.add_step(
            trajectory.response_embedding.clone(),
            trajectory.routing_features.clone(),
            trajectory.quality_score,
        );

        // End trajectory with final quality
        engine.end_trajectory(builder, trajectory.quality_score);

        self.instant_updates.fetch_add(1, Ordering::SeqCst);

        Ok(())
    }

    /// Trigger background loop (hourly, ~10s target)
    pub fn trigger_background_loop(&self) -> Result<()> {
        let now = chrono::Utc::now().timestamp() as u64;
        self.last_background.store(now, Ordering::SeqCst);

        // Get high-quality trajectories from buffer
        let trajectories: Vec<_> = {
            let buffer = self.trajectory_buffer.read();
            buffer
                .iter()
                .filter(|t| t.quality_score >= self.config.quality_threshold)
                .cloned()
                .collect()
        };

        if trajectories.is_empty() {
            return Ok(());
        }

        // Update EWC++ Fisher information
        {
            let mut ewc = self.ewc.write();
            for traj in &trajectories {
                // Convert trajectory to gradients (simplified)
                let gradients = self.compute_pseudo_gradients(traj);
                ewc.update_fisher(&gradients);
            }
        }

        // Add trajectories to reasoning bank for pattern extraction
        {
            let mut rb = self.reasoning_bank.write();
            for traj in &trajectories {
                // Create a QueryTrajectory from our Trajectory
                let query_traj = ruvector_sona::QueryTrajectory::new(
                    traj.request_id.parse().unwrap_or(0),
                    traj.query_embedding.clone(),
                );
                rb.add_trajectory(&query_traj);
            }
            // Extract patterns periodically
            rb.extract_patterns();
        }

        // Clear old trajectories from buffer
        {
            let mut buffer = self.trajectory_buffer.write();
            let cutoff = chrono::Utc::now() - chrono::Duration::hours(1);
            buffer.retain(|t| t.timestamp > cutoff);
        }

        self.background_updates.fetch_add(1, Ordering::SeqCst);

        Ok(())
    }

    /// Trigger deep loop (weekly, ~10min target)
    pub fn trigger_deep_loop(&self) -> Result<()> {
        let now = chrono::Utc::now().timestamp() as u64;
        self.last_deep.store(now, Ordering::SeqCst);

        // Consolidate similar patterns in reasoning bank
        {
            let mut rb = self.reasoning_bank.write();
            rb.consolidate(0.9); // Merge patterns with >90% similarity
        }

        // Prune low-quality patterns
        {
            let mut rb = self.reasoning_bank.write();
            rb.prune_patterns(
                0.3,    // min_quality
                5,      // min_accesses
                604800, // max_age_secs (1 week)
            );
        }

        self.deep_updates.fetch_add(1, Ordering::SeqCst);

        Ok(())
    }

    /// Compute pseudo-gradients for EWC++ (simplified)
    fn compute_pseudo_gradients(&self, trajectory: &Trajectory) -> Vec<f32> {
        // In production, this would compute actual gradients from the model
        // Here we use a simplified version based on embedding differences
        let mut gradients = vec![0.0; self.config.hidden_dim];

        if trajectory.query_embedding.len() >= self.config.hidden_dim {
            for (i, g) in gradients.iter_mut().enumerate() {
                *g = trajectory.query_embedding[i] * trajectory.quality_score;
            }
        }

        gradients
    }

    /// Search for similar patterns in ReasoningBank
    pub fn search_patterns(&self, query: &[f32], limit: usize) -> Vec<LearnedPattern> {
        let rb = self.reasoning_bank.read();
        rb.find_similar(query, limit).into_iter().cloned().collect()
    }

    /// Apply learned transformations to input
    pub fn apply_transform(&self, input: &[f32]) -> Vec<f32> {
        let engine = self.engine.read();
        let mut output = vec![0.0; input.len()];
        engine.apply_micro_lora(input, &mut output);
        output
    }

    /// Get router recommendations based on learned patterns
    pub fn get_routing_recommendation(&self, query_embedding: &[f32]) -> RoutingRecommendation {
        let patterns = self.search_patterns(query_embedding, 5);

        if patterns.is_empty() {
            return RoutingRecommendation::default();
        }

        // Aggregate recommendations from similar patterns
        let avg_quality: f32 =
            patterns.iter().map(|p| p.avg_quality).sum::<f32>() / patterns.len() as f32;

        // Calculate confidence from pattern similarity
        let confidence = patterns
            .first()
            .map(|p| p.similarity(query_embedding))
            .unwrap_or(0.5);

        RoutingRecommendation {
            suggested_model: if avg_quality > 0.8 {
                0
            } else if avg_quality > 0.6 {
                1
            } else {
                2
            },
            confidence,
            based_on_patterns: patterns.len(),
            average_quality: avg_quality,
        }
    }

    /// Record a witness entry and extract trajectory
    pub fn record_from_witness(&self, entry: &WitnessEntry) -> Result<()> {
        let trajectory = Trajectory {
            request_id: entry.request_id.to_string(),
            session_id: entry.session_id.clone(),
            query_embedding: entry.query_embedding.clone(),
            response_embedding: entry.response_embedding.clone(),
            quality_score: entry.quality_score,
            routing_features: vec![
                entry.routing_decision.temperature,
                entry.routing_decision.top_p,
                entry.routing_decision.confidence,
                entry.routing_decision.context_size as f32 / 4096.0,
            ],
            model_index: match entry.model_used {
                crate::types::ModelSize::Tiny => 0,
                crate::types::ModelSize::Small => 1,
                crate::types::ModelSize::Medium => 2,
                crate::types::ModelSize::Large => 3,
            },
            timestamp: entry.timestamp,
        };

        self.record_trajectory(trajectory)
    }

    /// Export learned patterns to policy store
    pub fn export_to_policy_store(&self, store: &PolicyStore) -> Result<usize> {
        let rb = self.reasoning_bank.read();
        let patterns = rb.get_all_patterns();

        let mut count = 0;
        for pattern in patterns {
            let entry = PolicyEntry {
                id: uuid::Uuid::new_v4(),
                policy_type: PolicyType::Pattern,
                embedding: pattern.centroid.clone(),
                parameters: serde_json::json!({
                    "avg_quality": pattern.avg_quality,
                    "cluster_size": pattern.cluster_size,
                    "pattern_type": format!("{:?}", pattern.pattern_type),
                }),
                confidence: pattern.avg_quality, // Use avg_quality as confidence
                fisher_diagonal: None,
                created_at: chrono::Utc::now(),
                last_accessed: chrono::Utc::now(),
                source: PolicySource::BackgroundLoop,
                tags: vec!["sona".to_string(), "pattern".to_string()],
            };

            store.store(entry)?;
            count += 1;
        }

        Ok(count)
    }

    /// Get statistics
    pub fn stats(&self) -> SonaStats {
        let rb = self.reasoning_bank.read();
        SonaStats {
            total_trajectories: self.total_trajectories.load(Ordering::SeqCst),
            instant_updates: self.instant_updates.load(Ordering::SeqCst),
            background_updates: self.background_updates.load(Ordering::SeqCst),
            deep_updates: self.deep_updates.load(Ordering::SeqCst),
            patterns_learned: rb.pattern_count(),
            buffer_size: self.trajectory_buffer.read().len(),
            last_background_secs_ago: {
                let now = chrono::Utc::now().timestamp() as u64;
                now - self.last_background.load(Ordering::SeqCst)
            },
            last_deep_secs_ago: {
                let now = chrono::Utc::now().timestamp() as u64;
                now - self.last_deep.load(Ordering::SeqCst)
            },
        }
    }
}

/// Routing recommendation from SONA
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutingRecommendation {
    /// Suggested model index (0=tiny, 1=small, 2=medium, 3=large)
    pub suggested_model: usize,
    /// Confidence in recommendation (0.0 - 1.0)
    pub confidence: f32,
    /// Number of patterns used for recommendation
    pub based_on_patterns: usize,
    /// Average quality of similar patterns
    pub average_quality: f32,
}

/// SONA statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SonaStats {
    /// Total trajectories processed
    pub total_trajectories: u64,
    /// Instant loop updates
    pub instant_updates: u64,
    /// Background loop updates
    pub background_updates: u64,
    /// Deep loop updates
    pub deep_updates: u64,
    /// Patterns learned in ReasoningBank
    pub patterns_learned: usize,
    /// Current buffer size
    pub buffer_size: usize,
    /// Seconds since last background loop
    pub last_background_secs_ago: u64,
    /// Seconds since last deep loop
    pub last_deep_secs_ago: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sona_config_default() {
        let config = SonaConfig::default();
        assert_eq!(config.hidden_dim, 256);
        assert_eq!(config.embedding_dim, 768);
        assert_eq!(config.micro_lora_rank, 2);
    }

    #[test]
    fn test_sona_integration_creation() {
        let config = SonaConfig::default();
        let sona = SonaIntegration::new(config);

        let stats = sona.stats();
        assert_eq!(stats.total_trajectories, 0);
        assert_eq!(stats.patterns_learned, 0);
    }

    #[test]
    fn test_routing_recommendation() {
        let config = SonaConfig::default();
        let sona = SonaIntegration::new(config);

        let query = vec![0.1; 256]; // Use smaller embedding for pattern config
        let rec = sona.get_routing_recommendation(&query);

        // With no patterns, should return defaults
        assert_eq!(rec.based_on_patterns, 0);
    }

    #[test]
    fn test_trajectory_recording() {
        let config = SonaConfig {
            quality_threshold: 0.0, // Accept all
            embedding_dim: 256,     // Use smaller embedding
            ..Default::default()
        };
        let sona = SonaIntegration::new(config);

        let trajectory = Trajectory {
            request_id: "req-1".to_string(),
            session_id: "sess-1".to_string(),
            query_embedding: vec![0.1; 256],
            response_embedding: vec![0.2; 256],
            quality_score: 0.8,
            routing_features: vec![0.7, 0.9, 0.5, 0.5],
            model_index: 1,
            timestamp: chrono::Utc::now(),
        };

        sona.record_trajectory(trajectory).unwrap();

        let stats = sona.stats();
        assert_eq!(stats.total_trajectories, 1);
        assert_eq!(stats.instant_updates, 1);
    }
}
