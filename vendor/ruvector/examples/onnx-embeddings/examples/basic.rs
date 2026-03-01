//! Basic embedding example demonstrating single text embedding

use anyhow::Result;
use ruvector_onnx_embeddings::{Embedder, EmbedderConfig, PretrainedModel};

#[tokio::main]
async fn main() -> Result<()> {
    // Create embedder with a specific model
    let config = EmbedderConfig::pretrained(PretrainedModel::AllMiniLmL6V2);
    let mut embedder = Embedder::new(config).await?;

    // Embed text
    let text = "Hello, RuVector!";
    let embedding = embedder.embed_one(text)?;

    println!("Text: {}", text);
    println!("Embedding dimension: {}", embedding.len());
    println!("First 10 values: {:?}", &embedding[..10]);

    // Compute similarity
    let similar_text = "Greetings, RuVector!";
    let different_text = "The weather is sunny.";

    let sim1 = embedder.similarity(text, similar_text)?;
    let sim2 = embedder.similarity(text, different_text)?;

    println!("\nSimilarity scores:");
    println!("  '{}' <-> '{}': {:.4}", text, similar_text, sim1);
    println!("  '{}' <-> '{}': {:.4}", text, different_text, sim2);

    Ok(())
}
