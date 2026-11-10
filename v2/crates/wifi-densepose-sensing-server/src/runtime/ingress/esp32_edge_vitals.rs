//! ESP32 edge vitals packet ingress handling.
//!
//! Complies with ELMAS.md: <= 350 lines per file, early returns, clean abstraction.

use std::net::SocketAddr;
use tracing::{debug, info, warn};

use wifi_densepose_sensing_server::domain::{
    attach_field_positions, build_node_features, edge_vitals_message_for_publication,
    is_plausible_rssi, node_inference_for, node_is_fresh, node_positions_by_active_id,
    note_default_position_nodes, update_edge_node_classification, ClassificationInfo,
    FeatureInfo, NodeInfo, NodeState, SensingUpdate, DEFAULT_NODE_POSITION,
    NODE_STALE_AFTER_MS,
};
use wifi_densepose_sensing_server::domain::fusion::fuse_multi_node_features;
use wifi_densepose_sensing_server::dsp::generate_signal_field;
use wifi_densepose_sensing_server::inference::fuse_room;
use wifi_densepose_sensing_server::ingest::parse_esp32_vitals;
use wifi_densepose_sensing_server::pose::derive_pose_from_sensing;
use wifi_densepose_sensing_server::vital_signs::VitalSigns;
use wifi_densepose_sensing_server::{semconv, telemetry};

use crate::runtime::ingress::observe::{build_active_node_info, observe_sensing_update};
use crate::state::types::AppStateInner;
use crate::state::{
    assess_legacy_image_pose, calibrated_vitals_for_publication, update_room_person_count,
};
use crate::tracker_bridge;
use crate::SharedState;

/// Attempt to parse and handle an ESP32 edge vitals packet (magic 0xC511_0002).
/// Returns `true` if handled, `false` otherwise.
pub async fn handle_esp32_vitals(
    buf: &[u8],
    len: usize,
    src: SocketAddr,
    state: &SharedState,
) -> bool {
    let Some(vitals) = parse_esp32_vitals(&buf[..len]) else {
        return false;
    };

    debug!(
        "ESP32 vitals from {src}: node={} br={:.1} hr={:.1} pres={}",
        vitals.node_id, vitals.breathing_rate_bpm, vitals.heartrate_bpm, vitals.presence
    );

    let mut s = state.write().await;
    s.source = "esp32".to_string();
    s.last_esp32_frame = Some(std::time::Instant::now());

    let node_id = vitals.node_id;
    let ns = s.node_states.entry(node_id).or_insert_with(NodeState::new);
    let first_sensing_frame = ns.last_frame_time.is_none();
    ns.last_frame_time = Some(std::time::Instant::now());
    if first_sensing_frame && telemetry::curated_events_enabled() {
        info!(
            name: semconv::EVENT_RUVIEW_NODE_ONLINE,
            { "ruview.node.id" = node_id },
            "node {node_id} online (edge vitals)"
        );
    }

    let prev_fall = ns.edge_vitals.as_ref().is_some_and(|v| v.fall_detected);
    if vitals.fall_detected && !prev_fall && telemetry::curated_events_enabled() {
        warn!(
            name: semconv::EVENT_RUVIEW_FALL_DETECTED,
            { "ruview.node.id" = node_id },
            "fall detected by node {node_id}"
        );
    }
    ns.edge_vitals = Some(vitals.clone());

    let mean_rssi_dbm = if is_plausible_rssi(vitals.rssi) {
        ns.rssi_history.push_back(vitals.rssi as f64);
        if ns.rssi_history.len() > 60 {
            ns.rssi_history.pop_front();
        }
        vitals.rssi as f64
    } else {
        ns.rssi_history.back().copied().unwrap_or(0.0)
    };

    update_edge_node_classification(ns, &vitals);

    let tick = s.advance_tick();
    let motion_score = if vitals.motion {
        0.8
    } else if vitals.presence {
        0.3
    } else {
        0.05
    };

    let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let bootstrap_empty = s.bootstrap_empty_prior_applies(observed_at_unix_ms);
    let now = std::time::Instant::now();

    {
        let sref: &mut AppStateInner = &mut s;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        sref.engine_bridge.observe_cycle(&sref.node_states, now_ms);
    }

    let active_nodes = build_active_node_info(&mut s, now);

    let room_inference = fuse_room(
        active_nodes.iter().filter_map(|ni| ni.node_inference.as_ref()),
        NODE_STALE_AFTER_MS,
    );

    let features = FeatureInfo {
        mean_rssi: mean_rssi_dbm,
        variance: vitals.motion_energy as f64,
        motion_band_power: vitals.motion_energy as f64,
        breathing_band_power: if vitals.presence { 0.5 } else { 0.0 },
        dominant_freq_hz: vitals.breathing_rate_bpm / 60.0,
        change_points: 0,
        spectral_power: vitals.motion_energy as f64,
    };

    if let Some(ns) = s.node_states.get_mut(&node_id) {
        ns.latest_features = Some(features.clone());
    }

    let fused_features = fuse_multi_node_features(&features, &s.node_states);
    let classification = if bootstrap_empty {
        ClassificationInfo {
            motion_level: "absent".to_string(),
            presence: false,
            confidence: 0.0,
        }
    } else {
        s.debounce_room_classification(&room_inference)
    };

    let total_persons = update_room_person_count(&mut s, &classification, now, observed_at_unix_ms);
    let signal_field = generate_signal_field(
        fused_features.mean_rssi,
        motion_score,
        vitals.breathing_rate_bpm / 60.0,
        (vitals.presence_score as f64).min(1.0),
        &[],
    );

    let vital_candidates = VitalSigns {
        breathing_rate_bpm: (vitals.breathing_rate_bpm > 0.0).then_some(vitals.breathing_rate_bpm),
        heart_rate_bpm: (vitals.heartrate_bpm > 0.0).then_some(vitals.heartrate_bpm),
        breathing_confidence: if vitals.presence { 0.7 } else { 0.0 },
        heartbeat_confidence: if vitals.presence { 0.7 } else { 0.0 },
        signal_quality: vitals.presence_score as f64,
    };

    let explicit_calibration_fresh = s.explicit_calibration_fresh_at(observed_at_unix_ms);
    let published_vitals = calibrated_vitals_for_publication(
        &s,
        &vital_candidates,
        total_persons,
        observed_at_unix_ms,
    );

    let edge_vitals_message = edge_vitals_message_for_publication(
        &vitals,
        published_vitals.as_ref(),
        bootstrap_empty,
        explicit_calibration_fresh,
        total_persons,
    );
    if let Ok(json) = serde_json::to_string(&edge_vitals_message) {
        let _ = s.tx.send(json);
    }

    let calibrated_presence_evidence = s.calibrated_presence_evidence(
        node_id,
        tick,
        chrono::Utc::now().timestamp_millis().max(0) as u64,
    );

    let mut update = SensingUpdate {
        msg_type: "sensing_update".to_string(),
        timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
        source: "esp32".to_string(),
        tick,
        nodes: active_nodes,
        features: fused_features.clone(),
        classification,
        signal_field,
        vital_signs: published_vitals,
        detector_diagnostics: calibrated_presence_evidence
            .as_ref()
            .and_then(|e| e.diagnostics.clone()),
        calibrated_presence_evidence,
        enhanced_motion: None,
        enhanced_breathing: None,
        posture: None,
        signal_quality_score: None,
        quality_verdict: None,
        bssid_count: None,
        pose_keypoints: None,
        model_status: None,
        persons: None,
        estimated_persons: if total_persons > 0 {
            Some(total_persons)
        } else {
            None
        },
        node_features: build_node_features(&s.node_states, now),
        room_inference: Some(room_inference),
        classifier: None,
        activity: None,
    };

    let raw_persons = derive_pose_from_sensing(&update);
    let mut last_tracker_instant = s.last_tracker_instant.take();
    let tracked =
        tracker_bridge::tracker_update(&mut s.pose_tracker, &mut last_tracker_instant, raw_persons);
    s.last_tracker_instant = last_tracker_instant;
    if !tracked.is_empty() {
        update.persons = Some(tracked);
    }
    attach_field_positions(&mut update);
    assess_legacy_image_pose(&mut s, &update);

    if let Ok(json) = serde_json::to_string(&update) {
        let _ = s.tx.send(json);
    }
    observe_sensing_update(s.latest_update.as_ref(), &update);
    s.latest_update = Some(update);
    s.edge_vitals = Some(vitals);
    true
}
