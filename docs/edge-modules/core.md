# Core Modules -- WiFi-DensePose Edge Intelligence

> The foundation modules that every ESP32 node runs. These handle gesture detection, signal quality monitoring, anomaly detection, zone occupancy, vital sign tracking, intrusion classification, and model packaging.

All seven modules compile to `wasm32-unknown-unknown` and run inside the WASM3 interpreter on ESP32-S3 after Tier 2 DSP completes (ADR-040). They share a common `no_std`-compatible design: a struct with `const fn new()`, a `process_frame` (or `on_timer`) entry point, and zero heap allocation.

## Overview

| Module | File | What It Does | Compute Budget |
|--------|------|-------------|----------------|
| Gesture Classifier | `gesture.rs` | Recognizes hand gestures from CSI phase sequences using DTW template matching | ~2,400 f32 ops/frame (60x40 cost matrix) |
| Coherence Monitor | `coherence.rs` | Measures signal quality via phasor coherence across subcarriers | ~100 trig ops/frame (32 subcarriers) |
| Anomaly Detector | `adversarial.rs` | Flags physically impossible signals: phase jumps, flatlines, energy spikes | ~130 f32 ops/frame |
| Intrusion Detector | `intrusion.rs` | Detects unauthorized entry via phase velocity and amplitude disturbance | ~130 f32 ops/frame |
| Occupancy Detector | `occupancy.rs` | Divides sensing area into spatial zones and reports which are occupied | ~100 f32 ops/frame |
| Vital Trend Analyzer | `vital_trend.rs` | Monitors breathing/heart rate over 1-min and 5-min windows for clinical alerts | ~20 f32 ops/timer tick |
| RVF Container | `rvf.rs` | Binary container format that packages WASM modules with manifest and signature | Builder only (std), no per-frame cost |

## Modules

---

### Gesture Classifier (`gesture.rs`)

**What it does**: Recognizes predefined hand gestures from WiFi CSI phase sequences. It compares a sliding window of phase deltas against 4 built-in templates (wave, push, pull, swipe) using Dynamic Time Warping.

**How it works**: Each incoming frame provides subcarrier phases. The detector computes the phase delta from the previous frame and pushes it into a 60-sample ring buffer. When enough samples accumulate, it runs constrained DTW (with a Sakoe-Chiba band of width 5) between the tail of the observation window and each template. If the best normalized distance falls below the threshold (2.5), the corresponding gesture ID is emitted. A 40-frame cooldown prevents duplicate detections.

#### API

| Item | Type | Description |
|------|------|-------------|
| `GestureDetector` | struct | Main state holder. Contains ring buffer, templates, and cooldown timer. |
| `GestureDetector::new()` | `const fn` | Creates a detector with 4 built-in templates. |
| `GestureDetector::process_frame(&mut self, phases: &[f32]) -> Option<u8>` | method | Feed one frame of phase data. Returns `Some(gesture_id)` on match. |
| `MAX_TEMPLATE_LEN` | const (40) | Maximum number of samples in a gesture template. |
| `MAX_WINDOW_LEN` | const (60) | Maximum observation window length. |
| `NUM_TEMPLATES` | const (4) | Number of built-in templates. |
| `DTW_THRESHOLD` | const (2.5) | Normalized DTW distance threshold for a match. |
| `BAND_WIDTH` | const (5) | Sakoe-Chiba band width (limits warping). |

#### Configuration

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `DTW_THRESHOLD` | 2.5 | 0.5 -- 10.0 | Lower = stricter matching, fewer false positives but may miss soft gestures |
| `BAND_WIDTH` | 5 | 1 -- 20 | Width of the Sakoe-Chiba band. Wider = more flexible time warping but more computation |
| Cooldown frames | 40 | 10 -- 200 | Frames to wait before next detection. At 20 Hz, 40 frames = 2 seconds |

#### Events Emitted

| Event ID | Constant | When Emitted |
|----------|----------|-------------|
| 1 | `event_types::GESTURE_DETECTED` | A gesture template matched. Value = gesture ID (1=wave, 2=push, 3=pull, 4=swipe). |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::gesture::GestureDetector;

let mut detector = GestureDetector::new();

// Feed frames from CSI data (typically at 20 Hz).
let phases: Vec<f32> = get_csi_phases(); // your phase data
if let Some(gesture_id) = detector.process_frame(&phases) {
    println!("Detected gesture {}", gesture_id);
    // 1 = wave, 2 = push, 3 = pull, 4 = swipe
}
```

#### Tutorial: Adding a Custom Gesture Template

1. **Collect reference data**: Record the phase-delta sequence for your gesture by feeding CSI frames through the detector and logging the delta values in the ring buffer.

2. **Normalize the template**: Scale the phase-delta values so they span roughly -1.0 to 1.0. This ensures consistent DTW distances across different signal strengths.

3. **Edit the template array**: In `gesture.rs`, increase `NUM_TEMPLATES` by 1 and add a new entry in the `templates` array inside `GestureDetector::new()`:
   ```rust
   GestureTemplate {
       values: {
           let mut v = [0.0f32; MAX_TEMPLATE_LEN];
           v[0] = 0.2; v[1] = 0.6; // ... your values
           v
       },
       len: 8,  // number of valid samples
       id: 5,   // unique gesture ID
   },
   ```

4. **Tune the threshold**: Run test data through `dtw_distance()` directly to see the distance between your template and real observations. Adjust `DTW_THRESHOLD` if your gesture is consistently matched at a distance higher than 2.5.

5. **Test**: Add a unit test that feeds the template values as phase inputs and verifies that `process_frame` returns your new gesture ID.

---

### Coherence Monitor (`coherence.rs`)

**What it does**: Measures the phase coherence of the WiFi signal across subcarriers. High coherence means the signal is stable and sensing is accurate. Low coherence means multipath interference or environmental changes are degrading the signal.

**How it works**: For each frame, it computes the inter-frame phase delta per subcarrier, converts each delta to a unit phasor (cos + j*sin), and averages them. The magnitude of this mean phasor is the raw coherence (0 = random, 1 = perfectly aligned). This raw value is smoothed with an exponential moving average (alpha = 0.1). A hysteresis gate classifies the result into Accept (>0.7), Warn (0.4--0.7), or Reject (<0.4).

#### API

| Item | Type | Description |
|------|------|-------------|
| `CoherenceMonitor` | struct | Tracks phasor sums, EMA score, and gate state. |
| `CoherenceMonitor::new()` | `const fn` | Creates a monitor with initial coherence of 1.0 (Accept). |
| `process_frame(&mut self, phases: &[f32]) -> f32` | method | Feed one frame of phase data. Returns EMA-smoothed coherence [0, 1]. |
| `gate_state(&self) -> GateState` | method | Current gate classification (Accept, Warn, Reject). |
| `mean_phasor_angle(&self) -> f32` | method | Dominant phase drift direction in radians. |
| `coherence_score(&self) -> f32` | method | Current EMA-smoothed coherence score. |
| `GateState` | enum | `Accept`, `Warn`, `Reject` -- signal quality classification. |

#### Configuration

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `ALPHA` | 0.1 | 0.01 -- 0.5 | EMA smoothing factor. Lower = slower response, more stable. Higher = faster response, more noisy |
| `HIGH_THRESHOLD` | 0.7 | 0.5 -- 0.95 | Coherence above this = Accept |
| `LOW_THRESHOLD` | 0.4 | 0.1 -- 0.6 | Coherence below this = Reject |
| `MAX_SC` | 32 | 1 -- 64 | Maximum subcarriers tracked (compile-time) |

#### Events Emitted

| Event ID | Constant | When Emitted |
|----------|----------|-------------|
| 2 | `event_types::COHERENCE_SCORE` | Emitted every 20 frames with the current coherence score (from the combined pipeline in `lib.rs`). |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::coherence::{CoherenceMonitor, GateState};

let mut monitor = CoherenceMonitor::new();

let phases: Vec<f32> = get_csi_phases();
let score = monitor.process_frame(&phases);

match monitor.gate_state() {
    GateState::Accept => { /* full accuracy */ }
    GateState::Warn   => { /* predictions may be degraded */ }
    GateState::Reject => { /* sensing unreliable, recalibrate */ }
}
```

---

### Anomaly Detector (`adversarial.rs`)

**What it does**: Detects physically impossible or suspicious CSI signals that may indicate sensor malfunction, RF jamming, replay attacks, or environmental interference. It runs three independent checks on every frame.

**How it works**: During the first 100 frames it accumulates a baseline (mean amplitude per subcarrier and mean total energy). After calibration, it checks each frame for three anomaly types:

1. **Phase jump**: If more than 50% of subcarriers show a phase discontinuity greater than 2.5 radians, something non-physical happened.
2. **Amplitude flatline**: If amplitude variance across subcarriers is near zero (below 0.001) while the mean is nonzero, the sensor may be stuck.
3. **Energy spike**: If total signal energy exceeds 50x the baseline, an external source may be injecting power.

A 20-frame cooldown prevents event flooding.

#### API

| Item | Type | Description |
|------|------|-------------|
| `AnomalyDetector` | struct | Tracks baseline, previous phases, cooldown, and anomaly count. |
| `AnomalyDetector::new()` | `const fn` | Creates an uncalibrated detector. |
| `process_frame(&mut self, phases: &[f32], amplitudes: &[f32]) -> bool` | method | Returns `true` if an anomaly is detected on this frame. |
| `total_anomalies(&self) -> u32` | method | Lifetime count of detected anomalies. |

#### Configuration

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `PHASE_JUMP_THRESHOLD` | 2.5 rad | 1.0 -- pi | Phase jump to flag per subcarrier |
| `MIN_AMPLITUDE_VARIANCE` | 0.001 | 0.0001 -- 0.1 | Below this = flatline |
| `MAX_ENERGY_RATIO` | 50.0 | 5.0 -- 500.0 | Energy spike threshold vs baseline |
| `BASELINE_FRAMES` | 100 | 50 -- 500 | Frames to calibrate baseline |
| `ANOMALY_COOLDOWN` | 20 | 5 -- 100 | Frames between anomaly reports |

#### Events Emitted

| Event ID | Constant | When Emitted |
|----------|----------|-------------|
| 3 | `event_types::ANOMALY_DETECTED` | When any anomaly check fires (after cooldown). |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::adversarial::AnomalyDetector;

let mut detector = AnomalyDetector::new();

// First 100 frames calibrate the baseline (always returns false).
for _ in 0..100 {
    detector.process_frame(&phases, &amplitudes);
}

// Now anomalies are reported.
if detector.process_frame(&phases, &amplitudes) {
    log!("Signal anomaly detected! Total: {}", detector.total_anomalies());
}
```

---

### Intrusion Detector (`intrusion.rs`)

**What it does**: Detects unauthorized entry into a monitored area. It is designed for security applications with a bias toward low false-negative rate (it would rather alarm falsely than miss a real intrusion).

**How it works**: The detector goes through four states:

1. **Calibrating** (200 frames): Learns baseline amplitude mean and variance per subcarrier.
2. **Monitoring**: Waits for the environment to be quiet (low disturbance for 100 consecutive frames) before arming.
3. **Armed**: Actively watching. Computes a disturbance score combining phase velocity (60% weight) and amplitude deviation (40% weight). If disturbance exceeds 0.8 for 3 consecutive frames, it triggers an alert.
4. **Alert**: Intrusion detected. Returns to Armed once disturbance drops below 0.3 for 50 frames.

#### API

| Item | Type | Description |
|------|------|-------------|
| `IntrusionDetector` | struct | State machine with baseline, debounce, and cooldown. |
| `IntrusionDetector::new()` | `const fn` | Creates a detector in Calibrating state. |
| `process_frame(&mut self, phases: &[f32], amplitudes: &[f32]) -> &[(i32, f32)]` | method | Returns a slice of events (up to 4 per frame). |
| `state(&self) -> DetectorState` | method | Current state machine state. |
| `total_alerts(&self) -> u32` | method | Lifetime alert count. |
| `DetectorState` | enum | `Calibrating`, `Monitoring`, `Armed`, `Alert`. |

#### Configuration

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `INTRUSION_VELOCITY_THRESH` | 1.5 rad/frame | 0.5 -- 3.0 | Phase velocity that counts as fast movement |
| `AMPLITUDE_CHANGE_THRESH` | 3.0 sigma | 1.0 -- 10.0 | Amplitude deviation in standard deviations |
| `ARM_FRAMES` | 100 | 20 -- 500 | Quiet frames needed to arm (at 20 Hz: 5 sec) |
| `DETECT_DEBOUNCE` | 3 | 1 -- 10 | Consecutive detection frames before alert |
| `ALERT_COOLDOWN` | 100 | 20 -- 500 | Frames between alerts |
| `BASELINE_FRAMES` | 200 | 100 -- 1000 | Calibration window |

#### Events Emitted

| Event ID | Constant | When Emitted |
|----------|----------|-------------|
| 200 | `EVENT_INTRUSION_ALERT` | Intrusion detected. Value = disturbance score. |
| 201 | `EVENT_INTRUSION_ZONE` | Identifies which subcarrier zone has the most disturbance. |
| 202 | `EVENT_INTRUSION_ARMED` | Detector has armed after a quiet period. |
| 203 | `EVENT_INTRUSION_DISARMED` | Detector disarmed (not currently emitted). |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::intrusion::{IntrusionDetector, DetectorState};

let mut detector = IntrusionDetector::new();

// Calibrate and arm (feed quiet frames).
for _ in 0..300 {
    detector.process_frame(&quiet_phases, &quiet_amps);
}
assert_eq!(detector.state(), DetectorState::Armed);

// Now process live data.
let events = detector.process_frame(&live_phases, &live_amps);
for &(event_type, value) in events {
    if event_type == 200 {
        trigger_alarm(value);
    }
}
```

---

### Occupancy Detector (`occupancy.rs`)

**What it does**: Divides the sensing area into spatial zones (based on subcarrier groupings) and determines which zones are currently occupied by people. Useful for smart building applications such as HVAC control and lighting automation.

**How it works**: Subcarriers are divided into groups of 4, with each group representing a spatial zone (up to 8 zones). For each zone, the detector computes the variance of amplitude values within that group. During calibration (200 frames), it learns the baseline variance. After calibration, it computes the deviation from baseline, applies EMA smoothing (alpha=0.15), and uses a hysteresis threshold to classify each zone as occupied or empty. Events include per-zone occupancy (emitted every 10 frames) and zone transitions (emitted immediately on change).

#### API

| Item | Type | Description |
|------|------|-------------|
| `OccupancyDetector` | struct | Per-zone state, calibration accumulators, frame counter. |
| `OccupancyDetector::new()` | `const fn` | Creates uncalibrated detector. |
| `process_frame(&mut self, phases: &[f32], amplitudes: &[f32]) -> &[(i32, f32)]` | method | Returns events (up to 12 per frame). |
| `occupied_count(&self) -> u8` | method | Number of currently occupied zones. |
| `is_zone_occupied(&self, zone_id: usize) -> bool` | method | Check a specific zone. |

#### Configuration

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `MAX_ZONES` | 8 | 1 -- 16 | Maximum number of spatial zones |
| `ZONE_THRESHOLD` | 0.02 | 0.005 -- 0.5 | Score above this = occupied. Hysteresis exit at 0.5x |
| `ALPHA` | 0.15 | 0.05 -- 0.5 | EMA smoothing factor for zone scores |
| `BASELINE_FRAMES` | 200 | 100 -- 1000 | Calibration window length |

#### Events Emitted

| Event ID | Constant | When Emitted |
|----------|----------|-------------|
| 300 | `EVENT_ZONE_OCCUPIED` | Every 10 frames for each occupied zone. Value = `zone_id + confidence`. |
| 301 | `EVENT_ZONE_COUNT` | Every 10 frames. Value = total occupied zone count. |
| 302 | `EVENT_ZONE_TRANSITION` | Immediately on zone state change. Value = `zone_id + 0.5` (entered) or `zone_id + 0.0` (vacated). |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::occupancy::OccupancyDetector;

let mut detector = OccupancyDetector::new();

// Calibrate with empty-room data.
for _ in 0..200 {
    detector.process_frame(&empty_phases, &empty_amps);
}

// Live monitoring.
let events = detector.process_frame(&live_phases, &live_amps);
println!("Occupied zones: {}", detector.occupied_count());
println!("Zone 0 occupied: {}", detector.is_zone_occupied(0));
```

---

### Vital Trend Analyzer (`vital_trend.rs`)

**What it does**: Monitors breathing rate and heart rate over time and alerts on clinically significant conditions. It tracks 1-minute and 5-minute trends and detects apnea, bradypnea, tachypnea, bradycardia, and tachycardia.

**How it works**: Called at 1 Hz with current vital sign readings (from Tier 2 DSP). It pushes each reading into a 300-sample ring buffer (5-minute history). Each call checks for:

- **Apnea**: Breathing BPM below 1.0 for 20+ consecutive seconds.
- **Bradypnea**: Sustained breathing below 12 BPM (5+ consecutive samples).
- **Tachypnea**: Sustained breathing above 25 BPM (5+ consecutive samples).
- **Bradycardia**: Sustained heart rate below 50 BPM (5+ consecutive samples).
- **Tachycardia**: Sustained heart rate above 120 BPM (5+ consecutive samples).

Every 60 seconds, it emits 1-minute averages for both breathing and heart rate.

#### API

| Item | Type | Description |
|------|------|-------------|
| `VitalTrendAnalyzer` | struct | Two ring buffers (breathing, heartrate), debounce counters, apnea counter. |
| `VitalTrendAnalyzer::new()` | `const fn` | Creates analyzer with empty history. |
| `on_timer(&mut self, breathing_bpm: f32, heartrate_bpm: f32) -> &[(i32, f32)]` | method | Called at 1 Hz. Returns clinical alerts (up to 8). |
| `breathing_avg_1m(&self) -> f32` | method | 1-minute breathing rate average. |
| `breathing_trend_5m(&self) -> f32` | method | 5-minute breathing trend (positive = increasing). |

#### Configuration

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `BRADYPNEA_THRESH` | 12.0 BPM | 8 -- 15 | Below this = dangerously slow breathing |
| `TACHYPNEA_THRESH` | 25.0 BPM | 20 -- 35 | Above this = dangerously fast breathing |
| `BRADYCARDIA_THRESH` | 50.0 BPM | 40 -- 60 | Below this = dangerously slow heart rate |
| `TACHYCARDIA_THRESH` | 120.0 BPM | 100 -- 150 | Above this = dangerously fast heart rate |
| `APNEA_SECONDS` | 20 | 10 -- 60 | Seconds of near-zero breathing before alert |
| `ALERT_DEBOUNCE` | 5 | 2 -- 15 | Consecutive abnormal samples before alert |

#### Events Emitted

| Event ID | Constant | When Emitted |
|----------|----------|-------------|
| 100 | `EVENT_VITAL_TREND` | Reserved for generic trend events. |
| 101 | `EVENT_BRADYPNEA` | Sustained slow breathing. Value = current BPM. |
| 102 | `EVENT_TACHYPNEA` | Sustained fast breathing. Value = current BPM. |
| 103 | `EVENT_BRADYCARDIA` | Sustained slow heart rate. Value = current BPM. |
| 104 | `EVENT_TACHYCARDIA` | Sustained fast heart rate. Value = current BPM. |
| 105 | `EVENT_APNEA` | Breathing stopped. Value = seconds of apnea. |
| 110 | `EVENT_BREATHING_AVG` | 1-minute breathing average. Emitted every 60 seconds. |
| 111 | `EVENT_HEARTRATE_AVG` | 1-minute heart rate average. Emitted every 60 seconds. |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::vital_trend::VitalTrendAnalyzer;

let mut analyzer = VitalTrendAnalyzer::new();

// Called at 1 Hz from the on_timer WASM export.
let events = analyzer.on_timer(breathing_bpm, heartrate_bpm);
for &(event_type, value) in events {
    match event_type {
        105 => alert_apnea(value as u32),
        101 => alert_bradypnea(value),
        104 => alert_tachycardia(value),
        110 => log_breathing_avg(value),
        _ => {}
    }
}

// Query trend data.
let avg = analyzer.breathing_avg_1m();
let trend = analyzer.breathing_trend_5m();
```

---

### RVF Container (`rvf.rs`)

**What it does**: Defines the RVF (RuVector Format) binary container that packages a compiled WASM module with its manifest (name, author, capabilities, budget, hash) and an optional Ed25519 signature. This is the file format that gets uploaded to ESP32 nodes via the `/api/wasm/upload` endpoint.

**How it works**: The format has four sections laid out sequentially:

```
[Header: 32 bytes][Manifest: 96 bytes][WASM: N bytes][Signature: 0|64 bytes]
```

The header contains magic bytes (`RVF\x01`), format version, section sizes, and flags. The manifest describes the module's identity (name, author), resource requirements (max frame time, memory limit), and capability flags (which host APIs it needs). The WASM section is the raw compiled binary. The signature section is optional (indicated by `FLAG_HAS_SIGNATURE`) and covers everything before it.

The builder (available only with the `std` feature) creates RVF files from WASM binary data and a configuration struct. It automatically computes a SHA-256 hash of the WASM payload and embeds it in the manifest for integrity verification.

#### API

| Item | Type | Description |
|------|------|-------------|
| `RvfHeader` | `#[repr(C, packed)]` struct | 32-byte header with magic, version, section sizes. |
| `RvfManifest` | `#[repr(C, packed)]` struct | 96-byte manifest with module metadata. |
| `RvfConfig` | struct (std only) | Builder configuration input. |
| `build_rvf(wasm_data: &[u8], config: &RvfConfig) -> Vec<u8>` | function (std only) | Build a complete RVF container. |
| `patch_signature(rvf: &mut [u8], signature: &[u8; 64])` | function (std only) | Patch an Ed25519 signature into an existing RVF. |
| `RVF_MAGIC` | const (`0x0146_5652`) | Magic bytes: `RVF\x01` as little-endian u32. |
| `RVF_FORMAT_VERSION` | const (1) | Current format version. |
| `RVF_HEADER_SIZE` | const (32) | Header size in bytes. |
| `RVF_MANIFEST_SIZE` | const (96) | Manifest size in bytes. |
| `RVF_SIGNATURE_LEN` | const (64) | Ed25519 signature length. |
| `RVF_HOST_API_V1` | const (1) | Host API version this crate supports. |

#### Capability Flags

| Flag | Value | Description |
|------|-------|-------------|
| `CAP_READ_PHASE` | `1 << 0` | Module reads phase data |
| `CAP_READ_AMPLITUDE` | `1 << 1` | Module reads amplitude data |
| `CAP_READ_VARIANCE` | `1 << 2` | Module reads variance data |
| `CAP_READ_VITALS` | `1 << 3` | Module reads vital sign data |
| `CAP_READ_HISTORY` | `1 << 4` | Module reads phase history |
| `CAP_EMIT_EVENTS` | `1 << 5` | Module emits events |
| `CAP_LOG` | `1 << 6` | Module uses logging |
| `CAP_ALL` | `0x7F` | All capabilities |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::rvf::builder::{build_rvf, RvfConfig, patch_signature};
use wifi_densepose_wasm_edge::rvf::*;

// Read compiled WASM binary.
let wasm_data = std::fs::read("target/wasm32-unknown-unknown/release/my_module.wasm")?;

// Configure the module.
let config = RvfConfig {
    module_name: "my-gesture-v2".into(),
    author: "team-alpha".into(),
    capabilities: CAP_READ_PHASE | CAP_EMIT_EVENTS,
    max_frame_us: 5000,      // 5 ms budget per frame
    max_events_per_sec: 20,
    memory_limit_kb: 64,
    min_subcarriers: 8,
    max_subcarriers: 64,
    ..Default::default()
};

// Build the RVF container.
let rvf = build_rvf(&wasm_data, &config);

// Optionally sign and patch.
let signature = sign_with_ed25519(&rvf[..rvf.len() - RVF_SIGNATURE_LEN]);
let mut rvf_mut = rvf;
patch_signature(&mut rvf_mut, &signature);

// Upload to ESP32.
std::fs::write("my-gesture-v2.rvf", &rvf_mut)?;
```

---

## Testing

### Running Core Module Tests

From the crate directory:

```bash
cd rust-port/wifi-densepose-rs/crates/wifi-densepose-wasm-edge
cargo test --features std -- gesture coherence adversarial intrusion occupancy vital_trend rvf
```

This runs all tests whose names contain any of the seven module names. The `--features std` flag is required because the RVF builder tests need `sha2` and `std::io`.

### Expected Output

All tests should pass:

```
running 32 tests
test adversarial::tests::test_anomaly_detector_init ... ok
test adversarial::tests::test_calibration_phase ... ok
test adversarial::tests::test_normal_signal_no_anomaly ... ok
test adversarial::tests::test_phase_jump_detection ... ok
test adversarial::tests::test_amplitude_flatline_detection ... ok
test adversarial::tests::test_energy_spike_detection ... ok
test adversarial::tests::test_cooldown_prevents_flood ... ok
test coherence::tests::test_coherence_monitor_init ... ok
test coherence::tests::test_empty_phases_returns_current_score ... ok
test coherence::tests::test_first_frame_returns_one ... ok
test coherence::tests::test_constant_phases_high_coherence ... ok
test coherence::tests::test_incoherent_phases_lower_coherence ... ok
test coherence::tests::test_gate_hysteresis ... ok
test coherence::tests::test_mean_phasor_angle_zero_for_no_drift ... ok
test gesture::tests::test_gesture_detector_init ... ok
test gesture::tests::test_empty_phases_returns_none ... ok
test gesture::tests::test_first_frame_initializes ... ok
test gesture::tests::test_constant_phase_no_gesture_after_cooldown ... ok
test gesture::tests::test_dtw_identical_sequences ... ok
test gesture::tests::test_dtw_different_sequences ... ok
test gesture::tests::test_dtw_empty_input ... ok
test gesture::tests::test_cooldown_prevents_duplicate_detection ... ok
test gesture::tests::test_window_ring_buffer_wraps ... ok
test intrusion::tests::test_intrusion_init ... ok
test intrusion::tests::test_calibration_phase ... ok
test intrusion::tests::test_arm_after_quiet ... ok
test intrusion::tests::test_intrusion_detection ... ok
test occupancy::tests::test_occupancy_detector_init ... ok
test occupancy::tests::test_occupancy_calibration ... ok
test occupancy::tests::test_occupancy_detection ... ok
test vital_trend::tests::test_vital_trend_init ... ok
test vital_trend::tests::test_normal_vitals_no_alerts ... ok
test vital_trend::tests::test_apnea_detection ... ok
test vital_trend::tests::test_tachycardia_detection ... ok
test vital_trend::tests::test_breathing_average ... ok
test rvf::builder::tests::test_build_rvf_roundtrip ... ok
test rvf::builder::tests::test_build_hash_integrity ... ok
```

### Test Coverage Notes

| Module | Tests | Coverage |
|--------|-------|----------|
| `gesture.rs` | 8 | Init, empty input, first frame, constant input, DTW identical/different/empty, ring buffer wrap, cooldown |
| `coherence.rs` | 7 | Init, empty input, first frame, constant phases, incoherent phases, gate hysteresis, phasor angle |
| `adversarial.rs` | 7 | Init, calibration, normal signal, phase jump, flatline, energy spike, cooldown |
| `intrusion.rs` | 4 | Init, calibration, arming, intrusion detection |
| `occupancy.rs` | 3 | Init, calibration, zone detection |
| `vital_trend.rs` | 5 | Init, normal vitals, apnea, tachycardia, breathing average |
| `rvf.rs` | 2 | Build roundtrip, hash integrity |

## Common Patterns

All seven core modules share these design patterns:

### 1. Const-constructible state

Every module's main struct can be created with `const fn new()`, which means it can be placed in a `static` variable without runtime initialization. This is essential for WASM modules where there is no allocator.

```rust
static mut STATE: MyModule = MyModule::new();
```

### 2. Calibration-then-detect lifecycle

Modules that need a baseline (`adversarial`, `intrusion`, `occupancy`) follow the same pattern: accumulate statistics for N frames, compute mean/variance, then switch to detection mode. The calibration frame count is always a compile-time constant.

### 3. Ring buffer for history

Both `gesture` (phase deltas) and `vital_trend` (BPM readings) use fixed-size ring buffers with modular index arithmetic. The pattern is:

```rust
self.values[self.idx] = new_value;
self.idx = (self.idx + 1) % MAX_SIZE;
if self.len < MAX_SIZE { self.len += 1; }
```

### 4. Static event buffers

Modules that return multiple events per frame (`intrusion`, `occupancy`, `vital_trend`) use `static mut` arrays as return buffers to avoid heap allocation. This is safe in single-threaded WASM but requires `unsafe` blocks. The pattern is:

```rust
static mut EVENTS: [(i32, f32); N] = [(0, 0.0); N];
let mut n_events = 0;
// ... populate EVENTS[n_events] ...
unsafe { &EVENTS[..n_events] }
```

### 5. Cooldown/debounce

Every detection module uses a cooldown counter to prevent event flooding. After firing an event, the counter is set to a constant value and decremented each frame. No new events are emitted while the counter is positive.

### 6. EMA smoothing

Modules that track continuous scores (`coherence`, `occupancy`) use exponential moving average smoothing: `smoothed = alpha * raw + (1 - alpha) * smoothed`. The alpha constant controls responsiveness vs. stability.

### 7. Hysteresis thresholds

To prevent oscillation at detection boundaries, modules use different thresholds for entering and exiting a state. For example, the coherence monitor requires a score above 0.7 to enter Accept but only drops to Reject below 0.4.
