//! Agent Router for Claude Flow
//!
//! Routes tasks to optimal agent types using RuvLTRA embeddings and SONA learning.

use super::{ClaudeFlowAgent, ClaudeFlowTask};
use crate::sona::{RoutingRecommendation, SonaConfig, SonaIntegration, Trajectory};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Agent type for routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentType {
    /// Code implementation specialist
    Coder,
    /// Research and analysis
    Researcher,
    /// Testing and validation
    Tester,
    /// Code review and quality
    Reviewer,
    /// System architecture
    Architect,
    /// Security specialist
    Security,
    /// Performance optimization
    Performance,
    /// Machine learning
    MlDeveloper,
}

impl From<ClaudeFlowAgent> for AgentType {
    fn from(agent: ClaudeFlowAgent) -> Self {
        match agent {
            ClaudeFlowAgent::Coder | ClaudeFlowAgent::BackendDev => AgentType::Coder,
            ClaudeFlowAgent::Researcher => AgentType::Researcher,
            ClaudeFlowAgent::Tester => AgentType::Tester,
            ClaudeFlowAgent::Reviewer => AgentType::Reviewer,
            ClaudeFlowAgent::Architect => AgentType::Architect,
            ClaudeFlowAgent::SecurityAuditor => AgentType::Security,
            ClaudeFlowAgent::PerformanceEngineer => AgentType::Performance,
            ClaudeFlowAgent::MlDeveloper => AgentType::MlDeveloper,
            ClaudeFlowAgent::CicdEngineer => AgentType::Coder,
        }
    }
}

/// Routing decision with confidence
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    /// Primary agent recommendation
    pub primary_agent: AgentType,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Alternative agents
    pub alternatives: Vec<(AgentType, f32)>,
    /// Task classification
    pub task_type: ClaudeFlowTask,
    /// Reasoning for decision
    pub reasoning: String,
    /// Based on learned patterns
    pub learned_patterns: usize,
}

/// Agent router using RuvLTRA + SONA
pub struct AgentRouter {
    /// SONA integration for learning
    sona: Arc<RwLock<SonaIntegration>>,
    /// Keyword-based routing cache
    keyword_cache: HashMap<String, AgentType>,
    /// Total routing decisions
    total_decisions: u64,
    /// Successful routings (positive feedback)
    successful_routings: u64,
}

impl AgentRouter {
    /// Create a new agent router
    pub fn new(sona_config: SonaConfig) -> Self {
        Self {
            sona: Arc::new(RwLock::new(SonaIntegration::new(sona_config))),
            keyword_cache: Self::build_keyword_cache(),
            total_decisions: 0,
            successful_routings: 0,
        }
    }

    /// Build keyword to agent mapping
    fn build_keyword_cache() -> HashMap<String, AgentType> {
        let mut cache = HashMap::new();

        for agent in ClaudeFlowAgent::all() {
            let agent_type: AgentType = (*agent).into();
            for keyword in agent.keywords() {
                cache.insert(keyword.to_lowercase(), agent_type);
            }
        }

        cache
    }

    /// Route a task to the optimal agent
    pub fn route(&mut self, task_description: &str, embedding: Option<&[f32]>) -> RoutingDecision {
        self.total_decisions += 1;

        // Try SONA-based routing first if we have an embedding
        if let Some(emb) = embedding {
            let sona = self.sona.read();
            let recommendation = sona.get_routing_recommendation(emb);

            if recommendation.based_on_patterns > 0 && recommendation.confidence > 0.6 {
                return self.sona_to_routing_decision(recommendation, task_description);
            }
        }

        // Fall back to keyword-based routing
        self.keyword_route(task_description)
    }

    /// Route based on keywords in task description
    fn keyword_route(&self, task_description: &str) -> RoutingDecision {
        let lower = task_description.to_lowercase();
        let mut scores: HashMap<AgentType, f32> = HashMap::new();

        // Score each agent based on keyword matches
        for (keyword, agent_type) in &self.keyword_cache {
            if lower.contains(keyword) {
                *scores.entry(*agent_type).or_insert(0.0) += 1.0;
            }
        }

        // Find best match
        let (primary_agent, primary_score) = scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(a, s)| (*a, *s))
            .unwrap_or((AgentType::Coder, 0.0));

        // Calculate confidence
        let total_matches: f32 = scores.values().sum();
        let confidence = if total_matches > 0.0 {
            (primary_score / total_matches).min(0.95)
        } else {
            0.3 // Low confidence default
        };

        // Get alternatives
        let mut alternatives: Vec<(AgentType, f32)> = scores
            .into_iter()
            .filter(|(a, _)| *a != primary_agent)
            .map(|(a, s)| (a, s / total_matches.max(1.0)))
            .collect();
        alternatives.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        alternatives.truncate(3);

        // Determine task type
        let task_type = self.classify_task(&lower);

        RoutingDecision {
            primary_agent,
            confidence,
            alternatives,
            task_type,
            reasoning: format!(
                "Keyword match: {} keywords matched for {:?}",
                primary_score as usize, primary_agent
            ),
            learned_patterns: 0,
        }
    }

    /// Convert SONA recommendation to routing decision
    fn sona_to_routing_decision(&self, rec: RoutingRecommendation, task: &str) -> RoutingDecision {
        let primary_agent = match rec.suggested_model {
            0 => AgentType::Coder,
            1 => AgentType::Researcher,
            2 => AgentType::Tester,
            3 => AgentType::Reviewer,
            _ => AgentType::Coder,
        };

        let task_type = self.classify_task(&task.to_lowercase());

        RoutingDecision {
            primary_agent,
            confidence: rec.confidence,
            alternatives: vec![],
            task_type,
            reasoning: format!(
                "SONA pattern match: {} patterns, avg quality {:.2}",
                rec.based_on_patterns, rec.average_quality
            ),
            learned_patterns: rec.based_on_patterns,
        }
    }

    /// Classify task type from description
    fn classify_task(&self, lower: &str) -> ClaudeFlowTask {
        if lower.contains("test") || lower.contains("verify") || lower.contains("validate") {
            ClaudeFlowTask::Testing
        } else if lower.contains("review") || lower.contains("audit") {
            ClaudeFlowTask::CodeReview
        } else if lower.contains("research")
            || lower.contains("analyze")
            || lower.contains("investigate")
        {
            ClaudeFlowTask::Research
        } else if lower.contains("security") || lower.contains("vulnerability") {
            ClaudeFlowTask::Security
        } else if lower.contains("performance")
            || lower.contains("optimize")
            || lower.contains("benchmark")
        {
            ClaudeFlowTask::Performance
        } else if lower.contains("architecture") || lower.contains("design") {
            ClaudeFlowTask::Architecture
        } else if lower.contains("debug") || lower.contains("fix") || lower.contains("error") {
            ClaudeFlowTask::Debugging
        } else if lower.contains("refactor") || lower.contains("clean") {
            ClaudeFlowTask::Refactoring
        } else if lower.contains("document") || lower.contains("readme") {
            ClaudeFlowTask::Documentation
        } else {
            ClaudeFlowTask::CodeGeneration
        }
    }

    /// Record feedback for learning
    pub fn record_feedback(
        &mut self,
        task: &str,
        embedding: &[f32],
        agent_used: AgentType,
        success: bool,
    ) {
        if success {
            self.successful_routings += 1;
        }

        // Record trajectory for SONA learning
        let trajectory = Trajectory {
            request_id: uuid::Uuid::new_v4().to_string(),
            session_id: "claude-flow".to_string(),
            query_embedding: embedding.to_vec(),
            response_embedding: embedding.to_vec(), // Simplified
            quality_score: if success { 0.9 } else { 0.3 },
            routing_features: vec![
                agent_used as u8 as f32 / 10.0,
                if success { 1.0 } else { 0.0 },
            ],
            model_index: agent_used as usize,
            timestamp: chrono::Utc::now(),
        };

        let sona = self.sona.read();
        let _ = sona.record_trajectory(trajectory);
    }

    /// Get routing accuracy
    pub fn accuracy(&self) -> f32 {
        if self.total_decisions == 0 {
            0.0
        } else {
            self.successful_routings as f32 / self.total_decisions as f32
        }
    }

    /// Get SONA stats
    pub fn sona_stats(&self) -> crate::sona::SonaStats {
        self.sona.read().stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_routing() {
        let config = SonaConfig::default();
        let mut router = AgentRouter::new(config);

        let decision = router.route("implement a new REST API endpoint", None);
        assert_eq!(decision.primary_agent, AgentType::Coder);

        let decision = router.route("research best practices for authentication", None);
        assert_eq!(decision.primary_agent, AgentType::Researcher);

        let decision = router.route("write unit tests for the user service", None);
        assert_eq!(decision.primary_agent, AgentType::Tester);
    }

    #[test]
    fn test_task_classification() {
        let config = SonaConfig::default();
        let router = AgentRouter::new(config);

        assert_eq!(router.classify_task("write tests"), ClaudeFlowTask::Testing);
        assert_eq!(
            router.classify_task("review code"),
            ClaudeFlowTask::CodeReview
        );
        assert_eq!(
            router.classify_task("optimize performance"),
            ClaudeFlowTask::Performance
        );
    }
}
