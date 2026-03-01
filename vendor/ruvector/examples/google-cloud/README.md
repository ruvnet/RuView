# RuVector Cloud Run GPU Deployment

High-performance vector database benchmarks and deployment on Google Cloud Run with GPU acceleration (NVIDIA L4).

## Table of Contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Step-by-Step Tutorial](#step-by-step-tutorial)
- [Deployment Options](#deployment-options)
- [Benchmarking](#benchmarking)
- [Architecture](#architecture)
- [API Reference](#api-reference)
- [Troubleshooting](#troubleshooting)

## Overview

This example provides:

- **GPU-Accelerated Benchmarks**: SIMD (AVX-512, AVX2, NEON) and CUDA optimized operations
- **Cloud Run Deployment**: Scalable, serverless deployment with GPU support
- **Multiple Deployment Models**:
  - Single-node benchmark service
  - Attention/GNN inference service
  - Raft consensus cluster (3+ nodes)
  - Primary-replica replication

### Supported RuVector Capabilities

| Capability | Description | Cloud Run Support |
|------------|-------------|-------------------|
| **Core Vector Search** | HNSW indexing, k-NN search | ✅ Full GPU |
| **Attention Mechanisms** | Multi-head attention layers | ✅ Full GPU |
| **GNN Inference** | Graph neural network forward pass | ✅ Full GPU |
| **Raft Consensus** | Distributed consensus protocol | ✅ Multi-service |
| **Replication** | Primary-replica data replication | ✅ Multi-service |
| **Quantization** | INT8/PQ compression | ✅ GPU optimized |

## Prerequisites

### Required Tools

```bash
# Google Cloud CLI
curl https://sdk.cloud.google.com | bash
gcloud init

# Docker
# Install from: https://docs.docker.com/get-docker/

# Rust (for local development)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### GCP Setup

```bash
# Authenticate
gcloud auth login

# Set project
gcloud config set project YOUR_PROJECT_ID

# Enable required APIs
gcloud services enable \
    run.googleapis.com \
    containerregistry.googleapis.com \
    cloudbuild.googleapis.com \
    compute.googleapis.com
```

## Quick Start

### 1. One-Command Deployment

```bash
cd examples/google-cloud

# Setup and deploy
./deploy.sh setup
./deploy.sh build Dockerfile.gpu latest
./deploy.sh push latest
./deploy.sh deploy latest true  # true = GPU enabled

# Run benchmark
./deploy.sh benchmark ruvector-benchmark quick
```

### 2. View Results

```bash
# Get service URL
gcloud run services describe ruvector-benchmark \
    --region=us-central1 \
    --format='value(status.url)'

# Test endpoints
curl $URL/health
curl $URL/info
curl -X POST $URL/benchmark/quick
```

## Step-by-Step Tutorial

### Step 1: Project Setup

```bash
# Clone the repository
git clone https://github.com/ruvnet/ruvector.git
cd ruvector/examples/google-cloud

# Set environment variables
export GCP_PROJECT_ID="your-project-id"
export GCP_REGION="us-central1"

# Run setup
./deploy.sh setup
```

### Step 2: Build the Docker Image

**Option A: Local Build (faster iteration)**

```bash
# Build locally
./deploy.sh build Dockerfile.gpu latest

# Push to Container Registry
./deploy.sh push latest
```

**Option B: Cloud Build (no local Docker required)**

```bash
# Build in the cloud
./deploy.sh build-cloud Dockerfile.gpu latest
```

### Step 3: Deploy to Cloud Run

**Basic Deployment (with GPU)**

```bash
./deploy.sh deploy latest true
```

**Custom Configuration**

```bash
# High-memory configuration for large vector sets
MEMORY=16Gi CPU=8 ./deploy.sh deploy latest true

# Scale settings
MIN_INSTANCES=1 MAX_INSTANCES=20 ./deploy.sh deploy latest true
```

### Step 4: Run Benchmarks

```bash
# Quick benchmark (128d, 10k vectors)
./deploy.sh benchmark ruvector-benchmark quick

# Distance computation benchmark
./deploy.sh benchmark ruvector-benchmark distance

# HNSW index benchmark
./deploy.sh benchmark ruvector-benchmark hnsw

# Full benchmark suite
./deploy.sh benchmark ruvector-benchmark full
```

### Step 5: View Results

```bash
# Get all results
./deploy.sh results ruvector-benchmark

# View logs
./deploy.sh logs ruvector-benchmark

# Check service status
./deploy.sh status
```

## Deployment Options

### 1. Single-Node Benchmark Service

Best for: Development, testing, single-user benchmarks

```bash
./deploy.sh deploy latest true
```

### 2. Attention/GNN Service

Best for: Neural network inference, embedding generation

```bash
./deploy.sh deploy-attention latest
```

**Features:**
- 16GB memory for large models
- 3-layer GNN with 8 attention heads
- Optimized for batch inference

### 3. Raft Consensus Cluster

Best for: High availability, consistent distributed state

```bash
# Deploy 3-node cluster
CLUSTER_SIZE=3 ./deploy.sh deploy-raft

# Deploy 5-node cluster for higher fault tolerance
CLUSTER_SIZE=5 ./deploy.sh deploy-raft
```

**Architecture:**
```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Node 1    │◄───►│   Node 2    │◄───►│   Node 3    │
│  (Leader)   │     │  (Follower) │     │  (Follower) │
└─────────────┘     └─────────────┘     └─────────────┘
       │                  │                   │
       └──────────────────┴───────────────────┘
                    Raft Consensus
```

**Configuration:**
```bash
# Environment variables for Raft nodes
RUVECTOR_NODE_ID=0              # Node identifier (0, 1, 2, ...)
RUVECTOR_CLUSTER_SIZE=3         # Total cluster size
RUVECTOR_RAFT_ELECTION_TIMEOUT=150  # Election timeout (ms)
RUVECTOR_RAFT_HEARTBEAT_INTERVAL=50 # Heartbeat interval (ms)
```

### 4. Primary-Replica Replication

Best for: Read scaling, geographic distribution

```bash
# Deploy with 3 replicas
./deploy.sh deploy-replication 3
```

**Architecture:**
```
                    ┌─────────────┐
          Writes───►│   Primary   │
                    └──────┬──────┘
                           │ Replication
          ┌────────────────┼────────────────┐
          ▼                ▼                ▼
    ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
    │  Replica 1  │  │  Replica 2  │  │  Replica 3  │
    └─────────────┘  └─────────────┘  └─────────────┘
          │                │                │
          └────────────────┴────────────────┘
                      Reads (load balanced)
```

**Configuration:**
```bash
# Primary node
RUVECTOR_MODE=primary
RUVECTOR_REPLICATION_FACTOR=3
RUVECTOR_SYNC_MODE=async  # or "sync" for strong consistency

# Replica nodes
RUVECTOR_MODE=replica
RUVECTOR_PRIMARY_URL=https://ruvector-primary-xxx.run.app
```

## Benchmarking

### Available Benchmarks

| Benchmark | Description | Dimensions | Vector Count |
|-----------|-------------|------------|--------------|
| `quick` | Fast sanity check | 128 | 10,000 |
| `distance` | Distance computation | configurable | configurable |
| `hnsw` | HNSW index search | configurable | configurable |
| `gnn` | GNN forward pass | 256 | 10,000 nodes |
| `cuda` | CUDA kernel perf | - | - |
| `quantization` | INT8/PQ compression | configurable | configurable |

### Running Benchmarks via API

```bash
# Quick benchmark
curl -X POST https://YOUR-SERVICE-URL/benchmark/quick

# Custom distance benchmark
curl -X POST "https://YOUR-SERVICE-URL/benchmark/distance?dims=768&num_vectors=100000&batch_size=64"

# Custom HNSW benchmark
curl -X POST "https://YOUR-SERVICE-URL/benchmark/hnsw?dims=768&num_vectors=100000&k=10"

# Full custom benchmark
curl -X POST https://YOUR-SERVICE-URL/benchmark \
    -H "Content-Type: application/json" \
    -d '{
        "dims": 768,
        "num_vectors": 100000,
        "num_queries": 1000,
        "k": 10,
        "benchmark_type": "hnsw"
    }'
```

### Expected Performance

**NVIDIA L4 GPU (Cloud Run default):**

| Operation | Dimensions | Vectors | P99 Latency | QPS |
|-----------|------------|---------|-------------|-----|
| L2 Distance | 128 | 10k | 0.5ms | 2,000 |
| L2 Distance | 768 | 100k | 5ms | 200 |
| HNSW Search | 128 | 100k | 1ms | 1,000 |
| HNSW Search | 768 | 1M | 10ms | 100 |
| GNN Forward | 256 | 10k nodes | 15ms | 66 |

### SIMD Capabilities

The benchmark automatically detects and uses:

| Architecture | SIMD | Vector Width | Speedup |
|--------------|------|--------------|---------|
| x86_64 | AVX-512 | 16 floats | 8-16x |
| x86_64 | AVX2 | 8 floats | 4-8x |
| x86_64 | SSE4.1 | 4 floats | 2-4x |
| ARM64 | NEON | 4 floats | 2-4x |

## Architecture

### System Components

```
┌─────────────────────────────────────────────────────────────────┐
│                        Cloud Run                                 │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │ HTTP Server │  │  Benchmark  │  │    SIMD/GPU Runtime     │ │
│  │   (Axum)    │  │   Engine    │  │  AVX-512 │ CUDA │ NEON  │ │
│  └──────┬──────┘  └──────┬──────┘  └────────────────┬────────┘ │
│         │                │                          │          │
│  ┌──────┴────────────────┴──────────────────────────┴────────┐ │
│  │                    RuVector Core                          │ │
│  │  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────────────┐  │ │
│  │  │  HNSW  │  │  GNN   │  │ Quant  │  │  Attention     │  │ │
│  │  │ Index  │  │ Layers │  │  INT8  │  │  Multi-Head    │  │ │
│  │  └────────┘  └────────┘  └────────┘  └────────────────┘  │ │
│  └───────────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                      NVIDIA L4 GPU                              │
└─────────────────────────────────────────────────────────────────┘
```

### File Structure

```
examples/google-cloud/
├── Cargo.toml              # Rust dependencies
├── Dockerfile.gpu          # GPU-optimized Docker image
├── cloudrun.yaml           # Cloud Run service configs
├── deploy.sh               # Deployment automation
├── README.md               # This file
└── src/
    ├── main.rs             # CLI entry point
    ├── benchmark.rs        # Benchmark implementations
    ├── simd.rs             # SIMD-optimized operations
    ├── cuda.rs             # GPU/CUDA operations
    ├── report.rs           # Report generation
    └── server.rs           # HTTP server for Cloud Run
```

## API Reference

### Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/` | API info and available endpoints |
| GET | `/health` | Health check |
| GET | `/info` | System information (GPU, SIMD, memory) |
| POST | `/benchmark` | Run custom benchmark |
| POST | `/benchmark/quick` | Run quick benchmark |
| POST | `/benchmark/distance` | Run distance benchmark |
| POST | `/benchmark/hnsw` | Run HNSW benchmark |
| GET | `/results` | Get all benchmark results |
| POST | `/results/clear` | Clear stored results |

### Health Check Response

```json
{
    "status": "healthy",
    "version": "0.1.0",
    "gpu_available": true,
    "gpu_name": "NVIDIA L4",
    "simd_capability": "AVX2",
    "uptime_secs": 3600
}
```

### Benchmark Request

```json
{
    "dims": 768,
    "num_vectors": 100000,
    "num_queries": 1000,
    "k": 10,
    "benchmark_type": "hnsw"
}
```

### Benchmark Response

```json
{
    "status": "success",
    "message": "Benchmark completed",
    "result": {
        "name": "hnsw_768d_100000v",
        "operation": "hnsw_search",
        "dimensions": 768,
        "num_vectors": 100000,
        "mean_time_ms": 2.5,
        "p50_ms": 2.1,
        "p95_ms": 3.8,
        "p99_ms": 5.2,
        "qps": 400.0,
        "memory_mb": 585.9,
        "gpu_enabled": true
    }
}
```

## Troubleshooting

### Common Issues

**1. GPU not detected**

```bash
# Check GPU availability
gcloud run services describe ruvector-benchmark \
    --region=us-central1 \
    --format='yaml(spec.template.metadata.annotations)'

# Ensure GPU annotations are present:
# run.googleapis.com/gpu-type: nvidia-l4
# run.googleapis.com/gpu-count: "1"
```

**2. Container fails to start**

```bash
# Check logs
./deploy.sh logs ruvector-benchmark 200

# Common causes:
# - Missing CUDA libraries (use nvidia/cuda base image)
# - Memory limit too low (increase MEMORY env var)
# - Health check failing (check /health endpoint)
```

**3. Slow cold starts**

```bash
# Set minimum instances
MIN_INSTANCES=1 ./deploy.sh deploy latest true

# Enable startup CPU boost (already in cloudrun.yaml)
```

**4. Out of memory**

```bash
# Increase memory allocation
MEMORY=16Gi ./deploy.sh deploy latest true

# Or reduce vector count in benchmark
curl -X POST "$URL/benchmark?num_vectors=50000"
```

### Performance Optimization

1. **Enable CPU boost for cold starts**
   ```yaml
   run.googleapis.com/startup-cpu-boost: "true"
   ```

2. **Disable CPU throttling**
   ```yaml
   run.googleapis.com/cpu-throttling: "false"
   ```

3. **Use Gen2 execution environment**
   ```yaml
   run.googleapis.com/execution-environment: gen2
   ```

4. **Tune concurrency based on workload**
   - CPU-bound: Lower concurrency (10-20)
   - Memory-bound: Medium concurrency (50-80)
   - I/O-bound: Higher concurrency (100+)

### Cleanup

```bash
# Remove all RuVector services
./deploy.sh cleanup

# Remove specific service
gcloud run services delete ruvector-benchmark --region=us-central1

# Remove container images
gcloud container images delete gcr.io/PROJECT_ID/ruvector-benchmark
```

## Cost Estimation

| Configuration | vCPU | Memory | GPU | Cost/hour |
|---------------|------|--------|-----|-----------|
| Basic | 2 | 4GB | None | ~$0.10 |
| GPU Standard | 4 | 8GB | L4 | ~$0.80 |
| GPU High-Mem | 8 | 16GB | L4 | ~$1.20 |
| Raft Cluster (3) | 6 | 12GB | None | ~$0.30 |

*Costs are approximate and vary by region. See [Cloud Run Pricing](https://cloud.google.com/run/pricing).*

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run benchmarks to verify performance
5. Submit a pull request

## License

MIT License - see [LICENSE](../../LICENSE) for details.
