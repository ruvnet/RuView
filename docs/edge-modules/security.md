# Security & Safety Modules -- WiFi-DensePose Edge Intelligence

> Perimeter monitoring and threat detection using WiFi Channel State Information (CSI).
> Works through walls, in complete darkness, without visible cameras.
> Each module runs on an $8 ESP32-S3 chip at 20 Hz frame rate.
> All modules are `no_std`-compatible and compile to WASM for hot-loading via ADR-040 Tier 3.

## Overview

| Module | File | What It Does | Event IDs | Budget |
|--------|------|--------------|-----------|--------|
| Intrusion Detection | `intrusion.rs` | Phase/amplitude anomaly intrusion alarm with arm/disarm | 200-203 | S (<5 ms) |
| Perimeter Breach | `sec_perimeter_breach.rs` | Multi-zone perimeter crossing with approach/departure | 210-213 | S (<5 ms) |
| Weapon Detection | `sec_weapon_detect.rs` | Concealed metallic object detection via RF reflectivity ratio | 220-222 | S (<5 ms) |
| Tailgating Detection | `sec_tailgating.rs` | Double-peak motion envelope for unauthorized following | 230-232 | L (<2 ms) |
| Loitering Detection | `sec_loitering.rs` | Prolonged stationary presence with 4-state machine | 240-242 | L (<2 ms) |
| Panic Motion | `sec_panic_motion.rs` | Erratic motion, struggle, and fleeing patterns | 250-252 | S (<5 ms) |

Budget key: **S** = Standard (<5 ms per frame), **L** = Light (<2 ms per frame).

## Shared Design Patterns

All security modules follow these conventions:

- **`const fn new()`**: Zero-allocation constructor, no heap, suitable for `static mut` on ESP32.
- **`process_frame(...) -> &[(i32, f32)]`**: Returns event tuples `(event_id, value)` via a static buffer (safe in single-threaded WASM).
- **Calibration phase**: First N frames (typically 100-200 at 20 Hz = 5-10 seconds) learn ambient baseline. No events during calibration.
- **Debounce**: Consecutive-frame counters prevent single-frame noise from triggering alerts.
- **Cooldown**: After emitting an event, a cooldown window suppresses duplicate emissions (40-100 frames = 2-5 seconds).
- **Hysteresis**: Debounce counters use `saturating_sub(1)` for gradual decay rather than hard reset, reducing flap on borderline signals.

---

## Modules

### Intrusion Detection (`intrusion.rs`)

**What it does**: Monitors a previously-empty space and triggers an alarm when someone enters. Works like a traditional motion alarm -- the environment must settle before the system arms itself.

**How it works**: During calibration (200 frames), the detector learns per-subcarrier amplitude mean and variance. After calibration, it waits for the environment to be quiet (100 consecutive frames with low disturbance) before arming. Once armed, it computes a composite disturbance score from phase velocity (sudden phase jumps between frames) and amplitude deviation (amplitude departing from baseline by more than 3 sigma). If the disturbance exceeds 0.8 for 3+ consecutive frames, an alert fires.

#### State Machine

```
Calibrating --> Monitoring --> Armed --> Alert
                   ^                      |
                   |        (quiet for     |
                   |         50 frames)    |
                   +---- Armed <----------+
```

- **Calibrating**: Accumulates baseline amplitude statistics for 200 frames.
- **Monitoring**: Waits for 100 consecutive quiet frames before arming.
- **Armed**: Active detection. Triggers alert on 3+ consecutive high-disturbance frames.
- **Alert**: Active alert. Returns to Armed after 50 consecutive quiet frames. 100-frame cooldown prevents re-triggering.

#### API

| Item | Type | Description |
|------|------|-------------|
| `IntrusionDetector::new()` | `const fn` | Create detector in Calibrating state |
| `process_frame(phases, amplitudes)` | `fn` | Process one CSI frame, returns events |
| `state()` | `fn -> DetectorState` | Current state (Calibrating/Monitoring/Armed/Alert) |
| `total_alerts()` | `fn -> u32` | Cumulative alert count |

#### Events Emitted

| Event ID | Constant | When Emitted |
|----------|----------|--------------|
| 200 | `EVENT_INTRUSION_ALERT` | Intrusion detected (disturbance score as value) |
| 201 | `EVENT_INTRUSION_ZONE` | Zone index of highest disturbance |
| 202 | `EVENT_INTRUSION_ARMED` | System transitioned to Armed state |
| 203 | `EVENT_INTRUSION_DISARMED` | System disarmed (currently unused -- reserved) |

#### Configuration

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `INTRUSION_VELOCITY_THRESH` | 1.5 | 0.5-3.0 | Phase velocity threshold (rad/frame) |
| `AMPLITUDE_CHANGE_THRESH` | 3.0 | 2.0-5.0 | Sigma multiplier for amplitude deviation |
| `ARM_FRAMES` | 100 | 40-200 | Quiet frames required before arming (5s at 20 Hz) |
| `DETECT_DEBOUNCE` | 3 | 2-10 | Consecutive disturbed frames before alert |
| `ALERT_COOLDOWN` | 100 | 20-200 | Frames between re-alerts (5s at 20 Hz) |
| `BASELINE_FRAMES` | 200 | 100-500 | Calibration frames (10s at 20 Hz) |

---

### Perimeter Breach Detection (`sec_perimeter_breach.rs`)

**What it does**: Divides the monitored area into 4 zones (mapped to subcarrier groups) and detects movement crossing zone boundaries. Classifies motion direction as approaching or departing using energy gradient trends.

**How it works**: Subcarriers are split into 4 equal groups, each representing a spatial zone. Per-zone metrics are computed every frame:
1. **Phase gradient**: Mean absolute phase difference between current and previous frame within the zone's subcarrier range.
2. **Variance ratio**: Current zone variance divided by calibrated baseline variance.

A breach is flagged when phase gradient exceeds 0.6 rad/subcarrier AND variance ratio exceeds 2.5x baseline. Direction is determined by linear regression slope over an 8-frame energy history buffer -- positive slope = approaching, negative = departing.

#### State Machine

There is no explicit state machine enum. Instead, per-zone counters track:
- `disturb_run`: Consecutive breach frames (resets to 0 when zone is quiet).
- `approach_run` / `departure_run`: Consecutive frames with positive/negative energy trend (debounced to 3 frames).
- Four independent cooldown timers for breach, approach, departure, and transition events.

No stuck states possible: all counters either reset on quiet input or are bounded by `saturating_add`.

#### API

| Item | Type | Description |
|------|------|-------------|
| `PerimeterBreachDetector::new()` | `const fn` | Create uncalibrated detector |
| `process_frame(phases, amplitudes, variance, motion_energy)` | `fn` | Process one frame, returns up to 4 events |
| `is_calibrated()` | `fn -> bool` | Whether baseline calibration is complete |
| `frame_count()` | `fn -> u32` | Total frames processed |

#### Events Emitted

| Event ID | Constant | When Emitted |
|----------|----------|--------------|
| 210 | `EVENT_PERIMETER_BREACH` | Significant disturbance in any zone (value = energy score) |
| 211 | `EVENT_APPROACH_DETECTED` | Energy trend rising in a breached zone (value = zone index) |
| 212 | `EVENT_DEPARTURE_DETECTED` | Energy trend falling in a zone (value = zone index) |
| 213 | `EVENT_ZONE_TRANSITION` | Movement shifted from one zone to another (value = `from*10 + to`) |

#### Configuration

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `BASELINE_FRAMES` | 100 | 60-200 | Calibration frames (5s at 20 Hz) |
| `BREACH_GRADIENT_THRESH` | 0.6 | 0.3-1.5 | Phase gradient for breach (rad/subcarrier) |
| `VARIANCE_RATIO_THRESH` | 2.5 | 1.5-5.0 | Variance ratio above baseline for disturbance |
| `DIRECTION_DEBOUNCE` | 3 | 2-8 | Consecutive trend frames for direction confirmation |
| `COOLDOWN` | 40 | 20-100 | Frames between events of same type (2s at 20 Hz) |
| `HISTORY_LEN` | 8 | 4-16 | Energy history buffer for trend estimation |
| `MAX_ZONES` | 4 | 2-4 | Number of perimeter zones |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::sec_perimeter_breach::*;

let mut detector = PerimeterBreachDetector::new();

// Feed CSI frames (phases, amplitudes, variance arrays, motion energy scalar)
let events = detector.process_frame(&phases, &amplitudes, &variance, motion_energy);

for &(event_id, value) in events {
    match event_id {
        EVENT_PERIMETER_BREACH => {
            // value = energy score (higher = more severe)
            log!("Breach detected, energy={:.2}", value);
        }
        EVENT_APPROACH_DETECTED => {
            // value = zone index (0-3)
            log!("Approach in zone {}", value as u32);
        }
        EVENT_ZONE_TRANSITION => {
            // value encodes from*10 + to
            let from = (value as u32) / 10;
            let to = (value as u32) % 10;
            log!("Movement from zone {} to zone {}", from, to);
        }
        _ => {}
    }
}
```

#### Tutorial: Setting Up a 4-Zone Perimeter System

1. **Sensor placement**: Mount the ESP32-S3 at the center of the monitored boundary (e.g., warehouse entrance, property line). The WiFi AP should be on the opposite side so the sensing link crosses all 4 zones.

2. **Zone mapping**: Subcarriers are divided equally among 4 zones. With 32 subcarriers:
   - Zone 0: subcarriers 0-7 (nearest to the ESP32)
   - Zone 1: subcarriers 8-15
   - Zone 2: subcarriers 16-23
   - Zone 3: subcarriers 24-31 (nearest to the AP)

3. **Calibration**: Power on the system with no one in the monitored area. Wait 5 seconds (100 frames) for calibration to complete. `is_calibrated()` returns `true`.

4. **Alert integration**: Forward events to your security system:
   - `EVENT_PERIMETER_BREACH` (210) -> Trigger alarm siren / camera recording
   - `EVENT_APPROACH_DETECTED` (211) -> Pre-alert: someone approaching
   - `EVENT_ZONE_TRANSITION` (213) -> Track movement direction through zones

5. **Tuning**: If false alarms occur in windy or high-traffic environments, increase `BREACH_GRADIENT_THRESH` and `VARIANCE_RATIO_THRESH`. If detections are missed, decrease them.

---

### Concealed Metallic Object Detection (`sec_weapon_detect.rs`)

**What it does**: Detects concealed metallic objects (knives, firearms, tools) carried by a person walking through the sensing area. Metal has significantly higher RF reflectivity than human tissue, producing a characteristic amplitude-variance-to-phase-variance ratio.

**How it works**: During calibration (100 frames in an empty room), the detector computes baseline amplitude and phase variance per subcarrier using online variance accumulation. After calibration, running Welford statistics track amplitude and phase variance in real-time. The ratio of running amplitude variance to running phase variance is computed across all subcarriers. Metal produces a high ratio (amplitude swings wildly from specular reflection while phase varies less than diffuse tissue).

Two thresholds are applied:
- **Metal anomaly** (ratio > 4.0, debounce 4 frames): General metallic object detection.
- **Weapon alert** (ratio > 8.0, debounce 6 frames): High-reflectivity alert for larger metal masses.

Detection requires `presence >= 1` and `motion_energy >= 0.5` to avoid false positives on environmental noise.

**Important**: This module is research-grade and experimental. It requires per-environment calibration and should not be used as a sole security measure.

#### API

| Item | Type | Description |
|------|------|-------------|
| `WeaponDetector::new()` | `const fn` | Create uncalibrated detector |
| `process_frame(phases, amplitudes, variance, motion_energy, presence)` | `fn` | Process one frame, returns up to 3 events |
| `is_calibrated()` | `fn -> bool` | Whether baseline calibration is complete |
| `frame_count()` | `fn -> u32` | Total frames processed |

#### Events Emitted

| Event ID | Constant | When Emitted |
|----------|----------|--------------|
| 220 | `EVENT_METAL_ANOMALY` | Metallic object signature detected (value = amp/phase ratio) |
| 221 | `EVENT_WEAPON_ALERT` | High-reflectivity metal signature (value = amp/phase ratio) |
| 222 | `EVENT_CALIBRATION_NEEDED` | Baseline drift exceeds threshold (value = max drift ratio) |

#### Configuration

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `BASELINE_FRAMES` | 100 | 60-200 | Calibration frames (empty room, 5s at 20 Hz) |
| `METAL_RATIO_THRESH` | 4.0 | 2.0-8.0 | Amp/phase variance ratio for metal detection |
| `WEAPON_RATIO_THRESH` | 8.0 | 5.0-15.0 | Ratio for weapon-grade alert |
| `MIN_MOTION_ENERGY` | 0.5 | 0.2-2.0 | Minimum motion to consider detection valid |
| `METAL_DEBOUNCE` | 4 | 2-10 | Consecutive frames for metal anomaly |
| `WEAPON_DEBOUNCE` | 6 | 3-12 | Consecutive frames for weapon alert |
| `COOLDOWN` | 60 | 20-120 | Frames between events (3s at 20 Hz) |
| `RECALIB_DRIFT_THRESH` | 3.0 | 2.0-5.0 | Drift ratio triggering recalibration alert |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::sec_weapon_detect::*;

let mut detector = WeaponDetector::new();

// Calibrate in empty room (100 frames)
for _ in 0..100 {
    detector.process_frame(&phases, &amplitudes, &variance, 0.0, 0);
}
assert!(detector.is_calibrated());

// Normal operation: person walks through
let events = detector.process_frame(&phases, &amplitudes, &variance, motion_energy, presence);

for &(event_id, value) in events {
    match event_id {
        EVENT_METAL_ANOMALY => {
            log!("Metal detected, ratio={:.1}", value);
        }
        EVENT_WEAPON_ALERT => {
            log!("WEAPON ALERT, ratio={:.1}", value);
            // Trigger security response
        }
        EVENT_CALIBRATION_NEEDED => {
            log!("Environment changed, recalibration recommended");
        }
        _ => {}
    }
}
```

---

### Tailgating Detection (`sec_tailgating.rs`)

**What it does**: Detects tailgating at doorways -- two or more people passing through in rapid succession. A single authorized passage produces one smooth energy peak; a tailgater following closely produces a second peak within a configurable window (default 3 seconds).

**How it works**: The detector uses temporal clustering of motion energy peaks through a 3-state machine:

1. **Idle**: Waiting for motion energy to exceed the adaptive threshold.
2. **InPeak**: Tracking an active peak. Records peak maximum energy and duration. Peak ends when energy drops below 30% of peak maximum. Noise spikes (peaks shorter than 3 frames) are discarded.
3. **Watching**: Peak ended, monitoring for another peak within the tailgate window (60 frames = 3s). If another peak arrives, it transitions back to InPeak. When the window expires, it evaluates: 1 peak = single passage, 2+ peaks = tailgating.

The threshold adapts to ambient noise via exponential moving average of variance.

#### State Machine

```
Idle ----[energy > threshold]----> InPeak
                                      |
                          [energy < 30% of peak max]
                                      |
             [peak too short]         v
Idle <------------------------- InPeak end
                                      |
                          [peak valid (>= 3 frames)]
                                      v
                                  Watching
                                   /    \
              [new peak starts]   /      \  [window expires]
                                 v        v
                              InPeak    Evaluate
                                        /     \
                               [1 peak]        [2+ peaks]
                                  |                |
                          SINGLE_PASSAGE    TAILGATE_DETECTED
                                  |           + MULTI_PASSAGE
                                  v                v
                                Idle             Idle
```

#### API

| Item | Type | Description |
|------|------|-------------|
| `TailgateDetector::new()` | `const fn` | Create detector |
| `process_frame(motion_energy, presence, n_persons, variance)` | `fn` | Process one frame, returns up to 3 events |
| `frame_count()` | `fn -> u32` | Total frames processed |
| `tailgate_count()` | `fn -> u32` | Total tailgating events detected |
| `single_passages()` | `fn -> u32` | Total single passages recorded |

#### Events Emitted

| Event ID | Constant | When Emitted |
|----------|----------|--------------|
| 230 | `EVENT_TAILGATE_DETECTED` | Two or more peaks within window (value = peak count) |
| 231 | `EVENT_SINGLE_PASSAGE` | Single peak followed by quiet window (value = peak energy) |
| 232 | `EVENT_MULTI_PASSAGE` | Three or more peaks within window (value = peak count) |

#### Configuration

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `ENERGY_PEAK_THRESH` | 2.0 | 1.0-5.0 | Motion energy threshold for peak start |
| `ENERGY_VALLEY_FRAC` | 0.3 | 0.1-0.5 | Fraction of peak max to end peak |
| `TAILGATE_WINDOW` | 60 | 20-120 | Max inter-peak gap for tailgating (3s at 20 Hz) |
| `MIN_PEAK_ENERGY` | 1.5 | 0.5-3.0 | Minimum peak energy for valid passage |
| `COOLDOWN` | 100 | 40-200 | Frames between events (5s at 20 Hz) |
| `MIN_PEAK_FRAMES` | 3 | 2-10 | Minimum peak duration to filter noise spikes |
| `MAX_PEAKS` | 8 | 4-16 | Maximum peaks tracked in one window |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::sec_tailgating::*;

let mut detector = TailgateDetector::new();

// Process frames from host
let events = detector.process_frame(motion_energy, presence, n_persons, variance_mean);

for &(event_id, value) in events {
    match event_id {
        EVENT_TAILGATE_DETECTED => {
            log!("TAILGATE: {} people in rapid succession", value as u32);
            // Lock door / alert security
        }
        EVENT_SINGLE_PASSAGE => {
            log!("Normal passage, energy={:.2}", value);
        }
        EVENT_MULTI_PASSAGE => {
            log!("Multi-passage: {} people", value as u32);
        }
        _ => {}
    }
}
```

---

### Loitering Detection (`sec_loitering.rs`)

**What it does**: Detects prolonged stationary presence in a monitored area. Distinguishes between a person passing through (normal) and someone standing still for an extended time (loitering). Default dwell threshold is 5 minutes.

**How it works**: Uses a 4-state machine that tracks presence duration and motion level. Only stationary frames (motion energy below 0.5) count toward the dwell threshold -- a person actively walking through does not accumulate loitering time. The exit cooldown (30 seconds) prevents false "loitering ended" events from brief signal dropouts or occlusions.

#### State Machine

```
Absent --[presence + no post_end cooldown]--> Entering
                                                  |
                                   [60 frames with presence]
                                                  |
            [absence before 60]                   v
Absent <------------------------------ Entering confirmed
                                                  |
                                                  v
                                              Present
                                             /       \
                          [6000 stationary   /         \ [absent > 300
                            frames]         /           \  frames]
                                           v             v
                                      Loitering       Absent
                                       /     \
                    [presence continues]       [absent >= 600 frames]
                              |                        |
                     LOITERING_ONGOING          LOITERING_END
                     (every 600 frames)                |
                              |                        v
                              v                     Absent
                          Loitering              (post_end_cd = 200)
```

#### API

| Item | Type | Description |
|------|------|-------------|
| `LoiteringDetector::new()` | `const fn` | Create detector in Absent state |
| `process_frame(presence, motion_energy)` | `fn` | Process one frame, returns up to 2 events |
| `state()` | `fn -> LoiterState` | Current state (Absent/Entering/Present/Loitering) |
| `frame_count()` | `fn -> u32` | Total frames processed |
| `loiter_count()` | `fn -> u32` | Total loitering events |
| `dwell_frames()` | `fn -> u32` | Current accumulated stationary dwell frames |

#### Events Emitted

| Event ID | Constant | When Emitted |
|----------|----------|--------------|
| 240 | `EVENT_LOITERING_START` | Dwell threshold exceeded (value = dwell time in seconds) |
| 241 | `EVENT_LOITERING_ONGOING` | Periodic report while loitering (value = total dwell seconds) |
| 242 | `EVENT_LOITERING_END` | Loiterer departed after exit cooldown (value = total dwell seconds) |

#### Configuration

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `ENTER_CONFIRM_FRAMES` | 60 | 20-120 | Presence confirmation (3s at 20 Hz) |
| `DWELL_THRESHOLD` | 6000 | 1200-12000 | Stationary frames for loitering (5 min at 20 Hz) |
| `EXIT_COOLDOWN` | 600 | 200-1200 | Absent frames before ending loitering (30s at 20 Hz) |
| `STATIONARY_MOTION_THRESH` | 0.5 | 0.2-1.5 | Motion energy below which person is stationary |
| `ONGOING_REPORT_INTERVAL` | 600 | 200-1200 | Frames between ongoing reports (30s at 20 Hz) |
| `POST_END_COOLDOWN` | 200 | 100-600 | Cooldown after end before re-detection (10s at 20 Hz) |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::sec_loitering::*;

let mut detector = LoiteringDetector::new();

let events = detector.process_frame(presence, motion_energy);

for &(event_id, value) in events {
    match event_id {
        EVENT_LOITERING_START => {
            log!("Loitering started after {:.0}s", value);
            // Alert security
        }
        EVENT_LOITERING_ONGOING => {
            log!("Still loitering, total {:.0}s", value);
        }
        EVENT_LOITERING_END => {
            log!("Loiterer departed after {:.0}s total", value);
        }
        _ => {}
    }
}

// Check state programmatically
if detector.state() == LoiterState::Loitering {
    // Continuous monitoring actions
}
```

---

### Panic/Erratic Motion Detection (`sec_panic_motion.rs`)

**What it does**: Detects three categories of distress-related motion:
1. **Panic**: Erratic, high-jerk motion with rapid random direction changes (e.g., someone flailing, being attacked).
2. **Struggle**: Elevated jerk with moderate energy and some direction changes (e.g., physical altercation, trying to break free).
3. **Fleeing**: Sustained high energy with low entropy -- running in one direction.

**How it works**: Maintains a 100-frame (5-second) circular buffer of motion energy and variance values. Computes window-level statistics each frame:

- **Mean jerk**: Average absolute rate-of-change of motion energy across the window. High jerk = erratic, unpredictable motion.
- **Entropy proxy**: Fraction of frames with direction reversals (energy transitions from increasing to decreasing or vice versa). High entropy = chaotic motion.
- **High jerk fraction**: Fraction of individual frame-to-frame jerks exceeding `JERK_THRESH`. Ensures the high mean is not from a single spike.

Detection logic:
- **Panic** = `mean_jerk > 2.0` AND `entropy > 0.35` AND `high_jerk_frac > 0.3`
- **Struggle** = `mean_jerk > 1.5` AND `energy in [1.0, 5.0)` AND `entropy > 0.175` AND not panic
- **Fleeing** = `mean_energy > 5.0` AND `mean_jerk > 0.05` AND `entropy < 0.25` AND not panic

#### API

| Item | Type | Description |
|------|------|-------------|
| `PanicMotionDetector::new()` | `const fn` | Create detector |
| `process_frame(motion_energy, variance_mean, phase_mean, presence)` | `fn` | Process one frame, returns up to 3 events |
| `frame_count()` | `fn -> u32` | Total frames processed |
| `panic_count()` | `fn -> u32` | Total panic events detected |

#### Events Emitted

| Event ID | Constant | When Emitted |
|----------|----------|--------------|
| 250 | `EVENT_PANIC_DETECTED` | Erratic high-jerk + high-entropy motion (value = severity 0-10) |
| 251 | `EVENT_STRUGGLE_PATTERN` | Elevated jerk at moderate energy (value = mean jerk) |
| 252 | `EVENT_FLEEING_DETECTED` | Sustained high-energy directional motion (value = mean energy) |

#### Configuration

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `WINDOW` | 100 | 40-200 | Analysis window size (5s at 20 Hz) |
| `JERK_THRESH` | 2.0 | 1.0-4.0 | Per-frame jerk threshold for panic |
| `ENTROPY_THRESH` | 0.35 | 0.2-0.6 | Direction reversal rate threshold |
| `MIN_MOTION` | 1.0 | 0.3-2.0 | Minimum motion energy (ignore idle) |
| `TRIGGER_FRAC` | 0.3 | 0.2-0.5 | Fraction of window frames exceeding thresholds |
| `COOLDOWN` | 100 | 40-200 | Frames between events (5s at 20 Hz) |
| `FLEE_ENERGY_THRESH` | 5.0 | 3.0-10.0 | Minimum energy for fleeing detection |
| `FLEE_JERK_THRESH` | 0.05 | 0.01-0.5 | Minimum jerk for fleeing (above noise floor) |
| `FLEE_MAX_ENTROPY` | 0.25 | 0.1-0.4 | Maximum entropy for fleeing (directional motion) |
| `STRUGGLE_JERK_THRESH` | 1.5 | 0.8-3.0 | Minimum mean jerk for struggle pattern |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::sec_panic_motion::*;

let mut detector = PanicMotionDetector::new();

let events = detector.process_frame(motion_energy, variance_mean, phase_mean, presence);

for &(event_id, value) in events {
    match event_id {
        EVENT_PANIC_DETECTED => {
            log!("PANIC: severity={:.1}", value);
            // Immediate security dispatch
        }
        EVENT_STRUGGLE_PATTERN => {
            log!("Struggle detected, jerk={:.2}", value);
            // Investigate
        }
        EVENT_FLEEING_DETECTED => {
            log!("Person fleeing, energy={:.1}", value);
            // Track direction via perimeter module
        }
        _ => {}
    }
}
```

---

## Event ID Registry (Security Range 200-299)

| Range | Module | Events |
|-------|--------|--------|
| 200-203 | `intrusion.rs` | INTRUSION_ALERT, INTRUSION_ZONE, INTRUSION_ARMED, INTRUSION_DISARMED |
| 210-213 | `sec_perimeter_breach.rs` | PERIMETER_BREACH, APPROACH_DETECTED, DEPARTURE_DETECTED, ZONE_TRANSITION |
| 220-222 | `sec_weapon_detect.rs` | METAL_ANOMALY, WEAPON_ALERT, CALIBRATION_NEEDED |
| 230-232 | `sec_tailgating.rs` | TAILGATE_DETECTED, SINGLE_PASSAGE, MULTI_PASSAGE |
| 240-242 | `sec_loitering.rs` | LOITERING_START, LOITERING_ONGOING, LOITERING_END |
| 250-252 | `sec_panic_motion.rs` | PANIC_DETECTED, STRUGGLE_PATTERN, FLEEING_DETECTED |
| 253-299 | | Reserved for future security modules |

---

## Testing

```bash
# Run all security module tests (requires std feature)
cd rust-port/wifi-densepose-rs/crates/wifi-densepose-wasm-edge
cargo test --features std -- sec_ intrusion
```

### Test Coverage Summary

| Module | Tests | Coverage Notes |
|--------|-------|----------------|
| `intrusion.rs` | 4 | Init, calibration, arming, intrusion detection |
| `sec_perimeter_breach.rs` | 6 | Init, calibration, breach, zone transition, approach, quiet signal |
| `sec_weapon_detect.rs` | 6 | Init, calibration, no presence, metal anomaly, normal person, drift recalib |
| `sec_tailgating.rs` | 7 | Init, single passage, tailgate, wide spacing, noise spike, multi-passage, low energy |
| `sec_loitering.rs` | 7 | Init, entering, cancel, loitering start/ongoing/end, brief absence, moving person |
| `sec_panic_motion.rs` | 7 | Init, window fill, calm motion, panic, no presence, fleeing, struggle, low motion |

---

## Deployment Considerations

### Coverage Area per Sensor

Each ESP32-S3 with a WiFi AP link covers a single sensing path. The coverage area depends on:
- **Distance**: 1-10 meters between ESP32 and AP (optimal: 3-5 meters for indoor).
- **Width**: First Fresnel zone width -- approximately 0.5-1.5 meters at 5 GHz.
- **Through-wall**: WiFi CSI penetrates drywall and wood but attenuates through concrete/metal. Signal quality degrades beyond one wall.

### Multi-Sensor Coordination

For larger areas, deploy multiple ESP32 sensors in a mesh:
- Each sensor runs its own WASM module instance independently.
- The aggregator server (`wifi-densepose-sensing-server`) collects events from all sensors.
- Cross-sensor correlation (e.g., tracking a person across zones) is done server-side, not on-device.
- Use `EVENT_ZONE_TRANSITION` (213) from perimeter breach to correlate movement across adjacent sensors.

### False Alarm Reduction

1. **Calibration**: Always calibrate in the intended operating conditions (time of day, HVAC state, door positions).
2. **Threshold tuning**: Start with defaults, increase thresholds if false alarms occur, decrease if detections are missed.
3. **Debounce tuning**: Increase debounce counters in high-noise environments (near HVAC vents, open windows).
4. **Multi-module correlation**: Require 2+ modules to agree before triggering high-severity responses. For example: perimeter breach + panic motion = confirmed threat; perimeter breach alone = investigation.
5. **Time-of-day filtering**: Server-side logic can suppress certain events during business hours (e.g., single passages are normal during the day).

### Integration with Existing Security Systems

- **Event forwarding**: Events are emitted via `csi_emit_event()` to the host firmware, which packs them into UDP packets sent to the aggregator.
- **REST API**: The sensing server exposes events at `/api/v1/sensing/events` for integration with SIEM, VMS, or access control systems.
- **Webhook support**: Configure the server to POST event payloads to external endpoints.
- **MQTT**: For IoT integration, events can be published to MQTT topics (one per event type or per sensor).

### Resource Usage on ESP32-S3

| Resource | Budget | Notes |
|----------|--------|-------|
| RAM | ~2-4 KB per module | Static buffers, no heap allocation |
| CPU | <5 ms per frame (S budget) | Well within 50 ms frame budget at 20 Hz |
| Flash | ~3-8 KB WASM per module | Compiled with `opt-level = "s"` and LTO |
| Total (6 modules) | ~15-25 KB RAM, ~30 KB Flash | Fits in 925 KB firmware with headroom |
