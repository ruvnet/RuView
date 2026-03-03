#!/usr/bin/env bash
set -e

echo "Installing Midstream application..."

# Create project structure
mkdir -p src/tests

# Install hyprstream
cargo add hyprstream

# Install test dependencies
cargo add --dev mockall tokio

# Install dependencies and build project
cargo build

# Run tests
cargo test

echo "Installation complete!"
