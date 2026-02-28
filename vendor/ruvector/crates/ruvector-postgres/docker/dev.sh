#!/usr/bin/env bash
# RuVector-Postgres Development Environment
# Starts PostgreSQL with hot-reload support for extension development

set -e  # Exit on error
set -u  # Exit on undefined variable
set -o pipefail  # Exit on pipe failure

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
CONTAINER_NAME="ruvector-postgres-dev"
IMAGE_NAME="ruvector-postgres:dev"
POSTGRES_PORT="${POSTGRES_PORT:-5432}"
POSTGRES_USER="${POSTGRES_USER:-postgres}"
POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-postgres}"
POSTGRES_DB="${POSTGRES_DB:-ruvector_dev}"

# Detect OS
OS_TYPE="$(uname -s)"
case "${OS_TYPE}" in
    Linux*)     PLATFORM="linux";;
    Darwin*)    PLATFORM="macos";;
    *)          PLATFORM="unknown";;
esac

# Functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[⚠]${NC} $1"
}

log_error() {
    echo -e "${RED}[✗]${NC} $1"
}

log_cmd() {
    echo -e "${CYAN}[$]${NC} $1"
}

check_dependencies() {
    log_info "Checking dependencies..."

    # Check Docker
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed. Please install Docker first."
        exit 1
    fi
    log_success "Docker found"

    # Check cargo-pgrx
    if ! command -v cargo-pgrx &> /dev/null; then
        log_warn "cargo-pgrx not found. Installing..."
        cargo install cargo-pgrx --version 0.12.6 --locked
    fi
    log_success "cargo-pgrx found"
}

cleanup() {
    log_info "Stopping development environment..."
    docker stop "${CONTAINER_NAME}" 2>/dev/null || true
    docker rm "${CONTAINER_NAME}" 2>/dev/null || true
}

wait_for_postgres() {
    log_info "Waiting for PostgreSQL to be ready..."
    local max_attempts=30
    local attempt=1

    while [ ${attempt} -le ${max_attempts} ]; do
        if docker exec "${CONTAINER_NAME}" pg_isready -U "${POSTGRES_USER}" &>/dev/null; then
            log_success "PostgreSQL is ready!"
            return 0
        fi

        echo -n "."
        sleep 1
        attempt=$((attempt + 1))
    done

    log_error "PostgreSQL failed to become ready"
    docker logs "${CONTAINER_NAME}"
    return 1
}

build_extension() {
    log_info "Building ruvector-postgres extension..."

    cd "${PROJECT_ROOT}/crates/ruvector-postgres"

    # Build with pgrx
    cargo pgrx install --pg-config "$(which pg_config)" --release

    log_success "Extension built and installed"
}

start_dev_container() {
    log_info "Starting development PostgreSQL container..."

    # Create volume for data persistence
    docker volume create "${CONTAINER_NAME}_data" || true

    # Start PostgreSQL container
    docker run -d \
        --name "${CONTAINER_NAME}" \
        -p "${POSTGRES_PORT}:5432" \
        -e POSTGRES_USER="${POSTGRES_USER}" \
        -e POSTGRES_PASSWORD="${POSTGRES_PASSWORD}" \
        -e POSTGRES_DB="${POSTGRES_DB}" \
        -v "${CONTAINER_NAME}_data:/var/lib/postgresql/data" \
        -v "${HOME}/.pgrx:/home/postgres/.pgrx:ro" \
        --health-cmd="pg_isready -U ${POSTGRES_USER}" \
        --health-interval=5s \
        --health-timeout=5s \
        --health-retries=5 \
        postgres:16-bookworm

    log_success "Container started: ${CONTAINER_NAME}"
}

setup_extension() {
    log_info "Setting up extension in database..."

    # Create extension
    docker exec -it "${CONTAINER_NAME}" psql -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" -c "CREATE EXTENSION IF NOT EXISTS ruvector_postgres CASCADE;" || {
        log_warn "Extension not yet installed. Run 'cargo pgrx install' first."
        return 1
    }

    log_success "Extension loaded successfully"
}

show_connection_info() {
    local connection_string="postgresql://${POSTGRES_USER}:${POSTGRES_PASSWORD}@localhost:${POSTGRES_PORT}/${POSTGRES_DB}"

    echo ""
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${GREEN}  RuVector-Postgres Development Environment Ready!${NC}"
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo -e "${CYAN}Connection String:${NC}"
    echo -e "  ${connection_string}"
    echo ""
    echo -e "${CYAN}Quick Connect Commands:${NC}"
    log_cmd "psql ${connection_string}"
    log_cmd "docker exec -it ${CONTAINER_NAME} psql -U ${POSTGRES_USER} -d ${POSTGRES_DB}"
    echo ""
    echo -e "${CYAN}Development Workflow:${NC}"
    echo -e "  1. Make changes to extension code"
    echo -e "  2. Rebuild: ${YELLOW}cargo pgrx install${NC}"
    echo -e "  3. Reload: ${YELLOW}docker exec ${CONTAINER_NAME} psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c 'DROP EXTENSION ruvector_postgres CASCADE; CREATE EXTENSION ruvector_postgres;'${NC}"
    echo ""
    echo -e "${CYAN}Useful Commands:${NC}"
    log_cmd "cargo pgrx test pg16          # Run tests"
    log_cmd "cargo pgrx package            # Create distributable package"
    log_cmd "docker logs -f ${CONTAINER_NAME}  # View PostgreSQL logs"
    log_cmd "docker stop ${CONTAINER_NAME}     # Stop development environment"
    echo ""
    echo -e "${CYAN}Container Info:${NC}"
    echo -e "  Name: ${CONTAINER_NAME}"
    echo -e "  Port: ${POSTGRES_PORT}"
    echo -e "  User: ${POSTGRES_USER}"
    echo -e "  Database: ${POSTGRES_DB}"
    echo -e "  Platform: ${PLATFORM}"
    echo ""
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

watch_and_reload() {
    log_info "Starting file watcher for hot-reload..."
    log_warn "File watching requires 'cargo-watch'. Install with: cargo install cargo-watch"

    cd "${PROJECT_ROOT}/crates/ruvector-postgres"

    cargo watch -x "pgrx install" -s "docker exec ${CONTAINER_NAME} psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c 'DROP EXTENSION IF EXISTS ruvector_postgres CASCADE; CREATE EXTENSION ruvector_postgres;'"
}

show_usage() {
    cat << EOF
RuVector-Postgres Development Environment

Usage: $0 [OPTIONS] [COMMAND]

Commands:
    start       Start development environment (default)
    stop        Stop development environment
    restart     Restart development environment
    logs        Show PostgreSQL logs
    psql        Open psql shell
    watch       Start file watcher for hot-reload
    rebuild     Rebuild and reload extension
    status      Show container status

Options:
    -p, --port PORT        PostgreSQL port (default: 5432)
    -u, --user USER        PostgreSQL user (default: postgres)
    -d, --database DB      PostgreSQL database (default: ruvector_dev)
    -b, --background       Start in background (default)
    -f, --foreground       Start in foreground with logs
    -h, --help             Show this help message

Environment Variables:
    POSTGRES_PORT          PostgreSQL port (default: 5432)
    POSTGRES_USER          PostgreSQL user (default: postgres)
    POSTGRES_PASSWORD      PostgreSQL password (default: postgres)
    POSTGRES_DB            PostgreSQL database (default: ruvector_dev)

Examples:
    # Start development environment
    $0 start

    # Start with custom port
    $0 --port 5433 start

    # Open psql shell
    $0 psql

    # Watch for changes and auto-reload
    $0 watch

    # View logs
    $0 logs
EOF
}

cmd_start() {
    check_dependencies

    # Stop existing container if running
    docker stop "${CONTAINER_NAME}" 2>/dev/null || true
    docker rm "${CONTAINER_NAME}" 2>/dev/null || true

    start_dev_container
    wait_for_postgres

    # Try to setup extension if already built
    setup_extension || log_warn "Run 'cargo pgrx install' to build and install the extension"

    show_connection_info
}

cmd_stop() {
    cleanup
    log_success "Development environment stopped"
}

cmd_restart() {
    cmd_stop
    sleep 2
    cmd_start
}

cmd_logs() {
    docker logs -f "${CONTAINER_NAME}"
}

cmd_psql() {
    docker exec -it "${CONTAINER_NAME}" psql -U "${POSTGRES_USER}" -d "${POSTGRES_DB}"
}

cmd_rebuild() {
    log_info "Rebuilding extension..."
    cd "${PROJECT_ROOT}/crates/ruvector-postgres"
    cargo pgrx install --release

    log_info "Reloading extension in database..."
    docker exec "${CONTAINER_NAME}" psql -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" << 'EOF'
DROP EXTENSION IF EXISTS ruvector_postgres CASCADE;
CREATE EXTENSION ruvector_postgres;
SELECT extname, extversion FROM pg_extension WHERE extname = 'ruvector_postgres';
EOF

    log_success "Extension rebuilt and reloaded!"
}

cmd_status() {
    if docker ps --filter "name=${CONTAINER_NAME}" --format "{{.Names}}" | grep -q "${CONTAINER_NAME}"; then
        log_success "Container ${CONTAINER_NAME} is running"
        docker ps --filter "name=${CONTAINER_NAME}"
        echo ""
        show_connection_info
    else
        log_warn "Container ${CONTAINER_NAME} is not running"
        echo "Start with: $0 start"
    fi
}

main() {
    local command="start"
    local foreground=false

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            start|stop|restart|logs|psql|watch|rebuild|status)
                command="$1"
                shift
                ;;
            -p|--port)
                POSTGRES_PORT="$2"
                shift 2
                ;;
            -u|--user)
                POSTGRES_USER="$2"
                shift 2
                ;;
            -d|--database)
                POSTGRES_DB="$2"
                shift 2
                ;;
            -b|--background)
                foreground=false
                shift
                ;;
            -f|--foreground)
                foreground=true
                shift
                ;;
            -h|--help)
                show_usage
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                show_usage
                exit 1
                ;;
        esac
    done

    # Execute command
    case "${command}" in
        start)
            cmd_start
            if [ "${foreground}" == "true" ]; then
                cmd_logs
            fi
            ;;
        stop)
            cmd_stop
            ;;
        restart)
            cmd_restart
            ;;
        logs)
            cmd_logs
            ;;
        psql)
            cmd_psql
            ;;
        watch)
            watch_and_reload
            ;;
        rebuild)
            cmd_rebuild
            ;;
        status)
            cmd_status
            ;;
        *)
            log_error "Unknown command: ${command}"
            show_usage
            exit 1
            ;;
    esac
}

# Run main function
main "$@"
