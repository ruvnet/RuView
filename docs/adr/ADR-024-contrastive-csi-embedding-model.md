# ADR-024: Project AETHER -- Contrastive CSI Embedding Model via CsiToPoseTransformer Backbone

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-03-01 |
| **Deciders** | ruv |
| **Codename** | **AETHER** -- Ambient Electromagnetic Topology for Hierarchical Embedding and Recognition |
| **Relates to** | ADR-004 (HNSW Fingerprinting), ADR-005 (SONA Self-Learning), ADR-006 (GNN-Enhanced CSI), ADR-014 (SOTA Signal Processing), ADR-015 (Public Datasets), ADR-016 (RuVector Integration), ADR-023 (Trained DensePose Pipeline) |

---

## 1. Context

### 1.1 The Embedding Gap

WiFi CSI signals encode a rich manifold of environmental and human information: room geometry via multipath reflections, human body configuration via Fresnel zone perturbations, and temporal dynamics via Doppler-like subcarrier phase shifts. The CsiToPoseTransformer (ADR-023) already learns to decode this manifold into 17-keypoint body poses through cross-attention and GNN message passing, producing intermediate `body_part_features` of shape `[17 x d_model]` that implicitly represent the latent CSI state.

These representations are currently **task-coupled**: they exist only as transient activations during pose regression and are discarded after the `xyz_head` and `conf_head` produce keypoint predictions. There is no mechanism to:

1. **Extract and persist** these representations as reusable, queryable embedding vectors
2. **Compare** CSI observations via learned similarity ("is this the same room?" / "is this the same person?")
3. **Pretrain** the backbone in a self-supervised manner from unlabeled CSI streams -- the most abundant data source
4. **Transfer** learned representations across WiFi hardware, environments, or deployment sites
5. **Feed** semantically meaningful vectors into HNSW indices (ADR-004) instead of hand-crafted feature encodings

The gap between what the transformer *internally knows* and what the system *externally exposes* is the central problem AETHER addresses.

### 1.2 Why "AETHER"?

The name reflects the historical concept of the luminiferous aether -- the invisible medium through which electromagnetic waves were once theorized to propagate. In our context, WiFi signals propagate through physical space, and AETHER extracts a latent geometric understanding of that space from the signals themselves. The name captures three core ideas:

- **Ambient**: Works with the WiFi signals already present in any indoor environment
- **Electromagnetic Topology**: Captures the topological structure of multipath propagation
- **Hierarchical Embedding**: Produces embeddings at multiple semantic levels (environment, activity, person)

### 1.3 Why Contrastive, Not Generative?

We evaluated and rejected a generative "RuvLLM" approach. The GOAP analysis:

| Factor | Generative (Autoregressive) | Contrastive (AETHER) |
|--------|---------------------------|---------------------|
| **Domain fit** | CSI is 56 continuous floats at 20 Hz -- not a discrete token vocabulary. Autoregressive generation is architecturally mismatched. | Contrastive learning on continuous sensor data is the established SOTA (SimCLR, BYOL, VICReg, CAPC). |
| **Model size** | Generative transformers need millions of parameters for meaningful sequence modeling. | Reuses existing 28K-param CsiToPoseTransformer + 25K projection head = 53K total. |
| **Edge deployment** | Cannot run on ESP32 (240 MHz, 520 KB SRAM). | INT8-quantized 53K params = ~53 KB. 10% of ESP32 SRAM. |
| **Training data** | Requires massive CSI corpus for autoregressive pretraining to converge. | Self-supervised augmentations work with any CSI stream -- even minutes of data. |
| **Inference** | Autoregressive decoding is sequential; violates 20 Hz real-time constraint. | Single forward pass: <2 ms at INT8. |
| **Infrastructure** | New model architecture, tokenizer, trainer, quantizer, RVF packaging. | One new module (`embedding.rs`), one new loss term, one new RVF segment type. |
| **Collapse risk** | Mode collapse in generation manifests as repetitive outputs. | Embedding collapse is detectable (variance monitoring) and preventable (VICReg regularization). |

### 1.4 What Already Exists

| Component | File | Relevant API |
|-----------|------|-------------|
| **CsiToPoseTransformer** | `graph_transformer.rs` | `embed()` returns `[17 x d_model]` body_part_features (already exists) |
| **Linear layers** | `graph_transformer.rs` | `Linear::new()`, `flatten_into()`, `unflatten_from()` |
| **GnnStack** | `graph_transformer.rs` | 2-layer GCN on COCO skeleton with symmetric normalized adjacency |
| **CrossAttention** | `graph_transformer.rs` | 4-head scaled dot-product attention |
| **SONA** | `sona.rs` | `LoraAdapter`, `EwcRegularizer`, `EnvironmentDetector`, `SonaProfile` |
| **Trainer** | `trainer.rs` | 6-term composite loss, SGD+momentum, cosine LR, PCK/OKS metrics, checkpointing |
| **Sparse Inference** | `sparse_inference.rs` | INT8 symmetric/asymmetric quantization, FP16, neuron profiling, sparse forward |
| **RVF Container** | `rvf_container.rs` | Segment-based binary format: VEC, META, QUANT, WITNESS, PROFILE, MANIFEST |
| **Dataset Pipeline** | `dataset.rs` | MM-Fi (56 subcarriers, 17 COCO keypoints), Wi-Pose (resampled), unified DataPipeline |
| **HNSW Index** | `ruvector-core` | `VectorIndex` trait: `add()`, `search()`, `remove()`, cosine/L2/dot metrics |
| **Micro-HNSW** | `micro-hnsw-wasm` | `no_std` HNSW for WASM/edge: 16-dim, 32 vectors/core, LIF neurons, STDP |

### 1.5 SOTA Landscape (2024-2025)

Recent advances that directly inform AETHER's design:

- **IdentiFi** (2025): Contrastive learning for WiFi-based person identification using latent CSI representations. Demonstrates that contrastive pretraining in the signal domain produces identity-discriminative embeddings without requiring spatial position labels.
- **WhoFi** (2025): Transformer-based WiFi CSI encoding for person re-identification achieving 95.5% accuracy on NTU-Fi. Validates that transformer backbones learn re-identification-quality features from CSI.
- **CAPC** (2024): Context-Aware Predictive Coding for WiFi sensing -- integrates CPC and Barlow Twins to learn temporally and contextually consistent representations from unlabeled WiFi data.
- **SSL for WiFi HAR Survey** (2025, arXiv:2506.12052): Comprehensive evaluation of SimCLR, VICReg, Barlow Twins, and SimSiam on WiFi CSI for human activity recognition. VICReg achieves best downstream accuracy but requires careful hyperparameter tuning; SimCLR shows more stable training.
- **ContraWiMAE** (2024-2025): Masked autoencoder + contrastive pretraining for wireless channel representation learning, demonstrating that hybrid SSL objectives outperform pure contrastive or pure reconstructive approaches.
- **Wi-PER81** (2025): Benchmark dataset of 162K wireless packets for WiFi-based person re-identification using Siamese networks on signal amplitude heatmaps.

---

## 2. Decision

### 2.1 Architecture: Dual-Head Transformer with Contrastive Projection

Add a lightweight projection head that maps the GNN body-part features into a normalized embedding space while preserving the existing pose regression path:

```
CSI Frame(s) [n_pairs x n_subcarriers]
     |
     v
  csi_embed (Linear 56 -> d_model=64)           [EXISTING]
     |
     v
  CrossAttention (Q=keypoint_queries,            [EXISTING]
                   K,V=csi_embed)
     |
     v
  GnnStack (2-layer GCN, COCO skeleton)          [EXISTING]
     |
     +---> body_part_features [17 x 64]           [EXISTING, now exposed via embed()]
     |          |
     |          v
     |     GlobalMeanPool --> frame_feature [64]   [NEW: mean over 17 keypoints]
     |          |
     |          v
     |     ProjectionHead:                         [NEW]
     |       proj_1: Linear(64, 128) + BatchNorm1D(128) + ReLU
     |       proj_2: Linear(128, 128)
     |       L2-normalize
     |          |
     |          v
     |     z_csi [128-dim unit vector]             [NEW: contrastive embedding]
     |
     +---> xyz_head (Linear 64->3) + conf_head    [EXISTING: pose regression]
            --> keypoints [17 x (x,y,z,conf)]
```

**Key design choices:**

1. **2-layer MLP with BatchNorm**: Following SimCLR v2 findings that a deeper projection head with batch normalization improves downstream task performance. The projection head discards information not useful for the contrastive objective, keeping the backbone representations richer.

2. **128-dim output**: Standard in contrastive learning literature (SimCLR, MoCo, CLIP). Large enough for high-recall HNSW search, small enough for edge deployment. L2-normalized to the unit hypersphere for cosine similarity.

3. **BatchNorm1D in projection head**: Prevents representation collapse by maintaining feature variance across the batch dimension. Acts as an implicit contrastive mechanism (VICReg insight) -- decorrelates embedding dimensions.

4. **Shared backbone, independent heads**: The backbone (csi_embed, cross-attention, GNN) is shared between pose regression and embedding extraction. This enables multi-task training where contrastive and supervised signals co-regularize the backbone.

### 2.2 Mathematical Foundations

#### 2.2.1 InfoNCE Contrastive Loss

Given a batch of N CSI windows, each augmented twice to produce 2N views, the InfoNCE loss for positive pair (i, j) is:

```
L_InfoNCE(i, j) = -log(  exp(sim(z_i, z_j) / tau)  /  sum_{k != i} exp(sim(z_i, z_k) / tau)  )
```

where:
- `sim(u, v) = u^T v / (||u|| * ||v||)` is cosine similarity (= dot product for L2-normalized vectors)
- `tau` is the temperature hyperparameter controlling concentration
- The sum in the denominator runs over all 2N-1 views excluding i itself (including the positive j and 2N-2 negatives)

The symmetric NT-Xent loss averages over both directions of each positive pair:

```
L_NT-Xent = (1 / 2N) * sum_{k=1}^{N} [ L_InfoNCE(2k-1, 2k) + L_InfoNCE(2k, 2k-1) ]
```

**Temperature selection**: `tau = 0.07` (following SimCLR). Lower temperature sharpens the distribution, making the loss more sensitive to hard negatives. We use a learnable temperature initialized to 0.07 with a floor of 0.01.

#### 2.2.2 VICReg Regularization (Collapse Prevention)

Pure InfoNCE can collapse when batch sizes are small (common in CSI settings). We add VICReg regularization terms:

```
L_variance = (1/d) * sum_{j=1}^{d} max(0, gamma - sqrt(Var(z_j) + epsilon))

L_covariance = (1/d) * sum_{i != j} C(z)_{ij}^2

L_AETHER = alpha * L_NT-Xent + beta * L_variance + gamma_cov * L_covariance
```

where:
- `Var(z_j)` is the variance of embedding dimension j across the batch
- `C(z)` is the covariance matrix of embeddings in the batch
- `gamma = 1.0` is the target standard deviation per dimension
- `epsilon = 1e-4` prevents zero-variance gradients
- Default weights: `alpha = 1.0, beta = 25.0, gamma_cov = 1.0` (per VICReg paper)

The variance term prevents all embeddings from collapsing to a single point. The covariance term decorrelates dimensions, maximizing information content.

#### 2.2.3 CSI-Specific Augmentation Strategy

Each augmentation must preserve the identity of the CSI observation (same room, same person, same activity) while varying the irrelevant dimensions (noise, timing, hardware drift). All augmentations are **physically motivated** by WiFi signal propagation:

| Augmentation | Operation | Physical Motivation | Default Params |
|-------------|-----------|--------------------| --------------|
| **Temporal jitter** | Shift window start by `U(-J, +J)` frames | Clock synchronization offset between AP and client | `J = 3` frames |
| **Subcarrier masking** | Zero `p_mask` fraction of random subcarriers | Frequency-selective fading from narrowband interference | `p_mask ~ U(0.05, 0.20)` |
| **Gaussian noise** | Add `N(0, sigma)` to amplitude | Thermal noise at the receiver front-end | `sigma ~ U(0.01, 0.05)` |
| **Phase rotation** | Add `U(0, 2*pi)` uniform random offset per frame | Local oscillator phase drift and carrier frequency offset | per-frame |
| **Amplitude scaling** | Multiply by `U(s_lo, s_hi)` | Path loss variation from distance/obstruction changes | `s_lo=0.8, s_hi=1.2` |
| **Subcarrier permutation** | Randomly swap adjacent subcarrier pairs with probability `p_swap` | Subcarrier reordering artifacts in different WiFi chipsets | `p_swap = 0.1` |
| **Temporal crop** | Randomly drop `p_drop` fraction of frames from the window, then interpolate | Packet loss and variable CSI reporting rates | `p_drop ~ U(0.0, 0.15)` |

Each view applies 2-4 randomly selected augmentations composed sequentially. The composition is sampled per-view, ensuring the two views of the same CSI window differ.

#### 2.2.4 Cross-Modal Alignment (Optional Phase C)

When paired CSI + camera pose data is available (MM-Fi, Wi-Pose), align the CSI embedding space with pose semantics:

```
z_pose = L2_normalize(PoseEncoder(pose_keypoints_flat))

PoseEncoder: Linear(51, 128) -> ReLU -> Linear(128, 128)  [51 = 17 keypoints * 3 coords]

L_cross = (1/N) * sum_{k=1}^{N} [ -log( exp(sim(z_csi_k, z_pose_k) / tau) / sum_{j} exp(sim(z_csi_k, z_pose_j) / tau) ) ]

L_total = L_supervised_pose + lambda_c * L_contrastive + lambda_x * L_cross
```

This ensures that CSI embeddings of the same pose are close in embedding space, enabling pose retrieval from CSI queries.

### 2.3 Training Strategy: Three-Phase Pipeline

#### Phase A -- Self-Supervised Pretraining (No Labels)

```
Raw CSI Window W (any stream, any environment)
     |
     +---> Aug_1(W) ---> CsiToPoseTransformer.embed() ---> MeanPool ---> ProjectionHead ---> z_1
     |                                                                                         |
     |                                                                              L_AETHER(z_1, z_2)
     |                                                                                         |
     +---> Aug_2(W) ---> CsiToPoseTransformer.embed() ---> MeanPool ---> ProjectionHead ---> z_2
```

- **Optimizer**: SGD with momentum 0.9, weight decay 1e-4 (SGD preferred over Adam for contrastive learning per SimCLR)
- **LR schedule**: Warmup 10 epochs linear 0 -> 0.03, then cosine decay to 1e-5
- **Batch size**: 256 positive pairs (512 total views). Smaller batches (32-64) acceptable with VICReg regularization.
- **Epochs**: 100-200 (convergence monitored via embedding uniformity and alignment metrics)
- **Monitoring**: Track `alignment = E[||z_i - z_j||^2]` for positive pairs (should decrease) and `uniformity = log(E[exp(-2 * ||z_i - z_j||^2)])` over all pairs (should decrease, indicating uniform distribution on hypersphere)

#### Phase B -- Supervised Fine-Tuning (Labeled Data)

After pretraining, attach `xyz_head` and `conf_head` and fine-tune with the existing 6-term composite loss (ADR-023 Phase 4), optionally keeping the contrastive loss as a regularizer:

```
L_total = L_pose_composite + lambda_c * L_contrastive

lambda_c = 0.1 (contrastive acts as regularizer, not primary objective)
```

The pretrained backbone starts with representations that already understand CSI spatial structure, typically requiring 3-10x fewer labeled samples for equivalent pose accuracy.

#### Phase C -- Cross-Modal Alignment (Optional, requires paired data)

Adds `L_cross` to align CSI and pose embedding spaces. Only applicable when paired CSI + camera pose data is available (MM-Fi provides this).

### 2.4 HNSW Index Architecture

The 128-dim L2-normalized `z_csi` embeddings feed four specialized HNSW indices, each serving a distinct recognition task:

| Index | Source Embedding | Update Frequency | Distance Metric | M | ef_construction | Max Elements | Use Case |
|-------|-----------------|-----------------|-----------------|---|----------------|-------------|----------|
| `env_fingerprint` | Mean of `z_csi` over 10-second window (200 frames @ 20 Hz) | On environment change detection (SONA drift) | Cosine | 16 | 200 | 10K | Room/zone identification |
| `activity_pattern` | `z_csi` at activity transition boundaries (detected via embedding velocity) | Per detected activity segment | Cosine | 12 | 150 | 50K | Activity classification |
| `temporal_baseline` | `z_csi` during calibration period (first 60 seconds) | At deployment / recalibration | Cosine | 16 | 200 | 1K | Anomaly/intrusion detection |
| `person_track` | Per-person `z_csi` sequences (clustered by embedding trajectory) | Per confirmed detection | Cosine | 16 | 200 | 10K | Re-identification across sessions |

**Index operations:**

```rust
pub trait EmbeddingIndex {
    /// Insert an embedding with metadata
    fn insert(&mut self, embedding: &[f32; 128], metadata: EmbeddingMetadata) -> VectorId;

    /// Search for k nearest neighbors
    fn search(&self, query: &[f32; 128], k: usize) -> Vec<(VectorId, f32, EmbeddingMetadata)>;

    /// Remove stale entries older than `max_age`
    fn prune(&mut self, max_age: std::time::Duration) -> usize;

    /// Index statistics
    fn stats(&self) -> IndexStats;
}

pub struct EmbeddingMetadata {
    pub timestamp: u64,
    pub environment_id: Option<String>,
    pub person_id: Option<u32>,
    pub activity_label: Option<String>,
    pub confidence: f32,
    pub sona_profile: Option<String>,
}
```

**Anomaly detection** uses the `temporal_baseline` index: compute `d = 1 - cosine_sim(z_current, nearest_baseline)`. If `d > threshold_anomaly` (default 0.3) for `>= n_consecutive` frames (default 5), flag as anomaly. This catches intrusions, falls, and environmental changes without any task-specific model.

### 2.5 Integration with Existing Systems

#### 2.5.1 SONA Integration (ADR-005)

Each `SonaProfile` already represents an environment-specific adaptation. AETHER adds a compact environment descriptor:

```rust
pub struct SonaProfile {
    // ... existing fields ...

    /// AETHER: Mean embedding of calibration CSI in this environment.
    /// 128 floats = 512 bytes. Used for O(1) environment identification
    /// before loading the full LoRA profile.
    pub env_embedding: Option<[f32; 128]>,
}
```

**Environment switching workflow:**
1. Compute `z_csi` for incoming CSI
2. Compare against `env_embedding` of all known `SonaProfile`s (128-dim dot product, <1 us each)
3. If closest profile distance < threshold: load that profile's LoRA weights
4. If no profile is close: trigger SONA adaptation for new environment, store new `env_embedding`

This replaces the current `EnvironmentDetector` statistical drift test with a semantically-aware embedding comparison.

#### 2.5.2 RVF Container Extension (ADR-003)

Add a new segment type for embedding model configuration:

```rust
/// Embedding model configuration and projection head weights.
/// Segment type: SEG_EMBED = 0x0C
const SEG_EMBED: u8 = 0x0C;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModelConfig {
    /// Backbone feature dimension (input to projection head)
    pub d_model: usize,           // 64
    /// Embedding output dimension
    pub d_proj: usize,            // 128
    /// Whether to L2-normalize the output
    pub normalize: bool,          // true
    /// Pretraining method used
    pub pretrain_method: String,  // "simclr" | "vicreg" | "capc"
    /// Temperature for InfoNCE (if applicable)
    pub temperature: f32,         // 0.07
    /// Augmentations used during pretraining
    pub augmentations: Vec<String>,
    /// Number of pretraining epochs completed
    pub pretrain_epochs: usize,
    /// Alignment metric at end of pretraining
    pub alignment_score: f32,
    /// Uniformity metric at end of pretraining
    pub uniformity_score: f32,
}
```

The projection head weights (25K floats = 100 KB at FP32, 25 KB at INT8) are stored in the existing VEC segment alongside the transformer weights. The RVF manifest distinguishes model types:

```json
{
    "model_type": "aether-embedding",
    "backbone": "csi-to-pose-transformer",
    "embedding_dim": 128,
    "pose_capable": true,
    "pretrain_method": "simclr+vicreg"
}
```

#### 2.5.3 Sparse Inference Integration (ADR-023 Phase 6)

Embedding extraction benefits from the same INT8 quantization and sparse neuron pruning. **Critical validation**: cosine distance ordering must be preserved under quantization.

**Rank preservation metric:**

```
rho = SpearmanRank(ranking_fp32, ranking_int8)
```

where `ranking` is the order of k-nearest neighbors for a test query. Requirement: `rho > 0.95` for `k = 10`. If `rho < 0.95`, apply mixed-precision: backbone at INT8, projection head at FP16.

**Quantization budget:**

| Component | Parameters | FP32 | INT8 | FP16 |
|-----------|-----------|------|------|------|
| CsiToPoseTransformer backbone | ~28,000 | 112 KB | 28 KB | 56 KB |
| ProjectionHead (proj_1 + proj_2) | ~24,960 | 100 KB | 25 KB | 50 KB |
| PoseEncoder (cross-modal, optional) | ~7,040 | 28 KB | 7 KB | 14 KB |
| **Total (without PoseEncoder)** | **~53,000** | **212 KB** | **53 KB** | **106 KB** |
| **Total (with PoseEncoder)** | **~60,000** | **240 KB** | **60 KB** | **120 KB** |

ESP32 SRAM budget: 520 KB. Model at INT8: 53-60 KB = 10-12% of SRAM. Ample margin for activations, HNSW index, and runtime stack.

### 2.6 Concrete Module Additions

All new/modified files in `rust-port/wifi-densepose-rs/crates/wifi-densepose-sensing-server/src/`:

#### 2.6.1 `embedding.rs` (NEW, ~450 lines)

```rust
// ── Core types ──────────────────────────────────────────────────────

/// Configuration for the AETHER embedding system.
pub struct AetherConfig {
    pub d_model: usize,          // 64 (from TransformerConfig)
    pub d_proj: usize,           // 128
    pub temperature: f32,        // 0.07
    pub vicreg_alpha: f32,       // 1.0  (InfoNCE weight)
    pub vicreg_beta: f32,        // 25.0 (variance weight)
    pub vicreg_gamma: f32,       // 1.0  (covariance weight)
    pub variance_target: f32,    // 1.0
    pub n_augmentations: usize,  // 2-4 per view
}

/// 2-layer MLP projection head: Linear -> BN -> ReLU -> Linear -> L2-norm.
pub struct ProjectionHead {
    proj_1: Linear,       // d_model -> d_proj
    bn_running_mean: Vec<f32>,   // d_proj
    bn_running_var: Vec<f32>,    // d_proj
    bn_gamma: Vec<f32>,          // d_proj (learnable scale)
    bn_beta: Vec<f32>,           // d_proj (learnable shift)
    proj_2: Linear,       // d_proj -> d_proj
}

impl ProjectionHead {
    pub fn new(d_model: usize, d_proj: usize) -> Self;
    pub fn forward(&self, x: &[f32]) -> Vec<f32>;   // returns L2-normalized
    pub fn forward_train(&mut self, batch: &[Vec<f32>]) -> Vec<Vec<f32>>; // updates BN stats
    pub fn flatten_into(&self, out: &mut Vec<f32>);
    pub fn unflatten_from(data: &[f32], d_model: usize, d_proj: usize) -> (Self, usize);
    pub fn param_count(&self) -> usize;
}

/// CSI-specific data augmentation pipeline.
pub struct CsiAugmenter {
    rng: Rng64,
    config: AugmentConfig,
}

pub struct AugmentConfig {
    pub temporal_jitter_frames: usize,  // 3
    pub mask_ratio_range: (f32, f32),   // (0.05, 0.20)
    pub noise_sigma_range: (f32, f32),  // (0.01, 0.05)
    pub scale_range: (f32, f32),        // (0.8, 1.2)
    pub swap_prob: f32,                 // 0.1
    pub drop_ratio_range: (f32, f32),   // (0.0, 0.15)
}

impl CsiAugmenter {
    pub fn new(seed: u64) -> Self;
    pub fn augment(&mut self, csi_window: &[Vec<f32>]) -> Vec<Vec<f32>>;
}

/// InfoNCE loss with temperature scaling.
pub fn info_nce_loss(embeddings_a: &[Vec<f32>], embeddings_b: &[Vec<f32>], temperature: f32) -> f32;

/// VICReg variance loss: penalizes dimensions with std < target.
pub fn variance_loss(embeddings: &[Vec<f32>], target: f32) -> f32;

/// VICReg covariance loss: penalizes correlated dimensions.
pub fn covariance_loss(embeddings: &[Vec<f32>]) -> f32;

/// Combined AETHER loss = alpha * InfoNCE + beta * variance + gamma * covariance.
pub fn aether_loss(
    z_a: &[Vec<f32>], z_b: &[Vec<f32>],
    temperature: f32, alpha: f32, beta: f32, gamma: f32, var_target: f32,
) -> AetherLossComponents;

pub struct AetherLossComponents {
    pub total: f32,
    pub info_nce: f32,
    pub variance: f32,
    pub covariance: f32,
}

/// Full embedding extraction pipeline.
pub struct EmbeddingExtractor {
    transformer: CsiToPoseTransformer,
    projection: ProjectionHead,
    config: AetherConfig,
}

impl EmbeddingExtractor {
    pub fn new(transformer: CsiToPoseTransformer, config: AetherConfig) -> Self;

    /// Extract 128-dim L2-normalized embedding from CSI features.
    pub fn embed(&self, csi_features: &[Vec<f32>]) -> Vec<f32>;

    /// Extract both pose keypoints AND embedding in a single forward pass.
    pub fn forward_dual(&self, csi_features: &[Vec<f32>]) -> (PoseOutput, Vec<f32>);

    /// Flatten all weights (transformer + projection head).
    pub fn flatten_weights(&self) -> Vec<f32>;

    /// Unflatten all weights.
    pub fn unflatten_weights(&mut self, params: &[f32]) -> Result<(), String>;

    /// Total trainable parameters.
    pub fn param_count(&self) -> usize;
}

// ── Monitoring ──────────────────────────────────────────────────────

/// Alignment metric: mean L2 distance between positive pair embeddings.
pub fn alignment_metric(z_a: &[Vec<f32>], z_b: &[Vec<f32>]) -> f32;

/// Uniformity metric: log of average pairwise Gaussian kernel.
pub fn uniformity_metric(embeddings: &[Vec<f32>], t: f32) -> f32;
```

#### 2.6.2 `trainer.rs` (MODIFICATIONS)

```rust
// Add to LossComponents:
pub struct LossComponents {
    // ... existing 6 terms ...
    pub contrastive: f32,      // NEW: AETHER contrastive loss
}

// Add to LossWeights:
pub struct LossWeights {
    // ... existing 6 weights ...
    pub contrastive: f32,      // NEW: default 0.0 (disabled), set to 0.1 for joint training
}

// Add to TrainerConfig:
pub struct TrainerConfig {
    // ... existing fields ...
    pub contrastive_loss_weight: f32,  // NEW: 0.0 = no contrastive, 0.1 = regularizer
    pub aether_config: Option<AetherConfig>,  // NEW: None = no AETHER
}

// New method on Trainer:
impl Trainer {
    /// Self-supervised pretraining epoch using AETHER contrastive loss.
    /// No pose labels required -- only raw CSI windows.
    pub fn pretrain_epoch(
        &mut self,
        csi_windows: &[Vec<Vec<f32>>],
        augmenter: &mut CsiAugmenter,
    ) -> PretrainEpochStats;

    /// Full self-supervised pretraining loop.
    pub fn run_pretraining(
        &mut self,
        csi_windows: &[Vec<Vec<f32>>],
        n_epochs: usize,
    ) -> PretrainResult;
}

pub struct PretrainEpochStats {
    pub epoch: usize,
    pub loss: f32,
    pub info_nce: f32,
    pub variance: f32,
    pub covariance: f32,
    pub alignment: f32,
    pub uniformity: f32,
    pub lr: f32,
}

pub struct PretrainResult {
    pub best_epoch: usize,
    pub best_alignment: f32,
    pub best_uniformity: f32,
    pub history: Vec<PretrainEpochStats>,
    pub total_time_secs: f64,
}
```

#### 2.6.3 `rvf_container.rs` (MINOR ADDITION)

```rust
/// Embedding model configuration segment type.
const SEG_EMBED: u8 = 0x0C;

impl RvfBuilder {
    /// Add AETHER embedding model configuration.
    pub fn add_embedding_config(&mut self, config: &EmbeddingModelConfig) {
        let payload = serde_json::to_vec(config).unwrap_or_default();
        self.push_segment(SEG_EMBED, &payload);
    }
}

impl RvfReader {
    /// Parse and return the embedding model config, if present.
    pub fn embedding_config(&self) -> Option<EmbeddingModelConfig> {
        self.find_segment(SEG_EMBED)
            .and_then(|data| serde_json::from_slice(data).ok())
    }
}
```

#### 2.6.4 `graph_transformer.rs` (NO CHANGES NEEDED)

The `embed()` method already exists and returns `[17 x d_model]`. No modifications required.

### 2.7 Parameter Budget

| Component | Params | Breakdown | FP32 | INT8 |
|-----------|--------|-----------|------|------|
| `csi_embed` | 3,648 | 56*64 + 64 | 14.6 KB | 3.6 KB |
| `keypoint_queries` | 1,088 | 17*64 | 4.4 KB | 1.1 KB |
| `CrossAttention` (4-head) | 16,640 | 4*(64*64+64) | 66.6 KB | 16.6 KB |
| `GnnStack` (2 layers) | 8,320 | 2*(64*64+64) | 33.3 KB | 8.3 KB |
| `xyz_head` | 195 | 64*3 + 3 | 0.8 KB | 0.2 KB |
| `conf_head` | 65 | 64*1 + 1 | 0.3 KB | 0.1 KB |
| **Backbone subtotal** | **29,956** | | **119.8 KB** | **29.9 KB** |
| `proj_1` (Linear) | 8,320 | 64*128 + 128 | 33.3 KB | 8.3 KB |
| `bn_1` (gamma + beta) | 256 | 128 + 128 | 1.0 KB | 0.3 KB |
| `proj_2` (Linear) | 16,512 | 128*128 + 128 | 66.0 KB | 16.5 KB |
| **ProjectionHead subtotal** | **25,088** | | **100.4 KB** | **25.1 KB** |
| **AETHER Total** | **55,044** | | **220.2 KB** | **55.0 KB** |
| `PoseEncoder` (optional) | 7,040 | 51*128+128 + 128*128+128 | 28.2 KB | 7.0 KB |
| **Full system** | **62,084** | | **248.3 KB** | **62.1 KB** |

### 2.8 Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| Embedding extraction latency (FP32, x86) | < 1 ms | `BenchmarkRunner::benchmark_inference()` |
| Embedding extraction latency (INT8, ESP32) | < 2 ms | Hardware benchmark at 240 MHz |
| HNSW search latency (10K vectors, k=5) | < 0.5 ms | `ruvector-core` benchmark suite |
| Self-supervised pretrain convergence | < 200 epochs | Alignment/uniformity plateau detection |
| Room identification accuracy (5 rooms) | > 95% | k-NN on `env_fingerprint` index |
| Activity classification accuracy (6 activities) | > 85% | k-NN on `activity_pattern` index |
| Person re-identification mAP (5 subjects) | > 80% | Rank-1 on `person_track` index |
| Anomaly detection F1 | > 0.90 | Distance threshold on `temporal_baseline` |
| INT8 rank correlation vs FP32 | > 0.95 | Spearman over 1000 query-neighbor pairs |
| Model size at INT8 | < 65 KB | `param_count * 1 byte` |
| Training memory overhead | < 50 MB | Peak RSS during pretraining |

### 2.9 Edge Deployment Strategy

#### 2.9.1 ESP32 (via C/Rust cross-compilation)

- INT8 quantization mandatory (53 KB model + 20 KB activation buffer = 73 KB of 520 KB SRAM)
- `micro-hnsw-wasm` stores up to 32 reference embeddings per core (256 cores = 8K embeddings)
- Embedding extraction runs at 20 Hz (50 ms budget, target <2 ms)
- HNSW search adds <0.1 ms for 32-vector index
- Total pipeline: CSI capture (25 ms) + embedding (2 ms) + search (0.1 ms) = 27.1 ms < 50 ms budget

#### 2.9.2 WASM (browser/server)

- FP32 or FP16 model (size constraints are relaxed)
- `ruvector-core` HNSW index in full mode (up to 1M vectors)
- Web Worker for non-blocking inference
- REST API endpoint: `POST /api/v1/embedding/extract` (input: CSI frame, output: 128-dim vector)
- REST API endpoint: `POST /api/v1/embedding/search` (input: 128-dim vector, output: k nearest neighbors)
- WebSocket endpoint: `ws://.../embedding/stream` (streaming CSI -> streaming embeddings)

---

## 3. Implementation Phases

### Phase 1: Embedding Module (2-3 days)

**Files:**
- `embedding.rs` (NEW): `ProjectionHead`, `CsiAugmenter`, `EmbeddingExtractor`, loss functions, metrics
- `rvf_container.rs` (MODIFY): Add `SEG_EMBED`, `add_embedding_config()`, `embedding_config()`
- `lib.rs` (MODIFY): Add `pub mod embedding;`

**Deliverables:**
- `ProjectionHead` with `forward()`, `forward_train()`, `flatten_into()`, `unflatten_from()`
- `CsiAugmenter` with all 7 augmentation strategies
- `info_nce_loss()`, `variance_loss()`, `covariance_loss()`, `aether_loss()`
- `EmbeddingExtractor` with `embed()` and `forward_dual()`
- `alignment_metric()` and `uniformity_metric()`
- Unit tests: augmentation output shape, loss gradient direction, L2-normalization, projection head roundtrip
- **Lines**: ~450

### Phase 2: Self-Supervised Pretraining (1-2 days)

**Files:**
- `trainer.rs` (MODIFY): Add `pretrain_epoch()`, `run_pretraining()`, contrastive loss to composite
- `embedding.rs` (EXTEND): Add `PretrainEpochStats`, `PretrainResult`

**Deliverables:**
- `Trainer::pretrain_epoch()` running SimCLR+VICReg on raw CSI windows
- `Trainer::run_pretraining()` full loop with monitoring
- Contrastive weight in `LossComponents` and `LossWeights`
- Integration test: pretrain 10 epochs on synthetic CSI, verify alignment improves
- **Lines**: ~200 additions to `trainer.rs`

### Phase 3: HNSW Fingerprint Pipeline (2-3 days)

**Files:**
- `embedding.rs` (EXTEND): Add `EmbeddingIndex` trait, `EmbeddingMetadata`, index management
- `main.rs` or new `api_embedding.rs` (MODIFY/NEW): REST endpoints for embedding search

**Deliverables:**
- Four HNSW index types with insert/search/prune operations
- Environment switching via embedding comparison (replaces statistical drift)
- Anomaly detection via baseline distance threshold
- REST API: `/api/v1/embedding/extract`, `/api/v1/embedding/search`
- Integration with existing SONA `EnvironmentDetector`
- **Lines**: ~300

### Phase 4: Cross-Modal Alignment (1 day, optional)

**Files:**
- `embedding.rs` (EXTEND): Add `PoseEncoder`, `cross_modal_loss()`

**Deliverables:**
- `PoseEncoder`: Linear(51 -> 128) -> ReLU -> Linear(128 -> 128) -> L2-norm
- Cross-modal InfoNCE loss on paired CSI + pose data
- Evaluation script for pose retrieval from CSI query
- **Lines**: ~150

### Phase 5: Quantized Embedding Validation (1 day)

**Files:**
- `sparse_inference.rs` (EXTEND): Add `SpearmanRankCorrelation`, embedding-specific quantization tests
- `rvf_pipeline.rs` (MODIFY): Package AETHER model into RVF with SEG_EMBED

**Deliverables:**
- Spearman rank correlation test for INT8 vs FP32 embeddings
- Mixed-precision fallback (INT8 backbone + FP16 projection head)
- ESP32 latency benchmark target verification
- RVF packaging of complete AETHER model
- **Lines**: ~150

### Phase 6: Integration Testing & Benchmarks (1-2 days)

**Deliverables:**
- End-to-end test: CSI -> embed -> HNSW insert -> HNSW search -> verify nearest neighbor correctness
- Pretraining convergence benchmark on MM-Fi dataset
- Quantization rank preservation benchmark
- ESP32 simulation latency benchmark
- All performance targets verified

**Total estimated effort: 8-12 days**

---

## 4. Consequences

### Positive

- **Self-supervised pretraining from unlabeled CSI**: Any WiFi CSI stream (no cameras, no annotations) can pretrain the embedding backbone, radically reducing labeled data requirements. This is the single most impactful capability: WiFi signals are ubiquitous and free.
- **Reuses 100% of existing infrastructure**: No new model architecture -- extends the existing CsiToPoseTransformer with one module, one loss term, one RVF segment type.
- **HNSW-ready embeddings**: 128-dim L2-normalized vectors plug directly into the HNSW indices proposed in ADR-004, fulfilling that ADR's "vector encode" pipeline gap.
- **Multi-use embeddings**: Same model produces pose keypoints AND embedding vectors in a single forward pass. Two capabilities for the price of one inference.
- **Anomaly detection without task-specific models**: OOD CSI frames produce embeddings distant from the training distribution. Fall detection, intrusion detection, and environment change detection emerge as byproducts of the embedding space geometry.
- **Compact environment fingerprints**: 128-dim embedding (512 bytes) replaces ~448 KB `SonaProfile` for environment identification. 900x compression with better discriminative power.
- **Cross-environment transfer**: Contrastive pretraining on diverse environments produces features that capture environment-invariant body dynamics, enabling few-shot adaptation (5-10 labeled samples) to new spaces.
- **Edge-deployable**: 55 KB at INT8 fits ESP32 SRAM with 88% headroom. The entire embedding + search pipeline completes in <3 ms.
- **Privacy-preserving**: Embeddings are not invertible to raw CSI. The projection head's information bottleneck (17x64 -> 128) discards environment-specific details, making embeddings suitable for cross-site comparison without revealing room geometry.

### Negative

- **Embedding quality coupled to backbone**: Unlike a standalone embedding model, quality depends on the CsiToPoseTransformer. Mitigated by the projection head adding a task-specific non-linear transformation.
- **Augmentation sensitivity**: Self-supervised embedding quality depends on augmentation design. Too aggressive = collapsed embeddings; too mild = trivial invariances. Mitigated by VICReg variance regularization and monitoring via alignment/uniformity metrics.
- **Additional training phase**: Pretrain-then-finetune is longer than direct supervised training. Mitigated by: (a) pretraining is a one-time cost, (b) the resulting backbone converges faster on supervised tasks.
- **Cosine distance under quantization**: INT8 can distort relative distances, degrading HNSW recall. Mitigated by Spearman rank correlation test with FP16 fallback for the projection head.
- **BatchNorm in projection head**: Adds training/inference mode distinction (running stats vs batch stats). At inference, uses running mean/var accumulated during training. On-device, this is a fixed per-dimension scale+shift operation.

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Augmentations produce collapsed embeddings (all vectors identical) | Medium | High | VICReg variance term (`beta=25`) with per-dimension variance monitoring. Alert if `Var(z_j) < 0.1` for any j. Switch to BYOL (stop-gradient) if collapse persists. |
| INT8 quantization degrades HNSW recall below 90% | Low | Medium | Spearman `rho > 0.95` gate. Mixed-precision fallback: INT8 backbone + FP16 projection head (+25 KB). |
| Contrastive pretraining does not improve downstream pose accuracy | Low | Low | Pretraining is optional. Supervised-only training (ADR-023) remains the fallback path. Even if pose accuracy is unchanged, embeddings still enable fingerprinting/search. |
| Cross-modal alignment requires too much paired data for convergence | Medium | Low | Phase C is optional. Self-supervised CSI-only pretraining (Phase A) is the primary path. Cross-modal alignment is an enhancement, not a requirement. |
| Projection head overfits to pretraining augmentations | Low | Medium | Freeze projection head during supervised fine-tuning (only fine-tune backbone + pose heads). Alternatively, use stop-gradient on the projection head during joint training. |
| Embedding space is not discriminative enough for person re-identification | Medium | Medium | WhoFi (2025) demonstrates 95.5% accuracy with transformer CSI encoding. Our architecture is comparable. If insufficient, add a supervised contrastive loss with person labels during fine-tuning. |

---

## 5. Testing Strategy

### 5.1 Unit Tests (in `embedding.rs`)

```rust
#[cfg(test)]
mod tests {
    // ProjectionHead
    fn projection_head_output_is_128_dim();
    fn projection_head_output_is_l2_normalized();
    fn projection_head_zero_input_does_not_nan();
    fn projection_head_flatten_unflatten_roundtrip();
    fn projection_head_param_count_correct();

    // CsiAugmenter
    fn augmenter_output_same_shape_as_input();
    fn augmenter_two_views_differ();
    fn augmenter_deterministic_with_same_seed();
    fn temporal_jitter_shifts_window();
    fn subcarrier_masking_zeros_expected_fraction();
    fn gaussian_noise_changes_values();
    fn amplitude_scaling_within_range();

    // Loss functions
    fn info_nce_zero_for_identical_embeddings();
    fn info_nce_positive_for_different_embeddings();
    fn info_nce_decreases_with_closer_positives();
    fn variance_loss_zero_when_variance_at_target();
    fn variance_loss_positive_when_variance_below_target();
    fn covariance_loss_zero_for_uncorrelated_dims();
    fn aether_loss_finite_for_random_embeddings();

    // Metrics
    fn alignment_zero_for_identical_pairs();
    fn uniformity_decreases_with_uniform_distribution();

    // EmbeddingExtractor
    fn extractor_embed_output_shape();
    fn extractor_dual_forward_produces_both_outputs();
    fn extractor_flatten_unflatten_preserves_output();
}
```

### 5.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    // Pretraining
    fn pretrain_5_epochs_alignment_improves();
    fn pretrain_loss_is_finite_throughout();
    fn pretrain_embeddings_not_collapsed(); // variance > 0.5 per dim

    // Joint training
    fn joint_train_contrastive_plus_pose_loss_finite();
    fn joint_train_pose_accuracy_not_degraded();

    // RVF
    fn rvf_embed_config_round_trip();
    fn rvf_full_aether_model_package();

    // Quantization
    fn int8_embedding_rank_correlation_above_095();
    fn fp16_embedding_rank_correlation_above_099();
}
```

---

## 6. Phase 7: Deep RuVector Integration — MicroLoRA + EWC++ + Library Losses

**Status**: Required (promoted from Future Work after capability audit)

The RuVector v2.0.4 vendor crates provide 50+ attention mechanisms, contrastive losses, and optimization tools that Phases 1-6 do not use (0% utilization). Phase 7 integrates the highest-impact capabilities directly into the embedding pipeline.

### 6.1 MicroLoRA on ProjectionHead (Environment-Specific Embeddings)

Integrate `sona.rs::LoraAdapter` into `ProjectionHead` for environment-adaptive embedding projection with minimal parameters:

```rust
pub struct ProjectionHead {
    proj_1: Linear,                       // base weights (frozen after pretraining)
    proj_1_lora: Option<LoraAdapter>,     // rank-4 environment delta (NEW)
    // ... bn fields ...
    proj_2: Linear,                       // base weights (frozen)
    proj_2_lora: Option<LoraAdapter>,     // rank-4 environment delta (NEW)
}
```

**Parameter budget per environment:**
- `proj_1_lora`: rank 4 * (64 + 128) = **768 params**
- `proj_2_lora`: rank 4 * (128 + 128) = **1,024 params**
- **Total: 1,792 params/env** vs 24,832 full ProjectionHead = **93% reduction**

**Methods to add:**
- `ProjectionHead::with_lora(rank: usize)` — constructor with LoRA adapters
- `ProjectionHead::forward()` modified: `out = base_out + lora.forward(input)` when adapters present
- `ProjectionHead::merge_lora()` / `unmerge_lora()` — for fast environment switching
- `ProjectionHead::freeze_base()` — freeze base weights, train only LoRA
- `ProjectionHead::lora_params() -> Vec<f32>` — flatten only LoRA weights for checkpoint

**Environment switching workflow:**
1. Compute `z_csi` for incoming CSI
2. Compare against stored `env_embedding` of all known profiles (128-dim dot product, <1us)
3. If closest profile < threshold: `unmerge_lora(old)` then `merge_lora(new)`
4. If no profile close: start LoRA adaptation for new environment

**Effort**: ~120 lines in `embedding.rs`

### 6.2 EWC++ Consolidation for Pretrain-to-Finetune Transition

Apply `sona.rs::EwcRegularizer` to prevent catastrophic forgetting of contrastive structure during supervised fine-tuning:

```
Phase A (pretrain):   Train backbone + projection with InfoNCE + VICReg
                      ↓
Consolidation:        fisher = EwcRegularizer::compute_fisher(pretrained_params, contrastive_loss)
                      ewc.consolidate(pretrained_params)
                      ↓
Phase B (finetune):   L_total = L_pose + lambda * ewc.penalty(current_params)
                      grad += ewc.penalty_gradient(current_params)
```

**Implementation:**
- Add `embedding_ewc: Option<EwcRegularizer>` field to `Trainer`
- After `run_pretraining()` completes, call `ewc.compute_fisher()` on contrastive loss surface
- During `train_epoch()`, add `ewc.penalty(current_params)` to total loss
- Add `ewc.penalty_gradient(current_params)` to gradient computation
- Lambda default: 5000.0 (from SONA config), decays over fine-tuning epochs

**Effort**: ~80 lines in `trainer.rs`

### 6.3 EnvironmentDetector in Embedding Pipeline

Wire `sona.rs::EnvironmentDetector` into `EmbeddingExtractor` for real-time drift awareness:

```rust
pub struct EmbeddingExtractor {
    transformer: CsiToPoseTransformer,
    projection: ProjectionHead,
    config: AetherConfig,
    drift_detector: EnvironmentDetector,   // NEW
}
```

**Behavior:**
- `extract()` calls `drift_detector.update(csi_mean, csi_var)` on each frame
- When `drift_detected()` returns true:
  - New embeddings tagged `anomalous: true` in `FingerprintIndex`
  - Triggers LoRA adaptation on ProjectionHead (6.1)
  - Optionally pauses HNSW insertion until drift stabilizes
- `DriftInfo` exposed via REST: `GET /api/v1/embedding/drift`

**Effort**: ~60 lines across `embedding.rs`

### 6.4 Hard-Negative Mining for Contrastive Training

Add hard-negative mining to the contrastive loss for more efficient training:

```rust
pub struct HardNegativeMiner {
    pub ratio: f32,        // 0.5 = use top 50% hardest negatives
    pub warmup_epochs: usize, // 5 = use all negatives for first 5 epochs
}

impl HardNegativeMiner {
    /// Select top-K hardest negatives from similarity matrix.
    /// Hard negatives are non-matching pairs with highest cosine similarity
    /// (i.e., the model is most confused about them).
    pub fn mine(&self, sim_matrix: &[Vec<f32>], epoch: usize) -> Vec<(usize, usize)>;
}
```

Modify `info_nce_loss()` to accept optional miner:
- First `warmup_epochs`: use all negatives (standard InfoNCE)
- After warmup: use only top `ratio` hardest negatives per anchor
- Increases effective batch difficulty without increasing batch size

**Effort**: ~80 lines in `embedding.rs`

### 6.5 RVF SEG_EMBED with LoRA Profile Storage

Extend RVF container to store embedding model config AND per-environment LoRA deltas:

```rust
pub const SEG_EMBED: u8 = 0x0C;
pub const SEG_LORA: u8 = 0x0D;  // NEW: LoRA weight deltas

pub struct EmbeddingModelConfig {
    pub d_model: usize,
    pub d_proj: usize,
    pub normalize: bool,
    pub pretrain_method: String,
    pub temperature: f32,
    pub augmentations: Vec<String>,
    pub lora_rank: Option<usize>,     // Some(4) if MicroLoRA enabled
    pub ewc_lambda: Option<f32>,      // Some(5000.0) if EWC active
    pub hard_negative_ratio: Option<f32>,
}

impl RvfBuilder {
    pub fn add_embedding_config(&mut self, config: &EmbeddingModelConfig);
    pub fn add_lora_profile(&mut self, name: &str, lora_weights: &[f32]);
}

impl RvfReader {
    pub fn embedding_config(&self) -> Option<EmbeddingModelConfig>;
    pub fn lora_profile(&self, name: &str) -> Option<Vec<f32>>;
    pub fn lora_profiles(&self) -> Vec<String>;  // list all stored profiles
}
```

**Effort**: ~100 lines in `rvf_container.rs`

### Phase 7 Summary

| Sub-phase | What | New Params | Lines |
|-----------|------|-----------|-------|
| 7.1 MicroLoRA on ProjectionHead | Environment-specific embeddings | 1,792/env | ~120 |
| 7.2 EWC++ consolidation | Pretrain→finetune memory preservation | 0 (regularizer) | ~80 |
| 7.3 EnvironmentDetector integration | Drift-aware embedding extraction | 0 | ~60 |
| 7.4 Hard-negative mining | More efficient contrastive training | 0 | ~80 |
| 7.5 RVF SEG_EMBED + SEG_LORA | Full model + LoRA profile packaging | 0 | ~100 |
| **Total** | | **1,792/env** | **~440** |

## 7. Future Work

- **Masked Autoencoder pretraining (ContraWiMAE-style)**: Combine contrastive with masked reconstruction for richer pre-trained representations. Mask random subcarrier-time patches and reconstruct them, using the reconstruction loss as an additional pretraining signal.
- **Hyperbolic embeddings**: Use the `ruvector-hyperbolic-hnsw` crate to embed activities in Poincare ball space, capturing the natural hierarchy (locomotion > walking > shuffling).
- **Temporal contrastive loss**: Extend from single-frame InfoNCE to temporal CPC (Contrastive Predictive Coding), where the model predicts future CSI embeddings from past ones, capturing temporal dynamics.
- **Federated AETHER**: Train embeddings across multiple deployment sites without centralizing raw CSI data. Each site computes local gradient updates; a central server aggregates using FedAvg. Only embedding-space gradients cross site boundaries.
- **RuVector Advanced Attention**: Integrate `MoEAttention` for routing CSI frames to specialized embedding experts, `HyperbolicAttention` for hierarchical CSI structure, and `SheafAttention` for early-exit during embedding extraction.

---

## 7. References

### Contrastive Learning Foundations
- [SimCLR: A Simple Framework for Contrastive Learning of Visual Representations](https://arxiv.org/abs/2002.05709) (Chen et al., ICML 2020)
- [SimCLR v2: Big Self-Supervised Models are Strong Semi-Supervised Learners](https://arxiv.org/abs/2006.10029) (Chen et al., NeurIPS 2020)
- [MoCo v3: An Empirical Study of Training Self-Supervised Vision Transformers](https://arxiv.org/abs/2104.02057) (Chen et al., ICCV 2021)
- [BYOL: Bootstrap Your Own Latent](https://arxiv.org/abs/2006.07733) (Grill et al., NeurIPS 2020)
- [VICReg: Variance-Invariance-Covariance Regularization for Self-Supervised Learning](https://arxiv.org/abs/2105.04906) (Bardes et al., ICLR 2022)
- [DINO: Emerging Properties in Self-Supervised Vision Transformers](https://arxiv.org/abs/2104.14294) (Caron et al., ICCV 2021)
- [Barlow Twins: Self-Supervised Learning via Redundancy Reduction](https://arxiv.org/abs/2103.03230) (Zbontar et al., ICML 2021)
- [Understanding Contrastive Representation Learning through Alignment and Uniformity on the Hypersphere](https://arxiv.org/abs/2005.10242) (Wang & Isola, ICML 2020)
- [CLIP: Learning Transferable Visual Models From Natural Language Supervision](https://arxiv.org/abs/2103.00020) (Radford et al., ICML 2021)

### WiFi Sensing and CSI Embeddings
- [DensePose From WiFi](https://arxiv.org/abs/2301.00250) (Geng et al., CMU, 2023)
- [WhoFi: Deep Person Re-Identification via Wi-Fi Channel Signal Encoding](https://arxiv.org/abs/2507.12869) (2025)
- [IdentiFi: Self-Supervised WiFi-Based Identity Recognition in Multi-User Smart Environments](https://pmc.ncbi.nlm.nih.gov/articles/PMC12115556/) (2025)
- [Context-Aware Predictive Coding (CAPC): A Representation Learning Framework for WiFi Sensing](https://arxiv.org/abs/2410.01825) (2024)
- [A Tutorial-cum-Survey on Self-Supervised Learning for Wi-Fi Sensing](https://arxiv.org/abs/2506.12052) (2025)
- [Evaluating Self-Supervised Learning for WiFi CSI-Based Human Activity Recognition](https://dl.acm.org/doi/10.1145/3715130) (ACM TOSN, 2025)
- [Wi-Fi CSI Fingerprinting-Based Indoor Positioning Using Deep Learning and Vector Embedding](https://www.sciencedirect.com/science/article/abs/pii/S0957417424026691) (2024)
- [SelfHAR: Improving Human Activity Recognition through Self-training with Unlabeled Data](https://arxiv.org/abs/2102.06073) (2021)
- [WiFi CSI Contrastive Pre-training for Activity Recognition](https://doi.org/10.1145/3580305.3599383) (Wang et al., KDD 2023)
- [Wi-PER81: Benchmark Dataset for Radio Signal Image-based Person Re-Identification](https://www.nature.com/articles/s41597-025-05804-0) (Nature Sci Data, 2025)
- [SignFi: Sign Language Recognition Using WiFi](https://arxiv.org/abs/1806.04583) (Ma et al., 2018)

### Self-Supervised Learning for Time Series
- [Self-Supervised Contrastive Learning for Long-term Forecasting](https://openreview.net/forum?id=nBCuRzjqK7) (2024)
- [Resampling Augmentation for Time Series Contrastive Learning](https://arxiv.org/abs/2506.18587) (2025)
- [Diffusion Model-based Contrastive Learning for Human Activity Recognition](https://arxiv.org/abs/2408.05567) (2024)
- [Self-Supervised Contrastive Learning for 6G UM-MIMO THz Communications](https://rings.winslab.lids.mit.edu/wp-content/uploads/2024/06/MurUllSaqWin-ICC-06-2024.pdf) (ICC 2024)

### Internal ADRs
- ADR-003: RVF Cognitive Containers for CSI Data
- ADR-004: HNSW Vector Search for Signal Fingerprinting
- ADR-005: SONA Self-Learning for Pose Estimation
- ADR-006: GNN-Enhanced CSI Pattern Recognition
- ADR-014: SOTA Signal Processing Algorithms
- ADR-015: Public Dataset Training Strategy
- ADR-016: RuVector Integration for Training Pipeline
- ADR-023: Trained DensePose Model with RuVector Signal Intelligence Pipeline
