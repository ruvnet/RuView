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

### Visual Feedback: AR Camera Overlay

The primary view is the **live camera feed with AR overlays** — the person is visible
with charts, skeleton, and data rendered on top. This creates a "Minority Report" style
interface where you see yourself manipulating data in real-time.

```
┌──────────────────────────────────────────────────────────────┐
│                                                              │
│  ╔══════════════════════════════════════════════════════════╗ │
│  ║                                                          ║ │
│  ║     [Live Camera Feed — person visible]                  ║ │
│  ║                                                          ║ │
│  ║          ╭─────╮                                         ║ │
│  ║          │     │  ← skeleton overlay (17 keypoints)      ║ │
│  ║          ╰──┬──╯                                         ║ │
│  ║           ╱   ╲                                          ║ │
│  ║          ╱     ╲    ┌──────────────────────┐             ║ │
│  ║         │       │   │  CSI Amplitude Chart │             ║ │
│  ║         │  🖐→   │   │  ┌─╮ ╭─╮   ╭──╮     │             ║ │
│  ║         │       │   │  │ ╰─╯ ╰───╯  │     │             ║ │
│  ║          ╲     ╱    │  │             │     │             ║ │
│  ║           ╲   ╱     └──────────────────────┘             ║ │
│  ║            │ │      ↑ chart follows hand position        ║ │
│  ║           ╱   ╲                                          ║ │
│  ║          ╱     ╲                                         ║ │
│  ║                                                          ║ │
│  ╚══════════════════════════════════════════════════════════╝ │
│                                                              │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │                    LOWER THIRD                            │ │
│  │  ┌────┐                                                  │ │
│  │  │ pi │  RuView Sensing   HR: 72 BPM   BR: 16 BPM      │ │
│  │  │    │  v0.7.0           Presence: 1   Motion: 0.23    │ │
│  │  └────┘                                                  │ │
│  │  [logo]  [gesture: Swipe Right]  [CSI ●] [CAM ●] [28fps]│ │
│  └──────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

#### AR Overlay Layers (bottom to top)

| Layer | Content | Opacity | Update Rate |
|-------|---------|---------|-------------|
| 0 | Live camera feed (full frame) | 100% | 30fps |
| 1 | Skeleton overlay (17 keypoints + bones) | 70% | 30fps |
| 2 | Gesture cursor (hand position + state) | 90% | 30fps |
| 3 | Floating chart (anchored to hand/body region) | 85% | 30fps |
| 4 | Data labels + tooltips | 95% | On gesture |
| 5 | Lower third (RuView branding + vitals + status) | 95% | 1fps |

#### Floating Chart Placement

Charts are **anchored to the person's body** and follow movement:

```
Placement rules:
  - Default: chart floats to the right of the person's dominant hand
  - If hand moves left: chart slides to left side
  - Chart stays within frame bounds (never clips off-screen)
  - Multiple charts: stack vertically with 10% gap
  - Inactive charts: shrink to thumbnail and anchor near shoulder

Chart anchor point = wrist_position + offset(0.15, -0.1)  // right and slightly above hand
Chart size: 30% of frame width × 20% of frame height
```

#### Lower Third Design

The lower third bar provides persistent status in broadcast-style framing:

```
┌──────────────────────────────────────────────────────────────┐
│  ┌──────┐                                                    │
│  │  pi  │   RuView Sensing v0.7.0                            │
│  │      │   ──────────────────────────────────────────────   │
│  │ logo │   HR: 72 BPM  |  BR: 16 BPM  |  Persons: 1       │
│  └──────┘   Motion: Low  |  Gesture: Swipe Right  |  28fps  │
│             [CSI ●] [CAM ●] [FUSE]          PCK@20: 92.9%   │
└──────────────────────────────────────────────────────────────┘

Design:
  - Background: semi-transparent dark (#1a1a2e, 80% opacity)
  - Logo: RuView "pi" icon (32x32px), left-aligned
  - Text: white (#ffffff) primary, gray (#a0a0a0) secondary
  - Accent: teal (#00d4aa) for active indicators
  - Height: 15% of frame
  - Font: system monospace for data, sans-serif for labels
  - Divider: thin teal line separating logo from data
```

#### RuView Logo Placement

```
The "pi" logo appears in two contexts:

1. Lower third (persistent):
   - Position: bottom-left corner, 12px padding
   - Size: 32x32px
   - Style: white outline on dark background
   - Always visible during gesture mode

2. Watermark (optional):
   - Position: top-right corner, 8px padding
   - Size: 24x24px, 30% opacity
   - Style: subtle, doesn't interfere with data
```

#### Skeleton Rendering Style

```
Keypoint rendering:
  - Detected joints: teal circles (#00d4aa), radius 6px
  - Low-confidence joints: gray circles (#666), radius 4px
  - Active hand (gesturing): yellow highlight (#ffcc00), radius 8px, glow effect

Bone rendering:
  - Normal bones: teal lines (#00d4aa), 2px stroke
  - Active arm (gesturing): yellow lines (#ffcc00), 3px stroke, glow
  - Torso: slightly thicker (3px) to anchor the skeleton visually

Style: dark-theme friendly, high contrast against camera feed
```

**Cursor types:**
- **Open hand** — teal ring around wrist, rays extending from fingers
- **Pointing** — teal ray from index finger toward chart
- **Grabbing** — yellow fist icon, chart border highlights
- **Pinching** — two teal dots (thumb + index) with distance line
- **Ghost cursor** — CSI-only mode: larger, more diffuse circle (no finger detail)

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

### Optimization: ruvector-cnn Spectrogram Gesture Classification

Replace DTW template matching with a CNN operating on CSI spectrograms via the
`ruvector-cnn` WASM package (ADR-076). This treats each gesture as an image
classification problem on the CSI time-frequency representation.

#### Why CNN Over DTW

| | DTW (current, ADR-029) | CNN Spectrogram (proposed) |
|---|---|---|
| Input | 1D keypoint trajectories | 2D CSI spectrogram image |
| Features | Hand-crafted (wrist velocity, elbow angle) | Learned end-to-end |
| Robustness | Sensitive to speed variation | Warp-invariant (pooling layers) |
| Multi-scale | Single scale | Hierarchical (dilated convolutions) |
| Training | Template recording + DTW distance | Supervised from camera labels |
| New gestures | Record new template | Retrain (or few-shot with embedding) |
| Accuracy | ~85% (DTW literature) | ~95%+ (CNN on spectrograms, literature) |

#### Pipeline

```
CSI [N_subcarriers, T=30] (1-second window)
        ↓
Spectrogram transform: STFT per subcarrier
        → [N_sub, F_bins, T_bins] ≈ [35, 16, 15]
        ↓
Reshape to grayscale image: [35×16, 15] = [560, 15]
        → Resize to [64, 64] (bilinear)
        ↓
ruvector-cnn CnnEmbedder (WASM-accelerated)
        → 128-dim gesture embedding
        ↓
Classifier head: Linear(128 → 18 gestures) + softmax
        → gesture_id + confidence
```

#### ruvector-cnn Integration

The `@ruvector/cnn` WASM package provides:

```javascript
const { init, CnnEmbedder, InfoNCELoss } = require('@ruvector/cnn');
await init();

// Create embedder for 64x64 CSI spectrogram "images"
const embedder = new CnnEmbedder({
  inputSize: 64,
  embeddingDim: 128,
  normalize: true,
});

// Extract embedding from CSI spectrogram
const spectrogram = csiToSpectrogram(csiWindow);  // [64, 64] Uint8Array
const embedding = embedder.extract(spectrogram, 64, 64);

// Classify gesture via nearest-neighbor to trained templates
const gesture = classifyGesture(embedding, gestureTemplates);
```

#### Training with Contrastive + Classification

Two-phase training using ruvector-cnn's built-in losses:

**Phase 1: Contrastive embedding (unsupervised)**
```javascript
const loss = new InfoNCELoss(0.07);
// Same gesture performed at different speeds → positive pairs
// Different gestures → negative pairs
// Train CnnEmbedder to cluster same-gesture spectrograms
```

**Phase 2: Gesture classification (supervised)**
```javascript
// Linear classifier on frozen embeddings
// 18 gestures × 20 examples each = 360 labeled samples
// Camera auto-labels: MediaPipe Hands detects gesture type
```

#### Dual-Path Architecture

Run both CNN and DTW in parallel for maximum robustness:

```
CSI input ──┬──→ WiFlow → keypoints → DTW templates → gesture_A (conf_A)
            │
            └──→ Spectrogram → ruvector-cnn → embedding → classifier → gesture_B (conf_B)
            
Fusion: if gesture_A == gesture_B → conf = max(conf_A, conf_B) + 0.15
        if conflict → pick higher confidence
        if only one detects → use it at 0.8× confidence
```

This dual-path approach provides:
- **DTW** catches gestures the CNN might miss (novel variations)
- **CNN** provides higher accuracy for trained gesture types
- **Fusion** reduces false positives (both must agree for high-confidence)

### Optimization: Temporal Gesture Encoding

Alternative lightweight path for when ruvector-cnn WASM overhead matters
(e.g., ESP32 edge deployment):

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
