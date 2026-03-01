use anyhow::{Context, Result};
use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use clap::Args;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, warn};

use super::{OcrConfig, OcrResult};
use crate::cli::Cli;

/// Start the API server
#[derive(Args, Debug, Clone)]
pub struct ServeArgs {
    /// Port to listen on
    #[arg(
        short,
        long,
        default_value = "8080",
        env = "MATHPIX_PORT",
        help = "Port to listen on"
    )]
    pub port: u16,

    /// Host to bind to
    #[arg(
        short = 'H',
        long,
        default_value = "127.0.0.1",
        env = "MATHPIX_HOST",
        help = "Host address to bind to"
    )]
    pub host: String,

    /// Directory containing ML models
    #[arg(
        long,
        value_name = "DIR",
        help = "Directory containing ML models to preload"
    )]
    pub model_dir: Option<PathBuf>,

    /// Enable CORS
    #[arg(long, help = "Enable CORS for cross-origin requests")]
    pub cors: bool,

    /// Maximum request size in MB
    #[arg(long, default_value = "10", help = "Maximum request size in megabytes")]
    pub max_size: usize,

    /// Number of worker threads
    #[arg(
        short = 'w',
        long,
        default_value = "4",
        help = "Number of worker threads"
    )]
    pub workers: usize,
}

#[derive(Clone)]
struct AppState {
    config: Arc<OcrConfig>,
    max_size: usize,
}

pub async fn execute(args: ServeArgs, cli: &Cli) -> Result<()> {
    info!("Starting Scipix API server");

    // Load configuration
    let config = Arc::new(load_config(cli.config.as_ref())?);

    // Preload models if specified
    if let Some(model_dir) = &args.model_dir {
        info!("Preloading models from: {}", model_dir.display());
        preload_models(model_dir)?;
    }

    // Create app state
    let state = AppState {
        config,
        max_size: args.max_size * 1024 * 1024,
    };

    // Build router
    let mut app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/api/v1/ocr", post(ocr_handler))
        .route("/api/v1/batch", post(batch_handler))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    // Add CORS if enabled
    if args.cors {
        app = app.layer(CorsLayer::permissive());
        info!("CORS enabled");
    }

    // Create socket address
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .context("Invalid host/port combination")?;

    info!("Server listening on http://{}", addr);
    info!("API endpoints:");
    info!("  POST http://{}/api/v1/ocr - Single file OCR", addr);
    info!("  POST http://{}/api/v1/batch - Batch processing", addr);
    info!("  GET  http://{}/health - Health check", addr);

    // Create server
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("Failed to bind to address")?;

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Server error")?;

    info!("Server shutdown complete");
    Ok(())
}

async fn root() -> &'static str {
    "Scipix OCR API Server\n\nEndpoints:\n  POST /api/v1/ocr - Single file OCR\n  POST /api/v1/batch - Batch processing\n  GET /health - Health check"
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn ocr_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<OcrResult>, (StatusCode, String)> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            let data = field
                .bytes()
                .await
                .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

            if data.len() > state.max_size {
                return Err((
                    StatusCode::PAYLOAD_TOO_LARGE,
                    format!(
                        "File too large: {} bytes (max: {} bytes)",
                        data.len(),
                        state.max_size
                    ),
                ));
            }

            // Process the file
            let result = process_image_data(&data, &state.config)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            return Ok(Json(result));
        }
    }

    Err((StatusCode::BAD_REQUEST, "No file provided".to_string()))
}

async fn batch_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Vec<OcrResult>>, (StatusCode, String)> {
    let mut results = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();

        if name == "files" {
            let data = field
                .bytes()
                .await
                .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

            if data.len() > state.max_size {
                warn!("Skipping file: too large ({} bytes)", data.len());
                continue;
            }

            // Process the file
            match process_image_data(&data, &state.config).await {
                Ok(result) => results.push(result),
                Err(e) => warn!("Failed to process file: {}", e),
            }
        }
    }

    if results.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "No valid files processed".to_string(),
        ));
    }

    Ok(Json(results))
}

async fn process_image_data(data: &[u8], _config: &OcrConfig) -> Result<OcrResult> {
    // TODO: Implement actual OCR processing
    // For now, return a mock result

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    Ok(OcrResult {
        file: PathBuf::from("uploaded_file"),
        text: format!("OCR text from uploaded image ({} bytes)", data.len()),
        latex: Some(r"\text{Sample LaTeX}".to_string()),
        confidence: 0.92,
        processing_time_ms: 50,
        errors: Vec::new(),
    })
}

fn preload_models(model_dir: &PathBuf) -> Result<()> {
    if !model_dir.exists() {
        anyhow::bail!("Model directory not found: {}", model_dir.display());
    }

    if !model_dir.is_dir() {
        anyhow::bail!("Not a directory: {}", model_dir.display());
    }

    // TODO: Implement model preloading
    info!("Models preloaded from {}", model_dir.display());

    Ok(())
}

fn load_config(config_path: Option<&PathBuf>) -> Result<OcrConfig> {
    if let Some(path) = config_path {
        let content = std::fs::read_to_string(path).context("Failed to read config file")?;
        toml::from_str(&content).context("Failed to parse config file")
    } else {
        Ok(OcrConfig::default())
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received terminate signal");
        },
    }
}
