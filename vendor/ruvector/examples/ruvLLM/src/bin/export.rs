//! RuvLLM HuggingFace Export Binary
//!
//! Export learned SONA patterns, LoRA weights, and preference pairs to HuggingFace.

use anyhow::Result;
use ruvector_sona::{HuggingFaceExporter, PretrainPipeline, SonaConfig, SonaEngine};
use std::path::PathBuf;
use tracing::{error, info, warn};

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("ruvllm=info".parse().unwrap()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "safetensors" => export_safetensors(&args[2..])?,
        "patterns" => export_patterns(&args[2..])?,
        "preferences" => export_preferences(&args[2..])?,
        "all" => export_all(&args[2..])?,
        "push" => push_to_hub(&args[2..])?,
        "pretrain" => generate_pretrain_script(&args[2..])?,
        "help" | "--help" | "-h" => print_usage(),
        cmd => {
            error!("Unknown command: {}", cmd);
            print_usage();
        }
    }

    Ok(())
}

fn print_usage() {
    println!(
        r#"
RuvLLM HuggingFace Export Tool

USAGE:
    ruvllm-export <COMMAND> [OPTIONS]

COMMANDS:
    safetensors <output_dir>    Export LoRA weights in PEFT-compatible SafeTensors format
    patterns <output_dir>       Export learned patterns as JSONL dataset
    preferences <output_dir>    Export DPO/RLHF preference pairs
    all <output_dir>            Export all artifacts (weights, patterns, preferences)
    push <repo_id>              Push exported artifacts to HuggingFace Hub
    pretrain <output_dir>       Generate pretraining pipeline configuration
    help                        Show this help message

EXAMPLES:
    # Export LoRA weights
    ruvllm-export safetensors ./exports/lora

    # Export all artifacts
    ruvllm-export all ./exports

    # Push to HuggingFace Hub
    ruvllm-export push username/my-sona-model

    # Generate pretraining script
    ruvllm-export pretrain ./exports

ENVIRONMENT:
    HF_TOKEN                    HuggingFace API token (required for push)
    RUVLLM_DIM                  Hidden dimension (default: 256)
    RUVLLM_PATTERNS             Pattern clusters (default: 100)
"#
    );
}

fn create_demo_engine() -> SonaEngine {
    let dim = std::env::var("RUVLLM_DIM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(256);

    let clusters = std::env::var("RUVLLM_PATTERNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    info!(
        "Creating SONA engine with dim={}, clusters={}",
        dim, clusters
    );

    let config = SonaConfig {
        hidden_dim: dim,
        embedding_dim: dim,
        pattern_clusters: clusters,
        ..Default::default()
    };

    let engine = SonaEngine::with_config(config);

    // Generate some demo trajectories for demonstration
    info!("Generating demo trajectories...");
    for i in 0..200 {
        let quality = 0.3 + (i as f32 / 200.0) * 0.6; // Quality from 0.3 to 0.9
        let mut builder = engine.begin_trajectory(vec![0.1 + (i as f32 * 0.001); dim]);
        builder.add_step(vec![0.5; dim], vec![], quality);
        builder.add_step(vec![0.6; dim], vec![], quality + 0.05);
        engine.end_trajectory(builder, quality);
    }

    // Force learning to extract patterns
    info!("Running pattern extraction...");
    let result = engine.force_learn();
    info!("{}", result);

    engine
}

fn export_safetensors(args: &[String]) -> Result<()> {
    let output_dir = args
        .get(0)
        .map(|s| PathBuf::from(s))
        .unwrap_or_else(|| PathBuf::from("./exports/safetensors"));

    info!("Exporting SafeTensors to {:?}", output_dir);
    std::fs::create_dir_all(&output_dir)?;

    let engine = create_demo_engine();
    let exporter = HuggingFaceExporter::new(&engine);

    match exporter.export_lora_safetensors(&output_dir) {
        Ok(result) => {
            info!(
                "Exported SafeTensors: {} items, {} bytes",
                result.items_exported, result.size_bytes
            );
            println!("  -> {}", result.output_path);
        }
        Err(e) => error!("Failed to export SafeTensors: {}", e),
    }

    Ok(())
}

fn export_patterns(args: &[String]) -> Result<()> {
    let output_dir = args
        .get(0)
        .map(|s| PathBuf::from(s))
        .unwrap_or_else(|| PathBuf::from("./exports/patterns"));

    info!("Exporting patterns to {:?}", output_dir);
    std::fs::create_dir_all(&output_dir)?;

    let engine = create_demo_engine();
    let exporter = HuggingFaceExporter::new(&engine);

    match exporter.export_patterns_jsonl(output_dir.join("patterns.jsonl")) {
        Ok(result) => {
            info!(
                "Exported patterns: {} items, {} bytes",
                result.items_exported, result.size_bytes
            );
            println!("  -> {}", result.output_path);
        }
        Err(e) => error!("Failed to export patterns: {}", e),
    }

    Ok(())
}

fn export_preferences(args: &[String]) -> Result<()> {
    let output_dir = args
        .get(0)
        .map(|s| PathBuf::from(s))
        .unwrap_or_else(|| PathBuf::from("./exports/preferences"));

    info!("Exporting preference pairs to {:?}", output_dir);
    std::fs::create_dir_all(&output_dir)?;

    let engine = create_demo_engine();
    let exporter = HuggingFaceExporter::new(&engine);

    match exporter.export_preference_pairs(output_dir.join("preferences.jsonl")) {
        Ok(result) => {
            info!(
                "Exported preferences: {} items, {} bytes",
                result.items_exported, result.size_bytes
            );
            println!("  -> {}", result.output_path);
        }
        Err(e) => error!("Failed to export preferences: {}", e),
    }

    Ok(())
}

fn export_all(args: &[String]) -> Result<()> {
    let output_dir = args
        .get(0)
        .map(|s| PathBuf::from(s))
        .unwrap_or_else(|| PathBuf::from("./exports"));

    info!("Exporting all artifacts to {:?}", output_dir);
    std::fs::create_dir_all(&output_dir)?;

    let engine = create_demo_engine();
    let exporter = HuggingFaceExporter::new(&engine);

    match exporter.export_all(&output_dir) {
        Ok(results) => {
            let total_items: usize = results.iter().map(|r| r.items_exported).sum();
            let total_bytes: u64 = results.iter().map(|r| r.size_bytes).sum();
            info!(
                "Exported all: {} items, {} bytes total",
                total_items, total_bytes
            );
            for result in &results {
                println!("  -> {}", result.output_path);
            }
        }
        Err(e) => error!("Failed to export: {}", e),
    }

    Ok(())
}

fn push_to_hub(args: &[String]) -> Result<()> {
    if args.is_empty() {
        error!("Usage: ruvllm-export push <repo_id>");
        return Ok(());
    }

    let repo_id = &args[0];

    let token = std::env::var("HF_TOKEN")
        .or_else(|_| std::env::var("HUGGINGFACE_API_KEY"))
        .ok();
    if token.is_none() {
        warn!("HF_TOKEN or HUGGINGFACE_API_KEY not set - will attempt without auth");
    }

    info!("Pushing to HuggingFace Hub: {}", repo_id);

    let engine = create_demo_engine();
    let exporter = HuggingFaceExporter::new(&engine);

    match exporter.push_to_hub(repo_id, token.as_deref()) {
        Ok(_) => info!("Successfully pushed to https://huggingface.co/{}", repo_id),
        Err(e) => error!("Failed to push: {}", e),
    }

    Ok(())
}

fn generate_pretrain_script(args: &[String]) -> Result<()> {
    let output_dir = args
        .get(0)
        .map(|s| PathBuf::from(s))
        .unwrap_or_else(|| PathBuf::from("./exports"));

    info!("Generating pretraining configuration to {:?}", output_dir);
    std::fs::create_dir_all(&output_dir)?;

    let engine = create_demo_engine();
    let pipeline = PretrainPipeline::new(&engine);

    // Export complete pretraining package
    match pipeline.export_package(&output_dir) {
        Ok(package) => {
            info!("Generated pretraining package:");
            println!("  -> {}", package.script_path);
            println!("  -> {}", package.config_path);
            println!("  -> {} (output dir)", package.output_dir);

            println!("\nTo start pretraining:");
            println!("  cd {:?}", output_dir);
            println!("  pip install -r requirements.txt");
            println!("  python train.py");
        }
        Err(e) => error!("Failed to generate pretrain package: {}", e),
    }

    Ok(())
}
