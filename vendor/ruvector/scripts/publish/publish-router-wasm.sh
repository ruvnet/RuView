#!/bin/bash
# Script to publish router-wasm v0.1.1 to crates.io
# This script waits for router-core v0.1.1 to be available

set -e

echo "=========================================="
echo "router-wasm v0.1.1 Publication Script"
echo "=========================================="
echo ""

# Load environment variables
if [ -f /workspaces/ruvector/.env ]; then
    echo "✓ Loading CRATES_API_KEY from .env..."
    export $(grep "^CRATES_API_KEY=" /workspaces/ruvector/.env | xargs)
else
    echo "✗ Error: .env file not found"
    exit 1
fi

if [ -z "$CRATES_API_KEY" ]; then
    echo "✗ Error: CRATES_API_KEY not found in .env"
    exit 1
fi

echo "✓ CRATES_API_KEY loaded"
echo ""

# Step 1: Wait for router-core v0.1.1
echo "Step 1: Checking for router-core v0.1.1..."
MAX_ATTEMPTS=30
ATTEMPT=0

while [ $ATTEMPT -lt $MAX_ATTEMPTS ]; do
    ATTEMPT=$((ATTEMPT + 1))
    echo "  Check $ATTEMPT/$MAX_ATTEMPTS ($(date +%H:%M:%S))"

    if cargo search router-core 2>&1 | grep -q "router-core.*0\.1\.1"; then
        echo "✓ router-core v0.1.1 found on crates.io!"
        break
    fi

    if [ $ATTEMPT -eq $MAX_ATTEMPTS ]; then
        echo "✗ Timeout: router-core v0.1.1 not found after $MAX_ATTEMPTS attempts"
        echo "  Current version: $(cargo search router-core 2>&1 | grep "router-core =" | head -1)"
        exit 1
    fi

    sleep 10
done

echo ""

# Step 2: Login to crates.io
echo "Step 2: Logging in to crates.io..."
cargo login "$CRATES_API_KEY"
echo "✓ Successfully logged in"
echo ""

# Step 3: Navigate to router-wasm directory
echo "Step 3: Navigating to router-wasm directory..."
cd /workspaces/ruvector/crates/router-wasm
echo "✓ Current directory: $(pwd)"
echo ""

# Step 4: Verify package
echo "Step 4: Verifying package..."
cargo package --list --allow-dirty | head -20
echo "..."
echo ""

# Step 5: Publish
echo "Step 5: Publishing router-wasm v0.1.1..."
echo ""
cargo publish --allow-dirty

echo ""
echo "=========================================="
echo "✓ SUCCESS: router-wasm v0.1.1 published!"
echo "=========================================="
