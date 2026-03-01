//! Background Worker for Self-Healing Engine
//!
//! Provides continuous health monitoring and async remediation:
//! - Periodic health checks
//! - Automatic problem detection
//! - Async remediation execution
//! - Integration with integrity control plane

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use super::detector::ProblemDetector;
use super::engine::HealingOutcome;
use super::get_healing_engine;

// ============================================================================
// Worker Configuration
// ============================================================================

/// Configuration for the healing background worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealingWorkerConfig {
    /// Health check interval
    pub check_interval: Duration,
    /// Whether to auto-remediate detected problems
    pub auto_remediate: bool,
    /// Minimum severity to auto-remediate
    pub min_auto_severity: u8, // 0=Info, 1=Low, 2=Medium, 3=High, 4=Critical
    /// Maximum concurrent remediations
    pub max_concurrent: usize,
    /// Whether to log health status
    pub log_status: bool,
    /// Enable metrics collection
    pub collect_metrics: bool,
}

impl Default for HealingWorkerConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(60),
            auto_remediate: true,
            min_auto_severity: 2, // Medium and above
            max_concurrent: 2,
            log_status: true,
            collect_metrics: true,
        }
    }
}

// ============================================================================
// Worker State
// ============================================================================

/// State of the healing background worker
pub struct HealingWorkerState {
    /// Configuration
    config: RwLock<HealingWorkerConfig>,
    /// Whether worker is running
    running: AtomicBool,
    /// Last health check timestamp
    last_check: AtomicU64,
    /// Total health checks performed
    checks_completed: AtomicU64,
    /// Total problems detected
    problems_detected: AtomicU64,
    /// Total remediations triggered
    remediations_triggered: AtomicU64,
    /// Recent health statuses
    recent_statuses: RwLock<Vec<HealthCheckResult>>,
}

impl HealingWorkerState {
    /// Create new worker state
    pub fn new(config: HealingWorkerConfig) -> Self {
        Self {
            config: RwLock::new(config),
            running: AtomicBool::new(false),
            last_check: AtomicU64::new(0),
            checks_completed: AtomicU64::new(0),
            problems_detected: AtomicU64::new(0),
            remediations_triggered: AtomicU64::new(0),
            recent_statuses: RwLock::new(Vec::new()),
        }
    }

    /// Check if worker is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Start worker
    pub fn start(&self) {
        self.running.store(true, Ordering::SeqCst);
    }

    /// Stop worker
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Get configuration
    pub fn get_config(&self) -> HealingWorkerConfig {
        self.config.read().clone()
    }

    /// Update configuration
    pub fn set_config(&self, config: HealingWorkerConfig) {
        *self.config.write() = config;
    }

    /// Record a health check
    pub fn record_check(&self, result: HealthCheckResult) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.last_check.store(now, Ordering::SeqCst);
        self.checks_completed.fetch_add(1, Ordering::SeqCst);
        self.problems_detected
            .fetch_add(result.problems_found as u64, Ordering::SeqCst);
        self.remediations_triggered
            .fetch_add(result.remediations_triggered as u64, Ordering::SeqCst);

        // Keep last 100 statuses
        let mut statuses = self.recent_statuses.write();
        statuses.push(result);
        while statuses.len() > 100 {
            statuses.remove(0);
        }
    }

    /// Get worker statistics
    pub fn get_stats(&self) -> WorkerStats {
        WorkerStats {
            running: self.running.load(Ordering::SeqCst),
            last_check: self.last_check.load(Ordering::SeqCst),
            checks_completed: self.checks_completed.load(Ordering::SeqCst),
            problems_detected: self.problems_detected.load(Ordering::SeqCst),
            remediations_triggered: self.remediations_triggered.load(Ordering::SeqCst),
        }
    }

    /// Get recent health check results
    pub fn get_recent_checks(&self, limit: usize) -> Vec<HealthCheckResult> {
        let statuses = self.recent_statuses.read();
        statuses.iter().rev().take(limit).cloned().collect()
    }
}

/// Worker statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStats {
    pub running: bool,
    pub last_check: u64,
    pub checks_completed: u64,
    pub problems_detected: u64,
    pub remediations_triggered: u64,
}

impl WorkerStats {
    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "running": self.running,
            "last_check": self.last_check,
            "checks_completed": self.checks_completed,
            "problems_detected": self.problems_detected,
            "remediations_triggered": self.remediations_triggered,
        })
    }
}

// ============================================================================
// Health Check Result
// ============================================================================

/// Result of a health check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// Timestamp of check
    pub timestamp: u64,
    /// Whether system is healthy
    pub healthy: bool,
    /// Number of problems found
    pub problems_found: usize,
    /// Number of remediations triggered
    pub remediations_triggered: usize,
    /// Remediation outcomes
    pub outcomes: Vec<serde_json::Value>,
    /// Metrics collected
    pub metrics: Option<serde_json::Value>,
    /// Duration of check in milliseconds
    pub duration_ms: u64,
}

impl HealthCheckResult {
    /// Create a healthy result
    pub fn healthy() -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            healthy: true,
            problems_found: 0,
            remediations_triggered: 0,
            outcomes: vec![],
            metrics: None,
            duration_ms: 0,
        }
    }

    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "timestamp": self.timestamp,
            "healthy": self.healthy,
            "problems_found": self.problems_found,
            "remediations_triggered": self.remediations_triggered,
            "outcomes": self.outcomes,
            "duration_ms": self.duration_ms,
        })
    }
}

// ============================================================================
// Healing Worker
// ============================================================================

/// Background worker for continuous health monitoring
pub struct HealingWorker {
    /// Worker state
    state: Arc<HealingWorkerState>,
    /// Problem detector
    detector: ProblemDetector,
}

impl HealingWorker {
    /// Create new healing worker
    pub fn new(config: HealingWorkerConfig) -> Self {
        Self {
            state: Arc::new(HealingWorkerState::new(config)),
            detector: ProblemDetector::new(),
        }
    }

    /// Create with shared state
    pub fn with_state(state: Arc<HealingWorkerState>) -> Self {
        Self {
            state,
            detector: ProblemDetector::new(),
        }
    }

    /// Get worker state
    pub fn state(&self) -> &Arc<HealingWorkerState> {
        &self.state
    }

    /// Perform one health check cycle
    pub fn check_health(&self) -> HealthCheckResult {
        let start = std::time::Instant::now();
        let config = self.state.get_config();

        // Collect metrics
        let metrics = self.detector.collect_metrics();

        // Detect problems
        let problems = self.detector.detect_problems(&metrics);
        let problems_found = problems.len();

        if config.log_status {
            if problems_found > 0 {
                pgrx::log!("Healing worker: {} problems detected", problems_found);
            } else {
                pgrx::debug1!("Healing worker: no problems detected");
            }
        }

        let mut remediations_triggered = 0;
        let mut outcomes = Vec::new();

        // Auto-remediate if enabled
        if config.auto_remediate && problems_found > 0 {
            let engine = get_healing_engine();
            let engine_lock = engine.read();

            for problem in &problems {
                // Check severity threshold
                if problem.severity.value() < config.min_auto_severity {
                    continue;
                }

                // Attempt remediation
                let outcome = engine_lock.remediation.heal(problem);
                outcomes.push(outcome.to_json());

                if matches!(outcome, HealingOutcome::Completed { .. }) {
                    remediations_triggered += 1;
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        let result = HealthCheckResult {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            healthy: problems_found == 0,
            problems_found,
            remediations_triggered,
            outcomes,
            metrics: if config.collect_metrics {
                Some(metrics.to_json())
            } else {
                None
            },
            duration_ms,
        };

        self.state.record_check(result.clone());

        result
    }

    /// Run the worker loop (blocking)
    pub fn run(&self) {
        self.state.start();
        pgrx::log!("Healing background worker started");

        while self.state.is_running() {
            // Perform health check
            let _result = self.check_health();

            // Sleep until next check
            let interval = self.state.get_config().check_interval;

            // Use PostgreSQL's WaitLatch for interruptible sleep
            self.wait_for_interval(interval);
        }

        pgrx::log!("Healing background worker stopped");
    }

    /// Wait for interval with interruption support
    fn wait_for_interval(&self, interval: Duration) {
        // Use simple thread sleep which works in all contexts.
        // In production as a full background worker, one would use
        // PostgreSQL's WaitLatch for interruptible sleep.
        std::thread::sleep(interval);
    }

    /// Stop the worker
    pub fn stop(&self) {
        self.state.stop();
    }
}

// ============================================================================
// Background Worker Entry Point
// ============================================================================

/// PostgreSQL background worker entry point
#[pgrx::pg_guard]
pub extern "C" fn healing_bgworker_main(_arg: pgrx::pg_sys::Datum) {
    pgrx::log!("RuVector healing background worker starting");

    let config = HealingWorkerConfig::default();
    let worker = HealingWorker::new(config);

    worker.run();
}

/// Register the background worker with PostgreSQL
pub fn register_healing_worker() {
    pgrx::log!("Registering RuVector healing background worker");

    // In production, use pg_sys::RegisterBackgroundWorker
    // This is a placeholder for now
    //
    // unsafe {
    //     let mut worker = pg_sys::BackgroundWorker::default();
    //     // Configure worker...
    //     pg_sys::RegisterBackgroundWorker(&mut worker);
    // }
}

// ============================================================================
// SQL Functions for Worker Control
// ============================================================================

use pgrx::prelude::*;

/// Start the healing background worker
#[pg_extern]
pub fn ruvector_healing_worker_start() -> bool {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    if engine_lock.worker_state.is_running() {
        pgrx::warning!("Healing worker is already running");
        return false;
    }

    // In production, would launch actual background worker
    engine_lock.worker_state.start();
    pgrx::log!("Healing worker started");
    true
}

/// Stop the healing background worker
#[pg_extern]
pub fn ruvector_healing_worker_stop() -> bool {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    if !engine_lock.worker_state.is_running() {
        pgrx::warning!("Healing worker is not running");
        return false;
    }

    engine_lock.worker_state.stop();
    pgrx::log!("Healing worker stopped");
    true
}

/// Get healing worker status
#[pg_extern]
pub fn ruvector_healing_worker_status() -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    let stats = engine_lock.worker_state.get_stats();
    let config = engine_lock.worker_state.get_config();

    let status = serde_json::json!({
        "stats": stats.to_json(),
        "config": {
            "check_interval_secs": config.check_interval.as_secs(),
            "auto_remediate": config.auto_remediate,
            "min_auto_severity": config.min_auto_severity,
            "max_concurrent": config.max_concurrent,
        }
    });

    pgrx::JsonB(status)
}

/// Configure the healing worker
#[pg_extern]
pub fn ruvector_healing_worker_config(
    check_interval_secs: Option<i32>,
    auto_remediate: Option<bool>,
    min_auto_severity: Option<i32>,
) -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    let mut config = engine_lock.worker_state.get_config();

    if let Some(interval) = check_interval_secs {
        if interval > 0 {
            config.check_interval = Duration::from_secs(interval as u64);
        }
    }

    if let Some(auto_rem) = auto_remediate {
        config.auto_remediate = auto_rem;
    }

    if let Some(severity) = min_auto_severity {
        if severity >= 0 && severity <= 4 {
            config.min_auto_severity = severity as u8;
        }
    }

    engine_lock.worker_state.set_config(config.clone());

    pgrx::JsonB(serde_json::json!({
        "status": "updated",
        "config": {
            "check_interval_secs": config.check_interval.as_secs(),
            "auto_remediate": config.auto_remediate,
            "min_auto_severity": config.min_auto_severity,
        }
    }))
}

/// Manually trigger a health check
#[pg_extern]
pub fn ruvector_healing_check_now() -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    let detector = ProblemDetector::new();
    let start = std::time::Instant::now();

    let metrics = detector.collect_metrics();
    let problems = detector.detect_problems(&metrics);

    let mut outcomes = Vec::new();
    for problem in &problems {
        let outcome = engine_lock.remediation.heal(problem);
        outcomes.push(outcome.to_json());
    }

    let result = serde_json::json!({
        "healthy": problems.is_empty(),
        "problems_found": problems.len(),
        "problems": problems.iter().map(|p| p.to_json()).collect::<Vec<_>>(),
        "outcomes": outcomes,
        "metrics": metrics.to_json(),
        "duration_ms": start.elapsed().as_millis() as u64,
    });

    pgrx::JsonB(result)
}

/// Get recent health check results
#[pg_extern]
pub fn ruvector_healing_recent_checks(limit: default!(i32, 10)) -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    let checks = engine_lock.worker_state.get_recent_checks(limit as usize);

    pgrx::JsonB(serde_json::json!({
        "checks": checks.iter().map(|c| c.to_json()).collect::<Vec<_>>(),
        "count": checks.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_state() {
        let state = HealingWorkerState::new(HealingWorkerConfig::default());

        assert!(!state.is_running());
        state.start();
        assert!(state.is_running());
        state.stop();
        assert!(!state.is_running());
    }

    #[test]
    fn test_health_check_result() {
        let result = HealthCheckResult::healthy();
        assert!(result.healthy);
        assert_eq!(result.problems_found, 0);
    }

    #[test]
    fn test_worker_config() {
        let config = HealingWorkerConfig::default();
        assert!(config.auto_remediate);
        assert_eq!(config.min_auto_severity, 2);
    }

    #[test]
    fn test_state_recording() {
        let state = HealingWorkerState::new(HealingWorkerConfig::default());

        let result = HealthCheckResult {
            timestamp: 12345,
            healthy: false,
            problems_found: 2,
            remediations_triggered: 1,
            outcomes: vec![],
            metrics: None,
            duration_ms: 100,
        };

        state.record_check(result);

        let stats = state.get_stats();
        assert_eq!(stats.checks_completed, 1);
        assert_eq!(stats.problems_detected, 2);
        assert_eq!(stats.remediations_triggered, 1);
    }

    #[test]
    fn test_recent_checks() {
        let state = HealingWorkerState::new(HealingWorkerConfig::default());

        for i in 0..5 {
            state.record_check(HealthCheckResult {
                timestamp: i,
                healthy: true,
                problems_found: 0,
                remediations_triggered: 0,
                outcomes: vec![],
                metrics: None,
                duration_ms: 10,
            });
        }

        let recent = state.get_recent_checks(3);
        assert_eq!(recent.len(), 3);
        // Most recent first
        assert_eq!(recent[0].timestamp, 4);
    }

    #[test]
    fn test_worker_creation() {
        let worker = HealingWorker::new(HealingWorkerConfig::default());
        assert!(!worker.state().is_running());
    }
}
