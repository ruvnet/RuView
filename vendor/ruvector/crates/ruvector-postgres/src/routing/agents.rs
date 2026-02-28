// Agent Registry and Management
//
// Thread-safe registry for managing AI agents with capabilities and performance metrics.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Type of AI agent
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentType {
    /// Language model (GPT, Claude, etc.)
    LLM,
    /// Embedding model
    Embedding,
    /// Specialized task agent
    Specialized,
    /// Vision model
    Vision,
    /// Audio model
    Audio,
    /// Multimodal agent
    Multimodal,
    /// Custom agent type
    Custom(String),
}

impl AgentType {
    /// Parse agent type from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "llm" => AgentType::LLM,
            "embedding" => AgentType::Embedding,
            "specialized" => AgentType::Specialized,
            "vision" => AgentType::Vision,
            "audio" => AgentType::Audio,
            "multimodal" => AgentType::Multimodal,
            _ => AgentType::Custom(s.to_string()),
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &str {
        match self {
            AgentType::LLM => "llm",
            AgentType::Embedding => "embedding",
            AgentType::Specialized => "specialized",
            AgentType::Vision => "vision",
            AgentType::Audio => "audio",
            AgentType::Multimodal => "multimodal",
            AgentType::Custom(s) => s,
        }
    }
}

/// Cost model for agent usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostModel {
    /// Cost per request
    pub per_request: f32,
    /// Cost per token (if applicable)
    pub per_token: Option<f32>,
    /// Fixed monthly cost
    pub monthly_fixed: Option<f32>,
}

impl Default for CostModel {
    fn default() -> Self {
        Self {
            per_request: 0.0,
            per_token: None,
            monthly_fixed: None,
        }
    }
}

/// Performance metrics for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Average latency in milliseconds
    pub avg_latency_ms: f32,
    /// 95th percentile latency
    pub p95_latency_ms: f32,
    /// 99th percentile latency
    pub p99_latency_ms: f32,
    /// Quality score (0-1)
    pub quality_score: f32,
    /// Success rate (0-1)
    pub success_rate: f32,
    /// Total requests processed
    pub total_requests: u64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            avg_latency_ms: 100.0,
            p95_latency_ms: 200.0,
            p99_latency_ms: 500.0,
            quality_score: 0.8,
            success_rate: 0.99,
            total_requests: 0,
        }
    }
}

/// AI Agent definition with capabilities and metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Unique agent name
    pub name: String,
    /// Agent type
    pub agent_type: AgentType,
    /// Capabilities (e.g., ["code_generation", "translation"])
    pub capabilities: Vec<String>,
    /// Cost model
    pub cost_model: CostModel,
    /// Performance metrics
    pub performance: PerformanceMetrics,
    /// Agent embedding for similarity matching (384-dim)
    pub embedding: Option<Vec<f32>>,
    /// Whether agent is currently active
    pub is_active: bool,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

impl Agent {
    /// Create a new agent
    pub fn new(name: String, agent_type: AgentType, capabilities: Vec<String>) -> Self {
        Self {
            name,
            agent_type,
            capabilities,
            cost_model: CostModel::default(),
            performance: PerformanceMetrics::default(),
            embedding: None,
            is_active: true,
            metadata: serde_json::Value::Null,
        }
    }

    /// Check if agent has a specific capability
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities
            .iter()
            .any(|c| c.eq_ignore_ascii_case(capability))
    }

    /// Calculate total cost for a request
    pub fn calculate_cost(&self, token_count: Option<u32>) -> f32 {
        let mut cost = self.cost_model.per_request;

        if let (Some(tokens), Some(per_token)) = (token_count, self.cost_model.per_token) {
            cost += tokens as f32 * per_token;
        }

        cost
    }

    /// Update performance metrics with new observation
    pub fn update_metrics(&mut self, latency_ms: f32, success: bool, quality: Option<f32>) {
        let n = self.performance.total_requests as f32;
        let new_n = n + 1.0;

        // Update average latency with exponential moving average
        self.performance.avg_latency_ms =
            (self.performance.avg_latency_ms * n + latency_ms) / new_n;

        // Update success rate
        let prev_successes = (self.performance.success_rate * n) as u64;
        let new_successes = prev_successes + if success { 1 } else { 0 };
        self.performance.success_rate = new_successes as f32 / new_n;

        // Update quality score if provided
        if let Some(q) = quality {
            self.performance.quality_score = (self.performance.quality_score * n + q) / new_n;
        }

        self.performance.total_requests += 1;

        // Update percentiles (simplified approach)
        if latency_ms > self.performance.avg_latency_ms * 1.5 {
            self.performance.p95_latency_ms =
                (self.performance.p95_latency_ms * 0.95 + latency_ms * 0.05).max(latency_ms);
        }
        if latency_ms > self.performance.avg_latency_ms * 2.0 {
            self.performance.p99_latency_ms =
                (self.performance.p99_latency_ms * 0.99 + latency_ms * 0.01).max(latency_ms);
        }
    }
}

/// Thread-safe agent registry
pub struct AgentRegistry {
    /// Agents stored by name
    agents: Arc<DashMap<String, Agent>>,
}

impl AgentRegistry {
    /// Create a new agent registry
    pub fn new() -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
        }
    }

    /// Register a new agent
    pub fn register(&self, agent: Agent) -> Result<(), String> {
        if self.agents.contains_key(&agent.name) {
            return Err(format!("Agent '{}' already exists", agent.name));
        }

        self.agents.insert(agent.name.clone(), agent);
        Ok(())
    }

    /// Update an existing agent
    pub fn update(&self, agent: Agent) -> Result<(), String> {
        if !self.agents.contains_key(&agent.name) {
            return Err(format!("Agent '{}' not found", agent.name));
        }

        self.agents.insert(agent.name.clone(), agent);
        Ok(())
    }

    /// Get an agent by name
    pub fn get(&self, name: &str) -> Option<Agent> {
        self.agents.get(name).map(|entry| entry.clone())
    }

    /// Remove an agent
    pub fn remove(&self, name: &str) -> Option<Agent> {
        self.agents.remove(name).map(|(_, agent)| agent)
    }

    /// List all active agents
    pub fn list_active(&self) -> Vec<Agent> {
        self.agents
            .iter()
            .filter(|entry| entry.is_active)
            .map(|entry| entry.clone())
            .collect()
    }

    /// List all agents
    pub fn list_all(&self) -> Vec<Agent> {
        self.agents.iter().map(|entry| entry.clone()).collect()
    }

    /// Find agents by capability
    pub fn find_by_capability(&self, capability: &str, k: usize) -> Vec<Agent> {
        let mut agents: Vec<Agent> = self
            .agents
            .iter()
            .filter(|entry| entry.is_active && entry.has_capability(capability))
            .map(|entry| entry.clone())
            .collect();

        // Sort by quality score (descending)
        agents.sort_by(|a, b| {
            b.performance
                .quality_score
                .partial_cmp(&a.performance.quality_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        agents.into_iter().take(k).collect()
    }

    /// Find agents by type
    pub fn find_by_type(&self, agent_type: &AgentType) -> Vec<Agent> {
        self.agents
            .iter()
            .filter(|entry| entry.is_active && &entry.agent_type == agent_type)
            .map(|entry| entry.clone())
            .collect()
    }

    /// Get agent count
    pub fn count(&self) -> usize {
        self.agents.len()
    }

    /// Get active agent count
    pub fn count_active(&self) -> usize {
        self.agents.iter().filter(|entry| entry.is_active).count()
    }

    /// Clear all agents
    pub fn clear(&self) {
        self.agents.clear();
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_type_parsing() {
        assert_eq!(AgentType::from_str("llm"), AgentType::LLM);
        assert_eq!(AgentType::from_str("LLM"), AgentType::LLM);
        assert_eq!(AgentType::from_str("embedding"), AgentType::Embedding);
        assert_eq!(
            AgentType::from_str("custom"),
            AgentType::Custom("custom".to_string())
        );
    }

    #[test]
    fn test_agent_creation() {
        let agent = Agent::new(
            "gpt-4".to_string(),
            AgentType::LLM,
            vec!["code_generation".to_string(), "translation".to_string()],
        );

        assert_eq!(agent.name, "gpt-4");
        assert_eq!(agent.agent_type, AgentType::LLM);
        assert_eq!(agent.capabilities.len(), 2);
        assert!(agent.is_active);
    }

    #[test]
    fn test_agent_has_capability() {
        let agent = Agent::new(
            "test".to_string(),
            AgentType::LLM,
            vec!["code_generation".to_string()],
        );

        assert!(agent.has_capability("code_generation"));
        assert!(agent.has_capability("CODE_GENERATION"));
        assert!(!agent.has_capability("translation"));
    }

    #[test]
    fn test_agent_cost_calculation() {
        let mut agent = Agent::new("test".to_string(), AgentType::LLM, vec![]);
        agent.cost_model.per_request = 0.01;
        agent.cost_model.per_token = Some(0.0001);

        assert_eq!(agent.calculate_cost(None), 0.01);
        assert_eq!(agent.calculate_cost(Some(1000)), 0.11); // 0.01 + 1000 * 0.0001
    }

    #[test]
    fn test_agent_update_metrics() {
        let mut agent = Agent::new("test".to_string(), AgentType::LLM, vec![]);

        // Initial state
        assert_eq!(agent.performance.total_requests, 0);

        // Add first observation
        agent.update_metrics(100.0, true, Some(0.9));
        assert_eq!(agent.performance.total_requests, 1);
        assert_eq!(agent.performance.avg_latency_ms, 100.0);
        assert_eq!(agent.performance.success_rate, 1.0);
        assert_eq!(agent.performance.quality_score, 0.9);

        // Add second observation
        agent.update_metrics(200.0, true, Some(0.8));
        assert_eq!(agent.performance.total_requests, 2);
        assert_eq!(agent.performance.avg_latency_ms, 150.0);
        assert_eq!(agent.performance.success_rate, 1.0);
        assert!((agent.performance.quality_score - 0.85).abs() < 0.01);
    }

    #[test]
    fn test_registry_register() {
        let registry = AgentRegistry::new();
        let agent = Agent::new("test".to_string(), AgentType::LLM, vec![]);

        assert!(registry.register(agent.clone()).is_ok());
        assert_eq!(registry.count(), 1);

        // Duplicate registration should fail
        assert!(registry.register(agent).is_err());
    }

    #[test]
    fn test_registry_get() {
        let registry = AgentRegistry::new();
        let agent = Agent::new("test".to_string(), AgentType::LLM, vec![]);

        registry.register(agent.clone()).unwrap();

        let retrieved = registry.get("test").unwrap();
        assert_eq!(retrieved.name, "test");

        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_remove() {
        let registry = AgentRegistry::new();
        let agent = Agent::new("test".to_string(), AgentType::LLM, vec![]);

        registry.register(agent).unwrap();
        assert_eq!(registry.count(), 1);

        let removed = registry.remove("test").unwrap();
        assert_eq!(removed.name, "test");
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_registry_list_active() {
        let registry = AgentRegistry::new();

        let mut agent1 = Agent::new("active".to_string(), AgentType::LLM, vec![]);
        agent1.is_active = true;

        let mut agent2 = Agent::new("inactive".to_string(), AgentType::LLM, vec![]);
        agent2.is_active = false;

        registry.register(agent1).unwrap();
        registry.register(agent2).unwrap();

        let active = registry.list_active();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "active");
    }

    #[test]
    fn test_registry_find_by_capability() {
        let registry = AgentRegistry::new();

        let agent1 = Agent::new(
            "agent1".to_string(),
            AgentType::LLM,
            vec!["coding".to_string()],
        );
        let agent2 = Agent::new(
            "agent2".to_string(),
            AgentType::LLM,
            vec!["translation".to_string()],
        );
        let agent3 = Agent::new(
            "agent3".to_string(),
            AgentType::LLM,
            vec!["coding".to_string(), "translation".to_string()],
        );

        registry.register(agent1).unwrap();
        registry.register(agent2).unwrap();
        registry.register(agent3).unwrap();

        let coders = registry.find_by_capability("coding", 10);
        assert_eq!(coders.len(), 2);

        let translators = registry.find_by_capability("translation", 10);
        assert_eq!(translators.len(), 2);
    }

    #[test]
    fn test_registry_find_by_type() {
        let registry = AgentRegistry::new();

        registry
            .register(Agent::new("llm1".to_string(), AgentType::LLM, vec![]))
            .unwrap();
        registry
            .register(Agent::new("llm2".to_string(), AgentType::LLM, vec![]))
            .unwrap();
        registry
            .register(Agent::new(
                "embed1".to_string(),
                AgentType::Embedding,
                vec![],
            ))
            .unwrap();

        let llms = registry.find_by_type(&AgentType::LLM);
        assert_eq!(llms.len(), 2);

        let embeddings = registry.find_by_type(&AgentType::Embedding);
        assert_eq!(embeddings.len(), 1);
    }

    #[test]
    fn test_registry_clear() {
        let registry = AgentRegistry::new();
        registry
            .register(Agent::new("test".to_string(), AgentType::LLM, vec![]))
            .unwrap();

        assert_eq!(registry.count(), 1);
        registry.clear();
        assert_eq!(registry.count(), 0);
    }
}
