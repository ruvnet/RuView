# WiFi-DensePose Deployment Guide

## Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Docker Deployment](#docker-deployment)
4. [Kubernetes Deployment](#kubernetes-deployment)
5. [Cloud Deployment](#cloud-deployment)
6. [Production Configuration](#production-configuration)
7. [Scaling and Load Balancing](#scaling-and-load-balancing)
8. [Monitoring and Observability](#monitoring-and-observability)
9. [Security Considerations](#security-considerations)
10. [Backup and Recovery](#backup-and-recovery)
11. [Troubleshooting](#troubleshooting)

## Overview

This guide covers deploying WiFi-DensePose in production environments, from single-node Docker deployments to large-scale Kubernetes clusters. The system is designed for high availability, scalability, and security.

### Deployment Options

- **Docker Compose**: Single-node development and small production deployments
- **Kubernetes**: Multi-node production deployments with auto-scaling
- **Cloud Platforms**: AWS, GCP, Azure with managed services
- **Edge Deployment**: IoT gateways and edge computing devices

### Architecture Components

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Load Balancer │    │   WiFi Routers  │    │   Monitoring    │
│    (Nginx)      │    │   (CSI Source)  │    │  (Prometheus)   │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│  WiFi-DensePose │    │    Database     │    │     Redis       │
│   API Servers   │◄──►│  (PostgreSQL)   │    │    (Cache)      │
│   (3+ replicas) │    │  (TimescaleDB)  │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## Prerequisites

### System Requirements

#### Minimum Requirements
- **CPU**: 4 cores, 2.4GHz
- **Memory**: 8GB RAM
- **Storage**: 100GB SSD
- **Network**: 1Gbps Ethernet

#### Recommended Requirements
- **CPU**: 8+ cores, 3.0GHz
- **Memory**: 16GB+ RAM
- **Storage**: 500GB+ NVMe SSD
- **Network**: 10Gbps Ethernet
- **GPU**: NVIDIA GPU with 8GB+ VRAM (optional)

### Software Dependencies

#### Container Runtime
```bash
# Docker (20.10+)
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

# Docker Compose (2.0+)
sudo curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
sudo chmod +x /usr/local/bin/docker-compose
```

#### Kubernetes (for K8s deployment)
```bash
# kubectl
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl

# Helm (3.0+)
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
```

## Docker Deployment

### Single-Node Docker Compose

#### 1. Download Configuration

```bash
# Clone repository
git clone https://github.com/ruvnet/wifi-densepose.git
cd wifi-densepose

# Copy environment template
cp .env.example .env
```

#### 2. Configure Environment

Edit `.env` file:

```bash
# Application Settings
APP_NAME=WiFi-DensePose API
VERSION=1.0.0
ENVIRONMENT=production
DEBUG=false

# Server Settings
HOST=0.0.0.0
PORT=8000
WORKERS=4

# Security (CHANGE THESE!)
SECRET_KEY=your-super-secret-key-change-this
JWT_SECRET=your-jwt-secret-change-this

# Database
DATABASE_URL=postgresql://postgres:password@postgres:5432/wifi_densepose
REDIS_URL=redis://:password@redis:6379/0

# Hardware
WIFI_INTERFACE=wlan0
CSI_BUFFER_SIZE=1000
HARDWARE_POLLING_INTERVAL=0.1

# Features
ENABLE_AUTHENTICATION=true
ENABLE_RATE_LIMITING=true
ENABLE_WEBSOCKETS=true
```

#### 3. Deploy with Docker Compose

```bash
# Start all services
docker-compose up -d

# Check service status
docker-compose ps

# View logs
docker-compose logs -f wifi-densepose

# Scale API servers
docker-compose up -d --scale wifi-densepose=3
```

#### 4. Verify Deployment

```bash
# Health check
curl http://localhost:8000/api/v1/health

# API documentation
open http://localhost:8000/docs
```

### Production Docker Compose

Create `docker-compose.prod.yml`:

```yaml
version: '3.8'

services:
  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx/nginx.conf:/etc/nginx/nginx.conf:ro
      - ./nginx/ssl:/etc/nginx/ssl:ro
    depends_on:
      - wifi-densepose
    restart: unless-stopped

  wifi-densepose:
    image: wifi-densepose:latest
    build:
      context: .
      target: production
    environment:
      - ENVIRONMENT=production
      - WORKERS=4
    env_file:
      - .env
    volumes:
      - ./data:/app/data
      - ./logs:/app/logs
      - ./models:/app/models
    depends_on:
      - postgres
      - redis
    restart: unless-stopped
    deploy:
      replicas: 3
      resources:
        limits:
          cpus: '2.0'
          memory: 4G
        reservations:
          cpus: '0.5'
          memory: 1G

  postgres:
    image: postgres:15-alpine
    environment:
      POSTGRES_DB: wifi_densepose
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./init.sql:/docker-entrypoint-initdb.d/init.sql:ro
    restart: unless-stopped
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 2G

  redis:
    image: redis:7-alpine
    command: redis-server --appendonly yes --requirepass ${REDIS_PASSWORD}
    volumes:
      - redis_data:/data
    restart: unless-stopped

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus_data:/prometheus
    restart: unless-stopped

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      GF_SECURITY_ADMIN_PASSWORD: ${GRAFANA_PASSWORD}
    volumes:
      - grafana_data:/var/lib/grafana
      - ./monitoring/grafana:/etc/grafana/provisioning:ro
    restart: unless-stopped

volumes:
  postgres_data:
  redis_data:
  prometheus_data:
  grafana_data:
```

## Kubernetes Deployment

### 1. Prepare Kubernetes Cluster

#### Create Namespace

```bash
kubectl create namespace wifi-densepose
kubectl config set-context --current --namespace=wifi-densepose
```

#### Install Required Operators

```bash
# Prometheus Operator
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm install prometheus prometheus-community/kube-prometheus-stack \
  --namespace monitoring --create-namespace

# Ingress Controller
helm repo add ingress-nginx https://kubernetes.github.io/ingress-nginx
helm install ingress-nginx ingress-nginx/ingress-nginx \
  --namespace ingress-nginx --create-namespace
```

### 2. Configure Secrets and ConfigMaps

#### Create Secrets

```bash
# Database secrets
kubectl create secret generic postgres-secret \
  --from-literal=POSTGRES_DB=wifi_densepose \
  --from-literal=POSTGRES_USER=postgres \
  --from-literal=POSTGRES_PASSWORD=your-secure-password

# Redis secrets
kubectl create secret generic redis-secret \
  --from-literal=REDIS_PASSWORD=your-redis-password

# Application secrets
kubectl create secret generic wifi-densepose-secrets \
  --from-literal=SECRET_KEY=your-super-secret-key \
  --from-literal=JWT_SECRET=your-jwt-secret \
  --from-literal=DATABASE_URL=postgresql://postgres:password@postgres:5432/wifi_densepose \
  --from-literal=REDIS_URL=redis://:password@redis:6379/0

# TLS certificates
kubectl create secret tls tls-secret \
  --cert=path/to/tls.crt \
  --key=path/to/tls.key
```

#### Create ConfigMaps

```bash
# Application configuration
kubectl create configmap wifi-densepose-config \
  --from-literal=ENVIRONMENT=production \
  --from-literal=LOG_LEVEL=INFO \
  --from-literal=WORKERS=4 \
  --from-literal=ENABLE_AUTHENTICATION=true \
  --from-literal=ENABLE_RATE_LIMITING=true

# Nginx configuration
kubectl create configmap nginx-config \
  --from-file=nginx.conf=./k8s/nginx.conf

# PostgreSQL initialization
kubectl create configmap postgres-init \
  --from-file=init.sql=./k8s/init.sql
```

### 3. Deploy Persistent Volumes

```yaml
# k8s/pvc.yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: wifi-densepose-data-pvc
  namespace: wifi-densepose
spec:
  accessModes:
    - ReadWriteOnce
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
    - ReadWriteOnce
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

---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: redis-data-pvc
  namespace: wifi-densepose
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 20Gi
  storageClassName: fast-ssd
```

### 4. Deploy Application

```bash
# Apply all Kubernetes manifests
kubectl apply -f k8s/pvc.yaml
kubectl apply -f k8s/deployment.yaml
kubectl apply -f k8s/service.yaml
kubectl apply -f k8s/ingress.yaml

# Check deployment status
kubectl get pods -w
kubectl get services
kubectl get ingress
```

### 5. Configure Ingress

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
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
    nginx.ingress.kubernetes.io/proxy-body-size: "50m"
    nginx.ingress.kubernetes.io/rate-limit: "100"
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
            name: wifi-densepose-service
            port:
              number: 80
```

## Cloud Deployment

### AWS Deployment

#### 1. EKS Cluster Setup

```bash
# Install eksctl
curl --silent --location "https://github.com/weaveworks/eksctl/releases/latest/download/eksctl_$(uname -s)_amd64.tar.gz" | tar xz -C /tmp
sudo mv /tmp/eksctl /usr/local/bin

# Create EKS cluster
eksctl create cluster \
  --name wifi-densepose \
  --region us-west-2 \
  --nodegroup-name workers \
  --node-type m5.xlarge \
  --nodes 3 \
  --nodes-min 1 \
  --nodes-max 10 \
  --managed
```

#### 2. RDS Database

```bash
# Create RDS PostgreSQL instance
aws rds create-db-instance \
  --db-instance-identifier wifi-densepose-db \
  --db-instance-class db.r5.large \
  --engine postgres \
  --engine-version 15.4 \
  --allocated-storage 100 \
  --storage-type gp2 \
  --storage-encrypted \
  --master-username postgres \
  --master-user-password your-secure-password \
  --vpc-security-group-ids sg-xxxxxxxxx \
  --db-subnet-group-name default \
  --backup-retention-period 7 \
  --multi-az
```

#### 3. ElastiCache Redis

```bash
# Create ElastiCache Redis cluster
aws elasticache create-cache-cluster \
  --cache-cluster-id wifi-densepose-redis \
  --cache-node-type cache.r5.large \
  --engine redis \
  --num-cache-nodes 1 \
  --security-group-ids sg-xxxxxxxxx \
  --subnet-group-name default
```

### GCP Deployment

#### 1. GKE Cluster Setup

```bash
# Create GKE cluster
gcloud container clusters create wifi-densepose \
  --zone us-central1-a \
  --machine-type n1-standard-4 \
  --num-nodes 3 \
  --enable-autoscaling \
  --min-nodes 1 \
  --max-nodes 10 \
  --enable-autorepair \
  --enable-autoupgrade
```

#### 2. Cloud SQL

```bash
# Create Cloud SQL PostgreSQL instance
gcloud sql instances create wifi-densepose-db \
  --database-version POSTGRES_15 \
  --tier db-n1-standard-2 \
  --region us-central1 \
  --storage-size 100GB \
  --storage-type SSD \
  --backup-start-time 02:00
```

### Azure Deployment

#### 1. AKS Cluster Setup

```bash
# Create resource group
az group create --name wifi-densepose-rg --location eastus

# Create AKS cluster
az aks create \
  --resource-group wifi-densepose-rg \
  --name wifi-densepose-aks \
  --node-count 3 \
  --node-vm-size Standard_D4s_v3 \
  --enable-addons monitoring \
  --generate-ssh-keys
```

#### 2. Azure Database for PostgreSQL

```bash
# Create PostgreSQL server
az postgres server create \
  --resource-group wifi-densepose-rg \
  --name wifi-densepose-db \
  --location eastus \
  --admin-user postgres \
  --admin-password your-secure-password \
  --sku-name GP_Gen5_2 \
  --storage-size 102400
```

## Production Configuration

### Environment Variables

```bash
# Production environment file
cat > .env.prod << EOF
# Application
APP_NAME=WiFi-DensePose API
VERSION=1.0.0
ENVIRONMENT=production
DEBUG=false

# Server
HOST=0.0.0.0
PORT=8000
WORKERS=4

# Security
SECRET_KEY=${SECRET_KEY}
JWT_SECRET=${JWT_SECRET}
JWT_ALGORITHM=HS256
JWT_EXPIRE_HOURS=24

# Database
DATABASE_URL=${DATABASE_URL}
DATABASE_POOL_SIZE=20
DATABASE_MAX_OVERFLOW=30
DATABASE_POOL_TIMEOUT=30

# Redis
REDIS_URL=${REDIS_URL}
REDIS_POOL_SIZE=10

# Hardware
WIFI_INTERFACE=wlan0
CSI_BUFFER_SIZE=2000
HARDWARE_POLLING_INTERVAL=0.05

# Pose Processing
POSE_CONFIDENCE_THRESHOLD=0.7
POSE_PROCESSING_BATCH_SIZE=64
POSE_MAX_PERSONS=20

# Features
ENABLE_AUTHENTICATION=true
ENABLE_RATE_LIMITING=true
ENABLE_WEBSOCKETS=true
ENABLE_REAL_TIME_PROCESSING=true

# Monitoring
ENABLE_METRICS=true
METRICS_PORT=8080
LOG_LEVEL=INFO

# Performance
ENABLE_GPU=true
MIXED_PRECISION=true
OPTIMIZE_FOR_INFERENCE=true
EOF
```

### Database Configuration

#### PostgreSQL Optimization

```sql
-- postgresql.conf optimizations
shared_buffers = 256MB
effective_cache_size = 1GB
maintenance_work_mem = 64MB
checkpoint_completion_target = 0.9
wal_buffers = 16MB
default_statistics_target = 100
random_page_cost = 1.1
effective_io_concurrency = 200
work_mem = 4MB
min_wal_size = 1GB
max_wal_size = 4GB

-- TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Create hypertables for time-series data
SELECT create_hypertable('csi_data', 'timestamp');
SELECT create_hypertable('pose_detections', 'timestamp');
SELECT create_hypertable('system_metrics', 'timestamp');

-- Create indexes
CREATE INDEX idx_pose_detections_person_id ON pose_detections (person_id);
CREATE INDEX idx_pose_detections_zone_id ON pose_detections (zone_id);
CREATE INDEX idx_csi_data_router_id ON csi_data (router_id);
```

### Redis Configuration

```bash
# redis.conf optimizations
maxmemory 2gb
maxmemory-policy allkeys-lru
save 900 1
save 300 10
save 60 10000
appendonly yes
appendfsync everysec
```

## Scaling and Load Balancing

### Horizontal Pod Autoscaler

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: wifi-densepose-hpa
  namespace: wifi-densepose
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: wifi-densepose
  minReplicas: 3
  maxReplicas: 20
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
  behavior:
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Percent
        value: 10
        periodSeconds: 60
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
      - type: Percent
        value: 50
        periodSeconds: 60
```

### Vertical Pod Autoscaler

```yaml
apiVersion: autoscaling.k8s.io/v1
kind: VerticalPodAutoscaler
metadata:
  name: wifi-densepose-vpa
  namespace: wifi-densepose
spec:
  targetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: wifi-densepose
  updatePolicy:
    updateMode: "Auto"
  resourcePolicy:
    containerPolicies:
    - containerName: wifi-densepose
      maxAllowed:
        cpu: 4
        memory: 8Gi
      minAllowed:
        cpu: 500m
        memory: 1Gi
```

### Load Balancer Configuration

#### Nginx Configuration

```nginx
upstream wifi_densepose_backend {
    least_conn;
    server wifi-densepose-1:8000 max_fails=3 fail_timeout=30s;
    server wifi-densepose-2:8000 max_fails=3 fail_timeout=30s;
    server wifi-densepose-3:8000 max_fails=3 fail_timeout=30s;
}

server {
    listen 80;
    listen 443 ssl http2;
    server_name api.wifi-densepose.com;

    # SSL configuration
    ssl_certificate /etc/nginx/ssl/tls.crt;
    ssl_certificate_key /etc/nginx/ssl/tls.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512;

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;
    limit_req zone=api burst=20 nodelay;

    # Gzip compression
    gzip on;
    gzip_types text/plain application/json application/javascript text/css;

    location / {
        proxy_pass http://wifi_densepose_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # WebSocket support
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        
        # Timeouts
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
    }

    location /health {
        access_log off;
        proxy_pass http://wifi_densepose_backend;
    }
}
```

## Monitoring and Observability

### Prometheus Configuration

```yaml
# monitoring/prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

rule_files:
  - "wifi_densepose_rules.yml"

scrape_configs:
  - job_name: 'wifi-densepose'
    static_configs:
      - targets: ['wifi-densepose:8080']
    metrics_path: /metrics
    scrape_interval: 10s

  - job_name: 'postgres'
    static_configs:
      - targets: ['postgres-exporter:9187']

  - job_name: 'redis'
    static_configs:
      - targets: ['redis-exporter:9121']

  - job_name: 'nginx'
    static_configs:
      - targets: ['nginx-exporter:9113']
```

### Grafana Dashboards

```json
{
  "dashboard": {
    "title": "WiFi-DensePose Monitoring",
    "panels": [
      {
        "title": "Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(http_requests_total[5m])",
            "legendFormat": "{{method}} {{status}}"
          }
        ]
      },
      {
        "title": "Response Time",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))",
            "legendFormat": "95th percentile"
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
      }
    ]
  }
}
```

### Alerting Rules

```yaml
# monitoring/wifi_densepose_rules.yml
groups:
  - name: wifi-densepose
    rules:
      - alert: HighErrorRate
        expr: rate(http_requests_total{status=~"5.."}[5m]) > 0.1
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: High error rate detected
          description: "Error rate is {{ $value }} errors per second"

      - alert: HighResponseTime
        expr: histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m])) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: High response time detected
          description: "95th percentile response time is {{ $value }} seconds"

      - alert: PoseDetectionDown
        expr: rate(pose_detections_total[5m]) == 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: Pose detection stopped
          description: "No pose detections in the last 2 minutes"
```

## Security Considerations

### Network Security

```yaml
# Network policies
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: wifi-densepose-netpol
  namespace: wifi-densepose
spec:
  podSelector:
    matchLabels:
      app: wifi-densepose
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
      port: 8000
  egress:
  - to:
    - podSelector:
        matchLabels:
          component: postgres
    ports:
    - protocol: TCP
      port: 5432
  - to:
    - podSelector:
        matchLabels:
          component: redis
    ports:
    - protocol: TCP
      port: 6379
```

### Pod Security Standards

```yaml
apiVersion: v1
kind: Pod
spec:
  securityContext:
    runAsNonRoot: true
    runAsUser: 1000
    runAsGroup: 1000
    fsGroup: 1000
    seccompProfile:
      type: RuntimeDefault
  containers:
  - name: wifi-densepose
    securityContext:
      allowPrivilegeEscalation: false
      readOnlyRootFilesystem: true
      capabilities:
        drop:
        - ALL
```

### Secrets Management

```bash
# Using external secrets operator
helm repo add external-secrets https://charts.external-secrets.io
helm install external-secrets external-secrets/external-secrets \
  --namespace external-secrets-system \
  --create-namespace

# AWS Secrets Manager integration
kubectl apply -f - <<EOF
apiVersion: external-secrets.io/v1beta1
kind: SecretStore
metadata:
  name: aws-secrets-manager
  namespace: wifi-densepose
spec:
  provider:
    aws:
      service: SecretsManager
      region: us-west-2
      auth:
        jwt:
          serviceAccountRef:
            name: external-secrets-sa
EOF
```

## Backup and Recovery

### Database Backup

```bash
#!/bin/bash
# backup-database.sh

BACKUP_DIR="/backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="wifi_densepose_backup_${TIMESTAMP}.sql"

# Create backup
pg_dump -h postgres -U postgres -d wifi_densepose > "${BACKUP_DIR}/${BACKUP_FILE}"

# Compress backup
gzip "${BACKUP_DIR}/${BACKUP_FILE}"

# Upload to S3
aws s3 cp "${BACKUP_DIR}/${BACKUP_FILE}.gz" s3://wifi-densepose-backups/

# Clean old backups (keep last 30 days)
find ${BACKUP_DIR} -name "*.gz" -mtime +30 -delete
```

### Disaster Recovery

```yaml
# Velero backup configuration
apiVersion: velero.io/v1
kind: Schedule
metadata:
  name: wifi-densepose-backup
  namespace: velero
spec:
  schedule: "0 2 * * *"
  template:
    includedNamespaces:
    - wifi-densepose
    storageLocation: default
    volumeSnapshotLocations:
    - default
    ttl: 720h0m0s
```

## Troubleshooting

### Common Issues

#### 1. Pod Startup Issues

```bash
# Check pod status
kubectl get pods -n wifi-densepose

# Check pod logs
kubectl logs -f deployment/wifi-densepose -n wifi-densepose

# Describe pod for events
kubectl describe pod <pod-name> -n wifi-densepose
```

#### 2. Database Connection Issues

```bash
# Test database connectivity
kubectl run -it --rm debug --image=postgres:15-alpine --restart=Never -- \
  psql -h postgres -U postgres -d wifi_densepose

# Check database logs
kubectl logs -f deployment/postgres -n wifi-densepose
```

#### 3. Performance Issues

```bash
# Check resource usage
kubectl top pods -n wifi-densepose
kubectl top nodes

# Check HPA status
kubectl get hpa -n wifi-densepose

# Check metrics
curl http://localhost:8080/metrics
```

### Debug Commands

```bash
# Port forward for local debugging
kubectl port-forward service/wifi-densepose-service 8000:80 -n wifi-densepose

# Execute commands in pod
kubectl exec -it deployment/wifi-densepose -n wifi-densepose -- /bin/bash

# Check service endpoints
kubectl get endpoints -n wifi-densepose

# View ingress status
kubectl describe ingress wifi-densepose-ingress -n wifi-densepose
```

---

For more information, see:
- [User Guide](user_guide.md)
- [API Reference](api_reference.md)
- [Troubleshooting Guide](troubleshooting.md)