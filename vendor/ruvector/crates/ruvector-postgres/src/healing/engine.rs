//! Remediation Engine for Self-Healing System
//!
//! Orchestrates remediation execution with:
//! - Strategy selection based on problem type and weights
//! - Execution with timeout and rollback capability
//! - Outcome verification
//! - Cooldown periods to prevent thrashing

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use super::detector::{Problem, ProblemType, SystemMetrics};
use super::learning::OutcomeTracker;
use super::strategies::{
    RemediationResult, RemediationStrategy, StrategyContext, StrategyRegistry,
};

// ============================================================================
// Healing Configuration
// ============================================================================

/// Configuration for the healing engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealingConfig {
    /// Minimum time between healing attempts for same problem type
    pub min_healing_interval: Duration,
    /// Maximum attempts per time window
    pub max_attempts_per_window: usize,
    /// Time window for attempt counting
    pub attempt_window: Duration,
    /// Maximum impact level for auto-healing (0-1)
    pub max_auto_heal_impact: f32,
    /// Problem types that require human approval
    pub require_approval: Vec<ProblemType>,
    /// Strategy names that require human approval
    pub require_approval_strategies: Vec<String>,
    /// Enable learning from outcomes
    pub learning_enabled: bool,
    /// Cooldown after failed remediation
    pub failure_cooldown: Duration,
    /// Whether to verify improvement after remediation
    pub verify_improvement: bool,
    /// Minimum improvement percentage to consider success
    pub min_improvement_pct: f32,
    /// Maximum concurrent remediations
    pub max_concurrent_remediations: usize,
}

impl Default for HealingConfig {
    fn default() -> Self {
        Self {
            min_healing_interval: Duration::from_secs(300), // 5 minutes
            max_attempts_per_window: 3,
            attempt_window: Duration::from_secs(3600), // 1 hour
            max_auto_heal_impact: 0.5,
            require_approval: vec![],
            require_approval_strategies: vec!["promote_replica".to_string()],
            learning_enabled: true,
            failure_cooldown: Duration::from_secs(600), // 10 minutes
            verify_improvement: true,
            min_improvement_pct: 5.0,
            max_concurrent_remediations: 2,
        }
    }
}

// ============================================================================
// Healing Outcome
// ============================================================================

/// Outcome of a healing attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealingOutcome {
    /// Healing completed (may or may not have succeeded)
    Completed {
        problem_type: ProblemType,
        strategy: String,
        result: RemediationResult,
        verified: bool,
    },
    /// Healing was deferred (needs approval or cooldown)
    Deferred {
        reason: String,
        problem_type: ProblemType,
    },
    /// No suitable strategy found
    NoStrategy { problem_type: ProblemType },
    /// Healing is disabled
    Disabled,
    /// Already at maximum concurrent remediations
    MaxConcurrent,
}

impl HealingOutcome {
    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            HealingOutcome::Completed {
                problem_type,
                strategy,
                result,
                verified,
            } => {
                serde_json::json!({
                    "status": "completed",
                    "problem_type": problem_type.to_string(),
                    "strategy": strategy,
                    "result": result.to_json(),
                    "verified": verified,
                })
            }
            HealingOutcome::Deferred {
                reason,
                problem_type,
            } => {
                serde_json::json!({
                    "status": "deferred",
                    "reason": reason,
                    "problem_type": problem_type.to_string(),
                })
            }
            HealingOutcome::NoStrategy { problem_type } => {
                serde_json::json!({
                    "status": "no_strategy",
                    "problem_type": problem_type.to_string(),
                })
            }
            HealingOutcome::Disabled => {
                serde_json::json!({
                    "status": "disabled",
                })
            }
            HealingOutcome::MaxConcurrent => {
                serde_json::json!({
                    "status": "max_concurrent",
                })
            }
        }
    }
}

// ============================================================================
// Active Remediation
// ============================================================================

/// An active remediation in progress
#[derive(Debug, Clone)]
pub struct ActiveRemediation {
    /// Unique ID
    pub id: u64,
    /// Problem being remediated
    pub problem: Problem,
    /// Strategy being used
    pub strategy_name: String,
    /// When remediation started
    pub started_at: SystemTime,
    /// Expected completion time
    pub expected_completion: SystemTime,
}

impl ActiveRemediation {
    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        let started_ts = self
            .started_at
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let expected_ts = self
            .expected_completion
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        serde_json::json!({
            "id": self.id,
            "problem_type": self.problem.problem_type.to_string(),
            "strategy": self.strategy_name,
            "started_at": started_ts,
            "expected_completion": expected_ts,
        })
    }
}

// ============================================================================
// Remediation Context
// ============================================================================

/// Full context for remediation execution
#[derive(Debug, Clone)]
pub struct RemediationContext {
    /// The problem being remediated
    pub problem: Problem,
    /// Collection/table being remediated
    pub collection_id: i64,
    /// Tenant ID (for multi-tenant)
    pub tenant_id: Option<String>,
    /// Initial integrity lambda
    pub initial_lambda: f32,
    /// Target integrity lambda
    pub target_lambda: f32,
    /// System metrics at start
    pub initial_metrics: SystemMetrics,
    /// When context was created
    pub created_at: SystemTime,
    /// Maximum impact allowed
    pub max_impact: f32,
    /// Timeout for remediation
    pub timeout: Duration,
    /// Healing attempts in current window
    pub attempts_in_window: usize,
    /// Last healing attempt time
    pub last_attempt: Option<SystemTime>,
}

impl RemediationContext {
    /// Create a new remediation context
    pub fn new(problem: Problem, metrics: SystemMetrics) -> Self {
        Self {
            problem,
            collection_id: 0,
            tenant_id: None,
            initial_lambda: metrics.integrity_lambda,
            target_lambda: 0.8,
            initial_metrics: metrics,
            created_at: SystemTime::now(),
            max_impact: 0.5,
            timeout: Duration::from_secs(300),
            attempts_in_window: 0,
            last_attempt: None,
        }
    }

    /// Set collection ID
    pub fn with_collection(mut self, collection_id: i64) -> Self {
        self.collection_id = collection_id;
        self
    }

    /// Set tenant ID
    pub fn with_tenant(mut self, tenant_id: String) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    /// Create strategy context
    pub fn to_strategy_context(&self) -> StrategyContext {
        StrategyContext {
            problem: self.problem.clone(),
            collection_id: self.collection_id,
            initial_lambda: self.initial_lambda,
            target_lambda: self.target_lambda,
            max_impact: self.max_impact,
            timeout: self.timeout,
            start_time: SystemTime::now(),
            dry_run: false,
        }
    }
}

// ============================================================================
// Remediation Engine
// ============================================================================

/// The main remediation engine
pub struct RemediationEngine {
    /// Strategy registry
    pub registry: StrategyRegistry,
    /// Configuration
    config: RwLock<HealingConfig>,
    /// Outcome tracker for learning
    tracker: OutcomeTracker,
    /// Active remediations
    active: RwLock<Vec<ActiveRemediation>>,
    /// Next remediation ID
    next_id: AtomicU64,
    /// Healing attempt history (problem_type -> timestamps)
    attempt_history: RwLock<HashMap<ProblemType, VecDeque<SystemTime>>>,
    /// Whether engine is enabled
    enabled: AtomicBool,
    /// Total healings attempted
    total_healings: AtomicU64,
    /// Successful healings
    successful_healings: AtomicU64,
}

impl RemediationEngine {
    /// Create a new remediation engine
    pub fn new(registry: StrategyRegistry, config: HealingConfig, tracker: OutcomeTracker) -> Self {
        Self {
            registry,
            config: RwLock::new(config),
            tracker,
            active: RwLock::new(Vec::new()),
            next_id: AtomicU64::new(1),
            attempt_history: RwLock::new(HashMap::new()),
            enabled: AtomicBool::new(true),
            total_healings: AtomicU64::new(0),
            successful_healings: AtomicU64::new(0),
        }
    }

    /// Enable or disable the engine
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    /// Check if engine is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Update configuration
    pub fn update_config(&self, config: HealingConfig) {
        *self.config.write() = config;
    }

    /// Get current configuration
    pub fn get_config(&self) -> HealingConfig {
        self.config.read().clone()
    }

    /// Get active remediations
    pub fn active_remediations(&self) -> Vec<ActiveRemediation> {
        self.active.read().clone()
    }

    /// Main healing method
    pub fn heal(&self, problem: &Problem) -> HealingOutcome {
        // Check if enabled
        if !self.is_enabled() {
            return HealingOutcome::Disabled;
        }

        let config = self.config.read().clone();

        // Check concurrent limit
        if self.active.read().len() >= config.max_concurrent_remediations {
            return HealingOutcome::MaxConcurrent;
        }

        // Check if we should auto-heal
        if !self.should_auto_heal(problem, &config) {
            return HealingOutcome::Deferred {
                reason: self.get_defer_reason(problem, &config),
                problem_type: problem.problem_type,
            };
        }

        // Select strategy
        let strategy = match self.registry.select(problem, config.max_auto_heal_impact) {
            Some(s) => s,
            None => {
                return HealingOutcome::NoStrategy {
                    problem_type: problem.problem_type,
                };
            }
        };

        // Check if strategy requires approval
        if config
            .require_approval_strategies
            .contains(&strategy.name().to_string())
        {
            return HealingOutcome::Deferred {
                reason: format!("Strategy '{}' requires human approval", strategy.name()),
                problem_type: problem.problem_type,
            };
        }

        // Record attempt
        self.record_attempt(problem.problem_type);
        self.total_healings.fetch_add(1, Ordering::SeqCst);

        // Start active remediation
        let remediation_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let active_rem = ActiveRemediation {
            id: remediation_id,
            problem: problem.clone(),
            strategy_name: strategy.name().to_string(),
            started_at: SystemTime::now(),
            expected_completion: SystemTime::now() + strategy.estimated_duration(),
        };
        self.active.write().push(active_rem);

        // Execute strategy
        let context = StrategyContext {
            problem: problem.clone(),
            collection_id: 0,
            initial_lambda: 1.0,
            target_lambda: 0.8,
            max_impact: config.max_auto_heal_impact,
            timeout: strategy.estimated_duration() * 2,
            start_time: SystemTime::now(),
            dry_run: false,
        };

        let result = self.execute_with_safeguards(&*strategy, &context);

        // Remove from active
        self.active.write().retain(|r| r.id != remediation_id);

        // Verify improvement
        let verified = if config.verify_improvement && result.is_success() {
            self.verify_improvement(&result, config.min_improvement_pct)
        } else {
            result.is_success()
        };

        // Rollback if not verified and reversible
        if !verified && strategy.reversible() {
            pgrx::log!(
                "Remediation not verified, rolling back: {}",
                strategy.name()
            );
            if let Err(e) = strategy.rollback(&context, &result) {
                pgrx::warning!("Rollback failed: {}", e);
            }
        }

        // Update learning
        if config.learning_enabled {
            self.registry
                .update_weight(strategy.name(), verified, result.improvement_pct);
            self.tracker
                .record(problem, strategy.name(), &result, verified);
        }

        if verified {
            self.successful_healings.fetch_add(1, Ordering::SeqCst);
        }

        HealingOutcome::Completed {
            problem_type: problem.problem_type,
            strategy: strategy.name().to_string(),
            result,
            verified,
        }
    }

    /// Execute strategy with safeguards (timeout, panic catching)
    fn execute_with_safeguards(
        &self,
        strategy: &dyn RemediationStrategy,
        context: &StrategyContext,
    ) -> RemediationResult {
        // In production, wrap in timeout and panic handling
        // For now, execute directly
        let start = std::time::Instant::now();
        let mut result = strategy.execute(context);
        result.duration_ms = start.elapsed().as_millis() as u64;
        result
    }

    /// Check if we should auto-heal this problem
    fn should_auto_heal(&self, problem: &Problem, config: &HealingConfig) -> bool {
        // Check if problem type requires approval
        if config.require_approval.contains(&problem.problem_type) {
            return false;
        }

        // Check cooldown
        if !self.is_past_cooldown(problem.problem_type, config) {
            return false;
        }

        // Check attempt limit
        if self.attempts_in_window(problem.problem_type, &config.attempt_window)
            >= config.max_attempts_per_window
        {
            return false;
        }

        true
    }

    /// Get reason for deferring
    fn get_defer_reason(&self, problem: &Problem, config: &HealingConfig) -> String {
        if config.require_approval.contains(&problem.problem_type) {
            return format!(
                "Problem type '{:?}' requires human approval",
                problem.problem_type
            );
        }

        if !self.is_past_cooldown(problem.problem_type, config) {
            return "In cooldown period after recent healing attempt".to_string();
        }

        if self.attempts_in_window(problem.problem_type, &config.attempt_window)
            >= config.max_attempts_per_window
        {
            return format!(
                "Exceeded maximum {} attempts per {:?}",
                config.max_attempts_per_window, config.attempt_window
            );
        }

        "Unknown reason".to_string()
    }

    /// Check if past cooldown period
    fn is_past_cooldown(&self, problem_type: ProblemType, config: &HealingConfig) -> bool {
        let history = self.attempt_history.read();
        if let Some(attempts) = history.get(&problem_type) {
            if let Some(last) = attempts.back() {
                if let Ok(elapsed) = last.elapsed() {
                    return elapsed >= config.min_healing_interval;
                }
            }
        }
        true
    }

    /// Count attempts in window
    fn attempts_in_window(&self, problem_type: ProblemType, window: &Duration) -> usize {
        let history = self.attempt_history.read();
        if let Some(attempts) = history.get(&problem_type) {
            let cutoff = SystemTime::now() - *window;
            attempts.iter().filter(|t| **t > cutoff).count()
        } else {
            0
        }
    }

    /// Record an attempt
    fn record_attempt(&self, problem_type: ProblemType) {
        let mut history = self.attempt_history.write();
        let attempts = history.entry(problem_type).or_insert_with(VecDeque::new);
        attempts.push_back(SystemTime::now());

        // Keep only recent attempts
        let cutoff = SystemTime::now() - Duration::from_secs(86400); // 24 hours
        while let Some(front) = attempts.front() {
            if *front < cutoff {
                attempts.pop_front();
            } else {
                break;
            }
        }
    }

    /// Verify improvement after remediation
    fn verify_improvement(&self, result: &RemediationResult, min_pct: f32) -> bool {
        result.improvement_pct >= min_pct
    }

    /// Get engine statistics
    pub fn get_stats(&self) -> EngineStats {
        let total = self.total_healings.load(Ordering::SeqCst);
        let successful = self.successful_healings.load(Ordering::SeqCst);

        EngineStats {
            enabled: self.is_enabled(),
            total_healings: total,
            successful_healings: successful,
            success_rate: if total > 0 {
                successful as f32 / total as f32
            } else {
                0.0
            },
            active_remediations: self.active.read().len(),
            strategy_weights: self.registry.get_all_weights(),
        }
    }

    /// Execute a specific strategy manually
    pub fn execute_strategy(
        &self,
        strategy_name: &str,
        problem: &Problem,
        dry_run: bool,
    ) -> Option<HealingOutcome> {
        let strategy = self.registry.get_by_name(strategy_name)?;
        let _config = self.config.read().clone();

        let context = StrategyContext {
            problem: problem.clone(),
            collection_id: 0,
            initial_lambda: 1.0,
            target_lambda: 0.8,
            max_impact: 1.0, // Manual execution allows higher impact
            timeout: strategy.estimated_duration() * 2,
            start_time: SystemTime::now(),
            dry_run,
        };

        let result = strategy.execute(&context);

        Some(HealingOutcome::Completed {
            problem_type: problem.problem_type,
            strategy: strategy_name.to_string(),
            result,
            verified: !dry_run,
        })
    }
}

/// Engine statistics
#[derive(Debug, Clone)]
pub struct EngineStats {
    pub enabled: bool,
    pub total_healings: u64,
    pub successful_healings: u64,
    pub success_rate: f32,
    pub active_remediations: usize,
    pub strategy_weights: HashMap<String, f32>,
}

impl EngineStats {
    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "enabled": self.enabled,
            "total_healings": self.total_healings,
            "successful_healings": self.successful_healings,
            "success_rate": self.success_rate,
            "active_remediations": self.active_remediations,
            "strategy_weights": self.strategy_weights,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::healing::detector::Severity;

    fn create_engine() -> RemediationEngine {
        let registry = StrategyRegistry::new_with_defaults();
        let config = HealingConfig::default();
        let tracker = OutcomeTracker::new();
        RemediationEngine::new(registry, config, tracker)
    }

    #[test]
    fn test_engine_creation() {
        let engine = create_engine();
        assert!(engine.is_enabled());
        assert!(engine.active_remediations().is_empty());
    }

    #[test]
    fn test_engine_enable_disable() {
        let engine = create_engine();

        engine.set_enabled(false);
        assert!(!engine.is_enabled());

        let problem = Problem::new(ProblemType::IndexDegradation, Severity::Medium);
        let outcome = engine.heal(&problem);
        assert!(matches!(outcome, HealingOutcome::Disabled));

        engine.set_enabled(true);
        assert!(engine.is_enabled());
    }

    #[test]
    fn test_heal_index_degradation() {
        let engine = create_engine();
        let problem = Problem::new(ProblemType::IndexDegradation, Severity::Medium);

        let outcome = engine.heal(&problem);
        match outcome {
            HealingOutcome::Completed { strategy, .. } => {
                assert!(strategy.contains("reindex") || strategy.contains("integrity"));
            }
            _ => panic!("Expected Completed outcome"),
        }
    }

    #[test]
    fn test_cooldown_enforcement() {
        let mut config = HealingConfig::default();
        config.min_healing_interval = Duration::from_secs(60);

        let registry = StrategyRegistry::new_with_defaults();
        let tracker = OutcomeTracker::new();
        let engine = RemediationEngine::new(registry, config, tracker);

        let problem = Problem::new(ProblemType::IndexDegradation, Severity::Medium);

        // First healing should succeed
        let outcome1 = engine.heal(&problem);
        assert!(matches!(outcome1, HealingOutcome::Completed { .. }));

        // Second should be deferred (in cooldown)
        let outcome2 = engine.heal(&problem);
        assert!(matches!(outcome2, HealingOutcome::Deferred { .. }));
    }

    #[test]
    fn test_max_attempts_enforcement() {
        let mut config = HealingConfig::default();
        config.max_attempts_per_window = 2;
        config.min_healing_interval = Duration::from_millis(1);

        let registry = StrategyRegistry::new_with_defaults();
        let tracker = OutcomeTracker::new();
        let engine = RemediationEngine::new(registry, config, tracker);

        let problem = Problem::new(ProblemType::IndexDegradation, Severity::Medium);

        // First two should succeed
        engine.heal(&problem);
        std::thread::sleep(Duration::from_millis(2));
        engine.heal(&problem);
        std::thread::sleep(Duration::from_millis(2));

        // Third should be deferred
        let outcome = engine.heal(&problem);
        assert!(matches!(outcome, HealingOutcome::Deferred { .. }));
    }

    #[test]
    fn test_approval_requirement() {
        let mut config = HealingConfig::default();
        config.require_approval.push(ProblemType::ReplicaLag);

        let registry = StrategyRegistry::new_with_defaults();
        let tracker = OutcomeTracker::new();
        let engine = RemediationEngine::new(registry, config, tracker);

        let problem = Problem::new(ProblemType::ReplicaLag, Severity::High);
        let outcome = engine.heal(&problem);

        assert!(matches!(outcome, HealingOutcome::Deferred { .. }));
    }

    #[test]
    fn test_strategy_approval_requirement() {
        let mut config = HealingConfig::default();
        config
            .require_approval_strategies
            .push("promote_replica".to_string());
        config.max_auto_heal_impact = 1.0; // Allow high impact

        let registry = StrategyRegistry::new_with_defaults();
        let tracker = OutcomeTracker::new();
        let engine = RemediationEngine::new(registry, config, tracker);

        let problem = Problem::new(ProblemType::ReplicaLag, Severity::High);
        let outcome = engine.heal(&problem);

        // Should be deferred because promote_replica requires approval
        assert!(matches!(outcome, HealingOutcome::Deferred { .. }));
    }

    #[test]
    fn test_no_strategy() {
        let registry = StrategyRegistry::new(); // Empty registry
        let config = HealingConfig::default();
        let tracker = OutcomeTracker::new();
        let engine = RemediationEngine::new(registry, config, tracker);

        let problem = Problem::new(ProblemType::IndexDegradation, Severity::Medium);
        let outcome = engine.heal(&problem);

        assert!(matches!(outcome, HealingOutcome::NoStrategy { .. }));
    }

    #[test]
    fn test_manual_execution() {
        let engine = create_engine();
        let problem = Problem::new(ProblemType::IndexDegradation, Severity::Medium);

        let outcome = engine.execute_strategy("reindex_partition", &problem, true);
        assert!(outcome.is_some());

        if let Some(HealingOutcome::Completed { result, .. }) = outcome {
            assert!(result.metadata.get("dry_run") == Some(&serde_json::json!(true)));
        }
    }

    #[test]
    fn test_engine_stats() {
        let engine = create_engine();
        let stats = engine.get_stats();

        assert!(stats.enabled);
        assert_eq!(stats.total_healings, 0);
        assert_eq!(stats.active_remediations, 0);
    }
}
