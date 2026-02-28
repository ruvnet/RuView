# Scipix Clone - System Requirements Specification

**Version:** 1.0.0
**Date:** 2025-11-28
**Project:** ruvector-scipix
**Methodology:** SPARC (Specification Phase)

---

## Table of Contents

1. [Project Overview & Goals](#1-project-overview--goals)
2. [Functional Requirements](#2-functional-requirements)
3. [Non-Functional Requirements](#3-non-functional-requirements)
4. [Input/Output Specifications](#4-inputoutput-specifications)
5. [API Design](#5-api-design)
6. [Data Models](#6-data-models)
7. [Use Cases and User Stories](#7-use-cases-and-user-stories)
8. [Success Criteria and Acceptance Tests](#8-success-criteria-and-acceptance-tests)
9. [Constraints and Limitations](#9-constraints-and-limitations)
10. [Dependencies](#10-dependencies)

---

## 1. Project Overview & Goals

### 1.1 Purpose

This system provides an open-source Rust implementation of mathematical and scientific content recognition, compatible with the Scipix API v3. The system converts images containing mathematical equations, chemical formulas, tables, and diagrams into machine-readable formats (LaTeX, MathML, Markdown, etc.).

### 1.2 Scope

**In Scope:**
- Mathematical equation recognition (printed and handwritten)
- Chemical formula recognition
- Table and diagram extraction
- Multi-format input support (JPEG, PNG, PDF, etc.)
- Multi-format output (LaTeX, MathML, Markdown, HTML, DOCX)
- RESTful API compatible with Scipix v3
- Vector storage integration via ruvector-core
- Confidence scoring and metadata extraction
- Line/word segmentation and geometry analysis

**Out of Scope:**
- Real-time video processing
- 3D model recognition
- Audio transcription
- Mobile app development (API only)

### 1.3 Target Users

- **Researchers**: Converting papers to digital format
- **Students**: Digitizing handwritten notes
- **Educators**: Creating accessible educational content
- **Developers**: Building applications requiring math OCR
- **Publishers**: Converting legacy documents to modern formats

### 1.4 Project Goals

1. **API Compatibility**: 95%+ compatibility with Scipix API v3
2. **Performance**: <100ms latency for single image processing
3. **Accuracy**: 95%+ on printed math, 90%+ on handwritten
4. **Open Source**: Fully auditable, extensible, community-driven
5. **Scalability**: Handle concurrent requests efficiently
6. **Cost Efficiency**: Reduce OCR costs by 10x vs commercial solutions

---

## 2. Functional Requirements

### 2.1 Image Processing

#### FR-2.1.1: Image Input Support
**Priority:** High
**Description:** System shall accept images in multiple formats

**Acceptance Criteria:**
- Support JPEG, PNG, GIF, TIFF, WebP, BMP formats
- Accept Base64-encoded image data
- Accept image URLs (HTTP/HTTPS)
- Handle images up to 10MB in size
- Support images from 100x100 to 4000x4000 pixels
- Auto-rotate based on EXIF orientation

**Example:**
```rust
pub enum ImageInput {
    Base64(String),
    Url(String),
    Binary(Vec<u8>),
}

pub struct ImageConstraints {
    max_size_bytes: usize,      // 10MB
    min_dimension: u32,          // 100px
    max_dimension: u32,          // 4000px
    supported_formats: Vec<ImageFormat>,
}
```

#### FR-2.1.2: PDF Processing
**Priority:** High
**Description:** System shall extract and process mathematical content from PDF documents

**Acceptance Criteria:**
- Support PDF files up to 100 pages
- Extract text with position information
- Render pages to images for OCR
- Preserve page structure and layout
- Support both text-based and scanned PDFs
- Extract embedded LaTeX if available

#### FR-2.1.3: Document Processing
**Priority:** Medium
**Description:** System shall process EPUB, DOCX, PPTX documents

**Acceptance Criteria:**
- Extract text and images from EPUB
- Parse DOCX mathematical content (Office Math ML)
- Extract slides from PPTX
- Maintain document structure metadata
- Support password-protected documents (optional)

### 2.2 Mathematical Recognition

#### FR-2.2.1: Equation Recognition
**Priority:** High
**Description:** System shall recognize and convert mathematical equations

**Acceptance Criteria:**
- Recognize inline and display equations
- Support basic arithmetic operations (+, -, ×, ÷)
- Support algebraic notation (variables, exponents, subscripts)
- Support calculus (integrals, derivatives, limits)
- Support linear algebra (matrices, vectors)
- Support set theory and logic notation
- Output confidence scores per equation

**Example:**
```rust
pub struct EquationRecognition {
    detected_math: Vec<MathRegion>,
    confidence: f32,
    latex: String,
    mathml: Option<String>,
    asciimath: Option<String>,
}

pub struct MathRegion {
    bbox: BoundingBox,
    equation_type: EquationType,
    symbols: Vec<Symbol>,
}

pub enum EquationType {
    Inline,
    Display,
    Numbered,
}
```

#### FR-2.2.2: Chemical Formula Recognition
**Priority:** Medium
**Description:** System shall recognize chemical formulas and reactions

**Acceptance Criteria:**
- Recognize molecular formulas (H₂O, C₆H₁₂O₆)
- Support chemical equations and reactions
- Recognize structural formulas (basic)
- Output in SMILES or InChI notation
- Support subscripts and superscripts (charges)

#### FR-2.2.3: Handwritten Math Recognition
**Priority:** High
**Description:** System shall recognize handwritten mathematical notation

**Acceptance Criteria:**
- Process handwritten equations with 90%+ accuracy
- Support various handwriting styles
- Handle connected and separated characters
- Detect stroke order (if available)
- Provide confidence scores per symbol

### 2.3 Output Formats

#### FR-2.3.1: LaTeX Output
**Priority:** High
**Description:** System shall generate valid LaTeX markup

**Acceptance Criteria:**
- Generate compilable LaTeX code
- Support standard LaTeX packages (amsmath, amssymb)
- Include proper math delimiters ($, $$, \[, \])
- Maintain equation structure and alignment
- Support custom LaTeX macros (configurable)

**Example:**
```rust
pub struct LatexOutput {
    latex: String,
    packages_required: Vec<String>,
    preamble: Option<String>,
    errors: Vec<LatexValidationError>,
}

impl LatexOutput {
    pub fn validate(&self) -> Result<(), LatexError> {
        // Validate LaTeX syntax
    }

    pub fn compile_test(&self) -> Result<Vec<u8>, CompilationError> {
        // Test compilation to PDF
    }
}
```

#### FR-2.3.2: Scipix Markdown (MMD)
**Priority:** High
**Description:** System shall generate Scipix Markdown format

**Acceptance Criteria:**
- Support MMD syntax extensions
- Include metadata blocks
- Preserve document structure
- Support tables, lists, headings
- Include image references and captions

#### FR-2.3.3: MathML Output
**Priority:** Medium
**Description:** System shall generate MathML markup

**Acceptance Criteria:**
- Generate valid MathML 3.0
- Support both Presentation and Content MathML
- Include semantic annotations
- Validate against MathML schema

#### FR-2.3.4: AsciiMath Output
**Priority:** Low
**Description:** System shall generate AsciiMath notation

**Acceptance Criteria:**
- Generate human-readable AsciiMath
- Support basic mathematical operations
- Maintain expression structure

#### FR-2.3.5: HTML/DOCX Export
**Priority:** Medium
**Description:** System shall export to HTML and DOCX formats

**Acceptance Criteria:**
- Generate semantic HTML with MathJax
- Create valid DOCX with Office Math ML
- Preserve formatting and structure
- Include CSS styling (HTML)

### 2.4 API Endpoints

#### FR-2.4.1: Text Recognition Endpoint
**Priority:** High
**Description:** POST /v3/text endpoint for image-to-text conversion

**Acceptance Criteria:**
- Accept multipart/form-data or JSON
- Support batch processing (multiple images)
- Return confidence scores
- Support async processing for large batches
- Implement rate limiting

#### FR-2.4.2: Strokes Recognition Endpoint
**Priority:** Medium
**Description:** POST /v3/strokes endpoint for handwritten strokes

**Acceptance Criteria:**
- Accept stroke data (x, y coordinates, timestamps)
- Process real-time input
- Return incremental results
- Support stroke order analysis

#### FR-2.4.3: LaTeX Rendering Endpoint
**Priority:** Medium
**Description:** POST /v3/latex endpoint for LaTeX-to-image

**Acceptance Criteria:**
- Render LaTeX to PNG/SVG
- Support custom DPI settings
- Return rendered image and metadata
- Cache rendered results

#### FR-2.4.4: PDF Conversion Endpoint
**Priority:** High
**Description:** POST /v3/pdf endpoint for PDF processing

**Acceptance Criteria:**
- Accept PDF uploads
- Process multi-page documents
- Return page-by-page results
- Support partial processing (page ranges)

### 2.5 Additional Features

#### FR-2.5.1: Confidence Scoring
**Priority:** High
**Description:** System shall provide confidence scores for all recognition

**Acceptance Criteria:**
- Score range: 0.0 to 1.0
- Per-symbol confidence scores
- Overall equation confidence
- Calibrated probability estimates

```rust
pub struct ConfidenceScores {
    overall: f32,
    per_symbol: Vec<(Symbol, f32)>,
    per_line: Vec<f32>,
    calibrated: bool,
}
```

#### FR-2.5.2: Geometry Analysis
**Priority:** Medium
**Description:** System shall extract geometric information

**Acceptance Criteria:**
- Detect bounding boxes for all elements
- Identify text baseline and orientation
- Detect equation alignment
- Extract line and paragraph structure

```rust
pub struct GeometryInfo {
    bounding_boxes: Vec<BoundingBox>,
    baselines: Vec<Line>,
    text_orientation: f32,
    line_spacing: f32,
    columns: Option<Vec<Column>>,
}

pub struct BoundingBox {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rotation: f32,
}
```

#### FR-2.5.3: Line/Word Segmentation
**Priority:** Medium
**Description:** System shall segment text into lines and words

**Acceptance Criteria:**
- Detect individual words
- Identify line breaks
- Separate equations from text
- Handle multi-column layouts

---

## 3. Non-Functional Requirements

### 3.1 Performance

#### NFR-3.1.1: Latency
**Priority:** High
**Requirement:** Single image processing <100ms (95th percentile)

**Measurement:**
- p50 latency: <50ms
- p95 latency: <100ms
- p99 latency: <200ms

**Test Cases:**
```rust
#[tokio::test]
async fn test_single_image_latency() {
    let image = load_test_image("simple_equation.png");
    let start = Instant::now();
    let result = processor.process(image).await.unwrap();
    let duration = start.elapsed();
    assert!(duration < Duration::from_millis(100));
}
```

#### NFR-3.1.2: Throughput
**Priority:** High
**Requirement:** Process 100 requests per second per core

**Measurement:**
- Single core: 100 req/s
- 4 cores: 350+ req/s (accounting for overhead)
- 8 cores: 650+ req/s

#### NFR-3.1.3: Batch Processing
**Priority:** Medium
**Requirement:** Process 100-image batch in <5 seconds

**Measurement:**
- Average time per image in batch: <50ms
- Total batch overhead: <500ms

### 3.2 Accuracy

#### NFR-3.2.1: Printed Math Accuracy
**Priority:** High
**Requirement:** 95%+ character-level accuracy on printed equations

**Measurement:**
- Use standard math OCR benchmark datasets
- Calculate Character Error Rate (CER)
- Test on various fonts and sizes

**Validation:**
```rust
pub fn calculate_accuracy(ground_truth: &str, predicted: &str) -> AccuracyMetrics {
    AccuracyMetrics {
        character_error_rate: calculate_cer(ground_truth, predicted),
        word_error_rate: calculate_wer(ground_truth, predicted),
        equation_match: exact_match(ground_truth, predicted),
    }
}
```

#### NFR-3.2.2: Handwritten Math Accuracy
**Priority:** High
**Requirement:** 90%+ character-level accuracy on handwritten equations

**Measurement:**
- Test on CROHME dataset
- Calculate symbol recognition rate
- Measure expression recognition rate

#### NFR-3.2.3: Chemical Formula Accuracy
**Priority:** Medium
**Requirement:** 93%+ accuracy on chemical formulas

**Measurement:**
- Test on ChemDraw and standard chemistry datasets
- Validate SMILES generation
- Check stoichiometry preservation

### 3.3 Scalability

#### NFR-3.3.1: Concurrent Users
**Priority:** High
**Requirement:** Support 1000+ concurrent users

**Constraints:**
- Connection pooling
- Request queueing
- Resource limits per user

#### NFR-3.3.2: Horizontal Scaling
**Priority:** High
**Requirement:** Linear scaling up to 10 nodes

**Architecture:**
- Stateless API servers
- Shared vector database
- Distributed caching

#### NFR-3.3.3: Memory Usage
**Priority:** High
**Requirement:** <2GB RAM per worker process

**Constraints:**
- Model size optimization
- Efficient image buffering
- Memory-mapped model loading

### 3.4 Reliability

#### NFR-3.4.1: Availability
**Priority:** High
**Requirement:** 99.9% uptime (SLA)

**Measurement:**
- Planned downtime excluded
- Maximum 8.76 hours downtime per year

#### NFR-3.4.2: Error Handling
**Priority:** High
**Requirement:** Graceful degradation for all error cases

**Implementation:**
```rust
pub enum ProcessingError {
    ImageFormatUnsupported(String),
    ImageTooLarge { size: usize, max: usize },
    ImageDimensionInvalid { width: u32, height: u32 },
    OCRProcessingFailed { reason: String },
    LatexGenerationFailed { partial_result: Option<String> },
    TimeoutExceeded { duration: Duration },
}

impl ProcessingError {
    pub fn to_user_message(&self) -> String {
        // User-friendly error messages
    }

    pub fn recovery_action(&self) -> Option<RecoveryAction> {
        // Suggest recovery actions
    }
}
```

#### NFR-3.4.3: Data Validation
**Priority:** High
**Requirement:** Validate all inputs before processing

**Checks:**
- File format validation
- Size limits enforcement
- Content type verification
- Malicious content detection

### 3.5 Security

#### NFR-3.5.1: Authentication
**Priority:** High
**Requirement:** API key-based authentication

**Implementation:**
- SHA-256 hashed API keys
- Rate limiting per key
- Key rotation support
- Expiration policies

```rust
pub struct ApiKey {
    id: Uuid,
    key_hash: String,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    rate_limit: RateLimit,
    permissions: Vec<Permission>,
}
```

#### NFR-3.5.2: Data Privacy
**Priority:** High
**Requirement:** No persistent storage of user images

**Policies:**
- Images processed in memory
- Automatic cleanup after processing
- Optional temporary storage (user consent)
- No logging of image content

#### NFR-3.5.3: Input Sanitization
**Priority:** High
**Requirement:** Sanitize all inputs to prevent attacks

**Protections:**
- Image bomb detection
- Zip bomb prevention
- Path traversal prevention
- Script injection prevention

### 3.6 Usability

#### NFR-3.6.1: API Design
**Priority:** High
**Requirement:** RESTful API following OpenAPI 3.0 specification

**Standards:**
- Consistent error responses
- Comprehensive documentation
- Example code in 5+ languages
- Interactive API explorer

#### NFR-3.6.2: Error Messages
**Priority:** Medium
**Requirement:** Clear, actionable error messages

**Format:**
```rust
pub struct ApiError {
    code: String,
    message: String,
    details: Option<serde_json::Value>,
    suggestion: Option<String>,
    documentation_url: Option<String>,
}
```

### 3.7 Maintainability

#### NFR-3.7.1: Code Quality
**Priority:** High
**Requirements:**
- 80%+ test coverage
- Clippy warnings as errors
- Rustfmt formatting enforced
- Documentation for public APIs

#### NFR-3.7.2: Logging
**Priority:** High
**Requirement:** Structured logging at multiple levels

**Levels:**
- ERROR: Processing failures
- WARN: Degraded performance
- INFO: Request/response logs
- DEBUG: Detailed processing steps
- TRACE: Symbol-level recognition

```rust
use tracing::{info, debug, error};

#[instrument(skip(image_data))]
async fn process_image(image_data: &[u8]) -> Result<Recognition> {
    info!("Starting image processing");
    debug!("Image size: {} bytes", image_data.len());

    let result = recognize(image_data).await?;

    info!(
        confidence = %result.confidence,
        symbols_detected = result.symbols.len(),
        "Processing complete"
    );

    Ok(result)
}
```

#### NFR-3.7.3: Monitoring
**Priority:** High
**Requirement:** Prometheus metrics for all operations

**Metrics:**
- Request rate
- Error rate
- Processing latency
- Model inference time
- Memory usage
- Queue depth

---

## 4. Input/Output Specifications

### 4.1 Input Specifications

#### 4.1.1 Image Input

**Supported Formats:**
```rust
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    Tiff,
    WebP,
    Bmp,
}

pub struct ImageInput {
    format: ImageFormat,
    data: ImageData,
    metadata: Option<ImageMetadata>,
}

pub enum ImageData {
    Base64(String),
    Binary(Vec<u8>),
    Url(String),
}

pub struct ImageMetadata {
    width: u32,
    height: u32,
    dpi: Option<u32>,
    color_space: ColorSpace,
    exif: Option<ExifData>,
}
```

**Constraints:**
```rust
pub const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB
pub const MIN_DIMENSION: u32 = 100;
pub const MAX_DIMENSION: u32 = 4000;
pub const SUPPORTED_MIME_TYPES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/tiff",
    "image/webp",
    "image/bmp",
];
```

**Example JSON Request:**
```json
{
  "src": "data:image/jpeg;base64,/9j/4AAQSkZJRg...",
  "formats": ["latex", "mathml", "text"],
  "ocr": ["math", "text"],
  "metadata": {
    "include_geometry": true,
    "include_confidence": true,
    "include_line_data": true
  }
}
```

#### 4.1.2 PDF Input

```rust
pub struct PdfInput {
    data: Vec<u8>,
    options: PdfProcessingOptions,
}

pub struct PdfProcessingOptions {
    page_range: Option<Range<usize>>,
    dpi: u32, // Default: 300
    extract_text: bool,
    extract_images: bool,
    preserve_layout: bool,
}
```

**Example Request:**
```json
{
  "pdf": "base64_encoded_pdf_data",
  "conversion_formats": {
    "latex": true,
    "mmd": true
  },
  "page_ranges": [[1, 10]],
  "options": {
    "dpi": 300,
    "extract_text": true
  }
}
```

#### 4.1.3 Stroke Input (Handwriting)

```rust
pub struct StrokeInput {
    strokes: Vec<Stroke>,
    canvas_size: (u32, u32),
}

pub struct Stroke {
    points: Vec<Point>,
    timestamps: Option<Vec<u64>>, // milliseconds
    pressure: Option<Vec<f32>>,   // 0.0 to 1.0
}

pub struct Point {
    x: f32,
    y: f32,
}
```

**Example Request:**
```json
{
  "strokes": [
    {
      "points": [[10, 20], [15, 25], [20, 30]],
      "timestamps": [0, 50, 100]
    }
  ],
  "canvas_size": [800, 600],
  "formats": ["latex"]
}
```

### 4.2 Output Specifications

#### 4.2.1 Recognition Response

```rust
pub struct RecognitionResponse {
    // Core recognition
    text: String,
    latex: Option<String>,
    mathml: Option<String>,
    asciimath: Option<String>,
    mmd: Option<String>,

    // Confidence and quality
    confidence: f32,
    confidence_rate: f32,

    // Geometric information
    line_data: Option<Vec<LineData>>,
    word_data: Option<Vec<WordData>>,
    position: Option<Position>,

    // Metadata
    is_printed: Option<bool>,
    is_handwritten: Option<bool>,
    detected_alphabets: Vec<Alphabet>,

    // Processing info
    processing_time_ms: u64,
    model_version: String,
}

pub struct LineData {
    text: String,
    confidence: f32,
    bbox: BoundingBox,
    type_: LineType,
}

pub enum LineType {
    Text,
    Math,
    ChemicalFormula,
    Table,
    Diagram,
}

pub struct WordData {
    text: String,
    confidence: f32,
    bbox: BoundingBox,
}

pub enum Alphabet {
    Latin,
    Greek,
    Cyrillic,
    Hebrew,
    Arabic,
    Mathematical,
    Chemical,
}
```

**Example JSON Response:**
```json
{
  "text": "The quadratic formula is x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}",
  "latex": "x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}",
  "mathml": "<math>...</math>",
  "confidence": 0.97,
  "confidence_rate": 0.95,
  "line_data": [
    {
      "text": "The quadratic formula is",
      "confidence": 0.99,
      "bbox": {"x": 10, "y": 20, "width": 200, "height": 25},
      "type": "text"
    },
    {
      "text": "x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}",
      "confidence": 0.96,
      "bbox": {"x": 10, "y": 50, "width": 300, "height": 40},
      "type": "math"
    }
  ],
  "is_printed": true,
  "is_handwritten": false,
  "detected_alphabets": ["latin", "mathematical"],
  "processing_time_ms": 87,
  "model_version": "1.0.0"
}
```

#### 4.2.2 Error Response

```rust
pub struct ErrorResponse {
    error: String,
    error_code: ErrorCode,
    message: String,
    details: Option<serde_json::Value>,
    suggestion: Option<String>,
    documentation_url: String,
}

pub enum ErrorCode {
    InvalidInput,
    UnsupportedFormat,
    ImageTooLarge,
    ProcessingTimeout,
    InternalError,
    RateLimitExceeded,
    UnauthorizedRequest,
}
```

**Example Error Response:**
```json
{
  "error": "invalid_image_format",
  "error_code": "UNSUPPORTED_FORMAT",
  "message": "The provided image format is not supported",
  "details": {
    "detected_format": "image/svg+xml",
    "supported_formats": ["image/jpeg", "image/png", "image/gif"]
  },
  "suggestion": "Convert your image to JPEG or PNG format before uploading",
  "documentation_url": "https://docs.scipix.com/formats"
}
```

#### 4.2.3 Batch Processing Response

```rust
pub struct BatchResponse {
    results: Vec<BatchResult>,
    total_processing_time_ms: u64,
    success_count: usize,
    failure_count: usize,
}

pub struct BatchResult {
    index: usize,
    success: bool,
    result: Option<RecognitionResponse>,
    error: Option<ErrorResponse>,
}
```

---

## 5. API Design

### 5.1 REST API Specification

#### Base URL
```
https://api.scipix.com/v3/
```

#### Authentication
```http
Authorization: Bearer <api_key>
Content-Type: application/json
```

### 5.2 Endpoints

#### 5.2.1 Text Recognition

**Endpoint:** `POST /v3/text`

**Description:** Convert image to text and mathematical markup

**Request:**
```rust
pub struct TextRecognitionRequest {
    /// Image source (Base64, URL, or binary)
    src: ImageSource,

    /// Output formats to generate
    #[serde(default)]
    formats: Vec<OutputFormat>,

    /// OCR modes to use
    #[serde(default)]
    ocr: Vec<OcrMode>,

    /// Processing options
    #[serde(default)]
    options: ProcessingOptions,

    /// Metadata to include in response
    #[serde(default)]
    metadata: MetadataOptions,
}

pub enum ImageSource {
    Base64(String),
    Url(String),
    Binary(Vec<u8>),
}

pub enum OutputFormat {
    Text,
    Latex,
    MathML,
    AsciiMath,
    MMD,
    HTML,
}

pub enum OcrMode {
    Math,
    Text,
    Chemistry,
    Table,
    Diagram,
}

pub struct ProcessingOptions {
    /// Enable equation numbering
    pub equation_numbers: Option<bool>,

    /// Include LaTeX packages
    pub latex_packages: Option<Vec<String>>,

    /// Custom delimiters for math
    pub math_delimiters: Option<MathDelimiters>,

    /// Confidence threshold (0.0-1.0)
    pub confidence_threshold: Option<f32>,

    /// Enable preprocessing
    pub preprocessing: Option<PreprocessingOptions>,
}

pub struct MetadataOptions {
    pub include_geometry: bool,
    pub include_confidence: bool,
    pub include_line_data: bool,
    pub include_word_data: bool,
}
```

**Example Request:**
```http
POST /v3/text HTTP/1.1
Authorization: Bearer sk_live_abc123
Content-Type: application/json

{
  "src": "data:image/png;base64,iVBORw0KGgo...",
  "formats": ["latex", "mathml", "text"],
  "ocr": ["math", "text"],
  "options": {
    "equation_numbers": true,
    "confidence_threshold": 0.8
  },
  "metadata": {
    "include_geometry": true,
    "include_confidence": true
  }
}
```

**Response:** `200 OK`
```json
{
  "request_id": "req_abc123",
  "text": "Einstein's equation: E = mc^2",
  "latex": "E = mc^2",
  "mathml": "<math><mi>E</mi><mo>=</mo><mi>m</mi><msup><mi>c</mi><mn>2</mn></msup></math>",
  "confidence": 0.98,
  "processing_time_ms": 75
}
```

#### 5.2.2 Stroke Recognition

**Endpoint:** `POST /v3/strokes`

**Description:** Convert handwritten strokes to mathematical notation

**Request:**
```rust
pub struct StrokeRecognitionRequest {
    strokes: Vec<Stroke>,
    canvas_size: (u32, u32),
    formats: Vec<OutputFormat>,
    options: StrokeProcessingOptions,
}

pub struct StrokeProcessingOptions {
    /// Recognize as equation or expression
    pub mode: StrokeMode,

    /// Previous context for incremental recognition
    pub context: Option<String>,

    /// Language/alphabet hint
    pub alphabet_hint: Option<Vec<Alphabet>>,
}

pub enum StrokeMode {
    Expression,
    Equation,
    Text,
}
```

**Example Request:**
```http
POST /v3/strokes HTTP/1.1
Authorization: Bearer sk_live_abc123
Content-Type: application/json

{
  "strokes": [
    {
      "points": [[50, 100], [55, 95], [60, 90]],
      "timestamps": [0, 50, 100]
    }
  ],
  "canvas_size": [800, 600],
  "formats": ["latex", "text"]
}
```

#### 5.2.3 LaTeX Rendering

**Endpoint:** `POST /v3/latex`

**Description:** Render LaTeX to image

**Request:**
```rust
pub struct LatexRenderRequest {
    latex: String,
    format: ImageFormat,
    options: RenderOptions,
}

pub struct RenderOptions {
    pub dpi: u32,              // Default: 300
    pub foreground: String,     // Hex color
    pub background: String,     // Hex color
    pub padding: u32,          // Pixels
    pub font_size: u32,        // Points
}
```

**Example Request:**
```http
POST /v3/latex HTTP/1.1
Authorization: Bearer sk_live_abc123
Content-Type: application/json

{
  "latex": "\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}",
  "format": "png",
  "options": {
    "dpi": 300,
    "foreground": "#000000",
    "background": "#FFFFFF"
  }
}
```

**Response:** Binary image data or Base64

#### 5.2.4 PDF Processing

**Endpoint:** `POST /v3/pdf`

**Description:** Convert PDF to text and mathematical markup

**Request:**
```rust
pub struct PdfProcessingRequest {
    pdf: Vec<u8>,  // Base64 or binary
    conversion_formats: ConversionFormats,
    page_ranges: Option<Vec<Range<usize>>>,
    options: PdfOptions,
}

pub struct ConversionFormats {
    pub latex: bool,
    pub mathml: bool,
    pub mmd: bool,
    pub docx: bool,
    pub html: bool,
}

pub struct PdfOptions {
    pub dpi: u32,
    pub extract_text: bool,
    pub extract_images: bool,
    pub preserve_layout: bool,
    pub ocr_strategy: OcrStrategy,
}

pub enum OcrStrategy {
    Auto,
    AlwaysOcr,
    TextOnly,
}
```

**Example Request:**
```http
POST /v3/pdf HTTP/1.1
Authorization: Bearer sk_live_abc123
Content-Type: multipart/form-data

{
  "pdf": "base64_pdf_data",
  "conversion_formats": {
    "latex": true,
    "mmd": true
  },
  "page_ranges": [[1, 5]],
  "options": {
    "dpi": 300,
    "ocr_strategy": "auto"
  }
}
```

**Response:**
```json
{
  "pages": [
    {
      "page_number": 1,
      "text": "...",
      "latex": "...",
      "mmd": "..."
    }
  ],
  "total_pages": 5,
  "processing_time_ms": 2340
}
```

### 5.3 Rate Limiting

```rust
pub struct RateLimiter {
    requests_per_second: u32,
    requests_per_hour: u32,
    concurrent_requests: u32,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            requests_per_second: 10,
            requests_per_hour: 1000,
            concurrent_requests: 5,
        }
    }
}
```

**Rate Limit Headers:**
```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 950
X-RateLimit-Reset: 1640995200
```

### 5.4 Versioning

- API version in URL: `/v3/`
- Backward compatibility for minor versions
- Deprecation notices 6 months before removal

---

## 6. Data Models

### 6.1 Core Models

#### 6.1.1 Mathematical Expression

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MathExpression {
    pub id: Uuid,
    pub latex: String,
    pub mathml: Option<String>,
    pub asciimath: Option<String>,
    pub expression_tree: ExpressionTree,
    pub symbols: Vec<MathSymbol>,
    pub bounding_box: BoundingBox,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionTree {
    pub root: ExpressionNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionNode {
    pub node_type: NodeType,
    pub value: Option<String>,
    pub children: Vec<ExpressionNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    Number,
    Variable,
    Operator(Operator),
    Function(Function),
    Fraction,
    Exponent,
    Subscript,
    Matrix,
    Integral,
    Sum,
    Product,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equals,
    LessThan,
    GreaterThan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Function {
    Sin,
    Cos,
    Tan,
    Log,
    Ln,
    Sqrt,
    Custom(String),
}
```

#### 6.1.2 Symbol Recognition

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MathSymbol {
    pub id: Uuid,
    pub symbol: String,
    pub unicode: u32,
    pub latex_command: String,
    pub category: SymbolCategory,
    pub bounding_box: BoundingBox,
    pub confidence: f32,
    pub alternatives: Vec<SymbolAlternative>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SymbolCategory {
    Digit,
    Letter,
    GreekLetter,
    Operator,
    Relation,
    Delimiter,
    Arrow,
    Accent,
    LargeOperator,
    BinaryOperator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolAlternative {
    pub symbol: String,
    pub confidence: f32,
}
```

#### 6.1.3 Document Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: Uuid,
    pub pages: Vec<Page>,
    pub metadata: DocumentMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub page_number: usize,
    pub blocks: Vec<ContentBlock>,
    pub dimensions: (u32, u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentBlock {
    Text(TextBlock),
    Math(MathBlock),
    Table(TableBlock),
    Image(ImageBlock),
    Diagram(DiagramBlock),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    pub text: String,
    pub lines: Vec<TextLine>,
    pub bounding_box: BoundingBox,
    pub font_info: Option<FontInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MathBlock {
    pub expression: MathExpression,
    pub display_mode: bool,
    pub numbered: bool,
    pub equation_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableBlock {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<Vec<ContentBlock>>,
    pub bounding_box: BoundingBox,
}
```

### 6.2 Processing Models

#### 6.2.1 Recognition Pipeline

```rust
#[derive(Debug, Clone)]
pub struct RecognitionPipeline {
    pub stages: Vec<PipelineStage>,
}

#[derive(Debug, Clone)]
pub enum PipelineStage {
    Preprocessing(PreprocessingConfig),
    Detection(DetectionConfig),
    Recognition(RecognitionConfig),
    Postprocessing(PostprocessingConfig),
}

#[derive(Debug, Clone)]
pub struct PreprocessingConfig {
    pub denoise: bool,
    pub deskew: bool,
    pub binarize: bool,
    pub enhance_contrast: bool,
    pub remove_artifacts: bool,
}

#[derive(Debug, Clone)]
pub struct DetectionConfig {
    pub detect_text: bool,
    pub detect_math: bool,
    pub detect_tables: bool,
    pub detect_diagrams: bool,
    pub min_confidence: f32,
}

#[derive(Debug, Clone)]
pub struct RecognitionConfig {
    pub model_type: ModelType,
    pub beam_width: usize,
    pub temperature: f32,
    pub max_length: usize,
}

#[derive(Debug, Clone)]
pub enum ModelType {
    CnnLstm,
    Transformer,
    Hybrid,
}
```

### 6.3 Storage Models

#### 6.3.1 Vector Embeddings

```rust
use ruvector_core::{Vector, VectorId, VectorMetadata};

#[derive(Debug, Clone)]
pub struct SymbolEmbedding {
    pub symbol_id: Uuid,
    pub vector_id: VectorId,
    pub embedding: Vector,
    pub metadata: SymbolMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMetadata {
    pub symbol: String,
    pub category: SymbolCategory,
    pub frequency: u32,
    pub variants: Vec<String>,
    pub created_at: i64,
}

impl From<SymbolEmbedding> for VectorMetadata {
    fn from(embedding: SymbolEmbedding) -> Self {
        VectorMetadata {
            id: embedding.vector_id,
            tags: vec![
                format!("category:{}", embedding.metadata.category.to_string()),
                format!("symbol:{}", embedding.metadata.symbol),
            ],
            ..Default::default()
        }
    }
}
```

#### 6.3.2 Pattern Cache

```rust
#[derive(Debug, Clone)]
pub struct PatternCache {
    pub patterns: HashMap<String, CachedPattern>,
    pub max_size: usize,
}

#[derive(Debug, Clone)]
pub struct CachedPattern {
    pub pattern: String,
    pub latex: String,
    pub confidence: f32,
    pub usage_count: u32,
    pub last_used: DateTime<Utc>,
}
```

---

## 7. Use Cases and User Stories

### 7.1 Academic Researcher

**User Story:**
> "As an academic researcher, I want to convert my handwritten mathematical derivations into LaTeX so that I can include them in my papers without retyping."

**Use Case UC-001: Handwritten Notes Conversion**

**Actor:** Academic Researcher

**Preconditions:**
- User has handwritten mathematical notes
- User has photographed or scanned the notes
- Image quality is sufficient (300+ DPI)

**Main Flow:**
1. User uploads image via API or web interface
2. System preprocesses image (deskew, denoise)
3. System detects mathematical regions
4. System recognizes handwritten symbols
5. System generates LaTeX code
6. System returns result with confidence scores
7. User reviews and makes corrections if needed
8. User exports to LaTeX document

**Postconditions:**
- LaTeX code generated
- Original image preserved
- Confidence scores provided

**Alternative Flows:**
- **3a.** Low confidence: System requests higher quality image
- **4a.** Ambiguous symbols: System provides alternatives
- **5a.** Complex layout: System segments into regions

**Acceptance Criteria:**
- [ ] 90%+ accuracy on handwritten math
- [ ] Processing time <5 seconds per page
- [ ] Confidence scores for all symbols
- [ ] Alternative suggestions for low-confidence symbols

### 7.2 Student

**User Story:**
> "As a student, I want to quickly digitize equations from my textbook so that I can solve them in Mathematica or WolframAlpha."

**Use Case UC-002: Textbook Equation Extraction**

**Actor:** Student

**Preconditions:**
- User has textbook with equations
- User can photograph equations clearly

**Main Flow:**
1. Student photographs equation with phone
2. Student uploads via mobile app or API
3. System recognizes printed equation
4. System generates multiple formats (LaTeX, AsciiMath, MathML)
5. Student copies format of choice
6. Student pastes into computational tool

**Postconditions:**
- Equation converted to multiple formats
- Copy-paste ready output

**Alternative Flows:**
- **3a.** Image quality issues: System requests retake
- **4a.** Multiple equations: System segments automatically

**Acceptance Criteria:**
- [ ] 95%+ accuracy on printed equations
- [ ] Processing time <2 seconds
- [ ] Support for inline and display equations
- [ ] Output compatible with major math tools

### 7.3 Publisher

**User Story:**
> "As a publisher, I want to convert legacy mathematical documents to modern formats so that we can create accessible digital editions."

**Use Case UC-003: Legacy Document Conversion**

**Actor:** Publisher

**Preconditions:**
- Publisher has scanned PDFs of legacy documents
- Documents contain mathematical content
- OCR text layer may be absent or poor quality

**Main Flow:**
1. Publisher uploads PDF document
2. System processes pages in parallel
3. System extracts text and math separately
4. System generates Scipix Markdown (MMD)
5. System generates accessible HTML with MathML
6. Publisher reviews and exports final format

**Postconditions:**
- Document converted to multiple formats
- Accessibility standards met (WCAG 2.1)
- Mathematical content preserved

**Alternative Flows:**
- **2a.** Large document: System provides progress updates
- **3a.** Complex layouts: System preserves structure
- **4a.** Tables and diagrams: System maintains formatting

**Acceptance Criteria:**
- [ ] Process 100-page document in <10 minutes
- [ ] Preserve document structure (headings, lists, etc.)
- [ ] Generate accessible output (WCAG 2.1 AA)
- [ ] Support for tables and diagrams

### 7.4 Developer

**User Story:**
> "As a developer, I want to integrate math OCR into my educational app so that students can solve problems by taking photos."

**Use Case UC-004: API Integration**

**Actor:** Application Developer

**Preconditions:**
- Developer has API credentials
- Developer's app can capture images
- Developer can make HTTP requests

**Main Flow:**
1. Developer reads API documentation
2. Developer implements authentication
3. Developer captures image in app
4. Developer sends image to API
5. API returns recognition results
6. Developer displays results in app
7. Developer implements error handling

**Postconditions:**
- Math OCR integrated into app
- Users can recognize equations
- Errors handled gracefully

**Alternative Flows:**
- **4a.** Rate limit exceeded: Developer implements backoff
- **5a.** Low confidence: Developer requests user verification
- **6a.** Network error: Developer shows offline message

**Acceptance Criteria:**
- [ ] Clear API documentation with examples
- [ ] SDKs for major languages (Python, JavaScript, etc.)
- [ ] Comprehensive error codes and messages
- [ ] Rate limiting with clear headers

### 7.5 Chemistry Student

**User Story:**
> "As a chemistry student, I want to digitize chemical equations from my lab notebook so that I can maintain a digital record."

**Use Case UC-005: Chemical Formula Recognition**

**Actor:** Chemistry Student

**Preconditions:**
- Student has lab notebook with chemical formulas
- Formulas include subscripts, superscripts, arrows

**Main Flow:**
1. Student photographs chemical equation
2. System recognizes chemical notation
3. System generates LaTeX (mhchem package)
4. System generates SMILES notation
5. Student exports to digital lab notebook

**Postconditions:**
- Chemical equation digitized
- Multiple output formats available

**Alternative Flows:**
- **2a.** Complex structural formula: System generates SVG
- **3a.** Reaction mechanism: System preserves arrows and conditions

**Acceptance Criteria:**
- [ ] 93%+ accuracy on chemical formulas
- [ ] Support for subscripts and superscripts
- [ ] Recognize reaction arrows and conditions
- [ ] Generate SMILES for molecules

---

## 8. Success Criteria and Acceptance Tests

### 8.1 Performance Benchmarks

#### Test Suite 1: Latency Benchmarks

```rust
#[cfg(test)]
mod latency_tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_single_image_p50_latency() {
        let processor = MathProcessor::new();
        let image = load_test_image("simple_equation.png");

        let mut measurements = vec![];
        for _ in 0..100 {
            let start = Instant::now();
            let _ = processor.process(&image).await.unwrap();
            measurements.push(start.elapsed());
        }

        measurements.sort();
        let p50 = measurements[50];

        assert!(
            p50 < Duration::from_millis(50),
            "P50 latency {} exceeds 50ms target",
            p50.as_millis()
        );
    }

    #[tokio::test]
    async fn test_single_image_p95_latency() {
        let processor = MathProcessor::new();
        let image = load_test_image("complex_equation.png");

        let mut measurements = vec![];
        for _ in 0..100 {
            let start = Instant::now();
            let _ = processor.process(&image).await.unwrap();
            measurements.push(start.elapsed());
        }

        measurements.sort();
        let p95 = measurements[95];

        assert!(
            p95 < Duration::from_millis(100),
            "P95 latency {} exceeds 100ms target",
            p95.as_millis()
        );
    }

    #[tokio::test]
    async fn test_batch_processing_time() {
        let processor = MathProcessor::new();
        let images: Vec<_> = (0..100)
            .map(|i| load_test_image(&format!("equation_{}.png", i)))
            .collect();

        let start = Instant::now();
        let results = processor.process_batch(&images).await.unwrap();
        let duration = start.elapsed();

        assert_eq!(results.len(), 100);
        assert!(
            duration < Duration::from_secs(5),
            "Batch processing took {}s, exceeds 5s target",
            duration.as_secs()
        );
    }
}
```

#### Test Suite 2: Accuracy Benchmarks

```rust
#[cfg(test)]
mod accuracy_tests {
    use super::*;

    #[tokio::test]
    async fn test_printed_math_accuracy() {
        let processor = MathProcessor::new();
        let test_dataset = load_dataset("printed_math_benchmark");

        let mut total_cer = 0.0;
        let mut count = 0;

        for (image, ground_truth) in test_dataset.iter() {
            let result = processor.process(image).await.unwrap();
            let cer = calculate_character_error_rate(&result.latex, ground_truth);
            total_cer += cer;
            count += 1;
        }

        let avg_cer = total_cer / count as f32;
        let accuracy = 1.0 - avg_cer;

        assert!(
            accuracy >= 0.95,
            "Printed math accuracy {:.2}% is below 95% target",
            accuracy * 100.0
        );
    }

    #[tokio::test]
    async fn test_handwritten_math_accuracy() {
        let processor = MathProcessor::new();
        let test_dataset = load_dataset("crohme_2019");

        let mut correct = 0;
        let mut total = 0;

        for (strokes, ground_truth) in test_dataset.iter() {
            let result = processor.process_strokes(strokes).await.unwrap();
            if normalize_latex(&result.latex) == normalize_latex(ground_truth) {
                correct += 1;
            }
            total += 1;
        }

        let accuracy = correct as f32 / total as f32;

        assert!(
            accuracy >= 0.90,
            "Handwritten math accuracy {:.2}% is below 90% target",
            accuracy * 100.0
        );
    }

    #[tokio::test]
    async fn test_chemical_formula_accuracy() {
        let processor = MathProcessor::new();
        let test_dataset = load_dataset("chemistry_formulas");

        let mut correct = 0;
        let mut total = 0;

        for (image, ground_truth) in test_dataset.iter() {
            let result = processor.process(image).await.unwrap();
            if result.latex == ground_truth.latex {
                correct += 1;
            }
            total += 1;
        }

        let accuracy = correct as f32 / total as f32;

        assert!(
            accuracy >= 0.93,
            "Chemical formula accuracy {:.2}% is below 93% target",
            accuracy * 100.0
        );
    }
}
```

#### Test Suite 3: Scalability Tests

```rust
#[cfg(test)]
mod scalability_tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrent_requests() {
        let processor = Arc::new(MathProcessor::new());
        let mut handles = vec![];

        for i in 0..1000 {
            let processor = processor.clone();
            let handle = tokio::spawn(async move {
                let image = generate_test_image(i);
                processor.process(&image).await
            });
            handles.push(handle);
        }

        let results: Vec<_> = futures::future::join_all(handles)
            .await
            .into_iter()
            .collect();

        let success_count = results.iter().filter(|r| r.is_ok()).count();
        let success_rate = success_count as f32 / 1000.0;

        assert!(
            success_rate >= 0.99,
            "Success rate {:.2}% below 99% target",
            success_rate * 100.0
        );
    }

    #[tokio::test]
    async fn test_memory_usage() {
        let processor = MathProcessor::new();

        let initial_memory = get_memory_usage();

        // Process 1000 images
        for i in 0..1000 {
            let image = generate_test_image(i);
            let _ = processor.process(&image).await.unwrap();
        }

        let final_memory = get_memory_usage();
        let memory_increase = final_memory - initial_memory;

        assert!(
            memory_increase < 2_000_000_000, // 2GB
            "Memory usage increased by {} bytes, exceeds 2GB limit",
            memory_increase
        );
    }
}
```

### 8.2 API Compatibility Tests

```rust
#[cfg(test)]
mod api_compatibility_tests {
    use super::*;

    #[tokio::test]
    async fn test_scipix_api_request_format() {
        let client = TestClient::new();

        let request = json!({
            "src": "data:image/png;base64,...",
            "formats": ["latex", "mathml"],
            "ocr": ["math", "text"]
        });

        let response = client
            .post("/v3/text")
            .json(&request)
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);

        let body: serde_json::Value = response.json().await.unwrap();
        assert!(body.get("latex").is_some());
        assert!(body.get("mathml").is_some());
        assert!(body.get("confidence").is_some());
    }

    #[tokio::test]
    async fn test_error_response_format() {
        let client = TestClient::new();

        let request = json!({
            "src": "invalid_data"
        });

        let response = client
            .post("/v3/text")
            .json(&request)
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 400);

        let body: ErrorResponse = response.json().await.unwrap();
        assert!(!body.error.is_empty());
        assert!(!body.message.is_empty());
    }
}
```

### 8.3 Acceptance Criteria Checklist

#### Functional Requirements
- [ ] Support all specified image formats (JPEG, PNG, GIF, TIFF, WebP, BMP)
- [ ] Process PDF documents (up to 100 pages)
- [ ] Recognize printed mathematical equations (95%+ accuracy)
- [ ] Recognize handwritten equations (90%+ accuracy)
- [ ] Recognize chemical formulas (93%+ accuracy)
- [ ] Generate LaTeX output
- [ ] Generate MathML output
- [ ] Generate Scipix Markdown
- [ ] Provide confidence scores
- [ ] Extract bounding boxes and geometry
- [ ] Segment lines and words
- [ ] Support batch processing

#### Non-Functional Requirements
- [ ] Single image latency <100ms (p95)
- [ ] Batch processing: 100 images in <5 seconds
- [ ] Support 1000+ concurrent users
- [ ] 99.9% uptime SLA
- [ ] Memory usage <2GB per worker
- [ ] Horizontal scaling to 10+ nodes

#### API Requirements
- [ ] RESTful API following OpenAPI 3.0
- [ ] API key authentication
- [ ] Rate limiting
- [ ] Comprehensive error messages
- [ ] API documentation with examples
- [ ] Compatible with Scipix API v3 (95%+)

#### Quality Requirements
- [ ] 80%+ test coverage
- [ ] No Clippy warnings
- [ ] Formatted with Rustfmt
- [ ] Documentation for all public APIs
- [ ] Structured logging with tracing
- [ ] Prometheus metrics

---

## 9. Constraints and Limitations

### 9.1 Technical Constraints

#### 9.1.1 Processing Limitations

**Image Size Constraints:**
```rust
pub const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB
pub const MIN_IMAGE_DIMENSION: u32 = 100;           // 100px
pub const MAX_IMAGE_DIMENSION: u32 = 4000;          // 4000px
pub const RECOMMENDED_DPI: u32 = 300;               // 300 DPI
```

**Performance Limitations:**
- Processing time increases with image size
- Complex equations may exceed 100ms target
- Very low quality images may fail recognition
- Batch processing limited to 1000 images per request

**Accuracy Limitations:**
- Handwritten accuracy depends on legibility
- Very stylized fonts may reduce accuracy
- Mixed languages in same equation may confuse recognition
- Structural formulas (chemistry) have limited support

#### 9.1.2 Format Limitations

**Input Formats:**
- SVG not supported (rasterize first)
- Animated GIFs (only first frame processed)
- HEIC/HEIF require conversion
- Password-protected PDFs require password

**Output Formats:**
- LaTeX: Requires standard packages (amsmath, amssymb)
- MathML: Version 3.0 only
- DOCX: Basic formatting only
- HTML: Requires MathJax or KaTeX for rendering

#### 9.1.3 Character Set Limitations

```rust
pub enum SupportLevel {
    Full,        // 95%+ accuracy
    Partial,     // 80-95% accuracy
    Limited,     // 60-80% accuracy
    Experimental, // <60% accuracy
}

pub const CHARACTER_SUPPORT: &[(CharacterSet, SupportLevel)] = &[
    (CharacterSet::BasicLatin, SupportLevel::Full),
    (CharacterSet::Greek, SupportLevel::Full),
    (CharacterSet::MathematicalOperators, SupportLevel::Full),
    (CharacterSet::Cyrillic, SupportLevel::Partial),
    (CharacterSet::Hebrew, SupportLevel::Limited),
    (CharacterSet::Arabic, SupportLevel::Limited),
    (CharacterSet::CJK, SupportLevel::Experimental),
];
```

### 9.2 Operational Constraints

#### 9.2.1 Resource Requirements

**Minimum Hardware:**
- CPU: 4 cores (2.0 GHz+)
- RAM: 8GB
- Storage: 20GB (including models)
- Network: 100 Mbps

**Recommended Hardware:**
- CPU: 8+ cores (3.0 GHz+)
- RAM: 16GB+
- Storage: 100GB SSD
- Network: 1 Gbps
- GPU: Optional (CUDA-capable for acceleration)

#### 9.2.2 Dependency Constraints

```toml
[dependencies]
# Core dependencies
ruvector-core = "0.3"        # Vector storage
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }

# Image processing
image = "0.24"
imageproc = "0.23"

# ML models (size constraints)
onnxruntime = "0.0.14"       # Model size: ~500MB
tensorflow = { version = "0.20", optional = true }  # Model size: ~1GB

# Document processing
pdf = "0.8"
lopdf = "0.26"
docx-rs = "0.4"

# Constraints
# - ONNX runtime: Prebuilt binaries required
# - TensorFlow: Optional, adds 1GB+ to binary
# - PDF libraries: Limited to PDF 1.7
```

#### 9.2.3 Compliance Constraints

**Privacy Requirements:**
- GDPR: No persistent storage of user data
- CCPA: User data deletion within 30 days
- HIPAA: Not certified (avoid medical documents)

**Accessibility Requirements:**
- WCAG 2.1 AA for HTML output
- Screen reader compatible MathML
- Alt text for all images

**License Constraints:**
- MIT/Apache-2.0 for core library
- Model licenses vary by source
- Dataset licenses must be respected

### 9.3 Design Constraints

#### 9.3.1 API Compatibility

**Must Maintain:**
- URL structure: `/v3/{endpoint}`
- Request/response formats
- Error codes and messages
- Authentication mechanism
- Rate limit headers

**May Differ:**
- Internal implementation
- Performance characteristics
- Additional features
- Model architectures

#### 9.3.2 Extensibility Requirements

```rust
// Plugin architecture for custom models
pub trait RecognitionModel: Send + Sync {
    fn recognize(&self, image: &Image) -> Result<Recognition>;
    fn model_info(&self) -> ModelInfo;
}

// Hook system for preprocessing
pub trait PreprocessingHook: Send + Sync {
    fn process(&self, image: Image) -> Result<Image>;
    fn priority(&self) -> i32;
}

// Custom output formatters
pub trait OutputFormatter: Send + Sync {
    fn format(&self, recognition: &Recognition) -> Result<String>;
    fn mime_type(&self) -> &str;
}
```

#### 9.3.3 Scalability Constraints

**Vertical Scaling:**
- Limited by single-machine resources
- Model size limits memory scaling
- CPU-bound processing limits throughput

**Horizontal Scaling:**
- Stateless design required
- Shared storage for models
- Coordinated caching strategy
- Load balancer required

---

## 10. Dependencies

### 10.1 Core Dependencies

#### 10.1.1 ruvector-core Integration

**Purpose:** Vector storage for symbol embeddings and pattern matching

```rust
use ruvector_core::{
    VectorDatabase, Vector, VectorId, VectorMetadata,
    SearchOptions, SearchResult,
};

pub struct SymbolDatabase {
    db: VectorDatabase,
}

impl SymbolDatabase {
    pub async fn new(path: &str) -> Result<Self> {
        let db = VectorDatabase::open(path).await?;
        Ok(Self { db })
    }

    pub async fn find_similar_symbols(
        &self,
        embedding: &Vector,
        limit: usize,
    ) -> Result<Vec<SymbolMatch>> {
        let options = SearchOptions {
            limit,
            threshold: 0.8,
            ..Default::default()
        };

        let results = self.db.search(embedding, &options).await?;

        Ok(results
            .into_iter()
            .map(|r| SymbolMatch {
                symbol: r.metadata.get("symbol").unwrap().to_string(),
                confidence: r.score,
            })
            .collect())
    }

    pub async fn add_symbol(
        &self,
        symbol: &str,
        embedding: Vector,
        metadata: SymbolMetadata,
    ) -> Result<VectorId> {
        let vector_metadata = VectorMetadata {
            tags: vec![
                format!("symbol:{}", symbol),
                format!("category:{}", metadata.category.to_string()),
            ],
            ..Default::default()
        };

        self.db.insert(embedding, vector_metadata).await
    }
}
```

**Use Cases:**
- Symbol recognition via nearest neighbor search
- Pattern matching for common equations
- Caching of recognized expressions
- Similarity-based error correction

**Performance Requirements:**
- Search latency: <10ms for 1M vectors
- Insert throughput: 10,000+ vectors/sec
- Memory efficiency: Quantization support
- Horizontal scaling: Distributed mode

#### 10.1.2 Machine Learning Models

**Symbol Recognition Model:**
```rust
pub struct SymbolRecognitionModel {
    session: onnxruntime::Session,
    embedder: Embedder,
    symbol_db: SymbolDatabase,
}

impl SymbolRecognitionModel {
    pub fn load(model_path: &str, symbol_db: SymbolDatabase) -> Result<Self> {
        let session = onnxruntime::SessionBuilder::new()?
            .with_model_from_file(model_path)?;

        let embedder = Embedder::new(embedding_dim: 512);

        Ok(Self { session, embedder, symbol_db })
    }

    pub async fn recognize(&self, image: &Image) -> Result<Vec<Symbol>> {
        // 1. Extract symbol regions
        let regions = self.detect_symbols(image)?;

        // 2. Generate embeddings
        let embeddings: Vec<_> = regions
            .iter()
            .map(|r| self.embedder.embed(r))
            .collect();

        // 3. Search in vector database
        let mut symbols = vec![];
        for (region, embedding) in regions.iter().zip(embeddings.iter()) {
            let matches = self.symbol_db
                .find_similar_symbols(embedding, 5)
                .await?;

            symbols.push(Symbol {
                bounding_box: region.bbox,
                symbol: matches[0].symbol.clone(),
                confidence: matches[0].confidence,
                alternatives: matches[1..].to_vec(),
            });
        }

        Ok(symbols)
    }
}
```

**Model Requirements:**
- Format: ONNX Runtime compatible
- Size: <500MB per model
- Quantization: INT8 support for deployment
- Input: 224x224 RGB images (normalized)
- Output: 512-dimensional embeddings

#### 10.1.3 Image Processing

**Dependencies:**
```toml
[dependencies]
image = "0.24"           # Image loading/saving
imageproc = "0.23"       # Image processing primitives
fast_image_resize = "2.7" # High-performance resizing
```

**Processing Pipeline:**
```rust
pub struct ImagePreprocessor {
    config: PreprocessingConfig,
}

impl ImagePreprocessor {
    pub fn preprocess(&self, image: DynamicImage) -> Result<ProcessedImage> {
        let mut img = image;

        // 1. Deskew
        if self.config.deskew {
            img = self.deskew_image(img)?;
        }

        // 2. Denoise
        if self.config.denoise {
            img = self.apply_bilateral_filter(img)?;
        }

        // 3. Binarize
        if self.config.binarize {
            img = self.adaptive_threshold(img)?;
        }

        // 4. Enhance contrast
        if self.config.enhance_contrast {
            img = self.enhance_contrast(img)?;
        }

        Ok(ProcessedImage { image: img })
    }
}
```

### 10.2 External Dependencies

#### 10.2.1 Document Processing

**PDF Processing:**
```toml
pdf = "0.8"              # PDF parsing
lopdf = "0.26"           # Low-level PDF operations
pdfium-render = "0.7"    # PDF rendering
```

**DOCX Processing:**
```toml
docx-rs = "0.4"          # DOCX reading/writing
zip = "0.6"              # DOCX is ZIP-based
```

#### 10.2.2 Web Framework

```toml
axum = "0.6"             # Web framework
tower = "0.4"            # Middleware
tower-http = "0.4"       # HTTP middleware
```

**API Server:**
```rust
use axum::{
    routing::{post, get},
    Router, Json, extract::State,
};

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/v3/text", post(text_recognition_handler))
        .route("/v3/strokes", post(stroke_recognition_handler))
        .route("/v3/latex", post(latex_render_handler))
        .route("/v3/pdf", post(pdf_processing_handler))
        .route("/health", get(health_check))
        .layer(/* authentication middleware */)
        .layer(/* rate limiting middleware */)
        .layer(/* logging middleware */)
        .with_state(state)
}
```

### 10.3 Development Dependencies

```toml
[dev-dependencies]
criterion = "0.5"        # Benchmarking
proptest = "1.0"         # Property testing
mockall = "0.11"         # Mocking
tokio-test = "0.4"       # Async testing
insta = "1.26"           # Snapshot testing
```

### 10.4 Dependency Version Matrix

| Dependency | Minimum Version | Recommended | Notes |
|-----------|----------------|-------------|-------|
| ruvector-core | 0.3.0 | 0.3.x | Vector storage |
| tokio | 1.0 | 1.35+ | Async runtime |
| axum | 0.6 | 0.7+ | Web framework |
| onnxruntime | 0.0.14 | latest | ML inference |
| image | 0.24 | 0.24+ | Image processing |
| pdf | 0.8 | 0.8+ | PDF parsing |

### 10.5 Build Requirements

**System Dependencies:**
```bash
# Ubuntu/Debian
apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    cmake

# macOS
brew install cmake openssl
```

**Rust Toolchain:**
```bash
rustc >= 1.70.0
cargo >= 1.70.0
```

---

## Appendix A: Glossary

**AsciiMath:** Simplified mathematical notation for web

**Bounding Box:** Rectangle enclosing a detected object

**CER (Character Error Rate):** Metric for OCR accuracy

**CROHME:** Competition on Recognition of Online Handwritten Mathematical Expressions

**LaTeX:** Document preparation system for technical content

**MathML:** Mathematical Markup Language (XML-based)

**Scipix Markdown (MMD):** Extended Markdown with math support

**OCR:** Optical Character Recognition

**ONNX:** Open Neural Network Exchange format

**Quantization:** Reducing model precision to save memory

**SMILES:** Simplified Molecular Input Line Entry System

**Stroke:** Continuous pen/stylus movement

**Vector Embedding:** Dense numerical representation of data

---

## Appendix B: References

1. **Scipix API Documentation**
   - https://docs.scipix.com/

2. **CROHME Dataset**
   - https://www.isical.ac.in/~crohme/

3. **OpenAPI Specification 3.0**
   - https://swagger.io/specification/

4. **WCAG 2.1 Guidelines**
   - https://www.w3.org/WAI/WCAG21/quickref/

5. **LaTeX Documentation**
   - https://www.latex-project.org/help/documentation/

6. **MathML Specification**
   - https://www.w3.org/TR/MathML3/

7. **ruvector-core Documentation**
   - https://github.com/ruvnet/ruvector

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-11-28 | SPARC Agent | Initial specification |

---

**Next Phase:** [02_PSEUDOCODE.md](./02_PSEUDOCODE.md) - Algorithm design and processing pipelines
