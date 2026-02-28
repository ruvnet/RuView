//! Agent Factory for SONA
//!
//! Create and manage multiple specialized agents.

use super::metrics::TrainingMetrics;
use super::templates::{AgentType, TrainingTemplate};
use crate::engine::SonaEngine;
use crate::time_compat::SystemTime;
use crate::types::SonaConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Handle to a managed agent
#[derive(Clone, Debug)]
pub struct AgentHandle {
    /// Agent identifier
    pub id: String,
    /// Agent type
    pub agent_type: AgentType,
    /// Creation timestamp
    pub created_at: u64,
}

/// Managed agent with engine and metadata
pub struct ManagedAgent {
    /// Agent handle
    pub handle: AgentHandle,
    /// SONA engine
    pub engine: SonaEngine,
    /// Training metrics
    pub metrics: TrainingMetrics,
    /// Purpose/description
    pub purpose: String,
    /// Training count
    pub training_count: u64,
    /// Tags for organization
    pub tags: Vec<String>,
}

impl ManagedAgent {
    /// Create a new managed agent
    pub fn new(
        id: impl Into<String>,
        agent_type: AgentType,
        config: SonaConfig,
        purpose: impl Into<String>,
    ) -> Self {
        let now = SystemTime::now().duration_since_epoch().as_secs();

        let id = id.into();
        Self {
            handle: AgentHandle {
                id: id.clone(),
                agent_type,
                created_at: now,
            },
            engine: SonaEngine::with_config(config),
            metrics: TrainingMetrics::new(&id),
            purpose: purpose.into(),
            training_count: 0,
            tags: Vec::new(),
        }
    }

    /// Get agent stats
    pub fn stats(&self) -> AgentStats {
        AgentStats {
            id: self.handle.id.clone(),
            agent_type: self.handle.agent_type.clone(),
            training_count: self.training_count,
            patterns_learned: self.metrics.patterns_learned,
            avg_quality: self.metrics.avg_quality(),
            total_examples: self.metrics.total_examples,
        }
    }
}

/// Agent statistics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentStats {
    /// Agent ID
    pub id: String,
    /// Agent type
    pub agent_type: AgentType,
    /// Number of training sessions
    pub training_count: u64,
    /// Patterns learned
    pub patterns_learned: usize,
    /// Average quality score
    pub avg_quality: f32,
    /// Total examples processed
    pub total_examples: usize,
}

/// Factory for creating and managing agents
pub struct AgentFactory {
    /// Base configuration for all agents
    base_config: SonaConfig,
    /// Managed agents
    agents: HashMap<String, ManagedAgent>,
    /// Default hidden dimension
    default_hidden_dim: usize,
}

impl Default for AgentFactory {
    fn default() -> Self {
        Self::new(SonaConfig::default())
    }
}

impl AgentFactory {
    /// Create a new agent factory
    pub fn new(base_config: SonaConfig) -> Self {
        let default_hidden_dim = base_config.hidden_dim;
        Self {
            base_config,
            agents: HashMap::new(),
            default_hidden_dim,
        }
    }

    /// Create factory with specific hidden dimension
    pub fn with_hidden_dim(hidden_dim: usize) -> Self {
        let config = SonaConfig {
            hidden_dim,
            embedding_dim: hidden_dim,
            ..SonaConfig::default()
        };
        Self::new(config)
    }

    /// Create an agent from a template
    pub fn create_from_template(
        &mut self,
        name: impl Into<String>,
        template: &TrainingTemplate,
    ) -> &ManagedAgent {
        let name = name.into();
        let agent = ManagedAgent::new(
            name.clone(),
            template.agent_type.clone(),
            template.sona_config.clone(),
            &template.name,
        );
        self.agents.insert(name.clone(), agent);
        self.agents.get(&name).unwrap()
    }

    /// Create an agent with custom configuration
    pub fn create_agent(
        &mut self,
        name: impl Into<String>,
        agent_type: AgentType,
        purpose: impl Into<String>,
    ) -> &ManagedAgent {
        let name = name.into();
        let config = self.config_for_agent_type(&agent_type);
        let mut agent = ManagedAgent::new(name.clone(), agent_type, config, purpose);
        agent.tags.push("custom".into());
        self.agents.insert(name.clone(), agent);
        self.agents.get(&name).unwrap()
    }

    /// Create a code agent
    pub fn create_code_agent(&mut self, name: impl Into<String>) -> &ManagedAgent {
        let template = TrainingTemplate::code_agent().with_hidden_dim(self.default_hidden_dim);
        self.create_from_template(name, &template)
    }

    /// Create a chat agent
    pub fn create_chat_agent(&mut self, name: impl Into<String>) -> &ManagedAgent {
        let template = TrainingTemplate::chat_agent().with_hidden_dim(self.default_hidden_dim);
        self.create_from_template(name, &template)
    }

    /// Create a RAG agent
    pub fn create_rag_agent(&mut self, name: impl Into<String>) -> &ManagedAgent {
        let template = TrainingTemplate::rag_agent().with_hidden_dim(self.default_hidden_dim);
        self.create_from_template(name, &template)
    }

    /// Create a task planner agent
    pub fn create_task_planner(&mut self, name: impl Into<String>) -> &ManagedAgent {
        let template = TrainingTemplate::task_planner().with_hidden_dim(self.default_hidden_dim);
        self.create_from_template(name, &template)
    }

    /// Create a reasoning agent
    pub fn create_reasoning_agent(&mut self, name: impl Into<String>) -> &ManagedAgent {
        let template = TrainingTemplate::reasoning_agent().with_hidden_dim(self.default_hidden_dim);
        self.create_from_template(name, &template)
    }

    /// Create a codebase helper agent
    pub fn create_codebase_helper(&mut self, name: impl Into<String>) -> &ManagedAgent {
        let template = TrainingTemplate::codebase_helper().with_hidden_dim(self.default_hidden_dim);
        self.create_from_template(name, &template)
    }

    /// Get an agent by name
    pub fn get_agent(&self, name: &str) -> Option<&ManagedAgent> {
        self.agents.get(name)
    }

    /// Get a mutable agent by name
    pub fn get_agent_mut(&mut self, name: &str) -> Option<&mut ManagedAgent> {
        self.agents.get_mut(name)
    }

    /// Remove an agent
    pub fn remove_agent(&mut self, name: &str) -> Option<ManagedAgent> {
        self.agents.remove(name)
    }

    /// List all agents
    pub fn list_agents(&self) -> Vec<AgentStats> {
        self.agents.values().map(|a| a.stats()).collect()
    }

    /// Get agent count
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Train an agent with examples
    pub fn train_agent<E>(
        &mut self,
        name: &str,
        examples: impl Iterator<Item = E>,
    ) -> Result<usize, String>
    where
        E: TrainingExample,
    {
        let agent = self
            .agents
            .get_mut(name)
            .ok_or_else(|| format!("Agent '{}' not found", name))?;

        let mut count = 0;
        for example in examples {
            // Use builder-based trajectory API
            let mut builder = agent.engine.begin_trajectory(example.embedding());

            // Set route if available
            if let Some(route) = example.route() {
                builder.set_model_route(&route);
            }

            // Add context if available
            for ctx in example.context() {
                builder.add_context(&ctx);
            }

            // Add step with activations
            builder.add_step(example.activations(), example.attention(), example.reward());

            // End trajectory with quality
            agent.engine.end_trajectory(builder, example.quality());

            count += 1;
            agent.metrics.total_examples += 1;
            agent.metrics.add_quality_sample(example.quality());
        }

        // Force learning after batch
        agent.engine.force_learn();
        agent.training_count += 1;
        agent.metrics.training_sessions += 1;

        Ok(count)
    }

    /// Get configuration for agent type
    fn config_for_agent_type(&self, agent_type: &AgentType) -> SonaConfig {
        let mut config = self.base_config.clone();

        match agent_type {
            AgentType::CodeAgent | AgentType::CodebaseHelper => {
                config.base_lora_rank = 16;
                config.pattern_clusters = 200;
                config.quality_threshold = 0.2;
            }
            AgentType::ChatAgent => {
                config.base_lora_rank = 8;
                config.pattern_clusters = 50;
                config.quality_threshold = 0.4;
            }
            AgentType::RagAgent => {
                config.pattern_clusters = 200;
                config.trajectory_capacity = 10000;
            }
            AgentType::TaskPlanner => {
                config.base_lora_rank = 16;
                config.ewc_lambda = 2000.0;
            }
            AgentType::ReasoningAgent => {
                config.base_lora_rank = 16;
                config.ewc_lambda = 3000.0;
                config.pattern_clusters = 150;
            }
            AgentType::DomainExpert => {
                config.quality_threshold = 0.1;
                config.trajectory_capacity = 20000;
            }
            AgentType::DataAnalyst => {
                config.base_lora_rank = 8;
                config.pattern_clusters = 100;
            }
            AgentType::CreativeWriter => {
                config.base_lora_rank = 8;
                config.pattern_clusters = 50;
                config.quality_threshold = 0.5;
            }
            _ => {}
        }

        config
    }
}

/// Trait for training examples
pub trait TrainingExample {
    /// Get embedding vector
    fn embedding(&self) -> Vec<f32>;

    /// Get activations (can be same as embedding)
    fn activations(&self) -> Vec<f32> {
        self.embedding()
    }

    /// Get attention weights
    fn attention(&self) -> Vec<f32> {
        vec![1.0 / 64.0; 64]
    }

    /// Get reward signal
    fn reward(&self) -> f32 {
        self.quality()
    }

    /// Get quality score
    fn quality(&self) -> f32;

    /// Get optional route
    fn route(&self) -> Option<String> {
        None
    }

    /// Get context identifiers
    fn context(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Simple training example implementation
#[derive(Clone, Debug)]
pub struct SimpleExample {
    /// Embedding vector
    pub embedding: Vec<f32>,
    /// Quality score
    pub quality: f32,
    /// Optional route
    pub route: Option<String>,
    /// Context IDs
    pub context: Vec<String>,
}

impl SimpleExample {
    /// Create a new simple example
    pub fn new(embedding: Vec<f32>, quality: f32) -> Self {
        Self {
            embedding,
            quality,
            route: None,
            context: Vec::new(),
        }
    }

    /// Set route
    pub fn with_route(mut self, route: impl Into<String>) -> Self {
        self.route = Some(route.into());
        self
    }

    /// Add context
    pub fn with_context(mut self, ctx: impl Into<String>) -> Self {
        self.context.push(ctx.into());
        self
    }
}

impl TrainingExample for SimpleExample {
    fn embedding(&self) -> Vec<f32> {
        self.embedding.clone()
    }

    fn quality(&self) -> f32 {
        self.quality
    }

    fn route(&self) -> Option<String> {
        self.route.clone()
    }

    fn context(&self) -> Vec<String> {
        self.context.clone()
    }
}

/// Thread-safe agent factory wrapper
pub struct SharedAgentFactory {
    inner: Arc<RwLock<AgentFactory>>,
}

impl SharedAgentFactory {
    /// Create a new shared factory
    pub fn new(config: SonaConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(AgentFactory::new(config))),
        }
    }

    /// Get read access to factory
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, AgentFactory> {
        self.inner.read().unwrap()
    }

    /// Get write access to factory
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, AgentFactory> {
        self.inner.write().unwrap()
    }

    /// Clone the Arc
    pub fn clone_arc(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Clone for SharedAgentFactory {
    fn clone(&self) -> Self {
        self.clone_arc()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_creation() {
        let factory = AgentFactory::default();
        assert_eq!(factory.agent_count(), 0);
    }

    #[test]
    fn test_create_agents() {
        let mut factory = AgentFactory::with_hidden_dim(256);

        factory.create_code_agent("code-1");
        factory.create_chat_agent("chat-1");
        factory.create_rag_agent("rag-1");

        assert_eq!(factory.agent_count(), 3);
        assert!(factory.get_agent("code-1").is_some());
        assert!(factory.get_agent("unknown").is_none());
    }

    #[test]
    fn test_agent_from_template() {
        let mut factory = AgentFactory::with_hidden_dim(256);
        let template = TrainingTemplate::reasoning_agent().with_hidden_dim(256);

        factory.create_from_template("reasoner", &template);

        let agent = factory.get_agent("reasoner").unwrap();
        assert_eq!(agent.handle.agent_type, AgentType::ReasoningAgent);
    }

    #[test]
    fn test_train_agent() {
        let mut factory = AgentFactory::with_hidden_dim(256);
        factory.create_chat_agent("bot");

        let examples = vec![
            SimpleExample::new(vec![0.1; 256], 0.8).with_route("greeting"),
            SimpleExample::new(vec![0.2; 256], 0.9).with_route("question"),
            SimpleExample::new(vec![0.3; 256], 0.7).with_route("farewell"),
        ];

        let count = factory.train_agent("bot", examples.into_iter()).unwrap();
        assert_eq!(count, 3);

        let agent = factory.get_agent("bot").unwrap();
        assert_eq!(agent.training_count, 1);
        assert_eq!(agent.metrics.total_examples, 3);
    }

    #[test]
    fn test_list_agents() {
        let mut factory = AgentFactory::with_hidden_dim(256);
        factory.create_code_agent("code");
        factory.create_chat_agent("chat");

        let agents = factory.list_agents();
        assert_eq!(agents.len(), 2);
    }
}
