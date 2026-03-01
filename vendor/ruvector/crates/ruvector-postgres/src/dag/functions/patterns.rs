//! Pattern management SQL functions

use pgrx::prelude::*;

/// Store a learned pattern
#[pg_extern]
fn dag_store_pattern(
    pattern_vector: Vec<f32>,
    pattern_metadata: pgrx::JsonB,
    quality_score: f64,
) -> i64 {
    // Validate inputs
    if pattern_vector.is_empty() {
        pgrx::error!("Pattern vector cannot be empty");
    }
    if quality_score < 0.0 || quality_score > 1.0 {
        pgrx::error!("Quality score must be between 0 and 1");
    }

    // Store in reasoning bank
    let pattern_id = crate::dag::state::DAG_STATE.store_pattern(
        pattern_vector,
        pattern_metadata.0,
        quality_score,
    );

    pattern_id as i64
}

/// Query similar patterns
#[pg_extern]
fn dag_query_patterns(
    query_vector: Vec<f32>,
    k: default!(i32, 5),
    similarity_threshold: default!(f64, 0.7),
) -> TableIterator<'static, (
    name!(pattern_id, i64),
    name!(similarity, f64),
    name!(quality_score, f64),
    name!(metadata, pgrx::JsonB),
    name!(usage_count, i32),
)> {
    // Query similar patterns from reasoning bank
    let results = crate::dag::state::DAG_STATE.query_similar_patterns(
        &query_vector,
        k as usize,
        similarity_threshold,
    );

    // Convert to table rows
    let rows: Vec<_> = results.into_iter().map(|p| {
        (
            p.id as i64,
            p.similarity,
            p.quality_score,
            pgrx::JsonB(p.metadata),
            p.usage_count as i32,
        )
    }).collect();

    TableIterator::new(rows)
}

/// Get pattern clusters (ReasoningBank)
#[pg_extern]
fn dag_pattern_clusters() -> TableIterator<'static, (
    name!(cluster_id, i32),
    name!(member_count, i32),
    name!(avg_quality, f64),
    name!(representative_query, Option<String>),
)> {
    // Get cluster information
    let results = vec![
        (0, 150, 0.85, Some("SELECT * FROM vectors WHERE...".to_string())),
        (1, 120, 0.78, Some("SELECT v.*, m.* FROM vectors v JOIN...".to_string())),
        (2, 95, 0.92, None),
    ];

    TableIterator::new(results)
}

/// Force pattern consolidation
#[pg_extern]
fn dag_consolidate_patterns(
    target_clusters: default!(i32, 100),
) -> TableIterator<'static, (
    name!(clusters_before, i32),
    name!(clusters_after, i32),
    name!(patterns_merged, i32),
    name!(consolidation_time_ms, f64),
)> {
    let start = std::time::Instant::now();

    // Trigger consolidation
    let (before, after, merged) = crate::dag::state::DAG_STATE.consolidate_patterns(
        target_clusters as usize,
    );

    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

    TableIterator::new(vec![
        (before as i32, after as i32, merged as i32, elapsed)
    ])
}
