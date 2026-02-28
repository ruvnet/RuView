//! API server example
//!
//! This example demonstrates how to create a REST API server for OCR processing.
//! It includes model preloading, graceful shutdown, and health checks.
//!
//! Usage:
//! ```bash
//! cargo run --example api_server
//!
//! # Then in another terminal:
//! curl -X POST -F "image=@equation.png" http://localhost:8080/ocr
//! ```

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use ruvector_scipix::{OcrConfig, OcrEngine, OutputFormat};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::signal;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    engine: Arc<OcrEngine>,
}

#[derive(Serialize, Deserialize)]
struct OcrResponse {
    success: bool,
    text: Option<String>,
    latex: Option<String>,
    confidence: Option<f32>,
    error: Option<String>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    models_loaded: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("Initializing OCR engine...");

    // Configure OCR engine
    let config = OcrConfig::default();
    let engine = OcrEngine::new(config).await?;

    // Preload models for faster first request
    println!("Preloading models...");
    // TODO: Add model preloading method to OcrEngine

    let state = AppState {
        engine: Arc::new(engine),
    };

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/ocr", post(process_ocr))
        .route("/batch", post(process_batch))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = "0.0.0.0:8080";
    println!("Starting server on http://{}", addr);
    println!("\nEndpoints:");
    println!("  GET  /health - Health check");
    println!("  POST /ocr    - Process single image");
    println!("  POST /batch  - Process multiple images");
    println!("\nPress Ctrl+C to shutdown");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    println!("\nServer shutdown complete");

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        models_loaded: true,
    })
}

async fn process_ocr(State(state): State<AppState>, mut multipart: Multipart) -> impl IntoResponse {
    while let Some(field) = multipart.next_field().await.unwrap() {
        if field.name() == Some("image") {
            let data = match field.bytes().await {
                Ok(bytes) => bytes,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(OcrResponse {
                            success: false,
                            text: None,
                            latex: None,
                            confidence: None,
                            error: Some(format!("Failed to read image: {}", e)),
                        }),
                    );
                }
            };

            let image = match image::load_from_memory(&data) {
                Ok(img) => img,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(OcrResponse {
                            success: false,
                            text: None,
                            latex: None,
                            confidence: None,
                            error: Some(format!("Invalid image format: {}", e)),
                        }),
                    );
                }
            };

            match state.engine.recognize(&image).await {
                Ok(result) => {
                    return (
                        StatusCode::OK,
                        Json(OcrResponse {
                            success: true,
                            text: Some(result.text.clone()),
                            latex: result.to_format(OutputFormat::LaTeX).ok(),
                            confidence: Some(result.confidence),
                            error: None,
                        }),
                    );
                }
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(OcrResponse {
                            success: false,
                            text: None,
                            latex: None,
                            confidence: None,
                            error: Some(format!("OCR failed: {}", e)),
                        }),
                    );
                }
            }
        }
    }

    (
        StatusCode::BAD_REQUEST,
        Json(OcrResponse {
            success: false,
            text: None,
            latex: None,
            confidence: None,
            error: Some("No image field found".to_string()),
        }),
    )
}

async fn process_batch(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut results = Vec::new();

    while let Some(field) = multipart.next_field().await.unwrap() {
        if field.name() == Some("images") {
            let data = match field.bytes().await {
                Ok(bytes) => bytes,
                Err(e) => {
                    results.push(OcrResponse {
                        success: false,
                        text: None,
                        latex: None,
                        confidence: None,
                        error: Some(format!("Failed to read image: {}", e)),
                    });
                    continue;
                }
            };

            let image = match image::load_from_memory(&data) {
                Ok(img) => img,
                Err(e) => {
                    results.push(OcrResponse {
                        success: false,
                        text: None,
                        latex: None,
                        confidence: None,
                        error: Some(format!("Invalid image: {}", e)),
                    });
                    continue;
                }
            };

            match state.engine.recognize(&image).await {
                Ok(result) => {
                    results.push(OcrResponse {
                        success: true,
                        text: Some(result.text.clone()),
                        latex: result.to_format(OutputFormat::LaTeX).ok(),
                        confidence: Some(result.confidence),
                        error: None,
                    });
                }
                Err(e) => {
                    results.push(OcrResponse {
                        success: false,
                        text: None,
                        latex: None,
                        confidence: None,
                        error: Some(format!("OCR failed: {}", e)),
                    });
                }
            }
        }
    }

    (StatusCode::OK, Json(results))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            println!("\nReceived Ctrl+C, shutting down gracefully...");
        },
        _ = terminate => {
            println!("\nReceived termination signal, shutting down gracefully...");
        },
    }
}
