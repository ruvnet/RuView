//! Calibration API module and route aggregator.

pub(crate) mod cancel;
pub(crate) mod identity;
pub(crate) mod promote;
pub(crate) mod refine;
pub(crate) mod start;
pub(crate) mod status;
pub(crate) mod stop;
pub(crate) mod types;

#[cfg(test)]
mod tests_expiry;
#[cfg(test)]
mod tests_identity;
#[cfg(test)]
mod tests_grid_selection;
#[cfg(test)]
mod tests_sequence_holdout;
#[cfg(test)]
mod tests_vital_publication;

use axum::{
    routing::{get, post},
    Router,
};
use crate::SharedState;

pub(crate) use cancel::{calibration_cancel, calibration_reset};
pub(crate) use identity::{
    calibration_source_accepts, calibration_source_has_fresh_csi, opaque_calibration_id,
    opaque_calibration_model_id, valid_binding_digest, valid_source_node_ids,
};
pub(crate) use promote::calibration_promote_bootstrap;
pub(crate) use refine::calibration_refine_bootstrap;
pub(crate) use start::{calibration_start, validate_startup_calibration_request};
pub(crate) use status::calibration_status;
pub(crate) use stop::calibration_stop;
pub(crate) use types::{
    calibration_identity_error, calibration_identity_matches, BootstrapRefineQuery,
    CalibrationBindingRequest, CalibrationIdentityRequest, CalibrationStartQuery,
};

/// Field model calibration routes.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        // Dashboard-compatible aliases for field-model calibration API
        .route("/api/v1/pose/calibrate", post(calibration_start))
        .route("/api/v1/pose/calibration/status", get(calibration_status))
        // Explicit calibration endpoints
        .route("/api/v1/calibration/start", post(calibration_start))
        .route("/api/v1/calibration/stop", post(calibration_stop))
        .route("/api/v1/calibration/cancel", post(calibration_cancel))
        .route("/api/v1/calibration/status", get(calibration_status))
        .route("/api/v1/calibration/reset", post(calibration_reset))
        .route(
            "/api/v1/calibration/bootstrap/promote",
            post(calibration_promote_bootstrap),
        )
        .route(
            "/api/v1/calibration/bootstrap/refine",
            post(calibration_refine_bootstrap),
        )
}
