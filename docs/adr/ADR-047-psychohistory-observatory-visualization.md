# ADR-047: RuView Observatory — Immersive Three.js WiFi Sensing Visualization

## Status

Accepted (Implemented)

## Date

2026-03-04

## Context

The project has a functional tabbed dashboard UI (`ui/index.html`) with existing Three.js components (body model, gaussian splats, signal visualization, environment). While effective for monitoring, it lacks a cinematic, immersive visualization suitable for demonstrations and stakeholder presentations.

We need an immersive Three.js room-based visualization with practical WiFi sensing data overlays — human wireframe pose, dot-matrix body mass, vital signs HUD, signal field heatmap — powered by ESP32 CSI data (demo mode with live WebSocket path).

## Decision

### Standalone Page Architecture

`ui/observatory.html` is a standalone full-screen entry point, separate from the tabbed dashboard. Linked via "Observatory" nav tab in `ui/index.html`. No build step — vanilla JS modules with Three.js r160 via CDN importmap.

### Room-Based Visualization

Instead of abstract holographic panels, the observatory renders a practical room scene with:

| Element | Implementation | Data Source |
|---------|---------------|-------------|
| Human wireframe | COCO 17-keypoint skeleton, CylinderGeometry tube bones, SphereGeometry joints with glow halos | `persons[].position`, `vital_signs.breathing_rate_bpm` |
| Dot-matrix mist | 800 Points with per-particle alpha ShaderMaterial, body-shaped distribution | `persons[].position`, `persons[].motion_score` |
| Particle trail | 200 Points with age-based fade, emitted from moving person | `persons[].position`, `persons[].motion_score` |
| Signal field | 400 floor-level Points with green→amber color ramp | `signal_field.values` (20×20 grid) |
| WiFi waves | 5 wireframe SphereGeometry shells, AdditiveBlending, pulsing outward | Always-on animation from router position |
| Router | BoxGeometry body, 3 CylinderGeometry antennas, pulsing LED, PointLight | Static scene element |
| Room | GridHelper floor, BoxGeometry wireframe boundary, reflective MeshStandardMaterial floor, furniture (table, bed) | Static scene element |

### HUD Overlay

Glass-morphism HTML panels overlaid on the 3D canvas:

- **Left panel (Vital Signs):** Heart rate (BPM), respiration (RPM), confidence (%) with animated bars
- **Right panel (WiFi Signal):** RSSI, variance, motion power, person count, 2D RSSI sparkline, presence state badge, fall alert
- **Top-right:** Data source badge (DEMO/LIVE), scenario badge, FPS counter, settings gear
- **Bottom:** Capability bar (Pose Estimation, Vital Monitoring, Presence Detection)
- **Bottom-right:** Keyboard shortcut hints

### Settings Dialog (4 Tabs)

Full customization with localStorage persistence and JSON export:

| Tab | Controls |
|-----|----------|
| **Rendering** | Bloom strength/radius/threshold, exposure, vignette, film grain, chromatic aberration |
| **Wireframe** | Bone thickness, joint size, glow intensity, particle trail, wireframe color, joint color, aura opacity |
| **Scene** | Signal field opacity, WiFi wave intensity, room brightness, floor reflection, FOV, orbit speed, grid toggle, room boundary toggle |
| **Data** | Scenario selector (auto-cycle or fixed), cycle speed, data source (demo/WebSocket), WS URL, reset camera, export settings |

### Demo-First with Live Data Path

Four auto-cycling scenarios (30s default, configurable) with 2s cosine crossfade:

| Scenario | Description |
|----------|-------------|
| `empty_room` | Low variance, no presence, flat amplitude, stable RSSI -45dBm |
| `single_breathing` | 1 person, breathing 16 BPM, HR 72 BPM, sinusoidal subcarrier modulation |
| `two_walking` | 2 persons, high motion, Doppler-like shifts, moving signal field peaks |
| `fall_event` | 2s variance spike at t=5s, then stillness, fall flag, confidence drop |

Data contract matches `SensingUpdate` struct from the Rust sensing server. Live WebSocket connection configurable in settings dialog.

### Post-Processing Pipeline

EffectComposer chain: RenderPass → UnrealBloomPass → custom VignetteShader

- **UnrealBloom:** strength 1.0, radius 0.5, threshold 0.25 (configurable)
- **VignetteShader:** warm shadow shift, edge chromatic aberration, film grain
- **Adaptive quality:** Auto-degrades when FPS < 25, restores when FPS > 55

### RuView Foundation Color Palette

| Role | Color | Hex |
|------|-------|-----|
| Background | Deep dark | `#080c14` |
| Primary wireframe | Green glow | `#00d878` |
| Warm accent | Amber | `#ffb020` |
| Signal | Blue | `#2090ff` |
| Heart / joints | Red | `#ff4060` |
| Alert | Crimson | `#ff3040` |

### Technology Choices

| Decision | Rationale |
|----------|-----------|
| Standalone page vs tab | Full-screen immersion, independent loading |
| Room-based vs abstract panels | Practical spatial context for WiFi sensing data |
| Vanilla JS + CDN, no build step | Matches existing `ui/` pattern, served as static files by Axum |
| Custom ShaderMaterial for mist | Per-particle alpha, body-shaped distribution, AdditiveBlending |
| CylinderGeometry tube bones | Visible at any zoom vs thin Line geometry |
| COCO 17-keypoint skeleton | Standard pose format, 16 bone connections |
| localStorage settings | Persistent customization without server round-trip |
| Adaptive quality | 3 levels, auto-switches based on FPS measurement |

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `A` | Toggle autopilot orbit |
| `D` | Cycle demo scenario |
| `F` | Toggle FPS counter |
| `S` | Open/close settings |
| `Space` | Pause/resume data |

## Files

| File | Purpose |
|------|---------|
| `ui/observatory.html` | Full-screen entry point with HUD overlay + settings dialog |
| `ui/observatory/js/main.js` | Scene orchestrator (~1,100 lines): room, wireframe, mist, trails, settings, HUD, animation loop |
| `ui/observatory/js/demo-data.js` | 4 scenarios with cosine crossfade, setScenario/setCycleDuration API |
| `ui/observatory/js/nebula-background.js` | Procedural fBM nebula + star field background sphere |
| `ui/observatory/js/post-processing.js` | EffectComposer: UnrealBloom + VignetteShader (chromatic, grain, warmth) |
| `ui/observatory/css/observatory.css` | Foundation color scheme, glass-morphism panels, settings dialog, responsive |
| `ui/index.html` | Modified: added Observatory nav link |

## Consequences

### Positive
- Standalone page does not affect existing dashboard stability
- Demo-first allows offline presentations without hardware
- Same `SensingUpdate` contract enables seamless live WebSocket switch
- Room-based visualization provides intuitive spatial context for WiFi sensing
- Dot-matrix mist gives visual body mass without occluding wireframe
- Full settings customization without code changes (localStorage + JSON export)
- Adaptive quality ensures usability on weaker hardware
- ~20 draw calls keeps performance well within budget

### Negative
- Additional static files served by Axum (minimal overhead)
- Three.js r160 loaded from CDN (no build step, matches existing pattern)
- Settings persistence is per-browser (localStorage, not synced)

### Risks
- CDN dependency for Three.js (mitigated: can vendor locally if needed)
- Post-processing may not work on very old GPUs (mitigated: adaptive quality disables bloom)

## References

- ADR-045: AMOLED display support
- ADR-046: Android TV / Armbian deployment
- Existing `ui/components/scene.js` — Three.js scene pattern
- Existing `ui/components/gaussian-splats.js` — ShaderMaterial pattern
- Existing `ui/services/sensing.service.js` — WebSocket data contract
