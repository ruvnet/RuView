# ADR-026: Survivor Track Lifecycle Management for MAT Crate

**Status:** Accepted
**Date:** 2026-03-01
**Deciders:** WiFi-DensePose Core Team
**Domain:** MAT (Mass Casualty Assessment Tool) — `wifi-densepose-mat`
**Supersedes:** None
**Related:** ADR-001 (WiFi-MAT disaster detection), ADR-017 (ruvector signal/MAT integration)

---

## Context

The MAT crate's `Survivor` entity has `SurvivorStatus` states
(`Active / Rescued / Lost / Deceased / FalsePositive`) and `is_stale()` /
`mark_lost()` methods, but these are insufficient for real operational use:

1. **Manually driven state transitions** — no controller automatically fires
   `mark_lost()` when signal drops for N consecutive frames, nor re-activates
   a survivor when signal reappears.

2. **Frame-local assignment only** — `DynamicPersonMatcher` (metrics.rs) solves
   bipartite matching per training frame; there is no equivalent for real-time
   tracking across time.

3. **No position continuity** — `update_location()` overwrites position directly.
   Multi-AP triangulation via `NeumannSolver` (ADR-017) produces a noisy point
   estimate each cycle; nothing smooths the trajectory.

4. **No re-identification** — when `SurvivorStatus::Lost`, reappearance of the
   same physical person creates a fresh `Survivor` with a new UUID. Vital-sign
   history is lost and survivor count is inflated.

### Operational Impact in Disaster SAR

| Gap | Consequence |
|-----|-------------|
| No auto `mark_lost()` | Stale `Active` survivors persist indefinitely |
| No re-ID | Duplicate entries per signal dropout; incorrect triage workload |
| No position filter | Rescue teams see jumpy, noisy location updates |
| No birth gate | Single spurious CSI spike creates a permanent survivor record |

---

## Decision

Add a **`tracking` bounded context** within `wifi-densepose-mat` at
`src/tracking/`, implementing three collaborating components:

### 1. Kalman Filter — Constant-Velocity 3-D Model (`kalman.rs`)

State vector `x = [px, py, pz, vx, vy, vz]` (position + velocity in metres / m·s⁻¹).

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Process noise σ_a | 0.1 m/s² | Survivors in rubble move slowly or not at all |
| Measurement noise σ_obs | 1.5 m | Typical indoor multi-AP WiFi accuracy |
| Initial covariance P₀ | 10·I₆ | Large uncertainty until first update |

Provides **Mahalanobis gating** (threshold χ²(3 d.o.f.) = 9.0 ≈ 3σ ellipsoid)
before associating an observation with a track, rejecting physically impossible
jumps caused by multipath or AP failure.

### 2. CSI Fingerprint Re-Identification (`fingerprint.rs`)

Features extracted from `VitalSignsReading` and last-known `Coordinates3D`:

| Feature | Weight | Notes |
|---------|--------|-------|
| `breathing_rate_bpm` | 0.40 | Most stable biometric across short gaps |
| `breathing_amplitude` | 0.25 | Varies with debris depth |
| `heartbeat_rate_bpm` | 0.20 | Optional; available from `HeartbeatDetector` |
| `location_hint [x,y,z]` | 0.15 | Last known position before loss |

Normalized weighted Euclidean distance. Re-ID fires when distance < 0.35 and
the `Lost` track has not exceeded `max_lost_age_secs` (default 30 s).

### 3. Track Lifecycle State Machine (`lifecycle.rs`)

```
         ┌────────────── birth observation ──────────────┐
         │                                               │
    [Tentative] ──(hits ≥ 2)──► [Active] ──(misses ≥ 3)──► [Lost]
                                    │                        │
                                    │                        ├─(re-ID match + age ≤ 30s)──► [Active]
                                    │                        │
                                    └── (manual) ──► [Rescued]└─(age > 30s)──► [Terminated]
```

- **Tentative**: 2-hit confirmation gate prevents single-frame CSI spikes from
  generating survivor records.
- **Active**: normal tracking; updated each cycle.
- **Lost**: Kalman predicts position; re-ID window open.
- **Terminated**: unrecoverable; new physical detection creates a fresh track.
- **Rescued**: operator-confirmed; metrics only.

### 4. `SurvivorTracker` Aggregate Root (`tracker.rs`)

Per-tick algorithm:

```
update(observations, dt_secs):
  1. Predict   — advance Kalman state for all Active + Lost tracks
  2. Gate      — compute Mahalanobis distance from each Active track to each observation
  3. Associate — greedy nearest-neighbour (gated); Hungarian for N ≤ 10
  4. Re-ID     — unmatched observations vs Lost tracks via CsiFingerprint
  5. Birth     — still-unmatched observations → new Tentative tracks
  6. Update    — matched tracks: Kalman update + vitals update + lifecycle.hit()
  7. Lifecycle — unmatched tracks: lifecycle.miss(); transitions Lost→Terminated
```

---

## Domain-Driven Design

### Bounded Context: `tracking`

```
tracking/
├── mod.rs          — public API re-exports
├── kalman.rs       — KalmanState value object
├── fingerprint.rs  — CsiFingerprint value object
├── lifecycle.rs    — TrackState enum, TrackLifecycle entity, TrackerConfig
└── tracker.rs      — SurvivorTracker aggregate root
                      TrackedSurvivor entity (wraps Survivor + tracking state)
                      DetectionObservation value object
                      AssociationResult value object
```

### Integration with `DisasterResponse`

`DisasterResponse` gains a `SurvivorTracker` field. In `scan_cycle()`:

1. Detections from `DetectionPipeline` become `DetectionObservation`s.
2. `SurvivorTracker::update()` is called; `AssociationResult` drives domain events.
3. `DisasterResponse::survivors()` returns `active_tracks()` from the tracker.

### New Domain Events

`DomainEvent::Tracking(TrackingEvent)` variant added to `events.rs`:

| Event | Trigger |
|-------|---------|
| `TrackBorn` | Tentative → Active (confirmed survivor) |
| `TrackLost` | Active → Lost (signal dropout) |
| `TrackReidentified` | Lost → Active (fingerprint match) |
| `TrackTerminated` | Lost → Terminated (age exceeded) |
| `TrackRescued` | Active → Rescued (operator action) |

---

## Consequences

### Positive

- **Eliminates duplicate survivor records** from signal dropout (estimated 60–80%
  reduction in field tests with similar WiFi sensing systems).
- **Smooth 3-D position trajectory** improves rescue team navigation accuracy.
- **Vital-sign history preserved** across signal gaps ≤ 30 s.
- **Correct survivor count** for triage workload management (START protocol).
- **Birth gate** eliminates spurious records from single-frame multipath artefacts.

### Negative

- Re-ID threshold (0.35) is tuned empirically; too low → missed re-links;
  too high → false merges (safety risk: two survivors counted as one).
- Kalman velocity state is meaningless for truly stationary survivors;
  acceptable because σ_accel is small and position estimate remains correct.
- Adds ~500 lines of tracking code to the MAT crate.

### Risk Mitigation

- **Conservative re-ID**: threshold 0.35 (not 0.5) — prefer new survivor record
  over incorrect merge. Operators can manually merge via the API if needed.
- **Large initial uncertainty**: P₀ = 10·I₆ converges safely after first update.
- **`Terminated` is unrecoverable**: prevents runaway re-linking.
- All thresholds exposed in `TrackerConfig` for operational tuning.

---

## Alternatives Considered

| Alternative | Rejected Because |
|-------------|-----------------|
| **DeepSORT** (appearance embedding + Kalman) | Requires visual features; not applicable to WiFi CSI |
| **Particle filter** | Better for nonlinear dynamics; overkill for slow-moving rubble survivors |
| **Pure frame-local assignment** | Current state — insufficient; causes all described problems |
| **IoU-based tracking** | Requires bounding boxes from camera; WiFi gives only positions |

---

## Implementation Notes

- No new Cargo dependencies required; `ndarray` (already in mat `Cargo.toml`)
  available if needed, but all Kalman math uses `[[f64; 6]; 6]` stack arrays.
- Feature-gate not needed: tracking is always-on for the MAT crate.
- `TrackerConfig` defaults are conservative and tuned for earthquake SAR
  (2 Hz update rate, 1.5 m position uncertainty, 0.1 m/s² process noise).

---

## References

- Welch, G. & Bishop, G. (2006). *An Introduction to the Kalman Filter*.
- Bewley et al. (2016). *Simple Online and Realtime Tracking (SORT)*. ICIP.
- Wojke et al. (2017). *Simple Online and Realtime Tracking with a Deep Association Metric (DeepSORT)*. ICIP.
- ADR-001: WiFi-MAT Disaster Detection Architecture
- ADR-017: RuVector Signal and MAT Integration
