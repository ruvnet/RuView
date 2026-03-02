# ADR-035: Live Sensing UI Accuracy & Data Source Transparency

## Status
Accepted

## Date
2026-03-02

## Context

Issue #86 reported that the live demo shows a static/barely-animated stick figure and the sensing page displays inaccurate data, despite a working ESP32 sending real CSI frames. Investigation revealed three root causes:

1. **Docker defaults to `--source simulated`** — even with a real ESP32 connected, the server generates synthetic sine-wave data instead of reading UDP frames.
2. **Live demo pose is analytically computed** — `derive_pose_from_sensing()` generates keypoints using `sin(tick)` math unrelated to actual signal content. No trained `.rvf` model is loaded by default.
3. **Sensing feature extraction is oversimplified** — the server uses single-frame thresholds for motion detection and has no temporal analysis (breathing FFT, sliding window variance, frame history).
4. **No data source indicator** — users cannot tell whether they are seeing real or simulated data.

## Decision

### 1. Docker: Auto-detect data source
- Default `CSI_SOURCE` changed from `simulated` to `auto`.
- `auto` probes UDP port 5005 for an ESP32; falls back to simulation if none found.
- Users override via `CSI_SOURCE=esp32 docker-compose up`.

### 2. Signal-responsive pose derivation
- `derive_pose_from_sensing()` now reads actual sensing features:
  - `motion_band_power` drives limb splay and walking gait detection (> 0.55).
  - `breathing_band_power` drives torso expansion/contraction phased to breathing rate.
  - `variance` seeds per-joint noise so the skeleton moves independently.
  - `dominant_freq_hz` drives lateral torso lean.
  - `change_points` add burst jitter to extremity keypoints.
- Tick rate reduced from 500ms to 100ms (2 fps → 10 fps).
- `pose_source` field (`signal_derived` | `model_inference`) added to every WebSocket frame.

### 3. Temporal feature extraction
- 100-frame circular buffer (`VecDeque`) added to `AppStateInner`.
- Per-subcarrier temporal variance via Welford-style accumulation.
- Breathing rate estimation via 9-candidate Goertzel filter bank (0.1–0.5 Hz) with 3x SNR gate.
- Frame-to-frame L2 motion score replaces single-frame amplitude thresholds.
- Signal quality metric: SNR-based (RSSI − noise floor) blended with temporal stability.
- Signal field driven by subcarrier variance spatial mapping instead of fixed animation.

### 4. Data source transparency in UI
- **Sensing tab**: Banner showing "LIVE - ESP32" (green), "RECONNECTING..." (yellow), or "SIMULATED DATA" (red).
- **Live Demo tab**: "Estimation Mode" badge showing "Signal-Derived" (green) or "Model Inference" (blue).
- **Setup Guide** panel explaining what each ESP32 count provides (1x: presence/breathing, 3x: localization, 4x+: full pose with trained model).
- Simulation fallback delayed from immediate to 5 failed reconnect attempts (~30s).

## Consequences

### Positive
- Users with real ESP32 hardware get real data by default (auto-detect).
- Simulated data is clearly labeled — no more confusion about data authenticity.
- Pose skeleton visually responds to actual signal changes (motion, breathing, variance).
- Feature extraction produces physiologically meaningful metrics (breathing rate via Goertzel, temporal motion detection).
- Setup guide manages expectations about what each hardware configuration provides.

### Negative
- Signal-derived pose is still an approximation, not neural network inference. Per-limb tracking requires a trained `.rvf` model + 4+ ESP32 nodes.
- Goertzel filter bank adds ~O(9×N) computation per frame (negligible at 100 frames).
- Users with only 1 ESP32 may still be disappointed that arm tracking doesn't work — but the UI now explains why.

## Files Changed
- `docker/Dockerfile.rust` — `CSI_SOURCE=auto` env, shell entrypoint for variable expansion
- `docker/docker-compose.yml` — `CSI_SOURCE=${CSI_SOURCE:-auto}`, shell command string
- `wifi-densepose-sensing-server/src/main.rs` — frame history buffer, Goertzel breathing estimation, temporal motion score, signal-driven pose derivation, pose_source field, 100ms tick default
- `ui/services/sensing.service.js` — `dataSource` state, delayed simulation fallback, `_simulated` marker
- `ui/components/SensingTab.js` — data source banner, "About This Data" card
- `ui/components/LiveDemoTab.js` — estimation mode badge, setup guide panel
- `ui/style.css` — banner, badge, and guide panel styles

## References
- Issue: https://github.com/ruvnet/wifi-densepose/issues/86
- ADR-029: RuvSense multistatic sensing mode (proposed — full pipeline integration)
- ADR-014: SOTA signal processing
