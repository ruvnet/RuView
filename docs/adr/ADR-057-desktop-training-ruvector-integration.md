# ADR-057: Desktop App Training & RuVector Integration

| Field | Value |
|-------|-------|
| Status | Proposed |
| Date | 2026-03-10 |
| Authors | RuView Team |
| Reviewers | - |
| Related | ADR-016, ADR-017, ADR-024, ADR-027 |

## Context

The RuView desktop application currently provides device discovery, firmware flashing, OTA updates, and real-time sensing visualization. However, users cannot train models or configure RuVector signal processing modules directly from the desktop app.

The following crates exist in the workspace but are not exposed in the desktop UI:

### Training Crate (`wifi-densepose-train`)
- Dataset management (MM-Fi, Wi-Pose formats)
- Model architectures (CSI encoder, pose decoder)
- Training loops with metrics tracking
- Checkpoint save/load
- ruview_metrics integration

### RuVector Crates (5 modules)
1. **ruvector-mincut** - Graph-based person segmentation, DynamicPersonMatcher
2. **ruvector-attn-mincut** - Attention-weighted antenna selection
3. **ruvector-temporal-tensor** - Temporal CSI compression, breathing detection
4. **ruvector-solver** - Sparse interpolation, triangulation
5. **ruvector-attention** - Spatial attention, BVP extraction

## Decision

Add a new **"Training"** page to the desktop application with tabbed navigation:

### Tab Structure

```
┌─────────────────────────────────────────────────────────────┐
│  Training & Models                                          │
├──────────┬──────────┬──────────┬──────────┬────────────────┤
│ Datasets │  Models  │ Training │ RuVector │    Metrics     │
└──────────┴──────────┴──────────┴──────────┴────────────────┘
```

### Tab 1: Datasets
- **Download** standard datasets (MM-Fi, Wi-Pose)
- **Import** custom CSI recordings
- **Preview** dataset samples (CSI heatmaps, labels)
- **Split** into train/val/test sets
- **Statistics** - sample counts, class distribution

### Tab 2: Models
- **Browse** available architectures:
  - CSI Encoder (CNN, Transformer)
  - Pose Decoder (LSTM, GRU)
  - AETHER embedding network (ADR-024)
  - MERIDIAN domain adaptor (ADR-027)
- **Load** checkpoints from disk
- **View** model summary (params, layers, memory)
- **Export** to ONNX/TorchScript

### Tab 3: Training
- **Configure** training:
  - Learning rate, batch size, epochs
  - Optimizer (Adam, SGD, AdamW)
  - Loss function selection
  - Data augmentation toggles
- **GPU Detection** - CUDA/Metal availability
- **Start/Stop** training jobs
- **Progress** - live loss curves, ETA
- **Checkpointing** - auto-save best model

### Tab 4: RuVector
- **Module Configuration**:
  - MinCut graph parameters
  - Attention weights
  - Temporal compression ratio
  - Solver interpolation settings
- **Live Testing** - apply to real-time CSI stream
- **Comparison** - A/B test configurations
- **Export** - save optimal config

### Tab 5: Metrics
- **Loss Curves** - training/validation over epochs
- **Evaluation** - PCK, mAP, IoU scores
- **Confusion Matrix** - per-joint accuracy
- **Export** - CSV, JSON, TensorBoard format

## Architecture

### Backend (Rust/Tauri)

```
wifi-densepose-desktop/
├── src/
│   ├── commands/
│   │   ├── training.rs      # NEW: Training job management
│   │   ├── datasets.rs      # NEW: Dataset download/import
│   │   ├── models.rs        # NEW: Model loading/export
│   │   ├── ruvector.rs      # NEW: RuVector config
│   │   └── metrics.rs       # NEW: Metrics retrieval
│   └── domain/
│       ├── training.rs      # Training state machine
│       └── ruvector.rs      # RuVector config types
```

### Frontend (React/TypeScript)

```
ui/src/pages/
├── Training/
│   ├── index.tsx            # Tab container
│   ├── DatasetsTab.tsx      # Dataset management
│   ├── ModelsTab.tsx        # Model browser
│   ├── TrainingTab.tsx      # Training control
│   ├── RuVectorTab.tsx      # Signal processing config
│   └── MetricsTab.tsx       # Visualization
```

### Tauri Commands

| Command | Description |
|---------|-------------|
| `list_datasets` | Get available datasets |
| `download_dataset` | Download standard dataset |
| `import_dataset` | Import custom recordings |
| `list_models` | Get model architectures |
| `load_checkpoint` | Load model weights |
| `export_model` | Export to ONNX |
| `detect_gpu` | Check CUDA/Metal |
| `start_training` | Begin training job |
| `stop_training` | Cancel training |
| `training_progress` | Get current status |
| `get_ruvector_config` | Load RuVector settings |
| `set_ruvector_config` | Update settings |
| `test_ruvector_live` | Apply to live CSI |
| `get_metrics` | Retrieve training metrics |

### Event System

Training progress updates via Tauri events:

```rust
#[derive(Serialize, Clone)]
pub struct TrainingProgress {
    pub epoch: u32,
    pub total_epochs: u32,
    pub batch: u32,
    pub total_batches: u32,
    pub train_loss: f32,
    pub val_loss: Option<f32>,
    pub learning_rate: f32,
    pub eta_secs: u64,
    pub gpu_memory_mb: Option<u64>,
}

// Emit every batch
app.emit("training:progress", progress)?;

// Emit on completion
app.emit("training:complete", result)?;
```

## Implementation Plan

### Phase 1: Foundation (Week 1-2)
1. Create `Training` page skeleton with tabs
2. Implement `detect_gpu` command
3. Add dataset listing/download commands
4. Design TypeScript types for all entities

### Phase 2: Dataset Management (Week 3)
1. MM-Fi dataset downloader
2. Wi-Pose dataset downloader
3. Custom dataset import (CSV/NPZ)
4. Dataset preview component

### Phase 3: Model Management (Week 4)
1. Model architecture browser
2. Checkpoint loading
3. Model summary display
4. ONNX export

### Phase 4: Training Loop (Week 5-6)
1. Training configuration UI
2. Background training thread
3. Progress event emission
4. Checkpoint auto-save
5. Training history persistence

### Phase 5: RuVector Integration (Week 7)
1. RuVector config UI
2. Live CSI testing
3. A/B comparison mode
4. Config export/import

### Phase 6: Metrics & Polish (Week 8)
1. Loss curve visualization (Chart.js/Recharts)
2. Evaluation metrics display
3. Export functionality
4. Error handling & edge cases

## Risks & Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| No GPU available | Medium | High | CPU fallback with warning |
| Large dataset downloads | High | Medium | Resume support, progress UI |
| Training crashes | Medium | High | Checkpoint recovery, error reporting |
| Memory exhaustion | Low | High | Batch size auto-tuning |
| UI blocking | Medium | High | All training in background thread |

## Success Criteria

1. User can download MM-Fi dataset from UI
2. User can start training with GPU detection
3. Live progress updates without UI freeze
4. Training can be paused/resumed
5. RuVector config changes apply to live CSI
6. Metrics display updates in real-time
7. Models can be exported to ONNX

## Alternatives Considered

### 1. Separate Training App
- **Rejected**: Fragments user experience, duplicates code

### 2. Web-based Training Dashboard
- **Rejected**: Requires server, no offline support

### 3. CLI-only Training
- **Rejected**: Poor UX for non-technical users

## References

- ADR-016: RuVector Training Pipeline Integration
- ADR-017: RuVector Signal + MAT Integration
- ADR-024: AETHER Contrastive CSI Embedding
- ADR-027: MERIDIAN Domain Generalization
- Tauri v2 Events: https://v2.tauri.app/develop/calling-rust/#events
