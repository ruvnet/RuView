//! # Action Executor: Mandatory Witness Creation
//!
//! The executor is responsible for running actions through the coherence gate
//! and ensuring every execution produces a witness record.
//!
//! ## Design Principle
//!
//! > All decisions and external side effects produce mandatory witness and
//! > lineage records, making every action auditable and replayable.
//!
//! ## Execution Flow
//!
//! ```text
//! Action Submitted
//!       │
//!       ▼
//! ┌─────────────────┐
//! │ Gate Evaluation │ → Witness Created (MANDATORY)
//! └─────────────────┘
//!       │
//!       ├── Denied ──────────────────────┐
//!       │                                 ▼
//!       │                        Return Denial + Witness
//!       │
//!       ├── Human Lane ──────────────────┐
//!       │                                 ▼
//!       │                        Queue for Human Review
//!       │
//!       └── Allowed ─────────────────────┐
//!                                         ▼
//!                                  ┌─────────────────┐
//!                                  │ Execute Action  │
//!                                  └─────────────────┘
//!                                         │
//!                                         ├── Success ──┐
//!                                         │              ▼
//!                                         │      Return Success + Witness
//!                                         │
//!                                         └── Failure ──┐
//!                                                        ▼
//!                                                  Retry or Return Error
//! ```

use super::action::{Action, ActionError, ActionId, ActionResult, ExecutionContext};
use super::gate::{CoherenceGate, EnergySnapshot, GateDecision, WitnessRecord};
use super::ladder::ComputeLane;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Configuration for the action executor.
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Maximum retry attempts for failed actions.
    pub max_retries: u32,

    /// Base delay between retries (with exponential backoff).
    pub retry_delay: Duration,

    /// Maximum delay between retries.
    pub max_retry_delay: Duration,

    /// Maximum pending human review queue size.
    pub max_human_queue: usize,

    /// Whether to store all witnesses (vs. only failures/escalations).
    pub store_all_witnesses: bool,

    /// Maximum witnesses to keep in memory.
    pub max_witnesses_in_memory: usize,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay: Duration::from_millis(100),
            max_retry_delay: Duration::from_secs(5),
            max_human_queue: 1000,
            store_all_witnesses: true,
            max_witnesses_in_memory: 10000,
        }
    }
}

/// Statistics about executor operation.
#[derive(Debug, Clone, Default)]
pub struct ExecutorStats {
    /// Total actions submitted.
    pub total_submitted: u64,

    /// Actions allowed through gate.
    pub total_allowed: u64,

    /// Actions denied by gate.
    pub total_denied: u64,

    /// Actions escalated.
    pub total_escalated: u64,

    /// Actions executed successfully.
    pub total_success: u64,

    /// Actions that failed execution.
    pub total_failed: u64,

    /// Actions in human review queue.
    pub pending_human_review: usize,

    /// Total witnesses created.
    pub witnesses_created: u64,

    /// Actions by lane count.
    pub by_lane: [u64; 4],
}

impl ExecutorStats {
    /// Get the success rate (0.0 to 1.0).
    pub fn success_rate(&self) -> f64 {
        if self.total_allowed == 0 {
            return 1.0;
        }
        self.total_success as f64 / self.total_allowed as f64
    }

    /// Get the denial rate (0.0 to 1.0).
    pub fn denial_rate(&self) -> f64 {
        if self.total_submitted == 0 {
            return 0.0;
        }
        self.total_denied as f64 / self.total_submitted as f64
    }

    /// Get the escalation rate (0.0 to 1.0).
    pub fn escalation_rate(&self) -> f64 {
        if self.total_submitted == 0 {
            return 0.0;
        }
        self.total_escalated as f64 / self.total_submitted as f64
    }
}

/// Item in the human review queue.
#[derive(Debug)]
pub struct HumanReviewItem {
    /// The action ID awaiting review.
    pub action_id: ActionId,

    /// The witness record for the gate decision.
    pub witness: WitnessRecord,

    /// When this was queued.
    pub queued_at: Instant,

    /// Energy snapshot at queue time.
    pub energy_snapshot: EnergySnapshot,
}

/// Result of an execution attempt.
#[derive(Debug)]
pub struct ExecutionResult<T> {
    /// The action result.
    pub result: Result<T, ActionError>,

    /// The witness record (ALWAYS present).
    pub witness: WitnessRecord,

    /// The gate decision.
    pub decision: GateDecision,

    /// Execution statistics.
    pub stats: ExecutionStats,
}

/// Statistics for a single execution.
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    /// Time spent in gate evaluation.
    pub gate_time_us: u64,

    /// Time spent in actual execution.
    pub execution_time_us: u64,

    /// Total time including overhead.
    pub total_time_us: u64,

    /// Number of retry attempts.
    pub retry_count: u32,

    /// The lane used for execution.
    pub lane: ComputeLane,
}

/// The action executor with mandatory witness creation.
///
/// This is the primary interface for executing actions in the coherence engine.
/// Every execution attempt produces a witness record, regardless of success or failure.
pub struct ActionExecutor {
    /// The coherence gate for decision making.
    gate: Arc<RwLock<CoherenceGate>>,

    /// Configuration.
    config: ExecutorConfig,

    /// Statistics (thread-safe).
    stats: Arc<RwLock<ExecutorStats>>,

    /// Witness storage (in-memory ring buffer).
    witnesses: Arc<RwLock<VecDeque<WitnessRecord>>>,

    /// Human review queue.
    human_queue: Arc<RwLock<VecDeque<HumanReviewItem>>>,
}

impl ActionExecutor {
    /// Create a new action executor.
    pub fn new(gate: CoherenceGate, config: ExecutorConfig) -> Self {
        Self {
            gate: Arc::new(RwLock::new(gate)),
            config,
            stats: Arc::new(RwLock::new(ExecutorStats::default())),
            witnesses: Arc::new(RwLock::new(VecDeque::new())),
            human_queue: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// Create with default configuration.
    pub fn with_defaults(gate: CoherenceGate) -> Self {
        Self::new(gate, ExecutorConfig::default())
    }

    /// Execute an action with mandatory witness creation.
    ///
    /// This is the main entry point for action execution. It:
    /// 1. Evaluates the action through the coherence gate
    /// 2. Creates a witness record (MANDATORY)
    /// 3. Executes the action if allowed
    /// 4. Returns both the result and the witness
    pub fn execute<A: Action>(
        &self,
        action: &A,
        energy: &EnergySnapshot,
    ) -> ExecutionResult<A::Output> {
        let start_time = Instant::now();

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.total_submitted += 1;
        }

        // Gate evaluation with witness creation
        let gate_start = Instant::now();
        let (decision, witness) = {
            let mut gate = self.gate.write();
            gate.evaluate_with_witness(action, energy)
        };
        let gate_time_us = gate_start.elapsed().as_micros() as u64;

        // Extract lane before any potential moves
        let lane = decision.lane;
        let is_escalated = decision.is_escalated();
        let allow = decision.allow;

        // Store witness
        self.store_witness(&witness);

        // Update lane stats
        {
            let mut stats = self.stats.write();
            stats.witnesses_created += 1;
            stats.by_lane[lane.as_u8() as usize] += 1;

            if is_escalated {
                stats.total_escalated += 1;
            }
        }

        // Handle decision
        if !allow {
            debug!(
                action_id = %action.metadata().id,
                lane = ?lane,
                reason = decision.reason.as_deref().unwrap_or("unknown"),
                "Action denied by coherence gate"
            );

            let mut stats = self.stats.write();
            stats.total_denied += 1;

            let reason = decision
                .reason
                .clone()
                .unwrap_or_else(|| "Gate denied".to_string());

            return ExecutionResult {
                result: Err(ActionError::Denied(reason)),
                witness,
                decision,
                stats: ExecutionStats {
                    gate_time_us,
                    execution_time_us: 0,
                    total_time_us: start_time.elapsed().as_micros() as u64,
                    retry_count: 0,
                    lane,
                },
            };
        }

        // Handle human review lane
        if lane == ComputeLane::Human {
            info!(
                action_id = %action.metadata().id,
                "Action queued for human review"
            );

            self.queue_for_human_review(
                action.metadata().id.clone(),
                witness.clone(),
                energy.clone(),
            );

            let mut stats = self.stats.write();
            stats.total_allowed += 1;

            return ExecutionResult {
                result: Err(ActionError::Denied("Queued for human review".to_string())),
                witness,
                decision,
                stats: ExecutionStats {
                    gate_time_us,
                    execution_time_us: 0,
                    total_time_us: start_time.elapsed().as_micros() as u64,
                    retry_count: 0,
                    lane,
                },
            };
        }

        // Execute with retries
        let mut stats = self.stats.write();
        stats.total_allowed += 1;
        drop(stats);

        let execution_start = Instant::now();
        let (result, retry_count) = self.execute_with_retries(action, &decision, energy);
        let execution_time_us = execution_start.elapsed().as_micros() as u64;

        // Update success/failure stats
        {
            let mut stats = self.stats.write();
            if result.is_ok() {
                stats.total_success += 1;
            } else {
                stats.total_failed += 1;
            }
        }

        ExecutionResult {
            result,
            witness,
            decision,
            stats: ExecutionStats {
                gate_time_us,
                execution_time_us,
                total_time_us: start_time.elapsed().as_micros() as u64,
                retry_count,
                lane,
            },
        }
    }

    /// Execute action with retry logic.
    fn execute_with_retries<A: Action>(
        &self,
        action: &A,
        decision: &GateDecision,
        energy: &EnergySnapshot,
    ) -> (Result<A::Output, ActionError>, u32) {
        let mut ctx = ExecutionContext::new(
            action.metadata().id.clone(),
            energy.scope_energy,
            decision.lane,
        );

        let mut last_error_str: Option<String> = None;
        let mut delay = self.config.retry_delay;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                ctx = ExecutionContext::retry(&ctx);

                // Exponential backoff
                std::thread::sleep(delay);
                delay = (delay * 2).min(self.config.max_retry_delay);

                debug!(
                    action_id = %action.metadata().id,
                    attempt = attempt,
                    "Retrying action execution"
                );
            }

            match action.execute(&ctx) {
                Ok(output) => {
                    if attempt > 0 {
                        info!(
                            action_id = %action.metadata().id,
                            attempts = attempt + 1,
                            "Action succeeded after retries"
                        );
                    }
                    return (Ok(output), attempt);
                }
                Err(e) => {
                    let err_str = e.to_string();
                    warn!(
                        action_id = %action.metadata().id,
                        attempt = attempt,
                        error = %err_str,
                        "Action execution failed"
                    );
                    last_error_str = Some(err_str);

                    // Check if action supports retry
                    if !action.impact().allows_retry() {
                        break;
                    }
                }
            }
        }

        let error_msg = last_error_str.unwrap_or_else(|| "Unknown error".to_string());

        error!(
            action_id = %action.metadata().id,
            max_retries = self.config.max_retries,
            error = %error_msg,
            "Action failed after all retries"
        );

        (
            Err(ActionError::ExecutionFailed(format!(
                "Failed after {} retries: {}",
                self.config.max_retries, error_msg
            ))),
            self.config.max_retries,
        )
    }

    /// Store a witness record.
    fn store_witness(&self, witness: &WitnessRecord) {
        if !self.config.store_all_witnesses
            && witness.decision.allow
            && !witness.decision.is_escalated()
        {
            return;
        }

        let mut witnesses = self.witnesses.write();
        witnesses.push_back(witness.clone());

        // Trim old witnesses
        while witnesses.len() > self.config.max_witnesses_in_memory {
            witnesses.pop_front();
        }
    }

    /// Queue an action for human review.
    fn queue_for_human_review(
        &self,
        action_id: ActionId,
        witness: WitnessRecord,
        energy: EnergySnapshot,
    ) {
        let mut queue = self.human_queue.write();

        if queue.len() >= self.config.max_human_queue {
            warn!("Human review queue full, dropping oldest item");
            queue.pop_front();
        }

        queue.push_back(HumanReviewItem {
            action_id,
            witness,
            queued_at: Instant::now(),
            energy_snapshot: energy,
        });

        let mut stats = self.stats.write();
        stats.pending_human_review = queue.len();
    }

    /// Get the next item from the human review queue.
    pub fn pop_human_review(&self) -> Option<HumanReviewItem> {
        let mut queue = self.human_queue.write();
        let item = queue.pop_front();

        if item.is_some() {
            let mut stats = self.stats.write();
            stats.pending_human_review = queue.len();
        }

        item
    }

    /// Peek at the human review queue without removing.
    pub fn peek_human_review(&self) -> Option<HumanReviewItem> {
        let queue = self.human_queue.read();
        queue.front().map(|item| HumanReviewItem {
            action_id: item.action_id.clone(),
            witness: item.witness.clone(),
            queued_at: item.queued_at,
            energy_snapshot: item.energy_snapshot.clone(),
        })
    }

    /// Get current executor statistics.
    pub fn stats(&self) -> ExecutorStats {
        self.stats.read().clone()
    }

    /// Get recent witnesses.
    pub fn recent_witnesses(&self, limit: usize) -> Vec<WitnessRecord> {
        let witnesses = self.witnesses.read();
        witnesses.iter().rev().take(limit).cloned().collect()
    }

    /// Get a witness by ID.
    pub fn get_witness(&self, id: &super::gate::WitnessId) -> Option<WitnessRecord> {
        let witnesses = self.witnesses.read();
        witnesses.iter().find(|w| w.id == *id).cloned()
    }

    /// Get access to the gate for configuration updates.
    pub fn gate(&self) -> Arc<RwLock<CoherenceGate>> {
        self.gate.clone()
    }

    /// Reset executor state (for testing).
    pub fn reset(&self) {
        {
            let mut gate = self.gate.write();
            gate.reset();
        }
        {
            let mut stats = self.stats.write();
            *stats = ExecutorStats::default();
        }
        {
            let mut witnesses = self.witnesses.write();
            witnesses.clear();
        }
        {
            let mut queue = self.human_queue.write();
            queue.clear();
        }
    }
}

impl Clone for ActionExecutor {
    fn clone(&self) -> Self {
        Self {
            gate: self.gate.clone(),
            config: self.config.clone(),
            stats: self.stats.clone(),
            witnesses: self.witnesses.clone(),
            human_queue: self.human_queue.clone(),
        }
    }
}

/// Builder for creating a configured action result.
pub struct ActionResultBuilder {
    action_id: ActionId,
    success: bool,
    error_message: Option<String>,
    duration_us: u64,
    lane: ComputeLane,
    retry_count: u32,
}

impl ActionResultBuilder {
    /// Create a new builder.
    pub fn new(action_id: ActionId, lane: ComputeLane) -> Self {
        Self {
            action_id,
            success: true,
            error_message: None,
            duration_us: 0,
            lane,
            retry_count: 0,
        }
    }

    /// Mark as failed.
    pub fn failed(mut self, message: impl Into<String>) -> Self {
        self.success = false;
        self.error_message = Some(message.into());
        self
    }

    /// Set duration.
    pub fn duration_us(mut self, us: u64) -> Self {
        self.duration_us = us;
        self
    }

    /// Set retry count.
    pub fn retries(mut self, count: u32) -> Self {
        self.retry_count = count;
        self
    }

    /// Build the result.
    pub fn build(self) -> ActionResult {
        if self.success {
            ActionResult::success(
                self.action_id,
                self.duration_us,
                self.lane,
                self.retry_count,
            )
        } else {
            ActionResult::failure(
                self.action_id,
                self.error_message.unwrap_or_default(),
                self.duration_us,
                self.lane,
                self.retry_count,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::action::{ActionImpact, ActionMetadata, ScopeId};
    use crate::execution::gate::PolicyBundleRef;
    use std::sync::atomic::{AtomicU32, Ordering};

    // Test action that tracks execution
    struct TrackedAction {
        scope: ScopeId,
        metadata: ActionMetadata,
        execute_count: Arc<AtomicU32>,
        should_fail: bool,
    }

    impl TrackedAction {
        fn new(scope: &str) -> Self {
            Self {
                scope: ScopeId::new(scope),
                metadata: ActionMetadata::new("TrackedAction", "Test action", "test-actor"),
                execute_count: Arc::new(AtomicU32::new(0)),
                should_fail: false,
            }
        }

        fn failing(scope: &str) -> Self {
            Self {
                scope: ScopeId::new(scope),
                metadata: ActionMetadata::new("TrackedAction", "Failing action", "test-actor"),
                execute_count: Arc::new(AtomicU32::new(0)),
                should_fail: true,
            }
        }

        fn execution_count(&self) -> u32 {
            self.execute_count.load(Ordering::SeqCst)
        }
    }

    impl Action for TrackedAction {
        type Output = ();
        type Error = ActionError;

        fn scope(&self) -> &ScopeId {
            &self.scope
        }

        fn impact(&self) -> ActionImpact {
            ActionImpact::low()
        }

        fn metadata(&self) -> &ActionMetadata {
            &self.metadata
        }

        fn execute(&self, _ctx: &ExecutionContext) -> Result<(), ActionError> {
            self.execute_count.fetch_add(1, Ordering::SeqCst);
            if self.should_fail {
                Err(ActionError::ExecutionFailed(
                    "Simulated failure".to_string(),
                ))
            } else {
                Ok(())
            }
        }

        fn content_hash(&self) -> [u8; 32] {
            let hash = blake3::hash(self.scope.as_str().as_bytes());
            let mut result = [0u8; 32];
            result.copy_from_slice(hash.as_bytes());
            result
        }

        fn make_rollback_not_supported_error() -> ActionError {
            ActionError::RollbackNotSupported
        }
    }

    #[test]
    fn test_executor_success() {
        let gate = CoherenceGate::with_defaults(PolicyBundleRef::placeholder());
        let executor = ActionExecutor::with_defaults(gate);

        let action = TrackedAction::new("test.scope");
        let energy = EnergySnapshot::new(0.1, 0.05, action.scope.clone());

        let result = executor.execute(&action, &energy);

        assert!(result.result.is_ok());
        assert!(result.decision.allow);
        assert_eq!(result.decision.lane, ComputeLane::Reflex);
        assert!(result.witness.verify_integrity());
        assert_eq!(action.execution_count(), 1);

        let stats = executor.stats();
        assert_eq!(stats.total_submitted, 1);
        assert_eq!(stats.total_allowed, 1);
        assert_eq!(stats.total_success, 1);
    }

    #[test]
    fn test_executor_denial() {
        let gate = CoherenceGate::with_defaults(PolicyBundleRef::placeholder());
        let executor = ActionExecutor::with_defaults(gate);

        let action = TrackedAction::new("test.scope");
        let energy = EnergySnapshot::new(0.95, 0.9, action.scope.clone());

        let result = executor.execute(&action, &energy);

        assert!(result.result.is_err());
        assert!(!result.decision.allow);
        assert_eq!(result.decision.lane, ComputeLane::Human);
        assert_eq!(action.execution_count(), 0); // Never executed

        let stats = executor.stats();
        assert_eq!(stats.total_denied, 1);
    }

    #[test]
    fn test_executor_retry() {
        let gate = CoherenceGate::with_defaults(PolicyBundleRef::placeholder());
        let mut config = ExecutorConfig::default();
        config.max_retries = 2;
        config.retry_delay = Duration::from_millis(1);

        let executor = ActionExecutor::new(gate, config);

        let action = TrackedAction::failing("test.scope");
        let energy = EnergySnapshot::new(0.1, 0.05, action.scope.clone());

        let result = executor.execute(&action, &energy);

        assert!(result.result.is_err());
        assert_eq!(action.execution_count(), 3); // Initial + 2 retries
        assert_eq!(result.stats.retry_count, 2);

        let stats = executor.stats();
        assert_eq!(stats.total_failed, 1);
    }

    #[test]
    fn test_executor_witness_storage() {
        let gate = CoherenceGate::with_defaults(PolicyBundleRef::placeholder());
        let executor = ActionExecutor::with_defaults(gate);

        // Execute multiple actions
        for i in 0..5 {
            let action = TrackedAction::new(&format!("test.scope.{}", i));
            let energy = EnergySnapshot::new(0.1, 0.05, action.scope.clone());
            executor.execute(&action, &energy);
        }

        let witnesses = executor.recent_witnesses(10);
        assert_eq!(witnesses.len(), 5);

        // Witnesses should be in reverse chronological order
        for witness in &witnesses {
            assert!(witness.verify_integrity());
        }
    }

    #[test]
    fn test_executor_stats() {
        let gate = CoherenceGate::with_defaults(PolicyBundleRef::placeholder());
        let executor = ActionExecutor::with_defaults(gate);

        // Mix of successful and denied
        for i in 0..10 {
            let action = TrackedAction::new(&format!("test.scope.{}", i));
            let energy = if i % 3 == 0 {
                EnergySnapshot::new(0.95, 0.9, action.scope.clone()) // Will be denied
            } else {
                EnergySnapshot::new(0.1, 0.05, action.scope.clone()) // Will succeed
            };
            executor.execute(&action, &energy);
        }

        let stats = executor.stats();
        assert_eq!(stats.total_submitted, 10);
        assert!(stats.total_denied > 0);
        assert!(stats.total_success > 0);
        assert!(stats.success_rate() > 0.0);
        assert!(stats.denial_rate() > 0.0);
    }

    #[test]
    fn test_executor_clone() {
        let gate = CoherenceGate::with_defaults(PolicyBundleRef::placeholder());
        let executor = ActionExecutor::with_defaults(gate);

        let executor2 = executor.clone();

        // Execute on original
        let action = TrackedAction::new("test.scope");
        let energy = EnergySnapshot::new(0.1, 0.05, action.scope.clone());
        executor.execute(&action, &energy);

        // Stats should be shared
        assert_eq!(
            executor.stats().total_submitted,
            executor2.stats().total_submitted
        );
    }

    #[test]
    fn test_action_result_builder() {
        let action_id = ActionId::new();

        let success = ActionResultBuilder::new(action_id.clone(), ComputeLane::Reflex)
            .duration_us(500)
            .build();
        assert!(success.success);

        let failure = ActionResultBuilder::new(action_id, ComputeLane::Retrieval)
            .failed("Test error")
            .duration_us(1000)
            .retries(2)
            .build();
        assert!(!failure.success);
        assert_eq!(failure.retry_count, 2);
    }
}
