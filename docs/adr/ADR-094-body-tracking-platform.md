# ADR-094: Body Tracking Platform

## Status
Accepted

## Context

RuView currently detects presence and estimates person count via CSI variance heuristics and RF tomography cluster analysis. Individual body tracking (assigning persistent IDs, tracking movement over time, detecting room transitions) is missing. The `pose_tracker` module exists but operates on synthetic keypoints.

The voxel cluster system provides per-body centroid, bounding box, and pose hint. This is the foundation for tracking.

Note: Upstream ADR-081 (Adaptive CSI Mesh Firmware Kernel, accepted 2026-04-19) covers a different concern — firmware vtable and adaptive kernel selection. This ADR-094 covers the body-tracking layer and is numbered to avoid collision with that accepted ADR.

## Decision

Build a body tracking layer on top of the existing cluster output that:
1. Assigns persistent IDs to clusters across frames (Hungarian algorithm / nearest-neighbor)
2. Tracks centroid trajectories with Kalman filtering for smooth paths
3. Detects room zone membership (configurable zones via API)
4. Emits tracking events to the event stream
5. Exposes tracking state via REST API

## Architecture

```
spatial_pipeline → find_body_clusters() → Vec<BodyCluster>
                                              ↓
                              body_tracker.rs → Vec<TrackedBody>
                                              ↓
                              zone_transitions → zone_enter/zone_exit events
                                              ↓
                              REST: /api/v1/tracking/bodies
                              REST: /api/v1/tracking/zones
                              WS: tracking_update messages
```

### TrackedBody
```rust
struct TrackedBody {
    id: u32,              // persistent ID (increments, never reused in session)
    centroid: [f64; 3],   // current position (Kalman-filtered)
    velocity: [f64; 3],   // estimated velocity (m/s)
    pose_hint: String,    // standing/sitting/lying
    zone: Option<String>, // current zone name
    first_seen: Instant,
    last_seen: Instant,
    track_quality: f64,   // 0-1, decays when cluster is missing
}
```

### Association Algorithm
- Each frame: compute distance matrix between existing tracks and new clusters
- Greedy nearest-neighbor assignment (< 2m threshold)
- Unmatched clusters → new track
- Unmatched tracks → decay quality; remove after 20 absent frames

### Zone System
- Zones defined as axis-aligned boxes in 3D space
- Configurable via `POST /api/v1/tracking/zones`
- Default: whole room = one zone
- Zone transitions emit `ZoneTransition` events

## Implementation

### New file: `body_tracker.rs`
- `BodyTracker` struct with track state
- `update(clusters: &[BodyCluster]) -> TrackingUpdate`
- Kalman filter (simple: position + velocity, 6-state)
- Zone membership check
- Self-contained: `BodyCluster` defined inline (no dependency on spatial_pipeline module)

### Modified files
- `main.rs`: add `body_tracker` module, wire tracker into `AppStateInner`, add REST endpoints

### API Endpoints
- `GET /api/v1/tracking/bodies` — current tracked bodies with IDs and positions
- `GET /api/v1/tracking/zones` — configured zones
- `POST /api/v1/tracking/zones` — add/update zone
- `DELETE /api/v1/tracking/zones/{name}` — remove zone

## Consequences

- Persistent body IDs enable time-series analytics
- Zone transitions enable room-level automation (lights, HVAC)
- Kalman smoothing reduces jitter from frame-to-frame cluster centroid noise
- Track quality metric lets UI show confidence in each detection
- Foundation for activity history, fall detection, sleep detection, and dashboard floor plan features
