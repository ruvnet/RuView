#!/bin/bash
################################################################################
# Test script for deploy.sh
#
# This script validates the deployment script without actually publishing
# anything. It runs through all deployment steps in dry-run mode and checks
# for common issues.
#
# Usage: ./scripts/test-deploy.sh
################################################################################

set -euo pipefail

readonly GREEN='\033[0;32m'
readonly RED='\033[0;31m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m'

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo -e "${BLUE}╔═══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║         Testing RuVector Deployment Script                   ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Test counter
tests_passed=0
tests_failed=0

test_step() {
    local description="$1"
    echo -e "${BLUE}Testing:${NC} $description"
}

test_pass() {
    echo -e "${GREEN}✓ PASS${NC}"
    ((tests_passed++))
    echo ""
}

test_fail() {
    local reason="$1"
    echo -e "${RED}✗ FAIL: $reason${NC}"
    ((tests_failed++))
    echo ""
}

# Test 1: Script exists and is executable
test_step "Deployment script exists and is executable"
if [[ -x "$SCRIPT_DIR/deploy.sh" ]]; then
    test_pass
else
    test_fail "deploy.sh is not executable or doesn't exist"
fi

# Test 2: Required tools
test_step "Required tools are installed"
missing_tools=()
for tool in cargo rustc npm node wasm-pack jq; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        missing_tools+=("$tool")
    fi
done

if [[ ${#missing_tools[@]} -eq 0 ]]; then
    test_pass
else
    test_fail "Missing tools: ${missing_tools[*]}"
fi

# Test 3: Help message
test_step "Help message displays correctly"
if "$SCRIPT_DIR/deploy.sh" --help >/dev/null 2>&1; then
    test_pass
else
    test_fail "Help message not working"
fi

# Test 4: Workspace Cargo.toml exists
test_step "Workspace Cargo.toml exists"
if [[ -f "$PROJECT_ROOT/Cargo.toml" ]]; then
    test_pass
else
    test_fail "Cargo.toml not found"
fi

# Test 5: Version can be extracted
test_step "Version extraction from Cargo.toml"
cd "$PROJECT_ROOT"
version=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/' || echo "")
if [[ -n "$version" ]]; then
    echo "  Found version: $version"
    test_pass
else
    test_fail "Could not extract version"
fi

# Test 6: Package.json files exist
test_step "NPM package.json files exist"
package_count=0
for pkg in crates/ruvector-node crates/ruvector-wasm crates/ruvector-gnn-node; do
    if [[ -f "$PROJECT_ROOT/$pkg/package.json" ]]; then
        ((package_count++))
    fi
done

if [[ $package_count -gt 0 ]]; then
    echo "  Found $package_count package.json files"
    test_pass
else
    test_fail "No package.json files found"
fi

# Test 7: Crate directories exist
test_step "Crate directories exist"
crate_count=0
for crate in crates/ruvector-core crates/ruvector-node crates/ruvector-graph; do
    if [[ -d "$PROJECT_ROOT/$crate" ]]; then
        ((crate_count++))
    fi
done

if [[ $crate_count -gt 0 ]]; then
    echo "  Found $crate_count crate directories"
    test_pass
else
    test_fail "No crate directories found"
fi

# Test 8: Dry run without credentials (should work)
test_step "Dry run without credentials"
cd "$PROJECT_ROOT"
if PUBLISH_CRATES=false PUBLISH_NPM=false "$SCRIPT_DIR/deploy.sh" --dry-run --skip-tests --skip-checks --force 2>&1 | grep -q "Deployment completed successfully"; then
    test_pass
else
    test_fail "Dry run failed even with skips"
fi

# Test 9: Check logging directory creation
test_step "Log directory creation"
if [[ -d "$PROJECT_ROOT/logs/deployment" ]]; then
    log_count=$(find "$PROJECT_ROOT/logs/deployment" -name "deploy-*.log" 2>/dev/null | wc -l)
    echo "  Found $log_count deployment logs"
    test_pass
else
    test_fail "Log directory not created"
fi

# Test 10: Version flag works
test_step "Version flag parsing"
cd "$PROJECT_ROOT"
if PUBLISH_CRATES=false PUBLISH_NPM=false "$SCRIPT_DIR/deploy.sh" --version 9.9.9 --dry-run --skip-tests --skip-checks --force 2>&1 | grep -q "9.9.9"; then
    test_pass
else
    test_fail "Version flag not working"
fi

# Test 11: JSON manipulation with jq
test_step "Version synchronization (jq test)"
temp_json=$(mktemp)
echo '{"version":"0.0.0"}' > "$temp_json"
jq --arg version "1.2.3" '.version = $version' "$temp_json" > "${temp_json}.new"
mv "${temp_json}.new" "$temp_json"
result=$(jq -r '.version' "$temp_json")
rm "$temp_json"

if [[ "$result" == "1.2.3" ]]; then
    test_pass
else
    test_fail "jq version update failed"
fi

# Test 12: Build scripts exist for WASM packages
test_step "WASM build scripts exist"
wasm_build_count=0
for pkg in crates/ruvector-wasm crates/ruvector-gnn-wasm; do
    if [[ -f "$PROJECT_ROOT/$pkg/build.sh" ]] || [[ -f "$PROJECT_ROOT/$pkg/package.json" ]]; then
        ((wasm_build_count++))
    fi
done

if [[ $wasm_build_count -gt 0 ]]; then
    echo "  Found build scripts for $wasm_build_count WASM packages"
    test_pass
else
    test_fail "No WASM build scripts found"
fi

# Test 13: Dependency order validation
test_step "Crate dependency order validation"
# Check that core comes before node
deploy_script_content=$(cat "$SCRIPT_DIR/deploy.sh")
core_line=$(echo "$deploy_script_content" | grep -n "ruvector-core" | head -1 | cut -d: -f1)
node_line=$(echo "$deploy_script_content" | grep -n "ruvector-node" | grep -v "gnn-node" | head -1 | cut -d: -f1)

if [[ -n "$core_line" ]] && [[ -n "$node_line" ]] && [[ $core_line -lt $node_line ]]; then
    echo "  Dependency order is correct (core before bindings)"
    test_pass
else
    test_fail "Dependency order may be incorrect"
fi

# Summary
echo ""
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}                     Test Summary                              ${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo ""

total_tests=$((tests_passed + tests_failed))
echo -e "Total tests: $total_tests"
echo -e "${GREEN}Passed: $tests_passed${NC}"

if [[ $tests_failed -gt 0 ]]; then
    echo -e "${RED}Failed: $tests_failed${NC}"
    echo ""
    echo -e "${RED}Some tests failed. Please review the output above.${NC}"
    exit 1
else
    echo -e "${RED}Failed: $tests_failed${NC}"
    echo ""
    echo -e "${GREEN}All tests passed! The deployment script is ready to use.${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Set required environment variables:"
    echo "     export CRATES_API_KEY='your-token'"
    echo "     export NPM_TOKEN='your-token'"
    echo ""
    echo "  2. Test with dry run:"
    echo "     ./scripts/deploy.sh --dry-run"
    echo ""
    echo "  3. Deploy:"
    echo "     ./scripts/deploy.sh"
    exit 0
fi
