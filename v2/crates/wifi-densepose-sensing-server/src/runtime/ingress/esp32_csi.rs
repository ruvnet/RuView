//! ESP32 raw CSI frame ingress processing.
//!
//! Complies with ELMAS.md: <= 350 lines per file, early returns, clean abstraction.

use std::net::SocketAddr;
use tracing::{debug, info};

use wifi_densepose_sensing_server::domain::{
    attach_field_positions, build_node_features, evict_stale_node_states,
    node_inference_for, node_is_fresh, node_positions_by_active_id,
    note_default_position_nodes, update_node_person_count, ClassificationInfo,
    CsiGridKey, NodeInfo, NodeState, SensingUpdate, DEFAULT_NODE_POSITION,
    NODE_STALE_AFTER_MS,
};
use wifi_densepose_sensing_server::domain::fusion::{
    corr_persons_to_score, estimate_persons_from_correlation, fuse_multi_node_features,
};
use wifi_densepose_sensing_server::domain::smoothing::{
    smooth_and_classify_node, smooth_vitals_node,
};
use wifi_densepose_sensing_server::dsp::{
    extract_features_from_frame, generate_signal_field,
};
use wifi_densepose_sensing_server::inference::fuse_room;
use wifi_densepose_sensing_server::ingest::parse_esp32_frame;
use wifi_densepose_sensing_server::pose::derive_pose_from_sensing;
use wifi_densepose_sensing_server::{semconv, telemetry};

use crate::adaptive_classifier;
use crate::runtime::ingress::observe::{
    build_active_node_info, observe_sensing_update, update_introspection_tap,
};
use crate::state::types::{AppStateInner, FRAME_HISTORY_CAPACITY};
use crate::state::{
    assess_legacy_image_pose, calibrated_vitals_for_publication, emit_rufield_event,
    update_room_person_count,
};
use crate::tracker_bridge;
use crate::SharedState;

/// Attempt to parse and handle an ESP32 raw CSI frame.
/// Returns `true` if handled, `false` otherwise.
pub async fn handle_esp32_csi_frame(
    buf: &[u8],
    len: usize,
    src: SocketAddr,
    state: &SharedState,
) -> bool {
    let Some(frame) = parse_esp32_frame(&buf[..len]) else {
        return false;
    };

    debug!(
        "ESP32 frame from {src}: node={}, subs={}, seq={}",
        frame.node_id, frame.n_subcarriers, frame.sequence
    );

    let mut s = state.write().await;
    s.source = "esp32".to_string();
    let observed_at = std::time::Instant::now();
    s.last_esp32_frame = Some(observed_at);
    let grid_key = CsiGridKey::from_frame(&frame);
    s.node_states
        .entry(frame.node_id)
        .or_insert_with(NodeState::new)
        .observe_raw_grid(grid_key, observed_at);

    s.maybe_feed_calibration_frame(
        frame.node_id,
        grid_key,
        frame.sequence,
        &frame.amplitudes,
        observed_at,
    );

    let grid_accepted = s
        .node_states
        .entry(frame.node_id)
        .or_insert_with(NodeState::new)
        .accept_grid(frame.grid());
    if !grid_accepted {
        debug!(
            "node {}: skipping {}-subcarrier {:?} frame (active grid {:?})",
            frame.node_id,
            frame.n_subcarriers,
            frame.ppdu_type,
            s.node_states.get(&frame.node_id).and_then(|ns| ns.active_grid),
        );
        if let Some(ns) = s.node_states.get_mut(&frame.node_id) {
            ns.observe_csi_frame_arrival(std::time::Instant::now());
        }
        return true;
    }

    s.frame_history.push_back(frame.amplitudes.clone());
    if s.frame_history.len() > FRAME_HISTORY_CAPACITY {
        s.frame_history.pop_front();
    }

    update_introspection_tap(&mut s, &frame.amplitudes);

    let node_id = frame.node_id;
    let adaptive_model_clone = s.adaptive_model.clone();

    let ns = s.node_states.entry(node_id).or_insert_with(NodeState::new);
    let first_sensing_frame = ns.observe_accepted_csi_frame(
        frame.sequence,
        frame.adr018_flags.ieee802154_sync_valid,
        std::time::Instant::now(),
    );
    if first_sensing_frame && telemetry::curated_events_enabled() {
        info!(
            name: semconv::EVENT_RUVIEW_NODE_ONLINE,
            { "ruview.node.id" = node_id },
            "node {node_id} online (CSI)"
        );
    }

    ns.update_novelty(&frame.amplitudes);
    ns.frame_history.push_back(frame.amplitudes.clone());
    if ns.frame_history.len() > FRAME_HISTORY_CAPACITY {
        ns.frame_history.pop_front();
    }

    let sample_rate_hz = ns.effective_sample_rate_hz();
    let (features, mut classification, breathing_rate_hz, sub_variances, raw_motion) =
        extract_features_from_frame(&frame, &ns.frame_history, sample_rate_hz);
    smooth_and_classify_node(ns, &mut classification, raw_motion);

    if let Some(ref model) = adaptive_model_clone {
        let amps = ns.frame_history.back().map(|v| v.as_slice()).unwrap_or(&[]);
        let dominant_frequency = adaptive_classifier::compatible_dominant_frequency(
            model.version,
            features.dominant_freq_hz,
            amps,
        );
        let feat_arr = adaptive_classifier::features_from_runtime(
            &serde_json::json!({
                "variance": features.variance,
                "motion_band_power": features.motion_band_power,
                "breathing_band_power": features.breathing_band_power,
                "spectral_power": features.spectral_power,
                "dominant_freq_hz": dominant_frequency,
                "change_points": features.change_points,
                "mean_rssi": features.mean_rssi,
            }),
            amps,
        );
        let (label, conf) = model.classify(&feat_arr);
        classification.motion_level = label.to_string();
        classification.presence = label != "absent";
        classification.confidence = (conf * 0.7 + classification.confidence * 0.3).clamp(0.0, 1.0);
    }

    ns.rssi_history.push_back(features.mean_rssi);
    if ns.rssi_history.len() > 60 {
        ns.rssi_history.pop_front();
    }

    if ns.csi_fps_samples >= 5 && ns.vital_detector.reconfigure_sample_rate(sample_rate_hz) {
        ns.smoothed_hr = 0.0;
        ns.smoothed_br = 0.0;
        ns.smoothed_hr_conf = 0.0;
        ns.smoothed_br_conf = 0.0;
        ns.hr_buffer.clear();
        ns.br_buffer.clear();
    }

    let raw_vitals = ns
        .vital_detector
        .process_frame(&frame.amplitudes, &frame.phases);
    let vitals = smooth_vitals_node(ns, &raw_vitals);
    ns.latest_vitals = vitals.clone();

    let corr_persons = estimate_persons_from_correlation(&ns.frame_history);
    let raw_score = corr_persons_to_score(corr_persons);
    ns.smoothed_person_score = ns.smoothed_person_score * 0.92 + raw_score * 0.08;
    update_node_person_count(ns);

    ns.latest_classification_confidence = Some(classification.confidence);
    ns.latest_features = Some(features.clone());

    s.rssi_history.push_back(features.mean_rssi);
    if s.rssi_history.len() > 60 {
        s.rssi_history.pop_front();
    }
    s.latest_vitals = vitals.clone();

    let fused_features = fuse_multi_node_features(&features, &s.node_states);
    let tick = s.advance_tick();

    let motion_score = if classification.motion_level == "active" {
        0.8
    } else if classification.motion_level == "present_still" {
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
    let room_classification = if bootstrap_empty {
        ClassificationInfo {
            motion_level: "absent".to_string(),
            presence: false,
            confidence: 0.0,
        }
    } else {
        s.debounce_room_classification(&room_inference)
    };
    let total_persons = update_room_person_count(&mut s, &room_classification, now, observed_at_unix_ms);
    let published_vitals =
        calibrated_vitals_for_publication(&s, &vitals, total_persons, observed_at_unix_ms);

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
        classification: room_classification,
        signal_field: generate_signal_field(
            fused_features.mean_rssi,
            motion_score,
            breathing_rate_hz,
            fused_features.variance.min(1.0),
            &sub_variances,
        ),
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

    emit_rufield_event(&s, &update, node_id);
    observe_sensing_update(s.latest_update.as_ref(), &update);
    s.latest_update = Some(update);

    if tick % 100 == 0 {
        evict_stale_node_states(&mut s.node_states, now);
    }

    true
}
