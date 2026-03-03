# Industrial & Specialized Modules -- WiFi-DensePose Edge Intelligence

> Worker safety and compliance monitoring using WiFi CSI signals. Works through
> dust, smoke, shelving, and walls where cameras fail. Designed for warehouses,
> factories, clean rooms, farms, and construction sites.

**ADR-041 Category 5 | Event IDs 500--599 | Crate `wifi-densepose-wasm-edge`**

## Safety Warning

These modules are **supplementary monitoring tools**. They do NOT replace:

- Certified safety systems (SIL-rated controllers, safety PLCs)
- Gas detectors, O2 monitors, or LEL sensors
- OSHA-required personal protective equipment
- Physical barriers, guardrails, or interlocks
- Trained safety attendants or rescue teams

Always deploy alongside certified primary safety systems. WiFi CSI sensing is
susceptible to environmental changes (new metal objects, humidity, temperature)
that can cause false negatives. Calibrate regularly and validate against ground
truth.

---

## Overview

| Module | File | What It Does | Event IDs | Budget |
|---|---|---|---|---|
| Forklift Proximity | `ind_forklift_proximity.rs` | Warns when pedestrians are near moving forklifts/AGVs | 500--502 | S (<5 ms) |
| Confined Space | `ind_confined_space.rs` | Monitors worker vitals in tanks, manholes, vessels | 510--514 | L (<2 ms) |
| Clean Room | `ind_clean_room.rs` | Personnel count and turbulent motion for ISO 14644 | 520--523 | L (<2 ms) |
| Livestock Monitor | `ind_livestock_monitor.rs` | Animal health monitoring in pens, barns, enclosures | 530--533 | L (<2 ms) |
| Structural Vibration | `ind_structural_vibration.rs` | Seismic, resonance, and structural drift detection | 540--543 | H (<10 ms) |

---

## Modules

### Forklift Proximity Warning (`ind_forklift_proximity.rs`)

**What it does**: Warns when a person is too close to a moving forklift, AGV,
or mobile robot, even around blind corners and through shelving racks.

**How it works**: The module separates forklift signatures from human
signatures using three CSI features:

1. **Amplitude ratio**: Large metal bodies (forklifts) produce 2--5x amplitude
   increases across all subcarriers relative to an empty-warehouse baseline.
2. **Low-frequency phase dominance**: Forklifts move slowly (<0.3 Hz phase
   modulation) compared to walking humans (0.5--2 Hz). The module computes
   the ratio of low-frequency energy to total phase energy.
3. **Motor vibration**: Electric forklift motors produce elevated, uniform
   variance across subcarriers (>0.08 threshold).

When all three conditions are met for 4 consecutive frames (debounced), the
module declares a vehicle present. If a human signature (host-reported
presence + motion energy >0.15) co-occurs, a proximity warning is emitted
with a distance category derived from amplitude ratio.

#### API

```rust
pub struct ForkliftProximityDetector { /* ... */ }

impl ForkliftProximityDetector {
    /// Create a new detector. Requires 100-frame calibration (~5 s at 20 Hz).
    pub const fn new() -> Self;

    /// Process one CSI frame. Returns events as (event_id, value) pairs.
    pub fn process_frame(
        &mut self,
        phases: &[f32],       // per-subcarrier phase values
        amplitudes: &[f32],   // per-subcarrier amplitude values
        variance: &[f32],     // per-subcarrier variance values
        motion_energy: f32,   // host-reported motion energy
        presence: i32,        // host-reported presence flag (0/1)
        n_persons: i32,       // host-reported person count
    ) -> &[(i32, f32)];

    /// Whether a vehicle is currently detected.
    pub fn is_vehicle_present(&self) -> bool;

    /// Current amplitude ratio (proxy for vehicle proximity).
    pub fn amplitude_ratio(&self) -> f32;
}
```

#### Events Emitted

| Event ID | Constant | Value | Meaning |
|---|---|---|---|
| 500 | `EVENT_PROXIMITY_WARNING` | Distance category: 0.0 = critical, 1.0 = warning, 2.0 = caution | Person dangerously close to vehicle |
| 501 | `EVENT_VEHICLE_DETECTED` | Amplitude ratio (float) | Forklift/AGV entered sensor zone |
| 502 | `EVENT_HUMAN_NEAR_VEHICLE` | Motion energy (float) | Human detected in vehicle zone (fires once on transition) |

#### State Machine

```
                  +-----------+
                  |           |
        +-------->| No Vehicle|<---------+
        |         |           |          |
        |         +-----+-----+          |
        |               |               |
        |   amp_ratio > 2.5 AND         |
        |   low_freq_dominant AND        | debounce drops
        |   vibration > 0.08            | below threshold
        |   (4 frames debounce)          |
        |               |               |
        |         +-----v-----+          |
        |         |           |----------+
        +---------|  Vehicle  |
                  |  Present  |
                  +-----+-----+
                        |
          human present |  (presence + motion > 0.15)
          + debounce    |
                  +-----v-----+
                  | Proximity |----> EVENT 500 (cooldown 40 frames)
                  |  Warning  |----> EVENT 502 (once on transition)
                  +-----------+
```

#### Configuration

| Parameter | Default | Range | Safety Implication |
|---|---|---|---|
| `FORKLIFT_AMP_RATIO` | 2.5 | 1.5--5.0 | Lower = more sensitive, more false positives |
| `HUMAN_MOTION_THRESH` | 0.15 | 0.05--0.5 | Lower = catches slow-moving workers |
| `VEHICLE_DEBOUNCE` | 4 frames | 2--10 | Higher = fewer false alarms, slower response |
| `PROXIMITY_DEBOUNCE` | 2 frames | 1--5 | Higher = fewer false alarms, slower response |
| `ALERT_COOLDOWN` | 40 frames (2 s) | 10--200 | Lower = more frequent warnings |
| `DIST_CRITICAL` | amp ratio > 4.0 | -- | Very close proximity |
| `DIST_WARNING` | amp ratio > 3.0 | -- | Close proximity |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::ind_forklift_proximity::ForkliftProximityDetector;

let mut detector = ForkliftProximityDetector::new();

// Calibration phase: feed 100 frames of empty warehouse
for _ in 0..100 {
    detector.process_frame(&phases, &amps, &variance, 0.0, 0, 0);
}

// Normal operation
let events = detector.process_frame(&phases, &amps, &variance, 0.5, 1, 1);
for &(event_id, value) in events {
    match event_id {
        500 => {
            let category = match value as i32 {
                0 => "CRITICAL -- stop forklift immediately",
                1 => "WARNING -- reduce speed",
                _ => "CAUTION -- be alert",
            };
            trigger_alarm(category);
        }
        501 => log("Vehicle detected, amplitude ratio: {}", value),
        502 => log("Human entered vehicle zone"),
        _ => {}
    }
}
```

#### Tutorial: Setting Up Warehouse Proximity Alerts

1. **Sensor placement**: Mount one ESP32 WiFi sensor per aisle, at shelf
   height (1.5--2 m). Each sensor covers approximately one aisle width
   (3--4 m) and 10--15 m of aisle length.

2. **Calibration**: Power on during a quiet period (no forklifts, no
   workers). The module auto-calibrates over the first 100 frames (5 s
   at 20 Hz). The baseline amplitude represents the empty aisle.

3. **Threshold tuning**: If false alarms occur due to hand trucks or
   pallet jacks, increase `FORKLIFT_AMP_RATIO` from 2.5 to 3.0. If
   forklifts are missed, decrease to 2.0.

4. **Integration**: Connect `EVENT_PROXIMITY_WARNING` (500) to a warning
   light (amber for caution/warning, red for critical) and audible alarm.
   Connect to the facility SCADA system for logging.

5. **Validation**: Walk through the aisle while a forklift operates.
   Verify all three distance categories trigger at appropriate ranges.

---

### Confined Space Monitor (`ind_confined_space.rs`)

**What it does**: Monitors workers inside tanks, manholes, vessels, or any
enclosed space. Confirms they are breathing and alerts if they stop moving
or breathing.

**Compliance**: Designed to support OSHA 29 CFR 1910.146 confined space
entry requirements. The module provides continuous proof-of-life monitoring
to supplement (not replace) the required safety attendant.

**How it works**: Uses debounced presence detection to track entry/exit
transitions. While a worker is inside, the module continuously monitors
two vital indicators:

1. **Breathing**: Host-reported breathing BPM must stay above 4.0 BPM.
   If breathing is not detected for 300 frames (15 seconds at 20 Hz),
   an extraction alert is emitted.
2. **Motion**: Host-reported motion energy must stay above 0.02. If no
   motion is detected for 1200 frames (60 seconds), an immobility alert
   is emitted.

The module transitions between `Empty`, `Present`, `BreathingCeased`, and
`Immobile` states. When breathing or motion resumes, the state recovers
back to `Present`.

#### API

```rust
pub enum WorkerState {
    Empty,           // No worker in the space
    Present,         // Worker present, vitals normal
    BreathingCeased, // No breathing detected (danger)
    Immobile,        // No motion detected (danger)
}

pub struct ConfinedSpaceMonitor { /* ... */ }

impl ConfinedSpaceMonitor {
    pub const fn new() -> Self;

    /// Process one frame.
    pub fn process_frame(
        &mut self,
        presence: i32,       // host-reported presence (0/1)
        breathing_bpm: f32,  // host-reported breathing rate
        motion_energy: f32,  // host-reported motion energy
        variance: f32,       // mean CSI variance
    ) -> &[(i32, f32)];

    /// Current worker state.
    pub fn state(&self) -> WorkerState;

    /// Whether a worker is inside the space.
    pub fn is_worker_inside(&self) -> bool;

    /// Seconds since last confirmed breathing.
    pub fn seconds_since_breathing(&self) -> f32;

    /// Seconds since last detected motion.
    pub fn seconds_since_motion(&self) -> f32;
}
```

#### Events Emitted

| Event ID | Constant | Value | Meaning |
|---|---|---|---|
| 510 | `EVENT_WORKER_ENTRY` | 1.0 | Worker entered the confined space |
| 511 | `EVENT_WORKER_EXIT` | 1.0 | Worker exited the confined space |
| 512 | `EVENT_BREATHING_OK` | BPM (float) | Periodic breathing confirmation (~every 5 s) |
| 513 | `EVENT_EXTRACTION_ALERT` | Seconds since last breath | No breathing for >15 s -- initiate rescue |
| 514 | `EVENT_IMMOBILE_ALERT` | Seconds without motion | No motion for >60 s -- check on worker |

#### State Machine

```
            +---------+
            |  Empty  |<----------+
            +----+----+           |
                 |                |
     presence    |                | absence (10 frames)
     (10 frames) |                |
                 v                |
            +---------+           |
    +------>| Present |-----------+
    |       +----+----+
    |            |          |
    |  breathing | no       | no motion
    |  resumes   | breathing| (1200 frames)
    |            | (300     |
    |            |  frames) |
    |       +----v------+   |
    +-------|Breathing  |   |
    |       | Ceased    |   |
    |       +-----------+   |
    |                       |
    |       +-----------+   |
    +-------| Immobile  |<--+
            +-----------+
              motion resumes -> Present
```

#### Configuration

| Parameter | Default | Range | Safety Implication |
|---|---|---|---|
| `BREATHING_CEASE_FRAMES` | 300 (15 s) | 100--600 | Lower = faster alert, more false positives |
| `IMMOBILE_FRAMES` | 1200 (60 s) | 400--3600 | Lower = catches slower collapses |
| `MIN_BREATHING_BPM` | 4.0 | 2.0--8.0 | Lower = more tolerant of slow breathing |
| `MIN_MOTION_ENERGY` | 0.02 | 0.005--0.1 | Lower = catches subtle movements |
| `ENTRY_EXIT_DEBOUNCE` | 10 frames | 5--30 | Higher = fewer false entry/exits |
| `MIN_PRESENCE_VAR` | 0.005 | 0.001--0.05 | Noise rejection for empty space |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::ind_confined_space::{
    ConfinedSpaceMonitor, WorkerState,
    EVENT_EXTRACTION_ALERT, EVENT_IMMOBILE_ALERT,
};

let mut monitor = ConfinedSpaceMonitor::new();

// Process each CSI frame
let events = monitor.process_frame(presence, breathing_bpm, motion_energy, variance);

for &(event_id, value) in events {
    match event_id {
        513 => {  // EXTRACTION_ALERT
            activate_rescue_alarm();
            notify_safety_attendant(value);  // seconds since last breath
        }
        514 => {  // IMMOBILE_ALERT
            notify_safety_attendant(value);  // seconds without motion
        }
        _ => {}
    }
}

// Query state for dashboard display
match monitor.state() {
    WorkerState::Empty => display_green("Space empty"),
    WorkerState::Present => display_green("Worker OK"),
    WorkerState::BreathingCeased => display_red("NO BREATHING"),
    WorkerState::Immobile => display_amber("Worker immobile"),
}
```

---

### Clean Room Monitor (`ind_clean_room.rs`)

**What it does**: Tracks personnel count and movement patterns in cleanrooms
to enforce ISO 14644 occupancy limits and detect turbulent motion that could
disturb laminar airflow.

**How it works**: Uses the host-reported person count with debounced
violation detection. Turbulent motion (rapid movement with energy >0.6) is
flagged because it disrupts the laminar airflow that keeps particulate counts
low. The module maintains a running compliance percentage for audit reporting.

#### API

```rust
pub struct CleanRoomMonitor { /* ... */ }

impl CleanRoomMonitor {
    /// Create with default max occupancy of 4.
    pub const fn new() -> Self;

    /// Create with custom maximum occupancy.
    pub const fn with_max_occupancy(max: u8) -> Self;

    /// Process one frame.
    pub fn process_frame(
        &mut self,
        n_persons: i32,      // host-reported person count
        presence: i32,       // host-reported presence (0/1)
        motion_energy: f32,  // host-reported motion energy
    ) -> &[(i32, f32)];

    /// Current occupancy count.
    pub fn current_count(&self) -> u8;

    /// Maximum allowed occupancy.
    pub fn max_occupancy(&self) -> u8;

    /// Whether currently in violation.
    pub fn is_in_violation(&self) -> bool;

    /// Compliance percentage (0--100).
    pub fn compliance_percent(&self) -> f32;

    /// Total number of violation events.
    pub fn total_violations(&self) -> u32;
}
```

#### Events Emitted

| Event ID | Constant | Value | Meaning |
|---|---|---|---|
| 520 | `EVENT_OCCUPANCY_COUNT` | Person count (float) | Occupancy changed |
| 521 | `EVENT_OCCUPANCY_VIOLATION` | Current count (float) | Count exceeds max allowed |
| 522 | `EVENT_TURBULENT_MOTION` | Motion energy (float) | Rapid movement detected (airflow risk) |
| 523 | `EVENT_COMPLIANCE_REPORT` | Compliance % (0--100) | Periodic compliance summary (~30 s) |

#### State Machine

```
    +------------------+
    |  Monitoring      |
    |  (count <= max)  |
    +--------+---------+
             |  count > max
             |  (10 frames debounce)
    +--------v---------+
    |  Violation       |----> EVENT 521 (cooldown 200 frames)
    |  (count > max)   |
    +--------+---------+
             |  count <= max
             |
    +--------v---------+
    |  Monitoring      |
    +------------------+

    Parallel:
    motion_energy > 0.6 (3 frames) ----> EVENT 522 (cooldown 100 frames)
    Every 600 frames (~30 s) ----------> EVENT 523 (compliance %)
```

#### Configuration

| Parameter | Default | Range | Safety Implication |
|---|---|---|---|
| `DEFAULT_MAX_OCCUPANCY` | 4 | 1--255 | Per ISO 14644 room class |
| `TURBULENT_MOTION_THRESH` | 0.6 | 0.3--0.9 | Lower = stricter movement control |
| `VIOLATION_DEBOUNCE` | 10 frames | 3--20 | Higher = tolerates brief over-counts |
| `VIOLATION_COOLDOWN` | 200 frames (10 s) | 40--600 | Alert repeat interval |
| `COMPLIANCE_REPORT_INTERVAL` | 600 frames (30 s) | 200--6000 | Audit report frequency |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::ind_clean_room::{
    CleanRoomMonitor, EVENT_OCCUPANCY_VIOLATION, EVENT_COMPLIANCE_REPORT,
};

// ISO Class 5 cleanroom: max 3 personnel
let mut monitor = CleanRoomMonitor::with_max_occupancy(3);

let events = monitor.process_frame(n_persons, presence, motion_energy);
for &(event_id, value) in events {
    match event_id {
        521 => alert_cleanroom_supervisor(value as u8),
        522 => alert_turbulent_motion(),
        523 => log_compliance_audit(value),
        _ => {}
    }
}

// Dashboard
println!("Occupancy: {}/{}", monitor.current_count(), monitor.max_occupancy());
println!("Compliance: {:.1}%", monitor.compliance_percent());
```

---

### Livestock Monitor (`ind_livestock_monitor.rs`)

**What it does**: Monitors animal presence and health in pens, barns, and
enclosures. Detects abnormal stillness (possible illness), labored breathing,
and escape events.

**How it works**: Tracks presence with debounced entry/exit detection.
Monitors breathing rate against species-specific normal ranges. Detects
prolonged stillness (>5 minutes) as a sign of illness, and sudden absence
after confirmed presence as an escape event.

Species-specific breathing ranges:

| Species | Normal BPM | Labored: below | Labored: above |
|---|---|---|---|
| Cattle | 12--30 | 8.4 (0.7x min) | 39.0 (1.3x max) |
| Sheep | 12--20 | 8.4 (0.7x min) | 26.0 (1.3x max) |
| Poultry | 15--30 | 10.5 (0.7x min) | 39.0 (1.3x max) |
| Custom | configurable | 0.7x min | 1.3x max |

#### API

```rust
pub enum Species {
    Cattle,
    Sheep,
    Poultry,
    Custom { min_bpm: f32, max_bpm: f32 },
}

pub struct LivestockMonitor { /* ... */ }

impl LivestockMonitor {
    /// Create with default species (Cattle).
    pub const fn new() -> Self;

    /// Create with a specific species.
    pub const fn with_species(species: Species) -> Self;

    /// Process one frame.
    pub fn process_frame(
        &mut self,
        presence: i32,       // host-reported presence (0/1)
        breathing_bpm: f32,  // host-reported breathing rate
        motion_energy: f32,  // host-reported motion energy
        variance: f32,       // mean CSI variance (unused, reserved)
    ) -> &[(i32, f32)];

    /// Whether an animal is currently detected.
    pub fn is_animal_present(&self) -> bool;

    /// Configured species.
    pub fn species(&self) -> Species;

    /// Minutes of stillness.
    pub fn stillness_minutes(&self) -> f32;

    /// Last observed breathing BPM.
    pub fn last_breathing_bpm(&self) -> f32;
}
```

#### Events Emitted

| Event ID | Constant | Value | Meaning |
|---|---|---|---|
| 530 | `EVENT_ANIMAL_PRESENT` | BPM (float) | Periodic presence report (~10 s) |
| 531 | `EVENT_ABNORMAL_STILLNESS` | Minutes still (float) | No motion for >5 minutes |
| 532 | `EVENT_LABORED_BREATHING` | BPM (float) | Breathing outside normal range |
| 533 | `EVENT_ESCAPE_ALERT` | Minutes present before escape (float) | Animal suddenly absent after confirmed presence |

#### State Machine

```
    +---------+
    |  Empty  |<---------+
    +----+----+          |
         |               |
   presence              | absence >= 20 frames
   (10 frames)           | (after >= 200 frames presence
         v               |  -> EVENT 533 escape alert)
    +---------+          |
    | Present |----------+
    +----+----+
         |
   no motion (6000 frames = 5 min) -> EVENT 531 (once)
   breathing outside range (20 frames) -> EVENT 532 (repeating)
```

#### Configuration

| Parameter | Default | Range | Safety Implication |
|---|---|---|---|
| `STILLNESS_FRAMES` | 6000 (5 min) | 1200--12000 | Lower = earlier illness detection |
| `MIN_PRESENCE_FOR_ESCAPE` | 200 (10 s) | 60--600 | Minimum presence before escape counts |
| `ESCAPE_ABSENCE_FRAMES` | 20 (1 s) | 10--100 | Brief absences tolerated |
| `LABORED_DEBOUNCE` | 20 frames (1 s) | 5--60 | Lower = faster breathing alerts |
| `MIN_MOTION_ACTIVE` | 0.03 | 0.01--0.1 | Sensitivity to subtle movement |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::ind_livestock_monitor::{
    LivestockMonitor, Species, EVENT_ESCAPE_ALERT, EVENT_LABORED_BREATHING,
};

// Dairy barn: monitor cows
let mut monitor = LivestockMonitor::with_species(Species::Cattle);

let events = monitor.process_frame(presence, breathing_bpm, motion_energy, variance);
for &(event_id, value) in events {
    match event_id {
        532 => alert_veterinarian(value),  // labored breathing BPM
        533 => alert_farm_security(value), // escape: minutes present before loss
        531 => log_health_concern(value),  // minutes of stillness
        _ => {}
    }
}
```

---

### Structural Vibration Monitor (`ind_structural_vibration.rs`)

**What it does**: Detects building vibration, seismic activity, and structural
stress using CSI phase stability. Only operates when the monitored space is
unoccupied (human movement masks structural signals).

**How it works**: When no humans are present, WiFi CSI phase is highly stable
(noise floor ~0.02 rad). The module detects three types of structural events:

1. **Seismic**: Broadband energy increase (>60% of subcarriers affected,
   RMS >0.15 rad). Indicates earthquake, heavy vehicle pass-by, or
   construction activity.
2. **Mechanical resonance**: Narrowband peaks detected via autocorrelation
   of the mean-phase time series. A peak-to-mean ratio >3.0 with RMS above
   2x noise floor indicates periodic mechanical vibration (HVAC, pumps,
   rotating equipment).
3. **Structural drift**: Slow monotonic phase change across >50% of
   subcarriers for >30 seconds. Indicates material stress, foundation
   settlement, or thermal expansion.

#### API

```rust
pub struct StructuralVibrationMonitor { /* ... */ }

impl StructuralVibrationMonitor {
    /// Create a new monitor. Requires 100-frame calibration when empty.
    pub const fn new() -> Self;

    /// Process one CSI frame.
    pub fn process_frame(
        &mut self,
        phases: &[f32],       // per-subcarrier phase values
        amplitudes: &[f32],   // per-subcarrier amplitude values
        variance: &[f32],     // per-subcarrier variance values
        presence: i32,        // 0 = empty (analyze), 1 = occupied (skip)
    ) -> &[(i32, f32)];

    /// Current RMS vibration level.
    pub fn rms_vibration(&self) -> f32;

    /// Whether baseline has been established.
    pub fn is_calibrated(&self) -> bool;
}
```

#### Events Emitted

| Event ID | Constant | Value | Meaning |
|---|---|---|---|
| 540 | `EVENT_SEISMIC_DETECTED` | RMS vibration level (rad) | Broadband seismic activity |
| 541 | `EVENT_MECHANICAL_RESONANCE` | Dominant frequency (Hz) | Narrowband mechanical vibration |
| 542 | `EVENT_STRUCTURAL_DRIFT` | Drift rate (rad/s) | Slow structural deformation |
| 543 | `EVENT_VIBRATION_SPECTRUM` | RMS level (rad) | Periodic spectrum report (~5 s) |

#### State Machine

```
    +--------------+
    | Calibrating  |  (100 frames, presence=0 required)
    +------+-------+
           |
    +------v-------+
    |   Idle       |  (presence=1: skip analysis, reset drift)
    | (Occupied)   |
    +------+-------+
           |  presence=0
    +------v-------+
    |  Analyzing   |
    +------+-------+
           |
           +-----> RMS > 0.15 + broadband -------> EVENT 540 (seismic)
           +-----> autocorr peak ratio > 3.0 ----> EVENT 541 (resonance)
           +-----> monotonic drift > 30 s -------> EVENT 542 (drift)
           +-----> every 100 frames -------------> EVENT 543 (spectrum)
```

#### Configuration

| Parameter | Default | Range | Safety Implication |
|---|---|---|---|
| `SEISMIC_THRESH` | 0.15 rad RMS | 0.05--0.5 | Lower = more sensitive to tremors |
| `RESONANCE_PEAK_RATIO` | 3.0 | 2.0--5.0 | Lower = detects weaker resonances |
| `DRIFT_RATE_THRESH` | 0.0005 rad/frame | 0.0001--0.005 | Lower = detects slower drift |
| `DRIFT_MIN_FRAMES` | 600 (30 s) | 200--2400 | Minimum drift duration before alert |
| `SEISMIC_DEBOUNCE` | 4 frames | 2--10 | Higher = fewer false seismic alerts |
| `SEISMIC_COOLDOWN` | 200 frames (10 s) | 40--600 | Alert repeat interval |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::ind_structural_vibration::{
    StructuralVibrationMonitor, EVENT_SEISMIC_DETECTED, EVENT_STRUCTURAL_DRIFT,
};

let mut monitor = StructuralVibrationMonitor::new();

// Calibrate during unoccupied period
for _ in 0..100 {
    monitor.process_frame(&phases, &amps, &variance, 0);
}
assert!(monitor.is_calibrated());

// Normal operation
let events = monitor.process_frame(&phases, &amps, &variance, presence);
for &(event_id, value) in events {
    match event_id {
        540 => {
            trigger_building_alarm();
            log_seismic_event(value);  // RMS vibration level
        }
        542 => {
            notify_structural_engineer(value);  // drift rate rad/s
        }
        _ => {}
    }
}
```

---

## OSHA Compliance Notes

### Forklift Proximity (OSHA 29 CFR 1910.178)

- **Standard**: Powered Industrial Trucks -- operator must warn others.
- **Module supports**: Automated proximity detection supplements horn/light
  warnings. Does NOT replace operator training, seat belts, or speed limits.
- **Additional equipment required**: Physical barriers, floor markings,
  traffic mirrors, operator training program.

### Confined Space (OSHA 29 CFR 1910.146)

- **Standard**: Permit-Required Confined Spaces.
- **Module supports**: Continuous proof-of-life monitoring (breathing and
  motion confirmation). Assists the required safety attendant.
- **Additional equipment required**:
  - Atmospheric monitoring (O2, H2S, CO, LEL) -- the WiFi module cannot
    detect gas hazards.
  - Communication system between entrant and attendant.
  - Rescue equipment (retrieval system, harness, tripod).
  - Entry permit documenting hazards and controls.
- **Audit trail**: `EVENT_BREATHING_OK` (512) provides timestamped
  proof-of-life records for compliance documentation.

### Clean Room (ISO 14644)

- **Standard**: Cleanrooms and associated controlled environments.
- **Module supports**: Real-time occupancy enforcement and turbulent motion
  detection for particulate control.
- **Additional equipment required**: Particle counters, differential pressure
  monitors, HEPA/ULPA filtration systems.
- **Documentation**: `EVENT_COMPLIANCE_REPORT` (523) provides periodic
  compliance percentages for audit records.

### Livestock (no direct OSHA standard; see USDA Animal Welfare Act)

- **Module supports**: Automated health monitoring reduces manual inspection
  burden. Escape detection supports perimeter security.
- **Additional equipment required**: Veterinary monitoring systems, proper
  fencing, temperature/humidity sensors.

### Structural Vibration (OSHA 29 CFR 1926 Subpart P, Excavations)

- **Standard**: Structural stability requirements for construction.
- **Module supports**: Continuous vibration monitoring during unoccupied
  periods. Seismic detection provides early warning.
- **Additional equipment required**: Certified structural inspection,
  accelerometers for critical structures, tilt sensors.

---

## Deployment Guide

### Sensor Placement for Warehouse Coverage

```
    +---+---+---+---+---+
    | S |   |   |   | S |   S = WiFi sensor (ESP32)
    +---+ Aisle 1   +---+   Mounted at shelf height (1.5-2 m)
    |   |           |   |   One sensor per aisle intersection
    +---+ Aisle 2   +---+
    | S |           | S |   Coverage: ~15 m range per sensor
    +---+---+---+---+---+   For proximity: sensor every 10 m along aisle
```

- Mount sensors at shelf height (1.5--2 m) for best human/forklift separation.
- Place at aisle intersections for blind-corner coverage.
- Each sensor covers approximately 10--15 m of aisle length.
- For critical zones (loading docks, charging areas), use overlapping sensors.

### Multi-Sensor Setup for Confined Spaces

```
    Ground Level
    +-----------+
    |  Sensor A | <-- Entry point monitoring
    +-----+-----+
          |
          | Manhole / Hatch
          |
    +-----v-----+
    |  Sensor B | <-- Inside space (if possible)
    +-----------+
```

- Sensor A at the entry point detects worker entry/exit.
- Sensor B inside the confined space (if safely mountable) provides
  breathing and motion monitoring.
- If only one sensor is available, mount at the entry facing into the space.
- WiFi signals penetrate metal walls poorly -- use multiple sensors for
  large vessels.

### Integration with Safety PLCs

Connect ESP32 event output to safety PLCs via:

1. **UDP**: The sensing server receives ESP32 CSI data and emits events
   via REST API. Poll `/api/v1/events` for real-time alerts.
2. **Modbus TCP**: Use a gateway to convert UDP events to Modbus registers
   for direct PLC integration.
3. **GPIO**: For hard-wired safety circuits, connect ESP32 GPIO outputs
   to PLC safety inputs. Configure the ESP32 firmware to assert GPIO on
   specific event IDs.

### Calibration Checklist

1. Ensure the monitored space is in its normal empty state.
2. Power on the sensor and wait for calibration to complete:
   - Forklift Proximity: 100 frames (5 seconds)
   - Structural Vibration: 100 frames (5 seconds)
   - Confined Space: No calibration needed (uses host presence)
   - Clean Room: No calibration needed (uses host person count)
   - Livestock: No calibration needed (uses host presence)
3. Validate by walking through the space and confirming presence detection.
4. For forklift proximity, drive a forklift through and verify vehicle
   detection and proximity warnings at appropriate distances.
5. Document calibration date, sensor position, and firmware version.

---

## Event ID Registry (Category 5)

| Range | Module | Events |
|---|---|---|
| 500--502 | Forklift Proximity | `PROXIMITY_WARNING`, `VEHICLE_DETECTED`, `HUMAN_NEAR_VEHICLE` |
| 510--514 | Confined Space | `WORKER_ENTRY`, `WORKER_EXIT`, `BREATHING_OK`, `EXTRACTION_ALERT`, `IMMOBILE_ALERT` |
| 520--523 | Clean Room | `OCCUPANCY_COUNT`, `OCCUPANCY_VIOLATION`, `TURBULENT_MOTION`, `COMPLIANCE_REPORT` |
| 530--533 | Livestock Monitor | `ANIMAL_PRESENT`, `ABNORMAL_STILLNESS`, `LABORED_BREATHING`, `ESCAPE_ALERT` |
| 540--543 | Structural Vibration | `SEISMIC_DETECTED`, `MECHANICAL_RESONANCE`, `STRUCTURAL_DRIFT`, `VIBRATION_SPECTRUM` |

Total: 20 event types across 5 modules.
