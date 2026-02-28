// Neural-Powered Agent Router
//
// Dynamic routing with FastGRNN and multi-objective optimization.

use super::agents::{Agent, AgentRegistry};
use super::fastgrnn::FastGRNN;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Optimization target for routing decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptimizationTarget {
    /// Minimize cost
    Cost,
    /// Minimize latency
    Latency,
    /// Maximize quality
    Quality,
    /// Balanced optimization
    Balanced,
}

impl OptimizationTarget {
    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "cost" => OptimizationTarget::Cost,
            "latency" => OptimizationTarget::Latency,
            "quality" => OptimizationTarget::Quality,
            "balanced" => OptimizationTarget::Balanced,
            _ => OptimizationTarget::Balanced,
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &str {
        match self {
            OptimizationTarget::Cost => "cost",
            OptimizationTarget::Latency => "latency",
            OptimizationTarget::Quality => "quality",
            OptimizationTarget::Balanced => "balanced",
        }
    }
}

/// Constraints for routing decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConstraints {
    /// Maximum acceptable cost
    pub max_cost: Option<f32>,
    /// Maximum acceptable latency in ms
    pub max_latency_ms: Option<f32>,
    /// Minimum required quality score (0-1)
    pub min_quality: Option<f32>,
    /// Required capabilities
    pub required_capabilities: Vec<String>,
    /// Excluded agent names
    pub excluded_agents: Vec<String>,
}

impl Default for RoutingConstraints {
    fn default() -> Self {
        Self {
            max_cost: None,
            max_latency_ms: None,
            min_quality: None,
            required_capabilities: Vec::new(),
            excluded_agents: Vec::new(),
        }
    }
}

impl RoutingConstraints {
    /// Create new constraints
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum cost
    pub fn with_max_cost(mut self, cost: f32) -> Self {
        self.max_cost = Some(cost);
        self
    }

    /// Set maximum latency
    pub fn with_max_latency(mut self, latency_ms: f32) -> Self {
        self.max_latency_ms = Some(latency_ms);
        self
    }

    /// Set minimum quality
    pub fn with_min_quality(mut self, quality: f32) -> Self {
        self.min_quality = Some(quality);
        self
    }

    /// Add required capability
    pub fn with_capability(mut self, capability: String) -> Self {
        self.required_capabilities.push(capability);
        self
    }

    /// Add excluded agent
    pub fn with_excluded_agent(mut self, agent_name: String) -> Self {
        self.excluded_agents.push(agent_name);
        self
    }
}

/// Routing decision result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    /// Selected agent name
    pub agent_name: String,
    /// Confidence score (0-1)
    pub confidence: f32,
    /// Estimated cost
    pub estimated_cost: f32,
    /// Estimated latency in ms
    pub estimated_latency_ms: f32,
    /// Expected quality
    pub expected_quality: f32,
    /// Similarity score to request
    pub similarity_score: f32,
    /// Reasoning for the decision
    pub reasoning: String,
    /// Alternative agents considered
    pub alternatives: Vec<AlternativeAgent>,
}

/// Alternative agent option
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternativeAgent {
    /// Agent name
    pub name: String,
    /// Score
    pub score: f32,
    /// Why it wasn't selected
    pub reason: String,
}

/// Neural-powered agent router
pub struct Router {
    /// Agent registry
    registry: Arc<AgentRegistry>,
    /// FastGRNN model for neural routing
    grnn: Option<FastGRNN>,
    /// Embedding dimension
    embedding_dim: usize,
}

impl Router {
    /// Create a new router
    pub fn new() -> Self {
        Self {
            registry: Arc::new(AgentRegistry::new()),
            grnn: None,
            embedding_dim: 384, // Default embedding size
        }
    }

    /// Create router with custom registry
    pub fn with_registry(registry: Arc<AgentRegistry>) -> Self {
        Self {
            registry,
            grnn: None,
            embedding_dim: 384,
        }
    }

    /// Initialize FastGRNN model
    pub fn init_grnn(&mut self, hidden_dim: usize) {
        self.grnn = Some(FastGRNN::new(self.embedding_dim, hidden_dim));
    }

    /// Set FastGRNN model from weights
    pub fn set_grnn(&mut self, grnn: FastGRNN) {
        self.grnn = Some(grnn);
    }

    /// Route a request to the best agent
    pub fn route(
        &self,
        request_embedding: &[f32],
        constraints: &RoutingConstraints,
        target: OptimizationTarget,
    ) -> Result<RoutingDecision, String> {
        // Get candidate agents
        let candidates = self.get_candidates(constraints)?;

        if candidates.is_empty() {
            return Err("No agents match the constraints".to_string());
        }

        // Score all candidates
        let mut scored_candidates: Vec<(Agent, f32, f32)> = candidates
            .iter()
            .filter_map(|agent| {
                // Calculate similarity
                let similarity = if let Some(agent_emb) = &agent.embedding {
                    cosine_similarity(request_embedding, agent_emb)
                } else {
                    0.5 // Default similarity if no embedding
                };

                // Calculate score based on target
                let score = self.score_agent(agent, request_embedding, target, similarity);

                // Apply constraints
                if self.meets_constraints(agent, constraints) {
                    Some((agent.clone(), score, similarity))
                } else {
                    None
                }
            })
            .collect();

        if scored_candidates.is_empty() {
            return Err("No agents meet the specified constraints".to_string());
        }

        // Sort by score (descending)
        scored_candidates
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Select best agent
        let (best_agent, best_score, similarity) = &scored_candidates[0];

        // Calculate confidence using FastGRNN if available
        let confidence = if let Some(ref grnn) = self.grnn {
            let hidden = grnn.forward_single(request_embedding);
            // Use hidden state magnitude as confidence
            let magnitude: f32 = hidden.iter().map(|&h| h * h).sum::<f32>().sqrt();
            (magnitude / hidden.len() as f32).min(1.0).max(0.0)
        } else {
            *best_score
        };

        // Build alternatives list
        let alternatives: Vec<AlternativeAgent> = scored_candidates
            .iter()
            .skip(1)
            .take(3)
            .map(|(agent, score, _)| AlternativeAgent {
                name: agent.name.clone(),
                score: *score,
                reason: self.compare_to_best(agent, best_agent, target),
            })
            .collect();

        // Generate reasoning
        let reasoning = self.generate_reasoning(best_agent, target, *similarity);

        Ok(RoutingDecision {
            agent_name: best_agent.name.clone(),
            confidence,
            estimated_cost: best_agent.cost_model.per_request,
            estimated_latency_ms: best_agent.performance.avg_latency_ms,
            expected_quality: best_agent.performance.quality_score,
            similarity_score: *similarity,
            reasoning,
            alternatives,
        })
    }

    /// Get candidate agents based on constraints
    fn get_candidates(&self, constraints: &RoutingConstraints) -> Result<Vec<Agent>, String> {
        let mut agents = self.registry.list_active();

        // Filter by required capabilities
        if !constraints.required_capabilities.is_empty() {
            agents.retain(|agent| {
                constraints
                    .required_capabilities
                    .iter()
                    .all(|cap| agent.has_capability(cap))
            });
        }

        // Filter excluded agents
        if !constraints.excluded_agents.is_empty() {
            agents.retain(|agent| !constraints.excluded_agents.contains(&agent.name));
        }

        Ok(agents)
    }

    /// Check if agent meets constraints
    fn meets_constraints(&self, agent: &Agent, constraints: &RoutingConstraints) -> bool {
        // Check cost constraint
        if let Some(max_cost) = constraints.max_cost {
            if agent.cost_model.per_request > max_cost {
                return false;
            }
        }

        // Check latency constraint
        if let Some(max_latency) = constraints.max_latency_ms {
            if agent.performance.avg_latency_ms > max_latency {
                return false;
            }
        }

        // Check quality constraint
        if let Some(min_quality) = constraints.min_quality {
            if agent.performance.quality_score < min_quality {
                return false;
            }
        }

        true
    }

    /// Score an agent for a given target
    fn score_agent(
        &self,
        agent: &Agent,
        _request_embedding: &[f32],
        target: OptimizationTarget,
        similarity: f32,
    ) -> f32 {
        match target {
            OptimizationTarget::Cost => {
                // Lower cost = higher score
                let cost_score = 1.0 / (1.0 + agent.cost_model.per_request);
                cost_score * 0.7 + similarity * 0.3
            }
            OptimizationTarget::Latency => {
                // Lower latency = higher score
                let latency_score = 1.0 / (1.0 + agent.performance.avg_latency_ms / 1000.0);
                latency_score * 0.7 + similarity * 0.3
            }
            OptimizationTarget::Quality => {
                // Higher quality = higher score
                agent.performance.quality_score * 0.7 + similarity * 0.3
            }
            OptimizationTarget::Balanced => {
                // Balanced scoring
                let cost_score = 1.0 / (1.0 + agent.cost_model.per_request);
                let latency_score = 1.0 / (1.0 + agent.performance.avg_latency_ms / 1000.0);
                let quality_score = agent.performance.quality_score;

                cost_score * 0.25 + latency_score * 0.25 + quality_score * 0.25 + similarity * 0.25
            }
        }
    }

    /// Compare agent to best agent
    fn compare_to_best(&self, agent: &Agent, best: &Agent, target: OptimizationTarget) -> String {
        match target {
            OptimizationTarget::Cost => {
                let diff = agent.cost_model.per_request - best.cost_model.per_request;
                format!("${:.4} more expensive", diff)
            }
            OptimizationTarget::Latency => {
                let diff = agent.performance.avg_latency_ms - best.performance.avg_latency_ms;
                format!("{:.1}ms slower", diff)
            }
            OptimizationTarget::Quality => {
                let diff = best.performance.quality_score - agent.performance.quality_score;
                format!("{:.2} lower quality", diff)
            }
            OptimizationTarget::Balanced => "Lower overall score".to_string(),
        }
    }

    /// Generate reasoning for decision
    fn generate_reasoning(
        &self,
        agent: &Agent,
        target: OptimizationTarget,
        similarity: f32,
    ) -> String {
        let target_reason = match target {
            OptimizationTarget::Cost => {
                format!("lowest cost (${:.4}/request)", agent.cost_model.per_request)
            }
            OptimizationTarget::Latency => format!(
                "fastest response ({:.1}ms avg)",
                agent.performance.avg_latency_ms
            ),
            OptimizationTarget::Quality => format!(
                "highest quality (score: {:.2})",
                agent.performance.quality_score
            ),
            OptimizationTarget::Balanced => "best overall balance".to_string(),
        };

        format!(
            "Selected {} for {} with {:.1}% similarity to request",
            agent.name,
            target_reason,
            similarity * 100.0
        )
    }

    /// Get registry reference
    pub fn registry(&self) -> &Arc<AgentRegistry> {
        &self.registry
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    (dot_product / (norm_a * norm_b)).max(-1.0).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::agents::AgentType;

    fn create_test_agent(name: &str, cost: f32, latency: f32, quality: f32) -> Agent {
        let mut agent = Agent::new(name.to_string(), AgentType::LLM, vec!["test".to_string()]);
        agent.cost_model.per_request = cost;
        agent.performance.avg_latency_ms = latency;
        agent.performance.quality_score = quality;
        agent.embedding = Some(vec![0.1; 384]);
        agent
    }

    #[test]
    fn test_optimization_target_parsing() {
        assert_eq!(
            OptimizationTarget::from_str("cost"),
            OptimizationTarget::Cost
        );
        assert_eq!(
            OptimizationTarget::from_str("LATENCY"),
            OptimizationTarget::Latency
        );
        assert_eq!(
            OptimizationTarget::from_str("quality"),
            OptimizationTarget::Quality
        );
        assert_eq!(
            OptimizationTarget::from_str("balanced"),
            OptimizationTarget::Balanced
        );
        assert_eq!(
            OptimizationTarget::from_str("unknown"),
            OptimizationTarget::Balanced
        );
    }

    #[test]
    fn test_routing_constraints_builder() {
        let constraints = RoutingConstraints::new()
            .with_max_cost(0.1)
            .with_max_latency(500.0)
            .with_min_quality(0.8)
            .with_capability("test".to_string());

        assert_eq!(constraints.max_cost, Some(0.1));
        assert_eq!(constraints.max_latency_ms, Some(500.0));
        assert_eq!(constraints.min_quality, Some(0.8));
        assert_eq!(constraints.required_capabilities.len(), 1);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![1.0, 0.0, 0.0];
        let d = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&c, &d).abs() < 1e-6);

        let e = vec![1.0, 1.0, 0.0];
        let f = vec![1.0, 1.0, 0.0];
        assert!((cosine_similarity(&e, &f) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_router_creation() {
        let router = Router::new();
        assert!(router.grnn.is_none());
        assert_eq!(router.registry().count(), 0);
    }

    #[test]
    fn test_router_init_grnn() {
        let mut router = Router::new();
        router.init_grnn(64);
        assert!(router.grnn.is_some());
    }

    #[test]
    fn test_route_cost_optimization() {
        let router = Router::new();

        // Register agents with different costs
        router
            .registry()
            .register(create_test_agent("cheap", 0.01, 100.0, 0.7))
            .unwrap();
        router
            .registry()
            .register(create_test_agent("expensive", 0.10, 100.0, 0.9))
            .unwrap();

        let request_emb = vec![0.1; 384];
        let constraints = RoutingConstraints::new();

        let decision = router
            .route(&request_emb, &constraints, OptimizationTarget::Cost)
            .unwrap();
        assert_eq!(decision.agent_name, "cheap");
    }

    #[test]
    fn test_route_latency_optimization() {
        let router = Router::new();

        router
            .registry()
            .register(create_test_agent("fast", 0.05, 50.0, 0.7))
            .unwrap();
        router
            .registry()
            .register(create_test_agent("slow", 0.05, 500.0, 0.9))
            .unwrap();

        let request_emb = vec![0.1; 384];
        let constraints = RoutingConstraints::new();

        let decision = router
            .route(&request_emb, &constraints, OptimizationTarget::Latency)
            .unwrap();
        assert_eq!(decision.agent_name, "fast");
    }

    #[test]
    fn test_route_quality_optimization() {
        let router = Router::new();

        router
            .registry()
            .register(create_test_agent("low_quality", 0.05, 100.0, 0.5))
            .unwrap();
        router
            .registry()
            .register(create_test_agent("high_quality", 0.05, 100.0, 0.95))
            .unwrap();

        let request_emb = vec![0.1; 384];
        let constraints = RoutingConstraints::new();

        let decision = router
            .route(&request_emb, &constraints, OptimizationTarget::Quality)
            .unwrap();
        assert_eq!(decision.agent_name, "high_quality");
    }

    #[test]
    fn test_route_with_constraints() {
        let router = Router::new();

        router
            .registry()
            .register(create_test_agent("expensive", 1.0, 100.0, 0.9))
            .unwrap();
        router
            .registry()
            .register(create_test_agent("cheap", 0.01, 100.0, 0.7))
            .unwrap();

        let request_emb = vec![0.1; 384];
        let constraints = RoutingConstraints::new().with_max_cost(0.5);

        let decision = router
            .route(&request_emb, &constraints, OptimizationTarget::Quality)
            .unwrap();
        // Should select cheap even though expensive has higher quality
        assert_eq!(decision.agent_name, "cheap");
    }

    #[test]
    fn test_route_no_candidates() {
        let router = Router::new();
        let request_emb = vec![0.1; 384];
        let constraints = RoutingConstraints::new();

        let result = router.route(&request_emb, &constraints, OptimizationTarget::Balanced);
        assert!(result.is_err());
    }

    #[test]
    fn test_route_capability_filter() {
        let router = Router::new();

        let mut agent1 = create_test_agent("coder", 0.05, 100.0, 0.8);
        agent1.capabilities = vec!["coding".to_string()];

        let mut agent2 = create_test_agent("translator", 0.05, 100.0, 0.8);
        agent2.capabilities = vec!["translation".to_string()];

        router.registry().register(agent1).unwrap();
        router.registry().register(agent2).unwrap();

        let request_emb = vec![0.1; 384];
        let constraints = RoutingConstraints::new().with_capability("coding".to_string());

        let decision = router
            .route(&request_emb, &constraints, OptimizationTarget::Balanced)
            .unwrap();
        assert_eq!(decision.agent_name, "coder");
    }
}
