#!/bin/bash
# Regression tests for docker-entrypoint.sh
#
# Validates that the entrypoint script correctly handles:
# 1. No arguments → uses env var defaults
# 2. Flag arguments → prepends sensing-server binary
# 3. Explicit binary path → passes through unchanged
# 4. CSI_SOURCE env var substitution
# 5. MODELS_DIR env var propagation
#
# These tests use a stub sensing-server that just prints its args.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ENTRYPOINT="$SCRIPT_DIR/../docker/docker-entrypoint.sh"

PASS=0
FAIL=0

assert_contains() {
    local test_name="$1"
    local haystack="$2"
    local needle="$3"
    if printf '%s\n' "$haystack" | grep -qF -- "$needle"; then
        PASS=$((PASS + 1))
        echo "  ✓ $test_name"
    else
        FAIL=$((FAIL + 1))
        echo "  ✗ $test_name"
        echo "    expected to contain: $needle"
        echo "    got: $haystack"
    fi
}

assert_not_contains() {
    local test_name="$1"
    local haystack="$2"
    local needle="$3"
    if printf '%s\n' "$haystack" | grep -qF -- "$needle"; then
        FAIL=$((FAIL + 1))
        echo "  ✗ $test_name"
        echo "    expected NOT to contain: $needle"
        echo "    got: $haystack"
    else
        PASS=$((PASS + 1))
        echo "  ✓ $test_name"
    fi
}

# Create a temporary stub for /app/sensing-server that just prints args
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

STUB="$TMPDIR/sensing-server"
cat > "$STUB" << 'EOF'
#!/bin/sh
echo "EXEC_ARGS: $@"
EOF
chmod +x "$STUB"

# We'll modify the entrypoint to use our stub path for testing
TEST_ENTRYPOINT="$TMPDIR/docker-entrypoint.sh"
sed "s|/app/sensing-server|$STUB|g" "$ENTRYPOINT" > "$TEST_ENTRYPOINT"
chmod +x "$TEST_ENTRYPOINT"

echo "=== Docker entrypoint tests ==="

# Test 1: No arguments — should use CSI_SOURCE default (auto)
echo ""
echo "Test 1: No arguments (default CSI_SOURCE=auto)"
OUT=$(CSI_SOURCE=auto "$TEST_ENTRYPOINT" 2>&1)
assert_contains "includes --source auto" "$OUT" "--source auto"
assert_contains "includes --tick-ms 100" "$OUT" "--tick-ms 100"
assert_contains "includes --ui-path" "$OUT" "--ui-path /app/ui"
assert_contains "includes --http-port 3000" "$OUT" "--http-port 3000"
assert_contains "includes --ws-port 3001" "$OUT" "--ws-port 3001"
assert_contains "includes --bind-addr 0.0.0.0" "$OUT" "--bind-addr 0.0.0.0"

# Test 2: CSI_SOURCE=esp32 — should substitute
echo ""
echo "Test 2: CSI_SOURCE=esp32"
OUT=$(CSI_SOURCE=esp32 "$TEST_ENTRYPOINT" 2>&1)
assert_contains "includes --source esp32" "$OUT" "--source esp32"

# Test 3: Flag arguments — should prepend binary
echo ""
echo "Test 3: User passes --source wifi --tick-ms 500"
OUT=$(CSI_SOURCE=auto "$TEST_ENTRYPOINT" --source wifi --tick-ms 500 2>&1)
assert_contains "includes --source wifi" "$OUT" "--source wifi"
assert_contains "includes --tick-ms 500" "$OUT" "--tick-ms 500"

# Test 4: No CSI_SOURCE set — should default to auto
echo ""
echo "Test 4: CSI_SOURCE unset"
OUT=$(unset CSI_SOURCE; "$TEST_ENTRYPOINT" 2>&1)
assert_contains "includes --source auto (default)" "$OUT" "--source auto"

# Test 5: User passes --model flag — should be appended
echo ""
echo "Test 5: User passes --model /app/models/my.rvf"
OUT=$(CSI_SOURCE=esp32 "$TEST_ENTRYPOINT" --model /app/models/my.rvf 2>&1)
assert_contains "includes --model" "$OUT" "--model /app/models/my.rvf"
assert_contains "also includes default flags" "$OUT" "--source esp32"

# Test 6: CSI_SOURCE=simulated
echo ""
echo "Test 6: CSI_SOURCE=simulated"
OUT=$(CSI_SOURCE=simulated "$TEST_ENTRYPOINT" 2>&1)
assert_contains "includes --source simulated" "$OUT" "--source simulated"

# Test 7: Explicit binary path passed (e.g., docker run <image> /bin/sh)
# First arg does NOT start with -, so entrypoint should exec it directly
echo ""
echo "Test 7: Explicit command (echo hello)"
OUT=$("$TEST_ENTRYPOINT" echo hello 2>&1)
assert_contains "passes through explicit command" "$OUT" "hello"
assert_not_contains "does not inject sensing-server flags" "$OUT" "--source"

# Test 8: MODELS_DIR env var is passed through to the process
echo ""
echo "Test 8: MODELS_DIR env var propagation"
# Create a stub that prints MODELS_DIR
ENV_STUB="$TMPDIR/env-sensing-server"
cat > "$ENV_STUB" << 'ENVEOF'
#!/bin/sh
echo "MODELS_DIR=${MODELS_DIR:-unset}"
ENVEOF
chmod +x "$ENV_STUB"
ENV_ENTRYPOINT="$TMPDIR/env-entrypoint.sh"
sed "s|/app/sensing-server|$ENV_STUB|g" "$ENTRYPOINT" > "$ENV_ENTRYPOINT"
chmod +x "$ENV_ENTRYPOINT"

OUT=$(MODELS_DIR=/app/models CSI_SOURCE=auto "$ENV_ENTRYPOINT" 2>&1)
assert_contains "MODELS_DIR is visible" "$OUT" "MODELS_DIR=/app/models"

OUT=$(unset MODELS_DIR; CSI_SOURCE=auto "$ENV_ENTRYPOINT" 2>&1)
assert_contains "MODELS_DIR defaults to unset" "$OUT" "MODELS_DIR=unset"

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
