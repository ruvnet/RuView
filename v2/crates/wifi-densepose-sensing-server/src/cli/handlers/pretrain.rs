//! Handlers for `--pretrain`, `--embed`, and `--build-index` CLI modes.

use std::path::PathBuf;
use crate::cli::Args;
use crate::dataset;
use crate::embedding;
use crate::graph_transformer;
use crate::rvf_container::{RvfBuilder, RvfReader};
use crate::trainer;

/// Run self-supervised contrastive pretraining.
pub fn run_pretrain(args: &Args) {
    eprintln!("=== WiFi-DensePose Contrastive Pretraining (ADR-024) ===");

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

    let generate_synthetic_windows = || -> Vec<Vec<Vec<f32>>> {
        (0..50)
            .map(|i| {
                (0..4)
                    .map(|a| {
                        (0..56)
                            .map(|s| ((i * 7 + a * 13 + s) as f32 * 0.31).sin() * 0.5)
                            .collect()
                    })
                    .collect()
            })
            .collect()
    };

    let csi_windows: Vec<Vec<Vec<f32>>> = match pipeline.load() {
        Ok(s) if !s.is_empty() => {
            eprintln!("Loaded {} samples from {}", s.len(), ds_path.display());
            s.into_iter().map(|s| s.csi_window).collect()
        }
        _ => {
            eprintln!("Using synthetic data for pretraining.");
            generate_synthetic_windows()
        }
    };

    let n_subcarriers = csi_windows
        .first()
        .and_then(|w| w.first())
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
        epochs: args.pretrain_epochs,
        batch_size: 8,
        lr: 0.001,
        warmup_epochs: 2,
        min_lr: 1e-6,
        early_stop_patience: args.pretrain_epochs + 1,
        pretrain_temperature: 0.07,
        ..Default::default()
    };
    let mut t = trainer::Trainer::with_transformer(trainer_config, transformer);

    let e_config = embedding::EmbeddingConfig {
        d_model: 64,
        d_proj: 128,
        temperature: 0.07,
        normalize: true,
    };
    let mut projection = embedding::ProjectionHead::new(e_config.clone());
    let augmenter = embedding::CsiAugmenter::new();

    eprintln!(
        "Starting contrastive pretraining for {} epochs...",
        args.pretrain_epochs
    );
    let start = std::time::Instant::now();
    for epoch in 0..args.pretrain_epochs {
        let loss = t.pretrain_epoch(&csi_windows, &augmenter, &mut projection, 0.07, epoch);
        if epoch % 10 == 0 || epoch == args.pretrain_epochs - 1 {
            eprintln!("  Epoch {epoch}: contrastive loss = {loss:.4}");
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    eprintln!("Pretraining complete in {elapsed:.1}s");

    if let Some(ref save_path) = args.save_rvf {
        eprintln!("Saving pretrained model to RVF: {}", save_path.display());
        t.sync_transformer_weights();
        let weights = t.params().to_vec();
        let mut proj_weights = Vec::new();
        projection.flatten_into(&mut proj_weights);

        let mut builder = RvfBuilder::new();
        builder.add_manifest(
            "wifi-densepose-pretrained",
            env!("CARGO_PKG_VERSION"),
            "WiFi DensePose contrastive pretrained model (ADR-024)",
        );
        builder.add_weights(&weights);
        builder.add_embedding(
            &serde_json::json!({
                "d_model": e_config.d_model,
                "d_proj": e_config.d_proj,
                "temperature": e_config.temperature,
                "normalize": e_config.normalize,
                "pretrain_epochs": args.pretrain_epochs,
            }),
            &proj_weights,
        );
        match builder.write_to_file(save_path) {
            Ok(()) => eprintln!(
                "RVF saved ({} transformer + {} projection params)",
                weights.len(),
                proj_weights.len()
            ),
            Err(e) => eprintln!("Failed to save RVF: {e}"),
        }
    }
}

/// Run embedding extraction on CSI data.
pub fn run_embed(args: &Args) {
    eprintln!("=== WiFi-DensePose Embedding Extraction (ADR-024) ===");

    let model_path = match &args.model {
        Some(p) => p.clone(),
        None => {
            eprintln!("Error: --embed requires --model <path> to a pretrained .rvf file");
            std::process::exit(1);
        }
    };

    let reader = match RvfReader::from_file(&model_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to load model: {e}");
            std::process::exit(1);
        }
    };

    let weights = reader.weights().unwrap_or_default();
    let (embed_config_json, proj_weights) = reader.embedding().unwrap_or_else(|| {
        eprintln!("Warning: no embedding segment in RVF, using defaults");
        (
            serde_json::json!({"d_model":64,"d_proj":128,"temperature":0.07,"normalize":true}),
            Vec::new(),
        )
    });

    let d_model = embed_config_json["d_model"].as_u64().unwrap_or(64) as usize;
    let d_proj = embed_config_json["d_proj"].as_u64().unwrap_or(128) as usize;

    let tf_config = graph_transformer::TransformerConfig {
        n_subcarriers: 56,
        n_keypoints: 17,
        d_model,
        n_heads: 4,
        n_gnn_layers: 2,
    };
    let e_config = embedding::EmbeddingConfig {
        d_model,
        d_proj,
        temperature: 0.07,
        normalize: true,
    };
    let mut extractor = embedding::EmbeddingExtractor::new(tf_config, e_config.clone());

    if !weights.is_empty() {
        if let Err(e) = extractor.transformer.unflatten_weights(&weights) {
            eprintln!("Warning: failed to load transformer weights: {e}");
        }
    }
    if !proj_weights.is_empty() {
        let (proj, _) = embedding::ProjectionHead::unflatten_from(&proj_weights, &e_config);
        extractor.projection = proj;
    }

    let csi_windows: Vec<Vec<Vec<f32>>> = (0..10)
        .map(|i| {
            (0..4)
                .map(|a| {
                    (0..56)
                        .map(|s| ((i * 7 + a * 13 + s) as f32 * 0.31).sin() * 0.5)
                        .collect()
                })
                .collect()
        })
        .collect();

    eprintln!(
        "Extracting embeddings from {} CSI windows...",
        csi_windows.len()
    );
    let embeddings = extractor.extract_batch(&csi_windows);
    for (i, emb) in embeddings.iter().enumerate() {
        let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        eprintln!("  Window {i}: {d_proj}-dim embedding, ||e|| = {norm:.4}");
    }
    eprintln!(
        "Extracted {} embeddings of dimension {d_proj}",
        embeddings.len()
    );
}

/// Build a fingerprint index from embeddings.
pub fn run_build_index(index_type_str: &str) {
    eprintln!("=== WiFi-DensePose Fingerprint Index Builder (ADR-024) ===");

    let index_type = match index_type_str {
        "env" | "environment" => embedding::IndexType::EnvironmentFingerprint,
        "activity" => embedding::IndexType::ActivityPattern,
        "temporal" => embedding::IndexType::TemporalBaseline,
        "person" => embedding::IndexType::PersonTrack,
        _ => {
            eprintln!(
                "Unknown index type '{index_type_str}'. Use: env, activity, temporal, person"
            );
            std::process::exit(1);
        }
    };

    let tf_config = graph_transformer::TransformerConfig::default();
    let e_config = embedding::EmbeddingConfig::default();
    let mut extractor = embedding::EmbeddingExtractor::new(tf_config, e_config);

    let csi_windows: Vec<Vec<Vec<f32>>> = (0..20)
        .map(|i| {
            (0..4)
                .map(|a| {
                    (0..56)
                        .map(|s| ((i * 7 + a * 13 + s) as f32 * 0.31).sin() * 0.5)
                        .collect()
                })
                .collect()
        })
        .collect();

    let mut index = embedding::FingerprintIndex::new(index_type);
    for (i, window) in csi_windows.iter().enumerate() {
        let emb = extractor.extract(window);
        index.insert(emb, format!("window_{i}"), i as u64 * 100);
    }

    eprintln!("Built {index_type:?} index with {} entries", index.len());

    let query_emb = extractor.extract(&csi_windows[0]);
    let results = index.search(&query_emb, 5);
    eprintln!("Top-5 nearest to window_0:");
    for r in &results {
        eprintln!(
            "  entry={}, distance={:.4}, metadata={}",
            r.entry, r.distance, r.metadata
        );
    }
}

