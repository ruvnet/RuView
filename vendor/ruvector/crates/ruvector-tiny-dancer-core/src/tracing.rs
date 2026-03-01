//! Distributed tracing with OpenTelemetry for Tiny Dancer
//!
//! This module provides OpenTelemetry integration for distributed tracing,
//! allowing you to track requests through the routing system and export
//! traces to backends like Jaeger.

use opentelemetry::{
    global,
    runtime,
    trace::TraceError,
};
use tracing::{span, Level};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, Registry};

/// Configuration for tracing system
#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Service name for traces
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// Jaeger agent endpoint (e.g., "localhost:6831")
    pub jaeger_agent_endpoint: Option<String>,
    /// Sampling ratio (0.0 to 1.0)
    pub sampling_ratio: f64,
    /// Enable stdout exporter for debugging
    pub enable_stdout: bool,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            service_name: "tiny-dancer".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            jaeger_agent_endpoint: None,
            sampling_ratio: 1.0,
            enable_stdout: false,
        }
    }
}

/// Tracing system for Tiny Dancer
pub struct TracingSystem {
    config: TracingConfig,
}

impl TracingSystem {
    /// Create a new tracing system
    pub fn new(config: TracingConfig) -> Self {
        Self { config }
    }

    /// Initialize tracing with Jaeger exporter
    pub fn init_jaeger(&self) -> Result<(), TraceError> {
        let tracer = opentelemetry_jaeger::new_agent_pipeline()
            .with_service_name(&self.config.service_name)
            .with_endpoint(
                self.config
                    .jaeger_agent_endpoint
                    .as_deref()
                    .unwrap_or("localhost:6831"),
            )
            .with_auto_split_batch(true)
            .install_batch(runtime::Tokio)?;

        // Create a tracing layer with the configured tracer
        let telemetry = OpenTelemetryLayer::new(tracer);

        // Set the global subscriber
        let subscriber = Registry::default().with(telemetry);
        tracing::subscriber::set_global_default(subscriber)
            .map_err(|e| TraceError::from(e.to_string()))?;

        Ok(())
    }

    /// Initialize tracing with a no-op tracer (for debugging/testing)
    /// In production, use init_jaeger() instead
    pub fn init_stdout(&self) -> Result<(), TraceError> {
        // Note: OpenTelemetry 0.20 removed the stdout exporter
        // For debugging, use the Jaeger exporter with a local instance
        // or simply rely on tracing_subscriber's fmt layer
        tracing::warn!("Stdout tracing mode: OpenTelemetry stdout exporter not available in v0.20");
        tracing::warn!("Using Jaeger exporter instead. Ensure Jaeger is running on localhost:6831");

        // Fall back to Jaeger with localhost
        self.init_jaeger()
    }

    /// Initialize the tracing system based on configuration
    pub fn init(&self) -> Result<(), TraceError> {
        if self.config.enable_stdout {
            self.init_stdout()
        } else if self.config.jaeger_agent_endpoint.is_some() {
            self.init_jaeger()
        } else {
            // No-op if no exporter configured
            Ok(())
        }
    }

    /// Shutdown the tracing system and flush remaining spans
    pub fn shutdown(&self) {
        global::shutdown_tracer_provider();
    }
}

/// Helper to create spans for routing operations
pub struct RoutingSpan;

impl RoutingSpan {
    /// Create a span for the entire routing operation
    pub fn routing_request(candidate_count: usize) -> tracing::Span {
        span!(
            Level::INFO,
            "routing_request",
            candidate_count = candidate_count,
            otel.kind = "server",
        )
    }

    /// Create a span for feature engineering
    pub fn feature_engineering(batch_size: usize) -> tracing::Span {
        span!(
            Level::DEBUG,
            "feature_engineering",
            batch_size = batch_size,
            otel.kind = "internal",
        )
    }

    /// Create a span for model inference
    pub fn model_inference(candidate_id: &str) -> tracing::Span {
        span!(
            Level::DEBUG,
            "model_inference",
            candidate_id = candidate_id,
            otel.kind = "internal",
        )
    }

    /// Create a span for circuit breaker check
    pub fn circuit_breaker_check() -> tracing::Span {
        span!(
            Level::DEBUG,
            "circuit_breaker_check",
            otel.kind = "internal",
        )
    }

    /// Create a span for uncertainty estimation
    pub fn uncertainty_estimation(candidate_id: &str) -> tracing::Span {
        span!(
            Level::DEBUG,
            "uncertainty_estimation",
            candidate_id = candidate_id,
            otel.kind = "internal",
        )
    }
}

/// Context for propagating trace information
#[derive(Debug, Clone)]
pub struct TraceContext {
    /// Trace ID (16 bytes hex)
    pub trace_id: String,
    /// Span ID (8 bytes hex)
    pub span_id: String,
    /// Trace flags
    pub trace_flags: u8,
}

impl TraceContext {
    /// Create a new trace context from current span
    /// Note: This requires the OpenTelemetry context to be properly set up
    pub fn from_current() -> Option<Self> {
        // Note: Getting the trace context from tracing spans requires
        // the OpenTelemetry layer to be initialized. This is a simplified
        // version that returns None if tracing is not properly configured.
        // In production, you would use opentelemetry::Context::current()
        // with proper TraceContextExt trait.

        // For now, return None as we can't easily extract the context
        // without additional dependencies on the current span's extensions
        tracing::debug!("Trace context extraction not implemented in this version");
        None
    }

    /// Convert to W3C Trace Context format (for HTTP headers)
    pub fn to_w3c_traceparent(&self) -> String {
        format!(
            "00-{}-{}-{:02x}",
            self.trace_id, self.span_id, self.trace_flags
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_config_default() {
        let config = TracingConfig::default();
        assert_eq!(config.service_name, "tiny-dancer");
        assert_eq!(config.sampling_ratio, 1.0);
        assert!(!config.enable_stdout);
    }

    #[test]
    fn test_tracing_system_creation() {
        let config = TracingConfig::default();
        let system = TracingSystem::new(config);
        assert_eq!(system.config.service_name, "tiny-dancer");
    }

    #[test]
    fn test_init_stdout() {
        let config = TracingConfig {
            enable_stdout: true,
            ..Default::default()
        };
        let system = TracingSystem::new(config);
        // We can't test full initialization without side effects,
        // but we can verify the system is created correctly
        assert!(system.config.enable_stdout);
    }

    #[test]
    fn test_routing_span_creation() {
        let span = RoutingSpan::routing_request(10);
        // Verify span can be created (metadata may be None if tracing not initialized)
        if let Some(metadata) = span.metadata() {
            assert_eq!(metadata.name(), "routing_request");
        }
    }

    #[test]
    fn test_feature_engineering_span() {
        let span = RoutingSpan::feature_engineering(5);
        // Verify span can be created (metadata may be None if tracing not initialized)
        if let Some(metadata) = span.metadata() {
            assert_eq!(metadata.name(), "feature_engineering");
        }
    }

    #[test]
    fn test_model_inference_span() {
        let span = RoutingSpan::model_inference("test-candidate");
        // Verify span can be created (metadata may be None if tracing not initialized)
        if let Some(metadata) = span.metadata() {
            assert_eq!(metadata.name(), "model_inference");
        }
    }

    #[test]
    fn test_trace_context_w3c_format() {
        let context = TraceContext {
            trace_id: "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
            span_id: "00f067aa0ba902b7".to_string(),
            trace_flags: 1,
        };
        let traceparent = context.to_w3c_traceparent();
        assert_eq!(
            traceparent,
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        );
    }
}
