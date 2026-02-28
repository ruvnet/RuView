#!/usr/bin/env bash
# ======================================================================
#  WiFi-DensePose Installer
#
#  Step-by-step installer with hardware detection, environment checks,
#  and environment-specific RVF builds.
#
#  Usage:
#    ./install.sh                     Interactive guided install
#    ./install.sh --profile browser   Non-interactive with profile
#    ./install.sh --check-only        Hardware/environment check only
#    ./install.sh --help              Show help
#
#  Profiles:
#    verify   - Verification only (Python + numpy + scipy)
#    python   - Full Python pipeline (API server, sensing, analytics)
#    rust     - Rust pipeline (signal processing, API, CLI)
#    browser  - WASM build for browser deployment
#    iot      - ESP32 sensor mesh + aggregator
#    docker   - Docker-based deployment
#    field    - Disaster response (WiFi-Mat) field deployment
#    full     - Everything
# ======================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_DIR="${SCRIPT_DIR}/rust-port/wifi-densepose-rs"

# ─── Colors ───────────────────────────────────────────────────────────
if [ -t 1 ]; then
    RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
    CYAN='\033[0;36m'; BLUE='\033[0;34m'; MAGENTA='\033[0;35m'
    BOLD='\033[1m'; DIM='\033[2m'; RESET='\033[0m'
else
    RED=''; GREEN=''; YELLOW=''; CYAN=''; BLUE=''; MAGENTA=''
    BOLD=''; DIM=''; RESET=''
fi

# ─── Globals ──────────────────────────────────────────────────────────
PROFILE=""
CHECK_ONLY=false
VERBOSE=false
SKIP_CONFIRM=false
INSTALL_LOG="${SCRIPT_DIR}/.install.log"

# Hardware detection results
HAS_PYTHON=false; PYTHON_CMD=""
HAS_RUST=false; RUST_VERSION=""
HAS_CARGO=false
HAS_WASM_PACK=false
HAS_WASM_TARGET=false
HAS_DOCKER=false; DOCKER_VERSION=""
HAS_NODE=false; NODE_VERSION=""
HAS_NPM=false
HAS_ESPIDF=false
HAS_GIT=false
HAS_GPU=false; GPU_INFO=""
HAS_WIFI=false; WIFI_IFACE=""
HAS_OPENBLAS=false
HAS_PKGCONFIG=false
HAS_GCC=false
TOTAL_RAM_MB=0
DISK_FREE_MB=0
OS_TYPE=""; OS_RELEASE=""
ARCH=""

# ─── Helpers ──────────────────────────────────────────────────────────
log() { echo -e "$1" | tee -a "${INSTALL_LOG}"; }
step() { echo -e "\n${CYAN}[$1]${RESET} ${BOLD}$2${RESET}"; }
ok() { echo -e "  ${GREEN}OK${RESET}    $1"; }
warn() { echo -e "  ${YELLOW}WARN${RESET}  $1"; }
fail() { echo -e "  ${RED}FAIL${RESET}  $1"; }
info() { echo -e "  ${DIM}$1${RESET}"; }
need() { echo -e "  ${BLUE}NEED${RESET}  $1"; }

banner() {
    echo ""
    echo -e "${BOLD}======================================================================"
    echo "  WiFi-DensePose Installer"
    echo "  Hardware detection + environment-specific RVF builds"
    echo -e "======================================================================${RESET}"
    echo ""
}

usage() {
    echo "Usage: ./install.sh [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --profile PROFILE   Install specific profile (see below)"
    echo "  --check-only        Run hardware/environment checks only"
    echo "  --verbose           Show detailed output"
    echo "  --yes               Skip confirmation prompts"
    echo "  --help              Show this help"
    echo ""
    echo "Profiles:"
    echo "  verify   Verification only (Python + numpy + scipy)"
    echo "  python   Full Python pipeline (API, sensing, analytics)"
    echo "  rust     Rust pipeline (signal processing, benchmarks)"
    echo "  browser  WASM build for browser deployment (~10MB)"
    echo "  iot      ESP32 sensor mesh + aggregator"
    echo "  docker   Docker-based deployment"
    echo "  field    WiFi-Mat disaster response field kit (~62MB)"
    echo "  full     Everything"
    echo ""
    echo "Examples:"
    echo "  ./install.sh                       # Interactive"
    echo "  ./install.sh --profile verify      # Quick verification"
    echo "  ./install.sh --profile rust --yes  # Rust build, no prompts"
    echo "  ./install.sh --check-only          # Just detect hardware"
}

# ─── Argument parsing ─────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --profile)   PROFILE="$2"; shift 2 ;;
        --check-only) CHECK_ONLY=true; shift ;;
        --verbose)   VERBOSE=true; shift ;;
        --yes)       SKIP_CONFIRM=true; shift ;;
        --help|-h)   usage; exit 0 ;;
        *)           echo "Unknown option: $1"; usage; exit 1 ;;
    esac
done

# ─── Initialize log ──────────────────────────────────────────────────
echo "WiFi-DensePose install log - $(date -u +%Y-%m-%dT%H:%M:%SZ)" > "${INSTALL_LOG}"

# ======================================================================
#  STEP 1: SYSTEM DETECTION
# ======================================================================

detect_system() {
    step "1/7" "System Detection"
    echo ""

    # OS
    if [[ "$(uname)" == "Darwin" ]]; then
        OS_TYPE="macos"
        OS_RELEASE="$(sw_vers -productVersion 2>/dev/null || echo 'unknown')"
        ok "macOS ${OS_RELEASE}"
    elif [[ "$(uname)" == "Linux" ]]; then
        OS_TYPE="linux"
        if [ -f /etc/os-release ]; then
            OS_RELEASE="$(. /etc/os-release && echo "${PRETTY_NAME}")"
        else
            OS_RELEASE="$(uname -r)"
        fi
        ok "Linux: ${OS_RELEASE}"
    else
        OS_TYPE="other"
        OS_RELEASE="$(uname -s)"
        warn "Unsupported OS: ${OS_RELEASE}"
    fi

    # Architecture
    ARCH="$(uname -m)"
    ok "Architecture: ${ARCH}"

    # RAM
    if [[ "$OS_TYPE" == "linux" ]]; then
        TOTAL_RAM_MB=$(awk '/MemTotal/ {print int($2/1024)}' /proc/meminfo 2>/dev/null || echo 0)
    elif [[ "$OS_TYPE" == "macos" ]]; then
        TOTAL_RAM_MB=$(( $(sysctl -n hw.memsize 2>/dev/null || echo 0) / 1024 / 1024 ))
    fi
    if [ "$TOTAL_RAM_MB" -ge 8192 ]; then
        ok "RAM: ${TOTAL_RAM_MB} MB (recommended: 8192+)"
    elif [ "$TOTAL_RAM_MB" -ge 4096 ]; then
        warn "RAM: ${TOTAL_RAM_MB} MB (minimum met, 8192+ recommended)"
    elif [ "$TOTAL_RAM_MB" -gt 0 ]; then
        warn "RAM: ${TOTAL_RAM_MB} MB (below 4096 minimum)"
    fi

    # Disk
    DISK_FREE_MB=$(df -m "${SCRIPT_DIR}" 2>/dev/null | awk 'NR==2 {print $4}' || echo 0)
    if [ "$DISK_FREE_MB" -ge 5000 ]; then
        ok "Disk: ${DISK_FREE_MB} MB free"
    elif [ "$DISK_FREE_MB" -ge 2000 ]; then
        warn "Disk: ${DISK_FREE_MB} MB free (5000+ recommended)"
    else
        warn "Disk: ${DISK_FREE_MB} MB free (2000+ required)"
    fi

    # GPU
    if command -v nvidia-smi &>/dev/null; then
        GPU_INFO="$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -1 || echo '')"
        if [ -n "$GPU_INFO" ]; then
            HAS_GPU=true
            ok "GPU: ${GPU_INFO} (NVIDIA CUDA)"
        fi
    elif command -v metal &>/dev/null || [ -d "/System/Library/Frameworks/Metal.framework" ]; then
        HAS_GPU=true
        GPU_INFO="Apple Metal"
        ok "GPU: Apple Metal"
    fi
    if ! $HAS_GPU; then
        info "GPU: None detected (CPU inference will be used)"
    fi
}

# ======================================================================
#  STEP 2: TOOLCHAIN DETECTION
# ======================================================================

detect_toolchains() {
    step "2/7" "Toolchain Detection"
    echo ""

    # Python
    if command -v python3 &>/dev/null; then
        PYTHON_CMD=python3
        HAS_PYTHON=true
        PY_VER="$($PYTHON_CMD --version 2>&1)"
        ok "Python: ${PY_VER}"
    elif command -v python &>/dev/null; then
        PY_VER="$(python --version 2>&1)"
        if [[ "$PY_VER" == *"3."* ]]; then
            PYTHON_CMD=python
            HAS_PYTHON=true
            ok "Python: ${PY_VER}"
        else
            fail "Python 2 found but Python 3 required"
        fi
    else
        need "Python 3.8+ not found (install: https://python.org)"
    fi

    # Check Python packages
    if $HAS_PYTHON; then
        NUMPY_OK=false; SCIPY_OK=false; TORCH_OK=false; FASTAPI_OK=false
        $PYTHON_CMD -c "import numpy" 2>/dev/null && NUMPY_OK=true
        $PYTHON_CMD -c "import scipy" 2>/dev/null && SCIPY_OK=true
        $PYTHON_CMD -c "import torch" 2>/dev/null && TORCH_OK=true
        $PYTHON_CMD -c "import fastapi" 2>/dev/null && FASTAPI_OK=true

        if $NUMPY_OK && $SCIPY_OK; then
            ok "Python packages: numpy, scipy (verification ready)"
        else
            need "Python packages: numpy/scipy missing (pip install numpy scipy)"
        fi
        if $TORCH_OK; then ok "PyTorch: installed"; else info "PyTorch: not installed (needed for full pipeline)"; fi
        if $FASTAPI_OK; then ok "FastAPI: installed"; else info "FastAPI: not installed (needed for API server)"; fi
    fi

    # Rust
    if command -v rustc &>/dev/null; then
        HAS_RUST=true
        RUST_VERSION="$(rustc --version 2>&1)"
        ok "Rust: ${RUST_VERSION}"
    else
        info "Rust: not installed (install: https://rustup.rs)"
    fi

    # Cargo
    if command -v cargo &>/dev/null; then
        HAS_CARGO=true
        ok "Cargo: $(cargo --version 2>&1)"
    fi

    # wasm-pack
    if command -v wasm-pack &>/dev/null; then
        HAS_WASM_PACK=true
        ok "wasm-pack: $(wasm-pack --version 2>&1)"
    else
        info "wasm-pack: not installed (needed for browser profile)"
    fi

    # WASM target
    if $HAS_RUST && rustup target list --installed 2>/dev/null | grep -q "wasm32-unknown-unknown"; then
        HAS_WASM_TARGET=true
        ok "WASM target: wasm32-unknown-unknown installed"
    fi

    # Docker
    if command -v docker &>/dev/null; then
        HAS_DOCKER=true
        DOCKER_VERSION="$(docker --version 2>&1)"
        ok "Docker: ${DOCKER_VERSION}"
    else
        info "Docker: not installed (needed for docker profile)"
    fi

    # Node.js
    if command -v node &>/dev/null; then
        HAS_NODE=true
        NODE_VERSION="$(node --version 2>&1)"
        ok "Node.js: ${NODE_VERSION}"
    else
        info "Node.js: not installed (optional for UI dev)"
    fi

    if command -v npm &>/dev/null; then
        HAS_NPM=true
    fi

    # Git
    if command -v git &>/dev/null; then
        HAS_GIT=true
        ok "Git: $(git --version 2>&1)"
    else
        need "Git: not installed"
    fi

    # ESP-IDF
    if command -v idf.py &>/dev/null || [ -d "${HOME}/esp/esp-idf" ] || [ -n "${IDF_PATH:-}" ]; then
        HAS_ESPIDF=true
        ok "ESP-IDF: found"
    else
        info "ESP-IDF: not installed (needed for IoT profile)"
    fi

    # System libraries (for Rust builds)
    if command -v pkg-config &>/dev/null; then
        HAS_PKGCONFIG=true
    fi
    if command -v gcc &>/dev/null || command -v cc &>/dev/null; then
        HAS_GCC=true
    fi
    if pkg-config --exists openblas 2>/dev/null || [ -f /usr/lib/libopenblas.so ] || [ -f /usr/lib/x86_64-linux-gnu/libopenblas.so ] || brew list openblas &>/dev/null 2>&1; then
        HAS_OPENBLAS=true
        ok "OpenBLAS: found"
    elif $HAS_RUST; then
        need "OpenBLAS: not found (needed for Rust signal crate)"
    fi
}

# ======================================================================
#  STEP 3: WIFI HARDWARE DETECTION
# ======================================================================

detect_wifi_hardware() {
    step "3/7" "WiFi Hardware Detection"
    echo ""

    local hw_found=false

    # Check for WiFi interfaces
    if [[ "$OS_TYPE" == "linux" ]]; then
        # Check /proc/net/wireless for active WiFi
        if [ -f /proc/net/wireless ]; then
            local ifaces
            ifaces="$(awk 'NR>2 {print $1}' /proc/net/wireless 2>/dev/null | tr -d ':' || true)"
            if [ -n "$ifaces" ]; then
                for iface in $ifaces; do
                    HAS_WIFI=true
                    WIFI_IFACE="$iface"
                    ok "WiFi interface: ${iface} (Linux /proc/net/wireless)"
                    hw_found=true
                done
            fi
        fi

        # Check for iwconfig interfaces
        if ! $hw_found && command -v iwconfig &>/dev/null; then
            local iface
            iface="$(iwconfig 2>/dev/null | grep -o '^\S*' | head -1 || true)"
            if [ -n "$iface" ] && [ "$iface" != "lo" ]; then
                HAS_WIFI=true
                WIFI_IFACE="$iface"
                ok "WiFi interface: ${iface} (iwconfig)"
                hw_found=true
            fi
        fi

        # Check for ip link wireless interfaces
        if ! $hw_found && command -v ip &>/dev/null; then
            local wireless_ifaces
            wireless_ifaces="$(ip link show 2>/dev/null | grep -oP '^\d+: \K(wl\S+)' || true)"
            if [ -n "$wireless_ifaces" ]; then
                for iface in $wireless_ifaces; do
                    HAS_WIFI=true
                    WIFI_IFACE="$iface"
                    ok "WiFi interface: ${iface} (ip link)"
                    hw_found=true
                    break
                done
            fi
        fi

        # Check for ESP32 USB serial devices
        local usb_devs
        usb_devs="$(ls /dev/ttyUSB* /dev/ttyACM* 2>/dev/null || true)"
        if [ -n "$usb_devs" ]; then
            ok "USB serial devices detected (possible ESP32)"
            for dev in $usb_devs; do
                info "  ${dev}"
            done
            hw_found=true
        fi

        # Check for Intel 5300 CSI tool
        if [ -d /sys/kernel/debug/ieee80211 ]; then
            for phy in /sys/kernel/debug/ieee80211/*/; do
                if [ -d "${phy}iwlwifi" ]; then
                    ok "Intel WiFi debug interface: $(basename "$phy")"
                    hw_found=true
                fi
            done
        fi

    elif [[ "$OS_TYPE" == "macos" ]]; then
        # macOS WiFi
        local airport="/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport"
        if [ -x "$airport" ]; then
            local ssid
            ssid="$($airport -I 2>/dev/null | awk '/ SSID/ {print $2}' || true)"
            if [ -n "$ssid" ]; then
                HAS_WIFI=true
                WIFI_IFACE="en0"
                ok "WiFi: connected to '${ssid}' (en0)"
                hw_found=true
            fi
        fi
        if ! $hw_found; then
            local mac_wifi
            mac_wifi="$(networksetup -listallhardwareports 2>/dev/null | awk '/Wi-Fi/{getline; print $2}' || true)"
            if [ -n "$mac_wifi" ]; then
                HAS_WIFI=true
                WIFI_IFACE="$mac_wifi"
                ok "WiFi interface: ${mac_wifi}"
                hw_found=true
            fi
        fi
    fi

    if ! $hw_found; then
        info "No WiFi hardware detected"
        info "You can still run verification and build WASM/Docker targets"
    fi

    # CSI capability assessment
    echo ""
    echo -e "  ${BOLD}CSI Capability Assessment:${RESET}"
    if $HAS_WIFI; then
        echo -e "  ${GREEN}*${RESET} RSSI-based presence detection: ${GREEN}available${RESET} (commodity WiFi)"
        echo -e "  ${DIM}*${RESET} Full CSI extraction: requires ESP32-S3 mesh or Intel 5300/Atheros NIC"
        echo -e "  ${DIM}*${RESET} DensePose estimation: requires 3+ ESP32 nodes or research-grade NIC"
    else
        echo -e "  ${DIM}*${RESET} No WiFi = build/verify only (no live sensing)"
    fi
}

# ======================================================================
#  STEP 4: PROFILE RECOMMENDATION
# ======================================================================

recommend_profile() {
    step "4/7" "Profile Selection"
    echo ""

    # Auto-recommend based on detected hardware
    local recommended=""
    local available_profiles=()

    # verify is always available
    available_profiles+=("verify")

    if $HAS_PYTHON; then
        available_profiles+=("python")
    fi
    if $HAS_RUST && $HAS_CARGO; then
        available_profiles+=("rust")
        if $HAS_WASM_PACK || $HAS_WASM_TARGET; then
            available_profiles+=("browser")
        fi
    fi
    if $HAS_DOCKER; then
        available_profiles+=("docker")
    fi
    if $HAS_ESPIDF; then
        available_profiles+=("iot")
    fi
    if $HAS_RUST && $HAS_CARGO; then
        available_profiles+=("field")
    fi

    # Determine recommendation (Rust is the primary runtime)
    if $HAS_RUST && $HAS_CARGO; then
        recommended="rust"
    elif $HAS_PYTHON; then
        recommended="python"
    else
        recommended="verify"
    fi

    echo "  Available profiles based on your system:"
    echo ""

    local idx=1
    declare -A PROFILE_MAP

    for p in "${available_profiles[@]}"; do
        local marker=""
        if [ "$p" == "$recommended" ]; then
            marker=" ${GREEN}(recommended)${RESET}"
        fi
        case "$p" in
            verify)  echo -e "    ${BOLD}${idx})${RESET} verify  - Pipeline verification only (~5 MB)${marker}" ;;
            python)  echo -e "    ${BOLD}${idx})${RESET} python  - Full Python pipeline + API server (~500 MB)${marker}" ;;
            rust)    echo -e "    ${BOLD}${idx})${RESET} rust    - Rust pipeline with ~810x speedup (~200 MB)${marker}" ;;
            browser) echo -e "    ${BOLD}${idx})${RESET} browser - WASM for browser deployment (~10 MB output)${marker}" ;;
            docker)  echo -e "    ${BOLD}${idx})${RESET} docker  - Docker-based deployment (~1 GB image)${marker}" ;;
            iot)     echo -e "    ${BOLD}${idx})${RESET} iot     - ESP32 sensor mesh + aggregator${marker}" ;;
            field)   echo -e "    ${BOLD}${idx})${RESET} field   - WiFi-Mat disaster response kit (~62 MB)${marker}" ;;
        esac
        PROFILE_MAP[$idx]="$p"
        idx=$((idx + 1))
    done

    # Always show full as the last option
    echo -e "    ${BOLD}${idx})${RESET} full    - Install everything available"
    PROFILE_MAP[$idx]="full"

    if [ -n "$PROFILE" ]; then
        echo ""
        echo -e "  Profile specified via --profile: ${BOLD}${PROFILE}${RESET}"
        return
    fi

    if $CHECK_ONLY; then
        return
    fi

    echo ""
    read -rp "  Select profile [1-${idx}] (default: ${recommended}): " choice

    if [ -z "$choice" ]; then
        PROFILE="$recommended"
    elif [[ -n "${PROFILE_MAP[$choice]+x}" ]]; then
        PROFILE="${PROFILE_MAP[$choice]}"
    else
        echo -e "  ${RED}Invalid choice. Using ${recommended}.${RESET}"
        PROFILE="$recommended"
    fi

    echo ""
    echo -e "  Selected: ${BOLD}${PROFILE}${RESET}"
}

# ======================================================================
#  STEP 5: INSTALL DEPENDENCIES
# ======================================================================

install_deps() {
    step "5/7" "Installing Dependencies"
    echo ""

    case "$PROFILE" in
        verify)
            install_verify_deps
            ;;
        python)
            install_verify_deps
            install_python_deps
            ;;
        rust)
            install_rust_deps
            ;;
        browser)
            install_rust_deps
            install_wasm_deps
            ;;
        iot)
            install_rust_deps
            install_iot_deps
            ;;
        docker)
            check_docker_deps
            ;;
        field)
            install_rust_deps
            install_field_deps
            ;;
        full)
            install_verify_deps
            install_python_deps
            install_rust_deps
            if $HAS_WASM_PACK || $HAS_WASM_TARGET; then
                install_wasm_deps
            fi
            if $HAS_DOCKER; then
                check_docker_deps
            fi
            ;;
    esac
}

install_verify_deps() {
    echo -e "  ${CYAN}Verification dependencies:${RESET}"
    if ! $HAS_PYTHON; then
        fail "Python 3 required but not found. Install from https://python.org"
        exit 1
    fi

    local NEED_INSTALL=false
    $PYTHON_CMD -c "import numpy" 2>/dev/null || NEED_INSTALL=true
    $PYTHON_CMD -c "import scipy" 2>/dev/null || NEED_INSTALL=true

    if $NEED_INSTALL; then
        echo "  Installing numpy and scipy..."
        if [ -f "${SCRIPT_DIR}/v1/requirements-lock.txt" ]; then
            $PYTHON_CMD -m pip install -r "${SCRIPT_DIR}/v1/requirements-lock.txt" 2>&1 | tail -3
        else
            $PYTHON_CMD -m pip install numpy scipy 2>&1 | tail -3
        fi
        ok "numpy + scipy installed"
    else
        ok "numpy + scipy already installed"
    fi
}

install_python_deps() {
    echo -e "  ${CYAN}Python pipeline dependencies:${RESET}"
    if [ -f "${SCRIPT_DIR}/requirements.txt" ]; then
        echo "  Installing from requirements.txt..."
        $PYTHON_CMD -m pip install -r "${SCRIPT_DIR}/requirements.txt" 2>&1 | tail -5
        ok "Python dependencies installed"
    else
        warn "requirements.txt not found"
    fi
}

install_rust_deps() {
    echo -e "  ${CYAN}Rust dependencies:${RESET}"

    if ! $HAS_RUST; then
        echo "  Rust not found. Installing via rustup..."
        if ! $SKIP_CONFIRM; then
            read -rp "  Install Rust via rustup? [Y/n]: " yn
            if [[ "$yn" =~ ^[Nn] ]]; then
                fail "Rust required for this profile. Skipping."
                return 1
            fi
        fi
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        # shellcheck source=/dev/null
        source "${HOME}/.cargo/env" 2>/dev/null || true
        HAS_RUST=true
        HAS_CARGO=true
        ok "Rust installed"
    else
        ok "Rust already installed"
    fi

    # System libraries for OpenBLAS
    if ! $HAS_OPENBLAS; then
        echo ""
        echo -e "  ${CYAN}System libraries:${RESET}"
        if [[ "$OS_TYPE" == "linux" ]]; then
            if command -v apt-get &>/dev/null; then
                echo "  Installing build-essential, gfortran, libopenblas-dev, pkg-config..."
                if ! $SKIP_CONFIRM; then
                    read -rp "  Install system packages via apt? [Y/n]: " yn
                    if [[ "$yn" =~ ^[Nn] ]]; then
                        warn "Skipping system packages. Rust build may fail."
                        return
                    fi
                fi
                sudo apt-get update -qq
                sudo apt-get install -y -qq build-essential gfortran libopenblas-dev pkg-config
                ok "System libraries installed"
            elif command -v dnf &>/dev/null; then
                echo "  Installing gcc, gcc-fortran, openblas-devel, pkgconf..."
                sudo dnf install -y gcc gcc-fortran openblas-devel pkgconf
                ok "System libraries installed"
            else
                warn "Cannot auto-install OpenBLAS. Install manually."
            fi
        elif [[ "$OS_TYPE" == "macos" ]]; then
            if command -v brew &>/dev/null; then
                echo "  Installing openblas via Homebrew..."
                brew install openblas
                ok "OpenBLAS installed"
            else
                warn "Install Homebrew and then: brew install openblas"
            fi
        fi
    fi
}

install_wasm_deps() {
    echo ""
    echo -e "  ${CYAN}WASM dependencies:${RESET}"

    if ! $HAS_WASM_TARGET; then
        echo "  Adding wasm32-unknown-unknown target..."
        rustup target add wasm32-unknown-unknown
        ok "WASM target added"
    else
        ok "WASM target already installed"
    fi

    if ! $HAS_WASM_PACK; then
        echo "  Installing wasm-pack..."
        cargo install wasm-pack 2>&1 | tail -3
        ok "wasm-pack installed"
    else
        ok "wasm-pack already installed"
    fi
}

install_iot_deps() {
    echo ""
    echo -e "  ${CYAN}IoT (ESP32) dependencies:${RESET}"
    if $HAS_ESPIDF; then
        ok "ESP-IDF already available"
    else
        echo ""
        echo "  ESP-IDF is required for ESP32 firmware builds."
        echo "  Install guide: https://docs.espressif.com/projects/esp-idf/en/latest/esp32s3/get-started/"
        echo ""
        echo "  Quick install:"
        echo "    mkdir -p ~/esp && cd ~/esp"
        echo "    git clone --recursive https://github.com/espressif/esp-idf.git"
        echo "    cd esp-idf && git checkout v5.2"
        echo "    ./install.sh esp32s3"
        echo "    . ./export.sh"
        warn "ESP-IDF not installed. Aggregator will be built but firmware flashing requires ESP-IDF."
    fi
}

install_field_deps() {
    echo ""
    echo -e "  ${CYAN}Field deployment dependencies:${RESET}"
    ok "Using Rust toolchain for WiFi-Mat build"
    install_wasm_deps
}

check_docker_deps() {
    echo -e "  ${CYAN}Docker dependencies:${RESET}"
    if $HAS_DOCKER; then
        ok "Docker available"
        if docker compose version &>/dev/null; then
            ok "Docker Compose available"
        elif docker-compose --version &>/dev/null; then
            ok "Docker Compose (standalone) available"
        else
            warn "Docker Compose not found"
        fi
    else
        fail "Docker required for this profile"
        exit 1
    fi
}

# ======================================================================
#  STEP 6: BUILD
# ======================================================================

run_build() {
    step "6/7" "Building (profile: ${PROFILE})"
    echo ""

    case "$PROFILE" in
        verify)
            build_verify
            ;;
        python)
            build_verify
            build_python
            ;;
        rust)
            build_rust
            ;;
        browser)
            build_wasm
            ;;
        iot)
            build_rust_crate "wifi-densepose-hardware" "ESP32 aggregator"
            ;;
        docker)
            build_docker
            ;;
        field)
            build_rust_crate "wifi-densepose-mat" "WiFi-Mat disaster module"
            build_wasm_field
            ;;
        full)
            build_verify
            build_python
            build_rust
            if $HAS_WASM_PACK; then
                build_wasm
            fi
            if $HAS_DOCKER; then
                build_docker
            fi
            ;;
    esac
}

build_verify() {
    echo -e "  ${CYAN}Running pipeline verification...${RESET}"
    echo ""
    if "${SCRIPT_DIR}/verify" 2>&1; then
        ok "Pipeline verification PASSED"
    else
        warn "Pipeline verification returned non-zero (see output above)"
    fi
}

build_python() {
    echo ""
    echo -e "  ${CYAN}Setting up Python environment...${RESET}"

    # Create .env if it doesn't exist
    if [ ! -f "${SCRIPT_DIR}/.env" ] && [ -f "${SCRIPT_DIR}/example.env" ]; then
        cp "${SCRIPT_DIR}/example.env" "${SCRIPT_DIR}/.env"
        ok "Created .env from example.env"
    fi

    # Install package in development mode
    if [ -f "${SCRIPT_DIR}/pyproject.toml" ]; then
        echo "  Installing wifi-densepose in development mode..."
        (cd "${SCRIPT_DIR}" && $PYTHON_CMD -m pip install -e . 2>&1 | tail -3)
        ok "Package installed in dev mode"
    fi
}

build_rust() {
    echo -e "  ${CYAN}Building Rust workspace (release)...${RESET}"
    echo ""

    if [ ! -d "${RUST_DIR}" ]; then
        fail "Rust workspace not found at ${RUST_DIR}"
        return 1
    fi

    (cd "${RUST_DIR}" && cargo build --release 2>&1 | tail -10)
    local exit_code=$?

    if [ $exit_code -eq 0 ]; then
        ok "Rust workspace built successfully"

        # Show binary sizes
        echo ""
        echo -e "  ${BOLD}Build artifacts:${RESET}"
        local target_dir="${RUST_DIR}/target/release"
        for bin in wifi-densepose-cli wifi-densepose-api; do
            if [ -f "${target_dir}/${bin}" ]; then
                local size
                size=$(du -h "${target_dir}/${bin}" 2>/dev/null | cut -f1)
                info "  ${target_dir}/${bin} (${size})"
            fi
        done

        # Run tests
        echo ""
        echo -e "  ${CYAN}Running Rust tests...${RESET}"
        (cd "${RUST_DIR}" && cargo test --workspace 2>&1 | tail -5)
        ok "Rust tests completed"
    else
        fail "Rust build failed (exit code: ${exit_code})"
    fi
}

build_rust_crate() {
    local crate="$1"
    local label="$2"
    echo -e "  ${CYAN}Building ${label}...${RESET}"
    (cd "${RUST_DIR}" && cargo build --release --package "${crate}" 2>&1 | tail -5)
    ok "${label} built"
}

build_wasm() {
    echo -e "  ${CYAN}Building WASM package (browser profile ~10MB)...${RESET}"
    echo ""
    (cd "${RUST_DIR}" && wasm-pack build crates/wifi-densepose-wasm --target web --release 2>&1 | tail -10)

    if [ -d "${RUST_DIR}/crates/wifi-densepose-wasm/pkg" ]; then
        local wasm_size
        wasm_size=$(du -sh "${RUST_DIR}/crates/wifi-densepose-wasm/pkg" 2>/dev/null | cut -f1)
        ok "WASM package built (${wasm_size})"
        info "Output: ${RUST_DIR}/crates/wifi-densepose-wasm/pkg/"
    else
        warn "WASM package directory not found after build"
    fi
}

build_wasm_field() {
    echo ""
    echo -e "  ${CYAN}Building WASM package with WiFi-Mat (field profile ~62MB)...${RESET}"
    (cd "${RUST_DIR}" && wasm-pack build crates/wifi-densepose-wasm --target web --release -- --features mat 2>&1 | tail -10)

    if [ -d "${RUST_DIR}/crates/wifi-densepose-wasm/pkg" ]; then
        local wasm_size
        wasm_size=$(du -sh "${RUST_DIR}/crates/wifi-densepose-wasm/pkg" 2>/dev/null | cut -f1)
        ok "Field WASM package built (${wasm_size})"
    fi
}

build_docker() {
    echo -e "  ${CYAN}Building Docker image...${RESET}"
    echo ""

    local target="production"
    if $VERBOSE; then
        target="development"
    fi

    (cd "${SCRIPT_DIR}" && docker build --target "${target}" -t wifi-densepose:latest . 2>&1 | tail -10)

    if docker images wifi-densepose:latest --format "{{.Size}}" 2>/dev/null | head -1; then
        ok "Docker image built"
    fi
}

# ======================================================================
#  STEP 7: POST-INSTALL SUMMARY
# ======================================================================

post_install() {
    step "7/7" "Installation Complete"
    echo ""

    echo -e "${BOLD}======================================================================"
    echo "  WiFi-DensePose: Installation Summary"
    echo -e "======================================================================${RESET}"
    echo ""

    echo -e "  ${BOLD}Profile:${RESET}  ${PROFILE}"
    echo -e "  ${BOLD}OS:${RESET}       ${OS_TYPE} (${ARCH})"
    echo -e "  ${BOLD}RAM:${RESET}      ${TOTAL_RAM_MB} MB"
    if $HAS_WIFI; then
        echo -e "  ${BOLD}WiFi:${RESET}     ${WIFI_IFACE}"
    fi
    if $HAS_GPU; then
        echo -e "  ${BOLD}GPU:${RESET}      ${GPU_INFO}"
    fi
    echo ""

    echo -e "  ${BOLD}Next steps:${RESET}"
    echo ""

    case "$PROFILE" in
        verify)
            echo "    # Re-run verification at any time:"
            echo "    ./verify"
            echo ""
            echo "    # Upgrade to a richer profile:"
            echo "    ./install.sh --profile python   # Add API server"
            echo "    ./install.sh --profile rust     # Add Rust performance"
            ;;
        python)
            echo "    # Start the API server:"
            echo "    uvicorn v1.src.api.main:app --host 0.0.0.0 --port 8000"
            echo ""
            echo "    # Open API docs: http://localhost:8000/docs"
            echo ""
            if $HAS_WIFI; then
                echo "    # With WiFi detected (${WIFI_IFACE}), commodity sensing is available:"
                echo "    # The system can detect presence and motion via RSSI."
            fi
            ;;
        rust)
            echo "    # Run benchmarks:"
            echo "    cd rust-port/wifi-densepose-rs"
            echo "    cargo bench --package wifi-densepose-signal"
            echo ""
            echo "    # Start Rust API server:"
            echo "    cargo run --release --package wifi-densepose-api"
            ;;
        browser)
            echo "    # WASM package is at:"
            echo "    # rust-port/wifi-densepose-rs/crates/wifi-densepose-wasm/pkg/"
            echo ""
            echo "    # Open the 3D visualization:"
            echo "    python3 -m http.server 3000 --directory ui"
            echo "    # Then open: http://localhost:3000/viz.html"
            ;;
        iot)
            echo "    # 1. Configure WiFi credentials:"
            echo "    cp firmware/esp32-csi-node/sdkconfig.defaults.example \\"
            echo "       firmware/esp32-csi-node/sdkconfig.defaults"
            echo "    # Edit sdkconfig.defaults: set SSID, password, aggregator IP"
            echo ""
            echo "    # 2. Build firmware (Docker — no local ESP-IDF needed):"
            echo "    cd firmware/esp32-csi-node"
            echo "    docker run --rm -v \"\$(pwd):/project\" -w /project \\"
            echo "      espressif/idf:v5.2 bash -c 'idf.py set-target esp32s3 && idf.py build'"
            echo ""
            echo "    # 3. Flash to ESP32-S3 (replace COM7 with your port):"
            echo "    cd build && python -m esptool --chip esp32s3 --port COM7 \\"
            echo "      --baud 460800 write-flash @flash_args"
            echo ""
            echo "    # 4. Run the aggregator:"
            echo "    cargo run -p wifi-densepose-hardware --bin aggregator -- \\"
            echo "      --bind 0.0.0.0:5005 --verbose"
            ;;
        docker)
            echo "    # Development (with Postgres, Redis, Prometheus, Grafana):"
            echo "    docker compose up"
            echo ""
            echo "    # Production:"
            echo "    docker run -d -p 8000:8000 wifi-densepose:latest"
            ;;
        field)
            echo "    # WiFi-Mat disaster response module built."
            echo ""
            echo "    # Run WiFi-Mat tests:"
            echo "    cd rust-port/wifi-densepose-rs"
            echo "    cargo test --package wifi-densepose-mat"
            echo ""
            echo "    # Field deployment WASM package at:"
            echo "    # rust-port/wifi-densepose-rs/crates/wifi-densepose-wasm/pkg/"
            ;;
        full)
            echo "    # Verification:  ./verify"
            echo "    # Python API:    uvicorn v1.src.api.main:app --host 0.0.0.0 --port 8000"
            echo "    # Rust API:      cd rust-port/wifi-densepose-rs && cargo run --release --package wifi-densepose-api"
            echo "    # Benchmarks:    cd rust-port/wifi-densepose-rs && cargo bench"
            echo "    # Visualization: python3 -m http.server 3000 --directory ui"
            echo "    # Docker:        docker compose up"
            ;;
    esac

    echo ""
    echo -e "  ${BOLD}RVF Container Sizes:${RESET}"
    echo "    IoT (ESP32):        ~0.7 MB  (int4 quantized)"
    echo "    Browser (Chrome):   ~10 MB   (int8 quantized)"
    echo "    Mobile (WebView):   ~6 MB    (int8 quantized)"
    echo "    Field (Disaster):   ~62 MB   (fp16 weights)"
    echo ""

    echo -e "  ${BOLD}Documentation:${RESET}"
    echo "    Build guide:    docs/build-guide.md"
    echo "    Architecture:   docs/adr/"
    echo "    SOTA research:  docs/research/wifi-sensing-ruvector-sota-2026.md"
    echo ""

    echo -e "  ${BOLD}Trust verification:${RESET}"
    echo "    ./verify               # One-command proof replay"
    echo "    make verify-audit      # Full audit with mock scan"
    echo ""

    echo -e "  Install log saved to: ${INSTALL_LOG}"
    echo ""
    echo -e "${BOLD}======================================================================${RESET}"
}

# ======================================================================
#  MAIN
# ======================================================================

main() {
    banner
    detect_system
    detect_toolchains
    detect_wifi_hardware

    if $CHECK_ONLY; then
        echo ""
        echo -e "${BOLD}Hardware check complete. Run without --check-only to install.${RESET}"
        exit 0
    fi

    recommend_profile

    if [ -z "$PROFILE" ]; then
        echo "No profile selected. Exiting."
        exit 0
    fi

    # Confirm
    if ! $SKIP_CONFIRM; then
        echo ""
        read -rp "  Proceed with '${PROFILE}' installation? [Y/n]: " confirm
        if [[ "$confirm" =~ ^[Nn] ]]; then
            echo "  Cancelled."
            exit 0
        fi
    fi

    install_deps
    run_build
    post_install
}

main
