use axum::{
    routing::{delete, get, post},
    Router,
};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::Level;

use super::{
    handlers::{
        convert_document, delete_pdf_job, get_health, get_ocr_results, get_ocr_usage,
        get_pdf_status, process_latex, process_pdf, process_strokes, process_text,
        stream_pdf_results,
    },
    middleware::{auth_middleware, rate_limit_middleware},
    state::AppState,
};

/// Create the main application router with all routes and middleware
pub fn router(state: AppState) -> Router {
    // API v3 routes
    let api_routes = Router::new()
        // Image processing
        .route("/v3/text", post(process_text))
        // Digital ink processing
        .route("/v3/strokes", post(process_strokes))
        // Legacy equation processing
        .route("/v3/latex", post(process_latex))
        // Async PDF processing
        .route("/v3/pdf", post(process_pdf))
        .route("/v3/pdf/:id", get(get_pdf_status))
        .route("/v3/pdf/:id", delete(delete_pdf_job))
        .route("/v3/pdf/:id/stream", get(stream_pdf_results))
        // Document conversion
        .route("/v3/converter", post(convert_document))
        // History and usage
        .route("/v3/ocr-results", get(get_ocr_results))
        .route("/v3/ocr-usage", get(get_ocr_usage))
        // Apply auth and rate limiting to all API routes
        .layer(
            ServiceBuilder::new()
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    auth_middleware,
                ))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    rate_limit_middleware,
                )),
        );

    // Health check (no auth required)
    let health_routes = Router::new().route("/health", get(get_health));

    // Combine all routes
    Router::new()
        .merge(api_routes)
        .merge(health_routes)
        .layer(
            ServiceBuilder::new()
                // Tracing layer
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                        .on_response(DefaultOnResponse::new().level(Level::INFO)),
                )
                // CORS layer
                .layer(CorsLayer::permissive())
                // Compression layer
                .layer(CompressionLayer::new()),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        let state = AppState::new();
        let app = router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
