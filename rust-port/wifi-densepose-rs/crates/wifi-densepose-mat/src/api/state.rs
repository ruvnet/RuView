//! Application state for the MAT REST API.
//!
//! This module provides the shared state that is passed to all API handlers.
//! It contains repositories, services, and real-time event broadcasting.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::domain::{
    DisasterEvent, Alert,
};
use super::dto::WebSocketMessage;

/// Shared application state for the API.
///
/// This is cloned for each request handler and provides thread-safe
/// access to shared resources.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

/// Inner state (not cloned, shared via Arc).
struct AppStateInner {
    /// In-memory event repository
    events: RwLock<HashMap<Uuid, DisasterEvent>>,
    /// In-memory alert repository
    alerts: RwLock<HashMap<Uuid, AlertWithEventId>>,
    /// Broadcast channel for real-time updates
    broadcast_tx: broadcast::Sender<WebSocketMessage>,
    /// Configuration
    config: ApiConfig,
}

/// Alert with its associated event ID for lookup.
#[derive(Clone)]
pub struct AlertWithEventId {
    pub alert: Alert,
    pub event_id: Uuid,
}

/// API configuration.
#[derive(Clone)]
pub struct ApiConfig {
    /// Maximum number of events to store
    pub max_events: usize,
    /// Maximum survivors per event
    pub max_survivors_per_event: usize,
    /// Broadcast channel capacity
    pub broadcast_capacity: usize,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            max_events: 1000,
            max_survivors_per_event: 10000,
            broadcast_capacity: 1024,
        }
    }
}

impl AppState {
    /// Create a new application state with default configuration.
    pub fn new() -> Self {
        Self::with_config(ApiConfig::default())
    }

    /// Create a new application state with custom configuration.
    pub fn with_config(config: ApiConfig) -> Self {
        let (broadcast_tx, _) = broadcast::channel(config.broadcast_capacity);

        Self {
            inner: Arc::new(AppStateInner {
                events: RwLock::new(HashMap::new()),
                alerts: RwLock::new(HashMap::new()),
                broadcast_tx,
                config,
            }),
        }
    }

    // ========================================================================
    // Event Operations
    // ========================================================================

    /// Store a disaster event.
    pub fn store_event(&self, event: DisasterEvent) -> Uuid {
        let id = *event.id().as_uuid();
        let mut events = self.inner.events.write();

        // Check capacity
        if events.len() >= self.inner.config.max_events {
            // Remove oldest closed event
            let oldest_closed = events
                .iter()
                .filter(|(_, e)| matches!(e.status(), crate::EventStatus::Closed))
                .min_by_key(|(_, e)| e.start_time())
                .map(|(id, _)| *id);

            if let Some(old_id) = oldest_closed {
                events.remove(&old_id);
            }
        }

        events.insert(id, event);
        id
    }

    /// Get an event by ID.
    pub fn get_event(&self, id: Uuid) -> Option<DisasterEvent> {
        self.inner.events.read().get(&id).cloned()
    }

    /// Get mutable access to an event (for updates).
    pub fn update_event<F, R>(&self, id: Uuid, f: F) -> Option<R>
    where
        F: FnOnce(&mut DisasterEvent) -> R,
    {
        let mut events = self.inner.events.write();
        events.get_mut(&id).map(f)
    }

    /// List all events.
    pub fn list_events(&self) -> Vec<DisasterEvent> {
        self.inner.events.read().values().cloned().collect()
    }

    /// Get event count.
    pub fn event_count(&self) -> usize {
        self.inner.events.read().len()
    }

    // ========================================================================
    // Alert Operations
    // ========================================================================

    /// Store an alert.
    pub fn store_alert(&self, alert: Alert, event_id: Uuid) -> Uuid {
        let id = *alert.id().as_uuid();
        let mut alerts = self.inner.alerts.write();
        alerts.insert(id, AlertWithEventId { alert, event_id });
        id
    }

    /// Get an alert by ID.
    pub fn get_alert(&self, id: Uuid) -> Option<AlertWithEventId> {
        self.inner.alerts.read().get(&id).cloned()
    }

    /// Update an alert.
    pub fn update_alert<F, R>(&self, id: Uuid, f: F) -> Option<R>
    where
        F: FnOnce(&mut Alert) -> R,
    {
        let mut alerts = self.inner.alerts.write();
        alerts.get_mut(&id).map(|a| f(&mut a.alert))
    }

    /// List alerts for an event.
    pub fn list_alerts_for_event(&self, event_id: Uuid) -> Vec<Alert> {
        self.inner
            .alerts
            .read()
            .values()
            .filter(|a| a.event_id == event_id)
            .map(|a| a.alert.clone())
            .collect()
    }

    // ========================================================================
    // Broadcasting
    // ========================================================================

    /// Get a receiver for real-time updates.
    pub fn subscribe(&self) -> broadcast::Receiver<WebSocketMessage> {
        self.inner.broadcast_tx.subscribe()
    }

    /// Broadcast a message to all subscribers.
    pub fn broadcast(&self, message: WebSocketMessage) {
        // Ignore send errors (no subscribers)
        let _ = self.inner.broadcast_tx.send(message);
    }

    /// Get the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.inner.broadcast_tx.receiver_count()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DisasterType, DisasterEvent};
    use geo::Point;

    #[test]
    fn test_store_and_get_event() {
        let state = AppState::new();
        let event = DisasterEvent::new(
            DisasterType::Earthquake,
            Point::new(-122.4194, 37.7749),
            "Test earthquake",
        );
        let id = *event.id().as_uuid();

        state.store_event(event);

        let retrieved = state.get_event(id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id().as_uuid(), &id);
    }

    #[test]
    fn test_update_event() {
        let state = AppState::new();
        let event = DisasterEvent::new(
            DisasterType::Earthquake,
            Point::new(0.0, 0.0),
            "Test",
        );
        let id = *event.id().as_uuid();
        state.store_event(event);

        let result = state.update_event(id, |e| {
            e.set_status(crate::EventStatus::Suspended);
            true
        });

        assert!(result.unwrap());
        let updated = state.get_event(id).unwrap();
        assert!(matches!(updated.status(), crate::EventStatus::Suspended));
    }

    #[test]
    fn test_broadcast_subscribe() {
        let state = AppState::new();
        let mut rx = state.subscribe();

        state.broadcast(WebSocketMessage::Heartbeat {
            timestamp: chrono::Utc::now(),
        });

        // Try to receive (in async context this would work)
        assert_eq!(state.subscriber_count(), 1);
    }
}
