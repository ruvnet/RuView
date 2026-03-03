# ADR-041: WASM Module Collection -- Curated Sensing Algorithm Registry

**Status**: Accepted (Phase 1 implemented, hardware-validated on RuView ESP32-S3)
**Date**: 2026-03-02
**Deciders**: @ruvnet
**Supersedes**: None
**Related**: ADR-039 (Edge Intelligence), ADR-040 (WASM Programmable Sensing)

## Context

ADR-040 established the Tier 3 WASM programmable sensing runtime: a WASM3
interpreter on ESP32-S3 that executes hot-loadable Rust-to-wasm32 modules with
a 12-function Host API, RVF container format, Ed25519 signing, and adaptive
budget control. Three flagship modules were defined (gesture, coherence,
adversarial) as proof of capability.

A runtime without a library of modules is an empty platform. The difference
between a product and a platform is the ecosystem -- and the ecosystem is the
module collection. Three strategic dynamics make a curated collection essential:

**1. Platform flywheel effect.** Each new module increases the value of every
deployed ESP32 node. A node purchased for sleep apnea monitoring becomes a
fall detector, an intrusion sensor, and an occupancy counter -- all via OTA
WASM uploads. This multiplies the addressable market without multiplying
hardware SKUs.

**2. Community velocity.** WiFi CSI sensing is a research-active field with
hundreds of labs publishing new algorithms annually. A well-defined module
contract (Host API v1, RVF container, event type registry) lowers the barrier
from "fork the firmware and cross-compile" to "write 50 lines of no_std Rust,
compile to wasm32, submit a PR." The module collection is the contribution
surface.

**3. Vertical market expansion.** The root README lists 12+ deployment
scenarios spanning healthcare, retail, industrial safety, smart buildings,
disaster response, and fitness. Each vertical requires domain-specific
algorithms that share the same underlying CSI primitives. A module collection
allows vertical specialists to build on a common sensing substrate without
understanding RF engineering.

This ADR defines a curated collection of 37 modules across 6 categories,
with event type registries, budget tiers, implementation priorities, and a
community contribution workflow.

## Decision

### Module Collection Overview

37 modules organized into 6 categories. Every module targets Host API v1
(ADR-040), ships as an RVF container, and declares its event type IDs,
budget tier, and capability bitmask.

### Budget Tiers

| Tier | Label | Per-frame budget | Use case |
|------|-------|------------------|----------|
| L | Lightweight | < 2,000 us (2 ms) | Simple threshold checks, single-value outputs |
| S | Standard | < 5,000 us (5 ms) | Moderate DSP, windowed statistics |
| H | Heavy | < 10,000 us (10 ms) | Complex pattern matching, multi-signal fusion |

When multiple modules run concurrently, the adaptive budget controller
(ADR-040 Appendix B) divides the total frame budget B across active modules.
Heavy modules should generally run alone or paired only with lightweight ones.

### Naming Convention

All modules follow the pattern `wdp-{category}-{name}`:

| Category | Prefix | Event ID range |
|----------|--------|----------------|
| Medical & Health | `wdp-med-` | 100--199 |
| Security & Safety | `wdp-sec-` | 200--299 |
| Smart Building | `wdp-bld-` | 300--399 |
| Retail & Hospitality | `wdp-ret-` | 400--499 |
| Industrial & Specialized | `wdp-ind-` | 500--599 |
| Exotic & Research | `wdp-exo-` | 600--699 |

Event type IDs 0--99 are reserved for the three ADR-040 flagship modules and
future core system events.

---

## Category 1: Medical & Health (Event IDs 100--199)

### 1.1 `wdp-med-sleep-apnea`

**Description**: Detects obstructive and central sleep apnea episodes by
monitoring breathing rate cessation. When the breathing BPM drops below
4 BPM and remains there for more than 10 consecutive seconds, the module
emits an apnea alert with duration. It also tracks apnea-hypopnea index
(AHI) over a sleep session by counting events per hour.

**Host API dependencies**: `csi_get_bpm_breathing`, `csi_get_presence`,
`csi_get_variance`, `csi_get_timestamp`, `csi_emit_event`, `csi_log`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 100 | `APNEA_START` | Duration threshold (seconds) |
| 101 | `APNEA_END` | Episode duration (seconds) |
| 102 | `AHI_UPDATE` | Events per hour (float) |

**Estimated .wasm size**: 4 KB
**Budget tier**: L (lightweight, < 2 ms) -- primarily threshold checks on Tier 2 vitals
**Difficulty**: Easy

---

### 1.2 `wdp-med-cardiac-arrhythmia`

**Description**: Detects irregular heartbeat patterns from heart rate
variability (HRV) extracted from the CSI phase signal. Monitors for
tachycardia (>100 BPM sustained), bradycardia (<50 BPM sustained), and
missed-beat patterns where the inter-beat interval suddenly doubles. Uses
a sliding window of 30 seconds of heart rate samples to compute RMSSD
(root mean square of successive differences) and flags anomalies when
RMSSD exceeds 3 standard deviations from baseline.

**Host API dependencies**: `csi_get_bpm_heartrate`, `csi_get_phase`,
`csi_get_phase_history`, `csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 110 | `TACHYCARDIA` | Current BPM |
| 111 | `BRADYCARDIA` | Current BPM |
| 112 | `MISSED_BEAT` | Gap duration (ms) |
| 113 | `HRV_ANOMALY` | RMSSD value |

**Estimated .wasm size**: 8 KB
**Budget tier**: S (standard, < 5 ms) -- requires phase history windowing
**Difficulty**: Hard

---

### 1.3 `wdp-med-respiratory-distress`

**Description**: Detects respiratory distress patterns including tachypnea
(rapid shallow breathing > 25 BPM), labored breathing (high amplitude
variance in the breathing band), and Cheyne-Stokes respiration (cyclical
crescendo-decrescendo breathing pattern with apneic pauses). Cheyne-Stokes
detection uses autocorrelation of the breathing amplitude envelope over a
60-second window to find the characteristic 30--90 second periodicity.

**Host API dependencies**: `csi_get_bpm_breathing`, `csi_get_phase`,
`csi_get_variance`, `csi_get_phase_history`, `csi_get_timestamp`,
`csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 120 | `TACHYPNEA` | Current breathing BPM |
| 121 | `LABORED_BREATHING` | Amplitude variance ratio |
| 122 | `CHEYNE_STOKES` | Cycle period (seconds) |
| 123 | `RESP_DISTRESS_LEVEL` | Severity 0.0--1.0 |

**Estimated .wasm size**: 10 KB
**Budget tier**: H (heavy, < 10 ms) -- autocorrelation over 60 s window
**Difficulty**: Hard

---

### 1.4 `wdp-med-gait-analysis`

**Description**: Analyzes walking patterns from CSI motion signatures to
detect Parkinsonian gait (shuffling, reduced arm swing, festination),
post-stroke asymmetric gait, and elevated fall risk. Extracts step
cadence, step regularity, stride-to-stride variability, and bilateral
asymmetry from phase variance periodicity. Outputs a composite fall-risk
score (0--100) based on gait instability metrics published in clinical
biomechanics literature.

**Host API dependencies**: `csi_get_phase`, `csi_get_amplitude`,
`csi_get_variance`, `csi_get_motion_energy`, `csi_get_phase_history`,
`csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 130 | `STEP_CADENCE` | Steps per minute |
| 131 | `GAIT_ASYMMETRY` | Asymmetry index 0.0--1.0 |
| 132 | `FALL_RISK_SCORE` | Risk score 0--100 |
| 133 | `SHUFFLING_DETECTED` | Confidence 0.0--1.0 |
| 134 | `FESTINATION` | Acceleration pattern flag |

**Estimated .wasm size**: 12 KB
**Budget tier**: H (heavy, < 10 ms) -- windowed periodicity analysis
**Difficulty**: Hard

---

### 1.5 `wdp-med-seizure-detect`

**Description**: Detects tonic-clonic (grand mal) epileptic seizures via
sudden onset of high-energy rhythmic motion in the 3--8 Hz band, distinct
from normal voluntary movement. The tonic phase produces a sustained
high-amplitude CSI disturbance; the clonic phase shows characteristic
rhythmic oscillation at 3--5 Hz with decreasing frequency. Discriminates
from falls (single impulse) and tremor (lower amplitude, continuous).
Emits a graded alert: pre-ictal warning (motion pattern change), seizure
onset, and post-ictal stillness.

**Host API dependencies**: `csi_get_phase`, `csi_get_amplitude`,
`csi_get_motion_energy`, `csi_get_phase_history`, `csi_get_presence`,
`csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 140 | `SEIZURE_ONSET` | Confidence 0.0--1.0 |
| 141 | `SEIZURE_TONIC` | Duration (seconds) |
| 142 | `SEIZURE_CLONIC` | Oscillation frequency (Hz) |
| 143 | `POST_ICTAL` | Stillness duration (seconds) |

**Estimated .wasm size**: 10 KB
**Budget tier**: S (standard, < 5 ms) -- frequency analysis on motion energy
**Difficulty**: Hard

---

### 1.6 `wdp-med-vital-trend`

**Description**: Long-term trending of breathing rate and heart rate over
hours and days. Maintains exponentially weighted moving averages (EWMA)
with multiple time constants (5 min, 1 hr, 4 hr) and detects gradual
deterioration. Emits a NEWS2-inspired early warning score when vitals
deviate from the patient's personal baseline by clinically significant
margins. Designed for sepsis early warning, post-surgical monitoring,
and chronic disease management. Stores trend state in module memory
across on_timer calls.

**Host API dependencies**: `csi_get_bpm_breathing`, `csi_get_bpm_heartrate`,
`csi_get_presence`, `csi_get_timestamp`, `csi_emit_event`, `csi_log`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 150 | `TREND_BREATHING_DELTA` | % deviation from 4-hr baseline |
| 151 | `TREND_HEARTRATE_DELTA` | % deviation from 4-hr baseline |
| 152 | `EARLY_WARNING_SCORE` | NEWS2-style score 0--20 |
| 153 | `BASELINE_ESTABLISHED` | Hours of data collected |

**Estimated .wasm size**: 6 KB
**Budget tier**: L (lightweight, < 2 ms) -- EWMA updates are O(1)
**Difficulty**: Medium

---

## Category 2: Security & Safety (Event IDs 200--299)

### 2.1 `wdp-sec-intrusion-detect`

**Description**: Detects unauthorized human entry into a secured zone
during armed periods. Distinguishes human movement signatures (bipedal
gait, 0.5--2.0 Hz periodicity in phase variance) from false alarm
sources: HVAC airflow (broadband low-frequency), pets (lower amplitude,
quadrupedal cadence), and environmental drift (monotonic phase change).
Uses a two-stage classifier: a fast energy gate followed by a cadence
discriminator on the top-K subcarriers.

**Host API dependencies**: `csi_get_phase`, `csi_get_variance`,
`csi_get_amplitude`, `csi_get_motion_energy`, `csi_get_presence`,
`csi_get_n_persons`, `csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 200 | `INTRUSION_ALERT` | Confidence 0.0--1.0 |
| 201 | `HUMAN_CONFIRMED` | Number of persons detected |
| 202 | `FALSE_ALARM_SOURCE` | Source type (1=HVAC, 2=pet, 3=env) |

**Estimated .wasm size**: 8 KB
**Budget tier**: S (standard, < 5 ms)
**Difficulty**: Medium

---

### 2.2 `wdp-sec-perimeter-breach`

**Description**: Multi-zone perimeter monitoring using phase gradient
analysis across subcarrier groups. Determines direction of movement
(approach vs departure) from the temporal ordering of phase disturbances
across spatially diverse subcarriers. Divides the monitored space into
configurable zones (up to 4) and tracks the progression of a moving
target across zone boundaries. Emits zone-transition events with
directional vectors.

**Host API dependencies**: `csi_get_phase`, `csi_get_amplitude`,
`csi_get_variance`, `csi_get_phase_history`, `csi_get_motion_energy`,
`csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 210 | `PERIMETER_BREACH` | Zone ID (0--3) |
| 211 | `APPROACH_DETECTED` | Approach velocity proxy (0.0--1.0) |
| 212 | `DEPARTURE_DETECTED` | Departure velocity proxy (0.0--1.0) |
| 213 | `ZONE_TRANSITION` | Encoded (from_zone << 4 | to_zone) |

**Estimated .wasm size**: 10 KB
**Budget tier**: S (standard, < 5 ms)
**Difficulty**: Medium

---

### 2.3 `wdp-sec-weapon-detect`

**Description**: Research-grade module for detecting concealed metallic
objects (knives, firearms) based on differential CSI multipath signatures.
Metallic objects have significantly higher RF reflectivity than biological
tissue, creating distinctive amplitude spikes on specific subcarrier
groups when a person carrying metal passes through the sensing field.
The module computes a metal-presence index from the ratio of amplitude
variance to phase variance -- pure tissue produces coupled amplitude/phase
changes, while metallic reflectors produce disproportionate amplitude
perturbation. **Experimental: requires controlled environment calibration
and should not be used as a sole security measure.**

**Host API dependencies**: `csi_get_phase`, `csi_get_amplitude`,
`csi_get_variance`, `csi_get_motion_energy`, `csi_get_presence`,
`csi_get_timestamp`, `csi_emit_event`, `csi_log`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 220 | `METAL_ANOMALY` | Metal-presence index 0.0--1.0 |
| 221 | `WEAPON_ALERT` | Confidence 0.0--1.0 (threshold: 0.7) |
| 222 | `CALIBRATION_NEEDED` | Drift metric |

**Estimated .wasm size**: 8 KB
**Budget tier**: S (standard, < 5 ms)
**Difficulty**: Hard

---

### 2.4 `wdp-sec-tailgating`

**Description**: Detects tailgating (piggybacking) at access-controlled
doorways by identifying two or more people passing through a chokepoint
in rapid succession. Uses temporal clustering of motion energy peaks:
a single person produces one motion envelope; two people in quick
succession produce a double-peaked or prolonged envelope. The inter-peak
interval threshold is configurable (default: 3 seconds). Also detects
side-by-side passage from broadened phase disturbance patterns.

**Host API dependencies**: `csi_get_motion_energy`, `csi_get_presence`,
`csi_get_n_persons`, `csi_get_variance`, `csi_get_timestamp`,
`csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 230 | `TAILGATE_DETECTED` | Person count estimate |
| 231 | `SINGLE_PASSAGE` | Passage duration (ms) |
| 232 | `MULTI_PASSAGE` | Inter-person gap (ms) |

**Estimated .wasm size**: 6 KB
**Budget tier**: L (lightweight, < 2 ms)
**Difficulty**: Medium

---

### 2.5 `wdp-sec-loitering`

**Description**: Detects prolonged stationary presence in a designated
zone beyond a configurable dwell threshold (default: 5 minutes). Uses
sustained presence detection (Tier 2 presence flag) combined with low
motion energy to distinguish loitering from active use of a space. Tracks
dwell duration with a state machine: absent, entering, present, loitering.
The loitering-to-absent transition requires sustained absence for a
configurable cooldown (default: 30 seconds) to avoid flapping.

**Host API dependencies**: `csi_get_presence`, `csi_get_motion_energy`,
`csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 240 | `LOITERING_START` | Dwell threshold exceeded (minutes) |
| 241 | `LOITERING_ONGOING` | Current dwell duration (minutes) |
| 242 | `LOITERING_END` | Total dwell duration (minutes) |

**Estimated .wasm size**: 3 KB
**Budget tier**: L (lightweight, < 2 ms)
**Difficulty**: Easy

---

### 2.6 `wdp-sec-panic-motion`

**Description**: Detects erratic, high-energy movement patterns consistent
with distress, struggle, or fleeing. Computes jerk (rate of change of
motion energy) and motion entropy (randomness of phase variance across
subcarriers). Normal walking produces low jerk and low entropy; panicked
motion produces high jerk with high entropy (unpredictable direction
changes). The module maintains a 5-second sliding window and triggers
when both jerk and entropy exceed their respective thresholds simultaneously.

**Host API dependencies**: `csi_get_motion_energy`, `csi_get_variance`,
`csi_get_phase`, `csi_get_presence`, `csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 250 | `PANIC_DETECTED` | Severity 0.0--1.0 |
| 251 | `STRUGGLE_PATTERN` | Jerk magnitude |
| 252 | `FLEEING_DETECTED` | Motion energy peak |

**Estimated .wasm size**: 6 KB
**Budget tier**: S (standard, < 5 ms)
**Difficulty**: Medium

---

## Category 3: Smart Building (Event IDs 300--399)

### 3.1 `wdp-bld-occupancy-zones`

**Description**: Divides the monitored room into a configurable grid of
zones (default: 2x2 = 4 zones) and estimates per-zone occupancy from
subcarrier group variance patterns. Each subcarrier group maps to a
spatial zone based on initial calibration. The module outputs a zone
occupancy vector on each frame where changes occur, enabling
spatial heatmaps, desk-level presence detection, and room utilization
analytics.

**Host API dependencies**: `csi_get_variance`, `csi_get_amplitude`,
`csi_get_presence`, `csi_get_n_persons`, `csi_get_timestamp`,
`csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 300 | `ZONE_OCCUPIED` | Zone ID (0--15) |
| 301 | `ZONE_VACANT` | Zone ID (0--15) |
| 302 | `TOTAL_OCCUPANCY` | Total person count |
| 303 | `ZONE_MAP_UPDATE` | Encoded zone bitmap (u16) |

**Estimated .wasm size**: 8 KB
**Budget tier**: S (standard, < 5 ms)
**Difficulty**: Medium

---

### 3.2 `wdp-bld-hvac-presence`

**Description**: Optimized for HVAC control integration with appropriate
hysteresis to prevent rapid cycling of heating/cooling equipment. Reports
presence with a configurable arrival debounce (default: 10 seconds) and
a departure timeout (default: 5 minutes). The departure timeout ensures
HVAC does not shut down during brief absences (bathroom break, coffee
run). Also reports an activity level (sedentary/active) for adaptive
comfort control -- sedentary occupants may prefer different temperature
setpoints.

**Host API dependencies**: `csi_get_presence`, `csi_get_motion_energy`,
`csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 310 | `HVAC_OCCUPIED` | 1 = occupied, 0 = vacant (with hysteresis) |
| 311 | `ACTIVITY_LEVEL` | 0 = sedentary, 1 = active |
| 312 | `DEPARTURE_COUNTDOWN` | Seconds remaining until vacancy declared |

**Estimated .wasm size**: 3 KB
**Budget tier**: L (lightweight, < 2 ms)
**Difficulty**: Easy

---

### 3.3 `wdp-bld-lighting-zones`

**Description**: Presence-triggered zone lighting control with occupancy-
aware dimming. Maps to the same zone grid as `occupancy-zones`. Outputs
lighting commands per zone: ON (occupied, active), DIM (occupied,
sedentary for > 10 min), and OFF (vacant for > departure timeout). The
dimming ramp is gradual (configurable 30-second fade) to avoid jarring
transitions. Integrates with standard building automation protocols
via the event stream.

**Host API dependencies**: `csi_get_presence`, `csi_get_motion_energy`,
`csi_get_variance`, `csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 320 | `LIGHT_ON` | Zone ID |
| 321 | `LIGHT_DIM` | Zone ID (dimming level as value 0.0--1.0) |
| 322 | `LIGHT_OFF` | Zone ID |

**Estimated .wasm size**: 4 KB
**Budget tier**: L (lightweight, < 2 ms)
**Difficulty**: Easy

---

### 3.4 `wdp-bld-elevator-count`

**Description**: Counts occupants in an elevator cab using confined-space
CSI multipath analysis. In an elevator, the metal walls create a highly
reflective RF cavity where each person creates a measurable perturbation
in the standing wave pattern. The module uses amplitude variance
decomposition to estimate person count (1--12) and detects door-open
events from sudden multipath geometry changes. Supports weight-limit
proxying: emits overload warning when count exceeds configurable threshold.

**Host API dependencies**: `csi_get_amplitude`, `csi_get_variance`,
`csi_get_phase`, `csi_get_motion_energy`, `csi_get_n_persons`,
`csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 330 | `ELEVATOR_COUNT` | Person count (0--12) |
| 331 | `DOOR_OPEN` | 1.0 |
| 332 | `DOOR_CLOSE` | 1.0 |
| 333 | `OVERLOAD_WARNING` | Count above threshold |

**Estimated .wasm size**: 8 KB
**Budget tier**: S (standard, < 5 ms)
**Difficulty**: Medium

---

### 3.5 `wdp-bld-meeting-room`

**Description**: Meeting room utilization tracking. Detects room state
transitions (empty, pre-meeting gathering, active meeting, post-meeting
departure) from occupancy patterns. Tracks meeting start time, end time,
peak headcount, and actual vs booked utilization. Emits "room available"
events for opportunistic booking systems. Distinguishes genuine meetings
(sustained multi-person presence > 5 minutes) from transient occupancy
(someone ducking in to grab a laptop).

**Host API dependencies**: `csi_get_presence`, `csi_get_n_persons`,
`csi_get_motion_energy`, `csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 340 | `MEETING_START` | Headcount at start |
| 341 | `MEETING_END` | Duration (minutes) |
| 342 | `PEAK_HEADCOUNT` | Maximum persons detected |
| 343 | `ROOM_AVAILABLE` | 1.0 (available for booking) |

**Estimated .wasm size**: 5 KB
**Budget tier**: L (lightweight, < 2 ms)
**Difficulty**: Easy

---

### 3.6 `wdp-bld-energy-audit`

**Description**: Correlates occupancy patterns with time-of-day and day-
of-week to build occupancy schedules for building energy optimization.
Maintains hourly occupancy histograms (24 bins per day, 7 days) in module
memory and emits daily schedule summaries via on_timer. Identifies
consistently unoccupied periods where HVAC and lighting can be scheduled
off. Also detects after-hours occupancy anomalies (someone working late
on a normally vacant floor).

**Host API dependencies**: `csi_get_presence`, `csi_get_n_persons`,
`csi_get_timestamp`, `csi_emit_event`, `csi_log`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 350 | `SCHEDULE_SUMMARY` | Encoded daily pattern (packed bits) |
| 351 | `AFTER_HOURS_ALERT` | Hour of detection (0--23) |
| 352 | `UTILIZATION_RATE` | % of working hours occupied |

**Estimated .wasm size**: 6 KB
**Budget tier**: L (lightweight, < 2 ms)
**Difficulty**: Medium

---

## Category 4: Retail & Hospitality (Event IDs 400--499)

### 4.1 `wdp-ret-queue-length`

**Description**: Estimates queue length and wait time from sequential
presence detection along a linear zone. Models the queue as an ordered
sequence of occupied positions. Tracks join rate (new arrivals per minute),
service rate (departures from the head), and estimates current wait time
using Little's Law (L = lambda * W). Emits queue length at every change and
wait-time estimates at configurable intervals. Designed for checkout lines,
customer service counters, and bank branches.

**Host API dependencies**: `csi_get_presence`, `csi_get_n_persons`,
`csi_get_variance`, `csi_get_motion_energy`, `csi_get_timestamp`,
`csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 400 | `QUEUE_LENGTH` | Estimated person count in queue |
| 401 | `WAIT_TIME_ESTIMATE` | Estimated wait (seconds) |
| 402 | `SERVICE_RATE` | Persons served per minute |
| 403 | `QUEUE_ALERT` | Length exceeds threshold |

**Estimated .wasm size**: 6 KB
**Budget tier**: L (lightweight, < 2 ms)
**Difficulty**: Medium

---

### 4.2 `wdp-ret-dwell-heatmap`

**Description**: Tracks dwell time per spatial zone and generates a dwell-
time heatmap for spatial engagement analysis. Divides the sensing area
into a configurable grid (default 3x3) and accumulates dwell-seconds per
zone. Emits per-zone dwell updates at configurable intervals (default:
30 seconds) and session summaries when the space empties. Designed for
retail floor optimization, museum exhibit engagement, and trade show
booth analytics.

**Host API dependencies**: `csi_get_presence`, `csi_get_variance`,
`csi_get_motion_energy`, `csi_get_n_persons`, `csi_get_timestamp`,
`csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 410 | `DWELL_ZONE_UPDATE` | Zone ID (high byte) + seconds (value) |
| 411 | `HOT_ZONE` | Zone ID with highest dwell |
| 412 | `COLD_ZONE` | Zone ID with lowest dwell |
| 413 | `SESSION_SUMMARY` | Total dwell-seconds across all zones |

**Estimated .wasm size**: 6 KB
**Budget tier**: L (lightweight, < 2 ms)
**Difficulty**: Medium

---

### 4.3 `wdp-ret-customer-flow`

**Description**: Directional foot traffic counting at entry/exit points
and between departments. Uses asymmetric phase gradient analysis to
determine movement direction. Maintains running counts of ingress and
egress events and computes net occupancy (in - out). Handles simultaneous
bidirectional traffic by decomposing the CSI disturbance into directional
components. Emits count deltas and periodic summaries.

**Host API dependencies**: `csi_get_phase`, `csi_get_amplitude`,
`csi_get_variance`, `csi_get_phase_history`, `csi_get_motion_energy`,
`csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 420 | `INGRESS` | Count (+1 per entry) |
| 421 | `EGRESS` | Count (+1 per exit) |
| 422 | `NET_OCCUPANCY` | Current in-out difference |
| 423 | `HOURLY_TRAFFIC` | Total passages in last hour |

**Estimated .wasm size**: 8 KB
**Budget tier**: S (standard, < 5 ms) -- phase gradient computation
**Difficulty**: Medium

---

### 4.4 `wdp-ret-table-turnover`

**Description**: Restaurant table occupancy and turnover tracking. Detects
table-level presence states: empty, seated (low motion, sustained
presence), eating (moderate motion), and departing (rising motion followed
by absence). Tracks seating duration and emits turnover events for
waitlist management. Designed for a single-table sensing zone per node.

**Host API dependencies**: `csi_get_presence`, `csi_get_motion_energy`,
`csi_get_n_persons`, `csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 430 | `TABLE_SEATED` | Person count |
| 431 | `TABLE_VACATED` | Seating duration (minutes) |
| 432 | `TABLE_AVAILABLE` | 1.0 (ready for next party) |
| 433 | `TURNOVER_RATE` | Tables per hour (on_timer) |

**Estimated .wasm size**: 4 KB
**Budget tier**: L (lightweight, < 2 ms)
**Difficulty**: Easy

---

### 4.5 `wdp-ret-shelf-engagement`

**Description**: Detects customer stopping near and interacting with
retail shelving. A "shelf engagement" event fires when a person's
presence is detected with low translational motion (not walking past)
combined with localized high-frequency phase perturbation (reaching,
picking up, examining products). Distinguishes browse (short stop,
< 5 seconds), consider (5--30 seconds), and deep engagement (> 30
seconds). Provides product-interaction proxying without cameras.

**Host API dependencies**: `csi_get_presence`, `csi_get_motion_energy`,
`csi_get_variance`, `csi_get_phase`, `csi_get_timestamp`,
`csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 440 | `SHELF_BROWSE` | Dwell seconds |
| 441 | `SHELF_CONSIDER` | Dwell seconds |
| 442 | `SHELF_ENGAGE` | Dwell seconds |
| 443 | `REACH_DETECTED` | Confidence 0.0--1.0 |

**Estimated .wasm size**: 6 KB
**Budget tier**: S (standard, < 5 ms)
**Difficulty**: Medium

---

## Category 5: Industrial & Specialized (Event IDs 500--599)

### 5.1 `wdp-ind-forklift-proximity`

**Description**: Detects dangerous proximity between pedestrian workers
and forklifts/AGVs in warehouse and factory environments. Forklifts
produce a distinctive CSI signature: high-amplitude, low-frequency
(< 0.3 Hz) phase modulation from the large metal body moving slowly,
combined with engine/motor vibration harmonics. When this signature
co-occurs with a human motion signature, a proximity alert fires.
Priority: CRITICAL -- this is a life-safety module.

**Host API dependencies**: `csi_get_phase`, `csi_get_amplitude`,
`csi_get_variance`, `csi_get_motion_energy`, `csi_get_presence`,
`csi_get_n_persons`, `csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 500 | `PROXIMITY_WARNING` | Estimated distance category (0=critical, 1=warning, 2=caution) |
| 501 | `VEHICLE_DETECTED` | Confidence 0.0--1.0 |
| 502 | `HUMAN_NEAR_VEHICLE` | 1.0 (co-presence confirmed) |

**Estimated .wasm size**: 10 KB
**Budget tier**: S (standard, < 5 ms)
**Difficulty**: Hard

---

### 5.2 `wdp-ind-confined-space`

**Description**: Monitors worker presence and vital signs in confined
spaces (tanks, silos, manholes, crawl spaces) where WiFi CSI excels
due to strong multipath in enclosed metal environments. Tracks entry/exit
events, continuous breathing confirmation (proof of life), and triggers
emergency extraction alerts if breathing ceases for > 15 seconds or
if all motion stops for > 60 seconds. Designed to satisfy OSHA confined
space monitoring requirements (29 CFR 1910.146).

**Host API dependencies**: `csi_get_presence`, `csi_get_bpm_breathing`,
`csi_get_motion_energy`, `csi_get_variance`, `csi_get_timestamp`,
`csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 510 | `WORKER_ENTRY` | 1.0 |
| 511 | `WORKER_EXIT` | Duration inside (seconds) |
| 512 | `BREATHING_OK` | Breathing BPM (periodic heartbeat event) |
| 513 | `EXTRACTION_ALERT` | Seconds since last breathing detected |
| 514 | `IMMOBILE_ALERT` | Seconds of zero motion |

**Estimated .wasm size**: 5 KB
**Budget tier**: L (lightweight, < 2 ms)
**Difficulty**: Medium

---

### 5.3 `wdp-ind-clean-room`

**Description**: Personnel count and movement tracking for cleanroom
contamination control (ISO 14644). Cleanrooms require strict occupancy
limits and controlled movement patterns. The module enforces maximum
occupancy (configurable, default: 4 persons), detects rapid/turbulent
movement that could disturb laminar airflow, and logs personnel dwell
time for compliance reporting. Emits violations when occupancy exceeds
the limit or movement energy exceeds the turbulence threshold.

**Host API dependencies**: `csi_get_n_persons`, `csi_get_presence`,
`csi_get_motion_energy`, `csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 520 | `OCCUPANCY_COUNT` | Current person count |
| 521 | `OCCUPANCY_VIOLATION` | Count above maximum |
| 522 | `TURBULENT_MOTION` | Motion energy above threshold |
| 523 | `COMPLIANCE_REPORT` | Encoded summary (on_timer) |

**Estimated .wasm size**: 4 KB
**Budget tier**: L (lightweight, < 2 ms)
**Difficulty**: Easy

---

### 5.4 `wdp-ind-livestock-monitor`

**Description**: Detects animal presence, movement patterns, and
breathing in agricultural settings (barns, stalls, coops). Animal
CSI signatures differ from human signatures: quadrupedal gait has
different periodicity, and livestock breathing rates are species-
dependent (cattle: 12--30 BPM, sheep: 12--20, poultry: 15--30).
The module detects abnormal stillness (potential illness), labored
breathing, and escape events (sudden absence from a normally occupied
stall). Configurable for species via initialization parameters.

**Host API dependencies**: `csi_get_presence`, `csi_get_bpm_breathing`,
`csi_get_motion_energy`, `csi_get_variance`, `csi_get_timestamp`,
`csi_emit_event`, `csi_log`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 530 | `ANIMAL_PRESENT` | Count estimate |
| 531 | `ABNORMAL_STILLNESS` | Duration (seconds) |
| 532 | `LABORED_BREATHING` | Deviation from species baseline |
| 533 | `ESCAPE_ALERT` | Stall vacancy detected |

**Estimated .wasm size**: 6 KB
**Budget tier**: L (lightweight, < 2 ms)
**Difficulty**: Medium

---

### 5.5 `wdp-ind-structural-vibration`

**Description**: Uses CSI phase stability to detect building vibration,
earthquake P-wave early arrival, and structural stress. In a static
environment with no human presence, CSI phase should be stable to within
the noise floor (~0.02 rad). Structural vibration causes coherent
phase oscillation across all subcarriers simultaneously -- unlike
human movement which affects subcarrier groups selectively. The module
maintains a vibration spectral density estimate and alerts on: seismic
activity (broadband > 1 Hz), mechanical resonance (narrowband harmonics
from HVAC or machinery), and structural drift (slow monotonic phase
change indicating settlement or thermal expansion).

**Host API dependencies**: `csi_get_phase`, `csi_get_amplitude`,
`csi_get_variance`, `csi_get_phase_history`, `csi_get_presence`,
`csi_get_timestamp`, `csi_emit_event`, `csi_log`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 540 | `SEISMIC_DETECTED` | Peak acceleration proxy |
| 541 | `MECHANICAL_RESONANCE` | Dominant frequency (Hz) |
| 542 | `STRUCTURAL_DRIFT` | Phase drift rate (rad/hour) |
| 543 | `VIBRATION_SPECTRUM` | Encoded spectral peaks |

**Estimated .wasm size**: 10 KB
**Budget tier**: H (heavy, < 10 ms) -- spectral density estimation
**Difficulty**: Hard

---

## Category 6: Exotic & Research (Event IDs 600--699)

These modules push WiFi CSI sensing into territory that sounds like science
fiction -- but every one is grounded in published peer-reviewed research.
WiFi signals at 2.4/5 GHz have wavelengths (12.5 cm / 6 cm) that interact
with the human body at a resolution sufficient to detect chest wall
displacement of 0.1 mm (breathing), wrist pulse of 0.01 mm (heartbeat),
and even the micro-tremors of REM sleep eye movement. The following modules
exploit these physical phenomena in ways that challenge assumptions about
what contactless sensing can achieve.

### 6.1 `wdp-exo-dream-stage`

**Description**: Non-contact sleep stage classification from WiFi CSI alone.
During sleep, the body cycles through distinct physiological states that
produce measurable CSI signatures:

- **Awake**: Frequent large body movements, irregular breathing, variable
  heart rate.
- **NREM Stage 1-2 (light sleep)**: Reduced movement, regular breathing
  (12--20 BPM), heart rate stabilizes.
- **NREM Stage 3 (deep/slow-wave sleep)**: Near-zero voluntary movement,
  slow deep breathing (8--14 BPM), minimal heart rate variability.
- **REM sleep**: Body atonia (complete stillness of torso/limbs) combined
  with rapid irregular breathing, elevated heart rate variability, and
  micro-movements of the face/eyes that produce faint but detectable
  high-frequency CSI perturbations.

The module uses a state machine driven by breathing regularity, motion
energy, heart rate variability (from phase signal), and a micro-movement
spectral feature. Published research (Liu et al., MobiCom 2020; Niu et al.,
IEEE TMC 2022) has demonstrated >85% agreement with clinical polysomnography
using WiFi CSI. The module emits sleep stage transitions and computes sleep
quality metrics (sleep efficiency, REM percentage, deep sleep percentage).

This is non-contact polysomnography. No wearables, no electrodes, no cameras.
Just WiFi signals reflecting off a sleeping body.

**Host API dependencies**: `csi_get_bpm_breathing`, `csi_get_bpm_heartrate`,
`csi_get_motion_energy`, `csi_get_phase`, `csi_get_variance`,
`csi_get_phase_history`, `csi_get_presence`, `csi_get_timestamp`,
`csi_emit_event`, `csi_log`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 600 | `SLEEP_STAGE` | Stage (0=awake, 1=NREM1-2, 2=NREM3, 3=REM) |
| 601 | `SLEEP_QUALITY` | Sleep efficiency 0.0--1.0 |
| 602 | `REM_EPISODE` | Duration (minutes) |
| 603 | `DEEP_SLEEP_RATIO` | % of total sleep |

**Estimated .wasm size**: 14 KB
**Budget tier**: H (heavy, < 10 ms) -- multi-feature state machine
**Difficulty**: Hard

---

### 6.2 `wdp-exo-emotion-detect`

**Description**: Affect computing without cameras, microphones, or
wearables. Emotional states produce involuntary physiological changes
that alter CSI signatures:

- **Stress/anxiety**: Elevated breathing rate, shallow breathing pattern,
  increased heart rate, elevated micro-movement jitter (fidgeting,
  restlessness), reduced breathing regularity.
- **Calm/relaxation**: Slow deep breathing (6--10 BPM diaphragmatic
  pattern), low heart rate, minimal micro-movement, high breathing
  regularity.
- **Agitation/anger**: Rapid irregular breathing, sharp sudden movements,
  elevated motion energy with high temporal variance.

The module computes a multi-dimensional stress vector from breathing
pattern analysis (rate, depth, regularity), heart rate features (mean,
variability), and motion features (energy, jerk, entropy). Published
research (Zhao et al., UbiComp 2018; Yang et al., IEEE TAFFC 2021) has
demonstrated >70% accuracy in classifying calm/stress/agitation states.
The module outputs a continuous arousal-valence estimate rather than
discrete emotion labels, acknowledging the complexity of emotional states.

**Host API dependencies**: `csi_get_bpm_breathing`, `csi_get_bpm_heartrate`,
`csi_get_motion_energy`, `csi_get_phase`, `csi_get_variance`,
`csi_get_phase_history`, `csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 610 | `AROUSAL_LEVEL` | Low(0.0) to high(1.0) arousal |
| 611 | `STRESS_INDEX` | Composite stress score 0.0--1.0 |
| 612 | `CALM_DETECTED` | Confidence 0.0--1.0 |
| 613 | `AGITATION_DETECTED` | Confidence 0.0--1.0 |

**Estimated .wasm size**: 10 KB
**Budget tier**: H (heavy, < 10 ms)
**Difficulty**: Hard

---

### 6.3 `wdp-exo-gesture-language`

**Description**: Sign language letter recognition from hand and arm movement
CSI signatures. This extends the ADR-040 gesture module from simple hand
swipes to the 26 letters of American Sign Language (ASL) fingerspelling.
Each letter produces a distinctive sequence of phase disturbances across
frequency-diverse subcarriers as the hand and fingers assume different
configurations.

The module uses DTW (Dynamic Time Warping) template matching against a
library of 26 reference signatures, with a decision threshold to reject
non-letter movements. At 5 GHz (6 cm wavelength), finger-scale movements
produce measurable phase shifts of 0.1--0.5 radians. Published research
(Li et al., MobiCom 2019; Ma et al., NSDI 2019) has demonstrated
per-letter recognition accuracy of >90% at distances up to 2 meters.

This is an accessibility breakthrough: a deaf person can fingerspell
words in the air and have them recognized by WiFi -- no camera required,
works through visual obstructions, and preserves privacy since no images
are captured.

**Host API dependencies**: `csi_get_phase`, `csi_get_amplitude`,
`csi_get_variance`, `csi_get_phase_history`, `csi_get_motion_energy`,
`csi_get_presence`, `csi_get_timestamp`, `csi_emit_event`, `csi_log`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 620 | `LETTER_RECOGNIZED` | ASCII code of recognized letter |
| 621 | `LETTER_CONFIDENCE` | Recognition confidence 0.0--1.0 |
| 622 | `WORD_BOUNDARY` | Pause duration (ms) between letters |
| 623 | `GESTURE_REJECTED` | Non-letter movement detected |

**Estimated .wasm size**: 18 KB (includes 26 DTW templates)
**Budget tier**: H (heavy, < 10 ms) -- DTW matching against 26 templates
**Difficulty**: Hard

---

### 6.4 `wdp-exo-music-conductor`

**Description**: Tracks conductor baton or hand movements to generate MIDI-
compatible control signals. Extracts tempo (beats per minute from periodic
arm movement), dynamics (forte/piano from motion amplitude), and basic
gesture vocabulary (downbeat, upbeat, cutoff, fermata) from CSI phase
patterns. The conducting pattern at 4/4 time produces a characteristic
phase trajectory: strong downbeat, lateral second beat, higher third
beat, rebounding fourth beat -- each with distinct subcarrier signatures.

The module outputs BPM, beat position (1-2-3-4), and dynamic level as
events. A host application can map these to MIDI clock and CC messages
for controlling synthesizers, lighting rigs, or interactive installations.
This is an air instrument -- conduct an orchestra with WiFi.

**Host API dependencies**: `csi_get_phase`, `csi_get_amplitude`,
`csi_get_motion_energy`, `csi_get_phase_history`, `csi_get_variance`,
`csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 630 | `CONDUCTOR_BPM` | Detected tempo (BPM) |
| 631 | `BEAT_POSITION` | Beat number (1--4) |
| 632 | `DYNAMIC_LEVEL` | 0.0 (pianissimo) to 1.0 (fortissimo) |
| 633 | `GESTURE_CUTOFF` | 1.0 (stop gesture detected) |
| 634 | `GESTURE_FERMATA` | 1.0 (hold gesture detected) |

**Estimated .wasm size**: 10 KB
**Budget tier**: S (standard, < 5 ms)
**Difficulty**: Medium

---

### 6.5 `wdp-exo-plant-growth`

**Description**: Detects plant growth and leaf movement from micro-CSI
changes accumulated over hours and days. Plants are not static: leaves
undergo circadian nastic movements (opening/closing with light cycles),
growing tips extend at rates measurable in mm/day, and water-stressed
plants exhibit wilting that changes their RF cross-section.

The module operates on an extremely long time scale. It maintains
multi-hour EWMA baselines of amplitude and phase per subcarrier and
detects slow monotonic drift (growth), diurnal oscillation (circadian
movement), and sudden change (wilting, pruning, watering). Requires a
static environment with no human presence during measurement windows.
The presence flag gates measurement: data is only accumulated when
presence = 0.

This is botanical sensing through walls. Monitor your greenhouse from
the next room using only WiFi reflections off leaves.

**Host API dependencies**: `csi_get_amplitude`, `csi_get_phase`,
`csi_get_variance`, `csi_get_presence`, `csi_get_timestamp`,
`csi_emit_event`, `csi_log`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 640 | `GROWTH_RATE` | Amplitude drift rate (dB/day) |
| 641 | `CIRCADIAN_PHASE` | Estimated circadian cycle phase (hours) |
| 642 | `WILT_DETECTED` | Amplitude drop rate (sudden change) |
| 643 | `WATERING_EVENT` | Rapid amplitude recovery detected |

**Estimated .wasm size**: 6 KB
**Budget tier**: L (lightweight, < 2 ms) -- only updates EWMA
**Difficulty**: Medium

---

### 6.6 `wdp-exo-ghost-hunter`

**Description**: Environmental anomaly detector for CSI perturbations that
occur when no humans are present. Marketed as a paranormal investigation
tool (and genuinely used by ghost hunting communities), its actual utility
is detecting:

- **Hidden persons**: Someone concealed behind furniture or in a closet
  still displaces air and produces micro-CSI signatures from breathing.
- **Gas leaks**: Air density changes from gas accumulation alter the
  RF propagation medium, producing slow phase drift.
- **Structural settling**: Building creaks and shifts produce impulsive
  CSI disturbances.
- **Pest activity**: Rodents and large insects produce faint but
  detectable motion signatures.
- **HVAC anomalies**: Unusual airflow patterns from duct failures.
- **Electromagnetic interference**: External RF sources that modulate
  the CSI channel.

The module requires presence = 0 (room declared empty) and monitors
for any CSI perturbation above the noise floor. It classifies anomalies
by temporal signature: impulsive (structural), periodic (mechanical/
biological), drift (environmental), and random (interference). Every
anomaly is logged with timestamp and spectral fingerprint.

Whether you are looking for ghosts or gas leaks, this module watches
the invisible.

**Host API dependencies**: `csi_get_phase`, `csi_get_amplitude`,
`csi_get_variance`, `csi_get_phase_history`, `csi_get_presence`,
`csi_get_motion_energy`, `csi_get_timestamp`, `csi_emit_event`,
`csi_log`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 650 | `ANOMALY_DETECTED` | Anomaly energy (dB above noise floor) |
| 651 | `ANOMALY_CLASS` | Type (1=impulsive, 2=periodic, 3=drift, 4=random) |
| 652 | `HIDDEN_PRESENCE` | Confidence 0.0--1.0 (breathing-like signature) |
| 653 | `ENVIRONMENTAL_DRIFT` | Phase drift rate (rad/hour) |

**Estimated .wasm size**: 8 KB
**Budget tier**: S (standard, < 5 ms)
**Difficulty**: Medium

---

### 6.7 `wdp-exo-rain-detect`

**Description**: Detects rain on windows and roofing from vibration-induced
CSI micro-disturbances. Raindrops striking a surface produce broadband
impulse vibrations that propagate through the building structure and
modulate the CSI channel. The module detects rain onset, estimates
intensity (light/moderate/heavy) from the aggregate vibration energy,
and identifies cessation. Works because the ESP32 node is physically
mounted to the building structure, coupling rainfall vibrations into
the RF path.

This is weather sensing without any outdoor sensors -- the WiFi signal
inside the building feels the rain on the roof.

**Host API dependencies**: `csi_get_phase`, `csi_get_variance`,
`csi_get_amplitude`, `csi_get_presence`, `csi_get_timestamp`,
`csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 660 | `RAIN_ONSET` | 1.0 |
| 661 | `RAIN_INTENSITY` | 0=none, 1=light, 2=moderate, 3=heavy |
| 662 | `RAIN_CESSATION` | Total duration (minutes) |

**Estimated .wasm size**: 4 KB
**Budget tier**: L (lightweight, < 2 ms)
**Difficulty**: Easy

---

### 6.8 `wdp-exo-breathing-sync`

**Description**: Detects when multiple people's breathing patterns
synchronize -- a real phenomenon observed in meditation groups, sleeping
couples, and audience/performer interactions. When two or more people are
in the same CSI field, their individual breathing signatures appear as
superimposed periodic components in the phase signal. The module performs
pairwise cross-correlation of breathing components (extracted via
subcarrier group decomposition from Tier 2) and reports synchronization
when the phase-locked value exceeds a threshold.

Published research (Adib et al., SIGCOMM 2015; Wang et al., MobiSys
2017) has demonstrated the ability to separate and correlate multiple
people's breathing using WiFi CSI. Applications include:

- **Meditation quality assessment**: Group coherence metric for
  mindfulness sessions.
- **Couple sleep monitoring**: Detect when partners' breathing aligns
  during sleep (associated with deeper sleep quality).
- **Crowd resonance**: Large-group breathing synchronization at concerts,
  sports events, or religious gatherings -- a measurable indicator of
  collective emotional engagement.
- **Therapeutic monitoring**: Breathing synchronization between therapist
  and patient (rapport indicator).

The social coherence metric -- a number that quantifies how in-sync a
group of humans is breathing -- is something that was unmeasurable before
contactless sensing. WiFi CSI makes the invisible visible.

**Host API dependencies**: `csi_get_bpm_breathing`, `csi_get_phase`,
`csi_get_variance`, `csi_get_n_persons`, `csi_get_phase_history`,
`csi_get_timestamp`, `csi_emit_event`

**Event types emitted**:

| Event ID | Name | Value semantics |
|----------|------|-----------------|
| 670 | `SYNC_DETECTED` | Phase-locked value 0.0--1.0 |
| 671 | `SYNC_PAIR_COUNT` | Number of synchronized pairs |
| 672 | `GROUP_COHERENCE` | Overall group coherence index 0.0--1.0 |
| 673 | `SYNC_LOST` | Desynchronization event |

**Estimated .wasm size**: 10 KB
**Budget tier**: S (standard, < 5 ms) -- cross-correlation of breathing components
**Difficulty**: Hard

---

## Module Summary Table

| # | Module | Category | Events | .wasm | Budget | Difficulty |
|---|--------|----------|--------|-------|--------|------------|
| 1 | `wdp-med-sleep-apnea` | Medical | 100--102 | 4 KB | L | Easy |
| 2 | `wdp-med-cardiac-arrhythmia` | Medical | 110--113 | 8 KB | S | Hard |
| 3 | `wdp-med-respiratory-distress` | Medical | 120--123 | 10 KB | H | Hard |
| 4 | `wdp-med-gait-analysis` | Medical | 130--134 | 12 KB | H | Hard |
| 5 | `wdp-med-seizure-detect` | Medical | 140--143 | 10 KB | S | Hard |
| 6 | `wdp-med-vital-trend` | Medical | 150--153 | 6 KB | L | Medium |
| 7 | `wdp-sec-intrusion-detect` | Security | 200--202 | 8 KB | S | Medium |
| 8 | `wdp-sec-perimeter-breach` | Security | 210--213 | 10 KB | S | Medium |
| 9 | `wdp-sec-weapon-detect` | Security | 220--222 | 8 KB | S | Hard |
| 10 | `wdp-sec-tailgating` | Security | 230--232 | 6 KB | L | Medium |
| 11 | `wdp-sec-loitering` | Security | 240--242 | 3 KB | L | Easy |
| 12 | `wdp-sec-panic-motion` | Security | 250--252 | 6 KB | S | Medium |
| 13 | `wdp-bld-occupancy-zones` | Building | 300--303 | 8 KB | S | Medium |
| 14 | `wdp-bld-hvac-presence` | Building | 310--312 | 3 KB | L | Easy |
| 15 | `wdp-bld-lighting-zones` | Building | 320--322 | 4 KB | L | Easy |
| 16 | `wdp-bld-elevator-count` | Building | 330--333 | 8 KB | S | Medium |
| 17 | `wdp-bld-meeting-room` | Building | 340--343 | 5 KB | L | Easy |
| 18 | `wdp-bld-energy-audit` | Building | 350--352 | 6 KB | L | Medium |
| 19 | `wdp-ret-queue-length` | Retail | 400--403 | 6 KB | L | Medium |
| 20 | `wdp-ret-dwell-heatmap` | Retail | 410--413 | 6 KB | L | Medium |
| 21 | `wdp-ret-customer-flow` | Retail | 420--423 | 8 KB | S | Medium |
| 22 | `wdp-ret-table-turnover` | Retail | 430--433 | 4 KB | L | Easy |
| 23 | `wdp-ret-shelf-engagement` | Retail | 440--443 | 6 KB | S | Medium |
| 24 | `wdp-ind-forklift-proximity` | Industrial | 500--502 | 10 KB | S | Hard |
| 25 | `wdp-ind-confined-space` | Industrial | 510--514 | 5 KB | L | Medium |
| 26 | `wdp-ind-clean-room` | Industrial | 520--523 | 4 KB | L | Easy |
| 27 | `wdp-ind-livestock-monitor` | Industrial | 530--533 | 6 KB | L | Medium |
| 28 | `wdp-ind-structural-vibration` | Industrial | 540--543 | 10 KB | H | Hard |
| 29 | `wdp-exo-dream-stage` | Exotic | 600--603 | 14 KB | H | Hard |
| 30 | `wdp-exo-emotion-detect` | Exotic | 610--613 | 10 KB | H | Hard |
| 31 | `wdp-exo-gesture-language` | Exotic | 620--623 | 18 KB | H | Hard |
| 32 | `wdp-exo-music-conductor` | Exotic | 630--634 | 10 KB | S | Medium |
| 33 | `wdp-exo-plant-growth` | Exotic | 640--643 | 6 KB | L | Medium |
| 34 | `wdp-exo-ghost-hunter` | Exotic | 650--653 | 8 KB | S | Medium |
| 35 | `wdp-exo-rain-detect` | Exotic | 660--662 | 4 KB | L | Easy |
| 36 | `wdp-exo-breathing-sync` | Exotic | 670--673 | 10 KB | S | Hard |

**Totals**: 37 modules, 133 event types, median size 6 KB, 15 easy / 12 medium / 11 hard.

---

## Module Manifest Convention

### RVF Manifest Fields

Every module ships as an RVF container (ADR-040 Appendix C) with these
standardized manifest fields:

| Field | Convention |
|-------|-----------|
| `module_name` | `wdp-{category}-{name}`, max 32 chars |
| `required_host_api` | `1` (all modules target Host API v1) |
| `capabilities` | Bitmask of required host functions (ADR-040 C.4) |
| `max_frame_us` | Budget tier: L=2000, S=5000, H=10000 |
| `max_events_per_sec` | Typical: 10 for lightweight, 20 for standard, 5 for heavy |
| `memory_limit_kb` | Module-specific, default 32 KB |
| `event_schema_version` | `1` for all initial modules |
| `min_subcarriers` | Minimum required (8 for most, 32 for exotic) |
| `author` | Contributor handle, max 10 chars |

### TOML Manifest (Human-Readable)

Each module includes a `.toml` companion for human review and tooling:

```toml
[module]
name = "wdp-med-sleep-apnea"
version = "1.0.0"
description = "Detects breathing cessation during sleep"
author = "ruvnet"
license = "MIT"
category = "medical"
difficulty = "easy"

[api]
host_api_version = 1
capabilities = ["READ_VITALS", "EMIT_EVENTS", "LOG"]

[budget]
tier = "lightweight"
max_frame_us = 2000
max_events_per_sec = 10
memory_limit_kb = 16

[events]
100 = { name = "APNEA_START", unit = "seconds" }
101 = { name = "APNEA_END", unit = "seconds" }
102 = { name = "AHI_UPDATE", unit = "events_per_hour" }

[build]
target = "wasm32-unknown-unknown"
profile = "release"
min_subcarriers = 8
```

### Event Type ID Registry

| Range | Category | Allocation |
|-------|----------|------------|
| 0--99 | Core / ADR-040 flagship | Reserved for system and flagship modules |
| 100--199 | Medical & Health | 6 modules, ~24 event types allocated |
| 200--299 | Security & Safety | 6 modules, ~18 event types allocated |
| 300--399 | Smart Building | 6 modules, ~20 event types allocated |
| 400--499 | Retail & Hospitality | 5 modules, ~16 event types allocated |
| 500--599 | Industrial & Specialized | 5 modules, ~16 event types allocated |
| 600--699 | Exotic & Research | 8 modules, ~30 event types allocated |
| 700--899 | Reserved for future categories | Unallocated |
| 900--999 | Community / third-party | Open allocation via registry PR |

Within each range, modules are assigned 10-ID blocks (e.g., sleep-apnea
gets 100--109, cardiac-arrhythmia gets 110--119). This leaves room for
future event types within each module without reallocating.

---

## Registry Structure

```
modules/
  registry.toml              # Master index of all modules with versions
  README.md                  # Auto-generated catalog with descriptions
  medical/
    sleep-apnea/
      wdp-med-sleep-apnea.rvf         # Signed RVF container
      wdp-med-sleep-apnea.toml        # Human-readable manifest
      wdp-med-sleep-apnea.wasm        # Raw WASM (for dev/debug)
      src/
        lib.rs                         # Module source code
        Cargo.toml                     # Crate manifest
      tests/
        integration.rs                 # Test against mock host API
      CHANGELOG.md
    cardiac-arrhythmia/
      ...
    respiratory-distress/
      ...
  security/
    intrusion-detect/
      wdp-sec-intrusion-detect.rvf
      wdp-sec-intrusion-detect.toml
      src/
        lib.rs
        Cargo.toml
      tests/
        integration.rs
      CHANGELOG.md
    perimeter-breach/
      ...
  building/
    occupancy-zones/
      ...
    hvac-presence/
      ...
  retail/
    queue-length/
      ...
    dwell-heatmap/
      ...
  industrial/
    forklift-proximity/
      ...
    confined-space/
      ...
  exotic/
    dream-stage/
      ...
    emotion-detect/
      ...
    ghost-hunter/
      ...
```

### `registry.toml` Format

```toml
[registry]
version = "1.0.0"
host_api_version = 1
total_modules = 37

[[modules]]
name = "wdp-med-sleep-apnea"
version = "1.0.0"
category = "medical"
event_range = [100, 102]
wasm_size_kb = 4
budget_tier = "lightweight"
status = "stable"      # stable | beta | experimental | deprecated
sha256 = "abc123..."

[[modules]]
name = "wdp-exo-dream-stage"
version = "0.1.0"
category = "exotic"
event_range = [600, 603]
wasm_size_kb = 14
budget_tier = "heavy"
status = "experimental"
sha256 = "def456..."
```

---

## Consequences

### Positive

1. **Market multiplier**: A single $8 ESP32-S3 node becomes a multi-purpose
   sensing platform. A hospital buys one SKU and deploys sleep apnea
   detection in the ICU, fall detection in geriatrics, and queue management
   in the ER -- all via WASM module uploads. No hardware changes, no
   reflashing.

2. **Community velocity**: The module contract (12 host functions, RVF
   container, TOML manifest) is simple enough for a graduate student to
   implement a new sensing algorithm in a weekend. The 15 "easy" difficulty
   modules are specifically designed as on-ramps for first-time contributors.

3. **Research platform**: The exotic modules provide a credible,
   reproducible platform for WiFi sensing research. Instead of each lab
   building their own CSI collection and processing pipeline, researchers
   can focus on their algorithm and package it as a WASM module that runs
   on any WiFi-DensePose deployment.

4. **Vertical expansion**: Each category targets a different market segment
   with its own buyers, compliance requirements, and ROI models. Medical
   modules sell to hospitals and eldercare. Security modules sell to
   commercial real estate. Retail modules sell to chains. Industrial
   modules sell to manufacturing. This diversifies the addressable market
   by 10x without diversifying the hardware.

5. **Regulatory pathway**: Medical modules can pursue FDA 510(k) clearance
   independently of the base firmware. The WASM isolation boundary provides
   a natural regulatory decomposition: the firmware is the platform
   (Class I), individual medical modules pursue device classification
   independently.

6. **Graceful degradation**: Every module is optional. A node runs with
   zero modules (Tier 0-2 only) or any combination. If a module faults,
   the runtime auto-stops it and the rest continue. There is no single
   point of failure in the module collection.

### Negative

1. **Event type sprawl**: 133 event types across 37 modules create a
   large surface area for the receiving application to handle. Consumers
   must filter by event type range and can safely ignore unknown types,
   but documentation and SDK effort scales with the collection size.

2. **Quality assurance burden**: Each module needs testing, documentation,
   and ongoing maintenance. Community-contributed modules may have
   inconsistent quality. The curated registry model (PR-based submission
   with review) adds editorial overhead.

3. **Accuracy expectations**: Medical and security modules carry
   liability risk if accuracy claims are overstated. Every medical module
   must carry a disclaimer that it is not a medical device unless
   separately cleared. Every security module must state it supplements
   but does not replace physical security.

4. **Module interaction**: Running multiple modules concurrently may
   produce conflicting events (e.g., `intrusion-detect` and `ghost-hunter`
   both fire on the same CSI anomaly). Consumers must handle event
   deduplication. The event type ID system makes this tractable but
   not automatic.

5. **WASM size growth**: The exotic modules (gesture-language at 18 KB,
   dream-stage at 14 KB) approach the PSRAM arena limit. Only 2-3 heavy
   modules can coexist in the 4-slot runtime. Module authors must
   optimize aggressively for size.

6. **Calibration requirements**: Many modules (occupancy-zones, perimeter-
   breach, gait-analysis) require environment-specific calibration.
   A standardized calibration protocol and tooling are needed but are
   outside the scope of this ADR.

---

## Implementation Priority

### Phase 1 -- Ship First (Q2 2026)

These modules deliver immediate value with low implementation risk.
They form the "launch collection" for the WASM module marketplace.

| Module | Status | Rationale |
|--------|--------|-----------|
| `wdp-bld-occupancy-zones` | **Implemented** (`occupancy.rs`) | Most requested feature; direct revenue from smart building contracts |
| `wdp-sec-intrusion-detect` | **Implemented** (`intrusion.rs`) | Security is the #1 use case after occupancy; differentiator vs PIR |
| `wdp-med-sleep-apnea` | Planned | High-impact medical use case; simple to implement on Tier 2 vitals |
| `wdp-ret-queue-length` | Planned | Retail deployments already in pipeline; queue analytics requested |
| `wdp-med-vital-trend` | **Implemented** (`vital_trend.rs`) | Leverages existing vitals data; needed for clinical pilot |

### Phase 2 -- Community (Q3-Q4 2026)

These modules are medium-difficulty and designed for community contribution.
Each has a well-defined scope and clear test criteria.

| Module | Rationale |
|--------|-----------|
| `wdp-med-gait-analysis` | High clinical value; active research community |
| `wdp-ret-dwell-heatmap` | Builds on occupancy-zones; clear commercial demand |
| `wdp-bld-meeting-room` | Extends occupancy for workplace analytics market |
| `wdp-bld-hvac-presence` | Low effort (wraps presence with hysteresis); BMS integration |
| `wdp-sec-loitering` | Simple state machine; good first contribution |
| `wdp-ind-confined-space` | OSHA compliance driver; clear acceptance criteria |
| `wdp-exo-ghost-hunter` | Community enthusiasm driver; good PR and engagement |
| `wdp-exo-rain-detect` | Simple and delightful; demonstrates CSI versatility |

### Phase 3 -- Research Frontier (2027+)

These modules push the boundaries of WiFi CSI sensing and require
specialized expertise, larger datasets, and possibly new Host API
extensions.

| Module | Rationale |
|--------|-----------|
| `wdp-exo-dream-stage` | Highest novelty; needs sleep lab validation dataset |
| `wdp-exo-emotion-detect` | Requires controlled study; IRB considerations |
| `wdp-exo-gesture-language` | Needs ASL template library; accessibility impact |
| `wdp-sec-weapon-detect` | Research-grade only; security implications require careful positioning |
| `wdp-ind-structural-vibration` | Needs civil engineering domain expertise |
| `wdp-med-cardiac-arrhythmia` | Needs clinical validation; potential regulatory pathway |
| `wdp-med-seizure-detect` | Needs neurology collaboration; high clinical impact |
| `wdp-exo-breathing-sync` | Needs multi-person datasets; novel social metric |

---

## Community Contribution Guide

### How to Write a Module

**1. Set up the development environment.**

```bash
# Clone the repo and navigate to the module template
git clone https://github.com/ruvnet/wifi-densepose.git
cd wifi-densepose/modules

# Copy the template
cp -r _template/ exotic/my-module/
cd exotic/my-module/src/
```

**2. Write the module in Rust (`no_std`).**

Every module implements three exported functions:

```rust
#![no_std]
#![no_main]

// Host API imports (provided by the WASM3 runtime)
extern "C" {
    fn csi_get_phase(sc: i32) -> f32;
    fn csi_get_amplitude(sc: i32) -> f32;
    fn csi_get_variance(sc: i32) -> f32;
    fn csi_get_bpm_breathing() -> f32;
    fn csi_get_bpm_heartrate() -> f32;
    fn csi_get_presence() -> i32;
    fn csi_get_motion_energy() -> f32;
    fn csi_get_n_persons() -> i32;
    fn csi_get_timestamp() -> i32;
    fn csi_emit_event(event_type: i32, value: f32);
    fn csi_log(ptr: i32, len: i32);
    fn csi_get_phase_history(buf: i32, max: i32) -> i32;
}

// Module state (lives in WASM linear memory)
static mut STATE: ModuleState = ModuleState::new();

struct ModuleState {
    // Your state fields here
    initialized: bool,
}

impl ModuleState {
    const fn new() -> Self {
        Self { initialized: false }
    }
}

#[no_mangle]
pub extern "C" fn on_init() {
    unsafe {
        STATE = ModuleState::new();
        STATE.initialized = true;
    }
}

#[no_mangle]
pub extern "C" fn on_frame(n_subcarriers: i32) {
    unsafe {
        if !STATE.initialized { return; }

        // Your per-frame logic here
        // Call csi_get_* functions to read sensor data
        // Call csi_emit_event(EVENT_TYPE, value) to emit results
    }
}

#[no_mangle]
pub extern "C" fn on_timer() {
    // Periodic tasks (called at configurable interval)
}

// Panic handler required for no_std
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
```

**3. Build to WASM.**

```bash
# Install the wasm32 target
rustup target add wasm32-unknown-unknown

# Build in release mode (optimized for size)
cargo build --target wasm32-unknown-unknown --release

# Strip debug symbols
wasm-strip target/wasm32-unknown-unknown/release/my_module.wasm

# Verify size (should be < 128 KB, ideally < 20 KB)
ls -la target/wasm32-unknown-unknown/release/my_module.wasm
```

**4. Write the TOML manifest.**

```toml
[module]
name = "wdp-exo-my-module"
version = "0.1.0"
description = "Brief description of what it detects"
author = "your-handle"
license = "MIT"
category = "exotic"
difficulty = "medium"

[api]
host_api_version = 1
capabilities = ["READ_PHASE", "READ_VARIANCE", "EMIT_EVENTS"]

[budget]
tier = "standard"
max_frame_us = 5000
max_events_per_sec = 10
memory_limit_kb = 32

[events]
900 = { name = "MY_EVENT", unit = "score" }
901 = { name = "MY_OTHER_EVENT", unit = "confidence" }
```

**5. Test locally.**

The repository provides a mock Host API for desktop testing:

```bash
# Run against the mock host with synthetic CSI data
cargo test --target x86_64-unknown-linux-gnu

# Run against recorded CSI data (if available)
cargo run --example replay -- --input ../../data/recordings/test.csv
```

**6. Package as RVF.**

```bash
# Build the RVF container (requires the wasm-edge CLI tool)
cargo run -p wifi-densepose-wasm-edge --features std -- \
  rvf pack \
  --wasm target/wasm32-unknown-unknown/release/my_module.wasm \
  --manifest wdp-exo-my-module.toml \
  --output wdp-exo-my-module.rvf
```

**7. Submit a PR.**

```
modules/exotic/my-module/
  wdp-exo-my-module.rvf
  wdp-exo-my-module.toml
  wdp-exo-my-module.wasm
  src/
    lib.rs
    Cargo.toml
  tests/
    integration.rs
  CHANGELOG.md
```

PR checklist:
- [ ] Module name follows `wdp-{category}-{name}` convention
- [ ] Event type IDs are within the correct category range
- [ ] TOML manifest is complete and valid
- [ ] WASM binary is < 128 KB (< 20 KB preferred)
- [ ] Budget tier is appropriate (verified by benchmark)
- [ ] Integration tests pass against mock Host API
- [ ] No `std` dependencies (pure `no_std`)
- [ ] CHANGELOG.md describes the module
- [ ] Code is formatted with `rustfmt`
- [ ] No unsafe code beyond the Host API FFI bindings

### Signing for Release

Community modules are unsigned during development. For inclusion in the
official registry, a project maintainer signs the RVF with the project
Ed25519 key:

```bash
# Maintainer-only: sign and publish
wifi-densepose-wasm-edge rvf sign \
  --input wdp-exo-my-module.rvf \
  --key keys/signing.ed25519 \
  --output wdp-exo-my-module.signed.rvf
```

Unsigned modules can still be loaded on nodes with `wasm_verify=0`
(development mode). Production nodes require signed RVF containers.

### Event Type ID Allocation

- Categories 100--599: Allocated by this ADR. New modules in existing
  categories use the next available 10-ID block.
- Category 600--699 (Exotic): Allocated by this ADR. New exotic modules
  use the next available 10-ID block starting at 680.
- Range 900--999: Open for community/third-party modules. Claim a 10-ID
  block by adding an entry to `modules/registry.toml` in your PR.
- Conflicts are resolved during PR review on a first-come basis.

---

## References

- ADR-039: ESP32-S3 Edge Intelligence Pipeline
- ADR-040: WASM Programmable Sensing (Tier 3)
- Liu et al., "Monitoring Vital Signs and Postures During Sleep Using
  WiFi Signals," MobiCom 2020
- Niu et al., "WiFi-Based Sleep Stage Monitoring," IEEE TMC 2022
- Zhao et al., "Emotion Recognition Using Wireless Signals," UbiComp 2018
- Yang et al., "WiFi-Based Emotion Detection," IEEE TAFFC 2021
- Li et al., "Sign Language Recognition via WiFi," MobiCom 2019
- Ma et al., "WiFi Sensing with Channel State Information," NSDI 2019
- Adib et al., "Smart Homes that Monitor Breathing and Heart Rate,"
  SIGCOMM 2015
- Wang et al., "Human Respiration Detection with Commodity WiFi Devices,"
  MobiSys 2017
- Halperin et al., "Tool Release: Gathering 802.11n Traces with Channel
  State Information," ACM CCR 2011
