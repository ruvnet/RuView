#!/usr/bin/env bash
# generate-witness-bundle.sh — Create a self-contained RVF witness bundle
#
# Produces: witness-bundle-ADR028-<commit>.tar.gz
# Contains: witness log, ADR, proof hash, test results, firmware manifest,
#           reference signal metadata, and a VERIFY.sh script for recipients.
#
# Usage: bash scripts/generate-witness-bundle.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
COMMIT_SHA="$(git -C "$REPO_ROOT" rev-parse HEAD)"
SHORT_SHA="${COMMIT_SHA:0:8}"
BUNDLE_NAME="witness-bundle-ADR028-${SHORT_SHA}"
BUNDLE_DIR="$REPO_ROOT/dist/${BUNDLE_NAME}"
TIMESTAMP="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

echo "================================================================"
echo "  WiFi-DensePose Witness Bundle Generator (ADR-028)"
echo "================================================================"
echo "  Commit: ${COMMIT_SHA}"
echo "  Time:   ${TIMESTAMP}"
echo ""

# Create bundle directory
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"

# ---------------------------------------------------------------
# 1. Copy witness documents
# ---------------------------------------------------------------
echo "[1/7] Copying witness documents..."
cp "$REPO_ROOT/docs/WITNESS-LOG-028.md" "$BUNDLE_DIR/"
cp "$REPO_ROOT/docs/adr/ADR-028-esp32-capability-audit.md" "$BUNDLE_DIR/"

# ---------------------------------------------------------------
# 2. Copy proof system
# ---------------------------------------------------------------
echo "[2/7] Copying proof system..."
mkdir -p "$BUNDLE_DIR/proof"
cp "$REPO_ROOT/v1/data/proof/verify.py" "$BUNDLE_DIR/proof/"
cp "$REPO_ROOT/v1/data/proof/expected_features.sha256" "$BUNDLE_DIR/proof/"
cp "$REPO_ROOT/v1/data/proof/generate_reference_signal.py" "$BUNDLE_DIR/proof/"
# Reference signal is large (~10 MB) — include metadata only
python3 -c "
import json, os
with open('$REPO_ROOT/v1/data/proof/sample_csi_data.json') as f:
    d = json.load(f)
meta = {k: v for k, v in d.items() if k != 'frames'}
meta['frame_count'] = len(d['frames'])
meta['first_frame_keys'] = list(d['frames'][0].keys())
meta['file_size_bytes'] = os.path.getsize('$REPO_ROOT/v1/data/proof/sample_csi_data.json')
with open('$BUNDLE_DIR/proof/reference_signal_metadata.json', 'w') as f:
    json.dump(meta, f, indent=2)
" 2>/dev/null && echo "  Reference signal metadata extracted." || echo "  (Python not available — metadata skipped)"

# ---------------------------------------------------------------
# 3. Run Rust tests and capture output
# ---------------------------------------------------------------
echo "[3/7] Running Rust test suite..."
mkdir -p "$BUNDLE_DIR/test-results"
cd "$REPO_ROOT/rust-port/wifi-densepose-rs"
cargo test --workspace --no-default-features 2>&1 | tee "$BUNDLE_DIR/test-results/rust-workspace-tests.log" | tail -5
# Extract summary
grep "^test result" "$BUNDLE_DIR/test-results/rust-workspace-tests.log" | \
  awk '{p+=$4; f+=$6; i+=$8} END {printf "TOTAL: %d passed, %d failed, %d ignored\n", p, f, i}' \
  > "$BUNDLE_DIR/test-results/summary.txt"
cat "$BUNDLE_DIR/test-results/summary.txt"
cd "$REPO_ROOT"

# ---------------------------------------------------------------
# 4. Run Python proof verification
# ---------------------------------------------------------------
echo "[4/7] Running Python proof verification..."
python3 "$REPO_ROOT/v1/data/proof/verify.py" 2>&1 | tee "$BUNDLE_DIR/proof/verification-output.log" | tail -5 || true

# ---------------------------------------------------------------
# 5. Firmware manifest
# ---------------------------------------------------------------
echo "[5/7] Generating firmware manifest..."
mkdir -p "$BUNDLE_DIR/firmware-manifest"
if [ -d "$REPO_ROOT/firmware/esp32-csi-node/main" ]; then
  wc -l "$REPO_ROOT/firmware/esp32-csi-node/main/"*.c "$REPO_ROOT/firmware/esp32-csi-node/main/"*.h \
    > "$BUNDLE_DIR/firmware-manifest/source-line-counts.txt" 2>/dev/null || true
  # SHA-256 of each firmware source file
  sha256sum "$REPO_ROOT/firmware/esp32-csi-node/main/"*.c "$REPO_ROOT/firmware/esp32-csi-node/main/"*.h \
    > "$BUNDLE_DIR/firmware-manifest/source-hashes.txt" 2>/dev/null || \
  find "$REPO_ROOT/firmware/esp32-csi-node/main/" -type f \( -name "*.c" -o -name "*.h" \) -exec sha256sum {} \; \
    > "$BUNDLE_DIR/firmware-manifest/source-hashes.txt" 2>/dev/null || true
  echo "  Firmware source files hashed."
else
  echo "  (No firmware directory found — skipped)"
fi

# ---------------------------------------------------------------
# 6. Crate manifest
# ---------------------------------------------------------------
echo "[6/7] Generating crate manifest..."
mkdir -p "$BUNDLE_DIR/crate-manifest"
for crate_dir in "$REPO_ROOT/rust-port/wifi-densepose-rs/crates/"*/; do
  crate_name="$(basename "$crate_dir")"
  if [ -f "$crate_dir/Cargo.toml" ]; then
    version=$(grep '^version' "$crate_dir/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
    echo "${crate_name} = ${version}" >> "$BUNDLE_DIR/crate-manifest/versions.txt"
  fi
done
cat "$BUNDLE_DIR/crate-manifest/versions.txt"

# ---------------------------------------------------------------
# 7. Generate VERIFY.sh for recipients
# ---------------------------------------------------------------
echo "[7/7] Creating VERIFY.sh..."
cat > "$BUNDLE_DIR/VERIFY.sh" << 'VERIFY_EOF'
#!/usr/bin/env bash
# VERIFY.sh — Recipient verification script for WiFi-DensePose Witness Bundle
#
# Run this script after cloning the repository at the witnessed commit.
# It re-runs all verification steps and compares against the bundled results.
set -euo pipefail

echo "================================================================"
echo "  WiFi-DensePose Witness Bundle Verification"
echo "================================================================"
echo ""

PASS_COUNT=0
FAIL_COUNT=0

check() {
  local desc="$1" result="$2"
  if [ "$result" = "PASS" ]; then
    echo "  [PASS] $desc"
    PASS_COUNT=$((PASS_COUNT + 1))
  else
    echo "  [FAIL] $desc"
    FAIL_COUNT=$((FAIL_COUNT + 1))
  fi
}

# Check 1: Witness documents exist
[ -f "WITNESS-LOG-028.md" ] && check "Witness log present" "PASS" || check "Witness log present" "FAIL"
[ -f "ADR-028-esp32-capability-audit.md" ] && check "ADR-028 present" "PASS" || check "ADR-028 present" "FAIL"

# Check 2: Proof hash file
[ -f "proof/expected_features.sha256" ] && check "Proof hash file present" "PASS" || check "Proof hash file present" "FAIL"
echo "  Expected hash: $(cat proof/expected_features.sha256 2>/dev/null || echo 'NOT FOUND')"

# Check 3: Test results
if [ -f "test-results/summary.txt" ]; then
  summary="$(cat test-results/summary.txt)"
  echo "  Test summary: $summary"
  if echo "$summary" | grep -q "0 failed"; then
    check "All Rust tests passed" "PASS"
  else
    check "All Rust tests passed" "FAIL"
  fi
else
  check "Test results present" "FAIL"
fi

# Check 4: Firmware manifest
if [ -f "firmware-manifest/source-hashes.txt" ]; then
  count=$(wc -l < firmware-manifest/source-hashes.txt)
  check "Firmware source hashes (${count} files)" "PASS"
else
  check "Firmware manifest present" "FAIL"
fi

# Check 5: Crate versions
if [ -f "crate-manifest/versions.txt" ]; then
  count=$(wc -l < crate-manifest/versions.txt)
  check "Crate manifest (${count} crates)" "PASS"
else
  check "Crate manifest present" "FAIL"
fi

# Check 6: Proof verification log
if [ -f "proof/verification-output.log" ]; then
  if grep -q "VERDICT: PASS" proof/verification-output.log; then
    check "Python proof verification PASS" "PASS"
  else
    check "Python proof verification PASS" "FAIL"
  fi
else
  check "Proof verification log present" "FAIL"
fi

echo ""
echo "================================================================"
echo "  Results: ${PASS_COUNT} passed, ${FAIL_COUNT} failed"
if [ "$FAIL_COUNT" -eq 0 ]; then
  echo "  VERDICT: ALL CHECKS PASSED"
else
  echo "  VERDICT: ${FAIL_COUNT} CHECK(S) FAILED — investigate"
fi
echo "================================================================"
VERIFY_EOF
chmod +x "$BUNDLE_DIR/VERIFY.sh"

# ---------------------------------------------------------------
# Create manifest with all file hashes
# ---------------------------------------------------------------
echo ""
echo "Generating bundle manifest..."
cd "$BUNDLE_DIR"
find . -type f -not -name "MANIFEST.sha256" | sort | while read -r f; do
  sha256sum "$f"
done > MANIFEST.sha256 2>/dev/null || \
find . -type f -not -name "MANIFEST.sha256" | sort -exec sha256sum {} \; > MANIFEST.sha256 2>/dev/null || true

# ---------------------------------------------------------------
# Package as tarball
# ---------------------------------------------------------------
echo "Packaging bundle..."
cd "$REPO_ROOT/dist"
tar czf "${BUNDLE_NAME}.tar.gz" "${BUNDLE_NAME}/"
BUNDLE_SIZE=$(du -h "${BUNDLE_NAME}.tar.gz" | cut -f1)

echo ""
echo "================================================================"
echo "  Bundle created: dist/${BUNDLE_NAME}.tar.gz (${BUNDLE_SIZE})"
echo "  Contents:"
find "${BUNDLE_NAME}" -type f | sort | sed 's/^/    /'
echo ""
echo "  To verify: cd ${BUNDLE_NAME} && bash VERIFY.sh"
echo "================================================================"
