//! Handler for calibration status query.

use axum::{extract::State, Json};
use crate::{
    CALIBRATION_SEQUENCE_REORDER_WINDOW, ESP32_OFFLINE_TIMEOUT, FRAME_HISTORY_CAPACITY,
    SharedState,
};

pub(crate) async fn calibration_status(
    State(state): State<SharedState>,
) -> Json<serde_json::Value> {
    let s = state.read().await;
    let now = std::time::Instant::now();
    let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let effective_status = s.field_model_status_at(observed_at_unix_ms);
    let bootstrap_active = s.bootstrap_baseline_active_at(observed_at_unix_ms);
    let bound = s.calibration_session_id.is_some();
    let active = bootstrap_active || bound;
    let bootstrap_background_match = s.bootstrap_background_match(observed_at_unix_ms);
    let runtime_reference = s.field_model.as_ref().and_then(|model| {
        model.modes().map(|modes| serde_json::json!({
            "window_size": modes.baseline_runtime_window_size,
            "significant_eigenvalue_p95": modes.baseline_runtime_eigenvalue_count,
            "residual_energy_threshold": modes.empty_room_residual_energy_threshold,
            "residual_reference_window_count": modes.empty_room_residual_energy_reference.len(),
            "residual_refinement_count": modes.empty_room_residual_refinement_count,
            "raw_calibration_frames_persisted": false,
        }))
    });
    let (frame_count, min_frames, elapsed_s, frames_per_second, min_duration_s) = s
        .field_model
        .as_ref()
        .map_or((0, 0, 0.0, 0.0, 0.0), |model| {
            (
                model.calibration_frame_count(),
                model.min_calibration_frames(),
                model.calibration_elapsed_s(),
                model.calibration_frames_per_second(),
                model.min_calibration_duration_s(),
            )
        });
    let status = effective_status.map_or_else(
        || "none".to_string(),
        |status| format!("{status:?}").to_lowercase(),
    );
    let status = if active || status == "expired" {
        status
    } else {
        "none".to_string()
    };
    let grid_binding = s.calibration_grid_binding.map(|binding| {
        let node = s.node_states.get(&binding.source_node_id);
        let latest_seen_ms = node
            .and_then(|node| node.field_model_latest_seen)
            .map(|seen| now.saturating_duration_since(seen).as_millis() as u64);
        serde_json::json!({
            "source_node_id": binding.source_node_id,
            "grid": binding.grid,
            "selection_evidence": binding.evidence,
            "latest_seen_ms": latest_seen_ms,
            "runtime_history_frames": node.map_or(0, |node| node.field_model_history.len()),
            "runtime_history_capacity": FRAME_HISTORY_CAPACITY,
            "status": if latest_seen_ms.is_some_and(|age| age < ESP32_OFFLINE_TIMEOUT.as_millis() as u64)
                && !s.calibration_sequence_fault_node_ids.contains(&binding.source_node_id)
            { "active" } else { "stale" },
        })
    });
    let mut response = serde_json::json!({
        "active": active,
        "status": status,
        "frame_count": frame_count,
        "min_frames": min_frames,
        "elapsed_s": elapsed_s,
        "frames_per_second": frames_per_second,
        "min_duration_s": min_duration_s,
        "model_id": s.calibration_model_id,
        "source_node_ids": s.calibration_source_node_ids,
        "grid_binding": grid_binding,
        "last_sequence_by_node": s.calibration_last_sequences,
        "sequence_policy": "forward_only_with_bounded_udp_reorder_drop_v1",
        "reorder_window_sequences": CALIBRATION_SEQUENCE_REORDER_WINDOW,
        "reordered_packets_by_node": s.calibration_reordered_packets,
        "max_reorder_depth_by_node": s.calibration_max_reorder_depth,
        "sequence_fault_node_ids": s.calibration_sequence_fault_node_ids,
        "runtime_reference": runtime_reference,
        "binding_mode": if bootstrap_active { "bootstrap_only" } else if bound { "bound" } else { "none" },
        "bootstrap_baseline": s.bootstrap_baseline.as_ref().map(|metadata| serde_json::json!({
            "stored": true,
            "active": bootstrap_active,
            "authority": metadata.authority,
            "source_node_ids": metadata.source_node_ids,
            "source_grid": metadata.source_grid,
            "source_model_id": metadata.source_model_id,
            "created_at_unix_ms": metadata.created_at_unix_ms,
            "expires_at_unix_ms": metadata.expires_at_unix_ms,
            "content_sha256": metadata.content_sha256,
            "background_match": bootstrap_background_match.map(|result| serde_json::json!({
                "state": if result.matches_empty { "matched" } else { "changed" },
                "matches_empty": result.matches_empty,
                "score": result.score,
                "normalized_residual_z": result.normalized_residual_z,
                "maturity": result.maturity,
                "reliable": result.reliable,
                "residual_energy": result.residual_energy,
                "residual_energy_threshold": result.residual_energy_threshold,
                "window_size": result.window_size,
                "reference_window_count": result.reference_window_count,
            })),
            "calibrated_evidence_authorized": false,
            "numeric_vitals_authorized": false,
        })).unwrap_or_else(|| serde_json::json!({
            "stored": false,
            "active": false,
            "authority": "none",
            "calibrated_evidence_authorized": false,
            "numeric_vitals_authorized": false,
        })),
    });
    response["boot_epoch"] = serde_json::json!(s.calibration_boot_epoch);
    response["session_id"] = serde_json::json!(if bound { s.calibration_session_id.clone() } else { None });
    response["binding_digest"] = serde_json::json!(if bound { s.calibration_binding_digest.clone() } else { None });
    response["legacy"] = serde_json::json!(false);
    response["model_id"] = serde_json::json!(if bound { s.calibration_model_id.clone() } else { None });
    response["model_receipt"] = serde_json::json!(if bound { s.calibration_model_receipt.clone() } else { None });
    response["source_node_ids"] = serde_json::json!(if bound { s.calibration_source_node_ids.iter().copied().collect::<Vec<_>>() } else { Vec::new() });
    response["observed_source_node_ids"] = serde_json::json!(if bound { s.calibration_observed_source_node_ids.iter().copied().collect::<Vec<_>>() } else { Vec::new() });
    response["missing_source_node_ids"] = serde_json::json!(if bound { s.calibration_source_node_ids.difference(&s.calibration_observed_source_node_ids).copied().collect::<Vec<_>>() } else { Vec::new() });
    Json(response)
}
