//! Batch embedding example with parallel processing

use anyhow::Result;
use ruvector_onnx_embeddings::{
    EmbedderBuilder, PretrainedModel, PoolingStrategy,
};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    // Create embedder with custom settings
    let mut embedder = EmbedderBuilder::new()
        .pretrained(PretrainedModel::AllMiniLmL6V2)
        .pooling(PoolingStrategy::Mean)
        .normalize(true)
        .batch_size(32)
        .max_length(256)
        .build()
        .await?;

    // Generate test data
    let texts: Vec<String> = (0..100)
        .map(|i| format!("This is test sentence number {} for batch embedding.", i))
        .collect();

    println!("Embedding {} texts...", texts.len());

    // Sequential embedding
    let start = Instant::now();
    let output = embedder.embed(&texts)?;
    let seq_time = start.elapsed();

    println!("Sequential: {:?} ({:.2} texts/sec)",
        seq_time,
        texts.len() as f64 / seq_time.as_secs_f64()
    );

    // Parallel embedding
    let start = Instant::now();
    let output_parallel = embedder.embed_parallel(&texts)?;
    let par_time = start.elapsed();

    println!("Parallel: {:?} ({:.2} texts/sec)",
        par_time,
        texts.len() as f64 / par_time.as_secs_f64()
    );

    println!("\nSpeedup: {:.2}x", seq_time.as_secs_f64() / par_time.as_secs_f64());
    println!("Total embeddings: {}", output.len());
    println!("Dimension: {}", output.dimension);

    Ok(())
}
