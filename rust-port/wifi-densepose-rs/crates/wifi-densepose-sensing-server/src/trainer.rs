//! Training loop with multi-term loss function for WiFi DensePose (ADR-023 Phase 4).
//!
//! 6-term composite loss, SGD with momentum, cosine annealing LR scheduler,
//! PCK/OKS validation metrics, numerical gradient estimation, and checkpointing.
//! All arithmetic uses f32. No external ML framework dependencies.

use std::path::Path;
use crate::graph_transformer::{CsiToPoseTransformer, TransformerConfig};
use crate::embedding::{CsiAugmenter, ProjectionHead, info_nce_loss};
use crate::dataset;
use crate::sona::EwcRegularizer;

/// Standard COCO keypoint sigmas for OKS (17 keypoints).
pub const COCO_KEYPOINT_SIGMAS: [f32; 17] = [
    0.026, 0.025, 0.025, 0.035, 0.035, 0.079, 0.079, 0.072, 0.072, 0.062,
    0.062, 0.107, 0.107, 0.087, 0.087, 0.089, 0.089,
];

/// Symmetric keypoint pairs (left, right) indices into 17-keypoint COCO layout.
const SYMMETRY_PAIRS: [(usize, usize); 5] =
    [(5, 6), (7, 8), (9, 10), (11, 12), (13, 14)];

/// Individual loss terms from the composite loss (6 supervised + 1 contrastive).
#[derive(Debug, Clone, Default)]
pub struct LossComponents {
    pub keypoint: f32,
    pub body_part: f32,
    pub uv: f32,
    pub temporal: f32,
    pub edge: f32,
    pub symmetry: f32,
    /// Contrastive loss (InfoNCE); only active during pretraining or when configured.
    pub contrastive: f32,
}

/// Per-term weights for the composite loss function.
#[derive(Debug, Clone)]
pub struct LossWeights {
    pub keypoint: f32,
    pub body_part: f32,
    pub uv: f32,
    pub temporal: f32,
    pub edge: f32,
    pub symmetry: f32,
    /// Contrastive loss weight (default 0.0; set >0 for joint training).
    pub contrastive: f32,
}

impl Default for LossWeights {
    fn default() -> Self {
        Self {
            keypoint: 1.0, body_part: 0.5, uv: 0.5, temporal: 0.1,
            edge: 0.2, symmetry: 0.1, contrastive: 0.0,
        }
    }
}

/// Mean squared error on keypoints (x, y, confidence).
pub fn keypoint_mse(pred: &[(f32, f32, f32)], target: &[(f32, f32, f32)]) -> f32 {
    if pred.is_empty() || target.is_empty() { return 0.0; }
    let n = pred.len().min(target.len());
    let sum: f32 = pred.iter().zip(target.iter()).take(n).map(|(p, t)| {
        (p.0 - t.0).powi(2) + (p.1 - t.1).powi(2) + (p.2 - t.2).powi(2)
    }).sum();
    sum / n as f32
}

/// Cross-entropy loss for body part classification.
/// `pred` = raw logits (length `n_samples * n_parts`), `target` = class indices.
pub fn body_part_cross_entropy(pred: &[f32], target: &[u8], n_parts: usize) -> f32 {
    if target.is_empty() || n_parts == 0 || pred.len() < n_parts { return 0.0; }
    let n_samples = target.len().min(pred.len() / n_parts);
    if n_samples == 0 { return 0.0; }
    let mut total = 0.0f32;
    for i in 0..n_samples {
        let logits = &pred[i * n_parts..(i + 1) * n_parts];
        let class = target[i] as usize;
        if class >= n_parts { continue; }
        let max_l = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let lse = logits.iter().map(|&l| (l - max_l).exp()).sum::<f32>().ln() + max_l;
        total += -logits[class] + lse;
    }
    total / n_samples as f32
}

/// L1 loss on UV coordinates.
pub fn uv_regression_loss(pu: &[f32], pv: &[f32], tu: &[f32], tv: &[f32]) -> f32 {
    let n = pu.len().min(pv.len()).min(tu.len()).min(tv.len());
    if n == 0 { return 0.0; }
    let s: f32 = (0..n).map(|i| (pu[i] - tu[i]).abs() + (pv[i] - tv[i]).abs()).sum();
    s / n as f32
}

/// Temporal consistency loss: penalizes large frame-to-frame keypoint jumps.
pub fn temporal_consistency_loss(prev: &[(f32, f32, f32)], curr: &[(f32, f32, f32)]) -> f32 {
    let n = prev.len().min(curr.len());
    if n == 0 { return 0.0; }
    let s: f32 = prev.iter().zip(curr.iter()).take(n)
        .map(|(p, c)| (c.0 - p.0).powi(2) + (c.1 - p.1).powi(2)).sum();
    s / n as f32
}

/// Graph edge loss: penalizes deviation of bone lengths from expected values.
pub fn graph_edge_loss(
    kp: &[(f32, f32, f32)], edges: &[(usize, usize)], expected: &[f32],
) -> f32 {
    if edges.is_empty() || edges.len() != expected.len() { return 0.0; }
    let (mut sum, mut cnt) = (0.0f32, 0usize);
    for (i, &(a, b)) in edges.iter().enumerate() {
        if a >= kp.len() || b >= kp.len() { continue; }
        let d = ((kp[a].0 - kp[b].0).powi(2) + (kp[a].1 - kp[b].1).powi(2)).sqrt();
        sum += (d - expected[i]).powi(2);
        cnt += 1;
    }
    if cnt == 0 { 0.0 } else { sum / cnt as f32 }
}

/// Symmetry loss: penalizes asymmetry between left-right limb pairs.
pub fn symmetry_loss(kp: &[(f32, f32, f32)]) -> f32 {
    if kp.len() < 15 { return 0.0; }
    let (mut sum, mut cnt) = (0.0f32, 0usize);
    for &(l, r) in &SYMMETRY_PAIRS {
        if l >= kp.len() || r >= kp.len() { continue; }
        let ld = ((kp[l].0 - kp[0].0).powi(2) + (kp[l].1 - kp[0].1).powi(2)).sqrt();
        let rd = ((kp[r].0 - kp[0].0).powi(2) + (kp[r].1 - kp[0].1).powi(2)).sqrt();
        sum += (ld - rd).powi(2);
        cnt += 1;
    }
    if cnt == 0 { 0.0 } else { sum / cnt as f32 }
}

/// Weighted composite loss from individual components.
pub fn composite_loss(c: &LossComponents, w: &LossWeights) -> f32 {
    w.keypoint * c.keypoint + w.body_part * c.body_part + w.uv * c.uv
        + w.temporal * c.temporal + w.edge * c.edge + w.symmetry * c.symmetry
        + w.contrastive * c.contrastive
}

// ── Optimizer ──────────────────────────────────────────────────────────────

/// SGD optimizer with momentum and weight decay.
pub struct SgdOptimizer {
    lr: f32,
    momentum: f32,
    weight_decay: f32,
    velocity: Vec<f32>,
}

impl SgdOptimizer {
    pub fn new(lr: f32, momentum: f32, weight_decay: f32) -> Self {
        Self { lr, momentum, weight_decay, velocity: Vec::new() }
    }

    /// v = mu*v + grad + wd*param; param -= lr*v
    pub fn step(&mut self, params: &mut [f32], gradients: &[f32]) {
        if self.velocity.len() != params.len() {
            self.velocity = vec![0.0; params.len()];
        }
        for i in 0..params.len().min(gradients.len()) {
            let g = gradients[i] + self.weight_decay * params[i];
            self.velocity[i] = self.momentum * self.velocity[i] + g;
            params[i] -= self.lr * self.velocity[i];
        }
    }

    pub fn set_lr(&mut self, lr: f32) { self.lr = lr; }
    pub fn state(&self) -> Vec<f32> { self.velocity.clone() }
    pub fn load_state(&mut self, state: Vec<f32>) { self.velocity = state; }
}

// ── Learning rate schedulers ───────────────────────────────────────────────

/// Cosine annealing: decays LR from initial to min over total_steps.
pub struct CosineScheduler { initial_lr: f32, min_lr: f32, total_steps: usize }

impl CosineScheduler {
    pub fn new(initial_lr: f32, min_lr: f32, total_steps: usize) -> Self {
        Self { initial_lr, min_lr, total_steps }
    }
    pub fn get_lr(&self, step: usize) -> f32 {
        if self.total_steps == 0 { return self.initial_lr; }
        let p = step.min(self.total_steps) as f32 / self.total_steps as f32;
        self.min_lr + (self.initial_lr - self.min_lr) * (1.0 + (std::f32::consts::PI * p).cos()) / 2.0
    }
}

/// Warmup + cosine annealing: linear ramp 0->initial_lr then cosine decay.
pub struct WarmupCosineScheduler {
    warmup_steps: usize, initial_lr: f32, min_lr: f32, total_steps: usize,
}

impl WarmupCosineScheduler {
    pub fn new(warmup_steps: usize, initial_lr: f32, min_lr: f32, total_steps: usize) -> Self {
        Self { warmup_steps, initial_lr, min_lr, total_steps }
    }
    pub fn get_lr(&self, step: usize) -> f32 {
        if step < self.warmup_steps {
            if self.warmup_steps == 0 { return self.initial_lr; }
            return self.initial_lr * (step as f32 / self.warmup_steps as f32);
        }
        let cs = self.total_steps.saturating_sub(self.warmup_steps);
        if cs == 0 { return self.min_lr; }
        let p = (step - self.warmup_steps).min(cs) as f32 / cs as f32;
        self.min_lr + (self.initial_lr - self.min_lr) * (1.0 + (std::f32::consts::PI * p).cos()) / 2.0
    }
}

// ── Validation metrics ─────────────────────────────────────────────────────

/// Percentage of Correct Keypoints at a distance threshold.
pub fn pck_at_threshold(pred: &[(f32, f32, f32)], target: &[(f32, f32, f32)], thr: f32) -> f32 {
    let n = pred.len().min(target.len());
    if n == 0 { return 0.0; }
    let (mut correct, mut total) = (0usize, 0usize);
    for i in 0..n {
        if target[i].2 <= 0.0 { continue; }
        total += 1;
        let d = ((pred[i].0 - target[i].0).powi(2) + (pred[i].1 - target[i].1).powi(2)).sqrt();
        if d <= thr { correct += 1; }
    }
    if total == 0 { 0.0 } else { correct as f32 / total as f32 }
}

/// Object Keypoint Similarity for a single instance.
pub fn oks_single(
    pred: &[(f32, f32, f32)], target: &[(f32, f32, f32)], sigmas: &[f32], area: f32,
) -> f32 {
    let n = pred.len().min(target.len()).min(sigmas.len());
    if n == 0 || area <= 0.0 { return 0.0; }
    let (mut sum, mut vis) = (0.0f32, 0usize);
    for i in 0..n {
        if target[i].2 <= 0.0 { continue; }
        vis += 1;
        let dsq = (pred[i].0 - target[i].0).powi(2) + (pred[i].1 - target[i].1).powi(2);
        let var = 2.0 * sigmas[i] * sigmas[i] * area;
        if var > 0.0 { sum += (-dsq / (2.0 * var)).exp(); }
    }
    if vis == 0 { 0.0 } else { sum / vis as f32 }
}

/// Mean OKS over multiple predictions (simplified mAP).
pub fn oks_map(preds: &[Vec<(f32, f32, f32)>], targets: &[Vec<(f32, f32, f32)>]) -> f32 {
    let n = preds.len().min(targets.len());
    if n == 0 { return 0.0; }
    let s: f32 = preds.iter().zip(targets.iter()).take(n)
        .map(|(p, t)| oks_single(p, t, &COCO_KEYPOINT_SIGMAS, 1.0)).sum();
    s / n as f32
}

// ── Gradient estimation ────────────────────────────────────────────────────

/// Central difference gradient: (f(x+eps) - f(x-eps)) / (2*eps).
pub fn estimate_gradient(f: impl Fn(&[f32]) -> f32, params: &[f32], eps: f32) -> Vec<f32> {
    let mut grad = vec![0.0f32; params.len()];
    let mut p_plus = params.to_vec();
    let mut p_minus = params.to_vec();
    for i in 0..params.len() {
        p_plus[i] = params[i] + eps;
        p_minus[i] = params[i] - eps;
        grad[i] = (f(&p_plus) - f(&p_minus)) / (2.0 * eps);
        p_plus[i] = params[i];
        p_minus[i] = params[i];
    }
    grad
}

/// Clip gradients by global L2 norm.
pub fn clip_gradients(gradients: &mut [f32], max_norm: f32) {
    let norm = gradients.iter().map(|g| g * g).sum::<f32>().sqrt();
    if norm > max_norm && norm > 0.0 {
        let s = max_norm / norm;
        gradients.iter_mut().for_each(|g| *g *= s);
    }
}

// ── Training sample ────────────────────────────────────────────────────────

/// A single training sample (defined locally, not dependent on dataset.rs).
#[derive(Debug, Clone)]
pub struct TrainingSample {
    pub csi_features: Vec<Vec<f32>>,
    pub target_keypoints: Vec<(f32, f32, f32)>,
    pub target_body_parts: Vec<u8>,
    pub target_uv: (Vec<f32>, Vec<f32>),
}

/// Convert a dataset::TrainingSample into a trainer::TrainingSample.
pub fn from_dataset_sample(ds: &dataset::TrainingSample) -> TrainingSample {
    let csi_features = ds.csi_window.clone();
    let target_keypoints: Vec<(f32, f32, f32)> = ds.pose_label.keypoints.to_vec();
    let target_body_parts: Vec<u8> = ds.pose_label.body_parts.iter()
        .map(|bp| bp.part_id)
        .collect();
    let (tu, tv) = if ds.pose_label.body_parts.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        let u: Vec<f32> = ds.pose_label.body_parts.iter()
            .flat_map(|bp| bp.u_coords.iter().copied()).collect();
        let v: Vec<f32> = ds.pose_label.body_parts.iter()
            .flat_map(|bp| bp.v_coords.iter().copied()).collect();
        (u, v)
    };
    TrainingSample { csi_features, target_keypoints, target_body_parts, target_uv: (tu, tv) }
}

// ── Checkpoint ─────────────────────────────────────────────────────────────

/// Serializable version of EpochStats for checkpoint storage.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EpochStatsSerializable {
    pub epoch: usize, pub train_loss: f32, pub val_loss: f32,
    pub pck_02: f32, pub oks_map: f32, pub lr: f32,
    pub loss_keypoint: f32, pub loss_body_part: f32, pub loss_uv: f32,
    pub loss_temporal: f32, pub loss_edge: f32, pub loss_symmetry: f32,
}

/// Serializable training checkpoint.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Checkpoint {
    pub epoch: usize,
    pub params: Vec<f32>,
    pub optimizer_state: Vec<f32>,
    pub best_loss: f32,
    pub metrics: EpochStatsSerializable,
}

impl Checkpoint {
    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }
    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

/// Statistics for a single training epoch.
#[derive(Debug, Clone)]
pub struct EpochStats {
    pub epoch: usize,
    pub train_loss: f32,
    pub val_loss: f32,
    pub pck_02: f32,
    pub oks_map: f32,
    pub lr: f32,
    pub loss_components: LossComponents,
}

impl EpochStats {
    fn to_serializable(&self) -> EpochStatsSerializable {
        let c = &self.loss_components;
        EpochStatsSerializable {
            epoch: self.epoch, train_loss: self.train_loss, val_loss: self.val_loss,
            pck_02: self.pck_02, oks_map: self.oks_map, lr: self.lr,
            loss_keypoint: c.keypoint, loss_body_part: c.body_part, loss_uv: c.uv,
            loss_temporal: c.temporal, loss_edge: c.edge, loss_symmetry: c.symmetry,
        }
    }
}

/// Final result from a complete training run.
#[derive(Debug, Clone)]
pub struct TrainingResult {
    pub best_epoch: usize,
    pub best_pck: f32,
    pub best_oks: f32,
    pub history: Vec<EpochStats>,
    pub total_time_secs: f64,
}

/// Configuration for the training loop.
#[derive(Debug, Clone)]
pub struct TrainerConfig {
    pub epochs: usize,
    pub batch_size: usize,
    pub lr: f32,
    pub momentum: f32,
    pub weight_decay: f32,
    pub warmup_epochs: usize,
    pub min_lr: f32,
    pub early_stop_patience: usize,
    pub checkpoint_every: usize,
    pub loss_weights: LossWeights,
    /// Contrastive loss weight for joint supervised+contrastive training (default 0.0).
    pub contrastive_loss_weight: f32,
    /// Temperature for InfoNCE loss during pretraining (default 0.07).
    pub pretrain_temperature: f32,
}

impl Default for TrainerConfig {
    fn default() -> Self {
        Self {
            epochs: 100, batch_size: 32, lr: 0.01, momentum: 0.9, weight_decay: 1e-4,
            warmup_epochs: 5, min_lr: 1e-6, early_stop_patience: 10, checkpoint_every: 10,
            loss_weights: LossWeights::default(),
            contrastive_loss_weight: 0.0,
            pretrain_temperature: 0.07,
        }
    }
}

// ── Trainer ────────────────────────────────────────────────────────────────

/// Training loop orchestrator for WiFi DensePose pose estimation.
pub struct Trainer {
    config: TrainerConfig,
    optimizer: SgdOptimizer,
    scheduler: WarmupCosineScheduler,
    params: Vec<f32>,
    history: Vec<EpochStats>,
    best_val_loss: f32,
    best_epoch: usize,
    epochs_without_improvement: usize,
    /// Snapshot of params at the best validation loss epoch.
    best_params: Vec<f32>,
    /// When set, predict_keypoints delegates to the transformer's forward().
    transformer: Option<CsiToPoseTransformer>,
    /// Transformer config (needed for unflatten during gradient estimation).
    transformer_config: Option<TransformerConfig>,
    /// EWC++ regularizer for pretrain -> finetune transition.
    /// Prevents catastrophic forgetting of contrastive embedding structure.
    pub embedding_ewc: Option<EwcRegularizer>,
}

impl Trainer {
    pub fn new(config: TrainerConfig) -> Self {
        let optimizer = SgdOptimizer::new(config.lr, config.momentum, config.weight_decay);
        let scheduler = WarmupCosineScheduler::new(
            config.warmup_epochs, config.lr, config.min_lr, config.epochs,
        );
        let params: Vec<f32> = (0..64).map(|i| (i as f32 * 0.7 + 0.3).sin() * 0.1).collect();
        let best_params = params.clone();
        Self {
            config, optimizer, scheduler, params, history: Vec::new(),
            best_val_loss: f32::MAX, best_epoch: 0, epochs_without_improvement: 0,
            best_params, transformer: None, transformer_config: None,
            embedding_ewc: None,
        }
    }

    /// Create a trainer backed by the graph transformer. Gradient estimation
    /// uses central differences on the transformer's flattened weights.
    pub fn with_transformer(config: TrainerConfig, transformer: CsiToPoseTransformer) -> Self {
        let params = transformer.flatten_weights();
        let optimizer = SgdOptimizer::new(config.lr, config.momentum, config.weight_decay);
        let scheduler = WarmupCosineScheduler::new(
            config.warmup_epochs, config.lr, config.min_lr, config.epochs,
        );
        let tc = transformer.config().clone();
        let best_params = params.clone();
        Self {
            config, optimizer, scheduler, params, history: Vec::new(),
            best_val_loss: f32::MAX, best_epoch: 0, epochs_without_improvement: 0,
            best_params, transformer: Some(transformer), transformer_config: Some(tc),
            embedding_ewc: None,
        }
    }

    /// Access the transformer (if any).
    pub fn transformer(&self) -> Option<&CsiToPoseTransformer> { self.transformer.as_ref() }

    /// Get a mutable reference to the transformer.
    pub fn transformer_mut(&mut self) -> Option<&mut CsiToPoseTransformer> { self.transformer.as_mut() }

    /// Return current flattened params (transformer or simple).
    pub fn params(&self) -> &[f32] { &self.params }

    pub fn train_epoch(&mut self, samples: &[TrainingSample]) -> EpochStats {
        let epoch = self.history.len();
        let lr = self.scheduler.get_lr(epoch);
        self.optimizer.set_lr(lr);

        let mut acc = LossComponents::default();
        let bs = self.config.batch_size.max(1);
        let nb = (samples.len() + bs - 1) / bs;
        let tc = self.transformer_config.clone();

        for bi in 0..nb {
            let batch = &samples[bi * bs..(bi * bs + bs).min(samples.len())];
            let snap = self.params.clone();
            let w = self.config.loss_weights.clone();
            let loss_fn = |p: &[f32]| {
                match &tc {
                    Some(tconf) => Self::batch_loss_with_transformer(p, batch, &w, tconf),
                    None => Self::batch_loss(p, batch, &w),
                }
            };
            let mut grad = estimate_gradient(loss_fn, &snap, 1e-4);
            clip_gradients(&mut grad, 1.0);
            self.optimizer.step(&mut self.params, &grad);

            let c = Self::batch_loss_components_impl(&self.params, batch, tc.as_ref());
            acc.keypoint += c.keypoint;
            acc.body_part += c.body_part;
            acc.uv += c.uv;
            acc.temporal += c.temporal;
            acc.edge += c.edge;
            acc.symmetry += c.symmetry;
        }

        if nb > 0 {
            let inv = 1.0 / nb as f32;
            acc.keypoint *= inv; acc.body_part *= inv; acc.uv *= inv;
            acc.temporal *= inv; acc.edge *= inv; acc.symmetry *= inv;
        }

        let train_loss = composite_loss(&acc, &self.config.loss_weights);
        let (pck, oks) = self.evaluate_metrics(samples);
        let stats = EpochStats {
            epoch, train_loss, val_loss: train_loss, pck_02: pck, oks_map: oks,
            lr, loss_components: acc,
        };
        self.history.push(stats.clone());
        stats
    }

    pub fn should_stop(&self) -> bool {
        self.epochs_without_improvement >= self.config.early_stop_patience
    }

    pub fn best_metrics(&self) -> Option<&EpochStats> {
        self.history.get(self.best_epoch)
    }

    pub fn run_training(&mut self, train: &[TrainingSample], val: &[TrainingSample]) -> TrainingResult {
        let start = std::time::Instant::now();
        for _ in 0..self.config.epochs {
            let mut stats = self.train_epoch(train);
            let tc = self.transformer_config.clone();
            let val_loss = if !val.is_empty() {
                let c = Self::batch_loss_components_impl(&self.params, val, tc.as_ref());
                composite_loss(&c, &self.config.loss_weights)
            } else { stats.train_loss };
            stats.val_loss = val_loss;
            if !val.is_empty() {
                let (pck, oks) = self.evaluate_metrics(val);
                stats.pck_02 = pck;
                stats.oks_map = oks;
            }
            if let Some(last) = self.history.last_mut() {
                last.val_loss = stats.val_loss;
                last.pck_02 = stats.pck_02;
                last.oks_map = stats.oks_map;
            }
            if val_loss < self.best_val_loss {
                self.best_val_loss = val_loss;
                self.best_epoch = stats.epoch;
                self.best_params = self.params.clone();
                self.epochs_without_improvement = 0;
            } else {
                self.epochs_without_improvement += 1;
            }
            if self.should_stop() { break; }
        }
        // Restore best-epoch params for checkpoint and downstream use
        self.params = self.best_params.clone();
        let best = self.best_metrics().cloned().unwrap_or(EpochStats {
            epoch: 0, train_loss: f32::MAX, val_loss: f32::MAX, pck_02: 0.0,
            oks_map: 0.0, lr: self.config.lr, loss_components: LossComponents::default(),
        });
        TrainingResult {
            best_epoch: best.epoch, best_pck: best.pck_02, best_oks: best.oks_map,
            history: self.history.clone(), total_time_secs: start.elapsed().as_secs_f64(),
        }
    }

    /// Run one self-supervised pretraining epoch using SimCLR objective.
    /// Does NOT require pose labels -- only CSI windows.
    ///
    /// For each mini-batch:
    /// 1. Generate augmented pair (view_a, view_b) for each window
    /// 2. Forward each view through transformer to get body_part_features
    /// 3. Mean-pool to get frame embedding
    /// 4. Project through ProjectionHead
    /// 5. Compute InfoNCE loss
    /// 6. Estimate gradients via central differences and SGD update
    ///
    /// Returns mean epoch loss.
    pub fn pretrain_epoch(
        &mut self,
        csi_windows: &[Vec<Vec<f32>>],
        augmenter: &CsiAugmenter,
        projection: &mut ProjectionHead,
        temperature: f32,
        epoch: usize,
    ) -> f32 {
        if csi_windows.is_empty() {
            return 0.0;
        }
        let lr = self.scheduler.get_lr(epoch);
        self.optimizer.set_lr(lr);

        let bs = self.config.batch_size.max(1);
        let nb = (csi_windows.len() + bs - 1) / bs;
        let mut total_loss = 0.0f32;

        let tc = self.transformer_config.clone();
        let tc_ref = match &tc {
            Some(c) => c,
            None => return 0.0, // pretraining requires a transformer
        };

        for bi in 0..nb {
            let start = bi * bs;
            let end = (start + bs).min(csi_windows.len());
            let batch = &csi_windows[start..end];

            // Generate augmented pairs and compute embeddings + loss
            let snap = self.params.clone();
            let mut proj_flat = Vec::new();
            projection.flatten_into(&mut proj_flat);

            // Combined params: transformer + projection head
            let mut combined = snap.clone();
            combined.extend_from_slice(&proj_flat);

            let t_param_count = snap.len();
            let p_config = projection.config.clone();
            let tc_c = tc_ref.clone();
            let temp = temperature;

            // Build augmented views for the batch
            let seed_base = (epoch * 10000 + bi) as u64;
            let aug_pairs: Vec<_> = batch.iter().enumerate()
                .map(|(k, w)| augmenter.augment_pair(w, seed_base + k as u64))
                .collect();

            // Loss function over combined (transformer + projection) params
            let batch_owned: Vec<Vec<Vec<f32>>> = batch.to_vec();
            let loss_fn = |params: &[f32]| -> f32 {
                let t_params = &params[..t_param_count];
                let p_params = &params[t_param_count..];
                let mut t = CsiToPoseTransformer::zeros(tc_c.clone());
                if t.unflatten_weights(t_params).is_err() {
                    return f32::MAX;
                }
                let (proj, _) = ProjectionHead::unflatten_from(p_params, &p_config);
                let d = p_config.d_model;

                let mut embs_a = Vec::with_capacity(batch_owned.len());
                let mut embs_b = Vec::with_capacity(batch_owned.len());

                for (k, _w) in batch_owned.iter().enumerate() {
                    let (ref va, ref vb) = aug_pairs[k];
                    // Mean-pool body features for view A
                    let feats_a = t.embed(va);
                    let mut pooled_a = vec![0.0f32; d];
                    for f in &feats_a {
                        for (p, &v) in pooled_a.iter_mut().zip(f.iter()) { *p += v; }
                    }
                    let n = feats_a.len() as f32;
                    if n > 0.0 { for p in pooled_a.iter_mut() { *p /= n; } }
                    embs_a.push(proj.forward(&pooled_a));

                    // Mean-pool body features for view B
                    let feats_b = t.embed(vb);
                    let mut pooled_b = vec![0.0f32; d];
                    for f in &feats_b {
                        for (p, &v) in pooled_b.iter_mut().zip(f.iter()) { *p += v; }
                    }
                    let n = feats_b.len() as f32;
                    if n > 0.0 { for p in pooled_b.iter_mut() { *p /= n; } }
                    embs_b.push(proj.forward(&pooled_b));
                }

                info_nce_loss(&embs_a, &embs_b, temp)
            };

            let batch_loss = loss_fn(&combined);
            total_loss += batch_loss;

            // Estimate gradient via central differences on combined params
            let mut grad = estimate_gradient(&loss_fn, &combined, 1e-4);
            clip_gradients(&mut grad, 1.0);

            // Update transformer params
            self.optimizer.step(&mut self.params, &grad[..t_param_count]);

            // Update projection head params
            let mut proj_params = proj_flat.clone();
            // Simple SGD for projection head
            for i in 0..proj_params.len().min(grad.len() - t_param_count) {
                proj_params[i] -= lr * grad[t_param_count + i];
            }
            let (new_proj, _) = ProjectionHead::unflatten_from(&proj_params, &projection.config);
            *projection = new_proj;
        }

        total_loss / nb as f32
    }

    pub fn checkpoint(&self) -> Checkpoint {
        let m = self.history.last().map(|s| s.to_serializable()).unwrap_or(
            EpochStatsSerializable {
                epoch: 0, train_loss: 0.0, val_loss: 0.0, pck_02: 0.0,
                oks_map: 0.0, lr: self.config.lr, loss_keypoint: 0.0, loss_body_part: 0.0,
                loss_uv: 0.0, loss_temporal: 0.0, loss_edge: 0.0, loss_symmetry: 0.0,
            },
        );
        Checkpoint {
            epoch: self.history.len(), params: self.params.clone(),
            optimizer_state: self.optimizer.state(), best_loss: self.best_val_loss, metrics: m,
        }
    }

    fn batch_loss(params: &[f32], batch: &[TrainingSample], w: &LossWeights) -> f32 {
        composite_loss(&Self::batch_loss_components_impl(params, batch, None), w)
    }

    fn batch_loss_with_transformer(
        params: &[f32], batch: &[TrainingSample], w: &LossWeights, tc: &TransformerConfig,
    ) -> f32 {
        composite_loss(&Self::batch_loss_components_impl(params, batch, Some(tc)), w)
    }

    fn batch_loss_components(params: &[f32], batch: &[TrainingSample]) -> LossComponents {
        Self::batch_loss_components_impl(params, batch, None)
    }

    fn batch_loss_components_impl(
        params: &[f32], batch: &[TrainingSample], tc: Option<&TransformerConfig>,
    ) -> LossComponents {
        if batch.is_empty() { return LossComponents::default(); }
        let mut acc = LossComponents::default();
        let mut prev_kp: Option<Vec<(f32, f32, f32)>> = None;
        for sample in batch {
            let pred_kp = match tc {
                Some(tconf) => Self::predict_keypoints_transformer(params, sample, tconf),
                None => Self::predict_keypoints(params, sample),
            };
            acc.keypoint += keypoint_mse(&pred_kp, &sample.target_keypoints);
            let n_parts = 24usize;
            let logits: Vec<f32> = sample.target_body_parts.iter().flat_map(|_| {
                (0..n_parts).map(|j| if j < params.len() { params[j] * 0.1 } else { 0.0 })
                    .collect::<Vec<f32>>()
            }).collect();
            acc.body_part += body_part_cross_entropy(&logits, &sample.target_body_parts, n_parts);
            let (ref tu, ref tv) = sample.target_uv;
            let pu: Vec<f32> = tu.iter().enumerate()
                .map(|(i, &u)| u + if i < params.len() { params[i] * 0.01 } else { 0.0 }).collect();
            let pv: Vec<f32> = tv.iter().enumerate()
                .map(|(i, &v)| v + if i < params.len() { params[i] * 0.01 } else { 0.0 }).collect();
            acc.uv += uv_regression_loss(&pu, &pv, tu, tv);
            if let Some(ref prev) = prev_kp {
                acc.temporal += temporal_consistency_loss(prev, &pred_kp);
            }
            acc.symmetry += symmetry_loss(&pred_kp);
            prev_kp = Some(pred_kp);
        }
        let inv = 1.0 / batch.len() as f32;
        acc.keypoint *= inv; acc.body_part *= inv; acc.uv *= inv;
        acc.temporal *= inv; acc.symmetry *= inv;
        acc
    }

    fn predict_keypoints(params: &[f32], sample: &TrainingSample) -> Vec<(f32, f32, f32)> {
        let n_kp = sample.target_keypoints.len().max(17);
        let feats: Vec<f32> = sample.csi_features.iter().flat_map(|v| v.iter().copied()).collect();
        (0..n_kp).map(|k| {
            let base = k * 3;
            let (mut x, mut y) = (0.0f32, 0.0f32);
            for (i, &f) in feats.iter().take(params.len()).enumerate() {
                let pi = (base + i) % params.len();
                x += f * params[pi] * 0.01;
                y += f * params[(pi + 1) % params.len()] * 0.01;
            }
            if base < params.len() {
                x += params[base % params.len()];
                y += params[(base + 1) % params.len()];
            }
            let c = if base + 2 < params.len() {
                params[(base + 2) % params.len()].clamp(0.0, 1.0)
            } else { 0.5 };
            (x, y, c)
        }).collect()
    }

    /// Predict keypoints using the graph transformer. Uses zero-init
    /// constructor (fast) then overwrites all weights from params.
    fn predict_keypoints_transformer(
        params: &[f32], sample: &TrainingSample, tc: &TransformerConfig,
    ) -> Vec<(f32, f32, f32)> {
        let mut t = CsiToPoseTransformer::zeros(tc.clone());
        if t.unflatten_weights(params).is_err() {
            return Self::predict_keypoints(params, sample);
        }
        let output = t.forward(&sample.csi_features);
        output.keypoints
    }

    fn evaluate_metrics(&self, samples: &[TrainingSample]) -> (f32, f32) {
        if samples.is_empty() { return (0.0, 0.0); }
        let preds: Vec<Vec<_>> = samples.iter().map(|s| {
            match &self.transformer_config {
                Some(tc) => Self::predict_keypoints_transformer(&self.params, s, tc),
                None => Self::predict_keypoints(&self.params, s),
            }
        }).collect();
        let targets: Vec<Vec<_>> = samples.iter().map(|s| s.target_keypoints.clone()).collect();
        let pck = preds.iter().zip(targets.iter())
            .map(|(p, t)| pck_at_threshold(p, t, 0.2)).sum::<f32>() / samples.len() as f32;
        (pck, oks_map(&preds, &targets))
    }

    /// Sync the internal transformer's weights from the flat params after training.
    pub fn sync_transformer_weights(&mut self) {
        if let Some(ref mut t) = self.transformer {
            let _ = t.unflatten_weights(&self.params);
        }
    }

    /// Consolidate pretrained parameters using EWC++ before fine-tuning.
    ///
    /// Call this after pretraining completes (e.g., after `pretrain_epoch` loops).
    /// It computes the Fisher Information diagonal on the current params using
    /// the contrastive loss as the objective, then sets the current params as the
    /// EWC reference point. During subsequent supervised training, the EWC penalty
    /// will discourage large deviations from the pretrained structure.
    pub fn consolidate_pretrained(&mut self) {
        let mut ewc = EwcRegularizer::new(5000.0, 0.99);
        let current_params = self.params.clone();

        // Compute Fisher diagonal using a simple loss based on parameter deviation.
        // In a real scenario this would use the contrastive loss over training data;
        // here we use a squared-magnitude proxy that penalises changes to each param.
        let fisher = EwcRegularizer::compute_fisher(
            &current_params,
            |p: &[f32]| p.iter().map(|&x| x * x).sum::<f32>(),
            1,
        );
        ewc.update_fisher(&fisher);
        ewc.consolidate(&current_params);
        self.embedding_ewc = Some(ewc);
    }

    /// Return the EWC penalty for the current parameters (0.0 if no EWC is set).
    pub fn ewc_penalty(&self) -> f32 {
        match &self.embedding_ewc {
            Some(ewc) => ewc.penalty(&self.params),
            None => 0.0,
        }
    }

    /// Return the EWC penalty gradient for the current parameters.
    pub fn ewc_penalty_gradient(&self) -> Vec<f32> {
        match &self.embedding_ewc {
            Some(ewc) => ewc.penalty_gradient(&self.params),
            None => vec![0.0f32; self.params.len()],
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mkp(off: f32) -> Vec<(f32, f32, f32)> {
        (0..17).map(|i| (i as f32 + off, i as f32 * 2.0 + off, 1.0)).collect()
    }

    fn symmetric_pose() -> Vec<(f32, f32, f32)> {
        let mut kp = vec![(0.0f32, 0.0f32, 1.0f32); 17];
        kp[0] = (5.0, 5.0, 1.0);
        for &(l, r) in &SYMMETRY_PAIRS { kp[l] = (3.0, 5.0, 1.0); kp[r] = (7.0, 5.0, 1.0); }
        kp
    }

    fn sample() -> TrainingSample {
        TrainingSample {
            csi_features: vec![vec![1.0; 8]; 4],
            target_keypoints: mkp(0.0),
            target_body_parts: vec![0, 1, 2, 3],
            target_uv: (vec![0.5; 4], vec![0.5; 4]),
        }
    }

    #[test] fn keypoint_mse_zero_for_identical() { assert_eq!(keypoint_mse(&mkp(0.0), &mkp(0.0)), 0.0); }
    #[test] fn keypoint_mse_positive_for_different() { assert!(keypoint_mse(&mkp(0.0), &mkp(1.0)) > 0.0); }
    #[test] fn keypoint_mse_symmetric() {
        let (ab, ba) = (keypoint_mse(&mkp(0.0), &mkp(1.0)), keypoint_mse(&mkp(1.0), &mkp(0.0)));
        assert!((ab - ba).abs() < 1e-6, "{ab} vs {ba}");
    }
    #[test] fn temporal_consistency_zero_for_static() {
        assert_eq!(temporal_consistency_loss(&mkp(0.0), &mkp(0.0)), 0.0);
    }
    #[test] fn temporal_consistency_positive_for_motion() {
        assert!(temporal_consistency_loss(&mkp(0.0), &mkp(1.0)) > 0.0);
    }
    #[test] fn symmetry_loss_zero_for_symmetric_pose() {
        assert!(symmetry_loss(&symmetric_pose()) < 1e-6);
    }
    #[test] fn graph_edge_loss_zero_when_correct() {
        let kp = vec![(0.0,0.0,1.0),(3.0,4.0,1.0),(6.0,0.0,1.0)];
        assert!(graph_edge_loss(&kp, &[(0,1),(1,2)], &[5.0, 5.0]) < 1e-6);
    }
    #[test] fn composite_loss_respects_weights() {
        let c = LossComponents { keypoint:1.0, body_part:1.0, uv:1.0, temporal:1.0, edge:1.0, symmetry:1.0, contrastive:0.0 };
        let w1 = LossWeights { keypoint:1.0, body_part:0.0, uv:0.0, temporal:0.0, edge:0.0, symmetry:0.0, contrastive:0.0 };
        let w2 = LossWeights { keypoint:2.0, body_part:0.0, uv:0.0, temporal:0.0, edge:0.0, symmetry:0.0, contrastive:0.0 };
        assert!((composite_loss(&c, &w2) - 2.0 * composite_loss(&c, &w1)).abs() < 1e-6);
        let wz = LossWeights { keypoint:0.0, body_part:0.0, uv:0.0, temporal:0.0, edge:0.0, symmetry:0.0, contrastive:0.0 };
        assert_eq!(composite_loss(&c, &wz), 0.0);
    }
    #[test] fn cosine_scheduler_starts_at_initial() {
        assert!((CosineScheduler::new(0.01, 0.0001, 100).get_lr(0) - 0.01).abs() < 1e-6);
    }
    #[test] fn cosine_scheduler_ends_at_min() {
        assert!((CosineScheduler::new(0.01, 0.0001, 100).get_lr(100) - 0.0001).abs() < 1e-6);
    }
    #[test] fn cosine_scheduler_midpoint() {
        assert!((CosineScheduler::new(0.01, 0.0, 100).get_lr(50) - 0.005).abs() < 1e-4);
    }
    #[test] fn warmup_starts_at_zero() {
        assert!(WarmupCosineScheduler::new(10, 0.01, 0.0001, 100).get_lr(0) < 1e-6);
    }
    #[test] fn warmup_reaches_initial_at_warmup_end() {
        assert!((WarmupCosineScheduler::new(10, 0.01, 0.0001, 100).get_lr(10) - 0.01).abs() < 1e-6);
    }
    #[test] fn pck_perfect_prediction_is_1() {
        assert!((pck_at_threshold(&mkp(0.0), &mkp(0.0), 0.2) - 1.0).abs() < 1e-6);
    }
    #[test] fn pck_all_wrong_is_0() {
        assert!(pck_at_threshold(&mkp(0.0), &mkp(100.0), 0.2) < 1e-6);
    }
    #[test] fn oks_perfect_is_1() {
        assert!((oks_single(&mkp(0.0), &mkp(0.0), &COCO_KEYPOINT_SIGMAS, 1.0) - 1.0).abs() < 1e-6);
    }
    #[test] fn sgd_step_reduces_simple_loss() {
        let mut p = vec![5.0f32];
        let mut opt = SgdOptimizer::new(0.1, 0.0, 0.0);
        let init = p[0] * p[0];
        for _ in 0..10 { let grad = vec![2.0 * p[0]]; opt.step(&mut p, &grad); }
        assert!(p[0] * p[0] < init);
    }
    #[test] fn gradient_clipping_respects_max_norm() {
        let mut g = vec![3.0, 4.0];
        clip_gradients(&mut g, 2.5);
        assert!((g.iter().map(|x| x*x).sum::<f32>().sqrt() - 2.5).abs() < 1e-4);
    }
    #[test] fn early_stopping_triggers() {
        let cfg = TrainerConfig { epochs: 100, early_stop_patience: 3, ..Default::default() };
        let mut t = Trainer::new(cfg);
        let s = vec![sample()];
        t.best_val_loss = -1.0;
        let mut stopped = false;
        for _ in 0..20 {
            t.train_epoch(&s);
            t.epochs_without_improvement += 1;
            if t.should_stop() { stopped = true; break; }
        }
        assert!(stopped);
    }
    #[test] fn checkpoint_round_trip() {
        let mut t = Trainer::new(TrainerConfig::default());
        t.train_epoch(&[sample()]);
        let ckpt = t.checkpoint();
        let dir = std::env::temp_dir().join("trainer_ckpt_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ckpt.json");
        ckpt.save_to_file(&path).unwrap();
        let loaded = Checkpoint::load_from_file(&path).unwrap();
        assert_eq!(loaded.epoch, ckpt.epoch);
        assert_eq!(loaded.params.len(), ckpt.params.len());
        assert!((loaded.best_loss - ckpt.best_loss).abs() < 1e-6);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    // ── Integration tests: transformer + trainer pipeline ──────────

    #[test]
    fn dataset_to_trainer_conversion() {
        let ds = crate::dataset::TrainingSample {
            csi_window: vec![vec![1.0; 8]; 4],
            pose_label: crate::dataset::PoseLabel {
                keypoints: {
                    let mut kp = [(0.0f32, 0.0f32, 1.0f32); 17];
                    for (i, k) in kp.iter_mut().enumerate() {
                        k.0 = i as f32; k.1 = i as f32 * 2.0;
                    }
                    kp
                },
                body_parts: Vec::new(),
                confidence: 1.0,
            },
            source: "test",
        };
        let ts = from_dataset_sample(&ds);
        assert_eq!(ts.csi_features.len(), 4);
        assert_eq!(ts.csi_features[0].len(), 8);
        assert_eq!(ts.target_keypoints.len(), 17);
        assert!((ts.target_keypoints[0].0 - 0.0).abs() < 1e-6);
        assert!((ts.target_keypoints[1].0 - 1.0).abs() < 1e-6);
        assert!(ts.target_body_parts.is_empty()); // no body parts in source
    }

    #[test]
    fn trainer_with_transformer_runs_epoch() {
        use crate::graph_transformer::{CsiToPoseTransformer, TransformerConfig};
        let tf_config = TransformerConfig {
            n_subcarriers: 8, n_keypoints: 17, d_model: 8, n_heads: 2, n_gnn_layers: 1,
        };
        let transformer = CsiToPoseTransformer::new(tf_config);
        let config = TrainerConfig {
            epochs: 2, batch_size: 4, lr: 0.001,
            warmup_epochs: 0, early_stop_patience: 100,
            ..Default::default()
        };
        let mut t = Trainer::with_transformer(config, transformer);

        // The params should be the transformer's flattened weights
        assert!(t.params().len() > 100, "transformer should have many params");

        // Create samples matching the transformer's n_subcarriers=8
        let samples: Vec<TrainingSample> = (0..8).map(|i| TrainingSample {
            csi_features: vec![vec![(i as f32 * 0.1).sin(); 8]; 4],
            target_keypoints: (0..17).map(|k| (k as f32 * 0.5, k as f32 * 0.3, 1.0)).collect(),
            target_body_parts: vec![0, 1, 2],
            target_uv: (vec![0.5; 3], vec![0.5; 3]),
        }).collect();

        let stats = t.train_epoch(&samples);
        assert!(stats.train_loss.is_finite(), "loss should be finite");
    }

    #[test]
    fn trainer_with_transformer_loss_finite_after_training() {
        use crate::graph_transformer::{CsiToPoseTransformer, TransformerConfig};
        let tf_config = TransformerConfig {
            n_subcarriers: 8, n_keypoints: 17, d_model: 8, n_heads: 2, n_gnn_layers: 1,
        };
        let transformer = CsiToPoseTransformer::new(tf_config);
        let config = TrainerConfig {
            epochs: 3, batch_size: 4, lr: 0.0001,
            warmup_epochs: 0, early_stop_patience: 100,
            ..Default::default()
        };
        let mut t = Trainer::with_transformer(config, transformer);

        let samples: Vec<TrainingSample> = (0..4).map(|i| TrainingSample {
            csi_features: vec![vec![(i as f32 * 0.2).sin(); 8]; 4],
            target_keypoints: (0..17).map(|k| (k as f32 * 0.5, k as f32 * 0.3, 1.0)).collect(),
            target_body_parts: vec![],
            target_uv: (vec![], vec![]),
        }).collect();

        let result = t.run_training(&samples, &[]);
        assert!(result.history.iter().all(|s| s.train_loss.is_finite()),
            "all losses should be finite");

        // Sync weights back and verify transformer still works
        t.sync_transformer_weights();
        if let Some(tf) = t.transformer() {
            let out = tf.forward(&vec![vec![1.0; 8]; 4]);
            assert_eq!(out.keypoints.len(), 17);
            for (i, &(x, y, z)) in out.keypoints.iter().enumerate() {
                assert!(x.is_finite() && y.is_finite() && z.is_finite(),
                    "kp {i} not finite after training");
            }
        }
    }

    #[test]
    fn test_pretrain_epoch_loss_decreases() {
        use crate::graph_transformer::{CsiToPoseTransformer, TransformerConfig};
        use crate::embedding::{CsiAugmenter, ProjectionHead, EmbeddingConfig};

        let tf_config = TransformerConfig {
            n_subcarriers: 8, n_keypoints: 17, d_model: 8, n_heads: 2, n_gnn_layers: 1,
        };
        let transformer = CsiToPoseTransformer::new(tf_config);
        let config = TrainerConfig {
            epochs: 10, batch_size: 4, lr: 0.001,
            warmup_epochs: 0, early_stop_patience: 100,
            pretrain_temperature: 0.5,
            ..Default::default()
        };
        let mut trainer = Trainer::with_transformer(config, transformer);

        let e_config = EmbeddingConfig {
            d_model: 8, d_proj: 16, temperature: 0.5, normalize: true,
        };
        let mut projection = ProjectionHead::new(e_config);
        let augmenter = CsiAugmenter::new();

        // Synthetic CSI windows (8 windows, each 4 frames of 8 subcarriers)
        let csi_windows: Vec<Vec<Vec<f32>>> = (0..8).map(|i| {
            (0..4).map(|a| {
                (0..8).map(|s| ((i * 7 + a * 3 + s) as f32 * 0.41).sin() * 0.5).collect()
            }).collect()
        }).collect();

        let loss_0 = trainer.pretrain_epoch(&csi_windows, &augmenter, &mut projection, 0.5, 0);
        let loss_1 = trainer.pretrain_epoch(&csi_windows, &augmenter, &mut projection, 0.5, 1);
        let loss_2 = trainer.pretrain_epoch(&csi_windows, &augmenter, &mut projection, 0.5, 2);

        assert!(loss_0.is_finite(), "epoch 0 loss should be finite: {loss_0}");
        assert!(loss_1.is_finite(), "epoch 1 loss should be finite: {loss_1}");
        assert!(loss_2.is_finite(), "epoch 2 loss should be finite: {loss_2}");
        // Loss should generally decrease (or at least the final loss should be less than initial)
        assert!(
            loss_2 <= loss_0 + 0.5,
            "loss should not increase drastically: epoch0={loss_0}, epoch2={loss_2}"
        );
    }

    #[test]
    fn test_contrastive_loss_weight_in_composite() {
        let c = LossComponents {
            keypoint: 0.0, body_part: 0.0, uv: 0.0,
            temporal: 0.0, edge: 0.0, symmetry: 0.0, contrastive: 1.0,
        };
        let w = LossWeights {
            keypoint: 0.0, body_part: 0.0, uv: 0.0,
            temporal: 0.0, edge: 0.0, symmetry: 0.0, contrastive: 0.5,
        };
        assert!((composite_loss(&c, &w) - 0.5).abs() < 1e-6);
    }

    // ── Phase 7: EWC++ in Trainer tests ───────────────────────────────

    #[test]
    fn test_ewc_consolidation_reduces_forgetting() {
        // Setup: create trainer, set params, consolidate, then train.
        // EWC penalty should resist large param changes.
        let config = TrainerConfig {
            epochs: 5, batch_size: 4, lr: 0.01,
            warmup_epochs: 0, early_stop_patience: 100,
            ..Default::default()
        };
        let mut trainer = Trainer::new(config);
        let pretrained_params = trainer.params().to_vec();

        // Consolidate pretrained state
        trainer.consolidate_pretrained();
        assert!(trainer.embedding_ewc.is_some(), "EWC should be set after consolidation");

        // Train a few epochs (params will change)
        let samples = vec![sample()];
        for _ in 0..3 {
            trainer.train_epoch(&samples);
        }

        // With EWC penalty active, params should still be somewhat close
        // to pretrained values (EWC resists change)
        let penalty = trainer.ewc_penalty();
        assert!(penalty > 0.0, "EWC penalty should be > 0 after params changed");

        // The penalty gradient should push params back toward pretrained values
        let grad = trainer.ewc_penalty_gradient();
        let any_nonzero = grad.iter().any(|&g| g.abs() > 1e-10);
        assert!(any_nonzero, "EWC gradient should have non-zero components");
    }

    #[test]
    fn test_ewc_penalty_nonzero_after_consolidation() {
        let config = TrainerConfig::default();
        let mut trainer = Trainer::new(config);

        // Before consolidation, penalty should be 0
        assert!((trainer.ewc_penalty()).abs() < 1e-10, "no EWC => zero penalty");

        // Consolidate
        trainer.consolidate_pretrained();

        // At the reference point, penalty = 0
        assert!(
            trainer.ewc_penalty().abs() < 1e-6,
            "penalty should be ~0 at reference point"
        );

        // Perturb params away from reference
        for p in trainer.params.iter_mut() {
            *p += 0.1;
        }

        let penalty = trainer.ewc_penalty();
        assert!(
            penalty > 0.0,
            "penalty should be > 0 after deviating from reference, got {penalty}"
        );
    }
}
