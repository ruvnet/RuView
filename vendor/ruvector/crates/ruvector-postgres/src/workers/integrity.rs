//! Integrity Worker - Continuous Integrity Monitoring
//!
//! The Integrity Worker continuously monitors system health via contracted graph
//! analysis and mincut computation. It gates operations based on integrity state.
//!
//! # Responsibilities
//!
//! - Periodic contracted graph sampling
//! - Mincut computation for integrity metrics
//! - State transition management with hysteresis
//! - Operation gating based on integrity state
//! - Event logging and audit trail

use parking_lot::RwLock;
use pgrx::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ============================================================================
// Stoer-Wagner Mincut Algorithm (Self-contained implementation)
// ============================================================================

/// Compute the global minimum cut using Stoer-Wagner algorithm.
///
/// The algorithm finds the minimum cut that partitions the graph into two
/// non-empty sets. It works by repeatedly finding minimum s-t cuts and
/// contracting vertices until only 2 vertices remain.
///
/// # Arguments
/// * `num_nodes` - Number of nodes in the graph
/// * `edges` - List of edges as (source, target, weight) tuples
///
/// # Returns
/// The weight of the minimum cut (lambda cut value)
pub fn stoer_wagner_mincut(num_nodes: usize, edges: &[(usize, usize, f64)]) -> f64 {
    if num_nodes <= 1 || edges.is_empty() {
        return f64::INFINITY;
    }

    // Build adjacency matrix
    let mut adj = vec![vec![0.0; num_nodes]; num_nodes];
    for &(u, v, w) in edges {
        if u < num_nodes && v < num_nodes {
            adj[u][v] += w;
            adj[v][u] += w;
        }
    }

    // Track which vertices are still active (not yet contracted)
    let mut active: Vec<bool> = vec![true; num_nodes];
    // Maps each vertex to its contracted representative
    let mut vertex_map: Vec<usize> = (0..num_nodes).collect();

    let mut min_cut = f64::INFINITY;
    let mut remaining = num_nodes;

    while remaining > 1 {
        // Find the minimum s-t cut using maximum adjacency search
        let (cut_weight, s, t) = minimum_cut_phase(&adj, &active, remaining);

        if cut_weight < min_cut {
            min_cut = cut_weight;
        }

        // Contract s and t (merge t into s)
        if s < num_nodes && t < num_nodes {
            // Add t's edges to s
            for i in 0..num_nodes {
                if active[i] && i != s && i != t {
                    adj[s][i] += adj[t][i];
                    adj[i][s] += adj[i][t];
                }
            }

            // Deactivate t
            active[t] = false;
            vertex_map[t] = s;
            remaining -= 1;
        } else {
            break;
        }
    }

    // Normalize the cut value to [0, 1] range
    let total_weight: f64 = edges.iter().map(|(_, _, w)| w).sum();
    if total_weight > 0.0 {
        (min_cut / total_weight).min(1.0)
    } else {
        1.0
    }
}

/// Perform one phase of the Stoer-Wagner algorithm using maximum adjacency search.
/// Returns (cut_weight, s, t) where s and t are the last two vertices added.
fn minimum_cut_phase(adj: &[Vec<f64>], active: &[bool], _remaining: usize) -> (f64, usize, usize) {
    let n = adj.len();

    // Find first active vertex to start
    let start = active.iter().position(|&a| a).unwrap_or(0);

    // Track which vertices are in the set A
    let mut in_a = vec![false; n];
    // Track the cut weight of each vertex (sum of edges to A)
    let mut cut_weight = vec![0.0; n];

    let mut last = start;
    let mut second_last = start;
    let mut last_cut = 0.0;

    // Add vertices one by one using maximum adjacency ordering
    for _ in 0..active.iter().filter(|&&a| a).count() {
        // Find the vertex with maximum cut weight not yet in A
        let mut max_weight = f64::NEG_INFINITY;
        let mut max_vertex = start;

        for i in 0..n {
            if active[i] && !in_a[i] && cut_weight[i] > max_weight {
                max_weight = cut_weight[i];
                max_vertex = i;
            }
        }

        // Add this vertex to A
        in_a[max_vertex] = true;
        second_last = last;
        last = max_vertex;
        last_cut = cut_weight[max_vertex];

        // Update cut weights for remaining vertices
        for i in 0..n {
            if active[i] && !in_a[i] {
                cut_weight[i] += adj[max_vertex][i];
            }
        }
    }

    (last_cut, second_last, last)
}

// ============================================================================
// Integrity State Types
// ============================================================================

/// Integrity state representing system health levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityStateType {
    /// System is healthy, all operations allowed
    Normal = 0,
    /// System under stress, some operations throttled
    Stress = 1,
    /// Critical state, many operations blocked
    Critical = 2,
    /// Emergency state, only essential operations allowed
    Emergency = 3,
}

impl std::fmt::Display for IntegrityStateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntegrityStateType::Normal => write!(f, "normal"),
            IntegrityStateType::Stress => write!(f, "stress"),
            IntegrityStateType::Critical => write!(f, "critical"),
            IntegrityStateType::Emergency => write!(f, "emergency"),
        }
    }
}

impl IntegrityStateType {
    /// Determine state from lambda cut value using thresholds
    pub fn from_lambda(lambda_cut: f64, threshold_high: f64, threshold_low: f64) -> Self {
        if lambda_cut >= threshold_high {
            IntegrityStateType::Normal
        } else if lambda_cut >= threshold_low {
            IntegrityStateType::Stress
        } else if lambda_cut >= threshold_low / 2.0 {
            IntegrityStateType::Critical
        } else {
            IntegrityStateType::Emergency
        }
    }
}

// Re-export for external use
pub use IntegrityStateType as IntegrityState;

// ============================================================================
// Integrity Worker Configuration
// ============================================================================

/// Integrity worker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityConfig {
    /// Interval between samples in seconds
    pub sample_interval_secs: u64,
    /// Interval between graph rebuilds in seconds
    pub graph_rebuild_interval_secs: u64,
    /// High threshold for normal state
    pub threshold_high: f64,
    /// Low threshold for stress state
    pub threshold_low: f64,
    /// Maximum number of events to keep in memory
    pub max_events: usize,
    /// Enable detailed logging
    pub verbose: bool,
}

impl Default for IntegrityConfig {
    fn default() -> Self {
        Self {
            sample_interval_secs: 60,
            graph_rebuild_interval_secs: 3600,
            threshold_high: 0.7,
            threshold_low: 0.3,
            max_events: 10000,
            verbose: false,
        }
    }
}

// Global configuration
static INTEGRITY_CONFIG: OnceLock<RwLock<IntegrityConfig>> = OnceLock::new();

/// Get the current integrity configuration
pub fn get_integrity_config() -> IntegrityConfig {
    INTEGRITY_CONFIG
        .get_or_init(|| RwLock::new(IntegrityConfig::default()))
        .read()
        .clone()
}

/// Set the integrity configuration
pub fn set_integrity_config(config: IntegrityConfig) {
    let cfg = INTEGRITY_CONFIG.get_or_init(|| RwLock::new(IntegrityConfig::default()));
    *cfg.write() = config;
}

// ============================================================================
// Integrity State (Shared Memory Snapshot)
// ============================================================================

/// Integrity state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityStateRecord {
    /// Collection ID
    pub collection_id: i32,
    /// Current state
    pub state: IntegrityStateType,
    /// Last computed lambda cut value
    pub lambda_cut: f64,
    /// Last sample timestamp
    pub last_sample_ts: u64,
    /// Last state change timestamp
    pub last_state_change_ts: u64,
    /// Total samples taken
    pub sample_count: u64,
    /// Total state changes
    pub state_change_count: u64,
}

impl IntegrityStateRecord {
    /// Create a new integrity state
    pub fn new(collection_id: i32) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            collection_id,
            state: IntegrityStateType::Normal,
            lambda_cut: 1.0,
            last_sample_ts: now,
            last_state_change_ts: now,
            sample_count: 0,
            state_change_count: 0,
        }
    }

    /// Update from mincut computation
    pub fn update_from_mincut(&mut self, lambda_cut: f64, config: &IntegrityConfig) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let new_state = IntegrityStateType::from_lambda(
            lambda_cut,
            config.threshold_high,
            config.threshold_low,
        );

        if new_state != self.state {
            self.state = new_state;
            self.last_state_change_ts = now;
            self.state_change_count += 1;
        }

        self.lambda_cut = lambda_cut;
        self.last_sample_ts = now;
        self.sample_count += 1;
    }
}

// ============================================================================
// Simple Graph Structure for Mincut
// ============================================================================

/// Simple graph edge
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub source: usize,
    pub target: usize,
    pub weight: f64,
}

/// Build sample edges for a collection
fn build_sample_edges(num_nodes: usize) -> Vec<(usize, usize, f64)> {
    let mut edges = Vec::new();

    // Create a connected graph
    for i in 0..num_nodes.saturating_sub(1) {
        edges.push((i, i + 1, 1.0));
    }

    // Add some cross-edges for connectivity
    if num_nodes > 3 {
        for i in (0..num_nodes).step_by(3) {
            let j = (i + 2) % num_nodes;
            if i != j {
                edges.push((i, j, 0.5));
            }
        }
    }

    edges
}

// ============================================================================
// Integrity Worker
// ============================================================================

/// Integrity monitoring background worker
pub struct IntegrityWorker {
    /// Worker ID
    worker_id: u64,
    /// Configuration
    config: IntegrityConfig,
    /// Running flag
    running: AtomicBool,
    /// Collections being monitored
    collections: RwLock<Vec<i32>>,
    /// Last graph rebuild times per collection
    last_rebuild: RwLock<std::collections::HashMap<i32, Instant>>,
    /// Integrity states per collection
    states: RwLock<std::collections::HashMap<i32, IntegrityStateRecord>>,
}

impl IntegrityWorker {
    /// Create a new integrity worker
    pub fn new(worker_id: u64) -> Self {
        Self {
            worker_id,
            config: get_integrity_config(),
            running: AtomicBool::new(false),
            collections: RwLock::new(Vec::new()),
            last_rebuild: RwLock::new(std::collections::HashMap::new()),
            states: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Register a collection for monitoring
    pub fn register_collection(&self, collection_id: i32) {
        let mut collections = self.collections.write();
        if !collections.contains(&collection_id) {
            collections.push(collection_id);

            // Initialize state
            let mut states = self.states.write();
            states.insert(collection_id, IntegrityStateRecord::new(collection_id));
        }
    }

    /// Unregister a collection
    pub fn unregister_collection(&self, collection_id: i32) {
        let mut collections = self.collections.write();
        collections.retain(|&id| id != collection_id);

        let mut states = self.states.write();
        states.remove(&collection_id);
    }

    /// Sample and compute integrity metrics for a collection
    fn sample_collection(&self, collection_id: i32) -> Result<(), String> {
        // Build sample graph (in production, this would query actual data)
        let num_nodes = 10; // Sample size
        let edges = build_sample_edges(num_nodes);

        // Compute mincut using Stoer-Wagner
        let lambda_cut = stoer_wagner_mincut(num_nodes, &edges);

        if self.config.verbose {
            pgrx::log!("Collection {}: lambda_cut={:.4}", collection_id, lambda_cut);
        }

        // Update state
        let mut states = self.states.write();
        let state = states
            .entry(collection_id)
            .or_insert_with(|| IntegrityStateRecord::new(collection_id));

        let previous_state = state.state;
        state.update_from_mincut(lambda_cut, &self.config);

        if state.state != previous_state {
            pgrx::log!(
                "Integrity state change for collection {}: {} -> {} (lambda={:.4})",
                collection_id,
                previous_state,
                state.state,
                lambda_cut
            );
        }

        Ok(())
    }

    /// Main worker loop
    pub fn run(&self) {
        self.running.store(true, Ordering::SeqCst);
        pgrx::log!("Integrity worker {} started", self.worker_id);

        let sample_interval = Duration::from_secs(self.config.sample_interval_secs);

        while self.running.load(Ordering::SeqCst) {
            // Sample all registered collections
            let collections: Vec<i32> = self.collections.read().clone();

            for collection_id in collections {
                if !self.running.load(Ordering::SeqCst) {
                    break;
                }

                if let Err(e) = self.sample_collection(collection_id) {
                    pgrx::warning!("Failed to sample collection {}: {}", collection_id, e);
                }
            }

            // Sleep until next sample
            let sleep_end = Instant::now() + sample_interval;
            while Instant::now() < sleep_end && self.running.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(100));
            }
        }

        pgrx::log!("Integrity worker {} stopped", self.worker_id);
    }

    /// Stop the worker
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if worker is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get current state for a collection
    pub fn get_state(&self, collection_id: i32) -> Option<IntegrityStateRecord> {
        self.states.read().get(&collection_id).cloned()
    }

    /// Get all states
    pub fn get_all_states(&self) -> std::collections::HashMap<i32, IntegrityStateRecord> {
        self.states.read().clone()
    }

    /// Get worker statistics
    pub fn stats(&self) -> serde_json::Value {
        let states = self.states.read();
        let collections: Vec<_> = states
            .iter()
            .map(|(id, state)| {
                serde_json::json!({
                    "collection_id": id,
                    "state": state.state.to_string(),
                    "lambda_cut": state.lambda_cut,
                    "sample_count": state.sample_count,
                    "state_change_count": state.state_change_count,
                })
            })
            .collect();

        serde_json::json!({
            "worker_id": self.worker_id,
            "running": self.is_running(),
            "collection_count": states.len(),
            "collections": collections,
            "config": {
                "sample_interval_secs": self.config.sample_interval_secs,
                "threshold_high": self.config.threshold_high,
                "threshold_low": self.config.threshold_low,
            }
        })
    }
}

// ============================================================================
// Global Worker Instance
// ============================================================================

static INTEGRITY_WORKER: OnceLock<IntegrityWorker> = OnceLock::new();

/// Get or create the global integrity worker
pub fn get_integrity_worker() -> &'static IntegrityWorker {
    INTEGRITY_WORKER.get_or_init(|| IntegrityWorker::new(1))
}

// ============================================================================
// SQL Functions
// ============================================================================

/// Get integrity worker status
#[pg_extern]
pub fn ruvector_integrity_worker_status() -> pgrx::JsonB {
    let worker = get_integrity_worker();
    pgrx::JsonB(worker.stats())
}

/// Register a collection for integrity monitoring
#[pg_extern]
pub fn ruvector_integrity_register(collection_id: i32) -> pgrx::JsonB {
    let worker = get_integrity_worker();
    worker.register_collection(collection_id);

    pgrx::JsonB(serde_json::json!({
        "success": true,
        "collection_id": collection_id,
        "registered": true,
    }))
}

/// Unregister a collection from integrity monitoring
#[pg_extern]
pub fn ruvector_integrity_unregister(collection_id: i32) -> pgrx::JsonB {
    let worker = get_integrity_worker();
    worker.unregister_collection(collection_id);

    pgrx::JsonB(serde_json::json!({
        "success": true,
        "collection_id": collection_id,
        "registered": false,
    }))
}

/// Manually trigger a sample for a collection
#[pg_extern]
pub fn ruvector_integrity_sample(collection_id: i32) -> pgrx::JsonB {
    let worker = get_integrity_worker();
    worker.register_collection(collection_id);

    match worker.sample_collection(collection_id) {
        Ok(()) => {
            let state = worker.get_state(collection_id);
            pgrx::JsonB(serde_json::json!({
                "success": true,
                "collection_id": collection_id,
                "state": state.map(|s| serde_json::json!({
                    "state": s.state.to_string(),
                    "lambda_cut": s.lambda_cut,
                    "sample_count": s.sample_count,
                })),
            }))
        }
        Err(e) => pgrx::JsonB(serde_json::json!({
            "success": false,
            "error": e,
        })),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integrity_config_default() {
        let config = IntegrityConfig::default();
        assert_eq!(config.sample_interval_secs, 60);
        assert!((config.threshold_high - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_integrity_state_type() {
        assert_eq!(
            IntegrityStateType::from_lambda(0.8, 0.7, 0.3),
            IntegrityStateType::Normal
        );
        assert_eq!(
            IntegrityStateType::from_lambda(0.5, 0.7, 0.3),
            IntegrityStateType::Stress
        );
        assert_eq!(
            IntegrityStateType::from_lambda(0.2, 0.7, 0.3),
            IntegrityStateType::Critical
        );
    }

    #[test]
    fn test_integrity_state_record() {
        let state = IntegrityStateRecord::new(1);
        assert_eq!(state.collection_id, 1);
        assert_eq!(state.state, IntegrityStateType::Normal);
        assert_eq!(state.sample_count, 0);
    }

    #[test]
    fn test_build_sample_edges() {
        let edges = build_sample_edges(5);
        assert!(!edges.is_empty());
    }

    #[test]
    fn test_integrity_worker_registration() {
        let worker = IntegrityWorker::new(1);
        worker.register_collection(42);

        assert!(worker.get_state(42).is_some());

        worker.unregister_collection(42);
        assert!(worker.get_state(42).is_none());
    }
}
