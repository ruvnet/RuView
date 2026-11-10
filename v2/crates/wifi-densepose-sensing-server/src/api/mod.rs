//! Server HTTP REST API endpoints.
//!
//! Submodules organize routes by resource domain (models, recordings, etc.).

pub mod adaptive;
pub mod auth;
pub mod calibration;
pub mod config;
pub mod health;
pub mod introspection;
pub mod mediatek;
pub mod model_inspect;
pub mod models;
pub mod nodes;
pub mod pose;
pub mod recordings;
pub mod sensing;
pub mod stream;
pub mod vitals;

#[cfg(test)]
mod tests_adr186;
#[cfg(test)]
mod tests_data_dir;
#[cfg(test)]
mod tests_model_load;
#[cfg(test)]
mod tests_nodes_sync;
#[cfg(test)]
mod tests_pose_physics;
#[cfg(test)]
mod tests_privacy;
#[cfg(test)]
mod tests_status;

use axum::Router;
use crate::SharedState;

/// Combine all API domain sub-routers.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .merge(health::routes())
        .merge(model_inspect::routes())
        .merge(models::routes())
        .merge(nodes::routes())
        .merge(pose::routes())
        .merge(recordings::routes())
        .merge(sensing::routes())
        .merge(vitals::routes())
        .merge(mediatek::routes())
        .merge(auth::routes())
        .merge(config::routes())
        .merge(calibration::routes())
        .merge(adaptive::routes())
        .merge(introspection::routes())
        .merge(stream::routes())
}
