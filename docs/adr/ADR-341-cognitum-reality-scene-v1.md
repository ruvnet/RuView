# ADR 341: Cognitum Reality scene projection v1

Status: Proposed, proof of concept only. Date: 2026-11-22.

## Decision

Use existing RuView Sensor HAL Observation, WorldGraph, and Cognitum Spaces semantic API. Make WorldGraph the authoritative state and treat rendered output as a read-only projection. Do not reconstruct person coordinates from aggregate occupancy.

Scene v1 contains: version=1, siteId, source (live|synthetic|replay), sequence, timestampMs, entities. Entities contain id, kind, position [x,y,z] in metres, confidence [0,1], and evidence. Room entities may include size [width,height,depth].

The browser may render authorized spatial observations. The Cognitum Spaces semantic API provides occupancy/state only, not raw pose or geometry. A live person track requires authenticated RuView spatial evidence and provenance. Renderer output, including future diffusion video, must never feed back as sensor evidence.

## Implementation

See `examples/cognitum-reality/`. It is a browser-only synthetic proof of concept with strict provenance checks and no dependencies. The implementation is deliberately not represented as a live customer deployment.

RuView ADR-320 defines the common SensorHal Observation, ADR-139 describes WorldGraph, and ADR-340 describes the iPhone LiDAR bridge.

## Integration blockers

1. Both Cognitum MCP GET /v1/spaces and GET /v1/spatial/spaces returned HTTP 401 on 2026-11-22. Diagnose token issuer, audience, expiry, and credential type. No credential bypass.
2. Cognitum Spaces exposes semantic occupancy. Connecting poses requires a separate authorized edge projection with identity, privacy policy, track expiry, provenance and calibrated coordinates.
3. LiDAR real device tests and actual p95 latency are pending. No measured 150ms claim.
4. Diffusion rendering is a separate, optional research step, not currently part of this prototype.

## Security invariants

No cross-tenant state; reject replayed sequence numbers and stale timestamps; display source and evidence labels; keep raw CSI and recordings off the semantic API; generated visual output is non-authoritative; production requires authenticated bounded WSS, message limits, per-tenant authorization, backpressure and data retention controls.

## Acceptance gates

* npm test passes for the full local reference implementation.
* Browser shows synthetic provenance prominently.
* No person track appears solely from occupied:true.
* Physical LiDAR data and authenticated Spaces must pass integration and latency tests before enabling live mode.
