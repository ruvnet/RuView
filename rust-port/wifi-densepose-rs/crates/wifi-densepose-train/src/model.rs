//! End-to-end WiFi-DensePose model (tch-rs / LibTorch backend).
//!
//! Architecture (following CMU arXiv:2301.00250):
//!
//! ```text
//! amplitude [B, T*tx*rx, sub]  ─┐
//!                                ├─► ModalityTranslator ─► [B, 3, 48, 48]
//! phase     [B, T*tx*rx, sub]  ─┘        │
//!                                         ▼
//!                                  ResNet18-like backbone
//!                                         │
//!                              ┌──────────┴──────────┐
//!                              ▼                     ▼
//!                        KeypointHead          DensePoseHead
//!                      [B,17,H,W] heatmaps   [B,25,H,W] parts
//!                                             [B,48,H,W] UV
//! ```
//!
//! Sub-networks are instantiated once in [`WiFiDensePoseModel::new`] and
//! stored as struct fields so layer weights persist correctly across forward
//! passes.  A lazy `forward_impl` reconstruction approach is intentionally
//! avoided here.
//!
//! # No pre-trained weights
//!
//! Weights are initialised from scratch (Kaiming uniform, default from tch).
//! Pre-trained ImageNet weights are not loaded because network access is not
//! guaranteed during training runs.

use std::path::Path;
use tch::{nn, nn::Module, nn::ModuleT, Device, Kind, Tensor};

use ruvector_attn_mincut::attn_mincut;
use ruvector_attention::attention::ScaledDotProductAttention;
use ruvector_attention::traits::Attention;

use crate::config::TrainingConfig;
use crate::error::TrainError;

// ---------------------------------------------------------------------------
// Public output type
// ---------------------------------------------------------------------------

/// Outputs produced by a single forward pass of [`WiFiDensePoseModel`].
#[derive(Debug)]
pub struct ModelOutput {
    /// Keypoint heatmaps: `[B, 17, H, W]`.
    pub keypoints: Tensor,
    /// Body-part logits (24 parts + background): `[B, 25, H, W]`.
    pub part_logits: Tensor,
    /// UV surface coordinates (24 × 2 channels): `[B, 48, H, W]`.
    pub uv_coords: Tensor,
    /// Backbone feature map for cross-modal transfer loss: `[B, 256, H/4, W/4]`.
    pub features: Tensor,
}

// ---------------------------------------------------------------------------
// WiFiDensePoseModel
// ---------------------------------------------------------------------------

/// End-to-end WiFi-DensePose model.
///
/// Input CSI tensors have shape `[B, T * n_tx * n_rx, n_sub]`.
/// All sub-networks are built once at construction and stored as fields so
/// their parameters persist correctly across calls.
pub struct WiFiDensePoseModel {
    vs: nn::VarStore,
    translator: ModalityTranslator,
    backbone: Backbone,
    kp_head: KeypointHead,
    dp_head: DensePoseHead,
    /// Active training configuration.
    pub config: TrainingConfig,
}

impl WiFiDensePoseModel {
    /// Build a new model with randomly-initialised weights on `device`.
    ///
    /// Call `tch::manual_seed(seed)` before this for reproducibility.
    pub fn new(config: &TrainingConfig, device: Device) -> Self {
        let vs = nn::VarStore::new(device);
        let root = vs.root();

        // Compute the flattened CSI input size used by the modality translator.
        let n_ant = (config.window_frames
            * config.num_antennas_tx
            * config.num_antennas_rx) as i64;
        let n_sc = config.num_subcarriers as i64;
        let flat_csi = n_ant * n_sc;

        let num_parts = config.num_body_parts as i64;

        let translator =
            ModalityTranslator::new(&root / "translator", flat_csi, n_ant, n_sc);
        let backbone = Backbone::new(&root / "backbone", config.backbone_channels as i64);
        let kp_head = KeypointHead::new(
            &root / "kp_head",
            config.backbone_channels as i64,
            config.num_keypoints as i64,
        );
        let dp_head = DensePoseHead::new(
            &root / "dp_head",
            config.backbone_channels as i64,
            num_parts,
        );

        WiFiDensePoseModel {
            vs,
            translator,
            backbone,
            kp_head,
            dp_head,
            config: config.clone(),
        }
    }

    /// Forward pass in training mode (dropout / batch-norm in train mode).
    ///
    /// # Arguments
    ///
    /// - `amplitude`: `[B, T*n_tx*n_rx, n_sub]`
    /// - `phase`:     `[B, T*n_tx*n_rx, n_sub]`
    pub fn forward_t(&self, amplitude: &Tensor, phase: &Tensor) -> ModelOutput {
        self.forward_impl(amplitude, phase, true)
    }

    /// Forward pass without gradient tracking (inference mode).
    pub fn forward_inference(&self, amplitude: &Tensor, phase: &Tensor) -> ModelOutput {
        tch::no_grad(|| self.forward_impl(amplitude, phase, false))
    }

    /// Save model weights to a file (tch safetensors / .pt format).
    ///
    /// # Errors
    ///
    /// Returns [`TrainError::TrainingStep`] if the file cannot be written.
    pub fn save(&self, path: &Path) -> Result<(), TrainError> {
        self.vs
            .save(path)
            .map_err(|e| TrainError::training_step(format!("save failed: {e}")))
    }

    /// Load model weights from a file.
    ///
    /// # Errors
    ///
    /// Returns [`TrainError::TrainingStep`] if the file cannot be read or the
    /// weights are incompatible with this model's architecture.
    pub fn load(&mut self, path: &Path) -> Result<(), TrainError> {
        self.vs
            .load(path)
            .map_err(|e| TrainError::training_step(format!("load failed: {e}")))
    }

    /// Return a reference to the internal `VarStore` (e.g. to build an
    /// optimiser).
    pub fn varstore(&self) -> &nn::VarStore {
        &self.vs
    }

    /// Mutable access to the internal `VarStore`.
    pub fn varstore_mut(&mut self) -> &mut nn::VarStore {
        &mut self.vs
    }

    /// Alias for [`varstore`](Self::varstore) — matches the `var_store` naming
    /// convention used by the training loop.
    pub fn var_store(&self) -> &nn::VarStore {
        &self.vs
    }

    /// Alias for [`varstore_mut`](Self::varstore_mut).
    pub fn var_store_mut(&mut self) -> &mut nn::VarStore {
        &mut self.vs
    }

    /// Alias for [`forward_t`](Self::forward_t) kept for compatibility with
    /// the training-loop code.
    pub fn forward_train(&self, amplitude: &Tensor, phase: &Tensor) -> ModelOutput {
        self.forward_t(amplitude, phase)
    }

    /// Total number of trainable scalar parameters.
    pub fn num_parameters(&self) -> i64 {
        self.vs
            .trainable_variables()
            .iter()
            .map(|t| t.numel())
            .sum()
    }

    // ------------------------------------------------------------------
    // Internal implementation
    // ------------------------------------------------------------------

    fn forward_impl(&self, amplitude: &Tensor, phase: &Tensor, train: bool) -> ModelOutput {
        let cfg = &self.config;

        // ── Phase sanitization (differentiable, no learned params) ───────
        let clean_phase = phase_sanitize(phase);

        // ── Flatten antenna×time×subcarrier dimensions ───────────────────
        let batch = amplitude.size()[0];
        let flat_amp = amplitude.reshape([batch, -1]);
        let flat_phase = clean_phase.reshape([batch, -1]);

        // ── Modality translator: CSI → pseudo spatial image ──────────────
        // Output: [B, 3, 48, 48]
        let spatial = self.translator.forward_t(&flat_amp, &flat_phase, train);

        // ── ResNet-style backbone ─────────────────────────────────────────
        // Output: [B, backbone_channels, H', W']
        let features = self.backbone.forward_t(&spatial, train);

        // ── Keypoint head ─────────────────────────────────────────────────
        let hs = cfg.heatmap_size as i64;
        let keypoints = self.kp_head.forward_t(&features, hs, train);

        // ── DensePose head ────────────────────────────────────────────────
        let (part_logits, uv_coords) = self.dp_head.forward_t(&features, hs, train);

        ModelOutput {
            keypoints,
            part_logits,
            uv_coords,
            features,
        }
    }
}

// ---------------------------------------------------------------------------
// Phase sanitizer (no learned parameters)
// ---------------------------------------------------------------------------

/// Differentiable phase sanitization via subcarrier-differential method.
///
/// Computes first-order differences along the subcarrier axis to cancel
/// common-mode phase drift (carrier frequency offset, sampling offset).
///
/// Input:  `[B, T*n_ant, n_sub]`
/// Output: `[B, T*n_ant, n_sub]`  (zero-padded on the left)
fn phase_sanitize(phase: &Tensor) -> Tensor {
    let n_sub = phase.size()[2];
    if n_sub <= 1 {
        return phase.zeros_like();
    }

    // φ_clean[k] = φ[k] - φ[k-1] for k > 0; φ_clean[0] = 0
    let later = phase.slice(2, 1, n_sub, 1);
    let earlier = phase.slice(2, 0, n_sub - 1, 1);
    let diff = later - earlier;

    let zeros = Tensor::zeros(
        [phase.size()[0], phase.size()[1], 1],
        (Kind::Float, phase.device()),
    );
    Tensor::cat(&[zeros, diff], 2)
}

// ---------------------------------------------------------------------------
// ruvector attention helpers
// ---------------------------------------------------------------------------

/// Apply min-cut gated attention over the antenna-path dimension.
///
/// Treats each antenna path as a "token" and subcarriers as the feature
/// dimension. Uses `attn_mincut` to gate irrelevant antenna-pair correlations,
/// which is equivalent to automatic antenna selection.
///
/// # Arguments
///
/// - `x`: CSI tensor `[B, n_ant, n_sc]` — amplitude or phase
/// - `lambda`: min-cut threshold (0.3 = moderate pruning)
///
/// # Returns
///
/// Attended tensor `[B, n_ant, n_sc]` with irrelevant antenna paths suppressed.
fn apply_antenna_attention(x: &Tensor, lambda: f32) -> Tensor {
    let sizes = x.size();
    let n_ant = sizes[1];
    let n_sc = sizes[2];

    // Skip trivial cases where attention is a no-op.
    if n_ant <= 1 || n_sc <= 1 {
        return x.shallow_clone();
    }

    let b = sizes[0] as usize;
    let n_ant_usize = n_ant as usize;
    let n_sc_usize = n_sc as usize;

    let device = x.device();
    let kind = x.kind();

    // Process each batch element independently (attn_mincut operates on 2D inputs).
    let mut results: Vec<Tensor> = Vec::with_capacity(b);

    for bi in 0..b {
        // Extract [n_ant, n_sc] slice for this batch element.
        let xi = x.select(0, bi as i64); // [n_ant, n_sc]

        // Move to CPU and convert to f32 for the pure-Rust attention kernel.
        let flat: Vec<f32> =
            Vec::from(xi.to_kind(Kind::Float).to_device(Device::Cpu).contiguous());

        // Q = K = V = the antenna features (self-attention over antenna paths).
        let out = attn_mincut(
            &flat,        // q: [n_ant * n_sc]
            &flat,        // k: [n_ant * n_sc]
            &flat,        // v: [n_ant * n_sc]
            n_sc_usize,   // d: feature dim = n_sc subcarriers
            n_ant_usize,  // seq_len: number of antenna paths
            lambda,       // lambda: min-cut threshold
            1,            // tau: no temporal hysteresis (single-frame)
            1e-6,         // eps: numerical epsilon
        );

        let attended = Tensor::from_slice(&out.output)
            .reshape([n_ant, n_sc])
            .to_device(device)
            .to_kind(kind);

        results.push(attended);
    }

    Tensor::stack(&results, 0) // [B, n_ant, n_sc]
}

/// Apply scaled dot-product attention over spatial locations.
///
/// Input: `[B, C, H, W]` feature map — each spatial location (H×W) becomes a
/// token; C is the feature dimension. Captures long-range spatial dependencies
/// between antenna-footprint regions.
///
/// Returns `[B, C, H, W]` with spatial attention applied.
///
/// This function can be applied after backbone features when long-range spatial
/// context is needed. It is defined here for completeness and may be called
/// from head implementations or future backbone variants.
#[allow(dead_code)]
fn apply_spatial_attention(x: &Tensor) -> Tensor {
    let sizes = x.size();
    let (b, c, h, w) = (sizes[0], sizes[1], sizes[2], sizes[3]);
    let n_spatial = (h * w) as usize;
    let d = c as usize;

    let device = x.device();
    let kind = x.kind();

    let attn = ScaledDotProductAttention::new(d);

    let mut results: Vec<Tensor> = Vec::with_capacity(b as usize);

    for bi in 0..b {
        // Extract [C, H*W] and transpose to [H*W, C].
        let xi = x.select(0, bi).reshape([c, h * w]).transpose(0, 1); // [H*W, C]
        let flat: Vec<f32> =
            Vec::from(xi.to_kind(Kind::Float).to_device(Device::Cpu).contiguous());

        // Build token slices — one per spatial position.
        let tokens: Vec<&[f32]> = (0..n_spatial)
            .map(|i| &flat[i * d..(i + 1) * d])
            .collect();

        // For each spatial token as query, compute attended output.
        let mut out_flat = vec![0.0f32; n_spatial * d];
        for i in 0..n_spatial {
            let query = &flat[i * d..(i + 1) * d];
            match attn.compute(query, &tokens, &tokens) {
                Ok(attended) => {
                    out_flat[i * d..(i + 1) * d].copy_from_slice(&attended);
                }
                Err(_) => {
                    // Fallback: identity — keep original features unchanged.
                    out_flat[i * d..(i + 1) * d].copy_from_slice(query);
                }
            }
        }

        let out_tensor = Tensor::from_slice(&out_flat)
            .reshape([h * w, c])
            .transpose(0, 1) // [C, H*W]
            .reshape([c, h, w]) // [C, H, W]
            .to_device(device)
            .to_kind(kind);

        results.push(out_tensor);
    }

    Tensor::stack(&results, 0) // [B, C, H, W]
}

// ---------------------------------------------------------------------------
// Modality Translator
// ---------------------------------------------------------------------------

/// Translates flattened (amplitude, phase) CSI vectors into a pseudo-image.
///
/// ```text
/// amplitude [B, flat_csi] ─► attn_mincut ─► amp_fc1 ► relu ► amp_fc2 ► relu ─┐
///                                                                                ├─► fuse_fc ► reshape ► spatial_conv ► [B, 3, 48, 48]
/// phase     [B, flat_csi] ─► attn_mincut ─► ph_fc1  ► relu ► ph_fc2  ► relu ─┘
/// ```
///
/// The `attn_mincut` step performs self-attention over the antenna-path dimension
/// (`n_ant` tokens, each with `n_sc` subcarrier features) to gate out irrelevant
/// antenna-pair correlations before the FC fusion layers.
struct ModalityTranslator {
    amp_fc1: nn::Linear,
    amp_fc2: nn::Linear,
    ph_fc1: nn::Linear,
    ph_fc2: nn::Linear,
    fuse_fc: nn::Linear,
    // Spatial refinement conv layers
    sp_conv1: nn::Conv2D,
    sp_bn1: nn::BatchNorm,
    sp_conv2: nn::Conv2D,
    /// Number of antenna paths: T * n_tx * n_rx (used for attention reshape).
    n_ant: i64,
    /// Number of subcarriers per antenna path (used for attention reshape).
    n_sc: i64,
}

impl ModalityTranslator {
    fn new(vs: nn::Path, flat_csi: i64, n_ant: i64, n_sc: i64) -> Self {
        let amp_fc1 = nn::linear(&vs / "amp_fc1", flat_csi, 512, Default::default());
        let amp_fc2 = nn::linear(&vs / "amp_fc2", 512, 256, Default::default());
        let ph_fc1 = nn::linear(&vs / "ph_fc1", flat_csi, 512, Default::default());
        let ph_fc2 = nn::linear(&vs / "ph_fc2", 512, 256, Default::default());
        // Fuse 256+256 → 3*48*48
        let fuse_fc = nn::linear(&vs / "fuse_fc", 512, 3 * 48 * 48, Default::default());

        // Two conv layers that mix spatial information in the pseudo-image.
        let sp_conv1 = nn::conv2d(
            &vs / "sp_conv1",
            3,
            32,
            3,
            nn::ConvConfig {
                padding: 1,
                bias: false,
                ..Default::default()
            },
        );
        let sp_bn1 = nn::batch_norm2d(&vs / "sp_bn1", 32, Default::default());
        let sp_conv2 = nn::conv2d(
            &vs / "sp_conv2",
            32,
            3,
            3,
            nn::ConvConfig {
                padding: 1,
                ..Default::default()
            },
        );

        ModalityTranslator {
            amp_fc1,
            amp_fc2,
            ph_fc1,
            ph_fc2,
            fuse_fc,
            sp_conv1,
            sp_bn1,
            sp_conv2,
            n_ant,
            n_sc,
        }
    }

    fn forward_t(&self, amp: &Tensor, ph: &Tensor, train: bool) -> Tensor {
        let b = amp.size()[0];

        // === ruvector-attn-mincut: gate irrelevant antenna paths ===
        //
        // Reshape from [B, flat_csi] to [B, n_ant, n_sc], apply min-cut
        // self-attention over the antenna-path dimension (antenna paths are
        // "tokens", subcarrier responses are "features"), then flatten back.
        let amp_3d = amp.reshape([b, self.n_ant, self.n_sc]);
        let ph_3d = ph.reshape([b, self.n_ant, self.n_sc]);

        let amp_attended = apply_antenna_attention(&amp_3d, 0.3);
        let ph_attended = apply_antenna_attention(&ph_3d, 0.3);

        let amp_flat = amp_attended.reshape([b, -1]); // [B, flat_csi]
        let ph_flat = ph_attended.reshape([b, -1]); // [B, flat_csi]

        // Amplitude branch (uses attended input)
        let a = amp_flat
            .apply(&self.amp_fc1)
            .relu()
            .dropout(0.2, train)
            .apply(&self.amp_fc2)
            .relu();

        // Phase branch (uses attended input)
        let p = ph_flat
            .apply(&self.ph_fc1)
            .relu()
            .dropout(0.2, train)
            .apply(&self.ph_fc2)
            .relu();

        // Fuse and reshape to spatial map
        let fused = Tensor::cat(&[a, p], 1) // [B, 512]
            .apply(&self.fuse_fc) // [B, 3*48*48]
            .view([b, 3, 48, 48])
            .relu();

        // Spatial refinement
        let out = fused
            .apply(&self.sp_conv1)
            .apply_t(&self.sp_bn1, train)
            .relu()
            .apply(&self.sp_conv2)
            .tanh(); // bound to [-1, 1] before backbone

        out
    }
}

// ---------------------------------------------------------------------------
// Backbone
// ---------------------------------------------------------------------------

/// ResNet18-compatible backbone.
///
/// ```text
/// Input:  [B, 3, 48, 48]
/// Stem:   Conv2d(3→64, k=3, s=1, p=1) + BN + ReLU         → [B, 64, 48, 48]
/// Layer1: 2 × BasicBlock(64→64,   stride=1)                → [B, 64, 48, 48]
/// Layer2: 2 × BasicBlock(64→128,  stride=2)                → [B, 128, 24, 24]
/// Layer3: 2 × BasicBlock(128→256, stride=2)                → [B, 256, 12, 12]
/// Output: [B, out_channels, 12, 12]
/// ```
struct Backbone {
    stem_conv: nn::Conv2D,
    stem_bn: nn::BatchNorm,
    // Layer 1
    l1b1: BasicBlock,
    l1b2: BasicBlock,
    // Layer 2
    l2b1: BasicBlock,
    l2b2: BasicBlock,
    // Layer 3
    l3b1: BasicBlock,
    l3b2: BasicBlock,
}

impl Backbone {
    fn new(vs: nn::Path, out_channels: i64) -> Self {
        let stem_conv = nn::conv2d(
            &vs / "stem_conv",
            3,
            64,
            3,
            nn::ConvConfig {
                padding: 1,
                bias: false,
                ..Default::default()
            },
        );
        let stem_bn = nn::batch_norm2d(&vs / "stem_bn", 64, Default::default());

        Backbone {
            stem_conv,
            stem_bn,
            l1b1: BasicBlock::new(&vs / "l1b1", 64, 64, 1),
            l1b2: BasicBlock::new(&vs / "l1b2", 64, 64, 1),
            l2b1: BasicBlock::new(&vs / "l2b1", 64, 128, 2),
            l2b2: BasicBlock::new(&vs / "l2b2", 128, 128, 1),
            l3b1: BasicBlock::new(&vs / "l3b1", 128, out_channels, 2),
            l3b2: BasicBlock::new(&vs / "l3b2", out_channels, out_channels, 1),
        }
    }

    fn forward_t(&self, x: &Tensor, train: bool) -> Tensor {
        let x = self
            .stem_conv
            .forward(x)
            .apply_t(&self.stem_bn, train)
            .relu();
        let x = self.l1b1.forward_t(&x, train);
        let x = self.l1b2.forward_t(&x, train);
        let x = self.l2b1.forward_t(&x, train);
        let x = self.l2b2.forward_t(&x, train);
        let x = self.l3b1.forward_t(&x, train);
        self.l3b2.forward_t(&x, train)
    }
}

// ---------------------------------------------------------------------------
// BasicBlock
// ---------------------------------------------------------------------------

/// ResNet BasicBlock with optional projection shortcut.
///
/// ```text
/// x ── Conv2d(s) ── BN ── ReLU ── Conv2d(1) ── BN ──┐
///  │                                                   +── ReLU
///  └── (1×1 Conv+BN if in_ch≠out_ch or stride≠1) ───┘
/// ```
struct BasicBlock {
    conv1: nn::Conv2D,
    bn1: nn::BatchNorm,
    conv2: nn::Conv2D,
    bn2: nn::BatchNorm,
    downsample: Option<(nn::Conv2D, nn::BatchNorm)>,
}

impl BasicBlock {
    fn new(vs: nn::Path, in_ch: i64, out_ch: i64, stride: i64) -> Self {
        let conv1 = nn::conv2d(
            &vs / "conv1",
            in_ch,
            out_ch,
            3,
            nn::ConvConfig {
                stride,
                padding: 1,
                bias: false,
                ..Default::default()
            },
        );
        let bn1 = nn::batch_norm2d(&vs / "bn1", out_ch, Default::default());

        let conv2 = nn::conv2d(
            &vs / "conv2",
            out_ch,
            out_ch,
            3,
            nn::ConvConfig {
                padding: 1,
                bias: false,
                ..Default::default()
            },
        );
        let bn2 = nn::batch_norm2d(&vs / "bn2", out_ch, Default::default());

        let downsample = if in_ch != out_ch || stride != 1 {
            let ds_conv = nn::conv2d(
                &vs / "ds_conv",
                in_ch,
                out_ch,
                1,
                nn::ConvConfig {
                    stride,
                    bias: false,
                    ..Default::default()
                },
            );
            let ds_bn = nn::batch_norm2d(&vs / "ds_bn", out_ch, Default::default());
            Some((ds_conv, ds_bn))
        } else {
            None
        };

        BasicBlock {
            conv1,
            bn1,
            conv2,
            bn2,
            downsample,
        }
    }

    fn forward_t(&self, x: &Tensor, train: bool) -> Tensor {
        let residual = match &self.downsample {
            Some((ds_conv, ds_bn)) => ds_conv.forward(x).apply_t(ds_bn, train),
            None => x.shallow_clone(),
        };

        let out = self
            .conv1
            .forward(x)
            .apply_t(&self.bn1, train)
            .relu();
        let out = self.conv2.forward(&out).apply_t(&self.bn2, train);

        (out + residual).relu()
    }
}

// ---------------------------------------------------------------------------
// Keypoint Head
// ---------------------------------------------------------------------------

/// Predicts per-joint Gaussian heatmaps.
///
/// ```text
/// Input:  [B, in_channels, H', W']
/// ► Conv2d(in→256, 3×3, p=1) + BN + ReLU
/// ► Conv2d(256→128, 3×3, p=1) + BN + ReLU
/// ► Conv2d(128→num_keypoints, 1×1)
/// ► upsample_bilinear2d → [B, num_keypoints, heatmap_size, heatmap_size]
/// ```
struct KeypointHead {
    conv1: nn::Conv2D,
    bn1: nn::BatchNorm,
    conv2: nn::Conv2D,
    bn2: nn::BatchNorm,
    out_conv: nn::Conv2D,
}

impl KeypointHead {
    fn new(vs: nn::Path, in_ch: i64, num_kp: i64) -> Self {
        let conv1 = nn::conv2d(
            &vs / "conv1",
            in_ch,
            256,
            3,
            nn::ConvConfig {
                padding: 1,
                bias: false,
                ..Default::default()
            },
        );
        let bn1 = nn::batch_norm2d(&vs / "bn1", 256, Default::default());

        let conv2 = nn::conv2d(
            &vs / "conv2",
            256,
            128,
            3,
            nn::ConvConfig {
                padding: 1,
                bias: false,
                ..Default::default()
            },
        );
        let bn2 = nn::batch_norm2d(&vs / "bn2", 128, Default::default());

        let out_conv = nn::conv2d(&vs / "out_conv", 128, num_kp, 1, Default::default());

        KeypointHead {
            conv1,
            bn1,
            conv2,
            bn2,
            out_conv,
        }
    }

    fn forward_t(&self, x: &Tensor, heatmap_size: i64, train: bool) -> Tensor {
        let h = x
            .apply(&self.conv1)
            .apply_t(&self.bn1, train)
            .relu()
            .apply(&self.conv2)
            .apply_t(&self.bn2, train)
            .relu()
            .apply(&self.out_conv);

        h.upsample_bilinear2d(&[heatmap_size, heatmap_size], false, None, None)
    }
}

// ---------------------------------------------------------------------------
// DensePose Head
// ---------------------------------------------------------------------------

/// Predicts body-part segmentation and continuous UV surface coordinates.
///
/// ```text
/// Input: [B, in_channels, H', W']
///
/// Shared trunk:
///   ► Conv2d(in→256, 3×3, p=1) + BN + ReLU
///   ► Conv2d(256→256, 3×3, p=1) + BN + ReLU
///   ► upsample_bilinear2d → [B, 256, out_size, out_size]
///
/// Part branch:  Conv2d(256→num_parts+1, 1×1) → part logits
/// UV branch:    Conv2d(256→num_parts*2, 1×1) → sigmoid → UV ∈ [0,1]
/// ```
struct DensePoseHead {
    shared_conv1: nn::Conv2D,
    shared_bn1: nn::BatchNorm,
    shared_conv2: nn::Conv2D,
    shared_bn2: nn::BatchNorm,
    part_out: nn::Conv2D,
    uv_out: nn::Conv2D,
}

impl DensePoseHead {
    fn new(vs: nn::Path, in_ch: i64, num_parts: i64) -> Self {
        let shared_conv1 = nn::conv2d(
            &vs / "shared_conv1",
            in_ch,
            256,
            3,
            nn::ConvConfig {
                padding: 1,
                bias: false,
                ..Default::default()
            },
        );
        let shared_bn1 = nn::batch_norm2d(&vs / "shared_bn1", 256, Default::default());

        let shared_conv2 = nn::conv2d(
            &vs / "shared_conv2",
            256,
            256,
            3,
            nn::ConvConfig {
                padding: 1,
                bias: false,
                ..Default::default()
            },
        );
        let shared_bn2 = nn::batch_norm2d(&vs / "shared_bn2", 256, Default::default());

        // num_parts + 1: 24 body-part classes + 1 background class
        let part_out = nn::conv2d(
            &vs / "part_out",
            256,
            num_parts + 1,
            1,
            Default::default(),
        );
        // num_parts * 2: U and V channel for each of the 24 body parts
        let uv_out = nn::conv2d(
            &vs / "uv_out",
            256,
            num_parts * 2,
            1,
            Default::default(),
        );

        DensePoseHead {
            shared_conv1,
            shared_bn1,
            shared_conv2,
            shared_bn2,
            part_out,
            uv_out,
        }
    }

    /// Returns `(part_logits, uv_coords)`.
    fn forward_t(&self, x: &Tensor, out_size: i64, train: bool) -> (Tensor, Tensor) {
        let f = x
            .apply(&self.shared_conv1)
            .apply_t(&self.shared_bn1, train)
            .relu()
            .apply(&self.shared_conv2)
            .apply_t(&self.shared_bn2, train)
            .relu();

        // Upsample shared features to output resolution
        let f = f.upsample_bilinear2d(&[out_size, out_size], false, None, None);

        let parts = f.apply(&self.part_out);
        // Sigmoid constrains UV predictions to [0, 1]
        let uv = f.apply(&self.uv_out).sigmoid();

        (parts, uv)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TrainingConfig;
    use tch::Device;

    fn tiny_config() -> TrainingConfig {
        let mut cfg = TrainingConfig::default();
        cfg.num_subcarriers = 8;
        cfg.window_frames = 4;
        cfg.num_antennas_tx = 1;
        cfg.num_antennas_rx = 1;
        cfg.heatmap_size = 12;
        cfg.backbone_channels = 64;
        cfg.num_epochs = 2;
        cfg.warmup_epochs = 1;
        cfg
    }

    #[test]
    fn model_forward_output_shapes() {
        tch::manual_seed(0);
        let cfg = tiny_config();
        let device = Device::Cpu;
        let model = WiFiDensePoseModel::new(&cfg, device);

        let batch = 2_i64;
        let antennas =
            (cfg.num_antennas_tx * cfg.num_antennas_rx * cfg.window_frames) as i64;
        let n_sub = cfg.num_subcarriers as i64;

        let amp = Tensor::ones([batch, antennas, n_sub], (Kind::Float, device));
        let ph = Tensor::zeros([batch, antennas, n_sub], (Kind::Float, device));

        let out = model.forward_t(&amp, &ph);

        // Keypoints: [B, 17, heatmap_size, heatmap_size]
        assert_eq!(out.keypoints.size()[0], batch);
        assert_eq!(out.keypoints.size()[1], cfg.num_keypoints as i64);
        assert_eq!(out.keypoints.size()[2], cfg.heatmap_size as i64);
        assert_eq!(out.keypoints.size()[3], cfg.heatmap_size as i64);

        // Part logits: [B, num_body_parts+1, heatmap_size, heatmap_size]
        assert_eq!(out.part_logits.size()[0], batch);
        assert_eq!(out.part_logits.size()[1], (cfg.num_body_parts + 1) as i64);

        // UV: [B, num_body_parts*2, heatmap_size, heatmap_size]
        assert_eq!(out.uv_coords.size()[0], batch);
        assert_eq!(out.uv_coords.size()[1], (cfg.num_body_parts * 2) as i64);
    }

    #[test]
    fn model_has_nonzero_parameters() {
        tch::manual_seed(0);
        let cfg = tiny_config();
        let model = WiFiDensePoseModel::new(&cfg, Device::Cpu);
        let n = model.num_parameters();
        assert!(n > 0, "model must have trainable parameters");
    }

    #[test]
    fn inference_mode_gives_same_shapes() {
        tch::manual_seed(0);
        let cfg = tiny_config();
        let model = WiFiDensePoseModel::new(&cfg, Device::Cpu);

        let batch = 1_i64;
        let antennas =
            (cfg.num_antennas_tx * cfg.num_antennas_rx * cfg.window_frames) as i64;
        let n_sub = cfg.num_subcarriers as i64;
        let amp = Tensor::rand([batch, antennas, n_sub], (Kind::Float, Device::Cpu));
        let ph = Tensor::rand([batch, antennas, n_sub], (Kind::Float, Device::Cpu));

        let out = model.forward_inference(&amp, &ph);
        assert_eq!(out.keypoints.size()[0], batch);
        assert_eq!(out.part_logits.size()[0], batch);
        assert_eq!(out.uv_coords.size()[0], batch);
    }

    #[test]
    fn uv_coords_bounded_zero_one() {
        tch::manual_seed(0);
        let cfg = tiny_config();
        let model = WiFiDensePoseModel::new(&cfg, Device::Cpu);

        let batch = 2_i64;
        let antennas =
            (cfg.num_antennas_tx * cfg.num_antennas_rx * cfg.window_frames) as i64;
        let n_sub = cfg.num_subcarriers as i64;
        let amp = Tensor::rand([batch, antennas, n_sub], (Kind::Float, Device::Cpu));
        let ph = Tensor::rand([batch, antennas, n_sub], (Kind::Float, Device::Cpu));

        let out = model.forward_inference(&amp, &ph);

        let uv_min: f64 = out.uv_coords.min().double_value(&[]);
        let uv_max: f64 = out.uv_coords.max().double_value(&[]);
        assert!(
            uv_min >= 0.0 - 1e-5,
            "UV min should be >= 0, got {uv_min}"
        );
        assert!(
            uv_max <= 1.0 + 1e-5,
            "UV max should be <= 1, got {uv_max}"
        );
    }

    #[test]
    fn phase_sanitize_zeros_first_column() {
        let ph = Tensor::ones([2, 3, 8], (Kind::Float, Device::Cpu));
        let out = phase_sanitize(&ph);
        let first_col = out.slice(2, 0, 1, 1);
        let max_abs: f64 = first_col.abs().max().double_value(&[]);
        assert!(max_abs < 1e-6, "first diff column should be 0");
    }

    #[test]
    fn phase_sanitize_captures_ramp() {
        // φ[k] = k → diffs should all be 1.0 (except the padded zero)
        let ph = Tensor::arange(8, (Kind::Float, Device::Cpu))
            .reshape([1, 1, 8])
            .expand([2, 3, 8], true);
        let out = phase_sanitize(&ph);
        let tail = out.slice(2, 1, 8, 1);
        let min_val: f64 = tail.min().double_value(&[]);
        let max_val: f64 = tail.max().double_value(&[]);
        assert!(
            (min_val - 1.0).abs() < 1e-5,
            "expected 1.0 diff, got {min_val}"
        );
        assert!(
            (max_val - 1.0).abs() < 1e-5,
            "expected 1.0 diff, got {max_val}"
        );
    }

    #[test]
    fn save_and_load_roundtrip() {
        use tempfile::tempdir;

        tch::manual_seed(42);
        let cfg = tiny_config();
        let mut model = WiFiDensePoseModel::new(&cfg, Device::Cpu);

        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("weights.pt");

        model.save(&path).expect("save should succeed");
        model.load(&path).expect("load should succeed");

        // After loading, a forward pass should still work.
        let batch = 1_i64;
        let antennas =
            (cfg.num_antennas_tx * cfg.num_antennas_rx * cfg.window_frames) as i64;
        let n_sub = cfg.num_subcarriers as i64;
        let amp = Tensor::rand([batch, antennas, n_sub], (Kind::Float, Device::Cpu));
        let ph = Tensor::rand([batch, antennas, n_sub], (Kind::Float, Device::Cpu));
        let out = model.forward_inference(&amp, &ph);
        assert_eq!(out.keypoints.size()[0], batch);
    }

    #[test]
    fn varstore_accessible() {
        let cfg = tiny_config();
        let mut model = WiFiDensePoseModel::new(&cfg, Device::Cpu);
        // Both varstore() and varstore_mut() must compile and return the store.
        let _vs = model.varstore();
        let _vs_mut = model.varstore_mut();
    }
}
