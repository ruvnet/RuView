//! Simulated CSI and presence data generator task.

use std::time::Duration;
use tracing::info;
use wifi_densepose_sensing_server::dsp::features::extract_features_from_frame;
use wifi_densepose_sensing_server::dsp::signal_field::generate_signal_field;
use wifi_densepose_sensing_server::ingest::generate_simulated_frame;
use wifi_densepose_sensing_server::pose::derive_pose_from_sensing;

use crate::{
    assess_legacy_image_pose, attach_field_positions, compute_person_score,
    observe_sensing_update, smooth_and_classify, adaptive_override, smooth_vitals,
    tracker_bridge, NodeInfo, SensingUpdate, SharedState,
    FRAME_HISTORY_CAPACITY,
};

/// Continuous background generator of synthetic CSI frames.
pub async fn simulated_data_task(state: SharedState, tick_ms: u64) {
    let mut interval = tokio::time::interval(Duration::from_millis(tick_ms));
    info!("Simulated data source active (tick={}ms)", tick_ms);

    loop {
        interval.tick().await;

        let mut s = state.write().await;

        // Issue #1004: in `auto` mode this task runs alongside `udp_receiver_task`.
        // Once a real frame promotes `source` → "esp32", stop emitting synthetic
        // frames so we never clobber live CSI with simulated poses. (For an
        // explicit `--source simulated` demo, `source` stays "simulated" and the
        // simulator keeps running — that path never binds UDP, so it is never
        // promoted.) The task stays alive so it can resume serving if the real
        // source later ages out to "esp32:offline".
        if s.effective_source() == "esp32" {
            continue;
        }

        let tick = s.advance_tick();

        let frame = generate_simulated_frame(tick);

        // Append current amplitudes to history before feature extraction.
        s.frame_history.push_back(frame.amplitudes.clone());
        if s.frame_history.len() > FRAME_HISTORY_CAPACITY {
            s.frame_history.pop_front();
        }

        let sample_rate_hz = 1000.0 / tick_ms as f64;
        let (features, mut classification, breathing_rate_hz, sub_variances, raw_motion) =
            extract_features_from_frame(&frame, &s.frame_history, sample_rate_hz);
        smooth_and_classify(&mut s, &mut classification, raw_motion);
        adaptive_override(&s, &features, &mut classification);

        s.rssi_history.push_back(features.mean_rssi);
        if s.rssi_history.len() > 60 {
            s.rssi_history.pop_front();
        }

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

        let frame_amplitudes = frame.amplitudes.clone();
        let frame_n_sub = frame.n_subcarriers;

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
            source: "simulated".to_string(),
            tick,
            nodes: vec![NodeInfo {
                node_id: 1,
                device_id: None,
                rssi_dbm: features.mean_rssi,
                position: [2.0, 0.0, 1.5],
                amplitude: frame_amplitudes,
                subcarrier_count: frame_n_sub as usize,
                sync: None,
                node_inference: None,
                mediatek_diagnostics: None,
            }],
            features: features.clone(),
            classification,
            signal_field: generate_signal_field(
                features.mean_rssi,
                motion_score,
                breathing_rate_hz,
                features.variance.min(1.0),
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
            model_status: if s.model_loaded {
                Some(serde_json::json!({
                    "loaded": true,
                    "layers": s.progressive_loader.as_ref()
                        .map(|l| { let (a,b,c) = l.layer_status(); a as u8 + b as u8 + c as u8 })
                        .unwrap_or(0),
                    "sona_profile": s.active_sona_profile.as_deref().unwrap_or("default"),
                }))
            } else {
                None
            },
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
        let tracked = tracker_bridge::tracker_update(
            &mut s.pose_tracker,
            &mut last_tracker_instant,
            raw_persons,
        );
        s.last_tracker_instant = last_tracker_instant;
        if !tracked.is_empty() {
            update.persons = Some(tracked);
        }
        // #1050: attach real signal_field-peak positions to each person.
        attach_field_positions(&mut update);
        assess_legacy_image_pose(&mut s, &update);

        if update.classification.presence {
            s.total_detections += 1;
        }
        if let Ok(json) = serde_json::to_string(&update) {
            let _ = s.tx.send(json);
        }
        observe_sensing_update(s.latest_update.as_ref(), &update);
        s.latest_update = Some(update);
    }
}
