//! # RuvLLM - LLM Serving Runtime with Ruvector Integration
//!
//! RuvLLM is an edge-focused LLM serving runtime designed for portable, high-performance
//! inference across heterogeneous hardware. It integrates with Ruvector for intelligent
//! memory capabilities, enabling continuous self-improvement through SONA learning.
//!
//! ## Architecture
//!
//! RuvLLM uses Ruvector as a unified memory layer with three distinct roles:
//!
//! - **Policy Memory Store**: Learned thresholds and parameters for runtime decisions
//! - **Session State Index**: Multi-turn conversation state with KV cache references
//! - **Witness Log Index**: Audit logging with semantic search capabilities
//!
//! ## Key Components
//!
//! - [`PagedAttention`]: Memory-efficient attention mechanism with page tables
//! - [`TwoTierKvCache`]: FP16 tail + quantized store for optimal memory/quality tradeoff
//! - [`AdapterManager`]: LoRA adapter loading and hot-swapping
//! - [`SessionManager`]: Session lifecycle and state management
//! - [`PolicyStore`]: Ruvector-backed policy storage with semantic search
//! - [`WitnessLog`]: Audit logging with HNSW-indexed semantic search
//! - [`SonaIntegration`]: Three-tier learning loop integration
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::{RuvLLMConfig, RuvLLMEngine};
//!
//! // Create engine with default configuration
//! let config = RuvLLMConfig::default();
//! let engine = RuvLLMEngine::new(config)?;
//!
//! // Create a session
//! let session = engine.create_session("user-123")?;
//!
//! // Process a request
//! let response = engine.process(&session, "Hello, world!")?;
//! ```

#![allow(missing_docs)]
#![warn(clippy::all)]
// Allow lints that are style/convention rather than correctness
#![allow(clippy::incompatible_msrv)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::excessive_precision)]
#![allow(clippy::vec_init_then_push)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::unnecessary_map_or)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::approx_constant)]
#![allow(clippy::useless_vec)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::len_zero)]
#![allow(clippy::single_char_add_str)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::double_ended_iterator_last)]
#![allow(clippy::manual_clamp)]
#![allow(clippy::len_without_is_empty)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::map_flatten)]
#![allow(clippy::manual_inspect)]
#![allow(clippy::useless_format)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::manual_strip)]
#![allow(clippy::identity_op)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::missing_const_for_thread_local)]
#![allow(clippy::manual_range_patterns)]
#![allow(clippy::question_mark)]
#![allow(clippy::let_and_return)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::manual_map)]
#![allow(clippy::map_entry)]
#![allow(clippy::same_item_push)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::implicit_saturating_sub)]
#![allow(clippy::ref_as_ptr)]
#![allow(clippy::multiple_bound_locations)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_mut)]
#![allow(mismatched_lifetime_syntaxes)]
#![allow(unreachable_code)]
#![allow(unused_assignments)]
#![allow(unused_must_use)]
#![allow(clippy::module_inception)]
#![allow(clippy::items_after_test_module)]
#![allow(clippy::new_without_default)]
#![allow(clippy::inherent_to_string)]
#![allow(clippy::manual_is_ascii_check)]
#![allow(private_interfaces)]
#![allow(unexpected_cfgs)]
#![allow(unused_doc_comments)]
#![allow(clippy::assign_op_pattern)]
#![allow(clippy::cast_slice_from_raw_parts)]
#![allow(clippy::cloned_ref_to_slice_refs)]
#![allow(clippy::double_comparisons)]
#![allow(clippy::for_kv_map)]
#![allow(clippy::manual_pattern_char_comparison)]
#![allow(clippy::mut_from_ref)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::unnecessary_unwrap)]

pub mod adapter_manager;
pub mod autodetect;
pub mod backends;
pub mod bitnet;
pub mod capabilities;
pub mod claude_flow;
pub mod context;
pub mod error;
pub mod evaluation;
pub mod gguf;
pub mod hub;
pub mod intelligence;
pub mod kernels;
pub mod kv_cache;
pub mod lora;
pub mod memory_pool;
#[cfg(all(target_os = "macos", feature = "metal-compute"))]
pub mod metal;
pub mod models;
pub mod optimization;
pub mod paged_attention;
pub mod policy_store;
pub mod quality;
pub mod quantize;
pub mod reasoning_bank;
pub mod reflection;
pub mod ruvector_integration;
pub mod serving;
pub mod session;
pub mod session_index;
pub mod sona;
pub mod speculative;
pub mod tokenizer;
pub mod training;
pub mod types;
pub mod witness_log;

// Test modules
#[cfg(test)]
mod tests;

// Re-exports
pub use adapter_manager::{AdapterConfig, AdapterManager, LoraAdapter};
pub use autodetect::{
    Architecture, ComputeBackend, CoreInfo, CpuFeatures, GpuBackend, GpuCapabilities,
    InferenceConfig, Platform, SystemCapabilities,
};
#[cfg(feature = "candle")]
pub use backends::CandleBackend;
pub use backends::{
    create_backend, DType, DeviceType, GenerateParams, GeneratedToken, LlmBackend,
    ModelArchitecture, ModelConfig, ModelInfo, Quantization, SharedBackend, SpecialTokens,
    StreamEvent, TokenStream, Tokenizer,
};
#[cfg(feature = "async-runtime")]
pub use backends::{AsyncTokenStream, LlmBackendAsync};
pub use claude_flow::{
    AgentContext,
    AgentCoordinator,
    AgentRouter,
    AgentState,
    AgentType,
    AnalyzerStats as ModelAnalyzerStats,
    ClassificationResult,
    ClaudeFlowAgent,
    ClaudeFlowTask,
    // Claude API Integration (NEW)
    ClaudeModel,
    ClaudeRequest,
    ClaudeResponse,
    // Model Router (NEW) - Intelligent routing to Haiku/Sonnet/Opus
    ComplexityFactors,
    ComplexityScore,
    ComplexityWeights,
    ContentBlock,
    ContextManager,
    ContextWindow,
    CoordinatorStats,
    CostEstimator,
    FlowOptimizer,
    HnswDistanceMetric,
    // HNSW semantic router (150x faster pattern search)
    HnswRouter,
    HnswRouterConfig,
    HnswRouterStats,
    HnswRoutingResult,
    HooksConfig,
    // Hooks Integration (NEW v2.3) - Unified Claude Flow hooks interface
    HooksIntegration,
    HybridRouter,
    LatencySample,
    LatencyStats as ClaudeLatencyStats,
    LatencyTracker,
    LearningMetrics,
    Message,
    MessageRole,
    ModelRouter,
    ModelRoutingDecision,
    ModelSelector,
    OptimizationConfig,
    OptimizationResult,
    PatternMatch,
    PostEditInput,
    PostEditResult,
    PostTaskInput,
    PostTaskResult,
    PreEditInput,
    PreEditResult,
    PreTaskInput,
    PreTaskResult,
    QualityAssessment,
    QualityMonitor,
    ResponseStreamer,
    RoutingDecision as AgentRoutingDecision,
    SelectionCriteria,
    SelectorStats,
    SessionEndResult,
    SessionMetrics,
    SessionState as HooksSessionState,
    StepResult,
    StreamEvent as ClaudeStreamEvent,
    StreamStats,
    StreamToken,
    TaskClassifier,
    TaskComplexityAnalyzer,
    TaskPattern,
    TaskType,
    UsageStats,
    WorkflowResult,
    WorkflowStep,
};
pub use error::{Result, RuvLLMError};
pub use gguf::{
    GgufFile,
    GgufHeader,
    // New GGUF loading types
    GgufLoader,
    GgufModelLoader,
    GgufQuantType,
    GgufValue,
    LayerWeights,
    LoadConfig,
    LoadProgress,
    LoadedTensor,
    LoadedWeights,
    ModelConfig as GgufModelConfig,
    ModelInitializer,
    ModelWeights,
    ProgressModelBuilder,
    QuantizedTensor,
    QuantizedWeight,
    StreamingLoader,
    TensorCategory,
    TensorInfo,
    TensorNameMapper,
    WeightTensor,
};
pub use hub::{
    default_cache_dir,
    get_hf_token,
    get_model_info,
    ChecksumVerifier,
    DatasetInfo,
    DownloadConfig,
    DownloadError,
    DownloadProgress,
    Framework,
    HardwareRequirements,
    // Common
    HubError,
    License,
    MetricResult,
    // Model Card
    ModelCard,
    ModelCardBuilder,
    // Download
    ModelDownloader,
    ModelInfo as HubModelInfo,
    ModelMetadata,
    ModelSize,
    // Upload
    ModelUploader,
    MultiProgress,
    // Progress
    ProgressBar,
    ProgressCallback,
    ProgressIndicator,
    ProgressStyle,
    QuantizationLevel,
    // Registry
    RuvLtraRegistry,
    TaskType as HubTaskType,
    UploadConfig,
    UploadError,
    UploadProgress,
};
pub use kv_cache::{
    CacheQuantization, CacheTier, KvCacheConfig, KvCacheStats, PooledKvBlock, PooledKvCache,
    PooledKvCacheStats, TwoTierKvCache,
};
pub use lora::{
    AdaptFeedback, AdapterComposer, AdapterPool, AdapterRegistry, CompositionStrategy,
    EwcRegularizer, LearningRateSchedule, MicroLoRA, MicroLoraConfig, TargetModule, TrainingConfig,
    TrainingPipeline,
};
pub use memory_pool::{
    ArenaStats, BufferPool, BufferPoolStats, BufferSize, InferenceArena, MemoryManager,
    MemoryManagerConfig, MemoryManagerStats, PooledBuffer, ScratchSpace, ScratchSpaceManager,
    ScratchStats, CACHE_LINE_SIZE, DEFAULT_ALIGNMENT,
};
pub use optimization::{
    AdaptationResult, BatchSizeStrategy, ConsolidationStrategy, InferenceMetrics,
    KvCachePressurePolicy, LatencyHistogram, LearningLoopStats, MetricsCollector, MetricsSnapshot,
    MovingAverage, OptimizationDecision, OptimizationTrigger, RealtimeConfig, RealtimeOptimizer,
    SonaLlm, SonaLlmConfig, SpeculativeConfig, TokenBudgetAllocation, TrainingSample,
};
pub use paged_attention::{PageBlock, PageTable, PagedAttention, PagedAttentionConfig};
pub use policy_store::{PolicyEntry, PolicyStore, PolicyType, QuantizationPolicy, RouterPolicy};
pub use quantize::{
    dequantize_for_ane,
    // Memory estimation
    estimate_memory_q4,
    estimate_memory_q5,
    estimate_memory_q8,
    // Quantization functions
    quantize_ruvltra_q4,
    quantize_ruvltra_q5,
    quantize_ruvltra_q8,
    MemoryEstimate,
    // Block types
    Q4KMBlock,
    Q5KMBlock,
    Q8Block,
    QuantConfig,
    // Progress tracking
    QuantProgress,
    QuantStats,
    // Core quantizer
    RuvltraQuantizer,
    TargetFormat,
};
pub use serving::{
    BatchStats,
    // Batch types
    BatchedRequest,
    CompletedRequest,
    // Scheduler
    ContinuousBatchScheduler,
    DecodeTask,
    FinishReason,
    GenerationResult,
    // Request types
    InferenceRequest,
    IterationPlan,
    IterationScheduler,
    KvCacheAllocation,
    // KV cache management
    KvCacheManager,
    KvCacheManagerStats,
    KvCachePoolConfig,
    PreemptionMode,
    PrefillTask,
    Priority,
    PriorityPolicy,
    RequestId,
    RequestQueue,
    RequestState,
    RunningRequest,
    ScheduledBatch,
    SchedulerConfig,
    SchedulerStats,
    // Engine
    ServingEngine,
    ServingEngineConfig,
    ServingMetrics,
    TokenBudget,
    TokenOutput,
};
pub use session::{Session, SessionConfig, SessionManager};
pub use session_index::{KvCacheReference, SessionIndex, SessionState};
pub use sona::{LearningLoop, SonaConfig, SonaIntegration};
pub use speculative::{
    log_softmax, sample_from_probs, softmax, top_k_filter, top_p_filter, AtomicSpeculativeStats,
    SpeculationTree, SpeculativeConfig as SpeculativeDecodingConfig, SpeculativeDecoder,
    SpeculativeStats, TreeNode, VerificationResult,
};
pub use tokenizer::{
    ChatMessage, ChatTemplate, Role, RuvTokenizer, StreamingDecodeBuffer, TokenizerSpecialTokens,
};
pub use training::{
    AugmentationConfig,
    // Claude task dataset
    ClaudeTaskDataset,
    ClaudeTaskExample,
    ComplexityLevel,
    DatasetConfig,
    DatasetGenerator,
    DatasetStats,
    DifficultyLevel,
    DifficultyWeights,
    DomainType,
    EvaluationMetrics,
    GrpoBatch,
    // GRPO optimizer for reinforcement learning
    GrpoConfig,
    GrpoOptimizer,
    GrpoSample,
    GrpoStats,
    GrpoUpdateResult,
    McpToolCategory,
    McpToolDef,
    // MCP tool training
    McpToolTrainer,
    McpTrainingConfig,
    ParamType,
    SampleGroup,
    StepBuilder,
    TaskCategory,
    TaskMetadata,
    // Tool calling dataset
    ToolCallDataset,
    ToolCallExample,
    ToolDatasetConfig,
    ToolDatasetStats,
    ToolParam,
    ToolTrajectory,
    TrainingCheckpoint,
    TrainingResult,
    TrainingStats,
    TrajectoryBuilder,
    TrajectoryMetadata,
    TrajectoryStep,
};
pub use types::*;
pub use witness_log::{
    AsyncWriteConfig, LatencyBreakdown, RoutingDecision, WitnessEntry, WitnessLog, WitnessLogStats,
};

// RuvLTRA model architecture exports
pub use models::{
    AneDispatcher,
    AneOptimization,
    MemoryLayout,
    QuantizationType,
    RuvLtraAttention,
    // Configuration
    RuvLtraConfig,
    RuvLtraDecoderLayer,
    RuvLtraMLP,
    // Model components
    RuvLtraModel,
    // Utilities
    RuvLtraModelInfo,
};

// Ruvector integration exports (unified entry point for all Ruvector capabilities)
pub use capabilities::{
    gate_feature, gate_feature_or, RuvectorCapabilities, ATTENTION_AVAILABLE, GNN_AVAILABLE,
    GRAPH_AVAILABLE, HNSW_AVAILABLE, PARALLEL_AVAILABLE, SIMD_AVAILABLE, SONA_AVAILABLE,
};
pub use ruvector_integration::{
    IndexStats,
    IntegrationConfig,
    IntegrationStats,
    // Intelligence layer
    IntelligenceLayer,
    IntelligenceLayerStats,
    IntelligentRoutingDecision,
    // Main integration
    RuvectorIntegration,
    SearchResultWithMetadata,
    // Unified index
    UnifiedIndex,
    VectorMetadata,
};

// Intelligence provider exports
pub use intelligence::{
    FileSignalProvider, HumanVerdict, IntelligenceLoader, IntelligenceProvider, Outcome,
    ProviderError, ProviderQualityWeights, ProviderResult, QualityFactors, QualitySignal,
};

// Quality scoring exports
pub use quality::{
    CoherenceConfig,
    // Coherence validation
    CoherenceValidator,
    CoherenceViolation,
    CombinedValidator,
    ComparisonResult,
    ContradictionResult,
    DiversificationSuggestion,
    // Diversity analysis
    DiversityAnalyzer,
    DiversityConfig,
    DiversityResult,
    FormatValidator,
    ImprovementRecommendation,
    JsonSchemaValidator,
    LogicalFlowResult,
    ModeCollapseResult,
    QualityDimension,
    QualityHistory,
    // Core metrics
    QualityMetrics,
    // Scoring engine
    QualityScoringEngine,
    QualitySummary,
    QualityWeights,
    RangeValidator,
    // Schema validators
    SchemaValidator,
    ScoringConfig,
    ScoringContext,
    SemanticConsistencyResult,
    TrendAnalysis,
    TrendDirection,
    TypeValidator,
    ValidationCombinator,
    ValidationError,
    ValidationResult,
};

// Context management exports (intelligent pruning and semantic memory)
pub use context::{
    // Agentic memory
    AgenticMemory,
    AgenticMemoryConfig,
    AttentionWeights,
    CacheStats,
    CachedToolResult,
    ClaudeFlowBridgeConfig,
    // Claude Flow bridge
    ClaudeFlowMemoryBridge,
    CompressedEpisode,
    ContextElement,
    ContextManagerConfig,
    ElementPriority,
    Episode,
    EpisodeMetadata,
    EpisodeTrajectory,
    // Episodic memory
    EpisodicMemory,
    EpisodicMemoryConfig,
    // Context manager
    IntelligentContextManager,
    MemoryType,
    PreparedContext,
    PriorityScorer,
    ScratchpadEntry,
    SemanticCacheConfig,
    // Semantic cache
    SemanticToolCache,
    SyncResult,
    TaskContext,
    // Working memory
    WorkingMemory,
    WorkingMemoryConfig,
};

// Self-Reflection architecture exports (error recovery and self-correction)
pub use reflection::{
    BaseAgent,
    CompletenessChecker,
    ConfidenceCheckRecord,
    // Confidence-based revision (IoE pattern)
    ConfidenceChecker,
    ConfidenceConfig,
    ConfidenceFactorWeights,
    ConfidenceLevel,
    ConsistencyChecker,
    CorrectnessChecker,
    CritiqueIssue,
    CritiqueResult,
    ErrorCategory,
    ErrorCluster,
    ErrorLearnerStats,
    ErrorPattern,
    // Error pattern learning
    ErrorPatternLearner,
    ErrorPatternLearnerConfig,
    ExecutionContext,
    ExecutionResult,
    IssueCategory,
    // Multi-perspective critique
    Perspective,
    PerspectiveConfig,
    PreviousAttempt,
    RecoveryOutcome,
    RecoveryStrategy,
    RecoverySuggestion,
    Reflection,
    ReflectionConfig,
    ReflectionStrategy,
    // Reflective agent wrapper
    ReflectiveAgent,
    ReflectiveAgentStats,
    RetryConfig,
    RevisionResult,
    SimilarError,
    UnifiedCritique,
    WeakPoint,
    WeaknessType,
};

// ReasoningBank exports (learning from Claude trajectories)
pub use reasoning_bank::{
    CompressedTrajectory,
    ConsolidationConfig,
    DistillationConfig,
    FailurePattern as VerdictFailurePattern,
    FisherInformation,
    ImportanceScore,
    KeyLesson,
    // Memory distillation
    MemoryDistiller,
    Pattern,
    PatternCategory,
    // EWC++ consolidation
    PatternConsolidator,
    PatternSearchResult,
    PatternStats,
    // Pattern storage with HNSW
    PatternStore,
    PatternStoreConfig,
    // Main ReasoningBank
    ReasoningBank,
    ReasoningBankConfig,
    ReasoningBankStats,
    RecoveryStrategy as VerdictRecoveryStrategy,
    RootCause,
    StepOutcome,
    // Trajectory recording (aliased to avoid conflict with training::TrajectoryStep)
    Trajectory as ReasoningTrajectory,
    TrajectoryId,
    TrajectoryRecorder,
    TrajectoryStep as ReasoningTrajectoryStep,
    // Verdict system (aliased to avoid conflict with claude_flow::reasoning_bank::Verdict)
    Verdict as ReasoningVerdict,
    VerdictAnalyzer,
};

// Metal GPU acceleration exports (macOS only)
#[cfg(all(target_os = "macos", feature = "metal-compute"))]
pub use metal::{
    get_device_info, is_metal_available, shader_source, tile_sizes, AttentionParams, GemmParams,
    MetalBuffer, MetalBufferPool, MetalConfig, MetalContext, MetalDeviceInfo, MetalPipelines,
    NormParams, RopeParams,
};

/// RuvLLM engine configuration.
///
/// This configuration struct controls all aspects of the RuvLLM engine,
/// including storage paths, attention mechanisms, KV cache settings,
/// session management, and SONA learning parameters.
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::{RuvLLMConfig, PagedAttentionConfig, KvCacheConfig};
///
/// let config = RuvLLMConfig {
///     storage_path: "/var/ruvllm".to_string(),
///     max_sessions: 500,
///     embedding_dim: 1024,
///     ..Default::default()
/// };
/// ```
///
/// # Performance Tuning
///
/// | Parameter | Default | High Throughput | Low Latency |
/// |-----------|---------|-----------------|-------------|
/// | `max_sessions` | 1000 | 2000 | 500 |
/// | `embedding_dim` | 768 | 1024 | 512 |
#[derive(Debug, Clone)]
pub struct RuvLLMConfig {
    /// Path to Ruvector storage
    pub storage_path: String,
    /// Paged attention configuration
    pub paged_attention: PagedAttentionConfig,
    /// KV cache configuration
    pub kv_cache: KvCacheConfig,
    /// Session configuration
    pub session: SessionConfig,
    /// SONA learning configuration
    pub sona: SonaConfig,
    /// Maximum concurrent sessions
    pub max_sessions: usize,
    /// Embedding dimension for semantic search
    pub embedding_dim: usize,
}

impl Default for RuvLLMConfig {
    fn default() -> Self {
        Self {
            storage_path: ".ruvllm".to_string(),
            paged_attention: PagedAttentionConfig::default(),
            kv_cache: KvCacheConfig::default(),
            session: SessionConfig::default(),
            sona: SonaConfig::default(),
            max_sessions: 1000,
            embedding_dim: 768,
        }
    }
}

/// Main RuvLLM engine for LLM inference with intelligent memory.
///
/// The `RuvLLMEngine` is the primary entry point for RuvLLM, providing:
///
/// - **Session Management**: Create and manage user sessions with state persistence
/// - **Policy Storage**: Ruvector-backed semantic search for runtime policies
/// - **Adapter Management**: Hot-swapping LoRA adapters for task-specific tuning
/// - **Witness Logging**: Audit trail with HNSW-indexed semantic search
/// - **SONA Learning**: Three-tier continuous learning integration
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::{RuvLLMEngine, RuvLLMConfig};
///
/// // Create engine with configuration
/// let config = RuvLLMConfig::default();
/// let engine = RuvLLMEngine::new(config)?;
///
/// // Create a session for a user
/// let session = engine.create_session(Some("user-123"))?;
///
/// // Search for relevant policies
/// let embedding = compute_embedding("code completion task");
/// let policies = engine.search_policies(&embedding, 5)?;
///
/// // Record audit entry
/// let entry = WitnessEntry::new("completion", latency, routing);
/// engine.record_witness(entry)?;
/// ```
///
/// # Architecture
///
/// ```text
/// +-------------------+     +-------------------+
/// | RuvLLMEngine      |---->| PolicyStore       |
/// |                   |     | (Ruvector)        |
/// |                   |     +-------------------+
/// |                   |
/// |                   |---->| SessionIndex      |
/// |                   |     | (Ruvector)        |
/// |                   |     +-------------------+
/// |                   |
/// |                   |---->| WitnessLog        |
/// |                   |     | (HNSW search)     |
/// +-------------------+     +-------------------+
/// ```
pub struct RuvLLMEngine {
    /// Configuration
    config: RuvLLMConfig,
    /// Policy store backed by Ruvector
    policy_store: PolicyStore,
    /// Session manager
    session_manager: SessionManager,
    /// Session index backed by Ruvector
    session_index: SessionIndex,
    /// Adapter manager
    adapter_manager: AdapterManager,
    /// Witness log for audit
    witness_log: WitnessLog,
    /// SONA learning integration
    sona: SonaIntegration,
}

impl RuvLLMEngine {
    /// Create a new RuvLLM engine with the given configuration.
    ///
    /// This initializes all subsystems including:
    /// - Policy store for learned thresholds
    /// - Session index for conversation state
    /// - Witness log for audit trails
    /// - SONA integration for learning loops
    ///
    /// # Arguments
    ///
    /// * `config` - Engine configuration
    ///
    /// # Errors
    ///
    /// Returns an error if storage paths cannot be created or initialized.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ruvllm::{RuvLLMEngine, RuvLLMConfig};
    ///
    /// let engine = RuvLLMEngine::new(RuvLLMConfig::default())?;
    /// ```
    pub fn new(config: RuvLLMConfig) -> Result<Self> {
        let storage_path = &config.storage_path;

        let policy_store =
            PolicyStore::new(&format!("{}/policies", storage_path), config.embedding_dim)?;

        let session_index =
            SessionIndex::new(&format!("{}/sessions", storage_path), config.embedding_dim)?;

        let witness_log =
            WitnessLog::new(&format!("{}/witness", storage_path), config.embedding_dim)?;

        let session_manager = SessionManager::new(config.session.clone());
        let adapter_manager = AdapterManager::new();
        let sona = SonaIntegration::new(config.sona.clone());

        Ok(Self {
            config,
            policy_store,
            session_manager,
            session_index,
            adapter_manager,
            witness_log,
            sona,
        })
    }

    /// Create a new session for a user.
    ///
    /// Sessions track conversation state, KV cache references, and enable
    /// multi-turn interactions. Each session is automatically indexed in
    /// Ruvector for semantic retrieval.
    ///
    /// # Arguments
    ///
    /// * `user_id` - Optional user identifier for session tracking
    ///
    /// # Returns
    ///
    /// A new `Session` instance with a unique ID.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Anonymous session
    /// let session = engine.create_session(None)?;
    ///
    /// // User-identified session
    /// let session = engine.create_session(Some("user-123"))?;
    /// println!("Session ID: {}", session.id());
    /// ```
    pub fn create_session(&self, user_id: Option<&str>) -> Result<Session> {
        let session = self.session_manager.create_session(user_id)?;

        // Index the session in Ruvector
        let state = SessionState::from_session(&session);
        self.session_index.store(&state)?;

        Ok(session)
    }

    /// Get session by ID
    pub fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        self.session_manager.get_session(session_id)
    }

    /// Search for policies matching the given context embedding.
    ///
    /// Uses HNSW-indexed semantic search to find relevant policies
    /// (quantization settings, routing rules, etc.) based on the
    /// current request context.
    ///
    /// # Arguments
    ///
    /// * `context_embedding` - Vector embedding of the current context
    /// * `limit` - Maximum number of policies to return
    ///
    /// # Returns
    ///
    /// Vector of matching `PolicyEntry` items, sorted by relevance.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let context = compute_embedding("code completion for Python");
    /// let policies = engine.search_policies(&context, 5)?;
    ///
    /// for policy in policies {
    ///     println!("Policy: {:?}, score: {}", policy.policy_type, policy.score);
    /// }
    /// ```
    pub fn search_policies(
        &self,
        context_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<PolicyEntry>> {
        self.policy_store.search(context_embedding, limit)
    }

    /// Record a witness entry for audit logging.
    ///
    /// Witness entries provide an audit trail of inference decisions,
    /// including latency breakdowns, routing decisions, and quality scores.
    /// All entries are HNSW-indexed for semantic search.
    ///
    /// # Arguments
    ///
    /// * `entry` - The witness entry to record
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ruvllm::{WitnessEntry, LatencyBreakdown, RoutingDecision};
    ///
    /// let entry = WitnessEntry {
    ///     session_id: session.id().to_string(),
    ///     request_type: "completion".to_string(),
    ///     latency: LatencyBreakdown {
    ///         prefill_ms: 45.0,
    ///         decode_ms: 120.0,
    ///         total_ms: 165.0,
    ///     },
    ///     routing: RoutingDecision::default(),
    ///     ..Default::default()
    /// };
    ///
    /// engine.record_witness(entry)?;
    /// ```
    pub fn record_witness(&self, entry: WitnessEntry) -> Result<()> {
        self.witness_log.record(entry)
    }

    /// Search witness logs semantically
    pub fn search_witness(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<WitnessEntry>> {
        self.witness_log.search(query_embedding, limit)
    }

    /// Get the SONA integration for learning
    pub fn sona(&self) -> &SonaIntegration {
        &self.sona
    }

    /// Get the adapter manager
    pub fn adapters(&self) -> &AdapterManager {
        &self.adapter_manager
    }

    /// Get the policy store
    pub fn policies(&self) -> &PolicyStore {
        &self.policy_store
    }
}
