#!/usr/bin/env bash
# RuVector-Postgres Test Runner Script
# Runs pgrx tests and outputs JUnit XML for CI integration

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Configuration
PG_VERSION="${PG_VERSION:-17}"
TEST_RESULTS_DIR="${TEST_RESULTS_DIR:-/test-results}"
JUNIT_OUTPUT="${JUNIT_OUTPUT:-${TEST_RESULTS_DIR}/junit.xml}"
TEST_LOG="${TEST_RESULTS_DIR}/test.log"

# Ensure test results directory exists
mkdir -p "${TEST_RESULTS_DIR}"

log_info "RuVector-Postgres Test Runner"
log_info "PostgreSQL Version: ${PG_VERSION}"
log_info "Test Results Directory: ${TEST_RESULTS_DIR}"

# Navigate to the crate directory
cd /app/crates/ruvector-postgres 2>/dev/null || cd /app

# Check if we have the source code
if [ ! -f "Cargo.toml" ]; then
    log_error "Cargo.toml not found. Mount the source code to /app"
    exit 1
fi

# Run pgrx tests with JSON output for conversion to JUnit
log_info "Running pgrx tests for pg${PG_VERSION}..."

# Start test execution timestamp
START_TIME=$(date +%s)

# Run cargo test with JSON output and capture result
set +e
cargo test --features pg${PG_VERSION} --no-fail-fast -- -Z unstable-options --format json 2>&1 | tee "${TEST_LOG}.json"
TEST_EXIT_CODE=${PIPESTATUS[0]}
set -e

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

log_info "Test execution completed in ${DURATION}s"

# Convert JSON output to JUnit XML
if command -v cargo2junit &> /dev/null; then
    log_info "Converting test results to JUnit XML..."
    cat "${TEST_LOG}.json" | cargo2junit > "${JUNIT_OUTPUT}" 2>/dev/null || true
else
    log_warn "cargo2junit not found, generating basic JUnit XML..."
    # Generate basic JUnit XML
    TESTS_RUN=$(grep -c '"type":"test"' "${TEST_LOG}.json" 2>/dev/null || echo "0")
    TESTS_FAILED=$(grep -c '"event":"failed"' "${TEST_LOG}.json" 2>/dev/null || echo "0")
    TESTS_PASSED=$((TESTS_RUN - TESTS_FAILED))

    cat > "${JUNIT_OUTPUT}" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="ruvector-postgres" tests="${TESTS_RUN}" failures="${TESTS_FAILED}" time="${DURATION}">
  <testsuite name="cargo-test" tests="${TESTS_RUN}" failures="${TESTS_FAILED}" time="${DURATION}">
    <testcase name="pgrx-tests" classname="ruvector_postgres" time="${DURATION}">
      $([ "${TEST_EXIT_CODE}" != "0" ] && echo "<failure message=\"Tests failed with exit code ${TEST_EXIT_CODE}\"/>" || true)
    </testcase>
  </testsuite>
</testsuites>
EOF
fi

# Run pgrx-specific tests if available
log_info "Running pgrx integration tests..."
set +e
cargo pgrx test pg${PG_VERSION} 2>&1 | tee -a "${TEST_LOG}"
PGRX_EXIT_CODE=$?
set -e

# Generate test summary
log_info "Generating test summary..."
cat > "${TEST_RESULTS_DIR}/summary.json" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "pg_version": "${PG_VERSION}",
  "duration_seconds": ${DURATION},
  "cargo_test_exit_code": ${TEST_EXIT_CODE},
  "pgrx_test_exit_code": ${PGRX_EXIT_CODE},
  "success": $([ "${TEST_EXIT_CODE}" == "0" ] && [ "${PGRX_EXIT_CODE}" == "0" ] && echo "true" || echo "false")
}
EOF

# Print summary
echo ""
echo "=========================================="
echo "         TEST SUMMARY"
echo "=========================================="
echo "PostgreSQL Version: ${PG_VERSION}"
echo "Duration: ${DURATION}s"
echo "Cargo Test Exit Code: ${TEST_EXIT_CODE}"
echo "PGRX Test Exit Code: ${PGRX_EXIT_CODE}"
echo "JUnit XML: ${JUNIT_OUTPUT}"
echo "=========================================="

if [ "${TEST_EXIT_CODE}" != "0" ] || [ "${PGRX_EXIT_CODE}" != "0" ]; then
    log_error "Tests failed!"
    exit 1
fi

log_success "All tests passed!"
exit 0
