#!/bin/bash
# Install profiling and benchmarking tools

set -e

echo "Installing Ruvector profiling tools..."

# Install perf (Linux performance tools)
if ! command -v perf &> /dev/null; then
    echo "Installing perf..."
    sudo apt-get update
    sudo apt-get install -y linux-tools-common linux-tools-generic linux-tools-$(uname -r) || true
fi

# Install valgrind
if ! command -v valgrind &> /dev/null; then
    echo "Installing valgrind..."
    sudo apt-get install -y valgrind
fi

# Install heaptrack
if ! command -v heaptrack &> /dev/null; then
    echo "Installing heaptrack..."
    sudo apt-get install -y heaptrack || echo "heaptrack not available, skipping..."
fi

# Install flamegraph tools
if ! command -v cargo-flamegraph &> /dev/null; then
    echo "Installing cargo-flamegraph..."
    cargo install flamegraph
fi

# Install cargo benchmarking tools
if ! command -v cargo-criterion &> /dev/null; then
    echo "Installing cargo-criterion..."
    cargo install cargo-criterion || echo "cargo-criterion installation failed, using built-in criterion"
fi

# Install hyperfine for command-line benchmarking
if ! command -v hyperfine &> /dev/null; then
    echo "Installing hyperfine..."
    cargo install hyperfine
fi

# Install cargo-bench-cmp for comparing benchmarks
if ! command -v cargo-bench-cmp &> /dev/null; then
    echo "Installing cargo-bench-cmp..."
    cargo install cargo-bench-cmp || echo "cargo-bench-cmp not available, skipping..."
fi

echo "✅ Profiling tools installation complete!"
echo ""
echo "Available tools:"
echo "  - perf: CPU profiling"
echo "  - valgrind: Memory profiling"
echo "  - heaptrack: Heap profiling"
echo "  - cargo-flamegraph: Flamegraph generation"
echo "  - hyperfine: Command-line benchmarking"
echo ""
echo "Note: Some tools may require sudo privileges to run."
