use std::collections::HashMap;
use std::time::{Duration, Instant};

use wifi_densepose_sensing_server::domain::classification::{
    classify_vitals, debounce_room_classification_state, score_to_person_count, NODE_STALE_AFTER_MS,
};
use wifi_densepose_sensing_server::domain::fusion::corr_persons_to_score;
use wifi_densepose_sensing_server::domain::node::{
    node_inference_for, update_edge_node_classification, NodeState,
};
use wifi_densepose_sensing_server::domain::vitals::edge_vitals_message_for_publication;
use wifi_densepose_sensing_server::inference::{fuse_room, RoomInference};
use wifi_densepose_sensing_server::ingest::Esp32VitalsPacket;
use crate::state::{
    aggregate_person_count, update_room_person_count, AppStateInner,
};

fn node_with_count(c: usize) -> NodeState {
    let mut n = NodeState::new();
    n.prev_person_count = c;
    n.last_frame_time = Some(Instant::now());
    n
}

#[test]
fn empty_nodes_fall_back_to_activity_count() {
    let nodes: HashMap<u8, NodeState> = HashMap::new();
    assert_eq!(aggregate_person_count(1, &nodes, Instant::now()), 1);
    assert_eq!(aggregate_person_count(0, &nodes, Instant::now()), 0);
}

#[test]
fn node_estimate_raises_a_saturated_activity_count() {
    let mut nodes = HashMap::new();
    nodes.insert(1u8, node_with_count(2));
    assert_eq!(
        aggregate_person_count(1, &nodes, Instant::now()),
        2,
        "a node reporting 2 must not be discarded by the activity count"
    );
}

#[test]
fn activity_count_wins_when_higher_than_nodes() {
    let mut nodes = HashMap::new();
    nodes.insert(1u8, node_with_count(1));
    assert_eq!(aggregate_person_count(3, &nodes, Instant::now()), 3);
}

#[test]
fn takes_max_across_multiple_nodes() {
    let mut nodes = HashMap::new();
    nodes.insert(1u8, node_with_count(1));
    nodes.insert(2u8, node_with_count(3));
    nodes.insert(3u8, node_with_count(2));
    assert_eq!(aggregate_person_count(1, &nodes, Instant::now()), 3);
}

#[test]
fn single_occupant_is_never_inflated() {
    let mut nodes = HashMap::new();
    nodes.insert(1u8, node_with_count(1));
    nodes.insert(2u8, node_with_count(1));
    assert_eq!(aggregate_person_count(1, &nodes, Instant::now()), 1);
}

#[test]
fn stale_three_cannot_override_fresh_one() {
    let now = Instant::now();
    let mut fresh = node_with_count(1);
    fresh.last_frame_time = Some(now);
    let mut stale = node_with_count(3);
    stale.last_frame_time = Some(now - Duration::from_secs(30));
    let nodes = HashMap::from([(1, fresh), (2, stale)]);
    assert_eq!(aggregate_person_count(1, &nodes, now), 1);
}

#[test]
fn counts_expire_at_the_room_votes_exact_boundary() {
    let now = Instant::now();
    let mut node = node_with_count(3);
    node.last_frame_time = Some(now - Duration::from_millis(NODE_STALE_AFTER_MS));
    let mut nodes = HashMap::from([(1, node)]);
    assert_eq!(aggregate_person_count(0, &nodes, now), 0);
    assert!(node_inference_for(&nodes[&1], now).is_stale(NODE_STALE_AFTER_MS));

    nodes.get_mut(&1).unwrap().last_frame_time =
        Some(now - Duration::from_millis(NODE_STALE_AFTER_MS - 1));
    assert_eq!(aggregate_person_count(0, &nodes, now), 3);
}

#[test]
fn missing_or_future_timestamps_cannot_contribute() {
    let now = Instant::now();
    for timestamp in [None, Some(now + Duration::from_secs(1))] {
        let mut node = node_with_count(3);
        node.last_frame_time = timestamp;
        assert_eq!(aggregate_person_count(1, &HashMap::from([(1, node)]), now), 1);
    }
}

fn edge(presence: bool, motion: bool, count: u8) -> Esp32VitalsPacket {
    Esp32VitalsPacket {
        node_id: 1,
        presence,
        motion,
        n_persons: count,
        person_count_valid: true,
        fall_detected: false,
        breathing_rate_bpm: 0.0,
        heartrate_bpm: 0.0,
        rssi: -50,
        motion_energy: 0.0,
        presence_score: if presence || motion { 0.9 } else { 0.1 },
        timestamp_ms: 0,
    }
}

fn observe(state: &mut AppStateInner, id: u8, packet: Esp32VitalsPacket, now: Instant) {
    let node = state.node_states.entry(id).or_insert_with(NodeState::new);
    node.last_frame_time = Some(now);
    update_edge_node_classification(node, &packet);
}

fn room(state: &AppStateInner, now: Instant) -> RoomInference {
    let inferences: Vec<_> = state.node_states.values()
        .map(|node| node_inference_for(node, now)).collect();
    fuse_room(inferences.iter(), NODE_STALE_AFTER_MS)
}

#[test]
fn edge_only_nodes_supply_their_own_room_vote() {
    let now = Instant::now();
    let mut state = AppStateInner::minimal();
    observe(&mut state, 1, edge(true, false, 2), now);
    assert_eq!(room(&state, now).classification, "present_still");
    observe(&mut state, 1, edge(false, true, 0), now);
    assert_eq!(room(&state, now).classification, "present_moving");
    assert_eq!(state.node_states[&1].prev_person_count, 1);
    observe(&mut state, 1, edge(false, false, 0), now);
    assert_eq!(room(&state, now).classification, "absent");
    assert_eq!(state.node_states[&1].prev_person_count, 0);
}

#[test]
fn edge_confidence_does_not_inflate_the_raw_csi_count_score() {
    for confidence in [0.9, f32::NAN] {
        let mut node = NodeState::new();
        node.smoothed_person_score = corr_persons_to_score(1);
        let mut packet = edge(true, false, 1);
        packet.presence_score = confidence;
        update_edge_node_classification(&mut node, &packet);
        node.smoothed_person_score = node.smoothed_person_score * 0.92
            + corr_persons_to_score(1) * 0.08;
        assert_eq!(score_to_person_count(node.smoothed_person_score, node.prev_person_count), 1);
        assert!(node_inference_for(&node, Instant::now()).confidence.is_finite());
    }
}

#[test]
fn legacy_edge_count_stays_local_when_another_node_keeps_room_present() {
    let absent = edge_vitals_message_for_publication(&edge(false, false, 0), None, false, false, 2);
    assert_eq!(absent["presence"], false);
    assert_eq!(absent["n_persons"], 0);
    let present = edge_vitals_message_for_publication(&edge(true, false, 1), None, false, false, 2);
    assert_eq!(present["presence"], true);
    assert_eq!(present["n_persons"], 1);
    let suppressed = edge_vitals_message_for_publication(&edge(true, true, 3), None, false, false, 0);
    assert_eq!(suppressed["presence"], false);
    assert_eq!(suppressed["n_persons"], 0);
}

#[test]
fn pending_room_presence_cannot_publish_a_positive_count() {
    let now = Instant::now();
    let mut state = AppStateInner::minimal();
    observe(&mut state, 1, edge(true, false, 2), now);
    let inference = room(&state, now);
    let pending = debounce_room_classification_state(
        &mut state.room_debounced_level,
        &mut state.room_debounce_candidate,
        &mut state.room_debounce_since,
        &inference,
        now,
    );
    assert!(!pending.presence);
    assert_eq!(update_room_person_count(&mut state, &pending, now, 0), 0);
    state.room_debounce_since = Some(now - Duration::from_secs(2));
    let committed = debounce_room_classification_state(
        &mut state.room_debounced_level,
        &mut state.room_debounce_candidate,
        &mut state.room_debounce_since,
        &inference,
        now,
    );
    assert!(committed.presence);
    assert_eq!(update_room_person_count(&mut state, &committed, now, 0), 2);
}

#[test]
fn alternating_absent_node_does_not_clear_present_room_count() {
    let now = Instant::now();
    let mut state = AppStateInner::minimal();
    state.room_debounced_level = "present_still".to_string();
    observe(&mut state, 1, edge(true, false, 2), now);
    observe(&mut state, 2, edge(true, false, 2), now);
    for id in [3, 1, 3, 2, 3] {
        observe(&mut state, id, edge(id != 3, false, if id == 3 { 0 } else { 2 }), now);
        let inference = room(&state, now);
        let classification = debounce_room_classification_state(
            &mut state.room_debounced_level,
            &mut state.room_debounce_candidate,
            &mut state.room_debounce_since,
            &inference,
            now,
        );
        assert!(classification.presence);
        assert_eq!(update_room_person_count(&mut state, &classification, now, 0), 2);
        assert_eq!(state.prev_person_count, 2, "fallback updates room history too");
    }
}

#[test]
fn absent_room_suppresses_a_fresh_positive_node() {
    let now = Instant::now();
    let mut state = AppStateInner::minimal();
    observe(&mut state, 1, edge(false, false, 0), now);
    observe(&mut state, 2, edge(false, false, 0), now);
    observe(&mut state, 3, edge(true, false, 3), now);
    let inference = room(&state, now);
    let classification = debounce_room_classification_state(
        &mut state.room_debounced_level,
        &mut state.room_debounce_candidate,
        &mut state.room_debounce_since,
        &inference,
        now,
    );
    assert!(!classification.presence);
    assert_eq!(update_room_person_count(&mut state, &classification, now, 0), 0);
    assert_eq!(state.node_states[&3].prev_person_count, 3, "room must not overwrite node evidence");
}

#[test]
fn bootstrap_absence_and_stale_room_cannot_restore_a_positive_count() {
    let now = Instant::now();
    let mut state = AppStateInner::minimal();
    observe(&mut state, 1, edge(true, false, 3), now);
    state.prev_person_count = 3;
    let suppressed = wifi_densepose_sensing_server::domain::classification_from_room(&RoomInference::unavailable());
    assert_eq!(update_room_person_count(&mut state, &suppressed, now, 0), 0);
    assert_eq!(state.prev_person_count, 0);

    let held_present = classify_vitals(false, true, 0.9);
    let expired = now + Duration::from_millis(NODE_STALE_AFTER_MS);
    assert_eq!(update_room_person_count(&mut state, &held_present, expired, 0), 0);
    assert_eq!(room(&state, expired), RoomInference::unavailable());
}

