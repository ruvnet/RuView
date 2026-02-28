//! Learning control SQL functions

use pgrx::prelude::*;

/// Trigger immediate learning cycle
#[pg_extern]
fn dag_learn_now() -> TableIterator<'static, (
    name!(patterns_updated, i32),
    name!(new_clusters, i32),
    name!(ewc_constraints_updated, i32),
    name!(cycle_time_ms, f64),
)> {
    let start = std::time::Instant::now();

    // Trigger learning
    let result = crate::dag::state::DAG_STATE.run_learning_cycle();

    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

    TableIterator::new(vec![
        (result.patterns_updated as i32,
         result.new_clusters as i32,
         result.ewc_updated as i32,
         elapsed)
    ])
}

/// Reset learning state
#[pg_extern]
fn dag_reset_learning(
    preserve_patterns: default!(bool, true),
    preserve_trajectories: default!(bool, false),
) {
    pgrx::warning!(
        "Resetting learning state (preserve_patterns={}, preserve_trajectories={})",
        preserve_patterns, preserve_trajectories
    );

    crate::dag::state::DAG_STATE.reset_learning(
        preserve_patterns,
        preserve_trajectories,
    );

    pgrx::notice!("Learning state reset complete");
}

/// Export learned state
#[pg_extern]
fn dag_export_state() -> Vec<u8> {
    crate::dag::state::DAG_STATE.export_state()
}

/// Import learned state
#[pg_extern]
fn dag_import_state(state_data: Vec<u8>) -> TableIterator<'static, (
    name!(patterns_imported, i32),
    name!(trajectories_imported, i32),
    name!(clusters_restored, i32),
)> {
    let result = crate::dag::state::DAG_STATE.import_state(&state_data);

    TableIterator::new(vec![
        (result.patterns as i32, result.trajectories as i32, result.clusters as i32)
    ])
}

/// Get EWC constraint info
#[pg_extern]
fn dag_ewc_constraints() -> TableIterator<'static, (
    name!(parameter_name, String),
    name!(fisher_importance, f64),
    name!(optimal_value, f64),
)> {
    let constraints = crate::dag::state::DAG_STATE.get_ewc_constraints();

    let results: Vec<_> = constraints.into_iter().map(|c| {
        (c.name, c.fisher, c.optimal)
    }).collect();

    TableIterator::new(results)
}
