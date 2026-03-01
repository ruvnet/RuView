# Ruvector Integration Architecture
## ruvector-scipix Integration Design

**Version:** 1.0.0
**Date:** 2025-11-28
**Status:** Design Phase

---

## Executive Summary

This document defines the integration architecture for `ruvector-scipix`, a specialized OCR crate for mathematical expressions, with the existing ruvector ecosystem. The integration leverages ruvector's high-performance vector database, HNSW indexing, distributed clustering, and WASM capabilities to provide scalable, intelligent mathematical OCR processing.

**Key Integration Points:**
- Vector-based caching of OCR results using ruvector-core
- REST API endpoints via ruvector-server extension
- Browser-based OCR using ruvector-wasm
- Distributed processing with ruvector-cluster
- Performance tracking via ruvector-metrics
- Shared configuration and error handling patterns

---

## 1. Workspace Integration

### 1.1 Adding to Workspace Members

**Root Cargo.toml Modification:**

```toml
[workspace]
members = [
    # ... existing members ...
    "crates/ruvector-gnn-wasm",

    # Scipix Integration - NEW
    "crates/ruvector-scipix-core",      # Core OCR logic
    "crates/ruvector-scipix-node",      # Node.js bindings
    "crates/ruvector-scipix-wasm",      # Browser WASM
    "crates/ruvector-scipix-server",    # HTTP server extension

    "examples/refrag-pipeline",
    "examples/scipix",                   # Examples and demos
]

[workspace.dependencies]
# ... existing dependencies ...

# Scipix-specific dependencies - NEW
reqwest = { version = "0.12", features = ["json", "multipart"] }
base64 = "0.22"
image = { version = "0.25", features = ["png", "jpeg"] }
tesseract-rs = { version = "0.14", optional = true }  # Local fallback
pdf-extract = { version = "0.7", optional = true }
```

**Dependency Version Strategy:**
- Use `version = "0.1.16"` (workspace version) for internal crates
- Use `workspace = true` for shared dependencies
- Add scipix-specific deps to workspace.dependencies for consistency

### 1.2 Crate Structure

```
crates/
├── ruvector-scipix-core/      # Core OCR engine
│   ├── src/
│   │   ├── lib.rs              # Public API
│   │   ├── api_client.rs       # Scipix API client
│   │   ├── ocr_engine.rs       # OCR processing
│   │   ├── cache.rs            # Vector-based cache
│   │   ├── preprocessing.rs    # Image preprocessing
│   │   ├── postprocessing.rs   # LaTeX refinement
│   │   └── error.rs            # Error types
│   └── Cargo.toml
│
├── ruvector-scipix-node/      # Node.js bindings (NAPI-RS)
│   ├── src/
│   │   └── lib.rs
│   ├── npm/                    # Platform binaries
│   └── Cargo.toml
│
├── ruvector-scipix-wasm/      # WASM bindings
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
│
└── ruvector-scipix-server/    # Server extension
    ├── src/
    │   ├── main.rs
    │   ├── routes.rs
    │   └── middleware.rs
    └── Cargo.toml

examples/scipix/               # Examples (NOT workspace member)
├── src/
├── tests/
├── docs/
└── Cargo.toml                  # Standalone example
```

### 1.3 Feature Flags Strategy

**Core Crate (ruvector-scipix-core):**

```toml
[features]
default = ["api-client", "cache", "simd"]

# Backend features
api-client = ["reqwest", "base64"]
tesseract = ["tesseract-rs"]        # Local OCR fallback
pdf-support = ["pdf-extract"]

# Performance features
cache = ["ruvector-core/storage"]   # Vector cache
simd = ["ruvector-core/simd"]       # SIMD optimizations
quantization = ["ruvector-core"]    # Quantized embeddings

# Environment features
wasm = []                           # WASM-compatible mode
memory-only = []                    # No file I/O
```

---

## 2. ruvector-core Usage

### 2.1 Storing Math Expression Embeddings

**Integration Pattern:**

```rust
// crates/ruvector-scipix-core/src/cache.rs

use ruvector_core::{VectorDB, VectorEntry, DistanceMetric, SearchQuery};
use std::path::Path;

/// OCR result cache using vector similarity
pub struct ScipixCache {
    /// Vector database for image embeddings
    image_db: VectorDB,
    /// Vector database for LaTeX embeddings
    latex_db: VectorDB,
    /// Embedding dimension
    dimension: usize,
}

impl ScipixCache {
    /// Create new cache with specified dimension
    pub fn new(cache_dir: &Path, dimension: usize) -> Result<Self> {
        let image_path = cache_dir.join("image_vectors.db");
        let latex_path = cache_dir.join("latex_vectors.db");

        Ok(Self {
            image_db: VectorDB::new(
                &image_path,
                dimension,
                DistanceMetric::Cosine,
            )?,
            latex_db: VectorDB::new(
                &latex_path,
                dimension,
                DistanceMetric::Cosine,
            )?,
            dimension,
        })
    }

    /// Store OCR result with image embedding
    pub fn store_result(
        &mut self,
        image_embedding: Vec<f32>,
        latex: String,
        confidence: f32,
    ) -> Result<uuid::Uuid> {
        // Store image embedding
        let id = uuid::Uuid::new_v4();
        self.image_db.add_vector(
            id,
            image_embedding.clone(),
            Some(serde_json::json!({
                "latex": latex,
                "confidence": confidence,
                "timestamp": chrono::Utc::now(),
            })),
        )?;

        // Also store LaTeX embedding for semantic search
        let latex_embedding = self.encode_latex(&latex)?;
        self.latex_db.add_vector(id, latex_embedding, None)?;

        Ok(id)
    }

    /// Find similar cached results
    pub fn find_similar(
        &self,
        image_embedding: Vec<f32>,
        threshold: f32,
    ) -> Result<Option<CachedResult>> {
        let query = SearchQuery::new(image_embedding)
            .with_k(1)
            .with_ef(50);

        let results = self.image_db.search(&query)?;

        if let Some(result) = results.first() {
            if result.distance <= threshold {
                let metadata = result.metadata.as_ref()
                    .ok_or(RuvectorError::MetadataMissing)?;

                return Ok(Some(CachedResult {
                    latex: metadata["latex"].as_str().unwrap().to_string(),
                    confidence: metadata["confidence"].as_f64().unwrap() as f32,
                    distance: result.distance,
                }));
            }
        }

        Ok(None)
    }

    /// Encode LaTeX to vector using simple hashing
    fn encode_latex(&self, latex: &str) -> Result<Vec<f32>> {
        // Use TF-IDF or learned embeddings
        // For now, simple character n-gram hashing
        let mut embedding = vec![0.0; self.dimension];

        for ngram in latex.chars().collect::<Vec<_>>().windows(3) {
            let hash = ngram.iter().fold(0u64, |acc, &c| {
                acc.wrapping_mul(31).wrapping_add(c as u64)
            });
            let idx = (hash % self.dimension as u64) as usize;
            embedding[idx] += 1.0;
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|&x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            embedding.iter_mut().for_each(|x| *x /= norm);
        }

        Ok(embedding)
    }
}

#[derive(Debug, Clone)]
pub struct CachedResult {
    pub latex: String,
    pub confidence: f32,
    pub distance: f32,
}
```

### 2.2 Quantization for Memory Efficiency

```rust
use ruvector_core::quantization::{ScalarQuantizer, QuantizationConfig};

impl ScipixCache {
    /// Create cache with quantization (4-32x memory reduction)
    pub fn new_quantized(
        cache_dir: &Path,
        dimension: usize,
        bits: u8,  // 4 or 8
    ) -> Result<Self> {
        let config = QuantizationConfig {
            bits,
            ..Default::default()
        };

        // Quantizer will be used internally by VectorDB
        let mut cache = Self::new(cache_dir, dimension)?;
        cache.image_db.enable_quantization(config)?;

        Ok(cache)
    }
}
```

### 2.3 HNSW Parameters for OCR Cache

```rust
use ruvector_core::index::HNSWConfig;

impl ScipixCache {
    /// Optimize HNSW for OCR workload
    pub fn with_hnsw_config(mut self, config: HNSWConfig) -> Self {
        // Typical OCR workload:
        // - High recall needed (mathematical expressions must be accurate)
        // - Moderate write throughput
        // - Low latency reads

        let optimized = HNSWConfig {
            m: 32,              // Connections per layer (higher = better recall)
            ef_construction: 200, // Construction effort
            max_elements: 100_000, // Expected cache size
            ..Default::default()
        };

        self.image_db.configure_hnsw(optimized);
        self
    }
}
```

---

## 3. ruvector-server Extension

### 3.1 Server Crate Structure

**crates/ruvector-scipix-server/Cargo.toml:**

```toml
[package]
name = "ruvector-scipix-server"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "HTTP server for Scipix OCR with vector caching"

[dependencies]
# Core dependencies
ruvector-core = { version = "0.1.16", path = "../ruvector-core" }
ruvector-server = { version = "0.1.16", path = "../ruvector-server" }
ruvector-scipix-core = { version = "0.1.16", path = "../ruvector-scipix-core" }

# Web framework
axum = { version = "0.7", features = ["json", "multipart"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace", "limit"] }

# Async runtime
tokio = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }

# Error handling
thiserror = { workspace = true }
anyhow = { workspace = true }

# Utilities
tracing = { workspace = true }
uuid = { workspace = true }
base64 = { workspace = true }

[features]
default = ["api-client"]
api-client = ["ruvector-scipix-core/api-client"]
metrics = ["ruvector-metrics"]
```

### 3.2 REST API Endpoints

**crates/ruvector-scipix-server/src/routes.rs:**

```rust
use axum::{
    Router,
    routing::{post, get},
    extract::{State, Multipart},
    Json,
    http::StatusCode,
};
use ruvector_scipix_core::{ScipixClient, ScipixCache};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub scipix_client: Arc<ScipixClient>,
    pub cache: Arc<parking_lot::RwLock<ScipixCache>>,
}

/// Create Scipix routes
pub fn scipix_routes() -> Router<AppState> {
    Router::new()
        // Scipix API v3 endpoints
        .route("/v3/text", post(ocr_text))
        .route("/v3/pdf", post(ocr_pdf))
        .route("/v3/batch", post(ocr_batch))

        // Cache management
        .route("/cache/stats", get(cache_stats))
        .route("/cache/search", post(search_cache))
        .route("/cache/clear", post(clear_cache))
}

/// POST /v3/text - OCR text from image
async fn ocr_text(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<OcrResponse>, AppError> {
    let mut image_data = Vec::new();

    // Extract image from multipart
    while let Some(field) = multipart.next_field().await? {
        if field.name() == Some("image") {
            image_data = field.bytes().await?.to_vec();
        }
    }

    // Generate image embedding for cache lookup
    let embedding = state.scipix_client
        .generate_image_embedding(&image_data)?;

    // Check cache first
    if let Some(cached) = state.cache.read()
        .find_similar(embedding.clone(), 0.95)? {
        return Ok(Json(OcrResponse {
            latex: cached.latex,
            confidence: cached.confidence,
            cached: true,
        }));
    }

    // Cache miss - call Scipix API
    let result = state.scipix_client.ocr_image(&image_data).await?;

    // Store in cache
    state.cache.write().store_result(
        embedding,
        result.latex.clone(),
        result.confidence,
    )?;

    Ok(Json(OcrResponse {
        latex: result.latex,
        confidence: result.confidence,
        cached: false,
    }))
}

/// POST /v3/pdf - OCR entire PDF
async fn ocr_pdf(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<PdfOcrResponse>, AppError> {
    let mut pdf_data = Vec::new();

    while let Some(field) = multipart.next_field().await? {
        if field.name() == Some("pdf") {
            pdf_data = field.bytes().await?.to_vec();
        }
    }

    // Extract pages and process in parallel
    let pages = state.scipix_client.extract_pdf_pages(&pdf_data)?;
    let results = futures::future::join_all(
        pages.into_iter().map(|page| {
            let client = state.scipix_client.clone();
            async move { client.ocr_image(&page).await }
        })
    ).await;

    let pages: Vec<_> = results.into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(PdfOcrResponse { pages }))
}

#[derive(serde::Serialize)]
struct OcrResponse {
    latex: String,
    confidence: f32,
    cached: bool,
}

#[derive(serde::Serialize)]
struct PdfOcrResponse {
    pages: Vec<PageResult>,
}

#[derive(serde::Serialize)]
struct PageResult {
    page_num: usize,
    latex: String,
    confidence: f32,
}
```

### 3.3 Authentication Integration

```rust
use axum::{
    extract::Request,
    middleware::Next,
    http::StatusCode,
};

/// API key authentication middleware
pub async fn auth_middleware(
    mut req: Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    let auth_header = req.headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(key) if validate_api_key(key) => {
            // Store user context in extensions
            req.extensions_mut().insert(ApiUser {
                key: key.to_string(),
            });
            Ok(next.run(req).await)
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

fn validate_api_key(key: &str) -> bool {
    // Check against database or environment
    std::env::var("MATHPIX_API_KEY")
        .map(|k| k == key)
        .unwrap_or(false)
}
```

### 3.4 Rate Limiting

```rust
use tower::ServiceBuilder;
use tower_http::limit::RequestBodyLimitLayer;

pub fn create_server(state: AppState) -> Router {
    Router::new()
        .merge(scipix_routes())
        .layer(
            ServiceBuilder::new()
                // Rate limiting (100 req/min per IP)
                .layer(tower_http::timeout::TimeoutLayer::new(
                    std::time::Duration::from_secs(30)
                ))
                // Body size limit (10MB)
                .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024))
                // Authentication
                .layer(axum::middleware::from_fn(auth_middleware))
        )
        .with_state(state)
}
```

---

## 4. ruvector-wasm Integration

### 4.1 WASM Crate Configuration

**crates/ruvector-scipix-wasm/Cargo.toml:**

```toml
[package]
name = "ruvector-scipix-wasm"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Browser-based OCR for mathematical expressions"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
# Core - use memory-only features
ruvector-core = {
    version = "0.1.16",
    path = "../ruvector-core",
    default-features = false,
    features = ["memory-only", "simd"]
}
ruvector-wasm = { version = "0.1.16", path = "../ruvector-wasm" }
ruvector-scipix-core = {
    version = "0.1.16",
    path = "../ruvector-scipix-core",
    default-features = false,
    features = ["wasm"]
}

# WASM bindings
wasm-bindgen = { workspace = true }
wasm-bindgen-futures = { workspace = true }
js-sys = { workspace = true }
web-sys = { workspace = true, features = [
    "CanvasRenderingContext2d",
    "HtmlCanvasElement",
    "ImageData",
    "console",
] }

# Utilities
serde = { workspace = true }
serde-wasm-bindgen = "0.6"
console_error_panic_hook = "0.1"
getrandom = { workspace = true, features = ["wasm_js"] }

[features]
default = []

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
```

### 4.2 Browser API

**crates/ruvector-scipix-wasm/src/lib.rs:**

```rust
use wasm_bindgen::prelude::*;
use web_sys::{ImageData, CanvasRenderingContext2d};
use ruvector_scipix_core::{ScipixClient, ScipixCache};

#[wasm_bindgen]
pub struct ScipixWasm {
    client: ScipixClient,
    cache: ScipixCache,
}

#[wasm_bindgen]
impl ScipixWasm {
    /// Create new instance with API key
    #[wasm_bindgen(constructor)]
    pub fn new(api_key: String, app_id: String) -> Result<ScipixWasm, JsValue> {
        console_error_panic_hook::set_once();

        let client = ScipixClient::new(api_key, app_id)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Use in-memory cache for WASM
        let cache = ScipixCache::new_memory(512) // 512-dim embeddings
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(Self { client, cache })
    }

    /// OCR from canvas ImageData
    #[wasm_bindgen]
    pub async fn ocr_image_data(
        &mut self,
        image_data: ImageData,
    ) -> Result<JsValue, JsValue> {
        let width = image_data.width();
        let height = image_data.height();
        let data = image_data.data().0;

        // Convert to PNG bytes
        let png_bytes = self.rgba_to_png(width, height, &data)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Check cache
        let embedding = self.client.generate_image_embedding(&png_bytes)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        if let Some(cached) = self.cache.find_similar(embedding.clone(), 0.95)
            .map_err(|e| JsValue::from_str(&e.to_string()))? {
            return Ok(serde_wasm_bindgen::to_value(&OcrResult {
                latex: cached.latex,
                confidence: cached.confidence,
                cached: true,
            })?);
        }

        // Call API
        let result = self.client.ocr_image(&png_bytes).await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Cache result
        self.cache.store_result(embedding, result.latex.clone(), result.confidence)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(serde_wasm_bindgen::to_value(&OcrResult {
            latex: result.latex,
            confidence: result.confidence,
            cached: false,
        })?)
    }

    /// OCR from canvas element
    #[wasm_bindgen]
    pub async fn ocr_canvas(
        &mut self,
        canvas_id: String,
    ) -> Result<JsValue, JsValue> {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let canvas = document
            .get_element_by_id(&canvas_id)
            .ok_or_else(|| JsValue::from_str("Canvas not found"))?
            .dyn_into::<web_sys::HtmlCanvasElement>()?;

        let context = canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()?;

        let image_data = context.get_image_data(
            0.0, 0.0,
            canvas.width() as f64,
            canvas.height() as f64,
        )?;

        self.ocr_image_data(image_data).await
    }

    fn rgba_to_png(&self, width: u32, height: u32, data: &[u8])
        -> Result<Vec<u8>, String> {
        // Use image crate to encode PNG
        // (simplified - actual implementation would use image crate)
        Ok(data.to_vec())
    }
}

#[derive(serde::Serialize)]
struct OcrResult {
    latex: String,
    confidence: f32,
    cached: bool,
}
```

### 4.3 TypeScript Definitions

**crates/ruvector-scipix-wasm/scipix.d.ts:**

```typescript
export class ScipixWasm {
  constructor(apiKey: string, appId: string);

  ocr_image_data(imageData: ImageData): Promise<OcrResult>;
  ocr_canvas(canvasId: string): Promise<OcrResult>;

  free(): void;
}

export interface OcrResult {
  latex: string;
  confidence: number;
  cached: boolean;
}
```

---

## 5. ruvector-metrics Integration

### 5.1 OCR-Specific Metrics

**crates/ruvector-scipix-core/src/metrics.rs:**

```rust
use prometheus::{
    Counter, Histogram, IntGauge, Registry,
    HistogramOpts, Opts,
};
use lazy_static::lazy_static;

lazy_static! {
    /// Total OCR requests
    pub static ref OCR_REQUESTS: Counter = Counter::new(
        "scipix_ocr_requests_total",
        "Total number of OCR requests"
    ).unwrap();

    /// Cache hit rate
    pub static ref CACHE_HITS: Counter = Counter::new(
        "scipix_cache_hits_total",
        "Number of cache hits"
    ).unwrap();

    pub static ref CACHE_MISSES: Counter = Counter::new(
        "scipix_cache_misses_total",
        "Number of cache misses"
    ).unwrap();

    /// OCR latency histogram
    pub static ref OCR_LATENCY: Histogram = Histogram::with_opts(
        HistogramOpts::new(
            "scipix_ocr_duration_seconds",
            "OCR processing duration"
        ).buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0])
    ).unwrap();

    /// Confidence score distribution
    pub static ref CONFIDENCE_SCORE: Histogram = Histogram::with_opts(
        HistogramOpts::new(
            "scipix_confidence_score",
            "OCR confidence scores"
        ).buckets(vec![0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 0.99])
    ).unwrap();

    /// Active API calls
    pub static ref ACTIVE_CALLS: IntGauge = IntGauge::new(
        "scipix_active_calls",
        "Number of active API calls"
    ).unwrap();

    /// Error counter by type
    pub static ref OCR_ERRORS: Counter = Counter::new(
        "scipix_errors_total",
        "Total OCR errors"
    ).unwrap();
}

/// Register all metrics
pub fn register_metrics(registry: &Registry) -> Result<(), Box<dyn std::error::Error>> {
    registry.register(Box::new(OCR_REQUESTS.clone()))?;
    registry.register(Box::new(CACHE_HITS.clone()))?;
    registry.register(Box::new(CACHE_MISSES.clone()))?;
    registry.register(Box::new(OCR_LATENCY.clone()))?;
    registry.register(Box::new(CONFIDENCE_SCORE.clone()))?;
    registry.register(Box::new(ACTIVE_CALLS.clone()))?;
    registry.register(Box::new(OCR_ERRORS.clone()))?;
    Ok(())
}

/// Track OCR operation
pub struct OcrMetrics;

impl OcrMetrics {
    pub fn record_request() {
        OCR_REQUESTS.inc();
        ACTIVE_CALLS.inc();
    }

    pub fn record_cache_hit() {
        CACHE_HITS.inc();
    }

    pub fn record_cache_miss() {
        CACHE_MISSES.inc();
    }

    pub fn record_latency(duration: std::time::Duration) {
        OCR_LATENCY.observe(duration.as_secs_f64());
        ACTIVE_CALLS.dec();
    }

    pub fn record_confidence(score: f32) {
        CONFIDENCE_SCORE.observe(score as f64);
    }

    pub fn record_error() {
        OCR_ERRORS.inc();
        ACTIVE_CALLS.dec();
    }
}
```

### 5.2 Integration with ruvector-metrics

```rust
// In ScipixClient implementation
impl ScipixClient {
    pub async fn ocr_image(&self, image: &[u8]) -> Result<OcrResult> {
        use crate::metrics::OcrMetrics;

        OcrMetrics::record_request();
        let start = std::time::Instant::now();

        let result = self.ocr_image_internal(image).await;

        match result {
            Ok(ref res) => {
                OcrMetrics::record_latency(start.elapsed());
                OcrMetrics::record_confidence(res.confidence);
            }
            Err(_) => {
                OcrMetrics::record_error();
            }
        }

        result
    }
}
```

### 5.3 Prometheus Endpoint

```rust
// In server routes
use prometheus::{Encoder, TextEncoder};

async fn metrics_handler() -> Result<String, AppError> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer)?;
    Ok(String::from_utf8(buffer)?)
}

// Add to router
Router::new()
    .route("/metrics", get(metrics_handler))
```

---

## 6. ruvector-cluster for Distributed OCR

### 6.1 Sharding Strategy

**crates/ruvector-scipix-core/src/distributed.rs:**

```rust
use ruvector_cluster::{ClusterNode, ShardingStrategy, NodeId};
use std::sync::Arc;

/// Distributed OCR coordinator
pub struct DistributedOcr {
    cluster: Arc<ClusterNode>,
    shard_count: usize,
}

impl DistributedOcr {
    pub fn new(cluster: Arc<ClusterNode>, shard_count: usize) -> Self {
        Self { cluster, shard_count }
    }

    /// Process PDF across cluster
    pub async fn process_pdf_distributed(
        &self,
        pdf_data: Vec<u8>,
    ) -> Result<Vec<PageResult>> {
        // Extract pages
        let pages = extract_pdf_pages(&pdf_data)?;
        let total_pages = pages.len();

        // Shard pages across cluster nodes
        let nodes = self.cluster.get_active_nodes().await?;
        let pages_per_node = (total_pages + nodes.len() - 1) / nodes.len();

        // Distribute work
        let mut tasks = Vec::new();
        for (i, node) in nodes.iter().enumerate() {
            let start = i * pages_per_node;
            let end = ((i + 1) * pages_per_node).min(total_pages);
            let node_pages: Vec<_> = pages[start..end].to_vec();

            let task = self.cluster.send_task(
                node.id,
                OcrTask {
                    pages: node_pages,
                    start_page: start,
                },
            );
            tasks.push(task);
        }

        // Collect results
        let results = futures::future::join_all(tasks).await;

        // Aggregate and sort by page number
        let mut all_results = Vec::new();
        for result in results {
            all_results.extend(result?);
        }
        all_results.sort_by_key(|r| r.page_num);

        Ok(all_results)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct OcrTask {
    pages: Vec<Vec<u8>>,
    start_page: usize,
}
```

### 6.2 Load Balancing

```rust
use ruvector_cluster::LoadBalancer;

/// Smart load balancer for OCR workload
pub struct OcrLoadBalancer {
    balancer: LoadBalancer,
}

impl OcrLoadBalancer {
    /// Assign work based on node capacity and queue depth
    pub async fn assign_task(&self, task_size: usize) -> Result<NodeId> {
        let nodes = self.balancer.get_nodes().await?;

        // Score each node
        let mut best_node = None;
        let mut best_score = f64::MAX;

        for node in nodes {
            let metrics = self.balancer.get_node_metrics(node.id).await?;

            // Score based on:
            // - Queue depth (lower is better)
            // - CPU usage (lower is better)
            // - Task size compatibility
            let score =
                metrics.queue_depth as f64 * 10.0 +
                metrics.cpu_usage * 100.0 +
                (task_size as f64 - metrics.avg_task_size).abs();

            if score < best_score {
                best_score = score;
                best_node = Some(node.id);
            }
        }

        best_node.ok_or_else(|| RuvectorError::NoNodesAvailable)
    }
}
```

### 6.3 Result Aggregation

```rust
/// Aggregate OCR results from multiple nodes
pub struct ResultAggregator {
    results: dashmap::DashMap<uuid::Uuid, Vec<PageResult>>,
}

impl ResultAggregator {
    pub fn add_result(&self, job_id: uuid::Uuid, result: PageResult) {
        self.results.entry(job_id)
            .or_insert_with(Vec::new)
            .push(result);
    }

    pub fn get_results(&self, job_id: uuid::Uuid) -> Option<Vec<PageResult>> {
        self.results.get(&job_id).map(|r| {
            let mut results = r.clone();
            results.sort_by_key(|p| p.page_num);
            results
        })
    }

    pub fn is_complete(&self, job_id: uuid::Uuid, expected_pages: usize) -> bool {
        self.results.get(&job_id)
            .map(|r| r.len() == expected_pages)
            .unwrap_or(false)
    }
}
```

---

## 7. Shared Configuration

### 7.1 Environment Variables

**config/scipix.env:**

```bash
# Scipix API Configuration
MATHPIX_API_KEY=your_api_key_here
MATHPIX_APP_ID=your_app_id_here
MATHPIX_API_URL=https://api.scipix.com/v3

# Cache Configuration
MATHPIX_CACHE_DIR=./data/scipix_cache
MATHPIX_CACHE_DIMENSION=512
MATHPIX_CACHE_SIZE_MB=1000
MATHPIX_CACHE_THRESHOLD=0.95

# Vector DB Configuration
RUVECTOR_HNSW_M=32
RUVECTOR_HNSW_EF_CONSTRUCTION=200
RUVECTOR_DISTANCE_METRIC=cosine

# Quantization
MATHPIX_QUANTIZE_BITS=8  # 0 for no quantization

# Server Configuration
MATHPIX_SERVER_PORT=3000
MATHPIX_SERVER_HOST=0.0.0.0
MATHPIX_MAX_BODY_SIZE_MB=10
MATHPIX_RATE_LIMIT_PER_MIN=100

# Cluster Configuration
MATHPIX_CLUSTER_ENABLED=false
MATHPIX_CLUSTER_NODES=node1:8000,node2:8000
MATHPIX_SHARD_COUNT=4

# Metrics
MATHPIX_METRICS_ENABLED=true
MATHPIX_METRICS_PORT=9090
```

### 7.2 TOML Configuration

**config/scipix.toml:**

```toml
[api]
key = "${MATHPIX_API_KEY}"
app_id = "${MATHPIX_APP_ID}"
url = "https://api.scipix.com/v3"
timeout_secs = 30

[cache]
enabled = true
dir = "./data/scipix_cache"
dimension = 512
size_mb = 1000
threshold = 0.95

[cache.hnsw]
m = 32
ef_construction = 200
max_elements = 100_000

[cache.quantization]
enabled = true
bits = 8  # 4, 8, or 0 for disabled

[server]
host = "0.0.0.0"
port = 3000
max_body_size_mb = 10

[server.rate_limit]
enabled = true
requests_per_minute = 100

[cluster]
enabled = false
nodes = ["node1:8000", "node2:8000"]
shard_count = 4
replication_factor = 2

[metrics]
enabled = true
port = 9090
prometheus_endpoint = "/metrics"

[preprocessing]
# Image preprocessing options
auto_rotate = true
denoise = true
contrast_enhancement = true
dpi = 300

[postprocessing]
# LaTeX postprocessing
validate_syntax = true
normalize_symbols = true
confidence_threshold = 0.7
```

### 7.3 Configuration Loading

**crates/ruvector-scipix-core/src/config.rs:**

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScipixConfig {
    pub api: ApiConfig,
    pub cache: CacheConfig,
    pub server: ServerConfig,
    pub cluster: ClusterConfig,
    pub metrics: MetricsConfig,
    pub preprocessing: PreprocessingConfig,
    pub postprocessing: PostprocessingConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiConfig {
    pub key: String,
    pub app_id: String,
    pub url: String,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheConfig {
    pub enabled: bool,
    pub dir: String,
    pub dimension: usize,
    pub size_mb: usize,
    pub threshold: f32,
    pub hnsw: HnswConfig,
    pub quantization: QuantizationConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HnswConfig {
    pub m: usize,
    pub ef_construction: usize,
    pub max_elements: usize,
}

impl ScipixConfig {
    /// Load from TOML file with environment variable substitution
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;

        // Expand environment variables
        let expanded = Self::expand_env_vars(&content);

        let config: ScipixConfig = toml::from_str(&expanded)?;
        Ok(config)
    }

    /// Load from environment variables
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            api: ApiConfig {
                key: std::env::var("MATHPIX_API_KEY")?,
                app_id: std::env::var("MATHPIX_APP_ID")?,
                url: std::env::var("MATHPIX_API_URL")
                    .unwrap_or_else(|_| "https://api.scipix.com/v3".to_string()),
                timeout_secs: 30,
            },
            cache: CacheConfig::from_env()?,
            // ... rest of config
        })
    }

    fn expand_env_vars(s: &str) -> String {
        let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();
        re.replace_all(s, |caps: &regex::Captures| {
            std::env::var(&caps[1]).unwrap_or_default()
        }).to_string()
    }
}
```

---

## 8. Cross-Crate Types

### 8.1 Common Error Types

**crates/ruvector-scipix-core/src/error.rs:**

```rust
use thiserror::Error;
use ruvector_core::RuvectorError;

#[derive(Error, Debug)]
pub enum ScipixError {
    #[error("Scipix API error: {0}")]
    ApiError(String),

    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Vector database error: {0}")]
    VectorDbError(#[from] RuvectorError),

    #[error("Image processing error: {0}")]
    ImageError(String),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("LaTeX validation error: {0}")]
    LatexError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Confidence too low: {0}")]
    LowConfidence(f32),
}

pub type Result<T> = std::result::Result<T, ScipixError>;

/// Convert to HTTP status code
impl ScipixError {
    pub fn status_code(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            Self::ApiError(_) => StatusCode::BAD_GATEWAY,
            Self::HttpError(_) => StatusCode::BAD_GATEWAY,
            Self::VectorDbError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ImageError(_) => StatusCode::BAD_REQUEST,
            Self::ConfigError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::CacheError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::SerializationError(_) => StatusCode::BAD_REQUEST,
            Self::IoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::LatexError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            Self::AuthenticationFailed => StatusCode::UNAUTHORIZED,
            Self::LowConfidence(_) => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }
}
```

### 8.2 Shared Traits

**crates/ruvector-scipix-core/src/traits.rs:**

```rust
use async_trait::async_trait;

/// OCR engine trait (allows swapping implementations)
#[async_trait]
pub trait OcrEngine: Send + Sync {
    /// Process image to LaTeX
    async fn ocr(&self, image: &[u8]) -> Result<OcrResult>;

    /// Generate embedding for caching
    fn generate_embedding(&self, image: &[u8]) -> Result<Vec<f32>>;

    /// Batch processing
    async fn ocr_batch(&self, images: Vec<Vec<u8>>) -> Result<Vec<OcrResult>> {
        let mut results = Vec::new();
        for image in images {
            results.push(self.ocr(&image).await?);
        }
        Ok(results)
    }
}

/// Cache trait (allows different cache backends)
pub trait OcrCache: Send + Sync {
    fn store(&mut self, embedding: Vec<f32>, result: OcrResult) -> Result<uuid::Uuid>;
    fn find_similar(&self, embedding: Vec<f32>, threshold: f32) -> Result<Option<OcrResult>>;
    fn clear(&mut self) -> Result<()>;
    fn stats(&self) -> CacheStats;
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub memory_usage_mb: f64,
    pub hit_rate: f64,
}

/// Preprocessing trait
pub trait ImagePreprocessor: Send + Sync {
    fn preprocess(&self, image: &[u8]) -> Result<Vec<u8>>;
}

/// Postprocessing trait
pub trait LatexPostprocessor: Send + Sync {
    fn postprocess(&self, latex: &str) -> Result<String>;
    fn validate(&self, latex: &str) -> bool;
}
```

### 8.3 API Contracts

**crates/ruvector-scipix-core/src/types.rs:**

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// OCR result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub latex: String,
    pub confidence: f32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub cached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// PDF page result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageResult {
    pub page_num: usize,
    pub latex: String,
    pub confidence: f32,
    pub bounding_boxes: Vec<BoundingBox>,
}

/// Bounding box for detected regions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub confidence: f32,
}

/// Batch OCR request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOcrRequest {
    pub images: Vec<ImageInput>,
    pub options: OcrOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ImageInput {
    #[serde(rename = "base64")]
    Base64 { data: String },
    #[serde(rename = "url")]
    Url { url: String },
    #[serde(rename = "bytes")]
    Bytes { data: Vec<u8> },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OcrOptions {
    #[serde(default)]
    pub preprocess: bool,
    #[serde(default)]
    pub postprocess: bool,
    #[serde(default = "default_confidence")]
    pub min_confidence: f32,
    #[serde(default)]
    pub use_cache: bool,
}

fn default_confidence() -> f32 { 0.7 }

/// Job status for async processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatus {
    pub job_id: Uuid,
    pub status: JobState,
    pub progress: f32,  // 0.0 to 1.0
    pub result: Option<Vec<PageResult>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    Pending,
    Processing,
    Completed,
    Failed,
}
```

---

## 9. Workspace Cargo.toml Modifications

### 9.1 Complete Workspace Configuration

```toml
# Add to root Cargo.toml

[workspace]
members = [
    # ... existing members ...

    # Scipix Integration
    "crates/ruvector-scipix-core",
    "crates/ruvector-scipix-node",
    "crates/ruvector-scipix-wasm",
    "crates/ruvector-scipix-server",
]

[workspace.dependencies]
# ... existing dependencies ...

# Scipix-specific
reqwest = { version = "0.12", default-features = false, features = ["json", "multipart", "rustls-tls"] }
base64 = "0.22"
image = { version = "0.25", features = ["png", "jpeg", "webp"] }
async-trait = "0.1"
regex = "1.10"
toml = "0.8"

# Optional OCR backends
tesseract-rs = { version = "0.14", optional = true }
pdf-extract = { version = "0.7", optional = true }
```

### 9.2 Individual Crate Cargo.toml

**crates/ruvector-scipix-core/Cargo.toml:**

```toml
[package]
name = "ruvector-scipix-core"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "Mathematical OCR with vector-based caching"

[dependencies]
# Ruvector ecosystem
ruvector-core = { version = "0.1.16", path = "../ruvector-core" }
ruvector-metrics = { version = "0.1.16", path = "../ruvector-metrics", optional = true }

# HTTP client
reqwest = { workspace = true, optional = true }
base64 = { workspace = true }

# Image processing
image = { workspace = true, optional = true }

# Async
tokio = { workspace = true, features = ["rt-multi-thread"] }
async-trait = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }

# Error handling
thiserror = { workspace = true }
anyhow = { workspace = true }

# Utilities
uuid = { workspace = true }
chrono = { workspace = true }
tracing = { workspace = true }
dashmap = { workspace = true }
parking_lot = { workspace = true }

# Configuration
toml = { workspace = true, optional = true }
regex = { workspace = true, optional = true }

# Optional backends
tesseract-rs = { workspace = true, optional = true }
pdf-extract = { workspace = true, optional = true }

# Metrics
prometheus = { version = "0.13", optional = true }
lazy_static = { version = "1.5", optional = true }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "test-util"] }
tempfile = "3.13"
mockall = { workspace = true }

[features]
default = ["api-client", "cache", "preprocessing"]

# Core features
api-client = ["reqwest"]
cache = ["ruvector-core/storage"]
preprocessing = ["image"]
metrics = ["dep:ruvector-metrics", "prometheus", "lazy_static"]
config = ["toml", "regex"]

# Optional backends
tesseract = ["dep:tesseract-rs"]
pdf = ["dep:pdf-extract"]

# Performance
simd = ["ruvector-core/simd"]
quantization = []

# Environment
wasm = []
memory-only = []
```

---

## 10. Module Structure

### 10.1 Core Module Organization

```
crates/ruvector-scipix-core/src/
├── lib.rs                      # Public API
├── error.rs                    # Error types
├── types.rs                    # Shared types
├── traits.rs                   # Shared traits
├── config.rs                   # Configuration
│
├── api/
│   ├── mod.rs
│   ├── client.rs              # Scipix API client
│   └── models.rs              # API request/response types
│
├── cache/
│   ├── mod.rs
│   ├── vector_cache.rs        # ruvector-core integration
│   ├── memory_cache.rs        # In-memory cache for WASM
│   └── stats.rs               # Cache statistics
│
├── ocr/
│   ├── mod.rs
│   ├── engine.rs              # Main OCR engine
│   ├── batch.rs               # Batch processing
│   └── backends/
│       ├── mod.rs
│       ├── scipix.rs         # Scipix backend
│       └── tesseract.rs       # Tesseract fallback
│
├── preprocessing/
│   ├── mod.rs
│   ├── image_ops.rs           # Image preprocessing
│   ├── filters.rs             # Denoising, enhancement
│   └── rotation.rs            # Auto-rotation
│
├── postprocessing/
│   ├── mod.rs
│   ├── latex_validate.rs      # LaTeX validation
│   └── normalize.rs           # Symbol normalization
│
├── embeddings/
│   ├── mod.rs
│   ├── image_embedder.rs      # Image to vector
│   └── latex_embedder.rs      # LaTeX to vector
│
├── distributed/
│   ├── mod.rs
│   ├── coordinator.rs         # Cluster coordination
│   ├── sharding.rs            # Work distribution
│   └── aggregator.rs          # Result aggregation
│
└── metrics/
    ├── mod.rs
    └── prometheus.rs          # Metrics collection
```

### 10.2 Server Module Organization

```
crates/ruvector-scipix-server/src/
├── main.rs                     # Server entry point
├── routes/
│   ├── mod.rs
│   ├── ocr.rs                 # OCR endpoints
│   ├── cache.rs               # Cache management
│   ├── health.rs              # Health checks
│   └── metrics.rs             # Metrics endpoint
│
├── middleware/
│   ├── mod.rs
│   ├── auth.rs                # API key auth
│   ├── rate_limit.rs          # Rate limiting
│   └── logging.rs             # Request logging
│
├── state.rs                    # Shared app state
└── error.rs                    # HTTP error handling
```

---

## 11. Integration Checklist

### Phase 1: Core Integration
- [ ] Create `ruvector-scipix-core` crate
- [ ] Implement vector cache using `ruvector-core`
- [ ] Add Scipix API client
- [ ] Implement image preprocessing
- [ ] Add metrics collection
- [ ] Write unit tests

### Phase 2: Server Extension
- [ ] Create `ruvector-scipix-server` crate
- [ ] Implement REST API endpoints
- [ ] Add authentication middleware
- [ ] Implement rate limiting
- [ ] Add health checks
- [ ] Integration tests

### Phase 3: WASM Support
- [ ] Create `ruvector-scipix-wasm` crate
- [ ] Implement browser API
- [ ] Add TypeScript definitions
- [ ] Create example web app
- [ ] Browser testing

### Phase 4: Distributed Processing
- [ ] Integrate `ruvector-cluster`
- [ ] Implement work sharding
- [ ] Add load balancing
- [ ] Implement result aggregation
- [ ] Distributed tests

### Phase 5: Node.js Bindings
- [ ] Create `ruvector-scipix-node` crate
- [ ] Implement NAPI bindings
- [ ] Add TypeScript types
- [ ] Build platform binaries
- [ ] NPM package

### Phase 6: Optimization
- [ ] Enable quantization
- [ ] SIMD optimizations
- [ ] Cache tuning
- [ ] Performance benchmarks
- [ ] Documentation

---

## 12. Performance Targets

### Cache Performance
- **Hit Rate:** >80% on repeated expressions
- **Lookup Latency:** <10ms (p99)
- **Memory Overhead:** 4-8x reduction with quantization

### API Performance
- **OCR Latency:** <2s for single image
- **Throughput:** >100 req/min per node
- **PDF Processing:** <10s for 10-page document

### Cluster Performance
- **Scaling Efficiency:** >90% up to 8 nodes
- **Fault Tolerance:** Continue with 1 node failure
- **Shard Rebalancing:** <30s

---

## 13. Security Considerations

### API Key Management
- Never commit API keys to repository
- Use environment variables or secure vaults
- Rotate keys regularly
- Implement key-per-user for multi-tenant

### Rate Limiting
- Per-IP and per-API-key limits
- Sliding window algorithm
- Graceful degradation under load

### Input Validation
- Image size limits (10MB default)
- Format validation (PNG, JPEG only)
- Sanitize LaTeX output
- Prevent injection attacks

### Cache Security
- Encrypt sensitive cached data
- Implement cache eviction policies
- Prevent cache poisoning
- Audit cache access

---

## 14. Monitoring & Observability

### Key Metrics
- `scipix_ocr_requests_total` - Total requests
- `scipix_cache_hit_rate` - Cache effectiveness
- `scipix_ocr_duration_seconds` - Latency distribution
- `scipix_confidence_score` - Quality tracking
- `scipix_errors_total` - Error rate

### Dashboards
- Real-time OCR throughput
- Cache performance
- Error rates by type
- Confidence score distribution
- Cluster health

### Alerts
- Error rate >5%
- Latency p99 >5s
- Cache hit rate <60%
- Node failures
- API quota exhaustion

---

## 15. Migration Path

### From Standalone to Integrated

**Step 1:** Add ruvector-core dependency
```bash
cd crates/ruvector-scipix-core
cargo add ruvector-core --path ../ruvector-core
```

**Step 2:** Migrate cache to VectorDB
```rust
// Old: HashMap-based cache
let cache = HashMap::new();

// New: Vector-based cache
let cache = ScipixCache::new("./cache", 512)?;
```

**Step 3:** Integrate metrics
```rust
use ruvector_scipix_core::metrics::OcrMetrics;

OcrMetrics::record_request();
// ... perform OCR ...
OcrMetrics::record_latency(duration);
```

**Step 4:** Deploy with cluster support
```bash
# Enable cluster feature
cargo build --release --features cluster

# Start with cluster config
MATHPIX_CLUSTER_ENABLED=true cargo run
```

---

## 16. Testing Strategy

### Unit Tests
- Vector cache operations
- Embedding generation
- LaTeX validation
- Error handling

### Integration Tests
- End-to-end OCR flow
- Cache hit/miss scenarios
- Cluster coordination
- API endpoint testing

### Performance Tests
- Cache lookup benchmarks
- HNSW search performance
- Quantization overhead
- Distributed scaling

### Browser Tests (WASM)
- Canvas image capture
- API calls from browser
- Memory management
- Error handling

---

## 17. Documentation Requirements

### API Documentation
- OpenAPI/Swagger spec
- Example requests/responses
- Error codes
- Rate limits

### Integration Guides
- Quick start guide
- Configuration reference
- Cluster setup
- WASM integration

### Performance Tuning
- Cache configuration
- HNSW parameters
- Quantization trade-offs
- Cluster sizing

---

## Conclusion

This integration architecture provides a comprehensive blueprint for incorporating `ruvector-scipix` into the ruvector ecosystem. By leveraging existing infrastructure for vector storage, clustering, metrics, and WASM support, we achieve:

1. **Performance:** 80%+ cache hit rate, <10ms lookup latency
2. **Scalability:** Horizontal scaling via ruvector-cluster
3. **Flexibility:** Multiple deployment targets (server, browser, Node.js)
4. **Maintainability:** Shared types, errors, and configuration patterns
5. **Observability:** Rich metrics and monitoring

The modular design allows incremental adoption, starting with core OCR functionality and progressively adding caching, clustering, and advanced features.

---

**Next Steps:**
1. Review and approve architecture
2. Create Phase 1 crates (`ruvector-scipix-core`)
3. Implement vector cache integration
4. Add comprehensive tests
5. Deploy initial server with basic endpoints
6. Iterate based on performance metrics
