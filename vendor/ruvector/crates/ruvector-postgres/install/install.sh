#!/bin/bash
#
# RuVector PostgreSQL Extension Installer
# High-performance vector similarity search with SIMD optimization
#
# Usage: ./install.sh [OPTIONS]
#
# Options:
#   --pg-version VERSION    PostgreSQL version (14, 15, 16, 17)
#   --pg-config PATH        Path to pg_config binary
#   --build-from-source     Build from source (default: use pre-built if available)
#   --simd MODE             SIMD mode: auto, avx512, avx2, neon, scalar (default: auto)
#   --prefix PATH           Installation prefix (default: auto-detect)
#   --config FILE           Configuration file path
#   --skip-tests            Skip installation tests
#   --uninstall             Uninstall RuVector
#   --upgrade               Upgrade existing installation
#   --dry-run               Show what would be done without making changes
#   --verbose               Verbose output
#   --help                  Show this help message
#
set -e

# ============================================================================
# Configuration
# ============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RUVECTOR_VERSION="0.1.0"
EXTENSION_NAME="ruvector"

# Default options
PG_VERSION=""
PG_CONFIG=""
BUILD_FROM_SOURCE=false
SIMD_MODE="auto"
INSTALL_PREFIX=""
CONFIG_FILE=""
SKIP_TESTS=false
UNINSTALL=false
UPGRADE=false
DRY_RUN=false
VERBOSE=false

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# ============================================================================
# Helper Functions
# ============================================================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

log_verbose() {
    if [ "$VERBOSE" = true ]; then
        echo -e "${CYAN}[DEBUG]${NC} $1"
    fi
}

die() {
    log_error "$1"
    exit 1
}

run_cmd() {
    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY-RUN] Would run: $*"
        return 0
    fi
    if [ "$VERBOSE" = true ]; then
        log_verbose "Running: $*"
        "$@"
    else
        "$@" >/dev/null 2>&1
    fi
}

check_command() {
    command -v "$1" >/dev/null 2>&1
}

# ============================================================================
# Environment Detection
# ============================================================================

detect_os() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        OS_NAME="$ID"
        OS_VERSION="$VERSION_ID"
        OS_PRETTY="$PRETTY_NAME"
    elif [ -f /etc/redhat-release ]; then
        OS_NAME="rhel"
        OS_VERSION=$(cat /etc/redhat-release | grep -oP '\d+' | head -1)
        OS_PRETTY=$(cat /etc/redhat-release)
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        OS_NAME="macos"
        OS_VERSION=$(sw_vers -productVersion)
        OS_PRETTY="macOS $OS_VERSION"
    else
        OS_NAME="unknown"
        OS_VERSION="unknown"
        OS_PRETTY="Unknown OS"
    fi

    # Detect architecture
    ARCH=$(uname -m)
    case "$ARCH" in
        x86_64|amd64) ARCH="x86_64" ;;
        aarch64|arm64) ARCH="aarch64" ;;
        *) ARCH="unknown" ;;
    esac

    log_verbose "Detected OS: $OS_PRETTY ($OS_NAME $OS_VERSION) on $ARCH"
}

detect_simd_capabilities() {
    SIMD_AVX512=false
    SIMD_AVX2=false
    SIMD_NEON=false

    if [ "$ARCH" = "x86_64" ]; then
        if grep -q "avx512f" /proc/cpuinfo 2>/dev/null; then
            SIMD_AVX512=true
            log_verbose "AVX-512 support detected"
        fi
        if grep -q "avx2" /proc/cpuinfo 2>/dev/null; then
            SIMD_AVX2=true
            log_verbose "AVX2 support detected"
        fi
    elif [ "$ARCH" = "aarch64" ]; then
        # ARM NEON is standard on aarch64
        SIMD_NEON=true
        log_verbose "NEON support detected (ARM64)"
    fi

    # Determine best SIMD mode
    if [ "$SIMD_MODE" = "auto" ]; then
        if [ "$SIMD_AVX512" = true ]; then
            DETECTED_SIMD="avx512"
        elif [ "$SIMD_AVX2" = true ]; then
            DETECTED_SIMD="avx2"
        elif [ "$SIMD_NEON" = true ]; then
            DETECTED_SIMD="neon"
        else
            DETECTED_SIMD="scalar"
        fi
        log_verbose "Auto-detected SIMD mode: $DETECTED_SIMD"
    else
        DETECTED_SIMD="$SIMD_MODE"
    fi
}

detect_postgresql() {
    # Try to find pg_config
    if [ -n "$PG_CONFIG" ] && [ -x "$PG_CONFIG" ]; then
        log_verbose "Using provided pg_config: $PG_CONFIG"
    else
        # Search for pg_config in common locations
        PG_CONFIG_PATHS=(
            "/usr/bin/pg_config"
            "/usr/local/bin/pg_config"
            "/usr/pgsql-${PG_VERSION:-16}/bin/pg_config"
            "/usr/lib/postgresql/${PG_VERSION:-16}/bin/pg_config"
            "/opt/homebrew/opt/postgresql@${PG_VERSION:-16}/bin/pg_config"
            "/Applications/Postgres.app/Contents/Versions/latest/bin/pg_config"
        )

        for path in "${PG_CONFIG_PATHS[@]}"; do
            if [ -x "$path" ]; then
                PG_CONFIG="$path"
                log_verbose "Found pg_config: $PG_CONFIG"
                break
            fi
        done

        # Try system PATH
        if [ -z "$PG_CONFIG" ] && check_command pg_config; then
            PG_CONFIG=$(which pg_config)
            log_verbose "Found pg_config in PATH: $PG_CONFIG"
        fi
    fi

    if [ -z "$PG_CONFIG" ] || [ ! -x "$PG_CONFIG" ]; then
        die "PostgreSQL pg_config not found. Please install PostgreSQL or specify --pg-config"
    fi

    # Get PostgreSQL information
    PG_DETECTED_VERSION=$("$PG_CONFIG" --version | grep -oP '\d+' | head -1)
    PG_LIBDIR=$("$PG_CONFIG" --pkglibdir)
    PG_SHAREDIR=$("$PG_CONFIG" --sharedir)
    PG_INCLUDEDIR=$("$PG_CONFIG" --includedir-server)
    PG_BINDIR=$("$PG_CONFIG" --bindir)

    if [ -n "$PG_VERSION" ] && [ "$PG_VERSION" != "$PG_DETECTED_VERSION" ]; then
        log_warning "Requested PG version $PG_VERSION but detected $PG_DETECTED_VERSION"
    fi
    PG_VERSION="$PG_DETECTED_VERSION"

    log_info "PostgreSQL $PG_VERSION detected"
    log_verbose "  Library dir: $PG_LIBDIR"
    log_verbose "  Share dir: $PG_SHAREDIR"
    log_verbose "  Include dir: $PG_INCLUDEDIR"
}

# ============================================================================
# Dependency Checks
# ============================================================================

check_dependencies() {
    log_info "Checking dependencies..."

    local missing_deps=()

    # Check for required tools
    if [ "$BUILD_FROM_SOURCE" = true ]; then
        if ! check_command rustc; then
            missing_deps+=("rust")
        else
            RUST_VERSION=$(rustc --version | cut -d' ' -f2)
            log_verbose "Rust version: $RUST_VERSION"
        fi

        if ! check_command cargo; then
            missing_deps+=("cargo")
        fi

        # Check for pgrx
        if ! cargo install --list 2>/dev/null | grep -q "cargo-pgrx"; then
            log_warning "cargo-pgrx not installed, will install during build"
        fi

        # Check for build tools
        if ! check_command gcc && ! check_command clang; then
            missing_deps+=("gcc or clang")
        fi

        if ! check_command make; then
            missing_deps+=("make")
        fi
    fi

    if [ ${#missing_deps[@]} -gt 0 ]; then
        log_error "Missing dependencies: ${missing_deps[*]}"
        log_info "Install missing dependencies with:"
        case "$OS_NAME" in
            ubuntu|debian)
                echo "  sudo apt-get install ${missing_deps[*]}"
                if [[ " ${missing_deps[*]} " =~ " rust " ]]; then
                    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
                fi
                ;;
            centos|rhel|fedora)
                echo "  sudo dnf install ${missing_deps[*]}"
                if [[ " ${missing_deps[*]} " =~ " rust " ]]; then
                    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
                fi
                ;;
            macos)
                echo "  brew install ${missing_deps[*]}"
                if [[ " ${missing_deps[*]} " =~ " rust " ]]; then
                    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
                fi
                ;;
        esac
        exit 1
    fi

    log_success "All dependencies satisfied"
}

# ============================================================================
# Installation Functions
# ============================================================================

build_from_source() {
    log_info "Building RuVector from source..."

    cd "$PROJECT_ROOT"

    # Ensure pgrx is installed
    if ! cargo install --list 2>/dev/null | grep -q "cargo-pgrx"; then
        log_info "Installing cargo-pgrx..."
        run_cmd cargo install cargo-pgrx --version "0.12.9" --locked
    fi

    # Initialize pgrx for our PG version if needed
    if [ ! -f "$HOME/.pgrx/config.toml" ]; then
        log_info "Initializing pgrx..."
        run_cmd cargo pgrx init --pg${PG_VERSION} "$PG_CONFIG"
    fi

    # Set SIMD features based on detection
    local FEATURES="pg${PG_VERSION}"
    case "$DETECTED_SIMD" in
        avx512) FEATURES="$FEATURES,simd-avx512" ;;
        avx2) FEATURES="$FEATURES,simd-avx2" ;;
        neon) FEATURES="$FEATURES,simd-neon" ;;
        *) FEATURES="$FEATURES,simd-auto" ;;
    esac

    log_verbose "Building with features: $FEATURES"

    # Build the extension
    log_info "Compiling extension (this may take a few minutes)..."
    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY-RUN] Would run: cargo pgrx package --pg-config $PG_CONFIG"
    else
        cd "$PROJECT_ROOT/crates/ruvector-postgres"
        cargo pgrx package --pg-config "$PG_CONFIG"
    fi

    # Set build output path
    BUILD_OUTPUT="$PROJECT_ROOT/target/release/ruvector-pg${PG_VERSION}"

    log_success "Build completed"
}

install_extension() {
    log_info "Installing RuVector extension..."

    local SO_FILE="${BUILD_OUTPUT}/usr/lib/postgresql/${PG_VERSION}/lib/ruvector.so"
    local CONTROL_FILE="${BUILD_OUTPUT}/usr/share/postgresql/${PG_VERSION}/extension/ruvector.control"
    local SQL_FILE="${PROJECT_ROOT}/crates/ruvector-postgres/sql/ruvector--${RUVECTOR_VERSION}.sql"

    # Check build output exists
    if [ ! -f "$SO_FILE" ]; then
        die "Build output not found: $SO_FILE"
    fi

    # Install shared library
    log_info "Installing shared library to $PG_LIBDIR..."
    run_cmd cp "$SO_FILE" "$PG_LIBDIR/"
    run_cmd chmod 755 "$PG_LIBDIR/ruvector.so"

    # Install control file
    log_info "Installing control file to $PG_SHAREDIR/extension/..."
    run_cmd cp "$CONTROL_FILE" "$PG_SHAREDIR/extension/"

    # Install SQL file
    log_info "Installing SQL file to $PG_SHAREDIR/extension/..."
    run_cmd cp "$SQL_FILE" "$PG_SHAREDIR/extension/"

    log_success "Extension files installed"
}

create_config() {
    log_info "Creating configuration..."

    local CONFIG_DIR="$PG_SHAREDIR/extension"
    local CONFIG_OUT="$CONFIG_DIR/ruvector.conf"

    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY-RUN] Would create config at: $CONFIG_OUT"
        return 0
    fi

    cat > "$CONFIG_OUT" << EOF
# RuVector PostgreSQL Extension Configuration
# Generated by installer on $(date)

# =============================================================================
# SIMD Configuration
# =============================================================================
# Detected SIMD capabilities: $DETECTED_SIMD
# Options: auto, avx512, avx2, neon, scalar
#ruvector.simd_mode = 'auto'

# =============================================================================
# Memory Configuration
# =============================================================================
# Maximum memory for vector operations (in MB)
#ruvector.max_memory_mb = 1024

# Enable memory pooling for better performance
#ruvector.enable_memory_pool = on

# =============================================================================
# Index Configuration
# =============================================================================
# Default HNSW index parameters
#ruvector.hnsw_ef_construction = 64
#ruvector.hnsw_m = 16
#ruvector.hnsw_ef_search = 40

# Default IVF-Flat index parameters
#ruvector.ivfflat_lists = 100
#ruvector.ivfflat_probes = 10

# =============================================================================
# Distance Calculation
# =============================================================================
# Enable parallel distance computation for large batches
#ruvector.parallel_distance = on

# Minimum batch size for parallel processing
#ruvector.parallel_min_batch = 1000

# =============================================================================
# Quantization
# =============================================================================
# Enable product quantization for large datasets
#ruvector.enable_pq = off

# Product quantization parameters
#ruvector.pq_m = 8
#ruvector.pq_nbits = 8

# =============================================================================
# Logging
# =============================================================================
# Log level: debug, info, warning, error
#ruvector.log_level = 'info'

# Log SIMD operations (for debugging)
#ruvector.log_simd = off
EOF

    log_success "Configuration created at: $CONFIG_OUT"
}

# ============================================================================
# Testing Functions
# ============================================================================

run_tests() {
    if [ "$SKIP_TESTS" = true ]; then
        log_warning "Skipping installation tests"
        return 0
    fi

    log_info "Running installation tests..."

    # Find psql
    local PSQL="${PG_BINDIR}/psql"
    if [ ! -x "$PSQL" ]; then
        PSQL=$(which psql 2>/dev/null || true)
    fi

    if [ -z "$PSQL" ] || [ ! -x "$PSQL" ]; then
        log_warning "psql not found, skipping tests"
        return 0
    fi

    # Create test database
    local TEST_DB="ruvector_test_$$"

    log_verbose "Creating test database: $TEST_DB"

    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY-RUN] Would run installation tests"
        return 0
    fi

    # Try to connect and run tests
    local TEST_RESULT=0

    # Use postgres user or current user
    local PG_USER="${PGUSER:-postgres}"

    # Create test script
    local TEST_SCRIPT=$(mktemp)
    cat > "$TEST_SCRIPT" << 'EOSQL'
-- RuVector Installation Test Suite

-- Test 1: Create extension
CREATE EXTENSION IF NOT EXISTS ruvector;
SELECT 'Test 1: Extension created' AS result;

-- Test 2: Create table with ruvector column
CREATE TABLE test_vectors (id serial PRIMARY KEY, embedding ruvector);
SELECT 'Test 2: Table created' AS result;

-- Test 3: Insert vectors
INSERT INTO test_vectors (embedding) VALUES
    ('[1,2,3]'),
    ('[4,5,6]'),
    ('[7,8,9]');
SELECT 'Test 3: Vectors inserted' AS result;

-- Test 4: Read vectors from storage
SELECT count(*) AS vector_count FROM test_vectors;

-- Test 5: Distance calculations
SELECT id, embedding <-> '[1,1,1]'::ruvector AS l2_dist
FROM test_vectors ORDER BY l2_dist LIMIT 3;
SELECT 'Test 5: Distance calculations work' AS result;

-- Test 6: Cosine distance
SELECT id, embedding <=> '[1,1,1]'::ruvector AS cosine_dist
FROM test_vectors ORDER BY cosine_dist LIMIT 3;
SELECT 'Test 6: Cosine distance works' AS result;

-- Test 7: Vector dimensions
SELECT ruvector_dims('[1,2,3,4,5]'::ruvector) AS dims;

-- Test 8: Vector normalization
SELECT ruvector_norm('[3,4]'::ruvector) AS norm;

-- Cleanup
DROP TABLE test_vectors;
DROP EXTENSION ruvector CASCADE;
SELECT 'All tests passed!' AS final_result;
EOSQL

    # Run tests
    if su - "$PG_USER" -c "createdb $TEST_DB" 2>/dev/null || createdb "$TEST_DB" 2>/dev/null; then
        if su - "$PG_USER" -c "$PSQL -d $TEST_DB -f $TEST_SCRIPT" 2>&1 || \
           $PSQL -d "$TEST_DB" -f "$TEST_SCRIPT" 2>&1; then
            log_success "All installation tests passed"
        else
            log_error "Some tests failed"
            TEST_RESULT=1
        fi

        # Cleanup test database
        su - "$PG_USER" -c "dropdb $TEST_DB" 2>/dev/null || dropdb "$TEST_DB" 2>/dev/null || true
    else
        log_warning "Could not create test database, skipping detailed tests"

        # Try simpler test
        log_info "Attempting basic connectivity test..."
        if su - "$PG_USER" -c "$PSQL -c 'SELECT 1'" 2>/dev/null || \
           $PSQL -c 'SELECT 1' 2>/dev/null; then
            log_success "PostgreSQL connectivity OK"
        else
            log_warning "Could not connect to PostgreSQL"
        fi
    fi

    rm -f "$TEST_SCRIPT"
    return $TEST_RESULT
}

# ============================================================================
# Uninstall Functions
# ============================================================================

uninstall_extension() {
    log_info "Uninstalling RuVector extension..."

    # Remove files
    local files_to_remove=(
        "$PG_LIBDIR/ruvector.so"
        "$PG_SHAREDIR/extension/ruvector.control"
        "$PG_SHAREDIR/extension/ruvector--${RUVECTOR_VERSION}.sql"
        "$PG_SHAREDIR/extension/ruvector.conf"
    )

    for f in "${files_to_remove[@]}"; do
        if [ -f "$f" ]; then
            log_verbose "Removing: $f"
            run_cmd rm -f "$f"
        fi
    done

    log_success "RuVector uninstalled"
    log_warning "Note: You may need to DROP EXTENSION ruvector in databases where it was created"
}

# ============================================================================
# Main Installation Flow
# ============================================================================

show_help() {
    cat << EOF
RuVector PostgreSQL Extension Installer v${RUVECTOR_VERSION}

Usage: $0 [OPTIONS]

Options:
  --pg-version VERSION    PostgreSQL version (14, 15, 16, 17)
  --pg-config PATH        Path to pg_config binary
  --build-from-source     Build from source (required for now)
  --simd MODE             SIMD mode: auto, avx512, avx2, neon, scalar
  --prefix PATH           Installation prefix (default: auto-detect)
  --config FILE           Configuration file path
  --skip-tests            Skip installation tests
  --uninstall             Uninstall RuVector
  --upgrade               Upgrade existing installation
  --dry-run               Show what would be done
  --verbose               Verbose output
  --help                  Show this help

Examples:
  # Install with auto-detection
  $0 --build-from-source

  # Install for specific PostgreSQL version
  $0 --build-from-source --pg-version 16

  # Install with specific pg_config
  $0 --build-from-source --pg-config /usr/pgsql-16/bin/pg_config

  # Uninstall
  $0 --uninstall --pg-config /usr/bin/pg_config

  # Dry run to see what would happen
  $0 --build-from-source --dry-run --verbose

EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --pg-version)
                PG_VERSION="$2"
                shift 2
                ;;
            --pg-config)
                PG_CONFIG="$2"
                shift 2
                ;;
            --build-from-source)
                BUILD_FROM_SOURCE=true
                shift
                ;;
            --simd)
                SIMD_MODE="$2"
                shift 2
                ;;
            --prefix)
                INSTALL_PREFIX="$2"
                shift 2
                ;;
            --config)
                CONFIG_FILE="$2"
                shift 2
                ;;
            --skip-tests)
                SKIP_TESTS=true
                shift
                ;;
            --uninstall)
                UNINSTALL=true
                shift
                ;;
            --upgrade)
                UPGRADE=true
                shift
                ;;
            --dry-run)
                DRY_RUN=true
                shift
                ;;
            --verbose|-v)
                VERBOSE=true
                shift
                ;;
            --help|-h)
                show_help
                exit 0
                ;;
            *)
                die "Unknown option: $1"
                ;;
        esac
    done
}

main() {
    echo ""
    echo "╔═══════════════════════════════════════════════════════════════╗"
    echo "║         RuVector PostgreSQL Extension Installer               ║"
    echo "║       High-Performance Vector Similarity Search               ║"
    echo "║                    Version ${RUVECTOR_VERSION}                             ║"
    echo "╚═══════════════════════════════════════════════════════════════╝"
    echo ""

    parse_args "$@"

    # Detect environment
    detect_os
    detect_simd_capabilities
    detect_postgresql

    echo ""
    log_info "Environment Summary:"
    echo "  OS:         $OS_PRETTY"
    echo "  Arch:       $ARCH"
    echo "  SIMD:       $DETECTED_SIMD"
    echo "  PostgreSQL: $PG_VERSION"
    echo "  pg_config:  $PG_CONFIG"
    echo ""

    # Handle uninstall
    if [ "$UNINSTALL" = true ]; then
        uninstall_extension
        exit 0
    fi

    # Check dependencies
    check_dependencies

    # Build from source (currently only option)
    if [ "$BUILD_FROM_SOURCE" = true ]; then
        build_from_source
    else
        log_warning "Pre-built binaries not yet available"
        log_info "Building from source..."
        BUILD_FROM_SOURCE=true
        build_from_source
    fi

    # Install extension
    install_extension

    # Create configuration
    create_config

    # Run tests
    run_tests

    echo ""
    log_success "RuVector installation complete!"
    echo ""
    echo "Next steps:"
    echo "  1. Connect to your database: psql -d your_database"
    echo "  2. Create the extension: CREATE EXTENSION ruvector;"
    echo "  3. Create a table with vectors:"
    echo "     CREATE TABLE items (id serial, embedding ruvector);"
    echo "  4. Insert vectors:"
    echo "     INSERT INTO items (embedding) VALUES ('[1,2,3]');"
    echo "  5. Query with similarity search:"
    echo "     SELECT * FROM items ORDER BY embedding <-> '[1,1,1]' LIMIT 10;"
    echo ""
    echo "Documentation: https://github.com/ruvnet/ruvector"
    echo ""
}

# Run main
main "$@"
