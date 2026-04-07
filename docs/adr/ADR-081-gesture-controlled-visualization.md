# ADR-081: Gesture-Controlled Data Visualization

- **Status**: Proposed
- **Date**: 2026-04-07
- **Deciders**: ruv
- **Relates to**: ADR-079 (Camera Ground-Truth Training), ADR-029 (RuvSense Gesture Recognition), ADR-072 (WiFlow Architecture), ADR-076 (CNN Spectrogram Embeddings)

## Context

RuView can now track 17 COCO keypoints at 92.9% PCK@20 (ADR-079) and detect gestures
via DTW template matching (ADR-029). These capabilities exist independently — pose
estimation produces skeleton coordinates, and the UI displays static charts. There is no
system that connects hand/arm movements to interactive data exploration.

Gesture-controlled visualization would let users manipulate charts and graphs by waving
their hands in front of the ESP32 sensing zone — no mouse, no touchscreen, no wearable.
This is particularly valuable for:

- **Lab/cleanroom** — gloved hands can't use touchscreens
- **Kitchen/workshop** — dirty or wet hands
- **Presentations** — stand back and gesture at projected dashboards
- **Accessibility** — motor impairments that make mouse use difficult
- **Digital signage** — public displays without touch hardware

### Why Camera + CSI Fusion

Camera alone can do gesture control (e.g., Leap Motion, MediaPipe Hands). CSI alone can
detect coarse gestures (ADR-029). The fusion provides:

| Modality | Strengths | Weaknesses |
|----------|-----------|-----------|
| Camera (MediaPipe Hands) | 21 hand landmarks, finger-level precision, 30fps | Requires line of sight, lighting dependent, privacy concern |
| CSI (ESP32) | Through-wall, works in dark, privacy-preserving, $9 | Coarse spatial resolution, no finger tracking |
| **Fusion** | **Finger precision near camera + coarse tracking everywhere** | Requires both sensors during training |

The fusion model trains on camera + CSI pairs (like ADR-079), then deploys in two modes:
1. **Camera-assisted** — full precision when camera is available
2. **CSI-only** — reduced but functional gesture control without camera

## Decision

Build a gesture-to-visualization control system that maps hand/arm movements to chart
interactions using fused camera + CSI input.

### Gesture Vocabulary

#### Navigation Gestures (arm-level, CSI-detectable)

| Gesture | Motion | Chart Action | CSI Feasibility |
|---------|--------|-------------|-----------------|
| **Swipe left** | Open hand sweeps left | Pan chart left / previous dataset | High — clear directional motion |
| **Swipe right** | Open hand sweeps right | Pan chart right / next dataset | High |
| **Swipe up** | Open hand sweeps up | Scroll up / zoom out | High |
| **Swipe down** | Open hand sweeps down | Scroll down / zoom in | High |
| **Push forward** | Palm pushes toward screen | Select / drill into data point | Medium — depth motion harder |
| **Pull back** | Hand pulls away from screen | Back / zoom out | Medium |
| **Circular CW** | Hand circles clockwise | Increase value / rotate view | Medium — temporal pattern |
| **Circular CCW** | Hand circles counter-clockwise | Decrease value / rotate back | Medium |
| **Hold still** | Hand stationary 2+ seconds | Hover / show tooltip | High — absence of motion |
| **Both hands apart** | Arms spread outward | Expand / zoom into selection | High — bilateral motion |
| **Both hands together** | Arms move inward | Collapse / zoom out | High |

#### Precision Gestures (finger-level, camera-required)

| Gesture | Motion | Chart Action | Sensor |
|---------|--------|-------------|--------|
| **Pinch zoom** | Thumb + index spread/close | Continuous zoom | Camera only |
| **Point** | Index finger extended | Cursor position on chart | Camera only |
| **Grab** | Close fist | Grab and drag data point | Camera only |
| **Thumb up** | Thumbs up | Confirm / approve | Camera only |
| **Thumb down** | Thumbs down | Reject / undo | Camera only |
| **Two-finger rotate** | Two fingers twist | Rotate 3D visualization | Camera only |
| **Finger slider** | Index finger moves along axis | Adjust parameter value | Camera only |

### Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                      Input Layer                                  │
│                                                                  │
│  ESP32 CSI (UDP 5005) ──→ CSI Gesture Detector (DTW + WiFlow)   │
│                               ↓                                  │
│  Webcam (MediaPipe Hands) ──→ Hand Landmark Tracker (21 joints) │
│                               ↓                                  │
│                    Gesture Fusion Engine                          │
│                    ├── CSI coarse: swipe/circle/hold             │
│                    ├── Camera fine: pinch/point/grab             │
│                    └── Confidence weighting by modality          │
└──────────────────────────────────────────────────────────────────┘
                               ↓
┌──────────────────────────────────────────────────────────────────┐
│                   Gesture Interpreter                             │
│                                                                  │
│  Raw gestures ──→ State Machine ──→ Chart Commands               │
│                                                                  │
│  States:                                                         │
│    IDLE ──(motion detected)──→ TRACKING                          │
│    TRACKING ──(gesture matched)──→ ACTING                        │
│    ACTING ──(gesture complete)──→ COOLDOWN                       │
│    COOLDOWN ──(500ms)──→ IDLE                                    │
│                                                                  │
│  Debounce: 200ms minimum gesture duration                        │
│  Cooldown: 500ms between consecutive gestures                    │
│  Confidence threshold: 0.7 for CSI, 0.9 for camera              │
└──────────────────────────────────────────────────────────────────┘
                               ↓
┌──────────────────────────────────────────────────────────────────┐
│                 Visualization Controller                          │
│                                                                  │
│  Chart Commands ──→ WebSocket ──→ UI                             │
│                                                                  │
│  Commands:                                                       │
│    { type: "pan",    dx: -0.1, dy: 0 }                          │
│    { type: "zoom",   factor: 1.2, center: [0.5, 0.5] }         │
│    { type: "select", x: 0.45, y: 0.62 }                        │
│    { type: "rotate", angle: 15 }                                │
│    { type: "slider", axis: "x", value: 0.73 }                  │
│    { type: "hover",  x: 0.45, y: 0.62 }                        │
│    { type: "back" }                                              │
│    { type: "confirm" }                                           │
│    { type: "reject" }                                            │
└──────────────────────────────────────────────────────────────────┘
                               ↓
┌──────────────────────────────────────────────────────────────────┐
│                    Visualization UI                               │
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │  Line Chart  │  │  Bar Chart  │  │  3D Scatter  │              │
│  │  (time       │  │  (category  │  │  (spatial    │              │
│  │   series)    │  │   compare)  │  │   data)      │              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │  Heatmap     │  │  Gauge      │  │  Spectrogram │              │
│  │  (CSI grid)  │  │  (vitals)   │  │  (frequency) │              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
│                                                                  │
│  Visual feedback: gesture cursor overlay + action indicator       │
│  Framework: D3.js / Observable Plot in existing UI               │
└──────────────────────────────────────────────────────────────────┘
```

### Gesture Detection Pipeline

#### CSI Gesture Detection (arm-level)

Extends the existing DTW gesture classifier (ADR-029) with WiFlow pose input:

```
CSI [35, 20] ──→ WiFlow lite ──→ 17 keypoints ──→ Extract arm features:
                                                    - Wrist velocity (dx/dt, dy/dt)
                                                    - Elbow angle (shoulder-elbow-wrist)
                                                    - Bilateral symmetry (left vs right)
                                                    - Motion energy (frame differencing)
                                                    ↓
                                              DTW template matching:
                                                    - 11 gesture templates
                                                    - Sliding window (1s)
                                                    - Top match + confidence
```

#### Camera Gesture Detection (finger-level)

Uses MediaPipe Hands (21 landmarks per hand, 30fps):

```
Webcam ──→ MediaPipe Hands ──→ 21 landmarks × 2 hands ──→ Extract:
                                                           - Finger states (extended/curled)
                                                           - Pinch distance (thumb-index)
                                                           - Grab state (all fingers curled)
                                                           - Point direction (index ray)
                                                           - Hand center velocity
                                                           ↓
                                                     Rule-based classifier:
                                                           - Pinch: thumb-index < 0.05
                                                           - Point: only index extended
                                                           - Grab: all fingers curled
                                                           - Thumbs up/down: thumb angle
```

#### Fusion Strategy

```
CSI confidence ──┐
                  ├──→ Weighted fusion ──→ Final gesture + confidence
Camera conf    ──┘

Rules:
  - If both agree: confidence = max(csi_conf, cam_conf) + 0.1 * min(csi_conf, cam_conf)
  - If only CSI: use CSI gesture, confidence *= 0.8
  - If only camera: use camera gesture, confidence *= 0.95
  - If conflict: prefer camera for fine gestures, CSI for coarse gestures
  - Minimum confidence for action: 0.6
```

### Chart Interaction Mapping

#### Line Chart (Time Series)

| Gesture | Action | Parameters |
|---------|--------|-----------|
| Swipe left/right | Pan time axis | dx proportional to swipe speed |
| Pinch zoom | Zoom time axis | Continuous, centered on hand position |
| Both hands apart/together | Zoom (CSI-only alternative) | Binary zoom in/out |
| Point | Show tooltip at nearest data point | x from index finger position |
| Hold still | Sticky tooltip | Duration-based activation |
| Swipe up/down | Switch dataset / Y-axis scale | Discrete steps |

#### Bar Chart (Category Comparison)

| Gesture | Action | Parameters |
|---------|--------|-----------|
| Swipe left/right | Navigate categories | One category per swipe |
| Point | Highlight bar | Nearest bar to finger X position |
| Push forward | Select bar for drill-down | Depth gesture |
| Grab + drag | Reorder bars | Camera-only |
| Circular | Sort ascending/descending | Direction determines order |

#### 3D Scatter Plot

| Gesture | Action | Parameters |
|---------|--------|-----------|
| Swipe left/right | Rotate Y axis | Angle proportional to speed |
| Swipe up/down | Rotate X axis | Angle proportional to speed |
| Two-finger rotate | Rotate Z axis | Camera-only |
| Pinch zoom | Zoom | Camera-only |
| Both hands apart | Zoom in (CSI alternative) | Binary |
| Point | Highlight nearest point | Ray-cast from finger direction |

#### Heatmap (CSI Grid)

| Gesture | Action | Parameters |
|---------|--------|-----------|
| Swipe | Pan view | dx, dy |
| Pinch | Zoom region | Center + scale |
| Hold | Show cell value | Position-based |
| Circular | Adjust color scale range | CW = expand, CCW = contract |

#### Gauge (Vital Signs)

| Gesture | Action | Parameters |
|---------|--------|-----------|
| Swipe left/right | Switch vital (HR → BR → SpO2) | Discrete |
| Circular CW | Set high alert threshold | Continuous |
| Circular CCW | Set low alert threshold | Continuous |
| Thumb up | Acknowledge alert | Binary |

### Visual Feedback

The UI provides real-time feedback so users know the system is tracking them:

```
┌────────────────────────────────────────────────┐
│                                                │
│   ┌──────────────────────────────────────┐     │
│   │                                      │     │
│   │        Chart (D3.js)                 │     │
│   │                                      │     │
│   │         ◉ ← gesture cursor           │     │
│   │                                      │     │
│   └──────────────────────────────────────┘     │
│                                                │
│   ┌──────────────────────────────────────┐     │
│   │ 🖐 Tracking  │ ← Swipe Right │ 0.87 │     │
│   └──────────────────────────────────────┘     │
│   Gesture bar: state | detected gesture | conf │
│                                                │
│   ┌────┐ ┌────┐ ┌────┐ ┌────┐                 │
│   │ CSI│ │ CAM│ │ FPS│ │MODE│                  │
│   │ ●  │ │ ●  │ │ 28 │ │FUSE│                  │
│   └────┘ └────┘ └────┘ └────┘                  │
│   Status: sensor health, framerate, mode        │
└────────────────────────────────────────────────┘
```

**Cursor types:**
- **Open hand** (🖐) — tracking, ready for gesture
- **Pointing** (👆) — hovering over data point
- **Grabbing** (✊) — dragging / selected
- **Pinching** (🤏) — zooming
- **Ghost cursor** — CSI-only mode (lower opacity, larger radius)

### Data Flow Protocol

WebSocket messages from gesture engine to UI:

```typescript
interface GestureEvent {
  type: 'gesture';
  gesture: 'swipe_left' | 'swipe_right' | 'swipe_up' | 'swipe_down'
         | 'pinch_zoom' | 'point' | 'grab' | 'hold' | 'circle_cw'
         | 'circle_ccw' | 'push' | 'pull' | 'spread' | 'contract'
         | 'thumb_up' | 'thumb_down';
  confidence: number;     // 0-1
  source: 'csi' | 'camera' | 'fusion';
  position?: [number, number];  // Normalized [0,1] hand position
  velocity?: [number, number];  // Hand velocity for proportional control
  param?: number;               // Gesture-specific parameter (pinch distance, rotation angle)
}

interface CursorEvent {
  type: 'cursor';
  x: number;              // 0-1 normalized
  y: number;              // 0-1 normalized
  state: 'tracking' | 'pointing' | 'grabbing' | 'pinching' | 'idle';
  hands: number;          // 0, 1, or 2
}

interface StatusEvent {
  type: 'status';
  csi_active: boolean;
  camera_active: boolean;
  mode: 'fusion' | 'csi_only' | 'camera_only';
  fps: number;
  gesture_count: number;  // Total gestures detected this session
}
```

### Training the CSI Gesture Model

Extends ADR-079's camera ground-truth pipeline:

```bash
# 1. Collect gesture training data (camera + CSI, 10 min)
#    Perform each gesture 20+ times with natural variation
python scripts/collect-gesture-gt.py --duration 600 --gestures all --preview

# 2. Label gesture segments (auto-detected from camera)
node scripts/label-gestures.js \
  --gt data/ground-truth/gestures-*.jsonl \
  --csi data/recordings/csi-*.jsonl

# 3. Train gesture classifier
node scripts/train-gesture-model.js \
  --data data/gestures/labeled-*.jsonl \
  --scale lite

# 4. Deploy
#    CSI-only mode: gestures detected from WiFlow keypoint motion
#    Fusion mode: camera adds finger-level precision
```

**Training data per gesture:** ~20 examples × 11 gestures = 220 labeled samples.
With augmentation (time warp, amplitude noise): ~1,000 effective samples.

### Optimization: Temporal Gesture Encoding

Instead of classifying single frames, encode gesture trajectories:

```
Keypoint sequence [T=30 frames, 1 second]:
  wrist_x[0..29], wrist_y[0..29],
  elbow_angle[0..29],
  hand_velocity[0..29]
                    ↓
1D CNN (k=5, d=[1,2,4]) → 64-dim gesture embedding
                    ↓
Nearest-neighbor to gesture templates (cosine distance)
                    ↓
Top gesture + confidence
```

This is lighter than DTW for real-time use and can be trained end-to-end with
the WiFlow backbone (shared TCN features).

## File Structure

```
scripts/
  collect-gesture-gt.py       # Camera + CSI gesture data collection
  label-gestures.js           # Auto-label gesture segments from camera
  train-gesture-model.js      # Train CSI gesture classifier
  gesture-server.js           # WebSocket gesture detection server

ui/
  components/
    GestureOverlay.js         # Cursor + feedback overlay
    GestureChart.js           # Gesture-controlled chart wrapper
    GestureStatus.js          # Sensor health bar
  services/
    gesture.service.js        # WebSocket client for gesture events
```

## Consequences

### Positive

- **Hands-free data exploration** — manipulate charts without touching anything
- **Works in dark/dirty/gloved conditions** — CSI-only mode needs no camera
- **Natural interaction** — swipe, pinch, point are intuitive
- **Builds on existing infrastructure** — WiFlow + DTW + MediaPipe all exist
- **Dual-mode deployment** — degrade gracefully from fusion to CSI-only
- **Low latency** — WiFlow inference is 0.79ms, gesture detection adds ~5ms

### Negative

- **Learning curve** — users must learn gesture vocabulary
- **False positives** — normal movement may trigger gestures (mitigated by state machine + cooldown)
- **CSI-only precision** — coarse gestures only without camera
- **Single-user** — multi-user gesture disambiguation is hard

### Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Gesture false positives from normal movement | Medium | High | State machine with IDLE→TRACKING threshold, 200ms debounce, 0.7 confidence gate |
| CSI gestures too coarse for chart control | Medium | Medium | Camera fallback for precision; CSI handles navigation-level gestures only |
| Latency > 100ms feels unresponsive | Low | High | WiFlow 0.79ms + gesture 5ms + WebSocket <10ms = ~16ms total |
| User fatigue ("gorilla arm") | Medium | Medium | Support seated gestures; small wrist movements, not full arm sweeps |
| MediaPipe Hands not detecting in low light | Medium | Low | CSI-only fallback; works in complete darkness |

## Implementation Plan

| Phase | Task | Effort | Dependencies |
|-------|------|--------|-------------|
| P1 | `gesture-server.js` — WebSocket server with camera hand tracking | 3 hrs | MediaPipe Hands model |
| P2 | Camera gesture classifier (rule-based from hand landmarks) | 2 hrs | P1 |
| P3 | CSI gesture classifier (WiFlow keypoints → DTW templates) | 3 hrs | WiFlow model (ADR-079) |
| P4 | Fusion engine (confidence-weighted merge) | 2 hrs | P2 + P3 |
| P5 | `GestureOverlay.js` — cursor + feedback UI component | 2 hrs | P1 |
| P6 | `GestureChart.js` — gesture-controlled D3 chart wrapper | 4 hrs | P4 + P5 |
| P7 | Gesture training data collection + model training | 2 hrs | P3 |
| P8 | Integration with existing sensing UI | 2 hrs | P6 |
| **Total** | | **~20 hrs** | |

## References

- MediaPipe Hands — Google's 21-landmark hand tracking (30fps, CPU)
- ADR-029 — RuvSense DTW gesture recognition
- ADR-079 — Camera ground-truth training pipeline (92.9% PCK@20)
- Leap Motion — commercial gesture controller (comparison point)
- SolidJS/D3 gesture interaction patterns
- "GestureWiFi" (IEEE 2023) — WiFi gesture recognition survey
