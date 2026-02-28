# SciPix - Rust OCR Engine for Scientific Documents & Math Equations

[![Crates.io](https://img.shields.io/crates/v/ruvector-scipix.svg)](https://crates.io/crates/ruvector-scipix)
[![Documentation](https://docs.rs/ruvector-scipix/badge.svg)](https://docs.rs/ruvector-scipix)
[![Downloads](https://img.shields.io/crates/d/ruvector-scipix.svg)](https://crates.io/crates/ruvector-scipix)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.77+-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/ruvnet/ruvector/workflows/CI/badge.svg)](https://github.com/ruvnet/ruvector/actions)

<p align="center">
  <strong>🔬 Production-ready Rust OCR library for extracting LaTeX, MathML, and text from scientific images</strong>
</p>

<p align="center">
  <em>Convert mathematical equations, scientific papers, and technical diagrams to structured text with GPU-accelerated inference</em>
</p>

<p align="center">
  <a href="#installation">Installation</a> |
  <a href="#quick-start">Quick Start</a> |
  <a href="#sdk-usage">SDK Usage</a> |
  <a href="#cli-reference">CLI Reference</a> |
  <a href="#tutorials">Tutorials</a> |
  <a href="#api-reference">API Reference</a>
</p>

---

## Why SciPix?

**SciPix** is a blazing-fast, memory-safe OCR (Optical Character Recognition) engine written in pure Rust. Unlike traditional OCR tools, SciPix is purpose-built for **scientific documents**, **mathematical equations**, and **technical diagrams** — making it the ideal choice for researchers, academics, and developers working with STEM content.

### Use Cases

- 📄 **Academic Paper Digitization** - Extract text and equations from scanned research papers
- 🧮 **Math Homework Assistance** - Convert handwritten equations to LaTeX for AI tutoring apps
- 📊 **Technical Documentation** - Process engineering diagrams and scientific charts
- 🔬 **Research Data Extraction** - Batch process journal articles and extract structured data
- 🤖 **AI/LLM Integration** - Feed scientific content to language models via MCP protocol

### Key Features

| Feature | Description |
|---------|-------------|
| 🚀 **ONNX Runtime** | GPU-accelerated neural network inference with CUDA, TensorRT, and CoreML support |
| 📐 **LaTeX Output** | Accurate mathematical equation recognition with LaTeX, MathML, and AsciiMath export |
| ⚡ **SIMD Optimized** | 4x faster image preprocessing with AVX2, SSE4, and NEON vectorization |
| 🌐 **REST API** | Production-ready HTTP server with rate limiting, caching, and authentication |
| 💻 **CLI Tool** | Batch processing, PDF conversion, and watch mode for continuous OCR |
| 🦀 **Pure Rust SDK** | Type-safe, async/await native library with zero-copy image processing |
| 🔌 **WebAssembly** | Run OCR directly in browsers with full WASM support |
| 🤖 **MCP Server** | Integrate with Claude, ChatGPT, and other AI assistants via Model Context Protocol |
| 📦 **Cross-Platform** | Linux, macOS, Windows, and ARM64 support out of the box |

### Performance Benchmarks

| Operation | SciPix | Tesseract | Mathpix |
|-----------|--------|-----------|---------|
| Simple Text OCR | **50ms** | 120ms | 200ms* |
| Math Equation | **80ms** | N/A | 150ms* |
| Batch (100 images) | **2.1s** | 8.5s | N/A |
| Memory Usage | **45MB** | 180MB | Cloud |

*API latency, not processing time

---

## Installation

### From crates.io (Rust SDK)

```bash
cargo add ruvector-scipix
```

Or add to your `Cargo.toml`:

```toml
[dependencies]
ruvector-scipix = "0.1.16"

# With specific features
ruvector-scipix = { version = "0.1.16", features = ["ocr", "math", "optimize"] }
```

### From Source (CLI & Server)

```bash
# Clone the repository
git clone https://github.com/ruvnet/ruvector.git
cd ruvector/examples/scipix

# Build CLI and Server
cargo build --release

# Install globally (optional)
cargo install --path .
```

### Pre-built Binaries

```bash
# Download latest release (Linux)
curl -L https://github.com/ruvnet/ruvector/releases/latest/download/scipix-cli-linux-x64 -o scipix-cli
chmod +x scipix-cli

# Download latest release (macOS)
curl -L https://github.com/ruvnet/ruvector/releases/latest/download/scipix-cli-darwin-arm64 -o scipix-cli
chmod +x scipix-cli
```

### Feature Flags

| Flag | Description | Default |
|------|-------------|---------|
| `default` | preprocess, cache, optimize | ✅ |
| `ocr` | ONNX-based OCR engine | ❌ |
| `math` | Math expression parsing | ❌ |
| `preprocess` | Image preprocessing | ✅ |
| `cache` | Result caching | ✅ |
| `optimize` | SIMD & parallel optimizations | ✅ |
| `wasm` | WebAssembly support | ❌ |

---

## Quick Start

### 30-Second Setup

```bash
# Build and run the server
cd examples/scipix
cargo run --release --bin scipix-server

# In another terminal, test the API
curl http://localhost:3000/health
# {"status":"healthy","version":"0.1.16"}
```

### Process Your First Image

```bash
# Encode an image to base64
BASE64_IMAGE=$(base64 -w 0 equation.png)

# Send OCR request
curl -X POST http://localhost:3000/v3/text \
  -H "Content-Type: application/json" \
  -H "app_id: demo" \
  -H "app_key: demo_key" \
  -d "{\"base64\": \"$BASE64_IMAGE\", \"metadata\": {\"formats\": [\"text\", \"latex\"]}}"
```

---

## SDK Usage

### Basic Usage

```rust
use ruvector_scipix::{Config, Result};

fn main() -> Result<()> {
    // Load default configuration
    let config = Config::default();

    // Validate configuration
    config.validate()?;

    println!("SciPix version: {}", ruvector_scipix::VERSION);
    Ok(())
}
```

### Image Preprocessing

```rust
use ruvector_scipix::preprocess::{PreprocessPipeline, transforms};
use image::open;

fn preprocess_image(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Load image
    let img = open(path)?;

    // Create preprocessing pipeline
    let pipeline = PreprocessPipeline::new()
        .with_auto_rotate(true)
        .with_auto_deskew(true)
        .with_noise_reduction(true)
        .with_contrast_enhancement(true);

    // Process image
    let processed = pipeline.process(img)?;

    // Save result
    processed.save("processed.png")?;

    Ok(())
}
```

### OCR Engine (requires `ocr` feature)

```rust
use ruvector_scipix::ocr::{OcrEngine, OcrOptions};
use ruvector_scipix::OcrConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize OCR engine
    let config = OcrConfig::default();
    let engine = OcrEngine::new(config).await?;

    // Load and process image
    let image = image::open("equation.png")?;
    let result = engine.recognize(&image).await?;

    println!("Text: {}", result.text);
    println!("Confidence: {:.2}%", result.confidence * 100.0);

    // Get LaTeX output
    if let Some(latex) = result.latex {
        println!("LaTeX: {}", latex);
    }

    Ok(())
}
```

### Math Parsing (requires `math` feature)

```rust
use ruvector_scipix::math::{parse_expression, to_latex, to_mathml};

fn parse_math() -> Result<(), Box<dyn std::error::Error>> {
    // Parse a mathematical expression
    let expr = parse_expression("x^2 + 2x + 1")?;

    // Convert to different formats
    let latex = to_latex(&expr)?;
    let mathml = to_mathml(&expr)?;

    println!("LaTeX: {}", latex);
    println!("MathML: {}", mathml);

    Ok(())
}
```

### Caching Results

```rust
use ruvector_scipix::cache::CacheManager;
use ruvector_scipix::CacheConfig;

fn use_cache() -> Result<(), Box<dyn std::error::Error>> {
    let config = CacheConfig {
        max_size: 1000,
        ttl_seconds: 3600,
        ..Default::default()
    };

    let cache = CacheManager::new(config)?;

    // Store result
    cache.store("image_hash_123", &result)?;

    // Retrieve result
    if let Some(cached) = cache.get("image_hash_123")? {
        println!("Cache hit: {}", cached.latex);
    }

    Ok(())
}
```

### Configuration Presets

```rust
use ruvector_scipix::{default_config, high_accuracy_config, high_speed_config};

fn configure() {
    // Default balanced configuration
    let config = default_config();

    // High accuracy (slower, more precise)
    let accurate = high_accuracy_config();

    // High speed (faster, may sacrifice accuracy)
    let fast = high_speed_config();
}
```

---

## CLI Reference

### Installation

```bash
# Install from source
cargo install --path examples/scipix

# Or use pre-built binary
./scipix-cli --help
```

### Commands

#### `ocr` - Process Single Image

```bash
# Basic OCR
scipix-cli ocr --input document.png

# With output file and format
scipix-cli ocr --input equation.png --output result.json --format latex

# Specify output formats
scipix-cli ocr --input image.png --formats text,latex,mathml
```

**Options:**
| Flag | Description | Default |
|------|-------------|---------|
| `-i, --input` | Input image path | Required |
| `-o, --output` | Output file path | stdout |
| `-f, --format` | Output format (json, text, latex) | json |
| `--formats` | OCR formats (text, latex, mathml, html) | text |
| `--confidence` | Minimum confidence threshold | 0.5 |

#### `batch` - Process Multiple Images

```bash
# Process directory
scipix-cli batch --input-dir ./images --output-dir ./results

# With parallel processing
scipix-cli batch -i ./images -o ./results --parallel 8

# Recursive with specific formats
scipix-cli batch -i ./docs -o ./output --recursive --format latex

# Watch mode for continuous processing
scipix-cli batch -i ./inbox -o ./processed --watch
```

**Options:**
| Flag | Description | Default |
|------|-------------|---------|
| `-i, --input-dir` | Input directory | Required |
| `-o, --output-dir` | Output directory | Required |
| `-p, --parallel` | Parallel workers | CPU cores |
| `-r, --recursive` | Process subdirectories | false |
| `--watch` | Watch for new files | false |
| `--max-retries` | Retry failed files | 3 |

#### `serve` - Start API Server

```bash
# Start with defaults
scipix-cli serve

# Custom address and port
scipix-cli serve --address 0.0.0.0 --port 8080

# With configuration file
scipix-cli serve --config ./config.toml

# Enable debug logging
RUST_LOG=debug scipix-cli serve
```

**Options:**
| Flag | Description | Default |
|------|-------------|---------|
| `-a, --address` | Bind address | 127.0.0.1 |
| `-p, --port` | Port number | 3000 |
| `-c, --config` | Config file path | None |
| `--workers` | Worker threads | CPU cores |

#### `config` - Manage Configuration

```bash
# Show current configuration
scipix-cli config show

# Initialize default config file
scipix-cli config init

# Set specific values
scipix-cli config set ocr.confidence_threshold 0.8
scipix-cli config set server.port 8080

# Validate configuration
scipix-cli config validate
```

#### `doctor` - Environment Check

```bash
# Run full diagnostics
scipix-cli doctor

# Check specific components
scipix-cli doctor --check cpu,memory,deps

# Output as JSON
scipix-cli doctor --format json

# Auto-fix issues
scipix-cli doctor --fix
```

**Checks performed:**
- CPU cores and SIMD capabilities (SSE2, AVX, AVX2, AVX-512, NEON)
- Memory availability
- ONNX Runtime installation
- Model file availability
- Configuration validity
- Network port availability

#### `mcp` - MCP Server Mode

```bash
# Start MCP server for AI integration
scipix-cli mcp

# With debug logging
scipix-cli mcp --debug

# With custom models directory
scipix-cli mcp --models-dir ./custom-models
```

**Available MCP Tools:**
| Tool | Description |
|------|-------------|
| `ocr_image` | Process image file with OCR |
| `ocr_base64` | Process base64-encoded image |
| `batch_ocr` | Batch process multiple images |
| `preprocess_image` | Apply image preprocessing |
| `latex_to_mathml` | Convert LaTeX to MathML |
| `benchmark_performance` | Run performance benchmarks |

**Claude Code Integration:**
```bash
claude mcp add scipix -- scipix-cli mcp
```

---

## Tutorials

### Tutorial 1: Basic Image OCR

Learn to extract text from images using the REST API.

```bash
# Step 1: Start the server
cargo run --bin scipix-server

# Step 2: Encode your image
BASE64=$(base64 -w 0 document.png)

# Step 3: Send OCR request
curl -X POST http://localhost:3000/v3/text \
  -H "Content-Type: application/json" \
  -H "app_id: test" \
  -H "app_key: test123" \
  -d "{\"base64\": \"$BASE64\", \"metadata\": {\"formats\": [\"text\"]}}"
```

### Tutorial 2: Mathematical Equation Recognition

Convert math images to LaTeX format.

```bash
curl -X POST http://localhost:3000/v3/text \
  -H "Content-Type: application/json" \
  -H "app_id: test" \
  -H "app_key: test123" \
  -d '{
    "url": "https://example.com/equation.png",
    "metadata": {
      "formats": ["latex", "mathml"],
      "math_mode": true
    }
  }'
```

**Response:**
```json
{
  "latex": "\\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}",
  "mathml": "<math>...</math>",
  "confidence": 0.92
}
```

### Tutorial 3: Batch PDF Processing

Process multi-page PDFs asynchronously.

```bash
# Submit PDF job
JOB=$(curl -s -X POST http://localhost:3000/v3/pdf \
  -H "Content-Type: application/json" \
  -H "app_id: test" \
  -H "app_key: test123" \
  -d '{
    "url": "https://example.com/paper.pdf",
    "options": {"format": "mmd", "enable_ocr": true}
  }')

JOB_ID=$(echo $JOB | jq -r '.pdf_id')

# Poll for completion
curl http://localhost:3000/v3/pdf/$JOB_ID \
  -H "app_id: test" -H "app_key: test123"
```

### Tutorial 4: CLI Batch Processing

```bash
# Process entire directory
scipix-cli batch \
  --input-dir ./documents \
  --output-dir ./results \
  --format latex \
  --parallel 4 \
  --recursive

# Watch mode for continuous processing
scipix-cli batch \
  --input-dir ./inbox \
  --output-dir ./processed \
  --watch
```

### Tutorial 5: WebAssembly Integration

```bash
# Build WASM module
cargo install wasm-pack
wasm-pack build --target web --features wasm
```

```html
<script type="module">
  import init, { ScipixWasm } from './pkg/ruvector_scipix.js';

  async function processImage() {
    await init();
    const scipix = new ScipixWasm();
    await scipix.initialize();

    const canvas = document.getElementById('canvas');
    const imageData = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height);
    const result = await scipix.recognize(imageData.data);
    console.log('Result:', result);
  }

  processImage();
</script>
```

### Tutorial 6: Using as MCP Server

Integrate SciPix with Claude Code or other AI assistants.

```bash
# Add to Claude Code
claude mcp add scipix -- scipix-cli mcp

# Or run standalone
scipix-cli mcp --debug
```

Then use tools in your AI conversations:
- "Use the ocr_image tool to extract text from ./screenshot.png"
- "Convert this LaTeX to MathML: \\frac{1}{2}"

---

## API Reference

### Authentication

All API endpoints (except `/health`) require authentication:

```
app_id: your_application_id
app_key: your_secret_key
```

### Endpoints

#### `POST /v3/text` - Image OCR

```json
{
  "base64": "...",
  "url": "https://...",
  "metadata": {
    "formats": ["text", "latex", "mathml"],
    "confidence_threshold": 0.5,
    "math_mode": false
  }
}
```

#### `POST /v3/strokes` - Digital Ink

```json
{
  "strokes": [{"x": [0, 10, 20], "y": [0, 10, 0]}],
  "metadata": {"formats": ["latex"]}
}
```

#### `POST /v3/pdf` - PDF Processing

```json
{
  "url": "https://example.com/doc.pdf",
  "options": {
    "format": "mmd",
    "enable_ocr": true,
    "page_range": "1-10"
  }
}
```

#### `GET /health` - Health Check

```json
{"status": "healthy", "version": "0.1.16"}
```

---

## Configuration

### Environment Variables

```bash
SERVER_ADDR=127.0.0.1:3000
RUST_LOG=scipix=info
RATE_LIMIT_PER_MINUTE=100
CACHE_MAX_SIZE=1000
MODEL_PATH=./models
```

### Configuration File

```toml
[server]
address = "127.0.0.1"
port = 3000
workers = 4

[ocr]
model_path = "./models"
confidence_threshold = 0.5

[cache]
max_size = 1000
ttl_seconds = 3600

[rate_limit]
requests_per_minute = 100
burst_size = 20
```

---

## Performance

| Operation | Time (avg) | Throughput |
|-----------|------------|------------|
| SIMD Grayscale | 101µs | 4.2x faster |
| SIMD Resize | 2.63ms | 1.5x faster |
| Full Pipeline | 0.49ms | 4.4x faster |
| Simple text OCR | ~50ms | 20 img/s |
| Math equation | ~80ms | 12 img/s |

---

## Troubleshooting

```bash
# Check environment
scipix-cli doctor

# Enable debug logging
RUST_LOG=debug scipix-cli serve

# Verify models installed
ls -la models/
```

---

## Contributing

```bash
# Run tests
cargo test --all-features

# Run linting
cargo clippy --all-features

# Format code
cargo fmt
```

---

## License

MIT License - see [LICENSE](../../LICENSE) for details.

---

<p align="center">
  Part of the <a href="https://github.com/ruvnet/ruvector">ruvector</a> ecosystem<br>
  Built with Rust 🦀 | Powered by ONNX Runtime
</p>
