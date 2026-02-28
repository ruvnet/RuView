# Tiny Dancer Admin API Documentation

## Overview

The Tiny Dancer Admin API provides a production-ready REST API for monitoring, health checks, and administration of the AI routing system. It's designed to integrate seamlessly with Kubernetes, Prometheus, and other cloud-native tools.

## Features

- **Health Checks**: Kubernetes-compatible liveness and readiness probes
- **Metrics Export**: Prometheus-compatible metrics endpoint
- **Hot Reloading**: Update models without downtime
- **Circuit Breaker Management**: Monitor and control circuit breaker state
- **Configuration Management**: View and update router configuration
- **Optional Authentication**: Bearer token authentication for admin endpoints
- **CORS Support**: Configurable CORS for web applications

## Quick Start

### Running the Server

```bash
# With admin API feature enabled
cargo run --example admin-server --features admin-api
```

### Basic Configuration

```rust
use ruvector_tiny_dancer_core::api::{AdminServer, AdminServerConfig};
use ruvector_tiny_dancer_core::router::Router;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let router = Router::default()?;

    let config = AdminServerConfig {
        bind_address: "0.0.0.0".to_string(),
        port: 8080,
        auth_token: Some("your-secret-token".to_string()),
        enable_cors: true,
    };

    let server = AdminServer::new(Arc::new(router), config);
    server.serve().await?;
    Ok(())
}
```

## API Endpoints

### Health Checks

#### `GET /health`

Basic liveness probe that always returns 200 OK if the service is running.

**Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

**Use Case:** Kubernetes liveness probe

```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 8080
  initialDelaySeconds: 3
  periodSeconds: 10
```

---

#### `GET /health/ready`

Readiness probe that checks if the service can accept traffic.

**Checks:**
- Circuit breaker state
- Model loaded status

**Response (Ready):**
```json
{
  "ready": true,
  "circuit_breaker": "closed",
  "model_loaded": true,
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

**Response (Not Ready):**
```json
{
  "ready": false,
  "circuit_breaker": "open",
  "model_loaded": true,
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

**Status Codes:**
- `200 OK`: Service is ready
- `503 Service Unavailable`: Service is not ready

**Use Case:** Kubernetes readiness probe

```yaml
readinessProbe:
  httpGet:
    path: /health/ready
    port: 8080
  initialDelaySeconds: 5
  periodSeconds: 5
```

---

### Metrics

#### `GET /metrics`

Exports metrics in Prometheus exposition format.

**Response Format:** `text/plain; version=0.0.4`

**Metrics Exported:**

```
# HELP tiny_dancer_requests_total Total number of routing requests
# TYPE tiny_dancer_requests_total counter
tiny_dancer_requests_total 12345

# HELP tiny_dancer_lightweight_routes_total Requests routed to lightweight model
# TYPE tiny_dancer_lightweight_routes_total counter
tiny_dancer_lightweight_routes_total 10000

# HELP tiny_dancer_powerful_routes_total Requests routed to powerful model
# TYPE tiny_dancer_powerful_routes_total counter
tiny_dancer_powerful_routes_total 2345

# HELP tiny_dancer_inference_time_microseconds Average inference time
# TYPE tiny_dancer_inference_time_microseconds gauge
tiny_dancer_inference_time_microseconds 450.5

# HELP tiny_dancer_latency_microseconds Latency percentiles
# TYPE tiny_dancer_latency_microseconds gauge
tiny_dancer_latency_microseconds{quantile="0.5"} 400
tiny_dancer_latency_microseconds{quantile="0.95"} 800
tiny_dancer_latency_microseconds{quantile="0.99"} 1200

# HELP tiny_dancer_errors_total Total number of errors
# TYPE tiny_dancer_errors_total counter
tiny_dancer_errors_total 5

# HELP tiny_dancer_circuit_breaker_trips_total Circuit breaker trip count
# TYPE tiny_dancer_circuit_breaker_trips_total counter
tiny_dancer_circuit_breaker_trips_total 2

# HELP tiny_dancer_uptime_seconds Service uptime
# TYPE tiny_dancer_uptime_seconds counter
tiny_dancer_uptime_seconds 3600
```

**Use Case:** Prometheus scraping

```yaml
scrape_configs:
  - job_name: 'tiny-dancer'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
```

---

### Admin Endpoints

All admin endpoints support optional bearer token authentication.

#### `POST /admin/reload`

Hot reload the routing model from disk without restarting the service.

**Headers:**
```
Authorization: Bearer your-secret-token
```

**Response:**
```json
{
  "success": true,
  "message": "Model reloaded successfully"
}
```

**Status Codes:**
- `200 OK`: Model reloaded successfully
- `401 Unauthorized`: Invalid or missing authentication token
- `500 Internal Server Error`: Failed to reload model

**Example:**
```bash
curl -X POST http://localhost:8080/admin/reload \
  -H "Authorization: Bearer your-token-here"
```

---

#### `GET /admin/config`

Get the current router configuration.

**Headers:**
```
Authorization: Bearer your-secret-token
```

**Response:**
```json
{
  "model_path": "./models/fastgrnn.safetensors",
  "confidence_threshold": 0.85,
  "max_uncertainty": 0.15,
  "enable_circuit_breaker": true,
  "circuit_breaker_threshold": 5,
  "enable_quantization": true,
  "database_path": null
}
```

**Status Codes:**
- `200 OK`: Configuration retrieved
- `401 Unauthorized`: Invalid or missing authentication token

**Example:**
```bash
curl http://localhost:8080/admin/config \
  -H "Authorization: Bearer your-token-here"
```

---

#### `PUT /admin/config`

Update the router configuration (runtime only, not persisted).

**Headers:**
```
Authorization: Bearer your-secret-token
Content-Type: application/json
```

**Request Body:**
```json
{
  "confidence_threshold": 0.90,
  "max_uncertainty": 0.10,
  "circuit_breaker_threshold": 10
}
```

**Response:**
```json
{
  "success": true,
  "message": "Configuration updated",
  "updated_fields": ["confidence_threshold", "max_uncertainty"]
}
```

**Status Codes:**
- `200 OK`: Configuration updated
- `401 Unauthorized`: Invalid or missing authentication token
- `501 Not Implemented`: Feature not yet implemented

**Note:** Currently returns 501 as runtime config updates require Router API extensions.

---

#### `GET /admin/circuit-breaker`

Get the current circuit breaker status.

**Headers:**
```
Authorization: Bearer your-secret-token
```

**Response:**
```json
{
  "enabled": true,
  "state": "closed",
  "failure_count": 2,
  "success_count": 1234
}
```

**Status Codes:**
- `200 OK`: Status retrieved
- `401 Unauthorized`: Invalid or missing authentication token

**Example:**
```bash
curl http://localhost:8080/admin/circuit-breaker \
  -H "Authorization: Bearer your-token-here"
```

---

#### `POST /admin/circuit-breaker/reset`

Reset the circuit breaker to closed state.

**Headers:**
```
Authorization: Bearer your-secret-token
```

**Response:**
```json
{
  "success": true,
  "message": "Circuit breaker reset successfully"
}
```

**Status Codes:**
- `200 OK`: Circuit breaker reset
- `401 Unauthorized`: Invalid or missing authentication token
- `501 Not Implemented`: Feature not yet implemented

**Note:** Currently returns 501 as circuit breaker reset requires Router API extensions.

---

### System Information

#### `GET /info`

Get comprehensive system information.

**Response:**
```json
{
  "version": "0.1.0",
  "api_version": "v1",
  "uptime_seconds": 3600,
  "config": {
    "model_path": "./models/fastgrnn.safetensors",
    "confidence_threshold": 0.85,
    "max_uncertainty": 0.15,
    "enable_circuit_breaker": true,
    "circuit_breaker_threshold": 5,
    "enable_quantization": true,
    "database_path": null
  },
  "circuit_breaker_enabled": true,
  "metrics": {
    "total_requests": 12345,
    "lightweight_routes": 10000,
    "powerful_routes": 2345,
    "avg_inference_time_us": 450.5,
    "p50_latency_us": 400,
    "p95_latency_us": 800,
    "p99_latency_us": 1200,
    "error_count": 5,
    "circuit_breaker_trips": 2
  }
}
```

**Example:**
```bash
curl http://localhost:8080/info
```

---

## Authentication

The admin API supports optional bearer token authentication for admin endpoints.

### Configuration

```rust
let config = AdminServerConfig {
    bind_address: "0.0.0.0".to_string(),
    port: 8080,
    auth_token: Some("your-secret-token-here".to_string()),
    enable_cors: true,
};
```

### Usage

Include the bearer token in the Authorization header:

```bash
curl -H "Authorization: Bearer your-secret-token-here" \
  http://localhost:8080/admin/reload
```

### Security Best Practices

1. **Always enable authentication in production**
2. **Use strong, random tokens** (minimum 32 characters)
3. **Rotate tokens regularly**
4. **Use HTTPS in production** (configure via reverse proxy)
5. **Limit admin API access** to internal networks only
6. **Monitor failed authentication attempts**

### Environment Variables

```bash
export TINY_DANCER_AUTH_TOKEN="your-secret-token-here"
export TINY_DANCER_BIND_ADDRESS="0.0.0.0"
export TINY_DANCER_PORT="8080"
```

---

## Kubernetes Integration

### Deployment Example

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: tiny-dancer
spec:
  replicas: 3
  selector:
    matchLabels:
      app: tiny-dancer
  template:
    metadata:
      labels:
        app: tiny-dancer
    spec:
      containers:
      - name: tiny-dancer
        image: tiny-dancer:latest
        ports:
        - containerPort: 8080
          name: admin-api
        env:
        - name: TINY_DANCER_AUTH_TOKEN
          valueFrom:
            secretKeyRef:
              name: tiny-dancer-secrets
              key: auth-token
        livenessProbe:
          httpGet:
            path: /health
            port: admin-api
          initialDelaySeconds: 3
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health/ready
            port: admin-api
          initialDelaySeconds: 5
          periodSeconds: 5
        resources:
          requests:
            memory: "256Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
```

### Service Example

```yaml
apiVersion: v1
kind: Service
metadata:
  name: tiny-dancer
  annotations:
    prometheus.io/scrape: "true"
    prometheus.io/port: "8080"
    prometheus.io/path: "/metrics"
spec:
  selector:
    app: tiny-dancer
  ports:
  - name: admin-api
    port: 8080
    targetPort: 8080
  type: ClusterIP
```

---

## Monitoring with Grafana

### Prometheus Query Examples

```promql
# Request rate
rate(tiny_dancer_requests_total[5m])

# Error rate
rate(tiny_dancer_errors_total[5m]) / rate(tiny_dancer_requests_total[5m])

# P95 latency
tiny_dancer_latency_microseconds{quantile="0.95"}

# Lightweight routing ratio
tiny_dancer_lightweight_routes_total / tiny_dancer_requests_total

# Circuit breaker trips over time
increase(tiny_dancer_circuit_breaker_trips_total[1h])
```

### Dashboard Panels

1. **Request Rate**: Line graph of requests per second
2. **Error Rate**: Gauge showing error percentage
3. **Latency Percentiles**: Multi-line graph (P50, P95, P99)
4. **Routing Distribution**: Pie chart (lightweight vs powerful)
5. **Circuit Breaker Status**: Single stat panel
6. **Uptime**: Single stat panel

---

## Performance Considerations

### Metrics Collection

The metrics endpoint is designed for high-performance scraping:

- **No locks during read**: Uses atomic operations where possible
- **O(1) complexity**: All metrics are pre-aggregated
- **Minimal allocations**: Prometheus format generated on-the-fly
- **Scrape interval**: Recommended 15-30 seconds

### Health Check Latency

- Health check: ~10μs
- Readiness check: ~50μs (includes circuit breaker check)

### Memory Overhead

- Admin server: ~2MB base memory
- Per-connection overhead: ~50KB
- Metrics storage: ~1KB

---

## Error Handling

### Common Error Responses

#### 401 Unauthorized
```json
{
  "error": "Missing or invalid Authorization header"
}
```

#### 500 Internal Server Error
```json
{
  "success": false,
  "message": "Failed to reload model: File not found"
}
```

#### 503 Service Unavailable
```json
{
  "ready": false,
  "circuit_breaker": "open",
  "model_loaded": true,
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

---

## Production Checklist

- [ ] Enable authentication for admin endpoints
- [ ] Configure HTTPS via reverse proxy (nginx, Envoy, etc.)
- [ ] Set up Prometheus scraping
- [ ] Configure Grafana dashboards
- [ ] Set up alerts for error rate and latency
- [ ] Implement log aggregation
- [ ] Configure network policies (K8s)
- [ ] Set resource limits
- [ ] Enable CORS only for trusted origins
- [ ] Rotate authentication tokens regularly
- [ ] Monitor circuit breaker trips
- [ ] Set up automated model reload workflows

---

## Troubleshooting

### Server Won't Start

**Symptom:** `Failed to bind to 0.0.0.0:8080: Address already in use`

**Solution:** Change the port or stop the conflicting service:
```bash
lsof -i :8080
kill <PID>
```

### Authentication Failing

**Symptom:** `401 Unauthorized`

**Solution:** Check that the token matches exactly:
```bash
# Test with curl
curl -H "Authorization: Bearer your-token" http://localhost:8080/admin/config
```

### Metrics Not Updating

**Symptom:** Metrics show zero values

**Solution:** Ensure you're recording metrics after each routing operation:
```rust
use ruvector_tiny_dancer_core::api::record_routing_metrics;

// After routing
record_routing_metrics(&metrics, inference_time_us, lightweight_count, powerful_count);
```

---

## Future Enhancements

- [ ] Runtime configuration persistence
- [ ] Circuit breaker manual reset API
- [ ] WebSocket support for real-time metrics streaming
- [ ] OpenTelemetry integration
- [ ] Custom metric labels
- [ ] Rate limiting
- [ ] Request/response logging middleware
- [ ] Distributed tracing integration
- [ ] GraphQL API alternative
- [ ] Admin UI dashboard

---

## Support

For issues, questions, or contributions, please visit:
- GitHub: https://github.com/ruvnet/ruvector
- Documentation: https://docs.ruvector.io

---

## License

This API is part of the Tiny Dancer routing system and follows the same license terms.
