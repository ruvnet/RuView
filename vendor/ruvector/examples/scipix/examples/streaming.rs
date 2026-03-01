//! Streaming PDF processing example
//!
//! This example demonstrates streaming processing of large PDF documents.
//! Results are streamed as pages are processed, with real-time progress reporting.
//!
//! Usage:
//! ```bash
//! cargo run --example streaming -- document.pdf output/
//! ```

use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use ruvector_scipix::ocr::OcrEngine;
use ruvector_scipix::output::{OcrResult, OutputFormat};
use ruvector_scipix::OcrConfig;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
struct PageResult {
    page_number: usize,
    text: String,
    latex: Option<String>,
    confidence: f32,
    processing_time_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DocumentResult {
    total_pages: usize,
    pages: Vec<PageResult>,
    total_processing_time_ms: u64,
    average_confidence: f32,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <pdf_path> <output_directory>", args[0]);
        eprintln!("\nExample:");
        eprintln!("  {} document.pdf ./output", args[0]);
        std::process::exit(1);
    }

    let pdf_path = Path::new(&args[1]);
    let output_dir = Path::new(&args[2]);

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Create output directory
    fs::create_dir_all(output_dir).await?;

    println!("Loading PDF: {}", pdf_path.display());

    // Extract pages from PDF
    let pages = extract_pdf_pages(pdf_path)?;
    println!("Extracted {} pages", pages.len());

    // Initialize OCR engine
    let config = OcrConfig::default();
    let engine = OcrEngine::new(config).await?;

    // Setup progress bar
    let progress = ProgressBar::new(pages.len() as u64);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );

    let start_time = std::time::Instant::now();
    let mut page_results = Vec::new();

    // Process pages as a stream
    let mut stream = stream::iter(pages.into_iter().enumerate())
        .map(|(idx, page_data)| {
            let engine = &engine;
            async move { process_page(engine, idx + 1, page_data).await }
        })
        .buffer_unordered(4); // Process 4 pages concurrently

    // Stream results and save incrementally
    while let Some(result) = stream.next().await {
        match result {
            Ok(page_result) => {
                // Save individual page result
                let page_file =
                    output_dir.join(format!("page_{:04}.json", page_result.page_number));
                let json = serde_json::to_string_pretty(&page_result)?;
                fs::write(&page_file, json).await?;

                progress.set_message(format!(
                    "Page {} - {:.1}%",
                    page_result.page_number,
                    page_result.confidence * 100.0
                ));
                progress.inc(1);

                page_results.push(page_result);
            }
            Err(e) => {
                eprintln!("Error processing page: {}", e);
                progress.inc(1);
            }
        }
    }

    progress.finish_with_message("Complete");

    let total_time = start_time.elapsed().as_millis() as u64;

    // Calculate statistics
    let avg_confidence =
        page_results.iter().map(|p| p.confidence).sum::<f32>() / page_results.len() as f32;

    // Create document result
    let doc_result = DocumentResult {
        total_pages: page_results.len(),
        pages: page_results,
        total_processing_time_ms: total_time,
        average_confidence: avg_confidence,
    };

    // Save complete document result
    let doc_file = output_dir.join("document.json");
    let json = serde_json::to_string_pretty(&doc_result)?;
    fs::write(&doc_file, json).await?;

    println!("\n{}", "=".repeat(80));
    println!("Processing Complete");
    println!("{}", "=".repeat(80));
    println!("Total pages: {}", doc_result.total_pages);
    println!("Total time: {:.2}s", total_time as f32 / 1000.0);
    println!(
        "Average time per page: {:.2}s",
        (total_time as f32 / doc_result.total_pages as f32) / 1000.0
    );
    println!("Average confidence: {:.2}%", avg_confidence * 100.0);
    println!("Results saved to: {}", output_dir.display());
    println!("{}", "=".repeat(80));

    Ok(())
}

fn extract_pdf_pages(pdf_path: &Path) -> Result<Vec<Vec<u8>>> {
    // TODO: Implement actual PDF extraction using pdf-extract or similar
    // For now, this is a placeholder that returns mock data
    println!("Note: PDF extraction is not yet implemented");
    println!("This example shows the streaming architecture");

    // Mock implementation - in real use, this would extract actual PDF pages
    Ok(vec![vec![0u8; 100]]) // Placeholder
}

async fn process_page(
    engine: &OcrEngine,
    page_number: usize,
    page_data: Vec<u8>,
) -> Result<PageResult> {
    let start = std::time::Instant::now();

    // TODO: Convert page_data to image
    // For now, using a placeholder
    let image = image::DynamicImage::new_rgb8(100, 100);

    let result = engine
        .recognize(&image)
        .await
        .context(format!("Failed to process page {}", page_number))?;

    let processing_time = start.elapsed().as_millis() as u64;

    Ok(PageResult {
        page_number,
        text: result.text.clone(),
        latex: result.to_format(OutputFormat::LaTeX).ok(),
        confidence: result.confidence,
        processing_time_ms: processing_time,
    })
}
