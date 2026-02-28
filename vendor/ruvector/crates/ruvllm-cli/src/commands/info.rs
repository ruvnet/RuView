//! Model info command implementation
//!
//! Shows detailed information about a model, including its architecture,
//! memory requirements, and recommended settings.

use anyhow::{Context, Result};
use bytesize::ByteSize;
use colored::Colorize;
use console::style;
use std::path::PathBuf;

use crate::models::{get_model, resolve_model_id, QuantPreset};

/// Run the info command
pub async fn run(model: &str, cache_dir: &str) -> Result<()> {
    let model_id = resolve_model_id(model);

    println!();
    println!("{} {}", style("Model Information:").bold().cyan(), model_id);
    println!();

    // Check if model is from our recommended list
    if let Some(model_def) = get_model(model) {
        print_model_definition(&model_def);
    } else {
        println!(
            "{}",
            "Model not in recommended list. Fetching from HuggingFace...".dimmed()
        );
        println!();
        fetch_model_info(&model_id).await?;
    }

    // Check if downloaded
    let model_path = PathBuf::from(cache_dir).join("models").join(&model_id);
    if model_path.exists() {
        println!();
        println!("{}", style("Local Cache:").bold().green());
        print_local_info(&model_path).await?;
    } else {
        println!();
        println!("{} {}", style("Status:").bold(), "Not downloaded".red());
        println!();
        println!("Run 'ruvllm download {}' to download.", model);
    }

    // Print memory estimates
    println!();
    println!("{}", style("Memory Estimates by Quantization:").bold());
    print_memory_estimates(model);

    // Print recommended settings
    println!();
    println!("{}", style("Recommended Settings:").bold());
    print_recommended_settings(model);

    println!();

    Ok(())
}

/// Print model definition from our database
fn print_model_definition(model: &crate::models::ModelDefinition) {
    println!("  {} {}", "Alias:".dimmed(), model.alias.cyan());
    println!("  {} {}", "Name:".dimmed(), model.name);
    println!("  {} {}", "HuggingFace ID:".dimmed(), model.hf_id);
    println!("  {} {}", "Architecture:".dimmed(), model.architecture);
    println!("  {} {}B parameters", "Size:".dimmed(), model.params_b);
    println!(
        "  {} {} tokens",
        "Context Length:".dimmed(),
        model.context_length
    );
    println!("  {} {}", "Primary Use:".dimmed(), model.use_case);
    println!(
        "  {} {}",
        "Recommended Quant:".dimmed(),
        model.recommended_quant
    );
    println!(
        "  {} ~{:.1} GB (with {})",
        "Memory:".dimmed(),
        model.memory_gb,
        model.recommended_quant
    );
    println!("  {} {}", "Notes:".dimmed(), model.notes);
}

/// Fetch model info from HuggingFace API
async fn fetch_model_info(model_id: &str) -> Result<()> {
    use hf_hub::api::tokio::Api;
    use hf_hub::{Repo, RepoType};

    let api = Api::new().context("Failed to initialize HuggingFace API")?;
    let repo = api.repo(Repo::new(model_id.to_string(), RepoType::Model));

    // Try to get config.json
    match repo.get("config.json").await {
        Ok(config_path) => {
            let config_str = tokio::fs::read_to_string(&config_path).await?;
            let config: serde_json::Value = serde_json::from_str(&config_str)?;

            if let Some(arch) = config.get("architectures").and_then(|a| a.get(0)) {
                println!("  {} {}", "Architecture:".dimmed(), arch);
            }
            if let Some(hidden) = config.get("hidden_size") {
                println!("  {} {}", "Hidden Size:".dimmed(), hidden);
            }
            if let Some(layers) = config.get("num_hidden_layers") {
                println!("  {} {}", "Layers:".dimmed(), layers);
            }
            if let Some(heads) = config.get("num_attention_heads") {
                println!("  {} {}", "Attention Heads:".dimmed(), heads);
            }
            if let Some(vocab) = config.get("vocab_size") {
                println!("  {} {}", "Vocab Size:".dimmed(), vocab);
            }
            if let Some(ctx) = config.get("max_position_embeddings") {
                println!("  {} {}", "Max Context:".dimmed(), ctx);
            }
        }
        Err(_) => {
            println!(
                "  {} Could not fetch model configuration",
                "Warning:".yellow()
            );
        }
    }

    Ok(())
}

/// Print local cache information
async fn print_local_info(model_path: &PathBuf) -> Result<()> {
    println!("  {} {}", "Path:".dimmed(), model_path.display());

    // Calculate total size
    let mut total_size = 0u64;
    let mut file_count = 0usize;
    let mut entries = tokio::fs::read_dir(model_path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if metadata.is_file() {
            total_size += metadata.len();
            file_count += 1;
        }
    }

    println!("  {} {}", "Size:".dimmed(), ByteSize(total_size));
    println!("  {} {}", "Files:".dimmed(), file_count);

    // Check for specific files
    let has_tokenizer = model_path.join("tokenizer.json").exists();
    let has_config = model_path.join("config.json").exists();

    // Find model weights
    let mut weights_file = None;
    let mut entries = tokio::fs::read_dir(model_path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".gguf") || name.ends_with(".safetensors") || name.ends_with(".bin") {
            weights_file = Some(name);
            break;
        }
    }

    println!(
        "  {} {}",
        "Tokenizer:".dimmed(),
        if has_tokenizer {
            "Yes".green()
        } else {
            "No".red()
        }
    );
    println!(
        "  {} {}",
        "Config:".dimmed(),
        if has_config {
            "Yes".green()
        } else {
            "No".red()
        }
    );
    println!(
        "  {} {}",
        "Weights:".dimmed(),
        weights_file.unwrap_or_else(|| "Not found".red().to_string())
    );

    Ok(())
}

/// Print memory estimates for different quantization levels
fn print_memory_estimates(model: &str) {
    if let Some(model_def) = get_model(model) {
        let params = model_def.params_b;

        println!(
            "  {} {:>8}",
            "Q4_K_M (4-bit):".dimmed(),
            format!("{:.1} GB", QuantPreset::Q4K.estimate_memory_gb(params))
        );
        println!(
            "  {} {:>8}",
            "Q8_0 (8-bit):".dimmed(),
            format!("{:.1} GB", QuantPreset::Q8.estimate_memory_gb(params))
        );
        println!(
            "  {} {:>8}",
            "F16 (16-bit):".dimmed(),
            format!("{:.1} GB", QuantPreset::F16.estimate_memory_gb(params))
        );
        println!(
            "  {} {:>8}",
            "F32 (32-bit):".dimmed(),
            format!("{:.1} GB", QuantPreset::None.estimate_memory_gb(params))
        );
    } else {
        println!(
            "  {} Memory estimates not available for custom models",
            "Note:".dimmed()
        );
    }
}

/// Print recommended settings for the model
fn print_recommended_settings(model: &str) {
    if let Some(model_def) = get_model(model) {
        // Determine best settings based on model size and type
        let (temp, top_p, context) = match model_def.alias.as_str() {
            "qwen" | "qwen-large" => (0.7, 0.9, 8192),
            "mistral" => (0.7, 0.95, 4096),
            "phi" => (0.6, 0.9, 2048),
            "llama" => (0.8, 0.95, 4096),
            "qwen-coder" => (0.2, 0.95, 8192), // Lower temp for code
            _ => (0.7, 0.9, 4096),
        };

        println!("  {} {}", "Temperature:".dimmed(), temp);
        println!("  {} {}", "Top-P:".dimmed(), top_p);
        println!("  {} {} tokens", "Context:".dimmed(), context);
        println!(
            "  {} {}",
            "Quantization:".dimmed(),
            model_def.recommended_quant
        );

        // Special notes based on model
        match model_def.alias.as_str() {
            "qwen-coder" => {
                println!(
                    "  {} Use lower temperature (0.1-0.3) for code completion",
                    "Tip:".cyan()
                );
            }
            "llama" => {
                println!(
                    "  {} Excellent for function calling and structured output",
                    "Tip:".cyan()
                );
            }
            "phi" => {
                println!(
                    "  {} Great for quick testing and resource-constrained environments",
                    "Tip:".cyan()
                );
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_estimates() {
        let model = get_model("qwen").unwrap();
        let mem = QuantPreset::Q4K.estimate_memory_gb(model.params_b);
        assert!(mem > 5.0 && mem < 15.0);
    }
}
