#!/bin/bash
#
# RuVector Installation Verification Script
# Comprehensive test suite to verify the extension works correctly
#
# Usage: ./verify_installation.sh [OPTIONS]
#
# Options:
#   --database DB    Database to use for testing (default: creates temp db)
#   --host HOST      PostgreSQL host (default: localhost)
#   --port PORT      PostgreSQL port (default: 5432)
#   --user USER      PostgreSQL user (default: postgres)
#   --verbose        Show detailed output
#   --benchmark      Run performance benchmarks
#   --cleanup        Clean up test artifacts
#
set -e

# Configuration
TEST_DB=""
PG_HOST="${PGHOST:-localhost}"
PG_PORT="${PGPORT:-5432}"
PG_USER="${PGUSER:-postgres}"
VERBOSE=false
BENCHMARK=false
CLEANUP=false
TEMP_DB=false

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Counters
TESTS_PASSED=0
TESTS_FAILED=0
TESTS_SKIPPED=0

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[PASS]${NC} $1"; }
log_fail() { echo -e "${RED}[FAIL]${NC} $1"; }
log_skip() { echo -e "${YELLOW}[SKIP]${NC} $1"; }
log_verbose() { [ "$VERBOSE" = true ] && echo -e "[DEBUG] $1" || true; }

run_test() {
    local test_name="$1"
    local test_sql="$2"
    local expected="$3"

    log_verbose "Running: $test_sql"

    local result
    if result=$(psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$TEST_DB" \
                     -tAc "$test_sql" 2>&1); then
        if [ -z "$expected" ] || [[ "$result" == *"$expected"* ]]; then
            log_success "$test_name"
            ((TESTS_PASSED++))
            return 0
        else
            log_fail "$test_name (expected: $expected, got: $result)"
            ((TESTS_FAILED++))
            return 1
        fi
    else
        log_fail "$test_name (error: $result)"
        ((TESTS_FAILED++))
        return 1
    fi
}

run_test_numeric() {
    local test_name="$1"
    local test_sql="$2"
    local expected="$3"
    local tolerance="${4:-0.001}"

    log_verbose "Running: $test_sql"

    local result
    if result=$(psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$TEST_DB" \
                     -tAc "$test_sql" 2>&1); then
        # Compare with tolerance
        local diff=$(echo "$result - $expected" | bc -l 2>/dev/null | tr -d '-')
        if [ -n "$diff" ] && (( $(echo "$diff <= $tolerance" | bc -l) )); then
            log_success "$test_name (got: $result)"
            ((TESTS_PASSED++))
            return 0
        else
            log_fail "$test_name (expected: ~$expected, got: $result)"
            ((TESTS_FAILED++))
            return 1
        fi
    else
        log_fail "$test_name (error: $result)"
        ((TESTS_FAILED++))
        return 1
    fi
}

# ============================================================================
# Test Suites
# ============================================================================

test_extension_load() {
    echo ""
    echo "=== Extension Loading Tests ==="

    run_test "Create extension" \
        "DROP EXTENSION IF EXISTS ruvector CASCADE; CREATE EXTENSION ruvector;" \
        ""

    run_test "Extension exists" \
        "SELECT extname FROM pg_extension WHERE extname = 'ruvector';" \
        "ruvector"

    run_test "Check version" \
        "SELECT extversion FROM pg_extension WHERE extname = 'ruvector';" \
        "0.1.0"
}

test_type_creation() {
    echo ""
    echo "=== Type Creation Tests ==="

    run_test "Create table with ruvector" \
        "DROP TABLE IF EXISTS test_vec; CREATE TABLE test_vec (id serial, v ruvector);" \
        ""

    run_test "Create table with dimension constraint" \
        "DROP TABLE IF EXISTS test_vec_dim; CREATE TABLE test_vec_dim (id serial, v ruvector(128));" \
        ""
}

test_vector_io() {
    echo ""
    echo "=== Vector I/O Tests ==="

    run_test "Insert vector" \
        "INSERT INTO test_vec (v) VALUES ('[1,2,3]') RETURNING id;" \
        "1"

    run_test "Read vector" \
        "SELECT v FROM test_vec WHERE id = 1;" \
        "[1,2,3]"

    run_test "Insert multiple vectors" \
        "INSERT INTO test_vec (v) VALUES ('[4,5,6]'), ('[7,8,9]'), ('[10,11,12]'); SELECT count(*) FROM test_vec;" \
        "4"

    run_test "Insert high-dimensional vector" \
        "INSERT INTO test_vec (v) VALUES ('[' || array_to_string(array_agg(i::float4), ',') || ']') FROM generate_series(1, 128) i; SELECT count(*) FROM test_vec;" \
        "5"
}

test_distance_functions() {
    echo ""
    echo "=== Distance Function Tests ==="

    # L2 distance: sqrt((4-1)^2 + (5-2)^2 + (6-3)^2) = sqrt(27) = 5.196...
    run_test_numeric "L2 distance operator" \
        "SELECT '[1,2,3]'::ruvector <-> '[4,5,6]'::ruvector;" \
        "5.196" \
        "0.01"

    # Cosine distance
    run_test_numeric "Cosine distance operator" \
        "SELECT '[1,0,0]'::ruvector <=> '[0,1,0]'::ruvector;" \
        "1.0" \
        "0.01"

    # Inner product
    run_test_numeric "Inner product operator" \
        "SELECT '[1,2,3]'::ruvector <#> '[4,5,6]'::ruvector;" \
        "-32" \
        "0.01"

    # Test stored vector distances
    run_test "Distance from stored vectors" \
        "SELECT id FROM test_vec ORDER BY v <-> '[1,1,1]'::ruvector LIMIT 1;" \
        "1"
}

test_vector_functions() {
    echo ""
    echo "=== Vector Function Tests ==="

    run_test "Get dimensions" \
        "SELECT ruvector_dims('[1,2,3,4,5]'::ruvector);" \
        "5"

    run_test_numeric "Get norm" \
        "SELECT ruvector_norm('[3,4]'::ruvector);" \
        "5.0" \
        "0.001"

    run_test "Normalize vector" \
        "SELECT ruvector_dims(ruvector_normalize('[1,2,3]'::ruvector));" \
        "3"

    run_test_numeric "Normalized vector norm" \
        "SELECT ruvector_norm(ruvector_normalize('[3,4,0]'::ruvector));" \
        "1.0" \
        "0.001"
}

test_vector_arithmetic() {
    echo ""
    echo "=== Vector Arithmetic Tests ==="

    run_test "Vector addition" \
        "SELECT ruvector_add('[1,2,3]'::ruvector, '[4,5,6]'::ruvector);" \
        "[5,7,9]"

    run_test "Vector subtraction" \
        "SELECT ruvector_sub('[4,5,6]'::ruvector, '[1,2,3]'::ruvector);" \
        "[3,3,3]"

    run_test "Scalar multiplication" \
        "SELECT ruvector_mul_scalar('[1,2,3]'::ruvector, 2.0);" \
        "[2,4,6]"
}

test_aggregate_operations() {
    echo ""
    echo "=== Aggregate Operation Tests ==="

    run_test "Count vectors" \
        "SELECT count(*) FROM test_vec WHERE v <-> '[0,0,0]'::ruvector < 100;" \
        ""

    run_test "Min distance" \
        "SELECT count(*) FROM (SELECT min(v <-> '[1,1,1]'::ruvector) FROM test_vec) t;" \
        "1"

    run_test "Nearest neighbor query" \
        "SELECT count(*) FROM (SELECT id FROM test_vec ORDER BY v <-> '[1,1,1]'::ruvector LIMIT 3) t;" \
        "3"
}

test_temporal_functions() {
    echo ""
    echo "=== Temporal Function Tests ==="

    run_test "Temporal delta" \
        "SELECT temporal_delta(ARRAY[2.0,4.0,6.0], ARRAY[1.0,2.0,3.0]);" \
        "{1,2,3}"

    run_test "Temporal undelta" \
        "SELECT temporal_undelta(ARRAY[1.0,2.0,3.0], ARRAY[1.0,2.0,3.0]);" \
        "{2,4,6}"

    run_test_numeric "Temporal EMA update" \
        "SELECT (temporal_ema_update(ARRAY[1.0], ARRAY[0.0], 0.5))[1];" \
        "0.5" \
        "0.001"
}

test_attention_functions() {
    echo ""
    echo "=== Attention Function Tests ==="

    run_test_numeric "Attention score" \
        "SELECT attention_score(ARRAY[1.0,0.0], ARRAY[1.0,0.0]);" \
        "0.707" \
        "0.01"

    run_test "Attention softmax" \
        "SELECT array_length(attention_softmax(ARRAY[1.0, 2.0, 3.0]), 1);" \
        "3"

    run_test "Attention init" \
        "SELECT array_length(attention_init(128), 1);" \
        "128"
}

test_graph_functions() {
    echo ""
    echo "=== Graph Function Tests ==="

    run_test_numeric "Graph edge similarity (identical)" \
        "SELECT graph_edge_similarity(ARRAY[1.0,0.0], ARRAY[1.0,0.0]);" \
        "1.0" \
        "0.001"

    run_test_numeric "PageRank contribution" \
        "SELECT graph_pagerank_contribution(1.0, 4, 0.85);" \
        "0.2125" \
        "0.001"

    run_test "Graph is connected" \
        "SELECT graph_is_connected(ARRAY[1.0,0.0], ARRAY[0.9,0.1], 0.9);" \
        "t"
}

test_error_handling() {
    echo ""
    echo "=== Error Handling Tests ==="

    # Dimension mismatch
    local result
    if result=$(psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$TEST_DB" \
                     -c "SELECT '[1,2,3]'::ruvector <-> '[1,2]'::ruvector;" 2>&1); then
        log_fail "Should reject dimension mismatch"
        ((TESTS_FAILED++))
    else
        log_success "Rejects dimension mismatch"
        ((TESTS_PASSED++))
    fi

    # Invalid format
    if result=$(psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$TEST_DB" \
                     -c "SELECT 'invalid'::ruvector;" 2>&1); then
        log_fail "Should reject invalid format"
        ((TESTS_FAILED++))
    else
        log_success "Rejects invalid format"
        ((TESTS_PASSED++))
    fi
}

run_benchmarks() {
    echo ""
    echo "=== Performance Benchmarks ==="

    # Create benchmark table
    psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$TEST_DB" -c "
        DROP TABLE IF EXISTS bench_vec;
        CREATE TABLE bench_vec (id serial PRIMARY KEY, embedding ruvector);
    " >/dev/null 2>&1

    # Insert test data
    log_info "Generating 10,000 128-dimensional test vectors..."
    psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$TEST_DB" -c "
        DO \$\$
        DECLARE
            i INTEGER;
            vec TEXT;
            j INTEGER;
            vals TEXT[];
        BEGIN
            FOR i IN 1..10000 LOOP
                vals := ARRAY[]::TEXT[];
                FOR j IN 1..128 LOOP
                    vals := array_append(vals, (random() * 2 - 1)::float4::text);
                END LOOP;
                vec := '[' || array_to_string(vals, ',') || ']';
                INSERT INTO bench_vec (embedding) VALUES (vec::ruvector);
            END LOOP;
        END \$\$;
    " >/dev/null 2>&1

    # Run benchmark
    log_info "Running nearest neighbor benchmark (10K vectors, 128 dims)..."
    local result
    result=$(psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$TEST_DB" -c "
        EXPLAIN ANALYZE
        SELECT id, embedding <-> (SELECT embedding FROM bench_vec WHERE id = 1) AS dist
        FROM bench_vec
        ORDER BY dist
        LIMIT 10;
    " 2>&1)

    # Extract execution time
    local exec_time=$(echo "$result" | grep -oP 'Execution Time: \K[\d.]+')
    if [ -n "$exec_time" ]; then
        log_success "Nearest neighbor query: ${exec_time}ms"

        # Calculate throughput
        local throughput=$(echo "scale=2; 10000 / $exec_time * 1000" | bc)
        log_info "Throughput: ~${throughput} distance calculations/second"
    else
        log_info "Benchmark result:"
        echo "$result" | grep -E "(Execution Time|Planning Time|Seq Scan)"
    fi

    # Cleanup
    psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$TEST_DB" -c "
        DROP TABLE IF EXISTS bench_vec;
    " >/dev/null 2>&1
}

cleanup_tests() {
    log_info "Cleaning up test artifacts..."

    psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$TEST_DB" -c "
        DROP TABLE IF EXISTS test_vec CASCADE;
        DROP TABLE IF EXISTS test_vec_dim CASCADE;
        DROP TABLE IF EXISTS bench_vec CASCADE;
    " >/dev/null 2>&1

    if [ "$TEMP_DB" = true ]; then
        log_info "Dropping temporary database: $TEST_DB"
        dropdb -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" "$TEST_DB" 2>/dev/null || true
    fi
}

# ============================================================================
# Main
# ============================================================================

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --database) TEST_DB="$2"; shift 2 ;;
            --host) PG_HOST="$2"; shift 2 ;;
            --port) PG_PORT="$2"; shift 2 ;;
            --user) PG_USER="$2"; shift 2 ;;
            --verbose) VERBOSE=true; shift ;;
            --benchmark) BENCHMARK=true; shift ;;
            --cleanup) CLEANUP=true; shift ;;
            --help)
                echo "Usage: $0 [OPTIONS]"
                echo "Options:"
                echo "  --database DB    Database to use for testing"
                echo "  --host HOST      PostgreSQL host (default: localhost)"
                echo "  --port PORT      PostgreSQL port (default: 5432)"
                echo "  --user USER      PostgreSQL user (default: postgres)"
                echo "  --verbose        Show detailed output"
                echo "  --benchmark      Run performance benchmarks"
                echo "  --cleanup        Clean up test artifacts"
                exit 0
                ;;
            *) echo "Unknown option: $1"; exit 1 ;;
        esac
    done
}

main() {
    parse_args "$@"

    echo ""
    echo "╔═══════════════════════════════════════════════════════════════╗"
    echo "║        RuVector Installation Verification Suite               ║"
    echo "╚═══════════════════════════════════════════════════════════════╝"
    echo ""

    # Create temp database if needed
    if [ -z "$TEST_DB" ]; then
        TEST_DB="ruvector_verify_$$"
        TEMP_DB=true
        log_info "Creating temporary database: $TEST_DB"
        createdb -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" "$TEST_DB" || {
            log_fail "Could not create test database"
            exit 1
        }
    fi

    # Set trap for cleanup
    trap cleanup_tests EXIT

    # Run test suites
    test_extension_load
    test_type_creation
    test_vector_io
    test_distance_functions
    test_vector_functions
    test_vector_arithmetic
    test_aggregate_operations
    test_temporal_functions
    test_attention_functions
    test_graph_functions
    test_error_handling

    if [ "$BENCHMARK" = true ]; then
        run_benchmarks
    fi

    # Summary
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "                    TEST SUMMARY"
    echo "═══════════════════════════════════════════════════════════════"
    echo -e "  Passed:  ${GREEN}${TESTS_PASSED}${NC}"
    echo -e "  Failed:  ${RED}${TESTS_FAILED}${NC}"
    echo -e "  Skipped: ${YELLOW}${TESTS_SKIPPED}${NC}"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""

    if [ "$TESTS_FAILED" -gt 0 ]; then
        log_fail "Some tests failed!"
        exit 1
    else
        log_success "All tests passed!"
        exit 0
    fi
}

main "$@"
