# Ruvector-Scipix Architecture

## Document Information
- **Version**: 1.0.0
- **Status**: Draft
- **Date**: 2025-11-28
- **SPARC Phase**: Architecture

## Executive Summary

Ruvector-scipix is a high-performance Rust-based OCR system specialized for mathematical content extraction, inspired by Scipix. The architecture leverages ruvector's vector database capabilities for intelligent caching, semantic search of mathematical structures, and integration with agentic workflows through lean-agentic orchestration.

## 1. High-Level System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         CLIENT LAYER                                 │
├─────────────┬──────────────┬──────────────┬─────────────────────────┤
│   CLI Tool  │  REST API    │  WASM Web    │  Lean-Agentic Agents   │
│  (ruvector- │  (ruvector-  │  (ruvector-  │  (Agent Integration)   │
│  scipix-   │  scipix-    │  scipix-    │                         │
│  cli)       │  api)        │  wasm)       │                         │
└─────────────┴──────────────┴──────────────┴─────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     CORE PROCESSING LAYER                            │
│                   (ruvector-scipix-core)                            │
├──────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │   Image      │  │   OCR        │  │   Math Structure         │  │
│  │   Processor  │→ │   Engine     │→ │   Parser & Recognizer    │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
│         │                 │                      │                   │
│         └─────────────────┼──────────────────────┘                   │
│                           ▼                                          │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │              Output Formatter & Converter                     │  │
│  │        (LaTeX, MathML, MMD, ASCII, HTML, PNG)                │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     MODEL MANAGEMENT LAYER                           │
│                   (ruvector-scipix-models)                          │
├──────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │   Model      │  │   Model      │  │   Model                  │  │
│  │   Loader     │  │   Cache      │  │   Validator              │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
│         │                 │                      │                   │
│         └─────────────────┴──────────────────────┘                   │
│                           ▼                                          │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │        ONNX Runtime / TensorFlow Lite Integration            │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     DATA & CACHE LAYER                               │
│                   (ruvector-core integration)                        │
├──────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │   Vector     │  │   Embedding  │  │   Result                 │  │
│  │   Cache      │  │   Generator  │  │   Cache                  │  │
│  │   (HNSW)     │  │              │  │   (Key-Value)            │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
│         │                 │                      │                   │
│         └─────────────────┴──────────────────────┘                   │
│                           ▼                                          │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │              Storage Backend (redb + memmap2)                 │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                  INFRASTRUCTURE LAYER                                │
├──────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │   Metrics &  │  │   Tracing &  │  │   Configuration          │  │
│  │   Monitoring │  │   Logging    │  │   Management             │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

## 2. Crate Structure

### 2.1 Workspace Organization

```
ruvector/
├── crates/
│   └── (existing ruvector crates)
└── examples/
    └── scipix/                           # Main scipix directory
        ├── Cargo.toml                     # Workspace definition
        ├── README.md
        ├── LICENSE
        ├── src/
        │   └── lib.rs                     # Re-exports all crates
        ├── crates/
        │   ├── ruvector-scipix-core/     # Core OCR engine
        │   ├── ruvector-scipix-models/   # Model management
        │   ├── ruvector-scipix-api/      # REST API server
        │   ├── ruvector-scipix-cli/      # Command-line interface
        │   └── ruvector-scipix-wasm/     # WebAssembly bindings
        ├── docs/                          # Architecture docs
        ├── models/                        # Pre-trained models
        ├── tests/                         # Integration tests
        ├── benchmarks/                    # Performance benchmarks
        └── config/                        # Configuration files
```

### 2.2 Crate Dependency Graph

```
                    ┌──────────────────────┐
                    │  ruvector-core       │
                    │  (vector database)   │
                    └──────────┬───────────┘
                               │
                ┌──────────────┴──────────────┐
                │                             │
    ┌───────────▼───────────┐    ┌───────────▼────────────┐
    │ ruvector-scipix-     │    │  ruvector-scipix-     │
    │ models                │    │  core                  │
    │ (Model Management)    │    │  (OCR Engine)          │
    └───────────┬───────────┘    └───────────┬────────────┘
                │                            │
                └──────────────┬─────────────┘
                               │
          ┌────────────────────┼────────────────────┐
          │                    │                    │
┌─────────▼──────────┐  ┌──────▼────────┐  ┌───────▼────────────┐
│ ruvector-scipix-  │  │ ruvector-     │  │ ruvector-scipix-  │
│ api                │  │ scipix-cli   │  │ wasm               │
│ (REST Server)      │  │ (CLI Tool)    │  │ (Web Interface)    │
└────────────────────┘  └───────────────┘  └────────────────────┘
```

### 2.3 Crate Specifications

#### 2.3.1 ruvector-scipix-core

**Purpose**: Core OCR engine and mathematical structure recognition

```toml
[package]
name = "ruvector-scipix-core"
version = "0.1.0"
edition = "2021"

[dependencies]
# Core ruvector
ruvector-core = { path = "../../../crates/ruvector-core" }

# Image processing
image = "0.25"
imageproc = "0.25"
fast_image_resize = "4.0"

# ML/OCR
tract-onnx = "0.21"       # ONNX runtime
ndarray = "0.16"

# Math parsing
pest = "2.7"              # PEG parser for LaTeX
pest_derive = "2.7"

# Text/OCR
tesseract-sys = { version = "0.6", optional = true }

# Async & concurrency
rayon = "1.10"
tokio = { version = "1.41", features = ["full"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rkyv = "0.8"

# Error handling
thiserror = "2.0"
anyhow = "1.0"

# Utilities
tracing = "0.1"
uuid = { version = "1.11", features = ["v4", "serde"] }
chrono = "0.4"
dashmap = "6.1"
parking_lot = "0.12"
once_cell = "1.20"

[features]
default = ["tesseract", "gpu"]
tesseract = ["tesseract-sys"]
gpu = []
```

**Module Structure**:

```
ruvector-scipix-core/
├── src/
│   ├── lib.rs                      # Public API
│   ├── error.rs                    # Error types
│   ├── config.rs                   # Configuration
│   ├── types.rs                    # Common types
│   │
│   ├── preprocess/
│   │   ├── mod.rs                  # Image preprocessing
│   │   ├── normalize.rs            # Normalization
│   │   ├── denoise.rs              # Noise removal
│   │   ├── deskew.rs               # Rotation correction
│   │   ├── enhance.rs              # Contrast enhancement
│   │   └── segment.rs              # Region segmentation
│   │
│   ├── ocr/
│   │   ├── mod.rs                  # OCR engine
│   │   ├── detector.rs             # Text detection
│   │   ├── recognizer.rs           # Character recognition
│   │   ├── tesseract.rs            # Tesseract integration
│   │   └── neural.rs               # Neural OCR models
│   │
│   ├── math/
│   │   ├── mod.rs                  # Math parsing
│   │   ├── parser.rs               # Structure parser
│   │   ├── recognizer.rs           # Symbol recognition
│   │   ├── layout.rs               # Layout analysis
│   │   ├── grammar.pest            # LaTeX grammar
│   │   └── symbols.rs              # Math symbol mapping
│   │
│   ├── output/
│   │   ├── mod.rs                  # Output formatting
│   │   ├── latex.rs                # LaTeX generation
│   │   ├── mathml.rs               # MathML generation
│   │   ├── mmd.rs                  # MMD generation
│   │   ├── ascii.rs                # ASCII art
│   │   ├── html.rs                 # HTML generation
│   │   └── renderer.rs             # PNG/SVG rendering
│   │
│   ├── pipeline/
│   │   ├── mod.rs                  # Processing pipeline
│   │   ├── executor.rs             # Pipeline execution
│   │   ├── stages.rs               # Pipeline stages
│   │   └── parallel.rs             # Parallel processing
│   │
│   └── cache/
│       ├── mod.rs                  # Caching layer
│       ├── vector_cache.rs         # Vector-based cache
│       ├── result_cache.rs         # Result cache
│       └── embedding.rs            # Embedding generation
│
├── benches/
│   ├── pipeline_bench.rs
│   ├── ocr_bench.rs
│   └── parser_bench.rs
│
└── tests/
    ├── integration_test.rs
    ├── ocr_test.rs
    └── parser_test.rs
```

#### 2.3.2 ruvector-scipix-models

**Purpose**: ML model management, loading, and caching

```toml
[package]
name = "ruvector-scipix-models"
version = "0.1.0"
edition = "2021"

[dependencies]
ruvector-core = { path = "../../../crates/ruvector-core" }
ruvector-scipix-core = { path = "../ruvector-scipix-core" }

# ML frameworks
tract-onnx = "0.21"
ndarray = "0.16"

# Model management
reqwest = { version = "0.12", features = ["blocking", "json"] }
sha2 = "0.10"
tar = "0.4"
flate2 = "1.0"

# Async
tokio = { version = "1.41", features = ["full"] }
futures = "0.3"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# Error handling
thiserror = "2.0"
anyhow = "1.0"

# Utilities
tracing = "0.1"
dashmap = "6.1"
parking_lot = "0.12"
once_cell = "1.20"

[features]
default = ["download", "validate"]
download = []
validate = []
```

**Module Structure**:

```
ruvector-scipix-models/
├── src/
│   ├── lib.rs                      # Public API
│   ├── error.rs                    # Error types
│   ├── types.rs                    # Model types
│   │
│   ├── registry/
│   │   ├── mod.rs                  # Model registry
│   │   ├── catalog.rs              # Model catalog
│   │   └── metadata.rs             # Model metadata
│   │
│   ├── loader/
│   │   ├── mod.rs                  # Model loader
│   │   ├── onnx.rs                 # ONNX loader
│   │   ├── download.rs             # Model downloader
│   │   └── validator.rs            # Model validator
│   │
│   ├── cache/
│   │   ├── mod.rs                  # Model cache
│   │   ├── memory.rs               # In-memory cache
│   │   └── disk.rs                 # Disk cache
│   │
│   └── inference/
│       ├── mod.rs                  # Inference engine
│       ├── session.rs              # Inference session
│       ├── batch.rs                # Batch inference
│       └── pool.rs                 # Model pool
│
└── tests/
    ├── loader_test.rs
    └── cache_test.rs
```

#### 2.3.3 ruvector-scipix-api

**Purpose**: REST API server with WebSocket support

```toml
[package]
name = "ruvector-scipix-api"
version = "0.1.0"
edition = "2021"

[dependencies]
ruvector-core = { path = "../../../crates/ruvector-core" }
ruvector-scipix-core = { path = "../ruvector-scipix-core" }
ruvector-scipix-models = { path = "../ruvector-scipix-models" }

# Web framework
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace", "compression"] }

# WebSocket
tokio-tungstenite = "0.23"

# Async runtime
tokio = { version = "1.41", features = ["full"] }
futures = "0.3"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
thiserror = "2.0"
anyhow = "1.0"

# Tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Auth & Security
jsonwebtoken = "9.3"
argon2 = "0.5"

# Rate limiting
governor = "0.6"

# Utilities
uuid = { version = "1.11", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }

[features]
default = ["auth", "rate-limit"]
auth = []
rate-limit = []
```

**Module Structure**:

```
ruvector-scipix-api/
├── src/
│   ├── main.rs                     # Server entry point
│   ├── lib.rs                      # Library exports
│   ├── config.rs                   # Server configuration
│   ├── error.rs                    # Error handling
│   │
│   ├── routes/
│   │   ├── mod.rs                  # Route definitions
│   │   ├── ocr.rs                  # OCR endpoints
│   │   ├── batch.rs                # Batch processing
│   │   ├── status.rs               # Status endpoints
│   │   └── health.rs               # Health checks
│   │
│   ├── handlers/
│   │   ├── mod.rs                  # Request handlers
│   │   ├── ocr_handler.rs          # OCR handler
│   │   ├── upload_handler.rs       # Upload handler
│   │   └── stream_handler.rs       # WebSocket handler
│   │
│   ├── middleware/
│   │   ├── mod.rs                  # Middleware
│   │   ├── auth.rs                 # Authentication
│   │   ├── rate_limit.rs           # Rate limiting
│   │   ├── logging.rs              # Request logging
│   │   └── metrics.rs              # Metrics collection
│   │
│   ├── state/
│   │   ├── mod.rs                  # Application state
│   │   └── app_state.rs            # Shared state
│   │
│   └── ws/
│       ├── mod.rs                  # WebSocket support
│       ├── connection.rs           # WS connection
│       └── protocol.rs             # WS protocol
│
└── tests/
    ├── api_test.rs
    └── integration_test.rs
```

#### 2.3.4 ruvector-scipix-cli

**Purpose**: Command-line interface tool

```toml
[package]
name = "ruvector-scipix-cli"
version = "0.1.0"
edition = "2021"

[dependencies]
ruvector-scipix-core = { path = "../ruvector-scipix-core" }
ruvector-scipix-models = { path = "../ruvector-scipix-models" }

# CLI
clap = { version = "4.5", features = ["derive", "cargo"] }
indicatif = "0.17"
console = "0.15"
dialoguer = "0.11"

# Async runtime
tokio = { version = "1.41", features = ["full"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# Error handling
thiserror = "2.0"
anyhow = "1.0"

# Utilities
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
colored = "2.1"

[features]
default = []
```

**Module Structure**:

```
ruvector-scipix-cli/
├── src/
│   ├── main.rs                     # CLI entry point
│   ├── cli.rs                      # CLI argument parsing
│   ├── config.rs                   # CLI configuration
│   ├── error.rs                    # Error handling
│   │
│   ├── commands/
│   │   ├── mod.rs                  # Command definitions
│   │   ├── ocr.rs                  # OCR command
│   │   ├── batch.rs                # Batch processing
│   │   ├── convert.rs              # Format conversion
│   │   ├── models.rs               # Model management
│   │   └── config.rs               # Configuration
│   │
│   ├── output/
│   │   ├── mod.rs                  # Output formatting
│   │   ├── table.rs                # Table output
│   │   ├── json.rs                 # JSON output
│   │   └── text.rs                 # Text output
│   │
│   └── utils/
│       ├── mod.rs                  # Utilities
│       ├── progress.rs             # Progress bars
│       └── interactive.rs          # Interactive mode
│
└── tests/
    └── cli_test.rs
```

#### 2.3.5 ruvector-scipix-wasm

**Purpose**: WebAssembly bindings for browser integration

```toml
[package]
name = "ruvector-scipix-wasm"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
ruvector-scipix-core = { path = "../ruvector-scipix-core", default-features = false }
ruvector-scipix-models = { path = "../ruvector-scipix-models" }

# WASM
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"
web-sys = { version = "0.3", features = [
    "Document",
    "Element",
    "HtmlCanvasElement",
    "ImageData",
    "CanvasRenderingContext2d",
    "File",
    "FileReader",
    "Blob",
] }

# Async
futures = "0.3"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde-wasm-bindgen = "0.6"

# Error handling
thiserror = "2.0"

# Utilities
console_error_panic_hook = "0.1"
tracing-wasm = "0.2"

[features]
default = []
```

**Module Structure**:

```
ruvector-scipix-wasm/
├── src/
│   ├── lib.rs                      # WASM exports
│   ├── error.rs                    # Error types
│   ├── types.rs                    # JS-compatible types
│   │
│   ├── api/
│   │   ├── mod.rs                  # WASM API
│   │   ├── ocr.rs                  # OCR functions
│   │   ├── convert.rs              # Conversion functions
│   │   └── utils.rs                # Utility functions
│   │
│   ├── worker/
│   │   ├── mod.rs                  # Web Worker support
│   │   └── pool.rs                 # Worker pool
│   │
│   └── canvas/
│       ├── mod.rs                  # Canvas integration
│       └── renderer.rs             # Canvas rendering
│
├── js/
│   ├── index.js                    # JS wrapper
│   └── worker.js                   # Web Worker
│
└── tests/
    └── wasm_test.rs
```

## 3. Component Breakdown

### 3.1 Image Preprocessor

**Responsibility**: Transform raw images into clean, standardized format for OCR

```rust
pub struct ImagePreprocessor {
    config: PreprocessConfig,
    cache: Arc<RwLock<LruCache<u64, ProcessedImage>>>,
}

pub struct PreprocessConfig {
    pub target_dpi: u32,
    pub max_dimension: u32,
    pub denoise_strength: f32,
    pub contrast_enhancement: bool,
    pub auto_rotate: bool,
    pub binarization_method: BinarizationMethod,
}

pub enum BinarizationMethod {
    Otsu,
    Adaptive,
    Sauvola,
}

impl ImagePreprocessor {
    /// Main preprocessing pipeline
    pub async fn preprocess(&self, image: DynamicImage) -> Result<ProcessedImage> {
        let steps = vec![
            Self::normalize_dimensions,
            Self::denoise,
            Self::enhance_contrast,
            Self::detect_and_correct_skew,
            Self::binarize,
            Self::segment_regions,
        ];

        // Execute pipeline stages
        let mut result = image;
        for step in steps {
            result = step(result, &self.config).await?;
        }

        Ok(ProcessedImage::new(result))
    }

    /// Parallel batch preprocessing
    pub async fn preprocess_batch(&self, images: Vec<DynamicImage>) -> Result<Vec<ProcessedImage>> {
        images.par_iter()
            .map(|img| self.preprocess(img.clone()))
            .collect()
    }
}
```

**Key Algorithms**:
- **Deskewing**: Hough transform + rotation detection
- **Denoising**: Bilateral filter, non-local means
- **Binarization**: Otsu's method, adaptive thresholding
- **Segmentation**: Connected component analysis

### 3.2 OCR Engine

**Responsibility**: Extract text from preprocessed images

```rust
pub struct OcrEngine {
    detector: TextDetector,
    recognizer: CharacterRecognizer,
    model_pool: Arc<ModelPool>,
    cache: Arc<VectorCache>,
}

pub struct TextDetector {
    model: Arc<OnnxModel>,
    confidence_threshold: f32,
}

pub struct CharacterRecognizer {
    model: Arc<OnnxModel>,
    language: Language,
    math_mode: bool,
}

impl OcrEngine {
    /// Detect text regions in image
    pub async fn detect_text(&self, image: &ProcessedImage) -> Result<Vec<TextRegion>> {
        // Run EAST/CRAFT text detection model
        let detections = self.detector.detect(image).await?;

        // Filter by confidence and merge overlapping regions
        let regions = self.post_process_detections(detections)?;

        Ok(regions)
    }

    /// Recognize text in detected regions
    pub async fn recognize_text(&self, image: &ProcessedImage, regions: &[TextRegion])
        -> Result<Vec<TextResult>> {
        // Parallel recognition
        regions.par_iter()
            .map(|region| {
                let cropped = image.crop(region.bbox);
                self.recognizer.recognize(&cropped)
            })
            .collect()
    }

    /// Full OCR pipeline
    pub async fn ocr(&self, image: &ProcessedImage) -> Result<OcrResult> {
        // Check cache first
        let cache_key = image.hash();
        if let Some(cached) = self.cache.get(&cache_key).await? {
            return Ok(cached);
        }

        // Run detection and recognition
        let regions = self.detect_text(image).await?;
        let text_results = self.recognize_text(image, &regions).await?;

        // Assemble result
        let result = OcrResult {
            text_results,
            regions,
            confidence: self.calculate_confidence(&text_results),
        };

        // Cache result
        self.cache.put(cache_key, &result).await?;

        Ok(result)
    }
}
```

**Model Architecture**:
- **Text Detection**: EAST (Efficient and Accurate Scene Text) or CRAFT
- **Text Recognition**: CRNN (Convolutional Recurrent Neural Network)
- **Math Symbols**: Custom CNN trained on math symbol datasets

### 3.3 Math Parser

**Responsibility**: Parse OCR'd text into mathematical structure and generate LaTeX

```rust
pub struct MathParser {
    grammar: PestParser,
    symbol_recognizer: SymbolRecognizer,
    layout_analyzer: LayoutAnalyzer,
}

pub struct SymbolRecognizer {
    symbol_map: HashMap<String, MathSymbol>,
    model: Arc<OnnxModel>,
}

pub struct LayoutAnalyzer {
    spatial_threshold: f32,
    baseline_detector: BaselineDetector,
}

impl MathParser {
    /// Parse mathematical expression
    pub fn parse(&self, ocr_result: &OcrResult) -> Result<MathExpression> {
        // Analyze spatial layout
        let layout = self.layout_analyzer.analyze(&ocr_result.regions)?;

        // Recognize math symbols
        let symbols = self.symbol_recognizer.recognize_batch(&ocr_result.text_results)?;

        // Build expression tree
        let expr = self.build_expression_tree(symbols, layout)?;

        Ok(expr)
    }

    /// Generate LaTeX from expression
    pub fn to_latex(&self, expr: &MathExpression) -> Result<String> {
        let mut latex = String::new();
        self.traverse_and_generate(expr, &mut latex)?;
        Ok(latex)
    }

    /// Detect structure types
    fn detect_structure(&self, regions: &[TextRegion]) -> MathStructure {
        // Detect fractions, exponents, subscripts, matrices, etc.
        if self.is_fraction(regions) {
            MathStructure::Fraction
        } else if self.is_exponent(regions) {
            MathStructure::Exponent
        } else if self.is_matrix(regions) {
            MathStructure::Matrix
        } else {
            MathStructure::Linear
        }
    }
}

#[derive(Debug, Clone)]
pub enum MathStructure {
    Linear,
    Fraction,
    Exponent,
    Subscript,
    Matrix,
    Integral,
    Sum,
    Root,
}

#[derive(Debug, Clone)]
pub struct MathExpression {
    pub structure: MathStructure,
    pub children: Vec<MathExpression>,
    pub symbol: Option<MathSymbol>,
    pub bbox: BoundingBox,
}
```

**Parsing Strategy**:
1. **Spatial Analysis**: Analyze relative positions (baseline, superscript, subscript)
2. **Symbol Recognition**: Match OCR text to math symbols
3. **Structure Detection**: Identify fractions, matrices, integrals
4. **Tree Building**: Construct expression tree
5. **LaTeX Generation**: Traverse tree and generate LaTeX

### 3.4 Output Formatter

**Responsibility**: Convert parsed expressions to various output formats

```rust
pub struct OutputFormatter {
    latex_generator: LatexGenerator,
    mathml_generator: MathMLGenerator,
    mmd_generator: MmdGenerator,
    ascii_generator: AsciiGenerator,
    html_generator: HtmlGenerator,
    renderer: MathRenderer,
}

impl OutputFormatter {
    /// Convert to LaTeX
    pub fn to_latex(&self, expr: &MathExpression) -> Result<String> {
        self.latex_generator.generate(expr)
    }

    /// Convert to MathML
    pub fn to_mathml(&self, expr: &MathExpression) -> Result<String> {
        self.mathml_generator.generate(expr)
    }

    /// Convert to MMD (Markdown Math Display)
    pub fn to_mmd(&self, expr: &MathExpression) -> Result<String> {
        self.mmd_generator.generate(expr)
    }

    /// Convert to ASCII art
    pub fn to_ascii(&self, expr: &MathExpression) -> Result<String> {
        self.ascii_generator.generate(expr)
    }

    /// Render to image
    pub async fn render_to_image(&self, latex: &str, options: RenderOptions)
        -> Result<Vec<u8>> {
        self.renderer.render(latex, options).await
    }

    /// Batch conversion
    pub async fn convert_batch(&self,
        expressions: Vec<MathExpression>,
        formats: Vec<OutputFormat>,
    ) -> Result<Vec<ConversionResult>> {
        expressions.par_iter()
            .flat_map(|expr| {
                formats.iter().map(|fmt| {
                    self.convert_single(expr, *fmt)
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Latex,
    MathML,
    Mmd,
    Ascii,
    Html,
    Png,
    Svg,
}
```

### 3.5 Vector Cache Integration

**Responsibility**: Leverage ruvector-core for intelligent caching and semantic search

```rust
pub struct VectorCache {
    vector_db: Arc<VectorDB>,
    embedding_model: Arc<EmbeddingModel>,
    result_cache: Arc<DashMap<String, CachedResult>>,
}

impl VectorCache {
    /// Generate embedding for image
    pub async fn generate_embedding(&self, image: &ProcessedImage) -> Result<Vec<f32>> {
        self.embedding_model.embed(image).await
    }

    /// Find similar cached results
    pub async fn find_similar(&self, image: &ProcessedImage, limit: usize)
        -> Result<Vec<SimilarResult>> {
        let embedding = self.generate_embedding(image).await?;

        // Search vector database
        let results = self.vector_db.search(&embedding, limit, None).await?;

        // Map to cached results
        results.iter()
            .filter_map(|r| self.result_cache.get(&r.id))
            .map(|cached| SimilarResult {
                result: cached.value().clone(),
                similarity: cached.confidence,
            })
            .collect()
    }

    /// Cache result with embedding
    pub async fn cache_result(&self,
        image: &ProcessedImage,
        result: &OcrResult,
    ) -> Result<()> {
        let embedding = self.generate_embedding(image).await?;
        let id = uuid::Uuid::new_v4().to_string();

        // Store in vector database
        self.vector_db.add(&id, &embedding, None).await?;

        // Cache result
        self.result_cache.insert(id.clone(), CachedResult {
            result: result.clone(),
            timestamp: chrono::Utc::now(),
            hit_count: 0,
        });

        Ok(())
    }
}
```

## 4. Data Flow Pipeline

### 4.1 Processing Pipeline Architecture

```
Input Image (PNG/JPEG/PDF)
        │
        ▼
┌───────────────────────┐
│  1. Image Loading     │  ← Check image cache
│  - Format detection   │
│  - Validation         │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│  2. Preprocessing     │  ← GPU acceleration
│  - Normalization      │
│  - Denoising          │
│  - Deskewing          │
│  - Enhancement        │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│  3. Vector Cache      │  ← Check similar images
│     Lookup            │
└───────────┬───────────┘
            │
     ┌──────┴──────┐
     │ Cache Hit?  │
     └──────┬──────┘
         No │
            ▼
┌───────────────────────┐
│  4. Text Detection    │  ← ONNX model inference
│  - Region detection   │
│  - Bounding boxes     │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│  5. OCR Recognition   │  ← Parallel processing
│  - Character recog    │
│  - Confidence scores  │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│  6. Math Parsing      │
│  - Symbol recognition │
│  - Layout analysis    │
│  - Structure building │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│  7. Output Format     │  ← Parallel conversion
│  - LaTeX generation   │
│  - MathML conversion  │
│  - Multiple formats   │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│  8. Cache Update      │
│  - Store embedding    │
│  - Cache result       │
└───────────┬───────────┘
            │
            ▼
     Final Result
```

### 4.2 Pipeline Implementation

```rust
pub struct Pipeline {
    stages: Vec<Box<dyn PipelineStage>>,
    executor: PipelineExecutor,
}

#[async_trait]
pub trait PipelineStage: Send + Sync {
    async fn execute(&self, context: &mut PipelineContext) -> Result<()>;
    fn name(&self) -> &str;
    fn can_skip(&self, context: &PipelineContext) -> bool;
}

pub struct PipelineExecutor {
    parallelism: usize,
    timeout: Duration,
}

impl Pipeline {
    pub async fn execute(&self, input: ImageInput) -> Result<PipelineResult> {
        let mut context = PipelineContext::new(input);

        for stage in &self.stages {
            // Check if stage can be skipped (e.g., cache hit)
            if stage.can_skip(&context) {
                tracing::info!("Skipping stage: {}", stage.name());
                continue;
            }

            // Execute stage with timeout
            let result = timeout(
                self.executor.timeout,
                stage.execute(&mut context)
            ).await;

            match result {
                Ok(Ok(())) => continue,
                Ok(Err(e)) => return Err(e),
                Err(_) => return Err(Error::StageTimeout(stage.name().to_string())),
            }
        }

        Ok(context.into_result())
    }

    /// Execute batch of images in parallel
    pub async fn execute_batch(&self, inputs: Vec<ImageInput>)
        -> Result<Vec<PipelineResult>> {
        let semaphore = Arc::new(Semaphore::new(self.executor.parallelism));

        let tasks: Vec<_> = inputs.into_iter()
            .map(|input| {
                let pipeline = self.clone();
                let sem = semaphore.clone();

                tokio::spawn(async move {
                    let _permit = sem.acquire().await.unwrap();
                    pipeline.execute(input).await
                })
            })
            .collect();

        futures::future::try_join_all(tasks).await
    }
}
```

### 4.3 Stream Processing

```rust
pub struct StreamProcessor {
    pipeline: Arc<Pipeline>,
    buffer_size: usize,
}

impl StreamProcessor {
    /// Process stream of images
    pub async fn process_stream<S>(&self, stream: S) -> impl Stream<Item = Result<PipelineResult>>
    where
        S: Stream<Item = ImageInput>,
    {
        stream
            .map(|input| {
                let pipeline = self.pipeline.clone();
                async move { pipeline.execute(input).await }
            })
            .buffer_unordered(self.buffer_size)
    }
}
```

## 5. Model Loading and Caching Strategy

### 5.1 Model Management Architecture

```rust
pub struct ModelManager {
    registry: ModelRegistry,
    loader: ModelLoader,
    cache: ModelCache,
    pool: ModelPool,
}

pub struct ModelRegistry {
    models: HashMap<String, ModelMetadata>,
    storage_path: PathBuf,
}

pub struct ModelMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub size: u64,
    pub format: ModelFormat,
    pub checksum: String,
    pub url: Option<String>,
    pub dependencies: Vec<String>,
}

pub enum ModelFormat {
    Onnx,
    TensorflowLite,
    Torch,
}
```

### 5.2 Lazy Loading Strategy

```rust
impl ModelManager {
    /// Load model on-demand
    pub async fn get_or_load(&self, model_id: &str) -> Result<Arc<Model>> {
        // Check memory cache
        if let Some(model) = self.cache.get(model_id).await {
            return Ok(model);
        }

        // Load from disk
        let metadata = self.registry.get_metadata(model_id)?;
        let model = self.loader.load_from_disk(&metadata).await?;

        // Cache in memory
        self.cache.put(model_id, model.clone()).await;

        Ok(model)
    }

    /// Preload models
    pub async fn preload(&self, model_ids: &[&str]) -> Result<()> {
        futures::future::try_join_all(
            model_ids.iter().map(|id| self.get_or_load(id))
        ).await?;
        Ok(())
    }
}
```

### 5.3 Model Pool for Concurrent Inference

```rust
pub struct ModelPool {
    models: Arc<DashMap<String, Vec<Arc<Model>>>>,
    max_instances: usize,
}

impl ModelPool {
    /// Acquire model instance
    pub async fn acquire(&self, model_id: &str) -> Result<ModelGuard> {
        let mut instances = self.models.entry(model_id.to_string())
            .or_insert_with(Vec::new);

        // Return existing instance if available
        if let Some(model) = instances.pop() {
            return Ok(ModelGuard::new(model, self.clone(), model_id));
        }

        // Load new instance if under limit
        if instances.len() < self.max_instances {
            let model = self.load_model(model_id).await?;
            return Ok(ModelGuard::new(model, self.clone(), model_id));
        }

        // Wait for available instance
        self.wait_for_instance(model_id).await
    }
}

pub struct ModelGuard {
    model: Arc<Model>,
    pool: ModelPool,
    model_id: String,
}

impl Drop for ModelGuard {
    fn drop(&mut self) {
        // Return model to pool
        self.pool.return_model(&self.model_id, self.model.clone());
    }
}
```

### 5.4 Model Download and Validation

```rust
pub struct ModelLoader {
    http_client: reqwest::Client,
    storage_path: PathBuf,
}

impl ModelLoader {
    /// Download model from URL
    pub async fn download(&self, metadata: &ModelMetadata) -> Result<PathBuf> {
        let url = metadata.url.as_ref()
            .ok_or(Error::NoDownloadUrl)?;

        let response = self.http_client.get(url)
            .send()
            .await?;

        let path = self.storage_path.join(&metadata.id);
        let mut file = tokio::fs::File::create(&path).await?;

        // Stream download with progress
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            file.write_all(&chunk?).await?;
        }

        // Validate checksum
        self.validate_checksum(&path, &metadata.checksum).await?;

        Ok(path)
    }

    /// Validate model integrity
    async fn validate_checksum(&self, path: &Path, expected: &str) -> Result<()> {
        let bytes = tokio::fs::read(path).await?;
        let hash = sha2::Sha256::digest(&bytes);
        let actual = format!("{:x}", hash);

        if actual != expected {
            return Err(Error::ChecksumMismatch);
        }

        Ok(())
    }
}
```

## 6. Concurrency Model

### 6.1 Hybrid Threading Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
├─────────────────────────────────────────────────────────────┤
│  Async (Tokio)              │       CPU (Rayon)             │
│  - I/O operations           │       - Image processing      │
│  - API requests             │       - OCR inference         │
│  - WebSocket               │       - Batch operations      │
│  - Database queries         │       - Vector operations     │
└─────────────────────────────────────────────────────────────┘
```

### 6.2 Tokio for I/O-Bound Operations

```rust
pub struct AsyncRuntime {
    runtime: tokio::runtime::Runtime,
}

impl AsyncRuntime {
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(num_cpus::get())
            .thread_name("scipix-tokio")
            .enable_all()
            .build()
            .unwrap();

        Self { runtime }
    }

    /// Spawn async task
    pub fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.spawn(future)
    }
}
```

### 6.3 Rayon for CPU-Bound Operations

```rust
pub struct CpuRuntime {
    thread_pool: rayon::ThreadPool,
}

impl CpuRuntime {
    pub fn new() -> Self {
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_cpus::get())
            .thread_name(|idx| format!("scipix-rayon-{}", idx))
            .build()
            .unwrap();

        Self { thread_pool }
    }

    /// Execute parallel computation
    pub fn parallel_map<T, U, F>(&self, items: Vec<T>, f: F) -> Vec<U>
    where
        T: Send,
        U: Send,
        F: Fn(T) -> U + Sync + Send,
    {
        self.thread_pool.install(|| {
            items.into_par_iter().map(f).collect()
        })
    }
}
```

### 6.4 Task Coordination

```rust
pub struct TaskCoordinator {
    async_runtime: Arc<AsyncRuntime>,
    cpu_runtime: Arc<CpuRuntime>,
}

impl TaskCoordinator {
    /// Coordinate async and CPU tasks
    pub async fn process_image(&self, image: DynamicImage) -> Result<OcrResult> {
        // CPU-bound: Image preprocessing
        let preprocessed = self.cpu_runtime.parallel_map(
            vec![image],
            |img| preprocess_image(img)
        ).pop().unwrap();

        // I/O-bound: Check cache
        let cache_result = self.async_runtime.spawn(async move {
            check_cache(&preprocessed).await
        }).await?;

        if let Some(cached) = cache_result {
            return Ok(cached);
        }

        // CPU-bound: OCR inference
        let ocr_result = self.cpu_runtime.parallel_map(
            vec![preprocessed],
            |img| run_ocr(img)
        ).pop().unwrap();

        // I/O-bound: Store in cache
        self.async_runtime.spawn(async move {
            store_cache(&preprocessed, &ocr_result).await
        }).await?;

        Ok(ocr_result)
    }
}
```

### 6.5 Backpressure Management

```rust
pub struct BackpressureController {
    semaphore: Arc<Semaphore>,
    max_concurrent: usize,
}

impl BackpressureController {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            max_concurrent,
        }
    }

    /// Acquire permit with backpressure
    pub async fn acquire(&self) -> SemaphorePermit {
        self.semaphore.acquire().await.unwrap()
    }

    /// Execute with backpressure
    pub async fn execute<F, T>(&self, f: F) -> T
    where
        F: Future<Output = T>,
    {
        let _permit = self.acquire().await;
        f.await
    }
}
```

## 7. Error Handling Strategy

### 7.1 Error Type Hierarchy

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScipixError {
    // Image errors
    #[error("Invalid image format: {0}")]
    InvalidImageFormat(String),

    #[error("Image preprocessing failed: {0}")]
    PreprocessingError(String),

    #[error("Image too large: {size} bytes (max: {max})")]
    ImageTooLarge { size: u64, max: u64 },

    // Model errors
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Model loading failed: {0}")]
    ModelLoadError(String),

    #[error("Model inference failed: {0}")]
    InferenceError(String),

    // OCR errors
    #[error("Text detection failed: {0}")]
    DetectionError(String),

    #[error("Text recognition failed: {0}")]
    RecognitionError(String),

    #[error("Low confidence score: {0} (threshold: {1})")]
    LowConfidence(f32, f32),

    // Math parsing errors
    #[error("Math parsing failed: {0}")]
    ParseError(String),

    #[error("Invalid math expression: {0}")]
    InvalidExpression(String),

    // Cache errors
    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Vector database error: {0}")]
    VectorDbError(#[from] ruvector_core::Error),

    // I/O errors
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    // API errors
    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    // System errors
    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    #[error("Out of memory")]
    OutOfMemory,

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, ScipixError>;
```

### 7.2 Error Recovery Strategies

```rust
pub struct ErrorRecovery {
    retry_config: RetryConfig,
}

pub struct RetryConfig {
    pub max_retries: usize,
    pub backoff: ExponentialBackoff,
    pub retryable_errors: Vec<ErrorKind>,
}

impl ErrorRecovery {
    /// Execute with automatic retry
    pub async fn with_retry<F, T>(&self, mut f: F) -> Result<T>
    where
        F: FnMut() -> Future<Output = Result<T>>,
    {
        let mut attempts = 0;
        let mut delay = self.retry_config.backoff.initial_delay;

        loop {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) if self.is_retryable(&e) && attempts < self.retry_config.max_retries => {
                    attempts += 1;
                    tracing::warn!(
                        "Attempt {} failed: {}. Retrying in {:?}",
                        attempts, e, delay
                    );
                    tokio::time::sleep(delay).await;
                    delay *= 2; // Exponential backoff
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Check if error is retryable
    fn is_retryable(&self, error: &ScipixError) -> bool {
        matches!(error,
            ScipixError::Timeout(_) |
            ScipixError::ApiError { status: 503, .. } |
            ScipixError::InferenceError(_)
        )
    }
}
```

### 7.3 Graceful Degradation

```rust
pub struct GracefulDegradation {
    fallback_chain: Vec<Box<dyn FallbackStrategy>>,
}

#[async_trait]
pub trait FallbackStrategy: Send + Sync {
    async fn try_fallback(&self, context: &ProcessingContext) -> Result<Option<OcrResult>>;
}

impl GracefulDegradation {
    /// Try fallbacks in order
    pub async fn process_with_fallback(&self, context: ProcessingContext) -> Result<OcrResult> {
        // Try primary method
        match self.primary_method(&context).await {
            Ok(result) => return Ok(result),
            Err(primary_error) => {
                tracing::warn!("Primary method failed: {}", primary_error);

                // Try fallbacks
                for (idx, fallback) in self.fallback_chain.iter().enumerate() {
                    match fallback.try_fallback(&context).await {
                        Ok(Some(result)) => {
                            tracing::info!("Fallback {} succeeded", idx);
                            return Ok(result);
                        }
                        Ok(None) => continue,
                        Err(e) => {
                            tracing::warn!("Fallback {} failed: {}", idx, e);
                            continue;
                        }
                    }
                }

                // All fallbacks failed
                Err(primary_error)
            }
        }
    }
}
```

## 8. Configuration Management

### 8.1 Configuration Structure

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScipixConfig {
    pub preprocessing: PreprocessConfig,
    pub ocr: OcrConfig,
    pub math: MathConfig,
    pub cache: CacheConfig,
    pub models: ModelConfig,
    pub api: ApiConfig,
    pub performance: PerformanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessConfig {
    pub target_dpi: u32,
    pub max_dimension: u32,
    pub denoise_strength: f32,
    pub contrast_enhancement: bool,
    pub auto_rotate: bool,
    pub binarization_method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrConfig {
    pub detection_model: String,
    pub recognition_model: String,
    pub confidence_threshold: f32,
    pub batch_size: usize,
    pub languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MathConfig {
    pub symbol_recognition_model: String,
    pub layout_analysis_threshold: f32,
    pub parser_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub enable_vector_cache: bool,
    pub enable_result_cache: bool,
    pub max_cache_size: usize,
    pub ttl_seconds: u64,
    pub similarity_threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub storage_path: String,
    pub auto_download: bool,
    pub max_instances: usize,
    pub preload_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub enable_auth: bool,
    pub rate_limit: usize,
    pub cors_origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub worker_threads: usize,
    pub max_concurrent_requests: usize,
    pub request_timeout_secs: u64,
    pub enable_gpu: bool,
}
```

### 8.2 Configuration Loading

```rust
pub struct ConfigLoader {
    config_path: PathBuf,
}

impl ConfigLoader {
    /// Load configuration from file
    pub fn load(&self) -> Result<ScipixConfig> {
        let contents = std::fs::read_to_string(&self.config_path)?;

        // Support multiple formats
        if self.config_path.extension() == Some("toml".as_ref()) {
            Ok(toml::from_str(&contents)?)
        } else if self.config_path.extension() == Some("json".as_ref()) {
            Ok(serde_json::from_str(&contents)?)
        } else {
            Err(ScipixError::InvalidConfig("Unsupported format".into()))
        }
    }

    /// Load with environment variable overrides
    pub fn load_with_env(&self) -> Result<ScipixConfig> {
        let mut config = self.load()?;
        self.apply_env_overrides(&mut config)?;
        Ok(config)
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(&self, config: &mut ScipixConfig) -> Result<()> {
        // Example: MATHPIX_API_PORT=8080
        if let Ok(port) = std::env::var("MATHPIX_API_PORT") {
            config.api.port = port.parse()?;
        }

        if let Ok(workers) = std::env::var("MATHPIX_WORKER_THREADS") {
            config.performance.worker_threads = workers.parse()?;
        }

        Ok(())
    }
}
```

### 8.3 Default Configuration File

**config/default.toml**:

```toml
[preprocessing]
target_dpi = 300
max_dimension = 4096
denoise_strength = 0.5
contrast_enhancement = true
auto_rotate = true
binarization_method = "adaptive"

[ocr]
detection_model = "craft_mlt_25k"
recognition_model = "crnn_vgg_bilstm_ctc"
confidence_threshold = 0.7
batch_size = 4
languages = ["en", "math"]

[math]
symbol_recognition_model = "math_symbol_classifier_v2"
layout_analysis_threshold = 0.85
parser_mode = "strict"

[cache]
enable_vector_cache = true
enable_result_cache = true
max_cache_size = 1000
ttl_seconds = 3600
similarity_threshold = 0.95

[models]
storage_path = "./models"
auto_download = true
max_instances = 3
preload_models = ["craft_mlt_25k", "crnn_vgg_bilstm_ctc"]

[api]
host = "0.0.0.0"
port = 8080
enable_auth = false
rate_limit = 100
cors_origins = ["*"]

[performance]
worker_threads = 0  # 0 = number of CPUs
max_concurrent_requests = 100
request_timeout_secs = 30
enable_gpu = false
```

## 9. Plugin/Extension Architecture

### 9.1 Plugin System Design

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Plugin metadata
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn description(&self) -> &str;

    /// Lifecycle hooks
    async fn initialize(&mut self, context: &PluginContext) -> Result<()>;
    async fn shutdown(&mut self) -> Result<()>;
}

/// Extension point for custom preprocessors
#[async_trait]
pub trait PreprocessorPlugin: Plugin {
    async fn preprocess(&self, image: DynamicImage) -> Result<DynamicImage>;
}

/// Extension point for custom OCR engines
#[async_trait]
pub trait OcrEnginePlugin: Plugin {
    async fn recognize(&self, image: &ProcessedImage) -> Result<OcrResult>;
}

/// Extension point for custom output formatters
#[async_trait]
pub trait FormatterPlugin: Plugin {
    async fn format(&self, expr: &MathExpression) -> Result<String>;
    fn supported_format(&self) -> OutputFormat;
}

/// Extension point for custom caching strategies
#[async_trait]
pub trait CachePlugin: Plugin {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&self, key: &str, value: Vec<u8>) -> Result<()>;
}
```

### 9.2 Plugin Manager

```rust
pub struct PluginManager {
    preprocessors: Vec<Box<dyn PreprocessorPlugin>>,
    ocr_engines: Vec<Box<dyn OcrEnginePlugin>>,
    formatters: Vec<Box<dyn FormatterPlugin>>,
    caches: Vec<Box<dyn CachePlugin>>,
}

impl PluginManager {
    /// Register plugin
    pub fn register_preprocessor(&mut self, plugin: Box<dyn PreprocessorPlugin>) {
        self.preprocessors.push(plugin);
    }

    /// Initialize all plugins
    pub async fn initialize_all(&mut self, context: &PluginContext) -> Result<()> {
        for plugin in self.preprocessors.iter_mut() {
            plugin.initialize(context).await?;
        }

        for plugin in self.ocr_engines.iter_mut() {
            plugin.initialize(context).await?;
        }

        for plugin in self.formatters.iter_mut() {
            plugin.initialize(context).await?;
        }

        for plugin in self.caches.iter_mut() {
            plugin.initialize(context).await?;
        }

        Ok(())
    }

    /// Get plugin by name
    pub fn get_formatter(&self, name: &str) -> Option<&dyn FormatterPlugin> {
        self.formatters.iter()
            .find(|p| p.name() == name)
            .map(|p| p.as_ref())
    }
}
```

### 9.3 Example Plugin: Custom LaTeX Formatter

```rust
pub struct CustomLatexFormatter {
    config: LatexFormatterConfig,
}

#[async_trait]
impl Plugin for CustomLatexFormatter {
    fn name(&self) -> &str {
        "custom_latex_formatter"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "Custom LaTeX formatter with additional macros"
    }

    async fn initialize(&mut self, context: &PluginContext) -> Result<()> {
        // Load custom macros from config
        tracing::info!("Initialized custom LaTeX formatter");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl FormatterPlugin for CustomLatexFormatter {
    async fn format(&self, expr: &MathExpression) -> Result<String> {
        // Custom LaTeX generation logic
        let mut latex = String::new();
        self.generate_custom_latex(expr, &mut latex)?;
        Ok(latex)
    }

    fn supported_format(&self) -> OutputFormat {
        OutputFormat::Latex
    }
}
```

## 10. Integration with Lean-Agentic Orchestration

### 10.1 Agent Integration Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Lean-Agentic Layer                         │
│                  (Agent Orchestration)                       │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │  Document    │  │  Batch       │  │  Quality         │  │
│  │  Processor   │  │  Coordinator │  │  Validator       │  │
│  │  Agent       │  │  Agent       │  │  Agent           │  │
│  └──────┬───────┘  └──────┬───────┘  └────────┬─────────┘  │
│         │                 │                    │             │
│         └─────────────────┼────────────────────┘             │
│                           │                                  │
└───────────────────────────┼──────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                  Scipix Integration Layer                   │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────┐  │
│  │            Agent Task Interface                       │  │
│  │  - process_document(path) -> Result                  │  │
│  │  - batch_process(paths) -> Vec<Result>               │  │
│  │  - validate_output(result) -> Quality                │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                  Ruvector-Scipix Core                       │
│                  (OCR Processing)                            │
└─────────────────────────────────────────────────────────────┘
```

### 10.2 Agent Task Interface

```rust
/// Integration layer for lean-agentic orchestration
pub struct AgentTaskInterface {
    pipeline: Arc<Pipeline>,
    config: Arc<ScipixConfig>,
    metrics: Arc<MetricsCollector>,
}

#[async_trait]
pub trait AgentTask: Send + Sync {
    async fn execute(&self, input: TaskInput) -> Result<TaskOutput>;
    fn task_type(&self) -> &str;
    fn estimated_duration(&self, input: &TaskInput) -> Duration;
}

impl AgentTaskInterface {
    /// Create agent-compatible task
    pub fn create_ocr_task(&self) -> Box<dyn AgentTask> {
        Box::new(OcrTask {
            pipeline: self.pipeline.clone(),
            metrics: self.metrics.clone(),
        })
    }

    /// Create batch processing task
    pub fn create_batch_task(&self, paths: Vec<PathBuf>) -> Box<dyn AgentTask> {
        Box::new(BatchTask {
            pipeline: self.pipeline.clone(),
            paths,
            metrics: self.metrics.clone(),
        })
    }

    /// Create validation task
    pub fn create_validation_task(&self) -> Box<dyn AgentTask> {
        Box::new(ValidationTask {
            quality_threshold: 0.9,
            metrics: self.metrics.clone(),
        })
    }
}

/// OCR task for single document
pub struct OcrTask {
    pipeline: Arc<Pipeline>,
    metrics: Arc<MetricsCollector>,
}

#[async_trait]
impl AgentTask for OcrTask {
    async fn execute(&self, input: TaskInput) -> Result<TaskOutput> {
        let start = Instant::now();

        // Load image
        let image = image::open(&input.path)?;

        // Process through pipeline
        let result = self.pipeline.execute(ImageInput::new(image)).await?;

        // Record metrics
        self.metrics.record_task_duration(
            "ocr_task",
            start.elapsed()
        );

        Ok(TaskOutput {
            result: result.into_json(),
            metadata: TaskMetadata {
                duration: start.elapsed(),
                confidence: result.confidence,
            },
        })
    }

    fn task_type(&self) -> &str {
        "ocr_document"
    }

    fn estimated_duration(&self, input: &TaskInput) -> Duration {
        // Estimate based on file size
        Duration::from_secs(5)
    }
}

/// Batch processing task
pub struct BatchTask {
    pipeline: Arc<Pipeline>,
    paths: Vec<PathBuf>,
    metrics: Arc<MetricsCollector>,
}

#[async_trait]
impl AgentTask for BatchTask {
    async fn execute(&self, input: TaskInput) -> Result<TaskOutput> {
        let start = Instant::now();

        // Load all images
        let images: Vec<_> = self.paths.iter()
            .map(|path| image::open(path))
            .collect::<Result<_>>()?;

        // Batch process
        let results = self.pipeline.execute_batch(
            images.into_iter().map(ImageInput::new).collect()
        ).await?;

        self.metrics.record_batch_task(
            results.len(),
            start.elapsed()
        );

        Ok(TaskOutput {
            result: serde_json::to_value(results)?,
            metadata: TaskMetadata {
                duration: start.elapsed(),
                confidence: self.calculate_avg_confidence(&results),
            },
        })
    }

    fn task_type(&self) -> &str {
        "batch_process"
    }

    fn estimated_duration(&self, input: &TaskInput) -> Duration {
        Duration::from_secs(5 * self.paths.len() as u64)
    }
}
```

### 10.3 Agent Communication Protocol

```rust
/// Message types for agent communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMessage {
    TaskRequest {
        task_id: String,
        task_type: String,
        input: TaskInput,
        priority: u8,
    },
    TaskProgress {
        task_id: String,
        progress: f32,
        stage: String,
    },
    TaskComplete {
        task_id: String,
        output: TaskOutput,
    },
    TaskFailed {
        task_id: String,
        error: String,
    },
    StatusQuery {
        agent_id: String,
    },
    StatusResponse {
        agent_id: String,
        status: AgentStatus,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub active_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub avg_task_duration: Duration,
    pub memory_usage: u64,
}

/// Agent coordinator for lean-agentic
pub struct AgentCoordinator {
    task_queue: Arc<DashMap<String, AgentMessage>>,
    agents: Arc<DashMap<String, AgentHandle>>,
    message_bus: Arc<MessageBus>,
}

impl AgentCoordinator {
    /// Register scipix agent
    pub async fn register_agent(&self, agent_id: String) -> Result<AgentHandle> {
        let handle = AgentHandle {
            id: agent_id.clone(),
            interface: Arc::new(AgentTaskInterface::new()),
            status: Arc::new(RwLock::new(AgentStatus::default())),
        };

        self.agents.insert(agent_id, handle.clone());
        Ok(handle)
    }

    /// Distribute task to available agent
    pub async fn distribute_task(&self, task: AgentMessage) -> Result<String> {
        // Find available agent
        let agent = self.find_available_agent().await?;

        // Send task via message bus
        self.message_bus.send(&agent.id, task).await?;

        Ok(agent.id)
    }

    /// Collect task results
    pub async fn collect_results(&self, task_id: &str) -> Result<TaskOutput> {
        // Wait for task completion message
        loop {
            if let Some(msg) = self.task_queue.get(task_id) {
                if let AgentMessage::TaskComplete { output, .. } = msg.value() {
                    return Ok(output.clone());
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
```

### 10.4 Workflow Example: Document Processing Pipeline

```rust
/// Lean-agentic workflow for document processing
pub async fn process_documents_workflow(
    coordinator: &AgentCoordinator,
    documents: Vec<PathBuf>,
) -> Result<Vec<ProcessedDocument>> {
    // Step 1: Distribute OCR tasks to agents
    let mut task_ids = vec![];
    for doc in &documents {
        let task = AgentMessage::TaskRequest {
            task_id: Uuid::new_v4().to_string(),
            task_type: "ocr_document".to_string(),
            input: TaskInput {
                path: doc.clone(),
                options: Default::default(),
            },
            priority: 5,
        };

        let agent_id = coordinator.distribute_task(task.clone()).await?;
        task_ids.push((task.task_id, agent_id));
    }

    // Step 2: Collect results
    let mut results = vec![];
    for (task_id, _) in task_ids {
        let output = coordinator.collect_results(&task_id).await?;
        results.push(output);
    }

    // Step 3: Validate results with quality agent
    let validation_tasks: Vec<_> = results.iter()
        .map(|result| {
            AgentMessage::TaskRequest {
                task_id: Uuid::new_v4().to_string(),
                task_type: "validate_output".to_string(),
                input: TaskInput {
                    path: PathBuf::new(),
                    options: serde_json::to_value(result).unwrap(),
                },
                priority: 8,
            }
        })
        .collect();

    let mut validated = vec![];
    for task in validation_tasks {
        let agent_id = coordinator.distribute_task(task.clone()).await?;
        let output = coordinator.collect_results(&task.task_id).await?;
        validated.push(output);
    }

    Ok(validated.into_iter()
        .map(|v| ProcessedDocument::from_json(&v.result))
        .collect::<Result<_>>()?)
}
```

## 11. Deployment Architecture

### 11.1 Deployment Options

```
┌────────────────────────────────────────────────────────────┐
│                    Deployment Options                       │
├────────────────────────────────────────────────────────────┤
│                                                             │
│  1. Standalone Binary (CLI)                                │
│     └─> Linux, macOS, Windows                              │
│                                                             │
│  2. Docker Container (API Server)                          │
│     └─> Kubernetes, Docker Swarm, ECS                      │
│                                                             │
│  3. WebAssembly (Browser)                                  │
│     └─> Edge computing, offline web apps                   │
│                                                             │
│  4. Library (Rust Crate)                                   │
│     └─> Integration into other Rust projects               │
│                                                             │
└────────────────────────────────────────────────────────────┘
```

### 11.2 Docker Deployment

**Dockerfile**:

```dockerfile
FROM rust:1.77 as builder

WORKDIR /app
COPY . .

# Build release binary
RUN cargo build --release --bin ruvector-scipix-api

# Runtime image
FROM debian:bookworm-slim

# Install dependencies
RUN apt-get update && apt-get install -y \
    libgomp1 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary and models
COPY --from=builder /app/target/release/ruvector-scipix-api /usr/local/bin/
COPY models /app/models
COPY config /app/config

WORKDIR /app

EXPOSE 8080

CMD ["ruvector-scipix-api"]
```

## 12. Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Single image OCR | < 500ms | For images < 2MB |
| Batch processing (10 images) | < 3s | Parallel processing |
| LaTeX accuracy | > 95% | For clean images |
| Memory usage | < 500MB | Base + 2 models loaded |
| Cache hit rate | > 80% | With vector similarity search |
| API throughput | > 100 req/s | With 8 CPU cores |

## 13. Security Considerations

1. **Input Validation**: Strict image format and size validation
2. **Sandboxing**: Isolate model inference in separate processes
3. **Rate Limiting**: Prevent abuse of API endpoints
4. **Authentication**: JWT-based auth for API access
5. **Data Privacy**: No image storage by default, opt-in caching

## 14. Future Enhancements

1. **GPU Acceleration**: CUDA/OpenCL support for faster inference
2. **Streaming OCR**: Process video streams frame-by-frame
3. **Multi-Language**: Support for non-Latin scripts
4. **Handwriting Recognition**: Support handwritten math
5. **Equation Solving**: Integration with symbolic math engines
6. **Real-time Collaboration**: WebSocket-based live OCR

## References

- [Scipix OCR](https://scipix.com/)
- [CRAFT Text Detection](https://arxiv.org/abs/1904.01941)
- [CRNN Text Recognition](https://arxiv.org/abs/1507.05717)
- [ONNX Runtime](https://onnxruntime.ai/)
- [Ruvector Documentation](https://github.com/ruvnet/ruvector)

---

**Document Status**: Draft
**Next Phase**: Refinement (Test-Driven Development)
**Review Required**: Yes
