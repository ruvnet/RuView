use axum::{body::Body, http::{Request, StatusCode}, routing::get, Router};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt;

use crate::api::pose::{pose_current, pose_physics_metrics};
use crate::state::AppStateInner;

fn app() -> Router {
    Router::new()
        .route("/api/v1/pose/current", get(pose_current))
        .route("/api/v1/pose/physics/metrics", get(pose_physics_metrics))
        .with_state(Arc::new(RwLock::new(AppStateInner::minimal())))
}

#[tokio::test]
async fn raw_view_remains_backward_compatible() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/v1/pose/current?view=raw")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 65_536)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(body.get("persons").is_some());
    assert!(body.get("physics").is_none());
}

#[tokio::test]
async fn refined_view_is_typed_conflict_when_no_selected_pose_exists() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/v1/pose/current?view=refined")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = axum::body::to_bytes(response.into_body(), 65_536)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["code"], "pose_refined_unavailable");
}

#[tokio::test]
async fn physics_metrics_use_prometheus_content_type() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/v1/pose/physics/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[axum::http::header::CONTENT_TYPE],
        "text/plain; version=0.0.4"
    );
}

