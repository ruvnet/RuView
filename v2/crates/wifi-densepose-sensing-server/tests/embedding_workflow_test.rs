//! Regression coverage for dataset-backed offline modes (#2591).
//! All fixtures in this file are SYNTHETIC; no recognition accuracy is claimed.
use std::{path::Path, process::Command};
use wifi_densepose_sensing_server::{
    embedding::{EmbeddingConfig, EmbeddingExtractor},
    embedding_workflow::{self as workflow, FingerprintFile},
    graph_transformer::TransformerConfig,
    rvf_container::RvfBuilder,
};

fn npy(path: &Path, shape: &[usize], values: &[f32]) {
    let shape = shape
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let header = format!("{{'descr': '<f4', 'fortran_order': False, 'shape': ({shape},), }}\n");
    let mut bytes = b"\x93NUMPY\x01\x00".to_vec();
    bytes.extend_from_slice(&(header.len() as u16).to_le_bytes());
    bytes.extend_from_slice(header.as_bytes());
    for v in values {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

fn dataset(path: &Path, frames: usize, offset: f32) {
    std::fs::create_dir_all(path).unwrap();
    let data: Vec<_> = (0..frames * 56)
        .map(|i| (i as f32 * 0.017 + offset).sin())
        .collect();
    npy(&path.join("csi.npy"), &[frames, 56], &data);
    npy(
        &path.join("labels.npy"),
        &[frames, 17, 3],
        &vec![0.0; frames * 51],
    );
}

fn model(path: &Path, width: usize, legacy: bool, changed: bool) -> EmbeddingExtractor {
    let mut extractor = EmbeddingExtractor::new(
        TransformerConfig {
            n_subcarriers: width,
            d_model: 8,
            ..Default::default()
        },
        EmbeddingConfig {
            d_model: 8,
            d_proj: 12,
            ..Default::default()
        },
    );
    if changed {
        // Distinct projection biases prove that loading the RVF affects output.
        let mut weights = extractor.flatten_weights();
        let last = weights.len() - 1;
        weights[last] += 10.0;
        extractor.unflatten_weights(&weights).unwrap();
    }
    let mut config =
        serde_json::json!({"d_model":8,"d_proj":12,"temperature":0.07,"normalize":true});
    if !legacy {
        config["n_subcarriers"] = width.into();
        config["n_heads"] = 4.into();
        config["n_gnn_layers"] = 2.into();
    }
    let mut builder = RvfBuilder::new();
    builder.add_weights(&extractor.transformer.flatten_weights());
    let mut projection = vec![];
    extractor.projection.flatten_into(&mut projection);
    builder.add_embedding(&config, &projection);
    builder.write_to_file(path).unwrap();
    extractor
}

#[test]
fn loading_uses_dataset_windows_and_rejects_missing_short_or_malformed_data() {
    let dir = tempfile::tempdir().unwrap();
    assert!(workflow::load_windows(None, "mmfi").is_err());
    assert!(workflow::load_windows(Some(dir.path()), "typo").is_err());
    assert!(workflow::load_windows(Some(dir.path()), "mmfi").is_err());
    dataset(dir.path(), 5, 0.0);
    assert!(workflow::load_windows(Some(dir.path()), "mmfi")
        .unwrap_err()
        .contains("no complete"));
    dataset(dir.path(), 20, 0.0);
    let a = workflow::load_windows(Some(dir.path()), "mmfi").unwrap();
    assert_eq!(a.len(), 3);
    assert_eq!(a[0].len(), 10);
    assert_eq!(a[0][0].len(), 56);
    dataset(dir.path(), 25, 2.0);
    let b = workflow::load_windows(Some(dir.path()), "mmfi").unwrap();
    assert_eq!(b.len(), 4);
    assert_ne!(a[0], b[0]);
    std::fs::write(dir.path().join("csi.npy"), b"bad npy").unwrap();
    assert!(workflow::load_windows(Some(dir.path()), "mmfi").is_err());
}

#[test]
fn windows_reject_nonfinite_or_ragged_values() {
    for windows in [
        vec![],
        vec![vec![]],
        vec![vec![vec![f32::NAN]]],
        vec![vec![vec![1.0], vec![1.0, 2.0]]],
    ] {
        assert!(workflow::window_width(&windows).is_err());
    }
}

#[test]
fn rvf_weights_and_nondefault_width_are_honored_including_legacy_models() {
    let dir = tempfile::tempdir().unwrap();
    for legacy in [false, true] {
        let path = dir.path().join("model.rvf");
        let mut original = model(&path, 32, legacy, true);
        let mut loaded = workflow::load_extractor(Some(&path), 32).unwrap();
        let csi = vec![vec![0.3; 32]; 10];
        assert_eq!(original.extract(&csi), loaded.extract(&csi));
        assert!(workflow::load_extractor(Some(&path), 56).is_err());
    }
}

#[test]
fn invalid_or_pose_only_models_are_errors_not_untrained_fallbacks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.rvf");
    assert!(workflow::load_extractor(None, 56).is_err());
    assert!(workflow::load_extractor(Some(&path), 56).is_err());
    let mut builder = RvfBuilder::new();
    builder.add_weights(&[1.0]);
    builder.write_to_file(&path).unwrap();
    assert!(workflow::load_extractor(Some(&path), 56).is_err());
    for (d_model, weights) in [(8u64, vec![1.0]), (u64::MAX, vec![])] {
        let mut builder = RvfBuilder::new();
        builder.add_weights(&weights);
        builder.add_embedding(
            &serde_json::json!({
                "d_model":d_model,"d_proj":12,"temperature":0.07,"normalize":true
            }),
            &[],
        );
        builder.write_to_file(&path).unwrap();
        assert!(workflow::load_extractor(Some(&path), 56).is_err());
    }
}

#[test]
fn persisted_index_round_trips_and_remains_searchable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.json");
    let vectors = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
    let file = FingerprintFile::new("activity", vectors.clone()).unwrap();
    file.save(&path).unwrap();
    let read: FingerprintFile = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let index = read.to_index().unwrap();
    assert_eq!(index.len(), 2);
    assert_eq!(index.search(&vectors[1], 1)[0].metadata, "window_1");
    assert!(file.save(&path).is_err());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        serde_json::to_vec(&file).unwrap()
    );
    assert!(FingerprintFile::new("typo", vectors).is_err());
    assert!(FingerprintFile::new("env", vec![]).is_err());
}

#[test]
fn cli_modes_fail_without_valid_data_and_do_not_write_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    for mode in ["--pretrain", "--embed", "--build-index"] {
        for frames in [None, Some(5)] {
            let ds = dir.path().join("data");
            if let Some(n) = frames {
                dataset(&ds, n, 0.0);
            }
            let output = dir.path().join("output.rvf");
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_sensing-server"));
            cmd.arg(mode);
            if mode == "--build-index" {
                cmd.arg("env").arg("--index-output").arg(&output);
            }
            cmd.arg("--dataset")
                .arg(&ds)
                .arg("--model")
                .arg("missing.rvf")
                .arg("--save-rvf")
                .arg(&output);
            let result = cmd.output().unwrap();
            assert!(!result.status.success(), "{mode} unexpectedly succeeded");
            assert!(!output.exists());
            assert!(!String::from_utf8_lossy(&result.stderr).contains("Using synthetic"));
        }
    }
}

#[test]
fn cli_embeds_actual_data_and_index_honors_model_and_output_path() {
    let dir = tempfile::tempdir().unwrap();
    let ds = dir.path().join("data");
    dataset(&ds, 20, 0.0);
    let rvf = dir.path().join("model.rvf");
    let mut original = model(&rvf, 56, false, true);
    let windows = workflow::load_windows(Some(&ds), "mmfi").unwrap();
    let expected = original.extract_batch(&windows);
    let run = |mode: &str, output: Option<&Path>| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_sensing-server"));
        cmd.arg("--dataset")
            .arg(&ds)
            .arg("--model")
            .arg(&rvf)
            .arg(mode);
        if let Some(path) = output {
            cmd.arg("env").arg("--index-output").arg(path);
        }
        let result = cmd.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        result
    };
    let output = run("--embed", None);
    let actual: Vec<Vec<f32>> = String::from_utf8(output.stdout.clone())
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(actual, expected);
    let index_path = dir.path().join("fingerprints.json");
    run("--build-index", Some(&index_path));
    let saved: FingerprintFile =
        serde_json::from_slice(&std::fs::read(index_path).unwrap()).unwrap();
    assert_eq!(saved.embeddings, expected);
    model(&rvf, 56, false, false);
    assert_ne!(run("--embed", None).stdout, output.stdout);
    dataset(&ds, 25, 2.0);
    let changed = run("--embed", None);
    assert_ne!(changed.stdout, output.stdout);
    assert_eq!(
        String::from_utf8(changed.stdout).unwrap().lines().count(),
        4
    );
}
