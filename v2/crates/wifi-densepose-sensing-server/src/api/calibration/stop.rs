//! Handler for stopping/finalizing empty-room field model calibration.

use axum::{extract::State, Json};
use tracing::info;
use crate::domain::csi_grid::CalibrationModelReceipt;
use crate::error_response;
use crate::field_bridge;
use crate::SharedState;
use super::types::{calibration_identity_error, calibration_identity_matches, CalibrationIdentityRequest};

pub(crate) async fn calibration_stop(
    State(state): State<SharedState>,
    Json(request): Json<CalibrationIdentityRequest>,
) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    if !calibration_identity_matches(&s, &request) {
        return calibration_identity_error();
    }
    let model_id = s.calibration_model_id.clone();
    let source_node_ids: Vec<u8> = s.calibration_source_node_ids.iter().copied().collect();
    let source_grid = s.calibration_grid_binding.map(|binding| binding.grid);
    let reordered_packets_by_node = s.calibration_reordered_packets.clone();
    let max_reorder_depth_by_node = s.calibration_max_reorder_depth.clone();
    if !s.calibration_sequence_fault_node_ids.is_empty() {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "calibration_sequence_discontinuity",
            "error": "Raw CSI sequence moved backward beyond the bounded UDP reorder window. Cancel and restart the empty-room calibration.",
            "model_id": model_id,
            "source_node_ids": source_node_ids,
            "source_grid": source_grid,
            "reordered_packets_by_node": reordered_packets_by_node,
            "max_reorder_depth_by_node": max_reorder_depth_by_node,
            "sequence_fault_node_ids": s.calibration_sequence_fault_node_ids,
        }));
    }
    let Some(binding) = s.calibration_grid_binding else {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "calibration_grid_identity_missing",
            "error": "The active calibration has no frozen CSI grid identity. Cancel and restart the empty room capture.",
            "model_id": model_id,
            "source_node_ids": source_node_ids,
        }));
    };
    let missing_source_node_ids: Vec<u8> = s
        .calibration_source_node_ids
        .difference(&s.calibration_observed_source_node_ids)
        .copied()
        .collect();
    if !missing_source_node_ids.is_empty() {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "calibration_source_nodes_missing",
            "error": "Every node in the room binding must contribute raw CSI before calibration can be finalized.",
            "missing_source_node_ids": missing_source_node_ids,
        }));
    }
    if !s.calibration_grid_is_fresh(binding, std::time::Instant::now()) {
        let latest_seen_ms = s
            .node_states
            .get(&binding.source_node_id)
            .and_then(|node| node.field_model_latest_seen)
            .map(|seen| {
                std::time::Instant::now()
                    .saturating_duration_since(seen)
                    .as_millis() as u64
            });
        return Json(serde_json::json!({
            "success": false,
            "error_code": "calibration_grid_stale",
            "error": "The frozen calibration CSI grid is stale or no longer matches its source. Restore the source and retry before finalizing.",
            "model_id": model_id,
            "source_node_ids": source_node_ids,
            "source_grid": source_grid,
            "latest_seen_ms": latest_seen_ms,
        }));
    }
    let Some(ref mut fm) = s.field_model else {
        return Json(serde_json::json!({
            "success": false,
            "error": "No field model active — call /calibration/start first.",
        }));
    };

    // Guard: finalizing before enough empty-room frames have accumulated
    let have = fm.calibration_frame_count();
    let need = fm.min_calibration_frames() as u64;
    let elapsed_s = fm.calibration_elapsed_s();
    let need_s = fm.min_calibration_duration_s();
    if have < need || (need_s > 0.0 && elapsed_s < need_s) {
        let mut resp = serde_json::json!({
            "success": false,
            "error": "Calibration not complete yet — keep the room empty and poll /calibration/status until both frame_count and elapsed_s reach their targets.",
            "frame_count": have,
            "frames_needed": need,
            "elapsed_s": elapsed_s,
            "duration_needed_s": need_s,
            "frames_per_second": fm.calibration_frames_per_second(),
        });
        if have >= need && elapsed_s < need_s {
            tracing::warn!(
                "Calibration frame target reached in {elapsed_s:.1}s — substantially less than the intended {need_s:.0}s window; continuing to collect for slow environmental variation (#1756)"
            );
            resp["warning"] = serde_json::json!(format!(
                "frame target reached in {elapsed_s:.1}s — substantially less than the intended {need_s:.0}s window; continuing to collect for slow environmental variation"
            ));
        }
        return Json(resp);
    }
    let ts = chrono::Utc::now().timestamp_micros() as u64;
    match fm.finalize_calibration(ts, 0) {
        Ok(modes) => {
            let baseline = modes.baseline_eigenvalue_count;
            let variance_explained = modes.variance_explained;
            let holdout_window_size = modes.baseline_runtime_window_size;
            let final_frame_count = fm.calibration_frame_count();
            let receipt = CalibrationModelReceipt {
                schema: field_bridge::CALIBRATION_MODEL_RECEIPT_SCHEMA,
                boot_epoch: s.calibration_boot_epoch.clone(),
                session_id: s.calibration_session_id.clone().expect("validated identity"),
                model_id: model_id.clone().expect("bound calibration model identity"),
                binding_digest: s.calibration_binding_digest.clone().expect("validated binding"),
                source_node_ids: source_node_ids.clone(),
                frame_count: final_frame_count,
                variance_explained,
                baseline_eigenvalue_count: baseline,
                completed_at_unix_ms: chrono::Utc::now().timestamp_millis().max(0) as u64,
            };
            s.calibration_model_receipt = Some(receipt.clone());
            s.begin_field_model_holdout(binding);
            info!("Field model calibrated: baseline_eigenvalues={baseline}, variance_explained={variance_explained:.2}");
            Json(serde_json::json!({
                "success": true,
                "baseline_eigenvalue_count": baseline,
                "variance_explained": variance_explained,
                "frame_count": final_frame_count,
                "elapsed_s": elapsed_s,
                "holdout_window_size": holdout_window_size,
                "model_id": model_id,
                "source_node_ids": source_node_ids,
                "source_grid": source_grid,
                "reordered_packets_by_node": reordered_packets_by_node,
                "max_reorder_depth_by_node": max_reorder_depth_by_node,
                "model_receipt": receipt,
            }))
        }
        // ADR-080 #2: finalize error chain stays server-side only.
        Err(e) => error_response::internal_error_json("calibration stop", e),
    }
}
