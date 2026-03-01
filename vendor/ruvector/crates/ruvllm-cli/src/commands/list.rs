//! List command implementation
//!
//! Lists available and downloaded models with their details.

use anyhow::Result;
use bytesize::ByteSize;
use colored::Colorize;
use console::style;
use prettytable::{row, Table};
use std::path::PathBuf;

use crate::models::{get_recommended_models, ModelDefinition};

/// Run the list command
pub async fn run(downloaded_only: bool, long_format: bool, cache_dir: &str) -> Result<()> {
    println!();

    if downloaded_only {
        list_downloaded_models(cache_dir, long_format).await?;
    } else {
        list_all_models(cache_dir, long_format).await?;
    }

    Ok(())
}

/// List all recommended models
async fn list_all_models(cache_dir: &str, long_format: bool) -> Result<()> {
    let models = get_recommended_models();

    println!(
        "{}",
        style("Recommended Models for Mac M4 Pro").bold().cyan()
    );
    println!();

    if long_format {
        print_models_long(&models, cache_dir).await;
    } else {
        print_models_short(&models, cache_dir).await;
    }

    println!();
    println!("{}", "Usage:".bold());
    println!("  ruvllm download <alias>   # Download a model");
    println!("  ruvllm chat <alias>       # Start chatting");
    println!("  ruvllm serve <alias>      # Start server");
    println!();

    Ok(())
}

/// List only downloaded models
async fn list_downloaded_models(cache_dir: &str, long_format: bool) -> Result<()> {
    let models_dir = PathBuf::from(cache_dir).join("models");

    if !models_dir.exists() {
        println!("{}", "No models downloaded yet.".dimmed());
        println!();
        println!("Run 'ruvllm download <model>' to download a model.");
        return Ok(());
    }

    let mut downloaded = Vec::new();
    let mut entries = tokio::fs::read_dir(&models_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            let model_id = entry.file_name().to_string_lossy().to_string();
            let model_path = entry.path();

            // Calculate total size
            let size = calculate_dir_size(&model_path).await.unwrap_or(0);

            downloaded.push((model_id, model_path, size));
        }
    }

    if downloaded.is_empty() {
        println!("{}", "No models downloaded yet.".dimmed());
        println!();
        println!("Run 'ruvllm download <model>' to download a model.");
        return Ok(());
    }

    println!("{}", style("Downloaded Models").bold().green());
    println!();

    let mut table = Table::new();
    table.add_row(row!["Model", "Size", "Path"]);

    for (model_id, path, size) in &downloaded {
        table.add_row(row![
            model_id.green(),
            ByteSize(*size).to_string(),
            path.display()
        ]);
    }

    table.printstd();

    // Calculate total
    let total_size: u64 = downloaded.iter().map(|(_, _, s)| s).sum();
    println!();
    println!(
        "Total: {} models, {}",
        downloaded.len(),
        ByteSize(total_size)
    );

    Ok(())
}

/// Print models in short format
async fn print_models_short(models: &[ModelDefinition], cache_dir: &str) {
    let mut table = Table::new();
    table.add_row(row!["Alias", "Name", "Params", "Memory", "Status"]);

    for model in models {
        let is_downloaded = check_model_downloaded(&model.hf_id, cache_dir).await;
        let status = if is_downloaded {
            "Downloaded".green().to_string()
        } else {
            "Not downloaded".dimmed().to_string()
        };

        table.add_row(row![
            model.alias.cyan(),
            model.name,
            format!("{}B", model.params_b),
            format!("~{:.1}GB", model.memory_gb),
            status
        ]);
    }

    table.printstd();
}

/// Print models in long format
async fn print_models_long(models: &[ModelDefinition], cache_dir: &str) {
    for model in models {
        let is_downloaded = check_model_downloaded(&model.hf_id, cache_dir).await;

        println!("{}", style(&model.alias).bold().cyan());
        println!("  {} {}", "Name:".dimmed(), model.name);
        println!("  {} {}", "HF ID:".dimmed(), model.hf_id);
        println!("  {} {}", "Architecture:".dimmed(), model.architecture);
        println!("  {} {}B", "Parameters:".dimmed(), model.params_b);
        println!("  {} ~{:.1} GB", "Memory:".dimmed(), model.memory_gb);
        println!("  {} {}", "Context:".dimmed(), model.context_length);
        println!("  {} {}", "Use Case:".dimmed(), model.use_case);
        println!("  {} {}", "Quant:".dimmed(), model.recommended_quant);
        println!("  {} {}", "Notes:".dimmed(), model.notes);
        println!(
            "  {} {}",
            "Status:".dimmed(),
            if is_downloaded {
                "Downloaded".green()
            } else {
                "Not downloaded".red()
            }
        );
        println!();
    }
}

/// Check if a model is downloaded
async fn check_model_downloaded(model_id: &str, cache_dir: &str) -> bool {
    let model_path = PathBuf::from(cache_dir).join("models").join(model_id);
    model_path.exists() && model_path.join("tokenizer.json").exists()
}

/// Calculate directory size recursively
async fn calculate_dir_size(path: &PathBuf) -> Result<u64> {
    let mut total = 0u64;
    let mut entries = tokio::fs::read_dir(path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if metadata.is_file() {
            total += metadata.len();
        } else if metadata.is_dir() {
            total += Box::pin(calculate_dir_size(&entry.path())).await?;
        }
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_models() {
        let models = get_recommended_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.alias == "qwen"));
    }
}
