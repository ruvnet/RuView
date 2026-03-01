//! Background Workers for RuVector Postgres v2
//!
//! This module provides specialized background workers for:
//! - Engine coordination (query routing, load balancing)
//! - Index maintenance (compaction, cleanup, stats)
//! - GNN training (incremental model updates)
//! - Integrity monitoring (mincut recomputation)
//!
//! # Architecture
//!
//! ```text
//! +------------------------------------------------------------------+
//! |                     PostgreSQL Server                            |
//! +------------------------------------------------------------------+
//! |                                                                  |
//! |  +------------------------+  +------------------------+          |
//! |  |   Engine Worker (1)    |  |  Maintenance Worker    |          |
//! |  |  - Per database        |  |  - Per server          |          |
//! |  |  - Long-lived          |  |  - Periodic            |          |
//! |  +------------------------+  +------------------------+          |
//! |                                                                  |
//! |  +------------------------+  +------------------------+          |
//! |  |   GNN Training Worker  |  |  Integrity Worker      |          |
//! |  |  - On-demand           |  |  - Per database        |          |
//! |  |  - Resource-intensive  |  |  - Continuous          |          |
//! |  +------------------------+  +------------------------+          |
//! |                                                                  |
//! +------------------------------------------------------------------+
//! |                     Shared Memory Region                         |
//! |  +------------------+  +------------------+  +------------------+ |
//! |  | Work Queues      |  | Index State      |  | Integrity State  | |
//! |  +------------------+  +------------------+  +------------------+ |
//! +------------------------------------------------------------------+
//! ```

pub mod engine;
pub mod gnn;
pub mod integrity;
pub mod ipc;
pub mod lifecycle;
pub mod maintenance;
pub mod queue;

// Re-exports
pub use engine::{EngineWorker, EngineWorkerConfig, SearchResult};
pub use gnn::{
    get_gnn_worker, set_gnn_config, GnnModel, GnnTrainingConfig, GnnTrainingRequest,
    GnnTrainingWorker,
};
pub use integrity::{
    get_integrity_worker, set_integrity_config, IntegrityConfig, IntegrityState,
    IntegrityStateType, IntegrityWorker,
};
pub use ipc::{Operation, SearchRequest};
pub use ipc::{PayloadRef, SharedMemory, SharedMemoryLayout, WorkItem, WorkResult};
pub use lifecycle::{shutdown_worker, spawn_worker, WorkerHandle, WorkerLifecycle, WorkerStatus};
pub use maintenance::{MaintenanceConfig, MaintenanceWorker, TierCandidate};
pub use queue::{QueueStats, QueueStatsSnapshot, TaskPriority, TaskQueue, TaskType};

use parking_lot::RwLock;
use pgrx::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;

// ============================================================================
// Worker Type Enumeration
// ============================================================================

/// Types of background workers supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkerType {
    /// Main engine worker for query processing
    Engine,
    /// Periodic maintenance worker
    Maintenance,
    /// On-demand GNN training worker
    GnnTraining,
    /// Continuous integrity monitoring
    Integrity,
}

impl std::fmt::Display for WorkerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerType::Engine => write!(f, "engine"),
            WorkerType::Maintenance => write!(f, "maintenance"),
            WorkerType::GnnTraining => write!(f, "gnn_training"),
            WorkerType::Integrity => write!(f, "integrity"),
        }
    }
}

impl std::str::FromStr for WorkerType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "engine" => Ok(WorkerType::Engine),
            "maintenance" => Ok(WorkerType::Maintenance),
            "gnn_training" | "gnn" | "training" => Ok(WorkerType::GnnTraining),
            "integrity" => Ok(WorkerType::Integrity),
            _ => Err(format!("Unknown worker type: {}", s)),
        }
    }
}

// ============================================================================
// Global Worker Registry
// ============================================================================

/// Global registry of active workers
pub struct WorkerRegistry {
    /// Active workers by type
    workers: RwLock<std::collections::HashMap<WorkerType, Vec<WorkerHandle>>>,
    /// Total workers spawned
    total_spawned: AtomicU64,
    /// Whether workers are enabled
    enabled: AtomicBool,
}

impl WorkerRegistry {
    /// Create a new worker registry
    pub fn new() -> Self {
        Self {
            workers: RwLock::new(std::collections::HashMap::new()),
            total_spawned: AtomicU64::new(0),
            enabled: AtomicBool::new(true),
        }
    }

    /// Register a new worker
    pub fn register(&self, worker_type: WorkerType, handle: WorkerHandle) {
        let mut workers = self.workers.write();
        workers.entry(worker_type).or_default().push(handle);
        self.total_spawned.fetch_add(1, Ordering::SeqCst);
    }

    /// Unregister a worker
    pub fn unregister(&self, worker_type: WorkerType, worker_id: u64) {
        let mut workers = self.workers.write();
        if let Some(list) = workers.get_mut(&worker_type) {
            list.retain(|h| h.id != worker_id);
        }
    }

    /// Get all workers of a type
    pub fn get_workers(&self, worker_type: WorkerType) -> Vec<WorkerHandle> {
        let workers = self.workers.read();
        workers.get(&worker_type).cloned().unwrap_or_default()
    }

    /// Get all active workers
    pub fn get_all_workers(&self) -> Vec<(WorkerType, WorkerHandle)> {
        let workers = self.workers.read();
        workers
            .iter()
            .flat_map(|(wt, handles)| handles.iter().map(|h| (*wt, h.clone())))
            .collect()
    }

    /// Get worker count by type
    pub fn count(&self, worker_type: WorkerType) -> usize {
        let workers = self.workers.read();
        workers.get(&worker_type).map(|v| v.len()).unwrap_or(0)
    }

    /// Get total worker count
    pub fn total_count(&self) -> usize {
        let workers = self.workers.read();
        workers.values().map(|v| v.len()).sum()
    }

    /// Check if workers are enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Enable/disable workers
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }
}

impl Default for WorkerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Global worker registry instance
static WORKER_REGISTRY: OnceLock<WorkerRegistry> = OnceLock::new();

/// Get the global worker registry
pub fn get_worker_registry() -> &'static WorkerRegistry {
    WORKER_REGISTRY.get_or_init(WorkerRegistry::new)
}

// ============================================================================
// Worker Initialization
// ============================================================================

/// Initialize the background worker subsystem
pub fn init_workers() {
    pgrx::log!("RuVector background worker subsystem initializing");

    // Initialize shared memory
    if let Err(e) = ipc::init_shared_memory() {
        pgrx::warning!("Failed to initialize shared memory: {}", e);
    }

    // Initialize task queues
    queue::init_task_queues();

    pgrx::log!("RuVector background worker subsystem ready");
}

/// Shutdown all background workers gracefully
pub fn shutdown_all_workers() {
    pgrx::log!("Shutting down all RuVector background workers");

    let registry = get_worker_registry();
    registry.set_enabled(false);

    // Shutdown each worker type
    for (worker_type, handle) in registry.get_all_workers() {
        if let Err(e) = lifecycle::shutdown_worker(handle.id) {
            pgrx::warning!(
                "Failed to shutdown {} worker {}: {}",
                worker_type,
                handle.id,
                e
            );
        }
    }

    pgrx::log!("All RuVector background workers stopped");
}

// ============================================================================
// SQL Functions for Worker Management
// ============================================================================

/// Get status of all workers
#[pg_extern]
pub fn ruvector_worker_status() -> pgrx::JsonB {
    let registry = get_worker_registry();
    let workers: Vec<_> = registry
        .get_all_workers()
        .iter()
        .map(|(wt, h)| {
            serde_json::json!({
                "type": wt.to_string(),
                "id": h.id,
                "pid": h.pid,
                "status": format!("{:?}", h.status),
                "started_at": h.started_at,
                "last_activity": h.last_activity,
            })
        })
        .collect();

    let status = serde_json::json!({
        "enabled": registry.is_enabled(),
        "total_count": registry.total_count(),
        "total_spawned": registry.total_spawned.load(Ordering::SeqCst),
        "workers": workers,
        "by_type": {
            "engine": registry.count(WorkerType::Engine),
            "maintenance": registry.count(WorkerType::Maintenance),
            "gnn_training": registry.count(WorkerType::GnnTraining),
            "integrity": registry.count(WorkerType::Integrity),
        }
    });

    pgrx::JsonB(status)
}

/// Spawn a new worker of the specified type
#[pg_extern]
pub fn ruvector_worker_spawn(worker_type: &str) -> pgrx::JsonB {
    let wt = match worker_type.parse::<WorkerType>() {
        Ok(t) => t,
        Err(e) => {
            return pgrx::JsonB(serde_json::json!({
                "success": false,
                "error": e,
            }));
        }
    };

    match lifecycle::spawn_worker(wt) {
        Ok(handle) => pgrx::JsonB(serde_json::json!({
            "success": true,
            "worker_id": handle.id,
            "worker_type": wt.to_string(),
            "pid": handle.pid,
        })),
        Err(e) => pgrx::JsonB(serde_json::json!({
            "success": false,
            "error": e,
        })),
    }
}

/// Configure worker settings
#[pg_extern]
pub fn ruvector_worker_configure(config: pgrx::JsonB) -> pgrx::JsonB {
    let config_value = config.0;

    // Parse and apply configuration
    let mut applied = serde_json::Map::new();

    if let Some(enabled) = config_value.get("enabled").and_then(|v| v.as_bool()) {
        get_worker_registry().set_enabled(enabled);
        applied.insert("enabled".to_string(), serde_json::Value::Bool(enabled));
    }

    if let Some(engine_config) = config_value.get("engine") {
        if let Ok(cfg) = serde_json::from_value::<EngineWorkerConfig>(engine_config.clone()) {
            engine::set_engine_config(cfg.clone());
            applied.insert("engine".to_string(), serde_json::to_value(&cfg).unwrap());
        }
    }

    if let Some(maintenance_config) = config_value.get("maintenance") {
        if let Ok(cfg) = serde_json::from_value::<MaintenanceConfig>(maintenance_config.clone()) {
            maintenance::set_maintenance_config(cfg.clone());
            applied.insert(
                "maintenance".to_string(),
                serde_json::to_value(&cfg).unwrap(),
            );
        }
    }

    if let Some(gnn_config) = config_value.get("gnn") {
        if let Ok(cfg) = serde_json::from_value::<GnnTrainingConfig>(gnn_config.clone()) {
            gnn::set_gnn_config(cfg.clone());
            applied.insert("gnn".to_string(), serde_json::to_value(&cfg).unwrap());
        }
    }

    if let Some(integrity_config) = config_value.get("integrity") {
        if let Ok(cfg) = serde_json::from_value::<IntegrityConfig>(integrity_config.clone()) {
            integrity::set_integrity_config(cfg.clone());
            applied.insert("integrity".to_string(), serde_json::to_value(&cfg).unwrap());
        }
    }

    pgrx::JsonB(serde_json::json!({
        "success": true,
        "applied": applied,
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_type_parsing() {
        assert_eq!("engine".parse::<WorkerType>().unwrap(), WorkerType::Engine);
        assert_eq!(
            "maintenance".parse::<WorkerType>().unwrap(),
            WorkerType::Maintenance
        );
        assert_eq!(
            "gnn".parse::<WorkerType>().unwrap(),
            WorkerType::GnnTraining
        );
        assert_eq!(
            "integrity".parse::<WorkerType>().unwrap(),
            WorkerType::Integrity
        );
        assert!("unknown".parse::<WorkerType>().is_err());
    }

    #[test]
    fn test_worker_type_display() {
        assert_eq!(WorkerType::Engine.to_string(), "engine");
        assert_eq!(WorkerType::Maintenance.to_string(), "maintenance");
        assert_eq!(WorkerType::GnnTraining.to_string(), "gnn_training");
        assert_eq!(WorkerType::Integrity.to_string(), "integrity");
    }

    #[test]
    fn test_worker_registry() {
        let registry = WorkerRegistry::new();

        let handle = WorkerHandle {
            id: 1,
            pid: 12345,
            worker_type: WorkerType::Engine,
            status: WorkerStatus::Running,
            started_at: 0,
            last_activity: 0,
        };

        registry.register(WorkerType::Engine, handle.clone());
        assert_eq!(registry.count(WorkerType::Engine), 1);
        assert_eq!(registry.total_count(), 1);

        registry.unregister(WorkerType::Engine, 1);
        assert_eq!(registry.count(WorkerType::Engine), 0);
    }
}
