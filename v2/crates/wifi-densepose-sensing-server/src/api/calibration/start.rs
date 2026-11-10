//! Handler for starting field model calibration.

use axum::{
    extract::{Query, State},
    Json,
};
use wifi_densepose_signal::ruvsense::field_model::{CalibrationStatus, FieldModel};
use crate::domain::csi_grid::CalibrationGridBinding;
use crate::error_response;
use crate::field_bridge;
use crate::{
    opaque_calibration_id, opaque_calibration_model_id, valid_binding_digest,
    valid_source_node_ids, SharedState,
};
use super::types::{CalibrationBindingRequest, CalibrationStartQuery};

pub(crate) async fn calibration_start(
    State(state): State<SharedState>,
    Query(query): Query<CalibrationStartQuery>,
    Json(request): Json<CalibrationBindingRequest>,
) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    if !valid_binding_digest(&request.binding_digest)
        || !valid_source_node_ids(&request.source_node_ids)
    {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "calibration_binding_invalid",
            "error": "Calibration requires a lowercase SHA-256 room binding and 1 to 16 sorted unique node IDs.",
        }));
    }
    let now = std::time::Instant::now();
    let explicit_source = query
        .source_node_id
        .filter(|node_id| request.source_node_ids.binary_search(node_id).is_ok());
    let selected_binding = if let Some(source_node_id) = explicit_source {
        let Some(node) = s.node_states.get(&source_node_id) else {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_source_unavailable",
                "error": "The requested calibration source is missing.",
                "source_node_id": source_node_id,
            }));
        };
        let candidates = node.calibration_grid_candidates(now);
        let Some((grid, evidence)) = node.select_calibration_grid(now) else {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_grid_unavailable",
                "error": "No raw CSI grid has at least 10 seconds of evidence, a 2 Hz measured rate, and a maximum gap below 5 seconds.",
                "source_node_id": source_node_id,
                "candidates": candidates.into_iter().map(|(grid, evidence)| serde_json::json!({
                    "grid": grid,
                    "evidence": evidence,
                })).collect::<Vec<_>>(),
            }));
        };
        CalibrationGridBinding {
            source_node_id,
            grid,
            evidence: Some(evidence),
        }
    } else {
        let eligible: Vec<CalibrationGridBinding> = request
            .source_node_ids
            .iter()
            .filter_map(|&source_node_id| {
                let node = s.node_states.get(&source_node_id)?;
                node.select_calibration_grid(now).map(|(grid, evidence)| {
                    CalibrationGridBinding {
                        source_node_id,
                        grid,
                        evidence: Some(evidence),
                    }
                })
            })
            .collect();
        let Some(binding) = eligible.first() else {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_source_required",
                "error": "None of the bound ESP32 nodes has an eligible raw CSI grid for room calibration.",
                "eligible_sources": eligible.iter().map(|binding| serde_json::json!({
                    "source_node_id": binding.source_node_id,
                    "grid": binding.grid,
                    "evidence": binding.evidence,
                })).collect::<Vec<_>>(),
            }));
        };
        *binding
    };
    if s.bootstrap_baseline_active {
        s.field_model = None;
        s.bootstrap_baseline_active = false;
    }
    // Guard: don't discard an in-progress or fresh calibration
    if let Some(ref fm) = s.field_model {
        let observed_at_us =
            (chrono::Utc::now().timestamp_millis().max(0) as u64).saturating_mul(1_000);
        match field_bridge::calibration_status_at(fm, observed_at_us) {
            CalibrationStatus::Collecting => {
                return Json(serde_json::json!({
                    "success": false,
                    "error": "Calibration already in progress. Call /calibration/stop first.",
                    "frame_count": fm.calibration_frame_count(),
                }));
            }
            CalibrationStatus::Fresh => {
                return Json(serde_json::json!({
                    "success": false,
                    "error": "A fresh calibration already exists. Call /calibration/stop or wait for expiry.",
                }));
            }
            _ => {} // Stale/Expired/Uncalibrated — ok to recalibrate
        }
    }
    match FieldModel::new(field_bridge::single_link_config()) {
        Ok(fm) => {
            s.field_model = Some(fm);
            s.calibration_model_id = Some(opaque_calibration_model_id());
            s.calibration_session_id = Some(opaque_calibration_id("cal-session"));
            s.calibration_binding_digest = Some(request.binding_digest);
            s.calibration_source_node_ids.clear();
            s.calibration_observed_source_node_ids.clear();
            s.calibration_model_receipt = None;
            s.clear_field_model_binding();
            s.clear_calibration_sequence_state();
            s.calibration_source_node_ids
                .extend(request.source_node_ids.iter().copied());
            s.calibration_grid_binding = Some(selected_binding);
            Json(serde_json::json!({
                "success": true,
                "message": "Calibration started — keep room empty while frames accumulate.",
                "status": "uncalibrated",
                "frame_count": 0,
                "boot_epoch": s.calibration_boot_epoch,
                "session_id": s.calibration_session_id,
                "binding_digest": s.calibration_binding_digest,
                "model_id": s.calibration_model_id,
                "source_node_ids": s.calibration_source_node_ids,
                "source_grid": selected_binding.grid,
                "grid_evidence": selected_binding.evidence,
            }))
        }
        // ADR-080 #2: FieldModel init error chain stays server-side only.
        Err(e) => error_response::internal_error_json("calibration start", e),
    }
}

pub fn validate_startup_calibration_request(calibrate: bool) -> Result<(), &'static str> {
    if calibrate {
        Err("--calibrate cannot safely select a source and CSI grid at process start. Start the server normally, wait at least 10 seconds for live grid evidence, then POST /api/v1/calibration/start with one explicit source_node_id.")
    } else {
        Ok(())
    }
}
