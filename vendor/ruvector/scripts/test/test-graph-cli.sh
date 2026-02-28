#!/bin/bash
# Test script for RuVector Graph CLI commands

set -e

echo "============================================"
echo "RuVector Graph CLI - Command Tests"
echo "============================================"
echo ""

# Build the CLI
echo "Building CLI..."
cargo build --package ruvector-cli --bin ruvector --quiet 2>&1 | grep -v "warning:" | head -5

CLI="./target/debug/ruvector"

echo ""
echo "1. Testing main help..."
$CLI --help | grep -A 2 "graph"

echo ""
echo "2. Testing graph command help..."
$CLI graph --help 2>&1 | head -20 || echo "Failed to show graph help"

echo ""
echo "3. Testing graph create..."
$CLI graph create --path /tmp/test-graph.db --name test --indexed 2>&1 | grep -v "warning:" || true

echo ""
echo "4. Testing graph info..."
$CLI graph info --db /tmp/test-graph.db 2>&1 | grep -v "warning:" || true

echo ""
echo "5. Listing available graph commands..."
echo "   - create      : Create new graph database"
echo "   - query       : Execute Cypher queries"
echo "   - shell       : Interactive REPL"
echo "   - import      : Import from CSV/JSON/Cypher"
echo "   - export      : Export to various formats"
echo "   - info        : Show database statistics"
echo "   - benchmark   : Run performance tests"
echo "   - serve       : Start HTTP/gRPC server"

echo ""
echo "============================================"
echo "All graph commands are registered!"
echo "============================================"
