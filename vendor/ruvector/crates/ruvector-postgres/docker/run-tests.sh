#!/usr/bin/env bash
# RuVector-Postgres Test Runner
# Builds Docker image, runs tests, and cleans up

set -e  # Exit on error
set -u  # Exit on undefined variable
set -o pipefail  # Exit on pipe failure

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
CONTAINER_NAME="ruvector-postgres-test"
IMAGE_NAME="ruvector-postgres:test"
POSTGRES_PORT="${POSTGRES_PORT:-5433}"
POSTGRES_USER="${POSTGRES_USER:-ruvector}"
POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-ruvector}"
POSTGRES_DB="${POSTGRES_DB:-ruvector_test}"

# Detect OS
OS_TYPE="$(uname -s)"
case "${OS_TYPE}" in
    Linux*)     PLATFORM="linux";;
    Darwin*)    PLATFORM="macos";;
    *)          PLATFORM="unknown";;
esac

# Functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

cleanup() {
    log_info "Cleaning up containers and volumes..."
    docker stop "${CONTAINER_NAME}" 2>/dev/null || true
    docker rm "${CONTAINER_NAME}" 2>/dev/null || true
    if [ "${KEEP_VOLUMES:-false}" != "true" ]; then
        docker volume rm "${CONTAINER_NAME}_data" 2>/dev/null || true
    fi
}

wait_for_postgres() {
    log_info "Waiting for PostgreSQL to be healthy..."
    local max_attempts=30
    local attempt=1

    while [ ${attempt} -le ${max_attempts} ]; do
        if docker exec "${CONTAINER_NAME}" pg_isready -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" &>/dev/null; then
            log_success "PostgreSQL is ready!"
            return 0
        fi

        echo -n "."
        sleep 1
        attempt=$((attempt + 1))
    done

    log_error "PostgreSQL failed to become ready after ${max_attempts} seconds"
    docker logs "${CONTAINER_NAME}"
    return 1
}

build_image() {
    log_info "Building Docker image: ${IMAGE_NAME}"
    log_info "Platform: ${PLATFORM}"

    cd "${PROJECT_ROOT}"

    # Build with BuildKit for better caching
    DOCKER_BUILDKIT=1 docker build \
        -f crates/ruvector-postgres/docker/Dockerfile \
        -t "${IMAGE_NAME}" \
        --build-arg BUILDKIT_INLINE_CACHE=1 \
        --progress=plain \
        .

    log_success "Docker image built successfully"
}

start_container() {
    log_info "Starting PostgreSQL container: ${CONTAINER_NAME}"

    # Create volume for data persistence
    docker volume create "${CONTAINER_NAME}_data" || true

    # Start container
    docker run -d \
        --name "${CONTAINER_NAME}" \
        -p "${POSTGRES_PORT}:5432" \
        -e POSTGRES_USER="${POSTGRES_USER}" \
        -e POSTGRES_PASSWORD="${POSTGRES_PASSWORD}" \
        -e POSTGRES_DB="${POSTGRES_DB}" \
        -v "${CONTAINER_NAME}_data:/var/lib/postgresql/data" \
        --health-cmd="pg_isready -U ${POSTGRES_USER} -d ${POSTGRES_DB}" \
        --health-interval=5s \
        --health-timeout=5s \
        --health-retries=5 \
        "${IMAGE_NAME}"

    log_success "Container started"
}

run_tests() {
    log_info "Running test suite..."

    # Export connection string for tests
    export DATABASE_URL="postgresql://${POSTGRES_USER}:${POSTGRES_PASSWORD}@localhost:${POSTGRES_PORT}/${POSTGRES_DB}"

    log_info "Connection string: ${DATABASE_URL}"

    # Run pgrx tests
    cd "${PROJECT_ROOT}/crates/ruvector-postgres"

    log_info "Running pgrx tests..."
    if cargo pgrx test pg16; then
        log_success "All tests passed!"
        return 0
    else
        log_error "Tests failed!"
        return 1
    fi
}

run_integration_tests() {
    log_info "Running integration tests via SQL..."

    # Wait a bit more for full initialization
    sleep 2

    # Test extension loading
    log_info "Testing extension installation..."
    docker exec -it "${CONTAINER_NAME}" psql -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" -c "CREATE EXTENSION IF NOT EXISTS ruvector_postgres;" || {
        log_error "Failed to create extension"
        return 1
    }

    # Test basic vector operations
    log_info "Testing basic vector operations..."
    docker exec -it "${CONTAINER_NAME}" psql -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" << 'EOF'
-- Test vector creation
SELECT '[1,2,3]'::vector;

-- Test distance functions
SELECT vector_l2_distance('[1,2,3]'::vector, '[4,5,6]'::vector);
SELECT vector_cosine_distance('[1,2,3]'::vector, '[4,5,6]'::vector);
SELECT vector_inner_product('[1,2,3]'::vector, '[4,5,6]'::vector);

-- Test table creation with vector column
CREATE TABLE IF NOT EXISTS test_vectors (
    id SERIAL PRIMARY KEY,
    embedding vector(3)
);

-- Insert test data
INSERT INTO test_vectors (embedding) VALUES
    ('[1,2,3]'::vector),
    ('[4,5,6]'::vector),
    ('[7,8,9]'::vector);

-- Test similarity search
SELECT * FROM test_vectors ORDER BY embedding <-> '[1,2,3]'::vector LIMIT 3;

-- Cleanup
DROP TABLE test_vectors;
EOF

    if [ $? -eq 0 ]; then
        log_success "Integration tests passed!"
        return 0
    else
        log_error "Integration tests failed!"
        return 1
    fi
}

collect_results() {
    log_info "Collecting test results..."

    # Create results directory
    local results_dir="${PROJECT_ROOT}/test-results"
    mkdir -p "${results_dir}"

    # Export container logs
    docker logs "${CONTAINER_NAME}" > "${results_dir}/postgres.log" 2>&1

    # Export test database dump (if needed)
    if [ "${EXPORT_DB:-false}" == "true" ]; then
        log_info "Exporting database dump..."
        docker exec "${CONTAINER_NAME}" pg_dump -U "${POSTGRES_USER}" "${POSTGRES_DB}" > "${results_dir}/test_db_dump.sql"
    fi

    log_success "Results collected in ${results_dir}"
}

show_usage() {
    cat << EOF
RuVector-Postgres Test Runner

Usage: $0 [OPTIONS]

Options:
    -b, --build-only       Build Docker image only, don't run tests
    -t, --test-only        Run tests only (skip build)
    -i, --integration      Run integration tests only
    -k, --keep-running     Keep container running after tests
    -c, --clean            Clean up before starting
    -v, --keep-volumes     Keep volumes after cleanup
    -p, --port PORT        PostgreSQL port (default: 5433)
    -h, --help             Show this help message

Environment Variables:
    POSTGRES_PORT          PostgreSQL port (default: 5433)
    POSTGRES_USER          PostgreSQL user (default: ruvector)
    POSTGRES_PASSWORD      PostgreSQL password (default: ruvector)
    POSTGRES_DB            PostgreSQL database (default: ruvector_test)
    KEEP_VOLUMES           Keep volumes after cleanup (default: false)
    EXPORT_DB              Export database dump (default: false)

Examples:
    # Run full test suite
    $0

    # Build and keep container running for debugging
    $0 --keep-running

    # Run integration tests only
    $0 --integration --test-only

    # Clean rebuild
    $0 --clean --build-only
EOF
}

main() {
    local build_only=false
    local test_only=false
    local integration_only=false
    local keep_running=false
    local clean_first=false

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -b|--build-only)
                build_only=true
                shift
                ;;
            -t|--test-only)
                test_only=true
                shift
                ;;
            -i|--integration)
                integration_only=true
                shift
                ;;
            -k|--keep-running)
                keep_running=true
                shift
                ;;
            -c|--clean)
                clean_first=true
                shift
                ;;
            -v|--keep-volumes)
                KEEP_VOLUMES=true
                shift
                ;;
            -p|--port)
                POSTGRES_PORT="$2"
                shift 2
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

    # Setup trap for cleanup
    if [ "${keep_running}" != "true" ]; then
        trap cleanup EXIT
    fi

    log_info "RuVector-Postgres Test Runner"
    log_info "Platform: ${PLATFORM}"
    log_info "PostgreSQL Port: ${POSTGRES_PORT}"

    # Clean if requested
    if [ "${clean_first}" == "true" ]; then
        cleanup
    fi

    # Build phase
    if [ "${test_only}" != "true" ]; then
        build_image
    fi

    if [ "${build_only}" == "true" ]; then
        log_success "Build complete!"
        exit 0
    fi

    # Test phase
    start_container
    wait_for_postgres

    local test_result=0

    if [ "${integration_only}" == "true" ]; then
        run_integration_tests || test_result=$?
    else
        # Run both pgrx and integration tests
        run_integration_tests || test_result=$?

        if [ ${test_result} -eq 0 ]; then
            # Only run pgrx tests if integration tests passed
            run_tests || test_result=$?
        fi
    fi

    collect_results

    if [ "${keep_running}" == "true" ]; then
        log_info "Container is still running: ${CONTAINER_NAME}"
        log_info "Connection: postgresql://${POSTGRES_USER}:${POSTGRES_PASSWORD}@localhost:${POSTGRES_PORT}/${POSTGRES_DB}"
        log_info "To stop: docker stop ${CONTAINER_NAME}"
        trap - EXIT  # Disable cleanup trap
    fi

    if [ ${test_result} -eq 0 ]; then
        log_success "All tests completed successfully!"
        exit 0
    else
        log_error "Tests failed with exit code ${test_result}"
        exit ${test_result}
    fi
}

# Run main function
main "$@"
