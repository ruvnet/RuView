//! Node.js bindings for Tiny Dancer neural routing via NAPI-RS
//!
//! High-performance Rust neural routing with zero-copy buffer sharing,
//! async/await support, and complete TypeScript type definitions.

#![deny(clippy::all)]
#![warn(clippy::pedantic)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use parking_lot::RwLock;
use ruvector_tiny_dancer_core::{
    types::{
        Candidate as CoreCandidate, RouterConfig as CoreRouterConfig,
        RoutingDecision as CoreRoutingDecision, RoutingRequest as CoreRoutingRequest,
        RoutingResponse as CoreRoutingResponse,
    },
    Router as CoreRouter,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Router configuration
#[napi(object)]
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Model path
    pub model_path: String,
    /// Confidence threshold (0.0 to 1.0)
    pub confidence_threshold: Option<f64>,
    /// Maximum uncertainty (0.0 to 1.0)
    pub max_uncertainty: Option<f64>,
    /// Enable circuit breaker
    pub enable_circuit_breaker: Option<bool>,
    /// Circuit breaker threshold
    pub circuit_breaker_threshold: Option<u32>,
    /// Enable quantization
    pub enable_quantization: Option<bool>,
    /// Database path
    pub database_path: Option<String>,
}

impl From<RouterConfig> for CoreRouterConfig {
    fn from(config: RouterConfig) -> Self {
        CoreRouterConfig {
            model_path: config.model_path,
            confidence_threshold: config.confidence_threshold.unwrap_or(0.85) as f32,
            max_uncertainty: config.max_uncertainty.unwrap_or(0.15) as f32,
            enable_circuit_breaker: config.enable_circuit_breaker.unwrap_or(true),
            circuit_breaker_threshold: config.circuit_breaker_threshold.unwrap_or(5),
            enable_quantization: config.enable_quantization.unwrap_or(true),
            database_path: config.database_path,
        }
    }
}

/// Candidate for routing
#[napi(object)]
#[derive(Clone)]
pub struct Candidate {
    /// Candidate ID
    pub id: String,
    /// Embedding vector
    pub embedding: Float32Array,
    /// Metadata (JSON string)
    pub metadata: Option<String>,
    /// Creation timestamp
    pub created_at: Option<i64>,
    /// Access count
    pub access_count: Option<u32>,
    /// Success rate (0.0 to 1.0)
    pub success_rate: Option<f64>,
}

impl Candidate {
    fn to_core(&self) -> Result<CoreCandidate> {
        let metadata: HashMap<String, serde_json::Value> = if let Some(ref meta_str) = self.metadata
        {
            serde_json::from_str(meta_str)
                .map_err(|e| Error::from_reason(format!("Invalid metadata JSON: {}", e)))?
        } else {
            HashMap::new()
        };

        Ok(CoreCandidate {
            id: self.id.clone(),
            embedding: self.embedding.to_vec(),
            metadata,
            created_at: self
                .created_at
                .unwrap_or_else(|| chrono::Utc::now().timestamp()),
            access_count: self.access_count.unwrap_or(0) as u64,
            success_rate: self.success_rate.unwrap_or(0.0) as f32,
        })
    }
}

/// Routing request
#[napi(object)]
pub struct RoutingRequest {
    /// Query embedding
    pub query_embedding: Float32Array,
    /// Candidates to score
    pub candidates: Vec<Candidate>,
    /// Optional metadata (JSON string)
    pub metadata: Option<String>,
}

impl RoutingRequest {
    fn to_core(&self) -> Result<CoreRoutingRequest> {
        let candidates: Result<Vec<CoreCandidate>> =
            self.candidates.iter().map(|c| c.to_core()).collect();

        let metadata = if let Some(ref meta_str) = self.metadata {
            Some(
                serde_json::from_str(meta_str)
                    .map_err(|e| Error::from_reason(format!("Invalid metadata JSON: {}", e)))?,
            )
        } else {
            None
        };

        Ok(CoreRoutingRequest {
            query_embedding: self.query_embedding.to_vec(),
            candidates: candidates?,
            metadata,
        })
    }
}

/// Routing decision
#[napi(object)]
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    /// Candidate ID
    pub candidate_id: String,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Whether to use lightweight model
    pub use_lightweight: bool,
    /// Uncertainty estimate (0.0 to 1.0)
    pub uncertainty: f64,
}

impl From<CoreRoutingDecision> for RoutingDecision {
    fn from(decision: CoreRoutingDecision) -> Self {
        Self {
            candidate_id: decision.candidate_id,
            confidence: decision.confidence as f64,
            use_lightweight: decision.use_lightweight,
            uncertainty: decision.uncertainty as f64,
        }
    }
}

/// Routing response
#[napi(object)]
#[derive(Debug, Clone)]
pub struct RoutingResponse {
    /// Routing decisions
    pub decisions: Vec<RoutingDecision>,
    /// Total inference time in microseconds
    pub inference_time_us: u32,
    /// Number of candidates processed
    pub candidates_processed: u32,
    /// Feature engineering time in microseconds
    pub feature_time_us: u32,
}

impl From<CoreRoutingResponse> for RoutingResponse {
    fn from(response: CoreRoutingResponse) -> Self {
        Self {
            decisions: response.decisions.into_iter().map(Into::into).collect(),
            inference_time_us: response.inference_time_us as u32,
            candidates_processed: response.candidates_processed as u32,
            feature_time_us: response.feature_time_us as u32,
        }
    }
}

/// Tiny Dancer neural router
#[napi]
pub struct Router {
    inner: Arc<RwLock<CoreRouter>>,
}

#[napi]
impl Router {
    /// Create a new router with configuration
    ///
    /// # Example
    /// ```javascript
    /// const router = new Router({
    ///   modelPath: './models/fastgrnn.safetensors',
    ///   confidenceThreshold: 0.85,
    ///   maxUncertainty: 0.15,
    ///   enableCircuitBreaker: true
    /// });
    /// ```
    #[napi(constructor)]
    pub fn new(config: RouterConfig) -> Result<Self> {
        let core_config: CoreRouterConfig = config.into();
        let router = CoreRouter::new(core_config)
            .map_err(|e| Error::from_reason(format!("Failed to create router: {}", e)))?;

        Ok(Self {
            inner: Arc::new(RwLock::new(router)),
        })
    }

    /// Route a request through the neural routing system
    ///
    /// Returns routing decisions with confidence scores and model recommendations
    ///
    /// # Example
    /// ```javascript
    /// const response = await router.route({
    ///   queryEmbedding: new Float32Array([0.1, 0.2, ...]),
    ///   candidates: [
    ///     { id: '1', embedding: new Float32Array([...]) },
    ///     { id: '2', embedding: new Float32Array([...]) }
    ///   ]
    /// });
    /// console.log('Top decision:', response.decisions[0]);
    /// console.log('Inference time:', response.inferenceTimeUs, 'μs');
    /// ```
    #[napi]
    pub async fn route(&self, request: RoutingRequest) -> Result<RoutingResponse> {
        let core_request = request.to_core()?;
        let router = self.inner.clone();

        tokio::task::spawn_blocking(move || {
            let router = router.read();
            router.route(core_request)
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task failed: {}", e)))?
        .map_err(|e| Error::from_reason(format!("Routing failed: {}", e)))
        .map(Into::into)
    }

    /// Reload the model from disk (hot-reload)
    ///
    /// # Example
    /// ```javascript
    /// await router.reloadModel();
    /// ```
    #[napi]
    pub async fn reload_model(&self) -> Result<()> {
        let router = self.inner.clone();

        tokio::task::spawn_blocking(move || {
            let router = router.read();
            router.reload_model()
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task failed: {}", e)))?
        .map_err(|e| Error::from_reason(format!("Model reload failed: {}", e)))
    }

    /// Check circuit breaker status
    ///
    /// Returns true if the circuit is closed (healthy), false if open (unhealthy)
    ///
    /// # Example
    /// ```javascript
    /// const isHealthy = router.circuitBreakerStatus();
    /// ```
    #[napi]
    pub fn circuit_breaker_status(&self) -> Option<bool> {
        let router = self.inner.read();
        router.circuit_breaker_status()
    }
}

/// Get the version of the Tiny Dancer library
#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Hello function for testing bindings
#[napi]
pub fn hello() -> String {
    "Hello from Tiny Dancer Node.js bindings!".to_string()
}
