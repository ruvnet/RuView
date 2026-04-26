# Domain Events

This document catalogs all domain events in the WiFi-DensePose system. Domain events represent significant occurrences within the domain that other parts of the system may need to react to.

---

## Event Design Principles

### Event Structure

All domain events follow a consistent structure:

```rust
/// Base trait for all domain events
pub trait DomainEvent: Send + Sync + 'static {
    /// Unique event type identifier
    fn event_type(&self) -> &'static str;

    /// When the event occurred
    fn occurred_at(&self) -> DateTime<Utc>;

    /// Correlation ID for tracing
    fn correlation_id(&self) -> Option<Uuid>;

    /// Aggregate ID that produced the event
    fn aggregate_id(&self) -> String;

    /// Event schema version for evolution
    fn version(&self) -> u32 { 1 }
}

/// Event envelope for serialization and transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope<E: DomainEvent> {
    pub id: Uuid,
    pub event_type: String,
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub sequence_number: u64,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Option<Uuid>,
    pub causation_id: Option<Uuid>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub payload: E,
}
```

### Event Naming Conventions

- Use past tense: `CsiFrameReceived`, not `ReceiveCsiFrame`
- Include aggregate name: `Device` + `Connected` = `DeviceConnected`
- Be specific: `FallDetected`, not `AlertRaised`

---

## Signal Domain Events

### CsiFrameReceived

Emitted when raw CSI data is received from hardware.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsiFrameReceived {
    /// Unique frame identifier
    pub frame_id: FrameId,

    /// Source device
    pub device_id: DeviceId,

    /// Associated session (if any)
    pub session_id: Option<SessionId>,

    /// Frame sequence number
    pub sequence_number: u64,

    /// Reception timestamp
    pub timestamp: DateTime<Utc>,

    /// Frame dimensions
    pub num_subcarriers: u16,
    pub num_antennas: u8,

    /// Signal quality
    pub snr_db: f64,

    /// Raw data size in bytes
    pub payload_size: usize,
}

impl DomainEvent for CsiFrameReceived {
    fn event_type(&self) -> &'static str { "signal.csi_frame_received" }
    fn occurred_at(&self) -> DateTime<Utc> { self.timestamp }
    fn correlation_id(&self) -> Option<Uuid> { self.session_id.map(|s| s.0) }
    fn aggregate_id(&self) -> String { self.frame_id.0.to_string() }
}
```

**Producers:** Hardware Domain (CSI Extractor)
**Consumers:** Signal Domain (Preprocessor), Storage Domain (if persistence enabled)

---

### CsiFrameValidated

Emitted when a CSI frame passes integrity validation.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsiFrameValidated {
    pub frame_id: FrameId,
    pub device_id: DeviceId,
    pub timestamp: DateTime<Utc>,

    /// Validation results
    pub quality_score: f32,
    pub is_complete: bool,
    pub validation_time_us: u64,

    /// Detected issues (if any)
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    pub code: String,
    pub message: String,
    pub severity: WarningSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WarningSeverity {
    Info,
    Warning,
    Error,
}
```

**Producers:** Signal Domain (Validator)
**Consumers:** Signal Domain (Preprocessor)

---

### SignalProcessed

Emitted when CSI features have been extracted and signal is ready for inference.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalProcessed {
    /// Processed signal identifier
    pub signal_id: SignalId,

    /// Source frame(s)
    pub source_frames: Vec<FrameId>,

    /// Source device
    pub device_id: DeviceId,

    /// Associated session
    pub session_id: Option<SessionId>,

    /// Processing timestamp
    pub timestamp: DateTime<Utc>,

    /// Processing window
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,

    /// Feature summary (not full data)
    pub feature_summary: FeatureSummary,

    /// Human presence detection
    pub human_detected: bool,
    pub presence_confidence: f32,
    pub estimated_person_count: Option<u8>,

    /// Quality metrics
    pub quality_score: f32,

    /// Processing performance
    pub processing_time_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSummary {
    pub amplitude_mean: f32,
    pub amplitude_std: f32,
    pub phase_variance: f32,
    pub dominant_frequency_hz: f32,
    pub motion_indicator: f32,
}
```

**Producers:** Signal Domain (Feature Extractor)
**Consumers:** Pose Domain (Inference Engine), Streaming Domain (if CSI streaming enabled)

---

### SignalProcessingFailed

Emitted when signal processing fails.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalProcessingFailed {
    pub frame_id: FrameId,
    pub device_id: DeviceId,
    pub timestamp: DateTime<Utc>,

    /// Error details
    pub error_code: String,
    pub error_message: String,
    pub error_category: ProcessingErrorCategory,

    /// Recovery suggestion
    pub recoverable: bool,
    pub suggested_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingErrorCategory {
    InvalidData,
    InsufficientQuality,
    CalibrationRequired,
    ResourceExhausted,
    InternalError,
}
```

**Producers:** Signal Domain
**Consumers:** Monitoring, Alerting

---

## Pose Domain Events

### PoseEstimated

Emitted when pose inference completes successfully.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseEstimated {
    /// Estimate identifier
    pub estimate_id: EstimateId,

    /// Source signal
    pub signal_id: SignalId,

    /// Session context
    pub session_id: SessionId,

    /// Zone (if applicable)
    pub zone_id: Option<ZoneId>,

    /// Estimation timestamp
    pub timestamp: DateTime<Utc>,

    /// Frame number in session
    pub frame_number: u64,

    /// Detection results summary
    pub person_count: u8,
    pub persons: Vec<PersonSummary>,

    /// Confidence metrics
    pub overall_confidence: f32,

    /// Processing performance
    pub processing_time_ms: f64,
    pub model_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonSummary {
    pub person_id: PersonId,
    pub bounding_box: BoundingBoxDto,
    pub confidence: f32,
    pub activity: String,
    pub keypoint_count: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBoxDto {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
```

**Producers:** Pose Domain (Inference Engine)
**Consumers:** Streaming Domain, Storage Domain, Monitoring

---

### PersonDetected

Emitted when a new person enters the detection zone.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonDetected {
    /// Person identifier (tracking ID)
    pub person_id: PersonId,

    /// Detection context
    pub session_id: SessionId,
    pub zone_id: Option<ZoneId>,
    pub estimate_id: EstimateId,

    /// Detection details
    pub timestamp: DateTime<Utc>,
    pub confidence: f32,
    pub bounding_box: BoundingBoxDto,

    /// Initial activity classification
    pub initial_activity: String,

    /// Entry point (if trackable)
    pub entry_position: Option<Position2DDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position2DDto {
    pub x: f32,
    pub y: f32,
}
```

**Producers:** Pose Domain (Tracker)
**Consumers:** Streaming Domain, Analytics, Alerting

---

### PersonLost

Emitted when a tracked person leaves the detection zone.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonLost {
    /// Person identifier
    pub person_id: PersonId,

    /// Context
    pub session_id: SessionId,
    pub zone_id: Option<ZoneId>,

    /// Timing
    pub timestamp: DateTime<Utc>,
    pub first_seen: DateTime<Utc>,
    pub duration_seconds: f64,

    /// Exit details
    pub last_position: Option<Position2DDto>,
    pub last_activity: String,

    /// Tracking statistics
    pub total_frames_tracked: u64,
    pub average_confidence: f32,
}
```

**Producers:** Pose Domain (Tracker)
**Consumers:** Streaming Domain, Analytics

---

### ActivityChanged

Emitted when a person's classified activity changes.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityChanged {
    pub person_id: PersonId,
    pub session_id: SessionId,
    pub zone_id: Option<ZoneId>,
    pub timestamp: DateTime<Utc>,

    /// Activity transition
    pub previous_activity: String,
    pub new_activity: String,

    /// Confidence in new classification
    pub confidence: f32,

    /// Duration of previous activity
    pub previous_activity_duration_seconds: f64,
}
```

**Producers:** Pose Domain (Activity Classifier)
**Consumers:** Streaming Domain, Analytics, Alerting (for certain transitions)

---

### MotionDetected

Emitted when significant motion is detected in a zone.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionDetected {
    /// Event identification
    pub event_id: Uuid,

    /// Context
    pub session_id: Option<SessionId>,
    pub zone_id: Option<ZoneId>,
    pub device_id: DeviceId,

    /// Detection details
    pub timestamp: DateTime<Utc>,
    pub motion_score: f32,
    pub motion_type: MotionType,

    /// Associated persons (if identifiable)
    pub person_ids: Vec<PersonId>,
    pub person_count: u8,

    /// Motion characteristics
    pub velocity_estimate: Option<f32>,
    pub direction: Option<f32>, // Angle in radians
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MotionType {
    /// General movement
    General,
    /// Walking motion pattern
    Walking,
    /// Running motion pattern
    Running,
    /// Sudden/rapid motion
    Sudden,
    /// Repetitive motion
    Repetitive,
}
```

**Producers:** Pose Domain, Signal Domain (for CSI-based motion)
**Consumers:** Streaming Domain, Alerting, Analytics

---

### FallDetected

Emitted when a potential fall event is detected. This is a critical alert event.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallDetected {
    /// Event identification
    pub event_id: Uuid,

    /// Person involved
    pub person_id: PersonId,

    /// Context
    pub session_id: SessionId,
    pub zone_id: Option<ZoneId>,

    /// Detection details
    pub timestamp: DateTime<Utc>,
    pub confidence: f32,

    /// Fall characteristics
    pub fall_type: FallType,
    pub duration_ms: Option<u64>,
    pub impact_severity: ImpactSeverity,

    /// Position information
    pub fall_location: Option<Position2DDto>,
    pub pre_fall_activity: String,

    /// Verification status
    pub requires_verification: bool,
    pub auto_alert_sent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FallType {
    /// Forward fall
    Forward,
    /// Backward fall
    Backward,
    /// Sideways fall
    Lateral,
    /// Gradual lowering (sitting/lying)
    Gradual,
    /// Unknown pattern
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImpactSeverity {
    Low,
    Medium,
    High,
    Critical,
}
```

**Producers:** Pose Domain (Fall Detector)
**Consumers:** Alerting (high priority), Streaming Domain, Storage Domain

---

## Streaming Domain Events

### SessionStarted

Emitted when a client establishes a streaming session.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStarted {
    pub session_id: SessionId,
    pub client_id: ClientId,
    pub timestamp: DateTime<Utc>,

    /// Connection details
    pub stream_type: String,
    pub remote_addr: Option<String>,
    pub user_agent: Option<String>,

    /// Initial subscription
    pub zone_subscriptions: Vec<String>,
    pub filters: SubscriptionFiltersDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionFiltersDto {
    pub min_confidence: Option<f32>,
    pub max_persons: Option<u8>,
    pub include_keypoints: bool,
    pub include_segmentation: bool,
    pub throttle_ms: Option<u32>,
}
```

**Producers:** Streaming Domain (Connection Manager)
**Consumers:** Monitoring, Analytics

---

### SessionEnded

Emitted when a streaming session terminates.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEnded {
    pub session_id: SessionId,
    pub client_id: ClientId,
    pub timestamp: DateTime<Utc>,

    /// Session duration
    pub started_at: DateTime<Utc>,
    pub duration_seconds: f64,

    /// Termination reason
    pub reason: SessionEndReason,
    pub error_message: Option<String>,

    /// Session statistics
    pub messages_sent: u64,
    pub messages_failed: u64,
    pub bytes_sent: u64,
    pub average_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionEndReason {
    ClientDisconnect,
    ServerShutdown,
    Timeout,
    Error,
    Evicted,
}
```

**Producers:** Streaming Domain (Connection Manager)
**Consumers:** Monitoring, Analytics

---

### SubscriptionUpdated

Emitted when a client changes their subscription filters.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionUpdated {
    pub session_id: SessionId,
    pub timestamp: DateTime<Utc>,

    /// Old filters
    pub previous_filters: SubscriptionFiltersDto,

    /// New filters
    pub new_filters: SubscriptionFiltersDto,

    /// Zone changes
    pub zones_added: Vec<String>,
    pub zones_removed: Vec<String>,
}
```

**Producers:** Streaming Domain
**Consumers:** Monitoring

---

### MessageDelivered

Emitted for tracking message delivery (optional, high-volume).

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDelivered {
    pub session_id: SessionId,
    pub message_id: Uuid,
    pub timestamp: DateTime<Utc>,

    pub message_type: String,
    pub payload_bytes: usize,
    pub latency_ms: f64,
}
```

**Producers:** Streaming Domain
**Consumers:** Metrics Collector

---

### MessageDeliveryFailed

Emitted when message delivery fails.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDeliveryFailed {
    pub session_id: SessionId,
    pub message_id: Uuid,
    pub timestamp: DateTime<Utc>,

    pub message_type: String,
    pub error_code: String,
    pub error_message: String,
    pub retry_count: u8,
    pub will_retry: bool,
}
```

**Producers:** Streaming Domain
**Consumers:** Monitoring, Alerting

---

## Hardware Domain Events

### DeviceDiscovered

Emitted when a new device is found on the network.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceDiscovered {
    pub discovery_id: Uuid,
    pub timestamp: DateTime<Utc>,

    /// Device identification
    pub mac_address: String,
    pub ip_address: Option<String>,
    pub device_type: String,

    /// Discovered capabilities
    pub capabilities: DeviceCapabilitiesDto,

    /// Firmware info
    pub firmware_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilitiesDto {
    pub max_subcarriers: u16,
    pub max_antennas: u8,
    pub supported_bandwidths: Vec<String>,
    pub max_sampling_rate_hz: u32,
}
```

**Producers:** Hardware Domain (Discovery Service)
**Consumers:** Device Management UI, Auto-Configuration

---

### DeviceConnected

Emitted when connection to a device is established.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConnected {
    pub device_id: DeviceId,
    pub timestamp: DateTime<Utc>,

    /// Connection details
    pub ip_address: String,
    pub protocol: String,
    pub connection_time_ms: u64,

    /// Device state
    pub firmware_version: Option<String>,
    pub current_config: DeviceConfigDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfigDto {
    pub sampling_rate_hz: u32,
    pub subcarriers: u16,
    pub antennas: u8,
    pub bandwidth: String,
    pub channel: u8,
}
```

**Producers:** Hardware Domain (Device Connector)
**Consumers:** Signal Domain, Monitoring

---

### DeviceDisconnected

Emitted when connection to a device is lost.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceDisconnected {
    pub device_id: DeviceId,
    pub timestamp: DateTime<Utc>,

    /// Disconnection details
    pub reason: DisconnectReason,
    pub error_message: Option<String>,

    /// Session statistics
    pub connected_since: DateTime<Utc>,
    pub uptime_seconds: f64,
    pub frames_transmitted: u64,
    pub errors_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisconnectReason {
    Graceful,
    ConnectionLost,
    Timeout,
    Error,
    MaintenanceMode,
}
```

**Producers:** Hardware Domain
**Consumers:** Signal Domain, Alerting, Monitoring

---

### DeviceConfigured

Emitted when device configuration is applied.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfigured {
    pub device_id: DeviceId,
    pub timestamp: DateTime<Utc>,

    /// Configuration applied
    pub config: DeviceConfigDto,

    /// Previous configuration
    pub previous_config: Option<DeviceConfigDto>,

    /// Configuration source
    pub source: ConfigurationSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigurationSource {
    Api,
    AutoConfig,
    Calibration,
    Default,
}
```

**Producers:** Hardware Domain (Configurator)
**Consumers:** Monitoring

---

### DeviceCalibrated

Emitted when device calibration completes.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCalibrated {
    pub device_id: DeviceId,
    pub calibration_id: Uuid,
    pub timestamp: DateTime<Utc>,

    /// Calibration results
    pub success: bool,
    pub calibration_type: String,
    pub duration_seconds: f64,

    /// Calibration parameters
    pub noise_floor_db: f64,
    pub antenna_offsets: Vec<f64>,
    pub phase_correction: Vec<f64>,

    /// Quality metrics
    pub quality_before: f32,
    pub quality_after: f32,
    pub improvement_percent: f32,
}
```

**Producers:** Hardware Domain (Calibration Service)
**Consumers:** Signal Domain, Monitoring

---

### DeviceHealthChanged

Emitted when device health status changes.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceHealthChanged {
    pub device_id: DeviceId,
    pub timestamp: DateTime<Utc>,

    /// Health transition
    pub previous_status: String,
    pub new_status: String,

    /// Health metrics
    pub cpu_usage_percent: Option<f32>,
    pub memory_usage_percent: Option<f32>,
    pub temperature_celsius: Option<f32>,
    pub error_rate: Option<f32>,

    /// Consecutive failures
    pub failure_count: u8,

    /// Recommended action
    pub recommended_action: Option<String>,
}
```

**Producers:** Hardware Domain (Health Monitor)
**Consumers:** Alerting, Monitoring

---

### DeviceError

Emitted when a device encounters an error condition.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceError {
    pub device_id: DeviceId,
    pub timestamp: DateTime<Utc>,

    /// Error details
    pub error_code: String,
    pub error_message: String,
    pub error_category: DeviceErrorCategory,

    /// Context
    pub operation: String,
    pub stack_trace: Option<String>,

    /// Recovery
    pub recoverable: bool,
    pub retry_after_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceErrorCategory {
    Connection,
    Configuration,
    Hardware,
    Firmware,
    Protocol,
    Resource,
    Unknown,
}
```

**Producers:** Hardware Domain
**Consumers:** Alerting, Monitoring, Auto-Recovery

---

## Event Flow Diagrams

### CSI to Pose Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           EVENT FLOW: CSI TO POSE                           │
└─────────────────────────────────────────────────────────────────────────────┘

  Hardware          Signal Domain         Pose Domain         Streaming
  ─────────         ─────────────         ───────────         ─────────

     │                    │                    │                   │
     │ CsiFrameReceived   │                    │                   │
     │───────────────────>│                    │                   │
     │                    │                    │                   │
     │                    │ CsiFrameValidated  │                   │
     │                    │─────────┐          │                   │
     │                    │         │          │                   │
     │                    │<────────┘          │                   │
     │                    │                    │                   │
     │                    │ SignalProcessed    │                   │
     │                    │───────────────────>│                   │
     │                    │                    │                   │
     │                    │                    │ PoseEstimated     │
     │                    │                    │──────────────────>│
     │                    │                    │                   │
     │                    │                    │ [if detected]     │
     │                    │                    │                   │
     │                    │                    │ MotionDetected    │
     │                    │                    │──────────────────>│
     │                    │                    │                   │
     │                    │                    │ FallDetected      │
     │                    │                    │──────────────────>│
     │                    │                    │                   │
```

### Session Lifecycle

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        EVENT FLOW: SESSION LIFECYCLE                        │
└─────────────────────────────────────────────────────────────────────────────┘

  Client              Streaming Domain              Pose Domain
  ──────              ────────────────              ───────────

     │                       │                           │
     │  WebSocket Connect    │                           │
     │──────────────────────>│                           │
     │                       │                           │
     │                       │ SessionStarted            │
     │                       │───────────┐               │
     │                       │           │               │
     │                       │<──────────┘               │
     │                       │                           │
     │  Subscribe to zones   │                           │
     │──────────────────────>│                           │
     │                       │                           │
     │                       │ SubscriptionUpdated       │
     │                       │───────────┐               │
     │                       │           │               │
     │                       │<──────────┘               │
     │                       │                           │
     │                       │          PoseEstimated    │
     │                       │<──────────────────────────│
     │                       │                           │
     │  Pose data            │                           │
     │<──────────────────────│                           │
     │                       │                           │
     │  Disconnect           │                           │
     │──────────────────────>│                           │
     │                       │                           │
     │                       │ SessionEnded              │
     │                       │───────────┐               │
     │                       │           │               │
     │                       │<──────────┘               │
```

---

## Event Bus Implementation

### Event Publisher

```rust
/// Trait for publishing domain events
#[async_trait]
pub trait EventPublisher: Send + Sync {
    /// Publish a single event
    async fn publish<E: DomainEvent + Serialize>(&self, event: E) -> Result<(), EventError>;

    /// Publish multiple events atomically
    async fn publish_batch<E: DomainEvent + Serialize>(&self, events: Vec<E>) -> Result<(), EventError>;
}

/// In-memory event bus for development
pub struct InMemoryEventBus {
    subscribers: RwLock<HashMap<String, Vec<Box<dyn EventHandler>>>>,
}

/// Redis-based event bus for production
pub struct RedisEventBus {
    client: redis::Client,
    stream_name: String,
}

/// Kafka-based event bus for high-throughput
pub struct KafkaEventBus {
    producer: FutureProducer,
    topic_prefix: String,
}
```

### Event Handler

```rust
/// Trait for handling domain events
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Event types this handler is interested in
    fn event_types(&self) -> Vec<&'static str>;

    /// Handle an event
    async fn handle(&self, event: EventEnvelope<serde_json::Value>) -> Result<(), EventError>;
}

/// Example handler for fall detection alerts
pub struct FallAlertHandler {
    notifier: Arc<dyn AlertNotifier>,
}

#[async_trait]
impl EventHandler for FallAlertHandler {
    fn event_types(&self) -> Vec<&'static str> {
        vec!["pose.fall_detected"]
    }

    async fn handle(&self, event: EventEnvelope<serde_json::Value>) -> Result<(), EventError> {
        let fall_event: FallDetected = serde_json::from_value(event.payload)?;

        if fall_event.confidence > 0.8 {
            self.notifier.send_alert(Alert {
                severity: AlertSeverity::Critical,
                title: "Fall Detected".to_string(),
                message: format!(
                    "Person {} detected falling in zone {:?}",
                    fall_event.person_id.0,
                    fall_event.zone_id
                ),
                timestamp: fall_event.timestamp,
            }).await?;
        }

        Ok(())
    }
}
```

---

## Event Versioning

Events evolve over time. Use explicit versioning:

```rust
/// Version 1 of PoseEstimated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseEstimatedV1 {
    pub estimate_id: EstimateId,
    pub person_count: u8,
    pub confidence: f32,
    pub timestamp: DateTime<Utc>,
}

/// Version 2 adds zone support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseEstimatedV2 {
    pub estimate_id: EstimateId,
    pub signal_id: SignalId,  // Added
    pub zone_id: Option<ZoneId>,  // Added
    pub person_count: u8,
    pub persons: Vec<PersonSummary>,  // Changed from just count
    pub overall_confidence: f32,  // Renamed
    pub timestamp: DateTime<Utc>,
}

/// Event upgrader for migration
pub trait EventUpgrader {
    fn upgrade_v1_to_v2(v1: PoseEstimatedV1) -> PoseEstimatedV2 {
        PoseEstimatedV2 {
            estimate_id: v1.estimate_id,
            signal_id: SignalId(Uuid::nil()),  // Unknown
            zone_id: None,  // Not available in V1
            person_count: v1.person_count,
            persons: vec![],  // Cannot reconstruct
            overall_confidence: v1.confidence,
            timestamp: v1.timestamp,
        }
    }
}
```

---

## Event Sourcing Support

For aggregates requiring full audit trail:

```rust
/// Event store interface
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Append events to aggregate stream
    async fn append(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        expected_version: u64,
        events: Vec<EventEnvelope<serde_json::Value>>,
    ) -> Result<u64, EventStoreError>;

    /// Load all events for an aggregate
    async fn load(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<Vec<EventEnvelope<serde_json::Value>>, EventStoreError>;

    /// Load events from a specific version
    async fn load_from_version(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        from_version: u64,
    ) -> Result<Vec<EventEnvelope<serde_json::Value>>, EventStoreError>;
}

/// Reconstruct aggregate from events
pub trait EventSourced: Sized {
    fn apply(&mut self, event: &dyn DomainEvent);

    fn replay(events: Vec<EventEnvelope<serde_json::Value>>) -> Result<Self, ReplayError>;
}
```
