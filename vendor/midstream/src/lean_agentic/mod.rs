//! # Lean Agentic Learning System
//!
//! A revolutionary learning framework combining:
//! - Formal reasoning (Lean-style theorem proving)
//! - Agentic AI (autonomous decision-making)
//! - Stream learning (real-time online adaptation)
//! - Knowledge evolution (dynamic theorem store)
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │              Lean Agentic Learning System               │
//! ├─────────────────────────────────────────────────────────┤
//! │                                                         │
//! │  ┌──────────────┐      ┌──────────────┐               │
//! │  │   Formal     │      │   Agentic    │               │
//! │  │  Reasoning   │◄────►│    Loop      │               │
//! │  │   Engine     │      │  (P-A-O-L)   │               │
//! │  └──────┬───────┘      └──────┬───────┘               │
//! │         │                     │                        │
//! │         │    ┌────────────────▼─────┐                 │
//! │         └───►│  Knowledge Graph &   │                 │
//! │              │   Theorem Store      │                 │
//! │              └────────────┬─────────┘                 │
//! │                           │                            │
//! │              ┌────────────▼─────────┐                 │
//! │              │  Stream Learning &   │                 │
//! │              │  Online Adaptation   │                 │
//! │              └──────────────────────┘                 │
//! └─────────────────────────────────────────────────────────┘
//! ```

pub mod reasoning;
pub mod agent;
pub mod knowledge;
pub mod learning;
pub mod types;
pub mod optimized;
pub mod temporal;
pub mod scheduler;
pub mod attractor;
pub mod temporal_neural;
pub mod strange_loop;

pub use reasoning::{FormalReasoner, Theorem, Proof, ProofStep};
pub use agent::{AgenticLoop, Action, Observation, Plan, LearningSignal};
pub use knowledge::{KnowledgeGraph, TheoremStore, Entity, Relation};
pub use learning::{StreamLearner, OnlineModel, AdaptationStrategy};
pub use types::{AgentState, Context, Reward};
pub use optimized::{
    FeatureCache, BufferPool, PredictionCache, BatchProcessor,
    FastEntityExtractor, fast_hash, simd,
};
pub use temporal::{
    TemporalComparator, Sequence, ComparisonAlgorithm, CacheStats,
};
pub use scheduler::{
    RealtimeScheduler, ScheduledTask, SchedulingPolicy, Priority,
    SchedulableAction, SchedulerStats,
};
pub use attractor::{
    AttractorAnalyzer, BehaviorAttractorAnalyzer, AttractorType,
    AttractorInfo, Trajectory, PhasePoint, BehaviorSummary,
};
pub use temporal_neural::{
    TemporalNeuralSolver, TemporalFormula, TemporalOperator,
    TemporalTrace, TemporalState, VerificationResult,
};
pub use midstreamer_strange_loop::{
    MetaLearner, MetaLevel, MetaKnowledge, StrangeLoop,
    ModificationRule, SafetyConstraint, MetaLearningSummary,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// The main lean agentic system orchestrator
pub struct LeanAgenticSystem {
    /// Formal reasoning engine for verification
    pub reasoner: Arc<RwLock<FormalReasoner>>,

    /// Agentic loop for autonomous decision-making
    pub agent_loop: Arc<RwLock<AgenticLoop>>,

    /// Knowledge graph and theorem store
    pub knowledge: Arc<RwLock<KnowledgeGraph>>,

    /// Stream learning system
    pub learner: Arc<RwLock<StreamLearner>>,

    /// System configuration
    pub config: LeanAgenticConfig,
}

/// Configuration for the lean agentic system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeanAgenticConfig {
    /// Enable formal verification of actions
    pub enable_formal_verification: bool,

    /// Learning rate for online adaptation
    pub learning_rate: f64,

    /// Maximum planning depth
    pub max_planning_depth: usize,

    /// Confidence threshold for action execution
    pub action_threshold: f64,

    /// Enable multi-agent collaboration
    pub enable_multi_agent: bool,

    /// Knowledge graph update frequency
    pub kg_update_freq: u64,
}

impl Default for LeanAgenticConfig {
    fn default() -> Self {
        Self {
            enable_formal_verification: true,
            learning_rate: 0.01,
            max_planning_depth: 5,
            action_threshold: 0.7,
            enable_multi_agent: true,
            kg_update_freq: 100,
        }
    }
}

impl LeanAgenticSystem {
    /// Create a new lean agentic system
    pub fn new(config: LeanAgenticConfig) -> Self {
        Self {
            reasoner: Arc::new(RwLock::new(FormalReasoner::new())),
            agent_loop: Arc::new(RwLock::new(AgenticLoop::new(config.clone()))),
            knowledge: Arc::new(RwLock::new(KnowledgeGraph::new())),
            learner: Arc::new(RwLock::new(StreamLearner::new(config.learning_rate))),
            config,
        }
    }

    /// Process a stream chunk with lean agentic learning
    pub async fn process_stream_chunk(
        &self,
        chunk: &str,
        context: Context,
    ) -> Result<ProcessingResult, LeanAgenticError> {
        // 1. Update knowledge graph with new information
        let mut kg = self.knowledge.write().await;
        let entities = kg.extract_entities(chunk).await?;
        kg.update(entities).await?;
        drop(kg);

        // 2. Agent loop: Plan-Act-Observe-Learn
        let mut agent = self.agent_loop.write().await;
        let plan = agent.plan(&context, chunk).await?;
        let action = agent.select_action(&plan).await?;

        // 3. Formal verification (if enabled)
        if self.config.enable_formal_verification {
            let reasoner = self.reasoner.read().await;
            let proof = reasoner.verify_action(&action, &context).await?;
            if !proof.is_valid() {
                return Err(LeanAgenticError::VerificationFailed(proof));
            }
        }

        // 4. Execute action
        let observation = agent.execute(&action).await?;

        // 5. Online learning and adaptation
        let mut learner = self.learner.write().await;
        let reward = agent.compute_reward(&observation).await?;
        learner.update(&action, reward, chunk).await?;

        // 6. Learn from experience
        agent.learn(LearningSignal {
            action: action.clone(),
            observation: observation.clone(),
            reward,
        }).await?;

        Ok(ProcessingResult {
            action,
            observation,
            reward,
            verified: self.config.enable_formal_verification,
        })
    }

    /// Get system statistics
    pub async fn get_stats(&self) -> SystemStats {
        let kg = self.knowledge.read().await;
        let learner = self.learner.read().await;
        let agent = self.agent_loop.read().await;

        SystemStats {
            total_theorems: kg.theorem_count(),
            total_entities: kg.entity_count(),
            learning_iterations: learner.iteration_count(),
            total_actions: agent.action_count(),
            average_reward: agent.average_reward(),
        }
    }
}

/// Result of processing a stream chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingResult {
    pub action: Action,
    pub observation: Observation,
    pub reward: f64,
    pub verified: bool,
}

/// System statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    pub total_theorems: usize,
    pub total_entities: usize,
    pub learning_iterations: u64,
    pub total_actions: u64,
    pub average_reward: f64,
}

/// Errors that can occur in the lean agentic system
#[derive(Debug, thiserror::Error)]
pub enum LeanAgenticError {
    #[error("Formal verification failed: {0:?}")]
    VerificationFailed(Proof),

    #[error("Planning error: {0}")]
    PlanningError(String),

    #[error("Action execution failed: {0}")]
    ActionExecutionError(String),

    #[error("Learning error: {0}")]
    LearningError(String),

    #[error("Knowledge graph error: {0}")]
    KnowledgeGraphError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lean_agentic_system() {
        let config = LeanAgenticConfig::default();
        let system = LeanAgenticSystem::new(config);

        let context = Context::default();
        let chunk = "Hello, world!";

        let result = system.process_stream_chunk(chunk, context).await;
        assert!(result.is_ok());
    }
}
