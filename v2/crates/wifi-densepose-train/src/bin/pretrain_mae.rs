//! `pretrain-mae` — drive the MERIDIAN CSI masked-autoencoder pre-train on a
//! deterministic `SyntheticCsiDataset` (ADR-027 §2.0, prototype iteration 2).
//!
//! This is the *prototype* driver — it exercises the full pre-train loop
//! (mask → encode visible → reconstruct masked amplitude+phase → optimiser
//! step) end-to-end on synthetic CSI. Real cross-domain pre-training (iter 3+)
//! ingests heterogeneous capture — MM-Fi / Wi-Pose / `data/recordings/` /
//! multi-band virtual sub-carriers — and runs on GPU (`scripts/gcloud-train.sh`
//! / the cognitum project).
//!
//! ```text
//! cargo run -p wifi-densepose-train --features tch-backend --bin pretrain-mae -- --epochs 5
//! ```
//!
//! Only compiled with `--features tch-backend` (see Cargo.toml `required-features`).

use clap::Parser;
use tch::nn::OptimizerConfig;
use tch::{nn, Device};

use wifi_densepose_train::csi_mae::model::{pretrain_step, CsiMae, MaeBatch};
use wifi_densepose_train::csi_mae::{MaeConfig, MaskStrategy, TokenLayout};
use wifi_densepose_train::dataset::{CsiDataset, SyntheticConfig, SyntheticCsiDataset};

/// MERIDIAN CSI masked-autoencoder pre-train (prototype, synthetic data).
#[derive(Parser, Debug)]
#[command(name = "pretrain-mae", version, about)]
struct Cli {
    /// Number of epochs over the synthetic dataset.
    #[arg(long, default_value_t = 5)]
    epochs: usize,
    /// Mini-batch size (windows per optimiser step).
    #[arg(long, default_value_t = 8)]
    batch: usize,
    /// Number of synthetic samples to generate.
    #[arg(long, default_value_t = 256)]
    samples: usize,
    /// Adam learning rate.
    #[arg(long, default_value_t = 1e-3)]
    lr: f64,
    /// Fraction of tokens masked per window.
    #[arg(long, default_value_t = 0.75)]
    mask_ratio: f64,
    /// Optional path to save the pre-trained variable store (`.ot`).
    #[arg(long)]
    save: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let _ = tracing_subscriber::fmt::try_init();

    let ds = SyntheticCsiDataset::new(cli.samples, SyntheticConfig::default());
    if ds.len() < cli.batch {
        anyhow::bail!("need at least --batch ({}) samples, have {}", cli.batch, ds.len());
    }
    let s0 = ds.get(0)?;
    let layout = TokenLayout::from_window(s0.amplitude.view());
    let n_tokens = layout.n_tokens as i64;

    let mut cfg = MaeConfig::default();
    cfg.token_dim = layout.token_dim;
    cfg.mask_ratio = cli.mask_ratio;
    cfg.validate().map_err(anyhow::Error::msg)?;

    let device = Device::cuda_if_available();
    let vs = nn::VarStore::new(device);
    let model = CsiMae::new(&vs.root(), &cfg, n_tokens);
    let mut opt = nn::Adam::default().build(&vs, cli.lr)?;

    println!(
        "pretrain-mae: device={device:?} n_tokens={n_tokens} token_dim={} V={} M={} samples={} batch={} epochs={} lr={} mask_ratio={}",
        cfg.token_dim, model.n_visible, model.n_masked, cli.samples, cli.batch, cli.epochs, cli.lr, cli.mask_ratio
    );

    let mut step: u64 = 0;
    for epoch in 0..cli.epochs {
        let mut epoch_loss = 0.0_f64;
        let mut nb = 0_usize;
        let mut i = 0_usize;
        while i + cli.batch <= ds.len() {
            let mut windows = Vec::with_capacity(cli.batch);
            for j in i..i + cli.batch {
                let s = ds.get(j)?;
                windows.push((s.amplitude, s.phase));
            }
            let seed = step.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xC511_0027;
            let batch = MaeBatch::from_windows(&windows, &cfg, seed, MaskStrategy::InfoGuided, device)
                .map_err(anyhow::Error::msg)?;
            let loss = pretrain_step(&model, &mut opt, &batch);
            if !loss.is_finite() {
                anyhow::bail!("non-finite loss at epoch {epoch} step {step}");
            }
            epoch_loss += loss;
            nb += 1;
            step += 1;
            i += cli.batch;
        }
        let avg = if nb > 0 { epoch_loss / nb as f64 } else { f64::NAN };
        println!("epoch {epoch}: avg reconstruction loss = {avg:.6}  ({nb} batches)");
    }

    if let Some(path) = cli.save {
        vs.save(&path)?;
        println!("saved pre-trained variable store → {path}");
    }
    Ok(())
}
