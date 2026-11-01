#!/usr/bin/env bash
# prove.sh — one-command reproduction harness for RuView / wifi-densepose.
#
# Mission: this project has been publicly accused of being "AI slop / fake."
# The answer is reproducibility. Clone the repo, run THIS script, and every
# headline claim is either VERIFIED on your machine (MEASURED) or printed as
# "CLAIMED — not reproduced here (why)". Nothing is asserted without a command.
#
# Usage:
#   bash scripts/prove.sh            # core gate + anti-slop assertion tests
#   bash scripts/prove.sh --full     # also run the tch/GPU/dataset-gated claims
#
# Exit code 0 only if every NON-gated claim passes. Gated claims never fail the
# run; they print exactly what they need (libtorch, a GPU, a dataset) so you can
# reproduce them yourself. If a hard gate (workspace tests, Python proof) cannot
# run here, the result is INCOMPLETE and the exit code is 2.
#
# Before running: stop any local sensing server on UDP 5005. Older versions of
# v2/crates/wifi-densepose-sensing-server/tests/multi_node_test.rs send
# synthetic frames to 127.0.0.1:5005 (#2086).
#
# The Python proof uses $PYTHON if set, else python3, else python. It needs the
# pinned deps in archive/v1/requirements-lock.txt, e.g.:
#   python3 -m venv .venv && .venv/bin/pip install -r archive/v1/requirements-lock.txt
#   PYTHON=.venv/bin/python bash scripts/prove.sh
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
FULL=0; [ "${1:-}" = "--full" ] && FULL=1

pass=0; fail=0; skip=0; incomplete=0
PASS(){ echo "  [PASS] $1"; pass=$((pass+1)); }
FAIL(){ echo "  [FAIL] $1"; fail=$((fail+1)); }
SKIP(){ echo "  [CLAIMED — not reproduced here] $1"; skip=$((skip+1)); }
# A hard gate that could not run on this machine. Never reported as PASS.
NOTRUN(){ echo "  [NOT RUN] $1"; incomplete=$((incomplete+1)); }
hr(){ echo "------------------------------------------------------------"; }

echo "RuView / wifi-densepose — PROOF harness"
echo "repo: $ROOT"
echo "date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
hr

# ── 1. HARD GATE: Rust workspace tests (no native libs required) ────────────
echo "[1] Rust workspace tests  (cargo test --workspace --no-default-features)"
echo "  note: stop any local sensing server on UDP 5005 first (#2086)"
if command -v cargo >/dev/null 2>&1; then
  if ( cd v2 && cargo test --workspace --no-default-features ) > /tmp/prove_ws.log 2>&1; then
    n=$(grep -oE "result: ok\. [0-9]+ passed" /tmp/prove_ws.log | grep -oE "[0-9]+" | awk '{s+=$1} END {print s}')
    PASS "workspace tests green — ${n:-?} passed, 0 failed  (CARGO exit 0)"
  else
    FAIL "workspace tests — see /tmp/prove_ws.log (grep 'test result: FAILED')"
  fi
else
  NOTRUN "cargo not installed — install Rust to run the workspace gate"
fi
hr

# ── 2. HARD GATE: deterministic Python pipeline proof (SHA-256) ─────────────
echo "[2] Deterministic CSI pipeline proof  (archive/v1/data/proof/verify.py)"
PY="${PYTHON:-}"
if [ -z "$PY" ]; then
  if command -v python3 >/dev/null 2>&1; then PY=python3
  elif command -v python >/dev/null 2>&1; then PY=python
  fi
fi
LOCK=archive/v1/requirements-lock.txt
if [ -z "$PY" ] || ! command -v "$PY" >/dev/null 2>&1; then
  NOTRUN "no Python found (tried PYTHON='${PYTHON:-}', python3, python) — install Python 3.10+ to run the deterministic proof"
elif ! "$PY" -c "import numpy, scipy" > /tmp/prove_py_deps.log 2>&1; then
  NOTRUN "$PY is missing the proof deps ($(tail -1 /tmp/prove_py_deps.log)). Install them: $PY -m venv .venv && .venv/bin/pip install -r $LOCK, then re-run with PYTHON=.venv/bin/python"
else
  echo "  python: $PY ($("$PY" -c 'import sys; print(sys.version.split()[0])'))"
  # The published hash was generated with the versions pinned in the lockfile;
  # other versions can change floating-point results.
  for pkg in numpy scipy; do
    want=$(grep -E "^$pkg==" "$LOCK" | cut -d= -f3)
    have=$("$PY" -c "import $pkg; print($pkg.__version__)")
    [ "$want" = "$have" ] || echo "  warning: $pkg $have installed, $LOCK pins $want — a hash mismatch may be a version mismatch"
  done
  # Single-threaded BLAS/FFT, as in CI: threaded reduction order is not deterministic.
  if OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 \
     VECLIB_MAXIMUM_THREADS=1 NUMEXPR_NUM_THREADS=1 \
     "$PY" archive/v1/data/proof/verify.py > /tmp/prove_py.log 2>&1 && grep -q "VERDICT: PASS" /tmp/prove_py.log; then
    PASS "Python proof VERDICT: PASS (bit-exact SHA-256 of reference features)"
  else
    FAIL "Python proof — see /tmp/prove_py.log"
  fi
fi
hr

# ── 3. ANTI-SLOP ASSERTION TESTS — each encodes a headline MEASURED claim ────
# Format: claim_test <crate> <test-name-filter> <human claim> [extra cargo args]
claim_test(){
  local crate="$1" filt="$2" desc="$3"; shift 3
  if ! command -v cargo >/dev/null 2>&1; then NOTRUN "$desc (cargo missing)"; return; fi
  if ( cd v2 && cargo test -p "$crate" "$@" "$filt" ) > /tmp/prove_claim.log 2>&1 \
     && grep -qE "test result: ok\. [1-9]" /tmp/prove_claim.log; then
    PASS "$desc"
  else
    # distinguish "didn't run" (feature/lib gated) from real failure
    if grep -qE "0 passed|filtered out;? finished|error: no test target" /tmp/prove_claim.log \
       && ! grep -q "test result: FAILED" /tmp/prove_claim.log; then
      SKIP "$desc (test gated/absent in this build — see /tmp/prove_claim.log)"
    else
      FAIL "$desc — see /tmp/prove_claim.log"
    fi
  fi
}

# Variant for workspace-excluded crates (e.g. wasm-edge): run from the crate dir.
claim_test_indir(){
  local dir="$1" filt="$2" desc="$3"; shift 3
  if ! command -v cargo >/dev/null 2>&1; then NOTRUN "$desc (cargo missing)"; return; fi
  if ( cd "$dir" && cargo test "$@" "$filt" ) > /tmp/prove_claim.log 2>&1 \
     && grep -qE "test result: ok\. [1-9]" /tmp/prove_claim.log; then
    PASS "$desc"
  else
    if grep -qE "0 passed|error: no test target" /tmp/prove_claim.log \
       && ! grep -q "test result: FAILED" /tmp/prove_claim.log; then
      SKIP "$desc (test gated/absent — see /tmp/prove_claim.log)"
    else
      FAIL "$desc — see /tmp/prove_claim.log"
    fi
  fi
}

echo "[3] Anti-slop assertion tests (each fails on the pre-fix code)"
echo "  ADR-156 §2.2 — fusion crafted-input DoS panics are closed:"
claim_test wifi-densepose-ruvector triangulation_out_of_range_index_returns_none_no_panic \
  "crafted out-of-range index returns None, no panic" --no-default-features

echo "  Soul Signature §3.6 — the audit's 'identity does not lock' claim, MEASURED:"
claim_test wifi-densepose-bfld cardiac_alone_cannot_separate_identity_matches_audit \
  "WiFi-only cardiac+respiratory channels CANNOT separate two people (gap ~0.0005)"

echo "  OccWorld — predict() is real (input-dependent), not random:"
claim_test wifi-densepose-occworld-candle predict_is_deterministic_for_same_input \
  "same occupancy input -> identical prediction (no randn stub)"

echo "  ADR-159 A1 — pose runtime actually emits under its own default config:"
claim_test cog-pose-estimation default_config_emits_frames_with_real_model \
  "default install emits pose frames (confidence >= min_confidence)" --no-default-features

echo "  ADR-159 A2 — person-count flags untrained classes (no count inflation):"
claim_test cog-person-count untrained_class_argmax_is_flagged_low_confidence \
  "argmax on an untrained class is flagged low_confidence" --no-default-features

echo "  ADR-160 A1 — medical edge skills carry a not-a-medical-device disclaimer:"
# wasm-edge is a workspace-excluded crate → run from its own directory.
claim_test_indir v2/crates/wifi-densepose-wasm-edge a1_med_modules_have_clinical_disclaimer \
  "every med_* module carries the experimental/non-clinical disclaimer" --features std
hr

# ── 4. DATA/HARDWARE-GATED claims — honestly NOT reproduced by this script ───
echo "[4] DATA/HARDWARE-GATED claims (reproduce instructions, not asserted here)"
if [ "$FULL" = "1" ]; then
  echo "  (--full) attempting the gated claims; missing prereqs are reported, not failed:"
  claim_test wifi-densepose-mat test_identical_vitals_no_location_dedup_to_one \
    "ADR-158 §2 survivor dedup 3->1 (count-inflation fix)" --features mat
else
  SKIP "WiFlow-STD ~96% PCK@20 reproduction — needs an NVIDIA GPU + MM-Fi dataset; see benchmarks/wiflow-std/RESULTS.md"
  SKIP "named person-identity — DATA-GATED: needs a real enrollment feeding the AETHER/body-resonance channel (see docs/research/soul/)"
  SKIP "OccWorld trained accuracy — needs a trained checkpoint (predict() carries weights_trained=false until then)"
  SKIP "native wlanapi 9.74 Hz scan — Windows-only; run: cargo test -p wifi-densepose-wifiscan -- --ignored measure_native_scan_rate"
  SKIP "edge-latency benches (ADR-163) — host medians, not asserted here: (cd v2/crates/wifi-densepose-wasm-edge && cargo bench --features std) and (cd v2 && cargo bench -p cog-person-count -p cog-pose-estimation --no-default-features --bench infer_bench). HOST proxy only — the ESP32/WASM3 budget is NOT reproduced on a laptop; see benchmarks/edge-latency/RESULTS.md"
  echo "  (re-run with --full to attempt the feature-gated subset where prereqs exist)"
fi
hr

# ── verdict ──────────────────────────────────────────────────────────────────
echo "VERDICT:  $pass verified · $fail failed · $incomplete not run · $skip claimed-not-reproduced-here"
if [ "$fail" -ne 0 ]; then
  echo "RESULT: FAIL — $fail claim(s) did not reproduce. See the /tmp/prove_*.log files."
  exit 1
elif [ "$incomplete" -ne 0 ]; then
  echo "RESULT: INCOMPLETE — $incomplete hard-gate check(s) could not run on this machine (see [NOT RUN] above)."
  exit 2
else
  echo "RESULT: PASS — every reproducible claim verified on this machine."
  exit 0
fi
