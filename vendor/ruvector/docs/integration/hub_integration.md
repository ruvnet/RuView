# HuggingFace Hub Integration for RuvLTRA

This document describes the HuggingFace Hub integration for publishing and downloading RuvLTRA models.

## Overview

The `ruvllm::hub` module provides comprehensive functionality for:

1. **Model Download**: Pull GGUF files from HuggingFace Hub with progress tracking and resume support
2. **Model Upload**: Push models to HuggingFace Hub with automatic model card generation
3. **Model Registry**: Pre-configured RuvLTRA model collection with hardware requirements
4. **Progress Tracking**: Visual progress bars with ETA and speed indicators
5. **Checksum Verification**: SHA256 validation for downloaded files

## Module Structure

```
crates/ruvllm/src/hub/
├── mod.rs              # Main module with exports and common types
├── download.rs         # Model download functionality
├── upload.rs           # Model upload functionality
├── registry.rs         # RuvLTRA model registry
├── model_card.rs       # HuggingFace model card generation
└── progress.rs         # Progress tracking utilities
```

## Model Registry

The registry includes pre-configured RuvLTRA models:

### Base Models

| Model ID | Size | Params | Quantization | Use Case |
|----------|------|--------|--------------|----------|
| `ruvltra-small` | 662MB | 0.5B | Q4_K_M | Edge devices, includes SONA weights |
| `ruvltra-small-q8` | 1.3GB | 0.5B | Q8_0 | High quality, small model |
| `ruvltra-medium` | 2.1GB | 3B | Q4_K_M | General purpose, extended context |
| `ruvltra-medium-q8` | 4.2GB | 3B | Q8_0 | High quality, balanced model |

### LoRA Adapters

| Adapter ID | Size | Base Model | Purpose |
|------------|------|------------|---------|
| `ruvltra-small-coder` | 50MB | ruvltra-small | Code completion specialization |

## Usage

### 1. Model Download

#### Using the CLI Example

```bash
# Download RuvLTRA Small
cargo run -p ruvllm --example hub_cli -- pull ruvltra-small

# Download to custom directory
cargo run -p ruvllm --example hub_cli -- pull ruvltra-medium --output ./models

# List all available models
cargo run -p ruvllm --example hub_cli -- list

# Show detailed model info
cargo run -p ruvllm --example hub_cli -- info ruvltra-small
```

#### Using the API

```rust
use ruvllm::hub::{ModelDownloader, RuvLtraRegistry};

// Download by model ID
let downloader = ModelDownloader::new();
let path = downloader.download_by_id("ruvltra-small")?;

// Or download with custom config
let registry = RuvLtraRegistry::new();
let model_info = registry.get("ruvltra-small").unwrap();

let config = DownloadConfig {
    cache_dir: PathBuf::from("./models"),
    resume: true,
    show_progress: true,
    verify_checksum: true,
    ..Default::default()
};

let downloader = ModelDownloader::with_config(config);
let path = downloader.download(model_info, None)?;
```

### 2. Model Upload

#### Using the CLI Example

```bash
# Upload a custom model (requires HF_TOKEN)
export HF_TOKEN=your_huggingface_token

cargo run -p ruvllm --example hub_cli -- push \
  --model ./my-ruvltra-custom.gguf \
  --repo username/my-ruvltra-custom \
  --description "My custom RuvLTRA model" \
  --params 0.5 \
  --architecture llama \
  --context 4096 \
  --quant Q4_K_M
```

#### Using the API

```rust
use ruvllm::hub::{ModelUploader, ModelMetadata, UploadConfig};

// Create metadata
let metadata = ModelMetadata {
    name: "My RuvLTRA Model".to_string(),
    description: Some("A custom RuvLTRA variant".to_string()),
    architecture: "llama".to_string(),
    params_b: 0.5,
    context_length: 4096,
    quantization: Some("Q4_K_M".to_string()),
    license: Some("MIT".to_string()),
    datasets: vec!["custom-dataset".to_string()],
    tags: vec!["ruvltra".to_string(), "custom".to_string()],
};

// Configure uploader
let config = UploadConfig::new(hf_token)
    .private(false)
    .commit_message("Upload custom RuvLTRA model");

let uploader = ModelUploader::with_config(config);
let url = uploader.upload(
    "./my-model.gguf",
    "username/my-ruvltra-custom",
    Some(metadata),
)?;

println!("Model uploaded to: {}", url);
```

### 3. Model Registry

```rust
use ruvllm::hub::{RuvLtraRegistry, ModelSize};

let registry = RuvLtraRegistry::new();

// Get a specific model
let model = registry.get("ruvltra-small").unwrap();
println!("Model: {}", model.name);
println!("Size: {} MB", model.size_bytes / (1024 * 1024));

// List all models
for model in registry.list_all() {
    println!("{}: {}", model.id, model.description);
}

// List by size category
for model in registry.list_by_size(ModelSize::Small) {
    println!("Small model: {}", model.id);
}

// Get adapters for a base model
for adapter in registry.list_adapters("ruvltra-small") {
    println!("Adapter: {}", adapter.id);
}

// Recommend model based on available RAM
let model = registry.recommend_for_ram(4.0).unwrap();
println!("Recommended for 4GB RAM: {}", model.id);
```

### 4. Model Card Generation

```rust
use ruvllm::hub::{
    ModelCardBuilder, TaskType, Framework, License
};

let card = ModelCardBuilder::new("RuvLTRA Custom")
    .description("A custom RuvLTRA variant")
    .task(TaskType::TextGeneration)
    .framework(Framework::Gguf)
    .architecture("llama")
    .parameters(500_000_000)
    .context_length(4096)
    .license(License::Mit)
    .add_dataset("training-data", Some("Custom dataset".to_string()))
    .add_metric("perplexity", 5.2, Some("test-set".to_string()))
    .add_tag("ruvltra")
    .add_tag("custom")
    .build();

// Generate markdown for HuggingFace
let markdown = card.to_markdown();
```

### 5. Progress Tracking

```rust
use ruvllm::hub::{ProgressBar, ProgressStyle};

let mut pb = ProgressBar::new(total_bytes)
    .with_style(ProgressStyle::Detailed)
    .with_width(50);

// Update progress
pb.update(downloaded_bytes);

// Finish
pb.finish();
```

## Hardware Requirements

Each model in the registry includes hardware requirements:

```rust
let model = registry.get("ruvltra-small").unwrap();

println!("Minimum RAM: {:.1} GB", model.hardware.min_ram_gb);
println!("Recommended RAM: {:.1} GB", model.hardware.recommended_ram_gb);
println!("Apple Neural Engine: {}", model.hardware.supports_ane);
println!("Metal GPU: {}", model.hardware.supports_metal);
println!("CUDA: {}", model.hardware.supports_cuda);
```

## Environment Variables

- `HF_TOKEN`: HuggingFace API token (required for uploads and private repos)
- `HUGGING_FACE_HUB_TOKEN`: Alternative name for HF token
- `RUVLLM_MODELS_DIR`: Default cache directory for downloaded models

## Dependencies

The hub integration requires:

- `curl` or `wget` for downloads (uses system tools for efficiency)
- `huggingface-cli` for uploads (install with `pip install huggingface_hub[cli]`)
- SHA256 for checksum verification (built-in via `sha2` crate)

## Features

### Download Features

- ✅ Resume interrupted downloads
- ✅ Progress bar with ETA
- ✅ SHA256 checksum verification
- ✅ Automatic retry on failure
- ✅ HuggingFace token authentication
- ✅ Cache directory management

### Upload Features

- ✅ Automatic repository creation
- ✅ Model card generation
- ✅ Public/private repository support
- ✅ SONA weights upload
- ✅ Custom metadata
- ✅ Commit message customization

### Registry Features

- ✅ Pre-configured model catalog
- ✅ Hardware requirement tracking
- ✅ Quantization level support
- ✅ LoRA adapter registry
- ✅ RAM-based recommendations
- ✅ Download time estimation

## Error Handling

All hub operations return `Result<T, HubError>`:

```rust
use ruvllm::hub::{HubError, ModelDownloader};

match downloader.download_by_id("ruvltra-small") {
    Ok(path) => println!("Downloaded to: {}", path.display()),
    Err(HubError::NotFound(id)) => eprintln!("Model {} not found", id),
    Err(HubError::ChecksumMismatch { expected, actual }) => {
        eprintln!("Checksum mismatch: expected {}, got {}", expected, actual);
    }
    Err(HubError::Network(msg)) => eprintln!("Network error: {}", msg),
    Err(e) => eprintln!("Error: {}", e),
}
```

## Testing

Run the hub integration tests:

```bash
# Test model registry
cargo test -p ruvllm --lib hub::registry

# Test download (requires network)
cargo test -p ruvllm --lib hub::download

# Test model card generation
cargo test -p ruvllm --lib hub::model_card

# Run all hub tests
cargo test -p ruvllm --lib hub
```

## Examples

See the examples for complete usage:

1. `examples/download_test_model.rs` - Legacy downloader with hub integration
2. `examples/hub_cli.rs` - Full CLI with pull/push/list/info commands

## Future Enhancements

Planned improvements:

- [ ] Direct API uploads (without huggingface-cli dependency)
- [ ] Parallel chunk downloads for faster transfers
- [ ] Delta updates for model weights
- [ ] Model versioning support
- [ ] Automatic quantization variant selection
- [ ] Multi-repo synchronization
- [ ] Offline model registry cache

## Contributing

To add a new model to the registry:

1. Add model definition to `registry.rs` in `RuvLtraRegistry::new()`
2. Include hardware requirements
3. Set checksum after first upload
4. Update this documentation

## License

MIT License - See LICENSE file for details
