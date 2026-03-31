# ADR-058: Dual-Modal WASM Browser Pose Estimation - Live Video + WiFi CSI Fusion

- **Status**: Proposed
- **Date**: 2026-03-12
- **Deciders**: ruv
- **Tags**: wasm, browser, cnn, pose-estimation, ruvector, video, multimodal, fusion

## Context

WiFi-DensePose estimates human poses from WiFi CSI (Channel State Information).
The `ruvector-cnn` crate provides a pure Rust CNN (MobileNet-V3) with WASM bindings.
Both modalities exist independently - what's missing is **fusing live webcam video
with WiFi CSI** in a single browser demo to achieve robust pose estimation that
works even when one modality degrades (occlusion, signal noise, poor lighting).

Existing assets:

1. **`wifi-densepose-wasm`** - CSI signal processing compiled to WASM
2. **`wifi-densepose-sensing-server`** - Axum server streaming live CSI via WebSocket
3. **`ruvector-cnn`** - Pure Rust CNN with MobileNet-V3 backbones, SIMD, contrastive learning
4. **`ruvector-cnn-wasm`** - wasm-bindgen bindings: `WasmCnnEmbedder`, `SimdOps`, `LayerOps`, contrastive losses
5. **`vendor/ruvector/examples/wasm-vanilla/`** - Reference vanilla JS WASM example

Research shows multi-modal fusion (camera + WiFi) significantly outperforms either alone:
- Camera fails under occlusion, poor lighting, privacy constraints
- WiFi CSI fails with signal noise, multipath, low spatial resolution
- Fusion compensates: WiFi provides through-wall coverage, camera provides fine-grained detail

## Decision

Build a **dual-modal browser demo** at `examples/wasm-browser-pose/` that:

1. Captures **live webcam video** via `getUserMedia` API
2. Receives **live WiFi CSI** via WebSocket from the sensing server
3. Processes **both streams** through separate CNN pipelines in `ruvector-cnn-wasm`
4. **Fuses embeddings** with learned attention weights for combined pose estimation
5. Renders **video overlay** with skeleton + WiFi confidence heatmap on Canvas
6. Runs entirely in the browser - all inference client-side via WASM

### Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│  Browser                                                         │
│                                                                  │
│  ┌────────────┐    ┌────────────────┐    ┌───────────────────┐   │
│  │ getUserMedia│───▶│ Video Frame    │───▶│ CNN WASM          │   │
│  │ (Webcam)   │    │ Capture        │    │ (Visual Embedder) │   │
│  └────────────┘    │ 224×224 RGB    │    │ → 512-dim         │   │
│                    └────────────────┘    └────────┬──────────┘   │
│                                                   │              │
│                                          visual_embedding        │
│                                                   │              │
│                                            ┌──────▼──────┐       │
│  ┌────────────┐    ┌────────────────┐      │             │       │
│  │ WebSocket  │───▶│ CSI WASM       │      │  Attention  │       │
│  │ Client     │    │ (densepose-    │      │  Fusion     │       │
│  │            │    │  wasm)         │      │  Module     │       │
│  └────────────┘    └───────┬────────┘      │             │       │
│                            │               └──────┬──────┘       │
│                    ┌───────▼────────┐             │              │
│                    │ CNN WASM       │      fused_embedding       │
│                    │ (CSI Embedder) │             │              │
│                    │ → 512-dim      │      ┌──────▼──────┐       │
│                    └───────┬────────┘      │ Pose        │       │
│                            │               │ Decoder     │       │
│                     csi_embedding           │ → 17 kpts   │       │
│                            │               └──────┬──────┘       │
│                            └──────────────────────┘              │
│                                                   │              │
│                    ┌──────────────┐         ┌─────▼──────┐       │
│                    │ Video Canvas │◀────────│ Overlay    │       │
│                    │ + Skeleton   │         │ Renderer   │       │
│                    │ + Heatmap    │         └────────────┘       │
│                    └──────────────┘                               │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
         ▲                                     ▲
         │ getUserMedia                        │ WebSocket
         │ (camera)                            │ (ws://host:3030/ws/csi)
         │                                     │
    ┌────┴────┐                        ┌───────┴─────────┐
    │ Webcam  │                        │ Sensing Server   │
    └─────────┘                        └─────────────────┘
```

### Dual Pipeline Design

Two parallel CNN pipelines run on each frame tick (~30 FPS):

| Pipeline | Input | Preprocessing | CNN Config | Output |
|----------|-------|---------------|------------|--------|
| **Visual** | Webcam frame (640×480) | Resize to 224×224 RGB, ImageNet normalize | MobileNet-V3 Small, 512-dim | Visual embedding |
| **CSI** | CSI frame (ADR-018 binary) | Amplitude/phase/delta → 224×224 pseudo-RGB | MobileNet-V3 Small, 512-dim | CSI embedding |

Both use the same `WasmCnnEmbedder` but with separate instances and weight sets.

### Fusion Strategy

**Learned attention-weighted fusion** combines the two 512-dim embeddings:

```javascript
// Attention fusion: learn which modality to trust per-dimension
// α ∈ [0,1]^512 - attention weights (shipped as JSON, trained offline)
// visual_emb, csi_emb ∈ R^512

function fuseEmbeddings(visual_emb, csi_emb, attention_weights) {
    const fused = new Float32Array(512);
    for (let i = 0; i < 512; i++) {
        const α = attention_weights[i];
        fused[i] = α * visual_emb[i] + (1 - α) * csi_emb[i];
    }
    return fused;
}
```

**Dynamic confidence gating** adjusts fusion based on signal quality:

| Condition | Behavior |
|-----------|----------|
| Good video + good CSI | Balanced fusion (α ≈ 0.5) |
| Poor lighting / occlusion | CSI-dominant (α → 0, WiFi takes over) |
| CSI noise / no ESP32 | Video-dominant (α → 1, camera only) |
| Video-only mode (no WiFi) | α = 1.0, pure visual CNN pose estimation |
| CSI-only mode (no camera) | α = 0.0, pure WiFi pose estimation |

Quality detection:
- **Video quality**: Frame brightness variance (dark = low quality), motion blur score
- **CSI quality**: Signal-to-noise ratio from `wifi-densepose-wasm`, coherence gate output

### CSI-to-Image Encoding

CSI data encoded as 3-channel pseudo-image for the CSI CNN pipeline:

| Channel | Data | Normalization |
|---------|------|---------------|
| R | CSI amplitude (subcarrier × time window) | Min-max to [0, 255] |
| G | CSI phase (unwrapped, subcarrier × time window) | Min-max to [0, 255] |
| B | Temporal difference (frame-to-frame Δ amplitude) | Abs, min-max to [0, 255] |

### Video Processing

Webcam frames processed through standard ImageNet pipeline:

```javascript
// Capture frame from video element
const frame = captureVideoFrame(videoElement, 224, 224); // Returns Uint8Array RGB

// ImageNet normalization happens inside WasmCnnEmbedder.extract()
const visual_embedding = visual_embedder.extract(frame, 224, 224);
```

### Pose Keypoint Mapping

17 COCO-format keypoints decoded from the fused 512-dim embedding:

```
 0: nose          1: left_eye       2: right_eye
 3: left_ear      4: right_ear      5: left_shoulder
 6: right_shoulder 7: left_elbow    8: right_elbow
 9: left_wrist   10: right_wrist   11: left_hip
12: right_hip    13: left_knee     14: right_knee
15: left_ankle   16: right_ankle
```

Each keypoint decoded as (x, y, confidence) = 51 values from the 512-dim embedding
via a learned linear projection.

### Operating Modes

The demo supports three modes, selectable in the UI:

| Mode | Video | CSI | Fusion | Use Case |
|------|-------|-----|--------|----------|
| **Dual (default)** | ✅ | ✅ | Attention-weighted | Best accuracy, full demo |
| **Video Only** | ✅ | ❌ | α = 1.0 | No ESP32 available, quick demo |
| **CSI Only** | ❌ | ✅ | α = 0.0 | Privacy mode, through-wall sensing |

**Video Only mode works without any hardware** - just a webcam - making the demo
instantly accessible for anyone wanting to try it.

### File Layout

```
examples/wasm-browser-pose/
├── index.html                  # Single-page app (vanilla JS, no bundler)
├── js/
│   ├── app.js                  # Main entry, mode selection, orchestration
│   ├── video-capture.js        # getUserMedia, frame extraction, quality detection
│   ├── csi-processor.js        # WebSocket CSI client, frame parsing, pseudo-image encoding
│   ├── fusion.js               # Attention-weighted embedding fusion, confidence gating
│   ├── pose-decoder.js         # Fused embedding → 17 keypoints
│   └── canvas-renderer.js      # Video overlay, skeleton, CSI heatmap, confidence bars
├── data/
│   ├── visual-weights.json     # Visual CNN → embedding projection (placeholder until trained)
│   ├── csi-weights.json        # CSI CNN → embedding projection (placeholder until trained)
│   ├── fusion-weights.json     # Attention fusion α weights (512 values)
│   └── pose-weights.json       # Fused embedding → keypoint projection
├── css/
│   └── style.css               # Dark theme UI styling
├── pkg/                        # Built WASM packages (gitignored, built by script)
│   ├── wifi_densepose_wasm/
│   └── ruvector_cnn_wasm/
├── build.sh                    # wasm-pack build script for both packages
└── README.md                   # Setup and usage instructions
```

### Build Pipeline

```bash
#!/bin/bash
# build.sh - builds both WASM packages into pkg/

set -e

# Build wifi-densepose-wasm (CSI processing)
wasm-pack build ../../rust-port/wifi-densepose-rs/crates/wifi-densepose-wasm \
  --target web --out-dir "$(pwd)/pkg/wifi_densepose_wasm" --no-typescript

# Build ruvector-cnn-wasm (CNN inference for both video and CSI)
wasm-pack build ../../vendor/ruvector/crates/ruvector-cnn-wasm \
  --target web --out-dir "$(pwd)/pkg/ruvector_cnn_wasm" --no-typescript

echo "Build complete. Serve with: python3 -m http.server 8080"
```

### UI Layout

```
┌─────────────────────────────────────────────────────────┐
│  WiFi-DensePose - Live Dual-Modal Pose Estimation       │
│  [Dual Mode ▼]  [⚙ Settings]          FPS: 28  ◉ Live  │
├───────────────────────────┬─────────────────────────────┤
│                           │                             │
│   ┌───────────────────┐   │   ┌───────────────────┐     │
│   │                   │   │   │                   │     │
│   │  Video + Skeleton │   │   │  CSI Heatmap      │     │
│   │  Overlay          │   │   │  (amplitude ×     │     │
│   │  (main canvas)    │   │   │   subcarrier)     │     │
│   │                   │   │   │                   │     │
│   └───────────────────┘   │   └───────────────────┘     │
│                           │                             │
├───────────────────────────┴─────────────────────────────┤
│  Fusion Confidence: ████████░░ 78%                      │
│  Video: ██████████ 95%  │  CSI: ██████░░░░ 61%          │
├─────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────┐    │
│  │  Embedding Space (2D projection)                 │    │
│  │     ·  ·    ·                                    │    │
│  │   · · ·  ·    · ·    (color = pose cluster)     │    │
│  │      ·  · · ·                                    │    │
│  └─────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────┤
│  Latency: Video 12ms │ CSI 8ms │ Fusion 1ms │ Total 21ms│
│  [▶ Record]  [📷 Snapshot]  [Confidence: ████ 0.6]      │
└─────────────────────────────────────────────────────────┘
```

### WASM Module Structure

| Package | Source Crate | Provides | Size (est.) |
|---------|-------------|----------|-------------|
| `wifi_densepose_wasm` | `wifi-densepose-wasm` | CSI frame parsing, signal processing, feature extraction | ~200KB |
| `ruvector_cnn_wasm` | `ruvector-cnn-wasm` | `WasmCnnEmbedder` (×2 instances), `SimdOps`, `LayerOps`, contrastive losses | ~150KB |

Two `WasmCnnEmbedder` instances are created - one for video frames, one for CSI pseudo-images.
They share the same WASM module but have independent state.

### Browser API Requirements

| API | Purpose | Required | Fallback |
|-----|---------|----------|----------|
| `getUserMedia` | Webcam capture | For video mode | CSI-only mode |
| WebAssembly | CNN inference | Yes | None (hard requirement) |
| WASM SIMD128 | Accelerated inference | No | Scalar fallback (~2× slower) |
| WebSocket | CSI data stream | For CSI mode | Video-only mode |
| Canvas 2D | Rendering | Yes | None |
| `requestAnimationFrame` | Render loop | Yes | `setTimeout` fallback |
| ES Modules | Code organization | Yes | None |

Target: Chrome 89+, Firefox 89+, Safari 15+, Edge 89+

### Performance Budget

| Stage | Target Latency | Notes |
|-------|---------------|-------|
| Video frame capture + resize | <3ms | `drawImage` to offscreen canvas |
| Video CNN embedding | <15ms | 224×224 RGB → 512-dim |
| CSI receive + parse | <2ms | Binary WebSocket message |
| CSI pseudo-image encoding | <3ms | Amplitude/phase/delta channels |
| CSI CNN embedding | <15ms | 224×224 pseudo-RGB → 512-dim |
| Attention fusion | <1ms | Element-wise weighted sum |
| Pose decoding | <1ms | Linear projection |
| Canvas overlay render | <3ms | Video + skeleton + heatmap |
| **Total (dual mode)** | **<33ms** | **30 FPS capable** |
| **Total (video only)** | **<22ms** | **45 FPS capable** |

Note: Video and CSI CNN pipelines can run in parallel using Web Workers,
reducing dual-mode latency to ~max(15, 15) + 5 = ~20ms (50 FPS).

### Contrastive Learning Integration

The demo optionally shows real-time contrastive learning in the browser:

- **InfoNCE loss** (`WasmInfoNCELoss`): Compare video vs CSI embeddings for the same pose - trains cross-modal alignment
- **Triplet loss** (`WasmTripletLoss`): Push apart different poses, pull together same pose across modalities
- **SimdOps**: Accelerated dot products for real-time similarity computation
- **Embedding space panel**: Live 2D projection shows video and CSI embeddings converging when viewing the same person

### Relationship to Existing Crates

| Existing Crate | Role in This Demo |
|---------------|-------------------|
| `ruvector-cnn-wasm` | CNN inference for **both** video frames and CSI pseudo-images |
| `wifi-densepose-wasm` | CSI frame parsing and signal processing |
| `wifi-densepose-sensing-server` | WebSocket CSI data source |
| `wifi-densepose-core` | ADR-018 frame format definitions |
| `ruvector-cnn` | Underlying MobileNet-V3, layers, contrastive learning |

No new Rust crates are needed. The example is pure HTML/JS consuming existing WASM packages.

## Consequences

### Positive

- **Instant demo**: Video-only mode works with just a webcam - no ESP32 needed
- **Multi-modal showcase**: Demonstrates camera + WiFi fusion, the core innovation of the project
- **Graceful degradation**: Works with video-only, CSI-only, or both
- **Through-wall capability**: CSI mode shows pose estimation where cameras cannot reach
- **Zero-install**: Anyone with a browser can try it
- **Training data collection**: Can record paired (video, CSI) data for offline model training
- **Reusable**: JS modules embed directly in the Tauri desktop app's webview

### Negative

- **Model weights**: Requires offline-trained weights for visual CNN, CSI CNN, fusion, and pose decoder (~200KB total JSON)
- **WASM size**: Two WASM modules total ~350KB (acceptable)
- **No GPU**: CPU-only WASM inference; adequate at 224×224 but limits resolution scaling
- **Camera privacy**: Video mode requires camera permission (mitigated: CSI-only mode available)
- **Two CNN instances**: Memory footprint doubles vs single-modal (~10MB total, acceptable for desktop browsers)

### Risks

- **Cross-modal alignment**: Video and CSI embeddings must be trained jointly for fusion to work;
  without proper training, fusion may be worse than either modality alone
- **Latency on mobile**: Dual CNN on mobile browsers may exceed 33ms; implement automatic quality reduction
- **WebSocket drops**: Network jitter → CSI frame gaps; buffer last 3 frames, interpolate missing data

## Implementation Plan

1. **Phase 1 - Scaffold**: File layout, build.sh, index.html shell, mode selector UI
2. **Phase 2 - Video pipeline**: getUserMedia → frame capture → CNN embedding → basic pose display
3. **Phase 3 - CSI pipeline**: WebSocket client → CSI parsing → pseudo-image → CNN embedding
4. **Phase 4 - Fusion**: Attention-weighted combination, confidence gating, mode switching
5. **Phase 5 - Pose decoder**: Linear projection with placeholder weights → 17 keypoints
6. **Phase 6 - Overlay renderer**: Video canvas with skeleton overlay, CSI heatmap panel
7. **Phase 7 - Training**: Use `wifi-densepose-train` to generate real weights for both CNNs + fusion + decoder
8. **Phase 8 - Contrastive demo**: Embedding space visualization, cross-modal similarity display
9. **Phase 9 - Web Workers**: Move CNN inference to workers for parallel video + CSI processing
10. **Phase 10 - Polish**: Recording, snapshots, adaptive quality, mobile optimization

## Alternatives Considered

### 1. CSI-Only (No Video)
Rejected: Misses the opportunity to show multi-modal fusion and makes the demo less
accessible (requires ESP32 hardware). Video-only mode as a fallback is strictly better.

### 2. Server-Side Video Inference
Rejected: Adds latency, requires webcam stream upload (privacy concern), and defeats
the WASM-first architecture. All inference must be client-side.

### 3. TensorFlow.js for Video, ruvector-cnn-wasm for CSI
Rejected: Would require two different ML frameworks. Using `ruvector-cnn-wasm` for both
keeps a single WASM module, unified embedding space, and simpler fusion.

### 4. Pre-recorded Video Demo
Rejected: Live webcam input is far more compelling for demonstrations.
Pre-recorded mode can be added as a secondary option.

### 5. React/Vue Framework
Rejected: Adds build tooling. Vanilla JS + ES modules keeps the demo self-contained.

## References

- [ADR-018: Binary CSI Frame Format](ADR-018-binary-csi-frame-format.md)
- [ADR-024: Contrastive CSI Embedding / AETHER](ADR-024-contrastive-csi-embedding.md)
- [ADR-055: Integrated Sensing Server](ADR-055-integrated-sensing-server.md)
- `vendor/ruvector/crates/ruvector-cnn/src/lib.rs` - CNN embedder implementation
- `vendor/ruvector/crates/ruvector-cnn-wasm/src/lib.rs` - WASM bindings
- `vendor/ruvector/examples/wasm-vanilla/index.html` - Reference vanilla JS WASM pattern
- Person-in-WiFi: Fine-grained Person Perception using WiFi (ICCV 2019) - camera+WiFi fusion precedent
- WiPose: Multi-Person WiFi Pose Estimation (TMC 2022) - cross-modal embedding approach
