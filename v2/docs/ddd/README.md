# WiFi-DensePose Domain-Driven Design Documentation

## Overview

This documentation describes the Domain-Driven Design (DDD) architecture for the WiFi-DensePose Rust port. The system uses WiFi Channel State Information (CSI) to perform non-invasive human pose estimation, translating radio frequency signals into body positioning data.

## Strategic Design

### Core Domain

The **Pose Estimation Domain** represents the core business logic that provides unique value. This domain translates WiFi CSI signals into DensePose-compatible human body representations. The algorithms for modality translation (RF to visual features) and pose inference constitute the competitive advantage of the system.

### Supporting Domains

1. **Signal Domain** - CSI acquisition and preprocessing
2. **Streaming Domain** - Real-time data delivery infrastructure
3. **Storage Domain** - Persistence and retrieval mechanisms
4. **Hardware Domain** - Device abstraction and management

### Generic Domains

- Authentication and authorization
- Logging and monitoring
- Configuration management

## Tactical Design Patterns

### Aggregates

Each bounded context contains aggregates that enforce invariants and maintain consistency:

- **CsiFrame** - Raw signal data with validation rules
- **ProcessedSignal** - Feature-extracted signal ready for inference
- **PoseEstimate** - Inference results with confidence scoring
- **Session** - Client connection lifecycle management
- **Device** - Hardware abstraction with state machine

### Domain Events

Events flow between bounded contexts through an event-driven architecture:

```
CsiFrameReceived -> SignalProcessed -> PoseEstimated -> (MotionDetected | FallDetected)
```

### Repositories

Each aggregate root has a corresponding repository for persistence:

- `CsiFrameRepository`
- `SessionRepository`
- `DeviceRepository`
- `PoseEstimateRepository`

### Domain Services

Cross-aggregate operations are handled by domain services:

- `PoseEstimationService` - Orchestrates CSI-to-pose pipeline
- `CalibrationService` - Hardware calibration workflows
- `AlertService` - Motion and fall detection alerts

## Context Map

```
                    +------------------+
                    |  Pose Domain     |
                    |  (Core Domain)   |
                    +--------+---------+
                             |
              +--------------+---------------+
              |              |               |
    +---------v----+  +------v------+  +-----v-------+
    | Signal Domain|  | Streaming   |  | Storage     |
    | (Upstream)   |  | Domain      |  | Domain      |
    +---------+----+  +------+------+  +------+------+
              |              |                |
              +--------------+----------------+
                             |
                    +--------v--------+
                    | Hardware Domain |
                    | (Foundation)    |
                    +-----------------+
```

### Relationships

| Upstream | Downstream | Relationship |
|----------|------------|--------------|
| Hardware | Signal | Conformist |
| Signal | Pose | Customer-Supplier |
| Pose | Streaming | Published Language |
| Pose | Storage | Shared Kernel |

## Architecture Principles

### 1. Hexagonal Architecture

Each bounded context follows hexagonal (ports and adapters) architecture:

```
                    +--------------------+
                    |    Application     |
                    |      Services      |
                    +---------+----------+
                              |
              +---------------+---------------+
              |                               |
    +---------v---------+           +---------v---------+
    |   Domain Layer    |           |   Domain Layer    |
    |  (Entities, VOs,  |           |   (Aggregates,    |
    |   Domain Events)  |           |    Repositories)  |
    +---------+---------+           +---------+---------+
              |                               |
    +---------v---------+           +---------v---------+
    | Infrastructure    |           | Infrastructure    |
    | (Adapters: DB,    |           | (Adapters: API,   |
    |  Hardware, MQ)    |           |  WebSocket)       |
    +-------------------+           +-------------------+
```

### 2. CQRS (Command Query Responsibility Segregation)

The system separates read and write operations:

- **Commands**: `ProcessCsiFrame`, `CreateSession`, `UpdateDeviceConfig`
- **Queries**: `GetCurrentPose`, `GetSessionHistory`, `GetDeviceStatus`

### 3. Event Sourcing (Optional)

For audit and replay capabilities, CSI processing events can be stored as an event log:

```rust
pub enum DomainEvent {
    CsiFrameReceived(CsiFrameReceivedEvent),
    SignalProcessed(SignalProcessedEvent),
    PoseEstimated(PoseEstimatedEvent),
    MotionDetected(MotionDetectedEvent),
    FallDetected(FallDetectedEvent),
}
```

## Rust Implementation Guidelines

### Module Structure

```
wifi-densepose-rs/
  crates/
    wifi-densepose-core/         # Shared kernel
      src/
        domain/
          entities/
          value_objects/
          events/
    wifi-densepose-signal/       # Signal bounded context
      src/
        domain/
        application/
        infrastructure/
    wifi-densepose-nn/           # Pose bounded context
      src/
        domain/
        application/
        infrastructure/
    wifi-densepose-api/          # Streaming bounded context
      src/
        domain/
        application/
        infrastructure/
    wifi-densepose-db/           # Storage bounded context
      src/
        domain/
        application/
        infrastructure/
    wifi-densepose-hardware/     # Hardware bounded context
      src/
        domain/
        application/
        infrastructure/
```

### Type-Driven Design

Leverage Rust's type system to encode domain invariants:

```rust
// Newtype pattern for domain identifiers
pub struct DeviceId(Uuid);
pub struct SessionId(Uuid);
pub struct FrameId(u64);

// State machines via enums
pub enum DeviceState {
    Disconnected,
    Connecting(ConnectionAttempt),
    Connected(ActiveConnection),
    Streaming(StreamingSession),
    Error(DeviceError),
}

// Validated value objects
pub struct Frequency {
    hz: f64, // Invariant: always > 0
}

impl Frequency {
    pub fn new(hz: f64) -> Result<Self, DomainError> {
        if hz <= 0.0 {
            return Err(DomainError::InvalidFrequency);
        }
        Ok(Self { hz })
    }
}
```

### Error Handling

Domain errors are distinct from infrastructure errors:

```rust
#[derive(Debug, thiserror::Error)]
pub enum SignalDomainError {
    #[error("Invalid CSI frame: {0}")]
    InvalidFrame(String),

    #[error("Signal quality below threshold: {snr} dB")]
    LowSignalQuality { snr: f64 },

    #[error("Calibration required for device {device_id}")]
    CalibrationRequired { device_id: DeviceId },
}
```

## Testing Strategy

### Unit Tests
- Value object invariants
- Aggregate business rules
- Domain service logic

### Integration Tests
- Repository implementations
- Inter-context communication
- Event publishing/subscription

### Property-Based Tests
- Signal processing algorithms
- Pose estimation accuracy
- Event ordering guarantees

## References

- Evans, Eric. *Domain-Driven Design: Tackling Complexity in the Heart of Software*. Addison-Wesley, 2003.
- Vernon, Vaughn. *Implementing Domain-Driven Design*. Addison-Wesley, 2013.
- Millett, Scott and Tune, Nick. *Patterns, Principles, and Practices of Domain-Driven Design*. Wrox, 2015.

## Document Index

1. [Bounded Contexts](./bounded-contexts.md) - Detailed context definitions
2. [Aggregates](./aggregates.md) - Aggregate root specifications
3. [Domain Events](./domain-events.md) - Event catalog and schemas
4. [Ubiquitous Language](./ubiquitous-language.md) - Domain terminology glossary
