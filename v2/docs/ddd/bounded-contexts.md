# Bounded Contexts

This document defines the five bounded contexts that compose the WiFi-DensePose system. Each context represents a distinct subdomain with its own ubiquitous language, models, and boundaries.

---

## 1. Signal Domain (CSI Processing)

### Purpose

The Signal Domain is responsible for acquiring, validating, preprocessing, and extracting features from Channel State Information (CSI) data. It transforms raw RF measurements into structured signal features suitable for pose inference.

### Ubiquitous Language (Context-Specific)

| Term | Definition |
|------|------------|
| CSI Frame | A single capture of channel state information across all subcarriers and antennas |
| Subcarrier | Individual frequency bin in OFDM modulation carrying amplitude and phase data |
| Amplitude | Signal strength component of CSI measurement |
| Phase | Signal timing component of CSI measurement |
| Doppler Shift | Frequency change caused by moving objects |
| Noise Floor | Background electromagnetic interference level |
| SNR | Signal-to-Noise Ratio, quality metric for CSI data |

### Core Responsibilities

1. **CSI Acquisition** - Interface with hardware to receive raw CSI bytes
2. **Frame Parsing** - Decode vendor-specific CSI formats (ESP32, Atheros, Intel)
3. **Validation** - Verify frame integrity, antenna counts, subcarrier dimensions
4. **Preprocessing** - Noise removal, windowing, normalization
5. **Feature Extraction** - Compute amplitude statistics, phase differences, correlations, PSD

### Aggregate: CsiFrame

```rust
pub struct CsiFrame {
    id: FrameId,
    device_id: DeviceId,
    session_id: Option<SessionId>,
    timestamp: Timestamp,
    sequence_number: u64,

    // Raw measurements
    amplitude: Matrix<f32>,     // [antennas x subcarriers]
    phase: Matrix<f32>,         // [antennas x subcarriers]

    // Signal characteristics
    frequency: Frequency,       // Center frequency (Hz)
    bandwidth: Bandwidth,       // Channel bandwidth (Hz)
    num_subcarriers: u16,
    num_antennas: u8,

    // Quality metrics
    snr: SignalToNoise,
    rssi: Option<Rssi>,
    noise_floor: Option<NoiseFloor>,

    // Processing state
    status: ProcessingStatus,
    metadata: FrameMetadata,
}
```

### Value Objects

```rust
// Validated frequency with invariants
pub struct Frequency(f64); // Hz, must be > 0

// Bandwidth with common WiFi values
pub enum Bandwidth {
    Bw20MHz,
    Bw40MHz,
    Bw80MHz,
    Bw160MHz,
}

// SNR with reasonable bounds
pub struct SignalToNoise(f64); // dB, typically -50 to +50

// Processing pipeline status
pub enum ProcessingStatus {
    Pending,
    Preprocessing,
    FeatureExtraction,
    Completed,
    Failed(ProcessingError),
}
```

### Domain Services

```rust
pub trait CsiPreprocessor {
    fn remove_noise(&self, frame: &CsiFrame, threshold: NoiseThreshold) -> Result<CsiFrame>;
    fn apply_window(&self, frame: &CsiFrame, window: WindowFunction) -> Result<CsiFrame>;
    fn normalize_amplitude(&self, frame: &CsiFrame) -> Result<CsiFrame>;
    fn sanitize_phase(&self, frame: &CsiFrame) -> Result<CsiFrame>;
}

pub trait FeatureExtractor {
    fn extract_amplitude_features(&self, frame: &CsiFrame) -> AmplitudeFeatures;
    fn extract_phase_features(&self, frame: &CsiFrame) -> PhaseFeatures;
    fn extract_correlation_features(&self, frame: &CsiFrame) -> CorrelationFeatures;
    fn extract_doppler_features(&self, frames: &[CsiFrame]) -> DopplerFeatures;
    fn compute_power_spectral_density(&self, frame: &CsiFrame) -> PowerSpectralDensity;
}
```

### Outbound Events

- `CsiFrameReceived` - Raw frame acquired from hardware
- `CsiFrameValidated` - Frame passed integrity checks
- `SignalProcessed` - Features extracted and ready for inference

### Integration Points

| Context | Direction | Mechanism |
|---------|-----------|-----------|
| Hardware Domain | Inbound | Raw bytes via async channel |
| Pose Domain | Outbound | ProcessedSignal via event bus |
| Storage Domain | Outbound | Persistence via repository |

---

## 2. Pose Domain (DensePose Inference)

### Purpose

The Pose Domain is the core of the system. It translates processed CSI features into human body pose estimates using neural network inference. This domain encapsulates the modality translation algorithms and DensePose model integration.

### Ubiquitous Language (Context-Specific)

| Term | Definition |
|------|------------|
| Modality Translation | Converting RF signal features to visual-like representations |
| DensePose | Dense human pose estimation mapping pixels to body surface |
| Body Part | Anatomical region (head, torso, limbs) identified in segmentation |
| UV Coordinates | 2D surface coordinates on body mesh |
| Keypoint | Named anatomical landmark (nose, shoulder, knee, etc.) |
| Confidence Score | Probability that a detection is correct |
| Bounding Box | Rectangular region containing a detected person |

### Core Responsibilities

1. **Modality Translation** - Transform CSI features to visual feature space
2. **Person Detection** - Identify presence and count of humans
3. **Body Segmentation** - Classify pixels/regions into body parts
4. **UV Regression** - Predict continuous surface coordinates
5. **Keypoint Localization** - Detect anatomical landmarks
6. **Activity Classification** - Infer high-level activities (standing, sitting, walking)

### Aggregate: PoseEstimate

```rust
pub struct PoseEstimate {
    id: EstimateId,
    session_id: SessionId,
    frame_id: FrameId,
    timestamp: Timestamp,

    // Detection results
    persons: Vec<PersonDetection>,
    person_count: u8,

    // Processing metadata
    processing_time: Duration,
    model_version: ModelVersion,
    algorithm: InferenceAlgorithm,

    // Quality assessment
    overall_confidence: Confidence,
    is_valid: bool,
}

pub struct PersonDetection {
    person_id: PersonId,
    bounding_box: BoundingBox,
    keypoints: Vec<Keypoint>,
    body_parts: BodyPartSegmentation,
    uv_coordinates: UvMap,
    confidence: Confidence,
    activity: Option<Activity>,
}

pub struct Keypoint {
    name: KeypointName,
    position: Position2D,
    confidence: Confidence,
}

pub enum KeypointName {
    Nose,
    LeftEye,
    RightEye,
    LeftEar,
    RightEar,
    LeftShoulder,
    RightShoulder,
    LeftElbow,
    RightElbow,
    LeftWrist,
    RightWrist,
    LeftHip,
    RightHip,
    LeftKnee,
    RightKnee,
    LeftAnkle,
    RightAnkle,
}
```

### Value Objects

```rust
// Confidence score bounded [0, 1]
pub struct Confidence(f32);

impl Confidence {
    pub fn new(value: f32) -> Result<Self, DomainError> {
        if value < 0.0 || value > 1.0 {
            return Err(DomainError::InvalidConfidence);
        }
        Ok(Self(value))
    }

    pub fn is_high(&self) -> bool {
        self.0 >= 0.8
    }
}

// 2D position in normalized coordinates [0, 1]
pub struct Position2D {
    x: NormalizedCoordinate,
    y: NormalizedCoordinate,
}

// Activity classification
pub enum Activity {
    Standing,
    Sitting,
    Walking,
    Lying,
    Falling,
    Unknown,
}
```

### Domain Services

```rust
pub trait ModalityTranslator {
    fn translate(&self, signal: &ProcessedSignal) -> Result<VisualFeatures>;
}

pub trait PoseInferenceEngine {
    fn detect_persons(&self, features: &VisualFeatures) -> Vec<PersonDetection>;
    fn segment_body_parts(&self, detection: &PersonDetection) -> BodyPartSegmentation;
    fn regress_uv_coordinates(&self, detection: &PersonDetection) -> UvMap;
    fn classify_activity(&self, detection: &PersonDetection) -> Activity;
}

pub trait HumanPresenceDetector {
    fn detect_presence(&self, signal: &ProcessedSignal) -> HumanPresenceResult;
    fn estimate_count(&self, signal: &ProcessedSignal) -> PersonCount;
}
```

### Outbound Events

- `PoseEstimated` - Pose inference completed successfully
- `PersonDetected` - New person entered detection zone
- `PersonLost` - Person left detection zone
- `ActivityChanged` - Person's activity classification changed
- `MotionDetected` - Significant motion observed
- `FallDetected` - Potential fall event identified

### Integration Points

| Context | Direction | Mechanism |
|---------|-----------|-----------|
| Signal Domain | Inbound | ProcessedSignal events |
| Streaming Domain | Outbound | PoseEstimate broadcasts |
| Storage Domain | Outbound | Persistence via repository |

---

## 3. Streaming Domain (WebSocket, Real-time)

### Purpose

The Streaming Domain manages real-time data delivery to clients via WebSocket connections. It handles connection lifecycle, message routing, filtering by zones/topics, and maintains streaming quality of service.

### Ubiquitous Language (Context-Specific)

| Term | Definition |
|------|------------|
| Connection | Active WebSocket session with a client |
| Stream Type | Category of data stream (pose, csi, alerts, status) |
| Zone | Logical or physical area for filtering pose data |
| Subscription | Client's expressed interest in specific stream/zone |
| Broadcast | Message sent to all matching subscribers |
| Heartbeat | Periodic ping to verify connection liveness |
| Backpressure | Flow control when client cannot keep up |

### Core Responsibilities

1. **Connection Management** - Accept, track, and close WebSocket connections
2. **Subscription Handling** - Manage client subscriptions to streams and zones
3. **Message Routing** - Deliver events to matching subscribers
4. **Quality of Service** - Handle backpressure, buffering, reconnection
5. **Metrics Collection** - Track latency, throughput, error rates

### Aggregate: Session

```rust
pub struct Session {
    id: SessionId,
    client_id: ClientId,

    // Connection details
    connected_at: Timestamp,
    last_activity: Timestamp,
    remote_addr: Option<IpAddr>,
    user_agent: Option<String>,

    // Subscription state
    stream_type: StreamType,
    zone_subscriptions: Vec<ZoneId>,
    filters: SubscriptionFilters,

    // Session state
    status: SessionStatus,
    message_count: u64,

    // Quality metrics
    latency_stats: LatencyStats,
    error_count: u32,
}

pub enum StreamType {
    Pose,
    Csi,
    Alerts,
    SystemStatus,
    All,
}

pub enum SessionStatus {
    Active,
    Paused,
    Reconnecting,
    Completed,
    Failed(SessionError),
    Cancelled,
}

pub struct SubscriptionFilters {
    min_confidence: Option<Confidence>,
    max_persons: Option<u8>,
    include_keypoints: bool,
    include_segmentation: bool,
    throttle_ms: Option<u32>,
}
```

### Value Objects

```rust
// Zone identifier with validation
pub struct ZoneId(String);

impl ZoneId {
    pub fn new(id: impl Into<String>) -> Result<Self, DomainError> {
        let id = id.into();
        if id.is_empty() || id.len() > 64 {
            return Err(DomainError::InvalidZoneId);
        }
        Ok(Self(id))
    }
}

// Latency tracking
pub struct LatencyStats {
    min_ms: f64,
    max_ms: f64,
    avg_ms: f64,
    p99_ms: f64,
    samples: u64,
}
```

### Domain Services

```rust
pub trait ConnectionManager {
    async fn connect(&self, socket: WebSocket, config: ConnectionConfig) -> Result<SessionId>;
    async fn disconnect(&self, session_id: &SessionId) -> Result<()>;
    async fn update_subscription(&self, session_id: &SessionId, filters: SubscriptionFilters) -> Result<()>;
    fn get_active_sessions(&self) -> Vec<&Session>;
}

pub trait MessageRouter {
    async fn broadcast(&self, message: StreamMessage, filter: BroadcastFilter) -> BroadcastResult;
    async fn send_to_session(&self, session_id: &SessionId, message: StreamMessage) -> Result<()>;
    async fn send_to_zone(&self, zone_id: &ZoneId, message: StreamMessage) -> BroadcastResult;
}

pub trait StreamBuffer {
    fn buffer_message(&mut self, message: StreamMessage);
    fn get_recent(&self, count: usize) -> Vec<&StreamMessage>;
    fn clear(&mut self);
}
```

### Outbound Events

- `SessionStarted` - Client connected and subscribed
- `SessionEnded` - Client disconnected
- `SubscriptionUpdated` - Client changed filter preferences
- `MessageDelivered` - Confirmation of successful delivery
- `DeliveryFailed` - Message could not be delivered

### Integration Points

| Context | Direction | Mechanism |
|---------|-----------|-----------|
| Pose Domain | Inbound | PoseEstimate events |
| Signal Domain | Inbound | ProcessedSignal events (if CSI streaming enabled) |
| API Layer | Bidirectional | WebSocket upgrade, REST for management |

---

## 4. Storage Domain (Persistence)

### Purpose

The Storage Domain handles all persistence operations including saving CSI frames, pose estimates, session records, and device configurations. It provides repositories for aggregate roots and supports both real-time writes and historical queries.

### Ubiquitous Language (Context-Specific)

| Term | Definition |
|------|------------|
| Repository | Interface for aggregate persistence operations |
| Entity | Persistent domain object with identity |
| Query | Read operation against stored data |
| Migration | Schema evolution script |
| Transaction | Atomic unit of work |
| Aggregate Store | Persistence layer for aggregate roots |

### Core Responsibilities

1. **CRUD Operations** - Create, read, update, delete for all aggregates
2. **Query Support** - Time-range queries, filtering, aggregation
3. **Transaction Management** - Ensure consistency across operations
4. **Schema Evolution** - Handle database migrations
5. **Performance Optimization** - Indexing, partitioning, caching

### Repository Interfaces

```rust
#[async_trait]
pub trait CsiFrameRepository {
    async fn save(&self, frame: &CsiFrame) -> Result<FrameId>;
    async fn save_batch(&self, frames: &[CsiFrame]) -> Result<Vec<FrameId>>;
    async fn find_by_id(&self, id: &FrameId) -> Result<Option<CsiFrame>>;
    async fn find_by_session(&self, session_id: &SessionId, limit: usize) -> Result<Vec<CsiFrame>>;
    async fn find_by_time_range(&self, start: Timestamp, end: Timestamp) -> Result<Vec<CsiFrame>>;
    async fn delete_older_than(&self, cutoff: Timestamp) -> Result<u64>;
}

#[async_trait]
pub trait PoseEstimateRepository {
    async fn save(&self, estimate: &PoseEstimate) -> Result<EstimateId>;
    async fn find_by_id(&self, id: &EstimateId) -> Result<Option<PoseEstimate>>;
    async fn find_by_session(&self, session_id: &SessionId) -> Result<Vec<PoseEstimate>>;
    async fn find_by_zone_and_time(&self, zone_id: &ZoneId, start: Timestamp, end: Timestamp) -> Result<Vec<PoseEstimate>>;
    async fn get_statistics(&self, start: Timestamp, end: Timestamp) -> Result<PoseStatistics>;
}

#[async_trait]
pub trait SessionRepository {
    async fn save(&self, session: &Session) -> Result<SessionId>;
    async fn update(&self, session: &Session) -> Result<()>;
    async fn find_by_id(&self, id: &SessionId) -> Result<Option<Session>>;
    async fn find_active(&self) -> Result<Vec<Session>>;
    async fn find_by_device(&self, device_id: &DeviceId) -> Result<Vec<Session>>;
    async fn mark_completed(&self, id: &SessionId, end_time: Timestamp) -> Result<()>;
}

#[async_trait]
pub trait DeviceRepository {
    async fn save(&self, device: &Device) -> Result<DeviceId>;
    async fn update(&self, device: &Device) -> Result<()>;
    async fn find_by_id(&self, id: &DeviceId) -> Result<Option<Device>>;
    async fn find_by_mac(&self, mac: &MacAddress) -> Result<Option<Device>>;
    async fn find_all(&self) -> Result<Vec<Device>>;
    async fn find_by_status(&self, status: DeviceStatus) -> Result<Vec<Device>>;
}
```

### Query Objects

```rust
pub struct TimeRangeQuery {
    start: Timestamp,
    end: Timestamp,
    zone_ids: Option<Vec<ZoneId>>,
    device_ids: Option<Vec<DeviceId>>,
    limit: Option<usize>,
    offset: Option<usize>,
}

pub struct PoseStatistics {
    total_detections: u64,
    successful_detections: u64,
    failed_detections: u64,
    average_confidence: f32,
    average_processing_time_ms: f32,
    unique_persons: u32,
    activity_distribution: HashMap<Activity, f32>,
}

pub struct AggregatedPoseData {
    timestamp: Timestamp,
    interval_seconds: u32,
    total_persons: u32,
    zones: HashMap<ZoneId, ZoneOccupancy>,
}
```

### Integration Points

| Context | Direction | Mechanism |
|---------|-----------|-----------|
| All Domains | Inbound | Repository trait implementations |
| Infrastructure | Outbound | SQLx, Redis adapters |

---

## 5. Hardware Domain (Device Management)

### Purpose

The Hardware Domain abstracts physical WiFi devices (routers, ESP32, Intel NICs) and manages their lifecycle. It handles device discovery, connection establishment, configuration, and health monitoring.

### Ubiquitous Language (Context-Specific)

| Term | Definition |
|------|------------|
| Device | Physical WiFi hardware capable of CSI extraction |
| Firmware | Software running on the device |
| MAC Address | Unique hardware identifier |
| Calibration | Process of tuning device for accurate CSI |
| Health Check | Periodic verification of device status |
| Driver | Software interface to hardware |

### Core Responsibilities

1. **Device Discovery** - Scan network for compatible devices
2. **Connection Management** - Establish and maintain hardware connections
3. **Configuration** - Apply and persist device settings
4. **Health Monitoring** - Track device status and performance
5. **Firmware Management** - Version tracking, update coordination

### Aggregate: Device

```rust
pub struct Device {
    id: DeviceId,

    // Identification
    name: DeviceName,
    device_type: DeviceType,
    mac_address: MacAddress,
    ip_address: Option<IpAddress>,

    // Hardware details
    firmware_version: Option<FirmwareVersion>,
    hardware_version: Option<HardwareVersion>,
    capabilities: DeviceCapabilities,

    // Location
    location: Option<Location>,
    zone_id: Option<ZoneId>,

    // State
    status: DeviceStatus,
    last_seen: Option<Timestamp>,
    error_count: u32,

    // Configuration
    config: DeviceConfig,
    calibration: Option<CalibrationData>,
}

pub enum DeviceType {
    Esp32,
    AtheriosRouter,
    IntelNic,
    Nexmon,
    Custom(String),
}

pub enum DeviceStatus {
    Disconnected,
    Connecting,
    Connected,
    Streaming,
    Calibrating,
    Maintenance,
    Error(DeviceError),
}

pub struct DeviceCapabilities {
    max_subcarriers: u16,
    max_antennas: u8,
    supported_bandwidths: Vec<Bandwidth>,
    supported_frequencies: Vec<Frequency>,
    csi_rate_hz: u32,
}

pub struct DeviceConfig {
    sampling_rate: u32,
    subcarriers: u16,
    antennas: u8,
    bandwidth: Bandwidth,
    channel: WifiChannel,
    gain: Option<f32>,
    custom_params: HashMap<String, serde_json::Value>,
}
```

### Value Objects

```rust
// MAC address with validation
pub struct MacAddress([u8; 6]);

impl MacAddress {
    pub fn parse(s: &str) -> Result<Self, DomainError> {
        // Parse "AA:BB:CC:DD:EE:FF" format
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 6 {
            return Err(DomainError::InvalidMacAddress);
        }
        let mut bytes = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            bytes[i] = u8::from_str_radix(part, 16)
                .map_err(|_| DomainError::InvalidMacAddress)?;
        }
        Ok(Self(bytes))
    }
}

// Physical location
pub struct Location {
    name: String,
    room_id: Option<String>,
    coordinates: Option<Coordinates3D>,
}

pub struct Coordinates3D {
    x: f64,
    y: f64,
    z: f64,
}
```

### Domain Services

```rust
pub trait DeviceDiscovery {
    async fn scan(&self, timeout: Duration) -> Vec<DiscoveredDevice>;
    async fn identify(&self, address: &IpAddress) -> Option<DeviceType>;
}

pub trait DeviceConnector {
    async fn connect(&self, device: &Device) -> Result<DeviceConnection>;
    async fn disconnect(&self, device_id: &DeviceId) -> Result<()>;
    async fn reconnect(&self, device_id: &DeviceId) -> Result<DeviceConnection>;
}

pub trait DeviceConfigurator {
    async fn apply_config(&self, device_id: &DeviceId, config: &DeviceConfig) -> Result<()>;
    async fn read_config(&self, device_id: &DeviceId) -> Result<DeviceConfig>;
    async fn reset_to_defaults(&self, device_id: &DeviceId) -> Result<()>;
}

pub trait CalibrationService {
    async fn start_calibration(&self, device_id: &DeviceId) -> Result<CalibrationSession>;
    async fn get_calibration_status(&self, session_id: &CalibrationSessionId) -> CalibrationStatus;
    async fn apply_calibration(&self, device_id: &DeviceId, data: &CalibrationData) -> Result<()>;
}

pub trait HealthMonitor {
    async fn check_health(&self, device_id: &DeviceId) -> HealthStatus;
    async fn get_metrics(&self, device_id: &DeviceId) -> DeviceMetrics;
}
```

### Outbound Events

- `DeviceDiscovered` - New device found on network
- `DeviceConnected` - Connection established
- `DeviceDisconnected` - Connection lost
- `DeviceConfigured` - Configuration applied
- `DeviceCalibrated` - Calibration completed
- `DeviceHealthChanged` - Status change (healthy/unhealthy)
- `DeviceError` - Error condition detected

### Integration Points

| Context | Direction | Mechanism |
|---------|-----------|-----------|
| Signal Domain | Outbound | Raw CSI bytes via channel |
| Storage Domain | Outbound | Device persistence |
| API Layer | Bidirectional | REST endpoints for management |

---

## Context Integration Patterns

### Anti-Corruption Layer

When integrating with vendor-specific CSI formats, the Signal Domain uses an Anti-Corruption Layer to translate external formats:

```rust
pub trait CsiParser: Send + Sync {
    fn parse(&self, raw: &[u8]) -> Result<CsiFrame>;
    fn device_type(&self) -> DeviceType;
}

pub struct Esp32Parser;
pub struct AtheriosParser;
pub struct IntelParser;

pub struct ParserRegistry {
    parsers: HashMap<DeviceType, Box<dyn CsiParser>>,
}
```

### Published Language

The Pose Domain publishes events in a standardized format that other contexts consume:

```rust
#[derive(Serialize, Deserialize)]
pub struct PoseEventPayload {
    pub event_type: String,
    pub version: String,
    pub timestamp: DateTime<Utc>,
    pub correlation_id: Uuid,
    pub payload: PoseEstimate,
}
```

### Shared Kernel

The `wifi-densepose-core` crate contains shared types used across all contexts:

- Identifiers: `DeviceId`, `SessionId`, `FrameId`, `EstimateId`
- Timestamps: `Timestamp`, `Duration`
- Common errors: `DomainError`
- Configuration: `ConfigurationLoader`
