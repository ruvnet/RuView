# Code Quality and Complexity Analysis Report

**Project:** wifi-densepose (ruview)
**Date:** 2026-04-05
**Analyzer:** QE Code Complexity Analyzer v3
**Scope:** Full codebase -- Rust, Python, C firmware, TypeScript/React Native

---

## Executive Summary

This report analyzes code complexity across the entire wifi-densepose project --
153,139 lines of Rust, 21,399 lines of Python, 7,987 lines of C firmware, and
7,457 lines of TypeScript/React Native. The analysis identified **231 Rust
functions with cyclomatic complexity > 10**, a single 4,846-line Rust file that
constitutes the most critical hotspot in the entire codebase, and systematic
code duplication patterns that inflate maintenance cost.

### Key Findings

| Metric | Rust | Python | C Firmware | TypeScript |
|--------|------|--------|------------|------------|
| Source files | 379 | 63 | 32 | 71 |
| Total lines | 153,139 | 21,399 | 7,987 | 7,457 |
| Functions analyzed | 6,641 | 888 | 145 | 97 |
| CC > 10 | 231 (3.5%) | 16 (1.8%) | 22 (15.2%) | 3 (3.1%) |
| CC > 20 | 74 (1.1%) | 0 | 5 (3.4%) | 1 (1.0%) |
| Functions > 50 lines | 282 (4.2%) | 49 (5.5%) | 26 (17.9%) | 3 (3.1%) |
| Functions > 100 lines | 81 (1.2%) | 6 (0.7%) | 6 (4.1%) | 1 (1.0%) |
| Files > 500 lines | 92 (24%) | 11 (17%) | 4 (25%) | 1 (1.4%) |
| Files > 1000 lines | 24 (6%) | 0 | 1 (6%) | 0 |
| Max nesting > 4 | 215 (3.2%) | 7 (0.8%) | 4 (2.8%) | 2 (2.1%) |

### Overall Quality Score: 62/100 (MODERATE)

The Python and TypeScript codebases are well-structured. The Rust codebase has
pockets of extreme complexity concentrated in the sensing server, and the C
firmware has proportionally the highest rate of complex functions.

---

## 1. Rust Codebase (153,139 lines, 17 crates)

### 1.1 Crate Size Breakdown

| Crate | Files | Lines | Assessment |
|-------|-------|-------|------------|
| wifi-densepose-wasm-edge | 68 | 28,888 | Largest; 68 vendor modules with repetitive `process_frame` |
| wifi-densepose-mat | 43 | 19,572 | Mass casualty assessment; moderate complexity |
| wifi-densepose-sensing-server | 18 | 17,825 | **CRITICAL** -- contains the worst hotspot |
| wifi-densepose-signal | 28 | 16,194 | RuvSense multistatic modules; well-decomposed |
| wifi-densepose-train | 18 | 10,562 | Training pipeline; moderate complexity |
| wifi-densepose-wifiscan | 23 | 5,779 | Multi-BSSID pipeline; clean architecture |
| wifi-densepose-ruvector | 16 | 4,629 | Cross-viewpoint fusion |
| wifi-densepose-hardware | 11 | 4,005 | ESP32 TDM protocol |
| wifi-densepose-desktop | 15 | 3,309 | Tauri desktop app |
| wifi-densepose-nn | 7 | 2,959 | Neural network inference |
| wifi-densepose-core | 5 | 2,596 | Core types and traits |
| Other (6 crates) | 14 | 4,987 | Small, well-sized |
| **Total** | **267** | **121,306** (src only) | |

### 1.2 Top 20 Most Complex Rust Functions

| Rank | CC | Lines | Depth | Function | File | Line |
|------|-----|-------|-------|----------|------|------|
| 1 | 121 | 776 | 8 | `main` | sensing-server/src/main.rs | 4070 |
| 2 | 66 | 422 | 8 | `udp_receiver_task` | sensing-server/src/main.rs | 3504 |
| 3 | 55 | 278 | 5 | `update` | mat/src/tracking/tracker.rs | 171 |
| 4 | 50 | 184 | 8 | `process_frame` | wasm-edge/src/med_seizure_detect.rs | 157 |
| 5 | 47 | 232 | 6 | `train_from_recordings` | sensing-server/src/adaptive_classifier.rs | 284 |
| 6 | 42 | 381 | 5 | `detect_format` | mat/src/integration/csi_receiver.rs | 815 |
| 7 | 41 | 78 | 4 | `deserialize_nvs_config` | desktop/src/commands/provision.rs | 345 |
| 8 | 41 | 169 | 4 | `process_frame` | wasm-edge/src/sec_perimeter_breach.rs | 140 |
| 9 | 40 | 472 | 6 | `real_training_loop` | sensing-server/src/training_api.rs | 825 |
| 10 | 37 | 153 | 6 | `process_frame` | wasm-edge/src/bld_lighting_zones.rs | 118 |
| 11 | 37 | 178 | 7 | `process_frame` | wasm-edge/src/ret_table_turnover.rs | 134 |
| 12 | 36 | 154 | 7 | `process_frame` | wasm-edge/src/lrn_dtw_gesture_learn.rs | 145 |
| 13 | 34 | 167 | 4 | `process_frame` | wasm-edge/src/exo_breathing_sync.rs | 197 |
| 14 | 34 | 170 | 4 | `process_frame` | wasm-edge/src/exo_ghost_hunter.rs | 198 |
| 15 | 33 | 134 | 5 | `process_frame` | wasm-edge/src/ind_structural_vibration.rs | 137 |
| 16 | 33 | 90 | 4 | `process_frame` | wasm-edge/src/ais_prompt_shield.rs | 65 |
| 17 | 32 | 144 | 5 | `process_frame` | wasm-edge/src/ret_shelf_engagement.rs | 163 |
| 18 | 32 | 174 | 5 | `process_frame` | wasm-edge/src/exo_plant_growth.rs | 170 |
| 19 | 31 | 129 | 6 | `process_frame` | wasm-edge/src/bld_meeting_room.rs | 98 |
| 20 | 31 | 125 | 5 | `process_frame` | wasm-edge/src/ret_dwell_heatmap.rs | 116 |

### 1.3 Critical Hotspot: `sensing-server/src/main.rs` (4,846 lines)

This is the single worst file in the entire codebase. At 4,846 lines, it is
**9.7x the project's 500-line guideline** and contains:

**God Object: `AppStateInner`** (lines 424-525)
- 40+ fields spanning unrelated concerns: vital signs, recording state, training
  state, adaptive model, per-node state, field model calibration, model management
- Violates Single Responsibility Principle -- mixes signal processing state,
  application lifecycle, network I/O, and persistence concerns

**Monolithic `main()` function** (lines 4070-4846)
- CC=121, 776 lines, nesting depth 8
- Handles CLI dispatch (benchmark, export, pretrain, embed, build-index, train,
  server startup) all in one function
- Should be decomposed into at least 8 separate command handlers

**`udp_receiver_task()` function** (lines 3504-3926)
- CC=66, 422 lines, nesting depth 8
- Handles three different packet types (vitals 0xC511_0002, WASM 0xC511_0004,
  CSI 0xC511_0001) in a single monolithic match chain
- Each branch duplicates the full sensing update construction and broadcast logic

**Systematic Code Duplication (6 instances):**
- `smooth_and_classify` / `smooth_and_classify_node` -- identical logic, differs
  only in operating on `AppStateInner` vs `NodeState` (could use a trait)
- `smooth_vitals` / `smooth_vitals_node` -- same pattern, identical algorithm
  duplicated for `AppStateInner` vs `NodeState`
- `SensingUpdate` construction -- built identically in 6 different places
  (WiFi task, WiFi fallback, simulate task, ESP32 CSI handler, ESP32 vitals
  handler, broadcast tick)
- Person count estimation -- repeated in WiFi, ESP32, and simulate paths

### 1.4 Code Smell: `wasm-edge` Vendor Modules

The `wifi-densepose-wasm-edge` crate contains 68 files (28,888 lines), with
nearly every module implementing a `process_frame` function following the same
pattern. At least 20 of these have CC > 25. This is a textbook case for:
- Extracting a common `process_frame` trait with shared scaffolding
- Using a generic signal pipeline builder

### 1.5 Oversized Rust Files (> 500 lines, violating project guideline)

92 Rust files exceed the 500-line guideline. The worst offenders:

| Lines | File |
|-------|------|
| 4,846 | sensing-server/src/main.rs |
| 1,946 | sensing-server/src/training_api.rs |
| 1,673 | wasm/src/mat.rs |
| 1,664 | train/src/metrics.rs |
| 1,523 | signal/src/ruvsense/pose_tracker.rs |
| 1,498 | sensing-server/src/embedding.rs |
| 1,430 | ruvector/src/crv/mod.rs |
| 1,401 | mat/src/integration/csi_receiver.rs |
| 1,360 | mat/src/integration/hardware_adapter.rs |
| 1,346 | signal/src/ruvsense/field_model.rs |

### 1.6 Dependency Analysis

No circular dependencies detected. The dependency graph is clean and follows
the documented crate publishing order. Maximum depth is 3 (CLI -> MAT -> core/signal/nn).

---

## 2. Python Codebase (21,399 lines, 63 files)

### 2.1 Overall Assessment: GOOD

The Python codebase is significantly better structured than the Rust codebase.
Only 16 functions (1.8%) exceed CC=10, and no function exceeds CC=20. The code
follows clean separation of concerns with distinct layers (api, services, core,
hardware, middleware, sensing).

### 2.2 Top 10 Most Complex Python Functions

| Rank | CC | Lines | Depth | Function | File | Line |
|------|-----|-------|-------|----------|------|------|
| 1 | 19 | 90 | 4 | `estimate_poses` | services/pose_service.py | 491 |
| 2 | 18 | 126 | 6 | `_print_text_status` | commands/status.py | 350 |
| 3 | 15 | 72 | 4 | `websocket_events_stream` | api/routers/stream.py | 156 |
| 4 | 14 | 100 | 3 | `health_check` | database/connection.py | 349 |
| 5 | 14 | 47 | 3 | `get_overall_health` | services/health_check.py | 384 |
| 6 | 13 | 52 | 3 | `_authenticate_request` | middleware/auth.py | 236 |
| 7 | 13 | 64 | 4 | `_handle_preflight` | middleware/cors.py | 89 |
| 8 | 13 | 84 | 4 | `websocket_pose_stream` | api/routers/stream.py | 69 |
| 9 | 13 | 65 | 4 | `generate_signal_field` | sensing/ws_server.py | 236 |
| 10 | 13 | 74 | 6 | `create_collector` | sensing/rssi_collector.py | 770 |

### 2.3 Files Exceeding 500 Lines

| Lines | File | Concern |
|-------|------|---------|
| 856 | services/pose_service.py | Pose estimation service -- acceptable for a service class |
| 843 | sensing/rssi_collector.py | RSSI collection with 3 collector implementations |
| 772 | tasks/monitoring.py | Background monitoring tasks |
| 640 | database/connection.py | Database connection management |
| 620 | cli.py | CLI command handler |
| 610 | tasks/backup.py | Backup task logic |
| 598 | tasks/cleanup.py | Cleanup task logic |
| 519 | sensing/ws_server.py | WebSocket server |
| 515 | hardware/csi_extractor.py | CSI data extraction |
| 510 | commands/status.py | Status reporting |
| 504 | middleware/error_handler.py | Error handling middleware |

### 2.4 Observations

- **Well-typed**: Uses type hints consistently throughout
- **Clean separation**: API routers, services, core, and middleware are distinct
- **Moderate nesting**: Only 7 functions (0.8%) exceed nesting depth 4
- **Minor concern**: `_print_text_status` (CC=18, 126 lines) in `commands/status.py`
  is essentially a large formatting function that could be split into per-component
  formatters

---

## 3. C Firmware (7,987 lines, 32 files)

### 3.1 Overall Assessment: MODERATE

The C firmware has the highest proportion of complex functions (15.2% with CC>10).
This is partly expected for embedded C, but several functions warrant attention.

### 3.2 Top 10 Most Complex C Functions

| Rank | CC | Lines | Depth | Function | File | Line |
|------|-----|-------|-------|----------|------|------|
| 1 | 59 | 314 | 3 | `nvs_config_load` | nvs_config.c | 19 |
| 2 | 40 | 185 | 3 | `process_frame` | edge_processing.c | 708 |
| 3 | 25 | 125 | 5 | `display_ui_update` | display_ui.c | 259 |
| 4 | 22 | 94 | 3 | `mock_timer_cb` | mock_csi.c | 518 |
| 5 | 22 | 174 | 3 | `app_main` | main.c | 127 |
| 6 | 21 | 136 | 3 | `rvf_parse` | rvf_parser.c | 33 |
| 7 | 19 | 119 | 3 | `wasm_runtime_load` | wasm_runtime.c | 442 |
| 8 | 18 | 84 | 3 | `send_vitals_packet` | edge_processing.c | 554 |
| 9 | 17 | 74 | 4 | `update_multi_person_vitals` | edge_processing.c | 474 |
| 10 | 17 | 34 | 3 | `ld2410_feed_byte` | mmwave_sensor.c | 274 |

### 3.3 Critical Hotspot: `nvs_config_load` (CC=59, 314 lines)

This function in `nvs_config.c` has the highest complexity of any C function.
It loads 30+ configuration parameters from NVS flash storage, each with its own
error handling and default-value fallback. This is a classic case for:
- Table-driven configuration loading with a descriptor array
- Macro-based parameter definition to eliminate repetition

### 3.4 `edge_processing.c` (1,067 lines)

This is the only C file exceeding 1,000 lines. It implements the full dual-core
CSI processing pipeline (11 processing stages). The `process_frame` function
(CC=40, 185 lines) combines phase extraction, variance tracking, subcarrier
selection, bandpass filtering, BPM estimation, presence detection, and fall
detection in a single function.

### 3.5 Stack Safety Concern

The code documents that `process_frame` + `update_multi_person_vitals` combined
used 6.5-7.5 KB of the 8 KB task stack, necessitating static scratch buffers.
This indicates the functions are pushing resource limits and should be
decomposed for safety margin.

---

## 4. TypeScript/React Native (7,457 lines, 71 files)

### 4.1 Overall Assessment: GOOD

The UI codebase is the cleanest in the project. Only 3 functions exceed CC=10,
no file exceeds 1,000 lines, and the component architecture follows React
best practices with proper separation of screens, components, stores, and services.

### 4.2 Critical Hotspot: `GaussianSplatWebView.web.tsx` (CC=70, 747 lines)

This is the only significant complexity hotspot in the TypeScript codebase.
The `GaussianSplatWebViewWeb` component (CC=70, 467 lines) manages:
- Three.js scene initialization and teardown
- Multi-person skeleton rendering with DensePose-style body parts
- Signal field visualization
- Animation loop management
- Frame data parsing and keypoint mapping

This component should be decomposed into:
- A Three.js scene manager (initialization, camera, lighting, animation)
- A skeleton renderer (body parts, keypoints, bones)
- A signal field renderer (grid, heatmap)
- A data adapter (frame parsing, person mapping)

### 4.3 Well-Structured Patterns

- **Zustand stores** (`poseStore.ts`, `matStore.ts`, `settingsStore.ts`): Clean
  state management with proper typing
- **Custom hooks** (`useMatBridge`, `useOccupancyGrid`, `useGaussianBridge`):
  Good separation of WebSocket logic from UI components
- **Component decomposition**: Screens are split into sub-components
  (AlertCard, SurvivorCounter, MetricCard, etc.)

---

## 5. Top 20 Hotspots (Cross-Codebase, Risk-Ranked)

Hotspots are ranked by a composite score combining complexity, file size,
nesting depth, and duplication density.

| Rank | Risk | CC | Lines | File | Function | Primary Issue |
|------|------|----|-------|------|----------|---------------|
| 1 | 0.98 | 121 | 776 | sensing-server/main.rs:4070 | `main` | God function; CLI dispatch |
| 2 | 0.96 | -- | 4,846 | sensing-server/main.rs | (file) | God file; 9.7x guideline |
| 3 | 0.94 | 66 | 422 | sensing-server/main.rs:3504 | `udp_receiver_task` | 3 packet types monolithic |
| 4 | 0.90 | -- | 40+ fields | sensing-server/main.rs:424 | `AppStateInner` | God object |
| 5 | 0.87 | 59 | 314 | nvs_config.c:19 | `nvs_config_load` | Needs table-driven approach |
| 6 | 0.85 | 55 | 278 | mat/tracking/tracker.rs:171 | `update` | Complex tracking logic |
| 7 | 0.82 | 50 | 184 | wasm-edge/med_seizure_detect.rs:157 | `process_frame` | Deep nesting (8) |
| 8 | 0.80 | 70 | 467 | GaussianSplatWebView.web.tsx:277 | `GaussianSplatWebViewWeb` | Three.js god component |
| 9 | 0.78 | 47 | 232 | sensing-server/adaptive_classifier.rs:284 | `train_from_recordings` | Complex training logic |
| 10 | 0.76 | 42 | 381 | mat/csi_receiver.rs:815 | `detect_format` | Format detection chain |
| 11 | 0.75 | 40 | 472 | sensing-server/training_api.rs:825 | `real_training_loop` | Long training loop |
| 12 | 0.73 | 40 | 185 | edge_processing.c:708 | `process_frame` | 11-stage DSP in one func |
| 13 | 0.70 | -- | 6x | sensing-server/main.rs | `SensingUpdate` builds | Duplicated 6 times |
| 14 | 0.68 | 19 | 90 | services/pose_service.py:491 | `estimate_poses` | Highest Python CC |
| 15 | 0.65 | -- | 1,946 | sensing-server/training_api.rs | (file) | 3.9x guideline |
| 16 | 0.63 | -- | 1,673 | wasm/mat.rs | (file) | 3.3x guideline |
| 17 | 0.61 | -- | 1,664 | train/metrics.rs | (file) | 3.3x guideline |
| 18 | 0.59 | -- | 1,523 | signal/ruvsense/pose_tracker.rs | (file) | 3.0x guideline |
| 19 | 0.57 | 25 | 125 | display_ui.c:259 | `display_ui_update` | Deep nesting (5) |
| 20 | 0.55 | 28 | 106 | sensing-server/main.rs:2161 | `estimate_persons_from_correlation` | Complex graph algorithm |

---

## 6. Code Smell Catalog

### 6.1 God Class / God File

| Smell | Location | Severity |
|-------|----------|----------|
| God File | sensing-server/main.rs (4,846 lines) | CRITICAL |
| God Object | `AppStateInner` (40+ fields) | CRITICAL |
| God Function | `main()` (776 lines, CC=121) | CRITICAL |
| God Function | `udp_receiver_task()` (422 lines, CC=66) | HIGH |

### 6.2 Duplicated Code

| Pattern | Instances | Lines Duplicated | Severity |
|---------|-----------|-----------------|----------|
| `smooth_and_classify` / `smooth_and_classify_node` | 2 | ~50 per copy | HIGH |
| `smooth_vitals` / `smooth_vitals_node` | 2 | ~50 per copy | HIGH |
| `SensingUpdate {}` construction | 6 | ~40 per instance | HIGH |
| Person count estimation pattern | 3+ | ~15 per instance | MEDIUM |
| `frame_history` capacity check | 6+ | ~3 per instance | LOW |
| `tracker_bridge::tracker_update` call pattern | 5 | ~5 per instance | MEDIUM |

Estimated duplicated code in `main.rs` alone: **~450 lines** (9.3% of file).

### 6.3 Deep Nesting (> 4 levels)

215 Rust functions exceed 4 levels of nesting. The worst cases:
- `main()`: 8 levels (lines 4070-4846)
- `udp_receiver_task()`: 8 levels (lines 3504-3926)
- Multiple `process_frame` in wasm-edge: 7-8 levels

### 6.4 Long Parameter Lists (> 5 parameters)

43 Rust functions have more than 5 parameters. Notable:
- `process_frame` variants in wasm-edge: 5-7 parameters each
- `extract_features_from_frame`: 3 parameters but returns a 5-tuple

### 6.5 Repetitive Vendor Modules (wasm-edge)

The `wifi-densepose-wasm-edge` crate has 68 files following a near-identical
pattern. At least 35 have a `process_frame` function with CC > 20. A trait-based
or macro-based approach would reduce this to a fraction of the code.

---

## 7. Testability Assessment

| Component | Score | Rating | Key Blockers |
|-----------|-------|--------|-------------|
| wifi-densepose-core | 85/100 | EASY | Pure types, no side effects |
| wifi-densepose-signal | 78/100 | EASY | Mostly pure computation |
| wifi-densepose-train | 72/100 | MODERATE | External dataset dependencies |
| wifi-densepose-mat | 68/100 | MODERATE | Integration with core+signal+nn |
| wifi-densepose-wifiscan | 75/100 | EASY | Platform-specific but well-abstracted |
| wifi-densepose-sensing-server | 32/100 | VERY DIFFICULT | God object, coupled state, async |
| wifi-densepose-wasm-edge | 55/100 | MODERATE | Repetitive but self-contained |
| archive/v1/src (Python) | 70/100 | MODERATE | Good DI, some tight coupling |
| firmware (C) | 40/100 | DIFFICULT | Hardware deps, global state |
| ui/mobile (TypeScript) | 72/100 | MODERATE | Component isolation is good |

---

## 8. Refactoring Recommendations

### Priority 1: CRITICAL -- sensing-server/main.rs Decomposition

**Estimated effort:** 3-5 days
**Impact:** Reduces maintenance cost for the most-changed file in the project

1. **Extract `AppStateInner` into bounded contexts:**
   - `SensingState` -- frame history, features, classification
   - `VitalSignState` -- HR/BR smoothing, detector, buffers
   - `RecordingState` -- recording lifecycle, file handles
   - `TrainingState` -- training status, config
   - `ModelState` -- loaded model, progressive loader, SONA profiles
   - `NodeRegistry` -- per-node states, pose tracker, multistatic fuser

2. **Extract command handlers from `main()`:**
   - `run_benchmark()` (lines 4082-4089)
   - `run_export_rvf()` (lines 4092-4142)
   - `run_pretrain()` (lines 4145-4247)
   - `run_embed()` (lines 4250-4312)
   - `run_build_index()` (lines 4315-4357)
   - `run_train()` (lines 4360-end)
   - `run_server()` -- the remaining server startup

3. **Extract `SensingUpdate` builder:**
   Create a `SensingUpdateBuilder` that encapsulates the repeated 6-instance
   construction pattern.

4. **Unify node vs global variants via trait:**
   ```rust
   trait SmoothingState {
       fn smoothed_motion(&self) -> f64;
       fn set_smoothed_motion(&mut self, v: f64);
       // ... etc
   }
   impl SmoothingState for AppStateInner { ... }
   impl SmoothingState for NodeState { ... }
   ```
   Then a single `smooth_and_classify<S: SmoothingState>()` replaces both copies.

5. **Extract `udp_receiver_task` into packet-type handlers:**
   - `handle_vitals_packet()`
   - `handle_wasm_packet()`
   - `handle_csi_frame()`

### Priority 2: HIGH -- C Firmware `nvs_config_load` Table-Driven Refactor

**Estimated effort:** 1 day
**Impact:** Reduces CC from 59 to approximately 5

Replace the 314-line sequential NVS load with a descriptor table:
```c
typedef struct {
    const char *key;
    nvs_type_t type;
    void *dest;
    size_t size;
    const void *default_val;
} nvs_param_desc_t;

static const nvs_param_desc_t params[] = {
    {"node_id", NVS_U8, &cfg->node_id, 1, &(uint8_t){1}},
    // ... 30+ entries
};
```

### Priority 3: HIGH -- wasm-edge `process_frame` Trait Extraction

**Estimated effort:** 2-3 days
**Impact:** Reduces 28,888 lines by an estimated 30-40%

Define a common trait:
```rust
trait WasmEdgeModule {
    fn name(&self) -> &str;
    fn init(&mut self, config: &ModuleConfig);
    fn process_frame(&mut self, ctx: &mut FrameContext) -> Vec<WasmEvent>;
}
```
Extract shared signal processing (phase extraction, variance tracking, BPM
estimation) into reusable pipeline stages.

### Priority 4: MEDIUM -- GaussianSplatWebView.web.tsx Decomposition

**Estimated effort:** 1 day
**Impact:** Reduces CC from 70 to approximately 10-15 per component

Split into:
- `SceneManager` -- Three.js initialization, camera, lighting
- `SkeletonRenderer` -- body parts, keypoints, bones
- `SignalFieldRenderer` -- grid, heatmap visualization
- `useFrameAdapter` -- data parsing hook

### Priority 5: MEDIUM -- `edge_processing.c` Pipeline Decomposition

**Estimated effort:** 1-2 days
**Impact:** Reduces `process_frame` CC from 40 to ~10; improves stack safety

Split into stage functions:
```c
static void stage_phase_extract(frame_ctx_t *ctx);
static void stage_variance_update(frame_ctx_t *ctx);
static void stage_subcarrier_select(frame_ctx_t *ctx);
static void stage_bandpass_filter(frame_ctx_t *ctx);
static void stage_bpm_estimate(frame_ctx_t *ctx);
static void stage_presence_detect(frame_ctx_t *ctx);
static void stage_fall_detect(frame_ctx_t *ctx);
```

### Priority 6: LOW -- Python Status Formatter Decomposition

**Estimated effort:** 0.5 days
**Impact:** Reduces `_print_text_status` CC from 18 to ~5 per formatter

Split `_print_text_status` (126 lines) into per-component formatters:
`_format_api_status`, `_format_hardware_status`, `_format_streaming_status`, etc.

---

## 9. Quality Gate Recommendations

### Proposed Complexity Thresholds for CI/CD

| Metric | Warn | Fail | Current Violations |
|--------|------|------|--------------------|
| File size | > 500 lines | > 1,000 lines | 92 warn, 25 fail |
| Function CC | > 15 | > 25 | ~150 warn, ~74 fail |
| Function lines | > 50 | > 100 | ~360 warn, ~94 fail |
| Nesting depth | > 4 | > 6 | ~215 warn, ~30 fail |
| Parameter count | > 5 | > 7 | ~43 warn, ~10 fail |

### Recommended Immediate Actions

1. **Block new functions with CC > 25** in CI (addresses future growth)
2. **Block new files exceeding 500 lines** (enforces project guideline)
3. **Add complexity linting** via `cargo clippy` with custom lints or `complexity-rs`
4. **Prioritize the sensing-server decomposition** -- it is the single largest
   contributor to technical debt in the project

---

## 10. Complexity Distribution Charts (Text)

### Rust Cyclomatic Complexity Distribution

```
CC Range    | Functions | Percentage | Bar
------------|-----------|------------|----------------------------------
  1-5       |     5,728 |     86.2%  | ####################################
  6-10      |       682 |     10.3%  | ####
 11-15      |       107 |      1.6%  | #
 16-20      |        50 |      0.8%  | 
 21-30      |        41 |      0.6%  | 
 31-50      |        24 |      0.4%  | 
   >50      |         9 |      0.1%  | 
```

### Python Cyclomatic Complexity Distribution

```
CC Range    | Functions | Percentage | Bar
------------|-----------|------------|----------------------------------
  1-5       |       740 |     83.3%  | ####################################
  6-10      |       132 |     14.9%  | ######
 11-15      |        13 |      1.5%  | #
 16-20      |         3 |      0.3%  | 
```

### C Firmware Cyclomatic Complexity Distribution

```
CC Range    | Functions | Percentage | Bar
------------|-----------|------------|----------------------------------
  1-5       |        73 |     50.3%  | ####################################
  6-10      |        50 |     34.5%  | #########################
 11-15      |         6 |      4.1%  | ###
 16-20      |         8 |      5.5%  | ####
 21-30      |         3 |      2.1%  | ##
   >30      |         5 |      3.4%  | ##
```

---

## Appendix A: Methodology

### Metrics Calculated

- **Cyclomatic Complexity (CC):** McCabe's cyclomatic complexity counting
  decision points (if, else if, match, for, while, boolean operators, match arms)
- **Cognitive Complexity:** Approximated via nesting depth and CC combination
- **Function Length:** Raw line count from function signature to closing brace
- **Nesting Depth:** Maximum brace/indent depth within function body
- **Parameter Count:** Number of non-self parameters
- **File Size:** Total lines including comments and blank lines

### Tools Used

- Custom Python AST analysis for Python files
- Custom regex-based analysis for Rust, C, and TypeScript files
- AST parsing provides higher accuracy for Python; regex-based analysis may
  slightly overcount CC for Rust (e.g., match arms in comments) but provides
  consistent cross-language comparison

### Limitations

- CC for Rust match arms counted via `=>` may include non-decision match arms
- TypeScript analysis captures top-level and exported functions but may miss
  deeply nested callbacks
- C analysis requires function signatures to start at column 0
- Dead code detection is heuristic-only (unused imports not checked at scale)

---

*Report generated by QE Code Complexity Analyzer v3*
*Codebase snapshot: commit 85434229 on branch qe-reports*
