# Architecture Decision Records

This folder contains 44 Architecture Decision Records (ADRs) that document every significant technical choice in the RuView / WiFi-DensePose project.

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
| [ADR-099](ADR-099-midstream-introspection-tap.md) | Adopt midstream as RuView's real-time introspection + low-latency tap | Proposed |

---

## Related

- [DDD Domain Models](../ddd/) — Bounded context definitions, aggregate roots, and ubiquitous language
- [User Guide](../user-guide.md) — Setup, API reference, and hardware instructions
- [Build Guide](../build-guide.md) — Building from source
