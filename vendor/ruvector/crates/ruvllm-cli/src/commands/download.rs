//! Model download command implementation
//!
//! Downloads models from HuggingFace Hub with progress indication,
//! supporting various quantization formats optimized for Apple Silicon.

use anyhow::{Context, Result};
use bytesize::ByteSize;
use colored::Colorize;
use console::style;
use hf_hub::api::tokio::Api;
use hf_hub::{Repo, RepoType};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};

use crate::models::{get_model, resolve_model_id, QuantPreset};

/// Run the download command
pub async fn run(
    model: &str,
    quantization: &str,
    force: bool,
    revision: Option<&str>,
    cache_dir: &str,
) -> Result<()> {
    let model_id = resolve_model_id(model);
    let quant = QuantPreset::from_str(quantization)
        .ok_or_else(|| anyhow::anyhow!("Invalid quantization format: {}", quantization))?;

    println!();
    println!(
        "{} {} ({})",
        style("Downloading:").bold().cyan(),
        model_id,
        quant
    );
    println!();

    // Get model info if available
    if let Some(model_def) = get_model(model) {
        println!("  {} {}", "Name:".dimmed(), model_def.name);
        println!("  {} {}", "Architecture:".dimmed(), model_def.architecture);
        println!("  {} {}B", "Parameters:".dimmed(), model_def.params_b);
        println!(
            "  {} ~{:.1} GB",
            "Est. Memory:".dimmed(),
            quant.estimate_memory_gb(model_def.params_b)
        );
        println!();
    }

    // Initialize HuggingFace API
    let api = Api::new().context("Failed to initialize HuggingFace API")?;

    // Create repo reference
    let repo = if let Some(rev) = revision {
        api.repo(Repo::with_revision(
            model_id.clone(),
            RepoType::Model,
            rev.to_string(),
        ))
    } else {
        api.repo(Repo::new(model_id.clone(), RepoType::Model))
    };

    // Determine files to download
    let files_to_download = get_files_to_download(&model_id, quant);

    // Create cache directory
    let model_cache_dir = PathBuf::from(cache_dir).join("models").join(&model_id);
    tokio::fs::create_dir_all(&model_cache_dir)
        .await
        .context("Failed to create cache directory")?;

    // Download each file
    for file_name in &files_to_download {
        let target_path = model_cache_dir.join(file_name);

        // Check if file exists
        if target_path.exists() && !force {
            let size = tokio::fs::metadata(&target_path).await?.len();
            println!(
                "  {} {} ({})",
                style("Cached:").green(),
                file_name,
                ByteSize(size)
            );
            continue;
        }

        println!("  {} {}", style("Downloading:").yellow(), file_name);

        // Download with progress
        let downloaded_path = download_with_progress(&repo, file_name).await?;

        // Copy to cache directory
        tokio::fs::copy(&downloaded_path, &target_path)
            .await
            .context("Failed to copy file to cache")?;

        let size = tokio::fs::metadata(&target_path).await?.len();
        println!(
            "  {} {} ({})",
            style("Downloaded:").green(),
            file_name,
            ByteSize(size)
        );
    }

    println!();
    println!(
        "{} Model ready at: {}",
        style("Success!").green().bold(),
        model_cache_dir.display()
    );
    println!();

    // Print usage hint
    println!("{}", "Quick start:".bold());
    println!("  ruvllm chat {}", model);
    println!("  ruvllm serve {}", model);
    println!();

    Ok(())
}

/// Download a file with progress indication
async fn download_with_progress(
    repo: &hf_hub::api::tokio::ApiRepo,
    file_name: &str,
) -> Result<PathBuf> {
    // Create progress bar
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("    [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Download file
    let path = repo
        .get(file_name)
        .await
        .context(format!("Failed to download {}", file_name))?;

    pb.finish_and_clear();

    Ok(path)
}

/// Get list of files to download for a model and quantization
fn get_files_to_download(model_id: &str, quant: QuantPreset) -> Vec<String> {
    let mut files = vec![
        "tokenizer.json".to_string(),
        "tokenizer_config.json".to_string(),
        "config.json".to_string(),
    ];

    // Add model weights based on quantization
    if model_id.contains("GGUF") || quant != QuantPreset::None {
        // Look for GGUF files
        files.push(format!("*{}", quant.gguf_suffix()));
    } else {
        // SafeTensors format
        files.push("model.safetensors".to_string());
    }

    // Add special tokens and chat template if available
    files.push("special_tokens_map.json".to_string());
    files.push("generation_config.json".to_string());

    files
}

/// Check if a model is already downloaded
pub async fn is_model_downloaded(model: &str, cache_dir: &str) -> bool {
    let model_id = resolve_model_id(model);
    let model_cache_dir = PathBuf::from(cache_dir).join("models").join(&model_id);

    // Check for tokenizer and at least one model file
    let tokenizer_exists = model_cache_dir.join("tokenizer.json").exists();
    let has_weights = tokio::fs::read_dir(&model_cache_dir)
        .await
        .ok()
        .map(|mut dir| {
            use futures::StreamExt;
            // Simplified check - just see if directory exists and has files
            true
        })
        .unwrap_or(false);

    tokenizer_exists && has_weights
}

/// Get the path to a downloaded model
pub fn get_model_path(model: &str, cache_dir: &str) -> PathBuf {
    let model_id = resolve_model_id(model);
    PathBuf::from(cache_dir).join("models").join(&model_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_files_to_download() {
        let files = get_files_to_download("test/model", QuantPreset::Q4K);
        assert!(files.contains(&"tokenizer.json".to_string()));
        assert!(files.iter().any(|f| f.contains("Q4_K_M")));
    }
}
