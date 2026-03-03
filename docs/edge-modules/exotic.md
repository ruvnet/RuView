# Exotic & Research Modules -- WiFi-DensePose Edge Intelligence

> Experimental sensing applications that push the boundaries of what WiFi
> signals can detect. From contactless sleep staging to sign language
> recognition, these modules explore novel uses of RF sensing. Some are
> highly experimental -- marked with their maturity level.

## Maturity Levels

- **Proven**: Based on published research with validated results
- **Experimental**: Working implementation, needs real-world validation
- **Research**: Proof of concept, exploratory

## Overview

| Module | File | What It Does | Event IDs | Maturity |
|--------|------|-------------|-----------|----------|
| Sleep Stage Classification | `exo_dream_stage.rs` | Classifies sleep phases from breathing + micro-movements | 600-603 | Experimental |
| Emotion Detection | `exo_emotion_detect.rs` | Estimates arousal/stress from physiological proxies | 610-613 | Research |
| Sign Language Recognition | `exo_gesture_language.rs` | DTW-based letter recognition from hand/arm CSI patterns | 620-623 | Research |
| Music Conductor Tracking | `exo_music_conductor.rs` | Extracts tempo, beat, dynamics from conducting motions | 630-634 | Research |
| Plant Growth Detection | `exo_plant_growth.rs` | Detects plant growth drift and circadian leaf movement | 640-643 | Research |
| Ghost Hunter (Anomaly) | `exo_ghost_hunter.rs` | Classifies unexplained perturbations in empty rooms | 650-653 | Experimental |
| Rain Detection | `exo_rain_detect.rs` | Detects rain from broadband structural vibrations | 660-662 | Experimental |
| Breathing Synchronization | `exo_breathing_sync.rs` | Detects phase-locked breathing between multiple people | 670-673 | Research |
| Time Crystal Detection | `exo_time_crystal.rs` | Detects period-doubling and temporal coordination | 680-682 | Research |
| Hyperbolic Space Embedding | `exo_hyperbolic_space.rs` | Poincare ball location classification with hierarchy | 685-687 | Research |

## Architecture

All modules share these design constraints:

- **`no_std`** -- no heap allocation, runs on WASM3 interpreter on ESP32-S3
- **`const fn new()`** -- all state is stack-allocated and const-constructible
- **Static event buffer** -- events are returned via `&[(i32, f32)]` from a static array (max 3-5 events per frame)
- **Budget-aware** -- each module declares its per-frame time budget (L/S/H)
- **Frame rate** -- all modules assume 20 Hz CSI frame rate from the host Tier 2 DSP

Shared utilities from `vendor_common.rs`:
- `CircularBuffer<N>` -- fixed-size ring buffer with O(1) push and indexed access
- `Ema` -- exponential moving average with configurable alpha
- `WelfordStats` -- online mean/variance computation (Welford's algorithm)

---

## Modules

### Sleep Stage Classification (`exo_dream_stage.rs`)

**What it does**: Classifies sleep phases (Awake, NREM Light, NREM Deep, REM) from breathing patterns, heart rate variability, and micro-movements -- without touching the person.

**Maturity**: Experimental

**Research basis**: WiFi-based contactless sleep monitoring has been demonstrated in peer-reviewed research. See [1] for RF-based sleep staging using breathing patterns and body movement.

#### How It Works

The module uses a four-feature state machine with hysteresis:

1. **Breathing regularity** -- Coefficient of variation (CV) of a 64-sample breathing BPM window. Low CV (<0.08) indicates deep sleep; high CV (>0.20) indicates REM or wakefulness.

2. **Motion energy** -- EMA-smoothed motion from host Tier 2. Below 0.15 = sleep-like; above 0.5 = awake.

3. **Heart rate variability (HRV)** -- Variance of recent HR BPM values. High HRV (>8.0) correlates with REM; very low HRV (<2.0) with deep sleep.

4. **Phase micro-movements** -- High-pass energy of the phase signal (successive differences). Captures muscle atonia disruption during REM.

Stage transitions require 10 consecutive frames of the candidate stage (hysteresis), preventing jittery classification.

#### Sleep Stages

| Stage | Code | Conditions |
|-------|------|-----------|
| Awake | 0 | No presence, high motion, or moderate motion + irregular breathing |
| NREM Light | 1 | Low motion, moderate breathing regularity, default sleep state |
| NREM Deep | 2 | Very low motion, very regular breathing (CV < 0.08), low HRV (< 2.0) |
| REM | 3 | Very low motion, high HRV (> 8.0), micro-movements above threshold |

#### Events

| Event | ID | Value | Frequency |
|-------|-----|-------|-----------|
| `SLEEP_STAGE` | 600 | 0-3 (Awake/Light/Deep/REM) | Every frame (after warmup) |
| `SLEEP_QUALITY` | 601 | Sleep efficiency [0, 100] | Every 20 frames |
| `REM_EPISODE` | 602 | Current/last REM episode length (frames) | When REM active or just ended |
| `DEEP_SLEEP_RATIO` | 603 | Deep/total sleep ratio [0, 1] | Every 20 frames |

#### Quality Metrics

- **Efficiency** = (sleep_frames / total_frames) * 100
- **Deep ratio** = deep_frames / sleep_frames
- **REM ratio** = rem_frames / sleep_frames

#### Configuration Constants

| Parameter | Default | Description |
|-----------|---------|-------------|
| `BREATH_HIST_LEN` | 64 | Rolling window for breathing BPM history |
| `HR_HIST_LEN` | 64 | Rolling window for heart rate history |
| `PHASE_BUF_LEN` | 128 | Phase buffer for micro-movement detection |
| `MOTION_ALPHA` | 0.1 | Motion EMA smoothing factor |
| `MIN_WARMUP` | 40 | Minimum frames before classification begins |
| `STAGE_HYSTERESIS` | 10 | Consecutive frames required for stage transition |

#### API

```rust
let mut detector = DreamStageDetector::new();
let events = detector.process_frame(
    breathing_bpm,   // f32: from Tier 2 DSP
    heart_rate_bpm,  // f32: from Tier 2 DSP
    motion_energy,   // f32: from Tier 2 DSP
    phase,           // f32: representative subcarrier phase
    variance,        // f32: representative subcarrier variance
    presence,        // i32: 1 if person detected, 0 otherwise
);
// events: &[(i32, f32)] -- event ID + value pairs

let stage = detector.stage();          // SleepStage enum
let eff = detector.efficiency();       // f32 [0, 100]
let deep = detector.deep_ratio();      // f32 [0, 1]
let rem = detector.rem_ratio();        // f32 [0, 1]
```

#### Tutorial: Setting Up Contactless Sleep Tracking

1. **Placement**: Mount the WiFi transmitter and receiver so the line of sight crosses the bed at chest height. Place the ESP32 node 1-3 meters from the bed.

2. **Calibration**: Let the system run for 40+ frames (2 seconds at 20 Hz) with the person in bed before expecting valid stage classifications.

3. **Interpreting Results**: Monitor `SLEEP_STAGE` events. A healthy sleep cycle progresses through Light -> Deep -> Light -> REM, repeating in ~90 minute cycles. The `SLEEP_QUALITY` event (601) gives an overall efficiency percentage -- above 85% is considered good.

4. **Limitations**: The module requires the Tier 2 DSP to provide valid `breathing_bpm` and `heart_rate_bpm`. If the person is too far from the WiFi path or behind thick walls, these vitals may not be detectable.

---

### Emotion Detection (`exo_emotion_detect.rs`)

**What it does**: Estimates continuous arousal level and discrete stress/calm/agitation states from WiFi CSI without cameras or microphones. Uses physiological proxies: breathing rate, heart rate, fidgeting, and phase variance.

**Maturity**: Research

**Limitations**: This module does NOT detect emotions directly. It detects physiological arousal -- elevated heart rate, rapid breathing, and fidgeting. These correlate with stress and anxiety but can also be caused by exercise, caffeine, or excitement. The module cannot distinguish between positive and negative arousal. It is a research tool for exploring the feasibility of affect sensing via RF, not a clinical instrument.

#### How It Works

The arousal level is a weighted sum of four normalized features:

| Feature | Weight | Source | Score = 0 | Score = 1 |
|---------|--------|--------|-----------|-----------|
| Breathing rate | 0.30 | Host Tier 2 | 6-10 BPM (calm) | >= 20 BPM (stressed) |
| Heart rate | 0.20 | Host Tier 2 | <= 70 BPM (baseline) | 100+ BPM (elevated) |
| Fidget energy | 0.30 | Motion successive diffs | No fidgeting | Continuous fidgeting |
| Phase variance | 0.20 | Subcarrier variance | Stable signal | Sharp body movements |

The stress index uses different weights (0.4/0.3/0.2/0.1) emphasizing breathing and heart rate over fidgeting.

#### Events

| Event | ID | Value | Frequency |
|-------|-----|-------|-----------|
| `AROUSAL_LEVEL` | 610 | Continuous arousal [0, 1] | Every frame |
| `STRESS_INDEX` | 611 | Stress index [0, 1] | Every frame |
| `CALM_DETECTED` | 612 | 1.0 when calm state detected | When conditions met |
| `AGITATION_DETECTED` | 613 | 1.0 when agitation detected | When conditions met |

#### Discrete State Detection

- **Calm**: arousal < 0.25 AND motion < 0.08 AND breathing 6-10 BPM AND breath CV < 0.08
- **Agitation**: arousal > 0.75 AND (motion > 0.6 OR fidget > 0.15 OR breath CV > 0.25)

#### API

```rust
let mut detector = EmotionDetector::new();
let events = detector.process_frame(
    breathing_bpm,   // f32
    heart_rate_bpm,  // f32
    motion_energy,   // f32
    phase,           // f32 (unused in current implementation)
    variance,        // f32
);

let arousal = detector.arousal();      // f32 [0, 1]
let stress = detector.stress_index();  // f32 [0, 1]
let calm = detector.is_calm();         // bool
let agitated = detector.is_agitated(); // bool
```

---

### Sign Language Recognition (`exo_gesture_language.rs`)

**What it does**: Classifies hand/arm movements into sign language letter groups using WiFi CSI phase and amplitude patterns. Uses DTW (Dynamic Time Warping) template matching on compact 6D feature sequences.

**Maturity**: Research

**Limitations**: Full 26-letter ASL alphabet recognition via WiFi is extremely challenging. This module provides a proof-of-concept framework. Real-world accuracy depends heavily on: (a) template quality and diversity, (b) environmental stability, (c) person-to-person variation. Expect proof-of-concept accuracy, not production ASL translation.

#### How It Works

1. **Feature extraction**: Per frame, compute 6 features: mean phase, phase spread, mean amplitude, amplitude spread, motion energy, variance. These are accumulated in a gesture window (max 32 frames).

2. **Gesture segmentation**: Active gestures are bounded by pauses (low motion for 15+ frames). When a pause is detected, the accumulated gesture window is matched against templates.

3. **DTW matching**: Each template is a reference feature sequence. Multivariate DTW with Sakoe-Chiba band (width=4) computes the alignment distance. The best match below threshold (0.5) is accepted.

4. **Word boundaries**: Extended pauses (15+ low-motion frames) emit word boundary events.

#### Events

| Event | ID | Value | Frequency |
|-------|-----|-------|-----------|
| `LETTER_RECOGNIZED` | 620 | Letter index (0=A, ..., 25=Z) | On match after pause |
| `LETTER_CONFIDENCE` | 621 | Inverse DTW distance [0, 1] | With recognized letter |
| `WORD_BOUNDARY` | 622 | 1.0 | After extended pause |
| `GESTURE_REJECTED` | 623 | 1.0 | When gesture does not match |

#### API

```rust
let mut detector = GestureLanguageDetector::new();

// Load templates (required before recognition works)
detector.load_synthetic_templates();  // 26 ramp-pattern templates for testing
// OR load custom templates:
detector.set_template(0, &features_for_letter_a);  // 0 = 'A'

let events = detector.process_frame(
    &phases,         // &[f32]: per-subcarrier phase
    &amplitudes,     // &[f32]: per-subcarrier amplitude
    variance,        // f32
    motion_energy,   // f32
    presence,        // i32
);
```

---

### Music Conductor Tracking (`exo_music_conductor.rs`)

**What it does**: Extracts musical conducting parameters from WiFi CSI motion signatures: tempo (BPM), beat position (1-4 in 4/4 time), dynamic level (MIDI velocity 0-127), and special gestures (cutoff and fermata).

**Maturity**: Research

**Research basis**: Gesture tracking via WiFi CSI has been demonstrated for coarse arm movements. Conductor tracking extends this to periodic rhythmic motion analysis.

#### How It Works

1. **Tempo detection**: Autocorrelation of a 128-point motion energy buffer at lags 4-64. The dominant peak determines the period, converted to BPM: `BPM = 60 * 20 / lag` (at 20 Hz frame rate). Valid range: 30-240 BPM.

2. **Beat position**: A modular frame counter relative to the detected period maps to beats 1-4 in 4/4 time.

3. **Dynamic level**: Motion energy relative to the EMA-smoothed peak, scaled to MIDI velocity [0, 127].

4. **Cutoff detection**: Sharp drop in motion energy (ratio < 0.2 of recent peak) with high preceding motion.

5. **Fermata detection**: Sustained low motion (< 0.05) for 10+ consecutive frames.

#### Events

| Event | ID | Value | Frequency |
|-------|-----|-------|-----------|
| `CONDUCTOR_BPM` | 630 | Detected tempo in BPM | After tempo lock |
| `BEAT_POSITION` | 631 | Beat number (1-4) | After tempo lock |
| `DYNAMIC_LEVEL` | 632 | MIDI velocity [0, 127] | Every frame |
| `GESTURE_CUTOFF` | 633 | 1.0 | On cutoff gesture |
| `GESTURE_FERMATA` | 634 | 1.0 | During fermata hold |

#### API

```rust
let mut detector = MusicConductorDetector::new();
let events = detector.process_frame(
    phase,           // f32 (unused)
    amplitude,       // f32 (unused)
    motion_energy,   // f32: from Tier 2 DSP
    variance,        // f32 (unused)
);

let bpm = detector.tempo_bpm();        // f32
let fermata = detector.is_fermata();   // bool
let cutoff = detector.is_cutoff();     // bool
```

---

### Plant Growth Detection (`exo_plant_growth.rs`)

**What it does**: Detects plant growth and leaf movement from micro-CSI changes over hours/days. Plants cause extremely slow, monotonic drift in CSI amplitude (growth) and diurnal phase oscillations (circadian leaf movement -- nyctinasty).

**Maturity**: Research

**Requirements**: Room must be empty (`presence == 0`) to isolate plant-scale perturbations from human motion. This module is designed for long-running monitoring (hours to days).

#### How It Works

- **Growth rate**: Tracks the slow drift of amplitude baseline via a very slow EWMA (alpha=0.0001, half-life ~175 seconds). Plant growth produces continuous ~0.01 dB/hour amplitude decrease as new leaf area intercepts RF energy.

- **Circadian phase**: Tracks peak-to-trough oscillation in phase EWMA over a rolling window. Nyctinastic leaf movement (folding at night) produces ~24-hour oscillations.

- **Wilting detection**: Short-term amplitude rises above baseline (less absorption) combined with reduced phase variance.

- **Watering event**: Abrupt amplitude drop (more water = more RF absorption) followed by recovery.

#### Events

| Event | ID | Value | Frequency |
|-------|-----|-------|-----------|
| `GROWTH_RATE` | 640 | Amplitude drift rate (scaled) | Every 100 empty-room frames |
| `CIRCADIAN_PHASE` | 641 | Oscillation magnitude [0, 1] | When oscillation detected |
| `WILT_DETECTED` | 642 | 1.0 | When wilting signature seen |
| `WATERING_EVENT` | 643 | 1.0 | When watering signature seen |

#### API

```rust
let mut detector = PlantGrowthDetector::new();
let events = detector.process_frame(
    &amplitudes,  // &[f32]: per-subcarrier amplitudes (up to 32)
    &phases,      // &[f32]: per-subcarrier phases (up to 32)
    &variance,    // &[f32]: per-subcarrier variance (up to 32)
    presence,     // i32: 0 = empty room (required for detection)
);

let calibrated = detector.is_calibrated();  // true after MIN_EMPTY_FRAMES
let empty = detector.empty_frames();        // frames of empty-room data
```

---

### Ghost Hunter -- Environmental Anomaly Detector (`exo_ghost_hunter.rs`)

**What it does**: Monitors CSI when no humans are detected for any perturbation above the noise floor. When the room should be empty but CSI changes are detected, something unexplained is happening. Classifies anomalies by their temporal signature.

**Maturity**: Experimental

**Practical applications**: Despite the playful name, this module has serious uses: detecting HVAC compressor cycling, pest/animal movement, structural settling, gas leaks (which alter dielectric properties), hidden intruders who evade the primary presence detector, and electromagnetic interference.

#### Anomaly Classification

| Class | Code | Signature | Typical Sources |
|-------|------|-----------|----------------|
| Impulsive | 1 | < 5 frames, sharp transient | Object falling, thermal cracking |
| Periodic | 2 | Recurring, detectable autocorrelation peak | HVAC, appliances, pest movement |
| Drift | 3 | 30+ frames same-sign amplitude delta | Temperature change, humidity, gas leak |
| Random | 4 | Stochastic, no pattern | EMI, co-channel WiFi interference |

#### Hidden Presence Detection

A sub-detector looks for breathing signatures in the phase signal: periodic oscillation at 0.2-2.0 Hz via autocorrelation at lags 5-15 (at 20 Hz frame rate). This can detect a motionless person who evades the main presence detector.

#### Events

| Event | ID | Value | Frequency |
|-------|-----|-------|-----------|
| `ANOMALY_DETECTED` | 650 | Energy level [0, 1] | When anomaly active |
| `ANOMALY_CLASS` | 651 | 1-4 (see table above) | With anomaly detection |
| `HIDDEN_PRESENCE` | 652 | Confidence [0, 1] | When breathing signature found |
| `ENVIRONMENTAL_DRIFT` | 653 | Drift magnitude | When sustained drift detected |

#### API

```rust
let mut detector = GhostHunterDetector::new();
let events = detector.process_frame(
    &phases,         // &[f32]
    &amplitudes,     // &[f32]
    &variance,       // &[f32]
    presence,        // i32: must be 0 for detection
    motion_energy,   // f32
);

let class = detector.anomaly_class();                // AnomalyClass enum
let hidden = detector.hidden_presence_confidence();   // f32 [0, 1]
let energy = detector.anomaly_energy();               // f32
```

---

### Rain Detection (`exo_rain_detect.rs`)

**What it does**: Detects rain from broadband CSI phase variance perturbations caused by raindrop impacts on building surfaces. Classifies intensity as light, moderate, or heavy.

**Maturity**: Experimental

**Research basis**: Raindrops impacting surfaces produce broadband impulse vibrations that propagate through building structure and modulate CSI phase. These are distinguishable from human motion by their broadband nature (all subcarrier groups affected equally), stochastic timing, and small amplitude.

#### How It Works

1. **Requires empty room** (`presence == 0`) to avoid confounding with human motion.
2. **Broadband criterion**: Compute per-group variance ratio (short-term / baseline). If >= 75% of groups (6/8) have elevated variance (ratio > 2.5x), the signal is broadband -- consistent with rain.
3. **Hysteresis state machine**: Onset requires 10 consecutive broadband frames; cessation requires 20 consecutive quiet frames.
4. **Intensity classification**: Based on smoothed excess energy above baseline.

#### Events

| Event | ID | Value | Frequency |
|-------|-----|-------|-----------|
| `RAIN_ONSET` | 660 | 1.0 | On rain start |
| `RAIN_INTENSITY` | 661 | 1=light, 2=moderate, 3=heavy | While raining |
| `RAIN_CESSATION` | 662 | 1.0 | On rain stop |

#### Intensity Thresholds

| Level | Code | Energy Range |
|-------|------|-------------|
| None | 0 | (not raining) |
| Light | 1 | energy < 0.3 |
| Moderate | 2 | 0.3 <= energy < 0.7 |
| Heavy | 3 | energy >= 0.7 |

#### API

```rust
let mut detector = RainDetector::new();
let events = detector.process_frame(
    &phases,      // &[f32]
    &variance,    // &[f32]
    &amplitudes,  // &[f32]
    presence,     // i32: must be 0
);

let raining = detector.is_raining();   // bool
let intensity = detector.intensity();  // RainIntensity enum
let energy = detector.energy();        // f32 [0, 1]
```

---

### Breathing Synchronization (`exo_breathing_sync.rs`)

**What it does**: Detects when multiple people's breathing patterns synchronize. Extracts per-person breathing components via subcarrier group decomposition and computes pairwise normalized cross-correlation.

**Maturity**: Research

**Research basis**: Breathing synchronization (interpersonal physiological synchrony) is a known phenomenon in couples, parent-infant pairs, and close social groups. This module attempts to detect it contactlessly via WiFi CSI.

#### How It Works

1. **Per-person decomposition**: With N persons, the 8 subcarrier groups are divided among persons (e.g., 2 persons = 4 groups each). Each person's phase signal is bandpass-filtered to the breathing band using dual EWMA (DC removal + low-pass).

2. **Pairwise correlation**: For each pair, compute normalized zero-lag cross-correlation over a 64-sample buffer: `rho = sum(x_i * x_j) / sqrt(sum(x_i^2) * sum(x_j^2))`

3. **Synchronization state machine**: High correlation (|rho| > 0.6) for 20+ consecutive frames declares synchronization. Low correlation for 15+ frames declares sync lost.

#### Events

| Event | ID | Value | Frequency |
|-------|-----|-------|-----------|
| `SYNC_DETECTED` | 670 | 1.0 | On sync onset |
| `SYNC_PAIR_COUNT` | 671 | Number of synced pairs | On count change |
| `GROUP_COHERENCE` | 672 | Average coherence [0, 1] | Every 10 frames |
| `SYNC_LOST` | 673 | 1.0 | On sync loss |

#### Constraints

- Maximum 4 persons (6 pairwise comparisons)
- Requires >= 8 subcarriers and >= 2 persons
- 64-frame warmup before analysis begins

#### API

```rust
let mut detector = BreathingSyncDetector::new();
let events = detector.process_frame(
    &phases,          // &[f32]: per-subcarrier phases
    &variance,        // &[f32]: per-subcarrier variance
    breathing_bpm,    // f32: host aggregate (unused internally)
    n_persons,        // i32: number of persons detected
);

let synced = detector.is_synced();           // bool
let coherence = detector.group_coherence();  // f32 [0, 1]
let persons = detector.active_persons();     // usize
```

---

### Time Crystal Detection (`exo_time_crystal.rs`)

**What it does**: Detects temporal symmetry breaking patterns -- specifically period doubling -- in motion energy. A "time crystal" in this context is when the system oscillates at a sub-harmonic of the driving frequency. Also counts independent non-harmonic periodic components as a "coordination index" for multi-person temporal coordination.

**Maturity**: Research

**Background**: In condensed matter physics, discrete time crystals exhibit period doubling under periodic driving. This module applies the same mathematical criterion (autocorrelation peak at lag L AND lag 2L) to human motion patterns. Two people walking at different cadences produce independent periodic peaks at non-harmonic ratios.

#### How It Works

1. **Autocorrelation**: 256-point motion energy buffer, autocorrelation at lags 1-128. Pre-linearized for performance (eliminates modulus ops in inner loop).

2. **Period doubling**: Search for peaks where a strong autocorrelation at lag L is accompanied by a strong peak at lag 2L (+/- 2 frame tolerance).

3. **Coordination index**: Count peaks whose lag ratios are not integer multiples of any other peak (within 5% tolerance). These represent independent periodic motions.

4. **Stability tracking**: Crystal detection is tracked over 200-frame windows. The stability score is the fraction of frames where the crystal was detected, EMA-smoothed.

#### Events

| Event | ID | Value | Frequency |
|-------|-----|-------|-----------|
| `CRYSTAL_DETECTED` | 680 | Period multiplier (2 = doubling) | When detected |
| `CRYSTAL_STABILITY` | 681 | Stability score [0, 1] | Every frame |
| `COORDINATION_INDEX` | 682 | Non-harmonic peak count | When > 0 |

#### API

```rust
let mut detector = TimeCrystalDetector::new();
let events = detector.process_frame(motion_energy);

let detected = detector.is_detected();          // bool
let multiplier = detector.multiplier();          // u8 (0 or 2)
let stability = detector.stability();            // f32 [0, 1]
let coordination = detector.coordination_index(); // u8
```

---

### Hyperbolic Space Embedding (`exo_hyperbolic_space.rs`)

**What it does**: Embeds CSI fingerprints into a 2D Poincare disk to exploit the natural hierarchy of indoor spaces (rooms contain zones). Hyperbolic geometry provides exponentially more representational capacity near the boundary, ideal for tree-structured location taxonomies.

**Maturity**: Research

**Research basis**: Hyperbolic embeddings have been shown to outperform Euclidean embeddings for hierarchical data (Nickel & Kiela, 2017). This module applies the concept to indoor localization.

#### How It Works

1. **Feature extraction**: 8D vector from mean amplitude across 8 subcarrier groups.
2. **Linear projection**: 2x8 matrix maps features to 2D Poincare disk coordinates.
3. **Normalization**: If the projected point exceeds the disk boundary, scale to radius 0.95.
4. **Nearest reference**: Compute Poincare distance to 16 reference points and find the closest.
5. **Hierarchy level**: Points near the center (radius < 0.5) are room-level; near the boundary are zone-level.

#### Poincare Distance

```
d(x, y) = acosh(1 + 2 * ||x-y||^2 / ((1 - ||x||^2) * (1 - ||y||^2)))
```

This metric respects the hyperbolic geometry: distances near the boundary grow exponentially.

#### Default Reference Layout

| Index | Label | Radius | Description |
|-------|-------|--------|-------------|
| 0-3 | Rooms | 0.3 | Bathroom, Kitchen, Living room, Bedroom |
| 4-6 | Zone 0a-c | 0.7 | Bathroom sub-zones |
| 7-9 | Zone 1a-c | 0.7 | Kitchen sub-zones |
| 10-12 | Zone 2a-c | 0.7 | Living room sub-zones |
| 13-15 | Zone 3a-c | 0.7 | Bedroom sub-zones |

#### Events

| Event | ID | Value | Frequency |
|-------|-----|-------|-----------|
| `HIERARCHY_LEVEL` | 685 | 0 = room, 1 = zone | Every frame |
| `HYPERBOLIC_RADIUS` | 686 | Disk radius [0, 1) | Every frame |
| `LOCATION_LABEL` | 687 | Nearest reference (0-15) | Every frame |

#### API

```rust
let mut embedder = HyperbolicEmbedder::new();
let events = embedder.process_frame(&amplitudes);

let label = embedder.label();        // u8 (0-15)
let pos = embedder.position();       // &[f32; 2]

// Custom calibration:
embedder.set_reference(0, [0.2, 0.1]);
embedder.set_projection_row(0, [0.05, 0.03, 0.02, 0.01, -0.01, -0.02, -0.03, -0.04]);
```

---

## Event ID Registry (600-699)

| Range | Module | Events |
|-------|--------|--------|
| 600-603 | Dream Stage | SLEEP_STAGE, SLEEP_QUALITY, REM_EPISODE, DEEP_SLEEP_RATIO |
| 610-613 | Emotion Detect | AROUSAL_LEVEL, STRESS_INDEX, CALM_DETECTED, AGITATION_DETECTED |
| 620-623 | Gesture Language | LETTER_RECOGNIZED, LETTER_CONFIDENCE, WORD_BOUNDARY, GESTURE_REJECTED |
| 630-634 | Music Conductor | CONDUCTOR_BPM, BEAT_POSITION, DYNAMIC_LEVEL, GESTURE_CUTOFF, GESTURE_FERMATA |
| 640-643 | Plant Growth | GROWTH_RATE, CIRCADIAN_PHASE, WILT_DETECTED, WATERING_EVENT |
| 650-653 | Ghost Hunter | ANOMALY_DETECTED, ANOMALY_CLASS, HIDDEN_PRESENCE, ENVIRONMENTAL_DRIFT |
| 660-662 | Rain Detect | RAIN_ONSET, RAIN_INTENSITY, RAIN_CESSATION |
| 670-673 | Breathing Sync | SYNC_DETECTED, SYNC_PAIR_COUNT, GROUP_COHERENCE, SYNC_LOST |
| 680-682 | Time Crystal | CRYSTAL_DETECTED, CRYSTAL_STABILITY, COORDINATION_INDEX |
| 685-687 | Hyperbolic Space | HIERARCHY_LEVEL, HYPERBOLIC_RADIUS, LOCATION_LABEL |

## Code Quality Notes

All 10 modules have been reviewed for:

- **Edge cases**: Division by zero is guarded everywhere (explicit checks before division, EPSILON constants). Negative variance from floating-point rounding is clamped to zero. Empty buffers return safe defaults.
- **NaN protection**: All computations use `libm` functions (`sqrtf`, `acoshf`, `sinf`) which are well-defined for valid inputs. Inputs are validated before reaching math functions.
- **Buffer safety**: All `CircularBuffer` accesses use the `get(i)` method which returns 0.0 for out-of-bounds indices. Fixed-size arrays prevent overflow.
- **Range clamping**: All outputs that represent ratios or probabilities are clamped to [0, 1]. MIDI velocity is clamped to [0, 127]. Poincare disk coordinates are normalized to radius < 1.
- **Test coverage**: Each module has 7-10 tests covering: construction, warmup period, happy path detection, edge cases (no presence, insufficient data), range validation, and reset.

## Research References

1. Liu, J., et al. "Monitoring Vital Signs and Postures During Sleep Using WiFi Signals." IEEE Internet of Things Journal, 2018. -- WiFi-based sleep monitoring using CSI breathing patterns.
2. Zhao, M., et al. "Through-Wall Human Pose Estimation Using Radio Signals." CVPR 2018. -- RF-based pose estimation foundations.
3. Wang, H., et al. "RT-Fall: A Real-Time and Contactless Fall Detection System with Commodity WiFi Devices." IEEE Transactions on Mobile Computing, 2017. -- WiFi CSI for human activity recognition.
4. Li, H., et al. "WiFinger: Talk to Your Smart Devices with Finger Gesture." UbiComp 2016. -- WiFi-based gesture recognition using CSI.
5. Ma, Y., et al. "SignFi: Sign Language Recognition Using WiFi." ACM IMWUT, 2018. -- WiFi CSI for sign language.
6. Nickel, M. & Kiela, D. "Poincare Embeddings for Learning Hierarchical Representations." NeurIPS 2017. -- Hyperbolic embedding foundations.
7. Wang, W., et al. "Understanding and Modeling of WiFi Signal Based Human Activity Recognition." MobiCom 2015. -- CSI-based activity recognition.
8. Adib, F., et al. "Smart Homes that Monitor Breathing and Heart Rate." CHI 2015. -- Contactless vital sign monitoring via RF signals.

## Contributing New Research Modules

### Adding a New Exotic Module

1. **Choose an event ID range**: Use the next available range in the 600-699 block. Check `lib.rs` event_types for allocated IDs.

2. **Create the source file**: Name it `exo_<name>.rs` in `src/`. Follow the existing pattern:
   - Module-level doc comment with algorithm description, events, and budget
   - `const fn new()` constructor
   - `process_frame()` returning `&[(i32, f32)]` via static buffer
   - Public accessor methods for key state
   - `reset()` method

3. **Register in `lib.rs`**: Add `pub mod exo_<name>;` in the Category 6 section.

4. **Register event constants**: Add entries to `event_types` in `lib.rs`.

5. **Update this document**: Add the module to the overview table and write its section.

6. **Testing requirements**:
   - At minimum: `test_const_new`, `test_warmup_no_events`, one happy-path detection test, `test_reset`
   - Test edge cases: empty input, extreme values, insufficient data
   - Verify all output values are in their documented ranges
   - Run: `cargo test --features std -- exo_` (from within the wasm-edge crate directory)

### Design Constraints

- **`no_std`**: No heap allocation. Use `CircularBuffer`, `Ema`, `WelfordStats` from `vendor_common`.
- **Stack budget**: Keep total struct size reasonable. The ESP32-S3 WASM3 stack is limited.
- **Time budget**: Stay within your declared budget (L < 2ms, S < 5ms, H < 10ms at 20 Hz).
- **Static events**: Use a `static mut EVENTS` array for zero-allocation event returns.
- **Input validation**: Always check array lengths, handle missing data gracefully.
