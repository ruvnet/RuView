# ADR-001: WiFi-Mat Disaster Detection Architecture

## Status
Accepted

## Date
2026-01-13

## Context

Natural disasters such as earthquakes, building collapses, avalanches, and floods trap victims under rubble or debris. Traditional search and rescue methods using visual inspection, thermal cameras, or acoustic devices have significant limitations:

- **Visual/Optical**: Cannot penetrate rubble, debris, or collapsed structures
- **Thermal**: Limited penetration depth, affected by ambient temperature
- **Acoustic**: Requires victim to make sounds, high false positive rate
- **K9 Units**: Limited availability, fatigue, environmental hazards

WiFi-based sensing offers a unique advantage: **RF signals can penetrate non-metallic debris** (concrete, wood, drywall) and detect subtle human movements including breathing patterns and heartbeats through Channel State Information (CSI) analysis.

### Problem Statement

We need a modular extension to the WiFi-DensePose Rust implementation that:

1. Detects human presence in disaster scenarios with high sensitivity
2. Localizes survivors within rubble/debris fields
3. Classifies victim status (conscious movement, breathing only, critical)
4. Provides real-time alerts to rescue teams
5. Operates in degraded/field conditions with portable hardware

## Decision

We will create a new crate `wifi-densepose-mat` (Mass Casualty Assessment Tool) as a modular addition to the existing Rust workspace with the following architecture:

### 1. Domain-Driven Design (DDD) Approach

The module follows DDD principles with clear bounded contexts:

```
wifi-densepose-mat/
├── src/
│   ├── domain/           # Core domain entities and value objects
│   │   ├── survivor.rs   # Survivor entity with status tracking
│   │   ├── disaster_event.rs  # Disaster event aggregate root
│   │   ├── scan_zone.rs  # Geographic zone being scanned
│   │   └── alert.rs      # Alert value objects
│   ├── detection/        # Life sign detection bounded context
│   │   ├── breathing.rs  # Breathing pattern detection
│   │   ├── heartbeat.rs  # Micro-doppler heartbeat detection
│   │   ├── movement.rs   # Gross/fine movement classification
│   │   └── classifier.rs # Multi-modal victim classifier
│   ├── localization/     # Position estimation bounded context
│   │   ├── triangulation.rs  # Multi-AP triangulation
│   │   ├── fingerprinting.rs # CSI fingerprint matching
│   │   └── depth.rs      # Depth/layer estimation in rubble
│   ├── alerting/         # Notification bounded context
│   │   ├── priority.rs   # Triage priority calculation
│   │   ├── dispatcher.rs # Alert routing and dispatch
│   │   └── protocols.rs  # Emergency protocol integration
│   └── integration/      # Anti-corruption layer
│       ├── signal_adapter.rs  # Adapts wifi-densepose-signal
│       └── nn_adapter.rs      # Adapts wifi-densepose-nn
```

### 2. Core Architectural Decisions

#### 2.1 Event-Driven Architecture
- All survivor detections emit domain events
- Events enable audit trails and replay for post-incident analysis
- Supports distributed deployments with multiple scan teams

#### 2.2 Configurable Detection Pipeline
```rust
pub struct DetectionPipeline {
    breathing_detector: BreathingDetector,
    heartbeat_detector: HeartbeatDetector,
    movement_classifier: MovementClassifier,
    ensemble_classifier: EnsembleClassifier,
}
```

#### 2.3 Triage Classification (START Protocol Compatible)
| Status | Detection Criteria | Priority |
|--------|-------------------|----------|
| Immediate (Red) | Breathing detected, no movement | P1 |
| Delayed (Yellow) | Movement + breathing, stable vitals | P2 |
| Minor (Green) | Strong movement, responsive patterns | P3 |
| Deceased (Black) | No vitals for >30 minutes continuous scan | P4 |

#### 2.4 Hardware Abstraction
Supports multiple deployment scenarios:
- **Portable**: Single TX/RX with handheld device
- **Distributed**: Multiple APs deployed around collapse site
- **Drone-mounted**: UAV-based scanning for large areas
- **Vehicle-mounted**: Mobile command post with array

### 3. Integration Strategy

The module integrates with existing crates through adapters:

```
┌─────────────────────────────────────────────────────────────┐
│                    wifi-densepose-mat                        │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  Detection  │  │ Localization│  │      Alerting       │  │
│  │  Context    │  │   Context   │  │      Context        │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
│         │                │                     │             │
│         └────────────────┼─────────────────────┘             │
│                          │                                   │
│              ┌───────────▼───────────┐                       │
│              │   Integration Layer   │                       │
│              │  (Anti-Corruption)    │                       │
│              └───────────┬───────────┘                       │
└──────────────────────────┼───────────────────────────────────┘
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
        ▼                  ▼                  ▼
┌───────────────┐  ┌───────────────┐  ┌───────────────┐
│wifi-densepose │  │wifi-densepose │  │wifi-densepose │
│    -signal    │  │     -nn       │  │   -hardware   │
└───────────────┘  └───────────────┘  └───────────────┘
```

### 4. Performance Requirements

| Metric | Target | Rationale |
|--------|--------|-----------|
| Detection Latency | <500ms | Real-time feedback for rescuers |
| False Positive Rate | <5% | Minimize wasted rescue efforts |
| False Negative Rate | <1% | Cannot miss survivors |
| Penetration Depth | 3-5m | Typical rubble pile depth |
| Battery Life (portable) | >8 hours | Full shift operation |
| Concurrent Zones | 16+ | Large disaster site coverage |

### 5. Safety and Reliability

- **Fail-safe defaults**: Always assume life present on ambiguous signals
- **Redundant detection**: Multiple algorithms vote on presence
- **Continuous monitoring**: Re-scan zones periodically
- **Offline operation**: Full functionality without network
- **Audit logging**: Complete trace of all detections

## Consequences

### Positive
- Modular design allows independent development and testing
- DDD ensures domain experts can validate logic
- Event-driven enables distributed deployments
- Adapters isolate from upstream changes
- Compatible with existing WiFi-DensePose infrastructure

### Negative
- Additional complexity from event system
- Learning curve for rescue teams
- Requires calibration for different debris types
- RF interference in disaster zones

### Risks and Mitigations
| Risk | Mitigation |
|------|------------|
| Metal debris blocking signals | Multi-angle scanning, adaptive frequency |
| Environmental RF interference | Spectral sensing, frequency hopping |
| False positives from animals | Size/pattern classification |
| Power constraints in field | Low-power modes, solar charging |

## References

- [WiFi-based Vital Signs Monitoring](https://dl.acm.org/doi/10.1145/3130944)
- [Through-Wall Human Sensing](https://ieeexplore.ieee.org/document/8645344)
- [START Triage Protocol](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC3088332/)
- [CSI-based Human Activity Recognition](https://arxiv.org/abs/2004.03661)
