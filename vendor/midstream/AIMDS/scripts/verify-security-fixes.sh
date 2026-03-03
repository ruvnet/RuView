#!/bin/bash
# AIMDS Security Verification Script
# Run this after applying security fixes to verify compliance

set -e

echo "================================================================================"
echo "AIMDS Security Verification"
echo "================================================================================"
echo ""

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR"

PASSED=0
FAILED=0
WARNINGS=0

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

check_pass() {
    echo -e "${GREEN}✅ PASS${NC}: $1"
    ((PASSED++))
}

check_fail() {
    echo -e "${RED}❌ FAIL${NC}: $1"
    ((FAILED++))
}

check_warn() {
    echo -e "${YELLOW}⚠️  WARN${NC}: $1"
    ((WARNINGS++))
}

echo "================================================================================"
echo "1. CHECKING FOR HARDCODED SECRETS"
echo "================================================================================"
echo ""

# Check if .env exists
if [ -f ".env" ]; then
    check_warn ".env file exists (should not be in git)"

    # Check if .env contains real secrets
    if grep -q "sk-" .env 2>/dev/null; then
        check_fail "Found API keys in .env file"
    else
        check_pass "No obvious API keys in .env"
    fi
else
    check_pass ".env file not found (good)"
fi

# Check git status
if git ls-files --error-unmatch .env 2>/dev/null; then
    check_fail ".env is tracked in git - MUST REMOVE"
else
    check_pass ".env is not tracked in git"
fi

# Check .gitignore
if grep -q "^\.env$" .gitignore 2>/dev/null; then
    check_pass ".env is in .gitignore"
else
    check_fail ".env NOT in .gitignore"
fi

# Check for hardcoded secrets in source code
echo ""
echo "Checking source code for hardcoded secrets..."
SECRET_PATTERNS="sk-|AKIA|ghp_|xox[baprs]-|AIza"
if grep -rn "$SECRET_PATTERNS" src/ crates/ 2>/dev/null | grep -v ".md:" | grep -v "test" | grep -v "example"; then
    check_fail "Found potential secrets in source code"
else
    check_pass "No obvious secrets in source code"
fi

echo ""
echo "================================================================================"
echo "2. CHECKING COMPILATION"
echo "================================================================================"
echo ""

# Check Rust compilation
echo "Compiling Rust crates..."
if cargo build --release --quiet 2>&1 | grep -q "error"; then
    check_fail "Rust compilation failed"
    cargo build 2>&1 | grep "error" | head -5
else
    check_pass "Rust compilation successful"
fi

# Check for clippy warnings
echo ""
echo "Running clippy..."
CLIPPY_OUTPUT=$(cargo clippy --all-targets --all-features -- -D warnings 2>&1)
if echo "$CLIPPY_OUTPUT" | grep -q "error"; then
    check_fail "Clippy found errors"
    echo "$CLIPPY_OUTPUT" | grep "error" | head -5
else
    check_pass "Clippy check passed"
fi

echo ""
echo "================================================================================"
echo "3. CHECKING DEPENDENCIES"
echo "================================================================================"
echo ""

# NPM audit
echo "Running npm audit..."
if [ -f "package.json" ]; then
    NPM_AUDIT=$(npm audit --json 2>/dev/null || echo "{}")
    VULNERABILITIES=$(echo "$NPM_AUDIT" | jq -r '.metadata.vulnerabilities.total // 0' 2>/dev/null || echo "0")
    CRITICAL=$(echo "$NPM_AUDIT" | jq -r '.metadata.vulnerabilities.critical // 0' 2>/dev/null || echo "0")
    HIGH=$(echo "$NPM_AUDIT" | jq -r '.metadata.vulnerabilities.high // 0' 2>/dev/null || echo "0")

    if [ "$CRITICAL" -gt 0 ] || [ "$HIGH" -gt 0 ]; then
        check_fail "Found $CRITICAL critical, $HIGH high vulnerabilities"
    elif [ "$VULNERABILITIES" -gt 0 ]; then
        check_warn "Found $VULNERABILITIES moderate/low vulnerabilities"
    else
        check_pass "No npm vulnerabilities found"
    fi
fi

# Cargo audit (if installed)
echo ""
echo "Checking cargo dependencies..."
if command -v cargo-audit &> /dev/null; then
    if cargo audit 2>&1 | grep -q "error"; then
        check_fail "Cargo audit found vulnerabilities"
    else
        check_pass "No cargo vulnerabilities found"
    fi
else
    check_warn "cargo-audit not installed (run: cargo install cargo-audit)"
fi

echo ""
echo "================================================================================"
echo "4. CHECKING SECURITY CONFIGURATION"
echo "================================================================================"
echo ""

# Check for TLS configuration
if grep -q "https.createServer" src/gateway/server.ts; then
    check_pass "HTTPS configuration found"
else
    check_fail "No HTTPS configuration found"
fi

# Check for authentication middleware
if grep -q "authMiddleware\|authenticate\|verifyApiKey" src/gateway/server.ts; then
    check_pass "Authentication middleware found"
else
    check_fail "No authentication middleware found"
fi

# Check for proper CORS config
if grep -q "cors({" src/gateway/server.ts; then
    check_pass "CORS configuration found"
else
    check_warn "CORS not configured (using defaults)"
fi

# Check for rate limiting
if grep -q "rateLimit" src/gateway/server.ts; then
    check_pass "Rate limiting configured"
else
    check_fail "Rate limiting not found"
fi

# Check for helmet
if grep -q "helmet" src/gateway/server.ts; then
    check_pass "Helmet security headers enabled"
else
    check_fail "Helmet not configured"
fi

echo ""
echo "================================================================================"
echo "5. RUNNING TESTS"
echo "================================================================================"
echo ""

# Rust tests
echo "Running Rust tests..."
if cargo test --quiet 2>&1 | grep -q "FAILED"; then
    check_fail "Rust tests failed"
else
    check_pass "Rust tests passed"
fi

# TypeScript tests
echo ""
echo "Running TypeScript tests..."
if [ -f "package.json" ]; then
    if npm test 2>&1 | grep -q "FAIL"; then
        check_fail "TypeScript tests failed"
    else
        check_pass "TypeScript tests passed"
    fi
fi

echo ""
echo "================================================================================"
echo "6. CHECKING CODE QUALITY"
echo "================================================================================"
echo ""

# Check for mock implementations
if grep -rn "Hash-based embedding for demo\|TODO:\|FIXME:\|HACK:" src/ crates/ | grep -v ".md:"; then
    check_warn "Found TODOs/FIXMEs or mock implementations"
else
    check_pass "No obvious mock implementations or TODOs"
fi

# Check for proper error handling
if grep -q "\.expect(\|\.unwrap(" crates/*/src/*.rs; then
    check_warn "Found .expect()/.unwrap() calls (consider proper error handling)"
else
    check_pass "No .expect()/.unwrap() calls found"
fi

echo ""
echo "================================================================================"
echo "FINAL SCORE"
echo "================================================================================"
echo ""

TOTAL=$((PASSED + FAILED + WARNINGS))
SCORE=$(( (PASSED * 100) / TOTAL ))

echo -e "Passed:   ${GREEN}$PASSED${NC}"
echo -e "Failed:   ${RED}$FAILED${NC}"
echo -e "Warnings: ${YELLOW}$WARNINGS${NC}"
echo ""
echo -e "Security Score: ${SCORE}/100"
echo ""

if [ $FAILED -eq 0 ] && [ $SCORE -ge 80 ]; then
    echo -e "${GREEN}✅ READY FOR PRODUCTION DEPLOYMENT${NC}"
    exit 0
elif [ $FAILED -eq 0 ]; then
    echo -e "${YELLOW}⚠️  ACCEPTABLE - Some improvements needed${NC}"
    exit 0
else
    echo -e "${RED}❌ NOT READY - Critical issues must be fixed${NC}"
    echo ""
    echo "See SECURITY_AUDIT_REPORT.md for detailed findings"
    echo "See CRITICAL_FIXES_REQUIRED.md for fix instructions"
    exit 1
fi
