# Domain-Driven Design: WiFi-DensePose Domain Model

## Bounded Contexts

### 1. Signal Domain
**Purpose**: Raw CSI data acquisition and preprocessing

**Aggregates**:
- `CsiFrame`: Raw CSI measurement from WiFi hardware
- `ProcessedSignal`: Cleaned and feature-extracted signal

**Value Objects**:
- `Amplitude`: Signal strength measurements
- `Phase`: Phase angle measurements
- `SubcarrierData`: Per-subcarrier information
- `Timestamp`: Measurement timing

**Domain Services**:
- `CsiProcessor`: Preprocesses raw CSI data
- `PhaseSanitizer`: Unwraps and cleans phase data
- `FeatureExtractor`: Extracts signal features

### 2. Pose Domain
**Purpose**: Human pose estimation from processed signals

**Aggregates**:
- `PoseEstimate`: Complete DensePose output
- `InferenceSession`: Neural network session state

**Value Objects**:
- `BodyPart`: Labeled body segment (torso, arms, legs, etc.)
- `UVCoordinate`: Surface mapping coordinate
- `Keypoint`: Body joint position
- `Confidence`: Prediction confidence score

**Domain Services**:
- `ModalityTranslator`: CSI → visual feature translation
- `DensePoseHead`: Body part segmentation and UV regression

### 3. Streaming Domain
**Purpose**: Real-time data delivery to clients

**Aggregates**:
- `Session`: Client connection with history
- `StreamConfig`: Client streaming preferences

**Value Objects**:
- `WebSocketMessage`: Typed message payload
- `ConnectionState`: Active/idle/disconnected

**Domain Services**:
- `StreamManager`: Manages client connections
- `BroadcastService`: Pushes updates to subscribers

### 4. Storage Domain
**Purpose**: Persistence and retrieval

**Aggregates**:
- `Recording`: Captured CSI session
- `ModelArtifact`: Neural network weights

**Repositories**:
- `SessionRepository`: Session CRUD operations
- `RecordingRepository`: Recording storage
- `ModelRepository`: Model management

### 5. Hardware Domain
**Purpose**: Physical device management

**Aggregates**:
- `Device`: WiFi router/receiver
- `Antenna`: Individual antenna configuration

**Domain Services**:
- `DeviceManager`: Device discovery and control
- `CsiExtractor`: Raw CSI extraction

## Context Map

```
┌─────────────────────────────────────────────────────────────┐
│                      WiFi-DensePose                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐     ┌──────────────┐     ┌─────────────┐ │
│  │   Hardware   │────▶│    Signal    │────▶│    Pose     │ │
│  │   Domain     │     │    Domain    │     │   Domain    │ │
│  └──────────────┘     └──────────────┘     └─────────────┘ │
│         │                    │                    │        │
│         │                    │                    │        │
│         ▼                    ▼                    ▼        │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                   Storage Domain                      │  │
│  └──────────────────────────────────────────────────────┘  │
│         │                    │                    │        │
│         ▼                    ▼                    ▼        │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                  Streaming Domain                     │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Ubiquitous Language

| Term | Definition |
|------|------------|
| CSI | Channel State Information - WiFi signal properties |
| Subcarrier | Individual frequency component in OFDM |
| Phase Unwrapping | Correcting 2π phase discontinuities |
| DensePose | Dense human pose estimation with UV mapping |
| Modality Translation | Converting CSI features to visual features |
| Body Part | One of 15 labeled human body segments |
| UV Mapping | 2D surface parameterization of 3D body |
