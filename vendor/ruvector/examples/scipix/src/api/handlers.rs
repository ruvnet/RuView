use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{sse::Event, IntoResponse, Sse},
    Json,
};
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, time::Duration};
use tracing::{error, info, warn};
use validator::Validate;

use super::{
    jobs::{JobStatus, PdfJob},
    requests::{LatexRequest, PdfRequest, StrokesRequest, TextRequest},
    responses::{ErrorResponse, PdfResponse, TextResponse},
    state::AppState,
};

/// Health check handler
pub async fn get_health() -> impl IntoResponse {
    #[derive(Serialize)]
    struct Health {
        status: &'static str,
        version: &'static str,
    }

    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Process text/image OCR request
/// Supports multipart/form-data, base64, and URL inputs
///
/// # Important
/// This endpoint requires OCR models to be configured. If models are not available,
/// returns a 503 Service Unavailable error with instructions.
pub async fn process_text(
    State(_state): State<AppState>,
    Json(request): Json<TextRequest>,
) -> Result<Json<TextResponse>, ErrorResponse> {
    info!("Processing text OCR request");

    // Validate request
    request.validate().map_err(|e| {
        warn!("Invalid request: {:?}", e);
        ErrorResponse::validation_error(format!("Validation failed: {}", e))
    })?;

    // Download or decode image
    let image_data = match request.get_image_data().await {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to get image data: {:?}", e);
            return Err(ErrorResponse::internal_error("Failed to process image"));
        }
    };

    // Validate image data is not empty
    if image_data.is_empty() {
        return Err(ErrorResponse::validation_error("Image data is empty"));
    }

    // OCR processing requires models to be configured
    // Return informative error explaining how to set up the service
    Err(ErrorResponse::service_unavailable(
        "OCR service not fully configured. ONNX models are required for OCR processing. \
         Please download compatible models (PaddleOCR, TrOCR) and configure the model directory. \
         See documentation at /docs/MODEL_SETUP.md for setup instructions.",
    ))
}

/// Process digital ink strokes
///
/// # Important
/// This endpoint requires OCR models to be configured.
pub async fn process_strokes(
    State(_state): State<AppState>,
    Json(request): Json<StrokesRequest>,
) -> Result<Json<TextResponse>, ErrorResponse> {
    info!(
        "Processing strokes request with {} strokes",
        request.strokes.len()
    );

    request
        .validate()
        .map_err(|e| ErrorResponse::validation_error(format!("Validation failed: {}", e)))?;

    // Validate we have stroke data
    if request.strokes.is_empty() {
        return Err(ErrorResponse::validation_error("No strokes provided"));
    }

    // Stroke recognition requires models to be configured
    Err(ErrorResponse::service_unavailable(
        "Stroke recognition service not configured. ONNX models required for ink recognition.",
    ))
}

/// Process legacy LaTeX equation request
///
/// # Important
/// This endpoint requires OCR models to be configured.
pub async fn process_latex(
    State(_state): State<AppState>,
    Json(request): Json<LatexRequest>,
) -> Result<Json<TextResponse>, ErrorResponse> {
    info!("Processing legacy LaTeX request");

    request
        .validate()
        .map_err(|e| ErrorResponse::validation_error(format!("Validation failed: {}", e)))?;

    // LaTeX recognition requires models to be configured
    Err(ErrorResponse::service_unavailable(
        "LaTeX recognition service not configured. ONNX models required.",
    ))
}

/// Create async PDF processing job
pub async fn process_pdf(
    State(state): State<AppState>,
    Json(request): Json<PdfRequest>,
) -> Result<Json<PdfResponse>, ErrorResponse> {
    info!("Creating PDF processing job");

    request
        .validate()
        .map_err(|e| ErrorResponse::validation_error(format!("Validation failed: {}", e)))?;

    // Create job
    let job = PdfJob::new(request);
    let job_id = job.id.clone();

    // Queue job
    state.job_queue.enqueue(job).await.map_err(|e| {
        error!("Failed to enqueue job: {:?}", e);
        ErrorResponse::internal_error("Failed to create PDF job")
    })?;

    let response = PdfResponse {
        pdf_id: job_id,
        status: JobStatus::Processing,
        message: Some("PDF processing started".to_string()),
        result: None,
        error: None,
    };

    Ok(Json(response))
}

/// Get PDF job status
pub async fn get_pdf_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PdfResponse>, ErrorResponse> {
    info!("Getting PDF job status: {}", id);

    let status = state
        .job_queue
        .get_status(&id)
        .await
        .ok_or_else(|| ErrorResponse::not_found("Job not found"))?;

    let response = PdfResponse {
        pdf_id: id.clone(),
        status: status.clone(),
        message: Some(format!("Job status: {:?}", status)),
        result: state.job_queue.get_result(&id).await,
        error: state.job_queue.get_error(&id).await,
    };

    Ok(Json(response))
}

/// Delete PDF job
pub async fn delete_pdf_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ErrorResponse> {
    info!("Deleting PDF job: {}", id);

    state
        .job_queue
        .cancel(&id)
        .await
        .map_err(|_| ErrorResponse::not_found("Job not found"))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Stream PDF processing results via SSE
pub async fn stream_pdf_results(
    State(_state): State<AppState>,
    Path(_id): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("Streaming PDF results for job: {}", _id);

    let stream = stream::unfold(0, move |page| {
        async move {
            if page > 10 {
                // Example: stop after 10 pages
                return None;
            }

            tokio::time::sleep(Duration::from_millis(500)).await;

            let event = Event::default()
                .json_data(serde_json::json!({
                    "page": page,
                    "text": format!("Content from page {}", page),
                    "progress": (page as f32 / 10.0) * 100.0
                }))
                .ok()?;

            Some((Ok(event), page + 1))
        }
    });

    Sse::new(stream)
}

/// Convert document to different format (MMD/DOCX/etc)
///
/// # Note
/// Document conversion requires additional backend services to be configured.
pub async fn convert_document(
    State(_state): State<AppState>,
    Json(_request): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    info!("Converting document");

    // Document conversion is not yet implemented
    Err(ErrorResponse::not_implemented(
        "Document conversion is not yet implemented. This feature requires additional backend services."
    ))
}

/// Get OCR processing history
#[derive(Deserialize)]
pub struct HistoryQuery {
    #[serde(default)]
    page: u32,
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    50
}

/// Get OCR processing history
///
/// # Note
/// History storage requires a database backend to be configured.
/// Returns empty results if no database is available.
pub async fn get_ocr_results(
    State(_state): State<AppState>,
    Query(params): Query<HistoryQuery>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    info!(
        "Getting OCR results history: page={}, limit={}",
        params.page, params.limit
    );

    // History storage not configured - return empty results with notice
    Ok(Json(serde_json::json!({
        "results": [],
        "total": 0,
        "page": params.page,
        "limit": params.limit,
        "notice": "History storage not configured. Results are not persisted."
    })))
}

/// Get OCR usage statistics
///
/// # Note
/// Usage tracking requires a database backend to be configured.
/// Returns zeros if no database is available.
pub async fn get_ocr_usage(
    State(_state): State<AppState>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    info!("Getting OCR usage statistics");

    // Usage tracking not configured - return zeros with notice
    Ok(Json(serde_json::json!({
        "requests_today": 0,
        "requests_month": 0,
        "quota_limit": null,
        "quota_remaining": null,
        "notice": "Usage tracking not configured. Statistics are not recorded."
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let response = get_health().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
