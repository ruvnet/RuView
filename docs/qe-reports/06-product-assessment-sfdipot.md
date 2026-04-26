# SFDIPOT Product Factors Assessment: wifi-densepose

**Assessment Date:** 2026-04-05
**Assessor:** QE Product Factors Assessor (HTSM v6.3)
**Framework:** James Bach's Heuristic Test Strategy Model -- Product Factors (SFDIPOT)
**Scope:** Full wifi-densepose system -- Rust workspace (18 crates, 153k LoC), Python v1 (105 files, 39k LoC), ESP32 firmware (48 files, 1.6k LoC), CI/CD pipelines (8 workflows)
**Test Count:** 2,618 Rust `#[test]` functions + 33 Python test files

---

## Executive Summary

The wifi-densepose project is an ambitious WiFi-based human pose estimation system spanning five deployment targets (server, desktop, WASM/browser, ESP32 embedded, mobile). This SFDIPOT assessment identifies **47 risk areas** across all seven product factors. The highest concentration of risk lies in **Time** (real-time processing constraints with no latency testing), **Platform** (6 target architectures with limited cross-platform validation), and **Interfaces** (multiple protocol boundaries with incomplete contract testing).

**Overall Risk Rating: HIGH** -- The system's safety-critical use case (Mass Casualty Assessment Tool) combined with multi-platform deployment and real-time signal processing demands rigorous testing that is currently only partially in place.

### Risk Heat Map

| Factor | Risk | Confidence | Test Coverage | Key Concern |
|--------|------|------------|---------------|-------------|
| **Structure** | MEDIUM | High | Good | 18 crates well-organized; MAT lib.rs at 626 lines pushes limit |
| **Function** | HIGH | High | Moderate | Vital signs extraction, pose estimation accuracy unvalidated in production conditions |
| **Data** | MEDIUM | High | Moderate | Proof-of-reality system strong; CSI data integrity across protocols untested |
| **Interfaces** | HIGH | Medium | Low | REST API stub in Rust; Python/Rust boundary undefined; ESP32 serial protocol loosely coupled |
| **Platform** | HIGH | Medium | Low | 6 deployment targets; ESP32 original/C3 excluded but not enforced at build level |
| **Operations** | MEDIUM | Medium | Low | No Dockerfile; firmware OTA path defined but unvalidated end-to-end |
| **Time** | CRITICAL | High | Very Low | 20 Hz target; no latency benchmarks; concurrent multi-node processing untested |

---

## S -- Structure

### What the product IS

#### S1: Code Integrity

**Finding:** The Rust workspace is well-structured with 18 crates following Domain-Driven Design bounded contexts. The `wifi-densepose-core` crate uses `#![forbid(unsafe_code)]` and provides clean trait abstractions (`SignalProcessor`, `NeuralInference`, `DataStore`). The crate dependency graph has a clear publish order documented in CLAUDE.md.

**Risk: MEDIUM**
- The `wifi-densepose-mat` lib.rs is 626 lines, exceeding the project's own 500-line limit specified in CLAUDE.md. The `DisasterResponse` struct owns 8 fields including an `Arc<dyn EventStore>`, making it a coordination bottleneck.
- The `wifi-densepose-wasm-edge` crate is excluded from the workspace (`exclude = ["crates/wifi-densepose-wasm-edge"]`), meaning `cargo test --workspace` does not exercise it. This creates a coverage gap for edge deployment code (662 lines).
- The `wifi-densepose-api` Rust crate is a 1-line stub (`//! WiFi-DensePose REST API (stub)`), while the Python v1 has a full FastAPI implementation. This implies the Rust port's API surface is incomplete.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| S-01 | P1 | Build `wifi-densepose-wasm-edge` separately (`cargo build -p wifi-densepose-wasm-edge --target wasm32-unknown-unknown`) and run any embedded tests to confirm they pass outside the workspace test run | Integration |
| S-02 | P2 | Measure cyclomatic complexity of `DisasterResponse::scan_cycle` which spans 80+ lines with nested borrows and conditional event emission -- flag if complexity exceeds 15 | Unit |
| S-03 | P2 | Run `cargo check --workspace --all-features` to surface feature-flag interaction issues across all 18 crates that are hidden by `--no-default-features` in CI | Integration |
| S-04 | P3 | Count lines per file across all crates; flag any `.rs` file exceeding the 500-line project policy | Lint/CI |

#### S2: Dependencies

**Finding:** The workspace has 30+ external crate dependencies including heavy ones: `tch` (PyTorch FFI), `ort` (ONNX Runtime), `ndarray-linalg` with `openblas-static`, and 7 `ruvector-*` crates from crates.io. The `ruvector` dependency comment notes "Vendored at v2.1.0 in vendor/ruvector; using crates.io versions until published" -- suggesting a version mismatch risk between vendored and published code.

**Risk: MEDIUM**
- `ort = "2.0.0-rc.11"` is a release candidate. RC dependencies in production code carry API stability risk.
- `ndarray-linalg` with `openblas-static` forces a specific BLAS implementation that may conflict on certain platforms (ARM, WASM).
- The `tch-backend` feature flag gates the entire training pipeline. If a developer enables it without libtorch installed, the build fails without a clear error path.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| S-05 | P1 | Run `cargo audit` to detect known vulnerabilities in the 30+ dependencies, particularly `ort` RC and `tch` FFI bindings | CI/Unit |
| S-06 | P2 | Build the workspace on ARM64 (aarch64-unknown-linux-gnu) to confirm `openblas-static` compiles; the current CI only runs x86_64 | Integration |
| S-07 | P2 | Toggle `tch-backend` feature on `wifi-densepose-train` without libtorch installed; confirm error message is actionable, not a cryptic linker failure | Human Exploration |

#### S3: Non-Executable Files

**Finding:** 43+ ADR documents, proof data files (`sample_csi_data.json`, `expected_features.sha256`), NVS configuration files for ESP32. The proof-of-reality system uses a published SHA-256 hash of pipeline output as a trust anchor.

**Risk: LOW**
- The `expected_features.sha256` file is the single point of truth for pipeline integrity. If it is regenerated incorrectly (e.g., with a different numpy version), the proof becomes meaningless.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| S-08 | P0 | Run `python archive/v1/data/proof/verify.py` in CI on every PR that touches `archive/v1/src/core/` or `archive/v1/src/hardware/` to catch proof-breaking changes | CI |
| S-09 | P2 | Pin numpy/scipy versions in requirements.txt and confirm `verify.py --generate-hash` produces the same hash across Python 3.10, 3.11, and 3.12 | Integration |

---

## F -- Function

### What the product DOES

#### F1: Application -- Core Capabilities

**Finding:** The system advertises five core capabilities:
1. CSI extraction from ESP32 hardware
2. Signal processing (noise removal, phase sanitization, feature extraction, Doppler)
3. Human presence detection and pose estimation (17-keypoint COCO format)
4. Vital signs extraction (breathing rate, heart rate)
5. Mass casualty assessment (survivor detection through debris)

The Python v1 CSI processor (`csi_processor.py`) implements a complete pipeline from raw CSI frames through feature extraction to human detection. The Rust port replicates and extends this with 14 RuvSense modules for multistatic sensing.

**Risk: HIGH**
- The human detection confidence calculation in `_calculate_detection_confidence` uses hardcoded binary thresholds (`> 0.1`, `> 0.05`, `> 0.3`) with fixed weights (`0.4`, `0.3`, `0.3`). These are not calibrated against ground truth data.
- The temporal smoothing factor (`smoothing_factor = 0.9`) means the system takes ~10 frames to respond to a presence change. For a 20 Hz system, that is 500ms of latency injected by design -- acceptable for presence but too slow for pose tracking.
- The `EnsembleClassifier` in the MAT crate combines breathing, heartbeat, and movement classifiers but there are no integration tests validating that the ensemble confidence actually correlates with real survivor detection.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| F-01 | P0 | Feed 100 known-good CSI frames (from `sample_csi_data.json`) through the full Python pipeline and assert detection confidence is within expected range (0.7-0.95 for human-present frames) | Unit |
| F-02 | P0 | Feed 100 CSI frames of background noise (no human present) and confirm detection confidence stays below threshold (< 0.3); false positive rate must be < 5% | Unit |
| F-03 | P1 | Measure temporal smoothing convergence: inject a step change from no-human to human-present and count frames until confidence exceeds threshold; assert < 15 frames at 20 Hz | Unit |
| F-04 | P1 | Run the MAT `EnsembleClassifier` with synthetic vital signs at confidence boundary (0.49, 0.50, 0.51) and confirm correct accept/reject behavior at the `confidence_threshold` boundary | Unit |
| F-05 | P2 | Inject CSI data with `amplitudes.len() != phases.len()` into `DisasterResponse::push_csi_data` and confirm the error path returns `MatError::Detection` with descriptive message | Unit |

#### F2: Calculation Accuracy

**Finding:** The signal processing pipeline involves FFT (via `rustfft` and `scipy.fft`), correlation matrices, bandpass filtering, zero-crossing analysis, autocorrelation, and SVD decomposition. These are numerically sensitive operations.

**Risk: HIGH**
- The Doppler extraction in Python uses `scipy.fft.fft` with `n=64` bins on a sliding window of cached phase values. The normalization divides by `max_val` which can amplify noise when the max is near zero.
- The vital signs extractor (`BreathingExtractor`, `HeartRateExtractor`) uses bandpass filtering in specific Hz ranges (0.1-0.5 Hz for breathing, 0.8-2.0 Hz for heart rate). These filter boundaries are physiologically reasonable but have no tolerance handling for edge cases (e.g., athlete with 40 bpm resting heart rate = 0.67 Hz, below the 0.8 Hz lower bound).

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| F-06 | P0 | Generate a synthetic CSI signal with known Doppler shift (e.g., 2 Hz sinusoidal phase modulation) and confirm the Doppler extraction peak is within +/- 0.5 Hz of the injected frequency | Unit |
| F-07 | P1 | Feed the `HeartRateExtractor` a signal at 0.67 Hz (40 bpm, athletic resting rate) and confirm it is either detected correctly or reported as `VitalEstimate::unavailable` -- not misclassified as breathing | Unit |
| F-08 | P1 | Test Doppler normalization edge case: when `max_val` approaches zero (< 1e-12), confirm division does not produce NaN or Inf values | Unit |
| F-09 | P2 | Compare Python `scipy.fft.fft` output against Rust `rustfft` output for the same 64-element input vector; assert difference < 1e-6 per bin | Integration |

#### F3: Error Handling

**Finding:** The Rust crates use `thiserror` with per-crate error enums (`MatError`, `SignalError`, `RuvSenseError`) that chain properly. The Python code uses custom exception classes (`CSIProcessingError`, `DatabaseConnectionError`). Both handle errors with descriptive messages.

**Risk: MEDIUM**
- The Python `CSIProcessor.process_csi_data` catches all exceptions with a blanket `except Exception as e` and wraps them in `CSIProcessingError`. This loses the original exception type and stack trace from the caller's perspective.
- The Rust `scan_cycle` method silently discards event store errors with `let _ = self.event_store.append(...)`. In a disaster response context, losing domain events could mean missing survivor detections.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| F-10 | P1 | Make the `InMemoryEventStore` return an error on `append()` and confirm `scan_cycle` either propagates the error or logs it at WARN+ level -- not silently discard it | Unit |
| F-11 | P2 | Inject a `numpy.linalg.LinAlgError` in the correlation matrix computation and confirm the error chain preserves the original exception type through `CSIProcessingError` | Unit |

#### F4: Security

**Finding:** The Python API implements authentication middleware (`AuthMiddleware`), rate limiting (`RateLimitMiddleware`), CORS configuration, and trusted host middleware for production. Settings require a `secret_key` field. The dev config endpoint redacts sensitive fields containing "secret", "password", "token", "key", "credential", "auth".

**Risk: MEDIUM**
- The `secret_key` field uses `Field(...)` (required) but there is no validation on minimum key length or entropy.
- CORS defaults to `["*"]` which is permissive. While overridable, the default is risky if deployed without configuration.
- The readiness check at `/health/ready` hardcodes `ready = True` with a comment "Basic readiness - API is responding" and `checks["hardware_ready"] = True` regardless of actual hardware state. This defeats the purpose of a readiness probe.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| F-12 | P0 | Set `secret_key` to a 3-character string and confirm the application either rejects it at startup or logs a security warning | Unit |
| F-13 | P1 | Submit a request to `/health/ready` when `pose_service` is `None` and confirm `ready` is reported as `False`, not hardcoded `True` | Integration |
| F-14 | P1 | Set `environment=production` and confirm `/docs`, `/redoc`, and `/openapi.json` endpoints return 404, not the Swagger UI | E2E |
| F-15 | P2 | Send 101 requests within the rate limit window and confirm the 101st is rejected with HTTP 429 | Integration |

#### F5: State Transitions

**Finding:** The system has multiple state machines:
- `DeviceStatus`: ACTIVE -> INACTIVE -> MAINTENANCE -> ERROR
- `SessionStatus`: ACTIVE -> COMPLETED / FAILED / CANCELLED
- `ProcessingStatus`: PENDING -> PROCESSING -> COMPLETED / FAILED
- ESP32 firmware: WiFi connecting -> connected -> CSI streaming
- RuvSense `TrackLifecycleState`: lifecycle for pose tracks
- MAT `ZoneStatus`: Active scan zones

**Risk: MEDIUM**
- The database models define valid states via `CheckConstraint` but do not enforce transition rules (e.g., can a device go from ERROR directly to ACTIVE without going through MAINTENANCE?).

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| F-16 | P1 | Attempt to transition `DeviceStatus` from ERROR to ACTIVE directly and confirm the system either prevents it or logs the anomaly | Unit |
| F-17 | P2 | Simulate a `Session` that is in COMPLETED status and attempt to add new CSI data to it; confirm it is rejected | Unit |

---

## D -- Data

### What the product PROCESSES

#### D1: Input Data

**Finding:** The system ingests CSI frames from multiple sources:
- ESP32 ADR-018 binary protocol (UDP)
- Serial port data via `serialport` crate
- Sample JSON data (`sample_csi_data.json` with 1,000 synthetic frames)
- `CsiData` Python dataclass: amplitude (ndarray), phase (ndarray), frequency, bandwidth, num_subcarriers, num_antennas, snr, metadata

The Rust `Esp32CsiParser::parse_frame` takes raw bytes and returns structured `CsiFrame` with amplitude/phase arrays.

**Risk: MEDIUM**
- The Python `CSIData` dataclass accepts arbitrary-shaped numpy arrays for amplitude and phase. There is no validation that `amplitude.shape == (num_antennas, num_subcarriers)`.
- The ESP32 parser returns `ParseError::InsufficientData { needed, got }` but there is no handling for malformed data that has the right length but corrupt content (e.g., all-zero subcarrier data).

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| D-01 | P1 | Create a `CSIData` with `amplitude.shape = (3, 64)` but `num_antennas = 2` and confirm the processor rejects or reshapes it | Unit |
| D-02 | P1 | Feed the ESP32 parser a correctly-sized but all-zero byte buffer and confirm it either rejects the frame (quality check) or marks `quality_score` as degraded | Unit |
| D-03 | P2 | Feed the ESP32 parser a buffer with valid header but truncated subcarrier data; confirm `ParseError::InsufficientData` | Unit |
| D-04 | P2 | Test boundary: exactly 256 subcarriers (MAX_SUBCARRIERS constant) and 257 subcarriers -- confirm correct handling | Unit |

#### D2: Data Persistence

**Finding:** The Python v1 uses SQLAlchemy with PostgreSQL (primary) and SQLite (failsafe fallback). The database schema includes 6 tables: `devices`, `sessions`, `csi_data`, `pose_detections`, `system_metrics`, `audit_logs`. The `csi_data` table stores amplitude and phase as `FloatArray` columns with a unique constraint on `(device_id, sequence_number, timestamp_ns)`.

**Risk: MEDIUM**
- Storing raw CSI amplitude/phase arrays as database columns (FloatArray) is expensive. At 20 Hz with 56 subcarriers, that is 2,240 floats/second per device stored to PostgreSQL. No data retention policy or archival strategy is documented.
- The SQLite fallback uses `NullPool` which means no connection reuse. Under load, this could exhaust file handles.
- The `audit_logs` table tracks changes but there is no mention of log rotation or size limits.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| D-05 | P1 | Insert 100,000 CSI frames (simulating ~83 minutes of data at 20 Hz) into the database and measure query performance for time-range retrievals | Integration |
| D-06 | P1 | Trigger PostgreSQL failover to SQLite and confirm: (a) no data loss during transition, (b) API continues responding, (c) health endpoint reports "degraded" not "healthy" | Integration |
| D-07 | P2 | Insert CSI data with duplicate `(device_id, sequence_number, timestamp_ns)` and confirm the unique constraint fires with an appropriate error message | Unit |
| D-08 | P3 | Run 1,000 concurrent SQLite connections via the NullPool fallback and monitor for "database is locked" errors | Integration |

#### D3: Proof Data Integrity

**Finding:** The proof-of-reality system (`archive/v1/data/proof/verify.py`) is a deterministic pipeline verification tool. It feeds 1,000 synthetic CSI frames through the production CSI processor, hashes the output with SHA-256, and compares against a published hash. This is a strong engineering practice.

**Risk: LOW**
- The proof only exercises the Python v1 pipeline. The Rust port has no equivalent proof-of-reality check.
- The proof uses `seed=42` for synthetic data generation. If `numpy.random` changes its RNG implementation across versions, the proof breaks without any pipeline code change.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| D-09 | P0 | Run `verify.py` with `--audit` flag to scan for mock/random patterns in the codebase that could compromise pipeline integrity | CI |
| D-10 | P1 | Create an equivalent proof-of-reality test for the Rust `wifi-densepose-signal` crate: feed the same 1,000 frames through `CsiProcessor::new(config)` and assert deterministic output | Unit |

---

## I -- Interfaces

### How the product CONNECTS

#### I1: REST API

**Finding:** The Python v1 exposes a FastAPI application with three router groups:
- `/health/*` -- Health, readiness, liveness, metrics, version (5 endpoints)
- `/api/v1/pose/*` -- Pose estimation endpoints
- `/api/v1/stream/*` -- Streaming endpoints

The Rust `wifi-densepose-api` crate is a 1-line stub. The `wifi-densepose-mat` crate has its own `api` module with an Axum router (`create_router, AppState`).

**Risk: HIGH**
- Two separate API implementations (Python FastAPI for v1, Rust Axum for MAT) with no shared contract or OpenAPI schema. A consumer cannot rely on interface consistency.
- The Python API's general exception handler returns a generic "Internal server error" for all unhandled exceptions in production, but logs the full traceback. If logs are not monitored, 500 errors go unnoticed.
- No API versioning enforcement: the prefix is configurable via `settings.api_prefix` but defaults to `/api/v1`. There is no v2 migration path documented.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| I-01 | P0 | Export OpenAPI spec from the Python FastAPI app and validate it against the actual endpoint behavior using Schemathesis or Dredd | E2E |
| I-02 | P1 | Send malformed JSON to every POST endpoint and confirm each returns HTTP 422 with validation error details, not 500 | Integration |
| I-03 | P1 | Hit the MAT Axum API and the Python FastAPI health endpoints in parallel and confirm they use compatible response schemas | Integration |
| I-04 | P2 | Send a request with `Content-Type: text/xml` to a JSON endpoint and confirm HTTP 415 Unsupported Media Type, not a 500 crash | Integration |

#### I2: WebSocket Protocol

**Finding:** The Python v1 has a WebSocket subsystem (`connection_manager.py`, `pose_stream.py`) for real-time pose data streaming. The connection manager tracks active connections and provides stats.

**Risk: MEDIUM**
- No WebSocket protocol specification (message format, heartbeat interval, reconnection policy).
- The `connection_manager.shutdown()` is called during cleanup but there is no graceful disconnect message sent to connected clients.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| I-05 | P1 | Connect 100 WebSocket clients simultaneously and confirm: (a) all receive pose data, (b) connection stats are accurate, (c) no memory leak over 60 seconds | Integration |
| I-06 | P1 | Disconnect a WebSocket client abruptly (TCP reset) and confirm the server cleans up the connection without leaking resources | Integration |
| I-07 | P2 | Send a malformed message over WebSocket and confirm the server rejects it without disconnecting the client | Integration |

#### I3: ESP32 Serial/UDP Protocol

**Finding:** The ESP32 firmware uses ADR-018 binary format for CSI frames sent over UDP. The firmware includes WiFi reconnection logic with exponential retry (up to MAX_RETRY=10), NVS configuration persistence, OTA update capability, and WASM runtime support.

The Rust `Esp32CsiParser` parses the binary frames from UDP bytes.

**Risk: HIGH**
- The ADR-018 binary protocol has no version field visible in the main.c header. If the protocol format changes, there is no way for the receiver to detect version mismatch.
- The UDP transport is fire-and-forget. There is no acknowledgment, no sequence gap detection documented in the receiver, and no backpressure mechanism.
- The `stream_sender.c` sends to a hardcoded or NVS-configured target IP. If the aggregator moves, the sensor is stranded until re-provisioned.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| I-08 | P0 | Inject a CSI frame with a future/unknown protocol version byte and confirm the parser returns `ParseError` with a version mismatch message, not a crash | Unit |
| I-09 | P1 | Send 1,000 UDP CSI frames at 20 Hz from a simulated ESP32 and measure packet loss rate at the aggregator; assert < 1% loss on loopback | Integration |
| I-10 | P1 | Simulate network partition: stop sending UDP frames for 5 seconds, then resume. Confirm the aggregator recovers without manual intervention | Integration |
| I-11 | P2 | Send a UDP frame from a spoofed MAC address and confirm the aggregator either rejects or flags it (ADR-032 security hardening) | Integration |

#### I4: Inter-Crate Boundaries (Rust)

**Finding:** The Rust workspace has clear crate boundaries with `pub use` re-exports. The core traits (`SignalProcessor`, `NeuralInference`, `DataStore`) define contracts. However, some inter-crate communication uses concrete types rather than trait objects.

**Risk: MEDIUM**
- `wifi-densepose-mat` depends on `wifi-densepose-signal::SignalError` directly via `#[from]`. This couples the MAT error hierarchy to Signal internals.
- The `wifi-densepose-train` crate conditionally compiles 5 modules (`losses`, `metrics`, `model`, `proof`, `trainer`) behind the `tch-backend` feature. This means the training crate's public API surface changes dramatically based on feature flags.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| I-12 | P1 | Build `wifi-densepose-mat` with `wifi-densepose-signal` at a different version (e.g., mock a breaking change in `SignalError`) and confirm the type error is caught at compile time | Unit |
| I-13 | P2 | Compile `wifi-densepose-train` with and without `tch-backend` and diff the public API symbols; document the feature-gated surface area | Integration |

#### I5: CLI Interface

**Finding:** The Rust CLI (`wifi-densepose-cli`) provides subcommands for MAT operations: `mat scan`, `mat status`, `mat survivors`, `mat alerts`. Built with `clap` derive macros.

**Risk: LOW**
- CLI is narrowly scoped to MAT operations. No CLI for CSI data capture, signal processing, or model training.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| I-14 | P2 | Run `wifi-densepose --help`, `wifi-densepose mat --help`, and confirm all documented subcommands are present and help text is accurate | E2E |
| I-15 | P3 | Run `wifi-densepose mat scan --zone ""` (empty zone name) and confirm a user-friendly error, not a panic | Unit |

---

## P -- Platform

### What the product DEPENDS ON

#### P1: Multi-Platform Build Targets

**Finding:** The project targets 6 platforms:
1. **Linux x86_64** -- Primary development/server platform (CI runs here)
2. **Windows** -- ESP32 firmware build requires special MSYSTEM env var stripping
3. **macOS** -- CoreWLAN WiFi sensing (ADR-025), `mac_wifi.swift` in sensing module
4. **ESP32-S3** -- Xtensa dual-core, 8MB/4MB flash variants
5. **WASM (wasm32-unknown-unknown)** -- Browser deployment via wasm-pack
6. **Desktop** -- `wifi-densepose-desktop` crate (52 lines in lib.rs, minimal)

Explicitly unsupported: ESP32 (original) and ESP32-C3 (single-core, cannot run DSP pipeline).

**Risk: HIGH**
- The CI workflow (`ci.yml`) only runs on `ubuntu-latest`. No Windows, macOS, or ARM64 CI jobs for the Rust crates.
- The macOS CoreWLAN integration (`mac_wifi.swift`) exists in the Python sensing module but there are no tests or build validation for it.
- The `openblas-static` dependency in `ndarray-linalg` does not compile on `wasm32-unknown-unknown`, yet `wifi-densepose-signal` depends on it. This means any crate depending on `signal` cannot target WASM without feature gating.
- The firmware CI (`firmware-ci.yml`, `firmware-qemu.yml`) exists but the `verify-pipeline.yml` suggests a separate verification path.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| P-01 | P0 | Add macOS and Windows CI runners for `cargo test --workspace --no-default-features` to catch platform-specific compilation failures | CI |
| P-02 | P1 | Build `wifi-densepose-wasm` with `wasm-pack build --target web` in CI and confirm it produces a valid `.wasm` binary under 5 MB | CI |
| P-03 | P1 | Flash the 4MB firmware variant to an ESP32-S3 and confirm it boots, connects to WiFi, and streams CSI frames within 30 seconds | Hardware/Human |
| P-04 | P2 | Attempt to build the firmware for ESP32 (original, non-S3) and confirm the build fails with a clear error message about single-core incompatibility | Integration |

#### P2: External Software Dependencies

**Finding:** The system depends on:
- PostgreSQL (primary database)
- Redis (caching, rate limiting -- optional)
- libtorch (PyTorch C++ backend -- optional via `tch-backend` feature)
- ONNX Runtime (`ort` crate)
- OpenBLAS (via `ndarray-linalg`)
- ESP-IDF v5.4 (firmware toolchain)
- wasm-pack (WASM build tool)

**Risk: MEDIUM**
- The PostgreSQL-to-SQLite failsafe is a good design but the SQLite fallback does not support all PostgreSQL features (e.g., `UUID` columns, array types via `StringArray`/`FloatArray`). The `model_types.py` file likely provides compatibility shims but this is an untested assumption.
- Redis is marked optional but the `RateLimitMiddleware` likely depends on it for distributed rate limiting. If Redis is down and rate limiting is enabled, what happens?

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| P-05 | P1 | Start the API with `redis_enabled=True` but Redis unavailable, and `redis_required=False`. Confirm the API starts, rate limiting degrades gracefully, and health reports "degraded" | Integration |
| P-06 | P1 | Insert a `Device` record via SQLite fallback with a UUID primary key and StringArray capabilities column; confirm round-trip read matches the write | Integration |
| P-07 | P2 | Run the full Python test suite on Python 3.12 (the CI uses 3.11) to catch forward-compatibility issues | CI |

#### P3: Hardware Compatibility

**Finding:** Supported hardware:
- ESP32-S3 (8MB flash) at ~$9
- ESP32-S3 SuperMini (4MB flash) at ~$6
- ESP32-C6 + Seeed MR60BHA2 (60 GHz FMCW mmWave) at ~$15
- HLK-LD2410 (24 GHz FMCW presence sensor) at ~$3

The ESP32-S3 is the primary sensing node. The mmWave sensors are auxiliary.

**Risk: MEDIUM**
- The 4MB flash variant (`sdkconfig.defaults.4mb`) may not have room for OTA + WASM runtime + display driver. Partition table conflicts are plausible but not tested in CI.
- The mmWave sensor integration (`mmwave_sensor.c`) exists in firmware but there are no tests validating the serial protocol parsing for the MR60BHA2 radar.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| P-08 | P1 | Build 4MB firmware with OTA + WASM + display all enabled and confirm the binary fits within the 4MB flash partition | CI |
| P-09 | P2 | Send synthetic MR60BHA2 serial output to the `mmwave_sensor.c` parser and confirm correct heart rate / breathing rate extraction | Unit |

---

## O -- Operations

### How the product is USED

#### O1: Deployment Model

**Finding:** No Dockerfile exists (only `.dockerignore`). CI includes `cd.yml` (continuous deployment) but deployment target is unknown. The firmware has a documented flash process using `idf.py` and a provisioning script (`provision.py`).

**Risk: HIGH**
- Without a Dockerfile, the Python v1 API has no standardized deployment. Server setup is manual and environment-specific.
- The firmware OTA update mechanism (`ota_update.c`) exists but the end-to-end update path (build -> sign -> distribute -> apply -> verify) is undocumented.
- No Kubernetes manifests, systemd service files, or other deployment automation.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| O-01 | P1 | Create a Docker image for the Python v1 API and confirm it starts, responds to `/health/live`, and connects to a PostgreSQL container | Integration |
| O-02 | P1 | Test the firmware OTA path: build a new firmware image, host it on HTTP, trigger OTA from the device, and confirm the device reboots with the new version | Hardware/Human |
| O-03 | P2 | Run `wifi-densepose mat scan` on a freshly provisioned ESP32-S3 and confirm end-to-end data flow from sensor to CLI output | E2E/Human |

#### O2: Monitoring and Observability

**Finding:** The Python API provides comprehensive health checks (`/health/health`, `/health/ready`, `/health/live`), system metrics (CPU, memory, disk, network via `psutil`), and per-component health status. The Rust crates use `tracing` for structured logging.

**Risk: MEDIUM**
- The health check calls `psutil.cpu_percent(interval=1)` which blocks for 1 second. This makes the health endpoint slow and potentially a bottleneck under load.
- The system metrics endpoint is available to unauthenticated users at `/health/metrics`. Only "detailed metrics" require authentication.
- There is no distributed tracing (e.g., OpenTelemetry) for correlating requests across the Python API, ESP32 firmware, and potential Rust services.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| O-04 | P1 | Call `/health/health` 10 times concurrently and confirm total response time is < 15 seconds (not 10x the 1-second cpu_percent block) | Integration |
| O-05 | P2 | Confirm `/health/metrics` does not expose PII, database credentials, or internal IP addresses in the response body | Security/E2E |

#### O3: User Workflows

**Finding:** Primary user workflows:
1. Researcher: Configure sensors -> Collect CSI data -> Train model -> Evaluate
2. Disaster responder: Deploy sensors -> Start MAT scan -> Monitor survivors -> Triage
3. Developer: Clone repo -> Build -> Run tests -> Submit PR

**Risk: MEDIUM**
- The disaster responder workflow is safety-critical. A false negative (missing a survivor) has life-or-death consequences. The system should have explicit false negative rate metrics but none are defined.
- The developer workflow requires installing OpenBLAS, potentially libtorch, and ESP-IDF v5.4. No `devcontainer.json` or `nix-shell` to standardize the development environment.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| O-06 | P0 | Run the complete developer setup workflow from a clean Ubuntu 22.04 VM: clone, install deps, `cargo test --workspace --no-default-features`, `python archive/v1/data/proof/verify.py` -- measure total setup time and document any manual steps | Human Exploration |
| O-07 | P1 | Simulate a MAT scan with 5 survivors at varying signal strengths (strong, weak, borderline) and confirm the triage classification matches expected START protocol categories | Integration |

#### O4: Extreme Use

**Finding:** No load testing, stress testing, or chaos engineering infrastructure exists.

**Risk: HIGH**
- The system targets disaster response scenarios where multiple ESP32 nodes stream simultaneously. The aggregator's behavior under 10+ concurrent node streams is unknown.
- The database writes CSI data at 20 Hz per device. With 10 devices, that is 200 inserts/second of array data into PostgreSQL.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| O-08 | P1 | Simulate 10 ESP32 nodes streaming at 20 Hz to the aggregator and measure: packet loss, processing latency per frame, memory growth over 5 minutes | Performance |
| O-09 | P2 | Fill the CSI history deque to `max_history_size=500` and confirm the oldest entry is evicted, not causing an OOM | Unit |

---

## T -- Time

### WHEN things happen

#### T1: Real-Time Processing

**Finding:** The RuvSense pipeline targets 20 Hz output (50ms per TDMA cycle). The vital signs extraction uses sample rates of 100 Hz with 30-second windows. The CSI processor uses configurable `sampling_rate`, `window_size`, and `overlap`.

**Risk: CRITICAL**
- No latency benchmarks exist anywhere in the codebase. The 20 Hz target implies each frame must be processed in < 50ms including multi-band fusion, phase alignment, multistatic fusion, coherence gating, and pose tracking. This budget has never been measured.
- The Python `process_csi_data` method is `async` but all the numpy operations inside are synchronous and CPU-bound. The `await` is cosmetic -- it does not yield to the event loop during computation.
- The Doppler extraction iterates over the phase cache on every call. With `max_history_size=500`, this means constructing a 500-element numpy array from a deque on each frame.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| T-01 | P0 | Benchmark the Rust `RuvSensePipeline` end-to-end latency for a single frame with 4 nodes and 56 subcarriers; assert total processing time < 50ms on x86_64 | Benchmark |
| T-02 | P0 | Benchmark the Python `CSIProcessor.process_csi_data` method for a single frame and assert it completes in < 25ms (leaving budget for I/O and networking) | Benchmark |
| T-03 | P1 | Profile the Doppler extraction path with `max_history_size=500`: measure time spent in `list(self._phase_cache)` and `np.array(cache_list[-window:])` | Benchmark |
| T-04 | P1 | Run the Python CSI processor with `asyncio.run()` and confirm it does not block the event loop for > 10ms per frame; use `asyncio.get_event_loop().slow_callback_duration` | Integration |

#### T2: Concurrency

**Finding:** The Rust system uses `tokio` for async runtime with `features = ["full"]`. The Python API uses FastAPI (async) with uvicorn workers. The ESP32 firmware uses FreeRTOS tasks. The `DisasterResponse::running` flag uses `AtomicBool` for thread-safe scanning control.

**Risk: HIGH**
- The `DisasterResponse` struct is not `Send + Sync` safe by default (it contains `dyn EventStore` behind an `Arc`, but the struct itself is not wrapped in a `Mutex`). If `start_scanning` is called from multiple threads, the mutable self-reference causes a data race.
- The Python `get_database_manager` uses a module-level global `_db_manager` with no thread-safety protection. With multiple uvicorn workers, each worker gets its own instance (process isolation), but within a single worker, concurrent requests could race on initialization.
- The ESP32 firmware uses FreeRTOS event groups for WiFi state but the CSI callback runs in the WiFi driver context. If the callback takes too long (e.g., edge processing), it blocks WiFi reception.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| T-05 | P0 | Run `cargo test` under Miri (or ThreadSanitizer) for the `wifi-densepose-mat` crate to detect data races in `DisasterResponse` | CI |
| T-06 | P1 | Call `DatabaseManager.initialize()` concurrently from 10 async tasks and confirm only one initialization occurs (no double-init race) | Integration |
| T-07 | P1 | Measure the CSI callback execution time on ESP32 and confirm it completes in < 1ms to avoid blocking the WiFi driver | Hardware/Benchmark |
| T-08 | P2 | Start and stop `DisasterResponse::start_scanning` from two different tokio tasks simultaneously and confirm no panic or deadlock | Unit |

#### T3: Scheduling and Timeouts

**Finding:** The MAT scan interval is configurable (`scan_interval_ms`, default 500ms, minimum 100ms). The database connection pool has `pool_timeout=30s` and `pool_recycle=3600s`. Redis has `socket_timeout=5s` and `connect_timeout=5s`.

**Risk: MEDIUM**
- The ESP32 WiFi reconnection has `MAX_RETRY=10` but no backoff strategy. Ten rapid reconnection attempts could flood the AP.
- No timeout on the `scan_cycle` method itself. If detection takes longer than `scan_interval_ms`, cycles overlap without back-pressure.
- The `pool_recycle=3600` means database connections are recycled every hour. In a long-running deployment, this causes periodic connection churn.

**Test Ideas:**
| # | Priority | Test Idea | Automation |
|---|----------|-----------|------------|
| T-09 | P1 | Set `scan_interval_ms=100` (minimum) and run a scan cycle that takes 200ms to complete; confirm the system does not accumulate a backlog of overlapping cycles | Unit |
| T-10 | P2 | Simulate 10 WiFi disconnects in rapid succession on ESP32 and confirm the retry counter increments correctly and stops at MAX_RETRY=10 | Integration/Hardware |
| T-11 | P2 | Keep the API running for 2 hours and confirm database pool recycling does not cause request failures during connection rotation | Integration |

---

## Product Coverage Outline (PCO)

| # | Testable Element | Reference | Product Factor(s) |
|---|------------------|-----------|-------------------|
| 1 | Cargo workspace build integrity | Cargo.toml, 18 crates | Structure |
| 2 | WASM-edge crate exclusion gap | Cargo.toml `exclude` | Structure |
| 3 | Dependency vulnerability surface | 30+ external crates | Structure |
| 4 | CSI processing pipeline determinism | csi_processor.py, verify.py | Function, Data |
| 5 | Human detection accuracy | _calculate_detection_confidence | Function |
| 6 | Vital signs extraction boundaries | BreathingExtractor, HeartRateExtractor | Function, Data |
| 7 | MAT ensemble classification | EnsembleClassifier | Function |
| 8 | Error chain preservation | CSIProcessingError, MatError | Function |
| 9 | Event store silent error discard | scan_cycle let _ = | Function |
| 10 | Authentication and secrets management | Settings.secret_key, AuthMiddleware | Function |
| 11 | Readiness probe accuracy | /health/ready hardcoded True | Function, Interfaces |
| 12 | State machine transition enforcement | DeviceStatus, SessionStatus | Function |
| 13 | CSI data shape validation | CSIData ndarray shapes | Data |
| 14 | ESP32 binary protocol parsing | Esp32CsiParser | Data, Interfaces |
| 15 | Database failover correctness | PostgreSQL -> SQLite | Data, Platform |
| 16 | Proof-of-reality cross-platform | verify.py, Rust equivalent | Data |
| 17 | REST API contract consistency | FastAPI, Axum MAT API | Interfaces |
| 18 | WebSocket connection management | connection_manager.py | Interfaces |
| 19 | UDP CSI transport reliability | stream_sender.c, aggregator | Interfaces |
| 20 | Cross-platform compilation | Linux, macOS, Windows, WASM, ESP32 | Platform |
| 21 | Hardware compatibility matrix | ESP32-S3 4MB/8MB, mmWave | Platform |
| 22 | External service dependencies | PostgreSQL, Redis, libtorch | Platform |
| 23 | Deployment automation | Missing Dockerfile | Operations |
| 24 | OTA firmware update path | ota_update.c | Operations |
| 25 | Health endpoint performance | psutil.cpu_percent blocking | Operations |
| 26 | Multi-node stress testing | 10+ concurrent ESP32 streams | Operations, Time |
| 27 | Real-time latency budget | 50ms target at 20 Hz | Time |
| 28 | Async processing correctness | CPU-bound in async context | Time |
| 29 | Thread safety and data races | DisasterResponse, DatabaseManager | Time |
| 30 | Scan cycle timing overlap | scan_interval_ms vs processing time | Time |

---

## Test Data Suggestions

### Test Data for Structure-Based Tests
- Cargo.toml with intentionally broken dependency versions to test build failure modes
- `.rs` files at exactly 500 lines and 501 lines to test line-count policy enforcement
- A workspace member list with a typo in the path to test error reporting

### Test Data for Function-Based Tests
- 1,000 CSI frames from `sample_csi_data.json` as baseline input
- Synthetic CSI frames with known Doppler shifts (1 Hz, 2 Hz, 5 Hz, 10 Hz)
- Vital signs signals at physiological extremes: 8 bpm breathing (sleep apnea boundary), 200 bpm heart rate (tachycardia)
- Empty CSI frames (all zeros), single-subcarrier frames, maximum-subcarrier frames (256)
- EnsembleClassifier inputs at confidence boundary: 0.499, 0.500, 0.501

### Test Data for Data-Based Tests
- 100,000 CSI frames for database stress testing (~83 minutes at 20 Hz)
- Duplicate `(device_id, sequence_number, timestamp_ns)` tuples for constraint testing
- CSIData with mismatched array shapes (`amplitude.shape != (num_antennas, num_subcarriers)`)
- SQLite database files at 100 MB, 1 GB, and 10 GB for scaling tests

### Test Data for Interface-Based Tests
- Valid and malformed ADR-018 binary frames (truncated, corrupted, oversized)
- Spoofed MAC addresses in UDP frames for security testing
- 100 concurrent WebSocket connections with varying message rates
- OpenAPI specification exported from FastAPI for contract validation

### Test Data for Platform-Based Tests
- Cross-compiled binaries for aarch64, x86_64, wasm32
- ESP32-S3 4MB partition tables with all features enabled (should overflow)
- MR60BHA2 radar serial output samples (synthetic)

### Test Data for Operations-Based Tests
- Docker compose configuration with PostgreSQL + Redis + API
- Firmware OTA images (valid, corrupted, oversized)
- 10-node ESP32 mesh simulation traffic capture

### Test Data for Time-Based Tests
- CSI frames with monotonically increasing timestamps at exactly 50ms intervals
- CSI frames with jittered timestamps (+/- 10ms, +/- 25ms, +/- 50ms)
- Phase cache at sizes: 0, 1, 2, 63, 64, 65, 499, 500 (boundary values for Doppler window)

---

## Suggestions for Exploratory Test Sessions

### Exploratory Test Sessions: Structure
1. **Session: Crate Dependency Graph Walk** -- Starting from `wifi-densepose-cli`, trace every transitive dependency and look for diamond dependencies, version conflicts, or unnecessary coupling between crates that should be independent.
2. **Session: Feature Flag Combinatorics** -- Systematically toggle feature flags on `wifi-densepose-train` (tch-backend on/off) and `wifi-densepose-core` (std/serde/async) and build each combination. Look for compilation failures, missing exports, or confusing error messages.

### Exploratory Test Sessions: Function
3. **Session: Detection Confidence Calibration** -- Feed the CSI processor a sequence of frames that transitions from empty room to one person to two people. Observe how the confidence score evolves. Look for oscillation, slow convergence, or failure to distinguish scenarios.
4. **Session: MAT Disaster Scenario Walkthrough** -- Set up a full MAT scan with 3 zones, inject synthetic CSI data representing 5 survivors at varying depths (0.5m, 2m, 5m). Observe triage classification, alert generation, and event store entries. Look for missing events or incorrect triage.

### Exploratory Test Sessions: Data
5. **Session: Database Failover Chaos** -- Start the API with PostgreSQL, insert data, kill PostgreSQL, observe failover to SQLite, insert more data, restart PostgreSQL, and examine whether the system recovers. Look for data loss, schema incompatibilities, or stuck states.
6. **Session: Proof of Reality Deep Dive** -- Run `verify.py --verbose` and `verify.py --audit` on a fresh checkout. Modify one line of `csi_processor.py` (e.g., change a threshold) and re-run verify. Look for how quickly the hash changes and whether the error message identifies what changed.

### Exploratory Test Sessions: Interfaces
7. **Session: API Fuzzing Marathon** -- Use `schemathesis` or `restler` against the running FastAPI application for 30 minutes. Focus on edge cases: empty bodies, huge payloads (10 MB JSON), unicode in string fields, negative numbers in integer fields. Track every 500 response.
8. **Session: ESP32 Protocol Mismatch Hunt** -- Capture real UDP traffic from an ESP32-S3, modify bytes at various offsets, and feed them to the `Esp32CsiParser`. Look for panics, undefined behavior, or incorrect but accepted frames.

### Exploratory Test Sessions: Platform
9. **Session: macOS CoreWLAN Availability** -- On a macOS machine, attempt to use the `mac_wifi.swift` sensing module. Look for compilation issues, missing entitlements, or WiFi permission dialogs that block unattended operation.
10. **Session: WASM in Browser** -- Build `wifi-densepose-wasm` and load it in Chrome, Firefox, and Safari. Call `MatDashboard` methods from the JavaScript console. Look for WASM memory limits, missing `web-sys` features, or browser-specific failures.

### Exploratory Test Sessions: Operations
11. **Session: First-Time Setup Experience** -- Follow the README as a new developer on a clean Ubuntu 22.04 VM. Document every step that fails, every missing dependency, and every confusing error. Measure total time from `git clone` to first passing test.
12. **Session: Firmware Provisioning End-to-End** -- Use the `provision.py` script to configure a real ESP32-S3 with WiFi credentials. Monitor serial output. Disconnect and reconnect. Look for edge cases in NVS persistence, WiFi credential storage, and recovery from bad configuration.

### Exploratory Test Sessions: Time
13. **Session: Latency Budget Profiling** -- Instrument the Rust `RuvSensePipeline` with `tracing` spans on each stage (multiband, phase_align, multistatic, coherence, pose_tracker). Run 1,000 frames and produce a flame graph. Identify which stage consumes the most of the 50ms budget.
14. **Session: Concurrent Scanning Stress** -- Start `DisasterResponse::start_scanning` with `continuous_monitoring=true` and `scan_interval_ms=100`. While scanning, call `push_csi_data` from a separate thread at 200 Hz. Look for data races, queue overflow, or missed scans.

---

## Clarifying Questions

Suggestions based on general risk patterns and analysis of the existing codebase:

### Structure
1. What is the intended relationship between the Python v1 API and the Rust `wifi-densepose-api` stub? Is the Rust API planned to replace Python, or will they coexist?
2. Why is `wifi-densepose-wasm-edge` excluded from the workspace? Are its tests run in a separate CI job, or are they not run at all?

### Function
3. What is the acceptable false positive rate for human detection? What is the acceptable false negative rate for MAT survivor detection? These are not documented anywhere.
4. The `HeartRateExtractor` bandpass filter starts at 0.8 Hz (48 bpm). Is this intentional, given that athletic resting heart rates can be 40 bpm (0.67 Hz)?
5. The `smoothing_factor` of 0.9 introduces ~500ms lag at 20 Hz. Is this acceptable for the pose tracking use case, or should it be configurable per-mode?

### Data
6. What is the data retention policy for CSI frames in PostgreSQL? At 20 Hz per device, storage grows at ~2.7 GB/day per device (estimated). Who is responsible for archival?
7. Is there a plan to create a Rust-equivalent proof-of-reality test to ensure the Rust signal processing pipeline matches the Python pipeline output?

### Interfaces
8. Does the ADR-018 binary protocol include a version byte? If the firmware and server are at different protocol versions, how is this detected?
9. What is the WebSocket message format for pose data streaming? Is it documented in an ADR or schema file?
10. Is there authentication on the UDP CSI data stream, or can any device on the network inject frames into the aggregator?

### Platform
11. Is ARM64 (e.g., Raspberry Pi 4/5) a supported deployment target for the server? If so, has `openblas-static` been validated on ARM64?
12. Are there plans for an Android or iOS mobile app, or is the `wifi-densepose-desktop` crate the only non-server deployment target?

### Operations
13. Is there a Docker image on Docker Hub as mentioned in the pre-merge checklist? If so, what is the image name and how is it built?
14. What is the firmware signing process for OTA updates? Is there a code-signing key, and how is it managed?
15. Who monitors the `/health/health` endpoint in production? Is there an alerting integration (PagerDuty, Opsgenie, etc.)?

### Time
16. Has the 20 Hz (50ms per frame) latency budget ever been measured on actual hardware with real CSI data? What is the measured P99 latency?
17. What happens when `scan_cycle` takes longer than `scan_interval_ms`? Does the next cycle start immediately, or is there a backlog mechanism?
18. The ESP32 CSI callback runs in the WiFi driver context. What is the maximum allowed execution time before WiFi reception is impacted?

---

## Assessment Quality Metrics

| Metric | Value | Target | Status |
|--------|-------|--------|--------|
| SFDIPOT categories covered | 7/7 | 7/7 | PASS |
| Test ideas generated | 57 | 50+ | PASS |
| P0 (Critical) | 10 (17.5%) | 8-12% | PASS (slightly above due to safety-critical MAT domain) |
| P1 (High) | 20 (35.1%) | 20-30% | PASS |
| P2 (Medium) | 20 (35.1%) | 35-45% | PASS |
| P3 (Low) | 7 (12.3%) | 20-30% | BELOW (complex system with fewer trivial tests) |
| Automation: Unit | 22 (38.6%) | 30-40% | PASS |
| Automation: Integration | 19 (33.3%) | -- | PASS |
| Automation: E2E | 5 (8.8%) | <=50% | PASS |
| Automation: Benchmark | 5 (8.8%) | -- | N/A |
| Automation: Human Exploration | 6 (10.5%) | >=10% | PASS |
| Clarifying questions | 18 | 10+ | PASS |
| Exploratory sessions | 14 | 7+ (one per factor) | PASS |

---

## Priority Summary: Top 10 Actions

1. **T-01/T-02 (P0):** Benchmark real-time processing latency against the 50ms budget. The entire system's viability depends on this.
2. **F-01/F-02 (P0):** Establish baseline false positive/negative rates for human detection with known test data.
3. **T-05 (P0):** Run ThreadSanitizer on the MAT crate to detect data races in the multi-threaded scanning path.
4. **P-01 (P0):** Add macOS and Windows CI runners. A 6-platform project tested on 1 platform is a risk multiplier.
5. **I-08 (P0):** Add protocol version detection to the ESP32 parser to prevent silent data corruption from version mismatches.
6. **S-08/D-09 (P0):** Ensure proof-of-reality runs on every PR touching the signal processing pipeline.
7. **F-12 (P0):** Validate that weak secrets are rejected at startup, not silently accepted.
8. **O-06 (P0):** Document and automate the developer setup experience. A system this complex needs reproducible environments.
9. **F-04 (P1):** Test MAT ensemble classifier at confidence boundaries. In disaster response, boundary behavior determines life-or-death decisions.
10. **I-01 (P0):** Generate and validate OpenAPI contract. Two API implementations (Python + Rust) without a shared contract will inevitably diverge.

---

*Assessment generated using James Bach's HTSM Product Factors framework (SFDIPOT). All findings are based on static analysis of the codebase at commit 85434229 on the qe-reports branch. Risk ratings reflect both probability and impact, with the MAT safety-critical use case amplifying severity for all Function and Time findings.*
