#!/bin/bash
#
# RuVector Setup Script for RHEL/CentOS/Fedora
# Installs all required dependencies for building RuVector
#
set -e

echo "RuVector Dependency Setup for RHEL/CentOS/Fedora"
echo "================================================="
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    SUDO="sudo"
else
    SUDO=""
fi

# Detect distro
if [ -f /etc/os-release ]; then
    . /etc/os-release
    DISTRO="$ID"
    VERSION="$VERSION_ID"
else
    DISTRO="unknown"
fi

echo "Detected: $DISTRO $VERSION"

# Determine package manager
if command -v dnf &> /dev/null; then
    PKG_MGR="dnf"
elif command -v yum &> /dev/null; then
    PKG_MGR="yum"
else
    echo "Error: Neither dnf nor yum found"
    exit 1
fi

# Install EPEL if needed (for CentOS/RHEL)
if [[ "$DISTRO" == "centos" || "$DISTRO" == "rhel" ]]; then
    echo "Installing EPEL repository..."
    $SUDO $PKG_MGR install -y epel-release
fi

# Install development tools
echo "Installing development tools..."
$SUDO $PKG_MGR groupinstall -y "Development Tools"
$SUDO $PKG_MGR install -y \
    openssl-devel \
    clang \
    clang-devel \
    llvm-devel \
    cmake \
    git \
    curl \
    ca-certificates

# Determine PostgreSQL version to install
PG_VERSION="${1:-16}"
echo "Setting up PostgreSQL $PG_VERSION..."

# Add PostgreSQL repository
if ! $PKG_MGR repolist | grep -q pgdg; then
    echo "Adding PostgreSQL repository..."
    $SUDO $PKG_MGR install -y \
        "https://download.postgresql.org/pub/repos/yum/reporpms/EL-${VERSION%%.*}-x86_64/pgdg-redhat-repo-latest.noarch.rpm"
fi

# Disable built-in PostgreSQL module (for RHEL 8+)
if [[ "$VERSION" =~ ^8 || "$VERSION" =~ ^9 ]]; then
    $SUDO dnf -qy module disable postgresql 2>/dev/null || true
fi

# Install PostgreSQL
echo "Installing PostgreSQL $PG_VERSION..."
$SUDO $PKG_MGR install -y \
    "postgresql${PG_VERSION}-server" \
    "postgresql${PG_VERSION}-devel"

# Initialize PostgreSQL if needed
if [ ! -f "/var/lib/pgsql/${PG_VERSION}/data/postgresql.conf" ]; then
    echo "Initializing PostgreSQL database..."
    $SUDO "/usr/pgsql-${PG_VERSION}/bin/postgresql-${PG_VERSION}-setup" initdb
fi

# Start PostgreSQL
echo "Starting PostgreSQL..."
$SUDO systemctl enable "postgresql-${PG_VERSION}"
$SUDO systemctl start "postgresql-${PG_VERSION}"

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
cargo pgrx init --pg$PG_VERSION "/usr/pgsql-${PG_VERSION}/bin/pg_config"

echo ""
echo "================================================="
echo "Setup complete!"
echo ""
echo "You can now build RuVector with:"
echo "  cd /path/to/ruvector"
echo "  ./install/install.sh --build-from-source --pg-version $PG_VERSION"
echo ""
