#!/usr/bin/env bash
# RuVector-Postgres Docker Test Script
# Quick start script for building and running tests in Docker
#
# Usage:
#   ./scripts/docker-test.sh              # Run all tests
#   ./scripts/docker-test.sh --build      # Build only
#   ./scripts/docker-test.sh --benchmark  # Run benchmarks
#   ./scripts/docker-test.sh --matrix     # Run matrix tests (all PG versions)
#   ./scripts/docker-test.sh --clean      # Clean up containers and volumes

set -e
set -u
set -o pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Script configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$(cd "${SCRIPT_DIR}/../docker" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

# Default configuration
PG_VERSION="${PG_VERSION:-17}"
COMPOSE_FILE="${DOCKER_DIR}/docker-compose.yml"
COMPOSE_PROJECT="ruvector"

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_header() {
    echo ""
    echo -e "${CYAN}============================================================${NC}"
    echo -e "${CYAN}  $1${NC}"
    echo -e "${CYAN}============================================================${NC}"
    echo ""
}

# Show usage
show_usage() {
    cat << EOF
RuVector-Postgres Docker Test Script

Usage: $0 [OPTIONS] [COMMAND]

Commands:
    test        Run test suite (default)
    benchmark   Run performance benchmarks
    matrix      Run tests for all PostgreSQL versions
    build       Build Docker images only
    clean       Clean up containers, images, and volumes
    shell       Open development shell
    logs        Show container logs

Options:
    -p, --pg-version VERSION   PostgreSQL version (14, 15, 16, 17) [default: 17]
    -v, --verbose              Enable verbose output
    -k, --keep                 Keep containers running after tests
    -f, --follow               Follow logs after starting
    --no-cache                 Build without Docker cache
    -h, --help                 Show this help message

Environment Variables:
    PG_VERSION          PostgreSQL version [default: 17]
    RUST_VERSION        Rust version [default: 1.83]
    POSTGRES_PORT       Host port for PostgreSQL [default: 5432]
    COMPARE_BASELINE    Compare benchmarks to baseline [default: false]

Examples:
    # Run tests with default PostgreSQL 17
    $0 test

    # Run tests with PostgreSQL 16
    $0 --pg-version 16 test

    # Run benchmarks
    $0 benchmark

    # Run matrix tests (all PG versions)
    $0 matrix

    # Build images without cache
    $0 --no-cache build

    # Open development shell
    $0 shell

    # Clean up everything
    $0 clean
EOF
}

# Docker Compose wrapper
dc() {
    docker compose -f "${COMPOSE_FILE}" -p "${COMPOSE_PROJECT}" "$@"
}

# Build Docker images
cmd_build() {
    log_header "Building Docker Images"
    log_info "PostgreSQL Version: ${PG_VERSION}"
    log_info "Docker Context: ${PROJECT_ROOT}"

    local build_args="--build-arg PG_VERSION=${PG_VERSION}"
    if [ "${NO_CACHE:-false}" == "true" ]; then
        build_args="${build_args} --no-cache"
    fi

    cd "${PROJECT_ROOT}"

    # Build main PostgreSQL image
    log_info "Building ruvector-postgres image..."
    DOCKER_BUILDKIT=1 docker build \
        ${build_args} \
        -f crates/ruvector-postgres/docker/Dockerfile \
        -t "ruvector-postgres:pg${PG_VERSION}" \
        --progress=plain \
        .

    log_success "Docker images built successfully"
}

# Run test suite
cmd_test() {
    log_header "Running Test Suite"
    log_info "PostgreSQL Version: ${PG_VERSION}"

    cd "${DOCKER_DIR}"

    # Start PostgreSQL and wait for it to be healthy
    log_info "Starting PostgreSQL..."
    PG_VERSION=${PG_VERSION} dc up -d postgres

    log_info "Waiting for PostgreSQL to be ready..."
    local max_wait=60
    local waited=0
    while [ ${waited} -lt ${max_wait} ]; do
        if dc exec -T postgres pg_isready -U ruvector -d ruvector_test &>/dev/null; then
            log_success "PostgreSQL is ready!"
            break
        fi
        echo -n "."
        sleep 2
        waited=$((waited + 2))
    done
    echo ""

    if [ ${waited} -ge ${max_wait} ]; then
        log_error "PostgreSQL failed to start within ${max_wait} seconds"
        dc logs postgres
        exit 1
    fi

    # Run test runner
    log_info "Running tests..."
    PG_VERSION=${PG_VERSION} dc run --rm test-runner

    # Collect results
    log_info "Test results available in: ruvector-test-results volume"

    if [ "${KEEP_RUNNING:-false}" != "true" ]; then
        log_info "Stopping containers..."
        dc down
    else
        log_info "Containers kept running. Stop with: docker-compose -f ${COMPOSE_FILE} down"
    fi

    log_success "Test suite completed!"
}

# Run benchmarks
cmd_benchmark() {
    log_header "Running Performance Benchmarks"
    log_info "PostgreSQL Version: ${PG_VERSION}"

    cd "${DOCKER_DIR}"

    # Start PostgreSQL
    log_info "Starting PostgreSQL..."
    PG_VERSION=${PG_VERSION} dc up -d postgres

    log_info "Waiting for PostgreSQL..."
    sleep 10

    # Run benchmarks
    log_info "Running benchmarks..."
    PG_VERSION=${PG_VERSION} dc --profile benchmark run --rm benchmark

    log_info "Benchmark results available in: ruvector-benchmark-results volume"

    if [ "${KEEP_RUNNING:-false}" != "true" ]; then
        dc down
    fi

    log_success "Benchmarks completed!"
}

# Run matrix tests (all PG versions)
cmd_matrix() {
    log_header "Running Matrix Tests (All PostgreSQL Versions)"

    local versions=(14 15 16 17)
    local failed=()

    for version in "${versions[@]}"; do
        log_header "Testing PostgreSQL ${version}"

        if PG_VERSION=${version} cmd_test; then
            log_success "PostgreSQL ${version}: PASSED"
        else
            log_error "PostgreSQL ${version}: FAILED"
            failed+=("${version}")
        fi
    done

    echo ""
    log_header "Matrix Test Summary"

    if [ ${#failed[@]} -eq 0 ]; then
        log_success "All PostgreSQL versions passed!"
        return 0
    else
        log_error "Failed versions: ${failed[*]}"
        return 1
    fi
}

# Open development shell
cmd_shell() {
    log_header "Opening Development Shell"

    cd "${DOCKER_DIR}"

    # Start PostgreSQL
    log_info "Starting PostgreSQL..."
    PG_VERSION=${PG_VERSION} dc up -d postgres

    log_info "Waiting for PostgreSQL..."
    sleep 5

    # Start dev shell
    log_info "Opening shell..."
    PG_VERSION=${PG_VERSION} dc --profile dev run --rm dev
}

# Show logs
cmd_logs() {
    cd "${DOCKER_DIR}"

    if [ -n "${1:-}" ]; then
        dc logs -f "$1"
    else
        dc logs -f
    fi
}

# Clean up
cmd_clean() {
    log_header "Cleaning Up Docker Resources"

    cd "${DOCKER_DIR}"

    log_info "Stopping all containers..."
    dc down --volumes --remove-orphans 2>/dev/null || true
    dc --profile benchmark down --volumes 2>/dev/null || true
    dc --profile dev down --volumes 2>/dev/null || true
    dc --profile matrix down --volumes 2>/dev/null || true

    log_info "Removing images..."
    docker rmi ruvector-postgres:pg14 2>/dev/null || true
    docker rmi ruvector-postgres:pg15 2>/dev/null || true
    docker rmi ruvector-postgres:pg16 2>/dev/null || true
    docker rmi ruvector-postgres:pg17 2>/dev/null || true

    log_info "Removing volumes..."
    docker volume rm ruvector-postgres-data 2>/dev/null || true
    docker volume rm ruvector-cargo-cache 2>/dev/null || true
    docker volume rm ruvector-cargo-git 2>/dev/null || true
    docker volume rm ruvector-target-cache 2>/dev/null || true
    docker volume rm ruvector-test-results 2>/dev/null || true
    docker volume rm ruvector-benchmark-results 2>/dev/null || true

    log_info "Pruning unused Docker resources..."
    docker system prune -f

    log_success "Cleanup completed!"
}

# Main function
main() {
    local command="test"
    local verbose=false

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            test|benchmark|matrix|build|clean|shell|logs)
                command="$1"
                shift
                ;;
            -p|--pg-version)
                PG_VERSION="$2"
                shift 2
                ;;
            -v|--verbose)
                verbose=true
                set -x
                shift
                ;;
            -k|--keep)
                export KEEP_RUNNING=true
                shift
                ;;
            -f|--follow)
                export FOLLOW_LOGS=true
                shift
                ;;
            --no-cache)
                export NO_CACHE=true
                shift
                ;;
            -h|--help)
                show_usage
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                show_usage
                exit 1
                ;;
        esac
    done

    # Validate PG version
    case "${PG_VERSION}" in
        14|15|16|17) ;;
        *)
            log_error "Invalid PostgreSQL version: ${PG_VERSION}"
            log_error "Valid versions: 14, 15, 16, 17"
            exit 1
            ;;
    esac

    export PG_VERSION

    # Check Docker
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed. Please install Docker first."
        exit 1
    fi

    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running. Please start Docker."
        exit 1
    fi

    # Execute command
    case "${command}" in
        test)
            cmd_test
            ;;
        benchmark)
            cmd_benchmark
            ;;
        matrix)
            cmd_matrix
            ;;
        build)
            cmd_build
            ;;
        clean)
            cmd_clean
            ;;
        shell)
            cmd_shell
            ;;
        logs)
            cmd_logs "${2:-}"
            ;;
        *)
            log_error "Unknown command: ${command}"
            show_usage
            exit 1
            ;;
    esac
}

# Run main
main "$@"
