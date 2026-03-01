//! Simple OCR example
//!
//! This example demonstrates basic OCR functionality with ruvector-scipix.
//! It processes a single image and outputs the recognized text and LaTeX.
//!
//! Usage:
//! ```bash
//! cargo run --example simple_ocr -- image.png
//! ```

use anyhow::{Context, Result};
use ruvector_scipix::{OcrConfig, OcrEngine, OutputFormat};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <image_path>", args[0]);
        eprintln!("\nExample:");
        eprintln!("  {} equation.png", args[0]);
        std::process::exit(1);
    }

    let image_path = &args[1];

    // Initialize logger for debug output
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("Loading image: {}", image_path);

    // Create default OCR configuration
    let config = OcrConfig::default();

    // Initialize OCR engine
    println!("Initializing OCR engine...");
    let engine = OcrEngine::new(config)
        .await
        .context("Failed to initialize OCR engine")?;

    // Load and process the image
    let image = image::open(image_path).context(format!("Failed to open image: {}", image_path))?;

    println!("Processing image...");
    let result = engine
        .recognize(&image)
        .await
        .context("OCR recognition failed")?;

    // Display results
    println!("\n{}", "=".repeat(80));
    println!("OCR Results");
    println!("{}", "=".repeat(80));

    println!("\n📝 Plain Text:");
    println!("{}", result.text);

    println!("\n🔢 LaTeX:");
    println!("{}", result.to_format(OutputFormat::LaTeX)?);

    println!("\n📊 Confidence: {:.2}%", result.confidence * 100.0);

    if let Some(metadata) = &result.metadata {
        println!("\n📋 Metadata:");
        println!("  Language: {:?}", metadata.get("language"));
        println!(
            "  Processing time: {:?}",
            metadata.get("processing_time_ms")
        );
    }

    println!("\n{}", "=".repeat(80));

    Ok(())
}
