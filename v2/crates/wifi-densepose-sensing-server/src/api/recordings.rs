//! CSI Recording REST endpoints (ADR-080, ADR-166, ADR-295, issue #2088).
//!
//! Provides recording of live CSI streams to `.jsonl` files on disk,
//! with watermarking for synthetic data and redaction for privacy mode.

use axum::{
    extract::{Path, State},
    response::Json,
    routing::{delete, get, post},
    Router,
};
use std::path::{Path as StdPath, PathBuf};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};
use wifi_densepose_sensing_server::{error_response, privacy_filter};

use crate::SharedState;

#[inline]
fn chrono_timestamp() -> u64 {
    chrono::Utc::now().timestamp_millis().max(0) as u64
}

/// Return the directory CSI recordings are written to and read from:
/// `<data_dir>/recordings` (issue #2088).
#[inline]
pub fn recordings_dir(data_dir: &StdPath) -> PathBuf {
    data_dir.join("recordings")
}

/// Scan the recordings directory for `.jsonl` files and return metadata.
pub fn scan_recording_files(dir: &StdPath) -> Vec<serde_json::Value> {
    let mut recordings = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return recordings;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
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

        let frame_count = std::fs::read_to_string(&path)
            .map(|s| s.lines().count())
            .unwrap_or(0);

        recordings.push(serde_json::json!({
            "id": name,
            "name": name,
            "path": path.display().to_string(),
            "size_bytes": size,
            "frames": frame_count,
            "modified_epoch": modified,
            "status": "completed",
        }));
    }
    recordings
}

/// `GET /api/v1/recording/list` — list CSI recordings.
pub(crate) async fn list_recordings(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let recordings = scan_recording_files(&recordings_dir(&state.read().await.data_dir));
    Json(serde_json::json!({ "recordings": recordings }))
}

/// `POST /api/v1/recording/start` — start recording CSI data.
pub(crate) async fn start_recording(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    if s.recording_active {
        return Json(serde_json::json!({
            "error": "recording already in progress",
            "success": false,
            "recording_id": s.recording_current_id,
        }));
    }

    let id = body
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("rec_{}", chrono_timestamp()));

    let watermark = s.source_state().export_watermark();
    let rec_path = recordings_dir(&s.data_dir).join(format!("{id}.jsonl"));

    let file = match std::fs::File::create(&rec_path) {
        Ok(f) => f,
        Err(e) => return error_response::internal_error_json("recording create", e),
    };

    let (stop_tx, mut stop_rx) = tokio::sync::watch::channel(false);
    s.recording_active = true;
    s.recording_start_time = Some(std::time::Instant::now());
    s.recording_current_id = Some(id.clone());
    s.recording_stop_tx = Some(stop_tx);

    let mut rx = s.tx.subscribe();
    let privacy_mode = s.privacy_mode;

    s.recordings.push(serde_json::json!({
        "id": id,
        "path": rec_path.display().to_string(),
        "status": "recording",
        "started_at": chrono_timestamp(),
        "frames": 0,
        "watermark": watermark,
    }));

    let rec_id = id.clone();

    tokio::spawn(async move {
        use std::io::Write;
        let mut writer = std::io::BufWriter::new(file);
        let mut frame_count: u64 = 0;

        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(frame_json) => {
                            let frame_json = if privacy_mode {
                                match privacy_filter::redact_json_str(&frame_json) {
                                    Some(json) => json,
                                    None => continue,
                                }
                            } else {
                                frame_json
                            };
                            if writeln!(writer, "{frame_json}").is_err() {
                                warn!("Recording {rec_id}: write error, stopping");
                                break;
                            }
                            frame_count += 1;
                            if frame_count % 100 == 0 {
                                let _ = writer.flush();
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            debug!("Recording {rec_id}: lagged {n} frames");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Recording {rec_id}: broadcast closed, stopping");
                            break;
                        }
                    }
                }
                _ = stop_rx.changed() => {
                    if *stop_rx.borrow() {
                        info!("Recording {rec_id}: stop signal received ({frame_count} frames)");
                        break;
                    }
                }
            }
        }
        let _ = writer.flush();
        info!("Recording {rec_id} finished: {frame_count} frames written");
    });

    if let Some(mark) = watermark {
        info!("Recording started: {id} (source watermarked {mark})");
    } else {
        info!("Recording started: {id}");
    }

    Json(serde_json::json!({ "success": true, "recording_id": id, "watermark": watermark }))
}

/// `POST /api/v1/recording/stop` — stop recording CSI data.
pub(crate) async fn stop_recording(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    if !s.recording_active {
        return Json(serde_json::json!({
            "error": "no recording in progress",
            "success": false,
        }));
    }

    if let Some(tx) = s.recording_stop_tx.take() {
        let _ = tx.send(true);
    }

    let duration_secs = s
        .recording_start_time
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(0);
    let rec_id = s.recording_current_id.take().unwrap_or_default();
    s.recording_active = false;
    s.recording_start_time = None;

    for rec in s.recordings.iter_mut() {
        if rec.get("id").and_then(|v| v.as_str()) == Some(rec_id.as_str()) {
            rec["status"] = serde_json::json!("completed");
            rec["duration_secs"] = serde_json::json!(duration_secs);
        }
    }

    info!("Recording stopped: {rec_id} ({duration_secs}s)");
    Json(serde_json::json!({
        "success": true,
        "recording_id": rec_id,
        "duration_secs": duration_secs,
    }))
}

/// `DELETE /api/v1/recording/:id` — delete a recording file.
pub(crate) async fn delete_recording(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let safe_id = StdPath::new(&id)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    if safe_id.is_empty() || safe_id != id {
        return Json(serde_json::json!({ "error": "invalid recording id", "success": false }));
    }

    let path = recordings_dir(&state.read().await.data_dir).join(format!("{safe_id}.jsonl"));
    if !path.exists() {
        return Json(serde_json::json!({ "error": "recording not found", "success": false }));
    }

    if let Err(e) = std::fs::remove_file(&path) {
        return error_response::internal_error_json("recording delete", e);
    }

    let mut s = state.write().await;
    s.recordings
        .retain(|r| r.get("id").and_then(|v| v.as_str()) != Some(id.as_str()));

    info!("Recording deleted: {id}");
    Json(serde_json::json!({ "success": true, "deleted": id }))
}

/// Build the recording routes.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/recording/list", get(list_recordings))
        .route("/api/v1/recording/start", post(start_recording))
        .route("/api/v1/recording/stop", post(stop_recording))
        .route("/api/v1/recording/{id}", delete(delete_recording))
}
