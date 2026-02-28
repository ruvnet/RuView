#![allow(
    clippy::all,
    unused_imports,
    unused_variables,
    dead_code,
    unused_mut,
    unused_assignments,
    non_camel_case_types,
    clippy::approx_constant,
    unexpected_cfgs,
    unused_must_use,
    unused_parens
)]
//! RuvLLM Hub CLI - Manage models on HuggingFace Hub
//!
//! This CLI provides commands for downloading, uploading, and listing RuvLTRA models.
//!
//! ## Usage
//!
//! ```bash
//! # Pull a model from the registry
//! cargo run -p ruvllm --example hub_cli -- pull ruvltra-small
//!
//! # Push a custom model to HuggingFace Hub
//! HF_TOKEN=your_token cargo run -p ruvllm --example hub_cli -- push \
//!   --model ./my-model.gguf \
//!   --repo username/my-ruvltra \
//!   --description "My custom RuvLTRA model"
//!
//! # List available models in registry
//! cargo run -p ruvllm --example hub_cli -- list
//!
//! # Show detailed model information
//! cargo run -p ruvllm --example hub_cli -- info ruvltra-small
//! ```
//!
//! ## Environment Variables
//!
//! - `HF_TOKEN`: HuggingFace token (required for push operations)
//! - `RUVLLM_MODELS_DIR`: Default cache directory for downloaded models

use ruvllm::hub::{
    default_cache_dir, get_hf_token, DownloadConfig, ModelDownloader, ModelMetadata, ModelUploader,
    RuvLtraRegistry, UploadConfig,
};
use std::env;
use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_help();
        return;
    }

    let command = &args[1];
    match command.as_str() {
        "pull" => cmd_pull(&args[2..]),
        "push" => cmd_push(&args[2..]),
        "list" => cmd_list(&args[2..]),
        "info" => cmd_info(&args[2..]),
        "help" | "--help" | "-h" => print_help(),
        _ => {
            eprintln!("Unknown command: {}", command);
            eprintln!("Run 'hub_cli help' for usage information");
            process::exit(1);
        }
    }
}

/// Pull (download) a model
fn cmd_pull(args: &[String]) {
    if args.is_empty() {
        eprintln!("Error: Model ID required");
        eprintln!("Usage: hub_cli pull <model-id> [--output <dir>]");
        process::exit(1);
    }

    let model_id = &args[0];
    let mut output_dir: Option<PathBuf> = None;

    // Parse optional flags
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                i += 1;
                if i < args.len() {
                    output_dir = Some(PathBuf::from(&args[i]));
                }
            }
            _ => {}
        }
        i += 1;
    }

    let registry = RuvLtraRegistry::new();
    let model_info = match registry.get(model_id) {
        Some(info) => info,
        None => {
            eprintln!("Error: Model '{}' not found in registry", model_id);
            eprintln!("\nAvailable models:");
            for id in registry.model_ids() {
                eprintln!("  - {}", id);
            }
            process::exit(1);
        }
    };

    println!("📥 Pulling model: {}", model_info.name);
    println!("   Repository: {}", model_info.repo);
    println!(
        "   Size: {:.1} GB",
        model_info.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    println!("   Quantization: {:?}", model_info.quantization);
    println!();

    // Configure downloader
    let cache_dir = output_dir
        .or_else(|| env::var("RUVLLM_MODELS_DIR").ok().map(PathBuf::from))
        .unwrap_or_else(default_cache_dir);

    let config = DownloadConfig {
        cache_dir,
        hf_token: get_hf_token(),
        resume: true,
        show_progress: true,
        verify_checksum: model_info.checksum.is_some(),
        max_retries: 3,
    };

    let downloader = ModelDownloader::with_config(config);

    match downloader.download(model_info, None) {
        Ok(path) => {
            println!();
            println!("✅ Download complete!");
            println!("   Saved to: {}", path.display());
            println!();
            println!("   Minimum RAM: {:.1} GB", model_info.hardware.min_ram_gb);
            println!(
                "   Recommended RAM: {:.1} GB",
                model_info.hardware.recommended_ram_gb
            );

            if model_info.hardware.supports_ane {
                println!("   Apple Neural Engine: ✓");
            }
            if model_info.hardware.supports_metal {
                println!("   Metal GPU: ✓");
            }
            if model_info.hardware.supports_cuda {
                println!("   CUDA: ✓");
            }
        }
        Err(e) => {
            eprintln!("❌ Download failed: {}", e);
            process::exit(1);
        }
    }
}

/// Push (upload) a model
fn cmd_push(args: &[String]) {
    let mut model_path: Option<PathBuf> = None;
    let mut repo_id: Option<String> = None;
    let mut description: Option<String> = None;
    let mut private = false;
    let mut architecture = "llama".to_string();
    let mut params_b = 0.5;
    let mut context_length = 4096;
    let mut quantization: Option<String> = None;

    // Parse arguments
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--model" | "-m" => {
                i += 1;
                if i < args.len() {
                    model_path = Some(PathBuf::from(&args[i]));
                }
            }
            "--repo" | "-r" => {
                i += 1;
                if i < args.len() {
                    repo_id = Some(args[i].clone());
                }
            }
            "--description" | "-d" => {
                i += 1;
                if i < args.len() {
                    description = Some(args[i].clone());
                }
            }
            "--private" => {
                private = true;
            }
            "--architecture" | "-a" => {
                i += 1;
                if i < args.len() {
                    architecture = args[i].clone();
                }
            }
            "--params" | "-p" => {
                i += 1;
                if i < args.len() {
                    params_b = args[i].parse().unwrap_or(0.5);
                }
            }
            "--context" | "-c" => {
                i += 1;
                if i < args.len() {
                    context_length = args[i].parse().unwrap_or(4096);
                }
            }
            "--quant" | "-q" => {
                i += 1;
                if i < args.len() {
                    quantization = Some(args[i].clone());
                }
            }
            _ => {}
        }
        i += 1;
    }

    // Validate required arguments
    let model_path = match model_path {
        Some(p) => p,
        None => {
            eprintln!("Error: --model required");
            eprintln!("Usage: hub_cli push --model <path> --repo <username/repo-name>");
            process::exit(1);
        }
    };

    let repo_id = match repo_id {
        Some(r) => r,
        None => {
            eprintln!("Error: --repo required");
            eprintln!("Usage: hub_cli push --model <path> --repo <username/repo-name>");
            process::exit(1);
        }
    };

    // Get HF token
    let hf_token = match get_hf_token() {
        Some(t) => t,
        None => {
            eprintln!("Error: HF_TOKEN environment variable required for uploads");
            eprintln!("Set it with: export HF_TOKEN=your_token_here");
            process::exit(1);
        }
    };

    println!("📤 Pushing model to HuggingFace Hub");
    println!("   Local path: {}", model_path.display());
    println!("   Repository: {}", repo_id);
    println!(
        "   Visibility: {}",
        if private { "Private" } else { "Public" }
    );
    println!();

    // Create metadata
    let metadata = ModelMetadata {
        name: repo_id.split('/').last().unwrap_or("model").to_string(),
        description,
        architecture,
        params_b,
        context_length,
        quantization,
        license: Some("MIT".to_string()),
        datasets: vec![],
        tags: vec!["ruvltra".to_string()],
    };

    // Configure uploader
    let config = UploadConfig::new(hf_token)
        .private(private)
        .commit_message(format!("Upload {} model", metadata.name));

    let uploader = ModelUploader::with_config(config);

    match uploader.upload(&model_path, &repo_id, Some(metadata)) {
        Ok(url) => {
            println!("✅ Upload complete!");
            println!("   View at: {}", url);
        }
        Err(e) => {
            eprintln!("❌ Upload failed: {}", e);
            process::exit(1);
        }
    }
}

/// List available models
fn cmd_list(_args: &[String]) {
    let registry = RuvLtraRegistry::new();

    println!("📚 Available RuvLTRA Models\n");

    // Base models
    println!("Base Models:");
    println!(
        "{:<20} {:>8}  {:>6}  {:>8}  {:<40}",
        "ID", "SIZE", "PARAMS", "QUANT", "DESCRIPTION"
    );
    println!("{}", "=".repeat(90));

    for model in registry.list_base_models() {
        println!(
            "{:<20} {:>6}MB  {:>5.1}B  {:>8?}  {}",
            model.id,
            model.size_bytes / (1024 * 1024),
            model.params_b,
            model.quantization,
            truncate(&model.description, 38)
        );
    }

    // Adapters
    let adapters = registry
        .list_all()
        .into_iter()
        .filter(|m| m.is_adapter)
        .collect::<Vec<_>>();

    if !adapters.is_empty() {
        println!("\nLoRA Adapters:");
        println!("{:<20} {:>8}  {:<30}", "ID", "SIZE", "BASE MODEL");
        println!("{}", "=".repeat(60));

        for model in adapters {
            println!(
                "{:<20} {:>6}MB  {}",
                model.id,
                model.size_bytes / (1024 * 1024),
                model.base_model.as_ref().unwrap()
            );
        }
    }

    println!();
    println!("💡 Recommendations:");
    println!("   • Edge devices (< 2GB RAM): ruvltra-small");
    println!("   • General purpose (4-8GB RAM): ruvltra-medium");
    println!("   • Higher quality: Use Q8 quantization variants");
}

/// Show detailed model information
fn cmd_info(args: &[String]) {
    if args.is_empty() {
        eprintln!("Error: Model ID required");
        eprintln!("Usage: hub_cli info <model-id>");
        process::exit(1);
    }

    let model_id = &args[0];
    let registry = RuvLtraRegistry::new();

    let model = match registry.get(model_id) {
        Some(m) => m,
        None => {
            eprintln!("Error: Model '{}' not found", model_id);
            process::exit(1);
        }
    };

    println!("📋 Model Information: {}\n", model.name);
    println!("Repository:     {}", model.repo);
    println!("Hub URL:        {}", model.hub_url());
    println!("Download URL:   {}", model.download_url());
    println!();
    println!("Model Details:");
    println!("  Parameters:   {:.1}B", model.params_b);
    println!("  Architecture: {}", model.id);
    println!("  Quantization: {:?}", model.quantization);
    println!("  Context:      {} tokens", model.context_length);
    println!(
        "  File Size:    {:.2} GB",
        model.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    println!();
    println!("Hardware Requirements:");
    println!("  Min RAM:      {:.1} GB", model.hardware.min_ram_gb);
    println!(
        "  Rec RAM:      {:.1} GB",
        model.hardware.recommended_ram_gb
    );
    println!(
        "  ANE Support:  {}",
        if model.hardware.supports_ane {
            "✓"
        } else {
            "✗"
        }
    );
    println!(
        "  Metal GPU:    {}",
        if model.hardware.supports_metal {
            "✓"
        } else {
            "✗"
        }
    );
    println!(
        "  CUDA:         {}",
        if model.hardware.supports_cuda {
            "✓"
        } else {
            "✗"
        }
    );
    println!();
    println!("Features:");
    println!(
        "  SONA Weights: {}",
        if model.has_sona_weights { "✓" } else { "✗" }
    );
    println!(
        "  LoRA Adapter: {}",
        if model.is_adapter { "✓" } else { "✗" }
    );

    if let Some(base) = &model.base_model {
        println!("  Base Model:   {}", base);
    }

    println!();
    println!("Description:");
    println!("  {}", model.description);

    println!();
    println!("Download with:");
    println!(
        "  cargo run -p ruvllm --example hub_cli -- pull {}",
        model_id
    );

    // Estimate download time
    let time_10mbps = model.estimate_download_time(10.0);
    let time_100mbps = model.estimate_download_time(100.0);
    println!();
    println!("Estimated download time:");
    println!("  @ 10 Mbps:  {:.0} seconds", time_10mbps);
    println!("  @ 100 Mbps: {:.0} seconds", time_100mbps);
}

fn print_help() {
    println!("RuvLLM Hub CLI - Manage models on HuggingFace Hub\n");
    println!("USAGE:");
    println!("    hub_cli <COMMAND> [OPTIONS]\n");
    println!("COMMANDS:");
    println!("    pull      Download a model from the registry");
    println!("    push      Upload a model to HuggingFace Hub");
    println!("    list      List available models in the registry");
    println!("    info      Show detailed information about a model");
    println!("    help      Print this help message\n");
    println!("EXAMPLES:");
    println!("    # Download a model");
    println!("    hub_cli pull ruvltra-small\n");
    println!("    # Upload a custom model");
    println!("    HF_TOKEN=xxx hub_cli push --model ./model.gguf --repo user/model\n");
    println!("    # List all models");
    println!("    hub_cli list\n");
    println!("    # Show model details");
    println!("    hub_cli info ruvltra-medium\n");
    println!("For more details on a specific command:");
    println!("    hub_cli <command> --help");
}

/// Truncate string to max length
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
