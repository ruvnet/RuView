//! Configuration SQL functions for neural DAG learning

use pgrx::prelude::*;

/// Enable or disable neural DAG learning
#[pg_extern]
fn dag_set_enabled(enabled: bool) {
    crate::dag::state::DAG_STATE.set_enabled(enabled);

    if enabled {
        pgrx::notice!("Neural DAG learning enabled");
    } else {
        pgrx::notice!("Neural DAG learning disabled");
    }
}

/// Set the learning rate for neural adaptation
#[pg_extern]
fn dag_set_learning_rate(rate: f64) {
    if !(0.0..=1.0).contains(&rate) {
        pgrx::error!("Learning rate must be between 0 and 1, got {}", rate);
    }
    crate::dag::state::DAG_STATE.set_learning_rate(rate);
    pgrx::notice!("Learning rate set to {}", rate);
}

/// Set the attention mechanism for query analysis
#[pg_extern]
fn dag_set_attention(mechanism: &str) {
    let valid_mechanisms = [
        "topological",
        "causal_cone",
        "critical_path",
        "mincut_gated",
        "hierarchical_lorentz",
        "parallel_branch",
        "temporal_btsp",
        "auto",
    ];

    if !valid_mechanisms.contains(&mechanism) {
        pgrx::error!(
            "Invalid attention mechanism '{}'. Valid options: {:?}",
            mechanism,
            valid_mechanisms
        );
    }

    crate::dag::state::DAG_STATE.set_attention_mechanism(mechanism.to_string());
    pgrx::notice!("Attention mechanism set to '{}'", mechanism);
}

/// Configure SONA (Scalable On-device Neural Adaptation) parameters
#[pg_extern]
fn dag_configure_sona(
    micro_lora_rank: default!(i32, 2),
    base_lora_rank: default!(i32, 8),
    ewc_lambda: default!(f64, 5000.0),
    pattern_clusters: default!(i32, 100),
) {
    // Validation
    if !(1..=4).contains(&micro_lora_rank) {
        pgrx::error!(
            "micro_lora_rank must be between 1 and 4, got {}",
            micro_lora_rank
        );
    }
    if !(4..=16).contains(&base_lora_rank) {
        pgrx::error!(
            "base_lora_rank must be between 4 and 16, got {}",
            base_lora_rank
        );
    }
    if ewc_lambda < 0.0 {
        pgrx::error!("ewc_lambda must be non-negative, got {}", ewc_lambda);
    }
    if !(10..=1000).contains(&pattern_clusters) {
        pgrx::error!(
            "pattern_clusters must be between 10 and 1000, got {}",
            pattern_clusters
        );
    }

    // Store in state
    crate::dag::state::DAG_STATE.configure_sona(
        micro_lora_rank,
        base_lora_rank,
        ewc_lambda,
        pattern_clusters,
    );

    pgrx::notice!(
        "SONA configured: micro_lora_rank={}, base_lora_rank={}, ewc_lambda={}, pattern_clusters={}",
        micro_lora_rank, base_lora_rank, ewc_lambda, pattern_clusters
    );
}

/// Get current configuration as JSON
#[pg_extern]
fn dag_config() -> pgrx::JsonB {
    let config = crate::dag::state::DAG_STATE.get_config();

    let json = serde_json::json!({
        "enabled": config.enabled,
        "learning_rate": config.learning_rate,
        "attention_mechanism": config.attention_mechanism,
        "sona": {
            "micro_lora_rank": config.micro_lora_rank,
            "base_lora_rank": config.base_lora_rank,
            "ewc_lambda": config.ewc_lambda,
            "pattern_clusters": config.pattern_clusters,
        }
    });

    pgrx::JsonB(json)
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_dag_set_enabled() {
        dag_set_enabled(true);
        assert!(crate::dag::state::DAG_STATE.is_enabled());

        dag_set_enabled(false);
        assert!(!crate::dag::state::DAG_STATE.is_enabled());
    }

    #[pg_test]
    fn test_dag_set_learning_rate() {
        dag_set_learning_rate(0.05);
        assert!((crate::dag::state::DAG_STATE.get_learning_rate() - 0.05).abs() < 1e-10);
    }

    #[pg_test]
    #[should_panic(expected = "Learning rate must be between 0 and 1")]
    fn test_dag_set_learning_rate_invalid() {
        dag_set_learning_rate(1.5);
    }

    #[pg_test]
    fn test_dag_set_attention() {
        dag_set_attention("topological");
        assert_eq!(
            crate::dag::state::DAG_STATE.get_attention_mechanism(),
            "topological"
        );
    }

    #[pg_test]
    #[should_panic(expected = "Invalid attention mechanism")]
    fn test_dag_set_attention_invalid() {
        dag_set_attention("invalid_mechanism");
    }
}
