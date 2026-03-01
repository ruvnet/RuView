# Tiny Dancer Examples

This directory contains example applications demonstrating how to use Tiny Dancer.

## Admin Server Example

**File:** `admin-server.rs`

A production-ready admin API server with health checks, metrics, and administration endpoints.

### Features

- Health check endpoints (K8s liveness & readiness probes)
- Prometheus metrics export
- Hot model reloading
- Configuration management
- Circuit breaker monitoring
- Optional bearer token authentication

### Running

```bash
cargo run --example admin-server --features admin-api
```

### Testing

Once running, test the endpoints:

```bash
# Health check
curl http://localhost:8080/health

# Readiness check
curl http://localhost:8080/health/ready

# Prometheus metrics
curl http://localhost:8080/metrics

# System information
curl http://localhost:8080/info
```

### Admin Endpoints

Admin endpoints support optional authentication:

```bash
# Reload model (if auth enabled)
curl -X POST http://localhost:8080/admin/reload \
  -H "Authorization: Bearer your-token-here"

# Get configuration
curl http://localhost:8080/admin/config \
  -H "Authorization: Bearer your-token-here"

# Circuit breaker status
curl http://localhost:8080/admin/circuit-breaker \
  -H "Authorization: Bearer your-token-here"
```

### Configuration

Edit the example to configure:
- Bind address and port
- Authentication token
- CORS settings
- Router configuration

### Production Deployment

For production use:

1. **Enable authentication:**
   ```rust
   auth_token: Some("your-secret-token".to_string())
   ```

2. **Use environment variables:**
   ```rust
   let token = std::env::var("ADMIN_AUTH_TOKEN").ok();
   ```

3. **Deploy behind HTTPS proxy** (nginx, Envoy, etc.)

4. **Set up Prometheus scraping:**
   ```yaml
   scrape_configs:
     - job_name: 'tiny-dancer'
       static_configs:
         - targets: ['localhost:8080']
   ```

5. **Configure Kubernetes probes:**
   ```yaml
   livenessProbe:
     httpGet:
       path: /health
       port: 8080
   readinessProbe:
     httpGet:
       path: /health/ready
       port: 8080
   ```

## Documentation

- [Admin API Full Documentation](../docs/API.md)
- [Quick Start Guide](../docs/ADMIN_API_QUICKSTART.md)

## Next Steps

1. Integrate with your application
2. Set up monitoring (Prometheus + Grafana)
3. Configure alerts
4. Deploy to production

## Support

For issues or questions, see the main repository documentation.
