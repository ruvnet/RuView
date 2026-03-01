#!/bin/bash
# RuVector Cloud Run Deployment Script
# Comprehensive deployment with GPU support, Raft clusters, and replication

set -euo pipefail

# =============================================================================
# CONFIGURATION
# =============================================================================

PROJECT_ID="${GCP_PROJECT_ID:-agentics-foundation25lon-1899}"
REGION="${GCP_REGION:-us-central1}"
SERVICE_NAME="${SERVICE_NAME:-ruvector-benchmark}"
IMAGE_NAME="gcr.io/${PROJECT_ID}/${SERVICE_NAME}"
ARTIFACT_REGISTRY="${ARTIFACT_REGISTRY:-${REGION}-docker.pkg.dev/${PROJECT_ID}/ruvector}"

# Cloud Run Configuration
MEMORY="${MEMORY:-8Gi}"
CPU="${CPU:-4}"
GPU_TYPE="${GPU_TYPE:-nvidia-l4}"
GPU_COUNT="${GPU_COUNT:-1}"
MIN_INSTANCES="${MIN_INSTANCES:-0}"
MAX_INSTANCES="${MAX_INSTANCES:-10}"
TIMEOUT="${TIMEOUT:-3600}"
CONCURRENCY="${CONCURRENCY:-80}"

# Cluster Configuration (for Raft/Replication)
CLUSTER_SIZE="${CLUSTER_SIZE:-3}"
CLUSTER_NAME="${CLUSTER_NAME:-ruvector-cluster}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${CYAN}[STEP]${NC} $1"; }

# =============================================================================
# HELPER FUNCTIONS
# =============================================================================

check_prerequisites() {
    log_step "Checking prerequisites..."

    # Check gcloud
    if ! command -v gcloud &> /dev/null; then
        log_error "gcloud CLI not found. Install from: https://cloud.google.com/sdk/docs/install"
        exit 1
    fi

    # Check docker
    if ! command -v docker &> /dev/null; then
        log_error "Docker not found. Install from: https://docs.docker.com/get-docker/"
        exit 1
    fi

    # Check authentication
    if ! gcloud auth print-identity-token &> /dev/null; then
        log_warning "Not authenticated with gcloud. Running 'gcloud auth login'..."
        gcloud auth login
    fi

    # Set project
    gcloud config set project "$PROJECT_ID" 2>/dev/null

    log_success "Prerequisites check passed"
}

enable_apis() {
    log_step "Enabling required Google Cloud APIs..."

    local apis=(
        "run.googleapis.com"
        "containerregistry.googleapis.com"
        "artifactregistry.googleapis.com"
        "cloudbuild.googleapis.com"
        "compute.googleapis.com"
        "secretmanager.googleapis.com"
    )

    for api in "${apis[@]}"; do
        log_info "Enabling $api..."
        gcloud services enable "$api" --quiet || true
    done

    log_success "APIs enabled"
}

# =============================================================================
# BUILD COMMANDS
# =============================================================================

build_image() {
    local dockerfile="${1:-Dockerfile.gpu}"
    local tag="${2:-latest}"

    log_step "Building Docker image: ${IMAGE_NAME}:${tag}"

    # Build locally
    docker build \
        -f "$dockerfile" \
        -t "${IMAGE_NAME}:${tag}" \
        --build-arg BUILDKIT_INLINE_CACHE=1 \
        ../.. || {
            log_error "Docker build failed"
            exit 1
        }

    log_success "Image built: ${IMAGE_NAME}:${tag}"
}

build_cloud() {
    local dockerfile="${1:-Dockerfile.gpu}"
    local tag="${2:-latest}"

    log_step "Building with Cloud Build: ${IMAGE_NAME}:${tag}"

    # Create cloudbuild.yaml
    cat > /tmp/cloudbuild.yaml << EOF
steps:
  - name: 'gcr.io/cloud-builders/docker'
    args: ['build', '-f', '${dockerfile}', '-t', '${IMAGE_NAME}:${tag}', '.']
    dir: 'examples/google-cloud'
  - name: 'gcr.io/cloud-builders/docker'
    args: ['push', '${IMAGE_NAME}:${tag}']
images:
  - '${IMAGE_NAME}:${tag}'
timeout: '3600s'
options:
  machineType: 'E2_HIGHCPU_32'
EOF

    gcloud builds submit \
        --config=/tmp/cloudbuild.yaml \
        --timeout=3600s \
        ../..

    log_success "Cloud Build completed"
}

push_image() {
    local tag="${1:-latest}"

    log_step "Pushing image to Container Registry..."

    # Configure Docker for GCR
    gcloud auth configure-docker --quiet

    docker push "${IMAGE_NAME}:${tag}"

    log_success "Image pushed: ${IMAGE_NAME}:${tag}"
}

# =============================================================================
# DEPLOY COMMANDS
# =============================================================================

deploy_benchmark() {
    local tag="${1:-latest}"
    local gpu="${2:-true}"

    log_step "Deploying RuVector Benchmark Service..."

    local gpu_args=""
    if [ "$gpu" = "true" ]; then
        gpu_args="--gpu=${GPU_COUNT} --gpu-type=${GPU_TYPE}"
    fi

    gcloud run deploy "${SERVICE_NAME}" \
        --image="${IMAGE_NAME}:${tag}" \
        --region="${REGION}" \
        --platform=managed \
        --memory="${MEMORY}" \
        --cpu="${CPU}" \
        ${gpu_args} \
        --min-instances="${MIN_INSTANCES}" \
        --max-instances="${MAX_INSTANCES}" \
        --timeout="${TIMEOUT}" \
        --concurrency="${CONCURRENCY}" \
        --port=8080 \
        --allow-unauthenticated \
        --set-env-vars="RUVECTOR_GPU_ENABLED=${gpu},RUST_LOG=info"

    local url=$(gcloud run services describe "${SERVICE_NAME}" \
        --region="${REGION}" \
        --format='value(status.url)')

    log_success "Deployed to: ${url}"
    echo ""
    echo "Test endpoints:"
    echo "  Health:    curl ${url}/health"
    echo "  Info:      curl ${url}/info"
    echo "  Benchmark: curl -X POST ${url}/benchmark/quick"
}

deploy_attention_gnn() {
    local tag="${1:-latest}"

    log_step "Deploying RuVector Attention/GNN Service..."

    gcloud run deploy "ruvector-attention" \
        --image="${IMAGE_NAME}:${tag}" \
        --region="${REGION}" \
        --platform=managed \
        --memory="16Gi" \
        --cpu="8" \
        --gpu="${GPU_COUNT}" \
        --gpu-type="${GPU_TYPE}" \
        --min-instances="1" \
        --max-instances="5" \
        --timeout="3600" \
        --concurrency="20" \
        --port=8080 \
        --set-env-vars="RUVECTOR_MODE=attention,RUVECTOR_GNN_LAYERS=3,RUVECTOR_GNN_HEADS=8"

    log_success "Attention/GNN service deployed"
}

deploy_raft_cluster() {
    log_step "Deploying RuVector Raft Consensus Cluster (${CLUSTER_SIZE} nodes)..."

    # Deploy each node in the Raft cluster
    for i in $(seq 1 $CLUSTER_SIZE); do
        local node_name="${CLUSTER_NAME}-node-${i}"
        local node_id=$((i - 1))

        log_info "Deploying Raft node ${i}/${CLUSTER_SIZE}: ${node_name}"

        # Build peer list (excluding self)
        local peers=""
        for j in $(seq 1 $CLUSTER_SIZE); do
            if [ "$j" != "$i" ]; then
                if [ -n "$peers" ]; then
                    peers="${peers},"
                fi
                peers="${peers}${CLUSTER_NAME}-node-${j}"
            fi
        done

        gcloud run deploy "${node_name}" \
            --image="${IMAGE_NAME}:latest" \
            --region="${REGION}" \
            --platform=managed \
            --memory="4Gi" \
            --cpu="2" \
            --min-instances="1" \
            --max-instances="1" \
            --timeout="3600" \
            --port=8080 \
            --no-allow-unauthenticated \
            --set-env-vars="RUVECTOR_MODE=raft,RUVECTOR_NODE_ID=${node_id},RUVECTOR_CLUSTER_SIZE=${CLUSTER_SIZE},RUVECTOR_PEERS=${peers}"
    done

    log_success "Raft cluster deployed with ${CLUSTER_SIZE} nodes"
}

deploy_replication() {
    local replicas="${1:-3}"

    log_step "Deploying RuVector with Replication (${replicas} replicas)..."

    # Deploy primary
    log_info "Deploying primary node..."
    gcloud run deploy "ruvector-primary" \
        --image="${IMAGE_NAME}:latest" \
        --region="${REGION}" \
        --platform=managed \
        --memory="8Gi" \
        --cpu="4" \
        --gpu="${GPU_COUNT}" \
        --gpu-type="${GPU_TYPE}" \
        --min-instances="1" \
        --max-instances="1" \
        --port=8080 \
        --set-env-vars="RUVECTOR_MODE=primary,RUVECTOR_REPLICATION_FACTOR=${replicas}"

    local primary_url=$(gcloud run services describe "ruvector-primary" \
        --region="${REGION}" \
        --format='value(status.url)')

    # Deploy replicas
    for i in $(seq 1 $((replicas - 1))); do
        log_info "Deploying replica ${i}..."
        gcloud run deploy "ruvector-replica-${i}" \
            --image="${IMAGE_NAME}:latest" \
            --region="${REGION}" \
            --platform=managed \
            --memory="8Gi" \
            --cpu="4" \
            --gpu="${GPU_COUNT}" \
            --gpu-type="${GPU_TYPE}" \
            --min-instances="1" \
            --max-instances="3" \
            --port=8080 \
            --set-env-vars="RUVECTOR_MODE=replica,RUVECTOR_PRIMARY_URL=${primary_url}"
    done

    log_success "Replication cluster deployed: 1 primary + $((replicas - 1)) replicas"
}

# =============================================================================
# MANAGEMENT COMMANDS
# =============================================================================

status() {
    log_step "Checking deployment status..."

    echo ""
    echo "=== Cloud Run Services ==="
    gcloud run services list --region="${REGION}" \
        --filter="metadata.name~ruvector" \
        --format="table(metadata.name,status.url,status.conditions[0].status)"

    echo ""
    echo "=== Container Images ==="
    gcloud container images list-tags "${IMAGE_NAME}" \
        --limit=5 \
        --format="table(tags,timestamp,digest)"
}

logs() {
    local service="${1:-${SERVICE_NAME}}"
    local limit="${2:-100}"

    log_step "Fetching logs for ${service}..."

    gcloud run services logs read "${service}" \
        --region="${REGION}" \
        --limit="${limit}"
}

metrics() {
    local service="${1:-${SERVICE_NAME}}"

    log_step "Fetching metrics for ${service}..."

    gcloud run services describe "${service}" \
        --region="${REGION}" \
        --format="yaml(status)"
}

cleanup() {
    log_step "Cleaning up RuVector deployments..."

    # List services to delete
    local services=$(gcloud run services list --region="${REGION}" \
        --filter="metadata.name~ruvector" \
        --format="value(metadata.name)")

    if [ -z "$services" ]; then
        log_info "No RuVector services found to clean up"
        return
    fi

    echo "Services to delete:"
    echo "$services"
    echo ""

    read -p "Delete these services? (y/N) " confirm
    if [ "$confirm" = "y" ] || [ "$confirm" = "Y" ]; then
        for service in $services; do
            log_info "Deleting ${service}..."
            gcloud run services delete "${service}" \
                --region="${REGION}" \
                --quiet
        done
        log_success "Cleanup complete"
    else
        log_info "Cleanup cancelled"
    fi
}

# =============================================================================
# BENCHMARK COMMANDS
# =============================================================================

run_benchmark() {
    local service="${1:-${SERVICE_NAME}}"
    local benchmark_type="${2:-quick}"

    local url=$(gcloud run services describe "${service}" \
        --region="${REGION}" \
        --format='value(status.url)')

    if [ -z "$url" ]; then
        log_error "Service ${service} not found"
        exit 1
    fi

    log_step "Running ${benchmark_type} benchmark on ${service}..."

    case "$benchmark_type" in
        quick)
            curl -X POST "${url}/benchmark/quick" \
                -H "Content-Type: application/json" | jq .
            ;;
        distance)
            curl -X POST "${url}/benchmark/distance?dims=768&num_vectors=100000" \
                -H "Content-Type: application/json" | jq .
            ;;
        hnsw)
            curl -X POST "${url}/benchmark/hnsw?dims=768&num_vectors=100000&k=10" \
                -H "Content-Type: application/json" | jq .
            ;;
        full)
            curl -X POST "${url}/benchmark" \
                -H "Content-Type: application/json" \
                -d '{"dims": 768, "num_vectors": 100000, "benchmark_type": "distance"}' | jq .

            curl -X POST "${url}/benchmark" \
                -H "Content-Type: application/json" \
                -d '{"dims": 768, "num_vectors": 100000, "benchmark_type": "hnsw", "k": 10}' | jq .
            ;;
        *)
            log_error "Unknown benchmark type: ${benchmark_type}"
            exit 1
            ;;
    esac
}

get_results() {
    local service="${1:-${SERVICE_NAME}}"

    local url=$(gcloud run services describe "${service}" \
        --region="${REGION}" \
        --format='value(status.url)')

    log_step "Fetching results from ${service}..."

    curl -s "${url}/results" | jq .
}

# =============================================================================
# USAGE
# =============================================================================

usage() {
    cat << EOF
RuVector Cloud Run Deployment Script

Usage: $0 <command> [options]

Build Commands:
    build [dockerfile] [tag]      Build Docker image locally
    build-cloud [dockerfile] [tag] Build with Cloud Build
    push [tag]                    Push image to Container Registry

Deploy Commands:
    deploy [tag] [gpu=true/false] Deploy benchmark service
    deploy-attention [tag]        Deploy attention/GNN service
    deploy-raft                   Deploy Raft consensus cluster
    deploy-replication [replicas] Deploy with replication

Management Commands:
    status                        Show deployment status
    logs [service] [limit]        View service logs
    metrics [service]             View service metrics
    cleanup                       Delete all RuVector services

Benchmark Commands:
    benchmark [service] [type]    Run benchmark (quick/distance/hnsw/full)
    results [service]             Get benchmark results

Setup Commands:
    setup                         Enable APIs and configure project
    prerequisites                 Check prerequisites

Environment Variables:
    GCP_PROJECT_ID     GCP project (default: ${PROJECT_ID})
    GCP_REGION         Region (default: ${REGION})
    SERVICE_NAME       Service name (default: ${SERVICE_NAME})
    MEMORY             Memory allocation (default: ${MEMORY})
    CPU                CPU allocation (default: ${CPU})
    GPU_TYPE           GPU type (default: ${GPU_TYPE})
    GPU_COUNT          GPU count (default: ${GPU_COUNT})
    CLUSTER_SIZE       Raft cluster size (default: ${CLUSTER_SIZE})

Examples:
    $0 setup                              # First-time setup
    $0 build Dockerfile.gpu latest        # Build GPU image
    $0 push latest                        # Push to registry
    $0 deploy latest true                 # Deploy with GPU
    $0 benchmark ruvector-benchmark quick # Run quick benchmark
    $0 deploy-raft                        # Deploy 3-node Raft cluster
    $0 cleanup                            # Remove all services

EOF
}

# =============================================================================
# MAIN
# =============================================================================

main() {
    local command="${1:-help}"
    shift || true

    case "$command" in
        # Setup
        setup)
            check_prerequisites
            enable_apis
            ;;
        prerequisites|prereq)
            check_prerequisites
            ;;

        # Build
        build)
            build_image "$@"
            ;;
        build-cloud)
            build_cloud "$@"
            ;;
        push)
            push_image "$@"
            ;;

        # Deploy
        deploy)
            deploy_benchmark "$@"
            ;;
        deploy-attention|deploy-gnn)
            deploy_attention_gnn "$@"
            ;;
        deploy-raft)
            deploy_raft_cluster
            ;;
        deploy-replication|deploy-replica)
            deploy_replication "$@"
            ;;

        # Management
        status)
            status
            ;;
        logs)
            logs "$@"
            ;;
        metrics)
            metrics "$@"
            ;;
        cleanup|clean)
            cleanup
            ;;

        # Benchmarks
        benchmark|bench)
            run_benchmark "$@"
            ;;
        results)
            get_results "$@"
            ;;

        # Help
        help|--help|-h)
            usage
            ;;

        *)
            log_error "Unknown command: $command"
            usage
            exit 1
            ;;
    esac
}

main "$@"
