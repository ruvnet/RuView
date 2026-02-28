//! # Action Trait: External Side Effects with Governance
//!
//! Defines the Action trait for operations that produce external side effects.
//! All actions are subject to coherence gating and produce mandatory witness records.
//!
//! ## Design Philosophy
//!
//! Actions are the boundary between the coherence engine and the external world.
//! Every action must:
//!
//! 1. Declare its scope (what coherence region it affects)
//! 2. Estimate its impact (resource cost, reversibility)
//! 3. Be executable with a witness record
//! 4. Support rollback when possible
//!
//! ## Example
//!
//! ```ignore
//! struct UpdateUserRecord {
//!     user_id: UserId,
//!     new_data: UserData,
//! }
//!
//! impl Action for UpdateUserRecord {
//!     type Output = ();
//!     type Error = DatabaseError;
//!
//!     fn scope(&self) -> &ScopeId {
//!         &self.user_id.scope
//!     }
//!
//!     fn impact(&self) -> ActionImpact {
//!         ActionImpact::medium()
//!     }
//!
//!     fn execute(&self, ctx: &ExecutionContext) -> Result<Self::Output, Self::Error> {
//!         // Execute the action
//!     }
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for an action instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActionId(pub uuid::Uuid);

impl ActionId {
    /// Generate a new random action ID.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Create from an existing UUID.
    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID.
    pub fn as_uuid(&self) -> &uuid::Uuid {
        &self.0
    }

    /// Convert to bytes for hashing.
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for ActionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "action-{}", self.0)
    }
}

/// Scope identifier for coherence energy scoping.
///
/// Actions affect specific regions of the coherence graph. The scope
/// determines which subgraph's energy is relevant for gating.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScopeId(pub String);

impl ScopeId {
    /// Create a new scope ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Global scope (affects entire system).
    pub fn global() -> Self {
        Self::new("__global__")
    }

    /// Create a scoped path (e.g., "users.123.profile").
    pub fn path(parts: &[&str]) -> Self {
        Self::new(parts.join("."))
    }

    /// Check if this is the global scope.
    pub fn is_global(&self) -> bool {
        self.0 == "__global__"
    }

    /// Get the scope as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Check if this scope is a parent of another.
    pub fn is_parent_of(&self, other: &ScopeId) -> bool {
        if self.is_global() {
            return true;
        }
        other.0.starts_with(&self.0) && other.0.len() > self.0.len()
    }
}

impl fmt::Display for ScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ScopeId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ScopeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Impact assessment for an action.
///
/// Used by the coherence gate to make risk-aware decisions about
/// whether to allow, delay, or deny actions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ActionImpact {
    /// Resource cost estimate (0.0 = free, 1.0 = maximum).
    pub cost: f32,

    /// Reversibility (0.0 = irreversible, 1.0 = fully reversible).
    pub reversibility: f32,

    /// Blast radius (0.0 = isolated, 1.0 = system-wide).
    pub blast_radius: f32,

    /// Latency sensitivity (0.0 = can wait, 1.0 = time-critical).
    pub latency_sensitivity: f32,
}

impl ActionImpact {
    /// Create a new impact assessment.
    pub const fn new(
        cost: f32,
        reversibility: f32,
        blast_radius: f32,
        latency_sensitivity: f32,
    ) -> Self {
        Self {
            cost,
            reversibility,
            blast_radius,
            latency_sensitivity,
        }
    }

    /// Minimal impact (cheap, reversible, isolated).
    pub const fn minimal() -> Self {
        Self::new(0.1, 0.9, 0.1, 0.5)
    }

    /// Low impact action.
    pub const fn low() -> Self {
        Self::new(0.2, 0.8, 0.2, 0.5)
    }

    /// Medium impact action.
    pub const fn medium() -> Self {
        Self::new(0.5, 0.5, 0.5, 0.5)
    }

    /// High impact action.
    pub const fn high() -> Self {
        Self::new(0.8, 0.3, 0.7, 0.7)
    }

    /// Critical impact (expensive, irreversible, wide blast radius).
    pub const fn critical() -> Self {
        Self::new(0.95, 0.1, 0.9, 0.9)
    }

    /// Calculate overall risk score (0.0 to 1.0).
    ///
    /// Higher risk = more likely to require escalation.
    pub fn risk_score(&self) -> f32 {
        // Weighted combination favoring irreversibility and blast radius
        let weights = [0.2, 0.35, 0.3, 0.15]; // cost, reversibility(inverted), blast_radius, latency

        let scores = [
            self.cost,
            1.0 - self.reversibility, // Invert: low reversibility = high risk
            self.blast_radius,
            self.latency_sensitivity,
        ];

        scores.iter().zip(weights.iter()).map(|(s, w)| s * w).sum()
    }

    /// Whether this action should be considered high-risk.
    pub fn is_high_risk(&self) -> bool {
        self.risk_score() > 0.6
    }

    /// Whether this action is reversible enough for automatic retry.
    pub fn allows_retry(&self) -> bool {
        self.reversibility > 0.5
    }
}

impl Default for ActionImpact {
    fn default() -> Self {
        Self::medium()
    }
}

/// Action metadata for governance and audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionMetadata {
    /// Unique action identifier.
    pub id: ActionId,

    /// Action type name.
    pub action_type: String,

    /// Human-readable description.
    pub description: String,

    /// Actor who initiated the action.
    pub actor_id: String,

    /// Timestamp when action was created (Unix millis).
    pub created_at_ms: u64,

    /// Optional tags for categorization.
    pub tags: Vec<String>,

    /// Optional correlation ID for tracing.
    pub correlation_id: Option<String>,
}

impl ActionMetadata {
    /// Create new metadata with required fields.
    pub fn new(
        action_type: impl Into<String>,
        description: impl Into<String>,
        actor_id: impl Into<String>,
    ) -> Self {
        Self {
            id: ActionId::new(),
            action_type: action_type.into(),
            description: description.into(),
            actor_id: actor_id.into(),
            created_at_ms: Self::current_timestamp_ms(),
            tags: Vec::new(),
            correlation_id: None,
        }
    }

    /// Add a tag to the metadata.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add multiple tags.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Set correlation ID.
    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    fn current_timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// Execution context provided to actions during execution.
///
/// Contains references to system resources and the witness record being built.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// The action's unique ID.
    pub action_id: ActionId,

    /// Current coherence energy for the action's scope.
    pub current_energy: f32,

    /// The compute lane assigned for this execution.
    pub assigned_lane: super::ladder::ComputeLane,

    /// Whether this is a retry attempt.
    pub is_retry: bool,

    /// Retry attempt number (0 for first attempt).
    pub retry_count: u32,

    /// Maximum allowed execution time in milliseconds.
    pub timeout_ms: u64,
}

impl ExecutionContext {
    /// Create a new execution context.
    pub fn new(
        action_id: ActionId,
        current_energy: f32,
        assigned_lane: super::ladder::ComputeLane,
    ) -> Self {
        Self {
            action_id,
            current_energy,
            assigned_lane,
            is_retry: false,
            retry_count: 0,
            timeout_ms: assigned_lane.latency_budget_us() / 1000,
        }
    }

    /// Create a retry context from an existing context.
    pub fn retry(previous: &Self) -> Self {
        Self {
            action_id: previous.action_id.clone(),
            current_energy: previous.current_energy,
            assigned_lane: previous.assigned_lane,
            is_retry: true,
            retry_count: previous.retry_count + 1,
            timeout_ms: previous.timeout_ms,
        }
    }

    /// Check if we've exceeded the retry limit.
    pub fn exceeded_retries(&self, max_retries: u32) -> bool {
        self.retry_count >= max_retries
    }
}

/// The core Action trait for all external side effects.
///
/// Actions are the fundamental unit of work in the coherence engine.
/// They represent operations that modify external state and must be
/// governed by coherence gating.
pub trait Action: Send + Sync {
    /// The successful output type of the action.
    type Output: Send;

    /// The error type that can occur during execution.
    type Error: std::error::Error + Send + 'static;

    /// Get the scope this action affects.
    ///
    /// The scope determines which region of the coherence graph
    /// is consulted for gating decisions.
    fn scope(&self) -> &ScopeId;

    /// Assess the impact of this action.
    ///
    /// Used for risk-based gating decisions.
    fn impact(&self) -> ActionImpact;

    /// Get metadata for this action.
    fn metadata(&self) -> &ActionMetadata;

    /// Execute the action within the given context.
    ///
    /// This method performs the actual side effect. It should:
    /// - Check the context for retry status
    /// - Respect the timeout
    /// - Return a meaningful error on failure
    fn execute(&self, ctx: &ExecutionContext) -> Result<Self::Output, Self::Error>;

    /// Compute a content hash for witness records.
    ///
    /// This should include all relevant action parameters.
    fn content_hash(&self) -> [u8; 32];

    /// Whether this action supports rollback.
    fn supports_rollback(&self) -> bool {
        false
    }

    /// Attempt to rollback this action.
    ///
    /// Only called if `supports_rollback()` returns true.
    fn rollback(&self, _ctx: &ExecutionContext, _output: &Self::Output) -> Result<(), Self::Error> {
        Err(Self::make_rollback_not_supported_error())
    }

    /// Create an error indicating rollback is not supported.
    ///
    /// Implementations should override this to return an appropriate error type.
    fn make_rollback_not_supported_error() -> Self::Error;
}

/// A boxed action that erases the output/error types.
///
/// Useful for storing heterogeneous actions in queues.
pub type BoxedAction = Box<dyn Action<Output = (), Error = ActionError> + Send + Sync>;

/// Generic action error for boxed actions.
#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    #[error("Action execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Action timed out after {0}ms")]
    Timeout(u64),

    #[error("Action was denied by coherence gate: {0}")]
    Denied(String),

    #[error("Rollback not supported")]
    RollbackNotSupported,

    #[error("Rollback failed: {0}")]
    RollbackFailed(String),

    #[error("Invalid action state: {0}")]
    InvalidState(String),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Result of an action execution attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    /// The action ID.
    pub action_id: ActionId,

    /// Whether execution succeeded.
    pub success: bool,

    /// Error message if failed.
    pub error_message: Option<String>,

    /// Execution duration in microseconds.
    pub duration_us: u64,

    /// The compute lane used.
    pub lane: super::ladder::ComputeLane,

    /// Retry count.
    pub retry_count: u32,

    /// Timestamp of completion (Unix millis).
    pub completed_at_ms: u64,
}

impl ActionResult {
    /// Create a successful result.
    pub fn success(
        action_id: ActionId,
        duration_us: u64,
        lane: super::ladder::ComputeLane,
        retry_count: u32,
    ) -> Self {
        Self {
            action_id,
            success: true,
            error_message: None,
            duration_us,
            lane,
            retry_count,
            completed_at_ms: Self::current_timestamp_ms(),
        }
    }

    /// Create a failure result.
    pub fn failure(
        action_id: ActionId,
        error: impl fmt::Display,
        duration_us: u64,
        lane: super::ladder::ComputeLane,
        retry_count: u32,
    ) -> Self {
        Self {
            action_id,
            success: false,
            error_message: Some(error.to_string()),
            duration_us,
            lane,
            retry_count,
            completed_at_ms: Self::current_timestamp_ms(),
        }
    }

    fn current_timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_id() {
        let id1 = ActionId::new();
        let id2 = ActionId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_scope_id() {
        let global = ScopeId::global();
        assert!(global.is_global());

        let user_scope = ScopeId::path(&["users", "123"]);
        assert!(!user_scope.is_global());
        assert_eq!(user_scope.as_str(), "users.123");

        let parent = ScopeId::new("users");
        assert!(parent.is_parent_of(&user_scope));
        assert!(global.is_parent_of(&user_scope));
    }

    #[test]
    fn test_action_impact() {
        let minimal = ActionImpact::minimal();
        let critical = ActionImpact::critical();

        assert!(minimal.risk_score() < critical.risk_score());
        assert!(!minimal.is_high_risk());
        assert!(critical.is_high_risk());
        assert!(minimal.allows_retry());
        assert!(!critical.allows_retry());
    }

    #[test]
    fn test_execution_context_retry() {
        let ctx = ExecutionContext::new(
            ActionId::new(),
            0.5,
            super::super::ladder::ComputeLane::Reflex,
        );

        assert!(!ctx.is_retry);
        assert_eq!(ctx.retry_count, 0);

        let retry_ctx = ExecutionContext::retry(&ctx);
        assert!(retry_ctx.is_retry);
        assert_eq!(retry_ctx.retry_count, 1);
    }

    #[test]
    fn test_action_result() {
        let action_id = ActionId::new();

        let success = ActionResult::success(
            action_id.clone(),
            500,
            super::super::ladder::ComputeLane::Reflex,
            0,
        );
        assert!(success.success);
        assert!(success.error_message.is_none());

        let failure = ActionResult::failure(
            action_id,
            "Something went wrong",
            1000,
            super::super::ladder::ComputeLane::Retrieval,
            1,
        );
        assert!(!failure.success);
        assert!(failure.error_message.is_some());
    }
}
