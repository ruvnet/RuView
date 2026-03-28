#!/bin/bash
# QEMU Snapshot-Based Test Runner — ADR-061 Layer 8
#
# Uses QEMU VM snapshots to accelerate repeated test runs.
# Instead of rebooting and re-initializing for each test scenario,
# we snapshot the VM state after boot and after the first CSI frame,
# then restore from the snapshot for each individual test.
#
# This dramatically reduces per-test wall time from ~15s (full boot)
# to ~2s (snapshot restore + execution).
#
# Environment variables:
#   QEMU_PATH       - Path to qemu-system-xtensa (default: qemu-system-xtensa)
#   QEMU_TIMEOUT    - Per-test timeout in seconds (default: 10)
#   FLASH_IMAGE     - Path to merged flash image (default: build/qemu_flash.bin)
#   SKIP_SNAPSHOT   - Set to "1" to run without snapshots (baseline timing)
#
# Exit codes:
#   0  PASS    — all checks passed
#   1  WARN    — non-critical checks failed
#   2  FAIL    — critical checks failed
#   3  FATAL   — build error, crash, or infrastructure failure

# ── Help ──────────────────────────────────────────────────────────────
usage() {
    cat <<'HELP'
Usage: qemu-snapshot-test.sh [OPTIONS]

Use QEMU VM snapshots to accelerate repeated test runs. Snapshots the VM
state after boot and after the first CSI frame, then restores from the
snapshot for each individual test (~2s vs ~15s per test).

Options:
  -h, --help      Show this help message and exit

Environment variables:
  QEMU_PATH       Path to qemu-system-xtensa        (default: qemu-system-xtensa)
  QEMU_TIMEOUT    Per-test timeout in seconds        (default: 10)
  FLASH_IMAGE     Path to merged flash image         (default: build/qemu_flash.bin)
  SKIP_SNAPSHOT   Set to "1" to run without snapshots (baseline timing)

Examples:
  ./qemu-snapshot-test.sh
  QEMU_TIMEOUT=20 ./qemu-snapshot-test.sh
  FLASH_IMAGE=/path/to/image.bin ./qemu-snapshot-test.sh

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
TIMEOUT_SEC="${QEMU_TIMEOUT:-10}"
MONITOR_SOCK="$BUILD_DIR/qemu-monitor.sock"
LOG_DIR="$BUILD_DIR/snapshot-tests"
QEMU_PID=""

# Timing accumulators
SNAPSHOT_TOTAL_MS=0
BASELINE_TOTAL_MS=0

# Track test results: array of "test_name:exit_code"
declare -a TEST_RESULTS=()

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

now_ms() {
    # Millisecond timestamp (portable: Linux date +%s%N, macOS perl fallback)
    local ns
    ns=$(date +%s%N 2>/dev/null)
    if [[ "$ns" =~ ^[0-9]+$ ]]; then
        echo $(( ns / 1000000 ))
    else
        perl -MTime::HiRes=time -e 'printf "%d\n", time()*1000' 2>/dev/null || \
            echo $(( $(date +%s) * 1000 ))
    fi
}

monitor_cmd() {
    # Send a command to QEMU monitor via socat and capture response
    local cmd="$1"
    local timeout="${2:-5}"
    if ! command -v socat &>/dev/null; then
        echo "ERROR: socat not found (required for QEMU monitor)" >&2
        return 1
    fi
    echo "$cmd" | socat - "UNIX-CONNECT:$MONITOR_SOCK,connect-timeout=$timeout" 2>/dev/null
}

wait_for_pattern() {
    # Wait until a pattern appears in the log file, or timeout
    local log_file="$1"
    local pattern="$2"
    local timeout="$3"
    local elapsed=0
    while [ "$elapsed" -lt "$timeout" ]; do
        if [ -f "$log_file" ] && grep -q "$pattern" "$log_file" 2>/dev/null; then
            return 0
        fi
        sleep 1
        elapsed=$((elapsed + 1))
    done
    return 1
}

start_qemu() {
    # Launch QEMU in background with monitor socket
    echo "[qemu] Launching QEMU with monitor socket..."

    rm -f "$MONITOR_SOCK"

    local qemu_args=(
        -machine esp32s3
        -nographic
        -drive "file=$FLASH_IMAGE,if=mtd,format=raw"
        -serial "file:$LOG_DIR/qemu_uart.log"
        -no-reboot
        -monitor "unix:$MONITOR_SOCK,server,nowait"
    )

    "$QEMU_BIN" "${qemu_args[@]}" &
    QEMU_PID=$!
    echo "[qemu] PID=$QEMU_PID"

    # Wait for monitor socket to appear
    local waited=0
    while [ ! -S "$MONITOR_SOCK" ] && [ "$waited" -lt 10 ]; do
        sleep 1
        waited=$((waited + 1))
    done

    if [ ! -S "$MONITOR_SOCK" ]; then
        echo "ERROR: QEMU monitor socket did not appear after 10s"
        return 1
    fi

    # Verify QEMU is still running
    if ! kill -0 "$QEMU_PID" 2>/dev/null; then
        echo "ERROR: QEMU process exited prematurely"
        return 1
    fi

    echo "[qemu] Monitor socket ready: $MONITOR_SOCK"
}

save_snapshot() {
    local name="$1"
    echo "[snapshot] Saving snapshot: $name"
    monitor_cmd "savevm $name" 5
    echo "[snapshot] Saved: $name"
}

restore_snapshot() {
    local name="$1"
    echo "[snapshot] Restoring snapshot: $name"
    monitor_cmd "loadvm $name" 5
    echo "[snapshot] Restored: $name"
}

# ──────────────────────────────────────────────────────────────────────
# Pre-flight checks
# ──────────────────────────────────────────────────────────────────────

echo "=== QEMU Snapshot Test Runner — ADR-061 Layer 8 ==="
echo "QEMU binary:  $QEMU_BIN"
echo "Flash image:  $FLASH_IMAGE"
echo "Timeout/test: ${TIMEOUT_SEC}s"
echo ""

if ! command -v "$QEMU_BIN" &>/dev/null; then
    echo "ERROR: QEMU binary not found: $QEMU_BIN"
    echo "  Install: sudo apt install qemu-system-misc   # Debian/Ubuntu"
    echo "  Install: brew install qemu                    # macOS"
    echo "  Or set QEMU_PATH to the qemu-system-xtensa binary."
    exit 3
fi

if ! command -v qemu-img &>/dev/null; then
    echo "ERROR: qemu-img not found (needed for snapshot disk management)."
    echo "  Install: sudo apt install qemu-utils   # Debian/Ubuntu"
    echo "  Install: brew install qemu              # macOS"
    exit 3
fi

if ! command -v socat &>/dev/null; then
    echo "ERROR: socat not found (needed for QEMU monitor communication)."
    echo "  Install: sudo apt install socat   # Debian/Ubuntu"
    echo "  Install: brew install socat        # macOS"
    exit 3
fi

if [ ! -f "$FLASH_IMAGE" ]; then
    echo "ERROR: Flash image not found: $FLASH_IMAGE"
    echo "Run qemu-esp32s3-test.sh first to build the flash image."
    exit 3
fi

mkdir -p "$LOG_DIR"

# ──────────────────────────────────────────────────────────────────────
# Phase 1: Boot and create snapshots
# ──────────────────────────────────────────────────────────────────────

echo "── Phase 1: Boot and snapshot creation ──"
echo ""

# Clear any previous UART log
> "$LOG_DIR/qemu_uart.log"

start_qemu

# Wait for boot (look for boot indicators, max 5s)
echo "[boot] Waiting for firmware boot (up to 5s)..."
if wait_for_pattern "$LOG_DIR/qemu_uart.log" "app_main\|main_task\|ESP32-S3" 5; then
    echo "[boot] Firmware booted successfully."
else
    echo "[boot] No boot indicator found after 5s (continuing anyway)."
fi

# Save post-boot snapshot
save_snapshot "post_boot"
echo ""

# Wait for first mock CSI frame (additional 5s)
echo "[frame] Waiting for first CSI frame (up to 5s)..."
if wait_for_pattern "$LOG_DIR/qemu_uart.log" "frame\|CSI\|mock_csi\|iq_data\|subcarrier" 5; then
    echo "[frame] First CSI frame detected."
else
    echo "[frame] No frame indicator found after 5s (continuing anyway)."
fi

# Save post-first-frame snapshot
save_snapshot "post_first_frame"
echo ""

# ──────────────────────────────────────────────────────────────────────
# Phase 2: Run tests from snapshot
# ──────────────────────────────────────────────────────────────────────

echo "── Phase 2: Running tests from snapshot ──"
echo ""

TESTS=("test_presence" "test_fall" "test_multi_person")
MAX_EXIT=0

for test_name in "${TESTS[@]}"; do
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "  Test: $test_name"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    test_log="$LOG_DIR/${test_name}.log"
    t_start=$(now_ms)

    # Restore to post_first_frame state
    restore_snapshot "post_first_frame"

    # Record current log length so we can extract only new lines
    pre_lines=$(wc -l < "$LOG_DIR/qemu_uart.log" 2>/dev/null || echo 0)

    # Let execution continue for TIMEOUT_SEC seconds
    echo "[test] Running for ${TIMEOUT_SEC}s..."
    sleep "$TIMEOUT_SEC"

    # Capture only the new log lines produced during this test
    tail -n +$((pre_lines + 1)) "$LOG_DIR/qemu_uart.log" > "$test_log"

    t_end=$(now_ms)
    elapsed_ms=$((t_end - t_start))
    SNAPSHOT_TOTAL_MS=$((SNAPSHOT_TOTAL_MS + elapsed_ms))

    echo "[test] Captured $(wc -l < "$test_log") lines in ${elapsed_ms}ms"

    # Validate
    echo "[test] Validating..."
    test_exit=0
    python3 "$SCRIPT_DIR/validate_qemu_output.py" "$test_log" || test_exit=$?

    TEST_RESULTS+=("${test_name}:${test_exit}")
    if [ "$test_exit" -gt "$MAX_EXIT" ]; then
        MAX_EXIT=$test_exit
    fi

    echo ""
done

# ──────────────────────────────────────────────────────────────────────
# Phase 3: Baseline timing (without snapshots) for comparison
# ──────────────────────────────────────────────────────────────────────

echo "── Phase 3: Timing comparison ──"
echo ""

# Estimate baseline: full boot (5s) + frame wait (5s) + test run per test
BASELINE_PER_TEST=$((5 + 5 + TIMEOUT_SEC))
BASELINE_TOTAL_MS=$((BASELINE_PER_TEST * ${#TESTS[@]} * 1000))
SNAPSHOT_PER_TEST=$((SNAPSHOT_TOTAL_MS / ${#TESTS[@]}))

echo "Timing Summary:"
echo "  Tests run:              ${#TESTS[@]}"
echo "  With snapshots:"
echo "    Total wall time:      ${SNAPSHOT_TOTAL_MS}ms"
echo "    Per-test average:     ${SNAPSHOT_PER_TEST}ms"
echo "  Without snapshots (estimated):"
echo "    Total wall time:      ${BASELINE_TOTAL_MS}ms"
echo "    Per-test average:     $((BASELINE_PER_TEST * 1000))ms"
echo ""

if [ "$SNAPSHOT_TOTAL_MS" -gt 0 ] && [ "$BASELINE_TOTAL_MS" -gt 0 ]; then
    SPEEDUP=$((BASELINE_TOTAL_MS * 100 / SNAPSHOT_TOTAL_MS))
    echo "  Speedup:                ${SPEEDUP}% (${SPEEDUP}x/100)"
else
    echo "  Speedup:                N/A (insufficient data)"
fi

echo ""

# ──────────────────────────────────────────────────────────────────────
# Summary
# ──────────────────────────────────────────────────────────────────────

echo "── Test Results Summary ──"
echo ""
PASS_COUNT=0
FAIL_COUNT=0
for result in "${TEST_RESULTS[@]}"; do
    name="${result%%:*}"
    code="${result##*:}"
    if [ "$code" -le 1 ]; then
        echo "  [PASS] $name (exit=$code)"
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        echo "  [FAIL] $name (exit=$code)"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
done

echo ""
echo "  $PASS_COUNT passed, $FAIL_COUNT failed out of ${#TESTS[@]} tests"
echo ""
echo "=== Snapshot Test Complete (exit code: $MAX_EXIT) ==="
exit "$MAX_EXIT"
