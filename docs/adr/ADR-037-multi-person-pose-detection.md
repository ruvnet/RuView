# ADR-037: Multi-Person Pose Detection from Single ESP32 CSI Stream

- **Status**: Proposed
- **Date**: 2026-03-02
- **Issue**: [#97](https://github.com/ruvnet/wifi-densepose/issues/97)
- **Deciders**: @ruvnet
- **Supersedes**: None
- **Related**: ADR-014 (SOTA signal processing), ADR-024 (AETHER re-ID), ADR-029 (multistatic sensing), ADR-036 (RVF training pipeline)

## Context

The current signal-derived pose estimation pipeline (`derive_pose_from_sensing()` in the sensing server) generates at most one skeleton per frame from aggregate CSI features. When multiple people are present, only a single blended skeleton is produced. Live testing with ESP32 hardware confirmed: 2 people in the room yields 1 detected person.

A single ESP32 node provides 1 TX × 1 RX × 56 subcarriers of CSI data per frame. While this is limited spatial resolution compared to camera-based systems, the signal contains composite reflections from all scatterers in the environment. The challenge is decomposing these composite signals into per-person contributions.

## Decision

Implement multi-person pose detection in four phases, progressively improving accuracy from heuristic to neural approaches.

### Phase 1: Person Count Estimation

Estimate occupancy count from CSI signal statistics without decomposition.

**Approach**: Eigenvalue analysis of the CSI covariance matrix across subcarriers.

- Compute the 56×56 covariance matrix of CSI amplitudes over a sliding window (e.g., 50 frames / 5 seconds)
- Count eigenvalues above a noise threshold — each significant eigenvalue corresponds to an independent scatterer (person or static object)
- Subtract the static environment baseline (estimated during calibration or from the field model's SVD eigenstructure)
- The residual significant eigenvalue count estimates person count

**Accuracy target**: > 80% for 0-3 people with single ESP32 node.

**Integration point**: `signal/src/ruvsense/field_model.rs` already computes SVD eigenstructure. Extend with a `estimate_occupancy()` method.

### Phase 2: Signal Decomposition

Separate per-person signal contributions using blind source separation.

**Approach**: Non-negative Matrix Factorization (NMF) on the CSI spectrogram.

- Construct a time-frequency matrix from CSI amplitudes: rows = subcarriers (56), columns = time frames
- Apply NMF with k components (k = estimated person count from Phase 1)
- Each component's frequency profile maps to a person's motion pattern
- NMF is preferred over ICA because CSI amplitudes are non-negative

**Alternative**: Independent Component Analysis (ICA) on complex CSI (amplitude + phase). More powerful but requires phase calibration (see `ruvsense/phase_align.rs`).

**Integration point**: New module `signal/src/ruvsense/separation.rs`.

### Phase 3: Multi-Skeleton Generation

Generate distinct pose skeletons per decomposed component.

**Approach**: Per-component feature extraction → per-person skeleton synthesis.

- Extract motion features (dominant frequency, energy, spectral centroid) per NMF component
- Map each component to a spatial position using subcarrier phase gradient (Fresnel zone model)
- Generate 17-keypoint COCO skeleton per person with position offset
- Assign person IDs using the existing Kalman tracker (`ruvsense/pose_tracker.rs`) with AETHER re-ID embeddings (ADR-024)

**Integration point**: Modify `derive_pose_from_sensing()` in `sensing-server/src/main.rs` to return `Vec<Person>` with length > 1.

### Phase 4: Neural Multi-Person Model

Train a dedicated multi-person model using the RVF pipeline (ADR-036).

- Use MM-Fi dataset (ADR-015) multi-person scenarios for training data
- Architecture: shared CSI encoder → person count head + per-person pose heads
- LoRA fine-tuning profile for multi-person specialization
- Inference via the model manager in the sensing server

**Accuracy target**: PCK@0.2 > 60% for 2-person scenarios.

## Consequences

### Positive

- Enables room occupancy counting (Phase 1 alone is useful)
- Distinct pose tracking per person enables activity recognition per individual
- Progressive approach — each phase delivers incremental value
- Reuses existing infrastructure (field model SVD, Kalman tracker, AETHER, RVF pipeline)

### Negative

- Single ESP32 node has fundamental spatial resolution limits — separating 2 people standing close together (< 0.5m) will be unreliable
- NMF decomposition adds ~5-10ms latency per frame
- Person count estimation will have false positives from large moving objects (pets, fans)
- Phase 4 neural model requires multi-person training data collection

### Neutral

- Multi-node multistatic mesh (ADR-029) dramatically improves multi-person separation but is a separate effort
- UI already supports multi-person rendering — no frontend changes needed for the `persons[]` array

## Affected Components

| Component | Phase | Change |
|-----------|-------|--------|
| `signal/src/ruvsense/field_model.rs` | 1 | Add `estimate_occupancy()` |
| `signal/src/ruvsense/separation.rs` | 2 | New module: NMF decomposition |
| `sensing-server/src/main.rs` | 3 | `derive_pose_from_sensing()` multi-person output |
| `signal/src/ruvsense/pose_tracker.rs` | 3 | Multi-target tracking |
| `nn/` | 4 | Multi-person inference head |
| `train/` | 4 | Multi-person training pipeline |

## Performance Budget

| Operation | Budget | Phase |
|-----------|--------|-------|
| Person count estimation | < 2ms | 1 |
| NMF decomposition (k=3) | < 10ms | 2 |
| Multi-skeleton synthesis | < 3ms | 3 |
| Neural inference (multi-person) | < 50ms | 4 |
| **Total pipeline** | **< 65ms** (15 FPS) | All |

## Alternatives Considered

1. **Camera fusion**: Use a camera for person detection and WiFi for pose — rejected because the project goal is camera-free sensing.
2. **Multiple single-person models**: Run N independent pose estimators — rejected because they would produce correlated outputs from the same CSI data.
3. **Spatial filtering (beamforming)**: Use antenna array beamforming to isolate directions — rejected because single ESP32 has only 1 antenna; viable with multistatic mesh (ADR-029).
4. **Skip signal-derived, go straight to neural**: Train an end-to-end multi-person model — rejected because signal-derived provides faster iteration and interpretability for the early phases.
