#!/bin/bash
# RuVector Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/ruvnet/ruvector/main/install.sh | bash
# Or:    wget -qO- https://raw.githubusercontent.com/ruvnet/ruvector/main/install.sh | bash

# Don't exit on error - we handle errors manually
set +e

# Colors (disable if not a terminal)
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    CYAN='\033[0;36m'
    NC='\033[0m'
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    CYAN=''
    NC=''
fi

print_banner() {
    echo -e "${CYAN}"
    echo "  ____        __     __        _             "
    echo " |  _ \ _   _ \ \   / /__  ___| |_ ___  _ __ "
    echo " | |_) | | | | \ \ / / _ \/ __| __/ _ \| '__|"
    echo " |  _ <| |_| |  \ V /  __/ (__| || (_) | |   "
    echo " |_| \_\\\\__,_|   \_/ \___|\___|\__\___/|_|   "
    echo -e "${NC}"
    echo -e "${YELLOW}Vector database that learns${NC}"
    echo ""
}

print_step() {
    echo -e "${BLUE}==>${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}!${NC} $1"
}

# Detect OS and architecture
detect_platform() {
    OS="$(uname -s 2>/dev/null || echo "Unknown")"
    ARCH="$(uname -m 2>/dev/null || echo "Unknown")"

    case "$OS" in
        Linux*)     PLATFORM="linux" ;;
        Darwin*)    PLATFORM="darwin" ;;
        MINGW*|MSYS*|CYGWIN*)
            PLATFORM="windows"
            print_warning "Windows detected. For best results, use WSL2 or run in Git Bash."
            print_warning "Native Windows: use 'cargo install' manually after installing Rust."
            ;;
        *)          PLATFORM="unknown" ;;
    esac

    case "$ARCH" in
        x86_64|amd64)   ARCH="x64" ;;
        aarch64|arm64)  ARCH="arm64" ;;
        *)              ARCH="unknown" ;;
    esac

    echo "${PLATFORM}-${ARCH}"
}

# Check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check for required build dependencies
check_dependencies() {
    print_step "Checking dependencies..."

    local missing=()

    # curl or wget required
    if ! command_exists curl && ! command_exists wget; then
        missing+=("curl or wget")
    fi

    # Check for C compiler (needed for Rust builds)
    if ! command_exists cc && ! command_exists gcc && ! command_exists clang; then
        missing+=("C compiler (gcc/clang)")
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        print_warning "Missing optional dependencies: ${missing[*]}"
        echo "  Some crates may fail to compile without a C compiler."

        if [ "$PLATFORM" = "linux" ]; then
            echo "  Install with: sudo apt-get install build-essential (Debian/Ubuntu)"
            echo "             or: sudo yum groupinstall 'Development Tools' (RHEL/CentOS)"
        elif [ "$PLATFORM" = "darwin" ]; then
            echo "  Install with: xcode-select --install"
        fi
        echo ""
    else
        print_success "All dependencies found"
    fi
}

# Install Rust if not present
install_rust() {
    if command_exists rustc; then
        RUST_VERSION=$(rustc --version | cut -d' ' -f2)
        print_success "Rust ${RUST_VERSION} already installed"
        return 0
    fi

    print_step "Installing Rust via rustup..."

    if ! command_exists curl; then
        print_error "curl is required to install Rust"
        echo "  Install curl first, then re-run this script"
        return 1
    fi

    if curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; then
        # Source cargo env if it exists
        if [ -f "$HOME/.cargo/env" ]; then
            . "$HOME/.cargo/env"
        elif [ -f "$HOME/.cargo/bin/cargo" ]; then
            export PATH="$HOME/.cargo/bin:$PATH"
        fi

        if command_exists rustc; then
            print_success "Rust installed successfully"
            return 0
        else
            print_error "Rust installation completed but rustc not found in PATH"
            echo "  Try running: source \$HOME/.cargo/env"
            return 1
        fi
    else
        print_error "Failed to install Rust"
        return 1
    fi
}

# Install CLI binary from crates.io
install_cli() {
    print_step "Installing ruvector-cli from crates.io..."

    if ! command_exists cargo; then
        print_error "cargo not found. Install Rust first."
        return 1
    fi

    # Install with visible output so users can see progress/errors
    echo "  Running: cargo install ruvector-cli"
    if cargo install ruvector-cli 2>&1 | while read -r line; do echo "    $line"; done; then
        if command_exists ruvector-cli; then
            print_success "ruvector-cli installed"
            return 0
        fi
    fi

    # Check if it was already installed
    if command_exists ruvector-cli; then
        print_success "ruvector-cli already installed"
        return 0
    fi

    print_warning "ruvector-cli installation had issues"
    echo "  This may be due to missing build dependencies."
    echo "  Try manually: cargo install ruvector-cli"
    return 1
}

# Install Node.js packages
install_npm() {
    if ! command_exists node; then
        print_warning "Node.js not found - skipping npm packages"
        echo "  Install Node.js from https://nodejs.org to use npm packages"
        return 0
    fi

    print_step "Installing npm packages..."

    if ! command_exists npm; then
        print_warning "npm not found"
        return 1
    fi

    NODE_VERSION=$(node --version)
    print_success "Node.js ${NODE_VERSION} detected"

    # Install with visible output
    echo "  Running: npm install -g ruvector"
    if npm install -g ruvector 2>&1 | while read -r line; do echo "    $line"; done; then
        print_success "ruvector npm package installed"
    else
        print_warning "npm install had issues (may need sudo on some systems)"
        echo "  Try: sudo npm install -g ruvector"
        echo "  Or use npx: npx ruvector"
    fi
}

# Show available crates (informational - these are LIBRARY crates)
show_crates() {
    echo ""
    echo -e "${CYAN}Available RuVector Rust Crates:${NC}"
    echo ""
    echo -e "${GREEN}Add to your Cargo.toml with 'cargo add':${NC}"
    echo ""
    echo "  # Core (vector database)"
    echo "  cargo add ruvector-core"
    echo ""
    echo "  # Graph database with Cypher"
    echo "  cargo add ruvector-graph"
    echo ""
    echo "  # Graph Neural Networks"
    echo "  cargo add ruvector-gnn"
    echo ""
    echo "  # Distributed systems"
    echo "  cargo add ruvector-cluster"
    echo "  cargo add ruvector-raft"
    echo "  cargo add ruvector-replication"
    echo ""
    echo "  # AI routing"
    echo "  cargo add ruvector-tiny-dancer-core"
    echo "  cargo add ruvector-router-core"
    echo ""
    echo -e "${YELLOW}Note: These are library crates. Use 'cargo add' in your project.${NC}"
    echo ""
}

# Show npm packages
show_npm() {
    echo -e "${CYAN}Available npm Packages:${NC}"
    echo ""
    echo "  # All-in-one CLI (recommended)"
    echo "  npm install ruvector"
    echo "  npx ruvector"
    echo ""
    echo "  # Individual packages"
    echo "  npm install @ruvector/core       # Vector database"
    echo "  npm install @ruvector/gnn        # Graph Neural Networks"
    echo "  npm install @ruvector/graph-node # Hypergraph database"
    echo ""
    echo "  # List all available packages"
    echo "  npx ruvector install"
    echo ""
}

# Show uninstall instructions
show_uninstall() {
    echo -e "${CYAN}Uninstall Instructions:${NC}"
    echo ""
    echo "  # Remove CLI binary"
    echo "  cargo uninstall ruvector-cli"
    echo ""
    echo "  # Remove npm packages"
    echo "  npm uninstall -g ruvector"
    echo ""
    echo "  # Remove Rust entirely (optional)"
    echo "  rustup self uninstall"
    echo ""
}

# Main installation
main() {
    print_banner

    PLATFORM=$(detect_platform)
    print_step "Detected platform: ${PLATFORM}"
    echo ""

    # Parse arguments
    INSTALL_RUST=true
    INSTALL_CLI=true
    INSTALL_NPM=true
    SHOW_ONLY=false
    SHOW_UNINSTALL=false

    while [[ $# -gt 0 ]]; do
        case $1 in
            --rust-only)
                INSTALL_NPM=false
                shift
                ;;
            --npm-only)
                INSTALL_RUST=false
                INSTALL_CLI=false
                shift
                ;;
            --cli-only)
                INSTALL_NPM=false
                shift
                ;;
            --list|--show)
                SHOW_ONLY=true
                shift
                ;;
            --uninstall)
                SHOW_UNINSTALL=true
                shift
                ;;
            --help|-h)
                echo "Usage: install.sh [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --rust-only    Install Rust and CLI only (no npm)"
                echo "  --npm-only     Install npm packages only (no Rust)"
                echo "  --cli-only     Install ruvector-cli binary only"
                echo "  --list         Show available packages without installing"
                echo "  --uninstall    Show uninstall instructions"
                echo "  --help         Show this help"
                echo ""
                echo "Examples:"
                echo "  curl -fsSL https://raw.githubusercontent.com/ruvnet/ruvector/main/install.sh | bash"
                echo "  curl -fsSL ... | bash -s -- --rust-only"
                echo "  curl -fsSL ... | bash -s -- --npm-only"
                echo "  curl -fsSL ... | bash -s -- --list"
                exit 0
                ;;
            *)
                print_error "Unknown option: $1"
                echo "Use --help for usage information"
                exit 1
                ;;
        esac
    done

    if [ "$SHOW_UNINSTALL" = true ]; then
        show_uninstall
        exit 0
    fi

    if [ "$SHOW_ONLY" = true ]; then
        show_crates
        show_npm
        exit 0
    fi

    # Check dependencies
    check_dependencies

    # Track what was installed
    INSTALLED_RUST=false
    INSTALLED_CLI=false
    INSTALLED_NPM=false

    # Install Rust
    if [ "$INSTALL_RUST" = true ]; then
        if install_rust; then
            INSTALLED_RUST=true
        fi
        echo ""
    fi

    # Install CLI
    if [ "$INSTALL_CLI" = true ] && { [ "$INSTALLED_RUST" = true ] || command_exists cargo; }; then
        if install_cli; then
            INSTALLED_CLI=true
        fi
        echo ""
    fi

    # Install npm
    if [ "$INSTALL_NPM" = true ]; then
        if install_npm; then
            INSTALLED_NPM=true
        fi
        echo ""
    fi

    # Show summary
    echo ""
    echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  Installation Summary${NC}"
    echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
    echo ""

    if [ "$INSTALLED_RUST" = true ] || command_exists rustc; then
        RUST_V=$(rustc --version 2>/dev/null | cut -d' ' -f2 || echo "installed")
        echo -e "  Rust:         ${GREEN}✓${NC} ${RUST_V}"
    else
        echo -e "  Rust:         ${YELLOW}○${NC} not installed"
    fi

    if [ "$INSTALLED_CLI" = true ] || command_exists ruvector-cli; then
        echo -e "  ruvector-cli: ${GREEN}✓${NC} installed"
    else
        echo -e "  ruvector-cli: ${YELLOW}○${NC} not installed"
    fi

    if command_exists node; then
        NODE_V=$(node --version)
        echo -e "  Node.js:      ${GREEN}✓${NC} ${NODE_V}"
        if npm list -g ruvector >/dev/null 2>&1; then
            echo -e "  ruvector npm: ${GREEN}✓${NC} installed"
        else
            echo -e "  ruvector npm: ${YELLOW}○${NC} use 'npx ruvector'"
        fi
    else
        echo -e "  Node.js:      ${YELLOW}○${NC} not installed"
    fi

    echo ""

    show_crates
    show_npm

    echo -e "${CYAN}Quick Start:${NC}"
    echo ""
    echo "  # Rust project"
    echo "  cargo new my-vector-app && cd my-vector-app"
    echo "  cargo add ruvector-core ruvector-gnn"
    echo ""
    echo "  # Node.js"
    echo "  npx ruvector info"
    echo "  npx ruvector benchmark"
    echo ""
    echo -e "${CYAN}Documentation:${NC} https://github.com/ruvnet/ruvector"
    echo -e "${CYAN}Issues:${NC}        https://github.com/ruvnet/ruvector/issues"
    echo ""
}

main "$@"
