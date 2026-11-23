# ADR-117: pip `wifi-densepose` modernization via PyO3 + maturin bindings

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-05-24 |
| **Deciders** | ruv |
| **Codename** | **PIP-PHOENIX** — rising from a pure-Python server to Rust-core Python bindings |
| **Relates to** | [ADR-021](ADR-021-esp32-vitals.md) (ESP32 vitals), [ADR-028](ADR-028-esp32-capability-audit.md) (capability audit / witness), [ADR-115](ADR-115-home-assistant-integration.md) (HA-DISCO + HA-MIND MQTT semantics), [ADR-116](ADR-116-cog-ha-matter-seed.md) (HA-COG Seed packaging) |
| **Tracking issue** | TBD — file under RuView issue tracker |

---

## 1. Context

### 1.1 What the pip package is today

`wifi-densepose` v1.1.0 was published to PyPI on **2025-06-07** (two releases the same
day: 1.0.0 at 13:24 UTC, 1.1.0 at 17:02 UTC). Both wheels carry the tag
`py3-none-any` — no compiled extension, no platform-specific code. The package is a
**pure-Python server application** sourced entirely from `archive/v1/`.

The package installs a 40-dependency stack including FastAPI, PyTorch, SQLAlchemy,
Redis, Celery, OpenCV, asyncpg, psycopg2, and Scapy (`archive/v1/setup.py:46–87`).
The declared entry points are:

```
wifi-densepose = src.cli:cli
wdp             = src.cli:cli
```

(`archive/v1/setup.py:178–179`)

The public API surface is centred on a FastAPI HTTP server, a SQLAlchemy/postgres
database layer, and a Redis/Celery task queue — none of which map to the current Rust
architecture. The `__init__.py` exports `app` (FastAPI), `CSIProcessor`,
`PhaseSanitizer`, `PoseEstimator`, `RouterInterface`, `ServiceOrchestrator`,
`HealthCheckService`, and `MetricsService` (`archive/v1/src/__init__.py:54–68`).

### 1.2 Why this matters now

ADR-115 (PR #778, merged 2026-05-23) shipped 21 Home Assistant entities, 10 semantic
primitives, mTLS, privacy mode, and a full witness bundle from the Rust crate
`wifi-densepose-sensing-server`. ADR-116 is packaging this as a Cognitum Seed cog.
Neither surface is reachable from `pip install wifi-densepose` — the pip package cannot
import a CsiFrame, decode an edge-vitals packet, call a DSP stage, verify a witness
bundle, or subscribe to the sensing server's MQTT or WebSocket endpoints. The ecosystem
split is now wide enough that the pip package actively misleads new users about what
the project does.

Three concrete customer pain points:

1. A Python user who `pip install wifi-densepose` expecting to consume live pose/vitals
   data gets a FastAPI server that requires postgres + redis, not a library they can
   script against.
2. Integrators writing HA automations or Node-RED flows in Python have no idiomatic
   Python API for the v0.7 telemetry surface (ADR-115 entities, semantic primitives).
3. The ADR-028 witness chain (deterministic pipeline proof) is Python-based and
   exercised via `archive/v1/data/proof/verify.py`, but it imports from the v1 stack —
   it cannot witness the Rust pipeline that is now the production implementation.

### 1.3 What this ADR is *not*

- Not a removal of `archive/v1/` from the repository. The v1 codebase stays as a
  research archive and its proof bundle stays in `archive/v1/data/proof/`.
- Not a port of the Rust crates to Python. The Rust workspace (`v2/`) is authoritative
  and unmodified by this ADR.
- Not a replacement of the `wifi-densepose-sensing-server` Rust binary. The pip
  package wraps or clients the binary; it does not reimplement it.
- Not an overlap with ADR-116 (Seed cog packaging). ADR-116 ships a Seed-installable
  artifact; ADR-117 ships a Python developer library for scripting, automation, and
  prototyping against the Rust stack.

---

## 2. Current state — evidence

| Artifact | Value | Source |
|---|---|---|
| Latest PyPI version | **1.1.0** | `pypi.org/pypi/wifi-densepose/json` |
| First release date | 2025-06-07T13:24:53Z | PyPI JSON metadata |
| Latest release date | 2025-06-07T17:02:40Z | PyPI JSON metadata |
| Months since last release | **~11.5 months** | as of 2026-05-24 |
| Wheel tag | `py3-none-any` | PyPI simple index |
| Hard dependencies | 40 (torch, fastapi, sqlalchemy, redis, celery, …) | `setup.py:46–87` |
| Entry point | `src.cli:cli` | `setup.py:178` |
| Python requires | `>=3.9` | `setup.py:108` |
| Classifiers Python versions | 3.9, 3.10, 3.11, 3.12 | PyPI JSON classifiers |
| Classifiers status | Beta (4) | PyPI JSON classifiers |
| Current Rust workspace version | **0.3.0** | `v2/Cargo.toml:version` |
| Rust crates in workspace | 20+ | `v2/Cargo.toml` members |
| ADR-115 shipped | 2026-05-23 | PR #778 |

The v1 source package (`archive/v1/setup.py:112–215`) was clearly designed as an
all-in-one server application, not a reusable library. The `find_packages` call at
line 134 searches from `"."` (the archive root), meaning the wheel ships `src.*` as the
importable namespace. The proof bundle (`archive/v1/data/proof/verify.py:56–57`) imports
`src.hardware.csi_extractor.CSIData` and `src.core.csi_processor.CSIProcessor` — v1 pure
Python only.

**PyPI org presence check:** a search for other `ruvnet`-published PyPI packages
(`ruvector`, `claude-flow`) returned no matches in the PyPI simple index as of this
writing. The `wifi-densepose` package is currently the only Python entry point for this
project's ecosystem.

---

## 3. Gap analysis

| Capability | Rust crate(s) | pip v1.1.0 status | Gap severity |
|---|---|---|---|
| `CsiFrame` / `CsiMetadata` core types | `wifi-densepose-core` (`types.rs`) | Not present — v1 uses `CSIData` Python class | **Critical** |
| HR/BR extraction from CSI buffer | `wifi-densepose-vitals` (4-stage pipeline: preprocessor → breathing → heartrate → anomaly) | Stub Python (`src/hardware/csi_extractor.py`) with no DSP | **Critical** |
| Phase sanitization / noise removal | `wifi-densepose-signal` (`phase_sanitizer`, `csi_processor`, `hampel`) | Python stubs in `src/core/phase_sanitizer.py` | **Critical** |
| Motion detection + presence scoring | `wifi-densepose-signal` (`motion.rs`, `MotionDetector`) | Not present | **Critical** |
| RuvSense multistatic sensing (13 modules) | `wifi-densepose-signal/src/ruvsense/` | Not present — ADR-029 post-dates v1 | **Critical** |
| 17-keypoint pose estimation | `wifi-densepose-nn`, `wifi-densepose-mat` | Stub `PoseEstimator` wrapping a `torch.nn.Module` that requires model weights | **High** |
| MQTT publisher (21 HA entities) | `wifi-densepose-sensing-server/src/mqtt/` | Not present — ADR-115 post-dates v1 | **High** |
| Semantic primitives (10 types) | `wifi-densepose-sensing-server/src/semantic/` | Not present | **High** |
| Matter bridge | `wifi-densepose-sensing-server/src/matter/` | Not present | **High** |
| WS/REST client for sensing-server | `wifi-densepose-sensing-server` (Axum) | v1 has a separate FastAPI server; no client | **High** |
| Witness bundle verification | ADR-028 / `scripts/generate-witness-bundle.sh` | `archive/v1/data/proof/verify.py` — proves v1 pipeline only | **High** |
| ESP32-C6 firmware telemetry (ADR-110) | `wifi-densepose-hardware` + `wifi-densepose-sensing-server` | Not present | **Medium** |
| Cross-viewpoint fusion (RuVector) | `wifi-densepose-ruvector/src/viewpoint/` | Not present | **Medium** |
| Semantic-primitive MQTT payload | `wifi-densepose-sensing-server/src/semantic/bus.rs` | Not present | **Medium** |
| PostgreSQL + Redis server mode | `archive/v1/` | Present (v1 only) | Low (not SOTA) |
| FastAPI HTTP REST server | `archive/v1/src/app.py` | Present (v1 only) | Low (not SOTA) |

---

## 4. Decision

Adopt **PyO3 + maturin Python extension bindings** as the primary modernization path,
shipping the pip package as a platform-native wheel (`manylinux`, `macosx`, `win-amd64`)
with compiled Rust extension modules, plus a pure-Python WS/MQTT client layer that talks
to a running `wifi-densepose-sensing-server` instance.

This path is called **PIP-PHOENIX**.

### 4.1 Why PyO3 + maturin over the three rejected alternatives

| Criterion | **PyO3 + maturin** (chosen) | Subprocess wrapper | REST/WS client only | Pure Python reimpl |
|---|---|---|---|---|
| Performance for DSP | Native Rust speed, zero copy | IPC overhead per call | N/A — no local DSP | Python bottleneck |
| Binary size in wheel | Core + vitals + signal only: ~2 MB stripped | Full sensing-server binary: ~15–30 MB | Minimal (~50 kB) | Minimal (~100 kB) |
| Works offline / no server | Yes | Yes (binary bundled) | No — server required | Partial |
| Proof bundle can cover Rust pipeline | Yes — bindings call the same Rust code the server uses | Partial — server is a black box | No | No |
| Install experience | `pip install wifi-densepose` — wheel has no system deps | `pip install` downloads 25 MB binary | `pip install` — pure Python | `pip install` — pure Python |
| Maintenance surface | Python bindings + Rust workspace | Python thin shim | Python client | Python reimpl must track Rust |
| Async / tokio support | PyO3 0.28 `pyo3-asyncio` or `pyo3-async-runtimes` for async export; sync entry points for the DSP hot path | N/A | Native asyncio on client | N/A |
| GIL concern | DSP-heavy calls release GIL via `py.allow_threads`; tokio runtime per module | N/A | None | N/A |
| Fits existing architecture | Core + vitals + signal already have clean public APIs (`lib.rs` re-exports) | Requires sensing-server to be running | Requires sensing-server | Forks the domain model |

**Subprocess wrapper** is rejected because shipping a 25 MB pre-built server binary
inside every pip wheel is an unacceptably heavy install, and it makes offline scripting
impossible without starting the server.

**REST/WS client only** is rejected because it provides zero DSP utility offline and
cannot close the witness gap — the proof bundle must exercise the same pipeline code.

**Pure Python reimplementation** is the root cause of the current drift and is
explicitly rejected.

The chosen path starts small: **bind only the three crates with the highest Python
utility** (`wifi-densepose-core`, `wifi-densepose-vitals`, `wifi-densepose-signal`),
ship a `py3-none-any` pure-Python WS/MQTT client layer as a separate sub-module, and
grow from there.

---

## 5. Detailed design

### 5.1 Rust crates bound in v2.0 (first wheel)

Three crates are in scope for the initial binding. They were chosen because they have
no heavy system dependencies (no libtorch, no ONNX runtime), have stable `pub` re-export
surfaces in `lib.rs`, and directly address the three most-requested missing capabilities.

| Crate | Exported Python types / functions | Binding rationale |
|---|---|---|
| `wifi-densepose-core` | `CsiFrame`, `CsiMetadata`, `Keypoint`, `KeypointType`, `PersonPose`, `PoseEstimate`, `Confidence`, `BoundingBox` | Foundation types shared by all other crates; without these users can't even describe a frame |
| `wifi-densepose-vitals` | `CsiVitalPreprocessor`, `BreathingExtractor`, `HeartRateExtractor`, `VitalAnomalyDetector`, `VitalSignStore`, `VitalReading`, `VitalEstimate`, `AnomalyAlert` | The most-asked-for surface: HR/BR from a CSI buffer in 4 lines of Python |
| `wifi-densepose-signal` | `CsiProcessor`, `CsiProcessorConfig`, `PhaseSanitizer`, `MotionDetector`, `MotionScore`, `FeatureExtractor`, `HardwareNormalizer` | DSP pipeline that produces the features vitals and pose estimation consume |

Crates **deferred to P6+**: `wifi-densepose-nn` (requires libtorch or candle — wheel
size risk), `wifi-densepose-mat` (depends on nn), `wifi-densepose-ruvector` (RuVector
GNN types — high value but adds ruvector-gnn 2.0.5 link dependency),
`wifi-densepose-hardware` (ESP32 HAL — not Python-scripting friendly).

### 5.2 New workspace member: `python/`

A new crate `python/` is added as a workspace member at `v2/crates/wifi-densepose-py/`.
It is a `cdylib` that re-exports the three bound crates behind a single maturin module
named `wifi_densepose._core`.

```toml
# v2/crates/wifi-densepose-py/Cargo.toml (sketch)
[package]
name = "wifi-densepose-py"
version.workspace = true
edition.workspace = true

[lib]
name = "_core"
crate-type = ["cdylib"]

[dependencies]
pyo3 = { version = "0.28", features = ["extension-module", "abi3-py310"] }
wifi-densepose-core   = { path = "../wifi-densepose-core", features = ["serde"] }
wifi-densepose-vitals = { path = "../wifi-densepose-vitals" }
wifi-densepose-signal = { path = "../wifi-densepose-signal" }
```

The `abi3-py310` feature locks the stable ABI to CPython 3.10+, so one wheel binary
works across 3.10, 3.11, 3.12, and 3.13 without recompilation.

PyO3 bindings pattern (example for `CsiFrame`):

```rust
// v2/crates/wifi-densepose-py/src/core_types.rs
use pyo3::prelude::*;
use wifi_densepose_core::CsiFrame as RustCsiFrame;

#[pyclass(name = "CsiFrame")]
#[derive(Clone)]
pub struct PyCsiFrame {
    inner: RustCsiFrame,
}

#[pymethods]
impl PyCsiFrame {
    #[new]
    fn new(amplitudes: Vec<f32>, phases: Vec<f32>, n_subcarriers: usize,
           sample_index: u64, sample_rate_hz: f32) -> Self {
        Self { inner: RustCsiFrame { amplitudes, phases, n_subcarriers,
                                     sample_index, sample_rate_hz } }
    }

    #[getter] fn amplitudes(&self) -> Vec<f32> { self.inner.amplitudes.clone() }
    #[getter] fn phases(&self) -> Vec<f32> { self.inner.phases.clone() }
    #[getter] fn n_subcarriers(&self) -> usize { self.inner.n_subcarriers }
}
```

DSP calls that execute >1 ms release the GIL:

```rust
#[pymethods]
impl PyCsiProcessor {
    fn process<'py>(&mut self, py: Python<'py>, frame: &PyCsiFrame)
        -> PyResult<Option<PyProcessedSignal>>
    {
        py.allow_threads(|| self.inner.process(&frame.inner))
            .map(|opt| opt.map(PyProcessedSignal::from))
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }
}
```

### 5.3 pip package layout

```
wifi-densepose/                  ← PyPI package name (unchanged)
  wifi_densepose/                ← importable namespace
    __init__.py                  ← re-exports core types + version
    _core.pyd / _core.so         ← compiled PyO3 extension (maturin build output)
    vitals.py                    ← thin Python wrapper + docstrings over _core vitals types
    signal.py                    ← thin Python wrapper over _core signal types
    client/
      __init__.py
      ws.py                      ← asyncio WebSocket client for sensing-server /ws/sensing
      mqtt.py                    ← paho-mqtt wrapper for ruview/<node_id>/raw/* topics
      ha.py                      ← helpers for HA-DISCO payloads (read-only, mirrors ADR-115 §3.2)
    witness/
      __init__.py
      verify.py                  ← Python-callable witness verifier (re-creates ADR-028 proof
                                     over the Rust pipeline via PyO3 bindings, not archive/v1/)
    compat/
      v1.py                      ← import shim that raises MigrationError (see §9)
    py.typed                     ← PEP 561 marker
```

The import path intentionally maps to Rust crate names:

```python
from wifi_densepose import CsiFrame           # core types
from wifi_densepose.vitals import BreathingExtractor, HeartRateExtractor
from wifi_densepose.signal import CsiProcessor, MotionDetector
from wifi_densepose.client.ws import SensingClient
from wifi_densepose.witness import verify_bundle
```

### 5.4 PyPI distribution — wheel matrix

Published as `wifi-densepose==2.0.0` using **cibuildwheel** driven by GitHub Actions.

| Platform | Arch | CPython | Tag (stable ABI) |
|---|---|---|---|
| `manylinux_2_28` | x86_64 | 3.10+ | `cp310-abi3-manylinux_2_28_x86_64` |
| `manylinux_2_28` | aarch64 | 3.10+ | `cp310-abi3-manylinux_2_28_aarch64` |
| `macosx_11_0` | x86_64 | 3.10+ | `cp310-abi3-macosx_11_0_x86_64` |
| `macosx_11_0` | arm64 | 3.10+ | `cp310-abi3-macosx_11_0_arm64` |
| `win` | amd64 | 3.10+ | `cp310-abi3-win_amd64` |
| sdist | — | — | source fallback |

The `abi3-py310` flag means **one binary per OS/arch** covers all supported Python
versions — 5 wheels total plus an sdist, compared to the 20-wheel matrix that would be
needed without stable ABI.

```yaml
# .github/workflows/pip-release.yml (sketch)
- uses: pypa/cibuildwheel@v2
  with:
    package-dir: v2/crates/wifi-densepose-py
    output-dir: dist
  env:
    CIBW_BUILD: "cp310-*"
    CIBW_ARCHS_LINUX: "x86_64 aarch64"
    CIBW_ARCHS_MACOS: "x86_64 arm64"
    CIBW_ARCHS_WINDOWS: "AMD64"
    CIBW_BEFORE_BUILD: "pip install maturin"
    CIBW_BUILD_FRONTEND: "build[uv]"
```

### 5.5 CLI parity

The pip wheel installs a `wifi-densepose` console script. In v2 this script is a thin
Python shim that:

1. Checks whether `wifi-densepose-sensing-server` binary is on `PATH` (installed
   separately via a platform-specific binary distribution or `cargo install`).
2. If found: proxies `wifi-densepose serve`, `wifi-densepose stream`, etc. to the Rust
   binary via `subprocess.run`.
3. If not found: falls back to the PyO3 module for offline DSP commands
   (`wifi-densepose vitals --file recording.jsonl`).

This is explicitly **not** a reimplementation of the CLI — the Rust binary
(`wifi-densepose-cli/src/main.rs`, currently exposes `mat` and `version` subcommands)
is the authoritative CLI. The pip shim is a discovery/convenience layer.

### 5.6 WS/MQTT client layer

`wifi_densepose.client.ws.SensingClient` is a pure-Python asyncio client wrapping the
sensing-server WebSocket at `/ws/sensing`:

```python
async with SensingClient("ws://localhost:8765/ws/sensing") as client:
    async for msg in client.stream():
        if msg.type == "edge_vitals":
            print(msg.breathing_rate_bpm, msg.heartrate_bpm)
```

`wifi_densepose.client.mqtt.RuViewMqttClient` wraps paho-mqtt and subscribes to
`ruview/<node_id>/raw/+` as defined in ADR-115 §3.2.

Both clients are **pure Python** (no PyO3) and are optional dependencies (`pip install
wifi-densepose[client]`). They depend on `websockets>=12` and `paho-mqtt>=2` respectively.

### 5.7a Beamforming Feedback Loop Data (BFLD) support — new binding target

**Added 2026-05-24 per maintainer feedback during P3 implementation.**

BFLD is the transmitter-side, AP-station-loop view of the WiFi channel
— compressed beamforming feedback frames that 802.11ac/ax/be stations
send to the AP per sounding cycle. From a sensing perspective it
complements receiver-side CSI:

| | Receiver-side CSI (current) | BFLD (this addition) |
|---|---|---|
| Source | RX side of the radio (e.g. Nexmon CSI on Pi 5, ESP32 promisc cb) | Sniffed BFR frames in the air or `mac80211` ACK trace |
| Subcarriers (HE20) | 52 (HT-LTF) or 242 (HE-LTF) | Up to 996 (HE160 compressed BFR) — denser |
| Hardware requirements | Patched Broadcom/Cypress or ESP32 specifically | **Any** 802.11ac+ station-AP pair — no patched firmware |
| Privacy model | Captures everyone in radio range | Same |
| Maturity in repo | Production (ADR-014, ADR-018, ADR-039) | Research; no Rust crate yet |
| Suitable use case | Through-wall pose + vitals | Dense subcarrier reflection profile for AETHER-class biometric (ADR-024) and the soul-signature spec (`docs/research/soul/`) |

#### Binding strategy

Because the Rust workspace has no `wifi-densepose-bfld` crate yet, P3
ships a **forward-compatible Python trait surface** that the future
Rust crate plugs into without changing the Python API:

```python
from wifi_densepose import BfldFrame, BfldReport

# Today (P3): construct from a parsed BFR feedback matrix (the bring-
# your-own-parser path). Users on Pi 5 + Wireshark BFR dissector
# pipe frames in directly.
frame = BfldFrame.from_compressed_feedback(
    timestamp_ms=…,
    sounding_index=…,
    sta_mac="aa:bb:cc:…",
    bandwidth_mhz=80,
    n_subcarriers=996,
    feedback_matrix=…,  # numpy ndarray complex64 [Nr × Nc × Nsc]
)

# P3 also ships a stub `BfldReport` aggregator that mirrors how
# `VitalEstimate` aggregates `VitalReading`s. Users who have BFR
# pipelines feeding RuView can use this today via the
# bring-your-own-parser path.

# Tomorrow (post-v2.0): the `wifi-densepose-bfld` Rust crate (TBD —
# separate ADR-1xx) provides ingestion from Nexmon `nl80211` traces +
# kernel `mac80211` debugfs hooks, and the pip wheel transparently
# binds it without changing this Python surface.
```

#### Why this matters

Three reasons BFLD belongs in v2.0 rather than waiting for the Rust
core:

1. **Customer pull**. Several integrators reading the ADR-115 release
   notes asked about WiFi-6 dense-subcarrier capture; the answer is
   BFLD, and we want the API stable before they build pipelines.
2. **Soul-signature dependency**. The soul-signature research spec
   (`docs/research/soul/specification.md`) lists "Subcarrier Reflection
   Profile" as one of seven biometric channels. At HE20/HE80 the
   dense BFR subcarriers are the right input — exposing `BfldFrame`
   now lets researchers prototype the channel without waiting on a
   Rust ingestion crate.
3. **Cross-vendor portability**. CSI ingestion needs patched
   firmware. BFR ingestion works on stock 802.11ac/ax hardware
   (capture via `tcpdump`/Wireshark + a BFR dissector). Shipping the
   Python data structures first gives the community a way to feed
   RuView from gear we don't directly support.

#### Implementation surface in P3

Lands as a new module `bindings/bfld.rs` (~150 lines, three
`#[pyclass]` types):

- `BfldFrame` (frozen) — one compressed feedback matrix snapshot.
  Constructors: `from_compressed_feedback(...)` and
  `from_uncompressed_v(...)` (the 802.11n V-matrix form).
  Properties: `timestamp_ms`, `sounding_index`, `sta_mac`,
  `bandwidth_mhz`, `n_subcarriers`, `n_rows` (Nr), `n_cols` (Nc),
  `feedback_matrix` (numpy ndarray complex64).
- `BfldReport` (frozen) — aggregator over a window of `BfldFrame`s.
  Properties: `n_frames`, `timestamp_first`, `timestamp_last`,
  `mean_amplitude_per_subcarrier`, `coherence_score`. The Python
  side gives users a stable handle for "all BFR data in this 60-s
  scan" without leaking the storage representation.
- `BfldKind` (`#[pyclass(eq, eq_int, hash, frozen)]`) — enum
  enumerating the BFR variants we support: `CompressedHE20`,
  `CompressedHE40`, `CompressedHE80`, `CompressedHE160`,
  `UncompressedHT20`, `UncompressedHT40`.

Stub Rust implementation lives in `python/src/bfld_stub.rs` until
the proper Rust crate exists; it's intentionally not in v2/crates/.
A new ADR-1xx will own the Rust ingestion crate when we commit to it.

#### Open questions added

- §9.11 — Should BFLD ingestion live in a new `wifi-densepose-bfld`
  crate or in `wifi-densepose-signal` extended?
- §9.12 — Per-vendor BFR variant compatibility (Broadcom vs Intel vs
  Qualcomm encode the compressed angles slightly differently) — how
  much normalisation belongs in the Python binding vs. the future
  Rust crate?

### 5.7 Witness chain (re-rooted to the Rust pipeline)

`wifi_densepose.witness.verify_bundle(path)` replaces the v1 proof verification with a
new chain that exercises the Rust pipeline via PyO3:

```python
from wifi_densepose.witness import verify_bundle

result = verify_bundle("dist/witness-bundle-ADR028-*/")
assert result.verdict == "PASS", result.detail
```

Internally it:
1. Loads the 1,000-frame reference JSON from the bundle.
2. Feeds each frame through `PyCsiProcessor` (PyO3 binding of the Rust `CsiProcessor`).
3. Hashes the output using the same SHA-256 scheme as `archive/v1/data/proof/verify.py`.
4. Compares against the published hash in `expected_features.sha256`.

The v1 proof (`archive/v1/data/proof/verify.py`) is **preserved unchanged** — it
continues to prove the v1 pipeline. The new `witness.py` proves the v2/Rust pipeline.
Both can coexist; the ADR-028 witness bundle ships with both.

---

## 6. Migration path (phased)

```
P1  ──►  P2  ──►  P3  ──►  P4  ──►  P5  ──►  P6+
scaffold  core   vitals+   client   publish  deferred
          types  signal    layer    v2.0.0
```

### P1 — Scaffold (1 week)

- [ ] Add `v2/crates/wifi-densepose-py/` as workspace member.
- [ ] `Cargo.toml`: `crate-type = ["cdylib"]`, pyo3 0.28 + `abi3-py310`, no
  workspace deps yet (empty module compiles and imports).
- [ ] `pyproject.toml` at repo root `python/` with `[build-system] requires =
  ["maturin>=1.8"]` and `[tool.maturin] features = ["pyo3/extension-module"]`.
- [ ] CI job: `maturin develop` on ubuntu-latest in a Python 3.12 venv; import
  `wifi_densepose._core` succeeds.
- [ ] Publish `wifi-densepose==1.99.0` to PyPI with a migration notice in the
  module body (see §9 — no new features, just the tombstone release).

### P2 — Core type bindings (1 week)

- [ ] Bind `CsiFrame`, `CsiMetadata`, `Confidence`, `Keypoint`, `KeypointType`,
  `BoundingBox`, `PoseEstimate`, `PersonPose` from `wifi-densepose-core`.
- [ ] All types: `__repr__`, `__eq__`, `__hash__` where meaningful; serde JSON
  round-trip via `pyo3-serde` or manual `to_dict()` / `from_dict()`.
- [ ] Add `py.typed` + stub `.pyi` file generated by `pyo3-stub-gen`.
- [ ] Unit tests: `tests/test_core.py` — construct each type, round-trip JSON.

### P3 — Vitals + signal DSP bindings (2 weeks)

- [ ] Bind the full 4-stage vitals pipeline:
  `CsiVitalPreprocessor`, `BreathingExtractor`, `HeartRateExtractor`,
  `VitalAnomalyDetector`, `VitalSignStore`, `VitalReading`, `VitalEstimate`,
  `AnomalyAlert`.
- [ ] Bind signal DSP entry points: `CsiProcessor`, `CsiProcessorConfig`,
  `PhaseSanitizer`, `MotionDetector`, `HardwareNormalizer`.
- [ ] GIL release (`py.allow_threads`) on all calls >0.5 ms (measured in bench).
- [ ] Integration test: feed 1,000 frames from `archive/v1/data/proof/sample_csi_data.json`
  through the PyO3 vitals pipeline; assert output is deterministic across runs.
- [x] Re-implement `witness/verify.py` using P3 bindings; compare SHA-256 against the
  v1 expected hash. **Note:** the hash will differ because the Rust and Python
  processors are not identical — generate and publish a new `expected_features_v2.sha256`.
  *Done 2026-11-10:* `python/wifi_densepose/witness.py`, hash generated in CI,
  provenance in ADR-028 §5.3.

### P4 — WS/MQTT client layer (1 week)

- [ ] Implement `wifi_densepose.client.ws.SensingClient` (asyncio, `websockets>=12`).
- [ ] Implement `wifi_densepose.client.mqtt.RuViewMqttClient` (paho-mqtt 2.x).
- [ ] Add `wifi_densepose.client.ha` helpers that parse ADR-115 MQTT discovery payloads
  into Python dataclasses.
- [ ] Integration test: spin up `sensing-server` in Docker with `--mock-frames`;
  assert `SensingClient` receives `edge_vitals` messages.

### P5 — First cibuildwheel publish as v2.0.0 (1 week)

- [ ] `.github/workflows/pip-release.yml` — cibuildwheel matrix (5 wheels + sdist).
- [ ] `python_requires = ">=3.10"` (stable ABI base).
- [ ] Populate `pyproject.toml` with minimal `install_requires`: `pyo3` is a build dep,
  not a runtime dep. Runtime extras: `[client]` adds `websockets>=12,paho-mqtt>=2`.
- [ ] `pip install wifi-densepose==2.0.0` and smoke-test on each CI platform.
- [ ] PyPI publish via Trusted Publisher (OIDC, no API token in secrets).
- [ ] Announce: `wifi-densepose==1.99.0` tombstone already on PyPI; `v2.0.0` replaces
  it in search results.

### P3.5 — BFLD binding surface (concurrent with P3)

**Added 2026-05-24 per maintainer feedback.** See §5.7a for the rationale.

- [ ] `python/src/bindings/bfld.rs` — `BfldFrame`, `BfldReport`,
  `BfldKind` `#[pyclass]` wrappers backed by a stub Rust impl
  pending the v3 `wifi-densepose-bfld` crate.
- [ ] `python/src/bfld_stub.rs` — minimal in-crate stub storage
  (vec of compressed feedback matrices) so the Python API is
  fully usable today even before the Rust ingestion crate lands.
- [ ] Numpy bridge for `feedback_matrix` (Complex64 ndarray) — same
  approach as `CsiFrame.amplitude` from P3.
- [ ] Tests covering: per-bandwidth constructor paths
  (HE20/HE40/HE80/HE160 + HT20/HT40), n_subcarriers contract,
  coherence_score sanity, BfldKind hashability + equality.
- [ ] Forward-compat contract test: `BfldFrame` constructed today
  from a numpy ndarray must round-trip through (de)serialisation
  identically once the Rust crate exists.
- [ ] §9.11 + §9.12 open questions raised so the eventual Rust crate
  has clear decisions waiting for it.

P3.5 is concurrent with P3 (no new schedule cushion needed) because
the Python surface is independent of the rest of the v2/ workspace.
Land in the same wheel as P3.

### P6+ — Deferred

- [ ] `wifi-densepose-bfld` Rust crate — proper ingestion from
  Nexmon BFR pcaps + `mac80211` debugfs. Replaces the P3.5 stub
  storage without changing the Python API. Owns its own ADR-1xx.
- [ ] `wifi-densepose-nn` bindings (libtorch / candle wheel size TBD — see Open
  Questions §13.3).
- [ ] `wifi-densepose-ruvector` bindings (RuVector attention types).
- [ ] MQTT/Matter integration helpers (`wifi_densepose.client.matter`).
- [ ] Deprecation notice on `wifi-densepose==1.x` releases (PyPI yank — see §9).
- [ ] `wifi-densepose-sensing-server` binary distribution via pip extra
  (`pip install wifi-densepose[server]` fetches pre-built binary for the platform).
- [ ] HACS Python integration built on top of the pip client layer (follow-on to
  ADR-115 §6.A).

---

## 7. Compatibility and deprecation

### 7.1 Version bump strategy

`wifi-densepose==2.0.0` is a **hard major-version break**. The 1.x import namespace
`src.*` is incompatible with the 2.x namespace `wifi_densepose.*`. There is no shim
that can bridge them transparently.

### 7.2 Tombstone release: v1.99.0

Before publishing v2.0.0, publish `wifi-densepose==1.99.0` as a pure-Python sdist/wheel
whose sole content is:

```python
# wifi_densepose/__init__.py  (v1.99.0)
raise ImportError(
    "wifi-densepose 1.x has been superseded by v2.0.0 which wraps "
    "the Rust-based stack. Run:\n\n"
    "    pip install wifi-densepose==2.0.0\n\n"
    "Migration guide: https://github.com/ruvnet/RuView/blob/main/docs/pip-migration.md\n"
    "Legacy v1 source: archive/v1/ in the repository"
)
```

This ensures any project pinned to `wifi-densepose>=1` that upgrades to 1.99.0 gets a
clear error rather than a silent broken import.

### 7.3 PyPI yank strategy

After v2.0.0 is stable (90-day observation window):

- Yank `wifi-densepose==1.0.0` — never had a separate stable release period; was
  superseded 4 hours after publication.
- Leave `wifi-densepose==1.1.0` un-yanked but deprecated in the description.
- Publish `wifi-densepose==1.99.0` as the canonical 1.x landing page (raise error).

Yanked versions remain installable with `pip install wifi-densepose==1.1.0 --force`
so users with reproducible builds pinned to exact versions are not broken silently.

### 7.4 Semver

| Version | Content |
|---|---|
| 1.0.0 – 1.1.0 | Legacy Python server (archive/v1/) |
| **1.99.0** | Tombstone: ImportError migration notice |
| **2.0.0** | PyO3 Rust bindings + WS/MQTT client |
| 2.x.y | Additive bindings + client improvements |
| 3.0.0 | If/when nn bindings added (libtorch wheel size may force a separate package) |

---

## 8. Alternatives considered and rejected

### Alt-A: Subprocess wrapper

Package the pre-built `wifi-densepose-sensing-server` Rust binary inside the pip wheel.
Python calls it via `subprocess`. **Rejected** because: the binary is 15–30 MB stripped;
the install footprint is prohibitive; offline DSP scripting still requires the server to
be running; the witness chain cannot exercise Rust code through a black-box binary.

### Alt-B: REST/WS client only

Ship a pure-Python package that is purely a client to a running `sensing-server`
instance. **Rejected** because: it provides zero offline utility; it cannot host the
witness chain over the Rust pipeline; it solves the "Python access to telemetry" problem
but not the "Python DSP / prototyping" problem that academic and embedded users need.

### Alt-C: Pure Python reimplementation

Rewrite the DSP pipeline in pure Python/NumPy to reach parity with the Rust
implementation. **Rejected explicitly** — this is the root cause of the current 11-month
drift and the pattern this ADR is designed to exit. Any Python reimplementation will
immediately begin drifting again as the Rust stack evolves.

---

## 9. Risks

| Risk | Likelihood | Severity | Mitigation |
|---|---|---|---|
| **Build matrix complexity** — 5 target triples × cibuildwheel setup; CI time; QEMU for aarch64 cross-compile | High | Medium | Use `abi3-py310` (5 wheels not 20); QEMU aarch64 emulation available in GitHub Actions; maturin handles auditwheel automatically |
| **Binary size** — future nn/ONNX bindings may push wheel past 50 MB | Medium | High | Keep nn bindings in a separate `wifi-densepose-nn` PyPI package; keep core+vitals+signal wheel lean (~2 MB stripped) |
| **GIL / async issues** — PyO3 wrapping tokio crates requires careful runtime management; `py.allow_threads` must be used around all blocking Rust calls | High | High | Restrict initial bindings to synchronous Rust APIs (vitals, signal, core are all sync); async sensing-server client stays in pure-Python `client/ws.py` |
| **Maintainer overhead** — two languages, two build systems, one PyPI package | Medium | Medium | maturin unifies the build; CI handles publishing; start with 3 bound crates only |
| **1.x user breakage** — users pinned to `wifi-densepose>=1,<2` will get the tombstone | Low | Medium | 1.99.0 tombstone gives a clear error; maintain 1.1.0 on PyPI un-yanked for 90 days post-v2 |
| **Windows Rust toolchain in CI** — linking PyO3 on Windows requires MSVC or mingw; extra CI complexity | Medium | Medium | GitHub Actions `windows-latest` has MSVC; maturin + cibuildwheel handle this natively |
| **Stable ABI limitations** — `abi3` precludes some advanced PyO3 features (e.g. `Buffer` protocol) | Low | Low | Core/vitals/signal types are scalar/Vec<f32> — no need for buffer protocol in P2–P3 |
| **PyPI name ownership** — we own `wifi-densepose` on PyPI (confirmed via rUv author field) | Low | Low | Confirm with `pypi.org/user/ruvnet` before publishing |

---

## 10. Acceptance criteria

The following checks must all pass before ADR-117 is considered Accepted:

- [ ] `pip install wifi-densepose==2.0.0` succeeds on Python 3.10, 3.11, 3.12, 3.13
  on linux/x86_64, macos/arm64, and windows/amd64 in a clean venv with no extra build tools.
- [ ] `python -c "import wifi_densepose; print(wifi_densepose.__version__)"` prints `2.0.0`.
- [ ] `python -c "from wifi_densepose import CsiFrame; f = CsiFrame([1.0]*56, [0.0]*56, 56, 0, 100.0); print(f)"` produces a non-error repr.
- [ ] The 4-stage vitals pipeline processes 1,000 frames in under 500 ms on a
  reference machine (CPython 3.12, linux x86_64, no GPU).
- [ ] `wifi_densepose.witness.verify_bundle(path)` returns `verdict="PASS"` for a
  freshly generated witness bundle from `scripts/generate-witness-bundle.sh`.
- [ ] `wifi_densepose.client.ws.SensingClient` receives at least one `edge_vitals`
  message from a `sensing-server --mock-frames` instance within 5 seconds.
- [ ] `pip install wifi-densepose==1.99.0` raises `ImportError` with the migration URL.
- [ ] The compiled `_core` extension has no unresolved dynamic library dependencies
  beyond libc/msvcrt (verified by `auditwheel show` on Linux, `delocate-listdeps` on macOS).
- [ ] Type stubs (`wifi_densepose/*.pyi`) are present; `mypy --strict` passes on the
  example code in `examples/vitals_from_buffer.py`.
- [ ] Total wheel size for core+vitals+signal: `≤ 5 MB` per platform.

---

## 11. Open questions

1. **Stable ABI base version**: `abi3-py310` drops support for Python 3.9, which v1.1.0
   declared. Is Python 3.9 EOL-enough (EOL 2025-10-05) to drop cleanly? *Tentative: yes,
   drop 3.9. Use abi3-py310.*

2. **Package name for nn bindings**: if `wifi-densepose-nn` bindings require a 30 MB
   libtorch wheel, should they live at `wifi-densepose-nn` (separate PyPI package) or
   as an optional heavy extra of `wifi-densepose[nn]`? *Tentative: separate package to
   avoid polluting the lean wheel.*

3. **Witness hash continuity**: the Rust pipeline will produce a different SHA-256 than
   the v1 Python pipeline for the same input frames. The new `expected_features_v2.sha256`
   must be generated and committed before v2.0.0 ships. Who generates it, and how is
   the generation process itself witnessed? *Tentative: generate in CI, commit hash to
   `archive/v1/data/proof/`, include in ADR-028 matrix.*

4. **`ruv-neural` crate**: `v2/crates/ruv-neural/` exists in the workspace. Is it a
   candidate for early Python bindings (useful for training-loop scripting), or should
   it wait for the nn/train tier? *Tentative: defer — it depends on training backends.*

5. **Tokio runtime**: `wifi-densepose-sensing-server` is tokio-based, but the three
   crates bound in P2–P3 (`core`, `vitals`, `signal`) are synchronous. Are there any
   hidden tokio dependencies that would force a runtime into the extension module?
   *Tentative: inspect each crate's Cargo.toml for tokio deps before P1 scaffold.*

6. **`pyo3-stub-gen` vs manual stubs**: automated stub generation from PyO3 has rough
   edges for generics and newtype patterns. Should we hand-write `.pyi` stubs for the
   first release? *Tentative: use `pyo3-stub-gen` for scaffolding, hand-tune for public
   API.*

7. **`wifi_densepose` vs `wifi-densepose` namespace**: the pip package name uses a dash
   (`wifi-densepose`) but Python imports use underscores (`wifi_densepose`). The v1
   package shipped under `src.*`, not `wifi_densepose.*`. Is there any tooling that
   hardcodes the `src` namespace? *Tentative: the `src.*` namespace was specific to
   `archive/v1/` and is cleanly dropped.*

8. **cibuildwheel version**: the current stable is cibuildwheel v2.x. Does the
   project's existing GitHub Actions config need updates for maturin builds vs
   the current `cargo build` / `build.py` patterns? *Tentative: yes, add a separate
   `pip-release.yml` workflow; do not modify existing Rust CI.*

9. **RuVector bindings timeline**: the `wifi-densepose-ruvector` crate (`v2/crates/`)
   depends on `ruvector-gnn = "2.0.5"`. Does ruvector-gnn ship as a pre-built static
   lib or require linking at build time? This directly affects the P6+ wheel size.
   *Tentative: investigate ruvector-gnn link strategy before committing to a timeline.*

10. **`wifi_densepose.client.ha` conflict with ADR-115/116**: the `ha.py` helper module
    should not duplicate the ADR-115 MQTT discovery logic in Python. Should it be read-only
    (parse HA discovery JSON → Python dataclasses) or also write (publish discovery JSON)?
    *Tentative: read-only for v2.0. Write path deferred to the HACS integration follow-on
    (ADR-115 §6.A).*

11. **BFLD Rust crate ownership** (added 2026-05-24): the P3.5 BFLD bindings ship with a
    stub Rust impl in `python/src/bfld_stub.rs`. The proper Rust crate (Nexmon BFR pcap
    parser + `mac80211` debugfs ingestor) will land later. Should it be a new
    `wifi-densepose-bfld` workspace member, or should it extend `wifi-densepose-signal`?
    *Tentative: new dedicated crate. Reasons: (a) the BFR parser is significant code
    (Wireshark's dissector is ~2k lines) and bloats `-signal`; (b) BFLD ingestion is
    optional — many deployments will only use CSI; gating behind a separate crate keeps
    the default `-signal` lean. Decide before committing to the crate name in any
    `pyproject.toml` extras.*

12. **BFLD per-vendor compressed-angle variants** (added 2026-05-24): 802.11 standardizes
    the compressed beamforming feedback format but vendors (Broadcom, Intel, Qualcomm,
    MediaTek) differ in psi/phi quantization step + ordering of consecutive matrix
    entries. How much normalisation belongs in the Python `BfldFrame.from_compressed_feedback`
    binding vs. the future Rust crate? *Tentative: Python binding is dumb (numpy ndarray
    in, numpy ndarray out — no decoding); the future Rust crate owns per-vendor
    normalisation, exposed via a `Vendor` enum on the binding constructor. Confirm via
    a per-vendor test fixture before P3.5 ships.*

---

## 12. References

### BFLD references (added 2026-05-24 for §5.7a + §11.11 + §11.12)

- Hernandez & Bulut, *"Wi-Fi Sensing With Compressed Beamforming Feedback"*, ACM TOSN 2024 — first systematic survey of BFR-as-sensing
- Yousefi, Soltanaghaei & Bharadia, *"Just-In-Time Wi-Fi Sensing Using Compressed Beamforming Feedback"*, MobiSys 2023 — practical pipeline for breath + heart-rate extraction from sniffed BFR
- IEEE 802.11ax-2021 §27.3.10 — Compressed Beamforming Feedback frame format
- Wireshark BFR dissector — `packet-ieee80211.c` reference implementation
- AX210 Linux mac80211 debugfs BFR capture path (kernel 6.10+)
- Sample BFR-vs-CSI parity dataset — TBD; we'll publish one alongside the
  `wifi-densepose-bfld` crate when it lands

### Original references

- **PyPI package (current)**: https://pypi.org/project/wifi-densepose/ — v1.1.0, released 2025-06-07
- **PyPI JSON metadata**: https://pypi.org/pypi/wifi-densepose/json
- **Local source**: `archive/v1/setup.py`, `archive/v1/src/__init__.py`, `archive/v1/data/proof/verify.py`
- **Rust workspace**: `v2/Cargo.toml`, `v2/crates/wifi-densepose-core/src/lib.rs`,
  `v2/crates/wifi-densepose-vitals/src/lib.rs`, `v2/crates/wifi-densepose-signal/src/lib.rs`,
  `v2/crates/wifi-densepose-sensing-server/src/lib.rs`
- **PyO3 docs**: https://pyo3.rs/ — v0.28.3 stable, Rust ≥1.83 required
- **maturin docs**: https://maturin.rs/ — supports Python 3.8+ on Linux/macOS/Windows/FreeBSD
- **cibuildwheel docs**: https://cibuildwheel.pypa.io/
- **ADR-021**: ESP32 vitals — defines the HR/BR extraction pipeline this ADR exposes in Python
- **ADR-028**: ESP32 capability audit — defines the witness bundle format `witness/verify.py` must re-verify
- **ADR-115**: HA-DISCO + HA-MIND + HA-FABRIC — defines the MQTT topic structure the `client/mqtt.py` helper consumes
- **ADR-116**: HA-COG cog packaging — parallel effort; ADR-117 pip library is the developer-facing Python surface; ADR-116 is the Seed-installable artifact
