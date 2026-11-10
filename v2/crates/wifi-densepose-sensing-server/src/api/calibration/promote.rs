//! Validation and promotion of completed calibration into startup baseline.

use axum::{extract::State, Json};
use std::time::Duration;
use tracing::info;
use crate::bootstrap_baseline::{
    self, BootstrapValidationSample, BOOTSTRAP_VALIDATION_MIN_SPACING_MS,
    BOOTSTRAP_VALIDATION_SAMPLES, BOOTSTRAP_VALIDATION_SAMPLE_TIMEOUT_MS,
};
use crate::error_response;
use crate::{
    BOOTSTRAP_HOLDOUT_PREWARM_TIMEOUT_MS, ESP32_OFFLINE_TIMEOUT, FRAME_HISTORY_CAPACITY,
    SharedState,
};
use super::types::{calibration_identity_error, calibration_identity_matches, CalibrationIdentityRequest};

/// Validate a completed empty room model against twelve fresh server observed
/// samples, then persist only aggregate statistics.
pub(crate) async fn calibration_promote_bootstrap(
    State(state): State<SharedState>,
    Json(request): Json<CalibrationIdentityRequest>,
) -> Json<serde_json::Value> {
    let (expected_model_id, expected_binding, holdout_window_size) = {
        let s = state.read().await;
        if !calibration_identity_matches(&s, &request) {
            return calibration_identity_error();
        }
        let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
        if s.bootstrap_baseline_active || !s.explicit_calibration_fresh_at(observed_at_unix_ms) {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_not_finalized",
                "error": "Finalize an explicit room calibration before validating a startup baseline.",
            }));
        }
        let model_id = match s.calibration_model_id.clone() {
            Some(model_id) => model_id,
            None => {
                return Json(serde_json::json!({
                    "success": false,
                    "error_code": "calibration_model_identity_missing",
                    "error": "The active calibration has no server generated model identity.",
                }));
            }
        };
        let Some(binding) = s.calibration_grid_binding else {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_grid_identity_missing",
                "error": "The completed calibration has no frozen CSI grid identity.",
            }));
        };
        if s.calibration_sequence_fault_node_ids
            .contains(&binding.source_node_id)
        {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_sequence_discontinuity",
                "error": "The calibration CSI sequence faulted before validation. The model was not stored.",
            }));
        }
        let Some(runtime_window_size) = s
            .field_model
            .as_ref()
            .and_then(|model| model.modes())
            .and_then(|modes| modes.baseline_runtime_window_size)
            .filter(|window_size| *window_size > 0 && *window_size <= FRAME_HISTORY_CAPACITY)
        else {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_runtime_window_invalid",
                "error": "The completed calibration has no valid runtime scoring window.",
            }));
        };
        (model_id, binding, runtime_window_size)
    };

    let prewarm_deadline = tokio::time::Instant::now()
        + Duration::from_millis(BOOTSTRAP_HOLDOUT_PREWARM_TIMEOUT_MS);
    loop {
        let (identity_matches, sequence_faulted, ready, current_window_size) = {
            let s = state.read().await;
            let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
            let identity_matches = s.calibration_model_id.as_deref()
                == Some(expected_model_id.as_str())
                && s.explicit_calibration_fresh_at(observed_at_unix_ms)
                && s.calibration_grid_binding.is_some_and(|binding| {
                    binding.source_node_id == expected_binding.source_node_id
                        && binding.grid == expected_binding.grid
                });
            let sequence_faulted = s
                .calibration_sequence_fault_node_ids
                .contains(&expected_binding.source_node_id);
            let current_window_size = s
                .node_states
                .get(&expected_binding.source_node_id)
                .map_or(0, |node| node.field_model_history.len());
            (
                identity_matches,
                sequence_faulted,
                s.field_model_holdout_ready(
                    expected_binding,
                    holdout_window_size,
                    std::time::Instant::now(),
                ),
                current_window_size,
            )
        };
        if sequence_faulted {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_sequence_discontinuity",
                "error": "The calibration CSI sequence faulted while preparing held out validation. The model was not stored.",
            }));
        }
        if !identity_matches {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_model_changed",
                "error": "Calibration changed while preparing held out validation. The model was not stored.",
            }));
        }
        if ready {
            break;
        }
        if tokio::time::Instant::now() >= prewarm_deadline {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "bootstrap_holdout_prewarm_timeout",
                "error": "A complete fresh CSI scoring window was not available before the held out validation timeout. The model was not stored.",
                "required_window_size": holdout_window_size,
                "current_window_size": current_window_size,
            }));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let mut samples = Vec::with_capacity(BOOTSTRAP_VALIDATION_SAMPLES);
    let mut last_sequence = None;
    let mut next_sample_at = tokio::time::Instant::now();
    for _ in 0..BOOTSTRAP_VALIDATION_SAMPLES {
        tokio::time::sleep_until(next_sample_at).await;
        let interval_deadline = tokio::time::Instant::now()
            + Duration::from_millis(BOOTSTRAP_VALIDATION_SAMPLE_TIMEOUT_MS);
        let mut sample = BootstrapValidationSample {
            fresh_tick: false,
            calibrated_empty: false,
            vital_signs_absent: true,
        };
        while tokio::time::Instant::now() < interval_deadline {
            let observed = {
                let s = state.read().await;
                let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
                if s.calibration_model_id.as_deref() != Some(expected_model_id.as_str())
                    || !s.explicit_calibration_fresh_at(observed_at_unix_ms)
                    || !s.calibration_grid_binding.is_some_and(|binding| {
                        binding.source_node_id == expected_binding.source_node_id
                            && binding.grid == expected_binding.grid
                    })
                    || s.calibration_sequence_fault_node_ids
                        .contains(&expected_binding.source_node_id)
                {
                    None
                } else {
                    s.node_states
                        .get(&expected_binding.source_node_id)
                        .and_then(|node| {
                            let sequence = node.field_model_latest_sequence?;
                            let fresh = node.field_model_latest_seen.is_some_and(|seen| {
                                std::time::Instant::now().saturating_duration_since(seen)
                                    < ESP32_OFFLINE_TIMEOUT
                            }) && Some(sequence) != last_sequence;
                            let held_out = s.field_model_holdout_ready(
                                expected_binding,
                                holdout_window_size,
                                std::time::Instant::now(),
                            );
                            (fresh && held_out).then_some((
                                sequence,
                                s.person_count() == 0,
                                s.latest_update
                                    .as_ref()
                                    .is_none_or(|update| update.vital_signs.is_none()),
                            ))
                        })
                }
            };
            if let Some((sequence, calibrated_empty, vital_signs_absent)) = observed {
                last_sequence = Some(sequence);
                sample = BootstrapValidationSample {
                    fresh_tick: true,
                    calibrated_empty,
                    vital_signs_absent,
                };
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        samples.push(sample);
        next_sample_at = tokio::time::Instant::now()
            + Duration::from_millis(BOOTSTRAP_VALIDATION_MIN_SPACING_MS);
    }

    let validation = bootstrap_baseline::evaluate_validation(&samples);
    let validation_diagnostics = validation.diagnostics();
    if !validation.passed {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "bootstrap_validation_failed",
            "error": format!("The held out empty room check did not pass: {}. The model was not stored.", validation_diagnostics.failure_summary()),
            "validation": validation,
            "validation_diagnostics": validation_diagnostics,
        }));
    }

    let mut s = state.write().await;
    let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    if s.calibration_model_id.as_deref() != Some(expected_model_id.as_str())
        || !s.explicit_calibration_fresh_at(observed_at_unix_ms)
        || !s.calibration_grid_binding.is_some_and(|binding| {
            binding.source_node_id == expected_binding.source_node_id
                && binding.grid == expected_binding.grid
        })
        || s.calibration_sequence_fault_node_ids
            .contains(&expected_binding.source_node_id)
    {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "calibration_model_changed",
            "error": "Calibration changed during validation. The model was not stored.",
        }));
    }
    let Some(installation_id) = s.installation_id.clone() else {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "installation_id_required",
            "error": "A stable installation identity is required to store a local startup baseline.",
        }));
    };
    let source_node_ids = vec![expected_binding.source_node_id];
    let Some(field_model) = s.field_model.as_ref() else {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "no_field_model",
            "error": "Calibration changed during validation. The model was not stored.",
        }));
    };
    let path = bootstrap_baseline::path_in(&s.data_dir);
    let created_at_unix_ms = chrono::Utc::now().timestamp_millis() as u64;
    let stored = match bootstrap_baseline::store(
        &path,
        &installation_id,
        &source_node_ids,
        expected_binding.grid.as_bootstrap_grid(),
        &expected_model_id,
        created_at_unix_ms,
        field_model,
    ) {
        Ok(metadata) => metadata,
        Err(error) => {
            return error_response::internal_error_json("bootstrap baseline store", error)
        }
    };
    s.bootstrap_baseline = Some(stored.clone());
    info!(
        model_id = %stored.source_model_id,
        source_node_ids = ?stored.source_node_ids,
        content_sha256 = %stored.content_sha256,
        "Stored validated privacy reduced startup baseline"
    );
    Json(serde_json::json!({
        "success": true,
        "message": "Validated startup baseline stored locally.",
        "validation": validation,
        "validation_diagnostics": validation_diagnostics,
        "holdout_window_size": holdout_window_size,
        "bootstrap_baseline": stored,
        "calibrated_evidence_authorized": false,
        "numeric_vitals_authorized": false,
    }))
}
