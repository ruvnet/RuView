//! Single-RSSI fallback WiFi collection via netsh wlan show interfaces.

#[cfg(not(target_os = "macos"))]
use tracing::{debug, warn};
#[cfg(not(target_os = "macos"))]
use wifi_densepose_hardware::{Adr018Flags, PpduType};
#[cfg(not(target_os = "macos"))]
use wifi_densepose_sensing_server::dsp::features::extract_features_from_frame;
#[cfg(not(target_os = "macos"))]
use wifi_densepose_sensing_server::dsp::signal_field::generate_signal_field;
#[cfg(not(target_os = "macos"))]
use wifi_densepose_sensing_server::ingest::{parse_netsh_interfaces_output, Esp32Frame};
#[cfg(not(target_os = "macos"))]
use wifi_densepose_sensing_server::pose::derive_pose_from_sensing;

#[cfg(not(target_os = "macos"))]
use crate::{
    assess_legacy_image_pose, attach_field_positions, compute_person_score,
    observe_sensing_update, smooth_and_classify, adaptive_override, smooth_vitals,
    tracker_bridge, NodeInfo, SensingUpdate, SharedState,
    FRAME_HISTORY_CAPACITY,
};

/// Fallback: single-RSSI collection via `netsh wlan show interfaces`.
///
/// Used when the multi-BSSID scan fails or returns 0 observations.
#[cfg(not(target_os = "macos"))]
pub async fn windows_wifi_fallback_tick(state: &SharedState, seq: u32) {
    let output = match tokio::process::Command::new("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()
        .await
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => {
            warn!("netsh interfaces fallback failed: {e}");
            return;
        }
    };

    let (rssi_dbm, signal_pct, ssid) = match parse_netsh_interfaces_output(&output) {
        Some(v) => v,
        None => {
            debug!("Fallback: no WiFi interface connected");
            return;
        }
    };

    let frame = Esp32Frame {
        magic: 0xC511_0001,
        node_id: 0,
        n_antennas: 1,
        n_subcarriers: 1,
        freq_mhz: 2437,
        sequence: seq,
        rssi: rssi_dbm as i8,
        noise_floor: -90,
        ppdu_type: PpduType::HtLegacy,
        adr018_flags: Adr018Flags::default(),
        amplitudes: vec![signal_pct],
        phases: vec![0.0],
    };

    let mut s = state.write().await;
    // Update frame history before extracting features.
    s.frame_history.push_back(frame.amplitudes.clone());
    if s.frame_history.len() > FRAME_HISTORY_CAPACITY {
        s.frame_history.pop_front();
    }
    let sample_rate_hz = 2.0_f64; // fallback tick ~ 500 ms => 2 Hz
    let (features, mut classification, breathing_rate_hz, sub_variances, raw_motion) =
        extract_features_from_frame(&frame, &s.frame_history, sample_rate_hz);
    smooth_and_classify(&mut s, &mut classification, raw_motion);
    adaptive_override(&s, &features, &mut classification);

    s.source = format!("wifi:{ssid}");
    s.rssi_history.push_back(rssi_dbm);
    if s.rssi_history.len() > 60 {
        s.rssi_history.pop_front();
    }

    let tick = s.advance_tick();

    let motion_score = if classification.motion_level == "active" {
        0.8
    } else if classification.motion_level == "present_still" {
        0.3
    } else {
        0.05
    };

    let raw_vitals = s
        .vital_detector
        .process_frame(&frame.amplitudes, &frame.phases);
    let vitals = smooth_vitals(&mut s, &raw_vitals);
    s.latest_vitals = vitals.clone();

    let feat_variance = features.variance;

    // ADR-044 §5.2: feed raw features into rolling-P95 estimators before scoring.
    s.p95_variance.push(features.variance);
    s.p95_motion_band_power.push(features.motion_band_power);
    s.p95_spectral_power.push(features.spectral_power);

    // Multi-person estimation with temporal smoothing (EMA α=0.10).
    let raw_score = compute_person_score(&s, &features);
    s.smoothed_person_score = s.smoothed_person_score * 0.90 + raw_score * 0.10;
    let est_persons = if classification.presence {
        let count = s.person_count();
        s.prev_person_count = count;
        count
    } else {
        s.prev_person_count = 0;
        0
    };

    let mut update = SensingUpdate {
        msg_type: "sensing_update".to_string(),
        timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
        source: format!("wifi:{ssid}"),
        tick,
        nodes: vec![NodeInfo {
            node_id: 0,
            device_id: None,
            rssi_dbm,
            position: [0.0, 0.0, 0.0],
            amplitude: vec![signal_pct],
            subcarrier_count: 1,
            sync: None,
            node_inference: None,
            mediatek_diagnostics: None,
        }],
        features,
        classification,
        signal_field: generate_signal_field(
            rssi_dbm,
            motion_score,
            breathing_rate_hz,
            feat_variance.min(1.0),
            &sub_variances,
        ),
        vital_signs: None,
        calibrated_presence_evidence: None,
        detector_diagnostics: None,
        enhanced_motion: None,
        enhanced_breathing: None,
        posture: None,
        signal_quality_score: None,
        quality_verdict: None,
        bssid_count: None,
        pose_keypoints: None,
        model_status: None,
        persons: None,
        estimated_persons: if est_persons > 0 {
            Some(est_persons)
        } else {
            None
        },
        node_features: None,
        room_inference: None,
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
    // #1050: attach real signal_field-peak positions to each person.
    attach_field_positions(&mut update);
    assess_legacy_image_pose(&mut s, &update);

    if let Ok(json) = serde_json::to_string(&update) {
        let _ = s.tx.send(json);
    }
    observe_sensing_update(s.latest_update.as_ref(), &update);
    s.latest_update = Some(update);
}
