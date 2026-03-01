//! WebSocket live event streaming for the RVF dashboard.
//!
//! Provides a `/ws/live` endpoint that broadcasts real-time events
//! (boundary alerts, new candidates, status updates) to connected clients.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use serde::Serialize;
use tokio::sync::broadcast;

use crate::http::AppState;

/// Live event broadcast to WebSocket clients.
#[derive(Clone, Debug, Serialize)]
pub struct LiveEvent {
    pub event_type: String,
    pub timestamp: String,
    pub data: serde_json::Value,
}

/// Shared broadcast channel for live events.
pub type EventSender = Arc<broadcast::Sender<LiveEvent>>;

/// Create a new event broadcast channel (capacity 256).
pub fn event_channel() -> (EventSender, broadcast::Receiver<LiveEvent>) {
    let (tx, rx) = broadcast::channel(256);
    (Arc::new(tx), rx)
}

/// WebSocket upgrade handler.
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state.events))
}

async fn handle_socket(mut socket: WebSocket, tx: EventSender) {
    let mut rx = tx.subscribe();
    loop {
        tokio::select! {
            // Forward broadcast events to the client
            event = rx.recv() => {
                match event {
                    Ok(evt) => {
                        let json = serde_json::to_string(&evt).unwrap_or_default();
                        if socket.send(Message::Text(json.into())).await.is_err() {
                            break; // client disconnected
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            // Handle incoming messages (ping/pong, close)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    _ => {} // ignore other messages
                }
            }
        }
    }
}
