//! Self-Reflection Architecture for RuvLLM
//!
//! This module provides a comprehensive self-reflection system enabling error recovery
//! and self-correction capabilities for LLM-based agents. The architecture supports
//! multiple reflection strategies and learns from past errors to improve future performance.
//!
//! ## Key Components
//!
//! - [`ReflectiveAgent`]: Wrapper that adds reflection capabilities to any base agent
//! - [`ConfidenceChecker`]: Implements If-or-Else (IoE) pattern for targeted revision
//! - [`ErrorPatternLearner`]: Learns recovery strategies from historical errors
//! - [`Perspective`]: Multi-perspective critique system for comprehensive reflection
//!
//! ## Reflection Strategies
//!
//! The module supports four reflection strategies (see [`ReflectionStrategy`]):
//!
//! 1. **Retry**: Simple retry with reflection context on failure
//! 2. **IfOrElse (IoE)**: Confidence-based revision - only revise when confidence is LOW
//! 3. **MultiPerspective**: Critique from multiple angles (correctness, completeness, consistency)
//! 4. **TrajectoryReflection**: Reflect on entire execution trajectory for learning
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +----------------------+     +------------------+
//! | ReflectiveAgent   |---->| ReflectionStrategy   |---->| Reflection       |
//! | - base_agent      |     | - Retry              |     | - context        |
//! | - strategy        |     | - IfOrElse           |     | - insights       |
//! | - error_learner   |     | - MultiPerspective   |     | - suggestions    |
//! +-------------------+     | - TrajectoryReflect  |     +------------------+
//!                           +----------------------+
//!                                    |
//!         +--------------------------|---------------------------+
//!         |                          |                           |
//!         v                          v                           v
//! +----------------+     +---------------------+     +--------------------+
//! | ConfidenceChk  |     | ErrorPatternLearner |     | Perspectives       |
//! | - threshold    |     | - clusters          |     | - Correctness      |
//! | - budget       |     | - strategies        |     | - Completeness     |
//! +----------------+     +---------------------+     | - Consistency      |
//!                                                    +--------------------+
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::reflection::{
//!     ReflectiveAgent, ReflectionStrategy, ConfidenceChecker,
//!     ErrorPatternLearner, CorrectnessChecker, CompletenessChecker,
//! };
//! use ruvllm::claude_flow::AgentType;
//!
//! // Create a reflective agent with multi-perspective strategy
//! let mut agent = ReflectiveAgent::new(
//!     base_agent,
//!     ReflectionStrategy::MultiPerspective {
//!         perspectives: vec![
//!             Box::new(CorrectnessChecker::new()),
//!             Box::new(CompletenessChecker::new()),
//!         ],
//!     },
//! );
//!
//! // Execute with automatic reflection on failure
//! let result = agent.execute_with_reflection("implement a REST API", &context).await?;
//!
//! // The result includes reflection insights if recovery occurred
//! if result.recovered_via_reflection {
//!     println!("Recovered via: {}", result.reflection.unwrap().strategy);
//! }
//! ```
//!
//! ## Integration with ReasoningBank
//!
//! This module integrates with the existing `Verdict` enum by adding a
//! `RecoveredViaReflection` variant to track successful error recovery:
//!
//! ```rust,ignore
//! use ruvllm::claude_flow::Verdict;
//!
//! let verdict = Verdict::RecoveredViaReflection {
//!     original_error: "Type mismatch in function call".to_string(),
//!     recovery_strategy: "Added explicit type annotation".to_string(),
//!     attempts: 2,
//! };
//! ```

mod confidence;
mod error_recovery;
mod perspectives;
mod reflective_agent;

// Re-export all public types
pub use confidence::{
    ConfidenceCheckRecord, ConfidenceChecker, ConfidenceConfig, ConfidenceFactorWeights,
    ConfidenceLevel, RevisionResult, WeakPoint, WeaknessType,
};
pub use error_recovery::{
    ErrorCategory, ErrorCluster, ErrorLearnerStats, ErrorPattern, ErrorPatternLearner,
    ErrorPatternLearnerConfig, RecoveryOutcome, RecoveryStrategy, RecoverySuggestion, SimilarError,
};
pub use perspectives::{
    CompletenessChecker, ConsistencyChecker, CorrectnessChecker, CritiqueIssue, CritiqueResult,
    IssueCategory, Perspective, PerspectiveConfig, UnifiedCritique,
};
pub use reflective_agent::{
    BaseAgent, ExecutionContext, ExecutionResult, PreviousAttempt, Reflection, ReflectionConfig,
    ReflectionStrategy, ReflectiveAgent, ReflectiveAgentStats, RetryConfig,
};
