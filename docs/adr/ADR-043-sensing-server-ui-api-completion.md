# ADR-043: Sensing Server UI API Completion

**Status**: Accepted
**Date**: 2026-03-03
**Deciders**: @ruvnet
**Supersedes**: None
**Related**: ADR-034, ADR-036, ADR-039, ADR-040, ADR-041

---

## Context

The WiFi-DensePose sensing server (`wifi-densepose-sensing-server`) is a single-binary Axum server that receives ESP32 CSI frames via UDP, processes them through the RuVector signal pipeline, and serves both a web UI at `/ui/` and a REST/WebSocket API. The UI provides tabs for live sensing visualization, model management, CSI recording, and training -- all designed to operate without external dependencies.

However, the UI's JavaScript expected several backend endpoints that were not yet implemented in the Rust server. Opening the browser console revealed persistent 404 errors for model, recording, and training API routes. Three categories of functionality were broken:

### 1. Model Management (7 endpoints missing)

The Models tab calls `GET /api/v1/models` to list available `.rvf` model files, `GET /api/v1/models/active` to show the currently loaded model, `POST /api/v1/models/load` and `POST /api/v1/models/unload` to control the model lifecycle, and `DELETE /api/v1/models/:id` to remove models from disk. LoRA fine-tuning profiles are managed via `GET /api/v1/models/lora/profiles` and `POST /api/v1/models/lora/activate`. All of these returned 404.

### 2. CSI Recording (5 endpoints missing)

The Recording tab calls `POST /api/v1/recording/start` and `POST /api/v1/recording/stop` to capture CSI frames to `.csi.jsonl` files for later training. `GET /api/v1/recording/list` enumerates stored sessions. `DELETE /api/v1/recording/:id` removes recordings. None of these were wired into the server's router.

### 3. Training Pipeline (5 endpoints missing)

The Training tab calls `POST /api/v1/train/start` to launch a background training run against recorded CSI data, `POST /api/v1/train/stop` to abort, and `GET /api/v1/train/status` to poll progress. Contrastive pretraining (`POST /api/v1/train/pretrain`) and LoRA fine-tuning (`POST /api/v1/train/lora`) endpoints were also unavailable. A WebSocket endpoint at `/ws/train/progress` streams epoch-level progress updates to the UI.

### 4. Sensing Service Not Started on App Init

The web UI's `sensingService` singleton (which manages the WebSocket connection to `/ws/sensing`) was only started lazily when the user navigated to the Sensing tab (`SensingTab.js:182`). However, the Dashboard and Live Demo tabs both read `sensingService.dataSource` at load time — and since the service was never started, the status permanently showed **"RECONNECTING"** with no WebSocket connection attempt and no console errors. This silent failure affected the first-load experience for every user.

### 5. Mobile App Defects

The Expo React Native mobile companion (ADR-034) had two integration defects:

- **WebSocket URL builder**: `ws.service.ts` hardcoded port `3001` for the WebSocket connection instead of using the same-origin port derived from the REST API URL. When the sensing server runs on a different port (e.g., `8080` or `3000`), the mobile app could not connect.
- **Test configuration**: `jest.config.js` contained a `testPathIgnorePatterns` entry that effectively excluded the entire test directory, causing all 25 tests to be skipped silently.
- **Placeholder tests**: All 25 mobile test files contained `it.todo()` stubs with no assertions, providing false confidence in test coverage.

---

## Decision

Implement the complete model management, CSI recording, and training API directly in the sensing server's `main.rs` as inline handler functions sharing `AppStateInner` via `Arc<RwLock<…>>`. Wire all 14 routes into the server's main router so the UI loads without any 404 console errors. Start the sensing WebSocket service on application init (not lazily on tab visit) so Dashboard and Live Demo tabs connect immediately. Fix the mobile app WebSocket URL builder, test configuration, and replace placeholder tests with real implementations.

### Architecture

All 14 new handler functions are implemented directly in `main.rs` as async functions taking `State<AppState>` extractors, sharing the existing `AppStateInner` via `Arc<RwLock<…>>`. This avoids introducing new module files and keeps all API routes in one place alongside the existing sensing and pose handlers.

```
┌───────────────────────────────────────────────────────────────────────┐
│                     Sensing Server (main.rs)                           │
│                                                                       │
│  Router::new()                                                        │
│  ├── /api/v1/sensing/*       (existing — CSI streaming)               │
│  ├── /api/v1/pose/*          (existing — pose estimation)             │
│  ├── /api/v1/models          GET    list_models          (NEW)        │
│  ├── /api/v1/models/active   GET    get_active_model     (NEW)        │
│  ├── /api/v1/models/load     POST   load_model           (NEW)        │
│  ├── /api/v1/models/unload   POST   unload_model         (NEW)        │
│  ├── /api/v1/models/:id      DELETE delete_model         (NEW)        │
│  ├── /api/v1/models/lora/profiles   GET  list_lora       (NEW)        │
│  ├── /api/v1/models/lora/activate   POST activate_lora   (NEW)        │
│  ├── /api/v1/recording/list  GET    list_recordings      (NEW)        │
│  ├── /api/v1/recording/start POST   start_recording      (NEW)        │
│  ├── /api/v1/recording/stop  POST   stop_recording       (NEW)        │
│  ├── /api/v1/recording/:id   DELETE delete_recording     (NEW)        │
│  ├── /api/v1/train/status    GET    train_status         (NEW)        │
│  ├── /api/v1/train/start     POST   train_start          (NEW)        │
│  ├── /api/v1/train/stop      POST   train_stop           (NEW)        │
│  ├── /ws/sensing             (existing — sensing WebSocket)           │
│  └── /ui/*                   (existing — static file serving)         │
│                                                                       │
│  AppStateInner (new fields)                                           │
│  ├── discovered_models: Vec<Value>                                    │
│  ├── active_model_id: Option<String>                                  │
│  ├── recordings: Vec<Value>                                           │
│  ├── recording_active / recording_start_time / recording_current_id   │
│  ├── recording_stop_tx: Option<watch::Sender<bool>>                   │
│  ├── training_status: Value                                           │
│  └── training_config: Option<Value>                                   │
│                                                                       │
│  data/                                                                │
│  ├── models/         *.rvf files scanned at startup                   │
│  └── recordings/     *.jsonl files written by background task         │
└───────────────────────────────────────────────────────────────────────┘
```

Routes are registered individually in the `http_app` Router before the static UI fallback handler.

### New Endpoints (17 total)

#### Model Management (`model_manager.rs`)

| Method | Path | Request Body | Response | Description |
|--------|------|-------------|----------|-------------|
| `GET` | `/api/v1/models` | -- | `{ models: ModelInfo[], count: usize }` | Scan `data/models/` for `.rvf` files and return manifest metadata |
| `GET` | `/api/v1/models/{id}` | -- | `ModelInfo` | Detailed info for a single model (version, PCK score, LoRA profiles, segment count) |
| `GET` | `/api/v1/models/active` | -- | `ActiveModelInfo \| { status: "no_model" }` | Active model with runtime stats (avg inference ms, frames processed) |
| `POST` | `/api/v1/models/load` | `{ model_id: string }` | `{ status: "loaded", model_id, weight_count }` | Load model weights into memory via `RvfReader`, set `model_loaded = true` |
| `POST` | `/api/v1/models/unload` | -- | `{ status: "unloaded", model_id }` | Drop loaded weights, set `model_loaded = false` |
| `POST` | `/api/v1/models/lora/activate` | `{ model_id, profile_name }` | `{ status: "activated", profile_name }` | Activate a LoRA adapter profile on the loaded model |
| `GET` | `/api/v1/models/lora/profiles` | -- | `{ model_id, profiles: string[], active }` | List LoRA profiles available in the loaded model |

#### CSI Recording (`recording.rs`)

| Method | Path | Request Body | Response | Description |
|--------|------|-------------|----------|-------------|
| `POST` | `/api/v1/recording/start` | `{ session_name, label?, duration_secs? }` | `{ status: "recording", session_id, file_path }` | Create a new `.csi.jsonl` file and begin appending frames |
| `POST` | `/api/v1/recording/stop` | -- | `{ status: "stopped", session_id, frame_count }` | Stop the active recording, write companion `.meta.json` |
| `GET` | `/api/v1/recording/list` | -- | `{ recordings: RecordingSession[], count }` | List all recordings by scanning `.meta.json` files |
| `GET` | `/api/v1/recording/download/{id}` | -- | `application/x-ndjson` file | Download the raw JSONL recording file |
| `DELETE` | `/api/v1/recording/{id}` | -- | `{ status: "deleted", deleted_files }` | Remove `.csi.jsonl` and `.meta.json` files |

#### Training Pipeline (`training_api.rs`)

| Method | Path | Request Body | Response | Description |
|--------|------|-------------|----------|-------------|
| `POST` | `/api/v1/train/start` | `TrainingConfig { epochs, batch_size, learning_rate, ... }` | `{ status: "started", run_id }` | Launch background training task against recorded CSI data |
| `POST` | `/api/v1/train/stop` | -- | `{ status: "stopped", run_id }` | Cancel the active training run via a stop signal |
| `GET` | `/api/v1/train/status` | -- | `TrainingStatus { phase, epoch, loss, ... }` | Current training state (idle, training, complete, failed) |
| `POST` | `/api/v1/train/pretrain` | `{ epochs?, learning_rate? }` | `{ status: "started", mode: "pretrain" }` | Start self-supervised contrastive pretraining (ADR-024) |
| `POST` | `/api/v1/train/lora` | `{ profile_name, epochs?, rank? }` | `{ status: "started", mode: "lora" }` | Start LoRA fine-tuning on a loaded base model |
| `WS` | `/ws/train/progress` | -- | Streaming `TrainingProgress` JSON | Epoch-level progress with loss, metrics, and ETA |

### State Management

All three modules share the server's `AppStateInner` via `Arc<RwLock<AppStateInner>>`. New fields added to `AppStateInner`:

```rust
/// Runtime state for a loaded RVF model (None if no model loaded).
pub loaded_model: Option<LoadedModelState>,

/// Runtime state for the active CSI recording session.
pub recording_state: RecordingState,

/// Runtime state for the active training run.
pub training_state: TrainingState,

/// Broadcast channel for training progress updates (consumed by WebSocket).
pub train_progress_tx: broadcast::Sender<TrainingProgress>,
```

Key design constraints:

- **Single writer**: Only one recording session can be active at a time. Starting a new recording while one is active returns an error.
- **Single model**: Only one model can be loaded at a time. Loading a new model implicitly unloads the previous one.
- **Background training**: Training runs in a spawned `tokio::task`. Progress is broadcast via a `tokio::sync::broadcast` channel. The WebSocket handler subscribes to this channel.
- **Auto-stop**: Recordings with a `duration_secs` parameter automatically stop after the specified elapsed time.

### Training Pipeline (No External Dependencies)

The training pipeline is implemented entirely in Rust without PyTorch or `tch` dependencies. The pipeline:

1. **Loads data**: Reads `.csi.jsonl` recording files from `data/recordings/`
2. **Extracts features**: Subcarrier variance (sliding window), temporal gradients, Goertzel frequency-domain power across 9 bands, and 3 global scalar features (mean amplitude, std, motion score)
3. **Trains model**: Regularised linear model via batch gradient descent targeting 17 COCO keypoints x 3 dimensions = 51 output targets
4. **Exports model**: Best checkpoint exported as `.rvf` container using `RvfBuilder`, stored in `data/models/`

This design means the sensing server is fully self-contained: a field operator can record CSI data, train a model, and load it for inference without any external tooling.

### File Layout

```
data/
├── models/                          # RVF model files
│   ├── wifi-densepose-v1.rvf       # Trained model container
│   └── wifi-densepose-v1.rvf       # (additional models...)
└── recordings/                      # CSI recording sessions
    ├── walking-20260303_140000.csi.jsonl      # Raw CSI frames (JSONL)
    ├── walking-20260303_140000.csi.meta.json  # Session metadata
    ├── standing-20260303_141500.csi.jsonl
    └── standing-20260303_141500.csi.meta.json
```

### Mobile App Fixes

Three defects were corrected in the Expo React Native mobile companion (`ui/mobile/`):

1. **WebSocket URL builder** (`src/services/ws.service.ts`): The URL construction logic previously hardcoded port `3001` for WebSocket connections. This was changed to derive the WebSocket port from the same-origin HTTP URL, using `window.location.port` on web and the configured server URL on native platforms. This ensures the mobile app connects to whatever port the sensing server is actually running on.

2. **Jest configuration** (`jest.config.js`): The `testPathIgnorePatterns` array previously contained an entry that matched the test directory itself, causing Jest to silently skip all test files. The pattern was corrected to only ignore `node_modules/`.

3. **Placeholder tests replaced**: All 25 mobile test files contained only `it.todo()` stubs. These were replaced with real test implementations covering:

   | Category | Test Files | Coverage |
   |----------|-----------|----------|
   | Utils | `format.test.ts`, `validation.test.ts` | Number formatting, URL validation, input sanitization |
   | Services | `ws.service.test.ts`, `api.service.test.ts` | WebSocket connection lifecycle, REST API calls, error handling |
   | Stores | `poseStore.test.ts`, `settingsStore.test.ts`, `matStore.test.ts` | Zustand state transitions, persistence, selector memoization |
   | Components | `BreathingGauge.test.tsx`, `HeartRateGauge.test.tsx`, `MetricCard.test.tsx`, `ConnectionBanner.test.tsx` | Rendering, prop validation, theme compliance |
   | Hooks | `useConnection.test.ts`, `useSensing.test.ts` | Hook lifecycle, cleanup, error states |
   | Screens | `LiveScreen.test.tsx`, `VitalsScreen.test.tsx`, `SettingsScreen.test.tsx` | Screen rendering, navigation, data binding |

---

## Rationale

### Why implement model/training/recording in the sensing server?

The alternative would be to run a separate Python training service and proxy requests. This was rejected for three reasons:

1. **Single-binary deployment**: WiFi-DensePose targets edge deployments (disaster response, building security, healthcare monitoring per ADR-034) where installing Python, pip, and PyTorch is impractical. A single Rust binary that handles sensing, recording, training, and inference is the correct architecture for field use.

2. **Zero-configuration UI**: The web UI is served by the same binary that exposes the API. When a user opens `http://server:8080/`, everything works -- no additional services to start, no ports to configure, no CORS to manage.

3. **Data locality**: CSI frames arrive via UDP, are processed for real-time display, and can simultaneously be written to disk for training. The recording module hooks directly into the CSI processing loop via `maybe_record_frame()`, avoiding any serialization overhead or inter-process communication.

### Why fix mobile in the same change?

The mobile app's WebSocket failure was caused by the same root problem -- assumptions about server port layout that did not match reality. Fixing the server API without fixing the mobile client would leave a broken user experience. The test fixes were included because the placeholder tests masked the WebSocket URL bug during development.

---

## Consequences

### Positive

- **UI loads with zero console errors**: All model, recording, and training tabs render correctly and receive real data from the server
- **End-to-end workflow**: Users can record CSI data, train a model, load it, and see pose estimation results -- all from the web UI without any external tools
- **LoRA fine-tuning support**: Users can adapt a base model to new environments via LoRA profiles, activated through the UI
- **Mobile app connects reliably**: The WebSocket URL builder uses same-origin port derivation, working correctly regardless of which port the server runs on
- **25 real mobile tests**: Provide actual regression protection for utils, services, stores, components, hooks, and screens
- **Self-contained sensing server**: No Python, PyTorch, or external training infrastructure required

### Negative

- **Sensing server binary grows**: The three new modules add approximately 2,000 lines of Rust to the sensing server crate, increasing compile time marginally
- **Training is lightweight**: The built-in training pipeline uses regularised linear regression, not deep learning. For production-grade pose estimation models, the full Python training pipeline (`wifi-densepose-train`) with PyTorch is still needed. The in-server training is designed for quick field calibration, not SOTA accuracy.
- **File-based storage**: Models and recordings are stored as files on the local filesystem (`data/models/`, `data/recordings/`). There is no database, no replication, and no access control. This is acceptable for single-node edge deployments but not for multi-user production environments.

### Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Disk fills up during long recording sessions | Medium | Medium | `duration_secs` auto-stop parameter; UI shows file size; manual `DELETE` endpoint |
| Concurrent model load/unload during inference causes race | Low | High | `RwLock` on `AppStateInner` serializes all state mutations; inference path acquires read lock |
| Training on insufficient data produces poor model | Medium | Low | Training API validates minimum frame count before starting; UI shows dataset statistics |
| JSONL recording format is inefficient for large datasets | Low | Low | Acceptable for field calibration (minutes of data); production datasets use the Python pipeline with HDF5 |

---

## Implementation

### Server-Side Changes

All 14 new handler functions were added directly to `main.rs` (~400 lines of new code). Key additions:

| Handler | Method | Path | Description |
|---------|--------|------|-------------|
| `list_models` | GET | `/api/v1/models` | Scans `data/models/` for `.rvf` files at startup, returns cached list |
| `get_active_model` | GET | `/api/v1/models/active` | Returns currently loaded model or `null` |
| `load_model` | POST | `/api/v1/models/load` | Sets `active_model_id` in state |
| `unload_model` | POST | `/api/v1/models/unload` | Clears `active_model_id` |
| `delete_model` | DELETE | `/api/v1/models/:id` | Removes model from disk and state |
| `list_lora_profiles` | GET | `/api/v1/models/lora/profiles` | Scans `data/models/lora/` directory |
| `activate_lora_profile` | POST | `/api/v1/models/lora/activate` | Activates a LoRA adapter |
| `list_recordings` | GET | `/api/v1/recording/list` | Scans `data/recordings/` for `.jsonl` files with frame counts |
| `start_recording` | POST | `/api/v1/recording/start` | Spawns tokio background task writing CSI frames to `.jsonl` |
| `stop_recording` | POST | `/api/v1/recording/stop` | Sends stop signal via `tokio::sync::watch`, returns duration |
| `delete_recording` | DELETE | `/api/v1/recording/:id` | Removes recording file from disk |
| `train_status` | GET | `/api/v1/train/status` | Returns training phase (idle/running/complete/failed) |
| `train_start` | POST | `/api/v1/train/start` | Sets training status to running with config |
| `train_stop` | POST | `/api/v1/train/stop` | Sets training status to idle |

Helper functions: `scan_model_files()`, `scan_lora_profiles()`, `scan_recording_files()`, `chrono_timestamp()`.

Startup creates `data/models/` and `data/recordings/` directories and populates initial state with scanned files.

### Web UI Fix

| File | Change | Description |
|------|--------|-------------|
| `ui/app.js` | Modified | Import `sensingService` and call `sensingService.start()` in `initializeServices()` after backend health check, so Dashboard and Live Demo tabs connect to `/ws/sensing` immediately on load instead of waiting for Sensing tab visit |
| `ui/services/sensing.service.js` | Comment | Updated comment documenting that `/ws/sensing` is on the same HTTP port |

### Mobile App Files

| File | Change | Description |
|------|--------|-------------|
| `ui/mobile/src/services/ws.service.ts` | Modified | `buildWsUrl()` uses `parsed.host` directly with `/ws/sensing` path instead of hardcoded port `3001` |
| `ui/mobile/jest.config.js` | Modified | `testPathIgnorePatterns` corrected to only ignore `node_modules/` |
| `ui/mobile/src/__tests__/*.test.ts{x}` | Replaced | 25 placeholder `it.todo()` tests replaced with real implementations |

---

## Verification

```bash
# 1. Start sensing server with auto source (simulated fallback)
cd rust-port/wifi-densepose-rs
cargo run -p wifi-densepose-sensing-server -- --http-port 3000 --source auto

# 2. Verify model endpoints return 200
curl -s http://localhost:3000/api/v1/models | jq '.count'
curl -s http://localhost:3000/api/v1/models/active | jq '.status'

# 3. Verify recording endpoints return 200
curl -s http://localhost:3000/api/v1/recording/list | jq '.count'
curl -s -X POST http://localhost:3000/api/v1/recording/start \
  -H 'Content-Type: application/json' \
  -d '{"session_name":"test","duration_secs":5}' | jq '.status'

# 4. Verify training endpoint returns 200
curl -s http://localhost:3000/api/v1/train/status | jq '.phase'

# 5. Verify LoRA endpoints return 200
curl -s http://localhost:3000/api/v1/models/lora/profiles | jq '.'

# 6. Open UI — check browser console for zero 404 errors
# Navigate to http://localhost:3000/ui/

# 7. Run mobile tests
cd ../../ui/mobile
npx jest --no-coverage

# 8. Run Rust workspace tests (must pass, 1031+ tests)
cd ../../rust-port/wifi-densepose-rs
cargo test --workspace --no-default-features
```

---

## References

- ADR-034: Expo React Native Mobile Application (mobile companion architecture)
- ADR-036: RVF Training Pipeline UI (training pipeline design)
- ADR-039: ESP32-S3 Edge Intelligence Pipeline (CSI frame format and processing tiers)
- ADR-040: WASM Programmable Sensing (Tier 3 edge compute)
- ADR-041: WASM Module Collection (module catalog)
- `crates/wifi-densepose-sensing-server/src/main.rs` -- all 14 new handler functions (model, recording, training)
- `ui/app.js` -- sensing service early initialization fix
- `ui/mobile/src/services/ws.service.ts` -- mobile WebSocket URL fix
