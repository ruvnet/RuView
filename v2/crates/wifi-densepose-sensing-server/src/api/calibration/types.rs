//! Types and validation helpers for calibration requests.

use axum::Json;
use serde::Deserialize;
use super::identity::valid_source_node_ids;
use crate::AppStateInner;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct CalibrationStartQuery {
    /// Optional explicit single-link source.
    pub(crate) source_node_id: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CalibrationBindingRequest {
    pub(crate) binding_digest: String,
    pub(crate) source_node_ids: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CalibrationIdentityRequest {
    pub(crate) boot_epoch: String,
    pub(crate) session_id: Option<String>,
    pub(crate) binding_digest: Option<String>,
    pub(crate) source_node_ids: Option<Vec<u8>>,
}

pub(crate) fn calibration_identity_matches(
    s: &AppStateInner,
    request: &CalibrationIdentityRequest,
) -> bool {
    request.boot_epoch == s.calibration_boot_epoch
        && request.session_id.as_ref() == s.calibration_session_id.as_ref()
        && request.binding_digest.as_ref() == s.calibration_binding_digest.as_ref()
        && request.source_node_ids.as_ref().is_some_and(|ids| {
            ids.iter().copied().collect::<std::collections::BTreeSet<_>>()
                == s.calibration_source_node_ids
                && valid_source_node_ids(ids)
        })
}

pub(crate) fn calibration_identity_error() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "success": false,
        "error_code": "calibration_identity_mismatch",
        "error": "The calibration request does not match the active boot, session, room binding, and node set.",
    }))
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct BootstrapRefineQuery {
    #[serde(default)]
    pub(crate) confirmed_empty: bool,
    pub(crate) source_node_id: Option<u8>,
}
