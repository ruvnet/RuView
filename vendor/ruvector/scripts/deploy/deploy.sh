#!/bin/bash
################################################################################
# RuVector Comprehensive Deployment Script
#
# This script orchestrates the complete deployment process for ruvector:
# - Version management and synchronization
# - Pre-deployment checks (tests, linting, formatting)
# - WASM package builds
# - Crate publishing to crates.io
# - NPM package publishing
# - GitHub Actions trigger for cross-platform native builds
#
# Usage:
#   ./scripts/deploy.sh [OPTIONS]
#
# Options:
#   --dry-run           Run without actually publishing
#   --skip-tests        Skip test suite execution
#   --skip-crates       Skip crates.io publishing
#   --skip-npm          Skip NPM publishing
#   --skip-checks       Skip pre-deployment checks
#   --force             Skip confirmation prompts
#   --version VERSION   Set explicit version (otherwise read from Cargo.toml)
#
# Environment Variables:
#   CRATES_API_KEY      API key for crates.io (required for crate publishing)
#   NPM_TOKEN           NPM authentication token (required for npm publishing)
#   GITHUB_TOKEN        GitHub token for Actions API (optional)
#
################################################################################

set -euo pipefail

# Color codes for output
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly CYAN='\033[0;36m'
readonly BOLD='\033[1m'
readonly NC='\033[0m' # No Color

# Configuration (can be overridden by command-line flags)
DRY_RUN=${DRY_RUN:-false}
SKIP_TESTS=${SKIP_TESTS:-false}
SKIP_CHECKS=${SKIP_CHECKS:-false}
PUBLISH_CRATES=${PUBLISH_CRATES:-true}
PUBLISH_NPM=${PUBLISH_NPM:-true}
FORCE=${FORCE:-false}
VERSION=""

# Project root
readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Log files
readonly LOG_DIR="$PROJECT_ROOT/logs/deployment"
readonly LOG_FILE="$LOG_DIR/deploy-$(date +%Y%m%d-%H%M%S).log"

################################################################################
# Logging Functions
################################################################################

setup_logging() {
    mkdir -p "$LOG_DIR"
    exec 1> >(tee -a "$LOG_FILE")
    exec 2>&1
    echo -e "${CYAN}Logging to: $LOG_FILE${NC}"
}

log_info() {
    echo -e "${BLUE}[INFO]${NC} $*"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $*"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*" >&2
}

log_step() {
    echo ""
    echo -e "${BOLD}${CYAN}========================================${NC}"
    echo -e "${BOLD}${CYAN}$*${NC}"
    echo -e "${BOLD}${CYAN}========================================${NC}"
}

################################################################################
# Utility Functions
################################################################################

parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --dry-run)
                DRY_RUN=true
                log_warning "DRY RUN MODE: No actual publishing will occur"
                shift
                ;;
            --skip-tests)
                SKIP_TESTS=true
                log_warning "Skipping test suite"
                shift
                ;;
            --skip-crates)
                PUBLISH_CRATES=false
                log_info "Skipping crates.io publishing"
                shift
                ;;
            --skip-npm)
                PUBLISH_NPM=false
                log_info "Skipping NPM publishing"
                shift
                ;;
            --skip-checks)
                SKIP_CHECKS=true
                log_warning "Skipping pre-deployment checks"
                shift
                ;;
            --force)
                FORCE=true
                log_warning "Force mode: Skipping confirmation prompts"
                shift
                ;;
            --version)
                VERSION="$2"
                log_info "Using explicit version: $VERSION"
                shift 2
                ;;
            --help|-h)
                show_help
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
}

show_help() {
    cat << EOF
RuVector Deployment Script

Usage: $0 [OPTIONS]

Options:
  --dry-run           Run without actually publishing
  --skip-tests        Skip test suite execution
  --skip-crates       Skip crates.io publishing
  --skip-npm          Skip NPM publishing
  --skip-checks       Skip pre-deployment checks
  --force             Skip confirmation prompts
  --version VERSION   Set explicit version
  -h, --help          Show this help message

Environment Variables:
  CRATES_API_KEY      API key for crates.io (required for crate publishing)
  NPM_TOKEN           NPM authentication token (required for npm publishing)
  GITHUB_TOKEN        GitHub token for Actions API (optional)

Examples:
  # Full deployment with all checks
  $0

  # Dry run to test the process
  $0 --dry-run

  # Publish only to crates.io
  $0 --skip-npm

  # Quick deployment skipping tests (not recommended for production)
  $0 --skip-tests --force
EOF
}

confirm_action() {
    local message="$1"

    if [[ "$FORCE" == "true" ]]; then
        return 0
    fi

    echo -e "${YELLOW}$message${NC}"
    read -p "Continue? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_error "Deployment cancelled by user"
        exit 1
    fi
}

################################################################################
# Prerequisites Check
################################################################################

check_prerequisites() {
    log_step "Checking Prerequisites"

    local missing_tools=()

    # Check required tools
    command -v cargo >/dev/null 2>&1 || missing_tools+=("cargo")
    command -v rustc >/dev/null 2>&1 || missing_tools+=("rustc")
    command -v npm >/dev/null 2>&1 || missing_tools+=("npm")
    command -v node >/dev/null 2>&1 || missing_tools+=("node")
    command -v wasm-pack >/dev/null 2>&1 || missing_tools+=("wasm-pack")
    command -v jq >/dev/null 2>&1 || missing_tools+=("jq")

    if [[ ${#missing_tools[@]} -gt 0 ]]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        log_error "Please install them and try again"
        exit 1
    fi

    log_success "All required tools found"

    # Check environment variables for publishing
    if [[ "$PUBLISH_CRATES" == "true" ]] && [[ -z "${CRATES_API_KEY:-}" ]]; then
        log_error "CRATES_API_KEY environment variable not set"
        log_error "Either set it or use --skip-crates flag"
        exit 1
    fi

    if [[ "$PUBLISH_NPM" == "true" ]] && [[ -z "${NPM_TOKEN:-}" ]]; then
        log_error "NPM_TOKEN environment variable not set"
        log_error "Either set it or use --skip-npm flag"
        exit 1
    fi

    # Display versions
    log_info "Rust version: $(rustc --version)"
    log_info "Cargo version: $(cargo --version)"
    log_info "Node version: $(node --version)"
    log_info "NPM version: $(npm --version)"
    log_info "wasm-pack version: $(wasm-pack --version)"
}

################################################################################
# Version Management
################################################################################

get_workspace_version() {
    log_step "Reading Workspace Version"

    cd "$PROJECT_ROOT"

    if [[ -n "$VERSION" ]]; then
        log_info "Using explicit version: $VERSION"
        return
    fi

    # Extract version from workspace Cargo.toml
    VERSION=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')

    if [[ -z "$VERSION" ]]; then
        log_error "Could not determine version from Cargo.toml"
        exit 1
    fi

    log_success "Workspace version: $VERSION"
}

sync_package_versions() {
    log_step "Synchronizing Package Versions"

    cd "$PROJECT_ROOT"

    # Update root package.json
    if [[ -f "package.json" ]]; then
        log_info "Updating root package.json to version $VERSION"
        local temp_file=$(mktemp)
        jq --arg version "$VERSION" '.version = $version' package.json > "$temp_file"
        mv "$temp_file" package.json
        log_success "Root package.json updated"
    fi

    # Update NPM package versions
    local npm_packages=(
        "crates/ruvector-node"
        "crates/ruvector-wasm"
        "crates/ruvector-gnn-node"
        "crates/ruvector-gnn-wasm"
        "crates/ruvector-graph-node"
        "crates/ruvector-graph-wasm"
        "crates/ruvector-tiny-dancer-node"
        "crates/ruvector-tiny-dancer-wasm"
    )

    for pkg in "${npm_packages[@]}"; do
        if [[ -f "$pkg/package.json" ]]; then
            log_info "Updating $pkg/package.json to version $VERSION"
            local temp_file=$(mktemp)
            jq --arg version "$VERSION" '.version = $version' "$pkg/package.json" > "$temp_file"
            mv "$temp_file" "$pkg/package.json"
        fi
    done

    log_success "All package versions synchronized to $VERSION"
}

################################################################################
# Pre-Deployment Checks
################################################################################

run_tests() {
    if [[ "$SKIP_TESTS" == "true" ]]; then
        log_warning "Skipping tests (--skip-tests flag set)"
        return
    fi

    log_step "Running Test Suite"

    cd "$PROJECT_ROOT"

    log_info "Running cargo test --all..."
    if ! cargo test --all --verbose; then
        log_error "Tests failed"
        exit 1
    fi

    log_success "All tests passed"
}

run_clippy() {
    if [[ "$SKIP_CHECKS" == "true" ]]; then
        log_warning "Skipping clippy checks"
        return
    fi

    log_step "Running Clippy Linter"

    cd "$PROJECT_ROOT"

    log_info "Running cargo clippy --all-targets..."
    if ! cargo clippy --all-targets --all-features -- -D warnings; then
        log_error "Clippy found issues"
        exit 1
    fi

    log_success "Clippy checks passed"
}

check_formatting() {
    if [[ "$SKIP_CHECKS" == "true" ]]; then
        log_warning "Skipping formatting check"
        return
    fi

    log_step "Checking Code Formatting"

    cd "$PROJECT_ROOT"

    log_info "Running cargo fmt --check..."
    if ! cargo fmt --all -- --check; then
        log_error "Code formatting issues found"
        log_error "Run 'cargo fmt --all' to fix"
        exit 1
    fi

    log_success "Code formatting is correct"
}

build_wasm_packages() {
    log_step "Building WASM Packages"

    cd "$PROJECT_ROOT"

    local wasm_packages=(
        "crates/ruvector-wasm"
        "crates/ruvector-gnn-wasm"
        "crates/ruvector-graph-wasm"
        "crates/ruvector-tiny-dancer-wasm"
    )

    for pkg in "${wasm_packages[@]}"; do
        if [[ -d "$pkg" ]]; then
            log_info "Building WASM package: $pkg"
            cd "$PROJECT_ROOT/$pkg"

            if [[ -f "build.sh" ]]; then
                log_info "Using build script for $pkg"
                bash build.sh
            elif [[ -f "package.json" ]] && grep -q '"build"' package.json; then
                log_info "Using npm build for $pkg"
                npm run build
            else
                log_info "Using wasm-pack for $pkg"
                wasm-pack build --target web --release
            fi

            log_success "Built WASM package: $pkg"
        fi
    done

    cd "$PROJECT_ROOT"
    log_success "All WASM packages built"
}

################################################################################
# Crate Publishing
################################################################################

publish_crates() {
    if [[ "$PUBLISH_CRATES" != "true" ]]; then
        log_warning "Skipping crates.io publishing"
        return
    fi

    log_step "Publishing Crates to crates.io"

    cd "$PROJECT_ROOT"

    # Configure cargo authentication
    log_info "Configuring cargo authentication..."
    if [[ "$DRY_RUN" != "true" ]]; then
        cargo login "$CRATES_API_KEY"
    fi

    # Crates in dependency order
    local crates=(
        # Core crates (no dependencies)
        "crates/ruvector-core"
        "crates/ruvector-metrics"
        "crates/ruvector-filter"

        # Cluster and replication (depend on core)
        "crates/ruvector-collections"
        "crates/ruvector-snapshot"
        "crates/ruvector-raft"
        "crates/ruvector-cluster"
        "crates/ruvector-replication"

        # Graph and GNN (depend on core)
        "crates/ruvector-graph"
        "crates/ruvector-gnn"

        # Router (depend on core)
        "crates/ruvector-router-core"
        "crates/ruvector-router-ffi"
        "crates/ruvector-router-wasm"
        "crates/ruvector-router-cli"

        # Tiny Dancer (depend on core)
        "crates/ruvector-tiny-dancer-core"
        "crates/ruvector-tiny-dancer-wasm"
        "crates/ruvector-tiny-dancer-node"

        # Bindings (depend on core)
        "crates/ruvector-node"
        "crates/ruvector-wasm"
        "crates/ruvector-gnn-node"
        "crates/ruvector-gnn-wasm"
        "crates/ruvector-graph-node"
        "crates/ruvector-graph-wasm"

        # CLI and server (depend on everything)
        "crates/ruvector-cli"
        "crates/ruvector-server"
        "crates/ruvector-bench"
    )

    local success_count=0
    local failed_crates=()
    local skipped_crates=()

    for crate in "${crates[@]}"; do
        if [[ ! -d "$crate" ]]; then
            log_warning "Crate directory not found: $crate (skipping)"
            skipped_crates+=("$crate")
            continue
        fi

        local crate_name=$(basename "$crate")
        log_info "Publishing $crate_name..."

        cd "$PROJECT_ROOT/$crate"

        # Check if already published
        if cargo search "$crate_name" --limit 1 | grep -q "^$crate_name = \"$VERSION\""; then
            log_warning "$crate_name v$VERSION already published (skipping)"
            ((success_count++))
            skipped_crates+=("$crate_name")
            continue
        fi

        # Verify package
        log_info "Verifying package: $crate_name"
        if ! cargo package --allow-dirty; then
            log_error "Package verification failed: $crate_name"
            failed_crates+=("$crate_name")
            continue
        fi

        # Publish
        if [[ "$DRY_RUN" == "true" ]]; then
            log_info "DRY RUN: Would publish $crate_name"
            ((success_count++))
        else
            log_info "Publishing $crate_name to crates.io..."
            if cargo publish --allow-dirty; then
                log_success "Published $crate_name v$VERSION"
                ((success_count++))

                # Wait for crates.io to index
                log_info "Waiting 30 seconds for crates.io indexing..."
                sleep 30
            else
                log_error "Failed to publish $crate_name"
                failed_crates+=("$crate_name")
            fi
        fi
    done

    cd "$PROJECT_ROOT"

    # Summary
    log_step "Crates Publishing Summary"
    log_info "Total crates: ${#crates[@]}"
    log_success "Successfully published: $success_count"
    log_warning "Skipped: ${#skipped_crates[@]}"

    if [[ ${#failed_crates[@]} -gt 0 ]]; then
        log_error "Failed to publish: ${#failed_crates[@]}"
        for crate in "${failed_crates[@]}"; do
            log_error "  - $crate"
        done
        exit 1
    fi

    log_success "All crates published successfully!"
}

################################################################################
# NPM Publishing
################################################################################

build_native_modules() {
    log_step "Building Native Modules for Current Platform"

    cd "$PROJECT_ROOT"

    local native_packages=(
        "crates/ruvector-node"
        "crates/ruvector-gnn-node"
        "crates/ruvector-graph-node"
        "crates/ruvector-tiny-dancer-node"
    )

    for pkg in "${native_packages[@]}"; do
        if [[ -d "$pkg" ]]; then
            log_info "Building native module: $pkg"
            cd "$PROJECT_ROOT/$pkg"

            # Install dependencies
            if [[ ! -d "node_modules" ]]; then
                log_info "Installing npm dependencies for $pkg"
                npm install
            fi

            # Build
            log_info "Building native module with napi"
            npm run build

            log_success "Built native module: $pkg"
        fi
    done

    cd "$PROJECT_ROOT"
}

publish_npm() {
    if [[ "$PUBLISH_NPM" != "true" ]]; then
        log_warning "Skipping NPM publishing"
        return
    fi

    log_step "Publishing NPM Packages"

    cd "$PROJECT_ROOT"

    # Configure npm authentication
    log_info "Configuring npm authentication..."
    if [[ "$DRY_RUN" != "true" ]]; then
        echo "//registry.npmjs.org/:_authToken=${NPM_TOKEN}" > ~/.npmrc
    fi

    local npm_packages=(
        "crates/ruvector-node"
        "crates/ruvector-wasm"
        "crates/ruvector-gnn-node"
        "crates/ruvector-gnn-wasm"
        "crates/ruvector-graph-node"
        "crates/ruvector-graph-wasm"
        "crates/ruvector-tiny-dancer-node"
        "crates/ruvector-tiny-dancer-wasm"
    )

    local success_count=0
    local failed_packages=()

    for pkg in "${npm_packages[@]}"; do
        if [[ ! -d "$pkg" ]] || [[ ! -f "$pkg/package.json" ]]; then
            log_warning "Package not found: $pkg (skipping)"
            continue
        fi

        local pkg_name=$(jq -r '.name' "$pkg/package.json")
        log_info "Publishing $pkg_name..."

        cd "$PROJECT_ROOT/$pkg"

        # Check if already published
        if npm view "$pkg_name@$VERSION" version >/dev/null 2>&1; then
            log_warning "$pkg_name@$VERSION already published (skipping)"
            ((success_count++))
            continue
        fi

        # Publish
        if [[ "$DRY_RUN" == "true" ]]; then
            log_info "DRY RUN: Would publish $pkg_name"
            ((success_count++))
        else
            log_info "Publishing $pkg_name to npm..."
            if npm publish --access public; then
                log_success "Published $pkg_name@$VERSION"
                ((success_count++))
            else
                log_error "Failed to publish $pkg_name"
                failed_packages+=("$pkg_name")
            fi
        fi
    done

    cd "$PROJECT_ROOT"

    # Summary
    log_step "NPM Publishing Summary"
    log_success "Successfully published: $success_count/${#npm_packages[@]}"

    if [[ ${#failed_packages[@]} -gt 0 ]]; then
        log_error "Failed to publish: ${#failed_packages[@]}"
        for pkg in "${failed_packages[@]}"; do
            log_error "  - $pkg"
        done
        exit 1
    fi

    log_success "All NPM packages published successfully!"
}

################################################################################
# GitHub Actions Integration
################################################################################

trigger_github_builds() {
    log_step "Triggering GitHub Actions for Cross-Platform Builds"

    if [[ -z "${GITHUB_TOKEN:-}" ]]; then
        log_warning "GITHUB_TOKEN not set, skipping GitHub Actions trigger"
        log_info "You can manually trigger the workflow from GitHub Actions UI"
        return
    fi

    if [[ "$DRY_RUN" == "true" ]]; then
        log_info "DRY RUN: Would trigger GitHub Actions workflow"
        return
    fi

    local repo_owner="ruvnet"
    local repo_name="ruvector"
    local workflow_name="native-builds.yml"

    log_info "Triggering workflow: $workflow_name"
    log_info "Repository: $repo_owner/$repo_name"
    log_info "Version tag: v$VERSION"

    # Create GitHub API request
    local response=$(curl -s -X POST \
        -H "Accept: application/vnd.github+json" \
        -H "Authorization: Bearer $GITHUB_TOKEN" \
        -H "X-GitHub-Api-Version: 2022-11-28" \
        "https://api.github.com/repos/$repo_owner/$repo_name/actions/workflows/$workflow_name/dispatches" \
        -d "{\"ref\":\"main\",\"inputs\":{\"version\":\"$VERSION\"}}")

    if [[ -z "$response" ]]; then
        log_success "GitHub Actions workflow triggered successfully"
        log_info "Check status at: https://github.com/$repo_owner/$repo_name/actions"
    else
        log_error "Failed to trigger GitHub Actions workflow"
        log_error "Response: $response"
    fi
}

################################################################################
# Deployment Summary
################################################################################

print_deployment_summary() {
    log_step "Deployment Summary"

    echo ""
    echo -e "${BOLD}Version:${NC} $VERSION"
    echo -e "${BOLD}Dry Run:${NC} $DRY_RUN"
    echo ""

    if [[ "$PUBLISH_CRATES" == "true" ]]; then
        echo -e "${GREEN}✓${NC} Crates published to crates.io"
        echo -e "  View at: ${CYAN}https://crates.io/crates/ruvector-core${NC}"
    fi

    if [[ "$PUBLISH_NPM" == "true" ]]; then
        echo -e "${GREEN}✓${NC} NPM packages published"
        echo -e "  View at: ${CYAN}https://www.npmjs.com/package/@ruvector/node${NC}"
    fi

    echo ""
    echo -e "${BOLD}${GREEN}Deployment completed successfully!${NC}"
    echo ""

    if [[ "$DRY_RUN" == "true" ]]; then
        echo -e "${YELLOW}NOTE: This was a dry run. No actual publishing occurred.${NC}"
        echo -e "${YELLOW}Run without --dry-run to perform actual deployment.${NC}"
    fi
}

################################################################################
# Main Deployment Flow
################################################################################

main() {
    echo -e "${BOLD}${CYAN}"
    cat << "EOF"
╔═══════════════════════════════════════════════════════════════╗
║                                                               ║
║          RuVector Comprehensive Deployment Script            ║
║                                                               ║
╚═══════════════════════════════════════════════════════════════╝
EOF
    echo -e "${NC}"

    # Setup
    setup_logging
    parse_args "$@"

    # Prerequisites
    check_prerequisites

    # Version management
    get_workspace_version
    sync_package_versions

    # Confirmation
    confirm_action "Ready to deploy version $VERSION. This will:
  - Run tests and quality checks
  - Build WASM packages
  - Publish $([ "$PUBLISH_CRATES" == "true" ] && echo "crates.io" || echo "")$([ "$PUBLISH_CRATES" == "true" ] && [ "$PUBLISH_NPM" == "true" ] && echo " and ")$([ "$PUBLISH_NPM" == "true" ] && echo "NPM packages" || echo "")"

    # Pre-deployment checks
    run_tests
    run_clippy
    check_formatting
    build_wasm_packages

    # Publishing
    publish_crates
    build_native_modules
    publish_npm

    # GitHub Actions
    trigger_github_builds

    # Summary
    print_deployment_summary

    log_info "Deployment log saved to: $LOG_FILE"
}

# Run main function
main "$@"
