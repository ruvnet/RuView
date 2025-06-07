# WiFi-DensePose DevOps & Deployment Guide

This guide provides comprehensive instructions for deploying and managing the WiFi-DensePose application infrastructure using modern DevOps practices.

## 🏗️ Architecture Overview

The WiFi-DensePose deployment architecture includes:

- **Container Orchestration**: Kubernetes with auto-scaling capabilities
- **Infrastructure as Code**: Terraform for AWS resource provisioning
- **CI/CD Pipelines**: GitHub Actions and GitLab CI support
- **Monitoring**: Prometheus, Grafana, and comprehensive alerting
- **Logging**: Centralized log aggregation with Fluentd and Elasticsearch
- **Security**: Automated security scanning and compliance checks

## 📋 Prerequisites

### Required Tools

Ensure the following tools are installed on your system:

```bash
# AWS CLI
curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
unzip awscliv2.zip
sudo ./aws/install

# kubectl
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl

# Helm
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash

# Terraform
wget -O- https://apt.releases.hashicorp.com/gpg | sudo gpg --dearmor -o /usr/share/keyrings/hashicorp-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/hashicorp-archive-keyring.gpg] https://apt.releases.hashicorp.com $(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/hashicorp.list
sudo apt update && sudo apt install terraform

# Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
```

### AWS Configuration

Configure AWS credentials with appropriate permissions:

```bash
aws configure
# Enter your AWS Access Key ID, Secret Access Key, and default region
```

Required AWS permissions:
- EC2 (VPC, Subnets, Security Groups, Load Balancers)
- EKS (Cluster management)
- ECR (Container registry)
- IAM (Roles and policies)
- S3 (State storage and log backup)
- CloudWatch (Monitoring and logging)

## 🚀 Quick Start

### 1. Clone and Setup

```bash
git clone <repository-url>
cd wifi-densepose
```

### 2. Configure Environment

```bash
# Set environment variables
export ENVIRONMENT=production
export AWS_REGION=us-west-2
export PROJECT_NAME=wifi-densepose
```

### 3. Deploy Everything

```bash
# Deploy complete infrastructure and application
./deploy.sh all
```

### 4. Verify Deployment

```bash
# Check application status
kubectl get pods -n wifi-densepose

# Access Grafana dashboard
kubectl port-forward svc/grafana 3000:80 -n monitoring
# Open http://localhost:3000 (admin/admin)

# Access application
kubectl get ingress -n wifi-densepose
```

## 📁 Directory Structure

```
├── deploy.sh                          # Main deployment script
├── Dockerfile                         # Application container image
├── docker-compose.yml                 # Local development setup
├── docker-compose.prod.yml           # Production deployment
├── .dockerignore                      # Docker build context optimization
├── .github/workflows/                 # GitHub Actions CI/CD
│   ├── ci.yml                        # Continuous Integration
│   ├── cd.yml                        # Continuous Deployment
│   └── security-scan.yml            # Security scanning
├── .gitlab-ci.yml                    # GitLab CI configuration
├── k8s/                              # Kubernetes manifests
│   ├── namespace.yaml                # Namespace definition
│   ├── deployment.yaml               # Application deployment
│   ├── service.yaml                  # Service configuration
│   ├── ingress.yaml                  # Ingress rules
│   ├── configmap.yaml               # Configuration management
│   ├── secrets.yaml                 # Secret management template
│   └── hpa.yaml                     # Horizontal Pod Autoscaler
├── terraform/                        # Infrastructure as Code
│   ├── main.tf                      # Main infrastructure definition
│   ├── variables.tf                 # Configuration variables
│   └── outputs.tf                   # Output values
├── ansible/                          # Server configuration
│   └── playbook.yml                 # Ansible playbook
├── monitoring/                       # Monitoring configuration
│   ├── prometheus-config.yml        # Prometheus configuration
│   ├── grafana-dashboard.json       # Grafana dashboard
│   └── alerting-rules.yml          # Alert rules
└── logging/                          # Logging configuration
    └── fluentd-config.yml           # Fluentd configuration
```

## 🔧 Deployment Options

### Individual Component Deployment

```bash
# Deploy only infrastructure
./deploy.sh infrastructure

# Deploy only Kubernetes resources
./deploy.sh kubernetes

# Deploy only monitoring stack
./deploy.sh monitoring

# Build and push Docker images
./deploy.sh images

# Run health checks
./deploy.sh health

# Setup CI/CD
./deploy.sh cicd
```

### Environment-Specific Deployment

```bash
# Development environment
ENVIRONMENT=development ./deploy.sh all

# Staging environment
ENVIRONMENT=staging ./deploy.sh all

# Production environment
ENVIRONMENT=production ./deploy.sh all
```

## 🐳 Docker Configuration

### Local Development

```bash
# Start local development environment
docker-compose up -d

# View logs
docker-compose logs -f

# Stop environment
docker-compose down
```

### Production Build

```bash
# Build production image
docker build -f Dockerfile -t wifi-densepose:latest .

# Multi-stage build for optimization
docker build --target production -t wifi-densepose:prod .
```

## ☸️ Kubernetes Management

### Common Operations

```bash
# View application logs
kubectl logs -f deployment/wifi-densepose -n wifi-densepose

# Scale application
kubectl scale deployment wifi-densepose --replicas=5 -n wifi-densepose

# Update application
kubectl set image deployment/wifi-densepose wifi-densepose=new-image:tag -n wifi-densepose

# Rollback deployment
kubectl rollout undo deployment/wifi-densepose -n wifi-densepose

# View resource usage
kubectl top pods -n wifi-densepose
kubectl top nodes
```

### Configuration Management

```bash
# Update ConfigMap
kubectl create configmap wifi-densepose-config \
  --from-file=config/ \
  --dry-run=client -o yaml | kubectl apply -f -

# Update Secrets
kubectl create secret generic wifi-densepose-secrets \
  --from-literal=database-password=secret \
  --dry-run=client -o yaml | kubectl apply -f -
```

## 📊 Monitoring & Observability

### Prometheus Metrics

Access Prometheus at: `http://localhost:9090` (via port-forward)

Key metrics to monitor:
- `http_requests_total` - HTTP request count
- `http_request_duration_seconds` - Request latency
- `wifi_densepose_data_processed_total` - Data processing metrics
- `wifi_densepose_model_inference_duration_seconds` - ML model performance

### Grafana Dashboards

Access Grafana at: `http://localhost:3000` (admin/admin)

Pre-configured dashboards:
- Application Overview
- Infrastructure Metrics
- Database Performance
- Kubernetes Cluster Status
- Security Alerts

### Log Analysis

```bash
# View application logs
kubectl logs -f -l app=wifi-densepose -n wifi-densepose

# Search logs in Elasticsearch
curl -X GET "elasticsearch:9200/wifi-densepose-*/_search" \
  -H 'Content-Type: application/json' \
  -d '{"query": {"match": {"level": "error"}}}'
```

## 🔒 Security Best Practices

### Implemented Security Measures

1. **Container Security**
   - Non-root user execution
   - Minimal base images
   - Regular vulnerability scanning
   - Resource limits and quotas

2. **Kubernetes Security**
   - Network policies
   - Pod security policies
   - RBAC configuration
   - Secret management

3. **Infrastructure Security**
   - VPC with private subnets
   - Security groups with minimal access
   - IAM roles with least privilege
   - Encrypted storage and transit

4. **CI/CD Security**
   - Automated security scanning
   - Dependency vulnerability checks
   - Container image scanning
   - Secret scanning

### Security Scanning

```bash
# Run security scan
docker run --rm -v /var/run/docker.sock:/var/run/docker.sock \
  aquasec/trivy image wifi-densepose:latest

# Kubernetes security scan
kubectl run --rm -i --tty kube-bench --image=aquasec/kube-bench:latest \
  --restart=Never -- --version 1.20
```

## 🔄 CI/CD Pipelines

### GitHub Actions

Workflows are triggered on:
- **CI Pipeline** (`ci.yml`): Pull requests and pushes to main
- **CD Pipeline** (`cd.yml`): Tags and main branch pushes
- **Security Scan** (`security-scan.yml`): Daily scheduled runs

### GitLab CI

Configure GitLab CI variables:
- `AWS_ACCESS_KEY_ID`
- `AWS_SECRET_ACCESS_KEY`
- `KUBE_CONFIG`
- `ECR_REPOSITORY`

## 🏗️ Infrastructure as Code

### Terraform Configuration

```bash
# Initialize Terraform
cd terraform
terraform init

# Plan deployment
terraform plan -var="environment=production"

# Apply changes
terraform apply

# Destroy infrastructure
terraform destroy
```

### Ansible Configuration

```bash
# Run Ansible playbook
ansible-playbook -i inventory ansible/playbook.yml
```

## 🚨 Troubleshooting

### Common Issues

1. **Pod Startup Issues**
   ```bash
   kubectl describe pod <pod-name> -n wifi-densepose
   kubectl logs <pod-name> -n wifi-densepose
   ```

2. **Service Discovery Issues**
   ```bash
   kubectl get endpoints -n wifi-densepose
   kubectl get services -n wifi-densepose
   ```

3. **Ingress Issues**
   ```bash
   kubectl describe ingress wifi-densepose-ingress -n wifi-densepose
   kubectl get events -n wifi-densepose
   ```

4. **Resource Issues**
   ```bash
   kubectl top pods -n wifi-densepose
   kubectl describe nodes
   ```

### Health Checks

```bash
# Application health
curl http://<ingress-url>/health

# Database connectivity
kubectl exec -it <pod-name> -n wifi-densepose -- pg_isready

# Redis connectivity
kubectl exec -it <pod-name> -n wifi-densepose -- redis-cli ping
```

## 📈 Scaling & Performance

### Horizontal Pod Autoscaler

```bash
# View HPA status
kubectl get hpa -n wifi-densepose

# Update HPA configuration
kubectl patch hpa wifi-densepose-hpa -n wifi-densepose -p '{"spec":{"maxReplicas":10}}'
```

### Cluster Autoscaler

```bash
# View cluster autoscaler logs
kubectl logs -f deployment/cluster-autoscaler -n kube-system
```

### Performance Tuning

1. **Resource Requests/Limits**
   - CPU: Request 100m, Limit 500m
   - Memory: Request 256Mi, Limit 512Mi

2. **Database Optimization**
   - Connection pooling
   - Query optimization
   - Index management

3. **Caching Strategy**
   - Redis for session storage
   - Application-level caching
   - CDN for static assets

## 🔄 Backup & Recovery

### Database Backup

```bash
# Create database backup
kubectl exec -it postgres-pod -n wifi-densepose -- \
  pg_dump -U postgres wifi_densepose > backup.sql

# Restore database
kubectl exec -i postgres-pod -n wifi-densepose -- \
  psql -U postgres wifi_densepose < backup.sql
```

### Configuration Backup

```bash
# Backup Kubernetes resources
kubectl get all -n wifi-densepose -o yaml > k8s-backup.yaml

# Backup ConfigMaps and Secrets
kubectl get configmaps,secrets -n wifi-densepose -o yaml > config-backup.yaml
```

## 📞 Support & Maintenance

### Regular Maintenance Tasks

1. **Weekly**
   - Review monitoring alerts
   - Check resource utilization
   - Update dependencies

2. **Monthly**
   - Security patch updates
   - Performance optimization
   - Backup verification

3. **Quarterly**
   - Disaster recovery testing
   - Security audit
   - Capacity planning

### Contact Information

- **DevOps Team**: devops@wifi-densepose.com
- **On-Call**: +1-555-0123
- **Documentation**: https://docs.wifi-densepose.com
- **Status Page**: https://status.wifi-densepose.com

## 📚 Additional Resources

- [Kubernetes Documentation](https://kubernetes.io/docs/)
- [Terraform AWS Provider](https://registry.terraform.io/providers/hashicorp/aws/latest/docs)
- [Prometheus Monitoring](https://prometheus.io/docs/)
- [Grafana Dashboards](https://grafana.com/docs/)
- [AWS EKS Best Practices](https://aws.github.io/aws-eks-best-practices/)