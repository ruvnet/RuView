# ruvector-scipix Examples

This directory contains comprehensive examples demonstrating various features and use cases of ruvector-scipix.

## Quick Start

All examples can be run using:
```bash
cargo run --example <example_name> -- [arguments]
```

## Examples Overview

### 1. Simple OCR (`simple_ocr.rs`)

**Basic OCR functionality with single image processing.**

Demonstrates:
- Loading and processing a single image
- OCR recognition
- Output in multiple formats (plain text, LaTeX)
- Confidence scores

**Usage:**
```bash
cargo run --example simple_ocr -- path/to/image.png
```

**Example Output:**
```
Plain Text: x² + 2x + 1 = 0
LaTeX: x^{2} + 2x + 1 = 0
Confidence: 95.3%
```

---

### 2. Batch Processing (`batch_processing.rs`)

**Parallel processing of multiple images with progress tracking.**

Demonstrates:
- Directory-based batch processing
- Parallel/concurrent processing
- Progress bar visualization
- Statistics and metrics
- JSON output

**Usage:**
```bash
cargo run --example batch_processing -- /path/to/images output.json
```

**Features:**
- Automatic CPU core detection for optimal parallelism
- Real-time progress visualization
- Per-file error handling
- Aggregate statistics

---

### 3. API Server (`api_server.rs`)

**REST API server for OCR processing.**

Demonstrates:
- HTTP server with Axum framework
- Single and batch image processing endpoints
- Health check endpoint
- Graceful shutdown
- CORS support
- Multipart file uploads

**Usage:**
```bash
# Start server
cargo run --example api_server

# In another terminal, test the API
curl -X POST -F "image=@equation.png" http://localhost:8080/ocr
curl http://localhost:8080/health
```

**Endpoints:**
- `GET /health` - Health check
- `POST /ocr` - Process single image
- `POST /batch` - Process multiple images

---

### 4. Streaming Processing (`streaming.rs`)

**Streaming PDF processing with real-time results.**

Demonstrates:
- Large document processing
- Streaming results as pages are processed
- Real-time progress reporting
- Incremental JSON output
- Memory-efficient processing

**Usage:**
```bash
cargo run --example streaming -- document.pdf output/
```

**Features:**
- Processes pages concurrently (4 at a time)
- Saves individual page results immediately
- Generates final document summary
- Per-page timing statistics

---

### 5. Custom Pipeline (`custom_pipeline.rs`)

**Custom OCR pipeline with preprocessing and post-processing.**

Demonstrates:
- Image preprocessing (denoising, sharpening, binarization)
- Post-processing filters
- LaTeX validation
- Confidence filtering
- Custom output formatting
- Otsu's thresholding

**Usage:**
```bash
cargo run --example custom_pipeline -- image.png
```

**Pipeline Steps:**
1. **Preprocessing:**
   - Denoising
   - Contrast enhancement
   - Sharpening
   - Binarization (Otsu's method)
   - Deskewing

2. **Post-processing:**
   - Confidence filtering
   - LaTeX validation
   - Spell checking
   - Custom formatting

---

### 6. WASM Browser Demo (`wasm_demo.html`)

**Browser-based OCR demonstration.**

Demonstrates:
- WebAssembly integration
- Browser-based image upload
- Drag-and-drop interface
- Real-time visualization
- Client-side processing

**Setup:**
```bash
# Build WASM module (when available)
wasm-pack build --target web

# Serve the demo
python3 -m http.server 8000
# Open http://localhost:8000/examples/wasm_demo.html
```

**Features:**
- Modern, responsive UI
- Drag-and-drop file upload
- Live preview
- Real-time results
- No server required (runs in browser)

---

### 7. Agent-Based Processing (`lean_agentic.rs`)

**Distributed OCR processing with agent coordination.**

Demonstrates:
- Multi-agent coordination
- Distributed task processing
- Fault tolerance
- Load balancing
- Agent statistics

**Usage:**
```bash
cargo run --example lean_agentic -- /path/to/documents
```

**Features:**
- Spawns multiple OCR agents (default: 4)
- Automatic task distribution
- Per-agent statistics
- Throughput metrics
- JSON result export

**Architecture:**
```
Coordinator
├── Agent 1 (tasks: 12)
├── Agent 2 (tasks: 15)
├── Agent 3 (tasks: 11)
└── Agent 4 (tasks: 13)
```

---

### 8. Accuracy Testing (`accuracy_test.rs`)

**OCR accuracy testing against ground truth datasets.**

Demonstrates:
- Dataset-based testing
- Multiple accuracy metrics
- Category-based analysis
- Statistical correlation
- Comprehensive reporting

**Usage:**
```bash
cargo run --example accuracy_test -- dataset.json
```

**Dataset Format:**
```json
[
  {
    "image_path": "tests/images/quadratic.png",
    "ground_truth_text": "x^2 + 2x + 1 = 0",
    "ground_truth_latex": "x^{2} + 2x + 1 = 0",
    "category": "quadratic"
  }
]
```

**Metrics Calculated:**
- **Text Accuracy** - Overall string similarity
- **Character Error Rate (CER)** - Character-level errors
- **Word Error Rate (WER)** - Word-level errors
- **LaTeX Accuracy** - LaTeX format correctness
- **Confidence Correlation** - Pearson correlation between confidence and accuracy
- **Category Breakdown** - Per-category statistics

**Example Output:**
```
Total Cases: 100
Successful: 98 (98.0%)
Average Confidence: 92.5%
Average Text Accuracy: 94.2%
Average CER: 3.1%
Average WER: 5.8%
Confidence Correlation: 0.847

Category Breakdown:
  quadratic: 25 cases, 96.3% accuracy
  linear: 30 cases, 98.1% accuracy
  calculus: 20 cases, 89.7% accuracy
```

---

## Common Patterns

### Error Handling
All examples use `anyhow::Result` for error handling:
```rust
use anyhow::{Context, Result};

fn main() -> Result<()> {
    let image = image::open(path)
        .context("Failed to open image")?;
    Ok(())
}
```

### Logging
Examples use `env_logger` for debug output:
```bash
# Run with debug logging
RUST_LOG=debug cargo run --example simple_ocr -- image.png

# Run with info logging (default)
RUST_LOG=info cargo run --example simple_ocr -- image.png
```

### Configuration
OCR engine configuration:
```rust
use ruvector_scipix::OcrConfig;

let config = OcrConfig {
    confidence_threshold: 0.7,
    max_image_size: 4096,
    enable_preprocessing: true,
    // ... other options
};
```

## Dependencies

Core dependencies used in examples:
- `anyhow` - Error handling
- `tokio` - Async runtime
- `image` - Image processing
- `serde/serde_json` - Serialization
- `indicatif` - Progress bars
- `axum` - HTTP server (api_server)
- `env_logger` - Logging

## Building Examples

Build all examples:
```bash
cargo build --examples
```

Build specific example:
```bash
cargo build --example simple_ocr
```

Run with optimizations:
```bash
cargo run --release --example batch_processing -- images/ output.json
```

## Testing Examples

Create test images:
```bash
# Create test directory
mkdir -p test_images

# Add some test images
cp /path/to/math_equation.png test_images/
```

Run examples:
```bash
# Simple OCR
cargo run --example simple_ocr -- test_images/equation.png

# Batch processing
cargo run --example batch_processing -- test_images/ results.json

# Accuracy test (requires dataset)
cargo run --example accuracy_test -- test_dataset.json
```

## Integration Guide

### Using in Your Project

1. **Add dependency:**
```toml
[dependencies]
ruvector-scipix = "0.1.0"
```

2. **Basic usage:**
```rust
use ruvector_scipix::{OcrEngine, OcrConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = OcrConfig::default();
    let engine = OcrEngine::new(config).await?;

    let image = image::open("equation.png")?;
    let result = engine.recognize(&image).await?;

    println!("Text: {}", result.text);
    Ok(())
}
```

3. **Advanced usage:**
See individual examples for advanced patterns like:
- Custom pipelines
- Batch processing
- API integration
- Agent-based processing

## Performance Tips

1. **Batch Processing:**
   - Use parallel processing for multiple images
   - Adjust concurrency based on CPU cores
   - Enable model caching for repeated runs

2. **Memory Management:**
   - Stream large documents instead of loading all at once
   - Use appropriate image resolution (downscale if needed)
   - Clear cache periodically for long-running processes

3. **Accuracy vs Speed:**
   - Higher confidence thresholds = more accuracy, slower processing
   - Preprocessing improves accuracy but adds overhead
   - Balance based on your use case

## Troubleshooting

### Common Issues

**"Model not found"**
```bash
# Download models first
./scripts/download_models.sh
```

**"Out of memory"**
- Reduce batch size or concurrent workers
- Downscale large images before processing
- Enable streaming for PDFs

**"Low confidence scores"**
- Enable preprocessing pipeline
- Improve image quality (resolution, contrast)
- Check for skewed or rotated images

## Contributing

When adding new examples:
1. Add the `.rs` file to `examples/`
2. Update `Cargo.toml` with example entry
3. Document in this README
4. Include usage examples and expected output
5. Add error handling and logging
6. Keep examples self-contained

## Resources

- [Main Documentation](../README.md)
- [API Reference](../docs/API.md)
- [Model Guide](../docs/MODELS.md)
- [Benchmarks](../benches/README.md)

## License

All examples are provided under the same license as ruvector-scipix.
