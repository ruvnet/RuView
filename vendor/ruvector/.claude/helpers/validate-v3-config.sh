#!/bin/bash
# V3 Configuration Validation Script
# Ensures all V3 development dependencies and configurations are properly set up

set -e

echo "🔍 Claude Flow V3 Configuration Validation"
echo "==========================================="
echo ""

ERRORS=0
WARNINGS=0

# Color codes
RED='\033[0;31m'
YELLOW='\033[0;33m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RESET='\033[0m'

# Helper functions
log_error() {
  echo -e "${RED}❌ ERROR: $1${RESET}"
  ((ERRORS++))
}

log_warning() {
  echo -e "${YELLOW}⚠️  WARNING: $1${RESET}"
  ((WARNINGS++))
}

log_success() {
  echo -e "${GREEN}✅ $1${RESET}"
}

log_info() {
  echo -e "${BLUE}ℹ️  $1${RESET}"
}

# Check 1: Required directories
echo "📁 Checking Directory Structure..."
required_dirs=(
  ".claude"
  ".claude/helpers"
  ".claude-flow/metrics"
  ".claude-flow/security"
  "src"
  "src/domains"
)

for dir in "${required_dirs[@]}"; do
  if [ -d "$dir" ]; then
    log_success "Directory exists: $dir"
  else
    log_error "Missing required directory: $dir"
  fi
done

# Check 2: Required files
echo ""
echo "📄 Checking Required Files..."
required_files=(
  ".claude/settings.json"
  ".claude/statusline.sh"
  ".claude/helpers/update-v3-progress.sh"
  ".claude-flow/metrics/v3-progress.json"
  ".claude-flow/metrics/performance.json"
  ".claude-flow/security/audit-status.json"
  "package.json"
)

for file in "${required_files[@]}"; do
  if [ -f "$file" ]; then
    log_success "File exists: $file"

    # Additional checks for specific files
    case "$file" in
      "package.json")
        if grep -q "agentic-flow.*alpha" "$file" 2>/dev/null; then
          log_success "agentic-flow@alpha dependency found"
        else
          log_warning "agentic-flow@alpha dependency not found in package.json"
        fi
        ;;
      ".claude/helpers/update-v3-progress.sh")
        if [ -x "$file" ]; then
          log_success "Helper script is executable"
        else
          log_error "Helper script is not executable: $file"
        fi
        ;;
      ".claude-flow/metrics/v3-progress.json")
        if jq empty "$file" 2>/dev/null; then
          log_success "V3 progress JSON is valid"
          domains=$(jq -r '.domains.total // "unknown"' "$file" 2>/dev/null)
          agents=$(jq -r '.swarm.totalAgents // "unknown"' "$file" 2>/dev/null)
          log_info "Configured for $domains domains, $agents agents"
        else
          log_error "Invalid JSON in v3-progress.json"
        fi
        ;;
    esac
  else
    log_error "Missing required file: $file"
  fi
done

# Check 3: Domain structure
echo ""
echo "🏗️ Checking Domain Structure..."
expected_domains=("task-management" "session-management" "health-monitoring" "lifecycle-management" "event-coordination")

for domain in "${expected_domains[@]}"; do
  domain_path="src/domains/$domain"
  if [ -d "$domain_path" ]; then
    log_success "Domain directory exists: $domain"
  else
    log_warning "Domain directory missing: $domain (will be created during development)"
  fi
done

# Check 4: Git configuration
echo ""
echo "🔀 Checking Git Configuration..."
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  log_success "Git repository detected"

  current_branch=$(git branch --show-current 2>/dev/null || echo "unknown")
  log_info "Current branch: $current_branch"

  if [ "$current_branch" = "v3" ]; then
    log_success "On V3 development branch"
  else
    log_warning "Not on V3 branch (current: $current_branch)"
  fi
else
  log_error "Not in a Git repository"
fi

# Check 5: Node.js and npm
echo ""
echo "📦 Checking Node.js Environment..."
if command -v node >/dev/null 2>&1; then
  node_version=$(node --version)
  log_success "Node.js installed: $node_version"

  # Check if Node.js version is 20+
  node_major=$(echo "$node_version" | cut -d'.' -f1 | sed 's/v//')
  if [ "$node_major" -ge 20 ]; then
    log_success "Node.js version meets requirements (≥20.0.0)"
  else
    log_error "Node.js version too old. Required: ≥20.0.0, Found: $node_version"
  fi
else
  log_error "Node.js not installed"
fi

if command -v npm >/dev/null 2>&1; then
  npm_version=$(npm --version)
  log_success "npm installed: $npm_version"
else
  log_error "npm not installed"
fi

# Check 6: Development tools
echo ""
echo "🔧 Checking Development Tools..."
dev_tools=("jq" "git")

for tool in "${dev_tools[@]}"; do
  if command -v "$tool" >/dev/null 2>&1; then
    tool_version=$($tool --version 2>/dev/null | head -n1 || echo "unknown")
    log_success "$tool installed: $tool_version"
  else
    log_error "$tool not installed"
  fi
done

# Check 7: Permissions
echo ""
echo "🔐 Checking Permissions..."
test_files=(
  ".claude/statusline.sh"
  ".claude/helpers/update-v3-progress.sh"
)

for file in "${test_files[@]}"; do
  if [ -f "$file" ]; then
    if [ -x "$file" ]; then
      log_success "Executable permissions: $file"
    else
      log_warning "Missing executable permissions: $file"
      log_info "Run: chmod +x $file"
    fi
  fi
done

# Summary
echo ""
echo "📊 Validation Summary"
echo "===================="
if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
  log_success "All checks passed! V3 development environment is ready."
  exit 0
elif [ $ERRORS -eq 0 ]; then
  echo -e "${YELLOW}⚠️  $WARNINGS warnings found, but no critical errors.${RESET}"
  log_info "V3 development can proceed with minor issues to address."
  exit 0
else
  echo -e "${RED}❌ $ERRORS critical errors found.${RESET}"
  if [ $WARNINGS -gt 0 ]; then
    echo -e "${YELLOW}⚠️  $WARNINGS warnings also found.${RESET}"
  fi
  log_error "Please fix critical errors before proceeding with V3 development."
  exit 1
fi