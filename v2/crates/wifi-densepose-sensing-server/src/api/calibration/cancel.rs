//! Handlers for cancelling and resetting calibration.

use axum::{extract::State, Json};
use tracing::warn;
use wifi_densepose_signal::ruvsense::field_model::CalibrationStatus;
use crate::bootstrap_baseline;
use crate::domain::csi_grid::{CalibrationGridBinding, CsiGridKey};
use crate::SharedState;
use super::types::{calibration_identity_error, calibration_identity_matches, CalibrationIdentityRequest};

/// Cancel only an unfinished capture.
pub(crate) async fn calibration_cancel(
    State(state): State<SharedState>,
    Json(request): Json<CalibrationIdentityRequest>,
) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    if !calibration_identity_matches(&s, &request) {
        return calibration_identity_error();
    }
    let cancellable = s.field_model.as_ref().is_some_and(|model| {
        matches!(
            model.status(),
            CalibrationStatus::Uncalibrated | CalibrationStatus::Collecting
        )
    });
    if !cancellable {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "calibration_not_collecting",
            "error": "Only an unfinished empty room capture can be cancelled.",
        }));
    }
    let cancelled_session_id = s.calibration_session_id.clone();
    s.field_model = None;
    s.calibration_model_id = None;
    s.calibration_session_id = None;
    s.calibration_binding_digest = None;
    s.calibration_source_node_ids.clear();
    s.calibration_observed_source_node_ids.clear();
    s.calibration_model_receipt = None;
    s.clear_field_model_binding();
    s.clear_calibration_sequence_state();
    if let Some(installation_id) = s.installation_id.clone() {
        let path = bootstrap_baseline::path_in(&s.data_dir);
        if let Ok((model, metadata)) = bootstrap_baseline::load(
            &path,
            &installation_id,
            chrono::Utc::now().timestamp_millis() as u64,
        ) {
            let [source_node_id] = metadata.source_node_ids.as_slice() else {
                return Json(serde_json::json!({
                    "success": false,
                    "error_code": "bootstrap_source_invalid",
                    "error": "The stored bootstrap source identity is invalid.",
                }));
            };
            let binding = CalibrationGridBinding {
                source_node_id: *source_node_id,
                grid: CsiGridKey::from_bootstrap_grid(metadata.source_grid),
                evidence: None,
            };
            s.field_model = Some(model);
            s.calibration_source_node_ids.insert(*source_node_id);
            s.calibration_grid_binding = Some(binding);
            s.bootstrap_baseline = Some(metadata);
            s.bootstrap_baseline_active = true;
        }
    }
    Json(serde_json::json!({
        "success": true,
        "message": "Unfinished empty room capture cancelled.",
        "status": if s.bootstrap_baseline_active { "bootstrap" } else { "none" },
        "frame_count": 0,
        "boot_epoch": s.calibration_boot_epoch,
        "cancelled_session_id": cancelled_session_id,
    }))
}

/// Reset calibration model.
pub(crate) async fn calibration_reset(
    State(state): State<SharedState>,
    Json(request): Json<CalibrationIdentityRequest>,
) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    let boot_only = request.boot_epoch == s.calibration_boot_epoch
        && request.session_id.is_none()
        && request.binding_digest.is_none()
        && request.source_node_ids.is_none();
    if !boot_only && !calibration_identity_matches(&s, &request) {
        return calibration_identity_error();
    }
    s.field_model = None;
    s.calibration_model_id = None;
    s.calibration_session_id = None;
    s.calibration_binding_digest = None;
    s.calibration_source_node_ids.clear();
    s.calibration_observed_source_node_ids.clear();
    s.calibration_model_receipt = None;
    s.clear_field_model_binding();
    s.clear_calibration_sequence_state();
    s.bootstrap_baseline_active = false;
    let path = bootstrap_baseline::path_in(&s.data_dir);
    let bootstrap_removed = match bootstrap_baseline::remove(&path) {
        Ok(removed) => {
            s.bootstrap_baseline = None;
            removed
        }
        Err(error) => {
            warn!(%error, "Could not remove stored bootstrap baseline during reset");
            false
        }
    };
    Json(serde_json::json!({
        "success": true,
        "message": "Calibration model reset.",
        "status": "none",
        "bootstrap_removed": bootstrap_removed,
    }))
}
