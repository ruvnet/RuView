#!/usr/bin/env bash
# Empty-room runtime calibration, one command.
#
# Why this exists: a finalized runtime calibration is the only thing that satisfies
# `explicit_calibration_fresh_at`, and it is *not* persisted — a server restart
# returns `/api/v1/calibration/status` to `none`. The bootstrap/promote route
# stores an image, but a restored image is deliberately a "bootstrap prior" with
# negative-only authority, so promoting does not keep the vitals gate open
# (`field_model.rs`: "A restored image is only a bootstrap prior").
#
# The collection itself needs 600 s and 1000 frames. The duration is not padding:
# `FieldModelConfig::min_calibration_duration_s` documents that slow environmental
# variation (HVAC cycles, thermal drift) can only be observed over time, however
# fast the nodes stream (#1756).
#
# Two ways to waste the ten minutes, both handled here:
#   1. binding to a subcarrier grid the node then stops emitting (the grid is
#      chosen from a 20 s evidence window, and these nodes interleave 192- and
#      306-bin frames) — so this picks the eligible source with the highest
#      measured rate and verifies the feed before committing;
#   2. starting while a person is still in the room — so it asks first.
#
# Usage:
#   ./calibrate-room.sh                 # interactive, asks you to confirm the room is empty
#   ./calibrate-room.sh --yes           # no prompt (scripted)
#   ./calibrate-room.sh --base http://host:8080
set -uo pipefail

BASE="http://localhost:8080"
ASSUME_YES=0
POLL_S=20

while [ $# -gt 0 ]; do
  case "$1" in
    --yes|-y) ASSUME_YES=1; shift ;;
    --base) BASE="$2"; shift 2 ;;
    -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

say()  { printf '%s\n' "$*"; }
fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

command -v curl >/dev/null || fail "curl is required"
command -v python3 >/dev/null || fail "python3 is required"

say "== preflight =="
curl -sf --max-time 5 "$BASE/health" >/dev/null || fail "no sensing server at $BASE (is it running?)"

STATUS=$(curl -sf --max-time 5 "$BASE/api/v1/calibration/status") || fail "cannot read calibration status"
STATE=$(printf '%s' "$STATUS" | python3 -c 'import sys,json;print(json.load(sys.stdin).get("status","?"))')
ACTIVE=$(printf '%s' "$STATUS" | python3 -c 'import sys,json;print(str(json.load(sys.stdin).get("active",False)).lower())')
say "server ok, calibration status: $STATE (active: $ACTIVE)"
if [ "$ACTIVE" = "true" ]; then
  fail "a calibration is already collecting. Finish it with: curl -X POST $BASE/api/v1/calibration/stop
       or discard it with: curl -X POST $BASE/api/v1/calibration/reset"
fi

say
say "This collects a 10 minute empty-room baseline (600 s, 1000 frames minimum)."
say "The room must stay empty for the whole window: a person present during"
say "collection becomes part of the 'empty' reference and poisons every later"
say "comparison, including the occupancy estimate and the vitals gate."
if [ "$ASSUME_YES" != "1" ]; then
  printf 'Is the room empty now and will stay empty? [y/N] '
  read -r reply
  case "$reply" in [yY]*) ;; *) fail "aborted by operator" ;; esac
fi

say
say "== choosing a source =="
# A start call without a source node returns the eligible list instead of starting.
LIST=$(curl -sf --max-time 10 -X POST "$BASE/api/v1/calibration/start") || fail "calibration start probe failed"
NODE=$(printf '%s' "$LIST" | python3 -c '
import sys, json
d = json.load(sys.stdin)
srcs = d.get("eligible_sources") or []
if not srcs:
    sys.stderr.write("no eligible source: " + json.dumps(d)[:400] + "\n")
    sys.exit(1)
for s in sorted(srcs, key=lambda s: -s["evidence"]["rate_hz"]):
    print("# %d-sub grid, node %d, %.1f Hz, max_gap %.2f s"
          % (s["grid"]["n_subcarriers"], s["source_node_id"],
             s["evidence"]["rate_hz"], s["evidence"]["max_gap_s"]), file=sys.stderr)
print(max(srcs, key=lambda s: s["evidence"]["rate_hz"])["source_node_id"])
') || fail "no eligible calibration source (are the nodes streaming?)"
say "picked node $NODE (highest documented grid rate)"

say
say "== starting =="
START=$(curl -sf --max-time 10 -X POST "$BASE/api/v1/calibration/start?source_node_id=$NODE") || fail "start failed"
printf '%s' "$START" | python3 -c '
import sys, json
d = json.load(sys.stdin)
print("  model_id:", d.get("model_id"))
print("  grid:    ", json.dumps(d.get("source_grid")))
print("  nodes:   ", json.dumps(d.get("source_node_ids")))
' || fail "unexpected start response: $START"

say
say "== verifying the feed (30 s) =="
sleep 30
V=$(curl -sf --max-time 5 "$BASE/api/v1/calibration/status")
RATE=$(printf '%s' "$V" | python3 -c 'import sys,json;print("%.2f" % (json.load(sys.stdin).get("frames_per_second") or 0))')
STALLED=$(printf '%s' "$V" | python3 -c 'import sys,json;print(str(json.load(sys.stdin).get("stalled")).lower())')
say "  collection rate: $RATE Hz, stalled: $STALLED"
if [ "$STALLED" = "true" ] || [ "$(printf '%.0f' "$RATE")" -lt 2 ]; then
  curl -sf --max-time 10 -X POST "$BASE/api/v1/calibration/reset" >/dev/null
  fail "the bound grid is not arriving (rate ${RATE} Hz). The calibration was reset:
       re-run this script — it ranks sources by rate, so a second run usually binds
       the grid the node is actually emitting."
fi

say
say "== collecting (do not enter the room) =="
DEADLINE=$(( $(date +%s) + 1800 ))
while :; do
  V=$(curl -sf --max-time 5 "$BASE/api/v1/calibration/status") || fail "status read failed"
  read -r ACTIVE ELAPSED NEED FRAMES RATE STALLED <<EOF
$(printf '%s' "$V" | python3 -c '
import sys, json
d = json.load(sys.stdin)
print(str(d.get("active", False)).lower(),
      "%.0f" % (d.get("elapsed_s") or 0),
      "%.0f" % (d.get("min_duration_s") or 600),
      d.get("frame_count") or 0,
      "%.1f" % (d.get("frames_per_second") or 0),
      str(d.get("stalled")).lower())
')
EOF
  say "  ${ELAPSED}/${NEED} s  frames=${FRAMES}  ${RATE} Hz  stalled=${STALLED}"
  if [ "$STALLED" = "true" ]; then
    curl -sf --max-time 10 -X POST "$BASE/api/v1/calibration/reset" >/dev/null
    fail "collection stalled mid-run; reset. Re-run when the nodes are streaming steadily."
  fi
  if [ "$ACTIVE" != "true" ]; then
    say "  collection is no longer active — finalizing what was collected"
    break
  fi
  [ "$ELAPSED" -ge "$NEED" ] && break
  [ "$(date +%s)" -gt "$DEADLINE" ] && fail "gave up after 30 minutes"
  sleep "$POLL_S"
done

say
say "== finalizing =="
STOP=$(curl -sf --max-time 20 -X POST "$BASE/api/v1/calibration/stop") || fail "stop failed"
printf '%s' "$STOP" | python3 -c '
import sys, json
d = json.load(sys.stdin)
if d.get("success"):
    print("  success: model_id", d.get("model_id"), "frames", d.get("frame_count"))
else:
    print("  FAILED:", d.get("error_code"), "-", d.get("error"))
    print("  (a sequence discontinuity or a grid that went stale mid-run needs a fresh run)")
    sys.exit(1)
' || exit 1

say
say "== resulting state =="
curl -sf --max-time 5 "$BASE/api/v1/calibration/status" | python3 -c '
import sys, json
d = json.load(sys.stdin)
print("  calibration:", d.get("status"), "| binding:", d.get("binding_mode"))
print("  NOTE: Fresh is valid for 12 h of wall clock (Stale until 24 h), and a server")
print("        restart discards it — the model is not persisted.")
'
curl -sf --max-time 5 "$BASE/api/v1/vital-signs" | python3 -c '
import sys, json
d = json.load(sys.stdin)
print("  vitals authority:", d.get("authority"))
if d.get("authority") != "explicit_calibration":
    print("  abstention:", d.get("abstention_reason"))
    print("  (the gate also needs exactly one occupant and qualified confidence)")
else:
    print("  published:", json.dumps(d.get("vital_signs")))
'
say
say "done."
