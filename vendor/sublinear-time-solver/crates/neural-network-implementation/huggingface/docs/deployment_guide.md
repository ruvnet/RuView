# 🚀 Deployment Guide - Temporal Neural Solver

## Overview

This comprehensive guide covers deploying the Temporal Neural Solver across various platforms and environments, from cloud services to edge devices, ensuring optimal performance and reliability in production.

## 📋 Prerequisites

### System Requirements

**Minimum Requirements:**
- **CPU**: Any x86_64 or ARM64 processor
- **Memory**: 100MB RAM
- **Storage**: 50MB disk space
- **OS**: Linux, macOS, or Windows

**Recommended for Production:**
- **CPU**: Multi-core processor with AVX2 support
- **Memory**: 512MB RAM
- **Storage**: 1GB SSD
- **Network**: Low-latency connection (< 10ms)

### Software Dependencies

**Python Environment:**
```bash
pip install onnxruntime>=1.12.0 numpy>=1.21.0 fastapi>=0.68.0 uvicorn>=0.15.0
```

**Rust Environment:**
```bash
cargo install temporal-neural-net
```

**Docker:**
```bash
docker --version  # 20.10+
docker-compose --version  # 1.29+
```

## 🐳 Docker Deployment

### Quick Start

```bash
# Download deployment scripts
git clone https://github.com/research/sublinear-time-solver
cd neural-network-implementation/huggingface

# Create Docker deployment
python scripts/deploy_inference.py --model system_b.onnx --platform docker

# Build and run
cd docker_deployment
./build.sh
docker-compose up -d
```

### Custom Docker Deployment

#### 1. Create Dockerfile

```dockerfile
FROM python:3.9-slim

# Install system dependencies
RUN apt-get update && apt-get install -y \
    gcc g++ curl \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy requirements and install
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy application files
COPY models/ ./models/
COPY inference_server.py .
COPY config.json .

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Run application
CMD ["python", "inference_server.py"]
```

#### 2. Configure Requirements

```txt
# requirements.txt
fastapi>=0.68.0
uvicorn[standard]>=0.15.0
onnxruntime>=1.12.0
numpy>=1.21.0
pydantic>=1.8.0
```

#### 3. Create Production Docker Compose

```yaml
# docker-compose.prod.yml
version: '3.8'

services:
  temporal-solver:
    build:
      context: .
      dockerfile: Dockerfile.prod
    ports:
      - "8080:8080"
    environment:
      - PYTHONUNBUFFERED=1
      - ONNX_NUM_THREADS=1
      - OMP_NUM_THREADS=1
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
    deploy:
      resources:
        limits:
          memory: 512M
          cpus: '1.0'
        reservations:
          memory: 256M
          cpus: '0.5'

  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
      - ./ssl:/etc/nginx/ssl:ro
    depends_on:
      - temporal-solver
    restart: unless-stopped

  redis:
    image: redis:alpine
    ports:
      - "6379:6379"
    restart: unless-stopped
    deploy:
      resources:
        limits:
          memory: 128M
```

#### 4. Nginx Configuration

```nginx
# nginx.conf
events {
    worker_connections 1024;
}

http {
    upstream temporal_solver {
        server temporal-solver:8080;
    }

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=api:10m rate=100r/s;

    server {
        listen 80;
        listen 443 ssl http2;

        # SSL configuration
        ssl_certificate /etc/nginx/ssl/cert.pem;
        ssl_certificate_key /etc/nginx/ssl/key.pem;

        # Performance optimizations
        keepalive_timeout 65;
        client_max_body_size 1M;

        location / {
            limit_req zone=api burst=10 nodelay;

            proxy_pass http://temporal_solver;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;

            # Timeout settings for low latency
            proxy_connect_timeout 5s;
            proxy_send_timeout 5s;
            proxy_read_timeout 5s;
        }

        location /health {
            proxy_pass http://temporal_solver/health;
            access_log off;
        }

        location /metrics {
            proxy_pass http://temporal_solver/metrics;
            access_log off;
        }
    }
}
```

## ☸️ Kubernetes Deployment

### Quick Deployment

```bash
# Create Kubernetes deployment
python scripts/deploy_inference.py --model system_b.onnx --platform kubernetes

# Deploy to cluster
cd k8s_deployment
kubectl apply -f namespace.yaml
kubectl apply -f deployment.yaml
kubectl apply -f service.yaml
kubectl apply -f ingress.yaml
```

### Production Kubernetes Configuration

#### 1. Namespace and ConfigMap

```yaml
# namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: temporal-solver
  labels:
    name: temporal-solver

---
# configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: temporal-solver-config
  namespace: temporal-solver
data:
  config.json: |
    {
      "model_config": {
        "system_type": "B",
        "precision": "int8",
        "enable_optimization": true
      },
      "inference_config": {
        "max_latency_ms": 1.0,
        "batch_size": 1
      }
    }
```

#### 2. Deployment with HPA

```yaml
# deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: temporal-solver
  namespace: temporal-solver
  labels:
    app: temporal-solver
    version: v1
spec:
  replicas: 3
  selector:
    matchLabels:
      app: temporal-solver
  template:
    metadata:
      labels:
        app: temporal-solver
        version: v1
    spec:
      containers:
      - name: temporal-solver
        image: temporal-neural-solver:latest
        ports:
        - containerPort: 8080
          name: http
        env:
        - name: ONNX_NUM_THREADS
          value: "1"
        - name: OMP_NUM_THREADS
          value: "1"
        resources:
          requests:
            cpu: 100m
            memory: 128Mi
          limits:
            cpu: 500m
            memory: 512Mi
        livenessProbe:
          httpGet:
            path: /health
            port: http
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: http
          initialDelaySeconds: 5
          periodSeconds: 5
        volumeMounts:
        - name: config
          mountPath: /app/config.json
          subPath: config.json
      volumes:
      - name: config
        configMap:
          name: temporal-solver-config

---
# hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: temporal-solver-hpa
  namespace: temporal-solver
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: temporal-solver
  minReplicas: 3
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

#### 3. Service and Ingress

```yaml
# service.yaml
apiVersion: v1
kind: Service
metadata:
  name: temporal-solver-service
  namespace: temporal-solver
  labels:
    app: temporal-solver
spec:
  selector:
    app: temporal-solver
  ports:
  - port: 80
    targetPort: 8080
    protocol: TCP
    name: http
  type: ClusterIP

---
# ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: temporal-solver-ingress
  namespace: temporal-solver
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
    nginx.ingress.kubernetes.io/rate-limit: "100"
    cert-manager.io/cluster-issuer: "letsencrypt-prod"
spec:
  tls:
  - hosts:
    - api.temporal-solver.ai
    secretName: temporal-solver-tls
  rules:
  - host: api.temporal-solver.ai
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: temporal-solver-service
            port:
              number: 80
```

#### 4. Monitoring with Prometheus

```yaml
# servicemonitor.yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: temporal-solver-metrics
  namespace: temporal-solver
spec:
  selector:
    matchLabels:
      app: temporal-solver
  endpoints:
  - port: http
    path: /metrics
    interval: 30s
```

## ☁️ Cloud Platform Deployments

### AWS Deployment

#### 1. AWS Lambda

```bash
# Create Lambda deployment
python scripts/deploy_inference.py --model system_b.onnx --platform aws-lambda

# Deploy using AWS CLI
cd lambda_deployment
aws lambda create-function \
  --function-name temporal-neural-solver \
  --runtime python3.9 \
  --role arn:aws:iam::YOUR-ACCOUNT:role/lambda-execution-role \
  --handler lambda_function.lambda_handler \
  --zip-file fileb://temporal-solver-lambda.zip \
  --timeout 15 \
  --memory-size 512
```

#### 2. AWS ECS Fargate

```yaml
# ecs-task-definition.json
{
  "family": "temporal-solver",
  "networkMode": "awsvpc",
  "requiresCompatibilities": ["FARGATE"],
  "cpu": "256",
  "memory": "512",
  "executionRoleArn": "arn:aws:iam::YOUR-ACCOUNT:role/ecsTaskExecutionRole",
  "containerDefinitions": [
    {
      "name": "temporal-solver",
      "image": "YOUR-ACCOUNT.dkr.ecr.REGION.amazonaws.com/temporal-solver:latest",
      "portMappings": [
        {
          "containerPort": 8080,
          "protocol": "tcp"
        }
      ],
      "essential": true,
      "logConfiguration": {
        "logDriver": "awslogs",
        "options": {
          "awslogs-group": "/ecs/temporal-solver",
          "awslogs-region": "us-west-2",
          "awslogs-stream-prefix": "ecs"
        }
      },
      "healthCheck": {
        "command": [
          "CMD-SHELL",
          "curl -f http://localhost:8080/health || exit 1"
        ],
        "interval": 30,
        "timeout": 5,
        "retries": 3
      }
    }
  ]
}
```

#### 3. AWS API Gateway Integration

```yaml
# api-gateway-template.yaml
AWSTemplateFormatVersion: '2010-09-09'
Transform: AWS::Serverless-2016-10-31

Resources:
  TemporalSolverApi:
    Type: AWS::Serverless::Api
    Properties:
      StageName: prod
      Cors:
        AllowMethods: "'POST, GET, OPTIONS'"
        AllowHeaders: "'Content-Type'"
        AllowOrigin: "'*'"
      ThrottleConfig:
        RateLimit: 1000
        BurstLimit: 2000

  TemporalSolverFunction:
    Type: AWS::Serverless::Function
    Properties:
      FunctionName: temporal-neural-solver
      CodeUri: lambda_deployment/
      Handler: lambda_function.lambda_handler
      Runtime: python3.9
      Timeout: 15
      MemorySize: 512
      Events:
        PredictApi:
          Type: Api
          Properties:
            RestApiId: !Ref TemporalSolverApi
            Path: /predict
            Method: POST
```

### Google Cloud Platform

#### 1. Cloud Run

```bash
# Build and deploy to Cloud Run
gcloud builds submit --tag gcr.io/PROJECT-ID/temporal-solver
gcloud run deploy temporal-solver \
  --image gcr.io/PROJECT-ID/temporal-solver \
  --platform managed \
  --region us-central1 \
  --allow-unauthenticated \
  --memory 512Mi \
  --cpu 1 \
  --concurrency 100 \
  --max-instances 10
```

#### 2. GKE Deployment

```yaml
# gke-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: temporal-solver
spec:
  replicas: 3
  selector:
    matchLabels:
      app: temporal-solver
  template:
    metadata:
      labels:
        app: temporal-solver
    spec:
      nodeSelector:
        cloud.google.com/gke-nodepool: high-memory
      containers:
      - name: temporal-solver
        image: gcr.io/PROJECT-ID/temporal-solver:latest
        ports:
        - containerPort: 8080
        resources:
          requests:
            cpu: 100m
            memory: 128Mi
          limits:
            cpu: 500m
            memory: 512Mi
```

### Microsoft Azure

#### 1. Azure Container Instances

```bash
# Deploy to Azure Container Instances
az container create \
  --resource-group myResourceGroup \
  --name temporal-solver \
  --image temporal-neural-solver:latest \
  --cpu 1 \
  --memory 1 \
  --ports 8080 \
  --dns-name-label temporal-solver \
  --restart-policy Always
```

#### 2. Azure Kubernetes Service

```yaml
# aks-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: temporal-solver
spec:
  replicas: 3
  selector:
    matchLabels:
      app: temporal-solver
  template:
    metadata:
      labels:
        app: temporal-solver
    spec:
      containers:
      - name: temporal-solver
        image: myregistry.azurecr.io/temporal-solver:latest
        ports:
        - containerPort: 8080
        resources:
          requests:
            cpu: 100m
            memory: 128Mi
          limits:
            cpu: 500m
            memory: 512Mi
```

## 📱 Edge Device Deployment

### IoT and Embedded Systems

#### 1. Raspberry Pi Deployment

```bash
# Install on Raspberry Pi
curl -sSL https://raw.githubusercontent.com/research/temporal-solver/main/install.sh | bash

# Or manual installation
sudo apt update
sudo apt install python3-pip
pip3 install onnxruntime numpy

# Copy model and run
python3 edge_inference.py --model system_b.onnx
```

#### 2. NVIDIA Jetson

```bash
# Install NVIDIA ONNX Runtime
sudo apt update
sudo apt install python3-pip
pip3 install onnxruntime-gpu

# Optimize for Jetson
export CUDA_VISIBLE_DEVICES=0
export OMP_NUM_THREADS=4
python3 edge_inference.py --model system_b.onnx --benchmark
```

#### 3. Docker on Edge

```dockerfile
# Dockerfile.edge
FROM arm64v8/python:3.9-slim

# Install minimal dependencies
RUN pip install --no-cache-dir onnxruntime numpy

COPY system_b.onnx /app/
COPY edge_inference.py /app/

WORKDIR /app

CMD ["python", "edge_inference.py", "--model", "system_b.onnx"]
```

### Mobile Deployment

#### 1. Android (via Termux)

```bash
# Install in Termux
pkg update
pkg install python
pip install onnxruntime numpy

# Run inference
python edge_inference.py --model system_b.onnx
```

#### 2. iOS (via PyTorch Mobile)

```python
# Convert ONNX to PyTorch Mobile
import torch
import onnx
from onnx2torch import convert

# Load ONNX model
onnx_model = onnx.load("system_b.onnx")
pytorch_model = convert(onnx_model)

# Convert to mobile
scripted_model = torch.jit.script(pytorch_model)
scripted_model._save_for_lite_interpreter("system_b_mobile.ptl")
```

## 🎛️ Production Configuration

### Performance Optimization

#### 1. ONNX Runtime Configuration

```python
import onnxruntime as ort

# Optimal session options
session_options = ort.SessionOptions()

# Graph optimizations
session_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL

# Execution settings
session_options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
session_options.intra_op_num_threads = 1  # Single thread for latency
session_options.inter_op_num_threads = 1

# Memory optimization
session_options.enable_mem_pattern = True
session_options.enable_cpu_mem_arena = True

# Provider options
provider_options = [
    {
        'use_arena': 1,
        'arena_extend_strategy': 'kSameAsRequested',
    }
]

session = ort.InferenceSession(
    "system_b.onnx",
    sess_options=session_options,
    providers=[('CPUExecutionProvider', provider_options)]
)
```

#### 2. Environment Variables

```bash
# Set for optimal performance
export OMP_NUM_THREADS=1
export MKL_NUM_THREADS=1
export NUMEXPR_NUM_THREADS=1
export VECLIB_MAXIMUM_THREADS=1

# ONNX Runtime specific
export ORT_DISABLE_ALL_OPTIMIZATION=0
export ORT_ENABLE_CPU_FP16_OPS=1
```

#### 3. System-Level Optimizations

```bash
# CPU governor for consistent performance
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor

# Disable CPU power management
echo 1 | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo

# Set process priority
nice -n -20 python inference_server.py

# CPU affinity (bind to specific cores)
taskset -c 0-3 python inference_server.py
```

### Monitoring and Observability

#### 1. Application Metrics

```python
# Add to inference server
from prometheus_client import Counter, Histogram, generate_latest

# Metrics
REQUEST_COUNT = Counter('requests_total', 'Total requests')
REQUEST_LATENCY = Histogram('request_latency_seconds', 'Request latency')
ERROR_COUNT = Counter('errors_total', 'Total errors')

@app.middleware("http")
async def metrics_middleware(request, call_next):
    start_time = time.time()

    REQUEST_COUNT.inc()

    try:
        response = await call_next(request)
        return response
    except Exception as e:
        ERROR_COUNT.inc()
        raise
    finally:
        REQUEST_LATENCY.observe(time.time() - start_time)

@app.get("/metrics")
async def metrics():
    return Response(generate_latest(), media_type="text/plain")
```

#### 2. Health Checks

```python
@app.get("/health")
async def health_check():
    # Check model availability
    if model_session is None:
        raise HTTPException(status_code=503, detail="Model not loaded")

    # Check memory usage
    memory_usage = psutil.virtual_memory().percent
    if memory_usage > 90:
        raise HTTPException(status_code=503, detail="High memory usage")

    # Check response time
    start = time.time()
    test_input = np.random.randn(1, 10, 4).astype(np.float32)
    _ = model_session.run(None, {"input_sequence": test_input})
    latency = time.time() - start

    if latency > 0.005:  # 5ms threshold
        raise HTTPException(status_code=503, detail="High latency")

    return {
        "status": "healthy",
        "latency_ms": latency * 1000,
        "memory_usage_percent": memory_usage,
        "timestamp": time.time()
    }
```

#### 3. Logging Configuration

```python
import logging
import structlog

# Configure structured logging
logging.basicConfig(level=logging.INFO)
structlog.configure(
    processors=[
        structlog.stdlib.filter_by_level,
        structlog.stdlib.add_logger_name,
        structlog.stdlib.add_log_level,
        structlog.stdlib.PositionalArgumentsFormatter(),
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.StackInfoRenderer(),
        structlog.processors.format_exc_info,
        structlog.processors.UnicodeDecoder(),
        structlog.processors.JSONRenderer()
    ],
    context_class=dict,
    logger_factory=structlog.stdlib.LoggerFactory(),
    wrapper_class=structlog.stdlib.BoundLogger,
    cache_logger_on_first_use=True,
)

logger = structlog.get_logger()

# Log predictions
@app.post("/predict")
async def predict(request: PredictionRequest):
    start_time = time.perf_counter()

    try:
        result = run_inference(request.sequence)
        latency_ms = (time.perf_counter() - start_time) * 1000

        logger.info(
            "prediction_completed",
            latency_ms=latency_ms,
            input_shape=request.sequence.shape,
            prediction_shape=result.shape
        )

        return {"prediction": result.tolist(), "latency_ms": latency_ms}

    except Exception as e:
        logger.error("prediction_failed", error=str(e))
        raise HTTPException(status_code=500, detail=str(e))
```

## 🔐 Security Configuration

### Authentication and Authorization

#### 1. API Key Authentication

```python
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials

security = HTTPBearer()

async def validate_api_key(credentials: HTTPAuthorizationCredentials = Depends(security)):
    if credentials.credentials != os.getenv("API_KEY"):
        raise HTTPException(status_code=401, detail="Invalid API key")
    return credentials.credentials

@app.post("/predict")
async def predict(
    request: PredictionRequest,
    api_key: str = Depends(validate_api_key)
):
    # Protected endpoint
    pass
```

#### 2. Rate Limiting

```python
from slowapi import Limiter, _rate_limit_exceeded_handler
from slowapi.util import get_remote_address
from slowapi.errors import RateLimitExceeded

limiter = Limiter(key_func=get_remote_address)
app.state.limiter = limiter
app.add_exception_handler(RateLimitExceeded, _rate_limit_exceeded_handler)

@app.post("/predict")
@limiter.limit("100/minute")
async def predict(request: Request):
    # Rate limited endpoint
    pass
```

### Network Security

#### 1. TLS Configuration

```yaml
# kubernetes-tls.yaml
apiVersion: v1
kind: Secret
metadata:
  name: temporal-solver-tls
  namespace: temporal-solver
type: kubernetes.io/tls
data:
  tls.crt: <base64-encoded-cert>
  tls.key: <base64-encoded-key>
```

#### 2. Network Policies

```yaml
# network-policy.yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: temporal-solver-netpol
  namespace: temporal-solver
spec:
  podSelector:
    matchLabels:
      app: temporal-solver
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: ingress-nginx
    ports:
    - protocol: TCP
      port: 8080
  egress:
  - to: []
    ports:
    - protocol: TCP
      port: 53
    - protocol: UDP
      port: 53
```

## 📊 Performance Validation

### Deployment Testing

```bash
#!/bin/bash
# deployment-test.sh

set -e

echo "🧪 Testing deployment..."

# Health check
echo "Checking health endpoint..."
curl -f http://localhost:8080/health || exit 1

# Latency test
echo "Testing prediction latency..."
python scripts/benchmark_onnx.py system_b.onnx --quick

# Load test
echo "Running load test..."
ab -n 1000 -c 10 -T application/json -p test_payload.json http://localhost:8080/predict

echo "✅ Deployment test complete!"
```

### Continuous Monitoring

```python
# monitoring.py
import time
import requests
import json
from dataclasses import dataclass

@dataclass
class MonitoringResult:
    timestamp: float
    latency_ms: float
    success: bool
    error: str = None

class DeploymentMonitor:
    def __init__(self, endpoint: str):
        self.endpoint = endpoint
        self.results = []

    def check_health(self) -> MonitoringResult:
        start = time.time()
        try:
            response = requests.get(f"{self.endpoint}/health", timeout=5)
            latency = (time.time() - start) * 1000

            return MonitoringResult(
                timestamp=time.time(),
                latency_ms=latency,
                success=response.status_code == 200
            )
        except Exception as e:
            return MonitoringResult(
                timestamp=time.time(),
                latency_ms=(time.time() - start) * 1000,
                success=False,
                error=str(e)
            )

    def check_prediction(self) -> MonitoringResult:
        start = time.time()
        try:
            test_data = {
                "sequence": [[1.0, 2.0, 3.0, 4.0]] * 10
            }

            response = requests.post(
                f"{self.endpoint}/predict",
                json=test_data,
                timeout=5
            )
            latency = (time.time() - start) * 1000

            return MonitoringResult(
                timestamp=time.time(),
                latency_ms=latency,
                success=response.status_code == 200
            )
        except Exception as e:
            return MonitoringResult(
                timestamp=time.time(),
                latency_ms=(time.time() - start) * 1000,
                success=False,
                error=str(e)
            )

# Run monitoring
if __name__ == "__main__":
    monitor = DeploymentMonitor("http://localhost:8080")

    while True:
        health = monitor.check_health()
        prediction = monitor.check_prediction()

        print(f"Health: {health.success} ({health.latency_ms:.1f}ms)")
        print(f"Prediction: {prediction.success} ({prediction.latency_ms:.1f}ms)")

        time.sleep(60)  # Check every minute
```

## 🚨 Troubleshooting

### Common Issues

#### 1. High Latency

**Symptoms:**
- P99.9 latency > 1ms
- Slow response times

**Solutions:**
```bash
# Check CPU frequency scaling
cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor

# Set performance mode
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor

# Check ONNX Runtime configuration
export ORT_DISABLE_ALL_OPTIMIZATION=0
export OMP_NUM_THREADS=1

# Monitor system resources
htop
iostat -x 1
```

#### 2. Memory Issues

**Symptoms:**
- Out of memory errors
- High memory usage

**Solutions:**
```bash
# Monitor memory usage
free -h
ps aux --sort=-%mem | head

# Optimize ONNX Runtime
session_options.enable_cpu_mem_arena = True
session_options.enable_mem_pattern = True

# Set memory limits in Docker
docker run --memory=512m temporal-solver
```

#### 3. Model Loading Failures

**Symptoms:**
- Model not found errors
- ONNX Runtime errors

**Solutions:**
```bash
# Verify model file
ls -la system_b.onnx
file system_b.onnx

# Check ONNX Runtime version compatibility
python -c "import onnxruntime; print(onnxruntime.__version__)"

# Validate ONNX model
python -c "import onnx; onnx.checker.check_model('system_b.onnx')"
```

### Performance Debugging

```python
# performance_debug.py
import onnxruntime as ort
import numpy as np
import time

# Enable profiling
session_options = ort.SessionOptions()
session_options.enable_profiling = True

session = ort.InferenceSession("system_b.onnx", sess_options=session_options)

# Run with profiling
input_data = np.random.randn(1, 10, 4).astype(np.float32)
outputs = session.run(None, {"input_sequence": input_data})

# Get profile results
prof_file = session.end_profiling()
print(f"Profile saved to: {prof_file}")
```

## 📚 Best Practices

### 1. **Single-Purpose Deployment**
- Deploy one model per container/service
- Avoid mixing inference with other workloads
- Use dedicated hardware when possible

### 2. **Resource Management**
- Set appropriate CPU/memory limits
- Use resource quotas in Kubernetes
- Monitor resource utilization

### 3. **Scaling Strategy**
- Use horizontal pod autoscaling
- Scale based on latency metrics
- Implement circuit breakers

### 4. **Monitoring and Alerting**
- Monitor P99.9 latency continuously
- Set up alerts for latency violations
- Track prediction accuracy over time

### 5. **Security**
- Use API keys for authentication
- Implement rate limiting
- Keep dependencies updated

## 🔗 Related Resources

- **[API Reference](api_reference.md)**: Complete API documentation
- **[Troubleshooting Guide](troubleshooting.md)**: Common issues and solutions
- **[Examples](../examples/)**: Complete deployment examples
- **[Scripts](../scripts/)**: Automated deployment tools

---

*This deployment guide ensures your Temporal Neural Solver achieves optimal performance in production environments. For additional support, see our [troubleshooting guide](troubleshooting.md) or contact the development team.*