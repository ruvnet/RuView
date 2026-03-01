//! Point operations endpoints

use crate::{error::Error, state::AppState, Result};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use ruvector_core::{SearchQuery, SearchResult, VectorEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Point upsert request
#[derive(Debug, Deserialize)]
pub struct UpsertPointsRequest {
    /// Points to upsert
    pub points: Vec<VectorEntry>,
}

/// Search request
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    /// Query vector
    pub vector: Vec<f32>,
    /// Number of results to return
    #[serde(default = "default_limit")]
    pub k: usize,
    /// Optional score threshold
    pub score_threshold: Option<f32>,
    /// Optional metadata filters
    pub filter: Option<HashMap<String, serde_json::Value>>,
}

fn default_limit() -> usize {
    10
}

/// Search response
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    /// Search results
    pub results: Vec<SearchResult>,
}

/// Upsert response
#[derive(Debug, Serialize)]
pub struct UpsertResponse {
    /// IDs of upserted points
    pub ids: Vec<String>,
}

/// Create point routes
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/collections/:name/points", put(upsert_points))
        .route("/collections/:name/points/search", post(search_points))
        .route("/collections/:name/points/:id", get(get_point))
}

/// Upsert points into a collection
///
/// PUT /collections/:name/points
async fn upsert_points(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<UpsertPointsRequest>,
) -> Result<impl IntoResponse> {
    let db = state
        .get_collection(&name)
        .ok_or_else(|| Error::CollectionNotFound(name.clone()))?;

    let ids = db.insert_batch(req.points).map_err(Error::Core)?;

    Ok((StatusCode::OK, Json(UpsertResponse { ids })))
}

/// Search for similar points
///
/// POST /collections/:name/points/search
async fn search_points(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<SearchRequest>,
) -> Result<impl IntoResponse> {
    let db = state
        .get_collection(&name)
        .ok_or_else(|| Error::CollectionNotFound(name))?;

    let query = SearchQuery {
        vector: req.vector,
        k: req.k,
        filter: req.filter,
        ef_search: None,
    };

    let mut results = db.search(query).map_err(Error::Core)?;

    // Apply score threshold if provided
    if let Some(threshold) = req.score_threshold {
        results.retain(|r| r.score >= threshold);
    }

    Ok(Json(SearchResponse { results }))
}

/// Get a point by ID
///
/// GET /collections/:name/points/:id
async fn get_point(
    State(state): State<AppState>,
    Path((name, id)): Path<(String, String)>,
) -> Result<impl IntoResponse> {
    let db = state
        .get_collection(&name)
        .ok_or_else(|| Error::CollectionNotFound(name))?;

    let entry = db.get(&id).map_err(Error::Core)?;

    Ok(Json(entry))
}
