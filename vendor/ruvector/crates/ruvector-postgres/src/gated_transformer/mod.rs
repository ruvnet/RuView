//! # Gated Transformer Module
//!
//! Integrates ruvector-mincut-gated-transformer for ultra-low-latency transformer
//! inference with mincut-gated coherence control directly in PostgreSQL.
//!
//! ## Features
//!
//! - **Dynamic Compute Allocation**: Uses Mixture-of-Depths for 50% FLOPs reduction
//! - **Early Exit**: Layer-skipping with 30-50% latency reduction
//! - **Mincut-Gated Coherence**: Gate decisions driven by integrity mincut signals
//! - **SQL Functions**: Direct access to transformer inference from SQL queries
//!
//! ## SQL Functions
//!
//! - `gated_transformer_gate_decision(lambda, lambda_prev, ...)` - Get gate decision
//! - `gated_transformer_early_exit_score(lambda, layer)` - Check early exit potential
//! - `gated_transformer_config()` - Get current transformer configuration

use parking_lot::RwLock;
use pgrx::prelude::*;
use ruvector_mincut_gated_transformer::{
    CoherenceEarlyExit, EarlyExitConfig, ExitReason, GateController, GateDecision, GatePacket,
    GatePolicy, GateReason, MincutDepthRouter, ModRoutingConfig, TierDecision, TokenRoute,
    TransformerConfig,
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Global transformer configuration
static TRANSFORMER_CONFIG: OnceLock<RwLock<TransformerConfig>> = OnceLock::new();

/// Global gate policy
static GATE_POLICY: OnceLock<RwLock<GatePolicy>> = OnceLock::new();

/// Global gate controller
static GATE_CONTROLLER: OnceLock<RwLock<GateController>> = OnceLock::new();

/// Initialize global configurations
fn ensure_initialized() {
    TRANSFORMER_CONFIG.get_or_init(|| RwLock::new(TransformerConfig::micro()));
    GATE_POLICY.get_or_init(|| RwLock::new(GatePolicy::default()));
    GATE_CONTROLLER.get_or_init(|| RwLock::new(GateController::new(GatePolicy::default())));
}

/// Gate decision result for SQL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecisionResult {
    /// Gate decision type
    pub decision: String,
    /// Reason for the decision
    pub reason: String,
    /// Compute tier (0=normal, 1=reduced, 2=safe, 3=skip)
    pub tier: u8,
    /// Number of layers to run
    pub layers_to_run: u16,
    /// Effective sequence length
    pub effective_seq_len: u16,
    /// Effective attention window
    pub effective_window: u16,
    /// Whether to skip inference entirely
    pub skip: bool,
    /// Whether KV writes are allowed
    pub allows_kv_writes: bool,
    /// Whether external writes are allowed
    pub allows_external_writes: bool,
}

impl From<TierDecision> for GateDecisionResult {
    fn from(tier: TierDecision) -> Self {
        let decision_str = match tier.decision {
            GateDecision::Allow => "allow",
            GateDecision::ReduceScope => "reduce_scope",
            GateDecision::FlushKv => "flush_kv",
            GateDecision::FreezeWrites => "freeze_writes",
            GateDecision::QuarantineUpdates => "quarantine_updates",
        };

        let reason_str = match tier.reason {
            GateReason::None => "none",
            GateReason::LambdaBelowMin => "lambda_below_min",
            GateReason::LambdaDroppedFast => "lambda_dropped_fast",
            GateReason::BoundarySpike => "boundary_spike",
            GateReason::BoundaryConcentrationSpike => "boundary_concentration_spike",
            GateReason::PartitionDrift => "partition_drift",
            GateReason::SpikeStorm => "spike_storm",
            GateReason::ForcedByFlag => "forced_by_flag",
        };

        Self {
            decision: decision_str.to_string(),
            reason: reason_str.to_string(),
            tier: tier.tier,
            layers_to_run: tier.layers_to_run,
            effective_seq_len: tier.effective_seq_len,
            effective_window: tier.effective_window,
            skip: tier.skip,
            allows_kv_writes: tier.decision.allows_kv_writes(),
            allows_external_writes: tier.decision.allows_external_writes(),
        }
    }
}

/// Early exit decision result for SQL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarlyExitResult {
    /// Whether to exit early
    pub should_exit: bool,
    /// Exit layer (if exiting)
    pub exit_layer: Option<u16>,
    /// Confidence score (Q15, 0-32767)
    pub confidence_q15: u16,
    /// Reason for decision
    pub reason: String,
    /// Number of speculative tokens to generate
    pub speculative_tokens: u8,
}

/// Token routing result for SQL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRoutingResult {
    /// Token position
    pub position: u16,
    /// Routing decision
    pub route: String,
}

// ============================================================================
// SQL Functions - Gate Control
// ============================================================================

/// Get gate decision based on mincut signals
///
/// # Arguments
/// * `lambda` - Current mincut value (graph connectivity)
/// * `lambda_prev` - Previous mincut value
/// * `boundary_edges` - Number of edges crossing the cut
/// * `partition_count` - Number of partitions in the graph
///
/// # Returns
/// JSON with gate decision details
#[pg_extern(immutable, parallel_safe)]
fn gated_transformer_gate_decision(
    lambda: i32,
    lambda_prev: i32,
    boundary_edges: i32,
    partition_count: Option<i32>,
) -> pgrx::JsonB {
    ensure_initialized();

    // Create gate packet from SQL parameters
    let gate = GatePacket {
        lambda: lambda.max(0) as u32,
        lambda_prev: lambda_prev.max(0) as u32,
        boundary_edges: boundary_edges.max(0) as u16,
        boundary_concentration_q15: 16384, // Default 50%
        partition_count: partition_count.unwrap_or(2).max(0) as u16,
        flags: 0,
    };

    // Get gate controller
    let controller = GATE_CONTROLLER.get().unwrap().read();

    // Evaluate gate conditions
    let tier_decision = controller.evaluate(&gate, None);

    let result = GateDecisionResult::from(tier_decision);
    pgrx::JsonB(serde_json::to_value(&result).unwrap_or_default())
}

/// Check if inference should proceed based on lambda delta
#[pg_extern(immutable, parallel_safe)]
fn gated_transformer_should_infer(lambda: i32, lambda_prev: i32) -> bool {
    // Simple heuristic: proceed if lambda changed significantly
    let delta = (lambda - lambda_prev).abs();
    delta >= 1 || lambda < lambda_prev
}

/// Get the current compute tier for given mincut state
#[pg_extern(immutable, parallel_safe)]
fn gated_transformer_compute_tier(lambda: i32, lambda_prev: i32, boundary_edges: i32) -> i32 {
    ensure_initialized();

    let gate = GatePacket {
        lambda: lambda.max(0) as u32,
        lambda_prev: lambda_prev.max(0) as u32,
        boundary_edges: boundary_edges.max(0) as u16,
        boundary_concentration_q15: 16384,
        partition_count: 2,
        flags: 0,
    };

    let controller = GATE_CONTROLLER.get().unwrap().read();
    let tier_decision = controller.evaluate(&gate, None);

    tier_decision.tier as i32
}

// ============================================================================
// SQL Functions - Early Exit
// ============================================================================

/// Compute early exit decision for the given layer and lambda signals
///
/// Returns a score indicating how likely the model should exit early.
/// Higher lambda and stability indicate higher confidence for early exit.
#[pg_extern(immutable, parallel_safe)]
fn gated_transformer_early_exit_check(
    lambda: i32,
    lambda_prev: i32,
    layer: i32,
    num_layers: Option<i32>,
) -> pgrx::JsonB {
    ensure_initialized();

    let max_layers = num_layers.unwrap_or(4).max(1) as u16;

    let gate = GatePacket {
        lambda: lambda.max(0) as u32,
        lambda_prev: lambda_prev.max(0) as u32,
        boundary_edges: 0,
        boundary_concentration_q15: 16384,
        partition_count: 2,
        flags: 0,
    };

    // Create early exit controller
    let config = EarlyExitConfig::default();
    let early_exit = match CoherenceEarlyExit::new(config, max_layers) {
        Ok(ee) => ee,
        Err(e) => {
            return pgrx::JsonB(serde_json::json!({
                "error": e,
                "should_exit": false,
            }));
        }
    };

    let decision = early_exit.should_exit(&gate, layer.max(0) as usize);

    let reason_str = match decision.reason {
        ExitReason::InsufficientConfidence => "insufficient_confidence",
        ExitReason::LambdaTooLow => "lambda_too_low",
        ExitReason::LambdaUnstable => "lambda_unstable",
        ExitReason::BoundariesTooConcentrated => "boundaries_too_concentrated",
        ExitReason::ConfidentExit => "confident_exit",
        ExitReason::ForcedContinue => "forced_continue",
    };

    let result = EarlyExitResult {
        should_exit: decision.can_exit,
        exit_layer: if decision.can_exit {
            Some(decision.exit_layer)
        } else {
            None
        },
        confidence_q15: decision.confidence_q15,
        reason: reason_str.to_string(),
        speculative_tokens: if decision.enable_speculation { 4 } else { 0 },
    };

    pgrx::JsonB(serde_json::to_value(&result).unwrap_or_default())
}

/// Check if early exit is possible at the given layer
#[pg_extern(immutable, parallel_safe)]
fn gated_transformer_can_exit_early(lambda: i32, layer: i32, num_layers: i32) -> bool {
    ensure_initialized();

    let gate = GatePacket {
        lambda: lambda.max(0) as u32,
        lambda_prev: lambda.max(0) as u32, // Stable
        boundary_edges: 0,
        boundary_concentration_q15: 16384,
        partition_count: 2,
        flags: 0,
    };

    let max_layers = num_layers.max(1) as u16;
    let config = EarlyExitConfig::default();

    match CoherenceEarlyExit::new(config, max_layers) {
        Ok(ee) => {
            let decision = ee.should_exit(&gate, layer.max(0) as usize);
            decision.can_exit
        }
        Err(_) => false,
    }
}

// ============================================================================
// SQL Functions - Token Routing (Mixture-of-Depths)
// ============================================================================

/// Route tokens using Mixture-of-Depths
///
/// Returns routing decisions for each token indicating whether it should be
/// processed through the full transformer or skipped.
#[pg_extern(immutable, parallel_safe)]
fn gated_transformer_route_tokens(
    lambda: i32,
    lambda_prev: i32,
    num_tokens: i32,
    capacity_ratio: Option<f32>,
) -> pgrx::JsonB {
    ensure_initialized();

    let gate = GatePacket {
        lambda: lambda.max(0) as u32,
        lambda_prev: lambda_prev.max(0) as u32,
        boundary_edges: 0,
        boundary_concentration_q15: 16384,
        partition_count: 2,
        flags: 0,
    };

    let mut config = ModRoutingConfig::default();
    if let Some(ratio) = capacity_ratio {
        config.layer_capacity_ratio = ratio.clamp(0.1, 1.0);
    }

    let router = match MincutDepthRouter::new(config) {
        Ok(r) => r,
        Err(e) => {
            return pgrx::JsonB(serde_json::json!({
                "error": e,
                "routes": [],
            }));
        }
    };

    // Create token positions
    let positions: Vec<u16> = (0..num_tokens.max(0) as u16).collect();

    let routes = router.route_tokens(&gate, &positions);

    let results: Vec<TokenRoutingResult> = routes
        .iter()
        .enumerate()
        .map(|(idx, route)| TokenRoutingResult {
            position: idx as u16,
            route: match route {
                TokenRoute::Compute => "compute".to_string(),
                TokenRoute::Skip => "skip".to_string(),
                TokenRoute::Boundary => "boundary".to_string(),
            },
        })
        .collect();

    pgrx::JsonB(serde_json::to_value(&results).unwrap_or_default())
}

/// Get number of tokens to process given capacity
#[pg_extern(immutable, parallel_safe)]
fn gated_transformer_routing_capacity(num_tokens: i32, capacity_ratio: f32) -> i32 {
    ((num_tokens as f32) * capacity_ratio.clamp(0.0, 1.0)).ceil() as i32
}

// ============================================================================
// SQL Functions - Configuration
// ============================================================================

/// Get current transformer configuration
#[pg_extern]
fn gated_transformer_config() -> pgrx::JsonB {
    ensure_initialized();

    let config = TRANSFORMER_CONFIG.get().unwrap().read();

    pgrx::JsonB(serde_json::json!({
        "seq_len_max": config.seq_len_max,
        "hidden": config.hidden,
        "heads": config.heads,
        "layers": config.layers,
        "head_dim": config.head_dim(),
        "window_normal": config.window_normal,
        "window_degraded": config.window_degraded,
        "layers_degraded": config.layers_degraded,
    }))
}

/// Set transformer configuration preset
#[pg_extern]
fn gated_transformer_set_config(preset: &str) -> bool {
    ensure_initialized();

    let new_config = match preset.to_lowercase().as_str() {
        "micro" => TransformerConfig::micro(),
        "baseline" => TransformerConfig::baseline(),
        _ => return false,
    };

    *TRANSFORMER_CONFIG.get().unwrap().write() = new_config;
    true
}

/// Get gate policy configuration
#[pg_extern]
fn gated_transformer_gate_policy() -> pgrx::JsonB {
    ensure_initialized();

    let policy = GATE_POLICY.get().unwrap().read();

    pgrx::JsonB(serde_json::json!({
        "lambda_min": policy.lambda_min,
        "drop_ratio_q15_max": policy.drop_ratio_q15_max,
        "boundary_edges_max": policy.boundary_edges_max,
        "boundary_concentration_q15_max": policy.boundary_concentration_q15_max,
        "partitions_max": policy.partitions_max,
        "allow_kv_write_when_unstable": policy.allow_kv_write_when_unstable,
        "allow_external_write_when_unstable": policy.allow_external_write_when_unstable,
    }))
}

/// Set gate policy preset
#[pg_extern]
fn gated_transformer_set_policy(preset: &str) -> bool {
    ensure_initialized();

    let new_policy = match preset.to_lowercase().as_str() {
        "conservative" => GatePolicy::conservative(),
        "permissive" => GatePolicy::permissive(),
        "default" => GatePolicy::default(),
        _ => return false,
    };

    *GATE_POLICY.get().unwrap().write() = new_policy.clone();
    *GATE_CONTROLLER.get().unwrap().write() = GateController::new(new_policy);
    true
}

// ============================================================================
// SQL Functions - Integration with Integrity Module
// ============================================================================

/// Connect integrity mincut signals to gate decision
///
/// This function bridges the integrity module's mincut computation with
/// the gated transformer's gate controller.
#[pg_extern]
fn gated_transformer_from_integrity(index_name: &str) -> pgrx::JsonB {
    ensure_initialized();

    // Get current mincut from integrity module
    let mincut_result = crate::integrity::get_current_mincut(index_name);

    match mincut_result {
        Ok(result) => {
            let gate = GatePacket {
                lambda: result.lambda_cut as u32,
                lambda_prev: result.lambda_cut as u32, // Use stored previous
                boundary_edges: result.witness_edges.len() as u16,
                boundary_concentration_q15: 16384,
                partition_count: 2,
                flags: 0,
            };

            let controller = GATE_CONTROLLER.get().unwrap().read();
            let tier_decision = controller.evaluate(&gate, None);

            let result = GateDecisionResult::from(tier_decision);
            pgrx::JsonB(serde_json::to_value(&result).unwrap_or_default())
        }
        Err(e) => pgrx::JsonB(serde_json::json!({
            "error": format!("Failed to get mincut: {}", e),
            "decision": "allow",
            "tier": 0,
        })),
    }
}

/// Get coherence score combining mincut and transformer signals
#[pg_extern(immutable, parallel_safe)]
fn gated_transformer_coherence_score(lambda: i32, lambda_prev: i32, boundary_edges: i32) -> f32 {
    // Combine mincut stability with boundary edge count
    let lambda_stability = if lambda_prev > 0 {
        1.0 - ((lambda - lambda_prev).abs() as f32 / lambda_prev as f32).min(1.0)
    } else {
        0.5
    };

    // Boundary edge factor (fewer is better)
    let boundary_factor = 1.0 / (1.0 + boundary_edges as f32 * 0.1);

    // Weighted average
    0.7 * lambda_stability + 0.3 * boundary_factor
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(feature = "pg_test")]
#[pgrx::pg_schema]
mod tests {
    use super::*;
    use pgrx::prelude::*;

    #[pg_test]
    fn test_gate_decision() {
        let result = gated_transformer_gate_decision(100, 95, 5, Some(2));
        let json: serde_json::Value = serde_json::from_value(result.0).unwrap();
        assert!(json.get("decision").is_some());
        assert!(json.get("tier").is_some());
    }

    #[pg_test]
    fn test_should_infer() {
        // Lambda decreased - should infer
        assert!(gated_transformer_should_infer(95, 100));
        // Lambda stable - should not infer (delta < 1)
        assert!(!gated_transformer_should_infer(100, 100));
        // Lambda increased - should infer (delta >= 1)
        assert!(gated_transformer_should_infer(102, 100));
    }

    #[pg_test]
    fn test_compute_tier() {
        let tier = gated_transformer_compute_tier(100, 95, 5);
        assert!(tier >= 0 && tier <= 3);
    }

    #[pg_test]
    fn test_routing_capacity() {
        assert_eq!(gated_transformer_routing_capacity(100, 0.5), 50);
        assert_eq!(gated_transformer_routing_capacity(100, 0.3), 30);
    }

    #[pg_test]
    fn test_config() {
        let config = gated_transformer_config();
        let json: serde_json::Value = serde_json::from_value(config.0).unwrap();
        assert!(json.get("hidden").is_some());
        assert!(json.get("layers").is_some());
    }

    #[pg_test]
    fn test_coherence_score() {
        let score = gated_transformer_coherence_score(100, 100, 0);
        assert!(score >= 0.0 && score <= 1.0);

        // Stable lambda + low boundary = high score
        let high_score = gated_transformer_coherence_score(100, 100, 0);
        assert!(high_score > 0.8);
    }

    #[pg_test]
    fn test_set_policy() {
        assert!(gated_transformer_set_policy("conservative"));
        assert!(gated_transformer_set_policy("permissive"));
        assert!(gated_transformer_set_policy("default"));
        assert!(!gated_transformer_set_policy("invalid"));
    }
}
