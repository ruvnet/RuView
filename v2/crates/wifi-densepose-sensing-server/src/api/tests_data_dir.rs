//! Issue #2088: recordings and models live under `--data-dir`, not under
//! `data/` relative to the process working directory.

use std::sync::Arc;
use tokio::sync::RwLock;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::Request;
use axum::routing::{get, post};
use axum::{Json, Router};
use tower::ServiceExt;

use crate::api::models::{delete_model, list_models};
use crate::api::recordings::{
    delete_recording, list_recordings, start_recording, stop_recording,
};
use crate::state::{AppStateInner, SharedState};

/// The delete handlers are called directly: their `{id}` routes do not
/// match under axum 0.7 (tracked separately), and this test is about paths.
fn app_with_data_dir(dir: &std::path::Path) -> (Router, SharedState) {
    let mut inner = AppStateInner::minimal();
    inner.data_dir = dir.to_path_buf();
    let state: SharedState = Arc::new(RwLock::new(inner));
    let app = Router::new()
        .route("/api/v1/recording/list", get(list_recordings))
        .route("/api/v1/recording/start", post(start_recording))
        .route("/api/v1/recording/stop", post(stop_recording))
        .route("/api/v1/models", get(list_models))
        .with_state(state.clone());
    (app, state)
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut req = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(b) => {
            req = req.header("content-type", "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let resp = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("{method} {uri}: HTTP {status}, body not JSON: {e}"))
}

#[tokio::test]
async fn recording_start_list_delete_use_data_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let rec_dir = tmp.path().join("recordings");
    std::fs::create_dir_all(&rec_dir).unwrap();
    let (app, state) = app_with_data_dir(tmp.path());

    let v = call(
        &app,
        "POST",
        "/api/v1/recording/start",
        Some(serde_json::json!({"id": "dd_2088"})),
    )
    .await;
    assert_eq!(v["success"], true, "start failed: {v}");
    let v = call(&app, "POST", "/api/v1/recording/stop", None).await;
    assert_eq!(v["success"], true, "stop failed: {v}");
    assert!(
        rec_dir.join("dd_2088.jsonl").is_file(),
        "recording must be written under <data_dir>/recordings"
    );

    let v = call(&app, "GET", "/api/v1/recording/list", None).await;
    let ids: Vec<&str> = v["recordings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["id"].as_str())
        .collect();
    assert!(
        ids.contains(&"dd_2088"),
        "list must read <data_dir>/recordings: {v}"
    );

    let Json(v) = delete_recording(State(state), Path("dd_2088".to_string())).await;
    assert_eq!(v["success"], true, "delete failed: {v}");
    assert!(
        !rec_dir.join("dd_2088.jsonl").exists(),
        "delete must remove the file under <data_dir>/recordings"
    );
}

#[tokio::test]
async fn models_list_and_delete_use_data_dir() {
    if std::env::var_os("MODELS_DIR").is_some() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let models_dir = tmp.path().join("models");
    std::fs::create_dir_all(&models_dir).unwrap();
    std::fs::write(models_dir.join("dd_model_2088.rvf"), b"rvf").unwrap();
    let (app, state) = app_with_data_dir(tmp.path());

    let v = call(&app, "GET", "/api/v1/models", None).await;
    let ids: Vec<&str> = v["models"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["id"].as_str())
        .collect();
    assert!(
        ids.contains(&"dd_model_2088"),
        "models list must read <data_dir>/models: {v}"
    );

    let Json(v) = delete_model(State(state), Path("dd_model_2088".to_string())).await;
    assert_eq!(v["success"], true, "delete failed: {v}");
    assert!(!models_dir.join("dd_model_2088.rvf").exists());
}

