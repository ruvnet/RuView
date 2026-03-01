//! Federated Learning for SONA
//!
//! Enable distributed learning across ephemeral agents that share
//! trajectories with a central coordinator.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
//! │  Agent A    │     │  Agent B    │     │  Agent C    │
//! │ (ephemeral) │     │ (ephemeral) │     │ (ephemeral) │
//! └──────┬──────┘     └──────┬──────┘     └──────┬──────┘
//!        │                   │                   │
//!        │    export()       │    export()       │    export()
//!        ▼                   ▼                   ▼
//!   ┌────────────────────────────────────────────────┐
//!   │            Federated Coordinator               │
//!   │         (persistent, large capacity)           │
//!   └────────────────────────────────────────────────┘
//! ```

use super::metrics::TrainingMetrics;
use crate::engine::SonaEngine;
use crate::time_compat::SystemTime;
use crate::types::{LearnedPattern, SonaConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Exported state from an ephemeral agent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentExport {
    /// Agent identifier
    pub agent_id: String,
    /// Exported trajectories (embedding, quality pairs)
    pub trajectories: Vec<TrajectoryExport>,
    /// Agent statistics
    pub stats: AgentExportStats,
    /// Session duration in milliseconds
    pub session_duration_ms: u64,
    /// Export timestamp
    pub timestamp: u64,
}

/// Single trajectory export
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrajectoryExport {
    /// Query embedding
    pub embedding: Vec<f32>,
    /// Quality score
    pub quality: f32,
    /// Model route (if any)
    pub route: Option<String>,
    /// Context identifiers
    pub context: Vec<String>,
    /// Timestamp
    pub timestamp: u64,
}

/// Agent export statistics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentExportStats {
    /// Total trajectories processed
    pub total_trajectories: usize,
    /// Average quality
    pub avg_quality: f32,
    /// Patterns learned locally
    pub patterns_learned: usize,
}

/// Ephemeral agent for federated learning
///
/// Collects trajectories during its session and exports state before termination.
pub struct EphemeralAgent {
    /// Agent identifier
    agent_id: String,
    /// SONA engine
    engine: SonaEngine,
    /// Collected trajectories
    trajectories: Vec<TrajectoryExport>,
    /// Session start time
    start_time: u64,
    /// Quality samples
    quality_samples: Vec<f32>,
}

impl EphemeralAgent {
    /// Create a new ephemeral agent
    pub fn new(agent_id: impl Into<String>, config: SonaConfig) -> Self {
        let now = SystemTime::now().duration_since_epoch().as_millis() as u64;

        Self {
            agent_id: agent_id.into(),
            engine: SonaEngine::with_config(config),
            trajectories: Vec::new(),
            start_time: now,
            quality_samples: Vec::new(),
        }
    }

    /// Create with default config for federated learning
    pub fn default_federated(agent_id: impl Into<String>, hidden_dim: usize) -> Self {
        Self::new(
            agent_id,
            SonaConfig {
                hidden_dim,
                embedding_dim: hidden_dim,
                micro_lora_rank: 2,
                base_lora_rank: 8,
                micro_lora_lr: 0.002,
                trajectory_capacity: 500, // Small buffer per agent
                pattern_clusters: 25,
                ..Default::default()
            },
        )
    }

    /// Get agent ID
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Get engine reference
    pub fn engine(&self) -> &SonaEngine {
        &self.engine
    }

    /// Get mutable engine reference
    pub fn engine_mut(&mut self) -> &mut SonaEngine {
        &mut self.engine
    }

    /// Process a task and record trajectory
    pub fn process_trajectory(
        &mut self,
        embedding: Vec<f32>,
        activations: Vec<f32>,
        quality: f32,
        route: Option<String>,
        context: Vec<String>,
    ) {
        let now = SystemTime::now().duration_since_epoch().as_millis() as u64;

        // Record in SONA engine
        let mut builder = self.engine.begin_trajectory(embedding.clone());
        if let Some(ref r) = route {
            builder.set_model_route(r);
        }
        for ctx in &context {
            builder.add_context(ctx);
        }
        builder.add_step(activations, vec![], quality);
        self.engine.end_trajectory(builder, quality);

        // Store for export
        self.trajectories.push(TrajectoryExport {
            embedding,
            quality,
            route,
            context,
            timestamp: now,
        });

        self.quality_samples.push(quality);
    }

    /// Apply micro-LoRA to hidden states
    pub fn apply_micro_lora(&self, input: &[f32], output: &mut [f32]) {
        self.engine.apply_micro_lora(input, output);
    }

    /// Get number of collected trajectories
    pub fn trajectory_count(&self) -> usize {
        self.trajectories.len()
    }

    /// Get average quality
    pub fn avg_quality(&self) -> f32 {
        if self.quality_samples.is_empty() {
            0.0
        } else {
            self.quality_samples.iter().sum::<f32>() / self.quality_samples.len() as f32
        }
    }

    /// Force local learning
    pub fn force_learn(&self) -> String {
        self.engine.force_learn()
    }

    /// Simple process task method
    pub fn process_task(&mut self, embedding: Vec<f32>, quality: f32) {
        self.process_trajectory(embedding.clone(), embedding, quality, None, vec![]);
    }

    /// Process task with route information
    pub fn process_task_with_route(&mut self, embedding: Vec<f32>, quality: f32, route: &str) {
        self.process_trajectory(
            embedding.clone(),
            embedding,
            quality,
            Some(route.to_string()),
            vec![],
        );
    }

    /// Get average quality (alias for avg_quality)
    pub fn average_quality(&self) -> f32 {
        self.avg_quality()
    }

    /// Get uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        let now = SystemTime::now().duration_since_epoch().as_millis() as u64;
        (now - self.start_time) / 1000
    }

    /// Get agent stats
    pub fn stats(&self) -> AgentExportStats {
        let engine_stats = self.engine.stats();
        AgentExportStats {
            total_trajectories: self.trajectories.len(),
            avg_quality: self.avg_quality(),
            patterns_learned: engine_stats.patterns_stored,
        }
    }

    /// Clear trajectories (after export)
    pub fn clear(&mut self) {
        self.trajectories.clear();
        self.quality_samples.clear();
    }

    /// Get learned patterns from agent
    pub fn get_patterns(&self) -> Vec<LearnedPattern> {
        self.engine.find_patterns(&[], 0)
    }

    /// Export agent state for federation
    ///
    /// Call this before terminating the agent.
    pub fn export_state(&self) -> AgentExport {
        let now = SystemTime::now().duration_since_epoch().as_millis() as u64;

        // Force learning before export
        self.engine.force_learn();

        let stats = self.engine.stats();

        AgentExport {
            agent_id: self.agent_id.clone(),
            trajectories: self.trajectories.clone(),
            stats: AgentExportStats {
                total_trajectories: self.trajectories.len(),
                avg_quality: self.avg_quality(),
                patterns_learned: stats.patterns_stored,
            },
            session_duration_ms: now - self.start_time,
            timestamp: now,
        }
    }
}

/// Agent contribution record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentContribution {
    /// Number of trajectories contributed
    pub trajectory_count: usize,
    /// Average quality of contributions
    pub avg_quality: f32,
    /// Contribution timestamp
    pub timestamp: u64,
    /// Session duration
    pub session_duration_ms: u64,
}

/// Federated learning coordinator
///
/// Aggregates learning from multiple ephemeral agents.
pub struct FederatedCoordinator {
    /// Coordinator identifier
    coordinator_id: String,
    /// Master SONA engine for aggregation
    master_engine: SonaEngine,
    /// Agent contributions
    contributions: HashMap<String, AgentContribution>,
    /// Quality threshold for accepting trajectories
    quality_threshold: f32,
    /// Total trajectories aggregated
    total_trajectories: usize,
    /// Consolidation interval (number of agents)
    consolidation_interval: usize,
    /// Metrics
    metrics: TrainingMetrics,
}

impl FederatedCoordinator {
    /// Create a new federated coordinator
    pub fn new(coordinator_id: impl Into<String>, config: SonaConfig) -> Self {
        let id = coordinator_id.into();
        Self {
            coordinator_id: id.clone(),
            master_engine: SonaEngine::with_config(config),
            contributions: HashMap::new(),
            quality_threshold: 0.4,
            total_trajectories: 0,
            consolidation_interval: 50,
            metrics: TrainingMetrics::new(&id),
        }
    }

    /// Create with default config for coordination
    pub fn default_coordinator(coordinator_id: impl Into<String>, hidden_dim: usize) -> Self {
        Self::new(
            coordinator_id,
            SonaConfig {
                hidden_dim,
                embedding_dim: hidden_dim,
                micro_lora_rank: 2,
                base_lora_rank: 16,         // Deeper for aggregation
                trajectory_capacity: 50000, // Large central buffer
                pattern_clusters: 200,
                ewc_lambda: 2000.0, // Strong regularization
                ..Default::default()
            },
        )
    }

    /// Get coordinator ID
    pub fn coordinator_id(&self) -> &str {
        &self.coordinator_id
    }

    /// Set quality threshold for accepting trajectories
    pub fn set_quality_threshold(&mut self, threshold: f32) {
        self.quality_threshold = threshold;
    }

    /// Set consolidation interval
    pub fn set_consolidation_interval(&mut self, interval: usize) {
        self.consolidation_interval = interval;
    }

    /// Get master engine reference
    pub fn master_engine(&self) -> &SonaEngine {
        &self.master_engine
    }

    /// Aggregate agent export into coordinator
    pub fn aggregate(&mut self, export: AgentExport) -> AggregationResult {
        let mut accepted = 0;
        let mut rejected = 0;

        // Replay trajectories into master engine
        for traj in &export.trajectories {
            if traj.quality >= self.quality_threshold {
                let mut builder = self.master_engine.begin_trajectory(traj.embedding.clone());
                if let Some(ref route) = traj.route {
                    builder.set_model_route(route);
                }
                for ctx in &traj.context {
                    builder.add_context(ctx);
                }
                self.master_engine.end_trajectory(builder, traj.quality);

                self.metrics.add_quality_sample(traj.quality);
                accepted += 1;
            } else {
                rejected += 1;
            }
        }

        self.total_trajectories += accepted;

        // Record contribution
        let now = SystemTime::now().duration_since_epoch().as_millis() as u64;

        self.contributions.insert(
            export.agent_id.clone(),
            AgentContribution {
                trajectory_count: export.trajectories.len(),
                avg_quality: export.stats.avg_quality,
                timestamp: now,
                session_duration_ms: export.session_duration_ms,
            },
        );

        // Auto-consolidate if needed
        let consolidated = if self.should_consolidate() {
            self.master_engine.force_learn();
            true
        } else {
            false
        };

        AggregationResult {
            agent_id: export.agent_id,
            trajectories_accepted: accepted,
            trajectories_rejected: rejected,
            consolidated,
            total_agents: self.contributions.len(),
            total_trajectories: self.total_trajectories,
        }
    }

    /// Check if consolidation is needed
    fn should_consolidate(&self) -> bool {
        self.contributions.len() % self.consolidation_interval == 0
    }

    /// Force consolidation
    pub fn force_consolidate(&self) -> String {
        self.master_engine.force_learn()
    }

    /// Get initial state for new agents
    ///
    /// Returns learned patterns that new agents can use for warm start.
    pub fn get_initial_patterns(&self, k: usize) -> Vec<LearnedPattern> {
        // Find patterns similar to a general query (empty or average)
        // Since we don't have a specific query, get all patterns
        self.master_engine
            .find_patterns(&[], 0)
            .into_iter()
            .take(k)
            .collect()
    }

    /// Get all learned patterns
    pub fn get_all_patterns(&self) -> Vec<LearnedPattern> {
        self.master_engine.find_patterns(&[], 0)
    }

    /// Get coordinator statistics
    pub fn stats(&self) -> CoordinatorStats {
        let engine_stats = self.master_engine.stats();

        CoordinatorStats {
            coordinator_id: self.coordinator_id.clone(),
            total_agents: self.contributions.len(),
            total_trajectories: self.total_trajectories,
            patterns_learned: engine_stats.patterns_stored,
            avg_quality: self.metrics.avg_quality(),
            quality_threshold: self.quality_threshold,
        }
    }

    /// Get contribution history
    pub fn contributions(&self) -> &HashMap<String, AgentContribution> {
        &self.contributions
    }

    /// Get metrics
    pub fn metrics(&self) -> &TrainingMetrics {
        &self.metrics
    }

    /// Get total number of contributing agents
    pub fn agent_count(&self) -> usize {
        self.contributions.len()
    }

    /// Get total trajectories aggregated
    pub fn total_trajectories(&self) -> usize {
        self.total_trajectories
    }

    /// Find similar patterns
    pub fn find_patterns(&self, query: &[f32], k: usize) -> Vec<LearnedPattern> {
        self.master_engine.find_patterns(query, k)
    }

    /// Apply coordinator's LoRA to input
    pub fn apply_lora(&self, input: &[f32]) -> Vec<f32> {
        let mut output = vec![0.0; input.len()];
        self.master_engine.apply_micro_lora(input, &mut output);
        output
    }

    /// Consolidate learning (alias for force_consolidate)
    pub fn consolidate(&self) -> String {
        self.force_consolidate()
    }

    /// Clear all contributions
    pub fn clear(&mut self) {
        self.contributions.clear();
        self.total_trajectories = 0;
    }
}

/// Result of aggregating an agent export
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AggregationResult {
    /// Agent ID that was aggregated
    pub agent_id: String,
    /// Number of trajectories accepted
    pub trajectories_accepted: usize,
    /// Number of trajectories rejected (below quality threshold)
    pub trajectories_rejected: usize,
    /// Whether consolidation was triggered
    pub consolidated: bool,
    /// Total number of contributing agents
    pub total_agents: usize,
    /// Total trajectories in coordinator
    pub total_trajectories: usize,
}

/// Coordinator statistics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoordinatorStats {
    /// Coordinator identifier
    pub coordinator_id: String,
    /// Number of contributing agents
    pub total_agents: usize,
    /// Total trajectories aggregated
    pub total_trajectories: usize,
    /// Patterns learned
    pub patterns_learned: usize,
    /// Average quality across all contributions
    pub avg_quality: f32,
    /// Quality threshold
    pub quality_threshold: f32,
}

impl std::fmt::Display for CoordinatorStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Coordinator(id={}, agents={}, trajectories={}, patterns={}, avg_quality={:.4})",
            self.coordinator_id,
            self.total_agents,
            self.total_trajectories,
            self.patterns_learned,
            self.avg_quality
        )
    }
}

/// Federated learning topology
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum FederatedTopology {
    /// Agents -> Central Coordinator (simple, single aggregation point)
    #[default]
    Star,
    /// Agents -> Regional -> Global (multi-datacenter)
    Hierarchical {
        /// Number of regional coordinators
        regions: usize,
    },
    /// Agents share directly (edge deployment)
    PeerToPeer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ephemeral_agent_creation() {
        let agent = EphemeralAgent::default_federated("agent-1", 256);
        assert_eq!(agent.agent_id(), "agent-1");
        assert_eq!(agent.trajectory_count(), 0);
    }

    #[test]
    fn test_trajectory_collection() {
        let mut agent = EphemeralAgent::default_federated("agent-1", 256);

        agent.process_trajectory(
            vec![0.1; 256],
            vec![0.5; 256],
            0.8,
            Some("code".into()),
            vec!["file:main.rs".into()],
        );

        assert_eq!(agent.trajectory_count(), 1);
        assert!((agent.avg_quality() - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_agent_export() {
        let mut agent = EphemeralAgent::default_federated("agent-1", 256);

        for i in 0..5 {
            agent.process_trajectory(
                vec![i as f32 * 0.1; 256],
                vec![0.5; 256],
                0.7 + i as f32 * 0.05,
                None,
                vec![],
            );
        }

        let export = agent.export_state();
        assert_eq!(export.agent_id, "agent-1");
        assert_eq!(export.trajectories.len(), 5);
        assert!(export.stats.avg_quality > 0.7);
    }

    #[test]
    fn test_coordinator_creation() {
        let coord = FederatedCoordinator::default_coordinator("coord-1", 256);
        assert_eq!(coord.coordinator_id(), "coord-1");

        let stats = coord.stats();
        assert_eq!(stats.total_agents, 0);
        assert_eq!(stats.total_trajectories, 0);
    }

    #[test]
    fn test_aggregation() {
        let mut coord = FederatedCoordinator::default_coordinator("coord-1", 256);
        coord.set_quality_threshold(0.5);

        // Create agent export
        let export = AgentExport {
            agent_id: "agent-1".into(),
            trajectories: vec![
                TrajectoryExport {
                    embedding: vec![0.1; 256],
                    quality: 0.8,
                    route: Some("code".into()),
                    context: vec![],
                    timestamp: 0,
                },
                TrajectoryExport {
                    embedding: vec![0.2; 256],
                    quality: 0.3, // Below threshold
                    route: None,
                    context: vec![],
                    timestamp: 0,
                },
            ],
            stats: AgentExportStats {
                total_trajectories: 2,
                avg_quality: 0.55,
                patterns_learned: 0,
            },
            session_duration_ms: 1000,
            timestamp: 0,
        };

        let result = coord.aggregate(export);
        assert_eq!(result.trajectories_accepted, 1);
        assert_eq!(result.trajectories_rejected, 1);
        assert_eq!(result.total_agents, 1);
    }

    #[test]
    fn test_multi_agent_aggregation() {
        let mut coord = FederatedCoordinator::default_coordinator("coord-1", 256);
        coord.set_consolidation_interval(2); // Consolidate every 2 agents

        for i in 0..3 {
            let export = AgentExport {
                agent_id: format!("agent-{}", i),
                trajectories: vec![TrajectoryExport {
                    embedding: vec![i as f32 * 0.1; 256],
                    quality: 0.8,
                    route: None,
                    context: vec![],
                    timestamp: 0,
                }],
                stats: AgentExportStats::default(),
                session_duration_ms: 1000,
                timestamp: 0,
            };

            let result = coord.aggregate(export);
            // Second agent should trigger consolidation
            if i == 1 {
                assert!(result.consolidated);
            }
        }

        let stats = coord.stats();
        assert_eq!(stats.total_agents, 3);
        assert_eq!(stats.total_trajectories, 3);
    }
}
