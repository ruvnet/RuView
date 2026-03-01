#!/bin/bash
# Pre-publish validation script for ruvector packages (without jq dependency)

set -e

echo "🔍 Validating ruvector packages for npm publishing..."
echo ""

PASSED=0
FAILED=0
WARNINGS=0

pass() { echo "✓ $1"; ((PASSED++)); }
fail() { echo "✗ $1"; ((FAILED++)); }
warn() { echo "⚠ $1"; ((WARNINGS++)); }

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  @ruvector/psycho-symbolic-integration"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

cd packages/psycho-symbolic-integration

[ -f "package.json" ] && pass "package.json exists" || fail "package.json missing"
[ -f "README.md" ] && pass "README.md exists" || fail "README.md missing"
[ -f "LICENSE" ] && pass "LICENSE exists" || warn "LICENSE missing"
[ -f ".npmignore" ] && pass ".npmignore exists" || warn ".npmignore missing"
[ -f "tsconfig.json" ] && pass "tsconfig.json exists" || warn "tsconfig.json missing"
[ -d "src" ] && pass "src/ directory exists" || fail "src/ directory missing"
[ -d "node_modules" ] && pass "dependencies installed" || warn "run npm install first"

grep -q '"name":' package.json && pass "name field exists" || fail "name field missing"
grep -q '"version":' package.json && pass "version field exists" || fail "version field missing"
grep -q '"description":' package.json && pass "description field exists" || fail "description field missing"
grep -q '"repository":' package.json && pass "repository field exists" || warn "repository field missing"
grep -q '"publishConfig":' package.json && pass "publishConfig exists" || warn "publishConfig missing"

cd ../..

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  @ruvector/psycho-synth-examples"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

cd packages/psycho-synth-examples

[ -f "package.json" ] && pass "package.json exists" || fail "package.json missing"
[ -f "README.md" ] && pass "README.md exists" || fail "README.md missing"
[ -f "LICENSE" ] && pass "LICENSE exists" || warn "LICENSE missing"
[ -f ".npmignore" ] && pass ".npmignore exists" || warn ".npmignore missing"
[ -f "tsconfig.json" ] && pass "tsconfig.json exists" || warn "tsconfig.json missing"
[ -d "src" ] && pass "src/ directory exists" || fail "src/ directory missing"
[ -d "bin" ] && pass "bin/ directory exists" || fail "bin/ directory missing"
[ -d "examples" ] && pass "examples/ directory exists" || fail "examples/ directory missing"
[ -d "node_modules" ] && pass "dependencies installed" || warn "run npm install first"

[ -f "bin/cli.js" ] && pass "CLI file exists" || fail "CLI file missing"
[ -x "bin/cli.js" ] && pass "CLI is executable" || warn "CLI not executable"

if head -1 bin/cli.js | grep -q "^#!/usr/bin/env node"; then
  pass "CLI has correct shebang"
else
  fail "CLI missing shebang"
fi

grep -q '"name":' package.json && pass "name field exists" || fail "name field missing"
grep -q '"version":' package.json && pass "version field exists" || fail "version field missing"
grep -q '"bin":' package.json && pass "bin field exists" || fail "bin field missing"
grep -q '"repository":' package.json && pass "repository field exists" || warn "repository field missing"
grep -q '"publishConfig":' package.json && pass "publishConfig exists" || warn "publishConfig missing"

# Test CLI
echo ""
if node bin/cli.js list > /dev/null 2>&1; then
  pass "CLI 'list' command works"
else
  fail "CLI 'list' command failed"
fi

cd ../..

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Summary"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Passed:   $PASSED"
echo "Warnings: $WARNINGS"
echo "Failed:   $FAILED"
echo ""

if [ $FAILED -gt 0 ]; then
  echo "❌ Validation failed with $FAILED errors"
  exit 1
elif [ $WARNINGS -gt 0 ]; then
  echo "⚠️  Validation passed with $WARNINGS warnings"
  exit 0
else
  echo "✅ All validations passed!"
  exit 0
fi
