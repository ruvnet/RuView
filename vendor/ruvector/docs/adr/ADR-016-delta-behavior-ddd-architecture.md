# ADR-016: Delta-Behavior System - Domain-Driven Design Architecture

**Status**: Proposed
**Date**: 2026-01-28
**Parent**: ADR-001 RuVector Core Architecture
**Author**: System Architecture Designer

## Abstract

This ADR defines a comprehensive Domain-Driven Design (DDD) architecture for a "Delta-Behavior" system using RuVector WASM modules. The system captures, propagates, aggregates, and applies differential changes (deltas) to vector representations, enabling efficient incremental updates, temporal versioning, and distributed state synchronization.

---

## 1. Executive Summary

The Delta-Behavior system models state changes as first-class domain objects rather than simple mutations. By treating deltas as immutable, causally-ordered events, the system enables:

- **Efficient incremental updates**: Only transmit/store changes, not full states
- **Temporal queries**: Reconstruct any historical state via delta replay
- **Conflict detection**: Identify and resolve concurrent modifications
- **Distributed sync**: Propagate deltas across nodes with eventual consistency
- **WASM portability**: Core logic runs in browser, edge, and server environments

---

## 2. Domain Analysis

### 2.1 Strategic Domain Design

The Delta-Behavior system spans five bounded contexts, each representing a distinct subdomain:

```
+------------------------------------------------------------------+
|                     DELTA-BEHAVIOR SYSTEM                         |
+------------------------------------------------------------------+
|                                                                    |
|  +----------------+    +-------------------+    +----------------+ |
|  | Delta Capture  |    | Delta Propagation |    | Delta         | |
|  | Domain         |--->| Domain            |--->| Aggregation   | |
|  |                |    |                   |    | Domain        | |
|  | - Observers    |    | - Routers         |    |               | |
|  | - Detectors    |    | - Channels        |    | - Windows     | |
|  | - Extractors   |    | - Subscribers     |    | - Batchers    | |
|  +----------------+    +-------------------+    +-------+--------+ |
|                                                         |          |
|                                                         v          |
|  +----------------+    +-------------------+    +----------------+ |
|  | Delta          |<---| Delta Application |<---| (Aggregated   | |
|  | Versioning     |    | Domain            |    |  Deltas)      | |
|  | Domain         |    |                   |    +----------------+ |
|  |                |    | - Applicators     |                       |
|  | - History      |    | - Validators      |                       |
|  | - Snapshots    |    | - Transformers    |                       |
|  | - Branches     |    |                   |                       |
|  +----------------+    +-------------------+                       |
|                                                                    |
+------------------------------------------------------------------+
```

### 2.2 Core Domain Concepts

| Domain Concept | Definition |
|----------------|------------|
| **Delta** | An immutable record of a differential change between two states |
| **DeltaStream** | Ordered sequence of deltas forming a causal chain |
| **DeltaGraph** | DAG structure representing delta dependencies and branches |
| **DeltaWindow** | Temporal container for batching deltas within a time/count boundary |
| **DeltaVector** | Sparse representation of the actual change data (vector diff) |
| **DeltaCheckpoint** | Full state snapshot at a specific delta sequence point |

---

## 3. Bounded Context Definitions

### 3.1 Delta Capture Domain

**Purpose**: Detect state changes and extract delta representations from source systems.

#### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Observer** | Component that monitors a source for state changes |
| **ChangeEvent** | Raw notification that a state modification occurred |
| **Detector** | Algorithm that identifies meaningful changes vs noise |
| **Extractor** | Component that computes the delta between old and new state |
| **CapturePolicy** | Rules governing when/how deltas are captured |
| **SourceBinding** | Connection between observer and monitored resource |

#### Aggregate Roots

```rust
/// Delta Capture Domain - Aggregate Roots and Entities
pub mod delta_capture {
    use std::collections::HashMap;
    use serde::{Deserialize, Serialize};

    // ============================================================
    // VALUE OBJECTS
    // ============================================================

    /// Unique identifier for a delta
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
    pub struct DeltaId(pub u128);

    impl DeltaId {
        pub fn new() -> Self {
            Self(uuid::Uuid::new_v4().as_u128())
        }

        pub fn from_bytes(bytes: [u8; 16]) -> Self {
            Self(u128::from_be_bytes(bytes))
        }

        pub fn to_bytes(&self) -> [u8; 16] {
            self.0.to_be_bytes()
        }
    }

    /// Logical timestamp for causal ordering
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
    pub struct DeltaTimestamp {
        /// Logical clock (Lamport timestamp)
        pub logical: u64,
        /// Physical wall-clock time (milliseconds since epoch)
        pub physical: u64,
        /// Node identifier for tie-breaking
        pub node_id: u32,
    }

    impl DeltaTimestamp {
        pub fn new(logical: u64, physical: u64, node_id: u32) -> Self {
            Self { logical, physical, node_id }
        }

        /// Advance logical clock, ensuring it's ahead of physical time
        pub fn tick(&self) -> Self {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            Self {
                logical: self.logical + 1,
                physical: now_ms.max(self.physical),
                node_id: self.node_id,
            }
        }

        /// Merge with another timestamp (for receiving events)
        pub fn merge(&self, other: &DeltaTimestamp) -> Self {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            Self {
                logical: self.logical.max(other.logical) + 1,
                physical: now_ms.max(self.physical).max(other.physical),
                node_id: self.node_id,
            }
        }
    }

    /// Checksum for delta integrity verification
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
    pub struct DeltaChecksum(pub [u8; 32]);

    impl DeltaChecksum {
        /// Compute Blake3 hash of delta payload
        pub fn compute(data: &[u8]) -> Self {
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(data);
            let result = hasher.finalize();
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&result);
            Self(bytes)
        }

        /// Chain with previous checksum for tamper-evidence
        pub fn chain(&self, previous: &DeltaChecksum, data: &[u8]) -> Self {
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(&previous.0);
            hasher.update(data);
            let result = hasher.finalize();
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&result);
            Self(bytes)
        }
    }

    /// Magnitude/size metric for a delta
    #[derive(Clone, Copy, Debug, Serialize, Deserialize)]
    pub struct DeltaMagnitude {
        /// Number of dimensions changed
        pub dimensions_changed: u32,
        /// Total L2 norm of the change
        pub l2_norm: f32,
        /// Maximum single-dimension change
        pub max_component: f32,
        /// Sparsity ratio (changed/total dimensions)
        pub sparsity: f32,
    }

    impl DeltaMagnitude {
        pub fn compute(old: &[f32], new: &[f32]) -> Self {
            assert_eq!(old.len(), new.len());

            let mut dims_changed = 0u32;
            let mut l2_sum = 0.0f32;
            let mut max_comp = 0.0f32;

            for (o, n) in old.iter().zip(new.iter()) {
                let diff = (n - o).abs();
                if diff > f32::EPSILON {
                    dims_changed += 1;
                    l2_sum += diff * diff;
                    max_comp = max_comp.max(diff);
                }
            }

            Self {
                dimensions_changed: dims_changed,
                l2_norm: l2_sum.sqrt(),
                max_component: max_comp,
                sparsity: dims_changed as f32 / old.len() as f32,
            }
        }

        /// Check if delta is significant enough to record
        pub fn is_significant(&self, threshold: f32) -> bool {
            self.l2_norm > threshold
        }
    }

    /// Sparse representation of vector changes
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct DeltaVector {
        /// Total dimensions of the full vector
        pub total_dims: u32,
        /// Indices of changed dimensions
        pub indices: Vec<u32>,
        /// Delta values (new - old) for each changed index
        pub values: Vec<f32>,
        /// Magnitude metrics
        pub magnitude: DeltaMagnitude,
    }

    impl DeltaVector {
        /// Create from old and new vectors
        pub fn from_diff(old: &[f32], new: &[f32], min_diff: f32) -> Self {
            assert_eq!(old.len(), new.len());

            let mut indices = Vec::new();
            let mut values = Vec::new();

            for (i, (o, n)) in old.iter().zip(new.iter()).enumerate() {
                let diff = n - o;
                if diff.abs() > min_diff {
                    indices.push(i as u32);
                    values.push(diff);
                }
            }

            Self {
                total_dims: old.len() as u32,
                indices,
                values,
                magnitude: DeltaMagnitude::compute(old, new),
            }
        }

        /// Apply delta to a base vector
        pub fn apply(&self, base: &mut [f32]) {
            assert_eq!(base.len(), self.total_dims as usize);

            for (&idx, &val) in self.indices.iter().zip(self.values.iter()) {
                base[idx as usize] += val;
            }
        }

        /// Invert delta (for rollback)
        pub fn invert(&self) -> Self {
            Self {
                total_dims: self.total_dims,
                indices: self.indices.clone(),
                values: self.values.iter().map(|v| -v).collect(),
                magnitude: self.magnitude,
            }
        }

        /// Compose two deltas (this then other)
        pub fn compose(&self, other: &DeltaVector) -> Self {
            assert_eq!(self.total_dims, other.total_dims);

            let mut combined: HashMap<u32, f32> = HashMap::new();

            for (&idx, &val) in self.indices.iter().zip(self.values.iter()) {
                *combined.entry(idx).or_insert(0.0) += val;
            }
            for (&idx, &val) in other.indices.iter().zip(other.values.iter()) {
                *combined.entry(idx).or_insert(0.0) += val;
            }

            // Filter out zero changes
            let filtered: Vec<_> = combined.into_iter()
                .filter(|(_, v)| v.abs() > f32::EPSILON)
                .collect();

            let mut indices: Vec<u32> = filtered.iter().map(|(i, _)| *i).collect();
            indices.sort();

            let values: Vec<f32> = indices.iter()
                .map(|i| filtered.iter().find(|(idx, _)| idx == i).unwrap().1)
                .collect();

            Self {
                total_dims: self.total_dims,
                indices,
                values,
                magnitude: DeltaMagnitude {
                    dimensions_changed: filtered.len() as u32,
                    l2_norm: values.iter().map(|v| v * v).sum::<f32>().sqrt(),
                    max_component: values.iter().map(|v| v.abs()).fold(0.0, f32::max),
                    sparsity: filtered.len() as f32 / self.total_dims as f32,
                },
            }
        }

        /// Serialize to bytes
        pub fn to_bytes(&self) -> Vec<u8> {
            bincode::serialize(self).unwrap()
        }

        /// Deserialize from bytes
        pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::Error> {
            bincode::deserialize(bytes)
        }
    }

    // ============================================================
    // AGGREGATES
    // ============================================================

    /// Source identifier being observed
    #[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
    pub struct SourceId(pub String);

    /// Observer configuration and state
    #[derive(Clone, Debug)]
    pub struct Observer {
        pub id: ObserverId,
        pub source_id: SourceId,
        pub capture_policy: CapturePolicy,
        pub status: ObserverStatus,
        pub last_capture: Option<DeltaTimestamp>,
        pub metrics: ObserverMetrics,
    }

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct ObserverId(pub u64);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ObserverStatus {
        Active,
        Paused,
        Error,
        Terminated,
    }

    #[derive(Clone, Debug)]
    pub struct CapturePolicy {
        /// Minimum time between captures (milliseconds)
        pub min_interval_ms: u64,
        /// Minimum magnitude threshold for capture
        pub magnitude_threshold: f32,
        /// Maximum deltas to buffer before force-flush
        pub buffer_limit: usize,
        /// Whether to capture zero-deltas as heartbeats
        pub capture_heartbeats: bool,
    }

    impl Default for CapturePolicy {
        fn default() -> Self {
            Self {
                min_interval_ms: 100,
                magnitude_threshold: 1e-6,
                buffer_limit: 1000,
                capture_heartbeats: false,
            }
        }
    }

    #[derive(Clone, Debug, Default)]
    pub struct ObserverMetrics {
        pub deltas_captured: u64,
        pub deltas_filtered: u64,
        pub bytes_processed: u64,
        pub avg_magnitude: f32,
        pub last_error: Option<String>,
    }

    // ============================================================
    // DELTA AGGREGATE ROOT
    // ============================================================

    /// The core Delta entity - immutable once created
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct Delta {
        /// Unique identifier
        pub id: DeltaId,
        /// Source that produced this delta
        pub source_id: SourceId,
        /// Causal timestamp
        pub timestamp: DeltaTimestamp,
        /// Previous delta in the chain (None for genesis)
        pub parent_id: Option<DeltaId>,
        /// The actual change data
        pub vector: DeltaVector,
        /// Integrity checksum (chained with parent)
        pub checksum: DeltaChecksum,
        /// Additional metadata
        pub metadata: HashMap<String, String>,
    }

    impl Delta {
        /// Create a new delta
        pub fn new(
            source_id: SourceId,
            timestamp: DeltaTimestamp,
            parent: Option<&Delta>,
            vector: DeltaVector,
            metadata: HashMap<String, String>,
        ) -> Self {
            let id = DeltaId::new();
            let parent_id = parent.map(|p| p.id);

            let payload = vector.to_bytes();
            let checksum = match parent {
                Some(p) => p.checksum.chain(&p.checksum, &payload),
                None => DeltaChecksum::compute(&payload),
            };

            Self {
                id,
                source_id,
                timestamp,
                parent_id,
                vector,
                checksum,
                metadata,
            }
        }

        /// Verify checksum chain integrity
        pub fn verify_chain(&self, parent: Option<&Delta>) -> bool {
            let payload = self.vector.to_bytes();
            let expected = match parent {
                Some(p) => p.checksum.chain(&p.checksum, &payload),
                None => DeltaChecksum::compute(&payload),
            };
            self.checksum == expected
        }

        /// Check if this delta is a descendant of another
        pub fn is_descendant_of(&self, ancestor_id: DeltaId) -> bool {
            self.parent_id == Some(ancestor_id)
        }
    }
}
```

#### Domain Events

| Event | Payload | Published When |
|-------|---------|----------------|
| `ChangeDetected` | source_id, old_state_hash, new_state_hash | Observer detects state modification |
| `DeltaExtracted` | delta_id, source_id, magnitude | Delta computed from change |
| `DeltaCaptured` | delta_id, timestamp, checksum | Delta committed to capture buffer |
| `CaptureBufferFlushed` | delta_count, batch_id | Buffer contents sent downstream |
| `ObserverError` | observer_id, error_type, message | Capture failure |
| `ObserverPaused` | observer_id, reason | Observer temporarily stopped |

#### Domain Services

```rust
/// Domain services for Delta Capture
pub mod capture_services {
    use super::delta_capture::*;

    /// Trait for change detection algorithms
    pub trait ChangeDetector: Send + Sync {
        /// Compare states and determine if change is significant
        fn detect(&self, old: &[f32], new: &[f32], policy: &CapturePolicy) -> bool;

        /// Get detector configuration
        fn config(&self) -> DetectorConfig;
    }

    #[derive(Clone, Debug)]
    pub struct DetectorConfig {
        pub algorithm: String,
        pub threshold: f32,
        pub use_cosine: bool,
    }

    /// Default detector using L2 norm threshold
    pub struct L2ThresholdDetector {
        pub threshold: f32,
    }

    impl ChangeDetector for L2ThresholdDetector {
        fn detect(&self, old: &[f32], new: &[f32], policy: &CapturePolicy) -> bool {
            let magnitude = DeltaMagnitude::compute(old, new);
            magnitude.l2_norm > self.threshold.max(policy.magnitude_threshold)
        }

        fn config(&self) -> DetectorConfig {
            DetectorConfig {
                algorithm: "l2_threshold".to_string(),
                threshold: self.threshold,
                use_cosine: false,
            }
        }
    }

    /// Trait for delta extraction
    pub trait DeltaExtractor: Send + Sync {
        /// Extract delta from state transition
        fn extract(
            &self,
            source_id: &SourceId,
            old_state: &[f32],
            new_state: &[f32],
            timestamp: DeltaTimestamp,
            parent: Option<&Delta>,
        ) -> Delta;
    }

    /// Default sparse delta extractor
    pub struct SparseDeltaExtractor {
        pub min_component_diff: f32,
    }

    impl DeltaExtractor for SparseDeltaExtractor {
        fn extract(
            &self,
            source_id: &SourceId,
            old_state: &[f32],
            new_state: &[f32],
            timestamp: DeltaTimestamp,
            parent: Option<&Delta>,
        ) -> Delta {
            let vector = DeltaVector::from_diff(old_state, new_state, self.min_component_diff);
            Delta::new(
                source_id.clone(),
                timestamp,
                parent,
                vector,
                std::collections::HashMap::new(),
            )
        }
    }

    /// Capture orchestration service
    pub trait CaptureService: Send + Sync {
        /// Register an observer for a source
        fn register_observer(
            &mut self,
            source_id: SourceId,
            policy: CapturePolicy,
        ) -> Result<ObserverId, CaptureError>;

        /// Process a state change notification
        fn on_state_change(
            &mut self,
            observer_id: ObserverId,
            old_state: &[f32],
            new_state: &[f32],
        ) -> Result<Option<Delta>, CaptureError>;

        /// Flush buffered deltas
        fn flush(&mut self, observer_id: ObserverId) -> Result<Vec<Delta>, CaptureError>;
    }

    #[derive(Debug)]
    pub enum CaptureError {
        ObserverNotFound(ObserverId),
        PolicyViolation(String),
        ExtractionFailed(String),
        BufferOverflow,
    }
}
```

---

### 3.2 Delta Propagation Domain

**Purpose**: Route deltas through the system to interested subscribers with ordering guarantees.

#### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Channel** | Named conduit for delta transmission |
| **Subscriber** | Consumer registered to receive deltas from channels |
| **Router** | Component that directs deltas to appropriate channels |
| **RoutingPolicy** | Rules for delta channel assignment |
| **Backpressure** | Flow control mechanism when subscribers are slow |
| **DeliveryGuarantee** | At-least-once, at-most-once, or exactly-once semantics |

#### Aggregate Roots

```rust
/// Delta Propagation Domain
pub mod delta_propagation {
    use super::delta_capture::*;
    use std::collections::{HashMap, HashSet};

    // ============================================================
    // VALUE OBJECTS
    // ============================================================

    #[derive(Clone, PartialEq, Eq, Hash, Debug)]
    pub struct ChannelId(pub String);

    #[derive(Clone, PartialEq, Eq, Hash, Debug)]
    pub struct SubscriberId(pub u64);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum DeliveryGuarantee {
        /// Fire and forget
        AtMostOnce,
        /// Retry until acknowledged
        AtLeastOnce,
        /// Deduplicated delivery
        ExactlyOnce,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SubscriberStatus {
        Active,
        Paused,
        Backpressured,
        Disconnected,
    }

    /// Filter for selective subscription
    #[derive(Clone, Debug)]
    pub struct SubscriptionFilter {
        /// Source patterns to match (glob-style)
        pub source_patterns: Vec<String>,
        /// Minimum magnitude to receive
        pub min_magnitude: Option<f32>,
        /// Metadata key-value matches
        pub metadata_filters: HashMap<String, String>,
    }

    impl SubscriptionFilter {
        pub fn matches(&self, delta: &Delta) -> bool {
            // Check source pattern
            let source_match = self.source_patterns.is_empty() ||
                self.source_patterns.iter().any(|pat| {
                    glob_match(pat, &delta.source_id.0)
                });

            // Check magnitude
            let magnitude_match = self.min_magnitude
                .map(|min| delta.vector.magnitude.l2_norm >= min)
                .unwrap_or(true);

            // Check metadata
            let metadata_match = self.metadata_filters.iter().all(|(k, v)| {
                delta.metadata.get(k).map(|mv| mv == v).unwrap_or(false)
            });

            source_match && magnitude_match && metadata_match
        }
    }

    fn glob_match(pattern: &str, text: &str) -> bool {
        // Simple glob matching (* = any)
        if pattern == "*" { return true; }
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                return text.starts_with(parts[0]) && text.ends_with(parts[1]);
            }
        }
        pattern == text
    }

    // ============================================================
    // AGGREGATES
    // ============================================================

    /// Channel for delta distribution
    #[derive(Clone, Debug)]
    pub struct Channel {
        pub id: ChannelId,
        pub name: String,
        pub delivery_guarantee: DeliveryGuarantee,
        pub subscribers: HashSet<SubscriberId>,
        pub metrics: ChannelMetrics,
        pub created_at: u64,
    }

    #[derive(Clone, Debug, Default)]
    pub struct ChannelMetrics {
        pub deltas_published: u64,
        pub deltas_delivered: u64,
        pub avg_latency_ms: f32,
        pub subscriber_count: u32,
    }

    impl Channel {
        pub fn new(id: ChannelId, name: String, guarantee: DeliveryGuarantee) -> Self {
            Self {
                id,
                name,
                delivery_guarantee: guarantee,
                subscribers: HashSet::new(),
                metrics: ChannelMetrics::default(),
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            }
        }

        pub fn add_subscriber(&mut self, sub_id: SubscriberId) -> bool {
            self.subscribers.insert(sub_id)
        }

        pub fn remove_subscriber(&mut self, sub_id: &SubscriberId) -> bool {
            self.subscribers.remove(sub_id)
        }
    }

    /// Subscriber registration
    #[derive(Clone, Debug)]
    pub struct Subscriber {
        pub id: SubscriberId,
        pub name: String,
        pub channels: HashSet<ChannelId>,
        pub filter: SubscriptionFilter,
        pub status: SubscriberStatus,
        pub cursor: SubscriberCursor,
        pub metrics: SubscriberMetrics,
    }

    /// Tracks subscriber progress through delta stream
    #[derive(Clone, Debug)]
    pub struct SubscriberCursor {
        /// Last acknowledged delta per channel
        pub last_acked: HashMap<ChannelId, DeltaId>,
        /// Last timestamp received
        pub last_timestamp: Option<DeltaTimestamp>,
        /// Pending deltas awaiting acknowledgment
        pub pending_count: u32,
    }

    #[derive(Clone, Debug, Default)]
    pub struct SubscriberMetrics {
        pub deltas_received: u64,
        pub deltas_acked: u64,
        pub avg_processing_time_ms: f32,
        pub backpressure_events: u32,
    }

    /// Routing decision for a delta
    #[derive(Clone, Debug)]
    pub struct RoutingDecision {
        pub delta_id: DeltaId,
        pub target_channels: Vec<ChannelId>,
        pub priority: RoutingPriority,
        pub ttl_ms: Option<u64>,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum RoutingPriority {
        Low = 0,
        Normal = 1,
        High = 2,
        Critical = 3,
    }

    /// Routing policy definition
    #[derive(Clone, Debug)]
    pub struct RoutingPolicy {
        pub id: String,
        pub source_pattern: String,
        pub target_channels: Vec<ChannelId>,
        pub priority: RoutingPriority,
        pub conditions: Vec<RoutingCondition>,
    }

    #[derive(Clone, Debug)]
    pub enum RoutingCondition {
        MinMagnitude(f32),
        MetadataEquals(String, String),
        MetadataExists(String),
        TimeOfDay { start_hour: u8, end_hour: u8 },
    }
}
```

#### Domain Events

| Event | Payload | Published When |
|-------|---------|----------------|
| `DeltaRouted` | delta_id, channel_ids, priority | Router assigns delta to channels |
| `DeltaPublished` | delta_id, channel_id, subscriber_count | Delta sent to channel |
| `DeltaDelivered` | delta_id, subscriber_id, latency_ms | Subscriber receives delta |
| `DeltaAcknowledged` | delta_id, subscriber_id | Subscriber confirms processing |
| `SubscriberBackpressured` | subscriber_id, pending_count | Subscriber overwhelmed |
| `ChannelCreated` | channel_id, delivery_guarantee | New channel registered |
| `SubscriptionChanged` | subscriber_id, added_channels, removed_channels | Subscription modified |

#### Domain Services

```rust
/// Domain services for Delta Propagation
pub mod propagation_services {
    use super::delta_propagation::*;
    use super::delta_capture::*;

    /// Router service for delta distribution
    pub trait DeltaRouter: Send + Sync {
        /// Determine target channels for a delta
        fn route(&self, delta: &Delta) -> RoutingDecision;

        /// Register a routing policy
        fn add_policy(&mut self, policy: RoutingPolicy) -> Result<(), RouterError>;

        /// Remove a routing policy
        fn remove_policy(&mut self, policy_id: &str) -> Result<(), RouterError>;
    }

    /// Channel management service
    pub trait ChannelService: Send + Sync {
        /// Create a new channel
        fn create_channel(
            &mut self,
            id: ChannelId,
            name: String,
            guarantee: DeliveryGuarantee,
        ) -> Result<Channel, ChannelError>;

        /// Publish delta to channel
        fn publish(&mut self, channel_id: &ChannelId, delta: Delta) -> Result<u32, ChannelError>;

        /// Get channel statistics
        fn get_metrics(&self, channel_id: &ChannelId) -> Option<ChannelMetrics>;
    }

    /// Subscription management service
    pub trait SubscriptionService: Send + Sync {
        /// Create subscriber
        fn subscribe(
            &mut self,
            name: String,
            channels: Vec<ChannelId>,
            filter: SubscriptionFilter,
        ) -> Result<SubscriberId, SubscriptionError>;

        /// Acknowledge delta receipt
        fn acknowledge(
            &mut self,
            subscriber_id: SubscriberId,
            delta_id: DeltaId,
        ) -> Result<(), SubscriptionError>;

        /// Get pending deltas for subscriber
        fn poll(
            &self,
            subscriber_id: SubscriberId,
            max_count: usize,
        ) -> Result<Vec<Delta>, SubscriptionError>;
    }

    #[derive(Debug)]
    pub enum RouterError {
        PolicyConflict(String),
        InvalidPattern(String),
    }

    #[derive(Debug)]
    pub enum ChannelError {
        NotFound(ChannelId),
        AlreadyExists(ChannelId),
        PublishFailed(String),
    }

    #[derive(Debug)]
    pub enum SubscriptionError {
        SubscriberNotFound(SubscriberId),
        ChannelNotFound(ChannelId),
        Backpressured,
        InvalidFilter(String),
    }
}
```

---

### 3.3 Delta Aggregation Domain

**Purpose**: Combine, batch, and compress deltas for efficient storage and transmission.

#### Ubiquitous Language

| Term | Definition |
|------|------------|
| **DeltaWindow** | Temporal container grouping deltas by time or count |
| **Batch** | Collection of deltas packaged for bulk processing |
| **Compaction** | Process of merging sequential deltas into fewer |
| **Compression** | Reducing delta byte size through encoding |
| **WindowPolicy** | Rules for window boundaries (time, count, size) |
| **AggregatedDelta** | Result of combining multiple deltas |

#### Aggregate Roots

```rust
/// Delta Aggregation Domain
pub mod delta_aggregation {
    use super::delta_capture::*;
    use std::collections::HashMap;

    // ============================================================
    // VALUE OBJECTS
    // ============================================================

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct WindowId(pub u64);

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct BatchId(pub u64);

    /// Window boundary policy
    #[derive(Clone, Debug)]
    pub struct WindowPolicy {
        /// Maximum time span in milliseconds
        pub max_duration_ms: u64,
        /// Maximum delta count
        pub max_count: usize,
        /// Maximum aggregate size in bytes
        pub max_bytes: usize,
        /// Force window close on these events
        pub close_on_metadata: Vec<String>,
    }

    impl Default for WindowPolicy {
        fn default() -> Self {
            Self {
                max_duration_ms: 1000,
                max_count: 100,
                max_bytes: 1024 * 1024, // 1MB
                close_on_metadata: vec![],
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum WindowStatus {
        Open,
        Closing,
        Closed,
        Compacted,
    }

    // ============================================================
    // AGGREGATES
    // ============================================================

    /// Temporal window for delta collection
    #[derive(Clone, Debug)]
    pub struct DeltaWindow {
        pub id: WindowId,
        pub source_id: SourceId,
        pub policy: WindowPolicy,
        pub status: WindowStatus,
        /// Deltas in this window (ordered by timestamp)
        pub deltas: Vec<Delta>,
        /// Window start timestamp
        pub started_at: DeltaTimestamp,
        /// Window close timestamp (if closed)
        pub closed_at: Option<DeltaTimestamp>,
        /// Aggregate metrics
        pub metrics: WindowMetrics,
    }

    #[derive(Clone, Debug, Default)]
    pub struct WindowMetrics {
        pub delta_count: u32,
        pub total_bytes: u64,
        pub total_magnitude: f32,
        pub dimensions_touched: u32,
    }

    impl DeltaWindow {
        pub fn new(id: WindowId, source_id: SourceId, policy: WindowPolicy, start: DeltaTimestamp) -> Self {
            Self {
                id,
                source_id,
                policy,
                status: WindowStatus::Open,
                deltas: Vec::new(),
                started_at: start,
                closed_at: None,
                metrics: WindowMetrics::default(),
            }
        }

        /// Check if window should close
        pub fn should_close(&self, current_time_ms: u64) -> bool {
            if self.status != WindowStatus::Open {
                return false;
            }

            // Time limit
            let elapsed = current_time_ms - self.started_at.physical;
            if elapsed >= self.policy.max_duration_ms {
                return true;
            }

            // Count limit
            if self.deltas.len() >= self.policy.max_count {
                return true;
            }

            // Size limit
            if self.metrics.total_bytes as usize >= self.policy.max_bytes {
                return true;
            }

            false
        }

        /// Add delta to window
        pub fn add(&mut self, delta: Delta) -> Result<(), WindowError> {
            if self.status != WindowStatus::Open {
                return Err(WindowError::WindowClosed);
            }

            // Check for close-on-metadata triggers
            for key in &self.policy.close_on_metadata {
                if delta.metadata.contains_key(key) {
                    self.status = WindowStatus::Closing;
                }
            }

            let delta_bytes = delta.vector.to_bytes().len() as u64;
            self.metrics.delta_count += 1;
            self.metrics.total_bytes += delta_bytes;
            self.metrics.total_magnitude += delta.vector.magnitude.l2_norm;
            self.metrics.dimensions_touched = self.metrics.dimensions_touched
                .max(delta.vector.magnitude.dimensions_changed);

            self.deltas.push(delta);
            Ok(())
        }

        /// Close the window
        pub fn close(&mut self, timestamp: DeltaTimestamp) {
            self.status = WindowStatus::Closed;
            self.closed_at = Some(timestamp);
        }

        /// Compact all deltas into an aggregated delta
        pub fn compact(&mut self) -> Option<AggregatedDelta> {
            if self.deltas.is_empty() {
                return None;
            }

            // Compose all deltas
            let mut composed = self.deltas[0].vector.clone();
            for delta in self.deltas.iter().skip(1) {
                composed = composed.compose(&delta.vector);
            }

            let first = self.deltas.first().unwrap();
            let last = self.deltas.last().unwrap();

            self.status = WindowStatus::Compacted;

            Some(AggregatedDelta {
                window_id: self.id,
                source_id: self.source_id.clone(),
                first_delta_id: first.id,
                last_delta_id: last.id,
                delta_count: self.deltas.len() as u32,
                composed_vector: composed,
                time_span: TimeSpan {
                    start: first.timestamp,
                    end: last.timestamp,
                },
                compression_ratio: self.compute_compression_ratio(&composed),
            })
        }

        fn compute_compression_ratio(&self, composed: &DeltaVector) -> f32 {
            let original_bytes: usize = self.deltas.iter()
                .map(|d| d.vector.to_bytes().len())
                .sum();
            let composed_bytes = composed.to_bytes().len();

            if composed_bytes > 0 {
                original_bytes as f32 / composed_bytes as f32
            } else {
                1.0
            }
        }
    }

    /// Result of window compaction
    #[derive(Clone, Debug)]
    pub struct AggregatedDelta {
        pub window_id: WindowId,
        pub source_id: SourceId,
        pub first_delta_id: DeltaId,
        pub last_delta_id: DeltaId,
        pub delta_count: u32,
        pub composed_vector: DeltaVector,
        pub time_span: TimeSpan,
        pub compression_ratio: f32,
    }

    #[derive(Clone, Debug)]
    pub struct TimeSpan {
        pub start: DeltaTimestamp,
        pub end: DeltaTimestamp,
    }

    /// Batch of deltas for bulk operations
    #[derive(Clone, Debug)]
    pub struct DeltaBatch {
        pub id: BatchId,
        pub deltas: Vec<Delta>,
        pub created_at: u64,
        pub checksum: DeltaChecksum,
    }

    impl DeltaBatch {
        pub fn new(deltas: Vec<Delta>) -> Self {
            let id = BatchId(rand::random());

            // Compute batch checksum
            let mut data = Vec::new();
            for delta in &deltas {
                data.extend(&delta.id.to_bytes());
            }
            let checksum = DeltaChecksum::compute(&data);

            Self {
                id,
                deltas,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                checksum,
            }
        }

        pub fn len(&self) -> usize {
            self.deltas.len()
        }

        pub fn is_empty(&self) -> bool {
            self.deltas.is_empty()
        }
    }

    #[derive(Debug)]
    pub enum WindowError {
        WindowClosed,
        PolicyViolation(String),
    }
}
```

#### Domain Events

| Event | Payload | Published When |
|-------|---------|----------------|
| `WindowOpened` | window_id, source_id, policy | New aggregation window started |
| `WindowClosed` | window_id, delta_count, duration_ms | Window reached boundary |
| `WindowCompacted` | window_id, compression_ratio | Deltas merged within window |
| `BatchCreated` | batch_id, delta_count, checksum | Batch assembled |
| `BatchCompressed` | batch_id, original_size, compressed_size | Batch compressed for storage |
| `AggregationPolicyChanged` | source_id, old_policy, new_policy | Window policy updated |

---

### 3.4 Delta Application Domain

**Purpose**: Apply deltas to target states with validation and transformation.

#### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Applicator** | Component that applies deltas to target state |
| **Target** | State vector being modified by deltas |
| **Validator** | Component that verifies delta applicability |
| **Transformer** | Component that modifies deltas before application |
| **ApplicationResult** | Outcome of delta application (success/failure) |
| **Rollback** | Reverting applied deltas |

#### Aggregate Roots

```rust
/// Delta Application Domain
pub mod delta_application {
    use super::delta_capture::*;
    use std::collections::HashMap;

    // ============================================================
    // VALUE OBJECTS
    // ============================================================

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct TargetId(pub u64);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ApplicationStatus {
        Pending,
        Applied,
        Failed,
        RolledBack,
    }

    #[derive(Clone, Debug)]
    pub enum ValidationResult {
        Valid,
        Invalid { reason: String },
        Warning { message: String },
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ConflictResolution {
        /// Last write wins
        LastWriteWins,
        /// First write wins
        FirstWriteWins,
        /// Merge by averaging
        Merge,
        /// Reject on conflict
        Reject,
        /// Custom resolution function
        Custom,
    }

    // ============================================================
    // AGGREGATES
    // ============================================================

    /// Target state that receives delta applications
    #[derive(Clone, Debug)]
    pub struct DeltaTarget {
        pub id: TargetId,
        pub source_id: SourceId,
        /// Current state vector
        pub state: Vec<f32>,
        /// Last applied delta
        pub last_delta_id: Option<DeltaId>,
        /// Last application timestamp
        pub last_applied_at: Option<DeltaTimestamp>,
        /// Application policy
        pub policy: ApplicationPolicy,
        /// Application history (ring buffer)
        pub history: ApplicationHistory,
    }

    #[derive(Clone, Debug)]
    pub struct ApplicationPolicy {
        /// How to handle conflicts
        pub conflict_resolution: ConflictResolution,
        /// Maximum magnitude allowed per delta
        pub max_magnitude: Option<f32>,
        /// Dimensions that are read-only
        pub locked_dimensions: Vec<u32>,
        /// Whether to validate checksum chain
        pub verify_chain: bool,
        /// Maximum history entries to keep
        pub history_limit: usize,
    }

    impl Default for ApplicationPolicy {
        fn default() -> Self {
            Self {
                conflict_resolution: ConflictResolution::LastWriteWins,
                max_magnitude: None,
                locked_dimensions: vec![],
                verify_chain: true,
                history_limit: 1000,
            }
        }
    }

    /// Ring buffer of recent applications
    #[derive(Clone, Debug)]
    pub struct ApplicationHistory {
        pub entries: Vec<ApplicationEntry>,
        pub capacity: usize,
        pub head: usize,
    }

    #[derive(Clone, Debug)]
    pub struct ApplicationEntry {
        pub delta_id: DeltaId,
        pub applied_at: DeltaTimestamp,
        pub status: ApplicationStatus,
        /// Delta for rollback (inverted)
        pub rollback_delta: Option<DeltaVector>,
    }

    impl ApplicationHistory {
        pub fn new(capacity: usize) -> Self {
            Self {
                entries: Vec::with_capacity(capacity),
                capacity,
                head: 0,
            }
        }

        pub fn push(&mut self, entry: ApplicationEntry) {
            if self.entries.len() < self.capacity {
                self.entries.push(entry);
            } else {
                self.entries[self.head] = entry;
            }
            self.head = (self.head + 1) % self.capacity;
        }

        pub fn last(&self) -> Option<&ApplicationEntry> {
            if self.entries.is_empty() {
                None
            } else {
                let idx = if self.head == 0 {
                    self.entries.len() - 1
                } else {
                    self.head - 1
                };
                self.entries.get(idx)
            }
        }

        /// Get entries for rollback (most recent first)
        pub fn rollback_entries(&self, count: usize) -> Vec<&ApplicationEntry> {
            let mut result = Vec::with_capacity(count);
            let len = self.entries.len().min(count);

            for i in 0..len {
                let idx = if self.head >= i + 1 {
                    self.head - i - 1
                } else {
                    self.entries.len() - (i + 1 - self.head)
                };
                if let Some(entry) = self.entries.get(idx) {
                    if entry.status == ApplicationStatus::Applied {
                        result.push(entry);
                    }
                }
            }

            result
        }
    }

    impl DeltaTarget {
        pub fn new(
            id: TargetId,
            source_id: SourceId,
            initial_state: Vec<f32>,
            policy: ApplicationPolicy,
        ) -> Self {
            Self {
                id,
                source_id,
                state: initial_state,
                last_delta_id: None,
                last_applied_at: None,
                policy: policy.clone(),
                history: ApplicationHistory::new(policy.history_limit),
            }
        }

        /// Validate a delta before application
        pub fn validate(&self, delta: &Delta) -> ValidationResult {
            // Check source matches
            if delta.source_id != self.source_id {
                return ValidationResult::Invalid {
                    reason: "Source ID mismatch".to_string(),
                };
            }

            // Check dimensions match
            if delta.vector.total_dims as usize != self.state.len() {
                return ValidationResult::Invalid {
                    reason: format!(
                        "Dimension mismatch: delta has {} dims, target has {}",
                        delta.vector.total_dims, self.state.len()
                    ),
                };
            }

            // Check magnitude limit
            if let Some(max_mag) = self.policy.max_magnitude {
                if delta.vector.magnitude.l2_norm > max_mag {
                    return ValidationResult::Invalid {
                        reason: format!(
                            "Magnitude {} exceeds limit {}",
                            delta.vector.magnitude.l2_norm, max_mag
                        ),
                    };
                }
            }

            // Check locked dimensions
            for &locked_dim in &self.policy.locked_dimensions {
                if delta.vector.indices.contains(&locked_dim) {
                    return ValidationResult::Invalid {
                        reason: format!("Dimension {} is locked", locked_dim),
                    };
                }
            }

            // Check causal ordering (parent should be our last applied)
            if self.policy.verify_chain {
                if let Some(expected_parent) = self.last_delta_id {
                    if delta.parent_id != Some(expected_parent) {
                        return ValidationResult::Warning {
                            message: format!(
                                "Non-sequential delta: expected parent {:?}, got {:?}",
                                expected_parent, delta.parent_id
                            ),
                        };
                    }
                }
            }

            ValidationResult::Valid
        }

        /// Apply a delta to the target state
        pub fn apply(&mut self, delta: &Delta) -> Result<ApplicationResult, ApplicationError> {
            // Validate first
            match self.validate(delta) {
                ValidationResult::Invalid { reason } => {
                    return Err(ApplicationError::ValidationFailed(reason));
                }
                ValidationResult::Warning { message } => {
                    // Log warning but continue
                    eprintln!("Warning: {}", message);
                }
                ValidationResult::Valid => {}
            }

            // Store rollback delta
            let rollback_delta = delta.vector.invert();

            // Apply the delta
            delta.vector.apply(&mut self.state);

            // Update metadata
            self.last_delta_id = Some(delta.id);
            self.last_applied_at = Some(delta.timestamp);

            // Record in history
            self.history.push(ApplicationEntry {
                delta_id: delta.id,
                applied_at: delta.timestamp,
                status: ApplicationStatus::Applied,
                rollback_delta: Some(rollback_delta),
            });

            Ok(ApplicationResult {
                delta_id: delta.id,
                target_id: self.id,
                status: ApplicationStatus::Applied,
                new_state_hash: self.compute_state_hash(),
            })
        }

        /// Rollback the last N applied deltas
        pub fn rollback(&mut self, count: usize) -> Result<Vec<DeltaId>, ApplicationError> {
            let entries = self.history.rollback_entries(count);

            if entries.is_empty() {
                return Err(ApplicationError::NothingToRollback);
            }

            let mut rolled_back = Vec::with_capacity(entries.len());

            for entry in entries {
                if let Some(ref rollback_delta) = entry.rollback_delta {
                    rollback_delta.apply(&mut self.state);
                    rolled_back.push(entry.delta_id);
                }
            }

            // Update last_delta_id to the one before rollback
            self.last_delta_id = self.history.entries
                .iter()
                .filter(|e| e.status == ApplicationStatus::Applied && !rolled_back.contains(&e.delta_id))
                .last()
                .map(|e| e.delta_id);

            Ok(rolled_back)
        }

        fn compute_state_hash(&self) -> [u8; 32] {
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            for val in &self.state {
                hasher.update(&val.to_le_bytes());
            }
            let result = hasher.finalize();
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&result);
            hash
        }
    }

    #[derive(Clone, Debug)]
    pub struct ApplicationResult {
        pub delta_id: DeltaId,
        pub target_id: TargetId,
        pub status: ApplicationStatus,
        pub new_state_hash: [u8; 32],
    }

    #[derive(Debug)]
    pub enum ApplicationError {
        ValidationFailed(String),
        TargetNotFound(TargetId),
        ConflictDetected { delta_id: DeltaId, reason: String },
        NothingToRollback,
        StateCurrupted(String),
    }
}
```

#### Domain Events

| Event | Payload | Published When |
|-------|---------|----------------|
| `DeltaApplied` | delta_id, target_id, new_state_hash | Delta successfully applied |
| `DeltaRejected` | delta_id, target_id, reason | Delta failed validation |
| `DeltaConflictDetected` | delta_id, conflicting_delta_id | Concurrent modification detected |
| `DeltaMerged` | delta_ids, merged_delta_id | Conflict resolved by merging |
| `DeltaRolledBack` | delta_ids, target_id | Deltas reverted |
| `TargetStateCorrupted` | target_id, expected_hash, actual_hash | Integrity check failed |

---

### 3.5 Delta Versioning Domain

**Purpose**: Manage temporal ordering, history, branching, and state reconstruction.

#### Ubiquitous Language

| Term | Definition |
|------|------------|
| **DeltaStream** | Linear sequence of causally-ordered deltas |
| **DeltaGraph** | DAG of deltas supporting branches and merges |
| **Snapshot** | Full state capture at a specific version |
| **Branch** | Named divergence from main delta stream |
| **Merge** | Combining two branches into one |
| **Replay** | Reconstructing state by applying deltas from a point |

#### Aggregate Roots

```rust
/// Delta Versioning Domain
pub mod delta_versioning {
    use super::delta_capture::*;
    use super::delta_aggregation::*;
    use std::collections::{HashMap, HashSet, BTreeMap};

    // ============================================================
    // VALUE OBJECTS
    // ============================================================

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct StreamId(pub u64);

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct SnapshotId(pub u64);

    #[derive(Clone, PartialEq, Eq, Hash, Debug)]
    pub struct BranchId(pub String);

    /// Version identifier (sequence number in stream)
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
    pub struct Version(pub u64);

    impl Version {
        pub fn next(&self) -> Self {
            Self(self.0 + 1)
        }

        pub fn genesis() -> Self {
            Self(0)
        }
    }

    // ============================================================
    // AGGREGATES
    // ============================================================

    /// Linear delta stream (append-only log)
    #[derive(Clone, Debug)]
    pub struct DeltaStream {
        pub id: StreamId,
        pub source_id: SourceId,
        /// Deltas indexed by version
        pub deltas: BTreeMap<Version, Delta>,
        /// Current head version
        pub head: Version,
        /// Periodic snapshots for fast replay
        pub snapshots: HashMap<Version, SnapshotId>,
        /// Snapshot interval
        pub snapshot_interval: u64,
        /// Stream metadata
        pub metadata: StreamMetadata,
    }

    #[derive(Clone, Debug, Default)]
    pub struct StreamMetadata {
        pub created_at: u64,
        pub last_updated: u64,
        pub total_deltas: u64,
        pub total_bytes: u64,
    }

    impl DeltaStream {
        pub fn new(id: StreamId, source_id: SourceId, snapshot_interval: u64) -> Self {
            Self {
                id,
                source_id,
                deltas: BTreeMap::new(),
                head: Version::genesis(),
                snapshots: HashMap::new(),
                snapshot_interval,
                metadata: StreamMetadata::default(),
            }
        }

        /// Append a delta to the stream
        pub fn append(&mut self, delta: Delta) -> Version {
            let version = self.head.next();
            self.head = version;

            self.metadata.total_deltas += 1;
            self.metadata.total_bytes += delta.vector.to_bytes().len() as u64;
            self.metadata.last_updated = delta.timestamp.physical;

            self.deltas.insert(version, delta);
            version
        }

        /// Get delta at specific version
        pub fn get(&self, version: Version) -> Option<&Delta> {
            self.deltas.get(&version)
        }

        /// Get delta range (inclusive)
        pub fn range(&self, from: Version, to: Version) -> Vec<&Delta> {
            self.deltas.range(from..=to)
                .map(|(_, d)| d)
                .collect()
        }

        /// Find nearest snapshot before version
        pub fn nearest_snapshot(&self, version: Version) -> Option<(Version, SnapshotId)> {
            self.snapshots.iter()
                .filter(|(v, _)| **v <= version)
                .max_by_key(|(v, _)| *v)
                .map(|(v, s)| (*v, *s))
        }

        /// Check if snapshot is due
        pub fn should_snapshot(&self) -> bool {
            let last_snapshot_version = self.snapshots.keys().max().copied()
                .unwrap_or(Version::genesis());

            self.head.0 - last_snapshot_version.0 >= self.snapshot_interval
        }

        /// Record a snapshot
        pub fn record_snapshot(&mut self, version: Version, snapshot_id: SnapshotId) {
            self.snapshots.insert(version, snapshot_id);
        }
    }

    /// DAG-based delta graph (supports branching)
    #[derive(Clone, Debug)]
    pub struct DeltaGraph {
        pub source_id: SourceId,
        /// All deltas by ID
        pub nodes: HashMap<DeltaId, DeltaGraphNode>,
        /// Child relationships (parent -> children)
        pub edges: HashMap<DeltaId, Vec<DeltaId>>,
        /// Named branches
        pub branches: HashMap<BranchId, DeltaId>,
        /// Main branch head
        pub main_head: Option<DeltaId>,
        /// Root deltas (no parent)
        pub roots: HashSet<DeltaId>,
    }

    #[derive(Clone, Debug)]
    pub struct DeltaGraphNode {
        pub delta: Delta,
        pub version: Version,
        pub branch: Option<BranchId>,
        pub is_merge: bool,
        /// Second parent for merge commits
        pub merge_parent: Option<DeltaId>,
    }

    impl DeltaGraph {
        pub fn new(source_id: SourceId) -> Self {
            Self {
                source_id,
                nodes: HashMap::new(),
                edges: HashMap::new(),
                branches: HashMap::new(),
                main_head: None,
                roots: HashSet::new(),
            }
        }

        /// Add a delta to the graph
        pub fn add(&mut self, delta: Delta, branch: Option<BranchId>) -> DeltaId {
            let delta_id = delta.id;
            let parent_id = delta.parent_id;

            // Determine version
            let version = match parent_id {
                Some(pid) => self.nodes.get(&pid)
                    .map(|n| n.version.next())
                    .unwrap_or(Version(1)),
                None => Version(1),
            };

            // Create node
            let node = DeltaGraphNode {
                delta,
                version,
                branch: branch.clone(),
                is_merge: false,
                merge_parent: None,
            };

            self.nodes.insert(delta_id, node);

            // Update edges
            if let Some(pid) = parent_id {
                self.edges.entry(pid).or_default().push(delta_id);
            } else {
                self.roots.insert(delta_id);
            }

            // Update branch head
            if let Some(ref b) = branch {
                self.branches.insert(b.clone(), delta_id);
            } else {
                self.main_head = Some(delta_id);
            }

            delta_id
        }

        /// Create a branch from a delta
        pub fn create_branch(&mut self, branch_id: BranchId, from_delta: DeltaId) -> Result<(), VersioningError> {
            if !self.nodes.contains_key(&from_delta) {
                return Err(VersioningError::DeltaNotFound(from_delta));
            }

            if self.branches.contains_key(&branch_id) {
                return Err(VersioningError::BranchExists(branch_id));
            }

            self.branches.insert(branch_id, from_delta);
            Ok(())
        }

        /// Merge two branches
        pub fn merge(
            &mut self,
            source_branch: &BranchId,
            target_branch: &BranchId,
            merged_vector: DeltaVector,
            timestamp: DeltaTimestamp,
        ) -> Result<DeltaId, VersioningError> {
            let source_head = self.branches.get(source_branch)
                .ok_or_else(|| VersioningError::BranchNotFound(source_branch.clone()))?;
            let target_head = self.branches.get(target_branch)
                .ok_or_else(|| VersioningError::BranchNotFound(target_branch.clone()))?;

            // Create merge delta
            let merge_delta = Delta::new(
                self.source_id.clone(),
                timestamp,
                self.nodes.get(target_head).map(|n| &n.delta),
                merged_vector,
                HashMap::from([("merge".to_string(), "true".to_string())]),
            );

            let merge_id = merge_delta.id;

            // Add merge node
            let version = self.nodes.get(target_head)
                .map(|n| n.version.next())
                .unwrap_or(Version(1));

            let node = DeltaGraphNode {
                delta: merge_delta,
                version,
                branch: Some(target_branch.clone()),
                is_merge: true,
                merge_parent: Some(*source_head),
            };

            self.nodes.insert(merge_id, node);

            // Update edges (merge has two parents)
            self.edges.entry(*target_head).or_default().push(merge_id);
            self.edges.entry(*source_head).or_default().push(merge_id);

            // Update target branch head
            self.branches.insert(target_branch.clone(), merge_id);

            Ok(merge_id)
        }

        /// Get ancestry path from root to delta
        pub fn ancestry(&self, delta_id: DeltaId) -> Vec<DeltaId> {
            let mut path = Vec::new();
            let mut current = Some(delta_id);

            while let Some(id) = current {
                path.push(id);
                current = self.nodes.get(&id)
                    .and_then(|n| n.delta.parent_id);
            }

            path.reverse();
            path
        }

        /// Find common ancestor of two deltas
        pub fn common_ancestor(&self, a: DeltaId, b: DeltaId) -> Option<DeltaId> {
            let ancestry_a: HashSet<_> = self.ancestry(a).into_iter().collect();

            for ancestor in self.ancestry(b) {
                if ancestry_a.contains(&ancestor) {
                    return Some(ancestor);
                }
            }

            None
        }

        /// Get all deltas in topological order
        pub fn topological_order(&self) -> Vec<DeltaId> {
            let mut result = Vec::new();
            let mut visited = HashSet::new();
            let mut stack: Vec<DeltaId> = self.roots.iter().copied().collect();

            while let Some(id) = stack.pop() {
                if visited.contains(&id) {
                    continue;
                }
                visited.insert(id);
                result.push(id);

                if let Some(children) = self.edges.get(&id) {
                    for &child in children {
                        if !visited.contains(&child) {
                            stack.push(child);
                        }
                    }
                }
            }

            result
        }
    }

    /// Full state snapshot for fast replay
    #[derive(Clone, Debug)]
    pub struct DeltaSnapshot {
        pub id: SnapshotId,
        pub source_id: SourceId,
        /// Full state vector at this point
        pub state: Vec<f32>,
        /// Delta that produced this state
        pub delta_id: DeltaId,
        /// Stream version (if from stream)
        pub version: Version,
        /// Timestamp of snapshot
        pub created_at: DeltaTimestamp,
        /// State checksum
        pub checksum: DeltaChecksum,
    }

    impl DeltaSnapshot {
        pub fn new(
            source_id: SourceId,
            state: Vec<f32>,
            delta_id: DeltaId,
            version: Version,
            timestamp: DeltaTimestamp,
        ) -> Self {
            let mut data = Vec::new();
            for val in &state {
                data.extend(&val.to_le_bytes());
            }
            let checksum = DeltaChecksum::compute(&data);

            Self {
                id: SnapshotId(rand::random()),
                source_id,
                state,
                delta_id,
                version,
                created_at: timestamp,
                checksum,
            }
        }

        /// Verify snapshot integrity
        pub fn verify(&self) -> bool {
            let mut data = Vec::new();
            for val in &self.state {
                data.extend(&val.to_le_bytes());
            }
            let computed = DeltaChecksum::compute(&data);
            computed == self.checksum
        }
    }

    /// Index for efficient delta lookup
    #[derive(Clone, Debug)]
    pub struct DeltaIndex {
        /// Delta by ID
        by_id: HashMap<DeltaId, Delta>,
        /// Deltas by source
        by_source: HashMap<SourceId, Vec<DeltaId>>,
        /// Deltas by time range
        by_time: BTreeMap<u64, Vec<DeltaId>>,
        /// Checksum -> Delta mapping
        by_checksum: HashMap<DeltaChecksum, DeltaId>,
    }

    impl DeltaIndex {
        pub fn new() -> Self {
            Self {
                by_id: HashMap::new(),
                by_source: HashMap::new(),
                by_time: BTreeMap::new(),
                by_checksum: HashMap::new(),
            }
        }

        /// Index a delta
        pub fn insert(&mut self, delta: Delta) {
            let id = delta.id;
            let source = delta.source_id.clone();
            let time = delta.timestamp.physical;
            let checksum = delta.checksum;

            self.by_source.entry(source).or_default().push(id);
            self.by_time.entry(time).or_default().push(id);
            self.by_checksum.insert(checksum, id);
            self.by_id.insert(id, delta);
        }

        /// Lookup by ID
        pub fn get(&self, id: &DeltaId) -> Option<&Delta> {
            self.by_id.get(id)
        }

        /// Query by source
        pub fn by_source(&self, source: &SourceId) -> Vec<&Delta> {
            self.by_source.get(source)
                .map(|ids| ids.iter().filter_map(|id| self.by_id.get(id)).collect())
                .unwrap_or_default()
        }

        /// Query by time range
        pub fn by_time_range(&self, start_ms: u64, end_ms: u64) -> Vec<&Delta> {
            self.by_time.range(start_ms..=end_ms)
                .flat_map(|(_, ids)| ids.iter())
                .filter_map(|id| self.by_id.get(id))
                .collect()
        }

        /// Verify by checksum
        pub fn verify(&self, checksum: &DeltaChecksum) -> Option<&Delta> {
            self.by_checksum.get(checksum)
                .and_then(|id| self.by_id.get(id))
        }
    }

    #[derive(Debug)]
    pub enum VersioningError {
        DeltaNotFound(DeltaId),
        BranchNotFound(BranchId),
        BranchExists(BranchId),
        SnapshotNotFound(SnapshotId),
        ReplayFailed(String),
        MergeConflict { delta_a: DeltaId, delta_b: DeltaId },
    }
}
```

#### Domain Events

| Event | Payload | Published When |
|-------|---------|----------------|
| `DeltaVersioned` | delta_id, version, stream_id | Delta assigned version number |
| `SnapshotCreated` | snapshot_id, version, state_hash | Full state captured |
| `BranchCreated` | branch_id, from_delta_id | New branch started |
| `BranchMerged` | source_branch, target_branch, merge_delta_id | Branches combined |
| `StreamCompacted` | stream_id, before_count, after_count | Old deltas archived |
| `ReplayStarted` | from_version, to_version | State reconstruction begun |
| `ReplayCompleted` | target_version, delta_count, duration_ms | State reconstruction finished |

---

## 4. Bounded Context Map

```
+-----------------------------------------------------------------------+
|                        CONTEXT MAP                                      |
+-----------------------------------------------------------------------+

     +-----------------+
     | Delta Capture   |
     |    Context      |
     | (Core Domain)   |
     +--------+--------+
              |
              | [Published Language: Delta, DeltaVector, DeltaTimestamp]
              |
              v
     +-----------------+        +-----------------+
     | Delta           |<------>| Delta           |
     | Propagation     |   ACL  | Aggregation     |
     | Context         |        | Context         |
     | (Supporting)    |        | (Supporting)    |
     +--------+--------+        +--------+--------+
              |                          |
              | [ACL]                    | [Shared Kernel: DeltaWindow]
              |                          |
              v                          v
     +-----------------+        +-----------------+
     | Delta           |<------>| Delta           |
     | Application     |   P/S  | Versioning      |
     | Context         |        | Context         |
     | (Core Domain)   |        | (Core Domain)   |
     +-----------------+        +-----------------+


LEGEND:
  [P/S]   = Partnership (bidirectional cooperation)
  [ACL]   = Anti-Corruption Layer
  [Published Language] = Shared vocabulary, immutable contracts
  [Shared Kernel] = Co-owned code/types

INTEGRATION PATTERNS:

| Upstream         | Downstream        | Pattern            | Shared Types                    |
|------------------|-------------------|--------------------|---------------------------------|
| Delta Capture    | Propagation       | Published Language | Delta, DeltaVector, DeltaId     |
| Delta Capture    | Aggregation       | Published Language | Delta, DeltaTimestamp           |
| Propagation      | Application       | ACL                | RoutingDecision -> ApplyRequest |
| Aggregation      | Application       | Shared Kernel      | AggregatedDelta                 |
| Aggregation      | Versioning        | Shared Kernel      | DeltaWindow, BatchId            |
| Application      | Versioning        | Partnership        | ApplicationResult <-> Version   |
```

---

## 5. Anti-Corruption Layers

### 5.1 Propagation to Application ACL

```rust
/// ACL: Translate propagation concepts to application domain
pub mod propagation_to_application_acl {
    use super::delta_propagation::*;
    use super::delta_application::*;
    use super::delta_capture::*;

    /// Adapter that translates routing decisions into application requests
    pub struct RoutingToApplicationAdapter;

    impl RoutingToApplicationAdapter {
        /// Convert a delivered delta into an application request
        pub fn to_apply_request(
            delta: &Delta,
            routing: &RoutingDecision,
            target_id: TargetId,
        ) -> ApplyRequest {
            ApplyRequest {
                delta: delta.clone(),
                target_id,
                priority: match routing.priority {
                    RoutingPriority::Critical => ApplicationPriority::Immediate,
                    RoutingPriority::High => ApplicationPriority::High,
                    RoutingPriority::Normal => ApplicationPriority::Normal,
                    RoutingPriority::Low => ApplicationPriority::Background,
                },
                timeout_ms: routing.ttl_ms,
                retry_policy: match routing.priority {
                    RoutingPriority::Critical => RetryPolicy::Infinite,
                    RoutingPriority::High => RetryPolicy::Count(5),
                    RoutingPriority::Normal => RetryPolicy::Count(3),
                    RoutingPriority::Low => RetryPolicy::None,
                },
            }
        }

        /// Map application result back to acknowledgment
        pub fn to_acknowledgment(
            result: &ApplicationResult,
            subscriber_id: SubscriberId,
        ) -> DeltaAcknowledgment {
            DeltaAcknowledgment {
                delta_id: result.delta_id,
                subscriber_id,
                success: matches!(result.status, ApplicationStatus::Applied),
                new_version: Some(result.new_state_hash),
            }
        }
    }

    #[derive(Clone, Debug)]
    pub struct ApplyRequest {
        pub delta: Delta,
        pub target_id: TargetId,
        pub priority: ApplicationPriority,
        pub timeout_ms: Option<u64>,
        pub retry_policy: RetryPolicy,
    }

    #[derive(Clone, Copy, Debug)]
    pub enum ApplicationPriority {
        Immediate,
        High,
        Normal,
        Background,
    }

    #[derive(Clone, Copy, Debug)]
    pub enum RetryPolicy {
        None,
        Count(u32),
        Infinite,
    }

    #[derive(Clone, Debug)]
    pub struct DeltaAcknowledgment {
        pub delta_id: DeltaId,
        pub subscriber_id: SubscriberId,
        pub success: bool,
        pub new_version: Option<[u8; 32]>,
    }
}
```

### 5.2 Aggregation to Versioning ACL

```rust
/// ACL: Translate aggregation windows to versioning streams
pub mod aggregation_to_versioning_acl {
    use super::delta_aggregation::*;
    use super::delta_versioning::*;
    use super::delta_capture::*;

    /// Adapter for converting aggregated deltas to stream entries
    pub struct AggregationToVersioningAdapter {
        snapshot_threshold: u32,
    }

    impl AggregationToVersioningAdapter {
        pub fn new(snapshot_threshold: u32) -> Self {
            Self { snapshot_threshold }
        }

        /// Convert a window's deltas into stream entries
        pub fn window_to_stream_entries(
            &self,
            window: &DeltaWindow,
            stream: &mut DeltaStream,
        ) -> Vec<Version> {
            let mut versions = Vec::new();

            for delta in &window.deltas {
                let version = stream.append(delta.clone());
                versions.push(version);
            }

            versions
        }

        /// Convert aggregated delta to single stream entry
        pub fn aggregated_to_stream_entry(
            &self,
            aggregated: &AggregatedDelta,
            stream: &mut DeltaStream,
            timestamp: DeltaTimestamp,
        ) -> (Version, bool) {
            // Create a synthetic delta from the aggregated vector
            let parent = stream.get(stream.head);

            let delta = Delta::new(
                aggregated.source_id.clone(),
                timestamp,
                parent,
                aggregated.composed_vector.clone(),
                std::collections::HashMap::from([
                    ("aggregated".to_string(), "true".to_string()),
                    ("delta_count".to_string(), aggregated.delta_count.to_string()),
                    ("compression_ratio".to_string(),
                     aggregated.compression_ratio.to_string()),
                ]),
            );

            let version = stream.append(delta);
            let should_snapshot = stream.should_snapshot();

            (version, should_snapshot)
        }

        /// Determine if window warrants a snapshot
        pub fn should_create_snapshot(
            &self,
            window: &DeltaWindow,
            current_version: Version,
        ) -> bool {
            window.metrics.delta_count >= self.snapshot_threshold ||
            window.metrics.total_magnitude > 10.0
        }
    }
}
```

---

## 6. Repository Interfaces

```rust
/// Repository interfaces for persistence
pub mod repositories {
    use super::delta_capture::*;
    use super::delta_versioning::*;
    use super::delta_aggregation::*;
    use async_trait::async_trait;

    // ============================================================
    // DELTA REPOSITORY
    // ============================================================

    #[async_trait]
    pub trait DeltaRepository: Send + Sync {
        /// Store a delta
        async fn save(&self, delta: &Delta) -> Result<(), RepositoryError>;

        /// Retrieve delta by ID
        async fn find_by_id(&self, id: &DeltaId) -> Result<Option<Delta>, RepositoryError>;

        /// Find deltas by source
        async fn find_by_source(
            &self,
            source_id: &SourceId,
            limit: usize,
            offset: usize,
        ) -> Result<Vec<Delta>, RepositoryError>;

        /// Find deltas in time range
        async fn find_by_time_range(
            &self,
            source_id: &SourceId,
            start_ms: u64,
            end_ms: u64,
        ) -> Result<Vec<Delta>, RepositoryError>;

        /// Find deltas by parent (for graph traversal)
        async fn find_children(&self, parent_id: &DeltaId) -> Result<Vec<Delta>, RepositoryError>;

        /// Verify checksum chain
        async fn verify_chain(
            &self,
            from_id: &DeltaId,
            to_id: &DeltaId,
        ) -> Result<bool, RepositoryError>;

        /// Bulk insert deltas
        async fn save_batch(&self, deltas: &[Delta]) -> Result<usize, RepositoryError>;

        /// Delete deltas older than version (for compaction)
        async fn delete_before(
            &self,
            source_id: &SourceId,
            before_timestamp: u64,
        ) -> Result<u64, RepositoryError>;
    }

    // ============================================================
    // SNAPSHOT REPOSITORY
    // ============================================================

    #[async_trait]
    pub trait SnapshotRepository: Send + Sync {
        /// Store a snapshot
        async fn save(&self, snapshot: &DeltaSnapshot) -> Result<(), RepositoryError>;

        /// Retrieve snapshot by ID
        async fn find_by_id(&self, id: &SnapshotId) -> Result<Option<DeltaSnapshot>, RepositoryError>;

        /// Find nearest snapshot before version
        async fn find_nearest(
            &self,
            source_id: &SourceId,
            before_version: Version,
        ) -> Result<Option<DeltaSnapshot>, RepositoryError>;

        /// List snapshots for source
        async fn list_by_source(
            &self,
            source_id: &SourceId,
        ) -> Result<Vec<DeltaSnapshot>, RepositoryError>;

        /// Delete old snapshots (keep N most recent)
        async fn cleanup(
            &self,
            source_id: &SourceId,
            keep_count: usize,
        ) -> Result<u64, RepositoryError>;
    }

    // ============================================================
    // STREAM REPOSITORY
    // ============================================================

    #[async_trait]
    pub trait StreamRepository: Send + Sync {
        /// Get or create stream
        async fn get_or_create(
            &self,
            source_id: &SourceId,
        ) -> Result<DeltaStream, RepositoryError>;

        /// Save stream metadata
        async fn save_metadata(
            &self,
            stream: &DeltaStream,
        ) -> Result<(), RepositoryError>;

        /// Append delta to stream
        async fn append(
            &self,
            stream_id: &StreamId,
            delta: &Delta,
        ) -> Result<Version, RepositoryError>;

        /// Get deltas in version range
        async fn get_range(
            &self,
            stream_id: &StreamId,
            from: Version,
            to: Version,
        ) -> Result<Vec<Delta>, RepositoryError>;

        /// Get current head version
        async fn get_head(&self, stream_id: &StreamId) -> Result<Version, RepositoryError>;
    }

    // ============================================================
    // INDEX REPOSITORY (for search)
    // ============================================================

    #[async_trait]
    pub trait IndexRepository: Send + Sync {
        /// Index a delta
        async fn index(&self, delta: &Delta) -> Result<(), RepositoryError>;

        /// Search by checksum
        async fn search_by_checksum(
            &self,
            checksum: &DeltaChecksum,
        ) -> Result<Option<DeltaId>, RepositoryError>;

        /// Search by metadata
        async fn search_by_metadata(
            &self,
            key: &str,
            value: &str,
        ) -> Result<Vec<DeltaId>, RepositoryError>;

        /// Full-text search in metadata
        async fn search_text(
            &self,
            query: &str,
            limit: usize,
        ) -> Result<Vec<DeltaId>, RepositoryError>;
    }

    // ============================================================
    // ERROR TYPES
    // ============================================================

    #[derive(Debug)]
    pub enum RepositoryError {
        NotFound(String),
        Conflict(String),
        ConnectionError(String),
        SerializationError(String),
        IntegrityError(String),
        StorageFull,
        Timeout,
    }
}
```

---

## 7. Event Flow Diagram

```
                           DELTA-BEHAVIOR EVENT FLOW
+----------------------------------------------------------------------------+
|                                                                              |
|   Source State Change                                                        |
|         |                                                                    |
|         v                                                                    |
|   +-------------+                                                            |
|   |  Observer   |-----> [ChangeDetected]                                    |
|   +------+------+                                                            |
|          |                                                                   |
|          v                                                                   |
|   +-------------+                                                            |
|   |  Extractor  |-----> [DeltaExtracted]                                    |
|   +------+------+                                                            |
|          |                                                                   |
|          | Delta                                                             |
|          v                                                                   |
|   +-------------+        +---------------+                                   |
|   |   Router    |------->| Channel Pub   |-----> [DeltaRouted]              |
|   +------+------+        +-------+-------+                                   |
|          |                       |                                           |
|          |                       v                                           |
|          |               +---------------+                                   |
|          |               |  Subscriber   |-----> [DeltaDelivered]           |
|          |               +-------+-------+                                   |
|          |                       |                                           |
|          v                       |                                           |
|   +-------------+                |                                           |
|   |   Window    |                |                                           |
|   | Aggregator  |-----> [WindowClosed, WindowCompacted]                     |
|   +------+------+                |                                           |
|          |                       |                                           |
|          | AggregatedDelta       | Delta                                     |
|          |                       |                                           |
|          v                       v                                           |
|   +-------------+         +-------------+                                    |
|   |  Versioning |<------->| Application |                                    |
|   |   Stream    |         |   Target    |                                    |
|   +------+------+         +------+------+                                    |
|          |                       |                                           |
|          |                       |                                           |
|          v                       v                                           |
|   [DeltaVersioned]        [DeltaApplied]                                    |
|   [SnapshotCreated]       [DeltaRolledBack]                                 |
|   [BranchCreated]         [DeltaConflictDetected]                           |
|   [BranchMerged]                                                             |
|                                                                              |
+----------------------------------------------------------------------------+
```

---

## 8. WASM Integration Architecture

```rust
/// WASM bindings for Delta-Behavior system
pub mod wasm_bindings {
    use wasm_bindgen::prelude::*;
    use serde::{Deserialize, Serialize};

    // ============================================================
    // WASM-OPTIMIZED TYPES (minimal footprint)
    // ============================================================

    /// Compact delta for WASM (minimized size)
    #[wasm_bindgen]
    #[derive(Clone)]
    pub struct WasmDelta {
        id_high: u64,
        id_low: u64,
        parent_high: u64,
        parent_low: u64,
        timestamp: u64,
        indices: Vec<u32>,
        values: Vec<f32>,
    }

    #[wasm_bindgen]
    impl WasmDelta {
        #[wasm_bindgen(constructor)]
        pub fn new(timestamp: u64, indices: Vec<u32>, values: Vec<f32>) -> Self {
            let id = uuid::Uuid::new_v4().as_u128();
            Self {
                id_high: (id >> 64) as u64,
                id_low: id as u64,
                parent_high: 0,
                parent_low: 0,
                timestamp,
                indices,
                values,
            }
        }

        /// Set parent delta ID
        pub fn set_parent(&mut self, high: u64, low: u64) {
            self.parent_high = high;
            self.parent_low = low;
        }

        /// Get delta ID as hex string
        pub fn id_hex(&self) -> String {
            format!("{:016x}{:016x}", self.id_high, self.id_low)
        }

        /// Apply to a vector (in-place)
        pub fn apply(&self, vector: &mut [f32]) {
            for (&idx, &val) in self.indices.iter().zip(self.values.iter()) {
                if (idx as usize) < vector.len() {
                    vector[idx as usize] += val;
                }
            }
        }

        /// Compute L2 magnitude
        pub fn magnitude(&self) -> f32 {
            self.values.iter().map(|v| v * v).sum::<f32>().sqrt()
        }

        /// Serialize to bytes
        pub fn to_bytes(&self) -> Vec<u8> {
            let mut bytes = Vec::with_capacity(
                16 + 16 + 8 + 4 + self.indices.len() * 4 + self.values.len() * 4
            );

            bytes.extend(&self.id_high.to_le_bytes());
            bytes.extend(&self.id_low.to_le_bytes());
            bytes.extend(&self.parent_high.to_le_bytes());
            bytes.extend(&self.parent_low.to_le_bytes());
            bytes.extend(&self.timestamp.to_le_bytes());
            bytes.extend(&(self.indices.len() as u32).to_le_bytes());

            for &idx in &self.indices {
                bytes.extend(&idx.to_le_bytes());
            }
            for &val in &self.values {
                bytes.extend(&val.to_le_bytes());
            }

            bytes
        }

        /// Deserialize from bytes
        pub fn from_bytes(bytes: &[u8]) -> Result<WasmDelta, JsValue> {
            if bytes.len() < 44 {
                return Err(JsValue::from_str("Buffer too small"));
            }

            let id_high = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
            let id_low = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
            let parent_high = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
            let parent_low = u64::from_le_bytes(bytes[24..32].try_into().unwrap());
            let timestamp = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
            let count = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;

            let expected_len = 44 + count * 8;
            if bytes.len() < expected_len {
                return Err(JsValue::from_str("Buffer too small for data"));
            }

            let mut indices = Vec::with_capacity(count);
            let mut values = Vec::with_capacity(count);

            let mut offset = 44;
            for _ in 0..count {
                indices.push(u32::from_le_bytes(bytes[offset..offset+4].try_into().unwrap()));
                offset += 4;
            }
            for _ in 0..count {
                values.push(f32::from_le_bytes(bytes[offset..offset+4].try_into().unwrap()));
                offset += 4;
            }

            Ok(Self {
                id_high,
                id_low,
                parent_high,
                parent_low,
                timestamp,
                indices,
                values,
            })
        }
    }

    // ============================================================
    // WASM DELTA STREAM
    // ============================================================

    #[wasm_bindgen]
    pub struct WasmDeltaStream {
        deltas: Vec<WasmDelta>,
        head_idx: Option<usize>,
    }

    #[wasm_bindgen]
    impl WasmDeltaStream {
        #[wasm_bindgen(constructor)]
        pub fn new() -> Self {
            Self {
                deltas: Vec::new(),
                head_idx: None,
            }
        }

        /// Append delta to stream
        pub fn append(&mut self, mut delta: WasmDelta) -> usize {
            // Set parent to current head
            if let Some(head) = self.head_idx {
                let parent = &self.deltas[head];
                delta.set_parent(parent.id_high, parent.id_low);
            }

            let idx = self.deltas.len();
            self.deltas.push(delta);
            self.head_idx = Some(idx);
            idx
        }

        /// Get delta count
        pub fn len(&self) -> usize {
            self.deltas.len()
        }

        /// Apply all deltas to a vector
        pub fn apply_all(&self, vector: &mut [f32]) {
            for delta in &self.deltas {
                delta.apply(vector);
            }
        }

        /// Apply deltas from index to head
        pub fn apply_from(&self, start_idx: usize, vector: &mut [f32]) {
            for delta in self.deltas.iter().skip(start_idx) {
                delta.apply(vector);
            }
        }

        /// Compact stream by composing deltas
        pub fn compact(&mut self) -> WasmDelta {
            let mut indices_map = std::collections::HashMap::new();

            for delta in &self.deltas {
                for (&idx, &val) in delta.indices.iter().zip(delta.values.iter()) {
                    *indices_map.entry(idx).or_insert(0.0f32) += val;
                }
            }

            let mut sorted: Vec<_> = indices_map.into_iter().collect();
            sorted.sort_by_key(|(idx, _)| *idx);

            let indices: Vec<u32> = sorted.iter().map(|(i, _)| *i).collect();
            let values: Vec<f32> = sorted.iter().map(|(_, v)| *v).collect();

            let timestamp = self.deltas.last()
                .map(|d| d.timestamp)
                .unwrap_or(0);

            WasmDelta::new(timestamp, indices, values)
        }

        /// Serialize entire stream
        pub fn to_bytes(&self) -> Vec<u8> {
            let mut bytes = Vec::new();
            bytes.extend(&(self.deltas.len() as u32).to_le_bytes());

            for delta in &self.deltas {
                let delta_bytes = delta.to_bytes();
                bytes.extend(&(delta_bytes.len() as u32).to_le_bytes());
                bytes.extend(&delta_bytes);
            }

            bytes
        }
    }

    // ============================================================
    // WASM DELTA DETECTOR (Change detection in WASM)
    // ============================================================

    #[wasm_bindgen]
    pub struct WasmDeltaDetector {
        threshold: f32,
        min_component_diff: f32,
        last_state: Option<Vec<f32>>,
    }

    #[wasm_bindgen]
    impl WasmDeltaDetector {
        #[wasm_bindgen(constructor)]
        pub fn new(threshold: f32, min_component_diff: f32) -> Self {
            Self {
                threshold,
                min_component_diff,
                last_state: None,
            }
        }

        /// Detect delta between last state and new state
        pub fn detect(&mut self, new_state: Vec<f32>) -> Option<WasmDelta> {
            let delta = match &self.last_state {
                Some(old) => {
                    if old.len() != new_state.len() {
                        return None;
                    }

                    let mut indices = Vec::new();
                    let mut values = Vec::new();
                    let mut magnitude_sq = 0.0f32;

                    for (i, (&old_val, &new_val)) in old.iter().zip(new_state.iter()).enumerate() {
                        let diff = new_val - old_val;
                        if diff.abs() > self.min_component_diff {
                            indices.push(i as u32);
                            values.push(diff);
                            magnitude_sq += diff * diff;
                        }
                    }

                    let magnitude = magnitude_sq.sqrt();
                    if magnitude < self.threshold {
                        None
                    } else {
                        let timestamp = js_sys::Date::now() as u64;
                        Some(WasmDelta::new(timestamp, indices, values))
                    }
                }
                None => None,
            };

            self.last_state = Some(new_state);
            delta
        }

        /// Reset detector state
        pub fn reset(&mut self) {
            self.last_state = None;
        }
    }
}
```

---

## 9. Technology Evaluation Matrix

| Component | Option A | Option B | Recommendation | Rationale |
|-----------|----------|----------|----------------|-----------|
| **Delta Storage** | PostgreSQL + JSONB | RuVector + Append Log | RuVector | Native vector support, HNSW for similarity |
| **Checksum Chain** | SHA-256 | Blake3 | Blake3 | 3x faster, streaming support |
| **Serialization** | JSON | Bincode | Bincode (WASM), JSON (API) | Size: 60% smaller, speed: 10x faster |
| **Timestamp** | Wall Clock | Hybrid Logical | Hybrid Logical | Causality without clock sync |
| **Conflict Resolution** | LWW | Vector Clocks | Vector Clocks | Concurrent detection |
| **WASM Runtime** | wasm-bindgen | wit-bindgen | wasm-bindgen | Mature, browser-compatible |
| **Pub/Sub** | Redis Streams | NATS | NATS (prod), in-process (embed) | Persistence + at-least-once |
| **Graph Storage** | Neo4j | ruvector-graph | ruvector-graph | Native integration |

---

## 10. Consequences

### Benefits

1. **Incremental Efficiency**: Only transmit/store actual changes (typically 1-5% of full vector)
2. **Temporal Queries**: Reconstruct any historical state via delta replay
3. **Conflict Visibility**: Concurrent modifications explicitly tracked and resolved
4. **Audit Trail**: Complete, tamper-evident history of all changes
5. **WASM Portability**: Core delta logic runs anywhere (browser, edge, server)
6. **Composability**: Deltas can be merged, compacted, or branched
7. **Clear Boundaries**: Each domain has explicit responsibilities

### Risks and Mitigations

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Replay performance at scale | High | Medium | Periodic snapshots (every N deltas) |
| Checksum chain corruption | High | Low | Redundant storage, verification on read |
| Window aggregation data loss | Medium | Low | WAL before window close |
| Branch merge conflicts | Medium | Medium | Clear resolution strategies per domain |
| WASM memory limits | Medium | Medium | Streaming delta application |

### Trade-offs

1. **Storage vs Replay Speed**: More frequent snapshots = faster replay, more storage
2. **Granularity vs Overhead**: Fine-grained deltas = better precision, more metadata overhead
3. **Compression vs Latency**: Window compaction = smaller storage, delayed visibility
4. **Consistency vs Availability**: Strict ordering = stronger guarantees, potential blocking

---

## 11. Implementation Roadmap

### Phase 1: Core Domain (4 weeks)
- [ ] Delta Capture domain implementation
- [ ] Value objects: DeltaId, DeltaTimestamp, DeltaVector
- [ ] WASM bindings for core types
- [ ] Unit tests for delta composition/inversion

### Phase 2: Propagation + Aggregation (3 weeks)
- [ ] Channel/Subscriber infrastructure
- [ ] Window aggregation with policies
- [ ] Routing rules engine
- [ ] Integration tests

### Phase 3: Application + Versioning (4 weeks)
- [ ] Delta application with validation
- [ ] Rollback support
- [ ] Version stream management
- [ ] Snapshot creation/restoration
- [ ] Branch/merge support

### Phase 4: Repositories + Integration (3 weeks)
- [ ] PostgreSQL repository implementations
- [ ] RuVector index integration
- [ ] NATS pub/sub integration
- [ ] End-to-end tests

### Phase 5: Production Hardening (2 weeks)
- [ ] Performance benchmarks
- [ ] WASM size optimization
- [ ] Monitoring/metrics
- [ ] Documentation

---

## 12. References

- Evans, Eric. "Domain-Driven Design: Tackling Complexity in the Heart of Software" (2003)
- Vernon, Vaughn. "Implementing Domain-Driven Design" (2013)
- Kleppmann, Martin. "Designing Data-Intensive Applications" (2017) - Chapter 5: Replication
- RuVector Core: `/workspaces/ruvector/crates/ruvector-core`
- RuVector DAG: `/workspaces/ruvector/crates/ruvector-dag`
- RuVector Replication: `/workspaces/ruvector/crates/ruvector-replication/src/conflict.rs`
- ADR-CE-004: Signed Event Log

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-28 | System Architecture Designer | Initial ADR |
