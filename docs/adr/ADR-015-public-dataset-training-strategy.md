# ADR-015: Public Dataset Strategy for Trained Pose Estimation Model

## Status

Accepted

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

### 56-Subcarrier Hardware Context

The system targets 56 subcarriers, which corresponds specifically to **Atheros 802.11n
chipsets on a 20 MHz channel** using the Atheros CSI Tool. No publicly available
dataset with paired pose annotations was collected at exactly 56 subcarriers:

| Hardware | Subcarriers | Datasets |
|----------|-------------|---------|
| Atheros CSI Tool (20 MHz) | **56** | None with pose labels |
| Atheros CSI Tool (40 MHz) | **114** | MM-Fi |
| Intel 5300 NIC (20 MHz) | **30** | Person-in-WiFi, Widar 3.0, Wi-Pose, XRF55 |
| Nexmon/Broadcom (80 MHz) | **242-256** | None with pose labels |

MM-Fi uses the same Atheros hardware family at 40 MHz, making 114→56 interpolation
physically meaningful (same chipset, different channel width).

## Decision

Use MM-Fi as the primary training dataset, supplemented by Wi-Pose (NjtechCVLab)
for additional diversity. XRF55 is downgraded to optional (Kinect labels need
post-processing). Teacher-student pipeline fills in DensePose UV labels where
only skeleton keypoints are available.

### Primary Dataset: MM-Fi

**Paper:** "MM-Fi: Multi-Modal Non-Intrusive 4D Human Dataset for Versatile Wireless
Sensing" (NeurIPS 2023 Datasets & Benchmarks)
**Repository:** https://github.com/ybhbingo/MMFi_dataset
**Size:** 40 subjects × 27 action classes × ~320,000 frames, 4 environments
**Modalities:** WiFi CSI, mmWave radar, LiDAR, RGB-D, IMU
**CSI format:** **1 TX × 3 RX antennas**, 114 subcarriers, 100 Hz sampling rate,
5 GHz 40 MHz (TP-Link N750 with Atheros CSI Tool), raw amplitude + phase
**Data tensor:** [3, 114, 10] per sample (antenna-pairs × subcarriers × time frames)
**Pose annotations:** 17-keypoint COCO skeleton in 3D + DensePose UV surface coords
**License:** CC BY-NC 4.0
**Why primary:** Largest public WiFi CSI + pose dataset; richest annotations (3D
keypoints + DensePose UV); same Atheros hardware family as target system; COCO
keypoints map directly to the `KeypointHead` output format; actively maintained
with NeurIPS 2023 benchmark status.

**Antenna correction:** MM-Fi uses 1 TX / 3 RX (3 antenna pairs), not 3×3.
The existing system targets 3×3 (ESP32 mesh). The 3 RX antennas match; the TX
difference means MM-Fi-trained weights will work but may benefit from fine-tuning
on data from a 3-TX setup.

### Secondary Dataset: Wi-Pose (NjtechCVLab)

**Paper:** CSI-Former (MDPI Entropy 2023) and related works
**Repository:** https://github.com/NjtechCVLab/Wi-PoseDataset
**Size:** 12 volunteers × 12 action classes × 166,600 packets
**CSI format:** 3 TX × 3 RX antennas, 30 subcarriers, 5 GHz, .mat format
**Pose annotations:** 18-keypoint AlphaPose skeleton (COCO-compatible subset)
**License:** Research use
**Why secondary:** 3×3 antenna array matches target ESP32 mesh hardware exactly;
fully public; adds 12 different subjects and environments not in MM-Fi.
**Note:** 30 subcarriers require zero-padding or interpolation to 56; 18→17
keypoint mapping drops one neck keypoint (index 1), compatible with COCO-17.

### Excluded / Deprioritized Datasets

| Dataset | Reason |
|---------|--------|
| RF-Pose / RF-Pose3D (MIT) | Custom FMCW radio, not 802.11n CSI; incompatible signal physics |
| Person-in-WiFi (CMU 2019) | Not publicly released (IRB restriction) |
| Person-in-WiFi 3D (CVPR 2024) | 30 subcarriers, Intel 5300; semi-public access |
| DensePose From WiFi (CMU) | Dataset not released; only paper + architecture |
| Widar 3.0 | Gesture labels only, no full-body pose keypoints |
| XRF55 | Activity labels primarily; Kinect pose requires email request; lower priority |
| UT-HAR, WiAR, SignFi | Activity/gesture labels only, no pose keypoints |

## Implementation Plan

### Phase 1: MM-Fi Loader (Rust `wifi-densepose-train` crate)

Implement `MmFiDataset` in Rust (`crates/wifi-densepose-train/src/dataset.rs`):
- Reads MM-Fi numpy .npy files: amplitude [N, 3, 3, 114] (antenna-pairs laid flat), phase [N, 3, 3, 114]
- Resamples from 114 → 56 subcarriers (linear interpolation via `subcarrier.rs`)
- Applies phase sanitization using SOTA algorithms from `wifi-densepose-signal` crate
- Returns typed `CsiSample` structs with amplitude, phase, keypoints, visibility
- Validation split: subjects 33–40 held out

### Phase 2: Wi-Pose Loader

Implement `WiPoseDataset` reading .mat files (via ndarray-based MATLAB reader or
pre-converted .npy). Subcarrier interpolation: 30 → 56 (zero-pad high frequencies
rather than interpolate, since 30-sub Intel data has different spectral occupancy
than 56-sub Atheros data).

### Phase 3: Teacher-Student DensePose Labels

For MM-Fi samples that provide 3D keypoints but not full DensePose UV maps:
- Run Detectron2 DensePose on paired RGB frames to generate `(part_labels, u_coords, v_coords)`
- Cache generated labels as .npy alongside original data
- This matches the training procedure in the CMU paper exactly

### Phase 4: Training Pipeline (Rust)

- **Model:** `WiFiDensePoseModel` (tch-rs, `crates/wifi-densepose-train/src/model.rs`)
- **Loss:** Keypoint heatmap (MSE) + DensePose part (cross-entropy) + UV (Smooth L1) + transfer (MSE)
- **Metrics:** PCK@0.2 + OKS with Hungarian min-cost assignment (`crates/wifi-densepose-train/src/metrics.rs`)
- **Optimizer:** Adam, lr=1e-3, step decay at epochs 40 and 80
- **Hardware:** Single GPU (RTX 3090 or A100); MM-Fi fits in ~50 GB disk
- **Checkpointing:** Save every epoch; keep best-by-validation-PCK

### Phase 5: Proof Verification

`verify-training` binary provides the "trust kill switch" for training:
- Fixed seed (MODEL_SEED=0, PROOF_SEED=42)
- 50 training steps on deterministic SyntheticDataset
- Verifies: loss decreases + SHA-256 of final weights matches stored hash
- EXIT 0 = PASS, EXIT 1 = FAIL, EXIT 2 = SKIP (no stored hash)

## Subcarrier Mismatch: MM-Fi (114) vs System (56)

MM-Fi captures 114 subcarriers at 5 GHz with 40 MHz bandwidth (Atheros CSI Tool).
The system is configured for 56 subcarriers (Atheros, 20 MHz). Resolution options:

1. **Interpolate MM-Fi → 56** (chosen for Phase 1): linear interpolation preserves
   spectral envelope, fast, no architecture change needed
2. **Train at native 114**: change `CSIProcessor` config; requires re-running
   `verify.py --generate-hash` to update proof hash; future option
3. **Collect native 56-sub data**: ESP32 mesh at 20 MHz; best for production

Option 1 unblocks training immediately. The Rust `subcarrier.rs` module handles
interpolation as a first-class operation with tests proving correctness.

## Consequences

**Positive:**
- Unblocks end-to-end training on real public data immediately
- MM-Fi's Atheros hardware family matches target system (same CSI Tool)
- 40 subjects × 27 actions provides reasonable diversity for first model
- Wi-Pose's 3×3 antenna setup is an exact hardware match for ESP32 mesh
- CC BY-NC license is compatible with research and internal use
- Rust implementation integrates natively with `wifi-densepose-signal` pipeline

**Negative:**
- CC BY-NC prohibits commercial deployment of weights trained solely on MM-Fi;
  custom data collection required before commercial release
- MM-Fi is 1 TX / 3 RX; system targets 3 TX / 3 RX; fine-tuning needed
- 114→56 subcarrier interpolation loses frequency resolution; acceptable for v1
- MM-Fi captured in controlled lab environments; real-world accuracy will be lower
  until fine-tuned on domain-specific data

## References

- Yang et al., "MM-Fi: Multi-Modal Non-Intrusive 4D Human Dataset" (NeurIPS 2023) — arXiv:2305.10345
- Geng et al., "DensePose From WiFi" (CMU, arXiv:2301.00250, 2023)
- Yan et al., "Person-in-WiFi 3D" (CVPR 2024)
- NjtechCVLab, "Wi-Pose Dataset" — github.com/NjtechCVLab/Wi-PoseDataset
- ADR-012: ESP32 CSI Sensor Mesh (hardware target)
- ADR-013: Feature-Level Sensing on Commodity Gear
- ADR-014: SOTA Signal Processing Algorithms
