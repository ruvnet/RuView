//! # sevensense-api
//!
//! REST, GraphQL, and WebSocket API layer for 7sense bioacoustic analysis.
//!
//! This crate provides a comprehensive API for:
//! - Audio recording upload and processing
//! - Segment similarity search via vector embeddings
//! - Cluster discovery and labeling
//! - Evidence pack generation for interpretability
//! - Real-time processing status via WebSocket
//!
//! ## Architecture
//!
//! The API follows a layered architecture:
//! - **REST API** (`/api/v1/*`) - RESTful endpoints for CRUD operations
//! - **GraphQL** (`/graphql`) - Flexible query interface with subscriptions
//! - **WebSocket** (`/ws`) - Real-time updates for long-running operations
//!
//! ## Example
//!
//! ```rust,ignore
//! use sevensense_api::{AppBuilder, Config};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = Config::from_env()?;
//!     let app = AppBuilder::new(config).build().await?;
//!
//!     axum::serve(listener, app).await?;
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

pub mod error;
pub mod graphql;
pub mod openapi;
pub mod rest;
pub mod services;
pub mod websocket;

use std::sync::Arc;

use axum::Router;
use tokio::sync::broadcast;
use tower_http::{
    compression::CompressionLayer,
    trace::TraceLayer,
};

pub use services::{
    AudioPipeline, ClusterEngine, EmbeddingModel, InterpretationEngine, VectorIndex,
};

/// Crate version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Application configuration loaded from environment or config file.
#[derive(Debug, Clone)]
pub struct Config {
    /// Server host address
    pub host: String,
    /// Server port
    pub port: u16,
    /// CORS allowed origins
    pub cors_origins: Vec<String>,
    /// Rate limit requests per second
    pub rate_limit_rps: u32,
    /// Maximum upload size in bytes
    pub max_upload_size: usize,
    /// Enable GraphQL playground
    pub enable_playground: bool,
    /// API key for authentication (optional)
    pub api_key: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            cors_origins: vec!["*".to_string()],
            rate_limit_rps: 100,
            max_upload_size: 100 * 1024 * 1024, // 100MB
            enable_playground: true,
            api_key: None,
        }
    }
}

impl Config {
    /// Load configuration from environment variables.
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        Ok(Self {
            host: std::env::var("SEVENSENSE_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("SEVENSENSE_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            cors_origins: std::env::var("SEVENSENSE_CORS_ORIGINS")
                .map(|s| s.split(',').map(String::from).collect())
                .unwrap_or_else(|_| vec!["*".to_string()]),
            rate_limit_rps: std::env::var("SEVENSENSE_RATE_LIMIT")
                .ok()
                .and_then(|r| r.parse().ok())
                .unwrap_or(100),
            max_upload_size: std::env::var("SEVENSENSE_MAX_UPLOAD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100 * 1024 * 1024),
            enable_playground: std::env::var("SEVENSENSE_ENABLE_PLAYGROUND")
                .map(|s| s == "true" || s == "1")
                .unwrap_or(true),
            api_key: std::env::var("SEVENSENSE_API_KEY").ok(),
        })
    }
}

/// Processing status event for WebSocket broadcasts.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcessingEvent {
    /// Recording identifier
    pub recording_id: uuid::Uuid,
    /// Current processing status
    pub status: ProcessingStatus,
    /// Progress percentage (0.0 to 1.0)
    pub progress: f32,
    /// Optional status message
    pub message: Option<String>,
}

/// Processing status stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingStatus {
    /// Recording queued for processing
    Queued,
    /// Loading audio file
    Loading,
    /// Detecting segments
    Segmenting,
    /// Generating embeddings
    Embedding,
    /// Adding to vector index
    Indexing,
    /// Running cluster analysis
    Analyzing,
    /// Processing complete
    Complete,
    /// Processing failed
    Failed,
}

/// Shared application context accessible from all handlers.
#[derive(Clone)]
pub struct AppContext {
    /// Audio processing pipeline
    pub audio_pipeline: Arc<AudioPipeline>,
    /// Embedding model for segment vectorization
    pub embedding_model: Arc<EmbeddingModel>,
    /// Vector index for similarity search
    pub vector_index: Arc<VectorIndex>,
    /// Cluster analysis engine
    pub cluster_engine: Arc<ClusterEngine>,
    /// Interpretation engine for evidence packs
    pub interpretation_engine: Arc<InterpretationEngine>,
    /// Broadcast channel for processing events
    pub event_tx: broadcast::Sender<ProcessingEvent>,
    /// Application configuration
    pub config: Arc<Config>,
}

impl AppContext {
    /// Create a new application context with all required services.
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        // Initialize audio pipeline
        let audio_pipeline = Arc::new(AudioPipeline::new(Default::default())?);

        // Initialize embedding model
        let embedding_model = Arc::new(EmbeddingModel::new(Default::default()).await?);

        // Initialize vector index
        let vector_index = Arc::new(VectorIndex::new(Default::default())?);

        // Initialize cluster engine
        let cluster_engine = Arc::new(ClusterEngine::new(Default::default())?);

        // Initialize interpretation engine
        let interpretation_engine = Arc::new(InterpretationEngine::new(Default::default())?);

        // Create broadcast channel for events (capacity of 1024)
        let (event_tx, _) = broadcast::channel(1024);

        Ok(Self {
            audio_pipeline,
            embedding_model,
            vector_index,
            cluster_engine,
            interpretation_engine,
            event_tx,
            config: Arc::new(config),
        })
    }

    /// Get a receiver for processing events.
    #[must_use]
    pub fn subscribe_events(&self) -> broadcast::Receiver<ProcessingEvent> {
        self.event_tx.subscribe()
    }

    /// Publish a processing event.
    pub fn publish_event(&self, event: ProcessingEvent) {
        // Ignore send errors (no receivers)
        let _ = self.event_tx.send(event);
    }
}

/// Builder for constructing the application router.
pub struct AppBuilder {
    config: Config,
    context: Option<AppContext>,
}

impl AppBuilder {
    /// Create a new app builder with configuration.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            context: None,
        }
    }

    /// Set a pre-built context (useful for testing).
    #[must_use]
    pub fn with_context(mut self, context: AppContext) -> Self {
        self.context = Some(context);
        self
    }

    /// Build the complete application router.
    pub async fn build(self) -> anyhow::Result<Router> {
        // Initialize context if not provided
        let context = match self.context {
            Some(ctx) => ctx,
            None => AppContext::new(self.config.clone()).await?,
        };

        // Build REST routes
        let rest_router = rest::routes::create_router(context.clone());

        // Build GraphQL routes
        let graphql_router = graphql::create_router(context.clone());

        // Build WebSocket routes
        let ws_router = websocket::create_router(context.clone());

        // Build OpenAPI documentation routes
        let openapi_router = openapi::create_router();

        // Combine all routers
        let app = Router::new()
            .nest("/api/v1", rest_router)
            .nest("/graphql", graphql_router)
            .nest("/ws", ws_router)
            .nest("/docs", openapi_router)
            .layer(
                tower::ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CompressionLayer::new())
                    .layer(rest::middleware::cors_layer(&self.config)),
            )
            .with_state(context);

        Ok(app)
    }
}

/// Health check response.
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    /// Service status
    pub status: String,
    /// API version
    pub version: String,
    /// Server uptime in seconds
    pub uptime_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert!(config.enable_playground);
    }

    #[test]
    fn test_processing_status_serialize() {
        let status = ProcessingStatus::Embedding;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"embedding\"");
    }
}
