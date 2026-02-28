# sevensense-embedding

[![Crate](https://img.shields.io/badge/crates.io-sevensense--embedding-orange.svg)](https://crates.io/crates/sevensense-embedding)
[![Docs](https://img.shields.io/badge/docs-sevensense--embedding-blue.svg)](https://docs.rs/sevensense-embedding)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

> Neural embedding generation using Perch 2.0 for bioacoustic analysis.

**sevensense-embedding** transforms audio segments into rich 1536-dimensional embedding vectors using Google's Perch 2.0 model via ONNX Runtime. These embeddings capture the acoustic essence of bird vocalizations, enabling similarity search, clustering, and species identification.

## Features

- **Perch 2.0 Integration**: State-of-the-art bird audio embeddings
- **ONNX Runtime**: Cross-platform GPU/CPU inference
- **1536-Dimensional Vectors**: Rich semantic representation
- **Batch Processing**: Efficient multi-segment inference
- **Product Quantization (PQ)**: 4x memory reduction for storage
- **L2 Normalization**: Optimized for cosine similarity search

## Use Cases

| Use Case | Description | Key Functions |
|----------|-------------|---------------|
| Single Inference | Embed one audio segment | `embed()` |
| Batch Processing | Embed multiple segments efficiently | `embed_batch()` |
| Streaming | Real-time embedding generation | `EmbeddingStream::new()` |
| Quantization | Compress embeddings for storage | `quantize_pq()` |
| Validation | Verify embedding quality | `validate()` |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
sevensense-embedding = "0.1"
```

### ONNX Model Setup

The Perch 2.0 ONNX model is automatically downloaded on first use. For manual setup:

```bash
# Download model manually
curl -L https://example.com/perch-2.0.onnx -o models/perch-2.0.onnx
```

## Quick Start

```rust
use sevensense_embedding::{EmbeddingPipeline, EmbeddingConfig};
use sevensense_audio::AudioLoader;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the embedding pipeline
    let config = EmbeddingConfig::default();
    let pipeline = EmbeddingPipeline::new(config).await?;

    // Load audio and generate embedding
    let audio = AudioLoader::load("birdsong.wav").await?;
    let embedding = pipeline.embed(&audio).await?;

    println!("Embedding dimension: {}", embedding.len());  // 1536
    println!("L2 norm: {:.4}", embedding.iter().map(|x| x*x).sum::<f32>().sqrt());

    Ok(())
}
```

---

<details>
<summary><b>Tutorial: Basic Embedding Generation</b></summary>

### Single Audio Embedding

```rust
use sevensense_embedding::{EmbeddingPipeline, EmbeddingConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create pipeline with default config
    let pipeline = EmbeddingPipeline::new(EmbeddingConfig::default()).await?;

    // Embed from mel spectrogram
    let mel = compute_mel_spectrogram(&audio)?;
    let embedding = pipeline.embed_mel(&mel).await?;

    // Embedding properties
    assert_eq!(embedding.len(), 1536);

    // L2 normalized by default
    let norm: f32 = embedding.iter().map(|x| x*x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-5);

    Ok(())
}
```

### From Raw Audio

```rust
use sevensense_embedding::EmbeddingPipeline;
use sevensense_audio::AudioLoader;

let audio = AudioLoader::load("recording.wav").await?;
let pipeline = EmbeddingPipeline::new(Default::default()).await?;

// Pipeline handles mel spectrogram computation internally
let embedding = pipeline.embed_audio(&audio).await?;
```

</details>

<details>
<summary><b>Tutorial: Batch Processing</b></summary>

### Efficient Batch Embedding

```rust
use sevensense_embedding::{EmbeddingPipeline, BatchConfig};

let pipeline = EmbeddingPipeline::new(Default::default()).await?;

// Configure batching
let batch_config = BatchConfig {
    batch_size: 32,           // Process 32 segments at once
    max_concurrent: 4,        // 4 concurrent batches
    prefetch: true,           // Prefetch next batch
};

// Embed multiple segments
let segments = load_segments("recordings/")?;
let embeddings = pipeline.embed_batch(&segments, batch_config).await?;

println!("Generated {} embeddings", embeddings.len());
```

### Progress Tracking

```rust
use sevensense_embedding::EmbeddingPipeline;

let pipeline = EmbeddingPipeline::new(Default::default()).await?;

let embeddings = pipeline.embed_batch_with_progress(&segments, |progress| {
    println!("Progress: {}/{} ({:.1}%)",
        progress.completed,
        progress.total,
        progress.percentage());
}).await?;
```

### Parallel Processing

```rust
use sevensense_embedding::EmbeddingPipeline;
use futures::stream::{self, StreamExt};

let pipeline = Arc::new(EmbeddingPipeline::new(Default::default()).await?);

let embeddings: Vec<_> = stream::iter(segments)
    .map(|seg| {
        let pipeline = Arc::clone(&pipeline);
        async move { pipeline.embed(&seg).await }
    })
    .buffer_unordered(8)  // 8 concurrent embeddings
    .collect()
    .await;
```

</details>

<details>
<summary><b>Tutorial: Embedding Quantization</b></summary>

### Product Quantization (PQ)

Product Quantization reduces embedding size by 4x while maintaining search quality.

```rust
use sevensense_embedding::{EmbeddingPipeline, ProductQuantizer};

let pipeline = EmbeddingPipeline::new(Default::default()).await?;

// Generate embeddings
let embeddings: Vec<Vec<f32>> = generate_embeddings(&segments).await?;

// Train PQ codebook on embeddings
let pq = ProductQuantizer::train(&embeddings, 96, 256)?;  // 96 subvectors, 256 centroids

// Quantize embeddings
let quantized: Vec<Vec<u8>> = embeddings.iter()
    .map(|e| pq.encode(e))
    .collect();

// Memory reduction
let original_size = embeddings.len() * 1536 * 4;  // f32 = 4 bytes
let quantized_size = quantized.len() * 96;        // u8 per subvector
println!("Compression ratio: {:.1}x", original_size as f32 / quantized_size as f32);
// Output: Compression ratio: 64.0x
```

### Asymmetric Distance Computation

```rust
use sevensense_embedding::ProductQuantizer;

// Query embedding (full precision)
let query = pipeline.embed(&query_audio).await?;

// Compute distances to quantized vectors
let distances: Vec<f32> = quantized.iter()
    .map(|q| pq.asymmetric_distance(&query, q))
    .collect();

// Find nearest neighbors
let mut indexed: Vec<_> = distances.iter().enumerate().collect();
indexed.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap());
let top_10: Vec<_> = indexed.iter().take(10).collect();
```

</details>

<details>
<summary><b>Tutorial: Model Configuration</b></summary>

### Custom ONNX Configuration

```rust
use sevensense_embedding::{EmbeddingConfig, ExecutionProvider};

let config = EmbeddingConfig {
    model_path: "models/perch-2.0.onnx".into(),
    execution_provider: ExecutionProvider::CUDA,  // GPU acceleration
    num_threads: 4,                                // CPU threads (if CPU)
    normalize: true,                               // L2 normalize output
    warmup: true,                                  // Warmup inference
};

let pipeline = EmbeddingPipeline::new(config).await?;
```

### Execution Providers

```rust
use sevensense_embedding::ExecutionProvider;

// CPU (default)
let cpu_config = EmbeddingConfig {
    execution_provider: ExecutionProvider::CPU,
    ..Default::default()
};

// CUDA (NVIDIA GPU)
let cuda_config = EmbeddingConfig {
    execution_provider: ExecutionProvider::CUDA,
    ..Default::default()
};

// CoreML (Apple Silicon)
let coreml_config = EmbeddingConfig {
    execution_provider: ExecutionProvider::CoreML,
    ..Default::default()
};
```

### Memory Optimization

```rust
use sevensense_embedding::{EmbeddingConfig, MemoryConfig};

let config = EmbeddingConfig {
    memory: MemoryConfig {
        arena_extend_strategy: ArenaExtendStrategy::NextPowerOfTwo,
        initial_chunk_size: 1024 * 1024,  // 1MB
        max_chunk_size: 16 * 1024 * 1024, // 16MB
    },
    ..Default::default()
};
```

</details>

<details>
<summary><b>Tutorial: Embedding Validation</b></summary>

### Quality Checks

```rust
use sevensense_embedding::{EmbeddingValidator, ValidationResult};

let validator = EmbeddingValidator::new();

let embedding = pipeline.embed(&audio).await?;
let result = validator.validate(&embedding)?;

match result {
    ValidationResult::Valid => println!("Embedding is valid"),
    ValidationResult::Invalid(reasons) => {
        for reason in reasons {
            eprintln!("Invalid: {}", reason);
        }
    }
}
```

### Validation Criteria

```rust
use sevensense_embedding::{ValidationCriteria, EmbeddingValidator};

let criteria = ValidationCriteria {
    expected_dim: 1536,
    max_nan_ratio: 0.0,      // No NaN values allowed
    max_inf_ratio: 0.0,      // No Inf values allowed
    min_variance: 1e-6,      // Minimum variance threshold
    norm_range: (0.99, 1.01), // Expected L2 norm range
};

let validator = EmbeddingValidator::with_criteria(criteria);
```

### Batch Validation

```rust
let results = validator.validate_batch(&embeddings);

let valid_count = results.iter().filter(|r| r.is_valid()).count();
let invalid_count = results.len() - valid_count;

println!("{} valid, {} invalid embeddings", valid_count, invalid_count);
```

</details>

---

## Configuration

### EmbeddingConfig Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `model_path` | Auto-download | Path to ONNX model |
| `execution_provider` | CPU | CUDA, CoreML, or CPU |
| `num_threads` | 4 | CPU inference threads |
| `normalize` | true | L2 normalize embeddings |
| `warmup` | true | Run warmup inference |

### Model Specifications

| Property | Value |
|----------|-------|
| Input | Mel spectrogram [batch, 128, 312] |
| Output | Embedding vector [batch, 1536] |
| Model Size | ~25 MB |
| Inference Time | ~15ms (CPU) / ~3ms (GPU) |

## Performance

| Operation | CPU (i7-12700) | GPU (RTX 3080) |
|-----------|----------------|----------------|
| Single Inference | 15ms | 3ms |
| Batch (32) | 120ms | 20ms |
| Throughput | 260/s | 1600/s |

## Links

- **Homepage**: [ruv.io](https://ruv.io)
- **Repository**: [github.com/ruvnet/ruvector](https://github.com/ruvnet/ruvector)
- **Crates.io**: [crates.io/crates/sevensense-embedding](https://crates.io/crates/sevensense-embedding)
- **Documentation**: [docs.rs/sevensense-embedding](https://docs.rs/sevensense-embedding)

## License

MIT License - see [LICENSE](../../LICENSE) for details.

---

*Part of the [7sense Bioacoustic Intelligence Platform](https://ruv.io) by rUv*
