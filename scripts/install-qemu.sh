#!/bin/bash
# install-qemu.sh - Install QEMU with ESP32-S3 support (Espressif fork)
# Usage: bash scripts/install-qemu.sh [OPTIONS]
set -euo pipefail

# ── Colors ────────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
err()   { echo -e "${RED}[ERROR]${NC} $*"; }
step()  { echo -e "\n${CYAN}${BOLD}▶ $*${NC}"; }

# ── Defaults ──────────────────────────────────────────────────────────────────
INSTALL_DIR="$HOME/.espressif/qemu"
BRANCH="esp-develop"
JOBS=""
SKIP_DEPS=false
UNINSTALL=false
CHECK_ONLY=false
QEMU_REPO="https://github.com/espressif/qemu.git"

# ── Usage ─────────────────────────────────────────────────────────────────────
usage() {
    cat <<EOF
${BOLD}install-qemu.sh${NC} - Install QEMU with ESP32-S3 support (Espressif fork)

${BOLD}USAGE${NC}
    bash scripts/install-qemu.sh [OPTIONS]

${BOLD}OPTIONS${NC}
    --install-dir DIR   Installation directory (default: ~/.espressif/qemu)
    --branch TAG        QEMU branch or tag to build (default: esp-develop)
    --jobs N            Parallel build jobs (default: nproc)
    --skip-deps         Skip system dependency installation
    --uninstall         Remove QEMU installation
    --check             Verify existing installation and exit
    -h, --help          Show this help

${BOLD}EXIT CODES${NC}
    0  Success
    1  Dependency installation failed
    2  Build failed
    3  Unsupported OS

${BOLD}EXAMPLES${NC}
    bash scripts/install-qemu.sh
    bash scripts/install-qemu.sh --install-dir /opt/qemu-esp --jobs 8
    bash scripts/install-qemu.sh --check
    bash scripts/install-qemu.sh --uninstall
EOF
}

# ── Parse args ────────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --install-dir)  INSTALL_DIR="$2"; shift 2 ;;
        --branch)       BRANCH="$2"; shift 2 ;;
        --jobs)         JOBS="$2"; shift 2 ;;
        --skip-deps)    SKIP_DEPS=true; shift ;;
        --uninstall)    UNINSTALL=true; shift ;;
        --check)        CHECK_ONLY=true; shift ;;
        -h|--help)      usage; exit 0 ;;
        *)              err "Unknown option: $1"; usage; exit 1 ;;
    esac
done

# ── OS detection ──────────────────────────────────────────────────────────────
detect_os() {
    OS="unknown"
    DISTRO="unknown"
    IS_WSL=false

    case "$(uname -s)" in
        Linux)
            OS="linux"
            if grep -qi microsoft /proc/version 2>/dev/null; then
                IS_WSL=true
            fi
            if [ -f /etc/os-release ]; then
                # shellcheck disable=SC1091
                . /etc/os-release
                case "$ID" in
                    ubuntu|debian|pop|linuxmint|elementary) DISTRO="debian" ;;
                    fedora|rhel|centos|rocky|alma)          DISTRO="fedora" ;;
                    arch|manjaro|endeavouros)               DISTRO="arch" ;;
                    opensuse*|sles)                         DISTRO="suse" ;;
                    *)                                      DISTRO="$ID" ;;
                esac
            fi
            ;;
        Darwin) OS="macos"; DISTRO="macos" ;;
        MINGW*|MSYS*)
            err "Native Windows/MINGW detected."
            err "QEMU ESP32-S3 must be built on Linux or macOS."
            err "Options:"
            err "  1. Use WSL:  wsl bash scripts/install-qemu.sh"
            err "  2. Use Docker: docker run -it ubuntu:22.04 bash"
            err "  3. Download pre-built: https://github.com/espressif/qemu/releases"
            exit 3
            ;;
        *)      err "Unsupported OS: $(uname -s)"; exit 3 ;;
    esac

    info "Detected: OS=${OS} Distro=${DISTRO} WSL=${IS_WSL}"
}

# ── Check existing installation ───────────────────────────────────────────────
check_installation() {
    local qemu_bin="$INSTALL_DIR/build/qemu-system-xtensa"
    if [ -x "$qemu_bin" ]; then
        local version
        version=$("$qemu_bin" --version 2>/dev/null | head -1) || true
        if [ -n "$version" ]; then
            ok "QEMU installed: $version"
            ok "Binary: $qemu_bin"
            return 0
        fi
    fi
    # Check PATH
    if command -v qemu-system-xtensa &>/dev/null; then
        local version
        version=$(qemu-system-xtensa --version 2>/dev/null | head -1) || true
        ok "QEMU found in PATH: $version"
        return 0
    fi
    warn "QEMU with ESP32-S3 support not found"
    return 1
}

if $CHECK_ONLY; then
    detect_os
    if check_installation; then exit 0; else exit 1; fi
fi

# ── Uninstall ─────────────────────────────────────────────────────────────────
if $UNINSTALL; then
    step "Uninstalling QEMU from $INSTALL_DIR"
    if [ -d "$INSTALL_DIR" ]; then
        rm -rf "$INSTALL_DIR"
        ok "Removed $INSTALL_DIR"
    else
        warn "Directory not found: $INSTALL_DIR"
    fi
    # Remove symlink
    local_bin="$HOME/.local/bin/qemu-system-xtensa"
    if [ -L "$local_bin" ]; then
        rm -f "$local_bin"
        ok "Removed symlink $local_bin"
    fi
    ok "Uninstall complete"
    exit 0
fi

# ── Main install flow ─────────────────────────────────────────────────────────
detect_os

# Default jobs = nproc
if [ -z "$JOBS" ]; then
    if command -v nproc &>/dev/null; then
        JOBS=$(nproc)
    elif command -v sysctl &>/dev/null; then
        JOBS=$(sysctl -n hw.ncpu 2>/dev/null || echo 4)
    else
        JOBS=4
    fi
fi
info "Build parallelism: $JOBS jobs"

# ── Step 1: Install dependencies ──────────────────────────────────────────────
install_deps() {
    step "Installing build dependencies"

    case "$DISTRO" in
        debian)
            info "Using apt (Debian/Ubuntu)"
            sudo apt-get update -qq
            sudo apt-get install -y -qq \
                git build-essential python3 python3-pip python3-venv \
                ninja-build pkg-config libglib2.0-dev libpixman-1-dev \
                libslirp-dev libgcrypt-dev
            ;;
        fedora)
            info "Using dnf (Fedora/RHEL)"
            sudo dnf install -y \
                git gcc gcc-c++ make python3 python3-pip \
                ninja-build pkgconfig glib2-devel pixman-devel \
                libslirp-devel libgcrypt-devel
            ;;
        arch)
            info "Using pacman (Arch)"
            sudo pacman -S --needed --noconfirm \
                git base-devel python python-pip \
                ninja pkgconf glib2 pixman libslirp libgcrypt
            ;;
        suse)
            info "Using zypper (openSUSE)"
            sudo zypper install -y \
                git gcc gcc-c++ make python3 python3-pip \
                ninja pkg-config glib2-devel libpixman-1-0-devel \
                libslirp-devel libgcrypt-devel
            ;;
        macos)
            info "Using Homebrew"
            if ! command -v brew &>/dev/null; then
                err "Homebrew not found. Install from https://brew.sh"
                exit 1
            fi
            brew install glib pixman ninja pkg-config libslirp libgcrypt || true
            ;;
        *)
            warn "Unknown distro '$DISTRO' - install these manually:"
            warn "  git, gcc/g++, python3, ninja, pkg-config, glib2-dev, pixman-dev, libslirp-dev"
            return 1
            ;;
    esac
    ok "Dependencies installed"
}

if ! $SKIP_DEPS; then
    install_deps || { err "Dependency installation failed"; exit 1; }
else
    info "Skipping dependency installation (--skip-deps)"
fi

# ── Step 2: Clone Espressif QEMU fork ─────────────────────────────────────────
step "Cloning Espressif QEMU fork"

SRC_DIR="$INSTALL_DIR"
if [ -d "$SRC_DIR/.git" ]; then
    info "Repository already exists at $SRC_DIR"
    info "Fetching latest changes on branch $BRANCH"
    git -C "$SRC_DIR" fetch origin "$BRANCH" --depth=1
    git -C "$SRC_DIR" checkout "$BRANCH" 2>/dev/null || git -C "$SRC_DIR" checkout "origin/$BRANCH"
    ok "Updated to latest $BRANCH"
else
    info "Cloning $QEMU_REPO (branch: $BRANCH)"
    mkdir -p "$(dirname "$SRC_DIR")"
    git clone --depth=1 --branch "$BRANCH" "$QEMU_REPO" "$SRC_DIR"
    ok "Cloned to $SRC_DIR"
fi

# ── Step 3: Configure and build ───────────────────────────────────────────────
step "Configuring QEMU (target: xtensa-softmmu)"

BUILD_DIR="$SRC_DIR/build"
mkdir -p "$BUILD_DIR"
cd "$SRC_DIR"

./configure \
    --target-list=xtensa-softmmu \
    --enable-slirp \
    --enable-gcrypt \
    --prefix="$INSTALL_DIR/dist" \
    2>&1 | tail -5

step "Building QEMU ($JOBS parallel jobs)"
make -j"$JOBS" -C "$BUILD_DIR" 2>&1 | tail -20

if [ ! -x "$BUILD_DIR/qemu-system-xtensa" ]; then
    err "Build failed - qemu-system-xtensa binary not found"
    err "Troubleshooting:"
    err "  1. Check build output above for errors"
    err "  2. Ensure all dependencies are installed: re-run without --skip-deps"
    err "  3. Try with fewer jobs: --jobs 1"
    err "  4. On macOS, ensure Xcode CLT: xcode-select --install"
    exit 2
fi
ok "Build succeeded: $BUILD_DIR/qemu-system-xtensa"

# ── Step 4: Create symlink / add to PATH ──────────────────────────────────────
step "Setting up PATH access"

LOCAL_BIN="$HOME/.local/bin"
mkdir -p "$LOCAL_BIN"
ln -sf "$BUILD_DIR/qemu-system-xtensa" "$LOCAL_BIN/qemu-system-xtensa"
ok "Symlinked to $LOCAL_BIN/qemu-system-xtensa"

# Check if ~/.local/bin is in PATH
if ! echo "$PATH" | tr ':' '\n' | grep -qx "$LOCAL_BIN"; then
    warn "$LOCAL_BIN is not in your PATH"
    warn "Add this to your shell profile (~/.bashrc or ~/.zshrc):"
    echo -e "  ${BOLD}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}"
fi

# ── Step 5: Verify ────────────────────────────────────────────────────────────
step "Verifying installation"

QEMU_VERSION=$("$BUILD_DIR/qemu-system-xtensa" --version | head -1)
ok "$QEMU_VERSION"

# Check ESP32-S3 machine support
if "$BUILD_DIR/qemu-system-xtensa" -machine help 2>/dev/null | grep -q esp32s3; then
    ok "ESP32-S3 machine type available"
else
    warn "ESP32-S3 machine type not listed (may still work with newer builds)"
fi

# ── Step 6: Install Python packages ──────────────────────────────────────────
step "Installing Python packages (esptool, pyyaml, nvs-partition-gen)"

PIP_CMD="pip3"
if ! command -v pip3 &>/dev/null; then
    PIP_CMD="python3 -m pip"
fi

$PIP_CMD install --user --quiet \
    esptool \
    pyyaml \
    esp-idf-nvs-partition-gen \
    2>&1 || warn "Some Python packages failed to install (non-fatal)"

ok "Python packages installed"

# ── Done ──────────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}Installation complete!${NC}"
echo ""
echo -e "${BOLD}Next steps:${NC}"
echo ""
echo "  1. Run a smoke test:"
echo -e "     ${CYAN}qemu-system-xtensa -nographic -machine esp32s3 \\${NC}"
echo -e "     ${CYAN}  -drive file=firmware.bin,if=mtd,format=raw \\${NC}"
echo -e "     ${CYAN}  -serial mon:stdio${NC}"
echo ""
echo "  2. Run the project QEMU tests:"
echo -e "     ${CYAN}cd $(dirname "$0")/.."
echo -e "     pytest firmware/esp32-csi-node/tests/qemu/ -v${NC}"
echo ""
echo "  3. Binary location:"
echo -e "     ${CYAN}$BUILD_DIR/qemu-system-xtensa${NC}"
echo ""
echo -e "  4. Uninstall:"
echo -e "     ${CYAN}bash scripts/install-qemu.sh --uninstall${NC}"
echo ""
