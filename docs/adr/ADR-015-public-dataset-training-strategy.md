# ADR-015: Public Dataset Strategy for Trained Pose Estimation Model

## Status

Proposed

## Context

The WiFi-DensePose system has a complete model architecture (`DensePoseHead`,
`ModalityTranslationNetwork`, `WiFiDensePoseRCNN`) and signal processing pipeline,
but no trained weights. Without a trained model, pose estimation produces random
outputs regardless of input quality.

Training requires paired data: simultaneous WiFi CSI captures alongside ground-truth
human pose annotations. Collecting this data from scratch requires months of effort
and specialized hardware (multiple WiFi nodes + camera + motion capture rig). Several
public datasets exist that can bootstrap training without custom collection.

### The Teacher-Student Constraint

The CMU "DensePose From WiFi" paper (2023) trains using a teacher-student approach:
a camera-based RGB pose model (e.g. Detectron2 DensePose) generates pseudo-labels
during training, so the WiFi model learns to replicate those outputs. At inference,
the camera is removed. This means any dataset that provides *either* ground-truth
pose annotations *or* synchronized RGB frames (from which a teacher can generate
labels) is sufficient for training.

## Decision

Use MM-Fi as the primary training dataset, supplemented by XRF55 for additional
diversity, with a teacher-student pipeline for any dataset that lacks dense pose
annotations but provides RGB video.

### Primary Dataset: MM-Fi

**Paper:** "MM-Fi: Multi-Modal Non-Intrusive 4D Human Dataset for Versatile Wireless
Sensing" (NeurIPS 2023 Datasets Track)
**Repository:** https://github.com/ybCliff/MM-Fi
**Size:** 40 volunteers × 27 action classes × ~320,000 frames
**Modalities:** WiFi CSI, mmWave radar, LiDAR, RGB-D, IMU
**CSI format:** 3 Tx × 3 Rx antennas, 114 subcarriers, 100 Hz sampling rate,
IEEE 802.11n 5 GHz, raw amplitude + phase
**Pose annotations:** 17-keypoint COCO skeleton (from RGB-D ground truth)
**License:** CC BY-NC 4.0
**Why primary:** Largest public WiFi CSI + pose dataset; raw amplitude and phase
available (not just processed features); antenna count (3×3) is compatible with the
existing `CSIProcessor` configuration; COCO keypoints map directly to the
`KeypointHead` output format.

### Secondary Dataset: XRF55

**Paper:** "XRF55: A Radio-Frequency Dataset for Human Indoor Action Recognition"
(ACM MM 2023)
**Repository:** https://github.com/aiotgroup/XRF55
**Size:** 55 action classes, multiple subjects and environments
**CSI format:** WiFi CSI + UWB radar, 3 Tx × 3 Rx, 30 subcarriers
**Pose annotations:** Skeleton keypoints from Kinect
**License:** Research use
**Why secondary:** Different environments and action vocabulary increase
generalization; 30 subcarriers requires subcarrier interpolation to match the
existing 56-subcarrier config.

### Excluded Datasets and Reasons

| Dataset | Reason for exclusion |
|---------|---------------------|
| RF-Pose / RF-Pose3D (MIT) | Uses 60 GHz mmWave, not 2.4/5 GHz WiFi CSI; incompatible signal physics |
| Person-in-WiFi (CMU 2019) | Amplitude only, no phase; not publicly released |
| Widar 3.0 | Gesture recognition only, no full-body pose |
| NTU-Fi | Activity labels only, no pose keypoints |
| WiPose | Limited release; superseded by MM-Fi |

## Implementation Plan

### Phase 1: MM-Fi Loader

Implement a `PyTorch Dataset` class that:
- Reads MM-Fi's HDF5/numpy CSI files
- Resamples from 114 subcarriers → 56 subcarriers (linear interpolation along
  frequency axis) to match the existing `CSIProcessor` config
- Normalizes amplitude and unwraps phase using the existing `PhaseSanitizer`
- Returns `(amplitude, phase, keypoints_17)` tuples

### Phase 2: Teacher-Student Labels

For samples where only skeleton keypoints are available (not full DensePose UV maps):
- Run Detectron2 DensePose on the paired RGB frames to generate `(part_labels,
  u_coords, v_coords)` pseudo-labels
- Cache generated labels to avoid recomputation during training epochs
- This matches the training procedure in the original CMU paper

### Phase 3: Training Pipeline

- **Loss:** Combined keypoint heatmap loss (MSE) + DensePose part classification
  (cross-entropy) + UV regression (Smooth L1) + transfer loss against teacher
  RGB backbone features
- **Optimizer:** Adam, lr=1e-3, milestones at 48k and 96k steps (paper schedule)
- **Hardware:** Single GPU (RTX 3090 or A100); MM-Fi fits in ~50 GB disk
- **Checkpointing:** Save every epoch; keep best-by-validation-PCK

### Phase 4: Evaluation

- **Keypoints:** PCK@0.2 (Percentage of Correct Keypoints within 20% of torso size)
- **DensePose:** GPS (Geodesic Point Similarity) and GPSM with segmentation mask
- **Held-out split:** MM-Fi subjects 33-40 (20%) for validation; no test-set leakage

## Subcarrier Mismatch: MM-Fi (114) vs System (56)

MM-Fi captures 114 subcarriers at 5 GHz with 40 MHz bandwidth. The existing system
is configured for 56 subcarriers. Resolution options in order of preference:

1. **Interpolate MM-Fi → 56** (recommended for initial training): linear interpolation
   preserves spectral envelope, fast, no architecture change needed
2. **Reconfigure system → 114**: change `CSIProcessor` config; requires re-running
   `verify.py --generate-hash` to update proof hash
3. **Train at native 114, serve at 56**: separate train/inference configs; adds
   complexity

Option 1 is chosen for Phase 1 to unblock training immediately.

## Consequences

**Positive:**
- Unblocks end-to-end training without hardware collection
- MM-Fi's 3×3 antenna setup matches this system's target hardware (ESP32 mesh, ADR-012)
- 40 subjects with 27 action classes provides reasonable diversity for a first model
- CC BY-NC license is compatible with research and internal use

**Negative:**
- CC BY-NC prohibits commercial deployment of weights trained solely on MM-Fi;
  custom data collection required before commercial release
- 114→56 subcarrier interpolation loses some frequency resolution; acceptable for
  initial training, revisit in Phase 2
- MM-Fi was captured in controlled lab environments; expect accuracy drop in
  complex real-world deployments until fine-tuned on domain-specific data

## References

- He et al., "MM-Fi: Multi-Modal Non-Intrusive 4D Human Dataset" (NeurIPS 2023)
- Yang et al., "DensePose From WiFi" (arXiv 2301.00250, CMU 2023)
- ADR-012: ESP32 CSI Sensor Mesh (hardware target)
- ADR-013: Feature-Level Sensing on Commodity Gear
- ADR-014: SOTA Signal Processing Algorithms
