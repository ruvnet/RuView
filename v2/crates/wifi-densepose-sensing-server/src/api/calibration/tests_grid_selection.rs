use wifi_densepose_sensing_server::domain::{CalibrationGridBinding, CsiGridKey};
use wifi_densepose_sensing_server::domain::node::NodeState;
use wifi_densepose_sensing_server::field_bridge;
use wifi_densepose_signal::ruvsense::field_model::FieldModel;

use crate::state::AppStateInner;

fn test_grid() -> CsiGridKey {
    CsiGridKey {
        n_subcarriers: 64,
        ppdu_type: 0,
    }
}

fn bind_test_calibration(state: &mut AppStateInner) {
    state.calibration_source_node_ids.insert(5);
    state.calibration_grid_binding = Some(CalibrationGridBinding {
        source_node_id: 5,
        grid: test_grid(),
        evidence: None,
    });
    state.node_states.entry(5).or_insert_with(NodeState::new);
}

fn feed_test_frame(
    state: &mut AppStateInner,
    node_id: u8,
    sequence: u32,
    amplitudes: &[f64],
) -> bool {
    state.maybe_feed_calibration_frame(
        node_id,
        test_grid(),
        sequence,
        amplitudes,
        std::time::Instant::now(),
    )
}

#[test]
fn multi_node_binding_contributes_but_only_the_bound_node_writes_the_baseline() {
    let mut state = AppStateInner::minimal();
    state.field_model = Some(
        FieldModel::new(field_bridge::single_link_config()).expect("field model"),
    );
    bind_test_calibration(&mut state);
    for node_id in [7_u8, 11, 13] {
        state.calibration_source_node_ids.insert(node_id);
        state.node_states.entry(node_id).or_insert_with(NodeState::new);
    }
    assert_eq!(
        state.calibration_grid_binding.expect("binding").source_node_id,
        5
    );

    for node_id in [5_u8, 7, 11, 13] {
        assert!(
            feed_test_frame(&mut state, node_id, 1, &[0.25; 64]),
            "node {node_id} must register as a contributor"
        );
    }

    let missing: Vec<u8> = state
        .calibration_source_node_ids
        .difference(&state.calibration_observed_source_node_ids)
        .copied()
        .collect();
    assert!(
        missing.is_empty(),
        "a multi-node binding must still be able to finalize, missing={missing:?}"
    );

    assert_eq!(
        state
            .field_model
            .as_ref()
            .expect("field model")
            .calibration_frame_count(),
        1,
        "exactly one radio may write the single-link baseline"
    );

    assert_eq!(state.bound_source_node_id(), Some(5));
}

#[test]
fn calibration_grid_selection_prefers_the_grid_the_node_keeps_emitting() {
    let mut node = NodeState::new();
    let start = std::time::Instant::now() - std::time::Duration::from_secs(20);
    let dense = CsiGridKey { n_subcarriers: 256, ppdu_type: 0 };
    let sparse = CsiGridKey { n_subcarriers: 64, ppdu_type: 0 };

    for i in 0..200_u32 {
        node.observe_raw_grid(
            dense,
            start + std::time::Duration::from_millis(u64::from(i) * 50),
        );
    }
    for i in 0..100_u32 {
        node.observe_raw_grid(
            dense,
            start
                + std::time::Duration::from_millis(12_000 + u64::from(i) * 50),
        );
    }
    for i in 0..50_u32 {
        node.observe_raw_grid(
            sparse,
            start + std::time::Duration::from_millis(u64::from(i) * 400),
        );
    }
    node.observe_raw_grid(dense, std::time::Instant::now());
    node.observe_raw_grid(sparse, std::time::Instant::now());

    let candidates = node.calibration_grid_candidates(std::time::Instant::now());
    let gap_of = |want: CsiGridKey| {
        candidates
            .iter()
            .find(|(g, _)| *g == want)
            .map(|(_, e)| e.max_gap_s)
            .expect("candidate present")
    };
    assert!(
        gap_of(sparse) < gap_of(dense),
        "test data must make the sparse grid look smoother: sparse={} dense={}",
        gap_of(sparse),
        gap_of(dense)
    );

    let (grid, _) = node
        .select_calibration_grid(std::time::Instant::now())
        .expect("a qualifying grid");
    assert_eq!(
        grid.n_subcarriers, 256,
        "selection must bind the grid the node keeps emitting, not the sparse minority admission rejects"
    );
}

#[test]
fn non_bound_grid_never_enters_field_model_history() {
    let mut state = AppStateInner::minimal();
    state.field_model = Some(
        FieldModel::new(field_bridge::single_link_config()).expect("field model"),
    );
    bind_test_calibration(&mut state);
    let wrong_grid = CsiGridKey {
        n_subcarriers: 192,
        ppdu_type: 0,
    };
    assert!(!state.maybe_feed_calibration_frame(
        5,
        wrong_grid,
        41,
        &[0.25; 192],
        std::time::Instant::now(),
    ));
    assert!(state
        .node_states
        .get(&5)
        .expect("node")
        .field_model_history
        .is_empty());
    assert!(state.calibration_last_sequences.is_empty());
}

