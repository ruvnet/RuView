//! Integrity Gating Module
//!
//! Implements the integrity gate check system with hysteresis-based state
//! transitions. Operations are allowed, throttled, or blocked based on the
//! current integrity state.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Integrity states representing system health levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityState {
    /// System is healthy, all operations allowed
    Normal = 0,
    /// System under stress, some operations throttled
    Stress = 1,
    /// Critical state, many operations blocked
    Critical = 2,
    /// Emergency state, only essential operations allowed
    Emergency = 3,
}

impl std::fmt::Display for IntegrityState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntegrityState::Normal => write!(f, "normal"),
            IntegrityState::Stress => write!(f, "stress"),
            IntegrityState::Critical => write!(f, "critical"),
            IntegrityState::Emergency => write!(f, "emergency"),
        }
    }
}

impl IntegrityState {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "normal" => Some(IntegrityState::Normal),
            "stress" => Some(IntegrityState::Stress),
            "critical" => Some(IntegrityState::Critical),
            "emergency" => Some(IntegrityState::Emergency),
            _ => None,
        }
    }

    /// Determine state from lambda cut value using thresholds
    pub fn from_lambda(
        lambda_cut: f64,
        threshold_high: f64,
        threshold_low: f64,
        threshold_critical: f64,
    ) -> Self {
        if lambda_cut >= threshold_high {
            IntegrityState::Normal
        } else if lambda_cut >= threshold_low {
            IntegrityState::Stress
        } else if lambda_cut >= threshold_critical {
            IntegrityState::Critical
        } else {
            IntegrityState::Emergency
        }
    }

    /// Convert to numeric value for atomic operations
    pub fn as_u32(&self) -> u32 {
        *self as u32
    }

    /// Convert from numeric value
    pub fn from_u32(v: u32) -> Self {
        match v {
            0 => IntegrityState::Normal,
            1 => IntegrityState::Stress,
            2 => IntegrityState::Critical,
            _ => IntegrityState::Emergency,
        }
    }
}

/// Hysteresis thresholds for state transitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HysteresisThresholds {
    /// Rising threshold to enter Normal from Stress
    pub normal_rising: f64,
    /// Falling threshold to enter Stress from Normal
    pub normal_falling: f64,
    /// Rising threshold to enter Stress from Critical
    pub stress_rising: f64,
    /// Falling threshold to enter Critical from Stress
    pub stress_falling: f64,
    /// Rising threshold to enter Critical from Emergency
    pub critical_rising: f64,
    /// Falling threshold to enter Emergency from Critical
    pub critical_falling: f64,
}

impl Default for HysteresisThresholds {
    fn default() -> Self {
        Self {
            normal_rising: 0.8,
            normal_falling: 0.7,
            stress_rising: 0.5,
            stress_falling: 0.4,
            critical_rising: 0.2,
            critical_falling: 0.1,
        }
    }
}

impl HysteresisThresholds {
    /// Compute next state with hysteresis
    pub fn compute_next_state(&self, current: IntegrityState, lambda_cut: f64) -> IntegrityState {
        match current {
            IntegrityState::Normal => {
                if lambda_cut < self.normal_falling {
                    IntegrityState::Stress
                } else {
                    IntegrityState::Normal
                }
            }
            IntegrityState::Stress => {
                if lambda_cut >= self.normal_rising {
                    IntegrityState::Normal
                } else if lambda_cut < self.stress_falling {
                    IntegrityState::Critical
                } else {
                    IntegrityState::Stress
                }
            }
            IntegrityState::Critical => {
                if lambda_cut >= self.stress_rising {
                    IntegrityState::Stress
                } else if lambda_cut < self.critical_falling {
                    IntegrityState::Emergency
                } else {
                    IntegrityState::Critical
                }
            }
            IntegrityState::Emergency => {
                if lambda_cut >= self.critical_rising {
                    IntegrityState::Critical
                } else {
                    IntegrityState::Emergency
                }
            }
        }
    }
}

/// Operation permissions for each state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatePermissions {
    /// Allow read operations
    pub allow_reads: bool,
    /// Allow single inserts
    pub allow_single_insert: bool,
    /// Allow bulk inserts
    pub allow_bulk_insert: bool,
    /// Allow deletes
    pub allow_delete: bool,
    /// Allow updates
    pub allow_update: bool,
    /// Allow index rewiring
    pub allow_index_rewire: bool,
    /// Allow compression/compaction
    pub allow_compression: bool,
    /// Allow replication
    pub allow_replication: bool,
    /// Allow backups
    pub allow_backup: bool,
    /// Throttle percentage for inserts (0-100)
    pub throttle_inserts_pct: u8,
    /// Throttle percentage for searches (0-100)
    pub throttle_searches_pct: u8,
    /// Maximum concurrent searches (None = unlimited)
    pub max_concurrent_searches: Option<u32>,
    /// Pause GNN training
    pub pause_gnn_training: bool,
    /// Pause tier management
    pub pause_tier_management: bool,
}

impl Default for StatePermissions {
    fn default() -> Self {
        Self::normal()
    }
}

impl StatePermissions {
    /// Normal state permissions - all operations allowed
    pub fn normal() -> Self {
        Self {
            allow_reads: true,
            allow_single_insert: true,
            allow_bulk_insert: true,
            allow_delete: true,
            allow_update: true,
            allow_index_rewire: true,
            allow_compression: true,
            allow_replication: true,
            allow_backup: true,
            throttle_inserts_pct: 0,
            throttle_searches_pct: 0,
            max_concurrent_searches: None,
            pause_gnn_training: false,
            pause_tier_management: false,
        }
    }

    /// Stress state permissions - throttled operations
    pub fn stress() -> Self {
        Self {
            allow_reads: true,
            allow_single_insert: true,
            allow_bulk_insert: false, // No bulk inserts
            allow_delete: true,
            allow_update: true,
            allow_index_rewire: false, // No index rewiring
            allow_compression: false,  // No compression
            allow_replication: true,
            allow_backup: true,
            throttle_inserts_pct: 50, // 50% throttle
            throttle_searches_pct: 0,
            max_concurrent_searches: Some(100),
            pause_gnn_training: true, // Pause training
            pause_tier_management: false,
        }
    }

    /// Critical state permissions - limited operations
    pub fn critical() -> Self {
        Self {
            allow_reads: true,
            allow_single_insert: true,
            allow_bulk_insert: false,
            allow_delete: false, // No deletes
            allow_update: false, // No updates
            allow_index_rewire: false,
            allow_compression: false,
            allow_replication: true,   // Keep replication
            allow_backup: true,        // Keep backups
            throttle_inserts_pct: 90,  // Heavy throttle
            throttle_searches_pct: 25, // Some search throttle
            max_concurrent_searches: Some(50),
            pause_gnn_training: true,
            pause_tier_management: true,
        }
    }

    /// Emergency state permissions - read-only mode
    pub fn emergency() -> Self {
        Self {
            allow_reads: true,
            allow_single_insert: false, // No writes
            allow_bulk_insert: false,
            allow_delete: false,
            allow_update: false,
            allow_index_rewire: false,
            allow_compression: false,
            allow_replication: false,  // Stop replication
            allow_backup: true,        // Allow backup for recovery
            throttle_inserts_pct: 100, // Block all inserts
            throttle_searches_pct: 50, // Heavy search throttle
            max_concurrent_searches: Some(20),
            pause_gnn_training: true,
            pause_tier_management: true,
        }
    }

    /// Get permissions for a given state
    pub fn for_state(state: IntegrityState) -> Self {
        match state {
            IntegrityState::Normal => Self::normal(),
            IntegrityState::Stress => Self::stress(),
            IntegrityState::Critical => Self::critical(),
            IntegrityState::Emergency => Self::emergency(),
        }
    }
}

/// Result of a gate check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    /// Whether the operation is allowed
    pub allowed: bool,
    /// Throttle percentage (0-100)
    pub throttle_pct: u8,
    /// Current integrity state
    pub state: IntegrityState,
    /// Reason for rejection (if any)
    pub reason: Option<String>,
    /// Suggested retry delay in milliseconds
    pub retry_delay_ms: Option<u64>,
}

impl GateResult {
    /// Create an allowed result
    pub fn allow(state: IntegrityState) -> Self {
        Self {
            allowed: true,
            throttle_pct: 0,
            state,
            reason: None,
            retry_delay_ms: None,
        }
    }

    /// Create a throttled result
    pub fn throttle(state: IntegrityState, throttle_pct: u8) -> Self {
        Self {
            allowed: true,
            throttle_pct,
            state,
            reason: None,
            retry_delay_ms: None,
        }
    }

    /// Create a blocked result
    pub fn block(state: IntegrityState, reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            throttle_pct: 100,
            state,
            reason: Some(reason.into()),
            retry_delay_ms: Some(5000), // 5 second default retry
        }
    }

    /// Should apply throttling
    pub fn should_throttle(&self) -> bool {
        self.throttle_pct > 0
    }
}

/// Integrity gate for a collection
pub struct IntegrityGate {
    /// Collection ID
    collection_id: i32,
    /// Current state (atomic for lock-free reads)
    state: AtomicU32,
    /// Current lambda cut value (scaled by 1000 for atomic storage)
    lambda_cut_scaled: AtomicU32,
    /// Hysteresis thresholds
    thresholds: HysteresisThresholds,
    /// Custom permissions (override defaults)
    custom_permissions: Option<HashMap<IntegrityState, StatePermissions>>,
    /// Concurrent search counter
    concurrent_searches: AtomicU32,
    /// Last state change time (epoch millis)
    last_state_change_ms: AtomicU64,
}

impl IntegrityGate {
    /// Create a new integrity gate
    pub fn new(collection_id: i32) -> Self {
        Self {
            collection_id,
            state: AtomicU32::new(IntegrityState::Normal.as_u32()),
            lambda_cut_scaled: AtomicU32::new(1000), // 1.0 scaled
            thresholds: HysteresisThresholds::default(),
            custom_permissions: None,
            concurrent_searches: AtomicU32::new(0),
            last_state_change_ms: AtomicU64::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            ),
        }
    }

    /// Create with custom thresholds
    pub fn with_thresholds(mut self, thresholds: HysteresisThresholds) -> Self {
        self.thresholds = thresholds;
        self
    }

    /// Set custom permissions
    pub fn with_permissions(
        mut self,
        permissions: HashMap<IntegrityState, StatePermissions>,
    ) -> Self {
        self.custom_permissions = Some(permissions);
        self
    }

    /// Get current state
    pub fn current_state(&self) -> IntegrityState {
        IntegrityState::from_u32(self.state.load(Ordering::Relaxed))
    }

    /// Get current lambda cut value
    pub fn current_lambda_cut(&self) -> f64 {
        self.lambda_cut_scaled.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Update state based on new lambda cut value
    pub fn update_lambda(&self, lambda_cut: f64) -> Option<IntegrityState> {
        let current = self.current_state();
        let new_state = self.thresholds.compute_next_state(current, lambda_cut);

        // Store lambda cut (scaled)
        let scaled = (lambda_cut * 1000.0).round() as u32;
        self.lambda_cut_scaled.store(scaled, Ordering::Relaxed);

        // Update state if changed
        if new_state != current {
            self.state.store(new_state.as_u32(), Ordering::Release);
            self.last_state_change_ms.store(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                Ordering::Relaxed,
            );
            Some(new_state)
        } else {
            None
        }
    }

    /// Force set state (for testing or admin override)
    pub fn force_state(&self, state: IntegrityState) {
        self.state.store(state.as_u32(), Ordering::Release);
        self.last_state_change_ms.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            Ordering::Relaxed,
        );
    }

    /// Get permissions for current state
    pub fn current_permissions(&self) -> StatePermissions {
        let state = self.current_state();
        self.custom_permissions
            .as_ref()
            .and_then(|p| p.get(&state).cloned())
            .unwrap_or_else(|| StatePermissions::for_state(state))
    }

    /// Check if an operation is allowed
    pub fn check_operation(&self, operation: &str) -> GateResult {
        let state = self.current_state();
        let permissions = self.current_permissions();

        // Map operation to permission
        let (allowed, throttle_pct) = match operation.to_lowercase().as_str() {
            "search" | "read" | "query" => {
                let within_limit = permissions.max_concurrent_searches.map_or(true, |max| {
                    self.concurrent_searches.load(Ordering::Relaxed) < max
                });
                (
                    permissions.allow_reads && within_limit,
                    permissions.throttle_searches_pct,
                )
            }
            "insert" => (
                permissions.allow_single_insert,
                permissions.throttle_inserts_pct,
            ),
            "bulk_insert" => (
                permissions.allow_bulk_insert,
                permissions.throttle_inserts_pct,
            ),
            "delete" => (permissions.allow_delete, 0),
            "update" => (permissions.allow_update, 0),
            "index_build" | "index_rewire" => (permissions.allow_index_rewire, 0),
            "compression" | "compact" => (permissions.allow_compression, 0),
            "replication" | "replicate" => (permissions.allow_replication, 0),
            "backup" => (permissions.allow_backup, 0),
            "gnn_train" | "gnn_training" => (!permissions.pause_gnn_training, 0),
            "tier_manage" | "tier_management" => (!permissions.pause_tier_management, 0),
            _ => {
                // Unknown operations allowed by default
                (true, 0)
            }
        };

        if !allowed {
            GateResult::block(
                state,
                format!(
                    "Operation '{}' blocked: system in {} state",
                    operation, state
                ),
            )
        } else if throttle_pct > 0 {
            GateResult::throttle(state, throttle_pct)
        } else {
            GateResult::allow(state)
        }
    }

    /// Increment concurrent search counter
    pub fn begin_search(&self) -> bool {
        let permissions = self.current_permissions();
        if let Some(max) = permissions.max_concurrent_searches {
            let current = self.concurrent_searches.fetch_add(1, Ordering::AcqRel);
            if current >= max {
                self.concurrent_searches.fetch_sub(1, Ordering::AcqRel);
                return false;
            }
        } else {
            self.concurrent_searches.fetch_add(1, Ordering::AcqRel);
        }
        true
    }

    /// Decrement concurrent search counter
    pub fn end_search(&self) {
        let prev = self.concurrent_searches.fetch_sub(1, Ordering::AcqRel);
        if prev == 0 {
            // Shouldn't happen, but prevent underflow
            self.concurrent_searches.store(0, Ordering::Relaxed);
        }
    }

    /// Get gate status as JSON
    pub fn status(&self) -> serde_json::Value {
        let state = self.current_state();
        let permissions = self.current_permissions();

        serde_json::json!({
            "collection_id": self.collection_id,
            "state": state.to_string(),
            "lambda_cut": self.current_lambda_cut(),
            "concurrent_searches": self.concurrent_searches.load(Ordering::Relaxed),
            "permissions": {
                "allow_reads": permissions.allow_reads,
                "allow_single_insert": permissions.allow_single_insert,
                "allow_bulk_insert": permissions.allow_bulk_insert,
                "allow_delete": permissions.allow_delete,
                "allow_update": permissions.allow_update,
                "allow_index_rewire": permissions.allow_index_rewire,
                "throttle_inserts_pct": permissions.throttle_inserts_pct,
                "throttle_searches_pct": permissions.throttle_searches_pct,
            }
        })
    }
}

/// Apply throttling based on percentage
/// Returns true if the operation should proceed, false if throttled
pub fn apply_throttle(throttle_pct: u8) -> bool {
    if throttle_pct == 0 {
        return true; // Not throttled
    }
    if throttle_pct >= 100 {
        return false; // Fully throttled
    }

    // Random rejection based on percentage
    let random_val = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
        % 100) as u8;

    random_val >= throttle_pct
}

/// Global registry for integrity gates
static GATE_REGISTRY: once_cell::sync::Lazy<DashMap<i32, IntegrityGate>> =
    once_cell::sync::Lazy::new(DashMap::new);

/// Get or create an integrity gate for a collection
pub fn get_or_create_gate(
    collection_id: i32,
) -> dashmap::mapref::one::Ref<'static, i32, IntegrityGate> {
    if !GATE_REGISTRY.contains_key(&collection_id) {
        GATE_REGISTRY.insert(collection_id, IntegrityGate::new(collection_id));
    }
    GATE_REGISTRY.get(&collection_id).unwrap()
}

/// Get an existing integrity gate
pub fn get_gate(
    collection_id: i32,
) -> Option<dashmap::mapref::one::Ref<'static, i32, IntegrityGate>> {
    GATE_REGISTRY.get(&collection_id)
}

/// Check integrity gate for an operation
pub fn check_integrity_gate(collection_id: i32, operation: &str) -> GateResult {
    let gate = get_or_create_gate(collection_id);
    gate.check_operation(operation)
}

/// Update lambda cut value for a collection
pub fn update_lambda_cut(collection_id: i32, lambda_cut: f64) -> Option<IntegrityState> {
    let gate = get_or_create_gate(collection_id);
    gate.update_lambda(lambda_cut)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integrity_state_display() {
        assert_eq!(IntegrityState::Normal.to_string(), "normal");
        assert_eq!(IntegrityState::Stress.to_string(), "stress");
        assert_eq!(IntegrityState::Critical.to_string(), "critical");
        assert_eq!(IntegrityState::Emergency.to_string(), "emergency");
    }

    #[test]
    fn test_state_parsing() {
        assert_eq!(
            IntegrityState::from_str("normal"),
            Some(IntegrityState::Normal)
        );
        assert_eq!(
            IntegrityState::from_str("STRESS"),
            Some(IntegrityState::Stress)
        );
        assert_eq!(IntegrityState::from_str("invalid"), None);
    }

    #[test]
    fn test_hysteresis_transitions() {
        let thresholds = HysteresisThresholds::default();

        // Normal -> Stress when lambda drops below 0.7
        let state = thresholds.compute_next_state(IntegrityState::Normal, 0.6);
        assert_eq!(state, IntegrityState::Stress);

        // Stress -> Normal when lambda rises above 0.8
        let state = thresholds.compute_next_state(IntegrityState::Stress, 0.85);
        assert_eq!(state, IntegrityState::Normal);

        // Stress stays Stress between 0.5 and 0.8
        let state = thresholds.compute_next_state(IntegrityState::Stress, 0.6);
        assert_eq!(state, IntegrityState::Stress);
    }

    #[test]
    fn test_gate_operations() {
        let gate = IntegrityGate::new(1);

        // Normal state - all allowed
        let result = gate.check_operation("insert");
        assert!(result.allowed);
        assert_eq!(result.throttle_pct, 0);

        // Force stress state
        gate.force_state(IntegrityState::Stress);

        // Bulk insert blocked in stress
        let result = gate.check_operation("bulk_insert");
        assert!(!result.allowed);

        // Single insert throttled in stress
        let result = gate.check_operation("insert");
        assert!(result.allowed);
        assert_eq!(result.throttle_pct, 50);
    }

    #[test]
    fn test_emergency_permissions() {
        let gate = IntegrityGate::new(1);
        gate.force_state(IntegrityState::Emergency);

        // Reads still allowed
        let result = gate.check_operation("search");
        assert!(result.allowed);

        // Writes blocked
        let result = gate.check_operation("insert");
        assert!(!result.allowed);

        let result = gate.check_operation("delete");
        assert!(!result.allowed);

        // Backups still allowed
        let result = gate.check_operation("backup");
        assert!(result.allowed);
    }

    #[test]
    fn test_lambda_update() {
        let gate = IntegrityGate::new(1);

        // Initially normal
        assert_eq!(gate.current_state(), IntegrityState::Normal);

        // Drop lambda to trigger stress
        let new_state = gate.update_lambda(0.5);
        assert_eq!(new_state, Some(IntegrityState::Stress));
        assert_eq!(gate.current_state(), IntegrityState::Stress);

        // Lambda cut stored correctly
        assert!((gate.current_lambda_cut() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_concurrent_search_limit() {
        let gate = IntegrityGate::new(1);
        gate.force_state(IntegrityState::Critical); // max 50 searches

        // Start many searches
        for _ in 0..50 {
            assert!(gate.begin_search());
        }

        // 51st should fail
        assert!(!gate.begin_search());

        // End one, then can start another
        gate.end_search();
        assert!(gate.begin_search());
    }

    #[test]
    fn test_throttle_function() {
        // 0% throttle always passes
        for _ in 0..100 {
            assert!(apply_throttle(0));
        }

        // 100% throttle always blocks
        for _ in 0..100 {
            assert!(!apply_throttle(100));
        }
    }

    #[test]
    fn test_gate_registry() {
        let gate1 = get_or_create_gate(9001);
        assert_eq!(gate1.collection_id, 9001);

        let gate2 = get_gate(9001);
        assert!(gate2.is_some());

        // Check non-existent
        let gate3 = get_gate(9999);
        assert!(gate3.is_none());
    }

    #[test]
    fn test_gate_status() {
        let gate = IntegrityGate::new(42);
        let status = gate.status();

        assert_eq!(status["collection_id"], 42);
        assert_eq!(status["state"], "normal");
        assert!(status["permissions"]["allow_reads"].as_bool().unwrap());
    }
}
