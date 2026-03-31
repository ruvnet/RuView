#!/usr/bin/env bash
# ============================================================================
# qemu-cli.sh - Unified QEMU ESP32-S3 testing CLI (ADR-061)
# Version: 1.0.0
#
# Single entry point for all QEMU testing operations.
# Run `qemu-cli.sh help` or `qemu-cli.sh --help` for usage.
# ============================================================================
set -euo pipefail

VERSION="1.0.0"

# --- Colors ----------------------------------------------------------------
if [[ -t 1 ]]; then
    RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
    BLUE='\033[0;34m'; CYAN='\033[0;36m'; BOLD='\033[1m'; RST='\033[0m'
else
    RED=''; GREEN=''; YELLOW=''; BLUE=''; CYAN=''; BOLD=''; RST=''
fi

# --- Resolve paths ---------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FIRMWARE_DIR="$PROJECT_ROOT/firmware/esp32-csi-node"
FUZZ_DIR="$FIRMWARE_DIR/test"

# --- Helpers ---------------------------------------------------------------
info()  { echo -e "${BLUE}[INFO]${RST}  $*"; }
ok()    { echo -e "${GREEN}[OK]${RST}    $*"; }
warn()  { echo -e "${YELLOW}[WARN]${RST}  $*"; }
err()   { echo -e "${RED}[ERROR]${RST} $*" >&2; }
die()   { err "$@"; exit 1; }

need_qemu() {
    detect_qemu >/dev/null 2>&1 || \
        die "QEMU not found. Install with: ${CYAN}qemu-cli.sh install${RST}"
}

detect_qemu() {
    # 1. Explicit env var
    if [[ -n "${QEMU_PATH:-}" ]] && [[ -x "$QEMU_PATH" ]]; then
        echo "$QEMU_PATH"; return 0
    fi
    # 2. On PATH
    local qemu
    qemu="$(command -v qemu-system-xtensa 2>/dev/null || true)"
    if [[ -n "$qemu" ]]; then echo "$qemu"; return 0; fi
    # 3. Espressif default build location
    local espressif_qemu="$HOME/.espressif/qemu/build/qemu-system-xtensa"
    if [[ -x "$espressif_qemu" ]]; then echo "$espressif_qemu"; return 0; fi
    return 1
}

detect_python() {
    command -v python3 2>/dev/null || command -v python 2>/dev/null || echo "python3"
}

# --- Command: help ---------------------------------------------------------
cmd_help() {
    cat <<EOF
${BOLD}qemu-cli.sh${RST} v${VERSION} - Unified QEMU ESP32-S3 testing CLI

${BOLD}USAGE${RST}
    qemu-cli.sh <command> [options]

${BOLD}COMMANDS${RST}
    ${CYAN}install${RST}             Install QEMU with ESP32-S3 support
    ${CYAN}test${RST}                Run single-node firmware test
    ${CYAN}mesh${RST} [N]            Run multi-node mesh test (default: 3 nodes)
    ${CYAN}swarm${RST} [args]        Run swarm configurator (qemu_swarm.py)
    ${CYAN}snapshot${RST} [args]     Run snapshot-based tests
    ${CYAN}chaos${RST} [args]        Run chaos / fault injection tests
    ${CYAN}fuzz${RST} [--duration N] Run all 3 fuzz targets (clang libFuzzer)
    ${CYAN}nvs${RST} [args]          Generate NVS test matrix
    ${CYAN}health${RST} <logfile>    Check firmware health from QEMU log
    ${CYAN}status${RST}              Show installation status and versions
    ${CYAN}help${RST}                Show this help message

${BOLD}EXAMPLES${RST}
    qemu-cli.sh install                     # Install QEMU
    qemu-cli.sh test                        # Run basic firmware test
    qemu-cli.sh test --timeout 120          # Test with longer timeout
    qemu-cli.sh swarm --preset smoke        # Quick swarm test
    qemu-cli.sh swarm --preset standard     # Standard 3-node test
    qemu-cli.sh swarm --list-presets        # List available presets
    qemu-cli.sh mesh 3                      # 3-node mesh test
    qemu-cli.sh chaos                       # Run chaos tests
    qemu-cli.sh fuzz --duration 60          # Fuzz for 60 seconds
    qemu-cli.sh nvs --list                  # List NVS configs
    qemu-cli.sh health build/qemu_output.log
    qemu-cli.sh status                      # Show what's installed

${BOLD}TAB COMPLETION${RST}
    Source the completions in your shell:
      eval "\$(qemu-cli.sh --completions)"

${BOLD}ENVIRONMENT${RST}
    QEMU_PATH       Path to qemu-system-xtensa binary (auto-detected)
    FUZZ_DURATION   Override fuzz duration in seconds (default: 30)
    FUZZ_JOBS       Parallel fuzzing jobs (default: 1)

EOF
}

# --- Command: install ------------------------------------------------------
cmd_install() {
    if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
        echo "Usage: qemu-cli.sh install"
        echo "Install QEMU with Espressif ESP32-S3 support."
        return 0
    fi
    local installer="$SCRIPT_DIR/install-qemu.sh"
    if [[ -f "$installer" ]]; then
        info "Running install-qemu.sh ..."
        bash "$installer" "$@"
    else
        info "No install-qemu.sh found. Showing manual install steps."
        cat <<EOF

${BOLD}Manual QEMU ESP32-S3 installation:${RST}
  1. git clone https://github.com/espressif/qemu.git ~/.espressif/qemu-src
  2. cd ~/.espressif/qemu-src
  3. ./configure --target-list=xtensa-softmmu --prefix=\$HOME/.espressif/qemu/build \\
       --enable-gcrypt --disable-bsd-user --disable-docs
  4. make -j\$(nproc) && make install
  5. Add to PATH: export PATH="\$HOME/.espressif/qemu/build/bin:\$PATH"

EOF
    fi
}

# --- Command: test ----------------------------------------------------------
cmd_test() {
    if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
        echo "Usage: qemu-cli.sh test [--timeout N] [extra args...]"
        echo "Run single-node QEMU ESP32-S3 firmware test."
        return 0
    fi
    need_qemu
    info "Running single-node firmware test ..."
    bash "$SCRIPT_DIR/qemu-esp32s3-test.sh" "$@"
}

# --- Command: mesh ----------------------------------------------------------
cmd_mesh() {
    if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
        echo "Usage: qemu-cli.sh mesh [N] [extra args...]"
        echo "Run multi-node mesh test. N = number of nodes (default: 3)."
        return 0
    fi
    need_qemu
    local nodes="${1:-3}"
    shift 2>/dev/null || true
    info "Running ${nodes}-node mesh test ..."
    bash "$SCRIPT_DIR/qemu-mesh-test.sh" "$nodes" "$@"
}

# --- Command: swarm ---------------------------------------------------------
cmd_swarm() {
    if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
        echo "Usage: qemu-cli.sh swarm [--preset NAME] [--list-presets] [args...]"
        echo "Run QEMU swarm configurator (qemu_swarm.py)."
        echo ""
        echo "Presets:  smoke, standard, full, stress"
        echo "List:     qemu-cli.sh swarm --list-presets"
        return 0
    fi
    need_qemu
    local py; py="$(detect_python)"
    info "Running swarm configurator ..."
    "$py" "$SCRIPT_DIR/qemu_swarm.py" "$@"
}

# --- Command: snapshot ------------------------------------------------------
cmd_snapshot() {
    if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
        echo "Usage: qemu-cli.sh snapshot [args...]"
        echo "Run snapshot-based QEMU tests."
        return 0
    fi
    need_qemu
    info "Running snapshot tests ..."
    bash "$SCRIPT_DIR/qemu-snapshot-test.sh" "$@"
}

# --- Command: chaos ---------------------------------------------------------
cmd_chaos() {
    if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
        echo "Usage: qemu-cli.sh chaos [args...]"
        echo "Run chaos / fault injection tests."
        return 0
    fi
    need_qemu
    info "Running chaos tests ..."
    bash "$SCRIPT_DIR/qemu-chaos-test.sh" "$@"
}

# --- Command: fuzz ----------------------------------------------------------
cmd_fuzz() {
    local duration="${FUZZ_DURATION:-30}"
    if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
        echo "Usage: qemu-cli.sh fuzz [--duration N]"
        echo "Build and run all 3 fuzz targets (clang libFuzzer)."
        echo "Requires: clang with libFuzzer support."
        return 0
    fi
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --duration) duration="$2"; shift 2 ;;
            *) warn "Unknown fuzz option: $1"; shift ;;
        esac
    done
    if ! command -v clang >/dev/null 2>&1; then
        die "clang not found. Fuzz targets require clang with libFuzzer."
    fi
    info "Building and running fuzz targets (${duration}s each) ..."
    make -C "$FUZZ_DIR" run_all FUZZ_DURATION="$duration"
    ok "Fuzz testing complete."
}

# --- Command: nvs -----------------------------------------------------------
cmd_nvs() {
    if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
        echo "Usage: qemu-cli.sh nvs [--list] [args...]"
        echo "Generate NVS test configuration matrix."
        return 0
    fi
    local py; py="$(detect_python)"
    info "Running NVS matrix generator ..."
    "$py" "$SCRIPT_DIR/generate_nvs_matrix.py" "$@"
}

# --- Command: health --------------------------------------------------------
cmd_health() {
    if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
        echo "Usage: qemu-cli.sh health <logfile>"
        echo "Analyze firmware health from a QEMU output log."
        return 0
    fi
    local logfile="${1:-}"
    if [[ -z "$logfile" ]]; then
        die "Usage: qemu-cli.sh health <logfile>"
    fi
    if [[ ! -f "$logfile" ]]; then
        die "Log file not found: $logfile"
    fi
    local py; py="$(detect_python)"
    info "Analyzing health from: $logfile"
    "$py" "$SCRIPT_DIR/check_health.py" --log "$logfile" --after-fault manual
}

# --- Command: status --------------------------------------------------------
cmd_status() {
    # Status should never fail - disable errexit locally
    set +e
    echo -e "${BOLD}=== QEMU ESP32-S3 Testing Status ===${RST}"
    echo ""

    # QEMU
    local qemu_bin
    qemu_bin="$(detect_qemu 2>/dev/null)"
    if [[ -n "$qemu_bin" ]]; then
        local qemu_ver
        qemu_ver="$("$qemu_bin" --version 2>/dev/null | head -1 || echo "unknown")"
        ok "QEMU: ${GREEN}installed${RST}  ($qemu_ver)"
        echo "       Path: $qemu_bin"
    else
        warn "QEMU: ${YELLOW}not found${RST}  (run: qemu-cli.sh install)"
    fi

    # ESP-IDF
    if [[ -n "${IDF_PATH:-}" ]] && [[ -d "$IDF_PATH" ]]; then
        ok "ESP-IDF: ${GREEN}available${RST}  ($IDF_PATH)"
    else
        warn "ESP-IDF: ${YELLOW}IDF_PATH not set${RST}"
    fi

    # Python
    local py; py="$(detect_python)"
    if command -v "$py" >/dev/null 2>&1; then
        ok "Python: ${GREEN}$("$py" --version 2>&1)${RST}"
    else
        warn "Python: ${YELLOW}not found${RST}"
    fi

    # Clang (for fuzz)
    if command -v clang >/dev/null 2>&1; then
        ok "Clang: ${GREEN}$(clang --version 2>/dev/null | head -1)${RST}"
    else
        warn "Clang: ${YELLOW}not found${RST} (needed for fuzz targets only)"
    fi

    # Firmware binary
    local fw_bin="$FIRMWARE_DIR/build/esp32-csi-node.bin"
    if [[ -f "$fw_bin" ]]; then
        local fw_size
        fw_size="$(stat -c%s "$fw_bin" 2>/dev/null || stat -f%z "$fw_bin" 2>/dev/null || echo "?")"
        ok "Firmware: ${GREEN}built${RST}  ($fw_bin, ${fw_size} bytes)"
    else
        warn "Firmware: ${YELLOW}not built${RST}  (expected at $fw_bin)"
    fi

    # Swarm presets
    local preset_dir="$SCRIPT_DIR/swarm_presets"
    if [[ -d "$preset_dir" ]]; then
        local presets
        presets="$(ls "$preset_dir"/ 2>/dev/null | \
                   sed 's/\.\(yaml\|json\)$//' | sort -u | tr '\n' ', ' | sed 's/,$//')"
        if [[ -n "$presets" ]]; then
            ok "Presets: ${GREEN}${presets}${RST}"
        else
            warn "Presets: ${YELLOW}none found${RST} in $preset_dir"
        fi
    fi

    echo ""
    set -e
}

# --- Completions output -----------------------------------------------------
print_completions() {
    cat <<'COMP'
_qemu_cli_completions() {
    local cmds="install test mesh swarm snapshot chaos fuzz nvs health status help"
    local cur="${COMP_WORDS[COMP_CWORD]}"
    if [[ $COMP_CWORD -eq 1 ]]; then
        COMPREPLY=( $(compgen -W "$cmds" - "$cur") )
    fi
}
complete -F _qemu_cli_completions qemu-cli.sh
COMP
}

# --- Main dispatch ----------------------------------------------------------
main() {
    local cmd="${1:-help}"
    shift 2>/dev/null || true

    case "$cmd" in
        install)        cmd_install "$@" ;;
        test)           cmd_test "$@" ;;
        mesh)           cmd_mesh "$@" ;;
        swarm)          cmd_swarm "$@" ;;
        snapshot)       cmd_snapshot "$@" ;;
        chaos)          cmd_chaos "$@" ;;
        fuzz)           cmd_fuzz "$@" ;;
        nvs)            cmd_nvs "$@" ;;
        health)         cmd_health "$@" ;;
        status)         cmd_status "$@" ;;
        help|-h|--help) cmd_help ;;
        --version)      echo "qemu-cli.sh v${VERSION}" ;;
        --completions)  print_completions ;;
        *)
            err "Unknown command: ${BOLD}${cmd}${RST}"
            echo ""
            cmd_help
            exit 1
            ;;
    esac
}

main "$@"
