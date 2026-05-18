# RuView · Developer Handbook

All the detailed crate maps, module tables, build commands, firmware
flashing recipes, publishing order, witness verification, swarm tooling
and memory commands extracted from `CLAUDE.md` to keep that file
under 200 lines. CLAUDE.md retains only the rules-of-engagement
sections; everything else lives here.

---

## Rust Workspace (v2/)

### Key Rust Crates
| Crate | Description |
|-------|-------------|
| `wifi-densepose-core` | Core types, traits, error types, CSI frame primitives |
| `wifi-densepose-signal` | SOTA signal processing + RuvSense multistatic sensing (14 modules) |
| `wifi-densepose-nn` | Neural network inference (ONNX, PyTorch, Candle backends) |
| `wifi-densepose-train` | Training pipeline with ruvector integration + ruview_metrics |
| `wifi-densepose-mat` | Mass Casualty Assessment Tool — disaster survivor detection |
| `wifi-densepose-hardware` | ESP32 aggregator, TDM protocol, channel hopping firmware |
| `wifi-densepose-ruvector` | RuVector v2.0.4 integration + cross-viewpoint fusion (5 modules) |
| `wifi-densepose-api` | REST API (Axum) |
| `wifi-densepose-db` | Database layer (Postgres, SQLite, Redis) |
| `wifi-densepose-config` | Configuration management |
| `wifi-densepose-wasm` | WebAssembly bindings for browser deployment |
| `wifi-densepose-cli` | CLI tool (`wifi-densepose` binary) |
| `wifi-densepose-sensing-server` | Lightweight Axum server for WiFi sensing UI |
| `wifi-densepose-wifiscan` | Multi-BSSID WiFi scanning (ADR-022) |
| `wifi-densepose-vitals` | ESP32 CSI-grade vital sign extraction (ADR-021) |
| `nvsim` | Deterministic NV-diamond magnetometer pipeline simulator (ADR-089) — standalone leaf, WASM-ready |
| `vendor/rvcsi` (submodule) | **rvCSI** — edge RF sensing runtime (ADR-095/096): 9 crates (`rvcsi-core`/`-dsp`/`-events`/`-adapter-file`/`-adapter-nexmon`/`-ruvector`/`-runtime`/`-node`/`-cli`). Lives in its own repo ([github.com/ruvnet/rvcsi](https://github.com/ruvnet/rvcsi)), vendored here under `vendor/rvcsi`, published to crates.io as `rvcsi-* 0.3.x` and to npm as `@ruv/rvcsi`. Not a `v2/` workspace member — depend on the published crates (or the submodule's `crates/rvcsi-*` paths). Normalized `CsiFrame`/`CsiWindow`/`CsiEvent` schema, validate-before-FFI, reusable DSP, typed confidence-scored events, the napi-c Nexmon shim (real nexmon_csi `.pcap` from a Raspberry Pi 5 / 4 / 3B+ — BCM43455c0), the napi-rs SDK, the `rvcsi` CLI, a Claude Code plugin. |

### RuvSense Modules (`signal/src/ruvsense/`)
| Module | Purpose |
|--------|---------|
| `multiband.rs` | Multi-band CSI frame fusion, cross-channel coherence |
| `phase_align.rs` | Iterative LO phase offset estimation, circular mean |
| `multistatic.rs` | Attention-weighted fusion, geometric diversity |
| `coherence.rs` | Z-score coherence scoring, DriftProfile |
| `coherence_gate.rs` | Accept/PredictOnly/Reject/Recalibrate gate decisions |
| `pose_tracker.rs` | 17-keypoint Kalman tracker with AETHER re-ID embeddings |
| `field_model.rs` | SVD room eigenstructure, perturbation extraction |
| `tomography.rs` | RF tomography, ISTA L1 solver, voxel grid |
| `longitudinal.rs` | Welford stats, biomechanics drift detection |
| `intention.rs` | Pre-movement lead signals (200-500ms) |
| `cross_room.rs` | Environment fingerprinting, transition graph |
| `gesture.rs` | DTW template matching gesture classifier |
| `adversarial.rs` | Physically impossible signal detection, multi-link consistency |

### Cross-Viewpoint Fusion (`ruvector/src/viewpoint/`)
| Module | Purpose |
|--------|---------|
| `attention.rs` | CrossViewpointAttention, GeometricBias, softmax with G_bias |
| `geometry.rs` | GeometricDiversityIndex, Cramer-Rao bounds, Fisher Information |
| `coherence.rs` | Phase phasor coherence, hysteresis gate |
| `fusion.rs` | MultistaticArray aggregate root, domain events |

### RuVector v2.0.4 Integration (ADR-016 complete, ADR-017 proposed)
All 5 ruvector crates integrated in workspace:
- `ruvector-mincut` → `metrics.rs` (DynamicPersonMatcher) + `subcarrier_selection.rs`
- `ruvector-attn-mincut` → `model.rs` (apply_antenna_attention) + `spectrogram.rs`
- `ruvector-temporal-tensor` → `dataset.rs` (CompressedCsiBuffer) + `breathing.rs`
- `ruvector-solver` → `subcarrier.rs` (sparse interpolation 114→56) + `triangulation.rs`
- `ruvector-attention` → `model.rs` (apply_spatial_attention) + `bvp.rs`


## Architecture Decisions, Hardware, Build Commands

### Architecture Decisions
43 ADRs in `docs/adr/` (ADR-001 through ADR-043). Key ones:
- ADR-014: SOTA signal processing (Accepted)
- ADR-015: MM-Fi + Wi-Pose training datasets (Accepted)
- ADR-016: RuVector training pipeline integration (Accepted — complete)
- ADR-017: RuVector signal + MAT integration (Proposed — next target)
- ADR-024: Contrastive CSI embedding / AETHER (Accepted)
- ADR-027: Cross-environment domain generalization / MERIDIAN (Accepted)
- ADR-028: ESP32 capability audit + witness verification (Accepted)
- ADR-029: RuvSense multistatic sensing mode (Proposed)
- ADR-030: RuvSense persistent field model (Proposed)
- ADR-031: RuView sensing-first RF mode (Proposed)
- ADR-032: Multistatic mesh security hardening (Proposed)

### Supported Hardware

| Device | Port | Chip | Role | Cost |
|--------|------|------|------|------|
| ESP32-S3 (8MB flash) | COM7 | Xtensa dual-core | WiFi CSI sensing node | ~$9 |
| ESP32-S3 SuperMini (4MB) | — | Xtensa dual-core | WiFi CSI (compact) | ~$6 |
| ESP32-C6 + Seeed MR60BHA2 | COM4 | RISC-V + 60 GHz FMCW | mmWave HR/BR/presence | ~$15 |
| HLK-LD2410 | — | 24 GHz FMCW | Presence + distance | ~$3 |

**Not supported:** ESP32 (original), ESP32-C3 — single-core, can't run CSI DSP pipeline.

### Build & Test Commands (this repo)
```bash
# Rust — full workspace tests (1,031+ tests, ~2 min)
cd v2
cargo test --workspace --no-default-features

# Rust — single crate check (no GPU needed)
cargo check -p wifi-densepose-train --no-default-features

# Python — deterministic proof verification (SHA-256)
python archive/v1/data/proof/verify.py

# Python — test suite
cd archive/v1 && python -m pytest tests/ -x -q
```


## Firmware Build + Flash + Provision + Release + Publish

### ESP32 Firmware Build (Windows — Python subprocess required)
```bash
# Build 8MB firmware (real WiFi CSI mode, no mocks)
# See CLAUDE.local.md for the full Python subprocess command
# Key: must strip MSYSTEM env vars for ESP-IDF v5.4 on Git Bash

# Build 4MB firmware
cp sdkconfig.defaults.4mb sdkconfig.defaults
# then same build process

# Flash to COM7
# [python, idf_py, '-p', 'COM7', 'flash']

# Provision WiFi
python firmware/esp32-csi-node/provision.py --port COM7 \
  --ssid "YourWiFi" --password "secret" --target-ip 192.168.1.20

# Monitor serial
python -m serial.tools.miniterm COM7 115200
```

### Firmware Release Process
1. Build 8MB from `sdkconfig.defaults.template` (no mock)
2. Build 4MB from `sdkconfig.defaults.4mb` (no mock)
3. Save 6 binaries: `esp32-csi-node.bin`, `bootloader.bin`, `partition-table.bin`, `ota_data_initial.bin`, `esp32-csi-node-4mb.bin`, `partition-table-4mb.bin`
4. Tag: `git tag v0.X.Y-esp32 && git push origin v0.X.Y-esp32`
5. Release: `gh release create v0.X.Y-esp32 <binaries> --title "..." --notes-file ...`
6. Verify on real hardware (COM7) before publishing
7. **CRITICAL:** Always test with real WiFi CSI, not mock mode — mock missed the Kconfig threshold bug

### Crate Publishing Order
Crates must be published in dependency order:
1. `wifi-densepose-core` (no internal deps)
2. `wifi-densepose-vitals` (no internal deps)
3. `wifi-densepose-wifiscan` (no internal deps)
4. `wifi-densepose-hardware` (no internal deps)
5. `wifi-densepose-config` (no internal deps)
6. `wifi-densepose-db` (no internal deps)
7. `wifi-densepose-signal` (depends on core)
8. `wifi-densepose-nn` (no internal deps, workspace only)
9. `wifi-densepose-ruvector` (no internal deps, workspace only)
10. `wifi-densepose-train` (depends on signal, nn)
11. `wifi-densepose-mat` (depends on core, signal, nn)
12. `wifi-densepose-api` (no internal deps)
13. `wifi-densepose-wasm` (depends on mat)
14. `wifi-densepose-sensing-server` (depends on wifiscan)
15. `wifi-densepose-cli` (depends on mat)


## Validation & Witness Verification (ADR-028)

### Validation & Witness Verification (ADR-028)

**After any significant code change, run the full validation:**

```bash
# 1. Rust tests — must be 1,031+ passed, 0 failed
cd v2
cargo test --workspace --no-default-features

# 2. Python proof — must print VERDICT: PASS
cd ..
python archive/v1/data/proof/verify.py

# 3. Generate witness bundle (includes both above + firmware hashes)
bash scripts/generate-witness-bundle.sh

# 4. Self-verify the bundle — must be 7/7 PASS
cd dist/witness-bundle-ADR028-*/
bash VERIFY.sh
```

**If the Python proof hash changes** (e.g., numpy/scipy version update):
```bash
# Regenerate the expected hash, then verify it passes
python archive/v1/data/proof/verify.py --generate-hash
python archive/v1/data/proof/verify.py
```

**Witness bundle contents** (`dist/witness-bundle-ADR028-<sha>.tar.gz`):
- `WITNESS-LOG-028.md` — 33-row attestation matrix with evidence per capability
- `ADR-028-esp32-capability-audit.md` — Full audit findings
- `proof/verify.py` + `expected_features.sha256` — Deterministic pipeline proof
- `test-results/rust-workspace-tests.log` — Full cargo test output
- `firmware-manifest/source-hashes.txt` — SHA-256 of all 7 ESP32 firmware files
- `crate-manifest/versions.txt` — All 15 crates with versions
- `VERIFY.sh` — One-command self-verification for recipients

**Key proof artifacts:**
- `archive/v1/data/proof/verify.py` — Trust Kill Switch: feeds reference signal through production pipeline, hashes output
- `archive/v1/data/proof/expected_features.sha256` — Published expected hash
- `archive/v1/data/proof/sample_csi_data.json` — 1,000 synthetic CSI frames (seed=42)
- `docs/WITNESS-LOG-028.md` — 11-step reproducible verification procedure
- `docs/adr/ADR-028-esp32-capability-audit.md` — Complete audit record

### Branch
Default branch: `main`
Active feature branch: `ruvsense-full-implementation` (PR #77)

