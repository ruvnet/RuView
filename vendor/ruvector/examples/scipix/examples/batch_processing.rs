//! Batch processing example
//!
//! This example demonstrates parallel batch processing of multiple images.
//! It processes all images in a directory concurrently with a progress bar.
//!
//! Note: This example requires the `ocr` feature to be enabled.
//!
//! Usage:
//! ```bash
//! cargo run --example batch_processing --features ocr -- /path/to/images output.json
//! ```

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use ruvector_scipix::ocr::OcrEngine;
use ruvector_scipix::output::{OcrResult, OutputFormat};
use ruvector_scipix::OcrConfig;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Debug, Serialize, Deserialize)]
struct BatchResult {
    file_path: String,
    success: bool,
    text: Option<String>,
    latex: Option<String>,
    confidence: Option<f32>,
    error: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <image_directory> <output_json>", args[0]);
        eprintln!("\nExample:");
        eprintln!("  {} ./images results.json", args[0]);
        std::process::exit(1);
    }

    let image_dir = Path::new(&args[1]);
    let output_file = &args[2];

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Collect all image files
    let image_files = collect_image_files(image_dir)?;

    if image_files.is_empty() {
        eprintln!("No image files found in: {}", image_dir.display());
        std::process::exit(1);
    }

    println!("Found {} images to process", image_files.len());

    // Initialize OCR engine
    let config = OcrConfig::default();
    let engine = Arc::new(OcrEngine::new(config).await?);

    // Create progress bar
    let progress = ProgressBar::new(image_files.len() as u64);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );

    // Limit concurrent processing to avoid overwhelming the system
    let max_concurrent = num_cpus::get();
    let semaphore = Arc::new(Semaphore::new(max_concurrent));

    // Process images in parallel
    let mut tasks = Vec::new();

    for image_path in image_files {
        let engine = Arc::clone(&engine);
        let semaphore = Arc::clone(&semaphore);
        let progress = progress.clone();

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            let result = process_image(&engine, &image_path).await;
            progress.inc(1);

            result
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await?);
    }

    progress.finish_with_message("Complete");

    // Calculate statistics
    let successful = results.iter().filter(|r| r.success).count();
    let failed = results.len() - successful;
    let avg_confidence =
        results.iter().filter_map(|r| r.confidence).sum::<f32>() / successful as f32;

    println!("\n{}", "=".repeat(80));
    println!("Batch Processing Complete");
    println!("{}", "=".repeat(80));
    println!("Total: {}", results.len());
    println!(
        "Successful: {} ({:.1}%)",
        successful,
        (successful as f32 / results.len() as f32) * 100.0
    );
    println!("Failed: {}", failed);
    println!("Average Confidence: {:.2}%", avg_confidence * 100.0);
    println!("{}", "=".repeat(80));

    // Save results to JSON
    let json = serde_json::to_string_pretty(&results)?;
    std::fs::write(output_file, json)?;
    println!("\nResults saved to: {}", output_file);

    Ok(())
}

fn collect_image_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let extensions = ["png", "jpg", "jpeg", "bmp", "tiff", "webp"];

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                if extensions.contains(&ext.to_str().unwrap_or("").to_lowercase().as_str()) {
                    files.push(path);
                }
            }
        }
    }

    Ok(files)
}

async fn process_image(engine: &OcrEngine, path: &Path) -> BatchResult {
    let file_path = path.to_string_lossy().to_string();

    match image::open(path) {
        Ok(img) => match engine.recognize(&img).await {
            Ok(result) => BatchResult {
                file_path,
                success: true,
                text: Some(result.text.clone()),
                latex: result.to_format(ruvector_scipix::OutputFormat::LaTeX).ok(),
                confidence: Some(result.confidence),
                error: None,
            },
            Err(e) => BatchResult {
                file_path,
                success: false,
                text: None,
                latex: None,
                confidence: None,
                error: Some(e.to_string()),
            },
        },
        Err(e) => BatchResult {
            file_path,
            success: false,
            text: None,
            latex: None,
            confidence: None,
            error: Some(e.to_string()),
        },
    }
}
