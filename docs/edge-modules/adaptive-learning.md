# Adaptive Learning Modules -- WiFi-DensePose Edge Intelligence

> On-device machine learning that runs without cloud connectivity. The ESP32 chip teaches itself what "normal" looks like for each environment and adapts over time. No training data needed -- it learns from what it sees.

## Overview

| Module | File | What It Does | Event IDs | Budget |
|--------|------|-------------|-----------|--------|
| DTW Gesture Learn | `lrn_dtw_gesture_learn.rs` | Teaches custom gestures via 3 rehearsals | 730-733 | H (<10ms) |
| Anomaly Attractor | `lrn_anomaly_attractor.rs` | Models room dynamics as a chaotic attractor | 735-738 | S (<5ms) |
| Meta Adapt | `lrn_meta_adapt.rs` | Self-tunes 8 detection thresholds via hill climbing | 740-743 | S (<5ms) |
| EWC Lifelong | `lrn_ewc_lifelong.rs` | Learns new environments without forgetting old ones | 745-748 | L (<2ms) |

## How the Learning Modules Work Together

```
  Raw CSI data (from signal intelligence pipeline)
       |
       v
  +-------------------------+     +--------------------------+
  | Anomaly Attractor        |     | DTW Gesture Learn        |
  | Learn what "normal"      |     | Users teach custom       |
  | looks like, detect       |     | gestures by performing   |
  | deviations from it       |     | them 3 times             |
  +-------------------------+     +--------------------------+
       |                                   |
       v                                   v
  +-------------------------+     +--------------------------+
  | EWC Lifelong             |     | Meta Adapt               |
  | Learn new rooms/layouts  |     | Auto-tune thresholds     |
  | without forgetting       |     | based on TP/FP feedback  |
  | old ones                 |     |                          |
  +-------------------------+     +--------------------------+
       |                                   |
       v                                   v
  Persistent on-device knowledge      Optimized detection parameters
  (survives power cycles via NVS)     (fewer false alarms over time)
```

- **Anomaly Attractor** learns the room's "normal" signal dynamics and alerts when something unexpected happens.
- **DTW Gesture Learn** lets users define custom gestures without any programming.
- **EWC Lifelong** ensures the device can move to a new room and learn it without losing knowledge of previous rooms.
- **Meta Adapt** continuously improves detection accuracy by tuning thresholds based on real-world feedback.

---

## Modules

### DTW Gesture Learning (`lrn_dtw_gesture_learn.rs`)

**What it does**: You teach the device custom gestures by performing them 3 times. It remembers up to 16 different gestures. When it recognizes a gesture you taught it, it fires an event with the gesture ID.

**Algorithm**: Dynamic Time Warping (DTW) with 3-rehearsal enrollment protocol.

DTW measures the similarity between two temporal sequences that may vary in speed. Unlike simple correlation, DTW can match a gesture performed slowly against one performed quickly. The Sakoe-Chiba band (width=8) constrains the warping path to prevent pathological matches.

#### Learning Protocol

```
  State Machine:

  Idle ──(60 frames stillness)──> WaitingStill
    ^                                 |
    |                            (motion detected)
    |                                 v
    |                             Recording ──(stillness)──> Captured
    |                                                           |
    |                                                    (save rehearsal)
    |                                                           |
    |                                          +----- < 3 rehearsals? ──> WaitingStill
    |                                          |
    |                                     >= 3 rehearsals
    |                                          |
    |                                   (check DTW similarity)
    |                                          |
    +-- (all 3 similar?) ──> commit template ──+
    +-- (too different?) ──> discard & reset ──+
```

#### Public API

```rust
pub struct GestureLearner { /* ... */ }

impl GestureLearner {
    pub const fn new() -> Self;
    pub fn process_frame(&mut self, phases: &[f32], motion_energy: f32) -> &[(i32, f32)];
    pub fn template_count() -> usize;    // Number of stored gesture templates (0-16)
}
```

#### Events

| ID | Name | Value | Meaning |
|----|------|-------|---------|
| 730 | `GESTURE_LEARNED` | Gesture ID (100+) | A new gesture template was successfully committed |
| 731 | `GESTURE_MATCHED` | Gesture ID | A stored gesture was recognized in the current signal |
| 732 | `MATCH_DISTANCE` | DTW distance | How closely the input matched the template (lower = better) |
| 733 | `TEMPLATE_COUNT` | Count (0-16) | Total number of stored templates |

#### Configuration

| Constant | Value | Purpose |
|----------|-------|---------|
| `TEMPLATE_LEN` | 64 | Maximum samples per gesture template |
| `MAX_TEMPLATES` | 16 | Maximum stored gestures |
| `REHEARSALS_REQUIRED` | 3 | Times you must perform a gesture to teach it |
| `STILLNESS_THRESHOLD` | 0.05 | Motion energy below this = stillness |
| `STILLNESS_FRAMES` | 60 | Frames of stillness to enter learning mode (~3s at 20Hz) |
| `LEARN_DTW_THRESHOLD` | 3.0 | Max DTW distance between rehearsals to accept as same gesture |
| `RECOGNIZE_DTW_THRESHOLD` | 2.5 | Max DTW distance for recognition match |
| `MATCH_COOLDOWN` | 40 | Frames between consecutive matches (~2s at 20Hz) |
| `BAND_WIDTH` | 8 | Sakoe-Chiba band width for DTW |

#### Tutorial: Teaching Your ESP32 a Custom Gesture

**Step 1: Enter training mode.**
Stand still for 3 seconds (60 frames at 20 Hz). The device detects sustained stillness and enters `WaitingStill` mode. There is no LED indicator in the base firmware, but you can add one by listening for the state transition.

**Step 2: Perform the gesture.**
Move your hand through the WiFi field. The device records the phase-delta trajectory. The recording captures up to 64 samples (3.2 seconds at 20 Hz). Keep the gesture under 3 seconds.

**Step 3: Return to stillness.**
Stop moving. The device captures the recording as "rehearsal 1 of 3."

**Step 4: Repeat 2 more times.**
The device stays in learning mode. Perform the same gesture two more times, returning to stillness after each.

**Step 5: Automatic validation.**
After the 3rd rehearsal, the device computes pairwise DTW distances between all 3 recordings. If all 3 are mutually similar (DTW distance < 3.0), it averages them into a template and assigns gesture ID 100 (the first custom gesture). Subsequent gestures get IDs 101, 102, etc.

**Step 6: Recognition.**
Once a template is stored, the device continuously matches the incoming phase-delta stream against all stored templates. When a match is found (DTW distance < 2.5), it emits `GESTURE_MATCHED` with the gesture ID and enters a 2-second cooldown to prevent double-firing.

**Tips for reliable gesture recognition:**
- Perform gestures in the same general area of the room
- Make gestures distinct (a wave is easier to distinguish from a circle than from a slower wave)
- Avoid ambient motion during training (other people walking, fans)
- Shorter gestures (0.5-1.5 seconds) tend to be more reliable than long ones

---

### Anomaly Attractor (`lrn_anomaly_attractor.rs`)

**What it does**: Models the room's WiFi signal as a dynamical system and classifies its behavior. An empty room produces a "point attractor" (stable signal). A room with HVAC produces a "limit cycle" (periodic). A room with people produces a "strange attractor" (complex but bounded). When the signal leaves the learned attractor basin, something unusual is happening.

**Algorithm**: 4D dynamical system analysis with Lyapunov exponent estimation.

The state vector is: `(mean_phase, mean_amplitude, variance, motion_energy)`

The Lyapunov exponent quantifies trajectory divergence:
```
lambda = (1/N) * sum(log(|delta_n+1| / |delta_n|))
```
- lambda < -0.01: **Point attractor** (stable, empty room)
- -0.01 <= lambda < 0.01: **Limit cycle** (periodic, machinery/HVAC)
- lambda >= 0.01: **Strange attractor** (chaotic, occupied room)

After 200 frames of learning (~10 seconds), the attractor type is classified and the basin radius is established. Subsequent departures beyond 3x the basin radius trigger anomaly alerts.

#### Public API

```rust
pub struct AttractorDetector { /* ... */ }

impl AttractorDetector {
    pub const fn new() -> Self;
    pub fn process_frame(&mut self, phases: &[f32], amplitudes: &[f32], motion_energy: f32)
        -> &[(i32, f32)];
    pub fn lyapunov_exponent() -> f32;
    pub fn attractor_type() -> AttractorType;    // Unknown/PointAttractor/LimitCycle/StrangeAttractor
    pub fn is_initialized() -> bool;             // True after 200 learning frames
}

pub enum AttractorType { Unknown, PointAttractor, LimitCycle, StrangeAttractor }
```

#### Events

| ID | Name | Value | Meaning |
|----|------|-------|---------|
| 735 | `ATTRACTOR_TYPE` | 1/2/3 | Point(1), LimitCycle(2), Strange(3) -- emitted when classification changes |
| 736 | `LYAPUNOV_EXPONENT` | Lambda | Current Lyapunov exponent estimate |
| 737 | `BASIN_DEPARTURE` | Distance ratio | Trajectory left the attractor basin (value = distance / radius) |
| 738 | `LEARNING_COMPLETE` | 1.0 | Initial 200-frame learning phase finished |

#### Configuration

| Constant | Value | Purpose |
|----------|-------|---------|
| `TRAJ_LEN` | 128 | Trajectory buffer length (circular) |
| `STATE_DIM` | 4 | State vector dimensionality |
| `MIN_FRAMES_FOR_CLASSIFICATION` | 200 | Learning phase length (~10s at 20Hz) |
| `LYAPUNOV_STABLE_UPPER` | -0.01 | Lambda below this = point attractor |
| `LYAPUNOV_PERIODIC_UPPER` | 0.01 | Lambda below this = limit cycle |
| `BASIN_DEPARTURE_MULT` | 3.0 | Departure threshold (3x learned radius) |
| `CENTER_ALPHA` | 0.01 | EMA alpha for attractor center tracking |
| `DEPARTURE_COOLDOWN` | 100 | Frames between departure alerts (~5s at 20Hz) |

#### Tutorial: Understanding Attractor Types

**Point Attractor (lambda < -0.01)**
The signal converges to a fixed point. This means the environment is completely static -- no people, no machinery, no airflow. The WiFi signal is deterministic and unchanging. Any disturbance will trigger a basin departure.

**Limit Cycle (lambda near 0)**
The signal follows a periodic orbit. This typically indicates mechanical systems: HVAC cycling, fans, elevator machinery. The period usually matches the equipment's duty cycle. Human activity on top of a limit cycle will push the Lyapunov exponent positive.

**Strange Attractor (lambda > 0.01)**
The signal is bounded but aperiodic -- classical chaos. This is the signature of human activity: walking, gesturing, breathing all create complex but bounded signal dynamics. The more people, the higher the Lyapunov exponent tends to be.

**Basin Departure**
A basin departure means the current signal state is more than 3x the learned radius away from the attractor center. This can indicate:
- Someone new entered the room
- A door or window opened
- Equipment turned on/off
- Environmental change (rain, temperature)

---

### Meta Adapt (`lrn_meta_adapt.rs`)

**What it does**: Automatically tunes 8 detection thresholds to reduce false alarms and improve detection accuracy. Uses real-world feedback (true positives and false positives) to drive a simple hill-climbing optimizer.

**Algorithm**: Iterative parameter perturbation with safety rollback.

The optimizer maintains 8 parameters, each with bounds and step sizes:

| Index | Parameter | Default | Range | Step |
|-------|-----------|---------|-------|------|
| 0 | Presence threshold | 0.05 | 0.01-0.50 | 0.01 |
| 1 | Motion threshold | 0.10 | 0.02-1.00 | 0.02 |
| 2 | Coherence threshold | 0.70 | 0.30-0.99 | 0.02 |
| 3 | Gesture DTW threshold | 2.50 | 0.50-5.00 | 0.20 |
| 4 | Anomaly energy ratio | 50.0 | 10.0-200.0 | 5.0 |
| 5 | Zone occupancy threshold | 0.02 | 0.005-0.10 | 0.005 |
| 6 | Vital apnea seconds | 20.0 | 10.0-60.0 | 2.0 |
| 7 | Intrusion sensitivity | 0.30 | 0.05-0.90 | 0.03 |

The optimization loop (runs on timer, not per-frame):
1. Measure baseline performance score: `score = TP_rate - 2 * FP_rate`
2. Perturb one parameter by its step size (alternating +/- direction)
3. Wait for `EVAL_WINDOW` (10) timer ticks
4. Measure new performance score
5. If improved, keep the change. If not, revert.
6. After 3 consecutive failures, safety rollback to the last known-good snapshot.
7. Sweep through all 8 parameters, then increment the meta-level counter.

The 2x penalty on false positives reflects the real-world cost: a false alarm (waking someone up at 3 AM because the system thought it detected motion) is worse than occasionally missing a true event.

#### Public API

```rust
pub struct MetaAdapter { /* ... */ }

impl MetaAdapter {
    pub const fn new() -> Self;
    pub fn report_true_positive(&mut self);   // Confirmed correct detection
    pub fn report_false_positive(&mut self);  // Detection that should not have fired
    pub fn report_event(&mut self);           // Generic event for normalization
    pub fn get_param(idx: usize) -> f32;      // Current value of parameter idx
    pub fn on_timer() -> &[(i32, f32)];       // Drive optimization loop (call at 1 Hz)
    pub fn iteration_count() -> u32;
    pub fn success_count() -> u32;
    pub fn meta_level() -> u16;               // Number of complete sweeps
    pub fn consecutive_failures() -> u8;
}
```

#### Events

| ID | Name | Value | Meaning |
|----|------|-------|---------|
| 740 | `PARAM_ADJUSTED` | param_idx + value/1000 | A parameter was successfully tuned |
| 741 | `ADAPTATION_SCORE` | Score [-2, 1] | Performance score after successful adaptation |
| 742 | `ROLLBACK_TRIGGERED` | Meta level | Safety rollback: 3 consecutive failures, reverting all params |
| 743 | `META_LEVEL` | Level | Number of complete optimization sweeps completed |

#### Configuration

| Constant | Value | Purpose |
|----------|-------|---------|
| `NUM_PARAMS` | 8 | Number of tunable parameters |
| `MAX_CONSECUTIVE_FAILURES` | 3 | Failures before safety rollback |
| `EVAL_WINDOW` | 10 | Timer ticks per evaluation phase |
| `DEFAULT_STEP_FRAC` | 0.05 | Step size as fraction of range |

#### Tutorial: Providing Feedback to Meta Adapt

The meta adapter needs feedback to know whether its changes helped. In a typical deployment:

1. **True positives**: When an event (presence detection, gesture match) is confirmed correct by another sensor or user acknowledgment, call `report_true_positive()`.
2. **False positives**: When an event fires but nothing actually happened (e.g., presence detected in an empty room), call `report_false_positive()`.
3. **Generic events**: Call `report_event()` for all events, regardless of correctness, to normalize the score.

In autonomous operation without human feedback, you can use cross-validation between modules: if both the coherence gate and the anomaly attractor agree that something happened, treat it as a true positive. If only one fires, it might be a false positive.

---

### EWC Lifelong (`lrn_ewc_lifelong.rs`)

**What it does**: Learns to classify which zone a person is in (up to 4 zones) using WiFi signal features. Critically, when moved to a new environment, it learns the new layout without forgetting previously learned ones. This is the "lifelong learning" property enabled by Elastic Weight Consolidation.

**Algorithm**: EWC (Kirkpatrick et al., 2017) on an 8-input, 4-output linear classifier.

The classifier has 32 learnable parameters (8 inputs x 4 outputs). Training uses gradient descent with an EWC penalty term:

```
L_total = L_current + (lambda/2) * sum_i(F_i * (theta_i - theta_i*)^2)
```

- `L_current` = MSE between predicted zone and one-hot target
- `F_i` = Fisher Information diagonal (how important each parameter is for previous tasks)
- `theta_i*` = parameter values at the end of the previous task
- `lambda` = 1000 (strong regularization to prevent forgetting)

Gradients are estimated via finite differences (perturb each parameter by epsilon=0.01, measure loss change). Only 4 parameters are updated per frame (round-robin) to stay within the 2ms budget.

#### Task Boundary Detection

A "task" corresponds to a stable environment (room layout). Task boundaries are detected automatically:
1. Track consecutive frames where loss < 0.1
2. After 100 consecutive stable frames, commit the task:
   - Snapshot parameters as `theta_star`
   - Update Fisher diagonal from accumulated gradient squares
   - Reset stability counter

Up to 32 tasks can be learned before the Fisher memory saturates.

#### Public API

```rust
pub struct EwcLifelong { /* ... */ }

impl EwcLifelong {
    pub const fn new() -> Self;
    pub fn process_frame(&mut self, features: &[f32], target_zone: i32) -> &[(i32, f32)];
    pub fn predict(features: &[f32]) -> u8;              // Inference only (zone 0-3)
    pub fn parameters() -> &[f32; 32];                   // Current model weights
    pub fn fisher_diagonal() -> &[f32; 32];              // Parameter importance
    pub fn task_count() -> u8;                            // Completed tasks
    pub fn last_loss() -> f32;                            // Last total loss
    pub fn last_penalty() -> f32;                         // Last EWC penalty
    pub fn frame_count() -> u32;
    pub fn has_prior_task() -> bool;
    pub fn reset(&mut self);
}
```

Note: `target_zone = -1` means inference only (no gradient update).

#### Events

| ID | Name | Value | Meaning |
|----|------|-------|---------|
| 745 | `KNOWLEDGE_RETAINED` | Penalty | EWC penalty magnitude (lower = less forgetting, emitted every 20 frames) |
| 746 | `NEW_TASK_LEARNED` | Task count | A new task was committed (environment successfully learned) |
| 747 | `FISHER_UPDATE` | Mean Fisher | Average Fisher information across all parameters |
| 748 | `FORGETTING_RISK` | Ratio | Ratio of EWC penalty to current loss (high = risk of forgetting) |

#### Configuration

| Constant | Value | Purpose |
|----------|-------|---------|
| `N_PARAMS` | 32 | Total learnable parameters (8x4) |
| `N_INPUT` | 8 | Input features (subcarrier group means) |
| `N_OUTPUT` | 4 | Output zones |
| `LAMBDA` | 1000.0 | EWC regularization strength |
| `EPSILON` | 0.01 | Finite-difference perturbation size |
| `PARAMS_PER_FRAME` | 4 | Round-robin gradient updates per frame |
| `LEARNING_RATE` | 0.001 | Gradient descent step size |
| `STABLE_FRAMES_THRESHOLD` | 100 | Consecutive stable frames to trigger task boundary |
| `STABLE_LOSS_THRESHOLD` | 0.1 | Loss below this = "stable" frame |
| `FISHER_ALPHA` | 0.01 | EMA alpha for Fisher diagonal updates |
| `MAX_TASKS` | 32 | Maximum tasks before Fisher saturates |

#### Tutorial: How Lifelong Learning Works on a Microcontroller

**The Problem**: Traditional neural networks suffer from "catastrophic forgetting." If you train a network on Room A and then train it on Room B, it forgets everything about Room A. This is a fundamental limitation, not a bug.

**The EWC Solution**: Before learning Room B, the system measures which parameters were important for Room A (via the Fisher Information diagonal). Then, while learning Room B, it adds a penalty that prevents important-for-Room-A parameters from changing too much. The result: the network learns Room B while retaining Room A knowledge.

**On the ESP32**: The classifier is intentionally tiny (32 parameters) to keep computation within 2ms per frame. Despite its simplicity, a linear classifier over 8 subcarrier group features can reliably distinguish 4 spatial zones. The Fisher diagonal only requires 32 floats (128 bytes) per task. With 32 tasks maximum, total Fisher memory is ~4 KB.

**Monitoring forgetting risk**: The `FORGETTING_RISK` event (ID 748) reports the ratio of EWC penalty to current loss. If this ratio exceeds 1.0, the EWC constraint is dominating the learning signal, meaning the system is struggling to learn the new task without forgetting old ones. This can happen when:
- The new environment is very different from all previous ones
- The 32-parameter model capacity is exhausted
- The Fisher diagonal has saturated from too many tasks

---

## How Learning Works on a Microcontroller

ESP32-S3 constraints that shape the design of all adaptive learning modules:

### No GPU
All computation is done on the CPU (Xtensa LX7 dual-core at 240 MHz) via the WASM3 interpreter. This means:
- No matrix multiplication hardware
- No parallel SIMD operations
- Every floating-point operation counts

### Fixed Memory
WASM3 allocates a fixed linear memory region. There is no heap, no `malloc`, no dynamic allocation:
- All arrays are fixed-size and stack-allocated
- Maximum data structure sizes are compile-time constants
- Buffer overflows are impossible (Rust's bounds checking + fixed arrays)

### EWC for Preventing Forgetting
Without EWC, moving the device to a new room would erase everything learned about the previous room. EWC adds ~32 floats of overhead per task (the Fisher diagonal snapshot), which is negligible on the ESP32.

### Round-Robin Gradient Estimation
Computing gradients for all 32 parameters every frame would take too long. Instead, the EWC module uses round-robin scheduling: 4 parameters per frame, cycling through all 32 in 8 frames. At 20 Hz, a full gradient pass takes 0.4 seconds -- fast enough for the slow dynamics of room occupancy.

### Task Boundary Detection
The system automatically detects when it has "converged" on a new environment (100 consecutive stable frames = 5 seconds of consistent low loss). No manual intervention needed. The user just places the device in a new room, and the learning happens automatically.

### Energy Budget

| Module | Budget | Per-Frame Operations | Memory |
|--------|--------|---------------------|--------|
| DTW Gesture Learn | H (<10ms) | DTW: 64x64=4096 mults per template, up to 16 templates | ~18 KB (templates + rehearsals) |
| Anomaly Attractor | S (<5ms) | 4D distance + log for Lyapunov + EMA | ~2.5 KB (128 trajectory points) |
| Meta Adapt | S (<5ms) | Score computation + perturbation (timer only, not per-frame) | ~256 bytes |
| EWC Lifelong | L (<2ms) | 4 finite-difference evals + gradient step | ~512 bytes (params + Fisher + theta_star) |

Total static memory for all 4 learning modules: approximately 21 KB.
