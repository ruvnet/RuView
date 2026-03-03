#!/usr/bin/env bash
set -e

echo "Setting up development environment..."

# Install Rust if not present
if ! command -v rustc &> /dev/null
then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

# Install system dependencies
echo "Installing system dependencies..."
apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev

echo "Environment setup complete!"
