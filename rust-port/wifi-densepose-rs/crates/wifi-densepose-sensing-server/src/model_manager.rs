//! Model loading and lifecycle management API.
//!
//! Provides REST endpoints for listing, loading, and unloading `.rvf` models.
//! Models are stored in `data/models/` and inspected using `RvfReader`.
//!
//! Endpoints:
//! - `GET  /api/v1/models`              — list all available models
//! - `GET  /api/v1/models/:id`          — detailed info for a specific model
//! - `POST /api/v1/models/load`         — load a model for inference
//! - `POST /api/v1/models/unload`       — unload the active model
//! - `GET  /api/v1/models/active`       — get active model info
//! - `POST /api/v1/models/lora/activate` — activate a LoRA profile
//! - `GET  /api/v1/models/lora/profiles` — list LoRA profiles for active model

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Path as AxumPath, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::rvf_container::RvfReader;

// ── Models data directory ────────────────────────────────────────────────────

/// Base directory for RVF model files.
pub const MODELS_DIR: &str = "data/models";

// ── Types ────────────────────────────────────────────────────────────────────

/// Summary information for a model discovered on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub filename: String,
    pub version: String,
    pub description: String,
    pub size_bytes: u64,
    pub created_at: String,
    pub pck_score: Option<f64>,
    pub has_quantization: bool,
    pub lora_profiles: Vec<String>,
    pub segment_count: usize,
}

/// Information about the currently loaded model, including runtime stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveModelInfo {
    pub model_id: String,
    pub filename: String,
    pub version: String,
    pub description: String,
    pub avg_inference_ms: f64,
    pub frames_processed: u64,
    pub pose_source: String,
    pub lora_profiles: Vec<String>,
    pub active_lora_profile: Option<String>,
}

/// Runtime state for the loaded model.
///
/// Stored inside `AppStateInner` and read by the inference path.
pub struct LoadedModelState {
    /// Model identifier (derived from filename).
    pub model_id: String,
    /// Original filename.
    pub filename: String,
    /// Version string from the RVF manifest.
    pub version: String,
    /// Description from the RVF manifest.
    pub description: String,
    /// LoRA profiles available in this model.
    pub lora_profiles: Vec<String>,
    /// Currently active LoRA profile (if any).
    pub active_lora_profile: Option<String>,
    /// Model weights (f32 parameters).
    pub weights: Vec<f32>,
    /// Number of frames processed since load.
    pub frames_processed: u64,
    /// Cumulative inference time for avg calculation.
    pub total_inference_ms: f64,
    /// When the model was loaded.
    pub loaded_at: Instant,
}

/// Request body for `POST /api/v1/models/load`.
#[derive(Debug, Deserialize)]
pub struct LoadModelRequest {
    pub model_id: String,
}

/// Request body for `POST /api/v1/models/lora/activate`.
#[derive(Debug, Deserialize)]
pub struct ActivateLoraRequest {
    pub model_id: String,
    pub profile_name: String,
}

/// Shared application state type.
pub type AppState = Arc<RwLock<super::AppStateInner>>;

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Scan the models directory and build `ModelInfo` for each `.rvf` file.
async fn scan_models() -> Vec<ModelInfo> {
    let dir = PathBuf::from(MODELS_DIR);
    let mut models = Vec::new();

    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(e) => e,
        Err(_) => return models,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rvf") {
            continue;
        }

        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let id = filename.trim_end_matches(".rvf").to_string();

        let size_bytes = tokio::fs::metadata(&path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        // Read the RVF to extract manifest info.
        // This is a blocking I/O operation so we use spawn_blocking.
        let path_clone = path.clone();
        let info = tokio::task::spawn_blocking(move || {
            RvfReader::from_file(&path_clone).ok()
        })
        .await
        .unwrap_or(None);

        let (version, description, pck_score, has_quant, lora_profiles, segment_count, created_at) =
            if let Some(reader) = &info {
                let manifest = reader.manifest().unwrap_or_default();
                let metadata = reader.metadata().unwrap_or_default();
                let version = manifest
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let description = manifest
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let created_at = manifest
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let pck = metadata
                    .get("training")
                    .and_then(|t| t.get("best_pck"))
                    .and_then(|v| v.as_f64());
                let has_quant = reader.quant_info().is_some();
                let lora = reader.lora_profiles();
                let seg_count = reader.segment_count();
                (version, description, pck, has_quant, lora, seg_count, created_at)
            } else {
                (
                    "unknown".to_string(),
                    String::new(),
                    None,
                    false,
                    Vec::new(),
                    0,
                    String::new(),
                )
            };

        models.push(ModelInfo {
            id,
            filename,
            version,
            description,
            size_bytes,
            created_at,
            pck_score,
            has_quantization: has_quant,
            lora_profiles,
            segment_count,
        });
    }

    models.sort_by(|a, b| a.id.cmp(&b.id));
    models
}

/// Load a model from disk by ID and return its `LoadedModelState`.
fn load_model_from_disk(model_id: &str) -> Result<LoadedModelState, String> {
    let file_path = PathBuf::from(MODELS_DIR).join(format!("{model_id}.rvf"));
    let reader = RvfReader::from_file(&file_path)?;

    let manifest = reader.manifest().unwrap_or_default();
    let version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let description = manifest
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let filename = format!("{model_id}.rvf");
    let lora_profiles = reader.lora_profiles();
    let weights = reader.weights().unwrap_or_default();

    Ok(LoadedModelState {
        model_id: model_id.to_string(),
        filename,
        version,
        description,
        lora_profiles,
        active_lora_profile: None,
        weights,
        frames_processed: 0,
        total_inference_ms: 0.0,
        loaded_at: Instant::now(),
    })
}

// ── Axum handlers ────────────────────────────────────────────────────────────

async fn list_models(State(_state): State<AppState>) -> Json<serde_json::Value> {
    let models = scan_models().await;
    Json(serde_json::json!({
        "models": models,
        "count": models.len(),
    }))
}

async fn get_model(
    State(_state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Json<serde_json::Value> {
    let models = scan_models().await;
    match models.into_iter().find(|m| m.id == id) {
        Some(model) => Json(serde_json::to_value(&model).unwrap_or_default()),
        None => Json(serde_json::json!({
            "status": "error",
            "message": format!("Model '{id}' not found"),
        })),
    }
}

async fn load_model(
    State(state): State<AppState>,
    Json(body): Json<LoadModelRequest>,
) -> Json<serde_json::Value> {
    let model_id = body.model_id.clone();

    // Perform blocking file I/O on spawn_blocking.
    let load_result = tokio::task::spawn_blocking(move || load_model_from_disk(&model_id))
        .await
        .map_err(|e| format!("spawn_blocking panicked: {e}"));

    let loaded = match load_result {
        Ok(Ok(loaded)) => loaded,
        Ok(Err(e)) => {
            error!("Failed to load model '{}': {e}", body.model_id);
            return Json(serde_json::json!({
                "status": "error",
                "message": format!("Failed to load model: {e}"),
            }));
        }
        Err(e) => {
            error!("Internal error loading model: {e}");
            return Json(serde_json::json!({
                "status": "error",
                "message": format!("Internal error: {e}"),
            }));
        }
    };

    let model_id = loaded.model_id.clone();
    let weight_count = loaded.weights.len();

    {
        let mut s = state.write().await;
        s.loaded_model = Some(loaded);
        s.model_loaded = true;
    }

    info!("Model loaded: {model_id} ({weight_count} params)");

    Json(serde_json::json!({
        "status": "loaded",
        "model_id": model_id,
        "weight_count": weight_count,
    }))
}

async fn unload_model(State(state): State<AppState>) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    if s.loaded_model.is_none() {
        return Json(serde_json::json!({
            "status": "error",
            "message": "No model is currently loaded.",
        }));
    }

    let model_id = s
        .loaded_model
        .as_ref()
        .map(|m| m.model_id.clone())
        .unwrap_or_default();
    s.loaded_model = None;
    s.model_loaded = false;

    info!("Model unloaded: {model_id}");

    Json(serde_json::json!({
        "status": "unloaded",
        "model_id": model_id,
    }))
}

async fn active_model(State(state): State<AppState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.loaded_model {
        Some(model) => {
            let avg_ms = if model.frames_processed > 0 {
                model.total_inference_ms / model.frames_processed as f64
            } else {
                0.0
            };
            let info = ActiveModelInfo {
                model_id: model.model_id.clone(),
                filename: model.filename.clone(),
                version: model.version.clone(),
                description: model.description.clone(),
                avg_inference_ms: avg_ms,
                frames_processed: model.frames_processed,
                pose_source: "model_inference".to_string(),
                lora_profiles: model.lora_profiles.clone(),
                active_lora_profile: model.active_lora_profile.clone(),
            };
            Json(serde_json::to_value(&info).unwrap_or_default())
        }
        None => Json(serde_json::json!({
            "status": "no_model",
            "message": "No model is currently loaded.",
        })),
    }
}

async fn activate_lora(
    State(state): State<AppState>,
    Json(body): Json<ActivateLoraRequest>,
) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    let model = match s.loaded_model.as_mut() {
        Some(m) => m,
        None => {
            return Json(serde_json::json!({
                "status": "error",
                "message": "No model is loaded. Load a model first.",
            }));
        }
    };

    if model.model_id != body.model_id {
        return Json(serde_json::json!({
            "status": "error",
            "message": format!(
                "Model '{}' is not loaded. Active model: '{}'",
                body.model_id, model.model_id
            ),
        }));
    }

    if !model.lora_profiles.contains(&body.profile_name) {
        return Json(serde_json::json!({
            "status": "error",
            "message": format!(
                "LoRA profile '{}' not found. Available: {:?}",
                body.profile_name, model.lora_profiles
            ),
        }));
    }

    model.active_lora_profile = Some(body.profile_name.clone());
    info!(
        "LoRA profile activated: {} on model {}",
        body.profile_name, body.model_id
    );

    Json(serde_json::json!({
        "status": "activated",
        "model_id": body.model_id,
        "profile_name": body.profile_name,
    }))
}

async fn list_lora_profiles(State(state): State<AppState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.loaded_model {
        Some(model) => Json(serde_json::json!({
            "model_id": model.model_id,
            "profiles": model.lora_profiles,
            "active": model.active_lora_profile,
        })),
        None => Json(serde_json::json!({
            "profiles": serde_json::Value::Array(vec![]),
            "message": "No model is loaded.",
        })),
    }
}

// ── Router factory ───────────────────────────────────────────────────────────

/// Build the model management sub-router.
///
/// All routes are prefixed with `/api/v1/models`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/models", get(list_models))
        .route("/api/v1/models/active", get(active_model))
        .route("/api/v1/models/load", post(load_model))
        .route("/api/v1/models/unload", post(unload_model))
        .route("/api/v1/models/lora/activate", post(activate_lora))
        .route("/api/v1/models/lora/profiles", get(list_lora_profiles))
        .route("/api/v1/models/{id}", get(get_model))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_info_serializes() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            filename: "test-model.rvf".to_string(),
            version: "1.0.0".to_string(),
            description: "A test model".to_string(),
            size_bytes: 1024,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            pck_score: Some(0.85),
            has_quantization: false,
            lora_profiles: vec!["default".to_string()],
            segment_count: 5,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("test-model"));
        assert!(json.contains("0.85"));
    }

    #[test]
    fn active_model_info_serializes() {
        let info = ActiveModelInfo {
            model_id: "demo".to_string(),
            filename: "demo.rvf".to_string(),
            version: "0.1.0".to_string(),
            description: String::new(),
            avg_inference_ms: 2.5,
            frames_processed: 100,
            pose_source: "model_inference".to_string(),
            lora_profiles: vec![],
            active_lora_profile: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("model_inference"));
    }
}
