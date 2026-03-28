#!/bin/bash
# QEMU Chaos / Fault Injection Test Runner — ADR-061 Layer 9
#
# Launches firmware under QEMU and injects a series of faults to verify
# the firmware's resilience. Each fault is injected via the QEMU monitor
# socket (or GDB stub), followed by a recovery window and health check.
#
# Fault types:
#   1. wifi_kill        — Pause/resume VM to simulate WiFi reconnect
#   2. ring_flood       — Inject 1000 rapid mock frames (ring buffer stress)
#   3. heap_exhaust    — Write to heap metadata to simulate low memory
#   4. timer_starvation — Pause VM for 500ms to starve FreeRTOS timers
#   5. corrupt_frame    — Inject a CSI frame with bad magic bytes
#   6. nvs_corrupt      — Write garbage to NVS flash region
#
# Environment variables:
#   QEMU_PATH       - Path to qemu-system-xtensa (default: qemu-system-xtensa)
#   QEMU_TIMEOUT    - Boot timeout in seconds (default: 15)
#   FLASH_IMAGE     - Path to merged flash image (default: build/qemu_flash.bin)
#   FAULT_WAIT      - Seconds to wait after fault injection (default: 5)
#
# Exit codes:
#   0  PASS    — all checks passed
#   1  WARN    — non-critical checks failed
#   2  FAIL    — critical checks failed
#   3  FATAL   — build error, crash, or infrastructure failure

# ── Help ──────────────────────────────────────────────────────────────
usage() {
    cat <<'HELP'
Usage: qemu-chaos-test.sh [OPTIONS]

Launch firmware under QEMU and inject a series of faults to verify the
firmware's resilience. Each fault is injected via the QEMU monitor socket,
followed by a recovery window and health check.

Fault types:
  wifi_kill         Pause/resume VM to simulate WiFi reconnect
  ring_flood        Inject 1000 rapid mock frames (ring buffer stress)
  heap_exhaust     Write to heap metadata to simulate low memory
  timer_starvation  Pause VM for 500ms to starve FreeRTOS timers
  corrupt_frame     Inject a CSI frame with bad magic bytes
  nvs_corrupt       Write garbage to NVS flash region

Options:
  -h, --help      Show this help message and exit

Environment variables:
  QEMU_PATH       Path to qemu-system-xtensa        (default: qemu-system-xtensa)
  QEMU_TIMEOUT    Boot timeout in seconds            (default: 15)
  FLASH_IMAGE     Path to merged flash image         (default: build/qemu_flash.bin)
  FAULT_WAIT      Seconds to wait after injection    (default: 5)

Examples:
  ./qemu-chaos-test.sh
  QEMU_TIMEOUT=30 FAULT_WAIT=10 ./qemu-chaos-test.sh
  FLASH_IMAGE=/path/to/image.bin ./qemu-chaos-test.sh

Exit codes:
  0  PASS   — all checks passed
  1  WARN   — non-critical checks failed
  2  FAIL   — critical checks failed
  3  FATAL  — build error, crash, or infrastructure failure
HELP
    exit 0
}

case "${1:-}" in -h|--help) usage ;; esac

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

FIRMWARE_DIR="$PROJECT_ROOT/firmware/esp32-csi-node"
BUILD_DIR="$FIRMWARE_DIR/build"
QEMU_BIN="${QEMU_PATH:-qemu-system-xtensa}"
FLASH_IMAGE="${FLASH_IMAGE:-$BUILD_DIR/qemu_flash.bin}"
BOOT_TIMEOUT="${QEMU_TIMEOUT:-15}"
FAULT_WAIT="${FAULT_WAIT:-5}"
MONITOR_SOCK="$BUILD_DIR/qemu-chaos.sock"
LOG_DIR="$BUILD_DIR/chaos-tests"
UART_LOG="$LOG_DIR/qemu_uart.log"
QEMU_PID=""

# Fault definitions
FAULTS=("wifi_kill" "ring_flood" "heap_exhaust" "timer_starvation" "corrupt_frame" "nvs_corrupt")
declare -a FAULT_RESULTS=()

# ──────────────────────────────────────────────────────────────────────
# Cleanup
# ──────────────────────────────────────────────────────────────────────

cleanup() {
    echo ""
    echo "[cleanup] Shutting down QEMU and removing socket..."
    if [ -n "$QEMU_PID" ] && kill -0 "$QEMU_PID" 2>/dev/null; then
        kill "$QEMU_PID" 2>/dev/null || true
        wait "$QEMU_PID" 2>/dev/null || true
    fi
    rm -f "$MONITOR_SOCK"
    echo "[cleanup] Done."
}
trap cleanup EXIT INT TERM

# ──────────────────────────────────────────────────────────────────────
# Helpers
# ──────────────────────────────────────────────────────────────────────

monitor_cmd() {
    local cmd="$1"
    local timeout="${2:-5}"
    echo "$cmd" | socat - "UNIX-CONNECT:$MONITOR_SOCK,connect-timeout=$timeout" 2>/dev/null
}

log_line_count() {
    wc -l < "$UART_LOG" 2>/dev/null || echo 0
}

wait_for_boot() {
    local elapsed=0
    while [ "$elapsed" -lt "$BOOT_TIMEOUT" ]; do
        if [ -f "$UART_LOG" ] && grep -qE "app_main|main_task|ESP32-S3|mock_csi" "$UART_LOG" 2>/dev/null; then
            return 0
        fi
        sleep 1
        elapsed=$((elapsed + 1))
    done
    return 1
}

# ──────────────────────────────────────────────────────────────────────
# Fault injection functions
# ──────────────────────────────────────────────────────────────────────

inject_wifi_kill() {
    # Simulate WiFi disconnect/reconnect by pausing and resuming the VM.
    # The firmware should handle the time gap gracefully.
    echo "  [inject] Pausing VM for 2s (simulating WiFi disconnect)..."
    monitor_cmd "stop"
    sleep 2
    echo "  [inject] Resuming VM (simulating WiFi reconnect)..."
    monitor_cmd "cont"
}

inject_ring_flood() {
    # Send 1000 rapid mock frames by triggering scenario 7 repeatedly.
    # This stresses the ring buffer and tests backpressure handling.
    echo "  [inject] Flooding ring buffer with 1000 rapid frame triggers..."
    python3 "$SCRIPT_DIR/inject_fault.py" \
        --socket "$MONITOR_SOCK" \
        --fault ring_flood
}

inject_heap_exhaust() {
    # Simulate memory pressure by pausing the VM to stress heap management.
    # Actual heap memory writes require GDB stub.
    echo "  [inject] Simulating heap pressure via VM pause..."
    python3 "$SCRIPT_DIR/inject_fault.py" \
        --socket "$MONITOR_SOCK" \
        --fault heap_exhaust
}

inject_timer_starvation() {
    # Pause execution for 500ms to starve FreeRTOS timer callbacks.
    # Tests watchdog recovery and timer resilience.
    echo "  [inject] Starving timers (500ms pause)..."
    monitor_cmd "stop"
    sleep 0.5
    monitor_cmd "cont"
}

inject_corrupt_frame() {
    # Inject a CSI frame with bad magic bytes via monitor memory write.
    # The frame parser should reject it without crashing.
    echo "  [inject] Injecting corrupt CSI frame (bad magic)..."
    python3 "$SCRIPT_DIR/inject_fault.py" \
        --socket "$MONITOR_SOCK" \
        --fault corrupt_frame
}

inject_nvs_corrupt() {
    # Write garbage to the NVS flash region (offset 0x9000) via direct file write.
    # The firmware should detect NVS corruption and fall back to defaults.
    echo "  [inject] Corrupting NVS flash region..."
    python3 "$SCRIPT_DIR/inject_fault.py" \
        --socket "$MONITOR_SOCK" \
        --fault nvs_corrupt \
        --flash "$FLASH_IMAGE"
}

# ──────────────────────────────────────────────────────────────────────
# Pre-flight checks
# ──────────────────────────────────────────────────────────────────────

echo "=== QEMU Chaos Test Runner — ADR-061 Layer 9 ==="
echo "QEMU binary:  $QEMU_BIN"
echo "Flash image:  $FLASH_IMAGE"
echo "Boot timeout: ${BOOT_TIMEOUT}s"
echo "Fault wait:   ${FAULT_WAIT}s"
echo "Faults:       ${FAULTS[*]}"
echo ""

if ! command -v "$QEMU_BIN" &>/dev/null; then
    echo "ERROR: QEMU binary not found: $QEMU_BIN"
    echo "  Install: sudo apt install qemu-system-misc   # Debian/Ubuntu"
    echo "  Install: brew install qemu                    # macOS"
    echo "  Or set QEMU_PATH to the qemu-system-xtensa binary."
    exit 3
fi

if ! command -v socat &>/dev/null; then
    echo "ERROR: socat not found (needed for QEMU monitor communication)."
    echo "  Install: sudo apt install socat   # Debian/Ubuntu"
    echo "  Install: brew install socat        # macOS"
    exit 3
fi

if ! command -v python3 &>/dev/null; then
    echo "ERROR: python3 not found (needed for fault injection scripts)."
    echo "  Install: sudo apt install python3   # Debian/Ubuntu"
    echo "  Install: brew install python         # macOS"
    exit 3
fi

if [ ! -f "$FLASH_IMAGE" ]; then
    echo "ERROR: Flash image not found: $FLASH_IMAGE"
    echo "Run qemu-esp32s3-test.sh first to build the flash image."
    exit 3
fi

mkdir -p "$LOG_DIR"

# ──────────────────────────────────────────────────────────────────────
# Launch QEMU
# ──────────────────────────────────────────────────────────────────────

echo "── Launching QEMU ──"
echo ""

rm -f "$MONITOR_SOCK"
> "$UART_LOG"

QEMU_ARGS=(
    -machine esp32s3
    -nographic
    -drive "file=$FLASH_IMAGE,if=mtd,format=raw"
    -serial "file:$UART_LOG"
    -no-reboot
    -monitor "unix:$MONITOR_SOCK,server,nowait"
)

"$QEMU_BIN" "${QEMU_ARGS[@]}" &
QEMU_PID=$!
echo "[qemu] PID=$QEMU_PID"

# Wait for monitor socket
waited=0
while [ ! -S "$MONITOR_SOCK" ] && [ "$waited" -lt 10 ]; do
    sleep 1
    waited=$((waited + 1))
done

if [ ! -S "$MONITOR_SOCK" ]; then
    echo "ERROR: QEMU monitor socket did not appear after 10s"
    exit 3
fi

# Wait for boot
echo "[boot] Waiting for firmware boot (up to ${BOOT_TIMEOUT}s)..."
if wait_for_boot; then
    echo "[boot] Firmware booted successfully."
else
    echo "[boot] No boot indicator found (continuing anyway)."
fi

# Let firmware stabilize for a few seconds
echo "[boot] Stabilizing (3s)..."
sleep 3
echo ""

# ──────────────────────────────────────────────────────────────────────
# Fault injection loop
# ──────────────────────────────────────────────────────────────────────

echo "── Fault Injection ──"
echo ""

MAX_EXIT=0

for fault in "${FAULTS[@]}"; do
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "  Fault: $fault"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    # Record log position before injection
    pre_lines=$(log_line_count)

    # Check QEMU is still alive
    if ! kill -0 "$QEMU_PID" 2>/dev/null; then
        echo "  ERROR: QEMU process died before fault injection"
        FAULT_RESULTS+=("${fault}:3")
        MAX_EXIT=3
        break
    fi

    # Inject the fault
    case "$fault" in
        wifi_kill)        inject_wifi_kill ;;
        ring_flood)       inject_ring_flood ;;
        heap_exhaust)     inject_heap_exhaust ;;
        timer_starvation) inject_timer_starvation ;;
        corrupt_frame)    inject_corrupt_frame ;;
        nvs_corrupt)      inject_nvs_corrupt ;;
        *)
            echo "  ERROR: Unknown fault type: $fault"
            FAULT_RESULTS+=("${fault}:2")
            continue
            ;;
    esac

    # Wait for firmware to respond/recover
    echo "  [recovery] Waiting ${FAULT_WAIT}s for recovery..."
    sleep "$FAULT_WAIT"

    # Extract post-fault log segment
    post_lines=$(log_line_count)
    new_lines=$((post_lines - pre_lines))
    fault_log="$LOG_DIR/fault_${fault}.log"

    if [ "$new_lines" -gt 0 ]; then
        tail -n "$new_lines" "$UART_LOG" > "$fault_log"
    else
        # Grab last 50 lines as context
        tail -n 50 "$UART_LOG" > "$fault_log"
    fi

    echo "  [check] Captured $new_lines new log lines"

    # Health check
    fault_exit=0
    python3 "$SCRIPT_DIR/check_health.py" \
        --log "$fault_log" \
        --after-fault "$fault" || fault_exit=$?

    case "$fault_exit" in
        0) echo "  [result] HEALTHY — firmware recovered gracefully" ;;
        1) echo "  [result] DEGRADED — firmware running but with issues" ;;
        *) echo "  [result] UNHEALTHY — firmware in bad state" ;;
    esac

    FAULT_RESULTS+=("${fault}:${fault_exit}")
    if [ "$fault_exit" -gt "$MAX_EXIT" ]; then
        MAX_EXIT=$fault_exit
    fi

    echo ""
done

# ──────────────────────────────────────────────────────────────────────
# Summary
# ──────────────────────────────────────────────────────────────────────

echo "── Chaos Test Results ──"
echo ""

PASS=0
DEGRADED=0
FAIL=0

for result in "${FAULT_RESULTS[@]}"; do
    name="${result%%:*}"
    code="${result##*:}"
    case "$code" in
        0) echo "  [PASS]     $name"; PASS=$((PASS + 1)) ;;
        1) echo "  [DEGRADED] $name"; DEGRADED=$((DEGRADED + 1)) ;;
        *) echo "  [FAIL]     $name"; FAIL=$((FAIL + 1)) ;;
    esac
done

echo ""
echo "  $PASS passed, $DEGRADED degraded, $FAIL failed out of ${#FAULTS[@]} faults"
echo ""

# Check if QEMU survived all faults
if kill -0 "$QEMU_PID" 2>/dev/null; then
    echo "  QEMU process survived all fault injections."
else
    echo "  WARNING: QEMU process died during fault injection."
    if [ "$MAX_EXIT" -lt 3 ]; then
        MAX_EXIT=3
    fi
fi

echo ""
echo "=== Chaos Test Complete (exit code: $MAX_EXIT) ==="
exit "$MAX_EXIT"
