//! RuvLLM HTTP Server Binary
//!
//! REST API server for RuvLLM inference.

#[cfg(feature = "server")]
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
#[cfg(feature = "server")]
use ruvllm::{Config, RuvLLM};
#[cfg(feature = "server")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "server")]
use std::sync::Arc;
#[cfg(feature = "server")]
use tower_http::cors::CorsLayer;
#[cfg(feature = "server")]
use tower_http::trace::TraceLayer;

#[cfg(feature = "server")]
#[derive(Clone)]
struct AppState {
    llm: Arc<RuvLLM>,
}

#[cfg(feature = "server")]
#[derive(Debug, Deserialize)]
struct QueryRequest {
    query: String,
    session_id: Option<String>,
}

#[cfg(feature = "server")]
#[derive(Debug, Serialize)]
struct QueryResponse {
    text: String,
    model_used: String,
    context_size: usize,
    confidence: f32,
    latency_ms: f64,
}

#[cfg(feature = "server")]
#[derive(Debug, Serialize)]
struct StatsResponse {
    total_queries: u64,
    cache_hits: u64,
    avg_latency_ms: f64,
    memory_nodes: usize,
    router_updates: u64,
}

#[cfg(feature = "server")]
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

#[cfg(feature = "server")]
#[derive(Debug, Deserialize)]
struct FeedbackRequest {
    query: String,
    response: String,
    quality: f32,
}

#[cfg(feature = "server")]
async fn health() -> impl IntoResponse {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[cfg(feature = "server")]
async fn query(
    State(state): State<AppState>,
    Json(req): Json<QueryRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let start = std::time::Instant::now();

    let response = if let Some(session_id) = req.session_id {
        state.llm.query_session(&session_id, &req.query).await
    } else {
        state.llm.query(&req.query).await
    };

    match response {
        Ok(resp) => {
            let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
            Ok(Json(QueryResponse {
                text: resp.text,
                model_used: format!("{:?}", resp.model_used),
                context_size: resp.context_size,
                confidence: resp.confidence,
                latency_ms,
            }))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

#[cfg(feature = "server")]
async fn stats(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.llm.stats();
    Json(StatsResponse {
        total_queries: stats.total_queries,
        cache_hits: stats.cache_hits,
        avg_latency_ms: stats.avg_latency_ms,
        memory_nodes: stats.memory_nodes,
        router_updates: stats.router_updates,
    })
}

#[cfg(feature = "server")]
async fn feedback(
    State(state): State<AppState>,
    Json(req): Json<FeedbackRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match state
        .llm
        .submit_feedback(&req.query, &req.response, req.quality)
        .await
    {
        Ok(_) => Ok(StatusCode::OK),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

#[cfg(feature = "server")]
async fn new_session(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "session_id": state.llm.new_session()
    }))
}

#[cfg(feature = "server")]
#[tokio::main]
async fn main() -> ruvllm::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("ruvllm=info".parse().unwrap())
                .add_directive("tower_http=debug".parse().unwrap()),
        )
        .init();

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║              RuvLLM HTTP Server                               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // Build configuration
    let config = Config::builder()
        .embedding_dim(768)
        .router_hidden_dim(128)
        .num_attention_heads(8)
        .learning_enabled(true)
        .build()?;

    println!("🚀 Initializing RuvLLM...");
    let llm = RuvLLM::new(config).await?;
    println!("✅ RuvLLM initialized!");

    let state = AppState { llm: Arc::new(llm) };

    // Build router
    let app = Router::new()
        .route("/health", get(health))
        .route("/query", post(query))
        .route("/stats", get(stats))
        .route("/feedback", post(feedback))
        .route("/session", post(new_session))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("🌐 Server listening on http://{}", addr);
    println!();
    println!("📖 Endpoints:");
    println!("   GET  /health   - Health check");
    println!("   POST /query    - Query the LLM");
    println!("   GET  /stats    - Get statistics");
    println!("   POST /feedback - Submit feedback");
    println!("   POST /session  - Create new session");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

#[cfg(not(feature = "server"))]
fn main() {
    eprintln!("Error: ruvllm-server requires the 'server' feature");
    eprintln!("Build with: cargo build --features server --bin ruvllm-server");
    std::process::exit(1);
}
