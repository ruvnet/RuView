//! Example: GGUF model loading and inspection
//!
//! Demonstrates how to parse and inspect GGUF model files.

use ruvector_sparse_inference::model::{GgufParser, ModelMetadata};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("GGUF Model Loader Example");
    println!("==========================\n");

    // Example: Parse a minimal GGUF file structure
    // In practice, you would load this from a file:
    // let data = std::fs::read("model.gguf")?;

    // Create a minimal valid GGUF header for demonstration
    let mut data = Vec::new();

    // Magic number "GGUF" = 0x46554747
    data.extend_from_slice(&0x46554747u32.to_le_bytes());

    // Version 3
    data.extend_from_slice(&3u32.to_le_bytes());

    // Tensor count: 0
    data.extend_from_slice(&0u64.to_le_bytes());

    // Metadata KV count: 1
    data.extend_from_slice(&1u64.to_le_bytes());

    // Add one metadata entry: general.architecture = "llama"
    // Key: "general.architecture" (22 chars)
    data.extend_from_slice(&22u64.to_le_bytes());
    data.extend_from_slice(b"general.architecture");
    data.extend_from_slice(&[0, 0]); // Padding

    // Value type: String (8)
    data.extend_from_slice(&8u32.to_le_bytes());

    // String value: "llama" (5 chars)
    data.extend_from_slice(&5u64.to_le_bytes());
    data.extend_from_slice(b"llama");
    data.extend_from_slice(&[0, 0, 0]); // Padding

    // Parse the GGUF file
    match GgufParser::parse(&data) {
        Ok(model) => {
            println!("✓ Successfully parsed GGUF file");
            println!("  Magic: 0x{:08X}", model.header.magic);
            println!("  Version: {}", model.header.version);
            println!("  Tensor count: {}", model.header.tensor_count);
            println!("  Metadata entries: {}", model.header.metadata_kv_count);
            println!("\nMetadata:");
            for (key, value) in &model.metadata {
                println!("  {} = {:?}", key, value);
            }

            // Extract model metadata
            match ModelMetadata::from_gguf(&model) {
                Ok(metadata) => {
                    println!("\nModel Configuration:");
                    println!("  Architecture: {:?}", metadata.architecture);
                    println!("  Hidden size: {}", metadata.hidden_size);
                    println!("  Num layers: {}", metadata.num_layers);
                    println!("  Num heads: {}", metadata.num_heads);
                }
                Err(e) => {
                    println!("\n⚠ Could not extract full metadata (demo data): {}", e);
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to parse GGUF file: {}", e);
        }
    }

    Ok(())
}
