//! REST API route definitions with versioning.
//!
//! Routes are organized by resource type and versioned under `/api/v1/`.

use axum::{
    routing::{get, post, put},
    Router,
};

use super::handlers;
use crate::AppContext;

/// Create the REST API router with all endpoints.
pub fn create_router(_ctx: AppContext) -> Router<AppContext> {
    Router::new()
        // Health check
        .route("/health", get(handlers::health_check))
        // Recordings
        .nest("/recordings", recordings_router())
        // Segments
        .nest("/segments", segments_router())
        // Clusters
        .nest("/clusters", clusters_router())
        // Evidence
        .nest("/evidence", evidence_router())
        // Search
        .route("/search", post(handlers::search))
}

/// Recording management routes.
fn recordings_router() -> Router<AppContext> {
    Router::new()
        // POST /recordings - Upload new recording
        .route("/", post(handlers::upload_recording))
        // GET /recordings/:id - Get recording by ID
        .route("/:id", get(handlers::get_recording))
}

/// Segment analysis routes.
fn segments_router() -> Router<AppContext> {
    Router::new()
        // GET /segments/:id/neighbors - Find similar segments
        .route("/:id/neighbors", get(handlers::get_neighbors))
}

/// Cluster management routes.
fn clusters_router() -> Router<AppContext> {
    Router::new()
        // GET /clusters - List all clusters
        .route("/", get(handlers::list_clusters))
        // GET /clusters/:id - Get specific cluster
        .route("/:id", get(handlers::get_cluster))
        // PUT /clusters/:id/label - Assign label to cluster
        .route("/:id/label", put(handlers::assign_cluster_label))
}

/// Evidence pack routes.
fn evidence_router() -> Router<AppContext> {
    Router::new()
        // POST /evidence - Generate evidence pack
        .route("/", post(handlers::generate_evidence_pack))
        // GET /evidence/:id - Get evidence pack by ID
        .route("/:id", get(handlers::get_evidence_pack))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    // Integration tests would go here with a mock AppContext
}
