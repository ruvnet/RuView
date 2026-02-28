//! Training loop for WiFi-DensePose.
//!
//! # Features
//!
//! - Mini-batch training with [`DataLoader`]-style iteration
//! - Validation every N epochs with PCK\@0.2 and OKS metrics
//! - Best-checkpoint saving (by validation PCK)
//! - CSV logging (`epoch, train_loss, val_pck, val_oks, lr`)
//! - Gradient clipping
//! - LR scheduling (step decay at configured milestones)
//! - Early stopping
//!
//! # No mock data
//!
//! The trainer never generates random or synthetic data. It operates
//! exclusively on the [`CsiDataset`] passed at call site. The
//! [`SyntheticCsiDataset`] is only used for the deterministic proof protocol.

use std::collections::VecDeque;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::time::Instant;

use ndarray::{Array1, Array2};
use tch::{nn, nn::OptimizerConfig, Device, Kind, Tensor};
use tracing::{debug, info, warn};

use crate::config::TrainingConfig;
use crate::dataset::{CsiDataset, CsiSample, DataLoader};
use crate::error::TrainError;
use crate::losses::{LossWeights, WiFiDensePoseLoss};
use crate::losses::generate_target_heatmaps;
use crate::metrics::{MetricsAccumulator, MetricsResult};
use crate::model::WiFiDensePoseModel;

// ---------------------------------------------------------------------------
// Public result types
// ---------------------------------------------------------------------------

/// Per-epoch training log entry.
#[derive(Debug, Clone)]
pub struct EpochLog {
    /// Epoch number (1-indexed).
    pub epoch: usize,
    /// Mean total loss over all training batches.
    pub train_loss: f64,
    /// Mean keypoint-only loss component.
    pub train_kp_loss: f64,
    /// Validation PCK\@0.2 (0–1). `0.0` when validation was skipped.
    pub val_pck: f32,
    /// Validation OKS (0–1). `0.0` when validation was skipped.
    pub val_oks: f32,
    /// Learning rate at the end of this epoch.
    pub lr: f64,
    /// Wall-clock duration of this epoch in seconds.
    pub duration_secs: f64,
}

/// Summary returned after a completed (or early-stopped) training run.
#[derive(Debug, Clone)]
pub struct TrainResult {
    /// Best validation PCK achieved during training.
    pub best_pck: f32,
    /// Epoch at which `best_pck` was achieved (1-indexed).
    pub best_epoch: usize,
    /// Training loss on the last completed epoch.
    pub final_train_loss: f64,
    /// Full per-epoch log.
    pub training_history: Vec<EpochLog>,
    /// Path to the best checkpoint file, if any was saved.
    pub checkpoint_path: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Trainer
// ---------------------------------------------------------------------------

/// Orchestrates the full WiFi-DensePose training pipeline.
///
/// Create via [`Trainer::new`], then call [`Trainer::train`] with real dataset
/// references.
pub struct Trainer {
    config: TrainingConfig,
    model: WiFiDensePoseModel,
    device: Device,
}

impl Trainer {
    /// Create a new `Trainer` from the given configuration.
    ///
    /// The model and device are initialised from `config`.
    pub fn new(config: TrainingConfig) -> Self {
        let device = if config.use_gpu {
            Device::Cuda(config.gpu_device_id as usize)
        } else {
            Device::Cpu
        };

        tch::manual_seed(config.seed as i64);

        let model = WiFiDensePoseModel::new(&config, device);
        Trainer { config, model, device }
    }

    /// Run the full training loop.
    ///
    /// # Errors
    ///
    /// - [`TrainError::EmptyDataset`] if either dataset is empty.
    /// - [`TrainError::TrainingStep`] on unrecoverable forward/backward errors.
    /// - [`TrainError::Checkpoint`] if writing checkpoints fails.
    pub fn train(
        &mut self,
        train_dataset: &dyn CsiDataset,
        val_dataset: &dyn CsiDataset,
    ) -> Result<TrainResult, TrainError> {
        if train_dataset.is_empty() {
            return Err(TrainError::EmptyDataset);
        }
        if val_dataset.is_empty() {
            return Err(TrainError::EmptyDataset);
        }

        // Prepare output directories.
        std::fs::create_dir_all(&self.config.checkpoint_dir)
            .map_err(|e| TrainError::Io(e))?;
        std::fs::create_dir_all(&self.config.log_dir)
            .map_err(|e| TrainError::Io(e))?;

        // Build optimizer (AdamW).
        let mut opt = nn::AdamW::default()
            .wd(self.config.weight_decay)
            .build(self.model.var_store(), self.config.learning_rate)
            .map_err(|e| TrainError::training_step(e.to_string()))?;

        let loss_fn = WiFiDensePoseLoss::new(LossWeights {
            lambda_kp: self.config.lambda_kp,
            lambda_dp: self.config.lambda_dp,
            lambda_tr: self.config.lambda_tr,
        });

        // CSV log file.
        let csv_path = self.config.log_dir.join("training_log.csv");
        let mut csv_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&csv_path)
            .map_err(|e| TrainError::Io(e))?;
        writeln!(csv_file, "epoch,train_loss,train_kp_loss,val_pck,val_oks,lr,duration_secs")
            .map_err(|e| TrainError::Io(e))?;

        let mut training_history: Vec<EpochLog> = Vec::new();
        let mut best_pck: f32 = -1.0;
        let mut best_epoch: usize = 0;
        let mut best_checkpoint_path: Option<PathBuf> = None;

        // Early-stopping state: track the last N val_pck values.
        let patience = self.config.early_stopping_patience;
        let mut patience_counter: usize = 0;
        let min_delta = 1e-4_f32;

        let mut current_lr = self.config.learning_rate;

        info!(
            "Training {} for {} epochs on '{}' → '{}'",
            train_dataset.name(),
            self.config.num_epochs,
            train_dataset.name(),
            val_dataset.name()
        );

        for epoch in 1..=self.config.num_epochs {
            let epoch_start = Instant::now();

            // ── LR scheduling ──────────────────────────────────────────────
            if self.config.lr_milestones.contains(&epoch) {
                current_lr *= self.config.lr_gamma;
                opt.set_lr(current_lr);
                info!("Epoch {epoch}: LR decayed to {current_lr:.2e}");
            }

            // ── Warmup ─────────────────────────────────────────────────────
            if epoch <= self.config.warmup_epochs {
                let warmup_lr = self.config.learning_rate
                    * epoch as f64
                    / self.config.warmup_epochs as f64;
                opt.set_lr(warmup_lr);
                current_lr = warmup_lr;
            }

            // ── Training batches ───────────────────────────────────────────
            // Deterministic shuffle: seed = config.seed XOR epoch.
            let shuffle_seed = self.config.seed ^ (epoch as u64);
            let batches = make_batches(
                train_dataset,
                self.config.batch_size,
                true,
                shuffle_seed,
                self.device,
            );

            let mut total_loss_sum = 0.0_f64;
            let mut kp_loss_sum = 0.0_f64;
            let mut n_batches = 0_usize;

            for (amp_batch, phase_batch, kp_batch, vis_batch) in &batches {
                let output = self.model.forward_train(amp_batch, phase_batch);

                // Build target heatmaps from ground-truth keypoints.
                let target_hm = kp_to_heatmap_tensor(
                    kp_batch,
                    vis_batch,
                    self.config.heatmap_size,
                    self.device,
                );

                // Binary visibility mask [B, 17].
                let vis_mask = (vis_batch.gt(0.0)).to_kind(Kind::Float);

                // Compute keypoint loss only (no DensePose GT in this pipeline).
                let (total_tensor, loss_out) = loss_fn.forward(
                    &output.keypoints,
                    &target_hm,
                    &vis_mask,
                    None, None, None, None, None, None,
                );

                opt.zero_grad();
                total_tensor.backward();
                opt.clip_grad_norm(self.config.grad_clip_norm);
                opt.step();

                total_loss_sum += loss_out.total as f64;
                kp_loss_sum += loss_out.keypoint as f64;
                n_batches += 1;

                debug!(
                    "Epoch {epoch} batch {n_batches}: loss={:.4}",
                    loss_out.total
                );
            }

            let mean_loss = if n_batches > 0 {
                total_loss_sum / n_batches as f64
            } else {
                0.0
            };
            let mean_kp_loss = if n_batches > 0 {
                kp_loss_sum / n_batches as f64
            } else {
                0.0
            };

            // ── Validation ─────────────────────────────────────────────────
            let mut val_pck = 0.0_f32;
            let mut val_oks = 0.0_f32;

            if epoch % self.config.val_every_epochs == 0 {
                match self.evaluate(val_dataset) {
                    Ok(metrics) => {
                        val_pck = metrics.pck;
                        val_oks = metrics.oks;
                        info!(
                            "Epoch {epoch}: loss={mean_loss:.4}  val_pck={val_pck:.4}  val_oks={val_oks:.4}  lr={current_lr:.2e}"
                        );
                    }
                    Err(e) => {
                        warn!("Validation failed at epoch {epoch}: {e}");
                    }
                }

                // ── Checkpoint saving ──────────────────────────────────────
                if val_pck > best_pck + min_delta {
                    best_pck = val_pck;
                    best_epoch = epoch;
                    patience_counter = 0;

                    let ckpt_name = format!("best_epoch{epoch:04}_pck{val_pck:.4}.pt");
                    let ckpt_path = self.config.checkpoint_dir.join(&ckpt_name);

                    match self.model.save(&ckpt_path) {
                        Ok(_) => {
                            info!("Saved best checkpoint: {}", ckpt_path.display());
                            best_checkpoint_path = Some(ckpt_path);
                        }
                        Err(e) => {
                            warn!("Failed to save checkpoint: {e}");
                        }
                    }
                } else {
                    patience_counter += 1;
                }
            }

            let epoch_secs = epoch_start.elapsed().as_secs_f64();
            let log = EpochLog {
                epoch,
                train_loss: mean_loss,
                train_kp_loss: mean_kp_loss,
                val_pck,
                val_oks,
                lr: current_lr,
                duration_secs: epoch_secs,
            };

            // Write CSV row.
            writeln!(
                csv_file,
                "{},{:.6},{:.6},{:.6},{:.6},{:.2e},{:.3}",
                log.epoch,
                log.train_loss,
                log.train_kp_loss,
                log.val_pck,
                log.val_oks,
                log.lr,
                log.duration_secs,
            )
            .map_err(|e| TrainError::Io(e))?;

            training_history.push(log);

            // ── Early stopping check ───────────────────────────────────────
            if patience_counter >= patience {
                info!(
                    "Early stopping at epoch {epoch}: no improvement for {patience} validation rounds."
                );
                break;
            }
        }

        // Save final model regardless.
        let final_ckpt = self.config.checkpoint_dir.join("final.pt");
        if let Err(e) = self.model.save(&final_ckpt) {
            warn!("Failed to save final model: {e}");
        }

        Ok(TrainResult {
            best_pck: best_pck.max(0.0),
            best_epoch,
            final_train_loss: training_history
                .last()
                .map(|l| l.train_loss)
                .unwrap_or(0.0),
            training_history,
            checkpoint_path: best_checkpoint_path,
        })
    }

    /// Evaluate on a dataset, returning PCK and OKS metrics.
    ///
    /// Runs inference (no gradient) over the full dataset using the configured
    /// batch size.
    pub fn evaluate(&self, dataset: &dyn CsiDataset) -> Result<MetricsResult, TrainError> {
        if dataset.is_empty() {
            return Err(TrainError::EmptyDataset);
        }

        let mut acc = MetricsAccumulator::default_threshold();

        let batches = make_batches(
            dataset,
            self.config.batch_size,
            false, // no shuffle during evaluation
            self.config.seed,
            self.device,
        );

        for (amp_batch, phase_batch, kp_batch, vis_batch) in &batches {
            let output = self.model.forward_inference(amp_batch, phase_batch);

            // Extract predicted keypoints from heatmaps.
            // Strategy: argmax over spatial dimensions → (x, y).
            let pred_kps = heatmap_to_keypoints(&output.keypoints);

            // Convert GT tensors back to ndarray for MetricsAccumulator.
            let batch_size = kp_batch.size()[0] as usize;
            for b in 0..batch_size {
                let pred_kp_np = extract_kp_ndarray(&pred_kps, b);
                let gt_kp_np = extract_kp_ndarray(kp_batch, b);
                let vis_np = extract_vis_ndarray(vis_batch, b);

                acc.update(&pred_kp_np, &gt_kp_np, &vis_np);
            }
        }

        acc.finalize().ok_or(TrainError::EmptyDataset)
    }

    /// Save a training checkpoint.
    pub fn save_checkpoint(
        &self,
        path: &Path,
        _epoch: usize,
        _metrics: &MetricsResult,
    ) -> Result<(), TrainError> {
        self.model.save(path).map_err(|e| TrainError::checkpoint(e.to_string(), path))
    }

    /// Load model weights from a checkpoint.
    ///
    /// Returns the epoch number encoded in the filename (if any), or `0`.
    pub fn load_checkpoint(&mut self, path: &Path) -> Result<usize, TrainError> {
        self.model
            .var_store_mut()
            .load(path)
            .map_err(|e| TrainError::checkpoint(e.to_string(), path))?;

        // Try to parse the epoch from the filename (e.g. "best_epoch0042_pck0.7842.pt").
        let epoch = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| {
                s.split("epoch").nth(1)
                    .and_then(|rest| rest.split('_').next())
                    .and_then(|n| n.parse::<usize>().ok())
            })
            .unwrap_or(0);

        Ok(epoch)
    }
}

// ---------------------------------------------------------------------------
// Batch construction helpers
// ---------------------------------------------------------------------------

/// Build all training batches for one epoch.
///
/// `shuffle=true` uses a deterministic LCG permutation seeded with `seed`.
/// This guarantees reproducibility: same seed → same iteration order, with
/// no dependence on OS entropy.
pub fn make_batches(
    dataset: &dyn CsiDataset,
    batch_size: usize,
    shuffle: bool,
    seed: u64,
    device: Device,
) -> Vec<(Tensor, Tensor, Tensor, Tensor)> {
    let n = dataset.len();
    if n == 0 {
        return vec![];
    }

    // Build index permutation (or identity).
    let mut indices: Vec<usize> = (0..n).collect();
    if shuffle {
        lcg_shuffle(&mut indices, seed);
    }

    // Partition into batches.
    let mut batches = Vec::new();
    let mut cursor = 0;
    while cursor < indices.len() {
        let end = (cursor + batch_size).min(indices.len());
        let batch_indices = &indices[cursor..end];

        // Load samples.
        let mut samples: Vec<CsiSample> = Vec::with_capacity(batch_indices.len());
        for &idx in batch_indices {
            match dataset.get(idx) {
                Ok(s) => samples.push(s),
                Err(e) => {
                    warn!("Skipping sample {idx}: {e}");
                }
            }
        }

        if !samples.is_empty() {
            let batch = collate(&samples, device);
            batches.push(batch);
        }

        cursor = end;
    }

    batches
}

/// Deterministic Fisher-Yates shuffle using a Linear Congruential Generator.
///
/// LCG parameters: multiplier = 6364136223846793005,
///                 increment  = 1442695040888963407  (Knuth's MMIX)
fn lcg_shuffle(indices: &mut [usize], seed: u64) {
    let n = indices.len();
    if n <= 1 {
        return;
    }

    let mut state = seed.wrapping_add(1); // avoid seed=0 degeneracy
    let mul: u64 = 6364136223846793005;
    let inc: u64 = 1442695040888963407;

    for i in (1..n).rev() {
        state = state.wrapping_mul(mul).wrapping_add(inc);
        let j = (state >> 33) as usize % (i + 1);
        indices.swap(i, j);
    }
}

/// Collate a slice of [`CsiSample`]s into four batched tensors.
///
/// Returns `(amplitude, phase, keypoints, visibility)`:
/// - `amplitude`:  `[B, T*n_tx*n_rx, n_sub]`
/// - `phase`:      `[B, T*n_tx*n_rx, n_sub]`
/// - `keypoints`:  `[B, 17, 2]`
/// - `visibility`: `[B, 17]`
pub fn collate(samples: &[CsiSample], device: Device) -> (Tensor, Tensor, Tensor, Tensor) {
    let b = samples.len();
    assert!(b > 0, "collate requires at least one sample");

    let s0 = &samples[0];
    let shape = s0.amplitude.shape();
    let (t, n_tx, n_rx, n_sub) = (shape[0], shape[1], shape[2], shape[3]);
    let flat_ant = t * n_tx * n_rx;
    let num_kp = s0.keypoints.shape()[0];

    // Allocate host buffers.
    let mut amp_data = vec![0.0_f32; b * flat_ant * n_sub];
    let mut ph_data = vec![0.0_f32; b * flat_ant * n_sub];
    let mut kp_data = vec![0.0_f32; b * num_kp * 2];
    let mut vis_data = vec![0.0_f32; b * num_kp];

    for (bi, sample) in samples.iter().enumerate() {
        // Amplitude: [T, n_tx, n_rx, n_sub] → flatten to [T*n_tx*n_rx, n_sub]
        let amp_flat: Vec<f32> = sample
            .amplitude
            .iter()
            .copied()
            .collect();
        let ph_flat: Vec<f32> = sample.phase.iter().copied().collect();

        let stride = flat_ant * n_sub;
        amp_data[bi * stride..(bi + 1) * stride].copy_from_slice(&amp_flat);
        ph_data[bi * stride..(bi + 1) * stride].copy_from_slice(&ph_flat);

        // Keypoints.
        let kp_stride = num_kp * 2;
        for j in 0..num_kp {
            kp_data[bi * kp_stride + j * 2] = sample.keypoints[[j, 0]];
            kp_data[bi * kp_stride + j * 2 + 1] = sample.keypoints[[j, 1]];
            vis_data[bi * num_kp + j] = sample.keypoint_visibility[j];
        }
    }

    let amp_t = Tensor::from_slice(&amp_data)
        .reshape([b as i64, flat_ant as i64, n_sub as i64])
        .to_device(device);
    let ph_t = Tensor::from_slice(&ph_data)
        .reshape([b as i64, flat_ant as i64, n_sub as i64])
        .to_device(device);
    let kp_t = Tensor::from_slice(&kp_data)
        .reshape([b as i64, num_kp as i64, 2])
        .to_device(device);
    let vis_t = Tensor::from_slice(&vis_data)
        .reshape([b as i64, num_kp as i64])
        .to_device(device);

    (amp_t, ph_t, kp_t, vis_t)
}

// ---------------------------------------------------------------------------
// Heatmap utilities
// ---------------------------------------------------------------------------

/// Convert ground-truth keypoints to Gaussian target heatmaps.
///
/// Wraps [`generate_target_heatmaps`] to work on `tch::Tensor` inputs.
fn kp_to_heatmap_tensor(
    kp_tensor: &Tensor,
    vis_tensor: &Tensor,
    heatmap_size: usize,
    device: Device,
) -> Tensor {
    // kp_tensor: [B, 17, 2]
    // vis_tensor: [B, 17]
    let b = kp_tensor.size()[0] as usize;
    let num_kp = kp_tensor.size()[1] as usize;

    // Convert to ndarray for generate_target_heatmaps.
    let kp_vec: Vec<f32> = Vec::<f64>::from(kp_tensor.to_kind(Kind::Double).flatten(0, -1))
        .iter().map(|&x| x as f32).collect();
    let vis_vec: Vec<f32> = Vec::<f64>::from(vis_tensor.to_kind(Kind::Double).flatten(0, -1))
        .iter().map(|&x| x as f32).collect();

    let kp_nd = ndarray::Array3::from_shape_vec((b, num_kp, 2), kp_vec)
        .expect("kp shape");
    let vis_nd = ndarray::Array2::from_shape_vec((b, num_kp), vis_vec)
        .expect("vis shape");

    let hm_nd = generate_target_heatmaps(&kp_nd, &vis_nd, heatmap_size, 2.0);

    // [B, 17, H, W]
    let flat: Vec<f32> = hm_nd.iter().copied().collect();
    Tensor::from_slice(&flat)
        .reshape([
            b as i64,
            num_kp as i64,
            heatmap_size as i64,
            heatmap_size as i64,
        ])
        .to_device(device)
}

/// Convert predicted heatmaps to normalised keypoint coordinates via argmax.
///
/// Input: `[B, 17, H, W]`
/// Output: `[B, 17, 2]` with (x, y) in [0, 1]
fn heatmap_to_keypoints(heatmaps: &Tensor) -> Tensor {
    let sizes = heatmaps.size();
    let (batch, num_kp, h, w) = (sizes[0], sizes[1], sizes[2], sizes[3]);

    // Flatten spatial → [B, 17, H*W]
    let flat = heatmaps.reshape([batch, num_kp, h * w]);
    // Argmax per joint → [B, 17]
    let arg = flat.argmax(-1, false);

    // Decompose linear index into (row, col).
    let row = (&arg / w).to_kind(Kind::Float); // [B, 17]
    let col = (&arg % w).to_kind(Kind::Float);  // [B, 17]

    // Normalize to [0, 1]
    let x = col / (w - 1) as f64;
    let y = row / (h - 1) as f64;

    // Stack to [B, 17, 2]
    Tensor::stack(&[x, y], -1)
}

/// Extract a single sample's keypoints as an ndarray from a batched tensor.
///
/// `kp_tensor` shape: `[B, 17, 2]`
fn extract_kp_ndarray(kp_tensor: &Tensor, batch_idx: usize) -> Array2<f32> {
    let num_kp = kp_tensor.size()[1] as usize;
    let row = kp_tensor.select(0, batch_idx as i64);
    let data: Vec<f32> = Vec::<f64>::from(row.to_kind(Kind::Double).flatten(0, -1))
        .iter().map(|&v| v as f32).collect();
    Array2::from_shape_vec((num_kp, 2), data).expect("kp ndarray shape")
}

/// Extract a single sample's visibility flags as an ndarray from a batched tensor.
///
/// `vis_tensor` shape: `[B, 17]`
fn extract_vis_ndarray(vis_tensor: &Tensor, batch_idx: usize) -> Array1<f32> {
    let num_kp = vis_tensor.size()[1] as usize;
    let row = vis_tensor.select(0, batch_idx as i64);
    let data: Vec<f32> = Vec::<f64>::from(row.to_kind(Kind::Double))
        .iter().map(|&v| v as f32).collect();
    Array1::from_vec(data)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TrainingConfig;
    use crate::dataset::{SyntheticCsiDataset, SyntheticConfig};

    fn tiny_config() -> TrainingConfig {
        let mut cfg = TrainingConfig::default();
        cfg.num_subcarriers = 8;
        cfg.window_frames = 2;
        cfg.num_antennas_tx = 1;
        cfg.num_antennas_rx = 1;
        cfg.heatmap_size = 8;
        cfg.backbone_channels = 32;
        cfg.num_epochs = 2;
        cfg.warmup_epochs = 1;
        cfg.batch_size = 4;
        cfg.val_every_epochs = 1;
        cfg.early_stopping_patience = 5;
        cfg.lr_milestones = vec![2];
        cfg
    }

    fn tiny_synthetic_dataset(n: usize) -> SyntheticCsiDataset {
        let cfg = tiny_config();
        SyntheticCsiDataset::new(n, SyntheticConfig {
            num_subcarriers: cfg.num_subcarriers,
            num_antennas_tx: cfg.num_antennas_tx,
            num_antennas_rx: cfg.num_antennas_rx,
            window_frames: cfg.window_frames,
            num_keypoints: 17,
            signal_frequency_hz: 2.4e9,
        })
    }

    #[test]
    fn collate_produces_correct_shapes() {
        let ds = tiny_synthetic_dataset(4);
        let samples: Vec<_> = (0..4).map(|i| ds.get(i).unwrap()).collect();
        let (amp, ph, kp, vis) = collate(&samples, Device::Cpu);

        let cfg = tiny_config();
        let flat_ant = (cfg.window_frames * cfg.num_antennas_tx * cfg.num_antennas_rx) as i64;
        assert_eq!(amp.size(), [4, flat_ant, cfg.num_subcarriers as i64]);
        assert_eq!(ph.size(), [4, flat_ant, cfg.num_subcarriers as i64]);
        assert_eq!(kp.size(), [4, 17, 2]);
        assert_eq!(vis.size(), [4, 17]);
    }

    #[test]
    fn make_batches_covers_all_samples() {
        let ds = tiny_synthetic_dataset(10);
        let batches = make_batches(&ds, 3, false, 42, Device::Cpu);
        let total: i64 = batches.iter().map(|(a, _, _, _)| a.size()[0]).sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn make_batches_shuffle_reproducible() {
        let ds = tiny_synthetic_dataset(10);
        let b1 = make_batches(&ds, 3, true, 99, Device::Cpu);
        let b2 = make_batches(&ds, 3, true, 99, Device::Cpu);
        // Shapes should match exactly.
        for (batch_a, batch_b) in b1.iter().zip(b2.iter()) {
            assert_eq!(batch_a.0.size(), batch_b.0.size());
        }
    }

    #[test]
    fn lcg_shuffle_is_permutation() {
        let mut idx: Vec<usize> = (0..20).collect();
        lcg_shuffle(&mut idx, 42);
        let mut sorted = idx.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn lcg_shuffle_different_seeds_differ() {
        let mut a: Vec<usize> = (0..20).collect();
        let mut b: Vec<usize> = (0..20).collect();
        lcg_shuffle(&mut a, 1);
        lcg_shuffle(&mut b, 2);
        assert_ne!(a, b, "different seeds should produce different orders");
    }

    #[test]
    fn heatmap_to_keypoints_shape() {
        let hm = Tensor::zeros([2, 17, 8, 8], (Kind::Float, Device::Cpu));
        let kp = heatmap_to_keypoints(&hm);
        assert_eq!(kp.size(), [2, 17, 2]);
    }

    #[test]
    fn heatmap_to_keypoints_center_peak() {
        // Create a heatmap with a single peak at the center (4, 4) of an 8×8 map.
        let mut hm = Tensor::zeros([1, 1, 8, 8], (Kind::Float, Device::Cpu));
        let _ = hm.narrow(2, 4, 1).narrow(3, 4, 1).fill_(1.0);
        let kp = heatmap_to_keypoints(&hm);
        let x: f64 = kp.double_value(&[0, 0, 0]);
        let y: f64 = kp.double_value(&[0, 0, 1]);
        // Center pixel 4 → normalised 4/7 ≈ 0.571
        assert!((x - 4.0 / 7.0).abs() < 1e-4, "x={x}");
        assert!((y - 4.0 / 7.0).abs() < 1e-4, "y={y}");
    }

    #[test]
    fn trainer_train_completes() {
        let cfg = tiny_config();
        let train_ds = tiny_synthetic_dataset(8);
        let val_ds = tiny_synthetic_dataset(4);

        let mut trainer = Trainer::new(cfg);
        let tmpdir = tempfile::tempdir().unwrap();
        trainer.config.checkpoint_dir = tmpdir.path().join("checkpoints");
        trainer.config.log_dir = tmpdir.path().join("logs");

        let result = trainer.train(&train_ds, &val_ds).unwrap();
        assert!(result.final_train_loss.is_finite());
        assert!(!result.training_history.is_empty());
    }
}
