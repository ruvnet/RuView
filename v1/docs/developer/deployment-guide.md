# Deployment Guide

## Overview

This guide provides comprehensive instructions for deploying the WiFi-DensePose system across different environments, from development to production. It covers containerized deployments, cloud platforms, edge computing, and monitoring setup.

## Table of Contents

1. [Deployment Overview](#deployment-overview)
2. [Prerequisites](#prerequisites)
3. [Environment Configuration](#environment-configuration)
4. [Docker Deployment](#docker-deployment)
5. [Kubernetes Deployment](#kubernetes-deployment)
6. [Cloud Platform Deployment](#cloud-platform-deployment)
7. [Edge Computing Deployment](#edge-computing-deployment)
8. [Database Setup](#database-setup)
9. [Monitoring and Logging](#monitoring-and-logging)
10. [Security Configuration](#security-configuration)
11. [Performance Optimization](#performance-optimization)
12. [Backup and Recovery](#backup-and-recovery)

## Deployment Overview

### Architecture Components

```
┌─────────────────────────────────────────────────────────────────┐
│                     Production Deployment                       │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │  Load Balancer  │  │   API Gateway   │  │   Web Dashboard │  │
│  │    (Nginx)      │  │   (FastAPI)     │  │    (React)      │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │ Neural Network  │  │ CSI Processor   │  │ Analytics       │  │
│  │   Service       │  │   Service       │  │   Service       │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │   PostgreSQL    │  │     Redis       │  │   File Storage  │  │
│  │  (TimescaleDB)  │  │    (Cache)      │  │   (MinIO/S3)    │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │   Prometheus    │  │    Grafana      │  │      ELK        │  │
│  │  (Metrics)      │  │  (Dashboards)   │  │   (Logging)     │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Deployment Environments

1. **Development**: Local development with Docker Compose
2. **Staging**: Cloud-based staging environment for testing
3. **Production**: High-availability production deployment
4. **Edge**: Lightweight deployment for edge computing

## Prerequisites

### System Requirements

#### Minimum Requirements
- **CPU**: 4 cores (Intel i5 or AMD Ryzen 5 equivalent)
- **RAM**: 8 GB
- **Storage**: 100 GB SSD
- **Network**: 1 Gbps Ethernet
- **OS**: Ubuntu 20.04 LTS or CentOS 8

#### Recommended Requirements
- **CPU**: 8+ cores (Intel i7/Xeon or AMD Ryzen 7/EPYC)
- **RAM**: 32 GB
- **Storage**: 500 GB NVMe SSD
- **GPU**: NVIDIA RTX 3080 or better (for neural network acceleration)
- **Network**: 10 Gbps Ethernet
- **OS**: Ubuntu 22.04 LTS

### Software Dependencies

```bash
# Docker and Docker Compose
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo usermod -aG docker $USER

# Docker Compose
sudo curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
sudo chmod +x /usr/local/bin/docker-compose

# Kubernetes (optional)
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl

# NVIDIA Container Toolkit (for GPU support)
distribution=$(. /etc/os-release;echo $ID$VERSION_ID)
curl -s -L https://nvidia.github.io/nvidia-docker/gpgkey | sudo apt-key add -
curl -s -L https://nvidia.github.io/nvidia-docker/$distribution/nvidia-docker.list | sudo tee /etc/apt/sources.list.d/nvidia-docker.list
sudo apt-get update && sudo apt-get install -y nvidia-container-toolkit
sudo systemctl restart docker
```

## Environment Configuration

### Environment Variables

Create environment-specific configuration files:

#### Production Environment (`.env.prod`)

```bash
# Application Configuration
ENVIRONMENT=production
DEBUG=false
SECRET_KEY=your-super-secret-production-key-here
API_HOST=0.0.0.0
API_PORT=8000

# Database Configuration
DATABASE_URL=postgresql://wifi_user:secure_password@postgres:5432/wifi_densepose
DATABASE_POOL_SIZE=20
DATABASE_MAX_OVERFLOW=30

# Redis Configuration
REDIS_URL=redis://redis:6379/0
REDIS_PASSWORD=secure_redis_password

# Neural Network Configuration
MODEL_PATH=/app/models/densepose_production.pth
BATCH_SIZE=32
ENABLE_GPU=true
GPU_MEMORY_FRACTION=0.8

# CSI Processing Configuration
CSI_BUFFER_SIZE=1000
CSI_SAMPLING_RATE=30
ENABLE_PHASE_SANITIZATION=true

# Security Configuration
JWT_SECRET_KEY=your-jwt-secret-key-here
JWT_ALGORITHM=HS256
JWT_EXPIRATION_HOURS=24
ENABLE_RATE_LIMITING=true
RATE_LIMIT_REQUESTS=1000
RATE_LIMIT_WINDOW=3600

# Monitoring Configuration
ENABLE_METRICS=true
METRICS_PORT=9090
LOG_LEVEL=INFO
SENTRY_DSN=https://your-sentry-dsn@sentry.io/project-id

# Storage Configuration
STORAGE_BACKEND=s3
S3_BUCKET=wifi-densepose-data
S3_REGION=us-west-2
AWS_ACCESS_KEY_ID=your-access-key
AWS_SECRET_ACCESS_KEY=your-secret-key

# Domain Configuration
DEFAULT_DOMAIN=healthcare
ENABLE_MULTI_DOMAIN=true

# Performance Configuration
WORKERS=4
WORKER_CONNECTIONS=1000
ENABLE_ASYNC_PROCESSING=true
```

#### Staging Environment (`.env.staging`)

```bash
# Application Configuration
ENVIRONMENT=staging
DEBUG=true
SECRET_KEY=staging-secret-key
API_HOST=0.0.0.0
API_PORT=8000

# Database Configuration
DATABASE_URL=postgresql://wifi_user:staging_password@postgres:5432/wifi_densepose_staging
DATABASE_POOL_SIZE=10

# Reduced resource configuration for staging
BATCH_SIZE=16
WORKERS=2
CSI_BUFFER_SIZE=500

# Enable additional logging for debugging
LOG_LEVEL=DEBUG
ENABLE_SQL_LOGGING=true
```

#### Development Environment (`.env.dev`)

```bash
# Application Configuration
ENVIRONMENT=development
DEBUG=true
SECRET_KEY=dev-secret-key
API_HOST=localhost
API_PORT=8000

# Local database
DATABASE_URL=postgresql://postgres:postgres@localhost:5432/wifi_densepose_dev
REDIS_URL=redis://localhost:6379/0

# Mock hardware for development
MOCK_HARDWARE=true
MOCK_CSI_DATA=true

# Development optimizations
BATCH_SIZE=8
WORKERS=1
ENABLE_HOT_RELOAD=true
```

### Configuration Management

```python
# src/config/environments.py
from pydantic import BaseSettings
from typing import Optional

class BaseConfig(BaseSettings):
    """Base configuration class."""
    
    # Application
    environment: str = "development"
    debug: bool = False
    secret_key: str
    api_host: str = "0.0.0.0"
    api_port: int = 8000
    
    # Database
    database_url: str
    database_pool_size: int = 10
    database_max_overflow: int = 20
    
    # Redis
    redis_url: str
    redis_password: Optional[str] = None
    
    # Neural Network
    model_path: str = "/app/models/densepose.pth"
    batch_size: int = 32
    enable_gpu: bool = True
    
    class Config:
        env_file = ".env"

class DevelopmentConfig(BaseConfig):
    """Development configuration."""
    debug: bool = True
    mock_hardware: bool = True
    log_level: str = "DEBUG"

class ProductionConfig(BaseConfig):
    """Production configuration."""
    debug: bool = False
    enable_metrics: bool = True
    log_level: str = "INFO"
    
    # Security
    jwt_secret_key: str
    enable_rate_limiting: bool = True
    
    # Performance
    workers: int = 4
    worker_connections: int = 1000

class StagingConfig(BaseConfig):
    """Staging configuration."""
    debug: bool = True
    log_level: str = "DEBUG"
    enable_sql_logging: bool = True

def get_config():
    """Get configuration based on environment."""
    env = os.getenv("ENVIRONMENT", "development")
    
    if env == "production":
        return ProductionConfig()
    elif env == "staging":
        return StagingConfig()
    else:
        return DevelopmentConfig()
```

## Docker Deployment

### Production Docker Compose

```yaml
# docker-compose.prod.yml
version: '3.8'

services:
  # Load Balancer
  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx/nginx.conf:/etc/nginx/nginx.conf
      - ./nginx/ssl:/etc/nginx/ssl
      - ./nginx/logs:/var/log/nginx
    depends_on:
      - wifi-densepose-api
    restart: unless-stopped
    networks:
      - frontend

  # Main API Service
  wifi-densepose-api:
    build:
      context: .
      dockerfile: Dockerfile.prod
    environment:
      - ENVIRONMENT=production
    env_file:
      - .env.prod
    volumes:
      - ./data:/app/data
      - ./models:/app/models
      - ./logs:/app/logs
    depends_on:
      - postgres
      - redis
      - neural-network
    restart: unless-stopped
    networks:
      - frontend
      - backend
    deploy:
      replicas: 3
      resources:
        limits:
          memory: 4G
          cpus: '2.0'
        reservations:
          memory: 2G
          cpus: '1.0'

  # Neural Network Service
  neural-network:
    build:
      context: ./neural_network
      dockerfile: Dockerfile.gpu
    runtime: nvidia
    environment:
      - CUDA_VISIBLE_DEVICES=0
    env_file:
      - .env.prod
    volumes:
      - ./models:/app/models
      - ./neural_network/cache:/app/cache
    restart: unless-stopped
    networks:
      - backend
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]

  # CSI Processing Service
  csi-processor:
    build:
      context: ./hardware
      dockerfile: Dockerfile
    env_file:
      - .env.prod
    volumes:
      - ./data/csi:/app/data
    restart: unless-stopped
    networks:
      - backend
    ports:
      - "5500:5500"  # CSI data port

  # Database
  postgres:
    image: timescale/timescaledb:latest-pg14
    environment:
      - POSTGRES_DB=wifi_densepose
      - POSTGRES_USER=wifi_user
      - POSTGRES_PASSWORD_FILE=/run/secrets/postgres_password
    secrets:
      - postgres_password
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./database/init:/docker-entrypoint-initdb.d
    restart: unless-stopped
    networks:
      - backend
    deploy:
      resources:
        limits:
          memory: 8G
          cpus: '4.0'

  # Redis Cache
  redis:
    image: redis:7-alpine
    command: redis-server --requirepass ${REDIS_PASSWORD}
    volumes:
      - redis_data:/data
      - ./redis/redis.conf:/usr/local/etc/redis/redis.conf
    restart: unless-stopped
    networks:
      - backend

  # Monitoring
  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    restart: unless-stopped
    networks:
      - monitoring

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana_data:/var/lib/grafana
      - ./monitoring/grafana:/etc/grafana/provisioning
    restart: unless-stopped
    networks:
      - monitoring

volumes:
  postgres_data:
  redis_data:
  prometheus_data:
  grafana_data:

networks:
  frontend:
    driver: bridge
  backend:
    driver: bridge
  monitoring:
    driver: bridge

secrets:
  postgres_password:
    file: ./secrets/postgres_password.txt
```

### Production Dockerfile

```dockerfile
# Dockerfile.prod
FROM python:3.10-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    libopencv-dev \
    libffi-dev \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create virtual environment
RUN python -m venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

# Install Python dependencies
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Production stage
FROM python:3.10-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libopencv-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy virtual environment from builder
COPY --from=builder /opt/venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

# Create app user
RUN groupadd -r appuser && useradd -r -g appuser appuser

# Set working directory
WORKDIR /app

# Copy application code
COPY --chown=appuser:appuser src/ ./src/
COPY --chown=appuser:appuser scripts/ ./scripts/
COPY --chown=appuser:appuser alembic.ini ./

# Create necessary directories
RUN mkdir -p /app/data /app/logs /app/models && \
    chown -R appuser:appuser /app

# Switch to app user
USER appuser

# Health check
HEALTHCHECK --interval=30s --timeout=30s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8000/api/v1/health || exit 1

# Expose port
EXPOSE 8000

# Start application
CMD ["python", "-m", "src.api.main"]
```

### Nginx Configuration

```nginx
# nginx/nginx.conf
events {
    worker_connections 1024;
}

http {
    upstream wifi_densepose_api {
        server wifi-densepose-api:8000;
    }

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;

    server {
        listen 80;
        server_name your-domain.com;
        
        # Redirect HTTP to HTTPS
        return 301 https://$server_name$request_uri;
    }

    server {
        listen 443 ssl http2;
        server_name your-domain.com;

        # SSL Configuration
        ssl_certificate /etc/nginx/ssl/cert.pem;
        ssl_certificate_key /etc/nginx/ssl/key.pem;
        ssl_protocols TLSv1.2 TLSv1.3;
        ssl_ciphers ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512;
        ssl_prefer_server_ciphers off;

        # Security headers
        add_header X-Frame-Options DENY;
        add_header X-Content-Type-Options nosniff;
        add_header X-XSS-Protection "1; mode=block";
        add_header Strict-Transport-Security "max-age=63072000; includeSubDomains; preload";

        # API routes
        location /api/ {
            limit_req zone=api burst=20 nodelay;
            
            proxy_pass http://wifi_densepose_api;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
            
            # Timeouts
            proxy_connect_timeout 60s;
            proxy_send_timeout 60s;
            proxy_read_timeout 60s;
        }

        # WebSocket support
        location /ws/ {
            proxy_pass http://wifi_densepose_api;
            proxy_http_version 1.1;
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection "upgrade";
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }

        # Static files
        location /static/ {
            alias /app/static/;
            expires 1y;
            add_header Cache-Control "public, immutable";
        }

        # Health check
        location /health {
            access_log off;
            proxy_pass http://wifi_densepose_api/api/v1/health;
        }
    }
}
```

## Kubernetes Deployment

### Namespace and ConfigMap

```yaml
# k8s/namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: wifi-densepose
  labels:
    name: wifi-densepose

---
# k8s/configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: wifi-densepose-config
  namespace: wifi-densepose
data:
  ENVIRONMENT: "production"
  API_HOST: "0.0.0.0"
  API_PORT: "8000"
  LOG_LEVEL: "INFO"
  BATCH_SIZE: "32"
  WORKERS: "4"
```

### Secrets

```yaml
# k8s/secrets.yaml
apiVersion: v1
kind: Secret
metadata:
  name: wifi-densepose-secrets
  namespace: wifi-densepose
type: Opaque
data:
  SECRET_KEY: <base64-encoded-secret>
  DATABASE_URL: <base64-encoded-database-url>
  JWT_SECRET_KEY: <base64-encoded-jwt-secret>
  REDIS_PASSWORD: <base64-encoded-redis-password>
```

### API Deployment

```yaml
# k8s/api-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: wifi-densepose-api
  namespace: wifi-densepose
  labels:
    app: wifi-densepose-api
spec:
  replicas: 3
  selector:
    matchLabels:
      app: wifi-densepose-api
  template:
    metadata:
      labels:
        app: wifi-densepose-api
    spec:
      containers:
      - name: api
        image: wifi-densepose:latest
        ports:
        - containerPort: 8000
        envFrom:
        - configMapRef:
            name: wifi-densepose-config
        - secretRef:
            name: wifi-densepose-secrets
        resources:
          requests:
            memory: "2Gi"
            cpu: "1000m"
          limits:
            memory: "4Gi"
            cpu: "2000m"
        livenessProbe:
          httpGet:
            path: /api/v1/health
            port: 8000
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /api/v1/health
            port: 8000
          initialDelaySeconds: 5
          periodSeconds: 5
        volumeMounts:
        - name: data-volume
          mountPath: /app/data
        - name: models-volume
          mountPath: /app/models
      volumes:
      - name: data-volume
        persistentVolumeClaim:
          claimName: wifi-densepose-data-pvc
      - name: models-volume
        persistentVolumeClaim:
          claimName: wifi-densepose-models-pvc

---
apiVersion: v1
kind: Service
metadata:
  name: wifi-densepose-api-service
  namespace: wifi-densepose
spec:
  selector:
    app: wifi-densepose-api
  ports:
  - protocol: TCP
    port: 80
    targetPort: 8000
  type: ClusterIP
```

### Neural Network Deployment

```yaml
# k8s/neural-network-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: neural-network
  namespace: wifi-densepose
spec:
  replicas: 2
  selector:
    matchLabels:
      app: neural-network
  template:
    metadata:
      labels:
        app: neural-network
    spec:
      nodeSelector:
        accelerator: nvidia-tesla-k80
      containers:
      - name: neural-network
        image: wifi-densepose-neural:latest
        resources:
          requests:
            nvidia.com/gpu: 1
            memory: "4Gi"
            cpu: "2000m"
          limits:
            nvidia.com/gpu: 1
            memory: "8Gi"
            cpu: "4000m"
        envFrom:
        - configMapRef:
            name: wifi-densepose-config
        - secretRef:
            name: wifi-densepose-secrets
        volumeMounts:
        - name: models-volume
          mountPath: /app/models
      volumes:
      - name: models-volume
        persistentVolumeClaim:
          claimName: wifi-densepose-models-pvc

---
apiVersion: v1
kind: Service
metadata:
  name: neural-network-service
  namespace: wifi-densepose
spec:
  selector:
    app: neural-network
  ports:
  - protocol: TCP
    port: 8080
    targetPort: 8080
  type: ClusterIP
```

### Persistent Volumes

```yaml
# k8s/persistent-volumes.yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: wifi-densepose-data-pvc
  namespace: wifi-densepose
spec:
  accessModes:
    - ReadWriteMany
  resources:
    requests:
      storage: 100Gi
  storageClassName: fast-ssd

---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: wifi-densepose-models-pvc
  namespace: wifi-densepose
spec:
  accessModes:
    - ReadOnlyMany
  resources:
    requests:
      storage: 50Gi
  storageClassName: fast-ssd

---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: postgres-data-pvc
  namespace: wifi-densepose
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 200Gi
  storageClassName: fast-ssd
```

### Ingress

```yaml
# k8s/ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: wifi-densepose-ingress
  namespace: wifi-densepose
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/rate-limit: "100"
    nginx.ingress.kubernetes.io/rate-limit-window: "1m"
spec:
  tls:
  - hosts:
    - api.wifi-densepose.com
    secretName: wifi-densepose-tls
  rules:
  - host: api.wifi-densepose.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: wifi-densepose-api-service
            port:
              number: 80
```

## Cloud Platform Deployment

### AWS Deployment

#### ECS Task Definition

```json
{
  "family": "wifi-densepose",
  "networkMode": "awsvpc",
  "requiresCompatibilities": ["FARGATE"],
  "cpu": "2048",
  "memory": "4096",
  "executionRoleArn": "arn:aws:iam::account:role/ecsTaskExecutionRole",
  "taskRoleArn": "arn:aws:iam::account:role/ecsTaskRole",
  "containerDefinitions": [
    {
      "name": "wifi-densepose-api",
      "image": "your-account.dkr.ecr.region.amazonaws.com/wifi-densepose:latest",
      "portMappings": [
        {
          "containerPort": 8000,
          "protocol": "tcp"
        }
      ],
      "environment": [
        {
          "name": "ENVIRONMENT",
          "value": "production"
        }
      ],
      "secrets": [
        {
          "name": "DATABASE_URL",
          "valueFrom": "arn:aws:secretsmanager:region:account:secret:wifi-densepose/database-url"
        }
      ],
      "logConfiguration": {
        "logDriver": "awslogs",
        "options": {
          "awslogs-group": "/ecs/wifi-densepose",
          "awslogs-region": "us-west-2",
          "awslogs-stream-prefix": "ecs"
        }
      },
      "healthCheck": {
        "command": [
          "CMD-SHELL",
          "curl -f http://localhost:8000/api/v1/health || exit 1"
        ],
        "interval": 30,
        "timeout": 5,
        "retries": 3
      }
    }
  ]
}
```

#### CloudFormation Template

```yaml
# aws/cloudformation.yaml
AWSTemplateFormatVersion: '2010-09-09'
Description: 'WiFi-DensePose Infrastructure'

Parameters:
  Environment:
    Type: String
    Default: production
    AllowedValues: [development, staging, production]

Resources:
  # VPC and Networking
  VPC:
    Type: AWS::EC2::VPC
    Properties:
      CidrBlock: 10.0.0.0/16
      EnableDnsHostnames: true
      EnableDnsSupport: true
      Tags:
        - Key: Name
          Value: !Sub '${Environment}-wifi-densepose-vpc'

  PublicSubnet1:
    Type: AWS::EC2::Subnet
    Properties:
      VpcId: !Ref VPC
      AvailabilityZone: !Select [0, !GetAZs '']
      CidrBlock: 10.0.1.0/24
      MapPublicIpOnLaunch: true

  PublicSubnet2:
    Type: AWS::EC2::Subnet
    Properties:
      VpcId: !Ref VPC
      AvailabilityZone: !Select [1, !GetAZs '']
      CidrBlock: 10.0.2.0/24
      MapPublicIpOnLaunch: true

  # ECS Cluster
  ECSCluster:
    Type: AWS::ECS::Cluster
    Properties:
      ClusterName: !Sub '${Environment}-wifi-densepose'
      CapacityProviders:
        - FARGATE
        - FARGATE_SPOT

  # RDS Database
  DBSubnetGroup:
    Type: AWS::RDS::DBSubnetGroup
    Properties:
      DBSubnetGroupDescription: Subnet group for WiFi-DensePose database
      SubnetIds:
        - !Ref PublicSubnet1
        - !Ref PublicSubnet2

  Database:
    Type: AWS::RDS::DBInstance
    Properties:
      DBInstanceIdentifier: !Sub '${Environment}-wifi-densepose-db'
      DBInstanceClass: db.t3.medium
      Engine: postgres
      EngineVersion: '14.9'
      AllocatedStorage: 100
      StorageType: gp2
      DBName: wifi_densepose
      MasterUsername: wifi_user
      MasterUserPassword: !Ref DatabasePassword
      DBSubnetGroupName: !Ref DBSubnetGroup
      VPCSecurityGroups:
        - !Ref DatabaseSecurityGroup

  # ElastiCache Redis
  RedisSubnetGroup:
    Type: AWS::ElastiCache::SubnetGroup
    Properties:
      Description: Subnet group for Redis
      SubnetIds:
        - !Ref PublicSubnet1
        - !Ref PublicSubnet2

  RedisCluster:
    Type: AWS::ElastiCache::CacheCluster
    Properties:
      CacheNodeType: cache.t3.micro
      Engine: redis
      NumCacheNodes: 1
      CacheSubnetGroupName: !Ref RedisSubnetGroup
      VpcSecurityGroupIds:
        - !Ref RedisSecurityGroup

  # Application Load Balancer
  LoadBalancer:
    Type: AWS::ElasticLoadBalancingV2::LoadBalancer
    Properties:
      Name: !Sub '${Environment}-wifi-densepose-alb'
      Scheme: internet-facing
      Type: application
      Subnets:
        - !Ref PublicSubnet1
        - !Ref PublicSubnet2
      SecurityGroups:
        - !Ref LoadBalancerSecurityGroup

Outputs:
  LoadBalancerDNS:
    Description: DNS name of the load balancer
    Value: !GetAtt LoadBalancer.DNSName
    Export:
      Name: !Sub '${Environment}-LoadBalancerDNS'
```

### Google Cloud Platform Deployment

#### GKE Cluster Configuration

```yaml
# gcp/gke-cluster.yaml
apiVersion: container.v1
kind: Cluster
metadata:
  name: wifi-densepose-cluster
spec:
  location: us-central1
  initialNodeCount: 3
  nodeConfig:
    machineType: n1-standard-4
    diskSizeGb: 100
    oauthScopes:
    - https://www.googleapis.com/auth/cloud-platform
  addonsConfig:
    httpLoadBalancing:
      disabled: false
    horizontalPodAutoscaling:
      disabled: false
  network: default
  subnetwork: default
```

### Azure Deployment

#### Container Instances

```yaml
# azure/container-instances.yaml
apiVersion: 2019-12-01
location: East US
name: wifi-densepose-container-group
properties:
  containers:
  - name: wifi-densepose-api
    properties:
      image: your-registry.azurecr.io/wifi-densepose:latest
      resources:
        requests:
          cpu: 2
          memoryInGb: 4
      ports:
      - port: 8000
        protocol: TCP
      environmentVariables:
      - name: ENVIRONMENT
        value: production
      - name: DATABASE_URL
        secureValue: postgresql://user:pass@host:5432/db
  osType: Linux
  restartPolicy: Always
  ipAddress:
    type: Public
    ports:
    - protocol: TCP
      port: 8000
type: Microsoft.ContainerInstance/containerGroups
```

## Edge Computing Deployment

### Lightweight Configuration

```yaml
# docker-compose.edge.yml
version: '3.8'

services:
  wifi-densepose-edge:
    build:
      context: .
      dockerfile: Dockerfile.edge
    environment:
      - ENVIRONMENT=edge
      - ENABLE_GPU=false
      - BATCH_SIZE=8
      - WORKERS=1
      - DATABASE_URL=sqlite:///app/data/wifi_densepose.db
    volumes:
      - ./data:/app/data
      - ./models:/app/models
    ports:
      - "8000:8000"
    restart: unless-stopped
    deploy:
      resources:
        limits:
          memory: 2G
          cpus: '1.0'

  redis-edge:
    image: redis:7-alpine
    command: redis-server --maxmemory 256mb --maxmemory-policy allkeys-lru
    volumes:
      - redis_edge_data:/data
    restart: unless-stopped

volumes:
  redis_edge_data:
```

### Edge Dockerfile

```dockerfile
# Dockerfile.edge
FROM python:3.10-slim

# Install minimal dependencies
RUN apt-get update && apt-get install -y \
    libopencv-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Python dependencies
COPY requirements-edge.txt .
RUN pip install --no-cache-dir -r requirements-edge.txt

# Create app directory
WORKDIR /app

# Copy application code
COPY src/ ./src/
COPY models/edge/ ./models/

# Create data directory
RUN mkdir -p /app/data

# Expose port
EXPOSE 8000

# Health check
HEALTHCHECK --interval=60s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8000/api/v1/health || exit 1

# Start application
CMD ["python", "-m", "src.api.main"]
```

### ARM64 Support

```dockerfile
# Dockerfile.arm64
FROM arm64v8/python:3.10-slim

# Install ARM64-specific dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    libopencv-dev \
    libatlas-base-dev \
    && rm -rf /var/lib/apt/lists/*

# Install optimized libraries for ARM64
RUN pip install --no-cache-dir \
    torch==1.13.0+cpu \
    torchvision==0.14.0+cpu \
    -f https://download.pytorch.org/whl/torch_stable.html

# Continue with standard setup...
```

## Database Setup

### PostgreSQL with TimescaleDB

```sql
-- database/init/01-create-database.sql
CREATE DATABASE wifi_densepose;
CREATE USER wifi_user WITH PASSWORD 'secure_password';
GRANT ALL PRIVILEGES ON DATABASE wifi_densepose TO wifi_user;

-- Connect to the database
\c wifi_densepose;

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Create tables
CREATE TABLE pose_data (
    id BIGSERIAL PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL,
    frame_id BIGINT NOT NULL,
    person_id INTEGER,
    track_id INTEGER,
    confidence REAL NOT NULL,
    bounding_box JSONB NOT NULL,
    keypoints JSONB NOT NULL,
    dense_pose JSONB,
    metadata JSONB,
    environment_id VARCHAR(50) NOT NULL
);

-- Convert to hypertable
SELECT create_hypertable('pose_data', 'timestamp');

-- Create indexes
CREATE INDEX idx_pose_data_timestamp ON pose_data (timestamp DESC);
CREATE INDEX idx_pose_data_person_id ON pose_data (person_id, timestamp DESC);
CREATE INDEX idx_pose_data_environment ON pose_data (environment_id, timestamp DESC);
CREATE INDEX idx_pose_data_track_id ON pose_data (track_id, timestamp DESC);

-- Create retention policy (keep data for 30 days)
SELECT add_retention_policy('pose_data', INTERVAL '30 days');
```

### Database Migration

```python
# database/migrations/001_initial_schema.py
from alembic import op
import sqlalchemy as sa
from sqlalchemy.dialects import postgresql

def upgrade():
    """Create initial schema."""
    op.create_table(
        'pose_data',
        sa.Column('id', sa.BigInteger(), nullable=False),
        sa.Column('timestamp', sa.DateTime(timezone=True), nullable=False),
        sa.Column('frame_id', sa.BigInteger(), nullable=False),
        sa.Column('person_id', sa.Integer(), nullable=True),
        sa.Column('track_id', sa.Integer(), nullable=True),
        sa.Column('confidence', sa.Float(), nullable=False),
        sa.Column('bounding_box', postgresql.JSONB(), nullable=False),
        sa.Column('keypoints', postgresql.JSONB(), nullable=False),
        sa.Column('dense_pose', postgresql.JSONB(), nullable=True),
        sa.Column('metadata', postgresql.JSONB(), nullable=True),
        sa.Column('environment_id', sa.String(50), nullable=False),
        sa.PrimaryKeyConstraint('id')
    )
    
    # Create indexes
    op.create_index('idx_pose_data_timestamp', 'pose_data', ['timestamp'])
    op.create_index('idx_pose_data_person_id', 'pose_data', ['person_id', 'timestamp'])
    op.create_index('idx_pose_data_environment', 'pose_data', ['environment_id', 'timestamp'])

def downgrade():
    """Drop initial schema."""
    op.drop_table('pose_data')
```

## Monitoring and Logging

### Prometheus Configuration

```yaml
# monitoring/prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

rule_files:
  - "alert_rules.yml"

scrape_configs:
  - job_name: 'wifi-densepose-api'
    static_configs:
      - targets: ['wifi-densepose-api:8000']
    metrics_path: '/metrics'
    scrape_interval: 5s

  - job_name: 'neural-network'
    static_configs:
      - targets: ['neural-network:8080']
    metrics_path: '/metrics'

  - job_name: 'postgres'
    static_configs:
      - targets: ['postgres-exporter:9187']

  - job_name: 'redis'
    static_configs:
      - targets: ['redis-exporter:9121']

alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - alertmanager:9093
```

### Grafana Dashboards

```json
{
  "dashboard": {
    "title": "WiFi-DensePose System Metrics",
    "panels": [
      {
        "title": "API Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(http_requests_total[5m])",
            "legendFormat": "{{method}} {{endpoint}}"
          }
        ]
      },
      {
        "title": "Pose Detection Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(pose_detections_total[5m])",
            "legendFormat": "Detections per second"
          }
        ]
      },
      {
        "title": "Neural Network Inference Time",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(neural_network_inference_duration_seconds_bucket[5m]))",
            "legendFormat": "95th percentile"
          }
        ]
      }
    ]
  }
}
```

### ELK Stack Configuration

```yaml
# monitoring/elasticsearch.yml
version: '3.8'

services:
  elasticsearch:
    image: docker.elastic.co/elasticsearch/elasticsearch:8.5.0
    environment:
      - discovery.type=single-node
      - "ES_JAVA_OPTS=-Xms2g -Xmx2g"
    volumes:
      - elasticsearch_data:/usr/share/elasticsearch/data
    ports:
      - "9200:9200"

  logstash:
    image: docker.elastic.co/logstash/logstash:8.5.0
    volumes:
      - ./logstash/pipeline:/usr/share/logstash/pipeline
      - ./logstash/config:/usr/share/logstash/config
    ports:
      - "5044:5044"
    depends_on:
      - elasticsearch

  kibana:
    image: docker.elastic.co/kibana/kibana:8.5.0
    ports:
      - "5601:5601"
    environment:
      - ELASTICSEARCH_HOSTS=http://elasticsearch:9200
    depends_on:
      - elasticsearch

volumes:
  elasticsearch_data:
```

## Security Configuration

### SSL/TLS Setup

```bash
# Generate SSL certificates
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
    -keyout nginx/ssl/key.pem \
    -out nginx/ssl/cert.pem \
    -subj "/C=US/ST=State/L=City/O=Organization/CN=your-domain.com"

# Or use Let's Encrypt
certbot certonly --standalone -d your-domain.com
```

### Security Headers

```nginx
# nginx/security.conf
# Security headers
add_header X-Frame-Options DENY;
add_header X-Content-Type-Options nosniff;
add_header X-XSS-Protection "1; mode=block";
add_header Strict-Transport-Security "max-age=63072000; includeSubDomains; preload";
add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline';";
add_header Referrer-Policy "strict-origin-when-cross-origin";

# Hide server information
server_tokens off;
```

### Firewall Configuration

```bash
# UFW firewall rules
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow ssh
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw allow 5500/tcp  # CSI data port
sudo ufw enable
```

## Performance Optimization

### Application Optimization

```python
# src/config/performance.py
import asyncio
import uvloop

# Use uvloop for better async performance
asyncio.set_event_loop_policy(uvloop.EventLoopPolicy())

# Gunicorn configuration
bind = "0.0.0.0:8000"
workers = 4
worker_class = "uvicorn.workers.UvicornWorker"
worker_connections = 1000
max_requests = 1000
max_requests_jitter = 100
preload_app = True
keepalive = 5
```

### Database Optimization

```sql
-- Database performance tuning
-- postgresql.conf optimizations
shared_buffers = 256MB
effective_cache_size = 1GB
maintenance_work_mem = 64MB
checkpoint_completion_target = 0.9
wal_buffers = 16MB
default_statistics_target = 100
random_page_cost = 1.1
effective_io_concurrency = 200

-- Connection pooling
max_connections = 200
```

### Caching Strategy

```python
# src/cache/strategy.py
from redis import Redis
import json

class CacheManager:
    def __init__(self, redis_client: Redis):
        self.redis = redis_client
    
    async def cache_pose_data(self, key: str, data: dict, ttl: int = 300):
        """Cache pose data with TTL."""
        await self.redis.setex(
            key, 
            ttl, 
            json.dumps(data, default=str)
        )
    
    async def get_cached_poses(self, key: str):
        """Get cached pose data."""
        cached = await self.redis.get(key)
        return json.loads(cached) if cached else None
```

## Backup and Recovery

### Database Backup

```bash
#!/bin/bash
# scripts/backup-database.sh

BACKUP_DIR="/backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="wifi_densepose_backup_${TIMESTAMP}.sql"

# Create backup
pg_dump -h postgres -U wifi_user -d wifi_densepose > "${BACKUP_DIR}/${BACKUP_FILE}"

# Compress backup
gzip "${BACKUP_DIR}/${BACKUP_FILE}"

# Upload to S3 (optional)
aws s3 cp "${BACKUP_DIR}/${BACKUP_FILE}.gz" s3://your-backup-bucket/database/

# Clean up old backups (keep last 7 days)
find ${BACKUP_DIR} -name "wifi_densepose_backup_*.sql.gz" -mtime +7 -delete

echo "Backup completed: ${BACKUP_FILE}.gz"
```

### Disaster Recovery

```bash
#!/bin/bash
# scripts/restore-database.sh

BACKUP_FILE=$1

if [ -z "$BACKUP_FILE" ]; then
    echo "Usage: $0 <backup_file>"
    exit 1
fi

# Stop application
docker-compose stop wifi-densepose-api

# Restore database
gunzip -c "$BACKUP_FILE" | psql -h postgres -U wifi_user -d wifi_densepose

# Start application
docker-compose start wifi-densepose-api

echo "Database restored from: $BACKUP_FILE"
```

### Data Migration

```python
# scripts/migrate-data.py
import asyncio
import asyncpg
from datetime import datetime

async def migrate_pose_data(source_db_url: str, target_db_url: str):
    """Migrate pose data between databases."""
    
    source_conn = await asyncpg.connect(source_db_url)
    target_conn = await asyncpg.connect(target_db_url)
    
    try:
        # Get data in batches
        batch_size = 1000
        offset = 0
        
        while True:
            rows = await source_conn.fetch(
                "SELECT * FROM pose_data ORDER BY timestamp LIMIT $1 OFFSET $2",
                batch_size, offset
            )
            
            if not rows:
                break
            
            # Insert into target database
            await target_conn.executemany(
                """
                INSERT INTO pose_data 
                (timestamp, frame_id, person_id, track_id, confidence, 
                 bounding_box, keypoints, dense_pose, metadata, environment_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                """,
                rows
            )
            
            offset += batch_size
            print(f"Migrated {offset} records...")
    
    finally:
        await source_conn.close()
        await target_conn.close()

if __name__ == "__main__":
    source_url = "postgresql://user:pass@old-host:5432/wifi_densepose"
    target_url = "postgresql://user:pass@new-host:5432/wifi_densepose"
    
    asyncio.run(migrate_pose_data(source_url, target_url))
```

---

This deployment guide provides comprehensive instructions for deploying the WiFi-DensePose system across various environments and platforms. Choose the deployment method that best fits your infrastructure requirements and scale.

For additional support:
- [Architecture Overview](architecture-overview.md)
- [Contributing Guide](contributing.md)
- [Testing Guide](testing-guide.md)
- [Troubleshooting Guide](../user-guide/troubleshooting.md)