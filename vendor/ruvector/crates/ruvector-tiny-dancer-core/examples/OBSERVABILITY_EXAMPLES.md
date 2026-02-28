# Tiny Dancer Observability Examples

This directory contains examples demonstrating the observability features of Tiny Dancer.

## Examples

### 1. Metrics Example (`metrics_example.rs`)

**Purpose**: Demonstrates Prometheus metrics collection

**Features**:
- Request counting
- Latency tracking
- Circuit breaker monitoring
- Routing decision metrics
- Prometheus format export

**Run**:
```bash
cargo run --example metrics_example
```

**Output**: Shows metrics in Prometheus text format

### 2. Tracing Example (`tracing_example.rs`)

**Purpose**: Shows distributed tracing with OpenTelemetry

**Features**:
- Jaeger integration
- Span creation
- Trace context propagation
- W3C Trace Context format

**Prerequisites**:
```bash
# Start Jaeger
docker run -d -p6831:6831/udp -p16686:16686 jaegertracing/all-in-one:latest
```

**Run**:
```bash
cargo run --example tracing_example
```

**View Traces**: http://localhost:16686

### 3. Full Observability Example (`full_observability.rs`)

**Purpose**: Comprehensive example combining all observability features

**Features**:
- Prometheus metrics
- Distributed tracing
- Structured logging
- Multiple scenarios (normal load, high load)
- Performance statistics

**Run**:
```bash
cargo run --example full_observability
```

**Output**: Complete observability stack demonstration

## Quick Start

1. **Basic Metrics** (no dependencies):
   ```bash
   cargo run --example metrics_example
   ```

2. **With Tracing** (requires Jaeger):
   ```bash
   # Terminal 1: Start Jaeger
   docker run -p6831:6831/udp -p16686:16686 jaegertracing/all-in-one:latest

   # Terminal 2: Run example
   cargo run --example tracing_example

   # Browser: Open http://localhost:16686
   ```

3. **Full Stack**:
   ```bash
   cargo run --example full_observability
   ```

## Metrics Available

- `tiny_dancer_routing_requests_total` - Request counter
- `tiny_dancer_routing_latency_seconds` - Latency histogram
- `tiny_dancer_circuit_breaker_state` - Circuit breaker gauge
- `tiny_dancer_routing_decisions_total` - Decision counter
- `tiny_dancer_confidence_scores` - Confidence histogram
- `tiny_dancer_uncertainty_estimates` - Uncertainty histogram
- `tiny_dancer_candidates_processed_total` - Candidates counter
- `tiny_dancer_errors_total` - Error counter
- `tiny_dancer_feature_engineering_duration_seconds` - Feature time
- `tiny_dancer_model_inference_duration_seconds` - Inference time

## Tracing Spans

Automatically created spans:
- `routing_request` - Full routing operation
- `circuit_breaker_check` - Circuit breaker validation
- `feature_engineering` - Feature extraction
- `model_inference` - Model inference (per candidate)
- `uncertainty_estimation` - Uncertainty calculation

## Production Setup

### Prometheus

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'tiny-dancer'
    scrape_interval: 15s
    static_configs:
      - targets: ['localhost:9090']
```

### Jaeger

```bash
# Production deployment
docker run -d \
  --name jaeger \
  -e COLLECTOR_ZIPKIN_HOST_PORT=:9411 \
  -p 5775:5775/udp \
  -p 6831:6831/udp \
  -p 6832:6832/udp \
  -p 5778:5778 \
  -p 16686:16686 \
  -p 14268:14268 \
  -p 14250:14250 \
  -p 9411:9411 \
  jaegertracing/all-in-one:latest
```

### Grafana Dashboard

1. Add Prometheus data source
2. Import dashboard from `docs/OBSERVABILITY.md`
3. Create alerts:
   - Circuit breaker open
   - High error rate
   - High latency

## Troubleshooting

### Metrics not showing

```rust
// Ensure router is processing requests
let response = router.route(request)?;

// Export and check metrics
let metrics = router.export_metrics()?;
println!("{}", metrics);
```

### Traces not in Jaeger

1. Check Jaeger is running: `docker ps`
2. Verify endpoint in config
3. Ensure sampling_ratio > 0
4. Call `tracing_system.shutdown()` to flush

### High memory usage

- Reduce sampling ratio to 0.01 (1%)
- Set log level to INFO
- Use appropriate histogram buckets

## Additional Resources

- Full documentation: `../docs/OBSERVABILITY.md`
- Implementation summary: `../docs/OBSERVABILITY_SUMMARY.md`
- Prometheus docs: https://prometheus.io/docs/
- OpenTelemetry docs: https://opentelemetry.io/docs/
- Jaeger docs: https://www.jaegertracing.io/docs/
