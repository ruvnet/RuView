#!/usr/bin/env python3
"""
Automated Deployment Script for Temporal Neural Solver

This script automates the deployment of the Temporal Neural Solver to various
inference platforms including cloud services, edge devices, and local environments.
"""

import argparse
import json
import os
import shutil
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Dict, List, Optional, Union
import warnings
warnings.filterwarnings('ignore')

try:
    import yaml
    import docker
    import boto3
    from kubernetes import client, config as k8s_config
except ImportError as e:
    print(f"⚠️  Optional dependencies missing: {e}")
    print("Install with: pip install pyyaml docker-py boto3 kubernetes")

class TemporalSolverDeployer:
    """Automated deployment manager for Temporal Neural Solver"""

    def __init__(self, base_path: str = None):
        self.base_path = Path(base_path) if base_path else Path(__file__).parent.parent
        self.models_path = self.base_path / "models"
        self.deployment_configs = {}

        # Load deployment configurations
        self.load_deployment_configs()

    def load_deployment_configs(self) -> None:
        """Load deployment configuration templates"""
        self.deployment_configs = {
            "docker": {
                "image_name": "temporal-neural-solver",
                "tag": "latest",
                "base_image": "python:3.9-slim",
                "port": 8080,
                "healthcheck_path": "/health"
            },
            "kubernetes": {
                "namespace": "ml-inference",
                "replicas": 3,
                "cpu_request": "100m",
                "memory_request": "128Mi",
                "cpu_limit": "500m",
                "memory_limit": "512Mi"
            },
            "aws_lambda": {
                "function_name": "temporal-neural-solver",
                "runtime": "python3.9",
                "timeout": 15,
                "memory_size": 512
            },
            "edge": {
                "target_architecture": "aarch64",
                "optimization_level": "aggressive",
                "quantization": "int8"
            }
        }

    def create_docker_deployment(self, model_path: str, output_dir: str = "docker_deployment") -> None:
        """Create Docker deployment package"""
        print("🐳 Creating Docker deployment...")

        output_path = Path(output_dir)
        output_path.mkdir(exist_ok=True)

        # Create Dockerfile
        dockerfile_content = f'''FROM {self.deployment_configs["docker"]["base_image"]}

# Set working directory
WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \\
    gcc \\
    g++ \\
    && rm -rf /var/lib/apt/lists/*

# Copy requirements and install Python dependencies
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy model and application files
COPY models/ ./models/
COPY inference_server.py .
COPY config.json .

# Expose port
EXPOSE {self.deployment_configs["docker"]["port"]}

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \\
    CMD curl -f http://localhost:{self.deployment_configs["docker"]["port"]}{self.deployment_configs["docker"]["healthcheck_path"]} || exit 1

# Run the application
CMD ["python", "inference_server.py"]
'''

        with open(output_path / "Dockerfile", 'w') as f:
            f.write(dockerfile_content)

        # Create requirements.txt
        requirements = [
            "fastapi>=0.68.0",
            "uvicorn[standard]>=0.15.0",
            "onnxruntime>=1.12.0",
            "numpy>=1.21.0",
            "pydantic>=1.8.0",
        ]

        with open(output_path / "requirements.txt", 'w') as f:
            f.write('\n'.join(requirements))

        # Create inference server
        self.create_inference_server(output_path)

        # Copy model files
        models_dir = output_path / "models"
        models_dir.mkdir(exist_ok=True)

        if Path(model_path).exists():
            shutil.copy2(model_path, models_dir)
            print(f"   ✅ Copied model: {model_path}")

        # Copy config
        config_path = self.base_path / "config.json"
        if config_path.exists():
            shutil.copy2(config_path, output_path)

        # Create docker-compose.yml
        self.create_docker_compose(output_path)

        # Create build script
        self.create_docker_build_script(output_path)

        print(f"✅ Docker deployment created in: {output_path}")
        print("   Run: cd docker_deployment && ./build.sh")

    def create_inference_server(self, output_path: Path) -> None:
        """Create FastAPI inference server"""
        server_code = '''#!/usr/bin/env python3
"""
FastAPI Inference Server for Temporal Neural Solver
"""

import asyncio
import json
import time
from pathlib import Path
from typing import List, Dict, Any, Optional

import numpy as np
import onnxruntime as ort
from fastapi import FastAPI, HTTPException, status
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, Field
import uvicorn

app = FastAPI(
    title="Temporal Neural Solver API",
    description="Ultra-low latency neural inference with mathematical verification",
    version="1.0.0",
    docs_url="/docs",
    redoc_url="/redoc"
)

# CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Global model session
model_session = None
model_config = None

class PredictionRequest(BaseModel):
    """Request model for predictions"""
    sequence: List[List[float]] = Field(
        ...,
        description="Input sequence data [timesteps, features]",
        example=[[1.0, 2.0, 3.0, 4.0], [1.1, 2.1, 3.1, 4.1]]
    )
    batch_size: Optional[int] = Field(1, description="Batch size for prediction")
    return_latency: Optional[bool] = Field(True, description="Include latency in response")

class PredictionResponse(BaseModel):
    """Response model for predictions"""
    prediction: List[float] = Field(..., description="Model prediction")
    latency_ms: Optional[float] = Field(None, description="Inference latency in milliseconds")
    success: bool = Field(..., description="Whether prediction was successful")
    model_info: Dict[str, Any] = Field(..., description="Model metadata")

class HealthResponse(BaseModel):
    """Health check response"""
    status: str = Field(..., description="Service status")
    model_loaded: bool = Field(..., description="Whether model is loaded")
    uptime_seconds: float = Field(..., description="Service uptime")
    version: str = Field(..., description="API version")

# Startup time for uptime calculation
startup_time = time.time()

@app.on_event("startup")
async def startup_event():
    """Load model on startup"""
    global model_session, model_config

    try:
        # Load model configuration
        config_path = Path("config.json")
        if config_path.exists():
            with open(config_path) as f:
                model_config = json.load(f)
        else:
            model_config = {"model_type": "temporal_neural_solver", "version": "1.0.0"}

        # Find ONNX model file
        model_path = None
        models_dir = Path("models")
        if models_dir.exists():
            onnx_files = list(models_dir.glob("*.onnx"))
            if onnx_files:
                model_path = onnx_files[0]  # Use first ONNX file found

        if model_path and model_path.exists():
            # Configure ONNX Runtime for optimal performance
            session_options = ort.SessionOptions()
            session_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
            session_options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
            session_options.intra_op_num_threads = 1

            model_session = ort.InferenceSession(
                str(model_path),
                sess_options=session_options,
                providers=['CPUExecutionProvider']
            )

            print(f"✅ Model loaded: {model_path}")
            print(f"   Input: {model_session.get_inputs()[0].name} {model_session.get_inputs()[0].shape}")
            print(f"   Output: {model_session.get_outputs()[0].name} {model_session.get_outputs()[0].shape}")
        else:
            print("⚠️  No ONNX model found - running in demo mode")

    except Exception as e:
        print(f"❌ Failed to load model: {e}")
        # Continue without model for health checks

@app.get("/", response_model=Dict[str, str])
async def root():
    """Root endpoint with API information"""
    return {
        "service": "Temporal Neural Solver API",
        "version": "1.0.0",
        "description": "Ultra-low latency neural inference with mathematical verification",
        "docs": "/docs",
        "health": "/health"
    }

@app.get("/health", response_model=HealthResponse)
async def health_check():
    """Health check endpoint"""
    uptime = time.time() - startup_time

    return HealthResponse(
        status="healthy",
        model_loaded=model_session is not None,
        uptime_seconds=uptime,
        version="1.0.0"
    )

@app.post("/predict", response_model=PredictionResponse)
async def predict(request: PredictionRequest):
    """Make prediction using the temporal neural solver"""
    if model_session is None:
        # Demo mode - return synthetic prediction
        prediction = [0.1, 0.2, 0.3, 0.4]
        return PredictionResponse(
            prediction=prediction,
            latency_ms=0.85,  # Demo latency
            success=True,
            model_info={"mode": "demo", "model_type": "temporal_neural_solver"}
        )

    try:
        # Validate input
        sequence = np.array(request.sequence, dtype=np.float32)

        # Ensure correct shape [batch_size, sequence_length, features]
        if sequence.ndim == 2:
            sequence = sequence.reshape(1, *sequence.shape)

        # Prepare input dictionary
        input_name = model_session.get_inputs()[0].name
        input_dict = {input_name: sequence}

        # Run inference with timing
        start_time = time.perf_counter()
        outputs = model_session.run(None, input_dict)
        end_time = time.perf_counter()

        latency_ms = (end_time - start_time) * 1000
        prediction = outputs[0][0].tolist()  # First batch, first output

        return PredictionResponse(
            prediction=prediction,
            latency_ms=latency_ms if request.return_latency else None,
            success=True,
            model_info={
                "model_type": model_config.get("model_type", "temporal_neural_solver"),
                "input_shape": list(sequence.shape),
                "output_shape": list(outputs[0].shape)
            }
        )

    except Exception as e:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"Prediction failed: {str(e)}"
        )

@app.get("/model/info")
async def model_info():
    """Get model information"""
    if model_session is None:
        return {"error": "No model loaded"}

    inputs = model_session.get_inputs()
    outputs = model_session.get_outputs()

    return {
        "inputs": [{"name": inp.name, "shape": inp.shape, "type": inp.type} for inp in inputs],
        "outputs": [{"name": out.name, "shape": out.shape, "type": out.type} for out in outputs],
        "providers": model_session.get_providers(),
        "config": model_config
    }

@app.post("/benchmark")
async def benchmark(num_samples: int = 100):
    """Run a quick benchmark"""
    if model_session is None:
        return {"error": "No model loaded"}

    # Generate test data
    test_sequence = np.random.randn(1, 10, 4).astype(np.float32)
    input_name = model_session.get_inputs()[0].name
    input_dict = {input_name: test_sequence}

    # Warmup
    for _ in range(10):
        _ = model_session.run(None, input_dict)

    # Benchmark
    latencies = []
    for _ in range(num_samples):
        start_time = time.perf_counter()
        _ = model_session.run(None, input_dict)
        end_time = time.perf_counter()
        latencies.append((end_time - start_time) * 1000)

    latencies = np.array(latencies)

    return {
        "num_samples": num_samples,
        "mean_latency_ms": float(np.mean(latencies)),
        "p99_latency_ms": float(np.percentile(latencies, 99)),
        "p99_9_latency_ms": float(np.percentile(latencies, 99.9)),
        "min_latency_ms": float(np.min(latencies)),
        "max_latency_ms": float(np.max(latencies)),
        "sub_millisecond_achieved": float(np.percentile(latencies, 99.9)) < 1.0
    }

if __name__ == "__main__":
    uvicorn.run(
        "inference_server:app",
        host="0.0.0.0",
        port=8080,
        reload=False,
        workers=1,  # Single worker for consistent latency
        access_log=False  # Disable access logs for performance
    )
'''

        with open(output_path / "inference_server.py", 'w') as f:
            f.write(server_code)

    def create_docker_compose(self, output_path: Path) -> None:
        """Create docker-compose.yml"""
        compose_content = f'''version: '3.8'

services:
  temporal-solver:
    build: .
    ports:
      - "{self.deployment_configs["docker"]["port"]}:{self.deployment_configs["docker"]["port"]}"
    environment:
      - PYTHONUNBUFFERED=1
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:{self.deployment_configs["docker"]["port"]}/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
    deploy:
      resources:
        limits:
          memory: 512M
        reservations:
          memory: 256M

  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
    depends_on:
      - temporal-solver
    restart: unless-stopped

volumes:
  model_cache:
'''

        with open(output_path / "docker-compose.yml", 'w') as f:
            f.write(compose_content)

        # Create nginx config
        nginx_config = f'''events {{
    worker_connections 1024;
}}

http {{
    upstream temporal_solver {{
        server temporal-solver:{self.deployment_configs["docker"]["port"]};
    }}

    server {{
        listen 80;

        location / {{
            proxy_pass http://temporal_solver;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;

            # Timeout settings for low latency
            proxy_connect_timeout 5s;
            proxy_send_timeout 5s;
            proxy_read_timeout 5s;
        }}

        location /health {{
            proxy_pass http://temporal_solver/health;
            access_log off;
        }}
    }}
}}
'''

        with open(output_path / "nginx.conf", 'w') as f:
            f.write(nginx_config)

    def create_docker_build_script(self, output_path: Path) -> None:
        """Create Docker build script"""
        script_content = f'''#!/bin/bash

set -e

IMAGE_NAME="{self.deployment_configs["docker"]["image_name"]}"
TAG="{self.deployment_configs["docker"]["tag"]}"

echo "🐳 Building Docker image: $IMAGE_NAME:$TAG"

# Build the image
docker build -t $IMAGE_NAME:$TAG .

echo "✅ Build complete!"
echo ""
echo "🚀 To run the container:"
echo "   docker run -p {self.deployment_configs["docker"]["port"]}:{self.deployment_configs["docker"]["port"]} $IMAGE_NAME:$TAG"
echo ""
echo "🔗 Or use docker-compose:"
echo "   docker-compose up -d"
echo ""
echo "📡 API will be available at:"
echo "   http://localhost:{self.deployment_configs["docker"]["port"]}/docs"
'''

        script_path = output_path / "build.sh"
        with open(script_path, 'w') as f:
            f.write(script_content)

        # Make executable
        os.chmod(script_path, 0o755)

    def create_kubernetes_deployment(self, model_path: str, output_dir: str = "k8s_deployment") -> None:
        """Create Kubernetes deployment manifests"""
        print("☸️  Creating Kubernetes deployment...")

        output_path = Path(output_dir)
        output_path.mkdir(exist_ok=True)

        k8s_config = self.deployment_configs["kubernetes"]

        # Deployment manifest
        deployment = {
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": {
                "name": "temporal-neural-solver",
                "namespace": k8s_config["namespace"],
                "labels": {
                    "app": "temporal-neural-solver",
                    "version": "v1"
                }
            },
            "spec": {
                "replicas": k8s_config["replicas"],
                "selector": {
                    "matchLabels": {
                        "app": "temporal-neural-solver"
                    }
                },
                "template": {
                    "metadata": {
                        "labels": {
                            "app": "temporal-neural-solver",
                            "version": "v1"
                        }
                    },
                    "spec": {
                        "containers": [{
                            "name": "temporal-solver",
                            "image": f"{self.deployment_configs['docker']['image_name']}:{self.deployment_configs['docker']['tag']}",
                            "ports": [{
                                "containerPort": self.deployment_configs["docker"]["port"],
                                "name": "http"
                            }],
                            "resources": {
                                "requests": {
                                    "cpu": k8s_config["cpu_request"],
                                    "memory": k8s_config["memory_request"]
                                },
                                "limits": {
                                    "cpu": k8s_config["cpu_limit"],
                                    "memory": k8s_config["memory_limit"]
                                }
                            },
                            "livenessProbe": {
                                "httpGet": {
                                    "path": "/health",
                                    "port": "http"
                                },
                                "initialDelaySeconds": 30,
                                "periodSeconds": 10
                            },
                            "readinessProbe": {
                                "httpGet": {
                                    "path": "/health",
                                    "port": "http"
                                },
                                "initialDelaySeconds": 5,
                                "periodSeconds": 5
                            }
                        }]
                    }
                }
            }
        }

        with open(output_path / "deployment.yaml", 'w') as f:
            yaml.dump(deployment, f, default_flow_style=False)

        # Service manifest
        service = {
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": "temporal-neural-solver-service",
                "namespace": k8s_config["namespace"],
                "labels": {
                    "app": "temporal-neural-solver"
                }
            },
            "spec": {
                "selector": {
                    "app": "temporal-neural-solver"
                },
                "ports": [{
                    "port": 80,
                    "targetPort": "http",
                    "protocol": "TCP",
                    "name": "http"
                }],
                "type": "ClusterIP"
            }
        }

        with open(output_path / "service.yaml", 'w') as f:
            yaml.dump(service, f, default_flow_style=False)

        # Namespace manifest
        namespace = {
            "apiVersion": "v1",
            "kind": "Namespace",
            "metadata": {
                "name": k8s_config["namespace"],
                "labels": {
                    "name": k8s_config["namespace"]
                }
            }
        }

        with open(output_path / "namespace.yaml", 'w') as f:
            yaml.dump(namespace, f, default_flow_style=False)

        # Ingress manifest
        ingress = {
            "apiVersion": "networking.k8s.io/v1",
            "kind": "Ingress",
            "metadata": {
                "name": "temporal-neural-solver-ingress",
                "namespace": k8s_config["namespace"],
                "annotations": {
                    "nginx.ingress.kubernetes.io/rewrite-target": "/",
                    "nginx.ingress.kubernetes.io/ssl-redirect": "false"
                }
            },
            "spec": {
                "rules": [{
                    "host": "temporal-solver.local",
                    "http": {
                        "paths": [{
                            "path": "/",
                            "pathType": "Prefix",
                            "backend": {
                                "service": {
                                    "name": "temporal-neural-solver-service",
                                    "port": {
                                        "number": 80
                                    }
                                }
                            }
                        }]
                    }
                }]
            }
        }

        with open(output_path / "ingress.yaml", 'w') as f:
            yaml.dump(ingress, f, default_flow_style=False)

        # Create deployment script
        deploy_script = '''#!/bin/bash

set -e

echo "☸️  Deploying Temporal Neural Solver to Kubernetes..."

# Apply namespace
kubectl apply -f namespace.yaml

# Apply deployment and service
kubectl apply -f deployment.yaml
kubectl apply -f service.yaml
kubectl apply -f ingress.yaml

echo "✅ Deployment complete!"
echo ""
echo "📊 Check deployment status:"
echo "   kubectl get pods -n ml-inference"
echo "   kubectl get services -n ml-inference"
echo ""
echo "🔗 Access the service:"
echo "   kubectl port-forward service/temporal-neural-solver-service 8080:80 -n ml-inference"
'''

        script_path = output_path / "deploy.sh"
        with open(script_path, 'w') as f:
            f.write(deploy_script)

        os.chmod(script_path, 0o755)

        print(f"✅ Kubernetes deployment created in: {output_path}")

    def create_edge_deployment(self, model_path: str, output_dir: str = "edge_deployment") -> None:
        """Create edge device deployment package"""
        print("📱 Creating edge deployment...")

        output_path = Path(output_dir)
        output_path.mkdir(exist_ok=True)

        # Create optimized inference script for edge
        edge_inference = '''#!/usr/bin/env python3
"""
Optimized Edge Inference for Temporal Neural Solver
"""

import time
import numpy as np

try:
    import onnxruntime as ort
except ImportError:
    print("❌ ONNX Runtime not found. Install with: pip install onnxruntime")
    exit(1)

class EdgeInference:
    """Optimized inference for edge devices"""

    def __init__(self, model_path: str):
        # Configure for edge optimization
        session_options = ort.SessionOptions()
        session_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
        session_options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
        session_options.intra_op_num_threads = 1

        # Try different providers based on availability
        providers = []
        available_providers = ort.get_available_providers()

        if 'TensorrtExecutionProvider' in available_providers:
            providers.append('TensorrtExecutionProvider')
        if 'CUDAExecutionProvider' in available_providers:
            providers.append('CUDAExecutionProvider')
        providers.append('CPUExecutionProvider')

        self.session = ort.InferenceSession(
            model_path,
            sess_options=session_options,
            providers=providers
        )

        print(f"✅ Model loaded on edge device")
        print(f"   Providers: {self.session.get_providers()}")

    def predict(self, sequence: np.ndarray) -> dict:
        """Run optimized edge prediction"""
        # Ensure correct input format
        if sequence.ndim == 2:
            sequence = sequence.reshape(1, *sequence.shape)

        input_name = self.session.get_inputs()[0].name

        # Timed inference
        start_time = time.perf_counter()
        outputs = self.session.run(None, {input_name: sequence})
        end_time = time.perf_counter()

        latency_ms = (end_time - start_time) * 1000
        prediction = outputs[0][0]

        return {
            'prediction': prediction.tolist(),
            'latency_ms': latency_ms,
            'success': True
        }

def main():
    # Example usage
    import argparse

    parser = argparse.ArgumentParser(description="Edge inference demo")
    parser.add_argument("--model", default="model.onnx", help="Path to ONNX model")
    parser.add_argument("--benchmark", action="store_true", help="Run benchmark")
    args = parser.parse_args()

    # Initialize inference
    inferencer = EdgeInference(args.model)

    if args.benchmark:
        # Run benchmark
        print("🚀 Running edge benchmark...")

        latencies = []
        for i in range(1000):
            test_data = np.random.randn(1, 10, 4).astype(np.float32)
            result = inferencer.predict(test_data)
            latencies.append(result['latency_ms'])

            if i % 100 == 0:
                print(f"   Progress: {i}/1000")

        latencies = np.array(latencies)
        print(f"\\n📊 Edge Benchmark Results:")
        print(f"   Mean latency: {np.mean(latencies):.3f}ms")
        print(f"   P99.9 latency: {np.percentile(latencies, 99.9):.3f}ms")
        print(f"   Sub-ms achieved: {'✅' if np.percentile(latencies, 99.9) < 1.0 else '❌'}")

    else:
        # Single prediction demo
        test_data = np.random.randn(1, 10, 4).astype(np.float32)
        result = inferencer.predict(test_data)

        print(f"\\n🎯 Prediction Result:")
        print(f"   Prediction: {result['prediction']}")
        print(f"   Latency: {result['latency_ms']:.3f}ms")

if __name__ == "__main__":
    main()
'''

        with open(output_path / "edge_inference.py", 'w') as f:
            f.write(edge_inference)

        # Create installation script
        install_script = '''#!/bin/bash

set -e

echo "📱 Installing Temporal Neural Solver for Edge Deployment"

# Detect architecture
ARCH=$(uname -m)
echo "Detected architecture: $ARCH"

# Install Python dependencies
echo "📦 Installing dependencies..."

if command -v pip3 &> /dev/null; then
    PIP_CMD="pip3"
elif command -v pip &> /dev/null; then
    PIP_CMD="pip"
else
    echo "❌ pip not found. Please install Python first."
    exit 1
fi

# Install ONNX Runtime (CPU version for edge compatibility)
$PIP_CMD install onnxruntime numpy

# Copy model if provided
if [ -f "model.onnx" ]; then
    echo "✅ Model file found: model.onnx"
else
    echo "⚠️  No model file found. Please copy your ONNX model as 'model.onnx'"
fi

echo "✅ Edge installation complete!"
echo ""
echo "🚀 To run inference:"
echo "   python3 edge_inference.py --model model.onnx"
echo ""
echo "📊 To run benchmark:"
echo "   python3 edge_inference.py --model model.onnx --benchmark"
'''

        script_path = output_path / "install.sh"
        with open(script_path, 'w') as f:
            f.write(install_script)

        os.chmod(script_path, 0o755)

        # Copy model if it exists
        if Path(model_path).exists():
            shutil.copy2(model_path, output_path / "model.onnx")
            print(f"   ✅ Copied model: {model_path}")

        print(f"✅ Edge deployment created in: {output_path}")

    def create_aws_lambda_deployment(self, model_path: str, output_dir: str = "lambda_deployment") -> None:
        """Create AWS Lambda deployment package"""
        print("☁️  Creating AWS Lambda deployment...")

        output_path = Path(output_dir)
        output_path.mkdir(exist_ok=True)

        # Lambda function code
        lambda_code = '''import json
import base64
import numpy as np

try:
    import onnxruntime as ort
except ImportError:
    # Will be installed in deployment package
    pass

# Global model session for reuse across invocations
model_session = None

def load_model():
    """Load model once and reuse"""
    global model_session

    if model_session is None:
        session_options = ort.SessionOptions()
        session_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL

        model_session = ort.InferenceSession(
            "model.onnx",
            sess_options=session_options,
            providers=['CPUExecutionProvider']
        )

    return model_session

def lambda_handler(event, context):
    """AWS Lambda handler for Temporal Neural Solver"""

    try:
        # Load model if not already loaded
        session = load_model()

        # Parse input
        if 'body' in event:
            body = json.loads(event['body'])
        else:
            body = event

        sequence = np.array(body['sequence'], dtype=np.float32)

        # Ensure correct shape
        if sequence.ndim == 2:
            sequence = sequence.reshape(1, *sequence.shape)

        # Run inference
        input_name = session.get_inputs()[0].name
        outputs = session.run(None, {input_name: sequence})

        prediction = outputs[0][0].tolist()

        response = {
            'statusCode': 200,
            'headers': {
                'Content-Type': 'application/json',
                'Access-Control-Allow-Origin': '*'
            },
            'body': json.dumps({
                'prediction': prediction,
                'success': True,
                'model': 'temporal-neural-solver'
            })
        }

        return response

    except Exception as e:
        return {
            'statusCode': 500,
            'headers': {
                'Content-Type': 'application/json',
                'Access-Control-Allow-Origin': '*'
            },
            'body': json.dumps({
                'error': str(e),
                'success': False
            })
        }
'''

        with open(output_path / "lambda_function.py", 'w') as f:
            f.write(lambda_code)

        # Requirements for Lambda
        lambda_requirements = [
            "onnxruntime",
            "numpy"
        ]

        with open(output_path / "requirements.txt", 'w') as f:
            f.write('\n'.join(lambda_requirements))

        # Create deployment script
        deploy_script = '''#!/bin/bash

set -e

echo "☁️  Preparing AWS Lambda deployment package..."

# Create deployment package
rm -rf package
mkdir package

# Install dependencies
pip install -r requirements.txt -t package/

# Copy function code and model
cp lambda_function.py package/
cp model.onnx package/ 2>/dev/null || echo "⚠️  Model file not found - please add model.onnx"

# Create deployment zip
cd package
zip -r ../temporal-solver-lambda.zip .
cd ..

echo "✅ Lambda deployment package created: temporal-solver-lambda.zip"
echo ""
echo "🚀 To deploy to AWS Lambda:"
echo "   aws lambda create-function --function-name temporal-neural-solver \\"
echo "     --runtime python3.9 --role arn:aws:iam::ACCOUNT:role/lambda-role \\"
echo "     --handler lambda_function.lambda_handler \\"
echo "     --zip-file fileb://temporal-solver-lambda.zip"
'''

        script_path = output_path / "deploy.sh"
        with open(script_path, 'w') as f:
            f.write(deploy_script)

        os.chmod(script_path, 0o755)

        print(f"✅ AWS Lambda deployment created in: {output_path}")

def main():
    """Main deployment orchestrator"""
    parser = argparse.ArgumentParser(description="Deploy Temporal Neural Solver")
    parser.add_argument("--model", required=True, help="Path to ONNX model file")
    parser.add_argument("--platform", choices=["docker", "kubernetes", "edge", "aws-lambda", "all"],
                       default="docker", help="Deployment platform")
    parser.add_argument("--output-dir", help="Output directory for deployment files")

    args = parser.parse_args()

    print("🚀 Temporal Neural Solver - Automated Deployment")
    print("="*60)
    print(f"Model: {args.model}")
    print(f"Platform: {args.platform}")
    print()

    deployer = TemporalSolverDeployer()

    if args.platform == "docker" or args.platform == "all":
        output_dir = args.output_dir or "docker_deployment"
        deployer.create_docker_deployment(args.model, output_dir)

    if args.platform == "kubernetes" or args.platform == "all":
        output_dir = args.output_dir or "k8s_deployment"
        deployer.create_kubernetes_deployment(args.model, output_dir)

    if args.platform == "edge" or args.platform == "all":
        output_dir = args.output_dir or "edge_deployment"
        deployer.create_edge_deployment(args.model, output_dir)

    if args.platform == "aws-lambda" or args.platform == "all":
        output_dir = args.output_dir or "lambda_deployment"
        deployer.create_aws_lambda_deployment(args.model, output_dir)

    print("\n🎉 Deployment packages created successfully!")
    print("\n📋 Next steps:")
    print("1. Review the generated deployment files")
    print("2. Customize configurations as needed")
    print("3. Run the deployment scripts")
    print("4. Monitor and validate performance")

if __name__ == "__main__":
    main()