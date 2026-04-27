# ADR-079: Camera Ground-Truth Training Pipeline

- **Status**: Accepted
- **Date**: 2026-04-06
- **Deciders**: ruv
- **Relates to**: ADR-072 (WiFlow Architecture), ADR-070 (Self-Supervised Pretraining), ADR-071 (ruvllm Training Pipeline), ADR-024 (AETHER Contrastive), ADR-064 (Multimodal Ambient Intelligence), ADR-075 (MinCut Person Separation)

## Context

WiFlow (ADR-072) currently trains without ground-truth pose labels, using proxy poses
generated from presence/motion heuristics. This produces a PCK@20 of only 2.5% — far
below the 30-50% achievable with supervised training. The fundamental bottleneck is the
absence of spatial keypoint labels.

Academic WiFi pose estimation systems (Wi-Pose, Person-in-WiFi 3D, MetaFi++) all train
with synchronized camera ground truth and achieve PCK@20 of 40-85%. They discard the
camera at deployment — the camera is a training-time teacher, not a runtime dependency.

ADR-064 already identified this: *"Record CSI + mmWave while performing signs with a
camera as ground truth, then deploy camera-free."* This ADR specifies the implementation.

### Current Training Pipeline Gap

```
Current:  CSI amplitude → WiFlow → 17 keypoints (proxy-supervised, PCK@20 = 2.5%)
                                    ↑
                            Heuristic proxies:
                            - Standing skeleton when presence > 0.3
                            - Limb perturbation from motion energy
                            - No spatial accuracy
```

### Target Pipeline

```
Training: CSI amplitude ──→ WiFlow ──→ 17 keypoints (camera-supervised, PCK@20 target: 35%+)
                                        ↑
          Laptop camera ──→ MediaPipe ──→ 17 COCO keypoints (ground truth)
                                        (time-synchronized, 30 fps)

Deploy:   CSI amplitude ──→ WiFlow ──→ 17 keypoints (camera-free, trained model only)
```

## Decision

Build a camera ground-truth collection and training pipeline using the laptop webcam
as a teacher signal. The camera is used **only during training data collection** and is
not required at deployment.

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    Data Collection Phase                         │
│                                                                 │
│  ESP32-S3 nodes ──UDP──→ Sensing Server ──→ CSI frames (.jsonl) │
│                              ↑ time sync                        │
│  Laptop Camera ──→ MediaPipe Pose ──→ Keypoints (.jsonl)        │
│                              ↑                                  │
│                     collect-ground-truth.py                      │
│                     (single orchestrator)                        │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    Training Phase                                │
│                                                                 │
│  Paired dataset: { csi_window[128,20], keypoints[17,2], conf }  │
│         ↓                                                       │
│  train-wiflow-supervised.js                                     │
│    Phase 1: Contrastive pretrain (ADR-072, reuse)               │
│    Phase 2: Supervised keypoint regression (NEW)                │
│    Phase 3: Fine-tune with bone constraints + confidence        │
│         ↓                                                       │
│  WiFlow model (1.8M params) → SafeTensors export                │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    Deployment (camera-free)                      │
│                                                                 │
│  ESP32-S3 CSI → Sensing Server → WiFlow inference → 17 keypoints│
│  (No camera. Trained model runs on CSI input only.)             │
└─────────────────────────────────────────────────────────────────┘
```

### Component 1: `scripts/collect-ground-truth.py`

Single Python script that orchestrates synchronized capture from the laptop camera
and the ESP32 CSI stream.

**Dependencies:** `mediapipe`, `opencv-python`, `requests` (all pip-installable, no GPU)

**Capture flow:**

```python
# Pseudocode
camera = cv2.VideoCapture(0)           # Laptop webcam
sensing_api = "http://localhost:3000"   # Sensing server

# Start CSI recording via existing API
requests.post(f"{sensing_api}/api/v1/recording/start")

while recording:
    frame = camera.read()
    t = time.time_ns()                  # Nanosecond timestamp

    # MediaPipe Pose: 33 landmarks → map to 17 COCO keypoints
    result = mp_pose.process(frame)
    keypoints_17 = map_mediapipe_to_coco(result.pose_landmarks)
    confidence = mean(landmark.visibility for relevant landmarks)

    # Write to ground-truth JSONL (one line per frame)
    write_jsonl({
        "ts_ns": t,
        "keypoints": keypoints_17,      # [[x,y], ...] normalized [0,1]
        "confidence": confidence,        # 0-1, used for loss weighting
        "n_visible": count(visibility > 0.5),
    })

    # Optional: show live preview with skeleton overlay
    if preview:
        draw_skeleton(frame, keypoints_17)
        cv2.imshow("Ground Truth", frame)

# Stop CSI recording
requests.post(f"{sensing_api}/api/v1/recording/stop")
```

**MediaPipe → COCO keypoint mapping:**

| COCO Index | Joint | MediaPipe Index |
|------------|-------|-----------------|
| 0 | Nose | 0 |
| 1 | Left Eye | 2 |
| 2 | Right Eye | 5 |
| 3 | Left Ear | 7 |
| 4 | Right Ear | 8 |
| 5 | Left Shoulder | 11 |
| 6 | Right Shoulder | 12 |
| 7 | Left Elbow | 13 |
| 8 | Right Elbow | 14 |
| 9 | Left Wrist | 15 |
| 10 | Right Wrist | 16 |
| 11 | Left Hip | 23 |
| 12 | Right Hip | 24 |
| 13 | Left Knee | 25 |
| 14 | Right Knee | 26 |
| 15 | Left Ankle | 27 |
| 16 | Right Ankle | 28 |

### Component 2: Time Alignment (`scripts/align-ground-truth.js`)

CSI frames arrive at ~100 Hz with server-side timestamps. Camera keypoints arrive at
~30 fps with client-side timestamps. Alignment is needed because:

1. Camera and sensing server clocks differ (typically < 50ms on LAN)
2. CSI is aggregated into 20-frame windows for WiFlow input
3. Ground-truth keypoints must be averaged over the same window

**Alignment algorithm:**

```
For each CSI window W_i (20 frames, ~200ms at 100Hz):
  t_start = W_i.first_frame.timestamp
  t_end   = W_i.last_frame.timestamp

  # Find all camera keypoints within this time window
  matching_keypoints = [k for k in camera_data if t_start <= k.ts <= t_end]

  if len(matching_keypoints) >= 3:   # At least 3 camera frames per window
    # Average keypoints, weighted by confidence
    avg_keypoints = weighted_mean(matching_keypoints, weights=confidences)
    avg_confidence = mean(confidences)

    paired_dataset.append({
      csi_window: W_i.amplitudes,    # [128, 20] float32
      keypoints: avg_keypoints,       # [17, 2] float32
      confidence: avg_confidence,     # scalar
      n_camera_frames: len(matching_keypoints),
    })
```

**Clock sync strategy:**

- NTP is sufficient (< 20ms error on LAN)
- The 200ms CSI window is 10x larger than typical clock drift
- For tighter sync: use a handclap/jump as a sync marker — visible spike in both
  CSI motion energy and camera skeleton velocity. Auto-detect and align.

**Output:** `data/recordings/paired-{timestamp}.jsonl` — one line per paired sample:
```json
{"csi": [128x20 flat], "kp": [[0.45,0.12], ...], "conf": 0.92, "ts": 1775300000000}
```

### Component 3: Supervised Training (`scripts/train-wiflow-supervised.js`)

Extends the existing `train-ruvllm.js` pipeline with a supervised phase.

**Phase 1: Contrastive Pretrain (reuse ADR-072)**
- Same as existing: temporal + cross-node triplets
- Learns CSI representation without labels
- 50 epochs, ~5 min on laptop

**Phase 2: Supervised Keypoint Regression (NEW)**
- Load paired dataset from Component 2
- Loss: confidence-weighted SmoothL1 on keypoints

```
L_supervised = (1/N) * sum_i [ conf_i * SmoothL1(pred_i, gt_i, beta=0.05) ]
```

- Only train on samples where `conf > 0.5` (discard frames where MediaPipe lost tracking)
- Learning rate: 1e-4 with cosine decay
- 200 epochs, ~15 min on laptop CPU (1.8M params, no GPU needed)

**Phase 3: Refinement with Bone Constraints**
- Fine-tune with combined loss:

```
L = L_supervised + 0.3 * L_bone + 0.1 * L_temporal

L_bone     = (1/14) * sum_b (bone_len_b - prior_b)^2   # ADR-072 bone priors
L_temporal = SmoothL1(kp_t, kp_{t-1})                   # Temporal smoothness
```

- 50 epochs at lower LR (1e-5)
- Tighten bone constraint weight from 0.3 → 0.5 over epochs

**Phase 4: Quantization + Export**
- Reuse ruvllm TurboQuant: float32 → int8 (4x smaller, ~881 KB)
- Export via SafeTensors for cross-platform deployment
- Validate quantized model PCK@20 within 2% of full-precision

### Component 4: Evaluation Script (`scripts/eval-wiflow.js`)

Measure actual PCK@20 using held-out paired data (20% split).

```
PCK@k = (1/N) * sum_i [ (||pred_i - gt_i|| < k * torso_length) ? 1 : 0 ]
```

**Metrics reported:**

| Metric | Description | Target |
|--------|-------------|--------|
| PCK@20 | % of keypoints within 20% torso length | > 35% |
| PCK@50 | % within 50% torso length | > 60% |
| MPJPE | Mean per-joint position error (pixels) | < 40px |
| Per-joint PCK | Breakdown by joint (wrists are hardest) | Report all 17 |
| Inference latency | Single window prediction time | < 50ms |

### Optimization Strategy

#### O1: Curriculum Learning

Train easy poses first, hard poses later:

| Stage | Epochs | Data Filter | Rationale |
|-------|--------|-------------|-----------|
| 1 | 50 | `conf > 0.9`, standing only | Establish stable skeleton baseline |
| 2 | 50 | `conf > 0.7`, low motion | Add sitting, subtle movements |
| 3 | 50 | `conf > 0.5`, all poses | Full dataset including occlusions |
| 4 | 50 | All data, with augmentation | Robustness via noise injection |

#### O2: Data Augmentation (CSI domain)

Augment CSI windows to increase effective dataset size without collecting more data:

| Augmentation | Implementation | Expected Gain |
|-------------|----------------|---------------|
| Time shift | Roll CSI window by ±2 frames | +30% data |
| Amplitude noise | Gaussian noise, sigma=0.02 | Robustness |
| Subcarrier dropout | Zero 10% of subcarriers randomly | Robustness |
| Temporal flip | Reverse window + reverse keypoint velocity | +100% data |
| Multi-node mix | Swap node CSI, keep same-time keypoints | Cross-node generalization |

#### O3: Knowledge Distillation from MediaPipe

Instead of raw keypoint regression, distill MediaPipe's confidence and heatmap
information:

```
L_distill = KL_div(softmax(wifi_heatmap / T), softmax(camera_heatmap / T))
```

- Temperature T=4 for soft targets (transfers inter-joint relationships)
- WiFlow predicts a 17-channel heatmap [17, H, W] instead of direct [17, 2]
- Argmax for final keypoint extraction
- **Trade-off:** Adds ~200K params for heatmap decoder, but improves spatial precision

#### O4: Active Learning Loop

Identify which poses the model is worst at and collect more data for those:

```
1. Train initial model on first collection session
2. Run inference on new CSI data, compute prediction entropy
3. Flag high-entropy windows (model is uncertain)
4. During next collection, the preview overlay highlights these moments:
   "Hold this pose — model needs more examples"
5. Re-train with augmented dataset
```

Expected: 2-3 active learning iterations reach saturation.

#### O6: Subcarrier Selection (ruvector-solver)

Variance-based top-K subcarrier selection, equivalent to ruvector-solver's sparse
interpolation (114→56). Removes noise/static subcarriers before training:

```
For each subcarrier d in [0, dim):
  variance[d] = mean over samples of temporal_variance(csi[d, :])
Select top-K by variance (K = dim * 0.5)
```

**Validated:** 128 → 56 subcarriers (56% input reduction), proportional model size reduction.

#### O7: Attention-Weighted Subcarriers (ruvector-attention)

Compute per-subcarrier attention weights based on temporal energy correlation with
ground-truth keypoint motion. High-energy subcarriers that covary with skeleton
movement get amplified:

```
For each subcarrier d:
  energy[d] = sum of squared first-differences over time
  weight[d] = softmax(energy, temperature=0.1)
Apply: csi[d, :] *= weight[d] * dim  (mean weight = 1)
```

**Validated:** Top-5 attention subcarriers identified automatically per dataset.

#### O8: Stoer-Wagner MinCut Person Separation (ruvector-mincut / ADR-075)

JS implementation of the Stoer-Wagner algorithm for person separation in CSI, equivalent
to `DynamicPersonMatcher` in `wifi-densepose-train/src/metrics.rs`. Builds a subcarrier
correlation graph and finds the minimum cut to identify person-specific subcarrier clusters:

```
1. Build dim×dim Pearson correlation matrix across subcarriers
2. Run Stoer-Wagner min-cut on correlation graph
3. Partition subcarriers into person-specific groups
4. Train per-partition models for multi-person scenarios
```

**Validated:** Stoer-Wagner executes on 56-dim graph, identifies partition boundaries.

#### O9: Multi-SPSA Gradient Estimation

Average over K=3 random perturbation directions per gradient step. Reduces variance
by sqrt(K) = 1.73x compared to single SPSA, at 3x forward pass cost (net win for
convergence quality):

```
For k in 1..K:
  delta_k = random ±1 per parameter
  grad_k = (loss(w + eps*delta_k) - loss(w - eps*delta_k)) / (2*eps*delta_k)
grad = mean(grad_1, ..., grad_K)
```

#### O10: Mac M4 Pro Training via Tailscale

Training runs on Mac Mini M4 Pro (16-core GPU, ARM NEON SIMD) via Tailscale SSH,
using ruvllm's native Node.js SIMD ops:

| | Windows (CPU) | Mac M4 Pro |
|---|---|---|
| Node.js | v24.12.0 (x86) | v25.9.0 (ARM) |
| SIMD | SSE4/AVX2 | NEON |
| Cores | Consumer laptop | 12P + 4E cores |
| Training | Slow (minutes/epoch) | Fast (seconds/epoch) |

#### O5: Cross-Environment Transfer

Train on one room, deploy in another:

| Strategy | Implementation |
|----------|---------------|
| Room-invariant features | Normalize CSI by running mean/variance |
| LoRA adapters | Train a 4-rank LoRA per room (ADR-071) — 7.3 KB each |
| Few-shot calibration | 2 min of camera data in new room → fine-tune LoRA only |
| AETHER embeddings | Use contrastive room-independent features (ADR-024) as input |

The LoRA approach is most practical: ship a base model + collect 2 min of calibration
data per new room using the laptop camera.

### Data Collection Protocol

Recommended collection sessions per room:

| Session | Duration | Activity | People | Total CSI Frames |
|---------|----------|----------|--------|-----------------|
| 1. Baseline | 5 min | Empty + 1 person entry/exit | 0-1 | 30,000 |
| 2. Standing poses | 5 min | Stand, arms up/down/sides, turn | 1 | 30,000 |
| 3. Sitting | 5 min | Sit, type, lean, stand up/sit down | 1 | 30,000 |
| 4. Walking | 5 min | Walk paths across room | 1 | 30,000 |
| 5. Mixed | 5 min | Varied activities, transitions | 1 | 30,000 |
| 6. Multi-person | 5 min | 2 people, varied activities | 2 | 30,000 |
| **Total** | **30 min** | | | **180,000** |

At 20-frame windows: **9,000 paired training samples** per 30-min session.
With augmentation (O2): **~27,000 effective samples**.

Camera placement: position laptop so the camera has a clear view of the sensing area.
The camera FOV should cover the same space the ESP32 nodes cover.

### File Structure

```
scripts/
  collect-ground-truth.py     # Camera capture + MediaPipe + CSI sync
  align-ground-truth.js       # Time-align CSI windows with camera keypoints
  train-wiflow-supervised.js  # Supervised training pipeline
  eval-wiflow.js              # PCK evaluation on held-out data

data/
  ground-truth/               # Raw camera keypoint captures
    gt-{timestamp}.jsonl
  paired/                     # Aligned CSI + keypoint pairs
    paired-{timestamp}.jsonl

models/
  wiflow-supervised/          # Trained model outputs
    wiflow-v1.safetensors
    wiflow-v1-int8.safetensors
    training-log.json
    eval-report.json
```

### Privacy Considerations

- Camera frames are processed **locally** by MediaPipe — no cloud upload
- Raw video is **never saved** — only extracted keypoint coordinates are stored
- The `.jsonl` ground-truth files contain only `[x,y]` joint coordinates, not images
- The trained model runs on CSI only — no camera data leaves the laptop
- Users can delete `data/ground-truth/` after training; the model is self-contained

## Consequences

### Positive

- **10-20x accuracy improvement**: PCK@20 from 2.5% → 35%+ with real supervision
- **Reuses existing infrastructure**: sensing server recording API, ruvllm training, SafeTensors
- **No new hardware**: laptop webcam + existing ESP32 nodes
- **Privacy preserved at deployment**: camera only needed during 30-min training session
- **Incremental**: can improve with more collection sessions + active learning
- **Distributable**: trained model weights can be shared on HuggingFace (ADR-070)

### Negative

- **Camera placement matters**: must see the same area ESP32 nodes sense
- **Single-room models**: need LoRA calibration per room (2 min + camera)
- **MediaPipe limitations**: occlusion, side views, multiple people reduce keypoint quality
- **Time sync**: NTP drift can misalign frames (mitigated by 200ms windows)

### Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| MediaPipe keypoints too noisy | Low | Medium | Filter by confidence; MediaPipe is robust indoors |
| Clock drift > 100ms | Low | High | Add handclap sync marker detection |
| Single camera can't see all poses | Medium | Medium | Position camera centrally; collect from 2 angles |
| Model overfits to one room | High | Medium | LoRA adapters + AETHER normalization (O5) |
| Insufficient data (< 5K pairs) | Low | High | Augmentation (O2) + active learning (O4) |

## Implementation Plan

| Phase | Task | Effort | Status |
|-------|------|--------|--------|
| P1 | `collect-ground-truth.py` — camera + MediaPipe capture | 2 hrs | **Done** |
| P2 | `align-ground-truth.js` — time alignment + pairing | 1 hr | **Done** |
| P3 | `train-wiflow-supervised.js` — supervised training | 3 hrs | **Done** |
| P4 | `eval-wiflow.js` — PCK evaluation | 1 hr | **Done** |
| P5 | ruvector optimizations (O6-O9) | 2 hrs | **Done** |
| P6 | Mac M4 Pro training via Tailscale (O10) | 1 hr | **Done** |
| P7 | Data collection session (30 min recording) | 1 hr | Pending |
| P8 | Training + evaluation on real paired data | 30 min | Pending |
| P9 | LoRA cross-room calibration (O5) | 2 hrs | Pending |

## Validated Hardware

| Component | Spec | Validated |
|-----------|------|-----------|
| Mac Mini camera | 1920x1080, 30fps | Yes — 14/17 keypoints, conf 0.94-1.0 |
| MediaPipe PoseLandmarker | v0.10.33 Tasks API, lite model | Yes — via Tailscale SSH |
| Mac M4 Pro GPU | 16-core, Metal 4, NEON SIMD | Yes — Node.js v25.9.0 |
| Tailscale SSH | LAN-accessible Mac, passwordless | Yes |
| ESP32-S3 CSI | 128 subcarriers, 100Hz | Yes — existing recordings |
| Sensing server recording API | `/api/v1/recording/start\|stop` | Yes — existing |

## Baseline Benchmark

Proxy-pose baseline (no camera supervision, standing skeleton heuristic):

```
PCK@10:  11.8%
PCK@20:  35.3%
PCK@50:  94.1%
MPJPE:   0.067
Latency: 0.03ms/sample
```

Per-joint PCK@20: upper body (nose, shoulders, wrists) at 0% — proxy has no spatial
accuracy for these. Camera supervision targets these joints specifically.

## References

- WiFlow: arXiv:2602.08661 — WiFi-based pose estimation with TCN + axial attention
- Wi-Pose (CVPR 2021) — 3D CNN WiFi pose with camera supervision
- Person-in-WiFi 3D (CVPR 2024) — Deformable attention with camera labels
- MediaPipe Pose — Google's real-time 33-landmark body pose estimator
- MetaFi++ (NeurIPS 2023) — Meta-learning cross-modal WiFi sensing
