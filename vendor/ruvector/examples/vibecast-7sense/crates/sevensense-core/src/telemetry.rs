//! # Telemetry Module
//!
//! Observability infrastructure for the 7sense platform.
//!
//! This module provides:
//! - Structured logging with tracing
//! - Distributed tracing with OpenTelemetry
//! - Metrics collection
//! - Health check utilities

use std::time::Duration;

use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::Level;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

use crate::config::{LogFormat, LoggingConfig, OpenTelemetryConfig};

/// Telemetry guard that cleans up on drop.
pub struct TelemetryGuard {
    _tracer_provider: Option<SdkTracerProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self._tracer_provider.take() {
            if let Err(e) = provider.shutdown() {
                eprintln!("Error shutting down tracer provider: {e:?}");
            }
        }
    }
}

/// Initializes telemetry with the given configuration.
///
/// # Errors
///
/// Returns an error if telemetry initialization fails.
pub fn init(config: &LoggingConfig) -> Result<TelemetryGuard, TelemetryError> {
    // Build the env filter
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.level));

    // Create the formatting layer based on config
    let fmt_layer = match config.format {
        LogFormat::Json => {
            fmt::layer()
                .json()
                .with_target(true)
                .with_thread_ids(true)
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_span_events(if config.include_spans {
                    FmtSpan::NEW | FmtSpan::CLOSE
                } else {
                    FmtSpan::NONE
                })
                .boxed()
        }
        LogFormat::Pretty => {
            fmt::layer()
                .pretty()
                .with_target(true)
                .with_thread_names(true)
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_span_events(if config.include_spans {
                    FmtSpan::NEW | FmtSpan::CLOSE
                } else {
                    FmtSpan::NONE
                })
                .boxed()
        }
        LogFormat::Compact => {
            fmt::layer()
                .compact()
                .with_target(true)
                .with_span_events(FmtSpan::NONE)
                .boxed()
        }
    };

    // Initialize OpenTelemetry if configured
    let (otel_layer, tracer_provider) = if let Some(otel_config) = &config.opentelemetry {
        let (layer, provider) = init_opentelemetry(otel_config)?;
        (Some(layer), Some(provider))
    } else {
        (None, None)
    };

    // Build and set the subscriber
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);

    if let Some(otel_layer) = otel_layer {
        subscriber.with(otel_layer).init();
    } else {
        subscriber.init();
    }

    tracing::info!(
        target: "sevensense::telemetry",
        level = %config.level,
        format = ?config.format,
        "Telemetry initialized"
    );

    Ok(TelemetryGuard {
        _tracer_provider: tracer_provider,
    })
}

/// Initializes OpenTelemetry tracing.
fn init_opentelemetry(
    config: &OpenTelemetryConfig,
) -> Result<(impl Layer<tracing_subscriber::Registry> + Send + Sync, SdkTracerProvider), TelemetryError> {
    use opentelemetry::KeyValue;
    use opentelemetry_sdk::{
        trace::{Config as TraceConfig, Sampler},
        Resource,
    };

    // Create the OTLP exporter
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&config.endpoint)
        .with_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| TelemetryError::OpenTelemetry(e.to_string()))?;

    // Create the tracer provider
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_config(
            TraceConfig::default()
                .with_sampler(Sampler::TraceIdRatioBased(config.sampling_ratio))
                .with_resource(Resource::new(vec![
                    KeyValue::new("service.name", config.service_name.clone()),
                    KeyValue::new("service.version", crate::VERSION),
                ])),
        )
        .build();

    // Create the tracing layer
    let tracer = provider.tracer(config.service_name.clone());
    let layer = tracing_opentelemetry::layer().with_tracer(tracer);

    Ok((layer, provider))
}

/// Telemetry errors.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    /// Failed to initialize OpenTelemetry.
    #[error("OpenTelemetry initialization failed: {0}")]
    OpenTelemetry(String),

    /// Failed to initialize logging.
    #[error("Logging initialization failed: {0}")]
    Logging(String),
}

/// Creates a new span for an operation.
#[macro_export]
macro_rules! span {
    ($level:expr, $name:expr) => {
        tracing::span!($level, $name)
    };
    ($level:expr, $name:expr, $($field:tt)*) => {
        tracing::span!($level, $name, $($field)*)
    };
}

/// Logs an event with timing information.
#[macro_export]
macro_rules! timed {
    ($name:expr, $block:expr) => {{
        let start = std::time::Instant::now();
        let result = $block;
        let elapsed = start.elapsed();
        tracing::debug!(
            target: "sevensense::timing",
            operation = $name,
            duration_ms = elapsed.as_millis() as u64,
            "Operation completed"
        );
        result
    }};
}

/// Health check status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Service is healthy.
    Healthy,
    /// Service is degraded but functional.
    Degraded,
    /// Service is unhealthy.
    Unhealthy,
}

impl HealthStatus {
    /// Returns whether the service is operational (healthy or degraded).
    #[must_use]
    pub const fn is_operational(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }
}

/// Component health check result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ComponentHealth {
    /// Component name.
    pub name: String,
    /// Health status.
    pub status: HealthStatus,
    /// Optional status message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Response time in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_time_ms: Option<u64>,
}

impl ComponentHealth {
    /// Creates a healthy component status.
    #[must_use]
    pub fn healthy(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Healthy,
            message: None,
            response_time_ms: None,
        }
    }

    /// Creates a degraded component status.
    #[must_use]
    pub fn degraded(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Degraded,
            message: Some(message.into()),
            response_time_ms: None,
        }
    }

    /// Creates an unhealthy component status.
    #[must_use]
    pub fn unhealthy(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Unhealthy,
            message: Some(message.into()),
            response_time_ms: None,
        }
    }

    /// Sets the response time.
    #[must_use]
    pub fn with_response_time(mut self, ms: u64) -> Self {
        self.response_time_ms = Some(ms);
        self
    }
}

/// Overall system health check result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemHealth {
    /// Overall health status.
    pub status: HealthStatus,
    /// Application version.
    pub version: String,
    /// Individual component health.
    pub components: Vec<ComponentHealth>,
}

impl SystemHealth {
    /// Creates a new system health check result.
    #[must_use]
    pub fn new(components: Vec<ComponentHealth>) -> Self {
        let status = components
            .iter()
            .map(|c| c.status)
            .fold(HealthStatus::Healthy, |acc, s| match (acc, s) {
                (HealthStatus::Unhealthy, _) | (_, HealthStatus::Unhealthy) => {
                    HealthStatus::Unhealthy
                }
                (HealthStatus::Degraded, _) | (_, HealthStatus::Degraded) => HealthStatus::Degraded,
                _ => HealthStatus::Healthy,
            });

        Self {
            status,
            version: crate::VERSION.to_string(),
            components,
        }
    }
}

/// Trait for components that support health checks.
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    /// Returns the component name.
    fn name(&self) -> &str;

    /// Performs a health check.
    async fn check(&self) -> ComponentHealth;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status() {
        assert!(HealthStatus::Healthy.is_operational());
        assert!(HealthStatus::Degraded.is_operational());
        assert!(!HealthStatus::Unhealthy.is_operational());
    }

    #[test]
    fn test_component_health() {
        let healthy = ComponentHealth::healthy("test");
        assert_eq!(healthy.status, HealthStatus::Healthy);
        assert!(healthy.message.is_none());

        let unhealthy = ComponentHealth::unhealthy("test", "connection failed");
        assert_eq!(unhealthy.status, HealthStatus::Unhealthy);
        assert!(unhealthy.message.is_some());
    }

    #[test]
    fn test_system_health_aggregation() {
        let components = vec![
            ComponentHealth::healthy("db"),
            ComponentHealth::degraded("cache", "high latency"),
        ];
        let health = SystemHealth::new(components);
        assert_eq!(health.status, HealthStatus::Degraded);

        let components = vec![
            ComponentHealth::healthy("db"),
            ComponentHealth::unhealthy("cache", "connection refused"),
        ];
        let health = SystemHealth::new(components);
        assert_eq!(health.status, HealthStatus::Unhealthy);
    }
}
