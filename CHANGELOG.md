# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- **Ghost skeletons in live UI with multi-node ESP32 setups** (#420, ADR-082) —
  `tracker_bridge::tracker_to_person_detections` documented itself as filtering
  to `is_alive()` tracks but in fact passed every non-Terminated track to the
  WebSocket stream. `Lost` tracks — kept inside `reid_window` for
  re-identification but not currently observed — were rendering as phantom
  skeletons, accumulating to 22-24 with 3 nodes × 10 Hz CSI while
  `estimated_persons` correctly reported 1. Added
  `PoseTracker::confirmed_tracks()` (Tentative + Active only) and rewired the
  bridge to use it. Lost tracks remain in the tracker for re-ID; they just
  no longer ship to the UI. Regression test:
  `test_lost_tracks_excluded_from_bridge_output`.
- **Rust workspace build with `--no-default-features` on Windows** (#366, #415) —
  `wifi-densepose-mat`, `wifi-densepose-sensing-server`, and `wifi-densepose-train`
  all depended on `wifi-densepose-signal` with default features enabled, which
  pulled `ndarray-linalg` → `openblas-src` → vcpkg/system-BLAS through the entire
  workspace. `--no-default-features` at the workspace root then could not opt out
  of BLAS, breaking `cargo build` / `cargo test` on Windows without vcpkg. All
  three consumers now declare `wifi-densepose-signal = { ..., default-features = false }`,
  so `cargo test --workspace --no-default-features` builds cleanly without
  vcpkg/openblas. Validated: 1,538 tests pass, 0 fail, 8 ignored.
- **`signal` test `test_estimate_occupancy_noise_only` failed without `eigenvalue`** —
  The test unwrapped the `NotCalibrated` stub returned when the BLAS-backed
  `estimate_occupancy` is compiled out. Gated with `#[cfg(feature = "eigenvalue")]`
  so it only runs when the real implementation is available.

## [v0.6.2-esp32] — 2026-04-20

Firmware release cutting ADR-081 and the Timer Svc stack fix discovered during
on-hardware validation. Cut from `main` at commit pointing to this entry.
Tested on ESP32-S3 (QFN56 rev v0.2, MAC `3c:0f:02:e9:b5:f8`), 30 s continuous
run: no crashes, 149 `rv_feature_state_t` emissions (~5 Hz), medium/slow ticks
firing cleanly, HEALTH mesh packets sent.

### Fixed
- **Firmware: Timer Svc stack overflow on ADR-081 fast loop** — `emit_feature_state()` runs inside the FreeRTOS Timer Svc task via the fast-loop callback; it calls `stream_sender` network I/O which pushes past the ESP-IDF 2 KiB default timer stack and panics ~1 s after boot. Bumped `CONFIG_FREERTOS_TIMER_TASK_STACK_DEPTH` to 8 KiB in `sdkconfig.defaults`, `sdkconfig.defaults.template`, and `sdkconfig.defaults.4mb`. Follow-up (tracked separately): move heavy work out of the timer daemon into a dedicated worker task.
- **Firmware: `adaptive_controller.c` implicit declaration** (#404) — `fast_loop_cb` called `emit_feature_state()` before its static definition, triggering `-Werror=implicit-function-declaration`. Added a forward declaration above the first use.

### Changed
- **CI: firmware build matrix (8MB + 4MB)** — `firmware-ci.yml` now matrix-builds both the default 8MB (`sdkconfig.defaults`) and 4MB SuperMini (`sdkconfig.defaults.4mb`) variants, uploading distinct artifacts and producing variant-named release binaries (`esp32-csi-node.bin` / `esp32-csi-node-4mb.bin`, `partition-table.bin` / `partition-table-4mb.bin`).

### Added
- **ADR-081: Adaptive CSI Mesh Firmware Kernel** — New 5-layer architecture
  (Radio Abstraction Layer / Adaptive Controller / Mesh Sensing Plane /
  On-device Feature Extraction / Rust handoff) that reframes the existing
  ESP32 firmware modules as components of a chipset-agnostic kernel. ADR
  in `docs/adr/ADR-081-adaptive-csi-mesh-firmware-kernel.md`. Goal: swap
  one radio family for another without changing the Rust signal /
  ruvector / train / mat crates.
- **Firmware: radio abstraction vtable (`rv_radio_ops_t`)** — New
  `firmware/esp32-csi-node/main/rv_radio_ops.{h}` defines the
  chipset-agnostic ops (init, set_channel, set_mode, set_csi_enabled,
  set_capture_profile, get_health), profile enum
  (`RV_PROFILE_PASSIVE_LOW_RATE` / `ACTIVE_PROBE` / `RESP_HIGH_SENS` /
  `FAST_MOTION` / `CALIBRATION`), and health snapshot struct.
  `rv_radio_ops_esp32.c` provides the ESP32 binding wrapping
  `csi_collector` + `esp_wifi_*`. A second binding (mock or alternate
  chipset) is the portability acceptance test for ADR-081.
- **Firmware: `rv_feature_state_t` packet (magic `0xC5110006`)** — New
  60-byte compact per-node sensing state (packed, verified by
  `_Static_assert`) in `firmware/esp32-csi-node/main/rv_feature_state.h`:
  motion, presence, respiration BPM/conf, heartbeat BPM/conf, anomaly
  score, env-shift score, node coherence, quality flags, IEEE CRC32.
  Replaces raw ADR-018 CSI as the default upstream stream (~99.7%
  bandwidth reduction: 300 B/s at 5 Hz vs. ~100 KB/s raw).
- **Firmware: mock radio ops binding for QEMU** — New
  `firmware/esp32-csi-node/main/rv_radio_ops_mock.c`, compiled only when
  `CONFIG_CSI_MOCK_ENABLED`. Satisfies ADR-081's portability acceptance
  test: a second `rv_radio_ops_t` binding compiles and runs against the
  same controller + mesh-plane code as the ESP32 binding.
- **Firmware: feature-state emitter wired into controller fast loop** —
  `adaptive_controller.c` now emits one 60-byte `rv_feature_state_t` per
  fast tick (default 200 ms → 5 Hz), pulling from the latest edge vitals
  and controller observation. This is the first end-to-end Layer 4/5
  path for ADR-081.
- **Firmware: `csi_collector_get_pkt_yield_per_sec()` /
  `_get_send_fail_count()` accessors** — Expose the CSI callback rate
  and UDP send-failure counter so the ESP32 radio ops binding can
  populate `rv_radio_health_t.pkt_yield_per_sec` and `.send_fail_count`,
  closing the adaptive controller's observation loop.
- **Firmware: host-side unit test suite for ADR-081 pure logic** — New
  `firmware/esp32-csi-node/tests/host/` (Makefile + 2 test files + shim
  `esp_err.h`). Exercises `adaptive_controller_decide()` (9 test cases:
  degraded gate on pkt-yield collapse + coherence loss, anomaly > motion,
  motion → SENSE_ACTIVE, aggressive cadence, stable presence →
  RESP_HIGH_SENS, empty-room default, hysteresis, NULL safety) and
  `rv_feature_state_*` helpers (size assertion, IEEE CRC32 known
  vectors, determinism, receiver-side verification). 33/33 assertions
  pass. Benchmarks: decide() 3.2 ns/call, CRC32(56 B) 614 ns/pkt
  (87 MB/s), full finalize() 616 ns/call. Pure function
  `adaptive_controller_decide()` extracted to
  `adaptive_controller_decide.c` so the firmware build and the host
  tests share a single source-of-truth implementation.
- **Scripts: `validate_qemu_output.py` ADR-081 checks** — Validator
  (invoked by ADR-061 `scripts/qemu-esp32s3-test.sh` in CI) gains three
  checks for adaptive controller boot line, mock radio ops
  registration, and slow-loop heartbeat, so QEMU runs regression-gate
  Layer 1/2 presence.
- **Firmware: ADR-081 Layer 3 mesh sensing plane** — New
  `firmware/esp32-csi-node/main/rv_mesh.{h,c}` defines 4 node roles
  (Anchor / Observer / Fusion relay / Coordinator), 7 on-wire message
  types (TIME_SYNC, ROLE_ASSIGN, CHANNEL_PLAN, CALIBRATION_START,
  FEATURE_DELTA, HEALTH, ANOMALY_ALERT), 3 authorization classes
  (None / HMAC-SHA256-session / Ed25519-batch), `rv_node_status_t`
  (28 B), `rv_anomaly_alert_t` (28 B), `rv_time_sync_t`,
  `rv_role_assign_t`, `rv_channel_plan_t`, `rv_calibration_start_t`.
  Pure-C encoder/decoder (`rv_mesh_encode()` / `rv_mesh_decode()`) with
  16-byte envelope + payload + IEEE CRC32 trailer; convenience encoders
  for each message type. Controller now emits `HEALTH` every slow-loop
  tick (30 s default) and `ANOMALY_ALERT` on state transitions to ALERT
  or DEGRADED. Host tests: `test_rv_mesh` exercises 27 assertions
  covering roundtrip, bad magic, truncation, CRC flipping, oversize
  payload rejection, and encode+decode throughput (1.0 μs/roundtrip
  on host).
- **Rust: ADR-081 Layer 1/3 mirror module** — New
  `crates/wifi-densepose-hardware/src/radio_ops.rs` mirrors the
  firmware-side `rv_radio_ops_t` vtable as the Rust `RadioOps` trait
  (init, set_channel, set_mode, set_csi_enabled, set_capture_profile,
  get_health) and provides `MockRadio` for offline testing.
  Also mirrors the `rv_mesh.h` types (`MeshHeader`, `NodeStatus`,
  `AnomalyAlert`, `MeshRole`, `MeshMsgType`, `AuthClass`) and ships
  byte-identical `crc32_ieee()`, `decode_mesh()`, `decode_node_status()`,
  `decode_anomaly_alert()`, and `encode_health()`. Exported from
  `lib.rs`. 8 unit tests pass; `crc32_matches_firmware_vectors`
  verifies parity with the firmware-side test vectors
  (`0xCBF43926` for `"123456789"`, `0xD202EF8D` for single-byte zero),
  and `mesh_constants_match_firmware` asserts `MESH_MAGIC`,
  `MESH_VERSION`, `MESH_HEADER_SIZE`, and `MESH_MAX_PAYLOAD` match
  `rv_mesh.h` byte-for-byte. Satisfies ADR-081's portability
  acceptance test: signal/ruvector/train/mat crates are untouched.
- **Firmware: adaptive controller** — New
  `firmware/esp32-csi-node/main/adaptive_controller.{c,h}` implements
  the three-loop closed-loop control specified by ADR-081: fast
  (~200 ms) for cadence and active probing, medium (~1 s) for channel
  selection and role transitions, slow (~30 s) for baseline
  recalibration. Pure `adaptive_controller_decide()` policy function is
  exposed in the header for offline unit testing. Default policy is
  conservative (`enable_channel_switch` and `enable_role_change` off);
  Kconfig surface added under "Adaptive Controller (ADR-081)".

### Fixed
- **`provision.py` esptool v5 compat** (#391) — Stale `write_flash` (underscore) syntax in the dry-run manual-flash hint now uses `write-flash` (hyphenated) for esptool >= 5.x. The primary flash command was already correct.
- **`provision.py` silent NVS wipe** (#391) — The script replaces the entire `csi_cfg` NVS namespace on every run, so partial invocations were silently erasing WiFi credentials and causing `Retrying WiFi connection (10/10)` in the field. Now refuses to run without `--ssid`, `--password`, and `--target-ip` unless `--force-partial` is passed. `--force-partial` prints a warning listing which keys will be wiped.
- **Firmware: defensive `node_id` capture** (#232, #375, #385, #386, #390) — Users on multi-node deployments reported `node_id` reverting to the Kconfig default (`1`) in UDP frames and in the `csi_collector` init log, despite NVS loading the correct value. The root cause (memory corruption of `g_nvs_config`) has not been definitively isolated, but the UDP frame header is now tamper-proof: `csi_collector_init()` captures `g_nvs_config.node_id` into a module-local `s_node_id` once, and `csi_serialize_frame()` plus all other consumers (`edge_processing.c`, `wasm_runtime.c`, `display_ui.c`, `swarm_bridge_init`) read it via the new `csi_collector_get_node_id()` accessor. A canary logs `WARN` if `g_nvs_config.node_id` diverges from `s_node_id` at end-of-init, helping isolate the upstream corruption path. Validated on attached ESP32-S3 (COM8): NVS `node_id=2` propagates through boot log, capture log, init log, and byte[4] of every UDP frame.

### Docs
- **CHANGELOG catch-up** (#367) — Added missing entries for v0.5.5, v0.6.0, and v0.7.0 releases.

## [v0.7.0] — 2026-04-06

Model release (no new firmware binary). Firmware remains at v0.6.0-esp32.

### Added
- **Camera ground-truth training pipeline (ADR-079)** — End-to-end supervised WiFlow pose training using MediaPipe + real ESP32 CSI.
  - `scripts/collect-ground-truth.py` — MediaPipe PoseLandmarker webcam capture (17 COCO keypoints, 30fps), synchronized with CSI recording over nanosecond timestamps.
  - `scripts/align-ground-truth.js` — Time-aligns camera keypoints with 20-frame CSI windows by binary search, confidence-weighted averaging.
  - `scripts/train-wiflow-supervised.js` — 3-phase curriculum training (contrastive → supervised SmoothL1 → bone/temporal refinement) with 4 scale presets (lite/small/medium/full).
  - `scripts/eval-wiflow.js` — PCK@10/20/50, MPJPE, per-joint breakdown, baseline proxy mode.
  - `scripts/record-csi-udp.py` — Lightweight ESP32 CSI UDP recorder (no Rust build required).
- **ruvector optimizations (O6-O10)** — Subcarrier selection (70→35, 50% reduction), attention-weighted subcarriers, Stoer-Wagner min-cut person separation, multi-SPSA gradient estimation, Mac M4 Pro training via Tailscale.
- **Scalable WiFlow presets** — `lite` (189K params, ~19 min) through `full` (7.7M params, ~8 hrs) to match dataset size.
- **Pre-trained WiFlow v1 model** — 92.9% PCK@20, 974 KB, 186,946 params. Published to [HuggingFace](https://huggingface.co/ruv/ruview) under `wiflow-v1/`.

### Validated
- **92.9% PCK@20** pose accuracy from a 5-minute data collection session with one $9 ESP32-S3 and one laptop webcam.
- Training pipeline validated on real paired data: 345 samples, 19 min training, eval loss 0.082, bone constraint 0.008.

## [v0.6.0-esp32] — 2026-04-03

### Added
- **Pre-trained CSI sensing weights published** — First official pre-trained models on [HuggingFace](https://huggingface.co/ruv/ruview). `model.safetensors` (48 KB), `model-q4.bin` (8 KB 4-bit), `model-q2.bin` (4 KB), `presence-head.json`, per-node LoRA adapters.
- **17 sensing applications** — Sleep monitor, apnea detector, stress monitor, gait analyzer, RF tomography, passive radar, material classifier, through-wall detector, device fingerprint, and more. Each as a standalone `scripts/*.js`.
- **ADRs 069-078** — 10 new architecture decisions covering Cognitum Seed integration, self-supervised pretraining, ruvllm pipeline, WiFlow architecture, channel hopping, SNN, MinCut person separation, CNN spectrograms, novel RF applications, multi-frequency mesh.
- **Kalman tracker** (PR #341 by @taylorjdawson) — temporal smoothing of pose keypoints.

### Fixed
- Security fix merged via PR #310.

### Performance
- Presence detection: 100% accuracy on 60,630 overnight samples.
- Inference: 0.008 ms per sample, 164K embeddings/sec.
- Contrastive self-supervised training: 51.6% improvement over baseline.

## [v0.5.5-esp32] — 2026-04-03

### Added
- **WiFlow SOTA architecture (ADR-072)** — TCN + axial attention pose decoder, 1.8M params, 881 KB at 4-bit. 17 COCO keypoints from CSI amplitude only (no phase).
- **Multi-frequency mesh scanning (ADR-073)** — ESP32 nodes hop across channels 1/3/5/6/9/11 at 200ms dwell. Neighbor WiFi networks used as passive radar illuminators. Null subcarriers reduced from 19% to 16%.
- **Spiking neural network (ADR-074)** — STDP online learning, adapts to new rooms in <30s with no labels, 16-160x less compute than batch training.
- **MinCut person counting (ADR-075)** — Stoer-Wagner min-cut on subcarrier correlation graph. Fixes #348 (was always reporting 4 people).
- **CNN spectrogram embeddings (ADR-076)** — Treat 64×20 CSI as an image, produce 128-dim environment fingerprints (0.95+ same-room similarity).
- **Graph transformer fusion** — Multi-node CSI fusion via GATv2 attention (replaces naive averaging).
- **Camera-free pose training pipeline** — Trains 17-keypoint model from 10 sensor signals with no camera required.

### Fixed
- **#348 person counting** — MinCut correctly counts 1-4 people (24/24 validation windows).

## [v0.5.4-esp32] — 2026-04-02

### Added
- **ADR-069: ESP32 CSI → Cognitum Seed RVF ingest pipeline** — Live-validated pipeline connecting ESP32-S3 CSI sensing to Cognitum Seed (Pi Zero 2 W) edge intelligence appliance. 339 vectors ingested, 100% kNN validation, SHA-256 witness chain verified.
- **Feature vector packet (magic 0xC5110003)** — New 48-byte packet with 8 normalized dimensions (presence, motion, breathing, heart rate, phase variance, person count, fall, RSSI) sent at 1 Hz alongside vitals.
- **`scripts/seed_csi_bridge.py`** — Python bridge: UDP listener → HTTPS ingest with bearer token auth, `--validate` (kNN + PIR ground truth), `--stats`, `--compact` modes, hash-based vector IDs, NaN/inf rejection, source IP filtering, retry logic.
- **Arena Physica research** — 26 research documents in `docs/research/` covering Maxwell's equations in WiFi sensing, Arena Physica Studio analysis, SOTA WiFi sensing 2025-2026, GOAP implementation plan for ESP32 + Pi Zero.
- **Cognitum Seed MCP integration** — 114-tool MCP proxy enables AI assistants to query sensing state, vectors, witness chain, and device status directly.

### Fixed
- **Compressed frame magic collision** — Reassigned compressed frame magic from `0xC5110003` to `0xC5110005` to free `0xC5110003` for feature vectors.
- **Uninitialized `s_top_k[0]` read** — Guarded variance computation against `s_top_k_count == 0` in `send_feature_vector()`.
- **Presence score normalization** — Bridge now divides by 15.0 instead of clamping, preserving dynamic range for raw values 1.41-14.92.
- **Stale magic references** — Updated ADR-039, DDD model to reflect `0xC5110005` for compressed frames.

### Security
- **Credential exposure remediation** — Removed hardcoded WiFi passwords and bearer tokens from source files. Added NVS binary/CSV patterns to `.gitignore`. Environment variable fallback for bearer token.
- **NaN/Inf injection prevention** — Bridge validates all feature dimensions are finite before Seed ingest.
- **UDP source filtering** — `--allowed-sources` argument restricts packet acceptance to known ESP32 IPs.

### Changed
- Wire format table now includes 6 magic numbers: `0xC5110001` (raw), `0xC5110002` (vitals), `0xC5110003` (features), `0xC5110004` (WASM events), `0xC5110005` (compressed), `0xC5110006` (fused vitals).

## [v0.5.3-esp32] — 2026-03-30

### Added
- **Cross-node RSSI-weighted feature fusion** — Multiple ESP32 nodes fuse CSI features using RSSI-based weighting. Closer node gets higher weight. Reduces variance noise by 29%, keypoint jitter by 72%.
- **DynamicMinCut person separation** — Uses `ruvector_mincut::DynamicMinCut` on the subcarrier temporal correlation graph to detect independent motion clusters. Replaces variance-based heuristic for multi-person counting.
- **RSSI-based position tracking** — Skeleton position driven by RSSI differential between nodes. Walk between ESP32s and the skeleton follows you.
- **Per-node state pipeline (ADR-068)** — Each ESP32 node gets independent `HashMap<u8, NodeState>` with frame history, classification, vitals, and person count. Fixes #249 (the #1 user-reported issue).
- **RuVector Phase 1-3 integration** — Subcarrier importance weighting, temporal keypoint smoothing (EMA), coherence gating, skeleton kinematic constraints (Jakobsen relaxation), compressed pose history.
- **Client-side lerp smoothing** — UI keypoints interpolate between frames (alpha=0.15) for fluid skeleton movement.
- **Multi-node mesh tests** — 8 integration tests covering 1-255 node configurations.
- **`wifi_densepose` Python package** — `from wifi_densepose import WiFiDensePose` now works (#314).

### Fixed
- **Watchdog crash on busy LANs (#321)** — Batch-limited edge_dsp to 4 frames before 20ms yield. Fixed idle-path busy-spin (`pdMS_TO_TICKS(5)==0`).
- **No detection from edge vitals (#323)** — Server now generates `sensing_update` from Tier 2+ vitals packets.
- **RSSI byte offset mismatch (#332)** — Server parsed RSSI from wrong byte (was reading sequence counter).
- **Stack overflow risk** — Moved 4KB of BPM scratch buffers from stack to static storage.
- **Stale node memory leak** — `node_states` HashMap evicts nodes inactive >60s.
- **Unsafe raw pointer removed** — Replaced with safe `.clone()` for adaptive model borrow.
- **Firmware CI** — Upgraded to IDF v5.4, replaced `xxd` with `od` (#327).
- **Person count double-counting** — Multi-node aggregation changed from `sum` to `max`.
- **Skeleton jitter** — Removed tick-based noise, dampened procedural animation, recalibrated feature scaling for real ESP32 data.

### Changed
- Motion-responsive skeleton: arm swing (0-80px) driven by CSI variance, leg kick (0-50px) by motion_band_power, vertical bob when walking.
- Person count thresholds recalibrated for real ESP32 hardware (1→2 at 0.70, EMA alpha 0.04).
- Vital sign filtering: larger median window (31), faster EMA (0.05), looser HR jump filter (15 BPM).
- Vendored ruvector updated to v2.1.0-40 (316 commits ahead).

### Benchmarks (2-node mesh, COM6 + COM9, 30s)
| Metric | Baseline | v0.5.3 | Improvement |
|--------|----------|--------|-------------|
| Variance noise | 109.4 | 77.6 | **-29%** |
| Feature stability | std=154.1 | std=105.4 | **-32%** |
| Keypoint jitter | std=4.5px | std=1.3px | **-72%** |
| Confidence | 0.643 | 0.686 | **+7%** |
| Presence accuracy | 93.4% | 94.6% | **+1.3pp** |

### Verified
- Real hardware: COM6 (node 1) + COM9 (node 2) on ruv.net WiFi
- All 284 Rust tests pass, 352 signal crate tests pass
- Firmware builds clean at 843 KB
- QEMU CI: 11/11 jobs green

## [v0.5.2-esp32] — 2026-03-28

### Fixed
- RSSI byte offset in frame parser (#332)
- Per-node state pipeline for multi-node sensing (#249)
- Firmware CI upgraded to IDF v5.4 (#327)

## [v0.5.1-esp32] — 2026-03-27

### Fixed
- Watchdog crash on busy LANs (#321)
- No detection from edge vitals (#323)
- `wifi_densepose` Python package import (#314)
- Pre-compiled firmware binaries added to release

## [v0.5.0-esp32] — 2026-03-15

### Added
- **60 GHz mmWave sensor fusion (ADR-063)** — Auto-detects Seeed MR60BHA2 (60 GHz, HR/BR/presence) and HLK-LD2410 (24 GHz, presence/distance) on UART at boot. Probes 115200 then 256000 baud, registers device capabilities, starts background parser.
- **48-byte fused vitals packet** (magic `0xC5110004`) — Kalman-style fusion: mmWave 80% + CSI 20% when both available. Automatic fallback to standard 32-byte CSI-only packet.
- **Server-side fusion bridge** (`scripts/mmwave_fusion_bridge.py`) — Reads two serial ports simultaneously for dual-sensor setups where mmWave runs on a separate ESP32.
- **Multimodal ambient intelligence roadmap (ADR-064)** — 25+ applications from fall detection to sleep monitoring to RF tomography.

### Verified
- Real hardware: ESP32-S3 (COM7) WiFi CSI + ESP32-C6/MR60BHA2 (COM4) 60 GHz mmWave running concurrently. HR=75 bpm, BR=25/min at 52 cm range. All 11 QEMU CI jobs green.

## [v0.4.3-esp32] — 2026-03-15

### Fixed
- **Fall detection false positives (#263)** — Default threshold raised from 2.0 to 15.0 rad/s²; normal walking (2-5 rad/s²) no longer triggers alerts. Added 3-consecutive-frame debounce and 5-second cooldown between alerts. Verified on real ESP32-S3 hardware: 0 false alerts in 60s / 1,300+ live WiFi CSI frames.
- **Kconfig default mismatch** — `CONFIG_EDGE_FALL_THRESH` Kconfig default was still 2000 (=2.0) while `nvs_config.c` fallback was updated to 15.0. Fixed Kconfig to 15000. Caught by real hardware testing — mock data did not reproduce.
- **provision.py NVS generator API change** — `esp_idf_nvs_partition_gen` package changed its `generate()` signature; switched to subprocess-first invocation for cross-version compatibility.
- **QEMU CI pipeline (11 jobs)** — Fixed all failures: fuzz test `esp_timer` stubs, QEMU `libgcrypt` dependency, NVS matrix generator, IDF container `pip` path, flash image padding, validation WARN handling, swarm `ip`/`cargo` missing.

### Added
- **4MB flash support (#265)** — `partitions_4mb.csv` and `sdkconfig.defaults.4mb` for ESP32-S3 boards with 4MB flash (e.g. SuperMini). Dual OTA slots, 1.856 MB each. Thanks to @sebbu for the community workaround that confirmed feasibility.
- **`--strict` flag** for `validate_qemu_output.py` — WARNs now pass by default in CI (no real WiFi in QEMU); use `--strict` to fail on warnings.

## [Unreleased]

### Added
- **QEMU ESP32-S3 testing platform (ADR-061)** — 9-layer firmware testing without hardware
  - Mock CSI generator with 10 physics-based scenarios (empty room, walking, fall, multi-person, etc.)
  - Single-node QEMU runner with 16-check UART validation
  - Multi-node TDM mesh simulation (TAP networking, 2-6 nodes)
  - GDB remote debugging with VS Code integration
  - Code coverage via gcov/lcov + apptrace
  - Fuzz testing (3 libFuzzer targets + ASAN/UBSAN)
  - NVS provisioning matrix (14 configs)
  - Snapshot-based regression testing (sub-second VM restore)
  - Chaos testing with fault injection + health monitoring
- **QEMU Swarm Configurator (ADR-062)** — YAML-driven multi-ESP32 test orchestration
  - 4 topologies: star, mesh, line, ring
  - 3 node roles: sensor, coordinator, gateway
  - 9 swarm-level assertions (boot, crashes, TDM, frame rate, fall detection, etc.)
  - 7 presets: smoke (2n/15s), standard (3n/60s), ci-matrix, large-mesh, line-relay, ring-fault, heterogeneous
  - Health oracle with cross-node validation
- **QEMU installer** (`install-qemu.sh`) — auto-detects OS, installs deps, builds Espressif QEMU fork
- **Unified QEMU CLI** (`qemu-cli.sh`) — single entry point for all 11 QEMU test commands
- CI: `firmware-qemu.yml` workflow with QEMU test matrix, fuzz testing, NVS validation, and swarm test jobs
- User guide: QEMU testing and swarm configurator section with plain-language walkthrough

### Fixed
- Firmware now boots in QEMU: WiFi/UDP/OTA/display guards for mock CSI mode
- 9 bugs in mock_csi.c (LFSR bias, MAC filter init, scenario loop, overflow burst timing)
- 23 bugs from ADR-061 deep review (inject_fault.py writes, CI cache, snapshot log corruption, etc.)
- 16 bugs from ADR-062 deep review (log filename mismatch, SLIRP port collision, heap false positives, etc.)
- All scripts: `--help` flags, prerequisite checks with install hints, standardized exit codes

- **Sensing server UI API completion (ADR-043)** — 14 fully-functional REST endpoints for model management, CSI recording, and training control
  - Model CRUD: `GET /api/v1/models`, `GET /api/v1/models/active`, `POST /api/v1/models/load`, `POST /api/v1/models/unload`, `DELETE /api/v1/models/:id`, `GET /api/v1/models/lora/profiles`, `POST /api/v1/models/lora/activate`
  - CSI recording: `GET /api/v1/recording/list`, `POST /api/v1/recording/start`, `POST /api/v1/recording/stop`, `DELETE /api/v1/recording/:id`
  - Training control: `GET /api/v1/train/status`, `POST /api/v1/train/start`, `POST /api/v1/train/stop`
  - Recording writes CSI frames to `.jsonl` files via tokio background task
  - Model/recording directories scanned at startup, state managed via `Arc<RwLock<AppStateInner>>`
- **ADR-044: Provisioning tool enhancements** — 5-phase plan for complete NVS coverage (7 missing keys), JSON config files, mesh presets, read-back/verify, and auto-detect
- **25 real mobile tests** replacing `it.todo()` placeholders — 205 assertions covering components, services, stores, hooks, screens, and utils
- **Project MERIDIAN (ADR-027)** — Cross-environment domain generalization for WiFi pose estimation (1,858 lines, 72 tests)
  - `HardwareNormalizer` — Catmull-Rom cubic interpolation resamples any hardware CSI to canonical 56 subcarriers; z-score + phase sanitization
  - `DomainFactorizer` + `GradientReversalLayer` — adversarial disentanglement of pose-relevant vs environment-specific features
  - `GeometryEncoder` + `FilmLayer` — Fourier positional encoding + DeepSets + FiLM for zero-shot deployment given AP positions
  - `VirtualDomainAugmentor` — synthetic environment diversity (room scale, wall material, scatterers, noise) for 4x training augmentation
  - `RapidAdaptation` — 10-second unsupervised calibration via contrastive test-time training + LoRA adapters
  - `CrossDomainEvaluator` — 6-metric evaluation protocol (MPJPE in-domain/cross-domain/few-shot/cross-hardware, domain gap ratio, adaptation speedup)
- ADR-027: Cross-Environment Domain Generalization — 10 SOTA citations (PerceptAlign, X-Fi ICLR 2025, AM-FM, DGSense, CVPR 2024)
- **Cross-platform RSSI adapters** — macOS CoreWLAN (`MacosCoreWlanScanner`) and Linux `iw` (`LinuxIwScanner`) Rust adapters with `#[cfg(target_os)]` gating
- macOS CoreWLAN Python sensing adapter with Swift helper (`mac_wifi.swift`)
- macOS synthetic BSSID generation (FNV-1a hash) for Sonoma 14.4+ BSSID redaction
- Linux `iw dev <iface> scan` parser with freq-to-channel conversion and `scan dump` (no-root) mode
- ADR-025: macOS CoreWLAN WiFi Sensing (ORCA)

### Fixed
- **sendto ENOMEM crash (Issue #127)** — CSI callbacks in promiscuous mode exhaust lwIP pbuf pool causing guru meditation crash. Fixed with 50 Hz rate limiter in `csi_collector.c` and 100 ms ENOMEM backoff in `stream_sender.c`. Hardware-verified on ESP32-S3 (200+ callbacks, zero crashes)
- **Provisioning script missing TDM/edge flags (Issue #130)** — Added `--tdm-slot`, `--tdm-total`, `--edge-tier`, `--pres-thresh`, `--fall-thresh`, `--vital-win`, `--vital-int`, `--subk-count` to `provision.py`
- **WebSocket "RECONNECTING" on Dashboard/Live Demo** — `sensingService.start()` now called on app init in `app.js` so WebSocket connects immediately instead of waiting for Sensing tab visit
- **Mobile WebSocket port** — `ws.service.ts` `buildWsUrl()` uses same-origin port instead of hardcoded port 3001
- **Mobile Jest config** — `testPathIgnorePatterns` no longer silently ignores the entire test directory
- Removed synthetic byte counters from Python `MacosWifiCollector` — now reports `tx_bytes=0, rx_bytes=0` instead of fake incrementing values

---

## [3.0.0] - 2026-03-01

Major release: AETHER contrastive embedding model, Docker Hub images, and comprehensive UI overhaul.

### Added — AETHER Contrastive Embedding Model (ADR-024)
- **Project AETHER** — self-supervised contrastive learning for WiFi CSI fingerprinting, similarity search, and anomaly detection (`9bbe956`)
- `embedding.rs` module: `ProjectionHead`, `InfoNceLoss`, `CsiAugmenter`, `FingerprintIndex`, `PoseEncoder`, `EmbeddingExtractor` (909 lines, zero external ML dependencies)
- SimCLR-style pretraining with 5 physically-motivated augmentations (temporal jitter, subcarrier masking, Gaussian noise, phase rotation, amplitude scaling)
- CLI flags: `--pretrain`, `--pretrain-epochs`, `--embed`, `--build-index <type>`
- Four HNSW-compatible fingerprint index types: `env_fingerprint`, `activity_pattern`, `temporal_baseline`, `person_track`
- Cross-modal `PoseEncoder` for WiFi-to-camera embedding alignment
- VICReg regularization for embedding collapse prevention
- 53K total parameters (55 KB at INT8) — fits on ESP32

### Added — Docker & Deployment
- Published Docker Hub images: `ruvnet/wifi-densepose:latest` (132 MB Rust) and `ruvnet/wifi-densepose:python` (569 MB) (`add9f19`)
- Multi-stage Dockerfile for Rust sensing server with RuVector crates
- `docker-compose.yml` orchestrating both Rust and Python services
- RVF model export via `--export-rvf` and load via `--load-rvf` CLI flags

### Added — Documentation
- 33 use cases across 4 vertical tiers: Everyday, Specialized, Robotics & Industrial, Extreme (`0afd9c5`)
- "Why WiFi Wins" comparison table (WiFi vs camera vs LIDAR vs wearable vs PIR)
- Mermaid architecture diagrams: end-to-end pipeline, signal processing detail, deployment topology (`50f0fc9`)
- Models & Training section with RuVector crate links (GitHub + crates.io), SONA component table (`965a1cc`)
- RVF container section with deployment targets table (ESP32 0.7 MB to server 50+ MB)
- Collapsible README sections for improved navigation (`478d964`, `99ec980`, `0ebd6be`)
- Installation and Quick Start moved above Table of Contents (`50acbf7`)
- CSI hardware requirement notice (`528b394`)

### Fixed
- **UI auto-detects server port from page origin** — no more hardcoded `localhost:8080`; works on any port (Docker :3000, native :8080, custom) (`3b72f35`, closes #55)
- **Docker port mismatch** — server now binds 3000/3001 inside container as documented (`44b9c30`)
- Added `/ws/sensing` WebSocket route to the HTTP server so UI only needs one port
- Fixed README API endpoint references: `/api/v1/health` → `/health`, `/api/v1/sensing` → `/api/v1/sensing/latest`
- Multi-person tracking limit corrected: configurable default 10, no hard software cap (`e2ce250`)

---

## [2.0.0] - 2026-02-28

Major release: complete Rust sensing server, full DensePose training pipeline, RuVector v2.0.4 integration, ESP32-S3 firmware, and 6 security hardening patches.

### Added — Rust Sensing Server
- **Full DensePose-compatible REST API** served by Axum (`d956c30`)
  - `GET /health` — server health
  - `GET /api/v1/sensing/latest` — live CSI sensing data
  - `GET /api/v1/vital-signs` — breathing rate (6-30 BPM) and heartbeat (40-120 BPM)
  - `GET /api/v1/pose/current` — 17 COCO keypoints derived from WiFi signal field
  - `GET /api/v1/info` — server build and feature info
  - `GET /api/v1/model/info` — RVF model container metadata
  - `ws://host/ws/sensing` — real-time WebSocket stream
- Three data sources: `--source esp32` (UDP CSI), `--source windows` (netsh RSSI), `--source simulated` (deterministic reference)
- Auto-detection: server probes ESP32 UDP and Windows WiFi, falls back to simulated
- Three.js visualization UI with 3D body skeleton, signal heatmap, phase plot, Doppler bars, vital signs panel
- Static UI serving via `--ui-path` flag
- Throughput: 9,520–11,665 frames/sec (release build)

### Added — ADR-021: Vital Sign Detection
- `VitalSignDetector` with breathing (6-30 BPM) and heartbeat (40-120 BPM) extraction from CSI fluctuations (`1192de9`)
- FFT-based spectral analysis with configurable band-pass filters
- Confidence scoring based on spectral peak prominence
- REST endpoint `/api/v1/vital-signs` with real-time JSON output

### Added — ADR-023: DensePose Training Pipeline (Phases 1-8)
- `wifi-densepose-train` crate with complete 8-phase pipeline (`fc409df`, `ec98e40`, `fce1271`)
  - Phase 1: `DataPipeline` with MM-Fi and Wi-Pose dataset loaders
  - Phase 2: `CsiToPoseTransformer` — 4-head cross-attention + 2-layer GCN on COCO skeleton
  - Phase 3: 6-term composite loss (MSE, bone length, symmetry, joint angle, temporal, confidence)
  - Phase 4: `DynamicPersonMatcher` via ruvector-mincut (O(n^1.5 log n) Hungarian assignment)
  - Phase 5: `SonaAdapter` — MicroLoRA rank-4 with EWC++ memory preservation
  - Phase 6: `SparseInference` — progressive 3-layer model loading (A: essential, B: refinement, C: full)
  - Phase 7: `RvfContainer` — single-file model packaging with segment-based binary format
  - Phase 8: End-to-end training with cosine-annealing LR, early stopping, checkpoint saving
- CLI: `--train`, `--dataset`, `--epochs`, `--save-rvf`, `--load-rvf`, `--export-rvf`
- Benchmark: ~11,665 fps inference, 229 tests passing

### Added — ADR-016: RuVector Training Integration (all 5 crates)
- `ruvector-mincut` → `DynamicPersonMatcher` in `metrics.rs` + subcarrier selection (`81ad09d`, `a7dd31c`)
- `ruvector-attn-mincut` → antenna attention in `model.rs` + noise-gated spectrogram
- `ruvector-temporal-tensor` → `CompressedCsiBuffer` in `dataset.rs` + compressed breathing/heartbeat
- `ruvector-solver` → sparse subcarrier interpolation (114→56) + Fresnel triangulation
- `ruvector-attention` → spatial attention in `model.rs` + attention-weighted BVP
- Vendored all 11 RuVector crates under `vendor/ruvector/` (`d803bfe`)

### Added — ADR-017: RuVector Signal & MAT Integration (7 integration points)
- `gate_spectrogram()` — attention-gated noise suppression (`18170d7`)
- `attention_weighted_bvp()` — sensitivity-weighted velocity profiles
- `mincut_subcarrier_partition()` — dynamic sensitive/insensitive subcarrier split
- `solve_fresnel_geometry()` — TX-body-RX distance estimation
- `CompressedBreathingBuffer` + `CompressedHeartbeatSpectrogram`
- `BreathingDetector` + `HeartbeatDetector` (MAT crate, real FFT + micro-Doppler)
- Feature-gated behind `cfg(feature = "ruvector")` (`ab2453e`)

### Added — ADR-018: ESP32-S3 Firmware & Live CSI Pipeline
- ESP32-S3 firmware with FreeRTOS CSI extraction (`92a5182`)
- ADR-018 binary frame format: `[0xAD, 0x18, len_hi, len_lo, payload]`
- Rust `Esp32Aggregator` receiving UDP frames on port 5005
- `bridge.rs` converting I/Q pairs to amplitude/phase vectors
- NVS provisioning for WiFi credentials
- Pre-built binary quick start documentation (`696a726`)

### Added — ADR-014: SOTA Signal Processing
- 6 algorithms, 83 tests (`fcb93cc`)
  - Hampel filter (median + MAD, resistant to 50% contamination)
  - Conjugate multiplication (reference-antenna ratio, cancels common-mode noise)
  - Phase sanitization (unwrap + linear detrend, removes CFO/SFO)
  - Fresnel zone geometry (TX-body-RX distance from first-principles physics)
  - Body Velocity Profile (micro-Doppler extraction, 5.7x speedup)
  - Attention-gated spectrogram (learned noise suppression)

### Added — ADR-015: Public Dataset Training Strategy
- MM-Fi and Wi-Pose dataset specifications with download links (`4babb32`, `5dc2f66`)
- Verified dataset dimensions, sampling rates, and annotation formats
- Cross-dataset evaluation protocol

### Added — WiFi-Mat Disaster Detection Module
- Multi-AP triangulation for through-wall survivor detection (`a17b630`, `6b20ff0`)
- Triage classification (breathing, heartbeat, motion)
- Domain events: `survivor_detected`, `survivor_updated`, `alert_created`
- WebSocket broadcast at `/ws/mat/stream`

### Added — Infrastructure
- Guided 7-step interactive installer with 8 hardware profiles (`8583f3e`)
- Comprehensive build guide for Linux, macOS, Windows, Docker, ESP32 (`45f8a0d`)
- 12 Architecture Decision Records (ADR-001 through ADR-012) (`337dd96`)

### Added — UI & Visualization
- Sensing-only UI mode with Gaussian splat visualization (`b7e0f07`)
- Three.js 3D body model (17 joints, 16 limbs) with signal-viz components
- Tabs: Dashboard, Hardware, Live Demo, Sensing, Architecture, Performance, Applications
- WebSocket client with automatic reconnection and exponential backoff

### Added — Rust Signal Processing Crate
- Complete Rust port of WiFi-DensePose with modular workspace (`6ed69a3`)
  - `wifi-densepose-signal` — CSI processing, phase sanitization, feature extraction
  - `wifi-densepose-core` — shared types and configuration
  - `wifi-densepose-nn` — neural network inference (DensePose head, RCNN)
  - `wifi-densepose-hardware` — ESP32 aggregator, hardware interfaces
  - `wifi-densepose-config` — configuration management
- Comprehensive benchmarks and validation tests (`3ccb301`)

### Added — Python Sensing Pipeline
- `WindowsWifiCollector` — RSSI collection via `netsh wlan show networks`
- `RssiFeatureExtractor` — variance, spectral bands (motion 0.5-4 Hz, breathing 0.1-0.5 Hz), change points
- `PresenceClassifier` — rule-based 3-state classification (ABSENT / PRESENT_STILL / ACTIVE)
- Cross-receiver agreement scoring for multi-AP confidence boosting
- WebSocket sensing server (`ws_server.py`) broadcasting JSON at 2 Hz
- Deterministic CSI proof bundles for reproducible verification (`archive/v1/data/proof/`)
- Commodity sensing unit tests (`b391638`)

### Changed
- Rust hardware adapters now return explicit errors instead of silent empty data (`6e0e539`)

### Fixed
- Review fixes for end-to-end training pipeline (`45f0304`)
- Dockerfile paths updated from `src/` to `archive/v1/src/` (`7872987`)
- IoT profile installer instructions updated for aggregator CLI (`f460097`)
- `process.env` reference removed from browser ES module (`e320bc9`)

### Performance
- 5.7x Doppler extraction speedup via optimized FFT windowing (`32c75c8`)
- Single 2.1 MB static binary, zero Python dependencies for Rust server

### Security
- Fix SQL injection in status command and migrations (`f9d125d`)
- Fix XSS vulnerabilities in UI components (`5db55fd`)
- Fix command injection in statusline.cjs (`4cb01fd`)
- Fix path traversal vulnerabilities (`896c4fc`)
- Fix insecure WebSocket connections — enforce wss:// on non-localhost (`ac094d4`)
- Fix GitHub Actions shell injection (`ab2e7b4`)
- Fix 10 additional vulnerabilities, remove 12 dead code instances (`7afdad0`)

---

## [1.1.0] - 2025-06-07

### Added
- Complete Python WiFi-DensePose system with CSI data extraction and router interface
- CSI processing and phase sanitization modules
- Batch processing for CSI data in `CSIProcessor` and `PhaseSanitizer`
- Hardware, pose, and stream services for WiFi-DensePose API
- Comprehensive CSS styles for UI components and dark mode support
- API and Deployment documentation

### Fixed
- Badge links for PyPI and Docker in README
- Async engine creation poolclass specification

---

## [1.0.0] - 2024-12-01

### Added
- Initial release of WiFi-DensePose
- Real-time WiFi-based human pose estimation using Channel State Information (CSI)
- DensePose neural network integration for body surface mapping
- RESTful API with comprehensive endpoint coverage
- WebSocket streaming for real-time pose data
- Multi-person tracking with configurable capacity (default 10, up to 50+)
- Fall detection and activity recognition
- Domain configurations: healthcare, fitness, smart home, security
- CLI interface for server management and configuration
- Hardware abstraction layer for multiple WiFi chipsets
- Phase sanitization and signal processing pipeline
- Authentication and rate limiting
- Background task management
- Cross-platform support (Linux, macOS, Windows)

### Documentation
- User guide and API reference
- Deployment and troubleshooting guides
- Hardware setup and calibration instructions
- Performance benchmarks
- Contributing guidelines

[Unreleased]: https://github.com/ruvnet/wifi-densepose/compare/v3.0.0...HEAD
[3.0.0]: https://github.com/ruvnet/wifi-densepose/compare/v2.0.0...v3.0.0
[2.0.0]: https://github.com/ruvnet/wifi-densepose/compare/v1.1.0...v2.0.0
[1.1.0]: https://github.com/ruvnet/wifi-densepose/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/ruvnet/wifi-densepose/releases/tag/v1.0.0
