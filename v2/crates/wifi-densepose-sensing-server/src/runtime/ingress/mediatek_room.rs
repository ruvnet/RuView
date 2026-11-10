//! MediaTek CSI room fusion, per-device eviction, and ring clamping.
//!
//! Complies with ELMAS.md: <= 350 lines, guard clauses, decoupled domain logic.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::warn;

use crate::mediatek_activity;
use crate::mediatek_heuristic;
use crate::AppStateInner;
use wifi_densepose_sensing_server::domain::{
    classification_from_room, FeatureInfo, MediatekNodeDiagnostics, NodeInfo, SensingUpdate,
    SignalField,
};
use wifi_densepose_sensing_server::inference::{fuse_room, NodeInference};

/// Upper bound on distinct MediaTek `device_id`s tracked at once.
pub const MAX_MEDIATEK_DEVICES: usize = 64;

/// Upper bound accepted for `--csi-ring` (frames retained per device).
pub const MAX_CSI_RING_CAPACITY: usize = 4096;

/// Clamp the `--csi-ring` flag into `1..=MAX_CSI_RING_CAPACITY`.
pub fn clamp_csi_ring(requested: usize) -> usize {
    requested.clamp(1, MAX_CSI_RING_CAPACITY)
}

/// Make room for `device_id` in the per-device MediaTek maps. If it is a new
/// device and the cap is reached, evict the oldest-updated device.
pub fn evict_mediatek_device_if_full(s: &mut AppStateInner, device_id: &str) {
    if s.mediatek_csi_by_device.contains_key(device_id)
        || s.mediatek_csi_by_device.len() < MAX_MEDIATEK_DEVICES
    {
        return;
    }
    let oldest = s
        .mediatek_csi_by_device
        .iter()
        .min_by_key(|(_, (_, seen))| *seen)
        .map(|(id, _)| id.clone());
    if let Some(id) = oldest {
        warn!("MediaTek device cap ({MAX_MEDIATEK_DEVICES}) reached; evicting {id}");
        s.mediatek_csi_by_device.remove(&id);
        s.mediatek_csi_ring_by_device.remove(&id);
        s.mediatek_heuristic_by_device.remove(&id);
        s.mediatek_activity_by_device.remove(&id);
    }
}

/// Build the MediaTek-heuristic `SensingUpdate` from every currently-fresh
/// device in `mediatek_csi_by_device` / `mediatek_heuristic_by_device`.
pub fn mediatek_room_update(
    s: &mut AppStateInner,
    now: Instant,
    stale_after: Duration,
    tick: u64,
    source: String,
) -> SensingUpdate {
    let mut verdicts: HashMap<String, mediatek_heuristic::DeviceVerdict> = HashMap::new();
    let fresh_ids: Vec<String> = s
        .mediatek_csi_by_device
        .iter()
        .filter(|(_, (_, seen))| now.saturating_duration_since(*seen) <= stale_after)
        .map(|(id, _)| id.clone())
        .collect();
    for id in fresh_ids {
        if let Some(hist) = s.mediatek_heuristic_by_device.get_mut(&id) {
            verdicts.insert(id, hist.verdict(now));
        }
    }
    let confidences = mediatek_heuristic::apply_agreement(&verdicts);

    let mut ids: Vec<&String> = verdicts.keys().collect();
    ids.sort();

    let nodes: Vec<NodeInfo> = ids
        .iter()
        .map(|&id| {
            let v = &verdicts[id];
            let confidence = confidences.get(id).copied().unwrap_or(0.0);
            let rssi = s
                .mediatek_csi_by_device
                .get(id)
                .and_then(|(snap, _)| snap.rssi_dbm.first().copied())
                .unwrap_or(0) as f64;
            let node_inference = v
                .presence
                .map(|_| NodeInference::new(v.motion_level.as_str(), confidence, Some(0)));
            NodeInfo {
                node_id: 0,
                device_id: Some(id.clone()),
                rssi_dbm: rssi,
                position: [0.0, 0.0, 0.0],
                amplitude: vec![],
                subcarrier_count: 0,
                sync: None,
                node_inference,
                mediatek_diagnostics: Some(MediatekNodeDiagnostics {
                    presence_state: v.presence_state.as_str().to_string(),
                    coefficient_of_variation: v.window.coefficient_of_variation,
                    baseline_deviation: v.window.baseline_deviation,
                    sample_count: v.window.sample_count,
                    span_ms: v.window.span_ms,
                }),
            }
        })
        .collect();

    let node_inferences: Vec<NodeInference> = nodes
        .iter()
        .filter_map(|n| n.node_inference.clone())
        .collect();
    let room = fuse_room(node_inferences.iter(), stale_after.as_millis() as u64);
    let classification = classification_from_room(&room);

    let mean_rssi = if nodes.is_empty() {
        0.0
    } else {
        nodes.iter().map(|n| n.rssi_dbm).sum::<f64>() / nodes.len() as f64
    };

    let room_activity_index = mediatek_activity::room_index(
        &s.mediatek_activity_by_device,
        now,
        mediatek_activity::ROOM_STALE_AFTER,
    );
    let room_activity_level = mediatek_activity::room_level(room_activity_index);
    let room_activity_peak_30s = mediatek_activity::room_peak_30s(
        &mut s.room_activity_peak_history,
        now,
        room_activity_level,
    );
    let room_activity_abs_level = mediatek_activity::room_abs_level(
        &s.mediatek_activity_by_device,
        now,
        mediatek_activity::ROOM_STALE_AFTER,
    );
    let room_activity_abs_peak_30s = mediatek_activity::room_peak_30s(
        &mut s.room_abs_activity_peak_history,
        now,
        room_activity_abs_level,
    );
    mediatek_activity::maybe_sample_history(
        &mut s.activity_history,
        &mut s.activity_history_last_sampled,
        now,
        room_activity_level,
        room_activity_abs_level,
        &s.mediatek_activity_by_device,
    );
    let activity_devices: serde_json::Map<String, serde_json::Value> = s
        .mediatek_activity_by_device
        .iter()
        .filter_map(|(id, tracker)| {
            let last_update = tracker.last_update()?;
            let age = now.saturating_duration_since(last_update);
            if age > mediatek_activity::ROOM_STALE_AFTER {
                return None;
            }
            let v = tracker.last_verdict()?;
            Some((
                id.clone(),
                serde_json::json!({
                    "level": v.level,
                    "index": v.index,
                    "fast": v.fast,
                    "slow": v.slow,
                    "peak_30s": v.peak_30s,
                    "frames_2s": v.frames_2s,
                    "floor": v.floor,
                    "abs_level": v.abs_level,
                    "abs_peak_30s": v.abs_peak_30s,
                    "age_ms": age.as_millis() as u64,
                    "is_occupancy_estimate": false,
                }),
            ))
        })
        .collect();
    let activity = serde_json::json!({
        "room": {
            "level": room_activity_level,
            "index": room_activity_index,
            "peak_30s": room_activity_peak_30s,
            "abs_level": room_activity_abs_level,
            "abs_peak_30s": room_activity_abs_peak_30s,
            "is_occupancy_estimate": false,
        },
        "devices": activity_devices,
    });

    SensingUpdate {
        msg_type: "sensing_update".to_string(),
        timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
        source,
        tick,
        nodes,
        features: FeatureInfo {
            mean_rssi,
            variance: 0.0,
            motion_band_power: 0.0,
            breathing_band_power: 0.0,
            dominant_freq_hz: 0.0,
            change_points: 0,
            spectral_power: 0.0,
        },
        classification,
        signal_field: SignalField {
            grid_size: [0, 0, 0],
            values: vec![],
        },
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
        estimated_persons: None,
        node_features: None,
        room_inference: Some(room),
        classifier: Some(mediatek_heuristic::CLASSIFIER_ID.to_string()),
        activity: Some(activity),
    }
}

