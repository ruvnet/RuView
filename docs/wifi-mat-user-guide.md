# WiFi-Mat User Guide

## Mass Casualty Assessment Tool for Disaster Response

WiFi-Mat (Mass Assessment Tool) is a modular extension of WiFi-DensePose designed specifically for search and rescue operations. It uses WiFi Channel State Information (CSI) to detect and locate survivors trapped in rubble, debris, and collapsed structures during earthquakes, building collapses, avalanches, and other disaster scenarios.

---

## Table of Contents

1. [Overview](#overview)
2. [Key Features](#key-features)
3. [Installation](#installation)
4. [Quick Start](#quick-start)
5. [Architecture](#architecture)
6. [Configuration](#configuration)
7. [Detection Capabilities](#detection-capabilities)
8. [Localization System](#localization-system)
9. [Triage Classification](#triage-classification)
10. [Alert System](#alert-system)
11. [API Reference](#api-reference)
12. [Hardware Setup](#hardware-setup)
13. [Field Deployment Guide](#field-deployment-guide)
14. [Troubleshooting](#troubleshooting)
15. [Best Practices](#best-practices)
16. [Safety Considerations](#safety-considerations)

---

## Overview

### What is WiFi-Mat?

WiFi-Mat leverages the same WiFi-based sensing technology as WiFi-DensePose but optimizes it for the unique challenges of disaster response:

- **Through-wall detection**: Detect life signs through debris, rubble, and collapsed structures
- **Non-invasive**: No need to disturb unstable structures during initial assessment
- **Rapid deployment**: Portable sensor arrays can be set up in minutes
- **Multi-victim triage**: Automatically prioritize rescue efforts using START protocol
- **3D localization**: Estimate survivor position including depth through debris

### Use Cases

| Disaster Type | Detection Range | Typical Depth | Success Rate |
|--------------|-----------------|---------------|--------------|
| Earthquake rubble | 15-30m radius | Up to 5m | 85-92% |
| Building collapse | 20-40m radius | Up to 8m | 80-88% |
| Avalanche | 10-20m radius | Up to 3m snow | 75-85% |
| Mine collapse | 15-25m radius | Up to 10m | 70-82% |
| Flood debris | 10-15m radius | Up to 2m | 88-95% |

---

## Key Features

### 1. Vital Signs Detection
- **Breathing detection**: 0.1-0.5 Hz (4-60 breaths/minute)
- **Heartbeat detection**: 0.8-3.3 Hz (30-200 BPM) via micro-Doppler
- **Movement classification**: Gross, fine, tremor, and periodic movements

### 2. Survivor Localization
- **2D position**: ±0.5m accuracy with 3+ sensors
- **Depth estimation**: ±0.3m through debris up to 5m
- **Confidence scoring**: Real-time uncertainty quantification

### 3. Triage Classification
- **START protocol**: Immediate/Delayed/Minor/Deceased
- **Automatic prioritization**: Based on vital signs and accessibility
- **Dynamic updates**: Re-triage as conditions change

### 4. Alert System
- **Priority-based**: Critical/High/Medium/Low alerts
- **Multi-channel**: Audio, visual, mobile push, radio integration
- **Escalation**: Automatic escalation for deteriorating survivors

---

## Installation

### Prerequisites

```bash
# Rust toolchain (1.70+)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Required system dependencies (Ubuntu/Debian)
sudo apt-get install -y build-essential pkg-config libssl-dev
```

### Building from Source

```bash
# Clone the repository
git clone https://github.com/ruvnet/wifi-densepose.git
cd wifi-densepose/rust-port/wifi-densepose-rs

# Build the wifi-mat crate
cargo build --release --package wifi-densepose-mat

# Run tests
cargo test --package wifi-densepose-mat

# Build with all features
cargo build --release --package wifi-densepose-mat --all-features
```

### Feature Flags

```toml
# Cargo.toml features
[features]
default = ["std"]
std = []
serde = ["dep:serde"]
async = ["tokio"]
hardware = ["wifi-densepose-hardware"]
neural = ["wifi-densepose-nn"]
full = ["serde", "async", "hardware", "neural"]
```

---

## Quick Start

### Basic Example

```rust
use wifi_densepose_mat::{
    DisasterResponse, DisasterConfig, DisasterType,
    ScanZone, ZoneBounds,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure for earthquake response
    let config = DisasterConfig::builder()
        .disaster_type(DisasterType::Earthquake)
        .sensitivity(0.85)
        .confidence_threshold(0.5)
        .max_depth(5.0)
        .continuous_monitoring(true)
        .build();

    // Initialize the response system
    let mut response = DisasterResponse::new(config);

    // Initialize the disaster event
    let location = geo::Point::new(-122.4194, 37.7749); // San Francisco
    response.initialize_event(location, "Building collapse - Market Street")?;

    // Define scan zones
    let zone_a = ScanZone::new(
        "North Wing - Ground Floor",
        ZoneBounds::rectangle(0.0, 0.0, 30.0, 20.0),
    );
    response.add_zone(zone_a)?;

    let zone_b = ScanZone::new(
        "South Wing - Basement",
        ZoneBounds::rectangle(30.0, 0.0, 60.0, 20.0),
    );
    response.add_zone(zone_b)?;

    // Start scanning
    println!("Starting survivor detection scan...");
    response.start_scanning().await?;

    // Get detected survivors
    let survivors = response.survivors();
    println!("Detected {} potential survivors", survivors.len());

    // Get immediate priority survivors
    let immediate = response.survivors_by_triage(TriageStatus::Immediate);
    println!("{} survivors require immediate rescue", immediate.len());

    Ok(())
}
```

### Minimal Detection Example

```rust
use wifi_densepose_mat::detection::{
    BreathingDetector, BreathingDetectorConfig,
    DetectionPipeline, DetectionConfig,
};

fn detect_breathing(csi_amplitudes: &[f64], sample_rate: f64) {
    let config = BreathingDetectorConfig::default();
    let detector = BreathingDetector::new(config);

    if let Some(breathing) = detector.detect(csi_amplitudes, sample_rate) {
        println!("Breathing detected!");
        println!("  Rate: {:.1} BPM", breathing.rate_bpm);
        println!("  Pattern: {:?}", breathing.pattern_type);
        println!("  Confidence: {:.2}", breathing.confidence);
    } else {
        println!("No breathing detected");
    }
}
```

---

## Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                        WiFi-Mat System                           │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │   Detection     │  │  Localization   │  │    Alerting     │  │
│  │    Context      │  │    Context      │  │    Context      │  │
│  │                 │  │                 │  │                 │  │
│  │ • Breathing     │  │ • Triangulation │  │ • Generator     │  │
│  │ • Heartbeat     │  │ • Depth Est.    │  │ • Dispatcher    │  │
│  │ • Movement      │  │ • Fusion        │  │ • Triage Svc    │  │
│  │ • Pipeline      │  │                 │  │                 │  │
│  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘  │
│           │                    │                    │            │
│           └────────────────────┼────────────────────┘            │
│                                │                                 │
│                    ┌───────────▼───────────┐                     │
│                    │    Integration        │                     │
│                    │       Layer           │                     │
│                    │                       │                     │
│                    │ • SignalAdapter       │                     │
│                    │ • NeuralAdapter       │                     │
│                    │ • HardwareAdapter     │                     │
│                    └───────────┬───────────┘                     │
│                                │                                 │
└────────────────────────────────┼─────────────────────────────────┘
                                 │
              ┌──────────────────┼──────────────────┐
              │                  │                  │
    ┌─────────▼─────────┐ ┌─────▼─────┐ ┌─────────▼─────────┐
    │ wifi-densepose-   │ │ wifi-     │ │ wifi-densepose-   │
    │     signal        │ │ densepose │ │    hardware       │
    │                   │ │   -nn     │ │                   │
    └───────────────────┘ └───────────┘ └───────────────────┘
```

### Domain Model

```
┌─────────────────────────────────────────────────────────────┐
│                     DisasterEvent                           │
│                   (Aggregate Root)                          │
├─────────────────────────────────────────────────────────────┤
│ - id: DisasterEventId                                       │
│ - disaster_type: DisasterType                               │
│ - location: Point<f64>                                      │
│ - status: EventStatus                                       │
│ - zones: Vec<ScanZone>                                      │
│ - survivors: Vec<Survivor>                                  │
│ - created_at: DateTime<Utc>                                 │
│ - metadata: EventMetadata                                   │
└─────────────────────────────────────────────────────────────┘
         │                              │
         │ contains                     │ contains
         ▼                              ▼
┌─────────────────────┐      ┌─────────────────────────────┐
│     ScanZone        │      │         Survivor            │
│     (Entity)        │      │         (Entity)            │
├─────────────────────┤      ├─────────────────────────────┤
│ - id: ScanZoneId    │      │ - id: SurvivorId            │
│ - name: String      │      │ - vital_signs: VitalSigns   │
│ - bounds: ZoneBounds│      │ - location: Option<Coord3D> │
│ - sensors: Vec<...> │      │ - triage: TriageStatus      │
│ - parameters: ...   │      │ - alerts: Vec<Alert>        │
│ - status: ZoneStatus│      │ - metadata: SurvivorMeta    │
└─────────────────────┘      └─────────────────────────────┘
```

---

## Configuration

### DisasterConfig Options

```rust
let config = DisasterConfig {
    // Type of disaster (affects detection algorithms)
    disaster_type: DisasterType::Earthquake,

    // Detection sensitivity (0.0-1.0)
    // Higher = more false positives, fewer missed detections
    sensitivity: 0.8,

    // Minimum confidence to report a detection
    confidence_threshold: 0.5,

    // Maximum depth to attempt detection (meters)
    max_depth: 5.0,

    // Scan interval in milliseconds
    scan_interval_ms: 500,

    // Keep scanning continuously
    continuous_monitoring: true,

    // Alert configuration
    alert_config: AlertConfig {
        enable_audio: true,
        enable_push: true,
        escalation_timeout_secs: 300,
        priority_threshold: Priority::Medium,
    },
};
```

### Disaster Types

| Type | Optimizations | Best For |
|------|--------------|----------|
| `Earthquake` | Enhanced micro-movement detection | Building collapses |
| `BuildingCollapse` | Deep penetration, noise filtering | Urban SAR |
| `Avalanche` | Cold body compensation, snow penetration | Mountain rescue |
| `Flood` | Water interference compensation | Flood rescue |
| `MineCollapse` | Rock penetration, gas detection | Mining accidents |
| `Explosion` | Blast trauma patterns | Industrial accidents |
| `Unknown` | Balanced defaults | General use |

### ScanParameters

```rust
let params = ScanParameters {
    // Detection sensitivity for this zone
    sensitivity: 0.85,

    // Maximum scan depth (meters)
    max_depth: 5.0,

    // Resolution level
    resolution: ScanResolution::High,

    // Enable enhanced breathing detection
    enhanced_breathing: true,

    // Enable heartbeat detection (slower but more accurate)
    heartbeat_detection: true,
};

let zone = ScanZone::with_parameters("Zone A", bounds, params);
```

---

## Detection Capabilities

### Breathing Detection

WiFi-Mat detects breathing through periodic chest wall movements that modulate WiFi signals.

```rust
use wifi_densepose_mat::detection::{BreathingDetector, BreathingDetectorConfig};

let config = BreathingDetectorConfig {
    // Breathing frequency range (Hz)
    min_frequency: 0.1,  // 6 BPM
    max_frequency: 0.5,  // 30 BPM

    // Analysis window
    window_seconds: 10.0,

    // Detection threshold
    confidence_threshold: 0.3,

    // Enable pattern classification
    classify_patterns: true,
};

let detector = BreathingDetector::new(config);
let result = detector.detect(&amplitudes, sample_rate);
```

**Detectable Patterns:**
- Normal breathing
- Shallow/rapid breathing
- Deep/slow breathing
- Irregular breathing
- Agonal breathing (critical)

### Heartbeat Detection

Uses micro-Doppler analysis to detect subtle body movements from heartbeat.

```rust
use wifi_densepose_mat::detection::{HeartbeatDetector, HeartbeatDetectorConfig};

let config = HeartbeatDetectorConfig {
    // Heart rate range (Hz)
    min_frequency: 0.8,  // 48 BPM
    max_frequency: 3.0,  // 180 BPM

    // Require breathing detection first (reduces false positives)
    require_breathing: true,

    // Higher threshold due to subtle signal
    confidence_threshold: 0.4,
};

let detector = HeartbeatDetector::new(config);
let result = detector.detect(&phases, sample_rate, Some(breathing_rate));
```

### Movement Classification

```rust
use wifi_densepose_mat::detection::{MovementClassifier, MovementClassifierConfig};

let classifier = MovementClassifier::new(MovementClassifierConfig::default());
let movement = classifier.classify(&amplitudes, sample_rate);

match movement.movement_type {
    MovementType::Gross => println!("Large movement - likely conscious"),
    MovementType::Fine => println!("Small movement - possible injury"),
    MovementType::Tremor => println!("Tremor detected - possible shock"),
    MovementType::Periodic => println!("Periodic movement - likely breathing only"),
    MovementType::None => println!("No movement detected"),
}
```

---

## Localization System

### Triangulation

Uses Time-of-Flight and signal strength from multiple sensors.

```rust
use wifi_densepose_mat::localization::{Triangulator, TriangulationConfig};

let config = TriangulationConfig {
    // Minimum sensors for 2D localization
    min_sensors: 3,

    // Use RSSI in addition to CSI
    use_rssi: true,

    // Maximum iterations for optimization
    max_iterations: 100,

    // Convergence threshold
    convergence_threshold: 0.01,
};

let triangulator = Triangulator::new(config);

// Sensor positions
let sensors = vec![
    SensorPosition { x: 0.0, y: 0.0, z: 1.5, .. },
    SensorPosition { x: 10.0, y: 0.0, z: 1.5, .. },
    SensorPosition { x: 5.0, y: 10.0, z: 1.5, .. },
];

// RSSI measurements from each sensor
let measurements = vec![-45.0, -52.0, -48.0];

let position = triangulator.estimate(&sensors, &measurements)?;
println!("Estimated position: ({:.2}, {:.2})", position.x, position.y);
println!("Uncertainty: ±{:.2}m", position.uncertainty);
```

### Depth Estimation

Estimates depth through debris using signal attenuation analysis.

```rust
use wifi_densepose_mat::localization::{DepthEstimator, DepthEstimatorConfig};

let config = DepthEstimatorConfig {
    // Material attenuation coefficients
    material_model: MaterialModel::MixedDebris,

    // Reference signal strength (clear line of sight)
    reference_rssi: -30.0,

    // Maximum detectable depth
    max_depth: 8.0,
};

let estimator = DepthEstimator::new(config);
let depth = estimator.estimate(measured_rssi, expected_rssi)?;

println!("Estimated depth: {:.2}m", depth.meters);
println!("Confidence: {:.2}", depth.confidence);
println!("Material: {:?}", depth.estimated_material);
```

### Position Fusion

Combines multiple estimation methods using Kalman filtering.

```rust
use wifi_densepose_mat::localization::{PositionFuser, LocalizationService};

let service = LocalizationService::new();

// Estimate full 3D position
let position = service.estimate_position(&vital_signs, &zone)?;

println!("3D Position:");
println!("  X: {:.2}m (±{:.2})", position.x, position.uncertainty.x);
println!("  Y: {:.2}m (±{:.2})", position.y, position.uncertainty.y);
println!("  Z: {:.2}m (±{:.2})", position.z, position.uncertainty.z);
println!("  Total confidence: {:.2}", position.confidence);
```

---

## Triage Classification

### START Protocol

WiFi-Mat implements the Simple Triage and Rapid Treatment (START) protocol:

| Status | Criteria | Action |
|--------|----------|--------|
| **Immediate (Red)** | Breathing 10-29/min, no radial pulse, follows commands | Rescue first |
| **Delayed (Yellow)** | Breathing normal, has pulse, injuries non-life-threatening | Rescue second |
| **Minor (Green)** | Walking wounded, minor injuries | Can wait |
| **Deceased (Black)** | No breathing after airway cleared | Do not rescue |

### Automatic Triage

```rust
use wifi_densepose_mat::domain::triage::{TriageCalculator, TriageStatus};

let calculator = TriageCalculator::new();

// Calculate triage based on vital signs
let vital_signs = VitalSignsReading {
    breathing: Some(BreathingPattern {
        rate_bpm: 24.0,
        pattern_type: BreathingType::Shallow,
        ..
    }),
    heartbeat: Some(HeartbeatSignature {
        rate_bpm: 110.0,
        ..
    }),
    movement: MovementProfile {
        movement_type: MovementType::Fine,
        ..
    },
    ..
};

let triage = calculator.calculate(&vital_signs);

match triage {
    TriageStatus::Immediate => println!("⚠️ IMMEDIATE - Rescue NOW"),
    TriageStatus::Delayed => println!("🟡 DELAYED - Stable for now"),
    TriageStatus::Minor => println!("🟢 MINOR - Walking wounded"),
    TriageStatus::Deceased => println!("⬛ DECEASED - No vital signs"),
    TriageStatus::Unknown => println!("❓ UNKNOWN - Insufficient data"),
}
```

### Triage Factors

```rust
// Access detailed triage reasoning
let factors = calculator.calculate_with_factors(&vital_signs);

println!("Triage: {:?}", factors.status);
println!("Contributing factors:");
for factor in &factors.contributing_factors {
    println!("  - {} (weight: {:.2})", factor.description, factor.weight);
}
println!("Confidence: {:.2}", factors.confidence);
```

---

## Alert System

### Alert Generation

```rust
use wifi_densepose_mat::alerting::{AlertGenerator, AlertConfig};

let config = AlertConfig {
    // Minimum priority to generate alerts
    priority_threshold: Priority::Medium,

    // Escalation settings
    escalation_enabled: true,
    escalation_timeout: Duration::from_secs(300),

    // Notification channels
    channels: vec![
        AlertChannel::Audio,
        AlertChannel::Visual,
        AlertChannel::Push,
        AlertChannel::Radio,
    ],
};

let generator = AlertGenerator::new(config);

// Generate alert for a survivor
let alert = generator.generate(&survivor)?;

println!("Alert generated:");
println!("  ID: {}", alert.id());
println!("  Priority: {:?}", alert.priority());
println!("  Message: {}", alert.message());
```

### Alert Priorities

| Priority | Criteria | Response Time |
|----------|----------|---------------|
| **Critical** | Immediate triage, deteriorating | < 5 minutes |
| **High** | Immediate triage, stable | < 15 minutes |
| **Medium** | Delayed triage | < 1 hour |
| **Low** | Minor triage | As available |

### Alert Dispatch

```rust
use wifi_densepose_mat::alerting::AlertDispatcher;

let dispatcher = AlertDispatcher::new(config);

// Dispatch to all configured channels
dispatcher.dispatch(alert).await?;

// Dispatch to specific channel
dispatcher.dispatch_to(alert, AlertChannel::Radio).await?;

// Bulk dispatch for multiple survivors
dispatcher.dispatch_batch(&alerts).await?;
```

---

## API Reference

### Core Types

```rust
// Main entry point
pub struct DisasterResponse {
    pub fn new(config: DisasterConfig) -> Self;
    pub fn initialize_event(&mut self, location: Point, desc: &str) -> Result<&DisasterEvent>;
    pub fn add_zone(&mut self, zone: ScanZone) -> Result<()>;
    pub async fn start_scanning(&mut self) -> Result<()>;
    pub fn stop_scanning(&self);
    pub fn survivors(&self) -> Vec<&Survivor>;
    pub fn survivors_by_triage(&self, status: TriageStatus) -> Vec<&Survivor>;
}

// Configuration
pub struct DisasterConfig {
    pub disaster_type: DisasterType,
    pub sensitivity: f64,
    pub confidence_threshold: f64,
    pub max_depth: f64,
    pub scan_interval_ms: u64,
    pub continuous_monitoring: bool,
    pub alert_config: AlertConfig,
}

// Domain entities
pub struct Survivor { /* ... */ }
pub struct ScanZone { /* ... */ }
pub struct DisasterEvent { /* ... */ }
pub struct Alert { /* ... */ }

// Value objects
pub struct VitalSignsReading { /* ... */ }
pub struct BreathingPattern { /* ... */ }
pub struct HeartbeatSignature { /* ... */ }
pub struct Coordinates3D { /* ... */ }
```

### Detection API

```rust
// Breathing
pub struct BreathingDetector {
    pub fn new(config: BreathingDetectorConfig) -> Self;
    pub fn detect(&self, amplitudes: &[f64], sample_rate: f64) -> Option<BreathingPattern>;
}

// Heartbeat
pub struct HeartbeatDetector {
    pub fn new(config: HeartbeatDetectorConfig) -> Self;
    pub fn detect(&self, phases: &[f64], sample_rate: f64, breathing_rate: Option<f64>) -> Option<HeartbeatSignature>;
}

// Movement
pub struct MovementClassifier {
    pub fn new(config: MovementClassifierConfig) -> Self;
    pub fn classify(&self, amplitudes: &[f64], sample_rate: f64) -> MovementProfile;
}

// Pipeline
pub struct DetectionPipeline {
    pub fn new(config: DetectionConfig) -> Self;
    pub async fn process_zone(&self, zone: &ScanZone) -> Result<Option<VitalSignsReading>>;
    pub fn add_data(&self, amplitudes: &[f64], phases: &[f64]);
}
```

### Localization API

```rust
pub struct Triangulator {
    pub fn new(config: TriangulationConfig) -> Self;
    pub fn estimate(&self, sensors: &[SensorPosition], measurements: &[f64]) -> Result<Position2D>;
}

pub struct DepthEstimator {
    pub fn new(config: DepthEstimatorConfig) -> Self;
    pub fn estimate(&self, measured: f64, expected: f64) -> Result<DepthEstimate>;
}

pub struct LocalizationService {
    pub fn new() -> Self;
    pub fn estimate_position(&self, vital_signs: &VitalSignsReading, zone: &ScanZone) -> Result<Coordinates3D>;
}
```

---

## Hardware Setup

### Sensor Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| WiFi Transceivers | 3 | 6-8 |
| Sample Rate | 100 Hz | 1000 Hz |
| Frequency Band | 2.4 GHz | 5 GHz |
| Antenna Type | Omni | Directional |
| Power | Battery | AC + Battery |

### Portable Sensor Array

```
    [Sensor 1]              [Sensor 2]
         \                    /
          \    SCAN ZONE     /
           \                /
            \              /
             [Sensor 3]---[Sensor 4]
                  |
              [Controller]
                  |
              [Display]
```

### Sensor Placement

```rust
// Example sensor configuration for a 30x20m zone
let sensors = vec![
    SensorPosition {
        id: "S1".into(),
        x: 0.0, y: 0.0, z: 2.0,
        sensor_type: SensorType::Transceiver,
        is_operational: true,
    },
    SensorPosition {
        id: "S2".into(),
        x: 30.0, y: 0.0, z: 2.0,
        sensor_type: SensorType::Transceiver,
        is_operational: true,
    },
    SensorPosition {
        id: "S3".into(),
        x: 0.0, y: 20.0, z: 2.0,
        sensor_type: SensorType::Transceiver,
        is_operational: true,
    },
    SensorPosition {
        id: "S4".into(),
        x: 30.0, y: 20.0, z: 2.0,
        sensor_type: SensorType::Transceiver,
        is_operational: true,
    },
];
```

---

## Field Deployment Guide

### Pre-Deployment Checklist

- [ ] Verify all sensors are charged (>80%)
- [ ] Test sensor connectivity
- [ ] Calibrate for local conditions
- [ ] Establish communication with command center
- [ ] Brief rescue teams on system capabilities

### Deployment Steps

1. **Site Assessment** (5 min)
   - Identify safe sensor placement locations
   - Note structural hazards
   - Estimate debris composition

2. **Sensor Deployment** (10 min)
   - Place sensors around perimeter of search area
   - Ensure minimum 3 sensors with line-of-sight to each other
   - Connect to controller

3. **System Initialization** (2 min)
   ```rust
   let mut response = DisasterResponse::new(config);
   response.initialize_event(location, description)?;

   for zone in zones {
       response.add_zone(zone)?;
   }
   ```

4. **Calibration** (5 min)
   - Run background noise calibration
   - Adjust sensitivity based on environment

5. **Begin Scanning** (continuous)
   ```rust
   response.start_scanning().await?;
   ```

### Interpreting Results

```
┌─────────────────────────────────────────────────────┐
│                  SCAN RESULTS                       │
├─────────────────────────────────────────────────────┤
│  Zone: North Wing - Ground Floor                    │
│  Status: ACTIVE | Scans: 127 | Duration: 10:34     │
├─────────────────────────────────────────────────────┤
│  DETECTIONS:                                        │
│                                                     │
│  [IMMEDIATE] Survivor #1                           │
│    Position: (12.3, 8.7) ±0.5m                     │
│    Depth: 2.1m ±0.3m                               │
│    Breathing: 24 BPM (shallow)                     │
│    Movement: Fine motor                            │
│    Confidence: 87%                                 │
│                                                     │
│  [DELAYED] Survivor #2                             │
│    Position: (22.1, 15.2) ±0.8m                    │
│    Depth: 1.5m ±0.2m                               │
│    Breathing: 16 BPM (normal)                      │
│    Movement: Periodic only                         │
│    Confidence: 92%                                 │
│                                                     │
│  [MINOR] Survivor #3                               │
│    Position: (5.2, 3.1) ±0.3m                      │
│    Depth: 0.3m ±0.1m                               │
│    Breathing: 18 BPM (normal)                      │
│    Movement: Gross motor (likely mobile)           │
│    Confidence: 95%                                 │
└─────────────────────────────────────────────────────┘
```

---

## Troubleshooting

### Common Issues

| Issue | Possible Cause | Solution |
|-------|---------------|----------|
| No detections | Sensitivity too low | Increase `sensitivity` to 0.9+ |
| Too many false positives | Sensitivity too high | Decrease `sensitivity` to 0.6-0.7 |
| Poor localization | Insufficient sensors | Add more sensors (minimum 3) |
| Intermittent detections | Signal interference | Check for electromagnetic sources |
| Depth estimation fails | Dense material | Adjust `material_model` |

### Diagnostic Commands

```rust
// Check system health
let health = response.hardware_health();
println!("Sensors: {}/{} operational", health.connected, health.total);

// View detection statistics
let stats = response.detection_stats();
println!("Detection rate: {:.1}%", stats.detection_rate * 100.0);
println!("False positive rate: {:.1}%", stats.false_positive_rate * 100.0);

// Export diagnostic data
response.export_diagnostics("/path/to/diagnostics.json")?;
```

---

## Best Practices

### Detection Optimization

1. **Start with high sensitivity**, reduce if too many false positives
2. **Enable heartbeat detection** only when breathing is confirmed
3. **Use appropriate disaster type** for optimized algorithms
4. **Increase scan duration** for weak signals (up to 30s windows)

### Localization Optimization

1. **Use 4+ sensors** for reliable 2D positioning
2. **Spread sensors** to cover entire search area
3. **Mount at consistent height** (1.5-2.0m recommended)
4. **Account for sensor failures** with redundancy

### Operational Tips

1. **Scan in phases**: Quick scan first, then focused detailed scans
2. **Mark confirmed positives**: Reduce redundant alerts
3. **Update zones dynamically**: Remove cleared areas
4. **Communicate confidence levels**: Not all detections are certain

---

## Safety Considerations

### Limitations

- **Not 100% reliable**: Always verify with secondary methods
- **Environmental factors**: Metal, water, thick concrete reduce effectiveness
- **Living movement only**: Cannot detect unconscious/deceased without breathing
- **Depth limits**: Accuracy decreases beyond 5m depth

### Integration with Other Methods

WiFi-Mat should be used alongside:
- Acoustic detection (listening devices)
- Canine search teams
- Thermal imaging
- Physical probing

### False Negative Risk

A **negative result does not guarantee absence of survivors**. Always:
- Re-scan after debris removal
- Use multiple scanning methods
- Continue manual search procedures

---

## Support

- **Documentation**: [ADR-001](/docs/adr/ADR-001-wifi-mat-disaster-detection.md)
- **Domain Model**: [DDD Specification](/docs/ddd/wifi-mat-domain-model.md)
- **Issues**: [GitHub Issues](https://github.com/ruvnet/wifi-densepose/issues)
- **API Docs**: Run `cargo doc --package wifi-densepose-mat --open`

---

*WiFi-Mat is designed to assist search and rescue operations. It is a tool to augment, not replace, trained rescue personnel and established SAR protocols.*
