# ADR-023: Trained DensePose Model with RuVector Signal Intelligence Pipeline

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-02-28 |
| **Deciders** | ruv |
| **Relates to** | ADR-003 (RVF Cognitive Containers), ADR-005 (SONA Self-Learning), ADR-015 (Public Dataset Strategy), ADR-016 (RuVector Integration), ADR-017 (RuVector-Signal-MAT), ADR-020 (Rust AI Migration), ADR-021 (Vital Sign Detection) |

## Context

### The Gap Between Sensing and DensePose

The WiFi-DensePose system currently operates in two distinct modes:

1. **WiFi CSI sensing** (working): ESP32 streams CSI frames → Rust aggregator → feature extraction → presence/motion classification. 41 tests passing, verified at ~20 Hz with real hardware.

2. **Heuristic pose derivation** (working but approximate): The Rust sensing server generates 17 COCO keypoints from WiFi signal properties using hand-crafted rules (`derive_pose_from_sensing()` in `sensing-server/src/main.rs`). This is not a trained model — keypoint positions are derived from signal amplitude, phase variance, and motion metrics rather than learned from labeled data.

Neither mode produces **DensePose-quality** body surface estimation. The CMU "DensePose From WiFi" paper (arXiv:2301.00250) demonstrated that a neural network trained on paired WiFi CSI + camera pose data can produce dense body surface UV coordinates from WiFi alone. However, that approach requires:

- **Environment-specific training**: The model must be trained or fine-tuned for each deployment environment because CSI multipath patterns are environment-dependent.
- **Paired training data**: Simultaneous WiFi CSI captures + ground-truth pose annotations (or a camera-based teacher model generating pseudo-labels).
- **Substantial compute**: Training a modality translation network + DensePose head requires GPU time (hours to days depending on dataset size).

### What Exists in the Codebase

The Rust workspace already has the complete model architecture ready for training:

| Component | Crate | File | Status |
|-----------|-------|------|--------|
| `WiFiDensePoseModel` | `wifi-densepose-train` | `model.rs` | Implemented (random weights) |
| `ModalityTranslator` | `wifi-densepose-train` | `model.rs` | Implemented with RuVector attention |
| `KeypointHead` | `wifi-densepose-train` | `model.rs` | Implemented (17 COCO heatmaps) |
| `DensePoseHead` | `wifi-densepose-nn` | `densepose.rs` | Implemented (25 parts + 48 UV) |
| `WiFiDensePoseLoss` | `wifi-densepose-train` | `losses.rs` | Implemented (keypoint + part + UV + transfer) |
| `MmFiDataset` loader | `wifi-densepose-train` | `dataset.rs` | Planned (ADR-015) |
| `WiFiDensePosePipeline` | `wifi-densepose-nn` | `inference.rs` | Implemented (generic over Backend) |
| Training proof verification | `wifi-densepose-train` | `proof.rs` | Implemented (deterministic hash) |
| Subcarrier resampling (114→56) | `wifi-densepose-train` | `subcarrier.rs` | Planned (ADR-016) |

### RuVector Crates Available

The `vendor/ruvector/` subtree provides 90+ crates. The following are directly relevant to a trained DensePose pipeline:

**Already integrated (5 crates, ADR-016):**

| Crate | Algorithm | Current Use |
|-------|-----------|-------------|
| `ruvector-mincut` | Subpolynomial dynamic min-cut O(n^{o(1)}) | Multi-person assignment in `metrics.rs` |
| `ruvector-attn-mincut` | Attention-gated min-cut | Noise-suppressed spectrogram in `model.rs` |
| `ruvector-attention` | Scaled dot-product + geometric attention | Spatial decoder in `model.rs` |
| `ruvector-solver` | Sparse Neumann solver O(√n) | Subcarrier resampling in `subcarrier.rs` |
| `ruvector-temporal-tensor` | Tiered temporal compression | CSI frame buffering in `dataset.rs` |

**Newly proposed for DensePose pipeline (6 additional crates):**

| Crate | Description | Proposed Use |
|-------|-------------|-------------|
| `ruvector-gnn` | Graph neural network on HNSW topology | Spatial body-graph reasoning |
| `ruvector-graph-transformer` | Proof-gated graph transformer (8 modules) | CSI-to-pose cross-attention |
| `ruvector-sparse-inference` | PowerInfer-style sparse inference engine | Edge deployment with neuron activation sparsity |
| `ruvector-sona` | Self-Optimizing Neural Architecture (LoRA + EWC++) | Online environment adaptation |
| `ruvector-fpga-transformer` | FPGA-optimized transformer | Hardware-accelerated inference path |
| `ruvector-math` | Optimal transport, information geometry | Domain adaptation loss functions |

### RVF Container Format

The RuVector Format (RVF) is a segment-based binary container format designed to package
intelligence artifacts — embeddings, HNSW indexes, quantized weights, WASM runtimes, witness
proofs, and metadata — into a single self-contained file. Key properties:

- **64-byte segment headers** (`SegmentHeader`, magic `0x52564653` "RVFS") with type discriminator, content hash, compression, and timestamp
- **Progressive loading**: Layer A (entry points, <5ms) → Layer B (hot adjacency, 100ms–1s) → Layer C (full graph, seconds)
- **20+ segment types**: `Vec` (embeddings), `Index` (HNSW), `Overlay` (min-cut witnesses), `Quant` (codebooks), `Witness` (proof-of-computation), `Wasm` (self-bootstrapping runtime), `Dashboard` (embedded UI), `AggregateWeights` (federated SONA deltas), `Crypto` (Ed25519 signatures), and more
- **Temperature-tiered quantization** (`rvf-quant`): f32 / f16 / u8 / binary per-segment, with SIMD-accelerated distance computation
- **AGI Cognitive Container** (`agi_container.rs`): packages kernel + WASM + world model + orchestrator + evaluation harness + witness chains into a single deployable file

The trained DensePose model will be packaged as an `.rvf` container, making it a single
self-contained artifact that includes model weights, HNSW-indexed embedding tables, min-cut
graph overlays, quantization codebooks, SONA adaptation deltas, and the WASM inference
runtime — deployable to any host without external dependencies.

## Decision

Implement a fully trained DensePose model using RuVector signal intelligence as the backbone signal processing layer, packaged in the RVF container format. The pipeline has three stages: (1) offline training on public datasets, (2) teacher-student distillation for DensePose UV labels, and (3) online SONA adaptation for environment-specific fine-tuning. The trained model, its embeddings, indexes, and adaptation state are serialized into a single `.rvf` file.

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    TRAINED DENSEPOSE PIPELINE                                │
│                                                                             │
│  ┌─────────────┐    ┌──────────────────────┐    ┌──────────────────────┐   │
│  │ ESP32 CSI    │    │  RuVector Signal      │    │  Trained Neural      │   │
│  │ Raw I/Q      │───▶│  Intelligence Layer   │───▶│  Network             │   │
│  │ [ant×sub×T]  │    │  (preprocessing)      │    │  (inference)         │   │
│  └─────────────┘    └──────────────────────┘    └──────────────────────┘   │
│                              │                           │                   │
│                    ┌─────────┴─────────┐       ┌────────┴────────┐         │
│                    │ 5 RuVector crates  │       │ 6 RuVector      │         │
│                    │ (signal processing)│       │ crates (neural) │         │
│                    └───────────────────┘       └─────────────────┘         │
│                                                        │                    │
│                              ┌──────────────────────────┘                   │
│                              ▼                                              │
│                    ┌──────────────────────────────────────┐                 │
│                    │              Outputs                   │                 │
│                    │  • 17 COCO keypoints [B,17,H,W]       │                 │
│                    │  • 25 body parts     [B,25,H,W]       │                 │
│                    │  • 48 UV coords      [B,48,H,W]       │                 │
│                    │  • Confidence scores                   │                 │
│                    └──────────────────────────────────────┘                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Stage 1: RuVector Signal Preprocessing Layer

Raw CSI frames from ESP32 (56–192 subcarriers × N antennas × T time frames) are processed through the RuVector signal intelligence stack before entering the neural network. This replaces hand-crafted feature extraction with learned, graph-aware preprocessing.

```
Raw CSI [ant, sub, T]
    │
    ▼
┌─────────────────────────────────────────────────────┐
│  1. ruvector-attn-mincut: gate_spectrogram()        │
│     Input:  Q=amplitude, K=phase, V=combined        │
│     Effect: Suppress multipath noise, keep motion-  │
│             relevant subcarrier paths                │
│     Output: Gated spectrogram [ant, sub', T]        │
├─────────────────────────────────────────────────────┤
│  2. ruvector-mincut: mincut_subcarrier_partition()   │
│     Input:  Subcarrier coherence graph               │
│     Effect: Partition into sensitive (motion-         │
│             responsive) vs insensitive (static)      │
│     Output: Partition mask + per-subcarrier weights   │
├─────────────────────────────────────────────────────┤
│  3. ruvector-attention: attention_weighted_bvp()     │
│     Input:  Gated spectrogram + partition weights    │
│     Effect: Compute body velocity profile with       │
│             sensitivity-weighted attention            │
│     Output: BVP feature vector [D_bvp]               │
├─────────────────────────────────────────────────────┤
│  4. ruvector-solver: solve_fresnel_geometry()        │
│     Input:  Amplitude + known TX/RX positions        │
│     Effect: Estimate TX-body-RX ellipsoid distances  │
│     Output: Fresnel geometry features [D_fresnel]    │
├─────────────────────────────────────────────────────┤
│  5. ruvector-temporal-tensor: compress + buffer      │
│     Input:  Temporal CSI window (100 frames)         │
│     Effect: Tiered quantization (hot/warm/cold)      │
│     Output: Compressed tensor, 50-75% memory saving  │
└─────────────────────────────────────────────────────┘
    │
    ▼
Feature tensor [B, T*tx*rx, sub] (preprocessed, noise-suppressed)
```

### Stage 2: Neural Network Architecture

The neural network follows the CMU teacher-student architecture with RuVector enhancements at three critical points.

#### 2a. ModalityTranslator (CSI → Visual Feature Space)

```
CSI features [B, T*tx*rx, sub]
    │
    ├──amplitude──┐
    │              ├─► Encoder (Conv1D stack, 64→128→256)
    └──phase──────┘         │
                            ▼
              ┌──────────────────────────────┐
              │  ruvector-graph-transformer   │
              │                              │
              │  Treat antenna-pair×time as  │
              │  graph nodes. Edges connect  │
              │  spatially adjacent antenna  │
              │  pairs and temporally        │
              │  adjacent frames.            │
              │                              │
              │  Proof-gated attention:      │
              │  Each layer verifies that    │
              │  attention weights satisfy   │
              │  physical constraints        │
              │  (Fresnel ellipsoid bounds)  │
              └──────────────────────────────┘
                            │
                            ▼
              Decoder (ConvTranspose2d stack, 256→128→64→3)
                            │
                            ▼
              Visual features [B, 3, 48, 48]
```

**RuVector enhancement**: Replace standard multi-head self-attention in the bottleneck with `ruvector-graph-transformer`. The graph structure encodes the physical antenna topology — nodes that are closer in space (adjacent ESP32 nodes in the mesh) or time (consecutive frames) have stronger edge weights. This injects domain-specific inductive bias that standard attention lacks.

#### 2b. GNN Body Graph Reasoning

```
Visual features [B, 3, 48, 48]
    │
    ▼
ResNet18 backbone → feature maps [B, 256, 12, 12]
    │
    ▼
┌─────────────────────────────────────────┐
│  ruvector-gnn: Body Graph Network       │
│                                         │
│  17 COCO keypoints as graph nodes       │
│  Edges: anatomical connections          │
│  (shoulder→elbow, hip→knee, etc.)       │
│                                         │
│  GNN message passing (3 rounds):        │
│  h_i^{l+1} = σ(W·h_i^l + Σ_j α_ij·h_j)│
│  α_ij = attention(h_i, h_j, edge_ij)   │
│                                         │
│  Enforces anatomical constraints:       │
│  - Limb length ratios                   │
│  - Joint angle limits                   │
│  - Left-right symmetry priors           │
└─────────────────────────────────────────┘
    │
    ├──────────────────┬──────────────────┐
    ▼                  ▼                  ▼
KeypointHead      DensePoseHead     ConfidenceHead
[B,17,H,W]       [B,25+48,H,W]     [B,1]
heatmaps          parts + UV         quality score
```

**RuVector enhancement**: `ruvector-gnn` replaces the flat spatial decoder with a graph neural network that operates on the human body graph. WiFi CSI is inherently noisy — GNN message passing between anatomically connected joints enforces that predicted keypoints maintain plausible body structure even when individual joint predictions are uncertain.

#### 2c. Sparse Inference for Edge Deployment

```
Trained model weights (full precision)
    │
    ▼
┌─────────────────────────────────────────────┐
│  ruvector-sparse-inference                   │
│                                              │
│  PowerInfer-style activation sparsity:       │
│  - Profile neuron activation frequency       │
│  - Partition into hot (always active, 20%)   │
│    and cold (conditionally active, 80%)      │
│  - Hot neurons: GPU/SIMD fast path           │
│  - Cold neurons: sparse lookup on demand     │
│                                              │
│  Quantization:                               │
│  - Backbone: INT8 (4x memory reduction)      │
│  - DensePose head: FP16 (2x reduction)       │
│  - ModalityTranslator: FP16                  │
│                                              │
│  Target: <50ms inference on ESP32-S3         │
│          <10ms on x86 with AVX2              │
└─────────────────────────────────────────────┘
```

### Stage 3: Training Pipeline

#### 3a. Dataset Loading and Preprocessing

Primary dataset: **MM-Fi** (NeurIPS 2023) — 40 subjects, 27 actions, 114 subcarriers, 3 RX antennas, 17 COCO keypoints + DensePose UV annotations.

Secondary dataset: **Wi-Pose** — 12 subjects, 12 actions, 30 subcarriers, 3×3 antenna array, 18 keypoints.

```
┌──────────────────────────────────────────────────────────┐
│  Data Loading Pipeline                                    │
│                                                          │
│  MM-Fi .npy ──► Resample 114→56 subcarriers ──┐         │
│                (ruvector-solver NeumannSolver)  │         │
│                                                ├──► Batch│
│  Wi-Pose .mat ──► Zero-pad 30→56 subcarriers ──┘  [B,T*│
│                                                    ant, │
│  Phase sanitize ──► Hampel filter ──► unwrap        sub] │
│  (wifi-densepose-signal::phase_sanitizer)                │
│                                                          │
│  Temporal buffer ──► ruvector-temporal-tensor             │
│  (100 frames/sample, tiered quantization)                │
└──────────────────────────────────────────────────────────┘
```

#### 3b. Teacher-Student DensePose Labels

For samples with 3D keypoints but no DensePose UV maps:

1. Run Detectron2 DensePose R-CNN on paired RGB frames (one-time preprocessing step on GPU workstation)
2. Generate `(part_labels [H,W], u_coords [H,W], v_coords [H,W])` pseudo-labels
3. Cache as `.npy` alongside original data
4. Teacher model is discarded after label generation — inference uses WiFi only

#### 3c. Loss Function

```rust
L_total = λ_kp  · L_keypoint      // MSE on predicted vs GT heatmaps
        + λ_part · L_part          // Cross-entropy on 25-class body part segmentation
        + λ_uv   · L_uv           // Smooth L1 on UV coordinate regression
        + λ_xfer · L_transfer     // MSE between CSI features and teacher visual features
        + λ_ot   · L_ot           // Optimal transport regularization (ruvector-math)
        + λ_graph · L_graph       // GNN edge consistency loss (ruvector-gnn)
```

**RuVector enhancement**: `ruvector-math` provides optimal transport (Wasserstein distance) as a regularization term. This penalizes predicted body part distributions that are far from the ground truth in the Wasserstein metric, which is more geometrically meaningful than pixel-wise cross-entropy for spatial body part segmentation.

#### 3d. Training Configuration

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Optimizer | AdamW | Weight decay regularization |
| Learning rate | 1e-3, cosine decay to 1e-5 | Standard for modality translation |
| Batch size | 32 | Fits in 24GB GPU VRAM |
| Epochs | 100 | With early stopping (patience=15) |
| Warmup | 5 epochs | Linear LR warmup |
| Train/val split | Subjects 1-32 / 33-40 | Subject-disjoint for generalization |
| Augmentation | Time-shift ±5 frames, amplitude noise ±2dB, antenna dropout 10% | CSI-domain augmentations |
| Hardware | Single RTX 3090 or A100 | ~8 hours on A100 |
| Checkpoint | Every epoch, keep best-by-validation-PCK | Deterministic seed |

#### 3e. Metrics

| Metric | Target | Description |
|--------|--------|-------------|
| PCK@0.2 | >70% on MM-Fi val | Percentage of correct keypoints (threshold = 0.2 × torso diameter) |
| OKS mAP | >0.50 on MM-Fi val | Object Keypoint Similarity, COCO-standard |
| DensePose GPS | >0.30 on MM-Fi val | Geodesic Point Similarity for UV accuracy |
| Inference latency | <50ms per frame | On x86 with ONNX Runtime |
| Model size | <25MB (FP16) | Suitable for edge deployment |

### Stage 4: Online Adaptation with SONA

After offline training produces a base model, SONA enables continuous adaptation to new environments without retraining from scratch.

```
┌──────────────────────────────────────────────────────────┐
│  SONA Online Adaptation Loop                              │
│                                                          │
│  Base model (frozen weights W)                           │
│       │                                                  │
│       ▼                                                  │
│  ┌──────────────────────────────────┐                    │
│  │  LoRA Adaptation Matrices        │                    │
│  │  W_effective = W + α · A·B       │                    │
│  │                                  │                    │
│  │  Rank r=4 for translator layers  │                    │
│  │  Rank r=2 for backbone layers    │                    │
│  │  Rank r=8 for DensePose head     │                    │
│  │                                  │                    │
│  │  Total trainable params: ~50K    │                    │
│  │  (vs ~5M frozen base)            │                    │
│  └──────────────────────────────────┘                    │
│       │                                                  │
│       ▼                                                  │
│  ┌──────────────────────────────────┐                    │
│  │  EWC++ Regularizer               │                    │
│  │  L = L_task + λ·Σ F_i(θ-θ*)²    │                    │
│  │                                  │                    │
│  │  Prevents forgetting base model  │                    │
│  │  knowledge when adapting to new  │                    │
│  │  environment                     │                    │
│  └──────────────────────────────────┘                    │
│       │                                                  │
│       ▼                                                  │
│  Adaptation triggers:                                    │
│  • First deployment in new room                          │
│  • PCK drops below threshold (drift detection)           │
│  • User manually initiates calibration                   │
│  • Furniture/layout change detected (CSI baseline shift) │
│                                                          │
│  Adaptation data:                                        │
│  • Self-supervised: temporal consistency loss             │
│    (pose at t should be similar to t-1 for slow motion)  │
│  • Semi-supervised: user confirmation of presence/count  │
│  • Optional: brief camera calibration session (5 min)    │
│                                                          │
│  Convergence: 10-50 gradient steps, <5 seconds on CPU    │
└──────────────────────────────────────────────────────────┘
```

### Stage 5: Inference Pipeline (Production)

```
ESP32 CSI (UDP :5005)
    │
    ▼
Rust Axum server (port 8080)
    │
    ├─► RuVector signal preprocessing (Stage 1)
    │       5 crates, ~2ms per frame
    │
    ├─► ONNX Runtime inference (Stage 2)
    │       Quantized model, ~10ms per frame
    │       OR ruvector-sparse-inference, ~8ms per frame
    │
    ├─► GNN post-processing (ruvector-gnn)
    │       Anatomical constraint enforcement, ~1ms
    │
    ├─► SONA adaptation check (Stage 4)
    │       <0.05ms per frame (gradient accumulation only)
    │
    └─► Output: DensePose results
            │
            ├──► /api/v1/stream/pose (WebSocket, 17 keypoints)
            ├──► /api/v1/pose/current (REST, full DensePose)
            └──► /ws/sensing (WebSocket, raw + processed)
```

Total inference budget: **<15ms per frame** at 20 Hz on x86, **<50ms** on ESP32-S3 (with sparse inference).

### Stage 6: RVF Model Container Format

The trained model is packaged as a single `.rvf` file that contains everything needed for
inference — no external weight files, no ONNX runtime, no Python dependencies.

#### RVF DensePose Container Layout

```
wifi-densepose-v1.rvf (single file, ~15-30 MB)
┌───────────────────────────────────────────────────────────────┐
│  SEGMENT 0: Manifest (0x05)                                   │
│  ├── Model ID: "wifi-densepose-v1.0"                          │
│  ├── Training dataset: "mmfi-v1+wipose-v1"                    │
│  ├── Training config hash: SHA-256                            │
│  ├── Target hardware: x86_64, aarch64, wasm32                 │
│  ├── Segment directory (offsets to all segments)               │
│  └── Level-1 TLV manifest with metadata tags                  │
├───────────────────────────────────────────────────────────────┤
│  SEGMENT 1: Vec (0x01) — Model Weight Embeddings              │
│  ├── ModalityTranslator weights [64→128→256→3, Conv1D+ConvT]  │
│  ├── ResNet18 backbone weights [3→64→128→256, residual blocks] │
│  ├── KeypointHead weights [256→17, deconv layers]             │
│  ├── DensePoseHead weights [256→25+48, deconv layers]         │
│  ├── GNN body graph weights [3 message-passing rounds]        │
│  └── Graph transformer attention weights [proof-gated layers] │
│  Format: flat f32 vectors, 768-dim per weight tensor          │
│  Total: ~5M parameters → ~20MB f32, ~10MB f16, ~5MB INT8     │
├───────────────────────────────────────────────────────────────┤
│  SEGMENT 2: Index (0x02) — HNSW Embedding Index               │
│  ├── Layer A: Entry points + coarse routing centroids          │
│  │   (loaded first, <5ms, enables approximate search)         │
│  ├── Layer B: Hot region adjacency for frequently             │
│  │   accessed weight clusters (100ms load)                    │
│  └── Layer C: Full adjacency graph for exact nearest          │
│      neighbor lookup across all weight partitions             │
│  Use: Fast weight lookup for sparse inference —               │
│  only load hot neurons, skip cold neurons via HNSW routing    │
├───────────────────────────────────────────────────────────────┤
│  SEGMENT 3: Overlay (0x03) — Dynamic Min-Cut Graph            │
│  ├── Subcarrier partition graph (sensitive vs insensitive)     │
│  ├── Min-cut witnesses from ruvector-mincut                   │
│  ├── Antenna topology graph (ESP32 mesh spatial layout)       │
│  └── Body skeleton graph (17 COCO joints, 16 edges)           │
│  Use: Pre-computed graph structures loaded at init time.       │
│  Dynamic updates via ruvector-mincut insert/delete_edge       │
│  as environment changes (furniture moves, new obstacles)      │
├───────────────────────────────────────────────────────────────┤
│  SEGMENT 4: Quant (0x06) — Quantization Codebooks             │
│  ├── INT8 codebook for backbone (4x memory reduction)         │
│  ├── FP16 scale factors for translator + heads                │
│  ├── Binary quantization tables for SIMD distance compute     │
│  └── Per-layer calibration statistics (min, max, zero-point)  │
│  Use: rvf-quant temperature-tiered quantization —             │
│  hot layers stay f16, warm layers u8, cold layers binary      │
├───────────────────────────────────────────────────────────────┤
│  SEGMENT 5: Witness (0x0A) — Training Proof Chain             │
│  ├── Deterministic training proof (seed, loss curve, hash)    │
│  ├── Dataset provenance (MM-Fi commit hash, download URL)     │
│  ├── Validation metrics (PCK@0.2, OKS mAP, GPS scores)       │
│  ├── Ed25519 signature over weight hash                       │
│  └── Attestation: training hardware, duration, config         │
│  Use: Verifiable proof that model weights match a specific    │
│  training run. Anyone can re-run training with same seed      │
│  and verify the weight hash matches the witness.              │
├───────────────────────────────────────────────────────────────┤
│  SEGMENT 6: Meta (0x07) — Model Metadata                      │
│  ├── COCO keypoint names and skeleton connectivity            │
│  ├── DensePose body part labels (24 parts + background)       │
│  ├── UV coordinate range and resolution                       │
│  ├── Input normalization statistics (mean, std per subcarrier)│
│  ├── RuVector crate versions used during training             │
│  └── Environment calibration profiles (named, per-room)       │
├───────────────────────────────────────────────────────────────┤
│  SEGMENT 7: AggregateWeights (0x36) — SONA LoRA Deltas        │
│  ├── Per-environment LoRA adaptation matrices (A, B per layer)│
│  ├── EWC++ Fisher information diagonal                        │
│  ├── Optimal θ* reference parameters                          │
│  ├── Adaptation round count and convergence metrics           │
│  └── Named profiles: "lab-a", "living-room", "office-3f"     │
│  Use: Multiple environment adaptations stored in one file.    │
│  Server loads the matching profile or creates a new one.      │
├───────────────────────────────────────────────────────────────┤
│  SEGMENT 8: Profile (0x0B) — RVDNA Domain Profile             │
│  ├── Domain: "wifi-csi-densepose"                             │
│  ├── Input spec: [B, T*ant, sub] CSI tensor format            │
│  ├── Output spec: keypoints [B,17,H,W], parts [B,25,H,W],    │
│  │   UV [B,48,H,W], confidence [B,1]                         │
│  ├── Hardware requirements: min RAM, recommended GPU          │
│  └── Supported data sources: esp32, wifi-rssi, simulation    │
├───────────────────────────────────────────────────────────────┤
│  SEGMENT 9: Crypto (0x0C) — Signature and Keys                │
│  ├── Ed25519 public key for model publisher                   │
│  ├── Signature over all segment content hashes                │
│  └── Certificate chain (optional, for enterprise deployment)  │
├───────────────────────────────────────────────────────────────┤
│  SEGMENT 10: Wasm (0x10) — Self-Bootstrapping Runtime         │
│  ├── Compiled WASM inference engine                           │
│  │   (ruvector-sparse-inference-wasm)                         │
│  ├── WASM microkernel for RVF segment parsing                 │
│  └── Browser-compatible: load .rvf → run inference in-browser │
│  Use: The .rvf file is fully self-contained — a WASM host     │
│  can execute inference without any external dependencies.     │
├───────────────────────────────────────────────────────────────┤
│  SEGMENT 11: Dashboard (0x11) — Embedded Visualization        │
│  ├── Three.js-based pose visualization (HTML/JS/CSS)          │
│  ├── Gaussian splat renderer for signal field                 │
│  └── Served at http://localhost:8080/ when model is loaded    │
│  Use: Open the .rvf file → get a working UI with no install  │
└───────────────────────────────────────────────────────────────┘
```

#### RVF Loading Sequence

```
1. Read tail → find_latest_manifest() → SegmentDirectory
2. Load Manifest (seg 0) → validate magic, version, model ID
3. Load Profile (seg 8) → verify input/output spec compatibility
4. Load Crypto (seg 9) → verify Ed25519 signature chain
5. Load Quant (seg 4) → prepare quantization codebooks
6. Load Index Layer A (seg 2) → entry points ready (<5ms)
       ↓ (inference available at reduced accuracy)
7. Load Vec (seg 1) → hot weight partitions via Layer A routing
8. Load Index Layer B (seg 2) → hot adjacency ready (100ms)
       ↓ (inference at full accuracy for common poses)
9. Load Overlay (seg 3) → min-cut graphs, body skeleton
10. Load AggregateWeights (seg 7) → apply matching SONA profile
11. Load Index Layer C (seg 2) → complete graph loaded
       ↓ (full inference with all weight partitions)
12. Load Wasm (seg 10) → WASM runtime available (optional)
13. Load Dashboard (seg 11) → UI served (optional)
```

**Progressive availability**: Inference begins after step 6 (~5ms) with approximate
results. Full accuracy is reached by step 9 (~500ms). This enables instant startup
with gradually improving quality — critical for real-time applications.

#### RVF Build Pipeline

After training completes, the model is packaged into an `.rvf` file:

```bash
# Build the RVF container from trained checkpoint
cargo run -p wifi-densepose-train --bin build-rvf -- \
    --checkpoint checkpoints/best-pck.pt \
    --quantize int8,fp16 \
    --hnsw-build \
    --sign --key model-signing-key.pem \
    --include-wasm \
    --include-dashboard ../../ui \
    --output wifi-densepose-v1.rvf

# Verify the built container
cargo run -p wifi-densepose-train --bin verify-rvf -- \
    --input wifi-densepose-v1.rvf \
    --verify-signature \
    --verify-witness \
    --benchmark-inference
```

#### RVF Runtime Integration

The sensing server loads the `.rvf` container at startup:

```bash
# Load model from RVF container
./target/release/sensing-server \
    --model wifi-densepose-v1.rvf \
    --source auto \
    --ui-from-rvf  # serve Dashboard segment instead of --ui-path
```

```rust
// In sensing-server/src/main.rs
use rvf_runtime::RvfContainer;
use rvf_index::layers::IndexLayer;
use rvf_quant::QuantizedVec;

let container = RvfContainer::open("wifi-densepose-v1.rvf")?;

// Progressive load: Layer A first for instant startup
let index = container.load_index(IndexLayer::A)?;
let weights = container.load_vec_hot(&index)?;  // hot partitions only

// Full load in background
tokio::spawn(async move {
    container.load_index(IndexLayer::B).await?;
    container.load_index(IndexLayer::C).await?;
    container.load_vec_cold().await?;  // remaining partitions
});

// SONA environment adaptation
let sona_deltas = container.load_aggregate_weights("office-3f")?;
model.apply_lora_deltas(&sona_deltas);

// Serve embedded dashboard
let dashboard = container.load_dashboard()?;
// Mount at /ui/* routes in Axum
```

## Implementation Plan

### Phase 1: Dataset Loaders (2 weeks)

- Implement `MmFiDataset` in `wifi-densepose-train/src/dataset.rs`
- Read MM-Fi `.npy` files with antenna correction (1TX/3RX → 3×3 zero-padding)
- Subcarrier resampling 114→56 via `ruvector-solver::NeumannSolver`
- Phase sanitization via `wifi-densepose-signal::phase_sanitizer`
- Implement `WiPoseDataset` for secondary dataset
- Temporal windowing with `ruvector-temporal-tensor`
- **Deliverable**: `cargo test -p wifi-densepose-train` with dataset loading tests

### Phase 2: Graph Transformer Integration (2 weeks)

- Add `ruvector-graph-transformer` dependency to `wifi-densepose-train`
- Replace bottleneck self-attention in `ModalityTranslator` with proof-gated graph transformer
- Build antenna topology graph (nodes = antenna pairs, edges = spatial/temporal proximity)
- Add `ruvector-gnn` dependency for body graph reasoning
- Build COCO body skeleton graph (17 nodes, 16 anatomical edges)
- Implement GNN message passing in spatial decoder
- **Deliverable**: Model forward pass produces correct output shapes with graph layers

### Phase 3: Teacher-Student Label Generation (1 week)

- Python script using Detectron2 DensePose to generate UV pseudo-labels from MM-Fi RGB frames
- Cache labels as `.npy` for Rust loader consumption
- Validate label quality on a random subset (visual inspection)
- **Deliverable**: Complete UV label set for MM-Fi training split

### Phase 4: Training Loop (3 weeks)

- Implement `WiFiDensePoseTrainer` with full loss function (6 terms)
- Add `ruvector-math` optimal transport loss term
- Integrate GNN edge consistency loss
- Training loop with cosine LR schedule, early stopping, checkpointing
- Validation metrics: PCK@0.2, OKS mAP, DensePose GPS
- Deterministic proof verification (`proof.rs`) with weight hash
- **Deliverable**: Trained model checkpoint achieving PCK@0.2 >70% on MM-Fi validation

### Phase 5: SONA Online Adaptation (2 weeks)

- Integrate `ruvector-sona` into inference pipeline
- Implement LoRA injection at translator, backbone, and DensePose head layers
- Implement EWC++ Fisher information computation and regularization
- Self-supervised temporal consistency loss for unsupervised adaptation
- Calibration mode: 5-minute camera session for supervised fine-tuning
- Drift detection: monitor rolling PCK on temporal consistency proxy
- **Deliverable**: Adaptation converges in <50 gradient steps, PCK recovers within 10% of base

### Phase 6: Sparse Inference and Edge Deployment (2 weeks)

- Profile neuron activation frequencies on validation set
- Apply `ruvector-sparse-inference` hot/cold neuron partitioning
- INT8 quantization for backbone, FP16 for heads
- ONNX export with quantized weights
- Benchmark on x86 (target: <10ms) and ARM (target: <50ms)
- WASM export via `ruvector-sparse-inference-wasm` for browser inference
- **Deliverable**: Quantized ONNX model, benchmark results, WASM binary

### Phase 7: RVF Container Build Pipeline (2 weeks)

- Implement `build-rvf` binary in `wifi-densepose-train`
- Serialize trained weights into `Vec` segment (SegmentType::Vec, 0x01)
- Build HNSW index over weight partitions for sparse inference (SegmentType::Index, 0x02)
- Serialize min-cut graph overlays: subcarrier partition, antenna topology, body skeleton (SegmentType::Overlay, 0x03)
- Generate quantization codebooks via `rvf-quant` (SegmentType::Quant, 0x06)
- Write training proof witness with Ed25519 signature (SegmentType::Witness, 0x0A)
- Store model metadata, COCO keypoint schema, normalization stats (SegmentType::Meta, 0x07)
- Store SONA LoRA adaptation deltas per environment (SegmentType::AggregateWeights, 0x36)
- Write RVDNA domain profile for WiFi CSI DensePose (SegmentType::Profile, 0x0B)
- Optionally embed WASM inference runtime (SegmentType::Wasm, 0x10)
- Optionally embed Three.js dashboard (SegmentType::Dashboard, 0x11)
- Build Level-1 manifest and segment directory (SegmentType::Manifest, 0x05)
- Implement `verify-rvf` binary for container validation
- **Deliverable**: `wifi-densepose-v1.rvf` single-file container, verifiable and self-contained

### Phase 8: Integration with Sensing Server (1 week)

- Load `.rvf` container in `wifi-densepose-sensing-server` via `rvf-runtime`
- Progressive loading: Layer A first for instant startup, full graph in background
- Replace `derive_pose_from_sensing()` heuristic with trained model inference
- Add `--model` CLI flag accepting `.rvf` path (or legacy `.onnx`)
- Apply SONA LoRA deltas from `AggregateWeights` segment based on `--env` flag
- Serve embedded Dashboard segment at `/ui/*` when `--ui-from-rvf` is set
- Graceful fallback to heuristic when no model file present
- Update WebSocket protocol to include DensePose UV data
- **Deliverable**: Sensing server serves trained model from single `.rvf` file

## File Changes

### New Files

| File | Purpose |
|------|---------|
| `rust-port/.../wifi-densepose-train/src/dataset_mmfi.rs` | MM-Fi dataset loader with subcarrier resampling |
| `rust-port/.../wifi-densepose-train/src/dataset_wipose.rs` | Wi-Pose dataset loader |
| `rust-port/.../wifi-densepose-train/src/graph_transformer.rs` | Graph transformer integration |
| `rust-port/.../wifi-densepose-train/src/body_gnn.rs` | GNN body graph reasoning |
| `rust-port/.../wifi-densepose-train/src/adaptation.rs` | SONA LoRA + EWC++ adaptation |
| `rust-port/.../wifi-densepose-train/src/trainer.rs` | Training loop with multi-term loss |
| `scripts/generate_densepose_labels.py` | Teacher-student UV label generation |
| `scripts/benchmark_inference.py` | Inference latency benchmarking |
| `rust-port/.../wifi-densepose-train/src/rvf_builder.rs` | RVF container build pipeline |
| `rust-port/.../wifi-densepose-train/src/bin/build_rvf.rs` | CLI binary for building `.rvf` containers |
| `rust-port/.../wifi-densepose-train/src/bin/verify_rvf.rs` | CLI binary for verifying `.rvf` containers |

### Modified Files

| File | Change |
|------|--------|
| `rust-port/.../wifi-densepose-train/Cargo.toml` | Add ruvector-gnn, graph-transformer, sona, sparse-inference, math, rvf-types, rvf-wire, rvf-manifest, rvf-index, rvf-quant, rvf-crypto, rvf-runtime deps |
| `rust-port/.../wifi-densepose-train/src/model.rs` | Integrate graph transformer + GNN layers |
| `rust-port/.../wifi-densepose-train/src/losses.rs` | Add optimal transport + GNN edge consistency loss terms |
| `rust-port/.../wifi-densepose-train/src/config.rs` | Add training hyperparameters for new components |
| `rust-port/.../sensing-server/Cargo.toml` | Add rvf-runtime, rvf-types, rvf-index, rvf-quant deps |
| `rust-port/.../sensing-server/src/main.rs` | Add `--model` flag, load `.rvf` container, progressive startup, serve embedded dashboard |

## Consequences

### Positive

- **Trained model produces accurate DensePose**: Moves from heuristic keypoints to learned body surface estimation backed by public dataset evaluation
- **RuVector signal intelligence is a differentiator**: Graph transformers on antenna topology and GNN body reasoning are novel — no prior WiFi pose system uses these techniques
- **SONA enables zero-shot deployment**: New environments don't require full retraining — LoRA adaptation with <50 gradient steps converges in seconds
- **Sparse inference enables edge deployment**: PowerInfer-style neuron partitioning brings DensePose inference to ESP32-class hardware
- **Graceful degradation**: Server falls back to heuristic pose when no model file is present — existing functionality is preserved
- **Single-file deployment via RVF**: Trained model, embeddings, HNSW index, quantization codebooks, SONA adaptation profiles, WASM runtime, and dashboard UI packaged in one `.rvf` file — deploy by copying a single file
- **Progressive loading**: RVF Layer A loads in <5ms for instant startup; full accuracy reached in ~500ms as remaining segments load
- **Verifiable provenance**: RVF Witness segment contains deterministic training proof with Ed25519 signature — anyone can re-run training and verify weight hash
- **Self-bootstrapping**: RVF Wasm segment enables browser-based inference with no server-side dependencies
- **Open evaluation**: PCK, OKS, GPS metrics on public MM-Fi dataset provide reproducible, comparable results

### Negative

- **Training requires GPU**: Initial model training needs RTX 3090 or better (~8 hours on A100). Not all developers will have access.
- **Teacher-student label generation requires Detectron2**: One-time Python + CUDA dependency for generating UV pseudo-labels from RGB frames
- **MM-Fi CC BY-NC license**: Weights trained on MM-Fi cannot be used commercially without collecting proprietary data
- **Environment-specific adaptation still required**: SONA reduces the burden but a brief calibration session in each new environment is still recommended for best accuracy
- **6 additional RuVector crate dependencies**: Increases compile time and binary size. Mitigated by feature flags (e.g., `--features trained-model`).
- **Model size on disk**: ~25MB (FP16) or ~12MB (INT8). Acceptable for server deployment, may need further pruning for WASM.

### Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| MM-Fi 114→56 interpolation loses accuracy | Train at native 114 as alternative; ESP32 mesh can collect 56-sub data natively |
| GNN overfits to training body types | Augment with diverse body proportions; Wi-Pose adds subject diversity |
| SONA adaptation diverges in adversarial environments | EWC++ regularization caps parameter drift; rollback to base weights on detection |
| Sparse inference degrades accuracy | Benchmark INT8 vs FP16 vs FP32; fall back to full precision if quality drops |
| Training proof hash changes with RuVector version updates | Pin ruvector crate versions in Cargo.toml; regenerate hash on version bumps |

## References

- Geng et al., "DensePose From WiFi" (CMU, arXiv:2301.00250, 2023)
- Yang et al., "MM-Fi: Multi-Modal Non-Intrusive 4D Human Dataset" (NeurIPS 2023, arXiv:2305.10345)
- Hu et al., "LoRA: Low-Rank Adaptation of Large Language Models" (ICLR 2022)
- Kirkpatrick et al., "Overcoming Catastrophic Forgetting in Neural Networks" (PNAS, 2017)
- Song et al., "PowerInfer: Fast Large Language Model Serving with a Consumer-grade GPU" (2024)
- ADR-005: SONA Self-Learning for Pose Estimation
- ADR-015: Public Dataset Strategy for Trained Pose Estimation Model
- ADR-016: RuVector Integration for Training Pipeline
- ADR-020: Migrate AI/Model Inference to Rust with RuVector and ONNX Runtime

## Appendix A: RuQu Consideration

**ruQu** ("Classical nervous system for quantum machines") provides real-time coherence
assessment via dynamic min-cut. While primarily designed for quantum error correction
(syndrome decoding, surface code arbitration), its core primitive — the `CoherenceGate` —
is architecturally relevant to WiFi CSI processing:

- **CoherenceGate** uses `ruvector-mincut` to make real-time gate/pass decisions on
  signal streams based on structural coherence thresholds. In quantum computing, this
  gates qubit syndrome streams. For WiFi CSI, the same mechanism could gate CSI
  subcarrier streams — passing only subcarriers whose coherence (phase stability across
  antennas) exceeds a dynamic threshold.

- **Syndrome filtering** (`filters.rs`) implements Kalman-like adaptive filters that
  could be repurposed for CSI noise filtering — treating each subcarrier's amplitude
  drift as a "syndrome" stream.

- **Min-cut gated transformer** integration (optional feature) provides coherence-optimized
  attention with 50% FLOP reduction — directly applicable to the `ModalityTranslator`
  bottleneck.

**Decision**: ruQu is not included in the initial pipeline (Phase 1-8) but is marked as a
**Phase 9 exploration** candidate for coherence-gated CSI filtering. The CoherenceGate
primitive maps naturally to subcarrier quality assessment, and the integration path is
clean since ruQu already depends on `ruvector-mincut`.

## Appendix B: Training Data Strategy

The pipeline supports three data sources for training, used in combination:

| Source | Subcarriers | Pose Labels | Volume | Cost | When |
|--------|-------------|-------------|--------|------|------|
| **MM-Fi** (public) | 114 → 56 (interpolated) | 17 COCO + DensePose UV | 40 subjects, 320K frames | Free (CC BY-NC) | Phase 1 — bootstrap |
| **Wi-Pose** (public) | 30 → 56 (zero-padded) | 18 keypoints | 12 subjects, 166K packets | Free (research) | Phase 1 — diversity |
| **ESP32 self-collected** | 56 (native) | Teacher-student from camera | Unlimited, environment-specific | Hardware only ($54) | Phase 4+ — fine-tuning |

**Recommended approach: Both public + ESP32 data.**

1. **Pre-train on MM-Fi + Wi-Pose** (public data, Phase 1-4): Provides the base model
   with diverse subjects and actions. The 114→56 subcarrier interpolation is acceptable
   for learning general CSI-to-pose mappings.

2. **Fine-tune on ESP32 self-collected data** (Phase 5+, SONA adaptation): Collect
   5-30 minutes of paired ESP32 CSI + camera data in each target environment. The camera
   serves as the teacher model (Detectron2 generates pseudo-labels). SONA LoRA adaptation
   takes <50 gradient steps to converge.

3. **Continuous adaptation** (runtime): SONA's self-supervised temporal consistency loss
   refines the model without any camera, using the assumption that poses change smoothly
   over short time windows.

This three-tier strategy gives you:
- A working model from day one (public data)
- Environment-specific accuracy (ESP32 fine-tuning)
- Ongoing drift correction (SONA runtime adaptation)
