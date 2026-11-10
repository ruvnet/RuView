use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt;

use crate::state::{AppStateInner, SharedState};
use crate::training_api;

static TRAIN_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn test_state() -> SharedState {
    Arc::new(RwLock::new(AppStateInner::minimal()))
}

#[tokio::test]
async fn ws_train_progress_route_is_wired_not_404() {
    let app = training_api::routes().with_state(test_state());
    let req = Request::builder()
        .uri("/ws/train/progress")
        .header("connection", "upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_ne!(resp.status(), StatusCode::NOT_FOUND, "route must not 404");
    assert_eq!(
        resp.status(),
        StatusCode::UPGRADE_REQUIRED,
        "a wired WS route returns 426 under oneshot — got {}",
        resp.status()
    );
}

#[tokio::test]
async fn ws_train_progress_live_101_and_frame() {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    use tokio_tungstenite::tungstenite::Message as TMsg;

    let _env_lock = TRAIN_ENV_LOCK.lock().unwrap();
    let shared = test_state();
    {
        let mut s = shared.write().await;
        for i in 0..40 {
            let sub: Vec<f64> = (0..56)
                .map(|k| 10.0 + ((i as f64) * 0.3 + (k as f64) * 0.1).sin() * 2.0)
                .collect();
            s.frame_history.push_back(sub);
        }
    }

    let app = training_api::routes().with_state(shared.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let (mut ws, resp) =
        tokio_tungstenite::connect_async(format!("ws://{addr}/ws/train/progress"))
            .await
            .expect("WebSocket handshake should succeed (101)");
    assert_eq!(resp.status().as_u16(), 101, "handshake must be 101");

    let body = r#"{"dataset_ids":[],"config":{"epochs":3,"batch_size":8,"warmup_epochs":1,"early_stopping_patience":10}}"#;
    let req = format!(
        "POST /api/v1/train/start HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let mut post = tokio::net::TcpStream::connect(addr).await.unwrap();
    post.write_all(req.as_bytes()).await.unwrap();
    post.flush().await.unwrap();

    let mut got_progress = false;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_secs(2), ws.next()).await {
            Ok(Some(Ok(TMsg::Text(txt)))) => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                    if v.get("type").and_then(|t| t.as_str()) == Some("progress") {
                        got_progress = true;
                        break;
                    }
                }
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(_))) | Ok(None) => break,
            Err(_) => {}
        }
    }
    assert!(
        got_progress,
        "should receive a real progress frame over the live WS after POST start"
    );
}

#[tokio::test]
async fn http_train_start_produces_model_and_streams() {
    let _env_lock = TRAIN_ENV_LOCK.lock().unwrap();
    let shared = test_state();
    {
        let mut s = shared.write().await;
        for i in 0..40 {
            let sub: Vec<f64> = (0..56)
                .map(|k| 10.0 + ((i as f64) * 0.3 + (k as f64) * 0.1).sin() * 2.0)
                .collect();
            s.frame_history.push_back(sub);
        }
    }
    let mut progress_rx = {
        let s = shared.read().await;
        s.training_progress_tx.subscribe()
    };

    let models_dir = crate::api::models::models_dir(&shared.read().await.data_dir);
    let before: std::collections::HashSet<std::path::PathBuf> = std::fs::read_dir(&models_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();

    let app = training_api::routes().with_state(shared.clone());

    let body = serde_json::json!({
        "dataset_ids": [],
        "config": {"epochs": 3, "batch_size": 8, "warmup_epochs": 1, "early_stopping_patience": 10}
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/train/start")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "start should be accepted");

    let mut completed = false;
    for _ in 0..250 {
        let req = Request::builder()
            .uri("/api/v1/train/status")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let bytes = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v.get("enabled"), Some(&serde_json::Value::Bool(true)));
        if v.get("active") == Some(&serde_json::Value::Bool(false))
            && v.get("phase").and_then(|p| p.as_str()) == Some("completed")
        {
            completed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(completed, "training should reach the completed phase");

    let mut saw_progress = false;
    while progress_rx.try_recv().is_ok() {
        saw_progress = true;
    }
    assert!(saw_progress, "expected streamed progress frames over the WS channel");

    let after: std::collections::HashSet<std::path::PathBuf> = std::fs::read_dir(&models_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    let new_models: Vec<_> = after
        .difference(&before)
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rvf"))
        .cloned()
        .collect();
    assert!(
        !new_models.is_empty(),
        "training should write a new .rvf model artifact under {}",
        models_dir.display()
    );
}

#[tokio::test]
async fn http_train_start_disabled_returns_structured_409() {
    let _env_lock = TRAIN_ENV_LOCK.lock().unwrap();
    std::env::set_var("RUVIEW_DISABLE_SERVER_TRAINING", "1");

    let app = training_api::routes().with_state(test_state());
    let body = serde_json::json!({"dataset_ids": [], "config": {"epochs": 1}});
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/train/start")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    std::env::remove_var("RUVIEW_DISABLE_SERVER_TRAINING");

    assert_eq!(status, StatusCode::CONFLICT, "disabled start must be 4xx/409");
    assert_eq!(v.get("enabled"), Some(&serde_json::Value::Bool(false)));
    assert_eq!(
        v.get("cli").and_then(|c| c.as_str()),
        Some("wifi-densepose train-room"),
        "must point at the CLI fallback, never a silent success"
    );
    assert_ne!(
        v.get("success"),
        Some(&serde_json::Value::Bool(true)),
        "must never claim success:true when disabled"
    );
}

#[tokio::test]
async fn http_train_uses_data_dir_for_recordings_and_models() {
    let _env_lock = TRAIN_ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let rec_dir = tmp.path().join("recordings");
    std::fs::create_dir_all(&rec_dir).unwrap();
    let lines: Vec<String> = (0..40)
        .map(|i| {
            let sub: Vec<f64> = (0..56)
                .map(|k| 10.0 + ((i as f64) * 0.3 + (k as f64) * 0.1).sin() * 2.0)
                .collect();
            serde_json::json!({ "timestamp": i as f64 * 0.1, "subcarriers": sub }).to_string()
        })
        .collect();
    std::fs::write(rec_dir.join("dd_train.csi.jsonl"), lines.join("\n")).unwrap();

    let shared = test_state();
    shared.write().await.data_dir = tmp.path().to_path_buf();
    let app = training_api::routes().with_state(shared.clone());

    let body = serde_json::json!({
        "dataset_ids": ["dd_train"],
        "config": {"epochs": 2, "batch_size": 8, "warmup_epochs": 1, "early_stopping_patience": 10}
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/train/start")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "start should be accepted");

    let mut phase = String::new();
    for _ in 0..250 {
        let req = Request::builder()
            .uri("/api/v1/train/status")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let bytes = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        phase = v
            .get("phase")
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string();
        if v.get("active") == Some(&serde_json::Value::Bool(false))
            && !phase.is_empty()
            && phase != "idle"
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(
        phase, "completed",
        "training should load the dataset from <data_dir>/recordings"
    );

    let models: Vec<_> = std::fs::read_dir(tmp.path().join("models"))
        .map(|rd| rd.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    assert!(
        models
            .iter()
            .any(|p| p.extension().and_then(|e| e.to_str()) == Some("rvf")),
        "the trained model should be written under <data_dir>/models, found {models:?}"
    );
}

