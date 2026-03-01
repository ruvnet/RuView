# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2024-11-28

### Added

#### Core Features
- **Mathematical OCR Engine**: Complete implementation of OCR for mathematical equations and expressions
- **Vector-Based Caching**: Intelligent caching using ruvector-core for image embeddings and similarity search
- **Multi-Format Output**: Support for LaTeX, MathML, AsciiMath, SMILES, HTML, DOCX, JSON, and MMD formats
- **Image Preprocessing Pipeline**: Advanced image enhancement, deskewing, rotation correction, and segmentation
- **Configuration Management**: Flexible TOML-based configuration with presets (default, high-accuracy, high-speed)

#### API Server
- **REST API Implementation**: Scipix v3 API compatible endpoints
  - `/v3/text` - Image OCR processing (multipart/base64/URL)
  - `/v3/strokes` - Digital ink recognition
  - `/v3/pdf` - Async PDF processing with job queue
  - `/v3/latex` - Legacy equation recognition
  - `/v3/converter` - Document format conversion
  - `/health` - Health check endpoint
- **Production-Ready Middleware**:
  - Authentication (app_id/app_key validation)
  - Token bucket rate limiting (100 req/min default)
  - Request tracing and structured logging
  - CORS support with configurable origins
  - Gzip compression for responses
- **Async Job Queue**: Background processing for PDF jobs with status tracking and webhook callbacks
- **Result Caching**: Moka-based async caching with TTL
- **Graceful Shutdown**: Proper resource cleanup on termination

#### WebAssembly Support
- **Browser-Based OCR**: Process images directly in the browser
- **Web Worker Support**: Off-main-thread processing with progress reporting
- **Multiple Input Formats**: File, Canvas, Base64, URL support
- **Optimized Bundle**: <2MB compressed size with efficient memory management
- **TypeScript Definitions**: Full type safety for JavaScript/TypeScript projects

#### CLI Tool
- **Interactive Commands**:
  - `ocr` - Process single or batch images
  - `serve` - Start API server
  - `batch` - Process multiple images in parallel
  - `config` - Manage configuration files
- **Rich Terminal UI**: Progress bars, colored output, and interactive tables
- **Shell Completions**: Support for bash, zsh, fish, and PowerShell

#### Performance Optimizations
- **SIMD Acceleration**: Vectorized operations for image processing
- **Parallel Processing**: Multi-threaded batch processing with rayon
- **Memory Optimization**: Efficient memory pooling and buffer reuse
- **Quantization Support**: Model quantization for reduced memory footprint
- **Batch Inference**: Optimized batch processing for throughput

#### Math Processing
- **LaTeX Parser**: Complete LaTeX to AST parsing with error recovery
- **MathML Generation**: AST to MathML conversion with proper semantics
- **AsciiMath Support**: AsciiMath parsing and conversion
- **Symbol Library**: Comprehensive mathematical symbol database
- **Format Conversion**: Convert between LaTeX, MathML, and AsciiMath

#### Developer Experience
- **Comprehensive Documentation**: 15+ detailed documentation files covering:
  - Architecture and design decisions
  - OCR research and algorithms
  - Rust ecosystem integration
  - Testing strategies
  - Security best practices
  - Optimization techniques
  - WASM implementation guide
  - Lean/Agentic integration roadmap
- **Example Programs**: 7 example applications demonstrating different use cases
- **Integration Tests**: Comprehensive test suite with >90% coverage target
- **Benchmarks**: Performance benchmarks using Criterion
- **Type Safety**: Strong typing throughout with comprehensive error handling

### Technical Details

#### Architecture
- **Modular Design**: Clean separation of concerns with feature flags
- **Feature Flags**:
  - `default` - Core functionality with preprocessing, caching, and optimization
  - `preprocess` - Image preprocessing pipeline
  - `cache` - Vector-based caching
  - `ocr` - OCR engine (requires ONNX models)
  - `math` - Mathematical parsing and conversion
  - `optimize` - Performance optimizations
  - `wasm` - WebAssembly bindings

#### Dependencies
- **Core**: ruvector-core, image, imageproc, serde, tokio
- **ML**: ort (ONNX Runtime) for model inference
- **Web**: axum, tower, tower-http for REST API
- **CLI**: clap, indicatif, console for command-line interface
- **Math**: nom for parsing, nalgebra for linear algebra
- **Performance**: rayon, memmap2, SIMD intrinsics
- **Testing**: criterion, proptest, mockall

#### Performance Benchmarks
- **OCR Throughput**: Target >100 images/second (batch mode)
- **API Latency**: <100ms for typical equations (cached)
- **Memory Usage**: <500MB baseline, <2GB peak
- **Cache Hit Rate**: >80% for similar equations
- **WASM Bundle**: <2MB compressed, <5MB uncompressed

### Known Limitations

- **ONNX Models**: Models not included in repository (must be downloaded separately)
- **GPU Support**: ONNX Runtime CPU-only (GPU support planned)
- **Language Support**: English and mathematical notation only
- **Handwriting**: Limited handwriting recognition (digital ink only)
- **Complex Layouts**: Advanced layout analysis planned for future releases
- **Database**: No persistent storage yet (planned for 0.2.0)

### Security

- **Input Validation**: Comprehensive validation using validator crate
- **Rate Limiting**: Default 100 req/min per client
- **Authentication**: Required for all API endpoints (except health)
- **No Secrets**: Environment variables for all credentials
- **CORS**: Configurable allowed origins
- **Size Limits**: Configurable max request/file sizes

### Breaking Changes

None (initial release)

### Migration Guide

This is the initial release. No migration required.

### Future Roadmap

#### Version 0.2.0 (Q1 2025)
- [ ] Database persistence (PostgreSQL/SQLite)
- [ ] Horizontal scaling with Redis
- [ ] Prometheus metrics
- [ ] OpenAPI/Swagger documentation
- [ ] Multi-tenancy support

#### Version 0.3.0 (Q2 2025)
- [ ] GPU acceleration via ONNX Runtime
- [ ] Advanced layout analysis
- [ ] Multi-language support
- [ ] Enhanced handwriting recognition
- [ ] Real-time collaborative editing

#### Version 1.0.0 (Q3 2025)
- [ ] Production-grade stability
- [ ] Enterprise features
- [ ] Cloud-native deployment
- [ ] Kubernetes operators
- [ ] Comprehensive monitoring

### Contributors

- Ruvector Team - Initial implementation and architecture
- Community - Testing and feedback

### License

MIT License - See LICENSE file for details

---

## Unreleased

### Added
- Nothing yet

### Changed
- Nothing yet

### Fixed
- Nothing yet

### Deprecated
- Nothing yet

### Removed
- Nothing yet

### Security
- Nothing yet
