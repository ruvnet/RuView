//! Claude Flow Integration for RuvLTRA
//!
//! Optimizes RuvLTRA-Small for Claude Flow use cases:
//! - Agent routing (task -> optimal agent type)
//! - Task classification (code/research/test/review)
//! - Semantic search (memory retrieval queries)
//! - Code generation (Rust/TypeScript output)
//! - HNSW-powered semantic routing (150x faster pattern search)
//! - ReasoningBank for intelligent pattern learning
//! - Multi-phase pretraining pipeline with curriculum learning
//! - **Full Claude API integration with streaming** (NEW)
//! - **Intelligent model routing (Haiku/Sonnet/Opus)** (NEW)
//!
//! ## Model Routing (NEW)
//!
//! Intelligent routing to optimal Claude model based on task complexity:
//!
//! | Model | Token Threshold | Complexity | Use Cases |
//! |-------|-----------------|------------|-----------|
//! | Haiku | < 500 tokens | Simple | Bug fixes, formatting, simple transforms |
//! | Sonnet | 500-2000 tokens | Moderate | Feature impl, refactoring, testing |
//! | Opus | > 2000 tokens | Complex | Architecture, security, deep reasoning |
//!
//! ```rust,ignore
//! use ruvllm::claude_flow::{ModelRouter, SelectionCriteria, ClaudeModel};
//!
//! let mut router = ModelRouter::new();
//!
//! // Route task to optimal model
//! let decision = router.route("implement a REST API endpoint", None, None);
//! println!("Model: {:?}, cost: ${:.4}", decision.model, decision.estimated_cost);
//!
//! // With cost preference
//! router.set_criteria(SelectionCriteria { prefer_cost: true, ..Default::default() });
//! let decision = router.route("fix a typo", None, None);
//! assert_eq!(decision.model, ClaudeModel::Haiku);
//! ```
//!
//! ## Multi-Agent Coordination (NEW)
//!
//! The [`AgentCoordinator`] orchestrates multi-agent workflows:
//!
//! ```rust,ignore
//! use ruvllm::claude_flow::{AgentCoordinator, ClaudeModel, AgentType, WorkflowStep};
//!
//! let mut coordinator = AgentCoordinator::new(ClaudeModel::Sonnet, 10);
//!
//! // Define workflow steps with dependencies
//! let steps = vec![
//!     WorkflowStep { step_id: "research".into(), agent_type: AgentType::Researcher, .. },
//!     WorkflowStep { step_id: "design".into(), agent_type: AgentType::Architect,
//!                    dependencies: vec!["research".into()], .. },
//!     WorkflowStep { step_id: "implement".into(), agent_type: AgentType::Coder,
//!                    dependencies: vec!["design".into()], .. },
//! ];
//!
//! // Execute with automatic dependency resolution
//! let result = coordinator.execute_workflow("my-workflow".into(), steps).await?;
//! println!("Total cost: ${:.4}", result.total_cost);
//! ```
//!
//! ## Advanced Pretraining Pipeline
//!
//! The [`PretrainPipeline`] provides a multi-phase pretraining system:
//!
//! - **Bootstrap Phase**: Seed patterns from agent keywords and typical tasks
//! - **Synthetic Phase**: Generate diverse training samples per agent type
//! - **Reinforce Phase**: Replay successful trajectories with SONA
//! - **Consolidate Phase**: EWC++ to lock in learned patterns
//!
//! ```rust,ignore
//! use ruvllm::claude_flow::{PretrainPipeline, PretrainConfig, Phase};
//!
//! let config = PretrainConfig::for_claude_flow();
//! let mut pipeline = PretrainPipeline::new(config);
//!
//! // Run full pretraining
//! let result = pipeline.run_full_pipeline()?;
//! println!("Trained {} patterns with {:.2}% quality", result.total_patterns, result.avg_quality * 100.0);
//!
//! // Save checkpoint
//! pipeline.save_checkpoint("./checkpoints/claude_flow_v1.bin")?;
//! ```
//!
//! ## Task Generation
//!
//! The [`TaskGenerator`] creates realistic training data for pretraining:
//!
//! - Coding tasks: implement, fix, refactor, optimize
//! - Research tasks: analyze, investigate, explore
//! - Review tasks: audit, inspect, verify
//! - Architecture tasks: design, structure, plan
//!
//! ```rust,ignore
//! use ruvllm::claude_flow::{TaskGenerator, TaskCategory, TaskComplexity};
//!
//! let mut generator = TaskGenerator::new();
//!
//! // Generate tasks for specific category
//! let task = generator.generate(TaskCategory::Coding, TaskComplexity::Moderate);
//! println!("Task: {}", task.description);
//!
//! // Generate for specific agent
//! let research_task = generator.generate_for_agent(ClaudeFlowAgent::Researcher, TaskComplexity::Complex);
//!
//! // Generate balanced batch
//! let tasks = generator.generate_balanced_batch(100);
//! ```
//!
//! ## HNSW Semantic Router
//!
//! The [`HnswRouter`] provides 150x faster pattern matching for task routing
//! using ruvector-core's HNSW index. It supports:
//!
//! - Semantic nearest-neighbor search for task patterns
//! - Online learning (add new patterns as tasks succeed)
//! - Integration with SONA for continuous improvement
//! - Hybrid routing combining keyword and semantic methods
//!
//! ```rust,ignore
//! use ruvllm::claude_flow::{HnswRouter, HnswRouterConfig, TaskPattern, AgentType, ClaudeFlowTask};
//!
//! let config = HnswRouterConfig::default();
//! let router = HnswRouter::new(config)?;
//!
//! // Add learned patterns
//! let pattern = TaskPattern::new(
//!     embedding,
//!     AgentType::Coder,
//!     ClaudeFlowTask::CodeGeneration,
//!     "implement a function".to_string(),
//! );
//! router.add_pattern(pattern)?;
//!
//! // Route by semantic similarity
//! let result = router.route_by_similarity(&query_embedding)?;
//! println!("Best agent: {:?}, confidence: {}", result.primary_agent, result.confidence);
//! ```
//!
//! ## ReasoningBank Integration
//!
//! The [`ReasoningBankIntegration`] provides intelligent pattern learning with:
//!
//! - **Trajectory Storage**: Records task executions with verdict judgments (success/failure/partial)
//! - **Memory Distillation**: Extracts key patterns from multiple trajectories using K-means clustering
//! - **EWC++ Consolidation**: Prevents catastrophic forgetting of learned patterns
//! - **Pattern-based Routing**: Recommends agents based on historical successes
//!
//! ```rust,ignore
//! use ruvllm::claude_flow::{
//!     ReasoningBankIntegration, ReasoningBankConfig, Verdict, TrajectoryStep, AgentType
//! };
//!
//! let config = ReasoningBankConfig::default();
//! let bank = ReasoningBankIntegration::new(config);
//!
//! // Record a successful task execution
//! let steps = vec![
//!     TrajectoryStep::new("analyze_requirements", 0.8).with_agent(AgentType::Researcher),
//!     TrajectoryStep::new("implement_code", 0.9).with_agent(AgentType::Coder),
//!     TrajectoryStep::new("run_tests", 0.95).with_agent(AgentType::Tester),
//! ];
//! bank.record_trajectory(
//!     "task-123",
//!     &embedding,
//!     steps,
//!     Verdict::Success { reason: "All tests passed".into() },
//! ).unwrap();
//!
//! // Distill patterns after accumulating trajectories
//! bank.distill_patterns().unwrap();
//!
//! // Get routing recommendation for a new task
//! let rec = bank.get_recommendation(&new_embedding);
//! println!("Recommended: {:?} (confidence: {:.2})", rec.agent, rec.confidence);
//!
//! // Periodically consolidate to prevent forgetting
//! bank.consolidate().unwrap();
//! ```

use serde::{Deserialize, Serialize};

mod agent_router;
mod claude_integration;
mod flow_optimizer;
mod hnsw_router;
mod hooks_integration;
mod model_router;
mod pretrain_pipeline;
mod reasoning_bank;
mod task_classifier;
mod task_generator;

pub use agent_router::{AgentRouter, AgentType, RoutingDecision};
pub use flow_optimizer::{FlowOptimizer, OptimizationConfig, OptimizationResult};
pub use hnsw_router::{
    HnswDistanceMetric, HnswRouter, HnswRouterConfig, HnswRouterStats, HnswRoutingResult,
    HybridRouter, TaskPattern,
};
pub use pretrain_pipeline::{
    Checkpoint, CurriculumScheduler, CurriculumStats, Phase, PhaseResult, PipelineResult,
    PretrainConfig, PretrainPipeline, ProgressTracker, QualityGate, QualityGateStats,
    SerializedPattern,
};
pub use reasoning_bank::{
    DistilledPattern, ReasoningBankConfig, ReasoningBankIntegration, ReasoningBankStats,
    RoutingRecommendation, Trajectory, TrajectoryStep, Verdict,
};
pub use task_classifier::{ClassificationResult, TaskClassifier, TaskType};
pub use task_generator::{seed_rng, GeneratedTask, TaskCategory, TaskComplexity, TaskGenerator};

// Hooks Integration exports (NEW v2.3)
pub use hooks_integration::{
    HooksConfig, HooksIntegration, LearningMetrics, PatternMatch, PostEditInput, PostEditResult,
    PostTaskInput, PostTaskResult, PreEditInput, PreEditResult, PreTaskInput, PreTaskResult,
    QualityAssessment, SessionEndResult, SessionMetrics, SessionState,
};

// Claude API Integration exports (NEW)
pub use claude_integration::{
    AgentContext,
    AgentCoordinator,
    // Multi-agent coordination
    AgentState,
    // Core types
    ClaudeModel,
    ClaudeRequest,
    ClaudeResponse,
    ContentBlock,
    ContextManager,
    // Context management
    ContextWindow,
    CoordinatorStats,
    // Cost and latency tracking
    CostEstimator,
    LatencySample,
    LatencyStats,
    LatencyTracker,
    Message,
    MessageRole,
    QualityMonitor,
    ResponseStreamer,
    StepResult,
    StreamEvent,
    StreamStats,
    // Streaming
    StreamToken,
    UsageStats,
    WorkflowResult,
    WorkflowStep,
};

// Model Router exports (NEW)
pub use model_router::{
    AnalyzerStats,
    // Complexity analysis
    ComplexityFactors,
    ComplexityScore,
    ComplexityWeights,
    // Integrated router
    ModelRouter,
    ModelRoutingDecision,
    ModelSelector,
    // Model selection
    SelectionCriteria,
    SelectorStats,
    TaskComplexityAnalyzer,
};

/// Claude Flow agent types supported by RuvLTRA routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClaudeFlowAgent {
    /// Code implementation specialist
    Coder,
    /// Research and analysis specialist
    Researcher,
    /// Testing and validation specialist
    Tester,
    /// Code review specialist
    Reviewer,
    /// System architecture specialist
    Architect,
    /// Security audit specialist
    SecurityAuditor,
    /// Performance optimization specialist
    PerformanceEngineer,
    /// Machine learning specialist
    MlDeveloper,
    /// Backend development specialist
    BackendDev,
    /// CI/CD engineering specialist
    CicdEngineer,
}

impl ClaudeFlowAgent {
    /// Get all agent types
    pub fn all() -> &'static [ClaudeFlowAgent] {
        &[
            Self::Coder,
            Self::Researcher,
            Self::Tester,
            Self::Reviewer,
            Self::Architect,
            Self::SecurityAuditor,
            Self::PerformanceEngineer,
            Self::MlDeveloper,
            Self::BackendDev,
            Self::CicdEngineer,
        ]
    }

    /// Get agent name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Coder => "coder",
            Self::Researcher => "researcher",
            Self::Tester => "tester",
            Self::Reviewer => "reviewer",
            Self::Architect => "system-architect",
            Self::SecurityAuditor => "security-auditor",
            Self::PerformanceEngineer => "performance-engineer",
            Self::MlDeveloper => "ml-developer",
            Self::BackendDev => "backend-dev",
            Self::CicdEngineer => "cicd-engineer",
        }
    }

    /// Get typical task keywords for this agent
    pub fn keywords(&self) -> &'static [&'static str] {
        match self {
            Self::Coder => &[
                "implement",
                "code",
                "write",
                "create",
                "build",
                "develop",
                "function",
                "class",
            ],
            Self::Researcher => &[
                "research",
                "analyze",
                "investigate",
                "explore",
                "find",
                "search",
                "understand",
            ],
            Self::Tester => &[
                "test",
                "verify",
                "validate",
                "check",
                "assert",
                "coverage",
                "unit",
                "integration",
            ],
            Self::Reviewer => &[
                "review",
                "audit",
                "inspect",
                "quality",
                "lint",
                "style",
                "best practice",
            ],
            Self::Architect => &[
                "design",
                "architecture",
                "structure",
                "pattern",
                "system",
                "scalable",
                "modular",
            ],
            Self::SecurityAuditor => &[
                "security",
                "vulnerability",
                "cve",
                "injection",
                "auth",
                "encrypt",
                "safe",
            ],
            Self::PerformanceEngineer => &[
                "performance",
                "optimize",
                "speed",
                "memory",
                "benchmark",
                "profile",
                "latency",
            ],
            Self::MlDeveloper => &[
                "model",
                "train",
                "neural",
                "ml",
                "ai",
                "embedding",
                "inference",
                "tensor",
            ],
            Self::BackendDev => &[
                "api", "endpoint", "database", "server", "rest", "graphql", "query",
            ],
            Self::CicdEngineer => &[
                "ci", "cd", "pipeline", "deploy", "workflow", "action", "build", "release",
            ],
        }
    }
}

/// Claude Flow task types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClaudeFlowTask {
    /// Code generation task
    CodeGeneration,
    /// Code review task
    CodeReview,
    /// Testing task
    Testing,
    /// Research task
    Research,
    /// Documentation task
    Documentation,
    /// Debugging task
    Debugging,
    /// Refactoring task
    Refactoring,
    /// Security audit task
    Security,
    /// Performance optimization task
    Performance,
    /// Architecture design task
    Architecture,
}

impl ClaudeFlowTask {
    /// Get recommended agents for this task type
    pub fn recommended_agents(&self) -> &'static [ClaudeFlowAgent] {
        match self {
            Self::CodeGeneration => &[ClaudeFlowAgent::Coder, ClaudeFlowAgent::BackendDev],
            Self::CodeReview => &[ClaudeFlowAgent::Reviewer, ClaudeFlowAgent::SecurityAuditor],
            Self::Testing => &[ClaudeFlowAgent::Tester, ClaudeFlowAgent::Coder],
            Self::Research => &[ClaudeFlowAgent::Researcher, ClaudeFlowAgent::Architect],
            Self::Documentation => &[ClaudeFlowAgent::Researcher, ClaudeFlowAgent::Coder],
            Self::Debugging => &[ClaudeFlowAgent::Coder, ClaudeFlowAgent::Tester],
            Self::Refactoring => &[ClaudeFlowAgent::Coder, ClaudeFlowAgent::Architect],
            Self::Security => &[ClaudeFlowAgent::SecurityAuditor, ClaudeFlowAgent::Reviewer],
            Self::Performance => &[ClaudeFlowAgent::PerformanceEngineer, ClaudeFlowAgent::Coder],
            Self::Architecture => &[ClaudeFlowAgent::Architect, ClaudeFlowAgent::Reviewer],
        }
    }
}
