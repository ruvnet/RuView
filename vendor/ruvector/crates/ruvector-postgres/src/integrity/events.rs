//! Integrity Events Module
//!
//! Defines integrity event types that trigger contracted graph updates
//! and state transitions. Events support delta updates for efficiency.

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use super::gating::IntegrityState;
use super::mincut::WitnessEdge;

/// Types of integrity events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityEventType {
    /// A new partition was created
    PartitionCreated,
    /// A partition was deleted
    PartitionDeleted,
    /// A partition's health changed
    PartitionHealthChanged,
    /// An IVFFlat centroid was moved/updated
    CentroidMoved,
    /// Centroids were rebalanced
    CentroidRebalanced,
    /// A shard was rebalanced
    ShardRebalanced,
    /// A new shard was added
    ShardAdded,
    /// A shard was removed
    ShardRemoved,
    /// An external dependency became unavailable
    DependencyDown,
    /// An external dependency recovered
    DependencyUp,
    /// Integrity state changed
    StateChanged,
    /// Lambda cut was sampled
    LambdaSampled,
    /// Graph was rebuilt
    GraphRebuilt,
    /// Edge capacity changed significantly
    EdgeCapacityChanged,
    /// Error rate threshold exceeded
    ErrorRateExceeded,
    /// Manual intervention
    ManualOverride,
}

impl std::fmt::Display for IntegrityEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string(self).unwrap_or_else(|_| "unknown".to_string());
        // Remove quotes from JSON string
        write!(f, "{}", s.trim_matches('"'))
    }
}

impl IntegrityEventType {
    /// Whether this event requires graph update
    pub fn requires_graph_update(&self) -> bool {
        matches!(
            self,
            IntegrityEventType::PartitionCreated
                | IntegrityEventType::PartitionDeleted
                | IntegrityEventType::CentroidMoved
                | IntegrityEventType::CentroidRebalanced
                | IntegrityEventType::ShardRebalanced
                | IntegrityEventType::ShardAdded
                | IntegrityEventType::ShardRemoved
                | IntegrityEventType::DependencyDown
                | IntegrityEventType::DependencyUp
        )
    }

    /// Whether this event requires mincut recomputation
    pub fn requires_mincut_recomputation(&self) -> bool {
        matches!(
            self,
            IntegrityEventType::PartitionCreated
                | IntegrityEventType::PartitionDeleted
                | IntegrityEventType::PartitionHealthChanged
                | IntegrityEventType::ShardRebalanced
                | IntegrityEventType::ShardAdded
                | IntegrityEventType::ShardRemoved
                | IntegrityEventType::DependencyDown
                | IntegrityEventType::DependencyUp
                | IntegrityEventType::EdgeCapacityChanged
                | IntegrityEventType::GraphRebuilt
        )
    }

    /// Event severity level (0 = info, 1 = warning, 2 = critical)
    pub fn severity(&self) -> u8 {
        match self {
            IntegrityEventType::LambdaSampled => 0,
            IntegrityEventType::GraphRebuilt => 0,
            IntegrityEventType::PartitionCreated => 0,
            IntegrityEventType::CentroidMoved => 0,
            IntegrityEventType::CentroidRebalanced => 1,
            IntegrityEventType::PartitionDeleted => 1,
            IntegrityEventType::PartitionHealthChanged => 1,
            IntegrityEventType::ShardRebalanced => 1,
            IntegrityEventType::ShardAdded => 1,
            IntegrityEventType::EdgeCapacityChanged => 1,
            IntegrityEventType::StateChanged => 2,
            IntegrityEventType::ShardRemoved => 2,
            IntegrityEventType::DependencyDown => 2,
            IntegrityEventType::DependencyUp => 1,
            IntegrityEventType::ErrorRateExceeded => 2,
            IntegrityEventType::ManualOverride => 2,
        }
    }
}

/// Content of an integrity event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityEventContent {
    /// Event ID (unique within collection)
    pub event_id: u64,
    /// Collection this event belongs to
    pub collection_id: i32,
    /// Type of event
    pub event_type: IntegrityEventType,
    /// Previous state (for state changes)
    pub previous_state: Option<IntegrityState>,
    /// New state (for state changes)
    pub new_state: Option<IntegrityState>,
    /// Lambda cut value at event time
    pub lambda_cut: Option<f32>,
    /// Witness edges (for mincut events)
    pub witness_edges: Option<Vec<WitnessEdge>>,
    /// Additional metadata
    pub metadata: serde_json::Value,
    /// Event timestamp
    pub created_at: SystemTime,
    /// Source of the event
    pub source: String,
}

impl IntegrityEventContent {
    /// Create a new event
    pub fn new(
        collection_id: i32,
        event_type: IntegrityEventType,
        source: impl Into<String>,
    ) -> Self {
        Self {
            event_id: 0, // Assigned by event store
            collection_id,
            event_type,
            previous_state: None,
            new_state: None,
            lambda_cut: None,
            witness_edges: None,
            metadata: serde_json::json!({}),
            created_at: SystemTime::now(),
            source: source.into(),
        }
    }

    /// Create a state change event
    pub fn state_change(
        collection_id: i32,
        previous: IntegrityState,
        new: IntegrityState,
        lambda_cut: f32,
        witness_edges: Vec<WitnessEdge>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            event_id: 0,
            collection_id,
            event_type: IntegrityEventType::StateChanged,
            previous_state: Some(previous),
            new_state: Some(new),
            lambda_cut: Some(lambda_cut),
            witness_edges: Some(witness_edges),
            metadata: serde_json::json!({
                "direction": if new > previous { "degrading" } else { "improving" }
            }),
            created_at: SystemTime::now(),
            source: source.into(),
        }
    }

    /// Create a lambda sampled event
    pub fn lambda_sampled(
        collection_id: i32,
        lambda_cut: f32,
        state: IntegrityState,
        source: impl Into<String>,
    ) -> Self {
        Self {
            event_id: 0,
            collection_id,
            event_type: IntegrityEventType::LambdaSampled,
            previous_state: None,
            new_state: Some(state),
            lambda_cut: Some(lambda_cut),
            witness_edges: None,
            metadata: serde_json::json!({}),
            created_at: SystemTime::now(),
            source: source.into(),
        }
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Add metadata field
    pub fn with_metadata_field(mut self, key: &str, value: serde_json::Value) -> Self {
        if let serde_json::Value::Object(ref mut map) = self.metadata {
            map.insert(key.to_string(), value);
        }
        self
    }
}

/// Delta update for contracted graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDelta {
    /// Collection ID
    pub collection_id: i32,
    /// Nodes to add
    pub add_nodes: Vec<DeltaNode>,
    /// Nodes to remove (by type and id)
    pub remove_nodes: Vec<(String, i64)>,
    /// Nodes to update
    pub update_nodes: Vec<DeltaNode>,
    /// Edges to add
    pub add_edges: Vec<DeltaEdge>,
    /// Edges to remove (by source and target)
    pub remove_edges: Vec<((String, i64), (String, i64))>,
    /// Edges to update
    pub update_edges: Vec<DeltaEdge>,
}

/// Node delta for graph updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaNode {
    pub node_type: String,
    pub node_id: i64,
    pub node_name: Option<String>,
    pub health_score: Option<f32>,
    pub metadata: Option<serde_json::Value>,
}

/// Edge delta for graph updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaEdge {
    pub source_type: String,
    pub source_id: i64,
    pub target_type: String,
    pub target_id: i64,
    pub edge_type: String,
    pub capacity: Option<f32>,
    pub current_flow: Option<f32>,
    pub error_rate: Option<f32>,
}

impl GraphDelta {
    /// Create an empty delta
    pub fn new(collection_id: i32) -> Self {
        Self {
            collection_id,
            add_nodes: Vec::new(),
            remove_nodes: Vec::new(),
            update_nodes: Vec::new(),
            add_edges: Vec::new(),
            remove_edges: Vec::new(),
            update_edges: Vec::new(),
        }
    }

    /// Check if delta is empty
    pub fn is_empty(&self) -> bool {
        self.add_nodes.is_empty()
            && self.remove_nodes.is_empty()
            && self.update_nodes.is_empty()
            && self.add_edges.is_empty()
            && self.remove_edges.is_empty()
            && self.update_edges.is_empty()
    }

    /// Count total changes
    pub fn change_count(&self) -> usize {
        self.add_nodes.len()
            + self.remove_nodes.len()
            + self.update_nodes.len()
            + self.add_edges.len()
            + self.remove_edges.len()
            + self.update_edges.len()
    }
}

/// Event store for persisting integrity events
pub struct IntegrityEventStore {
    /// Collection ID
    collection_id: i32,
    /// Maximum events to keep in memory
    max_events: usize,
    /// Event counter for IDs
    next_event_id: std::sync::atomic::AtomicU64,
    /// In-memory event buffer
    events: RwLock<VecDeque<IntegrityEventContent>>,
    /// Event listeners
    listeners: RwLock<Vec<Box<dyn Fn(&IntegrityEventContent) + Send + Sync>>>,
}

impl IntegrityEventStore {
    /// Create a new event store
    pub fn new(collection_id: i32, max_events: usize) -> Self {
        Self {
            collection_id,
            max_events,
            next_event_id: std::sync::atomic::AtomicU64::new(1),
            events: RwLock::new(VecDeque::with_capacity(max_events)),
            listeners: RwLock::new(Vec::new()),
        }
    }

    /// Record an event
    pub fn record(&self, mut event: IntegrityEventContent) -> u64 {
        // Assign event ID
        let event_id = self
            .next_event_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        event.event_id = event_id;

        // Add to buffer
        {
            let mut events = self.events.write().unwrap();
            if events.len() >= self.max_events {
                events.pop_front();
            }
            events.push_back(event.clone());
        }

        // Notify listeners
        {
            let listeners = self.listeners.read().unwrap();
            for listener in listeners.iter() {
                listener(&event);
            }
        }

        event_id
    }

    /// Get recent events
    pub fn get_recent(&self, count: usize) -> Vec<IntegrityEventContent> {
        let events = self.events.read().unwrap();
        events.iter().rev().take(count).cloned().collect()
    }

    /// Get events by type
    pub fn get_by_type(
        &self,
        event_type: IntegrityEventType,
        count: usize,
    ) -> Vec<IntegrityEventContent> {
        let events = self.events.read().unwrap();
        events
            .iter()
            .rev()
            .filter(|e| e.event_type == event_type)
            .take(count)
            .cloned()
            .collect()
    }

    /// Get events since a timestamp
    pub fn get_since(&self, since: SystemTime) -> Vec<IntegrityEventContent> {
        let events = self.events.read().unwrap();
        events
            .iter()
            .filter(|e| e.created_at >= since)
            .cloned()
            .collect()
    }

    /// Get state change events
    pub fn get_state_changes(&self, count: usize) -> Vec<IntegrityEventContent> {
        self.get_by_type(IntegrityEventType::StateChanged, count)
    }

    /// Add an event listener
    pub fn add_listener<F>(&self, listener: F)
    where
        F: Fn(&IntegrityEventContent) + Send + Sync + 'static,
    {
        let mut listeners = self.listeners.write().unwrap();
        listeners.push(Box::new(listener));
    }

    /// Get event count
    pub fn event_count(&self) -> usize {
        self.events.read().unwrap().len()
    }

    /// Clear all events
    pub fn clear(&self) {
        self.events.write().unwrap().clear();
    }

    /// Get statistics
    pub fn stats(&self) -> EventStoreStats {
        let events = self.events.read().unwrap();
        let mut by_type: std::collections::HashMap<IntegrityEventType, usize> =
            std::collections::HashMap::new();
        let mut by_severity = [0usize; 3];

        for event in events.iter() {
            *by_type.entry(event.event_type).or_insert(0) += 1;
            let severity = event.event_type.severity() as usize;
            if severity < 3 {
                by_severity[severity] += 1;
            }
        }

        EventStoreStats {
            total_events: events.len(),
            by_type,
            info_count: by_severity[0],
            warning_count: by_severity[1],
            critical_count: by_severity[2],
        }
    }
}

/// Statistics about the event store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStoreStats {
    pub total_events: usize,
    pub by_type: std::collections::HashMap<IntegrityEventType, usize>,
    pub info_count: usize,
    pub warning_count: usize,
    pub critical_count: usize,
}

/// Global registry for event stores
static EVENT_REGISTRY: once_cell::sync::Lazy<DashMap<i32, Arc<IntegrityEventStore>>> =
    once_cell::sync::Lazy::new(DashMap::new);

/// Get or create an event store for a collection
pub fn get_or_create_event_store(collection_id: i32) -> Arc<IntegrityEventStore> {
    EVENT_REGISTRY
        .entry(collection_id)
        .or_insert_with(|| Arc::new(IntegrityEventStore::new(collection_id, 10000)))
        .clone()
}

/// Get an existing event store
pub fn get_event_store(collection_id: i32) -> Option<Arc<IntegrityEventStore>> {
    EVENT_REGISTRY.get(&collection_id).map(|e| e.clone())
}

/// Record an integrity event
pub fn record_event(event: IntegrityEventContent) -> u64 {
    let store = get_or_create_event_store(event.collection_id);
    store.record(event)
}

/// Create a graph delta from an event
pub fn event_to_delta(event: &IntegrityEventContent) -> Option<GraphDelta> {
    if !event.event_type.requires_graph_update() {
        return None;
    }

    let mut delta = GraphDelta::new(event.collection_id);

    match event.event_type {
        IntegrityEventType::PartitionCreated => {
            if let Some(partition_id) = event.metadata.get("partition_id").and_then(|v| v.as_i64())
            {
                delta.add_nodes.push(DeltaNode {
                    node_type: "partition".to_string(),
                    node_id: partition_id,
                    node_name: Some(format!("partition_{}", partition_id)),
                    health_score: Some(1.0),
                    metadata: None,
                });
            }
        }
        IntegrityEventType::PartitionDeleted => {
            if let Some(partition_id) = event.metadata.get("partition_id").and_then(|v| v.as_i64())
            {
                delta
                    .remove_nodes
                    .push(("partition".to_string(), partition_id));
            }
        }
        IntegrityEventType::DependencyDown => {
            if let Some(dep_id) = event.metadata.get("dependency_id").and_then(|v| v.as_i64()) {
                delta.update_nodes.push(DeltaNode {
                    node_type: "external_dependency".to_string(),
                    node_id: dep_id,
                    node_name: None,
                    health_score: Some(0.0),
                    metadata: None,
                });
            }
        }
        IntegrityEventType::DependencyUp => {
            if let Some(dep_id) = event.metadata.get("dependency_id").and_then(|v| v.as_i64()) {
                delta.update_nodes.push(DeltaNode {
                    node_type: "external_dependency".to_string(),
                    node_id: dep_id,
                    node_name: None,
                    health_score: Some(1.0),
                    metadata: None,
                });
            }
        }
        _ => {
            // Other events handled elsewhere or require full graph info
        }
    }

    if delta.is_empty() {
        None
    } else {
        Some(delta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_display() {
        assert_eq!(
            IntegrityEventType::StateChanged.to_string(),
            "state_changed"
        );
        assert_eq!(
            IntegrityEventType::LambdaSampled.to_string(),
            "lambda_sampled"
        );
    }

    #[test]
    fn test_event_type_properties() {
        assert!(IntegrityEventType::PartitionCreated.requires_graph_update());
        assert!(!IntegrityEventType::LambdaSampled.requires_graph_update());

        assert!(IntegrityEventType::GraphRebuilt.requires_mincut_recomputation());
        assert!(!IntegrityEventType::ManualOverride.requires_mincut_recomputation());
    }

    #[test]
    fn test_event_creation() {
        let event = IntegrityEventContent::new(1, IntegrityEventType::GraphRebuilt, "test");
        assert_eq!(event.collection_id, 1);
        assert_eq!(event.event_type, IntegrityEventType::GraphRebuilt);
        assert_eq!(event.source, "test");
    }

    #[test]
    fn test_state_change_event() {
        let event = IntegrityEventContent::state_change(
            1,
            IntegrityState::Normal,
            IntegrityState::Stress,
            0.65,
            vec![],
            "integrity_worker",
        );

        assert_eq!(event.event_type, IntegrityEventType::StateChanged);
        assert_eq!(event.previous_state, Some(IntegrityState::Normal));
        assert_eq!(event.new_state, Some(IntegrityState::Stress));
        assert_eq!(event.lambda_cut, Some(0.65));
    }

    #[test]
    fn test_event_store() {
        let store = IntegrityEventStore::new(1, 100);

        // Record events
        let id1 = store.record(IntegrityEventContent::new(
            1,
            IntegrityEventType::GraphRebuilt,
            "test",
        ));
        let id2 = store.record(IntegrityEventContent::new(
            1,
            IntegrityEventType::LambdaSampled,
            "test",
        ));

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(store.event_count(), 2);

        // Get recent
        let recent = store.get_recent(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].event_id, 2); // Most recent first
    }

    #[test]
    fn test_event_store_overflow() {
        let store = IntegrityEventStore::new(1, 5);

        // Record more than max
        for i in 0..10 {
            store.record(IntegrityEventContent::new(
                1,
                IntegrityEventType::LambdaSampled,
                format!("test_{}", i),
            ));
        }

        assert_eq!(store.event_count(), 5);

        // Oldest events should be removed
        let events = store.get_recent(10);
        assert_eq!(events.len(), 5);
        assert!(events[0].source.contains("test_9")); // Most recent
    }

    #[test]
    fn test_get_by_type() {
        let store = IntegrityEventStore::new(1, 100);

        store.record(IntegrityEventContent::new(
            1,
            IntegrityEventType::GraphRebuilt,
            "test",
        ));
        store.record(IntegrityEventContent::new(
            1,
            IntegrityEventType::LambdaSampled,
            "test",
        ));
        store.record(IntegrityEventContent::new(
            1,
            IntegrityEventType::LambdaSampled,
            "test",
        ));

        let sampled = store.get_by_type(IntegrityEventType::LambdaSampled, 10);
        assert_eq!(sampled.len(), 2);
    }

    #[test]
    fn test_graph_delta() {
        let mut delta = GraphDelta::new(1);
        assert!(delta.is_empty());

        delta.add_nodes.push(DeltaNode {
            node_type: "partition".to_string(),
            node_id: 1,
            node_name: None,
            health_score: Some(1.0),
            metadata: None,
        });

        assert!(!delta.is_empty());
        assert_eq!(delta.change_count(), 1);
    }

    #[test]
    fn test_event_to_delta() {
        let event = IntegrityEventContent::new(1, IntegrityEventType::PartitionCreated, "test")
            .with_metadata_field("partition_id", serde_json::json!(42));

        let delta = event_to_delta(&event);
        assert!(delta.is_some());

        let delta = delta.unwrap();
        assert_eq!(delta.add_nodes.len(), 1);
        assert_eq!(delta.add_nodes[0].node_id, 42);
    }

    #[test]
    fn test_event_store_stats() {
        let store = IntegrityEventStore::new(1, 100);

        store.record(IntegrityEventContent::new(
            1,
            IntegrityEventType::LambdaSampled,
            "test",
        ));
        store.record(IntegrityEventContent::new(
            1,
            IntegrityEventType::StateChanged,
            "test",
        ));
        store.record(IntegrityEventContent::new(
            1,
            IntegrityEventType::DependencyDown,
            "test",
        ));

        let stats = store.stats();
        assert_eq!(stats.total_events, 3);
        assert_eq!(stats.info_count, 1);
        assert_eq!(stats.critical_count, 2);
    }

    #[test]
    fn test_global_event_registry() {
        let store = get_or_create_event_store(12345);
        let event_id = record_event(IntegrityEventContent::new(
            12345,
            IntegrityEventType::GraphRebuilt,
            "test",
        ));

        assert!(event_id > 0);

        let retrieved = get_event_store(12345);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().event_count(), 1);
    }
}
