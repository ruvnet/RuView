//! Operator-confirmed empty holdout refinement for bootstrap baseline.

use axum::{
    extract::{Query, State},
    Json,
};
use std::time::Duration;
use wifi_densepose_signal::ruvsense::field_model::FieldModel;
use crate::bootstrap_baseline::{
    self, BOOTSTRAP_VALIDATION_MIN_SPACING_MS, BOOTSTRAP_VALIDATION_SAMPLES,
    BOOTSTRAP_VALIDATION_SAMPLE_TIMEOUT_MS,
};
use crate::domain::csi_grid::CsiGridKey;
use crate::error_response;
use crate::{ESP32_OFFLINE_TIMEOUT, SharedState};
use super::types::BootstrapRefineQuery;

/// Measure one separate operator-confirmed empty holdout and make a bounded,
/// single-use adjustment to a restored bootstrap residual boundary.
pub(crate) async fn calibration_refine_bootstrap(
    State(state): State<SharedState>,
    Query(query): Query<BootstrapRefineQuery>,
) -> Json<serde_json::Value> {
    if !query.confirmed_empty {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "empty_confirmation_required",
            "error": "Explicit empty-room confirmation is required for bootstrap refinement.",
        }));
    }

    let (expected_model_id, expected_digest, source_node_id, expected_grid) = {
        let s = state.read().await;
        let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
        let Some(metadata) = s
            .bootstrap_baseline
            .as_ref()
            .filter(|_| s.bootstrap_baseline_active_at(observed_at_unix_ms))
        else {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "bootstrap_not_active",
                "error": "A restored startup baseline must be active before refinement.",
            }));
        };
        let [source_node_id] = metadata.source_node_ids.as_slice() else {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "bootstrap_source_invalid",
                "error": "Bootstrap refinement requires exactly one bound source node.",
            }));
        };
        if query
            .source_node_id
            .is_some_and(|requested| requested != *source_node_id)
        {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "bootstrap_source_mismatch",
                "error": "The requested source does not match the stored bootstrap source.",
            }));
        }
        (
            metadata.source_model_id.clone(),
            metadata.content_sha256.clone(),
            *source_node_id,
            CsiGridKey::from_bootstrap_grid(metadata.source_grid),
        )
    };

    let mut residuals = Vec::with_capacity(BOOTSTRAP_VALIDATION_SAMPLES);
    let mut stale_sample_count = 0usize;
    let mut vital_sign_sample_count = 0usize;
    let mut last_sequence = None;
    let mut next_sample_at = tokio::time::Instant::now();
    for _ in 0..BOOTSTRAP_VALIDATION_SAMPLES {
        tokio::time::sleep_until(next_sample_at).await;
        let interval_deadline = tokio::time::Instant::now()
            + Duration::from_millis(BOOTSTRAP_VALIDATION_SAMPLE_TIMEOUT_MS);
        let mut observed = None;
        while tokio::time::Instant::now() < interval_deadline {
            observed = {
                let s = state.read().await;
                let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
                let identity_matches = s.bootstrap_baseline_active_at(observed_at_unix_ms)
                    && s.bootstrap_baseline.as_ref().is_some_and(|metadata| {
                        metadata.source_model_id == expected_model_id
                            && metadata.content_sha256 == expected_digest
                            && metadata.source_node_ids.as_slice() == [source_node_id]
                            && CsiGridKey::from_bootstrap_grid(metadata.source_grid)
                                == expected_grid
                    })
                    && s.calibration_grid_binding.is_some_and(|binding| {
                        binding.source_node_id == source_node_id
                            && binding.grid == expected_grid
                    });
                if !identity_matches {
                    None
                } else {
                    s.node_states.get(&source_node_id).and_then(|node| {
                        let sequence = node.field_model_latest_sequence?;
                        let fresh = node.field_model_latest_seen.is_some_and(|seen| {
                            std::time::Instant::now().saturating_duration_since(seen)
                                < ESP32_OFFLINE_TIMEOUT
                        }) && Some(sequence) != last_sequence;
                        if !fresh {
                            return None;
                        }
                        let model = s.field_model.as_ref()?;
                        let recent_frames: Vec<Vec<f64>> =
                            node.field_model_history.iter().cloned().collect();
                        let residual = model.empty_room_match(&recent_frames)?.residual_energy;
                        let vital_signs_absent = s
                            .latest_update
                            .as_ref()
                            .is_some_and(|update| update.vital_signs.is_none());
                        Some((sequence, residual, vital_signs_absent))
                    })
                }
            };
            if observed.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        if let Some((sequence, residual, vital_signs_absent)) = observed {
            last_sequence = Some(sequence);
            residuals.push(residual);
            if !vital_signs_absent {
                vital_sign_sample_count += 1;
            }
        } else {
            stale_sample_count += 1;
        }
        next_sample_at = tokio::time::Instant::now()
            + Duration::from_millis(BOOTSTRAP_VALIDATION_MIN_SPACING_MS);
    }

    let validation_diagnostics = bootstrap_baseline::validation_diagnostics(
        residuals.len(),
        None,
        stale_sample_count,
        vital_sign_sample_count,
    );
    if residuals.len() != BOOTSTRAP_VALIDATION_SAMPLES
        || stale_sample_count != 0
        || vital_sign_sample_count != 0
    {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "bootstrap_refinement_validation_failed",
            "error": format!("The fresh empty-room refinement gate did not pass: {}. The stored baseline was not changed.", validation_diagnostics.failure_summary()),
            "validation": {
                "sample_count": residuals.len(),
                "stale_sample_count": stale_sample_count,
                "vital_sign_sample_count": vital_sign_sample_count,
            },
            "validation_diagnostics": validation_diagnostics,
        }));
    }

    let mut s = state.write().await;
    let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let identity_matches = s.bootstrap_baseline_active_at(observed_at_unix_ms)
        && s.bootstrap_baseline.as_ref().is_some_and(|metadata| {
            metadata.source_model_id == expected_model_id
                && metadata.content_sha256 == expected_digest
                && metadata.source_node_ids.as_slice() == [source_node_id]
                && CsiGridKey::from_bootstrap_grid(metadata.source_grid) == expected_grid
        })
        && s.calibration_grid_binding.is_some_and(|binding| {
            binding.source_node_id == source_node_id && binding.grid == expected_grid
        });
    if !identity_matches {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "bootstrap_changed",
            "error": "The bootstrap identity changed during refinement. The stored baseline was not changed.",
        }));
    }
    let candidate_snapshot = match s
        .field_model
        .as_ref()
        .ok_or(wifi_densepose_signal::ruvsense::field_model::FieldModelError::NotCalibrated)
        .and_then(|model| model.export_snapshot())
    {
        Ok(snapshot) => snapshot,
        Err(_) => {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "bootstrap_refinement_rejected",
                "error": "The bootstrap model rejected this bounded refinement.",
            }));
        }
    };
    let now_us = chrono::Utc::now().timestamp_micros() as u64;
    let mut candidate = match FieldModel::from_snapshot(candidate_snapshot, now_us) {
        Ok(model) => model,
        Err(_) => {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "bootstrap_refinement_rejected",
                "error": "The bootstrap model rejected this bounded refinement.",
            }));
        }
    };
    let refinement = match candidate.refine_empty_room_residual_boundary(&residuals) {
        Ok(refinement) => refinement,
        Err(_) => {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "bootstrap_refinement_rejected",
                "error": "The bootstrap model rejected this bounded refinement.",
            }));
        }
    };
    let Some(installation_id) = s.installation_id.clone() else {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "installation_id_required",
            "error": "A stable installation identity is required to store a local startup baseline.",
        }));
    };
    let path = bootstrap_baseline::path_in(&s.data_dir);
    let created_at_unix_ms = chrono::Utc::now().timestamp_millis() as u64;
    let stored = match bootstrap_baseline::store(
        &path,
        &installation_id,
        &[source_node_id],
        expected_grid.as_bootstrap_grid(),
        &expected_model_id,
        created_at_unix_ms,
        &candidate,
    ) {
        Ok(metadata) => metadata,
        Err(error) => {
            return error_response::internal_error_json("bootstrap baseline refinement store", error)
        }
    };
    s.field_model = Some(candidate);
    s.bootstrap_baseline = Some(stored.clone());
    Json(serde_json::json!({
        "success": true,
        "message": "The local empty-room startup baseline was refined.",
        "validation_diagnostics": validation_diagnostics,
        "validation": {
            "sample_count": residuals.len(),
            "stale_sample_count": stale_sample_count,
            "vital_sign_sample_count": vital_sign_sample_count,
        },
        "refinement": {
            "threshold_before": refinement.threshold_before,
            "threshold_after": refinement.threshold_after,
            "held_out_p95": refinement.held_out_p95,
            "held_out_sample_count": refinement.held_out_sample_count,
            "reference_window_count": refinement.reference_window_count,
        },
        "bootstrap_baseline": stored,
        "calibrated_evidence_authorized": false,
        "numeric_vitals_authorized": false,
    }))
}
