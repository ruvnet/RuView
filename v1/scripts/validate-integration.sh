#!/bin/bash

# WiFi-DensePose Integration Validation Script
# This script validates the complete system integration

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
VENV_PATH="${PROJECT_ROOT}/.venv"
TEST_DB_PATH="${PROJECT_ROOT}/test_integration.db"
LOG_FILE="${PROJECT_ROOT}/integration_validation.log"

# Functions
log() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $1" | tee -a "$LOG_FILE"
}

success() {
    echo -e "${GREEN}✅ $1${NC}" | tee -a "$LOG_FILE"
}

warning() {
    echo -e "${YELLOW}⚠️  $1${NC}" | tee -a "$LOG_FILE"
}

error() {
    echo -e "${RED}❌ $1${NC}" | tee -a "$LOG_FILE"
}

cleanup() {
    log "Cleaning up test resources..."
    
    # Stop any running servers
    pkill -f "wifi-densepose" || true
    pkill -f "uvicorn.*src.app" || true
    
    # Remove test database
    [ -f "$TEST_DB_PATH" ] && rm -f "$TEST_DB_PATH"
    
    # Remove test logs
    find "$PROJECT_ROOT" -name "*.log" -path "*/test*" -delete 2>/dev/null || true
    
    success "Cleanup completed"
}

check_prerequisites() {
    log "Checking prerequisites..."
    
    # Check Python version
    if ! python3 --version | grep -E "Python 3\.(9|10|11|12)" > /dev/null; then
        error "Python 3.9+ is required"
        exit 1
    fi
    success "Python version check passed"
    
    # Check if virtual environment exists
    if [ ! -d "$VENV_PATH" ]; then
        warning "Virtual environment not found, creating one..."
        python3 -m venv "$VENV_PATH"
    fi
    success "Virtual environment check passed"
    
    # Activate virtual environment
    source "$VENV_PATH/bin/activate"
    
    # Check if requirements are installed
    if ! pip list | grep -q "fastapi"; then
        warning "Dependencies not installed, installing..."
        pip install -e ".[dev]"
    fi
    success "Dependencies check passed"
}

validate_package_structure() {
    log "Validating package structure..."
    
    # Check main application files
    required_files=(
        "src/__init__.py"
        "src/main.py"
        "src/app.py"
        "src/config.py"
        "src/logger.py"
        "src/cli.py"
        "pyproject.toml"
        "setup.py"
        "MANIFEST.in"
    )
    
    for file in "${required_files[@]}"; do
        if [ ! -f "$PROJECT_ROOT/$file" ]; then
            error "Required file missing: $file"
            exit 1
        fi
    done
    success "Package structure validation passed"
    
    # Check directory structure
    required_dirs=(
        "src/config"
        "src/core"
        "src/api"
        "src/services"
        "src/middleware"
        "src/database"
        "src/tasks"
        "src/commands"
        "tests/unit"
        "tests/integration"
    )
    
    for dir in "${required_dirs[@]}"; do
        if [ ! -d "$PROJECT_ROOT/$dir" ]; then
            error "Required directory missing: $dir"
            exit 1
        fi
    done
    success "Directory structure validation passed"
}

validate_imports() {
    log "Validating Python imports..."
    
    cd "$PROJECT_ROOT"
    source "$VENV_PATH/bin/activate"
    
    # Test main package import
    if ! python -c "import src; print(f'Package version: {src.__version__}')"; then
        error "Failed to import main package"
        exit 1
    fi
    success "Main package import passed"
    
    # Test core components
    core_modules=(
        "src.app"
        "src.config.settings"
        "src.logger"
        "src.cli"
        "src.core.csi_processor"
        "src.core.phase_sanitizer"
        "src.core.pose_estimator"
        "src.core.router_interface"
        "src.services.orchestrator"
        "src.database.connection"
        "src.database.models"
    )
    
    for module in "${core_modules[@]}"; do
        if ! python -c "import $module" 2>/dev/null; then
            error "Failed to import module: $module"
            exit 1
        fi
    done
    success "Core modules import passed"
}

validate_configuration() {
    log "Validating configuration..."
    
    cd "$PROJECT_ROOT"
    source "$VENV_PATH/bin/activate"
    
    # Test configuration loading
    if ! python -c "
from src.config.settings import get_settings
settings = get_settings()
print(f'Environment: {settings.environment}')
print(f'Debug: {settings.debug}')
print(f'API Version: {settings.api_version}')
"; then
        error "Configuration validation failed"
        exit 1
    fi
    success "Configuration validation passed"
}

validate_database() {
    log "Validating database integration..."
    
    cd "$PROJECT_ROOT"
    source "$VENV_PATH/bin/activate"
    
    # Test database connection and models
    if ! python -c "
import asyncio
from src.config.settings import get_settings
from src.database.connection import get_database_manager

async def test_db():
    settings = get_settings()
    settings.database_url = 'sqlite+aiosqlite:///test_integration.db'
    
    db_manager = get_database_manager(settings)
    await db_manager.initialize()
    await db_manager.test_connection()
    
    # Test connection stats
    stats = await db_manager.get_connection_stats()
    print(f'Database connected: {stats[\"database\"][\"connected\"]}')
    
    await db_manager.close_all_connections()
    print('Database validation passed')

asyncio.run(test_db())
"; then
        error "Database validation failed"
        exit 1
    fi
    success "Database validation passed"
}

validate_api_endpoints() {
    log "Validating API endpoints..."
    
    cd "$PROJECT_ROOT"
    source "$VENV_PATH/bin/activate"
    
    # Start server in background
    export WIFI_DENSEPOSE_ENVIRONMENT=test
    export WIFI_DENSEPOSE_DATABASE_URL="sqlite+aiosqlite:///test_integration.db"
    
    python -m uvicorn src.app:app --host 127.0.0.1 --port 8888 --log-level error &
    SERVER_PID=$!
    
    # Wait for server to start
    sleep 5
    
    # Test endpoints
    endpoints=(
        "http://127.0.0.1:8888/health"
        "http://127.0.0.1:8888/metrics"
        "http://127.0.0.1:8888/api/v1/devices"
        "http://127.0.0.1:8888/api/v1/sessions"
    )
    
    for endpoint in "${endpoints[@]}"; do
        if ! curl -s -f "$endpoint" > /dev/null; then
            error "API endpoint failed: $endpoint"
            kill $SERVER_PID 2>/dev/null || true
            exit 1
        fi
    done
    
    # Stop server
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
    
    success "API endpoints validation passed"
}

validate_cli() {
    log "Validating CLI interface..."
    
    cd "$PROJECT_ROOT"
    source "$VENV_PATH/bin/activate"
    
    # Test CLI commands
    if ! python -m src.cli --help > /dev/null; then
        error "CLI help command failed"
        exit 1
    fi
    success "CLI help command passed"
    
    # Test version command
    if ! python -m src.cli version > /dev/null; then
        error "CLI version command failed"
        exit 1
    fi
    success "CLI version command passed"
    
    # Test config validation
    export WIFI_DENSEPOSE_ENVIRONMENT=test
    export WIFI_DENSEPOSE_DATABASE_URL="sqlite+aiosqlite:///test_integration.db"
    
    if ! python -m src.cli config validate > /dev/null; then
        error "CLI config validation failed"
        exit 1
    fi
    success "CLI config validation passed"
}

validate_background_tasks() {
    log "Validating background tasks..."
    
    cd "$PROJECT_ROOT"
    source "$VENV_PATH/bin/activate"
    
    # Test task managers
    if ! python -c "
import asyncio
from src.config.settings import get_settings
from src.tasks.cleanup import get_cleanup_manager
from src.tasks.monitoring import get_monitoring_manager
from src.tasks.backup import get_backup_manager

async def test_tasks():
    settings = get_settings()
    settings.database_url = 'sqlite+aiosqlite:///test_integration.db'
    
    # Test cleanup manager
    cleanup_manager = get_cleanup_manager(settings)
    cleanup_stats = cleanup_manager.get_stats()
    print(f'Cleanup manager initialized: {\"manager\" in cleanup_stats}')
    
    # Test monitoring manager
    monitoring_manager = get_monitoring_manager(settings)
    monitoring_stats = monitoring_manager.get_stats()
    print(f'Monitoring manager initialized: {\"manager\" in monitoring_stats}')
    
    # Test backup manager
    backup_manager = get_backup_manager(settings)
    backup_stats = backup_manager.get_stats()
    print(f'Backup manager initialized: {\"manager\" in backup_stats}')
    
    print('Background tasks validation passed')

asyncio.run(test_tasks())
"; then
        error "Background tasks validation failed"
        exit 1
    fi
    success "Background tasks validation passed"
}

run_integration_tests() {
    log "Running integration tests..."
    
    cd "$PROJECT_ROOT"
    source "$VENV_PATH/bin/activate"
    
    # Set test environment
    export WIFI_DENSEPOSE_ENVIRONMENT=test
    export WIFI_DENSEPOSE_DATABASE_URL="sqlite+aiosqlite:///test_integration.db"
    
    # Run integration tests
    if ! python -m pytest tests/integration/ -v --tb=short; then
        error "Integration tests failed"
        exit 1
    fi
    success "Integration tests passed"
}

validate_package_build() {
    log "Validating package build..."
    
    cd "$PROJECT_ROOT"
    source "$VENV_PATH/bin/activate"
    
    # Install build tools
    pip install build twine
    
    # Build package
    if ! python -m build; then
        error "Package build failed"
        exit 1
    fi
    success "Package build passed"
    
    # Check package
    if ! python -m twine check dist/*; then
        error "Package check failed"
        exit 1
    fi
    success "Package check passed"
    
    # Clean up build artifacts
    rm -rf build/ dist/ *.egg-info/
}

generate_report() {
    log "Generating integration report..."
    
    cat > "$PROJECT_ROOT/integration_report.md" << EOF
# WiFi-DensePose Integration Validation Report

**Date:** $(date)
**Status:** ✅ PASSED

## Validation Results

### Prerequisites
- ✅ Python version check
- ✅ Virtual environment setup
- ✅ Dependencies installation

### Package Structure
- ✅ Required files present
- ✅ Directory structure valid
- ✅ Python imports working

### Core Components
- ✅ Configuration management
- ✅ Database integration
- ✅ API endpoints
- ✅ CLI interface
- ✅ Background tasks

### Testing
- ✅ Integration tests passed
- ✅ Package build successful

## System Information

**Python Version:** $(python --version)
**Package Version:** $(python -c "import src; print(src.__version__)")
**Environment:** $(python -c "from src.config.settings import get_settings; print(get_settings().environment)")

## Next Steps

The WiFi-DensePose system has been successfully integrated and validated.
You can now:

1. Start the server: \`wifi-densepose start\`
2. Check status: \`wifi-densepose status\`
3. View configuration: \`wifi-densepose config show\`
4. Run tests: \`pytest tests/\`

For more information, see the documentation in the \`docs/\` directory.
EOF

    success "Integration report generated: integration_report.md"
}

main() {
    log "Starting WiFi-DensePose integration validation..."
    
    # Trap cleanup on exit
    trap cleanup EXIT
    
    # Run validation steps
    check_prerequisites
    validate_package_structure
    validate_imports
    validate_configuration
    validate_database
    validate_api_endpoints
    validate_cli
    validate_background_tasks
    run_integration_tests
    validate_package_build
    generate_report
    
    success "🎉 All integration validations passed!"
    log "Integration validation completed successfully"
}

# Run main function
main "$@"