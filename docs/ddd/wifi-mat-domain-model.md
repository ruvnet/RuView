# WiFi-Mat Domain Model

## Domain-Driven Design Specification

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Survivor** | A human detected within a scan zone, potentially trapped |
| **Vital Signs** | Detectable life indicators: breathing, heartbeat, movement |
| **Scan Zone** | A defined geographic area being actively monitored |
| **Detection Event** | An occurrence of vital signs being detected |
| **Triage Status** | Medical priority classification (Immediate/Delayed/Minor/Deceased) |
| **Confidence Score** | Statistical certainty of detection (0.0-1.0) |
| **Penetration Depth** | Estimated distance through debris to survivor |
| **Debris Field** | Collection of materials between sensor and survivor |

---

## Bounded Contexts

### 1. Detection Context

**Responsibility**: Analyze CSI data to detect and classify human vital signs

```
┌─────────────────────────────────────────────────────────┐
│                  Detection Context                       │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐    ┌──────────────┐                   │
│  │   Breathing  │    │  Heartbeat   │                   │
│  │   Detector   │    │  Detector    │                   │
│  └──────┬───────┘    └──────┬───────┘                   │
│         │                   │                            │
│         └─────────┬─────────┘                           │
│                   ▼                                      │
│         ┌─────────────────┐                             │
│         │    Movement     │                             │
│         │   Classifier    │                             │
│         └────────┬────────┘                             │
│                  ▼                                       │
│         ┌─────────────────┐                             │
│         │    Ensemble     │──▶ VitalSignsReading        │
│         │   Classifier    │                             │
│         └─────────────────┘                             │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

**Aggregates**:
- `VitalSignsReading` (Aggregate Root)

**Value Objects**:
- `BreathingPattern`
- `HeartbeatSignature`
- `MovementProfile`
- `ConfidenceScore`

### 2. Localization Context

**Responsibility**: Estimate survivor position within debris field

```
┌─────────────────────────────────────────────────────────┐
│                 Localization Context                     │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐    ┌──────────────┐                   │
│  │Triangulation │    │Fingerprinting│                   │
│  │   Engine     │    │   Matcher    │                   │
│  └──────┬───────┘    └──────┬───────┘                   │
│         │                   │                            │
│         └─────────┬─────────┘                           │
│                   ▼                                      │
│         ┌─────────────────┐                             │
│         │     Depth       │                             │
│         │   Estimator     │                             │
│         └────────┬────────┘                             │
│                  ▼                                       │
│         ┌─────────────────┐                             │
│         │   Position      │──▶ SurvivorLocation         │
│         │    Fuser        │                             │
│         └─────────────────┘                             │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

**Aggregates**:
- `SurvivorLocation` (Aggregate Root)

**Value Objects**:
- `Coordinates3D`
- `DepthEstimate`
- `LocationUncertainty`
- `DebrisProfile`

### 3. Alerting Context

**Responsibility**: Generate and dispatch alerts based on detections

```
┌─────────────────────────────────────────────────────────┐
│                   Alerting Context                       │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐    ┌──────────────┐                   │
│  │   Triage     │    │   Alert      │                   │
│  │  Calculator  │    │  Generator   │                   │
│  └──────┬───────┘    └──────┬───────┘                   │
│         │                   │                            │
│         └─────────┬─────────┘                           │
│                   ▼                                      │
│         ┌─────────────────┐                             │
│         │   Dispatcher    │──▶ Alert                    │
│         └─────────────────┘                             │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

**Aggregates**:
- `Alert` (Aggregate Root)

**Value Objects**:
- `TriageStatus`
- `Priority`
- `AlertPayload`

---

## Core Domain Entities

### Survivor (Entity)

```rust
pub struct Survivor {
    id: SurvivorId,
    detection_time: DateTime<Utc>,
    location: Option<SurvivorLocation>,
    vital_signs: VitalSignsHistory,
    triage_status: TriageStatus,
    confidence: ConfidenceScore,
    metadata: SurvivorMetadata,
}
```

**Invariants**:
- Must have at least one vital sign detection to exist
- Triage status must be recalculated on each vital sign update
- Confidence must be >= 0.3 to be considered valid detection

### DisasterEvent (Aggregate Root)

```rust
pub struct DisasterEvent {
    id: DisasterEventId,
    event_type: DisasterType,
    start_time: DateTime<Utc>,
    location: GeoLocation,
    scan_zones: Vec<ScanZone>,
    survivors: Vec<Survivor>,
    status: EventStatus,
}
```

**Invariants**:
- Must have at least one scan zone
- All survivors must be within a scan zone
- Cannot add survivors after event is closed

### ScanZone (Entity)

```rust
pub struct ScanZone {
    id: ScanZoneId,
    bounds: ZoneBounds,
    sensor_positions: Vec<SensorPosition>,
    scan_parameters: ScanParameters,
    status: ZoneStatus,
    last_scan: DateTime<Utc>,
}
```

---

## Value Objects

### VitalSignsReading

```rust
pub struct VitalSignsReading {
    breathing: Option<BreathingPattern>,
    heartbeat: Option<HeartbeatSignature>,
    movement: MovementProfile,
    timestamp: DateTime<Utc>,
    confidence: ConfidenceScore,
}
```

### TriageStatus (Enumeration)

```rust
pub enum TriageStatus {
    /// Immediate - Life-threatening, requires immediate intervention
    Immediate,  // Red tag

    /// Delayed - Serious but can wait for treatment
    Delayed,    // Yellow tag

    /// Minor - Walking wounded, minimal treatment needed
    Minor,      // Green tag

    /// Deceased - No vital signs detected over threshold period
    Deceased,   // Black tag

    /// Unknown - Insufficient data for classification
    Unknown,
}
```

### BreathingPattern

```rust
pub struct BreathingPattern {
    rate_bpm: f32,           // Breaths per minute (normal: 12-20)
    amplitude: f32,          // Signal strength
    regularity: f32,         // 0.0-1.0, consistency of pattern
    pattern_type: BreathingType,
}

pub enum BreathingType {
    Normal,
    Shallow,
    Labored,
    Irregular,
    Agonal,
}
```

### HeartbeatSignature

```rust
pub struct HeartbeatSignature {
    rate_bpm: f32,           // Beats per minute (normal: 60-100)
    variability: f32,        // Heart rate variability
    strength: SignalStrength,
}
```

### Coordinates3D

```rust
pub struct Coordinates3D {
    x: f64,  // East-West offset from reference (meters)
    y: f64,  // North-South offset from reference (meters)
    z: f64,  // Depth below surface (meters, negative = below)
    uncertainty: LocationUncertainty,
}

pub struct LocationUncertainty {
    horizontal_error: f64,  // meters (95% confidence)
    vertical_error: f64,    // meters (95% confidence)
}
```

---

## Domain Events

### Detection Events

```rust
pub enum DetectionEvent {
    /// New survivor detected
    SurvivorDetected {
        survivor_id: SurvivorId,
        zone_id: ScanZoneId,
        vital_signs: VitalSignsReading,
        location: Option<Coordinates3D>,
        timestamp: DateTime<Utc>,
    },

    /// Survivor vital signs updated
    VitalsUpdated {
        survivor_id: SurvivorId,
        previous: VitalSignsReading,
        current: VitalSignsReading,
        timestamp: DateTime<Utc>,
    },

    /// Survivor triage status changed
    TriageStatusChanged {
        survivor_id: SurvivorId,
        previous: TriageStatus,
        current: TriageStatus,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    /// Survivor location refined
    LocationRefined {
        survivor_id: SurvivorId,
        previous: Coordinates3D,
        current: Coordinates3D,
        timestamp: DateTime<Utc>,
    },

    /// Survivor no longer detected (may have been rescued or false positive)
    SurvivorLost {
        survivor_id: SurvivorId,
        last_detection: DateTime<Utc>,
        reason: LostReason,
    },
}

pub enum LostReason {
    Rescued,
    FalsePositive,
    SignalLost,
    ZoneDeactivated,
}
```

### Alert Events

```rust
pub enum AlertEvent {
    /// New alert generated
    AlertGenerated {
        alert_id: AlertId,
        survivor_id: SurvivorId,
        priority: Priority,
        payload: AlertPayload,
    },

    /// Alert acknowledged by rescue team
    AlertAcknowledged {
        alert_id: AlertId,
        acknowledged_by: TeamId,
        timestamp: DateTime<Utc>,
    },

    /// Alert resolved
    AlertResolved {
        alert_id: AlertId,
        resolution: AlertResolution,
        timestamp: DateTime<Utc>,
    },
}
```

---

## Domain Services

### TriageService

Calculates triage status based on vital signs using START protocol:

```rust
pub trait TriageService {
    fn calculate_triage(&self, vitals: &VitalSignsReading) -> TriageStatus;
    fn should_upgrade_priority(&self, history: &VitalSignsHistory) -> bool;
}
```

**Rules**:
1. No breathing detected → Check for movement
2. Movement but no breathing → Immediate (airway issue)
3. Breathing > 30/min → Immediate
4. Breathing < 10/min → Immediate
5. No radial pulse equivalent (weak heartbeat) → Immediate
6. Cannot follow commands (no responsive movement) → Immediate
7. Otherwise → Delayed or Minor based on severity

### LocalizationService

Fuses multiple localization techniques:

```rust
pub trait LocalizationService {
    fn estimate_position(
        &self,
        csi_data: &[CsiReading],
        sensor_positions: &[SensorPosition],
    ) -> Result<Coordinates3D, LocalizationError>;

    fn estimate_depth(
        &self,
        signal_attenuation: f64,
        debris_profile: &DebrisProfile,
    ) -> Result<DepthEstimate, LocalizationError>;
}
```

---

## Context Map

```
┌────────────────────────────────────────────────────────────────┐
│                        WiFi-Mat System                          │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐         ┌─────────────┐                       │
│  │  Detection  │◄───────►│ Localization│                       │
│  │   Context   │ Partner │   Context   │                       │
│  └──────┬──────┘         └──────┬──────┘                       │
│         │                       │                               │
│         │ Publishes             │ Publishes                     │
│         ▼                       ▼                               │
│  ┌─────────────────────────────────────┐                       │
│  │         Event Bus (Domain Events)    │                       │
│  └─────────────────┬───────────────────┘                       │
│                    │                                            │
│                    │ Subscribes                                 │
│                    ▼                                            │
│            ┌─────────────┐                                      │
│            │  Alerting   │                                      │
│            │   Context   │                                      │
│            └─────────────┘                                      │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│                    UPSTREAM (Conformist)                        │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐       │
│  │wifi-densepose │  │wifi-densepose │  │wifi-densepose │       │
│  │    -signal    │  │     -nn       │  │   -hardware   │       │
│  └───────────────┘  └───────────────┘  └───────────────┘       │
└─────────────────────────────────────────────────────────────────┘
```

**Relationship Types**:
- Detection ↔ Localization: **Partnership** (tight collaboration)
- Detection → Alerting: **Customer/Supplier** (Detection publishes, Alerting consumes)
- WiFi-Mat → Upstream crates: **Conformist** (adapts to their models)

---

## Anti-Corruption Layer

The integration module provides adapters to translate between upstream crate models and WiFi-Mat domain:

```rust
/// Adapts wifi-densepose-signal types to Detection context
pub struct SignalAdapter {
    processor: CsiProcessor,
    feature_extractor: FeatureExtractor,
}

impl SignalAdapter {
    pub fn extract_vital_features(
        &self,
        raw_csi: &[Complex<f64>],
    ) -> Result<VitalFeatures, AdapterError>;
}

/// Adapts wifi-densepose-nn for specialized detection models
pub struct NeuralAdapter {
    breathing_model: OnnxModel,
    heartbeat_model: OnnxModel,
}

impl NeuralAdapter {
    pub fn classify_breathing(
        &self,
        features: &VitalFeatures,
    ) -> Result<BreathingPattern, AdapterError>;
}
```

---

## Repository Interfaces

```rust
#[async_trait]
pub trait SurvivorRepository {
    async fn save(&self, survivor: &Survivor) -> Result<(), RepositoryError>;
    async fn find_by_id(&self, id: &SurvivorId) -> Result<Option<Survivor>, RepositoryError>;
    async fn find_by_zone(&self, zone_id: &ScanZoneId) -> Result<Vec<Survivor>, RepositoryError>;
    async fn find_active(&self) -> Result<Vec<Survivor>, RepositoryError>;
}

#[async_trait]
pub trait DisasterEventRepository {
    async fn save(&self, event: &DisasterEvent) -> Result<(), RepositoryError>;
    async fn find_active(&self) -> Result<Vec<DisasterEvent>, RepositoryError>;
    async fn find_by_location(&self, location: &GeoLocation, radius_km: f64) -> Result<Vec<DisasterEvent>, RepositoryError>;
}

#[async_trait]
pub trait AlertRepository {
    async fn save(&self, alert: &Alert) -> Result<(), RepositoryError>;
    async fn find_pending(&self) -> Result<Vec<Alert>, RepositoryError>;
    async fn find_by_survivor(&self, survivor_id: &SurvivorId) -> Result<Vec<Alert>, RepositoryError>;
}
```
