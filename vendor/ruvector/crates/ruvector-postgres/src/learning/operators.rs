//! PostgreSQL operator functions for self-learning

use pgrx::prelude::*;
use pgrx::JsonB;
use serde::{Deserialize, Serialize};

use super::optimizer::OptimizationTarget;
use super::{QueryTrajectory, LEARNING_MANAGER};

/// Configuration for enabling learning
#[derive(Debug, Serialize, Deserialize)]
pub struct LearningConfig {
    /// Maximum number of trajectories to track
    #[serde(default = "default_max_trajectories")]
    pub max_trajectories: usize,
    /// Number of clusters for pattern extraction
    #[serde(default = "default_num_clusters")]
    pub num_clusters: usize,
    /// Auto-tune interval in seconds (0 = disabled)
    #[serde(default)]
    pub auto_tune_interval: u64,
}

fn default_max_trajectories() -> usize {
    1000
}
fn default_num_clusters() -> usize {
    10
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            max_trajectories: 1000,
            num_clusters: 10,
            auto_tune_interval: 0,
        }
    }
}

/// Enable learning for a table
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_enable_learning('my_table', '{"max_trajectories": 2000}'::jsonb);
/// ```
#[pg_extern]
fn ruvector_enable_learning(
    table_name: &str,
    config: Option<JsonB>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let config: LearningConfig = match config {
        Some(jsonb) => serde_json::from_value(jsonb.0.clone())?,
        None => LearningConfig::default(),
    };

    LEARNING_MANAGER.enable_for_table(table_name, config.max_trajectories);

    Ok(format!(
        "Learning enabled for table '{}' with max_trajectories={}",
        table_name, config.max_trajectories
    ))
}

/// Record relevance feedback for a query
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_record_feedback(
///     'my_table',
///     ARRAY[0.1, 0.2, 0.3],
///     ARRAY[1, 2, 3]::bigint[],
///     ARRAY[4, 5]::bigint[]
/// );
/// ```
#[pg_extern]
fn ruvector_record_feedback(
    table_name: &str,
    query_vector: Vec<f32>,
    relevant_ids: Vec<i64>,
    irrelevant_ids: Vec<i64>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let tracker = LEARNING_MANAGER
        .get_tracker(table_name)
        .ok_or_else(|| format!("Learning not enabled for table: {}", table_name))?;

    // Find the most recent trajectory matching this query
    let mut recent = tracker.get_recent(10);

    // Find matching trajectory (same query vector)
    if let Some(traj) = recent.iter_mut().find(|t| t.query_vector == query_vector) {
        traj.add_feedback(
            relevant_ids.iter().map(|&id| id as u64).collect(),
            irrelevant_ids.iter().map(|&id| id as u64).collect(),
        );

        // Re-record the updated trajectory
        tracker.record(traj.clone());

        Ok(format!(
            "Feedback recorded: {} relevant, {} irrelevant",
            relevant_ids.len(),
            irrelevant_ids.len()
        ))
    } else {
        Err("No recent trajectory found matching query vector".into())
    }
}

/// Get learning statistics for a table
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_learning_stats('my_table');
/// ```
#[pg_extern]
fn ruvector_learning_stats(
    table_name: &str,
) -> Result<JsonB, Box<dyn std::error::Error + Send + Sync>> {
    let tracker = LEARNING_MANAGER
        .get_tracker(table_name)
        .ok_or_else(|| format!("Learning not enabled for table: {}", table_name))?;

    let bank = LEARNING_MANAGER
        .get_reasoning_bank(table_name)
        .ok_or_else(|| format!("ReasoningBank not found for table: {}", table_name))?;

    let trajectory_stats = tracker.stats();
    let bank_stats = bank.stats();

    let stats = serde_json::json!({
        "trajectories": {
            "total": trajectory_stats.total_trajectories,
            "with_feedback": trajectory_stats.trajectories_with_feedback,
            "avg_latency_us": trajectory_stats.avg_latency_us,
            "avg_precision": trajectory_stats.avg_precision,
            "avg_recall": trajectory_stats.avg_recall,
        },
        "patterns": {
            "total": bank_stats.total_patterns,
            "total_samples": bank_stats.total_samples,
            "avg_confidence": bank_stats.avg_confidence,
            "total_usage": bank_stats.total_usage,
        }
    });

    Ok(JsonB(stats))
}

/// Auto-tune search parameters for optimal performance
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_auto_tune(
///     'my_table',
///     'balanced',
///     '[[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]]'::jsonb
/// );
/// ```
#[pg_extern]
fn ruvector_auto_tune(
    table_name: &str,
    optimize_for: default!(&str, "'balanced'"),
    sample_queries: Option<JsonB>,
) -> Result<JsonB, Box<dyn std::error::Error + Send + Sync>> {
    let optimizer = LEARNING_MANAGER
        .get_optimizer(table_name)
        .ok_or_else(|| format!("Learning not enabled for table: {}", table_name))?;

    let target = match optimize_for {
        "speed" => OptimizationTarget::Speed,
        "accuracy" => OptimizationTarget::Accuracy,
        _ => OptimizationTarget::Balanced,
    };

    // Extract patterns first
    let patterns_extracted = LEARNING_MANAGER.extract_patterns(table_name, 10)?;

    let mut recommendations = Vec::new();

    if let Some(JsonB(json_val)) = sample_queries {
        // Parse JSON array of arrays as Vec<Vec<f32>>
        if let Some(queries_array) = json_val.as_array() {
            for query_val in queries_array {
                if let Some(query_array) = query_val.as_array() {
                    let query: Vec<f32> = query_array
                        .iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect();
                    let params = optimizer.optimize_with_target(&query, target);
                    recommendations.push(serde_json::json!({
                        "ef_search": params.ef_search,
                        "probes": params.probes,
                        "confidence": params.confidence,
                    }));
                }
            }
        }
    }

    let result = serde_json::json!({
        "patterns_extracted": patterns_extracted,
        "optimize_for": optimize_for,
        "recommendations": recommendations,
    });

    Ok(JsonB(result))
}

/// Consolidate similar patterns to reduce memory usage
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_consolidate_patterns('my_table', 0.95);
/// ```
#[pg_extern]
fn ruvector_consolidate_patterns(
    table_name: &str,
    similarity_threshold: default!(f64, 0.9),
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let bank = LEARNING_MANAGER
        .get_reasoning_bank(table_name)
        .ok_or_else(|| format!("Learning not enabled for table: {}", table_name))?;

    let merged = bank.consolidate(similarity_threshold);

    Ok(format!(
        "Consolidated {} similar patterns with threshold {}",
        merged, similarity_threshold
    ))
}

/// Prune low-quality patterns
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_prune_patterns('my_table', 5, 0.5);
/// ```
#[pg_extern]
fn ruvector_prune_patterns(
    table_name: &str,
    min_usage: default!(i32, 5),
    min_confidence: default!(f64, 0.5),
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let bank = LEARNING_MANAGER
        .get_reasoning_bank(table_name)
        .ok_or_else(|| format!("Learning not enabled for table: {}", table_name))?;

    let pruned = bank.prune(min_usage as usize, min_confidence);

    Ok(format!(
        "Pruned {} patterns with min_usage={}, min_confidence={}",
        pruned, min_usage, min_confidence
    ))
}

/// Get optimized search parameters for a query
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_get_search_params('my_table', ARRAY[0.1, 0.2, 0.3]);
/// ```
#[pg_extern]
fn ruvector_get_search_params(
    table_name: &str,
    query_vector: Vec<f32>,
) -> Result<JsonB, Box<dyn std::error::Error + Send + Sync>> {
    let optimizer = LEARNING_MANAGER
        .get_optimizer(table_name)
        .ok_or_else(|| format!("Learning not enabled for table: {}", table_name))?;

    let params = optimizer.optimize(&query_vector);

    let result = serde_json::json!({
        "ef_search": params.ef_search,
        "probes": params.probes,
        "confidence": params.confidence,
    });

    Ok(JsonB(result))
}

/// Extract patterns from collected trajectories
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_extract_patterns('my_table', 10);
/// ```
#[pg_extern]
fn ruvector_extract_patterns(
    table_name: &str,
    num_clusters: default!(i32, 10),
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let patterns_extracted =
        LEARNING_MANAGER.extract_patterns(table_name, num_clusters as usize)?;

    Ok(format!(
        "Extracted {} patterns from trajectories using {} clusters",
        patterns_extracted, num_clusters
    ))
}

/// Record a query trajectory for learning
///
/// This is typically called internally by search functions, but can be used manually
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_record_trajectory(
///     'my_table',
///     ARRAY[0.1, 0.2, 0.3],
///     ARRAY[1, 2, 3]::bigint[],
///     1500,
///     50,
///     10
/// );
/// ```
#[pg_extern]
fn ruvector_record_trajectory(
    table_name: &str,
    query_vector: Vec<f32>,
    result_ids: Vec<i64>,
    latency_us: i64,
    ef_search: i32,
    probes: i32,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let tracker = LEARNING_MANAGER
        .get_tracker(table_name)
        .ok_or_else(|| format!("Learning not enabled for table: {}", table_name))?;

    let trajectory = QueryTrajectory::new(
        query_vector,
        result_ids.iter().map(|&id| id as u64).collect(),
        latency_us as u64,
        ef_search as usize,
        probes as usize,
    );

    tracker.record(trajectory);

    Ok(format!(
        "Trajectory recorded for {} results",
        result_ids.len()
    ))
}

/// Clear all learning data for a table
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_clear_learning('my_table');
/// ```
#[pg_extern]
fn ruvector_clear_learning(
    table_name: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let bank = LEARNING_MANAGER
        .get_reasoning_bank(table_name)
        .ok_or_else(|| format!("Learning not enabled for table: {}", table_name))?;

    bank.clear();

    Ok(format!(
        "Cleared all learning data for table '{}'",
        table_name
    ))
}

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_enable_learning() {
        let result = ruvector_enable_learning("test_table", None);
        assert!(result.is_ok());
    }

    #[pg_test]
    fn test_learning_stats_empty() {
        ruvector_enable_learning("test_stats", None).unwrap();
        let stats = ruvector_learning_stats("test_stats");
        assert!(stats.is_ok());
    }

    #[pg_test]
    fn test_record_trajectory() {
        ruvector_enable_learning("test_trajectory", None).unwrap();

        let result = ruvector_record_trajectory(
            "test_trajectory",
            vec![1.0, 2.0, 3.0],
            vec![1, 2, 3],
            1000,
            50,
            10,
        );

        assert!(result.is_ok());
    }

    #[pg_test]
    fn test_extract_patterns() {
        ruvector_enable_learning("test_patterns", None).unwrap();

        // Record some trajectories
        for i in 0..20 {
            ruvector_record_trajectory(
                "test_patterns",
                vec![i as f32, (i * 2) as f32],
                vec![i, i + 1],
                1000 + i * 100,
                50,
                10,
            )
            .unwrap();
        }

        let result = ruvector_extract_patterns("test_patterns", 5);
        assert!(result.is_ok());
    }

    #[pg_test]
    fn test_auto_tune() {
        ruvector_enable_learning("test_autotune", None).unwrap();

        // Record some trajectories
        for i in 0..10 {
            ruvector_record_trajectory(
                "test_autotune",
                vec![i as f32, (i * 2) as f32],
                vec![i],
                1000,
                50,
                10,
            )
            .unwrap();
        }

        let result = ruvector_auto_tune("test_autotune", "balanced", None);

        assert!(result.is_ok());
    }

    #[pg_test]
    fn test_get_search_params() {
        ruvector_enable_learning("test_search_params", None).unwrap();

        // Record and extract patterns first
        for i in 0..20 {
            ruvector_record_trajectory(
                "test_search_params",
                vec![i as f32, 0.0],
                vec![i],
                1000,
                50,
                10,
            )
            .unwrap();
        }

        ruvector_extract_patterns("test_search_params", 3).unwrap();

        let result = ruvector_get_search_params("test_search_params", vec![5.0, 0.0]);

        assert!(result.is_ok());
    }

    #[pg_test]
    fn test_consolidate_patterns() {
        ruvector_enable_learning("test_consolidate", None).unwrap();

        // Record trajectories and extract patterns
        for i in 0..30 {
            ruvector_record_trajectory(
                "test_consolidate",
                vec![i as f32 / 10.0, 0.0],
                vec![i],
                1000,
                50,
                10,
            )
            .unwrap();
        }

        ruvector_extract_patterns("test_consolidate", 10).unwrap();

        let result = ruvector_consolidate_patterns("test_consolidate", 0.95);
        assert!(result.is_ok());
    }

    #[pg_test]
    fn test_prune_patterns() {
        ruvector_enable_learning("test_prune", None).unwrap();

        // Record trajectories and extract patterns
        for i in 0..20 {
            ruvector_record_trajectory("test_prune", vec![i as f32, 0.0], vec![i], 1000, 50, 10)
                .unwrap();
        }

        ruvector_extract_patterns("test_prune", 5).unwrap();

        let result = ruvector_prune_patterns("test_prune", 100, 0.9);
        assert!(result.is_ok());
    }

    #[pg_test]
    fn test_clear_learning() {
        ruvector_enable_learning("test_clear", None).unwrap();

        ruvector_record_trajectory("test_clear", vec![1.0, 2.0], vec![1], 1000, 50, 10).unwrap();

        let result = ruvector_clear_learning("test_clear");
        assert!(result.is_ok());

        let stats = ruvector_learning_stats("test_clear").unwrap();
        let stats_obj = stats.0.as_object().unwrap();
        let patterns = stats_obj.get("patterns").unwrap().as_object().unwrap();
        assert_eq!(patterns.get("total").unwrap().as_u64().unwrap(), 0);
    }
}
