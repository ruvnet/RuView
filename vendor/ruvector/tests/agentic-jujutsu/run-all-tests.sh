#!/bin/bash

###############################################################################
# Agentic-Jujutsu Test Runner
#
# Executes all test suites sequentially and generates comprehensive reports.
#
# Usage:
#   ./run-all-tests.sh [options]
#
# Options:
#   --verbose    Show detailed test output
#   --coverage   Generate coverage report
#   --bail       Stop on first failure
#   --watch      Watch mode for development
###############################################################################

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${TEST_DIR}/../.." && pwd)"
RESULTS_DIR="${TEST_DIR}/results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
RESULTS_FILE="${RESULTS_DIR}/test-results-${TIMESTAMP}.json"

# Parse command line arguments
VERBOSE=false
COVERAGE=false
BAIL=false
WATCH=false

for arg in "$@"; do
  case $arg in
    --verbose)
      VERBOSE=true
      shift
      ;;
    --coverage)
      COVERAGE=true
      shift
      ;;
    --bail)
      BAIL=true
      shift
      ;;
    --watch)
      WATCH=true
      shift
      ;;
    *)
      echo -e "${RED}Unknown option: $arg${NC}"
      exit 1
      ;;
  esac
done

# Create results directory
mkdir -p "${RESULTS_DIR}"

# Helper functions
print_header() {
  echo -e "\n${BLUE}================================${NC}"
  echo -e "${BLUE}$1${NC}"
  echo -e "${BLUE}================================${NC}\n"
}

print_success() {
  echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
  echo -e "${RED}✗ $1${NC}"
}

print_warning() {
  echo -e "${YELLOW}⚠ $1${NC}"
}

# Initialize results tracking
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
SKIPPED_TESTS=0
START_TIME=$(date +%s)

# Test suite results
declare -A SUITE_RESULTS
declare -A SUITE_DURATIONS

run_test_suite() {
  local suite_name=$1
  local test_file=$2

  print_header "Running $suite_name"

  local suite_start=$(date +%s)
  local suite_passed=true
  local test_output=""

  # Build test command
  local test_cmd="npx jest ${test_file}"

  if [ "$VERBOSE" = true ]; then
    test_cmd="$test_cmd --verbose"
  fi

  if [ "$COVERAGE" = true ]; then
    test_cmd="$test_cmd --coverage --coverageDirectory=${RESULTS_DIR}/coverage"
  fi

  if [ "$BAIL" = true ]; then
    test_cmd="$test_cmd --bail"
  fi

  # Run tests
  if [ "$VERBOSE" = true ]; then
    $test_cmd
    local exit_code=$?
  else
    test_output=$($test_cmd 2>&1)
    local exit_code=$?
  fi

  local suite_end=$(date +%s)
  local suite_duration=$((suite_end - suite_start))

  # Parse results
  if [ $exit_code -eq 0 ]; then
    print_success "$suite_name completed successfully"
    SUITE_RESULTS[$suite_name]="PASSED"
  else
    print_error "$suite_name failed"
    SUITE_RESULTS[$suite_name]="FAILED"
    suite_passed=false

    if [ "$VERBOSE" = false ]; then
      echo "$test_output"
    fi

    if [ "$BAIL" = true ]; then
      print_error "Stopping due to --bail flag"
      exit 1
    fi
  fi

  SUITE_DURATIONS[$suite_name]=$suite_duration
  echo -e "Duration: ${suite_duration}s\n"

  return $exit_code
}

# Main execution
print_header "Agentic-Jujutsu Test Suite"
echo "Project: ${PROJECT_ROOT}"
echo "Test Directory: ${TEST_DIR}"
echo "Results Directory: ${RESULTS_DIR}"
echo "Timestamp: ${TIMESTAMP}"
echo ""

# Check if Node.js and required packages are available
if ! command -v node &> /dev/null; then
  print_error "Node.js is not installed"
  exit 1
fi

if ! command -v npx &> /dev/null; then
  print_error "npx is not available"
  exit 1
fi

# Check if jest is available
if ! npx jest --version &> /dev/null; then
  print_warning "Jest is not installed. Installing test dependencies..."
  cd "${PROJECT_ROOT}" && npm install --save-dev jest @jest/globals @types/jest ts-jest
fi

# Run test suites
echo -e "${BLUE}Starting test execution...${NC}\n"

# 1. Integration Tests
if [ -f "${TEST_DIR}/integration-tests.ts" ]; then
  run_test_suite "Integration Tests" "${TEST_DIR}/integration-tests.ts"
  [ $? -eq 0 ] && ((PASSED_TESTS++)) || ((FAILED_TESTS++))
  ((TOTAL_TESTS++))
else
  print_warning "Integration tests not found: ${TEST_DIR}/integration-tests.ts"
fi

# 2. Performance Tests
if [ -f "${TEST_DIR}/performance-tests.ts" ]; then
  run_test_suite "Performance Tests" "${TEST_DIR}/performance-tests.ts"
  [ $? -eq 0 ] && ((PASSED_TESTS++)) || ((FAILED_TESTS++))
  ((TOTAL_TESTS++))
else
  print_warning "Performance tests not found: ${TEST_DIR}/performance-tests.ts"
fi

# 3. Validation Tests
if [ -f "${TEST_DIR}/validation-tests.ts" ]; then
  run_test_suite "Validation Tests" "${TEST_DIR}/validation-tests.ts"
  [ $? -eq 0 ] && ((PASSED_TESTS++)) || ((FAILED_TESTS++))
  ((TOTAL_TESTS++))
else
  print_warning "Validation tests not found: ${TEST_DIR}/validation-tests.ts"
fi

# Calculate final statistics
END_TIME=$(date +%s)
TOTAL_DURATION=$((END_TIME - START_TIME))

# Generate results report
print_header "Test Results Summary"

echo "Total Test Suites: ${TOTAL_TESTS}"
echo -e "Passed: ${GREEN}${PASSED_TESTS}${NC}"
echo -e "Failed: ${RED}${FAILED_TESTS}${NC}"
echo -e "Skipped: ${YELLOW}${SKIPPED_TESTS}${NC}"
echo "Total Duration: ${TOTAL_DURATION}s"
echo ""

# Detailed suite results
echo "Suite Results:"
for suite in "${!SUITE_RESULTS[@]}"; do
  status="${SUITE_RESULTS[$suite]}"
  duration="${SUITE_DURATIONS[$suite]}"

  if [ "$status" = "PASSED" ]; then
    echo -e "  ${GREEN}✓${NC} $suite (${duration}s)"
  else
    echo -e "  ${RED}✗${NC} $suite (${duration}s)"
  fi
done
echo ""

# Generate JSON results file
cat > "${RESULTS_FILE}" << EOF
{
  "timestamp": "${TIMESTAMP}",
  "summary": {
    "total": ${TOTAL_TESTS},
    "passed": ${PASSED_TESTS},
    "failed": ${FAILED_TESTS},
    "skipped": ${SKIPPED_TESTS},
    "duration": ${TOTAL_DURATION}
  },
  "suites": {
EOF

first=true
for suite in "${!SUITE_RESULTS[@]}"; do
  if [ "$first" = false ]; then
    echo "," >> "${RESULTS_FILE}"
  fi
  first=false

  status="${SUITE_RESULTS[$suite]}"
  duration="${SUITE_DURATIONS[$suite]}"

  cat >> "${RESULTS_FILE}" << EOF
    "${suite}": {
      "status": "${status}",
      "duration": ${duration}
    }
EOF
done

cat >> "${RESULTS_FILE}" << EOF

  }
}
EOF

print_success "Results saved to: ${RESULTS_FILE}"

# Generate coverage report link if coverage was enabled
if [ "$COVERAGE" = true ] && [ -d "${RESULTS_DIR}/coverage" ]; then
  print_success "Coverage report: ${RESULTS_DIR}/coverage/index.html"
fi

# Performance metrics
print_header "Performance Metrics"

if [ -f "${RESULTS_DIR}/performance-metrics.json" ]; then
  echo "Performance benchmarks available at: ${RESULTS_DIR}/performance-metrics.json"
else
  print_warning "No performance metrics generated"
fi

# Exit with appropriate code
if [ ${FAILED_TESTS} -gt 0 ]; then
  print_error "Tests failed!"
  exit 1
else
  print_success "All tests passed!"
  exit 0
fi
