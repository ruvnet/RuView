//! Pre-download ONNX embedding models for Docker build
//!
//! This script downloads the default embedding model during Docker build
//! so it's available immediately at runtime without network access.

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

fn main() {
    println!("=== Downloading Embedding Models ===");

    // Download the default model (all-MiniLM-L6-v2)
    println!("Downloading all-MiniLM-L6-v2...");
    let options = InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(true);

    match TextEmbedding::try_new(options) {
        Ok(mut model) => {
            // Test the model works
            let result = model.embed(vec!["test"], None);
            match result {
                Ok(embeddings) => {
                    println!("✓ Model loaded successfully");
                    println!("  Embedding dimensions: {}", embeddings[0].len());
                }
                Err(e) => {
                    eprintln!("✗ Model test failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to download model: {}", e);
            std::process::exit(1);
        }
    }

    // Optionally download BGE-small for better quality
    println!("\nDownloading BAAI/bge-small-en-v1.5...");
    let options = InitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(true);

    match TextEmbedding::try_new(options) {
        Ok(_) => println!("✓ BGE-small model loaded successfully"),
        Err(e) => println!("⚠ BGE-small download failed (optional): {}", e),
    }

    println!("\n=== Model Download Complete ===");
}
