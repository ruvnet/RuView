# API Server Design - Scipix API v3 Compatibility

## Overview

This document describes the REST API server implementation for ruvector-scipix, providing full compatibility with Scipix API v3 endpoints while leveraging Rust's performance and safety.

**Stack:**
- **Web Framework:** Axum (high-performance, ergonomic)
- **Serialization:** Serde (JSON/multipart)
- **Async Runtime:** Tokio
- **Middleware:** Tower
- **Auth:** Custom middleware
- **Rate Limiting:** tower-governor
- **Database:** PostgreSQL (job storage) + Redis (queue/cache)

---

## 1. API Design

### 1.1 Core Request/Response Structures

```rust
// src/api/models.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Authentication credentials
#[derive(Debug, Clone, Deserialize)]
pub struct AuthCredentials {
    pub app_id: String,
    pub app_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BearerAuth {
    pub app_token: String,
}

/// Common request options
#[derive(Debug, Deserialize, Clone)]
pub struct OcrOptions {
    /// Include image data in response
    #[serde(default)]
    pub include_detected_alphabets: bool,

    /// Include confidence scores
    #[serde(default)]
    pub include_confidence: bool,

    /// Include word/line bounding boxes
    #[serde(default)]
    pub include_geometry: bool,

    /// Include LaTeX output
    #[serde(default)]
    pub include_latex: bool,

    /// Include MathML output
    #[serde(default)]
    pub include_mathml: bool,

    /// Include table structure
    #[serde(default)]
    pub include_table_data: bool,

    /// Skip text detection
    #[serde(default)]
    pub skip_text_detection: bool,

    /// Alphabets to detect (e.g., ["en", "es", "de"])
    #[serde(default)]
    pub alphabets: Vec<String>,

    /// Output formats (json, latex, html, etc.)
    #[serde(default)]
    pub formats: Vec<String>,
}

/// POST /v3/text request
#[derive(Debug, Deserialize)]
pub struct TextRequest {
    /// Base64-encoded image or URL
    pub src: String,

    /// Optional processing options
    #[serde(flatten)]
    pub options: OcrOptions,

    /// Callback URL for async processing
    pub callback_url: Option<String>,

    /// Metadata for tracking
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Text detection result
#[derive(Debug, Serialize)]
pub struct TextResponse {
    /// Request ID for tracking
    pub request_id: String,

    /// Detected text
    pub text: String,

    /// LaTeX representation (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latex: Option<String>,

    /// MathML representation (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mathml: Option<String>,

    /// Confidence score (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,

    /// Word/line geometry
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<Vec<BoundingBox>>,

    /// Detected alphabets
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_alphabets: Option<Vec<String>>,

    /// Processing time (ms)
    pub processing_time_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub text: String,
    pub confidence: f32,
}

/// POST /v3/strokes request (digital ink)
#[derive(Debug, Deserialize)]
pub struct StrokesRequest {
    /// Array of stroke data
    pub strokes: Vec<Stroke>,

    #[serde(flatten)]
    pub options: OcrOptions,
}

#[derive(Debug, Deserialize)]
pub struct Stroke {
    /// X coordinates
    pub x: Vec<f32>,
    /// Y coordinates
    pub y: Vec<f32>,
    /// Timestamps (optional)
    pub t: Option<Vec<f32>>,
}

/// POST /v3/pdf request (async)
#[derive(Debug, Deserialize)]
pub struct PdfRequest {
    /// PDF source (URL or base64)
    pub src: String,

    /// Conversion format (mmd, docx, html, etc.)
    pub conversion_format: String,

    /// Math formatting options
    pub math_inline_delimiters: Option<Vec<String>>,
    pub math_display_delimiters: Option<Vec<String>>,

    /// Enable table detection
    #[serde(default)]
    pub enable_tables_fallback: bool,

    /// Callback URL
    pub callback_url: Option<String>,

    #[serde(flatten)]
    pub options: OcrOptions,
}

/// PDF job response
#[derive(Debug, Serialize)]
pub struct PdfJobResponse {
    pub pdf_id: String,
    pub status: JobStatus,
    pub created_at: String,

    /// Estimated completion time (seconds)
    pub estimated_completion_time: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

/// GET /v3/pdf/{id} response
#[derive(Debug, Serialize)]
pub struct PdfStatusResponse {
    pub pdf_id: String,
    pub status: JobStatus,
    pub progress: f32,  // 0.0-1.0

    /// Result URL (when completed)
    pub result_url: Option<String>,

    /// Error message (if failed)
    pub error: Option<String>,

    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

/// POST /v3/converter request
#[derive(Debug, Deserialize)]
pub struct ConverterRequest {
    /// MMD content
    pub src: String,

    /// Target format (html, pdf, docx)
    pub format: String,

    /// Conversion options
    pub options: Option<HashMap<String, serde_json::Value>>,
}

/// GET /v3/ocr-results query parameters
#[derive(Debug, Deserialize)]
pub struct OcrResultsQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub status: Option<JobStatus>,
}

/// GET /v3/ocr-usage response
#[derive(Debug, Serialize)]
pub struct UsageStats {
    pub period: String,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_processing_time_ms: u64,
    pub average_processing_time_ms: f64,
    pub requests_by_endpoint: HashMap<String, u64>,
}

/// Standard error response
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub error_code: String,
    pub message: String,
    pub request_id: Option<String>,
}
```

### 1.2 Error Codes

```rust
// src/api/errors.rs
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

#[derive(Debug)]
pub enum ApiErrorCode {
    // Authentication errors (401)
    InvalidCredentials,
    ExpiredToken,
    MissingAuth,

    // Authorization errors (403)
    InsufficientQuota,
    RateLimitExceeded,

    // Request errors (400)
    InvalidRequest,
    InvalidImageFormat,
    ImageTooLarge,
    InvalidPdfFormat,

    // Processing errors (422)
    ProcessingFailed,
    ModelLoadFailed,

    // Server errors (500)
    InternalError,
    ServiceUnavailable,

    // Resource errors (404)
    JobNotFound,
    ResultNotFound,
}

impl ApiErrorCode {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "invalid_credentials",
            Self::ExpiredToken => "expired_token",
            Self::MissingAuth => "missing_auth",
            Self::InsufficientQuota => "insufficient_quota",
            Self::RateLimitExceeded => "rate_limit_exceeded",
            Self::InvalidRequest => "invalid_request",
            Self::InvalidImageFormat => "invalid_image_format",
            Self::ImageTooLarge => "image_too_large",
            Self::InvalidPdfFormat => "invalid_pdf_format",
            Self::ProcessingFailed => "processing_failed",
            Self::ModelLoadFailed => "model_load_failed",
            Self::InternalError => "internal_error",
            Self::ServiceUnavailable => "service_unavailable",
            Self::JobNotFound => "job_not_found",
            Self::ResultNotFound => "result_not_found",
        }
    }

    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidCredentials | Self::ExpiredToken | Self::MissingAuth
                => StatusCode::UNAUTHORIZED,
            Self::InsufficientQuota | Self::RateLimitExceeded
                => StatusCode::FORBIDDEN,
            Self::InvalidRequest | Self::InvalidImageFormat
                | Self::ImageTooLarge | Self::InvalidPdfFormat
                => StatusCode::BAD_REQUEST,
            Self::ProcessingFailed | Self::ModelLoadFailed
                => StatusCode::UNPROCESSABLE_ENTITY,
            Self::JobNotFound | Self::ResultNotFound
                => StatusCode::NOT_FOUND,
            Self::InternalError | Self::ServiceUnavailable
                => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "Invalid app_id or app_key",
            Self::ExpiredToken => "Authentication token has expired",
            Self::MissingAuth => "Missing authentication credentials",
            Self::InsufficientQuota => "Insufficient API quota",
            Self::RateLimitExceeded => "Rate limit exceeded. Please retry later.",
            Self::InvalidRequest => "Invalid request parameters",
            Self::InvalidImageFormat => "Unsupported image format",
            Self::ImageTooLarge => "Image exceeds maximum size limit",
            Self::InvalidPdfFormat => "Invalid or corrupted PDF file",
            Self::ProcessingFailed => "Failed to process input",
            Self::ModelLoadFailed => "Failed to load processing model",
            Self::InternalError => "Internal server error",
            Self::ServiceUnavailable => "Service temporarily unavailable",
            Self::JobNotFound => "Job not found",
            Self::ResultNotFound => "Result not found or expired",
        }
    }
}

pub struct AppError {
    pub code: ApiErrorCode,
    pub context: Option<String>,
    pub request_id: Option<String>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let error_response = super::models::ApiError {
            error: self.code.code().to_string(),
            error_code: self.code.code().to_string(),
            message: self.context.unwrap_or_else(|| self.code.message().to_string()),
            request_id: self.request_id,
        };

        (self.code.status_code(), Json(error_response)).into_response()
    }
}
```

---

## 2. Axum Server Implementation

### 2.1 Server Setup

```rust
// src/api/server.rs
use axum::{
    Router,
    routing::{get, post, delete},
    middleware,
    Extension,
};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{
    cors::{CorsLayer, Any},
    trace::TraceLayer,
    compression::CompressionLayer,
};

pub struct ApiServer {
    config: Arc<ServerConfig>,
    state: Arc<AppState>,
}

#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
    pub redis_client: redis::aio::ConnectionManager,
    pub job_queue: Arc<JobQueue>,
    pub model_manager: Arc<ModelManager>,
    pub auth_service: Arc<AuthService>,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_upload_size: usize,  // bytes
    pub request_timeout: u64,     // seconds
    pub enable_tls: bool,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub model_path: String,
    pub storage_path: String,
    pub redis_url: String,
    pub database_url: String,
}

impl ApiServer {
    pub async fn new(config: ServerConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize database pool
        let db_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(20)
            .connect(&config.database_url)
            .await?;

        // Initialize Redis client
        let redis_client = redis::Client::open(config.redis_url.clone())?;
        let redis_conn = redis_client.get_connection_manager().await?;

        // Initialize job queue
        let job_queue = Arc::new(JobQueue::new(redis_conn.clone()));

        // Initialize model manager
        let model_manager = Arc::new(
            ModelManager::new(&config.model_path).await?
        );

        // Initialize auth service
        let auth_service = Arc::new(AuthService::new(db_pool.clone()));

        let state = Arc::new(AppState {
            db_pool,
            redis_client: redis_conn,
            job_queue,
            model_manager,
            auth_service,
        });

        Ok(Self {
            config: Arc::new(config),
            state,
        })
    }

    pub fn router(&self) -> Router {
        // API v3 routes
        let v3_routes = Router::new()
            // OCR endpoints
            .route("/text", post(handlers::process_text))
            .route("/strokes", post(handlers::process_strokes))
            .route("/latex", post(handlers::process_latex))

            // PDF processing
            .route("/pdf", post(handlers::submit_pdf))
            .route("/pdf/:id", get(handlers::get_pdf_status))
            .route("/pdf/:id", delete(handlers::delete_pdf_job))

            // Converter
            .route("/converter", post(handlers::convert_document))

            // Query endpoints
            .route("/ocr-results", get(handlers::query_results))
            .route("/ocr-usage", get(handlers::get_usage_stats))

            // Apply authentication middleware
            .layer(middleware::from_fn_with_state(
                self.state.clone(),
                auth_middleware,
            ))

            // Apply rate limiting
            .layer(middleware::from_fn_with_state(
                self.state.clone(),
                rate_limit_middleware,
            ));

        // Health check (no auth)
        let health_routes = Router::new()
            .route("/health", get(handlers::health_check))
            .route("/ready", get(handlers::readiness_check));

        Router::new()
            .nest("/v3", v3_routes)
            .merge(health_routes)
            .layer(
                ServiceBuilder::new()
                    // Logging
                    .layer(TraceLayer::new_for_http())
                    // CORS
                    .layer(
                        CorsLayer::new()
                            .allow_origin(Any)
                            .allow_methods(Any)
                            .allow_headers(Any)
                    )
                    // Compression
                    .layer(CompressionLayer::new())
                    // Request ID
                    .layer(middleware::from_fn(request_id_middleware))
            )
            .layer(Extension(self.state.clone()))
            .layer(Extension(self.config.clone()))
    }

    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;

        tracing::info!("API server listening on {}", addr);

        if self.config.enable_tls {
            // TLS configuration
            let tls_config = self.load_tls_config()?;
            axum_server::from_tcp_rustls(listener.into_std()?, tls_config)
                .serve(self.router().into_make_service())
                .await?;
        } else {
            axum::serve(listener, self.router())
                .await?;
        }

        Ok(())
    }

    fn load_tls_config(&self) -> Result<
        axum_server::tls_rustls::RustlsConfig,
        Box<dyn std::error::Error>
    > {
        let cert_path = self.config.tls_cert_path.as_ref()
            .ok_or("TLS cert path not configured")?;
        let key_path = self.config.tls_key_path.as_ref()
            .ok_or("TLS key path not configured")?;

        Ok(axum_server::tls_rustls::RustlsConfig::from_pem_file(
            cert_path,
            key_path,
        ))
    }
}
```

### 2.2 Middleware Stack

```rust
// src/api/middleware/auth.rs
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
    http::header,
};

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // Check for Bearer token
    if let Some(auth_header) = request.headers().get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                let user = state.auth_service
                    .validate_token(token)
                    .await
                    .map_err(|_| AppError {
                        code: ApiErrorCode::InvalidCredentials,
                        context: None,
                        request_id: None,
                    })?;

                request.extensions_mut().insert(user);
                return Ok(next.run(request).await);
            }
        }
    }

    // Check for app_id and app_key headers
    let app_id = request.headers()
        .get("app_id")
        .and_then(|v| v.to_str().ok());
    let app_key = request.headers()
        .get("app_key")
        .and_then(|v| v.to_str().ok());

    if let (Some(id), Some(key)) = (app_id, app_key) {
        let user = state.auth_service
            .validate_credentials(id, key)
            .await
            .map_err(|_| AppError {
                code: ApiErrorCode::InvalidCredentials,
                context: None,
                request_id: None,
            })?;

        request.extensions_mut().insert(user);
        return Ok(next.run(request).await);
    }

    Err(AppError {
        code: ApiErrorCode::MissingAuth,
        context: None,
        request_id: None,
    })
}

// src/api/middleware/rate_limit.rs
use tower_governor::{
    governor::GovernorConfigBuilder,
    key_extractor::SmartIpKeyExtractor,
    GovernorLayer,
};

pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // Extract user from request
    let user = request.extensions().get::<AuthUser>()
        .ok_or(AppError {
            code: ApiErrorCode::MissingAuth,
            context: None,
            request_id: None,
        })?;

    // Check rate limit
    let limit_key = format!("rate_limit:{}", user.id);
    let current_count: u64 = state.redis_client
        .clone()
        .incr(&limit_key, 1)
        .await
        .unwrap_or(1);

    if current_count == 1 {
        // Set expiry (1 minute window)
        let _: () = state.redis_client
            .clone()
            .expire(&limit_key, 60)
            .await
            .unwrap_or(());
    }

    // Check against user's rate limit
    if current_count > user.rate_limit {
        return Err(AppError {
            code: ApiErrorCode::RateLimitExceeded,
            context: Some(format!(
                "Rate limit: {} requests per minute",
                user.rate_limit
            )),
            request_id: None,
        });
    }

    Ok(next.run(request).await)
}

// src/api/middleware/request_id.rs
use uuid::Uuid;

pub async fn request_id_middleware(
    mut request: Request,
    next: Next,
) -> Response {
    let request_id = Uuid::new_v4().to_string();
    request.extensions_mut().insert(RequestId(request_id.clone()));

    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "X-Request-ID",
        request_id.parse().unwrap(),
    );

    response
}

#[derive(Clone)]
pub struct RequestId(pub String);
```

---

## 3. Request Handlers

### 3.1 Image Processing Endpoint

```rust
// src/api/handlers/text.rs
use axum::{
    extract::{State, Multipart},
    Json,
};

pub async fn process_text(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthUser>,
    Extension(request_id): Extension<RequestId>,
    payload: Json<TextRequest>,
) -> Result<Json<TextResponse>, AppError> {
    let start = std::time::Instant::now();

    // Parse image source
    let image_data = parse_image_source(&payload.src).await
        .map_err(|e| AppError {
            code: ApiErrorCode::InvalidImageFormat,
            context: Some(e.to_string()),
            request_id: Some(request_id.0.clone()),
        })?;

    // Validate image size
    if image_data.len() > state.config.max_upload_size {
        return Err(AppError {
            code: ApiErrorCode::ImageTooLarge,
            context: Some(format!(
                "Max size: {} bytes",
                state.config.max_upload_size
            )),
            request_id: Some(request_id.0.clone()),
        });
    }

    // Process image
    let result = state.model_manager
        .process_image(&image_data, &payload.options)
        .await
        .map_err(|e| AppError {
            code: ApiErrorCode::ProcessingFailed,
            context: Some(e.to_string()),
            request_id: Some(request_id.0.clone()),
        })?;

    // Record usage
    record_usage(&state.db_pool, &user, "text", start.elapsed()).await?;

    // Send callback if requested
    if let Some(callback_url) = &payload.callback_url {
        tokio::spawn(send_callback(
            callback_url.clone(),
            request_id.0.clone(),
            result.clone(),
        ));
    }

    Ok(Json(TextResponse {
        request_id: request_id.0,
        text: result.text,
        latex: payload.options.include_latex.then_some(result.latex),
        mathml: payload.options.include_mathml.then_some(result.mathml),
        confidence: payload.options.include_confidence.then_some(result.confidence),
        geometry: payload.options.include_geometry.then_some(result.geometry),
        detected_alphabets: payload.options.include_detected_alphabets
            .then_some(result.detected_alphabets),
        processing_time_ms: start.elapsed().as_millis() as u64,
    }))
}

async fn parse_image_source(src: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if src.starts_with("http://") || src.starts_with("https://") {
        // Download from URL
        let response = reqwest::get(src).await?;
        Ok(response.bytes().await?.to_vec())
    } else if src.starts_with("data:image/") {
        // Parse data URL
        let base64_data = src.split(',').nth(1)
            .ok_or("Invalid data URL")?;
        Ok(base64::decode(base64_data)?)
    } else {
        // Assume base64
        Ok(base64::decode(src)?)
    }
}

// Multipart upload handler
pub async fn process_text_multipart(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthUser>,
    Extension(request_id): Extension<RequestId>,
    mut multipart: Multipart,
) -> Result<Json<TextResponse>, AppError> {
    let mut image_data = None;
    let mut options = OcrOptions::default();

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                image_data = Some(field.bytes().await.unwrap().to_vec());
            }
            "options" => {
                let json_str = field.text().await.unwrap();
                options = serde_json::from_str(&json_str).unwrap_or_default();
            }
            _ => {}
        }
    }

    let image_data = image_data.ok_or(AppError {
        code: ApiErrorCode::InvalidRequest,
        context: Some("Missing image file".to_string()),
        request_id: Some(request_id.0.clone()),
    })?;

    // Process image (reuse logic from process_text)
    let start = std::time::Instant::now();
    let result = state.model_manager
        .process_image(&image_data, &options)
        .await
        .map_err(|e| AppError {
            code: ApiErrorCode::ProcessingFailed,
            context: Some(e.to_string()),
            request_id: Some(request_id.0.clone()),
        })?;

    Ok(Json(TextResponse {
        request_id: request_id.0,
        text: result.text,
        latex: options.include_latex.then_some(result.latex),
        mathml: options.include_mathml.then_some(result.mathml),
        confidence: options.include_confidence.then_some(result.confidence),
        geometry: options.include_geometry.then_some(result.geometry),
        detected_alphabets: options.include_detected_alphabets
            .then_some(result.detected_alphabets),
        processing_time_ms: start.elapsed().as_millis() as u64,
    }))
}
```

### 3.2 PDF Processing (Async)

```rust
// src/api/handlers/pdf.rs

pub async fn submit_pdf(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthUser>,
    Extension(request_id): Extension<RequestId>,
    Json(payload): Json<PdfRequest>,
) -> Result<Json<PdfJobResponse>, AppError> {
    // Parse PDF source
    let pdf_data = parse_pdf_source(&payload.src).await
        .map_err(|e| AppError {
            code: ApiErrorCode::InvalidPdfFormat,
            context: Some(e.to_string()),
            request_id: Some(request_id.0.clone()),
        })?;

    // Create job
    let pdf_id = Uuid::new_v4().to_string();
    let job = PdfJob {
        id: pdf_id.clone(),
        user_id: user.id,
        status: JobStatus::Queued,
        pdf_data,
        conversion_format: payload.conversion_format,
        options: payload.options,
        callback_url: payload.callback_url,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        completed_at: None,
        result_url: None,
        error: None,
    };

    // Store job in database
    sqlx::query!(
        r#"
        INSERT INTO pdf_jobs (id, user_id, status, conversion_format, options, callback_url, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        job.id,
        job.user_id,
        serde_json::to_value(&job.status).unwrap(),
        job.conversion_format,
        serde_json::to_value(&job.options).unwrap(),
        job.callback_url,
        job.created_at,
    )
    .execute(&state.db_pool)
    .await
    .map_err(|e| AppError {
        code: ApiErrorCode::InternalError,
        context: Some(e.to_string()),
        request_id: Some(request_id.0.clone()),
    })?;

    // Queue job
    state.job_queue.enqueue(job).await
        .map_err(|e| AppError {
            code: ApiErrorCode::InternalError,
            context: Some(e.to_string()),
            request_id: Some(request_id.0.clone()),
        })?;

    Ok(Json(PdfJobResponse {
        pdf_id,
        status: JobStatus::Queued,
        created_at: chrono::Utc::now().to_rfc3339(),
        estimated_completion_time: Some(300), // 5 minutes
    }))
}

pub async fn get_pdf_status(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthUser>,
    Extension(request_id): Extension<RequestId>,
    axum::extract::Path(pdf_id): axum::extract::Path<String>,
) -> Result<Json<PdfStatusResponse>, AppError> {
    // Query job status
    let job = sqlx::query_as!(
        PdfJobRecord,
        r#"
        SELECT * FROM pdf_jobs
        WHERE id = $1 AND user_id = $2
        "#,
        pdf_id,
        user.id,
    )
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| AppError {
        code: ApiErrorCode::InternalError,
        context: Some(e.to_string()),
        request_id: Some(request_id.0.clone()),
    })?
    .ok_or(AppError {
        code: ApiErrorCode::JobNotFound,
        context: None,
        request_id: Some(request_id.0.clone()),
    })?;

    Ok(Json(PdfStatusResponse {
        pdf_id: job.id,
        status: serde_json::from_value(job.status).unwrap(),
        progress: job.progress.unwrap_or(0.0),
        result_url: job.result_url,
        error: job.error,
        created_at: job.created_at.to_rfc3339(),
        updated_at: job.updated_at.to_rfc3339(),
        completed_at: job.completed_at.map(|dt| dt.to_rfc3339()),
    }))
}

pub async fn delete_pdf_job(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthUser>,
    Extension(request_id): Extension<RequestId>,
    axum::extract::Path(pdf_id): axum::extract::Path<String>,
) -> Result<StatusCode, AppError> {
    // Update job status to cancelled
    let result = sqlx::query!(
        r#"
        UPDATE pdf_jobs
        SET status = $1, updated_at = $2
        WHERE id = $3 AND user_id = $4 AND status != 'completed'
        "#,
        serde_json::to_value(&JobStatus::Cancelled).unwrap(),
        chrono::Utc::now(),
        pdf_id,
        user.id,
    )
    .execute(&state.db_pool)
    .await
    .map_err(|e| AppError {
        code: ApiErrorCode::InternalError,
        context: Some(e.to_string()),
        request_id: Some(request_id.0.clone()),
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError {
            code: ApiErrorCode::JobNotFound,
            context: Some("Job not found or already completed".to_string()),
            request_id: Some(request_id.0.clone()),
        });
    }

    Ok(StatusCode::NO_CONTENT)
}
```

### 3.3 Query Endpoints

```rust
// src/api/handlers/query.rs

pub async fn query_results(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthUser>,
    axum::extract::Query(params): axum::extract::Query<OcrResultsQuery>,
) -> Result<Json<Vec<OcrResult>>, AppError> {
    let limit = params.limit.unwrap_or(50).min(100);
    let offset = params.offset.unwrap_or(0);

    let mut query_builder = sqlx::QueryBuilder::new(
        "SELECT * FROM ocr_results WHERE user_id = "
    );
    query_builder.push_bind(user.id);

    if let Some(start_date) = params.start_date {
        query_builder.push(" AND created_at >= ");
        query_builder.push_bind(start_date);
    }

    if let Some(end_date) = params.end_date {
        query_builder.push(" AND created_at <= ");
        query_builder.push_bind(end_date);
    }

    if let Some(status) = params.status {
        query_builder.push(" AND status = ");
        query_builder.push_bind(serde_json::to_value(&status).unwrap());
    }

    query_builder.push(" ORDER BY created_at DESC LIMIT ");
    query_builder.push_bind(limit as i64);
    query_builder.push(" OFFSET ");
    query_builder.push_bind(offset as i64);

    let results = query_builder
        .build_query_as::<OcrResult>()
        .fetch_all(&state.db_pool)
        .await
        .map_err(|e| AppError {
            code: ApiErrorCode::InternalError,
            context: Some(e.to_string()),
            request_id: None,
        })?;

    Ok(Json(results))
}

pub async fn get_usage_stats(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthUser>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<UsageStats>, AppError> {
    let period = params.get("period").map(|s| s.as_str()).unwrap_or("month");

    let start_date = match period {
        "day" => chrono::Utc::now() - chrono::Duration::days(1),
        "week" => chrono::Utc::now() - chrono::Duration::weeks(1),
        "month" => chrono::Utc::now() - chrono::Duration::days(30),
        _ => chrono::Utc::now() - chrono::Duration::days(30),
    };

    let stats = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as total_requests,
            COUNT(*) FILTER (WHERE status = 'completed') as successful_requests,
            COUNT(*) FILTER (WHERE status = 'failed') as failed_requests,
            SUM(processing_time_ms) as total_processing_time_ms,
            AVG(processing_time_ms) as average_processing_time_ms
        FROM ocr_results
        WHERE user_id = $1 AND created_at >= $2
        "#,
        user.id,
        start_date,
    )
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| AppError {
        code: ApiErrorCode::InternalError,
        context: Some(e.to_string()),
        request_id: None,
    })?;

    // Get requests by endpoint
    let endpoint_stats = sqlx::query!(
        r#"
        SELECT endpoint, COUNT(*) as count
        FROM ocr_results
        WHERE user_id = $1 AND created_at >= $2
        GROUP BY endpoint
        "#,
        user.id,
        start_date,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError {
        code: ApiErrorCode::InternalError,
        context: Some(e.to_string()),
        request_id: None,
    })?;

    let mut requests_by_endpoint = HashMap::new();
    for stat in endpoint_stats {
        requests_by_endpoint.insert(stat.endpoint, stat.count as u64);
    }

    Ok(Json(UsageStats {
        period: period.to_string(),
        total_requests: stats.total_requests.unwrap_or(0) as u64,
        successful_requests: stats.successful_requests.unwrap_or(0) as u64,
        failed_requests: stats.failed_requests.unwrap_or(0) as u64,
        total_processing_time_ms: stats.total_processing_time_ms.unwrap_or(0) as u64,
        average_processing_time_ms: stats.average_processing_time_ms.unwrap_or(0.0),
        requests_by_endpoint,
    }))
}
```

---

## 4. Job Queue & Background Processing

### 4.1 Redis-based Job Queue

```rust
// src/api/queue.rs
use redis::AsyncCommands;

pub struct JobQueue {
    redis: redis::aio::ConnectionManager,
    queue_key: String,
}

impl JobQueue {
    pub fn new(redis: redis::aio::ConnectionManager) -> Self {
        Self {
            redis,
            queue_key: "pdf_jobs:queue".to_string(),
        }
    }

    pub async fn enqueue(&self, job: PdfJob) -> Result<(), redis::RedisError> {
        let job_json = serde_json::to_string(&job).unwrap();
        let mut conn = self.redis.clone();
        conn.rpush(&self.queue_key, job_json).await?;
        Ok(())
    }

    pub async fn dequeue(&self) -> Result<Option<PdfJob>, redis::RedisError> {
        let mut conn = self.redis.clone();
        let job_json: Option<String> = conn.lpop(&self.queue_key, None).await?;

        Ok(job_json.and_then(|json| serde_json::from_str(&json).ok()))
    }

    pub async fn queue_length(&self) -> Result<usize, redis::RedisError> {
        let mut conn = self.redis.clone();
        conn.llen(&self.queue_key).await
    }
}

// Worker process
pub struct PdfWorker {
    queue: Arc<JobQueue>,
    db_pool: sqlx::PgPool,
    model_manager: Arc<ModelManager>,
    storage_path: String,
}

impl PdfWorker {
    pub async fn run(&self) {
        loop {
            match self.process_next_job().await {
                Ok(true) => {
                    tracing::info!("Job processed successfully");
                }
                Ok(false) => {
                    // No jobs in queue, sleep
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
                Err(e) => {
                    tracing::error!("Job processing error: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn process_next_job(&self) -> Result<bool, Box<dyn std::error::Error>> {
        let job = match self.queue.dequeue().await? {
            Some(job) => job,
            None => return Ok(false),
        };

        tracing::info!("Processing PDF job: {}", job.id);

        // Update status to processing
        self.update_job_status(&job.id, JobStatus::Processing, 0.0).await?;

        // Process PDF
        match self.process_pdf(&job).await {
            Ok(result_url) => {
                // Update status to completed
                sqlx::query!(
                    r#"
                    UPDATE pdf_jobs
                    SET status = $1, result_url = $2, completed_at = $3, updated_at = $4, progress = 1.0
                    WHERE id = $5
                    "#,
                    serde_json::to_value(&JobStatus::Completed).unwrap(),
                    result_url,
                    chrono::Utc::now(),
                    chrono::Utc::now(),
                    job.id,
                )
                .execute(&self.db_pool)
                .await?;

                // Send callback
                if let Some(callback_url) = job.callback_url {
                    self.send_completion_callback(&callback_url, &job.id, &result_url).await?;
                }

                Ok(true)
            }
            Err(e) => {
                // Update status to failed
                sqlx::query!(
                    r#"
                    UPDATE pdf_jobs
                    SET status = $1, error = $2, updated_at = $3
                    WHERE id = $4
                    "#,
                    serde_json::to_value(&JobStatus::Failed).unwrap(),
                    e.to_string(),
                    chrono::Utc::now(),
                    job.id,
                )
                .execute(&self.db_pool)
                .await?;

                Err(e)
            }
        }
    }

    async fn process_pdf(&self, job: &PdfJob) -> Result<String, Box<dyn std::error::Error>> {
        // Process PDF with model manager
        let result = self.model_manager
            .process_pdf(&job.pdf_data, &job.conversion_format, &job.options)
            .await?;

        // Save result to storage
        let result_filename = format!("{}.{}", job.id, job.conversion_format);
        let result_path = format!("{}/{}", self.storage_path, result_filename);

        tokio::fs::write(&result_path, result).await?;

        // Return public URL
        Ok(format!("/results/{}", result_filename))
    }

    async fn update_job_status(
        &self,
        job_id: &str,
        status: JobStatus,
        progress: f32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE pdf_jobs
            SET status = $1, progress = $2, updated_at = $3
            WHERE id = $4
            "#,
            serde_json::to_value(&status).unwrap(),
            progress,
            chrono::Utc::now(),
            job_id,
        )
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    async fn send_completion_callback(
        &self,
        callback_url: &str,
        job_id: &str,
        result_url: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        client
            .post(callback_url)
            .json(&serde_json::json!({
                "pdf_id": job_id,
                "status": "completed",
                "result_url": result_url,
            }))
            .send()
            .await?;

        Ok(())
    }
}
```

---

## 5. Authentication Service

```rust
// src/api/auth.rs
use sha2::{Sha256, Digest};

#[derive(Clone)]
pub struct AuthUser {
    pub id: i64,
    pub app_id: String,
    pub email: String,
    pub rate_limit: u64,
    pub quota_remaining: i64,
}

pub struct AuthService {
    db_pool: sqlx::PgPool,
}

impl AuthService {
    pub fn new(db_pool: sqlx::PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn validate_credentials(
        &self,
        app_id: &str,
        app_key: &str,
    ) -> Result<AuthUser, Box<dyn std::error::Error>> {
        // Hash the app_key
        let mut hasher = Sha256::new();
        hasher.update(app_key.as_bytes());
        let key_hash = format!("{:x}", hasher.finalize());

        // Query database
        let user = sqlx::query_as!(
            AuthUser,
            r#"
            SELECT id, app_id, email, rate_limit, quota_remaining
            FROM users
            WHERE app_id = $1 AND app_key_hash = $2 AND active = true
            "#,
            app_id,
            key_hash,
        )
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or("Invalid credentials")?;

        Ok(user)
    }

    pub async fn validate_token(
        &self,
        token: &str,
    ) -> Result<AuthUser, Box<dyn std::error::Error>> {
        // Decode JWT token
        let claims = decode_jwt(token)?;

        // Query user
        let user = sqlx::query_as!(
            AuthUser,
            r#"
            SELECT id, app_id, email, rate_limit, quota_remaining
            FROM users
            WHERE id = $1 AND active = true
            "#,
            claims.user_id,
        )
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or("Invalid token")?;

        Ok(user)
    }

    pub async fn generate_token(
        &self,
        user_id: i64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Generate JWT token
        let claims = JwtClaims {
            user_id,
            exp: (chrono::Utc::now() + chrono::Duration::days(30)).timestamp() as usize,
        };

        encode_jwt(&claims)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    user_id: i64,
    exp: usize,
}

fn encode_jwt(claims: &JwtClaims) -> Result<String, Box<dyn std::error::Error>> {
    use jsonwebtoken::{encode, Header, EncodingKey};

    let secret = std::env::var("JWT_SECRET")?;
    let token = encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}

fn decode_jwt(token: &str) -> Result<JwtClaims, Box<dyn std::error::Error>> {
    use jsonwebtoken::{decode, Validation, DecodingKey};

    let secret = std::env::var("JWT_SECRET")?;
    let token_data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;

    Ok(token_data.claims)
}
```

---

## 6. Configuration

### 6.1 Server Configuration

```rust
// config/server.toml
[server]
host = "0.0.0.0"
port = 8080
max_upload_size = 10485760  # 10MB
request_timeout = 300       # 5 minutes
enable_tls = false
# tls_cert_path = "/path/to/cert.pem"
# tls_key_path = "/path/to/key.pem"

[storage]
model_path = "./models"
storage_path = "./storage/results"

[database]
url = "postgres://user:pass@localhost/ruvector"
max_connections = 20

[redis]
url = "redis://localhost:6379"

[rate_limiting]
default_rate_limit = 100  # requests per minute
default_quota = 10000     # requests per month

[workers]
pdf_workers = 4
cleanup_interval = 3600  # 1 hour

[features]
enable_webhooks = true
enable_streaming = true
enable_pdf_processing = true
```

### 6.2 Loading Configuration

```rust
// src/config.rs
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub rate_limiting: RateLimitConfig,
    pub workers: WorkerConfig,
    pub features: FeatureConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StorageConfig {
    pub model_path: String,
    pub storage_path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RateLimitConfig {
    pub default_rate_limit: u64,
    pub default_quota: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WorkerConfig {
    pub pdf_workers: usize,
    pub cleanup_interval: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FeatureConfig {
    pub enable_webhooks: bool,
    pub enable_streaming: bool,
    pub enable_pdf_processing: bool,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }
}
```

---

## 7. OpenAPI Specification

### 7.1 OpenAPI Schema

```yaml
# openapi.yaml
openapi: 3.0.3
info:
  title: RuVector Scipix API
  description: OCR and document processing API compatible with Scipix v3
  version: 1.0.0
  contact:
    name: API Support
    email: support@ruvector.io

servers:
  - url: https://api.ruvector.io/v3
    description: Production server
  - url: http://localhost:8080/v3
    description: Development server

security:
  - BearerAuth: []
  - ApiKeyAuth: []

components:
  securitySchemes:
    BearerAuth:
      type: http
      scheme: bearer
      bearerFormat: JWT

    ApiKeyAuth:
      type: apiKey
      in: header
      name: app_id
      description: Requires both app_id and app_key headers

  schemas:
    TextRequest:
      type: object
      required:
        - src
      properties:
        src:
          type: string
          description: Image source (base64, data URL, or HTTP URL)
        include_latex:
          type: boolean
          default: false
        include_mathml:
          type: boolean
          default: false
        include_confidence:
          type: boolean
          default: false
        include_geometry:
          type: boolean
          default: false
        alphabets:
          type: array
          items:
            type: string
          example: ["en", "es"]
        callback_url:
          type: string
          format: uri

    TextResponse:
      type: object
      properties:
        request_id:
          type: string
          format: uuid
        text:
          type: string
        latex:
          type: string
        mathml:
          type: string
        confidence:
          type: number
          format: float
        geometry:
          type: array
          items:
            $ref: '#/components/schemas/BoundingBox'
        processing_time_ms:
          type: integer

    BoundingBox:
      type: object
      properties:
        x:
          type: number
        y:
          type: number
        width:
          type: number
        height:
          type: number
        text:
          type: string
        confidence:
          type: number

    PdfRequest:
      type: object
      required:
        - src
        - conversion_format
      properties:
        src:
          type: string
        conversion_format:
          type: string
          enum: [mmd, docx, html, latex]
        enable_tables_fallback:
          type: boolean
        callback_url:
          type: string

    PdfJobResponse:
      type: object
      properties:
        pdf_id:
          type: string
          format: uuid
        status:
          type: string
          enum: [queued, processing, completed, failed, cancelled]
        created_at:
          type: string
          format: date-time
        estimated_completion_time:
          type: integer

    Error:
      type: object
      properties:
        error:
          type: string
        error_code:
          type: string
        message:
          type: string
        request_id:
          type: string

paths:
  /text:
    post:
      summary: Process image OCR
      tags:
        - OCR
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/TextRequest'
          multipart/form-data:
            schema:
              type: object
              properties:
                file:
                  type: string
                  format: binary
                options:
                  type: string
                  description: JSON-encoded options
      responses:
        '200':
          description: Success
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/TextResponse'
        '400':
          description: Bad request
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Error'
        '401':
          description: Unauthorized
        '429':
          description: Rate limit exceeded

  /pdf:
    post:
      summary: Submit PDF for processing
      tags:
        - PDF
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/PdfRequest'
      responses:
        '202':
          description: Job accepted
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/PdfJobResponse'

  /pdf/{id}:
    get:
      summary: Get PDF job status
      tags:
        - PDF
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
      responses:
        '200':
          description: Job status

    delete:
      summary: Cancel PDF job
      tags:
        - PDF
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
      responses:
        '204':
          description: Job cancelled

  /ocr-results:
    get:
      summary: Query OCR results
      tags:
        - Query
      parameters:
        - name: limit
          in: query
          schema:
            type: integer
            default: 50
        - name: offset
          in: query
          schema:
            type: integer
            default: 0
      responses:
        '200':
          description: Results list

  /ocr-usage:
    get:
      summary: Get usage statistics
      tags:
        - Query
      parameters:
        - name: period
          in: query
          schema:
            type: string
            enum: [day, week, month]
      responses:
        '200':
          description: Usage stats
```

---

## 8. Database Schema

```sql
-- migrations/001_initial.sql

-- Users table
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    app_id VARCHAR(64) UNIQUE NOT NULL,
    app_key_hash VARCHAR(64) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    active BOOLEAN DEFAULT true,
    rate_limit BIGINT DEFAULT 100,
    quota_remaining BIGINT DEFAULT 10000,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_users_app_id ON users(app_id);
CREATE INDEX idx_users_email ON users(email);

-- PDF jobs table
CREATE TABLE pdf_jobs (
    id VARCHAR(64) PRIMARY KEY,
    user_id BIGINT REFERENCES users(id),
    status JSONB NOT NULL,
    conversion_format VARCHAR(32) NOT NULL,
    options JSONB,
    callback_url TEXT,
    result_url TEXT,
    error TEXT,
    progress FLOAT DEFAULT 0.0,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_pdf_jobs_user_id ON pdf_jobs(user_id);
CREATE INDEX idx_pdf_jobs_status ON pdf_jobs((status->>'status'));
CREATE INDEX idx_pdf_jobs_created_at ON pdf_jobs(created_at);

-- OCR results table
CREATE TABLE ocr_results (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT REFERENCES users(id),
    request_id VARCHAR(64) UNIQUE NOT NULL,
    endpoint VARCHAR(64) NOT NULL,
    status VARCHAR(32) NOT NULL,
    processing_time_ms BIGINT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_ocr_results_user_id ON ocr_results(user_id);
CREATE INDEX idx_ocr_results_created_at ON ocr_results(created_at);
CREATE INDEX idx_ocr_results_endpoint ON ocr_results(endpoint);
```

---

## 9. Main Application Entry

```rust
// src/main.rs
use clap::Parser;

#[derive(Parser)]
#[command(name = "ruvector-api")]
#[command(about = "RuVector Scipix API Server")]
struct Cli {
    #[arg(short, long, default_value = "config/server.toml")]
    config: String,

    #[arg(long)]
    workers: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    // Load configuration
    let config = Config::from_file(&cli.config)?;

    // Start PDF workers
    let worker_count = cli.workers.unwrap_or(config.workers.pdf_workers);
    for i in 0..worker_count {
        let config = config.clone();
        tokio::spawn(async move {
            tracing::info!("Starting PDF worker {}", i);
            let worker = PdfWorker::new(config).await.unwrap();
            worker.run().await;
        });
    }

    // Start API server
    let server = ApiServer::new(config.server).await?;
    server.serve().await?;

    Ok(())
}
```

---

## 10. Cargo Dependencies

```toml
# Cargo.toml additions for API server
[dependencies]
# Web framework
axum = "0.7"
axum-server = { version = "0.6", features = ["tls-rustls"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace", "compression", "fs"] }
tower-governor = "0.3"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Database
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "chrono", "uuid"] }
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }

# Auth
jsonwebtoken = "9"
sha2 = "0.10"
bcrypt = "0.15"

# HTTP client
reqwest = { version = "0.11", features = ["json", "multipart"] }

# Utilities
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
base64 = "0.21"
bytes = "1"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# CLI
clap = { version = "4", features = ["derive"] }
```

---

## Summary

This API server design provides:

1. **Full Scipix v3 compatibility** - All major endpoints implemented
2. **Production-ready architecture** - Async processing, rate limiting, auth
3. **Scalable design** - Worker pool, Redis queue, PostgreSQL storage
4. **Type safety** - Leveraging Rust's type system with Serde
5. **Performance** - Axum + Tokio for high-throughput async I/O
6. **Observability** - Structured logging, metrics, request tracing
7. **Security** - JWT/API key auth, input validation, rate limiting
8. **Developer experience** - OpenAPI spec, clear error codes

The server can be extended with:
- WebSocket support for real-time updates
- GraphQL endpoint for flexible queries
- Prometheus metrics export
- Distributed tracing (OpenTelemetry)
- Multi-region deployment support
