# Tiny Dancer Admin API - Quick Reference Card

## Installation

```toml
[dependencies]
ruvector-tiny-dancer-core = { version = "0.1", features = ["admin-api"] }
tokio = { version = "1", features = ["full"] }
```

## Minimal Server Setup

```rust
use ruvector_tiny_dancer_core::api::{AdminServer, AdminServerConfig};
use ruvector_tiny_dancer_core::router::Router;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let router = Router::default()?;
    let config = AdminServerConfig::default();
    let server = AdminServer::new(Arc::new(router), config);
    server.serve().await?;
    Ok(())
}
```

## Configuration

```rust
let config = AdminServerConfig {
    bind_address: "0.0.0.0".to_string(),
    port: 8080,
    auth_token: Some("secret-token".to_string()), // Optional
    enable_cors: true,
};
```

## API Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/health` | GET | Liveness |
| `/health/ready` | GET | Readiness |
| `/metrics` | GET | Prometheus |
| `/info` | GET | System info |
| `/admin/reload` | POST | Reload model |
| `/admin/config` | GET | Get config |
| `/admin/circuit-breaker` | GET | CB status |

## Testing Commands

```bash
# Health check
curl http://localhost:8080/health

# Readiness
curl http://localhost:8080/health/ready

# Metrics
curl http://localhost:8080/metrics

# System info
curl http://localhost:8080/info

# Admin (with auth)
curl -H "Authorization: Bearer token" \
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
  - name: api
    image: tiny-dancer:latest
    ports:
    - containerPort: 8080
    livenessProbe:
      httpGet:
        path: /health
        port: 8080
    readinessProbe:
      httpGet:
        path: /health/ready
        port: 8080
```

## Prometheus Scraping

```yaml
scrape_configs:
  - job_name: 'tiny-dancer'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

## Recording Metrics

```rust
use ruvector_tiny_dancer_core::api::{
    record_routing_metrics,
    record_error,
    record_circuit_breaker_trip
};

// After routing
record_routing_metrics(&metrics, inference_time_us, lightweight_count, powerful_count);

// On error
record_error(&metrics);

// On CB trip
record_circuit_breaker_trip(&metrics);
```

## Environment Variables

```bash
export ADMIN_API_TOKEN="your-secret-token"
export ADMIN_API_PORT="8080"
export ADMIN_API_ADDR="0.0.0.0"
```

## Run Example

```bash
cargo run --example admin-server --features admin-api
```

## File Locations

- **Core:** `/home/user/ruvector/crates/ruvector-tiny-dancer-core/src/api.rs`
- **Example:** `/home/user/ruvector/crates/ruvector-tiny-dancer-core/examples/admin-server.rs`
- **Docs:** `/home/user/ruvector/crates/ruvector-tiny-dancer-core/docs/API.md`

## Key Features

- ✅ Kubernetes probes
- ✅ Prometheus metrics
- ✅ Hot model reload
- ✅ Circuit breaker monitoring
- ✅ Optional authentication
- ✅ CORS support
- ✅ Async/Tokio
- ✅ Production-ready

## See Also

- **Full API Docs:** `docs/API.md`
- **Quick Start:** `docs/ADMIN_API_QUICKSTART.md`
- **Implementation:** `docs/API_IMPLEMENTATION_SUMMARY.md`
