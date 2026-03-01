//! Training Templates for SONA
//!
//! Pre-configured training setups optimized for different use cases.

use crate::types::SonaConfig;
use serde::{Deserialize, Serialize};

/// Agent specialization types
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentType {
    /// Code generation and assistance
    CodeAgent,
    /// General chat and conversation
    ChatAgent,
    /// Document retrieval and Q&A
    RagAgent,
    /// Task decomposition and planning
    TaskPlanner,
    /// Domain-specific expert
    DomainExpert,
    /// Codebase-aware assistant
    CodebaseHelper,
    /// Data analysis and insights
    DataAnalyst,
    /// Creative writing and content
    CreativeWriter,
    /// Reasoning and logic
    ReasoningAgent,
    /// Multi-modal understanding
    MultiModal,
    /// Custom agent type
    Custom(String),
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentType::CodeAgent => write!(f, "code-agent"),
            AgentType::ChatAgent => write!(f, "chat-agent"),
            AgentType::RagAgent => write!(f, "rag-agent"),
            AgentType::TaskPlanner => write!(f, "task-planner"),
            AgentType::DomainExpert => write!(f, "domain-expert"),
            AgentType::CodebaseHelper => write!(f, "codebase-helper"),
            AgentType::DataAnalyst => write!(f, "data-analyst"),
            AgentType::CreativeWriter => write!(f, "creative-writer"),
            AgentType::ReasoningAgent => write!(f, "reasoning-agent"),
            AgentType::MultiModal => write!(f, "multi-modal"),
            AgentType::Custom(name) => write!(f, "custom-{}", name),
        }
    }
}

/// Task domain for training focus
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskDomain {
    /// Software development
    SoftwareDevelopment,
    /// Customer support
    CustomerSupport,
    /// Healthcare
    Healthcare,
    /// Finance
    Finance,
    /// Legal
    Legal,
    /// Education
    Education,
    /// Research
    Research,
    /// Marketing
    Marketing,
    /// General purpose
    General,
    /// Custom domain
    Custom(String),
}

/// Training method configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TrainingMethod {
    /// Standard supervised learning
    Supervised {
        /// Batch size for training
        batch_size: usize,
        /// Number of epochs
        epochs: usize,
    },
    /// Reinforcement learning from feedback
    RLHF {
        /// Reward model weight
        reward_weight: f32,
        /// KL divergence penalty
        kl_penalty: f32,
    },
    /// Direct preference optimization
    DPO {
        /// Beta parameter for DPO
        beta: f32,
        /// Reference model weight
        ref_weight: f32,
    },
    /// Continuous online learning
    Online {
        /// Learning rate decay
        lr_decay: f32,
        /// Window size for recent examples
        window_size: usize,
    },
    /// Few-shot adaptation
    FewShot {
        /// Number of examples per class
        k_shot: usize,
        /// Meta-learning rate
        meta_lr: f32,
    },
}

impl Default for TrainingMethod {
    fn default() -> Self {
        TrainingMethod::Online {
            lr_decay: 0.999,
            window_size: 1000,
        }
    }
}

/// Vertical-specific configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerticalConfig {
    /// Domain focus
    pub domain: TaskDomain,
    /// Specialized vocabulary size
    pub vocab_boost: usize,
    /// Domain-specific quality metrics
    pub quality_metrics: Vec<String>,
    /// Compliance requirements
    pub compliance_level: ComplianceLevel,
}

/// Compliance level for regulated industries
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum ComplianceLevel {
    #[default]
    None,
    /// Basic audit logging
    Basic,
    /// HIPAA compliance
    Hipaa,
    /// SOC2 compliance
    Soc2,
    /// GDPR compliance
    Gdpr,
    /// Custom compliance
    Custom(String),
}

/// Template preset for quick configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TemplatePreset {
    /// Minimal configuration for testing
    Minimal,
    /// Balanced for general use
    Balanced,
    /// High performance for production
    Production,
    /// Maximum quality regardless of speed
    MaxQuality,
    /// Edge deployment (<5MB)
    Edge,
    /// Research and experimentation
    Research,
}

/// Training template with full configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainingTemplate {
    /// Template name
    pub name: String,
    /// Agent type
    pub agent_type: AgentType,
    /// SONA configuration
    pub sona_config: SonaConfig,
    /// Training method
    pub training_method: TrainingMethod,
    /// Vertical configuration
    pub vertical: Option<VerticalConfig>,
    /// Expected training data size
    pub expected_data_size: DataSizeHint,
    /// Memory budget in MB
    pub memory_budget_mb: usize,
    /// Target latency in microseconds
    pub target_latency_us: u64,
    /// Enable continuous learning
    pub continuous_learning: bool,
    /// Auto-export trained adapters
    pub auto_export: bool,
    /// Tags for organization
    pub tags: Vec<String>,
}

/// Hint about training data size
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum DataSizeHint {
    /// <100 examples (few-shot)
    Tiny,
    /// 100-1000 examples
    Small,
    /// 1000-10000 examples
    #[default]
    Medium,
    /// 10000-100000 examples
    Large,
    /// >100000 examples
    Massive,
}

impl TrainingTemplate {
    /// Create a new training template
    pub fn new(name: impl Into<String>, agent_type: AgentType) -> Self {
        Self {
            name: name.into(),
            agent_type,
            sona_config: SonaConfig::default(),
            training_method: TrainingMethod::default(),
            vertical: None,
            expected_data_size: DataSizeHint::default(),
            memory_budget_mb: 100,
            target_latency_us: 1000,
            continuous_learning: true,
            auto_export: false,
            tags: Vec::new(),
        }
    }

    /// Create from preset
    pub fn from_preset(preset: TemplatePreset, agent_type: AgentType) -> Self {
        let mut template = Self::new(format!("{:?}-{}", preset, agent_type), agent_type.clone());

        match preset {
            TemplatePreset::Minimal => {
                template.sona_config = SonaConfig::edge_deployment();
                template.memory_budget_mb = 10;
                template.expected_data_size = DataSizeHint::Tiny;
            }
            TemplatePreset::Balanced => {
                template.sona_config = SonaConfig::default();
                template.memory_budget_mb = 100;
            }
            TemplatePreset::Production => {
                template.sona_config = SonaConfig::max_throughput();
                template.memory_budget_mb = 200;
                template.auto_export = true;
            }
            TemplatePreset::MaxQuality => {
                template.sona_config = SonaConfig::max_quality();
                template.memory_budget_mb = 500;
                template.expected_data_size = DataSizeHint::Large;
            }
            TemplatePreset::Edge => {
                template.sona_config = SonaConfig::edge_deployment();
                template.memory_budget_mb = 5;
                template.target_latency_us = 500;
            }
            TemplatePreset::Research => {
                template.sona_config = SonaConfig::max_quality();
                template.sona_config.trajectory_capacity = 50000;
                template.memory_budget_mb = 1000;
                template.expected_data_size = DataSizeHint::Massive;
            }
        }

        // Apply agent-specific optimizations
        template.apply_agent_optimizations();
        template
    }

    //------------------------------------------------------------------
    // Pre-built Templates for Common Use Cases
    //------------------------------------------------------------------

    /// Code agent template - optimized for code generation
    ///
    /// **Best for**: Code completion, bug fixes, refactoring
    /// **Config**: baseLoraRank=16, clusters=200, capacity=10000
    /// **Training data**: Code completions, fixes, reviews
    pub fn code_agent() -> Self {
        let mut template = Self::new("code-agent", AgentType::CodeAgent);
        template.sona_config.base_lora_rank = 16; // Deeper for code patterns
        template.sona_config.pattern_clusters = 200; // Many code patterns
        template.sona_config.trajectory_capacity = 10000;
        template.sona_config.quality_threshold = 0.2; // Learn from most examples
        template.training_method = TrainingMethod::Online {
            lr_decay: 0.9995,
            window_size: 5000,
        };
        template.tags = vec!["code".into(), "development".into(), "completion".into()];
        template
    }

    /// Chat agent template - optimized for conversational AI
    ///
    /// **Best for**: Customer support, general chat, assistants
    /// **Config**: baseLoraRank=8, clusters=50, fast response
    /// **Training data**: Conversation histories, feedback
    pub fn chat_agent() -> Self {
        let mut template = Self::new("chat-agent", AgentType::ChatAgent);
        template.sona_config.base_lora_rank = 8;
        template.sona_config.pattern_clusters = 50;
        template.sona_config.quality_threshold = 0.4;
        template.target_latency_us = 500; // Fast responses
        template.training_method = TrainingMethod::RLHF {
            reward_weight: 0.5,
            kl_penalty: 0.1,
        };
        template.tags = vec!["chat".into(), "conversation".into(), "support".into()];
        template
    }

    /// RAG agent template - optimized for retrieval-augmented generation
    ///
    /// **Best for**: Document Q&A, knowledge bases, search
    /// **Config**: clusters=200, capacity=10000, high pattern storage
    /// **Training data**: Document chunks, Q&A pairs
    pub fn rag_agent() -> Self {
        let mut template = Self::new("rag-agent", AgentType::RagAgent);
        template.sona_config.pattern_clusters = 200; // Many document patterns
        template.sona_config.trajectory_capacity = 10000;
        template.sona_config.embedding_dim = 512; // Larger embeddings for retrieval
        template.sona_config.hidden_dim = 512;
        template.training_method = TrainingMethod::Supervised {
            batch_size: 32,
            epochs: 10,
        };
        template.tags = vec!["rag".into(), "retrieval".into(), "documents".into()];
        template
    }

    /// Task planner template - optimized for task decomposition
    ///
    /// **Best for**: Project planning, task breakdown, scheduling
    /// **Config**: baseLoraRank=16, ewcLambda=2000, multi-task
    /// **Training data**: Task decompositions, planning examples
    pub fn task_planner() -> Self {
        let mut template = Self::new("task-planner", AgentType::TaskPlanner);
        template.sona_config.base_lora_rank = 16;
        template.sona_config.ewc_lambda = 2000.0; // Important for multi-task
        template.sona_config.pattern_clusters = 100;
        template.training_method = TrainingMethod::DPO {
            beta: 0.1,
            ref_weight: 0.5,
        };
        template.tags = vec!["planning".into(), "tasks".into(), "decomposition".into()];
        template
    }

    /// Domain expert template - optimized for specialized knowledge
    ///
    /// **Best for**: Legal, medical, financial expertise
    /// **Config**: qualityThreshold=0.1, high capacity, compliance
    /// **Training data**: Domain-specific Q&A, expert responses
    pub fn domain_expert(domain: TaskDomain) -> Self {
        let domain_name = format!("{:?}", domain).to_lowercase();
        let mut template = Self::new(
            format!("domain-expert-{}", domain_name),
            AgentType::DomainExpert,
        );
        template.sona_config.quality_threshold = 0.1; // Learn from all domain examples
        template.sona_config.trajectory_capacity = 20000;
        template.sona_config.base_lora_rank = 16;
        template.vertical = Some(VerticalConfig {
            domain: domain.clone(),
            vocab_boost: 10000,
            quality_metrics: vec!["accuracy".into(), "relevance".into(), "compliance".into()],
            compliance_level: match domain {
                TaskDomain::Healthcare => ComplianceLevel::Hipaa,
                TaskDomain::Finance => ComplianceLevel::Soc2,
                TaskDomain::Legal => ComplianceLevel::Basic,
                _ => ComplianceLevel::None,
            },
        });
        template.tags = vec!["domain".into(), "expert".into(), domain_name];
        template
    }

    /// Codebase helper template - learns your specific codebase
    ///
    /// **Best for**: Repository-specific assistance, code navigation
    /// **Config**: clusters=200, capacity=10000, high pattern storage
    /// **Training data**: Your repo's code, documentation
    pub fn codebase_helper() -> Self {
        let mut template = Self::new("codebase-helper", AgentType::CodebaseHelper);
        template.sona_config.pattern_clusters = 200;
        template.sona_config.trajectory_capacity = 10000;
        template.sona_config.quality_threshold = 0.2;
        template.sona_config.base_lora_rank = 16;
        template.expected_data_size = DataSizeHint::Large;
        template.training_method = TrainingMethod::Online {
            lr_decay: 0.999,
            window_size: 10000,
        };
        template.tags = vec!["codebase".into(), "repository".into(), "navigation".into()];
        template
    }

    /// Data analyst template - optimized for data insights
    ///
    /// **Best for**: Data analysis, visualization, statistics
    /// **Config**: baseLoraRank=8, clusters=100, reasoning focus
    pub fn data_analyst() -> Self {
        let mut template = Self::new("data-analyst", AgentType::DataAnalyst);
        template.sona_config.base_lora_rank = 8;
        template.sona_config.pattern_clusters = 100;
        template.vertical = Some(VerticalConfig {
            domain: TaskDomain::Research,
            vocab_boost: 5000,
            quality_metrics: vec!["accuracy".into(), "insight_quality".into()],
            compliance_level: ComplianceLevel::None,
        });
        template.tags = vec!["data".into(), "analysis".into(), "insights".into()];
        template
    }

    /// Creative writer template - optimized for content generation
    ///
    /// **Best for**: Marketing copy, blog posts, creative writing
    /// **Config**: High diversity, quality focus
    pub fn creative_writer() -> Self {
        let mut template = Self::new("creative-writer", AgentType::CreativeWriter);
        template.sona_config.base_lora_rank = 8;
        template.sona_config.pattern_clusters = 50; // Fewer clusters for diversity
        template.sona_config.quality_threshold = 0.5; // Only learn from high quality
        template.training_method = TrainingMethod::RLHF {
            reward_weight: 0.7,
            kl_penalty: 0.05, // Less constraint for creativity
        };
        template.vertical = Some(VerticalConfig {
            domain: TaskDomain::Marketing,
            vocab_boost: 0,
            quality_metrics: vec!["creativity".into(), "engagement".into(), "clarity".into()],
            compliance_level: ComplianceLevel::None,
        });
        template.tags = vec!["creative".into(), "writing".into(), "content".into()];
        template
    }

    /// Reasoning agent template - optimized for logical reasoning
    ///
    /// **Best for**: Math, logic, chain-of-thought reasoning
    /// **Config**: High rank, strong EWC, accuracy focus
    pub fn reasoning_agent() -> Self {
        let mut template = Self::new("reasoning-agent", AgentType::ReasoningAgent);
        template.sona_config.base_lora_rank = 16;
        template.sona_config.ewc_lambda = 3000.0; // Strong protection
        template.sona_config.pattern_clusters = 150;
        template.sona_config.quality_threshold = 0.3;
        template.training_method = TrainingMethod::DPO {
            beta: 0.15,
            ref_weight: 0.4,
        };
        template.tags = vec!["reasoning".into(), "logic".into(), "math".into()];
        template
    }

    //------------------------------------------------------------------
    // Builder Methods
    //------------------------------------------------------------------

    /// Set SONA configuration
    pub fn with_sona_config(mut self, config: SonaConfig) -> Self {
        self.sona_config = config;
        self
    }

    /// Set training method
    pub fn with_training_method(mut self, method: TrainingMethod) -> Self {
        self.training_method = method;
        self
    }

    /// Set vertical configuration
    pub fn with_vertical(mut self, vertical: VerticalConfig) -> Self {
        self.vertical = Some(vertical);
        self
    }

    /// Set memory budget
    pub fn with_memory_budget(mut self, mb: usize) -> Self {
        self.memory_budget_mb = mb;
        self
    }

    /// Set target latency
    pub fn with_target_latency(mut self, us: u64) -> Self {
        self.target_latency_us = us;
        self
    }

    /// Enable continuous learning
    pub fn with_continuous_learning(mut self, enabled: bool) -> Self {
        self.continuous_learning = enabled;
        self
    }

    /// Enable auto-export
    pub fn with_auto_export(mut self, enabled: bool) -> Self {
        self.auto_export = enabled;
        self
    }

    /// Add tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Set hidden dimension
    pub fn with_hidden_dim(mut self, dim: usize) -> Self {
        self.sona_config.hidden_dim = dim;
        self.sona_config.embedding_dim = dim;
        self
    }

    /// Set LoRA ranks
    pub fn with_lora_ranks(mut self, micro: usize, base: usize) -> Self {
        self.sona_config.micro_lora_rank = micro.min(2); // MicroLoRA max rank is 2
        self.sona_config.base_lora_rank = base;
        self
    }

    //------------------------------------------------------------------
    // Internal Methods
    //------------------------------------------------------------------

    /// Apply agent-specific optimizations
    fn apply_agent_optimizations(&mut self) {
        match &self.agent_type {
            AgentType::CodeAgent | AgentType::CodebaseHelper => {
                self.sona_config.pattern_clusters = 200;
                self.sona_config.base_lora_rank = 16;
            }
            AgentType::ChatAgent => {
                self.sona_config.pattern_clusters = 50;
                self.target_latency_us = 500;
            }
            AgentType::RagAgent => {
                self.sona_config.pattern_clusters = 200;
                self.sona_config.trajectory_capacity = 10000;
            }
            AgentType::ReasoningAgent => {
                self.sona_config.ewc_lambda = 3000.0;
                self.sona_config.base_lora_rank = 16;
            }
            AgentType::DomainExpert => {
                self.sona_config.quality_threshold = 0.1;
            }
            _ => {}
        }
    }

    /// Validate template configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.sona_config.micro_lora_rank > 2 {
            return Err("MicroLoRA rank must be 1 or 2".into());
        }
        if self.sona_config.hidden_dim == 0 {
            return Err("Hidden dimension must be > 0".into());
        }
        if self.memory_budget_mb < 1 {
            return Err("Memory budget must be >= 1 MB".into());
        }
        Ok(())
    }

    /// Get estimated memory usage in MB
    pub fn estimated_memory_mb(&self) -> usize {
        let config = &self.sona_config;

        // Base engine memory
        let engine_mb = 5;

        // LoRA weights: hidden_dim * rank * 2 (A and B matrices) * 4 bytes * 2 (micro + base)
        let lora_bytes =
            config.hidden_dim * (config.micro_lora_rank + config.base_lora_rank) * 2 * 4 * 2;
        let lora_mb = lora_bytes / (1024 * 1024);

        // Trajectory buffer: capacity * ~800 bytes per trajectory
        let traj_mb = (config.trajectory_capacity * 800) / (1024 * 1024);

        // Pattern storage: clusters * embedding_dim * 4 bytes
        let pattern_mb = (config.pattern_clusters * config.embedding_dim * 4) / (1024 * 1024);

        engine_mb + lora_mb + traj_mb + pattern_mb + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_creation() {
        let template = TrainingTemplate::code_agent();
        assert_eq!(template.agent_type, AgentType::CodeAgent);
        assert_eq!(template.sona_config.base_lora_rank, 16);
        assert_eq!(template.sona_config.pattern_clusters, 200);
    }

    #[test]
    fn test_preset_templates() {
        let production =
            TrainingTemplate::from_preset(TemplatePreset::Production, AgentType::ChatAgent);
        assert!(production.auto_export);

        let edge = TrainingTemplate::from_preset(TemplatePreset::Edge, AgentType::ChatAgent);
        assert_eq!(edge.memory_budget_mb, 5);
    }

    #[test]
    fn test_domain_expert() {
        let medical = TrainingTemplate::domain_expert(TaskDomain::Healthcare);
        assert!(medical.vertical.is_some());
        if let Some(v) = &medical.vertical {
            assert!(matches!(v.compliance_level, ComplianceLevel::Hipaa));
        }
    }

    #[test]
    fn test_builder_pattern() {
        let template = TrainingTemplate::new("custom", AgentType::Custom("test".into()))
            .with_hidden_dim(512)
            .with_lora_ranks(2, 16)
            .with_memory_budget(200)
            .with_continuous_learning(true);

        assert_eq!(template.sona_config.hidden_dim, 512);
        assert_eq!(template.sona_config.micro_lora_rank, 2);
        assert_eq!(template.sona_config.base_lora_rank, 16);
    }

    #[test]
    fn test_validation() {
        let mut template = TrainingTemplate::code_agent();
        assert!(template.validate().is_ok());

        template.sona_config.micro_lora_rank = 5;
        assert!(template.validate().is_err());
    }

    #[test]
    fn test_memory_estimation() {
        let template = TrainingTemplate::code_agent();
        let mem = template.estimated_memory_mb();
        assert!(mem > 0);
        assert!(mem < template.memory_budget_mb * 2);
    }
}
