//! PostgreSQL operator functions for Sona self-learning.

use pgrx::prelude::*;
use pgrx::JsonB;

use super::get_or_create_engine;

/// Record a learning trajectory for a table (Micro-LoRA).
#[pg_extern]
pub fn ruvector_sona_learn(table_name: &str, trajectory_json: JsonB) -> JsonB {
    let engine = get_or_create_engine(table_name);

    // Parse trajectory: {"initial": [f32...], "steps": [{"embedding": [f32...], "actions": [...], "reward": f32}]}
    let initial: Vec<f32> = trajectory_json
        .0
        .get("initial")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_f64().map(|f| f as f32))
                .collect()
        })
        .unwrap_or_else(|| vec![0.0; 256]);

    let steps = trajectory_json
        .0
        .get("steps")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Begin trajectory
    let mut builder = engine.begin_trajectory(initial);

    for step in &steps {
        let embedding: Vec<f32> = step
            .get("embedding")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_f64().map(|f| f as f32))
                    .collect()
            })
            .unwrap_or_else(|| vec![0.0; 256]);

        let attention_weights: Vec<f32> = step
            .get("attention_weights")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_f64().map(|f| f as f32))
                    .collect()
            })
            .unwrap_or_default();

        let reward = step.get("reward").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

        builder.add_step(embedding, attention_weights, reward);
    }

    let final_reward = trajectory_json
        .0
        .get("final_reward")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5) as f32;

    engine.end_trajectory(builder, final_reward);

    JsonB(serde_json::json!({
        "status": "learned",
        "table": table_name,
        "steps": steps.len(),
        "final_reward": final_reward,
    }))
}

/// Apply learned LoRA transformation to an embedding.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_sona_apply(table_name: &str, embedding: Vec<f32>) -> Vec<f32> {
    let engine = get_or_create_engine(table_name);

    let mut output = vec![0.0f32; embedding.len()];
    engine.apply_micro_lora(&embedding, &mut output);

    // If output is all zeros (no learned weights yet), return the input
    if output.iter().all(|&x| x == 0.0) {
        return embedding;
    }

    output
}

/// Get EWC++ forgetting metrics for a table.
#[pg_extern]
pub fn ruvector_sona_ewc_status(table_name: &str) -> JsonB {
    let engine = get_or_create_engine(table_name);
    let stats = engine.stats();

    JsonB(serde_json::json!({
        "table": table_name,
        "ewc_tasks": stats.ewc_tasks,
        "trajectories_buffered": stats.trajectories_buffered,
        "trajectories_dropped": stats.trajectories_dropped,
        "patterns_stored": stats.patterns_stored,
        "buffer_success_rate": stats.buffer_success_rate,
    }))
}

/// Get Sona engine statistics for a table.
#[pg_extern]
pub fn ruvector_sona_stats(table_name: &str) -> JsonB {
    let engine = get_or_create_engine(table_name);
    let stats = engine.stats();
    let config = engine.config();

    JsonB(serde_json::json!({
        "table": table_name,
        "trajectories_buffered": stats.trajectories_buffered,
        "trajectories_dropped": stats.trajectories_dropped,
        "buffer_success_rate": stats.buffer_success_rate,
        "patterns_stored": stats.patterns_stored,
        "ewc_tasks": stats.ewc_tasks,
        "instant_enabled": stats.instant_enabled,
        "background_enabled": stats.background_enabled,
        "hidden_dim": config.hidden_dim,
        "embedding_dim": config.embedding_dim,
        "micro_lora_rank": config.micro_lora_rank,
        "base_lora_rank": config.base_lora_rank,
    }))
}
