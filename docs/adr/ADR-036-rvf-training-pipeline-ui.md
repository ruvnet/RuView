# ADR-036: RVF Model Training Pipeline & UI Integration

## Status
Proposed

## Date
2026-03-02

## Context

The wifi-densepose system currently operates in **signal-derived** mode — `derive_pose_from_sensing()` maps aggregate CSI features (motion power, breathing rate, variance) to keypoint positions using deterministic math. This gives whole-body presence and gross motion but cannot track individual limbs.

The infrastructure for **model inference** mode exists but is disconnected:

1. **RVF container format** (`rvf_container.rs`, 1,102 lines) — a 64-byte-aligned binary format supporting model weights (`SEG_VEC`), metadata (`SEG_MANIFEST`), quantization (`SEG_QUANT`), LoRA profiles (`SEG_LORA`), contrastive embeddings (`SEG_EMBED`), and witness audit trails (`SEG_WITNESS`). Builder and reader are fully implemented with CRC32 integrity checks.

2. **Training crate** (`wifi-densepose-train`) — AdamW optimizer, PCK@0.2/OKS metrics, LR scheduling with warmup, early stopping, CSV logging, and checkpoint export. Supports `CsiDataset` trait with planned MM-Fi (114→56 subcarrier interpolation) and Wi-Pose (30→56 zero-pad) loaders per ADR-015.

3. **NN inference crate** (`wifi-densepose-nn`) — ONNX Runtime backend with CPU/GPU support, dynamic tensor shapes, thread-safe `OnnxBackend` wrapper, model info inspection, and warmup.

4. **Sensing server CLI** (`--model <path>`, `--train`, `--pretrain`, `--embed`) — flags exist for model loading, training mode, and embedding extraction, but the end-to-end path from raw CSI → trained `.rvf` → live inference is not wired together.

5. **UI gaps** — No model management, training progress visualization, LoRA profile switching, or embedding inspection. The Settings panel lacks model configuration. The Live Demo has no way to load a trained model or compare signal-derived vs model-inference output side-by-side.

### What users need

- A way to **collect labeled CSI data** from their own environment (self-supervised or teacher-student from camera).
- A way to **train an .rvf model** from collected data without leaving the UI.
- A way to **load and switch models** in the live demo, seeing the quality improvement.
- Visibility into **training progress** (loss curves, validation PCK, early stopping).
- **Environment adaptation** via LoRA profiles (office → home → warehouse) without full retraining.

## Decision

### Phase 1: Data Collection & Self-Supervised Pretraining

#### 1.1 CSI Recording API
Add REST endpoints to the sensing server:
```
POST /api/v1/recording/start   { duration_secs, label?, session_name }
POST /api/v1/recording/stop
GET  /api/v1/recording/list
GET  /api/v1/recording/download/:id
DELETE /api/v1/recording/:id
```
- Records raw CSI frames + extracted features to `.csi.jsonl` files.
- Optional camera-based label overlay via teacher model (Detectron2/MediaPipe on client).
- Each recording session tagged with environment metadata (room dimensions, node positions, AP count).

#### 1.2 Contrastive Pretraining (ADR-024 Phase 1)
- Self-supervised NT-Xent loss learns a 128-dim CSI embedding without pose labels.
- Positive pairs: adjacent frames from same person; negatives: different sessions/rooms.
- VICReg regularization prevents embedding collapse.
- Output: `.rvf` container with `SEG_EMBED` + `SEG_VEC` segments.
- Training triggered via `POST /api/v1/train/pretrain { dataset_ids[], epochs, lr }`.

### Phase 2: Supervised Training Pipeline

#### 2.1 Dataset Integration
- **MM-Fi loader**: Parse HDF5 files, 114→56 subcarrier interpolation via `ruvector-solver` sparse least-squares.
- **Wi-Pose loader**: Parse .mat files, 30→56 zero-padding with Hann window smoothing.
- **Self-collected**: `.csi.jsonl` from Phase 1 recording + camera-generated labels.
- All datasets implement `CsiDataset` trait and produce `(amplitude[B,T*links,56], phase[B,T*links,56], keypoints[B,17,2], visibility[B,17])`.

#### 2.2 Training API
```
POST /api/v1/train/start {
  dataset_ids: string[],
  config: {
    epochs: 100,
    batch_size: 32,
    learning_rate: 3e-4,
    weight_decay: 1e-4,
    early_stopping_patience: 15,
    warmup_epochs: 5,
    pretrained_rvf?: string,  // Base model for fine-tuning
    lora_profile?: string,    // Environment-specific LoRA
  }
}
POST /api/v1/train/stop
GET  /api/v1/train/status        // { epoch, train_loss, val_pck, val_oks, lr, eta_secs }
WS   /ws/train/progress          // Real-time streaming of training metrics
```

#### 2.3 RVF Export
On training completion:
- Best checkpoint exported as `.rvf` with `SEG_VEC` (weights), `SEG_MANIFEST` (metadata), `SEG_WITNESS` (training hash + final metrics), and optional `SEG_QUANT` (INT8 quantization).
- Stored in `data/models/` directory, indexed by model ID.
- `GET /api/v1/models` lists available models; `POST /api/v1/models/load { model_id }` hot-loads into inference.

### Phase 3: LoRA Environment Adaptation

#### 3.1 LoRA Fine-Tuning
- Given a base `.rvf` model, fine-tune only LoRA adapter weights (rank 4-16) on environment-specific recordings.
- 5-10 minutes of labeled data from new environment suffices.
- New LoRA profile appended to existing `.rvf` via `SEG_LORA` segment.
- `POST /api/v1/train/lora { base_model_id, dataset_ids[], profile_name, rank: 8, epochs: 20 }`.

#### 3.2 Profile Switching
- `POST /api/v1/models/lora/activate { model_id, profile_name }` — hot-swap LoRA weights without reloading base model.
- UI dropdown lists available profiles per loaded model.

### Phase 4: UI Integration

#### 4.1 Model Management Panel (new: `ui/components/ModelPanel.js`)
- **Model Library**: List loaded and available `.rvf` models with metadata (version, dataset, PCK score, size, created date).
- **Model Inspector**: Show RVF segment breakdown — weight count, quantization type, LoRA profiles, embedding config, witness hash.
- **Load/Unload**: One-click model loading with progress bar.
- **Compare**: Side-by-side signal-derived vs model-inference toggle in Live Demo.

#### 4.2 Training Dashboard (new: `ui/components/TrainingPanel.js`)
- **Recording Controls**: Start/stop CSI recording, session list with duration and frame counts.
- **Training Progress**: Real-time loss curve (train loss, val loss) and metric charts (PCK@0.2, OKS) via WebSocket streaming.
- **Epoch Table**: Scrollable table of per-epoch metrics with best-epoch highlighting.
- **Early Stopping Indicator**: Visual countdown of patience remaining.
- **Export Button**: Download trained `.rvf` from browser.

#### 4.3 Live Demo Enhancements
- **Model Selector**: Dropdown in toolbar to switch between signal-derived and loaded `.rvf` models.
- **LoRA Profile Selector**: Sub-dropdown showing environment profiles for the active model.
- **Confidence Heatmap Overlay**: Per-keypoint confidence visualization when model is loaded (toggle in render mode dropdown).
- **Pose Trail**: Ghosted keypoint history showing last N frames of motion trajectory.
- **A/B Split View**: Left half signal-derived, right half model-inference for quality comparison.

#### 4.4 Settings Panel Extensions
- **Model section**: Default model path, auto-load on startup, GPU/CPU toggle, inference threads.
- **Training section**: Default hyperparameters, checkpoint directory, auto-export on completion.
- **Recording section**: Default recording directory, max duration, auto-label with camera.

#### 4.5 Dark Mode
All new panels follow the dark mode established in ADR-035 (`#0d1117` backgrounds, `#e0e0e0` text, translucent dark panels with colored accents).

### Phase 5: Inference Pipeline Wiring

#### 5.1 Model-Inference Pose Path
When a `.rvf` model is loaded:
1. CSI frame arrives (UDP or simulated).
2. Extract amplitude + phase tensors from subcarrier data.
3. Feed through ONNX session: `input[1, T*links, 56]` → `output[1, 17, 4]` (x, y, z, conf).
4. Apply Kalman smoothing from `pose_tracker.rs`.
5. Broadcast via WebSocket with `pose_source: "model_inference"`.
6. UI Estimation Mode badge switches from green "SIGNAL-DERIVED" to blue "MODEL INFERENCE".

#### 5.2 Progressive Loading (ADR-031 Layer A/B/C)
- **Layer A** (instant): Signal-derived pose starts immediately.
- **Layer B** (5-10s): Contrastive embeddings loaded, HNSW index warm.
- **Layer C** (30-60s): Full pose model loaded, inference active.
- Transitions seamlessly; UI badge updates automatically.

## Consequences

### Positive
- Users can train a model on **their own environment** without external tools or Python dependencies.
- LoRA profiles mean a single base model adapts to multiple rooms in minutes, not hours.
- Training progress is visible in real-time — no black-box waiting.
- A/B comparison lets users see the quality jump from signal-derived to model-inference.
- RVF container bundles everything (weights, metadata, LoRA, witness) in one portable file.
- Self-supervised pretraining requires no labels — just leave ESP32s running.
- Progressive loading means the UI is never "loading..." — signal-derived kicks in immediately.

### Negative
- Training requires significant compute: GPU recommended for supervised training (CPU possible but 10-50x slower).
- MM-Fi and Wi-Pose datasets must be downloaded separately (10-50 GB each) — cannot be bundled.
- LoRA rank must be tuned per environment; too low loses expressiveness, too high overfits.
- ONNX Runtime adds ~50 MB to the binary size when GPU support is enabled.
- Real-time inference at 10 FPS requires ~10ms per frame — tight budget on CPU.
- Teacher-student labeling (camera → pose labels → CSI training) requires camera access, which may conflict with the privacy-first premise.

### Mitigations
- Provide pre-trained base `.rvf` model downloadable from releases (trained on MM-Fi + Wi-Pose).
- INT8 quantization (`SEG_QUANT`) reduces model size 4x and speeds inference ~2x on CPU.
- Camera-based labeling is **optional** — self-supervised pretraining works without camera.
- Training API validates VRAM availability before starting GPU training; falls back to CPU with warning.

## Implementation Order

| Phase | Effort | Dependencies | Priority |
|-------|--------|-------------|----------|
| 1.1 CSI Recording API | 2-3 days | sensing server | High |
| 1.2 Contrastive Pretraining | 3-5 days | ADR-024, recording API | High |
| 2.1 Dataset Integration | 3-5 days | ADR-015, CsiDataset trait | High |
| 2.2 Training API | 2-3 days | training crate, dataset loaders | High |
| 2.3 RVF Export | 1-2 days | RvfBuilder | Medium |
| 3.1 LoRA Fine-Tuning | 3-5 days | base trained model | Medium |
| 3.2 Profile Switching | 1 day | LoRA in RVF | Medium |
| 4.1 Model Panel UI | 2-3 days | models API | High |
| 4.2 Training Dashboard UI | 3-4 days | training API + WS | High |
| 4.3 Live Demo Enhancements | 2-3 days | model loading | Medium |
| 4.4 Settings Extensions | 1 day | model/training APIs | Low |
| 4.5 Dark Mode | 0.5 days | new panels | Low |
| 5.1 Inference Wiring | 3-5 days | ONNX backend, pose tracker | High |
| 5.2 Progressive Loading | 2-3 days | ADR-031 | Medium |

**Total estimate: 4-6 weeks** (phases can overlap; 1+2 parallel with 4).

## Files to Create/Modify

### New Files
- `ui/components/ModelPanel.js` — Model library, inspector, load/unload controls
- `ui/components/TrainingPanel.js` — Recording controls, training progress, metric charts
- `rust-port/.../sensing-server/src/recording.rs` — CSI recording API handlers
- `rust-port/.../sensing-server/src/training_api.rs` — Training API handlers + WS progress stream
- `rust-port/.../sensing-server/src/model_manager.rs` — Model loading, hot-swap, 32LoRA activation
- `data/models/` — Default model storage directory

### Modified Files
- `rust-port/.../sensing-server/src/main.rs` — Wire recording, training, and model APIs
- `rust-port/.../train/src/trainer.rs` — Add WebSocket progress callback, LoRA training mode
- `rust-port/.../train/src/dataset.rs` — MM-Fi and Wi-Pose dataset loaders
- `rust-port/.../nn/src/onnx.rs` — LoRA weight injection, INT8 quantization support
- `ui/components/LiveDemoTab.js` — Model selector, LoRA dropdown, A/B spsplit view
- `ui/components/SettingsPanel.js` — Model and training configuration sections
- `ui/components/PoseDetectionCanvas.js` — Pose trail rendering, confidence heatmap overlay
- `ui/services/pose.service.js` — Model-inference keypoint processing
- `ui/index.html` — Add Training tabhee
- `ui/style.css` — Styles for new panels 

## References
- ADR-015: MM-Fi + Wi-Pose training datasets
- ADR-016: RuVector training pipeline integration
- ADR-024: Project AETHER — contrastive CSI embedding model
- ADR-029: RuvSense multistatic sensing mode
- ADR-031: RuView sensing-first RF mode (progressive loading)
- ADR-035: Live sensing UI accuracy & data source transparency
- Issue: https://github.com/ruvnet/wifi-densepose/issues/92
- RVF format: `crates/wifi-densepose-sensing-server/src/rvf_container.rs`
- Training crate: `crates/wifi-densepose-train/src/trainer.rs`
- NN inference: `crates/wifi-densepose-nn/src/onnx.rs`
