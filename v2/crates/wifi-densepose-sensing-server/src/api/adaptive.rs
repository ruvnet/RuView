//! Adaptive classifier endpoints.

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use tracing::{info, warn};
use crate::adaptive_classifier;
use crate::api::recordings::recordings_dir;
use crate::SharedState;

/// Create sub-router for adaptive classifier routes.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/adaptive/train", post(adaptive_train))
        .route("/api/v1/adaptive/status", get(adaptive_status))
        .route("/api/v1/adaptive/unload", post(adaptive_unload))
}

/// POST /api/v1/adaptive/train — train the adaptive classifier from recordings.
pub(crate) async fn adaptive_train(
    State(state): State<SharedState>,
) -> Json<serde_json::Value> {
    let (rec_dir, model_path) = {
        let s = state.read().await;
        (
            recordings_dir(&s.data_dir),
            adaptive_classifier::model_path(&s.data_dir),
        )
    };
    eprintln!("=== Adaptive Classifier Training ===");
    match adaptive_classifier::train_from_recordings(&rec_dir) {
        Ok(model) => {
            let accuracy = model.training_accuracy;
            let frames = model.trained_frames;
            let stats: Vec<_> = model
                .class_stats
                .iter()
                .map(|cs| {
                    serde_json::json!({
                        "class": cs.label,
                        "samples": cs.count,
                        "feature_means": cs.mean,
                    })
                })
                .collect();

            // Save to disk.
            if let Err(e) = model.save(&model_path) {
                warn!("Failed to save adaptive model: {e}");
            } else {
                info!("Adaptive model saved to {}", model_path.display());
            }

            // Load into runtime state.
            let mut s = state.write().await;
            s.adaptive_model = Some(model);

            Json(serde_json::json!({
                "success": true,
                "trained_frames": frames,
                "accuracy": accuracy,
                "class_stats": stats,
            }))
        }
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": e,
        })),
    }
}

/// GET /api/v1/adaptive/status — check adaptive model status.
pub(crate) async fn adaptive_status(
    State(state): State<SharedState>,
) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.adaptive_model {
        Some(model) => Json(serde_json::json!({
            "loaded": true,
            "trained_frames": model.trained_frames,
            "accuracy": model.training_accuracy,
            "version": model.version,
            "classes": model.class_names,
            "class_stats": model.class_stats,
        })),
        None => Json(serde_json::json!({
            "loaded": false,
            "message": "No adaptive model. POST /api/v1/adaptive/train to train one.",
        })),
    }
}

/// POST /api/v1/adaptive/unload — unload the adaptive model (revert to thresholds).
pub(crate) async fn adaptive_unload(
    State(state): State<SharedState>,
) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    s.adaptive_model = None;
    Json(serde_json::json!({
        "success": true,
        "message": "Adaptive model unloaded."
    }))
}
