//! Handler for `--train` CLI mode.

use std::path::PathBuf;
use crate::cli::Args;
use crate::dataset;
use crate::graph_transformer;
use crate::rvf_container::{RvfBuilder, VitalSignConfig};
use crate::trainer;

/// Run WiFi-DensePose model training.
pub fn run_train(args: &Args) {
    eprintln!("=== WiFi-DensePose Training Mode ===");

    let ds_path = args
        .dataset
        .clone()
        .unwrap_or_else(|| PathBuf::from("data"));
    let source = match args.dataset_type.as_str() {
        "wipose" => dataset::DataSource::WiPose(ds_path.clone()),
        _ => dataset::DataSource::MmFi(ds_path.clone()),
    };
    let pipeline = dataset::DataPipeline::new(dataset::DataConfig {
        source,
        ..Default::default()
    });

    let generate_synthetic = || -> Vec<dataset::TrainingSample> {
        (0..50)
            .map(|i| {
                let csi: Vec<Vec<f32>> = (0..4)
                    .map(|a| {
                        (0..56)
                            .map(|s| ((i * 7 + a * 13 + s) as f32 * 0.31).sin() * 0.5)
                            .collect()
                    })
                    .collect();
                let mut kps = [(0.0f32, 0.0f32, 1.0f32); 17];
                for (k, kp) in kps.iter_mut().enumerate() {
                    kp.0 = (k as f32 * 0.1 + i as f32 * 0.02).sin() * 100.0 + 320.0;
                    kp.1 = (k as f32 * 0.15 + i as f32 * 0.03).cos() * 80.0 + 240.0;
                }
                dataset::TrainingSample {
                    csi_window: csi,
                    pose_label: dataset::PoseLabel {
                        keypoints: kps,
                        body_parts: Vec::new(),
                        confidence: 1.0,
                    },
                    source: "synthetic",
                }
            })
            .collect()
    };

    let samples = match pipeline.load() {
        Ok(s) if !s.is_empty() => {
            eprintln!("Loaded {} samples from {}", s.len(), ds_path.display());
            s
        }
        Ok(_) => {
            eprintln!("No samples found at {}. Using synthetic data.", ds_path.display());
            generate_synthetic()
        }
        Err(e) => {
            eprintln!("Failed to load dataset: {e}. Using synthetic data.");
            generate_synthetic()
        }
    };

    let trainer_samples: Vec<trainer::TrainingSample> =
        samples.iter().map(trainer::from_dataset_sample).collect();

    let split = (trainer_samples.len() * 4) / 5;
    let (train_data, val_data) = trainer_samples.split_at(split.max(1));
    eprintln!(
        "Train: {} samples, Val: {} samples",
        train_data.len(),
        val_data.len()
    );

    let n_subcarriers = train_data
        .first()
        .and_then(|s| s.csi_features.first())
        .map(|f| f.len())
        .unwrap_or(56);
    let tf_config = graph_transformer::TransformerConfig {
        n_subcarriers,
        n_keypoints: 17,
        d_model: 64,
        n_heads: 4,
        n_gnn_layers: 2,
    };
    let transformer = graph_transformer::CsiToPoseTransformer::new(tf_config);
    eprintln!("Transformer params: {}", transformer.param_count());

    let trainer_config = trainer::TrainerConfig {
        epochs: args.epochs,
        batch_size: 8,
        lr: 0.001,
        warmup_epochs: 5,
        min_lr: 1e-6,
        early_stop_patience: 20,
        checkpoint_every: 10,
        ..Default::default()
    };
    let mut t = trainer::Trainer::with_transformer(trainer_config, transformer);

    eprintln!("Starting training for {} epochs...", args.epochs);
    let result = t.run_training(train_data, val_data);
    eprintln!("Training complete in {:.1}s", result.total_time_secs);
    eprintln!(
        "  Best epoch: {}, pck_raw@0.2: {:.4}, oks_map(area=1.0 proxy): {:.4}",
        result.best_epoch, result.best_pck, result.best_oks
    );

    if let Some(ref ckpt_dir) = args.checkpoint_dir {
        let _ = std::fs::create_dir_all(ckpt_dir);
        let ckpt_path = ckpt_dir.join("best_checkpoint.json");
        let ckpt = t.checkpoint();
        match ckpt.save_to_file(&ckpt_path) {
            Ok(()) => eprintln!("Checkpoint saved to {}", ckpt_path.display()),
            Err(e) => eprintln!("Failed to save checkpoint: {e}"),
        }
    }

    t.sync_transformer_weights();
    if let Some(ref save_path) = args.save_rvf {
        eprintln!("Saving trained model to RVF: {}", save_path.display());
        let weights = t.params().to_vec();
        let mut builder = RvfBuilder::new();
        builder.add_manifest(
            "wifi-densepose-trained",
            env!("CARGO_PKG_VERSION"),
            "WiFi DensePose trained model weights",
        );
        builder.add_metadata(&serde_json::json!({
            "training": {
                "epochs": args.epochs,
                "best_epoch": result.best_epoch,
                "best_pck": result.best_pck,
                "best_oks": result.best_oks,
                "n_train_samples": train_data.len(),
                "n_val_samples": val_data.len(),
                "n_subcarriers": n_subcarriers,
                "param_count": weights.len(),
            },
        }));
        builder.add_vital_config(&VitalSignConfig::default());
        builder.add_weights(&weights);
        match builder.write_to_file(save_path) {
            Ok(()) => eprintln!(
                "RVF saved ({} params, {} bytes)",
                weights.len(),
                weights.len() * 4
            ),
            Err(e) => eprintln!("Failed to save RVF: {e}"),
        }
    }
}

