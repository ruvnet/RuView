//! Handler for `--convert-model` CLI flag and model format adaptation.

use std::path::Path;
use tracing::info;
use crate::model_format::{self, convert_to_rvf, detect_format, ModelFormat};
use crate::rvf_pipeline::ProgressiveLoader;

/// Convert a model file to the RVF binary container format.
pub fn run_convert_model(in_path: &Path, out_path: &Path) -> i32 {
    let data = match std::fs::read(in_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("convert-model: failed to read {}: {e}", in_path.display());
            return 1;
        }
    };
    let name = in_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let model_id = in_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("converted-model");

    let detected = model_format::detect_format(&data, &name);
    eprintln!(
        "convert-model: detected {} ({} bytes)",
        detected.label(),
        data.len()
    );

    match model_format::convert_to_rvf(&data, &name, model_id) {
        Ok(rvf_bytes) => {
            if let Err(e) = ProgressiveLoader::new(&rvf_bytes) {
                eprintln!("convert-model: produced RVF did NOT load (bug): {e}");
                return 1;
            }
            if let Err(e) = std::fs::write(out_path, &rvf_bytes) {
                eprintln!("convert-model: failed to write {}: {e}", out_path.display());
                return 1;
            }
            eprintln!(
                "convert-model: wrote {} ({} bytes). Load it with `--model {}`.",
                out_path.display(),
                rvf_bytes.len(),
                out_path.display()
            );
            0
        }
        Err(e) => {
            eprintln!("convert-model: {e}");
            1
        }
    }
}

/// Helper to classify model loading errors into user-actionable descriptions (#894).
pub fn diagnose_model_load_error(path: &Path, data: &[u8], err: &str) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let looks_safetensors = ext == "safetensors" || (data.len() > 9 && data[8] == b'{');
    let looks_jsonl =
        ext == "jsonl" || name.ends_with(".rvf.jsonl") || data.first() == Some(&b'{');
    let looks_quant_bin = ext == "bin" || name.contains("-q");

    let kind = if looks_safetensors {
        "a safetensors weight file"
    } else if looks_jsonl {
        "a JSONL manifest, not the binary container"
    } else if looks_quant_bin {
        "a quantized weight blob (e.g. HuggingFace model-q4.bin)"
    } else {
        "not an RVF binary container"
    };

    format!(
        "model `{}` could not be loaded: it is {kind}. The --model flag expects an \
         RVF binary container (`RVFS` magic 0x52564653) produced by the \
         wifi-densepose-train pipeline. The HuggingFace ruvnet/wifi-densepose-pretrained \
         files are a different format and encoder architecture, so they do not load \
         here directly (issue #894). Continuing with signal heuristics. (loader: {err})",
        path.display()
    )
}

/// Load a model for `--model`, auto-detecting and converting published
/// HuggingFace formats (safetensors / jsonl) when native RVF rejects them.
pub fn load_or_convert_model(
    path: &Path,
    data: &[u8],
) -> Result<ProgressiveLoader, String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let model_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("converted-model");

    match detect_format(data, &name) {
        ModelFormat::Rvf => ProgressiveLoader::new(data).map_err(|e| {
            model_format::classify_load_failure(data, &name, &e).to_string()
        }),
        ModelFormat::Safetensors | ModelFormat::JsonlManifest => {
            match convert_to_rvf(data, &name, model_id) {
                Ok(rvf_bytes) => {
                    info!(
                        "Model `{}` is {} — converting to RVF in-memory and loading (issue #894)",
                        path.display(),
                        detect_format(data, &name).label()
                    );
                    ProgressiveLoader::new(&rvf_bytes).map_err(|e| {
                        format!(
                            "converted {} to RVF but the container failed to load: {e}",
                            detect_format(data, &name).label()
                        )
                    })
                }
                Err(conv_err) => Err(conv_err.to_string()),
            }
        }
        _ => Err(model_format::classify_load_failure(
            data,
            &name,
            "RVF container parse failed",
        )
        .to_string()),
    }
}
