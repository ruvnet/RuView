#!/bin/bash
#
# RuVector Quick Start Installer
# Auto-detects platform and runs appropriate setup
#
# Usage: curl -sSL https://raw.githubusercontent.com/ruvnet/ruvector/main/install/quick-start.sh | bash
#    or: ./quick-start.sh [PG_VERSION]
#
set -e

PG_VERSION="${1:-16}"

echo ""
echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║              RuVector Quick Start Installer                   ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo ""

# Detect OS
detect_os() {
    if [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
    elif [ -f /etc/debian_version ]; then
        echo "debian"
    elif [ -f /etc/redhat-release ]; then
        echo "rhel"
    else
        echo "unknown"
    fi
}

OS=$(detect_os)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" 2>/dev/null && pwd)" || SCRIPT_DIR="."

echo "Detected OS: $OS"
echo "PostgreSQL version: $PG_VERSION"
echo ""

case "$OS" in
    debian)
        echo "Running Debian/Ubuntu setup..."
        if [ -f "$SCRIPT_DIR/scripts/setup-debian.sh" ]; then
            bash "$SCRIPT_DIR/scripts/setup-debian.sh" "$PG_VERSION"
        else
            echo "Downloading setup script..."
            curl -sSL https://raw.githubusercontent.com/ruvnet/ruvector/main/install/scripts/setup-debian.sh | bash -s "$PG_VERSION"
        fi
        ;;
    rhel)
        echo "Running RHEL/CentOS setup..."
        if [ -f "$SCRIPT_DIR/scripts/setup-rhel.sh" ]; then
            bash "$SCRIPT_DIR/scripts/setup-rhel.sh" "$PG_VERSION"
        else
            echo "Downloading setup script..."
            curl -sSL https://raw.githubusercontent.com/ruvnet/ruvector/main/install/scripts/setup-rhel.sh | bash -s "$PG_VERSION"
        fi
        ;;
    macos)
        echo "Running macOS setup..."
        if [ -f "$SCRIPT_DIR/scripts/setup-macos.sh" ]; then
            bash "$SCRIPT_DIR/scripts/setup-macos.sh" "$PG_VERSION"
        else
            echo "Downloading setup script..."
            curl -sSL https://raw.githubusercontent.com/ruvnet/ruvector/main/install/scripts/setup-macos.sh | bash -s "$PG_VERSION"
        fi
        ;;
    *)
        echo "Unsupported OS. Please install dependencies manually."
        echo ""
        echo "Required dependencies:"
        echo "  - Rust (rustup.rs)"
        echo "  - PostgreSQL $PG_VERSION with development headers"
        echo "  - Build tools (gcc/clang, make, pkg-config)"
        echo "  - cargo-pgrx (cargo install cargo-pgrx)"
        exit 1
        ;;
esac

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo ""
echo "Dependencies installed! Now clone and build RuVector:"
echo ""
echo "  git clone https://github.com/ruvnet/ruvector.git"
echo "  cd ruvector"
echo "  ./install/install.sh --build-from-source --pg-version $PG_VERSION"
echo ""
echo "Or for a dry run first:"
echo "  ./install/install.sh --build-from-source --dry-run --verbose"
echo ""
