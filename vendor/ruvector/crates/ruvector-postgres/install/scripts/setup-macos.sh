#!/bin/bash
#
# RuVector Setup Script for macOS
# Installs all required dependencies for building RuVector
#
set -e

echo "RuVector Dependency Setup for macOS"
echo "===================================="
echo ""

# Check for Homebrew
if ! command -v brew &> /dev/null; then
    echo "Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

    # Add to PATH for Apple Silicon Macs
    if [ -f "/opt/homebrew/bin/brew" ]; then
        eval "$(/opt/homebrew/bin/brew shellenv)"
    fi
fi

# Update Homebrew
echo "Updating Homebrew..."
brew update

# Install build tools
echo "Installing build tools..."
brew install \
    pkg-config \
    openssl \
    cmake \
    git \
    curl

# Determine PostgreSQL version to install
PG_VERSION="${1:-16}"
echo "Setting up PostgreSQL $PG_VERSION..."

# Install PostgreSQL
echo "Installing PostgreSQL $PG_VERSION..."
brew install "postgresql@$PG_VERSION"

# Link PostgreSQL
brew link "postgresql@$PG_VERSION" --force 2>/dev/null || true

# Add PostgreSQL to PATH
PG_PATH="/opt/homebrew/opt/postgresql@$PG_VERSION/bin"
if [ ! -d "$PG_PATH" ]; then
    PG_PATH="/usr/local/opt/postgresql@$PG_VERSION/bin"
fi

export PATH="$PG_PATH:$PATH"

# Start PostgreSQL service
echo "Starting PostgreSQL service..."
brew services start "postgresql@$PG_VERSION"

# Install Rust if not present
if ! command -v rustc &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# Install cargo-pgrx
echo "Installing cargo-pgrx..."
cargo install cargo-pgrx --version "0.12.9" --locked

# Initialize pgrx
echo "Initializing pgrx for PostgreSQL $PG_VERSION..."
cargo pgrx init --pg$PG_VERSION "$PG_PATH/pg_config"

echo ""
echo "===================================="
echo "Setup complete!"
echo ""
echo "Add PostgreSQL to your PATH:"
echo "  export PATH=\"$PG_PATH:\$PATH\""
echo ""
echo "You can now build RuVector with:"
echo "  cd /path/to/ruvector"
echo "  ./install/install.sh --build-from-source --pg-version $PG_VERSION"
echo ""
