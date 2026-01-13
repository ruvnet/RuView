# Aggregates

This document defines the core aggregates in the WiFi-DensePose system. Each aggregate is a cluster of domain objects that are treated as a single unit for data changes, with one entity designated as the aggregate root.

---

## Design Principles

### Aggregate Invariants

1. **Transactional Consistency** - All changes within an aggregate are atomic
2. **Identity** - Each aggregate root has a unique identifier
3. **Encapsulation** - Internal entities are only accessible through the root
4. **Eventual Consistency** - Cross-aggregate references use IDs, not direct references

### Rust Implementation Pattern

```rust
// Aggregate root with private constructor enforcing invariants
pub struct AggregateRoot {
    id: AggregateId,
    // ... fields
}

impl AggregateRoot {
    // Factory method enforcing invariants
    pub fn create(params: CreateParams) -> Result<Self, DomainError> {
        // Validate invariants
        Self::validate(&params)?;

        Ok(Self {
            id: AggregateId::generate(),
            // ... initialize fields
        })
    }

    // Commands return domain events
    pub fn handle_command(&mut self, cmd: Command) -> Result<Vec<DomainEvent>, DomainError> {
        // Validate command against current state
        // Apply state changes
        // Return events
    }
}
```

---

## 1. CsiFrame Aggregate

### Purpose

Represents a single capture of Channel State Information from WiFi hardware. This is the foundational data structure that flows through the signal processing pipeline.

### Aggregate Root: CsiFrame

```rust
use chrono::{DateTime, Utc};
use uuid::Uuid;
use ndarray::Array2;

/// Aggregate root for CSI frame data
#[derive(Debug, Clone)]
pub struct CsiFrame {
    // Identity
    id: FrameId,

    // Relationships (by ID, not reference)
    device_id: DeviceId,
    session_id: Option<SessionId>,

    // Temporal
    timestamp: DateTime<Utc>,
    sequence_number: u64,

    // Core CSI data (immutable after creation)
    amplitude: Array2<f32>,  // [antennas, subcarriers]
    phase: Array2<f32>,      // [antennas, subcarriers]

    // Signal parameters
    frequency: Frequency,
    bandwidth: Bandwidth,

    // Dimensions
    num_subcarriers: u16,
    num_antennas: u8,

    // Quality metrics
    snr: SignalToNoise,
    rssi: Option<Rssi>,
    noise_floor: Option<NoiseFloor>,

    // Processing state
    status: ProcessingStatus,
    processed_at: Option<DateTime<Utc>>,

    // Extensible metadata
    metadata: FrameMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameId(Uuid);

impl FrameId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}
```

### Value Objects

```rust
/// Center frequency in Hz (must be positive)
#[derive(Debug, Clone, Copy)]
pub struct Frequency(f64);

impl Frequency {
    pub fn new(hz: f64) -> Result<Self, DomainError> {
        if hz <= 0.0 {
            return Err(DomainError::InvalidFrequency { value: hz });
        }
        Ok(Self(hz))
    }

    pub fn as_hz(&self) -> f64 {
        self.0
    }

    pub fn as_ghz(&self) -> f64 {
        self.0 / 1_000_000_000.0
    }

    /// Common WiFi frequencies
    pub fn wifi_2_4ghz() -> Self {
        Self(2_400_000_000.0)
    }

    pub fn wifi_5ghz() -> Self {
        Self(5_000_000_000.0)
    }
}

/// Channel bandwidth
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bandwidth {
    Bw20MHz,
    Bw40MHz,
    Bw80MHz,
    Bw160MHz,
}

impl Bandwidth {
    pub fn as_hz(&self) -> f64 {
        match self {
            Self::Bw20MHz => 20_000_000.0,
            Self::Bw40MHz => 40_000_000.0,
            Self::Bw80MHz => 80_000_000.0,
            Self::Bw160MHz => 160_000_000.0,
        }
    }

    pub fn expected_subcarriers(&self) -> u16 {
        match self {
            Self::Bw20MHz => 56,
            Self::Bw40MHz => 114,
            Self::Bw80MHz => 242,
            Self::Bw160MHz => 484,
        }
    }
}

/// Signal-to-Noise Ratio in dB
#[derive(Debug, Clone, Copy)]
pub struct SignalToNoise(f64);

impl SignalToNoise {
    const MIN_DB: f64 = -50.0;
    const MAX_DB: f64 = 50.0;

    pub fn new(db: f64) -> Result<Self, DomainError> {
        if db < Self::MIN_DB || db > Self::MAX_DB {
            return Err(DomainError::InvalidSnr { value: db });
        }
        Ok(Self(db))
    }

    pub fn as_db(&self) -> f64 {
        self.0
    }

    pub fn is_good(&self) -> bool {
        self.0 >= 20.0
    }

    pub fn is_acceptable(&self) -> bool {
        self.0 >= 10.0
    }
}

/// Processing pipeline status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessingStatus {
    Pending,
    Preprocessing,
    FeatureExtraction,
    Completed,
    Failed { reason: String },
}
```

### Invariants

1. Amplitude and phase arrays must have matching dimensions
2. Dimensions must match num_subcarriers x num_antennas
3. Frequency must be positive
4. SNR must be within reasonable bounds (-50 to +50 dB)
5. Sequence numbers are monotonically increasing per session

### Factory Methods

```rust
impl CsiFrame {
    /// Create a new CSI frame with validation
    pub fn create(params: CreateCsiFrameParams) -> Result<Self, DomainError> {
        // Validate dimensions
        let (rows, cols) = params.amplitude.dim();
        if rows != params.num_antennas as usize || cols != params.num_subcarriers as usize {
            return Err(DomainError::DimensionMismatch {
                expected_antennas: params.num_antennas,
                expected_subcarriers: params.num_subcarriers,
                actual_rows: rows,
                actual_cols: cols,
            });
        }

        // Validate phase dimensions match amplitude
        if params.amplitude.dim() != params.phase.dim() {
            return Err(DomainError::PhaseDimensionMismatch);
        }

        Ok(Self {
            id: FrameId::generate(),
            device_id: params.device_id,
            session_id: params.session_id,
            timestamp: Utc::now(),
            sequence_number: params.sequence_number,
            amplitude: params.amplitude,
            phase: params.phase,
            frequency: params.frequency,
            bandwidth: params.bandwidth,
            num_subcarriers: params.num_subcarriers,
            num_antennas: params.num_antennas,
            snr: params.snr,
            rssi: params.rssi,
            noise_floor: params.noise_floor,
            status: ProcessingStatus::Pending,
            processed_at: None,
            metadata: params.metadata.unwrap_or_default(),
        })
    }

    /// Reconstruct from persistence (bypass validation)
    pub(crate) fn reconstitute(/* all fields */) -> Self {
        // Used by repository implementations
        // Assumes data was validated on creation
    }
}
```

### Commands

```rust
impl CsiFrame {
    /// Mark frame as being preprocessed
    pub fn start_preprocessing(&mut self) -> Result<CsiFramePreprocessingStarted, DomainError> {
        match &self.status {
            ProcessingStatus::Pending => {
                self.status = ProcessingStatus::Preprocessing;
                Ok(CsiFramePreprocessingStarted {
                    frame_id: self.id,
                    timestamp: Utc::now(),
                })
            }
            _ => Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Preprocessing".to_string(),
            }),
        }
    }

    /// Mark frame as having features extracted
    pub fn complete_feature_extraction(&mut self) -> Result<CsiFrameProcessed, DomainError> {
        match &self.status {
            ProcessingStatus::Preprocessing | ProcessingStatus::FeatureExtraction => {
                self.status = ProcessingStatus::Completed;
                self.processed_at = Some(Utc::now());
                Ok(CsiFrameProcessed {
                    frame_id: self.id,
                    processed_at: self.processed_at.unwrap(),
                })
            }
            _ => Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Completed".to_string(),
            }),
        }
    }

    /// Mark frame as failed
    pub fn fail(&mut self, reason: String) -> CsiFrameProcessingFailed {
        self.status = ProcessingStatus::Failed { reason: reason.clone() };
        CsiFrameProcessingFailed {
            frame_id: self.id,
            reason,
            timestamp: Utc::now(),
        }
    }
}
```

---

## 2. ProcessedSignal Aggregate

### Purpose

Represents the extracted features from one or more CSI frames, ready for pose inference. This is the output of the Signal Domain and input to the Pose Domain.

### Aggregate Root: ProcessedSignal

```rust
/// Aggregate root for processed signal features
#[derive(Debug, Clone)]
pub struct ProcessedSignal {
    // Identity
    id: SignalId,

    // Source frames
    source_frames: Vec<FrameId>,
    device_id: DeviceId,
    session_id: Option<SessionId>,

    // Temporal
    timestamp: DateTime<Utc>,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,

    // Extracted features
    features: SignalFeatures,

    // Human detection results
    human_presence: HumanPresenceResult,

    // Quality assessment
    quality_score: QualityScore,

    // Processing metadata
    processing_config: ProcessingConfig,
    extraction_time: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SignalId(Uuid);

/// Collection of extracted signal features
#[derive(Debug, Clone)]
pub struct SignalFeatures {
    // Amplitude features
    pub amplitude_mean: Array1<f32>,
    pub amplitude_variance: Array1<f32>,
    pub amplitude_skewness: Array1<f32>,
    pub amplitude_kurtosis: Array1<f32>,

    // Phase features
    pub phase_difference: Array1<f32>,
    pub phase_unwrapped: Array2<f32>,

    // Correlation features
    pub antenna_correlation: Array2<f32>,
    pub subcarrier_correlation: Array2<f32>,

    // Frequency domain features
    pub doppler_shift: Array1<f32>,
    pub power_spectral_density: Array1<f32>,
    pub dominant_frequencies: Vec<f32>,

    // Temporal features (if multiple frames)
    pub temporal_variance: Option<Array1<f32>>,
    pub motion_indicators: Option<MotionIndicators>,
}

/// Human presence detection result
#[derive(Debug, Clone)]
pub struct HumanPresenceResult {
    pub detected: bool,
    pub confidence: Confidence,
    pub motion_score: f32,
    pub estimated_count: Option<u8>,
}

/// Signal quality assessment
#[derive(Debug, Clone, Copy)]
pub struct QualityScore(f32);

impl QualityScore {
    pub fn new(score: f32) -> Result<Self, DomainError> {
        if score < 0.0 || score > 1.0 {
            return Err(DomainError::InvalidQualityScore { value: score });
        }
        Ok(Self(score))
    }

    pub fn is_usable(&self) -> bool {
        self.0 >= 0.3
    }

    pub fn is_good(&self) -> bool {
        self.0 >= 0.7
    }
}
```

### Factory Methods

```rust
impl ProcessedSignal {
    /// Create from extracted features
    pub fn create(
        source_frames: Vec<FrameId>,
        device_id: DeviceId,
        session_id: Option<SessionId>,
        features: SignalFeatures,
        human_presence: HumanPresenceResult,
        processing_config: ProcessingConfig,
        extraction_time: Duration,
    ) -> Result<Self, DomainError> {
        if source_frames.is_empty() {
            return Err(DomainError::NoSourceFrames);
        }

        let quality_score = Self::calculate_quality(&features)?;

        Ok(Self {
            id: SignalId(Uuid::new_v4()),
            source_frames,
            device_id,
            session_id,
            timestamp: Utc::now(),
            window_start: Utc::now(), // TODO: Calculate from frames
            window_end: Utc::now(),
            features,
            human_presence,
            quality_score,
            processing_config,
            extraction_time,
        })
    }

    fn calculate_quality(features: &SignalFeatures) -> Result<QualityScore, DomainError> {
        // Quality based on feature completeness and variance
        let amplitude_quality = if features.amplitude_variance.iter().any(|&v| v > 0.0) {
            1.0
        } else {
            0.5
        };

        let phase_quality = if !features.phase_difference.is_empty() {
            1.0
        } else {
            0.3
        };

        let score = 0.6 * amplitude_quality + 0.4 * phase_quality;
        QualityScore::new(score)
    }
}
```

---

## 3. PoseEstimate Aggregate

### Purpose

Represents the output of pose inference, containing detected persons with their body configurations, keypoints, and activity classifications.

### Aggregate Root: PoseEstimate

```rust
/// Aggregate root for pose estimation results
#[derive(Debug, Clone)]
pub struct PoseEstimate {
    // Identity
    id: EstimateId,

    // Source references
    signal_id: SignalId,
    session_id: SessionId,
    zone_id: Option<ZoneId>,

    // Temporal
    timestamp: DateTime<Utc>,
    frame_number: u64,

    // Detection results
    persons: Vec<PersonDetection>,
    person_count: u8,

    // Processing metadata
    processing_time: Duration,
    model_version: ModelVersion,
    algorithm: InferenceAlgorithm,

    // Quality metrics
    overall_confidence: Confidence,
    is_valid: bool,

    // Events generated during estimation
    detected_events: Vec<PoseEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EstimateId(Uuid);

/// Detected person with full pose information
#[derive(Debug, Clone)]
pub struct PersonDetection {
    pub person_id: PersonId,
    pub bounding_box: BoundingBox,
    pub keypoints: KeypointSet,
    pub body_parts: Option<BodyPartSegmentation>,
    pub uv_coordinates: Option<UvMap>,
    pub confidence: Confidence,
    pub activity: Activity,
    pub velocity: Option<Velocity2D>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PersonId(u32);

/// Set of anatomical keypoints
#[derive(Debug, Clone)]
pub struct KeypointSet {
    keypoints: HashMap<KeypointName, Keypoint>,
}

impl KeypointSet {
    pub fn new() -> Self {
        Self { keypoints: HashMap::new() }
    }

    pub fn add(&mut self, keypoint: Keypoint) {
        self.keypoints.insert(keypoint.name, keypoint);
    }

    pub fn get(&self, name: KeypointName) -> Option<&Keypoint> {
        self.keypoints.get(&name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Keypoint> {
        self.keypoints.values()
    }

    pub fn visible_count(&self) -> usize {
        self.keypoints.values().filter(|k| k.is_visible()).count()
    }
}

/// Single anatomical keypoint
#[derive(Debug, Clone)]
pub struct Keypoint {
    pub name: KeypointName,
    pub position: Position2D,
    pub confidence: Confidence,
    pub is_occluded: bool,
}

impl Keypoint {
    pub fn is_visible(&self) -> bool {
        !self.is_occluded && self.confidence.value() > 0.5
    }
}

/// Named keypoint locations following COCO format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

impl KeypointName {
    pub fn all() -> [Self; 17] {
        [
            Self::Nose,
            Self::LeftEye, Self::RightEye,
            Self::LeftEar, Self::RightEar,
            Self::LeftShoulder, Self::RightShoulder,
            Self::LeftElbow, Self::RightElbow,
            Self::LeftWrist, Self::RightWrist,
            Self::LeftHip, Self::RightHip,
            Self::LeftKnee, Self::RightKnee,
            Self::LeftAnkle, Self::RightAnkle,
        ]
    }
}
```

### Value Objects

```rust
/// Confidence score in [0, 1]
#[derive(Debug, Clone, Copy)]
pub struct Confidence(f32);

impl Confidence {
    pub fn new(value: f32) -> Result<Self, DomainError> {
        if value < 0.0 || value > 1.0 {
            return Err(DomainError::InvalidConfidence { value });
        }
        Ok(Self(value))
    }

    pub fn value(&self) -> f32 {
        self.0
    }

    pub fn is_high(&self) -> bool {
        self.0 >= 0.8
    }

    pub fn is_medium(&self) -> bool {
        self.0 >= 0.5 && self.0 < 0.8
    }

    pub fn is_low(&self) -> bool {
        self.0 < 0.5
    }
}

/// 2D position in normalized coordinates [0, 1]
#[derive(Debug, Clone, Copy)]
pub struct Position2D {
    x: NormalizedCoordinate,
    y: NormalizedCoordinate,
}

#[derive(Debug, Clone, Copy)]
pub struct NormalizedCoordinate(f32);

impl NormalizedCoordinate {
    pub fn new(value: f32) -> Result<Self, DomainError> {
        if value < 0.0 || value > 1.0 {
            return Err(DomainError::CoordinateOutOfRange { value });
        }
        Ok(Self(value))
    }
}

/// Rectangular bounding box
#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub x: NormalizedCoordinate,
    pub y: NormalizedCoordinate,
    pub width: f32,
    pub height: f32,
}

impl BoundingBox {
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    pub fn center(&self) -> Position2D {
        Position2D {
            x: NormalizedCoordinate(self.x.0 + self.width / 2.0),
            y: NormalizedCoordinate(self.y.0 + self.height / 2.0),
        }
    }
}

/// Classified activity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    Standing,
    Sitting,
    Walking,
    Running,
    Lying,
    Falling,
    Unknown,
}

impl Activity {
    pub fn is_alert_worthy(&self) -> bool {
        matches!(self, Activity::Falling)
    }

    pub fn is_mobile(&self) -> bool {
        matches!(self, Activity::Walking | Activity::Running)
    }
}
```

### Commands and Event Generation

```rust
impl PoseEstimate {
    /// Create new pose estimate from inference results
    pub fn create(
        signal_id: SignalId,
        session_id: SessionId,
        zone_id: Option<ZoneId>,
        persons: Vec<PersonDetection>,
        processing_time: Duration,
        model_version: ModelVersion,
    ) -> Result<(Self, Vec<DomainEvent>), DomainError> {
        let person_count = persons.len() as u8;
        let overall_confidence = Self::calculate_overall_confidence(&persons);

        let mut events = Vec::new();
        let mut detected_events = Vec::new();

        // Check for motion
        if persons.iter().any(|p| p.velocity.map(|v| v.is_significant()).unwrap_or(false)) {
            let event = PoseEvent::MotionDetected {
                timestamp: Utc::now(),
                zone_id: zone_id.clone(),
            };
            detected_events.push(event.clone());
            events.push(DomainEvent::MotionDetected(MotionDetectedEvent {
                zone_id: zone_id.clone(),
                person_count,
                timestamp: Utc::now(),
            }));
        }

        // Check for falls
        for person in &persons {
            if person.activity == Activity::Falling && person.confidence.is_high() {
                let event = PoseEvent::FallDetected {
                    person_id: person.person_id,
                    confidence: person.confidence,
                    timestamp: Utc::now(),
                };
                detected_events.push(event);
                events.push(DomainEvent::FallDetected(FallDetectedEvent {
                    person_id: person.person_id,
                    zone_id: zone_id.clone(),
                    confidence: person.confidence,
                    timestamp: Utc::now(),
                }));
            }
        }

        // Main estimation event
        events.push(DomainEvent::PoseEstimated(PoseEstimatedEvent {
            estimate_id: EstimateId(Uuid::new_v4()),
            signal_id,
            person_count,
            overall_confidence,
            timestamp: Utc::now(),
        }));

        let estimate = Self {
            id: EstimateId(Uuid::new_v4()),
            signal_id,
            session_id,
            zone_id,
            timestamp: Utc::now(),
            frame_number: 0, // TODO: Track frame numbers
            persons,
            person_count,
            processing_time,
            model_version,
            algorithm: InferenceAlgorithm::DensePose,
            overall_confidence,
            is_valid: true,
            detected_events,
        };

        Ok((estimate, events))
    }

    fn calculate_overall_confidence(persons: &[PersonDetection]) -> Confidence {
        if persons.is_empty() {
            return Confidence(0.0);
        }
        let sum: f32 = persons.iter().map(|p| p.confidence.value()).sum();
        Confidence(sum / persons.len() as f32)
    }
}
```

---

## 4. Session Aggregate

### Purpose

Represents a client connection session for real-time streaming. Tracks connection lifecycle, subscriptions, and delivery metrics.

### Aggregate Root: Session

```rust
/// Aggregate root for streaming sessions
#[derive(Debug)]
pub struct Session {
    // Identity
    id: SessionId,
    client_id: ClientId,

    // Connection details
    connected_at: DateTime<Utc>,
    last_activity: DateTime<Utc>,
    remote_addr: Option<IpAddr>,
    user_agent: Option<String>,

    // Subscription state
    stream_type: StreamType,
    zone_subscriptions: HashSet<ZoneId>,
    filters: SubscriptionFilters,

    // Session state (state machine)
    status: SessionStatus,

    // Metrics
    messages_sent: u64,
    messages_failed: u64,
    bytes_sent: u64,
    latency_samples: Vec<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(Uuid);

/// Session lifecycle states
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    /// Initial connection, not yet subscribed
    Connecting,

    /// Actively receiving data
    Active,

    /// Temporarily paused by client
    Paused,

    /// Connection lost, attempting reconnect
    Reconnecting { attempts: u8, last_attempt: DateTime<Utc> },

    /// Gracefully closed
    Completed { ended_at: DateTime<Utc> },

    /// Error termination
    Failed { reason: String, failed_at: DateTime<Utc> },

    /// Client-initiated cancellation
    Cancelled { cancelled_at: DateTime<Utc> },
}

/// Client subscription preferences
#[derive(Debug, Clone, Default)]
pub struct SubscriptionFilters {
    pub min_confidence: Option<Confidence>,
    pub max_persons: Option<u8>,
    pub include_keypoints: bool,
    pub include_segmentation: bool,
    pub include_uv_coordinates: bool,
    pub throttle_interval: Option<Duration>,
    pub activity_filter: Option<Vec<Activity>>,
}
```

### State Transitions

```rust
impl Session {
    /// Create new session
    pub fn create(
        client_id: ClientId,
        stream_type: StreamType,
        remote_addr: Option<IpAddr>,
        user_agent: Option<String>,
    ) -> (Self, SessionStartedEvent) {
        let session = Self {
            id: SessionId(Uuid::new_v4()),
            client_id,
            connected_at: Utc::now(),
            last_activity: Utc::now(),
            remote_addr,
            user_agent: user_agent.clone(),
            stream_type,
            zone_subscriptions: HashSet::new(),
            filters: SubscriptionFilters::default(),
            status: SessionStatus::Connecting,
            messages_sent: 0,
            messages_failed: 0,
            bytes_sent: 0,
            latency_samples: Vec::new(),
        };

        let event = SessionStartedEvent {
            session_id: session.id,
            client_id,
            stream_type,
            timestamp: Utc::now(),
        };

        (session, event)
    }

    /// Activate session after subscription setup
    pub fn activate(&mut self) -> Result<(), DomainError> {
        match &self.status {
            SessionStatus::Connecting | SessionStatus::Reconnecting { .. } => {
                self.status = SessionStatus::Active;
                self.last_activity = Utc::now();
                Ok(())
            }
            _ => Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Active".to_string(),
            }),
        }
    }

    /// Pause streaming
    pub fn pause(&mut self) -> Result<(), DomainError> {
        match &self.status {
            SessionStatus::Active => {
                self.status = SessionStatus::Paused;
                Ok(())
            }
            _ => Err(DomainError::CannotPause),
        }
    }

    /// Resume streaming
    pub fn resume(&mut self) -> Result<(), DomainError> {
        match &self.status {
            SessionStatus::Paused => {
                self.status = SessionStatus::Active;
                self.last_activity = Utc::now();
                Ok(())
            }
            _ => Err(DomainError::CannotResume),
        }
    }

    /// Handle connection loss
    pub fn connection_lost(&mut self) -> Result<(), DomainError> {
        match &self.status {
            SessionStatus::Active | SessionStatus::Paused => {
                self.status = SessionStatus::Reconnecting {
                    attempts: 0,
                    last_attempt: Utc::now(),
                };
                Ok(())
            }
            _ => Err(DomainError::AlreadyDisconnected),
        }
    }

    /// Complete session gracefully
    pub fn complete(&mut self) -> Result<SessionEndedEvent, DomainError> {
        match &self.status {
            SessionStatus::Active | SessionStatus::Paused => {
                let ended_at = Utc::now();
                self.status = SessionStatus::Completed { ended_at };

                Ok(SessionEndedEvent {
                    session_id: self.id,
                    duration: ended_at - self.connected_at,
                    messages_sent: self.messages_sent,
                    reason: "completed".to_string(),
                    timestamp: ended_at,
                })
            }
            _ => Err(DomainError::SessionNotActive),
        }
    }

    /// Update subscription filters
    pub fn update_filters(&mut self, filters: SubscriptionFilters) -> Result<SubscriptionUpdatedEvent, DomainError> {
        if !self.is_active() {
            return Err(DomainError::SessionNotActive);
        }

        self.filters = filters.clone();
        self.last_activity = Utc::now();

        Ok(SubscriptionUpdatedEvent {
            session_id: self.id,
            filters,
            timestamp: Utc::now(),
        })
    }

    /// Subscribe to zone
    pub fn subscribe_to_zone(&mut self, zone_id: ZoneId) -> Result<(), DomainError> {
        if !self.is_active() {
            return Err(DomainError::SessionNotActive);
        }

        self.zone_subscriptions.insert(zone_id);
        self.last_activity = Utc::now();
        Ok(())
    }

    /// Record successful message delivery
    pub fn record_message_sent(&mut self, bytes: u64, latency: Duration) {
        self.messages_sent += 1;
        self.bytes_sent += bytes;
        self.last_activity = Utc::now();

        // Keep last 100 latency samples
        if self.latency_samples.len() >= 100 {
            self.latency_samples.remove(0);
        }
        self.latency_samples.push(latency);
    }

    /// Record failed delivery
    pub fn record_message_failed(&mut self) {
        self.messages_failed += 1;
    }

    // Queries

    pub fn is_active(&self) -> bool {
        matches!(self.status, SessionStatus::Active)
    }

    pub fn is_subscribed_to_zone(&self, zone_id: &ZoneId) -> bool {
        self.zone_subscriptions.is_empty() || self.zone_subscriptions.contains(zone_id)
    }

    pub fn average_latency(&self) -> Option<Duration> {
        if self.latency_samples.is_empty() {
            return None;
        }
        let sum: Duration = self.latency_samples.iter().sum();
        Some(sum / self.latency_samples.len() as u32)
    }
}
```

---

## 5. Device Aggregate

### Purpose

Represents a physical WiFi hardware device capable of CSI extraction. Manages device lifecycle, configuration, and health status.

### Aggregate Root: Device

```rust
/// Aggregate root for hardware devices
#[derive(Debug)]
pub struct Device {
    // Identity
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

    // State machine
    status: DeviceStatus,

    // Health tracking
    last_seen: Option<DateTime<Utc>>,
    health_checks: VecDeque<HealthCheckResult>,
    consecutive_failures: u8,

    // Configuration
    config: DeviceConfig,
    calibration: Option<CalibrationData>,

    // Metadata
    tags: HashSet<String>,
    custom_properties: HashMap<String, serde_json::Value>,

    // Timestamps
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceId(Uuid);

/// Device state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceStatus {
    /// Not connected to network
    Disconnected,

    /// Attempting to establish connection
    Connecting { started_at: DateTime<Utc> },

    /// Connected and ready
    Connected { connected_at: DateTime<Utc> },

    /// Actively streaming CSI data
    Streaming { stream_started_at: DateTime<Utc>, frames_sent: u64 },

    /// Running calibration procedure
    Calibrating { calibration_id: CalibrationId, progress: u8 },

    /// Scheduled maintenance
    Maintenance { reason: String },

    /// Error state
    Error { error: DeviceError, occurred_at: DateTime<Utc> },
}

/// Device hardware capabilities
#[derive(Debug, Clone)]
pub struct DeviceCapabilities {
    pub max_subcarriers: u16,
    pub max_antennas: u8,
    pub supported_bandwidths: Vec<Bandwidth>,
    pub supported_frequencies: Vec<FrequencyBand>,
    pub max_sampling_rate_hz: u32,
    pub supports_mimo: bool,
    pub supports_beamforming: bool,
}

/// Device configuration
#[derive(Debug, Clone)]
pub struct DeviceConfig {
    pub sampling_rate_hz: u32,
    pub subcarriers: u16,
    pub antennas: u8,
    pub bandwidth: Bandwidth,
    pub channel: WifiChannel,
    pub tx_power: Option<TxPower>,
    pub gain: Option<f32>,
}
```

### Value Objects

```rust
/// MAC address
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacAddress([u8; 6]);

impl MacAddress {
    pub fn parse(s: &str) -> Result<Self, DomainError> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 6 {
            return Err(DomainError::InvalidMacFormat);
        }

        let mut bytes = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            bytes[i] = u8::from_str_radix(part, 16)
                .map_err(|_| DomainError::InvalidMacFormat)?;
        }
        Ok(Self(bytes))
    }

    pub fn to_string(&self) -> String {
        format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

/// Device type enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceType {
    Esp32,
    Esp32S3,
    AtherosRouter,
    IntelNic5300,
    IntelNic5500,
    Nexmon,
    PicoScenes,
    Custom(String),
}

/// WiFi frequency band
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyBand {
    Band2_4GHz,
    Band5GHz,
    Band6GHz,
}

/// WiFi channel
#[derive(Debug, Clone, Copy)]
pub struct WifiChannel {
    pub number: u8,
    pub band: FrequencyBand,
}

impl WifiChannel {
    pub fn frequency(&self) -> Frequency {
        match self.band {
            FrequencyBand::Band2_4GHz => {
                // 2.4 GHz band: channels 1-14
                let base_mhz = 2412.0;
                let offset_mhz = (self.number as f64 - 1.0) * 5.0;
                Frequency::new((base_mhz + offset_mhz) * 1_000_000.0).unwrap()
            }
            FrequencyBand::Band5GHz => {
                // 5 GHz band: various channels
                let mhz = 5000.0 + (self.number as f64 * 5.0);
                Frequency::new(mhz * 1_000_000.0).unwrap()
            }
            FrequencyBand::Band6GHz => {
                // 6 GHz band
                let mhz = 5950.0 + (self.number as f64 * 5.0);
                Frequency::new(mhz * 1_000_000.0).unwrap()
            }
        }
    }
}
```

### Commands

```rust
impl Device {
    /// Create new device
    pub fn register(
        name: DeviceName,
        device_type: DeviceType,
        mac_address: MacAddress,
        capabilities: DeviceCapabilities,
    ) -> (Self, DeviceRegisteredEvent) {
        let now = Utc::now();
        let device = Self {
            id: DeviceId(Uuid::new_v4()),
            name: name.clone(),
            device_type: device_type.clone(),
            mac_address,
            ip_address: None,
            firmware_version: None,
            hardware_version: None,
            capabilities,
            location: None,
            zone_id: None,
            status: DeviceStatus::Disconnected,
            last_seen: None,
            health_checks: VecDeque::with_capacity(10),
            consecutive_failures: 0,
            config: DeviceConfig::default(),
            calibration: None,
            tags: HashSet::new(),
            custom_properties: HashMap::new(),
            created_at: now,
            updated_at: now,
        };

        let event = DeviceRegisteredEvent {
            device_id: device.id,
            name,
            device_type,
            mac_address,
            timestamp: now,
        };

        (device, event)
    }

    /// Connect to device
    pub fn connect(&mut self) -> Result<DeviceConnectingEvent, DomainError> {
        match &self.status {
            DeviceStatus::Disconnected | DeviceStatus::Error { .. } => {
                self.status = DeviceStatus::Connecting { started_at: Utc::now() };
                self.updated_at = Utc::now();

                Ok(DeviceConnectingEvent {
                    device_id: self.id,
                    timestamp: Utc::now(),
                })
            }
            _ => Err(DomainError::DeviceAlreadyConnected),
        }
    }

    /// Confirm connection established
    pub fn connection_established(&mut self) -> Result<DeviceConnectedEvent, DomainError> {
        match &self.status {
            DeviceStatus::Connecting { .. } => {
                let now = Utc::now();
                self.status = DeviceStatus::Connected { connected_at: now };
                self.last_seen = Some(now);
                self.consecutive_failures = 0;
                self.updated_at = now;

                Ok(DeviceConnectedEvent {
                    device_id: self.id,
                    timestamp: now,
                })
            }
            _ => Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Connected".to_string(),
            }),
        }
    }

    /// Start streaming CSI data
    pub fn start_streaming(&mut self) -> Result<DeviceStreamingStartedEvent, DomainError> {
        match &self.status {
            DeviceStatus::Connected { .. } => {
                let now = Utc::now();
                self.status = DeviceStatus::Streaming {
                    stream_started_at: now,
                    frames_sent: 0,
                };
                self.updated_at = now;

                Ok(DeviceStreamingStartedEvent {
                    device_id: self.id,
                    config: self.config.clone(),
                    timestamp: now,
                })
            }
            _ => Err(DomainError::DeviceNotConnected),
        }
    }

    /// Stop streaming
    pub fn stop_streaming(&mut self) -> Result<DeviceStreamingStoppedEvent, DomainError> {
        match &self.status {
            DeviceStatus::Streaming { frames_sent, .. } => {
                let frames = *frames_sent;
                let now = Utc::now();
                self.status = DeviceStatus::Connected { connected_at: now };
                self.updated_at = now;

                Ok(DeviceStreamingStoppedEvent {
                    device_id: self.id,
                    frames_sent: frames,
                    timestamp: now,
                })
            }
            _ => Err(DomainError::DeviceNotStreaming),
        }
    }

    /// Apply configuration
    pub fn configure(&mut self, config: DeviceConfig) -> Result<DeviceConfiguredEvent, DomainError> {
        // Validate config against capabilities
        if config.subcarriers > self.capabilities.max_subcarriers {
            return Err(DomainError::ConfigExceedsCapabilities {
                field: "subcarriers".to_string(),
            });
        }
        if config.antennas > self.capabilities.max_antennas {
            return Err(DomainError::ConfigExceedsCapabilities {
                field: "antennas".to_string(),
            });
        }
        if !self.capabilities.supported_bandwidths.contains(&config.bandwidth) {
            return Err(DomainError::UnsupportedBandwidth);
        }

        self.config = config.clone();
        self.updated_at = Utc::now();

        Ok(DeviceConfiguredEvent {
            device_id: self.id,
            config,
            timestamp: Utc::now(),
        })
    }

    /// Record health check result
    pub fn record_health_check(&mut self, result: HealthCheckResult) {
        // Keep last 10 checks
        if self.health_checks.len() >= 10 {
            self.health_checks.pop_front();
        }

        if result.is_healthy {
            self.consecutive_failures = 0;
        } else {
            self.consecutive_failures += 1;
        }

        self.health_checks.push_back(result);
        self.last_seen = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    // Queries

    pub fn is_healthy(&self) -> bool {
        self.consecutive_failures < 3 && !matches!(self.status, DeviceStatus::Error { .. })
    }

    pub fn is_streaming(&self) -> bool {
        matches!(self.status, DeviceStatus::Streaming { .. })
    }

    pub fn uptime(&self) -> Option<Duration> {
        match &self.status {
            DeviceStatus::Connected { connected_at } |
            DeviceStatus::Streaming { stream_started_at: connected_at, .. } => {
                Some((Utc::now() - *connected_at).to_std().unwrap_or_default())
            }
            _ => None,
        }
    }
}
```

---

## Cross-Aggregate References

Aggregates reference each other by ID only, never by direct object reference:

```rust
// Correct: Reference by ID
pub struct CsiFrame {
    device_id: DeviceId,      // ID only
    session_id: Option<SessionId>,  // ID only
}

// Incorrect: Direct reference (never do this)
pub struct CsiFrame {
    device: Device,           // WRONG: Creates coupling
    session: Option<Session>, // WRONG: Violates boundary
}
```

## Repository Pattern

Each aggregate root has a corresponding repository interface:

```rust
#[async_trait]
pub trait AggregateRepository<A, ID> {
    async fn find_by_id(&self, id: &ID) -> Result<Option<A>, RepositoryError>;
    async fn save(&self, aggregate: &A) -> Result<(), RepositoryError>;
    async fn delete(&self, id: &ID) -> Result<bool, RepositoryError>;
}
```
