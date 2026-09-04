# Architecture Decision Records

Latest proposed decisions:

- [ADR-187: archive/v1 deprecation + model-weights honest labeling](ADR-187-archive-v1-deprecation-honest-labeling.md) (refs #509, #1125)
- [ADR-186: Training progress API — wire the orphaned in-server trainer to /ws/train/progress](ADR-186-training-progress-api.md) (refs #1233)
- [ADR-185: Python P6 SOTA bindings — AETHER, MERIDIAN, MAT](ADR-185-python-p6-sota-bindings.md)
- [ADR-184: ADR-117 completion via PyPI Trusted Publishing](ADR-184-adr117-completion-pypi-trusted-publishing.md) (refs #785)
- [ADR-264: Versioned wire protocol for RTL8720F CFR and Range-FFT reports](ADR-264-rtl8720f-radar-wire-protocol.md)
- [ADR-263: Adopt RTL8720F 2.4 GHz FMCW radar as an optional RuView sensing platform](ADR-263-rtl8720f-2-4ghz-fmcw-radar-platform.md)

This folder contains 210 Architecture Decision Records (ADRs) that document every significant technical choice in the RuView / WiFi-DensePose project. (The index tables below list a curated subset per domain; see the directory listing for the full set.)

## Why ADRs?

Building a system that turns WiFi signals into human pose estimation involves hundreds of non-obvious decisions: which signal processing algorithms to use, how to bridge ESP32 firmware to a Rust pipeline, whether to run inference on-device or on a server, how to handle multi-person separation with limited subcarriers.

ADRs capture the **context**, **options considered**, **decision made**, and **consequences** for each of these choices. They serve three purposes:

1. **Institutional memory** — Six months from now, anyone (human or AI) can read *why* we chose IIR bandpass filters over FIR for vital sign extraction, not just see the code.

2. **AI-assisted development** — When an AI agent works on this codebase, ADRs give it the constraints and rationale it needs to make changes that align with the existing architecture. Without them, AI-generated code tends to drift — reinventing patterns that already exist, contradicting earlier decisions, or optimizing for the wrong tradeoffs.

3. **Review checkpoints** — Each ADR is a reviewable artifact. When a proposed change touches the architecture, the ADR forces the author to articulate tradeoffs *before* writing code, not after.

### ADRs and Domain-Driven Design

The project uses [Domain-Driven Design](../ddd/) (DDD) to organize code into bounded contexts — each with its own language, types, and responsibilities. ADRs and DDD work together:

- **ADRs define boundaries**: ADR-029 (RuvSense) established multistatic sensing as a separate bounded context from single-node CSI. ADR-042 (CHCI) defined a new aggregate root for coherent channel imaging.
- **DDD models define the language**: The [RuvSense domain model](../ddd/ruvsense-domain-model.md) defines terms like "coherence gate", "dwell time", and "TDM slot" that ADRs reference precisely.
- **Together they prevent drift**: An AI agent reading ADR-039 knows that edge processing tiers are configured via NVS keys, not compile-time flags — because the ADR says so. The DDD model tells it which aggregate owns that configuration.

### How ADRs are structured

Each ADR follows a consistent format:

- **Context** — What problem or gap prompted this decision
- **Decision** — What we chose to do and how
- **Consequences** — What improved, what got harder, and what risks remain
- **References** — Related ADRs, papers, and code paths

Statuses: **Proposed** (under discussion), **Accepted** (approved and/or implemented), **Superseded** (replaced by a later ADR).

---

## ADR Index

### Hardware and firmware

| ADR | Title | Status |
|-----|-------|--------|
| [ADR-012](ADR-012-esp32-csi-sensor-mesh.md) | ESP32 CSI Sensor Mesh for Distributed Sensing | Accepted (partial) |
| [ADR-018](ADR-018-esp32-dev-implementation.md) | ESP32 Development Implementation Path | Proposed |
| [ADR-028](ADR-028-esp32-capability-audit.md) | ESP32 Capability Audit and Witness Record | Accepted |
| [ADR-029](ADR-029-ruvsense-multistatic-sensing-mode.md) | RuvSense Multistatic Sensing Mode (TDM, channel hopping) | Proposed |
| [ADR-032](ADR-032-multistatic-mesh-security-hardening.md) | Multistatic Mesh Security Hardening | Accepted |
| [ADR-039](ADR-039-esp32-edge-intelligence.md) | ESP32-S3 Edge Intelligence Pipeline (on-device vitals) | Accepted (hardware-validated) |
| [ADR-040](ADR-040-wasm-programmable-sensing.md) | WASM Programmable Sensing (Tier 3) | Accepted |
| [ADR-041](ADR-041-wasm-module-collection.md) | WASM Module Collection (65 edge modules) | Accepted (hardware-validated) |
| [ADR-044](ADR-044-provisioning-tool-enhancements.md) | Provisioning Tool Enhancements | Proposed |
| [ADR-110](ADR-110-esp32-c6-firmware-extension.md) | ESP32-C6 firmware extension — Wi-Fi 6 / 802.15.4 / TWT / LP-core | Accepted, P1-P10 complete, firmware-side substrate closed at **[v0.7.0-esp32](https://github.com/ruvnet/RuView/releases/tag/v0.7.0-esp32)**. Companion docs: [`WITNESS-LOG-110`](../WITNESS-LOG-110.md) (13 §A0.x entries · 99.56 % cross-board RX · **104.1 µs smoothed sync stdev** · ≤100 µs target met), [`ADR-110-REVIEW-GUIDE`](../ADR-110-REVIEW-GUIDE.md) (one-page reviewer tour), [`ADR-110-BRANCH-STATE`](../ADR-110-BRANCH-STATE.md) (coordination map vs `feat/adr-115-ha-mqtt-matter`). Host decoders + tests: Python `SyncPacketParser` (10) + Rust `wifi_densepose_hardware::SyncPacket` (15), cross-language hex pin gates drift. |

### Signal processing and sensing

| ADR | Title | Status |
|-----|-------|--------|
| [ADR-013](ADR-013-feature-level-sensing-commodity-gear.md) | Feature-Level Sensing on Commodity Gear | Accepted |
| [ADR-014](ADR-014-sota-signal-processing.md) | SOTA Signal Processing Algorithms | Accepted |
| [ADR-021](ADR-021-vital-sign-detection-rvdna-pipeline.md) | Vital Sign Detection (breathing, heart rate) | Partial |
| [ADR-030](ADR-030-ruvsense-persistent-field-model.md) | Persistent Field Model and Drift Detection | Proposed |
| [ADR-033](ADR-033-crv-signal-line-sensing-integration.md) | CRV Signal Line Sensing Integration | Proposed |
| [ADR-037](ADR-037-multi-person-pose-detection.md) | Multi-Person Pose Detection from Single ESP32 | Proposed |
| [ADR-042](ADR-042-coherent-human-channel-imaging.md) | Coherent Human Channel Imaging (beyond CSI) | Proposed |
| [ADR-134](ADR-134-csi-to-cir-time-domain-multipath.md) | First-Class Channel Impulse Response (CIR) Support | Proposed |
| [ADR-135](ADR-135-empty-room-baseline-calibration.md) | Empty-Room Baseline Calibration (per-subcarrier Welford statistics) | Proposed |

### Machine learning and training

| ADR | Title | Status |
|-----|-------|--------|
| [ADR-005](ADR-005-sona-self-learning-pose-estimation.md) | SONA Self-Learning for Pose Estimation | Partial |
| [ADR-006](ADR-006-gnn-enhanced-csi-pattern-recognition.md) | GNN-Enhanced CSI Pattern Recognition | Partial |
| [ADR-015](ADR-015-public-dataset-training-strategy.md) | Public Dataset Strategy (MM-Fi, Wi-Pose) | Accepted |
| [ADR-016](ADR-016-ruvector-integration.md) | RuVector Training Pipeline Integration | Accepted |
| [ADR-017](ADR-017-ruvector-signal-mat-integration.md) | RuVector Signal + MAT Integration | Proposed |
| [ADR-020](ADR-020-rust-ruvector-ai-model-migration.md) | Migrate AI Inference to Rust (ONNX Runtime) | Accepted |
| [ADR-023](ADR-023-trained-densepose-model-ruvector-pipeline.md) | Trained DensePose Model with RuVector Pipeline | Proposed |
| [ADR-024](ADR-024-contrastive-csi-embedding-model.md) | Project AETHER: Contrastive CSI Embeddings | Required |
| [ADR-027](ADR-027-cross-environment-domain-generalization.md) | Project MERIDIAN: Cross-Environment Generalization | Proposed |
| [ADR-149](ADR-149-public-community-leaderboard-huggingface.md) | AetherArena: public spatial-intelligence benchmark on Hugging Face | Proposed |
| [ADR-150](ADR-150-rf-foundation-encoder.md) | RF Foundation Encoder: pose-preserving, subject/room/device-invariant CSI embedding | Proposed |
| [ADR-151](ADR-151-room-calibration-specialist-training.md) | Per-Room Calibration & Specialized Model Training (room-first → bank of small ruVector specialists) | Proposed |
| [ADR-152](ADR-152-wifi-pose-sota-2026-intake.md) | WiFi-Pose SOTA 2026 Intake: geometry-conditioned calibration, external benchmarks, foundation-encoder recipe | Proposed |
| [ADR-348](ADR-348-independent-rust-multivariate-forecasting.md) | Independent Rust multivariate forecasting for RuView | Proposed |
| [ADR-349](ADR-349-governed-local-and-fal-forecast-training.md) | Governed local and fal.ai forecast training | Proposed |
| [ADR-350](ADR-350-ruvector-predictive-memory-and-ruvllm-boundary.md) | RuVector predictive memory and RuVLLM authority boundary | Proposed |
| [ADR-354](ADR-354-forecast-governance-enforcement-in-ruforecast-submodule.md) | fal.ai forecast governance and spend enforcement now real, in the vendored RuForecast submodule | Accepted |

### Platform and UI

| ADR | Title | Status |
|-----|-------|--------|
| [ADR-019](ADR-019-sensing-only-ui-mode.md) | Sensing-Only UI with Gaussian Splats | Accepted |
| [ADR-022](ADR-022-windows-wifi-enhanced-fidelity-ruvector.md) | Windows WiFi Enhanced Fidelity (multi-BSSID) | Partial |
| [ADR-025](ADR-025-macos-corewlan-wifi-sensing.md) | macOS CoreWLAN WiFi Sensing | Proposed |
| [ADR-031](ADR-031-ruview-sensing-first-rf-mode.md) | RuView Sensing-First RF Mode | Proposed |
| [ADR-034](ADR-034-expo-mobile-app.md) | Expo React Native Mobile App | Accepted |
| [ADR-035](ADR-035-live-sensing-ui-accuracy.md) | Live Sensing UI Accuracy and Data Transparency | Accepted |
| [ADR-036](ADR-036-rvf-training-pipeline-ui.md) | Training Pipeline UI Integration | Proposed |
| [ADR-043](ADR-043-sensing-server-ui-api-completion.md) | Sensing Server UI API Completion (14 endpoints) | Accepted |
| [ADR-344](ADR-344-adaptive-local-installation-discovery.md) | Adaptive Local Installation Discovery | Accepted (local software path) |
| [ADR-346](ADR-346-fail-closed-edge-occupancy-evidence.md) | Fail closed ESP32 occupancy evidence | Accepted (C6 occupancy integrity qualified) |
| [ADR-347](ADR-347-rate-aware-esp32-temporal-sensing.md) | Rate aware ESP32 temporal sensing | Accepted (C6 timing and transport qualified) |
| [ADR-115](ADR-115-home-assistant-integration.md) | Home Assistant integration via MQTT auto-discovery + Matter bridge (HA-DISCO + HA-FABRIC + HA-MIND) | Accepted (MQTT track) / Proposed (Matter SDK P8b) |
| [ADR-169](ADR-169-adam-mode-light-theme.md) | adam-mode — light theme toggle for the three.js realtime demo | Proposed |
| [ADR-170](ADR-170-yoga-mode-pose-system.md) | yoga-mode — yoga pose detection, classification, and scoring for the three.js realtime demo | Proposed |
| [ADR-324](ADR-324-off-axis-head-coupled-perspective-demo.md) | off-axis-mode — RF-assisted head-coupled perspective demo (clean-room Kooima projection; RF presence gating) | Proposed |

### Architecture and infrastructure

| ADR | Title | Status |
|-----|-------|--------|
| [ADR-001](ADR-001-wifi-mat-disaster-detection.md) | WiFi-Mat Disaster Detection Architecture | Accepted |
| [ADR-002](ADR-002-ruvector-rvf-integration-strategy.md) | RuVector RVF Integration Strategy | Superseded |
| [ADR-003](ADR-003-rvf-cognitive-containers-csi.md) | RVF Cognitive Containers for CSI | Proposed |
| [ADR-004](ADR-004-hnsw-vector-search-fingerprinting.md) | HNSW Vector Search for Fingerprinting | Partial |
| [ADR-007](ADR-007-post-quantum-cryptography-secure-sensing.md) | Post-Quantum Cryptography for Sensing | Proposed |
| [ADR-008](ADR-008-distributed-consensus-multi-ap.md) | Distributed Consensus for Multi-AP | Proposed |
| [ADR-009](ADR-009-rvf-wasm-runtime-edge-deployment.md) | RVF WASM Runtime for Edge Deployment | Proposed |
| [ADR-010](ADR-010-witness-chains-audit-trail-integrity.md) | Witness Chains for Audit Trail Integrity | Proposed |
| [ADR-011](ADR-011-python-proof-of-reality-mock-elimination.md) | Proof-of-Reality and Mock Elimination | Proposed |
| [ADR-026](ADR-026-survivor-track-lifecycle.md) | Survivor Track Lifecycle (MAT crate) | Accepted |
| [ADR-038](ADR-038-sublinear-goal-oriented-action-planning.md) | Sublinear GOAP for Roadmap Optimization | Proposed |
| [ADR-095](ADR-095-rvcsi-edge-rf-sensing-platform.md) | rvCSI — Edge RF Sensing Runtime Platform | Proposed |
| [ADR-096](ADR-096-rvcsi-ffi-crate-layout.md) | rvCSI — Crate Topology, the napi-c Shim, and the napi-rs Node Surface | Proposed |
| [ADR-097](ADR-097-adopt-rvcsi-as-ruview-csi-runtime.md) | Adopt rvCSI as RuView's primary CSI runtime (phased adoption) | Proposed |
| [ADR-098](ADR-098-evaluate-midstream-fit.md) | Evaluate `ruvnet/midstream` for RuView's CSI / WebSocket / mesh pipeline | Rejected |
| [ADR-099](ADR-099-midstream-introspection-tap.md) | Adopt midstream as RuView's real-time introspection + low-latency tap | Proposed |
| [ADR-263](ADR-263-ruview-npm-harness-deep-review.md) | `@ruvnet/ruview` npm harness — deep review + optimization strategy | Proposed |
| [ADR-264](ADR-264-rvagent-mcp-and-cli-npm-deep-review.md) | `@ruvnet/rvagent` MCP server + `@ruv/ruview-cli` — deep review + optimization strategy | Proposed |
| [ADR-265](ADR-265-ruview-npm-distribution-strategy.md) | RuView npm distribution strategy — CI gate, provenance, version single-sourcing, namespace | Proposed |
| [ADR-273](ADR-273-unified-rf-spatial-world-model.md) | Unified RF spatial world model — umbrella, anti-leakage protocol, acceptance gates | Accepted (P1 implemented) |
| [ADR-274](ADR-274-universal-rf-encoder-adapter-registry.md) | Universal RF foundation encoder + hardware adapter registry | Accepted (P1 implemented) |
| [ADR-275](ADR-275-rf-aware-gaussian-spatial-memory.md) | RF-aware Gaussian spatial memory | Accepted (P1 implemented) |
| [ADR-276](ADR-276-physics-guided-synthetic-rf-worlds.md) | Physics-guided synthetic RF world generator | Accepted (P1 implemented) |
| [ADR-277](ADR-277-edge-sensing-control-plane.md) | Edge sensing control plane (802.11bf / ETSI ISAC aligned) | Accepted (P1 implemented) |
| [ADR-278](ADR-278-radar-inverse-rendering-research-program.md) | Radar inverse rendering + differentiable RF SLAM research program | Proposed |
| [ADR-279](ADR-279-native-rf-frame-contract.md) | Native RF frame contract — `RfFrameV2` authoritative, canonical tensor derived | Accepted (implemented) |
| [ADR-280](ADR-280-active-sensing-programmable-perception.md) | Active sensing & programmable perception control plane | Accepted (implemented) |
| [ADR-281](ADR-281-ble-cs-delay-doppler-pose-factorization.md) | BLE Channel Sounding, delay-Doppler tensors, P3162 import, factorized pose | Accepted (implemented) |
| [ADR-282](ADR-282-ruview-ecosystem-positioning.md) | Ecosystem positioning + mandatory L0–L5 evidence ladder | Accepted |
| [ADR-287](ADR-287-coherent-wideband-rf-tomography-crate.md) | `wifi-densepose-sar` — coherent wideband RF tomography research crate | Accepted (implemented, published) |
| [ADR-285](ADR-285-homecore-wasm-first-metaharness.md) | WASM-first Homecore developer metaharness via `npx homecore` | Accepted (implemented and validated) |
| [ADR-286](ADR-286-wifi-densepose-sar-harness-via-metaharness.md) | `wifi-densepose-sar-harness` — MetaHarness with darwin/router/flywheel | Accepted (implemented, published) |
| [ADR-288](ADR-288-veil-privacy-shield-compliant-waveform.md) | VEIL — compliant-waveform privacy shield against unauthorized WiFi sensing (`wifi-densepose-privshield`) | Proposed (implemented, P1 reference) |
| [ADR-289](ADR-289-wifi-densepose-privshield-harness-via-metaharness.md) | `wifi-densepose-privshield-harness` — npm MetaHarness for the VEIL crate (guidance/router/flywheel) | Proposed (implemented, P1) |
| [ADR-290](ADR-290-veil-e2e-hardware-implementation-program.md) | VEIL end-to-end hardware implementation program — portable C core + multi-provider firmware scaffolds (openwifi/openwrt/nexmon/esp32) | Proposed (P4 scaffolding; C core host-validated) |
| [ADR-291](ADR-291-public-benchmark-evaluation-harness.md) | Public-benchmark evaluation harness — Widar3.0 ingest, split protocols, leakage guards | Accepted (initial implementation) |
| [ADR-292](ADR-292-wideband-80211ax-csi-ingest.md) | Wideband 802.11ax CSI ingest — FeitCSI/AX210 adapter, subcarrier-agnostic plumbing | Accepted (initial implementation) |
| [ADR-293](ADR-293-vitals-ground-truth-rig.md) | Vitals ground-truth rig — reference ingest, alignment, agreement metrics | Accepted (initial implementation) |
| [ADR-294](ADR-294-wifi-veil-integration.md) | WiFi Veil integration — emission-shaping countermeasure as advisory BFLD dependency | Accepted (initial implementation) |
| [ADR-295](ADR-295-source-provenance-state-machine.md) | Source provenance state machine — synthetic can never present as live | Accepted (initial implementation) |
| [ADR-296](ADR-296-sensor-data-plane-bind-hardening.md) | Sensor data-plane hardening — UDP bind control and source allowlist (step one) | Accepted (initial implementation) |
| [ADR-297](ADR-297-multi-node-semantic-correctness.md) | Multi-node semantic correctness — per-node inference, node-keyed rate limiting, stale state | Accepted (initial implementation) |
| [ADR-298](ADR-298-model-release-sanity-gates.md) | Model release sanity gates — block degenerate and mislabeled model artifacts | Accepted (initial implementation) |
| [ADR-299](ADR-299-csi-data-incident-repo-controls.md) | Repository CSI data-incident controls — ignore rules and pre-commit/CI policy check | Accepted (controls implemented; tree remediation gated) |
| [ADR-300](ADR-300-perception-substrate-program.md) | RuView perception substrate — phased 21-primitive program (calibration, evidence, trust, deployment) | Accepted (program; children ADR-301..317) |
| [ADR-301](ADR-301-automatic-domain-calibration.md) | Automatic domain calibration — signed, versioned, invalidatable room fingerprint | Accepted (phase 1) |
| [ADR-302](ADR-302-out-of-distribution-detection.md) | Out-of-distribution detection — KNOWN / DEGRADED / UNKNOWN gating | Accepted (phase 1) |
| [ADR-303](ADR-303-ground-truth-synchronization.md) | Ground-truth synchronization — reference sensors as a formal validation plane | Proposed (phase 2) |
| [ADR-304](ADR-304-evidence-engine.md) | Evidence engine — per-(room,device,subject) accuracy ledger | Accepted (phase 1) |
| [ADR-305](ADR-305-authenticated-sensor-identity.md) | Authenticated sensor identity — RF chain of custody | Accepted (phase 1) |
| [ADR-306](ADR-306-canonical-spatial-ontology.md) | Canonical spatial ontology — one Site→…→Event model for every surface | Accepted (phase 1) |
| [ADR-307](ADR-307-persistent-identity-tracking.md) | Persistent identity & tracking — privacy-preserving probabilistic tracks | Proposed (phase 2) |
| [ADR-308](ADR-308-sensor-placement-optimizer.md) | Sensor placement optimizer — floorplan + inventory → recommended positions | Proposed (phase 3) |
| [ADR-309](ADR-309-active-sensing.md) | Active sensing — closed-loop RF experiment control | Proposed (phase 3) |
| [ADR-310](ADR-310-80211bf-native-architecture.md) | 802.11bf-native architecture — standardized WLAN sensing as native measurement types | Proposed (phase 2) |
| [ADR-311](ADR-311-real-sensor-fusion.md) | Real sensor fusion — uncertainty-aware, multiple observations → one world state | Proposed (phase 2) |
| [ADR-312](ADR-312-long-term-spatial-memory.md) | Long-term spatial memory — learn the normal physics of a location | Proposed (phase 3) |
| [ADR-313](ADR-313-counterfactual-inference.md) | Counterfactual inference — generative spatial reasoning | Proposed (phase 3) |
| [ADR-314](ADR-314-information-gain-scheduler.md) | Information-gain scheduler — sample the most informative radios | Proposed (phase 3) |
| [ADR-315](ADR-315-digital-rf-twin.md) | Digital RF twin — persistent per-deployment RF model | Proposed (phase 3) |
| [ADR-316](ADR-316-fleet-control-plane.md) | Fleet control plane — provisioning to audit trails | Proposed (phase 2) |
| [ADR-317](ADR-317-benchmark-multi-domain-scorecard.md) | Multi-domain benchmark scorecard — regressions cannot hide behind pooled accuracy | Accepted (phase 1) |
| [ADR-318](ADR-318-capability-certificates.md) | Capability certificates — validated-for-this-environment claims | Accepted (phase 1) |
| [ADR-319](ADR-319-witness-chain.md) | Witness chain — staged, signed epistemic envelope | Accepted (phase 1) |
| [ADR-320](ADR-320-sensor-hal.md) | RuView sensor HAL — abstract all sensing hardware to one Observation type | Proposed (phase 2) |
| [ADR-321](ADR-321-decision-policy-action-authorization.md) | Decision policy — action authorization conditioned on certificate class, freshness, uncertainty, evidence | Accepted (phase 1) |
| [ADR-322](ADR-322-esp32-micro-llm-research-spike.md) | ESP32-S3 micro-LLM inference — bounded research spike, production non-goal | Proposed |
| [ADR-323](ADR-323-native-rust-physics-constrained-pose-refinement.md) | Native Rust physics-constrained pose refinement | Proposed |
| [ADR-328](ADR-328-flash-resident-edge-model-weight-streaming.md) | Flash-resident weight streaming for RuView edge models | Proposed |

---

## Related

- [DDD Domain Models](../ddd/) — Bounded context definitions, aggregate roots, and ubiquitous language
- [User Guide](../user-guide.md) — Setup, API reference, and hardware instructions
- [Build Guide](../build-guide.md) — Building from source
