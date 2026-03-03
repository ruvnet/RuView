# Signal Intelligence Modules -- WiFi-DensePose Edge Intelligence

> Real-time WiFi signal analysis and enhancement running directly on the ESP32 chip. These modules clean, compress, and extract features from raw WiFi channel data so that higher-level modules (health, security, etc.) get better input.

## Overview

| Module | File | What It Does | Event IDs | Budget |
|--------|------|-------------|-----------|--------|
| Flash Attention | `sig_flash_attention.rs` | Focuses processing on the most informative subcarrier groups | 700-702 | S (<5ms) |
| Coherence Gate | `sig_coherence_gate.rs` | Filters out noisy/corrupted CSI frames using phase coherence | 710-712 | L (<2ms) |
| Temporal Compress | `sig_temporal_compress.rs` | Stores CSI history in 3-tier compressed circular buffer | 705-707 | S (<5ms) |
| Sparse Recovery | `sig_sparse_recovery.rs` | Recovers dropped subcarriers using ISTA sparse optimization | 715-717 | H (<10ms) |
| Min-Cut Person Match | `sig_mincut_person_match.rs` | Maintains stable person IDs across frames using bipartite matching | 720-722 | H (<10ms) |
| Optimal Transport | `sig_optimal_transport.rs` | Detects subtle motion via sliced Wasserstein distance | 725-727 | S (<5ms) |

## How Signal Processing Fits In

The signal intelligence modules form a processing pipeline between raw CSI data and application-level modules:

```
  Raw CSI from WiFi chipset (Tier 0-2 firmware DSP)
       |
       v
  +---------------------+     +---------------------+
  | Coherence Gate       | --> | Sparse Recovery      |
  | Reject noisy frames, |     | Fill in dropped      |
  | gate quality levels  |     | subcarriers via ISTA  |
  +---------------------+     +---------------------+
       |                              |
       v                              v
  +---------------------+     +---------------------+
  | Flash Attention      |     | Temporal Compress    |
  | Focus on informative |     | Store CSI history    |
  | subcarrier groups    |     | at 3 quality tiers   |
  +---------------------+     +---------------------+
       |                              |
       v                              v
  +---------------------+     +---------------------+
  | Min-Cut Person Match |     | Optimal Transport    |
  | Track person IDs     |     | Detect subtle motion |
  | across frames        |     | via distribution     |
  +---------------------+     +---------------------+
       |                              |
       v                              v
  Application modules: Health, Security, Smart Building, etc.
```

The **Coherence Gate** acts as a quality filter at the top of the pipeline. Frames that pass the gate feed into the **Sparse Recovery** module (if subcarrier dropout is detected) and then into downstream analysis. **Flash Attention** identifies which spatial regions carry the most signal, while **Temporal Compress** maintains an efficient rolling history. **Min-Cut Person Match** and **Optimal Transport** extract higher-level features (person identity and motion) that application modules consume.

## Shared Utilities (`vendor_common.rs`)

All signal intelligence modules share these utilities from `vendor_common.rs`:

| Utility | Purpose |
|---------|---------|
| `CircularBuffer<N>` | Fixed-size ring buffer for phase history, stack-allocated |
| `Ema` | Exponential moving average with configurable alpha |
| `WelfordStats` | Online mean/variance/stddev in O(1) memory |
| `dot_product`, `l2_norm`, `cosine_similarity` | Fixed-size vector math |
| `dtw_distance`, `dtw_distance_banded` | Dynamic Time Warping for gesture/pattern matching |
| `FixedPriorityQueue<CAP>` | Top-K selection without heap allocation |

---

## Modules

### Flash Attention (`sig_flash_attention.rs`)

**What it does**: Focuses processing on the WiFi channels that carry the most useful information -- ignores noise. Divides 32 subcarriers into 8 groups and computes attention weights showing where signal activity is concentrated.

**Algorithm**: Tiled attention (Q*K/sqrt(d)) over 8 subcarrier groups with softmax normalization and Shannon entropy tracking.

1. Compute group means: Q = current phase per group, K = previous phase per group, V = amplitude per group
2. Score each group: `score[g] = Q[g] * K[g] / sqrt(8)`
3. Softmax normalization (numerically stable: subtract max before exp)
4. Track entropy H = -sum(p * ln(p)) via EMA smoothing

Low entropy means activity is focused in one spatial zone (a Fresnel region); high entropy means activity is spread uniformly.

#### Public API

```rust
pub struct FlashAttention { /* ... */ }

impl FlashAttention {
    pub const fn new() -> Self;
    pub fn process_frame(&mut self, phases: &[f32], amplitudes: &[f32]) -> &[(i32, f32)];
    pub fn weights() -> &[f32; 8];       // Current attention weights per group
    pub fn entropy() -> f32;             // EMA-smoothed entropy [0, ln(8)]
    pub fn peak_group() -> usize;        // Group index with highest weight
    pub fn centroid() -> f32;            // Weighted centroid position [0, 7]
    pub fn frame_count() -> u32;
    pub fn reset(&mut self);
}
```

#### Events

| ID | Name | Value | Meaning |
|----|------|-------|---------|
| 700 | `ATTENTION_PEAK_SC` | Group index (0-7) | Which subcarrier group has the strongest attention weight |
| 701 | `ATTENTION_SPREAD` | Entropy (0 to ~2.08) | How spread out the attention is (low = focused, high = uniform) |
| 702 | `SPATIAL_FOCUS_ZONE` | Centroid (0.0-7.0) | Weighted center of attention across groups |

#### Configuration

| Constant | Value | Purpose |
|----------|-------|---------|
| `N_GROUPS` | 8 | Number of subcarrier groups (tiles) |
| `MAX_SC` | 32 | Maximum subcarriers processed |
| `ENTROPY_ALPHA` | 0.15 | EMA smoothing factor for entropy |

#### Tutorial: Understanding Attention Weights

The 8 attention weights sum to 1.0. When a person stands in a particular area of the room, the WiFi signal changes most in the subcarrier group(s) whose Fresnel zones intersect that area.

- **All weights near 0.125 (= 1/8)**: Uniform attention. No localized activity -- either an empty room or whole-body motion affecting all subcarriers equally.
- **One weight near 1.0, others near 0.0**: Highly focused. Activity concentrated in one spatial zone. The `peak_group` index tells you which zone.
- **Two adjacent groups elevated**: Activity at the boundary between two spatial zones, or a person moving between them.
- **Entropy below 1.0**: Strong spatial focus. Good for zone-level localization.
- **Entropy above 1.8**: Nearly uniform. Hard to localize activity.

The `centroid` value (0.0 to 7.0) gives a weighted average position. Tracking centroid over time reveals motion direction across the room.

---

### Coherence Gate (`sig_coherence_gate.rs`)

**What it does**: Decides whether each incoming CSI frame is trustworthy enough to use for sensing, or should be discarded. Uses the statistical consistency of phase changes across subcarriers to measure signal quality.

**Algorithm**: Per-subcarrier phase deltas form unit phasors (cos + i*sin). The magnitude of the mean phasor is the coherence score [0,1]. Welford online statistics track mean/variance for Z-score computation. A hysteresis state machine prevents rapid oscillation between states.

State transitions:
- Accept -> PredictOnly: 5 consecutive frames below LOW_THRESHOLD (0.40)
- PredictOnly -> Reject: single frame below threshold
- Reject/PredictOnly -> Accept: 10 consecutive frames above HIGH_THRESHOLD (0.75)
- Any -> Recalibrate: running variance exceeds 4x the initial snapshot

#### Public API

```rust
pub struct CoherenceGate { /* ... */ }

impl CoherenceGate {
    pub const fn new() -> Self;
    pub fn process_frame(&mut self, phases: &[f32]) -> &[(i32, f32)];
    pub fn gate() -> GateDecision;       // Accept/PredictOnly/Reject/Recalibrate
    pub fn coherence() -> f32;           // Last coherence score [0, 1]
    pub fn zscore() -> f32;              // Z-score of last coherence
    pub fn variance() -> f32;            // Running variance of coherence
    pub fn frame_count() -> u32;
    pub fn reset(&mut self);
}

pub enum GateDecision { Accept, PredictOnly, Reject, Recalibrate }
```

#### Events

| ID | Name | Value | Meaning |
|----|------|-------|---------|
| 710 | `GATE_DECISION` | 2/1/0/-1 | Accept(2), PredictOnly(1), Reject(0), Recalibrate(-1) |
| 711 | `COHERENCE_SCORE` | [0.0, 1.0] | Phase phasor coherence magnitude |
| 712 | `RECALIBRATE_NEEDED` | Variance | Environment has changed significantly -- retrain baseline |

#### Configuration

| Constant | Value | Purpose |
|----------|-------|---------|
| `HIGH_THRESHOLD` | 0.75 | Coherence above this = good quality |
| `LOW_THRESHOLD` | 0.40 | Coherence below this = poor quality |
| `DEGRADE_COUNT` | 5 | Consecutive bad frames before degrading |
| `RECOVER_COUNT` | 10 | Consecutive good frames before recovering |
| `VARIANCE_DRIFT_MULT` | 4.0 | Variance multiplier triggering recalibrate |

#### Tutorial: Using the Coherence Gate

The coherence gate protects downstream modules from processing garbage data. In practice:

1. **Accept** (value=2): Frame is clean. Use it for all sensing tasks (vitals, presence, gestures).
2. **PredictOnly** (value=1): Frame quality is marginal. Use cached predictions from previous frames; do not update models.
3. **Reject** (value=0): Frame is too noisy. Skip entirely. Do not feed to any learning module.
4. **Recalibrate** (value=-1): The environment has changed fundamentally (furniture moved, new AP, door opened). Reset baselines and re-learn.

Common causes of low coherence:
- Microwave oven running (2.4 GHz interference)
- Multiple people walking in different directions (phase cancellation)
- Hardware glitch (intermittent antenna contact)

---

### Temporal Compress (`sig_temporal_compress.rs`)

**What it does**: Maintains a rolling history of up to 512 CSI snapshots in compressed form. Recent data is stored at high precision; older data is progressively compressed to save memory while retaining long-term trends.

**Algorithm**: Three-tier quantization with automatic demotion at age boundaries.

| Tier | Age Range | Bits | Quantization Levels | Max Error |
|------|-----------|------|---------------------|-----------|
| Hot | 0-63 (newest) | 8-bit | 256 | <0.5% |
| Warm | 64-255 | 5-bit | 32 | <3% |
| Cold | 256-511 | 3-bit | 8 | <15% |

At 20 Hz, the buffer stores approximately:
- Hot: 3.2 seconds of high-fidelity data
- Warm: 9.6 seconds of medium-fidelity data
- Cold: 12.8 seconds of low-fidelity data
- Total: ~25.6 seconds, or longer at lower frame rates

Each snapshot stores 8 phase + 8 amplitude values (group means), plus a scale factor and tier tag.

#### Public API

```rust
pub struct TemporalCompressor { /* ... */ }

impl TemporalCompressor {
    pub const fn new() -> Self;
    pub fn push_frame(&mut self, phases: &[f32], amps: &[f32], ts_ms: u32) -> &[(i32, f32)];
    pub fn on_timer() -> &[(i32, f32)];
    pub fn get_snapshot(age: usize) -> Option<[f32; 16]>;  // Decompressed 8 phase + 8 amp
    pub fn compression_ratio() -> f32;
    pub fn frame_rate() -> f32;
    pub fn total_written() -> u32;
    pub fn occupied() -> usize;
}
```

#### Events

| ID | Name | Value | Meaning |
|----|------|-------|---------|
| 705 | `COMPRESSION_RATIO` | Ratio (>1.0) | Raw bytes / compressed bytes |
| 706 | `TIER_TRANSITION` | Tier (1 or 2) | A snapshot was demoted to Warm(1) or Cold(2) |
| 707 | `HISTORY_DEPTH_HOURS` | Hours | How much wall-clock time the buffer covers |

#### Configuration

| Constant | Value | Purpose |
|----------|-------|---------|
| `CAP` | 512 | Total snapshot capacity |
| `HOT_END` | 64 | First N snapshots at 8-bit precision |
| `WARM_END` | 256 | Snapshots 64-255 at 5-bit precision |
| `RATE_ALPHA` | 0.05 | EMA alpha for frame rate estimation |

---

### Sparse Recovery (`sig_sparse_recovery.rs`)

**What it does**: When WiFi hardware drops some subcarrier measurements (nulls/zeros due to deep fades, firmware glitches, or multipath nulls), this module reconstructs the missing values using mathematical optimization.

**Algorithm**: Iterative Shrinkage-Thresholding Algorithm (ISTA) -- an L1-minimizing sparse recovery method.

```
x_{k+1} = soft_threshold(x_k + step * A^T * (b - A*x_k), lambda)
```

where:
- `A` is a tridiagonal correlation model (diagonal + immediate neighbors, 96 f32s instead of full 32x32=1024)
- `b` is the observed (non-null) subcarrier values
- `soft_threshold(x, t) = sign(x) * max(|x| - t, 0)` promotes sparsity
- Maximum 10 iterations per frame

The correlation model is learned online from valid frames using EMA-blended products.

#### Public API

```rust
pub struct SparseRecovery { /* ... */ }

impl SparseRecovery {
    pub const fn new() -> Self;
    pub fn process_frame(&mut self, amplitudes: &mut [f32]) -> &[(i32, f32)];
    pub fn dropout_rate() -> f32;           // Fraction of null subcarriers
    pub fn last_residual_norm() -> f32;     // L2 residual from last recovery
    pub fn last_recovered_count() -> u32;   // How many subcarriers were recovered
    pub fn is_initialized() -> bool;        // Whether correlation model is ready
}
```

Note: `process_frame` modifies `amplitudes` in place -- null subcarriers are overwritten with recovered values.

#### Events

| ID | Name | Value | Meaning |
|----|------|-------|---------|
| 715 | `RECOVERY_COMPLETE` | Count | Number of subcarriers recovered |
| 716 | `RECOVERY_ERROR` | L2 norm | Residual error of the recovery |
| 717 | `DROPOUT_RATE` | Fraction [0,1] | Fraction of null subcarriers (emitted every 20 frames) |

#### Configuration

| Constant | Value | Purpose |
|----------|-------|---------|
| `NULL_THRESHOLD` | 0.001 | Amplitude below this = dropped out |
| `MIN_DROPOUT_RATE` | 0.10 | Minimum dropout fraction to trigger recovery |
| `MAX_ITERATIONS` | 10 | ISTA iteration cap per frame |
| `STEP_SIZE` | 0.05 | Gradient descent learning rate |
| `LAMBDA` | 0.01 | L1 sparsity penalty weight |
| `CORR_ALPHA` | 0.05 | EMA alpha for correlation model updates |

#### Tutorial: When Recovery Kicks In

1. The module needs at least 10 fully valid frames to initialize the correlation model (`is_initialized() == true`).
2. Recovery only triggers when dropout exceeds 10% (e.g., 4+ of 32 subcarriers are null).
3. Below 10%, the nulls are too sparse to warrant recovery overhead.
4. The tridiagonal correlation model exploits the fact that adjacent WiFi subcarriers are highly correlated. A null at subcarrier 15 can be estimated from subcarriers 14 and 16.
5. Monitor `RECOVERY_ERROR` -- a rising residual suggests the correlation model is stale and the environment has changed.

---

### Min-Cut Person Match (`sig_mincut_person_match.rs`)

**What it does**: Maintains stable identity labels for up to 4 people in the sensing area. When people move around, their WiFi signatures change position -- this module tracks which signature belongs to which person across consecutive frames.

**Algorithm**: Inspired by `ruvector-mincut` (DynamicPersonMatcher). Each frame:

1. **Feature extraction**: For each detected person, extract the top-8 subcarrier variances (sorted descending) from their spatial region. This produces an 8D signature vector.
2. **Cost matrix**: Compute L2 distances between all current features and all stored signatures.
3. **Greedy assignment**: Pick the minimum-cost (detection, slot) pair, mark both as used, repeat. Like a simplified Hungarian algorithm, optimal for max 4 persons.
4. **Signature update**: Blend new features into stored signatures via EMA (alpha=0.15).
5. **Timeout**: Release slots after 100 frames of absence.

#### Public API

```rust
pub struct PersonMatcher { /* ... */ }

impl PersonMatcher {
    pub const fn new() -> Self;
    pub fn process_frame(&mut self, amplitudes: &[f32], variances: &[f32], n_persons: usize) -> &[(i32, f32)];
    pub fn active_persons() -> u8;
    pub fn total_swaps() -> u32;
    pub fn is_person_stable(slot: usize) -> bool;
    pub fn person_signature(slot: usize) -> Option<&[f32; 8]>;
}
```

#### Events

| ID | Name | Value | Meaning |
|----|------|-------|---------|
| 720 | `PERSON_ID_ASSIGNED` | person_id + confidence*0.01 | Which slot was assigned (integer part) and match confidence (fractional part) |
| 721 | `PERSON_ID_SWAP` | prev*16 + curr | An identity swap was detected (prev and curr slot indices encoded) |
| 722 | `MATCH_CONFIDENCE` | [0.0, 1.0] | Average matching confidence across all detected persons (emitted every 10 frames) |

#### Configuration

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_PERSONS` | 4 | Maximum simultaneous person tracks |
| `FEAT_DIM` | 8 | Signature vector dimension |
| `SIG_ALPHA` | 0.15 | EMA blending factor for signature updates |
| `MAX_MATCH_DISTANCE` | 5.0 | L2 distance threshold for valid match |
| `STABLE_FRAMES` | 10 | Frames before a track is considered stable |
| `ABSENT_TIMEOUT` | 100 | Frames of absence before slot release (~5s at 20Hz) |

---

### Optimal Transport (`sig_optimal_transport.rs`)

**What it does**: Detects subtle motion that traditional variance-based detectors miss. Computes how much the overall shape of the WiFi signal distribution changes between frames, even when the total power stays constant.

**Algorithm**: Sliced Wasserstein distance -- a computationally efficient approximation to the full Wasserstein (earth mover's) distance.

1. Generate 4 fixed random projection directions (deterministic LCG PRNG, const-computed at compile time)
2. Project both current and previous amplitude vectors onto each direction
3. Sort the projected values (Shell sort with Ciura gaps, O(n^1.3))
4. Compute 1D Wasserstein-1 distance between sorted projections (just mean absolute difference)
5. Average across all 4 projections
6. Smooth via EMA and compare against thresholds

**Subtle motion detection**: When the Wasserstein distance is elevated (distribution shape changed) but the variance is stable (total power unchanged), something moved without creating obvious disturbance -- e.g., slow hand motion, breathing, or a door slowly closing.

#### Public API

```rust
pub struct OptimalTransportDetector { /* ... */ }

impl OptimalTransportDetector {
    pub const fn new() -> Self;
    pub fn process_frame(&mut self, amplitudes: &[f32]) -> &[(i32, f32)];
    pub fn distance() -> f32;            // EMA-smoothed Wasserstein distance
    pub fn variance_smoothed() -> f32;   // EMA-smoothed variance
    pub fn frame_count() -> u32;
}
```

#### Events

| ID | Name | Value | Meaning |
|----|------|-------|---------|
| 725 | `WASSERSTEIN_DISTANCE` | Distance | Smoothed sliced Wasserstein distance (emitted every 5 frames) |
| 726 | `DISTRIBUTION_SHIFT` | Distance | Large distribution change detected (debounced, 3 consecutive frames > 0.25) |
| 727 | `SUBTLE_MOTION` | Distance | Motion detected despite stable variance (5 consecutive frames with distance > 0.10 and variance change < 15%) |

#### Configuration

| Constant | Value | Purpose |
|----------|-------|---------|
| `N_PROJ` | 4 | Number of random projection directions |
| `ALPHA` | 0.15 | EMA alpha for distance smoothing |
| `VAR_ALPHA` | 0.1 | EMA alpha for variance smoothing |
| `WASS_SHIFT` | 0.25 | Wasserstein threshold for distribution shift event |
| `WASS_SUBTLE` | 0.10 | Wasserstein threshold for subtle motion |
| `VAR_STABLE` | 0.15 | Maximum relative variance change for "stable" classification |
| `SHIFT_DEB` | 3 | Debounce count for distribution shift |
| `SUBTLE_DEB` | 5 | Debounce count for subtle motion |

#### Tutorial: Interpreting Wasserstein Distance

The Wasserstein distance measures the "cost" of transforming one distribution into another. Unlike variance-based metrics that only measure spread, it captures changes in shape, location, and mode structure.

**Typical values:**
- 0.00-0.05: No motion. Static environment.
- 0.05-0.15: Breathing, subtle body sway, environmental drift.
- 0.15-0.30: Walking, arm movement, normal activity.
- 0.30+: Large motion, multiple people moving, or sudden environmental change.

**Why "subtle motion" matters**: A person sitting still and slowly raising their hand creates almost no change in total signal variance, but the Wasserstein distance increases because the spatial distribution of signal strength shifts. This is critical for:
- Fall detection (pre-fall sway)
- Gesture recognition (micro-movements)
- Intruder detection (someone trying to move stealthily)

---

## Performance Budget

| Module | Budget Tier | Typical Latency | Stack Memory | Key Bottleneck |
|--------|-------------|-----------------|--------------|----------------|
| Flash Attention | S (<5ms) | ~0.5ms | ~512 bytes | Softmax exp() over 8 groups |
| Coherence Gate | L (<2ms) | ~0.3ms | ~320 bytes | sin/cos per subcarrier |
| Temporal Compress | S (<5ms) | ~0.8ms | ~12 KB | 512 snapshots * 24 bytes |
| Sparse Recovery | H (<10ms) | ~3ms | ~768 bytes | 10 ISTA iterations * 32 subcarriers |
| Min-Cut Person Match | H (<10ms) | ~1.5ms | ~640 bytes | 4x4 cost matrix + feature extraction |
| Optimal Transport | S (<5ms) | ~1.5ms | ~1 KB | 8 Shell sorts (4 projections * 2 distributions) |

All latencies are estimated for ESP32-S3 running WASM3 interpreter at 240 MHz. Actual performance varies with subcarrier count and frame complexity.

## Memory Layout

All modules use fixed-size stack/static allocations. No heap, no `alloc`, no `Vec`. This is required for `no_std` WASM deployment on the ESP32-S3.

Total static memory for all 6 signal modules: approximately 15 KB, well within the ESP32-S3's available WASM linear memory.
