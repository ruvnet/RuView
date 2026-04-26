# Ubiquitous Language

This glossary defines the domain terminology used throughout the WiFi-DensePose system. All team members (developers, domain experts, stakeholders) should use these terms consistently in code, documentation, and conversation.

---

## Core Concepts

### WiFi-DensePose

The system that uses WiFi signals to perform non-invasive human pose estimation. Unlike camera-based systems, it operates through walls and in darkness, providing privacy-preserving body tracking.

### Channel State Information (CSI)

The detailed information about how a WiFi signal propagates between transmitter and receiver. CSI captures amplitude and phase changes across multiple subcarriers and antennas, encoding environmental information including human presence and movement.

### DensePose

A computer vision technique that maps all pixels of a detected human body to a 3D surface representation. In our context, we translate WiFi signals into DensePose-compatible outputs.

### Pose Estimation

The process of determining the position and orientation of a human body, typically by identifying anatomical landmarks (keypoints) and body segments.

---

## Signal Domain Terms

### Amplitude

The magnitude (strength) of the CSI measurement for a specific subcarrier and antenna pair. Amplitude variations indicate physical changes in the environment, particularly human movement.

**Units:** Linear scale or decibels (dB)

**Example Usage:**
```rust
let amplitude = csi_frame.amplitude(); // Matrix of amplitude values
```

### Phase

The timing offset of the WiFi signal, measured in radians. Phase is highly sensitive to distance changes and is crucial for detecting subtle movements like breathing.

**Units:** Radians (-pi to pi)

**Note:** Raw phase requires sanitization (unwrapping, noise removal) before use.

### Subcarrier

An individual frequency component within an OFDM (Orthogonal Frequency-Division Multiplexing) WiFi signal. Each subcarrier provides an independent measurement of the channel state.

**Typical Values:**
- 20 MHz bandwidth: 56 subcarriers
- 40 MHz bandwidth: 114 subcarriers
- 80 MHz bandwidth: 242 subcarriers

### Antenna

A physical receiver element on the WiFi device. Multiple antennas enable MIMO (Multiple-Input Multiple-Output) and provide spatial diversity in CSI measurements.

**Typical Configurations:** 1x1, 2x2, 3x3, 4x4

### Signal-to-Noise Ratio (SNR)

A quality metric measuring the strength of the desired signal relative to background noise. Higher SNR indicates cleaner, more reliable CSI data.

**Units:** Decibels (dB)

**Quality Thresholds:**
- SNR < 10 dB: Poor quality, may be unusable
- SNR 10-20 dB: Acceptable quality
- SNR > 20 dB: Good quality

### Noise Floor

The ambient electromagnetic interference level in the environment. The noise floor limits the minimum detectable signal.

**Units:** dBm (decibels relative to milliwatt)

### Doppler Shift

A frequency change caused by moving objects. The Doppler effect in CSI reveals motion velocity and direction.

**Formula:** fd = (2 * v * f) / c

Where v is velocity, f is carrier frequency, c is speed of light.

### Power Spectral Density (PSD)

The distribution of signal power across frequencies. PSD analysis reveals periodic motions like walking or breathing.

**Units:** dB/Hz

### Feature Extraction

The process of computing meaningful statistics and transformations from raw CSI data. Features include amplitude mean/variance, phase differences, correlations, and frequency-domain characteristics.

### Preprocessing

Initial signal conditioning including:
- **Noise removal** - Filtering out low-quality measurements
- **Windowing** - Applying window functions (Hamming, Hann) to reduce spectral leakage
- **Normalization** - Scaling values to standard ranges
- **Phase sanitization** - Unwrapping and smoothing phase data

---

## Pose Domain Terms

### Modality Translation

The core innovation of WiFi-DensePose: converting radio frequency (RF) features into visual-like feature representations that can be processed by pose estimation models.

**Also Known As:** Cross-modal learning, RF-to-vision translation

### Human Presence Detection

Binary classification determining whether one or more humans are present in the sensing area. This is typically the first stage of the pose estimation pipeline.

### Person Count

The estimated number of individuals in the detection zone. Accurate counting is challenging with WiFi sensing due to signal superposition.

### Keypoint

A named anatomical landmark on the human body. WiFi-DensePose uses the COCO keypoint format with 17 points:

| Index | Name | Description |
|-------|------|-------------|
| 0 | Nose | Tip of nose |
| 1 | Left Eye | Center of left eye |
| 2 | Right Eye | Center of right eye |
| 3 | Left Ear | Left ear |
| 4 | Right Ear | Right ear |
| 5 | Left Shoulder | Left shoulder joint |
| 6 | Right Shoulder | Right shoulder joint |
| 7 | Left Elbow | Left elbow joint |
| 8 | Right Elbow | Right elbow joint |
| 9 | Left Wrist | Left wrist |
| 10 | Right Wrist | Right wrist |
| 11 | Left Hip | Left hip joint |
| 12 | Right Hip | Right hip joint |
| 13 | Left Knee | Left knee joint |
| 14 | Right Knee | Right knee joint |
| 15 | Left Ankle | Left ankle |
| 16 | Right Ankle | Right ankle |

### Body Part

A segmented region of the human body. DensePose defines 24 body parts:

| ID | Part | ID | Part |
|----|------|----|------|
| 1 | Torso | 13 | Left Lower Leg |
| 2 | Right Hand | 14 | Right Lower Leg |
| 3 | Left Hand | 15 | Left Foot |
| 4 | Right Foot | 16 | Right Foot |
| 5 | Left Foot | 17 | Right Upper Arm Back |
| 6 | Right Upper Arm Front | 18 | Left Upper Arm Back |
| 7 | Left Upper Arm Front | 19 | Right Lower Arm Back |
| 8 | Right Lower Arm Front | 20 | Left Lower Arm Back |
| 9 | Left Lower Arm Front | 21 | Right Upper Leg Back |
| 10 | Right Upper Leg Front | 22 | Left Upper Leg Back |
| 11 | Left Upper Leg Front | 23 | Right Lower Leg Back |
| 12 | Right Lower Leg Front | 24 | Left Lower Leg Back |

### UV Coordinates

A 2D parameterization of the body surface. U and V are continuous coordinates (0-1) that map any point on the body to a canonical 3D mesh.

**Purpose:** Enable consistent body surface representation regardless of pose.

### Bounding Box

A rectangular region in the detection space that encloses a detected person.

**Format:** (x, y, width, height) in normalized coordinates [0, 1]

### Confidence Score

A probability value [0, 1] indicating the model's certainty in a detection or classification. Higher values indicate greater confidence.

**Thresholds:**
- Low: < 0.5
- Medium: 0.5 - 0.8
- High: > 0.8

### Activity

A high-level classification of what a person is doing:

| Activity | Description |
|----------|-------------|
| Standing | Upright, stationary |
| Sitting | Seated position |
| Walking | Ambulatory movement |
| Running | Fast ambulatory movement |
| Lying | Horizontal position |
| Falling | Rapid transition to ground |
| Unknown | Unclassified activity |

### Fall Detection

Identification of a fall event, typically characterized by:
1. Rapid vertical velocity
2. Horizontal final position
3. Sudden deceleration (impact)
4. Subsequent immobility

**Critical Use Case:** Elderly care, healthcare facilities

### Motion Detection

Recognition of significant movement in the sensing area. Motion is detected through:
- CSI amplitude/phase variance
- Doppler shift analysis
- Temporal feature changes

---

## Streaming Domain Terms

### Session

A client connection for real-time data streaming. A session has a lifecycle: connecting, active, paused, reconnecting, completed, failed.

### Stream Type

The category of data being streamed:

| Type | Data Content |
|------|--------------|
| Pose | Pose estimation results |
| CSI | Raw or processed CSI data |
| Alerts | Critical events (falls, motion) |
| Status | System health and metrics |

### Zone

A logical or physical area for filtering and organizing detections. Zones enable:
- Multi-room coverage with single system
- Per-area subscriptions
- Location-aware alerting

### Subscription

A client's expressed interest in receiving specific data. Subscriptions include:
- Stream types
- Zone filters
- Confidence thresholds
- Throttling preferences

### Broadcast

Sending data to all clients matching subscription criteria.

### Heartbeat

A periodic ping message to verify connection liveness. Clients that fail to respond to heartbeats are disconnected.

### Backpressure

Flow control mechanism when a client cannot process messages fast enough. Options include:
- Buffering (limited)
- Dropping frames
- Throttling source

### Latency

The time delay between event occurrence and client receipt. Measured in milliseconds.

**Target:** < 100ms for real-time applications

---

## Hardware Domain Terms

### Device

A physical WiFi hardware unit capable of CSI extraction. Supported types:

| Type | Description |
|------|-------------|
| ESP32 | Low-cost microcontroller with WiFi |
| Atheros Router | Router with modified firmware |
| Intel NIC | Intel 5300/5500 network cards |
| Nexmon | Broadcom chips with Nexmon firmware |
| PicoScenes | Research-grade CSI platform |

### MAC Address

Media Access Control address - a unique hardware identifier for network interfaces.

**Format:** XX:XX:XX:XX:XX:XX (hexadecimal)

### Firmware

Software running on the WiFi device that enables CSI extraction.

### Calibration

The process of tuning a device for optimal CSI quality:
1. Measure noise floor
2. Compute antenna phase offsets
3. Establish baseline signal characteristics

### Health Check

Periodic verification that a device is functioning correctly. Checks include:
- Connectivity
- Data rate
- Error rate
- Temperature (if available)

---

## Storage Domain Terms

### Repository

An interface for persisting and retrieving aggregate roots. Each aggregate type has its own repository.

**Pattern:** Repository pattern from Domain-Driven Design

### Entity

An object with a distinct identity that persists over time. Entities are equal if their identifiers match.

**Examples:** Device, Session, CsiFrame

### Value Object

An object defined by its attributes rather than identity. Value objects are immutable and equal if all attributes match.

**Examples:** Frequency, Confidence, MacAddress

### Aggregate

A cluster of entities and value objects treated as a single unit. One entity is the aggregate root; all access goes through it.

### Event Store

A persistence mechanism that stores domain events as the source of truth. Supports event sourcing and audit trails.

---

## Cross-Cutting Terms

### Bounded Context

A logical boundary within which a particular domain model is defined and applicable. Each bounded context has its own ubiquitous language.

**WiFi-DensePose Contexts:**
1. Signal (CSI processing)
2. Pose (inference)
3. Streaming (real-time delivery)
4. Storage (persistence)
5. Hardware (device management)

### Domain Event

A record of something significant that happened in the domain. Events are immutable and named in past tense.

**Examples:** CsiFrameReceived, PoseEstimated, FallDetected

### Command

A request to perform an action that may change system state.

**Examples:** ProcessCsiFrame, EstimatePose, ConnectDevice

### Query

A request for information that does not change state.

**Examples:** GetCurrentPose, GetDeviceStatus, GetSessionHistory

### Correlation ID

A unique identifier that links related events across the system, enabling end-to-end tracing.

---

## Metrics and Quality Terms

### Throughput

The rate of data processing, typically measured in:
- Frames per second (FPS) for CSI
- Poses per second for inference
- Messages per second for streaming

### Processing Time

The duration to complete a processing step. Measured in milliseconds.

### Accuracy

How closely estimates match ground truth. For pose estimation:
- OKS (Object Keypoint Similarity) for keypoints
- IoU (Intersection over Union) for bounding boxes

### Precision

The proportion of positive detections that are correct.

**Formula:** TP / (TP + FP)

### Recall

The proportion of actual positives that are detected.

**Formula:** TP / (TP + FN)

### F1 Score

Harmonic mean of precision and recall.

**Formula:** 2 * (Precision * Recall) / (Precision + Recall)

---

## Acronyms

| Acronym | Expansion |
|---------|-----------|
| API | Application Programming Interface |
| CQRS | Command Query Responsibility Segregation |
| CSI | Channel State Information |
| dB | Decibel |
| dBm | Decibel-milliwatt |
| DDD | Domain-Driven Design |
| FPS | Frames Per Second |
| Hz | Hertz (cycles per second) |
| IoU | Intersection over Union |
| MAC | Media Access Control |
| MIMO | Multiple-Input Multiple-Output |
| OFDM | Orthogonal Frequency-Division Multiplexing |
| OKS | Object Keypoint Similarity |
| PSD | Power Spectral Density |
| RF | Radio Frequency |
| RSSI | Received Signal Strength Indicator |
| SNR | Signal-to-Noise Ratio |
| UUID | Universally Unique Identifier |
| UV | Texture mapping coordinates |
| VO | Value Object |
| WiFi | Wireless Fidelity (IEEE 802.11) |
| WS | WebSocket |

---

## Usage Guidelines

### In Code

Use exact terms from this glossary:

```rust
// Good: Uses ubiquitous language
pub struct CsiFrame { ... }
pub fn detect_human_presence(&self) -> HumanPresenceResult { ... }
pub fn estimate_pose(&self) -> PoseEstimate { ... }

// Bad: Non-standard terminology
pub struct WifiData { ... }  // Should be CsiFrame
pub fn find_people(&self) { ... }  // Should be detect_human_presence
pub fn get_body_position(&self) { ... }  // Should be estimate_pose
```

### In Documentation

Always use defined terms; avoid synonyms that could cause confusion.

### In Conversation

When discussing the system, use these terms consistently to ensure clear communication between technical and domain experts.

---

## Term Evolution

This glossary is a living document. To propose changes:

1. Discuss with domain experts and team
2. Update this document
3. Update code to reflect new terminology
4. Update all related documentation
