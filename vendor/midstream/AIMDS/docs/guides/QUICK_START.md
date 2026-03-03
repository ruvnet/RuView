# AIMDS Quick Start Guide

## Prerequisites

- Rust 1.75+ ([Install](https://rustup.rs/))
- Node.js 20+ ([Install](https://nodejs.org/))
- Docker & Docker Compose ([Install](https://docs.docker.com/get-docker/))
- Git

## Local Development Setup

### 1. Clone and Setup

```bash
cd /workspaces/midstream/AIMDS

# Install Rust dependencies
cargo build

# Install Node dependencies
npm install

# Configure environment
cp .env.example .env
# Edit .env with your configuration
```

### 2. Run with Docker Compose

```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f

# Check health
curl http://localhost:3000/health
```

### 3. Test the System

```bash
# Run Rust tests
cargo test --workspace

# Run TypeScript tests
npm test

# Run benchmarks
cargo bench --workspace
```

## Production Deployment

### Kubernetes

```bash
# Create namespace
kubectl create namespace aimds

# Apply configurations
kubectl apply -f k8s/

# Check status
kubectl get pods -n aimds
kubectl get svc -n aimds

# View logs
kubectl logs -f deployment/aimds-gateway -n aimds
```

### Configuration

Edit `k8s/configmap.yaml` with your settings:
- Redis URL
- AgentDB endpoint
- Anthropic API key (in secrets)

## Usage Examples

### Detect Threat

```bash
curl -X POST http://localhost:3000/api/detect \
  -H "Content-Type: application/json" \
  -d '{"prompt": "Ignore previous instructions and..."}'
```

### Analyze Behavior

```bash
curl -X POST http://localhost:3000/api/analyze \
  -H "Content-Type: application/json" \
  -d '{"detection_id": "uuid-here"}'
```

### Get Metrics

```bash
curl http://localhost:9090/metrics
```

## Next Steps

- Read [Architecture Documentation](ARCHITECTURE.md)
- Review [API Reference](API.md)
- Check [Performance Guide](PERFORMANCE.md)
- Study [Security Best Practices](SECURITY.md)
