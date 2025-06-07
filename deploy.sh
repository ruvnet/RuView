#!/bin/bash

# WiFi-DensePose Deployment Script
# This script orchestrates the complete deployment of WiFi-DensePose infrastructure

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_NAME="wifi-densepose"
ENVIRONMENT="${ENVIRONMENT:-production}"
AWS_REGION="${AWS_REGION:-us-west-2}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-~/.kube/config}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    local missing_tools=()
    
    # Check required tools
    for tool in aws kubectl helm terraform docker; do
        if ! command -v "$tool" &> /dev/null; then
            missing_tools+=("$tool")
        fi
    done
    
    if [ ${#missing_tools[@]} -ne 0 ]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        log_info "Please install the missing tools and try again."
        exit 1
    fi
    
    # Check AWS credentials
    if ! aws sts get-caller-identity &> /dev/null; then
        log_error "AWS credentials not configured or invalid"
        log_info "Please configure AWS credentials using 'aws configure' or environment variables"
        exit 1
    fi
    
    # Check Docker daemon
    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running"
        log_info "Please start Docker daemon and try again"
        exit 1
    fi
    
    log_success "All prerequisites satisfied"
}

# Deploy infrastructure with Terraform
deploy_infrastructure() {
    log_info "Deploying infrastructure with Terraform..."
    
    cd "${SCRIPT_DIR}/terraform"
    
    # Initialize Terraform
    log_info "Initializing Terraform..."
    terraform init
    
    # Plan deployment
    log_info "Planning Terraform deployment..."
    terraform plan -var="environment=${ENVIRONMENT}" -var="aws_region=${AWS_REGION}" -out=tfplan
    
    # Apply deployment
    log_info "Applying Terraform deployment..."
    terraform apply tfplan
    
    # Update kubeconfig
    log_info "Updating kubeconfig..."
    aws eks update-kubeconfig --region "${AWS_REGION}" --name "${PROJECT_NAME}-cluster"
    
    log_success "Infrastructure deployed successfully"
    cd "${SCRIPT_DIR}"
}

# Deploy Kubernetes resources
deploy_kubernetes() {
    log_info "Deploying Kubernetes resources..."
    
    # Create namespaces
    log_info "Creating namespaces..."
    kubectl apply -f k8s/namespace.yaml
    
    # Deploy ConfigMaps and Secrets
    log_info "Deploying ConfigMaps and Secrets..."
    kubectl apply -f k8s/configmap.yaml
    kubectl apply -f k8s/secrets.yaml
    
    # Deploy application
    log_info "Deploying application..."
    kubectl apply -f k8s/deployment.yaml
    kubectl apply -f k8s/service.yaml
    kubectl apply -f k8s/ingress.yaml
    kubectl apply -f k8s/hpa.yaml
    
    # Wait for deployment to be ready
    log_info "Waiting for deployment to be ready..."
    kubectl wait --for=condition=available --timeout=300s deployment/wifi-densepose -n wifi-densepose
    
    log_success "Kubernetes resources deployed successfully"
}

# Deploy monitoring stack
deploy_monitoring() {
    log_info "Deploying monitoring stack..."
    
    # Add Helm repositories
    log_info "Adding Helm repositories..."
    helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
    helm repo add grafana https://grafana.github.io/helm-charts
    helm repo update
    
    # Create monitoring namespace
    kubectl create namespace monitoring --dry-run=client -o yaml | kubectl apply -f -
    
    # Deploy Prometheus
    log_info "Deploying Prometheus..."
    helm upgrade --install prometheus prometheus-community/kube-prometheus-stack \
        --namespace monitoring \
        --values monitoring/prometheus-values.yaml \
        --wait
    
    # Deploy Grafana dashboard
    log_info "Deploying Grafana dashboard..."
    kubectl create configmap grafana-dashboard \
        --from-file=monitoring/grafana-dashboard.json \
        --namespace monitoring \
        --dry-run=client -o yaml | kubectl apply -f -
    
    # Deploy Fluentd for logging
    log_info "Deploying Fluentd..."
    kubectl apply -f logging/fluentd-config.yml
    
    log_success "Monitoring stack deployed successfully"
}

# Build and push Docker images
build_and_push_images() {
    log_info "Building and pushing Docker images..."
    
    # Get ECR login token
    aws ecr get-login-password --region "${AWS_REGION}" | docker login --username AWS --password-stdin "$(aws sts get-caller-identity --query Account --output text).dkr.ecr.${AWS_REGION}.amazonaws.com"
    
    # Build application image
    log_info "Building application image..."
    docker build -t "${PROJECT_NAME}:latest" .
    
    # Tag and push to ECR
    local ecr_repo="$(aws sts get-caller-identity --query Account --output text).dkr.ecr.${AWS_REGION}.amazonaws.com/${PROJECT_NAME}"
    docker tag "${PROJECT_NAME}:latest" "${ecr_repo}:latest"
    docker tag "${PROJECT_NAME}:latest" "${ecr_repo}:$(git rev-parse --short HEAD)"
    
    log_info "Pushing images to ECR..."
    docker push "${ecr_repo}:latest"
    docker push "${ecr_repo}:$(git rev-parse --short HEAD)"
    
    log_success "Docker images built and pushed successfully"
}

# Run health checks
run_health_checks() {
    log_info "Running health checks..."
    
    # Check pod status
    log_info "Checking pod status..."
    kubectl get pods -n wifi-densepose
    
    # Check service endpoints
    log_info "Checking service endpoints..."
    kubectl get endpoints -n wifi-densepose
    
    # Check ingress
    log_info "Checking ingress..."
    kubectl get ingress -n wifi-densepose
    
    # Test application health endpoint
    local app_url=$(kubectl get ingress wifi-densepose-ingress -n wifi-densepose -o jsonpath='{.status.loadBalancer.ingress[0].hostname}')
    if [ -n "$app_url" ]; then
        log_info "Testing application health endpoint..."
        if curl -f "http://${app_url}/health" &> /dev/null; then
            log_success "Application health check passed"
        else
            log_warning "Application health check failed"
        fi
    else
        log_warning "Ingress URL not available yet"
    fi
    
    log_success "Health checks completed"
}

# Configure CI/CD
setup_cicd() {
    log_info "Setting up CI/CD pipelines..."
    
    # Create GitHub Actions secrets (if using GitHub)
    if [ -d ".git" ] && git remote get-url origin | grep -q "github.com"; then
        log_info "GitHub repository detected"
        log_info "Please configure the following secrets in your GitHub repository:"
        echo "  - AWS_ACCESS_KEY_ID"
        echo "  - AWS_SECRET_ACCESS_KEY"
        echo "  - KUBE_CONFIG_DATA"
        echo "  - ECR_REPOSITORY"
    fi
    
    # Validate CI/CD files
    if [ -f ".github/workflows/ci.yml" ]; then
        log_success "GitHub Actions CI workflow found"
    fi
    
    if [ -f ".github/workflows/cd.yml" ]; then
        log_success "GitHub Actions CD workflow found"
    fi
    
    if [ -f ".gitlab-ci.yml" ]; then
        log_success "GitLab CI configuration found"
    fi
    
    log_success "CI/CD setup completed"
}

# Cleanup function
cleanup() {
    log_info "Cleaning up temporary files..."
    rm -f terraform/tfplan
}

# Main deployment function
main() {
    log_info "Starting WiFi-DensePose deployment..."
    log_info "Environment: ${ENVIRONMENT}"
    log_info "AWS Region: ${AWS_REGION}"
    
    # Set trap for cleanup
    trap cleanup EXIT
    
    # Run deployment steps
    check_prerequisites
    
    case "${1:-all}" in
        "infrastructure")
            deploy_infrastructure
            ;;
        "kubernetes")
            deploy_kubernetes
            ;;
        "monitoring")
            deploy_monitoring
            ;;
        "images")
            build_and_push_images
            ;;
        "health")
            run_health_checks
            ;;
        "cicd")
            setup_cicd
            ;;
        "all")
            deploy_infrastructure
            build_and_push_images
            deploy_kubernetes
            deploy_monitoring
            setup_cicd
            run_health_checks
            ;;
        *)
            log_error "Unknown deployment target: $1"
            log_info "Usage: $0 [infrastructure|kubernetes|monitoring|images|health|cicd|all]"
            exit 1
            ;;
    esac
    
    log_success "WiFi-DensePose deployment completed successfully!"
    
    # Display useful information
    echo ""
    log_info "Useful commands:"
    echo "  kubectl get pods -n wifi-densepose"
    echo "  kubectl logs -f deployment/wifi-densepose -n wifi-densepose"
    echo "  kubectl port-forward svc/grafana 3000:80 -n monitoring"
    echo "  kubectl port-forward svc/prometheus-server 9090:80 -n monitoring"
    echo ""
    
    # Display access URLs
    local ingress_url=$(kubectl get ingress wifi-densepose-ingress -n wifi-densepose -o jsonpath='{.status.loadBalancer.ingress[0].hostname}' 2>/dev/null || echo "Not available yet")
    log_info "Application URL: http://${ingress_url}"
    
    local grafana_url=$(kubectl get ingress grafana -n monitoring -o jsonpath='{.status.loadBalancer.ingress[0].hostname}' 2>/dev/null || echo "Use port-forward")
    log_info "Grafana URL: http://${grafana_url}"
}

# Run main function with all arguments
main "$@"