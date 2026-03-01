# ADR-027: Project MERIDIAN -- Cross-Environment Domain Generalization for WiFi Pose Estimation

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-03-01 |
| **Deciders** | ruv |
| **Codename** | **MERIDIAN** -- Multi-Environment Robust Inference via Domain-Invariant Alignment Networks |
| **Relates to** | ADR-005 (SONA Self-Learning), ADR-014 (SOTA Signal Processing), ADR-015 (Public Datasets), ADR-016 (RuVector Integration), ADR-023 (Trained DensePose Pipeline), ADR-024 (AETHER Contrastive Embeddings) |

---

## 1. Context

### 1.1 The Domain Gap Problem

WiFi-based pose estimation models exhibit severe performance degradation when deployed in environments different from their training setting. A model trained in Room A with a specific transceiver layout, wall material composition, and furniture arrangement can lose 40-70% accuracy when moved to Room B -- even in the same building. This brittleness is the single largest barrier to real-world WiFi sensing deployment.

The root cause is three-fold:

1. **Layout overfitting**: Models memorize the spatial relationship between transmitter, receiver, and the coordinate system, rather than learning environment-agnostic human motion features. PerceptAlign (Chen et al., 2026; arXiv:2601.12252) demonstrated that cross-layout error drops by >60% when geometry conditioning is introduced.

2. **Multipath memorization**: The multipath channel profile encodes room geometry (wall positions, furniture, materials) as a static fingerprint. Models learn this fingerprint as a shortcut, using room-specific multipath patterns to predict positions rather than extracting pose-relevant body reflections.

3. **Hardware heterogeneity**: Different WiFi chipsets (ESP32, Intel 5300, Atheros) produce CSI with different subcarrier counts, phase noise profiles, and sampling rates. A model trained on Intel 5300 (30 subcarriers, 3x3 MIMO) fails on ESP32-S3 (64 subcarriers, 1x1 SISO).

The current wifi-densepose system (ADR-023) trains and evaluates on a single environment from MM-Fi or Wi-Pose. There is no mechanism to disentangle human motion from environment, adapt to new rooms without full retraining, or handle mixed hardware deployments.

### 1.2 SOTA Landscape (2024-2026)

Five concurrent lines of research have converged on the domain generalization problem:

**Cross-Layout Pose Estimation:**
- **PerceptAlign** (Chen et al., 2026; arXiv:2601.12252): First geometry-conditioned framework. Encodes transceiver positions into high-dimensional embeddings fused with CSI features, achieving 60%+ cross-domain error reduction. Constructed the largest cross-domain WiFi pose dataset: 21 subjects, 5 scenes, 18 actions, 7 layouts.
- **AdaPose** (Zhou et al., 2024; IEEE IoT Journal, arXiv:2309.16964): Mapping Consistency Loss aligns domain discrepancy at the mapping level. First to address cross-domain WiFi pose estimation specifically.
- **Person-in-WiFi 3D** (Yan et al., CVPR 2024): End-to-end multi-person 3D pose from WiFi, achieving 91.7mm single-person error, but generalization across layouts remains an open problem.

**Domain Generalization Frameworks:**
- **DGSense** (Zhou et al., 2025; arXiv:2502.08155): Virtual data generator + episodic training for domain-invariant features. Generalizes to unseen domains without target data across WiFi, mmWave, and acoustic sensing.
- **Context-Aware Predictive Coding (CAPC)** (2024; arXiv:2410.01825; IEEE OJCOMS): Self-supervised CPC + Barlow Twins for WiFi, with 24.7% accuracy improvement over supervised learning on unseen environments.

**Foundation Models:**
- **X-Fi** (Chen & Yang, ICLR 2025; arXiv:2410.10167): First modality-invariant foundation model for human sensing. X-fusion mechanism preserves modality-specific features. 24.8% MPJPE improvement on MM-Fi.
- **AM-FM** (2026; arXiv:2602.11200): First WiFi foundation model, pre-trained on 9.2M unlabeled CSI samples across 20 device types over 439 days. Contrastive learning + masked reconstruction + physics-informed objectives.

**Generative Approaches:**
- **LatentCSI** (Ramesh et al., 2025; arXiv:2506.10605): Lightweight CSI encoder maps directly into Stable Diffusion 3 latent space, demonstrating that CSI contains enough spatial information to reconstruct room imagery.

### 1.3 What MERIDIAN Adds to the Existing System

| Current Capability | Gap | MERIDIAN Addition |
|-------------------|-----|------------------|
| AETHER embeddings (ADR-024) | Embeddings encode environment identity -- useful for fingerprinting but harmful for cross-environment transfer | Environment-disentangled embeddings with explicit factorization |
| SONA LoRA adapters (ADR-005) | Adapters must be manually created per environment; no mechanism to generate them from few-shot data | Zero-shot environment adaptation via geometry-conditioned inference |
| MM-Fi/Wi-Pose training (ADR-015) | Single-environment train/eval; no cross-domain protocol | Multi-domain training protocol with environment augmentation |
| SpotFi phase correction (ADR-014) | Hardware-specific phase calibration | Hardware-invariant CSI normalization layer |
| RuVector attention (ADR-016) | Attention weights learn environment-specific patterns | Domain-adversarial attention regularization |

---

## 2. Decision

### 2.1 Architecture: Environment-Disentangled Dual-Path Transformer

MERIDIAN adds a domain generalization layer between the CSI encoder and the pose/embedding heads. The core insight is explicit factorization: decompose the latent representation into a **pose-relevant** component (invariant across environments) and an **environment** component (captures room geometry, hardware, layout):

```
CSI Frame(s) [n_pairs x n_subcarriers]
     |
     v
  HardwareNormalizer                         [NEW: chipset-invariant preprocessing]
     |   - Resample to canonical 56 subcarriers
     |   - Normalize amplitude distribution to N(0,1) per-frame
     |   - Apply SanitizedPhaseTransform (hardware-agnostic)
     |
     v
  csi_embed (Linear 56 -> d_model=64)       [EXISTING]
     |
     v
  CrossAttention (Q=keypoint_queries,        [EXISTING]
                   K,V=csi_embed)
     |
     v
  GnnStack (2-layer GCN)                    [EXISTING]
     |
     v
  body_part_features [17 x 64]              [EXISTING]
     |
     +---> DomainFactorizer:                 [NEW]
     |       |
     |       +---> PoseEncoder:              [NEW: domain-invariant path]
     |       |       fc1: Linear(64, 128) + LayerNorm + GELU
     |       |       fc2: Linear(128, 64)
     |       |       --> h_pose [17 x 64]    (invariant to environment)
     |       |
     |       +---> EnvEncoder:               [NEW: environment-specific path]
     |               GlobalMeanPool [17 x 64] -> [64]
     |               fc_env: Linear(64, 32)
     |               --> h_env [32]           (captures room/hardware identity)
     |
     +---> h_pose ---> xyz_head + conf_head  [EXISTING: pose regression]
     |               --> keypoints [17 x (x,y,z,conf)]
     |
     +---> h_pose ---> MeanPool -> ProjectionHead -> z_csi [128]  [ADR-024 AETHER]
     |
     +---> h_env  ---> (discarded at inference; used only for training signal)
```

### 2.2 Domain-Adversarial Training with Gradient Reversal

To force `h_pose` to be environment-invariant, we employ domain-adversarial training (Ganin et al., 2016) with a gradient reversal layer (GRL):

```
h_pose [17 x 64]
     |
     +---> [Normal gradient]  --> xyz_head --> L_pose
     |
     +---> [GRL: multiply grad by -lambda_adv]
              |
              v
          DomainClassifier:
              MeanPool [17 x 64] -> [64]
              fc1: Linear(64, 32) + ReLU + Dropout(0.3)
              fc2: Linear(32, n_domains)
              --> domain_logits
              --> L_domain = CrossEntropy(domain_logits, domain_label)

Total loss:
  L = L_pose + lambda_c * L_contrastive + lambda_adv * L_domain
                                           + lambda_env * L_env_recon
```

The GRL reverses the gradient flowing from `L_domain` into `PoseEncoder`, meaning the PoseEncoder is trained to **maximize** domain classification error -- forcing `h_pose` to shed all environment-specific information.

**Key hyperparameters:**
- `lambda_adv`: Adversarial weight, annealed from 0.0 to 1.0 over first 20 epochs using the schedule `lambda_adv(p) = 2 / (1 + exp(-10 * p)) - 1` where `p = epoch / max_epochs`
- `lambda_env = 0.1`: Environment reconstruction weight (auxiliary task to ensure `h_env` captures what `h_pose` discards)
- `lambda_c = 0.1`: Contrastive loss weight from AETHER (unchanged)

### 2.3 Geometry-Conditioned Inference (Zero-Shot Adaptation)

Inspired by PerceptAlign, MERIDIAN conditions the pose decoder on the physical transceiver geometry. At deployment time, the user provides AP/sensor positions (known from installation), and the model adjusts its coordinate frame accordingly:

```rust
/// Encodes transceiver geometry into a conditioning vector.
/// Positions are in meters relative to an arbitrary room origin.
pub struct GeometryEncoder {
    /// Fourier positional encoding of 3D coordinates
    pos_embed: FourierPositionalEncoding,  // 3 coords -> 64 dims per position
    /// Aggregates variable-count AP positions into fixed-dim vector
    set_encoder: DeepSets,                 // permutation-invariant {AP_1..AP_n} -> 64
}

/// Fourier features: [sin(2^0 * pi * x), cos(2^0 * pi * x), ...,
///                     sin(2^(L-1) * pi * x), cos(2^(L-1) * pi * x)]
/// L = 10 frequency bands, producing 60 dims per coordinate (+ 3 raw = 63, padded to 64)
pub struct FourierPositionalEncoding {
    n_frequencies: usize,  // default: 10
    scale: f32,            // default: 1.0 (meters)
}

/// DeepSets: phi(x) -> mean-pool -> rho(.) for permutation-invariant set encoding
pub struct DeepSets {
    phi: Linear,    // 64 -> 64
    rho: Linear,    // 64 -> 64
}
```

The geometry embedding `g` (64-dim) is injected into the pose decoder via FiLM conditioning:

```
g = GeometryEncoder(ap_positions)   [64-dim]
gamma = Linear(64, 64)(g)           [per-feature scale]
beta  = Linear(64, 64)(g)           [per-feature shift]

h_pose_conditioned = gamma * h_pose + beta    [FiLM: Feature-wise Linear Modulation]
     |
     v
  xyz_head --> keypoints
```

This enables zero-shot deployment: given the positions of WiFi APs in a new room, the model adapts its coordinate prediction without any retraining.

### 2.4 Hardware-Invariant CSI Normalization

```rust
/// Normalizes CSI from heterogeneous hardware to a canonical representation.
/// Handles ESP32-S3 (64 sub), Intel 5300 (30 sub), Atheros (56 sub).
pub struct HardwareNormalizer {
    /// Target subcarrier count (project all hardware to this)
    canonical_subcarriers: usize,  // default: 56 (matches MM-Fi)
    /// Per-hardware amplitude statistics for z-score normalization
    hw_stats: HashMap<HardwareType, AmplitudeStats>,
}

pub enum HardwareType {
    Esp32S3 { subcarriers: usize, mimo: (u8, u8) },
    Intel5300 { subcarriers: usize, mimo: (u8, u8) },
    Atheros { subcarriers: usize, mimo: (u8, u8) },
    Generic { subcarriers: usize, mimo: (u8, u8) },
}

impl HardwareNormalizer {
    /// Normalize a raw CSI frame to canonical form:
    /// 1. Resample subcarriers to canonical count via cubic interpolation
    /// 2. Z-score normalize amplitude per-frame
    /// 3. Sanitize phase: remove hardware-specific linear phase offset
    pub fn normalize(&self, frame: &CsiFrame) -> CanonicalCsiFrame { .. }
}
```

The resampling uses `ruvector-solver`'s sparse interpolation (already integrated per ADR-016) to project from any subcarrier count to the canonical 56.

### 2.5 Virtual Environment Augmentation

Following DGSense's virtual data generator concept, MERIDIAN augments training data with synthetic domain shifts:

```rust
/// Generates virtual CSI domains by simulating environment variations.
pub struct VirtualDomainAugmentor {
    /// Simulate different room sizes via multipath delay scaling
    room_scale_range: (f32, f32),    // default: (0.5, 2.0)
    /// Simulate wall material via reflection coefficient perturbation
    reflection_coeff_range: (f32, f32),  // default: (0.3, 0.9)
    /// Simulate furniture via random scatterer injection
    n_virtual_scatterers: (usize, usize),  // default: (0, 5)
    /// Simulate hardware differences via subcarrier response shaping
    hw_response_filters: Vec<SubcarrierResponseFilter>,
}

impl VirtualDomainAugmentor {
    /// Apply a random virtual domain shift to a CSI batch.
    /// Each call generates a new "virtual environment" for training diversity.
    pub fn augment(&self, batch: &CsiBatch, rng: &mut impl Rng) -> CsiBatch { .. }
}
```

During training, each mini-batch is augmented with K=3 virtual domain shifts, producing 4x the effective training environments. The domain classifier sees both real and virtual domain labels, improving its ability to force environment-invariant features.

### 2.6 Few-Shot Rapid Adaptation

For deployment scenarios where a brief calibration period is available (10-60 seconds of CSI data from the new environment, no pose labels needed):

```rust
/// Rapid adaptation to a new environment using unlabeled CSI data.
/// Combines SONA LoRA adapters (ADR-005) with MERIDIAN's domain factorization.
pub struct RapidAdaptation {
    /// Number of unlabeled CSI frames needed for adaptation
    min_calibration_frames: usize,  // default: 200 (10 sec @ 20 Hz)
    /// LoRA rank for environment-specific adaptation
    lora_rank: usize,               // default: 4
    /// Self-supervised adaptation loss (AETHER contrastive + entropy min)
    adaptation_loss: AdaptationLoss,
}

pub enum AdaptationLoss {
    /// Test-time training with AETHER contrastive loss on unlabeled data
    ContrastiveTTT { epochs: usize, lr: f32 },
    /// Entropy minimization on pose confidence outputs
    EntropyMin { epochs: usize, lr: f32 },
    /// Combined: contrastive + entropy minimization
    Combined { epochs: usize, lr: f32, lambda_ent: f32 },
}
```

This leverages the existing SONA infrastructure (ADR-005) to generate environment-specific LoRA weights from unlabeled CSI alone, bridging the gap between zero-shot geometry conditioning and full supervised fine-tuning.

---

## 3. Comparison: MERIDIAN vs Alternatives

| Approach | Cross-Layout | Cross-Hardware | Zero-Shot | Few-Shot | Edge-Compatible | Multi-Person |
|----------|-------------|----------------|-----------|----------|-----------------|-------------|
| **MERIDIAN (this ADR)** | Yes (GRL + geometry FiLM) | Yes (HardwareNormalizer) | Yes (geometry conditioning) | Yes (SONA + contrastive TTT) | Yes (adds ~12K params) | Yes (via ADR-023) |
| PerceptAlign (2026) | Yes | No | Partial (needs layout) | No | Unknown (20M params) | No |
| AdaPose (2024) | Partial (2 domains) | No | No | Yes (mapping consistency) | Unknown | No |
| DGSense (2025) | Yes (virtual aug) | Yes (multi-modality) | Yes | No | No (ResNet backbone) | No |
| X-Fi (ICLR 2025) | Yes (foundation model) | Yes (multi-modal) | Yes | Yes (pre-trained) | No (large transformer) | Yes |
| AM-FM (2026) | Yes (439-day pretraining) | Yes (20 device types) | Yes | Yes | No (foundation scale) | Unknown |
| CAPC (2024) | Partial (transfer learning) | No | No | Yes (SSL fine-tune) | Yes (lightweight) | No |
| **Current wifi-densepose** | **No** | **No** | **No** | **Partial (SONA manual)** | **Yes** | **Yes** |

### MERIDIAN's Differentiators

1. **Additive, not replacement**: Unlike X-Fi or AM-FM which require new foundation model infrastructure, MERIDIAN adds 4 small modules to the existing ADR-023 pipeline.
2. **Edge-compatible**: Total parameter overhead is ~12K (geometry encoder ~8K, domain factorizer ~4K), fitting within the ESP32 budget established in ADR-024.
3. **Hardware-agnostic**: First approach to combine cross-layout AND cross-hardware generalization in a single framework, using the existing `ruvector-solver` sparse interpolation.
4. **Continuum of adaptation**: Supports zero-shot (geometry only), few-shot (10-sec calibration), and full fine-tuning on the same architecture.

---

## 4. Implementation

### 4.1 Phase 1 -- Hardware Normalizer (Week 1)

**Goal**: Canonical CSI representation across ESP32, Intel 5300, and Atheros hardware.

**Files modified:**
- `crates/wifi-densepose-signal/src/hardware_norm.rs` (new)
- `crates/wifi-densepose-signal/src/lib.rs` (export new module)
- `crates/wifi-densepose-train/src/dataset.rs` (apply normalizer in data pipeline)

**Dependencies**: `ruvector-solver` (sparse interpolation, already vendored)

**Acceptance criteria:**
- [ ] Resample any subcarrier count to canonical 56 within 50us per frame
- [ ] Z-score normalization produces mean=0, std=1 per-frame amplitude
- [ ] Phase sanitization removes linear trend (validated against SpotFi output)
- [ ] Unit tests with synthetic ESP32 (64 sub) and Intel 5300 (30 sub) frames

### 4.2 Phase 2 -- Domain Factorizer + GRL (Week 2-3)

**Goal**: Disentangle pose-relevant and environment-specific features during training.

**Files modified:**
- `crates/wifi-densepose-train/src/domain.rs` (new: DomainFactorizer, GRL, DomainClassifier)
- `crates/wifi-densepose-train/src/graph_transformer.rs` (wire factorizer after GNN)
- `crates/wifi-densepose-train/src/trainer.rs` (add L_domain to composite loss, GRL annealing)
- `crates/wifi-densepose-train/src/dataset.rs` (add domain labels to DataPipeline)

**Key implementation detail -- Gradient Reversal Layer:**

```rust
/// Gradient Reversal Layer: identity in forward pass, negates gradient in backward.
/// Used to train the PoseEncoder to produce domain-invariant features.
pub struct GradientReversalLayer {
    lambda: f32,
}

impl GradientReversalLayer {
    /// Forward: identity. Backward: multiply gradient by -lambda.
    /// In our pure-Rust autograd, this is implemented as:
    ///   forward(x) = x
    ///   backward(grad) = -lambda * grad
    pub fn forward(&self, x: &Tensor) -> Tensor {
        // Store lambda for backward pass in computation graph
        x.clone_with_grad_fn(GrlBackward { lambda: self.lambda })
    }
}
```

**Acceptance criteria:**
- [ ] Domain classifier achieves >90% accuracy on source domains (proves signal exists)
- [ ] After GRL training, domain classifier accuracy drops to near-chance (proves disentanglement)
- [ ] Pose accuracy on source domains degrades <5% vs non-adversarial baseline
- [ ] Cross-domain pose accuracy improves >20% on held-out environment

### 4.3 Phase 3 -- Geometry Encoder + FiLM Conditioning (Week 3-4)

**Goal**: Enable zero-shot deployment given AP positions.

**Files modified:**
- `crates/wifi-densepose-train/src/geometry.rs` (new: GeometryEncoder, FourierPositionalEncoding, DeepSets, FiLM)
- `crates/wifi-densepose-train/src/graph_transformer.rs` (inject FiLM conditioning before xyz_head)
- `crates/wifi-densepose-train/src/config.rs` (add geometry fields to TrainConfig)

**Acceptance criteria:**
- [ ] FourierPositionalEncoding produces 64-dim vectors from 3D coordinates
- [ ] DeepSets is permutation-invariant (same output regardless of AP ordering)
- [ ] FiLM conditioning reduces cross-layout MPJPE by >30% vs unconditioned baseline
- [ ] Inference overhead <100us per frame (geometry encoding is amortized per-session)

### 4.4 Phase 4 -- Virtual Domain Augmentation (Week 4-5)

**Goal**: Synthetic environment diversity to improve generalization.

**Files modified:**
- `crates/wifi-densepose-train/src/virtual_aug.rs` (new: VirtualDomainAugmentor)
- `crates/wifi-densepose-train/src/trainer.rs` (integrate augmentor into training loop)
- `crates/wifi-densepose-signal/src/fresnel.rs` (reuse Fresnel zone model for scatterer simulation)

**Dependencies**: `ruvector-attn-mincut` (attention-weighted scatterer placement)

**Acceptance criteria:**
- [ ] Generate K=3 virtual domains per batch with <1ms overhead
- [ ] Virtual domains produce measurably different CSI statistics (KL divergence >0.1)
- [ ] Training with virtual augmentation improves unseen-environment accuracy by >15%
- [ ] No regression on seen-environment accuracy (within 2%)

### 4.5 Phase 5 -- Few-Shot Rapid Adaptation (Week 5-6)

**Goal**: 10-second calibration enables environment-specific fine-tuning without labels.

**Files modified:**
- `crates/wifi-densepose-train/src/rapid_adapt.rs` (new: RapidAdaptation)
- `crates/wifi-densepose-train/src/sona.rs` (extend SonaProfile with MERIDIAN fields)
- `crates/wifi-densepose-sensing-server/src/main.rs` (add `--calibrate` CLI flag)

**Acceptance criteria:**
- [ ] 200-frame (10 sec) calibration produces usable LoRA adapter
- [ ] Adapted model MPJPE within 15% of fully-supervised in-domain baseline
- [ ] Calibration completes in <5 seconds on x86 (including contrastive TTT)
- [ ] Adapted LoRA weights serializable to RVF container (ADR-023 Segment type)

### 4.6 Phase 6 -- Cross-Domain Evaluation Protocol (Week 6-7)

**Goal**: Rigorous multi-domain evaluation using MM-Fi's scene/subject splits.

**Files modified:**
- `crates/wifi-densepose-train/src/eval.rs` (new: CrossDomainEvaluator)
- `crates/wifi-densepose-train/src/dataset.rs` (add domain-split loading for MM-Fi)

**Evaluation protocol (following PerceptAlign):**

| Metric | Description |
|--------|-------------|
| **In-domain MPJPE** | Mean Per Joint Position Error on training environment |
| **Cross-domain MPJPE** | MPJPE on held-out environment (zero-shot) |
| **Few-shot MPJPE** | MPJPE after 10-sec calibration in target environment |
| **Cross-hardware MPJPE** | MPJPE when trained on one hardware, tested on another |
| **Domain gap ratio** | cross-domain / in-domain MPJPE (lower = better; target <1.5) |
| **Adaptation speedup** | Labeled samples saved vs training from scratch (target >5x) |

### 4.7 Phase 7 -- RVF Container + Deployment (Week 7-8)

**Goal**: Package MERIDIAN-enhanced models for edge deployment.

**Files modified:**
- `crates/wifi-densepose-train/src/rvf_container.rs` (add GEOM and DOMAIN segment types)
- `crates/wifi-densepose-sensing-server/src/inference.rs` (load geometry + domain weights)
- `crates/wifi-densepose-sensing-server/src/main.rs` (add `--ap-positions` CLI flag)

**New RVF segments:**

| Segment | Type ID | Contents | Size |
|---------|---------|----------|------|
| `GEOM` | `0x47454F4D` | GeometryEncoder weights + FiLM layers | ~4 KB |
| `DOMAIN` | `0x444F4D4E` | DomainFactorizer weights (PoseEncoder only; EnvEncoder and GRL discarded) | ~8 KB |
| `HWSTATS` | `0x48575354` | Per-hardware amplitude statistics for HardwareNormalizer | ~1 KB |

**CLI usage:**

```bash
# Train with MERIDIAN domain generalization
cargo run -p wifi-densepose-sensing-server -- \
  --train --dataset data/mmfi/ --epochs 100 \
  --meridian --n-virtual-domains 3 \
  --save-rvf model-meridian.rvf

# Deploy with geometry conditioning (zero-shot)
cargo run -p wifi-densepose-sensing-server -- \
  --model model-meridian.rvf \
  --ap-positions "0,0,2.5;3.5,0,2.5;1.75,4,2.5"

# Calibrate in new environment (few-shot, 10 seconds)
cargo run -p wifi-densepose-sensing-server -- \
  --model model-meridian.rvf --calibrate --calibrate-duration 10
```

---

## 5. Consequences

### 5.1 Positive

- **Deploy once, work everywhere**: A single MERIDIAN-trained model generalizes across rooms, buildings, and hardware without per-environment retraining
- **Reduced deployment cost**: Zero-shot mode requires only AP position input; few-shot mode needs 10 seconds of ambient WiFi data
- **AETHER synergy**: Domain-invariant embeddings (ADR-024) become environment-agnostic fingerprints, enabling cross-building room identification
- **Hardware freedom**: HardwareNormalizer unblocks mixed-fleet deployments (ESP32 in some rooms, Intel 5300 in others)
- **Competitive positioning**: No existing open-source WiFi pose system offers cross-environment generalization; MERIDIAN would be the first

### 5.2 Negative

- **Training complexity**: Multi-domain training requires CSI data from multiple environments. MM-Fi provides multiple scenes but PerceptAlign's 7-layout dataset is not yet public.
- **Hyperparameter sensitivity**: GRL lambda annealing schedule and adversarial balance require careful tuning; unstable training is possible if adversarial signal is too strong early.
- **Geometry input requirement**: Zero-shot mode requires users to input AP positions, which may not always be precisely known. Degradation under inaccurate geometry input needs characterization.
- **Parameter overhead**: +12K parameters increases total model from 55K to 67K (22% increase), still well within ESP32 budget but notable.

### 5.3 Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| GRL training instability | Medium | Training diverges | Lambda annealing schedule; gradient clipping at 1.0; fallback to non-adversarial training |
| Virtual augmentation unrealistic | Low | No generalization improvement | Validate augmented CSI against real cross-domain data distributions |
| Geometry encoder overfits to training layouts | Medium | Zero-shot fails on novel geometries | Augment geometry inputs during training (jitter AP positions by +/-0.5m) |
| MM-Fi scenes insufficient diversity | High | Limited evaluation validity | Supplement with synthetic data; target PerceptAlign dataset when released |

---

## 6. References

1. Chen, L., et al. (2026). "Breaking Coordinate Overfitting: Geometry-Aware WiFi Sensing for Cross-Layout 3D Pose Estimation." arXiv:2601.12252. https://arxiv.org/abs/2601.12252
2. Zhou, Y., et al. (2024). "AdaPose: Towards Cross-Site Device-Free Human Pose Estimation with Commodity WiFi." IEEE Internet of Things Journal. arXiv:2309.16964. https://arxiv.org/abs/2309.16964
3. Yan, K., et al. (2024). "Person-in-WiFi 3D: End-to-End Multi-Person 3D Pose Estimation with Wi-Fi." CVPR 2024, pp. 969-978. https://openaccess.thecvf.com/content/CVPR2024/html/Yan_Person-in-WiFi_3D_End-to-End_Multi-Person_3D_Pose_Estimation_with_Wi-Fi_CVPR_2024_paper.html
4. Zhou, R., et al. (2025). "DGSense: A Domain Generalization Framework for Wireless Sensing." arXiv:2502.08155. https://arxiv.org/abs/2502.08155
5. CAPC (2024). "Context-Aware Predictive Coding: A Representation Learning Framework for WiFi Sensing." IEEE OJCOMS, Vol. 5, pp. 6119-6134. arXiv:2410.01825. https://arxiv.org/abs/2410.01825
6. Chen, X. & Yang, J. (2025). "X-Fi: A Modality-Invariant Foundation Model for Multimodal Human Sensing." ICLR 2025. arXiv:2410.10167. https://arxiv.org/abs/2410.10167
7. AM-FM (2026). "AM-FM: A Foundation Model for Ambient Intelligence Through WiFi." arXiv:2602.11200. https://arxiv.org/abs/2602.11200
8. Ramesh, S. et al. (2025). "LatentCSI: High-resolution efficient image generation from WiFi CSI using a pretrained latent diffusion model." arXiv:2506.10605. https://arxiv.org/abs/2506.10605
9. Ganin, Y. et al. (2016). "Domain-Adversarial Training of Neural Networks." JMLR 17(59):1-35. https://jmlr.org/papers/v17/15-239.html
10. Perez, E. et al. (2018). "FiLM: Visual Reasoning with a General Conditioning Layer." AAAI 2018. arXiv:1709.07871. https://arxiv.org/abs/1709.07871
