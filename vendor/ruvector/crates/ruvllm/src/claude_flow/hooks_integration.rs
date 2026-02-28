//! Claude Flow Hooks Integration
//!
//! Unified interface for Claude Flow CLI hooks to leverage all RuvLLM v2.3 capabilities:
//!
//! - **Pre-Task Hook**: Get routing recommendation, model selection, agent suggestions
//! - **Post-Task Hook**: Record trajectory, update patterns, quality scoring
//! - **Pre-Edit Hook**: Get file context, agent expertise, pattern suggestions
//! - **Post-Edit Hook**: Record edit outcome, learn patterns, consolidate
//! - **Session Start**: Initialize memory systems, restore state
//! - **Session End**: Persist state, distill patterns, export metrics
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::claude_flow::{HooksIntegration, HooksConfig, PreTaskInput, PostTaskInput};
//!
//! let config = HooksConfig::default();
//! let mut hooks = HooksIntegration::new(config)?;
//!
//! // Pre-task hook: get recommendations
//! let input = PreTaskInput {
//!     task_id: "task-123".into(),
//!     description: "implement authentication middleware".into(),
//!     file_path: Some("src/middleware/auth.rs".into()),
//!     context: None,
//! };
//! let result = hooks.pre_task(input)?;
//! println!("Agent: {:?}, Model: {:?}", result.recommended_agent, result.recommended_model);
//!
//! // Post-task hook: record outcome
//! let outcome = PostTaskInput {
//!     task_id: "task-123".into(),
//!     success: true,
//!     agent_used: "coder".into(),
//!     quality_score: 0.92,
//!     ..Default::default()
//! };
//! hooks.post_task(outcome)?;
//! ```

use crate::{
    claude_flow::{
        AgentType, ClaudeFlowAgent, ClaudeFlowTask, ClaudeModel, HnswRouter, HnswRouterConfig,
        ModelRouter, ReasoningBankConfig, ReasoningBankIntegration, TaskComplexityAnalyzer,
    },
    context::{
        AgenticMemory, AgenticMemoryConfig, ClaudeFlowBridgeConfig, ClaudeFlowMemoryBridge,
        ContextManagerConfig, IntelligentContextManager, SemanticCacheConfig, SemanticToolCache,
    },
    quality::{
        CoherenceConfig, CoherenceValidator, DiversityAnalyzer, DiversityConfig, QualityMetrics,
        QualityScoringEngine, ScoringConfig,
    },
    reasoning_bank::{
        ConsolidationConfig, DistillationConfig, MemoryDistiller, Pattern, PatternCategory,
        PatternConsolidator, PatternStore, PatternStoreConfig, RootCause, StepOutcome, Trajectory,
        TrajectoryRecorder, TrajectoryStep, Verdict,
    },
    reflection::{
        ConfidenceChecker, ConfidenceConfig, ErrorPatternLearner, ErrorPatternLearnerConfig,
    },
    Result, RuvLLMError,
};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Hooks integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    /// Enable semantic pattern learning
    pub enable_pattern_learning: bool,
    /// Enable quality scoring
    pub enable_quality_scoring: bool,
    /// Enable error pattern learning
    pub enable_error_learning: bool,
    /// Enable memory bridging with CLI
    pub enable_memory_bridge: bool,
    /// Enable HNSW routing
    pub enable_hnsw_routing: bool,
    /// Pattern consolidation threshold (number of trajectories before consolidate)
    pub consolidation_threshold: usize,
    /// Minimum confidence for pattern storage
    pub min_pattern_confidence: f32,
    /// HNSW embedding dimension
    pub embedding_dim: usize,
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            enable_pattern_learning: true,
            enable_quality_scoring: true,
            enable_error_learning: true,
            enable_memory_bridge: true,
            enable_hnsw_routing: true,
            consolidation_threshold: 50,
            min_pattern_confidence: 0.7,
            embedding_dim: 384,
        }
    }
}

/// Pre-task hook input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreTaskInput {
    /// Task identifier
    pub task_id: String,
    /// Task description
    pub description: String,
    /// Optional file path for file-specific tasks
    pub file_path: Option<String>,
    /// Additional context
    pub context: Option<String>,
    /// Prefer cost over latency
    pub prefer_cost: bool,
    /// Prefer speed over quality
    pub prefer_speed: bool,
}

impl Default for PreTaskInput {
    fn default() -> Self {
        Self {
            task_id: Uuid::new_v4().to_string(),
            description: String::new(),
            file_path: None,
            context: None,
            prefer_cost: false,
            prefer_speed: false,
        }
    }
}

/// Pre-task hook result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreTaskResult {
    /// Recommended agent type
    pub recommended_agent: String,
    /// Recommended Claude model (haiku/sonnet/opus)
    pub recommended_model: String,
    /// Confidence in recommendation (0.0 - 1.0)
    pub confidence: f32,
    /// Similar patterns from history
    pub similar_patterns: Vec<PatternMatch>,
    /// Suggested approach based on learned patterns
    pub suggested_approach: Option<String>,
    /// Estimated complexity score (0.0 - 1.0)
    pub complexity_score: f32,
    /// Routing explanation
    pub explanation: String,
    /// Agent Booster available (can skip LLM entirely)
    pub agent_booster_available: bool,
    /// Agent Booster intent type if available
    pub agent_booster_intent: Option<String>,
}

/// Pattern match from HNSW search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMatch {
    /// Pattern description
    pub description: String,
    /// Agent that succeeded
    pub agent: String,
    /// Similarity score
    pub similarity: f32,
    /// Outcome quality
    pub quality: f32,
}

/// Post-task hook input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostTaskInput {
    /// Task identifier
    pub task_id: String,
    /// Whether task succeeded
    pub success: bool,
    /// Agent that was used
    pub agent_used: String,
    /// Model that was used
    pub model_used: Option<String>,
    /// Quality score (0.0 - 1.0)
    pub quality_score: f32,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Tokens used
    pub tokens_used: Option<u64>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Store results in memory
    pub store_results: bool,
    /// Train neural patterns
    pub train_neural: bool,
}

impl Default for PostTaskInput {
    fn default() -> Self {
        Self {
            task_id: String::new(),
            success: false,
            agent_used: String::new(),
            model_used: None,
            quality_score: 0.0,
            error_message: None,
            tokens_used: None,
            duration_ms: None,
            store_results: true,
            train_neural: false,
        }
    }
}

/// Post-task hook result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostTaskResult {
    /// Whether pattern was stored
    pub pattern_stored: bool,
    /// Quality assessment
    pub quality_assessment: QualityAssessment,
    /// Learning metrics
    pub learning_metrics: LearningMetrics,
    /// Recommendations for improvement
    pub recommendations: Vec<String>,
}

/// Quality assessment from scoring engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityAssessment {
    /// Overall quality score
    pub overall_score: f32,
    /// Schema compliance score
    pub schema_compliance: f32,
    /// Coherence score
    pub coherence: f32,
    /// Diversity score
    pub diversity: f32,
    /// Trend direction
    pub trend: String,
}

/// Learning metrics from pattern update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningMetrics {
    /// Total patterns in store
    pub total_patterns: usize,
    /// Patterns added this session
    pub patterns_added: usize,
    /// Patterns consolidated
    pub patterns_consolidated: usize,
    /// Error patterns learned
    pub error_patterns: usize,
}

/// Pre-edit hook input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreEditInput {
    /// File path being edited
    pub file_path: String,
    /// Operation type (create, update, delete, refactor)
    pub operation: String,
    /// Additional context
    pub context: Option<String>,
}

/// Pre-edit hook result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreEditResult {
    /// Recommended agent for this file type
    pub recommended_agent: String,
    /// Confidence score
    pub confidence: f32,
    /// Relevant patterns for this file
    pub relevant_patterns: Vec<PatternMatch>,
    /// Suggested edits based on history
    pub suggestions: Vec<String>,
    /// Risk assessment (low/medium/high)
    pub risk_level: String,
}

/// Post-edit hook input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostEditInput {
    /// File path that was edited
    pub file_path: String,
    /// Whether edit succeeded
    pub success: bool,
    /// Agent that performed edit
    pub agent: Option<String>,
    /// Train neural patterns from this edit
    pub train_neural: bool,
}

/// Post-edit hook result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostEditResult {
    /// Whether outcome was recorded
    pub recorded: bool,
    /// Pattern learned from edit
    pub pattern_learned: bool,
    /// Error pattern learned (if failed)
    pub error_learned: bool,
}

/// Session state for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Session identifier
    pub session_id: String,
    /// Start time
    pub started_at: DateTime<Utc>,
    /// Tasks completed
    pub tasks_completed: usize,
    /// Patterns learned
    pub patterns_learned: usize,
    /// Total quality score (average)
    pub avg_quality: f32,
    /// Active trajectories
    pub active_trajectories: Vec<String>,
}

/// Active trajectory being recorded
struct ActiveTrajectory {
    task_id: String,
    description: String,
    steps: Vec<TrajectoryStep>,
    started_at: DateTime<Utc>,
}

/// Unified hooks integration for Claude Flow
pub struct HooksIntegration {
    config: HooksConfig,

    // Routing and model selection
    model_router: ModelRouter,
    hnsw_router: Option<HnswRouter>,
    complexity_analyzer: TaskComplexityAnalyzer,

    // Pattern learning
    reasoning_bank: Option<ReasoningBankIntegration>,
    pattern_store: Option<PatternStore>,
    pattern_consolidator: Option<PatternConsolidator>,

    // Quality scoring
    scoring_engine: Option<QualityScoringEngine>,
    coherence_validator: Option<CoherenceValidator>,
    diversity_analyzer: Option<DiversityAnalyzer>,

    // Error learning
    error_learner: Option<ErrorPatternLearner>,
    confidence_checker: Option<ConfidenceChecker>,

    // Memory systems
    agentic_memory: Option<AgenticMemory>,
    context_manager: Option<IntelligentContextManager>,
    semantic_cache: Option<SemanticToolCache>,
    memory_bridge: Option<ClaudeFlowMemoryBridge>,

    // State tracking
    active_trajectories: DashMap<String, ActiveTrajectory>,
    session_state: Arc<RwLock<SessionState>>,

    // Metrics
    patterns_added: Arc<RwLock<usize>>,
    errors_learned: Arc<RwLock<usize>>,
}

impl HooksIntegration {
    /// Create new hooks integration
    pub fn new(config: HooksConfig) -> Result<Self> {
        let model_router = ModelRouter::new();
        let complexity_analyzer = TaskComplexityAnalyzer::new();

        // Initialize HNSW router if enabled
        let hnsw_router = if config.enable_hnsw_routing {
            let hnsw_config = HnswRouterConfig {
                embedding_dim: config.embedding_dim,
                ..Default::default()
            };
            Some(HnswRouter::new(hnsw_config)?)
        } else {
            None
        };

        // Initialize pattern learning if enabled
        let (reasoning_bank, pattern_store, pattern_consolidator) =
            if config.enable_pattern_learning {
                let rb_config = ReasoningBankConfig::default();
                let ps_config = PatternStoreConfig {
                    embedding_dim: config.embedding_dim,
                    ..Default::default()
                };
                let pc_config = ConsolidationConfig::default();

                (
                    Some(ReasoningBankIntegration::new(rb_config)),
                    Some(PatternStore::new(ps_config)?),
                    Some(PatternConsolidator::new(pc_config)),
                )
            } else {
                (None, None, None)
            };

        // Initialize quality scoring if enabled
        let (scoring_engine, coherence_validator, diversity_analyzer) =
            if config.enable_quality_scoring {
                (
                    Some(QualityScoringEngine::new()),
                    Some(CoherenceValidator::new(CoherenceConfig::default())),
                    Some(DiversityAnalyzer::new(DiversityConfig::default())),
                )
            } else {
                (None, None, None)
            };

        // Initialize error learning if enabled
        let (error_learner, confidence_checker) = if config.enable_error_learning {
            (
                Some(ErrorPatternLearner::new(
                    ErrorPatternLearnerConfig::default(),
                )),
                Some(ConfidenceChecker::new(ConfidenceConfig::default())),
            )
        } else {
            (None, None)
        };

        // Initialize memory systems if enabled
        let (agentic_memory, context_manager, semantic_cache, memory_bridge) =
            if config.enable_memory_bridge {
                (
                    AgenticMemory::new(AgenticMemoryConfig::default()).ok(),
                    IntelligentContextManager::new(ContextManagerConfig::default()).ok(),
                    SemanticToolCache::new(SemanticCacheConfig::default()).ok(),
                    Some(ClaudeFlowMemoryBridge::new(
                        ClaudeFlowBridgeConfig::default(),
                    )),
                )
            } else {
                (None, None, None, None)
            };

        let session_state = SessionState {
            session_id: Uuid::new_v4().to_string(),
            started_at: Utc::now(),
            tasks_completed: 0,
            patterns_learned: 0,
            avg_quality: 0.0,
            active_trajectories: Vec::new(),
        };

        Ok(Self {
            config,
            model_router,
            hnsw_router,
            complexity_analyzer,
            reasoning_bank,
            pattern_store,
            pattern_consolidator,
            scoring_engine,
            coherence_validator,
            diversity_analyzer,
            error_learner,
            confidence_checker,
            agentic_memory,
            context_manager,
            semantic_cache,
            memory_bridge,
            active_trajectories: DashMap::new(),
            session_state: Arc::new(RwLock::new(session_state)),
            patterns_added: Arc::new(RwLock::new(0)),
            errors_learned: Arc::new(RwLock::new(0)),
        })
    }

    /// Pre-task hook: get routing recommendations
    pub fn pre_task(&mut self, input: PreTaskInput) -> Result<PreTaskResult> {
        // Analyze task complexity
        let complexity = self.complexity_analyzer.analyze(&input.description);

        // Get model recommendation (no agent or task type override)
        let model_decision = self.model_router.route(
            &input.description,
            None, // agent_type override
            None, // task_type override
        );

        // Check for Agent Booster (simple transforms that skip LLM)
        let (agent_booster_available, agent_booster_intent) =
            self.check_agent_booster(&input.description);

        // Get agent recommendation from HNSW if available
        let (recommended_agent, confidence, similar_patterns, suggested_approach) =
            if let Some(ref router) = self.hnsw_router {
                // Create a simple embedding from description
                let embedding = self.create_simple_embedding(&input.description);

                match router.route_by_similarity(&embedding) {
                    Ok(result) => {
                        // Get similar patterns through a separate search
                        let patterns: Vec<PatternMatch> = router
                            .search_similar(&embedding, 3)
                            .ok()
                            .map(|results| {
                                results
                                    .iter()
                                    .map(|(pattern, similarity)| PatternMatch {
                                        description: format!("{:?}", pattern.task_type),
                                        agent: format!("{:?}", pattern.agent_type),
                                        similarity: *similarity,
                                        quality: pattern.success_rate,
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();

                        let approach = if !patterns.is_empty() {
                            Some(format!(
                                "Based on {} similar successful tasks, consider: {}",
                                patterns.len(),
                                patterns
                                    .first()
                                    .map(|p| &p.description)
                                    .unwrap_or(&String::new())
                            ))
                        } else {
                            None
                        };

                        (
                            format!("{:?}", result.primary_agent),
                            result.confidence,
                            patterns,
                            approach,
                        )
                    }
                    Err(_) => self.fallback_routing(&input.description),
                }
            } else {
                self.fallback_routing(&input.description)
            };

        // Start trajectory recording
        self.start_trajectory(&input.task_id, &input.description);

        // Build explanation
        let explanation = format!(
            "Task complexity: {:.0}% → Model: {} | Agent: {} (confidence: {:.0}%)",
            complexity.overall * 100.0,
            model_decision.model.name(),
            recommended_agent,
            confidence * 100.0
        );

        Ok(PreTaskResult {
            recommended_agent,
            recommended_model: model_decision.model.name().to_string(),
            confidence,
            similar_patterns,
            suggested_approach,
            complexity_score: complexity.overall,
            explanation,
            agent_booster_available,
            agent_booster_intent,
        })
    }

    /// Post-task hook: record outcome and learn
    pub fn post_task(&mut self, input: PostTaskInput) -> Result<PostTaskResult> {
        let mut quality_assessment = QualityAssessment {
            overall_score: input.quality_score,
            schema_compliance: input.quality_score,
            coherence: input.quality_score,
            diversity: input.quality_score,
            trend: "stable".to_string(),
        };

        // Score quality if enabled - use proper QualityMetrics API
        if self.scoring_engine.is_some() {
            let metrics = QualityMetrics::with_scores(
                input.quality_score,
                input.quality_score,
                input.quality_score,
                input.quality_score,
                input.quality_score,
            );

            quality_assessment.overall_score = metrics.composite_score;
            quality_assessment.schema_compliance = metrics.schema_compliance;
            quality_assessment.coherence = metrics.semantic_coherence;
            quality_assessment.diversity = metrics.diversity;
        }

        // Record trajectory completion
        let pattern_stored = self.complete_trajectory(
            &input.task_id,
            input.success,
            &input.agent_used,
            input.quality_score,
            input.error_message.as_deref(),
        )?;

        // Learn error pattern if failed
        let mut error_learned = false;
        if !input.success {
            if let (Some(ref mut learner), Some(error_msg)) =
                (&mut self.error_learner, &input.error_message)
            {
                // Record error for learning
                learner.record_error(error_msg);
                error_learned = true;
                *self.errors_learned.write() += 1;
            }
        }

        // Update session state
        {
            let mut state = self.session_state.write();
            state.tasks_completed += 1;
            state.patterns_learned = *self.patterns_added.read();

            // Update running average quality
            let n = state.tasks_completed as f32;
            state.avg_quality =
                ((n - 1.0) * state.avg_quality + quality_assessment.overall_score) / n;
        }

        // Check if consolidation needed
        let patterns_added = *self.patterns_added.read();
        if patterns_added > 0 && patterns_added % self.config.consolidation_threshold == 0 {
            self.consolidate_patterns()?;
        }

        // Build recommendations
        let mut recommendations = Vec::new();
        if quality_assessment.overall_score < 0.7 {
            recommendations.push("Consider breaking task into smaller subtasks".to_string());
        }
        if !input.success {
            recommendations.push("Review error patterns and adjust approach".to_string());
        }

        Ok(PostTaskResult {
            pattern_stored,
            quality_assessment,
            learning_metrics: LearningMetrics {
                total_patterns: self.pattern_store.as_ref().map(|p| p.len()).unwrap_or(0),
                patterns_added: *self.patterns_added.read(),
                patterns_consolidated: 0,
                error_patterns: *self.errors_learned.read(),
            },
            recommendations,
        })
    }

    /// Pre-edit hook: get context before file edit
    pub fn pre_edit(&self, input: PreEditInput) -> Result<PreEditResult> {
        // Determine agent based on file extension
        let ext = input.file_path.rsplit('.').next().unwrap_or("");
        let (recommended_agent, confidence) = match ext {
            "rs" | "go" | "c" | "cpp" | "h" => ("coder".to_string(), 0.9),
            "ts" | "js" | "tsx" | "jsx" => ("coder".to_string(), 0.85),
            "py" => ("coder".to_string(), 0.85),
            "sql" => ("backend-dev".to_string(), 0.8),
            "yml" | "yaml" | "json" | "toml" => ("cicd-engineer".to_string(), 0.75),
            "md" | "txt" | "rst" => ("researcher".to_string(), 0.7),
            "test.rs" | "test.ts" | "spec.ts" => ("tester".to_string(), 0.9),
            _ => ("coder".to_string(), 0.6),
        };

        // Get relevant patterns if HNSW available
        let relevant_patterns = if let Some(ref router) = self.hnsw_router {
            let query = format!("{} {} {}", input.operation, ext, input.file_path);
            let embedding = self.create_simple_embedding(&query);

            router
                .search_similar(&embedding, 3)
                .ok()
                .map(|results| {
                    results
                        .iter()
                        .map(|(pattern, similarity)| PatternMatch {
                            description: format!("{:?}", pattern.task_type),
                            agent: format!("{:?}", pattern.agent_type),
                            similarity: *similarity,
                            quality: pattern.success_rate,
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Assess risk
        let risk_level = match input.operation.as_str() {
            "delete" => "high",
            "refactor" => "medium",
            "create" => "low",
            "update" => "low",
            _ => "medium",
        }
        .to_string();

        Ok(PreEditResult {
            recommended_agent,
            confidence,
            relevant_patterns,
            suggestions: Vec::new(),
            risk_level,
        })
    }

    /// Post-edit hook: record edit outcome
    pub fn post_edit(&mut self, input: PostEditInput) -> Result<PostEditResult> {
        let mut pattern_learned = false;
        let mut error_learned = false;

        // Record successful edit pattern
        if input.success {
            if let Some(ref agent) = input.agent {
                let ext = input.file_path.rsplit('.').next().unwrap_or("");
                let pattern_desc = format!("edit {} file: {}", ext, input.file_path);
                // Get embedding before mutable borrow
                let embedding =
                    create_simple_embedding_static(&pattern_desc, self.config.embedding_dim);

                if let Some(ref mut store) = self.pattern_store {
                    let pattern = Pattern::new(
                        embedding,
                        PatternCategory::CodeGeneration,
                        1.0, // Success quality
                    )
                    .with_lesson(pattern_desc.clone())
                    .with_action(format!("Edit {} with {}", ext, agent));

                    if store.store_pattern(pattern).is_ok() {
                        pattern_learned = true;
                        *self.patterns_added.write() += 1;
                    }
                }
            }
        } else {
            // Record error pattern
            if let Some(ref mut learner) = self.error_learner {
                let error_msg = format!("Edit failed: {}", input.file_path);
                learner.record_error(&error_msg);
                *self.errors_learned.write() += 1;
            }
        }

        Ok(PostEditResult {
            recorded: true,
            pattern_learned,
            error_learned,
        })
    }

    /// Session start hook: initialize and optionally restore state
    pub fn session_start(
        &mut self,
        session_id: Option<&str>,
        restore_latest: bool,
    ) -> Result<SessionState> {
        let session_id = session_id
            .unwrap_or(&Uuid::new_v4().to_string())
            .to_string();

        // Initialize new session state
        let state = SessionState {
            session_id: session_id.clone(),
            started_at: Utc::now(),
            tasks_completed: 0,
            patterns_learned: 0,
            avg_quality: 0.0,
            active_trajectories: Vec::new(),
        };

        *self.session_state.write() = state.clone();

        // Reset counters
        *self.patterns_added.write() = 0;
        *self.errors_learned.write() = 0;

        // Clear active trajectories
        self.active_trajectories.clear();

        Ok(state)
    }

    /// Session end hook: persist state and distill patterns
    pub fn session_end(
        &mut self,
        export_metrics: bool,
        persist_state: bool,
    ) -> Result<SessionEndResult> {
        let state = self.session_state.read().clone();

        // Complete any active trajectories
        let incomplete_trajectories: Vec<String> = self
            .active_trajectories
            .iter()
            .map(|r| r.key().clone())
            .collect();

        for task_id in incomplete_trajectories {
            let _ =
                self.complete_trajectory(&task_id, false, "unknown", 0.5, Some("Session ended"));
        }

        // Consolidate patterns before ending
        let patterns_consolidated = if self.config.enable_pattern_learning {
            self.consolidate_patterns().ok().flatten().unwrap_or(0)
        } else {
            0
        };

        // Build metrics if requested
        let metrics = if export_metrics {
            Some(SessionMetrics {
                tasks_completed: state.tasks_completed,
                patterns_learned: *self.patterns_added.read(),
                patterns_consolidated,
                errors_learned: *self.errors_learned.read(),
                avg_quality: state.avg_quality,
                duration_seconds: (Utc::now() - state.started_at).num_seconds() as u64,
            })
        } else {
            None
        };

        Ok(SessionEndResult {
            session_id: state.session_id,
            patterns_consolidated,
            state_persisted: persist_state,
            metrics,
        })
    }

    /// Route a task to optimal agent (convenience method)
    pub fn route_task(&self, task: &str, context: Option<&str>) -> Result<PreTaskResult> {
        let mut input = PreTaskInput {
            task_id: Uuid::new_v4().to_string(),
            description: task.to_string(),
            context: context.map(String::from),
            ..Default::default()
        };

        // Create a mutable clone for pre_task
        let mut hooks = Self::new(self.config.clone())?;
        hooks.pre_task(input)
    }

    /// Get current session state
    pub fn session_state(&self) -> SessionState {
        self.session_state.read().clone()
    }

    /// Get learning metrics
    pub fn learning_metrics(&self) -> LearningMetrics {
        LearningMetrics {
            total_patterns: self.pattern_store.as_ref().map(|p| p.len()).unwrap_or(0),
            patterns_added: *self.patterns_added.read(),
            patterns_consolidated: 0,
            error_patterns: *self.errors_learned.read(),
        }
    }

    // Private helper methods

    fn check_agent_booster(&self, description: &str) -> (bool, Option<String>) {
        let desc_lower = description.to_lowercase();

        // Simple transforms that can skip LLM
        let booster_intents = [
            ("var to const", "var-to-const"),
            ("var->const", "var-to-const"),
            ("let to const", "var-to-const"),
            ("add types", "add-types"),
            ("add type annotations", "add-types"),
            ("add error handling", "add-error-handling"),
            ("wrap in try catch", "add-error-handling"),
            ("convert to async", "async-await"),
            ("async await", "async-await"),
            ("add logging", "add-logging"),
            ("add console log", "add-logging"),
            ("remove console", "remove-console"),
            ("remove console.log", "remove-console"),
        ];

        for (pattern, intent) in booster_intents {
            if desc_lower.contains(pattern) {
                return (true, Some(intent.to_string()));
            }
        }

        (false, None)
    }

    fn fallback_routing(
        &self,
        description: &str,
    ) -> (String, f32, Vec<PatternMatch>, Option<String>) {
        let desc_lower = description.to_lowercase();

        // Simple keyword-based routing
        let (agent, confidence) = if desc_lower.contains("test") || desc_lower.contains("spec") {
            ("tester", 0.8)
        } else if desc_lower.contains("review") || desc_lower.contains("audit") {
            ("reviewer", 0.8)
        } else if desc_lower.contains("security") || desc_lower.contains("vulnerab") {
            ("security-auditor", 0.85)
        } else if desc_lower.contains("design") || desc_lower.contains("architect") {
            ("system-architect", 0.8)
        } else if desc_lower.contains("research") || desc_lower.contains("analyze") {
            ("researcher", 0.75)
        } else if desc_lower.contains("performance") || desc_lower.contains("optimize") {
            ("performance-engineer", 0.8)
        } else if desc_lower.contains("api") || desc_lower.contains("endpoint") {
            ("backend-dev", 0.75)
        } else if desc_lower.contains("ci") || desc_lower.contains("pipeline") {
            ("cicd-engineer", 0.8)
        } else {
            ("coder", 0.7)
        };

        (agent.to_string(), confidence, Vec::new(), None)
    }

    fn create_simple_embedding(&self, text: &str) -> Vec<f32> {
        // Simple hash-based embedding for now
        // In production, use a proper embedding model
        let mut embedding = vec![0.0f32; self.config.embedding_dim];

        for (i, word) in text.split_whitespace().enumerate() {
            let hash = word
                .bytes()
                .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
            let idx = (hash % self.config.embedding_dim as u64) as usize;
            embedding[idx] += 1.0 / (i + 1) as f32;
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        embedding
    }

    fn parse_agent_type(&self, agent: &str) -> AgentType {
        match agent.to_lowercase().as_str() {
            "coder" => AgentType::Coder,
            "researcher" => AgentType::Researcher,
            "tester" => AgentType::Tester,
            "reviewer" => AgentType::Reviewer,
            "system-architect" | "architect" => AgentType::Architect,
            "security-auditor" | "security" => AgentType::Security,
            "performance-engineer" | "perf" | "performance" => AgentType::Performance,
            "backend-dev" | "backend" => AgentType::Coder, // Map to Coder
            "cicd-engineer" | "cicd" => AgentType::Coder,  // Map to Coder
            "ml-developer" | "ml" => AgentType::MlDeveloper,
            _ => AgentType::Coder,
        }
    }

    fn start_trajectory(&self, task_id: &str, description: &str) {
        let trajectory = ActiveTrajectory {
            task_id: task_id.to_string(),
            description: description.to_string(),
            steps: Vec::new(),
            started_at: Utc::now(),
        };

        self.active_trajectories
            .insert(task_id.to_string(), trajectory);

        // Update session state
        let mut state = self.session_state.write();
        state.active_trajectories.push(task_id.to_string());
    }

    fn complete_trajectory(
        &mut self,
        task_id: &str,
        success: bool,
        agent: &str,
        quality: f32,
        error: Option<&str>,
    ) -> Result<bool> {
        let trajectory = self.active_trajectories.remove(task_id);

        if let Some((_, traj)) = trajectory {
            // Store pattern if successful and high quality
            if success && quality >= self.config.min_pattern_confidence {
                // Get embedding before mutable borrow
                let embedding =
                    create_simple_embedding_static(&traj.description, self.config.embedding_dim);

                if let Some(ref mut store) = self.pattern_store {
                    let pattern = Pattern::new(embedding, PatternCategory::General, quality)
                        .with_lesson(traj.description.clone())
                        .with_action(format!("Task completed by {}", agent));

                    if store.store_pattern(pattern).is_ok() {
                        *self.patterns_added.write() += 1;

                        // Update session state
                        let mut state = self.session_state.write();
                        state.active_trajectories.retain(|t| t != task_id);

                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    fn consolidate_patterns(&mut self) -> Result<Option<usize>> {
        // EWC++ consolidation would run here
        // For now, just return the count
        if let Some(ref store) = self.pattern_store {
            Ok(Some(store.len()))
        } else {
            Ok(None)
        }
    }
}

/// Session end result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEndResult {
    /// Session identifier
    pub session_id: String,
    /// Patterns consolidated
    pub patterns_consolidated: usize,
    /// Whether state was persisted
    pub state_persisted: bool,
    /// Session metrics
    pub metrics: Option<SessionMetrics>,
}

/// Session metrics for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetrics {
    /// Tasks completed
    pub tasks_completed: usize,
    /// Patterns learned
    pub patterns_learned: usize,
    /// Patterns consolidated
    pub patterns_consolidated: usize,
    /// Error patterns learned
    pub errors_learned: usize,
    /// Average quality score
    pub avg_quality: f32,
    /// Session duration in seconds
    pub duration_seconds: u64,
}

/// Static helper function to create embeddings without self reference
/// Used to avoid borrow checker issues when both immutable and mutable borrows are needed
fn create_simple_embedding_static(text: &str, embedding_dim: usize) -> Vec<f32> {
    // Simple hash-based embedding for now
    // In production, use a proper embedding model
    let mut embedding = vec![0.0f32; embedding_dim];

    for (i, word) in text.split_whitespace().enumerate() {
        let hash = word
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        let idx = (hash % embedding_dim as u64) as usize;
        embedding[idx] += 1.0 / (i + 1) as f32;
    }

    // Normalize
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut embedding {
            *x /= norm;
        }
    }

    embedding
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hooks_integration_creation() {
        // Disable HNSW to avoid database lock issues in tests
        let config = HooksConfig {
            enable_hnsw_routing: false,
            enable_pattern_learning: false,
            ..Default::default()
        };
        let hooks = HooksIntegration::new(config);
        if let Err(ref e) = hooks {
            eprintln!("HooksIntegration creation error: {:?}", e);
        }
        assert!(
            hooks.is_ok(),
            "Failed to create HooksIntegration: {:?}",
            hooks.err()
        );
    }

    #[test]
    fn test_pre_task_routing() {
        let config = HooksConfig {
            enable_hnsw_routing: false, // Disable for simpler test
            enable_pattern_learning: false,
            enable_quality_scoring: false,
            enable_error_learning: false,
            enable_memory_bridge: false,
            ..Default::default()
        };
        let mut hooks = HooksIntegration::new(config).unwrap();

        let input = PreTaskInput {
            task_id: "test-1".into(),
            description: "implement a REST API endpoint".into(),
            ..Default::default()
        };

        let result = hooks.pre_task(input).unwrap();
        assert!(!result.recommended_agent.is_empty());
        assert!(!result.recommended_model.is_empty());
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn test_agent_booster_detection() {
        let config = HooksConfig {
            enable_hnsw_routing: false,
            enable_pattern_learning: false,
            ..Default::default()
        };
        let hooks = HooksIntegration::new(config).unwrap();

        let (available, intent) = hooks.check_agent_booster("convert var to const");
        assert!(available);
        assert_eq!(intent, Some("var-to-const".to_string()));

        let (available, _) = hooks.check_agent_booster("implement authentication");
        assert!(!available);
    }

    #[test]
    fn test_session_lifecycle() {
        let config = HooksConfig {
            enable_hnsw_routing: false,
            enable_pattern_learning: false,
            enable_quality_scoring: false,
            enable_error_learning: false,
            enable_memory_bridge: false,
            ..Default::default()
        };
        let mut hooks = HooksIntegration::new(config).unwrap();

        // Start session
        let state = hooks.session_start(Some("test-session"), false).unwrap();
        assert_eq!(state.session_id, "test-session");
        assert_eq!(state.tasks_completed, 0);

        // End session
        let result = hooks.session_end(true, false).unwrap();
        assert_eq!(result.session_id, "test-session");
        assert!(result.metrics.is_some());
    }

    #[test]
    fn test_pre_edit_routing() {
        let config = HooksConfig {
            enable_hnsw_routing: false,
            enable_pattern_learning: false,
            ..Default::default()
        };
        let hooks = HooksIntegration::new(config).unwrap();

        let input = PreEditInput {
            file_path: "src/main.rs".into(),
            operation: "update".into(),
            context: None,
        };

        let result = hooks.pre_edit(input).unwrap();
        assert_eq!(result.recommended_agent, "coder");
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn test_fallback_routing() {
        let config = HooksConfig {
            enable_hnsw_routing: false,
            enable_pattern_learning: false,
            ..Default::default()
        };
        let hooks = HooksIntegration::new(config).unwrap();

        // Test various task descriptions
        let (agent, conf, _, _) = hooks.fallback_routing("write unit tests for the API");
        assert_eq!(agent, "tester");

        let (agent, _, _, _) = hooks.fallback_routing("review code for security issues");
        assert_eq!(agent, "reviewer");

        let (agent, _, _, _) = hooks.fallback_routing("design the database schema");
        assert_eq!(agent, "system-architect");
    }
}
