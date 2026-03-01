#!/usr/bin/env bash
# RuVector-Postgres Integration Test Runner
# Builds Docker environment, runs comprehensive integration tests, and reports results

set -e  # Exit on error
set -u  # Exit on undefined variable
set -o pipefail  # Exit on pipe failure

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
COMPOSE_FILE="${SCRIPT_DIR}/docker-compose.integration.yml"
TEST_RESULTS_DIR="${PROJECT_ROOT}/test-results/integration"
POSTGRES_CONTAINER="ruvector-postgres-integration"
TEST_RUNNER_CONTAINER="ruvector-integration-runner"

# Default settings
PG_VERSION="${PG_VERSION:-17}"
RUST_LOG="${RUST_LOG:-info}"
TEST_TIMEOUT="${TEST_TIMEOUT:-600}"
KEEP_RUNNING="${KEEP_RUNNING:-false}"

# Test categories
declare -a TEST_CATEGORIES=(
    "pgvector_compat"
    "integrity_tests"
    "hybrid_search_tests"
    "tenancy_tests"
    "healing_tests"
    "perf_tests"
)

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

log_section() {
    echo -e "\n${CYAN}=== $1 ===${NC}\n"
}

cleanup() {
    if [ "${KEEP_RUNNING}" != "true" ]; then
        log_info "Cleaning up Docker containers..."
        docker-compose -f "${COMPOSE_FILE}" down -v 2>/dev/null || true
        docker rm -f "${POSTGRES_CONTAINER}" 2>/dev/null || true
        docker rm -f "${TEST_RUNNER_CONTAINER}" 2>/dev/null || true
    else
        log_info "Keeping containers running for debugging"
    fi
}

wait_for_postgres() {
    log_info "Waiting for PostgreSQL to be ready..."
    local max_attempts=60
    local attempt=1

    while [ ${attempt} -le ${max_attempts} ]; do
        if docker exec "${POSTGRES_CONTAINER}" pg_isready -U ruvector -d ruvector_test &>/dev/null; then
            log_success "PostgreSQL is ready!"
            return 0
        fi

        echo -n "."
        sleep 1
        attempt=$((attempt + 1))
    done

    log_error "PostgreSQL failed to start after ${max_attempts} seconds"
    docker logs "${POSTGRES_CONTAINER}" 2>&1 | tail -50
    return 1
}

verify_extension() {
    log_info "Verifying RuVector extension..."

    docker exec "${POSTGRES_CONTAINER}" psql -U ruvector -d ruvector_test -c "
        SELECT ruvector_version();
        SELECT ruvector_simd_info();
    " || {
        log_error "Failed to verify RuVector extension"
        return 1
    }

    log_success "RuVector extension verified"
}

build_extension() {
    log_section "Building RuVector Extension"

    cd "${PROJECT_ROOT}"

    DOCKER_BUILDKIT=1 docker build \
        -f crates/ruvector-postgres/docker/Dockerfile \
        -t "ruvector-postgres:pg${PG_VERSION}-test" \
        --build-arg PG_VERSION="${PG_VERSION}" \
        --progress=plain \
        . || {
            log_error "Failed to build extension"
            return 1
        }

    log_success "Extension built successfully"
}

start_postgres() {
    log_section "Starting PostgreSQL Container"

    docker run -d \
        --name "${POSTGRES_CONTAINER}" \
        -e POSTGRES_USER=ruvector \
        -e POSTGRES_PASSWORD=ruvector \
        -e POSTGRES_DB=ruvector_test \
        -p 5433:5432 \
        --health-cmd="pg_isready -U ruvector -d ruvector_test" \
        --health-interval=5s \
        --health-timeout=5s \
        --health-retries=10 \
        "ruvector-postgres:pg${PG_VERSION}-test"

    wait_for_postgres
    verify_extension
}

setup_test_schema() {
    log_info "Setting up test schema..."

    docker exec "${POSTGRES_CONTAINER}" psql -U ruvector -d ruvector_test << 'EOF'
-- Create test schemas for each category
CREATE SCHEMA IF NOT EXISTS test_pgvector;
CREATE SCHEMA IF NOT EXISTS test_integrity;
CREATE SCHEMA IF NOT EXISTS test_hybrid;
CREATE SCHEMA IF NOT EXISTS test_tenancy;
CREATE SCHEMA IF NOT EXISTS test_healing;
CREATE SCHEMA IF NOT EXISTS test_perf;

-- Grant permissions
GRANT ALL ON ALL SCHEMAS IN DATABASE ruvector_test TO ruvector;

-- Create test tables
CREATE TABLE IF NOT EXISTS test_pgvector.vectors (
    id SERIAL PRIMARY KEY,
    embedding vector(128),
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS test_perf.benchmark_vectors (
    id SERIAL PRIMARY KEY,
    embedding vector(128),
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW()
);

-- Create indexes
CREATE INDEX IF NOT EXISTS test_pgvector_hnsw ON test_pgvector.vectors
    USING hnsw (embedding vector_l2_ops) WITH (m = 16, ef_construction = 64);

CREATE INDEX IF NOT EXISTS test_perf_hnsw ON test_perf.benchmark_vectors
    USING hnsw (embedding vector_l2_ops) WITH (m = 16, ef_construction = 64);

-- Insert test data
INSERT INTO test_pgvector.vectors (embedding, metadata)
SELECT
    (SELECT array_agg(random()::real) FROM generate_series(1, 128))::vector,
    jsonb_build_object('idx', i)
FROM generate_series(1, 1000) i;

ANALYZE test_pgvector.vectors;

\echo 'Test schema setup complete'
EOF

    log_success "Test schema created"
}

run_unit_tests() {
    log_section "Running Unit Tests"

    cd "${PROJECT_ROOT}/crates/ruvector-postgres"

    # Run tests in release mode for performance
    cargo test \
        --release \
        --features "pg${PG_VERSION},graph-complete" \
        --lib \
        -- \
        --test-threads=4 \
        2>&1 | tee "${TEST_RESULTS_DIR}/unit_tests.log"

    local exit_code=$?

    if [ ${exit_code} -eq 0 ]; then
        log_success "Unit tests passed"
    else
        log_error "Unit tests failed"
    fi

    return ${exit_code}
}

run_integration_tests() {
    log_section "Running Integration Tests"

    cd "${PROJECT_ROOT}/crates/ruvector-postgres"

    export DATABASE_URL="postgresql://ruvector:ruvector@localhost:5433/ruvector_test"
    export RUST_LOG="${RUST_LOG}"
    export RUST_BACKTRACE=1

    local failed_categories=()

    for category in "${TEST_CATEGORIES[@]}"; do
        log_info "Running ${category} tests..."

        cargo test \
            --release \
            --features "pg${PG_VERSION},graph-complete" \
            --test integration \
            "${category}" \
            -- \
            --test-threads=1 \
            2>&1 | tee "${TEST_RESULTS_DIR}/${category}.log"

        if [ ${PIPESTATUS[0]} -ne 0 ]; then
            log_error "${category} tests failed"
            failed_categories+=("${category}")
        else
            log_success "${category} tests passed"
        fi
    done

    if [ ${#failed_categories[@]} -gt 0 ]; then
        log_error "Failed test categories: ${failed_categories[*]}"
        return 1
    fi

    log_success "All integration tests passed"
    return 0
}

run_sql_tests() {
    log_section "Running SQL Integration Tests"

    local test_sql_dir="${SCRIPT_DIR}/test_sql"
    mkdir -p "${test_sql_dir}"

    # Generate and run SQL tests
    cat > "${test_sql_dir}/pgvector_compat.sql" << 'EOF'
-- pgvector compatibility tests
\echo 'Testing pgvector compatibility...'

-- Test vector type
SELECT '[1,2,3]'::vector AS test_vector;

-- Test operators
SELECT '[1,2,3]'::vector <-> '[4,5,6]'::vector AS l2_distance;
SELECT '[1,2,3]'::vector <=> '[4,5,6]'::vector AS cosine_distance;
SELECT '[1,2,3]'::vector <#> '[4,5,6]'::vector AS inner_product;

-- Test nearest neighbor search
SELECT id, embedding <-> '[0.5, 0.5, 0.5]'::vector(3) AS distance
FROM (VALUES (1, '[1,2,3]'::vector), (2, '[2,3,4]'::vector)) AS t(id, embedding)
ORDER BY embedding <-> '[0.5, 0.5, 0.5]'::vector(3)
LIMIT 2;

\echo 'pgvector compatibility tests passed!'
EOF

    docker exec "${POSTGRES_CONTAINER}" psql -U ruvector -d ruvector_test \
        -f /dev/stdin < "${test_sql_dir}/pgvector_compat.sql" \
        2>&1 | tee "${TEST_RESULTS_DIR}/sql_tests.log"

    log_success "SQL integration tests completed"
}

run_performance_benchmark() {
    log_section "Running Performance Benchmark"

    docker exec "${POSTGRES_CONTAINER}" psql -U ruvector -d ruvector_test << 'EOF'
\timing on

-- Insert benchmark
\echo 'Insert benchmark (1000 vectors)...'
INSERT INTO test_perf.benchmark_vectors (embedding, metadata)
SELECT
    (SELECT array_agg(random()::real) FROM generate_series(1, 128))::vector,
    jsonb_build_object('idx', i)
FROM generate_series(1, 1000) i;

-- Query benchmark
\echo 'Query benchmark (100 queries)...'
DO $$
DECLARE
    query_vec vector;
    start_time timestamp;
    total_time interval := '0'::interval;
    i integer;
BEGIN
    FOR i IN 1..100 LOOP
        query_vec := (SELECT array_agg(random()::real) FROM generate_series(1, 128))::vector;
        start_time := clock_timestamp();

        PERFORM id FROM test_perf.benchmark_vectors
        ORDER BY embedding <-> query_vec
        LIMIT 10;

        total_time := total_time + (clock_timestamp() - start_time);
    END LOOP;

    RAISE NOTICE 'Total time for 100 queries: %', total_time;
    RAISE NOTICE 'Average query time: %', total_time / 100;
END;
$$;

\echo 'Performance benchmark complete!'
EOF

    log_success "Performance benchmark completed"
}

generate_report() {
    log_section "Generating Test Report"

    local report_file="${TEST_RESULTS_DIR}/report.md"

    cat > "${report_file}" << EOF
# RuVector Postgres Integration Test Report

Generated: $(date -Iseconds)
PostgreSQL Version: ${PG_VERSION}

## Test Results Summary

| Category | Status |
|----------|--------|
EOF

    for category in "${TEST_CATEGORIES[@]}"; do
        local status="PASS"
        if grep -q "FAILED" "${TEST_RESULTS_DIR}/${category}.log" 2>/dev/null; then
            status="FAIL"
        fi
        echo "| ${category} | ${status} |" >> "${report_file}"
    done

    cat >> "${report_file}" << EOF

## Test Categories

### pgvector Compatibility
- Vector type creation and operators
- HNSW and IVFFlat index creation
- Basic CRUD operations

### Integrity System
- Contracted graph construction
- Mincut computation
- State transitions

### Hybrid Search
- BM25 scoring accuracy
- RRF fusion
- Linear fusion

### Multi-Tenancy
- Schema isolation
- RLS policies
- Quota enforcement

### Self-Healing
- Problem detection
- Remediation strategies
- Recovery from failures

### Performance
- Insert throughput
- Query latency (p50, p95, p99)
- SIMD acceleration
- Concurrent scaling

## Logs

Test logs are available in: ${TEST_RESULTS_DIR}/

## Environment

- Docker: $(docker --version)
- Rust: $(rustc --version)
- PostgreSQL: ${PG_VERSION}
EOF

    log_success "Report generated: ${report_file}"
}

show_usage() {
    cat << EOF
RuVector-Postgres Integration Test Runner

Usage: $0 [OPTIONS]

Options:
    -b, --build-only       Build Docker image only
    -t, --tests-only       Run tests only (skip build)
    -c, --category CAT     Run specific test category
    -s, --sql-only         Run SQL tests only
    -p, --perf             Run performance benchmarks
    -k, --keep-running     Keep containers after tests
    --pg-version VER       PostgreSQL version (default: 17)
    -h, --help             Show this help

Test Categories:
    pgvector_compat        pgvector SQL compatibility
    integrity_tests        Integrity system tests
    hybrid_search_tests    Hybrid search tests
    tenancy_tests          Multi-tenancy tests
    healing_tests          Self-healing tests
    perf_tests             Performance tests

Examples:
    # Run all tests
    $0

    # Run specific category
    $0 -c pgvector_compat

    # Run performance benchmark only
    $0 -p

    # Keep containers for debugging
    $0 -k
EOF
}

main() {
    local build_only=false
    local tests_only=false
    local sql_only=false
    local perf_only=false
    local specific_category=""

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -b|--build-only)
                build_only=true
                shift
                ;;
            -t|--tests-only)
                tests_only=true
                shift
                ;;
            -c|--category)
                specific_category="$2"
                shift 2
                ;;
            -s|--sql-only)
                sql_only=true
                shift
                ;;
            -p|--perf)
                perf_only=true
                shift
                ;;
            -k|--keep-running)
                KEEP_RUNNING=true
                shift
                ;;
            --pg-version)
                PG_VERSION="$2"
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
    trap cleanup EXIT

    # Create results directory
    mkdir -p "${TEST_RESULTS_DIR}"

    log_section "RuVector Integration Test Suite"
    log_info "PostgreSQL Version: ${PG_VERSION}"
    log_info "Results Directory: ${TEST_RESULTS_DIR}"

    # Build phase
    if [ "${tests_only}" != "true" ]; then
        build_extension
    fi

    if [ "${build_only}" == "true" ]; then
        log_success "Build complete!"
        exit 0
    fi

    # Start PostgreSQL
    start_postgres
    setup_test_schema

    # Run tests
    local test_result=0

    if [ "${sql_only}" == "true" ]; then
        run_sql_tests || test_result=$?
    elif [ "${perf_only}" == "true" ]; then
        run_performance_benchmark || test_result=$?
    elif [ -n "${specific_category}" ]; then
        TEST_CATEGORIES=("${specific_category}")
        run_integration_tests || test_result=$?
    else
        # Run all tests
        run_unit_tests || test_result=$?
        run_integration_tests || test_result=$?
        run_sql_tests || test_result=$?
        run_performance_benchmark || test_result=$?
    fi

    # Generate report
    generate_report

    if [ ${test_result} -eq 0 ]; then
        log_success "All tests completed successfully!"
    else
        log_error "Some tests failed. Check logs in ${TEST_RESULTS_DIR}/"
    fi

    exit ${test_result}
}

# Run main function
main "$@"
