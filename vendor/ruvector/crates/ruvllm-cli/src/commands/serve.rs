//! Inference server command implementation
//!
//! Starts an OpenAI-compatible HTTP server for model inference,
//! providing endpoints for chat completions, health checks, and metrics.
//! Supports Server-Sent Events (SSE) for streaming responses.

use anyhow::{Context, Result};
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Router,
};
use colored::Colorize;
use console::style;
use futures::stream::{self, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::models::{resolve_model_id, QuantPreset};

/// Server state
struct ServerState {
    model_id: String,
    backend: Option<Box<dyn ruvllm::LlmBackend>>,
    request_count: u64,
    total_tokens: u64,
    start_time: Instant,
}

type SharedState = Arc<RwLock<ServerState>>;

/// Run the serve command
pub async fn run(
    model: &str,
    host: &str,
    port: u16,
    max_concurrent: usize,
    max_context: usize,
    quantization: &str,
    cache_dir: &str,
) -> Result<()> {
    let model_id = resolve_model_id(model);
    let quant = QuantPreset::from_str(quantization)
        .ok_or_else(|| anyhow::anyhow!("Invalid quantization format: {}", quantization))?;

    println!();
    println!("{}", style("RuvLLM Inference Server").bold().cyan());
    println!();
    println!("  {} {}", "Model:".dimmed(), model_id);
    println!("  {} {}", "Quantization:".dimmed(), quant);
    println!("  {} {}", "Max Concurrent:".dimmed(), max_concurrent);
    println!("  {} {}", "Max Context:".dimmed(), max_context);
    println!();

    // Initialize backend
    println!("{}", "Loading model...".yellow());

    let mut backend = ruvllm::create_backend();
    let config = ruvllm::ModelConfig {
        architecture: detect_architecture(&model_id),
        quantization: Some(map_quantization(quant)),
        max_sequence_length: max_context,
        ..Default::default()
    };

    // Try to load from cache first, then from HuggingFace
    let model_path = PathBuf::from(cache_dir).join("models").join(&model_id);
    let load_result = if model_path.exists() {
        backend.load_model(model_path.to_str().unwrap(), config.clone())
    } else {
        backend.load_model(&model_id, config)
    };

    match load_result {
        Ok(_) => {
            if let Some(info) = backend.model_info() {
                println!(
                    "{} Loaded {} ({:.1}B params, {} memory)",
                    style("Success!").green().bold(),
                    info.name,
                    info.num_parameters as f64 / 1e9,
                    bytesize::ByteSize(info.memory_usage as u64)
                );
            } else {
                println!("{} Model loaded", style("Success!").green().bold());
            }
        }
        Err(e) => {
            // Create a mock server for development/testing
            println!(
                "{} Model loading failed: {}. Running in mock mode.",
                style("Warning:").yellow().bold(),
                e
            );
        }
    }

    // Create server state
    let state = Arc::new(RwLock::new(ServerState {
        model_id: model_id.clone(),
        backend: Some(backend),
        request_count: 0,
        total_tokens: 0,
        start_time: Instant::now(),
    }));

    // Build router
    let app = Router::new()
        // OpenAI-compatible endpoints
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/models", get(list_models))
        // Health and metrics
        .route("/health", get(health_check))
        .route("/metrics", get(metrics))
        .route("/", get(root))
        // State and middleware
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http());

    // Start server
    let addr = format!("{}:{}", host, port)
        .parse::<SocketAddr>()
        .context("Invalid address")?;

    println!();
    println!("{}", style("Server ready!").bold().green());
    println!();
    println!("  {} http://{}/v1/chat/completions", "API:".cyan(), addr);
    println!("  {} http://{}/health", "Health:".cyan(), addr);
    println!("  {} http://{}/metrics", "Metrics:".cyan(), addr);
    println!();
    println!("{}", "Example curl:".dimmed());
    println!(
        r#"  curl http://{}/v1/chat/completions \
    -H "Content-Type: application/json" \
    -d '{{"model": "{}", "messages": [{{"role": "user", "content": "Hello!"}}]}}'"#,
        addr, model_id
    );
    println!();
    println!("Press Ctrl+C to stop the server.");
    println!();

    // Set up graceful shutdown
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Server error")?;

    println!();
    println!("{}", "Server stopped.".dimmed());

    Ok(())
}

/// OpenAI-compatible chat completion request
#[derive(Debug, Deserialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(default = "default_max_tokens")]
    max_tokens: usize,
    #[serde(default = "default_temperature")]
    temperature: f32,
    #[serde(default)]
    top_p: Option<f32>,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    stop: Option<Vec<String>>,
}

fn default_max_tokens() -> usize {
    512
}

fn default_temperature() -> f32 {
    0.7
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// OpenAI-compatible chat completion response
#[derive(Debug, Serialize)]
struct ChatCompletionResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<ChatChoice>,
    usage: Usage,
}

#[derive(Debug, Serialize)]
struct ChatChoice {
    index: usize,
    message: ChatMessage,
    finish_reason: String,
}

#[derive(Debug, Serialize)]
struct Usage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

/// OpenAI-compatible streaming chunk response
#[derive(Debug, Serialize)]
struct ChatCompletionChunk {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<ChunkChoice>,
}

#[derive(Debug, Serialize)]
struct ChunkChoice {
    index: usize,
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
}

/// Chat completions endpoint - handles both streaming and non-streaming
async fn chat_completions(
    State(state): State<SharedState>,
    Json(request): Json<ChatCompletionRequest>,
) -> axum::response::Response {
    if request.stream {
        // Handle streaming response
        chat_completions_stream(state, request)
            .await
            .into_response()
    } else {
        // Handle non-streaming response
        chat_completions_non_stream(state, request)
            .await
            .into_response()
    }
}

/// Non-streaming chat completions
async fn chat_completions_non_stream(
    state: SharedState,
    request: ChatCompletionRequest,
) -> impl IntoResponse {
    let start = Instant::now();

    // Build prompt from messages
    let prompt = build_prompt(&request.messages);

    // Get state for generation
    let mut state_lock = state.write().await;
    state_lock.request_count += 1;

    // Generate response
    let response_text = if let Some(backend) = &state_lock.backend {
        if backend.is_model_loaded() {
            let params = ruvllm::GenerateParams {
                max_tokens: request.max_tokens,
                temperature: request.temperature,
                top_p: request.top_p.unwrap_or(0.9),
                stop_sequences: request.stop.unwrap_or_default(),
                ..Default::default()
            };

            match backend.generate(&prompt, params) {
                Ok(text) => text,
                Err(e) => format!("Generation error: {}", e),
            }
        } else {
            // Mock response
            mock_response(&prompt)
        }
    } else {
        mock_response(&prompt)
    };

    // Calculate tokens (rough estimate)
    let prompt_tokens = prompt.split_whitespace().count();
    let completion_tokens = response_text.split_whitespace().count();
    state_lock.total_tokens += (prompt_tokens + completion_tokens) as u64;

    drop(state_lock);

    // Build response
    let response = ChatCompletionResponse {
        id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        object: "chat.completion".to_string(),
        created: chrono::Utc::now().timestamp() as u64,
        model: request.model,
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: response_text,
            },
            finish_reason: "stop".to_string(),
        }],
        usage: Usage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        },
    };

    tracing::info!(
        "Chat completion: {} tokens in {:.2}ms",
        response.usage.total_tokens,
        start.elapsed().as_secs_f64() * 1000.0
    );

    Json(response)
}

/// SSE streaming chat completions
async fn chat_completions_stream(
    state: SharedState,
    request: ChatCompletionRequest,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let completion_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = chrono::Utc::now().timestamp() as u64;
    let model = request.model.clone();

    // Build prompt from messages
    let prompt = build_prompt(&request.messages);

    // Get state and prepare for generation
    let state_clone = state.clone();
    let params = ruvllm::GenerateParams {
        max_tokens: request.max_tokens,
        temperature: request.temperature,
        top_p: request.top_p.unwrap_or(0.9),
        stop_sequences: request.stop.unwrap_or_default(),
        ..Default::default()
    };

    // Create the SSE stream
    let stream = async_stream::stream! {
        // Increment request count
        {
            let mut state_lock = state_clone.write().await;
            state_lock.request_count += 1;
        }

        // First, send the role
        let initial_chunk = ChatCompletionChunk {
            id: completion_id.clone(),
            object: "chat.completion.chunk".to_string(),
            created,
            model: model.clone(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: Delta {
                    role: Some("assistant".to_string()),
                    content: None,
                },
                finish_reason: None,
            }],
        };
        yield Ok(Event::default().data(serde_json::to_string(&initial_chunk).unwrap_or_default()));

        // Get the backend and generate
        let state_lock = state_clone.read().await;
        let backend_opt = state_lock.backend.as_ref();

        if let Some(backend) = backend_opt {
            if backend.is_model_loaded() {
                // Use streaming generation
                match backend.generate_stream_v2(&prompt, params.clone()) {
                    Ok(token_stream) => {
                        // Need to drop the read lock before iterating
                        drop(state_lock);

                        for event_result in token_stream {
                            match event_result {
                                Ok(ruvllm::StreamEvent::Token(token)) => {
                                    let chunk = ChatCompletionChunk {
                                        id: completion_id.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        created,
                                        model: model.clone(),
                                        choices: vec![ChunkChoice {
                                            index: 0,
                                            delta: Delta {
                                                role: None,
                                                content: Some(token.text),
                                            },
                                            finish_reason: None,
                                        }],
                                    };
                                    yield Ok(Event::default().data(serde_json::to_string(&chunk).unwrap_or_default()));
                                }
                                Ok(ruvllm::StreamEvent::Done { total_tokens, .. }) => {
                                    // Update token count
                                    let mut state_lock = state_clone.write().await;
                                    state_lock.total_tokens += total_tokens as u64;
                                    drop(state_lock);

                                    // Send final chunk with finish_reason
                                    let final_chunk = ChatCompletionChunk {
                                        id: completion_id.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        created,
                                        model: model.clone(),
                                        choices: vec![ChunkChoice {
                                            index: 0,
                                            delta: Delta {
                                                role: None,
                                                content: None,
                                            },
                                            finish_reason: Some("stop".to_string()),
                                        }],
                                    };
                                    yield Ok(Event::default().data(serde_json::to_string(&final_chunk).unwrap_or_default()));
                                    break;
                                }
                                Ok(ruvllm::StreamEvent::Error(msg)) => {
                                    tracing::error!("Stream error: {}", msg);
                                    break;
                                }
                                Err(e) => {
                                    tracing::error!("Stream error: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        drop(state_lock);
                        tracing::error!("Failed to create stream: {}", e);
                        // Fall back to mock streaming
                        for chunk_data in mock_stream_response(&prompt, &completion_id, created, &model) {
                            yield Ok(Event::default().data(chunk_data));
                        }
                    }
                }
            } else {
                drop(state_lock);
                // Mock streaming response
                for chunk_data in mock_stream_response(&prompt, &completion_id, created, &model) {
                    yield Ok(Event::default().data(chunk_data));
                }
            }
        } else {
            drop(state_lock);
            // Mock streaming response
            for chunk_data in mock_stream_response(&prompt, &completion_id, created, &model) {
                yield Ok(Event::default().data(chunk_data));
            }
        }

        // Send [DONE] marker
        yield Ok(Event::default().data("[DONE]"));
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Generate mock streaming chunks
fn mock_stream_response(prompt: &str, id: &str, created: u64, model: &str) -> Vec<String> {
    let response_text = mock_response(prompt);
    let words: Vec<&str> = response_text.split_whitespace().collect();
    let mut chunks = Vec::new();

    for (i, word) in words.iter().enumerate() {
        let text = if i == 0 {
            word.to_string()
        } else {
            format!(" {}", word)
        };

        let chunk = ChatCompletionChunk {
            id: id.to_string(),
            object: "chat.completion.chunk".to_string(),
            created,
            model: model.to_string(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: Delta {
                    role: None,
                    content: Some(text),
                },
                finish_reason: None,
            }],
        };

        chunks.push(serde_json::to_string(&chunk).unwrap_or_default());
    }

    // Final chunk with finish_reason
    let final_chunk = ChatCompletionChunk {
        id: id.to_string(),
        object: "chat.completion.chunk".to_string(),
        created,
        model: model.to_string(),
        choices: vec![ChunkChoice {
            index: 0,
            delta: Delta {
                role: None,
                content: None,
            },
            finish_reason: Some("stop".to_string()),
        }],
    };
    chunks.push(serde_json::to_string(&final_chunk).unwrap_or_default());

    chunks
}

/// Build prompt from chat messages
fn build_prompt(messages: &[ChatMessage]) -> String {
    let mut prompt = String::new();

    for msg in messages {
        match msg.role.as_str() {
            "system" => {
                prompt.push_str(&format!("<|system|>\n{}\n", msg.content));
            }
            "user" => {
                prompt.push_str(&format!("<|user|>\n{}\n", msg.content));
            }
            "assistant" => {
                prompt.push_str(&format!("<|assistant|>\n{}\n", msg.content));
            }
            _ => {
                prompt.push_str(&format!("{}: {}\n", msg.role, msg.content));
            }
        }
    }

    prompt.push_str("<|assistant|>\n");
    prompt
}

/// Mock response for development/testing
fn mock_response(prompt: &str) -> String {
    let prompt_lower = prompt.to_lowercase();

    if prompt_lower.contains("hello") || prompt_lower.contains("hi") {
        "Hello! I'm RuvLLM, a local AI assistant running on your Mac. How can I help you today?"
            .to_string()
    } else if prompt_lower.contains("code") || prompt_lower.contains("function") {
        "Here's an example function:\n\n```rust\nfn hello() {\n    println!(\"Hello, world!\");\n}\n```\n\nWould you like me to explain this code?".to_string()
    } else {
        "I understand your request. To provide real responses, please ensure the model is properly loaded. Currently running in mock mode for development.".to_string()
    }
}

/// List available models
async fn list_models(State(state): State<SharedState>) -> impl IntoResponse {
    let state_lock = state.read().await;

    let models = serde_json::json!({
        "object": "list",
        "data": [{
            "id": state_lock.model_id,
            "object": "model",
            "owned_by": "ruvllm",
            "permission": []
        }]
    });

    Json(models)
}

/// Health check endpoint
async fn health_check(State(state): State<SharedState>) -> impl IntoResponse {
    let state_lock = state.read().await;

    let status = if state_lock
        .backend
        .as_ref()
        .map(|b| b.is_model_loaded())
        .unwrap_or(false)
    {
        "healthy"
    } else {
        "degraded"
    };

    let health = serde_json::json!({
        "status": status,
        "model": state_lock.model_id,
        "uptime_seconds": state_lock.start_time.elapsed().as_secs()
    });

    Json(health)
}

/// Metrics endpoint
async fn metrics(State(state): State<SharedState>) -> impl IntoResponse {
    let state_lock = state.read().await;
    let uptime = state_lock.start_time.elapsed();

    let metrics = serde_json::json!({
        "model": state_lock.model_id,
        "requests_total": state_lock.request_count,
        "tokens_total": state_lock.total_tokens,
        "uptime_seconds": uptime.as_secs(),
        "requests_per_second": if uptime.as_secs() > 0 {
            state_lock.request_count as f64 / uptime.as_secs() as f64
        } else {
            0.0
        },
        "tokens_per_second": if uptime.as_secs() > 0 {
            state_lock.total_tokens as f64 / uptime.as_secs() as f64
        } else {
            0.0
        }
    });

    Json(metrics)
}

/// Root endpoint
async fn root() -> impl IntoResponse {
    let info = serde_json::json!({
        "name": "RuvLLM Inference Server",
        "version": env!("CARGO_PKG_VERSION"),
        "endpoints": {
            "chat": "/v1/chat/completions",
            "models": "/v1/models",
            "health": "/health",
            "metrics": "/metrics"
        }
    });

    Json(info)
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!();
    println!("{}", "Shutting down...".yellow());
}

/// Detect model architecture from model ID
fn detect_architecture(model_id: &str) -> ruvllm::ModelArchitecture {
    let lower = model_id.to_lowercase();
    if lower.contains("mistral") {
        ruvllm::ModelArchitecture::Mistral
    } else if lower.contains("llama") {
        ruvllm::ModelArchitecture::Llama
    } else if lower.contains("phi") {
        ruvllm::ModelArchitecture::Phi
    } else if lower.contains("qwen") {
        ruvllm::ModelArchitecture::Qwen
    } else if lower.contains("gemma") {
        ruvllm::ModelArchitecture::Gemma
    } else {
        ruvllm::ModelArchitecture::Llama // Default
    }
}

/// Map our quantization preset to ruvllm quantization
fn map_quantization(quant: QuantPreset) -> ruvllm::Quantization {
    match quant {
        QuantPreset::Q4K => ruvllm::Quantization::Q4K,
        QuantPreset::Q8 => ruvllm::Quantization::Q8,
        QuantPreset::F16 => ruvllm::Quantization::F16,
        QuantPreset::None => ruvllm::Quantization::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt() {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are helpful.".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "Hello!".to_string(),
            },
        ];

        let prompt = build_prompt(&messages);
        assert!(prompt.contains("You are helpful"));
        assert!(prompt.contains("Hello"));
        assert!(prompt.ends_with("<|assistant|>\n"));
    }

    #[test]
    fn test_detect_architecture() {
        assert_eq!(
            detect_architecture("mistralai/Mistral-7B"),
            ruvllm::ModelArchitecture::Mistral
        );
        assert_eq!(
            detect_architecture("Qwen/Qwen2.5-14B"),
            ruvllm::ModelArchitecture::Qwen
        );
    }
}
