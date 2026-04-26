//! CSI frame recording API.
//!
//! Provides REST endpoints for recording CSI frames to `.csi.jsonl` files.
//! When recording is active, each processed CSI frame is appended as a JSON
//! line to the current session file stored under `data/recordings/`.
//!
//! Endpoints:
//! - `POST /api/v1/recording/start`   — start a new recording session
//! - `POST /api/v1/recording/stop`    — stop the active recording
//! - `GET  /api/v1/recording/list`    — list all recording sessions
//! - `GET  /api/v1/recording/download/:id` — download a recording file
//! - `DELETE /api/v1/recording/:id`   — delete a recording

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Path as AxumPath, State},
    response::{IntoResponse, Json},
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

// ── Recording data directory ─────────────────────────────────────────────────

/// Base directory for recording files.
pub const RECORDINGS_DIR: &str = "data/recordings";

// ── Types ────────────────────────────────────────────────────────────────────

/// Request body for `POST /api/v1/recording/start`.
#[derive(Debug, Deserialize)]
pub struct StartRecordingRequest {
    pub session_name: String,
    pub label: Option<String>,
    pub duration_secs: Option<u64>,
}

/// Metadata for a completed or active recording session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSession {
    pub id: String,
    pub name: String,
    pub label: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub frame_count: u64,
    pub file_size_bytes: u64,
    pub file_path: String,
}

/// A single recorded CSI frame line (JSONL format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedFrame {
    pub timestamp: f64,
    pub subcarriers: Vec<f64>,
    pub rssi: f64,
    pub noise_floor: f64,
    pub features: serde_json::Value,
}

/// Runtime state for the active recording session.
///
/// Stored inside `AppStateInner` and checked on each CSI frame tick.
pub struct RecordingState {
    /// Whether a recording is currently active.
    pub active: bool,
    /// Session ID of the active recording.
    pub session_id: String,
    /// Session display name.
    pub session_name: String,
    /// Optional label / activity tag.
    pub label: Option<String>,
    /// Path to the JSONL file being written.
    pub file_path: PathBuf,
    /// Number of frames written so far.
    pub frame_count: u64,
    /// When the recording started.
    pub start_time: Instant,
    /// ISO-8601 start timestamp for metadata.
    pub started_at: String,
    /// Optional auto-stop duration.
    pub duration_secs: Option<u64>,
}

impl Default for RecordingState {
    fn default() -> Self {
        Self {
            active: false,
            session_id: String::new(),
            session_name: String::new(),
            label: None,
            file_path: PathBuf::new(),
            frame_count: 0,
            start_time: Instant::now(),
            started_at: String::new(),
            duration_secs: None,
        }
    }
}

/// Shared application state type used across all handlers.
pub type AppState = Arc<RwLock<super::AppStateInner>>;

// ── Public helpers (called from the CSI processing loop in main.rs) ──────────

/// Append a single frame to the active recording file.
///
/// This is designed to be called from the main CSI processing tick.
/// If recording is not active, it returns immediately.
pub async fn maybe_record_frame(
    state: &AppState,
    subcarriers: &[f64],
    rssi: f64,
    noise_floor: f64,
    features: &serde_json::Value,
) {
    let should_write;
    let file_path;
    let auto_stop;
    {
        let s = state.read().await;
        let rec = &s.recording_state;
        if !rec.active {
            return;
        }
        should_write = true;
        file_path = rec.file_path.clone();
        auto_stop = rec.duration_secs.map(|d| rec.start_time.elapsed().as_secs() >= d).unwrap_or(false);
    }

    if auto_stop {
        // Duration exceeded — stop recording.
        stop_recording_inner(state).await;
        return;
    }

    if !should_write {
        return;
    }

    let frame = RecordedFrame {
        timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
        subcarriers: subcarriers.to_vec(),
        rssi,
        noise_floor,
        features: features.clone(),
    };

    let line = match serde_json::to_string(&frame) {
        Ok(l) => l,
        Err(e) => {
            warn!("Failed to serialize recording frame: {e}");
            return;
        }
    };

    // Append line to file (async).
    if let Err(e) = append_line(&file_path, &line).await {
        warn!("Failed to write recording frame: {e}");
        return;
    }

    // Increment frame counter.
    {
        let mut s = state.write().await;
        s.recording_state.frame_count += 1;
    }
}

async fn append_line(path: &Path, line: &str) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    file.write_all(line.as_bytes()).await?;
    file.write_all(b"\n").await?;
    Ok(())
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Stop the active recording and write session metadata.
async fn stop_recording_inner(state: &AppState) {
    let mut s = state.write().await;
    if !s.recording_state.active {
        return;
    }
    s.recording_state.active = false;

    let ended_at = chrono::Utc::now().to_rfc3339();
    let session = RecordingSession {
        id: s.recording_state.session_id.clone(),
        name: s.recording_state.session_name.clone(),
        label: s.recording_state.label.clone(),
        started_at: s.recording_state.started_at.clone(),
        ended_at: Some(ended_at),
        frame_count: s.recording_state.frame_count,
        file_size_bytes: std::fs::metadata(&s.recording_state.file_path)
            .map(|m| m.len())
            .unwrap_or(0),
        file_path: s.recording_state.file_path.to_string_lossy().to_string(),
    };

    // Write a companion .meta.json alongside the JSONL file.
    let meta_path = s.recording_state.file_path.with_extension("meta.json");
    if let Ok(json) = serde_json::to_string_pretty(&session) {
        if let Err(e) = tokio::fs::write(&meta_path, json).await {
            warn!("Failed to write recording metadata: {e}");
        }
    }

    info!(
        "Recording stopped: {} ({} frames)",
        session.id, session.frame_count
    );
}

/// Scan the recordings directory and return all sessions with metadata.
async fn list_sessions() -> Vec<RecordingSession> {
    let dir = PathBuf::from(RECORDINGS_DIR);
    let mut sessions = Vec::new();

    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(e) => e,
        Err(_) => return sessions,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json")
            && path.to_string_lossy().contains(".meta.")
        {
            if let Ok(data) = tokio::fs::read_to_string(&path).await {
                if let Ok(session) = serde_json::from_str::<RecordingSession>(&data) {
                    sessions.push(session);
                }
            }
        }
    }

    // Sort by started_at descending (newest first).
    sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    sessions
}

// ── Axum handlers ────────────────────────────────────────────────────────────

async fn start_recording(
    State(state): State<AppState>,
    Json(body): Json<StartRecordingRequest>,
) -> Json<serde_json::Value> {
    // Ensure recordings directory exists.
    if let Err(e) = tokio::fs::create_dir_all(RECORDINGS_DIR).await {
        error!("Failed to create recordings directory: {e}");
        return Json(serde_json::json!({
            "status": "error",
            "message": format!("Cannot create recordings directory: {e}"),
        }));
    }

    let mut s = state.write().await;
    if s.recording_state.active {
        return Json(serde_json::json!({
            "status": "error",
            "message": "A recording is already active. Stop it first.",
            "active_session": s.recording_state.session_id,
        }));
    }

    let session_id = format!(
        "{}-{}",
        body.session_name.replace(' ', "_"),
        chrono::Utc::now().format("%Y%m%d_%H%M%S")
    );
    let file_name = format!("{session_id}.csi.jsonl");
    let file_path = PathBuf::from(RECORDINGS_DIR).join(&file_name);
    let started_at = chrono::Utc::now().to_rfc3339();

    s.recording_state = RecordingState {
        active: true,
        session_id: session_id.clone(),
        session_name: body.session_name.clone(),
        label: body.label.clone(),
        file_path: file_path.clone(),
        frame_count: 0,
        start_time: Instant::now(),
        started_at: started_at.clone(),
        duration_secs: body.duration_secs,
    };

    info!(
        "Recording started: {session_id} (label={:?}, duration={:?}s)",
        body.label, body.duration_secs
    );

    Json(serde_json::json!({
        "status": "recording",
        "session_id": session_id,
        "session_name": body.session_name,
        "label": body.label,
        "started_at": started_at,
        "file_path": file_path.to_string_lossy(),
        "duration_secs": body.duration_secs,
    }))
}

async fn stop_recording(State(state): State<AppState>) -> Json<serde_json::Value> {
    {
        let s = state.read().await;
        if !s.recording_state.active {
            return Json(serde_json::json!({
                "status": "error",
                "message": "No active recording to stop.",
            }));
        }
    }

    stop_recording_inner(&state).await;

    let s = state.read().await;
    Json(serde_json::json!({
        "status": "stopped",
        "session_id": s.recording_state.session_id,
        "frame_count": s.recording_state.frame_count,
    }))
}

async fn list_recordings(
    State(_state): State<AppState>,
) -> Json<serde_json::Value> {
    let sessions = list_sessions().await;
    Json(serde_json::json!({
        "recordings": sessions,
        "count": sessions.len(),
    }))
}

async fn download_recording(
    State(_state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    let dir = PathBuf::from(RECORDINGS_DIR);
    // Find the JSONL file matching the ID.
    let file_path = dir.join(format!("{id}.csi.jsonl"));

    if !file_path.exists() {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "status": "error",
                "message": format!("Recording '{id}' not found"),
            })),
        )
            .into_response();
    }

    match tokio::fs::read(&file_path).await {
        Ok(data) => {
            let headers = [
                (
                    axum::http::header::CONTENT_TYPE,
                    "application/x-ndjson".to_string(),
                ),
                (
                    axum::http::header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{id}.csi.jsonl\""),
                ),
            ];
            (headers, data).into_response()
        }
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": format!("Failed to read recording: {e}"),
            })),
        )
            .into_response(),
    }
}

async fn delete_recording(
    State(_state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Json<serde_json::Value> {
    let dir = PathBuf::from(RECORDINGS_DIR);
    let jsonl_path = dir.join(format!("{id}.csi.jsonl"));
    let meta_path = dir.join(format!("{id}.csi.meta.json"));

    if !jsonl_path.exists() && !meta_path.exists() {
        return Json(serde_json::json!({
            "status": "error",
            "message": format!("Recording '{id}' not found"),
        }));
    }

    let mut deleted = Vec::new();
    if jsonl_path.exists() {
        if let Err(e) = tokio::fs::remove_file(&jsonl_path).await {
            warn!("Failed to delete {}: {e}", jsonl_path.display());
        } else {
            deleted.push(jsonl_path.to_string_lossy().to_string());
        }
    }
    if meta_path.exists() {
        if let Err(e) = tokio::fs::remove_file(&meta_path).await {
            warn!("Failed to delete {}: {e}", meta_path.display());
        } else {
            deleted.push(meta_path.to_string_lossy().to_string());
        }
    }

    Json(serde_json::json!({
        "status": "deleted",
        "id": id,
        "deleted_files": deleted,
    }))
}

// ── Router factory ───────────────────────────────────────────────────────────

/// Build the recording sub-router.
///
/// Mount this at the top level; all routes are prefixed with `/api/v1/recording`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/recording/start", post(start_recording))
        .route("/api/v1/recording/stop", post(stop_recording))
        .route("/api/v1/recording/list", get(list_recordings))
        .route(
            "/api/v1/recording/download/{id}",
            get(download_recording),
        )
        .route("/api/v1/recording/{id}", delete(delete_recording))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_recording_state_is_inactive() {
        let rs = RecordingState::default();
        assert!(!rs.active);
        assert_eq!(rs.frame_count, 0);
    }

    #[test]
    fn recorded_frame_serializes_to_json() {
        let frame = RecordedFrame {
            timestamp: 1700000000.0,
            subcarriers: vec![1.0, 2.0, 3.0],
            rssi: -45.0,
            noise_floor: -90.0,
            features: serde_json::json!({"motion": 0.5}),
        };
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("\"timestamp\""));
        assert!(json.contains("\"subcarriers\""));
    }

    #[test]
    fn recording_session_deserializes() {
        let json = r#"{
            "id": "test-20240101_120000",
            "name": "test",
            "label": "walking",
            "started_at": "2024-01-01T12:00:00Z",
            "ended_at": "2024-01-01T12:05:00Z",
            "frame_count": 3000,
            "file_size_bytes": 1500000,
            "file_path": "data/recordings/test-20240101_120000.csi.jsonl"
        }"#;
        let session: RecordingSession = serde_json::from_str(json).unwrap();
        assert_eq!(session.id, "test-20240101_120000");
        assert_eq!(session.frame_count, 3000);
        assert_eq!(session.label, Some("walking".to_string()));
    }
}
