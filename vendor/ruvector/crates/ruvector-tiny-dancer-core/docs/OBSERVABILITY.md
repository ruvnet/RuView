# Tiny Dancer Observability Guide

This guide covers the comprehensive observability features in Tiny Dancer, including Prometheus metrics, OpenTelemetry distributed tracing, and structured logging.

## Table of Contents

1. [Overview](#overview)
2. [Prometheus Metrics](#prometheus-metrics)
3. [Distributed Tracing](#distributed-tracing)
4. [Structured Logging](#structured-logging)
5. [Integration Guide](#integration-guide)
6. [Examples](#examples)
7. [Best Practices](#best-practices)

## Overview

Tiny Dancer provides three layers of observability:

- **Prometheus Metrics**: Real-time performance metrics and system health
- **OpenTelemetry Tracing**: Distributed tracing for request flow analysis
- **Structured Logging**: Context-rich logs with the `tracing` crate

All three work together to provide complete visibility into your routing system.

## Prometheus Metrics

### Available Metrics

#### Request Metrics

```
tiny_dancer_routing_requests_total{status="success|failure"}
```
Counter tracking total routing requests by status.

```
tiny_dancer_routing_latency_seconds{operation="total"}
```
Histogram of routing operation latency in seconds.

#### Feature Engineering Metrics

```
tiny_dancer_feature_engineering_duration_seconds{batch_size="1-10|11-50|51-100|100+"}
```
Histogram of feature engineering duration by batch size.

#### Model Inference Metrics

```
tiny_dancer_model_inference_duration_seconds{model_type="fastgrnn"}
```
Histogram of model inference duration.

#### Circuit Breaker Metrics

```
tiny_dancer_circuit_breaker_state
```
Gauge showing circuit breaker state:
- 0 = Closed (healthy)
- 1 = Half-Open (testing)
- 2 = Open (failing)

#### Routing Decision Metrics

```
tiny_dancer_routing_decisions_total{model_type="lightweight|powerful"}
```
Counter of routing decisions by target model type.

```
tiny_dancer_confidence_scores{decision_type="lightweight|powerful"}
```
Histogram of confidence scores by decision type.

```
tiny_dancer_uncertainty_estimates{decision_type="lightweight|powerful"}
```
Histogram of uncertainty estimates.

#### Candidate Metrics

```
tiny_dancer_candidates_processed_total{batch_size_range="1-10|11-50|51-100|100+"}
```
Counter of total candidates processed by batch size range.

#### Error Metrics

```
tiny_dancer_errors_total{error_type="inference_error|circuit_breaker_open|..."}
```
Counter of errors by type.

### Using Metrics

```rust
use ruvector_tiny_dancer_core::{Router, RouterConfig};

// Create router (metrics are automatically collected)
let router = Router::new(RouterConfig::default())?;

// Process requests...
let response = router.route(request)?;

// Export metrics in Prometheus format
let metrics = router.export_metrics()?;
println!("{}", metrics);
```

### Prometheus Configuration

```yaml
scrape_configs:
  - job_name: 'tiny-dancer'
    scrape_interval: 15s
    static_configs:
      - targets: ['localhost:9090']
```

### Example Grafana Dashboard

```json
{
  "dashboard": {
    "title": "Tiny Dancer Routing",
    "panels": [
      {
        "title": "Request Rate",
        "targets": [{
          "expr": "rate(tiny_dancer_routing_requests_total[5m])"
        }]
      },
      {
        "title": "P95 Latency",
        "targets": [{
          "expr": "histogram_quantile(0.95, rate(tiny_dancer_routing_latency_seconds_bucket[5m]))"
        }]
      },
      {
        "title": "Circuit Breaker State",
        "targets": [{
          "expr": "tiny_dancer_circuit_breaker_state"
        }]
      },
      {
        "title": "Lightweight vs Powerful Routing",
        "targets": [{
          "expr": "rate(tiny_dancer_routing_decisions_total[5m])"
        }]
      }
    ]
  }
}
```

## Distributed Tracing

### OpenTelemetry Integration

Tiny Dancer integrates with OpenTelemetry for distributed tracing, supporting exporters like Jaeger, Zipkin, and more.

### Trace Spans

The following spans are automatically created:

- `routing_request`: Complete routing operation
- `circuit_breaker_check`: Circuit breaker validation
- `feature_engineering`: Feature extraction and engineering
- `model_inference`: Neural model inference (per candidate)
- `uncertainty_estimation`: Uncertainty quantification

### Configuration

```rust
use ruvector_tiny_dancer_core::{TracingConfig, TracingSystem};

// Configure tracing
let config = TracingConfig {
    service_name: "tiny-dancer".to_string(),
    service_version: "1.0.0".to_string(),
    jaeger_agent_endpoint: Some("localhost:6831".to_string()),
    sampling_ratio: 1.0, // Sample 100% of traces
    enable_stdout: false,
};

// Initialize tracing
let tracing_system = TracingSystem::new(config);
tracing_system.init()?;

// Your application code...

// Shutdown and flush traces
tracing_system.shutdown();
```

### Jaeger Setup

```bash
# Run Jaeger all-in-one
docker run -d \
  -p 6831:6831/udp \
  -p 16686:16686 \
  jaegertracing/all-in-one:latest

# Access Jaeger UI at http://localhost:16686
```

### Trace Context Propagation

```rust
use ruvector_tiny_dancer_core::TraceContext;

// Get trace context from current span
if let Some(ctx) = TraceContext::from_current() {
    println!("Trace ID: {}", ctx.trace_id);
    println!("Span ID: {}", ctx.span_id);

    // W3C Trace Context format for HTTP headers
    let traceparent = ctx.to_w3c_traceparent();
    // Example: "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
}
```

### Custom Spans

```rust
use ruvector_tiny_dancer_core::RoutingSpan;
use tracing::info_span;

// Create custom span
let span = info_span!("my_operation", param1 = "value");
let _guard = span.enter();

// Or use pre-defined span helpers
let span = RoutingSpan::routing_request(candidate_count);
let _guard = span.enter();
```

## Structured Logging

### Log Levels

Tiny Dancer uses the `tracing` crate for structured logging:

- **ERROR**: Critical failures (circuit breaker open, inference errors)
- **WARN**: Warnings (model path not found, degraded performance)
- **INFO**: Normal operations (router initialization, request completion)
- **DEBUG**: Detailed information (feature extraction, inference results)
- **TRACE**: Very detailed information (internal state changes)

### Example Logs

```
INFO tiny_dancer_router: Initializing Tiny Dancer router
INFO tiny_dancer_router: Circuit breaker enabled with threshold: 5
INFO tiny_dancer_router: Processing routing request candidate_count=3
DEBUG tiny_dancer_router: Extracting features batch_size=3
DEBUG tiny_dancer_router: Model inference completed candidate_id="candidate-1" confidence=0.92
DEBUG tiny_dancer_router: Routing decision made candidate_id="candidate-1" use_lightweight=true uncertainty=0.08
INFO tiny_dancer_router: Routing request completed successfully inference_time_us=245 lightweight_routes=2 powerful_routes=1
```

### Configuring Logging

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Basic setup
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::INFO)
    .init();

// Advanced setup with JSON formatting
tracing_subscriber::registry()
    .with(tracing_subscriber::fmt::layer().json())
    .with(tracing_subscriber::filter::LevelFilter::from_level(
        tracing::Level::DEBUG
    ))
    .init();
```

## Integration Guide

### Complete Setup

```rust
use ruvector_tiny_dancer_core::{
    Router, RouterConfig, TracingConfig, TracingSystem
};
use tracing_subscriber;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize structured logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // 2. Initialize distributed tracing
    let tracing_config = TracingConfig {
        service_name: "my-service".to_string(),
        service_version: "1.0.0".to_string(),
        jaeger_agent_endpoint: Some("localhost:6831".to_string()),
        sampling_ratio: 0.1, // Sample 10% in production
        enable_stdout: false,
    };
    let tracing_system = TracingSystem::new(tracing_config);
    tracing_system.init()?;

    // 3. Create router (metrics automatically enabled)
    let router = Router::new(RouterConfig::default())?;

    // 4. Process requests (all observability automatic)
    let response = router.route(request)?;

    // 5. Periodically export metrics (e.g., to HTTP endpoint)
    let metrics = router.export_metrics()?;

    // 6. Cleanup
    tracing_system.shutdown();

    Ok(())
}
```

### HTTP Metrics Endpoint

```rust
use axum::{Router, routing::get};

async fn metrics_handler(
    router: Arc<ruvector_tiny_dancer_core::Router>
) -> String {
    router.export_metrics().unwrap_or_default()
}

let app = Router::new()
    .route("/metrics", get(metrics_handler));
```

## Examples

### 1. Metrics Only

```bash
cargo run --example metrics_example
```

Demonstrates Prometheus metrics collection and export.

### 2. Tracing Only

```bash
# Start Jaeger first
docker run -d -p6831:6831/udp -p16686:16686 jaegertracing/all-in-one:latest

# Run example
cargo run --example tracing_example
```

Shows distributed tracing with OpenTelemetry.

### 3. Full Observability

```bash
cargo run --example full_observability
```

Combines metrics, tracing, and structured logging.

## Best Practices

### Production Configuration

1. **Sampling**: Don't trace every request in production
   ```rust
   sampling_ratio: 0.01, // 1% sampling
   ```

2. **Log Levels**: Use INFO or WARN in production
   ```rust
   .with_max_level(tracing::Level::INFO)
   ```

3. **Metrics Cardinality**: Be careful with high-cardinality labels
   - ✓ Good: `{model_type="lightweight"}`
   - ✗ Bad: `{candidate_id="12345"}` (too many unique values)

4. **Performance**: Metrics collection is very lightweight (<1μs overhead)

### Alerting Rules

Example Prometheus alerting rules:

```yaml
groups:
  - name: tiny_dancer
    rules:
      - alert: HighErrorRate
        expr: rate(tiny_dancer_errors_total[5m]) > 0.05
        for: 5m
        annotations:
          summary: "High error rate detected"

      - alert: CircuitBreakerOpen
        expr: tiny_dancer_circuit_breaker_state == 2
        for: 1m
        annotations:
          summary: "Circuit breaker is open"

      - alert: HighLatency
        expr: histogram_quantile(0.95, rate(tiny_dancer_routing_latency_seconds_bucket[5m])) > 0.01
        for: 5m
        annotations:
          summary: "P95 latency above 10ms"
```

### Debugging Performance Issues

1. **Check metrics** for high-level patterns
   ```promql
   rate(tiny_dancer_routing_requests_total[5m])
   ```

2. **Use traces** to identify bottlenecks
   - Look for long spans
   - Identify slow candidates

3. **Review logs** for error details
   ```bash
   grep "ERROR" logs.txt | jq .
   ```

## Troubleshooting

### Metrics Not Appearing

- Ensure router is processing requests
- Check metrics export: `router.export_metrics()?`
- Verify Prometheus scrape configuration

### Traces Not in Jaeger

- Confirm Jaeger is running: `docker ps`
- Check endpoint: `jaeger_agent_endpoint: Some("localhost:6831")`
- Verify sampling ratio > 0
- Call `tracing_system.shutdown()` to flush

### High Memory Usage

- Reduce sampling ratio
- Decrease histogram buckets
- Lower log level to INFO or WARN

## Reference

- [Prometheus Documentation](https://prometheus.io/docs/)
- [OpenTelemetry Specification](https://opentelemetry.io/docs/)
- [Tracing Crate](https://docs.rs/tracing/)
- [Jaeger Documentation](https://www.jaegertracing.io/docs/)
