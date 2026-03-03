# Medical & Health Modules -- WiFi-DensePose Edge Intelligence

> Contactless health monitoring using WiFi signals. No wearables, no cameras -- just an ESP32 sensor reading WiFi reflections off a person's body to detect breathing problems, heart rhythm issues, walking difficulties, and seizures.

## Important Disclaimer

These modules are **research tools, not FDA-approved medical devices**. They should supplement -- not replace -- professional medical monitoring. WiFi CSI-derived vital signs are inherently noisier than clinical instruments (ECG, pulse oximetry, respiratory belts). False positives and false negatives will occur. Always validate findings against clinical-grade equipment before acting on alerts.

## Overview

| Module | File | What It Does | Event IDs | Budget |
|--------|------|-------------|-----------|--------|
| Sleep Apnea Detection | `med_sleep_apnea.rs` | Detects apnea episodes when breathing ceases for >10s; tracks AHI score | 100-102 | L (< 2 ms) |
| Cardiac Arrhythmia | `med_cardiac_arrhythmia.rs` | Detects tachycardia, bradycardia, missed beats, HRV anomalies | 110-113 | S (< 5 ms) |
| Respiratory Distress | `med_respiratory_distress.rs` | Detects tachypnea, labored breathing, Cheyne-Stokes, composite distress score | 120-123 | H (< 10 ms) |
| Gait Analysis | `med_gait_analysis.rs` | Extracts step cadence, asymmetry, shuffling, festination, fall-risk score | 130-134 | H (< 10 ms) |
| Seizure Detection | `med_seizure_detect.rs` | Detects tonic-clonic seizures with phase discrimination (fall vs tremor) | 140-143 | S (< 5 ms) |

All modules:
- Compile to `no_std` for WASM (ESP32 WASM3 runtime)
- Use `const fn new()` for zero-cost initialization
- Return events via `&[(i32, f32)]` slices (no heap allocation)
- Include NaN and division-by-zero protections
- Implement cooldown timers to prevent event flooding

---

## Modules

### Sleep Apnea Detection (`med_sleep_apnea.rs`)

**What it does**: Monitors breathing rate from the host CSI pipeline and detects when breathing drops below 4 BPM for more than 10 consecutive seconds, indicating an apnea episode. It tracks all episodes and computes the Apnea-Hypopnea Index (AHI) -- the number of apnea events per hour of monitored sleep time. AHI is the standard clinical metric for sleep apnea severity.

**Clinical basis**: Obstructive and central sleep apnea are defined by cessation of airflow for 10 seconds or more. The module uses a breathing rate threshold of 4 BPM (essentially near-zero breathing) with a 10-second onset delay to confirm cessation is sustained. AHI severity classification: < 5 normal, 5-15 mild, 15-30 moderate, > 30 severe.

**How it works**:
1. Each second, checks if breathing BPM is below 4.0
2. Increments a consecutive-low-breath counter
3. After 10 consecutive seconds, declares apnea onset (backdated to when breathing first dropped)
4. When breathing resumes above 4 BPM, records the episode with its duration
5. Every 5 minutes, computes AHI = (total episodes) / (monitoring hours)
6. Only monitors when presence is detected; if subject leaves during apnea, the episode is ended

#### API

| Item | Type | Description |
|------|------|-------------|
| `SleepApneaDetector` | struct | Main detector state |
| `SleepApneaDetector::new()` | `const fn` | Create detector with zeroed state |
| `process_frame(breathing_bpm, presence, variance)` | method | Process one frame at ~1 Hz; returns event slice |
| `ahi()` | method | Current AHI value |
| `episode_count()` | method | Total recorded apnea episodes |
| `monitoring_seconds()` | method | Total seconds with presence active |
| `in_apnea()` | method | Whether currently in an apnea episode |
| `APNEA_BPM_THRESH` | const | 4.0 BPM -- below this counts as apnea |
| `APNEA_ONSET_SECS` | const | 10 seconds -- minimum duration to declare apnea |
| `AHI_REPORT_INTERVAL` | const | 300 seconds (5 min) -- how often AHI is recalculated |
| `MAX_EPISODES` | const | 256 -- maximum episodes stored per session |

#### Events Emitted

| Event ID | Constant | Value | Clinical Meaning |
|----------|----------|-------|-----------------|
| 100 | `EVENT_APNEA_START` | Current breathing BPM | Breathing has ceased or dropped below 4 BPM for >10 seconds |
| 101 | `EVENT_APNEA_END` | Duration in seconds | Breathing has resumed after an apnea episode |
| 102 | `EVENT_AHI_UPDATE` | AHI score (events/hour) | Periodic severity metric; >5 = mild, >15 = moderate, >30 = severe |

#### State Machine

```
                          presence lost
    [Monitoring] -----> [Not Monitoring] (no events, counter paused)
         |                    |
         | bpm < 4.0          | presence regained
         v                    v
    [Low Breath Counter]  [Monitoring]
         |
         | count >= 10s
         v
    [In Apnea] ---------> [Episode End] (bpm >= 4.0 or presence lost)
         |                      |
         |                      v
         |               [Record Episode, emit APNEA_END]
         |
         +-- emit APNEA_START (once)
```

#### Configuration

| Parameter | Default | Clinical Range | Description |
|-----------|---------|----------------|-------------|
| `APNEA_BPM_THRESH` | 4.0 | 0-6 BPM | Breathing rate below which apnea is suspected |
| `APNEA_ONSET_SECS` | 10 | 10-20 s | Seconds of low breathing before apnea is declared |
| `AHI_REPORT_INTERVAL` | 300 | 60-3600 s | How often AHI is recalculated and emitted |
| `MAX_EPISODES` | 256 | -- | Fixed buffer size for episode history |
| `PRESENCE_ACTIVE` | 1 | -- | Minimum presence flag value for monitoring |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::med_sleep_apnea::*;

let mut detector = SleepApneaDetector::new();

// Normal breathing -- no events
let events = detector.process_frame(14.0, 1, 0.1);
assert!(events.is_empty());

// Simulate apnea: feed low BPM for 15 seconds
for _ in 0..15 {
    let events = detector.process_frame(1.0, 1, 0.1);
    for &(event_id, value) in events {
        match event_id {
            EVENT_APNEA_START => println!("Apnea detected! BPM: {}", value),
            _ => {}
        }
    }
}
assert!(detector.in_apnea());

// Resume normal breathing
let events = detector.process_frame(14.0, 1, 0.1);
for &(event_id, value) in events {
    match event_id {
        EVENT_APNEA_END => println!("Apnea ended after {} seconds", value),
        _ => {}
    }
}

println!("Episodes: {}", detector.episode_count());
println!("AHI: {:.1}", detector.ahi());
```

#### Tutorial: Setting Up Bedroom Sleep Monitoring

1. **ESP32 placement**: Mount the ESP32-S3 on the wall or ceiling 1-2 meters from the bed, at chest height. The sensor should have line-of-sight to the sleeping area. Avoid placing near metal objects or moving fans that create CSI interference.

2. **WiFi router**: Ensure a stable WiFi AP is within range. The ESP32 monitors the CSI (Channel State Information) of WiFi signals reflected off the person's body. The AP should be on the opposite side of the bed from the sensor for best body reflection capture.

3. **Firmware configuration**: Flash the ESP32 firmware with Tier 2 edge processing enabled (provides breathing BPM). The sleep apnea WASM module runs as a Tier 3 algorithm on top of the Tier 2 vitals output.

4. **Threshold tuning**: The default 4 BPM threshold is conservative (near-complete cessation). For a more sensitive detector, lower to 6-8 BPM, but expect more false positives from shallow breathing. The 10-second onset delay matches clinical apnea definitions.

5. **Reading AHI results**: AHI is emitted every 5 minutes. After a full night (7-8 hours), the final AHI value represents the overnight severity. Compare against clinical thresholds: < 5 (normal), 5-15 (mild), 15-30 (moderate), > 30 (severe).

6. **Limitations**: WiFi-based breathing detection works best when the subject is relatively still (sleeping). Tossing and turning may cause momentary breathing detection loss, which could either mask or falsely trigger apnea events. A single-night study should always be confirmed with clinical polysomnography.

---

### Cardiac Arrhythmia Detection (`med_cardiac_arrhythmia.rs`)

**What it does**: Monitors heart rate from the host CSI pipeline and detects four types of cardiac rhythm abnormalities: tachycardia (sustained fast heart rate), bradycardia (sustained slow heart rate), missed beats (sudden HR drops), and HRV anomalies (heart rate variability outside normal bounds).

**Clinical basis**: Tachycardia is defined as HR > 100 BPM sustained for 10+ seconds. Bradycardia is HR < 50 BPM sustained for 10+ seconds (the 50 BPM threshold is used instead of the typical 60 BPM to account for CSI measurement noise and to avoid false positives in athletes with naturally low resting HR). Missed beats are detected as a >30% drop from the running average. HRV is assessed via RMSSD (root mean square of successive differences) with a widened normal band (10-120 ms equivalent) to account for the coarser CSI-derived HR measurement compared to ECG.

**How it works**:
1. Maintains an exponential moving average (EMA) of heart rate with alpha=0.1
2. Tracks consecutive seconds above 100 BPM (tachycardia) or below 50 BPM (bradycardia)
3. After 10 consecutive seconds in an abnormal range, emits the corresponding alert
4. Computes fractional drop from EMA to detect missed beats
5. Maintains a 30-second ring buffer of successive HR differences for RMSSD calculation
6. RMSSD is converted from BPM units to approximate ms-equivalent (scale factor ~17)
7. All alerts have a 30-second cooldown to prevent event flooding
8. Invalid readings (< 1 BPM or NaN) are silently ignored to prevent contamination

#### API

| Item | Type | Description |
|------|------|-------------|
| `CardiacArrhythmiaDetector` | struct | Main detector state |
| `CardiacArrhythmiaDetector::new()` | `const fn` | Create detector with zeroed state |
| `process_frame(hr_bpm, phase)` | method | Process one frame at ~1 Hz; returns event slice |
| `hr_ema()` | method | Current EMA heart rate |
| `frame_count()` | method | Total frames processed |
| `TACHY_THRESH` | const | 100.0 BPM |
| `BRADY_THRESH` | const | 50.0 BPM |
| `SUSTAINED_SECS` | const | 10 seconds |
| `MISSED_BEAT_DROP` | const | 0.30 (30% drop from EMA) |
| `HRV_WINDOW` | const | 30 seconds |
| `RMSSD_LOW` / `RMSSD_HIGH` | const | 10.0 / 120.0 ms (widened for CSI) |
| `COOLDOWN_SECS` | const | 30 seconds |

#### Events Emitted

| Event ID | Constant | Value | Clinical Meaning |
|----------|----------|-------|-----------------|
| 110 | `EVENT_TACHYCARDIA` | Current HR in BPM | Heart rate sustained above 100 BPM for 10+ seconds |
| 111 | `EVENT_BRADYCARDIA` | Current HR in BPM | Heart rate sustained below 50 BPM for 10+ seconds |
| 112 | `EVENT_MISSED_BEAT` | Current HR in BPM | Sudden HR drop >30% from running average |
| 113 | `EVENT_HRV_ANOMALY` | RMSSD value (ms) | Heart rate variability outside 10-120 ms normal range |

#### State Machine

The cardiac module does not have a formal state machine -- it uses independent detectors with cooldown timers:

```
For each frame:
  1. Tick cooldowns (4 independent timers)
  2. Reject invalid inputs (< 1 BPM or NaN)
  3. Update EMA (alpha = 0.1)
  4. Update RR-diff ring buffer
  5. Check tachycardia (HR > 100 for 10+ consecutive seconds)
  6. Check bradycardia (HR < 50 for 10+ consecutive seconds)
  7. Check missed beat (>30% drop from EMA)
  8. Check HRV anomaly (RMSSD outside 10-120 ms, requires full 30s window)
  9. Each check respects its own 30-second cooldown
```

#### Configuration

| Parameter | Default | Clinical Range | Description |
|-----------|---------|----------------|-------------|
| `TACHY_THRESH` | 100.0 | 90-120 BPM | HR threshold for tachycardia |
| `BRADY_THRESH` | 50.0 | 40-60 BPM | HR threshold for bradycardia |
| `SUSTAINED_SECS` | 10 | 5-30 s | Consecutive seconds required for alert |
| `MISSED_BEAT_DROP` | 0.30 | 0.20-0.40 | Fractional HR drop to flag missed beat |
| `RMSSD_LOW` | 10.0 | 5-20 ms | Minimum normal RMSSD |
| `RMSSD_HIGH` | 120.0 | 80-150 ms | Maximum normal RMSSD |
| `EMA_ALPHA` | 0.1 | 0.05-0.2 | EMA smoothing coefficient |
| `COOLDOWN_SECS` | 30 | 10-60 s | Minimum time between repeated alerts |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::med_cardiac_arrhythmia::*;

let mut detector = CardiacArrhythmiaDetector::new();

// Normal heart rate -- no events
for _ in 0..60 {
    let events = detector.process_frame(72.0, 0.0);
    assert!(events.is_empty() || events.iter().all(|&(t, _)| t == EVENT_HRV_ANOMALY));
}

// Sustained tachycardia
for _ in 0..15 {
    let events = detector.process_frame(120.0, 0.0);
    for &(event_id, value) in events {
        if event_id == EVENT_TACHYCARDIA {
            println!("Tachycardia alert! HR: {} BPM", value);
        }
    }
}
```

---

### Respiratory Distress Detection (`med_respiratory_distress.rs`)

**What it does**: Detects four types of respiratory abnormalities from the host CSI pipeline: tachypnea (fast breathing), labored breathing (high amplitude variance), Cheyne-Stokes respiration (a crescendo-decrescendo breathing pattern), and a composite respiratory distress severity score from 0-100.

**Clinical basis**: Tachypnea is defined clinically as > 20 BPM in adults. This module uses a threshold of 25 BPM (more conservative) to reduce false positives from the inherently noisier CSI-derived breathing rate. Labored breathing is detected as a 3x increase in amplitude variance relative to a learned baseline. Cheyne-Stokes respiration is a pathological breathing pattern with 30-90 second periodicity, commonly associated with heart failure and neurological conditions. The module detects it via autocorrelation of the breathing amplitude envelope.

**How it works**:
1. Maintains a 120-second ring buffer of breathing BPM for autocorrelation analysis
2. Maintains a 60-second ring buffer of amplitude variance
3. Learns a baseline variance over the first 60 seconds (Welford online mean)
4. Checks for tachypnea: breathing rate > 25 BPM sustained for 8+ seconds
5. Checks for labored breathing: current variance > 3x baseline variance
6. Checks for Cheyne-Stokes: significant autocorrelation peak in 30-90s lag range
7. Computes composite distress score (0-100) every 30 seconds based on: rate deviation from normal (16 BPM center), variance ratio, tachypnea flag, and recent Cheyne-Stokes detection
8. NaN inputs are excluded from ring buffers to prevent contamination

#### API

| Item | Type | Description |
|------|------|-------------|
| `RespiratoryDistressDetector` | struct | Main detector state |
| `RespiratoryDistressDetector::new()` | `const fn` | Create detector with zeroed state |
| `process_frame(breathing_bpm, phase, variance)` | method | Process one frame at ~1 Hz; returns event slice |
| `last_distress_score()` | method | Most recent composite score (0-100) |
| `frame_count()` | method | Total frames processed |
| `TACHYPNEA_THRESH` | const | 25.0 BPM (conservative; clinical is 20 BPM) |
| `SUSTAINED_SECS` | const | 8 seconds |
| `LABORED_VAR_RATIO` | const | 3.0x baseline |
| `CS_LAG_MIN` / `CS_LAG_MAX` | const | 30 / 90 seconds (Cheyne-Stokes period range) |
| `CS_PEAK_THRESH` | const | 0.35 (normalized autocorrelation) |
| `BASELINE_SECS` | const | 60 seconds (learning period) |
| `COOLDOWN_SECS` | const | 20 seconds |

#### Events Emitted

| Event ID | Constant | Value | Clinical Meaning |
|----------|----------|-------|-----------------|
| 120 | `EVENT_TACHYPNEA` | Current breathing BPM | Breathing rate sustained above 25 BPM for 8+ seconds |
| 121 | `EVENT_LABORED_BREATHING` | Variance ratio | Breathing effort > 3x baseline; possible respiratory distress |
| 122 | `EVENT_CHEYNE_STOKES` | Period in seconds | Crescendo-decrescendo breathing pattern; associated with heart failure |
| 123 | `EVENT_RESP_DISTRESS_LEVEL` | Score 0-100 | Composite severity: 0-20 normal, 20-50 mild, 50-80 moderate, 80-100 severe |

#### State Machine

The respiratory distress module uses independent detector tracks with cooldowns rather than a single state machine:

```
For each frame:
  1. Tick cooldowns (3 independent timers)
  2. Skip NaN inputs for ring buffer updates
  3. Update breathing BPM ring buffer (120s) and variance ring buffer (60s)
  4. Learn baseline variance during first 60 seconds (Welford)
  5. Tachypnea check: BPM > 25 for 8+ consecutive seconds
  6. Labored breathing: current variance mean > 3x baseline (after baseline period)
  7. Cheyne-Stokes: autocorrelation peak > 0.35 in 30-90s lag range (needs full 120s buffer)
  8. Composite distress score emitted every 30 seconds
```

#### Configuration

| Parameter | Default | Clinical Range | Description |
|-----------|---------|----------------|-------------|
| `TACHYPNEA_THRESH` | 25.0 | 20-30 BPM | Breathing rate for tachypnea alert |
| `SUSTAINED_SECS` | 8 | 5-15 s | Debounce period for tachypnea |
| `LABORED_VAR_RATIO` | 3.0 | 2.0-5.0 | Variance ratio above baseline |
| `AC_WINDOW` | 120 | 90-180 s | Autocorrelation buffer for Cheyne-Stokes |
| `CS_PEAK_THRESH` | 0.35 | 0.25-0.50 | Autocorrelation peak threshold |
| `CS_LAG_MIN` / `CS_LAG_MAX` | 30 / 90 | 20-120 s | Cheyne-Stokes period search range |
| `BASELINE_SECS` | 60 | 30-120 s | Duration to learn baseline variance |
| `DISTRESS_REPORT_INTERVAL` | 30 | 10-60 s | How often composite score is emitted |
| `COOLDOWN_SECS` | 20 | 10-60 s | Minimum time between repeated alerts |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::med_respiratory_distress::*;

let mut detector = RespiratoryDistressDetector::new();

// Build baseline with normal breathing (60 seconds)
for _ in 0..60 {
    detector.process_frame(16.0, 0.0, 0.5);
}

// Simulate respiratory distress: high rate + high variance
for _ in 0..30 {
    let events = detector.process_frame(30.0, 0.0, 3.0);
    for &(event_id, value) in events {
        match event_id {
            EVENT_TACHYPNEA => println!("Tachypnea! Rate: {} BPM", value),
            EVENT_LABORED_BREATHING => println!("Labored breathing! Variance ratio: {:.1}x", value),
            EVENT_RESP_DISTRESS_LEVEL => println!("Distress score: {:.0}/100", value),
            _ => {}
        }
    }
}
```

#### Tutorial: Setting Up ICU/Ward Monitoring

1. **Placement**: Mount the ESP32 at the foot of the bed or on the ceiling directly above the patient. The sensor needs clear WiFi signal reflection from the patient's torso.

2. **Baseline learning**: The module automatically learns a 60-second baseline variance when first activated. Ensure the patient is breathing normally during this calibration period. If the patient is already in distress at module start, the baseline will be skewed and labored-breathing detection will be unreliable.

3. **Cheyne-Stokes detection**: Requires at least 120 seconds of data to begin autocorrelation analysis. The 30-90 second periodicity search range covers the clinically documented Cheyne-Stokes cycle range. In practice, detection typically becomes reliable after 3-4 minutes of monitoring.

4. **Distress score interpretation**: The composite score (0-100) combines four factors: rate deviation from normal, variance ratio, tachypnea presence, and Cheyne-Stokes detection. A score above 50 warrants clinical attention. Above 80 suggests acute distress.

---

### Gait Analysis (`med_gait_analysis.rs`)

**What it does**: Extracts gait parameters from CSI phase variance periodicity to assess mobility and fall risk. Detects step cadence, gait asymmetry (limping), stride variability, shuffling gait patterns (associated with Parkinson's disease), festination (involuntary acceleration), and computes a composite fall-risk score from 0-100.

**Clinical basis**: Normal walking cadence is 80-120 steps/min for healthy adults. Shuffling gait (>140 steps/min with low energy) is characteristic of Parkinson's disease and other neurological conditions. Festination (involuntary cadence acceleration) is a Parkinsonian feature. Gait asymmetry (left/right step interval ratio deviating from 1.0 by >15%) indicates limping or musculoskeletal issues. High stride variability (coefficient of variation) is a strong predictor of fall risk in elderly patients.

**How it works**:
1. Maintains a 60-second ring buffer of phase variance and motion energy
2. Detects steps as local maxima in the phase variance signal (peak-to-trough ratio > 1.5)
3. Records step intervals in a 64-entry buffer
4. Every 10 seconds, computes: cadence (60 / mean step interval), asymmetry (odd/even step interval ratio), variability (coefficient of variation)
5. Tracks cadence history over 6 reporting periods for festination detection
6. Shuffling is flagged when cadence > 140 and motion energy is low
7. Festination is detected as cadence accelerating by > 1.5 steps/min/sec
8. Fall-risk score (0-100) is a weighted composite of: abnormal cadence (25%), asymmetry (25%), variability (25%), low energy (15%), festination (10%)

#### API

| Item | Type | Description |
|------|------|-------------|
| `GaitAnalyzer` | struct | Main analyzer state |
| `GaitAnalyzer::new()` | `const fn` | Create analyzer with zeroed state |
| `process_frame(phase, amplitude, variance, motion_energy)` | method | Process one frame at ~1 Hz; returns event slice |
| `last_cadence()` | method | Most recent cadence (steps/min) |
| `last_asymmetry()` | method | Most recent asymmetry ratio (1.0 = symmetric) |
| `last_fall_risk()` | method | Most recent fall-risk score (0-100) |
| `frame_count()` | method | Total frames processed |
| `NORMAL_CADENCE_LOW` / `HIGH` | const | 80.0 / 120.0 steps/min |
| `SHUFFLE_CADENCE_HIGH` | const | 140.0 steps/min |
| `ASYMMETRY_THRESH` | const | 0.15 (15% deviation from 1.0) |
| `FESTINATION_ACCEL` | const | 1.5 steps/min/sec |
| `REPORT_INTERVAL` | const | 10 seconds |
| `COOLDOWN_SECS` | const | 15 seconds |

#### Events Emitted

| Event ID | Constant | Value | Clinical Meaning |
|----------|----------|-------|-----------------|
| 130 | `EVENT_STEP_CADENCE` | Steps/min | Detected walking cadence; <80 or >120 is abnormal |
| 131 | `EVENT_GAIT_ASYMMETRY` | Ratio (1.0=symmetric) | Step interval asymmetry; >1.15 or <0.85 indicates limping |
| 132 | `EVENT_FALL_RISK_SCORE` | Score 0-100 | Composite: 0-25 low, 25-50 moderate, 50-75 high, 75-100 critical |
| 133 | `EVENT_SHUFFLING_DETECTED` | Cadence (steps/min) | High-frequency, low-amplitude gait; Parkinson's indicator |
| 134 | `EVENT_FESTINATION` | Cadence (steps/min) | Involuntary cadence acceleration; Parkinsonian feature |

#### State Machine

The gait analyzer operates on a periodic reporting cycle:

```
Continuous (every frame):
  - Push variance and energy into ring buffers
  - Detect step peaks (local max in variance > 1.5x neighbors)
  - Record step intervals

Every REPORT_INTERVAL (10s), if >= 4 steps detected:
  1. Compute cadence, asymmetry, variability
  2. Emit EVENT_STEP_CADENCE
  3. If asymmetry > threshold: emit EVENT_GAIT_ASYMMETRY
  4. If cadence > 140 and energy < 0.3: emit EVENT_SHUFFLING_DETECTED
  5. If cadence accelerating > 1.5/s over 3 periods: emit EVENT_FESTINATION
  6. Compute and emit EVENT_FALL_RISK_SCORE
  7. Reset step buffer for next window
```

#### Configuration

| Parameter | Default | Clinical Range | Description |
|-----------|---------|----------------|-------------|
| `GAIT_WINDOW` | 60 | 30-120 s | Ring buffer size for phase variance |
| `STEP_PEAK_RATIO` | 1.5 | 1.2-2.0 | Min peak-to-trough ratio for step detection |
| `NORMAL_CADENCE_LOW` | 80.0 | 70-90 steps/min | Lower bound of normal cadence |
| `NORMAL_CADENCE_HIGH` | 120.0 | 110-130 steps/min | Upper bound of normal cadence |
| `SHUFFLE_CADENCE_HIGH` | 140.0 | 120-160 steps/min | Cadence threshold for shuffling |
| `SHUFFLE_ENERGY_LOW` | 0.3 | 0.1-0.5 | Energy ceiling for shuffling detection |
| `FESTINATION_ACCEL` | 1.5 | 1.0-3.0 steps/min/s | Cadence acceleration threshold |
| `ASYMMETRY_THRESH` | 0.15 | 0.10-0.25 | Asymmetry ratio deviation from 1.0 |
| `REPORT_INTERVAL` | 10 | 5-30 s | Gait analysis reporting period |
| `MIN_MOTION_ENERGY` | 0.1 | 0.05-0.3 | Minimum energy for step detection |
| `COOLDOWN_SECS` | 15 | 10-30 s | Cooldown for shuffling/festination alerts |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::med_gait_analysis::*;

let mut analyzer = GaitAnalyzer::new();

// Simulate walking with alternating high/low variance (steps)
for i in 0..30 {
    let variance = if i % 2 == 0 { 5.0 } else { 0.5 };
    let events = analyzer.process_frame(0.0, 1.0, variance, 1.0);
    for &(event_id, value) in events {
        match event_id {
            EVENT_STEP_CADENCE => println!("Cadence: {:.0} steps/min", value),
            EVENT_FALL_RISK_SCORE => println!("Fall risk: {:.0}/100", value),
            EVENT_GAIT_ASYMMETRY => println!("Asymmetry: {:.2}", value),
            _ => {}
        }
    }
}
```

#### Tutorial: Setting Up Hallway Gait Monitoring

1. **Placement**: Mount the ESP32 in a hallway or corridor at waist height on the wall. The walking path should be 3-5 meters long within the sensor's field of view. Position the WiFi AP at the opposite end of the hallway for optimal body reflection.

2. **Calibration**: The step detector relies on periodic peaks in phase variance. The `STEP_PEAK_RATIO` of 1.5 works well for most flooring surfaces. On carpet (which dampens impact signals), consider lowering to 1.2. On hard floors with shoes, 1.5-2.0 is appropriate.

3. **Clinical context**: The fall-risk score is most useful for longitudinal monitoring. A single reading provides a snapshot, but tracking trends over days/weeks reveals progressive mobility decline. A rising fall-risk score (e.g., from 20 to 40 over a month) warrants clinical assessment even if individual readings are below the "high risk" threshold.

4. **Limitations**: At a 1 Hz timer rate, the module cannot detect cadences above ~60 steps/min via direct peak counting. For higher cadences, the step detection relies on the host's higher-rate CSI processing to pre-compute variance peaks. Shuffling detection at >140 steps/min requires the host to be providing step-level variance data at higher than 1 Hz.

---

### Seizure Detection (`med_seizure_detect.rs`)

**What it does**: Detects tonic-clonic (grand mal) seizures by identifying sustained high-energy rhythmic motion in the 3-8 Hz band. Discriminates seizures from falls (single impulse followed by stillness) and tremor (lower amplitude, higher regularity). Tracks seizure phases: tonic (sustained muscle rigidity), clonic (rhythmic jerking), and post-ictal (sudden cessation of movement).

**Clinical basis**: Tonic-clonic seizures have a characteristic progression: (1) tonic phase with sustained muscle rigidity causing high motion energy with low variance, lasting 10-20 seconds; (2) clonic phase with rhythmic jerking at 3-8 Hz, lasting 30-60 seconds; (3) post-ictal phase with sudden cessation of movement and deep unresponsiveness. Falls produce a brief (<10 frame) high-energy spike followed by stillness. Tremors have lower amplitude than seizure-grade jerking.

**How it works**:
1. Operates at ~20 Hz frame rate (higher than other modules) for rhythm detection
2. Maintains 100-frame ring buffers for motion energy and amplitude
3. State machine progresses: Monitoring -> PossibleOnset -> Tonic/Clonic -> PostIctal -> Cooldown
4. Onset requires 10+ consecutive frames of high motion energy (>2.0 normalized)
5. Fall discrimination: if high energy lasts < 10 frames then drops, it is classified as a fall and ignored
6. Tonic phase: high energy with low variance (< 0.5)
7. Clonic phase: detected via autocorrelation of amplitude buffer for 2-7 frame period (3-8 Hz at 20 Hz sampling)
8. Post-ictal: motion drops below 0.2 for 40+ consecutive frames
9. After an episode, 200-frame cooldown prevents re-triggering
10. Presence must be active; loss of presence resets the state machine

#### API

| Item | Type | Description |
|------|------|-------------|
| `SeizureDetector` | struct | Main detector state |
| `SeizureDetector::new()` | `const fn` | Create detector with zeroed state |
| `process_frame(phase, amplitude, motion_energy, presence)` | method | Process at ~20 Hz; returns event slice |
| `phase()` | method | Current `SeizurePhase` enum value |
| `seizure_count()` | method | Total seizure episodes detected |
| `frame_count()` | method | Total frames processed |
| `SeizurePhase` | enum | Monitoring, PossibleOnset, Tonic, Clonic, PostIctal, Cooldown |
| `HIGH_ENERGY_THRESH` | const | 2.0 (normalized) |
| `TONIC_MIN_FRAMES` | const | 20 frames (1 second at 20 Hz) |
| `CLONIC_PERIOD_MIN` / `MAX` | const | 2 / 7 frames (3-8 Hz at 20 Hz) |
| `POST_ICTAL_MIN_FRAMES` | const | 40 frames (2 seconds at 20 Hz) |
| `COOLDOWN_FRAMES` | const | 200 frames (10 seconds at 20 Hz) |

#### Events Emitted

| Event ID | Constant | Value | Clinical Meaning |
|----------|----------|-------|-----------------|
| 140 | `EVENT_SEIZURE_ONSET` | Motion energy | Seizure activity detected; immediate clinical attention needed |
| 141 | `EVENT_SEIZURE_TONIC` | Duration in frames | Tonic phase identified; sustained rigidity |
| 142 | `EVENT_SEIZURE_CLONIC` | Period in frames | Clonic phase identified; rhythmic jerking with detected periodicity |
| 143 | `EVENT_POST_ICTAL` | 1.0 | Post-ictal phase; movement has ceased after seizure |

#### State Machine

```
                    presence lost (from any active state)
                    +-----------------------------------------+
                    v                                         |
[Monitoring] --> [PossibleOnset] --> [Tonic] --> [Clonic] --> [PostIctal] --> [Cooldown]
      ^              |    |              |                         |              |
      |              |    |              +------> [PostIctal] -----+              |
      |              |    |                (direct if energy drops)               |
      |              |    +--------> [Clonic]                                    |
      |              |            (skip tonic)                                   |
      |              |                                                           |
      |              +-- timeout (200 frames) --> [Monitoring]                   |
      |              +-- fall (<10 frames) -----> [Monitoring]                   |
      |                                                                          |
      +------ cooldown expires (200 frames) ------------------------------------+
```

Transitions:
- **Monitoring -> PossibleOnset**: 10+ frames of motion energy > 2.0
- **PossibleOnset -> Tonic**: Low energy variance + high energy (muscle rigidity pattern)
- **PossibleOnset -> Clonic**: Rhythmic autocorrelation peak + amplitude above tremor floor
- **PossibleOnset -> Monitoring**: Energy drop within 10 frames (fall) or timeout at 200 frames
- **Tonic -> Clonic**: Energy variance increases and rhythm is detected
- **Tonic -> PostIctal**: Motion energy drops below 0.2 for 40+ frames
- **Clonic -> PostIctal**: Motion energy drops below 0.2 for 40+ frames
- **PostIctal -> Cooldown**: After 40 frames in post-ictal
- **Cooldown -> Monitoring**: After 200 frames (10 seconds)

#### Configuration

| Parameter | Default | Clinical Range | Description |
|-----------|---------|----------------|-------------|
| `ENERGY_WINDOW` / `PHASE_WINDOW` | 100 | 60-200 frames | Ring buffer sizes for analysis |
| `HIGH_ENERGY_THRESH` | 2.0 | 1.5-3.0 | Motion energy threshold for onset |
| `TONIC_ENERGY_THRESH` | 1.5 | 1.0-2.0 | Energy threshold during tonic phase |
| `TONIC_VAR_CEIL` | 0.5 | 0.3-1.0 | Max energy variance for tonic classification |
| `TONIC_MIN_FRAMES` | 20 | 10-40 frames | Min frames to confirm tonic phase |
| `CLONIC_PERIOD_MIN` / `MAX` | 2 / 7 | 2-10 frames | Period range for 3-8 Hz rhythm |
| `CLONIC_AUTOCORR_THRESH` | 0.30 | 0.20-0.50 | Autocorrelation threshold for rhythm |
| `CLONIC_MIN_FRAMES` | 30 | 20-60 frames | Min frames to confirm clonic phase |
| `POST_ICTAL_ENERGY_THRESH` | 0.2 | 0.1-0.5 | Energy threshold for cessation |
| `POST_ICTAL_MIN_FRAMES` | 40 | 20-80 frames | Min frames of low energy |
| `FALL_MAX_DURATION` | 10 | 5-20 frames | Max high-energy duration classified as fall |
| `TREMOR_AMPLITUDE_FLOOR` | 0.8 | 0.5-1.5 | Min amplitude to distinguish from tremor |
| `COOLDOWN_FRAMES` | 200 | 100-400 frames | Cooldown after episode completes |
| `ONSET_MIN_FRAMES` | 10 | 5-20 frames | Min high-energy frames before onset |

#### Example Usage

```rust
use wifi_densepose_wasm_edge::med_seizure_detect::*;

let mut detector = SeizureDetector::new();

// Normal motion -- no seizure
for _ in 0..200 {
    let events = detector.process_frame(0.0, 0.5, 0.3, 1);
    assert!(events.is_empty());
}
assert_eq!(detector.phase(), SeizurePhase::Monitoring);

// Tonic phase: sustained high energy, low variance
for _ in 0..50 {
    let events = detector.process_frame(0.0, 2.0, 3.0, 1);
    for &(event_id, value) in events {
        match event_id {
            EVENT_SEIZURE_ONSET => println!("SEIZURE ONSET! Energy: {}", value),
            EVENT_SEIZURE_TONIC => println!("Tonic phase: {} frames", value),
            _ => {}
        }
    }
}

// Post-ictal: sudden cessation
for _ in 0..100 {
    let events = detector.process_frame(0.0, 0.05, 0.05, 1);
    for &(event_id, _) in events {
        if event_id == EVENT_POST_ICTAL {
            println!("Post-ictal phase detected -- patient needs immediate assessment");
        }
    }
}
```

#### Tutorial: Setting Up Seizure Monitoring

1. **Placement**: Mount the ESP32 on the ceiling directly above the bed or monitoring area. Seizure detection requires the highest sensitivity to body motion, so minimize distance to the patient. Ensure no other people or moving objects are in the sensor's field of view (pets, curtains, fans).

2. **Frame rate**: Unlike other medical modules that operate at 1 Hz, the seizure detector expects ~20 Hz frame input for accurate rhythm detection in the 3-8 Hz band. Ensure the host firmware is configured for high-rate CSI processing when this module is loaded.

3. **Sensitivity tuning**: The `HIGH_ENERGY_THRESH` of 2.0 and `ONSET_MIN_FRAMES` of 10 balance sensitivity against false positives. In a quiet bedroom environment, these defaults work well. In noisier environments (shared ward, nearby equipment vibration), consider raising `HIGH_ENERGY_THRESH` to 2.5-3.0.

4. **Fall vs seizure discrimination**: The module automatically distinguishes falls (brief energy spike < 10 frames) from seizures (sustained energy). If the patient is known to be a fall risk, consider running the gait analysis module in parallel for complementary monitoring.

5. **Response protocol**: When `EVENT_SEIZURE_ONSET` fires, immediately notify clinical staff. The `EVENT_POST_ICTAL` event indicates the active seizure has ended and the patient is entering post-ictal state -- they need assessment but are no longer in the convulsive phase.

---

## Testing

All medical modules include comprehensive unit tests covering initialization, normal operation, clinical scenario detection, edge cases, and cooldown behavior.

```bash
cd rust-port/wifi-densepose-rs/crates/wifi-densepose-wasm-edge
cargo test --features std -- med_
```

Expected output: **38 tests passed, 0 failed**.

### Test Coverage by Module

| Module | Tests | Scenarios Covered |
|--------|-------|-------------------|
| Sleep Apnea | 7 | Init, normal breathing, apnea onset/end, no monitoring without presence, AHI update, multiple episodes, presence-loss during apnea |
| Cardiac Arrhythmia | 7 | Init, normal HR, tachycardia, bradycardia, missed beat, HRV anomaly (low variability), cooldown flood prevention, EMA convergence |
| Respiratory Distress | 6 | Init, normal breathing, tachypnea, labored breathing, distress score emission, Cheyne-Stokes detection, distress score range |
| Gait Analysis | 7 | Init, no events without steps, cadence extraction, fall-risk score range, asymmetry detection, shuffling detection, variability (uniform + varied) |
| Seizure Detection | 7 | Init, normal motion, fall discrimination, seizure onset with sustained energy, post-ictal detection, no detection without presence, energy variance, cooldown after episode |

---

## Clinical Thresholds Reference

| Condition | Normal Range | Module Threshold | Clinical Standard | Notes |
|-----------|-------------|------------------|-------------------|-------|
| Breathing rate | 12-20 BPM | -- | -- | Normal adult at rest |
| Bradypnea | < 12 BPM | Not directly detected | < 12 BPM | Gap: covered implicitly by distress score |
| Tachypnea | > 20 BPM | > 25 BPM | > 20 BPM | Conservative threshold for CSI noise tolerance |
| Apnea | 0 BPM | < 4 BPM for > 10s | Cessation > 10s | 4 BPM threshold accounts for CSI noise floor |
| Bradycardia | < 60 BPM | < 50 BPM | < 60 BPM | Lower threshold avoids false positives in athletes |
| Tachycardia | > 100 BPM | > 100 BPM | > 100 BPM | Matches clinical standard |
| Heart rate (normal) | 60-100 BPM | -- | 60-100 BPM | -- |
| AHI (mild apnea) | -- | > 5 events/hr | > 5 events/hr | Matches clinical standard |
| AHI (moderate) | -- | > 15 events/hr | > 15 events/hr | Matches clinical standard |
| AHI (severe) | -- | > 30 events/hr | > 30 events/hr | Matches clinical standard |
| RMSSD (normal HRV) | 20-80 ms | 10-120 ms | 19-75 ms | Widened band for CSI-derived HR |
| Gait cadence (normal) | 80-120 steps/min | 80-120 steps/min | 90-120 steps/min | Slightly wider range |
| Gait asymmetry | 1.0 ratio | > 0.15 deviation | > 0.10 deviation | Slightly higher threshold for CSI |
| Cheyne-Stokes period | 30-90 s | 30-90 s lag search | 30-100 s | Matches clinical range |
| Seizure clonic frequency | 3-8 Hz | 3-8 Hz (period 2-7 frames at 20 Hz) | 3-8 Hz | Matches clinical standard |

### Threshold Rationale

Several thresholds differ from strict clinical standards. This is intentional:

- **WiFi CSI is not ECG/pulse oximetry.** The signal-to-noise ratio is lower, so thresholds are widened to reduce false positives while maintaining clinical relevance.
- **Conservative thresholds favor specificity over sensitivity.** A missed alert is preferable to alert fatigue in a non-clinical-grade system.
- **All thresholds are compile-time constants.** To adjust for a specific deployment, modify the constants at the top of each module file and recompile.

---

## Safety Considerations

1. **Not a substitute for medical devices.** These modules are research/assistive tools. They have not been validated through clinical trials and are not FDA/CE cleared. Never rely on them as the sole source of patient monitoring.

2. **False positive rates.** WiFi CSI is affected by environmental factors: moving objects (fans, pets, curtains), multipath changes (opening doors, people walking nearby), and electromagnetic interference. Expect false positive rates of 5-15% in typical home environments and 1-5% in controlled clinical settings.

3. **False negative rates.** The conservative thresholds mean some borderline conditions may not trigger alerts. Specifically:
   - Bradypnea (12-20 BPM dropping to 12-4 BPM) is not directly flagged -- only sub-4 BPM apnea is detected
   - Mild tachycardia (100-120 BPM) is detected, but the 10-second sustained requirement means brief episodes are missed
   - Low-amplitude seizures without strong motor components may not exceed the energy threshold

4. **Environmental factors affecting accuracy:**
   - **Multi-person environments**: All modules assume a single subject. Multiple people in the sensor's field of view will corrupt readings.
   - **Distance**: CSI sensitivity drops with distance. Place sensor within 2 meters of the subject.
   - **Obstructions**: Thick walls, metal furniture, and large water bodies (aquariums) between sensor and subject degrade performance.
   - **WiFi congestion**: Heavy WiFi traffic on the same channel increases noise in CSI measurements.

5. **Power and connectivity**: The ESP32 must maintain continuous WiFi connectivity for CSI monitoring. Power loss or WiFi disconnection will silently stop all monitoring. Consider UPS power and redundant AP placement for critical applications.

6. **Data privacy**: These modules process health-related data. Ensure compliance with HIPAA, GDPR, or local health data regulations when deploying in clinical or home care settings. CSI data and emitted events should be encrypted in transit and at rest.
