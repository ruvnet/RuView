//! Health check endpoints

use crate::{state::AppState, Result};
use axum::{extract::State, response::IntoResponse, Json};
use serde::Serialize;

/// Health status response
#[derive(Debug, Serialize)]
pub struct HealthStatus {
    /// Server status
    pub status: String,
}

/// Readiness status response
#[derive(Debug, Serialize)]
pub struct ReadinessStatus {
    /// Server status
    pub status: String,
    /// Number of collections
    pub collections: usize,
    /// Total number of points across all collections
    pub total_points: usize,
}

/// Simple health check endpoint
///
/// GET /health
pub async fn health_check() -> Result<impl IntoResponse> {
    Ok(Json(HealthStatus {
        status: "healthy".to_string(),
    }))
}

/// Readiness check endpoint with stats
///
/// GET /ready
pub async fn readiness(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let collections_count = state.collection_count();

    // Note: VectorDB doesn't expose count directly, so we report collections only
    Ok(Json(ReadinessStatus {
        status: "ready".to_string(),
        collections: collections_count,
        total_points: 0, // Would require tracking or querying each DB
    }))
}
