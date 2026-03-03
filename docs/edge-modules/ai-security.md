# AI Security Modules -- WiFi-DensePose Edge Intelligence

> Tamper detection and behavioral anomaly profiling that protect the sensing system from manipulation. These modules detect replay attacks, signal injection, jamming, and unusual behavior patterns -- all running on-device with no cloud dependency.

## Overview

| Module | File | What It Does | Event IDs | Budget |
|--------|------|--------------|-----------|--------|
| Signal Shield | `ais_prompt_shield.rs` | Detects replay, injection, and jamming attacks on CSI data | 820-823 | S (<5 ms) |
| Behavioral Profiler | `ais_behavioral_profiler.rs` | Learns normal behavior and detects anomalous deviations | 825-828 | S (<5 ms) |

---

## Signal Shield (`ais_prompt_shield.rs`)

**What it does**: Detects three types of attack on the WiFi sensing system:

1. **Replay attacks**: An adversary records legitimate CSI frames and plays them back to fool the sensor into seeing a "normal" scene while actually present in the room.
2. **Signal injection**: An adversary transmits a strong WiFi signal to overpower the legitimate CSI, creating amplitude spikes across many subcarriers.
3. **Jamming**: An adversary floods the WiFi channel with noise, degrading the signal-to-noise ratio below usable levels.

**How it works**:

- **Replay detection**: Each frame's features (mean phase, mean amplitude, amplitude variance) are quantized and hashed using FNV-1a. The hash is stored in a 64-entry ring buffer. If a new frame's hash matches any recent hash, it flags a replay.
- **Injection detection**: If more than 25% of subcarriers show a >10x amplitude jump from the previous frame, it flags injection.
- **Jamming detection**: The module calibrates a baseline SNR (signal / sqrt(variance)) over the first 100 frames. If the current SNR drops below 10% of baseline for 5+ consecutive frames, it flags jamming.

#### Public API

```rust
use wifi_densepose_wasm_edge::ais_prompt_shield::PromptShield;

let mut shield = PromptShield::new();                     // const fn, zero-alloc
let events = shield.process_frame(&phases, &amplitudes);  // per-frame analysis
let calibrated = shield.is_calibrated();                  // true after 100 frames
let frames = shield.frame_count();                        // total frames processed
```

#### Events

| Event ID | Constant | Value | Frequency |
|----------|----------|-------|-----------|
| 820 | `EVENT_REPLAY_ATTACK` | 1.0 (detected) | On detection (cooldown: 40 frames) |
| 821 | `EVENT_INJECTION_DETECTED` | Fraction of subcarriers with spikes [0.25, 1.0] | On detection (cooldown: 40 frames) |
| 822 | `EVENT_JAMMING_DETECTED` | SNR drop in dB (10 * log10(baseline/current)) | On detection (cooldown: 40 frames) |
| 823 | `EVENT_SIGNAL_INTEGRITY` | Composite integrity score [0.0, 1.0] | Every 20 frames |

#### Configuration Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_SC` | 32 | Maximum subcarriers processed |
| `HASH_RING` | 64 | Size of replay detection hash ring buffer |
| `INJECTION_FACTOR` | 10.0 | Amplitude jump threshold (10x previous) |
| `INJECTION_FRAC` | 0.25 | Minimum fraction of subcarriers with spikes |
| `JAMMING_SNR_FRAC` | 0.10 | SNR must drop below 10% of baseline |
| `JAMMING_CONSEC` | 5 | Consecutive low-SNR frames required |
| `BASELINE_FRAMES` | 100 | Calibration period length |
| `COOLDOWN` | 40 | Frames between repeated alerts (2 seconds at 20 Hz) |

#### Signal Integrity Score

The composite score (event 823) is emitted every 20 frames and ranges from 0.0 (compromised) to 1.0 (clean):

| Factor | Score Reduction | Condition |
|--------|-----------------|-----------|
| Replay detected | -0.4 | Frame hash matches ring buffer |
| Injection detected | up to -0.3 | Proportional to injection fraction |
| SNR degradation | up to -0.3 | Proportional to SNR drop below baseline |

#### FNV-1a Hash Details

The hash function quantizes three frame statistics to integer precision before hashing:

```
hash = FNV_OFFSET (2166136261)
for each of [mean_phase*100, mean_amp*100, amp_variance*100]:
    for each byte in value.to_le_bytes():
        hash ^= byte
        hash = hash.wrapping_mul(FNV_PRIME)   // FNV_PRIME = 16777619
```

This means two frames must have nearly identical statistical profiles (within 1% quantization) to trigger a replay alert.

#### Example: Detecting a Replay Attack

```
Calibration (frames 1-100):
  Normal CSI with varying phases -> baseline SNR established
  No alerts emitted during calibration

Frame 150: Normal operation
  phases = [0.31, 0.28, ...], amps = [1.02, 0.98, ...]
  hash = 0xA7F3B21C -> stored in ring buffer
  No alerts

Frame 200: Attacker replays frame 150 exactly
  phases = [0.31, 0.28, ...], amps = [1.02, 0.98, ...]
  hash = 0xA7F3B21C -> MATCH found in ring buffer!
  -> EVENT_REPLAY_ATTACK = 1.0
  -> EVENT_SIGNAL_INTEGRITY = 0.6 (reduced by 0.4)
```

#### Example: Detecting Signal Injection

```
Frame 300: Normal amplitudes
  amps = [1.0, 1.1, 0.9, 1.0, ...]

Frame 301: Adversary injects strong signal
  amps = [15.0, 12.0, 14.0, 13.0, ...]  (>10x jump on all subcarriers)
  injection_fraction = 1.0 (100% of subcarriers spiked)
  -> EVENT_INJECTION_DETECTED = 1.0
  -> EVENT_SIGNAL_INTEGRITY = 0.4
```

---

## Behavioral Profiler (`ais_behavioral_profiler.rs`)

**What it does**: Learns what "normal" behavior looks like over time, then detects anomalous deviations. It builds a 6-dimensional behavioral profile using online statistics (Welford's algorithm) and flags when new observations deviate significantly from the learned baseline.

**How it works**: Every 200 frames, the module computes a 6D feature vector from the observation window. During the learning phase (first 1000 frames), it trains Welford accumulators for each dimension. After maturity, it computes per-dimension Z-scores and a combined RMS Z-score. If the combined score exceeds 3.0, an anomaly is reported.

#### The 6 Behavioral Dimensions

| # | Dimension | Description | Typical Range |
|---|-----------|-------------|---------------|
| 0 | Presence Rate | Fraction of frames with presence | [0, 1] |
| 1 | Average Motion | Mean motion energy in window | [0, ~5] |
| 2 | Average Persons | Mean person count | [0, ~4] |
| 3 | Activity Variance | Variance of motion energy | [0, ~10] |
| 4 | Transition Rate | Presence state changes per frame | [0, 0.5] |
| 5 | Dwell Time | Average consecutive presence run length | [0, 200] |

#### Public API

```rust
use wifi_densepose_wasm_edge::ais_behavioral_profiler::BehavioralProfiler;

let mut bp = BehavioralProfiler::new();                   // const fn
let events = bp.process_frame(present, motion, n_persons); // per-frame
let mature = bp.is_mature();                               // true after learning
let anomalies = bp.total_anomalies();                      // cumulative count
let mean = bp.dim_mean(0);                                 // mean of dimension 0
let var = bp.dim_variance(1);                              // variance of dim 1
```

#### Events

| Event ID | Constant | Value | Frequency |
|----------|----------|-------|-----------|
| 825 | `EVENT_BEHAVIOR_ANOMALY` | Combined Z-score (RMS, > 3.0) | On detection (cooldown: 100 frames) |
| 826 | `EVENT_PROFILE_DEVIATION` | Index of most deviant dimension (0-5) | Paired with anomaly |
| 827 | `EVENT_NOVEL_PATTERN` | Count of dimensions with Z > 2.0 | When 3+ dimensions deviate |
| 828 | `EVENT_PROFILE_MATURITY` | Days since sensor start | On maturity + periodically |

#### Configuration Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `N_DIM` | 6 | Behavioral dimensions |
| `LEARNING_FRAMES` | 1000 | Frames before profiler matures |
| `ANOMALY_Z` | 3.0 | Combined Z-score threshold for anomaly |
| `NOVEL_Z` | 2.0 | Per-dimension Z-score threshold for novelty |
| `NOVEL_MIN` | 3 | Minimum deviating dimensions for NOVEL_PATTERN |
| `OBS_WIN` | 200 | Observation window size (frames) |
| `COOLDOWN` | 100 | Frames between repeated anomaly alerts |
| `MATURITY_INTERVAL` | 72000 | Frames between maturity reports (1 hour at 20 Hz) |

#### Welford's Online Algorithm

Each dimension maintains running statistics without storing all past values:

```
On each new observation x:
    count += 1
    delta = x - mean
    mean += delta / count
    m2 += delta * (x - mean)

Variance = m2 / count
Z-score  = |x - mean| / sqrt(variance)
```

This is numerically stable and requires only 12 bytes per dimension (count + mean + m2).

#### Example: Detecting an Intruder's Behavioral Signature

```
Learning phase (day 1-2):
  Normal pattern: 1 person, present 8am-10pm, moderate motion
  Profile matures -> EVENT_PROFILE_MATURITY = 0.58 (days)

Day 3, 3am:
  Observation window: presence=1, high motion, 1 person
  Z-scores: presence_rate=2.8, motion=4.1, persons=0.3,
            variance=3.5, transition=2.2, dwell=1.9
  Combined Z = sqrt(mean(z^2)) = 3.4 > 3.0
  -> EVENT_BEHAVIOR_ANOMALY = 3.4
  -> EVENT_PROFILE_DEVIATION = 1 (motion dimension most deviant)
  -> EVENT_NOVEL_PATTERN = 3 (3 dimensions above Z=2.0)
```

---

## Threat Model

### Attacks These Modules Detect

| Attack | Detection Module | Method | False Positive Rate |
|--------|-----------------|--------|---------------------|
| CSI frame replay | Signal Shield | FNV-1a hash ring matching | Low (1% quantization) |
| Signal injection (e.g., rogue AP) | Signal Shield | >25% subcarriers with >10x amplitude spike | Very low |
| Broadband jamming | Signal Shield | SNR drop below 10% of baseline for 5+ frames | Very low |
| Narrowband jamming | Partially -- Signal Shield | May not trigger if < 25% subcarriers affected | Medium |
| Behavioral anomaly (intruder at unusual time) | Behavioral Profiler | Combined Z-score > 3.0 across 6 dimensions | Low after maturation |
| Gradual environmental change | Behavioral Profiler | Welford stats adapt, may flag if change is abrupt | Very low |

### Attacks These Modules Cannot Detect

| Attack | Why Not | Recommended Mitigation |
|--------|---------|----------------------|
| Sophisticated replay with slight phase variation | FNV-1a uses 1% quantization; small perturbations change the hash | Add temporal correlation checks (consecutive frame deltas) |
| Man-in-the-middle on the WiFi channel | Modules analyze CSI content, not channel authentication | Use WPA3 encryption + MAC filtering |
| Physical obstruction (blocking line-of-sight) | Looks like a person leaving, not an attack | Cross-reference with PIR sensors |
| Slow amplitude drift (gradual injection) | Below the 10x threshold per frame | Add longer-term amplitude trend monitoring |
| Firmware tampering | Modules run in WASM sandbox, cannot detect host compromise | Secure boot + signed firmware (ADR-032) |

### Deployment Recommendations

1. **Always run both modules together**: Signal Shield catches active attacks, Behavioral Profiler catches passive anomalies.
2. **Allow full calibration**: Signal Shield needs 100 frames (5 seconds) for SNR baseline. Behavioral Profiler needs 1000 frames (~50 seconds) for reliable Z-scores.
3. **Combine with Temporal Logic Guard** (`tmp_temporal_logic_guard.rs`): Its safety invariants catch impossible state combinations (e.g., "fall alert when room is empty") that indicate sensor manipulation.
4. **Connect to the Self-Healing Mesh** (`aut_self_healing_mesh.rs`): If a node in the mesh is being jammed, the mesh can automatically reconfigure around the compromised node.

---

## Memory Layout

| Module | State Size (approx) | Static Event Buffer |
|--------|---------------------|---------------------|
| Signal Shield | ~420 bytes (64 hashes + 32 prev_amps + calibration) | 4 entries |
| Behavioral Profiler | ~2.4 KB (200-entry observation window + 6 Welford stats) | 4 entries |

Both modules use fixed-size arrays and static event buffers. No heap allocation. Fully no_std compliant.
