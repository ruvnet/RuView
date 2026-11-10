//! Handler for `--export-rvf` container format demo.

use std::path::Path;
use crate::rvf_pipeline::RvfModelBuilder;

/// Whether `--export-rvf` should emit the placeholder container-format demo.
pub fn export_emits_placeholder_demo(export_set: bool, train: bool, pretrain: bool) -> bool {
    export_set && !train && !pretrain
}

/// Export an RVF container package with placeholder demo weights.
pub fn run_export_rvf_placeholder(rvf_path: &Path) {
    eprintln!(
        "WARNING: --export-rvf writes a CONTAINER-FORMAT DEMO with placeholder \
         weights — it is NOT a trained model. Train one with \
         `--train --dataset <DIR>` (which exports a calibrated .rvf to the \
         models/ directory), or download a pretrained encoder. See issue #894."
    );
    eprintln!("Exporting RVF container package (placeholder weights)...");

    let mut builder = RvfModelBuilder::new("wifi-densepose", "1.0.0");
    builder.set_vital_config(0.1, 0.5, 0.8, 2.0);
    builder.set_model_profile(
        "56-subcarrier CSI amplitude/phase @ 10-100 Hz",
        "17 COCO keypoints + body part UV + vital signs",
        "ESP32-S3 or Windows WiFi RSSI, Rust 1.85+",
    );

    let placeholder_weights: Vec<f32> = (0..2856).map(|i| (i as f32 * 0.001).sin()).collect();
    builder.set_weights(&placeholder_weights);
    builder.set_training_proof(
        "wifi-densepose-rs-v1.0.0",
        serde_json::json!({
            "pipeline": "ADR-023 8-phase",
            "test_count": 229,
            "benchmark_fps": 9520,
            "framework": "wifi-densepose-rs",
        }),
    );

    let default_lora: Vec<f32> = vec![0.0; 64];
    builder.add_sona_profile("default", &default_lora, &default_lora);

    match builder.build() {
        Ok(rvf_bytes) => {
            if let Err(e) = std::fs::write(rvf_path, &rvf_bytes) {
                eprintln!("Error writing RVF: {e}");
                std::process::exit(1);
            }
            eprintln!("Wrote {} bytes to {}", rvf_bytes.len(), rvf_path.display());
            eprintln!("RVF container exported successfully.");
        }
        Err(e) => {
            eprintln!("Error building RVF: {e}");
            std::process::exit(1);
        }
    }
}

