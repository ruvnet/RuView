//! Worker Lifecycle Management
//!
//! Provides dynamic worker spawning, graceful shutdown, health monitoring,
//! and automatic restart capabilities.
//!
//! # Worker Lifecycle States
//!
//! ```text
//! +----------+     +----------+     +---------+     +----------+
//! |  Created | --> | Starting | --> | Running | --> | Stopping |
//! +----------+     +----------+     +---------+     +----------+
//!                        |               |               |
//!                        v               v               v
//!                   +--------+      +--------+      +--------+
//!                   | Failed |      | Paused |      | Stopped|
//!                   +--------+      +--------+      +--------+
//! ```

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{get_worker_registry, WorkerType};

// ============================================================================
// Worker Status
// ============================================================================

/// Current status of a worker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerStatus {
    /// Worker has been created but not started
    Created,
    /// Worker is starting up
    Starting,
    /// Worker is running normally
    Running,
    /// Worker is paused (not processing)
    Paused,
    /// Worker is shutting down
    Stopping,
    /// Worker has stopped
    Stopped,
    /// Worker failed to start or crashed
    Failed,
}

impl std::fmt::Display for WorkerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerStatus::Created => write!(f, "created"),
            WorkerStatus::Starting => write!(f, "starting"),
            WorkerStatus::Running => write!(f, "running"),
            WorkerStatus::Paused => write!(f, "paused"),
            WorkerStatus::Stopping => write!(f, "stopping"),
            WorkerStatus::Stopped => write!(f, "stopped"),
            WorkerStatus::Failed => write!(f, "failed"),
        }
    }
}

// ============================================================================
// Worker Handle
// ============================================================================

/// Handle to a background worker
#[derive(Debug, Clone)]
pub struct WorkerHandle {
    /// Unique worker ID
    pub id: u64,
    /// Process ID (if spawned)
    pub pid: i32,
    /// Worker type
    pub worker_type: WorkerType,
    /// Current status
    pub status: WorkerStatus,
    /// Start timestamp (epoch seconds)
    pub started_at: u64,
    /// Last activity timestamp (epoch seconds)
    pub last_activity: u64,
}

impl WorkerHandle {
    /// Create a new worker handle
    pub fn new(id: u64, worker_type: WorkerType) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id,
            pid: 0,
            worker_type,
            status: WorkerStatus::Created,
            started_at: now,
            last_activity: now,
        }
    }

    /// Check if worker is alive
    pub fn is_alive(&self) -> bool {
        matches!(
            self.status,
            WorkerStatus::Running | WorkerStatus::Paused | WorkerStatus::Starting
        )
    }

    /// Check if worker can accept work
    pub fn can_accept_work(&self) -> bool {
        self.status == WorkerStatus::Running
    }
}

// ============================================================================
// Worker Lifecycle Configuration
// ============================================================================

/// Configuration for worker lifecycle management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleConfig {
    /// Maximum time to wait for worker startup (seconds)
    pub startup_timeout_secs: u64,
    /// Maximum time to wait for graceful shutdown (seconds)
    pub shutdown_timeout_secs: u64,
    /// Health check interval (seconds)
    pub health_check_interval_secs: u64,
    /// Maximum consecutive health check failures before restart
    pub max_health_failures: u32,
    /// Restart delay after crash (seconds)
    pub restart_delay_secs: u64,
    /// Maximum restarts within window
    pub max_restarts: u32,
    /// Restart window (seconds)
    pub restart_window_secs: u64,
    /// Enable automatic restart on crash
    pub auto_restart: bool,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            startup_timeout_secs: 30,
            shutdown_timeout_secs: 30,
            health_check_interval_secs: 60,
            max_health_failures: 3,
            restart_delay_secs: 5,
            max_restarts: 5,
            restart_window_secs: 300,
            auto_restart: true,
        }
    }
}

// ============================================================================
// Worker Lifecycle Manager
// ============================================================================

/// Manages the lifecycle of background workers
pub struct WorkerLifecycle {
    /// Configuration
    config: RwLock<LifecycleConfig>,
    /// Next worker ID
    next_id: AtomicU64,
    /// Shutdown flag
    shutdown_requested: AtomicBool,
    /// Active worker handles
    handles: RwLock<std::collections::HashMap<u64, WorkerHandle>>,
    /// Restart counters (worker_id -> (count, first_restart_time))
    restart_counters: RwLock<std::collections::HashMap<u64, (u32, u64)>>,
}

impl WorkerLifecycle {
    /// Create a new lifecycle manager
    pub fn new() -> Self {
        Self {
            config: RwLock::new(LifecycleConfig::default()),
            next_id: AtomicU64::new(1),
            shutdown_requested: AtomicBool::new(false),
            handles: RwLock::new(std::collections::HashMap::new()),
            restart_counters: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Get configuration
    pub fn config(&self) -> LifecycleConfig {
        self.config.read().clone()
    }

    /// Update configuration
    pub fn set_config(&self, config: LifecycleConfig) {
        *self.config.write() = config;
    }

    /// Generate next worker ID
    pub fn next_worker_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Register a worker handle
    pub fn register(&self, handle: WorkerHandle) {
        let id = handle.id;
        let worker_type = handle.worker_type;
        self.handles.write().insert(id, handle.clone());
        get_worker_registry().register(worker_type, handle);
    }

    /// Update worker status
    pub fn update_status(&self, worker_id: u64, status: WorkerStatus) {
        if let Some(handle) = self.handles.write().get_mut(&worker_id) {
            handle.status = status;
            handle.last_activity = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
        }
    }

    /// Get worker handle
    pub fn get_handle(&self, worker_id: u64) -> Option<WorkerHandle> {
        self.handles.read().get(&worker_id).cloned()
    }

    /// Get all handles
    pub fn get_all_handles(&self) -> Vec<WorkerHandle> {
        self.handles.read().values().cloned().collect()
    }

    /// Remove worker handle
    pub fn unregister(&self, worker_id: u64) {
        if let Some(handle) = self.handles.write().remove(&worker_id) {
            get_worker_registry().unregister(handle.worker_type, worker_id);
        }
    }

    /// Check if shutdown is requested
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst)
    }

    /// Request shutdown
    pub fn request_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
    }

    /// Check if worker can restart
    pub fn can_restart(&self, worker_id: u64) -> bool {
        let config = self.config.read();
        if !config.auto_restart {
            return false;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut counters = self.restart_counters.write();
        let (count, first_restart) = counters.entry(worker_id).or_insert((0, now));

        // Reset counter if outside window
        if now - *first_restart > config.restart_window_secs {
            *count = 0;
            *first_restart = now;
        }

        *count < config.max_restarts
    }

    /// Record a restart
    pub fn record_restart(&self, worker_id: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut counters = self.restart_counters.write();
        let (count, first_restart) = counters.entry(worker_id).or_insert((0, now));

        let config = self.config.read();
        if now - *first_restart > config.restart_window_secs {
            *count = 1;
            *first_restart = now;
        } else {
            *count += 1;
        }
    }
}

impl Default for WorkerLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

// Global lifecycle manager
static LIFECYCLE_MANAGER: OnceLock<WorkerLifecycle> = OnceLock::new();

/// Get the global lifecycle manager
pub fn get_lifecycle_manager() -> &'static WorkerLifecycle {
    LIFECYCLE_MANAGER.get_or_init(WorkerLifecycle::new)
}

// ============================================================================
// Worker Spawning
// ============================================================================

/// Spawn a new background worker
pub fn spawn_worker(worker_type: WorkerType) -> Result<WorkerHandle, String> {
    let lifecycle = get_lifecycle_manager();

    // Check if shutdown is requested
    if lifecycle.is_shutdown_requested() {
        return Err("System is shutting down".to_string());
    }

    // Generate worker ID
    let worker_id = lifecycle.next_worker_id();

    // Create handle
    let mut handle = WorkerHandle::new(worker_id, worker_type);
    handle.status = WorkerStatus::Starting;

    // Register the worker
    lifecycle.register(handle.clone());

    // Spawn the actual background worker
    match spawn_pg_background_worker(worker_id, worker_type) {
        Ok(pid) => {
            handle.pid = pid;
            handle.status = WorkerStatus::Running;
            lifecycle.update_status(worker_id, WorkerStatus::Running);

            pgrx::log!(
                "Spawned {} worker {} (PID: {})",
                worker_type,
                worker_id,
                pid
            );

            Ok(handle)
        }
        Err(e) => {
            lifecycle.update_status(worker_id, WorkerStatus::Failed);
            Err(format!("Failed to spawn worker: {}", e))
        }
    }
}

/// Spawn a PostgreSQL background worker
fn spawn_pg_background_worker(worker_id: u64, worker_type: WorkerType) -> Result<i32, String> {
    // In production, this would use pg_sys::RegisterDynamicBackgroundWorker
    // For now, return a mock PID

    let name = format!("ruvector {} [{}]", worker_type, worker_id);

    // Register with PostgreSQL
    // NOTE: Actual implementation would look like:
    //
    // unsafe {
    //     let mut worker = pg_sys::BackgroundWorker::default();
    //
    //     // Copy name
    //     let name_bytes = name.as_bytes();
    //     let bgw_name = &mut worker.bgw_name as *mut i8;
    //     std::ptr::copy_nonoverlapping(
    //         name_bytes.as_ptr() as *const i8,
    //         bgw_name,
    //         name_bytes.len().min(BGW_MAXLEN - 1),
    //     );
    //
    //     worker.bgw_flags = pg_sys::BGWORKER_SHMEM_ACCESS
    //                      | pg_sys::BGWORKER_BACKEND_DATABASE_CONNECTION;
    //     worker.bgw_start_time = pg_sys::BgWorkerStart_RecoveryFinished;
    //     worker.bgw_restart_time = match worker_type {
    //         WorkerType::Engine => 10,
    //         WorkerType::Maintenance => 60,
    //         WorkerType::GnnTraining => pg_sys::BGW_NEVER_RESTART as i32,
    //         WorkerType::Integrity => 10,
    //     };
    //     worker.bgw_main = Some(match worker_type {
    //         WorkerType::Engine => super::engine::ruvector_engine_worker_main,
    //         WorkerType::Maintenance => super::maintenance::ruvector_maintenance_worker_main,
    //         WorkerType::GnnTraining => super::gnn::ruvector_gnn_training_worker_main,
    //         WorkerType::Integrity => super::integrity::ruvector_integrity_worker_main,
    //     });
    //     worker.bgw_main_arg = pg_sys::Datum::from(worker_id);
    //
    //     let mut handle: *mut pg_sys::BackgroundWorkerHandle = std::ptr::null_mut();
    //     if pg_sys::RegisterDynamicBackgroundWorker(&mut worker, &mut handle) {
    //         let mut pid: pg_sys::pid_t = 0;
    //         pg_sys::WaitForBackgroundWorkerStartup(handle, &mut pid);
    //         Ok(pid)
    //     } else {
    //         Err("Failed to register background worker".to_string())
    //     }
    // }

    // Mock implementation for testing
    Ok((worker_id as i32) + 10000)
}

// ============================================================================
// Worker Shutdown
// ============================================================================

/// Shutdown a specific worker
pub fn shutdown_worker(worker_id: u64) -> Result<(), String> {
    let lifecycle = get_lifecycle_manager();

    let handle = lifecycle
        .get_handle(worker_id)
        .ok_or_else(|| format!("Worker {} not found", worker_id))?;

    if !handle.is_alive() {
        return Err(format!("Worker {} is not alive", worker_id));
    }

    lifecycle.update_status(worker_id, WorkerStatus::Stopping);

    // Signal worker to stop
    signal_worker_stop(handle.pid)?;

    // Wait for graceful shutdown
    let config = lifecycle.config();
    let deadline = std::time::Instant::now() + Duration::from_secs(config.shutdown_timeout_secs);

    while std::time::Instant::now() < deadline {
        if let Some(h) = lifecycle.get_handle(worker_id) {
            if h.status == WorkerStatus::Stopped {
                lifecycle.unregister(worker_id);
                pgrx::log!("Worker {} stopped gracefully", worker_id);
                return Ok(());
            }
        } else {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // Force kill if still running
    force_kill_worker(handle.pid)?;
    lifecycle.update_status(worker_id, WorkerStatus::Stopped);
    lifecycle.unregister(worker_id);

    pgrx::warning!("Worker {} forcibly terminated", worker_id);
    Ok(())
}

/// Signal a worker to stop
fn signal_worker_stop(pid: i32) -> Result<(), String> {
    // In production, use pg_sys::SetLatch or signals
    // For now, this is a mock
    pgrx::debug1!("Signaling worker {} to stop", pid);
    Ok(())
}

/// Force kill a worker
fn force_kill_worker(pid: i32) -> Result<(), String> {
    // In production, use SIGKILL
    pgrx::debug1!("Force killing worker {}", pid);
    Ok(())
}

// ============================================================================
// Health Monitoring
// ============================================================================

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Worker ID
    pub worker_id: u64,
    /// Whether worker is healthy
    pub healthy: bool,
    /// Last response time (ms)
    pub response_time_ms: u64,
    /// Error message if unhealthy
    pub error: Option<String>,
}

/// Perform health check on a worker
pub fn check_worker_health(worker_id: u64) -> HealthCheckResult {
    let lifecycle = get_lifecycle_manager();

    let handle = match lifecycle.get_handle(worker_id) {
        Some(h) => h,
        None => {
            return HealthCheckResult {
                worker_id,
                healthy: false,
                response_time_ms: 0,
                error: Some("Worker not found".to_string()),
            };
        }
    };

    if !handle.is_alive() {
        return HealthCheckResult {
            worker_id,
            healthy: false,
            response_time_ms: 0,
            error: Some(format!("Worker status: {}", handle.status)),
        };
    }

    // Check if worker is responding
    let start = std::time::Instant::now();
    let responsive = ping_worker(handle.pid);
    let response_time = start.elapsed().as_millis() as u64;

    if responsive {
        // Update last activity
        lifecycle.update_status(worker_id, WorkerStatus::Running);
    }

    HealthCheckResult {
        worker_id,
        healthy: responsive,
        response_time_ms: response_time,
        error: if responsive {
            None
        } else {
            Some("Worker not responding".to_string())
        },
    }
}

/// Ping a worker to check if it's responsive
fn ping_worker(pid: i32) -> bool {
    // In production, send a ping through shared memory and wait for response
    // For now, always return true for mock
    true
}

/// Run health checks on all workers
pub fn run_health_checks() -> Vec<HealthCheckResult> {
    let lifecycle = get_lifecycle_manager();
    let handles = lifecycle.get_all_handles();

    handles
        .iter()
        .filter(|h| h.is_alive())
        .map(|h| check_worker_health(h.id))
        .collect()
}

// ============================================================================
// Automatic Restart
// ============================================================================

/// Handle worker failure with automatic restart if configured
pub fn handle_worker_failure(worker_id: u64, error: &str) {
    let lifecycle = get_lifecycle_manager();

    pgrx::warning!("Worker {} failed: {}", worker_id, error);

    if let Some(handle) = lifecycle.get_handle(worker_id) {
        lifecycle.update_status(worker_id, WorkerStatus::Failed);

        if lifecycle.can_restart(worker_id) {
            lifecycle.record_restart(worker_id);
            let worker_type = handle.worker_type;
            let config = lifecycle.config();

            // Schedule restart
            pgrx::log!(
                "Scheduling restart for {} worker {} in {} seconds",
                worker_type,
                worker_id,
                config.restart_delay_secs
            );

            // In production, use a timer or background scheduler
            std::thread::sleep(Duration::from_secs(config.restart_delay_secs));

            match spawn_worker(worker_type) {
                Ok(new_handle) => {
                    pgrx::log!(
                        "Worker {} restarted as {} (PID: {})",
                        worker_id,
                        new_handle.id,
                        new_handle.pid
                    );
                }
                Err(e) => {
                    pgrx::warning!("Failed to restart worker {}: {}", worker_id, e);
                }
            }
        } else {
            pgrx::warning!(
                "Worker {} has exceeded maximum restarts, not restarting",
                worker_id
            );
        }

        lifecycle.unregister(worker_id);
    }
}

// ============================================================================
// Worker Pause/Resume
// ============================================================================

/// Pause a worker
pub fn pause_worker(worker_id: u64) -> Result<(), String> {
    let lifecycle = get_lifecycle_manager();

    let handle = lifecycle
        .get_handle(worker_id)
        .ok_or_else(|| format!("Worker {} not found", worker_id))?;

    if handle.status != WorkerStatus::Running {
        return Err(format!("Worker {} is not running", worker_id));
    }

    lifecycle.update_status(worker_id, WorkerStatus::Paused);
    pgrx::log!("Worker {} paused", worker_id);
    Ok(())
}

/// Resume a paused worker
pub fn resume_worker(worker_id: u64) -> Result<(), String> {
    let lifecycle = get_lifecycle_manager();

    let handle = lifecycle
        .get_handle(worker_id)
        .ok_or_else(|| format!("Worker {} not found", worker_id))?;

    if handle.status != WorkerStatus::Paused {
        return Err(format!("Worker {} is not paused", worker_id));
    }

    lifecycle.update_status(worker_id, WorkerStatus::Running);
    pgrx::log!("Worker {} resumed", worker_id);
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_status_display() {
        assert_eq!(WorkerStatus::Running.to_string(), "running");
        assert_eq!(WorkerStatus::Stopped.to_string(), "stopped");
    }

    #[test]
    fn test_worker_handle() {
        let handle = WorkerHandle::new(1, WorkerType::Engine);
        assert!(!handle.is_alive()); // Created but not running
        assert!(!handle.can_accept_work());
    }

    #[test]
    fn test_lifecycle_manager() {
        let lifecycle = WorkerLifecycle::new();

        let id1 = lifecycle.next_worker_id();
        let id2 = lifecycle.next_worker_id();
        assert!(id2 > id1);

        let mut handle = WorkerHandle::new(id1, WorkerType::Engine);
        handle.status = WorkerStatus::Running;
        lifecycle.register(handle);

        assert!(lifecycle.get_handle(id1).is_some());
        assert!(lifecycle.get_handle(id2).is_none());

        lifecycle.unregister(id1);
        assert!(lifecycle.get_handle(id1).is_none());
    }

    #[test]
    fn test_restart_limiting() {
        let lifecycle = WorkerLifecycle::new();

        // Configure with max 2 restarts
        let config = LifecycleConfig {
            max_restarts: 2,
            restart_window_secs: 300,
            ..Default::default()
        };
        lifecycle.set_config(config);

        let worker_id = 1;

        assert!(lifecycle.can_restart(worker_id));
        lifecycle.record_restart(worker_id);

        assert!(lifecycle.can_restart(worker_id));
        lifecycle.record_restart(worker_id);

        assert!(!lifecycle.can_restart(worker_id)); // Exceeded max
    }

    #[test]
    fn test_health_check_result() {
        let result = HealthCheckResult {
            worker_id: 1,
            healthy: true,
            response_time_ms: 5,
            error: None,
        };

        assert!(result.healthy);
        assert!(result.error.is_none());
    }
}
