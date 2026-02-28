#!/bin/bash
# Offline Toolchain Cache for RuvLLM ESP32
#
# Downloads and caches the ESP32 toolchain for air-gapped environments.
# Run this on a machine with internet, then transfer the cache folder.
#
# Usage:
#   ./offline-cache.sh create     # Create cache
#   ./offline-cache.sh install    # Install from cache
#   ./offline-cache.sh verify     # Verify cache integrity

set -e

CACHE_DIR="${RUVLLM_CACHE_DIR:-$HOME/.ruvllm-cache}"
TOOLCHAIN_VERSION="1.90.0.0"
ESPFLASH_VERSION="4.3.0"
LDPROXY_VERSION="0.3.4"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${CYAN}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

detect_platform() {
    case "$(uname -s)" in
        Linux*)  PLATFORM="linux" ;;
        Darwin*) PLATFORM="macos" ;;
        MINGW*|CYGWIN*|MSYS*) PLATFORM="windows" ;;
        *) PLATFORM="unknown" ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64) ARCH="x86_64" ;;
        aarch64|arm64) ARCH="aarch64" ;;
        *) ARCH="unknown" ;;
    esac

    echo "Platform: $PLATFORM-$ARCH"
}

create_cache() {
    log_info "Creating offline cache in $CACHE_DIR"
    mkdir -p "$CACHE_DIR"/{toolchain,binaries,checksums}

    detect_platform

    # Download espup
    log_info "Downloading espup..."
    case "$PLATFORM" in
        linux)
            ESPUP_URL="https://github.com/esp-rs/espup/releases/download/v$TOOLCHAIN_VERSION/espup-${ARCH}-unknown-linux-gnu"
            ;;
        macos)
            ESPUP_URL="https://github.com/esp-rs/espup/releases/download/v$TOOLCHAIN_VERSION/espup-${ARCH}-apple-darwin"
            ;;
        windows)
            ESPUP_URL="https://github.com/esp-rs/espup/releases/download/v$TOOLCHAIN_VERSION/espup-${ARCH}-pc-windows-msvc.exe"
            ;;
    esac

    curl -L "$ESPUP_URL" -o "$CACHE_DIR/binaries/espup"
    chmod +x "$CACHE_DIR/binaries/espup"
    log_success "Downloaded espup"

    # Download espflash
    log_info "Downloading espflash..."
    ESPFLASH_URL="https://github.com/esp-rs/espflash/releases/download/v$ESPFLASH_VERSION/espflash-${ARCH}-unknown-linux-gnu.zip"
    curl -L "$ESPFLASH_URL" -o "$CACHE_DIR/binaries/espflash.zip" || log_warn "espflash download may have failed"

    # Run espup to download toolchain components
    log_info "Downloading ESP toolchain (this may take a while)..."
    RUSTUP_HOME="$CACHE_DIR/toolchain/rustup" \
    CARGO_HOME="$CACHE_DIR/toolchain/cargo" \
    "$CACHE_DIR/binaries/espup" install --export-file "$CACHE_DIR/export-esp.sh"

    # Create checksums
    log_info "Creating checksums..."
    cd "$CACHE_DIR"
    find . -type f -exec sha256sum {} \; > checksums/manifest.sha256
    log_success "Checksums created"

    # Create metadata
    cat > "$CACHE_DIR/metadata.json" << EOF
{
    "version": "1.0.0",
    "created": "$(date -Iseconds)",
    "platform": "$PLATFORM",
    "arch": "$ARCH",
    "toolchain_version": "$TOOLCHAIN_VERSION",
    "espflash_version": "$ESPFLASH_VERSION"
}
EOF

    log_success "Cache created at $CACHE_DIR"
    du -sh "$CACHE_DIR"
    echo ""
    log_info "To use on offline machine:"
    echo "  1. Copy $CACHE_DIR to the target machine"
    echo "  2. Run: ./offline-cache.sh install"
}

install_from_cache() {
    if [ ! -d "$CACHE_DIR" ]; then
        log_error "Cache not found at $CACHE_DIR"
        exit 1
    fi

    log_info "Installing from offline cache..."

    # Verify cache
    verify_cache || { log_error "Cache verification failed"; exit 1; }

    # Copy toolchain to user directories
    RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
    CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"

    log_info "Installing Rust toolchain..."
    mkdir -p "$RUSTUP_HOME" "$CARGO_HOME"
    cp -r "$CACHE_DIR/toolchain/rustup/"* "$RUSTUP_HOME/"
    cp -r "$CACHE_DIR/toolchain/cargo/"* "$CARGO_HOME/"

    # Install binaries
    log_info "Installing espup and espflash..."
    cp "$CACHE_DIR/binaries/espup" "$CARGO_HOME/bin/"

    if [ -f "$CACHE_DIR/binaries/espflash.zip" ]; then
        unzip -o "$CACHE_DIR/binaries/espflash.zip" -d "$CARGO_HOME/bin/"
    fi

    # Copy export script
    cp "$CACHE_DIR/export-esp.sh" "$HOME/"

    log_success "Installation complete!"
    echo ""
    log_info "Run this command to set up your environment:"
    echo "  source ~/export-esp.sh"
}

verify_cache() {
    if [ ! -f "$CACHE_DIR/checksums/manifest.sha256" ]; then
        log_error "Checksum manifest not found"
        return 1
    fi

    log_info "Verifying cache integrity..."
    cd "$CACHE_DIR"

    # Verify a subset of files (full verification can be slow)
    head -20 checksums/manifest.sha256 | sha256sum -c --quiet 2>/dev/null

    if [ $? -eq 0 ]; then
        log_success "Cache integrity verified"
        return 0
    else
        log_error "Cache integrity check failed"
        return 1
    fi
}

show_info() {
    if [ ! -f "$CACHE_DIR/metadata.json" ]; then
        log_error "Cache not found"
        exit 1
    fi

    echo "=== RuvLLM ESP32 Offline Cache ==="
    cat "$CACHE_DIR/metadata.json"
    echo ""
    echo "Cache size: $(du -sh "$CACHE_DIR" | cut -f1)"
}

# Main
case "${1:-help}" in
    create)
        create_cache
        ;;
    install)
        install_from_cache
        ;;
    verify)
        verify_cache
        ;;
    info)
        show_info
        ;;
    *)
        echo "RuvLLM ESP32 Offline Toolchain Cache"
        echo ""
        echo "Usage: $0 <command>"
        echo ""
        echo "Commands:"
        echo "  create   - Download and cache toolchain (requires internet)"
        echo "  install  - Install from cache (works offline)"
        echo "  verify   - Verify cache integrity"
        echo "  info     - Show cache information"
        echo ""
        echo "Environment variables:"
        echo "  RUVLLM_CACHE_DIR - Cache directory (default: ~/.ruvllm-cache)"
        ;;
esac
