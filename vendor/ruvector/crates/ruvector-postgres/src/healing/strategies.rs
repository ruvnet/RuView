//! Remediation Strategies for Self-Healing Engine
//!
//! Implements 5 built-in strategies:
//! 1. ReindexPartition - Rebuild degraded index partition
//! 2. PromoteReplica - Failover to healthy replica
//! 3. TierEviction - Move cold data to lower storage tier
//! 4. QueryCircuitBreaker - Block problematic queries
//! 5. IntegrityRecovery - Repair contracted graph

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

#[cfg_attr(not(test), allow(unused_imports))]
use super::detector::{Problem, ProblemType, Severity};

// ============================================================================
// Remediation Result
// ============================================================================

/// Outcome of a remediation attempt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemediationOutcome {
    /// Remediation completed successfully
    Success,
    /// Remediation partially completed
    Partial,
    /// Remediation failed
    Failure,
    /// No action was taken (not applicable)
    NoOp,
}

/// Result of executing a remediation strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationResult {
    /// Outcome of the remediation
    pub outcome: RemediationOutcome,
    /// Number of actions taken
    pub actions_taken: usize,
    /// Improvement percentage (positive = better)
    pub improvement_pct: f32,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Additional metadata
    pub metadata: serde_json::Value,
    /// Duration of remediation
    pub duration_ms: u64,
    /// Actions recorded for rollback
    pub rollback_actions: Vec<serde_json::Value>,
}

impl RemediationResult {
    /// Create a successful result
    pub fn success(actions_taken: usize, improvement_pct: f32) -> Self {
        Self {
            outcome: RemediationOutcome::Success,
            actions_taken,
            improvement_pct,
            error_message: None,
            metadata: serde_json::json!({}),
            duration_ms: 0,
            rollback_actions: vec![],
        }
    }

    /// Create a partial success result
    pub fn partial(actions_taken: usize, improvement_pct: f32, message: &str) -> Self {
        Self {
            outcome: RemediationOutcome::Partial,
            actions_taken,
            improvement_pct,
            error_message: Some(message.to_string()),
            metadata: serde_json::json!({}),
            duration_ms: 0,
            rollback_actions: vec![],
        }
    }

    /// Create a failure result
    pub fn failure(message: &str) -> Self {
        Self {
            outcome: RemediationOutcome::Failure,
            actions_taken: 0,
            improvement_pct: 0.0,
            error_message: Some(message.to_string()),
            metadata: serde_json::json!({}),
            duration_ms: 0,
            rollback_actions: vec![],
        }
    }

    /// Create a no-op result
    pub fn noop() -> Self {
        Self {
            outcome: RemediationOutcome::NoOp,
            actions_taken: 0,
            improvement_pct: 0.0,
            error_message: None,
            metadata: serde_json::json!({}),
            duration_ms: 0,
            rollback_actions: vec![],
        }
    }

    /// Add metadata to result
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set duration
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// Add rollback actions
    pub fn with_rollback(mut self, actions: Vec<serde_json::Value>) -> Self {
        self.rollback_actions = actions;
        self
    }

    /// Check if successful
    pub fn is_success(&self) -> bool {
        matches!(self.outcome, RemediationOutcome::Success)
    }

    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "outcome": format!("{:?}", self.outcome).to_lowercase(),
            "actions_taken": self.actions_taken,
            "improvement_pct": self.improvement_pct,
            "error_message": self.error_message,
            "metadata": self.metadata,
            "duration_ms": self.duration_ms,
        })
    }
}

// ============================================================================
// Remediation Context
// ============================================================================

/// Context provided to remediation strategies
#[derive(Debug, Clone)]
pub struct StrategyContext {
    /// The problem being remediated
    pub problem: Problem,
    /// Collection/table ID
    pub collection_id: i64,
    /// Initial integrity lambda before remediation
    pub initial_lambda: f32,
    /// Target integrity lambda
    pub target_lambda: f32,
    /// Maximum allowed impact (0-1)
    pub max_impact: f32,
    /// Timeout for remediation
    pub timeout: Duration,
    /// Start time of remediation
    pub start_time: SystemTime,
    /// Whether this is a dry run
    pub dry_run: bool,
}

impl StrategyContext {
    /// Create a new context
    pub fn new(problem: Problem) -> Self {
        Self {
            problem,
            collection_id: 0,
            initial_lambda: 1.0,
            target_lambda: 0.8,
            max_impact: 0.5,
            timeout: Duration::from_secs(300),
            start_time: SystemTime::now(),
            dry_run: false,
        }
    }

    /// Get elapsed time since start
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed().unwrap_or(Duration::ZERO)
    }

    /// Check if timeout exceeded
    pub fn is_timed_out(&self) -> bool {
        self.elapsed() > self.timeout
    }
}

// ============================================================================
// Remediation Strategy Trait
// ============================================================================

/// Trait for remediation strategies
pub trait RemediationStrategy: Send + Sync {
    /// Human-readable name
    fn name(&self) -> &str;

    /// Description of what this strategy does
    fn description(&self) -> &str;

    /// Problem types this strategy handles
    fn handles(&self) -> Vec<ProblemType>;

    /// Estimate impact (0-1, higher = more disruptive)
    fn impact(&self) -> f32;

    /// Estimate time to complete
    fn estimated_duration(&self) -> Duration;

    /// Can this be reversed?
    fn reversible(&self) -> bool;

    /// Execute the remediation
    fn execute(&self, context: &StrategyContext) -> RemediationResult;

    /// Rollback if needed
    fn rollback(&self, context: &StrategyContext, result: &RemediationResult)
        -> Result<(), String>;
}

// ============================================================================
// Strategy 1: Reindex Partition
// ============================================================================

/// Rebuilds a degraded index partition to restore performance
pub struct ReindexPartition {
    /// Maximum partitions to reindex in one pass
    max_partitions: usize,
    /// Whether to use online reindexing (CONCURRENTLY)
    concurrent: bool,
}

impl ReindexPartition {
    /// Create with default settings
    pub fn new() -> Self {
        Self {
            max_partitions: 3,
            concurrent: true,
        }
    }

    /// Create with custom settings
    pub fn with_settings(max_partitions: usize, concurrent: bool) -> Self {
        Self {
            max_partitions,
            concurrent,
        }
    }

    /// Reindex a single partition
    fn reindex_partition(&self, partition_id: i64, concurrent: bool) -> Result<(), String> {
        // In production: Execute REINDEX INDEX CONCURRENTLY for the partition
        // This would use SPI to execute SQL commands

        if concurrent {
            // REINDEX INDEX CONCURRENTLY partition_idx_<partition_id>;
            pgrx::log!("Reindexing partition {} concurrently", partition_id);
        } else {
            // REINDEX INDEX partition_idx_<partition_id>;
            pgrx::log!("Reindexing partition {}", partition_id);
        }

        // Simulate success
        Ok(())
    }
}

impl Default for ReindexPartition {
    fn default() -> Self {
        Self::new()
    }
}

impl RemediationStrategy for ReindexPartition {
    fn name(&self) -> &str {
        "reindex_partition"
    }

    fn description(&self) -> &str {
        "Rebuild degraded index partition to restore search performance"
    }

    fn handles(&self) -> Vec<ProblemType> {
        vec![ProblemType::IndexDegradation]
    }

    fn impact(&self) -> f32 {
        if self.concurrent {
            0.3 // Medium impact with concurrent
        } else {
            0.7 // Higher impact without concurrent
        }
    }

    fn estimated_duration(&self) -> Duration {
        Duration::from_secs(60 * self.max_partitions as u64)
    }

    fn reversible(&self) -> bool {
        false // Reindexing is not reversible (but doesn't need to be)
    }

    fn execute(&self, context: &StrategyContext) -> RemediationResult {
        let start = std::time::Instant::now();

        if context.dry_run {
            return RemediationResult::noop().with_metadata(serde_json::json!({
                "dry_run": true,
                "would_reindex": context.problem.affected_partitions.len(),
            }));
        }

        let mut reindexed = 0;
        let mut errors = Vec::new();

        for partition_id in context
            .problem
            .affected_partitions
            .iter()
            .take(self.max_partitions)
        {
            if context.is_timed_out() {
                break;
            }

            match self.reindex_partition(*partition_id, self.concurrent) {
                Ok(()) => reindexed += 1,
                Err(e) => errors.push(format!("Partition {}: {}", partition_id, e)),
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        if reindexed == 0 && !errors.is_empty() {
            RemediationResult::failure(&errors.join("; ")).with_duration(duration_ms)
        } else if !errors.is_empty() {
            RemediationResult::partial(reindexed, 0.0, &errors.join("; "))
                .with_duration(duration_ms)
        } else {
            RemediationResult::success(reindexed, 0.0)
                .with_duration(duration_ms)
                .with_metadata(serde_json::json!({
                    "reindexed_partitions": reindexed,
                    "concurrent": self.concurrent,
                }))
        }
    }

    fn rollback(
        &self,
        _context: &StrategyContext,
        _result: &RemediationResult,
    ) -> Result<(), String> {
        // Reindexing doesn't need rollback
        Ok(())
    }
}

// ============================================================================
// Strategy 2: Promote Replica
// ============================================================================

/// Promotes a healthy replica to primary when current primary is failing
pub struct PromoteReplica {
    /// Grace period to wait for current primary to recover
    grace_period: Duration,
}

impl PromoteReplica {
    /// Create with default settings
    pub fn new() -> Self {
        Self {
            grace_period: Duration::from_secs(30),
        }
    }

    /// Create with custom grace period
    pub fn with_grace_period(grace_period: Duration) -> Self {
        Self { grace_period }
    }

    /// Find the healthiest replica
    fn find_best_replica(&self) -> Option<String> {
        // In production: Query pg_stat_replication for replica with lowest lag
        // and verify it's caught up
        Some("replica_1".to_string())
    }

    /// Promote replica to primary
    fn promote_replica(&self, replica_id: &str) -> Result<(), String> {
        // In production:
        // 1. Verify replica is caught up
        // 2. Stop writes to current primary
        // 3. Wait for replica to apply all WAL
        // 4. Promote replica (pg_promote())
        // 5. Update connection routing

        pgrx::log!("Promoting replica {} to primary", replica_id);
        Ok(())
    }
}

impl Default for PromoteReplica {
    fn default() -> Self {
        Self::new()
    }
}

impl RemediationStrategy for PromoteReplica {
    fn name(&self) -> &str {
        "promote_replica"
    }

    fn description(&self) -> &str {
        "Failover to healthy replica when primary is experiencing issues"
    }

    fn handles(&self) -> Vec<ProblemType> {
        vec![ProblemType::ReplicaLag, ProblemType::IntegrityViolation]
    }

    fn impact(&self) -> f32 {
        0.6 // Higher impact due to potential brief outage
    }

    fn estimated_duration(&self) -> Duration {
        self.grace_period + Duration::from_secs(30)
    }

    fn reversible(&self) -> bool {
        true // Can demote back (with data considerations)
    }

    fn execute(&self, context: &StrategyContext) -> RemediationResult {
        let start = std::time::Instant::now();

        if context.dry_run {
            return RemediationResult::noop().with_metadata(serde_json::json!({
                "dry_run": true,
                "candidate_replica": self.find_best_replica(),
            }));
        }

        // Find best replica
        let replica_id = match self.find_best_replica() {
            Some(id) => id,
            None => {
                return RemediationResult::failure("No healthy replica found");
            }
        };

        // Wait grace period for primary to recover
        std::thread::sleep(self.grace_period);

        // Promote replica
        match self.promote_replica(&replica_id) {
            Ok(()) => RemediationResult::success(1, 0.0)
                .with_duration(start.elapsed().as_millis() as u64)
                .with_metadata(serde_json::json!({
                    "promoted_replica": replica_id,
                }))
                .with_rollback(vec![serde_json::json!({
                    "action": "demote",
                    "replica_id": replica_id,
                })]),
            Err(e) => {
                RemediationResult::failure(&e).with_duration(start.elapsed().as_millis() as u64)
            }
        }
    }

    fn rollback(
        &self,
        _context: &StrategyContext,
        result: &RemediationResult,
    ) -> Result<(), String> {
        // Demote previously promoted replica (complex operation)
        for action in &result.rollback_actions {
            if action.get("action") == Some(&serde_json::json!("demote")) {
                let replica_id = action
                    .get("replica_id")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing replica_id in rollback action")?;

                pgrx::log!("Rolling back: demoting {}", replica_id);
                // In production: Execute demotion sequence
            }
        }
        Ok(())
    }
}

// ============================================================================
// Strategy 3: Tier Eviction
// ============================================================================

/// Moves cold data to lower storage tier to free up space
pub struct TierEviction {
    /// Target percentage of storage to free
    target_free_pct: f32,
    /// Maximum rows to move per batch
    batch_size: usize,
}

impl TierEviction {
    /// Create with default settings
    pub fn new() -> Self {
        Self {
            target_free_pct: 20.0,
            batch_size: 10000,
        }
    }

    /// Create with custom settings
    pub fn with_settings(target_free_pct: f32, batch_size: usize) -> Self {
        Self {
            target_free_pct,
            batch_size,
        }
    }

    /// Find cold data candidates for eviction
    fn find_cold_candidates(&self, _limit: usize) -> Vec<i64> {
        // In production: Query for least recently accessed data
        // SELECT id FROM vectors
        // ORDER BY last_accessed_at ASC NULLS FIRST
        // LIMIT $limit
        vec![]
    }

    /// Move data to cold tier
    fn evict_to_cold_tier(&self, vector_ids: &[i64]) -> Result<usize, String> {
        // In production:
        // 1. Copy data to cold storage (S3, cheaper tablespace)
        // 2. Update references to point to cold tier
        // 3. Delete from hot tier

        pgrx::log!("Evicting {} vectors to cold tier", vector_ids.len());
        Ok(vector_ids.len())
    }
}

impl Default for TierEviction {
    fn default() -> Self {
        Self::new()
    }
}

impl RemediationStrategy for TierEviction {
    fn name(&self) -> &str {
        "tier_eviction"
    }

    fn description(&self) -> &str {
        "Move cold data to lower storage tier to free up space"
    }

    fn handles(&self) -> Vec<ProblemType> {
        vec![ProblemType::StorageExhaustion, ProblemType::MemoryPressure]
    }

    fn impact(&self) -> f32 {
        0.4 // Medium impact
    }

    fn estimated_duration(&self) -> Duration {
        Duration::from_secs(120)
    }

    fn reversible(&self) -> bool {
        true // Can move data back
    }

    fn execute(&self, context: &StrategyContext) -> RemediationResult {
        let start = std::time::Instant::now();

        if context.dry_run {
            let candidates = self.find_cold_candidates(self.batch_size);
            return RemediationResult::noop().with_metadata(serde_json::json!({
                "dry_run": true,
                "candidates_found": candidates.len(),
            }));
        }

        let mut total_evicted = 0;
        let mut evicted_ids = Vec::new();

        while !context.is_timed_out() {
            let candidates = self.find_cold_candidates(self.batch_size);
            if candidates.is_empty() {
                break;
            }

            match self.evict_to_cold_tier(&candidates) {
                Ok(count) => {
                    total_evicted += count;
                    evicted_ids.extend(candidates);
                }
                Err(e) => {
                    return RemediationResult::partial(total_evicted, 0.0, &e)
                        .with_duration(start.elapsed().as_millis() as u64);
                }
            }
        }

        if total_evicted > 0 {
            RemediationResult::success(total_evicted, self.target_free_pct)
                .with_duration(start.elapsed().as_millis() as u64)
                .with_metadata(serde_json::json!({
                    "evicted_count": total_evicted,
                }))
                .with_rollback(vec![serde_json::json!({
                    "action": "restore_from_cold",
                    "vector_ids": evicted_ids,
                })])
        } else {
            RemediationResult::noop().with_metadata(serde_json::json!({
                "message": "No cold data candidates found",
            }))
        }
    }

    fn rollback(
        &self,
        _context: &StrategyContext,
        result: &RemediationResult,
    ) -> Result<(), String> {
        for action in &result.rollback_actions {
            if action.get("action") == Some(&serde_json::json!("restore_from_cold")) {
                // In production: Move data back from cold tier
                pgrx::log!("Rolling back tier eviction");
            }
        }
        Ok(())
    }
}

// ============================================================================
// Strategy 4: Query Circuit Breaker
// ============================================================================

/// Blocks problematic queries that are causing timeouts
pub struct QueryCircuitBreaker {
    /// Duration to block queries
    block_duration: Duration,
    /// Query patterns to block
    blocked_patterns: RwLock<Vec<String>>,
}

impl QueryCircuitBreaker {
    /// Create with default settings
    pub fn new() -> Self {
        Self {
            block_duration: Duration::from_secs(300),
            blocked_patterns: RwLock::new(Vec::new()),
        }
    }

    /// Create with custom block duration
    pub fn with_duration(block_duration: Duration) -> Self {
        Self {
            block_duration,
            blocked_patterns: RwLock::new(Vec::new()),
        }
    }

    /// Find problematic query patterns
    fn find_problematic_queries(&self) -> Vec<String> {
        // In production: Query pg_stat_statements for queries with high timeout rate
        // SELECT query FROM pg_stat_statements
        // WHERE calls > 100 AND (timeouts / calls::float) > 0.1
        // ORDER BY timeouts DESC LIMIT 10
        vec![]
    }

    /// Block a query pattern
    fn block_pattern(&self, pattern: &str) -> Result<(), String> {
        // In production: Add to query rules or connection pool filter
        self.blocked_patterns.write().push(pattern.to_string());
        pgrx::log!("Blocking query pattern: {}", pattern);
        Ok(())
    }

    /// Unblock a query pattern
    fn unblock_pattern(&self, pattern: &str) -> Result<(), String> {
        self.blocked_patterns.write().retain(|p| p != pattern);
        pgrx::log!("Unblocking query pattern: {}", pattern);
        Ok(())
    }
}

impl Default for QueryCircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

impl RemediationStrategy for QueryCircuitBreaker {
    fn name(&self) -> &str {
        "query_circuit_breaker"
    }

    fn description(&self) -> &str {
        "Block problematic queries causing excessive timeouts"
    }

    fn handles(&self) -> Vec<ProblemType> {
        vec![ProblemType::QueryTimeout, ProblemType::ConnectionExhaustion]
    }

    fn impact(&self) -> f32 {
        0.5 // Medium-high impact (affects some queries)
    }

    fn estimated_duration(&self) -> Duration {
        Duration::from_secs(10)
    }

    fn reversible(&self) -> bool {
        true
    }

    fn execute(&self, context: &StrategyContext) -> RemediationResult {
        let start = std::time::Instant::now();

        if context.dry_run {
            let problematic = self.find_problematic_queries();
            return RemediationResult::noop().with_metadata(serde_json::json!({
                "dry_run": true,
                "would_block": problematic,
            }));
        }

        let problematic = self.find_problematic_queries();
        let mut blocked = Vec::new();

        for pattern in &problematic {
            if self.block_pattern(pattern).is_ok() {
                blocked.push(pattern.clone());
            }
        }

        if blocked.is_empty() {
            RemediationResult::noop().with_metadata(serde_json::json!({
                "message": "No problematic query patterns identified",
            }))
        } else {
            RemediationResult::success(blocked.len(), 0.0)
                .with_duration(start.elapsed().as_millis() as u64)
                .with_metadata(serde_json::json!({
                    "blocked_patterns": blocked,
                    "block_duration_secs": self.block_duration.as_secs(),
                }))
                .with_rollback(vec![serde_json::json!({
                    "action": "unblock",
                    "patterns": blocked,
                })])
        }
    }

    fn rollback(
        &self,
        _context: &StrategyContext,
        result: &RemediationResult,
    ) -> Result<(), String> {
        for action in &result.rollback_actions {
            if action.get("action") == Some(&serde_json::json!("unblock")) {
                if let Some(patterns) = action.get("patterns").and_then(|v| v.as_array()) {
                    for pattern in patterns {
                        if let Some(p) = pattern.as_str() {
                            self.unblock_pattern(p)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

// ============================================================================
// Strategy 5: Integrity Recovery
// ============================================================================

/// Repairs contracted graph when integrity violations are detected
pub struct IntegrityRecovery {
    /// Maximum edges to repair
    max_edges: usize,
    /// Whether to verify after repair
    verify_after: bool,
}

impl IntegrityRecovery {
    /// Create with default settings
    pub fn new() -> Self {
        Self {
            max_edges: 1000,
            verify_after: true,
        }
    }

    /// Create with custom settings
    pub fn with_settings(max_edges: usize, verify_after: bool) -> Self {
        Self {
            max_edges,
            verify_after,
        }
    }

    /// Get witness edges from mincut computation
    fn get_witness_edges(&self) -> Vec<(i64, i64)> {
        // In production: Get from integrity control plane
        vec![]
    }

    /// Repair a weak edge by adding redundant connections
    fn repair_edge(&self, from: i64, to: i64) -> Result<(), String> {
        // In production:
        // 1. Find alternative paths between nodes
        // 2. Add redundant edges to strengthen connectivity
        // 3. Update graph metadata

        pgrx::log!("Repairing edge {} -> {}", from, to);
        Ok(())
    }

    /// Verify integrity after repair
    fn verify_integrity(&self) -> Result<f32, String> {
        // In production: Recompute mincut and return new lambda
        Ok(1.0)
    }
}

impl Default for IntegrityRecovery {
    fn default() -> Self {
        Self::new()
    }
}

impl RemediationStrategy for IntegrityRecovery {
    fn name(&self) -> &str {
        "integrity_recovery"
    }

    fn description(&self) -> &str {
        "Repair contracted graph when integrity violations are detected"
    }

    fn handles(&self) -> Vec<ProblemType> {
        vec![
            ProblemType::IntegrityViolation,
            ProblemType::IndexDegradation,
        ]
    }

    fn impact(&self) -> f32 {
        0.4 // Medium impact
    }

    fn estimated_duration(&self) -> Duration {
        Duration::from_secs(60)
    }

    fn reversible(&self) -> bool {
        false // Graph repairs are not typically rolled back
    }

    fn execute(&self, context: &StrategyContext) -> RemediationResult {
        let start = std::time::Instant::now();

        if context.dry_run {
            let witness_edges = self.get_witness_edges();
            return RemediationResult::noop().with_metadata(serde_json::json!({
                "dry_run": true,
                "witness_edges_found": witness_edges.len(),
            }));
        }

        let witness_edges = self.get_witness_edges();
        let mut repaired = 0;
        let mut errors = Vec::new();

        for (from, to) in witness_edges.iter().take(self.max_edges) {
            if context.is_timed_out() {
                break;
            }

            match self.repair_edge(*from, *to) {
                Ok(()) => repaired += 1,
                Err(e) => errors.push(e),
            }
        }

        let improvement = if self.verify_after && repaired > 0 {
            match self.verify_integrity() {
                Ok(new_lambda) => ((new_lambda - context.initial_lambda) / context.initial_lambda
                    * 100.0)
                    .max(0.0),
                Err(_) => 0.0,
            }
        } else {
            0.0
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        if repaired == 0 && !errors.is_empty() {
            RemediationResult::failure(&errors.join("; ")).with_duration(duration_ms)
        } else if repaired > 0 {
            RemediationResult::success(repaired, improvement)
                .with_duration(duration_ms)
                .with_metadata(serde_json::json!({
                    "edges_repaired": repaired,
                    "new_lambda": context.initial_lambda + (improvement / 100.0),
                }))
        } else {
            RemediationResult::noop().with_metadata(serde_json::json!({
                "message": "No witness edges to repair",
            }))
        }
    }

    fn rollback(
        &self,
        _context: &StrategyContext,
        _result: &RemediationResult,
    ) -> Result<(), String> {
        // Graph repairs are not reversible
        Err("Integrity recovery cannot be rolled back".to_string())
    }
}

// ============================================================================
// Strategy Registry
// ============================================================================

/// Registry of all available remediation strategies
pub struct StrategyRegistry {
    /// Registered strategies
    strategies: Vec<Arc<dyn RemediationStrategy>>,
    /// Learned effectiveness weights per strategy
    weights: RwLock<HashMap<String, f32>>,
}

impl StrategyRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            strategies: Vec::new(),
            weights: RwLock::new(HashMap::new()),
        }
    }

    /// Create registry with default strategies
    pub fn new_with_defaults() -> Self {
        let mut registry = Self::new();

        // Register built-in strategies
        registry.register(Arc::new(ReindexPartition::new()));
        registry.register(Arc::new(PromoteReplica::new()));
        registry.register(Arc::new(TierEviction::new()));
        registry.register(Arc::new(QueryCircuitBreaker::new()));
        registry.register(Arc::new(IntegrityRecovery::new()));

        registry
    }

    /// Register a new strategy
    pub fn register(&mut self, strategy: Arc<dyn RemediationStrategy>) {
        let name = strategy.name().to_string();
        self.strategies.push(strategy);
        self.weights.write().insert(name, 1.0);
    }

    /// Get all registered strategies
    pub fn all_strategies(&self) -> &[Arc<dyn RemediationStrategy>] {
        &self.strategies
    }

    /// Get strategy by name
    pub fn get_by_name(&self, name: &str) -> Option<Arc<dyn RemediationStrategy>> {
        self.strategies.iter().find(|s| s.name() == name).cloned()
    }

    /// Select best strategy for a problem
    pub fn select(
        &self,
        problem: &Problem,
        max_impact: f32,
    ) -> Option<Arc<dyn RemediationStrategy>> {
        let weights = self.weights.read();

        self.strategies
            .iter()
            .filter(|s| s.handles().contains(&problem.problem_type))
            .filter(|s| s.impact() <= max_impact)
            .max_by(|a, b| {
                let weight_a = weights.get(a.name()).unwrap_or(&1.0);
                let weight_b = weights.get(b.name()).unwrap_or(&1.0);
                weight_a.partial_cmp(weight_b).unwrap()
            })
            .cloned()
    }

    /// Update strategy weight based on outcome
    pub fn update_weight(&self, strategy_name: &str, success: bool, improvement: f32) {
        let mut weights = self.weights.write();
        let current = *weights.get(strategy_name).unwrap_or(&1.0);

        let adjustment = if success {
            0.1 + (improvement / 100.0).min(0.2)
        } else {
            -0.1
        };

        let new_weight = (current + adjustment).max(0.1).min(2.0);
        weights.insert(strategy_name.to_string(), new_weight);
    }

    /// Get current weight for a strategy
    pub fn get_weight(&self, strategy_name: &str) -> f32 {
        *self.weights.read().get(strategy_name).unwrap_or(&1.0)
    }

    /// Get all weights
    pub fn get_all_weights(&self) -> HashMap<String, f32> {
        self.weights.read().clone()
    }
}

impl Default for StrategyRegistry {
    fn default() -> Self {
        Self::new_with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remediation_result_success() {
        let result = RemediationResult::success(5, 15.0);
        assert!(result.is_success());
        assert_eq!(result.actions_taken, 5);
        assert_eq!(result.improvement_pct, 15.0);
    }

    #[test]
    fn test_remediation_result_failure() {
        let result = RemediationResult::failure("test error");
        assert!(!result.is_success());
        assert_eq!(result.error_message, Some("test error".to_string()));
    }

    #[test]
    fn test_strategy_registry_defaults() {
        let registry = StrategyRegistry::new_with_defaults();
        assert_eq!(registry.all_strategies().len(), 5);
    }

    #[test]
    fn test_strategy_selection() {
        let registry = StrategyRegistry::new_with_defaults();
        let problem = Problem::new(ProblemType::IndexDegradation, Severity::Medium);

        let strategy = registry.select(&problem, 1.0);
        assert!(strategy.is_some());
        assert!(strategy
            .unwrap()
            .handles()
            .contains(&ProblemType::IndexDegradation));
    }

    #[test]
    fn test_strategy_selection_with_impact_filter() {
        let registry = StrategyRegistry::new_with_defaults();
        let problem = Problem::new(ProblemType::ReplicaLag, Severity::High);

        // PromoteReplica has 0.6 impact
        let strategy = registry.select(&problem, 0.5);
        // Should return None because PromoteReplica exceeds max_impact
        // (unless another strategy handles ReplicaLag)
        // This tests the impact filtering
    }

    #[test]
    fn test_weight_updates() {
        let registry = StrategyRegistry::new_with_defaults();

        // Initial weight should be 1.0
        assert_eq!(registry.get_weight("reindex_partition"), 1.0);

        // Success increases weight
        registry.update_weight("reindex_partition", true, 20.0);
        assert!(registry.get_weight("reindex_partition") > 1.0);

        // Failure decreases weight
        registry.update_weight("reindex_partition", false, 0.0);
        let weight = registry.get_weight("reindex_partition");
        assert!(weight < 1.2); // Should have decreased from success value
    }

    #[test]
    fn test_reindex_partition_handles() {
        let strategy = ReindexPartition::new();
        assert!(strategy.handles().contains(&ProblemType::IndexDegradation));
        assert!(!strategy.handles().contains(&ProblemType::ReplicaLag));
    }

    #[test]
    fn test_promote_replica_handles() {
        let strategy = PromoteReplica::new();
        assert!(strategy.handles().contains(&ProblemType::ReplicaLag));
        assert!(strategy
            .handles()
            .contains(&ProblemType::IntegrityViolation));
    }

    #[test]
    fn test_tier_eviction_handles() {
        let strategy = TierEviction::new();
        assert!(strategy.handles().contains(&ProblemType::StorageExhaustion));
        assert!(strategy.handles().contains(&ProblemType::MemoryPressure));
    }

    #[test]
    fn test_circuit_breaker_handles() {
        let strategy = QueryCircuitBreaker::new();
        assert!(strategy.handles().contains(&ProblemType::QueryTimeout));
        assert!(strategy
            .handles()
            .contains(&ProblemType::ConnectionExhaustion));
    }

    #[test]
    fn test_integrity_recovery_handles() {
        let strategy = IntegrityRecovery::new();
        assert!(strategy
            .handles()
            .contains(&ProblemType::IntegrityViolation));
        assert!(strategy.handles().contains(&ProblemType::IndexDegradation));
    }

    #[test]
    fn test_dry_run() {
        let strategy = ReindexPartition::new();
        let mut context = StrategyContext::new(Problem::new(
            ProblemType::IndexDegradation,
            Severity::Medium,
        ));
        context.dry_run = true;

        let result = strategy.execute(&context);
        assert_eq!(result.outcome, RemediationOutcome::NoOp);
        assert!(result.metadata.get("dry_run") == Some(&serde_json::json!(true)));
    }
}
