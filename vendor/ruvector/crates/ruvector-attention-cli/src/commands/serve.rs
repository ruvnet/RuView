use clap::Args;
use crate::config::Config;
use axum::{
    routing::{get, post},
    Router, Json, extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use ruvector_attention::{
    attention::{ScaledDotProductAttention, MultiHeadAttention},
    hyperbolic::HyperbolicAttention,
    sparse::{FlashAttention, LinearAttention},
    moe::MoEAttention,
};

#[derive(Args)]
pub struct ServeArgs {
    /// Host address
    #[arg(short = 'H', long, default_value = "0.0.0.0")]
    host: String,

    /// Port number
    #[arg(short, long, default_value = "8080")]
    port: u16,

    /// Enable CORS
    #[arg(long)]
    cors: bool,
}

struct ServerState {
    config: Config,
}

#[derive(Debug, Deserialize)]
struct AttentionRequest {
    query: Vec<Vec<f32>>,
    keys: Vec<Vec<f32>>,
    values: Vec<Vec<f32>>,
    #[serde(default)]
    num_heads: Option<usize>,
    #[serde(default)]
    num_experts: Option<usize>,
    #[serde(default)]
    top_k: Option<usize>,
    #[serde(default)]
    curvature: Option<f32>,
}

#[derive(Debug, Serialize)]
struct AttentionResponse {
    result: Vec<Vec<f32>>,
    compute_time_ms: f64,
    metadata: ResponseMetadata,
}

#[derive(Debug, Serialize)]
struct ResponseMetadata {
    attention_type: String,
    dimensions: (usize, usize),
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

pub async fn run(args: ServeArgs, config: &Config) -> anyhow::Result<()> {
    let state = Arc::new(ServerState {
        config: config.clone(),
    });

    let mut app = Router::new()
        .route("/health", get(health))
        .route("/attention/scaled_dot", post(scaled_dot_attention))
        .route("/attention/multi_head", post(multi_head_attention))
        .route("/attention/hyperbolic", post(hyperbolic_attention))
        .route("/attention/flash", post(flash_attention))
        .route("/attention/linear", post(linear_attention))
        .route("/attention/moe", post(moe_attention))
        .route("/batch", post(batch_compute))
        .with_state(state);

    if args.cors {
        app = app.layer(CorsLayer::permissive());
    }

    let addr = format!("{}:{}", args.host, args.port);
    tracing::info!("Starting server at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn scaled_dot_attention(
    State(_state): State<Arc<ServerState>>,
    Json(req): Json<AttentionRequest>,
) -> Result<Json<AttentionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start = std::time::Instant::now();

    let dim = req.query.first().map(|q| q.len()).unwrap_or(0);
    let attention = ScaledDotProductAttention::new(dim, None);

    let keys_refs: Vec<&[f32]> = req.keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = req.values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&req.query, &keys_refs, &values_refs)
        .map_err(|e| error_response(e.to_string()))?;

    let elapsed = start.elapsed();

    Ok(Json(AttentionResponse {
        result,
        compute_time_ms: elapsed.as_secs_f64() * 1000.0,
        metadata: ResponseMetadata {
            attention_type: "ScaledDotProduct".to_string(),
            dimensions: (req.query.len(), dim),
        },
    }))
}

async fn multi_head_attention(
    State(_state): State<Arc<ServerState>>,
    Json(req): Json<AttentionRequest>,
) -> Result<Json<AttentionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start = std::time::Instant::now();

    let dim = req.query.first().map(|q| q.len()).unwrap_or(0);
    let num_heads = req.num_heads.unwrap_or(8);
    let attention = MultiHeadAttention::new(dim, num_heads)
        .map_err(|e| error_response(e.to_string()))?;

    let keys_refs: Vec<&[f32]> = req.keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = req.values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&req.query, &keys_refs, &values_refs)
        .map_err(|e| error_response(e.to_string()))?;

    let elapsed = start.elapsed();

    Ok(Json(AttentionResponse {
        result,
        compute_time_ms: elapsed.as_secs_f64() * 1000.0,
        metadata: ResponseMetadata {
            attention_type: format!("MultiHead({})", num_heads),
            dimensions: (req.query.len(), dim),
        },
    }))
}

async fn hyperbolic_attention(
    State(_state): State<Arc<ServerState>>,
    Json(req): Json<AttentionRequest>,
) -> Result<Json<AttentionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start = std::time::Instant::now();

    let dim = req.query.first().map(|q| q.len()).unwrap_or(0);
    let curvature = req.curvature.unwrap_or(1.0);
    let attention = HyperbolicAttention::new(dim, curvature)
        .map_err(|e| error_response(e.to_string()))?;

    let keys_refs: Vec<&[f32]> = req.keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = req.values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&req.query, &keys_refs, &values_refs)
        .map_err(|e| error_response(e.to_string()))?;

    let elapsed = start.elapsed();

    Ok(Json(AttentionResponse {
        result,
        compute_time_ms: elapsed.as_secs_f64() * 1000.0,
        metadata: ResponseMetadata {
            attention_type: "Hyperbolic".to_string(),
            dimensions: (req.query.len(), dim),
        },
    }))
}

async fn flash_attention(
    State(_state): State<Arc<ServerState>>,
    Json(req): Json<AttentionRequest>,
) -> Result<Json<AttentionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start = std::time::Instant::now();

    let dim = req.query.first().map(|q| q.len()).unwrap_or(0);
    let attention = FlashAttention::new(dim, 64)
        .map_err(|e| error_response(e.to_string()))?;

    let keys_refs: Vec<&[f32]> = req.keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = req.values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&req.query, &keys_refs, &values_refs)
        .map_err(|e| error_response(e.to_string()))?;

    let elapsed = start.elapsed();

    Ok(Json(AttentionResponse {
        result,
        compute_time_ms: elapsed.as_secs_f64() * 1000.0,
        metadata: ResponseMetadata {
            attention_type: "Flash".to_string(),
            dimensions: (req.query.len(), dim),
        },
    }))
}

async fn linear_attention(
    State(_state): State<Arc<ServerState>>,
    Json(req): Json<AttentionRequest>,
) -> Result<Json<AttentionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start = std::time::Instant::now();

    let dim = req.query.first().map(|q| q.len()).unwrap_or(0);
    let attention = LinearAttention::new(dim)
        .map_err(|e| error_response(e.to_string()))?;

    let keys_refs: Vec<&[f32]> = req.keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = req.values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&req.query, &keys_refs, &values_refs)
        .map_err(|e| error_response(e.to_string()))?;

    let elapsed = start.elapsed();

    Ok(Json(AttentionResponse {
        result,
        compute_time_ms: elapsed.as_secs_f64() * 1000.0,
        metadata: ResponseMetadata {
            attention_type: "Linear".to_string(),
            dimensions: (req.query.len(), dim),
        },
    }))
}

async fn moe_attention(
    State(_state): State<Arc<ServerState>>,
    Json(req): Json<AttentionRequest>,
) -> Result<Json<AttentionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start = std::time::Instant::now();

    let dim = req.query.first().map(|q| q.len()).unwrap_or(0);
    let num_experts = req.num_experts.unwrap_or(4);
    let top_k = req.top_k.unwrap_or(2);
    let attention = MoEAttention::new(dim, num_experts, top_k)
        .map_err(|e| error_response(e.to_string()))?;

    let keys_refs: Vec<&[f32]> = req.keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = req.values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&req.query, &keys_refs, &values_refs)
        .map_err(|e| error_response(e.to_string()))?;

    let elapsed = start.elapsed();

    Ok(Json(AttentionResponse {
        result,
        compute_time_ms: elapsed.as_secs_f64() * 1000.0,
        metadata: ResponseMetadata {
            attention_type: format!("MoE({}/{})", top_k, num_experts),
            dimensions: (req.query.len(), dim),
        },
    }))
}

async fn batch_compute(
    State(_state): State<Arc<ServerState>>,
    Json(_req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    Err(error_response("Batch compute not yet implemented".to_string()))
}

fn error_response(message: String) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: message }),
    )
}
