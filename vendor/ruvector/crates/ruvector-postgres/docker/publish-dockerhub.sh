#!/usr/bin/env bash
# RuVector-Postgres Docker Hub Publication Script
# Builds and publishes multi-arch Docker images to Docker Hub
#
# Usage:
#   ./publish-dockerhub.sh                    # Build and push v2.0.0
#   ./publish-dockerhub.sh --dry-run          # Build only, don't push
#   ./publish-dockerhub.sh --pg-version 16    # Build for specific PG version
#   ./publish-dockerhub.sh --all-versions     # Build for all PG versions

set -e
set -u
set -o pipefail

# Configuration
DOCKER_REGISTRY="${DOCKER_REGISTRY:-ruvector}"
IMAGE_NAME="${IMAGE_NAME:-ruvector-postgres}"
VERSION="2.0.0"
RUST_VERSION="1.83"

# Supported PostgreSQL versions
PG_VERSIONS=(14 15 16 17)
DEFAULT_PG_VERSION=17

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Flags
DRY_RUN=false
ALL_VERSIONS=false
SINGLE_PG_VERSION=""
PUSH_LATEST=true

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --pg-version)
            SINGLE_PG_VERSION="$2"
            shift 2
            ;;
        --all-versions)
            ALL_VERSIONS=true
            shift
            ;;
        --no-latest)
            PUSH_LATEST=false
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --dry-run        Build only, don't push to Docker Hub"
            echo "  --pg-version N   Build for specific PostgreSQL version (14-17)"
            echo "  --all-versions   Build for all supported PostgreSQL versions"
            echo "  --no-latest      Don't tag as 'latest'"
            echo "  --help           Show this help"
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Determine which versions to build
if [[ -n "$SINGLE_PG_VERSION" ]]; then
    VERSIONS_TO_BUILD=("$SINGLE_PG_VERSION")
elif [[ "$ALL_VERSIONS" == "true" ]]; then
    VERSIONS_TO_BUILD=("${PG_VERSIONS[@]}")
else
    VERSIONS_TO_BUILD=("$DEFAULT_PG_VERSION")
fi

# Get script and project directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

log_info "=== RuVector-Postgres Docker Hub Publication ==="
log_info "Version: ${VERSION}"
log_info "Registry: ${DOCKER_REGISTRY}/${IMAGE_NAME}"
log_info "PostgreSQL versions: ${VERSIONS_TO_BUILD[*]}"
log_info "Dry run: ${DRY_RUN}"

# Verify Docker is available
if ! command -v docker &> /dev/null; then
    log_error "Docker is not installed"
    exit 1
fi

# Check Docker buildx for multi-arch support
if ! docker buildx version &> /dev/null; then
    log_warn "Docker buildx not available, multi-arch builds disabled"
    MULTI_ARCH=false
else
    log_info "Docker buildx available for multi-arch builds"
    MULTI_ARCH=true
fi

# Login check (skip for dry run)
if [[ "$DRY_RUN" == "false" ]]; then
    if ! docker info 2>/dev/null | grep -q "Username"; then
        log_warn "Not logged into Docker Hub. Please run: docker login"
        log_warn "Continuing with build only..."
        DRY_RUN=true
    fi
fi

# Create buildx builder if needed
if [[ "$MULTI_ARCH" == "true" ]]; then
    BUILDER_NAME="ruvector-builder"
    if ! docker buildx inspect "$BUILDER_NAME" &> /dev/null; then
        log_info "Creating buildx builder: ${BUILDER_NAME}"
        docker buildx create --name "$BUILDER_NAME" --driver docker-container --bootstrap
    fi
    docker buildx use "$BUILDER_NAME"
fi

# Build function
build_image() {
    local pg_version=$1
    local tags=()

    # Version tags
    tags+=("${DOCKER_REGISTRY}/${IMAGE_NAME}:${VERSION}-pg${pg_version}")
    tags+=("${DOCKER_REGISTRY}/${IMAGE_NAME}:v${VERSION}-pg${pg_version}")
    tags+=("${DOCKER_REGISTRY}/${IMAGE_NAME}:pg${pg_version}")

    # Latest tag for default PG version
    if [[ "$pg_version" == "$DEFAULT_PG_VERSION" && "$PUSH_LATEST" == "true" ]]; then
        tags+=("${DOCKER_REGISTRY}/${IMAGE_NAME}:latest")
        tags+=("${DOCKER_REGISTRY}/${IMAGE_NAME}:${VERSION}")
        tags+=("${DOCKER_REGISTRY}/${IMAGE_NAME}:v${VERSION}")
    fi

    log_info "Building image for PostgreSQL ${pg_version}..."
    log_info "Tags: ${tags[*]}"

    # Build tag arguments
    local tag_args=""
    for tag in "${tags[@]}"; do
        tag_args+=" -t ${tag}"
    done

    cd "$PROJECT_ROOT"

    if [[ "$MULTI_ARCH" == "true" ]]; then
        # Multi-arch build (amd64 + arm64)
        local push_flag=""
        if [[ "$DRY_RUN" == "false" ]]; then
            push_flag="--push"
        else
            push_flag="--load"
        fi

        docker buildx build \
            --platform linux/amd64,linux/arm64 \
            -f crates/ruvector-postgres/docker/Dockerfile \
            --build-arg PG_VERSION="${pg_version}" \
            --build-arg RUST_VERSION="${RUST_VERSION}" \
            ${tag_args} \
            ${push_flag} \
            .
    else
        # Single-arch build
        docker build \
            -f crates/ruvector-postgres/docker/Dockerfile \
            --build-arg PG_VERSION="${pg_version}" \
            --build-arg RUST_VERSION="${RUST_VERSION}" \
            ${tag_args} \
            .

        # Push if not dry run
        if [[ "$DRY_RUN" == "false" ]]; then
            for tag in "${tags[@]}"; do
                docker push "$tag"
            done
        fi
    fi

    log_success "Built image for PostgreSQL ${pg_version}"
}

# Build all requested versions
for pg_ver in "${VERSIONS_TO_BUILD[@]}"; do
    build_image "$pg_ver"
done

# Summary
echo ""
log_success "=== Publication Complete ==="
log_info "Images built:"
for pg_ver in "${VERSIONS_TO_BUILD[@]}"; do
    echo "  - ${DOCKER_REGISTRY}/${IMAGE_NAME}:${VERSION}-pg${pg_ver}"
done

if [[ "$DRY_RUN" == "true" ]]; then
    log_warn "Dry run mode - images were NOT pushed to Docker Hub"
    log_info "To push, run without --dry-run flag"
else
    log_success "Images pushed to Docker Hub!"
    log_info "Pull with: docker pull ${DOCKER_REGISTRY}/${IMAGE_NAME}:${VERSION}"
fi

# Print usage examples
echo ""
log_info "=== Usage Examples ==="
echo "  docker pull ${DOCKER_REGISTRY}/${IMAGE_NAME}:latest"
echo "  docker pull ${DOCKER_REGISTRY}/${IMAGE_NAME}:${VERSION}"
echo "  docker pull ${DOCKER_REGISTRY}/${IMAGE_NAME}:${VERSION}-pg17"
echo "  docker pull ${DOCKER_REGISTRY}/${IMAGE_NAME}:pg16"
echo ""
echo "  docker run -d -p 5432:5432 ${DOCKER_REGISTRY}/${IMAGE_NAME}:latest"
