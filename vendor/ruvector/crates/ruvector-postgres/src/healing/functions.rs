//! SQL Functions for Self-Healing Engine
//!
//! Provides PostgreSQL-accessible functions for:
//! - Health status monitoring
//! - Healing history queries
//! - Manual healing triggers
//! - Configuration management

use pgrx::prelude::*;

use super::detector::ProblemType;
use super::{get_healing_engine, Problem};

// ============================================================================
// Health Status Functions
// ============================================================================

/// Get current health status of the RuVector system
///
/// Returns JSON with:
/// - healthy: whether system is healthy
/// - problem_count: number of detected problems
/// - active_remediation_count: ongoing remediations
/// - problems: list of current problems
/// - enabled: whether healing is enabled
#[pg_extern]
pub fn ruvector_health_status() -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();
    let status = engine_lock.health_status();
    pgrx::JsonB(status.to_json())
}

/// Check if system is currently healthy (no detected problems)
#[pg_extern]
pub fn ruvector_is_healthy() -> bool {
    let engine = get_healing_engine();
    let engine_lock = engine.read();
    let status = engine_lock.health_status();
    status.healthy
}

/// Get system metrics used for problem detection
#[pg_extern]
pub fn ruvector_system_metrics() -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();
    let metrics = engine_lock.detector.collect_metrics();
    pgrx::JsonB(metrics.to_json())
}

// ============================================================================
// Healing History Functions
// ============================================================================

/// Get recent healing history
///
/// # Arguments
/// * `limit` - Maximum number of records to return (default 20)
#[pg_extern]
pub fn ruvector_healing_history(limit: default!(i32, 20)) -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    let records = engine_lock.tracker.get_recent(limit as usize);
    let history: Vec<serde_json::Value> = records.iter().map(|r| r.to_json()).collect();

    pgrx::JsonB(serde_json::json!({
        "history": history,
        "count": history.len(),
    }))
}

/// Get healing history since a specific timestamp
///
/// # Arguments
/// * `since_timestamp` - Unix timestamp to filter from
#[pg_extern]
pub fn ruvector_healing_history_since(since_timestamp: i64) -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    let records = engine_lock.tracker.get_since(since_timestamp as u64);
    let history: Vec<serde_json::Value> = records.iter().map(|r| r.to_json()).collect();

    pgrx::JsonB(serde_json::json!({
        "history": history,
        "count": history.len(),
        "since": since_timestamp,
    }))
}

/// Get healing history for a specific strategy
#[pg_extern]
pub fn ruvector_healing_history_for_strategy(
    strategy_name: &str,
    limit: default!(i32, 20),
) -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    let records = engine_lock
        .tracker
        .get_for_strategy(strategy_name, limit as usize);
    let history: Vec<serde_json::Value> = records.iter().map(|r| r.to_json()).collect();

    pgrx::JsonB(serde_json::json!({
        "strategy": strategy_name,
        "history": history,
        "count": history.len(),
    }))
}

// ============================================================================
// Healing Trigger Functions
// ============================================================================

/// Manually trigger healing for a specific problem type
///
/// # Arguments
/// * `problem_type` - One of: index_degradation, replica_lag, storage_exhaustion,
///                   query_timeout, integrity_violation, memory_pressure,
///                   connection_exhaustion, hot_partition
#[pg_extern]
pub fn ruvector_healing_trigger(problem_type: &str) -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    // Parse problem type
    let ptype = match problem_type.parse::<ProblemType>() {
        Ok(pt) => pt,
        Err(e) => {
            return pgrx::JsonB(serde_json::json!({
                "success": false,
                "error": e,
            }));
        }
    };

    // Trigger healing
    match engine_lock.trigger_healing(ptype) {
        Some(outcome) => pgrx::JsonB(serde_json::json!({
            "success": true,
            "outcome": outcome.to_json(),
        })),
        None => pgrx::JsonB(serde_json::json!({
            "success": false,
            "error": "Healing is disabled",
        })),
    }
}

/// Execute a specific healing strategy manually
///
/// # Arguments
/// * `strategy_name` - Strategy to execute
/// * `problem_type` - Problem type for context
/// * `dry_run` - If true, don't actually execute (default false)
#[pg_extern]
pub fn ruvector_healing_execute(
    strategy_name: &str,
    problem_type: &str,
    dry_run: default!(bool, false),
) -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    // Parse problem type
    let ptype = match problem_type.parse::<ProblemType>() {
        Ok(pt) => pt,
        Err(e) => {
            return pgrx::JsonB(serde_json::json!({
                "success": false,
                "error": e,
            }));
        }
    };

    let problem = Problem::new(ptype, super::detector::Severity::Medium);

    match engine_lock
        .remediation
        .execute_strategy(strategy_name, &problem, dry_run)
    {
        Some(outcome) => pgrx::JsonB(serde_json::json!({
            "success": true,
            "dry_run": dry_run,
            "outcome": outcome.to_json(),
        })),
        None => pgrx::JsonB(serde_json::json!({
            "success": false,
            "error": format!("Strategy '{}' not found", strategy_name),
        })),
    }
}

// ============================================================================
// Configuration Functions
// ============================================================================

/// Configure healing engine settings
///
/// # Arguments
/// * `config_json` - JSON configuration object with optional keys:
///   - min_healing_interval_secs
///   - max_attempts_per_window
///   - max_auto_heal_impact
///   - learning_enabled
///   - verify_improvement
///   - min_improvement_pct
#[pg_extern]
pub fn ruvector_healing_configure(config_json: pgrx::JsonB) -> pgrx::JsonB {
    let engine = get_healing_engine();
    let mut engine_lock = engine.write();

    let mut config = engine_lock.config.clone();
    let json = config_json.0;

    // Update configuration from JSON
    if let Some(interval) = json
        .get("min_healing_interval_secs")
        .and_then(|v| v.as_i64())
    {
        if interval > 0 {
            config.min_healing_interval = std::time::Duration::from_secs(interval as u64);
        }
    }

    if let Some(attempts) = json.get("max_attempts_per_window").and_then(|v| v.as_i64()) {
        if attempts > 0 {
            config.max_attempts_per_window = attempts as usize;
        }
    }

    if let Some(impact) = json.get("max_auto_heal_impact").and_then(|v| v.as_f64()) {
        if impact >= 0.0 && impact <= 1.0 {
            config.max_auto_heal_impact = impact as f32;
        }
    }

    if let Some(learning) = json.get("learning_enabled").and_then(|v| v.as_bool()) {
        config.learning_enabled = learning;
    }

    if let Some(verify) = json.get("verify_improvement").and_then(|v| v.as_bool()) {
        config.verify_improvement = verify;
    }

    if let Some(min_pct) = json.get("min_improvement_pct").and_then(|v| v.as_f64()) {
        if min_pct >= 0.0 {
            config.min_improvement_pct = min_pct as f32;
        }
    }

    if let Some(enabled) = json.get("enabled").and_then(|v| v.as_bool()) {
        engine_lock.set_enabled(enabled);
    }

    engine_lock.update_config(config.clone());

    pgrx::JsonB(serde_json::json!({
        "status": "updated",
        "config": {
            "min_healing_interval_secs": config.min_healing_interval.as_secs(),
            "max_attempts_per_window": config.max_attempts_per_window,
            "max_auto_heal_impact": config.max_auto_heal_impact,
            "learning_enabled": config.learning_enabled,
            "verify_improvement": config.verify_improvement,
            "min_improvement_pct": config.min_improvement_pct,
            "enabled": engine_lock.enabled,
        }
    }))
}

/// Get current healing configuration
#[pg_extern]
pub fn ruvector_healing_get_config() -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();
    let config = &engine_lock.config;

    pgrx::JsonB(serde_json::json!({
        "min_healing_interval_secs": config.min_healing_interval.as_secs(),
        "max_attempts_per_window": config.max_attempts_per_window,
        "attempt_window_secs": config.attempt_window.as_secs(),
        "max_auto_heal_impact": config.max_auto_heal_impact,
        "learning_enabled": config.learning_enabled,
        "failure_cooldown_secs": config.failure_cooldown.as_secs(),
        "verify_improvement": config.verify_improvement,
        "min_improvement_pct": config.min_improvement_pct,
        "max_concurrent_remediations": config.max_concurrent_remediations,
        "require_approval_strategies": config.require_approval_strategies,
        "enabled": engine_lock.enabled,
    }))
}

/// Enable or disable healing
#[pg_extern]
pub fn ruvector_healing_enable(enabled: bool) -> bool {
    let engine = get_healing_engine();
    let mut engine_lock = engine.write();
    engine_lock.set_enabled(enabled);
    engine_lock.enabled
}

// ============================================================================
// Strategy Functions
// ============================================================================

/// List all available healing strategies
#[pg_extern]
pub fn ruvector_healing_strategies() -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    let strategies: Vec<serde_json::Value> = engine_lock
        .remediation
        .registry
        .all_strategies()
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name(),
                "description": s.description(),
                "handles": s.handles().iter().map(|h| h.to_string()).collect::<Vec<_>>(),
                "impact": s.impact(),
                "estimated_duration_secs": s.estimated_duration().as_secs(),
                "reversible": s.reversible(),
                "weight": engine_lock.remediation.registry.get_weight(s.name()),
            })
        })
        .collect();

    pgrx::JsonB(serde_json::json!({
        "strategies": strategies,
        "count": strategies.len(),
    }))
}

/// Get effectiveness report for all strategies
#[pg_extern]
pub fn ruvector_healing_effectiveness() -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    let report = engine_lock.tracker.effectiveness_report();
    pgrx::JsonB(report.to_json())
}

/// Get statistics for the healing engine
#[pg_extern]
pub fn ruvector_healing_stats() -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    let engine_stats = engine_lock.remediation.get_stats();
    let tracker_stats = engine_lock.tracker.get_stats();

    pgrx::JsonB(serde_json::json!({
        "engine": engine_stats.to_json(),
        "tracker": tracker_stats.to_json(),
    }))
}

// ============================================================================
// Detection Threshold Functions
// ============================================================================

/// Get current detection thresholds
#[pg_extern]
pub fn ruvector_healing_thresholds() -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    let thresholds = engine_lock.detector.get_thresholds();

    pgrx::JsonB(serde_json::json!({
        "index_fragmentation_pct": thresholds.index_fragmentation_pct,
        "replica_lag_seconds": thresholds.replica_lag_seconds,
        "storage_usage_pct": thresholds.storage_usage_pct,
        "query_timeout_rate": thresholds.query_timeout_rate,
        "min_integrity_lambda": thresholds.min_integrity_lambda,
        "memory_usage_pct": thresholds.memory_usage_pct,
        "connection_usage_pct": thresholds.connection_usage_pct,
        "partition_load_ratio": thresholds.partition_load_ratio,
    }))
}

/// Update detection thresholds
#[pg_extern]
pub fn ruvector_healing_set_thresholds(thresholds_json: pgrx::JsonB) -> pgrx::JsonB {
    let engine = get_healing_engine();
    let engine_lock = engine.read();

    let mut thresholds = engine_lock.detector.get_thresholds();
    let json = thresholds_json.0;

    if let Some(v) = json.get("index_fragmentation_pct").and_then(|v| v.as_f64()) {
        thresholds.index_fragmentation_pct = v as f32;
    }
    if let Some(v) = json.get("replica_lag_seconds").and_then(|v| v.as_f64()) {
        thresholds.replica_lag_seconds = v as f32;
    }
    if let Some(v) = json.get("storage_usage_pct").and_then(|v| v.as_f64()) {
        thresholds.storage_usage_pct = v as f32;
    }
    if let Some(v) = json.get("query_timeout_rate").and_then(|v| v.as_f64()) {
        thresholds.query_timeout_rate = v as f32;
    }
    if let Some(v) = json.get("min_integrity_lambda").and_then(|v| v.as_f64()) {
        thresholds.min_integrity_lambda = v as f32;
    }
    if let Some(v) = json.get("memory_usage_pct").and_then(|v| v.as_f64()) {
        thresholds.memory_usage_pct = v as f32;
    }
    if let Some(v) = json.get("connection_usage_pct").and_then(|v| v.as_f64()) {
        thresholds.connection_usage_pct = v as f32;
    }
    if let Some(v) = json.get("partition_load_ratio").and_then(|v| v.as_f64()) {
        thresholds.partition_load_ratio = v as f32;
    }

    engine_lock.detector.update_thresholds(thresholds.clone());

    pgrx::JsonB(serde_json::json!({
        "status": "updated",
        "thresholds": {
            "index_fragmentation_pct": thresholds.index_fragmentation_pct,
            "replica_lag_seconds": thresholds.replica_lag_seconds,
            "storage_usage_pct": thresholds.storage_usage_pct,
            "query_timeout_rate": thresholds.query_timeout_rate,
            "min_integrity_lambda": thresholds.min_integrity_lambda,
            "memory_usage_pct": thresholds.memory_usage_pct,
            "connection_usage_pct": thresholds.connection_usage_pct,
            "partition_load_ratio": thresholds.partition_load_ratio,
        }
    }))
}

// ============================================================================
// Problem Type Reference
// ============================================================================

/// List all supported problem types
#[pg_extern]
pub fn ruvector_healing_problem_types() -> pgrx::JsonB {
    let types: Vec<serde_json::Value> = ProblemType::all()
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.to_string(),
                "description": t.description(),
            })
        })
        .collect();

    pgrx::JsonB(serde_json::json!({
        "problem_types": types,
        "count": types.len(),
    }))
}

#[cfg(test)]
mod tests {
    // These tests would run in a PostgreSQL context with pg_test
    // For now, they verify the function signatures compile correctly
}
