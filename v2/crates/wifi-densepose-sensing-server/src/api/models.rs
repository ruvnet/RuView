//! Model management REST endpoints (ADR-166, issue #2088).
//!
//! Provides discovery, loading, unloading, and deletion of `.rvf` model packages
//! and LoRA adapter profiles.

use axum::{
    extract::{Path, State},
    response::Json,
    routing::{delete, get, post},
    Router,
};
use std::path::{Path as StdPath, PathBuf};
use tracing::info;
use wifi_densepose_sensing_server::{error_response, semconv, telemetry};

use crate::SharedState;

/// Return the effective models directory: the `MODELS_DIR` environment
/// variable if set, otherwise `<data_dir>/models` (issue #2088).
#[inline]
pub fn models_dir(data_dir: &StdPath) -> PathBuf {
    std::env::var_os("MODELS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("models"))
}

/// Scan the models directory for `.rvf` files and return metadata.
pub fn scan_model_files(dir: &StdPath) -> Vec<serde_json::Value> {
    let mut models = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return models;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rvf") {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let modified = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        models.push(serde_json::json!({
            "id": name,
            "name": name,
            "path": path.display().to_string(),
            "size_bytes": size,
            "format": "rvf",
            "modified_epoch": modified,
        }));
    }
    models
}

/// Scan the models directory for `.lora.json` LoRA profile files.
pub fn scan_lora_profiles(dir: &StdPath) -> Vec<serde_json::Value> {
    let mut profiles = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return profiles;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.ends_with(".lora.json") {
            continue;
        }

        let profile_name = name.trim_end_matches(".lora.json").to_string();
        let config = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        profiles.push(serde_json::json!({
            "name": profile_name,
            "path": path.display().to_string(),
            "config": config,
        }));
    }
    profiles
}

/// `GET /api/v1/models` — list discovered RVF model files.
pub(crate) async fn list_models(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let dir = models_dir(&state.read().await.data_dir);
    let models = scan_model_files(&dir);
    let total = models.len();
    {
        let mut s = state.write().await;
        s.discovered_models = models.clone();
    }
    Json(serde_json::json!({ "models": models, "total": total }))
}

/// `GET /api/v1/models/active` — return currently loaded model or null.
pub(crate) async fn get_active_model(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.active_model_id {
        Some(id) => {
            let model = s
                .discovered_models
                .iter()
                .find(|m| m.get("id").and_then(|v| v.as_str()) == Some(id.as_str()));
            Json(serde_json::json!({
                "active": model.cloned().unwrap_or_else(|| serde_json::json!({ "id": id })),
            }))
        }
        None => Json(serde_json::json!({ "active": serde_json::Value::Null })),
    }
}

/// `POST /api/v1/models/load` — load a model by ID.
pub(crate) async fn load_model(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let model_id = body
        .get("id")
        .or_else(|| body.get("model_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if model_id.is_empty() {
        return Json(serde_json::json!({ "error": "missing 'id' field", "success": false }));
    }

    let mut s = state.write().await;
    s.active_model_id = Some(model_id.clone());
    s.model_loaded = true;

    if telemetry::curated_events_enabled() {
        info!(
            name: semconv::EVENT_RUVIEW_MODEL_LOADED,
            { "ruview.model.id" = %model_id },
            "Model loaded: {model_id}"
        );
    } else {
        info!("Model loaded: {model_id}");
    }

    Json(serde_json::json!({ "success": true, "model_id": model_id }))
}

/// `POST /api/v1/models/unload` — unload the current model.
pub(crate) async fn unload_model(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    let prev = s.active_model_id.take();
    s.model_loaded = false;
    info!("Model unloaded (was: {:?})", prev);
    Json(serde_json::json!({ "success": true, "previous": prev }))
}

/// `DELETE /api/v1/models/:id` — delete a model file.
pub(crate) async fn delete_model(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let safe_id = StdPath::new(&id)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    if safe_id.is_empty() || safe_id != id {
        return Json(serde_json::json!({ "error": "invalid model id", "success": false }));
    }

    let path = models_dir(&state.read().await.data_dir).join(format!("{safe_id}.rvf"));
    if !path.exists() {
        return Json(serde_json::json!({ "error": "model not found", "success": false }));
    }

    if let Err(e) = std::fs::remove_file(&path) {
        return error_response::internal_error_json("model delete", e);
    }

    let mut s = state.write().await;
    if s.active_model_id.as_deref() == Some(id.as_str()) {
        s.active_model_id = None;
        s.model_loaded = false;
    }
    s.discovered_models
        .retain(|m| m.get("id").and_then(|v| v.as_str()) != Some(id.as_str()));

    info!("Model deleted: {id}");
    Json(serde_json::json!({ "success": true, "deleted": id }))
}

/// `GET /api/v1/models/lora/profiles` — list LoRA adapter profiles.
pub(crate) async fn list_lora_profiles(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let profiles = scan_lora_profiles(&models_dir(&state.read().await.data_dir));
    Json(serde_json::json!({ "profiles": profiles }))
}

/// `POST /api/v1/models/lora/activate` — activate a LoRA adapter profile.
pub(crate) async fn activate_lora_profile(Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let profile = body
        .get("profile")
        .or_else(|| body.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if profile.is_empty() {
        return Json(serde_json::json!({ "error": "missing 'profile' field", "success": false }));
    }

    info!("LoRA profile activated: {profile}");
    Json(serde_json::json!({ "success": true, "profile": profile }))
}

/// Build the model management routes.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/models", get(list_models))
        .route("/api/v1/models/active", get(get_active_model))
        .route("/api/v1/models/load", post(load_model))
        .route("/api/v1/models/unload", post(unload_model))
        .route("/api/v1/models/{id}", delete(delete_model))
        .route("/api/v1/models/lora/profiles", get(list_lora_profiles))
        .route("/api/v1/models/lora/activate", post(activate_lora_profile))
}
