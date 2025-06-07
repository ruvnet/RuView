#!/bin/bash

# WiFi-DensePose Deployment Validation Script
# This script validates that all deployment components are functioning correctly

set -euo pipefail

# Configuration
NAMESPACE="wifi-densepose"
MONITORING_NAMESPACE="monitoring"
TIMEOUT=300

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

# Check if kubectl is available and configured
check_kubectl() {
    log_info "Checking kubectl configuration..."
    
    if ! command -v kubectl &> /dev/null; then
        log_error "kubectl is not installed or not in PATH"
        return 1
    fi
    
    if ! kubectl cluster-info &> /dev/null; then
        log_error "kubectl is not configured or cluster is not accessible"
        return 1
    fi
    
    log_success "kubectl is configured and cluster is accessible"
    return 0
}

# Validate namespace exists
validate_namespace() {
    local ns=$1
    log_info "Validating namespace: $ns"
    
    if kubectl get namespace "$ns" &> /dev/null; then
        log_success "Namespace $ns exists"
        return 0
    else
        log_error "Namespace $ns does not exist"
        return 1
    fi
}

# Validate deployments are ready
validate_deployments() {
    log_info "Validating deployments in namespace: $NAMESPACE"
    
    local deployments
    deployments=$(kubectl get deployments -n "$NAMESPACE" -o jsonpath='{.items[*].metadata.name}')
    
    if [ -z "$deployments" ]; then
        log_warning "No deployments found in namespace $NAMESPACE"
        return 1
    fi
    
    local failed=0
    for deployment in $deployments; do
        log_info "Checking deployment: $deployment"
        
        if kubectl wait --for=condition=available --timeout="${TIMEOUT}s" "deployment/$deployment" -n "$NAMESPACE" &> /dev/null; then
            local ready_replicas
            ready_replicas=$(kubectl get deployment "$deployment" -n "$NAMESPACE" -o jsonpath='{.status.readyReplicas}')
            local desired_replicas
            desired_replicas=$(kubectl get deployment "$deployment" -n "$NAMESPACE" -o jsonpath='{.spec.replicas}')
            
            if [ "$ready_replicas" = "$desired_replicas" ]; then
                log_success "Deployment $deployment is ready ($ready_replicas/$desired_replicas replicas)"
            else
                log_warning "Deployment $deployment has $ready_replicas/$desired_replicas replicas ready"
                failed=1
            fi
        else
            log_error "Deployment $deployment is not ready within ${TIMEOUT}s"
            failed=1
        fi
    done
    
    return $failed
}

# Validate services are accessible
validate_services() {
    log_info "Validating services in namespace: $NAMESPACE"
    
    local services
    services=$(kubectl get services -n "$NAMESPACE" -o jsonpath='{.items[*].metadata.name}')
    
    if [ -z "$services" ]; then
        log_warning "No services found in namespace $NAMESPACE"
        return 1
    fi
    
    local failed=0
    for service in $services; do
        log_info "Checking service: $service"
        
        local endpoints
        endpoints=$(kubectl get endpoints "$service" -n "$NAMESPACE" -o jsonpath='{.subsets[*].addresses[*].ip}')
        
        if [ -n "$endpoints" ]; then
            log_success "Service $service has endpoints: $endpoints"
        else
            log_error "Service $service has no endpoints"
            failed=1
        fi
    done
    
    return $failed
}

# Validate ingress configuration
validate_ingress() {
    log_info "Validating ingress configuration in namespace: $NAMESPACE"
    
    local ingresses
    ingresses=$(kubectl get ingress -n "$NAMESPACE" -o jsonpath='{.items[*].metadata.name}')
    
    if [ -z "$ingresses" ]; then
        log_warning "No ingress resources found in namespace $NAMESPACE"
        return 0
    fi
    
    local failed=0
    for ingress in $ingresses; do
        log_info "Checking ingress: $ingress"
        
        local hosts
        hosts=$(kubectl get ingress "$ingress" -n "$NAMESPACE" -o jsonpath='{.spec.rules[*].host}')
        
        if [ -n "$hosts" ]; then
            log_success "Ingress $ingress configured for hosts: $hosts"
            
            # Check if ingress has an IP/hostname assigned
            local address
            address=$(kubectl get ingress "$ingress" -n "$NAMESPACE" -o jsonpath='{.status.loadBalancer.ingress[0].ip}{.status.loadBalancer.ingress[0].hostname}')
            
            if [ -n "$address" ]; then
                log_success "Ingress $ingress has address: $address"
            else
                log_warning "Ingress $ingress does not have an assigned address yet"
            fi
        else
            log_error "Ingress $ingress has no configured hosts"
            failed=1
        fi
    done
    
    return $failed
}

# Validate ConfigMaps and Secrets
validate_config() {
    log_info "Validating ConfigMaps and Secrets in namespace: $NAMESPACE"
    
    # Check ConfigMaps
    local configmaps
    configmaps=$(kubectl get configmaps -n "$NAMESPACE" -o jsonpath='{.items[*].metadata.name}')
    
    if [ -n "$configmaps" ]; then
        log_success "ConfigMaps found: $configmaps"
    else
        log_warning "No ConfigMaps found in namespace $NAMESPACE"
    fi
    
    # Check Secrets
    local secrets
    secrets=$(kubectl get secrets -n "$NAMESPACE" -o jsonpath='{.items[*].metadata.name}' | tr ' ' '\n' | grep -v "default-token" | tr '\n' ' ')
    
    if [ -n "$secrets" ]; then
        log_success "Secrets found: $secrets"
    else
        log_warning "No custom secrets found in namespace $NAMESPACE"
    fi
    
    return 0
}

# Validate HPA configuration
validate_hpa() {
    log_info "Validating Horizontal Pod Autoscaler in namespace: $NAMESPACE"
    
    local hpas
    hpas=$(kubectl get hpa -n "$NAMESPACE" -o jsonpath='{.items[*].metadata.name}')
    
    if [ -z "$hpas" ]; then
        log_warning "No HPA resources found in namespace $NAMESPACE"
        return 0
    fi
    
    local failed=0
    for hpa in $hpas; do
        log_info "Checking HPA: $hpa"
        
        local current_replicas
        current_replicas=$(kubectl get hpa "$hpa" -n "$NAMESPACE" -o jsonpath='{.status.currentReplicas}')
        local desired_replicas
        desired_replicas=$(kubectl get hpa "$hpa" -n "$NAMESPACE" -o jsonpath='{.status.desiredReplicas}')
        
        if [ -n "$current_replicas" ] && [ -n "$desired_replicas" ]; then
            log_success "HPA $hpa: current=$current_replicas, desired=$desired_replicas"
        else
            log_warning "HPA $hpa metrics not available yet"
        fi
    done
    
    return $failed
}

# Test application health endpoints
test_health_endpoints() {
    log_info "Testing application health endpoints..."
    
    # Get application pods
    local pods
    pods=$(kubectl get pods -n "$NAMESPACE" -l app=wifi-densepose -o jsonpath='{.items[*].metadata.name}')
    
    if [ -z "$pods" ]; then
        log_error "No application pods found"
        return 1
    fi
    
    local failed=0
    for pod in $pods; do
        log_info "Testing health endpoint for pod: $pod"
        
        # Port forward and test health endpoint
        kubectl port-forward "pod/$pod" 8080:8080 -n "$NAMESPACE" &
        local pf_pid=$!
        sleep 2
        
        if curl -f http://localhost:8080/health &> /dev/null; then
            log_success "Health endpoint for pod $pod is responding"
        else
            log_error "Health endpoint for pod $pod is not responding"
            failed=1
        fi
        
        kill $pf_pid 2>/dev/null || true
        sleep 1
    done
    
    return $failed
}

# Validate monitoring stack
validate_monitoring() {
    log_info "Validating monitoring stack in namespace: $MONITORING_NAMESPACE"
    
    if ! validate_namespace "$MONITORING_NAMESPACE"; then
        log_warning "Monitoring namespace not found, skipping monitoring validation"
        return 0
    fi
    
    # Check Prometheus
    if kubectl get deployment prometheus-server -n "$MONITORING_NAMESPACE" &> /dev/null; then
        if kubectl wait --for=condition=available --timeout=60s deployment/prometheus-server -n "$MONITORING_NAMESPACE" &> /dev/null; then
            log_success "Prometheus is running"
        else
            log_error "Prometheus is not ready"
        fi
    else
        log_warning "Prometheus deployment not found"
    fi
    
    # Check Grafana
    if kubectl get deployment grafana -n "$MONITORING_NAMESPACE" &> /dev/null; then
        if kubectl wait --for=condition=available --timeout=60s deployment/grafana -n "$MONITORING_NAMESPACE" &> /dev/null; then
            log_success "Grafana is running"
        else
            log_error "Grafana is not ready"
        fi
    else
        log_warning "Grafana deployment not found"
    fi
    
    return 0
}

# Validate logging stack
validate_logging() {
    log_info "Validating logging stack..."
    
    # Check Fluentd DaemonSet
    if kubectl get daemonset fluentd -n kube-system &> /dev/null; then
        local desired
        desired=$(kubectl get daemonset fluentd -n kube-system -o jsonpath='{.status.desiredNumberScheduled}')
        local ready
        ready=$(kubectl get daemonset fluentd -n kube-system -o jsonpath='{.status.numberReady}')
        
        if [ "$desired" = "$ready" ]; then
            log_success "Fluentd DaemonSet is ready ($ready/$desired nodes)"
        else
            log_warning "Fluentd DaemonSet has $ready/$desired pods ready"
        fi
    else
        log_warning "Fluentd DaemonSet not found"
    fi
    
    return 0
}

# Check resource usage
check_resource_usage() {
    log_info "Checking resource usage..."
    
    # Check node resource usage
    log_info "Node resource usage:"
    kubectl top nodes 2>/dev/null || log_warning "Metrics server not available for node metrics"
    
    # Check pod resource usage
    log_info "Pod resource usage in namespace $NAMESPACE:"
    kubectl top pods -n "$NAMESPACE" 2>/dev/null || log_warning "Metrics server not available for pod metrics"
    
    return 0
}

# Generate validation report
generate_report() {
    local total_checks=$1
    local failed_checks=$2
    local passed_checks=$((total_checks - failed_checks))
    
    echo ""
    log_info "=== Deployment Validation Report ==="
    echo "Total checks: $total_checks"
    echo "Passed: $passed_checks"
    echo "Failed: $failed_checks"
    
    if [ $failed_checks -eq 0 ]; then
        log_success "All validation checks passed! 🎉"
        return 0
    else
        log_error "Some validation checks failed. Please review the output above."
        return 1
    fi
}

# Main validation function
main() {
    log_info "Starting WiFi-DensePose deployment validation..."
    
    local total_checks=0
    local failed_checks=0
    
    # Run validation checks
    checks=(
        "check_kubectl"
        "validate_namespace $NAMESPACE"
        "validate_deployments"
        "validate_services"
        "validate_ingress"
        "validate_config"
        "validate_hpa"
        "test_health_endpoints"
        "validate_monitoring"
        "validate_logging"
        "check_resource_usage"
    )
    
    for check in "${checks[@]}"; do
        total_checks=$((total_checks + 1))
        echo ""
        if ! eval "$check"; then
            failed_checks=$((failed_checks + 1))
        fi
    done
    
    # Generate final report
    generate_report $total_checks $failed_checks
}

# Run main function
main "$@"