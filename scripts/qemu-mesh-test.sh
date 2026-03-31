#!/bin/bash
# QEMU ESP32-S3 Multi-Node Mesh Simulation (ADR-061 Layer 3)
#
# Spawns N ESP32-S3 QEMU instances connected via a Linux bridge, each with
# unique NVS provisioning (node ID, TDM slot), and a Rust aggregator that
# collects frames from all nodes.  After a configurable timeout the script
# tears everything down and runs validate_mesh_test.py.
#
# Usage:
#   sudo ./qemu-mesh-test.sh [N_NODES]
#
# Environment variables:
#   QEMU_PATH       - Path to qemu-system-xtensa (default: qemu-system-xtensa)
#   QEMU_TIMEOUT    - Timeout in seconds (default: 45)
#   MESH_TIMEOUT    - Deprecated alias for QEMU_TIMEOUT
#   SKIP_BUILD      - Set to "1" to skip the idf.py build step
#   BRIDGE_NAME     - Bridge interface name (default: qemu-br0)
#   BRIDGE_SUBNET   - Bridge IP/mask (default: 10.0.0.1/24)
#   AGGREGATOR_PORT - UDP port the aggregator listens on (default: 5005)
#
# Prerequisites:
#   - Linux with bridge-utils and iproute2
#   - QEMU with ESP32-S3 machine support (qemu-system-xtensa)
#   - provision.py capable of --dry-run NVS generation
#   - Rust workspace with wifi-densepose-hardware crate (aggregator binary)
#
# Exit codes:
#   0  PASS    - all checks passed
#   1  WARN    - non-critical checks failed
#   2  FAIL    - critical checks failed
#   3  FATAL   - build error, crash, or infrastructure failure

# ── Help ──────────────────────────────────────────────────────────────
usage() {
    cat <<'HELP'
Usage: sudo ./qemu-mesh-test.sh [OPTIONS] [N_NODES]

Spawn N ESP32-S3 QEMU instances connected via a Linux bridge, each with
unique NVS provisioning (node ID, TDM slot), and a Rust aggregator that
collects frames from all nodes.

NOTE: Requires root/sudo for TAP/bridge creation.

Options:
  -h, --help      Show this help message and exit

Positional:
  N_NODES         Number of mesh nodes (default: 3, minimum: 2)

Environment variables:
  QEMU_PATH       Path to qemu-system-xtensa        (default: qemu-system-xtensa)
  QEMU_TIMEOUT    Timeout in seconds                 (default: 45)
  MESH_TIMEOUT    Alias for QEMU_TIMEOUT (deprecated)(default: 45)
  SKIP_BUILD      Set to "1" to skip idf.py build    (default: unset)
  BRIDGE_NAME     Bridge interface name               (default: qemu-br0)
  BRIDGE_SUBNET   Bridge IP/mask                      (default: 10.0.0.1/24)
  AGGREGATOR_PORT UDP port for aggregator             (default: 5005)

Examples:
  sudo ./qemu-mesh-test.sh
  sudo QEMU_TIMEOUT=90 ./qemu-mesh-test.sh 5
  sudo SKIP_BUILD=1 ./qemu-mesh-test.sh 4

Exit codes:
  0  PASS   - all checks passed
  1  WARN   - non-critical checks failed
  2  FAIL   - critical checks failed
  3  FATAL  - build error, crash, or infrastructure failure
HELP
    exit 0
}

case "${1:-}" in -h|--help) usage ;; esac

set -euo pipefail

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

FIRMWARE_DIR="$PROJECT_ROOT/firmware/esp32-csi-node"
BUILD_DIR="$FIRMWARE_DIR/build"
RUST_DIR="$PROJECT_ROOT/rust-port/wifi-densepose-rs"
PROVISION_SCRIPT="$FIRMWARE_DIR/provision.py"
VALIDATE_SCRIPT="$SCRIPT_DIR/validate_mesh_test.py"

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
N_NODES="${1:-3}"
QEMU_BIN="${QEMU_PATH:-qemu-system-xtensa}"
TIMEOUT="${QEMU_TIMEOUT:-${MESH_TIMEOUT:-45}}"
BRIDGE="${BRIDGE_NAME:-qemu-br0}"
BRIDGE_IP="${BRIDGE_SUBNET:-10.0.0.1/24}"
AGG_PORT="${AGGREGATOR_PORT:-5005}"
RESULTS_FILE="$BUILD_DIR/mesh_test_results.json"

echo "=== QEMU Multi-Node Mesh Test (ADR-061 Layer 3) ==="
echo "Nodes:        $N_NODES"
echo "Bridge:       $BRIDGE ($BRIDGE_IP)"
echo "Aggregator:   0.0.0.0:$AGG_PORT"
echo "QEMU binary:  $QEMU_BIN"
echo "Timeout:      ${TIMEOUT}s"
echo ""

# ---------------------------------------------------------------------------
# Preflight checks
# ---------------------------------------------------------------------------
if [ "$N_NODES" -lt 2 ]; then
    echo "ERROR: Need at least 2 nodes for mesh simulation (got $N_NODES)"
    exit 3
fi

if ! command -v "$QEMU_BIN" &>/dev/null; then
    echo "ERROR: QEMU binary not found: $QEMU_BIN"
    echo "  Install: sudo apt install qemu-system-misc   # Debian/Ubuntu"
    echo "  Install: brew install qemu                    # macOS"
    echo "  Or set QEMU_PATH to the qemu-system-xtensa binary."
    exit 3
fi

if ! command -v python3 &>/dev/null; then
    echo "ERROR: python3 not found."
    echo "  Install: sudo apt install python3   # Debian/Ubuntu"
    echo "  Install: brew install python         # macOS"
    exit 3
fi

if ! command -v ip &>/dev/null; then
    echo "ERROR: 'ip' command not found."
    echo "  Install: sudo apt install iproute2   # Debian/Ubuntu"
    exit 3
fi

if ! command -v brctl &>/dev/null && ! ip link help bridge &>/dev/null 2>&1; then
    echo "WARNING: bridge-utils not found; will use 'ip link' for bridge creation."
fi

if command -v socat &>/dev/null; then
    true  # optional, available
else
    echo "NOTE: socat not found (optional, used for advanced monitor communication)."
    echo "  Install: sudo apt install socat   # Debian/Ubuntu"
    echo "  Install: brew install socat        # macOS"
fi

if ! command -v cargo &>/dev/null; then
    echo "ERROR: cargo not found (needed to build the Rust aggregator)."
    echo "  Install: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 3
fi

if [ "$(id -u)" -ne 0 ]; then
    echo "ERROR: This script must be run as root (for TAP/bridge creation)."
    echo "Usage: sudo $0 [N_NODES]"
    exit 3
fi

mkdir -p "$BUILD_DIR"

# ---------------------------------------------------------------------------
# Cleanup trap - runs on EXIT regardless of success/failure
# ---------------------------------------------------------------------------
QEMU_PIDS=()
AGG_PID=""

cleanup() {
    echo ""
    echo "--- Cleaning up ---"

    # Kill QEMU instances
    for pid in "${QEMU_PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
    done

    # Kill aggregator
    if [ -n "$AGG_PID" ] && kill -0 "$AGG_PID" 2>/dev/null; then
        kill "$AGG_PID" 2>/dev/null || true
        wait "$AGG_PID" 2>/dev/null || true
    fi

    # Tear down TAP interfaces and bridge
    for i in $(seq 0 $((N_NODES - 1))); do
        local tap="tap${i}"
        if ip link show "$tap" &>/dev/null; then
            ip link set "$tap" down 2>/dev/null || true
            ip link delete "$tap" 2>/dev/null || true
        fi
    done

    if ip link show "$BRIDGE" &>/dev/null; then
        ip link set "$BRIDGE" down 2>/dev/null || true
        ip link delete "$BRIDGE" type bridge 2>/dev/null || true
    fi

    echo "Cleanup complete."
}

trap cleanup EXIT

# ---------------------------------------------------------------------------
# 1. Build flash image (if not already built)
# ---------------------------------------------------------------------------
if [ "${SKIP_BUILD:-}" != "1" ]; then
    echo "[1/6] Building firmware (mock CSI + QEMU overlay)..."
    idf.py -C "$FIRMWARE_DIR" \
        -D SDKCONFIG_DEFAULTS="sdkconfig.defaults;sdkconfig.qemu" \
        build
    echo ""
else
    echo "[1/6] Skipping build (SKIP_BUILD=1)"
    echo ""
fi

# Verify build artifacts
FLASH_IMAGE_BASE="$BUILD_DIR/qemu_flash_base.bin"
for artifact in \
    "$BUILD_DIR/bootloader/bootloader.bin" \
    "$BUILD_DIR/partition_table/partition-table.bin" \
    "$BUILD_DIR/esp32-csi-node.bin"; do
    if [ ! -f "$artifact" ]; then
        echo "ERROR: Build artifact not found: $artifact"
        echo "Run without SKIP_BUILD=1 or build the firmware first."
        exit 3
    fi
done

# Merge into base flash image
echo "[2/6] Creating base flash image..."
OTA_DATA_ARGS=""
if [ -f "$BUILD_DIR/ota_data_initial.bin" ]; then
    OTA_DATA_ARGS="0xf000 $BUILD_DIR/ota_data_initial.bin"
fi

python3 -m esptool --chip esp32s3 merge_bin -o "$FLASH_IMAGE_BASE" \
    --flash_mode dio --flash_freq 80m --flash_size 8MB \
    0x0     "$BUILD_DIR/bootloader/bootloader.bin" \
    0x8000  "$BUILD_DIR/partition_table/partition-table.bin" \
    $OTA_DATA_ARGS \
    0x20000 "$BUILD_DIR/esp32-csi-node.bin"

echo "Base flash image: $FLASH_IMAGE_BASE ($(stat -c%s "$FLASH_IMAGE_BASE" 2>/dev/null || stat -f%z "$FLASH_IMAGE_BASE") bytes)"
echo ""

# ---------------------------------------------------------------------------
# 3. Generate per-node NVS and flash images
# ---------------------------------------------------------------------------
echo "[3/6] Generating per-node NVS images..."

# Extract the aggregator IP from the bridge subnet (first host)
AGG_IP="${BRIDGE_IP%%/*}"

for i in $(seq 0 $((N_NODES - 1))); do
    NVS_BIN="$BUILD_DIR/nvs_node${i}.bin"
    NODE_FLASH="$BUILD_DIR/qemu_flash_node${i}.bin"

    # Generate NVS with provision.py --dry-run
    # --port is required by argparse but unused in dry-run; pass a dummy
    python3 "$PROVISION_SCRIPT" \
        --port /dev/null \
        --dry-run \
        --node-id "$i" \
        --tdm-slot "$i" \
        --tdm-total "$N_NODES" \
        --target-ip "$AGG_IP" \
        --target-port "$AGG_PORT"

    # provision.py --dry-run writes to nvs_provision.bin in CWD
    if [ -f "nvs_provision.bin" ]; then
        mv "nvs_provision.bin" "$NVS_BIN"
    else
        echo "ERROR: provision.py did not produce nvs_provision.bin for node $i"
        exit 3
    fi

    # Copy base image and inject NVS at 0x9000
    cp "$FLASH_IMAGE_BASE" "$NODE_FLASH"
    dd if="$NVS_BIN" of="$NODE_FLASH" \
        bs=1 seek=$((0x9000)) conv=notrunc 2>/dev/null

    echo "  Node $i: flash=$NODE_FLASH nvs=$NVS_BIN (TDM slot $i/$N_NODES)"
done
echo ""

# ---------------------------------------------------------------------------
# 4. Create bridge and TAP interfaces
# ---------------------------------------------------------------------------
echo "[4/6] Setting up network bridge and TAP interfaces..."

# Create bridge
ip link add name "$BRIDGE" type bridge 2>/dev/null || true
ip addr add "$BRIDGE_IP" dev "$BRIDGE" 2>/dev/null || true
ip link set "$BRIDGE" up

# Create TAP interfaces and attach to bridge
for i in $(seq 0 $((N_NODES - 1))); do
    TAP="tap${i}"
    ip tuntap add dev "$TAP" mode tap 2>/dev/null || true
    ip link set "$TAP" master "$BRIDGE"
    ip link set "$TAP" up
    echo "  $TAP -> $BRIDGE"
done
echo ""

# ---------------------------------------------------------------------------
# 5. Start aggregator and QEMU instances
# ---------------------------------------------------------------------------
echo "[5/6] Starting aggregator and $N_NODES QEMU nodes..."

# Start Rust aggregator in background
echo "  Starting aggregator: listen=0.0.0.0:$AGG_PORT expect-nodes=$N_NODES"
cargo run --manifest-path "$RUST_DIR/Cargo.toml" \
    -p wifi-densepose-hardware --bin aggregator - \
    --listen "0.0.0.0:$AGG_PORT" \
    --expect-nodes "$N_NODES" \
    --output "$RESULTS_FILE" \
    > "$BUILD_DIR/aggregator.log" 2>&1 &
AGG_PID=$!
echo "  Aggregator PID: $AGG_PID"

# Give aggregator a moment to bind
sleep 1

if ! kill -0 "$AGG_PID" 2>/dev/null; then
    echo "ERROR: Aggregator failed to start. Check $BUILD_DIR/aggregator.log"
    cat "$BUILD_DIR/aggregator.log" 2>/dev/null || true
    exit 3
fi

# Launch QEMU instances
for i in $(seq 0 $((N_NODES - 1))); do
    TAP="tap${i}"
    NODE_FLASH="$BUILD_DIR/qemu_flash_node${i}.bin"
    NODE_LOG="$BUILD_DIR/qemu_node${i}.log"
    NODE_MAC=$(printf "52:54:00:00:00:%02x" "$i")

    echo "  Starting QEMU node $i (tap=$TAP, mac=$NODE_MAC)..."

    "$QEMU_BIN" \
        -machine esp32s3 \
        -nographic \
        -drive "file=$NODE_FLASH,if=mtd,format=raw" \
        -serial "file:$NODE_LOG" \
        -no-reboot \
        -nic "tap,ifname=$TAP,script=no,downscript=no,mac=$NODE_MAC" \
        > /dev/null 2>&1 &

    QEMU_PIDS+=($!)
    echo "    PID: ${QEMU_PIDS[-1]}, log: $NODE_LOG"
done

echo ""
echo "All nodes launched. Waiting ${TIMEOUT}s for mesh simulation..."
echo ""

# ---------------------------------------------------------------------------
# Wait for timeout
# ---------------------------------------------------------------------------
sleep "$TIMEOUT"

echo "Timeout reached. Stopping all processes..."

# Kill QEMU instances (aggregator killed in cleanup)
for pid in "${QEMU_PIDS[@]}"; do
    if kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null || true
    fi
done

# Give aggregator a moment to flush results
sleep 2

# Kill aggregator
if [ -n "$AGG_PID" ] && kill -0 "$AGG_PID" 2>/dev/null; then
    kill "$AGG_PID" 2>/dev/null || true
    wait "$AGG_PID" 2>/dev/null || true
fi

echo ""

# ---------------------------------------------------------------------------
# 6. Validate results
# ---------------------------------------------------------------------------
echo "[6/6] Validating mesh test results..."

VALIDATE_ARGS=("--nodes" "$N_NODES")

# Pass results file if it was produced
if [ -f "$RESULTS_FILE" ]; then
    VALIDATE_ARGS+=("--results" "$RESULTS_FILE")
else
    echo "WARNING: Aggregator results file not found: $RESULTS_FILE"
    echo "Validation will rely on node logs only."
fi

# Pass node log files
for i in $(seq 0 $((N_NODES - 1))); do
    NODE_LOG="$BUILD_DIR/qemu_node${i}.log"
    if [ -f "$NODE_LOG" ]; then
        VALIDATE_ARGS+=("--log" "$NODE_LOG")
    fi
done

python3 "$VALIDATE_SCRIPT" "${VALIDATE_ARGS[@]}"
VALIDATE_EXIT=$?

echo ""
echo "=== Mesh Test Complete (exit code: $VALIDATE_EXIT) ==="
exit $VALIDATE_EXIT
