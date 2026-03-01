#!/bin/bash
# Download ANN benchmark datasets (SIFT1M, GIST1M, Deep1M)

set -e

DATASETS_DIR="datasets"
mkdir -p "$DATASETS_DIR"

echo "╔════════════════════════════════════════╗"
echo "║   ANN Benchmark Dataset Downloader    ║"
echo "╚════════════════════════════════════════╝"
echo ""

# Function to download and extract dataset
download_dataset() {
    local name=$1
    local url=$2
    local file=$(basename "$url")

    echo "Downloading $name..."
    if [ -f "$DATASETS_DIR/$file" ]; then
        echo "  ✓ Already downloaded: $file"
    else
        wget -q --show-progress -O "$DATASETS_DIR/$file" "$url"
        echo "  ✓ Downloaded: $file"
    fi

    echo "Extracting $name..."
    if [[ $file == *.tar.gz ]]; then
        tar -xzf "$DATASETS_DIR/$file" -C "$DATASETS_DIR"
    elif [[ $file == *.gz ]]; then
        gunzip -k "$DATASETS_DIR/$file"
    fi
    echo "  ✓ Extracted successfully"
    echo ""
}

# SIFT1M Dataset (128D, 1M vectors)
# http://corpus-texmex.irisa.fr/
echo "1. SIFT1M Dataset (128 dimensions, 1M vectors)"
echo "   Download from: http://corpus-texmex.irisa.fr/"
echo "   Note: Direct download requires manual intervention due to terms of service"
echo "   Please visit the website and download sift.tar.gz manually to datasets/"
echo ""

# GIST1M Dataset (960D, 1M vectors)
echo "2. GIST1M Dataset (960 dimensions, 1M vectors)"
echo "   Download from: http://corpus-texmex.irisa.fr/"
echo "   Note: Direct download requires manual intervention due to terms of service"
echo "   Please visit the website and download gist.tar.gz manually to datasets/"
echo ""

# Deep1M Dataset (96D, 1M vectors)
echo "3. Deep1M Dataset (96 dimensions, 1M vectors)"
echo "   Download from: http://sites.skoltech.ru/compvision/noimi/"
echo "   Note: This dataset may require registration"
echo ""

# Alternative: Generate synthetic datasets
echo "═══════════════════════════════════════════════════════════════"
echo "ALTERNATIVE: Generate Synthetic Datasets"
echo "═══════════════════════════════════════════════════════════════"
echo ""
echo "If you prefer to use synthetic data for benchmarking, the"
echo "benchmark tools will automatically generate appropriate datasets."
echo ""
echo "To run with synthetic data:"
echo "  cargo run --release --bin ann-benchmark -- --dataset synthetic"
echo ""

# Check for HDF5 support
echo "Checking dependencies..."
if command -v h5dump &> /dev/null; then
    echo "  ✓ HDF5 tools installed"
else
    echo "  ⚠ HDF5 tools not found. Install with:"
    echo "    Ubuntu/Debian: sudo apt-get install hdf5-tools"
    echo "    macOS: brew install hdf5"
    echo "    Note: HDF5 is optional for synthetic benchmarks"
fi
echo ""

echo "════════════════════════════════════════"
echo "Setup Instructions:"
echo "════════════════════════════════════════"
echo ""
echo "1. Manual Download (for real datasets):"
echo "   - Visit http://corpus-texmex.irisa.fr/"
echo "   - Download sift.tar.gz, gist.tar.gz"
echo "   - Place in: $DATASETS_DIR/"
echo "   - Extract: tar -xzf $DATASETS_DIR/sift.tar.gz -C $DATASETS_DIR/"
echo ""
echo "2. Synthetic Datasets (recommended for testing):"
echo "   - No download required"
echo "   - Generated automatically by benchmark tools"
echo "   - Suitable for performance testing and profiling"
echo ""
echo "3. Run Benchmarks:"
echo "   cd crates/ruvector-bench"
echo "   cargo run --release --bin ann-benchmark"
echo ""
echo "✓ Setup guide complete!"
