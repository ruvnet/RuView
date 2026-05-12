//! Event stream for home automation integration.
//!
//! Tracks state changes in the sensing pipeline and emits structured events
//! via an async broadcast channel. Used by the MQTT bridge and any future
//! webhook / SSE integrations.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

/// Event types emitted by the sensing pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EventData {
    PresenceChanged { body_count: usize, previous: usize },
    PostureChanged { body_id: usize, from: String, to: String },
    ActivityChanged { from: String, to: String },
    VitalsAlert { metric: String, value: f64, threshold: f64 },
    LyingStillAlert { body_id: usize, duration_minutes: f64 },
    NodeOnline { node_id: u8 },
    NodeOffline { node_id: u8 },
}

/// A timestamped event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: u64,
    pub timestamp: f64,
    #[serde(flatten)]
    pub data: EventData,
}

/// Tracks previous state to detect changes.
pub struct StateTracker {
    prev_body_count: usize,
    prev_postures: HashMap<usize, String>,
    prev_activity: String,
    lying_since: HashMap<usize, Instant>,
    lying_alerts_fired: HashMap<usize, Vec<u64>>,
    prev_online_nodes: std::collections::HashSet<u8>,
    prev_hr: f64,
    prev_br: f64,
}

impl StateTracker {
    pub fn new() -> Self {
        Self {
            prev_body_count: 0,
            prev_postures: HashMap::new(),
            prev_activity: "absent".to_string(),
            lying_since: HashMap::new(),
            lying_alerts_fired: HashMap::new(),
            prev_online_nodes: std::collections::HashSet::new(),
            prev_hr: 0.0,
            prev_br: 0.0,
        }
    }

    /// Compare current state to previous, emit events for any changes.
    pub fn diff(
        &mut self,
        body_count: usize,
        clusters: &[(usize, String)],
        activity: &str,
        online_nodes: &std::collections::HashSet<u8>,
        hr_bpm: f64,
        br_bpm: f64,
    ) -> Vec<EventData> {
        let mut events = Vec::new();

        // Presence changed
        if body_count != self.prev_body_count {
            events.push(EventData::PresenceChanged {
                body_count,
                previous: self.prev_body_count,
            });
            self.prev_body_count = body_count;
        }

        // Posture changed per body
        let mut current_postures = HashMap::new();
        for (id, pose) in clusters {
            current_postures.insert(*id, pose.clone());
            if let Some(prev) = self.prev_postures.get(id) {
                if prev != pose {
                    events.push(EventData::PostureChanged {
                        body_id: *id,
                        from: prev.clone(),
                        to: pose.clone(),
                    });
                }
            }
            // Track lying duration
            if pose == "lying" {
                self.lying_since.entry(*id).or_insert_with(Instant::now);
                if let Some(since) = self.lying_since.get(id) {
                    let minutes = since.elapsed().as_secs_f64() / 60.0;
                    let thresholds = [5u64, 15, 30, 60];
                    let fired = self.lying_alerts_fired.entry(*id).or_default();
                    for &t in &thresholds {
                        if minutes >= t as f64 && !fired.contains(&t) {
                            events.push(EventData::LyingStillAlert {
                                body_id: *id,
                                duration_minutes: minutes,
                            });
                            fired.push(t);
                        }
                    }
                }
            } else {
                self.lying_since.remove(id);
                self.lying_alerts_fired.remove(id);
            }
        }
        self.prev_postures = current_postures;

        // Activity changed
        if activity != self.prev_activity {
            events.push(EventData::ActivityChanged {
                from: self.prev_activity.clone(),
                to: activity.to_string(),
            });
            self.prev_activity = activity.to_string();
        }

        // Node online/offline
        for &nid in online_nodes {
            if !self.prev_online_nodes.contains(&nid) {
                events.push(EventData::NodeOnline { node_id: nid });
            }
        }
        for &nid in &self.prev_online_nodes {
            if !online_nodes.contains(&nid) {
                events.push(EventData::NodeOffline { node_id: nid });
            }
        }
        self.prev_online_nodes = online_nodes.clone();

        // Vitals alerts (HR outside 40-120, BR outside 8-25 when previously normal)
        if hr_bpm > 1.0 {
            if hr_bpm > 120.0 && self.prev_hr <= 120.0 {
                events.push(EventData::VitalsAlert {
                    metric: "heart_rate_high".to_string(),
                    value: hr_bpm,
                    threshold: 120.0,
                });
            }
            if hr_bpm < 40.0 && self.prev_hr >= 40.0 {
                events.push(EventData::VitalsAlert {
                    metric: "heart_rate_low".to_string(),
                    value: hr_bpm,
                    threshold: 40.0,
                });
            }
            self.prev_hr = hr_bpm;
        }
        if br_bpm > 1.0 {
            if br_bpm > 25.0 && self.prev_br <= 25.0 {
                events.push(EventData::VitalsAlert {
                    metric: "breathing_rate_high".to_string(),
                    value: br_bpm,
                    threshold: 25.0,
                });
            }
            if br_bpm < 8.0 && self.prev_br >= 8.0 {
                events.push(EventData::VitalsAlert {
                    metric: "breathing_rate_low".to_string(),
                    value: br_bpm,
                    threshold: 8.0,
                });
            }
            self.prev_br = br_bpm;
        }

        events
    }
}

/// Central event bus for the sensing server.
pub struct EventBus {
    tx: broadcast::Sender<Event>,
    recent: Arc<RwLock<VecDeque<Event>>>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
}

const RECENT_CAPACITY: usize = 100;
const BUS_CAPACITY: usize = 1000;

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BUS_CAPACITY);
        Self {
            tx,
            recent: Arc::new(RwLock::new(VecDeque::with_capacity(RECENT_CAPACITY))),
            next_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    /// Publish an event. Called from the sensing pipeline.
    pub async fn publish(&self, data: EventData) {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let event = Event {
            id,
            timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
            data,
        };
        let _ = self.tx.send(event.clone());
        let mut recent = self.recent.write().await;
        recent.push_back(event);
        if recent.len() > RECENT_CAPACITY {
            recent.pop_front();
        }
    }

    /// Subscribe to the event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    /// Get recent events (for polling clients).
    pub async fn recent_events(&self) -> Vec<Event> {
        self.recent.read().await.iter().cloned().collect()
    }

    /// Get recent events since a given ID.
    pub async fn events_since(&self, since_id: u64) -> Vec<Event> {
        self.recent.read().await.iter().filter(|e| e.id > since_id).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.publish(EventData::PresenceChanged { body_count: 2, previous: 0 }).await;
        bus.publish(EventData::NodeOnline { node_id: 1 }).await;
        bus.publish(EventData::ActivityChanged { from: "absent".into(), to: "active".into() }).await;

        let e1 = rx.recv().await.unwrap();
        assert_eq!(e1.id, 1);
        let e2 = rx.recv().await.unwrap();
        assert_eq!(e2.id, 2);
        let e3 = rx.recv().await.unwrap();
        assert_eq!(e3.id, 3);
    }

    #[tokio::test]
    async fn test_event_bus_recent() {
        let bus = EventBus::new();
        bus.publish(EventData::NodeOnline { node_id: 5 }).await;
        bus.publish(EventData::NodeOffline { node_id: 3 }).await;

        let recent = bus.recent_events().await;
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].id, 1);
        assert_eq!(recent[1].id, 2);
    }

    #[test]
    fn test_state_tracker_no_change() {
        let mut tracker = StateTracker::new();
        let nodes = std::collections::HashSet::new();
        let events = tracker.diff(0, &[], "absent", &nodes, 0.0, 0.0);
        assert!(events.is_empty(), "no state change should produce no events");
    }

    #[test]
    fn test_state_tracker_presence_change() {
        let mut tracker = StateTracker::new();
        let nodes = std::collections::HashSet::new();
        let events = tracker.diff(3, &[], "absent", &nodes, 0.0, 0.0);
        assert_eq!(events.len(), 1);
        match &events[0] {
            EventData::PresenceChanged { body_count, previous } => {
                assert_eq!(*body_count, 3);
                assert_eq!(*previous, 0);
            }
            _ => panic!("expected PresenceChanged"),
        }
    }
}
