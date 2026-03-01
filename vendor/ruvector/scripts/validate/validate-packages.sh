#!/bin/bash
# Pre-publish validation script for ruvector packages

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

echo "🔍 Validating ruvector packages for npm publishing..."
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Validation counters
PASSED=0
FAILED=0
WARNINGS=0

# Helper functions
pass() {
  echo -e "${GREEN}✓${NC} $1"
  ((PASSED++))
}

fail() {
  echo -e "${RED}✗${NC} $1"
  ((FAILED++))
}

warn() {
  echo -e "${YELLOW}⚠${NC} $1"
  ((WARNINGS++))
}

section() {
  echo ""
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo "  $1"
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
}

# Package validation function
validate_package() {
  local PKG_DIR="$1"
  local PKG_NAME="$2"

  section "Validating: $PKG_NAME"

  cd "$PKG_DIR"

  # Check package.json exists
  if [ -f "package.json" ]; then
    pass "package.json exists"
  else
    fail "package.json missing"
    return 1
  fi

  # Check required fields in package.json
  local name=$(jq -r '.name' package.json)
  local version=$(jq -r '.version' package.json)
  local description=$(jq -r '.description' package.json)
  local license=$(jq -r '.license' package.json)
  local repository=$(jq -r '.repository.url' package.json)

  [ "$name" != "null" ] && pass "name: $name" || fail "name missing"
  [ "$version" != "null" ] && pass "version: $version" || fail "version missing"
  [ "$description" != "null" ] && pass "description exists" || fail "description missing"
  [ "$license" != "null" ] && pass "license: $license" || fail "license missing"
  [ "$repository" != "null" ] && pass "repository URL set" || warn "repository URL missing"

  # Check README
  if [ -f "README.md" ]; then
    local readme_size=$(wc -c < README.md)
    if [ "$readme_size" -gt 500 ]; then
      pass "README.md exists ($(echo $readme_size | numfmt --to=iec-i --suffix=B))"
    else
      warn "README.md exists but seems short (${readme_size} bytes)"
    fi
  else
    fail "README.md missing"
  fi

  # Check LICENSE
  if [ -f "LICENSE" ]; then
    pass "LICENSE exists"
  else
    warn "LICENSE missing"
  fi

  # Check .npmignore
  if [ -f ".npmignore" ]; then
    pass ".npmignore exists"
  else
    warn ".npmignore missing (npm will use .gitignore)"
  fi

  # Check TypeScript configuration
  if [ -f "tsconfig.json" ]; then
    pass "tsconfig.json exists"
  else
    warn "tsconfig.json missing"
  fi

  # Check source directory
  if [ -d "src" ]; then
    local src_files=$(find src -name "*.ts" -type f | wc -l)
    pass "src/ directory exists ($src_files TypeScript files)"
  else
    fail "src/ directory missing"
  fi

  # Check if dependencies are installed
  if [ -d "node_modules" ]; then
    pass "node_modules exists (dependencies installed)"
  else
    warn "node_modules missing - run npm install"
  fi

  # Validate package scripts
  local has_build=$(jq -r '.scripts.build' package.json)
  [ "$has_build" != "null" ] && pass "build script defined" || warn "build script missing"

  # Check for bin (CLI packages)
  local has_bin=$(jq -r '.bin' package.json)
  if [ "$has_bin" != "null" ]; then
    pass "bin entry defined (CLI package)"

    # Validate bin files exist
    local bin_file=$(jq -r '.bin | if type=="object" then .[keys[0]] else . end' package.json)
    if [ -f "$bin_file" ]; then
      pass "bin file exists: $bin_file"

      # Check shebang
      if head -1 "$bin_file" | grep -q "^#!/usr/bin/env node"; then
        pass "bin file has correct shebang"
      else
        fail "bin file missing shebang: #!/usr/bin/env node"
      fi

      # Check executable permission
      if [ -x "$bin_file" ]; then
        pass "bin file is executable"
      else
        warn "bin file not executable - will be fixed by npm"
      fi
    else
      fail "bin file missing: $bin_file"
    fi
  fi

  # Check publishConfig
  local publish_access=$(jq -r '.publishConfig.access' package.json)
  [ "$publish_access" == "public" ] && pass "publishConfig.access: public" || warn "publishConfig.access not set to public (scoped packages need this)"

  # Validate files field
  local files=$(jq -r '.files' package.json)
  if [ "$files" != "null" ]; then
    pass "files field defined"

    # Check if listed files exist
    jq -r '.files[]' package.json | while read -r file; do
      if [ -e "$file" ] || [ "$file" == "dist" ]; then
        pass "  - $file exists (or will be created by build)"
      else
        warn "  - $file listed but not found"
      fi
    done
  else
    warn "files field not defined (npm will include everything not in .npmignore)"
  fi

  cd "$ROOT_DIR"
}

# Main validation
cd "$ROOT_DIR"

# Validate psycho-symbolic-integration
validate_package "$ROOT_DIR/packages/psycho-symbolic-integration" "@ruvector/psycho-symbolic-integration"

# Validate psycho-synth-examples
validate_package "$ROOT_DIR/packages/psycho-synth-examples" "@ruvector/psycho-synth-examples"

# Test CLI functionality
section "Testing CLI Functionality"

cd "$ROOT_DIR/packages/psycho-synth-examples"
if node bin/cli.js list > /dev/null 2>&1; then
  pass "CLI 'list' command works"
else
  fail "CLI 'list' command failed"
fi

cd "$ROOT_DIR"

# Summary
section "Validation Summary"
echo ""
echo -e "${GREEN}Passed:${NC}   $PASSED"
echo -e "${YELLOW}Warnings:${NC} $WARNINGS"
echo -e "${RED}Failed:${NC}   $FAILED"
echo ""

if [ $FAILED -gt 0 ]; then
  echo -e "${RED}❌ Validation failed with $FAILED errors${NC}"
  echo "Please fix the errors before publishing."
  exit 1
elif [ $WARNINGS -gt 0 ]; then
  echo -e "${YELLOW}⚠️  Validation passed with $WARNINGS warnings${NC}"
  echo "Consider addressing warnings before publishing."
  exit 0
else
  echo -e "${GREEN}✅ All validations passed!${NC}"
  echo "Packages are ready for publishing."
  exit 0
fi
