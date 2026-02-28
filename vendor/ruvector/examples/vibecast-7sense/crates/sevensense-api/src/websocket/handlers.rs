//! WebSocket handlers for real-time updates.
//!
//! These handlers manage WebSocket connections for streaming
//! processing status, cluster updates, and other real-time data.

use std::time::Duration;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{AppContext, ProcessingEvent, ProcessingStatus};

/// WebSocket message types for client communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    /// Processing status update
    #[serde(rename = "status")]
    Status(StatusUpdate),
    /// Error message
    #[serde(rename = "error")]
    Error(ErrorMessage),
    /// Ping/keepalive
    #[serde(rename = "ping")]
    Ping,
    /// Pong response
    #[serde(rename = "pong")]
    Pong,
    /// Subscription confirmation
    #[serde(rename = "subscribed")]
    Subscribed {
        /// Channel subscribed to
        channel: String,
    },
}

/// Processing status update message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusUpdate {
    /// Recording ID
    pub recording_id: Uuid,
    /// Status string
    pub status: String,
    /// Progress (0.0 to 1.0)
    pub progress: f32,
    /// Optional message
    pub message: Option<String>,
    /// Timestamp
    pub timestamp: i64,
}

/// Error message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    /// Error code
    pub code: String,
    /// Error message
    pub message: String,
}

impl From<ProcessingEvent> for StatusUpdate {
    fn from(event: ProcessingEvent) -> Self {
        Self {
            recording_id: event.recording_id,
            status: match event.status {
                ProcessingStatus::Queued => "queued",
                ProcessingStatus::Loading => "loading",
                ProcessingStatus::Segmenting => "segmenting",
                ProcessingStatus::Embedding => "embedding",
                ProcessingStatus::Indexing => "indexing",
                ProcessingStatus::Analyzing => "analyzing",
                ProcessingStatus::Complete => "complete",
                ProcessingStatus::Failed => "failed",
            }
            .to_string(),
            progress: event.progress,
            message: event.message,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// WebSocket handler for recording status updates.
///
/// Clients connect to `/ws/recordings/{id}` to receive real-time
/// status updates for a specific recording.
pub async fn recording_status_ws(
    ws: WebSocketUpgrade,
    Path(recording_id): Path<Uuid>,
    State(ctx): State<AppContext>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_recording_status(socket, recording_id, ctx))
}

async fn handle_recording_status(socket: WebSocket, recording_id: Uuid, ctx: AppContext) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to events
    let mut event_rx = ctx.subscribe_events();

    // Send subscription confirmation
    let confirm = WsMessage::Subscribed {
        channel: format!("recordings/{recording_id}"),
    };
    if let Ok(json) = serde_json::to_string(&confirm) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Spawn task to handle incoming messages (pings, etc.)
    let mut recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
    });

    // Main event loop
    let mut send_task = tokio::spawn(async move {
        // Keepalive interval
        let mut keepalive = tokio::time::interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                // Handle processing events
                event = event_rx.recv() => {
                    match event {
                        Ok(event) if event.recording_id == recording_id => {
                            let update: StatusUpdate = event.into();
                            let msg = WsMessage::Status(update);
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if sender.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                        _ => {}
                    }
                }
                // Keepalive ping
                _ = keepalive.tick() => {
                    let msg = WsMessage::Ping;
                    if let Ok(json) = serde_json::to_string(&msg) {
                        if sender.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    tracing::debug!(recording_id = %recording_id, "WebSocket connection closed");
}

/// WebSocket handler for all events stream.
///
/// Admin endpoint that streams all processing events.
pub async fn events_ws(ws: WebSocketUpgrade, State(ctx): State<AppContext>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_all_events(socket, ctx))
}

async fn handle_all_events(socket: WebSocket, ctx: AppContext) {
    let (mut sender, mut receiver) = socket.split();

    let mut event_rx = ctx.subscribe_events();

    // Send subscription confirmation
    let confirm = WsMessage::Subscribed {
        channel: "events".to_string(),
    };
    if let Ok(json) = serde_json::to_string(&confirm) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Spawn receiver task
    let mut recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
    });

    // Main send loop
    let mut send_task = tokio::spawn(async move {
        let mut keepalive = tokio::time::interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    match event {
                        Ok(event) => {
                            let update: StatusUpdate = event.into();
                            let msg = WsMessage::Status(update);
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if sender.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                        Err(_) => {} // Lagged, skip
                    }
                }
                _ = keepalive.tick() => {
                    if let Ok(json) = serde_json::to_string(&WsMessage::Ping) {
                        if sender.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    tracing::debug!("Events WebSocket connection closed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_message_serialization() {
        let msg = WsMessage::Status(StatusUpdate {
            recording_id: Uuid::new_v4(),
            status: "processing".to_string(),
            progress: 0.5,
            message: Some("Halfway done".to_string()),
            timestamp: 1_234_567_890,
        });

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("status"));
        assert!(json.contains("processing"));
    }

    #[test]
    fn test_status_update_from_event() {
        let event = ProcessingEvent {
            recording_id: Uuid::new_v4(),
            status: ProcessingStatus::Embedding,
            progress: 0.5,
            message: Some("Generating embeddings".to_string()),
        };

        let update: StatusUpdate = event.into();
        assert_eq!(update.status, "embedding");
        assert!((update.progress - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_error_message() {
        let error = WsMessage::Error(ErrorMessage {
            code: "not_found".to_string(),
            message: "Recording not found".to_string(),
        });

        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("not_found"));
    }
}
