#!/bin/bash
#
# RuVector Setup Script for Debian/Ubuntu
# Installs all required dependencies for building RuVector
#
set -e

echo "RuVector Dependency Setup for Debian/Ubuntu"
echo "============================================"
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    SUDO="sudo"
else
    SUDO=""
fi

# Update package lists
echo "Updating package lists..."
$SUDO apt-get update

# Install basic build tools
echo "Installing build tools..."
$SUDO apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libclang-dev \
    clang \
    cmake \
    git \
    curl \
    ca-certificates

# Determine PostgreSQL version to install
PG_VERSION="${1:-16}"
echo "Setting up PostgreSQL $PG_VERSION..."

# Add PostgreSQL repository
if ! grep -q "apt.postgresql.org" /etc/apt/sources.list.d/*.list 2>/dev/null; then
    echo "Adding PostgreSQL APT repository..."
    $SUDO install -d /usr/share/postgresql-common/pgdg
    $SUDO curl -o /usr/share/postgresql-common/pgdg/apt.postgresql.org.asc --fail \
        https://www.postgresql.org/media/keys/ACCC4CF8.asc
    $SUDO sh -c 'echo "deb [signed-by=/usr/share/postgresql-common/pgdg/apt.postgresql.org.asc] \
        https://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" > \
        /etc/apt/sources.list.d/pgdg.list'
    $SUDO apt-get update
fi

# Install PostgreSQL
echo "Installing PostgreSQL $PG_VERSION..."
$SUDO apt-get install -y \
    "postgresql-$PG_VERSION" \
    "postgresql-server-dev-$PG_VERSION"

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
cargo pgrx init --pg$PG_VERSION "/usr/lib/postgresql/$PG_VERSION/bin/pg_config"

echo ""
echo "============================================"
echo "Setup complete!"
echo ""
echo "You can now build RuVector with:"
echo "  cd /path/to/ruvector"
echo "  ./install/install.sh --build-from-source --pg-version $PG_VERSION"
echo ""
