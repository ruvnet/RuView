# Sensing Server Domain Model

The Sensing Server is the single-binary deployment surface of WiFi-DensePose. It receives raw CSI frames from ESP32 nodes, processes them into sensing features, streams live data to a web UI, and provides a self-contained workflow for recording data, training models, and running inference -- all without external dependencies.

This document defines the system using [Domain-Driven Design](https://martinfowler.com/bliki/DomainDrivenDesign.html) (DDD): bounded contexts that own their data and rules, aggregate roots that enforce invariants, value objects that carry meaning, and domain events that connect everything. The server is implemented as a single Axum binary (`wifi-densepose-sensing-server`) with all state managed through `Arc<RwLock<AppStateInner>>`.

**Bounded Contexts:**

| # | Context | Responsibility | Key ADRs | Code |
|---|---------|----------------|----------|------|
| 1 | [CSI Ingestion](#1-csi-ingestion-context) | Receive, decode, and feature-extract CSI frames from ESP32 UDP | [ADR-019](../adr/ADR-019-sensing-only-ui-mode.md), [ADR-035](../adr/ADR-035-live-sensing-ui-accuracy.md) | `sensing-server/src/main.rs` |
| 2 | [Model Management](#2-model-management-context) | Load, unload, list RVF models; LoRA profile activation | [ADR-043](../adr/ADR-043-sensing-server-ui-api-completion.md) | `sensing-server/src/model_manager.rs` |
| 3 | [CSI Recording](#3-csi-recording-context) | Record CSI frames to .jsonl files, manage recording sessions | [ADR-043](../adr/ADR-043-sensing-server-ui-api-completion.md) | `sensing-server/src/recording.rs` |
| 4 | [Training Pipeline](#4-training-pipeline-context) | Background training runs, progress streaming, contrastive pretraining | [ADR-043](../adr/ADR-043-sensing-server-ui-api-completion.md) | `sensing-server/src/training_api.rs` |
| 5 | [Visualization](#5-visualization-context) | WebSocket streaming to web UI, Gaussian splat rendering, data transparency | [ADR-019](../adr/ADR-019-sensing-only-ui-mode.md), [ADR-035](../adr/ADR-035-live-sensing-ui-accuracy.md) | `ui/` |

All code paths shown are relative to `rust-port/wifi-densepose-rs/crates/wifi-densepose-` unless otherwise noted.

---

## Domain-Driven Design Specification

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Sensing Update** | A complete JSON message broadcast to WebSocket clients each tick, containing node data, features, classification, signal field, and optional vital signs |
| **Tick** | One processing cycle of the sensing loop (default 100ms = 10 fps, configurable via `--tick-ms`) |
| **Data Source** | Origin of CSI data: `esp32` (UDP port 5005), `wifi` (Windows RSSI), `simulated` (synthetic), or `auto` (try ESP32 then fall back) |
| **RVF Model** | A `.rvf` container file holding trained weights, manifest metadata, optional LoRA adapters, and vital sign configuration |
| **LoRA Profile** | A lightweight adapter applied on top of a base RVF model for environment-specific fine-tuning without retraining the full model |
| **Recording Session** | A period during which CSI frames are appended to a `.csi.jsonl` file, identified by a session ID and optional activity label |
| **Training Run** | A background task that loads recorded CSI data, extracts features, trains a regularised linear model, and exports a `.rvf` container |
| **Frame History** | A circular buffer of the last 100 CSI amplitude vectors used for temporal analysis (sliding-window variance, Goertzel breathing estimation) |
| **Goertzel Filter** | A frequency-domain estimator applied to the frame history to detect breathing rate (0.1--0.5 Hz) via a 9-candidate filter bank |
| **Signal Field** | A 20x1x20 grid of interpolated signal intensity values rendered as Gaussian splats in the UI |
| **Pose Source** | Whether pose keypoints are `signal_derived` (analytical from CSI features) or `model_inference` (from a loaded RVF model) |
| **Progressive Loader** | A two-layer model loading strategy: Layer A loads instantly for basic inference, Layer B loads in background for full accuracy |
| **Sensing-Only Mode** | UI mode when the DensePose backend is unavailable; suppresses DensePose tabs, shows only sensing and signal visualization |
| **AppStateInner** | The single shared state struct holding all server state, accessed via `Arc<RwLock<AppStateInner>>` |
| **PCK Score** | Percentage of Correct Keypoints -- the primary accuracy metric for pose estimation models |
| **Contrastive Pretraining** | Self-supervised training on unlabeled CSI data that learns signal representations before supervised fine-tuning (ADR-024) |

---

## Bounded Contexts

### 1. CSI Ingestion Context

**Responsibility:** Receive raw CSI frames from ESP32 nodes via UDP (port 5005), decode the binary protocol, extract temporal and frequency-domain features, and produce a `SensingUpdate` each tick.

```
+------------------------------------------------------------+
|                  CSI Ingestion Context                      |
+------------------------------------------------------------+
|                                                            |
|  +----------------+    +----------------+                  |
|  |  UDP Listener  |    |  Data Source   |                  |
|  |  (port 5005)   |    |  Selector      |                  |
|  |  Esp32Frame    |    |  (auto/esp32/  |                  |
|  |  parser        |    |   wifi/sim)    |                  |
|  +-------+--------+    +-------+--------+                  |
|          |                     |                           |
|          +----------+----------+                           |
|                     v                                      |
|          +-------------------+                             |
|          |  Frame History    |                             |
|          |  Buffer           |                             |
|          |  (VecDeque<Vec>,  |                             |
|          |   100 frames)     |                             |
|          +--------+----------+                             |
|                   v                                        |
|          +-------------------+                             |
|          |  Feature          |                             |
|          |  Extractor        |                             |
|          |  (Welford stats,  |                             |
|          |   Goertzel FFT,   |                             |
|          |   L2 motion)      |                             |
|          +--------+----------+                             |
|                   v                                        |
|          +-------------------+                             |
|          |  Vital Sign       |                             |
|          |  Detector         |---> SensingUpdate           |
|          |  (HR, RR,         |                             |
|          |   breathing)      |                             |
|          +-------------------+                             |
|                                                            |
+------------------------------------------------------------+
```

**Aggregates:**

```rust
/// Aggregate Root: The central shared state of the sensing server.
/// All mutations go through RwLock. All handler functions receive
/// State<Arc<RwLock<AppStateInner>>>.
pub struct AppStateInner {
    /// Most recent sensing update broadcast to clients.
    latest_update: Option<SensingUpdate>,
    /// RSSI history for sparkline display.
    rssi_history: VecDeque<f64>,
    /// Circular buffer of recent CSI amplitude vectors (100 frames).
    frame_history: VecDeque<Vec<f64>>,
    /// Monotonic tick counter.
    tick: u64,
    /// Active data source identifier ("esp32", "wifi", "simulated").
    source: String,
    /// Broadcast channel for WebSocket fan-out.
    tx: broadcast::Sender<String>,
    /// Vital sign detector instance.
    vital_detector: VitalSignDetector,
    /// Most recent vital signs reading.
    latest_vitals: VitalSigns,
    /// Smoothed person count (EMA) for hysteresis.
    smoothed_person_score: f64,
    // ... model, recording, training fields (see other contexts)
}
```

**Value Objects:**

```rust
/// A complete sensing update broadcast to WebSocket clients each tick.
pub struct SensingUpdate {
    pub msg_type: String,         // always "sensing_update"
    pub timestamp: f64,           // Unix timestamp with ms precision
    pub source: String,           // "esp32" | "wifi" | "simulated"
    pub tick: u64,                // monotonic tick counter
    pub nodes: Vec<NodeInfo>,     // per-node CSI data
    pub features: FeatureInfo,    // extracted signal features
    pub classification: ClassificationInfo,
    pub signal_field: SignalField,
    pub vital_signs: Option<VitalSigns>,
    pub persons: Option<Vec<PersonDetection>>,
    pub estimated_persons: Option<usize>,
}

/// Per-node CSI data received from one ESP32.
pub struct NodeInfo {
    pub node_id: u8,
    pub rssi_dbm: f64,
    pub position: [f64; 3],
    pub amplitude: Vec<f64>,
    pub subcarrier_count: usize,
}

/// Extracted signal features from the frame history buffer.
pub struct FeatureInfo {
    pub mean_rssi: f64,
    pub variance: f64,
    pub motion_band_power: f64,
    pub breathing_band_power: f64,
    pub dominant_freq_hz: f64,
    pub change_points: usize,
    pub spectral_power: f64,
}

/// Motion classification derived from features.
pub struct ClassificationInfo {
    pub motion_level: String,  // "empty" | "static" | "active"
    pub presence: bool,
    pub confidence: f64,
}

/// Interpolated signal field for Gaussian splat visualization.
pub struct SignalField {
    pub grid_size: [usize; 3],  // [20, 1, 20]
    pub values: Vec<f64>,
}

/// ESP32 binary CSI frame (ADR-018 protocol, 20-byte header).
pub struct Esp32Frame {
    pub magic: u32,           // 0xC5100001
    pub node_id: u8,
    pub n_antennas: u8,
    pub n_subcarriers: u8,
    pub freq_mhz: u16,
    pub sequence: u32,
    pub rssi: i8,
    pub noise_floor: i8,
    pub amplitudes: Vec<f64>,
    pub phases: Vec<f64>,
}

/// Data source selection enum.
pub enum DataSource {
    Esp32Udp,     // Real ESP32 CSI via UDP port 5005
    WindowsRssi,  // Windows WiFi RSSI via netsh
    Simulated,    // Synthetic sine-wave data
    Auto,         // Try ESP32, fall back to Windows, then simulated
}
```

**Domain Services:**
- `FeatureExtractionService` -- Computes temporal variance (Welford), Goertzel breathing estimation (9-band filter bank), L2 frame-to-frame motion score, SNR-based signal quality
- `VitalSignDetectionService` -- Estimates breathing rate, heart rate, and confidence from CSI phase history
- `DataSourceSelectionService` -- Probes UDP port 5005 for ESP32 frames; falls back through Windows RSSI then simulation

**Invariants:**
- Frame history buffer never exceeds 100 entries (oldest dropped on push)
- Goertzel breathing estimate requires 3x SNR above noise to be reported
- Source type is determined once at startup and does not change during runtime

---

### 2. Model Management Context

**Responsibility:** Discover `.rvf` model files from `data/models/`, load weights into memory for inference, manage the active model lifecycle, and support LoRA profile activation.

```
+------------------------------------------------------------+
|               Model Management Context                     |
+------------------------------------------------------------+
|                                                            |
|  +----------------+    +----------------+                  |
|  |  Model Scanner |    |  RVF Reader    |                  |
|  |  (data/models/ |    |  (parse .rvf   |                  |
|  |   *.rvf enum)  |    |   manifest)    |                  |
|  +-------+--------+    +-------+--------+                  |
|          |                     |                           |
|          +----------+----------+                           |
|                     v                                      |
|          +-------------------+                             |
|          |  Model Registry   |                             |
|          |  (Vec<ModelInfo>) |                             |
|          +--------+----------+                             |
|                   v                                        |
|          +-------------------+                             |
|          |  Model Loader     |                             |
|          |  (RvfReader ->    |---> LoadedModelState        |
|          |   weights,        |                             |
|          |   LoRA profiles)  |                             |
|          +--------+----------+                             |
|                   v                                        |
|          +-------------------+                             |
|          |  LoRA Activator   |                             |
|          |  (profile switch) |                             |
|          +-------------------+                             |
|                                                            |
+------------------------------------------------------------+
```

**Aggregates:**

```rust
/// Aggregate Root: Runtime state for a loaded RVF model.
/// At most one LoadedModelState exists at any time.
pub struct LoadedModelState {
    /// Model identifier (derived from filename without .rvf extension).
    pub model_id: String,
    /// Original filename on disk.
    pub filename: String,
    /// Version string from the RVF manifest.
    pub version: String,
    /// Description from the RVF manifest.
    pub description: String,
    /// LoRA profiles available in this model.
    pub lora_profiles: Vec<String>,
    /// Currently active LoRA profile (if any).
    pub active_lora_profile: Option<String>,
    /// Model weights (f32 parameters).
    pub weights: Vec<f32>,
    /// Number of frames processed since load.
    pub frames_processed: u64,
    /// Cumulative inference time for avg calculation.
    pub total_inference_ms: f64,
    /// When the model was loaded.
    pub loaded_at: Instant,
}
```

**Value Objects:**

```rust
/// Summary information for a model discovered on disk.
pub struct ModelInfo {
    pub id: String,
    pub filename: String,
    pub version: String,
    pub description: String,
    pub size_bytes: u64,
    pub created_at: String,
    pub pck_score: Option<f64>,
    pub has_quantization: bool,
    pub lora_profiles: Vec<String>,
    pub segment_count: usize,
}

/// Information about the currently loaded model with runtime stats.
pub struct ActiveModelInfo {
    pub model_id: String,
    pub filename: String,
    pub version: String,
    pub description: String,
    pub avg_inference_ms: f64,
    pub frames_processed: u64,
    pub pose_source: String,      // "model_inference"
    pub lora_profiles: Vec<String>,
    pub active_lora_profile: Option<String>,
}

/// Request to load a model by ID.
pub struct LoadModelRequest {
    pub model_id: String,
}

/// Request to activate a LoRA profile.
pub struct ActivateLoraRequest {
    pub model_id: String,
    pub profile_name: String,
}
```

**Domain Services:**
- `ModelScanService` -- Scans `data/models/` at startup for `.rvf` files, parses each with `RvfReader` to extract manifest metadata
- `ModelLoadService` -- Reads model weights from an RVF container into memory, sets `model_loaded = true`
- `LoraActivationService` -- Switches the active LoRA adapter on a loaded model without full reload

**Invariants:**
- Only one model can be loaded at a time; loading a new model implicitly unloads the previous one
- A model must be loaded before a LoRA profile can be activated
- The `active_lora_profile` must be one of the model's declared `lora_profiles`
- Model deletion is refused if the model is currently loaded (must unload first)
- `data/models/` directory is created at startup if it does not exist

---

### 3. CSI Recording Context

**Responsibility:** Capture CSI frames to `.csi.jsonl` files during active recording sessions, manage session lifecycle, and provide download/delete operations on stored recordings.

```
+------------------------------------------------------------+
|               CSI Recording Context                        |
+------------------------------------------------------------+
|                                                            |
|  +----------------+    +----------------+                  |
|  |  Start/Stop    |    |  Auto-Stop     |                  |
|  |  Controller    |    |  Timer         |                  |
|  |  (REST API)    |    |  (duration_    |                  |
|  |                |    |   secs check)  |                  |
|  +-------+--------+    +-------+--------+                  |
|          |                     |                           |
|          +----------+----------+                           |
|                     v                                      |
|          +-------------------+                             |
|          |  Recording State  |                             |
|          |  (session_id,     |                             |
|          |   frame_count,    |                             |
|          |   file_path)      |                             |
|          +--------+----------+                             |
|                   v                                        |
|          +-------------------+                             |
|          |  Frame Writer     |                             |
|          |  (maybe_record_   |---> .csi.jsonl file         |
|          |   frame on each   |                             |
|          |   tick)           |                             |
|          +--------+----------+                             |
|                   v                                        |
|          +-------------------+                             |
|          |  Metadata Writer  |                             |
|          |  (.meta.json on   |                             |
|          |   stop)           |                             |
|          +-------------------+                             |
|                                                            |
+------------------------------------------------------------+
```

**Aggregates:**

```rust
/// Aggregate Root: Runtime state for the active CSI recording session.
/// At most one RecordingState can be active at any time.
pub struct RecordingState {
    /// Whether a recording is currently active.
    pub active: bool,
    /// Session ID of the active recording.
    pub session_id: String,
    /// Session display name.
    pub session_name: String,
    /// Optional label / activity tag (e.g., "walking", "standing").
    pub label: Option<String>,
    /// Path to the JSONL file being written.
    pub file_path: PathBuf,
    /// Number of frames written so far.
    pub frame_count: u64,
    /// When the recording started (monotonic clock).
    pub start_time: Instant,
    /// ISO-8601 start timestamp for metadata.
    pub started_at: String,
    /// Optional auto-stop duration in seconds.
    pub duration_secs: Option<u64>,
}
```

**Value Objects:**

```rust
/// Metadata for a completed or active recording session.
pub struct RecordingSession {
    pub id: String,
    pub name: String,
    pub label: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub frame_count: u64,
    pub file_size_bytes: u64,
    pub file_path: String,
}

/// A single recorded CSI frame line (JSONL format).
pub struct RecordedFrame {
    pub timestamp: f64,
    pub subcarriers: Vec<f64>,
    pub rssi: f64,
    pub noise_floor: f64,
    pub features: serde_json::Value,
}

/// Request to start a new recording session.
pub struct StartRecordingRequest {
    pub session_name: String,
    pub label: Option<String>,
    pub duration_secs: Option<u64>,
}
```

**Domain Services:**
- `RecordingLifecycleService` -- Creates a new `.csi.jsonl` file, generates session ID, manages start/stop transitions
- `FrameWriterService` -- Called on each tick via `maybe_record_frame()`, appends a `RecordedFrame` JSON line to the active file
- `AutoStopService` -- Checks elapsed time against `duration_secs` on each tick; triggers stop when exceeded
- `RecordingScanService` -- Enumerates `data/recordings/` for `.csi.jsonl` files and reads companion `.meta.json` for session metadata

**Invariants:**
- Only one recording session can be active at a time; starting a new recording while one is active returns HTTP 409 Conflict
- Recording with `duration_secs` set auto-stops after the specified elapsed time
- A `.meta.json` companion file is written when a recording stops, capturing final frame count and duration
- `data/recordings/` directory is created at startup if it does not exist
- Frame writer acquires a read lock on `AppStateInner` per tick; stop acquires a write lock

---

### 4. Training Pipeline Context

**Responsibility:** Run background training against recorded CSI data, stream epoch-level progress via WebSocket, and export trained models as `.rvf` containers. Supports supervised training, contrastive pretraining (ADR-024), and LoRA fine-tuning.

```
+------------------------------------------------------------+
|              Training Pipeline Context                     |
+------------------------------------------------------------+
|                                                            |
|  +----------------+    +----------------+                  |
|  |  Training API  |    |  WebSocket     |                  |
|  |  (start/stop/  |    |  Progress      |                  |
|  |   status)      |    |  Streamer      |                  |
|  +-------+--------+    +-------+--------+                  |
|          |                     ^                           |
|          v                     |                           |
|  +-------------------+        |                            |
|  |  Training         |        |                            |
|  |  Orchestrator     +--------+                            |
|  |  (tokio::spawn)   |  broadcast::Sender                  |
|  +--------+----------+                                     |
|           v                                                |
|  +-------------------+                                     |
|  |  Feature          |                                     |
|  |  Extractor        |                                     |
|  |  (subcarrier var, |                                     |
|  |   Goertzel power, |                                     |
|  |   temporal grad)  |                                     |
|  +--------+----------+                                     |
|           v                                                |
|  +-------------------+                                     |
|  |  Gradient Descent |                                     |
|  |  Trainer          |                                     |
|  |  (batch SGD,      |---> TrainingProgress                |
|  |   early stopping, |                                     |
|  |   warmup)         |                                     |
|  +--------+----------+                                     |
|           v                                                |
|  +-------------------+                                     |
|  |  RVF Exporter     |                                     |
|  |  (RvfBuilder ->   |---> data/models/*.rvf               |
|  |   .rvf container) |                                     |
|  +-------------------+                                     |
|                                                            |
+------------------------------------------------------------+
```

**Aggregates:**

```rust
/// Aggregate Root: Runtime training state stored in AppStateInner.
/// At most one training run can be active at any time.
pub struct TrainingState {
    /// Current status snapshot.
    pub status: TrainingStatus,
    /// Handle to the background training task (for cancellation).
    pub task_handle: Option<tokio::task::JoinHandle<()>>,
}
```

**Value Objects:**

```rust
/// Current training status (returned by GET /api/v1/train/status).
pub struct TrainingStatus {
    pub active: bool,
    pub epoch: u32,
    pub total_epochs: u32,
    pub train_loss: f64,
    pub val_pck: f64,         // Percentage of Correct Keypoints
    pub val_oks: f64,         // Object Keypoint Similarity
    pub lr: f64,              // current learning rate
    pub best_pck: f64,
    pub best_epoch: u32,
    pub patience_remaining: u32,
    pub eta_secs: Option<u64>,
    pub phase: String,        // "idle" | "training" | "complete" | "failed"
}

/// Progress update sent over WebSocket to connected UI clients.
pub struct TrainingProgress {
    pub epoch: u32,
    pub batch: u32,
    pub total_batches: u32,
    pub train_loss: f64,
    pub val_pck: f64,
    pub val_oks: f64,
    pub lr: f64,
    pub phase: String,
}

/// Training configuration submitted with a start request.
pub struct TrainingConfig {
    pub epochs: u32,                    // default: 100
    pub batch_size: u32,               // default: 8
    pub learning_rate: f64,            // default: 0.001
    pub weight_decay: f64,             // default: 1e-4
    pub early_stopping_patience: u32,  // default: 20
    pub warmup_epochs: u32,            // default: 5
    pub pretrained_rvf: Option<String>,
    pub lora_profile: Option<String>,
}

/// Request to start supervised training.
pub struct StartTrainingRequest {
    pub dataset_ids: Vec<String>,  // recording session IDs
    pub config: TrainingConfig,
}

/// Request to start contrastive pretraining (ADR-024).
pub struct PretrainRequest {
    pub dataset_ids: Vec<String>,
    pub epochs: u32,    // default: 50
    pub lr: f64,        // default: 0.001
}

/// Request to start LoRA fine-tuning.
pub struct LoraTrainRequest {
    pub base_model_id: String,
    pub dataset_ids: Vec<String>,
    pub profile_name: String,
    pub rank: u8,       // default: 8
    pub epochs: u32,    // default: 30
}
```

**Domain Services:**
- `TrainingOrchestrationService` -- Spawns a background `tokio::task`, loads recorded frames, runs feature extraction, executes gradient descent with early stopping and warmup
- `FeatureExtractionService` -- Computes per-subcarrier sliding-window variance, temporal gradients, Goertzel frequency-domain power across 9 bands, and 3 global scalar features (mean amplitude, std, motion score)
- `ProgressBroadcastService` -- Sends `TrainingProgress` messages through a `broadcast::Sender` channel that WebSocket handlers subscribe to
- `RvfExportService` -- Uses `RvfBuilder` to write the best checkpoint as a `.rvf` container to `data/models/`

**Invariants:**
- Only one training run can be active at a time; starting training while one is running returns HTTP 409 Conflict
- Training requires at least one recording with a minimum frame count before starting
- Early stopping halts training after `patience` epochs with no improvement in `val_pck`
- Learning rate warmup ramps linearly from 0 to `learning_rate` over `warmup_epochs`
- On completion, the best model (by `val_pck`) is automatically exported as `.rvf`
- Training status phase transitions: `idle` -> `training` -> `complete` | `failed` -> `idle`
- Stopping an active training run aborts the background task via `JoinHandle::abort()` and resets phase to `idle`

---

### 5. Visualization Context

**Responsibility:** Stream sensing data to web UI clients via WebSocket, render Gaussian splat visualizations, display data source transparency indicators, and manage UI mode (full vs. sensing-only).

```
+------------------------------------------------------------+
|               Visualization Context                        |
+------------------------------------------------------------+
|                                                            |
|  +----------------+    +----------------+                  |
|  |  WebSocket     |    |  Sensing       |                  |
|  |  Hub           |    |  Service (JS)  |                  |
|  |  (/ws/sensing) |    |  (client-side  |                  |
|  |  broadcast::   |    |   reconnect +  |                  |
|  |  Receiver      |    |   sim fallback)|                  |
|  +-------+--------+    +-------+--------+                  |
|          |                     |                           |
|          +----------+----------+                           |
|                     v                                      |
|  +----------------------------------------------+         |
|  |  UI Components                                |         |
|  |                                               |         |
|  |  +----------+  +----------+  +----------+    |         |
|  |  | Sensing  |  | Live     |  | Models   |    |         |
|  |  | Tab      |  | Demo Tab |  | Tab      |    |         |
|  |  | (splats) |  | (pose)   |  | (manage) |    |         |
|  |  +----------+  +----------+  +----------+    |         |
|  |  +----------+  +----------+                   |         |
|  |  | Recording|  | Training |                   |         |
|  |  | Tab      |  | Tab      |                   |         |
|  |  | (capture)|  | (train)  |                   |         |
|  |  +----------+  +----------+                   |         |
|  +----------------------------------------------+         |
|                                                            |
+------------------------------------------------------------+
```

**Value Objects:**

```rust
/// Data source indicator shown in the UI (ADR-035).
pub enum DataSourceIndicator {
    LiveEsp32,     // Green banner: "LIVE - ESP32"
    Reconnecting,  // Yellow banner: "RECONNECTING..."
    Simulated,     // Red banner: "SIMULATED DATA"
}

/// Pose estimation mode badge (ADR-035).
pub enum EstimationMode {
    SignalDerived,    // Green badge: analytical pose from CSI features
    ModelInference,   // Blue badge: neural network inference from loaded RVF
}

/// Render mode for pose visualization (ADR-035).
pub enum RenderMode {
    Skeleton,   // Green lines connecting joints + red keypoint dots
    Keypoints,  // Large colored dots with glow and labels
    Heatmap,    // Gaussian radial blobs per keypoint, faint skeleton overlay
    Dense,      // Body region segmentation with colored filled polygons
}
```

**Domain Services:**
- `WebSocketBroadcastService` -- Subscribes to `broadcast::Sender<String>`, forwards each `SensingUpdate` JSON to all connected WebSocket clients
- `SensingServiceJS` -- Client-side JavaScript that manages WebSocket connection, tracks `dataSource` state, falls back to simulation after 5 failed reconnect attempts (~30s delay)
- `GaussianSplatRenderer` -- Custom GLSL `ShaderMaterial` rendering point-cloud splats on a 20x20 floor grid, colored by signal intensity
- `PoseRenderer` -- Renders skeleton, keypoints, heatmap, or dense body segmentation modes
- `BackendDetector` -- Auto-detects whether the full DensePose backend is available; sets `sensingOnlyMode = true` if unreachable

**Invariants:**
- WebSocket sensing service is started on application init, not lazily on tab visit (ADR-043 fix)
- Simulation fallback is delayed to 5 failed reconnect attempts (~30 seconds) to avoid premature synthetic data
- `pose_source` field is passed through data conversion so the Estimation Mode badge displays correctly
- Dashboard and Live Demo tabs read `sensingService.dataSource` at load time -- the service must already be connected

---

## Domain Events

| Event | Published By | Consumed By | Payload |
|-------|-------------|-------------|---------|
| `ServerStarted` | CSI Ingestion | Visualization | `{ http_port, udp_port, source_type }` |
| `CsiFrameIngested` | CSI Ingestion | Recording, Visualization | `{ source, node_id, subcarrier_count, tick }` |
| `SensingUpdateBroadcast` | CSI Ingestion | Visualization (WebSocket) | Full `SensingUpdate` JSON |
| `ModelLoaded` | Model Management | CSI Ingestion (inference path) | `{ model_id, weight_count, version }` |
| `ModelUnloaded` | Model Management | CSI Ingestion | `{ model_id }` |
| `LoraProfileActivated` | Model Management | CSI Ingestion | `{ model_id, profile_name }` |
| `RecordingStarted` | Recording | Visualization | `{ session_id, session_name, file_path }` |
| `RecordingStopped` | Recording | Visualization | `{ session_id, frame_count, duration_secs }` |
| `TrainingStarted` | Training Pipeline | Visualization | `{ run_id, config, recording_ids }` |
| `TrainingEpochComplete` | Training Pipeline | Visualization (WebSocket) | `{ epoch, total_epochs, train_loss, val_pck, lr }` |
| `TrainingComplete` | Training Pipeline | Model Management, Visualization | `{ run_id, final_pck, model_path }` |
| `TrainingFailed` | Training Pipeline | Visualization | `{ run_id, error_message }` |
| `WebSocketClientConnected` | Visualization | -- | `{ endpoint, client_addr }` |
| `WebSocketClientDisconnected` | Visualization | -- | `{ endpoint, client_addr }` |

In the current implementation, events are realized through two mechanisms:
1. **`broadcast::Sender<String>`** for WebSocket fan-out of sensing updates
2. **`broadcast::Sender<TrainingProgress>`** for training progress streaming
3. **State mutations via RwLock** where other contexts read state changes on their next tick

---

## Context Map

```
+-------------------+          +---------------------+
|   CSI Ingestion   |--------->|   Visualization     |
|   (produces       | publish  |   (WebSocket        |
|    SensingUpdate)  | -------> |    consumers)       |
+--------+----------+          +----------+----------+
         |                                |
         | maybe_record_frame()           | reads dataSource
         v                                |
+-------------------+                     |
|   CSI Recording   |                     |
|   (hooks into     |                     |
|    tick loop)      |                     |
+--------+----------+                     |
         |                                |
         | provides dataset_ids           |
         v                                |
+-------------------+          +----------+----------+
| Training Pipeline |--------->| Model Management    |
| (reads .jsonl,    | exports  | (loads .rvf for     |
|  trains model)    | .rvf --> |  inference)         |
+-------------------+          +----------+----------+
                                          |
                                          | model weights
                                          v
                               +----------+----------+
                               |   CSI Ingestion      |
                               |   (inference path    |
                               |    uses loaded model)|
                               +----------------------+
```

**Relationships:**

| Upstream | Downstream | Relationship | Mechanism |
|----------|-----------|--------------|-----------|
| CSI Ingestion | Visualization | Published Language | `broadcast::Sender<String>` with `SensingUpdate` JSON schema |
| CSI Ingestion | CSI Recording | Shared Kernel | `maybe_record_frame()` called from the ingestion tick loop |
| CSI Recording | Training Pipeline | Conformist | Training reads `.csi.jsonl` files produced by recording; no negotiation on format |
| Training Pipeline | Model Management | Supplier-Consumer | Training exports `.rvf` to `data/models/`; Model Management scans and loads |
| Model Management | CSI Ingestion | Shared Kernel | Loaded weights stored in `AppStateInner`; ingestion reads them for inference |
| Training Pipeline | Visualization | Published Language | `broadcast::Sender<TrainingProgress>` with progress JSON schema |

---

## Anti-Corruption Layers

### ESP32 Binary Protocol ACL

The ESP32 sends CSI frames using a compact binary protocol (ADR-018): 20-byte header with magic `0xC5100001`, followed by amplitude and phase arrays. The `Esp32Frame` parser in the ingestion context decodes this binary format into domain value objects (`NodeInfo`, amplitude/phase vectors) before any downstream processing. No other context handles raw UDP bytes.

### RVF Container ACL

The `.rvf` container format encapsulates model weights, manifest metadata, vital sign configuration, and optional LoRA adapters. The `RvfReader` and `RvfBuilder` types in the `rvf_container` module provide the anti-corruption layer between the on-disk binary format and the domain types (`ModelInfo`, `LoadedModelState`). The training pipeline writes through `RvfBuilder`; the model management context reads through `RvfReader`.

### Sensing-Only Mode ACL (Client-Side)

When the DensePose backend (port 8000) is unreachable, the client-side `BackendDetector` sets `sensingOnlyMode = true`. The `ApiService.request()` method short-circuits all requests to the DensePose backend, returning empty responses instead of `ERR_CONNECTION_REFUSED`. This prevents DensePose-specific concerns from leaking into the sensing UI.

### JSONL Recording Format ACL

CSI frames are recorded as newline-delimited JSON (`.csi.jsonl`). The `RecordedFrame` struct defines the schema: `{timestamp, subcarriers, rssi, noise_floor, features}`. The training pipeline reads through this schema, extracting subcarrier arrays for feature computation. If the internal sensing representation changes, only the `maybe_record_frame()` serializer needs updating -- the training pipeline depends only on the `RecordedFrame` contract.

---

## REST API Surface

All endpoints share `AppStateInner` via `Arc<RwLock<AppStateInner>>`.

### CSI Ingestion & Sensing

| Method | Path | Context | Description |
|--------|------|---------|-------------|
| GET | `/api/v1/sensing/latest` | Ingestion | Latest sensing update |
| WS | `/ws/sensing` | Visualization | Streaming sensing updates |

### Model Management

| Method | Path | Context | Description |
|--------|------|---------|-------------|
| GET | `/api/v1/models` | Model Mgmt | List all discovered `.rvf` models |
| GET | `/api/v1/models/:id` | Model Mgmt | Detailed info for a specific model |
| GET | `/api/v1/models/active` | Model Mgmt | Active model with runtime stats |
| POST | `/api/v1/models/load` | Model Mgmt | Load model weights into memory |
| POST | `/api/v1/models/unload` | Model Mgmt | Unload the active model |
| DELETE | `/api/v1/models/:id` | Model Mgmt | Delete a model file from disk |
| GET | `/api/v1/models/lora/profiles` | Model Mgmt | List LoRA profiles for active model |
| POST | `/api/v1/models/lora/activate` | Model Mgmt | Activate a LoRA adapter |

### CSI Recording

| Method | Path | Context | Description |
|--------|------|---------|-------------|
| POST | `/api/v1/recording/start` | Recording | Start a new recording session |
| POST | `/api/v1/recording/stop` | Recording | Stop the active recording |
| GET | `/api/v1/recording/list` | Recording | List all recording sessions |
| GET | `/api/v1/recording/download/:id` | Recording | Download a `.csi.jsonl` file |
| DELETE | `/api/v1/recording/:id` | Recording | Delete a recording |

### Training Pipeline

| Method | Path | Context | Description |
|--------|------|---------|-------------|
| POST | `/api/v1/train/start` | Training | Start supervised training |
| POST | `/api/v1/train/stop` | Training | Stop the active training run |
| GET | `/api/v1/train/status` | Training | Current training phase and metrics |
| POST | `/api/v1/train/pretrain` | Training | Start contrastive pretraining |
| POST | `/api/v1/train/lora` | Training | Start LoRA fine-tuning |
| WS | `/ws/train/progress` | Training | Streaming training progress |

---

## File Layout

```
data/
+-- models/                              # RVF model files
|   +-- wifi-densepose-v1.rvf           # Trained model container
|   +-- wifi-densepose-field-v2.rvf     # Environment-calibrated model
+-- recordings/                          # CSI recording sessions
    +-- walking-20260303_140000.csi.jsonl       # Raw CSI frames (JSONL)
    +-- walking-20260303_140000.csi.meta.json   # Session metadata
    +-- standing-20260303_141500.csi.jsonl
    +-- standing-20260303_141500.csi.meta.json

crates/wifi-densepose-sensing-server/
+-- src/
    +-- main.rs            # Server entry, CLI args, AppStateInner, sensing loop
    +-- model_manager.rs   # Model Management bounded context
    +-- recording.rs       # CSI Recording bounded context
    +-- training_api.rs    # Training Pipeline bounded context
    +-- rvf_container.rs   # RVF format ACL (RvfReader, RvfBuilder)
    +-- rvf_pipeline.rs    # Progressive loader for model inference
    +-- vital_signs.rs     # Vital sign detection from CSI phase
    +-- dataset.rs         # Dataset loading for training
    +-- trainer.rs         # Core training loop implementation
    +-- embedding.rs       # Contrastive embedding extraction
    +-- graph_transformer.rs # Graph transformer architecture
    +-- sona.rs            # SONA self-optimizing profile
    +-- sparse_inference.rs # Sparse inference engine
    +-- lib.rs             # Public module re-exports
```

---

## Related

- [ADR-019: Sensing-Only UI Mode](../adr/ADR-019-sensing-only-ui-mode.md) -- Decoupled sensing UI, Gaussian splats, Python WebSocket bridge
- [ADR-035: Live Sensing UI Accuracy](../adr/ADR-035-live-sensing-ui-accuracy.md) -- Data transparency, Goertzel breathing estimation, signal-responsive pose
- [ADR-043: Sensing Server UI API Completion](../adr/ADR-043-sensing-server-ui-api-completion.md) -- Model, recording, training endpoints; single-binary deployment
- [RuvSense Domain Model](ruvsense-domain-model.md) -- Upstream signal processing domain (multistatic sensing, coherence, tracking)
- [WiFi-Mat Domain Model](wifi-mat-domain-model.md) -- Downstream disaster response domain
