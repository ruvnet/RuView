# Tiny Dancer Admin API - Quick Start Guide

## Overview

The Tiny Dancer Admin API provides production-ready endpoints for:
- **Health Checks**: Kubernetes liveness and readiness probes
- **Metrics**: Prometheus-compatible metrics export
- **Administration**: Hot model reloading, configuration management, circuit breaker control

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ruvector-tiny-dancer-core = { version = "0.1", features = ["admin-api"] }
tokio = { version = "1", features = ["full"] }
```

## Minimal Example

```rust
use ruvector_tiny_dancer_core::api::{AdminServer, AdminServerConfig};
use ruvector_tiny_dancer_core::router::Router;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create router
    let router = Router::default()?;

    // Configure admin server
    let config = AdminServerConfig {
        bind_address: "127.0.0.1".to_string(),
        port: 8080,
        auth_token: None, // Optional: Add "your-secret" for auth
        enable_cors: true,
    };

    // Start server
    let server = AdminServer::new(Arc::new(router), config);
    server.serve().await?;

    Ok(())
}
```

## Run the Example

```bash
cargo run --example admin-server --features admin-api
```

## Test the Endpoints

### Health Check (Liveness)
```bash
curl http://localhost:8080/health
```

Response:
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 42
}
```

### Readiness Check
```bash
curl http://localhost:8080/health/ready
```

Response:
```json
{
  "ready": true,
  "circuit_breaker": "closed",
  "model_loaded": true,
  "version": "0.1.0",
  "uptime_seconds": 42
}
```

### Prometheus Metrics
```bash
curl http://localhost:8080/metrics
```

Response:
```
# HELP tiny_dancer_requests_total Total number of routing requests
# TYPE tiny_dancer_requests_total counter
tiny_dancer_requests_total 12345
...
```

### System Info
```bash
curl http://localhost:8080/info
```

## With Authentication

```rust
let config = AdminServerConfig {
    bind_address: "0.0.0.0".to_string(),
    port: 8080,
    auth_token: Some("my-secret-token-12345".to_string()),
    enable_cors: true,
};
```

Test with token:
```bash
curl -H "Authorization: Bearer my-secret-token-12345" \
  http://localhost:8080/admin/config
```

## Kubernetes Deployment

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: tiny-dancer
spec:
  containers:
  - name: tiny-dancer
    image: your-image:latest
    ports:
    - containerPort: 8080
    livenessProbe:
      httpGet:
        path: /health
        port: 8080
      initialDelaySeconds: 3
      periodSeconds: 10
    readinessProbe:
      httpGet:
        path: /health/ready
        port: 8080
      initialDelaySeconds: 5
      periodSeconds: 5
```

## Next Steps

- Read the [full API documentation](./API.md)
- Configure [Prometheus scraping](#prometheus-integration)
- Set up [Grafana dashboards](#monitoring)
- Implement [custom metrics recording](#metrics-api)

## API Endpoints Summary

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/health` | GET | Liveness probe |
| `/health/ready` | GET | Readiness probe |
| `/metrics` | GET | Prometheus metrics |
| `/info` | GET | System information |
| `/admin/reload` | POST | Reload model |
| `/admin/config` | GET | Get configuration |
| `/admin/config` | PUT | Update configuration |
| `/admin/circuit-breaker` | GET | Circuit breaker status |
| `/admin/circuit-breaker/reset` | POST | Reset circuit breaker |

## Security Notes

1. **Always use authentication in production**
2. **Run behind HTTPS (nginx, Envoy, etc.)**
3. **Limit network access to admin endpoints**
4. **Rotate tokens regularly**
5. **Monitor failed authentication attempts**

---

For detailed documentation, see [API.md](./API.md)
