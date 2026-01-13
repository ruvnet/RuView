//! WebSocket handler for real-time survivor and alert streaming.
//!
//! This module provides a WebSocket endpoint that streams real-time updates
//! for survivor detections, status changes, and alerts.
//!
//! ## Protocol
//!
//! Clients connect to `/ws/mat/stream` and receive JSON-formatted messages.
//!
//! ### Message Types
//!
//! - `survivor_detected` - New survivor found
//! - `survivor_updated` - Survivor status/vitals changed
//! - `survivor_lost` - Survivor signal lost
//! - `alert_created` - New alert generated
//! - `alert_updated` - Alert status changed
//! - `zone_scan_complete` - Zone scan finished
//! - `event_status_changed` - Event status changed
//! - `heartbeat` - Keep-alive ping
//! - `error` - Error message
//!
//! ### Client Commands
//!
//! Clients can send JSON commands:
//! - `{"action": "subscribe", "event_id": "..."}`
//! - `{"action": "unsubscribe", "event_id": "..."}`
//! - `{"action": "subscribe_all"}`
//! - `{"action": "get_state", "event_id": "..."}`

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::dto::{WebSocketMessage, WebSocketRequest};
use super::state::AppState;

/// WebSocket connection handler.
///
/// # OpenAPI Specification
///
/// ```yaml
/// /ws/mat/stream:
///   get:
///     summary: Real-time event stream
///     description: |
///       WebSocket endpoint for real-time updates on survivors and alerts.
///
///       ## Connection
///
///       Connect using a WebSocket client to receive real-time updates.
///
///       ## Messages
///
///       All messages are JSON-formatted with a "type" field indicating
///       the message type.
///
///       ## Subscriptions
///
///       By default, clients receive updates for all events. Send a
///       subscribe/unsubscribe command to filter to specific events.
///     tags: [WebSocket]
///     responses:
///       101:
///         description: WebSocket connection established
/// ```
#[tracing::instrument(skip(state, ws))]
pub async fn ws_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle an established WebSocket connection.
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Subscription state for this connection
    let subscriptions: Arc<Mutex<SubscriptionState>> = Arc::new(Mutex::new(SubscriptionState::new()));

    // Subscribe to broadcast channel
    let mut broadcast_rx = state.subscribe();

    // Spawn task to forward broadcast messages to client
    let subs_clone = subscriptions.clone();
    let forward_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Receive from broadcast channel
                result = broadcast_rx.recv() => {
                    match result {
                        Ok(msg) => {
                            // Check if this message matches subscription filter
                            if subs_clone.lock().should_receive(&msg) {
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    if sender.send(Message::Text(json)).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(lagged = n, "WebSocket client lagged, messages dropped");
                            // Send error notification
                            let error = WebSocketMessage::Error {
                                code: "MESSAGES_DROPPED".to_string(),
                                message: format!("{} messages were dropped due to slow client", n),
                            };
                            if let Ok(json) = serde_json::to_string(&error) {
                                if sender.send(Message::Text(json)).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                // Periodic heartbeat
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    let heartbeat = WebSocketMessage::Heartbeat {
                        timestamp: chrono::Utc::now(),
                    };
                    if let Ok(json) = serde_json::to_string(&heartbeat) {
                        if sender.send(Message::Ping(json.into_bytes())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    // Handle incoming messages from client
    let subs_clone = subscriptions.clone();
    let state_clone = state.clone();
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                // Parse and handle client command
                if let Err(e) = handle_client_message(&text, &subs_clone, &state_clone).await {
                    tracing::warn!(error = %e, "Failed to handle WebSocket message");
                }
            }
            Message::Binary(_) => {
                // Binary messages not supported
                tracing::debug!("Ignoring binary WebSocket message");
            }
            Message::Ping(data) => {
                // Pong handled automatically by axum
                tracing::trace!(len = data.len(), "Received ping");
            }
            Message::Pong(_) => {
                // Heartbeat response
                tracing::trace!("Received pong");
            }
            Message::Close(_) => {
                tracing::debug!("Client closed WebSocket connection");
                break;
            }
        }
    }

    // Clean up
    forward_task.abort();
    tracing::debug!("WebSocket connection closed");
}

/// Handle a client message (subscription commands).
async fn handle_client_message(
    text: &str,
    subscriptions: &Arc<Mutex<SubscriptionState>>,
    state: &AppState,
) -> Result<(), Box<dyn std::error::Error>> {
    let request: WebSocketRequest = serde_json::from_str(text)?;

    match request {
        WebSocketRequest::Subscribe { event_id } => {
            // Verify event exists
            if state.get_event(event_id).is_some() {
                subscriptions.lock().subscribe(event_id);
                tracing::debug!(event_id = %event_id, "Client subscribed to event");
            }
        }
        WebSocketRequest::Unsubscribe { event_id } => {
            subscriptions.lock().unsubscribe(&event_id);
            tracing::debug!(event_id = %event_id, "Client unsubscribed from event");
        }
        WebSocketRequest::SubscribeAll => {
            subscriptions.lock().subscribe_all();
            tracing::debug!("Client subscribed to all events");
        }
        WebSocketRequest::GetState { event_id } => {
            // This would send current state - simplified for now
            tracing::debug!(event_id = %event_id, "Client requested state");
        }
    }

    Ok(())
}

/// Tracks subscription state for a WebSocket connection.
struct SubscriptionState {
    /// Subscribed event IDs (empty = all events)
    event_ids: HashSet<Uuid>,
    /// Whether subscribed to all events
    all_events: bool,
}

impl SubscriptionState {
    fn new() -> Self {
        Self {
            event_ids: HashSet::new(),
            all_events: true, // Default to receiving all events
        }
    }

    fn subscribe(&mut self, event_id: Uuid) {
        self.all_events = false;
        self.event_ids.insert(event_id);
    }

    fn unsubscribe(&mut self, event_id: &Uuid) {
        self.event_ids.remove(event_id);
        if self.event_ids.is_empty() {
            self.all_events = true;
        }
    }

    fn subscribe_all(&mut self) {
        self.all_events = true;
        self.event_ids.clear();
    }

    fn should_receive(&self, msg: &WebSocketMessage) -> bool {
        if self.all_events {
            return true;
        }

        // Extract event_id from message and check subscription
        let event_id = match msg {
            WebSocketMessage::SurvivorDetected { event_id, .. } => Some(*event_id),
            WebSocketMessage::SurvivorUpdated { event_id, .. } => Some(*event_id),
            WebSocketMessage::SurvivorLost { event_id, .. } => Some(*event_id),
            WebSocketMessage::AlertCreated { event_id, .. } => Some(*event_id),
            WebSocketMessage::AlertUpdated { event_id, .. } => Some(*event_id),
            WebSocketMessage::ZoneScanComplete { event_id, .. } => Some(*event_id),
            WebSocketMessage::EventStatusChanged { event_id, .. } => Some(*event_id),
            WebSocketMessage::Heartbeat { .. } => None, // Always receive
            WebSocketMessage::Error { .. } => None, // Always receive
        };

        match event_id {
            Some(id) => self.event_ids.contains(&id),
            None => true, // Non-event-specific messages always sent
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_state() {
        let mut state = SubscriptionState::new();

        // Default is all events
        assert!(state.all_events);

        // Subscribe to specific event
        let event_id = Uuid::new_v4();
        state.subscribe(event_id);
        assert!(!state.all_events);
        assert!(state.event_ids.contains(&event_id));

        // Unsubscribe returns to all events
        state.unsubscribe(&event_id);
        assert!(state.all_events);
    }

    #[test]
    fn test_should_receive() {
        let mut state = SubscriptionState::new();
        let event_id = Uuid::new_v4();
        let other_id = Uuid::new_v4();

        // All events mode - receive everything
        let msg = WebSocketMessage::Heartbeat {
            timestamp: chrono::Utc::now(),
        };
        assert!(state.should_receive(&msg));

        // Subscribe to specific event
        state.subscribe(event_id);

        // Should receive messages for subscribed event
        let msg = WebSocketMessage::SurvivorLost {
            event_id,
            survivor_id: Uuid::new_v4(),
        };
        assert!(state.should_receive(&msg));

        // Should not receive messages for other events
        let msg = WebSocketMessage::SurvivorLost {
            event_id: other_id,
            survivor_id: Uuid::new_v4(),
        };
        assert!(!state.should_receive(&msg));

        // Heartbeats always received
        let msg = WebSocketMessage::Heartbeat {
            timestamp: chrono::Utc::now(),
        };
        assert!(state.should_receive(&msg));
    }
}
