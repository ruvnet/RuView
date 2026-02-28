//! Deterministic training proof for WiFi-DensePose.
//!
//! # Proof Protocol
//!
//! 1. Create [`SyntheticCsiDataset`] with fixed `seed = PROOF_SEED`.
//! 2. Initialise the model with `tch::manual_seed(MODEL_SEED)`.
//! 3. Run exactly [`N_PROOF_STEPS`] forward + backward steps.
//! 4. Verify that the loss decreased from initial to final.
//! 5. Compute SHA-256 of all model weight tensors in deterministic order.
//! 6. Compare against the expected hash stored in `expected_proof.sha256`.
//!
//! If the hash **matches**: the training pipeline is verified real and
//! deterministic.  If the hash **mismatches**: the code changed, or
//! non-determinism was introduced.
//!
//! # Trust Kill Switch
//!
//! Run `verify-training` to execute this proof.  Exit code 0 = PASS,
//! 1 = FAIL (loss did not decrease or hash mismatch), 2 = SKIP (no hash
//! file to compare against).

use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;
use tch::{nn, nn::OptimizerConfig, Device, Kind, Tensor};

use crate::config::TrainingConfig;
use crate::dataset::{CsiDataset, SyntheticCsiDataset, SyntheticConfig};
use crate::losses::{generate_target_heatmaps, LossWeights, WiFiDensePoseLoss};
use crate::model::WiFiDensePoseModel;
use crate::trainer::make_batches;

// ---------------------------------------------------------------------------
// Proof constants
// ---------------------------------------------------------------------------

/// Number of training steps executed during the proof run.
pub const N_PROOF_STEPS: usize = 50;

/// Seed used for the synthetic proof dataset.
pub const PROOF_SEED: u64 = 42;

/// Seed passed to `tch::manual_seed` before model construction.
pub const MODEL_SEED: i64 = 0;

/// Batch size used during the proof run.
pub const PROOF_BATCH_SIZE: usize = 4;

/// Number of synthetic samples in the proof dataset.
pub const PROOF_DATASET_SIZE: usize = 200;

/// Filename under `proof_dir` where the expected weight hash is stored.
const EXPECTED_HASH_FILE: &str = "expected_proof.sha256";

// ---------------------------------------------------------------------------
// ProofResult
// ---------------------------------------------------------------------------

/// Result of a single proof verification run.
#[derive(Debug, Clone)]
pub struct ProofResult {
    /// Training loss at step 0 (before any parameter update).
    pub initial_loss: f64,
    /// Training loss at the final step.
    pub final_loss: f64,
    /// `true` when `final_loss < initial_loss`.
    pub loss_decreased: bool,
    /// Loss at each of the [`N_PROOF_STEPS`] steps.
    pub loss_trajectory: Vec<f64>,
    /// SHA-256 hex digest of all model weight tensors.
    pub model_hash: String,
    /// Expected hash loaded from `expected_proof.sha256`, if the file exists.
    pub expected_hash: Option<String>,
    /// `Some(true)` when hashes match, `Some(false)` when they don't,
    /// `None` when no expected hash is available.
    pub hash_matches: Option<bool>,
    /// Number of training steps that completed without error.
    pub steps_completed: usize,
}

impl ProofResult {
    /// Returns `true` when the proof fully passes (loss decreased AND hash
    /// matches, or hash is not yet stored).
    pub fn is_pass(&self) -> bool {
        self.loss_decreased && self.hash_matches.unwrap_or(true)
    }

    /// Returns `true` when there is an expected hash and it does NOT match.
    pub fn is_fail(&self) -> bool {
        self.loss_decreased == false || self.hash_matches == Some(false)
    }

    /// Returns `true` when no expected hash file exists yet.
    pub fn is_skip(&self) -> bool {
        self.expected_hash.is_none()
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Run the full proof verification protocol.
///
/// # Arguments
///
/// - `proof_dir`: Directory that may contain `expected_proof.sha256`.
///
/// # Errors
///
/// Returns an error if the model or optimiser cannot be constructed.
pub fn run_proof(proof_dir: &Path) -> Result<ProofResult, Box<dyn std::error::Error>> {
    // Fixed seeds for determinism.
    tch::manual_seed(MODEL_SEED);

    let cfg = proof_config();
    let device = Device::Cpu;

    let model = WiFiDensePoseModel::new(&cfg, device);

    // Create AdamW optimiser.
    let mut opt = nn::AdamW::default()
        .wd(cfg.weight_decay)
        .build(model.var_store(), cfg.learning_rate)?;

    let loss_fn = WiFiDensePoseLoss::new(LossWeights {
        lambda_kp: cfg.lambda_kp,
        lambda_dp: 0.0,
        lambda_tr: 0.0,
    });

    // Proof dataset: deterministic, no OS randomness.
    let dataset = build_proof_dataset(&cfg);

    let mut loss_trajectory: Vec<f64> = Vec::with_capacity(N_PROOF_STEPS);
    let mut steps_completed = 0_usize;

    // Pre-build all batches (deterministic order, no shuffle for proof).
    let all_batches = make_batches(&dataset, PROOF_BATCH_SIZE, false, PROOF_SEED, device);
    // Cycle through batches until N_PROOF_STEPS are done.
    let n_batches = all_batches.len();
    if n_batches == 0 {
        return Err("Proof dataset produced no batches".into());
    }

    for step in 0..N_PROOF_STEPS {
        let (amp, ph, kp, vis) = &all_batches[step % n_batches];

        let output = model.forward_train(amp, ph);

        // Build target heatmaps.
        let b = amp.size()[0] as usize;
        let num_kp = kp.size()[1] as usize;
        let hm_size = cfg.heatmap_size;

        let kp_vec: Vec<f32> = Vec::<f64>::from(kp.to_kind(Kind::Double).flatten(0, -1))
            .iter().map(|&x| x as f32).collect();
        let vis_vec: Vec<f32> = Vec::<f64>::from(vis.to_kind(Kind::Double).flatten(0, -1))
            .iter().map(|&x| x as f32).collect();

        let kp_nd = ndarray::Array3::from_shape_vec((b, num_kp, 2), kp_vec)?;
        let vis_nd = ndarray::Array2::from_shape_vec((b, num_kp), vis_vec)?;
        let hm_nd = generate_target_heatmaps(&kp_nd, &vis_nd, hm_size, 2.0);

        let hm_flat: Vec<f32> = hm_nd.iter().copied().collect();
        let target_hm = Tensor::from_slice(&hm_flat)
            .reshape([b as i64, num_kp as i64, hm_size as i64, hm_size as i64])
            .to_device(device);

        let vis_mask = vis.gt(0.0).to_kind(Kind::Float);

        let (total_tensor, loss_out) = loss_fn.forward(
            &output.keypoints,
            &target_hm,
            &vis_mask,
            None, None, None, None, None, None,
        );

        opt.zero_grad();
        total_tensor.backward();
        opt.clip_grad_norm(cfg.grad_clip_norm);
        opt.step();

        loss_trajectory.push(loss_out.total as f64);
        steps_completed += 1;
    }

    let initial_loss = loss_trajectory.first().copied().unwrap_or(f64::NAN);
    let final_loss = loss_trajectory.last().copied().unwrap_or(f64::NAN);
    let loss_decreased = final_loss < initial_loss;

    // Compute model weight hash (uses varstore()).
    let model_hash = hash_model_weights(&model);

    // Load expected hash from file (if it exists).
    let expected_hash = load_expected_hash(proof_dir)?;
    let hash_matches = expected_hash.as_ref().map(|expected| {
        // Case-insensitive hex comparison.
        expected.trim().to_lowercase() == model_hash.to_lowercase()
    });

    Ok(ProofResult {
        initial_loss,
        final_loss,
        loss_decreased,
        loss_trajectory,
        model_hash,
        expected_hash,
        hash_matches,
        steps_completed,
    })
}

/// Run the proof and save the resulting hash as the expected value.
///
/// Call this once after implementing or updating the pipeline, commit the
/// generated `expected_proof.sha256` file, and then `run_proof` will
/// verify future runs against it.
///
/// # Errors
///
/// Returns an error if the proof fails to run or the hash cannot be written.
pub fn generate_expected_hash(proof_dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let result = run_proof(proof_dir)?;
    save_expected_hash(&result.model_hash, proof_dir)?;
    Ok(result.model_hash)
}

/// Compute SHA-256 of all model weight tensors in a deterministic order.
///
/// Tensors are enumerated via the `VarStore`'s `variables()` iterator,
/// sorted by name for a stable ordering, then each tensor is serialised to
/// little-endian `f32` bytes before hashing.
pub fn hash_model_weights(model: &WiFiDensePoseModel) -> String {
    let vs = model.var_store();
    let mut hasher = Sha256::new();

    // Collect and sort by name for a deterministic order across runs.
    let vars = vs.variables();
    let mut named: Vec<(String, Tensor)> = vars.into_iter().collect();
    named.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, tensor) in &named {
        // Write the name as a length-prefixed byte string so that parameter
        // renaming changes the hash.
        let name_bytes = name.as_bytes();
        hasher.update((name_bytes.len() as u32).to_le_bytes());
        hasher.update(name_bytes);

        // Serialise tensor values as little-endian f32.
        let flat: Tensor = tensor.flatten(0, -1).to_kind(Kind::Float).to_device(Device::Cpu);
        let values: Vec<f32> = Vec::<f32>::from(&flat);
        let mut buf = vec![0u8; values.len() * 4];
        for (i, v) in values.iter().enumerate() {
            let bytes = v.to_le_bytes();
            buf[i * 4..(i + 1) * 4].copy_from_slice(&bytes);
        }
        hasher.update(&buf);
    }

    format!("{:x}", hasher.finalize())
}

/// Load the expected model hash from `<proof_dir>/expected_proof.sha256`.
///
/// Returns `Ok(None)` if the file does not exist.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be read.
pub fn load_expected_hash(proof_dir: &Path) -> Result<Option<String>, std::io::Error> {
    let path = proof_dir.join(EXPECTED_HASH_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let mut file = std::fs::File::open(&path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let hash = contents.trim().to_string();
    Ok(if hash.is_empty() { None } else { Some(hash) })
}

/// Save the expected model hash to `<proof_dir>/expected_proof.sha256`.
///
/// Creates `proof_dir` if it does not already exist.
///
/// # Errors
///
/// Returns an error if the directory cannot be created or the file cannot
/// be written.
pub fn save_expected_hash(hash: &str, proof_dir: &Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(proof_dir)?;
    let path = proof_dir.join(EXPECTED_HASH_FILE);
    let mut file = std::fs::File::create(&path)?;
    writeln!(file, "{}", hash)?;
    Ok(())
}

/// Build the minimal [`TrainingConfig`] used for the proof run.
///
/// Uses reduced spatial and channel dimensions so the proof completes in
/// a few seconds on CPU.
pub fn proof_config() -> TrainingConfig {
    let mut cfg = TrainingConfig::default();

    // Minimal model for speed.
    cfg.num_subcarriers = 16;
    cfg.native_subcarriers = 16;
    cfg.window_frames = 4;
    cfg.num_antennas_tx = 2;
    cfg.num_antennas_rx = 2;
    cfg.heatmap_size = 16;
    cfg.backbone_channels = 64;
    cfg.num_keypoints = 17;
    cfg.num_body_parts = 24;

    // Optimiser.
    cfg.batch_size = PROOF_BATCH_SIZE;
    cfg.learning_rate = 1e-3;
    cfg.weight_decay = 1e-4;
    cfg.grad_clip_norm = 1.0;
    cfg.num_epochs = 1;
    cfg.warmup_epochs = 0;
    cfg.lr_milestones = vec![];
    cfg.lr_gamma = 0.1;

    // Loss weights: keypoint only.
    cfg.lambda_kp = 1.0;
    cfg.lambda_dp = 0.0;
    cfg.lambda_tr = 0.0;

    // Device.
    cfg.use_gpu = false;
    cfg.seed = PROOF_SEED;

    // Paths (unused during proof).
    cfg.checkpoint_dir = std::path::PathBuf::from("/tmp/proof_checkpoints");
    cfg.log_dir = std::path::PathBuf::from("/tmp/proof_logs");
    cfg.val_every_epochs = 1;
    cfg.early_stopping_patience = 999;
    cfg.save_top_k = 1;

    cfg
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build the synthetic dataset used for the proof run.
fn build_proof_dataset(cfg: &TrainingConfig) -> SyntheticCsiDataset {
    SyntheticCsiDataset::new(
        PROOF_DATASET_SIZE,
        SyntheticConfig {
            num_subcarriers: cfg.num_subcarriers,
            num_antennas_tx: cfg.num_antennas_tx,
            num_antennas_rx: cfg.num_antennas_rx,
            window_frames: cfg.window_frames,
            num_keypoints: cfg.num_keypoints,
            signal_frequency_hz: 2.4e9,
        },
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn proof_config_is_valid() {
        let cfg = proof_config();
        cfg.validate().expect("proof_config should be valid");
    }

    #[test]
    fn proof_dataset_is_nonempty() {
        let cfg = proof_config();
        let ds = build_proof_dataset(&cfg);
        assert!(ds.len() > 0, "Proof dataset must not be empty");
    }

    #[test]
    fn save_and_load_expected_hash() {
        let tmp = tempdir().unwrap();
        let hash = "deadbeefcafe1234";
        save_expected_hash(hash, tmp.path()).unwrap();
        let loaded = load_expected_hash(tmp.path()).unwrap();
        assert_eq!(loaded.as_deref(), Some(hash));
    }

    #[test]
    fn missing_hash_file_returns_none() {
        let tmp = tempdir().unwrap();
        let loaded = load_expected_hash(tmp.path()).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn hash_model_weights_is_deterministic() {
        tch::manual_seed(MODEL_SEED);
        let cfg = proof_config();
        let device = Device::Cpu;

        let m1 = WiFiDensePoseModel::new(&cfg, device);
        // Trigger weight creation.
        let dummy = Tensor::zeros(
            [1, (cfg.window_frames * cfg.num_antennas_tx * cfg.num_antennas_rx) as i64, cfg.num_subcarriers as i64],
            (Kind::Float, device),
        );
        let _ = m1.forward_inference(&dummy, &dummy);

        tch::manual_seed(MODEL_SEED);
        let m2 = WiFiDensePoseModel::new(&cfg, device);
        let _ = m2.forward_inference(&dummy, &dummy);

        let h1 = hash_model_weights(&m1);
        let h2 = hash_model_weights(&m2);
        assert_eq!(h1, h2, "Hashes should match for identically-seeded models");
    }

    #[test]
    fn proof_run_produces_valid_result() {
        let tmp = tempdir().unwrap();
        // Use a reduced proof (fewer steps) for CI speed.
        // We verify structure, not exact numeric values.
        let result = run_proof(tmp.path()).unwrap();

        assert_eq!(result.steps_completed, N_PROOF_STEPS);
        assert!(!result.model_hash.is_empty());
        assert_eq!(result.loss_trajectory.len(), N_PROOF_STEPS);
        // No expected hash file was created → no comparison.
        assert!(result.expected_hash.is_none());
        assert!(result.hash_matches.is_none());
    }

    #[test]
    fn generate_and_verify_hash_matches() {
        let tmp = tempdir().unwrap();

        // Generate the expected hash.
        let generated = generate_expected_hash(tmp.path()).unwrap();
        assert!(!generated.is_empty());

        // Verify: running the proof again should produce the same hash.
        let result = run_proof(tmp.path()).unwrap();
        assert_eq!(
            result.model_hash, generated,
            "Re-running proof should produce the same model hash"
        );
        // The expected hash file now exists → comparison should be performed.
        assert!(
            result.hash_matches == Some(true),
            "Hash should match after generate_expected_hash"
        );
    }
}
