#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Setting up RuVector Mathpix Development Environment${NC}"
echo ""

# Check if Rust is installed
if ! command -v rustc &> /dev/null; then
    echo -e "${RED}Rust is not installed. Installing Rust...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
else
    echo -e "${GREEN}Rust is already installed: $(rustc --version)${NC}"
fi

# Update Rust toolchain
echo -e "${BLUE}Updating Rust toolchain...${NC}"
rustup update stable
rustup default stable

# Install required components
echo -e "${BLUE}Installing Rust components...${NC}"
rustup component add rustfmt clippy

# Install development tools
echo -e "${BLUE}Installing development tools...${NC}"

# Code coverage
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo -e "${YELLOW}Installing cargo-tarpaulin...${NC}"
    cargo install cargo-tarpaulin
else
    echo -e "${GREEN}cargo-tarpaulin is already installed${NC}"
fi

# Security audit
if ! command -v cargo-audit &> /dev/null; then
    echo -e "${YELLOW}Installing cargo-audit...${NC}"
    cargo install cargo-audit
else
    echo -e "${GREEN}cargo-audit is already installed${NC}"
fi

# Dependency checker
if ! command -v cargo-deny &> /dev/null; then
    echo -e "${YELLOW}Installing cargo-deny...${NC}"
    cargo install cargo-deny
else
    echo -e "${GREEN}cargo-deny is already installed${NC}"
fi

# License checker
if ! command -v cargo-license &> /dev/null; then
    echo -e "${YELLOW}Installing cargo-license...${NC}"
    cargo install cargo-license
else
    echo -e "${GREEN}cargo-license is already installed${NC}"
fi

# WASM tools
if ! command -v wasm-pack &> /dev/null; then
    echo -e "${YELLOW}Installing wasm-pack...${NC}"
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
else
    echo -e "${GREEN}wasm-pack is already installed${NC}"
fi

# Benchmark comparison tool
if ! command -v critcmp &> /dev/null; then
    echo -e "${YELLOW}Installing critcmp...${NC}"
    cargo install critcmp
else
    echo -e "${GREEN}critcmp is already installed${NC}"
fi

# Cargo watch for development
if ! command -v cargo-watch &> /dev/null; then
    echo -e "${YELLOW}Installing cargo-watch...${NC}"
    cargo install cargo-watch
else
    echo -e "${GREEN}cargo-watch is already installed${NC}"
fi

# Flamegraph for profiling
if ! command -v cargo-flamegraph &> /dev/null; then
    echo -e "${YELLOW}Installing cargo-flamegraph...${NC}"
    cargo install flamegraph
else
    echo -e "${GREEN}cargo-flamegraph is already installed${NC}"
fi

# Binary size analysis
if ! command -v cargo-bloat &> /dev/null; then
    echo -e "${YELLOW}Installing cargo-bloat...${NC}"
    cargo install cargo-bloat
else
    echo -e "${GREEN}cargo-bloat is already installed${NC}"
fi

# Outdated dependency checker
if ! command -v cargo-outdated &> /dev/null; then
    echo -e "${YELLOW}Installing cargo-outdated...${NC}"
    cargo install cargo-outdated
else
    echo -e "${GREEN}cargo-outdated is already installed${NC}"
fi

# Install WASM target
echo -e "${BLUE}Installing WASM target...${NC}"
rustup target add wasm32-unknown-unknown

# Install Node.js if not present (for WASM testing)
if ! command -v node &> /dev/null; then
    echo -e "${YELLOW}Node.js not found. Please install Node.js for WASM testing.${NC}"
    echo -e "${YELLOW}Visit: https://nodejs.org/${NC}"
else
    echo -e "${GREEN}Node.js is installed: $(node --version)${NC}"
fi

# Create necessary directories
echo -e "${BLUE}Creating project directories...${NC}"
mkdir -p models
mkdir -p benchmarks/results
mkdir -p coverage
mkdir -p docs
mkdir -p .github/workflows

# Download test models
echo -e "${BLUE}Downloading test models...${NC}"
if [ -f "./scripts/download_models.sh" ]; then
    chmod +x ./scripts/download_models.sh
    ./scripts/download_models.sh
else
    echo -e "${YELLOW}Model download script not found. Skipping model download.${NC}"
fi

# Initialize git hooks (if in git repo)
if [ -d ".git" ]; then
    echo -e "${BLUE}Setting up git hooks...${NC}"

    # Pre-commit hook
    cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
echo "Running pre-commit checks..."

# Format check
cargo fmt --check
if [ $? -ne 0 ]; then
    echo "Code formatting check failed. Run 'cargo fmt' to fix."
    exit 1
fi

# Clippy
cargo clippy -- -D warnings
if [ $? -ne 0 ]; then
    echo "Clippy check failed."
    exit 1
fi

# Tests
cargo test
if [ $? -ne 0 ]; then
    echo "Tests failed."
    exit 1
fi

echo "Pre-commit checks passed!"
EOF
    chmod +x .git/hooks/pre-commit
    echo -e "${GREEN}Git hooks installed${NC}"
fi

# Build the project
echo -e "${BLUE}Building project...${NC}"
cargo build

# Run tests
echo -e "${BLUE}Running tests...${NC}"
cargo test

echo ""
echo -e "${GREEN}====================================${NC}"
echo -e "${GREEN}Development environment setup complete!${NC}"
echo -e "${GREEN}====================================${NC}"
echo ""
echo -e "${BLUE}Available commands:${NC}"
echo -e "  ${GREEN}make help${NC}        - Show all available make commands"
echo -e "  ${GREEN}make build${NC}       - Build the project"
echo -e "  ${GREEN}make test${NC}        - Run tests"
echo -e "  ${GREEN}make bench${NC}       - Run benchmarks"
echo -e "  ${GREEN}make coverage${NC}    - Generate coverage report"
echo -e "  ${GREEN}make wasm${NC}        - Build WASM package"
echo -e "  ${GREEN}make watch${NC}       - Watch for changes and rebuild"
echo ""
echo -e "${BLUE}Quick start:${NC}"
echo -e "  1. Run ${GREEN}make test${NC} to verify everything works"
echo -e "  2. Run ${GREEN}make bench${NC} to see baseline performance"
echo -e "  3. Run ${GREEN}make coverage${NC} to check test coverage"
echo ""
echo -e "${GREEN}Happy coding!${NC}"
