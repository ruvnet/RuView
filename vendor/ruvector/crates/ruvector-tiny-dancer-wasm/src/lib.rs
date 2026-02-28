//! WASM bindings for Tiny Dancer neural routing

use ruvector_tiny_dancer_core::{
    types::{
        Candidate as CoreCandidate, RouterConfig as CoreRouterConfig,
        RoutingRequest as CoreRoutingRequest, RoutingResponse as CoreRoutingResponse,
    },
    Router as CoreRouter,
};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

/// Initialize panic hook for better error messages in WASM
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Router configuration for WASM
#[wasm_bindgen]
#[derive(Clone)]
pub struct RouterConfig {
    model_path: String,
    confidence_threshold: f32,
    max_uncertainty: f32,
    enable_circuit_breaker: bool,
    circuit_breaker_threshold: u32,
    enable_quantization: bool,
}

#[wasm_bindgen]
impl RouterConfig {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            model_path: "./models/fastgrnn.safetensors".to_string(),
            confidence_threshold: 0.85,
            max_uncertainty: 0.15,
            enable_circuit_breaker: true,
            circuit_breaker_threshold: 5,
            enable_quantization: true,
        }
    }

    #[wasm_bindgen(setter)]
    pub fn set_model_path(&mut self, path: String) {
        self.model_path = path;
    }

    #[wasm_bindgen(setter)]
    pub fn set_confidence_threshold(&mut self, threshold: f32) {
        self.confidence_threshold = threshold;
    }

    #[wasm_bindgen(setter)]
    pub fn set_max_uncertainty(&mut self, uncertainty: f32) {
        self.max_uncertainty = uncertainty;
    }
}

impl From<RouterConfig> for CoreRouterConfig {
    fn from(config: RouterConfig) -> Self {
        CoreRouterConfig {
            model_path: config.model_path,
            confidence_threshold: config.confidence_threshold,
            max_uncertainty: config.max_uncertainty,
            enable_circuit_breaker: config.enable_circuit_breaker,
            circuit_breaker_threshold: config.circuit_breaker_threshold,
            enable_quantization: config.enable_quantization,
            database_path: None,
        }
    }
}

/// Candidate for routing
#[wasm_bindgen]
pub struct Candidate {
    id: String,
    embedding: Vec<f32>,
    metadata: String,
    created_at: i64,
    access_count: u64,
    success_rate: f32,
}

#[wasm_bindgen]
impl Candidate {
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: String,
        embedding: Vec<f32>,
        metadata: String,
        created_at: i64,
        access_count: u64,
        success_rate: f32,
    ) -> Self {
        Self {
            id,
            embedding,
            metadata,
            created_at,
            access_count,
            success_rate,
        }
    }
}

impl TryFrom<Candidate> for CoreCandidate {
    type Error = JsValue;

    fn try_from(candidate: Candidate) -> Result<Self, Self::Error> {
        let metadata: HashMap<String, serde_json::Value> =
            serde_json::from_str(&candidate.metadata)
                .map_err(|e| JsValue::from_str(&format!("Invalid metadata: {}", e)))?;

        Ok(CoreCandidate {
            id: candidate.id,
            embedding: candidate.embedding,
            metadata,
            created_at: candidate.created_at,
            access_count: candidate.access_count,
            success_rate: candidate.success_rate,
        })
    }
}

/// Routing request
#[wasm_bindgen]
pub struct RoutingRequest {
    query_embedding: Vec<f32>,
    candidates: Vec<Candidate>,
    metadata: Option<String>,
}

#[wasm_bindgen]
impl RoutingRequest {
    #[wasm_bindgen(constructor)]
    pub fn new(query_embedding: Vec<f32>, candidates: Vec<Candidate>) -> Self {
        Self {
            query_embedding,
            candidates,
            metadata: None,
        }
    }

    #[wasm_bindgen(setter)]
    pub fn set_metadata(&mut self, metadata: String) {
        self.metadata = Some(metadata);
    }
}

impl TryFrom<RoutingRequest> for CoreRoutingRequest {
    type Error = JsValue;

    fn try_from(request: RoutingRequest) -> Result<Self, Self::Error> {
        let candidates: Result<Vec<CoreCandidate>, JsValue> = request
            .candidates
            .into_iter()
            .map(|c| c.try_into())
            .collect();

        let metadata = if let Some(meta_str) = request.metadata {
            Some(
                serde_json::from_str(&meta_str)
                    .map_err(|e| JsValue::from_str(&format!("Invalid metadata: {}", e)))?,
            )
        } else {
            None
        };

        Ok(CoreRoutingRequest {
            query_embedding: request.query_embedding,
            candidates: candidates?,
            metadata,
        })
    }
}

/// Routing response
#[wasm_bindgen]
pub struct RoutingResponse {
    decisions_json: String,
    inference_time_us: u64,
    candidates_processed: usize,
    feature_time_us: u64,
}

#[wasm_bindgen]
impl RoutingResponse {
    #[wasm_bindgen(getter)]
    pub fn decisions_json(&self) -> String {
        self.decisions_json.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn inference_time_us(&self) -> u64 {
        self.inference_time_us
    }

    #[wasm_bindgen(getter)]
    pub fn candidates_processed(&self) -> usize {
        self.candidates_processed
    }

    #[wasm_bindgen(getter)]
    pub fn feature_time_us(&self) -> u64 {
        self.feature_time_us
    }
}

impl From<CoreRoutingResponse> for RoutingResponse {
    fn from(response: CoreRoutingResponse) -> Self {
        let decisions_json = serde_json::to_string(&response.decisions).unwrap_or_default();

        Self {
            decisions_json,
            inference_time_us: response.inference_time_us,
            candidates_processed: response.candidates_processed,
            feature_time_us: response.feature_time_us,
        }
    }
}

/// Tiny Dancer router for WASM
#[wasm_bindgen]
pub struct Router {
    inner: CoreRouter,
}

#[wasm_bindgen]
impl Router {
    /// Create a new router with configuration
    #[wasm_bindgen(constructor)]
    pub fn new(config: RouterConfig) -> Result<Router, JsValue> {
        let core_config: CoreRouterConfig = config.into();
        let router = CoreRouter::new(core_config)
            .map_err(|e| JsValue::from_str(&format!("Failed to create router: {}", e)))?;

        Ok(Router { inner: router })
    }

    /// Route a request
    pub fn route(&self, request: RoutingRequest) -> Result<RoutingResponse, JsValue> {
        let core_request: CoreRoutingRequest = request.try_into()?;
        let core_response = self
            .inner
            .route(core_request)
            .map_err(|e| JsValue::from_str(&format!("Routing failed: {}", e)))?;

        Ok(core_response.into())
    }

    /// Check circuit breaker status
    pub fn circuit_breaker_status(&self) -> Option<bool> {
        self.inner.circuit_breaker_status()
    }
}

/// Get library version
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!version().is_empty());
    }
}
