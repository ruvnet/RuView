//! WiFi-DensePose end-to-end model using tch-rs (PyTorch Rust bindings).
//!
//! # Architecture
//!
//! ```text
//! CSI amplitude + phase
//!       │
//!       ▼
//! ┌─────────────────────┐
//! │  PhaseSanitizerNet  │  differentiable conjugate multiplication
//! └─────────────────────┘
//!       │
//!       ▼
//! ┌────────────────────────────┐
//! │  ModalityTranslatorNet     │  CSI → spatial pseudo-image [B, 3, 48, 48]
//! └────────────────────────────┘
//!       │
//!       ▼
//! ┌─────────────────┐
//! │  ResNet18-like  │  [B, 256, H/4, W/4] feature maps
//! │  Backbone       │
//! └─────────────────┘
//!       │
//!   ┌───┴───┐
//!   │       │
//!   ▼       ▼
//! ┌─────┐ ┌────────────┐
//! │ KP  │ │ DensePose  │
//! │ Head│ │ Head       │
//! └─────┘ └────────────┘
//! [B,17,H,W]  [B,25,H,W] + [B,48,H,W]
//! ```
//!
//! # No pre-trained weights
//!
//! The backbone uses a ResNet18-compatible architecture built purely with
//! `tch::nn`. Weights are initialised from scratch (Kaiming uniform by
//! default from tch).  Pre-trained ImageNet weights are not loaded because
//! network access is not guaranteed during training runs.

use std::path::Path;
use tch::{nn, nn::Module, nn::ModuleT, Device, Kind, Tensor};

use crate::config::TrainingConfig;

// ---------------------------------------------------------------------------
// Public output type
// ---------------------------------------------------------------------------

/// Outputs produced by a single forward pass of [`WiFiDensePoseModel`].
pub struct ModelOutput {
    /// Keypoint heatmaps: `[B, 17, H, W]`.
    pub keypoints: Tensor,
    /// Body-part logits (24 parts + background): `[B, 25, H, W]`.
    pub part_logits: Tensor,
    /// UV coordinates (24 × 2 channels interleaved): `[B, 48, H, W]`.
    pub uv_coords: Tensor,
    /// Backbone feature map used for cross-modal transfer loss: `[B, 256, H/4, W/4]`.
    pub features: Tensor,
}

// ---------------------------------------------------------------------------
// WiFiDensePoseModel
// ---------------------------------------------------------------------------

/// Complete WiFi-DensePose model.
///
/// Input: CSI amplitude and phase tensors with shape
/// `[B, T*n_tx*n_rx, n_sub]` (flattened antenna-time dimension).
///
/// Output: [`ModelOutput`] with keypoints and DensePose predictions.
pub struct WiFiDensePoseModel {
    vs: nn::VarStore,
    config: TrainingConfig,
}

// Internal model components stored in the VarStore.
// We use sub-paths inside the single VarStore to keep all parameters in
// one serialisable store.

impl WiFiDensePoseModel {
    /// Create a new model on `device`.
    ///
    /// All sub-networks are constructed and their parameters registered in the
    /// internal `VarStore`.
    pub fn new(config: &TrainingConfig, device: Device) -> Self {
        let vs = nn::VarStore::new(device);
        WiFiDensePoseModel {
            vs,
            config: config.clone(),
        }
    }

    /// Forward pass with gradient tracking (training mode).
    ///
    /// # Arguments
    ///
    /// - `amplitude`: `[B, T*n_tx*n_rx, n_sub]`
    /// - `phase`:     `[B, T*n_tx*n_rx, n_sub]`
    pub fn forward_train(&self, amplitude: &Tensor, phase: &Tensor) -> ModelOutput {
        self.forward_impl(amplitude, phase, true)
    }

    /// Forward pass without gradient tracking (inference mode).
    pub fn forward_inference(&self, amplitude: &Tensor, phase: &Tensor) -> ModelOutput {
        tch::no_grad(|| self.forward_impl(amplitude, phase, false))
    }

    /// Save model weights to `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        self.vs.save(path)?;
        Ok(())
    }

    /// Load model weights from `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or the weights are
    /// incompatible with the model architecture.
    pub fn load(
        path: &Path,
        config: &TrainingConfig,
        device: Device,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut model = Self::new(config, device);
        // Build parameter graph first so load can find named tensors.
        let _dummy_amp = Tensor::zeros(
            [1, 1, config.num_subcarriers as i64],
            (Kind::Float, device),
        );
        let _dummy_phase = _dummy_amp.shallow_clone();
        let _ = model.forward_impl(&_dummy_amp, &_dummy_phase, false);
        model.vs.load(path)?;
        Ok(model)
    }

    /// Return all trainable variable tensors.
    pub fn trainable_variables(&self) -> Vec<Tensor> {
        self.vs
            .trainable_variables()
            .into_iter()
            .map(|t| t.shallow_clone())
            .collect()
    }

    /// Count total trainable parameters.
    pub fn num_parameters(&self) -> usize {
        self.vs
            .trainable_variables()
            .iter()
            .map(|t| t.numel() as usize)
            .sum()
    }

    /// Access the internal `VarStore` (e.g. to create an optimizer).
    pub fn var_store(&self) -> &nn::VarStore {
        &self.vs
    }

    /// Mutable access to the internal `VarStore`.
    pub fn var_store_mut(&mut self) -> &mut nn::VarStore {
        &mut self.vs
    }

    // ------------------------------------------------------------------
    // Internal forward implementation
    // ------------------------------------------------------------------

    fn forward_impl(
        &self,
        amplitude: &Tensor,
        phase: &Tensor,
        train: bool,
    ) -> ModelOutput {
        let root = self.vs.root();
        let cfg = &self.config;

        // ── Phase sanitization ───────────────────────────────────────────
        let clean_phase = phase_sanitize(phase);

        // ── Modality translation ─────────────────────────────────────────
        // Flatten antenna-time and subcarrier dimensions → [B, flat]
        let batch = amplitude.size()[0];
        let flat_amp = amplitude.reshape([batch, -1]);
        let flat_phase = clean_phase.reshape([batch, -1]);
        let input_size = flat_amp.size()[1];

        let spatial = modality_translate(&root, &flat_amp, &flat_phase, input_size, train);
        // spatial: [B, 3, 48, 48]

        // ── ResNet18-like backbone ────────────────────────────────────────
        let (features, feat_h, feat_w) = resnet18_backbone(&root, &spatial, train, cfg.backbone_channels as i64);
        // features: [B, 256, 12, 12]

        // ── Keypoint head ────────────────────────────────────────────────
        let kp_h = cfg.heatmap_size as i64;
        let kp_w = kp_h;
        let keypoints = keypoint_head(&root, &features, cfg.num_keypoints as i64, (kp_h, kp_w), train);

        // ── DensePose head ───────────────────────────────────────────────
        let (part_logits, uv_coords) = densepose_head(
            &root,
            &features,
            (cfg.num_body_parts + 1) as i64,  // +1 for background
            (kp_h, kp_w),
            train,
        );

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

/// Differentiable phase sanitization via conjugate multiplication.
///
/// Implements the CSI ratio model: for each adjacent subcarrier pair, compute
/// the phase difference to cancel out common-mode phase drift (e.g. carrier
/// frequency offset, sampling offset).
///
/// Input:  `[B, T*n_ant, n_sub]`
/// Output: `[B, T*n_ant, n_sub]` (sanitized phase)
fn phase_sanitize(phase: &Tensor) -> Tensor {
    // For each subcarrier k, compute the differential phase:
    //   φ_clean[k] = φ[k] - φ[k-1]   for k > 0
    //   φ_clean[0] = 0
    //
    // This removes linear phase ramps caused by timing and CFO.
    // Implemented as: diff along last dimension with zero-padding on the left.

    let n_sub = phase.size()[2];
    if n_sub <= 1 {
        return phase.zeros_like();
    }

    // Slice k=1..N and k=0..N-1, compute difference.
    let later = phase.slice(2, 1, n_sub, 1);
    let earlier = phase.slice(2, 0, n_sub - 1, 1);
    let diff = later - earlier;

    // Prepend a zero column so the output has the same shape as input.
    let zeros = Tensor::zeros(
        [phase.size()[0], phase.size()[1], 1],
        (Kind::Float, phase.device()),
    );
    Tensor::cat(&[zeros, diff], 2)
}

// ---------------------------------------------------------------------------
// Modality translator
// ---------------------------------------------------------------------------

/// Build and run the modality translator network.
///
/// Architecture:
/// - Amplitude encoder: `Linear(input_size, 512) → ReLU → Linear(512, 256) → ReLU`
/// - Phase encoder:     same structure as amplitude encoder
/// - Fusion:            `Linear(512, 256) → ReLU → Linear(256, 48*48*3)`
///                       → reshape to `[B, 3, 48, 48]`
///
/// All layers share the same `root` VarStore path so weights accumulate
/// across calls (the parameters are created lazily on first call and reused).
fn modality_translate(
    root: &nn::Path,
    flat_amp: &Tensor,
    flat_phase: &Tensor,
    input_size: i64,
    train: bool,
) -> Tensor {
    let mt = root / "modality_translator";

    // Amplitude encoder
    let ae = |x: &Tensor| {
        let h = ((&mt / "amp_enc_fc1").linear(x, input_size, 512));
        let h = h.relu();
        let h = ((&mt / "amp_enc_fc2").linear(&h, 512, 256));
        h.relu()
    };

    // Phase encoder
    let pe = |x: &Tensor| {
        let h = ((&mt / "ph_enc_fc1").linear(x, input_size, 512));
        let h = h.relu();
        let h = ((&mt / "ph_enc_fc2").linear(&h, 512, 256));
        h.relu()
    };

    let amp_feat = ae(flat_amp);      // [B, 256]
    let phase_feat = pe(flat_phase);   // [B, 256]

    // Concatenate and fuse
    let fused = Tensor::cat(&[amp_feat, phase_feat], 1); // [B, 512]

    let spatial_out: i64 = 3 * 48 * 48;
    let fused = (&mt / "fusion_fc1").linear(&fused, 512, 256);
    let fused = fused.relu();
    let fused = (&mt / "fusion_fc2").linear(&fused, 256, spatial_out);
    // fused: [B, 3*48*48]

    let batch = fused.size()[0];
    let spatial_map = fused.reshape([batch, 3, 48, 48]);

    // Optional: apply tanh to bound activations before passing to CNN.
    spatial_map.tanh()
}

// ---------------------------------------------------------------------------
// Path::linear helper (creates or retrieves a Linear layer)
// ---------------------------------------------------------------------------

/// Extension trait to make `nn::Path` callable with `linear(x, in, out)`.
trait PathLinear {
    fn linear(&self, x: &Tensor, in_dim: i64, out_dim: i64) -> Tensor;
}

impl PathLinear for nn::Path<'_> {
    fn linear(&self, x: &Tensor, in_dim: i64, out_dim: i64) -> Tensor {
        let cfg = nn::LinearConfig::default();
        let layer = nn::linear(self, in_dim, out_dim, cfg);
        layer.forward(x)
    }
}

// ---------------------------------------------------------------------------
// ResNet18-like backbone
// ---------------------------------------------------------------------------

/// A ResNet18-style CNN backbone.
///
/// Input:  `[B, 3, 48, 48]`
/// Output: `[B, 256, 12, 12]` (spatial features)
///
/// Architecture:
/// - Stem:  Conv2d(3→64, k=3, s=1, p=1) + BN + ReLU
/// - Layer1: 2 × BasicBlock(64→64)
/// - Layer2: 2 × BasicBlock(64→128, stride=2)   → 24×24
/// - Layer3: 2 × BasicBlock(128→256, stride=2)  → 12×12
///
/// (No Layer4/pooling to preserve spatial resolution.)
fn resnet18_backbone(
    root: &nn::Path,
    x: &Tensor,
    train: bool,
    out_channels: i64,
) -> (Tensor, i64, i64) {
    let bb = root / "backbone";

    // Stem
    let stem_conv = nn::conv2d(
        &(&bb / "stem_conv"),
        3,
        64,
        3,
        nn::ConvConfig { padding: 1, ..Default::default() },
    );
    let stem_bn = nn::batch_norm2d(&(&bb / "stem_bn"), 64, Default::default());
    let x = stem_conv.forward(x).apply_t(&stem_bn, train).relu();

    // Layer 1: 64 → 64
    let x = basic_block(&(&bb / "l1b1"), &x, 64, 64, 1, train);
    let x = basic_block(&(&bb / "l1b2"), &x, 64, 64, 1, train);

    // Layer 2: 64 → 128 (stride 2 → half spatial)
    let x = basic_block(&(&bb / "l2b1"), &x, 64, 128, 2, train);
    let x = basic_block(&(&bb / "l2b2"), &x, 128, 128, 1, train);

    // Layer 3: 128 → out_channels (stride 2 → half spatial again)
    let x = basic_block(&(&bb / "l3b1"), &x, 128, out_channels, 2, train);
    let x = basic_block(&(&bb / "l3b2"), &x, out_channels, out_channels, 1, train);

    let shape = x.size();
    let h = shape[2];
    let w = shape[3];
    (x, h, w)
}

/// ResNet BasicBlock.
///
/// ```text
/// x ─── Conv2d(s) ─── BN ─── ReLU ─── Conv2d(1) ─── BN ──+── ReLU
///  │                                                         │
///  └── (downsample if needed) ──────────────────────────────┘
/// ```
fn basic_block(
    path: &nn::Path,
    x: &Tensor,
    in_ch: i64,
    out_ch: i64,
    stride: i64,
    train: bool,
) -> Tensor {
    let conv1 = nn::conv2d(
        &(path / "conv1"),
        in_ch,
        out_ch,
        3,
        nn::ConvConfig { stride, padding: 1, bias: false, ..Default::default() },
    );
    let bn1 = nn::batch_norm2d(&(path / "bn1"), out_ch, Default::default());

    let conv2 = nn::conv2d(
        &(path / "conv2"),
        out_ch,
        out_ch,
        3,
        nn::ConvConfig { padding: 1, bias: false, ..Default::default() },
    );
    let bn2 = nn::batch_norm2d(&(path / "bn2"), out_ch, Default::default());

    let out = conv1.forward(x).apply_t(&bn1, train).relu();
    let out = conv2.forward(&out).apply_t(&bn2, train);

    // Residual / skip connection
    let residual = if in_ch != out_ch || stride != 1 {
        let ds_conv = nn::conv2d(
            &(path / "ds_conv"),
            in_ch,
            out_ch,
            1,
            nn::ConvConfig { stride, bias: false, ..Default::default() },
        );
        let ds_bn = nn::batch_norm2d(&(path / "ds_bn"), out_ch, Default::default());
        ds_conv.forward(x).apply_t(&ds_bn, train)
    } else {
        x.shallow_clone()
    };

    (out + residual).relu()
}

// ---------------------------------------------------------------------------
// Keypoint head
// ---------------------------------------------------------------------------

/// Keypoint heatmap prediction head.
///
/// Input:  `[B, in_channels, H, W]`
/// Output: `[B, num_keypoints, out_h, out_w]` (after upsampling)
fn keypoint_head(
    root: &nn::Path,
    features: &Tensor,
    num_keypoints: i64,
    output_size: (i64, i64),
    train: bool,
) -> Tensor {
    let kp = root / "keypoint_head";

    let conv1 = nn::conv2d(
        &(&kp / "conv1"),
        256,
        256,
        3,
        nn::ConvConfig { padding: 1, bias: false, ..Default::default() },
    );
    let bn1 = nn::batch_norm2d(&(&kp / "bn1"), 256, Default::default());

    let conv2 = nn::conv2d(
        &(&kp / "conv2"),
        256,
        128,
        3,
        nn::ConvConfig { padding: 1, bias: false, ..Default::default() },
    );
    let bn2 = nn::batch_norm2d(&(&kp / "bn2"), 128, Default::default());

    let output_conv = nn::conv2d(
        &(&kp / "output_conv"),
        128,
        num_keypoints,
        1,
        Default::default(),
    );

    let x = conv1.forward(features).apply_t(&bn1, train).relu();
    let x = conv2.forward(&x).apply_t(&bn2, train).relu();
    let x = output_conv.forward(&x);

    // Upsample to (output_size_h, output_size_w)
    x.upsample_bilinear2d(
        [output_size.0, output_size.1],
        false,
        None,
        None,
    )
}

// ---------------------------------------------------------------------------
// DensePose head
// ---------------------------------------------------------------------------

/// DensePose prediction head.
///
/// Input:  `[B, in_channels, H, W]`
/// Outputs:
/// - part logits: `[B, num_parts, out_h, out_w]`
/// - UV coordinates: `[B, 2*(num_parts-1), out_h, out_w]`  (background excluded from UV)
fn densepose_head(
    root: &nn::Path,
    features: &Tensor,
    num_parts: i64,
    output_size: (i64, i64),
    train: bool,
) -> (Tensor, Tensor) {
    let dp = root / "densepose_head";

    // Shared convolutional block
    let shared_conv1 = nn::conv2d(
        &(&dp / "shared_conv1"),
        256,
        256,
        3,
        nn::ConvConfig { padding: 1, bias: false, ..Default::default() },
    );
    let shared_bn1 = nn::batch_norm2d(&(&dp / "shared_bn1"), 256, Default::default());

    let shared_conv2 = nn::conv2d(
        &(&dp / "shared_conv2"),
        256,
        256,
        3,
        nn::ConvConfig { padding: 1, bias: false, ..Default::default() },
    );
    let shared_bn2 = nn::batch_norm2d(&(&dp / "shared_bn2"), 256, Default::default());

    // Part segmentation head: 256 → num_parts
    let part_conv = nn::conv2d(
        &(&dp / "part_conv"),
        256,
        num_parts,
        1,
        Default::default(),
    );

    // UV regression head: 256 → 48 channels (2 × 24 body parts)
    let uv_conv = nn::conv2d(
        &(&dp / "uv_conv"),
        256,
        48, // 24 parts × 2 (U, V)
        1,
        Default::default(),
    );

    let shared = shared_conv1.forward(features).apply_t(&shared_bn1, train).relu();
    let shared = shared_conv2.forward(&shared).apply_t(&shared_bn2, train).relu();

    let parts = part_conv.forward(&shared);
    let uv = uv_conv.forward(&shared);

    // Upsample both heads to the target spatial resolution.
    let parts_up = parts.upsample_bilinear2d(
        [output_size.0, output_size.1],
        false,
        None,
        None,
    );
    let uv_up = uv.upsample_bilinear2d(
        [output_size.0, output_size.1],
        false,
        None,
        None,
    );

    // Apply sigmoid to UV to constrain predictions to [0, 1].
    let uv_out = uv_up.sigmoid();

    (parts_up, uv_out)
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
        let antennas = (cfg.num_antennas_tx * cfg.num_antennas_rx * cfg.window_frames) as i64;
        let n_sub = cfg.num_subcarriers as i64;

        let amp = Tensor::ones([batch, antennas, n_sub], (Kind::Float, device));
        let ph = Tensor::zeros([batch, antennas, n_sub], (Kind::Float, device));

        let out = model.forward_train(&amp, &ph);

        // Keypoints: [B, 17, heatmap_size, heatmap_size]
        assert_eq!(out.keypoints.size()[0], batch);
        assert_eq!(out.keypoints.size()[1], cfg.num_keypoints as i64);

        // Part logits: [B, 25, heatmap_size, heatmap_size]
        assert_eq!(out.part_logits.size()[0], batch);
        assert_eq!(out.part_logits.size()[1], (cfg.num_body_parts + 1) as i64);

        // UV: [B, 48, heatmap_size, heatmap_size]
        assert_eq!(out.uv_coords.size()[0], batch);
        assert_eq!(out.uv_coords.size()[1], 48);
    }

    #[test]
    fn model_has_nonzero_parameters() {
        tch::manual_seed(0);
        let cfg = tiny_config();
        let model = WiFiDensePoseModel::new(&cfg, Device::Cpu);

        // Trigger parameter creation by running a forward pass.
        let batch = 1_i64;
        let antennas = (cfg.num_antennas_tx * cfg.num_antennas_rx * cfg.window_frames) as i64;
        let n_sub = cfg.num_subcarriers as i64;
        let amp = Tensor::zeros([batch, antennas, n_sub], (Kind::Float, Device::Cpu));
        let ph = amp.shallow_clone();
        let _ = model.forward_train(&amp, &ph);

        let n = model.num_parameters();
        assert!(n > 0, "Model must have trainable parameters");
    }

    #[test]
    fn phase_sanitize_zeros_first_column() {
        let ph = Tensor::ones([2, 3, 8], (Kind::Float, Device::Cpu));
        let out = phase_sanitize(&ph);
        // First subcarrier column should be 0.
        let first_col = out.slice(2, 0, 1, 1);
        let max_abs: f64 = first_col.abs().max().double_value(&[]);
        assert!(max_abs < 1e-6, "First diff column should be 0");
    }

    #[test]
    fn phase_sanitize_captures_ramp() {
        // A linear phase ramp φ[k] = k should produce constant diffs of 1.
        let ph = Tensor::arange(8, (Kind::Float, Device::Cpu))
            .reshape([1, 1, 8])
            .expand([2, 3, 8], true);
        let out = phase_sanitize(&ph);
        // All columns except the first should be 1.0
        let tail = out.slice(2, 1, 8, 1);
        let min_val: f64 = tail.min().double_value(&[]);
        let max_val: f64 = tail.max().double_value(&[]);
        assert!((min_val - 1.0).abs() < 1e-5, "Expected 1.0 diff, got {min_val}");
        assert!((max_val - 1.0).abs() < 1e-5, "Expected 1.0 diff, got {max_val}");
    }

    #[test]
    fn inference_mode_gives_same_shapes() {
        tch::manual_seed(0);
        let cfg = tiny_config();
        let model = WiFiDensePoseModel::new(&cfg, Device::Cpu);

        let batch = 1_i64;
        let antennas = (cfg.num_antennas_tx * cfg.num_antennas_rx * cfg.window_frames) as i64;
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
        let antennas = (cfg.num_antennas_tx * cfg.num_antennas_rx * cfg.window_frames) as i64;
        let n_sub = cfg.num_subcarriers as i64;
        let amp = Tensor::rand([batch, antennas, n_sub], (Kind::Float, Device::Cpu));
        let ph = Tensor::rand([batch, antennas, n_sub], (Kind::Float, Device::Cpu));

        let out = model.forward_inference(&amp, &ph);

        let uv_min: f64 = out.uv_coords.min().double_value(&[]);
        let uv_max: f64 = out.uv_coords.max().double_value(&[]);
        assert!(uv_min >= 0.0 - 1e-5, "UV min should be >= 0, got {uv_min}");
        assert!(uv_max <= 1.0 + 1e-5, "UV max should be <= 1, got {uv_max}");
    }
}
